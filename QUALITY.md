# Quality

The measurements below apply to this commit.

Suite: 273 tests: 273 passed, 0 failed, 0 ignored.

Repair of 2026-11-23, finding B1 of the P review of 73df4b1: `looks_like_json_array` in `src/parse.rs` skipped only a space and a tab after the opening bracket, so a cut array with a CR after the bracket (`[`, CR, `{"message":...,"logGroup":"/aws/lambda/other","architecture":"arm64"}`, no closing bracket) was read as text, counted, and billed to `--function f` on x86_64 with exit 0. The code commit 282f8e2 skips a space, tab, CR and LF. A plain bracket line (`[INFO]`, `[ERROR]`, `[WARNING]`, `[main]`, `[YYYY-MM-DD`) stays text. Same files before (tip 73df4b1) and after (282f8e2), release build, `lambda-bill --function f <file>`, output filtered to the error line, the table rows and the exit code. `base.json` is the cut array without the CR, `c.json`, `spcr.json` and `tabcr.json` have CR, space+CR and tab+CR after the bracket, `valid.json` is the closed array with CR, `text.log` has `[INFO]`, `[main]` and `[2024-01-01]` lines around a REPORT.

Before, on 73df4b1:

```
== base.json
lambda-bill: base.json: line 1: not JSON
exit 2
== c.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== spcr.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== tabcr.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
advice f x86_64 512 MB: downsize to 128 MB; duration USD 0.000000833335 -> 0.00000020833375 if billed duration stays the same
exit 0
== valid.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds   duration_usd  request_usd      total_usd  advice
other     arm64               512           40         1            0        0.05  0.00000066667    0.0000002  0.00000086667  downsize to 128 MB
total                                                  1            0        0.05  0.00000066667    0.0000002  0.00000086667
advice other arm64 512 MB: downsize to 128 MB; duration USD 0.00000066667 -> 0.0000001666675 if billed duration stays the same
exit 0
== text.log
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
exit 0
```

After, on 282f8e2:

```
== base.json
lambda-bill: base.json: line 1: not JSON
exit 2
== c.json
lambda-bill: c.json: line 1: not JSON
exit 2
== spcr.json
lambda-bill: spcr.json: line 1: not JSON
exit 2
== tabcr.json
lambda-bill: tabcr.json: line 1: not JSON
exit 2
== valid.json
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds   duration_usd  request_usd      total_usd  advice
other     arm64               512           40         1            0        0.05  0.00000066667    0.0000002  0.00000086667  downsize to 128 MB
total                                                  1            0        0.05  0.00000066667    0.0000002  0.00000086667
advice other arm64 512 MB: downsize to 128 MB; duration USD 0.00000066667 -> 0.0000001666675 if billed duration stays the same
exit 0
== text.log
function  architecture  memory_mb  max_used_mb  requests  cold_starts  gb_seconds    duration_usd  request_usd       total_usd  advice
total                                                  1            0        0.05  0.000000833335    0.0000002  0.000001033335
exit 0
```

Wanted: `c.json`, `spcr.json` and `tabcr.json` exit 2 with `not JSON` and count nothing (they were exit 0, 1 request, function f, x86_64); `base.json` stays exit 2; `valid.json` and `text.log` stay exit 0 as before. Suite on 282f8e2: 273 tests, 273 passed. The existing test `a_bracket_prefix_that_is_not_json_stays_text_and_a_cut_array_is_not_json` now also has seven cut arrays with CR; with the old `src/parse.rs` it failed (`assertion left == right failed ... left: 0, right: 2`), with the fix it passes. Clippy `-D warnings` and `cargo fmt --check` are clean. Still open: the same review lists other first characters after the bracket (`\f`, `\v`, no-break space, zero-width space, NUL, BOM) that this repair does not touch; the limit `[1,2] x` stays as before. This is a repair, not a final verdict.

Repair of 2026-11-23, finding of the review of 0a1ebb8: since 080dab6 every line that starts with `[` and is not JSON and not `[YYYY-MM-DD` was `not JSON` (exit 2), so a plain `[INFO]` line stopped the whole directory, while the CHANGELOG says such a line is text. The code commit e710e44 reads a `[` line that is not JSON as text unless it opens like a JSON array (`{`, `[`, `]`, `"`, `true`/`false`/`null`, a number-only run, or nothing after the bracket). The same four files, `lambda-bill --json <file>`, output filtered to the error line and the `"requests"` line, release build, literal output.

Before, on 0a1ebb8:

```
== info.log
   | # function: f
   | [INFO] start
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [ERROR] x
   | [WARNING] y
   | [main] z
lambda-bill: info.log: line 2: not JSON
exit 2
== cut.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
lambda-bill: cut.log: line 3: not JSON
exit 2
== nobracket.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"x"}
lambda-bill: nobracket.log: line 3: not JSON
exit 2
== date.log
   | # function: f
   | [2024-01-01] REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
    "requests": 1,
exit 0
```

After, on e710e44:

```
== info.log
   | # function: f
   | [INFO] start
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [ERROR] x
   | [WARNING] y
   | [main] z
    "requests": 1,
exit 0
== cut.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
lambda-bill: cut.log: line 3: not JSON
exit 2
== nobracket.log
   | # function: f
   | REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 64 MB
   | [{"message":"x"}
lambda-bill: nobracket.log: line 3: not JSON
exit 2
== date.log
   | # function: f
   | [2024-01-01] REPORT RequestId: a1 Duration: 100.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 
    "requests": 1,
exit 0
```

Wanted: `info.log` text and exit 0 (it was exit 2), `cut.log` and `nobracket.log` stay exit 2 and count nothing, `date.log` stays text and exit 0. Suite on e710e44: 273 tests, 273 passed; the new test is `a_bracket_prefix_that_is_not_json_stays_text_and_a_cut_array_is_not_json`, and `a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error` now expects `not a Lambda REPORT line` for `[ts] x<marker>` (still exit 2, empty standard output). Not repaired, still open: many small gzip files, and `--md` with file names. Limit: a text line such as `[1,2] x` or `[true] x` opens like an array and is still `not JSON`.

