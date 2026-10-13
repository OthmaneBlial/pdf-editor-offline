//! One-request child processes for bounded PDF operations. Not a file/network sandbox.
#![deny(unsafe_code)]

pub mod document;
pub mod journal;
mod limits;
mod protocol;
mod transport;

#[cfg(feature = "test-helper")]
pub use limits::allocation_probe;
pub use protocol::{PayloadKind, Reply, Request};
use std::io::{self, Write};
use std::time::Duration;
pub use transport::{execute, execute_cli};

pub const MAX_CONTROL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PDF_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_RGBA_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 128 * 1024 * 1024;
// Preserve the core text tier even when JSON escapes every byte, plus bounded metadata.
pub const MAX_JSON_BYTES: usize = MAX_TEXT_BYTES * 6 + MAX_CONTROL_BYTES;
pub const MAX_STDERR_BYTES: usize = 64 * 1024;
pub const MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const CPU_SECONDS: u64 = 30;
const PROCESSING_STACK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("worker I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("worker protocol rejected: {0}")]
    Protocol(&'static str),
    #[error("worker resource setup failed: {0}")]
    Limits(String),
    #[error("document worker timed out; the committed document has been retained")]
    Timeout,
    #[error("worker exceeded its diagnostic output limit")]
    StderrLimit,
    #[error("worker exceeded sampled resident-memory budget ({observed} bytes > {budget} bytes)")]
    ResidentLimit { observed: u64, budget: u64 },
    #[error("document worker exited unsuccessfully: {0}")]
    Exited(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    memory: u64,
    cpu: u64,
    wall: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            memory: MEMORY_BYTES,
            cpu: CPU_SECONDS,
            wall: Duration::from_secs(CPU_SECONDS),
        }
    }
}
impl Limits {
    /// Shrink limits for failure tests. This cannot relax production limits.
    pub fn for_test(memory: u64, cpu: u64, wall: Duration) -> Result<Self> {
        if memory == 0
            || memory > MEMORY_BYTES
            || cpu == 0
            || cpu > CPU_SECONDS
            || wall.is_zero()
            || wall > Duration::from_secs(CPU_SECONDS)
        {
            return Err(Error::Protocol(
                "test limits must be positive and no larger than production",
            ));
        }
        Ok(Self { memory, cpu, wall })
    }
}

/// Apply limits before accepting input, announce readiness, execute one operation,
/// then emit one bounded reply. The caller's main must exit after this function.
pub fn serve_once(
    handler: impl FnOnce(Request) -> std::result::Result<Reply, String> + Send,
) -> Result<()> {
    let configured = limits::child_configuration()?;
    limits::install_child(configured)?;
    // Windows executable stacks are smaller than bounded parser/CLI call paths need.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("pdf-processing".into())
            .stack_size(PROCESSING_STACK_BYTES)
            .spawn_scoped(scope, move || serve_limited(handler, configured))
            .map_err(|error| Error::Limits(format!("processing thread setup failed: {error}")))?
            .join()
            .map_err(|_| Error::Limits("processing thread panicked".into()))?
    })
}

fn serve_limited(
    handler: impl FnOnce(Request) -> std::result::Result<Reply, String>,
    configured: Limits,
) -> Result<()> {
    #[cfg(unix)]
    let _parent = limits::watch_parent()?;
    let mut output = io::stdout().lock();
    protocol::write_ready(&mut output, configured)?;
    output.flush()?;
    let request = protocol::read_request(&mut io::stdin().lock())?;
    let reply=handler(request).unwrap_or_else(|message| Reply { control: serde_json::json!({"ok":false,"error":message.chars().take(8192).collect::<String>()}),payload:Vec::new(),kind:PayloadKind::None });
    protocol::write_reply(&mut output, &reply)?;
    output.flush()?;
    Ok(())
}
