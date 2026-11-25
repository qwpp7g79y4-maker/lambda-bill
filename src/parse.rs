use std::path::{Component, Path};

use flate2::bufread::GzDecoder;
use serde_json::Value;
use std::io::Read;

use crate::arch::Arch;
use crate::error::{is_unsafe_display_char, show_path, Error};
use crate::MAX_FILE_BYTES;
use crate::MAX_LINE_BYTES;

/// One invocation, after the function name and architecture are known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub function: String,
    pub arch: Arch,
    pub memory_mb: u32,
    pub max_used_mb: u32,
    pub billed_ms: u64,
    pub cold: bool,
    pub request_id: String,
}

#[derive(Clone)]
pub(crate) struct ParseInput<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
    pub strict: bool,
    pub default_arch: Arch,
    pub default_function: Option<&'a str>,
    pub functions: &'a std::collections::BTreeMap<String, Arch>,
}

#[derive(Debug)]
pub(crate) struct ParseOutput {
    pub invocations: Vec<Invocation>,
    pub skipped: Option<String>,
}

pub(crate) fn parse_input(input: &ParseInput<'_>) -> Result<ParseOutput, Error> {
    let name = show_path(Path::new(input.name));
    let decoded = match decode(&name, input.bytes) {
        Ok(text) => text,
        Err(DecodeFault::Utf8) if !input.strict => {
            return Ok(ParseOutput {
                invocations: Vec::new(),
                skipped: Some(format!("skipped {name}: not UTF-8")),
            });
        }
        Err(fault) => return Err(fault.into_error(&name)),
    };
    let path_names = function_names_in_path(Path::new(input.name))?;
    let events = parse_text(&name, &decoded)?;
    if events.reports.is_empty() {
        if input.strict {
            return Err(Error::new(format!("{name}: no REPORT lines")));
        }
        return Ok(ParseOutput {
            invocations: Vec::new(),
            skipped: Some(format!("skipped {name}: no REPORT lines")),
        });
    }
    let invocations = resolve(input, &events, path_names)?;
    Ok(ParseOutput {
        invocations,
        skipped: None,
    })
}

#[must_use]
pub(crate) fn valid_function_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn function_names_in_path(path: &Path) -> Result<Option<String>, Error> {
    let comps: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(text) => Some(text.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let mut found: Option<String> = None;
    for window in comps.windows(3) {
        if window[0] == "aws" && window[1] == "lambda" && valid_function_name(&window[2]) {
            if let Some(prev) = &found {
                if prev != &window[2] {
                    return Err(Error::new(format!(
                        "{}: function name conflict",
                        show_path(path)
                    )));
                }
            }
            found = Some(window[2].clone());
        }
    }
    Ok(found)
}

/// The most gzip members one file may have.
const MAX_GZIP_MEMBERS: u64 = 1_000_000;

enum DecodeFault {
    Utf8,
    Gzip,
    TooBig,
    Members,
}

impl DecodeFault {
    fn into_error(self, name: &str) -> Error {
        let detail = match self {
            Self::Utf8 => "not UTF-8",
            Self::Gzip => "could not read gzip",
            Self::TooBig => "file is larger than 268435456 bytes",
            Self::Members => "more than 1000000 gzip members",
        };
        Error::new(format!("{name}: {detail}"))
    }
}

fn decode(name: &str, bytes: &[u8]) -> Result<String, DecodeFault> {
    let _ = name;
    let plain = if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        // One member at a time, so the members can be counted: every member
        // starts a new decoder, and millions of empty ones take minutes.
        let mut rest = bytes;
        let mut out = Vec::new();
        let mut members = 0u64;
        while !rest.is_empty() {
            members += 1;
            if members > MAX_GZIP_MEMBERS {
                return Err(DecodeFault::Members);
            }
            let room = MAX_FILE_BYTES + 1 - out.len() as u64;
            let mut decoder = GzDecoder::new(&mut rest).take(room);
            decoder
                .read_to_end(&mut out)
                .map_err(|_| DecodeFault::Gzip)?;
            if out.len() as u64 > MAX_FILE_BYTES {
                return Err(DecodeFault::TooBig);
            }
        }
        out
    } else {
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(DecodeFault::TooBig);
        }
        bytes.to_vec()
    };
    String::from_utf8(plain).map_err(|_| DecodeFault::Utf8)
}

struct RawReport {
    label: String,
    request_id: String,
    memory_mb: u32,
    max_used_mb: u32,
    billed_ms: u64,
    cold: bool,
    event_function: Option<String>,
    event_arch: Option<Arch>,
}

struct Events {
    reports: Vec<RawReport>,
    header_function: Option<(String, String)>,
    header_arch: Option<(Arch, String)>,
}

fn parse_text(name: &str, text: &str) -> Result<Events, Error> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lead = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
    if lead.starts_with('{') || lead.starts_with('[') {
        // Counted before it is built: a small text can make a huge `Value`.
        if !crate::budget::within_budget(text) {
            return Err(Error::new(format!(
                "{name}: more than {} JSON values",
                crate::budget::MAX_JSON_UNITS
            )));
        }
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            // A repeated key can change the value serde keeps, so scan first.
            reject_duplicate_keys(name, text)?;
            if is_envelope(&value) {
                let events = events_from_document(name, &value)?;
                // A REPORT in a field that is not read must not vanish.
                reject_uncounted_marker(
                    name,
                    "document",
                    markers_in(&value),
                    events.reports.len(),
                )?;
                return Ok(events);
            }
            // A whole document that is one object with `@message` or `Message`.
            if let Some(message) = alternate_message(&value) {
                let mut events = Events {
                    reports: Vec::new(),
                    header_function: None,
                    header_arch: None,
                };
                ingest_alternate(name, "event 1", &value, message, &mut events)?;
                reject_uncounted_marker(name, "event 1", markers_in(&value), events.reports.len())?;
                return Ok(events);
            }
        }
    }
    parse_lines(name, text)
}

