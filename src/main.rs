#![deny(unsafe_code)]

use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[allow(unsafe_code)]
mod interrupt;
#[allow(unsafe_code)]
mod stdout_probe;

use lambda_bill::{
    execute_paths, load_functions_str, load_prices_str, show_path, Arch, Error, Options,
};

// `-V` and `--version` print this. A debug build adds the commit that git
// reported at compile time; a release build is the crate version only.
#[cfg(all(debug_assertions, lambda_bill_git_sha))]
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("LAMBDA_BILL_GIT_SHA"),
    ")"
);

#[cfg(not(all(debug_assertions, lambda_bill_git_sha)))]
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estimate AWS Lambda on-demand cost from CloudWatch REPORT lines. Reads files only, no network.
#[derive(Parser)]
#[command(
    name = "lambda-bill",
    bin_name = "lambda-bill",
    term_width = 0,
    version = VERSION,
    about = "Estimate AWS Lambda on-demand cost from CloudWatch REPORT lines. Reads files only, no network.",
    after_help = "Exit codes: 0 ok, 2 input or usage error, 3 --quiet together with --verbose, 130 SIGINT."
)]
struct Cli {
    /// Log files, gzip exports, directories, or `-` for standard input.
    #[arg(required = true, value_name = "FILE")]
    files: Vec<PathBuf>,
    /// Price file. The default is the built-in first-tier on-demand list.
    #[arg(long, value_name = "FILE")]
    prices: Option<PathBuf>,
    /// Architecture when the file does not say. `x86_64` (default) or `arm64`.
    #[arg(long, value_name = "ARCH", default_value = "x86_64")]
    arch: String,
    /// Function name when the file does not say one.
    #[arg(long, value_name = "NAME")]
    function: Option<String>,
    /// JSON object of function name to architecture (`x86_64` or `arm64`).
    #[arg(long, value_name = "FILE")]
    functions: Option<PathBuf>,
    /// Percent above the memory base for the advice. An integer from 0 to 100. Default 20.
    #[arg(long, value_name = "PERCENT", default_value_t = 20, value_parser = parse_headroom)]
    headroom: u8,
    /// Date for the price age check as YYYY-MM-DD. Default: today (UTC).
    #[arg(long, value_name = "DATE")]
    as_of: Option<String>,
    /// Print only the result and errors. No warning on standard error.
    #[arg(long)]
    quiet: bool,
    /// Also print progress on standard error.
    #[arg(long)]
    verbose: bool,
    /// Write the report to this file instead of standard output. An existing regular file is replaced.
    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// Print JSON instead of the text table.
    #[arg(long)]
    json: bool,
    /// Print a Markdown table instead of the text table.
    #[arg(long, conflicts_with = "json")]
    md: bool,
}

fn parse_headroom(text: &str) -> Result<u8, String> {
    let value: u8 = text.parse().map_err(|_| {
        format!(
            "invalid --headroom {}: expected an integer from 0 to 100",
            lambda_bill::escape_unsafe(text)
        )
    })?;
    if value > 100 {
        return Err(format!(
            "invalid --headroom {value}: expected an integer from 0 to 100"
        ));
    }
    Ok(value)
}

fn main() -> ExitCode {
    interrupt::install();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if err.use_stderr() {
                // The message is built from the arguments with every unsafe
                // character already written as `\u{..}`, so a CR, ESC or bidi
                // mark in an argument cannot rewrite the terminal line or add
                // a fake line, while clap's own line breaks stay.
                let rendered_escaped = if err.kind() == clap::error::ErrorKind::InvalidUtf8 {
                    // This message never repeats the argument, so its own
                    // layout is already safe to keep.
                    Some(err.to_string())
                } else {
                    let escaped: Vec<String> = std::iter::once("lambda-bill".to_string())
                        .chain(
                            std::env::args_os()
                                .skip(1)
                                .map(|arg| lambda_bill::escape_unsafe(&arg.to_string_lossy())),
                        )
                        .collect();
                    Cli::try_parse_from(escaped)
                        .err()
                        .map(|escaped_err| escaped_err.to_string())
                };
                let text =
                    lambda_bill::safe_clap_message(&err.to_string(), rendered_escaped.as_deref());
                let _ = std::io::stderr().write_all(text.as_bytes());
                return ExitCode::from(2);
            }
            if write_stdout(err.to_string().as_bytes()).is_err() {
                let _ = writeln!(std::io::stderr(), "lambda-bill: stdout is not writable");
                return ExitCode::from(2);
            }
            return ExitCode::SUCCESS;
        }
    };
    if cli.quiet && cli.verbose {
        let _ = writeln!(
            std::io::stderr(),
            "lambda-bill: --quiet and --verbose cannot be used together"
        );
        return ExitCode::from(3);
    }
    if cli.verbose {
        for path in &cli.files {
            let _ = writeln!(
                std::io::stderr(),
                "lambda-bill: reading {}",
                show_path(path)
            );
        }
    }
    match run(&cli) {
        Ok(output) => {
            if cli.json && !cli.quiet {
                for warning in &output.warnings {
                    let _ = writeln!(std::io::stderr(), "warning: {warning}");
                }
            }
            interrupt::begin_write();
            let written = match &cli.output {
                Some(path) => write_output_file(path, output.stdout.as_bytes(), cli.verbose),
                None => write_stdout(output.stdout.as_bytes())
                    .map_err(|_| "lambda-bill: could not write output".to_string()),
            };
            if interrupt::end_write() {
                let _ = std::io::stderr().write_all(interrupt::MESSAGE.as_bytes());
                return ExitCode::from(interrupt::EXIT_INTERRUPTED);
            }
            if let Err(message) = written {
                let _ = writeln!(std::io::stderr(), "{message}");
                return ExitCode::from(2);
            }
            if cli.verbose {
                let _ = writeln!(
                    std::io::stderr(),
                    "lambda-bill: done: {} functions, {} requests",
                    output.functions,
                    output.requests
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "lambda-bill: {err}");
            ExitCode::from(2)
        }
    }
}

