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

