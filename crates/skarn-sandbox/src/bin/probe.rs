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
