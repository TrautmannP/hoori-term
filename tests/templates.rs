use hoori_term::session::ProcessState;
use hoori_term::shortcuts::Shortcut;
use hoori_term::tabs::{Accent, CloseTarget, Identity, Shell, TabId, Tabs};
use hoori_term::templates::{
    Catalog, MAX_FILE_BYTES, Template, TemplateFile, TemplateId, TemplateShell,
};
use hoori_term::terminal::GridSize;
use iced::keyboard::{Key, Modifiers, key::Named};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "hoori-templates-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> TemplateFile {
        TemplateFile {
            path: self.0.join("config/templates-v1.json"),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn model_tab(tabs: &mut Tabs) -> TabId {
    tabs.open(
        &Shell {
            program: "/nonexistent/template-model".into(),
            args: vec![],
            directory: None,
        },
        GridSize::new(80, 24),
    )
    .unwrap()
}
fn binding(key: Named) -> Shortcut {
    Shortcut::capture(&Key::Named(key), Modifiers::empty())
        .unwrap()
        .unwrap()
}
fn draft(name: &str, key: Named, directory: &str) -> Template {
    Template {
        id: TemplateId(0),
        identity: Identity {
            name: Some(name.into()),
            accent: Accent::Blue,
            shortcut: Some(binding(key)),
        },
        shell: TemplateShell::Bash,
        directory: directory.into(),
    }
}

#[test]
fn linked_identity_changes_and_removal_never_touch_terminal_or_process() {
    let mut tabs = Tabs::default();
    let normal = model_tab(&mut tabs);
    let linked = model_tab(&mut tabs);
    tabs.get_mut(linked)
        .unwrap()
        .terminal
        .feed(b"retained\x1b]2;runtime title\x07");
    let mut catalog = Catalog::default();
    let id = catalog
        .save_tab(&mut tabs, linked, draft("Backend", Named::F8, "/tmp"))
        .unwrap();
    assert_eq!(tabs.items.len(), 2); // No new PTY/Tab on save.
    assert_eq!(tabs.get(linked).unwrap().template, Some(id));
    tabs.items.swap(0, 1);
    assert!(
        catalog
            .configure_tab(
                &mut tabs,
                linked,
                "Web",
                Accent::Green,
                Some(binding(Named::F9))
            )
            .unwrap()
    );
    assert_eq!(catalog.get(id).unwrap().name(), "Web");
    assert!(
        !catalog
            .configure_tab(&mut tabs, normal, "Temporary", Accent::Red, None)
            .unwrap()
    );
    assert_eq!(catalog.items.len(), 1);
    assert_eq!(
        tabs.get(linked).unwrap().terminal.shell_title,
        "runtime title"
    );
    // Opening a still-open failed/exited tab is activation, never a restart.
    assert_eq!(
        catalog.open(&mut tabs, id, GridSize::new(90, 25)).unwrap(),
        linked
    );
    assert_eq!(tabs.items.len(), 2);
    assert!(matches!(
        tabs.current().unwrap().state(),
        ProcessState::Failed(_)
    ));
    let before = tabs.get(linked).unwrap().state();
    catalog.remove(&mut tabs, id).unwrap();
    assert_eq!(tabs.get(linked).unwrap().state(), before);
    assert_eq!(tabs.get(linked).unwrap().label(), "Web");
    assert!(tabs.get(linked).unwrap().identity.shortcut.is_none());
    assert!(tabs.get(linked).unwrap().template.is_none());
    assert!(
        catalog
            .shortcut_target(&Key::Named(Named::F9), Modifiers::empty())
            .is_none()
    );
    let text: String = tabs
        .get(linked)
        .unwrap()
        .terminal
        .term
        .grid()
        .display_iter()
        .map(|c| c.cell.c)
        .collect();
    assert!(text.contains("retained"));
    let next = catalog
        .save_tab(&mut tabs, linked, draft("New", Named::F9, "/tmp"))
        .unwrap();
    assert_ne!(id, next);
}

#[test]
fn conflicts_with_closed_templates_live_tabs_and_app_actions_are_atomic() {
    let mut tabs = Tabs::default();
    let saved = model_tab(&mut tabs);
    let ordinary = model_tab(&mut tabs);
    let mut catalog = Catalog::default();
    let id = catalog
        .save_tab(&mut tabs, saved, draft("Saved", Named::F8, "/tmp"))
        .unwrap();
    tabs.remove(CloseTarget::Tab(saved));
    let snapshot = catalog.clone();
    assert!(
        catalog
            .configure_tab(
                &mut tabs,
                ordinary,
                "Bad",
                Accent::Red,
                Some(binding(Named::F8))
            )
            .is_err()
    );
    assert!(
        catalog
            .save_tab(&mut tabs, ordinary, draft("Bad", Named::F8, "/tmp"))
            .is_err()
    );
    assert_eq!(catalog, snapshot);
    assert!(tabs.get(ordinary).unwrap().identity.name.is_none());
    catalog
        .configure_tab(
            &mut tabs,
            ordinary,
            "Ordinary",
            Accent::Green,
            Some(binding(Named::F9)),
        )
        .unwrap();
    let mut edit = catalog.get(id).unwrap().clone();
    edit.identity.name = Some("Changed".into());
    edit.identity.shortcut = Some(binding(Named::F9));
    assert!(catalog.edit(&mut tabs, edit.clone()).is_err());
    assert_eq!(catalog, snapshot);
    for key in ["t", "p", "s"] {
        edit.identity.shortcut = Shortcut::capture(
            &Key::Character(key.into()),
            Modifiers::CTRL | Modifiers::SHIFT,
        )
        .unwrap();
        assert!(catalog.edit(&mut tabs, edit.clone()).is_err());
        assert_eq!(catalog, snapshot);
    }
}

#[test]
fn closed_template_reopens_once_in_literal_directory_and_exited_tab_does_not_restart() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("space ; literal $(text)");
    fs::create_dir(&directory).unwrap();
    let mut tabs = Tabs::default();
    let initial = model_tab(&mut tabs);
    let mut catalog = Catalog::default();
    let id = catalog
        .save_tab(
            &mut tabs,
            initial,
            draft("Bash", Named::F8, directory.to_str().unwrap()),
        )
        .unwrap();
    tabs.remove(CloseTarget::Tab(initial));
    assert_eq!(
        catalog.shortcut_target(&Key::Named(Named::F8), Modifiers::empty()),
        Some(id)
    );
    let reopened = catalog.open(&mut tabs, id, GridSize::new(90, 24)).unwrap();
    assert_ne!(initial, reopened);
    assert_eq!(
        catalog.open(&mut tabs, id, GridSize::new(90, 24)).unwrap(),
        reopened
    );
    assert_eq!(tabs.items.len(), 1);
    tabs.get_mut(reopened).unwrap().send(
        b"unset HISTFILE; PROMPT_COMMAND=; PS1='TEST> '; printf '\\033]2;cwd:%s\\007' \"$PWD\"\n"
            .to_vec(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while tabs.get(reopened).unwrap().terminal.shell_title != format!("cwd:{}", directory.display())
    {
        assert!(
            Instant::now() < deadline,
            "literal start directory was not observed"
        );
        tabs.get_mut(reopened).unwrap().poll_output();
        thread::sleep(Duration::from_millis(5));
    }
    // Editing next-spawn data must not change the live transport or its cwd.
    let mut edit = catalog.get(id).unwrap().clone();
    edit.shell = TemplateShell::Zsh;
    edit.directory = "/nonexistent/hoori-template-directory".into();
    catalog.edit(&mut tabs, edit).unwrap();
    assert_eq!(tabs.get(reopened).unwrap().shell.program, "/bin/bash");
    assert_eq!(
        tabs.get(reopened).unwrap().shell.directory.as_ref(),
        Some(&directory)
    );
    tabs.get_mut(reopened).unwrap().send(b"exit 7\n".to_vec());
    while tabs.get(reopened).unwrap().state() != ProcessState::Exited(7) {
        assert!(Instant::now() < deadline);
        tabs.get_mut(reopened).unwrap().poll_output();
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        catalog.open(&mut tabs, id, GridSize::new(90, 24)).unwrap(),
        reopened
    );
    assert_eq!(tabs.current().unwrap().state(), ProcessState::Exited(7));
    for s in tabs.remove(CloseTarget::Tab(reopened)) {
        s.shutdown();
    }
    let failed = catalog.open(&mut tabs, id, GridSize::new(90, 24)).unwrap();
    assert!(matches!(
        tabs.get(failed).unwrap().state(),
        ProcessState::Failed(_)
    ));
    assert!(tabs.get(failed).unwrap().error.is_some());
    assert_eq!(
        catalog.get(id).unwrap().directory,
        "/nonexistent/hoori-template-directory"
    );
    assert_eq!(
        catalog.open(&mut tabs, id, GridSize::new(90, 24)).unwrap(),
        failed
    );
}

#[test]
fn format_roundtrips_and_rejects_corrupt_unsupported_or_unbounded_data() {
    let mut tabs = Tabs::default();
    let tab = model_tab(&mut tabs);
    let mut catalog = Catalog::default();
    let id = catalog
        .save_tab(&mut tabs, tab, draft("Grüße 界", Named::F12, "/tmp"))
        .unwrap();
    let bytes = catalog.to_bytes().unwrap();
    assert_eq!(Catalog::from_bytes(&bytes).unwrap(), catalog);
    for change in [
        "version",
        "next_id",
        "duplicate",
        "shortcut",
        "name",
        "directory",
        "unknown",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        match change {
            "version" => value["version"] = 2.into(),
            "next_id" => value["next_id"] = id.0.into(),
            "duplicate" => {
                let t = value["items"][0].clone();
                value["items"].as_array_mut().unwrap().push(t);
            }
            "shortcut" => value["items"][0]["identity"]["shortcut"]["modifiers"] = 255.into(),
            "name" => value["items"][0]["identity"]["name"] = "bad\u{202e}".into(),
            "directory" => value["items"][0]["directory"] = "relative".into(),
            _ => value["command"] = "not allowed".into(),
        }
        assert!(
            Catalog::from_bytes(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{change}"
        );
    }
    assert!(Catalog::from_bytes(b"{\"version\":1,").is_err());
    assert!(Catalog::from_bytes(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
    for key in [
        Key::Named(Named::F8),
        Key::Named(Named::Escape),
        Key::Named(Named::Space),
        Key::Character("ä".into()),
    ] {
        let shortcut = Shortcut::capture(&key, Modifiers::CTRL | Modifiers::ALT)
            .unwrap()
            .unwrap();
        let saved = serde_json::to_vec(&shortcut).unwrap();
        assert_eq!(
            serde_json::from_slice::<Shortcut>(&saved).unwrap(),
            shortcut
        );
    }
    assert!(Shortcut::capture(&Key::Character("İ".into()), Modifiers::CTRL).is_err());
}

#[test]
fn catalog_count_and_encoded_byte_limits_reject_edits_before_mutating_configuration() {
    let items: Vec<_> = (1..=64).map(|id| serde_json::json!({
        "id": id, "identity": {"name": format!("Profile {id}"),"accent":"default","shortcut":null},
        "shell":"bash","directory":"/tmp"
    })).collect();
    let bytes =
        serde_json::to_vec(&serde_json::json!({"version":1,"next_id":65,"items":items})).unwrap();
    let mut catalog = Catalog::from_bytes(&bytes).unwrap();
    let mut tabs = Tabs::default();
    let tab = model_tab(&mut tabs);
    assert!(
        catalog
            .save_tab(&mut tabs, tab, draft("Too many", Named::F8, "/tmp"))
            .is_err()
    );
    assert!(tabs.get(tab).unwrap().template.is_none());
    let mut rejected = false;
    for index in 0..catalog.items.len() {
        let before = catalog.clone();
        let mut edit = catalog.items[index].clone();
        edit.directory = format!("/{}", "\"".repeat(4095));
        if let Err(error) = catalog.edit(&mut tabs, edit) {
            assert!(error.contains("256 KiB"));
            assert_eq!(catalog, before);
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    assert!(catalog.to_bytes().unwrap().len() <= MAX_FILE_BYTES);
}

#[test]
fn atomic_file_store_preserves_existing_data_on_conflicts_and_write_errors() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let initial = file.load().unwrap();
    assert!(initial.stamp.is_none());
    let saved = file.save(&initial.catalog, None).unwrap();
    assert!(saved.warning.is_none());
    let loaded = file.load().unwrap();
    assert_eq!(loaded.stamp.as_ref(), Some(&saved.stamp));
    assert_eq!(loaded.catalog, initial.catalog);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(file.save(&loaded.catalog, None).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), saved.stamp);
    // Deterministic failure before replacement: the lock path is a directory.
    let lock = file.path.with_extension("lock");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(file.save(&loaded.catalog, loaded.stamp.as_deref()).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), saved.stamp);
    fs::remove_dir(&lock).unwrap();
    let corrupt = b"{unfinished";
    fs::write(&file.path, corrupt).unwrap();
    assert!(file.load().is_err());
    assert!(file.save(&loaded.catalog, loaded.stamp.as_deref()).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), corrupt);
    fs::write(&file.path, vec![b' '; MAX_FILE_BYTES + 1]).unwrap();
    assert!(file.load().is_err());
    assert_eq!(
        fs::metadata(&file.path).unwrap().len(),
        (MAX_FILE_BYTES + 1) as u64
    );
    assert!(
        !fs::read_dir(file.path.parent().unwrap()).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
}

#[test]
fn interrupted_temporary_file_and_symlink_do_not_replace_valid_configuration() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let saved = file.save(&Catalog::default(), None).unwrap();
    let interrupted = file
        .path
        .parent()
        .unwrap()
        .join(".templates-interrupted.tmp");
    fs::write(&interrupted, b"unfinished new snapshot").unwrap();
    assert_eq!(file.load().unwrap().stamp.as_ref(), Some(&saved.stamp));
    let next = file.save(&Catalog::default(), Some(&saved.stamp)).unwrap();
    assert_eq!(next.stamp, saved.stamp);
    assert_eq!(fs::read(&interrupted).unwrap(), b"unfinished new snapshot");
    #[cfg(unix)]
    {
        let link = TemplateFile {
            path: fixture.0.join("symlink.json"),
        };
        std::os::unix::fs::symlink(&file.path, &link.path).unwrap();
        assert!(link.load().is_err());
        assert!(link.save(&Catalog::default(), None).is_err());
        assert_eq!(fs::read(&file.path).unwrap(), saved.stamp);
    }
}
