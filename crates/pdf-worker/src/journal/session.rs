use super::*;
use std::ffi::OsString;

const MAX_SESSIONS: usize = 64;
const MAX_ENTRIES: usize = 512;

#[derive(Debug)]
pub enum SessionState {
    Available(SessionSummary),
    Active,
    Invalid(String),
}
#[derive(Debug)]
pub struct SessionCandidate {
    pub path: PathBuf,
    pub state: SessionState,
    /// Includes known backup/orphan files; unavailable for active or unsafe entries.
    pub disk_bytes: Option<u64>,
    pub modified: Option<std::time::SystemTime>,
}

/// A private, app-owned recovery root. Per-session locks permit concurrent app instances.
pub struct SessionStore {
    root: PathBuf,
    handle: File,
}

impl SessionStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = std::path::absolute(root)?;
        create_parents(
            root.parent()
                .ok_or(JournalError::Invalid("missing root parent"))?,
            0,
        )?;
        match root.symlink_metadata() {
            Ok(_) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                create_private_directory(&root)?;
                sync_directory(
                    root.parent()
                        .ok_or(JournalError::Invalid("missing root parent"))?,
                )?;
            }
            Err(error) => return Err(error.into()),
        }
        let handle = open_directory(&root, false)?;
        Ok(Self { root, handle })
    }

    pub fn create(&self) -> Result<Journal> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        self.check_root()?;
        if entries(&self.handle, &self.root)?.len() >= MAX_SESSIONS {
            return Err(JournalError::Limit("recovery root has sixty-four entries"));
        }
        for _ in 0..16 {
            let seed = format!(
                "{}-{:?}-{}",
                std::process::id(),
                std::time::SystemTime::now(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            let name = format!("session-{}", &hash_bytes(seed.as_bytes())[..32]);
            let path = self.root.join(name);
            #[cfg(unix)]
            {
                match rustix::fs::mkdirat(
                    &self.handle,
                    path.file_name()
                        .ok_or(JournalError::Invalid("missing session name"))?,
                    rustix::fs::Mode::from_raw_mode(0o700),
                ) {
                    Ok(()) => (),
                    Err(rustix::io::Errno::EXIST) => continue,
                    Err(error) => return Err(io::Error::from(error).into()),
                }
            }
            #[cfg(windows)]
            match windows::create_directory(&path) {
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
            #[cfg(unix)]
            self.handle.sync_all()?;
            return Journal::open(path);
        }
        Err(JournalError::Limit("session name collision"))
    }

    pub fn discover(&self) -> Result<Vec<SessionCandidate>> {
        self.check_root()?;
        let names = entries(&self.handle, &self.root)?;
        if names.len() > MAX_SESSIONS {
            return Err(JournalError::Limit(
                "recovery root has more than sixty-four entries",
            ));
        }
        let mut result = Vec::new();
        for name in names {
            if !valid_session_name(&name) {
                continue;
            }
            let path = self.root.join(name);
            // Discovery validates manifests/locks, not 64 sessions worth of PDF blob payloads.
            let (state, disk_bytes, modified) = match Journal::open_internal(&path, false, true) {
                Ok(journal) => {
                    let modified = journal.directory_handle.metadata()?.modified().ok();
                    match journal.owned_disk_bytes() {
                        Ok(bytes) => (
                            SessionState::Available(journal.summary()),
                            Some(bytes),
                            modified,
                        ),
                        Err(error) => (SessionState::Invalid(error.to_string()), None, modified),
                    }
                }
                Err(JournalError::Locked) => (SessionState::Active, None, None),
                Err(error) => {
                    let handle = open_directory(&path, true).ok();
                    let disk_bytes = handle
                        .as_ref()
                        .and_then(|handle| directory_disk_usage(handle, &path).ok());
                    let modified = handle
                        .and_then(|handle| handle.metadata().ok())
                        .and_then(|metadata| metadata.modified().ok());
                    (
                        SessionState::Invalid(error.to_string()),
                        disk_bytes,
                        modified,
                    )
                }
            };
            result.push(SessionCandidate {
                path,
                state,
                disk_bytes,
                modified,
            });
        }
        result.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(result)
    }

    pub fn open_session(&self, path: impl AsRef<Path>) -> Result<Journal> {
        self.check_session_path(path.as_ref())?;
        Journal::open(path)
    }

    /// Explicitly discard a corrupt or intact inactive session after locking and file preflight.
    pub fn discard_session(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        self.check_session_path(path)?;
        path.symlink_metadata()?;
        Journal::open_internal(path, false, false)?.discard()
    }

    fn check_session_path(&self, path: &Path) -> Result<()> {
        self.check_root()?;
        if path.parent() != Some(self.root.as_path())
            || !path.file_name().is_some_and(valid_session_name)
        {
            return Err(JournalError::Invalid("session is outside recovery root"));
        }
        Ok(())
    }

    fn check_root(&self) -> Result<()> {
        same_directory(&self.handle, &self.root)
    }
}

fn valid_session_name(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix("session-"))
        .is_some_and(|id| {
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

pub(super) fn open_directory(path: &Path, deletable: bool) -> Result<File> {
    #[cfg(unix)]
    {
        let _ = deletable;
        let fd = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(io::Error::from)?;
        let file = File::from(fd);
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o077 != 0 {
            return Err(JournalError::Invalid(
                "recovery directory owner/mode is not private",
            ));
        }
        Ok(file)
    }
    #[cfg(windows)]
    {
        Ok(windows::open_pinned_directory(path, deletable)?)
    }
}

fn same_directory(handle: &File, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = path.symlink_metadata()?;
        let pinned = handle.metadata()?;
        if !actual.is_dir()
            || actual.file_type().is_symlink()
            || actual.dev() != pinned.dev()
            || actual.ino() != pinned.ino()
        {
            return Err(JournalError::Invalid("recovery directory was replaced"));
        }
    }
    #[cfg(windows)]
    {
        windows::validate_private(handle, true)?;
        windows::verify_path_identity(handle, path)?;
    }
    Ok(())
}

pub(super) fn entries(handle: &File, path: &Path) -> Result<Vec<OsString>> {
    let mut names = Vec::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let _ = path;
        let directory = rustix::fs::Dir::read_from(handle).map_err(io::Error::from)?;
        for entry in directory {
            let entry = entry.map_err(io::Error::from)?;
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            names.push(std::ffi::OsStr::from_bytes(bytes).to_owned());
            if names.len() > MAX_ENTRIES {
                return Err(JournalError::Limit("more than 512 journal entries"));
            }
        }
    }
    #[cfg(windows)]
    {
        same_directory(handle, path)?;
        for entry in std::fs::read_dir(path)? {
            names.push(entry?.file_name());
            if names.len() > MAX_ENTRIES {
                return Err(JournalError::Limit("more than 512 journal entries"));
            }
        }
    }
    Ok(names)
}

fn allowed_file(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        matches!(
            name,
            "session.lock"
                | "manifest.json"
                | "manifest.previous.json"
                | "manifest.json.tmp"
                | "manifest.previous.json.tmp"
        ) || name
            .strip_suffix(".blob")
            .is_some_and(|hash| validate_hash(hash).is_ok())
    })
}

