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