This commit changes documentation only. Every number below says on which commit it was measured; a row that did not run again says so.

Record of 2026-11-23, code commit e35a324 (`src/`, `tests/` and the manifest are the same as on 40aa20a and fe0a063; the commits after e35a324 change documents only). Linux x86_64, rustc 1.99.0 and cargo 1.99.0 (stable). Measured on e35a324 for this record: debug `cargo test --locked --offline`: 271 passed, 0 failed, 0 ignored, of which `cargo test --locked --offline --lib` printed 66 passed; `cargo fmt --check` and `cargo clippy --locked --offline --all-targets -- -D warnings`: exit 0. Measured on fe0a063 by the final review: `cargo fmt --check`, clippy, the debug and the release test run (271 passed, 0 failed each), `cargo package` and `cargo deny`; `cargo +1.85` only compiled the tests (`--no-run`), the 1.85 tests: not run. Every other row below is the record of 2026-11-20 on commit 2d51303 (269 tests then) and did not run again. CI for this commit: not run. Every check that was skipped is listed once, under Not run below. No tag, no publish, no push.

Red for green, 2026-11-23, finding B-1 of the final review: a line that starts with `[`, is not a `[YYYY-MM-DD` bracket timestamp and is not JSON went to the text parser, so a cut array lost its `logGroup` and `architecture` and was billed to `--function` on x86_64 with exit 0. The new test `a_broken_json_array_line_is_not_json_and_a_valid_one_keeps_its_envelope` first ran against the code of 61a50a1 and failed: `thread 'a_broken_json_array_line_is_not_json_and_a_valid_one_keeps_its_envelope' panicked at tests/cli.rs:2357:9: assertion `left == right` failed: cut`, `left: 0`, `right: 2`, `test result: FAILED. 0 passed; 1 failed`. With the fix in `src/parse.rs` (commit 080dab6) it passes, and the test `a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json` stays green. Debug `cargo test --locked --offline --no-fail-fast` on 080dab6: 272 tests, 271 passed, 1 failed: `the_documents_state_the_real_number_of_tests`, because README.md and README.nl.md still say 271; the test rows of the tables below are the numbers of e35a324. Two existing tests (`a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error`, `uncounted_report_marker_in_json_or_bracket_line_exits_2`) now expect `not JSON` for a bracket line that is no timestamp; the exit code is 2 and standard output is empty, as before.

Red for green, 2026-11-23, on a throwaway copy outside this repository: `MAX_JSON_UNITS` in `src/budget.rs` was changed from 4_000_000 to 400_000_000. `a_json_document_with_too_many_values_is_refused_before_it_is_built` then failed (`tests/cli.rs:2260:5`, `lambda-bill: /big.json: event 1: not a log event`, `test result: FAILED. 0 passed; 1 failed`). With the change undone it passed again. The copy is not in the repository.

Tests do not depend on the system clock: each test that runs the binary passes a fixed `--as-of 2026-11-20` (or its own date). Proof, run before the fix: in a copy of the tree, `src/money.rs` (`today_utc`) was changed to run the clock 400 days ahead, and `cargo test --no-fail-fast` then failed at least 27 tests in `tests/cli.rs` (`assertion failed: stderr.is_empty()` and others). With the fix, the same copy passes all tests (the 258 at the time).

## Commands

The column Measured on of this table says on which commit each row last ran: e35a324 (run for this record, with `-j 2` and `--test-threads 2`), fe0a063 (the final review) or 2d51303 (the record of 2026-11-20, not repeated; see Not run).

