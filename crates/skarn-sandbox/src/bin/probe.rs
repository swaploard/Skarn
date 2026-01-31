//! Sandbox self-test probe.
//!
//! Usage (driven by the integration tests):
//!   SKARN_PROBE_POLICY='<json>' SKARN_PROBE_SELFAPPLY=1 \
//!     skarn-sandbox-probe <op> <arg>
//!
//! Operations:
//!   write   <path>        try to create+write a file at <path>
//!   read    <path>        try to read a file at <path>
//!   connect <host:port>   try to open a TCP connection
//!
//! Exit codes:
//!   0   operation succeeded
//!   10  operation was denied (permission denied / connection refused-by-sandbox)
//!   11  operation failed for another reason
//!   12  applying the sandbox failed
//!   20  bad invocation

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use skarn_sandbox::Policy;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: probe <op> <arg>");
        std::process::exit(20);
    }
    let op = &args[1];
    let arg = &args[2];

    if std::env::var("SKARN_PROBE_SELFAPPLY").as_deref() == Ok("1") {
        let policy_json = std::env::var("SKARN_PROBE_POLICY").unwrap_or_default();
        let policy: Policy = match serde_json::from_str(&policy_json) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("bad policy json: {e}");
                std::process::exit(20);
            }
        };
        if let Err(e) = policy.apply_to_current_process() {
            eprintln!("apply failed: {e}");
            std::process::exit(12);
        }
    }
