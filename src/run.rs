use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::Path;

use rust_decimal::Decimal;

use crate::advice::advise;
use crate::arch::Arch;
use crate::error::{show_path, Error};
use crate::money::{exact_decimal, exact_duration, exact_gb, exact_request, Ratio};
use crate::parse::{parse_input, valid_function_name, Invocation, ParseInput};
use crate::price::{prices_builtin, Prices};
use crate::report::{render, render_markdown, Bill, Row, Totals};
use crate::MAX_FILE_BYTES;

/// What to price, and how to print it.
#[derive(Debug, Clone)]
pub struct Options {
    pub prices: Prices,
    pub default_arch: Arch,
    pub default_function: Option<String>,
    pub functions: BTreeMap<String, Arch>,
    pub headroom_percent: u8,
    pub json: bool,
    /// Markdown instead of the text table. Ignored when `json` is set.
    pub markdown: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prices: prices_builtin(),
            default_arch: Arch::X86_64,
            default_function: None,
            functions: BTreeMap::new(),
            headroom_percent: 20,
            json: false,
            markdown: false,
        }
    }
}

/// One file, or one member of a directory walk (`strict` is false for members).
pub struct NamedBytes {
    pub name: String,
    pub bytes: Vec<u8>,
    pub strict: bool,
}

/// Rendered report. `warnings` is also included in `stdout`.
pub struct Output {
    pub stdout: String,
    pub warnings: Vec<String>,
    /// Requests counted (duplicates are not counted).
    pub requests: u64,
    /// Distinct function names in the report.
    pub functions: usize,
}

pub fn duration_usd(memory_mb: u32, billed_ms: u64, rate: Decimal) -> Result<Decimal, Error> {
    exact_decimal(&exact_duration(memory_mb, billed_ms, rate))
}

pub fn request_usd(requests: u64, per_million: Decimal) -> Result<Decimal, Error> {
    exact_decimal(&exact_request(requests, per_million))
}

pub fn bill_from_inputs(inputs: &[NamedBytes], options: &Options) -> Result<Bill, Error> {
    if options.headroom_percent > 100 {
        return Err(Error::new("bad headroom"));
    }
    if let Some(name) = &options.default_function {
        if !valid_function_name(name) {
            return Err(Error::new("bad function name"));
        }
    }
    let mut warnings = Vec::new();
    let mut invocations = Vec::new();
    for input in inputs {
        let parsed = parse_input(&ParseInput {
            name: &input.name,
            bytes: &input.bytes,
            strict: input.strict,
            default_arch: options.default_arch,
            default_function: options.default_function.as_deref(),
            functions: &options.functions,
        })?;
        if let Some(skipped) = parsed.skipped {
            warnings.push(skipped);
        }
        invocations.extend(parsed.invocations);
    }
    if invocations.is_empty() {
        return Err(Error::new("no REPORT lines"));
    }
    price_invocations(invocations, options, warnings)
}

