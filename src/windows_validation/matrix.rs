//! Explicit synthetic Windows scenarios inside a real native iced window.
//! Compiled only with `validation`; never attaches to existing sessions.
use super::*;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::term::TermMode;
use serde::Deserialize;

#[derive(Debug, Clone)]
enum MatrixMessage {
    App(Message),
    Poll,
    OriginalClipboard(Option<String>),
    ClipboardChecked(bool),
}

#[derive(Deserialize)]
struct Step {
    id: u64,
    #[serde(flatten)]
    action: Action,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Action {
    Observe,
    OpenWsl {
        distro: String,
        shell: String,
    },
    OpenPowershell,
    SaveTemplate {
        tab: u64,
    },
    ShowTemplates,
    OpenTemplate {
        template: u64,
    },
    EditTemplate {
        template: u64,
    },
    RemoveTemplate {
        template: u64,
    },
    TemplateTarget {
        shell: TemplateShell,
        distribution: Option<String>,
    },
    Directory {
        directory: String,
    },
    RetryLoad,
    RetrySave,
    DiscardAndExit,
    OpenMissing {
        wsl: bool,
    },
    Inventory {
        fixture: String,
    },
    Choose {
        index: usize,
    },
    Text {
        text: String,
    },
    Key {
        key: String,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        alt: bool,
    },
    Activate {
        tab: u64,
    },
    Edit {
        tab: u64,
    },
    Name {
        name: String,
    },
    Accent {
        accent: Accent,
    },
    CaptureShortcut,
    ApplyEdit,
    CancelEdit,
    Move {
        source: u64,
        target: u64,
        after: bool,
    },
    Select {
        tab: u64,
        row: usize,
        column: usize,
        start: bool,
        right: bool,
    },
    CheckClipboard {
        expected: String,
    },
    RestoreClipboard,
    Resize {
        width: f32,
        height: f32,
    },
    Close {
        tab: Option<u64>,
    },
    Confirm,
    Cancel,
}

struct Matrix {
    app: App,
    root: PathBuf,
    last: u64,
    original_clipboard: Option<String>,
    clipboard_ready: bool,
    clipboard_matches: Option<bool>,
}

pub(super) fn run(root: PathBuf) -> iced::Result {
    iced::application(
        move || {
            let (app, task) = if std::env::var("HOORI_GUI_TEMPLATES").as_deref() == Ok("1") {
                let local = std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .expect("isolated LOCALAPPDATA required");
                assert_eq!(
                    local,
                    root.join("local-app-data"),
                    "never use the user's configuration in this fixture"
                );
                let shell = hoori_term::launch::powershell();
                App::boot(Options {
                    shell: shell.program,
                    args: vec!["-NoLogo".into(), "-NoProfile".into()],
                })
            } else {
                (
                    App::new(Options {
                        shell: String::new(),
                        args: vec![],
                    }),
                    Task::none(),
                )
            };
            (
                Matrix {
                    app,
                    root: root.clone(),
                    last: 0,
                    original_clipboard: None,
                    clipboard_ready: false,
                    clipboard_matches: None,
                },
                Task::batch([
                    task.map(MatrixMessage::App),
                    iced::clipboard::read().map(MatrixMessage::OriginalClipboard),
                ]),
            )
        },
        Matrix::update,
        Matrix::view,
    )
    .title(|matrix: &Matrix| matrix.app.title())
    .subscription(|matrix: &Matrix| {
        Subscription::batch([
            matrix.app.subscription().map(MatrixMessage::App),
            iced::time::every(std::time::Duration::from_millis(100)).map(|_| MatrixMessage::Poll),
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

impl Matrix {
    fn view(&self) -> Element<'_, MatrixMessage> {
        self.app.view().map(MatrixMessage::App)
    }

    fn update(&mut self, message: MatrixMessage) -> Task<MatrixMessage> {
        match message {
            MatrixMessage::App(message) => self.app.update(message).map(MatrixMessage::App),
            MatrixMessage::OriginalClipboard(text) => {
                self.original_clipboard = text;
                self.clipboard_ready = true;
                Task::none()
            }
            MatrixMessage::ClipboardChecked(matches) => {
                self.clipboard_matches = Some(matches);
                Task::none()
            }
            MatrixMessage::Poll => {
                let path = self.root.join("step.json");
                let Ok(bytes) = std::fs::read(path) else {
                    return Task::none();
                };
                assert!(bytes.len() <= 65_536, "fixture step exceeds its bound");
                let Ok(step) = serde_json::from_slice::<Step>(&bytes) else {
                    return Task::none();
                };
                if step.id <= self.last || !self.clipboard_ready {
                    return Task::none();
                }
                self.last = step.id;
                let task = self.advance(step.action);
                self.snapshot();
                task
            }
        }
    }

    fn advance(&mut self, action: Action) -> Task<MatrixMessage> {
        use keyboard::{Key, Modifiers, key::Named};
        let task = match action {
            Action::Observe => Task::none(),
            Action::OpenWsl { distro, shell } => {
                assert!(matches!(distro.as_str(), "Ubuntu-24.04" | "Ubuntu-26.04"));
                assert!(matches!(shell.as_str(), "bash" | "zsh"));
                let mut target = hoori_term::launch::wsl(&distro);
                let executable = target.args.iter().position(|arg| arg == "--exec").unwrap() + 1;
                target.args[executable] = format!("/bin/{shell}");
                if shell == "bash" {
                    target.args.splice(
                        executable + 1..executable + 1,
                        ["--noprofile".into(), "--norc".into()],
                    );
                } else {
                    target.args.insert(executable + 1, "-f".into());
                }
                self.app.update(Message::Open(target))
            }
            Action::OpenPowershell => {
                let mut target = hoori_term::launch::powershell();
                target.args.push("-NoProfile".into());
                self.app.update(Message::Open(target))
            }
            Action::SaveTemplate { tab } => self.app.update(Message::SaveTemplate(TabId(tab))),
            Action::ShowTemplates => self.app.update(Message::ShowTemplates),
            Action::OpenTemplate { template } => {
                self.app.update(Message::OpenTemplate(TemplateId(template)))
            }
            Action::EditTemplate { template } => {
                self.app.update(Message::EditTemplate(TemplateId(template)))
            }
            Action::RemoveTemplate { template } => self
                .app
                .update(Message::RemoveTemplate(TemplateId(template))),
            Action::TemplateTarget {
                shell,
                distribution,
            } => self.app.update(Message::EditTemplateTarget(TemplateTarget {
                shell,
                distribution,
            })),
            Action::Directory { directory } => self.app.update(Message::EditDirectory(directory)),
            Action::RetryLoad => self.app.update(Message::RetryLoad),
            Action::RetrySave => self.app.update(Message::RetrySave),
            Action::DiscardAndExit => self.app.update(Message::DiscardAndExit),
            Action::OpenMissing { wsl } => {
                let target = if wsl {
                    hoori_term::launch::wsl("hoori-nonexistent-test-distribution")
                } else {
                    Shell {
                        program: "C:\\hoori-nonexistent-matrix-shell.exe".into(),
                        args: vec![],
                        directory: None,
                    }
                };
                self.app.update(Message::Open(target))
            }
            Action::Inventory { fixture } => {
                assert!(self.app.tabs.items.is_empty());
                let inventory = match fixture.as_str() {
                    "missing" => {
                        Inventory::unavailable("WSL fehlt (isolierte Testvorgabe).".into())
                    }
                    "empty" => hoori_term::launch::parse("", "").unwrap(),
                    "legacy" => hoori_term::launch::parse(
                        "Legacy\nUbuntu-24.04",
                        "* Legacy Stopped 1\nUbuntu-24.04 Running 2",
                    )
                    .unwrap(),
                    "real" => hoori_term::launch::discover(),
                    _ => panic!("unknown inventory fixture"),
                };
                self.app.update(Message::LaunchesLoaded(inventory, true))
            }
            Action::Choose { index } => self.app.update(Message::ChooseShell(index)),
            Action::Text { text } => {
                self.app
                    .keyboard(Key::Character("x".into()), Modifiers::empty(), Some(text))
            }
            Action::Key {
                key,
                ctrl,
                shift,
                alt,
            } => {
                let key = match key.as_str() {
                    "Enter" => Key::Named(Named::Enter),
                    "Escape" => Key::Named(Named::Escape),
                    "Backspace" => Key::Named(Named::Backspace),
                    "Delete" => Key::Named(Named::Delete),
                    "Left" => Key::Named(Named::ArrowLeft),
                    "Right" => Key::Named(Named::ArrowRight),
                    "Up" => Key::Named(Named::ArrowUp),
                    "Down" => Key::Named(Named::ArrowDown),
                    "Home" => Key::Named(Named::Home),
                    "End" => Key::Named(Named::End),
                    "Tab" => Key::Named(Named::Tab),
                    "PageUp" => Key::Named(Named::PageUp),
                    "PageDown" => Key::Named(Named::PageDown),
                    "F8" => Key::Named(Named::F8),
                    "F9" => Key::Named(Named::F9),
                    "F10" => Key::Named(Named::F10),
                    _ if key.chars().count() == 1 => Key::Character(key.into()),
                    _ => panic!("unknown fixture key"),
                };
                let modifiers = if ctrl {
                    Modifiers::CTRL
                } else {
                    Modifiers::empty()
                } | if shift {
                    Modifiers::SHIFT
                } else {
                    Modifiers::empty()
                } | if alt {
                    Modifiers::ALT
                } else {
                    Modifiers::empty()
                };
                self.app.keyboard(key, modifiers, None)
            }
            Action::Activate { tab } => self.app.update(Message::Activate(TabId(tab))),
            Action::Edit { tab } => self.app.update(Message::Edit(TabId(tab))),
            Action::Name { name } => self.app.update(Message::EditName(name)),
            Action::Accent { accent } => self.app.update(Message::EditAccent(accent)),
            Action::CaptureShortcut => self.app.update(Message::CaptureShortcut),
            Action::ApplyEdit => self.app.update(Message::ApplyEdit),
            Action::CancelEdit => self.app.update(Message::CancelEdit),
            Action::Move {
                source,
                target,
                after,
            } => self.app.update(Message::MoveTab(
                TabId(source),
                tab_strip::DropTarget {
                    tab: TabId(target),
                    after,
                },
            )),
            Action::Select {
                tab,
                row,
                column,
                start,
                right,
            } => self.app.update(Message::Select {
                id: TabId(tab),
                row,
                column,
                start,
                side: if right { Side::Right } else { Side::Left },
            }),
            Action::CheckClipboard { expected } => {
                self.clipboard_matches = None;
                return iced::clipboard::read().map(move |text| {
                    MatrixMessage::ClipboardChecked(text.as_deref() == Some(&expected))
                });
            }
            Action::RestoreClipboard => {
                return iced::clipboard::write(self.original_clipboard.take().unwrap_or_default());
            }
            Action::Resize { width, height } => {
                assert!((640.0..=1600.0).contains(&width) && (360.0..=1000.0).contains(&height));
                window::resize(
                    self.app.chrome.id.expect("native window id"),
                    iced::Size::new(width, height),
                )
            }
            Action::Close { tab } => self.app.update(Message::Close(
                tab.map(|id| CloseTarget::Tab(TabId(id)))
                    .unwrap_or(CloseTarget::Window),
            )),
            Action::Confirm => self.app.update(Message::ConfirmClose),
            Action::Cancel => self.app.update(Message::CancelClose),
        };
        task.map(MatrixMessage::App)
    }

    fn snapshot(&self) {
        let tabs: Vec<_> = self.app.tabs.items.iter().map(|tab| {
            let grid = tab.terminal.term.grid();
            let offset = grid.display_offset();
            let screen: Vec<_> = (0..tab.terminal.size.rows).map(|row| {
                (0..tab.terminal.size.columns).map(|col| grid[Point::new(Line(i32::from(row) - offset as i32), Column(col as usize))].c).collect::<String>()
            }).collect();
            serde_json::json!({ "id": tab.id.0, "name": tab.label(), "original": tab.original_label(),
                "identity": tab.identity, "template": tab.template, "state": format!("{:?}", tab.state()), "error": tab.error,
                "shell": { "program": tab.shell.program, "args": tab.shell.args, "directory": tab.shell.directory }, "title": tab.terminal.shell_title,
                "columns": tab.terminal.size.columns, "rows": tab.terminal.size.rows,
                "history": grid.history_size(), "offset": offset,
                "alternate": tab.terminal.term.mode().contains(TermMode::ALT_SCREEN),
                "cursor": { "row": grid.cursor.point.line.0, "column": grid.cursor.point.column.0 },
                "screen": screen })
        }).collect();
        let snapshot = serde_json::json!({ "step": self.last, "commit": build_info::metadata(),
            "title": self.app.title(), "active": self.app.tabs.active.map(|id| id.0), "tabs": tabs,
            "confirming": self.app.confirming.is_some(), "closing": self.app.closing,
            "pending_shutdowns": self.app.pending_shutdowns, "clipboard_matches": self.clipboard_matches,
            "error": self.app.error, "launching": self.app.launching, "discovering":self.app.discovering,
            "editor_error": self.app.editing.as_ref().and_then(|edit| edit.error.as_deref()),
            "editor": self.app.editing.as_ref().map(|edit| serde_json::json!({"target":format!("{:?}",edit.target),"name":edit.name,"shell":edit.shell,"distribution":edit.wsl_distribution,"directory":edit.directory,"directory_pending":edit.directory_request.is_some()})),
            "catalog": self.app.catalog, "templates_open":self.app.templates_open,
            "storage": {"loaded":self.app.templates_loaded,"loading":self.app.templates_loading,"error":self.app.storage_error,"revision":self.app.revision,"saved_revision":self.app.saved_revision,"saving":self.app.saving},
            "choices": self.app.launches.choices.iter().map(|c| serde_json::json!({"label": c.label, "enabled": c.enabled})).collect::<Vec<_>>() });
        std::fs::write(
            self.root.join(format!("step-{}.json", self.last)),
            serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();
    }
}
