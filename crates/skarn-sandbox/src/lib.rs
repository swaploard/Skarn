//! OS-native process sandboxing with a single, type-safe API.
//!
//! `skarn-sandbox` abstracts three very different kernel mechanisms behind one
//! [`Policy`]:
//!
//! | Platform | Mechanism | Backend |
//! |----------|-----------|---------|
//! | macOS    | Seatbelt (`sandbox_init`) | [`Backend::Seatbelt`] |
//! | Linux    | Landlock LSM + seccomp-bpf | [`Backend::Landlock`] |
//! | Windows  | AppContainer + Job Object  | [`Backend::AppContainer`] |
//!
//! # Execution model
//!
//! The most robust way to confine *arbitrary* programs (including a program we
//! do not control, like `cat`) is to run them through a **worker that is born
//! sandboxed**. On Unix the worker calls [`apply_to_current_process`] as its
//! very first action — while it is still single-threaded, which avoids the
//! classic "fork in a multi-threaded process then call a non-async-signal-safe
//! function" deadlock — and then `exec`s the target. Landlock domains, seccomp
//! filters, and the Seatbelt profile all persist across `execve`, so the target
//! inherits the confinement. On Windows a process cannot move *itself* into an
//! AppContainer, so the parent launches the worker into one with
//! [`spawn_appcontainer`].
//!
//! The [`skarn`] CLI wires this together; this crate only provides the
//! primitives and the [`Policy`] type.
//!
//! [`skarn`]: https://crates.io/crates/skarn

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;
#[cfg(windows)]
pub use windows::{Captured, SandboxChild, spawn_appcontainer};

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use unsupported as imp;

pub use skarn_common::{Error, Result};

/// A network access policy.
///
/// Note the platform caveats: macOS Seatbelt can express all four variants;
/// Linux Landlock is *port*-based, so [`NetPolicy::AllowLoopback`] cannot be
/// expressed precisely and is treated as "allow outbound" there (documented in
/// the per-rule notes of the returned [`RestrictionReport`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetPolicy {
    /// Deny all network access (the secure default).
    #[default]
