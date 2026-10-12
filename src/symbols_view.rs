//! Original geometric drawings, shared by the strip, preview and chooser.
//! No external assets, fonts, resource paths, or terminal-controlled data.
use hoori_term::symbols::Symbol;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, mouse};

pub struct Icon(pub Option<Symbol>);

impl<Message> canvas::Program<Message> for Icon {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        draw(
            &mut frame,
            self.0,
            Point::ORIGIN,
            bounds.width.min(bounds.height),
            Color::from_rgb8(205, 214, 244),
        );
        vec![frame.into_geometry()]
    }
}

/// All shapes fit a 20x20 square; line width scales with the requested size.
pub fn draw(frame: &mut Frame, symbol: Option<Symbol>, origin: Point, size: f32, color: Color) {
    frame.with_save(|frame| {
        frame.translate(iced::Vector::new(origin.x, origin.y));
        frame.scale(size / 20.0);
        let stroke = Stroke::default().with_width(1.6).with_color(color);
        let line = |frame: &mut Frame, points: &[(f32, f32)]| {
            let path = Path::new(|p| {
                p.move_to(Point::new(points[0].0, points[0].1));
                for &(x, y) in &points[1..] {
                    p.line_to(Point::new(x, y));
                }
            });
            frame.stroke(&path, stroke);
        };
        match symbol {
            None => {
                frame.stroke(&Path::circle(Point::new(10.0, 10.0), 7.0), stroke);
                line(frame, &[(5.0, 15.0), (15.0, 5.0)]);
            }
            Some(Symbol::Terminal) => {
                frame.stroke(
                    &Path::rounded_rectangle(
                        Point::new(2.0, 3.0),
                        Size::new(16.0, 14.0),
                        2.0.into(),
                    ),
                    stroke,
                );
                line(frame, &[(5.0, 7.0), (8.0, 10.0), (5.0, 13.0)]);
                line(frame, &[(10.0, 13.0), (15.0, 13.0)]);
            }
            Some(Symbol::Code) => {
                line(frame, &[(6.0, 5.0), (2.0, 10.0), (6.0, 15.0)]);
                line(frame, &[(14.0, 5.0), (18.0, 10.0), (14.0, 15.0)]);
                line(frame, &[(12.0, 3.0), (8.0, 17.0)]);
            }
            Some(Symbol::Folder) => {
                line(
                    frame,
                    &[
                        (2.0, 16.0),
                        (2.0, 4.0),
                        (8.0, 4.0),
                        (10.0, 7.0),
                        (18.0, 7.0),
                        (18.0, 16.0),
                        (2.0, 16.0),
                    ],
                );
                line(frame, &[(2.0, 8.0), (18.0, 8.0)]);
            }
            Some(Symbol::Server) => {
                for y in [2.5, 11.0] {
                    frame.stroke(
                        &Path::rounded_rectangle(
                            Point::new(2.0, y),
                            Size::new(16.0, 6.5),
                            1.0.into(),
                        ),
                        stroke,
                    );
                    frame.fill(&Path::circle(Point::new(5.0, y + 3.25), 1.0), color);
                    line(frame, &[(9.0, y + 3.25), (15.0, y + 3.25)]);
                }
            }
            Some(Symbol::Database) => {
                let oval = |frame: &mut Frame, y: f32| {
                    frame.stroke(
                        &Path::new(|p| {
                            p.move_to(Point::new(3.0, y));
                            p.bezier_curve_to(
                                Point::new(3.0, y - 4.0),
                                Point::new(17.0, y - 4.0),
                                Point::new(17.0, y),
                            );
                            p.bezier_curve_to(
                                Point::new(17.0, y + 4.0),
                                Point::new(3.0, y + 4.0),
                                Point::new(3.0, y),
                            );
                        }),
                        stroke,
                    );
                };
                oval(frame, 5.0);
                line(frame, &[(3.0, 5.0), (3.0, 15.0)]);
                line(frame, &[(17.0, 5.0), (17.0, 15.0)]);
                for y in [10.0, 15.0] {
                    frame.stroke(
                        &Path::new(|p| {
                            p.move_to(Point::new(3.0, y));
                            p.bezier_curve_to(
                                Point::new(3.0, y + 4.0),
                                Point::new(17.0, y + 4.0),
                                Point::new(17.0, y),
                            );
                        }),
                        stroke,
                    );
                }
            }
            Some(Symbol::Branch) => {
                line(frame, &[(5.0, 5.0), (5.0, 14.0)]);
                line(frame, &[(5.0, 11.0), (15.0, 7.0), (15.0, 5.0)]);
                for (x, y) in [(5.0, 3.0), (5.0, 16.0), (15.0, 3.0)] {
                    frame.stroke(&Path::circle(Point::new(x, y), 2.0), stroke);
                }
            }
            Some(Symbol::Star) => {
                line(
                    frame,
                    &[
                        (10.0, 2.0),
                        (12.5, 7.0),
                        (18.0, 8.0),
                        (14.0, 12.0),
                        (15.0, 18.0),
                        (10.0, 15.0),
                        (5.0, 18.0),
                        (6.0, 12.0),
                        (2.0, 8.0),
                        (7.5, 7.0),
                        (10.0, 2.0),
                    ],
                );
            }
            Some(Symbol::Shield) => {
                line(
                    frame,
                    &[
                        (10.0, 2.0),
                        (17.0, 5.0),
                        (16.0, 12.0),
                        (13.0, 16.0),
                        (10.0, 18.0),
                        (7.0, 16.0),
                        (4.0, 12.0),
                        (3.0, 5.0),
                        (10.0, 2.0),
                    ],
                );
            }
        }
    });
}
