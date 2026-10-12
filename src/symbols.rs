//! Optional user identity, never inferred from shell output or process state.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Symbol {
    Terminal,
    Code,
    Folder,
    Server,
    Database,
    Branch,
    Star,
    Shield,
}

impl Symbol {
    /// Stable order for the local chooser; serialized IDs are the enum names.
    pub const ALL: [Self; 8] = [
        Self::Terminal,
        Self::Code,
        Self::Folder,
        Self::Server,
        Self::Database,
        Self::Branch,
        Self::Star,
        Self::Shield,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Code => "Code",
            Self::Folder => "Ordner",
            Self::Server => "Server",
            Self::Database => "Datenbank",
            Self::Branch => "Branch",
            Self::Star => "Stern",
            Self::Shield => "Schild",
        }
    }

    pub fn choice(index: usize) -> Option<Self> {
        index.checked_sub(1).and_then(|i| Self::ALL.get(i)).copied()
    }

    pub fn index(symbol: Option<Self>) -> usize {
        symbol
            .map(|s| Self::ALL.iter().position(|v| *v == s).unwrap() + 1)
            .unwrap_or(0)
    }
}

pub fn label(symbol: Option<Symbol>) -> &'static str {
    symbol.map(Symbol::label).unwrap_or("Kein Symbol")
}