/// The text of a JSON object that has no envelope but a `@message` or
/// `Message` string.
fn alternate_message(value: &Value) -> Option<&str> {
    ["@message", "Message"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn reject_duplicate_keys(place: &str, text: &str) -> Result<(), Error> {
    match crate::price::log_json_has_duplicate_key(text) {
        Ok(false) => Ok(()),
        Ok(true) => Err(Error::new(format!("{place}: duplicate key"))),
        // Serde already accepted this text. A scan fault must not pass as clean.
        Err(()) => Err(Error::new(format!("{place}: not JSON"))),
    }
}

fn is_envelope(value: &Value) -> bool {
    match value {
        Value::Array(_) => true,
        Value::Object(map) => {
            map.get("message").and_then(Value::as_str).is_some()
                || map.get("events").and_then(Value::as_array).is_some()
        }
        _ => false,
    }
}

fn events_from_document(name: &str, value: &Value) -> Result<Events, Error> {
    let mut events = Events {
        reports: Vec::new(),
        header_function: None,
        header_arch: None,
    };
    match value {
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                ingest_event(name, &format!("event {}", index + 1), item, &mut events)?;
            }
        }
        Value::Object(map) if map.contains_key("events") => {
            if map.contains_key("message") {
                return Err(Error::new(format!(
                    "{name}: event 1: has both message and events"
                )));
            }
            let Some(items) = map.get("events").and_then(Value::as_array) else {
                return Err(Error::new(format!("{name}: event 1: not a log event")));
            };
            for (index, item) in items.iter().enumerate() {
                ingest_event(name, &format!("event {}", index + 1), item, &mut events)?;
            }
        }
        other => ingest_event(name, "event 1", other, &mut events)?,
    }
    Ok(events)
}

fn ingest_event(name: &str, label: &str, value: &Value, events: &mut Events) -> Result<(), Error> {
    let Some(obj) = value.as_object() else {
        return Err(Error::new(format!("{name}: {label}: not a log event")));
    };
    let Some(message) = obj.get("message").and_then(Value::as_str) else {
        return Err(Error::new(format!("{name}: {label}: not a log event")));
    };
    if obj.contains_key("events") {
        return Err(Error::new(format!(
            "{name}: {label}: has both message and events"
        )));
    }
    let (event_function, event_arch) = event_meta(name, label, obj)?;
    ingest_message(name, label, message, event_function, event_arch, events)
}

/// The function (from `logGroup` or `logGroupName`) and the architecture
/// (from `architecture`) an event object names.
fn event_meta(
    name: &str,
    label: &str,
    obj: &serde_json::Map<String, Value>,
) -> Result<(Option<String>, Option<Arch>), Error> {
    let function = match log_group_of(obj) {
        Ok(function) => function,
        Err(LogGroupFault::Differ) => {
            return Err(Error::new(format!(
                "{name}: {label}: logGroup and logGroupName differ"
            )));
        }
        Err(LogGroupFault::NotAFunction) => {
            return Err(Error::new(format!(
                "{name}: {label}: log group is not /aws/lambda/<name>"
            )));
        }
    };
    let arch = match obj.get("architecture") {
        None => None,
        Some(value) => {
            let Some(arch) = value.as_str().and_then(Arch::parse) else {
                return Err(Error::new(format!("{name}: {label}: bad architecture")));
            };
            Some(arch)
        }
    };
    Ok((function, arch))
}

/// An object with `@message` or `Message` instead of `message`. It names its
/// function and architecture the same way an event does.
fn ingest_alternate(
    name: &str,
    label: &str,
    value: &Value,
    message: &str,
    events: &mut Events,
) -> Result<(), Error> {
    let (function, arch) = match value.as_object() {
        Some(obj) => event_meta(name, label, obj)?,
        None => (None, None),
    };
    ingest_message(name, label, message, function, arch, events)
}

fn ingest_message(
    name: &str,
    label: &str,
    message: &str,
    event_function: Option<String>,
    event_arch: Option<Arch>,
    events: &mut Events,
) -> Result<(), Error> {
    for line in message.split('\n') {
        if line.len() > MAX_LINE_BYTES {
            return Err(Error::new(format!(
                "{name}: {label}: line is longer than 1048576 bytes"
            )));
        }
        match parse_report_line(line) {
            Ok(Some(raw)) => events.reports.push(RawReport {
                label: label.to_string(),
                request_id: raw.request_id,
                memory_mb: raw.memory_mb,
                max_used_mb: raw.max_used_mb,
                billed_ms: raw.billed_ms,
                cold: raw.cold,
                event_function: event_function.clone(),
                event_arch,
            }),
            Ok(None) => {}
            Err(()) => {
                return Err(Error::new(format!(
                    "{name}: {label}: not a Lambda REPORT line"
                )));
            }
        }
    }
    Ok(())
}

