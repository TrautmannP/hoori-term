//! Opt-in native rendering fixture. Drives our own model, never OS input.
//! Not included in normal builds. Only newly created synthetic sessions are used.
use super::*;
use std::path::PathBuf;

mod matrix;

#[derive(Debug, Clone)]
enum FixtureMessage {
    App(Message),
    Poll,
}

struct Fixture {
    app: App,
    root: PathBuf,
    stage: String,
    distro: String,
}

pub fn run(root: PathBuf) -> iced::Result {
    struct GraphicsLog;
    impl log::Log for GraphicsLog {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Info && metadata.target().starts_with("iced_wgpu")
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("{}", record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: GraphicsLog = GraphicsLog;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    assert!(root.is_dir(), "create a fresh evidence directory first");
    if std::env::var("HOORI_GUI_MATRIX").as_deref() == Ok("1") {
        return matrix::run(root);
    }
    let distro =
        std::env::var("HOORI_TEST_WSL_DISTRO").expect("explicit fixture distribution required");
    iced::application(
        move || {
            let (app, task) = App::boot(Options {
                shell: String::new(),
                args: vec![],
            });
            (
                Fixture {
                    app,
                    root: root.clone(),
                    stage: String::new(),
                    distro: distro.clone(),
                },
                task.map(FixtureMessage::App),
            )
        },
        Fixture::update,
        Fixture::view,
    )
    .title(|fixture: &Fixture| fixture.app.title())
    .subscription(|fixture: &Fixture| {
        Subscription::batch([
            fixture.app.subscription().map(FixtureMessage::App),
            iced::time::every(std::time::Duration::from_millis(250)).map(|_| FixtureMessage::Poll),
        ])
    })
    .theme(Theme::TokyoNight)
    .window(window::Settings {
        size: iced::Size::new(1000.0, 700.0),
        decorations: false,
        exit_on_close_request: false,
        ..Default::default()
    })
    .run()
}

impl Fixture {
    fn view(&self) -> Element<'_, FixtureMessage> {
        self.app.view().map(FixtureMessage::App)
    }

    fn update(&mut self, message: FixtureMessage) -> Task<FixtureMessage> {
        match message {
            FixtureMessage::App(message) => self.app.update(message).map(FixtureMessage::App),
            FixtureMessage::Poll => {
                if self.app.discovering {
                    return Task::none();
                }
                let stage = std::fs::read_to_string(self.root.join("stage.txt"))
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if stage.is_empty() || stage == self.stage {
                    return Task::none();
                }
                if matches!(stage.as_str(), "missing" | "wsl")
                    && self
                        .app
                        .tabs
                        .items
                        .iter()
                        .any(|t| t.state() == ProcessState::Running)
                {
                    return Task::none();
                }
                self.stage = stage;
                let task = self.advance();
                let summary = serde_json::json!({
                    "stage": self.stage, "commit": build_info::metadata(), "title": self.app.title(),
                    "confirmation": self.app.confirming.is_some(),
                    "tabs": self.app.tabs.items.iter().map(|tab| serde_json::json!({
                        "id": tab.id.0, "name": tab.label(), "state": format!("{:?}", tab.state()),
                        "columns": tab.terminal.size.columns, "rows": tab.terminal.size.rows,
                    })).collect::<Vec<_>>()
                });
                std::fs::write(
                    self.root.join(format!("{}.json", self.stage)),
                    serde_json::to_vec_pretty(&summary).unwrap(),
                )
                .unwrap();
                task.map(FixtureMessage::App)
            }
        }
    }

    fn command(&mut self, command: &str) {
        let _ = self.app.keyboard(
            keyboard::Key::Character("x".into()),
            keyboard::Modifiers::empty(),
            Some(command.into()),
        );
        let _ = self.app.keyboard(
            keyboard::Key::Named(keyboard::key::Named::Enter),
            keyboard::Modifiers::empty(),
            None,
        );
    }

    fn advance(&mut self) -> Task<Message> {
        use keyboard::{Key, Modifiers, key::Named};
        match self.stage.as_str() {
            "inventory" | "inspect" => {}
            "menu" => self.app.launching = Some(0),
            "missing" => {
                assert!(
                    self.app
                        .tabs
                        .items
                        .iter()
                        .all(|t| t.state() != ProcessState::Running)
                );
                let chrome = std::mem::take(&mut self.app.chrome);
                self.app = App::new(Options {
                    shell: String::new(),
                    args: vec![],
                });
                self.app.chrome = chrome;
                self.app.launches = Inventory::unavailable(
                    "WSL nicht verfügbar (isolierte Testvorgabe). PowerShell bewusst auswählen."
                        .into(),
                );
                self.app.launches.choices[0]
                    .shell
                    .args
                    .push("-NoProfile".into());
                self.app.launching = Some(0);
                assert!(self.app.tabs.items.is_empty());
            }
            "powershell" => {
                let _ = self.app.update(Message::ChooseShell(0));
                assert_eq!(self.app.tabs.items.len(), 1);
                self.command("Write-Output ('Windows PowerShell ' + $PSVersionTable.PSVersion); Write-Output 'Grüße 界'; Write-Output ('BUILD_FIXTURE:' + 'ready')");
            }
            "powershell_exit" => self.command("exit 7"),
            "wsl" => {
                assert!(
                    self.app
                        .tabs
                        .items
                        .iter()
                        .all(|t| t.state() != ProcessState::Running)
                );
                let mut shell = hoori_term::launch::wsl(&self.distro);
                let index = shell.args.len() - 1;
                shell
                    .args
                    .splice(index..index, ["--noprofile".into(), "--norc".into()]);
                let chrome = std::mem::take(&mut self.app.chrome);
                self.app = App::new(Options {
                    shell: shell.program.clone(),
                    args: shell.args.clone(),
                });
                self.app.chrome = chrome;
                self.app.launches = hoori_term::launch::discover();
                for _ in 0..2 {
                    let _ = self.app.update(Message::Open(shell.clone()));
                }
                let ids: Vec<_> = self.app.tabs.items.iter().map(|t| t.id).collect();
                for (i, id) in ids.into_iter().enumerate() {
                    let _ = self.app.update(Message::Edit(id));
                    let _ = self.app.update(Message::EditName(format!("WSL {}", i + 1)));
                    let _ = self.app.update(Message::EditAccent(
                        [Accent::Cyan, Accent::Green, Accent::Purple][i],
                    ));
                    let _ = self.app.update(Message::CaptureShortcut);
                    let _ = self.app.keyboard(
                        Key::Named([Named::F8, Named::F9, Named::F10][i]),
                        Modifiers::empty(),
                        None,
                    );
                    let _ = self.app.update(Message::ApplyEdit);
                    let _ = self.app.update(Message::Activate(id));
                    self.command(&format!("printf '\\033]2;CLI title {}\\007\\033[31mRED\\033[32m GREEN\\033[34m BLUE\\033[0m\\nGrüße 界 · WSL {}\\n'; printf 'SHELL_PID:%s\\n' $$", i + 1, i + 1));
                }
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F8), Modifiers::empty(), None);
                self.command(
                    "for i in 1 2 3 4 5 6; do printf 'BACKGROUND:%s\\n' $i; sleep 1; done",
                );
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F9), Modifiers::empty(), None);
                assert_eq!(self.app.tabs.active, Some(self.app.tabs.items[1].id));
            }
            "show1" => {
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F8), Modifiers::empty(), None);
            }
            "show2" => {
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F9), Modifiers::empty(), None);
            }
            "show3" => {
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F10), Modifiers::empty(), None);
            }
            "vim" => {
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::F8), Modifiers::empty(), None);
                self.command(&format!(
                    "vim -Nu NONE -n /tmp/hoori-11-vim-{}.txt",
                    std::process::id()
                ));
            }
            "vimtext" => {
                let _ = self.app.keyboard(
                    Key::Character("i".into()),
                    Modifiers::empty(),
                    Some("iGrüße 界".into()),
                );
            }
            "resize" => {
                return window::resize(
                    self.app.chrome.id.expect("native window id"),
                    iced::Size::new(1120.0, 780.0),
                );
            }
            "savevim" => {
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::Escape), Modifiers::empty(), None);
                self.command(":wq");
            }
            "size" => self.command("stty size; printf 'AFTER_VIM:%s\\n' ready"),
            "close" => {
                let id = self.app.tabs.active.unwrap();
                let _ = self.app.update(Message::Close(CloseTarget::Tab(id)));
                assert!(self.app.confirming.is_some());
            }
            "cancel" => {
                let before = self.app.tabs.items.len();
                let _ = self
                    .app
                    .keyboard(Key::Named(Named::Escape), Modifiers::empty(), None);
                assert!(self.app.confirming.is_none());
                assert_eq!(self.app.tabs.items.len(), before);
            }
            "confirm" => return self.app.update(Message::ConfirmClose),
            "exit7" => self.command("exit 7"),
            "close_exited" => {
                let tab = self.app.tabs.current().unwrap();
                assert_eq!(tab.state(), ProcessState::Exited(7));
                return self.app.update(Message::Close(CloseTarget::Tab(tab.id)));
            }
            "window_close" => return self.app.update(Message::Close(CloseTarget::Window)),
            "missing_target" => {
                return self.app.update(Message::Open(hoori_term::launch::wsl(
                    "hoori-nonexistent-test-distribution",
                )));
            }
            "finish" => {
                let task = self.app.update(Message::Close(CloseTarget::Window));
                return if self.app.confirming.is_some() {
                    self.app.update(Message::ConfirmClose)
                } else {
                    task
                };
            }
            other => panic!("unknown fixture stage {other}"),
        }
        Task::none()
    }
}
