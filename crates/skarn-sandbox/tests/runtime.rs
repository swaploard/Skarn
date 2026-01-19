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
