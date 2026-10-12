//! Logical, window-local shortcuts. Built-in dispatch and conflict checks use
//! the same table; direct tab bindings are stored on the stable tab identity.
use iced::keyboard::{Key, Modifiers, key::Named};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    NewTab,
    NewBash,
    NextTab,
    PreviousTab,
    CloseTab,
    EditTab,
    FocusTabs,
    Templates,
    SaveTemplate,
    Copy,
    Paste,
    HistoryTop,
    Live,
    PageUp,
    PageDown,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::NewTab => "Neue Sitzung",
            Self::NewBash => "Neue Bash-Sitzung",
            Self::NextTab => "Nächster Tab",
            Self::PreviousTab => "Vorheriger Tab",
            Self::CloseTab => "Tab schließen",
            Self::EditTab => "Tab bearbeiten",
            Self::FocusTabs => "Tableiste fokussieren",
            Self::Templates => "Vorlagen anzeigen",
            Self::SaveTemplate => "Als Vorlage speichern",
            Self::Copy => "Kopieren",
            Self::Paste => "Einfügen",
            Self::HistoryTop => "Verlaufsanfang",
            Self::Live => "Live-Ausgabe",
            Self::PageUp => "Verlauf aufwärts",
            Self::PageDown => "Verlauf abwärts",
        }
    }
}

