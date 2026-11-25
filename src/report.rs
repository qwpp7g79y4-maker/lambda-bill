use std::fmt::Write as _;

use rust_decimal::Decimal;
use serde_json::{json, Value};

use crate::advice::Advice;
use crate::arch::Arch;
use crate::money::format_usd;
use crate::price::Prices;

/// Sentence printed with every report. Advice is not a duration forecast.
pub const ADVICE_NOTE: &str = "advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.";

pub const FREE_TIER_LINE: &str = "free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account";

pub const PC_LINE: &str =
    "provisioned concurrency not included: only on-demand duration and requests are priced";

/// Version of the JSON layout. It is the first key of every JSON document.
pub const SCHEMA_VERSION: u64 = 1;

pub const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

/// One function, one architecture, one configured memory size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub function: String,
    pub architecture: Arch,
    pub memory_mb: u32,
    pub max_memory_used_mb: u32,
    pub requests: u64,
    pub cold_starts: u64,
    pub gb_seconds: Decimal,
    pub duration_usd: Decimal,
    pub request_usd: Decimal,
    pub total_usd: Decimal,
    pub advice: Advice,
    pub duration_usd_if_duration_stays: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totals {
    pub requests: u64,
    pub cold_starts: u64,
    pub gb_seconds: Decimal,
    pub duration_usd: Decimal,
    pub request_usd: Decimal,
    pub total_usd: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bill {
    pub prices: Prices,
    pub headroom_percent: u8,
    pub rows: Vec<Row>,
    pub warnings: Vec<String>,
    pub totals: Totals,
}

#[must_use]
pub fn render(bill: &Bill, json_output: bool) -> String {
    if json_output {
        render_json(bill)
    } else {
        render_text(bill)
    }
}

/// Escape text for a Markdown table cell or list item: a backslash is doubled
/// first, so a source `\|` cannot turn into an escaped backslash followed by a
/// live `|`; control characters and line breaks become `\u{..}`; then `|` is
/// escaped.
fn escape_cell(text: &str) -> String {
    crate::escape_unsafe(&text.replace('\\', "\\\\")).replace('|', "\\|")
}

/// The same amounts as the text report, as a Markdown document with a table.
#[must_use]
pub fn render_markdown(bill: &Bill) -> String {
    let mut out = String::new();
    let prices = &bill.prices;
    out.push_str("# Lambda cost estimate\n\n");
    let _ = writeln!(out, "{}\n", escape_cell(ESTIMATE_LINE));
    let _ = writeln!(
        out,
        "- prices: {}, source {}, date {}, region {}, tier {}",
        escape_cell(&prices.currency),
        escape_cell(&prices.source),
        escape_cell(&prices.date),
        escape_cell(&prices.region),
        escape_cell(&prices.tier)
    );
    let _ = writeln!(out, "- {}", escape_cell(FREE_TIER_LINE));
    let _ = writeln!(out, "- {}", escape_cell(PC_LINE));
    let _ = writeln!(out, "- headroom: {}%", bill.headroom_percent);
    if !bill.warnings.is_empty() {
        out.push_str("\n### Warnings\n\n");
        for warning in &bill.warnings {
            let _ = writeln!(out, "- {}", escape_cell(warning));
        }
    }
    out.push_str("\n### Cost\n\n");
    let header = [
        "function",
        "architecture",
        "memory_mb",
        "max_used_mb",
        "requests",
        "cold_starts",
        "gb_seconds",
        "duration_usd",
        "request_usd",
        "total_usd",
        "advice",
    ];
    let _ = writeln!(out, "| {} |", header.join(" | "));
    let _ = writeln!(
        out,
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |"
    );
    for row in &bill.rows {
        let cells = [
            escape_cell(&row.function),
            row.architecture.to_string(),
            row.memory_mb.to_string(),
            row.max_memory_used_mb.to_string(),
            row.requests.to_string(),
            row.cold_starts.to_string(),
            format_dec_gb(&row.gb_seconds),
            format_usd(row.duration_usd),
            format_usd(row.request_usd),
            format_usd(row.total_usd),
            escape_cell(&row.advice.label()),
        ];
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    }
    let totals = &bill.totals;
    let _ = writeln!(
        out,
        "| total |  |  |  | {} | {} | {} | {} | {} | {} |  |",
        totals.requests,
        totals.cold_starts,
        format_dec_gb(&totals.gb_seconds),
        format_usd(totals.duration_usd),
        format_usd(totals.request_usd),
        format_usd(totals.total_usd)
    );
    let _ = writeln!(out, "\nestimated_usd={}", format_usd(totals.total_usd));
    let advice: Vec<&Row> = bill
        .rows
        .iter()
        .filter(|row| !matches!(row.advice, Advice::Keep))
        .collect();
    if !advice.is_empty() {
        out.push_str("\n### Advice\n\n");
        for row in advice {
            let _ = writeln!(
                out,
                "- {} {} {} MB: {}; duration USD {} -> {} if billed duration stays the same",
                escape_cell(&row.function),
                row.architecture,
                row.memory_mb,
                escape_cell(&row.advice.label()),
                format_usd(row.duration_usd),
                format_usd(row.duration_usd_if_duration_stays),
            );
        }
    }
    let _ = writeln!(out, "\nnote: {}", escape_cell(ADVICE_NOTE));
    out
}

fn render_text(bill: &Bill) -> String {
    let mut out = String::new();
    for warning in &bill.warnings {
        let _ = writeln!(out, "warning: {warning}");
    }
    let prices = &bill.prices;
    let _ = writeln!(
        out,
        "prices: {}, source {}, date {}, region {}, tier {}",
        prices.currency, prices.source, prices.date, prices.region, prices.tier
    );
    let _ = writeln!(out, "{FREE_TIER_LINE}");
    let _ = writeln!(out, "{PC_LINE}");
    let _ = writeln!(out, "{ESTIMATE_LINE}");
    let _ = writeln!(out, "headroom: {}%", bill.headroom_percent);
    out.push('\n');

    let header = [
        "function",
        "architecture",
        "memory_mb",
        "max_used_mb",
        "requests",
        "cold_starts",
        "gb_seconds",
        "duration_usd",
        "request_usd",
        "total_usd",
        "advice",
    ]
    .map(str::to_string);
    let mut table = vec![header.to_vec()];
    for row in &bill.rows {
        table.push(vec![
            row.function.clone(),
            row.architecture.to_string(),
            row.memory_mb.to_string(),
            row.max_memory_used_mb.to_string(),
            row.requests.to_string(),
            row.cold_starts.to_string(),
            format_dec_gb(&row.gb_seconds),
            format_usd(row.duration_usd),
            format_usd(row.request_usd),
            format_usd(row.total_usd),
            row.advice.label(),
        ]);
    }
    let totals = &bill.totals;
    table.push(vec![
        "total".to_string(),
        String::new(),
        String::new(),
        String::new(),
        totals.requests.to_string(),
        totals.cold_starts.to_string(),
        format_dec_gb(&totals.gb_seconds),
        format_usd(totals.duration_usd),
        format_usd(totals.request_usd),
        format_usd(totals.total_usd),
        String::new(),
    ]);
    push_table(&mut out, &table);
    out.push('\n');
    let _ = writeln!(out, "estimated_usd={}", format_usd(totals.total_usd));
    for row in &bill.rows {
        if matches!(row.advice, Advice::Keep) {
            continue;
        }
        let _ = writeln!(
            out,
            "advice {} {} {} MB: {}; duration USD {} -> {} if billed duration stays the same",
            row.function,
            row.architecture,
            row.memory_mb,
            row.advice.label(),
            format_usd(row.duration_usd),
            format_usd(row.duration_usd_if_duration_stays),
        );
    }
    let _ = writeln!(out, "note: {ADVICE_NOTE}");
    out
}

fn format_dec_gb(value: &Decimal) -> String {
    crate::money::format_dec(*value)
}

fn push_table(out: &mut String, rows: &[Vec<String>]) {
    let width_count = rows[0].len();
    let mut widths = vec![0; width_count];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    let right = [
        false, false, true, true, true, true, true, true, true, true, false,
    ];
    for row in rows {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            if right[index] {
                let _ = write!(line, "{:>w$}", cell, w = widths[index]);
            } else {
                let _ = write!(line, "{:<w$}", cell, w = widths[index]);
            }
        }
        out.push_str(line.trim_end_matches(|c: char| c.is_ascii_whitespace()));
        out.push('\n');
    }
}

