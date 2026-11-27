//! Each `C<n>` sentence in README.md "How it counts" has a test here,
//! except C21, which lives next to the gzip decoder in `src/parse.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use lambda_bill::{
    advise, bill_from_inputs, duration_usd, execute_paths, format_usd, gb_seconds, load_prices_str,
    prices_builtin, render, request_usd, Advice, Arch, Error, NamedBytes, Options, ADVICE_NOTE,
    FREE_TIER_GB_SECONDS, FREE_TIER_LINE, FREE_TIER_REQUESTS, MAX_LINE_BYTES, PC_LINE,
};

fn checkout() -> Options {
    Options {
        default_function: Some("checkout".to_string()),
        ..Options::default()
    }
}

fn bill_named(name: &str, text: &str, options: &Options) -> Result<lambda_bill::Bill, Error> {
    bill_from_inputs(
        &[NamedBytes {
            name: name.to_string(),
            bytes: text.as_bytes().to_vec(),
            strict: true,
        }],
        options,
    )
}

fn bill(text: &str, options: &Options) -> lambda_bill::Bill {
    bill_named("f.log", text, options).unwrap()
}

fn line(id: &str, duration: &str, billed: u64, memory: u32, max_used: u32) -> String {
    format!(
        "REPORT RequestId: {id} Duration: {duration} ms Billed Duration: {billed} ms Memory Size: {memory} MB Max Memory Used: {max_used} MB\n"
    )
}

fn line_init(id: &str, duration: &str, billed: u64, memory: u32, max_used: u32) -> String {
    format!(
        "REPORT RequestId: {id} Duration: {duration} ms Billed Duration: {billed} ms Memory Size: {memory} MB Max Memory Used: {max_used} MB Init Duration: 50.5 ms\n"
    )
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("lambda-bill-claims-{}-{n}", std::process::id()));
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

#[test]
fn claim_c1_one_report_line_is_one_request() {
    let text = format!(
        "START RequestId: abc Version: $LATEST\n{{\"level\":\"info\",\"msg\":\"hello\"}}\nEND RequestId: abc\n{}{}",
        line("a1", "1", 10, 128, 8),
        line_init("a2", "1", 10, 128, 8),
    );
    let got = bill(&text, &checkout());
    assert_eq!(got.rows.len(), 1);
    assert_eq!(got.rows[0].requests, 2);
    assert_eq!(got.rows[0].cold_starts, 1);
    assert_eq!(got.totals.requests, 2);
}

#[test]
fn claim_c2_gb_seconds_are_memory_times_billed_over_1024_over_1000() {
    assert_eq!(gb_seconds(512, 151).unwrap().to_string(), "0.0755");
    assert_eq!(gb_seconds(128, 1).unwrap().to_string(), "0.000125");
    let text = format!(
        "{}{}",
        line("a1", "100.00", 100, 512, 40),
        line("a2", "50.10", 51, 512, 64)
    );
    let got = bill(&text, &checkout());
    assert_eq!(got.rows[0].gb_seconds.to_string(), "0.0755");
    assert_eq!(got.totals.gb_seconds.to_string(), "0.0755");
}

#[test]
fn claim_c3_duration_field_is_not_used_for_the_cost() {
    let low = bill(&line("a1", "1.00", 10, 256, 20), &checkout());
    let high = bill(&line("a1", "99999.99", 10, 256, 20), &checkout());
    assert_eq!(low.rows[0].gb_seconds, high.rows[0].gb_seconds);
    assert_eq!(low.rows[0].duration_usd, high.rows[0].duration_usd);
    assert_eq!(low.rows[0].total_usd, high.rows[0].total_usd);
}

#[test]
fn claim_c4_init_duration_is_not_added_to_billed_duration() {
    let cold = bill(&line_init("a1", "1", 100, 128, 8), &checkout());
    let warm = bill(&line("a1", "1", 100, 128, 8), &checkout());
    assert_eq!(cold.rows[0].cold_starts, 1);
    assert_eq!(warm.rows[0].cold_starts, 0);
    assert_eq!(cold.rows[0].gb_seconds, warm.rows[0].gb_seconds);
    assert_eq!(cold.rows[0].duration_usd, warm.rows[0].duration_usd);
    // Init Duration counts only when it is the next field after Max Memory Used.
    let buried = "REPORT RequestId: a1 Duration: 1 ms Billed Duration: 100 ms Memory Size: 128 MB Max Memory Used: 8 MB XRAY TraceId: 1 Init Duration: 50000 ms\n";
    let got = bill(buried, &checkout());
    assert_eq!(got.rows[0].cold_starts, 0);
    assert_eq!(got.rows[0].gb_seconds, warm.rows[0].gb_seconds);
}

