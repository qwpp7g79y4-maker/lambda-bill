# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- All input files of one run together are limited to 268435456 bytes (`input is larger than 268435456 bytes in total`), and a gzip file to 1000000 members (`more than 1000000 gzip members`). Before, four files of 265 MB took 11.9 s and 2.9 GB, and 13000000 empty gzip members took 35 s.
- A run reports at most 100000 rows (one per function, architecture and memory size): more is `more than 100000 function and memory rows`, exit 2. Before, 1650000 functions with `--json` took 29 s and 9.4 GB.
- `QUALITY.md` and `QUALITY.nl.md` name code commit e35a324 as the commit the measurements apply to (a row that was not run there names the commit it was last run on). Commits after that one change documents only.
- The tests no longer depend on the system clock. Each test that runs the binary passes a fixed `--as-of` (a test with its own `--as-of` keeps it), because the price age warning would otherwise turn tests red once the date is more than 180 days after the price date (2027-05-09 for the built-in prices). Proof: a copy of the tree with the clock 400 days ahead failed the suite before the change and passes it after. No change in the behavior of the tool.
- The test count is 271 (debug `cargo test --locked --offline` on e35a324, 66 of them in the library). `QUALITY.md` and `QUALITY.nl.md` state under the title which commit the measurements apply to, and list everything that was not run once, in one section (two new tests in `tests/quality.rs` check both).

### Added

- `THIRD-PARTY-LICENSES.html`: the licenses of the third-party crates in the binary, generated with `cargo about generate --locked about.hbs`.
- Documentation to the standard of the other tools: `README.md` has Installation, Quick start (it writes only into a temporary directory), Aan de slag in 5 minuten, Options, Example, Exit codes, Prices and responsibility, Maintenance, Support and Contact; `README.nl.md` says the same in Dutch; `CONTRIBUTING.md`, `QUALITY.nl.md`, `examples/README.md` and the expected outputs `examples/*.expected.*` are new, and `SECURITY.md` names the contact address. Tests compare the sections, the console blocks, the claims and the figures of the two READMEs and run the example commands.
- `build.rs`: a debug build prints the commit after the version in `--version`; a release build, a build without git and a packaged crate print the version only.
- `about.hbs` and `about.toml` for `cargo about`, and `.github/workflows/release.yml`, which builds five archives with the third-party license listing when a version tag is pushed. The workflow has not run.

- `schema_version` (1) as the first key of the JSON document. Errors print no JSON, so every JSON document has it. The library exports `SCHEMA_VERSION`.
- `--output FILE`: the report goes to a temporary file in the same directory and is renamed over the target. An existing regular file is replaced; a symlink, a directory or a device is refused; a missing directory is not created; a write failure exits 2 with one `cannot write` line (ci-bill exits 1 there). A SIGINT or a failed run leaves an existing file as it was.
- SIGINT (Ctrl-C) ends the program with exit code 130 and the line `lambda-bill: interrupted by SIGINT` on standard error, with nothing on standard output. A SIGINT during the write of the report lets the write finish and then exits 130. `src/interrupt.rs` holds the handler (Unix only; other systems keep the default). The test `sigint_exits_130_with_a_message_and_no_output` sends a real SIGINT with `kill`.
- `--md`: the same amounts as a Markdown table, with the estimate line, the warnings and the advice. A `|`, a backslash and a line break in a cell are escaped. `--md` with `--json` is a usage error. `Options` has a `markdown` field and the library exports `render_markdown`.
- `--quiet` and `--verbose`. Quiet keeps the report and the errors and drops the warnings on standard error; verbose adds `reading` and `done:` lines on standard error. Both together exit 3, the exit code ci-bill uses for a usage error. `Output` has two new fields, `requests` and `functions`.
- `tests/documented_shapes.rs`: nine tests with the input shapes the AWS documentation shows (the `filter-log-events` document, plain lines with a timestamp and a stream name, CRLF, gzip, special characters in log group names, missing optional fields, empty and overlapping pages), each priced to the exact amount.
- `--as-of DATE`, and a warning on standard error when the date of the price file is more than 180 days before it (today in UTC when `--as-of` is not given). Exit code 0. A bad date exits 2.
- The text report prints `Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)` and a line `estimated_usd=` with the total. The JSON has `"estimate": true`, `"estimate_note"` and `totals.estimated_usd` (the same value as `totals.total_usd`). The label has no `_month`: the total is the export you passed, never scaled to a month.
- `SECURITY.md`, `man/lambda-bill.1` (it holds the `--help` text verbatim) and `tests/handbook_help.rs`, which checks the man page against `--help`.
- `tests/docs_clean.rs`, which fails when the README, QUALITY, CHANGELOG, SECURITY or man page contains a local absolute path, a build-wrapper option, the name of a file that stays out of the package, or the name of an internal tool or an AI system. The test source holds those words only in pieces.
- `.gitattributes` with `* text=auto eol=lf`, so the man page, the examples and the printed tables compare byte for byte in a checkout with CRLF line endings.
- Closed or read-only standard output is recognized at process start. On Linux a constructor in `src/stdout_probe.rs` records whether file descriptor 1 was already closed or open for reading only, because Rust replaces a closed descriptor with `/dev/null` before `main` and ignores the error of a write to a descriptor that is not open for writing. The library has `#![forbid(unsafe_code)]` and the binary `#![deny(unsafe_code)]`, with an exception for that module and for `src/interrupt.rs`.
- Public `escape_unsafe`, `safe_clap_message` and `read_text_limited`.
- `tests/quality.rs`, which compares the test count in `README.md` and `QUALITY.md` with the number of tests in `src/` and `tests/`.

