//! Asks git for the commit of this tree. A debug build prints it after the
//! version in `--version`; a release build never receives it. A missing git,
//! a failed command or a value that is not a full commit hash leaves the
//! version as it is.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(lambda_bill_git_sha)");
    println!("cargo:rerun-if-changed=build.rs");
    watch_head();
    if std::env::var("PROFILE").map_or(true, |profile| profile != "debug") {
        return;
    }
    if let Some(sha) = head_sha() {
        println!("cargo:rustc-cfg=lambda_bill_git_sha");
        println!("cargo:rustc-env=LAMBDA_BILL_GIT_SHA={sha}");
    }
}

fn manifest_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(manifest_dir())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|text| text.trim().to_string())
}

/// A path inside the git directory (also right in a linked worktree), if the
/// file exists.
fn git_file(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(git(&["rev-parse", "--git-path", name])?);
    let path = if path.is_absolute() {
        path
    } else {
        manifest_dir().join(path)
    };
    path.is_file().then_some(path)
}

/// Build again when the commit changes: `HEAD`, the branch file it points
/// at, or the packed refs.
fn watch_head() {
    let Some(head) = git_file("HEAD") else {
        return;
    };
    println!("cargo:rerun-if-changed={}", head.display());
    let Ok(text) = std::fs::read_to_string(&head) else {
        return;
    };
    let Some(reference) = text.trim().strip_prefix("ref:") else {
        return;
    };
    let target = git_file(reference.trim()).or_else(|| git_file("packed-refs"));
    if let Some(path) = target {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn head_sha() -> Option<String> {
    let sha = git(&["rev-parse", "--verify", "HEAD"])?;
    (sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(sha)
}
