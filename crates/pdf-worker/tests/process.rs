#![cfg(feature = "test-helper")]
#![allow(clippy::unwrap_used)]
use pdf_worker::{Error, Limits, PayloadKind, Request, execute};
use std::path::Path;
use std::time::{Duration, Instant};

fn run_cli(mode: &str, limits: Limits) -> pdf_worker::Result<pdf_worker::Reply> {
    pdf_worker::execute_cli(
        Path::new(env!("CARGO_BIN_EXE_pdf-worker-test-helper")),
        &[mode.into()],
        Request {
            control: serde_json::json!({"nonce":42}),
            payload: Vec::new(),
        },
        limits,
    )
}

#[test]
fn cli_output_domain_preserves_large_replies_and_shared_process_failure_guards() {
    for (mode, kind) in [("json", PayloadKind::Json), ("text", PayloadKind::Text)] {
        let reply = run_cli(mode, Limits::default()).unwrap();
        assert_eq!(reply.kind, kind);
        assert!(reply.payload.len() > pdf_worker::MAX_CONTROL_BYTES);
        assert_eq!(reply.control["nonce"], 42);
        assert!(run(mode, Limits::default()).is_err());
    }
    for mode in [
        "echo",
        "abort",
        "malformed",
        "truncated",
        "oversize",
        "cli-oversize",
        "cli-text-oversize",
        "cli-checksum",
        "trailing",
        "stderr",
    ] {
        let limits =
            Limits::for_test(pdf_worker::MEMORY_BYTES, 30, Duration::from_secs(3)).unwrap();
        assert!(
            run_cli(mode, limits).is_err(),
            "unexpected CLI success: {mode}"
        );
    }
    let short = Limits::for_test(pdf_worker::MEMORY_BYTES, 30, Duration::from_millis(200)).unwrap();
    assert!(matches!(run_cli("timeout", short), Err(Error::Timeout)));
    assert!(run_cli("text", Limits::default()).is_ok());
}

#[cfg(unix)]
#[test]
fn abrupt_supervisor_death_terminates_the_document_child() {
    let temporary = tempfile::tempdir().unwrap();
    let marker = temporary.path().join("child-pid");
    let mut supervisor = std::process::Command::new(env!("CARGO_BIN_EXE_pdf-worker-test-helper"))
        .arg("supervisor")
        .arg(&marker)
        .spawn()
        .unwrap();
    let started = Instant::now();
    let pid = loop {
        if let Ok(value) = std::fs::read_to_string(&marker) {
            if let Ok(pid) = value.parse::<i32>() {
                break rustix::process::Pid::from_raw(pid).unwrap();
            }
        }
        if started.elapsed() > Duration::from_secs(5) {
            let _ = supervisor.kill();
            let _ = supervisor.wait();
            panic!("child never accepted the supervised request");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    supervisor.kill().unwrap();
    supervisor.wait().unwrap();
    let deadline = Instant::now();
    while rustix::process::test_kill_process(pid).is_ok() {
        if deadline.elapsed() > Duration::from_secs(5) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
            panic!("child survived abrupt supervisor death");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(deadline.elapsed() < Duration::from_secs(5));
}

#[cfg(unix)]
#[test]
fn invalid_or_disappeared_supervisor_fails_before_readiness_or_input() {
    for expected in ["0", "not-a-pid"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_pdf-worker-test-helper"))
            .arg("echo")
            .env("PDF_WORKER_PARENT", expected)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        assert!(diagnostic.contains("parent"));
    }
}

fn run(mode: &str, limits: Limits) -> pdf_worker::Result<pdf_worker::Reply> {
    execute(
        Path::new(env!("CARGO_BIN_EXE_pdf-worker-test-helper")),
        &[mode.into()],
        Request {
            control: serde_json::json!({"nonce":42}),
            payload: b"opaque input bytes".to_vec(),
        },
        limits,
    )
}

#[test]
fn real_process_roundtrip_preserves_controls_and_binary_bytes() {
    let reply = run("echo", Limits::default()).unwrap();
    assert_eq!(reply.control, serde_json::json!({"nonce":42}));
    assert_eq!(reply.payload, b"opaque input bytes");
    assert_eq!(reply.kind, PayloadKind::Pdf);
}
#[test]
fn abort_does_not_kill_parent_and_next_process_still_works() {
    assert!(run("abort", Limits::default()).is_err());
    assert!(run("echo", Limits::default()).is_ok());
}
#[test]
fn deadline_terminates_reaps_and_allows_a_fresh_process() {
    let start = Instant::now();
    let limit = Limits::for_test(pdf_worker::MEMORY_BYTES, 30, Duration::from_millis(250)).unwrap();
    assert!(matches!(run("timeout", limit), Err(Error::Timeout)));
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(run("echo", Limits::default()).is_ok());
}
#[test]
fn malformed_truncated_oversize_checksum_and_extra_output_are_errors() {
    for mode in ["malformed", "truncated", "oversize", "checksum", "trailing"] {
        assert!(run(mode, Limits::default()).is_err(), "{mode}");
    }
}
#[test]
fn stderr_flood_is_bounded_and_does_not_deadlock() {
    let start = Instant::now();
    assert!(matches!(
        run("stderr", Limits::default()),
        Err(Error::StderrLimit)
    ));
    assert!(start.elapsed() < Duration::from_secs(3));
}
#[test]
fn actual_memory_allocation_is_rejected_by_installed_os_limit() {
    let budget = if cfg!(target_os = "macos") {
        32 * 1024 * 1024
    } else {
        64 * 1024 * 1024
    };
    let limit = Limits::for_test(budget, 30, Duration::from_secs(10)).unwrap();
    let reply = run("memory", limit).unwrap();
    assert_eq!(reply.control["allocation_refused"], true);
    assert_eq!(reply.control["small_mapping_succeeded"], true);
    eprintln!(
        "bounded VM probe: budget {budget}, 8MiB accepted,96MiB rejected with kernel error {}",
        reply.control["kernel_error"]
    );
}
#[test]
fn actual_cpu_limit_ends_a_spinning_worker_before_wall_deadline() {
    let start = Instant::now();
    let limit = Limits::for_test(pdf_worker::MEMORY_BYTES, 1, Duration::from_secs(5)).unwrap();
    assert!(run("echo", limit).is_ok());
    let error = run("cpu", limit).unwrap_err();
    assert!(!matches!(error, Error::Timeout));
    assert!(matches!(error, Error::Exited(_)), "{error}");
    assert!(start.elapsed() >= Duration::from_millis(800));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
#[cfg(target_os = "macos")]
fn sampled_resident_watchdog_catches_commits_inside_startup_reservations() {
    let limits = Limits::for_test(64 * 1024 * 1024, 30, Duration::from_secs(5)).unwrap();
    let start = Instant::now();
    match run("resident", limits).unwrap_err() {
        Error::ResidentLimit { observed, budget } => {
            assert_eq!(budget, 64 * 1024 * 1024);
            assert!(observed > budget);
            eprintln!(
                "resident watchdog: observed{observed} > budget{budget}, sampled enforcement may overshoot"
            );
        }
        error => panic!("resident watchdog did not terminate child: {error}"),
    }
    assert!(start.elapsed() < Duration::from_secs(3));
    assert!(run("echo", Limits::default()).is_ok());
}
