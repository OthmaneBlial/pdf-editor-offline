use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const MAX_CAPTURE: u64 = 32 * 1024 * 1024;
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub seconds: f64,
}

pub struct Runner {
    started: Instant,
    budget: Duration,
}
impl Runner {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            budget: Duration::from_secs(180),
        }
    }
    pub fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
    pub fn run(&self, program: &Path, args: &[OsString]) -> Result<Output, String> {
        let remaining = self
            .budget
            .checked_sub(self.started.elapsed())
            .ok_or("global 180-second evidence budget exceeded")?;
        let timeout = remaining.min(Duration::from_secs(60));
        let stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
        let stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
        let started = Instant::now();
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(stdout.try_clone().map_err(|e| e.to_string())?)
            .stderr(stderr.try_clone().map_err(|e| e.to_string())?)
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
            let oversized = [&stdout, &stderr]
                .iter()
                .any(|file| file.metadata().map_or(true, |m| m.len() > MAX_CAPTURE));
            if oversized || started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if oversized {
                    "subprocess output exceeded 32 MiB"
                } else {
                    "subprocess deadline exceeded (60 seconds or remaining global budget)"
                }
                .into());
            }
            thread::sleep(Duration::from_micros(100));
        };
        let seconds = started.elapsed().as_secs_f64();
        Ok(Output {
            code,
            stdout: capture(stdout)?,
            stderr: capture(stderr)?,
            seconds,
        })
    }
    pub fn checked(&self, program: &Path, args: &[OsString]) -> Result<Vec<u8>, String> {
        let result = self.run(program, args)?;
        if result.code != Some(0) {
            return Err(format!(
                "{} exited {:?}: {}",
                program.display(),
                result.code,
                String::from_utf8_lossy(&result.stderr[..result.stderr.len().min(1024)])
            ));
        }
        Ok(result.stdout)
    }
}

fn capture(mut file: File) -> Result<Vec<u8>, String> {
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_CAPTURE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_CAPTURE {
        return Err("subprocess output exceeded 32 MiB".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deadlines_kill_and_reap_a_real_child() -> Result<(), String> {
        let runner = Runner {
            started: Instant::now(),
            budget: Duration::from_millis(20),
        };
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let args = crate::args([
            "--ignored",
            "--exact",
            "process::tests::child_sleep",
            "--nocapture",
        ]);
        let started = Instant::now();
        let error = runner
            .run(&executable, &args)
            .err()
            .ok_or("sleeping child did not time out")?;
        assert!(error.contains("deadline"));
        assert!(started.elapsed() < Duration::from_secs(2));
        Ok(())
    }
    #[test]
    #[ignore = "subprocess helper, invoked by deadline test"]
    fn child_sleep() {
        thread::sleep(Duration::from_millis(200));
    }
    #[test]
    fn expired_global_budget_and_capture_limit() -> Result<(), String> {
        let runner = Runner {
            started: Instant::now(),
            budget: Duration::ZERO,
        };
        assert!(
            runner
                .run(Path::new("missing-program"), &[])
                .err()
                .ok_or("missing error")?
                .contains("global")
        );
        let file = tempfile::tempfile().map_err(|e| e.to_string())?;
        file.set_len(MAX_CAPTURE + 1).map_err(|e| e.to_string())?;
        assert!(capture(file).is_err());
        Ok(())
    }
}