enum LogGroupFault {
    NotAFunction,
    Differ,
}

fn group_text(value: &Value) -> Result<&str, LogGroupFault> {
    value.as_str().ok_or(LogGroupFault::NotAFunction)
}

fn log_group_of(obj: &serde_json::Map<String, Value>) -> Result<Option<String>, LogGroupFault> {
    let group = match (obj.get("logGroup"), obj.get("logGroupName")) {
        (None, None) => return Ok(None),
        (Some(value), None) | (None, Some(value)) => group_text(value)?,
        (Some(left), Some(right)) => {
            let left = group_text(left)?;
            let right = group_text(right)?;
            if left != right {
                return Err(LogGroupFault::Differ);
            }
            left
        }
    };
    function_from_log_group(group)
        .map(Some)
        .map_err(|()| LogGroupFault::NotAFunction)
}

fn function_from_log_group(group: &str) -> Result<String, ()> {
    let group = group
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .trim_end_matches('/');
    let name = group
        .strip_prefix("/aws/lambda/")
        .or_else(|| group.strip_prefix("aws/lambda/"))
        .ok_or(())?;
    if name.contains('/') || !valid_function_name(name) {
        return Err(());
    }
    Ok(name.to_string())
}

fn parse_lines(name: &str, text: &str) -> Result<Events, Error> {
    let mut events = Events {
        reports: Vec::new(),
        header_function: None,
        header_arch: None,
    };
    for (index, line) in text.split('\n').enumerate() {
        let line_no = index + 1;
        if line.len() > MAX_LINE_BYTES {
            return Err(Error::new(format!(
                "{name}: line {line_no}: line is longer than 1048576 bytes"
            )));
        }
        let trimmed = line.trim_matches(|c: char| c.is_ascii_whitespace());
        if trimmed.is_empty() {
            continue;
        }
        // Built after the blank-line check: a file of empty lines costs nothing.
        let label = format!("line {line_no}");
        // Only ASCII white space is stripped. A leftover NBSP, U+3000 or
        // U+2028 at either edge is an error, not a line that silently counts.
        if trimmed.starts_with(|c: char| c.is_whitespace())
            || trimmed.ends_with(|c: char| c.is_whitespace())
        {
            return Err(Error::new(format!(
                "{name}: {label}: non-ASCII white space around line"
            )));
        }
        // An invisible character (BOM, zero-width space, bidi mark, ...) in
        // front of a header, a JSON line or a REPORT would make the line a
        // comment to skip. That moves requests to the wrong function or
        // architecture, or drops them, so it is an error.
        if trimmed.starts_with(is_unsafe_display_char) {
            return Err(Error::new(format!(
                "{name}: {label}: invisible character at the start of the line"
            )));
        }
        if trimmed.starts_with('#') {
            apply_header(name, &label, trimmed, &mut events)?;
            continue;
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            let parsed = serde_json::from_str::<Value>(trimmed);
            if parsed.is_err() && trimmed.starts_with('[') && !looks_like_json_array(trimmed) {
                // A bracket prefix such as `[2024-01-01]`, `[INFO]` or `[main]` is text.
                // A line that opens like a JSON array and is not JSON is an error below.
                let before = events.reports.len();
                parse_text_line(name, label.clone(), line, &mut events)?;
                let found = trimmed.matches(REPORT_MARKER).count();
                reject_uncounted_marker(name, &label, found, events.reports.len() - before)?;
                continue;
            }
            // A line that opens an object but is not JSON must not be dropped
            // silently: that would lower the bill and skip the duplicate-key scan.
            let Ok(value) = parsed else {
                return Err(Error::new(format!("{name}: {label}: not JSON")));
            };
            reject_duplicate_keys(&format!("{name}: {label}"), trimmed)?;
            let before = events.reports.len();
            if is_envelope(&value) {
                if value.get("events").is_some() || value.is_array() {
                    let doc = events_from_document(name, &value)?;
                    events.reports.extend(doc.reports);
                } else {
                    ingest_event(name, &label, &value, &mut events)?;
                }
            } else if let Some(message) = alternate_message(&value) {
                ingest_alternate(name, &label, &value, message, &mut events)?;
            }
            // A REPORT inside a JSON line that yields no request must not vanish.
            reject_uncounted_marker(
                name,
                &label,
                markers_in(&value),
                events.reports.len() - before,
            )?;
            continue;
        }
        parse_text_line(name, label, line, &mut events)?;
    }
    Ok(events)
}

/// True when a line that starts with `[` opens like a JSON array: the first
/// thing after the bracket, past the JSON spaces (space, tab, CR, LF), is `{`, `[`, `]`, a string, a JSON literal, or a
/// number-only run (`[1,2`). A bracket prefix such as `[INFO]`, `[main]`,
/// `[12:00:01]` or `[2024-01-01]` does not, so it stays text.
fn looks_like_json_array(line: &str) -> bool {
    let rest = line[1..].trim_start_matches([' ', '\t', '\r', '\n']);
    let Some(first) = rest.chars().next() else {
        return true;
    };
    match first {
        '{' | '[' | ']' | '"' => true,
        '-' | '0'..='9' => {
            let head = rest.split(']').next().unwrap_or(rest);
            // `[2024-01-01]` is a date, not a number list.
            !starts_with_bracket_date(line)
                && head.chars().all(|c| {
                    c.is_ascii_digit()
                        || matches!(c, ',' | '.' | '+' | '-' | 'e' | 'E' | ' ' | '\t')
                })
        }
        _ => ["true", "false", "null"].iter().any(|word| {
            rest.strip_prefix(word).is_some_and(|tail| {
                !tail.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            })
        }),
    }
}

