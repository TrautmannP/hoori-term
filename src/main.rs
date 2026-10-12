mod tab_strip;
mod view;

use hoori_term::build_info;
use hoori_term::input;
use hoori_term::notifications::Notifications;
use hoori_term::session::ProcessState;
use hoori_term::shortcuts::{self, Action, Shortcut};
use hoori_term::terminal::GridSize;
use iced::futures::StreamExt;
use iced::widget::{button, canvas, column, container, operation, row, space, text, text_input};
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
    Event(Event, iced::event::Status),
    Resize(hoori_term::tabs::TabId, GridSize),
    Open(hoori_term::tabs::Shell),
    Strip(tab_strip::Target),
    StripWidth(f32),
    StripScroll(f32),
    StripHover(Option<tab_strip::Target>),
    ChooseShell(usize),
    CancelLaunch,
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
    Edit(TabId),
    EditName(String),
    EditAccent(Accent),
    CaptureShortcut,
    ClearShortcut,
    ApplyEdit,
    CancelEdit,
    CancelClose,
    ConfirmClose,
    ShutdownComplete,
}

use hoori_term::tabs::{Accent, CloseTarget, Shell, TabId, Tabs};

struct TabEditor {
    id: TabId,
    name: String,
    accent: Accent,
    shortcut: Option<Shortcut>,
    recording: bool,
    error: Option<String>,
}

