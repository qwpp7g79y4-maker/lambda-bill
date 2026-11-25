//! The test count that README.md and QUALITY.md state must be the real one.
//!
//! The count is the number of test attributes in `src/` and `tests/`. The
//! suite has no ignored test and no doc-test, so each attribute is one test
//! in the result of `cargo test`.

use std::fs;
use std::path::{Path, PathBuf};

const ATTRIBUTE: &str = concat!("#[te", "st]");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Lines that are exactly one test attribute, in every source and test file.
fn count_tests(text: &str) -> usize {
    text.lines().filter(|line| line.trim() == ATTRIBUTE).count()
}

fn real_count() -> usize {
    let mut files = Vec::new();
    rust_files(&root().join("src"), &mut files);
    rust_files(&root().join("tests"), &mut files);
    files
        .iter()
        .map(|path| count_tests(&fs::read_to_string(path).unwrap()))
        .sum()
}

/// The number in the first line of `text` that starts with `prefix`, read up
/// to the next space.
fn number_after(text: &str, prefix: &str) -> usize {
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("no line starts with `{prefix}`"));
    line.split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("no number after `{prefix}`"))
}

#[test]
fn the_documents_state_the_real_number_of_tests() {
    let real = real_count();
    assert!(real > 100, "found only {real} tests");
    let quality = fs::read_to_string(root().join("QUALITY.md")).unwrap();
    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    assert_eq!(number_after(&quality, "Suite: "), real, "QUALITY.md header");
    assert_eq!(
        number_after(&readme, "The suite has "),
        real,
        "README.md count"
    );
    let total_row = quality
        .lines()
        .find(|line| line.starts_with("| total |"))
        .expect("total row in QUALITY.md");
    let cells: Vec<&str> = total_row.split('|').map(str::trim).collect();
    assert_eq!(cells[2].parse::<usize>().unwrap(), real, "debug total");
    assert_eq!(cells[5].parse::<usize>().unwrap(), real, "release total");
}

#[test]
fn the_counter_counts_attributes_and_nothing_else() {
    let sample = format!("{ATTRIBUTE}\nfn a() {{}}\n  {ATTRIBUTE}\n// {ATTRIBUTE} in a comment\n");
    assert_eq!(count_tests(&sample), 2);
}

const QUALITY_FILES: [(&str, &str, &str, &str); 2] = [
    (
        "QUALITY.md",
        "## Not run",
        "This commit changes documentation only; the measurements below apply to code commit ",
        "The measurements below apply to this commit.",
    ),
    (
        "QUALITY.nl.md",
        "## Niet uitgevoerd",
        "Deze commit wijzigt alleen documentatie; de meting hieronder geldt voor codecommit ",
        "De meting hieronder geldt voor deze commit.",
    ),
];

/// The line directly under the title says which commit was measured: either
/// a real short hash of an ancestor, or "this commit" when the commit itself
/// changes code.
#[test]
fn quality_states_which_commit_it_measured() {
    for (name, _, with_hash, this_commit) in QUALITY_FILES {
        let text = fs::read_to_string(root().join(name)).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("# "), "{name}: title");
        assert_eq!(lines[1], "", "{name}: blank line under the title");
        let line = lines[2];
        if line == this_commit {
            continue;
        }
        let hash = line
            .strip_prefix(with_hash)
            .and_then(|rest| rest.strip_suffix('.'))
            .unwrap_or_else(|| panic!("{name}: line 3 is not the commit sentence: {line}"));
        assert_eq!(hash.len(), 7, "{name}: short hash");
        assert!(
            hash.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "{name}: hash is lowercase hex"
        );
        // Only when the sources are a git checkout; a packaged crate has no history.
        if root().join(".git").exists() {
            if let Ok(status) = std::process::Command::new("git")
                .args(["merge-base", "--is-ancestor", hash, "HEAD"])
                .current_dir(root())
                .status()
            {
                assert!(
                    status.success(),
                    "{name}: {hash} is not an ancestor of HEAD"
                );
            }
        }
    }
}

/// What was not run is listed in one place per file: the "Not run" section.
#[test]
fn what_was_not_run_is_listed_in_one_section_only() {
    const PHRASES: [&str; 8] = [
        "was not run",
        "were not run",
        "not re-run",
        "has not run",
        "niet gedraaid",
        "niet opnieuw gedraaid",
        "is niet uitgevoerd",
        "zijn niet gedraaid",
    ];
    for (name, section, _, _) in QUALITY_FILES {
        let text = fs::read_to_string(root().join(name)).unwrap();
        let mut in_section = false;
        let mut payment = 0;
        for line in text.lines() {
            if line.starts_with("## ") {
                in_section = line == section;
            }
            if line.contains("payment problem") || line.contains("betaalprobleem") {
                payment += 1;
            }
            if in_section {
                continue;
            }
            let lower = line.to_lowercase();
            for phrase in PHRASES {
                assert!(
                    !lower.contains(phrase),
                    "{name}: `{phrase}` outside the `{section}` section: {line}"
                );
            }
        }
        assert_eq!(payment, 1, "{name}: the payment problem is named once");
    }
}
