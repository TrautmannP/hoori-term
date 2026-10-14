#![cfg(windows)]
use hoori_term::launch;
use hoori_term::session::{ProcessState, Session};
use hoori_term::terminal::{GridSize, Terminal};
use std::time::{Duration, Instant};

fn until(session: &Session, terminal: &mut Terminal, done: impl Fn(&str) -> bool) -> String {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut output = String::new();
    while Instant::now() < deadline {
        while let Some(chunk) = session.read_chunk() {
            output.push_str(&String::from_utf8_lossy(&chunk));
            for reply in terminal.feed(&chunk) {
                session.write(reply).unwrap();
            }
        }
        if done(&output) {
            return output;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "Synthetic Windows PTY marker missing; state {:?}",
        session.state()
    );
}

fn shutdown(session: Session) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        session.shutdown();
        let _ = tx.send(());
    });
    rx.recv_timeout(Duration::from_secs(8))
        .expect("Windows PTY shutdown exceeded eight seconds");
}

#[test]
fn powershell_51_interactive_utf8_resize_exit_and_shutdown() {
    let shell = launch::powershell();
    let size = GridSize::new(80, 24);
    let session = Session::spawn(&shell.program, &shell.args, size.pty()).unwrap();
    let mut terminal = Terminal::new(size);
    session
        .write(b"Write-Output ('HOO_VERSION:' + $PSVersionTable.PSVersion.ToString())\r".to_vec())
        .unwrap();
    until(&session, &mut terminal, |s| s.contains("HOO_VERSION:5.1."));
    session.resize(GridSize::new(112, 34).pty()).unwrap();
    session.write("Write-Output ('HOO_UTF8:' + 'Grüße 界'); Write-Output ('HOO_SIZE:' + [Console]::WindowWidth)\r".as_bytes().to_vec()).unwrap();
    let output = until(&session, &mut terminal, |s| s.contains("HOO_SIZE:112"));
    assert!(output.contains("HOO_UTF8:Grüße 界"));
    session
        .write(b"Write-Output 'HOO_EDIT:abXc'".to_vec())
        .unwrap();
    // PSReadLine must edit the interactive buffer, rather than merely echoing
    // a preassembled command. Move before c, remove X, then submit the line.
    session.write(b"\x1b[D\x1b[D\x7f\x1b[F\r".to_vec()).unwrap();
    until(&session, &mut terminal, |s| s.contains("HOO_EDIT:abc"));
    session.write(b"exit 7\r".to_vec()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while session.state() == ProcessState::Running && Instant::now() < deadline {
        while let Some(chunk) = session.read_chunk() {
            terminal.feed(&chunk);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(session.state(), ProcessState::Exited(7));
    shutdown(session);
}

#[test]
fn conpty_shutdown_releases_an_idle_reader_and_saturated_writer() {
    let shell = launch::powershell();
    let session = Session::spawn(&shell.program, &shell.args, GridSize::new(80, 24).pty()).unwrap();
    session
        .write(b"Start-Sleep -Seconds 30\r".to_vec())
        .unwrap();
    std::thread::sleep(Duration::from_millis(250));
    for _ in 0..128 {
        if session.write(vec![b'x'; 64 * 1024]).is_err() {
            break;
        }
    }
    shutdown(session);
}

#[test]
#[ignore = "requires explicit HOORI_TEST_WSL_DISTRO and a working WSL-2 Ubuntu installation"]
fn wsl_shell_input_resize_and_targeted_close_preserve_foreign_session() {
    let distro = std::env::var("HOORI_TEST_WSL_DISTRO").expect("set HOORI_TEST_WSL_DISTRO");
    let foreign = ForeignSession::start(&distro);
    let inventory = launch::discover();
    assert!(
        inventory
            .choices
            .iter()
            .any(|c| c.enabled && c.shell.args.get(1) == Some(&distro))
    );
    let mut shell = launch::wsl(&distro);
    let target = std::env::var("HOORI_TEST_WSL_SHELL").unwrap_or_else(|_| "bash".into());
    assert!(matches!(target.as_str(), "bash" | "zsh"));
    let executable = shell.args.iter().position(|arg| arg == "--exec").unwrap() + 1;
    shell.args[executable] = format!("/bin/{target}");
    if target == "zsh" {
        shell.args.insert(executable + 1, "-f".into());
    }
    let size = GridSize::new(80, 24);
    let mut sessions = Vec::new();
    for _ in 0..3 {
        let session = Session::spawn(&shell.program, &shell.args, size.pty()).unwrap();
        let mut terminal = Terminal::new(size);
        session
            .write(b"printf 'HOO_PID:%s\\n' $$\r".to_vec())
            .unwrap();
        let output = until(&session, &mut terminal, |s| linux_pid(s).is_some());
        sessions.push((session, terminal, linux_pid(&output).unwrap()));
    }
    let foreign_pid = sessions[2].2;
    sessions[0]
        .0
        .write(b"sleep 120 & printf 'HOO_JOB:%s\\n' $!\r".to_vec())
        .unwrap();
    let (session, terminal, _) = &mut sessions[0];
    let output = until(session, terminal, |s| marked_pid(s, "HOO_JOB:").is_some());
    let child_pid = marked_pid(&output, "HOO_JOB:").unwrap();
    sessions[0].0.resize(GridSize::new(111, 35).pty()).unwrap();
    sessions[0]
        .0
        .write(
            "stty size; printf 'HOO_VALUE:%s\\n' 'Grüße 界'\r"
                .as_bytes()
                .to_vec(),
        )
        .unwrap();
    let (session, terminal, _) = &mut sessions[0];
    until(session, terminal, |s| {
        s.contains("35 111") && s.contains("HOO_VALUE:Grüße 界")
    });
    let (session, _, closed_pid) = sessions.remove(0);
    shutdown(session);
    let deadline = Instant::now() + Duration::from_secs(5);
    while linux_alive(&distro, closed_pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !linux_alive(&distro, closed_pid),
        "Linux shell survived closed Windows helper"
    );
    assert!(
        !linux_alive(&distro, child_pid),
        "Linux background job survived closed session"
    );
    assert!(linux_alive(&distro, foreign_pid));
    assert!(linux_alive(&distro, foreign.pid));
    for (session, mut terminal, _) in sessions {
        session
            .write(b"printf 'HOO_SURVIVOR:%s\\n' alive\r".to_vec())
            .unwrap();
        until(&session, &mut terminal, |s| {
            s.contains("HOO_SURVIVOR:alive")
        });
        shutdown(session);
    }
    assert!(linux_alive(&distro, foreign.pid));
}

struct ForeignSession {
    distro: String,
    pid: u32,
    helper: std::process::Child,
}
impl ForeignSession {
    fn start(distro: &str) -> Self {
        use std::io::BufRead;
        use std::os::windows::process::CommandExt;
        let mut helper = std::process::Command::new(launch::system_program("wsl.exe"))
            .args([
                "--distribution",
                distro,
                "--exec",
                "/bin/sh",
                "-c",
                "printf '%s\\n' $$; exec sleep 120",
            ])
            .creation_flags(0x08000000)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = helper.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = std::io::BufReader::new(stdout)
                .read_line(&mut line)
                .map(|_| line);
            let _ = tx.send(result);
        });
        let pid = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(line)) => line.trim().parse().unwrap(),
            _ => {
                let _ = helper.kill();
                let _ = helper.wait();
                panic!("Foreign WSL fixture did not start");
            }
        };
        assert!(linux_alive(distro, pid));
        Self {
            distro: distro.into(),
            pid,
            helper,
        }
    }
}
impl Drop for ForeignSession {
    fn drop(&mut self) {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new(launch::system_program("wsl.exe"))
            .args([
                "--distribution",
                &self.distro,
                "--exec",
                "/bin/kill",
                "-TERM",
                &self.pid.to_string(),
            ])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        let _ = self.helper.kill();
        let _ = self.helper.wait();
    }
}

fn linux_pid(output: &str) -> Option<u32> {
    marked_pid(output, "HOO_PID:")
}

fn marked_pid(output: &str, marker: &str) -> Option<u32> {
    output.match_indices(marker).find_map(|(at, _)| {
        output[at + marker.len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .ok()
    })
}

fn linux_alive(distro: &str, pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(launch::system_program("wsl.exe"))
        .args([
            "--distribution",
            distro,
            "--exec",
            "/bin/kill",
            "-0",
            &pid.to_string(),
        ])
        .creation_flags(0x08000000)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}
