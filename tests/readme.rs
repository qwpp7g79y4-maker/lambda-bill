//! README.md is executable: console blocks match the binary, and every
//! numbered claim has a test.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("missing heading {heading}"))
        + marker.len();
    let rest = &text[start..];
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

fn rust_sources() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::new();
    for dir in ["src", "tests"] {
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push_str(&fs::read_to_string(&path).unwrap());
                out.push('\n');
            }
        }
    }
    out
}

/// Every ```console block: the first line is `$ lambda-bill ARGS`, the rest is stdout.
fn console_blocks(doc: &str) -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line != "```console" {
            continue;
        }
        let call = lines.next().expect("empty console block");
        let args: Vec<String> = call
            .strip_prefix("$ lambda-bill ")
            .unwrap_or_else(|| panic!("console block does not start with `$ lambda-bill `: {call}"))
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let mut body = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "```" {
                closed = true;
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        assert!(closed, "console block not closed: {call}");
        out.push((call.to_string(), args, body));
    }
    assert!(!out.is_empty(), "README has no console block");
    out
}

#[test]
fn readme_sections_follow_the_documentation_standard() {
    let text = read("README.md");
    let headings: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| line.trim_start_matches("## "))
        .collect();
    assert_eq!(
        headings,
        vec![
            "Why",
            "Installation",
            "Quick start",
            "Quickstart in 5 minutes",
            "Options",
            "Example",
            "How it counts",
            "Exit codes",
            "Limitations",
            "Prices and responsibility",
            "Maintenance",
            "Support",
            "License",
            "Contact"
        ]
    );
}

/// Every run of five or more digits in `text`, sorted. Limits, sizes and
/// measured figures are written the same way in both languages.
fn long_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            current.push(c);
        } else {
            if current.len() >= 5 {
                out.push(current.clone());
            }
            current.clear();
        }
    }
    out.sort();
    out
}