pub fn execute_paths(paths: &[std::path::PathBuf], options: &Options) -> Result<Output, Error> {
    let mut named = Vec::new();
    let mut warnings = Vec::new();
    let mut total = 0u64;
    for path in paths {
        if path.as_os_str() == "-" {
            let mut buf = Vec::new();
            std::io::stdin()
                .take(MAX_FILE_BYTES + 1)
                .read_to_end(&mut buf)
                .map_err(|err| Error::new(format!("-: {err}")))?;
            if buf.len() as u64 > MAX_FILE_BYTES {
                return Err(Error::new("-: file is larger than 268435456 bytes"));
            }
            take_from_total(&mut total, buf.len() as u64)?;
            named.push(NamedBytes {
                name: "-".to_string(),
                bytes: buf,
                strict: true,
            });
            continue;
        }
        push_explicit(path, &mut named, &mut warnings, &mut total)?;
    }
    let mut bill = bill_from_inputs(&named, options)?;
    let mut all = warnings;
    all.append(&mut bill.warnings);
    bill.warnings = all;
    let warnings = bill.warnings.clone();
    let stdout = if options.markdown && !options.json {
        render_markdown(&bill)
    } else {
        render(&bill, options.json)
    };
    let functions = bill
        .rows
        .iter()
        .map(|row| row.function.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    Ok(Output {
        stdout,
        warnings,
        requests: bill.totals.requests,
        functions,
    })
}

fn push_explicit(
    path: &Path,
    named: &mut Vec<NamedBytes>,
    warnings: &mut Vec<String>,
    total: &mut u64,
) -> Result<(), Error> {
    let symlink_meta = fs::symlink_metadata(path)
        .map_err(|err| Error::new(format!("{}: {err}", show_path(path))))?;
    let meta = if symlink_meta.file_type().is_symlink() {
        fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", show_path(path))))?
    } else {
        symlink_meta
    };
    if meta.is_dir() {
        return collect_dir(path, 1, named, warnings, total);
    }
    if !meta.is_file() {
        return Err(Error::new(format!("{}: not a file", show_path(path))));
    }
    let bytes = read_limited(path, total)?;
    named.push(NamedBytes {
        name: show_path(path),
        bytes,
        strict: true,
    });
    Ok(())
}

fn collect_dir(
    dir: &Path,
    depth: usize,
    named: &mut Vec<NamedBytes>,
    warnings: &mut Vec<String>,
    total: &mut u64,
) -> Result<(), Error> {
    if depth > 64 {
        return Err(Error::new(format!(
            "{}: directory is nested too deeply",
            show_path(dir)
        )));
    }
    let entries =
        fs::read_dir(dir).map_err(|err| Error::new(format!("{}: {err}", show_path(dir))))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| Error::new(format!("{}: {err}", show_path(dir))))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| Error::new(format!("{}: {err}", show_path(&path))))?;
        let kind = meta.file_type();
        if kind.is_symlink() {
            warnings.push(format!("skipped symlink {}", show_path(path)));
            continue;
        }
        if kind.is_dir() {
            collect_dir(&path, depth + 1, named, warnings, total)?;
        } else if kind.is_file() {
            let bytes = read_limited(&path, total)?;
            named.push(NamedBytes {
                name: show_path(path),
                bytes,
                strict: false,
            });
        } else {
            warnings.push(format!("skipped non-regular file {}", show_path(path)));
        }
    }
    Ok(())
}

/// Reads a UTF-8 text file, refusing anything above `MAX_FILE_BYTES` before
/// the whole file is in memory.
pub fn read_text_limited(path: &Path) -> Result<String, Error> {
    let bytes = read_limited(path, &mut 0)?;
    String::from_utf8(bytes).map_err(|_| {
        Error::new(format!(
            "{}: stream did not contain valid UTF-8",
            show_path(path)
        ))
    })
}

/// Adds `bytes` to the input read so far in this run. All input files of one
/// run together may not exceed `MAX_FILE_BYTES`, so time and memory stay
/// bounded by one limit and not by the number of files.
fn take_from_total(total: &mut u64, bytes: u64) -> Result<(), Error> {
    *total = total.saturating_add(bytes);
    if *total > MAX_FILE_BYTES {
        return Err(Error::new("input is larger than 268435456 bytes in total"));
    }
    Ok(())
}

fn read_limited(path: &Path, total: &mut u64) -> Result<Vec<u8>, Error> {
    let meta =
        fs::metadata(path).map_err(|err| Error::new(format!("{}: {err}", show_path(path))))?;
    // A FIFO or a device would block or never end; a log file is refused the same way.
    if !meta.is_file() {
        return Err(Error::new(format!("{}: not a file", show_path(path))));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{}: file is larger than 268435456 bytes",
            show_path(path)
        )));
    }
    take_from_total(total, meta.len())?;
    let file =
        fs::File::open(path).map_err(|err| Error::new(format!("{}: {err}", show_path(path))))?;
    let mut buf = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|err| Error::new(format!("{}: {err}", show_path(path))))?;
    if buf.len() as u64 > MAX_FILE_BYTES {
        return Err(Error::new(format!(
            "{}: file is larger than 268435456 bytes",
            show_path(path)
        )));
    }
    Ok(buf)
}

/// The most rows (function, architecture and memory size) one run may report.
/// Every row costs memory in the report and, with `--json`, about 6 KB more.
const MAX_ROWS: usize = 100_000;

