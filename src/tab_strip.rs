//! Connected tab chrome. Layout, hit testing and keyboard focus share stable
//! targets; scrolling never changes the tab model or starts a session.
use crate::window_chrome::{Action as WindowAction, Control};
use crate::{Message, view};
use hoori_term::build_info;
use hoori_term::session::ProcessState;
use hoori_term::tabs::{TabId, Tabs};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Event, Font, Point, Rectangle, Renderer, Size, Theme, mouse};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const HEIGHT: f32 = 52.0;
const TAB_WIDTH: f32 = 224.0;
const COMPACT_WIDTH: f32 = 104.0;
const GROUP_GAP: f32 = 12.0;
const BAR: Color = Color::from_rgb8(16, 18, 27);
const HOVER: Color = Color::from_rgb8(31, 34, 48);
const TEXT: Color = Color::from_rgb8(219, 224, 239);
const MUTED: Color = Color::from_rgb8(151, 161, 187);
const FOCUS: Color = Color::from_rgb8(125, 210, 245);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Tab(TabId),
    Close(TabId),
    Previous,
    Next,
    Plus,
    Window(Control),
    Drag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropTarget {
    pub tab: TabId,
    pub after: bool,
}

pub struct TabDrag {
    pub source: TabId,
    pub origin: Point,
    pub moved: bool,
    pub target: Option<DropTarget>,
}

#[derive(Default)]
pub struct State {
    pub width: f32,
    pub offset: f32,
    pub focused: Option<Target>,
    pub hovered: Option<Target>,
    // The app owns this gesture so changing/removing a tooltip cannot reset
    // a held pointer. Targets are cached IDs, never positions on release.
    pub dragging: Option<TabDrag>,
}

impl State {
    pub fn layout(&self, tabs: &Tabs) -> Layout {
        Layout::new(self.width, tabs)
    }

    pub fn scroll(&mut self, tabs: &Tabs, delta: f32) {
        self.offset = (self.offset + delta).clamp(0.0, self.layout(tabs).max_offset);
        // Layout/navigation can move the old target without a pointer event.
        self.hovered = None;
    }

    pub fn reveal(&mut self, tabs: &Tabs, id: TabId) {
        if let Some(index) = tabs.items.iter().position(|tab| tab.id == id) {
            let layout = self.layout(tabs);
            let rect = layout.tab(index, 0.0);
            let left = rect.x - layout.viewport.x;
            let right = left + rect.width;
            if left < self.offset {
                self.offset = left;
            } else if right > self.offset + layout.viewport.width {
                self.offset = right - layout.viewport.width;
            }
            self.scroll(tabs, 0.0);
        }
    }

    pub fn traverse(&mut self, tabs: &Tabs, backwards: bool) {
        let targets: Vec<_> = tabs
            .items
            .iter()
            .flat_map(|tab| [Target::Tab(tab.id), Target::Close(tab.id)])
            .chain([Target::Plus])
            .chain(Control::ALL.map(Target::Window))
            .collect();
        let current = self
            .focused
            .and_then(|t| targets.iter().position(|x| *x == t));
        let index = match (current, backwards) {
            (Some(0) | None, true) => targets.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % targets.len(),
            (None, false) => 0,
        };
        self.focused = Some(targets[index]);
        if let Target::Tab(id) | Target::Close(id) = targets[index] {
            self.reveal(tabs, id);
        }
    }
}

pub struct Layout {
    pub viewport: Rectangle,
    pub max_offset: f32,
    plus: Rectangle,
    drag: Rectangle,
    window: [Rectangle; 3],
    overflow: bool,
    group_boundary: Option<usize>,
    slots: Vec<(f32, f32)>,
}

