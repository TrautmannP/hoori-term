//! Gate OSC strings before vte's std parser (whose OSC buffer is unbounded).
//! Ordinary terminal bytes pass through; only complete BEL/ST-terminated,
//! bounded OSC strings can reach the core or local metadata handlers.
use crate::program_status::MAX_SEQUENCE_BYTES;

pub enum Control<'a> {
    Core(&'a [u8]),
    Osc(&'a [u8], bool),
    Reset,
}

#[derive(Default, Clone, Copy)]
enum State {
    #[default]
    Ground,
    Escape,
    EscapeIntermediate,
    Osc,
    OscEscape,
}

#[derive(Default)]
pub struct ControlStream {
    state: State,
    osc: Vec<u8>,
    sequence_bytes: usize,
    core: Vec<u8>,
}

impl ControlStream {
    pub fn advance(&mut self, bytes: &[u8], mut dispatch: impl FnMut(Control<'_>)) {
        for &byte in bytes {
            match self.state {
                State::Ground => {
                    self.core.push(byte);
                    if byte == 0x1b {
                        self.state = State::Escape;
                    }
                }
                State::Escape | State::EscapeIntermediate => self.escape(byte, &mut dispatch),
                State::Osc => {
                    self.sequence_bytes = self
                        .sequence_bytes
                        .saturating_add(1)
                        .min(MAX_SEQUENCE_BYTES + 1);
                    match byte {
                        0x07 => self.finish(true, &mut dispatch),
                        0x1b => self.state = State::OscEscape,
                        0x18 | 0x1a => {
                            self.osc.clear();
                            self.state = State::Ground;
                        }
                        _ if self.sequence_bytes <= MAX_SEQUENCE_BYTES => self.osc.push(byte),
                        _ => {}
                    }
                }
                State::OscEscape => {
                    if byte == b'\\' {
                        self.sequence_bytes += 1;
                        self.finish(false, &mut dispatch);
                    } else {
                        // An ESC not followed by ST cancels the incomplete OSC
                        // and starts a new escape. Never apply its partial data.
                        self.osc.clear();
                        self.core.push(0x1b);
                        self.state = State::Escape;
                        self.escape(byte, &mut dispatch);
                    }
                }
            }
            if self.core.len() >= 8192 {
                self.flush(&mut dispatch);
            }
        }
        self.flush(&mut dispatch);
    }

    fn escape(&mut self, byte: u8, dispatch: &mut impl FnMut(Control<'_>)) {
        if matches!(self.state, State::Escape) && byte == b']' {
            // The already-forwarded ESC cancels the core's previous sequence.
            // CAN returns it to ground while the bounded OSC is withheld.
            self.core.push(0x18);
            self.flush(dispatch);
            self.osc.clear();
            self.sequence_bytes = 2;
            self.state = State::Osc;
            return;
        }
        self.core.push(byte);
        if matches!(self.state, State::Escape) && byte == b'c' {
            self.flush(dispatch);
            dispatch(Control::Reset);
        }
        match byte {
            0x1b => self.state = State::Escape,
            0x18 | 0x1a | 0x30..=0x7e => self.state = State::Ground,
            0x20..=0x2f => self.state = State::EscapeIntermediate,
            _ => {}
        }
    }

    fn finish(&mut self, bell: bool, dispatch: &mut impl FnMut(Control<'_>)) {
        if self.sequence_bytes <= MAX_SEQUENCE_BYTES {
            dispatch(Control::Osc(&self.osc, bell));
        }
        self.osc.clear();
        self.state = State::Ground;
    }

    fn flush(&mut self, dispatch: &mut impl FnMut(Control<'_>)) {
        if !self.core.is_empty() {
            dispatch(Control::Core(&self.core));
            self.core.clear();
        }
    }
}
