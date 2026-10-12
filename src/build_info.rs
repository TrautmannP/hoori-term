pub const COMMIT: &str = env!("HOORI_COMMIT");
pub const DIRTY: bool = matches!(env!("HOORI_DIRTY").as_bytes(), b"true");
pub const TARGET: &str = env!("HOORI_TARGET");

pub fn sanitize_title(shell_title: &str) -> String {
    shell_title
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(c, '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(120)
        .collect()
}

pub fn title(shell_title: &str) -> String {
    let label = sanitize_title(shell_title);
    format!(
        "hoori-term [{}{}] — {}",
        &COMMIT[..7],
        if DIRTY { " dirty" } else { "" },
        label
    )
}

pub fn metadata() -> String {
    format!(
        "hoori-term {}\ncommit: {COMMIT}\ndirty: {DIRTY}\ntarget: {TARGET}",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_titles_cannot_remove_build_identity_or_add_control_characters() {
        let title = title("shell\n\u{1b}\u{202e} title");
        assert!(title.starts_with(&format!("hoori-term [{}", &COMMIT[..7])));
        assert!(!title.contains('\n'));
        assert!(!title.contains('\u{1b}'));
        assert!(!title.contains('\u{202e}'));
    }
}
