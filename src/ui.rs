//! View composition only. Session, catalog and persistence rules stay in App.
use super::*;
use crate::appearance as colors;
use iced::widget::{
    button, column, container, pick_list, row, scrollable, space, stack, text, text_input, tooltip,
};
use iced::{Color, Element, Theme};

const NAVIGATION_WIDTH: f32 = 196.0;
const PANEL_WIDTH: f32 = 320.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    Edit,
    Save,
    Pin,
    Compact,
    MoveLeft,
    MoveRight,
    Close,
    ClosePinned,
}

#[derive(Debug, Clone, Copy)]
struct Layout {
    navigation_inline: bool,
    panel_inline: bool,
}

impl Layout {
    fn new(width: f32) -> Self {
        Self {
            navigation_inline: width >= 880.0,
            // Keep at least 700 pixels for terminal and navigation together.
            panel_inline: width >= 1200.0,
        }
    }
}

impl App {
    fn working(&self) -> bool {
        self.confirming.is_none()
            && self.editing.is_none()
            && self.inspecting.is_none()
            && self.launching.is_none()
            && !self.templates_open
            && !self.closing
    }

    pub(super) fn tab_action(&mut self, id: TabId, action: TabAction) -> Task<Message> {
        let Some(tab) = self.tabs.get(id) else {
            return Task::none();
        };
        let message = match action {
            TabAction::Edit => Message::Edit(id),
            TabAction::Save => Message::SaveTemplate(id),
            TabAction::Pin => Message::SetPinned(id, !tab.pinned),
            TabAction::Compact => Message::SetCompact(id, !tab.identity.compact),
            TabAction::Close => Message::Close(CloseTarget::Tab(id)),
            TabAction::ClosePinned => Message::Close(CloseTarget::PinnedTab(id)),
            TabAction::MoveLeft | TabAction::MoveRight => {
                let index = self.tabs.items.iter().position(|t| t.id == id).unwrap();
                let after = action == TabAction::MoveRight;
                let next = if after {
                    index.checked_add(1)
                } else {
                    index.checked_sub(1)
                };
                let Some(target) = next
                    .and_then(|i| self.tabs.items.get(i))
                    .filter(|t| t.pinned == tab.pinned)
                else {
                    return Task::none();
                };
                Message::MoveTab(
                    id,
                    tab_strip::DropTarget {
                        tab: target.id,
                        after,
                    },
                )
            }
        };
        self.update(message)
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        let interactive = self.working();
        let layout = Layout::new(self.viewport_width);
        let header = canvas(tab_strip::TabStrip {
            tabs: &self.tabs,
            state: &self.strip,
            interactive,
            window_interactive: !self.closing,
            maximized: self.chrome.maximized,
        })
        .width(Fill)
        .height(tab_strip::HEIGHT);
        let inspection = match self.strip.hovered {
            Some(tab_strip::Target::Tab(id) | tab_strip::Target::Close(id)) => self.inspect_tab(id, self.strip.hovered == Some(tab_strip::Target::Close(id))),
            Some(tab_strip::Target::Plus) => "Neue Sitzung auswählen".into(),
            Some(tab_strip::Target::Previous | tab_strip::Target::Next) => "Tableiste verschieben · auch mit Mausrad".into(),
            Some(tab_strip::Target::Window(control)) => format!("{} · Strg+Umschalt+L, Umschalt+Tab: Fensterknöpfe", control.label(self.chrome.maximized)),
            Some(tab_strip::Target::Drag) => format!("{}\nZiehen: Fenster bewegen · Doppelklick: maximieren/wiederherstellen", build_info::metadata()),
            None => "Tabs im eigenen Bereich ziehen · Esc: abbrechen\nStrg+Umschalt+L: Tableiste · Alt+Links/Rechts: verschieben · C: Ansicht · E: bearbeiten · I: Details".into(),
        };
        let header: Element<'_, Message> = if interactive && self.strip.dragging.is_none() {
            tooltip(
                header,
                container(text(inspection).size(12).width(420))
                    .padding(8)
                    .style(|_| colors::surface(colors::PANEL)),
                tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(600))
            .into()
        } else {
            header.into()
        };

