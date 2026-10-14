#![cfg(unix)]
use alacritty_terminal::grid::Dimensions;
use hoori_term::session::ProcessState;
use hoori_term::tabs::{CloseTarget, Shell, TabId, Tabs};
use hoori_term::terminal::{GridSize, HISTORY_LINES};
use std::thread;
use std::time::{Duration, Instant};

fn shell(program: &str) -> Shell {
    Shell {
        program: program.into(),
        args: if program.ends_with("zsh") {
            vec!["-f".into(), "-i".into()]
        } else {
            vec!["--noprofile".into(), "--norc".into(), "-i".into()]
        },
        directory: None,
    }
}

fn wait(tabs: &mut Tabs, done: impl Fn(&Tabs) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        for tab in &mut tabs.items {
            tab.poll_output();
        }
        if done(tabs) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "synthetic session state did not arrive"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn send(tabs: &mut Tabs, id: TabId, command: &str) {
    tabs.get_mut(id)
        .unwrap()
        .send(format!("{command}\r").into_bytes());
}

fn screen(tabs: &Tabs, id: TabId) -> String {
    tabs.get(id)
        .unwrap()
        .terminal
        .term
        .grid()
        .display_iter()
        .map(|c| c.cell.c)
        .collect()
}

#[test]
fn three_real_sessions_process_inactive_output_and_resize_independently() {
    let mut tabs = Tabs::default();
    let a = tabs
        .open(&shell("/usr/bin/zsh"), GridSize::new(80, 12))
        .unwrap();
    let b = tabs
        .open(&shell("/bin/bash"), GridSize::new(80, 12))
        .unwrap();
    let c = tabs
        .open(&shell("/bin/bash"), GridSize::new(80, 12))
        .unwrap();
    assert_eq!(tabs.active, Some(c));
    for (id, label) in [(a, "ONE"), (b, "TWO"), (c, "THREE")] {
        send(
            &mut tabs,
            id,
            &format!(
                "stty -echo; printf '\\033[2J\\033[H%s_%s\\n' {label} proof; printf '\\033]2;{label}-ready\\007'"
            ),
        );
    }
    wait(&mut tabs, |t| {
        t.items
            .iter()
            .all(|t| t.terminal.shell_title.ends_with("-ready"))
    });
    for (id, label) in [(a, "ONE"), (b, "TWO"), (c, "THREE")] {
        let text = screen(&tabs, id);
        assert!(text.contains(&format!("{label}_proof")));
        for other in ["ONE", "TWO", "THREE"].into_iter().filter(|x| *x != label) {
            assert!(!text.contains(&format!("{other}_proof")));
        }
    }
    tabs.get_mut(a).unwrap().resize(GridSize::new(111, 35));
    for id in [a, b, c] {
        send(
            &mut tabs,
            id,
            "printf '\\033]2;size %s\\007' \"$(stty size)\"",
        );
    }
    wait(&mut tabs, |t| {
        t.items
            .iter()
            .all(|t| t.terminal.shell_title.starts_with("size "))
    });
    assert_eq!(tabs.get(a).unwrap().terminal.shell_title, "size 35 111");
    assert_eq!(tabs.get(b).unwrap().terminal.shell_title, "size 12 80");
    assert_eq!(tabs.get(c).unwrap().terminal.shell_title, "size 12 80");
    // More output than the bounded channel and history, while a is inactive.
    send(
        &mut tabs,
        a,
        "for i in {1..6000}; do printf 'inactive-%04d\\n' $i; done; printf '\\033]2;load-done\\007'",
    );
    send(&mut tabs, c, "printf '\\033]2;active-responsive\\007'");
    wait(&mut tabs, |t| {
        t.get(c).unwrap().terminal.shell_title == "active-responsive"
    });
    wait(&mut tabs, |t| {
        t.get(a).unwrap().terminal.shell_title == "load-done"
    });
    assert_eq!(
        tabs.get(a).unwrap().terminal.term.grid().history_size(),
        HISTORY_LINES
    );
    assert!(screen(&tabs, a).contains("inactive-6000"));
    assert!(!screen(&tabs, c).contains("inactive-6000"));
    tabs.activate(a);
    assert_eq!(tabs.current().unwrap().id, a);
}

#[test]
fn close_targets_are_stable_and_other_sessions_survive_exit_start_failure_and_shutdown() {
    let mut tabs = Tabs::default();
    let a = tabs
        .open(&shell("/bin/bash"), GridSize::new(80, 12))
        .unwrap();
    let b = tabs
        .open(&shell("/bin/bash"), GridSize::new(80, 12))
        .unwrap();
    let failed = tabs
        .open(
            &shell("/nonexistent/hoori-test-shell"),
            GridSize::new(80, 12),
        )
        .unwrap();
    assert!(matches!(
        tabs.get(failed).unwrap().state(),
        ProcessState::Failed(_)
    ));
    assert!(tabs.running_names(CloseTarget::Tab(failed)).is_empty());
    assert_eq!(
        tabs.running_names(CloseTarget::Window),
        ["bash · 1", "bash · 2"]
    );
    // Request/cancel is read-only; only confirmed targets are removed.
    assert_eq!(tabs.running_names(CloseTarget::Tab(a)), ["bash · 1"]);
    assert_eq!(tabs.items.len(), 3);
    for session in tabs.remove(CloseTarget::Tab(a)) {
        session.shutdown();
    }
    assert_eq!(tabs.get(b).unwrap().state(), ProcessState::Running);
    tabs.remove(CloseTarget::Tab(failed));
    assert_eq!(tabs.active, Some(b));
    let next = tabs
        .open(&shell("/bin/bash"), GridSize::new(80, 12))
        .unwrap();
    assert_eq!(next, TabId(4));
    tabs.activate(a);
    assert_eq!(tabs.active, Some(next));
    send(&mut tabs, b, "exit 7");
    wait(&mut tabs, |t| {
        t.get(b).unwrap().state() == ProcessState::Exited(7)
    });
    assert!(tabs.running_names(CloseTarget::Tab(b)).is_empty());
    assert_eq!(tabs.items.len(), 2); // Exited output is retained until close.
    for session in tabs.remove(CloseTarget::Tab(b)) {
        session.shutdown();
    }
    send(&mut tabs, next, "printf '\\033]2;survivor-ready\\007'");
    wait(&mut tabs, |t| {
        t.get(next).unwrap().terminal.shell_title == "survivor-ready"
    });
    assert_eq!(tabs.running_names(CloseTarget::Window), ["bash · 4"]);
    for session in tabs.remove(CloseTarget::Window) {
        session.shutdown();
    }
    assert!(tabs.items.is_empty());
    assert!(tabs.active.is_none());
}

#[test]
fn tab_count_is_bounded_and_closed_capacity_can_be_reused_without_identity_reuse() {
    let mut tabs = Tabs::default();
    for _ in 0..hoori_term::tabs::MAX_TABS {
        tabs.open(
            &shell("/nonexistent/hoori-test-shell"),
            GridSize::new(20, 4),
        )
        .unwrap();
    }
    assert!(
        tabs.open(&shell("/bin/bash"), GridSize::new(20, 4))
            .is_err()
    );
    tabs.remove(CloseTarget::Tab(TabId(1)));
    let id = tabs
        .open(
            &shell("/nonexistent/hoori-test-shell"),
            GridSize::new(20, 4),
        )
        .unwrap();
    assert_eq!(id.0, hoori_term::tabs::MAX_TABS as u64 + 1);
}