- `.github/dependabot.yml`, `scripts/check-glibc.sh` and a `glibc` job in the workflow that fails when a release build on Ubuntu 22.04 needs a glibc symbol newer than 2.34. The workflow also has a time limit on every job, `persist-credentials: false` on every checkout, and `--no-fail-fast` on the test runs. The script stays out of the package.
- `tests/docs_clean.rs` now scans the manifest (without its `exclude` line), `src/`, `tests/`, `examples/`, `prices/`, `scripts/` and `.github/` as well, has more patterns, and plants a word in every scanned place to prove the scan reaches it.
- `src/budget.rs` with its tests and four tests in `tests/cli.rs` for the value budget (a document, a gzip file, a `--prices` and a `--functions` file over it, and a bracket text log under it).
- Tests for the documented limits: a product that is not an exact decimal is an error, the number fields are limited by their text length, text before a JSON line makes it text, a `# function:` header after the first report counts for the file, and a duplicate RequestId with other values is only counted.

### Changed

- A line that starts with an invisible character (BOM, zero-width space, bidirectional mark, ...) is `invisible character at the start of the line`, exit 2, also in front of a `# function:` or `# arch:` header or a JSON line. Such a header used to be skipped without a word and put requests under the wrong function or architecture. A single BOM at the start of the file is still accepted.
- The exact arithmetic divides with native 128-bit integers, one-limb short division and Knuth's algorithm D instead of bit by bit with an allocation per bit. 400000 events that each name another function: 76.1 s before, 4.4 s now (same machine, same file, release build). The result is the same exact amount.
- `open_finding_temp_dir_repeated_slash` runs on Unix only.
- An amount that is not an exact 96-bit decimal (at most 29 significant digits, at most 28 digits after the decimal point) is `price overflow`, exit 2. This covers GB-seconds, duration, requests, the advice amount and totals. A product or a sum used to be rounded once and the run still exited 0: 1000 MB for 1 ms at `1000000000000000000000001` USD per GB-second was stored as `976562500000000000000.0009766` instead of refusing the exact `...0009765625`. One GB-second at `10^-28` USD stays exact. Two requests at the widest accepted request rate stay exact. At or above 90 percent of the configured memory, a raise wins over a downsize (117 MB used of 129 MB with headroom 5 raises to 136).
- A JSON document, a `--prices` file or a `--functions` file with more than 4000000 JSON values (a scalar or a key counts 1, an array or object 4) is `more than 4000000 JSON values`, exit 2. A counting pass checks it before the document is built, also for the text inside a gzip file. Before, `serde_json` built the whole document: `[[0],...]` of 100000001 bytes needed 9916 MB (99 times its size), nested arrays about 150 times, a gzip file of 240272 bytes about 9 GB, and a 15 MB `--prices` file 2.2 GB. A document at the budget needs under 1 GB (measured 0.36 to 0.62 GB, 0.6 to 2.3 s). This adds `serde` as a direct dependency (it was already in the tree).
- A `REPORT RequestId:` that is written with JSON escapes (`REPORT\u0020RequestId:`, `\u0052EPORT RequestId:`) in a field that is not read is an error, like the same text without escapes. The check now searches the decoded strings and keys. It used to count the marker in the raw text, so such a second request vanished with exit 0, in a document and in a JSON line.
- A `--prices` or `--functions` file that is not a regular file (a FIFO, `<(command)`, a device) is `not a file`, exit 2. It used to block on a FIFO without a writer. A log file was already refused this way.
- A `logGroup` and a `logGroupName` that name different groups in one object are `logGroup and logGroupName differ`. The message used to say that the log group is not `/aws/lambda/<name>`.
- A file of empty lines is read about five times faster: the line label is no longer built for a blank line (256 MiB of empty lines: 17.9 s before, 3.3 s now).