fn price_invocations(
    invocations: Vec<Invocation>,
    options: &Options,
    mut warnings: Vec<String>,
) -> Result<Bill, Error> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut duplicates = 0_u64;
    let mut groups: BTreeMap<(String, Arch, u32), Acc> = BTreeMap::new();
    for invocation in invocations {
        let key = (invocation.function.clone(), invocation.request_id);
        if !seen.insert(key) {
            duplicates += 1;
            continue;
        }
        let group_key = (invocation.function, invocation.arch, invocation.memory_mb);
        if groups.len() >= MAX_ROWS && !groups.contains_key(&group_key) {
            return Err(Error::new("more than 100000 function and memory rows"));
        }
        let group = groups.entry(group_key).or_default();
        group.requests = checked(group.requests, 1, "requests")?;
        if invocation.cold {
            group.cold_starts = checked(group.cold_starts, 1, "cold starts")?;
        }
        group.billed_ms = checked(group.billed_ms, invocation.billed_ms, "billed duration")?;
        group.max_used_mb = group.max_used_mb.max(invocation.max_used_mb);
    }
    if duplicates > 0 {
        let noun = if duplicates == 1 {
            "RequestId"
        } else {
            "RequestIds"
        };
        warnings.push(format!("skipped {duplicates} duplicate {noun}"));
    }
    let mut rows = Vec::with_capacity(groups.len());
    let mut totals = Totals {
        requests: 0,
        cold_starts: 0,
        gb_seconds: Decimal::ZERO,
        duration_usd: Decimal::ZERO,
        request_usd: Decimal::ZERO,
        total_usd: Decimal::ZERO,
    };
    // Totals are the exact sum. A sum that is not an exact decimal is an error.
    let mut sum_gb = Ratio::zero();
    let mut sum_duration = Ratio::zero();
    let mut sum_request = Ratio::zero();
    let mut sum_total = Ratio::zero();
    for ((function, arch, memory_mb), acc) in groups {
        let rate = options.prices.rate(arch);
        let gb_exact = exact_gb(memory_mb, acc.billed_ms);
        let duration_exact = exact_duration(memory_mb, acc.billed_ms, rate);
        let request_exact = exact_request(acc.requests, options.prices.request_per_million);
        let total_exact = duration_exact.clone().add(request_exact.clone());
        let gb = exact_decimal(&gb_exact)?;
        let duration = exact_decimal(&duration_exact)?;
        let requests = exact_decimal(&request_exact)?;
        let total = exact_decimal(&total_exact)?;
        let advice = advise(memory_mb, acc.max_used_mb, options.headroom_percent);
        let advised = duration_usd(advice.memory_mb(memory_mb), acc.billed_ms, rate)?;
        totals.requests = checked(totals.requests, acc.requests, "requests")?;
        totals.cold_starts = checked(totals.cold_starts, acc.cold_starts, "cold starts")?;
        sum_gb = sum_gb.add(gb_exact);
        sum_duration = sum_duration.add(duration_exact);
        sum_request = sum_request.add(request_exact);
        sum_total = sum_total.add(total_exact);
        rows.push(Row {
            function,
            architecture: arch,
            memory_mb,
            max_memory_used_mb: acc.max_used_mb,
            requests: acc.requests,
            cold_starts: acc.cold_starts,
            gb_seconds: gb,
            duration_usd: duration,
            request_usd: requests,
            total_usd: total,
            advice,
            duration_usd_if_duration_stays: advised,
        });
    }
    totals.gb_seconds = exact_decimal(&sum_gb)?;
    totals.duration_usd = exact_decimal(&sum_duration)?;
    totals.request_usd = exact_decimal(&sum_request)?;
    totals.total_usd = exact_decimal(&sum_total)?;
    Ok(Bill {
        prices: options.prices.clone(),
        headroom_percent: options.headroom_percent,
        rows,
        warnings,
        totals,
    })
}

#[derive(Default)]
struct Acc {
    requests: u64,
    cold_starts: u64,
    max_used_mb: u32,
    billed_ms: u64,
}

fn checked(left: u64, right: u64, what: &str) -> Result<u64, Error> {
    left.checked_add(right)
        .ok_or_else(|| Error::new(format!("{what} overflow")))
}
