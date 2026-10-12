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

#[test]
fn pin_groups_move_only_the_target_and_normal_close_cannot_remove_it() {
    let mut tabs = Tabs::default();
    let ids: Vec<_> = (0..5).map(|_| open(&mut tabs)).collect();
    let target = ids[2];
    let key = Key::Named(Named::F8);
    tabs.configure(
        target,
        "Backend",
        Accent::Green,
        Some(shortcut(key.clone(), Modifiers::empty())),
    )
    .unwrap();
    tabs.get_mut(target)
        .unwrap()
        .terminal
        .feed(b"retained\x1b]2;still here\x07");
    let identity = tabs.get(target).unwrap().identity.clone();
    let order = |tabs: &Tabs| tabs.items.iter().map(|tab| tab.id).collect::<Vec<_>>();
    tabs.set_pinned(target, true).unwrap();
    tabs.set_pinned(ids[4], true).unwrap();
    assert_eq!(order(&tabs), [target, ids[4], ids[0], ids[1], ids[3]]);
    tabs.set_pinned(target, true).unwrap(); // No change preserves its position.
    assert_eq!(order(&tabs), [target, ids[4], ids[0], ids[1], ids[3]]);
    assert_eq!(tabs.pinned_count(), 2);
    assert_eq!(tabs.active, Some(ids[4]));
    assert_eq!(tabs.get(target).unwrap().identity, identity);
    assert_eq!(tabs.shortcut_target(&key, Modifiers::empty()), Some(target));
    assert_eq!(tabs.get(target).unwrap().terminal.shell_title, "still here");
    assert!(tabs.close_protected(CloseTarget::Tab(target)));
    assert!(!tabs.close_protected(CloseTarget::PinnedTab(target)));
    // Even failed/exited transports have normal close protection.
    tabs.remove(CloseTarget::Tab(target));
    assert!(tabs.get(target).is_some());
    tabs.set_pinned(target, false).unwrap();
    assert_eq!(order(&tabs), [ids[4], ids[0], ids[1], ids[3], target]);
    tabs.remove(CloseTarget::Tab(target));
    assert!(tabs.get(target).is_none());
    tabs.remove(CloseTarget::PinnedTab(ids[4]));
    assert!(tabs.get(ids[4]).is_none());
    assert_eq!(order(&tabs), [ids[0], ids[1], ids[3]]);
    tabs.set_pinned(ids[1], true).unwrap();
    tabs.remove(CloseTarget::Window);
    assert!(tabs.items.is_empty()); // Window close deliberately includes pins.
}

#[test]
fn free_moves_preserve_identity_and_active_tab_and_reject_stale_or_cross_group_targets() {
    let mut tabs = Tabs::default();
    let ids: Vec<_> = (0..12).map(|_| open(&mut tabs)).collect();
    for (index, id) in ids.iter().copied().enumerate() {
        tabs.configure(id, &format!("Tab {index}"), Accent::Green, None)
            .unwrap();
        let tab = tabs.get_mut(id).unwrap();
        tab.identity.compact = index % 2 == 0;
        tab.identity.symbol = Some(hoori_term::symbols::Symbol::Star);
        tab.terminal
            .feed(format!("retained-{index}\x1b]2;title-{index}\x07").as_bytes());
        if index < 4 {
            tabs.set_pinned(id, true).unwrap();
        }
    }
    tabs.get_mut(ids[1]).unwrap().identity.shortcut =
        Some(shortcut(Key::Named(Named::F8), Modifiers::empty()));
    let identities: Vec<_> = ids
        .iter()
        .map(|id| {
            let tab = tabs.get(*id).unwrap();
            (
                *id,
                tab.identity.clone(),
                tab.pinned,
                tab.template,
                tab.terminal.shell_title.clone(),
                tab.state(),
            )
        })
        .collect();
    let order = |tabs: &Tabs| tabs.items.iter().map(|tab| tab.id).collect::<Vec<_>>();
    assert!(tabs.move_tab(ids[3], ids[0], false).unwrap());
    assert!(tabs.move_tab(ids[5], ids[10], true).unwrap());
    assert_eq!(
        order(&tabs),
        [
            ids[3], ids[0], ids[1], ids[2], ids[4], ids[6], ids[7], ids[8], ids[9], ids[10],
            ids[5], ids[11]
        ]
    );
    let before = order(&tabs);
    for (source, target, after) in [
        (ids[3], ids[0], false),
        (ids[3], ids[3], true),
        (ids[5], ids[10], true),
    ] {
        assert!(!tabs.move_tab(source, target, after).unwrap());
        assert_eq!(order(&tabs), before);
    }
    for (source, target) in [(ids[1], ids[5]), (TabId(999), ids[1]), (ids[1], TabId(999))] {
        assert!(tabs.move_tab(source, target, false).is_err());
        assert_eq!(order(&tabs), before);
    }
    assert_eq!(tabs.active, Some(ids[11]));
    assert_eq!(
        tabs.shortcut_target(&Key::Named(Named::F8), Modifiers::empty()),
        Some(ids[1])
    );
    for (id, identity, pinned, template, title, state) in identities {
        let tab = tabs.get(id).unwrap();
        assert_eq!(
            (
                &tab.identity,
                tab.pinned,
                tab.template,
                &tab.terminal.shell_title,
                tab.state()
            ),
            (&identity, pinned, template, &title, state)
        );
    }
    tabs.remove(CloseTarget::Tab(ids[10]));
    let after_close = order(&tabs);
    assert!(tabs.move_tab(ids[5], ids[10], false).is_err());
    assert_eq!(order(&tabs), after_close); // Never falls back to the old positional neighbor.
    tabs.remove(CloseTarget::Tab(ids[5]));
    assert!(tabs.move_tab(ids[5], ids[11], true).is_err());
}
