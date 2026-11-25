# Examples

Invented fixtures for `lambda-bill`: no real account, function or customer data. Run every command from the root of the repository. The expected outputs were made with `--as-of 2026-11-20` so that the price age check does not depend on the date you run them; without it, a date more than 180 days after the price date adds a `warning:` line on standard error, and the report on standard output stays the same.

## Two requests, one function

```sh
lambda-bill --as-of 2026-11-20 examples/sample-report.log
```

Expected output: `examples/sample-report.expected.txt`

Expected exit code: 0

## An arm64 function

```sh
lambda-bill --as-of 2026-11-20 --arch arm64 --function worker examples/arm-report.log
```

Expected output: `examples/arm-report.expected.txt`

Expected exit code: 0

## The same as JSON

```sh
lambda-bill --as-of 2026-11-20 --json --arch arm64 --function worker examples/arm-report.log
```

Expected output: `examples/arm-report.expected.json`

Expected exit code: 0

## A CloudWatch export directory

`examples/export` has the path `aws/lambda/<name>` for two functions, `billing` and `checkout`.

```sh
lambda-bill --as-of 2026-11-20 examples/export
```

Expected output: `examples/export.expected.txt`

Expected exit code: 0

`examples/functions.json` maps a function name to its architecture, for `--functions`.

`tests/readme.rs` runs each command above and compares the output with the file.
