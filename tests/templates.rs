// These are Ubuntu template paths/lifecycle checks. Windows template support
// and its persistence matrix belong to HOO-14.
#![cfg(unix)]
use hoori_term::session::ProcessState;
use hoori_term::shortcuts::Shortcut;
use hoori_term::symbols::Symbol;
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
            symbol: None,
            compact: false,
        },
        shell: TemplateShell::Bash,
        wsl_distribution: None,
        directory: directory.into(),
        pinned: false,
    }
}

fn identity(
    name: &str,
    accent: Accent,
    shortcut: Option<Shortcut>,
    symbol: Option<Symbol>,
) -> Identity {
    Identity {
        name: Some(name.into()),
        accent,
        shortcut,
        symbol,
        compact: false,
    }
}

fn ordered_fixture(count: usize) -> (Tabs, Catalog, Vec<TabId>, Vec<TemplateId>) {
    let mut tabs = Tabs::default();
    let mut catalog = Catalog::default();
    let live: Vec<_> = (0..count).map(|_| model_tab(&mut tabs)).collect();
    let saved = live
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let mut template = draft(
                &format!("Saved {index}"),
                Named::F8,
                "/nonexistent/hoo26-order",
            );
            template.identity.shortcut = None;
            template.identity.symbol = Some(Symbol::Star);
            template.identity.compact = index % 2 == 0;
            catalog.save_tab(&mut tabs, *tab, template).unwrap()
        })
        .collect();
    (tabs, catalog, live, saved)
}

#[test]
fn template_order_changes_only_the_moved_link_with_closed_neighbors_and_first_save() {
    let (mut tabs, mut catalog, live, saved) = ordered_fixture(6);
    for index in [0, 2, 4] {
        catalog.set_pinned(&mut tabs, live[index], true).unwrap();
    }
    tabs.remove(CloseTarget::PinnedTab(live[2]));
    tabs.remove(CloseTarget::Tab(live[3]));
    let ordinary = model_tab(&mut tabs);
    let other = model_tab(&mut tabs);
    let active = tabs.active;
    let ids = |tabs: &Tabs| tabs.items.iter().map(|tab| tab.id).collect::<Vec<_>>();
    assert!(
        catalog
            .move_tab(&mut tabs, live[4], live[0], false)
            .unwrap()
    );
    assert_eq!(
        catalog.order(),
        [saved[4], saved[0], saved[1], saved[2], saved[3], saved[5]]
    );
    assert!(catalog.move_tab(&mut tabs, live[1], live[5], true).unwrap());
    assert_eq!(
        catalog.order(),
        [saved[4], saved[0], saved[2], saved[3], saved[5], saved[1]]
    );
    assert!(
        catalog
            .move_tab(&mut tabs, live[1], live[5], false)
            .unwrap()
    );
    assert_eq!(
        catalog.order(),
        [saved[4], saved[0], saved[2], saved[3], saved[1], saved[5]]
    );
    let before = catalog.clone();
    assert!(
        !catalog
            .move_tab(&mut tabs, ordinary, live[5], false)
            .unwrap()
    );
    assert_eq!(catalog, before); // Temporary tabs never become durable anchors.
    assert!(
        !catalog
            .move_tab(&mut tabs, live[1], ordinary, false)
            .unwrap()
    ); // Same live position.
    assert_eq!(catalog, before);
    let live_before_save = ids(&tabs);
    let new = catalog
        .save_tab(
            &mut tabs,
            ordinary,
            draft("First save", Named::F8, "/nonexistent/hoo26-order"),
        )
        .unwrap();
    assert_eq!(
        catalog.order(),
        [
            saved[4], saved[0], saved[2], saved[3], saved[1], new, saved[5]
        ]
    );
    assert_eq!(ids(&tabs), live_before_save);
    assert_eq!(tabs.active, active);
    assert_eq!(
        catalog.shortcut_target(&Key::Named(Named::F8), Modifiers::empty()),
        Some(new)
    );
    // No other linked tab in the pin group: moving around an ordinary tab
    // does not invent a saved position or move an entry from the other group.
    tabs.remove(CloseTarget::PinnedTab(live[0]));
    tabs.set_pinned(other, true).unwrap();
    let before = catalog.clone();
    assert!(!catalog.move_tab(&mut tabs, live[4], other, true).unwrap());
    assert_eq!(catalog, before);
    let live_before = ids(&tabs);
    assert!(
        catalog
            .move_tab(&mut tabs, live[4], live[5], false)
            .is_err()
    );
    assert!(
        catalog
            .move_tab(&mut tabs, ordinary, live[3], true)
            .is_err()
    );
    assert_eq!(ids(&tabs), live_before);
    assert_eq!(catalog, before);
    let order_before = catalog.order().to_vec();
    catalog.set_pinned(&mut tabs, ordinary, true).unwrap();
    catalog.set_pinned(&mut tabs, ordinary, false).unwrap();
    assert_eq!(catalog.order(), order_before); // Pin changes are a distinct action.
    let before_remove = ids(&tabs);
    let identity = tabs.get(ordinary).unwrap().identity.clone();
    catalog.remove(&mut tabs, new).unwrap();
    assert_eq!(ids(&tabs), before_remove);
    let tab = tabs.get(ordinary).unwrap();
    assert!(tab.template.is_none() && tab.identity.shortcut.is_none());
    assert_eq!(
        (
            &tab.identity.name,
            tab.identity.symbol,
            tab.identity.compact,
            tab.pinned
        ),
        (&identity.name, identity.symbol, identity.compact, false)
    );
    let before_close = catalog.clone();
    tabs.remove(CloseTarget::Tab(live[5]));
    assert_eq!(catalog, before_close);
    assert_eq!(
        Catalog::from_bytes(&catalog.to_bytes().unwrap()).unwrap(),
        catalog
    );
}

