//! PTY transport. Only bounded, nonblocking channel operations run on the UI thread.
use crate::notifications::Notifications;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const OUTPUT_CHUNK: usize = 8192;
pub const OUTPUT_CAPACITY: usize = 64;
pub const INPUT_CHUNK: usize = 4096;
pub const INPUT_CAPACITY: usize = 64;
pub const MAX_WRITE_BYTES: usize = 64 * 1024;
pub const INPUT_BYTE_CAPACITY: usize = INPUT_CHUNK * INPUT_CAPACITY;

struct InputPacket {
    bytes: Vec<u8>,
    queued_bytes: Arc<AtomicUsize>,
}

impl Drop for InputPacket {
    fn drop(&mut self) {
        self.queued_bytes
            .fetch_sub(self.bytes.len(), Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    Exited(u32),
    Failed(String),
}

pub struct Session {
    pub notifications: Notifications,
    master: Option<Box<dyn MasterPty + Send>>,
    input: Option<mpsc::SyncSender<InputPacket>>,
    queued_input_bytes: Arc<AtomicUsize>,
    output: Option<mpsc::Receiver<Vec<u8>>>,
    state: Arc<Mutex<ProcessState>>,
    io_error: Arc<Mutex<Option<String>>>,
    stopping: Arc<AtomicBool>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    #[cfg(unix)]
    pid: Option<u32>,
    workers: Vec<JoinHandle<()>>,
}

impl Session {
    pub fn spawn(program: &str, args: &[String], size: PtySize) -> Result<Self, String> {
        Self::spawn_in(program, args, None, size)
    }

    pub fn spawn_in(
        program: &str,
        args: &[String],
        directory: Option<&std::path::Path>,
        size: PtySize,
    ) -> Result<Self, String> {
        // portable-pty 0.9 silently replaces a nonexistent cwd with HOME.
        // On Ubuntu, hold a directory descriptor until spawn and pass its
        // stable /proc alias. This prevents path removal/replacement from
        // turning an explicit target into that fallback.
        #[cfg(target_os = "linux")]
        let directory_handle = directory
            .map(|path| {
                use std::os::fd::AsRawFd;
                use std::os::unix::fs::OpenOptionsExt;
                let handle = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(nix::libc::O_PATH | nix::libc::O_DIRECTORY)
                    .open(path)
                    .map_err(|e| format!("Startverzeichnis nicht erreichbar: {e}"))?;
                let alias =
                    std::path::PathBuf::from(format!("/proc/self/fd/{}", handle.as_raw_fd()));
                if !alias.is_dir() {
                    return Err(
                        "Startverzeichnis kann nicht sicher über /proc geöffnet werden".to_owned(),
                    );
                }
                Ok((handle, alias))
            })
            .transpose()?;
        #[cfg(windows)]
        let directory_handle = directory
            .map(crate::windows_fs::pin_directory)
            .transpose()?;
        #[cfg(not(any(target_os = "linux", windows)))]
        if directory.is_some_and(|path| !path.is_dir()) {
            return Err("Startverzeichnis ist nicht erreichbar oder kein Verzeichnis".into());
        }
        let pair = native_pty_system()
            .openpty(size)
            .map_err(|e| e.to_string())?;
        // Ubuntu transport uses nonblocking descriptors so shutdown can interrupt
        // a blocked reader/writer even when a descendant retains the PTY.
        #[cfg(unix)]
        if let Some(fd) = pair.master.as_raw_fd() {
            use nix::fcntl::{FcntlArg, OFlag, fcntl};
            let flags = fcntl(fd, FcntlArg::F_GETFL).map_err(|e| e.to_string())?;
            fcntl(
                fd,
                FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
            )
            .map_err(|e| e.to_string())?;
        }
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut command = CommandBuilder::new(program);
        command.args(args);
        if let Some(directory) = directory {
            #[cfg(any(target_os = "linux", windows))]
            let directory = directory_handle
                .as_ref()
                .map(|(_, alias)| alias.as_path())
                .unwrap_or(directory);
            command.cwd(directory);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| e.to_string())?;
        #[cfg(any(target_os = "linux", windows))]
        drop(directory_handle);
        drop(pair.slave);
        #[cfg(unix)]
        let pid = child.process_id();
        let killer = child.clone_killer();
        let state = Arc::new(Mutex::new(ProcessState::Running));
        let io_error = Arc::new(Mutex::new(None));
        let stopping = Arc::new(AtomicBool::new(false));
        let notifications = Notifications::default();
        let (output_tx, output_rx) = mpsc::sync_channel(OUTPUT_CAPACITY);
        let (input_tx, input_rx) = mpsc::sync_channel::<InputPacket>(INPUT_CAPACITY);
        let queued_input_bytes = Arc::new(AtomicUsize::new(0));
        let stop = stopping.clone();
        let error = io_error.clone();
        let wakeups = notifications.clone();
        let reader_thread = thread::spawn(move || {
            let mut buffer = [0; OUTPUT_CHUNK];
            // ClosePseudoConsole can wait for its output to be drained. Keep
            // reading through Windows shutdown, discarding only after stop.
            while cfg!(windows) || !stop.load(Ordering::Relaxed) {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if !stop.load(Ordering::Relaxed)
                            && output_tx.send(buffer[..n].to_vec()).is_err()
                            && !cfg!(windows)
                        {
                            break;
                        }
                        wakeups.wake();
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(4))
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    // Linux reports EIO when the last slave closes.
                    #[cfg(unix)]
                    Err(e) if e.raw_os_error() == Some(5) => break,
                    Err(e) => {
                        if !stop.load(Ordering::Relaxed) {
                            *error.lock().unwrap() = Some(format!("PTY-Lesefehler: {e}"));
                        }
                        break;
                    }
                }
            }
            wakeups.wake();
        });
        let stop = stopping.clone();
        let error = io_error.clone();
        let wakeups = notifications.clone();
        let writer_thread = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let bytes = match input_rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(bytes) => bytes,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let mut remaining = bytes.bytes.as_slice();
                while !remaining.is_empty() && !stop.load(Ordering::Relaxed) {
                    match writer.write(remaining) {
                        Ok(0) => {
                            *error.lock().unwrap() = Some("PTY-Schreibkanal geschlossen".into());
                            wakeups.wake();
                            return;
                        }
                        Ok(n) => remaining = &remaining[n..],
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(4))
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => {
                            *error.lock().unwrap() = Some(format!("PTY-Schreibfehler: {e}"));
                            wakeups.wake();
                            return;
                        }
                    }
                }
            }
        });
        let process_state = state.clone();
        let wakeups = notifications.clone();
        let waiter_thread = thread::spawn(move || {
            *process_state.lock().unwrap() = match child.wait() {
                Ok(status) => ProcessState::Exited(status.exit_code()),
                Err(e) => ProcessState::Failed(format!("Prozessstatus nicht verfügbar: {e}")),
            };
            wakeups.wake();
        });
        Ok(Self {
            notifications,
            master: Some(pair.master),
            input: Some(input_tx),
            queued_input_bytes,
            output: Some(output_rx),
            state,
            io_error,
            stopping,
            killer,
            #[cfg(unix)]
            pid,
            workers: vec![reader_thread, writer_thread, waiter_thread],
        })
    }

    pub fn state(&self) -> ProcessState {
        self.state.lock().unwrap().clone()
    }
    pub fn take_io_error(&self) -> Option<String> {
        self.io_error.lock().unwrap().take()
    }

    pub fn read_chunk(&self) -> Option<Vec<u8>> {
        self.output.as_ref()?.try_recv().ok()
    }

    pub fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.len() > MAX_WRITE_BYTES {
            return Err("Eingabe überschreitet die Puffergrenze".into());
        }
        self.queued_input_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |size| {
                size.checked_add(bytes.len())
                    .filter(|n| *n <= INPUT_BYTE_CAPACITY)
            })
            .map_err(|_| "Eingabepuffer voll; Eingabe wurde nicht gesendet")?;
        let packet = InputPacket {
            bytes,
            queued_bytes: self.queued_input_bytes.clone(),
        };
        self.input
            .as_ref()
            .ok_or("Sitzung geschlossen")?
            .try_send(packet)
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => {
                    "Eingabepuffer voll; Eingabe wurde nicht gesendet".into()
                }
                mpsc::TrySendError::Disconnected(_) => "PTY-Eingabekanal geschlossen".into(),
            })
    }

    pub fn resize(&self, size: PtySize) -> Result<(), String> {
        self.master
            .as_ref()
            .ok_or("Sitzung geschlossen")?
            .resize(size)
            .map_err(|e| e.to_string())
    }

    /// Run outside the UI thread. Close channels, terminate the foreground job
    /// and shell, reap the child, and join transport threads.
    pub fn shutdown(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        self.input.take();
        self.output.take(); // release a reader blocked on the bounded queue
        #[cfg(windows)]
        if let Some(writer) = self.workers.get(1) {
            use std::os::windows::io::AsRawHandle;
            // The owned thread remains alive until join; cancel only its I/O.
            unsafe {
                windows_sys::Win32::System::IO::CancelSynchronousIo(writer.as_raw_handle());
            }
        }
        #[cfg(unix)]
        let foreground = self.master.as_ref().and_then(|m| m.process_group_leader());
        #[cfg(unix)]
        if matches!(self.state(), ProcessState::Running) {
            use nix::sys::signal::{Signal, killpg};
            use nix::unistd::Pid;
            if let Some(group) = foreground.filter(|p| *p > 1) {
                let _ = killpg(Pid::from_raw(group), Signal::SIGHUP);
            }
        }
        if matches!(self.state(), ProcessState::Running) {
            let _ = self.killer.kill();
        }
        self.master.take();
        let deadline = Instant::now() + Duration::from_millis(200);
        while matches!(self.state(), ProcessState::Running) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        #[cfg(unix)]
        if matches!(self.state(), ProcessState::Running) {
            use nix::sys::signal::{Signal, kill, killpg};
            use nix::unistd::Pid;
            if let Some(group) = foreground.filter(|p| *p > 1) {
                let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
            }
            if let Some(pid) = self.pid {
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.workers.is_empty() {
            self.stop();
        }
    }
}
