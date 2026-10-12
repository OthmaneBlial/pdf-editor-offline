//! Windows journal privacy. All FFI is isolated here; returned allocations/handles have RAII.
//! This is compile-checked separately; runtime ACL/reparse evidence requires Windows.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, FILE_ALL_ACCESS, FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FileAttributeTagInfo,
    GetFileInformationByHandleEx,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns a successfully opened token handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: Windows security conversion/query functions allocate with LocalAlloc.
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn rejected(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

fn terminated_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "journal path contains a NUL unit",
        ));
    }
    units.push(0);
    Ok(units)
}

struct CurrentSid {
    storage: Vec<u8>,
    offset: usize,
}
impl CurrentSid {
    fn pointer(&self) -> PSID {
        self.storage
            .as_ptr()
            .wrapping_add(self.offset)
            .cast_mut()
            .cast()
    }
}
fn current_sid() -> io::Result<CurrentSid> {
    let mut raw = ptr::null_mut();
    // SAFETY: valid output pointer, process pseudo-handle is never owned/closed here.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(raw);
    let mut length = 0;
    // SAFETY: null/zero buffer is the documented length-query pattern.
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length);
    }
    if length < size_of::<TOKEN_USER>() as u32 || length > 4096 {
        return Err(rejected("invalid current SID size"));
    }
    let mut storage = vec![0u8; length as usize];
    // SAFETY: writable buffer length is checked and passed exactly to the API.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: API filled TOKEN_USER; unaligned read avoids relying on Vec<u8> alignment.
    let user = unsafe { ptr::read_unaligned(storage.as_ptr().cast::<TOKEN_USER>()) };
    let start = storage.as_ptr() as usize;
    let offset = (user.User.Sid as usize)
        .checked_sub(start)
        .ok_or_else(|| rejected("invalid SID pointer"))?;
    if offset > storage.len().saturating_sub(8) {
        return Err(rejected("SID pointer outside token buffer"));
    }
    // SAFETY: token API owns the validated SID inside the still-live storage allocation.
    if unsafe { IsValidSid(user.User.Sid) } == 0 {
        return Err(rejected("invalid current SID"));
    }
    // SAFETY: IsValidSid succeeded and TOKEN_USER API guarantees its SID is in the buffer.
    let sid_length = unsafe { GetLengthSid(user.User.Sid) } as usize;
    if offset
        .checked_add(sid_length)
        .is_none_or(|end| end > storage.len())
    {
        return Err(rejected("SID outside token buffer"));
    }
    Ok(CurrentSid { storage, offset })
}

pub(super) fn configure_options(options: &mut OpenOptions) {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
}

pub(super) fn open_pinned_directory(path: &Path, deletable: bool) -> io::Result<File> {
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let mut options = OpenOptions::new();
    configure_options(&mut options);
    // Denying delete sharing pins the pathname while all child files are inspected/removed.
    options.access_mode(FILE_GENERIC_READ | if deletable { DELETE } else { 0 });
    options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let file = options.open(path)?;
    validate_private(&file, true)?;
    Ok(file)
}

pub(super) fn open_deletable_file(path: &Path) -> io::Result<File> {
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let mut options = OpenOptions::new();
    configure_options(&mut options);
    options
        .access_mode(FILE_GENERIC_READ | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    let file = options.open(path)?;
    validate_private(&file, false)?;
    let mut information: windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION =
        unsafe { std::mem::zeroed() };
    // SAFETY: live file handle, fixed writable output structure of documented API type.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(
            file.as_raw_handle(),
            &mut information,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if information.nNumberOfLinks != 1 {
        return Err(rejected("discard refuses hard-linked files"));
    }
    if !file.metadata()?.is_file() || file.metadata()?.len() > super::super::MAX_PDF_BYTES as u64 {
        return Err(rejected("discard file is not a bounded regular file"));
    }
    Ok(file)
}

pub(super) fn delete_handle(file: &File) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_DISPOSITION_INFO, FileDispositionInfo, SetFileInformationByHandle,
    };
    let information = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: a live exclusively pinned DELETE-capable handle and exact writable-structure size.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            (&information as *const FILE_DISPOSITION_INFO).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn verify_path_identity(file: &File, path: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        GetFileInformationByHandle,
    };
    let mut options = OpenOptions::new();
    configure_options(&mut options);
    let current = options
        .read(true)
        .share_mode(FILE_SHARE_DELETE | FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)?;
    reject_reparse(&current)?;
    let identity = |handle: &File| -> io::Result<(u32, u32, u32)> {
        let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: live handle and fixed writable documented file identity structure.
        if unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &mut information) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((
            information.dwVolumeSerialNumber,
            information.nFileIndexHigh,
            information.nFileIndexLow,
        ))
    };
    if identity(file)? != identity(&current)? {
        return Err(rejected("recovery directory was replaced"));
    }
    Ok(())
}

