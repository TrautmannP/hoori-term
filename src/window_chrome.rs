//! Native window actions and a narrow resize border, independent of PTY state.
use crate::Message;
use iced::widget::canvas::{self, Geometry};
use iced::{Event, Point, Rectangle, Renderer, Theme, mouse, window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Minimize,
    Maximize,
    Close,
}

impl Control {
    pub const ALL: [Self; 3] = [Self::Minimize, Self::Maximize, Self::Close];

    pub fn label(self, maximized: bool) -> &'static str {
        match self {
            Self::Minimize => "Fenster minimieren",
            Self::Maximize if maximized => "Fenster wiederherstellen",
            Self::Maximize => "Fenster maximieren",
            Self::Close => "Fenster schließen",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Control(Control),
    Drag,
    Resize(Edge),
}

#[derive(Default)]
pub struct State {
    pub id: Option<window::Id>,
    pub maximized: bool,
    pub focused: bool,
    revision: u64,
}

impl State {
    pub fn query(&mut self) -> Option<(window::Id, u64)> {
        let id = self.id?;
        self.revision = self.revision.wrapping_add(1);
        Some((id, self.revision))
    }

    pub fn observed(&mut self, revision: u64, maximized: bool) {
        // A later native resize/focus/query can overtake an earlier receipt.
        if revision == self.revision {
            self.maximized = maximized;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Edge {
    pub fn direction(self) -> window::Direction {
        match self {
            Self::North => window::Direction::North,
            Self::South => window::Direction::South,
            Self::East => window::Direction::East,
            Self::West => window::Direction::West,
            Self::NorthEast => window::Direction::NorthEast,
            Self::NorthWest => window::Direction::NorthWest,
            Self::SouthEast => window::Direction::SouthEast,
            Self::SouthWest => window::Direction::SouthWest,
        }
    }

    fn interaction(self) -> mouse::Interaction {
        match self {
            Self::North | Self::South => mouse::Interaction::ResizingVertically,
            Self::East | Self::West => mouse::Interaction::ResizingHorizontally,
            Self::NorthEast | Self::SouthWest => mouse::Interaction::ResizingDiagonallyUp,
            Self::NorthWest | Self::SouthEast => mouse::Interaction::ResizingDiagonallyDown,
        }
    }

    fn at(bounds: Rectangle, point: Point) -> Option<Self> {
        if !bounds.contains(point) {
            return None;
        }
        let x = point.x - bounds.x;
        let y = point.y - bounds.y;
        let west = x < 6.0;
        let east = x >= bounds.width - 6.0;
        let north = y < 6.0;
        let south = y >= bounds.height - 6.0;
        if !west && !east && !north && !south {
            return None;
        }
        // Corners extend along the border, never into the content's interior.
        Some(if x < 16.0 && y < 16.0 {
            Self::NorthWest
        } else if x >= bounds.width - 16.0 && y < 16.0 {
            Self::NorthEast
        } else if x < 16.0 && y >= bounds.height - 16.0 {
            Self::SouthWest
        } else if x >= bounds.width - 16.0 && y >= bounds.height - 16.0 {
            Self::SouthEast
        } else if north {
            Self::North
        } else if south {
            Self::South
        } else if west {
            Self::West
        } else {
            Self::East
        })
    }
}

pub struct ResizeBorder {
    pub enabled: bool,
}

#[derive(Default)]
pub struct PointerState {
    content_pressed: bool,
}

impl canvas::Program<Message> for ResizeBorder {
    type State = PointerState;

    fn update(
        &self,
        state: &mut PointerState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let edge = self
                    .enabled
                    .then(|| cursor.position().and_then(|p| Edge::at(bounds, p)))
                    .flatten();
                state.content_pressed = edge.is_none();
                edge.map(|edge| {
                    canvas::Action::publish(Message::WindowAction(Action::Resize(edge)))
                        .and_capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Window(window::Event::Focused | window::Event::Unfocused) => {
                state.content_pressed = false;
                None
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        state: &PointerState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if !self.enabled || state.content_pressed {
            return mouse::Interaction::None;
        }
        cursor
            .position()
            .and_then(|p| Edge::at(bounds, p))
            .map(Edge::interaction)
            .unwrap_or_default()
    }

    fn draw(
        &self,
        _: &PointerState,
        _: &Renderer,
        _: &Theme,
        _: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Size;
    use iced::widget::canvas::Program;

    #[test]
    fn resize_starts_only_at_the_border_and_does_not_capture_existing_selection() {
        let bounds = Rectangle::new(Point::new(20.0, 30.0), Size::new(640.0, 700.0));
        for (point, edge) in [
            (Point::new(22.0, 32.0), Edge::NorthWest),
            (Point::new(658.0, 32.0), Edge::NorthEast),
            (Point::new(22.0, 728.0), Edge::SouthWest),
            (Point::new(658.0, 728.0), Edge::SouthEast),
            (Point::new(340.0, 32.0), Edge::North),
            (Point::new(340.0, 728.0), Edge::South),
            (Point::new(22.0, 380.0), Edge::West),
            (Point::new(658.0, 380.0), Edge::East),
        ] {
            assert_eq!(Edge::at(bounds, point), Some(edge));
        }
        assert!(Edge::at(bounds, Point::new(28.0, 38.0)).is_none());
        assert!(Edge::at(bounds, Point::new(19.0, 380.0)).is_none());
        let border = ResizeBorder { enabled: true };
        let mut state = PointerState::default();
        let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        assert!(
            border
                .update(
                    &mut state,
                    &press,
                    bounds,
                    mouse::Cursor::Available(bounds.center())
                )
                .is_none()
        );
        // A selection begun inside must retain the real cursor at the edge.
        let edge_cursor = mouse::Cursor::Available(Point::new(22.0, 380.0));
        assert_eq!(
            border.mouse_interaction(&state, bounds, edge_cursor),
            mouse::Interaction::None
        );
        let _ = border.update(
            &mut state,
            &Event::Window(window::Event::Unfocused),
            bounds,
            edge_cursor,
        );
        assert!(
            border
                .update(&mut state, &press, bounds, edge_cursor)
                .is_some()
        );
        assert!(
            ResizeBorder { enabled: false }
                .update(&mut state, &press, bounds, edge_cursor)
                .is_none()
        );
    }

    #[test]
    fn late_observation_cannot_overwrite_the_latest_native_state() {
        let mut state = State {
            id: Some(window::Id::unique()),
            ..Default::default()
        };
        let (_, old) = state.query().unwrap();
        let (_, current) = state.query().unwrap();
        state.observed(current, true);
        state.observed(old, false);
        assert!(state.maximized);
        let (_, next) = state.query().unwrap();
        assert!(state.maximized); // Requesting is not an optimistic state flip.
        state.observed(next, false);
        assert!(!state.maximized);
    }
}
