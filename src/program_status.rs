//! Local, bounded OSC 7501 Program Status Protocol (revision 0.4).
//! These are program claims, never process state or instructions to the host.
use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use std::collections::VecDeque;

pub const MAX_SEQUENCE_BYTES: usize = 4096;
pub const MAX_RECORDS: usize = 64;
pub const CAPABILITIES: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "bereit",
            Self::Working => "arbeitet",
            Self::Done => "fertig",
            Self::Blocked => "wartet",
            Self::Error => "Fehler",
        }
    }

    // Attention takes precedence. Ties select the most recently updated record.
    fn priority(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Done => 1,
            Self::Working => 2,
            Self::Blocked => 3,
            Self::Error => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Permission => "Freigabe",
            Self::Question => "Antwort",
            Self::Auth => "Anmeldung",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Empty identifies the root; a supplied empty id is rejected.
    pub id: String,
    pub state: State,
    pub kind: Option<Kind>,
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
}

enum Report {
    Update(Record),
    Clear(String),
}

#[derive(Default)]
pub struct ProgramStatus {
    // Oldest update first: bounded storage and deterministic eviction/ties.
    records: VecDeque<Record>,
}

impl ProgramStatus {
    pub fn apply(&mut self, body: &[u8]) {
        let Some(report) = parse(body) else { return };
        match report {
            Report::Clear(id) => self.records.retain(|r| {
                !id.is_empty()
                    && r.id != id
                    && !r
                        .id
                        .strip_prefix(&id)
                        .is_some_and(|rest| rest.starts_with('/'))
            }),
            Report::Update(record) => {
                self.records.retain(|r| r.id != record.id);
                if self.records.len() == MAX_RECORDS {
                    self.records.pop_front();
                }
                self.records.push_back(record);
            }
        }
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Both OSC 133 A and PTY exit drop transient reports. We also drop idle;
    /// done/error remain visible until the program clears/replaces them or RIS.
    pub fn prompt_or_exit(&mut self) -> bool {
        let count = self.records.len();
        self.records
            .retain(|r| matches!(r.state, State::Done | State::Error));
        self.records.len() != count
    }

    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.records.iter()
    }

    pub fn summary(&self) -> Option<&Record> {
        self.records.iter().max_by_key(|r| r.state.priority())
    }

    pub fn attention_counts(&self) -> (usize, usize) {
        (
            self.records
                .iter()
                .filter(|r| r.state == State::Blocked)
                .count(),
            self.records
                .iter()
                .filter(|r| r.state == State::Error)
                .count(),
        )
    }

    pub fn inherited_app<'a>(&'a self, record: &'a Record) -> Option<&'a str> {
        if let Some(app) = record.app.as_deref() {
            return Some(app);
        }
        let mut id = record.id.as_str();
        while !id.is_empty() {
            id = id.rsplit_once('/').map_or("", |(parent, _)| parent);
            if let Some(app) = self
                .records
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.app.as_deref())
            {
                return Some(app);
            }
        }
        None
    }

    pub fn summary_label(&self) -> String {
        let Some(record) = self.summary() else {
            return "CLI: keine Statusmeldung".into();
        };
        let (waiting, errors) = self.attention_counts();
        let mut text = format!("CLI: {}", record.state.label());
        if let Some(progress) = record.progress {
            text.push_str(&format!(" {progress}%"));
        }
        if self.records.len() > 1 {
            text.push_str(&format!(
                " · {} Meldungen, {waiting} warten, {errors} Fehler",
                self.records.len()
            ));
        }
        text
    }
}

fn decode_text(value: &[u8], encoded_limit: usize, decoded_limit: usize) -> Option<String> {
    if value.len() > encoded_limit {
        return None;
    }
    let bytes = BASE64.decode(value).ok()?;
    if bytes.len() > decoded_limit {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?;
    if text.chars().any(char::is_control) {
        return None;
    }
    Some(crate::build_info::sanitize_display_text(text))
}

fn segment(value: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(b))
}

fn valid_id(value: &[u8]) -> bool {
    let parts = value.split(|b| *b == b'/');
    parts.clone().count() <= 8 && parts.into_iter().all(segment)
}

fn parse(body: &[u8]) -> Option<Report> {
    // Count the OSC introducer, number, delimiter and shortest terminator.
    // The stream gate enforces the exact total; this entry point also bounds
    // direct callers independently (the one-byte BEL boundary is handled there).
    if body.len() > MAX_SEQUENCE_BYTES - 8 {
        return None;
    }
    let mut state = None;
    let mut id = None;
    let mut kind = None;
    let mut progress = None;
    let mut app = None;
    let mut title = None;
    let mut message = None;
    for pair in body.split(|b| *b == b':') {
        let Some(equal) = pair.iter().position(|b| *b == b'=') else {
            continue;
        };
        let key = pair[..equal].trim_ascii();
        let value = pair[equal + 1..].trim_ascii();
        if key.len() > 16 {
            return None;
        }
        // Caps apply before malformed/unknown pairs or repeated keys are
        // ignored. No partial record mutation is possible during validation.
        match key {
            b"msg" if value.len() > 2732 => return None,
            b"title" if value.len() > 256 => return None,
            b"app" if value.len() > 32 => return None,
            b"id" if value.len() > 128 => return None,
            _ => {}
        }
        // Never reinterpret a malformed supplied id as the root record,
        // including values that fail the general pair alphabet. Repeated ids
        // still follow the last-value rule before final grammar validation.
        if key == b"id" {
            id = Some(value);
        }
        if key.is_empty()
            || !key.iter().all(u8::is_ascii_lowercase)
            || !value
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"_.,+/=-".contains(b))
        {
            continue;
        }
        match key {
            b"state" => state = Some(value),
            b"id" => id = Some(value),
            b"kind" => {
                kind = match value {
                    b"permission" => Some(Kind::Permission),
                    b"question" => Some(Kind::Question),
                    b"auth" => Some(Kind::Auth),
                    _ => None,
                }
            }
            b"progress" => {
                progress = std::str::from_utf8(value)
                    .ok()
                    .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|v| v.parse::<u8>().ok())
                    .filter(|v| *v <= 100)
            }
            b"app" => app = segment(value).then(|| String::from_utf8(value.to_vec()).unwrap()),
            b"title" => title = Some(decode_text(value, 256, 192)?),
            b"msg" => message = Some(decode_text(value, 2732, 2048)?),
            _ => {}
        }
    }
    if id.is_some_and(|id| !valid_id(id)) {
        return None;
    }
    let id = String::from_utf8(id.unwrap_or_default().to_vec()).ok()?;
    let state = match state? {
        b"idle" => State::Idle,
        b"working" => State::Working,
        b"done" => State::Done,
        b"blocked" => State::Blocked,
        b"error" => State::Error,
        b"clear" => return Some(Report::Clear(id)),
        _ => return None,
    };
    if state != State::Blocked {
        kind = None;
    }
    if !matches!(state, State::Working | State::Blocked) {
        progress = None;
    }
    Some(Report::Update(Record {
        id,
        state,
        kind,
        progress,
        app,
        title,
        message,
    }))
}