- If standard output was closed at start (`>&-`), is open for reading only (`1<file`, `1<.`), or a write or flush fails (`/dev/full`, a closed pipe), the exit code is 2 and standard error says so. This includes `--help` and `--version` (`lambda-bill: stdout is not writable`). `>/dev/null` and `1<>/dev/null` stay exit 0.
- A usage error that clap prints (no argument, an unknown option, `--headroom abc`, `--json=1`) keeps its own line breaks and ends with a line break. Each unsafe display character of an argument it repeats (newline, CR, ESC, bidi marks, invisible characters) is written as `\u{..}`, so an argument with an ESC sequence and a newline gives no fake line. The `--arch` and `--headroom` values in their own errors are escaped the same way.
- The set of characters replaced by U+FFFD in a path is wider: besides control characters, line and paragraph separators and bidi marks, it now covers soft hyphen, zero-width characters, word joiner, Hangul fillers, variation selectors, interlinear annotation marks and tag characters. A path that is not valid UTF-8 is shown as escaped bytes. A `--prices` source or label with such a character is rejected.
- Only ASCII white space is trimmed from log lines, headers, log groups and money strings. A log line with a non-ASCII white space character at either edge is an error. A money string padded with other Unicode space is rejected.
- A log line that starts with `{` and is not JSON is an error (`not JSON`). A line that starts with `[` and is not JSON is read as text (`[INFO]`, `[main]`, `[2024-01-01]`), except a line that opens like a JSON array (`[{`, `[[`, `["`, `[1,2`, `[true`, a lone `[`), which is an error (`not JSON`). A JSON or bracket line that contains `REPORT RequestId:` but yields no request is an error. A JSON log line is also read through its `@message` or `Message` text. A JSON object with both `message` and `events` is an error, also inside an array element or a nested event (`event N: has both message and events`).
- A `REPORT RequestId:` marker glued to a byte-order mark, a zero-width or other invisible character, or a non-ASCII space is an error (`not a Lambda REPORT line`, or `non-ASCII white space around line` at the edge of a line). It used to be skipped without a message, so a request could vanish from the total, for example when two files with a byte-order mark were joined.
- A file that is one JSON object with `@message` or `Message`, also printed over several lines, is read. It used to fail with `line 1: not JSON`.
- A JSON document whose text has a `REPORT RequestId:` that is not read as a request is an error (`document: not a Lambda REPORT line`). It used to be dropped without a message, for example the second REPORT in `{"message":R1,"@message":R2}`, `[{"message":R1,"x":R2}]` or `{"events":[{"message":R1}],"@message":R2}`, with exit 0 and one request.
- An object with `@message` or `Message` now reads `logGroup`, `logGroupName` and `architecture` like an event with `message`. It used to ignore them, so `{"@message":R,"architecture":"arm64","logGroup":"/aws/lambda/zzz"}` was priced as x86_64 and had no function name. A bad value is an error.
- The usage error for an argument that is not valid UTF-8 (`--function $'a\xff'`) keeps clap's own lines and ends with a line break. It used to be one line with literal `\u{a}` and no final line break.
- The package no longer contains `.github/` and `.gitattributes`.
- `--prices` and `--functions` files are read with the 268435456-byte limit and must be UTF-8.
- `gb_seconds` returns a `Result`; a product or quotient that does not fit in a decimal is `price overflow`, not a panic.
- A money value with more decimal digits than a 28-digit decimal holds is rejected instead of rounded.
- The text table measures column width in characters, not bytes, and trims only ASCII white space at the end of a row.
- `README.md` states the input rules (`--headroom +5` and `05` are read as 5, a repeated field after the fixed fields is ignored), that an amount which is not an exact decimal is an error, the system requirements, the memory use measured for large JSON files, that Lambda's `platform.report` records are not read, and the request-rate overflow case. `README.md` and `QUALITY.md` state the current test count and what standard output does when it is closed, read-only or full. `QUALITY.md` now records the commands, exit codes, per-target counts, what was not run, known limits, and which test covers each README claim.

## [1.0.0] - 2026-11-15

### Added

- CI workflow: format, clippy and tests (debug and release) on Linux, Windows and two macOS targets, a Rust 1.85 job, `cargo audit`, and `cargo deny --locked check licenses`.
- `deny.toml` with a permissive-only license list.
- `QUALITY.md` with the test count.

### Changed

- Version 1.0.0.
- Prices that overflow a decimal are rejected, and control characters in a path shown in an error or warning are replaced.
- A duplicate key in a log event object or in a price file is rejected where the scan used to ignore it.
- A date field with a character that is not a digit is rejected.
- A line separator or bidi override in a path is replaced.
- Every member of a multi-member gzip file is read, and trailing bytes that are not a member are an error.

This version is not published and has no git tag.

## [0.1.0] - 2026-11-09

### Added

- `lambda-bill` reads Lambda `REPORT` lines from local files and prices on-demand GB-seconds and requests for `x86_64` and `arm64`.
- Memory advice from Max Memory Used. The advice dollars assume the billed duration stays the same, and the report says that is not a forecast.
- Built-in first-tier rates from the AWS Lambda pricing page, read on 2026-11-09, and a `--prices` file for another quote.
- Text table and `--json`. Gzip is detected by magic bytes. The function name comes from the log group, a header, or an `aws/lambda/<name>` path.

This version is not published and has no git tag.