/// True when the line opens with `[YYYY-MM-DD`, the start of a bracket timestamp.
fn starts_with_bracket_date(line: &str) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 11
        && bytes[0] == b'['
        && bytes[1..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'-'
        && bytes[6..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'-'
        && bytes[9..11].iter().all(u8::is_ascii_digit)
}

const REPORT_MARKER: &str = "REPORT RequestId:";

/// The number of `REPORT RequestId:` markers in every string and key of a
/// decoded JSON value. A JSON escape such as `\u0020` is decoded first, so
/// it cannot hide a marker from the check.
fn markers_in(value: &Value) -> usize {
    match value {
        Value::String(text) => text.matches(REPORT_MARKER).count(),
        Value::Array(items) => items.iter().map(markers_in).sum(),
        Value::Object(map) => map
            .iter()
            .map(|(key, item)| key.matches(REPORT_MARKER).count() + markers_in(item))
            .sum(),
        _ => 0,
    }
}

/// Every marker in a JSON or bracket line must be a counted report.
fn reject_uncounted_marker(
    name: &str,
    label: &str,
    found: usize,
    counted: usize,
) -> Result<(), Error> {
    if found > counted {
        return Err(Error::new(format!(
            "{name}: {label}: not a Lambda REPORT line"
        )));
    }
    Ok(())
}

fn parse_text_line(
    name: &str,
    label: String,
    line: &str,
    events: &mut Events,
) -> Result<(), Error> {
    match parse_report_line(line) {
        Ok(Some(raw)) => events.reports.push(RawReport {
            label,
            request_id: raw.request_id,
            memory_mb: raw.memory_mb,
            max_used_mb: raw.max_used_mb,
            billed_ms: raw.billed_ms,
            cold: raw.cold,
            event_function: None,
            event_arch: None,
        }),
        Ok(None) => {}
        Err(()) => {
            return Err(Error::new(format!(
                "{name}: {label}: not a Lambda REPORT line"
            )));
        }
    }
    Ok(())
}

fn apply_header(name: &str, label: &str, line: &str, events: &mut Events) -> Result<(), Error> {
    let rest = line.trim_start_matches('#').trim_ascii();
    let (kind, value) = if let Some(value) = rest.strip_prefix("lambda-bill-function:") {
        ("function", value.trim_ascii())
    } else if let Some(value) = rest.strip_prefix("function:") {
        ("function", value.trim_ascii())
    } else if let Some(value) = rest.strip_prefix("lambda-bill-arch:") {
        ("arch", value.trim_ascii())
    } else if let Some(value) = rest.strip_prefix("architecture:") {
        ("arch", value.trim_ascii())
    } else if let Some(value) = rest.strip_prefix("arch:") {
        ("arch", value.trim_ascii())
    } else {
        return Ok(());
    };
    if kind == "function" {
        if !valid_function_name(value) {
            return Err(Error::new(format!("{name}: {label}: bad function name")));
        }
        if let Some((prev, _)) = &events.header_function {
            if prev != value {
                return Err(Error::new(format!(
                    "{name}: {label}: function name conflict"
                )));
            }
        } else {
            events.header_function = Some((value.to_string(), label.to_string()));
        }
    } else {
        let Some(arch) = Arch::parse(value) else {
            return Err(Error::new(format!("{name}: {label}: bad architecture")));
        };
        if let Some((prev, _)) = &events.header_arch {
            if *prev != arch {
                return Err(Error::new(format!(
                    "{name}: {label}: architecture conflict"
                )));
            }
        } else {
            events.header_arch = Some((arch, label.to_string()));
        }
    }
    Ok(())
}

struct ParsedLine {
    request_id: String,
    memory_mb: u32,
    max_used_mb: u32,
    billed_ms: u64,
    cold: bool,
}

fn parse_report_line(line: &str) -> Result<Option<ParsedLine>, ()> {
    let line = line.trim_matches(|c: char| c.is_ascii_whitespace());
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let Some(start) = find_report(line)? else {
        return Ok(None);
    };
    if find_report(&line[start + "REPORT RequestId:".len()..])?.is_some() {
        return Err(());
    }
    let mut rest = line[start + "REPORT RequestId:".len()..].trim_ascii_start();
    let (request_id, after_id) = take_token(rest)?;
    if !valid_request_id(request_id) {
        return Err(());
    }
    rest = after_id;
    let (duration, after) = take_labeled(rest, "Duration:", "ms", true)?;
    parse_decimal_token(&duration)?;
    let (billed, after) = take_labeled(after, "Billed Duration:", "ms", false)?;
    let (memory, after) = take_labeled(after, "Memory Size:", "MB", false)?;
    let (max_used, after) = take_labeled(after, "Max Memory Used:", "MB", false)?;
    let after = after.trim_ascii_start();
    let cold = if after.starts_with("Init Duration:") {
        let (_init, _) = take_labeled(after, "Init Duration:", "ms", true)?;
        true
    } else {
        false
    };
    let memory_mb = parse_u32(&memory)?;
    if !(128..=10_240).contains(&memory_mb) {
        return Err(());
    }
    let max_used_mb = parse_u32(&max_used)?;
    if max_used_mb > 10_240 {
        return Err(());
    }
    let billed_ms = parse_u64(&billed)?;
    Ok(Some(ParsedLine {
        request_id: request_id.to_string(),
        memory_mb,
        max_used_mb,
        billed_ms,
        cold,
    }))
}

/// Position of a `REPORT RequestId:` marker that follows ASCII white space or
/// starts the line. A marker glued to an invisible or non-ASCII space
/// character (BOM, zero-width space, NBSP, ...) is an error: it would otherwise
/// drop a request without a word. `XREPORT` is simply not a marker.
fn find_report(line: &str) -> Result<Option<usize>, ()> {
    let marker = "REPORT RequestId:";
    let mut from = 0;
    while let Some(offset) = line[from..].find(marker) {
        let at = from + offset;
        if at == 0 || line.as_bytes()[at - 1].is_ascii_whitespace() {
            return Ok(Some(at));
        }
        if let Some(prev) = line[..at].chars().next_back() {
            if is_unsafe_display_char(prev) || prev.is_whitespace() {
                return Err(());
            }
        }
        from = at + 1;
    }
    Ok(None)
}

fn valid_request_id(id: &str) -> bool {
    (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn take_token(text: &str) -> Result<(&str, &str), ()> {
    let text = text.trim_ascii_start();
    if text.is_empty() {
        return Err(());
    }
    let end = text
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(text.len());
    if end == 0 {
        return Err(());
    }
    Ok((&text[..end], &text[end..]))
}

fn take_labeled<'a>(
    text: &'a str,
    label: &str,
    unit: &str,
    fraction: bool,
) -> Result<(String, &'a str), ()> {
    let text = text.trim_ascii_start();
    let rest = text.strip_prefix(label).ok_or(())?.trim_ascii_start();
    let bytes = rest.as_bytes();
    let mut i = 0;
    let mut dot = false;
    while i < bytes.len() {
        match bytes[i] {
            b'0'..=b'9' => i += 1,
            b'.' if fraction && !dot && i > 0 => {
                dot = true;
                i += 1;
            }
            _ => break,
        }
    }
    if i == 0 || bytes.get(i - 1) == Some(&b'.') {
        return Err(());
    }
    let number = rest[..i].to_string();
    let after = rest[i..].trim_ascii_start();
    let after = after.strip_prefix(unit).ok_or(())?;
    if after
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return Err(());
    }
    Ok((number, after))
}

fn parse_decimal_token(text: &str) -> Result<(), ()> {
    if text.is_empty() || text.ends_with('.') {
        return Err(());
    }
    let mut dot = false;
    for (i, byte) in text.bytes().enumerate() {
        match byte {
            b'0'..=b'9' => {}
            b'.' if !dot && i > 0 => dot = true,
            _ => return Err(()),
        }
    }
    Ok(())
}

fn parse_u32(text: &str) -> Result<u32, ()> {
    if text.is_empty() || text.len() > 10 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

fn parse_u64(text: &str) -> Result<u64, ()> {
    if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(());
    }
    text.parse().map_err(|_| ())
}

fn resolve(
    input: &ParseInput<'_>,
    events: &Events,
    path_function: Option<String>,
) -> Result<Vec<Invocation>, Error> {
    let name = show_path(Path::new(input.name));
    let header_function = events
        .header_function
        .as_ref()
        .map(|(name, _)| name.clone());
    let header_arch = events.header_arch.as_ref().map(|(arch, _)| *arch);
    if let (Some(header), Some(path_name)) = (&header_function, &path_function) {
        if header != path_name {
            return Err(Error::new(format!("{name}: function name conflict")));
        }
    }
    let mut out = Vec::with_capacity(events.reports.len());
    for report in &events.reports {
        if let Some(event_name) = &report.event_function {
            for other in [header_function.as_ref(), path_function.as_ref()]
                .into_iter()
                .flatten()
            {
                if other != event_name {
                    return Err(Error::new(format!(
                        "{name}: {}: function name conflict",
                        report.label
                    )));
                }
            }
        }
        let function = report
            .event_function
            .clone()
            .or_else(|| header_function.clone())
            .or_else(|| path_function.clone())
            .or_else(|| input.default_function.map(str::to_string));
        let Some(function) = function else {
            return Err(Error::new(format!(
                "{name}: {}: function name missing",
                report.label
            )));
        };
        if !valid_function_name(&function) {
            return Err(Error::new(format!(
                "{name}: {}: bad function name",
                report.label
            )));
        }
        let arch = resolve_arch(input, &name, report, header_arch, &function)?;
        out.push(Invocation {
            function,
            arch,
            memory_mb: report.memory_mb,
            max_used_mb: report.max_used_mb,
            billed_ms: report.billed_ms,
            cold: report.cold,
            request_id: report.request_id.clone(),
        });
    }
    Ok(out)
}

fn resolve_arch(
    input: &ParseInput<'_>,
    name: &str,
    report: &RawReport,
    header_arch: Option<Arch>,
    function: &str,
) -> Result<Arch, Error> {
    if let (Some(event_arch), Some(header)) = (report.event_arch, header_arch) {
        if event_arch != header {
            return Err(Error::new(format!(
                "{name}: {}: architecture conflict",
                report.label
            )));
        }
    }
    let explicit = report.event_arch.or(header_arch);
    if let Some(arch) = explicit {
        if let Some(mapped) = input.functions.get(function) {
            if *mapped != arch {
                return Err(Error::new(format!(
                    "{name}: {}: architecture conflict",
                    report.label
                )));
            }
        }
        return Ok(arch);
    }
    if let Some(mapped) = input.functions.get(function) {
        return Ok(*mapped);
    }
    Ok(input.default_arch)
}

#[cfg(test)]
mod tests {
    use super::{parse_input, valid_function_name, ParseInput};
    use crate::arch::Arch;
    use std::collections::BTreeMap;

    fn run<'a>(
        name: &'a str,
        text: &'a str,
        function: Option<&'a str>,
        arch: Arch,
    ) -> Result<Vec<super::Invocation>, crate::Error> {
        let map = BTreeMap::new();
        let input = ParseInput {
            name,
            bytes: text.as_bytes(),
            strict: true,
            default_arch: arch,
            default_function: function,
            functions: &map,
        };
        Ok(parse_input(&input)?.invocations)
    }

    fn report(memory: u32, billed: u64, max_used: u32) -> String {
        format!(
            "REPORT RequestId: abcdef Duration: 1.5 ms\tBilled Duration: {billed} ms\tMemory Size: {memory} MB\tMax Memory Used: {max_used} MB\t"
        )
    }

    #[test]
    fn tabs_timestamp_and_xray() {
        let line = "2024-01-15T12:00:00.000Z\tREPORT RequestId: abc-1 Duration: 12.34 ms Billed Duration: 13 ms Memory Size: 128 MB Max Memory Used: 58 MB XRAY TraceId: 1-abc SegmentId: def Sampled: true";
        let rows = run("f.log", line, Some("checkout"), Arch::X86_64).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].cold);
        assert_eq!(rows[0].billed_ms, 13);
        assert_eq!(rows[0].memory_mb, 128);
        assert_eq!(rows[0].max_used_mb, 58);
        assert_eq!(rows[0].function, "checkout");
    }

    #[test]
    fn init_duration_sets_cold_and_does_not_change_billed() {
        let with_init = format!("{}Init Duration: 200.5 ms\t", report(512, 100, 40));
        let rows = run("f.log", &with_init, Some("checkout"), Arch::X86_64).unwrap();
        assert!(rows[0].cold);
        assert_eq!(rows[0].billed_ms, 100);
        let plain = run(
            "f.log",
            &report(512, 100, 40),
            Some("checkout"),
            Arch::X86_64,
        )
        .unwrap();
        assert_eq!(plain[0].billed_ms, rows[0].billed_ms);
        assert!(!plain[0].cold);
    }

    #[test]
    fn rejects_bad_units_range_and_order() {
        let low = report(127, 1, 1);
        assert!(run("f.log", &low, Some("checkout"), Arch::X86_64).is_err());
        let high = report(10241, 1, 1);
        assert!(run("f.log", &high, Some("checkout"), Arch::X86_64).is_err());
        let edge = report(10240, 0, 10240);
        assert!(run("f.log", &edge, Some("checkout"), Arch::X86_64).is_ok());
        let min = report(128, 1, 0);
        assert!(run("f.log", &min, Some("checkout"), Arch::X86_64).is_ok());
        let mb = report(128, 1, 1).replace("MB", "mb");
        let err = run("f.log", &mb, Some("checkout"), Arch::X86_64).unwrap_err();
        assert!(err.message().contains("not a Lambda REPORT line"));
        assert!(!err.message().contains("Duration"));
        let fraction = "REPORT RequestId: abc Billed Duration: 1.5 ms Memory Size: 128 MB Max Memory Used: 1 MB Duration: 1 ms";
        assert!(run("f.log", fraction, Some("checkout"), Arch::X86_64).is_err());
    }

    #[test]
    fn start_end_and_application_json_are_skipped() {
        let text = "\
START RequestId: abc Version: $LATEST
{\"level\":\"info\",\"msg\":\"hello\"}
END RequestId: abc
REPORT RequestId: abcdef Duration: 1 ms Billed Duration: 2 ms Memory Size: 128 MB Max Memory Used: 9 MB
";
        let rows = run("f.log", text, Some("checkout"), Arch::Arm64).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].arch, Arch::Arm64);
        assert_eq!(rows[0].billed_ms, 2);
    }

    #[test]
    fn header_path_and_cli_fallback() {
        let text = "# lambda-bill-function: checkout\n# arch: arm64\n# a comment\n";
        let text = format!("{text}{}", report(128, 1, 1));
        let rows = run(
            "export/aws/lambda/other/a.log",
            &text,
            Some("cli"),
            Arch::X86_64,
        );
        assert!(rows
            .unwrap_err()
            .message()
            .contains("function name conflict"));

        let text = format!(
            "# function: checkout\n# architecture: arm64\n{}",
            report(128, 1, 1)
        );
        let rows = run("dir/a.log", &text, Some("cli"), Arch::X86_64).unwrap();
        assert_eq!(rows[0].function, "checkout");
        assert_eq!(rows[0].arch, Arch::Arm64);

        let rows = run(
            "export/task/aws/lambda/billing/part.log",
            &report(256, 5, 30),
            Some("cli"),
            Arch::X86_64,
        )
        .unwrap();
        assert_eq!(rows[0].function, "billing");

        let err = run("plain.log", &report(128, 1, 1), None, Arch::X86_64).unwrap_err();
        assert!(err.message().contains("function name missing"));
    }

    #[test]
    fn jsonl_log_group_and_duplicate_marker() {
        let line = "{\"logGroup\":\"/aws/lambda/checkout\",\"message\":\"REPORT RequestId: abc Duration: 1 ms Billed Duration: 2 ms Memory Size: 128 MB Max Memory Used: 3 MB\\nSTART RequestId: abc\"}";
        let rows = run("f.log", line, None, Arch::X86_64).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].function, "checkout");
        let bad_group = "{\"logGroup\":\"/var/SECRETGROUP\",\"message\":\"REPORT RequestId: abc Duration: 1 ms Billed Duration: 2 ms Memory Size: 128 MB Max Memory Used: 3 MB\"}";
        let err = run("f.log", bad_group, Some("checkout"), Arch::X86_64).unwrap_err();
        assert!(err
            .message()
            .contains("log group is not /aws/lambda/<name>"));
        assert!(!err.message().contains("SECRETGROUP"));
        let two = "REPORT RequestId: abc Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB REPORT RequestId: def Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB";
        assert!(run("f.log", two, Some("checkout"), Arch::X86_64).is_err());
    }

    #[test]
    fn jsonl_log_group_ascii_space_counts_unicode_space_does_not() {
        let msg = "REPORT RequestId: abc Duration: 1 ms Billed Duration: 2 ms Memory Size: 128 MB Max Memory Used: 3 MB";
        for group in [
            " /aws/lambda/checkout ",
            "\\t/aws/lambda/checkout\\r\\n",
            "/aws/lambda/checkout/ ",
        ] {
            let line = format!("{{\"logGroup\":\"{group}\",\"message\":\"{msg}\"}}");
            let rows = run("f.log", &line, None, Arch::X86_64).unwrap();
            assert_eq!(rows[0].function, "checkout", "{group:?}");
        }
        for edge in ["\\u00a0", "\\u3000", "\\u2028"] {
            for group in [
                format!("{edge}/aws/lambda/checkout"),
                format!("/aws/lambda/checkout{edge}"),
                format!("/aws/lambda/check{edge}out"),
            ] {
                for key in ["logGroup", "logGroupName"] {
                    let line = format!("{{\"{key}\":\"{group}\",\"message\":\"{msg}\"}}");
                    let err = run("f.log", &line, None, Arch::X86_64).unwrap_err();
                    assert!(err
                        .message()
                        .contains("log group is not /aws/lambda/<name>"));
                    assert!(!err.message().contains("checkout"));
                }
            }
        }
    }

    #[test]
    fn filter_log_events_document_skips_start_lines() {
        let text = r#"{
          "events": [
            {"message": "START RequestId: abc Version: $LATEST\n", "logGroup": "/aws/lambda/checkout"},
            {"message": "REPORT RequestId: abc Duration: 9 ms Billed Duration: 9 ms Memory Size: 256 MB Max Memory Used: 20 MB\n", "logGroupName": "/aws/lambda/checkout"}
          ]
        }"#;
        let rows = run("api.json", text, None, Arch::X86_64).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].memory_mb, 256);
    }

    #[test]
    fn claim_c21_gzip_is_recognized_by_magic_bytes_not_the_name() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let body = format!("# function: checkout\n{}", report(128, 4, 8));
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body.as_bytes()).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let map = BTreeMap::new();
        let rows = parse_input(&ParseInput {
            name: "export.txt",
            bytes: &bytes,
            strict: true,
            default_arch: Arch::X86_64,
            default_function: None,
            functions: &map,
        })
        .unwrap()
        .invocations;
        assert_eq!(rows[0].billed_ms, 4);
        let plain = parse_input(&ParseInput {
            name: "notes.gz",
            bytes: body.as_bytes(),
            strict: true,
            default_arch: Arch::X86_64,
            default_function: None,
            functions: &map,
        })
        .unwrap()
        .invocations;
        assert_eq!(plain[0].billed_ms, 4);
        let err = parse_input(&ParseInput {
            name: "notes.gz",
            bytes: b"\x1f\x8bnot-a-gzip",
            strict: true,
            default_arch: Arch::X86_64,
            default_function: Some("checkout"),
            functions: &map,
        })
        .unwrap_err();
        assert!(err.message().contains("could not read gzip"));
    }

    #[test]
    fn gzip_magic_not_the_file_name() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let body = format!("# function: checkout\n{}", report(128, 4, 8));
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body.as_bytes()).unwrap();
        let bytes = encoder.finish().unwrap();
        assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
        let map = BTreeMap::new();
        let input = ParseInput {
            name: "export.txt",
            bytes: &bytes,
            strict: true,
            default_arch: Arch::X86_64,
            default_function: None,
            functions: &map,
        };
        let rows = parse_input(&input).unwrap().invocations;
        assert_eq!(rows[0].billed_ms, 4);
        let mut named = body.into_bytes();
        named.splice(0..0, b"not-gzip".to_vec());
        // A file whose name is irrelevant: plain text still parses.
        let _ = named;
        let err = parse_input(&ParseInput {
            name: "notes.gz",
            bytes: b"\x1f\x8bnot-a-gzip",
            strict: true,
            default_arch: Arch::X86_64,
            default_function: Some("checkout"),
            functions: &map,
        })
        .unwrap_err();
        assert!(err.message().contains("could not read gzip"));
    }

    #[test]
    fn gzip_members_are_all_read_and_trailing_bytes_fail() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;
        let gz = |text: &str| {
            let mut e = GzEncoder::new(Vec::new(), Compression::fast());
            e.write_all(text.as_bytes()).unwrap();
            e.finish().unwrap()
        };
        let first = gz(&format!("# function: checkout\n{}\n", report(128, 4, 8)));
        let second = gz(&report(128, 6, 8));
        let map = BTreeMap::new();
        let parse = |bytes: &[u8]| {
            parse_input(&ParseInput {
                name: "export.gz",
                bytes,
                strict: true,
                default_arch: Arch::X86_64,
                default_function: None,
                functions: &map,
            })
        };
        let mut two = first.clone();
        two.extend_from_slice(&second);
        let rows = parse(&two).unwrap().invocations;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().map(|r| r.billed_ms).sum::<u64>(), 10);
        assert_eq!(parse(&first).unwrap().invocations.len(), 1);
        let mut junk = first.clone();
        junk.extend_from_slice(b"trailing junk");
        let err = parse(&junk).unwrap_err();
        assert!(err.message().contains("could not read gzip"));
        let mut bad = first;
        bad.extend_from_slice(b"\x1f\x8bnot-a-member");
        assert!(parse(&bad).is_err());
    }

    #[test]
    fn names_and_empty_file() {
        assert!(valid_function_name("a"));
        assert!(valid_function_name(&"a".repeat(64)));
        assert!(!valid_function_name(&"a".repeat(65)));
        assert!(!valid_function_name("a/b"));
        assert!(!valid_function_name(""));
        assert!(!valid_function_name("has space"));
        let err = run("f.log", "", Some("checkout"), Arch::X86_64).unwrap_err();
        assert!(err.message().contains("no REPORT lines"));
        let err = run("f.log", "# function: SECRET NAME\n", None, Arch::X86_64).unwrap_err();
        assert!(err.message().contains("bad function name") || err.message().contains("no REPORT"));
        // The bad header is reported before the empty-report check.
        let text = "# function: SECRET NAME\nREPORT RequestId: abc Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
        let err = run("f.log", text, None, Arch::X86_64).unwrap_err();
        assert_eq!(err.message(), "f.log: line 1: bad function name");
        assert!(!err.message().contains("SECRET"));
    }

    #[test]
    fn bracket_timestamp_crlf_and_bom() {
        let text = "\u{feff}[2024-01-15T00:00:00.000Z] REPORT RequestId: abc Duration: 1 ms Billed Duration: 8 ms Memory Size: 128 MB Max Memory Used: 1 MB\r\n";
        let rows = run("f.log", text, Some("checkout"), Arch::X86_64).unwrap();
        assert_eq!(rows[0].billed_ms, 8);
    }

    #[test]
    fn non_ascii_white_space_around_a_line_is_an_error() {
        let rep = "REPORT RequestId: abc Duration: 1 ms Billed Duration: 8 ms Memory Size: 128 MB Max Memory Used: 1 MB";
        for ws in ["\u{a0}", "\u{3000}", "\u{2028}"] {
            for text in [
                format!("{ws}{rep}\n"),
                format!("{rep}{ws}\n"),
                format!("{ws}{{\"message\":\"{rep}\"}}\n"),
            ] {
                let err = run("f.log", &text, Some("checkout"), Arch::X86_64).unwrap_err();
                assert!(err.message().contains("non-ASCII white space"));
            }
        }
        let ok = format!(" \t{rep}\r\n");
        assert!(run("f.log", &ok, Some("checkout"), Arch::X86_64).is_ok());
    }

    #[test]
    fn nbsp_inside_report_and_header_is_not_trimmed() {
        let tail = "Duration: 1 ms Billed Duration: 8 ms Memory Size: 128 MB Max Memory Used: 1 MB";
        for bad in [
            format!("REPORT RequestId:\u{a0}abc {tail}\n"),
            format!("REPORT RequestId: abc\u{a0}{tail}\n"),
            "REPORT RequestId: abc Duration:\u{a0}1 ms Billed Duration: 8 ms Memory Size: 128 MB Max Memory Used: 1 MB\n".to_string(),
        ] {
            assert!(super::parse_report_line(bad.trim_end()).is_err(), "{bad:?}");
        }
        assert!(super::parse_report_line(&format!("REPORT RequestId: abc {tail}")).is_ok());
        for h in [
            "# function:\u{a0}checkout\n",
            "# function: checkout\u{a0}\n",
        ] {
            let text = format!("{h}REPORT RequestId: abc {tail}\n");
            assert!(run("f.log", &text, None, Arch::X86_64).is_err(), "{h:?}");
        }
    }

    #[test]
    fn non_strict_skips_a_file_with_no_reports() {
        let map = BTreeMap::new();
        let input = ParseInput {
            name: "readme.txt",
            bytes: b"hello\n",
            strict: false,
            default_arch: Arch::X86_64,
            default_function: None,
            functions: &map,
        };
        let out = parse_input(&input).unwrap();
        assert!(out.invocations.is_empty());
        assert!(out.skipped.unwrap().contains("no REPORT lines"));
    }

    #[test]
    fn duplicate_scan_fault_is_refused() {
        let err = super::reject_duplicate_keys("sample.log", "{\"message\":\"SECRET\\uD800\"}")
            .unwrap_err();
        assert_eq!(err.message(), "sample.log: not JSON");
        assert!(!err.message().contains("SECRET"));
    }
}
