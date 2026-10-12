//! Failure-test-only fresh mapping probe. Never touches or fills mapped pages;
//! at most 96 MiB is reserved and successful mappings are immediately released.
use crate::{Error, Result};
pub fn allocation_probe(bytes: usize) -> Result<Option<i32>> {
    if bytes == 0 || bytes > 96 * 1024 * 1024 {
        return Err(Error::Protocol("allocation probe must be 1..96 MiB"));
    }
    #[cfg(unix)]
    {
        use rustix::mm::{MapFlags, ProtFlags, mmap_anonymous, munmap};
        // SAFETY: new anonymous private region, no pointer input or dereference.
        match unsafe {
            mmap_anonymous(
                std::ptr::null_mut(),
                bytes,
                ProtFlags::empty(),
                MapFlags::PRIVATE,
            )
        } {
            Ok(address) => {
                // SAFETY: exact base/length from successful mapping; no aliases exist.
                unsafe { munmap(address, bytes) }.map_err(std::io::Error::from)?;
                Ok(None)
            }
            Err(error) => Ok(Some(error.raw_os_error())),
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Memory::*;
        // SAFETY: null requests a new private region; no caller pointer is used.
        let address = unsafe {
            VirtualAlloc(
                std::ptr::null(),
                bytes,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            )
        };
        if address.is_null() {
            return Ok(Some(
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            ));
        }
        // SAFETY: owned allocation, exact base; MEM_RELEASE requires size zero.
        if unsafe { VirtualFree(address, 0, MEM_RELEASE) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(None)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::Limits("unsupported allocation probe".into()))
    }
}
