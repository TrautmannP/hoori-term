//! Explicit saved configuration, separate from live session identity.
//! The catalog has no filesystem effects. File operations run on a worker.
use crate::shortcuts::Shortcut;
use crate::tabs::{Identity, Shell, TabId, Tabs, validated_name};
use crate::terminal::GridSize;
use iced::keyboard::{Key, Modifiers};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_TEMPLATES: usize = 64;
pub const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_DIRECTORY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateShell {
    Zsh,
    Bash,
    #[serde(rename = "powershell_51")]
    PowerShell,
}

impl TemplateShell {
    pub fn label(self) -> &'static str {
        match self {
            Self::Zsh => "zsh",
            Self::Bash => "Bash",
            Self::PowerShell => "Windows PowerShell 5.1",
        }
    }
    pub fn shell(self, directory: &str, distribution: Option<&str>) -> Shell {
        if let Some(distribution) = distribution {
            return crate::launch::wsl_in(
                distribution,
                if self == Self::Zsh {
                    "/usr/bin/zsh"
                } else {
                    "/bin/bash"
                },
                directory,
            );
        }
        let mut shell = match self {
            Self::Zsh => Shell::zsh(),
            Self::Bash => Shell::bash(),
            Self::PowerShell => crate::launch::powershell(),
        };
        shell.directory = Some(directory.into());
        shell
    }
    pub fn for_shell(shell: &Shell) -> Self {
        if let Some((_, executable, _)) = crate::launch::wsl_context(shell) {
            return if executable.ends_with("zsh") {
                Self::Zsh
            } else {
                Self::Bash
            };
        }
        if cfg!(windows) {
            return Self::PowerShell;
        }
        if Path::new(&shell.program)
            .file_name()
            .is_some_and(|n| n == "bash")
        {
            Self::Bash
        } else {
            Self::Zsh
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub id: TemplateId,
    pub identity: Identity,
    pub shell: TemplateShell,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wsl_distribution: Option<String>,
    pub directory: String,
    #[serde(default)]
    pub pinned: bool,
}

impl Template {
    pub fn target_label(&self) -> String {
        match &self.wsl_distribution {
            Some(name) => format!("{name} · WSL 2 · {}", self.shell.label()),
            None => self.shell.label().into(),
        }
    }
    pub fn name(&self) -> &str {
        self.identity.name.as_deref().unwrap_or("")
    }
    fn validate(&self) -> Result<(), String> {
        let name = validated_name(self.name())?.ok_or("Eine Vorlage braucht einen Namen")?;
        if name != self.name() {
            return Err("Vorlagenname enthält äußere Leerzeichen".into());
        }
        if let Some(distribution) = &self.wsl_distribution {
            if !cfg!(windows)
                || self.shell == TemplateShell::PowerShell
                || !crate::launch::valid_distribution(distribution)
            {
                return Err(
                    "WSL-Ziel ist auf dieser Plattform ungültig oder nicht unterstützt".into(),
                );
            }
        } else if (cfg!(windows) && self.shell != TemplateShell::PowerShell)
            || (!cfg!(windows) && self.shell == TemplateShell::PowerShell)
        {
            return Err("Shell-Ziel ist auf dieser Plattform nicht unterstützt".into());
        }
        let absolute = if self.wsl_distribution.is_some() {
            self.directory.starts_with('/')
        } else {
            Path::new(&self.directory).is_absolute()
        };
        if self.directory.len() > MAX_DIRECTORY_BYTES
            || !absolute
            || self.directory.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) {
            return Err("Startverzeichnis muss ein absoluter Pfad ohne Steuerzeichen sein (maximal 4096 Bytes)".into());
        }
        if self
            .identity
            .shortcut
            .as_ref()
            .is_some_and(|s| s.reserved_action().is_some())
        {
            return Err("Vorlage verwendet einen reservierten App-Shortcut".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredCatalog")]
pub struct Catalog {
    version: u32,
    next_id: u64,
    pub items: Vec<Template>,
    order: Vec<TemplateId>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCatalog {
    version: u32,
    next_id: u64,
    items: Vec<Template>,
    // Absence means legacy listing order; an explicit null is invalid.
    #[serde(default, deserialize_with = "present_order")]
    order: Option<Vec<TemplateId>>,
}

fn present_order<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<TemplateId>>, D::Error> {
    Vec::<TemplateId>::deserialize(deserializer).map(Some)
}

impl From<StoredCatalog> for Catalog {
    fn from(stored: StoredCatalog) -> Self {
        let order = stored
            .order
            .unwrap_or_else(|| stored.items.iter().map(|item| item.id).collect());
        Self {
            version: stored.version,
            next_id: stored.next_id,
            items: stored.items,
            order,
        }
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: 1,
            next_id: 1,
            items: Vec::new(),
            order: Vec::new(),
        }
    }
}

impl Catalog {
    pub fn order(&self) -> &[TemplateId] {
        &self.order
    }

    pub fn ordered(&self) -> impl Iterator<Item = &Template> {
        self.order.iter().filter_map(|id| self.get(*id))
    }

    pub fn get(&self, id: TemplateId) -> Option<&Template> {
        self.items.iter().find(|t| t.id == id)
    }
    pub fn shortcut_target(&self, key: &Key, modifiers: Modifiers) -> Option<TemplateId> {
        self.items
            .iter()
            .find(|t| {
                t.identity
                    .shortcut
                    .as_ref()
                    .is_some_and(|s| s.matches(key, modifiers))
            })
            .map(|t| t.id)
    }
    pub fn check_shortcut(
        &self,
        tabs: &Tabs,
        template: Option<TemplateId>,
        tab: Option<TabId>,
        shortcut: &Shortcut,
    ) -> Result<(), String> {
        if let Some(action) = shortcut.reserved_action() {
            return Err(format!(
                "{} ist für „{}“ reserviert",
                shortcut.label(),
                action.label()
            ));
        }
        if let Some(other) = self
            .items
            .iter()
            .find(|t| Some(t.id) != template && t.identity.shortcut.as_ref() == Some(shortcut))
        {
            return Err(format!(
                "{} ist bereits der Vorlage „{}“ zugeordnet",
                shortcut.label(),
                other.name()
            ));
        }
        if let Some(other) = tabs.items.iter().find(|t| {
            Some(t.id) != tab
                && (template.is_none() || t.template != template)
                && t.identity.shortcut.as_ref() == Some(shortcut)
        }) {
            return Err(format!(
                "{} ist bereits „{}“ zugeordnet",
                shortcut.label(),
                other.description()
            ));
        }
        Ok(())
    }

    pub fn save_tab(
        &mut self,
        tabs: &mut Tabs,
        tab: TabId,
        mut draft: Template,
    ) -> Result<TemplateId, String> {
        let source = tabs.get(tab).ok_or("Dieser Tab ist nicht mehr geöffnet")?;
        if source.template.is_some() {
            return Err("Dieser Tab ist bereits mit einer Vorlage verknüpft".into());
        }
        if self.items.len() >= MAX_TEMPLATES {
            return Err(format!("Höchstens {MAX_TEMPLATES} Vorlagen sind möglich"));
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or("Keine weitere Vorlagen-ID verfügbar")?;
        draft.id = TemplateId(self.next_id);
        // Saving a tab captures its actual pin state, never an editor default.
        draft.pinned = source.pinned;
        draft.validate()?;
        if let Some(shortcut) = &draft.identity.shortcut {
            self.check_shortcut(tabs, None, Some(tab), shortcut)?;
        }
        let id = draft.id;
        let mut next_catalog = self.clone();
        next_catalog.items.push(draft.clone());
        next_catalog.order.push(id);
        next_catalog.next_id = next;
        let live: Vec<_> = tabs.items.iter().map(|tab| tab.id).collect();
        next_catalog.order_from_neighbors(tabs, tab, id, &live)?;
        next_catalog.to_bytes()?;
        let source = tabs.get_mut(tab).unwrap();
        source.template = Some(id);
        source.identity = draft.identity.clone();
        *self = next_catalog;
        Ok(id)
    }

    /// Only the moved/saved template changes position. Closed templates and
    /// all other entries retain their relative order; ordinary tabs are not
    /// durable anchors. The next linked neighbor takes precedence.
    fn order_from_neighbors(
        &mut self,
        tabs: &Tabs,
        source: TabId,
        template: TemplateId,
        live: &[TabId],
    ) -> Result<(), String> {
        let position = live
            .iter()
            .position(|id| *id == source)
            .ok_or("Dieser Tab ist nicht mehr geöffnet")?;
        let pinned = tabs
            .get(source)
            .ok_or("Dieser Tab ist nicht mehr geöffnet")?
            .pinned;
        let linked = |id: &TabId| {
            let tab = tabs.get(*id)?;
            (tab.pinned == pinned).then_some(tab.template).flatten()
        };
        let next = live[position + 1..].iter().find_map(linked);
        let previous = live[..position].iter().rev().find_map(linked);
        if next.is_none() && previous.is_none() {
            return Ok(());
        }
        self.order.retain(|id| *id != template);
        let destination = if let Some(next) = next {
            self.order
                .iter()
                .position(|id| *id == next)
                .ok_or("Verknüpfte Nachbarvorlage nicht vorhanden")?
        } else {
            self.order
                .iter()
                .position(|id| Some(*id) == previous)
                .ok_or("Verknüpfte Nachbarvorlage nicht vorhanden")?
                + 1
        };
        self.order.insert(destination, template);
        Ok(())
    }

    /// Validate the proposed live order and complete catalog before either
    /// model changes. The returned bool means durable order needs saving.
    pub fn move_tab(
        &mut self,
        tabs: &mut Tabs,
        source: TabId,
        target: TabId,
        after: bool,
    ) -> Result<bool, String> {
        let destination = tabs.move_destination(source, target, after)?;
        let from = tabs.items.iter().position(|tab| tab.id == source).unwrap();
        if destination == from {
            return Ok(false);
        }
        let mut next = self.clone();
        if let Some(id) = tabs.get(source).unwrap().template {
            if self.get(id).is_none() {
                return Err("Verknüpfte Vorlage nicht vorhanden".into());
            }
            let mut live: Vec<_> = tabs.items.iter().map(|tab| tab.id).collect();
            live.remove(from);
            live.insert(destination, source);
            next.order_from_neighbors(tabs, source, id, &live)?;
            next.to_bytes()?;
        }
        let changed = next.order != self.order;
        tabs.move_tab(source, target, after)?;
        *self = next;
        Ok(changed)
    }

    pub fn edit(&mut self, tabs: &mut Tabs, draft: Template) -> Result<(), String> {
        let index = self
            .items
            .iter()
            .position(|t| t.id == draft.id)
            .ok_or("Vorlage nicht mehr vorhanden")?;
        draft.validate()?;
        if let Some(shortcut) = &draft.identity.shortcut {
            self.check_shortcut(tabs, Some(draft.id), None, shortcut)?;
        }
        let mut next_catalog = self.clone();
        next_catalog.items[index] = draft.clone();
        next_catalog.to_bytes()?;
        // Start data changes affect the next spawn only. Keep the running PTY.
        for tab in &mut tabs.items {
            if tab.template == Some(draft.id) {
                tab.identity = draft.identity.clone();
            }
        }
        let linked: Vec<_> = tabs
            .items
            .iter()
            .filter(|tab| tab.template == Some(draft.id))
            .map(|tab| tab.id)
            .collect();
        for id in linked {
            tabs.set_pinned(id, draft.pinned)?;
        }
        *self = next_catalog;
        Ok(())
    }

    pub fn configure_tab(
        &mut self,
        tabs: &mut Tabs,
        tab: TabId,
        mut identity: Identity,
    ) -> Result<bool, String> {
        let source = tabs.get(tab).ok_or("Dieser Tab ist nicht mehr geöffnet")?;
        let link = source.template;
        identity.name = validated_name(identity.name.as_deref().unwrap_or_default())?;
        if let Some(shortcut) = &identity.shortcut {
            self.check_shortcut(tabs, link, Some(tab), shortcut)?;
        }
        if let Some(id) = link {
            let mut draft = self
                .get(id)
                .ok_or("Verknüpfte Vorlage nicht vorhanden")?
                .clone();
            identity.name = Some(
                identity
                    .name
                    .unwrap_or_else(|| source.original_label().to_owned()),
            );
            draft.identity = identity;
            self.edit(tabs, draft)?;
            Ok(true)
        } else {
            tabs.get_mut(tab).unwrap().identity = identity;
            Ok(false)
        }
    }

    /// Change presentation independently of pin group, transport and symbol.
    pub fn set_compact(
        &mut self,
        tabs: &mut Tabs,
        tab: TabId,
        compact: bool,
    ) -> Result<bool, String> {
        let mut identity = tabs
            .get(tab)
            .ok_or("Dieser Tab ist nicht mehr geöffnet")?
            .identity
            .clone();
        identity.compact = compact;
        self.configure_tab(tabs, tab, identity)
    }

    /// A linked change updates the existing template before moving the tab.
    /// Validation/size failures leave both models unchanged.
    pub fn set_pinned(
        &mut self,
        tabs: &mut Tabs,
        tab: TabId,
        pinned: bool,
    ) -> Result<bool, String> {
        let source = tabs.get(tab).ok_or("Dieser Tab ist nicht mehr geöffnet")?;
        if let Some(id) = source.template {
            let mut draft = self
                .get(id)
                .ok_or("Verknüpfte Vorlage nicht vorhanden")?
                .clone();
            draft.pinned = pinned;
            self.edit(tabs, draft)?;
            Ok(true)
        } else {
            tabs.set_pinned(tab, pinned)?;
            Ok(false)
        }
    }

    pub fn open(&self, tabs: &mut Tabs, id: TemplateId, size: GridSize) -> Result<TabId, String> {
        let template = self.get(id).ok_or("Vorlage nicht mehr vorhanden")?;
        if let Some(tab) = tabs.items.iter().find(|t| t.template == Some(id)) {
            let tab = tab.id;
            tabs.activate(tab);
            return Ok(tab);
        }
        if let Some(shortcut) = &template.identity.shortcut {
            self.check_shortcut(tabs, Some(id), None, shortcut)?;
        }
        let tab = tabs.open(
            &template
                .shell
                .shell(&template.directory, template.wsl_distribution.as_deref()),
            size,
        )?;
        let source = tabs.get_mut(tab).unwrap();
        source.template = Some(id);
        source.identity = template.identity.clone();
        tabs.set_pinned(tab, template.pinned)?;
        // Deliberate new opens follow saved ID order within the destination
        // group, preserving every existing tab's relative position.
        let index = self.order.iter().position(|id| *id == template.id).unwrap();
        let open = |id: &TemplateId| {
            tabs.items
                .iter()
                .find(|t| t.id != tab && t.template == Some(*id) && t.pinned == template.pinned)
                .map(|t| t.id)
        };
        let successor = self.order[index + 1..].iter().find_map(open);
        let predecessor = self.order[..index].iter().rev().find_map(open);
        if let Some(target) = successor.or(predecessor) {
            tabs.move_tab(tab, target, successor.is_none())?;
        }
        Ok(tab)
    }

    pub fn remove(&mut self, tabs: &mut Tabs, id: TemplateId) -> Result<(), String> {
        let index = self
            .items
            .iter()
            .position(|t| t.id == id)
            .ok_or("Vorlage nicht mehr vorhanden")?;
        self.items.remove(index);
        self.order.retain(|entry| *entry != id);
        for tab in &mut tabs.items {
            if tab.template == Some(id) {
                tab.template = None;
                tab.identity.shortcut = None;
            }
        }
        Ok(())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Vorlagendatei überschreitet 256 KiB".into());
        }
        let catalog: Self = serde_json::from_slice(bytes).map_err(|e| {
            format!(
                "Vorlagendatei ist ungültig (Zeile {}, Spalte {})",
                e.line(),
                e.column()
            )
        })?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes =
            serde_json::to_vec_pretty(self).map_err(|_| "Vorlagen konnten nicht kodiert werden")?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Vorlagendatei überschreitet 256 KiB".into());
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Nicht unterstützte Vorlagendatei-Version".into());
        }
        if self.items.len() > MAX_TEMPLATES || self.next_id == 0 {
            return Err("Ungültige Vorlagenanzahl oder ID".into());
        }
        if self.order.len() != self.items.len()
            || self
                .order
                .iter()
                .enumerate()
                .any(|(index, id)| self.get(*id).is_none() || self.order[..index].contains(id))
        {
            return Err("Vorlagenordnung enthält fehlende, doppelte oder unbekannte IDs".into());
        }
        for (i, template) in self.items.iter().enumerate() {
            template.validate()?;
            if template.id.0 == 0
                || template.id.0 >= self.next_id
                || self.items[..i].iter().any(|t| {
                    t.id == template.id
                        || (t.identity.shortcut.is_some()
                            && t.identity.shortcut == template.identity.shortcut)
                })
            {
                return Err("Doppelte/ungültige Vorlagen-ID oder Shortcut".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TemplateFile {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub catalog: Catalog,
    pub stamp: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Saved {
    pub stamp: Vec<u8>,
    pub warning: Option<String>,
}

impl TemplateFile {
    pub fn configured() -> Result<Self, String> {
        #[cfg(windows)]
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("Kein lokales Windows-Konfigurationsverzeichnis verfügbar")?;
        #[cfg(not(windows))]
        let root = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
            _ => PathBuf::from(
                std::env::var_os("HOME")
                    .ok_or("Kein lokales Konfigurationsverzeichnis verfügbar")?,
            )
            .join(".config"),
        };
        if !root.is_absolute() {
            return Err("Konfigurationsverzeichnis ist nicht absolut".into());
        }
        Ok(Self {
            path: root.join("hoori-term/templates-v1.json"),
        })
    }

    pub fn load(&self) -> Result<Loaded, String> {
        let stamp = self.read()?;
        let catalog = match &stamp {
            Some(bytes) => Catalog::from_bytes(bytes)?,
            None => Catalog::default(),
        };
        Ok(Loaded { catalog, stamp })
    }

    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("Vorlagendatei kann nicht gelesen werden: {e}")),
        };
        if !metadata.file_type().is_file() {
            return Err("Vorlagendatei ist keine reguläre Datei".into());
        }
        let mut bytes = Vec::new();
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK);
        }
        let file = options
            .open(&self.path)
            .map_err(|e| format!("Vorlagendatei kann nicht gelesen werden: {e}"))?;
        reject_reparse_file(&file)?;
        if !file
            .metadata()
            .map_err(|e| format!("Vorlagendatei kann nicht geprüft werden: {e}"))?
            .is_file()
        {
            return Err("Vorlagendatei ist keine reguläre Datei".into());
        }
        file.take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Vorlagendatei kann nicht gelesen werden: {e}"))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Vorlagendatei überschreitet 256 KiB".into());
        }
        Ok(Some(bytes))
    }

    /// Compare against the loaded bytes under an advisory lock, then replace
    /// with a flushed same-directory temporary file. Never overwrite bad loads.
    pub fn save(&self, catalog: &Catalog, expected: Option<&[u8]>) -> Result<Saved, String> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let bytes = catalog.to_bytes()?;
        let parent = self
            .path
            .parent()
            .ok_or("Vorlagenpfad hat kein Verzeichnis")?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Vorlagenverzeichnis kann nicht angelegt werden: {e}"))?;
        let mut options = OpenOptions::new();
        options.write(true).create(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(nix::libc::O_NOFOLLOW);
        }
        let lock = options
            .open(self.path.with_extension("lock"))
            .map_err(|e| format!("Vorlagensperre fehlgeschlagen: {e}"))?;
        reject_reparse_file(&lock)?;
        lock.try_lock()
            .map_err(|e| format!("Vorlagensperre nicht verfügbar: {e}"))?;
        if self.read()?.as_deref() != expected {
            return Err("Vorlagendatei wurde außerhalb dieser App geändert; vorhandene Datei bleibt erhalten".into());
        }
        let temporary = parent.join(format!(
            ".templates-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut created = false;
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|e| format!("Temporäre Vorlagendatei kann nicht angelegt werden: {e}"))?;
            created = true;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|e| {
                    format!("Vorlagen konnten nicht vollständig geschrieben werden: {e}")
                })?;
            // Close the flushed temporary handle before Windows replacement.
            drop(file);
            #[cfg(windows)]
            let replacement = crate::windows_fs::replace(&temporary, &self.path);
            #[cfg(not(windows))]
            let replacement = fs::rename(&temporary, &self.path);
            replacement.map_err(|e| format!("Vorlagendatei konnte nicht ersetzt werden: {e}"))?;
            #[cfg(not(windows))]
            let warning = File::open(parent)
                .and_then(|f| f.sync_all())
                .err()
                .map(|e| {
                    format!("Datei ersetzt, dauerhafte Verzeichnisspeicherung fehlgeschlagen: {e}")
                });
            #[cfg(windows)]
            let warning = None;
            Ok(Saved {
                stamp: bytes,
                warning,
            })
        })();
        if created {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

fn reject_reparse_file(file: &File) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if file
            .metadata()
            .map_err(|e| e.to_string())?
            .file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err("Vorlagenpfad ist ein Reparsepunkt; Datei bleibt erhalten".into());
        }
    }
    #[cfg(not(windows))]
    let _ = file;
    Ok(())
}
