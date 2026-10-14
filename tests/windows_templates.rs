#![cfg(windows)]
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use hoori_term::session::{ProcessState, Session};
use hoori_term::tabs::{Accent, CloseTarget, Identity, Shell, Tabs};
use hoori_term::templates::{Catalog, Template, TemplateFile, TemplateId, TemplateShell};
use hoori_term::terminal::{GridSize, Terminal};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    temporary_root: PathBuf,
}

#[test]
#[ignore = "requires an explicitly selected registered WSL 2 distribution"]
fn configured_wsl_home_resolves_without_an_interactive_console() {
    let distribution =
        std::env::var("HOORI_TEST_WSL_DISTRO").expect("explicit WSL fixture required");
    let home = hoori_term::launch::wsl_home(&distribution).unwrap();
    assert!(home.starts_with('/'));
    assert!(!home.ends_with(['\r', '\n']));
    println!("Resolved {distribution} start directory: {home}");
}
impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let temporary_root = std::env::temp_dir().canonicalize().unwrap();
        let root = temporary_root.join(format!(
            "hoori-win-templates-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root,
            temporary_root,
        }
    }
    fn file(&self) -> TemplateFile {
        TemplateFile {
            path: self.root.join("config/templates-v1.json"),
        }
    }
    fn draft(&self) -> Template {
        Template {
            id: TemplateId(0),
            identity: Identity {
                name: Some("Windows work".into()),
                accent: Accent::Green,
                ..Identity::default()
            },
            shell: TemplateShell::PowerShell,
            wsl_distribution: None,
            directory: self.root.to_string_lossy().into_owned(),
            pinned: false,
        }
    }
    fn catalog(&self) -> Catalog {
        let mut tabs = Tabs::default();
        let id = tabs
            .open(
                &Shell {
                    program: r"C:\hoori-nonexistent-model.exe".into(),
                    args: vec![],
                    directory: None,
                },
                GridSize::new(80, 24),
            )
            .unwrap();
        let mut catalog = Catalog::default();
        catalog.save_tab(&mut tabs, id, self.draft()).unwrap();
        catalog
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let resolved = self.root.canonicalize().unwrap();
        assert_eq!(resolved.parent(), Some(self.temporary_root.as_path()));
        assert!(
            resolved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("hoori-win-templates-")
        );
        fs::remove_dir_all(resolved).unwrap();
    }
}

#[test]
fn windows_store_replaces_existing_data_and_preserves_locked_or_conflicting_files() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    let fixture = Fixture::new();
    let store = fixture.file();
    let mut catalog = fixture.catalog();
    let first = store.save(&catalog, None).unwrap();
    assert!(first.warning.is_none());
    assert_eq!(store.load().unwrap().catalog, catalog);
    catalog.items[0].identity.name = Some("Edited".into());
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&store.path)
        .unwrap();
    assert!(store.save(&catalog, Some(&first.stamp)).is_err());
    assert_eq!(fs::read(&store.path).unwrap(), first.stamp);
    drop(locked);
    let second = store.save(&catalog, Some(&first.stamp)).unwrap();
    assert!(second.warning.is_none());
    assert_ne!(first.stamp, second.stamp);
    assert_eq!(store.load().unwrap().catalog, catalog);
    fs::write(&store.path, b"external, incomplete data").unwrap();
    assert!(store.load().is_err());
    assert!(store.save(&catalog, Some(&second.stamp)).is_err());
    assert_eq!(fs::read(&store.path).unwrap(), b"external, incomplete data");
}

#[test]
fn directory_contexts_and_wsl_target_data_are_validated_before_mutation() {
    let fixture = Fixture::new();
    let mut catalog = fixture.catalog();
    let mut tabs = Tabs::default();
    let before = catalog.clone();
    let mut draft = catalog.items[0].clone();
    draft.shell = TemplateShell::Bash;
    assert!(catalog.edit(&mut tabs, draft.clone()).is_err());
    assert_eq!(catalog, before);
    draft.wsl_distribution = Some("Ubuntu 24.04; argument data".into());
    assert!(catalog.edit(&mut tabs, draft.clone()).is_err()); // Windows path is not a WSL directory.
    draft.directory = "/home/test/Grüße ; $(literal)".into();
    catalog.edit(&mut tabs, draft.clone()).unwrap();
    let encoded = catalog.to_bytes().unwrap();
    assert_eq!(Catalog::from_bytes(&encoded).unwrap(), catalog);
    draft.wsl_distribution = Some("-invalid".into());
    assert!(catalog.edit(&mut tabs, draft).is_err());
    assert_eq!(catalog.to_bytes().unwrap(), encoded);
}

#[test]
fn missing_native_start_directory_retains_the_failed_link_without_home_fallback() {
    let fixture = Fixture::new();
    let mut catalog = fixture.catalog();
    catalog.items[0].directory = fixture.root.join("missing").to_string_lossy().into_owned();
    let mut tabs = Tabs::default();
    let first = catalog
        .open(&mut tabs, TemplateId(1), GridSize::new(80, 24))
        .unwrap();
    assert!(matches!(
        tabs.current().unwrap().state(),
        ProcessState::Failed(_)
    ));
    assert_eq!(
        catalog
            .open(&mut tabs, TemplateId(1), GridSize::new(80, 24))
            .unwrap(),
        first
    );
    assert_eq!(tabs.items.len(), 1);
    assert_eq!(tabs.current().unwrap().template, Some(TemplateId(1)));
    for session in tabs.remove(CloseTarget::Window) {
        session.shutdown();
    }
}

#[test]
fn powershell_starts_in_the_literal_unicode_directory_and_exits_normally() {
    let fixture = Fixture::new();
    let directory = fixture.root.join("cwd Grüße 界 ; $(literal)");
    fs::create_dir(&directory).unwrap();
    let mut shell = TemplateShell::PowerShell.shell(directory.to_str().unwrap(), None);
    shell.args.push("-NoProfile".into());
    let size = GridSize::new(220, 32);
    let session = Session::spawn_in(
        &shell.program,
        &shell.args,
        shell.directory.as_deref(),
        size.pty(),
    )
    .unwrap();
    let mut terminal = Terminal::new(size);
    session
        .write(b"Write-Output ('CWD:' + (Get-Location).ProviderPath); exit 7\r".to_vec())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut bytes = Vec::new();
    while Instant::now() < deadline {
        while let Some(chunk) = session.read_chunk() {
            bytes.extend_from_slice(&chunk);
            for reply in terminal.feed(&chunk) {
                session.write(reply).unwrap();
            }
        }
        if session.state() == ProcessState::Exited(7) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = (0..size.rows as i32)
        .map(|row| {
            (0..size.columns as usize)
                .map(|column| &terminal.term.grid()[Line(row)][Column(column)])
                .filter(|cell| !cell.flags.contains(Flags::WIDE_CHAR_SPACER))
                .map(|cell| cell.c)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let reported = output
        .lines()
        .find_map(|line| line.strip_prefix("CWD:"))
        .unwrap_or_else(|| {
            panic!(
                "Native PowerShell did not report its start directory: {output:?}; raw={:?}",
                String::from_utf8_lossy(&bytes)
            )
        });
    assert_eq!(
        PathBuf::from(reported)
            .canonicalize()
            .unwrap_or_else(|error| panic!(
                "Cannot resolve reported cwd {reported:?}: {error}; raw={:?}",
                String::from_utf8_lossy(&bytes)
            )),
        directory.canonicalize().unwrap()
    );
    assert_eq!(session.state(), ProcessState::Exited(7));
    session.shutdown();
}