#[test]
fn explicit_reopen_follows_saved_order_within_each_group_and_preserves_existing_tabs() {
    let (mut tabs, mut catalog, live, saved) = ordered_fixture(6);
    for index in [0, 2, 4] {
        catalog.set_pinned(&mut tabs, live[index], true).unwrap();
    }
    catalog
        .move_tab(&mut tabs, live[4], live[0], false)
        .unwrap();
    catalog.move_tab(&mut tabs, live[1], live[5], true).unwrap();
    assert_eq!(
        catalog.order(),
        [saved[4], saved[0], saved[2], saved[3], saved[5], saved[1]]
    );
    let catalog = Catalog::from_bytes(&catalog.to_bytes().unwrap()).unwrap();
    for sequence in [vec![1, 5, 3, 2, 0, 4], vec![4, 3, 0, 1, 2, 5]] {
        let mut restarted = Tabs::default();
        let ordinary = model_tab(&mut restarted);
        assert_eq!(restarted.items.len(), 1); // Loading configuration never starts a template.
        let mut opened = Vec::new();
        for index in sequence {
            let before: Vec<_> = restarted.items.iter().map(|tab| tab.id).collect();
            let new = catalog
                .open(&mut restarted, saved[index], GridSize::new(80, 24))
                .unwrap();
            assert_eq!(
                restarted
                    .items
                    .iter()
                    .filter(|tab| tab.id != new)
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                before
            );
            opened.push(new);
        }
        let links: Vec<_> = restarted.items.iter().map(|tab| tab.template).collect();
        assert_eq!(
            links,
            [
                Some(saved[4]),
                Some(saved[0]),
                Some(saved[2]),
                None,
                Some(saved[3]),
                Some(saved[5]),
                Some(saved[1])
            ]
        );
        assert_eq!(restarted.items[3].id, ordinary);
        let before: Vec<_> = restarted.items.iter().map(|tab| tab.id).collect();
        let count = before.len();
        let again = catalog
            .open(&mut restarted, saved[0], GridSize::new(111, 35))
            .unwrap();
        assert_eq!(restarted.active, Some(again));
        assert_eq!(restarted.items.len(), count);
        assert_eq!(
            restarted.items.iter().map(|tab| tab.id).collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            restarted.get(again).unwrap().terminal.size,
            GridSize::new(80, 24)
        );
        assert!(matches!(
            restarted.get(again).unwrap().state(),
            ProcessState::Failed(_)
        ));
    }
}

