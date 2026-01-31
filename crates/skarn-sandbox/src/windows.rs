//! Windows backend — AppContainer isolation plus a Job Object for resource
//! limits and tree-kill.
//!
//! Unlike Unix, a Windows process cannot move *itself* into an AppContainer, so
//! the parent must launch the worker into one. We:
//!
//! 1. Create (or reuse) an AppContainer profile and derive its SID.
//! 2. Grant that SID read/execute on the program and read/write on the
//!    workspace via ACLs (AppContainer processes are denied by default).
//! 3. Attach `SECURITY_CAPABILITIES` to a `STARTUPINFOEXW` attribute list.
//! 4. `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`.
//! 5. Assign the child to a Job Object with kill-on-close so the whole tree
//!    dies with the parent.
//!
//! This file is compiled and tested in CI on Windows runners.

#![cfg(windows)]

use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;

use skarn_common::{CommandSpec, Error, Result};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, HLOCAL, LocalFree,
    SetHandleInformation,
};
use windows::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
    SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows::Win32::Security::{
    ACL, CopySid, DACL_SECURITY_INFORMATION, DeriveCapabilitySidsFromName, GetLengthSid, PSID,
    SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES,
    SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows::Win32::Storage::FileSystem::{
    FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, ReadFile,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_LIMIT_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, UpdateProcThreadAttribute,
    WaitForSingleObject,
};
use windows::core::{BOOL, PCWSTR, PWSTR};

use crate::{Backend, NetPolicy, Policy, RestrictionReport, RestrictionStatus};

/// `SE_GROUP_ENABLED` — the capability SID is active in the token.
const SE_GROUP_ENABLED: u32 = 0x0000_0004;

const APPCONTAINER_NAME: &str = "Skarn.Sandbox";

/// A handle to a process running inside an AppContainer + Job Object, with the
/// parent's read ends of the child's stdout/stderr pipes.
pub struct SandboxChild {
    process: HANDLE,
    thread: HANDLE,
    job: HANDLE,
    stdout_read: HANDLE,
    stderr_read: HANDLE,
}

/// Captured output of a sandboxed child.
pub struct Captured {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub code: i32,
}

impl SandboxChild {
    /// Wait for the child to exit and return its exit code (output discarded).
    pub fn wait(&self) -> Result<i32> {
        // SAFETY: `process` is a valid handle until `Drop`.
        unsafe {
            WaitForSingleObject(self.process, u32::MAX);
            let mut code = 0u32;
            GetExitCodeProcess(self.process, &mut code)
                .map_err(|e| Error::sandbox(format!("GetExitCodeProcess: {e}")))?;
            Ok(code as i32)
        }
    }

    /// Drain the child's stdout and stderr to EOF (on separate threads, to avoid
    /// the classic full-pipe deadlock), wait for exit, and return the captured
    /// output and exit code.
    pub fn wait_with_output(self) -> Result<Captured> {
        // `HANDLE` is `!Send`; pass the raw value across the thread boundary.
        let out_val = self.stdout_read.0 as isize;
        let err_val = self.stderr_read.0 as isize;
        let out_thread = std::thread::spawn(move || drain_pipe(out_val));
        let err_thread = std::thread::spawn(move || drain_pipe(err_val));

        // SAFETY: `process` is valid until `self` (and thus `Drop`) ends, which
        // is after the joins below.
        unsafe { WaitForSingleObject(self.process, u32::MAX) };

        let stdout = out_thread.join().unwrap_or_default();
        let stderr = err_thread.join().unwrap_or_default();

        let mut code = 0u32;
        // SAFETY: `process` is a valid handle.
        unsafe {
            GetExitCodeProcess(self.process, &mut code)
                .map_err(|e| Error::sandbox(format!("GetExitCodeProcess: {e}")))?;
        }
        Ok(Captured {
            stdout,
            stderr,
            code: code as i32,
        })
    }
}

impl Drop for SandboxChild {
    fn drop(&mut self) {
        // SAFETY: closing valid handles; ignore errors during teardown.
        unsafe {
            let _ = CloseHandle(self.stdout_read);
            let _ = CloseHandle(self.stderr_read);
            let _ = CloseHandle(self.thread);
            let _ = CloseHandle(self.process);
            // Closing the job kills the tree (KILL_ON_JOB_CLOSE).
            let _ = CloseHandle(self.job);
        }
    }
}

/// Read a pipe handle to EOF. Any read error (notably `ERROR_BROKEN_PIPE` once
/// every write end is closed) ends the drain.
fn drain_pipe(handle_val: isize) -> Vec<u8> {
    let handle = HANDLE(handle_val as *mut c_void);
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let mut read = 0u32;
        // SAFETY: `handle` is a valid pipe read end owned by the caller for the
        // duration of this drain.
        match unsafe { ReadFile(handle, Some(&mut buf), Some(&mut read), None) } {
            Ok(()) => {
                if read == 0 {
                    break; // EOF
                }
                out.extend_from_slice(&buf[..read as usize]);
            }
            Err(e) => {
                if e.code() != ERROR_BROKEN_PIPE.to_hresult() {
                    tracing::debug!(error = %e, "sandbox pipe read ended");
                }
                break;
            }
        }
    }
    out
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// A process cannot move *itself* into an AppContainer on Windows, so the
/// self-apply path is unsupported here; callers must use [`spawn_appcontainer`].
pub fn apply(_policy: &Policy) -> Result<RestrictionReport> {
    Err(Error::SandboxUnsupported(
        "Windows cannot sandbox the current process; launch via spawn_appcontainer".to_string(),
    ))
}

/// Probe AppContainer availability. Always supported on Windows 8+, which is our
/// minimum, so this reports `FullyEnforced`.
pub fn probe() -> RestrictionReport {
    RestrictionReport::new(Backend::AppContainer, RestrictionStatus::FullyEnforced)
        .note("Windows AppContainer available")
}

/// Launch `spec` inside an AppContainer governed by `policy`, capturing its
/// stdout and stderr through pipes (stdin is closed → the child sees EOF).
pub fn spawn_appcontainer(policy: &Policy, spec: &CommandSpec) -> Result<SandboxChild> {
    let policy = policy.canonicalized();
    // SAFETY: a long unsafe block of Win32 calls; each is documented inline.
    unsafe {
        let sid = create_or_derive_sid()?;

        // Grant the AppContainer SID the access the policy implies.
        grant_access(
            sid,
            &spec.program,
            FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0,
        )?;
        for p in &policy.fs_read {
            grant_access(sid, &p.to_string_lossy(), FILE_GENERIC_READ.0)?;
        }
        for p in &policy.fs_read_write {
            grant_access(
                sid,
                &p.to_string_lossy(),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | FILE_GENERIC_EXECUTE.0,
            )?;
        }

        // Network is granted by attaching well-known capability SIDs. With none,
        // a default-deny AppContainer blocks all network (NetPolicy::DenyAll).
        // `_sid_bufs` owns the SID memory `cap_attrs` points into; both must live
        // until after CreateProcessW.
        let (mut cap_attrs, _sid_bufs) = capability_sids(policy.net)?;
        let security_caps = SECURITY_CAPABILITIES {
            AppContainerSid: sid,
            Capabilities: if cap_attrs.is_empty() {
                std::ptr::null_mut()
            } else {
                cap_attrs.as_mut_ptr()
            },
            CapabilityCount: cap_attrs.len() as u32,
            ..Default::default()
        };

        // Build the proc-thread attribute list carrying the capabilities.
        let mut size = 0usize;
        let _ = InitializeProcThreadAttributeList(None, 1, None, &mut size);
        let mut attr_buf = vec![0u8; size];
        let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_buf.as_mut_ptr() as _);
        InitializeProcThreadAttributeList(Some(attr_list), 1, None, &mut size)
            .map_err(|e| Error::sandbox(format!("InitializeProcThreadAttributeList: {e}")))?;
        UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            Some(&security_caps as *const _ as *const _),
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            None,
            None,
        )
        .map_err(|e| Error::sandbox(format!("UpdateProcThreadAttribute: {e}")))?;

        // Inheritable pipes for the child's stdio. The parent's read ends (and
        // the stdin write end) are marked non-inheritable so the child does not
        // hold a copy that would prevent EOF.
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: BOOL::from(true),
        };
        let (out_read, out_write) = make_pipe(&sa, PipeEnd::Read)?;
        let (err_read, err_write) = make_pipe(&sa, PipeEnd::Read)?;
        let (in_read, in_write) = make_pipe(&sa, PipeEnd::Write)?;

        let si = STARTUPINFOEXW {
            StartupInfo: STARTUPINFOW {
                cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: in_read,
                hStdOutput: out_write,
                hStdError: err_write,
                ..Default::default()
            },
            lpAttributeList: attr_list,
        };
