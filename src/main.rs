#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod appearance;
mod graphics_info;
mod symbols_view;
mod tab_strip;
mod ui;
mod view;
mod window_chrome;
#[cfg(all(windows, feature = "validation"))]
mod windows_validation;

use hoori_term::build_info;
use hoori_term::input;
use hoori_term::launch::Inventory;
use hoori_term::notifications::Notifications;
use hoori_term::session::ProcessState;
use hoori_term::shortcuts::{self, Action, Shortcut};
use hoori_term::symbols::{self, Symbol};
use hoori_term::templates::{
    Catalog, Loaded, Saved, Template, TemplateFile, TemplateId, TemplateShell,
};
use hoori_term::terminal::GridSize;
use iced::futures::StreamExt;
use iced::widget::{canvas, operation};
use iced::{Event, Fill, Subscription, Task, keyboard, window};

#[derive(Clone)]
struct Options {
    shell: String,
    args: Vec<String>,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut args = std::env::args().skip(1);
        let mut options = Self {
            shell: if cfg!(windows) { "" } else { "/usr/bin/zsh" }.into(),
            args: if cfg!(windows) {
                vec![]
            } else {
                vec!["-i".into()]
            },
        };
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--version" => {
                    println!("{}", build_info::metadata());
                    return Ok(None);
                }
                "--help" => {
                    println!(
                        "hoori-term [--shell executable] [-- shell-arguments]\nDefault: WSL default distribution on Windows; /usr/bin/zsh -i on Ubuntu.\n--version prints embedded build identity."
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
        if options.shell.is_empty() && !options.args.is_empty() {
            return Err("Shell arguments under Windows require --shell".into());
        }
        Ok(Some(options))
    }
}

