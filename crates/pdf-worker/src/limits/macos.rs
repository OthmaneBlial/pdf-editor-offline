//! Audited native observation of the current task's VM size. Darwin includes
//! large startup reservations in this figure; the cap bounds additional VM.
use crate::{Error, Result};

pub(super) fn resident_bytes(pid: u32) -> Result<u64> {
    let pid =
        i32::try_from(pid).map_err(|_| Error::Limits("child PID exceeds native range".into()))?;
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    // SAFETY: live owned child PID; matching public ABI flavor, aligned storage,
    // exact byte capacity. No pointers or document data come from the worker.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return Err(Error::Limits(format!(
            "child resident-memory observation unavailable: {}",
            std::io::Error::last_os_error()
        )));
    }
    // SAFETY: exact successful byte count initializes the entire ABI structure.
    Ok(unsafe { info.assume_init() }.pti_resident_size)
}
pub(super) fn address_limit(budget: u64) -> Result<u64> {
    let mut info = std::mem::MaybeUninit::<libc::mach_task_basic_info>::zeroed();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    // SAFETY: own task port; initialized count is the exact capacity in integer
    // words; out points to correctly aligned storage of that complete structure.
    // libc keeps the native ABI but recommends another crate; use the already
    // locked dependency at this narrowly audited call instead of adding mach2.
    #[allow(deprecated)]
    let status = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr().cast(),
            &mut count,
        )
    };
    if status != libc::KERN_SUCCESS || count != libc::MACH_TASK_BASIC_INFO_COUNT {
        return Err(Error::Limits(format!(
            "Mach VM baseline observation failed ({status}, count {count})"
        )));
    }
    // SAFETY: successful task_info and exact returned word count initialize all fields.
    let baseline = unsafe { info.assume_init() }.virtual_size;
    baseline
        .checked_add(budget)
        .ok_or_else(|| Error::Limits("Mach VM baseline plus budget overflow".into()))
}
