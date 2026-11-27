//! Command-line behavior: exit codes, warnings, and the file walker.

use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("lambda-bill-cli-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn output_of(cmd: &mut Command) -> (i32, String, String) {
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap(),
        hide_locations(&String::from_utf8(out.stdout).unwrap()),
        hide_locations(&String::from_utf8(out.stderr).unwrap()),
    )
}

/// Drop the temp directory and this process's scratch-directory name.
///
/// Warnings and errors repeat the file path. A pid or a temp-directory name
/// must not satisfy a check for a number or a word. The file name stays.
/// Repeated slashes between the temp directory and the scratch name count as
/// one separator: a temp directory of `.../t512//` is removed whole.
fn hide_locations(text: &str) -> String {
    let mut out = text.to_string();
    let pid = std::process::id().to_string();
    let tmp_owned = std::env::temp_dir().to_string_lossy().into_owned();
    let tmp = tmp_owned.trim_end_matches('/');
    let markers = [format!("lambda-bill-cli-{pid}-"), format!("lb-both-{pid}")];
    for marker in &markers {
        while let Some(start) = out.find(marker.as_str()) {
            let mut end = start + marker.len();
            if marker.ends_with('-') {
                let bytes = out.as_bytes();
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
            }
            let bytes = out.as_bytes();
            let mut from = start;
            while from > 0 && bytes[from - 1] == b'/' {
                from -= 1;
            }
            if !tmp.is_empty() && from >= tmp.len() {
                let tmp_at = from - tmp.len();
                if out.is_char_boundary(tmp_at) && &out[tmp_at..from] == tmp {
                    from = tmp_at;
                }
            }
            out.replace_range(from..end, "");
        }
    }
    out
}

/// Lines that are not warnings. A warning repeats the file path, so a number
/// or a word in that path is not part of the bill.
fn bill_text(stdout: &str) -> String {
    stdout
        .lines()
        .filter(|line| !line.starts_with("warning:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn has_word(text: &str, word: &str) -> bool {
    text.split_whitespace().any(|token| token == word)
}

fn command(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lambda-bill"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.env("NO_COLOR", "1");
    cmd.args(fixed_clock(args));
    cmd
}

/// The price date of the shipped file is fixed, so the date the age check
/// compares it with is fixed too: no test depends on the system clock. A test
/// that sets its own `--as-of` keeps it.
const TEST_AS_OF: &str = "2026-11-20";

fn fixed_clock<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let own = args
        .iter()
        .any(|a| *a == "--as-of" || a.starts_with("--as-of="));
    // No arguments is the usage error, which has no clock to fix.
    let mut all = Vec::new();
    if !own && !args.is_empty() {
        all.extend(["--as-of", TEST_AS_OF]);
    }
    all.extend(args);
    all
}

fn run(args: &[&str]) -> (i32, String, String) {
    output_of(&mut command(args))
}

fn run_stdin(args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut cmd = command(args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    // Wait on every path: a failed write must not leave a zombie behind.
    let written = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    written.unwrap();
    (
        out.status.code().unwrap(),
        hide_locations(&String::from_utf8(out.stdout).unwrap()),
        hide_locations(&String::from_utf8(out.stderr).unwrap()),
    )
}

fn report(id: &str, memory: u32, max_used: u32) -> String {
    format!(
        "# function: api\nREPORT RequestId: {id} Duration: 10 ms Billed Duration: 100 ms Memory Size: {memory} MB Max Memory Used: {max_used} MB\n"
    )
}

fn prices_with(request: &str, x86: &str, arm: &str) -> String {
    format!(
        "{{\"currency\":\"USD\",\"source\":\"https://example.test/a\",\"date\":\"2026-01-02\",\"region\":\"us-east-1\",\"tier\":\"first\",\"request_per_million\":\"{request}\",\"x86_64_per_gb_second\":\"{x86}\",\"arm64_per_gb_second\":\"{arm}\"}}"
    )
}

/// `--json` over a price file and one or more logs in a scratch directory.
fn run_priced(body: &str, logs: &[(&str, &str)]) -> (i32, String, String) {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, body).unwrap();
    let mut owned = vec![
        "--json".to_string(),
        "--as-of".to_string(),
        "2026-01-03".to_string(),
        "--prices".to_string(),
        prices.to_str().unwrap().to_string(),
    ];
    for (name, text) in logs {
        let path = scratch.path().join(name);
        fs::write(&path, text).unwrap();
        owned.push(path.to_str().unwrap().to_string());
    }
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    run(&args)
}

#[test]
fn help_exits_0() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("no network"));
    assert!(stdout.contains(
        "Exit codes: 0 ok, 2 input or usage error, 3 --quiet together with --verbose, 130 SIGINT."
    ));
    assert!(stdout.contains("--prices"));
    assert!(stdout.contains("--headroom"));
    assert!(stdout.contains("--json"));
}

#[test]
fn version_exits_0() {
    let (code, stdout, stderr) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("lambda-bill 1.0.2"));
}

