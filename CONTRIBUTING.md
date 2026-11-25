# Contributing

Thanks for your interest in lambda-bill. Bugs and requests go to GitHub Issues of this repository; a security problem goes through the private route in [SECURITY.md](SECURITY.md).

## Building and testing

With Rust 1.85 or newer:

```sh
cargo build
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

These checks must pass before a change is merged. A test does not depend on the system clock: a test that runs the binary passes a fixed `--as-of`, because a price file older than 180 days adds a warning on standard error.

Test input must be synthetic. Never add a real log export, a real request id, a real account or function name.

A claim in `README.md` has a test, and an example in `README.md` is run by a test. When you change behavior, change the claim, the example, `CHANGELOG.md` and the test count in `README.md` and `QUALITY.md` (`tests/quality.rs` compares it with the real number), and keep `README.nl.md` and `QUALITY.nl.md` in line.

## Commit messages

Commit messages are short English sentences that say what changed, for example `Reject a duplicate key in a price file`.

## Contact

PEPAI, pep_ai@icloud.com