| Command | Exit | Measured on |
| --- | ---: | --- |
| `cargo fmt --check` | 0 | e35a324 and fe0a063 (final review) |
| `cargo clippy --locked --offline --all-targets -- -D warnings` (stable) | 0 | e35a324 and fe0a063 (final review) |
| `cargo +1.85 clippy --locked --offline --all-targets -- -D warnings` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-pc-windows-gnu` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-apple-darwin` | 0 | 2d51303 |
| `cargo check --locked --offline --tests --target x86_64-unknown-linux-musl` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-apple-darwin -- -D warnings` | 0 | 2d51303 |
| `cargo clippy --locked --offline --all-targets --target x86_64-unknown-linux-musl -- -D warnings` | 0 | 2d51303 |
| `cargo test --locked --offline` (debug) | 0 | e35a324 and fe0a063 (final review) |
| `cargo test --locked --offline --lib` (66 passed) | 0 | e35a324 |
| `cargo test --locked --offline --release` | 0 | fe0a063 (final review) |
| `cargo +1.85 test --locked --offline` (debug) | 0 | 2d51303 (on fe0a063 the tests were only compiled) |
| `cargo +1.85 test --locked --offline --no-run` | 0 | fe0a063 (final review) |
| `cargo package --locked --offline --allow-dirty --no-verify --list` | 0 | fe0a063 (final review) |
| `cargo package --locked --offline --allow-dirty --no-verify`, then `cargo test --offline` in the unpacked crate | 0 | 2d51303 |
| `bash scripts/check-glibc.sh target/release/lambda-bill 2.34` | 0 | 2d51303 |
| `cargo deny --locked check licenses` | 0 | fe0a063 (final review) |
| `cargo audit --no-fetch` | 0 | 2d51303 |

The Windows, macOS and musl rows compile the code and the tests and run clippy. Nothing was run on those targets. `cargo deny` printed `licenses ok`. `cargo audit` read 1527 advisories (database of the machine, not refreshed) for 76 crates and reported none. `check-glibc.sh` printed `highest GLIBC symbol: 2.34 (allowed: 2.34)` for the release binary of this machine. On 2d51303 the unpacked crate gave the same test counts as that commit, in a directory without `.github/` and `scripts/`.

`cargo package --list` has 50 lines: `.cargo_vcs_info.json`, `.gitignore`, `CHANGELOG.md`, `CONTRIBUTING.md`, `Cargo.lock`, `Cargo.toml`, `Cargo.toml.orig`, both licenses, `QUALITY.md`, `QUALITY.nl.md`, `README.md`, `README.nl.md`, `SECURITY.md`, `about.hbs`, `about.toml`, `build.rs`, `deny.toml`, `examples/*` (ten files: the README, the logs, the functions map and four expected outputs, with the two export members), `man/lambda-bill.1`, `prices/lambda-ondemand.json`, `src/*.rs` (thirteen files) and `tests/*.rs` (seven files). `.github/`, `.gitattributes` and `scripts/` are excluded (the manifest names them in `exclude`).

## Test result per target

Debug (`cargo test --locked --offline`) on e35a324 gave the counts below; the lib row is what `cargo test --locked --offline --lib` printed (66). The release column is the count of the final review on fe0a063 (`--release`, same sources), not repeated for this record. The tests of `cargo +1.85`: not run on this tree.

| Target | Debug passed | Debug failed | Debug ignored | Release passed | Release failed | Release ignored |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| unittests `src/lib.rs` | 66 | 0 | 0 | 66 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `tests/claims.rs` | 39 | 0 | 0 | 39 | 0 | 0 |
| `tests/cli.rs` | 139 | 0 | 0 | 139 | 0 | 0 |
| `tests/documented_shapes.rs` | 9 | 0 | 0 | 9 | 0 | 0 |
| `tests/docs_clean.rs` | 3 | 0 | 0 | 3 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 | 1 | 0 | 0 |
| `tests/quality.rs` | 4 | 0 | 0 | 4 | 0 | 0 |
| `tests/readme.rs` | 11 | 0 | 0 | 11 | 0 | 0 |
| doc-tests | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 273 | 0 | 0 | 273 | 0 | 0 |

## Standard output closed, read-only or full

`src/stdout_probe.rs` recognizes a standard output that was closed, or open for reading only, when the program started. Rust gives a closed file descriptor 1 back as `/dev/null` (read and write) before `main`, and its standard output ignores the error of a write to a descriptor that is not open for writing. So afterwards `>&-` looks like `1<>/dev/null` and `1<file` looks like a working output. On Linux, a constructor in `.init_array` runs earlier and records the state of descriptor 1. `src/main.rs` then refuses to write and exits 2. On other systems the probe always says "writable". Its `unsafe` code is two read-only `fcntl(1, ...)` calls (`F_GETFD`, `F_GETFL`). The only other `unsafe` code is the SIGINT handler in `src/interrupt.rs` (see SECURITY.md).

Measured on 2026-11-19 with `examples/sample-report.log`. The debug and the release binary gave the same exit code and message in every row.

| Command | Exit | Standard error |
| --- | ---: | --- |
| `lambda-bill examples/sample-report.log >&-` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log >/dev/full` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log >/dev/null` | 0 | none |
| `bash -c 'lambda-bill examples/sample-report.log 1<>/dev/null'` | 0 | none |
| `lambda-bill examples/sample-report.log 1<examples/sample-report.log` | 2 | `lambda-bill: could not write output` |
| `lambda-bill examples/sample-report.log 1<.` | 2 | `lambda-bill: could not write output` |
| `lambda-bill --json examples/sample-report.log >&-` | 2 | `lambda-bill: could not write output` |
| `python3 -I -c '... subprocess.run([...], stdout=subprocess.DEVNULL)'` | 0 | none |
| `lambda-bill examples/sample-report.log \| true` (10 runs) | 2 each | `lambda-bill: could not write output` |
| `lambda-bill --help >&-` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --version >&-` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help >/dev/full` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --version >/dev/full` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help 1<examples/sample-report.log` | 2 | `lambda-bill: stdout is not writable` |
| `lambda-bill --help >/dev/null` | 0 | none |
| `lambda-bill --version 1<>/dev/null` | 0 | none |
| `lambda-bill --help \| true` (10 runs) | 2 each | `lambda-bill: stdout is not writable` |
| `lambda-bill --version \| true` (10 runs) | 2 each | `lambda-bill: stdout is not writable` |

`python3` with `stdout=subprocess.DEVNULL` gave exit 0 and 0 bytes of standard error for the report, `--help` and `--version`. The same rows with `/proc` replaced by an empty `tmpfs` (`unshare -Urm`) gave the same exit codes: `>&-` 2, `>/dev/null` 0, `1<>/dev/null` 0, `--help >&-` 2, `--help >/dev/null` 0. No run panicked. Whether a closed pipe fails the write depends on when the reader exits. The tests `closed_stdout_exits_2_without_panic`, `dev_full_stdout_exits_2`, `read_only_stdout_exits_2`, `help_and_version_to_unwritable_stdout_exit_2`, `dev_null_stdout_exits_0` and `read_write_dev_null_is_not_a_closed_stdout` (`tests/cli.rs`, Linux only) cover the `>&-`, `/dev/full`, `1<file`, `/dev/null`, `1<>/dev/null`, `--help` and `--version` rows. The `DEVNULL`, `/proc`-less and `| true` rows have no test.

## Usage errors

A usage error that clap prints keeps its line breaks and ends with a line break. Measured with the release binary: `lambda-bill --headroom $'1\x1b[31m\nFAKE: line' examples/sample-report.log` and `lambda-bill $'--x\x1b[31m\nFAKE: line' examples/sample-report.log` both exit 2 and print the argument as `\u{1b}[31m\u{a}FAKE: line` inside clap's own lines, with no line that starts with `FAKE` and no ESC byte. An argument that is not valid UTF-8 (`--function $'a\xff\x1b[31m\nFAKE: line'`) gives clap's own message, which does not repeat it, in three lines plus the usage block, ending with a line break (`clap_error_for_a_non_utf8_argument_keeps_its_lines_and_has_no_fake_line`, Linux only). Tests: `clap_errors_keep_their_line_breaks`, `clap_error_with_escape_and_newline_in_an_argument_has_no_fake_line`, `clap_error_escapes_cr_bidi_and_invisible_characters_on_one_line`.

## Red before, green after

Run on 2026-11-19 against commit dc217f5 (the tests of this state, the sources of dc217f5), then against this state.

| Test | dc217f5 | This state |
| --- | --- | --- |
| `an_escaped_marker_in_a_field_that_is_not_read_is_an_error` | fails: `REPORT\u0020RequestId:` in `@message` gives exit 0 and one request | passes |
| `a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang` | fails: the run is still blocked after 5 s | passes |
| `two_different_log_group_names_in_one_object_are_named_in_the_error` | fails: the message says the log group is not `/aws/lambda/<name>` | passes |
| `a_json_document_with_too_many_values_is_refused_before_it_is_built` | fails: `[[0],...]` of 16400005 bytes is built in full and ends as `not a log event`, no budget error | passes |
| `a_small_gzip_file_cannot_expand_into_too_many_values` | fails: the same text inside a gzip file of under 100000 bytes is built in full | passes |
| `a_prices_or_functions_file_with_too_many_values_is_refused` | fails: a `--prices` file with 4100000 extra arrays is accepted, exit 0 | passes |

The other tests added in this state (`billed_duration_and_memory_fields_are_limited_by_their_text_length`, `text_before_a_json_line_makes_it_text_and_its_marker_does_not_count`, `a_function_header_after_the_first_report_still_names_the_whole_file`, `a_duplicate_request_id_with_other_values_is_only_counted_as_a_duplicate`, the guard `a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json`, the three unit tests in `src/budget.rs`, and the two new tests in `tests/docs_clean.rs`) pin behavior that already existed and is now documented, so they pass on dc217f5 as well. On commit a7d2dd6, before the exact-decimal change, `a_duration_that_needs_more_than_28_digits_exits_2`, `a_product_beyond_28_digits_is_an_error`, `gb_seconds_quotient_beyond_28_digits_is_an_error`, `a_sum_of_exact_rows_that_does_not_fit_exits_2` and `open_finding_half_away_from_zero` exited 0 and printed a rounded amount. They now exit 2. `an_advice_amount_that_is_not_exact_exits_2` keeps an exact row and refuses the raised advice amount. `one_gb_second_at_scale_28_is_exact` keeps one GB-second at `10^-28`. That same commit, with `TMPDIR` set to an empty directory whose path ends in `t512//`, already passed 213 tests and failed none. `tests/docs_clean.rs` and `tests/handbook_help.rs` were shown to fail on a copy: a forbidden word planted in `src/` and in the workflow gave `forbidden text in [...]`, a long option removed from the OPTIONS section of the man page gave `long options in man/lambda-bill.1 must be exactly those in --help`, and a removed option in the usage block gave `man page SYNOPSIS must contain the --help stdout verbatim`.

## Not run

- A fuzz campaign.
- A mutation run.
- `cargo audit` with a refreshed advisory database (no network was used).
- The rows of the Commands table whose Measured on column says 2d51303 (the 1.85 clippy and test rows, the Windows, macOS and musl rows, the unpacked-crate test, `check-glibc.sh` and `cargo audit`): not run on this commit. The tests of `cargo +1.85`: not run, only compiled on fe0a063.
- GitHub Actions: neither `.github/workflows/ci.yml` nor `.github/workflows/release.yml` has run, because of the payment problem on the account. So the `glibc` job (Ubuntu 22.04), the time limits, the release build on five targets, the archive smoke test and the CI badge in the README have no result. The glibc figure above comes from this machine, and the release workflow was only checked as YAML.
- `directory_at_64_levels_is_accepted` builds a path of several hundred characters; on Windows it was not run.
- Any test on Windows, macOS or musl. Only the compile and clippy rows above ran there. The closed-standard-output tests and the tests with an ESC byte in a file name are Linux only, the symlink tests are Unix only, and the probe reports "writable" on other systems.
- `git push`, `git tag` and `cargo publish`: nothing was pushed, tagged or published. The commits are local, so the default branch on GitHub does not show this hash yet.
- The commands of `README.md` Quick start with the installed binary (`cargo install`): the test runs the same bytes through the test binary.
- `cargo test` with `--ignored`: the suite has no ignored test.

## Known limits

- This page records checks. It is not a guarantee and it is not a fuzz report.
- A closed or read-only standard output is recognized on Linux only. Elsewhere it is not detected, and the write itself decides.
- `lambda-bill ... | true` exits 2 when the reader has already closed the pipe at the write. A reader that closes later may see exit 0. No test pins this.
- Memory: every file is read into memory, and all input files of one run together are limited to 268435456 bytes (`all_input_files_together_are_limited_to_the_file_limit`). A JSON document, a `--prices` file and a `--functions` file are counted before they are built and refused over 4000000 values (`a_json_document_with_too_many_values_is_refused_before_it_is_built`, `a_small_gzip_file_cannot_expand_into_too_many_values`, `a_prices_or_functions_file_with_too_many_values_is_refused`). Measured with the release binary at this budget (4000000), peak memory of documents at the budget that were then rejected as not log events: 3990000 scalars (7999981 bytes) 424108 KB, 0.6 s; 1998999 object keys (24875884 bytes) 623372 KB, 2.3 s; 280000 events with four fields (19040001 bytes) 356556 KB, 0.7 s. At the old budget of 20000000 values the same kinds took up to 2846368 KB and 17.2 s. So a document at the budget needs well under 1 GB and a few seconds. Over the budget the refusal took 0.5 to 0.8 s and under 200 MB (`[0,...]` of 100000001 bytes 197076 KB; `[[0],...]` of 100000001 bytes 197900 KB; a gzip file of 657179 bytes with 170 MB of nested arrays 174724 KB). Before the budget the first two needed 5148 MB and 9916 MB. No test measures the memory itself; the tests check the refusal.
- A gzip file is limited to 1000000 members (`a_gzip_file_with_too_many_members_is_refused`); before, 13000000 empty members and one real member (260000109 bytes) took 39.3 s with exit 0. The number of rows is limited too (next item).
- A run reports at most 100000 rows (one per function, architecture and memory size). More is `more than 100000 function and memory rows`, exit 2, also with `--json`; this is pinned by `a_run_with_too_many_functions_is_refused`. The limit replaces the earlier open point about files with very many functions; no timing was taken for this record.
- A JSON line is limited by the line limit (1048576 bytes), not by the value budget. Such a line is built and freed one at a time.
- The crate is not published and has no tag, so no release archive exists (the release workflow: see Not run).
- Lambda's JSON log format (`type: platform.report` records) is not read. Such a record is skipped without a message, and its request is not counted (`platform_report_records_are_not_read`).
- A pretty-printed JSON object that is neither a CloudWatch event nor has `@message` or `Message` is rejected as `line 1: not JSON`. A text log with a line such as `{'a': 1}` is rejected as a whole, on purpose.
- A GB-second, a duration amount, a request amount, an advice amount and a total are stored only when they are already an exact 96-bit decimal (coefficient at most 79228162514264337593543950335, at most 28 digits after the decimal point). Multiplication and division keep every digit. Anything else is `price overflow`, exit 2. Nothing is rounded. Two requests at 79228162514264337593543950335 USD per million fit (`158456325028528675187087.90067`, `widest_request_rate_is_exact_for_one_and_two_requests`). One GB-second at `10^-28` USD stays exact (`one_gb_second_at_scale_28_is_exact`). 10240 MB for 18446744073709551615 ms at 1000000000000 USD per GB-second does not (`huge_price_is_an_input_error`).
- A path that is literally named `bad\xff\xfe.log` is shown the same way as a path with the bytes ff and fe.
- The total is the exact sum of the row amounts. A sum that is not an exact decimal is `price overflow`. The table shows the stored decimal (for example `0.00000165833585`) and does not round it again.
- "No network call" is checked through the dependency list (`cargo tree -e normal`), a search of `src/` for socket and process calls (the only file writes are the temporary file, the rename and the clean-up of `--output` in `write_output_file`, `src/main.rs`; `build.rs` runs `git` at build time), and `claim_c17_no_aws_sdk_and_no_network`. No test watches system calls.
- The test count in `README.md` and in this file is compared with the number of test attributes in `src/` and `tests/` (`tests/quality.rs`), not with the output of `cargo test`.
- A GB-second quotient that is not an exact decimal is `price overflow`, exit 2. Measured: 18446744073709551615 ms at 1023 MB (`gb_seconds_quotient_beyond_28_digits_is_an_error`). It used to be stored rounded. Lambda's longest invocation is 900000 ms.
- A product, a sum or an advice amount that is not an exact decimal is `price overflow`, exit 2. Measured: 1000 MB for 1 ms at `1000000000000000000000001` USD per GB-second is exactly `976562500000000000000.0009765625` (`a_duration_that_needs_more_than_28_digits_exits_2`); 128 MB for 1000 ms at `7922816251426433759354395033` is exactly `990352031428304219919299379.125` (`a_product_beyond_28_digits_is_an_error`); 512 MB for 1000 ms at `79228162514264337593543950333` is exactly `39614081257132168796771975166.5` (`open_finding_half_away_from_zero`); one row of `39614081257132168796771975168` plus one row of `0.50` (`a_sum_of_exact_rows_that_does_not_fit_exits_2`); the advice amount for a raise from 1024 MB to 1229 MB at `1237940039285380274899124224.5` USD per GB-second, while that row itself fits (`an_advice_amount_that_is_not_exact_exits_2`). With the built-in rates and real billed durations this does not show.
- Billed Duration is read with up to 20 characters and Memory Size and Max Memory Used with up to 10, by length and not by value (`billed_duration_and_memory_fields_are_limited_by_their_text_length`). Two rows of 18446744073709551615 ms in one function, architecture and memory are `billed duration overflow` (`billed_duration_overflow_is_an_error`).
- Text in front of a JSON line (a byte-order mark, a comma, `data: `, a timestamp) makes the line plain text; the marker in it is glued to a quote and does not count. Measured: line 1 `{"message":"REPORT RequestId: a1 ..."}` and line 2 the same with `2026-11-19T10:00:00Z ` in front give `"requests": 1` and exit 0 (`text_before_a_json_line_makes_it_text_and_its_marker_does_not_count`). Product decision, open point: refuse such a line, or read it.
- A `# function:` header after the first REPORT still names the whole file; two different headers are `function name conflict` (`a_function_header_after_the_first_report_still_names_the_whole_file`).
- A RequestId twice for one function with other memory or duration values counts once, the first wins, and the warning only counts duplicates (`a_duplicate_request_id_with_other_values_is_only_counted_as_a_duplicate`). Product decision, open point: warn when the values differ.
- A file of 268435456 empty lines took 3.3 s and 526208 KB with the release binary (17.9 s with the binary of dc217f5, which built a label for every line). The memory is the file twice, as bytes and as text. No test measures time.
- The `--prices` and `--functions` files must be regular files; a FIFO or `<(command)` is `not a file` (`a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang`, Unix only).
- A field repeated after the four fixed fields of a REPORT line is trailing text and ignored; the first counts (`a_repeated_field_after_the_fixed_fields_is_trailing_text_and_the_first_counts`).
- `--headroom +5` and `--headroom 05` are accepted as 5 (`headroom_accepts_a_plus_sign_and_leading_zeros`).
- A usage error for an argument that is not UTF-8 is clap's own text, not the escaped render; it does not repeat the argument.
- "Not published and no git tag" is a repository fact. No test checks it.
- The system requirements in `README.md` (glibc 2.34 for a release build on Linux x86_64) were read with `objdump -T` on 2026-11-19 for an earlier build. No test checks them, and no musl, Windows or macOS binary was run.
- The README claims that have no test, or only part of one, are marked in the table below.

## Claim → test

Every claim of `README.md`, with the test that covers it. Names are from the 2026-11-19 run. "Gap" means no test proves it, or only part of it.

| # | Claim | Test | Status |
| --- | --- | --- | --- |
| C1 | One `REPORT` line is one request | `claim_c1_one_report_line_is_one_request` | tested |
| C2 | GB-seconds are memory × billed ms / 1024 / 1000 | `claim_c2_gb_seconds_are_memory_times_billed_over_1024_over_1000` | tested |
| C3 | The Duration field is not used for the cost | `claim_c3_duration_field_is_not_used_for_the_cost` | tested |
| C4 | Init Duration is not added; it marks a cold start | `claim_c4_init_duration_is_not_added_to_billed_duration` | tested |
| C5 | Requests are 0.20 USD per million on both architectures | `claim_c5_requests_are_twenty_cents_per_million_on_both_architectures` | tested |
| C6 | x86_64 rate and the pricing-page example | `claim_c6_x86_duration_rate_is_the_pricing_page_example` | tested |
| C7 | arm64 rate is not exactly 80 percent | `claim_c7_arm_rate_is_not_an_exact_twenty_percent_discount` | tested |
| C8 | Every report names currency, source, date, region, tier | `claim_c8_every_report_names_source_date_region_and_tier` | tested |
| C9 | The free tier is not subtracted | `claim_c9_free_tier_is_not_subtracted` | tested |
| C10 | Provisioned Concurrency is not priced | `claim_c10_provisioned_concurrency_is_not_priced` | tested |
| C11 | Downsize target and its limits | `claim_c11_downsize_uses_ceil_of_max_used_and_floors_at_128` | tested |
| C12 | Raise target at 90 percent, and raise wins over downsize | `claim_c12_raise_uses_configured_memory_at_ninety_percent` | tested |
| C13 | Advice dollars assume the same billed duration; not a forecast | `claim_c13_advice_is_not_a_forecast` | tested |
| C14 | 1 GB is 1024 MB | `claim_c14_one_gb_is_1024_mb` | tested |
| C15 | One row per function, architecture and memory, in order | `claim_c15_a_row_is_function_architecture_and_memory` | tested |
| C16 | A request id counts once per function, first copy wins | `claim_c16_request_id_is_counted_once_and_the_first_copy_wins` | tested |
| C17 | No AWS SDK, no network connection, rates compiled in | `claim_c17_no_aws_sdk_and_no_network` | gap: manifest and sources only, no network test |
| C18 | Default architecture is x86_64; `--functions` wins | `claim_c18_default_architecture_is_x86_64` | tested |
| C19 | Function name precedence and the name rules | `claim_c19_function_name_precedence` | tested |
| C20 | Memory ranges; the number fields are limited by their text length; `billed duration overflow` | `claim_c20_memory_range_is_128_through_10240`, `billed_duration_and_memory_fields_are_limited_by_their_text_length`, `billed_duration_overflow_is_an_error` | tested |
| C21 | Gzip by magic bytes, every member read, trailing bytes are an error | `claim_c21_gzip_is_recognized_by_magic_bytes_not_the_name` (`src/parse.rs`), `gzip_magic_not_the_file_name`, `gzip_members_are_all_read_and_trailing_bytes_fail`, `corrupt_gzip_is_an_error`, `plain_text_named_gz_still_counts` | tested |
| C22 | Request ids are not printed | `claim_c22_request_ids_are_not_printed` | tested |
| C23 | A bad REPORT line, a non-JSON `{` line, an uncounted marker (also in a field of a document that is not read, and also written with JSON escapes), and message plus events (also nested) are errors without the text | `claim_c23_a_bad_report_line_is_not_printed`, `invalid_json_line_is_an_error_with_its_line_number`, `uncounted_report_marker_in_json_or_bracket_line_exits_2`, `root_object_with_message_and_events_exits_2`, `message_and_events_in_a_nested_event_is_an_error`, `a_second_report_in_another_field_of_a_document_is_an_error`, `an_escaped_marker_in_a_field_that_is_not_read_is_an_error`, `note_line_with_report_text_exits_2_with_empty_stdout` | tested |
| C24 | The total is the exact sum of the row amounts; a sum that is not an exact decimal is an error; not scaled | `claim_c24_the_total_is_the_sum_of_the_rows`, `a_sum_of_exact_rows_that_does_not_fit_exits_2`, `open_finding_half_away_from_zero` | tested |
| C25 | JSON field order, strings, trailing newline | `claim_c25_json_includes_the_source_url_and_the_date`, `json_arm_has_source_and_date` | tested |
| C26 | Marker and field order; `XREPORT` does not match; a marker glued to an invisible character is an error | `claim_c26_a_report_needs_the_marker_and_the_fields_in_order`, `invisible_character_before_report_is_an_error_not_a_silent_drop`, `a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error`, `a_repeated_field_after_the_fixed_fields_is_trailing_text_and_the_first_counts` | tested |
| C27 | Units `ms` and `MB`; Billed Duration is an integer | `claim_c27_units_are_ms_and_mb_and_billed_duration_is_an_integer` | tested |
| C28 | `--prices` file rules, including padding, 28 digits and unsafe characters in the source | `claim_c28_a_price_file_replaces_the_builtin_rates`, `price_string_trims_only_ascii_whitespace`, `bidi_and_separator_characters_in_source_are_rejected`, `invisible_filler_in_price_source_is_rejected_without_echo` | tested |
| C29 | Timestamp, bracket, BOM, CRLF ignored; JSON `message`, `@message`, `Message` read, also as a pretty-printed document, with its `logGroup` and `architecture` | `claim_c29_a_timestamp_prefix_is_ignored`, `at_message_line_counts_as_a_request`, `pretty_printed_at_message_document_is_read`, `at_message_document_reads_its_architecture_and_log_group` | tested |
| C30 | Errors do not include contents; unsafe characters in paths and arguments; clap keeps its line breaks | `claim_c30_errors_do_not_include_the_file_contents`, `invisible_filler_in_a_file_name_is_replaced`, `bad_arch_with_control_characters_is_escaped`, `unexpected_argument_with_bidi_override_is_escaped`, `headroom_with_cr_and_bidi_override_is_escaped`, `clap_errors_keep_their_line_breaks`, `clap_error_with_escape_and_newline_in_an_argument_has_no_fake_line`, `clap_error_escapes_cr_bidi_and_invisible_characters_on_one_line`, `non_utf8_path_is_shown_as_byte_escape`, `clap_error_for_a_non_utf8_argument_keeps_its_lines_and_has_no_fake_line` | tested |
| L1 | Install from a local path | `install_uses_a_local_path` (`tests/readme.rs`) | tested |
| L2 | The quick start file is `examples/sample-report.log` | `quickstart_heredoc_matches_the_sample_file` (`tests/readme.rs`) | tested |
| L3 | The console examples are what the binary prints | `console_examples_match_the_binary` (`tests/readme.rs`) | tested |
| L4 | The suite has 273 tests | `the_documents_state_the_real_number_of_tests`, `the_counter_counts_attributes_and_nothing_else` (`tests/quality.rs`) | tested: counts test attributes, not the output of `cargo test` |
| L5 | Not published, no git tag, no release archive | none | gap: repository fact, not tested |
| L6 | File, line and directory limits; symlink rules | `file_larger_than_268435456_bytes`, `line_longer_than_1048576_bytes_is_rejected`, `directory_at_64_levels_is_accepted`, `directory_deeper_than_64_levels_is_an_error`, `directory_symlink_is_skipped`, `explicit_symlink_is_followed` | tested |
| L7 | `--prices` and `--functions` files have the size limit, checked before the file is read, must be UTF-8, and must be regular files | `oversized_prices_and_functions_exit_2`, `oversized_prices_and_functions_are_refused_without_reading_them` (Linux only), `prices_and_functions_files_must_be_utf8`, `prices_file_at_the_limit_is_not_refused_for_size`, `a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang` | tested |
| L8 | A sum that does not fit is `price overflow`, not a panic | `huge_price_is_an_input_error`, `gb_seconds_at_u64_max_does_not_panic`, `a_sum_of_exact_rows_that_does_not_fit_exits_2` | tested |
| L9 | Closed, read-only or full standard output exits 2, also for `--help` and `--version`; `/dev/null` and `1<>/dev/null` exit 0 | `closed_stdout_exits_2_without_panic`, `dev_full_stdout_exits_2`, `read_only_stdout_exits_2`, `help_and_version_to_unwritable_stdout_exit_2`, `dev_null_stdout_exits_0`, `read_write_dev_null_is_not_a_closed_stdout` | tested, Linux only |
| L10 | Other input or usage errors exit 2 | `unknown_option_exits_2`, `missing_file_exits_2`, `no_args_exits_2`, `bad_arch_exits_2`, `headroom_out_of_range_exits_2` (`tests/cli.rs`) | tested |
| L11 | Built-in rates are the first-tier US East figures read on 2026-11-09 | `claim_c8_every_report_names_source_date_region_and_tier` checks the labels, not the page | gap: the page itself is not checked |
| L12 | Limitations list (what is not priced; no extrapolation) | `limitations_name_behaviors_the_tests_cover` (`tests/readme.rs`) checks the text and test names only | gap: partly |
| L13 | System requirements: Rust 1.85, glibc 2.34 for a Linux release build, tests run on Linux only; compiled for Windows, macOS and musl | `licenses_changelog_and_toolchain_match_the_package` checks the Rust version only | gap: glibc and platform statements are not tested |
| L14 | A JSON document, a `--prices` file or a `--functions` file over 4000000 values is refused before it is built, also inside gzip | `a_json_document_with_too_many_values_is_refused_before_it_is_built`, `the_value_budget_keeps_a_document_under_two_gigabytes`, `a_small_gzip_file_cannot_expand_into_too_many_values`, `a_prices_or_functions_file_with_too_many_values_is_refused`, `a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json`, and the unit tests in `src/budget.rs` | tested: the refusal. The memory of a document at the budget (up to about 3 GB) is measured by hand, see Known limits |
| L17 | `--headroom` takes 0 to 100 and reads `+5` and `05` as 5 | `headroom_accepts_a_plus_sign_and_leading_zeros`, `headroom_out_of_range_exits_2` (`tests/cli.rs`) | tested |
| L18 | A GB-second, a product, an advice amount or a sum that is not an exact 96-bit decimal is `price overflow`, exit 2 | `gb_seconds_quotient_beyond_28_digits_is_an_error`, `a_product_beyond_28_digits_is_an_error`, `a_duration_that_needs_more_than_28_digits_exits_2`, `a_sum_of_exact_rows_that_does_not_fit_exits_2`, `an_advice_amount_that_is_not_exact_exits_2`, `open_finding_half_away_from_zero`, `one_gb_second_at_scale_28_is_exact` | tested |
| L19 | Text in front of a JSON line makes it text; a `# function:` header applies to the whole file; a duplicate RequestId with other values counts once | `text_before_a_json_line_makes_it_text_and_its_marker_does_not_count`, `a_function_header_after_the_first_report_still_names_the_whole_file`, `a_duplicate_request_id_with_other_values_is_only_counted_as_a_duplicate` | tested (pins the documented behavior) |
| L20 | A `logGroup` and a `logGroupName` that differ are named in the error | `two_different_log_group_names_in_one_object_are_named_in_the_error` | tested |
| L21 | An empty-line file of 268435456 bytes takes about 3 s and 526208 KB | none | gap: measured by hand, see Known limits |
| L23 | A gzip file of many empty members takes 39 s; many functions cost memory | none | gap: measured by hand, see Known limits |
| L22 | The workflow checks glibc 2.34 on Ubuntu 22.04 | none | gap: the job is listed under Not run; only the script ran on this machine |
| L15 | `platform.report` records are not read; a pretty-printed object without envelope is rejected; the widest request rate stays exact | `platform_report_records_are_not_read`, `pretty_printed_object_without_a_log_envelope_is_rejected_at_line_1`, `widest_request_rate_is_exact_for_one_and_two_requests` | tested |
| L16 | The documents, the manifest, the sources, the tests, the examples, the workflow and the scripts hold no local path, wrapper option, internal file name, internal tool name or AI system name | `project_files_hold_no_local_paths_or_internal_names`, `the_check_finds_a_planted_reference`, `every_scanned_place_is_read_and_a_word_planted_there_is_found` (`tests/docs_clean.rs`) | tested |
| L24 | The text report says it is an estimate (one `Estimate from list prices` line, an `estimated_usd=` line with the total) and the JSON has `estimate`: true, `estimate_note` and `totals.estimated_usd` | `text_output_labels_the_total_as_an_estimate_once`, `json_output_says_it_is_an_estimate` (`tests/cli.rs`) | tested |
| L25 | A price file dated more than 180 days before `--as-of` (or today) gives a `warning:` line with its date on standard error and exit 0; exactly 180 days gives none; a bad `--as-of` exits 2 | `prices_older_than_180_days_warn_on_stderr_and_exit_0`, `the_price_age_limit_is_exactly_180_days`, `the_built_in_prices_are_checked_against_as_of`, `a_bad_as_of_date_exits_2_and_is_escaped` (`tests/cli.rs`), `days_from_date_counts_calendar_days` (`src/money.rs`) | tested: the tests pass `--as-of`, none reads the clock for its result |
| L26 | The shapes the AWS documentation shows (the `filter-log-events` document with tabs, `Init Duration` and `XRAY`, plain `aws logs tail` lines, CRLF, gzip, log groups with `-`, `_`, digits and capitals, pages that overlap, empty pages) are priced to the exact amount | the nine tests of `tests/documented_shapes.rs` | tested: shapes written from the documentation as remembered, no network, no real export. Sources named in the test: https://docs.aws.amazon.com/lambda/latest/dg/monitoring-cloudwatchlogs.html, https://docs.aws.amazon.com/cli/latest/reference/logs/filter-log-events.html and https://docs.aws.amazon.com/AmazonCloudWatch/latest/logs/S3ExportTasks.html. They were not compared with the live pages in this record (no network), so no check date is claimed |
| L27 | `--quiet` prints the result and errors only (no warning on standard error, stdout unchanged); `--verbose` adds a `reading` line per argument and a `done:` line on standard error; both together exit 3 | `help_lists_quiet_and_verbose`, `quiet_with_verbose_is_a_usage_error_with_exit_3`, `quiet_keeps_the_result_and_drops_the_warnings_on_stderr`, `quiet_still_prints_errors`, `verbose_adds_progress_on_stderr_and_keeps_stdout`, `verbose_escapes_the_path_it_reports` (`tests/cli.rs`) | tested |
| L28 | `--md` prints the same amounts as the text and the JSON report as a Markdown table, with the estimate line; a `|`, a backslash and a line break in a cell are escaped; `--md` with `--json` is a usage error | `markdown_amounts_equal_the_text_and_the_json_amounts`, `markdown_and_json_together_are_a_usage_error`, `markdown_escapes_a_pipe_and_a_line_break_in_a_cell` (`tests/cli.rs`, the last on Unix only) | tested |
| L29 | SIGINT exits 130 with the line `lambda-bill: interrupted by SIGINT` and an empty standard output; the handler is Unix only | `sigint_exits_130_with_a_message_and_no_output` (`tests/cli.rs`, Linux only, a real SIGINT sent with `kill`) | tested on Linux; a SIGINT during the write of the report is not tested |
| L30 | `--output FILE` writes the report atomically (temporary file in the same directory, then rename), replaces an existing regular file, refuses a symlink, a directory and a device, creates no directory, leaves no temporary file, and exits 2 with one `cannot write` line on failure; a SIGINT or a failed run keeps the old file | `output_writes_the_report_to_a_file_and_nothing_to_stdout`, `output_replaces_an_existing_file_and_says_so_in_verbose`, `output_in_a_missing_directory_exits_2_and_creates_nothing`, `output_to_a_directory_exits_2_and_leaves_no_temporary_file`, `output_does_not_follow_a_symlink`, `output_to_a_device_is_refused`, `a_failed_run_keeps_the_existing_output_file`, `sigint_keeps_the_existing_output_file` (`tests/cli.rs`; the symlink test on Unix, the device and SIGINT tests on Linux) | tested; a full disk is not tested |
| L31 | Every JSON document has `schema_version` (1) as its first key, as ci-bill does; no error path prints JSON, so none can lack it | `every_json_output_has_schema_version_as_its_first_key`, `a_json_run_that_fails_prints_no_json_at_all` (`tests/cli.rs`) | tested: the second test also passes without the field, it pins that errors print no JSON |

The man page is checked by `help_long_options_match_the_man_page_and_unknown_option_is_not_a_panic` (`tests/handbook_help.rs`): it holds the `--help` text verbatim, and both the whole page and its OPTIONS section have the same long options as `--help`.
| L32 | No test depends on the system clock: every test that runs the binary passes a fixed `--as-of` | the helper `fixed_clock` in `tests/cli.rs`, `console_examples_match_the_binary` and `every_example_command_matches_its_expected_output` (`tests/readme.rs`); `price_age_*` tests keep their own dates | tested: proof by a copy with the clock 400 days ahead, see the header |
| L33 | `--version` prints the crate version, and a debug build in a git checkout adds the commit | `version_is_the_crate_version_and_a_debug_build_may_add_the_commit` (`tests/cli.rs`) | tested |
| L34 | The README has the sections of the documentation standard, in order | `readme_sections_follow_the_documentation_standard` (`tests/readme.rs`) | tested |
| L35 | The quick start writes only into a new temporary directory | `quick_start_writes_nothing_into_the_repository` (`tests/readme.rs`) | tested |
| L36 | The prices section names the source and the date of the price file; the maintenance section names Issues and security | `prices_section_names_the_source_and_date_of_the_price_file` (`tests/readme.rs`) | tested |
| L37 | The examples README has four commands whose output is the expected file | `every_example_command_matches_its_expected_output` (`tests/readme.rs`) | tested |
| L38 | The Dutch README has the same sections, console blocks, claims and figures as the English one | `the_dutch_readme_says_the_same_as_the_english_one` (`tests/readme.rs`) | tested: the prose is not compared word by word |
| L39 | The release workflow builds five archives with the license listing | none | gap: the workflow is listed under Not run; `cargo about generate` was run once by hand |
