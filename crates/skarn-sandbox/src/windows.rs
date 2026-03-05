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
        let mut pi = PROCESS_INFORMATION::default();

        let mut cmdline = build_command_line(spec);
        let cwd = policy
            .fs_read_write
            .first()
            .map(|p| wide(&p.to_string_lossy()));

        CreateProcessW(
            PCWSTR::null(),
            Some(PWSTR(cmdline.as_mut_ptr())),
            None,
            None,
            true, // inherit the stdio pipe handles
            PROCESS_CREATION_FLAGS(EXTENDED_STARTUPINFO_PRESENT.0),
            None,
            cwd.as_ref()
                .map(|c| PCWSTR(c.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            &si.StartupInfo,
            &mut pi,
        )
        .map_err(|e| Error::sandbox(format!("CreateProcessW: {e}")))?;

        DeleteProcThreadAttributeList(attr_list);

        // Close the ends the child owns so our reads see EOF when it exits, and
        // close the unused stdin pipe entirely (the child gets immediate EOF).
        let _ = CloseHandle(out_write);
        let _ = CloseHandle(err_write);
        let _ = CloseHandle(in_read);
        let _ = CloseHandle(in_write);

        // Assign to a kill-on-close Job Object.
        let job = CreateJobObjectW(None, PCWSTR::null())
            .map_err(|e| Error::sandbox(format!("CreateJobObjectW: {e}")))?;
        let info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .map_err(|e| Error::sandbox(format!("SetInformationJobObject: {e}")))?;
        AssignProcessToJobObject(job, pi.hProcess)
            .map_err(|e| Error::sandbox(format!("AssignProcessToJobObject: {e}")))?;

        Ok(SandboxChild {
            process: pi.hProcess,
            thread: pi.hThread,
            job,
            stdout_read: out_read,
            stderr_read: err_read,
        })
    }
}

/// Which end of a freshly-created pipe the parent keeps (and must therefore make
/// non-inheritable so the child cannot hold a copy).
enum PipeEnd {
    Read,
    Write,
}

/// Create a pipe and return `(read_end, write_end)`, clearing the inherit flag on
/// the end the parent keeps.
///
/// # Safety
/// `sa` must be a valid `SECURITY_ATTRIBUTES`.
unsafe fn make_pipe(sa: &SECURITY_ATTRIBUTES, parent: PipeEnd) -> Result<(HANDLE, HANDLE)> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe {
        CreatePipe(&mut read, &mut write, Some(sa as *const _), 0)
            .map_err(|e| Error::sandbox(format!("CreatePipe: {e}")))?;
        let parent_handle = match parent {
            PipeEnd::Read => read,
            PipeEnd::Write => write,
        };
        SetHandleInformation(parent_handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
            .map_err(|e| Error::sandbox(format!("SetHandleInformation: {e}")))?;
    }
    Ok((read, write))
}

/// Build the AppContainer network capability SIDs for `net`. Returns the
/// `SID_AND_ATTRIBUTES` array and the buffers that own the SID bytes (the array
/// borrows them, so the caller must keep both alive together).
fn capability_sids(net: NetPolicy) -> Result<(Vec<SID_AND_ATTRIBUTES>, Vec<Vec<u8>>)> {
    let names: &[&str] = match net {
        NetPolicy::DenyAll => &[],
        NetPolicy::AllowLoopback => {
            tracing::warn!(
                "AppContainer cannot express loopback-only networking; treating as deny-all"
            );
            &[]
        }
        NetPolicy::AllowOutbound => &["internetClient"],
        NetPolicy::AllowAll => &[
            "internetClient",
            "internetClientServer",
            "privateNetworkClientServer",
        ],
    };

    let mut attrs = Vec::with_capacity(names.len());
    let mut bufs = Vec::with_capacity(names.len());
    for name in names {
        let buf = derive_capability_sid(name)?;
        // The `Vec<u8>` heap allocation is stable when moved into `bufs`.
        attrs.push(SID_AND_ATTRIBUTES {
            Sid: PSID(buf.as_ptr() as *mut c_void),
            Attributes: SE_GROUP_ENABLED,
        });
        bufs.push(buf);
    }
    Ok((attrs, bufs))
}