pub(super) fn create_directory(path: &Path) -> io::Result<()> {
    let security = private_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let path = terminated_path(path)?;
    // SAFETY: both structures remain live, path is NUL-terminated. DACL/owner are private at creation.
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn create_private_file(path: &Path, exclusive: bool) -> io::Result<File> {
    use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_ALWAYS,
    };
    let security = private_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let path = terminated_path(path)?;
    // SAFETY: valid live security descriptor, NUL-terminated native path, no handle inheritance.
    // OPEN_ALWAYS ignores the descriptor on existing files; callers still validate their owner/DACL.
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            if exclusive { CREATE_NEW } else { OPEN_ALWAYS },
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateFileW returned the exclusively owned, non-inheritable handle.
    let file = unsafe { File::from_raw_handle(raw) };
    validate_private(&file, false)?;
    Ok(file)
}

fn private_descriptor() -> io::Result<LocalAllocation> {
    let sid = current_sid()?;
    let mut text = ptr::null_mut();
    // SAFETY: SID storage is live, writable output pointer receives LocalAlloc string.
    if unsafe { ConvertSidToStringSidW(sid.pointer(), &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _text_owner = LocalAllocation(text.cast());
    let mut units = Vec::new();
    for index in 0..256 {
        // SAFETY: a valid SID conversion returns a NUL-terminated string shorter than 256 WCHARs.
        let unit = unsafe { *text.add(index) };
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    let sid_text = String::from_utf16(&units).map_err(|_| rejected("SID string invalid"))?;
    let descriptor: Vec<u16> = format!("O:{sid_text}D:P(A;OICI;FA;;;{sid_text})")
        .encode_utf16()
        .chain([0])
        .collect();
    let mut security = ptr::null_mut();
    // SAFETY: descriptor is NUL-terminated, output pointer receives a LocalAlloc descriptor.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut security,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(LocalAllocation(security))
}

pub(super) fn validate_directory(path: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    configure_options(&mut options);
    let file = options.read(true).open(path)?;
    validate_private(&file, true)
}

pub(super) fn reject_reparse(file: &File) -> io::Result<()> {
    let mut attributes: FILE_ATTRIBUTE_TAG_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: live std File handle; fixed-size writable struct with exact API size.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            (&mut attributes as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if attributes.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(rejected("journal reparse points are refused"));
    }
    Ok(())
}

pub(super) fn validate_private(file: &File, directory: bool) -> io::Result<()> {
    reject_reparse(file)?;
    let current = current_sid()?;
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: handle is live; output pointers are valid. All returned pointers belong to descriptor.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor_owner = LocalAllocation(descriptor);
    // SAFETY: query returned live security descriptor/SID/ACL pointers. Null ACL grants everyone, refused.
    if owner.is_null()
        || acl.is_null()
        || unsafe { IsValidSecurityDescriptor(descriptor) } == 0
        || unsafe { IsValidSid(owner) } == 0
        || unsafe { IsValidAcl(acl) } == 0
    {
        return Err(rejected("journal owner/DACL is not private"));
    }
    // TokenOwner may be the Administrators group, even though TokenUser remains the account SID.
    // Files we create explicitly use TokenUser; never accept or relabel an existing foreign owner.
    if unsafe { EqualSid(owner, current.pointer()) } == 0 {
        return Err(rejected("journal owner differs from current process user"));
    }
    let mut control = 0u16;
    let mut revision = 0;
    // SAFETY: validated descriptor and writable scalar outputs.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if directory && control & SE_DACL_PROTECTED == 0 {
        return Err(rejected("session DACL must be protected from inheritance"));
    }
    let mut information: ACL_SIZE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: validated ACL, correctly sized writable information structure.
    if unsafe {
        GetAclInformation(
            acl,
            (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if information.AceCount == 0 || information.AceCount > 16 {
        return Err(rejected("unexpected journal DACL entries"));
    }
    let mut full_access = false;
    for index in 0..information.AceCount {
        let mut entry = ptr::null_mut();
        // SAFETY: bounded index in valid ACL, output points into live descriptor allocation.
        if unsafe { GetAce(acl, index, &mut entry) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every valid ACE includes ACE_HEADER; unaligned read supports API layout.
        let header = unsafe { ptr::read_unaligned(entry.cast::<ACE_HEADER>()) };
        if header.AceType != 0 || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>() {
            return Err(rejected("only current-user allow ACEs are supported"));
        }
        // SAFETY: size/type checks identify ACCESS_ALLOWED_ACE layout; SID starts at SidStart.
        let allowed = unsafe { ptr::read_unaligned(entry.cast::<ACCESS_ALLOWED_ACE>()) };
        let sid = entry
            .cast::<u8>()
            .wrapping_add(std::mem::offset_of!(ACCESS_ALLOWED_ACE, SidStart))
            .cast();
        // SAFETY: validated ACL/ACE supplies valid SID within live descriptor allocation.
        if unsafe { IsValidSid(sid) } == 0 || unsafe { EqualSid(sid, current.pointer()) } == 0 {
            return Err(rejected("journal DACL grants another identity"));
        }
        if allowed.Mask & FILE_ALL_ACCESS == FILE_ALL_ACCESS
            && u32::from(header.AceFlags) & INHERIT_ONLY_ACE == 0
        {
            full_access = true;
        }
    }
    if !full_access {
        return Err(rejected("current user lacks journal full access"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::io::Write;

    #[test]
    fn embedded_nul_creation_refuses_without_creating_truncated_file_or_directory() {
        use std::os::windows::ffi::OsStringExt;
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("private-parent");
        create_directory(&directory).unwrap();
        let units: Vec<u16> = "truncated\0suffix".encode_utf16().collect();
        let invalid = directory.join(std::ffi::OsString::from_wide(&units));
        assert_eq!(
            create_directory(&invalid).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        for exclusive in [false, true] {
            assert_eq!(
                create_private_file(&invalid, exclusive).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert!(!directory.join("truncated").exists());
        assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    }

    #[test]
    fn lock_blob_and_manifest_files_receive_explicit_user_owner_and_protected_dacl() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("private-session");
        create_directory(&directory).unwrap();
        validate_directory(&directory).unwrap();
        for name in ["session.lock", "opaque.blob", "manifest.json.tmp"] {
            let path = directory.join(name);
            let mut file = create_private_file(&path, true).unwrap();
            file.write_all(b"private test bytes").unwrap();
            file.sync_all().unwrap();
            // true additionally demands protected DACL; it does not relax owner/ACE identity checks.
            validate_private(&file, true).unwrap();
            drop(file);
            validate_private(&create_private_file(&path, false).unwrap(), true).unwrap();
            assert!(create_private_file(&path, true).is_err());
        }
    }

    #[test]
    fn opening_an_existing_untrusted_file_never_relabels_its_security_descriptor() {
        use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::Storage::FileSystem::{
            CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ,
            FILE_SHARE_WRITE,
        };
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("existing-untrusted.lock");
        let descriptor: Vec<u16> = "D:P(A;;FA;;;WD)".encode_utf16().chain([0]).collect();
        let mut security = ptr::null_mut();
        // SAFETY: test-only Everyone ACL, valid NUL-terminated descriptor and writable output.
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    descriptor.as_ptr(),
                    1,
                    &mut security,
                    ptr::null_mut(),
                )
            },
            0
        );
        let _security = LocalAllocation(security);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security,
            bInheritHandle: 0,
        };
        let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: local test fixture, live explicit untrusted ACL/path, new non-inheritable handle.
        let raw = unsafe {
            CreateFileW(
                wide_path.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                &attributes,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        // SAFETY: successful CreateFileW returned an exclusively owned fixture handle.
        let mut fixture = unsafe { File::from_raw_handle(raw) };
        fixture.write_all(b"must remain unchanged").unwrap();
        drop(fixture);
        // OPEN_ALWAYS must ignore our private descriptor for this existing world-accessible file.
        assert!(create_private_file(&path, false).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"must remain unchanged");
    }
}