impl Layout {
    pub fn new(width: f32, tabs: &Tabs) -> Self {
        // The native minimum width is 640. Reserve the same chrome space at
        // every width: one complete tab remains revealable at that minimum.
        let width = width.max(640.0);
        let drag = Rectangle::new(Point::new(width - 268.0, 8.0), Size::new(144.0, 36.0));
        let plus = Rectangle::new(Point::new(drag.x - 48.0, 8.0), Size::new(40.0, 36.0));
        let window = std::array::from_fn(|index| {
            Rectangle::new(
                Point::new(width - 116.0 + index as f32 * 36.0, 8.0),
                Size::new(36.0, 36.0),
            )
        });
        let pinned = tabs.pinned_count();
        let group_boundary = (pinned > 0 && pinned < tabs.items.len()).then_some(pinned);
        let mut span = 0.0;
        let slots = tabs
            .items
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                if group_boundary == Some(index) {
                    span += GROUP_GAP;
                }
                let width = if tab.identity.compact {
                    COMPACT_WIDTH
                } else {
                    TAB_WIDTH
                };
                let slot = (span, width);
                span += width;
                slot
            })
            .collect();
        let overflow = span > drag.x - 64.0;
        let left = if overflow { 68.0 } else { 8.0 };
        let viewport = Rectangle::new(
            Point::new(left, 0.0),
            Size::new((plus.x - 8.0 - left).max(0.0), HEIGHT),
        );
        Self {
            max_offset: (span - viewport.width).max(0.0),
            viewport,
            plus,
            drag,
            window,
            overflow,
            group_boundary,
            slots,
        }
    }

    fn controls(&self) -> Vec<(Target, Rectangle, &'static str)> {
        let mut controls = vec![(Target::Plus, self.plus, "+")];
        controls.extend(
            Control::ALL
                .into_iter()
                .zip(self.window)
                .map(|(control, bounds)| (Target::Window(control), bounds, "")),
        );
        if self.overflow {
            for (target, x, label) in [(Target::Previous, 8.0, "‹"), (Target::Next, 36.0, "›")]
            {
                controls.push((
                    target,
                    Rectangle::new(Point::new(x, 12.0), Size::new(24.0, 28.0)),
                    label,
                ));
            }
        }
        controls
    }

    fn tab(&self, index: usize, offset: f32) -> Rectangle {
        let (left, width) = self.slots[index];
        Rectangle::new(
            Point::new(self.viewport.x + left - offset, 8.0),
            Size::new(width, HEIGHT - 8.0),
        )
    }

    fn close(tab: Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(
                tab.x + tab.width - 32.0,
                if tab.width < TAB_WIDTH { 10.0 } else { 18.0 },
            ),
            Size::new(24.0, 24.0),
        )
    }

    fn hit(&self, tabs: &Tabs, offset: f32, point: Point) -> Option<Target> {
        for (target, bounds, _) in self.controls() {
            if bounds.contains(point) {
                return Some(target);
            }
        }
        if self.viewport.contains(point) {
            for (index, tab) in tabs.items.iter().enumerate() {
                let bounds = self.tab(index, offset);
                if bounds.contains(point) {
                    return Some(if Self::close(bounds).contains(point) {
                        Target::Close(tab.id)
                    } else {
                        Target::Tab(tab.id)
                    });
                }
            }
        }
        self.drag.contains(point).then_some(Target::Drag)
    }

    fn drop_target(
        &self,
        tabs: &Tabs,
        offset: f32,
        source: TabId,
        point: Point,
    ) -> Option<DropTarget> {
        let (Target::Tab(id) | Target::Close(id)) = self.hit(tabs, offset, point)? else {
            return None;
        };
        let from = tabs.get(source)?;
        let index = tabs.items.iter().position(|tab| tab.id == id)?;
        (from.pinned == tabs.items[index].pinned).then_some(DropTarget {
            tab: id,
            after: point.x >= self.tab(index, offset).center_x(),
        })
    }
}

pub struct TabStrip<'a> {
    pub tabs: &'a Tabs,
    pub state: &'a State,
    pub interactive: bool,
    pub window_interactive: bool,
    pub maximized: bool,
}

impl TabStrip<'_> {
    fn enabled(&self, target: Target) -> bool {
        match target {
            Target::Window(_) | Target::Drag => self.window_interactive,
            _ => self.interactive,
        }
    }
}

#[derive(Default)]
pub struct PointerState {
    width: f32,
    pressed: Option<Target>,
    click: Option<(Point, std::time::Instant)>,
}