        let panel = self
            .editing
            .as_ref()
            .map(|editor| self.editor_view(editor))
            .or_else(|| self.inspecting.map(|id| self.inspection_view(id)));
        let mut work = row![].height(Fill);
        if layout.navigation_inline {
            work = work.push(self.navigation_view(false));
        }
        work = work.push(self.terminal_view(layout));
        let mut layers = Vec::new();
        if layout.panel_inline {
            if let Some(panel) = panel {
                work = work.push(panel);
            }
            layers.push(work.into());
        } else {
            layers.push(work.into());
            if let Some(panel) = panel {
                layers.push(row![space::horizontal(), panel].height(Fill).into());
            }
        }
        if self.templates_open && !layout.navigation_inline {
            layers.push(
                row![self.navigation_view(true), space::horizontal()]
                    .height(Fill)
                    .into(),
            );
        }
        if self.launching.is_some() {
            layers.push(
                row![self.launch_view(), space::horizontal()]
                    .height(Fill)
                    .into(),
            );
        }
        if self.confirming.is_some() {
            layers.push(self.close_view());
        }
        let mut content = column![header].spacing(0);
        if let Some(error) = &self.storage_error {
            content = content.push(
                container(
                    row![
                        text(format!(
                            "Vorlagenfehler · {}",
                            tab_strip::ellipsis(error, 95)
                        ))
                        .size(12)
                        .color(colors::ERROR)
                        .width(Fill),
                        if self.templates_loaded {
                            button(text("Erneut speichern").size(12))
                                .on_press_maybe(self.saving.is_none().then_some(Message::RetrySave))
                        } else {
                            button(text("Erneut laden").size(12)).on_press_maybe(
                                (!self.templates_loading).then_some(Message::RetryLoad),
                            )
                        }
                    ]
                    .spacing(10)
                    .align_y(iced::Center),
                )
                .padding([6, 12])
                .style(|_| colors::surface(colors::PANEL)),
            );
        }
        content = content.push(stack(layers).width(Fill).height(Fill));
        stack([
            container(content)
                .width(Fill)
                .height(Fill)
                .style(|_| colors::surface(colors::BACKGROUND))
                .into(),
            canvas(window_chrome::ResizeBorder {
                enabled: !self.closing && !self.chrome.maximized,
            })
            .width(Fill)
            .height(Fill)
            .into(),
        ])
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn storage_status(&self) -> &'static str {
        if self.templates_loading {
            "Vorlagen werden geladen …"
        } else if !self.templates_loaded {
            "Nicht geladen · Datei erhalten"
        } else if self.saving.is_some() {
            "Vorlagen werden gespeichert …"
        } else if self.revision != self.saved_revision {
            "Änderungen NICHT gespeichert"
        } else {
            "Lokal gespeichert · kein Autostart"
        }
    }

    fn navigation_view(&self, overlay: bool) -> Element<'_, Message> {
        let interactive = self.working() || self.templates_open;
        let mut entries = column![].spacing(6);
        for template in self.catalog.ordered() {
            let id = template.id;
            let opened = self.tabs.items.iter().find(|t| t.template == Some(id));
            let state = match opened.map(|t| t.state()) {
                Some(ProcessState::Running) => "geöffnet",
                Some(_) => "beendet / Fehler",
                None => "geschlossen",
            };
            let selected = self.templates_open && self.selected_template == Some(id);
            let shortcut = template
                .identity
                .shortcut
                .as_ref()
                .map(Shortcut::label)
                .unwrap_or_else(|| "ohne Shortcut".into());
            entries = entries.push(column![
                row![
                    button(
                        column![
                            text(format!(
                                "{}{}",
                                if selected { "› " } else { "" },
                                template.name()
                            ))
                            .size(13),
                            text(format!("{} · {state}", template.target_label())).size(11),
                            text(shortcut).size(11),
                        ]
                        .spacing(2)
                    )
                    .width(Fill)
                    .padding(7)
                    .on_press_maybe(
                        (interactive && self.templates_loaded).then_some(Message::OpenTemplate(id))
                    )
                    .style(move |theme, status| accent_style(
                        theme,
                        status,
                        template.identity.accent,
                        selected
                    )),
                    button(text("⋯").size(18))
                        .padding([5, 7])
                        .on_press_maybe(interactive.then_some(Message::SelectTemplate(id))),
                ]
                .spacing(3),
            ]);
        }
        if self.catalog.items.is_empty() {
            entries = entries.push(
                text("Noch keine Vorlagen. Einen Tab bewusst speichern.")
                    .size(12)
                    .color(colors::MUTED),
            );
        }
        let mut shell_choices = column![].spacing(4);
        for (index, target) in self.launches.choices.iter().enumerate() {
            shell_choices = shell_choices.push(
                button(text(&target.label).size(13))
                    .width(Fill)
                    .padding([7, 8])
                    .on_press_maybe(
                        (interactive && target.enabled && !self.discovering)
                            .then_some(Message::StartShell(index)),
                    )
                    .style(colors::secondary),
            );
        }
        if let Some(notice) = &self.launches.notice {
            shell_choices = shell_choices.push(text(notice).size(11).color(colors::MUTED));
        }
        let mut body = if self.templates_open {
            column![
                text("VORLAGEN").size(11).color(colors::MUTED),
                entries,
                text("Entfernen löst Shortcut und Verknüpfung; ein laufender Tab bleibt erhalten.")
                    .size(11)
                    .color(colors::MUTED)
            ]
            .spacing(12)
        } else {
            column![
                text("SHELLSTART").size(11).color(colors::MUTED),
                shell_choices,
                text("VORLAGEN").size(11).color(colors::MUTED),
                entries
            ]
            .spacing(12)
        };
        if let Some(error) = &self.storage_error {
            body = body.push(text(error).size(12).color(colors::ERROR));
        }
        if self.templates_open
            && let Some(template) = self.selected_template.and_then(|id| self.catalog.get(id))
        {
            body = body
                .push(text(format!("Ausgewählt · {}", template.name())).size(12))
                .push(
                    text("↑↓ wählen · Enter öffnen · E bearbeiten · Entf entfernen")
                        .size(11)
                        .color(colors::MUTED),
                );
        }
        let mut content = column![
            row![
                text(if self.templates_open {
                    "Vorlagen"
                } else {
                    "hoori-term"
                })
                .size(17)
                .width(Fill),
                button(
                    text(if self.templates_open || overlay {
                        "Zurück"
                    } else {
                        "Liste"
                    })
                    .size(11)
                )
                .on_press_maybe(interactive.then_some(if self.templates_open {
                    Message::HideTemplates
                } else {
                    Message::ShowTemplates
                }))
                .style(button::text),
            ]
            .align_y(iced::Center),
            button(text("+ Neue Sitzung").size(13))
                .width(Fill)
                .padding([8, 10])
                .on_press_maybe(interactive.then_some(Message::ShowLaunch)),
            scrollable(body.width(Fill))
                .spacing(6)
                .id("template-list")
                .width(Fill)
                .height(Fill),
        ]
        .spacing(8);
        if self.templates_open
            && let Some(template) = self.selected_template.and_then(|id| self.catalog.get(id))
        {
            content = content.push(
                text(format!(
                    "Vorlage {} · {}",
                    template.id.0,
                    tab_strip::ellipsis(template.name(), 22)
                ))
                .size(11),
            );
            content = content.push(
                row![
                    button(text("Bearbeiten").size(12))
                        .width(Fill)
                        .on_press_maybe(
                            self.templates_loaded
                                .then_some(Message::EditTemplate(template.id))
                        ),
                    button(text("Entfernen").size(12))
                        .width(Fill)
                        .style(button::danger)
                        .on_press_maybe(
                            self.templates_loaded
                                .then_some(Message::RemoveTemplate(template.id))
                        ),
                ]
                .spacing(6),
            );
        }
        content = content.push(
            button(
                text(
                    if self.tabs.current().is_some_and(|t| t.template.is_some()) {
                        "Vorlage bearbeiten"
                    } else {
                        "Tab als Vorlage speichern"
                    },
                )
                .size(12),
            )
            .width(Fill)
            .on_press_maybe(
                (interactive && self.templates_loaded)
                    .then(|| self.tabs.active.map(Message::SaveTemplate))
                    .flatten(),
            )
            .style(colors::secondary),
        );
        content = content.push(
            text(format!(
                "{} Vorlagen · {}",
                self.catalog.items.len(),
                self.storage_status()
            ))
            .size(11)
            .color(colors::MUTED),
        );
        container(content)
            .padding(12)
            .width(if overlay { 260.0 } else { NAVIGATION_WIDTH })
            .height(Fill)
            .style(|_| colors::surface(colors::NAVIGATION))
            .into()
    }

    fn terminal_view(&self, layout: Layout) -> Element<'_, Message> {
        if self.closing {
            return container(
                column![
                    text(if self.saving.is_some() {
                        "Vorlagen werden vor dem Beenden gespeichert …"
                    } else if self.revision != self.saved_revision {
                        "Sitzungen beendet. Vorlagenänderungen sind NICHT gespeichert."
                    } else {
                        "Sitzungen werden beendet …"
                    })
                    .size(16),
                    button("Erneut speichern").on_press_maybe(
                        (self.pending_shutdowns == 0
                            && self.saving.is_none()
                            && self.revision != self.saved_revision)
                            .then_some(Message::RetrySave)
                    ),
                    button("Ungespeichert beenden")
                        .style(button::danger)
                        .on_press_maybe(
                            (self.pending_shutdowns == 0
                                && self.saving.is_none()
                                && self.revision != self.saved_revision)
                                .then_some(Message::DiscardAndExit)
                        ),
                ]
                .spacing(16),
            )
            .padding(20)
            .width(Fill)
            .height(Fill)
            .into();
        }
        let interactive = self.working();
        let mut top = row![].spacing(6).align_y(iced::Center);
        if !layout.navigation_inline {
            top = top.push(
                button(text("Navigation").size(12))
                    .on_press_maybe(interactive.then_some(Message::ShowTemplates))
                    .style(colors::secondary),
            );
        }
        if let Some(tab) = self.tabs.current() {
            top = top
                .push(
                    column![
                        text(tab_strip::ellipsis(&tab.description(), 30)).size(13),
                        text(tab_strip::ellipsis(
                            &tab.terminal.program_status.summary_label(),
                            40
                        ))
                        .size(11)
                        .color(colors::MUTED),
                    ]
                    .spacing(2)
                    .width(Fill),
                )
                .push(
                    button(text("Bearbeiten").size(12))
                        .on_press_maybe(interactive.then_some(Message::Edit(tab.id)))
                        .style(colors::secondary),
                )
                .push(
                    button(text("Details / Aktionen").size(12))
                        .on_press_maybe(interactive.then_some(Message::InspectTab(tab.id)))
                        .style(colors::secondary),
                );
        } else {
            top = top.push(text("Neue Sitzung auswählen").size(14).width(Fill));
        }
        let mut content = column![
            container(top)
                .padding([6, 10])
                .style(|_| colors::surface(colors::BACKGROUND))
        ]
        .spacing(0)
        .width(Fill)
        .height(Fill);
        if interactive
            && let Some(tab_strip::Target::Tab(id) | tab_strip::Target::Close(id)) =
                self.strip.focused
        {
            content = content.push(container(column![
                text(self.inspect_tab(id, false)).size(12),
                text("Pfeile: Vorschau · I: Details · Alt+←/→: sortieren · C: Ansicht · E: bearbeiten · Esc: Terminal").size(11).color(colors::MUTED),
            ].spacing(3)).padding([6, 10]).style(|_| colors::surface(colors::RAISED)));
        }
        if let Some(tab) = self.tabs.current() {
            if let Some(error) = self.error.as_ref().or(tab.error.as_ref()) {
                content = content.push(
                    container(
                        text(format!(
                            "Fehler · {} · Details/Aktionen öffnen",
                            tab_strip::ellipsis(error, 80)
                        ))
                        .size(12)
                        .color(colors::ERROR),
                    )
                    .padding([5, 10]),
                );
            }
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
            let process = match tab.state() {
                ProcessState::Running => "läuft".into(),
                ProcessState::Exited(code) => format!("Exit {code}"),
                ProcessState::Failed(_) => "Startfehler".into(),
            };
            let offset = tab.terminal.term.grid().display_offset();
            content = content.push(
                container(
                    row![
                        text(format!(
                            "{} · {process} · {} × {} · Verlauf {offset}",
                            tab_strip::ellipsis(tab.original_label(), 24),
                            tab.terminal.size.columns,
                            tab.terminal.size.rows
                        ))
                        .size(11)
                        .color(colors::MUTED)
                        .width(Fill),
                        button(text("Kopieren").size(12))
                            .on_press_maybe(interactive.then_some(Message::Copy))
                            .style(button::text),
                        button(text("Einfügen").size(12))
                            .on_press_maybe(interactive.then_some(Message::Paste))
                            .style(button::text),
                        button(text("↓ Live").size(12))
                            .on_press_maybe(interactive.then_some(Message::Live))
                            .style(button::text),
                    ]
                    .spacing(4)
                    .align_y(iced::Center),
                )
                .padding([5, 10])
                .style(|_| colors::surface(colors::BACKGROUND)),
            );
        } else {
            content = content.push(container(text("Über + eine Sitzung auswählen. Es wird keine Ersatz-Shell automatisch gestartet.").size(14)).padding(20).height(Fill));
        }
        content.into()
    }

    fn launch_view(&self) -> Element<'_, Message> {
        let selected = self.launching.unwrap_or(0);
        let mut entries = column![
            text("Shell bewusst auswählen")
                .size(12)
                .color(colors::MUTED)
        ]
        .spacing(8);
        for (index, target) in self.launches.choices.iter().enumerate() {
            entries = entries.push(
                button(
                    text(format!(
                        "{}{}",
                        if selected == index { "› " } else { "" },
                        target.label
                    ))
                    .size(14),
                )
                .width(Fill)
                .padding(10)
                .on_press_maybe(
                    (target.enabled && !self.discovering).then_some(Message::ChooseShell(index)),
                )
                .style(if selected == index {
                    button::primary
                } else {
                    colors::secondary
                }),
            );
        }
        if let Some(notice) = &self.launches.notice {
            entries = entries.push(text(notice).size(12));
        }
        container(
            column![
                text("Neue Sitzung").size(18),
                text("Pfeile/Tab · Enter auswählen · Esc zurück")
                    .size(12)
                    .color(colors::MUTED),
                scrollable(entries).height(Fill),
                button("Abbrechen")
                    .width(Fill)
                    .on_press(Message::CancelLaunch)
                    .style(if selected == self.launches.choices.len() {
                        button::primary
                    } else {
                        colors::secondary
                    }),
            ]
            .spacing(12),
        )
        .padding(16)
        .width(PANEL_WIDTH)
        .height(Fill)
        .style(|_| colors::surface(colors::PANEL))
        .into()
    }

    fn close_view(&self) -> Element<'_, Message> {
        let target = self.confirming.unwrap();
        let names = self.tabs.running_names(target);
        let mut details = column![
            text(if names.is_empty() {
                "Sitzungen sind inzwischen beendet. Schließen?"
            } else {
                "Diese laufenden Sitzungen werden beendet:"
            })
            .size(14)
        ]
        .spacing(8);
        for name in names {
            details = details.push(text(format!("• {name}")).size(14));
        }
        let dialog = container(
            column![
                text("Schließen bestätigen").size(20),
                scrollable(details).height(Fill),
                row![
                    button("Abbrechen")
                        .width(Fill)
                        .on_press(Message::CancelClose)
                        .style(colors::secondary),
                    button("Beenden")
                        .width(Fill)
                        .on_press(Message::ConfirmClose)
                        .style(button::danger),
                ]
                .spacing(10),
            ]
            .spacing(14),
        )
        .padding(20)
        .width(540)
        .height(Fill)
        .style(|_| colors::surface(colors::PANEL));
        container(dialog)
            .padding(16)
            .center_x(Fill)
            .center_y(Fill)
            .into()
    }

    fn editor_view<'a>(&'a self, editor: &'a TabEditor) -> Element<'a, Message> {
        let heading = match editor.target {
            EditorTarget::Tab(id) => format!("Tab bearbeiten · Sitzung {}", id.0),
            EditorTarget::Save(id) => format!("Als Vorlage speichern · Sitzung {}", id.0),
            EditorTarget::Template(id) => format!("Vorlage bearbeiten · Vorlage {}", id.0),
        };
        if let Some(focused) = editor.symbol_picker {
            let mut choices = column![].spacing(8);
            for start in (0..=Symbol::ALL.len()).step_by(3) {
                let mut line = row![].spacing(6);
                for index in start..(start + 3).min(Symbol::ALL.len() + 1) {
                    let symbol = Symbol::choice(index);
                    line = line.push(
                        button(
                            column![
                                canvas(symbols_view::Icon(symbol)).width(24).height(24),
                                text(format!(
                                    "{}{}",
                                    if index == focused { "› " } else { "" },
                                    symbols::label(symbol)
                                ))
                                .size(11),
                            ]
                            .spacing(6)
                            .align_x(iced::Center),
                        )
                        .width(Fill)
                        .padding(6)
                        .on_press(Message::SelectSymbol(symbol))
                        .style(if index == focused {
                            button::primary
                        } else {
                            colors::secondary
                        }),
                    );
                }
                choices = choices.push(line);
            }
            return container(column![
                text("Tab-Symbol auswählen").size(18),
                text("Benutzersymbole sind keine Statusmeldungen. Pfeile/Tab · Enter · Esc zurück.").size(12).color(colors::MUTED),
                scrollable(choices).spacing(6).id("symbols-list").height(Fill),
                button("Abbrechen").width(Fill).on_press(Message::CancelSymbols).style(colors::secondary),
            ].spacing(12)).padding(16).width(PANEL_WIDTH).height(Fill).style(|_| colors::surface(colors::PANEL)).into();
        }
        let name = text_input("Leer = ursprünglicher Sitzungsname", &editor.name)
            .style(colors::input)
            .id("tab-name")
            .on_input_maybe(
                (!editor.recording).then_some(Message::EditName as fn(String) -> Message),
            )
            .size(14)
            .padding(9);
        let mut fields = column![
            text("Name").size(12).color(colors::MUTED), name,
            text(format!("Bis zu {} Zeichen. {}", hoori_term::tabs::MAX_NAME_CHARS,
                if matches!(editor.target, EditorTarget::Tab(id) if self.tabs.get(id).is_some_and(|t| t.template.is_some())) { "Änderungen führen in derselben Vorlage mit." }
                else if matches!(editor.target, EditorTarget::Tab(_)) { "Vorübergehender Tab; nicht gespeichert." }
                else { "Lokal speichern; kein zusätzlicher Prozessstart." })).size(11).color(colors::MUTED),
            text("Farbe").size(12).color(colors::MUTED),
        ].spacing(8);
        for accents in Accent::ALL.chunks(3) {
            let mut line = row![].spacing(6);
            for &accent in accents {
                let selected = editor.accent == accent;
                line = line.push(
                    button(
                        text(format!(
                            "{}{}",
                            if selected { "✓ " } else { "" },
                            accent.label()
                        ))
                        .size(12),
                    )
                    .width(Fill)
                    .padding([6, 4])
                    .on_press_maybe((!editor.recording).then_some(Message::EditAccent(accent)))
                    .style(move |theme, status| accent_style(theme, status, accent, selected)),
                );
            }
            fields = fields.push(line);
        }
        fields = fields
            .push(text("Symbol").size(12).color(colors::MUTED))
            .push(
                button(
                    row![
                        canvas(symbols_view::Icon(editor.symbol))
                            .width(20)
                            .height(20),
                        text(format!(
                            "{} · auswählen (F6)",
                            symbols::label(editor.symbol)
                        ))
                        .size(13),
                    ]
                    .spacing(8)
                    .align_y(iced::Center),
                )
                .width(Fill)
                .padding(8)
                .on_press_maybe((!editor.recording).then_some(Message::ShowSymbols))
                .style(colors::secondary),
            );
        fields = fields
            .push(text("Fester Shortcut").size(12).color(colors::MUTED))
            .push(
                text(
                    editor
                        .shortcut
                        .as_ref()
                        .map(Shortcut::label)
                        .unwrap_or_else(|| "Kein Shortcut".into()),
                )
                .size(14),
            )
            .push(
                row![
                    button(
                        text(if editor.recording {
                            "Aufnahme abbrechen"
                        } else {
                            "Aufnehmen"
                        })
                        .size(12)
                    )
                    .width(Fill)
                    .on_press(Message::CaptureShortcut),
                    button(text("Entfernen").size(12))
                        .width(Fill)
                        .on_press(Message::ClearShortcut)
                        .style(colors::secondary),
                ]
                .spacing(8),
            )
            .push(
                text(if editor.recording {
                    "Jetzt Tastenkombination drücken. Esc beendet die Aufnahme."
                } else {
                    "Strg, Alt oder Super + Taste; F1–F12 auch ohne. Nur im fokussierten Fenster."
                })
                .size(11)
                .color(colors::MUTED),
            );
        if !matches!(editor.target, EditorTarget::Tab(_)) {
            let selected = TemplateTarget {
                shell: editor.shell,
                distribution: editor.wsl_distribution.clone(),
            };
            let targets = self.template_targets();
            let available = targets.contains(&selected);
            let target: Element<'_, Message> = if editor.recording {
                text(selected.to_string()).size(13).into()
            } else {
                pick_list(targets, Some(selected), Message::EditTemplateTarget)
                    .width(Fill)
                    .text_size(13)
                    .padding(8)
                    .into()
            };
            fields = fields.push(text("Startziel").size(12).color(colors::MUTED)).push(target)
                .push(text("Startverzeichnis").size(12).color(colors::MUTED))
                .push(text_input("Absolutes Startverzeichnis", &editor.directory).style(colors::input).id("template-directory")
                    .on_input_maybe((!editor.recording).then_some(Message::EditDirectory as fn(String) -> Message)).size(13).padding(8))
                .push(text("Startziel und Verzeichnis gelten beim nächsten Prozess. Offene Tabs bleiben erhalten.").size(11).color(colors::MUTED));
            if !available {
                fields = fields.push(text("Gespeichertes Ziel derzeit nicht verfügbar. Es bleibt erhalten; bei Bedarf bewusst ein anderes auswählen.").size(12).color(colors::ERROR));
            }
            fields = fields.push(
                text(if editor.directory_request.is_some() {
                    "Startverzeichnis wird im gewählten WSL-Ziel ermittelt …"
                } else if editor.wsl_distribution.is_some() {
                    "Linux-Pfad in dieser Distribution, z. B. /home/patrick/Projekte."
                } else if cfg!(windows) {
                    "Absoluter Windows-Pfad, z. B. C:\\Repos\\hoori-term."
                } else {
                    "Absoluter Linux-Pfad, z. B. /home/patrick/Projekte."
                })
                .size(11)
                .color(colors::MUTED),
            );
        }
        fields = fields
            .push(text("Darstellung").size(12).color(colors::MUTED))
            .push(
                button(
                    text(if editor.compact {
                        "Kompakt · normal anzeigen (F7)"
                    } else {
                        "Normal · kompakt anzeigen (F7)"
                    })
                    .size(12),
                )
                .width(Fill)
                .on_press_maybe(
                    (!editor.recording).then_some(Message::EditCompact(!editor.compact)),
                )
                .style(colors::secondary),
            )
            .push(
                text(if editor.pinned {
                    "PIN · angepinnt. Über Tabaktionen bewusst entpinnen."
                } else {
                    "Nicht angepinnt. Über Tabaktionen bewusst anpinnen."
                })
                .size(11)
                .color(colors::MUTED),
            );
        if let Some(error) = &self.storage_error {
            fields = fields.push(
                text(format!("Vorlagenspeicherung: {error}"))
                    .size(12)
                    .color(colors::ERROR),
            );
        }
        let mut content = column![
            text(heading).size(17),
            text("Entwurf · Übernehmen oder Abbrechen")
                .size(11)
                .color(colors::MUTED),
            scrollable(fields.width(Fill))
                .spacing(6)
                .id("tab-editor-fields")
                .width(Fill)
                .height(Fill),
        ]
        .spacing(12);
        if let Some(error) = &editor.error {
            // A 4 KiB start path can appear in validation errors. Bound its
            // viewport so the fixed commit/cancel actions cannot be displaced.
            content = content.push(
                scrollable(text(error).size(12).color(colors::ERROR).width(Fill))
                    .spacing(6)
                    .height(64),
            );
        }
        content = content.push(
            row![
                button("Abbrechen")
                    .width(Fill)
                    .on_press(Message::CancelEdit)
                    .style(colors::secondary),
                button("Übernehmen").width(Fill).on_press_maybe(
                    (!editor.recording && editor.directory_request.is_none())
                        .then_some(Message::ApplyEdit)
                ),
            ]
            .spacing(8),
        );
        container(content)
            .padding(16)
            .width(PANEL_WIDTH)
            .height(Fill)
            .style(|_| colors::surface(colors::PANEL))
            .into()
    }
}