#[test]
fn unknown_option_exits_2() {
    let (code, stdout, stderr) = run(&["--not-a-flag", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.is_empty());
    assert!(!stderr.contains("panicked"));
}

#[test]
fn version_is_the_crate_version_and_a_debug_build_may_add_the_commit() {
    for flag in ["--version", "-V"] {
        let (code, stdout, stderr) = run(&[flag]);
        assert_eq!(code, 0, "{flag}: {stderr}");
        assert!(stderr.is_empty(), "{flag}: {stderr}");
        let line = stdout.trim_end_matches('\n');
        let rest = line
            .strip_prefix(concat!("lambda-bill ", env!("CARGO_PKG_VERSION")))
            .unwrap_or_else(|| panic!("unexpected version line {line:?}"));
        // A debug build in a git checkout ends in ` (<40 hex digits>)`. A
        // release build, or a build without git, ends after the version.
        if !rest.is_empty() {
            let sha = rest
                .strip_prefix(" (")
                .and_then(|text| text.strip_suffix(')'))
                .unwrap_or_else(|| panic!("unexpected version line {line:?}"));
            assert!(
                sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
                "{line:?}"
            );
        }
        assert!(!stdout.trim_end_matches('\n').contains('\n'), "{stdout:?}");
        // Where git reports a commit for this tree, a debug build names it.
        let head = Command::new("git")
            .args(["rev-parse", "--verify", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output();
        if let Some(head) = head.ok().filter(|out| out.status.success()) {
            let head = String::from_utf8(head.stdout).unwrap();
            if cfg!(debug_assertions) {
                assert!(rest.contains(head.trim()), "{line:?} lacks {head}");
            } else {
                assert!(rest.is_empty(), "a release build adds no commit: {line:?}");
            }
        }
    }
}

#[test]
fn missing_file_exits_2() {
    let (code, stdout, stderr) = run(&["examples/no-such-file.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("lambda-bill: "));
    assert!(stderr.contains("examples/no-such-file.log"));
    assert!(!stderr.contains("panicked"));
}

#[test]
fn no_args_exits_2() {
    let (code, stdout, stderr) = run(&[]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.is_empty());
}

#[test]
fn bad_arch_exits_2() {
    let (code, stdout, stderr) = run(&[
        "--arch",
        "x86",
        "--function",
        "worker",
        "examples/arm-report.log",
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("invalid --arch x86: expected x86_64 or arm64"));
}

#[test]
fn bad_arch_with_control_characters_is_escaped() {
    let (code, stdout, stderr) = run(&[
        "--arch",
        "\u{1b}[31m\nlambda-bill: fake",
        "--function",
        "worker",
        "examples/arm-report.log",
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.contains('\u{1b}'));
    assert_eq!(stderr.lines().count(), 1);
    assert!(stderr.contains("invalid --arch \\u{1b}[31m\\u{a}lambda-bill: fake:"));
}

#[test]
fn headroom_out_of_range_exits_2() {
    let cases: &[&[&str]] = &[
        &["--headroom", "101", "examples/sample-report.log"],
        &["--headroom=-1", "examples/sample-report.log"],
    ];
    for args in cases {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
        assert!(stderr.contains("headroom"), "{stderr}");
        assert!(stderr.contains("0 to 100"), "{stderr}");
    }
}

#[test]
fn unexpected_argument_with_bidi_override_is_escaped() {
    let (code, stdout, stderr) = run(&["--x\r\u{202E}", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.contains('\r'), "stderr has a raw CR");
    assert!(!stderr.contains('\u{202E}'), "stderr has a raw U+202E");
    assert!(stderr.contains("\\u{202e}"));
}

#[test]
fn json_with_a_value_and_bidi_override_is_escaped() {
    let (code, stdout, stderr) = run(&["--json=x\r\u{202E}", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.contains('\r'), "stderr has a raw CR");
    assert!(!stderr.contains('\u{202E}'), "stderr has a raw U+202E");
    assert!(stderr.contains("\\u{202e}"), "stderr has no escaped form");
}

#[test]
fn headroom_with_cr_and_bidi_override_is_escaped() {
    let (code, stdout, stderr) = run(&["--headroom", "7\r\u{202E}", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(!stderr.contains('\r'), "{stderr:?}");
    assert!(!stderr.contains('\u{202E}'), "{stderr:?}");
    assert!(stderr.contains("\\u{d}"));
    assert!(stderr.contains("\\u{202e}"));
    assert!(stderr.contains("0 to 100"));
}

#[test]
fn sample_report_exits_0() {
    let (code, stdout, stderr) = run(&["examples/sample-report.log"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("https://aws.amazon.com/lambda/pricing/"));
    assert!(stdout.contains("2026-11-09"));
    assert!(stdout.contains("downsize to 128 MB"));
    assert!(!stdout.contains("11111111-1111-1111-1111-111111111111"));
}

#[test]
fn stdin_larger_than_268435456_bytes_is_an_error() {
    let scratch = Scratch::new();
    let big = scratch.path().join("big.stdin");
    let file = File::create(&big).unwrap();
    file.set_len(268_435_457).unwrap();
    let mut cmd = command(&["-"]);
    cmd.stdin(Stdio::from(File::open(&big).unwrap()));
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("-: file is larger than 268435456 bytes"),
        "{stderr}"
    );
}

#[test]
fn stdin_matches_the_sample_file() {
    let bytes =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/sample-report.log")).unwrap();
    let (code, stdout, stderr) = run_stdin(&["-"], &bytes);
    let (file_code, file_stdout, file_stderr) = run(&["examples/sample-report.log"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(file_code, 0);
    assert!(file_stderr.is_empty());
    assert_eq!(stdout, file_stdout);
}

#[test]
fn export_directory_orders_billing_before_checkout() {
    let (code, stdout, stderr) = run(&["examples/export"]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let billing = stdout.find("billing").unwrap();
    let checkout = stdout.find("checkout").unwrap();
    assert!(billing < checkout);
    assert!(stdout.contains("downsize to 128 MB"));
}

#[test]
fn functions_file_selects_arm64() {
    let (mapped, _, mapped_err) = run(&[
        "--function",
        "worker",
        "--functions",
        "examples/functions.json",
        "examples/arm-report.log",
    ]);
    let (flag, flag_out, flag_err) = run(&[
        "--arch",
        "arm64",
        "--function",
        "worker",
        "examples/arm-report.log",
    ]);
    assert_eq!(mapped, 0);
    assert_eq!(flag, 0);
    assert!(mapped_err.is_empty());
    assert!(flag_err.is_empty());
    let (mapped_code, mapped_out, _) = run(&[
        "--function",
        "worker",
        "--functions",
        "examples/functions.json",
        "examples/arm-report.log",
    ]);
    assert_eq!(mapped_code, 0);
    assert_eq!(mapped_out, flag_out);
    assert!(mapped_out.contains("arm64"));
    assert!(!mapped_out.contains("x86_64"));
}

#[test]
fn json_arm_has_source_and_date() {
    let (code, stdout, stderr) = run(&[
        "--json",
        "--arch",
        "arm64",
        "--function",
        "worker",
        "examples/arm-report.log",
    ]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.ends_with('\n'));
    assert!(stdout.contains("\"source\": \"https://aws.amazon.com/lambda/pricing/\""));
    assert!(stdout.contains("\"date\": \"2026-11-09\""));
    assert!(stdout.contains("\"free_tier_applied\": false"));
    assert!(stdout.contains("\"provisioned_concurrency_included\": false"));
    assert!(!stdout.contains("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"));
}

#[test]
fn duplicate_warning_omits_the_request_id() {
    let scratch = Scratch::new();
    let first = scratch.path().join("a.log");
    let second = scratch.path().join("b.log");
    fs::write(&first, report("RIDSECRET", 128, 10)).unwrap();
    fs::write(&second, report("RIDSECRET", 512, 400)).unwrap();
    let (code, stdout, stderr) = run(&[first.to_str().unwrap(), second.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("warning: skipped 1 duplicate RequestId"));
    assert!(!stdout.contains("RIDSECRET"));
    let bill = bill_text(&stdout);
    assert!(has_word(&bill, "128"), "{bill}");
    assert!(!has_word(&bill, "512"), "{bill}");
}

#[test]
fn json_warnings_go_to_stderr_and_text_warnings_stay_on_stdout() {
    let scratch = Scratch::new();
    let first = scratch.path().join("a.log");
    let second = scratch.path().join("b.log");
    fs::write(&first, report("RIDSECRET", 128, 10)).unwrap();
    fs::write(&second, report("RIDSECRET", 256, 20)).unwrap();
    let paths = [first.to_str().unwrap(), second.to_str().unwrap()];
    let (code, stdout, stderr) = run(&paths);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("warning: skipped 1 duplicate RequestId"));

    let mut args = vec!["--json"];
    args.extend_from_slice(&paths);
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, 0);
    assert!(stderr.contains("warning: skipped 1 duplicate RequestId"));
    assert!(stdout.contains("\"skipped 1 duplicate RequestId\""));
    assert!(!stdout.contains("warning:"));
    assert!(!stdout.contains("RIDSECRET"));
    assert!(!stderr.contains("RIDSECRET"));
}

#[test]
fn prices_override_changes_the_rate() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{
            "currency": "USD",
            "source": "https://example.test/prices",
            "date": "2026-01-02",
            "region": "eu-west-1",
            "tier": "second",
            "request_per_million": "2",
            "x86_64_per_gb_second": "1",
            "arm64_per_gb_second": "1",
            "note": "ignored-SECRET"
        }"#,
    )
    .unwrap();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-01-03",
        "--prices",
        prices.to_str().unwrap(),
        log.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains(
        "source https://example.test/prices, date 2026-01-02, region eu-west-1, tier second"
    ));
    assert!(stdout.contains("0.100002"));
    assert!(!stdout.contains("SECRET"));
}

#[test]
fn headroom_0_and_20_change_the_advice() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 256, 200)).unwrap();
    let path = log.to_str().unwrap();
    let (code, tight, stderr) = run(&["--headroom", "0", path]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(tight.contains("downsize to 200 MB"));
    let (code, loose, stderr) = run(&["--headroom", "20", path]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(loose.contains("downsize to 240 MB"));
    assert!(loose.contains("headroom: 20%"));
    assert!(tight.contains("headroom: 0%"));
}

#[cfg(unix)]
#[test]
fn directory_symlink_is_skipped() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(walk.join("good.log"), report("good", 128, 10)).unwrap();
    let outside = scratch.path().join("outside.log");
    fs::write(&outside, report("other", 512, 400)).unwrap();
    symlink(&outside, walk.join("link.log")).unwrap();
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("skipped symlink"));
    let bill = bill_text(&stdout);
    assert!(has_word(&bill, "128"), "{bill}");
    assert!(!has_word(&bill, "512"), "{bill}");
    assert!(!has_word(&bill, "other"), "{bill}");
}

/// A temp directory named with `512` and a repeated slash used to make the
/// bill checks above fail, because the warning repeats the path. Re-run those
/// checks in that directory. The exit status is not a path.
///
/// Unix only: the shapes use `/` as separator and the walk checks use symlinks.
#[cfg(unix)]
#[test]
fn open_finding_temp_dir_repeated_slash() {
    let root = std::env::temp_dir().join(format!("lb-probe-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("t512")).unwrap();
    fs::create_dir_all(root.join("p512").join("leaf")).unwrap();
    let shapes = [
        format!("{}/t512//", root.display()),
        format!("{}/p512/leaf//", root.display()),
    ];
    let names = [
        "directory_symlink_is_skipped",
        "duplicate_warning_omits_the_request_id",
        "directory_walk_skips_a_non_regular_file_with_a_warning",
    ];
    for tmp in &shapes {
        for name in names {
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name, "--test-threads=1"])
                .env("TMPDIR", tmp)
                .env("NO_COLOR", "1")
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            // `1 passed` is the harness summary. A directory name cannot contain it.
            assert!(
                out.status.success() && text.contains("1 passed"),
                "{name} with TMPDIR {tmp} exited {:?}\n{text}",
                out.status.code()
            );
        }
    }
    let _ = fs::remove_dir_all(&root);
}

/// Amounts that are not an exact decimal. Calculated with `fractions.Fraction`.
///
/// 512 MB for 1000 ms is 1/2 GB-second. At `79228162514264337593543950333` the
/// product is `...166.5` (30 digits). A rate of `1e-28` times 1/2 GB-second is
/// `5e-29` (scale 29). Neither fits in a 96-bit decimal.
#[test]
fn open_finding_half_away_from_zero() {
    let line = "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 512 MB Max Memory Used: 5 MB\n";
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", "79228162514264337593543950333", "1"),
        &[("one.log", line)],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    assert!(
        !stderr.contains("39614081257132168796771975167"),
        "{stderr}"
    );
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", "79228162514264337593543950333", "1"),
        &[
            ("left.log", "# function: fa\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 512 MB Max Memory Used: 5 MB\n"),
            ("right.log", "# function: fb\nREPORT RequestId: b Duration: 1 ms Billed Duration: 1000 ms Memory Size: 512 MB Max Memory Used: 5 MB\n"),
        ],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", "0.0000000000000000000000000001", "1"),
        &[("one.log", line)],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn explicit_symlink_is_followed() {
    let scratch = Scratch::new();
    let real = scratch.path().join("real.log");
    fs::write(&real, report("a1", 128, 10)).unwrap();
    let link = scratch.path().join("link.log");
    symlink(&real, &link).unwrap();
    let (code, stdout, stderr) = run(&[link.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("api"));
    assert!(has_word(&bill_text(&stdout), "128"), "{stdout}");
    assert!(!stdout.contains("skipped symlink"));
}

#[test]
fn directory_at_64_levels_is_accepted() {
    let scratch = Scratch::new();
    let mut dir = scratch.path().to_path_buf();
    for i in 0..63 {
        dir.push(format!("n{i}"));
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.log"), report("a1", 128, 10)).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("api"));
}

#[test]
fn directory_deeper_than_64_levels_is_an_error() {
    let scratch = Scratch::new();
    let mut dir = scratch.path().to_path_buf();
    for i in 0..64 {
        dir.push(format!("n{i}"));
    }
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.log"), report("a1", 128, 10)).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("directory is nested too deeply"));
    assert!(stderr.starts_with("lambda-bill: "));
}

#[test]
fn bad_report_does_not_echo_the_line() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(
        &log,
        "REPORT RequestId: SECRETLINE Duration: nope ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not a Lambda REPORT line"));
    assert!(!stderr.contains("SECRETLINE"));
    assert!(!stderr.contains("nope"));
}

#[test]
fn file_larger_than_268435456_bytes() {
    let scratch = Scratch::new();
    let log = scratch.path().join("big.log");
    let file = File::create(&log).unwrap();
    file.set_len(268_435_457).unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("file is larger than 268435456 bytes"));
    assert!(!stderr.contains("SECRET"));
}

#[test]
fn directory_skips_non_utf8_and_empty_members() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("good.log"), report("a1", 128, 10)).unwrap();
    fs::write(scratch.path().join("empty.log"), b"").unwrap();
    let mut bad = vec![0xff, 0xfe];
    bad.extend(b"SECRETLEAK");
    fs::write(scratch.path().join("bad.bin"), bad).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("not UTF-8"));
    assert!(stdout.contains("no REPORT lines"));
    assert!(stdout.contains("api"));
    assert!(!stdout.contains("SECRETLEAK"));
}

#[test]
fn corrupt_gzip_is_an_error() {
    let scratch = Scratch::new();
    let log = scratch.path().join("notes.txt");
    let mut bytes = vec![0x1f, 0x8b];
    bytes.extend(b"SECRETLEAK");
    fs::write(&log, bytes).unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("could not read gzip"));
    assert!(!stderr.contains("SECRETLEAK"));
}

#[test]
fn plain_text_named_gz_still_counts() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("notes.gz"), report("a1", 128, 10)).unwrap();
    let (code, stdout, stderr) = run(&[scratch.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("api"));
    assert!(!stdout.contains("could not read gzip"));
}

#[test]
fn explicit_empty_file_errors() {
    let scratch = Scratch::new();
    let log = scratch.path().join("empty.log");
    fs::write(&log, b"").unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("no REPORT lines"));
}

#[test]
fn bad_function_name_is_not_echoed() {
    let (code, stdout, stderr) = run(&["--function", "SECRET NAME", "examples/arm-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad function name"));
    assert!(!stderr.contains("SECRET"));
}

#[test]
fn bad_functions_file_exits_2_without_echoing_the_value() {
    let scratch = Scratch::new();
    let functions = scratch.path().join("functions.json");
    fs::write(&functions, r#"{"worker":"SECRET99"}"#).unwrap();
    let (code, stdout, stderr) = run(&[
        "--functions",
        functions.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("functions file"));
    assert!(stderr.contains("bad architecture"));
    assert!(!stderr.contains("SECRET99"));
}

#[test]
fn price_file_error_does_not_echo_the_secret() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{"currency":"USD","source":"https://example.test","date":"2026-11-09","region":"us-east-1","tier":"first","request_per_million":"SECRET99","x86_64_per_gb_second":"1","arm64_per_gb_second":"1"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad request_per_million"));
    assert!(stderr.contains("price file"));
    assert!(!stderr.contains("SECRET99"));
}

#[test]
fn huge_price_is_an_input_error() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        r#"{
            "currency": "USD",
            "source": "https://example.test/prices",
            "date": "2026-01-02",
            "region": "us-east-1",
            "tier": "first",
            "request_per_million": "0.20",
            "x86_64_per_gb_second": "1000000000000",
            "arm64_per_gb_second": "1000000000000"
        }"#,
    )
    .unwrap();
    let log = scratch.path().join("a.log");
    fs::write(
        &log,
        "# function: api\nREPORT RequestId: a1 Duration: 1 ms Billed Duration: 18446744073709551615 ms Memory Size: 10240 MB Max Memory Used: 1 MB\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-01-03",
        "--prices",
        prices.to_str().unwrap(),
        log.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("lambda-bill: "));
    assert!(stderr.contains("price overflow"));
    assert!(!stderr.to_lowercase().contains("panic"));
    assert!(!stderr.contains("1000000000000"));
    assert!(!stderr.contains("18446744073709551615"));
}

