use std::collections::{BTreeMap, HashSet};

use rust_decimal::Decimal;
use serde_json::{Map, Value};

use crate::arch::Arch;
use crate::error::Error;
use crate::money::{days_from_date, money_error, parse_money, today_utc, valid_date};

/// Rates for one quote. The built-in quote is the first on-demand tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prices {
    pub currency: String,
    pub source: String,
    pub date: String,
    pub region: String,
    pub tier: String,
    pub request_per_million: Decimal,
    pub x86_64_per_gb_second: Decimal,
    pub arm64_per_gb_second: Decimal,
}

impl Prices {
    #[must_use]
    pub fn rate(&self, arch: Arch) -> Decimal {
        match arch {
            Arch::Arm64 => self.arm64_per_gb_second,
            Arch::X86_64 => self.x86_64_per_gb_second,
        }
    }
}

/// A price list older than this many days is reported with a warning.
pub const PRICE_MAX_AGE_DAYS: i64 = 180;

impl Prices {
    /// Warning text when the price date is more than [`PRICE_MAX_AGE_DAYS`]
    /// days before `as_of` (`YYYY-MM-DD`; today in UTC when `None`). A price
    /// date that is not before `as_of` never warns.
    ///
    /// # Errors
    /// `as_of` is not a valid `YYYY-MM-DD` date.
    pub fn age_warning(&self, as_of: Option<&str>) -> Result<Option<String>, Error> {
        let today;
        let as_of = match as_of {
            Some(text) => text,
            None => {
                today = today_utc();
                &today
            }
        };
        let (Some(now), Some(then)) = (days_from_date(as_of), days_from_date(&self.date)) else {
            return Err(Error::new(format!(
                "invalid --as-of {}: expected a date as YYYY-MM-DD",
                crate::error::escape_unsafe(as_of)
            )));
        };
        let age = now - then;
        if age > PRICE_MAX_AGE_DAYS {
            Ok(Some(format!(
                "price data is dated {}, {age} days before {as_of} (more than {PRICE_MAX_AGE_DAYS} days old): check the current list prices",
                self.date
            )))
        } else {
            Ok(None)
        }
    }
}

const BUILTIN_JSON: &str = include_str!("../prices/lambda-ondemand.json");

/// First-tier on-demand rates from the AWS Lambda pricing page, read 2026-11-09.
///
/// The page's worked examples use US East (N. Virginia): 0.20 USD per million
/// requests, 0.0000166667 USD per GB-second on x86, and 0.0000133334 USD per
/// GB-second on arm64 (the ARM durable-function example on the same page).
#[must_use]
pub fn prices_builtin() -> Prices {
    load_prices_str(BUILTIN_JSON).expect("built-in prices")
}

pub fn load_prices_str(text: &str) -> Result<Prices, Error> {
    let text = strip_bom(text);
    // Counted first: the key scan holds every key, so it must not see a text
    // that is over the budget.
    if !crate::budget::within_budget(text) {
        return Err(money_error(format!(
            "more than {} JSON values",
            crate::budget::MAX_JSON_UNITS
        )));
    }
    match top_level_keys(text) {
        Ok(_) => {}
        Err(KeyFault::Duplicate) => return Err(money_error("duplicate key")),
        Err(KeyFault::Bad) => return Err(money_error("not JSON")),
    }
    let value: Value = serde_json::from_str(text).map_err(|_| money_error("not JSON"))?;
    let Some(obj) = value.as_object() else {
        return Err(money_error("expected an object"));
    };
    Ok(Prices {
        currency: req_currency(obj)?,
        source: req_source(obj)?,
        date: req_date(obj)?,
        region: req_label(obj, "region")?,
        tier: req_label(obj, "tier")?,
        request_per_million: req_decimal(obj, "request_per_million")?,
        x86_64_per_gb_second: req_decimal(obj, "x86_64_per_gb_second")?,
        arm64_per_gb_second: req_decimal(obj, "arm64_per_gb_second")?,
    })
}

