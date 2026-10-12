use alacritty_terminal::term::TermMode;
use iced::keyboard::{Key, Modifiers, key::Named};

pub fn encode(
    key: &Key,
    modifiers: Modifiers,
    text: Option<&str>,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if modifiers.logo() {
        return None;
    }
    if modifiers.control()
        && modifiers.alt()
        && let Some(text) = text.filter(|s| !s.is_empty() && s.chars().all(|c| !c.is_control()))
    {
        // AltGr can be represented as Ctrl+Alt by the platform backend.
        return Some(text.as_bytes().to_vec());
    }
    let modifier = 1
        + u8::from(modifiers.shift())
        + 2 * u8::from(modifiers.alt())
        + 4 * u8::from(modifiers.control());
    let cursor = |letter| {
        if modifier != 1 {
            format!("\x1b[1;{modifier}{letter}")
        } else if mode.contains(TermMode::APP_CURSOR) {
            format!("\x1bO{letter}")
        } else {
            format!("\x1b[{letter}")
        }
    };
    let tilde = |number| {
        if modifier == 1 {
            format!("\x1b[{number}~")
        } else {
            format!("\x1b[{number};{modifier}~")
        }
    };
    let mut bytes = match key {
        Key::Named(named) => match named {
            Named::Space if modifiers.control() => vec![0],
            Named::Space => text?.as_bytes().to_vec(),
            Named::Enter => vec![b'\r'],
            Named::Backspace => vec![0x7f],
            Named::Escape => vec![0x1b],
            Named::Tab if modifiers.shift() => b"\x1b[Z".to_vec(),
            Named::Tab => vec![b'\t'],
            Named::ArrowUp => cursor('A').into_bytes(),
            Named::ArrowDown => cursor('B').into_bytes(),
            Named::ArrowRight => cursor('C').into_bytes(),
            Named::ArrowLeft => cursor('D').into_bytes(),
            Named::Home => cursor('H').into_bytes(),
            Named::End => cursor('F').into_bytes(),
            Named::Insert => tilde(2).into_bytes(),
            Named::Delete => tilde(3).into_bytes(),
            Named::PageUp => tilde(5).into_bytes(),
            Named::PageDown => tilde(6).into_bytes(),
            Named::F1 => b"\x1bOP".to_vec(),
            Named::F2 => b"\x1bOQ".to_vec(),
            Named::F3 => b"\x1bOR".to_vec(),
            Named::F4 => b"\x1bOS".to_vec(),
            Named::F5 => tilde(15).into_bytes(),
            Named::F6 => tilde(17).into_bytes(),
            Named::F7 => tilde(18).into_bytes(),
            Named::F8 => tilde(19).into_bytes(),
            Named::F9 => tilde(20).into_bytes(),
            Named::F10 => tilde(21).into_bytes(),
            Named::F11 => tilde(23).into_bytes(),
            Named::F12 => tilde(24).into_bytes(),
            _ => return None,
        },
        Key::Character(s) if modifiers.control() => {
            let c = s.chars().next()?.to_ascii_lowercase();
            match c {
                'a'..='z' => vec![(c as u8) - b'a' + 1],
                ' ' | '@' | '2' => vec![0],
                '[' | '3' => vec![27],
                '\\' | '4' => vec![28],
                ']' | '5' => vec![29],
                '^' | '6' => vec![30],
                '_' | '7' => vec![31],
                '?' | '8' => vec![127],
                _ => return None,
            }
        }
        Key::Character(_) => text?.as_bytes().to_vec(),
        // Dynamically mapped Unicode keys may have text even when the
        // unmodified logical key is not identified by the window backend.
        Key::Unidentified => text.filter(|s| !s.is_empty())?.as_bytes().to_vec(),
    };
    if modifiers.alt()
        && (matches!(key, Key::Character(_))
            || matches!(
                key,
                Key::Named(Named::Enter | Named::Backspace | Named::Space)
            ))
    {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_controls_unicode_and_application_cursor_mode() {
        assert_eq!(
            encode(
                &Key::Character("e".into()),
                Modifiers::empty(),
                None,
                TermMode::empty()
            ),
            None
        );
        assert_eq!(
            encode(
                &Key::Character("q".into()),
                Modifiers::CTRL | Modifiers::ALT,
                Some("@"),
                TermMode::empty()
            ),
            Some(vec![b'@'])
        );
        assert_eq!(
            encode(
                &Key::Unidentified,
                Modifiers::empty(),
                Some("界"),
                TermMode::empty()
            ),
            Some("界".as_bytes().to_vec())
        );
        assert_eq!(
            encode(
                &Key::Named(Named::Space),
                Modifiers::empty(),
                Some(" "),
                TermMode::empty()
            ),
            Some(vec![b' '])
        );
        assert_eq!(
            encode(
                &Key::Character("c".into()),
                Modifiers::CTRL,
                None,
                TermMode::empty()
            ),
            Some(vec![3])
        );
        assert_eq!(
            encode(
                &Key::Character("ü".into()),
                Modifiers::empty(),
                Some("ü"),
                TermMode::empty()
            ),
            Some("ü".as_bytes().to_vec())
        );
        assert_eq!(
            encode(
                &Key::Named(Named::ArrowUp),
                Modifiers::empty(),
                None,
                TermMode::APP_CURSOR
            ),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode(
                &Key::Named(Named::ArrowLeft),
                Modifiers::CTRL,
                None,
                TermMode::empty()
            ),
            Some(b"\x1b[1;5D".to_vec())
        );
    }
}
