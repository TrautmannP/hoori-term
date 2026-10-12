use hoori_term::shortcuts::Shortcut;
use hoori_term::tabs::{Accent, CloseTarget, MAX_NAME_CHARS, Shell, TabId, Tabs};
use hoori_term::terminal::GridSize;
use iced::keyboard::{Key, Modifiers, key::Named};

fn open(tabs: &mut Tabs) -> TabId {
    // This is a model test, deliberately without a live transport.
    tabs.open(
        &Shell {
            program: "/nonexistent/model-shell".into(),
            args: vec![],
            directory: None,
        },
        GridSize::new(80, 12),
    )
    .unwrap()
}

fn shortcut(key: Key, modifiers: Modifiers) -> Shortcut {
    Shortcut::capture(&key, modifiers).unwrap().unwrap()
}

#[test]
fn identity_and_bindings_survive_titles_reordering_and_other_tab_removal() {
    let mut tabs = Tabs::default();
    let first = open(&mut tabs);
    let target = open(&mut tabs);
    let third = open(&mut tabs);
    let key = Key::Named(Named::F8);
    let chord = shortcut(key.clone(), Modifiers::empty());
    tabs.get_mut(target)
        .unwrap()
        .terminal
        .feed(b"preserved terminal state\x1b]2;dynamic title\x07");
    tabs.configure(target, " Backend ", Accent::Green, Some(chord.clone()))
        .unwrap();
    assert_eq!(tabs.get(target).unwrap().label(), "Backend");
    assert_eq!(
        tabs.get(target).unwrap().terminal.shell_title,
        "dynamic title"
    );
    assert_eq!(tabs.shortcut_target(&key, Modifiers::empty()), Some(target));
    tabs.items.swap(0, 2);
    assert_eq!(tabs.shortcut_target(&key, Modifiers::empty()), Some(target));
    tabs.remove(CloseTarget::Tab(first));
    let next = open(&mut tabs);
    assert_ne!(next, first);
    assert_eq!(tabs.shortcut_target(&key, Modifiers::empty()), Some(target));
    tabs.configure(target, "Logs", Accent::Orange, Some(chord))
        .unwrap();
    tabs.get_mut(target)
        .unwrap()
        .terminal
        .feed(b"\x1b]2;another shell title\x07");
    tabs.activate(tabs.shortcut_target(&key, Modifiers::empty()).unwrap());
    assert_eq!(tabs.current().unwrap().id, target);
    assert_eq!(
        tabs.current().unwrap().description(),
        format!("Logs · {}", target.0)
    );
    assert_eq!(tabs.current().unwrap().identity.accent, Accent::Orange);
    let text: String = tabs
        .current()
        .unwrap()
        .terminal
        .term
        .grid()
        .display_iter()
        .map(|c| c.cell.c)
        .collect();
    assert!(text.contains("preserved terminal state"));
    assert!(tabs.get(third).unwrap().identity.shortcut.is_none());
    tabs.remove(CloseTarget::Tab(target));
    assert_eq!(tabs.shortcut_target(&key, Modifiers::empty()), None);
    let reopened = open(&mut tabs);
    assert_ne!(reopened, target);
    assert!(tabs.get(reopened).unwrap().identity.shortcut.is_none());
}

#[test]
fn conflicting_or_invalid_edits_leave_all_existing_settings_intact() {
    let mut tabs = Tabs::default();
    let first = open(&mut tabs);
    let second = open(&mut tabs);
    let chord = shortcut(Key::Character("a".into()), Modifiers::CTRL);
    tabs.configure(first, "Backend", Accent::Green, Some(chord.clone()))
        .unwrap();
    tabs.configure(second, "Frontend", Accent::Blue, None)
        .unwrap();
    let before = tabs.get(second).unwrap().identity.clone();
    let error = tabs
        .configure(second, "Changed", Accent::Red, Some(chord))
        .unwrap_err();
    assert!(error.contains("Backend"));
    assert_eq!(tabs.get(second).unwrap().identity, before);
    let reserved = shortcut(
        Key::Character("T".into()),
        Modifiers::CTRL | Modifiers::SHIFT,
    );
    assert!(
        tabs.configure(second, "Changed", Accent::Red, Some(reserved))
            .unwrap_err()
            .contains("reserviert")
    );
    for name in [
        "bad\nname".into(),
        "bad\u{202e}name".into(),
        "bad\u{200e}name".into(),
        "bad\u{061c}name".into(),
        "x".repeat(MAX_NAME_CHARS + 1),
    ] {
        assert!(tabs.configure(second, &name, Accent::Red, None).is_err());
        assert_eq!(tabs.get(second).unwrap().identity, before);
    }
    tabs.configure(second, "Grüße 界", Accent::Purple, None)
        .unwrap();
    assert_eq!(tabs.get(second).unwrap().label(), "Grüße 界");
    tabs.configure(second, "", Accent::Default, None).unwrap();
    assert!(tabs.get(second).unwrap().identity.name.is_none());
    assert_eq!(tabs.get(second).unwrap().label(), "model-shell · 2");
    tabs.remove(CloseTarget::Tab(second));
    assert!(tabs.configure(second, "Closed", Accent::Red, None).is_err());
}