#[test]
fn claim_c5_requests_are_twenty_cents_per_million_on_both_architectures() {
    let prices = prices_builtin();
    assert_eq!(format_usd(prices.request_per_million), "0.20");
    assert_eq!(
        format_usd(request_usd(1_000_000, prices.request_per_million).unwrap()),
        "0.20"
    );
    assert_eq!(
        format_usd(request_usd(3_000_000, prices.request_per_million).unwrap()),
        "0.60"
    );
    let text = line("a1", "1", 10, 128, 8);
    let mut x86 = checkout();
    x86.default_arch = Arch::X86_64;
    let mut arm = checkout();
    arm.default_arch = Arch::Arm64;
    let x86_bill = bill(&text, &x86);
    let arm_bill = bill(&text, &arm);
    assert_eq!(x86_bill.rows[0].request_usd, arm_bill.rows[0].request_usd);
    assert!(x86_bill.rows[0].duration_usd > arm_bill.rows[0].duration_usd);
}

#[test]
fn claim_c6_x86_duration_rate_is_the_pricing_page_example() {
    let prices = prices_builtin();
    assert_eq!(format_usd(prices.x86_64_per_gb_second), "0.0000166667");
    // 1024 MB for 1000 ms is one GB-second, so the cost is the rate itself.
    assert_eq!(
        format_usd(duration_usd(1024, 1000, prices.x86_64_per_gb_second).unwrap()),
        "0.0000166667"
    );
    // Pricing-page shape: 3e6 requests of 120 ms at 1536 MB is 540000 GB-seconds.
    assert_eq!(gb_seconds(1536, 360_000_000).unwrap().to_string(), "540000");
    assert_eq!(
        format_usd(duration_usd(1536, 360_000_000, prices.x86_64_per_gb_second).unwrap()),
        "9.000018"
    );
}

#[test]
fn claim_c7_arm_rate_is_not_an_exact_twenty_percent_discount() {
    let prices = prices_builtin();
    assert_eq!(format_usd(prices.arm64_per_gb_second), "0.0000133334");
    assert!(prices.arm64_per_gb_second < prices.x86_64_per_gb_second);
    // 10000 GB-seconds. 80% of the x86 cost 0.166667 would be 0.1333336.
    let x86 = duration_usd(1024, 10_000_000, prices.x86_64_per_gb_second).unwrap();
    let arm = duration_usd(1024, 10_000_000, prices.arm64_per_gb_second).unwrap();
    assert_eq!(format_usd(x86), "0.166667");
    assert_eq!(format_usd(arm), "0.133334");
    assert_ne!(format_usd(arm), "0.1333336");
}

#[test]
fn claim_c8_every_report_names_source_date_region_and_tier() {
    let prices = prices_builtin();
    assert_eq!(prices.currency, "USD");
    assert_eq!(prices.source, "https://aws.amazon.com/lambda/pricing/");
    assert_eq!(prices.date, "2026-11-09");
    assert_eq!(prices.region, "us-east-1");
    assert_eq!(prices.tier, "first");
    let text = render(&bill(&line("a1", "1", 10, 128, 8), &checkout()), false);
    assert!(text.contains(
        "prices: USD, source https://aws.amazon.com/lambda/pricing/, date 2026-11-09, region us-east-1, tier first"
    ));
}

#[test]
fn claim_c9_free_tier_is_not_subtracted() {
    assert_eq!(FREE_TIER_REQUESTS, 1_000_000);
    assert_eq!(FREE_TIER_GB_SECONDS, 400_000);
    let prices = prices_builtin();
    let duration = duration_usd(1024, 400_000_000, prices.x86_64_per_gb_second).unwrap();
    let requests = request_usd(FREE_TIER_REQUESTS, prices.request_per_million).unwrap();
    assert_eq!(format_usd(duration), "6.66668");
    assert_eq!(format_usd(requests), "0.20");
    assert_eq!(format_usd(duration + requests), "6.86668");
    let got = bill(&line("a1", "1", 10, 128, 8), &checkout());
    assert_ne!(format_usd(got.totals.total_usd), "0.00");
    let text = render(&got, false);
    assert!(text.contains(FREE_TIER_LINE));
    assert!(text.contains("1000000 requests"));
    assert!(text.contains("400000 GB-seconds per month"));
    let json = render(&got, true);
    assert!(json.contains("\"free_tier_applied\": false"));
}

