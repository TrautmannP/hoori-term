use crate::Message;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use hoori_term::terminal::{GridSize, Terminal};
use iced::widget::canvas::{self, Geometry, Path, Stroke};
use iced::{Color, Event, Font, Point, Rectangle, Renderer, Size, Theme, mouse};

const CELL_WIDTH: f32 = 10.0;
const CELL_HEIGHT: f32 = 20.0;
const MARGIN: f32 = 10.0;
const BACKGROUND: Color = Color::from_rgb8(22, 24, 35);
const FOREGROUND: Color = Color::from_rgb8(220, 224, 235);

pub struct TerminalView<'a> {
    pub terminal: &'a Terminal,
    pub cache: &'a canvas::Cache,
}

impl canvas::Program<Message> for TerminalView<'_> {
    type State = Option<GridSize>;

    fn update(
        &self,
        state: &mut Self::State,
        _event: &Event,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let size = GridSize::new(
            ((bounds.width - MARGIN * 2.0).max(20.0) / CELL_WIDTH) as u16,
            ((bounds.height - MARGIN * 2.0).max(20.0) / CELL_HEIGHT) as u16,
        );
        if *state != Some(size) {
            *state = Some(size);
            return Some(canvas::Action::publish(Message::Resize(size)));
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        vec![self.cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), BACKGROUND);
            let content = self.terminal.term.renderable_content();
            let offset = content.display_offset as i32;
            let cursor = content.cursor;
            // Backgrounds precede text so wide glyphs are not covered by
            // their following spacer cells.
            for indexed in self.terminal.term.grid().display_iter() {
                let row = indexed.point.line.0 + offset;
                if row < 0 || row >= self.terminal.size.rows as i32 {
                    continue;
                }
                let cell = indexed.cell;
                let background = color(
                    if cell.flags.contains(Flags::INVERSE) {
                        cell.fg
                    } else {
                        cell.bg
                    },
                    content.colors,
                );
                if background != BACKGROUND {
                    frame.fill_rectangle(
                        Point::new(
                            MARGIN + indexed.point.column.0 as f32 * CELL_WIDTH,
                            MARGIN + row as f32 * CELL_HEIGHT,
                        ),
                        Size::new(CELL_WIDTH, CELL_HEIGHT),
                        background,
                    );
                }
            }
            for indexed in content.display_iter {
                let cell = indexed.cell;
                let row = indexed.point.line.0 + offset;
                if row < 0 || row >= self.terminal.size.rows as i32 {
                    continue;
                }
                let position = Point::new(
                    MARGIN + indexed.point.column.0 as f32 * CELL_WIDTH,
                    MARGIN + row as f32 * CELL_HEIGHT,
                );
                let mut foreground = color(
                    if cell.flags.contains(Flags::INVERSE) {
                        cell.bg
                    } else {
                        cell.fg
                    },
                    content.colors,
                );
                if cell.flags.contains(Flags::DIM) {
                    foreground = Color {
                        r: foreground.r * 0.65,
                        g: foreground.g * 0.65,
                        b: foreground.b * 0.65,
                        a: 1.0,
                    };
                }
                let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                    2.0 * CELL_WIDTH
                } else {
                    CELL_WIDTH
                };
                if !cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::HIDDEN)
                    && (cell.c != ' ' || cell.zerowidth().is_some_and(|s| !s.is_empty()))
                {
                    let mut value = cell.c.to_string();
                    if let Some(combining) = cell.zerowidth() {
                        value.extend(combining);
                    }
                    let font = Font {
                        weight: if cell.flags.contains(Flags::BOLD) {
                            iced::font::Weight::Bold
                        } else {
                            iced::font::Weight::Normal
                        },
                        style: if cell.flags.contains(Flags::ITALIC) {
                            iced::font::Style::Italic
                        } else {
                            iced::font::Style::Normal
                        },
                        ..Font::MONOSPACE
                    };
                    frame.fill_text(canvas::Text {
                        content: value,
                        position,
                        color: foreground,
                        size: 16.0.into(),
                        line_height: iced::widget::text::LineHeight::Absolute(CELL_HEIGHT.into()),
                        font,
                        shaping: iced::widget::text::Shaping::Advanced,
                        ..Default::default()
                    });
                }
                if cell.flags.contains(Flags::UNDERLINE) {
                    frame.fill_rectangle(
                        Point::new(position.x, position.y + 18.0),
                        Size::new(width, 1.0),
                        foreground,
                    );
                }
                if cell.flags.contains(Flags::STRIKEOUT) {
                    frame.fill_rectangle(
                        Point::new(position.x, position.y + 10.0),
                        Size::new(width, 1.0),
                        foreground,
                    );
                }
            }
            let row = cursor.point.line.0 + offset;
            if cursor.shape != CursorShape::Hidden
                && row >= 0
                && row < self.terminal.size.rows as i32
            {
                let origin = Point::new(
                    MARGIN + cursor.point.column.0 as f32 * CELL_WIDTH,
                    MARGIN + row as f32 * CELL_HEIGHT,
                );
                match cursor.shape {
                    CursorShape::Beam => {
                        frame.fill_rectangle(origin, Size::new(2.0, CELL_HEIGHT), FOREGROUND)
                    }
                    CursorShape::Underline => frame.fill_rectangle(
                        Point::new(origin.x, origin.y + 18.0),
                        Size::new(CELL_WIDTH, 2.0),
                        FOREGROUND,
                    ),
                    _ => frame.stroke(
                        &Path::rectangle(origin, Size::new(CELL_WIDTH, CELL_HEIGHT)),
                        Stroke::default().with_color(FOREGROUND).with_width(1.0),
                    ),
                }
            }
        })]
    }
}

fn color(value: AnsiColor, overrides: &alacritty_terminal::term::color::Colors) -> Color {
    match value {
        AnsiColor::Spec(rgb) => Color::from_rgb8(rgb.r, rgb.g, rgb.b),
        AnsiColor::Named(name) => indexed(name as usize, overrides),
        AnsiColor::Indexed(index) => indexed(index as usize, overrides),
    }
}

fn indexed(index: usize, overrides: &alacritty_terminal::term::color::Colors) -> Color {
    if let Some(rgb) = overrides[index] {
        return Color::from_rgb8(rgb.r, rgb.g, rgb.b);
    }
    const PALETTE: [[u8; 3]; 16] = [
        [22, 24, 35],
        [240, 106, 117],
        [148, 210, 134],
        [237, 197, 112],
        [122, 162, 247],
        [187, 154, 247],
        [106, 210, 215],
        [205, 214, 244],
        [99, 106, 132],
        [255, 135, 145],
        [174, 235, 159],
        [255, 219, 146],
        [152, 185, 255],
        [211, 180, 255],
        [143, 236, 239],
        [245, 247, 255],
    ];
    let rgb = match index {
        0..=15 => PALETTE[index],
        16..=231 => {
            let n = index - 16;
            let component = |v| if v == 0 { 0 } else { (55 + v * 40) as u8 };
            [component(n / 36), component(n / 6 % 6), component(n % 6)]
        }
        232..=255 => [8 + (index as u8 - 232) * 10; 3],
        x if x == NamedColor::Background as usize => return BACKGROUND,
        _ => return FOREGROUND,
    };
    Color::from_rgb8(rgb[0], rgb[1], rgb[2])
}
