// Copyright (c) 2026 Stephen Roe

//! Colours from the current Omarchy theme; elsewhere, a dark or a light set
//! to go with the system's colour scheme. The text size the desktop is set
//! to comes from the same place.

use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    pub error: String,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            dark: true,
            background: "#101010".into(),
            foreground: "#eeeeee".into(),
            accent: "#5584aa".into(),
            selection: "#186a9a".into(),
            muted: "#808080".into(),
            error: "#e06c75".into(),
        }
    }
}

impl Theme {
    /// The colours where there is no Omarchy theme and the system is light.
    fn light() -> Theme {
        Theme {
            dark: false,
            background: "#fafafa".into(),
            foreground: "#202020".into(),
            accent: "#2f6f9f".into(),
            selection: "#b5d5ee".into(),
            muted: "#757575".into(),
            error: "#c0392b".into(),
        }
    }
}

fn luminance(hex: &str) -> Option<f64> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let part = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .ok()
            .map(|v| v as f64 / 255.0)
    };
    Some(0.299 * part(0)? + 0.587 * part(2)? + 0.114 * part(4)?)
}

/// Parse an Omarchy `colors.toml`: flat `key = "value"` lines.
pub fn parse(text: &str) -> Theme {
    let mut theme = Theme::default();
    let mut mode = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']).to_string();
        match key.trim() {
            "mode" => mode = Some(value),
            "background" => theme.background = value,
            "foreground" => theme.foreground = value,
            "accent" => theme.accent = value,
            "selection" => theme.selection = value,
            "muted" => theme.muted = value,
            "red" => theme.error = value,
            _ => {}
        }
    }
    theme.dark = match mode.as_deref() {
        Some("dark") => true,
        Some("light") => false,
        _ => luminance(&theme.background).is_none_or(|l| l < 0.5),
    };
    theme
}

/// Where Omarchy keeps the theme in use.
fn current() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state/omarchy/current"))
}

/// The Omarchy theme, or without one the colours for a dark or a light system.
pub fn load(system_dark: bool) -> Theme {
    current()
        .and_then(|dir| std::fs::read_to_string(dir.join("theme/colors.toml")).ok())
        .map_or_else(
            || {
                if system_dark {
                    Theme::default()
                } else {
                    Theme::light()
                }
            },
            |text| parse(&text),
        )
}

/// The text size the desktop is set to, in pixels: Omarchy's
/// `[font] base-size`, which `omarchy display text size` writes and which
/// is the root its own shell sizes everything from. `None` where there is
/// no Omarchy to ask.
///
/// Two files hold it, in the order Omarchy itself layers them: the theme
/// sets one, and the machine's own `shell.toml` is laid over the top.
pub fn base_font_size() -> Option<u32> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let layers = [
        home.join(".local/state/omarchy/current/theme/shell.toml"),
        home.join(".config/omarchy/shell.toml"),
    ];
    let mut found = None;
    for path in layers {
        if let Ok(text) = std::fs::read_to_string(&path) {
            found = font_base_size(&text).or(found);
        }
    }
    found
}

/// `base-size` under `[font]` in a `shell.toml`, past every other table.
fn font_base_size(text: &str) -> Option<u32> {
    let mut in_font = false;
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(table) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_font = table.trim() == "font";
            continue;
        }
        if !in_font || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "base-size" {
            // A value may carry a trailing comment.
            let value = value.split('#').next().unwrap_or(value).trim();
            if let Ok(size) = value.parse::<u32>() {
                found = Some(size);
            }
        }
    }
    found
}

/// Call `changed` each time the Omarchy theme changes, until it gives false.
pub fn watch(changed: impl Fn() -> bool + Send + 'static) {
    use notify::Watcher;

    let Some(dir) = current().filter(|dir| dir.is_dir()) else {
        return;
    };
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        // Reading the theme is itself an event: only a change counts.
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|event| !event.kind.is_access()) {
                let _ = tx.send(());
            }
        });
        let Ok(mut watcher) = watcher else {
            return;
        };
        // Omarchy swaps the whole theme directory, so watch the one above it.
        if watcher
            .watch(&dir, notify::RecursiveMode::Recursive)
            .is_err()
        {
            return;
        }
        while rx.recv().is_ok() {
            // A change of theme is many events: wait for them to stop.
            while rx.recv_timeout(Duration::from_millis(150)).is_ok() {}
            if !changed() {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_colours_and_mode() {
        let theme =
            parse("mode = \"light\"\naccent = \"#89b4fa\"\nbackground = '#ffffff'\n# note\n");
        assert!(!theme.dark);
        assert_eq!(theme.accent, "#89b4fa");
        assert_eq!(theme.background, "#ffffff");
        assert_eq!(theme.foreground, Theme::default().foreground);
    }

    #[test]
    fn guesses_mode_from_background() {
        assert!(!parse("background = \"#fafafa\"").dark);
        assert!(parse("background = \"#1e1e2e\"").dark);
    }

    #[test]
    fn reads_the_text_size_the_desktop_is_set_to() {
        let shell = "[bar]\nsize-horizontal = 26\n\n\
                     [font]\n# base-size is the rem root.\n\
                     base-size = 9  # as the CLI writes it\n\
                     family = \"monospace\"\n\n\
                     [popups]\nbase-size = 99\n";
        assert_eq!(font_base_size(shell), Some(9));
        // Only under `[font]`: another table's own `base-size` is not it.
        assert_eq!(font_base_size("[popups]\nbase-size = 99\n"), None);
        // And nothing where the file says nothing.
        assert_eq!(font_base_size("[font]\nfamily = \"monospace\"\n"), None);
        assert_eq!(font_base_size(""), None);
    }
}
