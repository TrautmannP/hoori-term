use crate::control_stream::{Control, ControlStream};
use crate::program_status::{CAPABILITIES, ProgramStatus};
use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::{Config, Osc52};
use alacritty_terminal::vte::ansi::{Handler, Processor};
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

#[derive(Default)]
struct PendingEvents {
    title: Option<Option<String>>,
    replies: Vec<Event>,
}

#[derive(Clone, Default)]
pub struct Events(Arc<Mutex<PendingEvents>>);

impl EventListener for Events {
    fn send_event(&self, event: Event) {
        // Bound event amplification within each parse batch. Clipboard/host
        // actions are deliberately absent, including OSC 52 at the core level.
        let mut events = self.0.lock().unwrap();
        match event {
            // A title flood must retain its last legal update, independently
            // of the bounded reply queue. It never generates one UI event per
            // report: all output is already coalesced by the PTY subscription.
            Event::Title(title) => {
                events.title = Some(Some(crate::build_info::sanitize_display_text(&title)))
            }
            Event::ResetTitle => events.title = Some(None),
            Event::PtyWrite(_) | Event::TextAreaSizeRequest(_) if events.replies.len() < 64 => {
                events.replies.push(event);
            }
            _ => {}
        }
    }
}

impl Events {
    fn drain(&self, size: GridSize, title: &mut String, replies: &mut Vec<Vec<u8>>) {
        let mut pending = self.0.lock().unwrap();
        if let Some(value) = pending.title.take() {
            *title = value.unwrap_or_default();
        }
        for event in pending.replies.drain(..) {
            if replies.len() >= 64 {
                continue;
            }
            match event {
                Event::PtyWrite(text) => replies.push(text.into_bytes()),
                Event::TextAreaSizeRequest(format) => replies.push(
                    format(WindowSize {
                        num_lines: size.rows,
                        num_cols: size.columns,
                        cell_width: 10,
                        cell_height: 20,
                    })
                    .into_bytes(),
                ),
                _ => {}
            }
        }
    }
}

pub struct Terminal {
    pub term: Term<Events>,
    parser: Processor,
    control: ControlStream,
    events: Events,
    pub size: GridSize,
    pub shell_title: String,
    pub program_status: ProgramStatus,
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
            control: ControlStream::default(),
            events,
            size,
            shell_title: String::new(),
            program_status: ProgramStatus::default(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut replies = Vec::new();
        self.control.advance(bytes, |control| match control {
            Control::Core(bytes) => self.parser.advance(&mut self.term, bytes),
            Control::Reset => {
                self.program_status.clear();
                // alacritty's RIS resets its title internally without sending
                // ResetTitle. Publish that reset after any earlier title event.
                self.term.set_title(None);
            }
            Control::Osc(body, bell) => {
                if let Some(status) = body.strip_prefix(b"7501;") {
                    // Preserve query/device-attributes reply ordering. Claude
                    // uses the latter to detect an unsupported terminal.
                    self.events
                        .drain(self.size, &mut self.shell_title, &mut replies);
                    if status == b"?" {
                        if replies.len() < 64 {
                            replies.push(CAPABILITIES.to_vec());
                        }
                    } else {
                        self.program_status.apply(status);
                    }
                } else if body == b"133;A" || body.starts_with(b"133;A;") {
                    self.program_status.prompt_or_exit();
                } else if let Some(title) = body
                    .strip_prefix(b"0;")
                    .or_else(|| body.strip_prefix(b"2;"))
                {
                    if let Ok(title) = std::str::from_utf8(title) {
                        // Retain the full bounded title, including semicolons;
                        // the core's generic OSC parameter cap must not cut it.
                        self.term
                            .set_title(Some(crate::build_info::sanitize_display_text(title)));
                    }
                } else {
                    self.parser.advance(&mut self.term, b"\x1b]");
                    self.parser.advance(&mut self.term, body);
                    self.parser
                        .advance(&mut self.term, if bell { b"\x07" } else { b"\x1b\\" });
                }
            }
        });
        self.events
            .drain(self.size, &mut self.shell_title, &mut replies);
        replies
    }

    pub fn process_ended(&mut self) -> bool {
        self.program_status.prompt_or_exit()
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
