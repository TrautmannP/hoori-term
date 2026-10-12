mod view;

use hoori_term::build_info;
use hoori_term::input;
use hoori_term::notifications::Notifications;
use hoori_term::session::ProcessState;
use hoori_term::terminal::GridSize;
use iced::futures::StreamExt;
use iced::widget::{button, canvas, column, container, row, space, text};
use iced::{Element, Event, Fill, Subscription, Task, Theme, keyboard, window};

#[derive(Clone)]
struct Options {
    shell: String,
    args: Vec<String>,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut args = std::env::args().skip(1);
        let mut options = Self {
            shell: "/usr/bin/zsh".into(),
            args: vec!["-i".into()],
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--version" => {
                    println!("{}", build_info::metadata());
                    return Ok(None);
                }
                "--help" => {
                    println!(
                        "hoori-term [--shell /path/to/shell] [-- shell-arguments]\nDefault: /usr/bin/zsh -i\n--version prints embedded build identity."
                    );
                    return Ok(None);
                }
                "--shell" => options.shell = args.next().ok_or("--shell needs a path")?,
                "--" => {
                    options.args = args.collect();
                    break;
                }
                _ => return Err(format!("Unknown argument: {arg}")),
            }
        }
        Ok(Some(options))
    }
}

fn main() -> iced::Result {
    let options = match Options::parse() {
        Ok(Some(options)) => options,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    iced::application(move || App::new(options.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(Theme::TokyoNight)
        .window_size((1000.0, 700.0))
        .exit_on_close_request(false)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    Tick(hoori_term::tabs::TabId),
    Event(Event),
    Resize(hoori_term::tabs::TabId, GridSize),
    Open(hoori_term::tabs::Shell),
    Activate(hoori_term::tabs::TabId),
    Close(hoori_term::tabs::CloseTarget),
    Select {
        id: hoori_term::tabs::TabId,
        row: usize,
        column: usize,
        side: alacritty_terminal::index::Side,
        start: bool,
    },
    Scroll(hoori_term::tabs::TabId, i32),
    Live,
    Copy,
    Paste,
    Pasted(hoori_term::tabs::TabId, Option<String>),
    CancelClose,
    ConfirmClose,
    ShutdownComplete,
}

use hoori_term::tabs::{CloseTarget, Shell, TabId, Tabs};

struct App {
    tabs: Tabs,
    default_shell: Shell,
    error: Option<String>,
    confirming: Option<CloseTarget>,
    closing: bool,
    pending_shutdowns: usize,
    cache: canvas::Cache,
}

impl App {
    fn new(options: Options) -> Self {
        let default_shell = Shell {
            program: options.shell,
            args: options.args,
        };
        let mut tabs = Tabs::default();
        tabs.open(&default_shell, GridSize::new(98, 30)).unwrap();
        Self {
            tabs,
            default_shell,
            error: None,
            confirming: None,
            closing: false,
            pending_shutdowns: 0,
            cache: canvas::Cache::new(),
        }
    }

    fn title(&self) -> String {
        let Some(tab) = self.tabs.current() else {
            return build_info::title("Wird beendet …");
        };
        let title = if tab.terminal.shell_title.is_empty() {
            tab.label.clone()
        } else {
            format!("{} — {}", tab.label, tab.terminal.shell_title)
        };
        build_info::title(&if self.confirming.is_some() {
            format!("Schließen bestätigen — {title}")
        } else {
            title
        })
    }

    fn subscription(&self) -> Subscription<Message> {
        fn output(
            source: &(TabId, Notifications),
        ) -> impl iced::futures::Stream<Item = Message> + Send + 'static + use<> {
            let id = source.0;
            source.1.stream().map(move |_| Message::Tick(id))
        }
        let mut subscriptions = vec![iced::event::listen().map(Message::Event)];
        for tab in &self.tabs.items {
            if let Some(session) = &tab.session {
                subscriptions.push(Subscription::run_with(
                    (tab.id, session.notifications.clone()),
                    output,
                ));
            }
        }
        Subscription::batch(subscriptions)
    }

    fn request_close(&mut self, target: CloseTarget) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        if self.tabs.running_names(target).is_empty() {
            self.close(target)
        } else {
            self.confirming = Some(target);
            Task::none()
        }
    }

    fn close(&mut self, target: CloseTarget) -> Task<Message> {
        self.confirming = None;
        let sessions = self.tabs.remove(target);
        self.cache.clear();
        self.closing = self.tabs.items.is_empty();
        if sessions.is_empty() {
            return if self.closing && self.pending_shutdowns == 0 {
                iced::exit()
            } else {
                Task::none()
            };
        }
        self.pending_shutdowns += 1;
        Task::perform(
            async move {
                let _ = tokio::task::spawn_blocking(move || {
                    for session in sessions {
                        session.shutdown();
                    }
                })
                .await;
            },
            |_| Message::ShutdownComplete,
        )
    }

    fn keyboard(
        &mut self,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
        text: Option<String>,
    ) -> Task<Message> {
        use keyboard::{Key, key::Named};
        if self.confirming.is_some() {
            return match key {
                Key::Named(Named::Escape) => self.update(Message::CancelClose),
                Key::Named(Named::Enter) => self.update(Message::ConfirmClose),
                _ => Task::none(),
            };
        }
        if self.closing {
            return Task::none();
        }
        if (modifiers == keyboard::Modifiers::CTRL
            || modifiers == keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT)
            && key == Key::Named(Named::Tab)
        {
            self.tabs.cycle(!modifiers.shift());
            self.cache.clear();
            return Task::none();
        }
        if modifiers == keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT {
            match key.as_ref() {
                Key::Character(c) if c.eq_ignore_ascii_case("t") => {
                    return self.update(Message::Open(self.default_shell.clone()));
                }
                Key::Character(c) if c.eq_ignore_ascii_case("b") => {
                    return self.update(Message::Open(Shell::bash()));
                }
                Key::Character(c) if c.eq_ignore_ascii_case("w") => {
                    if let Some(id) = self.tabs.active {
                        return self.request_close(CloseTarget::Tab(id));
                    }
                }
                Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                    return self.update(Message::Copy);
                }
                Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                    return self.update(Message::Paste);
                }
                Key::Named(Named::Home) => {
                    if let Some(tab) = self.tabs.current_mut() {
                        tab.terminal.scroll_to_top();
                    }
                    self.cache.clear();
                    return Task::none();
                }
                Key::Named(Named::End) => return self.update(Message::Live),
                _ => {}
            }
        }
        if modifiers == keyboard::Modifiers::SHIFT
            && matches!(key, Key::Named(Named::PageUp | Named::PageDown))
        {
            if let Some(tab) = self.tabs.current_mut() {
                let lines = i32::from(tab.terminal.size.rows).saturating_sub(1).max(1);
                tab.terminal.scroll(if key == Key::Named(Named::PageUp) {
                    lines
                } else {
                    -lines
                });
            }
            self.cache.clear();
            return Task::none();
        }
        if let Some(tab) = self.tabs.current_mut()
            && let Some(bytes) =
                input::encode(&key, modifiers, text.as_deref(), *tab.terminal.term.mode())
        {
            tab.terminal.scroll_to_bottom();
            tab.terminal.term.selection = None;
            tab.send(bytes);
            self.cache.clear();
        }
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // Dialogs freeze commands and selection, but transport/status updates
        // continue for every tab. No queued paste can bypass a close dialog.
        if (self.confirming.is_some() || self.closing)
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::Event(_)
                    | Message::CancelClose
                    | Message::ConfirmClose
                    | Message::ShutdownComplete
            )
        {
            return Task::none();
        }
        match message {
            Message::Tick(id) => {
                if let Some(tab) = self.tabs.get_mut(id)
                    && tab.poll_output()
                    && self.tabs.active == Some(id)
                {
                    self.cache.clear();
                }
            }
            Message::Resize(id, size) => {
                if self.tabs.active == Some(id)
                    && let Some(tab) = self.tabs.get_mut(id)
                {
                    tab.resize(size);
                    self.cache.clear();
                }
            }
            Message::Open(shell) => {
                let size = self
                    .tabs
                    .current()
                    .map(|t| t.terminal.size)
                    .unwrap_or(GridSize::new(98, 30));
                self.error = self.tabs.open(&shell, size).err();
                self.cache.clear();
            }
            Message::Activate(id) => {
                self.tabs.activate(id);
                self.cache.clear();
            }
            Message::Close(target) => return self.request_close(target),
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.request_close(CloseTarget::Window);
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            })) => return self.keyboard(key, modifiers, text.map(|t| t.to_string())),
            Message::Select {
                id,
                row,
                column,
                side,
                start,
            } => {
                if self.tabs.active == Some(id)
                    && let Some(tab) = self.tabs.get_mut(id)
                {
                    tab.terminal.select(row, column, side, start);
                    self.cache.clear();
                }
            }
            Message::Scroll(id, lines) => {
                if self.tabs.active == Some(id)
                    && let Some(tab) = self.tabs.get_mut(id)
                {
                    tab.terminal.scroll(lines);
                    self.cache.clear();
                }
            }
            Message::Live => {
                if let Some(tab) = self.tabs.current_mut() {
                    tab.terminal.scroll_to_bottom();
                    self.cache.clear();
                }
            }
            Message::Copy => {
                if let Some(text) = self
                    .tabs
                    .current()
                    .and_then(|t| t.terminal.term.selection_to_string())
                    .filter(|s| !s.is_empty())
                {
                    return iced::clipboard::write(text);
                }
            }
            Message::Paste => {
                if let Some(tab) = self
                    .tabs
                    .current()
                    .filter(|t| t.state() == ProcessState::Running)
                {
                    let id = tab.id;
                    return iced::clipboard::read().map(move |text| Message::Pasted(id, text));
                }
            }
            Message::Pasted(id, text) => {
                if self.tabs.active == Some(id)
                    && let Some(text) = text
                    && let Some(tab) = self.tabs.get_mut(id)
                {
                    match input::paste(&text, *tab.terminal.term.mode()) {
                        Ok(bytes) => {
                            tab.terminal.scroll_to_bottom();
                            tab.terminal.term.selection = None;
                            tab.send(bytes);
                            self.cache.clear();
                        }
                        Err(error) => tab.error = Some(error),
                    }
                }
            }
            Message::CancelClose => self.confirming = None,
            Message::ConfirmClose => {
                if let Some(target) = self.confirming {
                    return self.close(target);
                }
            }
            Message::ShutdownComplete => {
                self.pending_shutdowns = self.pending_shutdowns.saturating_sub(1);
                if self.closing && self.pending_shutdowns == 0 {
                    return iced::exit();
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        use iced::widget::scrollable;
        let mut tab_row = row![].spacing(5).align_y(iced::Center);
        for tab in &self.tabs.items {
            let marker = match tab.state() {
                ProcessState::Running => "",
                ProcessState::Exited(_) => " · beendet",
                ProcessState::Failed(_) => " · Fehler",
            };
            let active = self.tabs.active == Some(tab.id);
            tab_row = tab_row.push(
                row![
                    button(text(format!("{}{marker}", tab.label)).size(14))
                        .on_press(Message::Activate(tab.id))
                        .style(if active {
                            button::primary
                        } else {
                            button::secondary
                        }),
                    button("×")
                        .on_press(Message::Close(CloseTarget::Tab(tab.id)))
                        .style(button::text)
                ]
                .spacing(1),
            );
        }
        let header = row![
            scrollable(tab_row)
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::new()
                ))
                .width(Fill),
            button("+ zsh").on_press(Message::Open(Shell::zsh())),
            button("+ Bash").on_press(Message::Open(Shell::bash()))
        ]
        .spacing(8)
        .align_y(iced::Center);
        let mut content = column![container(header).padding(8).height(52)];
        if let Some(target) = self.confirming {
            let names = self.tabs.running_names(target);
            let title = if names.is_empty() {
                "Sitzungen sind inzwischen beendet. Schließen?".into()
            } else {
                format!("Laufende Sitzungen beenden: {}?", names.join(", "))
            };
            content = content.push(
                container(
                    row![
                        text(title).size(14).width(Fill),
                        button("Abbrechen").on_press(Message::CancelClose),
                        button("Beenden")
                            .on_press(Message::ConfirmClose)
                            .style(button::danger)
                    ]
                    .spacing(12)
                    .align_y(iced::Center),
                )
                .padding(10),
            );
        }
        if let Some(tab) = self.tabs.current() {
            content = content.push(
                canvas(view::TerminalView {
                    id: tab.id,
                    terminal: &tab.terminal,
                    cache: &self.cache,
                    interactive: self.confirming.is_none() && !self.closing,
                })
                .width(Fill)
                .height(Fill),
            );
            if let Some(error) = tab.error.as_ref().or(self.error.as_ref()) {
                content = content.push(container(text(error).size(14)).padding(8));
            }
            let status = match tab.state() {
                ProcessState::Running => "läuft".into(),
                ProcessState::Exited(code) => format!("beendet · Exit {code}"),
                ProcessState::Failed(_) => "Startfehler".into(),
            };
            let offset = tab.terminal.term.grid().display_offset();
            content = content.push(
                container(
                    row![
                        text(format!(
                            "{} · {status} · {} × {} · Verlauf {offset}",
                            tab.label, tab.terminal.size.columns, tab.terminal.size.rows
                        ))
                        .size(12),
                        space::horizontal(),
                        button(text("Kopieren").size(12)).on_press(Message::Copy),
                        button(text("Einfügen").size(12)).on_press(Message::Paste),
                        button(text("↓ Live").size(12)).on_press(Message::Live),
                    ]
                    .spacing(8)
                    .align_y(iced::Center),
                )
                .padding(8)
                .height(48),
            );
        } else {
            content = content.push(
                container(text("Sitzungen werden beendet …"))
                    .padding(16)
                    .height(Fill),
            );
        }
        container(content).width(Fill).height(Fill).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::Side;
    use iced::keyboard::{Key, Modifiers};

    fn app() -> App {
        App::new(Options {
            shell: "/bin/bash".into(),
            args: vec!["--noprofile".into(), "--norc".into(), "-i".into()],
        })
    }

    #[test]
    fn cancel_close_preserves_live_sessions_and_dialog_blocks_actions() {
        let mut app = app();
        let original = app.tabs.active.unwrap();
        let _ = app.update(Message::Open(Shell::bash()));
        let _ = app.request_close(CloseTarget::Window);
        assert_eq!(app.confirming, Some(CloseTarget::Window));
        assert_eq!(app.tabs.running_names(CloseTarget::Window).len(), 2);
        let _ = app.update(Message::Activate(original));
        let _ = app.update(Message::Open(Shell::bash()));
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            Some("T".into()),
        );
        assert_eq!(app.tabs.items.len(), 2);
        assert_ne!(app.tabs.active, Some(original));
        let _ = app.update(Message::Pasted(
            app.tabs.active.unwrap(),
            Some("unexpected".into()),
        ));
        let _ = app.update(Message::CancelClose);
        assert!(app.confirming.is_none());
        assert!(
            app.tabs
                .items
                .iter()
                .all(|t| t.state() == ProcessState::Running)
        );
        let _ = app.update(Message::Activate(original));
        assert_eq!(app.tabs.active, Some(original));
    }

    #[test]
    fn delayed_paste_and_resize_do_not_follow_a_tab_switch() {
        let mut app = app();
        let original = app.tabs.active.unwrap();
        let original_size = app.tabs.current().unwrap().terminal.size;
        let _ = app.update(Message::Open(Shell::bash()));
        let current = app.tabs.active.unwrap();
        let _ = app.update(Message::Pasted(original, Some("x".repeat(100_000))));
        assert!(app.tabs.get(original).unwrap().error.is_none());
        assert!(app.tabs.get(current).unwrap().error.is_none());
        let _ = app.update(Message::Resize(original, GridSize::new(200, 100)));
        assert_eq!(app.tabs.get(original).unwrap().terminal.size, original_size);
        assert_eq!(app.tabs.get(current).unwrap().terminal.size, original_size);
        let _ = app.update(Message::Resize(current, GridSize::new(111, 35)));
        assert_eq!(
            app.tabs.get(current).unwrap().terminal.size,
            GridSize::new(111, 35)
        );
        let _ = app.update(Message::Pasted(current, Some("x".repeat(100_000))));
        assert!(
            app.tabs
                .get(current)
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("64 KiB")
        );
        let _ = app.update(Message::Select {
            id: original,
            row: 0,
            column: 0,
            side: Side::Left,
            start: true,
        });
        assert!(
            app.tabs
                .get(original)
                .unwrap()
                .terminal
                .term
                .selection
                .is_none()
        );
    }

    #[test]
    fn keyboard_cycles_and_opens_without_falling_through_to_pty() {
        let mut app = app();
        let first = app.tabs.active;
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            Some("T".into()),
        );
        assert_eq!(app.tabs.items.len(), 2);
        let second = app.tabs.active;
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Tab), Modifiers::CTRL, None);
        assert_eq!(app.tabs.active, first);
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::CTRL | Modifiers::SHIFT,
            None,
        );
        assert_eq!(app.tabs.active, second);
        let _ = app.keyboard(
            Key::Character("w".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            Some("W".into()),
        );
        assert_eq!(app.confirming, second.map(CloseTarget::Tab));
    }

    #[test]
    fn invalid_shell_is_visible_and_close_needs_no_confirmation() {
        let mut app = App::new(Options {
            shell: "/nonexistent/hoori-test-shell".into(),
            args: vec![],
        });
        assert!(matches!(
            app.tabs.current().unwrap().state(),
            ProcessState::Failed(_)
        ));
        assert!(
            app.tabs
                .current()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("Shell konnte nicht gestartet")
        );
        let _ = app.request_close(CloseTarget::Window);
        assert!(app.closing);
        assert!(app.confirming.is_none());
    }
}