pub fn load_functions_str(text: &str) -> Result<BTreeMap<String, Arch>, Error> {
    let text = strip_bom(text);
    // Counted first: the key scan holds every key, so it must not see a text
    // that is over the budget.
    if !crate::budget::within_budget(text) {
        return Err(money_error(format!(
            "more than {} JSON values",
            crate::budget::MAX_JSON_UNITS
        )));
    }
    match top_level_keys(text) {
        Ok(_) => {}
        Err(KeyFault::Duplicate) => return Err(money_error("duplicate key")),
        Err(KeyFault::Bad) => return Err(money_error("not JSON")),
    }
    let value: Value = serde_json::from_str(text).map_err(|_| money_error("not JSON"))?;
    let Some(obj) = value.as_object() else {
        return Err(money_error("expected an object"));
    };
    let mut map = BTreeMap::new();
    for (name, arch_value) in obj {
        if !crate::parse::valid_function_name(name) {
            return Err(money_error("bad function name"));
        }
        let Some(text) = arch_value.as_str() else {
            return Err(money_error("bad architecture"));
        };
        let Some(arch) = Arch::parse(text) else {
            return Err(money_error("bad architecture"));
        };
        map.insert(name.clone(), arch);
    }
    Ok(map)
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn req_currency(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "currency")?;
    if text.len() == 3 && text.bytes().all(|b| b.is_ascii_uppercase()) {
        Ok(text)
    } else {
        Err(money_error("bad currency"))
    }
}

fn req_source(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "source")?;
    if (1..=500).contains(&text.chars().count())
        && !text.chars().any(crate::error::is_unsafe_display_char)
    {
        Ok(text)
    } else {
        Err(money_error("bad source"))
    }
}

fn req_date(obj: &Map<String, Value>) -> Result<String, Error> {
    let text = req_string(obj, "date")?;
    if valid_date(&text) {
        Ok(text)
    } else {
        Err(money_error("bad date"))
    }
}

fn req_label(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let text = req_string(obj, key)?;
    let ok = (1..=64).contains(&text.len())
        && text.chars().all(|c| c.is_ascii_graphic() || c == ' ')
        && !text.chars().any(crate::error::is_unsafe_display_char);
    if ok {
        Ok(text)
    } else {
        Err(money_error(format!("bad {key}")))
    }
}

fn req_decimal(obj: &Map<String, Value>, key: &str) -> Result<Decimal, Error> {
    let Some(value) = obj.get(key) else {
        return Err(money_error(format!("missing {key}")));
    };
    let raw = match value {
        Value::String(text) => text
            .trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
            .to_string(),
        Value::Number(number) => number.to_string(),
        _ => return Err(money_error(format!("bad {key}"))),
    };
    parse_money(&raw).map_err(|()| money_error(format!("bad {key}")))
}

fn req_string(obj: &Map<String, Value>, key: &str) -> Result<String, Error> {
    let Some(value) = obj.get(key) else {
        return Err(money_error(format!("missing {key}")));
    };
    value
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| money_error(format!("bad {key}")))
}

enum KeyFault {
    Duplicate,
    Bad,
}

/// Reject a repeated key on the top-level object. Nested keys are not checked.
fn top_level_keys(text: &str) -> Result<(), KeyFault> {
    let bytes = text.as_bytes();
    let mut i = 0;
    skip_ws(bytes, &mut i);
    if i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    // Arrays and other values are schema errors, not duplicate-key errors.
    if bytes[i] != b'{' {
        return Ok(());
    }
    i += 1;
    let mut keys = HashSet::new();
    loop {
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b'}' {
            return Ok(());
        }
        let key = parse_json_string(bytes, &mut i)?;
        if !keys.insert(key) {
            return Err(KeyFault::Duplicate);
        }
        skip_ws(bytes, &mut i);
        if i >= bytes.len() || bytes[i] != b':' {
            return Err(KeyFault::Bad);
        }
        i += 1;
        skip_value(bytes, &mut i, 1)?;
        skip_ws(bytes, &mut i);
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
            continue;
        }
        if i < bytes.len() && bytes[i] == b'}' {
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

/// Whether log-event JSON repeats a key on an event object.
///
/// Checks the root object, each object in a root array, and each object in a
/// root `events` array. A repeated key inside a nested value is allowed, the
/// same rule as a price file. `Err` means this scan does not understand the
/// text. Callers must refuse that. It is not "no duplicate".
pub(crate) fn log_json_has_duplicate_key(text: &str) -> Result<bool, ()> {
    let bytes = text.as_bytes();
    let mut i = 0;
    skip_ws(bytes, &mut i);
    if i >= bytes.len() {
        return Err(());
    }
    let duplicated = match bytes[i] {
        b'{' => object_has_duplicate_key(bytes, &mut i, true),
        b'[' => array_objects_have_duplicate_key(bytes, &mut i),
        _ => return Ok(false),
    }
    .map_err(|_| ())?;
    if duplicated {
        return Ok(true);
    }
    skip_ws(bytes, &mut i);
    if i == bytes.len() {
        Ok(false)
    } else {
        Err(())
    }
}

/// Keys of the object at `i`. When `inspect_events` is set, objects inside a
/// root `events` array are checked the same way.
fn object_has_duplicate_key(
    bytes: &[u8],
    i: &mut usize,
    inspect_events: bool,
) -> Result<bool, KeyFault> {
    if *i >= bytes.len() || bytes[*i] != b'{' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut keys = HashSet::new();
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(false);
        }
        let key = parse_json_string(bytes, i)?;
        let events_array = inspect_events && key == "events";
        if !keys.insert(key) {
            return Ok(true);
        }
        skip_ws(bytes, i);
        if *i >= bytes.len() || bytes[*i] != b':' {
            return Err(KeyFault::Bad);
        }
        *i += 1;
        if events_array {
            skip_ws(bytes, i);
            if *i < bytes.len() && bytes[*i] == b'[' {
                if array_objects_have_duplicate_key(bytes, i)? {
                    return Ok(true);
                }
            } else {
                skip_value(bytes, i, 1)?;
            }
        } else {
            skip_value(bytes, i, 1)?;
        }
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b'}' {
            *i += 1;
            return Ok(false);
        }
        return Err(KeyFault::Bad);
    }
}