#[test]
fn saved_id_order_loads_legacy_rejects_bad_files_and_recovers_from_failed_atomic_write() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let (mut tabs, mut catalog, live, saved) = ordered_fixture(3);
    let first = file.save(&catalog, None).unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&first.stamp).unwrap();
    legacy.as_object_mut().unwrap().remove("order");
    legacy["items"].as_array_mut().unwrap().swap(0, 2);
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(&file.path, &bytes).unwrap();
    let loaded = file.load().unwrap();
    assert_eq!(loaded.catalog.order(), [saved[2], saved[1], saved[0]]);
    assert_eq!(
        loaded
            .catalog
            .ordered()
            .map(Template::name)
            .collect::<Vec<_>>(),
        ["Saved 2", "Saved 1", "Saved 0"]
    );
    assert_eq!(fs::read(&file.path).unwrap(), bytes);
    for invalid in [
        serde_json::json!(null),
        serde_json::json!(42),
        serde_json::json!([]),
        serde_json::json!([1, 2]),
        serde_json::json!([1, 1, 3]),
        serde_json::json!([1, 2, 999]),
        serde_json::json!([1, 2, "3"]),
        serde_json::json!([1, 2, -3]),
        serde_json::json!([1, 2, 3, 4]),
    ] {
        legacy["order"] = invalid;
        let broken = serde_json::to_vec(&legacy).unwrap();
        fs::write(&file.path, &broken).unwrap();
        assert!(file.load().is_err());
        assert!(file.save(&catalog, None).is_err());
        assert_eq!(fs::read(&file.path).unwrap(), broken);
    }
    fs::write(&file.path, &first.stamp).unwrap();
    assert!(
        catalog
            .move_tab(&mut tabs, live[2], live[0], false)
            .unwrap()
    );
    assert_eq!(catalog.order(), [saved[2], saved[0], saved[1]]);
    let lock = file.path.with_extension("lock");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(file.save(&catalog, Some(&first.stamp)).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), first.stamp);
    fs::remove_dir(&lock).unwrap();
    let retry = file.save(&catalog, Some(&first.stamp)).unwrap();
    assert_eq!(
        file.load().unwrap().catalog.order(),
        [saved[2], saved[0], saved[1]]
    );
    assert_ne!(retry.stamp, first.stamp);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&file.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
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
                identity("Web", Accent::Green, Some(binding(Named::F9)), None),
            )
            .unwrap()
    );
    assert_eq!(catalog.get(id).unwrap().name(), "Web");
    assert!(
        !catalog
            .configure_tab(
                &mut tabs,
                normal,
                identity("Temporary", Accent::Red, None, None)
            )
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
                identity("Bad", Accent::Red, Some(binding(Named::F8)), None),
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
            identity("Ordinary", Accent::Green, Some(binding(Named::F9)), None),
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

#[test]
fn pin_state_survives_save_reopen_legacy_data_and_failed_persistence() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let mut tabs = Tabs::default();
    let tab = model_tab(&mut tabs);
    tabs.set_pinned(tab, true).unwrap();
    let mut catalog = Catalog::default();
    let id = catalog
        .save_tab(&mut tabs, tab, draft("Pinned", Named::F8, "/tmp"))
        .unwrap();
    assert!(catalog.get(id).unwrap().pinned); // Save inherits actual tab state.
    let saved = file.save(&catalog, None).unwrap();
    assert_eq!(file.load().unwrap().catalog, catalog);
    let mut legacy: serde_json::Value = serde_json::from_slice(&saved.stamp).unwrap();
    legacy["items"][0].as_object_mut().unwrap().remove("pinned");
    assert!(
        !Catalog::from_bytes(&serde_json::to_vec(&legacy).unwrap())
            .unwrap()
            .get(id)
            .unwrap()
            .pinned
    );
    legacy["items"][0]["pinned"] = "true".into();
    assert!(Catalog::from_bytes(&serde_json::to_vec(&legacy).unwrap()).is_err());
    tabs.remove(CloseTarget::PinnedTab(tab));
    let mut restarted = Tabs::default();
    let ordinary = model_tab(&mut restarted);
    let mut loaded = file.load().unwrap().catalog;
    assert_eq!(restarted.items.len(), 1); // Loading never starts templates.
    // Use a nonexistent future cwd so no real shell is needed in this model test.
    let mut draft = loaded.get(id).unwrap().clone();
    draft.directory = "/nonexistent/hoo17-fixture".into();
    loaded.edit(&mut restarted, draft).unwrap();
    let reopened = loaded
        .open(&mut restarted, id, GridSize::new(80, 24))
        .unwrap();
    assert_eq!(restarted.items[0].id, reopened);
    assert!(restarted.get(reopened).unwrap().pinned);
    assert_eq!(
        loaded
            .open(&mut restarted, id, GridSize::new(80, 24))
            .unwrap(),
        reopened
    );
    assert_eq!(restarted.items.len(), 2);
    assert!(loaded.set_pinned(&mut restarted, reopened, false).unwrap());
    assert!(!loaded.get(id).unwrap().pinned);
    let lock = file.path.with_extension("lock");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(file.save(&loaded, Some(&saved.stamp)).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), saved.stamp); // On-disk pin remains true.
    fs::remove_dir(&lock).unwrap();
    file.save(&loaded, Some(&saved.stamp)).unwrap();
    assert!(!file.load().unwrap().catalog.get(id).unwrap().pinned);
    loaded.set_pinned(&mut restarted, reopened, true).unwrap();
    loaded.remove(&mut restarted, id).unwrap();
    assert!(restarted.get(reopened).unwrap().pinned);
    assert!(restarted.get(reopened).unwrap().template.is_none());
    assert!(restarted.get(reopened).unwrap().identity.shortcut.is_none());
    assert_eq!(
        restarted.items.iter().map(|t| t.id).collect::<Vec<_>>(),
        [reopened, ordinary]
    );
}

