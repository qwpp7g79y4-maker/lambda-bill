//! Offline estimate of AWS Lambda on-demand cost from CloudWatch `REPORT` lines.
//!
//! The library reads bytes you already have. It does not open a socket and it
//! does not link an AWS SDK.
#![forbid(unsafe_code)]

mod advice;
mod arch;
mod budget;
mod error;
mod money;
mod parse;
mod price;
mod report;
mod run;

pub use advice::{advise, Advice};
pub use arch::Arch;
pub use error::{escape_unsafe, safe_clap_message, show_path, Error};
pub use money::{format_usd, gb_seconds};
pub use price::{load_functions_str, load_prices_str, prices_builtin, Prices, PRICE_MAX_AGE_DAYS};
pub use report::{
    render, render_markdown, Bill, Row, Totals, ADVICE_NOTE, FREE_TIER_LINE, PC_LINE,
    SCHEMA_VERSION,
};
pub use run::{
    bill_from_inputs, duration_usd, execute_paths, read_text_limited, request_usd, NamedBytes,
    Options, Output,
};

/// Largest input accepted per file, in bytes (256 MiB).
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Longest line accepted, in bytes (1 MiB).
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// Account free tier, documented and not applied.
pub const FREE_TIER_REQUESTS: u64 = 1_000_000;

/// Account free tier, documented and not applied.
pub const FREE_TIER_GB_SECONDS: u64 = 400_000;