fn accent_style(
    theme: &Theme,
    status: button::Status,
    accent: Accent,
    selected: bool,
) -> button::Style {
    let [r, g, b] = accent.rgb();
    let accent = Color::from_rgb8(r, g, b);
    let mut style = colors::secondary(theme, status);
    if status != button::Status::Disabled {
        let strength = if status == button::Status::Hovered {
            0.22
        } else if selected {
            0.16
        } else {
            0.06
        };
        style.background = Some(
            Color {
                r: colors::NAVIGATION.r + accent.r * strength,
                g: colors::NAVIGATION.g + accent.g * strength,
                b: colors::NAVIGATION.b + accent.b * strength,
                a: 1.0,
            }
            .into(),
        );
        style.border = iced::Border {
            color: if selected { colors::TEXT } else { accent },
            width: if selected { 2.0 } else { 1.0 },
            radius: 5.0.into(),
        };
    }
    style
}

impl App {
    fn inspection_view(&self, id: TabId) -> Element<'_, Message> {
        let Some(tab) = self.tabs.get(id) else {
            return container(button("Zurück").on_press(Message::HideInspection)).into();
        };
        let process = match tab.state() {
            ProcessState::Running => "läuft".into(),
            ProcessState::Exited(code) => format!("beendet · Exit {code}"),
            ProcessState::Failed(error) => format!("Startfehler · {error}"),
        };
        let status = &tab.terminal.program_status;
        let index = self
            .tabs
            .items
            .iter()
            .position(|item| item.id == id)
            .unwrap();
        let can_move = |after: bool| {
            let neighbor = if after {
                index.checked_add(1)
            } else {
                index.checked_sub(1)
            };
            neighbor
                .and_then(|index| self.tabs.items.get(index))
                .is_some_and(|item| item.pinned == tab.pinned)
        };
        let action = |label, value| {
            let enabled = match value {
                TabAction::Save => self.templates_loaded,
                TabAction::MoveLeft => can_move(false),
                TabAction::MoveRight => can_move(true),
                _ => true,
            };
            button(text(label).size(12))
                .width(Fill)
                .on_press_maybe(enabled.then_some(Message::InspectAction(id, value)))
                .style(colors::secondary)
        };
        let mut fields = column![
            text(self.inspect_tab(id, false)).size(13).width(Fill),
            row![
                action("Bearbeiten", TabAction::Edit),
                action("Vorlage speichern / bearbeiten", TabAction::Save)
            ]
            .spacing(6),
            row![
                action(
                    if tab.pinned { "Entpinnen" } else { "Anpinnen" },
                    TabAction::Pin
                ),
                action(
                    if tab.identity.compact {
                        "Normalansicht"
                    } else {
                        "Kompaktansicht"
                    },
                    TabAction::Compact
                )
            ]
            .spacing(6),
            row![
                action("← Verschieben", TabAction::MoveLeft),
                action("Verschieben →", TabAction::MoveRight)
            ]
            .spacing(6),
            action(
                if tab.pinned {
                    "Angepinnten Tab bewusst schließen"
                } else {
                    "Tab schließen"
                },
                if tab.pinned {
                    TabAction::ClosePinned
                } else {
                    TabAction::Close
                }
            ),
            text(format!("Buildinformationen\n{}", build_info::metadata()))
                .width(Fill)
                .size(12)
                .color(colors::MUTED),
            text(format!("Titel/Status · {}", tab.description())).size(18),
            text(format!("PTY-Prozess: {process}")).size(13),
            text(format!(
                "Terminaltitel: {}",
                if tab.terminal.shell_title.is_empty() {
                    "(leer)"
                } else {
                    &tab.terminal.shell_title
                }
            ))
            .size(13),
            text(status.summary_label()).size(13),
            text(
                "Vom Programm gemeldete Angaben · keine automatische Aktion · nur für diese Sitzung"
            )
            .size(12),
        ]
        .spacing(8);
        if let Some(error) = &tab.error {
            fields = fields.push(text(format!("PTY-/Eingabefehler: {error}")).size(12));
        }
        if let Some(error) = &self.error {
            fields = fields.push(
                text(format!("Aktion: {error}"))
                    .size(12)
                    .color(colors::ERROR),
            );
        }
        if let Some(error) = &self.storage_error {
            fields = fields.push(
                text(format!("Vorlagenspeicherung: {error}"))
                    .size(12)
                    .color(colors::ERROR),
            );
        }
        let mut records: Vec<_> = status.records().collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        for record in records {
            let source = if record.id.is_empty() {
                "Hauptmeldung"
            } else {
                &record.id
            };
            let mut details = column![
                text(format!(
                    "{source} · CLI: {}{}",
                    record.state.label(),
                    record
                        .kind
                        .map(|k| format!(" · {} benötigt", k.label()))
                        .unwrap_or_default()
                ))
                .size(13)
                .color(tab_strip::status_color(record.state)),
            ]
            .spacing(4);
            if let Some(app) = status.inherited_app(record) {
                details = details.push(text(format!("Gemeldete Programmkennung: {app}")).size(12));
            }
            if let Some(title) = &record.title {
                details = details.push(text(format!("Bezeichnung: {title}")).size(13));
            }
            if let Some(progress) = record.progress {
                details =
                    details.push(text(format!("Gemeldeter Fortschritt: {progress}%")).size(12));
            }
            if let Some(message) = &record.message {
                details = details.push(text(format!("Meldung: {message}")).size(13));
            }
            fields = fields.push(
                container(details)
                    .padding(8)
                    .width(Fill)
                    .style(|_| colors::surface(colors::RAISED)),
            );
        }
        container(
            column![
                scrollable(fields.width(Fill))
                    .width(Fill)
                    .spacing(6)
                    .id("runtime-details")
                    .height(Fill)
                    .on_scroll(|viewport| Message::InspectionScrolled(
                        viewport.relative_offset().y
                    )),
                button("Zurück zum Terminal · Esc / Enter")
                    .width(Fill)
                    .on_press(Message::HideInspection)
                    .style(colors::secondary),
            ]
            .spacing(8),
        )
        .width(PANEL_WIDTH)
        .height(Fill)
        .padding(16)
        .style(|_| colors::surface(colors::PANEL))
        .into()
    }
}