pub fn builtin(key: &Key, modifiers: Modifiers) -> Option<Action> {
    if modifiers == Modifiers::CTRL && *key == Key::Named(Named::Tab) {
        return Some(Action::NextTab);
    }
    if modifiers == Modifiers::CTRL | Modifiers::SHIFT {
        return match key.as_ref() {
            Key::Named(Named::Tab) => Some(Action::PreviousTab),
            Key::Named(Named::Home) => Some(Action::HistoryTop),
            Key::Named(Named::End) => Some(Action::Live),
            Key::Character(c) => match c.to_ascii_lowercase().as_str() {
                "t" => Some(Action::NewTab),
                "b" => Some(Action::NewBash),
                "w" => Some(Action::CloseTab),
                "e" => Some(Action::EditTab),
                "l" => Some(Action::FocusTabs),
                "p" => Some(Action::Templates),
                "s" => Some(Action::SaveTemplate),
                "c" => Some(Action::Copy),
                "v" => Some(Action::Paste),
                _ => None,
            },
            _ => None,
        };
    }
    if modifiers == Modifiers::SHIFT {
        return match key {
            Key::Named(Named::PageUp) => Some(Action::PageUp),
            Key::Named(Named::PageDown) => Some(Action::PageDown),
            _ => None,
        };
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StoredShortcut", into = "StoredShortcut")]
pub struct Shortcut {
    key: Key,
    modifiers: Modifiers,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredShortcut {
    key: String,
    modifiers: u8,
}

impl From<Shortcut> for StoredShortcut {
    fn from(value: Shortcut) -> Self {
        Self {
            key: match value.key.as_ref() {
                Key::Character(c) => c.into(),
                Key::Named(named) => format!("{named:?}"),
                Key::Unidentified => unreachable!(),
            },
            modifiers: u8::from(value.modifiers.control())
                | (u8::from(value.modifiers.alt()) << 1)
                | (u8::from(value.modifiers.shift()) << 2)
                | (u8::from(value.modifiers.logo()) << 3),
        }
    }
}

impl TryFrom<StoredShortcut> for Shortcut {
    type Error = String;

    fn try_from(value: StoredShortcut) -> Result<Self, Self::Error> {
        let key = if value.key.chars().count() == 1 {
            Key::Character(value.key.into())
        } else {
            Key::Named(match value.key.as_str() {
                "Space" => Named::Space,
                "Tab" => Named::Tab,
                "Enter" => Named::Enter,
                "Escape" => Named::Escape,
                "Backspace" => Named::Backspace,
                "Delete" => Named::Delete,
                "Insert" => Named::Insert,
                "Home" => Named::Home,
                "End" => Named::End,
                "PageUp" => Named::PageUp,
                "PageDown" => Named::PageDown,
                "ArrowUp" => Named::ArrowUp,
                "ArrowDown" => Named::ArrowDown,
                "ArrowLeft" => Named::ArrowLeft,
                "ArrowRight" => Named::ArrowRight,
                "F1" => Named::F1,
                "F2" => Named::F2,
                "F3" => Named::F3,
                "F4" => Named::F4,
                "F5" => Named::F5,
                "F6" => Named::F6,
                "F7" => Named::F7,
                "F8" => Named::F8,
                "F9" => Named::F9,
                "F10" => Named::F10,
                "F11" => Named::F11,
                "F12" => Named::F12,
                _ => return Err("Unbekannte Shortcut-Taste".into()),
            })
        };
        if value.modifiers > 15 {
            return Err("Ungültige Shortcut-Modifikatoren".into());
        }
        let mut modifiers = Modifiers::empty();
        for (bit, modifier) in [
            (1, Modifiers::CTRL),
            (2, Modifiers::ALT),
            (4, Modifiers::SHIFT),
            (8, Modifiers::LOGO),
        ] {
            if value.modifiers & bit != 0 {
                modifiers |= modifier;
            }
        }
        Self::capture(&key, modifiers)?.ok_or_else(|| "Ungültiger Shortcut".into())
    }
}

impl Shortcut {
    /// Capture an unmodified logical key plus its modifiers, rather than the
    /// shifted text or the tab's position. Ignore modifier-only key presses.
    pub fn capture(key: &Key, modifiers: Modifiers) -> Result<Option<Self>, String> {
        if matches!(
            key,
            Key::Named(
                Named::Shift
                    | Named::Control
                    | Named::Alt
                    | Named::AltGraph
                    | Named::Super
                    | Named::Meta
                    | Named::CapsLock
                    | Named::NumLock
            )
        ) {
            return Ok(None);
        }
        let key = normalize(key).ok_or("Diese Taste wird als Shortcut nicht unterstützt")?;
        let function = matches!(
            key,
            Key::Named(
                Named::F1
                    | Named::F2
                    | Named::F3
                    | Named::F4
                    | Named::F5
                    | Named::F6
                    | Named::F7
                    | Named::F8
                    | Named::F9
                    | Named::F10
                    | Named::F11
                    | Named::F12
            )
        );
        if !function && !modifiers.intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::LOGO) {
            return Err("Strg, Alt oder Super mit einer Taste verwenden; F1–F12 auch ohne".into());
        }
        Ok(Some(Self { key, modifiers }))
    }

    pub fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        self.modifiers == modifiers && normalize(key).as_ref() == Some(&self.key)
    }

    pub fn reserved_action(&self) -> Option<Action> {
        builtin(&self.key, self.modifiers)
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.control() {
            parts.push("Strg".into());
        }
        if self.modifiers.alt() {
            parts.push("Alt".into());
        }
        if self.modifiers.shift() {
            parts.push("Umschalt".into());
        }
        if self.modifiers.logo() {
            parts.push("Super".into());
        }
        parts.push(match self.key.as_ref() {
            Key::Character(c) => c.to_uppercase(),
            Key::Named(Named::Space) => "Leertaste".into(),
            Key::Named(named) => format!("{named:?}"),
            Key::Unidentified => unreachable!("shortcuts contain normalized keys"),
        });
        parts.join("+")
    }
}

fn normalize(key: &Key) -> Option<Key> {
    match key {
        Key::Character(c) if c.as_str() == " " => Some(Key::Named(Named::Space)),
        Key::Character(c) if c.chars().count() == 1 && !c.chars().any(char::is_control) => {
            let normalized = c.to_lowercase();
            (normalized.chars().count() == 1).then(|| Key::Character(normalized.into()))
        }
        Key::Named(
            Named::Space
            | Named::Tab
            | Named::Enter
            | Named::Escape
            | Named::Backspace
            | Named::Delete
            | Named::Insert
            | Named::Home
            | Named::End
            | Named::PageUp
            | Named::PageDown
            | Named::ArrowUp
            | Named::ArrowDown
            | Named::ArrowLeft
            | Named::ArrowRight
            | Named::F1
            | Named::F2
            | Named::F3
            | Named::F4
            | Named::F5
            | Named::F6
            | Named::F7
            | Named::F8
            | Named::F9
            | Named::F10
            | Named::F11
            | Named::F12,
        ) => Some(key.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_preserves_exact_modifiers_and_normalizes_logical_keys() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        let shortcut = Shortcut::capture(&Key::Character("A".into()), mods)
            .unwrap()
            .unwrap();
        assert!(shortcut.matches(&Key::Character("a".into()), mods));
        assert!(!shortcut.matches(&Key::Character("a".into()), Modifiers::CTRL));
        assert_eq!(shortcut.label(), "Strg+Umschalt+A");
        assert_eq!(
            Shortcut::capture(&Key::Character(" ".into()), Modifiers::CTRL).unwrap(),
            Shortcut::capture(&Key::Named(Named::Space), Modifiers::CTRL).unwrap()
        );
        assert!(Shortcut::capture(&Key::Character("a".into()), Modifiers::empty()).is_err());
        assert!(Shortcut::capture(&Key::Unidentified, mods).is_err());
        assert_eq!(
            Shortcut::capture(&Key::Named(Named::Control), mods),
            Ok(None)
        );
        assert!(Shortcut::capture(&Key::Named(Named::F8), Modifiers::empty()).is_ok());
    }

    #[test]
    fn reserved_actions_are_exact_and_case_independent() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        let shortcut = Shortcut::capture(&Key::Character("E".into()), mods)
            .unwrap()
            .unwrap();
        assert_eq!(shortcut.reserved_action(), Some(Action::EditTab));
        assert_eq!(
            builtin(&Key::Named(Named::Tab), Modifiers::CTRL),
            Some(Action::NextTab)
        );
        assert_eq!(
            builtin(&Key::Named(Named::Tab), mods),
            Some(Action::PreviousTab)
        );
        assert_eq!(builtin(&Key::Character("e".into()), Modifiers::CTRL), None);
        assert_eq!(
            builtin(&Key::Character("e".into()), mods | Modifiers::ALT),
            None
        );
        assert_eq!(
            builtin(&Key::Named(Named::PageDown), Modifiers::SHIFT),
            Some(Action::PageDown)
        );
    }
}
