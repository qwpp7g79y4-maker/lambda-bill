# Quality

This commit changes documentation only; the measurements below apply to code commit c5b0ea7.

Measured on 2026-11-25 on c5b0ea7 (origin/main), in a clean worktree. This file replaces the longer earlier record; it states only what this measurement did.

Toolchain: cargo 1.99.0 (5f94df478 2026-08-27), rustc 1.99.0 (b940084d7 2026-09-28), Linux.

Suite: 273 tests: 273 passed, 0 failed, 0 ignored.

## Commands

| Command | Result |
| --- | --- |
| `cargo fmt --check` | exit 0, no differences |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0, no warnings |
| `cargo test --locked` | exit 0, 273 passed, 0 failed, 0 ignored |
| `cargo test --release --locked` | exit 0, 273 passed, 0 failed, 0 ignored |

## Test result per target

| Target | Debug passed | Debug failed | Debug ignored | Release passed | Release failed | Release ignored |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| unittests `src/lib.rs` | 66 | 0 | 0 | 66 | 0 | 0 |
| unittests `src/main.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `tests/claims.rs` | 39 | 0 | 0 | 39 | 0 | 0 |
| `tests/cli.rs` | 140 | 0 | 0 | 140 | 0 | 0 |
| `tests/documented_shapes.rs` | 9 | 0 | 0 | 9 | 0 | 0 |
| `tests/docs_clean.rs` | 3 | 0 | 0 | 3 | 0 | 0 |
| `tests/handbook_help.rs` | 1 | 0 | 0 | 1 | 0 | 0 |
| `tests/quality.rs` | 4 | 0 | 0 | 4 | 0 | 0 |
| `tests/readme.rs` | 11 | 0 | 0 | 11 | 0 | 0 |
| doc-tests | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 273 | 0 | 0 | 273 | 0 | 0 |

## Not run

This measurement did not do the following. Each is not run:

- cargo deny: not run
- cargo audit: not run
- cargo llvm-cov (coverage): not run
- cargo mutants (mutation testing): not run
- cargo fuzz: not run
- Windows and macOS builds and tests: not run
- GitHub Actions (`ci.yml`, `release.yml`), the release build and a release archive: not run; the earlier record names a payment problem on the account
- memory and timing measurements of large inputs: not run
- before/after runs of earlier repairs: not run

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