impl canvas::Program<Message> for TabStrip<'_> {
    type State = PointerState;

    fn update(
        &self,
        pointer: &mut PointerState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            Event::Window(
                iced::window::Event::Unfocused
                    | iced::window::Event::Focused
                    | iced::window::Event::Moved(_)
                    | iced::window::Event::Resized(_)
            ) | Event::Mouse(mouse::Event::CursorLeft)
        ) {
            pointer.pressed = None;
            pointer.click = None;
            if self.state.dragging.is_some() {
                return Some(canvas::Action::publish(Message::CancelTabDrag));
            }
        }
        if pointer.width != bounds.width {
            pointer.width = bounds.width;
            if self.state.width != bounds.width {
                return Some(canvas::Action::publish(Message::StripWidth(bounds.width)));
            }
        }
        if !self.interactive && !self.window_interactive {
            pointer.pressed = None;
            return None;
        }
        let layout = Layout::new(bounds.width, self.tabs);
        let target = cursor
            .position_in(bounds)
            .and_then(|p| layout.hit(self.tabs, self.state.offset, p))
            .filter(|target| self.enabled(*target));
        if pointer
            .pressed
            .is_some_and(|pressed| !self.enabled(pressed))
        {
            pointer.pressed = None;
        }
        if let Some(drag) = &self.state.dragging {
            return match event {
                Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    let point = cursor.position_in(bounds);
                    let moved =
                        drag.moved || point.is_some_and(|point| drag.origin.distance(point) >= 6.0);
                    let target = moved
                        .then(|| {
                            point.and_then(|p| {
                                layout.drop_target(self.tabs, self.state.offset, drag.source, p)
                            })
                        })
                        .flatten();
                    Some(if moved != drag.moved || target != drag.target {
                        canvas::Action::publish(Message::PreviewTabDrag(drag.source, target, moved))
                            .and_capture()
                    } else {
                        canvas::Action::capture()
                    })
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    let valid = if drag.moved {
                        cursor
                            .position_in(bounds)
                            .and_then(|p| {
                                layout.drop_target(self.tabs, self.state.offset, drag.source, p)
                            })
                            .is_some()
                    } else {
                        target == Some(Target::Tab(drag.source))
                    };
                    Some(
                        canvas::Action::publish(Message::FinishTabDrag(drag.source, valid))
                            .and_capture(),
                    )
                }
                Event::Mouse(mouse::Event::WheelScrolled { delta })
                    if cursor
                        .position_in(bounds)
                        .is_some_and(|p| p.x < layout.drag.x) =>
                {
                    let delta = match delta {
                        mouse::ScrollDelta::Lines { x, y } => -(x + y) * 80.0,
                        mouse::ScrollDelta::Pixels { x, y } => -(x + y),
                    };
                    Some(canvas::Action::publish(Message::StripScroll(delta)).and_capture())
                }
                Event::Mouse(mouse::Event::ButtonPressed(_)) => Some(canvas::Action::capture()),
                _ => None,
            };
        }
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
                if self.interactive
                    && matches!(target, Some(Target::Tab(_) | Target::Close(_))) =>
            {
                let Some(Target::Tab(id) | Target::Close(id)) = target else {
                    unreachable!()
                };
                Some(canvas::Action::publish(Message::InspectTab(id)).and_capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if target == Some(Target::Drag) =>
            {
                let point = cursor.position().unwrap();
                let now = std::time::Instant::now();
                let double = pointer.click.is_some_and(|(position, time)| {
                    position.distance(point) < 6.0
                        && now.duration_since(time) <= std::time::Duration::from_millis(300)
                });
                pointer.click = (!double).then_some((point, now));
                pointer.pressed = (!double).then_some(Target::Drag);
                // A simple click must not start a native grab: GNOME/X11
                // emits Unfocused/Focused for it and breaks double-clicks.
                // Start movement as soon as the held pointer moves instead.
                Some(if double {
                    canvas::Action::publish(Message::WindowAction(WindowAction::Control(
                        Control::Maximize,
                    )))
                    .and_capture()
                } else {
                    canvas::Action::capture()
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. })
                if pointer.pressed == Some(Target::Drag) =>
            {
                let moving = pointer.click.is_some_and(|(origin, _)| {
                    cursor.position().is_some_and(|p| origin.distance(p) >= 4.0)
                });
                Some(if moving {
                    pointer.pressed = None;
                    pointer.click = None;
                    canvas::Action::publish(Message::WindowAction(WindowAction::Drag)).and_capture()
                } else {
                    canvas::Action::capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if matches!(target, Some(Target::Tab(_))) =>
            {
                let Some(Target::Tab(source)) = target else {
                    unreachable!()
                };
                pointer.pressed = None;
                pointer.click = None;
                Some(
                    canvas::Action::publish(Message::BeginTabDrag(
                        source,
                        cursor.position_in(bounds).unwrap(),
                    ))
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if target.is_some() => {
                pointer.pressed = target;
                pointer.click = None;
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if pointer.pressed.is_some() =>
            {
                let pressed = pointer.pressed.take();
                Some(if pressed == target && pressed != Some(Target::Drag) {
                    canvas::Action::publish(match target.unwrap() {
                        Target::Window(control) => {
                            Message::WindowAction(WindowAction::Control(control))
                        }
                        target => Message::Strip(target),
                    })
                    .and_capture()
                } else {
                    canvas::Action::capture()
                })
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if self.interactive
                    && cursor
                        .position_in(bounds)
                        .is_some_and(|p| p.x < layout.drag.x) =>
            {
                let delta = match delta {
                    mouse::ScrollDelta::Lines { x, y } => -(x + y) * 80.0,
                    mouse::ScrollDelta::Pixels { x, y } => -(x + y),
                };
                Some(canvas::Action::publish(Message::StripScroll(delta)).and_capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
                if target != self.state.hovered =>
            {
                Some(canvas::Action::publish(Message::StripHover(target)))
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        pointer: &PointerState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if pointer.pressed == Some(Target::Drag) || self.state.dragging.is_some() {
            return mouse::Interaction::Grabbing;
        }
        match cursor
            .position_in(bounds)
            .and_then(|p| Layout::new(bounds.width, self.tabs).hit(self.tabs, self.state.offset, p))
            .filter(|target| self.enabled(*target))
        {
            Some(Target::Drag) => mouse::Interaction::Grab,
            Some(_) => mouse::Interaction::Pointer,
            None => mouse::Interaction::default(),
        }
    }

    fn draw(
        &self,
        _: &PointerState,
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let layout = Layout::new(bounds.width, self.tabs);
        let hover = cursor
            .position_in(bounds)
            .and_then(|p| layout.hit(self.tabs, self.state.offset, p))
            .filter(|target| self.enabled(*target));
        // Finish the background in its own mesh before adding clipped tabs.
        // Frame's current geometry buffer is otherwise appended after drafts.
        frame.with_clip(Rectangle::new(Point::ORIGIN, bounds.size()), |background| {
            background.fill_rectangle(Point::ORIGIN, bounds.size(), BAR);
            background.fill_rectangle(
                Point::new(0.0, HEIGHT - 1.0),
                Size::new(bounds.width, 1.0),
                Color::from_rgb8(49, 54, 75),
            );
        });
        frame.with_clip(layout.viewport, |frame| {
            if let Some(boundary) = layout.group_boundary {
                let x = layout.tab(boundary, self.state.offset).x - GROUP_GAP / 2.0;
                frame.fill_rectangle(Point::new(x - 1.0, 12.0), Size::new(2.0, 32.0), MUTED);
            }
            for (index, tab) in self.tabs.items.iter().enumerate() {
                let rect = layout.tab(index, self.state.offset);
                if rect.x + rect.width <= layout.viewport.x
                    || rect.x >= layout.viewport.x + layout.viewport.width
                {
                    continue;
                }
                let target = Target::Tab(tab.id);
                let active = self.tabs.active == Some(tab.id);
                let hovered = hover == Some(target) || hover == Some(Target::Close(tab.id));
                let shape = Path::rounded_rectangle(
                    rect.position(),
                    rect.size(),
                    iced::border::Radius {
                        top_left: 10.0,
                        top_right: 10.0,
                        ..Default::default()
                    },
                );
                frame.fill(
                    &shape,
                    if active {
                        view::BACKGROUND
                    } else if hovered {
                        HOVER
                    } else if tab.pinned {
                        Color::from_rgb8(24, 28, 41)
                    } else {
                        BAR
                    },
                );
                if active {
                    frame.stroke(
                        &shape,
                        Stroke::default()
                            .with_width(1.0)
                            .with_color(Color::from_rgb8(76, 84, 112)),
                    );
                    // Open the bottom border into the terminal's surface.
                    frame.fill_rectangle(
                        Point::new(rect.x, HEIGHT - 2.0),
                        Size::new(rect.width, 2.0),
                        view::BACKGROUND,
                    );
                } else {
                    frame.fill_rectangle(
                        Point::new(rect.x + rect.width - 1.0, 20.0),
                        Size::new(1.0, 20.0),
                        Color::from_rgb8(49, 54, 75),
                    );
                }
                let [r, g, b] = tab.identity.accent.rgb();
                frame.fill(
                    &Path::circle(Point::new(rect.x + 15.0, 30.0), 3.5),
                    Color::from_rgb8(r, g, b),
                );
                if let Some(symbol) = tab.identity.display_symbol() {
                    crate::symbols_view::draw(
                        frame,
                        Some(symbol),
                        Point::new(rect.x + 28.0, if tab.identity.compact { 12.0 } else { 8.0 }),
                        16.0,
                        TEXT,
                    );
                }
                let name_offset = if tab.identity.symbol.is_some() {
                    50.0
                } else {
                    28.0
                };
                if !tab.identity.compact {
                    tab_label(
                        frame,
                        ellipsis(
                            tab.label(),
                            if tab.identity.symbol.is_some() {
                                16
                            } else {
                                18
                            },
                        ),
                        Point::new(rect.x + name_offset, 7.0),
                        13.0,
                        TEXT,
                        layout.viewport,
                        188.0 - name_offset,
                    );
                }
                let state = if tab.error.is_some() {
                    "Fehler"
                } else {
                    match tab.state() {
                        ProcessState::Running => "läuft",
                        ProcessState::Exited(_) => "beendet",
                        ProcessState::Failed(_) => "Fehler",
                    }
                };
                let shortcut = tab
                    .identity
                    .shortcut
                    .as_ref()
                    .map(|s| s.label())
                    .unwrap_or_default();
                let mut details = if tab.identity.compact || shortcut.is_empty() {
                    format!("PTY {state}")
                } else {
                    format!("{shortcut} · PTY {state}")
                };
                if tab.pinned {
                    details = format!("PIN · {details}");
                }
                let program = &tab.terminal.program_status;
                let reported = program.summary();
                let cli = match (program.attention_counts(), reported) {
                    ((waiting, errors), _) if waiting > 0 && errors > 0 => {
                        "CLI: Fehler/Wartet".into()
                    }
                    (_, Some(record)) => format!("CLI: {}", record.state.label()),
                    _ => "CLI: —".into(),
                };
                if !tab.identity.compact {
                    let title = if tab.terminal.shell_title.is_empty() {
                        "Titel: —".into()
                    } else {
                        format!("Titel: {}", tab.terminal.shell_title)
                    };
                    tab_label(
                        frame,
                        ellipsis(&title, 31),
                        Point::new(rect.x + 28.0, 25.0),
                        10.0,
                        MUTED,
                        layout.viewport,
                        160.0,
                    );
                }
                tab_label(
                    frame,
                    ellipsis(&details, if tab.identity.compact { 14 } else { 24 }),
                    Point::new(
                        rect.x + if tab.identity.compact { 8.0 } else { 28.0 },
                        if tab.identity.compact { 46.0 } else { 43.0 },
                    ),
                    9.0,
                    if state == "Fehler" {
                        Color::from_rgb8(255, 150, 161)
                    } else {
                        MUTED
                    },
                    layout.viewport,
                    if tab.identity.compact {
                        rect.width - 16.0
                    } else {
                        100.0
                    },
                );
                tab_label(
                    frame,
                    cli,
                    Point::new(
                        rect.x + if tab.identity.compact { 8.0 } else { 132.0 },
                        if tab.identity.compact { 32.0 } else { 43.0 },
                    ),
                    9.0,
                    reported.map(|r| status_color(r.state)).unwrap_or(MUTED),
                    layout.viewport,
                    if tab.identity.compact {
                        rect.width - 16.0
                    } else {
                        84.0
                    },
                );
                let close = Layout::close(rect);
                if hover == Some(Target::Close(tab.id)) {
                    frame.fill(
                        &Path::circle(close.center(), 12.0),
                        Color::from_rgb8(65, 48, 62),
                    );
                }
                label(
                    frame,
                    "×".into(),
                    Point::new(close.x + 6.0, close.y),
                    18.0,
                    if hovered { TEXT } else { MUTED },
                );
                if self.state.focused == Some(target) {
                    focus(
                        frame,
                        Rectangle::new(
                            Point::new(rect.x + 4.0, 12.0),
                            Size::new(rect.width - 8.0, 36.0),
                        ),
                    );
                } else if self.state.focused == Some(Target::Close(tab.id)) {
                    focus(frame, close);
                }
                if self
                    .state
                    .dragging
                    .as_ref()
                    .is_some_and(|drag| drag.source == tab.id)
                {
                    focus(
                        frame,
                        Rectangle::new(
                            Point::new(rect.x + 2.0, 10.0),
                            Size::new(rect.width - 4.0, 40.0),
                        ),
                    );
                }
            }
            if let Some(drop) = self.state.dragging.as_ref().and_then(|drag| drag.target)
                && let Some(index) = self.tabs.items.iter().position(|tab| tab.id == drop.tab)
            {
                let rect = layout.tab(index, self.state.offset);
                let x = if drop.after {
                    rect.x + rect.width - 2.0
                } else {
                    rect.x
                };
                frame.fill_rectangle(Point::new(x, 10.0), Size::new(3.0, 40.0), FOCUS);
            }
        });
        for (target, rect, content) in layout.controls() {
            let disabled = !self.enabled(target)
                || match target {
                    Target::Previous => self.state.offset <= 0.0,
                    Target::Next => self.state.offset >= layout.max_offset,
                    _ => false,
                };
            if hover == Some(target) && !disabled {
                frame.fill(
                    &Path::rounded_rectangle(rect.position(), rect.size(), 8.0.into()),
                    if target == Target::Window(Control::Close) {
                        Color::from_rgb8(112, 44, 60)
                    } else {
                        HOVER
                    },
                );
            }
            let color = if disabled {
                Color::from_rgb8(69, 77, 102)
            } else {
                TEXT
            };
            if let Target::Window(control) = target {
                window_icon(&mut frame, control, rect.center(), self.maximized, color);
            } else {
                label(
                    &mut frame,
                    content.into(),
                    Point::new(rect.center_x() - 6.0, rect.center_y() - 14.0),
                    22.0,
                    color,
                );
            }
            if self.state.focused == Some(target) {
                focus(&mut frame, rect);
            }
        }
        if hover == Some(Target::Drag) {
            frame.fill(
                &Path::rounded_rectangle(layout.drag.position(), layout.drag.size(), 6.0.into()),
                HOVER,
            );
        }
        for x in [layout.drag.x + 10.0, layout.drag.x + 14.0] {
            for y in [22.0, 27.0, 32.0] {
                frame.fill(&Path::circle(Point::new(x, y), 0.8), MUTED);
            }
        }
        label(
            &mut frame,
            format!(
                "[{}{}]",
                &build_info::COMMIT[..7],
                if build_info::DIRTY { " dirty" } else { "" }
            ),
            Point::new(layout.drag.x + 26.0, 18.0),
            12.0,
            TEXT,
        );
        vec![frame.into_geometry()]
    }
}

fn window_icon(frame: &mut Frame, control: Control, center: Point, maximized: bool, color: Color) {
    let stroke = Stroke::default().with_width(1.3).with_color(color);
    match control {
        Control::Minimize => frame.stroke(
            &Path::line(
                Point::new(center.x - 6.0, center.y + 4.0),
                Point::new(center.x + 6.0, center.y + 4.0),
            ),
            stroke,
        ),
        Control::Maximize => {
            if maximized {
                frame.stroke(
                    &Path::rectangle(
                        Point::new(center.x - 3.0, center.y - 7.0),
                        Size::new(11.0, 10.0),
                    ),
                    stroke,
                );
                frame.fill_rectangle(
                    Point::new(center.x - 7.0, center.y - 3.0),
                    Size::new(11.0, 10.0),
                    BAR,
                );
                frame.stroke(
                    &Path::rectangle(
                        Point::new(center.x - 7.0, center.y - 3.0),
                        Size::new(11.0, 10.0),
                    ),
                    stroke,
                );
            } else {
                frame.stroke(
                    &Path::rectangle(
                        Point::new(center.x - 6.0, center.y - 5.0),
                        Size::new(12.0, 10.0),
                    ),
                    stroke,
                );
            }
        }
        Control::Close => {
            frame.stroke(
                &Path::line(
                    Point::new(center.x - 5.0, center.y - 5.0),
                    Point::new(center.x + 5.0, center.y + 5.0),
                ),
                stroke,
            );
            frame.stroke(
                &Path::line(
                    Point::new(center.x + 5.0, center.y - 5.0),
                    Point::new(center.x - 5.0, center.y + 5.0),
                ),
                stroke,
            );
        }
    }
}

fn tab_label(
    frame: &mut Frame,
    content: String,
    position: Point,
    size: f32,
    color: Color,
    viewport: Rectangle,
    lane_width: f32,
) {
    // Each lane is clipped independently; wide/combining Unicode cannot cover
    // the close target, neighbouring tab, or shortcut lane.
    if let Some(clip) =
        viewport.intersection(&Rectangle::new(position, Size::new(lane_width, size + 4.0)))
    {
        frame.with_clip(clip, |frame| label(frame, content, position, size, color));
    }
}

fn label(frame: &mut Frame, content: String, position: Point, size: f32, color: Color) {
    frame.fill_text(canvas::Text {
        content,
        position,
        size: size.into(),
        color,
        font: Font::MONOSPACE,
        ..Default::default()
    });
}

pub(crate) fn status_color(state: hoori_term::program_status::State) -> Color {
    use hoori_term::program_status::State;
    match state {
        State::Idle => MUTED,
        State::Working => Color::from_rgb8(95, 210, 230),
        State::Done => Color::from_rgb8(145, 215, 145),
        State::Blocked => Color::from_rgb8(250, 205, 105),
        State::Error => Color::from_rgb8(255, 150, 161),
    }
}

pub(crate) fn ellipsis(value: &str, limit: usize) -> String {
    if value.width() <= limit {
        return value.into();
    }
    let mut cells = 0;
    value
        .chars()
        .take_while(|c| {
            cells += c.width().unwrap_or(0);
            cells < limit
        })
        .chain(['…'])
        .collect()
}

fn focus(frame: &mut Frame, rect: Rectangle) {
    frame.stroke(
        &Path::rounded_rectangle(rect.position(), rect.size(), 6.0.into()),
        Stroke::default().with_color(FOCUS).with_width(1.5),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use hoori_term::tabs::Shell;
    use hoori_term::terminal::GridSize;
    use iced::widget::canvas::Program;

    #[test]
    fn header_clicks_allow_double_click_and_only_held_motion_requests_drag() {
        let tabs = Tabs::default();
        let state = State::default();
        let strip = TabStrip {
            tabs: &tabs,
            state: &state,
            interactive: true,
            window_interactive: true,
            maximized: false,
        };
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, HEIGHT));
        let point = Layout::new(1000.0, &tabs).drag.center();
        let cursor = mouse::Cursor::Available(point);
        let mut pointer = PointerState {
            width: 1000.0,
            ..Default::default()
        };
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        assert!(
            strip
                .update(&mut pointer, &press, bounds, cursor)
                .unwrap()
                .into_inner()
                .0
                .is_none()
        );
        assert!(
            strip
                .update(&mut pointer, &release, bounds, cursor)
                .unwrap()
                .into_inner()
                .0
                .is_none()
        );
        // A click alone must not issue drag and trigger GNOME's native focus
        // grab before the next click can be recognized.
        pointer.click = Some((point, std::time::Instant::now()));
        assert!(matches!(
            strip
                .update(&mut pointer, &press, bounds, cursor)
                .unwrap()
                .into_inner()
                .0,
            Some(Message::WindowAction(WindowAction::Control(
                Control::Maximize
            )))
        ));
        let _ = strip.update(&mut pointer, &release, bounds, cursor);
        let _ = strip.update(&mut pointer, &press, bounds, cursor);
        let moved = Point::new(point.x + 8.0, point.y);
        let motion = Event::Mouse(mouse::Event::CursorMoved { position: moved });
        assert!(matches!(
            strip
                .update(
                    &mut pointer,
                    &motion,
                    bounds,
                    mouse::Cursor::Available(moved)
                )
                .unwrap()
                .into_inner()
                .0,
            Some(Message::WindowAction(WindowAction::Drag))
        ));
        assert!(pointer.click.is_none() && pointer.pressed.is_none());
        let _ = strip.update(&mut pointer, &press, bounds, cursor);
        let _ = strip.update(
            &mut pointer,
            &Event::Window(iced::window::Event::Unfocused),
            bounds,
            cursor,
        );
        assert!(pointer.click.is_none() && pointer.pressed.is_none());
        let action = strip.update(
            &mut pointer,
            &motion,
            bounds,
            mouse::Cursor::Available(moved),
        );
        assert!(!matches!(
            action.and_then(|a| a.into_inner().0),
            Some(Message::WindowAction(_))
        ));
    }

    #[test]
    fn overflow_hit_targets_and_reveal_use_stable_identity() {
        let mut tabs = Tabs::default();
        let shell = Shell {
            program: "/nonexistent/hoo19-layout-fixture".into(),
            args: vec![],
            directory: None,
        };
        for _ in 0..12 {
            tabs.open(&shell, GridSize::new(80, 24)).unwrap();
        }
        let target = tabs.items[10].id;
        for width in [640.0, 1280.0] {
            let mut state = State {
                width,
                ..Default::default()
            };
            state.reveal(&tabs, target);
            let layout = state.layout(&tabs);
            assert!(layout.viewport.width >= TAB_WIDTH);
            assert!(layout.viewport.x + layout.viewport.width <= layout.plus.x);
            assert!(layout.plus.x + layout.plus.width < layout.drag.x);
            assert!(layout.drag.x + layout.drag.width < layout.window[0].x);
            for (control, bounds) in Control::ALL.into_iter().zip(layout.window) {
                assert_eq!(
                    layout.hit(&tabs, state.offset, bounds.center()),
                    Some(Target::Window(control))
                );
            }
            assert_eq!(
                layout.hit(&tabs, state.offset, layout.drag.center()),
                Some(Target::Drag)
            );
            let tab = layout.tab(10, state.offset);
            assert!(
                tab.x >= layout.viewport.x
                    && tab.x + tab.width <= layout.viewport.x + layout.viewport.width
            );
            assert_eq!(
                layout.hit(&tabs, state.offset, tab.center()),
                Some(Target::Tab(target))
            );
            assert_eq!(
                layout.hit(&tabs, state.offset, Layout::close(tab).center()),
                Some(Target::Close(target))
            );
            assert_eq!(
                layout.hit(&tabs, state.offset, layout.plus.center()),
                Some(Target::Plus)
            );
            tabs.items.swap(0, 10);
            state.reveal(&tabs, target);
            assert_eq!(
                layout.hit(&tabs, state.offset, layout.tab(0, state.offset).center()),
                Some(Target::Tab(target))
            );
            tabs.items.swap(0, 10);
            state.focused = Some(Target::Tab(target));
            state.traverse(&tabs, false);
            assert_eq!(state.focused, Some(Target::Close(target)));
            state.scroll(&tabs, f32::MAX);
            assert_eq!(state.offset, layout.max_offset);
            state.scroll(&tabs, -f32::MAX);
            assert_eq!(state.offset, 0.0);
            state.focused = Some(Target::Tab(tabs.items[0].id));
            for control in Control::ALL.into_iter().rev() {
                state.traverse(&tabs, true);
                assert_eq!(state.focused, Some(Target::Window(control)));
            }
            state.traverse(&tabs, true);
            assert_eq!(state.focused, Some(Target::Plus));
        }
        tabs.set_pinned(target, true).unwrap();
        tabs.set_pinned(tabs.items[5].id, true).unwrap();
        for width in [640.0, 1280.0] {
            let mut state = State {
                width,
                ..Default::default()
            };
            let layout = state.layout(&tabs);
            assert_eq!(layout.group_boundary, Some(2));
            assert_eq!(
                layout.tab(2, 0.0).x - layout.tab(1, 0.0).x,
                TAB_WIDTH + GROUP_GAP
            );
            for tab in &tabs.items {
                state.reveal(&tabs, tab.id);
                let index = tabs.items.iter().position(|t| t.id == tab.id).unwrap();
                let rect = layout.tab(index, state.offset);
                assert!(
                    rect.x >= layout.viewport.x
                        && rect.x + rect.width <= layout.viewport.x + layout.viewport.width
                );
                assert_eq!(
                    layout.hit(&tabs, state.offset, rect.center()),
                    Some(Target::Tab(tab.id))
                );
            }
        }
    }

    #[test]
    fn mixed_widths_reveal_and_hit_each_identity_without_changing_pins_or_presentation() {
        let mut tabs = Tabs::default();
        let shell = Shell {
            program: "/nonexistent/hoo24-layout-fixture".into(),
            args: vec![],
            directory: None,
        };
        for index in 0..12 {
            let id = tabs.open(&shell, GridSize::new(80, 24)).unwrap();
            tabs.get_mut(id).unwrap().identity.compact = index % 3 != 0;
            if index < 3 {
                tabs.set_pinned(id, true).unwrap();
            }
        }
        let identities: Vec<_> = tabs
            .items
            .iter()
            .map(|tab| (tab.id, tab.identity.clone(), tab.pinned))
            .collect();
        for width in [640.0, 1280.0] {
            let mut state = State {
                width,
                ..Default::default()
            };
            let layout = state.layout(&tabs);
            assert!(layout.overflow);
            let normal_span = 12.0 * TAB_WIDTH + GROUP_GAP;
            assert!(layout.max_offset + layout.viewport.width < normal_span);
            assert_eq!(
                layout.tab(3, 0.0).x - (layout.tab(2, 0.0).x + layout.tab(2, 0.0).width),
                GROUP_GAP
            );
            for (index, tab) in tabs.items.iter().enumerate() {
                state.reveal(&tabs, tab.id);
                let rect = layout.tab(index, state.offset);
                assert_eq!(
                    rect.width,
                    if tab.identity.compact {
                        COMPACT_WIDTH
                    } else {
                        TAB_WIDTH
                    }
                );
                assert!(
                    rect.x >= layout.viewport.x
                        && rect.x + rect.width <= layout.viewport.x + layout.viewport.width
                );
                assert_eq!(
                    layout.hit(&tabs, state.offset, rect.center()),
                    Some(Target::Tab(tab.id))
                );
                let close = Layout::close(rect);
                assert!(rect.contains(close.center()));
                assert_eq!(
                    layout.hit(&tabs, state.offset, close.center()),
                    Some(Target::Close(tab.id))
                );
                assert!(!close.contains(Point::new(rect.x + 36.0, 20.0))); // Symbol stays a distinct activation target.
                state.focused = Some(Target::Tab(tab.id));
                state.traverse(&tabs, false);
                assert_eq!(state.focused, Some(Target::Close(tab.id)));
            }
            state.scroll(&tabs, f32::MAX);
            assert_eq!(state.offset, layout.max_offset);
            assert_eq!(
                layout.hit(&tabs, state.offset, layout.plus.center()),
                Some(Target::Plus)
            );
        }
        assert_eq!(
            tabs.items
                .iter()
                .map(|tab| (tab.id, tab.identity.clone(), tab.pinned))
                .collect::<Vec<_>>(),
            identities
        );
        // A deliberate width change before an overflowed target requires fresh
        // geometry. Reveal and hit testing still agree after that change.
        let target = tabs.items[11].id;
        tabs.items[0].identity.compact = true;
        let mut state = State {
            width: 640.0,
            ..Default::default()
        };
        state.reveal(&tabs, target);
        let layout = state.layout(&tabs);
        assert_eq!(
            layout.hit(&tabs, state.offset, layout.tab(11, state.offset).center()),
            Some(Target::Tab(target))
        );
    }

    #[test]
    fn drag_geometry_uses_stable_targets_and_gestures_survive_tooltip_reset_and_overflow() {
        let mut tabs = Tabs::default();
        let shell = Shell {
            program: "/nonexistent/hoo26-drag".into(),
            args: vec![],
            directory: None,
        };
        let ids: Vec<_> = (0..12)
            .map(|index| {
                let id = tabs.open(&shell, GridSize::new(80, 24)).unwrap();
                tabs.get_mut(id).unwrap().identity.compact = index % 2 == 0;
                if index < 3 {
                    tabs.set_pinned(id, true).unwrap();
                }
                id
            })
            .collect();
        for width in [640.0, 1280.0] {
            let mut state = State {
                width,
                ..Default::default()
            };
            let layout = state.layout(&tabs);
            for (source, target) in [(ids[0], ids[2]), (ids[4], ids[11])] {
                state.reveal(&tabs, target);
                let index = tabs.items.iter().position(|tab| tab.id == target).unwrap();
                let rect = layout.tab(index, state.offset);
                for (x, after) in [(rect.x + 12.0, false), (rect.x + rect.width - 12.0, true)] {
                    assert_eq!(
                        layout.drop_target(&tabs, state.offset, source, Point::new(x, 35.0)),
                        Some(DropTarget { tab: target, after })
                    );
                }
                let other_group = if source == ids[0] { ids[4] } else { ids[0] };
                assert!(
                    layout
                        .drop_target(&tabs, state.offset, other_group, rect.center())
                        .is_none()
                );
            }
            for (_, rect, _) in layout.controls() {
                assert!(
                    layout
                        .drop_target(&tabs, state.offset, ids[4], rect.center())
                        .is_none()
                );
            }
            assert!(
                layout
                    .drop_target(&tabs, state.offset, ids[4], Point::new(100.0, 70.0))
                    .is_none()
            );
            state.reveal(&tabs, ids[0]);
            let origin = Point::new(layout.tab(0, state.offset).x + 16.0, 28.0);
            let bounds = Rectangle::new(Point::ORIGIN, Size::new(width, HEIGHT));
            let mut pointer = PointerState {
                width,
                ..Default::default()
            };
            let strip = TabStrip {
                tabs: &tabs,
                state: &state,
                interactive: true,
                window_interactive: true,
                maximized: false,
            };
            let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
            let message = strip
                .update(
                    &mut pointer,
                    &press,
                    bounds,
                    mouse::Cursor::Available(origin),
                )
                .unwrap()
                .into_inner()
                .0;
            assert!(
                matches!(message,Some(Message::BeginTabDrag(id,point)) if id == ids[0] && point == origin)
            );
            assert!(pointer.pressed.is_none()); // Escape cannot later turn a release into an activation.
            state.dragging = Some(TabDrag {
                source: ids[0],
                origin,
                moved: false,
                target: None,
            });
            // Removing an already-open tooltip resets the canvas-local state.
            pointer = PointerState::default();
            let strip = TabStrip {
                tabs: &tabs,
                state: &state,
                interactive: true,
                window_interactive: true,
                maximized: false,
            };
            let motion = Event::Mouse(mouse::Event::CursorMoved { position: origin });
            let action = strip
                .update(
                    &mut pointer,
                    &motion,
                    bounds,
                    mouse::Cursor::Available(Point::new(origin.x + 2.0, origin.y)),
                )
                .unwrap()
                .into_inner();
            assert!(action.0.is_none()); // Below threshold, no preview and no width/cancel message.
            let point = Point::new(origin.x + 8.0, origin.y);
            assert!(
                matches!(strip.update(&mut pointer,&motion,bounds,mouse::Cursor::Available(point)).unwrap().into_inner().0,Some(Message::PreviewTabDrag(id,Some(DropTarget { tab, after:false }),true)) if id == ids[0] && tab == ids[0])
            );
            assert!(
                matches!(strip.update(&mut pointer,&Event::Mouse(mouse::Event::WheelScrolled { delta:mouse::ScrollDelta::Lines { x:0.0,y:-3.0 } }),bounds,mouse::Cursor::Available(origin)).unwrap().into_inner().0,Some(Message::StripScroll(delta)) if delta == 240.0)
            );
            assert!(matches!(
                strip
                    .update(
                        &mut pointer,
                        &Event::Window(iced::window::Event::Unfocused),
                        bounds,
                        mouse::Cursor::Available(origin)
                    )
                    .unwrap()
                    .into_inner()
                    .0,
                Some(Message::CancelTabDrag)
            ));
            // A cached ID is retained even if its old slot now contains a
            // different tab. The app will reject it when applying the drop.
            let cached = DropTarget {
                tab: ids[1],
                after: true,
            };
            state.dragging = Some(TabDrag {
                source: ids[0],
                origin,
                moved: true,
                target: Some(cached),
            });
            let strip = TabStrip {
                tabs: &tabs,
                state: &state,
                interactive: true,
                window_interactive: true,
                maximized: false,
            };
            assert!(
                matches!(strip.update(&mut pointer,&Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),bounds,mouse::Cursor::Available(Point::new(100.0,70.0))).unwrap().into_inner().0,Some(Message::FinishTabDrag(id,false)) if id == ids[0])
            );
            assert_eq!(state.dragging.as_ref().unwrap().target, Some(cached));
        }
    }
}
