# lambda-bill

[![crates.io](https://img.shields.io/crates/v/lambda-bill.svg)](https://crates.io/crates/lambda-bill) [![downloads](https://img.shields.io/crates/d/lambda-bill.svg)](https://crates.io/crates/lambda-bill) [![CI](https://github.com/qwpp7g79y4-maker/lambda-bill/actions/workflows/ci.yml/badge.svg)](https://github.com/qwpp7g79y4-maker/lambda-bill/actions/workflows/ci.yml) [![PEPAI checked](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fqwpp7g79y4-maker%2Flambda-bill%2Fmain%2F.github%2Fbadge.json)](QUALITY.md) [![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-blue.svg)](https://github.com/qwpp7g79y4-maker/lambda-bill/blob/main/Cargo.toml) [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/qwpp7g79y4-maker/lambda-bill#license)

Estimate AWS Lambda on-demand cost from CloudWatch `REPORT` lines, per function, for x86_64 and arm64, with a memory advice. For whoever has a Lambda log export and wants the list-price cost and a memory hint without an account or a network call.

The tests and the measured numbers are in `QUALITY.md`.

- It reads the files, directories and standard input you name. It writes the report to standard output, or to the file you give with `--output`. It changes nothing else.
- No network, no AWS SDK, no account, no token, no paid service.
- The rates are compiled in as a snapshot, or come from a `--prices` file you give.
- The result is an estimate from list prices, not your AWS invoice.

## Why

A Lambda log export already has the two numbers the on-demand invoice uses: billed duration and configured memory, once per request. lambda-bill adds those up on your machine, per function, for x86_64 and arm64, and looks at Max Memory Used to suggest a different memory setting. It does not call AWS and it does not need an account.

## Installation

With Rust 1.85 or newer, from a clone of this repository:

```sh
cargo install --locked --path .
lambda-bill --help
```

From crates.io: `cargo install lambda-bill`. No release archive exists yet: `.github/workflows/release.yml` builds archives with the third-party license texts when a version tag is pushed, and no tag has been pushed.

**Supported systems.** The suite was run on Linux x86_64 only. A release build there needs glibc 2.34 or newer; the workflow has a job that checks the highest glibc symbol of a release build on Ubuntu 22.04, and it has not run yet. The code is also compiled and checked with clippy for the Windows (`x86_64-pc-windows-gnu`), macOS (`x86_64-apple-darwin`) and musl (`x86_64-unknown-linux-musl`) targets, but no test ran on those systems. A standard output that is closed or read-only at start is detected on Linux only.

The suite has 273 tests (`cargo test --locked`), all passing. [QUALITY.md](QUALITY.md) has the table.

## Quick start

Two REPORT lines for one function. This writes them to a new temporary directory (the repository stays untouched) and prices them:

```sh
dir=$(mktemp -d)
cat > "$dir/report.log" <<'LOG'
# lambda-bill-function: checkout
# lambda-bill-arch: x86_64
REPORT RequestId: 11111111-1111-1111-1111-111111111111	Duration: 100.00 ms	Billed Duration: 100 ms	Memory Size: 512 MB	Max Memory Used: 40 MB	Init Duration: 200.00 ms	
REPORT RequestId: 22222222-2222-2222-2222-222222222222	Duration: 50.10 ms	Billed Duration: 51 ms	Memory Size: 512 MB	Max Memory Used: 64 MB	
LOG
lambda-bill "$dir/report.log"
```

The same bytes are in `examples/sample-report.log` of a clone, so `lambda-bill examples/sample-report.log` prints the same report. A test runs every console block in this file and compares it to this binary (with `--as-of 2026-11-20`, so the price age check does not depend on the clock):

```console
$ lambda-bill examples/sample-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
checkout  x86_64              512           64         2            1      0.0755  0.00000125833585    0.0000004  0.00000165833585  downsize to 128 MB
total                                                  2            1      0.0755  0.00000125833585    0.0000004  0.00000165833585

estimated_usd=0.00000165833585
advice checkout x86_64 512 MB: downsize to 128 MB; duration USD 0.00000125833585 -> 0.0000003145839625 if billed duration stays the same
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

512 MB for 151 billed milliseconds is 0.0755 GB-seconds. One of the two lines has `Init Duration`, so there is one cold start. The init time is not added again: it is already inside `Billed Duration`. Max Memory Used is 64 MB, so the advice is to set the memory to 128 MB. The dollar change on that line assumes the billed duration stays the same.

## Quickstart in 5 minutes

1. Install from a copy of this repository: `cargo install --locked --path .` (Rust 1.85 or newer).
2. Get a log export, for example with `aws logs filter-log-events` or through the CloudWatch console, and save it as a file or folder.
3. Run `lambda-bill export.log`. If the function name is not in the file, pass it with `--function name`. If the function is arm64, add `--arch arm64`.
4. Read the table: GB-seconds, amount per function, and the memory advice. The output is an estimate from list prices, not your AWS invoice.
5. To use the figures further, add `--json` or `--md`, or write to a file with `--output report.json`.

The full Dutch explanation is in [README.nl.md](README.nl.md).

## Options

```text
lambda-bill [OPTIONS] <FILE>...
```

`FILE` is a log file, a gzip export (recognized by its bytes, not its name), a directory (walked in sorted order), or `-` for standard input. Inputs are CloudWatch text `REPORT` lines, CloudWatch export JSON (a document or one event per line) and gzip of those. The shapes are listed under How it counts (C19, C23, C29).

| Option | Meaning | Default |
| --- | --- | --- |
| `--prices FILE` | Price file that replaces the built-in rates (C28) | built-in first-tier list |
| `--arch ARCH` | Architecture when the file does not say: `x86_64` or `arm64` | `x86_64` |
| `--function NAME` | Function name when the file or path does not say one | none |
| `--functions FILE` | JSON object of function name to architecture | none |
| `--headroom PERCENT` | Percent above the memory base for the advice, an integer from 0 to 100 | 20 |
| `--as-of DATE` | Date (`YYYY-MM-DD`) for the price age check | today (UTC) |
| `--quiet` | Print only the result and errors, no warning on standard error | off |
| `--verbose` | Also print progress on standard error | off |
| `--output FILE` | Write the report to this file instead of standard output | standard output |
| `--json` | JSON instead of the text table | off |
| `--md` | Markdown table instead of the text table (not with `--json`) | off |
| `-h`, `--help` | Print help | |
| `-V`, `--version` | Print the version (a debug build adds the commit) | |

`lambda-bill --help` prints the same list, and `man/lambda-bill.1` is the manual page.

## Example

An arm64 function has no architecture in the REPORT line. Pass it:

```console
$ lambda-bill --arch arm64 --function worker examples/arm-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
worker    arm64               128           20         1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675  keep
total                                                  1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675

estimated_usd=0.00000021666675
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

The same architecture can come from a map of function name to `x86_64` or `arm64`. The map does not name the function; `--function` still does, unless the file or the path already has a name.

```console
$ lambda-bill --function worker --functions examples/functions.json examples/arm-report.log
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds      duration_usd  request_usd         total_usd  advice
worker    arm64               128           20         1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675  keep
total                                                  1            0     0.00125  0.00000001666675    0.0000002  0.00000021666675

estimated_usd=0.00000021666675
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

A CloudWatch export directory names the function with the path `aws/lambda/<name>`. `examples/export` has `billing` and `checkout`:

```console
$ lambda-bill examples/export
prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
provisioned concurrency not included: only on-demand duration and requests are priced
Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)
headroom: 20%

function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds       duration_usd  request_usd          total_usd  advice
billing   x86_64              256           30         1            0     0.00125  0.000000020833375    0.0000002  0.000000220833375  downsize to 128 MB
checkout  x86_64              256           30         1            0     0.00125  0.000000020833375    0.0000002  0.000000220833375  downsize to 128 MB
total                                                  2            0      0.0025   0.00000004166675    0.0000004   0.00000044166675

estimated_usd=0.00000044166675
advice billing x86_64 256 MB: downsize to 128 MB; duration USD 0.000000020833375 -> 0.0000000104166875 if billed duration stays the same
advice checkout x86_64 256 MB: downsize to 128 MB; duration USD 0.000000020833375 -> 0.0000000104166875 if billed duration stays the same
note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

`--json` prints the same totals (and `schema_version`, the first key), with money as strings. The source URL and the date are in the document:

```console
$ lambda-bill --json --arch arm64 --function worker examples/arm-report.log
{
  "schema_version": 1,
  "version": "1.0.0",
  "currency": "USD",
  "source": "https://aws.amazon.com/lambda/pricing/",
  "date": "2026-11-09",
  "region": "us-east-1",
  "tier": "first",
  "estimate": true,
  "estimate_note": "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)",
  "free_tier_applied": false,
  "provisioned_concurrency_included": false,
  "headroom_percent": 20,
  "advice_note": "advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.",
  "warnings": [],
  "functions": [
    {
      "function": "worker",
      "architecture": "arm64",
      "memory_mb": 128,
      "max_memory_used_mb": 20,
      "requests": 1,
      "cold_starts": 0,
      "gb_seconds": "0.00125",
      "duration_usd": "0.00000001666675",
      "request_usd": "0.0000002",
      "total_usd": "0.00000021666675",
      "advice": "keep",
      "advice_mb": 128,
      "duration_usd_if_duration_stays": "0.00000001666675"
    }
  ],
  "totals": {
    "requests": 1,
    "cold_starts": 0,
    "gb_seconds": "0.00125",
    "duration_usd": "0.00000001666675",
    "request_usd": "0.0000002",
    "total_usd": "0.00000021666675",
    "estimated_usd": "0.00000021666675"
  }
}
```

`--md` prints the same amounts as a Markdown table. A `|`, a backslash or a line break inside a cell is escaped, and the estimate line is there too. `--md` and `--json` cannot be used together.

```console
$ lambda-bill --md --arch arm64 --function worker examples/arm-report.log
# Lambda cost estimate

Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)

- prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first
- free tier not applied: the account free tier is 1000000 requests and 400000 GB-seconds per month, shared across architectures, and this export is not the whole account
- provisioned concurrency not included: only on-demand duration and requests are priced
- headroom: 20%

### Cost

| function | architecture | memory_mb | max_used_mb | requests | cold_starts | gb_seconds | duration_usd | request_usd | total_usd | advice |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| worker | arm64 | 128 | 20 | 1 | 0 | 0.00125 | 0.00000001666675 | 0.0000002 | 0.00000021666675 | keep |
| total |  |  |  | 1 | 0 | 0.00125 | 0.00000001666675 | 0.0000002 | 0.00000021666675 |  |

estimated_usd=0.00000021666675

note: advice assumes billed duration stays the same. Lambda gives CPU in proportion to memory, so a smaller setting can run longer. This is not a forecast.
```

## How it counts

C1. One `REPORT` line is one request. `START`, `END`, and other lines are not requests.

C2. GB-seconds are `memory_mb * billed_ms / 1024 / 1000`. 512 MB for 151 ms is 0.0755 GB-seconds.

C3. The Duration field is not used for the cost. Only Billed Duration is.

C4. Init Duration is not added to Billed Duration. It marks a cold start only when it is the next field after Max Memory Used. The init time is already inside Billed Duration.

C5. Requests are 0.20 USD per million on x86_64 and on arm64.

C6. x86_64 duration is 0.0000166667 USD per GB-second. The pricing-page shape, 3 million requests of 120 ms at 1536 MB, is 540000 GB-seconds and 9.000018 USD of duration, plus 0.60 USD of requests.

C7. arm64 duration is 0.0000133334 USD per GB-second. That is not exactly 80 percent of the x86_64 rate: 10000 GB-seconds cost 0.166667 USD on x86_64 and 0.133334 USD on arm64, not 0.1333336.

C8. Every report names the currency, the source URL, the date, the region label, and the tier. The built-in quote is USD, `https://aws.amazon.com/lambda/pricing/`, 2026-11-09, `us-east-1`, `first`.

C9. The account free tier is not subtracted. It is 1000000 requests and 400000 GB-seconds per month, shared across architectures. At these rates that quantity is 6.86668 USD. This export is not the whole account, so a small report still costs more than 0.00 USD.

C10. Provisioned Concurrency is not priced. Every report says it is not included. The tool does not apply a Provisioned Concurrency duration rate or request rate.

C11. A downsize target is `ceil(max used * (100 + headroom) / 100)`, then at least 128 MB and at most 10240 MB. It is used only when that target is below the configured size. Headroom 20 on 200 MB used at 256 MB suggests 240 MB. Headroom 0 on 400 MB used at 512 MB suggests 400 MB. 40 MB used at 512 MB suggests 128 MB.

C12. At or above 90 percent of the configured size, the raise target is `ceil(configured * (100 + headroom) / 100)`, at least one megabyte above the current size, and at most 10240 MB. Max Memory Used is not the base, because it cannot show a peak above the setting. 900 of 1000 raises to 1200. 899 of 1000 stays. 231 of 256 raises to 308. 230 of 256 stays. 200 used on a 128 MB function raises to 154. 10240 stays when it cannot grow. When both this rule and a downsize could apply, raise wins: the downsize rule is not used at or above 90 percent. 117 MB used of 129 MB with headroom 5 raises to 136, not a downsize to 128.

C13. Advice dollars assume billed duration stays the same. The note says this is not a forecast. A keep row has no advice detail line.

C14. 1024 MB for 1000 ms is 1 GB-second. 1000 MB for 1000 ms is not. The pricing page uses 1 GB = 1024 MB.

C15. One row is one function, one architecture, and one configured memory size. Rows are ordered by function, then arm64 before x86_64, then memory. Reports that share that key are summed, and Max Memory Used is the highest of them.

C16. A RequestId is counted once per function. The first copy wins, in the order of the paths you pass. A directory is walked in sorted path order. The warning says how many duplicates were skipped and does not print the id. The same id on two functions counts twice.

C17. The tool has no AWS SDK and opens no network connection. It only reads the files you pass. The built-in rates are compiled in from `prices/lambda-ondemand.json`.

C18. The default architecture is x86_64. That rate is higher than arm64, so an unknown architecture is not underpriced. A `--functions` entry wins over that default and must agree with an architecture written in the file. Only the names `x86_64` and `arm64` are accepted.

C19. The function name comes from the log group (`/aws/lambda/<name>` or `aws/lambda/<name>`), then a header (`# lambda-bill-function:` or `# function:`), then an `aws/lambda/<name>` path, then `--function`. Two different detected names are an error. `--function` does not override a detected name. A missing name is an error. A name is 1 to 64 characters from `A-Z`, `a-z`, `0-9`, `_`, and `-`.

C20. Configured memory must be from 128 through 10240 MB. Max Memory Used may be from 0 through 10240, including a value above the configured size. A billed duration of 0 is accepted. The number fields are limited by their text length, not by their value: Billed Duration may have up to 20 characters (at most 18446744073709551615, so `00000000000000000100` is 100 and `000000000000000000100` is rejected), Memory Size and Max Memory Used up to 10 characters (`0000000128` is 128). The Billed Duration of the requests of one row is added in 64 bits: a sum above 18446744073709551615 is the error `billed duration overflow`.

C21. Gzip is recognized by the bytes `1f 8b`, not by the file name. A plain-text file named `.gz` is read as text. Every member of a multi-member gzip is read. Bytes after a member that are not a valid next member are an error. Corrupt gzip is an error.

C22. Request ids are not printed.

C23. A line that contains `REPORT RequestId:` and does not parse is an error, and the line text is not printed. A line that starts with `{` and is not JSON is an error too, so it cannot lower the bill unseen. A JSON line that contains a REPORT but yields no request is an error. A JSON object with both `message` and `events` is an error, also when it sits inside a document or inside another event. In a JSON document or JSON line, any `REPORT RequestId:` text that is not read as a request is an error too, for example a second REPORT in `@message` next to `message` (`document: not a Lambda REPORT line`). The strings and the keys are searched after the JSON escapes are decoded, so `REPORT\u0020RequestId:` is found as well.

C24. The total is the exact sum of the row amounts. An amount or a sum that is not an exact decimal is an error. It is not scaled to a day or a month. The report does not say "per day" and it does not print a `factor:`. The words "per month" appear only in the free-tier sentence.

C25. JSON starts with `schema_version` (the integer 1, the version of the layout, not of the program), then includes the version, the currency, the source URL, and the date, in that order. Money and GB-seconds are strings. `estimate` is true, with the sentence in `estimate_note`, and `totals.estimated_usd` repeats `totals.total_usd`. `free_tier_applied` is false. The document ends with a newline.

C26. A REPORT line needs `REPORT RequestId:` at a whitespace boundary, then Duration, Billed Duration, Memory Size, and Max Memory Used, in that order. A second marker on the same line is an error. `XREPORT` does not match: on a plain text line it is ignored. A marker glued to a byte-order mark, a zero-width or other invisible character, or a non-ASCII space is an error, not a silent skip. Inside a JSON or bracket line, any `REPORT RequestId:` text that is not counted as a request is an error, `XREPORT` included. Trailing `XRAY` text is ignored. A field repeated after the four fixed fields is trailing text as well: the first `Billed Duration` counts and a second one is not read.

C27. The units must be `ms` and `MB`. Billed Duration is an integer. Duration and Init Duration may be fractional.

C28. A `--prices` file replaces the built-in rates. Unknown fields are ignored. A money string may be padded only with space, tab, CR, or LF. Other Unicode space is rejected. A money value with more decimal digits than a 28-digit decimal holds is rejected. The source text and the labels may not contain control characters, line or paragraph separators, or bidi marks. A duplicate top-level key is rejected. A nested duplicate key is allowed. A negative number and a number written with an exponent are rejected. The file records its own source URL and date.

C29. A timestamp before `REPORT` is ignored. A bracketed timestamp, a BOM at the very start of the file, and CRLF are ignored too. A line that starts with any other invisible character (a BOM in the middle of a file, a zero-width space, a bidirectional mark) is an error, also in front of a header or a JSON line. A JSON log line is read through its `message`, `@message`, or `Message` text. A file that is one JSON object with `@message` or `Message` is read too, also when it is printed over several lines. Such an object names its function and architecture with `logGroup`, `logGroupName` and `architecture`, the same as an event with `message` does, and a bad value is an error. A `logGroup` and a `logGroupName` that name different groups are an error (`logGroup and logGroupName differ`).

C30. An error for a price file or a log line names the problem and does not include the file contents. A control character, line or paragraph separator, or bidi mark in a path becomes U+FFFD. In an argument that clap rejects it is written as `\u{..}`, so a file name or an argument cannot rewrite the terminal line. The line breaks that clap writes itself stay: an unknown option or a missing file argument prints several lines, ending with a line break. The same holds for an argument that is not valid UTF-8, which clap reports without repeating it.

## Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | The report was written. Warnings go to standard error and do not change the code. |
| 2 | An input or usage error: a bad file, line, price file, date or option, an amount that does not fit in an exact decimal (`price overflow`), a run with more than 100000 function and memory rows (`more than 100000 function and memory rows`), or a standard output that is closed, read-only or cannot be written. Nothing is printed on standard output. |
| 3 | `--quiet` together with `--verbose`. |
| 130 | SIGINT (Ctrl-C). Nothing is printed on standard output. |

A panic never replaces these codes: a closed or full standard output is exit 2, not a crash. The Limitations section has the details.

## Limitations

Only Lambda on-demand duration and requests are priced. Provisioned Concurrency, Managed Instances, MicroVMs, Web Functions, ephemeral storage, tenant isolation, durable-function operations, HTTP response streaming, data transfer, and SnapStart beyond what is already inside Billed Duration are not priced. A REPORT line does not say which of those applied. The tool always uses the on-demand rates and says that Provisioned Concurrency is not included.

The free tier is not subtracted. One export is not the account's month. There is no per day or per month extrapolation of the rows you passed.

The default architecture is x86_64 when the file does not say. That overstates the cost of an arm64 function, which is the safe direction. Pass `--arch` or `--functions` when you know.

Advice is not a forecast. A smaller memory setting also means less CPU, so the function can run longer than the dollars in the advice line.

One REPORT line is one request. The tool does not split or join async payload chunks.

A file larger than 268435456 bytes is rejected, and so is a `--prices` or `--functions` file. Those two must be UTF-8 and must be regular files: a pipe or a device, such as `<(command)` or a FIFO, is `not a file`, so a path that never ends cannot hang the run. A line longer than 1048576 bytes is rejected. A JSON document over 4000000 values is rejected too (see below). The directory you name is level 1. Level 64 is read. Level 65 is an error (`directory is nested too deeply`). A symlink you name is followed. A symlink found while walking a directory is skipped, and so is a member that is not UTF-8 or that has no REPORT line, with a warning. The same cases on an explicit path are errors. Corrupt gzip is always an error.

Request ids and the text of a rejected line are not printed.

Text in front of a JSON line makes the line text, and then the marker inside it is glued to a quote and does not count (C26). A file with `{"message":"REPORT RequestId: a1 ..."}` on line 1 and `2026-11-19T10:00:00Z {"message":"REPORT RequestId: a2 ..."}` on line 2 gives one request and exit 0. The same holds for a byte-order mark, a comma or `data: ` in front of the `{`. Strip such a prefix before you pass the file. A timestamp in front of a plain `REPORT` line is fine (C29).

A `# function:` header applies to the whole file, also to the REPORT lines above it. Two different headers are an error.

A RequestId that appears twice for one function counts once and the first copy wins, even when the second copy has other memory or duration values. The warning only counts the duplicates.

A file of 268435456 empty lines is read in about 3 s and 526208 KB (measured with a release build): the file is held twice in memory, once as bytes and once as text.

Lambda's JSON log format is not read. A record such as `{"type":"platform.report","record":{...}}` has no `message`, so it is skipped without a message and its request is not counted. Use the `REPORT` text line. A pretty-printed JSON object that is not a CloudWatch event and has no `@message` or `Message` is rejected as `line 1: not JSON`. A text log with a line such as `{'a': 1}` is rejected as a whole, on purpose: a line that opens an object and is not JSON must not drop a request unseen.

Every file you pass is read into memory. All input files of one run together (files, directories and standard input) may not exceed 268435456 bytes, counted before each file is read (`input is larger than 268435456 bytes in total`, exit 2), and one gzip file may have at most 1000000 members (`more than 1000000 gzip members`, exit 2). A run reports at most 100000 rows, one per function, architecture and memory size; more is `more than 100000 function and memory rows`, exit 2, also with `--json`. The `--prices` and `--functions` files are not part of that total.

A JSON document, a `--prices` file and a `--functions` file may hold at most 4000000 JSON values: a scalar and an object key count 1, an array or an object counts 4. The values are counted before the document is built, also for the text inside a gzip file, and a document over the budget is `more than 4000000 JSON values`, exit 2. The text size says little about the memory of a JSON document (a scalar needs about 100 bytes, a small array about 300), so the budget is what bounds it. Measured with a release build on Linux x86_64, peak memory of documents at the budget that were then rejected as not log events: 3990000 scalars `[0,0,...]` (7999981 bytes) 424108 KB in 0.6 s; 1998999 object keys (24875884 bytes) 623372 KB in 2.3 s; 280000 CloudWatch events with four fields each (19040001 bytes) 356556 KB in 0.7 s. A document at the budget needs well under 1 GB and a few seconds (it was 20000000 values, up to about 3 GB and 17 s). Over the budget, a `[0,0,...]` of 100000001 bytes, a `[[0],...]` of 100000001 bytes and a gzip file of 657179 bytes that holds 170 MB of nested arrays are refused in under 1 s and under 200 MB (measured at the old budget; the counting pass stops earlier now). A CloudWatch export with four fields per event reaches the budget at about 330000 events. A JSON line is limited by the line limit of 1048576 bytes, not by this budget.

Other measured costs inside the file limit (release build): a gzip file of 13000000 empty members took 39.3 s and 255808 KB before the member limit (now refused); a file with 100000 events that each name another function took 1.0 s and 119244 KB (measured before the row limit existed, at the limit; a file with more functions is now refused, see above). Each row (function, architecture and memory size) costs about 10 microseconds, and a file of 10113 memory sizes of one function (1465123 bytes) takes 0.14 s. All of these grow in a straight line with the input.

A GB-second, a duration amount, a request amount, an advice amount and a total are stored only when they are already an exact 96-bit decimal: a coefficient of at most `79228162514264337593543950335` and at most 28 digits after the decimal point. Multiplication and division keep every digit. A value that is not already that decimal is `price overflow`, exit 2. Nothing is rounded. Two requests at `79228162514264337593543950335` USD per million fit: the amount is `158456325028528675187087.90067`. One request is `79228162514264337593543.950335`. One GB-second at `10^-28` USD is `0.0000000000000000000000000001`. 10240 MB for 18446744073709551615 ms at `1000000000000` USD per GB-second does not fit.

These do not fit. Each exits 2 and prints nothing on standard output. 1000 MB for 1 ms at `1000000000000000000000001` USD per GB-second is exactly `976562500000000000000.0009765625`. 128 MB for 1000 ms at `7922816251426433759354395033` is exactly `990352031428304219919299379.125`. 512 MB for 1000 ms at `79228162514264337593543950333` is exactly `39614081257132168796771975166.5`. 1023 MB for 18446744073709551615 ms is not an exact decimal. One row of `39614081257132168796771975168` plus one row of `0.50` is not an exact decimal. At `1237940039285380274899124224.5` USD per GB-second the row of 1024 MB for 1000 ms fits, and the advice amount for a raise from 1024 MB to 1229 MB does not. The same rate kept at 1024 MB fits. The table shows the stored decimal and does not round it again. Lambda's longest invocation is 900000 ms.

`--headroom` takes an integer from 0 to 100. `+5` and `05` are read as 5.

`--quiet` prints only the result and errors: the warnings on standard error (the JSON warnings and the price age warning) are not printed, and the report on standard output is the same. `--verbose` adds progress on standard error: one `lambda-bill: reading <file>` line per argument and a closing `lambda-bill: done: <n> functions, <n> requests` line. `--quiet` together with `--verbose` is a usage error with exit code 3, and prints nothing else.

`--output FILE` writes the report (text, `--json` or `--md`) to the file instead of standard output, and prints nothing on standard output. The report goes to a temporary file in the same directory and is then renamed over the target, so a failed or interrupted run leaves an existing file as it was and no half-written report. An existing regular file is replaced (`--verbose` says so with a `replacing` line). A symlink, a directory or a device at that path is refused and never followed. A missing directory is not created. Any write failure exits 2 with one `cannot write` line.

SIGINT (Ctrl-C) exits with code 130 and prints `lambda-bill: interrupted by SIGINT` on standard error. Nothing is printed on standard output, so an empty output is never a success. A SIGINT that arrives while the report is being written lets that write finish and then exits 130. The handler is installed on Unix only; on Windows the default stays.

Price age: when the date of the price file (the built-in one or `--prices`) is more than 180 days before `--as-of DATE` (`YYYY-MM-DD`; today in UTC when it is not given), a `warning:` line with that date goes to standard error. The exit code stays 0. A bad `--as-of` date exits 2.

A sum that does not fit in a decimal is the error `price overflow`, not a panic. If standard output was closed when the program started (`>&-`), is open for reading only (`1<file`), or the write fails (for example `/dev/full` or a closed pipe), the exit code is 2 and standard error says so. This holds for `--help` and `--version` too. `>/dev/null` and `1<>/dev/null` are open and exit 0. Any other input or usage error exits 2.

The built-in rates are the first-tier US East (N. Virginia) figures on the AWS Lambda pricing page, read on 2026-11-09. Other regions and tiers are not built in. Put them in a `--prices` file if you have a quote. The program has no AWS SDK and opens no network connection.

## Prices and responsibility

The built-in prices are a snapshot: the first-tier US East (N. Virginia) on-demand list of AWS Lambda, source `https://aws.amazon.com/lambda/pricing/`, dated 2026-11-09 (`prices/lambda-ondemand.json`). Every report prints that source and date, and a warning goes to standard error when the date is more than 180 days before `--as-of` (or today).

Check the current prices yourself before you rely on a number. You are responsible for any decision you take on the output of this tool; it is an estimate from list prices, not your invoice. Use `--prices FILE` to give your own price file.

## Maintenance

Report bugs as a GitHub Issue on this repository. After a release the tool is in dormant mode: a new version appears only for a reported bug, a request or a security problem, and security always comes first.

## Support

Bugs and questions: GitHub Issues of this repository. A security problem goes through the private route in [SECURITY.md](SECURITY.md). How to build and test: [CONTRIBUTING.md](CONTRIBUTING.md). The checks that were run are in [QUALITY.md](QUALITY.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

The licenses of the third-party crates built into the binary are listed in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).

## Contact

- E-mail: pep_ai@icloud.com
- Web: https://www.pepai.nl
- X: https://x.com/pepai_nl
- Ko-fi: https://ko-fi.com/pepai
