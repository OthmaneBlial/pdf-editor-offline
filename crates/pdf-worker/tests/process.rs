#![cfg(feature = "test-helper")]
#![allow(clippy::unwrap_used)]
use pdf_worker::{Error, Limits, PayloadKind, Request, execute};
use std::path::Path;
use std::time::{Duration, Instant};

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
