//! The files of the project must not point at the machine they were written
//! on, at wrapper options of a build setup, at files that stay out of the
//! package, or at internal tools. The patterns are general: no tool or folder
//! is named here, and the words below are written in pieces so that this file
//! does not contain them either.
//!
//! Scanned: the documents and the man page, the manifest (without its
//! `exclude` line), the sources, the tests, the examples, the price file, the
//! workflows and the scripts. `.gitignore` is not scanned: it has to name the
//! files that stay out of git.

use std::fs;
use std::path::{Path, PathBuf};

/// Substrings that must not appear in a scanned file (compared lower case).
const FORBIDDEN: [&str; 21] = [
    concat!("/me", "dia/"),
    concat!("/ho", "me/"),
    concat!("/us", "ers/"),
    concat!("c:", "\\"),
    concat!("~", "/"),
    concat!("--max", "-gb"),
    concat!("--na", "am"),
    concat!("task", ".md"),
    concat!("blueprint", ".md"),
    concat!("geheugen", "wachter"),
    concat!("bouw", "-tmp"),
    concat!("brug", "-prompts"),
    concat!("gr", "ok"),
    concat!("cla", "ude"),
    concat!("anthr", "opic"),
    concat!("son", "net"),
    concat!("op", "us"),
    concat!("open", "bot"),
    concat!("open", "ai"),
    concat!("cod", "ex"),
    concat!("co-auth", "ored"),
];

/// Files at the top of the project that are scanned.
const ROOT_FILES: [&str; 10] = [
    "README.md",
    "README.nl.md",
    "CONTRIBUTING.md",
    "QUALITY.md",
    "CHANGELOG.md",
    "SECURITY.md",
    "Cargo.toml",
    "deny.toml",
    ".gitattributes",
    "THIRD-PARTY-LICENSES.html",
];

/// Directories that are scanned, with everything below them.
const DIRECTORIES: [&str; 7] = [
    "man", "src", "tests", "examples", "prices", "scripts", ".github",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.map(|entry| entry.unwrap().path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// The first forbidden pattern in `text`, if any.
fn find_forbidden(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    FORBIDDEN.into_iter().find(|word| lower.contains(word))
}

/// Every forbidden pattern in `text`.
fn find_all(text: &str) -> Vec<&'static str> {
    let lower = text.to_lowercase();
    FORBIDDEN
        .into_iter()
        .filter(|word| lower.contains(word))
        .collect()
}

/// The manifest names the files that stay out of the package; the `exclude`
/// array (one line in the repository, several in a packaged manifest) is the
/// only place a scanned file may name them.
fn without_package_exclude(label: &str, text: &str) -> String {
    if label != "Cargo.toml" {
        return text.to_string();
    }
    let mut kept = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !inside && line.trim_start().starts_with("exclude") {
            inside = true;
        }
        if inside {
            if line.contains(']') {
                inside = false;
            }
            continue;
        }
        kept.push(line);
    }
    kept.join("\n")
}

/// Every scanned file under `root` as (label, text).
fn scanned(root: &Path) -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = ROOT_FILES
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect();
    for dir in DIRECTORIES {
        walk(&root.join(dir), &mut files);
    }
    files
        .iter()
        .map(|path| {
            let label = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(path).unwrap_or_else(|err| panic!("read {label}: {err}"));
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let text = without_package_exclude(&label, &text);
            (label, text)
        })
        .collect()
}

/// The files under `root` that hold a forbidden pattern, with the pattern.
fn offenders(root: &Path) -> Vec<(String, &'static str)> {
    scanned(root)
        .into_iter()
        .filter_map(|(label, text)| find_forbidden(&text).map(|word| (label, word)))
        .collect()
}

#[test]
fn project_files_hold_no_local_paths_or_internal_names() {
    let files = scanned(&root());
    let labels: Vec<&str> = files.iter().map(|(label, _)| label.as_str()).collect();
    for needed in [
        "README.md",
        "QUALITY.md",
        "CHANGELOG.md",
        "SECURITY.md",
        "Cargo.toml",
        "man/lambda-bill.1",
        "src/lib.rs",
        "tests/docs_clean.rs",
        "examples/sample-report.log",
        "prices/lambda-ondemand.json",
    ] {
        assert!(labels.contains(&needed), "{needed} is not scanned");
    }
    for (name, text) in &files {
        assert!(!text.is_empty(), "{name} is empty");
    }
    let found = offenders(&root());
    assert!(found.is_empty(), "forbidden text in {found:?}");
}

#[test]
fn the_check_finds_a_planted_reference() {
    // Each pattern is found by itself, in lower and in upper case.
    for pattern in FORBIDDEN {
        for planted in [
            format!("x {pattern} y"),
            format!("x {} y", pattern.to_uppercase()),
        ] {
            assert!(find_all(&planted).contains(&pattern), "{planted}");
        }
    }
    // And a few references as they would be written.
    for (planted, pattern) in [
        (concat!("see /me", "dia/disk/x"), concat!("/me", "dia/")),
        (concat!("copy c:", "\\temp"), concat!("c:", "\\")),
        (concat!("cd ~", "/work"), concat!("~", "/")),
        (
            concat!("cargo test --max", "-gb 16"),
            concat!("--max", "-gb"),
        ),
        (concat!("notes in TASK", ".MD"), concat!("task", ".md")),
        (concat!("run brug", "-prompts"), concat!("brug", "-prompts")),
        (concat!("Co-Auth", "ored-By: x"), concat!("co-auth", "ored")),
    ] {
        assert!(find_all(planted).contains(&pattern), "{planted}");
    }
    assert_eq!(find_forbidden("plain text with a/b and 1~2"), None);
}

#[test]
fn every_scanned_place_is_read_and_a_word_planted_there_is_found() {
    let base = std::env::temp_dir().join(format!("lambda-bill-docs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let word = concat!("blueprint", ".md");
    fs::create_dir_all(&base).unwrap();
    for name in ROOT_FILES {
        fs::write(base.join(name), format!("x {word}")).unwrap();
    }
    for dir in DIRECTORIES {
        let nested = base.join(dir).join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("f.txt"), format!("x {word}")).unwrap();
    }
    let found = offenders(&base);
    let labels: Vec<&str> = found.iter().map(|(label, _)| label.as_str()).collect();
    let mut expected: Vec<String> = ROOT_FILES.iter().map(|n| (*n).to_string()).collect();
    for dir in DIRECTORIES {
        expected.push(format!("{dir}/a/b/f.txt"));
    }
    for label in &expected {
        assert!(labels.contains(&label.as_str()), "{label} is not scanned");
    }
    assert_eq!(labels.len(), expected.len(), "{labels:?}");
    fs::write(
        base.join("Cargo.toml"),
        format!("exclude = [\"/{word}\"]\nname = \"x\"\n"),
    )
    .unwrap();
    assert!(!offenders(&base)
        .iter()
        .any(|(label, _)| label == "Cargo.toml"));
    fs::write(
        base.join("Cargo.toml"),
        format!("exclude = [\n    \"/{word}\",\n    \"/x/\",\n]\nname = \"x\"\n"),
    )
    .unwrap();
    assert!(!offenders(&base)
        .iter()
        .any(|(label, _)| label == "Cargo.toml"));
    fs::write(base.join("Cargo.toml"), format!("name = \"{word}\"\n")).unwrap();
    assert!(offenders(&base)
        .iter()
        .any(|(label, _)| label == "Cargo.toml"));
    let _ = fs::remove_dir_all(&base);
}
