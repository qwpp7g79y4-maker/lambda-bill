//! The input shapes the AWS documentation shows, with exact amounts.
//!
//! Sources of the shapes (reproduced from memory of the documentation, no
//! network is used):
//! - the REPORT line, with its tab separators and the optional `Init Duration`
//!   and `XRAY` fields: <https://docs.aws.amazon.com/lambda/latest/dg/monitoring-cloudwatchlogs.html>
//! - the `events` document that `aws logs filter-log-events` prints
//!   (`logStreamName`, `timestamp`, `message`, `ingestionTime`, `eventId`,
//!   `searchedLogStreams`, `nextToken`):
//!   <https://docs.aws.amazon.com/cli/latest/reference/logs/filter-log-events.html>
//! - a gzip export of such text: <https://docs.aws.amazon.com/AmazonCloudWatch/latest/logs/S3ExportTasks.html>
//!
//! Every price is the built-in list (0.0000166667 USD per GB-second on x86_64,
//! 0.0000133334 on arm64, 0.20 USD per million requests). The expected amounts
//! were worked out by hand with exact decimals.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

const R1: &str = "REPORT RequestId: 3e25ade1-3b0f-4b47-8c0e-1c3b1d9a5f01\tDuration: 3.41 ms\tBilled Duration: 4 ms\tMemory Size: 128 MB\tMax Memory Used: 52 MB\tInit Duration: 105.84 ms\t\n";
const R2: &str = "REPORT RequestId: 8f1a9c52-0d3e-4c71-9b0a-6a2f5d7e1c02\tDuration: 0.62 ms\tBilled Duration: 1 ms\tMemory Size: 128 MB\tMax Memory Used: 52 MB\t\n";
const R3: &str = "REPORT RequestId: c47d20b9-5e6f-4a18-8d93-2b7e0f4a6c03\tDuration: 99.10 ms\tBilled Duration: 100 ms\tMemory Size: 512 MB\tMax Memory Used: 211 MB\nXRAY TraceId: 1-67234a5b-0123456789abcdef01234567\tSegmentId: 53995c3f42cd8ad8\tSampled: true\t\n";
const START: &str = "START RequestId: 3e25ade1-3b0f-4b47-8c0e-1c3b1d9a5f01 Version: $LATEST\n";
const END: &str = "END RequestId: 3e25ade1-3b0f-4b47-8c0e-1c3b1d9a5f01\n";

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("documented-shapes-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str], stdin: &[u8]) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_lambda-bill"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("NO_COLOR", "1")
        // Fixed date for the price age check: no dependence on the system clock.
        .args(["--as-of", "2026-11-20"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let written = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().unwrap();
    written.unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn event(message: &str, stream: &str) -> Value {
    serde_json::json!({
        "logStreamName": stream,
        "timestamp": 1_730_455_200_000_u64,
        "message": message,
        "ingestionTime": 1_730_455_200_123_u64,
        "eventId": "38384293748201837364758192837465019283746572819283",
    })
}

fn filter_document(events: &[Value]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "events": events,
        "searchedLogStreams": [],
        "nextToken": "Zm9vYmFyYmF6",
    }))
    .unwrap()
}

fn row<'a>(doc: &'a Value, function: &str, memory: u64) -> &'a Value {
    doc["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["function"] == function && row["memory_mb"] == memory)
        .unwrap_or_else(|| panic!("no row {function} {memory}: {doc}"))
}

/// R1 and R2 at 128 MB: 5 ms, 0.000625 GB-seconds, 0.0000000104166875 USD of
/// duration. R3 at 512 MB: 100 ms, 0.05 GB-seconds, 0.000000833335 USD.
fn check_three_reports(stdout: &str, function: &str) {
    let doc: Value = serde_json::from_str(stdout).unwrap();
    assert_eq!(doc["estimate"], Value::Bool(true));
    let small = row(&doc, function, 128);
    assert_eq!(small["requests"], 2);
    assert_eq!(small["cold_starts"], 1);
    assert_eq!(small["max_memory_used_mb"], 52);
    assert_eq!(small["gb_seconds"], "0.000625");
    assert_eq!(small["duration_usd"], "0.0000000104166875");
    assert_eq!(small["request_usd"], "0.0000004");
    assert_eq!(small["total_usd"], "0.0000004104166875");
    let big = row(&doc, function, 512);
    assert_eq!(big["requests"], 1);
    assert_eq!(big["cold_starts"], 0);
    assert_eq!(big["gb_seconds"], "0.05");
    assert_eq!(big["duration_usd"], "0.000000833335");
    assert_eq!(big["total_usd"], "0.000001033335");
    let totals = &doc["totals"];
    assert_eq!(totals["requests"], 3);
    assert_eq!(totals["cold_starts"], 1);
    assert_eq!(totals["gb_seconds"], "0.050625");
    assert_eq!(totals["duration_usd"], "0.0000008437516875");
    assert_eq!(totals["request_usd"], "0.0000006");
    assert_eq!(totals["total_usd"], "0.0000014437516875");
}