fn main() -> iced::Result {
    #[cfg(windows)]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        // The VMware OpenGL adapter omitted clipped geometry in HOO-11.
        // Prefer native Vulkan/D3D12, preserving an explicit user override.
        // Windows environment mutation is safe; this also precedes workers.
        unsafe {
            std::env::set_var("WGPU_BACKEND", "vulkan,dx12");
        }
    }
    #[cfg(all(windows, feature = "validation"))]
    if let Some(root) = std::env::var_os("HOORI_GUI_VALIDATION") {
        return windows_validation::run(root.into());
    }
    let options = match Options::parse() {
        Ok(Some(options)) => options,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    graphics_info::enable_if_requested();
    iced::application(move || App::boot(options.clone()), App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(appearance::theme())
        .window(window::Settings {
            size: iced::Size::new(1000.0, 700.0),
            min_size: Some(iced::Size::new(640.0, 360.0)),
            decorations: false,
            exit_on_close_request: false,
            ..Default::default()
        })
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    Tick(hoori_term::tabs::TabId),
    Event(Event, iced::event::Status),
    WindowEvent(window::Id, window::Event),
    WindowAction(window_chrome::Action),
    ObserveWindow,
    WindowObserved(u64, bool),
    Resize(hoori_term::tabs::TabId, GridSize),
    Open(hoori_term::tabs::Shell),
    Strip(tab_strip::Target),
    StripWidth(f32),
    StripScroll(f32),
    StripHover(Option<tab_strip::Target>),
    BeginTabDrag(TabId, iced::Point),
    PreviewTabDrag(TabId, Option<tab_strip::DropTarget>, bool),
    FinishTabDrag(TabId, bool),
    CancelTabDrag,
    MoveTab(TabId, tab_strip::DropTarget),
    ChooseShell(usize),
    StartShell(usize),
    ShowLaunch,
    CancelLaunch,
    #[cfg(windows)]
    LaunchesLoaded(Inventory, bool),
    Activate(hoori_term::tabs::TabId),
    Close(hoori_term::tabs::CloseTarget),
    SetPinned(TabId, bool),
    SetCompact(TabId, bool),
    InspectTab(TabId),
    InspectAction(TabId, ui::TabAction),
    HideInspection,
    InspectionScrolled(f32),
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
    EditCompact(bool),
    ShowSymbols,
    SelectSymbol(Option<Symbol>),
    CancelSymbols,
    CaptureShortcut,
    ClearShortcut,
    ApplyEdit,
    CancelEdit,
    CancelClose,
    ConfirmClose,
    ShutdownComplete,
    ShowTemplates,
    SelectTemplate(TemplateId),
    HideTemplates,
    SaveTemplate(TabId),
    OpenTemplate(TemplateId),
    EditTemplate(TemplateId),
    RemoveTemplate(TemplateId),
    EditTemplateTarget(TemplateTarget),
    #[cfg(windows)]
    DefaultDirectoryLoaded(u64, Result<String, String>),
    EditDirectory(String),
    Loaded(Result<Loaded, String>),
    Saved(u64, Result<Saved, String>),
    RetryLoad,
    RetrySave,
    DiscardAndExit,
}

use hoori_term::tabs::{Accent, CloseTarget, Identity, Shell, TabId, Tabs};

#[derive(Debug, Clone, Copy)]
enum EditorTarget {
    Tab(TabId),
    Save(TabId),
    Template(TemplateId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TemplateTarget {
    shell: TemplateShell,
    distribution: Option<String>,
}
impl std::fmt::Display for TemplateTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.distribution {
            Some(name) => write!(f, "{name} · WSL 2 · {}", self.shell.label()),
            None => f.write_str(self.shell.label()),
        }
    }
}

struct TabEditor {
    target: EditorTarget,
    name: String,
    accent: Accent,
    shortcut: Option<Shortcut>,
    recording: bool,
    error: Option<String>,
    shell: TemplateShell,
    wsl_distribution: Option<String>,
    directory: String,
    directory_request: Option<u64>,
    pinned: bool,
    symbol: Option<Symbol>,
    symbol_picker: Option<usize>,
    compact: bool,
}

impl TabEditor {
    fn identity(&self) -> Identity {
        Identity {
            name: Some(self.name.clone()),
            accent: self.accent,
            shortcut: self.shortcut.clone(),
            symbol: self.symbol,
            compact: self.compact,
        }
    }

    fn template(&self) -> Result<Template, String> {
        let name = hoori_term::tabs::validated_name(&self.name)?
            .ok_or("Eine Vorlage braucht einen Namen")?;
        Ok(Template {
            id: match self.target {
                EditorTarget::Template(id) => id,
                _ => TemplateId(0),
            },
            identity: Identity {
                name: Some(name),
                ..self.identity()
            },
            shell: self.shell,
            wsl_distribution: self.wsl_distribution.clone(),
            directory: self.directory.clone(),
            pinned: self.pinned,
        })
    }
}

struct App {
    tabs: Tabs,
    default_shell: Shell,
    error: Option<String>,
    confirming: Option<CloseTarget>,
    editing: Option<TabEditor>,
    inspecting: Option<TabId>,
    inspection_scroll: f32,
    closing: bool,
    pending_shutdowns: usize,
    strip: tab_strip::State,
    chrome: window_chrome::State,
    viewport_height: f32,
    viewport_width: f32,
    launching: Option<usize>,
    launches: Inventory,
    discovering: bool,
    catalog: Catalog,
    template_file: Option<TemplateFile>,
    template_stamp: Option<Vec<u8>>,
    templates_loaded: bool,
    templates_loading: bool,
    storage_error: Option<String>,
    revision: u64,
    saved_revision: u64,
    saving: Option<u64>,
    templates_open: bool,
    selected_template: Option<TemplateId>,
    #[cfg(windows)]
    next_directory_request: u64,
    cache: canvas::Cache,
}

impl App {
    fn new(options: Options) -> Self {
        let default_shell = Shell {
            program: options.shell,
            args: options.args,
            directory: None,
        };
        let mut tabs = Tabs::default();
        if !default_shell.program.is_empty() {
            tabs.open(&default_shell, GridSize::new(98, 30)).unwrap();
        }
        Self {
            tabs,
            default_shell,
            error: None,
            confirming: None,
            editing: None,
            inspecting: None,
            inspection_scroll: 0.0,
            closing: false,
            pending_shutdowns: 0,
            strip: tab_strip::State::default(),
            chrome: window_chrome::State::default(),
            viewport_height: 700.0,
            viewport_width: 1000.0,
            launching: None,
            launches: Inventory::local(),
            discovering: false,
            catalog: Catalog::default(),
            template_file: None,
            template_stamp: None,
            templates_loaded: true,
            templates_loading: false,
            storage_error: None,
            revision: 0,
            saved_revision: 0,
            saving: None,
            templates_open: false,
            selected_template: None,
            #[cfg(windows)]
            next_directory_request: 0,
            cache: canvas::Cache::new(),
        }
    }

    fn boot(options: Options) -> (Self, Task<Message>) {
        #[cfg(windows)]
        let auto_start = options.shell.is_empty();
        let mut app = Self::new(options);
        #[cfg(windows)]
        {
            app.discovering = true;
            app.launches = Inventory::unavailable("WSL-Ziele werden ermittelt …".into());
            let task = Task::perform(
                async move {
                    tokio::task::spawn_blocking(hoori_term::launch::discover)
                        .await
                        .unwrap_or_else(|_| {
                            Inventory::unavailable("WSL-Ermittlung wurde unterbrochen".into())
                        })
                },
                move |inventory| Message::LaunchesLoaded(inventory, auto_start),
            );
            let load = app.load_templates();
            (app, Task::batch([task, load]))
        }
        #[cfg(not(windows))]
        {
            let task = app.load_templates();
            (app, task)
        }
    }

    fn load_templates(&mut self) -> Task<Message> {
        if self.templates_loading || self.revision != self.saved_revision {
            return Task::none();
        }
        self.templates_loaded = false;
        let file = match TemplateFile::configured() {
            Ok(file) => file,
            Err(error) => {
                self.storage_error = Some(error);
                return Task::none();
            }
        };
        self.template_file = Some(file.clone());
        self.templates_loading = true;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || file.load())
                    .await
                    .map_err(|_| "Laden der Vorlagen wurde unterbrochen".to_owned())
                    .and_then(|r| r)
            },
            Message::Loaded,
        )
    }

    fn persist_changed(&mut self) -> Task<Message> {
        self.revision += 1;
        self.persist()
    }

    fn persist(&mut self) -> Task<Message> {
        if self.saving.is_some() || !self.templates_loaded || self.revision == self.saved_revision {
            return Task::none();
        }
        let Some(file) = self.template_file.clone() else {
            // Model-only App::new does not touch user configuration in tests.
            self.saved_revision = self.revision;
            return Task::none();
        };
        let revision = self.revision;
        self.saving = Some(revision);
        self.storage_error = None;
        let catalog = self.catalog.clone();
        let stamp = self.template_stamp.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || file.save(&catalog, stamp.as_deref()))
                    .await
                    .map_err(|_| "Speichern der Vorlagen wurde unterbrochen".to_owned())
                    .and_then(|r| r)
            },
            move |result| Message::Saved(revision, result),
        )
    }

    fn exit_when_ready(&self) -> Task<Message> {
        if self.closing
            && self.pending_shutdowns == 0
            && self.saving.is_none()
            && self.revision == self.saved_revision
        {
            iced::exit()
        } else {
            Task::none()
        }
    }

    fn editor_conflict(&self, editor: &TabEditor, shortcut: &Shortcut) -> Result<(), String> {
        let (template, tab) = match editor.target {
            EditorTarget::Tab(id) => (self.tabs.get(id).and_then(|t| t.template), Some(id)),
            EditorTarget::Save(id) => (None, Some(id)),
            EditorTarget::Template(id) => (Some(id), None),
        };
        self.catalog
            .check_shortcut(&self.tabs, template, tab, shortcut)
    }

    fn begin_tab_edit(&mut self, id: TabId, save: bool) -> Task<Message> {
        if save && !self.templates_loaded {
            self.error = Some("Vorlagen sind noch nicht verfügbar".into());
            return Task::none();
        }
        let Some(tab) = self.tabs.get(id) else {
            return Task::none();
        };
        if save && let Some(template) = tab.template {
            return self.update(Message::EditTemplate(template));
        }
        self.strip.focused = None;
        self.templates_open = false;
        self.editing = Some(TabEditor {
            target: if save {
                EditorTarget::Save(id)
            } else {
                EditorTarget::Tab(id)
            },
            name: if save {
                tab.label().to_owned()
            } else {
                tab.identity.name.clone().unwrap_or_default()
            },
            accent: tab.identity.accent,
            symbol: tab.identity.symbol,
            symbol_picker: None,
            compact: tab.identity.compact,
            shortcut: tab.identity.shortcut.clone(),
            pinned: tab.pinned,
            recording: false,
            error: None,
            shell: TemplateShell::for_shell(&tab.shell),
            wsl_distribution: hoori_term::launch::wsl_context(&tab.shell)
                .map(|(distro, _, _)| distro.to_owned()),
            directory: tab
                .shell
                .directory
                .clone()
                .or_else(|| {
                    hoori_term::launch::wsl_context(&tab.shell)
                        .and_then(|(_, _, directory)| directory.map(std::path::PathBuf::from))
                })
                .or_else(|| {
                    if hoori_term::launch::wsl_context(&tab.shell).is_some() {
                        None
                    } else {
                        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                            .map(std::path::PathBuf::from)
                    }
                })
                .and_then(|p| p.into_os_string().into_string().ok())
                .unwrap_or_default(),
            directory_request: None,
        });
        Task::batch([
            operation::focus("tab-name"),
            self.request_default_directory(),
        ])
    }

    fn request_default_directory(&mut self) -> Task<Message> {
        #[cfg(windows)]
        if let Some(editor) = &mut self.editing
            && !matches!(editor.target, EditorTarget::Tab(_))
            && editor.directory.is_empty()
            && let Some(distribution) = editor.wsl_distribution.clone()
        {
            self.next_directory_request = self.next_directory_request.wrapping_add(1);
            let request = self.next_directory_request;
            editor.directory_request = Some(request);
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || hoori_term::launch::wsl_home(&distribution))
                        .await
                        .unwrap_or_else(|_| Err("WSL-Home-Ermittlung wurde unterbrochen".into()))
                },
                move |result| Message::DefaultDirectoryLoaded(request, result),
            );
        }
        Task::none()
    }

    fn template_targets(&self) -> Vec<TemplateTarget> {
        if cfg!(windows) {
            let mut targets = vec![TemplateTarget {
                shell: TemplateShell::PowerShell,
                distribution: None,
            }];
            for choice in self.launches.choices.iter().filter(|choice| choice.enabled) {
                if let Some((distribution, _, _)) = hoori_term::launch::wsl_context(&choice.shell) {
                    for shell in [TemplateShell::Bash, TemplateShell::Zsh] {
                        targets.push(TemplateTarget {
                            shell,
                            distribution: Some(distribution.into()),
                        });
                    }
                }
            }
            targets
        } else {
            [TemplateShell::Zsh, TemplateShell::Bash]
                .into_iter()
                .map(|shell| TemplateTarget {
                    shell,
                    distribution: None,
                })
                .collect()
        }
    }

    fn template_target_error(&self, id: TemplateId) -> Option<String> {
        if !cfg!(windows) || self.tabs.items.iter().any(|tab| tab.template == Some(id)) {
            return None;
        }
        let distribution = self.catalog.get(id)?.wsl_distribution.as_deref()?;
        if self.discovering {
            return Some("WSL-Ziele werden noch ermittelt; bitte erneut öffnen.".into());
        }
        let choice = self.launches.choices.iter().find(|choice| {
            hoori_term::launch::wsl_context(&choice.shell)
                .is_some_and(|(name, _, _)| name == distribution)
        });
        match choice {
            Some(choice) if choice.enabled => None,
            Some(_) => Some(format!(
                "WSL-Ziel „{distribution}“ ist nicht als WSL 2 verfügbar. Vorlage bleibt erhalten."
            )),
            None => Some(format!(
                "WSL-Ziel „{distribution}“ fehlt oder WSL ist nicht verfügbar. Vorlage bleibt erhalten; Startziel bewusst bearbeiten."
            )),
        }
    }

    fn title(&self) -> String {
        let Some(tab) = self.tabs.current() else {
            return build_info::title(if self.closing {
                "Wird beendet …"
            } else {
                "Neue Sitzung auswählen"
            });
        };
        let title = if tab.terminal.shell_title.is_empty() {
            tab.description()
        } else {
            format!("{} — {}", tab.description(), tab.terminal.shell_title)
        };
        build_info::title(&if self.confirming.is_some() {
            format!("Schließen bestätigen — {title}")
        } else if let Some(editor) = &self.editing {
            format!(
                "{} — {title}",
                match editor.target {
                    EditorTarget::Tab(_) => "Tab bearbeiten",
                    EditorTarget::Save(_) => "Als Vorlage speichern",
                    EditorTarget::Template(_) => "Vorlage bearbeiten",
                }
            )
        } else if self.templates_open {
            format!("Vorlagen — {title}")
        } else if self.inspecting.is_some() {
            format!("Titel/Status — {title}")
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
        let mut subscriptions = vec![iced::event::listen_with(|event, status, id| {
            Some(match event {
                Event::Window(event) => Message::WindowEvent(id, event),
                event => Message::Event(event, status),
            })
        })];
        // Resize/focus notifications update promptly. This low-frequency
        // fallback also observes external maximization with unchanged geometry.
        // Unfocused/minimized windows do not need a chrome repaint timer.
        if self.chrome.id.is_some() && self.chrome.focused && !self.closing {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_secs(1))
                    .map(|_| Message::ObserveWindow),
            );
        }
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

    fn observe_window(&mut self) -> Task<Message> {
        self.chrome
            .query()
            .map(|(id, revision)| {
                window::is_maximized(id)
                    .map(move |maximized| Message::WindowObserved(revision, maximized))
            })
            .unwrap_or_else(Task::none)
    }

    fn window_event(&mut self, id: window::Id, event: window::Event) -> Task<Message> {
        if self.chrome.id.is_some_and(|current| current != id) {
            return Task::none();
        }
        self.chrome.id = Some(id);
        match event {
            window::Event::CloseRequested => self.request_close(CloseTarget::Window),
            window::Event::Opened { .. } | window::Event::Focused => {
                self.chrome.focused = true;
                self.observe_window()
            }
            window::Event::Resized(size) => {
                self.strip.dragging = None;
                self.viewport_height = size.height;
                self.viewport_width = size.width;
                self.observe_window()
            }
            window::Event::Rescaled(_) => {
                self.strip.dragging = None;
                self.observe_window()
            }
            window::Event::Unfocused => {
                self.strip.dragging = None;
                self.chrome.focused = false;
                self.strip.focused = None;
                self.strip.hovered = None;
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn window_action(&mut self, action: window_chrome::Action) -> Task<Message> {
        use window_chrome::{Action as WindowAction, Control};
        if self.closing {
            return Task::none();
        }
        self.strip.dragging = None;
        self.strip.focused = None;
        self.strip.hovered = None;
        if let WindowAction::Control(Control::Close) = action {
            return self.request_close(CloseTarget::Window);
        }
        let Some(id) = self.chrome.id else {
            return Task::none();
        };
        match action {
            WindowAction::Control(Control::Minimize) => window::minimize(id, true),
            WindowAction::Control(Control::Maximize) => {
                // iced reads the native state when dispatching this request;
                // the UI changes only after a subsequent native observation.
                Task::batch([
                    window::toggle_maximize(id),
                    Task::perform(
                        async {
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        },
                        |_| Message::ObserveWindow,
                    ),
                ])
            }
            WindowAction::Drag => window::drag(id),
            WindowAction::Resize(edge) if !self.chrome.maximized => {
                window::drag_resize(id, edge.direction())
            }
            _ => Task::none(),
        }
    }

    fn request_close(&mut self, target: CloseTarget) -> Task<Message> {
        if self.closing {
            return Task::none();
        }
        self.strip.dragging = None;
        if self.tabs.close_protected(target) {
            let CloseTarget::Tab(id) = target else {
                unreachable!()
            };
            self.error = self.tabs.get(id).map(|tab| {
                format!(
                    "„{}“ ist angepinnt. Erst entpinnen oder „Angepinnten Tab schließen“ wählen.",
                    tab.description()
                )
            });
            self.strip.focused = None;
            return Task::none();
        }
        self.editing = None;
        self.inspecting = None;
        self.templates_open = false;
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
            return self.exit_when_ready();
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
        if self.inspecting.is_some() {
            return match key {
                Key::Named(Named::Escape | Named::Enter) => self.update(Message::HideInspection),
                Key::Named(Named::Tab) => {
                    if modifiers.shift() {
                        operation::focus_previous()
                    } else {
                        operation::focus_next()
                    }
                }
                Key::Named(
                    Named::Home
                    | Named::End
                    | Named::ArrowUp
                    | Named::ArrowDown
                    | Named::PageUp
                    | Named::PageDown,
                ) => {
                    self.inspection_scroll = match key {
                        Key::Named(Named::Home) => 0.0,
                        Key::Named(Named::End) => 1.0,
                        Key::Named(Named::ArrowUp) => (self.inspection_scroll - 0.1).max(0.0),
                        Key::Named(Named::ArrowDown) => (self.inspection_scroll + 0.1).min(1.0),
                        Key::Named(Named::PageUp) => (self.inspection_scroll - 0.5).max(0.0),
                        _ => (self.inspection_scroll + 0.5).min(1.0),
                    };
                    operation::snap_to(
                        "runtime-details",
                        operation::RelativeOffset {
                            x: 0.0,
                            y: self.inspection_scroll,
                        },
                    )
                }
                _ => Task::none(),
            };
        }
        if self.strip.dragging.is_some() {
            if key == Key::Named(Named::Escape) {
                self.strip.dragging = None;
            }
            return Task::none();
        }
        if let Some(selected) = self.launching {
            let count = self.launches.choices.len() + 1;
            return match key {
                Key::Named(Named::Escape) => self.update(Message::CancelLaunch),
                Key::Named(Named::Enter) | Key::Named(Named::Space) => {
                    if selected < self.launches.choices.len() {
                        self.update(Message::ChooseShell(selected))
                    } else {
                        self.update(Message::CancelLaunch)
                    }
                }
                Key::Named(Named::ArrowUp | Named::ArrowLeft) => {
                    self.launching = Some((selected + count - 1) % count);
                    Task::none()
                }
                Key::Named(Named::ArrowDown | Named::ArrowRight | Named::Tab) => {
                    self.launching =
                        Some((selected + if modifiers.shift() { count - 1 } else { 1 }) % count);
                    Task::none()
                }
                _ => Task::none(),
            };
        }
        if let Some(mut editor) = self.editing.take() {
            if let Some(index) = editor.symbol_picker {
                let count = Symbol::ALL.len() + 1;
                match key {
                    Key::Named(Named::Escape) => {
                        self.editing = Some(editor);
                        return self.update(Message::CancelSymbols);
                    }
                    Key::Named(Named::Enter | Named::Space) => {
                        self.editing = Some(editor);
                        return self.update(Message::SelectSymbol(Symbol::choice(index)));
                    }
                    Key::Named(Named::ArrowLeft) => {
                        editor.symbol_picker = Some((index + count - 1) % count)
                    }
                    Key::Named(Named::ArrowRight) => {
                        editor.symbol_picker = Some((index + 1) % count)
                    }
                    Key::Named(Named::ArrowUp) => {
                        editor.symbol_picker = Some((index + count - 3) % count)
                    }
                    Key::Named(Named::ArrowDown) => {
                        editor.symbol_picker = Some((index + 3) % count)
                    }
                    Key::Named(Named::Tab) => {
                        editor.symbol_picker =
                            Some((index + if modifiers.shift() { count - 1 } else { 1 }) % count)
                    }
                    Key::Named(Named::Home) => editor.symbol_picker = Some(0),
                    Key::Named(Named::End) => editor.symbol_picker = Some(count - 1),
                    _ => {}
                }
                self.editing = Some(editor);
                return operation::snap_to(
                    "symbols-list",
                    operation::RelativeOffset {
                        x: 0.0,
                        y: (self.editing.as_ref().unwrap().symbol_picker.unwrap() / 3) as f32 / 2.0,
                    },
                );
            }
            if key == Key::Named(Named::Escape) && modifiers.is_empty() {
                if editor.recording {
                    editor.recording = false;
                    editor.error = None;
                } else {
                    self.editing = Some(editor);
                    return self.update(Message::CancelEdit);
                }
            } else if editor.recording {
                match Shortcut::capture(&key, modifiers) {
                    Ok(Some(shortcut)) => match self.editor_conflict(&editor, &shortcut) {
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
            } else if key == Key::Named(Named::F6) && modifiers.is_empty() {
                self.editing = Some(editor);
                return self.update(Message::ShowSymbols);
            } else if key == Key::Named(Named::F7) && modifiers.is_empty() {
                let compact = !editor.compact;
                self.editing = Some(editor);
                return self.update(Message::EditCompact(compact));
            } else if key == Key::Named(Named::Enter) {
                self.editing = Some(editor);
                return self.update(Message::ApplyEdit);
            } else if key == Key::Named(Named::Tab) {
                self.editing = Some(editor);
                return if modifiers.shift() {
                    operation::focus_previous()
                } else {
                    operation::focus_next()
                };
            }
            self.editing = Some(editor);
            // All other dialog text belongs to widgets, even when unhandled.
            return Task::none();
        }
        if self.templates_open {
            let index = self
                .catalog
                .order()
                .iter()
                .position(|id| Some(*id) == self.selected_template)
                .unwrap_or(0);
            let selected = self.catalog.order().get(index).copied();
            return match key {
                Key::Named(Named::Escape) => self.update(Message::HideTemplates),
                Key::Named(Named::ArrowUp | Named::ArrowDown | Named::Home | Named::End) => {
                    let last = self.catalog.items.len().saturating_sub(1);
                    let next = match key {
                        Key::Named(Named::ArrowUp) => index.saturating_sub(1),
                        Key::Named(Named::ArrowDown) => (index + 1).min(last),
                        Key::Named(Named::Home) => 0,
                        _ => last,
                    };
                    self.selected_template = self.catalog.order().get(next).copied();
                    operation::snap_to(
                        "template-list",
                        operation::RelativeOffset {
                            x: 0.0,
                            y: if last > 0 {
                                next as f32 / last as f32
                            } else {
                                0.0
                            },
                        },
                    )
                }
                Key::Named(Named::Enter) => selected
                    .map(|id| self.update(Message::OpenTemplate(id)))
                    .unwrap_or_else(Task::none),
                Key::Named(Named::Delete) => selected
                    .map(|id| self.update(Message::RemoveTemplate(id)))
                    .unwrap_or_else(Task::none),
                Key::Character(c) if c.eq_ignore_ascii_case("e") => selected
                    .map(|id| self.update(Message::EditTemplate(id)))
                    .unwrap_or_else(Task::none),
                Key::Named(Named::Tab) => {
                    if modifiers.shift() {
                        operation::focus_previous()
                    } else {
                        operation::focus_next()
                    }
                }
                _ => Task::none(),
            };
        }
        if self.strip.focused.is_some()
            && modifiers == keyboard::Modifiers::ALT
            && matches!(key, Key::Named(Named::ArrowLeft | Named::ArrowRight))
        {
            return self.strip_keyboard(key, modifiers);
        }
        if let Some(action) = shortcuts::builtin(&key, modifiers) {
            return self.action(action);
        }
        if let Some(id) = self.catalog.shortcut_target(&key, modifiers) {
            return self.update(Message::OpenTemplate(id));
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
            Action::Templates => return self.update(Message::ShowTemplates),
            Action::SaveTemplate => {
                if let Some(id) = self.tabs.active {
                    return self.update(Message::SaveTemplate(id));
                }
            }
            Action::NewTab | Action::NewBash
                if cfg!(windows) && self.default_shell.program.is_empty() =>
            {
                self.launching = Some(0);
                return Task::none();
            }
            Action::NewTab => return self.update(Message::Open(self.default_shell.clone())),
            Action::NewBash if cfg!(windows) => {
                self.launching = Some(0);
                return Task::none();
            }
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

    fn reveal_template_selection(&self) -> Task<Message> {
        if !self.templates_open {
            return Task::none();
        }
        let index = self
            .catalog
            .order()
            .iter()
            .position(|id| Some(*id) == self.selected_template)
            .unwrap_or(0);
        let last = self.catalog.items.len().saturating_sub(1);
        operation::snap_to(
            "template-list",
            operation::RelativeOffset {
                x: 0.0,
                y: if last > 0 {
                    index as f32 / last as f32
                } else {
                    0.0
                },
            },
        )
    }

    fn strip_keyboard(
        &mut self,
        key: keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Task<Message> {
        use keyboard::{Key, key::Named};
        use tab_strip::Target;
        if modifiers == keyboard::Modifiers::ALT
            && matches!(key, Key::Named(Named::ArrowLeft | Named::ArrowRight))
        {
            if let Some(Target::Tab(id) | Target::Close(id)) = self.strip.focused
                && let Some(index) = self.tabs.items.iter().position(|tab| tab.id == id)
            {
                let after = key == Key::Named(Named::ArrowRight);
                let neighbor = if after {
                    index.checked_add(1)
                } else {
                    index.checked_sub(1)
                };
                if let Some(target) = neighbor.and_then(|index| self.tabs.items.get(index))
                    && target.pinned == self.tabs.items[index].pinned
                {
                    return self.update(Message::MoveTab(
                        id,
                        tab_strip::DropTarget {
                            tab: target.id,
                            after,
                        },
                    ));
                }
            }
            return Task::none();
        }
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
            Key::Character(c) if modifiers.is_empty() && c.eq_ignore_ascii_case("c") => {
                if let Some(Target::Tab(id) | Target::Close(id)) = self.strip.focused
                    && let Some(tab) = self.tabs.get(id)
                {
                    return self.update(Message::SetCompact(id, !tab.identity.compact));
                }
            }
            Key::Character(c) if modifiers.is_empty() && c.eq_ignore_ascii_case("e") => {
                if let Some(Target::Tab(id) | Target::Close(id)) = self.strip.focused {
                    return self.update(Message::Edit(id));
                }
            }
            Key::Character(c) if modifiers.is_empty() && c.eq_ignore_ascii_case("i") => {
                if let Some(Target::Tab(id) | Target::Close(id)) = self.strip.focused {
                    return self.update(Message::InspectTab(id));
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        if self.confirming.is_some()
            || self.editing.is_some()
            || self.inspecting.is_some()
            || self.launching.is_some()
            || self.templates_open
            || self.closing
            || self
                .strip
                .dragging
                .as_ref()
                .is_some_and(|drag| self.tabs.get(drag.source).is_none())
            || matches!(
                message,
                Message::Open(_)
                    | Message::Activate(_)
                    | Message::SetPinned(..)
                    | Message::SetCompact(..)
                    | Message::Edit(_)
                    | Message::InspectTab(_)
                    | Message::ShowTemplates
                    | Message::SaveTemplate(_)
                    | Message::Close(_)
                    | Message::OpenTemplate(_)
            )
        {
            self.strip.dragging = None;
        }
        // Native window handling remains available on modal surfaces, just
        // like the OS close action. It never enters terminal key encoding.
        match message {
            #[cfg(windows)]
            Message::LaunchesLoaded(inventory, auto_start) => {
                self.discovering = false;
                self.launches = inventory;
                if !self.closing && auto_start && self.tabs.items.is_empty() {
                    if let Some(index) = self.launches.default {
                        self.default_shell = self.launches.choices[index].shell.clone();
                        if self.launching.is_none() {
                            return self.update(Message::Open(self.default_shell.clone()));
                        }
                        return Task::none();
                    }
                    self.launching = Some(0);
                }
                return Task::none();
            }
            #[cfg(windows)]
            Message::DefaultDirectoryLoaded(request, result) => {
                if let Some(editor) = &mut self.editing
                    && editor.directory_request == Some(request)
                {
                    editor.directory_request = None;
                    match result {
                        Ok(directory) => editor.directory = directory,
                        Err(error) => {
                            editor.error =
                                Some(format!("Startverzeichnis bitte selbst festlegen: {error}"))
                        }
                    }
                }
                return Task::none();
            }
            Message::WindowEvent(id, event) => return self.window_event(id, event),
            Message::WindowAction(action) => return self.window_action(action),
            Message::ObserveWindow => return self.observe_window(),
            Message::WindowObserved(revision, maximized) => {
                self.chrome.observed(revision, maximized);
                return Task::none();
            }
            _ => {}
        }
        let storage_message = matches!(
            &message,
            Message::Loaded(_)
                | Message::Saved(..)
                | Message::RetryLoad
                | Message::RetrySave
                | Message::DiscardAndExit
        );
        // Dialogs freeze commands and selection, but transport/status updates
        // continue for every tab. No queued paste can bypass a close dialog.
        if !storage_message
            && (self.confirming.is_some() || self.closing)
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
        if !storage_message
            && self.inspecting.is_some()
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::HideInspection
                    | Message::InspectionScrolled(_)
                    | Message::InspectAction(..)
                    | Message::ShutdownComplete
            )
        {
            return Task::none();
        }
        if !storage_message
            && self.editing.is_some()
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::EditName(_)
                    | Message::EditAccent(_)
                    | Message::EditCompact(_)
                    | Message::ShowSymbols
                    | Message::SelectSymbol(_)
                    | Message::CancelSymbols
                    | Message::EditTemplateTarget(_)
                    | Message::EditDirectory(_)
                    | Message::CaptureShortcut
                    | Message::ClearShortcut
                    | Message::ApplyEdit
                    | Message::CancelEdit
                    | Message::ShutdownComplete
            )
        {
            return Task::none();
        }
        if !storage_message
            && self.launching.is_some()
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
        if !storage_message
            && self.templates_open
            && !matches!(
                message,
                Message::Tick(_)
                    | Message::Resize(..)
                    | Message::StripWidth(_)
                    | Message::Event(..)
                    | Message::HideTemplates
                    | Message::SelectTemplate(_)
                    | Message::StartShell(_)
                    | Message::ShowLaunch
                    | Message::SaveTemplate(_)
                    | Message::OpenTemplate(_)
                    | Message::EditTemplate(_)
                    | Message::RemoveTemplate(_)
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
            Message::InspectAction(id, action) => {
                if self.inspecting == Some(id) {
                    self.inspecting = None;
                    self.strip.focused = None;
                    return self.tab_action(id, action);
                }
            }
            Message::InspectTab(id) => {
                if self.tabs.get(id).is_some() {
                    self.inspecting = Some(id);
                    self.inspection_scroll = 0.0;
                    self.strip.hovered = None;
                    self.cache.clear();
                    return operation::snap_to("runtime-details", operation::RelativeOffset::START);
                }
            }
            Message::HideInspection => {
                self.inspecting = None;
                self.strip.focused = None;
                self.cache.clear();
            }
            Message::InspectionScrolled(offset) => self.inspection_scroll = offset,
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
                    Target::Previous => self.strip.scroll(&self.tabs, -224.0),
                    Target::Next => self.strip.scroll(&self.tabs, 224.0),
                    Target::Plus => return self.update(Message::ShowLaunch),
                    Target::Window(control) => {
                        return self.window_action(window_chrome::Action::Control(control));
                    }
                    Target::Drag => {}
                }
            }
            Message::StripWidth(width) => {
                self.strip.dragging = None;
                self.strip.width = width;
                self.reveal_active();
            }
            Message::StripScroll(delta) => {
                self.strip.scroll(&self.tabs, delta);
                if let Some(drag) = &mut self.strip.dragging {
                    drag.target = None;
                }
            }
            Message::StripHover(target) => self.strip.hovered = target,
            Message::BeginTabDrag(source, origin) => {
                if self.tabs.get(source).is_some() {
                    self.strip.dragging = Some(tab_strip::TabDrag {
                        source,
                        origin,
                        moved: false,
                        target: None,
                    });
                    self.strip.hovered = None;
                }
            }
            Message::PreviewTabDrag(source, target, moved) => {
                if let Some(drag) = &mut self.strip.dragging
                    && drag.source == source
                {
                    drag.moved |= moved;
                    drag.target = target;
                }
            }
            Message::FinishTabDrag(source, valid) => {
                if let Some(drag) = self
                    .strip
                    .dragging
                    .take()
                    .filter(|drag| drag.source == source)
                    && valid
                {
                    if drag.moved {
                        if let Some(target) = drag.target {
                            return self.update(Message::MoveTab(source, target));
                        }
                    } else {
                        return self.update(Message::Strip(tab_strip::Target::Tab(source)));
                    }
                }
            }
            Message::CancelTabDrag => self.strip.dragging = None,
            Message::MoveTab(source, target) => {
                match self
                    .catalog
                    .move_tab(&mut self.tabs, source, target.tab, target.after)
                {
                    Ok(durable) => {
                        self.error = None;
                        self.strip.reveal(&self.tabs, source);
                        if durable {
                            return self.persist_changed();
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::ShowLaunch => {
                self.templates_open = false;
                self.launching = Some(0);
                self.strip.focused = None;
            }
            Message::StartShell(index) => {
                if !self.discovering && self.launches.choices.get(index).is_some_and(|c| c.enabled)
                {
                    self.templates_open = false;
                    self.launching = Some(index);
                    return self.update(Message::ChooseShell(index));
                }
            }
            Message::ChooseShell(index) => {
                // Only an explicit choice from the currently open menu starts
                // a session; stale/repeated choice messages cannot start twice.
                if !self.discovering
                    && self.launching.is_some()
                    && let Some(choice) = self.launches.choices.get(index)
                    && choice.enabled
                {
                    let shell = choice.shell.clone();
                    self.launching = None;
                    if self.default_shell.program.is_empty() {
                        self.default_shell = shell.clone();
                    }
                    return self.update(Message::Open(shell));
                }
            }
            Message::CancelLaunch => self.launching = None,
            Message::Close(target) => return self.request_close(target),
            Message::SetPinned(id, pinned) => {
                match self.catalog.set_pinned(&mut self.tabs, id, pinned) {
                    Ok(linked) => {
                        self.error = None;
                        self.strip.focused = None;
                        self.reveal_active();
                        if linked {
                            return self.persist_changed();
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::SetCompact(id, compact) => {
                match self.catalog.set_compact(&mut self.tabs, id, compact) {
                    Ok(linked) => {
                        self.error = None;
                        self.strip.reveal(&self.tabs, id);
                        if linked {
                            return self.persist_changed();
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
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
                || self.inspecting.is_some()
                || self.templates_open
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
            Message::Edit(id) => return self.begin_tab_edit(id, false),
            Message::SaveTemplate(id) => return self.begin_tab_edit(id, true),
            Message::EditTemplateTarget(target) => {
                if !self.template_targets().contains(&target) {
                    if let Some(editor) = &mut self.editing {
                        editor.error = Some("Dieses Startziel ist nicht mehr verfügbar".into());
                    }
                    return Task::none();
                }
                if let Some(editor) = &mut self.editing {
                    let context_changed = editor.wsl_distribution != target.distribution;
                    editor.shell = target.shell;
                    editor.wsl_distribution = target.distribution;
                    editor.error = None;
                    if context_changed {
                        editor.directory_request = None;
                        editor.directory = if editor.wsl_distribution.is_some() {
                            String::new()
                        } else {
                            std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                                .unwrap_or_default()
                        };
                    }
                }
                return self.request_default_directory();
            }
            Message::EditDirectory(directory) => {
                if let Some(editor) = &mut self.editing
                    && !editor.recording
                {
                    editor.directory = directory;
                    editor.directory_request = None;
                    editor.error = None;
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
            Message::ShowSymbols => {
                if let Some(editor) = &mut self.editing
                    && !editor.recording
                {
                    editor.symbol_picker = Some(Symbol::index(editor.symbol));
                }
            }
            Message::SelectSymbol(symbol) => {
                if let Some(editor) = &mut self.editing
                    && editor.symbol_picker.is_some()
                {
                    editor.symbol = symbol;
                    editor.symbol_picker = None;
                    return operation::focus("tab-name");
                }
            }
            Message::EditCompact(compact) => {
                if let Some(editor) = &mut self.editing
                    && !editor.recording
                    && editor.symbol_picker.is_none()
                {
                    editor.compact = compact;
                    return operation::snap_to(
                        "tab-editor-fields",
                        operation::RelativeOffset { x: 0.0, y: 1.0 },
                    );
                }
            }
            Message::CancelSymbols => {
                if let Some(editor) = &mut self.editing {
                    editor.symbol_picker = None;
                    return operation::focus("tab-name");
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
                    && editor.symbol_picker.is_none()
                    && editor.directory_request.is_none()
                {
                    let result = match editor.target {
                        EditorTarget::Tab(id) => {
                            self.catalog
                                .configure_tab(&mut self.tabs, id, editor.identity())
                        }
                        target => {
                            if !self.templates_loaded {
                                Err("Vorlagen sind nicht verfügbar".into())
                            } else {
                                editor.template().and_then(|draft| match target {
                                    EditorTarget::Save(tab) => self
                                        .catalog
                                        .save_tab(&mut self.tabs, tab, draft)
                                        .map(|_| true),
                                    EditorTarget::Template(_) => {
                                        self.catalog.edit(&mut self.tabs, draft).map(|_| true)
                                    }
                                    _ => unreachable!(),
                                })
                            }
                        }
                    };
                    match result {
                        Ok(changed) => {
                            self.templates_open =
                                matches!(editor.target, EditorTarget::Template(_));
                            self.editing = None;
                            self.strip.focused = None;
                            self.reveal_active();
                            self.cache.clear();
                            if changed {
                                let persist = self.persist_changed();
                                return Task::batch([persist, self.reveal_template_selection()]);
                            }
                        }
                        Err(error) => self.editing.as_mut().unwrap().error = Some(error),
                    }
                }
            }
            Message::CancelEdit => {
                self.templates_open = self
                    .editing
                    .as_ref()
                    .is_some_and(|e| matches!(e.target, EditorTarget::Template(_)));
                self.editing = None;
                self.strip.focused = None;
                self.cache.clear();
                return self.reveal_template_selection();
            }
            Message::CancelClose => self.confirming = None,
            Message::ConfirmClose => {
                if let Some(target) = self.confirming {
                    return self.close(target);
                }
            }
            Message::ShutdownComplete => {
                self.pending_shutdowns = self.pending_shutdowns.saturating_sub(1);
                return self.exit_when_ready();
            }
            Message::ShowTemplates => {
                self.templates_open = true;
                self.strip.focused = None;
                if !self
                    .selected_template
                    .is_some_and(|id| self.catalog.get(id).is_some())
                {
                    self.selected_template = self.catalog.order().first().copied();
                }
                return self.reveal_template_selection();
            }
            Message::SelectTemplate(id) => {
                if self.catalog.get(id).is_some() {
                    self.templates_open = true;
                    self.selected_template = Some(id);
                    self.strip.focused = None;
                    return self.reveal_template_selection();
                }
            }
            Message::HideTemplates => {
                self.templates_open = false;
                self.strip.focused = None;
            }
            Message::OpenTemplate(id) => {
                if self.templates_loaded {
                    if let Some(error) = self.template_target_error(id) {
                        self.error = Some(error);
                        return Task::none();
                    }
                    let size = self
                        .tabs
                        .current()
                        .map(|t| t.terminal.size)
                        .unwrap_or(GridSize::new(98, 30));
                    self.error = self.catalog.open(&mut self.tabs, id, size).err();
                    if self.error.is_none() {
                        self.templates_open = false;
                        self.strip.focused = None;
                        self.cache.clear();
                        self.reveal_active();
                    }
                }
            }
            Message::EditTemplate(id) => {
                if self.templates_loaded
                    && let Some(template) = self.catalog.get(id)
                {
                    self.selected_template = Some(id);
                    self.strip.focused = None;
                    self.templates_open = false;
                    self.editing = Some(TabEditor {
                        target: EditorTarget::Template(id),
                        name: template.name().into(),
                        accent: template.identity.accent,
                        symbol: template.identity.symbol,
                        symbol_picker: None,
                        compact: template.identity.compact,
                        shortcut: template.identity.shortcut.clone(),
                        recording: false,
                        error: None,
                        shell: template.shell,
                        wsl_distribution: template.wsl_distribution.clone(),
                        directory: template.directory.clone(),
                        directory_request: None,
                        pinned: template.pinned,
                    });
                    return operation::focus("tab-name");
                }
            }
            Message::RemoveTemplate(id) => {
                if self.templates_loaded {
                    self.error = self.catalog.remove(&mut self.tabs, id).err();
                    if self.error.is_none() {
                        self.selected_template = self.catalog.order().first().copied();
                        let persist = self.persist_changed();
                        return Task::batch([persist, self.reveal_template_selection()]);
                    }
                }
            }
            Message::Loaded(result) => {
                if self.templates_loading {
                    self.templates_loading = false;
                    match result {
                        Ok(loaded) => {
                            // Ordinary tabs can be edited while a slow initial
                            // read is pending. Loading must not steal a binding.
                            if let Some(error) = loaded.catalog.items.iter().find_map(|template| {
                                template.identity.shortcut.as_ref().and_then(|shortcut| {
                                    loaded
                                        .catalog
                                        .check_shortcut(
                                            &self.tabs,
                                            Some(template.id),
                                            None,
                                            shortcut,
                                        )
                                        .err()
                                })
                            }) {
                                self.storage_error =
                                    Some(format!("Vorlagenkonflikt beim Laden: {error}"));
                                return Task::none();
                            }
                            self.catalog = loaded.catalog;
                            self.template_stamp = loaded.stamp;
                            self.templates_loaded = true;
                            self.storage_error = None;
                        }
                        Err(error) => self.storage_error = Some(error),
                    }
                }
            }
            Message::Saved(revision, result) => {
                if self.saving == Some(revision) {
                    self.saving = None;
                    match result {
                        Ok(saved) => {
                            self.template_stamp = Some(saved.stamp);
                            self.storage_error = saved.warning;
                            if self.storage_error.is_none() {
                                self.saved_revision = revision;
                                if self.revision != revision {
                                    return self.persist();
                                }
                            }
                        }
                        Err(error) => self.storage_error = Some(error),
                    }
                    return self.exit_when_ready();
                }
            }
            Message::RetryLoad => return self.load_templates(),
            Message::RetrySave => return self.persist(),
            Message::DiscardAndExit
                if self.closing && self.pending_shutdowns == 0 && self.saving.is_none() =>
            {
                return iced::exit();
            }
            _ => {}
        }
        Task::none()
    }

    fn inspect_tab(&self, id: TabId, close: bool) -> String {
        self.tabs
            .get(id)
            .map(|tab| {
                let shortcut = tab
                    .identity
                    .shortcut
                    .as_ref()
                    .map(Shortcut::label)
                    .unwrap_or_else(|| "Kein Shortcut".into());
                let state = if tab.error.is_some() {
                    "Fehler".into()
                } else {
                    match tab.state() {
                        ProcessState::Running => "läuft".into(),
                        ProcessState::Exited(code) => format!("beendet · Exit {code}"),
                        ProcessState::Failed(_) => "Startfehler".into(),
                    }
                };
                let identity = format!(
                    "{}{} · {shortcut} · {state}{} · {} · Benutzersymbol: {}{}",
                    if close { "Schließen · " } else { "" },
                    tab.description(),
                    if tab.pinned {
                        " · angepinnt, normales Schließen geschützt"
                    } else {
                        ""
                    },
                    if tab.identity.compact {
                        "kompakt"
                    } else {
                        "normal"
                    },
                    symbols::label(tab.identity.symbol),
                    if tab.identity.compact && tab.identity.symbol.is_none() {
                        " (neutraler Terminal-Fallback)"
                    } else {
                        ""
                    }
                );
                format!("{identity}\nTerminaltitel: {}\n{}\nRechtsklick oder I in der Tableiste: vollständige Titel-/Statusdetails", if tab.terminal.shell_title.is_empty() { "(leer)".into() } else { tab_strip::ellipsis(&tab.terminal.shell_title, 70) }, tab.terminal.program_status.summary_label())
            })
            .unwrap_or_default()
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    fn empty() -> App {
        App::new(Options {
            shell: String::new(),
            args: vec![],
        })
    }

    #[test]
    fn unavailable_or_wsl1_default_requires_deliberate_selection_and_stale_choices_cannot_duplicate()
     {
        for inventory in [
            Inventory::unavailable("Missing WSL fixture".into()),
            hoori_term::launch::parse("Legacy\nUbuntu", "* Legacy Stopped 1\nUbuntu Stopped 2")
                .unwrap(),
        ] {
            let mut app = empty();
            let _ = app.update(Message::LaunchesLoaded(inventory, true));
            assert!(app.tabs.items.is_empty());
            assert_eq!(app.launching, Some(0));
            let _ = app.update(Message::ChooseShell(99));
            assert!(app.tabs.items.is_empty());
            let selected = app
                .launches
                .choices
                .iter()
                .position(|choice| choice.enabled)
                .unwrap();
            app.launches.choices[selected].shell.program =
                "C:\\hoori-nonexistent-model-shell.exe".into();
            let _ = app.update(Message::ChooseShell(selected));
            assert_eq!(app.tabs.items.len(), 1);
            let id = app.tabs.active;
            let _ = app.update(Message::ChooseShell(selected));
            assert_eq!(app.tabs.items.len(), 1);
            assert_eq!(app.tabs.active, id);
            assert!(matches!(
                app.tabs.current().unwrap().state(),
                ProcessState::Failed(_)
            ));
        }
    }

    #[test]
    fn late_discovery_cannot_launch_after_close_or_replace_an_explicit_launch() {
        let inventory = hoori_term::launch::parse("Ubuntu", "* Ubuntu Running 2").unwrap();
        let mut app = empty();
        app.closing = true;
        let _ = app.update(Message::LaunchesLoaded(inventory.clone(), true));
        assert!(app.tabs.items.is_empty());
        let mut app = App::new(Options {
            shell: "C:\\hoori-nonexistent-model-shell.exe".into(),
            args: vec![],
        });
        let id = app.tabs.active;
        let _ = app.update(Message::LaunchesLoaded(inventory, false));
        assert_eq!(app.tabs.active, id);
        assert_eq!(app.tabs.items.len(), 1);
    }

    #[test]
    fn supported_default_launches_its_exact_distribution_once() {
        let mut inventory = hoori_term::launch::parse(
            "Ubuntu Test\nOther",
            "* Ubuntu Test Running 2\nOther Stopped 2",
        )
        .unwrap();
        // Observe the selected spawn configuration without running a shell.
        inventory.choices[0].shell.program = "C:\\hoori-nonexistent-model-shell.exe".into();
        let mut app = empty();
        let _ = app.update(Message::LaunchesLoaded(inventory.clone(), true));
        assert_eq!(app.tabs.items.len(), 1);
        assert_eq!(app.tabs.current().unwrap().shell.args[1], "Ubuntu Test");
        assert!(app.launching.is_none());
        let _ = app.update(Message::LaunchesLoaded(inventory, true));
        assert_eq!(app.tabs.items.len(), 1);
    }

    #[test]
    fn late_wsl_home_result_cannot_overwrite_user_input_or_a_changed_target() {
        let mut app = empty();
        app.launches = hoori_term::launch::parse("Ubuntu", "* Ubuntu Running 2").unwrap();
        let id = app
            .tabs
            .open(
                &Shell {
                    program: "C:\\hoori-nonexistent-model-shell.exe".into(),
                    args: vec![],
                    directory: None,
                },
                GridSize::new(80, 24),
            )
            .unwrap();
        app.tabs.get_mut(id).unwrap().shell = hoori_term::launch::wsl("Ubuntu");
        let _ = app.update(Message::SaveTemplate(id));
        let pending = app.editing.as_ref().unwrap().directory_request.unwrap();
        let _ = app.update(Message::EditDirectory("/explicit/user/path".into()));
        let _ = app.update(Message::DefaultDirectoryLoaded(
            pending,
            Ok("/late/home".into()),
        ));
        assert_eq!(
            app.editing.as_ref().unwrap().directory,
            "/explicit/user/path"
        );
        let _ = app.update(Message::EditTemplateTarget(TemplateTarget {
            shell: TemplateShell::PowerShell,
            distribution: None,
        }));
        let native_directory = app.editing.as_ref().unwrap().directory.clone();
        let _ = app.update(Message::DefaultDirectoryLoaded(
            pending,
            Ok("/late/home".into()),
        ));
        let editor = app.editing.as_ref().unwrap();
        assert_eq!(editor.shell, TemplateShell::PowerShell);
        assert!(editor.wsl_distribution.is_none());
        assert_eq!(editor.directory, native_directory);
        let _ = app.update(Message::EditTemplateTarget(TemplateTarget {
            shell: TemplateShell::Zsh,
            distribution: Some("Ubuntu".into()),
        }));
        let editor = app.editing.as_ref().unwrap();
        assert_eq!(editor.shell, TemplateShell::Zsh);
        assert_eq!(editor.wsl_distribution.as_deref(), Some("Ubuntu"));
        assert_ne!(editor.directory_request, Some(pending));
        let _ = app.update(Message::ApplyEdit);
        assert!(
            app.catalog.items.is_empty(),
            "pending WSL directory must not be saved"
        );
    }
}

#[cfg(all(test, unix))]
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
        let _ = app.update(Message::Edit(original));
        let _ = app.update(Message::WindowAction(window_chrome::Action::Control(
            window_chrome::Control::Close,
        )));
        assert!(app.editing.is_none());
        assert_eq!(app.confirming, Some(CloseTarget::Window));
        assert_eq!(app.tabs.running_names(CloseTarget::Window).len(), 2);
        // Native and own close requests share the same confirmation, including
        // repetitions while it is already visible. Neither confirms it.
        let _ = app.update(Message::WindowEvent(
            window::Id::unique(),
            window::Event::CloseRequested,
        ));
        let _ = app.update(Message::WindowAction(window_chrome::Action::Control(
            window_chrome::Control::Close,
        )));
        assert_eq!(app.confirming, Some(CloseTarget::Window));
        assert_eq!(app.pending_shutdowns, 0);
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
    fn drag_and_keyboard_reorder_match_in_both_groups_and_invalid_gestures_do_not_move_neighbors() {
        use tab_strip::{DropTarget, Target};
        let setup = || {
            let mut app = app();
            for _ in 1..12 {
                let _ = app.update(Message::Open(app.default_shell.clone()));
            }
            for id in [TabId(1), TabId(2), TabId(3), TabId(4)] {
                let _ = app.update(Message::SetPinned(id, true));
            }
            app
        };
        let mut mouse = setup();
        let mut keyboard = setup();
        let ids = |app: &App| app.tabs.items.iter().map(|tab| tab.id).collect::<Vec<_>>();
        let origin = iced::Point::new(100.0, 25.0);
        for (source, target) in [(TabId(4), TabId(3)), (TabId(10), TabId(9))] {
            let _ = mouse.update(Message::BeginTabDrag(source, origin));
            let _ = mouse.update(Message::PreviewTabDrag(
                source,
                Some(DropTarget {
                    tab: target,
                    after: false,
                }),
                true,
            ));
            let _ = mouse.update(Message::FinishTabDrag(source, true));
            keyboard.strip.focused = Some(Target::Tab(source));
            let _ = keyboard.keyboard(
                Key::Named(keyboard::key::Named::ArrowLeft),
                Modifiers::ALT,
                None,
            );
            assert_eq!(ids(&mouse), ids(&keyboard));
            assert_eq!(mouse.tabs.active, Some(TabId(12)));
            assert_eq!(keyboard.tabs.active, mouse.tabs.active);
            assert_eq!(keyboard.strip.focused, Some(Target::Tab(source)));
        }
        let before = ids(&mouse);
        for target in [
            None,
            Some(DropTarget {
                tab: TabId(1),
                after: true,
            }),
            Some(DropTarget {
                tab: TabId(999),
                after: false,
            }),
        ] {
            let _ = mouse.update(Message::BeginTabDrag(TabId(10), origin));
            let _ = mouse.update(Message::PreviewTabDrag(TabId(10), target, true));
            let _ = mouse.update(Message::FinishTabDrag(TabId(10), true));
            assert_eq!(ids(&mouse), before);
        }
        for cancel in [
            Message::CancelTabDrag,
            Message::StripWidth(640.0),
            Message::Strip(tab_strip::Target::Plus),
        ] {
            let _ = mouse.update(Message::BeginTabDrag(TabId(10), origin));
            let _ = mouse.update(Message::PreviewTabDrag(
                TabId(10),
                Some(DropTarget {
                    tab: TabId(5),
                    after: false,
                }),
                true,
            ));
            let _ = mouse.update(cancel);
            let _ = mouse.update(Message::FinishTabDrag(TabId(10), true));
            assert_eq!(ids(&mouse), before);
            if mouse.launching.is_some() {
                let _ = mouse.update(Message::CancelLaunch);
            }
        }
        let _ = mouse.update(Message::BeginTabDrag(TabId(10), origin));
        let _ = mouse.update(Message::PreviewTabDrag(
            TabId(10),
            Some(DropTarget {
                tab: TabId(5),
                after: false,
            }),
            true,
        ));
        let _ = mouse.update(Message::StripScroll(100.0));
        assert!(mouse.strip.dragging.as_ref().unwrap().target.is_none());
        let _ = mouse.update(Message::FinishTabDrag(TabId(10), true));
        assert_eq!(ids(&mouse), before); // Scroll needs a fresh preview before release.
        let _ = mouse.update(Message::BeginTabDrag(TabId(10), origin));
        let _ = mouse.update(Message::PreviewTabDrag(
            TabId(10),
            Some(DropTarget {
                tab: TabId(9),
                after: false,
            }),
            true,
        ));
        mouse.tabs.remove(CloseTarget::Tab(TabId(9)));
        let after_close = ids(&mouse);
        let _ = mouse.update(Message::FinishTabDrag(TabId(10), true));
        assert_eq!(ids(&mouse), after_close);
        let _ = mouse.update(Message::BeginTabDrag(TabId(10), origin));
        mouse.tabs.remove(CloseTarget::Tab(TabId(10)));
        let after_close = ids(&mouse);
        let _ = mouse.update(Message::FinishTabDrag(TabId(10), true));
        assert_eq!(ids(&mouse), after_close);
        assert_eq!(mouse.tabs.active, Some(TabId(12)));
        keyboard.strip.focused = Some(Target::Tab(TabId(1)));
        let before = ids(&keyboard);
        let _ = keyboard.keyboard(
            Key::Named(keyboard::key::Named::ArrowLeft),
            Modifiers::ALT,
            None,
        );
        assert_eq!(ids(&keyboard), before); // Group endpoint does not wrap or unpin.
        keyboard.strip.focused = Some(Target::Tab(TabId(5)));
        let _ = keyboard.keyboard(
            Key::Named(keyboard::key::Named::ArrowLeft),
            Modifiers::ALT,
            None,
        );
        assert_eq!(ids(&keyboard), before); // First ordinary tab cannot enter the pin group.
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
    fn side_panel_and_navigation_actions_keep_their_id_and_reject_stale_commands() {
        let mut app = App::new(Options {
            shell: "/nonexistent/hoo37-model".into(),
            args: vec![],
        });
        let first = app.tabs.active.unwrap();
        app.tabs
            .configure(first, "Backend", Accent::Green, None)
            .unwrap();
        let identity = app.tabs.get(first).unwrap().identity.clone();
        let template = app
            .catalog
            .save_tab(
                &mut app.tabs,
                first,
                Template {
                    id: TemplateId(0),
                    identity,
                    shell: TemplateShell::Bash,
                    wsl_distribution: None,
                    directory: "/tmp".into(),
                    pinned: false,
                },
            )
            .unwrap();
        let _ = app.update(Message::Open(app.default_shell.clone()));
        let active = app.tabs.active.unwrap();
        let _ = app.update(Message::InspectTab(first));
        let _ = app.update(Message::InspectAction(active, ui::TabAction::Pin));
        assert_eq!(app.inspecting, Some(first));
        assert!(!app.tabs.get(active).unwrap().pinned);
        let _ = app.update(Message::InspectAction(first, ui::TabAction::Pin));
        assert!(app.tabs.get(first).unwrap().pinned);
        assert!(app.catalog.get(template).unwrap().pinned);
        assert_eq!(app.tabs.active, Some(active));
        let _ = app.update(Message::InspectAction(first, ui::TabAction::Pin));
        assert!(app.tabs.get(first).unwrap().pinned); // A delayed panel click cannot toggle twice.
        let _ = app.update(Message::InspectTab(first));
        let _ = app.update(Message::InspectAction(first, ui::TabAction::Edit));
        let _ = app.update(Message::EditName("Changed Backend".into()));
        let _ = app.update(Message::StartShell(0));
        let _ = app.update(Message::SelectTemplate(template));
        assert!(!app.templates_open);
        assert_eq!(app.tabs.items.len(), 2);
        let window = window::Id::unique();
        let _ = app.update(Message::WindowEvent(
            window,
            window::Event::Resized(iced::Size::new(640.0, 360.0)),
        ));
        assert_eq!((app.viewport_width, app.viewport_height), (640.0, 360.0));
        let _ = app.update(Message::Resize(active, GridSize::new(62, 9)));
        assert_eq!(
            app.tabs.get(active).unwrap().terminal.size,
            GridSize::new(62, 9)
        );
        let _ = app.update(Message::ApplyEdit);
        assert_eq!(app.tabs.active, Some(active));
        assert_eq!(app.tabs.get(first).unwrap().label(), "Changed Backend");
        assert_eq!(app.catalog.get(template).unwrap().name(), "Changed Backend");
        assert_eq!(app.tabs.get(first).unwrap().template, Some(template));
        let _ = app.update(Message::SelectTemplate(template));
        assert!(app.templates_open);
        assert_eq!(app.selected_template, Some(template));
        let _ = app.update(Message::EditTemplate(template));
        let _ = app.update(Message::EditName("Discarded".into()));
        let _ = app.update(Message::CancelEdit);
        assert!(app.templates_open); // The template editor returns to its list context.
        assert_eq!(app.catalog.get(template).unwrap().name(), "Changed Backend");
        let _ = app.update(Message::ShowLaunch);
        assert!(!app.templates_open);
        assert_eq!(app.launching, Some(0));
        assert_eq!(app.tabs.items.len(), 2);
        let _ = app.update(Message::CancelLaunch);
        let _ = app.update(Message::StartShell(usize::MAX));
        assert_eq!(app.tabs.items.len(), 2);
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
        let identity = app.tabs.get(first).unwrap().identity.clone();
        app.catalog
            .save_tab(
                &mut app.tabs,
                first,
                Template {
                    id: TemplateId(0),
                    identity,
                    shell: TemplateShell::Bash,
                    wsl_distribution: None,
                    directory: "/tmp".into(),
                    pinned: false,
                },
            )
            .unwrap();
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        assert_eq!(app.tabs.active, Some(first));
        // The same header focus path reaches the window controls in reverse
        // order. Native requests are tasks; these model tests check routing,
        // while the real desktop run verifies the native effects.
        for steps in [2, 3] {
            let _ = app.action(Action::FocusTabs);
            for _ in 0..steps {
                let _ = app.keyboard(
                    Key::Named(keyboard::key::Named::Tab),
                    Modifiers::SHIFT,
                    None,
                );
            }
            assert_eq!(
                app.strip.focused,
                Some(tab_strip::Target::Window(if steps == 2 {
                    window_chrome::Control::Maximize
                } else {
                    window_chrome::Control::Minimize
                }))
            );
            let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
            assert!(app.strip.focused.is_none());
        }
        let _ = app.update(Message::WindowAction(window_chrome::Action::Drag));
        let _ = app.update(Message::WindowAction(window_chrome::Action::Resize(
            window_chrome::Edge::East,
        )));
        let _ = app.action(Action::FocusTabs);
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::Tab),
            Modifiers::SHIFT,
            None,
        );
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert_eq!(app.confirming, Some(CloseTarget::Window));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.confirming.is_none());
        assert_eq!(app.tabs.items.len(), 2);
        // Real raw-mode readers make extra bytes observable. Focus movement
        // and both menu cancellation paths must not activate/start/send data.
        let _ = app.action(Action::FocusTabs);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::ArrowRight), mods, None);
        assert_eq!(app.strip.focused, Some(tab_strip::Target::Tab(second)));
        assert_eq!(app.tabs.active, Some(first));
        assert!(app.inspect_tab(second, false).contains("Logs · 2 · F9"));
        let before_inspection = app.catalog.clone();
        let _ = app.keyboard(Key::Character("i".into()), mods, Some("i".into()));
        assert_eq!(app.inspecting, Some(second));
        for key in [
            keyboard::key::Named::Home,
            keyboard::key::Named::End,
            keyboard::key::Named::ArrowUp,
            keyboard::key::Named::PageDown,
            keyboard::key::Named::F9,
        ] {
            let _ = app.keyboard(Key::Named(key), mods, None);
        }
        let _ = app.keyboard(Key::Character("x".into()), mods, Some("x".into()));
        let _ = app.update(Message::Pasted(first, Some("hidden status paste".into())));
        let _ = app.update(Message::Open(app.default_shell.clone()));
        assert_eq!(app.tabs.active, Some(first));
        assert_eq!(app.tabs.items.len(), 2);
        assert_eq!(app.catalog, before_inspection);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.inspecting.is_none());
        assert!(app.strip.focused.is_none());
        // Closing inactive-tab details returns ordinary text to the current
        // PTY immediately, rather than swallowing it as tab-strip navigation.
        let _ = app.keyboard(Key::Character("r".into()), mods, Some("r".into()));
        wait(&mut app, |a| {
            a.tabs.get(first).unwrap().terminal.shell_title == "data-72"
        });
        assert_eq!(app.tabs.get(second).unwrap().terminal.shell_title, "ready");
        let _ = app.update(Message::InspectTab(first)); // Same target-safe mouse route.
        assert_eq!(app.inspecting, Some(first));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert!(app.inspecting.is_none());
        assert!(app.strip.focused.is_none());
        let _ = app.action(Action::FocusTabs);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::ArrowRight), mods, None);
        let _ = app.keyboard(Key::Character("c".into()), mods, Some("c".into()));
        assert!(app.tabs.get(second).unwrap().identity.compact);
        assert!(!app.tabs.get(first).unwrap().identity.compact);
        assert_eq!(app.tabs.active, Some(first)); // Details/width change never activates.
        let _ = app.keyboard(Key::Character("c".into()), mods, Some("c".into()));
        assert!(!app.tabs.get(second).unwrap().identity.compact);
        let _ = app.update(Message::SetCompact(first, true)); // Mouse action route.
        assert!(app.tabs.get(first).unwrap().identity.compact);
        assert!(app.catalog.items[0].identity.compact);
        let order = |app: &App| app.tabs.items.iter().map(|tab| tab.id).collect::<Vec<_>>();
        let identities = [
            app.tabs.get(first).unwrap().identity.clone(),
            app.tabs.get(second).unwrap().identity.clone(),
        ];
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::ArrowLeft),
            Modifiers::ALT,
            None,
        );
        assert_eq!(order(&app), [second, first]);
        assert_eq!(app.tabs.active, Some(first));
        assert_eq!(app.strip.focused, Some(tab_strip::Target::Tab(second)));
        let _ = app.keyboard(
            Key::Named(keyboard::key::Named::ArrowRight),
            Modifiers::ALT,
            None,
        );
        assert_eq!(order(&app), [first, second]);
        let origin = iced::Point::new(240.0, 20.0);
        let target = tab_strip::DropTarget {
            tab: first,
            after: false,
        };
        let _ = app.update(Message::BeginTabDrag(second, origin));
        let _ = app.update(Message::PreviewTabDrag(second, Some(target), true));
        let _ = app.update(Message::FinishTabDrag(second, true));
        assert_eq!(order(&app), [second, first]); // Same result as the keyboard action.
        let _ = app.update(Message::FinishTabDrag(second, true));
        assert_eq!(order(&app), [second, first]); // One consumption per gesture.
        let _ = app.update(Message::MoveTab(
            second,
            tab_strip::DropTarget {
                tab: first,
                after: true,
            },
        ));
        let _ = app.update(Message::BeginTabDrag(second, origin));
        let _ = app.update(Message::PreviewTabDrag(second, Some(target), true));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        let _ = app.update(Message::FinishTabDrag(second, true));
        assert_eq!(order(&app), [first, second]);
        assert_eq!(app.tabs.active, Some(first));
        assert_eq!(
            [
                app.tabs.get(first).unwrap().identity.clone(),
                app.tabs.get(second).unwrap().identity.clone()
            ],
            identities
        );
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
        // Template navigation and dialog input must not also reach the real
        // raw reader, including a deferred clipboard result.
        let _ = app.action(Action::Templates);
        let _ = app.keyboard(Key::Character("x".into()), mods, Some("x".into()));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F9), mods, None);
        let _ = app.update(Message::Pasted(first, Some("hidden template paste".into())));
        let _ = app.update(Message::Open(app.default_shell.clone()));
        assert_eq!(app.tabs.active, Some(first));
        assert_eq!(app.tabs.items.len(), 2);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(!app.templates_open);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        let _ = app.update(Message::SetPinned(first, true));
        let _ = app.action(Action::CloseTab);
        let _ = app.update(Message::Strip(tab_strip::Target::Close(first)));
        assert!(app.confirming.is_none());
        assert!(app.error.as_ref().unwrap().contains("angepinnt"));
        let _ = app.action(Action::FocusTabs);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Delete), mods, None);
        assert!(app.tabs.get(first).unwrap().pinned);
        let _ = app.update(Message::Close(CloseTarget::PinnedTab(first)));
        assert_eq!(app.confirming, Some(CloseTarget::PinnedTab(first)));
        let _ = app.keyboard(
            Key::Character("ignored".into()),
            mods,
            Some("ignored".into()),
        );
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.tabs.get(first).unwrap().pinned);
        let _ = app.update(Message::SetPinned(first, false));
        let _ = app.keyboard(Key::Character("q".into()), mods, Some("q".into()));
        wait(&mut app, |a| {
            a.tabs.get(first).unwrap().terminal.shell_title == "data-7271"
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
        // A chooser has its own keyboard scope. Both chooser and editor
        // cancellation leave live identity untouched; deliberate Apply commits.
        let _ = app.update(Message::Edit(second));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F6), mods, None);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::End), mods, None);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.editing.as_ref().unwrap().symbol.is_none());
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F7), mods, None);
        assert!(app.editing.as_ref().unwrap().compact);
        assert!(!app.tabs.get(second).unwrap().identity.compact);
        let _ = app.update(Message::ShowSymbols);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::ArrowRight), mods, None);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert_eq!(app.editing.as_ref().unwrap().symbol, Some(Symbol::Terminal));
        assert!(app.tabs.get(second).unwrap().identity.symbol.is_none());
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        assert!(app.tabs.get(second).unwrap().identity.symbol.is_none());
        assert!(!app.tabs.get(second).unwrap().identity.compact);
        let _ = app.update(Message::Edit(second));
        let _ = app.update(Message::ShowSymbols);
        let _ = app.update(Message::SelectSymbol(Some(Symbol::Branch))); // Mouse route.
        let _ = app.update(Message::EditCompact(true));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert!(app.tabs.get(second).unwrap().identity.compact);
        assert_eq!(
            app.tabs.get(second).unwrap().identity.symbol,
            Some(Symbol::Branch)
        );
        assert_eq!(app.tabs.get(second).unwrap().label(), "Logs");
        let _ = app.update(Message::Edit(second));
        let _ = app.update(Message::ShowSymbols);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Home), mods, None);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Enter), mods, None);
        assert!(app.tabs.get(second).unwrap().identity.symbol.is_none());
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
        let _ = app.update(Message::Edit(second));
        let _ = app.update(Message::CaptureShortcut);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F7), mods, None);
        assert!(!app.editing.as_ref().unwrap().recording);
        assert!(app.editing.as_ref().unwrap().compact); // Recorder owns F7, no draft toggle.
        assert_eq!(
            app.editing
                .as_ref()
                .unwrap()
                .shortcut
                .as_ref()
                .unwrap()
                .label(),
            "F7"
        );
        let _ = app.update(Message::CancelEdit);
        assert_eq!(
            app.tabs
                .get(second)
                .unwrap()
                .identity
                .shortcut
                .as_ref()
                .unwrap()
                .label(),
            "F9"
        );
        let _ = app.keyboard(Key::Character("z".into()), mods, Some("z".into()));
        wait(&mut app, |a| {
            a.tabs.get(second).unwrap().terminal.shell_title == "data-777a"
        });
        assert_eq!(
            app.tabs.get(first).unwrap().terminal.shell_title,
            "data-7271"
        );
        assert_eq!(app.tabs.items.len(), 2);
        let pinned = app.tabs.get(first).unwrap().pinned;
        let _ = app.update(Message::InspectTab(first));
        let _ = app.keyboard(
            Key::Character("blocked".into()),
            mods,
            Some("blocked".into()),
        );
        let _ = app.update(Message::InspectAction(first, ui::TabAction::Pin));
        assert_eq!(app.tabs.get(first).unwrap().pinned, !pinned);
        assert_eq!(app.tabs.active, Some(second));
        assert!(app.strip.focused.is_none());
        let _ = app.keyboard(Key::Character("k".into()), mods, Some("k".into()));
        wait(&mut app, |a| {
            a.tabs.get(second).unwrap().terminal.shell_title == "data-777a6b"
        });
        let _ = app.update(Message::ShowTemplates);
        let _ = app.keyboard(Key::Character("j".into()), mods, Some("j".into()));
        let _ = app.keyboard(Key::Named(keyboard::key::Named::Escape), mods, None);
        let _ = app.keyboard(Key::Character("m".into()), mods, Some("m".into()));
        wait(&mut app, |a| {
            a.tabs.get(second).unwrap().terminal.shell_title == "data-777a6b6d"
        });
        assert_eq!(
            app.tabs.get(first).unwrap().terminal.shell_title,
            "data-7271"
        );
    }

    #[test]
    fn runtime_titles_and_statuses_never_change_identity_templates_or_navigation() {
        use hoori_term::program_status::State;
        let mut app = app();
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
        app.tabs.get_mut(first).unwrap().identity.symbol = Some(Symbol::Star);
        app.tabs.get_mut(first).unwrap().identity.compact = true;
        app.tabs.set_pinned(first, true).unwrap();
        let identity = app.tabs.get(first).unwrap().identity.clone();
        app.catalog
            .save_tab(
                &mut app.tabs,
                first,
                Template {
                    id: TemplateId(0),
                    identity,
                    shell: TemplateShell::Bash,
                    wsl_distribution: None,
                    directory: "/tmp".into(),
                    pinned: true,
                },
            )
            .unwrap();
        let saved = app.catalog.to_bytes().unwrap();
        let snapshot = |app: &App| {
            app.tabs
                .items
                .iter()
                .map(|t| (t.id, t.identity.clone(), t.pinned, t.template))
                .collect::<Vec<_>>()
        };
        let before = snapshot(&app);
        let full_title = format!("CLI {}", "long-title;界".repeat(32));
        app.tabs.get_mut(first).unwrap().terminal.feed(format!("\x1b]2;{full_title}\x1b\\\x1b]7501;state=blocked:kind=permission:progress=42:app=fixture\x07\x1b]7501;state=error:id=child\x07").as_bytes());
        app.tabs
            .get_mut(second)
            .unwrap()
            .terminal
            .feed(b"\x1b]0;Logs CLI\x07\x1b]7501;state=idle\x07");
        let _ = app.update(Message::Tick(first));
        let _ = app.update(Message::Tick(second));
        assert_eq!(app.tabs.active, Some(second));
        assert_eq!(
            app.tabs.get(first).unwrap().terminal.shell_title,
            full_title
        );
        assert_eq!(
            app.tabs
                .get(first)
                .unwrap()
                .terminal
                .program_status
                .attention_counts(),
            (1, 1)
        );
        assert_eq!(
            app.tabs
                .get(second)
                .unwrap()
                .terminal
                .program_status
                .summary()
                .unwrap()
                .state,
            State::Idle
        );
        let inspection = app.inspect_tab(first, false);
        assert!(inspection.contains("Backend · 1 · F8"));
        assert!(inspection.contains("1 warten, 1 Fehler"));
        let _ = app.update(Message::InspectTab(first));
        assert_eq!(app.tabs.active, Some(second));
        let _ = app.update(Message::HideInspection);
        let _ = app.keyboard(Key::Named(keyboard::key::Named::F8), mods, None);
        assert_eq!(app.tabs.active, Some(first));
        assert!(
            app.title()
                .starts_with(&format!("hoori-term [{}", &build_info::COMMIT[..7]))
        );
        assert!(app.title().contains("Backend · 1 — CLI"));
        let _ = app.request_close(CloseTarget::Window);
        assert_eq!(
            app.tabs.running_names(CloseTarget::Window),
            ["Backend · 1", "Logs · 2"]
        );
        let _ = app.update(Message::CancelClose);
        assert_eq!(snapshot(&app), before);
        assert_eq!(app.catalog.to_bytes().unwrap(), saved);
        assert_eq!(app.revision, 0);
        assert!(app.saving.is_none());
        // Only configuration crosses a restart. Neither terminal title nor any
        // protocol record can become a property of the restored template.
        let restored = Catalog::from_bytes(&saved).unwrap();
        let mut restart = App::new(Options {
            shell: app.default_shell.program.clone(),
            args: app.default_shell.args.clone(),
        });
        let _ = restart.update(Message::Loaded(Ok(Loaded {
            catalog: restored,
            stamp: Some(saved),
        })));
        assert_eq!(restart.tabs.items.len(), 1);
        assert!(
            restart
                .tabs
                .current()
                .unwrap()
                .terminal
                .shell_title
                .is_empty()
        );
        assert!(
            restart
                .tabs
                .current()
                .unwrap()
                .terminal
                .program_status
                .summary()
                .is_none()
        );
    }

    #[test]
    fn loaded_templates_do_not_start_or_link_the_default_tab_and_load_errors_stay_visible() {
        let mut source = app();
        let id = source.tabs.active.unwrap();
        source
            .catalog
            .save_tab(
                &mut source.tabs,
                id,
                Template {
                    id: TemplateId(0),
                    identity: hoori_term::tabs::Identity {
                        name: Some("Saved".into()),
                        accent: Accent::Blue,
                        shortcut: None,
                        symbol: None,
                        compact: false,
                    },
                    shell: TemplateShell::Bash,
                    wsl_distribution: None,
                    directory: "/tmp".into(),
                    pinned: false,
                },
            )
            .unwrap();
        let mut startup = app();
        let default = startup.tabs.active;
        startup.templates_loading = true;
        startup.templates_loaded = false;
        let _ = startup.update(Message::Loaded(Ok(Loaded {
            catalog: source.catalog.clone(),
            stamp: Some(source.catalog.to_bytes().unwrap()),
        })));
        assert!(startup.templates_loaded);
        assert_eq!(startup.catalog.items.len(), 1);
        let mut racing = app();
        let ordinary = racing.tabs.active.unwrap();
        let f8 =
            Shortcut::capture(&Key::Named(keyboard::key::Named::F8), Modifiers::empty()).unwrap();
        racing
            .tabs
            .configure(ordinary, "Edited during load", Accent::Green, f8.clone())
            .unwrap();
        let mut loaded = source.catalog.clone();
        loaded.items[0].identity.shortcut = f8;
        racing.templates_loading = true;
        racing.templates_loaded = false;
        let _ = racing.update(Message::Loaded(Ok(Loaded {
            catalog: loaded,
            stamp: None,
        })));
        assert!(!racing.templates_loaded);
        assert!(
            racing
                .storage_error
                .as_ref()
                .unwrap()
                .contains("Vorlagenkonflikt")
        );
        assert!(racing.catalog.items.is_empty());
        assert_eq!(
            racing
                .tabs
                .shortcut_target(&Key::Named(keyboard::key::Named::F8), Modifiers::empty()),
            Some(ordinary)
        );
        assert_eq!(racing.tabs.items.len(), 1);
        assert_eq!(startup.tabs.items.len(), 1);
        assert_eq!(startup.tabs.active, default);
        assert!(startup.tabs.current().unwrap().template.is_none());
        startup.templates_loading = true;
        startup.templates_loaded = false;
        let _ = startup.update(Message::Loaded(Err("Corrupt file".into())));
        assert_eq!(startup.storage_error.as_deref(), Some("Corrupt file"));
        assert!(!startup.templates_loaded);
        let _ = startup.update(Message::SaveTemplate(default.unwrap()));
        assert!(startup.editing.is_none());
        let _ = startup.update(Message::RemoveTemplate(source.catalog.items[0].id));
        assert_eq!(startup.catalog.items.len(), 1);
    }

    #[test]
    fn saves_coalesce_latest_revision_and_errors_do_not_claim_success_or_lose_pending_changes() {
        let mut app = App::new(Options {
            shell: "/nonexistent/template-state-model".into(),
            args: vec![],
        });
        app.template_file = Some(TemplateFile {
            path: "/nonexistent/model-only/templates.json".into(),
        });
        app.revision = 2;
        app.saved_revision = 0;
        app.saving = Some(1);
        let _ = app.update(Message::Saved(
            1,
            Ok(Saved {
                stamp: b"first".to_vec(),
                warning: None,
            }),
        ));
        assert_eq!(app.saved_revision, 1);
        assert_eq!(app.saving, Some(2));
        assert_eq!(app.template_stamp.as_deref(), Some(b"first".as_slice()));
        let _ = app.update(Message::Saved(1, Err("stale callback".into())));
        assert!(app.storage_error.is_none());
        assert_eq!(app.saving, Some(2));
        let _ = app.update(Message::Saved(2, Err("Disk write failed".into())));
        assert_eq!(app.saved_revision, 1);
        assert_eq!(app.revision, 2);
        assert!(app.saving.is_none());
        assert_eq!(app.storage_error.as_deref(), Some("Disk write failed"));
        let _ = app.request_close(CloseTarget::Window);
        assert!(app.closing); // Empty session set waits for pending configuration.
        assert_ne!(app.revision, app.saved_revision);
        let _ = app.update(Message::RetrySave);
        assert_eq!(app.saving, Some(2));
        let _ = app.update(Message::Saved(
            2,
            Ok(Saved {
                stamp: b"latest".to_vec(),
                warning: None,
            }),
        ));
        assert_eq!(app.revision, app.saved_revision);
        assert!(app.storage_error.is_none());
        assert!(app.saving.is_none());
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

    #[test]
    fn pinned_close_requires_explicit_target_and_window_cancel_keeps_every_session() {
        let mut app = app();
        let first = app.tabs.active.unwrap();
        app.tabs
            .configure(first, "Backend", Accent::Green, None)
            .unwrap();
        let _ = app.update(Message::Open(Shell::zsh()));
        let second = app.tabs.active.unwrap();
        app.tabs
            .configure(second, "Deploy", Accent::Blue, None)
            .unwrap();
        let _ = app.update(Message::SetPinned(first, true));
        let _ = app.update(Message::Close(CloseTarget::Tab(first)));
        assert!(app.confirming.is_none());
        assert_eq!(app.tabs.get(first).unwrap().state(), ProcessState::Running);
        let _ = app.update(Message::Close(CloseTarget::PinnedTab(first)));
        assert_eq!(app.confirming, Some(CloseTarget::PinnedTab(first)));
        assert_eq!(
            app.tabs.running_names(app.confirming.unwrap()),
            ["Backend · 1"]
        );
        let _ = app.update(Message::CancelClose);
        assert!(app.tabs.get(first).unwrap().pinned);
        let _ = app.request_close(CloseTarget::Window);
        assert_eq!(
            app.tabs.running_names(app.confirming.unwrap()),
            ["Backend · 1", "Deploy · 2"]
        );
        let _ = app.update(Message::CancelClose);
        assert!(
            app.tabs
                .items
                .iter()
                .all(|tab| tab.state() == ProcessState::Running)
        );
        let _ = app.update(Message::Close(CloseTarget::PinnedTab(first)));
        let _ = app.update(Message::ConfirmClose);
        assert!(app.tabs.get(first).is_none());
        assert_eq!(app.tabs.get(second).unwrap().state(), ProcessState::Running);
        assert_eq!(app.tabs.active, Some(second));
        assert!(!app.closing);
    }
}
