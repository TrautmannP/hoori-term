use hoori_term::session::{ProcessState, Session};
use hoori_term::terminal::GridSize;
use std::thread;
use std::time::{Duration, Instant};

fn wait_output(session: &Session, expected: &[u8]) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut output = Vec::new();
    while Instant::now() < deadline {
        while let Some(chunk) = session.read_chunk() {
            output.extend(chunk);
        }
        if output.windows(expected.len()).any(|w| w == expected) {
            return output;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "synthetic PTY marker was not received; state: {:?}",
        session.state()
    );
}

#[test]
fn interactive_bash_accepts_input_resize_and_exit() {
    let session = Session::spawn(
        "/bin/bash",
        &["--noprofile".into(), "--norc".into(), "-i".into()],
        GridSize::new(80, 24).pty(),
    )
    .unwrap();
    session
        .write(b"printf 'START_%s_END\\n' proof\r".to_vec())
        .unwrap();
    wait_output(&session, b"START_proof_END");
    session.resize(GridSize::new(111, 35).pty()).unwrap();
    session.write(b"stty size\r".to_vec()).unwrap();
    wait_output(&session, b"35 111");
    session.write(b"exit 7\r".to_vec()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.state() == ProcessState::Running && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session.state(), ProcessState::Exited(7));
    session.shutdown();
}

#[test]
fn bounded_output_applies_backpressure_without_losing_bytes() {
    let session = Session::spawn(
        "/bin/bash",
        &[
            "-c".into(),
            "head -c 2097152 /dev/zero | tr '\\0' x; printf DONE".into(),
        ],
        GridSize::new(80, 24).pty(),
    )
    .unwrap();
    // More output than the entire bounded queue while the consumer is idle.
    thread::sleep(Duration::from_millis(150));
    let output = wait_output(&session, b"DONE");
    assert_eq!(output.len(), 2_097_156);
    assert!(output[..2_097_152].iter().all(|b| *b == b'x'));
    session.shutdown();
}

#[test]
fn shutdown_interrupts_full_output_queue_and_reaps_child() {
    let session = Session::spawn(
        "/bin/bash",
        &[
            "-c".into(),
            "trap '' HUP; while :; do printf 'load\\n'; done".into(),
        ],
        GridSize::new(80, 24).pty(),
    )
    .unwrap();
    thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    session.shutdown();
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn unavailable_shell_returns_a_start_error() {
    assert!(
        Session::spawn(
            "/nonexistent/hoori-test-shell",
            &[],
            GridSize::new(80, 24).pty()
        )
        .is_err()
    );
}

#[test]
fn saturated_input_returns_immediately_and_shutdown_interrupts_writer() {
    let session = Session::spawn(
        "/bin/bash",
        &["-c".into(), "stty raw -echo; sleep 30".into()],
        GridSize::new(80, 24).pty(),
    )
    .unwrap();
    thread::sleep(Duration::from_millis(50));
    let start = Instant::now();
    let mut saw_backpressure = false;
    for _ in 0..1000 {
        if session.write(vec![b'x'; 4096]).is_err() {
            saw_backpressure = true;
            break;
        }
    }
    assert!(saw_backpressure);
    assert!(start.elapsed() < Duration::from_secs(1));
    session.shutdown();
}

#[test]
fn large_paste_is_one_bounded_entry_and_is_not_partially_rejected() {
    use hoori_term::session::MAX_WRITE_BYTES;
    let session = Session::spawn(
        "/bin/bash",
        &[
            "-c".into(),
            "stty raw -echo; printf READY; head -c 65536; printf DONE".into(),
        ],
        GridSize::new(80, 24).pty(),
    )
    .unwrap();
    wait_output(&session, b"READY");
    assert!(session.write(vec![b'z'; MAX_WRITE_BYTES + 1]).is_err());
    session.write(vec![b'x'; MAX_WRITE_BYTES]).unwrap();
    let output = wait_output(&session, b"DONE");
    assert_eq!(output.len(), MAX_WRITE_BYTES + 4);
    assert!(output[..MAX_WRITE_BYTES].iter().all(|b| *b == b'x'));
    session.shutdown();
}