fn array_objects_have_duplicate_key(bytes: &[u8], i: &mut usize) -> Result<bool, KeyFault> {
    skip_ws(bytes, i);
    if *i >= bytes.len() || bytes[*i] != b'[' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(false);
        }
        if *i < bytes.len() && bytes[*i] == b'{' {
            if object_has_duplicate_key(bytes, i, false)? {
                return Ok(true);
            }
        } else {
            skip_value(bytes, i, 1)?;
        }
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == b']' {
            *i += 1;
            return Ok(false);
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

fn skip_value(bytes: &[u8], i: &mut usize, depth: u32) -> Result<(), KeyFault> {
    if depth > 128 {
        return Err(KeyFault::Bad);
    }
    skip_ws(bytes, i);
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    match bytes[*i] {
        b'"' => {
            parse_json_string(bytes, i)?;
            Ok(())
        }
        b'{' => skip_container(bytes, i, b'{', b'}', depth),
        b'[' => skip_container(bytes, i, b'[', b']', depth),
        b't' => skip_lit(bytes, i, b"true"),
        b'f' => skip_lit(bytes, i, b"false"),
        b'n' => skip_lit(bytes, i, b"null"),
        b'-' | b'0'..=b'9' => skip_number(bytes, i),
        _ => Err(KeyFault::Bad),
    }
}

fn skip_container(
    bytes: &[u8],
    i: &mut usize,
    open: u8,
    close: u8,
    depth: u32,
) -> Result<(), KeyFault> {
    if bytes[*i] != open {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    loop {
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        if open == b'{' {
            parse_json_string(bytes, i)?;
            skip_ws(bytes, i);
            if *i >= bytes.len() || bytes[*i] != b':' {
                return Err(KeyFault::Bad);
            }
            *i += 1;
        }
        skip_value(bytes, i, depth + 1)?;
        skip_ws(bytes, i);
        if *i < bytes.len() && bytes[*i] == b',' {
            *i += 1;
            continue;
        }
        if *i < bytes.len() && bytes[*i] == close {
            *i += 1;
            return Ok(());
        }
        return Err(KeyFault::Bad);
    }
}

fn skip_lit(bytes: &[u8], i: &mut usize, lit: &[u8]) -> Result<(), KeyFault> {
    if bytes[*i..].starts_with(lit) {
        *i += lit.len();
        Ok(())
    } else {
        Err(KeyFault::Bad)
    }
}

fn skip_number(bytes: &[u8], i: &mut usize) -> Result<(), KeyFault> {
    let start = *i;
    if bytes[*i] == b'-' {
        *i += 1;
    }
    if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
        return Err(KeyFault::Bad);
    }
    while *i < bytes.len() && bytes[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b'.' {
        *i += 1;
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        if *i >= bytes.len() || !bytes[*i].is_ascii_digit() {
            return Err(KeyFault::Bad);
        }
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i == start {
        return Err(KeyFault::Bad);
    }
    Ok(())
}

fn parse_json_string(bytes: &[u8], i: &mut usize) -> Result<String, KeyFault> {
    if *i >= bytes.len() || bytes[*i] != b'"' {
        return Err(KeyFault::Bad);
    }
    *i += 1;
    let mut out = String::new();
    while *i < bytes.len() {
        match bytes[*i] {
            b'"' => {
                *i += 1;
                return Ok(out);
            }
            b'\\' => {
                *i += 1;
                out.push(unescape(bytes, i)?);
            }
            0x00..=0x1f => return Err(KeyFault::Bad),
            0x20..=0x7f => {
                out.push(bytes[*i] as char);
                *i += 1;
            }
            _ => out.push(take_one_char(bytes, i)?),
        }
    }
    Err(KeyFault::Bad)
}

fn unescape(bytes: &[u8], i: &mut usize) -> Result<char, KeyFault> {
    if *i >= bytes.len() {
        return Err(KeyFault::Bad);
    }
    let esc = bytes[*i];
    *i += 1;
    match esc {
        b'"' => Ok('"'),
        b'\\' => Ok('\\'),
        b'/' => Ok('/'),
        b'b' => Ok('\u{0008}'),
        b'f' => Ok('\u{000c}'),
        b'n' => Ok('\n'),
        b'r' => Ok('\r'),
        b't' => Ok('\t'),
        // U+10000 and above is a UTF-16 surrogate pair: \uD800-\uDBFF then \uDC00-\uDFFF.
        b'u' => decode_u_escape(bytes, i),
        _ => Err(KeyFault::Bad),
    }
}

fn decode_u_escape(bytes: &[u8], i: &mut usize) -> Result<char, KeyFault> {
    let unit = read_hex4(bytes, i)?;
    if (0xDC00..=0xDFFF).contains(&unit) {
        return Err(KeyFault::Bad);
    }
    if !(0xD800..=0xDBFF).contains(&unit) {
        return char::from_u32(unit).ok_or(KeyFault::Bad);
    }
    if *i + 2 > bytes.len() || bytes[*i] != b'\\' || bytes[*i + 1] != b'u' {
        return Err(KeyFault::Bad);
    }
    *i += 2;
    let low = read_hex4(bytes, i)?;
    if !(0xDC00..=0xDFFF).contains(&low) {
        return Err(KeyFault::Bad);
    }
    let point = 0x1_0000 + (((unit - 0xD800) << 10) | (low - 0xDC00));
    char::from_u32(point).ok_or(KeyFault::Bad)
}

fn read_hex4(bytes: &[u8], i: &mut usize) -> Result<u32, KeyFault> {
    if *i + 4 > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let mut code = 0u32;
    for _ in 0..4 {
        let nibble = match bytes[*i] {
            b'0'..=b'9' => bytes[*i] - b'0',
            b'a'..=b'f' => bytes[*i] - b'a' + 10,
            b'A'..=b'F' => bytes[*i] - b'A' + 10,
            _ => return Err(KeyFault::Bad),
        };
        *i += 1;
        code = (code << 4) | u32::from(nibble);
    }
    Ok(code)
}

/// One UTF-8 character at `i`. Only those bytes are read, not the rest of the text.
fn take_one_char(bytes: &[u8], i: &mut usize) -> Result<char, KeyFault> {
    let start = *i;
    let width = match bytes.get(start).copied() {
        Some(0x00..=0x7F) => 1,
        Some(0xC2..=0xDF) => 2,
        Some(0xE0..=0xEF) => 3,
        Some(0xF0..=0xF4) => 4,
        _ => return Err(KeyFault::Bad),
    };
    let end = start + width;
    if end > bytes.len() {
        return Err(KeyFault::Bad);
    }
    let text = std::str::from_utf8(&bytes[start..end]).map_err(|_| KeyFault::Bad)?;
    let ch = text.chars().next().ok_or(KeyFault::Bad)?;
    if ch.len_utf8() != width {
        return Err(KeyFault::Bad);
    }
    *i = end;
    Ok(ch)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{load_functions_str, load_prices_str, prices_builtin};
    use crate::arch::Arch;

    fn builtin_text() -> &'static str {
        include_str!("../prices/lambda-ondemand.json")
    }

    #[test]
    fn builtin_matches_the_pricing_page() {
        let prices = prices_builtin();
        assert_eq!(prices.currency, "USD");
        assert_eq!(prices.source, "https://aws.amazon.com/lambda/pricing/");
        assert_eq!(prices.date, "2026-11-09");
        assert_eq!(prices.region, "us-east-1");
        assert_eq!(prices.tier, "first");
        assert_eq!(
            prices.request_per_million,
            "0.20".parse::<Decimal>().unwrap()
        );
        assert_eq!(
            prices.x86_64_per_gb_second,
            "0.0000166667".parse::<Decimal>().unwrap()
        );
        assert_eq!(
            prices.arm64_per_gb_second,
            "0.0000133334".parse::<Decimal>().unwrap()
        );
        assert_eq!(prices.rate(Arch::X86_64), prices.x86_64_per_gb_second);
        assert_eq!(prices.rate(Arch::Arm64), prices.arm64_per_gb_second);
        assert!(prices.arm64_per_gb_second < prices.x86_64_per_gb_second);
    }

    #[test]
    fn file_round_trip_and_unknown_field() {
        let mut text = builtin_text().to_string();
        assert_eq!(load_prices_str(&text).unwrap(), prices_builtin());
        text = text.trim_end().trim_end_matches('}').to_string();
        text.push_str(", \"note\": \"ignored\"\n}\n");
        assert_eq!(load_prices_str(&text).unwrap().tier, "first");
    }

    #[test]
    fn json_number_is_accepted_and_exponent_is_not() {
        let text = r#"{"currency":"USD","source":"https://example.test","date":"2026-11-09","region":"us-east-1","tier":"first","request_per_million":0.2,"x86_64_per_gb_second":"0.0000166667","arm64_per_gb_second":"0.0000133334"}"#;
        let prices = load_prices_str(text).unwrap();
        assert_eq!(
            prices.request_per_million,
            "0.20".parse::<Decimal>().unwrap()
        );
        let bad = text.replace("0.2", "1e-7");
        let err = load_prices_str(&bad).unwrap_err();
        assert_eq!(err.message(), "bad request_per_million");
        assert!(!err.message().contains("1e-7"));
    }

    #[test]
    fn price_string_trims_only_ascii_whitespace() {
        let base = builtin_text();
        let ok = base.replace("\"0.20\"", "\" \\t0.20\\r\\n\"");
        assert_eq!(
            load_prices_str(&ok).unwrap().request_per_million,
            "0.20".parse::<Decimal>().unwrap()
        );
        for pad in ["\\u00a0", "\\u2028", "\\u3000"] {
            for bad in [format!("{pad}0.20"), format!("0.20{pad}")] {
                let text = base.replace("\"0.20\"", &format!("\"{bad}\""));
                assert_eq!(
                    load_prices_str(&text).unwrap_err().message(),
                    "bad request_per_million"
                );
            }
        }
    }

    #[test]
    fn rejects_negative_bad_date_missing_arm_and_lowercase_currency() {
        let base = builtin_text();
        let neg = base.replace("0.20", "-0.20");
        assert_eq!(
            load_prices_str(&neg).unwrap_err().message(),
            "bad request_per_million"
        );
        let date = base.replace("2026-11-09", "2025-02-29");
        assert_eq!(load_prices_str(&date).unwrap_err().message(), "bad date");
        let missing = base.replace(
            "\"arm64_per_gb_second\": \"0.0000133334\"",
            "\"nope\": \"1\"",
        );
        assert_eq!(
            load_prices_str(&missing).unwrap_err().message(),
            "missing arm64_per_gb_second"
        );
        let currency = base.replace("\"USD\"", "\"usd\"");
        assert_eq!(
            load_prices_str(&currency).unwrap_err().message(),
            "bad currency"
        );
        assert_eq!(
            load_prices_str("[]").unwrap_err().message(),
            "expected an object"
        );
        assert_eq!(load_prices_str("{").unwrap_err().message(), "not JSON");
    }

    #[test]
    fn duplicate_key_and_secret_are_not_echoed() {
        let text = "{\"currency\":\"USD\",\"currency\":\"SECRETTOKEN\"}";
        let err = load_prices_str(text).unwrap_err();
        assert_eq!(err.message(), "duplicate key");
        assert!(!err.message().contains("SECRETTOKEN"));
        let nested = builtin_text().replace(
            "\"tier\": \"first\"",
            "\"tier\": \"first\", \"extra\": {\"a\": 1, \"a\": 2}",
        );
        assert!(load_prices_str(&nested).is_ok());
    }

    #[test]
    fn bom_and_control_character_in_source() {
        let text = format!("\u{feff}{}", builtin_text());
        assert_eq!(load_prices_str(&text).unwrap().date, "2026-11-09");
        let bad = builtin_text().replace(
            "https://aws.amazon.com/lambda/pricing/",
            "https://aws.amazon.com/lambda/pricing/\\nSECRET",
        );
        let err = load_prices_str(&bad).unwrap_err();
        assert_eq!(err.message(), "bad source");
        assert!(!err.message().contains("SECRET"));
    }

    #[test]
    fn bidi_and_separator_characters_in_source_are_rejected() {
        for esc in [
            "\\u202e", "\\u2028", "\\u2029", "\\u202d", "\\u2066", "\\u200f",
        ] {
            let bad = builtin_text().replace(
                "https://aws.amazon.com/lambda/pricing/",
                &format!("https://aws.amazon.com/lambda/pricing/{esc}x"),
            );
            let err = load_prices_str(&bad).unwrap_err();
            assert_eq!(err.message(), "bad source", "{esc}");
        }
    }

    #[test]
    fn zero_rate_is_allowed() {
        let text = builtin_text().replace("0.20", "0");
        assert_eq!(
            load_prices_str(&text).unwrap().request_per_million,
            Decimal::ZERO
        );
    }

    #[test]
    fn the_value_budget_is_checked_before_the_duplicate_key_scan() {
        // Over the budget, with a repeated key at the very end: the scan would
        // have to read and hold every key first, so the budget must answer.
        let keys = usize::try_from(crate::budget::MAX_JSON_UNITS).unwrap();
        let mut text = String::with_capacity(keys * 8);
        text.push('{');
        for n in 0..=keys {
            text.push_str(&format!("\"k{n}\":0,"));
        }
        text.push_str("\"k0\":0}");
        let expected = format!("more than {} JSON values", crate::budget::MAX_JSON_UNITS);
        assert_eq!(load_prices_str(&text).unwrap_err().message(), expected);
        assert_eq!(load_functions_str(&text).unwrap_err().message(), expected);
    }

    #[test]
    fn functions_file() {
        let map = load_functions_str("{\"worker\":\"arm64\",\"auth\":\"x86_64\"}").unwrap();
        assert_eq!(map.get("worker"), Some(&Arch::Arm64));
        assert_eq!(map.get("auth"), Some(&Arch::X86_64));
        assert!(load_functions_str("{}").unwrap().is_empty());
        let err = load_functions_str("{\"TOPSECRET!\":\"arm64\"}").unwrap_err();
        assert_eq!(err.message(), "bad function name");
        assert!(!err.message().contains("TOPSECRET"));
        let err = load_functions_str("{\"worker\":\"x86\"}").unwrap_err();
        assert_eq!(err.message(), "bad architecture");
        let err = load_functions_str("{\"worker\":\"arm64\",\"worker\":\"x86_64\"}").unwrap_err();
        assert_eq!(err.message(), "duplicate key");
    }

    #[test]
    fn tier_label_is_not_special() {
        let text = builtin_text().replace("\"first\"", "\"second\"");
        assert_eq!(load_prices_str(&text).unwrap().tier, "second");
    }

    #[test]
    fn surrogate_pair_is_a_character_not_a_scan_fault() {
        let duplicated = "{\"message\":\"\\uD83D\\uDE00\",\"message\":\"x\"}";
        assert_eq!(super::log_json_has_duplicate_key(duplicated), Ok(true));
        let once = "{\"message\":\"\\uD83D\\uDE00\"}";
        assert_eq!(super::log_json_has_duplicate_key(once), Ok(false));
        assert!(super::log_json_has_duplicate_key("{\"a\":\"\\uD800\"}").is_err());
    }

    #[test]
    fn a_few_hundred_non_ascii_chars_do_not_hide_a_duplicate() {
        let mark = "é".repeat(300);
        assert_eq!(mark.chars().count(), 300);
        let duplicated = format!("{{\"message\":\"R1\",\"note\":\"{mark}\",\"message\":\"R2\"}}");
        assert_eq!(super::log_json_has_duplicate_key(&duplicated), Ok(true));
        let clean = format!("{{\"message\":\"R1\",\"note\":\"{mark}\"}}");
        assert_eq!(super::log_json_has_duplicate_key(&clean), Ok(false));
    }
}
