//! Platform launch inventory. Discovery never changes WSL configuration.
use crate::tabs::Shell;

#[derive(Debug, Clone)]
pub struct Choice {
    pub label: String,
    pub shell: Shell,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub choices: Vec<Choice>,
    pub default: Option<usize>,
    pub notice: Option<String>,
}

impl Inventory {
    pub fn local() -> Self {
        Self {
            choices: vec![
                Choice {
                    label: "zsh öffnen".into(),
                    shell: Shell::zsh(),
                    enabled: true,
                },
                Choice {
                    label: "Bash öffnen".into(),
                    shell: Shell::bash(),
                    enabled: true,
                },
            ],
            default: Some(0),
            notice: None,
        }
    }

    pub fn unavailable(reason: String) -> Self {
        Self {
            choices: vec![Choice {
                label: "Windows PowerShell 5.1 öffnen".into(),
                shell: powershell(),
                enabled: true,
            }],
            default: None,
            notice: Some(reason),
        }
    }
}

pub fn system_program(name: &str) -> String {
    std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "C:\\Windows".into())
        .join("System32")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

pub fn powershell() -> Shell {
    Shell {
        program: system_program("WindowsPowerShell/v1.0/powershell.exe"),
        args: vec!["-NoLogo".into()],
        directory: None,
    }
}

pub fn wsl(distribution: &str) -> Shell {
    Shell {
        program: system_program("wsl.exe"),
        args: vec![
            "--distribution".into(),
            distribution.into(),
            "--cd".into(),
            "~".into(),
            "--exec".into(),
            "/bin/bash".into(),
            "-i".into(),
        ],
        directory: None,
    }
}

// Fixed launcher code. Both user-controlled values are positional argument
// data; a failed cd exits before an interactive shell can start elsewhere.
pub const WSL_DIRECTORY_SCRIPT: &str = "cd -P -- \"$1\" || exit; exec \"$2\" -i";

pub fn wsl_in(distribution: &str, executable: &str, directory: &str) -> Shell {
    Shell {
        program: system_program("wsl.exe"),
        args: vec![
            "--distribution".into(),
            distribution.into(),
            "--exec".into(),
            "/bin/sh".into(),
            "-c".into(),
            WSL_DIRECTORY_SCRIPT.into(),
            "hoori-term-directory".into(),
            directory.into(),
            executable.into(),
        ],
        directory: None,
    }
}

/// Recognize only our supported direct/wrapped WSL launch forms.
pub fn wsl_context(shell: &Shell) -> Option<(&str, &str, Option<&str>)> {
    if !std::path::Path::new(&shell.program)
        .file_name()?
        .eq_ignore_ascii_case("wsl.exe")
    {
        return None;
    }
    let distro = shell.args.iter().position(|arg| arg == "--distribution")?;
    let distro = shell.args.get(distro + 1)?.as_str();
    let exec = shell.args.iter().position(|arg| arg == "--exec")?;
    let executable = shell.args.get(exec + 1)?.as_str();
    if matches!(
        executable,
        "/bin/bash" | "/usr/bin/bash" | "/bin/zsh" | "/usr/bin/zsh"
    ) {
        let directory = shell
            .args
            .iter()
            .position(|arg| arg == "--cd")
            .and_then(|index| shell.args.get(index + 1))
            .map(String::as_str)
            .filter(|path| *path != "~");
        return Some((distro, executable, directory));
    }
    let tail = &shell.args[exec + 1..];
    if tail.len() == 6
        && tail[0] == "/bin/sh"
        && tail[1] == "-c"
        && tail[2] == WSL_DIRECTORY_SCRIPT
        && tail[3] == "hoori-term-directory"
        && matches!(tail[5].as_str(), "/bin/bash" | "/usr/bin/zsh")
    {
        return Some((distro, &tail[5], Some(&tail[4])));
    }
    None
}

pub fn valid_distribution(name: &str) -> bool {
    !name.is_empty() && name == name.trim() && name.len() <= 256 && !name.starts_with('-')
        && !name.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
}

#[cfg(windows)]
pub fn wsl_home(distribution: &str) -> Result<String, String> {
    if !valid_distribution(distribution) {
        return Err("Ungültiges WSL-Ziel".into());
    }
    let bytes = command_output(
        &[
            "--distribution",
            distribution,
            "--cd",
            "~",
            "--exec",
            "/bin/pwd",
            "-P",
        ],
        15,
    )?;
    let path = decode(&bytes)?.trim_end_matches(['\r', '\n']).to_owned();
    if !path.starts_with('/') || path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("WSL-Home ist kein gültiges absolutes Startverzeichnis".into());
    }
    Ok(path)
}

/// wsl.exe emits UTF-16LE when redirected, but accepts UTF-8 on some versions.
pub fn decode(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > 64 * 1024 {
        return Err("WSL-Liste überschreitet die Grenze".into());
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.contains(&0) {
        if !bytes.len().is_multiple_of(2) {
            return Err("Unvollständige WSL-Liste".into());
        }
        let words: Vec<_> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        String::from_utf16(&words)
            .map(|s| s.trim_start_matches('\u{feff}').into())
            .map_err(|_| "Ungültige WSL-Liste".into())
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| "Ungültige WSL-Liste".into())
    }
}

