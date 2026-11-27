# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

Security fixes go into the latest release only. After a release the tool is in dormant mode: a new version appears only for a reported bug, a request or a security problem, and security always comes first.

## What lambda-bill does

- It only reads: the files, directories and standard input you name. It writes to standard output and standard error. The only file it writes is the one named with `--output`: `write_output_file` in `src/main.rs` creates a temporary file next to it, renames it over the target and removes the temporary file if that fails. The source starts no other program (the build script `build.rs` runs `git` at build time).
- It opens no network connection and links no AWS SDK. `cargo tree -e normal` lists only `clap`, `flate2`, `rust_decimal`, `serde` and `serde_json` with their dependencies, and no HTTP, TLS or async runtime crate.
- The built-in rates are compiled in. A `--prices` file is read as data and is never run.
- The only `unsafe` code is in two modules. `src/stdout_probe.rs`, on Linux: two read-only `fcntl` calls on descriptor 1 (`F_GETFD` and `F_GETFL`). They tell a standard output that was closed or opened for reading only from `1<>/dev/null`. The library has `#![forbid(unsafe_code)]` and the binary has `#![deny(unsafe_code)]`, with one exception for each of the two modules. `src/interrupt.rs`, on Unix: `signal` installs a SIGINT handler that writes one line to standard error and calls `_exit(130)` (both are async-signal-safe), or, while the report is being written, only records the signal.
- A file or standard input over 268435456 bytes, a `--prices` or `--functions` file over that size, and a line over 1048576 bytes are rejected. A `--prices` or `--functions` file that is not a regular file (a pipe or a device) is rejected before it is opened. A directory deeper than 64 levels is rejected.
- A JSON document, a `--prices` file and a `--functions` file may hold at most 4000000 JSON values (a scalar and an object key count 1, an array or object counts 4). A counting pass checks this before the document is built, also for the text inside a gzip file, so a small file cannot expand into tens of gigabytes. A document that stays within the budget peaked at 0.36 to 0.62 GB and 0.6 to 2.3 s in the measurements in `README.md`. Input files together are limited to 268435456 bytes per run, and a gzip file to 1000000 members, so many files or many empty gzip members no longer multiply time and memory. A run reports at most 100000 rows (one per function, architecture and memory size), so the report and its `--json` form stay bounded too. Time and memory are still not capped as a whole: they grow in proportion to the input, up to these limits (`README.md`, Limitations).
- A `REPORT RequestId:` text in a JSON document or JSON line that is not read as a request is an error, also when the marker is written with JSON escapes. Text in front of a JSON line is not covered: the line is then plain text (see the README, Limitations).
- Errors do not print the text of a rejected line, a request id, or the contents of a price file. A control character, line or paragraph separator, or bidi mark in a path or in a rejected argument is replaced or escaped, so it cannot rewrite the terminal line.

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and choose **Report a vulnerability**. If that is not possible, write to pep_ai@icloud.com. Examples are a log line, price file or file name that can add a fake line or a terminal escape to the report or to an error, an error that echoes a rejected line or a request id, a way to make lambda-bill write, run, or connect to something, a file that makes it read without the stated size limit or hang, and input that changes a cost or an advice line without an error.

Please include:

- the lambda-bill version (the `version` field in `Cargo.toml`, or the commit you built) and your operating system;
- the exact command;
- a minimal synthetic log or price file that reproduces the problem, never a real log export;
- what lambda-bill printed and what you expected.

We aim to respond within 7 days (best effort). Once a fix is released, the report can be credited in the changelog if you wish.