#[test]
fn symbols_roundtrip_validate_and_preserve_linked_identity_through_failed_storage_and_unlink() {
    use hoori_term::symbols::Symbol;
    let fixture = Fixture::new();
    let file = fixture.file();
    let mut tabs = Tabs::default();
    let tab = model_tab(&mut tabs);
    let mut catalog = Catalog::default();
    catalog
        .configure_tab(
            &mut tabs,
            tab,
            identity(
                "Symbols",
                Accent::Green,
                Some(binding(Named::F8)),
                Some(Symbol::Code),
            ),
        )
        .unwrap();
    assert!(catalog.items.is_empty()); // Symbol choice never saves an ordinary tab.
    tabs.set_pinned(tab, true).unwrap();
    tabs.get_mut(tab)
        .unwrap()
        .terminal
        .feed(b"kept\x1b]2;user-symbol-is-not-output\x07");
    let saved_identity = tabs.get(tab).unwrap().identity.clone();
    let mut saved_draft = draft("Symbols", Named::F8, "/tmp");
    saved_draft.identity = saved_identity.clone();
    let id = catalog.save_tab(&mut tabs, tab, saved_draft).unwrap();
    assert_eq!(catalog.get(id).unwrap().identity, saved_identity);
    let stamp = file.save(&catalog, None).unwrap().stamp;
    for symbol in Symbol::ALL {
        catalog
            .configure_tab(
                &mut tabs,
                tab,
                identity(
                    "Symbols",
                    Accent::Green,
                    Some(binding(Named::F8)),
                    Some(symbol),
                ),
            )
            .unwrap();
        let loaded = Catalog::from_bytes(&catalog.to_bytes().unwrap()).unwrap();
        assert_eq!(loaded.get(id).unwrap().identity.symbol, Some(symbol));
        assert_eq!(tabs.get(tab).unwrap().template, Some(id));
        assert!(tabs.get(tab).unwrap().pinned);
        assert_eq!(
            tabs.get(tab).unwrap().terminal.shell_title,
            "user-symbol-is-not-output"
        );
        assert_eq!(
            tabs.shortcut_target(&Key::Named(Named::F8), Modifiers::empty()),
            Some(tab)
        );
    }
    let mut legacy: serde_json::Value = serde_json::from_slice(&stamp).unwrap();
    legacy["items"][0]["identity"]
        .as_object_mut()
        .unwrap()
        .remove("symbol");
    assert!(
        Catalog::from_bytes(&serde_json::to_vec(&legacy).unwrap())
            .unwrap()
            .get(id)
            .unwrap()
            .identity
            .symbol
            .is_none()
    );
    for bad in [
        serde_json::json!("unknown-icon"),
        serde_json::json!(42),
        serde_json::json!({"path": "/tmp/icon.svg"}),
    ] {
        legacy["items"][0]["identity"]["symbol"] = bad;
        let bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(&file.path, &bytes).unwrap();
        assert!(file.load().is_err());
        assert_eq!(fs::read(&file.path).unwrap(), bytes);
    }
    fs::write(&file.path, &stamp).unwrap();
    let before = catalog.clone();
    let tab_before = tabs.get(tab).unwrap().identity.clone();
    assert!(
        catalog
            .configure_tab(
                &mut tabs,
                tab,
                identity("bad\nname", Accent::Red, None, None)
            )
            .is_err()
    );
    assert_eq!(catalog, before);
    assert_eq!(tabs.get(tab).unwrap().identity, tab_before);
    catalog
        .configure_tab(
            &mut tabs,
            tab,
            identity("Symbols", Accent::Green, Some(binding(Named::F8)), None),
        )
        .unwrap();
    assert!(tabs.get(tab).unwrap().identity.symbol.is_none());
    let lock = file.path.with_extension("lock");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(file.save(&catalog, Some(&stamp)).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), stamp);
    fs::remove_dir(&lock).unwrap();
    file.save(&catalog, Some(&stamp)).unwrap();
    assert!(
        file.load()
            .unwrap()
            .catalog
            .get(id)
            .unwrap()
            .identity
            .symbol
            .is_none()
    );
    catalog
        .configure_tab(
            &mut tabs,
            tab,
            identity(
                "Symbols",
                Accent::Green,
                Some(binding(Named::F8)),
                Some(Symbol::Folder),
            ),
        )
        .unwrap();
    tabs.remove(CloseTarget::PinnedTab(tab));
    assert_eq!(catalog.items.len(), 1);
    let ordinary = model_tab(&mut tabs);
    let reopened = catalog.open(&mut tabs, id, GridSize::new(80, 12)).unwrap();
    assert_ne!(reopened, tab);
    assert_eq!(
        catalog.open(&mut tabs, id, GridSize::new(80, 12)).unwrap(),
        reopened
    );
    assert_eq!(tabs.items.len(), 2);
    assert!(tabs.get(ordinary).unwrap().identity.symbol.is_none());
    catalog.remove(&mut tabs, id).unwrap();
    assert_eq!(
        tabs.get(reopened).unwrap().identity.symbol,
        Some(Symbol::Folder)
    );
    assert!(tabs.get(reopened).unwrap().pinned);
    assert!(tabs.get(reopened).unwrap().identity.shortcut.is_none());
    assert!(tabs.get(reopened).unwrap().template.is_none());
}