#[test]
fn the_dutch_readme_says_the_same_as_the_english_one() {
    let english = read("README.md");
    let dutch = read("README.nl.md");
    let headings: Vec<_> = dutch
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| line.trim_start_matches("## "))
        .collect();
    assert_eq!(
        headings,
        vec![
            "Waarom",
            "Installatie",
            "Snelstart",
            "Aan de slag in 5 minuten",
            "Opties",
            "Voorbeeld",
            "Hoe het telt",
            "Afsluitcodes",
            "Beperkingen",
            "Prijzen en verantwoordelijkheid",
            "Onderhoud",
            "Ondersteuning",
            "Licentie",
            "Contact"
        ]
    );
    // The console blocks are the real output, byte for byte.
    let blocks = |text: &str| -> Vec<String> {
        console_blocks(text)
            .into_iter()
            .map(|(call, _, body)| format!("{call}\n{body}"))
            .collect()
    };
    assert_eq!(blocks(&dutch), blocks(&english));
    // The same claims, in the same order, and the same limits and figures.
    let claims = |text: &str| -> Vec<String> {
        text.lines()
            .filter_map(|line| line.split_once(". ").map(|(id, _)| id.to_string()))
            .filter(|id| {
                id.strip_prefix('C')
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
            .collect()
    };
    assert_eq!(claims(&dutch), claims(&english));
    assert_eq!(claims(&dutch).len(), 30);
    assert_eq!(long_numbers(&dutch), long_numbers(&english));
    // Both name the real test count of the suite.
    for text in [&english, &dutch] {
        assert!(text.contains(&format!("{}", suite_size())), "test count");
    }
}

/// The number after `The suite has ` in the English README.
fn suite_size() -> usize {
    let readme = read("README.md");
    let at = readme.find("The suite has ").unwrap() + "The suite has ".len();
    readme[at..].split(' ').next().unwrap().parse().unwrap()
}

#[test]
fn install_uses_a_local_path() {
    let readme = read("README.md");
    let install = section(&readme, "## Installation");
    assert!(install.contains("cargo install --locked --path ."));
    assert!(install.contains("1.85"));
    assert!(install.contains("cargo install lambda-bill"));
    assert!(!install.contains("not published"));
}

#[test]
fn quick_start_writes_nothing_into_the_repository() {
    let readme = read("README.md");
    let quick = section(&readme, "## Quick start");
    assert!(quick.contains("mktemp -d"));
    for line in quick.lines().filter(|line| line.contains("cat >")) {
        assert!(
            line.contains("\"$dir/"),
            "writes outside the temp dir: {line}"
        );
    }
    assert!(!quick.contains("> examples/"), "writes into examples/");
}

#[test]
fn prices_section_names_the_source_and_date_of_the_price_file() {
    let readme = read("README.md");
    let prices = section(&readme, "## Prices and responsibility");
    let file = read("prices/lambda-ondemand.json");
    for key in ["source", "date"] {
        let needle = format!("\"{key}\": \"");
        let at = file.find(&needle).expect("key in the price file") + needle.len();
        let value = &file[at..at + file[at..].find('"').unwrap()];
        assert!(
            prices.contains(value),
            "{key} {value} is not in the section"
        );
    }
    assert!(prices.contains("--prices"));
    assert!(prices.contains("responsible"));
    let maintenance = section(&readme, "## Maintenance");
    assert!(maintenance.contains("GitHub Issue"));
    assert!(maintenance.contains("security"));
}

#[test]
fn every_example_command_matches_its_expected_output() {
    let doc = read("examples/README.md");
    let mut call: Option<Vec<String>> = None;
    let mut checked = 0;
    for line in doc.lines() {
        if let Some(rest) = line.strip_prefix("lambda-bill ") {
            call = Some(rest.split_whitespace().map(str::to_string).collect());
        } else if let Some(rest) = line.strip_prefix("Expected output: `") {
            let file = rest.strip_suffix('`').expect("closing backtick");
            let args = call.take().expect("a command before the expected output");
            let out = Command::new(env!("CARGO_BIN_EXE_lambda-bill"))
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .env("NO_COLOR", "1")
                .args(&args)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{args:?}");
            assert!(out.stderr.is_empty(), "{args:?}");
            assert_eq!(
                String::from_utf8(out.stdout).unwrap(),
                read(file),
                "{file} differs from {args:?}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn quickstart_heredoc_matches_the_sample_file() {
    let readme = read("README.md");
    let mut lines = readme.lines();
    let mut found = false;
    for line in lines.by_ref() {
        if line == "cat > \"$dir/report.log\" <<'LOG'" {
            found = true;
            break;
        }
    }
    assert!(found, "quick start heredoc is missing");
    let mut body = String::new();
    for line in lines {
        if line == "LOG" {
            break;
        }
        body.push_str(line);
        body.push('\n');
    }
    assert_eq!(body, read("examples/sample-report.log"));
}

#[test]
fn console_examples_match_the_binary() {
    for (call, args, expected) in console_blocks(&read("README.md")) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_lambda-bill"));
        cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
        cmd.env("NO_COLOR", "1");
        // Fixed date for the price age check: no dependence on the system clock.
        cmd.args(["--as-of", "2026-11-20"]);
        cmd.args(&args);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(out.status.success(), "{call} failed: {stderr}");
        assert!(stderr.is_empty(), "{call} wrote to stderr: {stderr}");
        assert_eq!(stdout, expected, "console mismatch for {call}");
    }
}

#[test]
fn every_how_it_counts_claim_has_a_test() {
    let readme = read("README.md");
    let how = section(&readme, "## How it counts");
    let mut ids = Vec::new();
    for line in how.lines() {
        let Some(rest) = line.trim().strip_prefix('C') else {
            continue;
        };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        ids.push(digits.parse::<u32>().unwrap());
    }
    assert_eq!(ids, (1..=30).collect::<Vec<_>>());
    let sources = rust_sources();
    // QUALITY.md maps each claim to the tests that cover it. Every test named
    // in a claim row must exist, and every claim must name at least one.
    let quality = read("QUALITY.md");
    for id in 1..=30 {
        let needle = format!("fn claim_c{id}_");
        assert!(sources.contains(&needle), "missing {needle}");
        let row = quality
            .lines()
            .find(|line| line.starts_with(&format!("| C{id} |")))
            .unwrap_or_else(|| panic!("QUALITY.md has no row for C{id}"));
        let cells: Vec<&str> = row.split('|').collect();
        let tests: Vec<&str> = cells[3]
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|name| name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .collect();
        assert!(!tests.is_empty(), "C{id} row names no test");
        for test in tests {
            assert!(
                sources.contains(&format!("fn {test}(")),
                "C{id} names a test that does not exist: {test}"
            );
        }
    }
}

#[test]
fn limitations_name_behaviors_the_tests_cover() {
    let readme = read("README.md");
    let limits = section(&readme, "## Limitations");
    for phrase in [
        "Provisioned Concurrency",
        "not a forecast",
        "x86_64",
        "268435456",
        "1048576",
        "symlink",
        "directory is nested too deeply",
        "AWS SDK",
        "no network",
        "per day",
    ] {
        assert!(limits.contains(phrase), "limitations missing {phrase}");
    }
    let sources = rust_sources();
    for needle in [
        "fn file_larger_than_268435456_bytes",
        "fn line_longer_than_1048576_bytes_is_rejected",
        "fn directory_symlink_is_skipped",
        "fn explicit_symlink_is_followed",
        "fn directory_at_64_levels_is_accepted",
        "fn directory_deeper_than_64_levels_is_an_error",
        "fn claim_c10_",
        "fn claim_c13_",
        "fn claim_c17_",
        "fn claim_c18_",
        "fn claim_c24_",
    ] {
        assert!(sources.contains(needle), "missing {needle}");
    }
}

#[test]
fn licenses_changelog_and_toolchain_match_the_package() {
    let mit = read("LICENSE-MIT");
    assert!(mit.contains("MIT License"));
    assert!(mit.contains("Copyright (c) 2026 PEPAI"));
    let apache = read("LICENSE-APACHE");
    assert!(apache.contains("Apache License"));
    assert!(apache.contains("Copyright 2026 PEPAI"));
    let changelog = read("CHANGELOG.md");
    assert!(changelog.contains("## [1.0.0] - 2026-11-15"));
    assert!(changelog.contains("## [0.1.0] - 2026-11-09"));
    assert!(changelog.contains("Semantic Versioning"));
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("rust-version = \"1.85\""));
    assert!(cargo.contains("MIT OR Apache-2.0"));
    assert!(cargo.contains("version = \"1.0.1\""));
    for needle in ["aws-sdk", "reqwest", "hyper", "tokio"] {
        assert!(!cargo.contains(needle), "{needle} in Cargo.toml");
    }
}
