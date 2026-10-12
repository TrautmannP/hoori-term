mod view;

use hoori_term::build_info;
use hoori_term::input;
use hoori_term::notifications::Notifications;
use hoori_term::session::{ProcessState, Session};
use hoori_term::terminal::{GridSize, Terminal};
use iced::futures::StreamExt;
use iced::widget::{button, canvas, column, container, row, space, text};
use iced::{Element, Event, Fill, Subscription, Task, Theme, keyboard, window};
use std::time::{Duration, Instant};

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
    Tick,
    Event(Event),
    Resize(GridSize),
    CancelClose,
    ConfirmClose,
    ShutdownComplete,
}

struct App {
    terminal: Terminal,
    session: Option<Session>,
    process: ProcessState,
    shell_name: String,
    error: Option<String>,
    confirming: bool,
    closing: bool,
    cache: canvas::Cache,
}

impl App {
    fn new(options: Options) -> Self {
        let size = GridSize::new(98, 30);
        let shell_name = std::path::Path::new(&options.shell)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let (session, process, error) =
            match Session::spawn(&options.shell, &options.args, size.pty()) {
                Ok(session) => (Some(session), ProcessState::Running, None),
                Err(e) => (
                    None,
                    ProcessState::Failed(e.clone()),
                    Some(format!("Shell konnte nicht gestartet werden: {e}")),
                ),
            };
        Self {
            terminal: Terminal::new(size),
            session,
            process,
            shell_name,
            error,
            confirming: false,
            closing: false,
            cache: canvas::Cache::new(),
        }
    }

    fn title(&self) -> String {
        build_info::title(if self.terminal.shell_title.is_empty() {
            &self.shell_name
        } else {
            &self.terminal.shell_title
        })
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![iced::event::listen().map(Message::Event)];
        if let Some(session) = &self.session {
            fn output(
                notifications: &Notifications,
            ) -> impl iced::futures::Stream<Item = Message> + Send + 'static + use<> {
                notifications.stream().map(|_| Message::Tick)
            }
            subscriptions.push(Subscription::run_with(
                session.notifications.clone(),
                output,
            ));
        }
        Subscription::batch(subscriptions)
    }

    fn send(&mut self, bytes: Vec<u8>) {
        if self.process == ProcessState::Running
            && let Some(session) = &self.session
            && let Err(e) = session.write(bytes)
        {
            self.error = Some(e);
        }
    }

    fn request_close(&mut self) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.state() == ProcessState::Running)
        {
            self.confirming = true;
            Task::none()
        } else {
            self.close()
        }
    }

    fn close(&mut self) -> Task<Message> {
        self.closing = true;
        self.confirming = false;
        if let Some(session) = self.session.take() {
            Task::perform(
                async move {
                    session.shutdown();
                },
                |_| Message::ShutdownComplete,
            )
        } else {
            iced::exit()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                let deadline = Instant::now() + Duration::from_millis(4);
                let mut consumed = 0;
                while consumed < 64 * 1024 && Instant::now() < deadline {
                    let Some(bytes) = self.session.as_ref().and_then(Session::read_chunk) else {
                        break;
                    };
                    consumed += bytes.len();
                    for reply in self.terminal.feed(&bytes) {
                        self.send(reply);
                    }
                }
                if consumed > 0 {
                    self.cache.clear();
                }
                if let Some(session) = &self.session {
                    if consumed >= 64 * 1024 || Instant::now() >= deadline {
                        session.notifications.wake();
                    }
                    self.process = session.state();
                    if let Some(error) = session.take_io_error() {
                        self.error = Some(error);
                    }
                }
            }
            Message::Resize(size) => {
                if size != self.terminal.size {
                    self.terminal.resize(size);
                    self.cache.clear();
                    if let Some(session) = &self.session
                        && let Err(e) = session.resize(size.pty())
                    {
                        self.error = Some(format!("Resize fehlgeschlagen: {e}"));
                    }
                }
            }
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                return self.request_close();
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            })) => {
                if self.confirming {
                    if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        self.confirming = false;
                    }
                } else if !self.closing
                    && let Some(bytes) =
                        input::encode(&key, modifiers, text.as_deref(), *self.terminal.term.mode())
                {
                    self.send(bytes);
                }
            }
            Message::CancelClose => self.confirming = false,
            Message::ConfirmClose => return self.close(),
            Message::ShutdownComplete => return iced::exit(),
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let status = match &self.process {
            ProcessState::Running => "Sitzung läuft".into(),
            ProcessState::Exited(code) => format!("Sitzung beendet · Exit {code}"),
            ProcessState::Failed(_) => "Sitzung nicht verfügbar".into(),
        };
        let header: Element<'_, Message> = if self.confirming {
            row![
                text(format!("Laufende Sitzung „{}“ beenden?", self.shell_name)),
                space::horizontal(),
                button("Abbrechen").on_press(Message::CancelClose),
                button("Sitzung beenden")
                    .on_press(Message::ConfirmClose)
                    .style(button::danger)
            ]
            .spacing(12)
            .align_y(iced::Center)
            .into()
        } else {
            row![
                text(&self.shell_name).size(18),
                text("Prototyp").size(13),
                space::horizontal(),
                text(if self.closing {
                    "Wird beendet …".to_owned()
                } else {
                    status
                })
                .size(13)
            ]
            .spacing(14)
            .align_y(iced::Center)
            .into()
        };
        let mut content = column![
            container(header).padding(12).height(58),
            canvas(view::TerminalView {
                terminal: &self.terminal,
                cache: &self.cache
            })
            .width(Fill)
            .height(Fill)
        ];
        if let Some(error) = &self.error {
            content = content.push(container(text(error).size(14)).padding(12));
        }
        content = content.push(
            container(
                text(format!(
                    "{} × {}",
                    self.terminal.size.columns, self.terminal.size.rows
                ))
                .size(12),
            )
            .padding(8),
        );
        container(content).width(Fill).height(Fill).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_shell_is_visible_and_close_needs_no_confirmation() {
        let mut app = App::new(Options {
            shell: "/nonexistent/hoori-test-shell".into(),
            args: vec![],
        });
        assert!(matches!(app.process, ProcessState::Failed(_)));
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .contains("Shell konnte nicht gestartet")
        );
        let _ = app.request_close();
        assert!(app.closing);
        assert!(!app.confirming);
    }

    #[test]
    fn cancel_close_preserves_the_live_session() {
        let mut app = App::new(Options {
            shell: "/bin/bash".into(),
            args: vec!["--noprofile".into(), "--norc".into(), "-i".into()],
        });
        let _ = app.request_close();
        assert!(app.confirming);
        let _ = app.update(Message::CancelClose);
        assert!(!app.confirming);
        assert_eq!(app.session.as_ref().unwrap().state(), ProcessState::Running);
    }
}