pub(super) fn disk_usage(journal: &Journal) -> Result<u64> {
    directory_disk_usage(&journal.directory_handle, &journal.directory)
}

fn directory_disk_usage(handle: &File, directory: &Path) -> Result<u64> {
    let mut size = 0u64;
    for name in entries(handle, directory)? {
        if !allowed_file(&name) {
            return Err(JournalError::Invalid("unknown journal entry"));
        }
        let name = name
            .to_str()
            .ok_or(JournalError::Invalid("invalid journal filename"))?;
        let file = open_private_relative(handle, directory, name, false)?;
        validate_handle(&file, crate::MAX_PDF_BYTES)?;
        size = size
            .checked_add(file.metadata()?.len())
            .ok_or(JournalError::Limit("journal disk byte overflow"))?;
    }
    Ok(size)
}

pub(super) fn discard(journal: Journal) -> Result<()> {
    let name = journal
        .directory
        .file_name()
        .ok_or(JournalError::Invalid("missing session name"))?;
    if !valid_session_name(name) {
        return Err(JournalError::Invalid(
            "discard requires an app-generated session name",
        ));
    }
    let parent_path = journal
        .directory
        .parent()
        .ok_or(JournalError::Invalid("missing session parent"))?;
    let parent = open_directory(parent_path, false)?;
    same_directory(&journal.directory_handle, &journal.directory)?;
    let mut names = entries(&journal.directory_handle, &journal.directory)?;
    names.sort_by_key(|name| name == "session.lock");
    let mut handles = Vec::new();
    // Validate every entry before deleting any. No recursion and no user output paths.
    for name in &names {
        if !allowed_file(name) {
            return Err(JournalError::Invalid(
                "unknown file prevents session discard",
            ));
        }
        #[cfg(unix)]
        {
            let fd = rustix::fs::openat(
                &journal.directory_handle,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(io::Error::from)?;
            let file = File::from(fd);
            validate_handle(&file, crate::MAX_PDF_BYTES)?;
            use std::os::unix::fs::MetadataExt;
            let metadata = file.metadata()?;
            if metadata.nlink() != 1 || metadata.uid() != rustix::process::geteuid().as_raw() {
                return Err(JournalError::Invalid(
                    "discard file ownership/link count invalid",
                ));
            }
            handles.push(file);
        }
        #[cfg(windows)]
        handles.push(windows::open_deletable_file(&journal.directory.join(name))?);
    }
    for (name, file) in names.iter().zip(&handles) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let current = rustix::fs::statat(
                &journal.directory_handle,
                name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(io::Error::from)?;
            let metadata = file.metadata()?;
            if current.st_ino != metadata.ino() || current.st_dev as u64 != metadata.dev() {
                return Err(JournalError::Invalid("discard file was replaced"));
            }
            rustix::fs::unlinkat(
                &journal.directory_handle,
                name,
                rustix::fs::AtFlags::empty(),
            )
            .map_err(io::Error::from)?;
        }
        #[cfg(windows)]
        {
            let _ = name;
            windows::delete_handle(file)?;
        }
    }
    #[cfg(unix)]
    {
        journal.directory_handle.sync_all()?;
        same_directory(&journal.directory_handle, &journal.directory)?;
        rustix::fs::unlinkat(&parent, name, rustix::fs::AtFlags::REMOVEDIR)
            .map_err(io::Error::from)?;
        parent.sync_all()?;
    }
    #[cfg(windows)]
    {
        drop(handles);
        drop(journal._lock);
        windows::delete_handle(&journal.directory_handle)?;
        let _ = parent;
    }
    Ok(())
}

fn create_parents(path: &Path, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(JournalError::Limit(
            "recovery directory nesting exceeds 128",
        ));
    }
    match path.symlink_metadata() {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(JournalError::Invalid(
            "recovery parent must be a real directory",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_parents(
                path.parent()
                    .ok_or(JournalError::Invalid("missing directory ancestor"))?,
                depth + 1,
            )?;
            create_private_directory(path)?;
            sync_directory(
                path.parent()
                    .ok_or(JournalError::Invalid("missing directory ancestor"))?,
            )
        }
        Err(error) => Err(error.into()),
    }
}

fn create_private_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    };
    #[cfg(windows)]
    let created = windows::create_directory(path);
    match created {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // Another app instance may have created the same private parent/root first.
            open_directory(path, false)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}