#[test]
fn claim_c10_provisioned_concurrency_is_not_priced() {
    let got = bill(&line("a1", "1", 10, 128, 8), &checkout());
    let text = render(&got, false);
    let json = render(&got, true);
    assert!(text.contains(PC_LINE));
    assert!(json.contains("\"provisioned_concurrency_included\": false"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let price_file = fs::read_to_string(root.join("prices/lambda-ondemand.json")).unwrap();
    for needle in ["0.0000097222", "0.0000041667"] {
        assert!(!text.contains(needle), "{needle} in text");
        assert!(!json.contains(needle), "{needle} in json");
        assert!(!price_file.contains(needle), "{needle} in price file");
        for entry in fs::read_dir(root.join("src")).unwrap() {
            let path = entry.unwrap().path();
            let source = fs::read_to_string(&path).unwrap();
            assert!(!source.contains(needle), "{needle} in {}", path.display());
        }
    }
}

#[test]
fn claim_c11_downsize_uses_ceil_of_max_used_and_floors_at_128() {
    assert_eq!(advise(256, 200, 20), Advice::Downsize { memory_mb: 240 });
    assert_eq!(advise(512, 40, 20), Advice::Downsize { memory_mb: 128 });
    assert_eq!(advise(512, 64, 20), Advice::Downsize { memory_mb: 128 });
    assert_eq!(advise(512, 400, 0), Advice::Downsize { memory_mb: 400 });
    assert_eq!(advise(512, 400, 100), Advice::Keep);
    assert_eq!(advise(10_240, 100, 20), Advice::Downsize { memory_mb: 128 });
    let mut options = checkout();
    options.headroom_percent = 20;
    let got = bill(&line("a1", "1", 100, 256, 200), &options);
    assert_eq!(got.rows[0].advice, Advice::Downsize { memory_mb: 240 });
    assert!(render(&got, false).contains("downsize to 240 MB"));
}

#[test]
fn claim_c12_raise_uses_configured_memory_at_ninety_percent() {
    assert_eq!(advise(1000, 899, 20), Advice::Keep);
    assert_eq!(advise(1000, 900, 20), Advice::Raise { memory_mb: 1200 });
    assert_eq!(advise(256, 230, 20), Advice::Keep);
    // 231/256 is at least 90%. The base is 256, not 231 (that would be 278).
    assert_eq!(advise(256, 231, 20), Advice::Raise { memory_mb: 308 });
    assert_eq!(advise(128, 200, 20), Advice::Raise { memory_mb: 154 });
    assert_eq!(advise(10_240, 10_000, 20), Advice::Keep);
    // 117 of 129 is at least 90 percent. Raise to 136 wins over a downsize to 128.
    assert_eq!(advise(129, 117, 5), Advice::Raise { memory_mb: 136 });
    let got = bill(&line("a1", "1", 1000, 1000, 900), &checkout());
    assert_eq!(got.rows[0].advice, Advice::Raise { memory_mb: 1200 });
    assert!(render(&got, false).contains("raise to 1200 MB"));
    assert!(!render(&got, false).contains("raise to 1080 MB"));
}

#[test]
fn claim_c13_advice_is_not_a_forecast() {
    let prices = prices_builtin();
    let down = bill(&line("a1", "1", 100, 256, 200), &checkout());
    assert_eq!(
        format_usd(down.rows[0].duration_usd_if_duration_stays),
        format_usd(duration_usd(240, 100, prices.x86_64_per_gb_second).unwrap())
    );
    let text = render(&down, false);
    assert!(text.contains(ADVICE_NOTE));
    assert!(text.contains("if billed duration stays the same"));
    assert!(text.contains("not a forecast"));
    let json = render(&down, true);
    assert!(json.contains(ADVICE_NOTE));
    let keep = bill(&line("a1", "1", 10, 128, 20), &checkout());
    assert_eq!(keep.rows[0].advice, Advice::Keep);
    assert_eq!(
        keep.rows[0].duration_usd_if_duration_stays,
        keep.rows[0].duration_usd
    );
    assert!(!render(&keep, false)
        .lines()
        .any(|line| line.starts_with("advice ")));
}

#[test]
fn claim_c14_one_gb_is_1024_mb() {
    assert_eq!(gb_seconds(1024, 1000).unwrap().to_string(), "1");
    assert_ne!(gb_seconds(1000, 1000).unwrap().to_string(), "1");
    let got = bill(&line("a1", "1", 1000, 1024, 100), &checkout());
    assert_eq!(got.rows[0].gb_seconds.to_string(), "1");
}

#[test]
fn claim_c15_a_row_is_function_architecture_and_memory() {
    let text = "\
{\"logGroup\":\"/aws/lambda/alpha\",\"architecture\":\"arm64\",\"message\":\"REPORT RequestId: a1 Duration: 1 ms Billed Duration: 10 ms Memory Size: 128 MB Max Memory Used: 10 MB\"}
{\"logGroup\":\"/aws/lambda/alpha\",\"architecture\":\"arm64\",\"message\":\"REPORT RequestId: a2 Duration: 1 ms Billed Duration: 10 ms Memory Size: 128 MB Max Memory Used: 40 MB\"}
{\"logGroup\":\"/aws/lambda/alpha\",\"architecture\":\"arm64\",\"message\":\"REPORT RequestId: a3 Duration: 1 ms Billed Duration: 10 ms Memory Size: 256 MB Max Memory Used: 10 MB\"}
{\"logGroup\":\"/aws/lambda/alpha\",\"architecture\":\"x86_64\",\"message\":\"REPORT RequestId: a4 Duration: 1 ms Billed Duration: 10 ms Memory Size: 256 MB Max Memory Used: 10 MB\"}
{\"logGroup\":\"/aws/lambda/zeta\",\"architecture\":\"arm64\",\"message\":\"REPORT RequestId: a5 Duration: 1 ms Billed Duration: 10 ms Memory Size: 512 MB Max Memory Used: 10 MB\"}
";
    let got = bill(text, &Options::default());
    let keys: Vec<_> = got
        .rows
        .iter()
        .map(|row| {
            (
                row.function.as_str(),
                row.architecture,
                row.memory_mb,
                row.requests,
                row.max_memory_used_mb,
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            ("alpha", Arch::Arm64, 128, 2, 40),
            ("alpha", Arch::Arm64, 256, 1, 10),
            ("alpha", Arch::X86_64, 256, 1, 10),
            ("zeta", Arch::Arm64, 512, 1, 10),
        ]
    );
}

#[test]
fn claim_c16_request_id_is_counted_once_and_the_first_copy_wins() {
    let first = line("same-id", "1", 10, 128, 11);
    let second = line("same-id", "1", 99, 512, 400);
    let third = line("same-id", "1", 70, 256, 80);
    let options = checkout();
    let inputs = [
        NamedBytes {
            name: "a.log".to_string(),
            bytes: first.into_bytes(),
            strict: true,
        },
        NamedBytes {
            name: "b.log".to_string(),
            bytes: second.clone().into_bytes(),
            strict: true,
        },
        NamedBytes {
            name: "c.log".to_string(),
            bytes: third.into_bytes(),
            strict: true,
        },
    ];
    let got = bill_from_inputs(&inputs, &options).unwrap();
    assert_eq!(got.rows.len(), 1);
    assert_eq!(got.rows[0].memory_mb, 128);
    assert_eq!(got.rows[0].max_memory_used_mb, 11);
    assert_eq!(got.rows[0].requests, 1);
    assert_eq!(
        got.warnings,
        vec!["skipped 2 duplicate RequestIds".to_string()]
    );
    let text = render(&got, false);
    assert!(text.contains("warning: skipped 2 duplicate RequestIds"));
    assert!(!text.contains("same-id"));

    let reversed = [
        NamedBytes {
            name: "b.log".to_string(),
            bytes: second.into_bytes(),
            strict: true,
        },
        NamedBytes {
            name: "a.log".to_string(),
            bytes: line("same-id", "1", 10, 128, 11).into_bytes(),
            strict: true,
        },
    ];
    let flipped = bill_from_inputs(&reversed, &options).unwrap();
    assert_eq!(flipped.rows[0].memory_mb, 512);
    assert_eq!(
        flipped.warnings,
        vec!["skipped 1 duplicate RequestId".to_string()]
    );

    // The key is (function, request id). The same id on another function counts.
    let split = bill_from_inputs(
        &[
            NamedBytes {
                name: "a.log".to_string(),
                bytes: line("same-id", "1", 10, 128, 8).into_bytes(),
                strict: true,
            },
            NamedBytes {
                name: "b.log".to_string(),
                bytes: format!("# function: other\n{}", line("same-id", "1", 10, 256, 8))
                    .into_bytes(),
                strict: true,
            },
        ],
        &options,
    )
    .unwrap();
    assert_eq!(split.rows.len(), 2);
    assert!(split.warnings.is_empty());

    // Directory members are read in sorted path order, not creation order.
    let scratch = Scratch::new();
    fs::write(
        scratch.path().join("z.log"),
        "# function: pay\nREPORT RequestId: samerid Duration: 1 ms Billed Duration: 99 ms Memory Size: 512 MB Max Memory Used: 400 MB\n",
    )
    .unwrap();
    fs::write(
        scratch.path().join("a.log"),
        "# function: pay\nREPORT RequestId: samerid Duration: 1 ms Billed Duration: 10 ms Memory Size: 128 MB Max Memory Used: 11 MB\n",
    )
    .unwrap();
    let out = execute_paths(
        std::slice::from_ref(&scratch.path().to_path_buf()),
        &Options::default(),
    )
    .unwrap();
    let bill = out
        .stdout
        .lines()
        .filter(|line| !line.starts_with("warning:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(out.stdout.contains("0.00125"));
    assert!(!bill.contains("0.0495"));
    assert!(!bill.split_whitespace().any(|word| word == "512"));
    assert!(!bill.contains("samerid"));
    assert!(out
        .warnings
        .iter()
        .any(|warning| warning == "skipped 1 duplicate RequestId"));
}

#[test]
fn claim_c17_no_aws_sdk_and_no_network() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let deps = cargo.split("[dev-dependencies]").next().unwrap();
    for needle in ["aws-sdk", "aws_sdk", "reqwest", "hyper", "tokio"] {
        assert!(!deps.contains(needle), "{needle} in Cargo.toml");
    }
    for entry in fs::read_dir(root.join("src")).unwrap() {
        let path = entry.unwrap().path();
        let source = fs::read_to_string(&path).unwrap();
        for needle in [
            "std::net",
            "TcpStream",
            "UdpSocket",
            "reqwest",
            "aws_sdk",
            "aws-sdk",
        ] {
            assert!(!source.contains(needle), "{needle} in {}", path.display());
        }
    }
    // Built-in rates come from the file compiled into the binary.
    let builtin = prices_builtin();
    assert_eq!(builtin.source, "https://aws.amazon.com/lambda/pricing/");
    assert_eq!(builtin.date, "2026-11-09");
}

#[test]
fn claim_c18_default_architecture_is_x86_64() {
    assert_eq!(Options::default().default_arch, Arch::X86_64);
    assert_eq!(Arch::parse("x86"), None);
    assert_eq!(Arch::parse("ARM64"), None);
    let text = "# function: checkout\n".to_string() + &line("a1", "1", 1000, 1024, 20);
    let priced = bill(&text, &Options::default());
    assert_eq!(priced.rows[0].architecture, Arch::X86_64);
    let prices = prices_builtin();
    assert_eq!(
        format_usd(priced.rows[0].duration_usd),
        format_usd(duration_usd(1024, 1000, prices.x86_64_per_gb_second).unwrap())
    );
    let mut mapped = Options::default();
    mapped.functions.insert("checkout".to_string(), Arch::Arm64);
    let arm = bill(&text, &mapped);
    assert_eq!(arm.rows[0].architecture, Arch::Arm64);
    assert!(priced.rows[0].duration_usd > arm.rows[0].duration_usd);

    let conflict = mapped;
    let headed =
        "# function: checkout\n# arch: x86_64\n".to_string() + &line("a1", "1", 10, 128, 8);
    let err = bill_named("f.log", &headed, &conflict).unwrap_err();
    assert_eq!(err.message(), "f.log: line 3: architecture conflict");
}

#[test]
fn claim_c19_function_name_precedence() {
    let report = line("a1", "1", 10, 128, 8);
    let options = Options {
        default_function: Some("from-cli".to_string()),
        ..Options::default()
    };

    let from_group = "{\"logGroup\":\"/aws/lambda/from-group\",\"message\":\"REPORT RequestId: a1 Duration: 1 ms Billed Duration: 10 ms Memory Size: 128 MB Max Memory Used: 8 MB\"}\n";
    let got = bill(from_group, &options);
    assert_eq!(got.rows[0].function, "from-group");

    let unslashed = "{\"logGroup\":\"aws/lambda/from-group\",\"message\":\"REPORT RequestId: a1 Duration: 1 ms Billed Duration: 10 ms Memory Size: 128 MB Max Memory Used: 8 MB\"}\n";
    assert_eq!(bill(unslashed, &options).rows[0].function, "from-group");

    let header_vs_group = format!("# function: from-header\n{from_group}");
    let err = bill_named("f.log", &header_vs_group, &options).unwrap_err();
    assert!(err.message().contains("function name conflict"));
    assert!(!err.message().contains("from-header"));
    assert!(!err.message().contains("from-group"));

    let header = format!("# lambda-bill-function: from-header\n{report}");
    let got = bill_named("dir/aws/lambda/from-path/a.log", &header, &options).unwrap_err();
    assert!(got.message().contains("function name conflict"));

    let header_only = format!("# function: from-header\n{report}");
    let got = bill_named("dir/aws/lambda/from-path/a.log", &header_only, &options);
    // Header and path disagree, so this is the conflict above. A header with no path wins over --function.
    assert!(got.is_err());
    let got = bill(&header_only, &options);
    assert_eq!(got.rows[0].function, "from-header");

    let got = bill_named("dir/aws/lambda/from-path/a.log", &report, &options).unwrap();
    assert_eq!(got.rows[0].function, "from-path");

    let got = bill(&report, &options);
    assert_eq!(got.rows[0].function, "from-cli");

    let err = bill_named("plain.log", &report, &Options::default()).unwrap_err();
    assert!(err.message().contains("function name missing"));
    assert!(!err.message().contains("RequestId"));

    let err = bill_named("dir/aws/lambda/one/aws/lambda/two/a.log", &report, &options).unwrap_err();
    assert_eq!(
        err.message(),
        "dir/aws/lambda/one/aws/lambda/two/a.log: function name conflict"
    );
    assert!(!err.message().contains("RequestId"));
}

#[test]
fn claim_c20_memory_range_is_128_through_10240() {
    assert!(bill_named("f.log", &line("RANGESECRET", "1", 10, 128, 0), &checkout()).is_ok());
    assert!(bill_named("f.log", &line("ok", "1", 10, 10240, 10240), &checkout()).is_ok());
    assert!(bill_named("f.log", &line("ok", "1", 0, 128, 1), &checkout()).is_ok());
    for (memory, max_used) in [(127, 1), (10241, 1), (128, 10241)] {
        let err = bill_named(
            "f.log",
            &line("RANGESECRET", "1", 10, memory, max_used),
            &checkout(),
        )
        .unwrap_err();
        assert_eq!(err.message(), "f.log: line 1: not a Lambda REPORT line");
        assert!(!err.message().contains("RANGESECRET"));
    }
    let raised = bill(&line("ok", "1", 10, 128, 200), &checkout());
    assert_eq!(raised.rows[0].max_memory_used_mb, 200);
    assert_eq!(raised.rows[0].advice, Advice::Raise { memory_mb: 154 });
}

#[test]
fn claim_c22_request_ids_are_not_printed() {
    let id = "11111111-2222-3333-4444-555555555555";
    let got = bill(&line(id, "1", 10, 128, 8), &checkout());
    let text = render(&got, false);
    let json = render(&got, true);
    assert!(!text.contains(id));
    assert!(!json.contains(id));
    assert!(!text.contains("RequestId"));
}

#[test]
fn claim_c23_a_bad_report_line_is_not_printed() {
    let text = "REPORT RequestId: SECRETLINE Duration: nope ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    let err = bill_named("f.log", text, &checkout()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 1: not a Lambda REPORT line");
    assert!(!err.message().contains("SECRETLINE"));
    assert!(!err.message().contains("Duration"));
}

#[test]
fn claim_c24_the_total_is_the_sum_of_the_rows() {
    let options = Options::default();
    let inputs = [
        NamedBytes {
            name: "aws/lambda/billing/a.log".to_string(),
            bytes: line("a1", "1", 5, 256, 30).into_bytes(),
            strict: true,
        },
        NamedBytes {
            name: "aws/lambda/checkout/a.log".to_string(),
            bytes: line_init("a2", "1", 5, 128, 20).into_bytes(),
            strict: true,
        },
    ];
    let got = bill_from_inputs(&inputs, &options).unwrap();
    assert_eq!(got.rows.len(), 2);
    assert_eq!(
        got.totals.requests,
        got.rows[0].requests + got.rows[1].requests
    );
    assert_eq!(
        got.totals.cold_starts,
        got.rows[0].cold_starts + got.rows[1].cold_starts
    );
    assert_eq!(
        got.totals.gb_seconds,
        got.rows[0].gb_seconds + got.rows[1].gb_seconds
    );
    assert_eq!(
        got.totals.duration_usd,
        got.rows[0].duration_usd + got.rows[1].duration_usd
    );
    assert_eq!(
        got.totals.request_usd,
        got.rows[0].request_usd + got.rows[1].request_usd
    );
    assert_eq!(
        got.totals.total_usd,
        got.rows[0].total_usd + got.rows[1].total_usd
    );
    let text = render(&got, false);
    assert!(text.lines().any(|line| line.starts_with("total")));
    assert!(!text.contains("per day"));
    assert!(!text.contains("factor:"));
    assert!(text.contains("per month"));
}

#[test]
fn claim_c25_json_includes_the_source_url_and_the_date() {
    let got = bill(&line("a1", "1", 10, 128, 8), &checkout());
    let json = render(&got, true);
    assert!(json.ends_with('\n'));
    assert!(json.contains("\"version\": \"1.0.2\""));
    assert!(json.contains("\"source\": \"https://aws.amazon.com/lambda/pricing/\""));
    assert!(json.contains("\"date\": \"2026-11-09\""));
    assert!(json.contains("\"currency\": \"USD\""));
    assert!(json.contains("\"region\": \"us-east-1\""));
    assert!(json.contains("\"tier\": \"first\""));
    let version = json.find("\"version\"").unwrap();
    let currency = json.find("\"currency\"").unwrap();
    let source = json.find("\"source\"").unwrap();
    let date = json.find("\"date\"").unwrap();
    assert!(version < currency && currency < source && source < date);
    assert!(json.contains("\"duration_usd\": \""));
    assert!(json.contains("\"gb_seconds\": \""));
    assert!(!json.contains("\"duration_usd\": 0"));
}

#[test]
fn claim_c26_a_report_needs_the_marker_and_the_fields_in_order() {
    let ok = format!(
        "START RequestId: abc Version: $LATEST\n{}",
        line("a1", "1", 10, 128, 8)
    );
    assert_eq!(bill(&ok, &checkout()).rows[0].requests, 1);

    let xreport = format!(
        "XREPORT RequestId: SECRETX Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n{}",
        line("a1", "1", 10, 128, 8)
    );
    let got = bill(&xreport, &checkout());
    assert_eq!(got.rows[0].requests, 1);
    assert!(!render(&got, false).contains("SECRETX"));

    let only_x = "XREPORT RequestId: SECRETX Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    let err = bill_named("f.log", only_x, &checkout()).unwrap_err();
    assert_eq!(err.message(), "f.log: no REPORT lines");
    assert!(!err.message().contains("SECRETX"));

    let missing =
        "REPORT RequestId: abc Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    let err = bill_named("f.log", missing, &checkout()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 1: not a Lambda REPORT line");

    let reversed = "REPORT RequestId: abc Billed Duration: 1 ms Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    assert!(bill_named("f.log", reversed, &checkout()).is_err());

    let two = "REPORT RequestId: abc Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB REPORT RequestId: def Duration: 1 ms Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    let err = bill_named("f.log", two, &checkout()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 1: not a Lambda REPORT line");
    assert!(!err.message().contains("def"));
}

#[test]
fn claim_c27_units_are_ms_and_mb_and_billed_duration_is_an_integer() {
    let err = bill_named(
        "f.log",
        &line("UNITSECRET", "1", 10, 128, 8).replace("MB", "mb"),
        &checkout(),
    )
    .unwrap_err();
    assert_eq!(err.message(), "f.log: line 1: not a Lambda REPORT line");
    assert!(!err.message().contains("UNITSECRET"));

    let millis = line("a1", "1", 10, 128, 8).replace(" ms", " MS");
    assert!(bill_named("f.log", &millis, &checkout()).is_err());

    let fraction = "REPORT RequestId: a1 Duration: 1 ms Billed Duration: 1.5 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    assert!(bill_named("f.log", fraction, &checkout()).is_err());

    let fractional_duration = bill(&line("a1", "100.00", 51, 128, 8), &checkout());
    assert_eq!(fractional_duration.rows[0].requests, 1);
    let cold = bill(&line_init("a1", "100.00", 51, 128, 8), &checkout());
    assert_eq!(cold.rows[0].cold_starts, 1);
}

#[test]
fn claim_c28_a_price_file_replaces_the_builtin_rates() {
    let text = r#"{
        "currency": "USD",
        "source": "https://example.test/prices",
        "date": "2026-01-02",
        "region": "eu-west-1",
        "tier": "second",
        "request_per_million": "2",
        "x86_64_per_gb_second": "1",
        "arm64_per_gb_second": "1",
        "note": "ignored-SECRET"
    }"#;
    let prices = load_prices_str(text).unwrap();
    assert_eq!(prices.tier, "second");
    assert_eq!(format_usd(prices.x86_64_per_gb_second), "1.00");
    let mut options = checkout();
    options.prices = prices;
    let got = bill(&line("a1", "1", 1000, 1024, 20), &options);
    assert_eq!(format_usd(got.rows[0].duration_usd), "1.00");
    assert_eq!(format_usd(got.rows[0].request_usd), "0.000002");
    assert_eq!(format_usd(got.totals.total_usd), "1.000002");
    let rendered = render(&got, false);
    assert!(rendered.contains(
        "source https://example.test/prices, date 2026-01-02, region eu-west-1, tier second"
    ));
    assert!(!rendered.contains("SECRET"));

    let duplicate = text.replacen("\"currency\"", "\"currency\":\"USD\",\"currency\"", 1);
    let err = load_prices_str(&duplicate).unwrap_err();
    assert_eq!(err.message(), "duplicate key");
    let exponent = text.replace("\"2\"", "\"1e-7\"");
    let err = load_prices_str(&exponent).unwrap_err();
    assert_eq!(err.message(), "bad request_per_million");
    assert!(!err.message().contains("1e-7"));
    let negative = text.replace("\"2\"", "\"-2\"");
    let err = load_prices_str(&negative).unwrap_err();
    assert_eq!(err.message(), "bad request_per_million");
    assert!(!err.message().contains("-2"));
}

#[test]
fn claim_c29_a_timestamp_prefix_is_ignored() {
    let stamped = format!(
        "2024-01-15T12:00:00.000Z {}",
        line("a1", "12.34", 13, 128, 58)
    );
    let plain = line("a1", "1.00", 13, 128, 58);
    let left = bill(&stamped, &checkout());
    let right = bill(&plain, &checkout());
    assert_eq!(left.rows[0].gb_seconds, right.rows[0].gb_seconds);
    assert_eq!(left.rows[0].requests, 1);
    let bracket = format!(
        "\u{feff}[2024-01-15T00:00:00.000Z]\t{}\r\n",
        line("a1", "1", 8, 128, 1).trim_end()
    );
    let got = bill(&bracket, &checkout());
    assert_eq!(got.totals.requests, 1);
    assert!(!render(&got, false).contains("2024-01-15"));
}

#[test]
fn claim_c30_errors_do_not_include_the_file_contents() {
    let prices = r#"{
        "currency": "USD",
        "source": "https://aws.amazon.com/lambda/pricing/",
        "date": "2026-11-09",
        "region": "us-east-1",
        "tier": "first",
        "request_per_million": "SECRET99",
        "x86_64_per_gb_second": "0.0000166667",
        "arm64_per_gb_second": "0.0000133334"
    }"#;
    let err = load_prices_str(prices).unwrap_err();
    assert_eq!(err.message(), "bad request_per_million");
    assert!(!err.message().contains("SECRET99"));
    assert!(!err.message().contains("0.0000166667"));

    let log = "REPORT RequestId: LOGSECRET Duration: nope Billed Duration: 1 ms Memory Size: 128 MB Max Memory Used: 1 MB\n";
    let err = bill_named("export.log", log, &checkout()).unwrap_err();
    assert_eq!(
        err.message(),
        "export.log: line 1: not a Lambda REPORT line"
    );
    assert!(!err.message().contains("LOGSECRET"));
    assert!(!err.message().contains("nope"));
}

#[test]
fn line_longer_than_1048576_bytes_is_rejected() {
    let mut line = b"REPORT RequestId: abc Duration: 1.00 ms Billed Duration: 5 ms Memory Size: 128 MB Max Memory Used: 9 MB".to_vec();
    assert!(line.len() < MAX_LINE_BYTES);
    line.resize(MAX_LINE_BYTES, b' ');
    let options = checkout();
    let ok = bill_from_inputs(
        &[NamedBytes {
            name: "f.log".to_string(),
            bytes: line.clone(),
            strict: true,
        }],
        &options,
    )
    .unwrap();
    assert_eq!(ok.rows[0].requests, 1);
    line.push(b' ');
    assert_eq!(line.len(), MAX_LINE_BYTES + 1);
    let err = bill_from_inputs(
        &[NamedBytes {
            name: "f.log".to_string(),
            bytes: line,
            strict: true,
        }],
        &options,
    )
    .unwrap_err();
    assert_eq!(
        err.message(),
        "f.log: line 1: line is longer than 1048576 bytes"
    );
    assert!(!err.message().contains("RequestId"));
}

#[test]
fn billed_duration_overflow_is_an_error() {
    let text = format!(
        "{}{}",
        line("a1", "1", u64::MAX, 128, 1),
        line("a2", "1", u64::MAX, 128, 1)
    );
    let err = bill_named("f.log", &text, &checkout()).unwrap_err();
    assert_eq!(err.message(), "billed duration overflow");
}

#[test]
fn max_memory_used_of_a_row_is_the_highest_of_its_requests() {
    let text = format!(
        "{}{}{}",
        line("m1", "1", 10, 512, 100),
        line("m2", "1", 10, 512, 300),
        line("m3", "1", 10, 512, 200)
    );
    let got = bill(&text, &checkout());
    assert_eq!(got.rows.len(), 1);
    assert_eq!(got.rows[0].requests, 3);
    assert_eq!(got.rows[0].max_memory_used_mb, 300);
    // 300 * 1.20 = 360, so the advice follows the highest value.
    assert_eq!(got.rows[0].advice, Advice::Downsize { memory_mb: 360 });
}

#[test]
fn library_rejects_headroom_above_100_and_a_bad_default_function() {
    let text = line("h1", "1", 10, 128, 8);
    let options = Options {
        headroom_percent: 101,
        ..checkout()
    };
    let err = bill_named("f.log", &text, &options).unwrap_err();
    assert_eq!(err.message(), "bad headroom");
    let options = Options {
        default_function: Some("bad name".to_string()),
        ..Options::default()
    };
    let err = bill_named("f.log", &text, &options).unwrap_err();
    assert_eq!(err.message(), "bad function name");
}

#[test]
fn json_advice_fields_name_the_action_and_the_suggested_memory() {
    let input = |name: &str, report: String| NamedBytes {
        name: format!("{name}.log"),
        bytes: format!("# function: {name}\n{report}").into_bytes(),
        strict: true,
    };
    let inputs = [
        input("down", line("d1", "1", 10, 512, 100)),
        input("keep", line("k1", "1", 10, 128, 20)),
        input("up", line("u1", "1", 10, 1000, 950)),
    ];
    let got = bill_from_inputs(&inputs, &Options::default()).unwrap();
    let json = render(&got, true);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rows = value["functions"].as_array().unwrap();
    let find = |name: &str| rows.iter().find(|row| row["function"] == name).unwrap();
    assert_eq!(find("down")["advice"], "downsize");
    assert_eq!(find("down")["advice_mb"], 128);
    assert_eq!(find("keep")["advice"], "keep");
    assert_eq!(find("keep")["advice_mb"], 128);
    assert_eq!(find("up")["advice"], "raise");
    assert_eq!(find("up")["advice_mb"], 1200);
}

#[test]
fn two_function_headers_that_differ_are_a_conflict() {
    let text = format!(
        "# function: one\n# function: two\n{}",
        line("a1", "1", 10, 128, 8)
    );
    let err = bill_named("f.log", &text, &Options::default()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 2: function name conflict");
    assert!(!err.message().contains("one"));
    assert!(!err.message().contains("two"));
}

#[test]
fn architecture_headers_and_events_that_differ_are_a_conflict() {
    let report = line("a1", "1", 10, 128, 8);
    let heads = format!("# function: f\n# arch: x86_64\n# arch: arm64\n{report}");
    let err = bill_named("f.log", &heads, &Options::default()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 3: architecture conflict");

    let message = format!("\"{}\"", report.trim_end());
    let event = format!(
        "# function: f\n# arch: arm64\n{{\"architecture\":\"x86_64\",\"message\":{message}}}\n"
    );
    let err = bill_named("f.log", &event, &Options::default()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 3: architecture conflict");
}

#[test]
fn a_bad_architecture_in_a_header_or_an_event_is_named_without_the_value() {
    let report = line("a1", "1", 10, 128, 8);
    let header = format!("# function: f\n# arch: SECRETARCH\n{report}");
    let err = bill_named("f.log", &header, &Options::default()).unwrap_err();
    assert_eq!(err.message(), "f.log: line 2: bad architecture");
    assert!(!err.message().contains("SECRETARCH"));

    let message = format!("\"{}\"", report.trim_end());
    for value in ["\"SECRETARCH\"", "7"] {
        let event = format!("{{\"architecture\":{value},\"message\":{message}}}\n");
        let err = bill_named("f.log", &event, &Options::default()).unwrap_err();
        assert_eq!(err.message(), "f.log: event 1: bad architecture", "{value}");
    }
}

#[test]
fn a_message_line_longer_than_1048576_bytes_inside_one_document_is_rejected() {
    let big = "x".repeat(MAX_LINE_BYTES + 1);
    let text = format!("{{\"message\":\"{big}\"}}");
    let err = bill_named("f.log", &text, &Options::default()).unwrap_err();
    assert_eq!(
        err.message(),
        "f.log: event 1: line is longer than 1048576 bytes"
    );
}

#[test]
fn functions_file_that_is_not_json_or_not_an_object_is_named_without_the_value() {
    let err = lambda_bill::load_functions_str("{").unwrap_err();
    assert_eq!(err.message(), "not JSON");
    let err = lambda_bill::load_functions_str("[\"SECRETITEM\"]").unwrap_err();
    assert_eq!(err.message(), "expected an object");
}