// An ESC byte in a file name: Windows refuses it, so Linux only.
#[cfg(target_os = "linux")]
#[test]
fn control_characters_in_a_file_name_are_replaced() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(walk.join("good.log"), report("good", 128, 10)).unwrap();
    let escaped = walk.join("bad\u{1b}name.log");
    fs::write(&escaped, b"").unwrap();
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(!stdout.contains('\u{1b}'));
    assert!(stdout.contains("bad\u{FFFD}name.log"));
    assert!(stdout.contains("no REPORT lines"));

    let (code, stdout, stderr) = run(&[escaped.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("lambda-bill: "));
    assert!(!stderr.contains('\u{1b}'));
    assert!(stderr.contains("bad\u{FFFD}name.log"));
    assert!(stderr.contains("no REPORT lines"));

    let prices = scratch.path().join("price\u{1b}.json");
    fs::write(&prices, b"{").unwrap();
    let (code, stdout, stderr) = run(&[
        "--prices",
        prices.to_str().unwrap(),
        walk.join("good.log").to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(!stderr.contains('\u{1b}'));
    assert!(stderr.contains("price\u{FFFD}.json"));
    assert!(stderr.contains("price file"));
}

#[test]
fn line_separator_and_bidi_override_in_a_file_name_are_replaced() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    let few = "# comment\nnot a report\nstill three lines\n";
    fs::write(walk.join("good.log"), report("good", 128, 10)).unwrap();
    let line_sep = walk.join("bad\u{2028}name.log");
    let bidi = walk.join("bad\u{202E}name.log");
    fs::write(&line_sep, few).unwrap();
    fs::write(&bidi, few).unwrap();

    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(!stdout.contains('\u{2028}'));
    assert!(!stdout.contains('\u{202E}'));
    assert!(stdout.contains("bad\u{FFFD}name.log"));
    assert!(stdout.contains("no REPORT lines"));
    assert!(stdout.contains("api"));

    for path in [&line_sep, &bidi] {
        let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.starts_with("lambda-bill: "));
        assert!(!stderr.contains('\u{2028}'));
        assert!(!stderr.contains('\u{202E}'));
        assert!(stderr.contains("bad\u{FFFD}name.log"));
        assert!(stderr.contains("no REPORT lines"));
    }

    let plain = scratch.path().join("plain.log");
    fs::write(&plain, few).unwrap();
    let (code, stdout, stderr) = run(&[plain.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    // The absolute directory holds the pid, so the check is the file name.
    assert!(stderr.contains("plain.log"), "{stderr}");
    assert!(!stderr.contains('\u{FFFD}'));
    assert!(!stderr.contains('\u{2028}'));
    assert!(!stderr.contains('\u{202E}'));
}

#[test]
fn duplicate_event_key_exits_2() {
    let scratch = Scratch::new();
    let big = "REPORT RequestId: a1 Duration: 1 ms Billed Duration: 100 ms Memory Size: 1024 MB Max Memory Used: 10 MB";
    let small = "REPORT RequestId: a2 Duration: 1 ms Billed Duration: 100 ms Memory Size: 512 MB Max Memory Used: 10 MB";
    let one = "REPORT RequestId: a3 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 10 MB";

    let ok = scratch.path().join("ok.json");
    fs::write(
        &ok,
        format!(
            "{{\n  \"events\": [\n    {{\"message\": \"{one}\", \"logGroup\": \"/aws/lambda/kept\"}}\n  ]\n}}\n"
        ),
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[ok.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("kept"));

    let files = [
        (
            "message.json",
            format!(
                "{{\n  \"message\": \"{big}\",\n  \"message\": \"{small}\",\n  \"logGroup\": \"/aws/lambda/alpha\"\n}}\n"
            ),
            false,
        ),
        (
            "loggroup.json",
            format!(
                "{{\n  \"message\": \"{one}\",\n  \"logGroup\": \"/aws/lambda/alpha\",\n  \"logGroup\": \"/aws/lambda/beta\"\n}}\n"
            ),
            false,
        ),
        (
            "arch.json",
            format!(
                "{{\n  \"message\": \"{one}\",\n  \"logGroup\": \"/aws/lambda/alpha\",\n  \"architecture\": \"arm64\",\n  \"architecture\": \"x86_64\"\n}}\n"
            ),
            false,
        ),
        (
            "events.json",
            format!(
                "{{\n  \"events\": [\n    {{\"message\": \"{big}\", \"logGroup\": \"/aws/lambda/alpha\"}}\n  ],\n  \"events\": [\n    {{\"message\": \"{small}\", \"logGroup\": \"/aws/lambda/beta\"}}\n  ]\n}}\n"
            ),
            false,
        ),
        (
            "inside.json",
            format!(
                "{{\n  \"events\": [\n    {{\"message\": \"{big}\", \"message\": \"{small}\"}}\n  ]\n}}\n"
            ),
            false,
        ),
        (
            "lines.json",
            format!(
                "{{\"message\": \"{one}\", \"logGroup\": \"/aws/lambda/alpha\"}}\n{{\"message\": \"{big}\", \"message\": \"{small}\"}}\n"
            ),
            true,
        ),
        (
            "array.json",
            format!(
                "[\n  {{\"message\": \"{big}\", \"message\": \"{small}\", \"logGroup\": \"/aws/lambda/alpha\"}}\n]\n"
            ),
            false,
        ),
        (
            "timestamp.json",
            format!(
                "{{\n  \"message\": \"{one}\",\n  \"logGroup\": \"/aws/lambda/alpha\",\n  \"timestamp\": 1,\n  \"timestamp\": 2\n}}\n"
            ),
            false,
        ),
    ];
    for (name, body, line_two) in files {
        let path = scratch.path().join(name);
        fs::write(&path, body).unwrap();
        let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}");
        assert!(stderr.starts_with("lambda-bill: "), "{name}: {stderr}");
        assert!(stderr.contains(": duplicate key"), "{name}: {stderr}");
        assert!(!stderr.contains("RequestId"), "{name}: {stderr}");
        assert!(!stderr.contains("Memory Size"), "{name}: {stderr}");
        assert!(!stderr.contains("/aws/lambda"), "{name}: {stderr}");
        assert!(!stderr.contains("arm64"), "{name}: {stderr}");
        assert!(!stderr.contains("beta"), "{name}: {stderr}");
        assert!(!stderr.to_lowercase().contains("panic"), "{name}: {stderr}");
        if line_two {
            assert!(stderr.contains("line 2"), "{name}: {stderr}");
        }
    }
}

#[test]
fn surrogate_pair_before_a_duplicate_key_exits_2() {
    let scratch = Scratch::new();
    let path = scratch.path().join("pair.json");
    fs::write(
        &path,
        r#"{"message":"REPORT RequestId: a1 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 10 MB \uD83D\uDE00","message":"REPORT RequestId: a2 Duration: 1 ms Billed Duration: 900 ms Memory Size: 128 MB Max Memory Used: 10 MB"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.starts_with("lambda-bill: "), "{stderr}");
    assert!(stderr.contains(": duplicate key"), "{stderr}");
    assert!(!stderr.contains("not JSON"), "{stderr}");
    assert!(!stderr.contains("RequestId"), "{stderr}");
    assert!(!stderr.contains("Memory Size"), "{stderr}");
    assert!(!stderr.contains("\u{1F600}"), "{stderr}");
    assert!(!stderr.to_lowercase().contains("panic"), "{stderr}");
}

#[test]
fn duplicate_key_that_changes_the_envelope_exits_2() {
    let scratch = Scratch::new();
    let path = scratch.path().join("shape.json");
    fs::write(
        &path,
        "{\"message\":\"R1\",\"message\":5}\n{\"message\":\"R2\"}\n",
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.starts_with("lambda-bill: "), "{stderr}");
    assert!(stderr.contains("line 1"), "{stderr}");
    assert!(stderr.contains(": duplicate key"), "{stderr}");
    assert!(!stderr.contains("R1"), "{stderr}");
    assert!(!stderr.contains("R2"), "{stderr}");
    assert!(!stderr.to_lowercase().contains("panic"), "{stderr}");
}

fn ndjson_event(id: &str, ms: u32, stream: &str, tail: &str) -> String {
    format!(
        r#"{{"message":"REPORT RequestId: {id} Duration: 1 ms Billed Duration: {ms} ms Memory Size: 128 MB Max Memory Used: 10 MB","logGroup":"/aws/lambda/f","logStream":"{stream}"{tail}}}"#
    )
}

#[test]
fn invalid_json_line_is_an_error_with_its_line_number() {
    let cases = [
        ("lone surrogate", r"x\ud800", ""),
        ("trailing comma", "x", ","),
    ];
    for (name, stream, tail) in cases {
        let scratch = Scratch::new();
        let path = scratch.path().join("nd.log");
        let text = format!(
            "{}\n{}\n{}\n",
            ndjson_event("a1", 100, "s", ""),
            ndjson_event("a2", 900, stream, tail),
            ndjson_event("a3", 100, "s", "")
        );
        fs::write(&path, text).unwrap();
        let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}: {stdout}");
        assert!(stderr.contains("line 2: not JSON"), "{name}: {stderr}");
        assert!(!stderr.contains("RequestId"), "{name}: {stderr}");
    }
}

#[test]
fn lone_surrogate_does_not_hide_a_duplicate_key_or_a_request() {
    let scratch = Scratch::new();
    let path = scratch.path().join("dup.log");
    fs::write(
        &path,
        r#"{"message":" REPORT RequestId: a1 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 10 MB","architecture":"arm64","architecture":"x86_64","x":"\udc00"}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("not JSON"), "{stderr}");
}

#[test]
fn valid_surrogate_pair_in_ndjson_counts_all_requests() {
    let scratch = Scratch::new();
    let path = scratch.path().join("pair.log");
    let text = format!(
        "{}\n{}\n{}\n",
        ndjson_event("a1", 100, "s", ""),
        ndjson_event("a2", 900, r"x😀", ""),
        ndjson_event("a3", 100, "s", "")
    );
    fs::write(&path, text).unwrap();
    let (code, stdout, stderr) = run(&["--json", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 3"), "{stdout}");
}

#[cfg(target_os = "linux")]
fn run_shell_redirect(redirect: &str) -> (i32, String) {
    run_shell_args("examples/sample-report.log", redirect)
}

#[cfg(target_os = "linux")]
fn run_shell_args(args: &str, redirect: &str) -> (i32, String) {
    let bin = env!("CARGO_BIN_EXE_lambda-bill");
    let script = format!("exec \"$0\" --as-of {TEST_AS_OF} {args} {redirect}");
    let out = Command::new("sh")
        .args(["-c", &script, bin])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[cfg(target_os = "linux")]
#[test]
fn closed_stdout_exits_2_without_panic() {
    let (code, stderr) = run_shell_redirect(">&-");
    assert_eq!(code, 2);
    assert!(stderr.contains("could not write output"));
    assert!(!stderr.contains("panicked"));
}

#[cfg(target_os = "linux")]
#[test]
fn dev_full_stdout_exits_2() {
    assert_eq!(run_shell_redirect(">/dev/full").0, 2);
}

#[cfg(target_os = "linux")]
#[test]
fn help_and_version_to_unwritable_stdout_exit_2() {
    for flag in ["--help", "--version"] {
        for redirect in [">&-", ">/dev/full"] {
            let (code, stderr) = run_shell_args(flag, redirect);
            assert_eq!(code, 2, "{flag} {redirect}: {stderr}");
            assert!(
                stderr.contains("stdout is not writable"),
                "{flag} {redirect}: {stderr}"
            );
            assert!(!stderr.contains("panicked"), "{flag} {redirect}: {stderr}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn dev_null_stdout_exits_0() {
    let (code, stderr) = run_shell_redirect(">/dev/null");
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn read_write_dev_null_is_not_a_closed_stdout() {
    let (code, stderr) = run_shell_redirect("1<>/dev/null");
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
}

fn sparse(path: &Path, len: u64) {
    File::create(path).unwrap().set_len(len).unwrap();
}

#[test]
fn oversized_prices_and_functions_exit_2() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    for flag in ["--prices", "--functions"] {
        let big = scratch.path().join("big.json");
        sparse(&big, 268_435_457);
        let (code, _, stderr) = run(&[flag, big.to_str().unwrap(), log.to_str().unwrap()]);
        assert_eq!(code, 2, "{flag}");
        assert!(
            stderr.contains("file is larger than 268435456 bytes"),
            "{flag}: {stderr}"
        );
    }
}

#[test]
fn prices_and_functions_files_must_be_utf8() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    for flag in ["--prices", "--functions"] {
        let bad = scratch.path().join("bad.json");
        fs::write(&bad, [b'{', 0xff, 0xfe, b'}']).unwrap();
        let (code, stdout, stderr) = run(&[flag, bad.to_str().unwrap(), log.to_str().unwrap()]);
        assert_eq!(code, 2, "{flag}");
        assert!(stdout.is_empty(), "{flag}: {stdout}");
        assert!(
            stderr.contains("stream did not contain valid UTF-8"),
            "{flag}: {stderr}"
        );
    }
}

#[test]
fn prices_file_at_the_limit_is_not_refused_for_size() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let big = scratch.path().join("edge.json");
    sparse(&big, 268_435_456);
    let (code, _, stderr) = run(&["--prices", big.to_str().unwrap(), log.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(!stderr.contains("larger than"), "{stderr}");
}

#[test]
fn root_object_with_message_and_events_exits_2() {
    let dir = std::env::temp_dir().join(format!("lb-both-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("both.json");
    std::fs::write(
        &f,
        r#"{"message":"REPORT RequestId: a1 Duration: 9 ms Billed Duration: 9 ms Memory Size: 256 MB Max Memory Used: 20 MB","events":[{"message":"REPORT RequestId: a2 Duration: 9 ms Billed Duration: 9 ms Memory Size: 256 MB Max Memory Used: 20 MB"}]}"#,
    )
    .unwrap();
    let (code, stdout, stderr) = run(&["--function", "worker", f.to_str().unwrap()]);
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("event 1: has both message and events"),
        "{stderr}"
    );
}

const R3: &str = "REPORT RequestId: r3 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 5 MB";

fn json_requests(content: &str) -> (i32, String, String) {
    let dir = Scratch::new();
    let file = dir.path().join("in.log");
    fs::write(&file, content).unwrap();
    run(&["--json", file.to_str().unwrap()])
}

#[test]
fn at_message_line_counts_as_a_request() {
    let text = format!(
        "# function: api\n{R3}\n{{\"@message\":\"2024 REPORT RequestId: r2 Duration: 1 ms Billed Duration: 200 ms Memory Size: 128 MB Max Memory Used: 5 MB\"}}\n"
    );
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 2"), "{stdout}");
    let upper = text.replace("@message", "Message");
    let (code, stdout, _) = json_requests(&upper);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"requests\": 2"), "{stdout}");
}

#[test]
fn envelope_line_is_not_counted_twice() {
    let text = format!("# function: api\n{{\"message\":\"{R3}\"}}\n");
    let (code, stdout, _) = json_requests(&text);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

#[test]
fn note_line_with_report_text_exits_2_with_empty_stdout() {
    let text = format!("# function: api\n{R3}\n{{\"message\":5,\"note\":\"x {R3}\"}}\n");
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("line 3: not a Lambda REPORT line"),
        "{stderr}"
    );
    assert!(!stderr.contains("r3"));
}

#[test]
fn uncounted_report_marker_in_json_or_bracket_line_exits_2() {
    let cases = [
        format!("{{\"note\":\"{R3}\"}}"),
        format!("{{\"@message\":5,\"x\":\"{R3}\"}}"),
        format!("{{\"Message\":\"{R3}\",\"note\":\"{R3}\"}}"),
        format!("[\"{R3}\""),
        format!("[\"{R3}\"] tail"),
    ];
    for case in cases {
        let (code, stdout, stderr) = json_requests(&format!("# function: api\n{R3}\n{case}\n"));
        assert_eq!(code, 2, "{case}: {stderr}");
        assert!(stdout.is_empty(), "{case}");
        assert!(
            stderr.contains("not a Lambda REPORT line") || stderr.contains("line 3: not JSON"),
            "{stderr}"
        );
        assert!(!stderr.contains("r3"), "{stderr}");
    }
}

#[test]
fn array_line_counts_its_request() {
    let text = format!("# function: api\n[{{\"message\":\"{R3}\"}}]\n");
    let (code, stdout, _) = json_requests(&text);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

#[test]
fn start_end_text_and_bad_json_lines() {
    let text = format!(
        "# function: api\n{R3}\nSTART RequestId: r9 Version: $LATEST\nEND RequestId: r9\n{{\"@message\":\"START RequestId: r9\"}}\nplain text\n"
    );
    let (code, stdout, _) = json_requests(&text);
    assert_eq!(code, 0);
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
    let (code, stdout, stderr) = json_requests("# function: api\n{not json\n");
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not JSON"));
}

#[cfg(unix)]
#[test]
fn stdin_read_error_starts_with_dash_colon() {
    let scratch = Scratch::new();
    let mut cmd = command(&["-"]);
    cmd.stdin(Stdio::from(File::open(scratch.path()).unwrap()));
    let (code, stdout, stderr) = output_of(&mut cmd);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("-: "), "{stderr}");
    assert!(!stderr.contains("larger than"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn device_file_is_not_a_file() {
    let (code, stdout, stderr) = run(&["/dev/null"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("/dev/null: not a file"), "{stderr}");
}

#[test]
fn non_object_event_is_not_a_log_event() {
    let scratch = Scratch::new();
    for (name, body) in [
        ("array.json", "[1]"),
        ("events_item.json", "{\"events\":[1]}"),
    ] {
        let file = scratch.path().join(name);
        fs::write(&file, body).unwrap();
        let (code, stdout, stderr) = run(&["--function", "a", file.to_str().unwrap()]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}");
        assert!(stderr.contains("not a log event"), "{name}: {stderr}");
    }
}

#[test]
fn foreign_log_group_is_rejected_without_echo() {
    let scratch = Scratch::new();
    let file = scratch.path().join("group.json");
    let message = report("g1", 128, 1);
    let event = format!(
        "{{\"logGroup\":\"/var/SECRETGROUP\",\"message\":{}}}",
        serde_json_string(&message)
    );
    fs::write(&file, event).unwrap();
    let (code, stdout, stderr) = run(&["--function", "a", file.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("log group is not /aws/lambda/"), "{stderr}");
    assert!(!stderr.contains("SECRETGROUP"), "{stderr}");
}

fn serde_json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[test]
fn headroom_error_texts_name_the_flag_and_the_range() {
    let (code, stdout, stderr) = run(&["--headroom", "101", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("invalid --headroom 101: expected an integer from 0 to 100"),
        "{stderr}"
    );
    let (code, stdout, stderr) = run(&["--headroom", "abc", "examples/sample-report.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("invalid --headroom abc: expected an integer from 0 to 100"),
        "{stderr}"
    );
}

#[test]
fn explicit_file_that_is_not_utf8_is_an_error() {
    let scratch = Scratch::new();
    let bad = scratch.path().join("bad.bin");
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(b"SECRETLEAK");
    fs::write(&bad, bytes).unwrap();
    let (code, stdout, stderr) = run(&[bad.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad.bin: not UTF-8"), "{stderr}");
    assert!(!stderr.contains("SECRETLEAK"));
    assert!(!stderr.contains("skipped"));
}

#[cfg(unix)]
#[test]
fn directory_walk_skips_a_non_regular_file_with_a_warning() {
    let scratch = Scratch::new();
    let walk = scratch.path().join("walk");
    fs::create_dir(&walk).unwrap();
    fs::write(walk.join("good.log"), report("good", 128, 10)).unwrap();
    let made = Command::new("mkfifo").arg(walk.join("pipe.log")).status();
    if !matches!(made, Ok(status) if status.success()) {
        eprintln!("mkfifo unavailable; skipping");
        return;
    }
    let (code, stdout, stderr) = run(&[walk.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("skipped non-regular file"), "{stdout}");
    assert!(stdout.contains("pipe.log"));
    assert!(has_word(&bill_text(&stdout), "128"), "{stdout}");
}

#[test]
fn invisible_filler_in_price_source_is_rejected_without_echo() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        "{\"currency\": \"USD\", \"source\": \"https://example.test/\u{3164}p\", \
         \"date\": \"2026-01-02\", \"region\": \"eu-west-1\", \"tier\": \"second\", \
         \"request_per_million\": \"2\", \"x86_64_per_gb_second\": \"1\", \
         \"arm64_per_gb_second\": \"1\"}",
    )
    .unwrap();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-01-03",
        "--prices",
        prices.to_str().unwrap(),
        log.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad source"));
    assert!(!stderr.contains('\u{3164}'));
}

#[test]
fn unassigned_u2065_in_price_source_is_rejected_without_echo() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        "{\"currency\": \"USD\", \"source\": \"https://example.test/\u{2065}p\", \
         \"date\": \"2026-01-02\", \"region\": \"eu-west-1\", \"tier\": \"second\", \
         \"request_per_million\": \"2\", \"x86_64_per_gb_second\": \"1\", \
         \"arm64_per_gb_second\": \"1\"}",
    )
    .unwrap();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-01-03",
        "--prices",
        prices.to_str().unwrap(),
        log.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad source"));
    assert!(!stderr.contains('\u{2065}'));
}

#[test]
fn hangul_choseong_filler_u115f_in_price_source_is_rejected_without_echo() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(
        &prices,
        "{\"currency\": \"USD\", \"source\": \"https://example.test/\u{115f}p\", \
         \"date\": \"2026-01-02\", \"region\": \"eu-west-1\", \"tier\": \"second\", \
         \"request_per_million\": \"2\", \"x86_64_per_gb_second\": \"1\", \
         \"arm64_per_gb_second\": \"1\"}",
    )
    .unwrap();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-01-03",
        "--prices",
        prices.to_str().unwrap(),
        log.to_str().unwrap(),
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.contains("bad source"));
    assert!(!stderr.contains('\u{115f}'));
}

#[test]
fn invisible_filler_in_a_file_name_is_replaced() {
    let scratch = Scratch::new();
    let log = scratch.path().join("bad\u{3164}name.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let (code, stdout, stderr) = run(&[log.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("api"));
    assert!(!stdout.contains('\u{3164}'));
    assert!(!stderr.contains('\u{3164}'));
}

// ---- usage errors keep their own lines -------------------------------------

/// Lines of stderr that clap wrote itself must stay separate lines, and the
/// message must end with a line break.
fn assert_clap_layout(stderr: &str) {
    assert!(stderr.starts_with("error: "), "{stderr:?}");
    assert!(stderr.ends_with('\n'), "no final line break: {stderr:?}");
    assert!(stderr.lines().count() >= 3, "one line only: {stderr:?}");
    assert!(
        stderr
            .lines()
            .any(|line| line == "For more information, try '--help'."),
        "{stderr:?}"
    );
    assert!(!stderr.contains("\\u{a}\n"), "{stderr:?}");
}

#[test]
fn clap_errors_keep_their_line_breaks() {
    let cases: &[&[&str]] = &[
        &[],
        &["--zz", "examples/sample-report.log"],
        &["--headroom", "abc", "examples/sample-report.log"],
        &["--json=1", "examples/sample-report.log"],
    ];
    for args in cases {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 2, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
        assert_clap_layout(&stderr);
    }
    let (_, _, stderr) = run(&[]);
    let stderr = stderr.replace("\r\n", "\n");
    assert!(
        stderr.contains("\n\nUsage: lambda-bill <FILE>...\n\n"),
        "{stderr:?}"
    );
}

#[test]
fn clap_error_with_escape_and_newline_in_an_argument_has_no_fake_line() {
    let evil = "1\u{1b}[31m\nFAKE: line";
    let option_cases: Vec<Vec<String>> = vec![
        vec!["--headroom".into(), evil.into()],
        vec![format!("--json={evil}")],
        vec!["--x\u{1b}[31m\nFAKE: line".to_string()],
        vec![format!("--headroom={evil}")],
    ];
    for extra in option_cases {
        let mut args: Vec<&str> = extra.iter().map(String::as_str).collect();
        args.push("examples/sample-report.log");
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 2, "{args:?}");
        assert!(stdout.is_empty(), "{args:?}");
        assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
        assert!(
            stderr.contains("\\u{1b}[31m\\u{a}FAKE: line"),
            "{args:?}: {stderr:?}"
        );
        assert!(
            !stderr.lines().any(|line| line.starts_with("FAKE")),
            "{args:?}: {stderr:?}"
        );
        assert_clap_layout(&stderr);
    }
    // Options that clap accepts and the program rejects stay on one line.
    for option in ["--arch", "--function"] {
        let (code, stdout, stderr) = run(&[option, evil, "examples/sample-report.log"]);
        assert_eq!(code, 2, "{option}");
        assert!(stdout.is_empty());
        assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
        assert!(
            !stderr.lines().any(|line| line.starts_with("FAKE")),
            "{option}: {stderr:?}"
        );
    }
}

#[test]
fn clap_error_escapes_cr_bidi_and_invisible_characters_on_one_line() {
    let (code, _, stderr) = run(&[
        "--headroom",
        "7\r\u{202E}\u{200B}\u{2028}",
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2);
    for raw in ['\r', '\u{202E}', '\u{200B}', '\u{2028}'] {
        assert!(!stderr.contains(raw), "{stderr:?}");
    }
    for escaped in ["\\u{d}", "\\u{202e}", "\\u{200b}", "\\u{2028}"] {
        assert!(stderr.contains(escaped), "{escaped}: {stderr:?}");
    }
    assert_clap_layout(&stderr);
}

// ---- N1, N2, N12: what a log may contain -----------------------------------

#[test]
fn invisible_character_before_report_is_an_error_not_a_silent_drop() {
    let first = R3.replace("r3", "a1");
    let second = R3.replace("r3", "a2");
    for hidden in ["\u{feff}", "\u{200b}", "\u{2060}", "\u{a0}", "\u{b}"] {
        let text = format!("# function: api\n{first}\n{hidden}{second}\n");
        let (code, stdout, stderr) = json_requests(&text);
        assert_eq!(
            code,
            2,
            "U+{:04X}: {stdout}",
            hidden.chars().next().unwrap() as u32
        );
        assert!(stdout.is_empty());
        assert!(stderr.contains("line 3: "), "{stderr}");
        assert!(!stderr.contains("a2"));
    }
    // Inside a message of a JSON envelope the same rule holds.
    let text = format!("{{\"message\":\"x\\u200b{R3}\",\"logGroup\":\"/aws/lambda/f\"}}");
    let (code, _, stderr) = json_requests(&text);
    assert_eq!(code, 2, "{stderr}");
}

#[test]
fn message_and_events_in_a_nested_event_is_an_error() {
    let documents = [
        format!("[{{\"message\":\"{R3}\",\"events\":[{{\"message\":\"{R3}\"}}]}}]"),
        format!("{{\"events\":[{{\"message\":\"{R3}\",\"events\":[{{\"message\":\"{R3}\"}}]}}]}}"),
    ];
    for doc in documents {
        let (code, stdout, stderr) = json_requests(&doc);
        assert_eq!(code, 2, "{doc}: {stdout}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("has both message and events"), "{stderr}");
    }
}

#[test]
fn pretty_printed_at_message_document_is_read() {
    let scratch = Scratch::new();
    let file = scratch.path().join("pretty.json");
    fs::write(&file, format!("{{\n  \"@message\": \"2024 {R3}\"\n}}\n")).unwrap();
    let (code, stdout, stderr) = run(&["--json", "--function", "f", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

// ---- standard output that cannot be written --------------------------------

#[cfg(target_os = "linux")]
#[test]
fn read_only_stdout_exits_2() {
    for redirect in ["1<examples/sample-report.log", "1<."] {
        let (code, stderr) = run_shell_redirect(redirect);
        assert_eq!(code, 2, "{redirect}: {stderr}");
        assert!(stderr.contains("could not write output"), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
        let (code, stderr) = run_shell_args("--help", redirect);
        assert_eq!(code, 2, "--help {redirect}: {stderr}");
        assert!(stderr.contains("stdout is not writable"), "{stderr}");
    }
}

// ---- documented limits: pinned so the README stays true --------------------

const PRICES_WITH_MAX_REQUEST_RATE: &str = "{\"currency\":\"USD\",\"source\":\"https://example.test/\",\"date\":\"2026-01-02\",\"region\":\"r\",\"tier\":\"t\",\"request_per_million\":\"79228162514264337593543950335\",\"x86_64_per_gb_second\":\"0\",\"arm64_per_gb_second\":\"0\"}";

#[test]
fn platform_report_records_are_not_read() {
    let text = format!(
        "# function: f\n{R3}\n{{\"time\":\"x\",\"type\":\"platform.report\",\"record\":{{\"requestId\":\"zz\",\"metrics\":{{\"billedDurationMs\":100,\"memorySizeMB\":128}}}}}}\n"
    );
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

#[test]
fn pretty_printed_object_without_a_log_envelope_is_rejected_at_line_1() {
    let scratch = Scratch::new();
    let file = scratch.path().join("pretty.json");
    fs::write(&file, "{\n \"a\": 1\n}\n").unwrap();
    let (code, stdout, stderr) = run(&["--function", "f", file.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains(": line 1: not JSON"), "{stderr}");
}

#[test]
fn widest_request_rate_is_exact_for_one_and_two_requests() {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, PRICES_WITH_MAX_REQUEST_RATE).unwrap();
    let two = scratch.path().join("two.log");
    fs::write(
        &two,
        format!("# function: f\n{R3}\n{}\n", R3.replace("r3", "r4")),
    )
    .unwrap();
    // 2 * (2^96-1) / 1_000_000 is exact at scale 5. The division is not
    // rounded away before it is stored.
    let (code, stdout, stderr) =
        run(&["--prices", prices.to_str().unwrap(), two.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("158456325028528675187087.90067"),
        "{stdout}"
    );
    let one = scratch.path().join("one.log");
    fs::write(&one, format!("# function: f\n{R3}\n")).unwrap();
    let (code, stdout, stderr) =
        run(&["--prices", prices.to_str().unwrap(), one.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.contains("79228162514264337593543.950335"),
        "{stdout}"
    );
}

#[test]
fn a_plain_line_with_a_glued_marker_is_ignored_and_a_json_line_with_one_is_an_error() {
    let glued = R3.replace("r3", "g1");
    let text = format!("# function: f\n{R3}\nx{glued}\n");
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
    let text = format!("# function: f\n{R3}\n[ts] x{glued}\n");
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("not a Lambda REPORT line"), "{stderr}");
}

#[test]
fn a_bracket_prefix_that_is_not_json_stays_text_and_a_cut_array_is_not_json() {
    for prefix in ["[INFO]", "[ERROR]", "[WARNING]", "[main]", "[12:00:01]"] {
        let text = format!("# function: f\n{prefix} start\n{R3}\n{prefix} end\n");
        let (code, stdout, stderr) = json_requests(&text);
        assert_eq!(code, 0, "{prefix}: {stderr}");
        assert!(stdout.contains("\"requests\": 1"), "{prefix}: {stdout}");
    }
    for cut in [
        "[{\"message\":\"x\"}",
        "[{",
        "[",
        "[1,2",
        "[\"a\",\"b\"",
        "[true,false",
        "[[1]",
        "[\r{\"message\":\"x\"}",
        "[ \r{",
        "[\t\r{\"message\":\"x\"}",
        "[\r",
        "[\r1,2",
        "[\r\"a\"",
        "[\rtrue",
    ] {
        let text = format!("# function: f\n{R3}\n{cut}\n");
        let (code, stdout, stderr) = json_requests(&text);
        assert_eq!(code, 2, "{cut}: {stderr}");
        assert!(stdout.is_empty(), "{cut}");
        assert!(stderr.contains("line 3: not JSON"), "{cut}: {stderr}");
    }
}

// ---- a document must not drop a second REPORT in another field -------------

#[test]
fn a_second_report_in_another_field_of_a_document_is_an_error() {
    let r2 = R3.replace("r3", "r2");
    let documents = [
        format!("{{\"message\":\"{R3}\",\"@message\":\"{r2}\",\"logGroup\":\"/aws/lambda/f\"}}"),
        format!("[{{\"message\":\"{R3}\",\"Message\":\"{r2}\",\"logGroup\":\"/aws/lambda/f\"}}]"),
        format!("[{{\"message\":\"{R3}\",\"x\":\"{r2}\",\"logGroup\":\"/aws/lambda/f\"}}]"),
        format!("{{\"events\":[{{\"message\":\"{R3}\",\"detail\":{{\"m\":\"{r2}\"}},\"logGroup\":\"/aws/lambda/f\"}}]}}"),
        format!("{{\"events\":[{{\"message\":\"{R3}\",\"logGroup\":\"/aws/lambda/f\"}}],\"@message\":\"{r2}\"}}"),
        format!("{{\n  \"message\": \"{R3}\",\n  \"@message\": \"{r2}\",\n  \"logGroup\": \"/aws/lambda/f\"\n}}\n"),
    ];
    for doc in documents {
        let (code, stdout, stderr) = json_requests(&doc);
        assert_eq!(code, 2, "{doc}: {stdout}");
        assert!(stdout.is_empty(), "{doc}");
        assert!(stderr.contains("not a Lambda REPORT line"), "{stderr}");
    }
    // One REPORT in one field stays valid.
    let ok = format!("{{\"message\":\"{R3}\",\"logGroup\":\"/aws/lambda/f\",\"x\":\"START\"}}");
    let (code, stdout, stderr) = json_requests(&ok);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

#[test]
fn at_message_document_reads_its_architecture_and_log_group() {
    let scratch = Scratch::new();
    for (key, pretty) in [("@message", false), ("Message", false), ("@message", true)] {
        let body = if pretty {
            format!("{{\n \"{key}\": \"{R3}\",\n \"architecture\": \"arm64\",\n \"logGroup\": \"/aws/lambda/zzz\"\n}}\n")
        } else {
            format!("{{\"{key}\":\"{R3}\",\"architecture\":\"arm64\",\"logGroup\":\"/aws/lambda/zzz\"}}\n")
        };
        let file = scratch.path().join("at.json");
        fs::write(&file, &body).unwrap();
        let (code, stdout, stderr) = run(&["--json", file.to_str().unwrap()]);
        assert_eq!(code, 0, "{body}: {stderr}");
        assert!(stdout.contains("\"function\": \"zzz\""), "{body}: {stdout}");
        assert!(stdout.contains("\"arm64\""), "{body}: {stdout}");
    }
    // A log group that is not a function, or a bad architecture, is an error.
    for body in [
        format!("{{\"@message\":\"{R3}\",\"logGroup\":\"/other/zzz\"}}\n"),
        format!("{{\"@message\":\"{R3}\",\"architecture\":\"riscv\"}}\n"),
    ] {
        let file = scratch.path().join("bad.json");
        fs::write(&file, &body).unwrap();
        let (code, stdout, stderr) = run(&["--function", "f", file.to_str().unwrap()]);
        assert_eq!(code, 2, "{body}: {stdout}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("event 1: "), "{stderr}");
    }
}

// ---- an argument that is not UTF-8 -----------------------------------------

#[cfg(target_os = "linux")]
#[test]
fn clap_error_for_a_non_utf8_argument_keeps_its_lines_and_has_no_fake_line() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    for bytes in [b"a\xff\xfe".to_vec(), b"a\xff\x1b[31m\nFAKE: line".to_vec()] {
        let mut cmd = command(&["--function"]);
        cmd.arg(OsString::from_vec(bytes));
        cmd.arg("examples/sample-report.log");
        let (code, stdout, stderr) = output_of(&mut cmd);
        assert_eq!(code, 2, "{stderr:?}");
        assert!(stdout.is_empty());
        assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
        assert!(
            !stderr.lines().any(|line| line.starts_with("FAKE")),
            "{stderr:?}"
        );
        assert_clap_layout(&stderr);
    }
}

// ---- a size limit must hold before the file is read ------------------------

#[cfg(target_os = "linux")]
#[test]
fn oversized_prices_and_functions_are_refused_without_reading_them() {
    // A cap of 192 MiB of address space is below the size of the file: a
    // program that tried to read it would abort instead of exiting 2.
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    for flag in ["--prices", "--functions"] {
        let big = scratch.path().join("big.json");
        sparse(&big, 268_435_457);
        let out = Command::new("sh")
            .args(["-c", "ulimit -v 196608; exec \"$0\" \"$@\""])
            .arg(env!("CARGO_BIN_EXE_lambda-bill"))
            .args([flag, big.to_str().unwrap(), log.to_str().unwrap()])
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{flag}: {stderr}");
        assert!(
            stderr.contains("file is larger than 268435456 bytes"),
            "{flag}: {stderr}"
        );
    }
}

// ---- documented limits ------------------------------------------------------

#[test]
fn a_repeated_field_after_the_fixed_fields_is_trailing_text_and_the_first_counts() {
    let line = format!("{R3} Billed Duration: 5 ms");
    let (code, stdout, stderr) = json_requests(&format!("# function: f\n{line}\n"));
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"gb_seconds\": \"0.0125\""), "{stdout}");
}

#[test]
fn gb_seconds_quotient_beyond_28_digits_is_an_error() {
    // 1023 * 18446744073709551615 / 1024000 is not an exact 96-bit decimal.
    // It used to be stored as 18428729675200069.631000976563.
    let line = "REPORT RequestId: a Duration: 1 ms Billed Duration: 18446744073709551615 ms Memory Size: 1023 MB Max Memory Used: 5 MB";
    let (code, stdout, stderr) = json_requests(&format!("# function: f\n{line}\n"));
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    assert!(
        !stderr.contains("18428729675200069.631000976563"),
        "{stderr}"
    );
}

#[test]
fn headroom_accepts_a_plus_sign_and_leading_zeros() {
    for value in ["+5", "05"] {
        let (code, stdout, stderr) = run(&["--headroom", value, "examples/sample-report.log"]);
        assert_eq!(code, 0, "{value}: {stderr}");
        assert!(stdout.contains("headroom: 5%"), "{value}: {stdout}");
    }
}

// ---- the marker is searched in decoded text, not in the raw bytes -----------

/// A second request whose marker is written with JSON escapes. The escape
/// forms below all decode to `REPORT RequestId:`.
fn escaped_second_reports() -> Vec<String> {
    let rest =
        " a2 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 5 MB";
    [
        "REPORT\\u0020RequestId:",
        "\\u0052EPORT RequestId:",
        "REPORT RequestId\\u003a",
        "REPORT Request\\u0049d:",
        "REPORT\\u0020RequestId\\u003a",
    ]
    .iter()
    .map(|marker| format!("{marker}{rest}"))
    .collect()
}

#[test]
fn an_escaped_marker_in_a_field_that_is_not_read_is_an_error() {
    for hidden in escaped_second_reports() {
        let documents = [
            format!("{{\"message\":\"{R3}\",\"@message\":\"{hidden}\",\"logGroup\":\"/aws/lambda/f\"}}"),
            format!("{{\"message\":\"{R3}\",\"x\":\"{hidden}\",\"logGroup\":\"/aws/lambda/f\"}}"),
            format!("[{{\"message\":\"{R3}\",\"x\":\"{hidden}\",\"logGroup\":\"/aws/lambda/f\"}}]"),
            format!("{{\"events\":[{{\"message\":\"{R3}\",\"logGroup\":\"/aws/lambda/f\"}}],\"@message\":\"{hidden}\"}}"),
            format!("{{\"@message\":\"{R3}\",\"Message\":\"{hidden}\",\"logGroup\":\"/aws/lambda/f\"}}"),
            format!("{{\"message\":\"{R3}\",\"n\":{{\"deep\":[\"{hidden}\"]}},\"logGroup\":\"/aws/lambda/f\"}}"),
            // A key that decodes to the marker.
            format!("{{\"message\":\"{R3}\",\"{hidden}\":1,\"logGroup\":\"/aws/lambda/f\"}}"),
        ];
        for doc in documents {
            let (code, stdout, stderr) = json_requests(&doc);
            assert_eq!(code, 2, "{doc}: {stdout}");
            assert!(stdout.is_empty(), "{doc}");
            assert!(
                stderr.contains("not a Lambda REPORT line"),
                "{doc}: {stderr}"
            );
        }
        // The same, one JSON object per line.
        let lines = format!(
            "# function: f\n{{\"message\":\"{R3}\"}}\n{{\"message\":\"{R3}\",\"logStreamName\":\"{hidden}\"}}\n"
        );
        let (code, stdout, stderr) = json_requests(&lines);
        assert_eq!(code, 2, "{lines}: {stdout}");
        assert!(stderr.contains("not a Lambda REPORT line"), "{stderr}");
        // A marker the escape writes into the read message is a normal request.
        let read = format!("{{\"message\":\"{hidden}\",\"logGroup\":\"/aws/lambda/f\"}}");
        let (code, stdout, stderr) = json_requests(&read);
        assert_eq!(code, 0, "{read}: {stderr}");
        assert!(stdout.contains("\"requests\": 1"), "{stdout}");
    }
}

// ---- a price or function file must be a regular file -------------------------

#[cfg(unix)]
#[test]
fn a_fifo_as_prices_or_functions_file_is_not_a_file_and_does_not_hang() {
    use std::time::{Duration, Instant};
    let scratch = Scratch::new();
    let pipe = scratch.path().join("pipe");
    let made = Command::new("mkfifo").arg(&pipe).status();
    if !matches!(made, Ok(status) if status.success()) {
        eprintln!("mkfifo unavailable; skipping");
        return;
    }
    let log = scratch.path().join("ok.log");
    fs::write(&log, report("a", 128, 10)).unwrap();
    for option in ["--prices", "--functions"] {
        let mut cmd = command(&[option, pipe.to_str().unwrap(), log.to_str().unwrap()]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        let started = Instant::now();
        let mut finished = false;
        while started.elapsed() < Duration::from_secs(5) {
            if child.try_wait().unwrap().is_some() {
                finished = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !finished {
            let _ = child.kill();
        }
        let out = child.wait_with_output().unwrap();
        assert!(finished, "{option} on a FIFO hung");
        assert_eq!(out.status.code(), Some(2), "{option}");
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr.contains("pipe: not a file"), "{option}: {stderr}");
    }
}

// ---- two different log group names say so ------------------------------------

#[test]
fn two_different_log_group_names_in_one_object_are_named_in_the_error() {
    let doc = format!(
        "{{\"message\":\"{R3}\",\"logGroup\":\"/aws/lambda/f\",\"logGroupName\":\"/aws/lambda/g\"}}"
    );
    let (code, stdout, stderr) = json_requests(&doc);
    assert_eq!(code, 2, "{stdout}");
    assert!(
        stderr.contains("logGroup and logGroupName differ"),
        "{stderr}"
    );
    // A value that is no function stays the old message.
    let bad = format!("{{\"message\":\"{R3}\",\"logGroup\":\"nonsense\"}}");
    let (code, _, stderr) = json_requests(&bad);
    assert_eq!(code, 2);
    assert!(
        stderr.contains("log group is not /aws/lambda/<name>"),
        "{stderr}"
    );
}

#[test]
fn a_product_beyond_28_digits_is_an_error() {
    // 128 MB for 1000 ms is 1/8 GB-second. Times
    // 7922816251426433759354395033 the exact product ends in `.125`
    // (30 digits). It used to be stored as `...379.10`.
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", "7922816251426433759354395033", "1"),
        &[(
            "one.log",
            "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 128 MB Max Memory Used: 5 MB\n",
        )],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    assert!(
        !stderr.contains("990352031428304219919299379.10"),
        "{stderr}"
    );
}

/// 1000 MB for 1 ms at `10^24 + 1` per GB-second.
///
/// `fractions.Fraction`: `(10**24 + 1) / 1024` =
/// `976562500000000000000.0009765625` (31 digits). The same product on arm64.
/// A request rate of `10^-28` is `10^-34` per request (scale 34).
/// An exact `10^-28` for one GB-second stays.
#[test]
fn a_duration_that_needs_more_than_28_digits_exits_2() {
    let rate = "1000000000000000000000001";
    let line = "# lambda-bill-function: f\n# lambda-bill-arch: x86_64\nREPORT RequestId: 11111111-1111-1111-1111-111111111111\tDuration: 1.00 ms\tBilled Duration: 1 ms\tMemory Size: 1000 MB\tMax Memory Used: 40 MB\n";
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", rate, "0.0000133334"),
        &[("r3.log", line)],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    assert!(!stderr.contains("0009766"), "{stderr}");
    assert!(!stderr.contains(rate), "{stderr}");
    let arm = "# lambda-bill-function: f\n# lambda-bill-arch: arm64\nREPORT RequestId: 11111111-1111-1111-1111-111111111111\tDuration: 1.00 ms\tBilled Duration: 1 ms\tMemory Size: 1000 MB\tMax Memory Used: 40 MB\n";
    let (code, stdout, stderr) = run_priced(&prices_with("0.20", "1", rate), &[("r3.log", arm)]);
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.0000000000000000000000000001", "0", "0"),
        &[(
            "one.log",
            "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 5 MB\n",
        )],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    let (code, stdout, stderr) = run_priced(
        &prices_with("0", "0.0000000000000000000000000001", "0"),
        &[(
            "one.log",
            "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 1024 MB Max Memory Used: 900 MB\n",
        )],
    );
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("\"duration_usd\": \"0.0000000000000000000000000001\""),
        "{stdout}"
    );
}

/// Each row fits. The sum is `2^95 + 1/2`, which is not an exact decimal.
/// `2^95` is `39614081257132168796771975168`.
#[test]
fn a_sum_of_exact_rows_that_does_not_fit_exits_2() {
    let (code, stdout, stderr) = run_priced(
        &prices_with("0", "39614081257132168796771975168", "1"),
        &[
            (
                "x.log",
                "# function: fa\n# arch: x86_64\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 1024 MB Max Memory Used: 10 MB\n",
            ),
            (
                "a.log",
                "# function: fb\n# arch: arm64\nREPORT RequestId: b Duration: 1 ms Billed Duration: 1000 ms Memory Size: 512 MB Max Memory Used: 10 MB\n",
            ),
        ],
    );
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
    assert!(
        !stderr.contains("39614081257132168796771975169"),
        "{stderr}"
    );
}

/// The row at `2^90 + 1/2` per GB-second fits (29 digits). Max Memory Used
/// 900 keeps that memory, so the advice amount is the same exact value.
/// Max Memory Used 1024 raises to 1229 MB, and that amount is 39 digits.
#[test]
fn an_advice_amount_that_is_not_exact_exits_2() {
    let rate = "1237940039285380274899124224.5";
    let low = "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 1024 MB Max Memory Used: 900 MB\n";
    let (code, stdout, stderr) = run_priced(&prices_with("0", rate, "0"), &[("low.log", low)]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(
        stdout.contains("\"duration_usd\": \"1237940039285380274899124224.50\""),
        "{stdout}"
    );
    let high = "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 1000 ms Memory Size: 1024 MB Max Memory Used: 1024 MB\n";
    let (code, stdout, stderr) = run_priced(&prices_with("0", rate, "0"), &[("high.log", high)]);
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains("price overflow"), "{stderr}");
}

#[test]
fn billed_duration_and_memory_fields_are_limited_by_their_text_length() {
    let line = |billed: &str, memory: &str| {
        format!(
            "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: {billed} ms Memory Size: {memory} MB Max Memory Used: 5 MB\n"
        )
    };
    // 20 characters are read, whatever their value; 21 are not.
    let (code, stdout, stderr) = json_requests(&line("00000000000000000100", "0000000128"));
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"memory_mb\": 128"), "{stdout}");
    assert!(stdout.contains("\"gb_seconds\": \"0.0125\""), "{stdout}");
    let (code, _, stderr) = json_requests(&line("000000000000000000100", "128"));
    assert_eq!(code, 2);
    assert!(stderr.contains("not a Lambda REPORT line"), "{stderr}");
    let (code, _, _) = json_requests(&line("18446744073709551616", "128"));
    assert_eq!(code, 2);
    let (code, _, _) = json_requests(&line("100", "00000000128"));
    assert_eq!(code, 2);
}

#[test]
fn text_before_a_json_line_makes_it_text_and_its_marker_does_not_count() {
    let json = format!("{{\"message\":\"{R3}\",\"logGroup\":\"/aws/lambda/f\"}}");
    let second = json.replace("r3", "r4");
    let both = format!("{json}\n2026-11-19T10:00:00Z {second}\n");
    let (code, stdout, stderr) = json_requests(&both);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
}

#[test]
fn a_function_header_after_the_first_report_still_names_the_whole_file() {
    let text = format!("{R3}\n# function: f\n");
    let (code, stdout, stderr) = json_requests(&text);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"function\": \"f\""), "{stdout}");
}

#[test]
fn a_duplicate_request_id_with_other_values_is_only_counted_as_a_duplicate() {
    let text = "# function: f\nREPORT RequestId: a Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 5 MB\nREPORT RequestId: a Duration: 1 ms Billed Duration: 999 ms Memory Size: 512 MB Max Memory Used: 5 MB\n";
    let (code, stdout, stderr) = json_requests(text);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("\"requests\": 1"), "{stdout}");
    assert!(stdout.contains("\"memory_mb\": 128"), "{stdout}");
    assert!(
        stdout.contains("skipped 1 duplicate RequestId"),
        "{stdout}{stderr}"
    );
}

/// A JSON array of `[0]` arrays: every `[0]` is five units of the value budget.
fn nested_array_text(items: usize) -> String {
    let mut text = String::with_capacity(items * 4 + 2);
    text.push('[');
    for _ in 0..items {
        text.push_str("[0],");
    }
    text.push_str("[0]]");
    text
}

const BUDGET_ERROR: &str = "more than 4000000 JSON values";

#[test]
fn a_json_document_with_too_many_values_is_refused_before_it_is_built() {
    let scratch = Scratch::new();
    let doc = scratch.path().join("big.json");
    fs::write(&doc, nested_array_text(4_100_000)).unwrap();
    let (code, stdout, stderr) = run(&[doc.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains(BUDGET_ERROR), "{stderr}");
}

/// 1000000 small arrays are 5000000 units: over the budget of 4000000, which
/// keeps one document under about 2 GB and a few seconds.
#[test]
fn the_value_budget_keeps_a_document_under_two_gigabytes() {
    let scratch = Scratch::new();
    let doc = scratch.path().join("mid.json");
    fs::write(&doc, nested_array_text(1_000_000)).unwrap();
    let (code, stdout, stderr) = run(&[doc.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains(BUDGET_ERROR), "{stderr}");
}

#[test]
fn a_small_gzip_file_cannot_expand_into_too_many_values() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let scratch = Scratch::new();
    let doc = scratch.path().join("big.json.gz");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(nested_array_text(4_100_000).as_bytes())
        .unwrap();
    let packed = encoder.finish().unwrap();
    assert!(packed.len() < 100_000, "{}", packed.len());
    fs::write(&doc, packed).unwrap();
    let (code, stdout, stderr) = run(&[doc.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.contains(BUDGET_ERROR), "{stderr}");
}

#[test]
fn a_prices_or_functions_file_with_too_many_values_is_refused() {
    let scratch = Scratch::new();
    let log = scratch.path().join("a.log");
    fs::write(&log, report("a1", 1024, 20)).unwrap();
    let prices = scratch.path().join("prices.json");
    let builtin = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/prices/lambda-ondemand.json"
    ))
    .unwrap();
    let extra = format!("{{\"x\":{},", nested_array_text(4_100_000));
    fs::write(&prices, builtin.replacen('{', &extra, 1)).unwrap();
    let functions = scratch.path().join("functions.json");
    fs::write(
        &functions,
        format!("{{\"x\":{}}}", nested_array_text(4_100_000)),
    )
    .unwrap();
    for (flag, file) in [("--prices", &prices), ("--functions", &functions)] {
        let (code, stdout, stderr) = run(&[flag, file.to_str().unwrap(), log.to_str().unwrap()]);
        assert_eq!(code, 2, "{flag}: {stderr}");
        assert!(stdout.is_empty(), "{flag}: {stdout}");
        assert!(stderr.contains(BUDGET_ERROR), "{flag}: {stderr}");
    }
}

#[test]
fn a_big_text_log_that_starts_with_a_bracket_is_not_counted_as_json() {
    // A bracket timestamp opens a text log; the budget is for JSON only.
    let scratch = Scratch::new();
    let doc = scratch.path().join("a.log");
    let mut text = String::new();
    for index in 0..3_000 {
        text.push_str(&format!(
            "[2026-11-19T10:00:00Z] REPORT RequestId: id-{index} Duration: 1.00 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 5 MB\n"
        ));
    }
    fs::write(&doc, text).unwrap();
    let (code, _, stderr) = run(&["--function", "f", doc.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
}

#[test]
fn a_broken_json_array_line_is_not_json_and_a_valid_one_keeps_its_envelope() {
    // A line that opens an array, is no bracket timestamp and is not JSON must
    // not fall through to the text parser: that drops logGroup and architecture
    // and bills the request to --function on x86_64 without a word.
    let message = "2026-11-19T10:00:00Z REPORT RequestId: a1 Duration: 1.00 ms Billed Duration: 100 ms Memory Size: 512 MB Max Memory Used: 40 MB";
    let element = format!(
        "{{\"message\":\"{message}\",\"logGroup\":\"/aws/lambda/other\",\"architecture\":\"arm64\"}}"
    );
    let scratch = Scratch::new();
    let broken = [
        ("cut", format!("[{element}")),
        ("comma", format!("[{element},]")),
        ("brace", format!("[{}]", &element[..element.len() - 1])),
    ];
    for (name, text) in &broken {
        let path = scratch.path().join(format!("{name}.json"));
        fs::write(&path, format!("{text}\n")).unwrap();
        let (code, stdout, stderr) = run(&["--function", "f", path.to_str().unwrap()]);
        assert_eq!(code, 2, "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}: {stdout}");
        assert!(stderr.contains("line 1: not JSON"), "{name}: {stderr}");
    }
    let valid = scratch.path().join("valid.json");
    fs::write(&valid, format!("[{element}]\n")).unwrap();
    let (code, stdout, stderr) = run(&["--function", "f", valid.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("other"), "{stdout}");
    assert!(stdout.contains("arm64"), "{stdout}");
    assert!(!stdout.contains("x86_64"), "{stdout}");
}

// ---- lam-perf: invisible characters before a header, and the cost per row ---

/// A header glued to an invisible character is not a comment to skip: it
/// would move requests to the wrong architecture or function without a word.
#[test]
fn invisible_character_before_a_header_is_an_error() {
    let first = R3.replace("r3", "a1");
    let second = R3.replace("r3", "b1");
    for hidden in [
        "\u{feff}", "\u{200b}", "\u{2060}", "\u{200d}", "\u{202e}", "\u{b}",
    ] {
        for header in [
            "# lambda-bill-arch: x86_64",
            "# lambda-bill-function: g",
            "# arch: arm64",
            "# function: g",
        ] {
            let text = format!(
                "# lambda-bill-arch: arm64\n# function: f\n{first}\n{hidden}{header}\n{second}\n"
            );
            let (code, stdout, stderr) = json_requests(&text);
            assert_eq!(
                code,
                2,
                "U+{:04X} before {header}: {stdout}",
                hidden.chars().next().unwrap() as u32
            );
            assert!(stdout.is_empty());
            assert!(stderr.contains("line 4: "), "{stderr}");
        }
        // The same character before a JSON line is not dropped either.
        let text = format!(
            "# function: f\n{first}\n{hidden}{{\"message\":\"{second}\",\"logGroup\":\"/aws/lambda/f\"}}\n"
        );
        let (code, stdout, _) = json_requests(&text);
        assert_eq!(code, 2, "{stdout}");
    }
    // One BOM at the very start of the file is still accepted.
    let text = format!("\u{feff}# function: f\n{first}\n");
    let (code, _, stderr) = json_requests(&text);
    assert_eq!(code, 0, "{stderr}");
}

/// Every row (function x architecture x memory) used to cost about 300 us of
/// exact arithmetic. 20000 rows must finish in a wide margin, also in a debug
/// build.
#[test]
fn many_rows_stay_cheap() {
    let scratch = Scratch::new();
    let file = scratch.path().join("many.log");
    let mut text = String::new();
    for index in 0..20_000 {
        text.push_str(&format!(
            "{{\"logGroup\":\"/aws/lambda/f{index}\",\"message\":\"REPORT RequestId: r{index} Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\"}}\n"
        ));
    }
    fs::write(&file, text).unwrap();
    let start = std::time::Instant::now();
    let (code, _, stderr) = run(&["--json", file.to_str().unwrap()]);
    let took = start.elapsed();
    assert_eq!(code, 0, "{stderr}");
    assert!(took.as_secs() < 20, "20000 rows took {took:?}");
}

const ESTIMATE_LINE: &str =
    "Estimate from list prices, not your AWS invoice (free tier, credits, discounts not included)";

#[test]
fn text_output_labels_the_total_as_an_estimate_once() {
    let (code, stdout, stderr) = run(&["examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout.matches(ESTIMATE_LINE).count(), 1, "{stdout}");
    let label: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("estimated_usd="))
        .collect();
    assert_eq!(label, ["estimated_usd=0.00000165833585"], "{stdout}");
    assert!(!stdout.contains("estimated_usd_month"), "{stdout}");
}

#[test]
fn json_output_says_it_is_an_estimate() {
    let (code, stdout, stderr) = run(&["--json", "examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["estimate"], serde_json::Value::Bool(true), "{stdout}");
    assert_eq!(value["estimate_note"], ESTIMATE_LINE, "{stdout}");
    assert_eq!(
        value["totals"]["estimated_usd"], value["totals"]["total_usd"],
        "{stdout}"
    );
}

fn old_prices(date: &str) -> String {
    prices_with("0.20", "0.0000166667", "0.0000133334").replace("2026-01-02", date)
}

fn run_with_price_date(date: &str, as_of: &str, extra: &[&str]) -> (i32, String, String) {
    let scratch = Scratch::new();
    let prices = scratch.path().join("prices.json");
    fs::write(&prices, old_prices(date)).unwrap();
    let prices = prices.to_string_lossy().into_owned();
    let mut args = vec!["--prices", prices.as_str(), "--as-of", as_of];
    args.extend_from_slice(extra);
    args.push("examples/sample-report.log");
    run(&args)
}

#[test]
fn prices_older_than_180_days_warn_on_stderr_and_exit_0() {
    let (code, stdout, stderr) = run_with_price_date("2026-01-01", "2026-11-20", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("2026-01-01"), "{stderr}");
    assert!(stderr.contains("323 days"), "{stderr}");
    assert!(stderr.starts_with("warning: "), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(!stdout.contains("price data is dated"), "{stdout}");
    let (jcode, jout, jerr) = run_with_price_date("2026-01-01", "2026-11-20", &["--json"]);
    assert_eq!(jcode, 0, "{jerr}");
    assert!(jerr.contains("2026-01-01"), "{jerr}");
    assert!(!jout.contains("price data is dated"), "{jout}");
}

#[test]
fn the_price_age_limit_is_exactly_180_days() {
    // 2026-11-09 plus 180 days is 2027-05-08.
    let (code, _, stderr) = run_with_price_date("2026-11-09", "2027-05-08", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "", "180 days old is not older than 180 days");
    let (code, _, stderr) = run_with_price_date("2026-11-09", "2027-05-09", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("181 days"), "{stderr}");
    // A price date after --as-of is not old.
    let (code, _, stderr) = run_with_price_date("2026-11-09", "2026-01-01", &[]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
}

#[test]
fn the_built_in_prices_are_checked_against_as_of() {
    let (code, _, stderr) = run(&["--as-of", "2027-06-01", "examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("2026-11-09"), "{stderr}");
    let (code, _, stderr) = run(&["--as-of", "2026-11-20", "examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
}

#[test]
fn a_bad_as_of_date_exits_2_and_is_escaped() {
    for bad in ["2026-02-30", "yesterday", "2026-11-20\x1b[2J", ""] {
        let (code, stdout, stderr) = run(&["--as-of", bad, "examples/sample-report.log"]);
        assert_eq!(code, 2, "{bad:?}: {stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert!(stderr.contains("--as-of"), "{stderr}");
        assert!(!stderr.contains('\x1b'), "{stderr:?}");
    }
}

fn duplicate_report() -> String {
    let line = report("same-id", 256, 30);
    format!("{line}{}", line.replace("# function: api\n", ""))
}

#[test]
fn help_lists_quiet_and_verbose() {
    let (code, stdout, stderr) = run(&["--help"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("--quiet"), "{stdout}");
    assert!(stdout.contains("--verbose"), "{stdout}");
}

#[test]
fn quiet_with_verbose_is_a_usage_error_with_exit_3() {
    for args in [
        ["--quiet", "--verbose", "examples/sample-report.log"],
        ["--verbose", "--quiet", "examples/sample-report.log"],
    ] {
        let (code, stdout, stderr) = run(&args);
        assert_eq!(code, 3, "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        assert_eq!(stderr.lines().count(), 1, "{stderr:?}");
        assert!(
            stderr.contains("--quiet") && stderr.contains("--verbose"),
            "{stderr}"
        );
    }
}

#[test]
fn quiet_keeps_the_result_and_drops_the_warnings_on_stderr() {
    let log = duplicate_report();
    let (code, stdout, stderr) =
        run_stdin(&["--json", "--as-of", "2026-11-20", "-"], log.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("duplicate RequestId"), "{stderr}");
    let (qcode, qout, qerr) = run_stdin(
        &["--quiet", "--json", "--as-of", "2026-11-20", "-"],
        log.as_bytes(),
    );
    assert_eq!(qcode, 0, "{qerr}");
    assert_eq!(qout, stdout, "quiet does not change the result");
    assert_eq!(qerr, "");
    // The price age warning goes too.
    let (code, _, stderr) = run(&["--as-of", "2030-01-01", "examples/sample-report.log"]);
    assert_eq!(code, 0);
    assert!(stderr.starts_with("warning: "), "{stderr}");
    let (code, _, stderr) = run(&[
        "--quiet",
        "--as-of",
        "2030-01-01",
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
}

#[test]
fn quiet_still_prints_errors() {
    let (code, stdout, stderr) = run(&["--quiet", "examples/does-not-exist.log"]);
    assert_eq!(code, 2);
    assert!(stdout.is_empty(), "{stdout}");
    assert!(stderr.starts_with("lambda-bill: "), "{stderr}");
}

#[test]
fn verbose_adds_progress_on_stderr_and_keeps_stdout() {
    let (code, stdout, stderr) = run(&["--as-of", "2026-11-20", "examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    let (vcode, vout, verr) = run(&[
        "--verbose",
        "--as-of",
        "2026-11-20",
        "examples/sample-report.log",
    ]);
    assert_eq!(vcode, 0, "{verr}");
    assert_eq!(vout, stdout, "verbose does not change the result");
    let lines: Vec<&str> = verr.lines().collect();
    assert_eq!(lines.len(), 2, "{verr:?}");
    assert_eq!(lines[0], "lambda-bill: reading examples/sample-report.log");
    assert_eq!(lines[1], "lambda-bill: done: 1 functions, 2 requests");
    // An error is still the error line, after the progress line.
    let (ecode, eout, eerr) = run(&["--verbose", "examples/does-not-exist.log"]);
    assert_eq!(ecode, 2);
    assert!(eout.is_empty(), "{eout}");
    assert!(
        eerr.starts_with("lambda-bill: reading examples/does-not-exist.log\n"),
        "{eerr}"
    );
    assert!(
        eerr.contains("lambda-bill: examples/does-not-exist.log"),
        "{eerr}"
    );
}

#[test]
fn verbose_escapes_the_path_it_reports() {
    let (code, _, stderr) = run(&["--verbose", "bad\u{1b}[2Jname\nfake.log"]);
    assert_eq!(code, 2);
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
    assert_eq!(stderr.lines().count(), 2, "{stderr:?}");
}

/// The ten value columns of the table rows of the text report, by row name.
fn text_rows(stdout: &str) -> Vec<Vec<String>> {
    stdout
        .lines()
        .filter(|line| {
            line.starts_with("checkout ")
                || line.starts_with("billing ")
                || line.starts_with("total ")
        })
        .map(|line| {
            let mut cells: Vec<String> = line.split_whitespace().map(str::to_string).collect();
            if cells[0] == "total" {
                // The total row leaves four cells empty.
                let mut full = vec![
                    "total".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                ];
                full.extend(cells.drain(1..));
                cells = full;
            }
            cells.truncate(10);
            cells
        })
        .collect()
}

/// The ten value columns of the table rows of the markdown report.
fn markdown_rows(stdout: &str) -> Vec<Vec<String>> {
    stdout
        .lines()
        .filter(|line| {
            line.starts_with("| checkout ")
                || line.starts_with("| billing ")
                || line.starts_with("| total ")
        })
        .map(|line| {
            let mut cells: Vec<String> = line
                .trim_matches('|')
                .split(" | ")
                .map(|cell| cell.trim().to_string())
                .collect();
            cells.truncate(10);
            cells
        })
        .collect()
}

#[test]
fn markdown_amounts_equal_the_text_and_the_json_amounts() {
    let args = ["examples/export"];
    let (code, text, stderr) = run(&args);
    assert_eq!(code, 0, "{stderr}");
    let (mcode, md, merr) = run(&["--md", "examples/export"]);
    assert_eq!(mcode, 0, "{merr}");
    let (jcode, json, jerr) = run(&["--json", "examples/export"]);
    assert_eq!(jcode, 0, "{jerr}");
    let from_text = text_rows(&text);
    let from_md = markdown_rows(&md);
    assert_eq!(from_text.len(), 3, "{text}");
    assert_eq!(from_md, from_text, "{md}");
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    for (row, function) in doc["functions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["billing", "checkout"])
    {
        let cells = from_md.iter().find(|cells| cells[0] == function).unwrap();
        assert_eq!(cells[6], row["gb_seconds"].as_str().unwrap());
        assert_eq!(cells[7], row["duration_usd"].as_str().unwrap());
        assert_eq!(cells[8], row["request_usd"].as_str().unwrap());
        assert_eq!(cells[9], row["total_usd"].as_str().unwrap());
    }
    let total = from_md.iter().find(|cells| cells[0] == "total").unwrap();
    assert_eq!(total[9], doc["totals"]["total_usd"].as_str().unwrap());
    assert!(md.contains(ESTIMATE_LINE), "{md}");
    assert!(md.contains("estimated_usd="), "{md}");
    assert!(md.starts_with("# "), "{md}");
    assert!(!md.contains('\u{1b}'));
}

#[test]
fn markdown_and_json_together_are_a_usage_error() {
    let (code, stdout, stderr) = run(&["--md", "--json", "examples/sample-report.log"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains("--md") && stderr.contains("--json"),
        "{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn markdown_escapes_a_pipe_and_a_line_break_in_a_cell() {
    let scratch = Scratch::new();
    let dir = scratch.path().join("in");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.log"), report("a1", 256, 30)).unwrap();
    symlink(dir.join("a.log"), dir.join("x|y\\|z\nrow.log")).unwrap();
    let (code, stdout, stderr) = run(&["--md", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    let warning = stdout
        .lines()
        .find(|line| line.contains("skipped symlink"))
        .unwrap_or_else(|| panic!("no symlink warning: {stdout}"));
    assert!(warning.contains("x\\|y\\\\\\|z"), "{warning}");
    assert!(!stdout.contains("z\nrow"), "{stdout:?}");
    // Every table line has the same number of cell separators.
    let widths: Vec<usize> = stdout
        .lines()
        .filter(|line| line.starts_with('|'))
        .map(|line| line.replace("\\|", "").matches('|').count())
        .collect();
    assert!(widths.len() >= 4, "{stdout}");
    assert!(
        widths.iter().all(|w| *w == widths[0]),
        "{widths:?}\n{stdout}"
    );
}

/// Wait until the child has a handler for SIGINT (bit 1 of `SigCgt`), so the
/// signal does not arrive before the program is ready for it.
#[cfg(target_os = "linux")]
fn wait_for_sigint_handler(pid: u32) -> bool {
    for _ in 0..500 {
        if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status")) {
            let caught = status
                .lines()
                .find_map(|line| line.strip_prefix("SigCgt:"))
                .and_then(|hex| u64::from_str_radix(hex.trim(), 16).ok())
                .unwrap_or(0);
            if caught & 0b10 != 0 {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
}

#[cfg(target_os = "linux")]
#[test]
fn sigint_exits_130_with_a_message_and_no_output() {
    let mut cmd = command(&["--json", "-"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    // The program is now reading standard input, which stays open.
    let stdin = child.stdin.take().unwrap();
    assert!(
        wait_for_sigint_handler(child.id()),
        "the program never installed a SIGINT handler"
    );
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let out = child.wait_with_output().unwrap();
    drop(stdin);
    assert_eq!(out.status.code(), Some(130), "{:?}", out.status);
    assert!(out.stdout.is_empty(), "{:?}", out.stdout);
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "lambda-bill: interrupted by SIGINT\n"
    );
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn output_writes_the_report_to_a_file_and_nothing_to_stdout() {
    let scratch = Scratch::new();
    let out = scratch.path().join("report.txt");
    let (code, plain, stderr) = run(&["--as-of", "2026-11-20", "examples/sample-report.log"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, stdout, stderr) = run(&[
        "--as-of",
        "2026-11-20",
        "--output",
        out.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
    assert_eq!(fs::read_to_string(&out).unwrap(), plain);
    assert_eq!(
        files_in(scratch.path()),
        ["report.txt"],
        "no temporary file is left"
    );
    // JSON and Markdown go to the file the same way.
    for flag in ["--json", "--md"] {
        let (_, expected, _) = run(&[flag, "examples/sample-report.log"]);
        let (code, stdout, stderr) = run(&[
            flag,
            "--output",
            out.to_str().unwrap(),
            "examples/sample-report.log",
        ]);
        assert_eq!(code, 0, "{stderr}");
        assert_eq!(stdout, "");
        assert_eq!(fs::read_to_string(&out).unwrap(), expected, "{flag}");
    }
    assert_eq!(files_in(scratch.path()), ["report.txt"]);
}

#[test]
fn output_replaces_an_existing_file_and_says_so_in_verbose() {
    let scratch = Scratch::new();
    let out = scratch.path().join("report.txt");
    fs::write(&out, "old report\n").unwrap();
    let (code, stdout, stderr) = run(&[
        "--verbose",
        "--output",
        out.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("replacing"), "{stderr}");
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("prices: "), "{text}");
    assert_eq!(files_in(scratch.path()), ["report.txt"]);
}

#[test]
fn output_in_a_missing_directory_exits_2_and_creates_nothing() {
    let scratch = Scratch::new();
    let out = scratch.path().join("no-such-dir").join("report.txt");
    let (code, stdout, stderr) = run(&[
        "--output",
        out.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.starts_with("lambda-bill: cannot write "), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(!stderr.to_lowercase().contains("panic"), "{stderr}");
    assert!(files_in(scratch.path()).is_empty());
}

#[test]
fn output_to_a_directory_exits_2_and_leaves_no_temporary_file() {
    let scratch = Scratch::new();
    let out = scratch.path().join("target-dir");
    fs::create_dir(&out).unwrap();
    let (code, stdout, stderr) = run(&[
        "--output",
        out.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("not a regular file"), "{stderr}");
    assert_eq!(files_in(scratch.path()), ["target-dir"]);
}

#[cfg(unix)]
#[test]
fn output_does_not_follow_a_symlink() {
    let scratch = Scratch::new();
    let real = scratch.path().join("real.txt");
    fs::write(&real, "keep me\n").unwrap();
    let link = scratch.path().join("link.txt");
    symlink(&real, &link).unwrap();
    let (code, stdout, stderr) = run(&[
        "--output",
        link.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("symlink"), "{stderr}");
    assert_eq!(fs::read_to_string(&real).unwrap(), "keep me\n");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(files_in(scratch.path()), ["link.txt", "real.txt"]);
}

#[cfg(target_os = "linux")]
#[test]
fn output_to_a_device_is_refused() {
    let (code, stdout, stderr) = run(&["--output", "/dev/full", "examples/sample-report.log"]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("not a regular file"), "{stderr}");
}

#[test]
fn a_failed_run_keeps_the_existing_output_file() {
    let scratch = Scratch::new();
    let out = scratch.path().join("report.txt");
    fs::write(&out, "old report\n").unwrap();
    let (code, _, stderr) = run(&[
        "--output",
        out.to_str().unwrap(),
        "examples/does-not-exist.log",
    ]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(fs::read_to_string(&out).unwrap(), "old report\n");
    assert_eq!(files_in(scratch.path()), ["report.txt"]);
}

#[cfg(target_os = "linux")]
#[test]
fn sigint_keeps_the_existing_output_file() {
    let scratch = Scratch::new();
    let out = scratch.path().join("report.txt");
    fs::write(&out, "old report\n").unwrap();
    let mut cmd = command(&["--output", out.to_str().unwrap(), "-"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let stdin = child.stdin.take().unwrap();
    assert!(wait_for_sigint_handler(child.id()));
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let result = child.wait_with_output().unwrap();
    drop(stdin);
    assert_eq!(result.status.code(), Some(130));
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read_to_string(&out).unwrap(), "old report\n");
    assert_eq!(files_in(scratch.path()), ["report.txt"]);
}

fn assert_schema_version_first(json: &str, what: &str) {
    let value: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{what}: {err}: {json}"));
    let first = value.as_object().unwrap().keys().next().unwrap();
    assert_eq!(first, "schema_version", "{what}: first key");
    assert_eq!(value["schema_version"], 1, "{what}");
}

#[test]
fn every_json_output_has_schema_version_as_its_first_key() {
    let scratch = Scratch::new();
    // A file, a directory, a gzip-free document on standard input, a price file
    // and quiet/verbose modes all print the same document shape.
    let cases: [&[&str]; 6] = [
        &["--json", "examples/sample-report.log"],
        &[
            "--json",
            "--arch",
            "arm64",
            "--function",
            "worker",
            "examples/arm-report.log",
        ],
        &["--json", "examples/export"],
        &["--json", "--quiet", "examples/sample-report.log"],
        &["--json", "--verbose", "examples/sample-report.log"],
        &[
            "--json",
            "--headroom",
            "0",
            "--as-of",
            "2030-01-01",
            "examples/sample-report.log",
        ],
    ];
    for args in cases {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 0, "{args:?}: {stderr}");
        assert_schema_version_first(&stdout, &format!("{args:?}"));
    }
    let (code, stdout, stderr) = run_stdin(&["--json", "-"], report("a1", 256, 30).as_bytes());
    assert_eq!(code, 0, "{stderr}");
    assert_schema_version_first(&stdout, "stdin");
    let out = scratch.path().join("report.json");
    let (code, _, stderr) = run(&[
        "--json",
        "--output",
        out.to_str().unwrap(),
        "examples/sample-report.log",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_schema_version_first(&fs::read_to_string(&out).unwrap(), "--output");
    // A price file and a duplicate warning change nothing about the field.
    let (code, stdout, stderr) = run_priced(
        &prices_with("0.20", "0.0000166667", "0.0000133334"),
        &[("a.log", &duplicate_report())],
    );
    assert_eq!(code, 0, "{stderr}");
    assert_schema_version_first(&stdout, "--prices");
}

#[test]
fn a_json_run_that_fails_prints_no_json_at_all() {
    // No error path prints a JSON document, so none can lack the field.
    for args in [
        &["--json", "examples/does-not-exist.log"][..],
        &["--json", "--headroom", "101", "examples/sample-report.log"][..],
        &["--json", "--arch", "sparc", "examples/sample-report.log"][..],
        &["--json", "--md", "examples/sample-report.log"][..],
        &[
            "--json",
            "--quiet",
            "--verbose",
            "examples/sample-report.log",
        ][..],
    ] {
        let (code, stdout, stderr) = run(args);
        assert_ne!(code, 0, "{args:?}");
        assert_eq!(stdout, "", "{args:?}");
        assert!(!stderr.contains("schema_version"), "{args:?}: {stderr}");
    }
}

#[test]
fn all_input_files_together_are_limited_to_the_file_limit() {
    let scratch = Scratch::new();
    let mut args = Vec::new();
    let mut names = Vec::new();
    for n in 0..3 {
        let path = scratch.path().join(format!("part{n}.log"));
        File::create(&path).unwrap().set_len(100_000_000).unwrap();
        names.push(path.to_string_lossy().into_owned());
    }
    for name in &names {
        args.push(name.as_str());
    }
    let (code, stdout, stderr) = run(&args);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("input is larger than 268435456 bytes in total"),
        "{stderr}"
    );
}

#[test]
fn a_gzip_file_with_too_many_members_is_refused() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let scratch = Scratch::new();
    let empty = GzEncoder::new(Vec::new(), Compression::fast())
        .finish()
        .unwrap();
    let mut bytes = Vec::new();
    for _ in 0..1_000_001 {
        bytes.extend_from_slice(&empty);
    }
    let mut real = GzEncoder::new(Vec::new(), Compression::fast());
    real.write_all(report("a1", 512, 100).as_bytes()).unwrap();
    bytes.extend_from_slice(&real.finish().unwrap());
    let path = scratch.path().join("many.log.gz");
    fs::write(&path, &bytes).unwrap();
    let (code, stdout, stderr) = run(&[path.to_str().unwrap()]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("more than 1000000 gzip members"),
        "{stderr}"
    );
}

fn lines_for_functions(count: usize) -> String {
    let mut text = String::new();
    for n in 0..count {
        text.push_str(&format!(
            "{{\"logGroup\":\"/aws/lambda/f{n}\",\"message\":\"REPORT RequestId: r{n} Duration: 10 ms Billed Duration: 100 ms Memory Size: 512 MB Max Memory Used: 100 MB\"}}\n"
        ));
    }
    text
}

#[test]
fn a_run_with_too_many_functions_is_refused() {
    let (code, stdout, stderr) =
        run_stdin(&["--json", "-"], lines_for_functions(100_001).as_bytes());
    assert_eq!(code, 2, "{stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("more than 100000 function and memory rows"),
        "{stderr}"
    );
}

#[test]
fn a_run_with_exactly_the_most_functions_is_accepted() {
    let (code, _stdout, stderr) = run_stdin(&["-"], lines_for_functions(100_000).as_bytes());
    assert_eq!(code, 0, "{stderr}");
}