struct App {
    tabs: Tabs,
    default_shell: Shell,
    error: Option<String>,
    confirming: Option<CloseTarget>,
    editing: Option<TabEditor>,
    closing: bool,
    pending_shutdowns: usize,
    strip: tab_strip::State,
    launching: Option<usize>,
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
            editing: None,
            closing: false,
            pending_shutdowns: 0,
            strip: tab_strip::State::default(),
            launching: None,
            cache: canvas::Cache::new(),
        }
    }

    fn title(&self) -> String {
        let Some(tab) = self.tabs.current() else {
            return build_info::title("Wird beendet …");
        };
        let title = if tab.terminal.shell_title.is_empty() {
            tab.description()
        } else {
            format!("{} — {}", tab.description(), tab.terminal.shell_title)
        };
        build_info::title(&if self.confirming.is_some() {
            format!("Schließen bestätigen — {title}")
        } else if self.editing.is_some() {
            format!("Tab bearbeiten — {title}")
        } else if self.launching.is_some() {
            format!("Neue Sitzung — {title}")
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
        // A text input captures Escape/Enter too. Observe their status so the
        // editor can handle dialog keys without forwarding captured text to PTY.
        let mut subscriptions = vec![iced::event::listen_with(|event, status, _| {
            Some(Message::Event(event, status))
        })];
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
        self.editing = None;
        self.launching = None;
        self.strip.focused = None;
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
        self.reveal_active();
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
        if let Some(selected) = self.launching {
            return match key {
                Key::Named(Named::Escape) => self.update(Message::CancelLaunch),
                Key::Named(Named::Enter) | Key::Named(Named::Space) => {
                    if selected < 2 {
                        self.update(Message::ChooseShell(selected))
                    } else {
                        self.update(Message::CancelLaunch)
                    }
                }
                Key::Named(Named::ArrowUp | Named::ArrowLeft) => {
                    self.launching = Some((selected + 2) % 3);
                    Task::none()
                }
                Key::Named(Named::ArrowDown | Named::ArrowRight | Named::Tab) => {
                    self.launching = Some((selected + if modifiers.shift() { 2 } else { 1 }) % 3);
                    Task::none()
                }
                _ => Task::none(),
            };
        }
        if let Some(editor) = &mut self.editing {
            if key == Key::Named(Named::Escape) && modifiers.is_empty() {
                if editor.recording {
                    editor.recording = false;
                    editor.error = None;
                } else {
                    return self.update(Message::CancelEdit);
                }
            } else if editor.recording {
                match Shortcut::capture(&key, modifiers) {
                    Ok(Some(shortcut)) => match self.tabs.check_shortcut(editor.id, &shortcut) {
                        Ok(()) => {
                            editor.shortcut = Some(shortcut);
                            editor.recording = false;
                            editor.error = None;
                        }
                        Err(error) => editor.error = Some(error),
                    },
                    Ok(None) => {}
                    Err(error) => editor.error = Some(error),
                }
            } else if key == Key::Named(Named::Enter) {
                return self.update(Message::ApplyEdit);
            } else if key == Key::Named(Named::Tab) {
                return if modifiers.shift() {
                    operation::focus_previous()
                } else {
                    operation::focus_next()
                };
            }
            // All other dialog text belongs to widgets, even when unhandled.
            return Task::none();
        }
        if let Some(action) = shortcuts::builtin(&key, modifiers) {
            return self.action(action);
        }
        if let Some(id) = self.tabs.shortcut_target(&key, modifiers) {
            self.strip.focused = None;
            return self.update(Message::Activate(id));
        }
        if self.strip.focused.is_some() {
            return self.strip_keyboard(key, modifiers);
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

    fn action(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::NewTab => return self.update(Message::Open(self.default_shell.clone())),
            Action::NewBash => return self.update(Message::Open(Shell::bash())),
            Action::NextTab | Action::PreviousTab => {
                self.tabs.cycle(action == Action::NextTab);
                self.strip.focused = None;
                self.reveal_active();
            }
            Action::FocusTabs => {
                self.strip.focused = self.tabs.active.map(tab_strip::Target::Tab);
                self.reveal_active();
            }
            Action::CloseTab => {
                if let Some(id) = self.tabs.active {
                    return self.request_close(CloseTarget::Tab(id));
                }
            }
            Action::EditTab => {
                if let Some(id) = self.tabs.active {
                    return self.update(Message::Edit(id));
                }
            }
            Action::Copy => return self.update(Message::Copy),
            Action::Paste => return self.update(Message::Paste),
            Action::Live => return self.update(Message::Live),
            Action::HistoryTop => {
                if let Some(tab) = self.tabs.current_mut() {
                    tab.terminal.scroll_to_top();
                }
            }
            Action::PageUp | Action::PageDown => {
                if let Some(tab) = self.tabs.current_mut() {
                    let lines = i32::from(tab.terminal.size.rows).saturating_sub(1).max(1);
                    tab.terminal.scroll(if action == Action::PageUp {
                        lines
                    } else {
                        -lines
                    });
                }
            }
        }
        self.cache.clear();
        Task::none()
    }

    fn reveal_active(&mut self) {
        if let Some(id) = self.tabs.active {
            self.strip.reveal(&self.tabs, id);
        }
    }

    fn strip_keyboard(
        &mut self,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::{Key, key::Named};
        use tab_strip::Target;
        match key {
            Key::Named(Named::Escape) => self.strip.focused = None,
            Key::Named(Named::Tab) => self.strip.traverse(&self.tabs, modifiers.shift()),
            Key::Named(Named::ArrowLeft | Named::ArrowRight | Named::Home | Named::End) => {
                let id = match self.strip.focused {
                    Some(Target::Tab(id) | Target::Close(id)) => Some(id),
                    _ => self.tabs.active,
                };
                let current = self
                    .tabs
                    .items
                    .iter()
                    .position(|t| Some(t.id) == id)
                    .unwrap_or(0);
                let last = self.tabs.items.len().saturating_sub(1);
                let index = match key {
                    Key::Named(Named::ArrowLeft) => current.saturating_sub(1),
                    Key::Named(Named::ArrowRight) => (current + 1).min(last),
                    Key::Named(Named::Home) => 0,
                    _ => last,
                };
                if let Some(tab) = self.tabs.items.get(index) {
                    self.strip.focused = Some(Target::Tab(tab.id));
                    self.strip.reveal(&self.tabs, tab.id);
                }
            }
            Key::Named(Named::Enter | Named::Space) => {
                if let Some(target) = self.strip.focused {
                    return self.update(Message::Strip(target));
                }
            }
            Key::Named(Named::Delete) => {
                if let Some(Target::Tab(id) | Target::Close(id)) = self.strip.focused {
                    return self.request_close(CloseTarget::Tab(id));
                }
            }
            _ => {}
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
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::CancelClose
                    | Message::ConfirmClose
                    | Message::ShutdownComplete
            )
        {
            return Task::none();
        }
        if self.editing.is_some()
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::EditName(_)
                    | Message::EditAccent(_)
                    | Message::CaptureShortcut
                    | Message::ClearShortcut
                    | Message::ApplyEdit
                    | Message::CancelEdit
                    | Message::ShutdownComplete
            )
        {
            return Task::none();
        }
        if self.launching.is_some()
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::ChooseShell(_)
                    | Message::CancelLaunch
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
                self.strip.focused = None;
                let size = self
                    .tabs
                    .current()
                    .map(|t| t.terminal.size)
                    .unwrap_or(GridSize::new(98, 30));
                self.error = self.tabs.open(&shell, size).err();
                self.cache.clear();
                self.reveal_active();
            }
            Message::Activate(id) => {
                self.tabs.activate(id);
                self.cache.clear();
                self.reveal_active();
            }
            Message::Strip(target) => {
                use tab_strip::Target;
                match target {
                    Target::Tab(id) => {
                        self.strip.focused = None;
                        return self.update(Message::Activate(id));
                    }
                    Target::Close(id) => return self.update(Message::Close(CloseTarget::Tab(id))),
                    Target::Previous => self.strip.scroll(self.tabs.items.len(), -224.0),
                    Target::Next => self.strip.scroll(self.tabs.items.len(), 224.0),
                    Target::Plus => {
                        self.launching = Some(0);
                        self.strip.focused = None;
                    }
                }
            }
            Message::StripWidth(width) => {
                self.strip.width = width;
                self.reveal_active();
            }
            Message::StripScroll(delta) => self.strip.scroll(self.tabs.items.len(), delta),
            Message::StripHover(target) => self.strip.hovered = target,
            Message::ChooseShell(index) => {
                // Only an explicit choice from the currently open menu starts
                // a session; stale/repeated choice messages cannot start twice.
                if self.launching.take().is_some() && index < 2 {
                    return self.update(Message::Open(if index == 0 {
                        Shell::zsh()
                    } else {
                        Shell::bash()
                    }));
                }
            }
            Message::CancelLaunch => self.launching = None,
            Message::Close(target) => return self.request_close(target),
            Message::Event(Event::Window(window::Event::CloseRequested), _) => {
                return self.request_close(CloseTarget::Window);
            }
            Message::Event(
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                }),
                status,
            ) if self.editing.is_some()
                || self.launching.is_some()
                || status == iced::event::Status::Ignored =>
            {
                return self.keyboard(key, modifiers, text.map(|t| t.to_string()));
            }
            Message::Select {
                id,
                row,
                column,
                side,
                start,
            } => {
                self.strip.focused = None;
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
            Message::Edit(id) => {
                self.strip.focused = None;
                if let Some(tab) = self.tabs.get(id) {
                    self.editing = Some(TabEditor {
                        id,
                        name: tab.identity.name.clone().unwrap_or_default(),
                        accent: tab.identity.accent,
                        shortcut: tab.identity.shortcut.clone(),
                        recording: false,
                        error: None,
                    });
                    return operation::focus("tab-name");
                }
            }
            Message::EditName(name) => {
                if let Some(editor) = &mut self.editing
                    && !editor.recording
                {
                    editor.name = name;
                    editor.error = None;
                }
            }
            Message::EditAccent(accent) => {
                if let Some(editor) = &mut self.editing {
                    editor.accent = accent;
                    editor.error = None;
                }
            }
            Message::CaptureShortcut => {
                if let Some(editor) = &mut self.editing {
                    editor.recording = !editor.recording;
                    editor.error = None;
                }
            }
            Message::ClearShortcut => {
                if let Some(editor) = &mut self.editing {
                    editor.shortcut = None;
                    editor.recording = false;
                    editor.error = None;
                }
            }
            Message::ApplyEdit => {
                if let Some(editor) = &self.editing
                    && !editor.recording
                {
                    match self.tabs.configure(
                        editor.id,
                        &editor.name,
                        editor.accent,
                        editor.shortcut.clone(),
                    ) {
                        Ok(()) => {
                            self.editing = None;
                            self.cache.clear();
                        }
                        Err(error) => self.editing.as_mut().unwrap().error = Some(error),
                    }
                }
            }
            Message::CancelEdit => self.editing = None,
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
        use iced::widget::tooltip;
        let interactive = self.confirming.is_none()
            && self.editing.is_none()
            && self.launching.is_none()
            && !self.closing;
        let header = canvas(tab_strip::TabStrip {
            tabs: &self.tabs,
            state: &self.strip,
            interactive,
        })
        .width(Fill)
        .height(tab_strip::HEIGHT);
        let inspection = match self.strip.hovered {
            Some(tab_strip::Target::Tab(id) | tab_strip::Target::Close(id)) => self.tabs.get(id).map(|tab| {
                let shortcut = tab.identity.shortcut.as_ref().map(|s| format!(" · {}", s.label())).unwrap_or_default();
                format!("{}{}{}", if self.strip.hovered == Some(tab_strip::Target::Close(id)) { "Schließen · " } else { "" }, tab.description(), shortcut)
            }).unwrap_or_default(),
            Some(tab_strip::Target::Plus) => "Neue Sitzung auswählen".into(),
            Some(tab_strip::Target::Previous | tab_strip::Target::Next) => "Tableiste verschieben · auch mit Mausrad".into(),
            None => "Tableiste: Strg+Umschalt+L · Pfeile/Tab navigieren · Enter auswählen · Esc zurück zum Terminal".into(),
        };
        // Removing the tooltip widget also removes an already visible overlay.
        // A longer delay only affects a future hover and leaves it on dialogs.
        let header: Element<'_, Message> = if interactive {
            tooltip(
                header,
                container(text(inspection).size(12).width(420))
                    .padding(8)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(600))
            .into()
        } else {
            header.into()
        };
        let mut content = column![header];
        if let Some(selected) = self.launching {
            let choice = |label: &'static str, index: usize, message: Message| {
                button(label).on_press(message).style(move |theme, status| {
                    if selected == index {
                        button::primary(theme, status)
                    } else {
                        button::secondary(theme, status)
                    }
                })
            };
            content = content.push(
                container(
                    column![
                        text("Neue Sitzung").size(18),
                        text("Shell bewusst auswählen · Pfeile/Tab, Enter · Esc bricht ab")
                            .size(12),
                        row![
                            choice("zsh öffnen", 0, Message::ChooseShell(0)),
                            choice("Bash öffnen", 1, Message::ChooseShell(1)),
                            space::horizontal(),
                            choice("Abbrechen", 2, Message::CancelLaunch)
                        ]
                        .spacing(8),
                    ]
                    .spacing(10),
                )
                .padding(12)
                .style(container::rounded_box),
            );
        }
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
        if let Some(editor) = &self.editing {
            content = content.push(self.editor_view(editor));
        }
        if let Some(tab) = self.tabs.current() {
            content = content.push(
                canvas(view::TerminalView {
                    id: tab.id,
                    terminal: &tab.terminal,
                    cache: &self.cache,
                    interactive,
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
                            tab.description(),
                            tab.terminal.size.columns,
                            tab.terminal.size.rows
                        ))
                        .size(12)
                        .width(Fill),
                        button(text("Tab bearbeiten").size(12))
                            .on_press_maybe(interactive.then_some(Message::Edit(tab.id))),
                        button(text("Kopieren").size(12))
                            .on_press_maybe(interactive.then_some(Message::Copy)),
                        button(text("Einfügen").size(12))
                            .on_press_maybe(interactive.then_some(Message::Paste)),
                        button(text("↓ Live").size(12))
                            .on_press_maybe(interactive.then_some(Message::Live)),
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

    fn editor_view<'a>(&'a self, editor: &'a TabEditor) -> Element<'a, Message> {
        let mut colors = row![text("Farbe").size(14).width(70)]
            .spacing(6)
            .align_y(iced::Center);
        for accent in Accent::ALL {
            let selected = editor.accent == accent;
            colors = colors.push(
                button(text(accent.label()).size(13))
                    .on_press(Message::EditAccent(accent))
                    .style(move |theme, status| accent_style(theme, status, accent, selected)),
            );
        }
        let shortcut = editor
            .shortcut
            .as_ref()
            .map(Shortcut::label)
            .unwrap_or_else(|| "Kein Shortcut".into());
        let helper = if editor.recording {
            "Jetzt Tastenkombination drücken. Esc beendet die Aufnahme."
        } else {
            "Strg, Alt oder Super + Taste; F1–F12 auch ohne. Gilt im fokussierten Fenster."
        };
        let name_input = text_input("Leer = ursprünglicher Sitzungsname", &editor.name)
            .id("tab-name")
            .on_input_maybe(
                (!editor.recording).then_some(Message::EditName as fn(String) -> Message),
            )
            .size(16)
            .padding(8);
        let mut fields = column![
            text(format!("Tab bearbeiten · Sitzung {}", editor.id.0)).size(18),
            row![text("Name").size(14).width(70), name_input]
                .spacing(6)
                .align_y(iced::Center),
            text(format!(
                "Bis zu {} Zeichen. Änderungen gelten für diesen geöffneten Tab.",
                hoori_term::tabs::MAX_NAME_CHARS
            ))
            .size(12),
            colors,
            row![
                text("Shortcut").size(14).width(70),
                text(shortcut).size(14).width(Fill),
                button(if editor.recording {
                    "Aufnahme abbrechen"
                } else {
                    "Aufnehmen"
                })
                .on_press(Message::CaptureShortcut),
                button("Entfernen")
                    .on_press(Message::ClearShortcut)
                    .style(button::secondary),
            ]
            .spacing(8)
            .align_y(iced::Center),
            text(helper).size(12),
        ]
        .spacing(10);
        if let Some(error) = &editor.error {
            fields = fields.push(
                text(error)
                    .size(14)
                    .color(iced::Color::from_rgb8(255, 125, 145)),
            );
        }
        fields = fields.push(
            row![
                space::horizontal(),
                button("Abbrechen")
                    .on_press(Message::CancelEdit)
                    .style(button::secondary),
                button("Übernehmen")
                    .on_press_maybe((!editor.recording).then_some(Message::ApplyEdit)),
            ]
            .spacing(8),
        );
        container(fields)
            .padding(12)
            .style(container::rounded_box)
            .into()
    }
}

fn accent_style(
    theme: &Theme,
    status: button::Status,
    accent: Accent,
    active: bool,
) -> button::Style {
    let [r, g, b] = accent.rgb();
    let color = iced::Color::from_rgb8(r, g, b);
    let strength = if status == button::Status::Hovered {
        0.35
    } else if active {
        0.25
    } else {
        0.12
    };
    let mut style = button::secondary(theme, status);
    style.background = Some(
        iced::Color {
            r: color.r * strength,
            g: color.g * strength,
            b: color.b * strength,
            a: 1.0,
        }
        .into(),
    );
    style.border = iced::Border {
        color: if active {
            iced::Color::from_rgb8(235, 240, 255)
        } else {
            color
        },
        width: if active { 2.0 } else { 1.0 },
        radius: 4.0.into(),
    };
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::Side;
    use iced::keyboard::{Key, Modifiers};
    use std::thread;
    use std::time::{Duration, Instant};

    fn wait(app: &mut App, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            for tab in &mut app.tabs.items {
                tab.poll_output();
            }
            if done(app) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "synthetic PTY input did not arrive"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

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

    #[test]
    fn shell_menu_requires_one_explicit_choice_and_rejects_stale_actions() {
        use tab_strip::Target;
        let mut app = app();
        let first = app.tabs.active;
        let _ = app.update(Message::Strip(Target::Plus));
        assert_eq!(app.launching, Some(0));
        assert_eq!(app.tabs.items.len(), 1);
        let _ = app.update(Message::Open(Shell::bash()));
        let _ = app.update(Message::Activate(TabId(999)));
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            None,
        );
        assert_eq!(app.tabs.items.len(), 1);
        assert_eq!(app.tabs.active, first);
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::empty(),
            None,
        );
        assert!(app.launching.is_none());
        let _ = app.update(Message::ChooseShell(1));
        assert_eq!(app.tabs.items.len(), 1);

        let _ = app.update(Message::Strip(Target::Plus));
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::ArrowDown),
            Modifiers::empty(),
            None,
        );
        assert_eq!(app.launching, Some(1));
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::Enter),
            Modifiers::empty(),
            None,
        );
        assert!(app.launching.is_none());
        assert_eq!(app.tabs.items.len(), 2);
        assert!(app.tabs.current().unwrap().label().starts_with("bash ·"));
        let target = app.tabs.active;
        let _ = app.update(Message::ChooseShell(1));
        assert_eq!(app.tabs.items.len(), 2);
        assert_eq!(app.tabs.active, target);
        assert_eq!(app.tabs.running_names(CloseTarget::Window).len(), 2);
    }

    #[test]
    fn direct_actions_and_editor_input_never_also_reach_the_pty() {
        let script = "import os,tty; tty.setraw(0); os.write(1,b'\x1b]2;ready\x07'); data=bytearray()\nwhile True:\n data.extend(os.read(0,4096)); os.write(1,b'\x1b]2;data-'+data.hex().encode()+b'\x07')";
        let mut app = App::new(Options {
            shell: "/usr/bin/python3".into(),
            args: vec!["-u".into(), "-c".into(), script.into()],
        });
        let first = app.tabs.active.unwrap();
        let _ = app.update(Message::Open(app.default_shell.clone()));
        let second = app.tabs.active.unwrap();
        let mods = Modifiers::empty();
        for (id, name, key) in [
            (first, "Backend", keyboard::key::Named::F8),
            (second, "Logs", keyboard::key::Named::F9),
        ] {
            app.tabs
                .configure(
                    id,
                    name,
                    Accent::Green,
                    Shortcut::capture(&Key::Named(key), mods).unwrap(),
                )
                .unwrap();
        }
        wait(&mut app, |a| {
            a.tabs
                .items
                .iter()
                .all(|t| t.terminal.shell_title == "ready")
        });
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        assert_eq!(app.tabs.active, Some(first));
        // Real raw-mode readers make extra bytes observable. Focus movement
        // and both menu cancellation paths must not activate/start/send data.
        let _ = app.action(Action::FocusTabs);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::ArrowRight), mods, None);
        assert_eq!(app.strip.focused, Some(tab_strip::Target::Tab(second)));
        assert_eq!(app.tabs.active, Some(first));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert_eq!(app.tabs.active, Some(second));
        assert!(app.strip.focused.is_none());
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        let _ = app.update(Message::Strip(tab_strip::Target::Plus));
        let _ = app.keyboard(Key::Character("x".into()), mods, Some("x".into()));
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            None,
        );
        let _ = app.update(Message::Pasted(first, Some("hidden".into())));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        let _ = app.update(Message::Strip(tab_strip::Target::Plus));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::ArrowUp), mods, None);
        assert_eq!(app.launching, Some(2));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert!(app.launching.is_none());
        assert_eq!(app.tabs.items.len(), 2);
        let _ = app.keyboard(Key::Character("q".into()), mods, Some("q".into()));
        wait(&mut app, |a| {
            a.tabs.get(first).unwrap().terminal.shell_title == "data-71"
        });
        assert_eq!(app.tabs.get(second).unwrap().terminal.shell_title, "ready");
        assert!(app.title().contains("Backend"));
        assert!(
            app.title()
                .starts_with(&format!("hoori-term [{}", &build_info::COMMIT[..7]))
        );

        let _ = app.keyboard(Key::Named(keyboard::key::Named::F9), mods, None);
        let _ = app.keyboard(Key::Character("w".into()), mods, Some("w".into()));
        wait(&mut app, |a| {
            a.tabs.get(second).unwrap().terminal.shell_title == "data-77"
        });
        let _ = app.update(Message::Edit(second));
        let _ = app.update(Message::EditName("Draft".into()));
        let _ = app.keyboard(Key::Character("r".into()), mods, Some("r".into()));
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            Some("T".into()),
        );
        let _ = app.update(Message::Open(app.default_shell.clone()));
        let _ = app.update(Message::Pasted(second, Some("hidden paste".into())));
        assert_eq!(app.tabs.items.len(), 2);
        assert_eq!(app.tabs.active, Some(second));
        let _ = app.update(Message::CaptureShortcut);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        assert!(
            app.editing
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("Backend")
        );
        assert!(app.editing.as_ref().unwrap().recording);
        assert_eq!(app.tabs.get(second).unwrap().label(), "Logs");
        let _ = app.keyboard(
            Key::Character("t".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            None,
        );
        assert!(
            app.editing
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("reserviert")
        );
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(!app.editing.as_ref().unwrap().recording);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.editing.is_none());
        assert_eq!(app.tabs.get(second).unwrap().label(), "Logs");
        let _ = app.keyboard(Key::Character("z".into()), mods, Some("z".into()));
        wait(&mut app, |a| {
            a.tabs.get(second).unwrap().terminal.shell_title == "data-777a"
        });
        assert_eq!(app.tabs.get(first).unwrap().terminal.shell_title, "data-71");
        assert_eq!(app.tabs.items.len(), 2);
    }

    #[test]
    fn editor_applies_atomically_and_close_confirmation_uses_custom_names() {
        let mut app = app();
        let id = app.tabs.active.unwrap();
        let _ = app.update(Message::Edit(id));
        let _ = app.update(Message::EditName("Backend".into()));
        let _ = app.update(Message::EditAccent(Accent::Orange));
        let _ = app.update(Message::CaptureShortcut);
        let _ = app.keyboard(
            Key::Character("1".into()),
            Modifiers::CTRL | Modifiers::SHIFT,
            None,
        );
        let _ = app.update(Message::ApplyEdit);
        assert!(app.editing.is_none());
        assert_eq!(app.tabs.current().unwrap().label(), "Backend");
        assert_eq!(app.tabs.current().unwrap().identity.accent, Accent::Orange);
        app.tabs
            .current_mut()
            .unwrap()
            .terminal
            .feed(b"\x1b]2;changed title\x07");
        assert!(app.title().contains("Backend · 1 — changed title"));
        assert_eq!(app.tabs.running_names(CloseTarget::Window), ["Backend · 1"]);
        let _ = app.update(Message::Edit(id));
        let _ = app.update(Message::CaptureShortcut);
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::Escape),
            Modifiers::CTRL,
            None,
        );
        let draft = app.editing.as_ref().unwrap();
        assert!(!draft.recording);
        assert!(
            draft
                .shortcut
                .as_ref()
                .unwrap()
                .matches(&Key::Named(keyboard::key::Named::Escape), Modifiers::CTRL)
        );
        let _ = app.update(Message::EditName("x".repeat(100)));
        let _ = app.update(Message::ClearShortcut);
        let _ = app.update(Message::ApplyEdit);
        assert!(app.editing.as_ref().unwrap().error.is_some());
        assert!(app.tabs.current().unwrap().identity.shortcut.is_some());
        assert_eq!(app.tabs.current().unwrap().label(), "Backend");
        let _ = app.request_close(CloseTarget::Window);
        assert!(app.editing.is_none());
        assert_eq!(app.confirming, Some(CloseTarget::Window));
    }
}
