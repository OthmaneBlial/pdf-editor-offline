use crate::{
    Error, Limits, MAX_CONTROL_BYTES, MAX_PDF_BYTES, MAX_STDERR_BYTES, Reply, Request, Result,
    limits, protocol,
};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct Process {
    child: Child,
    guard: limits::Guard,
    reaped: bool,
}
impl Drop for Process {
    fn drop(&mut self) {
        if !self.reaped {
            self.guard.terminate(&mut self.child);
            let _ = self.child.wait();
        }
    }
}

/// Runs one request under production limits (or smaller failure-test limits).
/// The executable must be absolute; callers may select a desktop `--pdf-worker` entry.
pub fn execute(
    helper: &Path,
    args: &[OsString],
    request: Request,
    limits: Limits,
) -> Result<Reply> {
    execute_with_policy(
        helper,
        args,
        request,
        limits,
        protocol::ReplyPolicy::Document,
    )
}

/// Execute a CLI worker, accepting bounded UTF-8 text/JSON replies rather than PDF/raster data.
pub fn execute_cli(
    helper: &Path,
    args: &[OsString],
    request: Request,
    limits: Limits,
) -> Result<Reply> {
    execute_with_policy(helper, args, request, limits, protocol::ReplyPolicy::Cli)
}

fn execute_with_policy(
    helper: &Path,
    args: &[OsString],
    request: Request,
    limits: Limits,
    policy: protocol::ReplyPolicy,
) -> Result<Reply> {
    if !helper.is_absolute() {
        return Err(Error::Protocol("worker executable must be absolute"));
    }
    if request.payload.len() > MAX_PDF_BYTES
        || serde_json::to_vec(&request.control)
            .map_err(|_| Error::Protocol("invalid request controls"))?
            .len()
            > MAX_CONTROL_BYTES
    {
        return Err(Error::Protocol("request exceeds transport limits"));
    }
    let started = Instant::now();
    let mut command = Command::new(helper);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    limits::configure(&mut command, limits);
    let mut child = command.spawn()?;
    let guard = match limits::Guard::attach(&child, limits) {
        Ok(guard) => guard,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut process = Process {
        child,
        guard,
        reaped: false,
    };
    // Fail closed before the I/O thread can send any document input.
    limits::check_resident(&process.child, limits)?;
    let input = process
        .child
        .stdin
        .take()
        .ok_or(Error::Protocol("worker stdin missing"))?;
    let output = process
        .child
        .stdout
        .take()
        .ok_or(Error::Protocol("worker stdout missing"))?;
    let diagnostics = process
        .child
        .stderr
        .take()
        .ok_or(Error::Protocol("worker stderr missing"))?;
    let flooded = Arc::new(AtomicBool::new(false));
    let drain_flag = Arc::clone(&flooded);
    let drain = std::thread::spawn(move || {
        let mut diagnostics = diagnostics;
        let mut retained = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            match diagnostics.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let remaining = MAX_STDERR_BYTES.saturating_sub(retained.len());
                    retained.extend_from_slice(&buffer[..count.min(remaining)]);
                    if count > remaining {
                        drain_flag.store(true, Ordering::Release);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        retained
    });
    let (send, receive) = mpsc::channel();
    let child_id = process.child.id();
    let io = std::thread::spawn(move || {
        let mut input = input;
        let mut output = output;
        let result = (|| {
            protocol::read_ready(&mut output, limits)?;
            limits::check_resident_id(child_id, limits)?;
            protocol::write_request(&mut input, &request)?;
            input.flush()?;
            drop(request);
            drop(input);
            let reply = protocol::read_reply(&mut output, policy)?;
            let mut extra = [0];
            if output.read(&mut extra)? != 0 {
                return Err(Error::Protocol("trailing worker output"));
            }
            Ok(reply)
        })();
        let _ = send.send(result);
    });
    let mut response = None;
    let outcome = loop {
        if let Err(error) = limits::check_resident(&process.child, limits) {
            // A just-exited child no longer has task information. Terminate the
            // still-owned group before reaping, then distinguish clean exit
            // from unavailable observation of a running child. Killing a zombie
            // changes neither its exit status nor an already completed reply.
            process.guard.terminate(&mut process.child);
            let status = process.child.wait()?;
            process.reaped = true;
            if status.success() {
                if let Some(reply) = response.take() {
                    break reply;
                }
                break match receive.recv_timeout(limits.wall.saturating_sub(started.elapsed())) {
                    Ok(reply) => reply,
                    Err(_) => Err(Error::Timeout),
                };
            }
            if matches!(error, Error::ResidentLimit { .. }) {
                break Err(error);
            }
            break Err(Error::Exited(format!("{status}; {error}")));
        }
        if flooded.load(Ordering::Acquire) {
            break Err(Error::StderrLimit);
        }
        if started.elapsed() >= limits.wall {
            break Err(Error::Timeout);
        }
        match receive.recv_timeout(Duration::from_millis(10)) {
            Ok(result) => response = Some(result),
            Err(mpsc::RecvTimeoutError::Disconnected) if response.is_none() => {
                break Err(Error::Protocol("worker I/O thread stopped"));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {}
        }
        if response.as_ref().is_some_and(Result::is_err) {
            if let Ok(Some(status)) = process.child.try_wait() {
                process.reaped = true;
                if !status.success() {
                    break Err(Error::Exited(status.to_string()));
                }
            }
            break response
                .take()
                .ok_or(Error::Protocol("missing worker error"))?;
        }
        // Do not reap until pipe reading has completed. The unreaped child PID
        // protects its process-group identity during timeout/error termination.
        if response.is_none() {
            continue;
        }
        match process.child.try_wait() {
            Ok(Some(status)) => {
                process.reaped = true;
                if !status.success() {
                    break Err(Error::Exited(status.to_string()));
                }
                if let Some(result) = response.take() {
                    break result;
                }
            }
            Ok(None) => {}
            Err(error) => break Err(Error::Io(error)),
        }
    };
    // Always terminate/reap before joining pipe readers; child/descendant handles
    // cannot leave blocked readers alive after a timeout or malformed frame.
    let final_status = if !process.reaped {
        process.guard.terminate(&mut process.child);
        let status = process.child.wait()?;
        process.reaped = true;
        Some(status)
    } else {
        None
    };
    drop(process);
    let _ = io.join();
    let diagnostics = drain.join().unwrap_or_default();
    if flooded.load(Ordering::Acquire) {
        return Err(Error::StderrLimit);
    }
    let outcome = match outcome {
        Err(Error::Io(error))
            if error.kind() == std::io::ErrorKind::UnexpectedEof
                && final_status.is_some_and(|status| !status.success()) =>
        {
            Err(Error::Exited(
                final_status
                    .map(|status| status.to_string())
                    .unwrap_or_default(),
            ))
        }
        outcome => outcome,
    };
    match outcome {
        Err(Error::Exited(message)) if !diagnostics.is_empty() => Err(Error::Exited(format!(
            "{message}: {}",
            String::from_utf8_lossy(&diagnostics)
                .chars()
                .take(8192)
                .collect::<String>()
        ))),
        Err(Error::Io(error)) if !diagnostics.is_empty() => Err(Error::Exited(format!(
            "{error}: {}",
            String::from_utf8_lossy(&diagnostics)
                .chars()
                .take(8192)
                .collect::<String>()
        ))),
        outcome => outcome,
    }
}
