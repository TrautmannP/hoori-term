use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Config, Osc52};
use alacritty_terminal::vte::ansi::Processor;
use std::sync::{Arc, Mutex};

pub const HISTORY_LINES: usize = 2000;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct GridSize {
    pub columns: u16,
    pub rows: u16,
}

impl GridSize {
    pub fn new(columns: u16, rows: u16) -> Self {
        Self {
            columns: columns.clamp(2, 500),
            rows: rows.clamp(1, 200),
        }
    }
    pub fn pty(self) -> portable_pty::PtySize {
        portable_pty::PtySize {
            rows: self.rows,
            cols: self.columns,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }
    fn screen_lines(&self) -> usize {
        self.rows as usize
    }
    fn columns(&self) -> usize {
        self.columns as usize
    }
}

#[derive(Clone, Default)]
pub struct Events(Arc<Mutex<Vec<Event>>>);

impl EventListener for Events {
    fn send_event(&self, event: Event) {
        // Bound event amplification within each parse batch. Clipboard/host
        // actions are deliberately absent, including OSC 52 at the core level.
        if matches!(
            event,
            Event::Title(_)
                | Event::ResetTitle
                | Event::PtyWrite(_)
                | Event::TextAreaSizeRequest(_)
        ) {
            let mut events = self.0.lock().unwrap();
            if events.len() < 64 {
                events.push(event);
            }
        }
    }
}

pub struct Terminal {
    pub term: Term<Events>,
    parser: Processor,
    events: Events,
    pub size: GridSize,
    pub shell_title: String,
}

impl Terminal {
    pub fn new(size: GridSize) -> Self {
        let events = Events::default();
        let config = Config {
            scrolling_history: HISTORY_LINES,
            osc52: Osc52::Disabled,
            ..Config::default()
        };
        Self {
            term: Term::new(config, &size, events.clone()),
            parser: Processor::new(),
            events,
            size,
            shell_title: String::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.parser.advance(&mut self.term, bytes);
        let mut replies = Vec::new();
        for event in self.events.0.lock().unwrap().drain(..) {
            match event {
                Event::Title(title) => self.shell_title = crate::build_info::sanitize_title(&title),
                Event::ResetTitle => self.shell_title.clear(),
                Event::PtyWrite(text) => replies.push(text.into_bytes()),
                Event::TextAreaSizeRequest(format) => replies.push(
                    format(WindowSize {
                        num_lines: self.size.rows,
                        num_cols: self.size.columns,
                        cell_width: 10,
                        cell_height: 20,
                    })
                    .into_bytes(),
                ),
                _ => {}
            }
        }
        replies
    }

    pub fn resize(&mut self, size: GridSize) {
        self.size = size;
        self.term.resize(size);
    }

    pub fn scroll(&mut self, lines: i32) {
        self.term.scroll_display(Scroll::Delta(lines));
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }
    pub fn scroll_to_top(&mut self) {
        self.term.scroll_display(Scroll::Top);
    }

    pub fn select(&mut self, row: usize, column: usize, side: Side, start: bool) {
        let offset = self.term.grid().display_offset();
        let point = Point::new(
            Line(row.min(self.size.rows as usize - 1) as i32 - offset as i32),
            Column(column.min(self.size.columns as usize - 1)),
        );
        if start {
            self.term.selection = Some(Selection::new(SelectionType::Simple, point, side));
        } else if let Some(selection) = &mut self.term.selection {
            selection.update(point, side);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::{Column, Line, Point};
    use alacritty_terminal::term::TermMode;

    #[test]
    fn parses_chunked_utf8_colors_cursor_and_alternate_screen() {
        let mut t = Terminal::new(GridSize::new(20, 4));
        let bytes = "\x1b[31mGrüße界\x1b[0m".as_bytes();
        for byte in bytes {
            t.feed(&[*byte]);
        }
        assert_eq!(t.term.grid()[Point::new(Line(0), Column(2))].c, 'ü');
        assert_eq!(t.term.grid()[Point::new(Line(0), Column(5))].c, '界');
        t.feed(b"\x1b[?1049h\x1b[2J\x1b[2;3Halt");
        assert!(t.term.mode().contains(TermMode::ALT_SCREEN));
        assert_eq!(t.term.grid()[Point::new(Line(1), Column(2))].c, 'a');
        t.feed(b"\x1b[?1049l");
        assert!(!t.term.mode().contains(TermMode::ALT_SCREEN));
        assert_eq!(t.term.grid()[Point::new(Line(0), Column(0))].c, 'G');
    }

    #[test]
    fn bounds_history_and_replies_without_executing_host_actions() {
        let mut t = Terminal::new(GridSize::new(20, 4));
        for _ in 0..5000 {
            t.feed(b"line\r\n");
        }
        assert!(t.term.grid().history_size() <= HISTORY_LINES);
        assert!(t.feed(b"\x1b]52;c;c2VjcmV0\x07\x1b]52;c;?\x07").is_empty());
        let replies = t.feed(b"\x1b[6n");
        assert_eq!(replies.len(), 1);
        assert!(replies[0].starts_with(b"\x1b["));
        t.feed(b"\x1b]2;safe title\x07");
        assert_eq!(t.shell_title, "safe title");
    }

    #[test]
    fn scrollback_discards_old_lines_and_selection_uses_visible_coordinates() {
        let mut t = Terminal::new(GridSize::new(40, 4));
        for i in 0..5000 {
            t.feed(format!("line {i:04}\r\n").as_bytes());
        }
        assert_eq!(t.term.grid().history_size(), HISTORY_LINES);
        t.scroll_to_top();
        let top: String = t.term.grid().display_iter().map(|c| c.cell.c).collect();
        assert!(!top.contains("line 0000"));
        assert!(top.contains("line 2997"));
        t.select(0, 0, Side::Left, true);
        t.select(0, 8, Side::Right, false);
        assert_eq!(t.term.selection_to_string().as_deref(), Some("line 2997"));
        t.scroll_to_bottom();
        let bottom: String = t.term.grid().display_iter().map(|c| c.cell.c).collect();
        assert!(bottom.contains("line 4999"));
        assert_eq!(t.term.grid().display_offset(), 0);
    }
}