#[test]
fn compact_presentation_is_independent_and_survives_template_storage_errors_and_lifetime() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let mut tabs = Tabs::default();
    let tab = model_tab(&mut tabs);
    let other = model_tab(&mut tabs);
    let mut catalog = Catalog::default();
    catalog
        .configure_tab(
            &mut tabs,
            tab,
            identity(
                "Identical names stay identifiable",
                Accent::Green,
                Some(binding(Named::F8)),
                None,
            ),
        )
        .unwrap();
    let initial = tabs.get(tab).unwrap().identity.clone();
    let other_identity = tabs.get(other).unwrap().identity.clone();
    assert!(!initial.compact && initial.display_symbol().is_none());
    tabs.get_mut(tab)
        .unwrap()
        .terminal
        .feed(b"retained\x1b]2;runtime does not set compact\x07");
    for (pinned, compact) in [
        (false, true),
        (true, true),
        (true, false),
        (false, false),
        (false, true),
    ] {
        let previous_compact = tabs.get(tab).unwrap().identity.compact;
        assert!(!catalog.set_pinned(&mut tabs, tab, pinned).unwrap());
        assert_eq!(tabs.get(tab).unwrap().identity.compact, previous_compact);
        assert!(!catalog.set_compact(&mut tabs, tab, compact).unwrap());
        let source = tabs.get(tab).unwrap();
        assert_eq!(source.pinned, pinned);
        assert_eq!(source.identity.compact, compact);
        assert_eq!(source.identity.name, initial.name);
        assert_eq!(source.identity.accent, initial.accent);
        assert_eq!(source.identity.shortcut, initial.shortcut);
        assert!(source.identity.symbol.is_none());
        assert_eq!(
            source.identity.display_symbol(),
            compact.then_some(Symbol::Terminal)
        );
        assert_eq!(source.terminal.shell_title, "runtime does not set compact");
        assert_eq!(
            tabs.shortcut_target(&Key::Named(Named::F8), Modifiers::empty()),
            Some(tab)
        );
        assert_eq!(tabs.get(other).unwrap().identity, other_identity);
        assert!(catalog.items.is_empty()); // Neither fallback nor compaction implicitly saves.
    }
    // Other identity edits preserve compact presentation, including the older
    // name/accent/shortcut entry point, rather than resetting its default.
    tabs.configure(
        tab,
        "Saved compact",
        Accent::Green,
        Some(binding(Named::F8)),
    )
    .unwrap();
    assert!(tabs.get(tab).unwrap().identity.compact);
    let mut chosen = tabs.get(tab).unwrap().identity.clone();
    chosen.symbol = Some(Symbol::Star);
    catalog.configure_tab(&mut tabs, tab, chosen).unwrap();
    assert_eq!(
        tabs.get(tab).unwrap().identity.display_symbol(),
        Some(Symbol::Star)
    );
    catalog.set_pinned(&mut tabs, tab, true).unwrap();
    let mut saved = draft("Saved compact", Named::F8, "/nonexistent/hoo24-future-cwd");
    saved.identity = tabs.get(tab).unwrap().identity.clone();
    let id = catalog.save_tab(&mut tabs, tab, saved).unwrap();
    assert_eq!(tabs.items.len(), 2);
    assert!(catalog.get(id).unwrap().identity.compact && catalog.get(id).unwrap().pinned);
    let first = file.save(&catalog, None).unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&first.stamp).unwrap();
    legacy["items"][0]["identity"]
        .as_object_mut()
        .unwrap()
        .remove("compact");
    let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(&file.path, &legacy_bytes).unwrap();
    assert!(
        !file
            .load()
            .unwrap()
            .catalog
            .get(id)
            .unwrap()
            .identity
            .compact
    );
    assert_eq!(fs::read(&file.path).unwrap(), legacy_bytes); // Loading does not rewrite legacy data.
    for bad in [
        serde_json::json!("true"),
        serde_json::json!(null),
        serde_json::json!(42),
        serde_json::json!([]),
    ] {
        legacy["items"][0]["identity"]["compact"] = bad;
        let bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(&file.path, &bytes).unwrap();
        assert!(file.load().is_err());
        assert_eq!(fs::read(&file.path).unwrap(), bytes);
    }
    fs::write(&file.path, &first.stamp).unwrap();
    assert!(catalog.set_compact(&mut tabs, tab, false).unwrap());
    assert!(tabs.get(tab).unwrap().pinned);
    assert_eq!(tabs.get(tab).unwrap().template, Some(id));
    let before = catalog.clone();
    let source_before = tabs.get(tab).unwrap().identity.clone();
    let mut invalid = source_before.clone();
    invalid.name = Some("bad\nname".into());
    invalid.compact = true;
    assert!(catalog.configure_tab(&mut tabs, tab, invalid).is_err());
    assert_eq!(catalog, before);
    assert_eq!(tabs.get(tab).unwrap().identity, source_before);
    let lock = file.path.with_extension("lock");
    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    assert!(file.save(&catalog, Some(&first.stamp)).is_err());
    assert_eq!(fs::read(&file.path).unwrap(), first.stamp); // On-disk compact remains true.
    assert!(!catalog.get(id).unwrap().identity.compact); // Pending model is not falsely durable.
    fs::remove_dir(&lock).unwrap();
    let retry = file.save(&catalog, Some(&first.stamp)).unwrap();
    assert!(
        !file
            .load()
            .unwrap()
            .catalog
            .get(id)
            .unwrap()
            .identity
            .compact
    );
    assert!(catalog.set_compact(&mut tabs, tab, true).unwrap());
    assert!(catalog.set_pinned(&mut tabs, tab, false).unwrap());
    assert!(tabs.get(tab).unwrap().identity.compact);
    assert!(catalog.set_pinned(&mut tabs, tab, true).unwrap());
    file.save(&catalog, Some(&retry.stamp)).unwrap();
    let grid: String = tabs
        .get(tab)
        .unwrap()
        .terminal
        .term
        .grid()
        .display_iter()
        .map(|c| c.cell.c)
        .collect();
    assert!(grid.contains("retained"));
    tabs.remove(CloseTarget::PinnedTab(tab));
    assert_eq!(catalog.items.len(), 1); // Closing does not delete saved presentation.
    let mut loaded = file.load().unwrap().catalog;
    let mut restarted = Tabs::default();
    let ordinary = model_tab(&mut restarted);
    assert!(!restarted.get(ordinary).unwrap().identity.compact);
    assert_eq!(restarted.items.len(), 1); // Loading starts no saved sessions.
    let reopened = loaded
        .open(&mut restarted, id, GridSize::new(80, 24))
        .unwrap();
    assert_ne!(reopened, ordinary);
    assert!(restarted.get(reopened).unwrap().identity.compact);
    assert!(restarted.get(reopened).unwrap().pinned);
    assert_eq!(
        loaded
            .open(&mut restarted, id, GridSize::new(80, 24))
            .unwrap(),
        reopened
    );
    assert_eq!(restarted.items.len(), 2);
    assert!(restarted.remove(CloseTarget::Tab(reopened)).is_empty());
    assert!(restarted.get(reopened).is_some());
    loaded.remove(&mut restarted, id).unwrap();
    let source = restarted.get(reopened).unwrap();
    assert!(source.identity.compact && source.pinned);
    assert_eq!(source.identity.symbol, Some(Symbol::Star));
    assert!(source.template.is_none() && source.identity.shortcut.is_none());
}
