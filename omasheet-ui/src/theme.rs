// Copyright (c) 2026 Stephen Roe

//! Colours from the current Omarchy theme, with fallbacks elsewhere.

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

pub fn load() -> Theme {
    std::env::var_os("HOME")
        .map(|home| {
            std::path::PathBuf::from(home).join(".local/state/omarchy/current/theme/colors.toml")
        })
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(Theme::default, |text| parse(&text))
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
