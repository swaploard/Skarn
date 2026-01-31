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
    DenyAll,
    /// Allow loopback traffic only (localhost).
    AllowLoopback,
    /// Allow outbound connections but deny inbound binds.
    AllowOutbound,
    /// Allow all network access (escape hatch; discouraged).
    AllowAll,
}

/// A declarative description of what a sandboxed process may do.
///
/// Build one with [`Policy::builder`]. The common case — confine a process to a
/// project directory with no network — is [`PolicyBuilder::workspace`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    /// Subtrees the process may read.
    pub fs_read: Vec<PathBuf>,
    /// Subtrees the process may read and write.
    pub fs_read_write: Vec<PathBuf>,
    /// Subtrees the process may execute binaries from. Empty means "no extra
    /// exec restriction beyond the system defaults" (see `allow_read_system`).
    pub fs_exec: Vec<PathBuf>,
    /// Secret subtrees that must NOT be readable (SSH keys, cloud credentials).
    /// Honored by backends that allow broad reads (macOS); on the allow-list
    /// backends (Linux) these are simply never granted in the first place.
    pub fs_deny_read: Vec<PathBuf>,
    /// Network policy.
    pub net: NetPolicy,
    /// Allow read (and exec) of the standard system directories so dynamically
    /// linked programs can actually start. Almost always `true`.
    pub allow_read_system: bool,
    /// If the active backend cannot enforce this policy, refuse to run rather
    /// than running unconfined. Defaults to `true` (fail closed).
    pub fail_closed: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            fs_read: Vec::new(),
            fs_read_write: Vec::new(),
            fs_exec: Vec::new(),
            fs_deny_read: Vec::new(),
            net: NetPolicy::DenyAll,
            allow_read_system: true,
            fail_closed: true,
        }
    }
}

/// Well-known secret locations under the user's home directory that should not
/// be readable by sandboxed code, even when broad reads are permitted.
pub fn default_secret_paths() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    [
        ".ssh",
        ".aws",
        ".gnupg",
        ".kube",
        ".docker",
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".config/gcloud",
        ".config/gh",
        ".cargo/credentials",
        ".cargo/credentials.toml",
    ]
    .iter()
    .map(|s| home.join(s))
    .collect()
}

impl Policy {
    /// Start building a policy.
    pub fn builder() -> PolicyBuilder {
        PolicyBuilder {
            policy: Policy::default(),
        }
    }

    /// Apply this policy to the **current process**, irreversibly.
    ///
    /// On Unix this calls the kernel sandbox primitive directly (Seatbelt on
    /// macOS, Landlock + seccomp on Linux). On Windows this is not possible — a
    /// process cannot move itself into an AppContainer — so it returns
    /// [`Error::SandboxUnsupported`]; use [`spawn_appcontainer`] instead.
    ///
    /// Call this as early as possible in a worker process, before spawning
    /// threads, so the restriction is inherited by everything that follows.
    pub fn apply_to_current_process(&self) -> Result<RestrictionReport> {
        let report = imp::apply(self);
        match report {
            Ok(r) => {
                if r.status == RestrictionStatus::NotEnforced && self.fail_closed {
                    return Err(Error::SandboxUnsupported(format!(
                        "{} backend could not enforce the policy and fail_closed is set",
                        r.backend
                    )));
                }
                Ok(r)
            }
            Err(e) => Err(e),
        }
    }

    /// Canonicalize all paths in the policy, dropping any that do not exist.
    ///
    /// Kernel sandboxes generally require real, absolute paths. This is applied
    /// automatically by the backends, but is exposed for inspection/tests.
    pub fn canonicalized(&self) -> Policy {
        fn canon(paths: &[PathBuf]) -> Vec<PathBuf> {
            paths
                .iter()
                .filter_map(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.clone())))
                .collect()
        }
        Policy {
            fs_read: canon(&self.fs_read),
            fs_read_write: canon(&self.fs_read_write),
