use base64::Engine;
use hoori_term::program_status::{CAPABILITIES, Kind, MAX_RECORDS, ProgramStatus, State};
use hoori_term::terminal::{GridSize, Terminal};

fn encoded(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

fn osc(body: &str) -> Vec<u8> {
    format!("\x1b]7501;{body}\x1b\\").into_bytes()
}

#[test]
fn fragmented_titles_reports_and_queries_preserve_order_and_terminal_rendering() {
    let title = format!("Grüße 界 {}", "part;".repeat(40));
    let mut bytes = b"\x1b[31mA\x1b[0m\x1b]0;old\x07".to_vec();
    bytes.extend(format!("\x1b]2;{title}\x1b\\").as_bytes());
    bytes.extend(osc(&format!(
        "state=working:unknown=future:state=blocked:kind=permission:progress=007:app=cargo:id=build/test:title={}:msg={}",
        encoded("Tests 界"), encoded("Freigabe benötigt")
    )));
    bytes.extend(b"\x1b]7501;?\x07\x1b[cB");
    for chunk in 1..=bytes.len() {
        let mut terminal = Terminal::new(GridSize::new(80, 24));
        let mut replies = Vec::new();
        for block in bytes.chunks(chunk) {
            replies.extend(terminal.feed(block));
        }
        assert_eq!(terminal.shell_title, title);
        let record = terminal.program_status.summary().unwrap();
        assert_eq!(record.id, "build/test");
        assert_eq!(record.state, State::Blocked);
        assert_eq!(record.kind, Some(Kind::Permission));
        assert_eq!(record.progress, Some(7));
        assert_eq!(record.title.as_deref(), Some("Tests 界"));
        assert_eq!(record.message.as_deref(), Some("Freigabe benötigt"));
        assert_eq!(replies[0], CAPABILITIES);
        assert_eq!(replies[1], b"\x1b[?6c");
        let visible: String = terminal
            .term
            .grid()
            .display_iter()
            .map(|c| c.cell.c)
            .collect();
        assert!(visible.starts_with("AB"));
    }
    let mut terminal = Terminal::new(GridSize::new(80, 24));
    terminal.feed(b"\x1b]2;abc\x07\x1b]0;\x1b\\");
    assert!(terminal.shell_title.is_empty());
    terminal.feed(b"\x1b]2;again\x07\x1bc");
    assert!(terminal.shell_title.is_empty());
}

#[test]
fn invalid_or_oversized_reports_are_atomic_and_unfinished_strings_recover() {
    let baseline = b"state=blocked:kind=question:app=fixture:msg=YW5zd2Vy";
    let mut status = ProgramStatus::default();
    status.apply(baseline);
    let expected = status.summary().unwrap().clone();
    let invalid = [
        "state=future".into(),
        "app=other".into(),
        "state=error:id=".into(),
        "state=error:id=build//test".into(),
        "state=error:id=/root".into(),
        "state=error:id=bad$".into(),
        "state=error:id=Grüße".into(),
        format!("state=error:id={}", "a/".repeat(8) + "a"),
        format!("state=error:id={}", "x".repeat(129)),
        format!("state=error:app={}", "a".repeat(33)),
        format!("state=error:{}=ok", "a".repeat(17)),
        "state=error:msg=a".into(),
        "state=clear:msg=//8=".into(),
        format!("state=error:msg={}", encoded("line\nnext")),
        format!("state=error:title={}", encoded("safe\u{85}")),
        format!("state=error:msg={}", encoded(&"x".repeat(2049))),
        format!("state=error:title={}", encoded(&"x".repeat(193))),
        format!("state=error:msg={}:msg=b2s=", "x".repeat(2733)),
    ];
    for body in invalid {
        status.apply(body.as_bytes());
        assert_eq!(
            status.summary(),
            Some(&expected),
            "invalid report was applied: {body:?}"
        );
    }
    let mut terminal = Terminal::new(GridSize::new(80, 24));
    terminal.feed(&osc(std::str::from_utf8(baseline).unwrap()));
    terminal.feed(&osc(&format!("state=error:future={}", "x".repeat(100_000))));
    assert_eq!(terminal.program_status.summary(), Some(&expected));
    terminal.feed(b"\x1b]7501;state=error");
    terminal.feed(b"\x18\x1b]7501;state=working\x1b");
    terminal.feed(b"]7501;state=done\x07");
    assert_eq!(
        terminal.program_status.summary().unwrap().state,
        State::Done
    );
    terminal.feed(b"\x1b]2;safe\x07");
    terminal.feed(format!("\x1b]2;{}\x1b\\", "x".repeat(100_000)).as_bytes());
    assert_eq!(terminal.shell_title, "safe");
    terminal.feed(b"\x1b\x07]2;after\x1b\\");
    assert_eq!(terminal.shell_title, "after");
    assert!(
        terminal
            .feed(b"\x1b]52;c;?\x07\x1b]52;c;b3duZWQ=\x07")
            .is_empty()
    );
}

#[test]
fn replacement_hierarchy_eviction_and_summary_are_deterministic() {
    let mut status = ProgramStatus::default();
    status.apply(b"state=idle:app=root");
    status.apply(b"state=working:id=build:app=cargo");
    status.apply(b"state=blocked:id=build/test:kind=auth:progress=55");
    status.apply(b"state=error:id=build2:msg=ZmFpbGVk");
    assert_eq!(
        status.inherited_app(status.records().find(|r| r.id == "build/test").unwrap()),
        Some("cargo")
    );
    assert_eq!(status.attention_counts(), (1, 1));
    assert_eq!(status.summary().unwrap().id, "build2");
    assert!(status.summary_label().contains("1 warten, 1 Fehler"));
    status.apply(b"state=clear:id=build");
    assert_eq!(
        status.records().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["", "build2"]
    );
    status.apply(b"state=working:id=build2:badpair:progress=101:kind=permission:app=bad/path");
    let record = status.summary().unwrap();
    assert_eq!(record.state, State::Working);
    assert_eq!(record.progress, None);
    assert_eq!(record.kind, None);
    assert_eq!(record.message, None);
    assert_eq!(record.app, None);
    assert_eq!(status.inherited_app(record), Some("root"));
    status.apply(b"state=clear");
    for index in 0..MAX_RECORDS {
        status.apply(format!("id=r{index}:state=idle").as_bytes());
    }
    status.apply(b"id=r0:state=working");
    status.apply(b"id=new:state=blocked");
    assert_eq!(status.records().count(), MAX_RECORDS);
    assert!(status.records().any(|r| r.id == "r0"));
    assert!(!status.records().any(|r| r.id == "r1"));
    status.apply(b"id=newer:state=blocked");
    assert_eq!(status.summary().unwrap().id, "newer");
    status.apply(
        format!(
            "id=newer:state=blocked:msg={}",
            encoded("<b>plain</b>\u{202e}\u{200b} ✓")
        )
        .as_bytes(),
    );
    assert_eq!(
        status.summary().unwrap().message.as_deref(),
        Some("<b>plain</b> ✓")
    );
    status.apply(b"state=clear");
    assert_eq!(status.records().count(), 0);
}

#[test]
fn prompt_exit_screens_and_reset_follow_record_lifetimes_without_time_heuristics() {
    let mut terminal = Terminal::new(GridSize::new(80, 24));
    for (id, state) in [
        ("i", "idle"),
        ("w", "working"),
        ("b", "blocked"),
        ("d", "done"),
        ("e", "error"),
    ] {
        terminal.feed(&osc(&format!("id={id}:state={state}")));
    }
    terminal.feed(b"\x1b[?1049h\x1b[!p\x1b[?1049l");
    assert_eq!(terminal.program_status.records().count(), 5);
    terminal.feed(b"\x1b]133;A;fixture=1\x1b\\");
    assert_eq!(
        terminal
            .program_status
            .records()
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>(),
        ["d", "e"]
    );
    terminal.feed(&osc("state=blocked"));
    assert!(terminal.process_ended());
    assert_eq!(terminal.program_status.records().count(), 2);
    terminal.feed(b"\x1bc");
    assert!(terminal.program_status.summary().is_none());
    // Floods coalesce display events, but the last legal title/status wins.
    let mut bytes = Vec::new();
    for index in 0..1000 {
        bytes.extend(format!("\x1b]2;title-{index}\x07").as_bytes());
        bytes.extend(osc(&format!("state=working:progress={}", index % 101)));
        bytes.extend(b"\x1b]7501;?\x07");
    }
    assert_eq!(terminal.feed(&bytes).len(), 64);
    assert_eq!(terminal.shell_title, "title-999");
    assert_eq!(
        terminal.program_status.summary().unwrap().progress,
        Some((999_u16 % 101) as u8)
    );
}

#[test]
#[cfg(unix)]
fn real_pty_capability_replies_return_to_their_source_and_exit_preserves_results() {
    use hoori_term::session::ProcessState;
    use hoori_term::tabs::{Accent, CloseTarget, Shell, Tabs};
    use std::time::{Duration, Instant};

    // These two independent programs actually read the capability/DA reply
    // from their own raw PTY before reporting a status. A wrong active-tab
    // route or reply order prevents the success title from ever appearing.
    let script = r#"
import os,sys,tty,select,time,base64
tty.setraw(0)
label=sys.argv[1]
expected=b'\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\\x1b[?6c'
os.write(1,b'\x1b]7501;?\x07\x1b[c')
reply=b''
deadline=time.monotonic()+5
while len(reply)<len(expected) and time.monotonic()<deadline:
    if select.select([0],[],[],0.1)[0]: reply+=os.read(0,4096)
if reply!=expected: sys.exit(90)
os.write(1,(' done '+label+'\r\n\x1b]7501;state=blocked:kind=question:app=fixture:msg='+base64.b64encode(label.encode()).decode()+'\x1b\\\x1b]2;'+label+'-query-ok\x07').encode())
if os.read(0,1)!=b'x': sys.exit(91)
os.write(1,b'\x1b]7501;state=working:id=transient\x07\x1b]7501;state=done:id=result\x07\x1b]7501;state=error:id=failure\x07')
sys.exit(7)
"#;
    let shell = |label: &str| Shell {
        program: "/usr/bin/python3".into(),
        args: vec!["-c".into(), script.into(), label.into()],
        directory: None,
    };
    let mut tabs = Tabs::default();
    let first = tabs.open(&shell("first"), GridSize::new(80, 24)).unwrap();
    let second = tabs.open(&shell("second"), GridSize::new(80, 24)).unwrap();
    tabs.configure(first, "Backend", Accent::Green, None)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        for tab in &mut tabs.items {
            tab.poll_output();
        }
        if tabs
            .items
            .iter()
            .all(|t| t.terminal.shell_title.ends_with("-query-ok"))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(tabs.active, Some(second));
    assert_eq!(
        tabs.get(first).unwrap().terminal.shell_title,
        "first-query-ok"
    );
    assert_eq!(
        tabs.get(second).unwrap().terminal.shell_title,
        "second-query-ok"
    );
    for (id, label) in [(first, "first"), (second, "second")] {
        let tab = tabs.get(id).unwrap();
        assert_eq!(
            tab.terminal
                .program_status
                .summary()
                .unwrap()
                .message
                .as_deref(),
            Some(label)
        );
    }
    tabs.get_mut(first).unwrap().send(b"x".to_vec());
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        for tab in &mut tabs.items {
            tab.poll_output();
        }
        if tabs.get(first).unwrap().state() == ProcessState::Exited(7) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Also drain a trailing chunk after process state first becomes observable.
    for _ in 0..3 {
        tabs.get_mut(first).unwrap().poll_output();
    }
    assert_eq!(tabs.get(first).unwrap().state(), ProcessState::Exited(7));
    assert_eq!(tabs.get(first).unwrap().label(), "Backend");
    let states: Vec<_> = tabs
        .get(first)
        .unwrap()
        .terminal
        .program_status
        .records()
        .map(|r| r.state)
        .collect();
    assert_eq!(states, [State::Done, State::Error]);
    assert_eq!(tabs.get(second).unwrap().state(), ProcessState::Running);
    assert_eq!(
        tabs.get(second)
            .unwrap()
            .terminal
            .program_status
            .summary()
            .unwrap()
            .state,
        State::Blocked
    );
    for session in tabs.remove(CloseTarget::Window) {
        session.shutdown();
    }
}
