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

#[derive(Default)]
pub struct State {
    pub width: f32,
    pub offset: f32,
    pub focused: Option<Target>,
    pub hovered: Option<Target>,
}

impl State {
    pub fn layout(&self, tabs: &Tabs) -> Layout {
        Layout::new(self.width, tabs.items.len(), tabs.pinned_count())
    }

    pub fn scroll(&mut self, tabs: &Tabs, delta: f32) {
        self.offset = (self.offset + delta).clamp(0.0, self.layout(tabs).max_offset);
        // Layout/navigation can move the old target without a pointer event.
        self.hovered = None;
    }

    pub fn reveal(&mut self, tabs: &Tabs, id: TabId) {
        if let Some(index) = tabs.items.iter().position(|tab| tab.id == id) {
            let layout = self.layout(tabs);
            let left = layout.tab(index, 0.0).x - layout.viewport.x;
            let right = left + TAB_WIDTH;
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
}

impl Layout {
    pub fn new(width: f32, count: usize, pinned: usize) -> Self {
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
        let group_boundary = (pinned > 0 && pinned < count).then_some(pinned);
        let span = count as f32 * TAB_WIDTH
            + if group_boundary.is_some() {
                GROUP_GAP
            } else {
                0.0
            };
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
        Rectangle::new(
            Point::new(
                self.viewport.x + index as f32 * TAB_WIDTH - offset
                    + if self
                        .group_boundary
                        .is_some_and(|boundary| index >= boundary)
                    {
                        GROUP_GAP
                    } else {
                        0.0
                    },
                8.0,
            ),
            Size::new(TAB_WIDTH, HEIGHT - 8.0),
        )
    }

    fn close(tab: Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(tab.x + TAB_WIDTH - 32.0, 18.0),
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
        }
        if pointer.width != bounds.width {
            pointer.width = bounds.width;
            return Some(canvas::Action::publish(Message::StripWidth(bounds.width)));
        }
        if !self.interactive && !self.window_interactive {
            pointer.pressed = None;
            return None;
        }
        let layout = Layout::new(
            bounds.width,
            self.tabs.items.len(),
            self.tabs.pinned_count(),
        );
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
        match event {
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
        if pointer.pressed == Some(Target::Drag) {
            return mouse::Interaction::Grabbing;
        }
        match cursor
            .position_in(bounds)
            .and_then(|p| {
                Layout::new(
                    bounds.width,
                    self.tabs.items.len(),
                    self.tabs.pinned_count(),
                )
                .hit(self.tabs, self.state.offset, p)
            })
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
        let layout = Layout::new(
            bounds.width,
            self.tabs.items.len(),
            self.tabs.pinned_count(),
        );
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
                let x = layout.viewport.x + boundary as f32 * TAB_WIDTH - self.state.offset
                    + GROUP_GAP / 2.0;
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
                        Size::new(TAB_WIDTH, 2.0),
                        view::BACKGROUND,
                    );
                } else {
                    frame.fill_rectangle(
                        Point::new(rect.x + TAB_WIDTH - 1.0, 20.0),
                        Size::new(1.0, 20.0),
                        Color::from_rgb8(49, 54, 75),
                    );
                }
                let [r, g, b] = tab.identity.accent.rgb();
                frame.fill(
                    &Path::circle(Point::new(rect.x + 15.0, 30.0), 3.5),
                    Color::from_rgb8(r, g, b),
                );
                tab_label(
                    frame,
                    ellipsis(tab.label(), 18),
                    Point::new(rect.x + 28.0, 12.0),
                    14.0,
                    TEXT,
                    layout.viewport,
                );
                let state = if tab.error.is_some() {
                    "Fehler"
                } else {
                    match tab.state() {
                        ProcessState::Running => "",
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
                let mut details = if state.is_empty() {
                    shortcut
                } else if shortcut.is_empty() {
                    state.into()
                } else {
                    format!("{state} · {shortcut}")
                };
                if tab.pinned {
                    details = format!("PIN · {details}");
                }
                tab_label(
                    frame,
                    ellipsis(&details, 24),
                    Point::new(rect.x + 28.0, 32.0),
                    10.0,
                    if state == "Fehler" {
                        Color::from_rgb8(255, 150, 161)
                    } else {
                        MUTED
                    },
                    layout.viewport,
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
                            Size::new(TAB_WIDTH - 8.0, 36.0),
                        ),
                    );
                } else if self.state.focused == Some(Target::Close(tab.id)) {
                    focus(frame, close);
                }
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
) {
    // Each lane is clipped independently; wide/combining Unicode cannot cover
    // the close target, neighbouring tab, or shortcut lane.
    if let Some(clip) =
        viewport.intersection(&Rectangle::new(position, Size::new(160.0, size + 4.0)))
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

fn ellipsis(value: &str, limit: usize) -> String {
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
        let point = Layout::new(1000.0, 0, 0).drag.center();
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
}