#[test]
fn filter_log_events_output_is_priced_exactly() {
    let stream_a = "2026/11/01/[$LATEST]5e1f3c9a8b7d4e6f9a0b1c2d3e4f5a6b";
    let stream_b = "2026/11/01/[$LATEST]0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d";
    let document = filter_document(&[
        event(START, stream_a),
        event(R1, stream_a),
        event(END, stream_a),
        event(R2, stream_a),
        event(R3, stream_b),
    ]);
    let (code, stdout, stderr) = run(
        &["--json", "--function", "order-api", "-"],
        document.as_bytes(),
    );
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stderr, "");
    check_three_reports(&stdout, "order-api");
}

#[test]
fn the_same_events_as_plain_lines_give_the_same_amounts() {
    // `aws logs tail` and a console copy: a timestamp, a stream name with
    // brackets and a `$`, then the message with its tabs.
    let mut text = String::new();
    for (stamp, stream, message) in [
        (
            "2026-11-01T10:00:00.123000+00:00",
            "2026/11/01/[$LATEST]5e1f3c9a",
            START,
        ),
        (
            "2026-11-01T10:00:00.140000+00:00",
            "2026/11/01/[$LATEST]5e1f3c9a",
            R1,
        ),
        (
            "2026-11-01T10:00:00.150000+00:00",
            "2026/11/01/[$LATEST]5e1f3c9a",
            END,
        ),
        (
            "2026-11-01T10:00:02.000000+00:00",
            "2026/11/01/[$LATEST]5e1f3c9a",
            R2,
        ),
        (
            "2026-11-01T10:00:03.000000+00:00",
            "2026/11/01/[$LATEST]0a9b8c7d",
            R3,
        ),
    ] {
        text.push_str(&format!("{stamp} {stream} {message}"));
    }
    let (code, stdout, stderr) = run(&["--json", "--function", "order-api", "-"], text.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    check_three_reports(&stdout, "order-api");
    // CRLF line endings and the bare message lines are the same bill.
    let bare = format!("{START}{R1}{END}{R2}{R3}").replace('\n', "\r\n");
    let (code, stdout, stderr) = run(&["--json", "--function", "order-api", "-"], bare.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    check_three_reports(&stdout, "order-api");
}

#[test]
fn a_gzip_export_is_priced_like_the_text() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let text = format!("{START}{R1}{END}{R2}{R3}");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    let bytes = encoder.finish().unwrap();
    let (code, stdout, stderr) = run(&["--json", "--function", "order-api", "-"], &bytes);
    assert_eq!(code, 0, "{stderr}");
    check_three_reports(&stdout, "order-api");
}

#[test]
fn log_group_names_with_special_characters_name_the_functions() {
    // Letters in both cases, digits, `-` and `_`, with and without the
    // leading slash and with a trailing slash.
    let group_a = "/aws/lambda/Billing-Service_v2";
    let group_b = "aws/lambda/checkout_2/";
    let mut a = event(R1, "2026/11/01/[$LATEST]aa");
    a["logGroupName"] = Value::String(group_a.to_string());
    let mut b = event(R2, "2026/11/01/[$LATEST]bb");
    b["logGroup"] = Value::String(group_b.to_string());
    let mut c = event(R3, "2026/11/01/[$LATEST]bb");
    c["logGroup"] = Value::String(group_b.to_string());
    let document = filter_document(&[a, b, c]);
    let (code, stdout, stderr) = run(&["--json", "-"], document.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    let doc: Value = serde_json::from_str(&stdout).unwrap();
    let first = row(&doc, "Billing-Service_v2", 128);
    assert_eq!(first["requests"], 1);
    assert_eq!(first["cold_starts"], 1);
    assert_eq!(first["gb_seconds"], "0.0005");
    assert_eq!(first["duration_usd"], "0.00000000833335");
    assert_eq!(first["total_usd"], "0.00000020833335");
    let second_small = row(&doc, "checkout_2", 128);
    assert_eq!(second_small["gb_seconds"], "0.000125");
    assert_eq!(second_small["total_usd"], "0.0000002020833375");
    let second_big = row(&doc, "checkout_2", 512);
    assert_eq!(second_big["total_usd"], "0.000001033335");
    assert_eq!(doc["totals"]["requests"], 3);
    assert_eq!(doc["totals"]["total_usd"], "0.0000014437516875");
}

#[test]
fn a_log_group_that_is_not_a_function_is_refused() {
    let mut a = event(R1, "2026/11/01/[$LATEST]aa");
    a["logGroupName"] = Value::String("/aws/lambda/bad name!".to_string());
    let (code, stdout, stderr) = run(&["--json", "-"], filter_document(&[a]).as_bytes());
    assert_eq!(code, 2);
    assert_eq!(stdout, "");
    assert!(
        stderr.contains("log group is not /aws/lambda/<name>"),
        "{stderr}"
    );
}

#[test]
fn optional_fields_may_be_missing_from_the_report_and_the_event() {
    // No Init Duration, no XRAY line, no timestamp or ingestion time in the
    // event, a fractional Duration and a single space between the fields.
    let report = "REPORT RequestId: 11111111-2222-3333-4444-555555555555 Duration: 12.5 ms Billed Duration: 13 ms Memory Size: 1024 MB Max Memory Used: 700 MB";
    let document = serde_json::json!({ "events": [{ "message": report }] }).to_string();
    let (code, stdout, stderr) = run(&["--json", "--function", "solo", "-"], document.as_bytes());
    assert_eq!(code, 0, "{stderr}");
    let doc: Value = serde_json::from_str(&stdout).unwrap();
    let only = row(&doc, "solo", 1024);
    assert_eq!(only["cold_starts"], 0);
    assert_eq!(only["gb_seconds"], "0.013");
    assert_eq!(only["duration_usd"], "0.0000002166671");
    assert_eq!(only["total_usd"], "0.0000004166671");
}

#[test]
fn an_arm64_function_comes_from_the_functions_file() {
    let dir = scratch("arm");
    let map = dir.join("functions.json");
    fs::write(&map, r#"{"order-api": "arm64"}"#).unwrap();
    let report = "REPORT RequestId: 99999999-aaaa-bbbb-cccc-dddddddddddd\tDuration: 2400.00 ms\tBilled Duration: 2500 ms\tMemory Size: 1024 MB\tMax Memory Used: 300 MB\n";
    let (code, stdout, stderr) = run(
        &[
            "--json",
            "--function",
            "order-api",
            "--functions",
            map.to_str().unwrap(),
            "-",
        ],
        report.as_bytes(),
    );
    assert_eq!(code, 0, "{stderr}");
    let doc: Value = serde_json::from_str(&stdout).unwrap();
    let only = row(&doc, "order-api", 1024);
    assert_eq!(only["architecture"], "arm64");
    assert_eq!(only["gb_seconds"], "2.5");
    assert_eq!(only["duration_usd"], "0.0000333335");
    assert_eq!(only["total_usd"], "0.0000335335");
}

#[test]
fn empty_lists_and_pages_without_a_report_are_handled() {
    let dir = scratch("empty");
    // A page with no events is an error on its own: nothing to price.
    let (code, stdout, stderr) = run(
        &["--function", "order-api", "-"],
        br#"{"events": [], "searchedLogStreams": []}"#,
    );
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("no REPORT lines"), "{stderr}");
    // A folder of exported pages: a page with no events and a page with only
    // START and END lines add nothing and are skipped, not an error.
    fs::write(
        dir.join("page-1.json"),
        r#"{"events": [], "searchedLogStreams": []}"#,
    )
    .unwrap();
    fs::write(
        dir.join("page-2.json"),
        filter_document(&[event(START, "s"), event(END, "s")]),
    )
    .unwrap();
    fs::write(dir.join("page-3.log"), format!("{R1}{R2}{R3}")).unwrap();
    let (code, stdout, stderr) = run(
        &["--json", "--function", "order-api", dir.to_str().unwrap()],
        b"",
    );
    assert_eq!(code, 0, "{stderr}");
    check_three_reports(&stdout, "order-api");
}

#[test]
fn pages_that_overlap_count_a_request_once() {
    let dir = scratch("pages");
    let first = dir.join("page-1.json");
    let second = dir.join("page-2.json");
    fs::write(&first, filter_document(&[event(R1, "s"), event(R2, "s")])).unwrap();
    fs::write(&second, filter_document(&[event(R2, "s"), event(R3, "t")])).unwrap();
    let (code, stdout, stderr) = run(
        &[
            "--json",
            "--function",
            "order-api",
            first.to_str().unwrap(),
            second.to_str().unwrap(),
        ],
        b"",
    );
    assert_eq!(code, 0, "{stderr}");
    check_three_reports(&stdout, "order-api");
    assert!(stderr.contains("skipped 1 duplicate RequestId"), "{stderr}");
}
