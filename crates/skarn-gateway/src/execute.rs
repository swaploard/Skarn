//! Running a Code Mode script — in-process, or in a dedicated OS-sandboxed
//! worker subprocess.
//!
//! Two execution strategies share one servicer pattern (fulfil host tool calls
//! against the [`DownstreamManager`] that owns the MCP clients):
//!
//! * **In-process** ([`execute_in_process`]): the `!Send` QuickJS isolate runs
//!   on a dedicated thread with its own current-thread runtime, bridged back to
//!   the main runtime over an mpsc channel. The isolate is hermetic but shares
//!   the gateway's address space.
//!
//! * **Worker** ([`execute_worker`], Unix only): the isolate runs in a child
//!   process that confines *itself* with the OS-native sandbox before touching
//!   the script, bridged back to the parent over its stdio pipes
//!   (newline-delimited JSON, see [`crate::worker_proto`]). A hypothetical
//!   isolate escape lands in a kernel-confined process with no network and no
//!   workspace writes.
//!
//! [`execute_code`] picks between them based on [`Isolation`] and whether an OS
//! sandbox backend is available.

use std::sync::Arc;