/// Derive the well-known capability SID for `name` and copy it into an owned
/// buffer (freeing the OS-allocated arrays).
fn derive_capability_sid(name: &str) -> Result<Vec<u8>> {
    let wname = wide(name);
    let mut group_sids: *mut PSID = std::ptr::null_mut();
    let mut group_count = 0u32;
    let mut cap_sids: *mut PSID = std::ptr::null_mut();
    let mut cap_count = 0u32;

    // SAFETY: out-params are valid; `wname` is a NUL-terminated wide string.
    unsafe {
        DeriveCapabilitySidsFromName(
            PCWSTR(wname.as_ptr()),
            &mut group_sids,
            &mut group_count,
            &mut cap_sids,
            &mut cap_count,
        )
        .map_err(|e| Error::sandbox(format!("DeriveCapabilitySidsFromName({name}): {e}")))?;

        let copied = if cap_sids.is_null() || cap_count == 0 {
            Err(Error::sandbox(format!(
                "no capability SID derived for {name}"
            )))
        } else {
            let src = *cap_sids; // first capability SID
            let len = GetLengthSid(src);
            if len == 0 {
                Err(Error::sandbox(format!(
                    "zero-length capability SID for {name}"
                )))
            } else {
                let mut buf = vec![0u8; len as usize];
                CopySid(len, PSID(buf.as_mut_ptr() as *mut c_void), src)
                    .map(|()| buf)
                    .map_err(|e| Error::sandbox(format!("CopySid({name}): {e}")))
            }
        };

        // Free the OS-allocated SID arrays regardless of outcome.
        if !group_sids.is_null() {
            LocalFree(Some(HLOCAL(group_sids as *mut c_void)));
        }
        if !cap_sids.is_null() {
            LocalFree(Some(HLOCAL(cap_sids as *mut c_void)));
        }
        copied
    }
}

fn create_or_derive_sid() -> Result<PSID> {
    let name = wide(APPCONTAINER_NAME);
    let display = wide("Skarn Sandbox");
    let desc = wide("Confines LLM-generated code and untrusted commands");
    // SAFETY: all arguments are valid NUL-terminated wide strings.
    unsafe {
        match CreateAppContainerProfile(
            PCWSTR(name.as_ptr()),
            PCWSTR(display.as_ptr()),
            PCWSTR(desc.as_ptr()),
            None,
        ) {
            Ok(sid) => Ok(sid),
            Err(_) => {
                // Already exists — derive the SID from the name.
                DeriveAppContainerSidFromAppContainerName(PCWSTR(name.as_ptr()))
                    .map_err(|e| Error::sandbox(format!("DeriveAppContainerSid: {e}")))
            }
        }
    }
}

/// Grant `sid` `access_mask` on the filesystem object at `path` (inheritable).
fn grant_access(sid: PSID, path: &str, access_mask: u32) -> Result<()> {
    let mut wpath = wide(path);
    let mut old_dacl: *mut ACL = std::ptr::null_mut();
    let mut sec_desc = windows::Win32::Security::PSECURITY_DESCRIPTOR::default();
    // SAFETY: FFI calls into the Win32 authorization APIs with valid pointers;
    // each return code is checked. `wpath` outlives the calls.
    unsafe {
        let rc = GetNamedSecurityInfoW(
            PCWSTR(wpath.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            &mut sec_desc,
        );
        if rc.is_err() {
            // Path may not exist; skip (mirrors the Unix backends, which also
            // tolerate missing policy paths) but record it for diagnostics.
            tracing::debug!(path, "skipping ACL grant for missing path");
            return Ok(());
        }

        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: PWSTR(sid.0 as *mut u16),
        };
        let ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: access_mask,
            grfAccessMode: GRANT_ACCESS,
