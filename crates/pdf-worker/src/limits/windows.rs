//! Audited FFI boundary: owned Job Object handle, valid live child handle and
//! initialized POD structures. No document data reaches these pointer operations.
use crate::{Error, Limits, Result};
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::*;

pub(crate) struct Guard(HANDLE);
impl Guard {
    pub(crate) fn attach(child: &Child, limits: Limits) -> Result<Self> {
        // SAFETY: null security/name creates a private noninheritable job handle.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(Error::Limits(std::io::Error::last_os_error().to_string()));
        }
        let guard = Self(handle);
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_PROCESS_TIME
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        info.BasicLimitInformation.PerProcessUserTimeLimit = (limits.cpu * 10_000_000) as i64;
        info.BasicLimitInformation.ActiveProcessLimit = 1;
        info.ProcessMemoryLimit = limits.memory as usize;
        info.JobMemoryLimit = limits.memory as usize;
        // SAFETY: handle is owned/live; pointer and exact size describe initialized POD.
        if unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            )
        } == 0
        {
            return Err(Error::Limits(std::io::Error::last_os_error().to_string()));
        }
        // SAFETY: std::process::Child owns a live process handle during this call.
        if unsafe { AssignProcessToJobObject(handle, child.as_raw_handle().cast()) } == 0 {
            return Err(Error::Limits(std::io::Error::last_os_error().to_string()));
        }
        Ok(guard)
    }
    pub(crate) fn terminate(&self, child: &mut Child) {
        // SAFETY: job handle remains owned/live until Drop; termination covers descendants.
        unsafe { TerminateJobObject(self.0, 1) };
        let _ = child.kill();
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: this handle was created once and is closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}