/// Match known names rather than localized column headings or state words.
pub fn parse(names: &str, verbose: &str) -> Result<Inventory, String> {
    let names: Vec<_> = names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    let mut inventory = Inventory::unavailable(
        "Keine WSL-2-Standarddistribution verfügbar. Bitte ein Ziel auswählen.".into(),
    );
    for &name in &names {
        if inventory.choices.len() > 64
            || name.len() > 256
            || name.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
            || name.starts_with('-')
        {
            return Err("WSL-Distributionsliste ist ungültig oder zu groß".into());
        }
        let mut matches = verbose.lines().filter_map(|line| {
            let line = line.trim();
            let default = line.starts_with('*');
            let row = line.strip_prefix('*').unwrap_or(line).trim_start();
            if names.iter().any(|other| {
                other.len() > name.len()
                    && row
                        .strip_prefix(other)
                        .is_some_and(|tail| tail.starts_with(char::is_whitespace))
            }) {
                return None;
            }
            let rest = row.strip_prefix(name)?;
            if !rest.starts_with(char::is_whitespace) {
                return None;
            }
            let mut columns = rest.split_whitespace();
            let version = columns.next_back()?;
            columns.next()?;
            Some((default, version))
        });
        let (default, version) = matches
            .next()
            .ok_or("WSL-Version konnte nicht zugeordnet werden")?;
        if matches.next().is_some() {
            return Err("Mehrdeutige WSL-Distributionsliste".into());
        }
        let enabled = version == "2";
        if !enabled && version != "1" {
            return Err("Unbekannte WSL-Version".into());
        }
        let index = inventory.choices.len() - 1;
        inventory.choices.insert(
            index,
            Choice {
                label: format!(
                    "{name} · WSL {version} · Bash{}{}",
                    if default { " · Standard" } else { "" },
                    if enabled {
                        ""
                    } else {
                        " · nicht unterstützt"
                    }
                ),
                shell: wsl(name),
                enabled,
            },
        );
        if default && enabled {
            if inventory.default.is_some() {
                return Err("Mehrere WSL-Standarddistributionen".into());
            }
            inventory.default = Some(index);
            inventory.notice = None;
        }
    }
    Ok(inventory)
}

#[cfg(windows)]
pub fn discover() -> Inventory {
    let result = (|| {
        let names = command_output(&["--list", "--quiet"], 5)?;
        let verbose = command_output(&["--list", "--verbose"], 5)?;
        parse(&decode(&names)?, &decode(&verbose)?)
    })();
    result.unwrap_or_else(|error| {
        Inventory::unavailable(format!(
            "WSL nicht verfügbar: {error}. PowerShell kann bewusst geöffnet werden."
        ))
    })
}

#[cfg(windows)]
fn command_output(args: &[&str], timeout_seconds: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(system_program("wsl.exe"))
        .args(args)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        // Drain both bounded output channels without exposing diagnostic
        // contents. Starting a distribution can be slower than listing it.
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("WSL-Aufruf fehlgeschlagen: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let errors = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes.len())
    });
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("WSL antwortet nicht rechtzeitig".to_owned());
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "WSL-Liste konnte nicht gelesen werden")?
        .map_err(|_| "WSL-Liste konnte nicht gelesen werden")?;
    let error_bytes = errors
        .join()
        .map_err(|_| "WSL-Fehlerkanal konnte nicht gelesen werden")?
        .map_err(|_| "WSL-Fehlerkanal konnte nicht gelesen werden")?;
    if error_bytes > 64 * 1024 {
        return Err("WSL-Fehlerkanal überschreitet die Grenze".into());
    }
    if !status?.success() {
        return Err("WSL ist nicht eingerichtet oder die Liste ist nicht lesbar".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_inventory_keeps_names_as_data_and_does_not_fallback_automatically() {
        let names = "Ubuntu Test & echo injected\r\nUbuntu\r\nLegacy\r\n";
        let inventory = parse(names, " NAME STATUS VERSION\n  Ubuntu Test & echo injected   Beendet   2\n  Ubuntu Läuft 2\n* Legacy Beendet 1").unwrap();
        assert_eq!(inventory.default, None);
        assert!(inventory.notice.is_some());
        assert_eq!(
            inventory.choices[0].shell.args[1],
            "Ubuntu Test & echo injected"
        );
        assert!(!inventory.choices[2].enabled);
        assert!(
            inventory.choices[3]
                .shell
                .program
                .ends_with("powershell.exe")
        );
        assert!(
            inventory
                .choices
                .iter()
                .all(|c| !c.shell.args.iter().any(|a| a == "--set-default"))
        );
        let inventory = parse(
            "Ubuntu\nUbuntu Test",
            "  Ubuntu Stopped 2\n* Ubuntu Test Running 2",
        )
        .unwrap();
        assert_eq!(inventory.default, Some(1));
        assert_eq!(
            parse(
                "Ubuntu\nUbuntu Test",
                "* Ubuntu Wird ausgeführt 2\nUbuntu Test Beendet 2"
            )
            .unwrap()
            .default,
            Some(0)
        );
    }

    #[test]
    fn empty_missing_ambiguous_and_bad_encoding_fail_visibly() {
        assert!(parse("", "").unwrap().notice.is_some());
        assert!(parse("Ubuntu", "unknown").is_err());
        assert!(parse("Ubuntu", "* Ubuntu Stopped 2\nUbuntu Stopped 2").is_err());
        assert!(decode(&[0xff, 0xfe, 0]).is_err());
        let bytes: Vec<_> = "* Ubuntu Beendet 2\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode(&bytes).unwrap(), "* Ubuntu Beendet 2\r\n");
    }
}
