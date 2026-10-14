//! The single opaque palette shared by native widgets and canvas chrome.
use iced::{
    Color, Theme,
    widget::{button, container, text_input},
};

pub const BACKGROUND: Color = Color::from_rgb8(11, 19, 32);
pub const NAVIGATION: Color = Color::from_rgb8(18, 31, 48);
pub const PANEL: Color = Color::from_rgb8(21, 34, 54);
pub const RAISED: Color = Color::from_rgb8(25, 43, 64);
pub const LINE: Color = Color::from_rgb8(40, 60, 83);
pub const TEXT: Color = Color::from_rgb8(231, 238, 247);
pub const MUTED: Color = Color::from_rgb8(157, 173, 193);
pub const ACCENT: Color = Color::from_rgb8(85, 170, 255);
pub const ERROR: Color = Color::from_rgb8(255, 125, 145);

pub fn theme() -> Theme {
    Theme::custom_with_fn(
        "hoori-term",
        iced::theme::Palette {
            background: BACKGROUND,
            text: TEXT,
            primary: ACCENT,
            success: Color::from_rgb8(125, 205, 170),
            warning: Color::from_rgb8(250, 205, 105),
            danger: ERROR,
        },
        |palette| {
            use iced::theme::palette::{Extended, Pair};
            let mut extended = Extended::generate(palette);
            // Keep complementary controls dark. The generic palette generator
            // otherwise chooses pale secondary surfaces for this background.
            extended.secondary.base = Pair {
                color: RAISED,
                text: TEXT,
            };
            extended.secondary.weak = Pair {
                color: PANEL,
                text: TEXT,
            };
            extended.secondary.strong = Pair {
                color: Color::from_rgb8(35, 59, 83),
                text: TEXT,
            };
            extended.primary.base = Pair {
                color: Color::from_rgb8(37, 110, 175),
                text: TEXT,
            };
            extended.primary.strong = Pair {
                color: Color::from_rgb8(43, 129, 201),
                text: TEXT,
            };
            extended.danger.base = Pair {
                color: Color::from_rgb8(94, 44, 61),
                text: TEXT,
            };
            extended.danger.strong = Pair {
                color: Color::from_rgb8(130, 52, 74),
                text: TEXT,
            };
            extended
        },
    )
}

pub fn surface(color: Color) -> container::Style {
    container::Style {
        background: Some(color.into()),
        text_color: Some(TEXT),
        border: iced::Border {
            color: LINE,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::secondary(theme, status);
    style.border = iced::Border {
        color: LINE,
        width: 1.0,
        radius: 5.0.into(),
    };
    style
}

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.placeholder = MUTED;
    style
}
