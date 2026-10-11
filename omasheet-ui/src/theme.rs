// Copyright (c) 2026 Stephen Roe

//! Colours from the current Omarchy theme; elsewhere, a dark or a light set
//! to go with the system's colour scheme.

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
}
