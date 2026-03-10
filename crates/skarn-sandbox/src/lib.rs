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
            fs_exec: canon(&self.fs_exec),
            fs_deny_read: canon(&self.fs_deny_read),
            net: self.net,
            allow_read_system: self.allow_read_system,
            fail_closed: self.fail_closed,
        }
    }
}

/// Builder for [`Policy`].
#[derive(Clone, Debug)]
pub struct PolicyBuilder {
    policy: Policy,
}

impl PolicyBuilder {
    /// Confine to a single project directory: read+write the directory, read
    /// (and exec) the system directories, deny the user's secret stores, and
    /// deny network. This is the right default for `skarn run`.
    pub fn workspace(mut self, dir: impl AsRef<Path>) -> Self {
        self.policy.fs_read_write.push(dir.as_ref().to_path_buf());
        self.policy.fs_deny_read.extend(default_secret_paths());
        self.policy.allow_read_system = true;
        self.policy.net = NetPolicy::DenyAll;
        self
    }

    /// Mark a subtree as a secret that must not be readable.
    pub fn deny_read(mut self, dir: impl AsRef<Path>) -> Self {
        self.policy.fs_deny_read.push(dir.as_ref().to_path_buf());
        self
    }

    /// Allow reading a subtree.
    pub fn read(mut self, dir: impl AsRef<Path>) -> Self {
        self.policy.fs_read.push(dir.as_ref().to_path_buf());
        self
    }

    /// Allow reading and writing a subtree.
    pub fn read_write(mut self, dir: impl AsRef<Path>) -> Self {
        self.policy.fs_read_write.push(dir.as_ref().to_path_buf());
        self
    }

    /// Allow executing binaries from a subtree.
    pub fn exec(mut self, dir: impl AsRef<Path>) -> Self {
        self.policy.fs_exec.push(dir.as_ref().to_path_buf());
        self
    }

    /// Set the network policy.
    pub fn net(mut self, net: NetPolicy) -> Self {
        self.policy.net = net;
        self
    }

    /// Allow (or forbid) reading the standard system directories.
    pub fn allow_read_system(mut self, yes: bool) -> Self {
        self.policy.allow_read_system = yes;
        self
    }

    /// Whether to fail closed when the backend cannot enforce the policy.
    pub fn fail_closed(mut self, yes: bool) -> Self {
        self.policy.fail_closed = yes;
        self
    }

    /// Finish building.
    pub fn build(self) -> Policy {
        self.policy
    }
}

/// Which kernel mechanism is in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    /// macOS Seatbelt (`sandbox_init`).
    Seatbelt,
    /// Linux Landlock LSM (+ seccomp-bpf).
    Landlock,
    /// Windows AppContainer.
    AppContainer,
    /// No sandbox available on this platform.
    None,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Backend::Seatbelt => "Seatbelt",
            Backend::Landlock => "Landlock",
            Backend::AppContainer => "AppContainer",
            Backend::None => "None",
        };
        f.write_str(s)
    }
}

/// How completely a policy was (or would be) enforced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestrictionStatus {
    /// The full policy is enforced.
    FullyEnforced,
    /// Some of the policy is enforced; the kernel is too old for the rest.
    PartiallyEnforced,
    /// Nothing is enforced (no backend / unsupported kernel).
    NotEnforced,
}

/// The result of applying (or probing) a sandbox.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestrictionReport {
    /// The backend that handled (or would handle) the policy.
    pub backend: Backend,
    /// Enforcement completeness.
    pub status: RestrictionStatus,
    /// Human-readable notes (e.g. degraded ABI levels, network caveats).
    pub notes: Vec<String>,
}

impl RestrictionReport {
    pub(crate) fn new(backend: Backend, status: RestrictionStatus) -> Self {
        Self {
            backend,
            status,
            notes: Vec::new(),
        }
    }

    pub(crate) fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Describe the sandbox backend available on this host **without applying it**.
///
/// Used by `skarn doctor`. The reported [`RestrictionStatus`] reflects whether
/// the kernel actually supports the mechanism (e.g. Landlock on the running
/// kernel, or `sandbox_init` being present).
pub fn backend_report() -> RestrictionReport {
    imp::probe()
}

/// The backend that this build targets.
pub const fn backend() -> Backend {
    #[cfg(target_os = "macos")]
    {
        Backend::Seatbelt
    }
    #[cfg(target_os = "linux")]
    {
        Backend::Landlock
    }
    #[cfg(windows)]
    {
        Backend::AppContainer
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        Backend::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_workspace_defaults() {
        let p = Policy::builder().workspace("/tmp/project").build();
        assert_eq!(p.fs_read_write, vec![PathBuf::from("/tmp/project")]);
        assert_eq!(p.net, NetPolicy::DenyAll);
        assert!(p.allow_read_system);
        assert!(p.fail_closed);
    }

    #[test]
    fn builder_chains() {
        let p = Policy::builder()
            .read("/etc/hosts")
            .read_write("/work")
            .exec("/usr/bin")
            .net(NetPolicy::AllowLoopback)
            .fail_closed(false)
