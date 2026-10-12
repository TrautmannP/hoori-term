//! Stable session identity, terminal state and bounded output processing.
use crate::session::{ProcessState, Session};
use crate::terminal::{GridSize, Terminal};
use std::time::{Duration, Instant};

pub const MAX_TABS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

#[derive(Debug, Clone)]
pub struct Shell {
    pub program: String,
    pub args: Vec<String>,
}

impl Shell {
    pub fn zsh() -> Self {
        Self {
            program: "/usr/bin/zsh".into(),
            args: vec!["-i".into()],
        }
    }
    pub fn bash() -> Self {
        Self {
            program: "/bin/bash".into(),
            args: vec!["-i".into()],
        }
    }
}

pub struct Tab {
    pub id: TabId,
    pub label: String,
    pub terminal: Terminal,
    pub session: Option<Session>,
    pub error: Option<String>,
}

impl Tab {
    fn new(id: TabId, shell: &Shell, size: GridSize) -> Self {
        let name = std::path::Path::new(&shell.program)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let (session, error) = match Session::spawn(&shell.program, &shell.args, size.pty()) {
            Ok(session) => (Some(session), None),
            Err(error) => (
                None,
                Some(format!("Shell konnte nicht gestartet werden: {error}")),
            ),
        };
        Self {
            id,
            label: format!("{name} · {}", id.0),
            terminal: Terminal::new(size),
            session,
            error,
        }
    }

    pub fn state(&self) -> ProcessState {
        self.session
            .as_ref()
            .map(Session::state)
            .unwrap_or_else(|| ProcessState::Failed(self.error.clone().unwrap_or_default()))
    }

    pub fn send(&mut self, bytes: Vec<u8>) {
        if self.state() == ProcessState::Running
            && let Some(session) = &self.session
            && let Err(error) = session.write(bytes)
        {
            self.error = Some(error);
        }
    }

    /// Every session gets its own refresh budget, including inactive tabs.
    /// Check the soft deadline between chunks, with backpressure and re-wakeup.
    pub fn poll_output(&mut self) -> bool {
        let deadline = Instant::now() + Duration::from_millis(2);
        let mut consumed = 0;
        while consumed < 32 * 1024 && Instant::now() < deadline {
            let Some(bytes) = self.session.as_ref().and_then(Session::read_chunk) else {
                break;
            };
            consumed += bytes.len();
            for reply in self.terminal.feed(&bytes) {
                self.send(reply);
            }
        }
        if let Some(session) = &self.session {
            if consumed >= 32 * 1024 || Instant::now() >= deadline {
                session.notifications.wake();
            }
            if let Some(error) = session.take_io_error() {
                self.error = Some(error);
            }
        }
        consumed > 0
    }

    pub fn resize(&mut self, size: GridSize) {
        if size == self.terminal.size {
            return;
        }
        self.terminal.resize(size);
        if let Some(session) = &self.session
            && let Err(error) = session.resize(size.pty())
        {
            self.error = Some(format!("Resize fehlgeschlagen: {error}"));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseTarget {
    Tab(TabId),
    Window,
}

pub struct Tabs {
    pub items: Vec<Tab>,
    pub active: Option<TabId>,
    next_id: u64,
}

impl Default for Tabs {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            active: None,
            next_id: 1,
        }
    }
}

impl Tabs {
    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.items.iter().find(|t| t.id == id)
    }
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.items.iter_mut().find(|t| t.id == id)
    }
    pub fn current(&self) -> Option<&Tab> {
        self.active.and_then(|id| self.get(id))
    }
    pub fn current_mut(&mut self) -> Option<&mut Tab> {
        self.active.and_then(|id| self.get_mut(id))
    }

    pub fn open(&mut self, shell: &Shell, size: GridSize) -> Result<TabId, String> {
        if self.items.len() >= MAX_TABS {
            return Err(format!(
                "Höchstens {MAX_TABS} Sitzungen können gleichzeitig geöffnet sein"
            ));
        }
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.items.push(Tab::new(id, shell, size));
        self.active = Some(id);
        Ok(id)
    }

    pub fn activate(&mut self, id: TabId) {
        if self.get(id).is_some() {
            self.active = Some(id);
        }
    }

    pub fn cycle(&mut self, forward: bool) {
        let Some(index) = self.items.iter().position(|t| Some(t.id) == self.active) else {
            return;
        };
        let next = if forward {
            (index + 1) % self.items.len()
        } else {
            (index + self.items.len() - 1) % self.items.len()
        };
        self.active = Some(self.items[next].id);
    }

    pub fn running_names(&self, target: CloseTarget) -> Vec<String> {
        self.items
            .iter()
            .filter(|t| target == CloseTarget::Window || target == CloseTarget::Tab(t.id))
            .filter(|t| t.state() == ProcessState::Running)
            .map(|t| t.label.clone())
            .collect()
    }

    /// Only call after the UI has confirmed all running targets. Return live
    /// transports so the caller can shut them down outside the event loop.
    pub fn remove(&mut self, target: CloseTarget) -> Vec<Session> {
        let index = self
            .items
            .iter()
            .position(|t| Some(t.id) == self.active)
            .unwrap_or(0);
        let mut removed = Vec::new();
        self.items.retain_mut(|tab| {
            if target == CloseTarget::Window || target == CloseTarget::Tab(tab.id) {
                if let Some(session) = tab.session.take() {
                    removed.push(session);
                }
                false
            } else {
                true
            }
        });
        if self.active.is_some_and(|id| self.get(id).is_none()) {
            self.active = self
                .items
                .get(index.min(self.items.len().saturating_sub(1)))
                .map(|t| t.id);
        }
        removed
    }
}
