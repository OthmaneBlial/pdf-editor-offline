use crate::{Error, Limits, Result};
use std::process::{Child, Command};

#[cfg(feature = "test-helper")]
#[allow(unsafe_code)]
mod probe;
#[cfg(feature = "test-helper")]
pub use probe::allocation_probe;

pub(crate) fn child_configuration() -> Result<Limits> {
    let mut limits = Limits::default();
    if let Ok(value) = std::env::var("PDF_WORKER_MEMORY") {
        limits.memory = value
            .parse()
            .map_err(|_| Error::Protocol("invalid memory limit"))?;
    }
    if let Ok(value) = std::env::var("PDF_WORKER_CPU") {
        limits.cpu = value
            .parse()
            .map_err(|_| Error::Protocol("invalid CPU limit"))?;
    }
    Limits::for_test(limits.memory, limits.cpu, limits.wall)
}
#[cfg(unix)]
pub(crate) fn install_child(limits: Limits) -> Result<()> {
    use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};
    let address_limit = address_limit(limits.memory)?;
    for (resource, value) in [
        (Resource::As, address_limit),
        (Resource::Cpu, limits.cpu),
        (Resource::Core, 0),
        (Resource::Nofile, 64),
    ] {
        let old = getrlimit(resource);
        let value = old.maximum.map_or(value, |maximum| value.min(maximum));
        setrlimit(
            resource,
            Rlimit {
                current: Some(value),
                maximum: Some(value),
            },
        )
        .map_err(|error| Error::Limits(format!("{resource:?}: {error}")))?;
        let actual = getrlimit(resource);
        if actual.current.is_none_or(|actual| actual > value)
            || actual.maximum.is_none_or(|actual| actual > value)
        {
            return Err(Error::Limits(format!("{resource:?} not enforced")));
        }
    }
    Ok(())
}
#[cfg(all(unix, not(target_os = "macos")))]
fn address_limit(budget: u64) -> Result<u64> {
    Ok(budget)
}
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos;
#[cfg(target_os = "macos")]
fn address_limit(budget: u64) -> Result<u64> {
    macos::address_limit(budget)
}

pub(crate) fn check_resident(child: &Child, limits: Limits) -> Result<()> {
    check_resident_id(child.id(), limits)
}
pub(crate) fn check_resident_id(pid: u32, limits: Limits) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let observed = macos::resident_bytes(pid)?;
        if observed > limits.memory {
            return Err(Error::ResidentLimit {
                observed,
                budget: limits.memory,
            });
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (pid, limits);
    Ok(())
}

#[cfg(windows)]
pub(crate) fn install_child(_: Limits) -> Result<()> {
    Ok(())
} // Parent assigns job before sending input.
#[cfg(not(any(unix, windows)))]
pub(crate) fn install_child(_: Limits) -> Result<()> {
    Err(Error::Limits("unsupported platform".into()))
}

pub(crate) fn configure(command: &mut Command, limits: Limits) {
    command
        .env_clear()
        .env("PDF_WORKER_MEMORY", limits.memory.to_string())
        .env("PDF_WORKER_CPU", limits.cpu.to_string());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::Guard;
#[cfg(not(windows))]
pub(crate) struct Guard;
#[cfg(not(windows))]
impl Guard {
    pub(crate) fn attach(_: &Child, _: Limits) -> Result<Self> {
        Ok(Self)
    }
    pub(crate) fn terminate(&self, child: &mut Child) {
        #[cfg(unix)]
        if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
        let _ = child.kill();
    }
}
