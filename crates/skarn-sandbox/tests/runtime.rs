//! Runtime sandbox enforcement tests.
//!
//! These spawn the `skarn-sandbox-probe` helper (a fresh, single-threaded
//! process) which self-applies a policy and then attempts a single operation.
//! We assert on the probe's exit code. Gated to Unix; on Linux CI runners
//! without Landlock the tests skip themselves.

#![cfg(unix)]

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use skarn_sandbox::{NetPolicy, Policy, RestrictionStatus, backend_report};

const PROBE: &str = env!("CARGO_BIN_EXE_skarn-sandbox-probe");

const EXIT_OK: i32 = 0;
const EXIT_DENIED: i32 = 10;

fn unique_root() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME set");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    let root = PathBuf::from(home).join(format!(".skarn-sbx-test-{pid}-{nanos}"));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn run_probe(policy: &Policy, op: &str, arg: &str) -> i32 {
    let json = serde_json::to_string(policy).unwrap();
    let status = Command::new(PROBE)
        .args([op, arg])
        .env("SKARN_PROBE_SELFAPPLY", "1")
        .env("SKARN_PROBE_POLICY", json)
        .status()
        .expect("spawn probe");
    status.code().expect("probe exited with a code")
}

fn skip_if_unenforced() -> bool {
    if backend_report().status == RestrictionStatus::NotEnforced {
        eprintln!("sandbox backend not enforced on this host; skipping runtime test");
        return true;
    }
    false
}

#[test]
fn writes_inside_workspace_are_allowed() {
    if skip_if_unenforced() {
        return;
    }
    let root = unique_root();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let policy = Policy::builder().workspace(&workspace).build();
    let target = workspace.join("out.txt");
    let code = run_probe(&policy, "write", target.to_str().unwrap());

    cleanup(&root);
    assert_eq!(code, EXIT_OK, "writing inside the workspace should succeed");
}

#[test]
fn writes_outside_workspace_are_denied() {
    if skip_if_unenforced() {
        return;
    }
    let root = unique_root();
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let policy = Policy::builder().workspace(&workspace).build();
    // `root` (under $HOME) is outside the workspace and outside the system tree.
    let target = root.join("escape.txt");
    let code = run_probe(&policy, "write", target.to_str().unwrap());

    cleanup(&root);
    assert_eq!(
        code, EXIT_DENIED,