/// Write the report to `path` through a temporary file in the same directory,
/// then rename it over the target. A symlink or anything that is not a regular
/// file at `path` is refused, so the write never follows a link and never
/// replaces a device or a directory. The temporary file is removed on failure.
fn write_output_file(path: &std::path::Path, bytes: &[u8], verbose: bool) -> Result<(), String> {
    let shown = show_path(path);
    let fail = |why: &str| format!("lambda-bill: cannot write {shown}: {why}");
    let replacing = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(fail("it is a symlink, not a regular file"));
        }
        Ok(meta) if !meta.is_file() => return Err(fail("not a regular file")),
        Ok(_) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(fail(&lambda_bill::escape_unsafe(&err.to_string()))),
    };
    let Some(name) = path.file_name() else {
        return Err(fail("no file name"));
    };
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let mut attempt = 0_u32;
    let (temp, mut file) = loop {
        let mut temp_name = std::ffi::OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(".{}.{attempt}.tmp", std::process::id()));
        let temp = dir.join(temp_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            Ok(file) => break (temp, file),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && attempt < 100 => {
                attempt += 1;
            }
            Err(err) => return Err(fail(&lambda_bill::escape_unsafe(&err.to_string()))),
        }
    };
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            std::fs::rename(&temp, path)
        });
    if let Err(err) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(fail(&lambda_bill::escape_unsafe(&err.to_string())));
    }
    if replacing && verbose {
        let _ = writeln!(std::io::stderr(), "lambda-bill: replacing {shown}");
    }
    Ok(())
}

/// Write all of `bytes` to stdout and flush. Fails when stdout was closed at
/// start or the write or flush fails.
fn write_stdout(bytes: &[u8]) -> std::io::Result<()> {
    if stdout_closed() {
        return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    }
    let mut out = stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

/// True when fd 1 was closed (`>&-`) or open for reading only (`1<file`) at
/// process start. `1<>/dev/null` and `>/dev/null` are writable. A missing
/// `/proc` changes nothing.
fn stdout_closed() -> bool {
    stdout_probe::stdout_unwritable_at_start()
}

fn run(cli: &Cli) -> Result<lambda_bill::Output, Error> {
    let Some(arch) = Arch::parse(&cli.arch) else {
        return Err(Error::new(format!(
            "invalid --arch {}: expected x86_64 or arm64",
            lambda_bill::escape_unsafe(&cli.arch)
        )));
    };
    let prices = if let Some(path) = &cli.prices {
        let text = lambda_bill::read_text_limited(path)?;
        load_prices_str(&text)
            .map_err(|err| Error::new(format!("price file {}: {err}", show_path(path))))?
    } else {
        lambda_bill::prices_builtin()
    };
    let functions = if let Some(path) = &cli.functions {
        let text = lambda_bill::read_text_limited(path)?;
        load_functions_str(&text)
            .map_err(|err| Error::new(format!("functions file {}: {err}", show_path(path))))?
    } else {
        std::collections::BTreeMap::new()
    };
    let options = Options {
        prices,
        default_arch: arch,
        default_function: cli.function.clone(),
        functions,
        headroom_percent: cli.headroom,
        json: cli.json,
        markdown: cli.md,
    };
    let age_warning = options.prices.age_warning(cli.as_of.as_deref())?;
    if let Some(warning) = age_warning.filter(|_| !cli.quiet) {
        let _ = writeln!(std::io::stderr(), "warning: {warning}");
    }
    execute_paths(&cli.files, &options)
}