fn render_json(bill: &Bill) -> String {
    let prices = &bill.prices;
    let functions: Vec<Value> = bill
        .rows
        .iter()
        .map(|row| {
            json!({
                "function": row.function,
                "architecture": row.architecture.as_str(),
                "memory_mb": row.memory_mb,
                "max_memory_used_mb": row.max_memory_used_mb,
                "requests": row.requests,
                "cold_starts": row.cold_starts,
                "gb_seconds": format_dec_gb(&row.gb_seconds),
                "duration_usd": format_usd(row.duration_usd),
                "request_usd": format_usd(row.request_usd),
                "total_usd": format_usd(row.total_usd),
                "advice": row.advice.action(),
                "advice_mb": row.advice.memory_mb(row.memory_mb),
                "duration_usd_if_duration_stays": format_usd(row.duration_usd_if_duration_stays),
            })
        })
        .collect();
    let totals = &bill.totals;
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "currency": prices.currency,
        "source": prices.source,
        "date": prices.date,
        "region": prices.region,
        "tier": prices.tier,
        "estimate": true,
        "estimate_note": ESTIMATE_LINE,
        "free_tier_applied": false,
        "provisioned_concurrency_included": false,
        "headroom_percent": bill.headroom_percent,
        "advice_note": ADVICE_NOTE,
        "warnings": bill.warnings,
        "functions": functions,
        "totals": {
            "requests": totals.requests,
            "cold_starts": totals.cold_starts,
            "gb_seconds": format_dec_gb(&totals.gb_seconds),
            "duration_usd": format_usd(totals.duration_usd),
            "request_usd": format_usd(totals.request_usd),
            "total_usd": format_usd(totals.total_usd),
            "estimated_usd": format_usd(totals.total_usd),
        }
    });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::push_table;

    #[test]
    fn push_table_measures_width_in_chars_not_bytes() {
        let mut row_a = vec!["x".to_string(); 11];
        let mut row_b = row_a.clone();
        row_a[0] = "héllo-ñ".to_string();
        row_b[0] = "abcdefg".to_string();
        row_b[10] = "end".to_string();
        row_a[10] = "end".to_string();
        let mut out = String::new();
        push_table(&mut out, &[row_a, row_b]);
        let cols: Vec<usize> = out
            .lines()
            .map(|l| l.find("  x").map(|i| l[..i].chars().count()).unwrap())
            .collect();
        assert_eq!(cols[0], cols[1]);
        assert_eq!(cols[0], 7);
    }

    #[test]
    fn push_table_keeps_a_trailing_nbsp_on_a_row() {
        let mut row = vec!["x".to_string(); 11];
        row[10] = "end\u{a0}".to_string();
        let mut out = String::new();
        push_table(&mut out, &[row]);
        assert!(out.ends_with("end\u{a0}\n"), "{out:?}");
    }
}
