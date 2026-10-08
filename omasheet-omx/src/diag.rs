//! Source locations and diagnostics.

/// A byte range in one source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub src: u32,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(src: u32, start: usize, end: usize) -> Span {
        Span {
            src,
            start: start as u32,
            end: end as u32,
        }
    }

    /// The span from the start of `self` to the end of `other`.
    pub fn to(self, other: Span) -> Span {
        Span {
            src: self.src,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            span,
            message: message.into(),
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Diagnostic {
        self.help = Some(help.into());
        self
    }
}

/// The source texts diagnostics refer to: a sheet file, an `eval` expression.
#[derive(Debug, Default)]
pub struct Sources {
    files: Vec<(String, String)>,
}

impl Sources {
    pub fn new() -> Sources {
        Sources::default()
    }

    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> u32 {
        self.files.push((name.into(), text.into()));
        (self.files.len() - 1) as u32
    }

    pub fn text(&self, src: u32) -> &str {
        &self.files[src as usize].1
    }

    pub fn name(&self, src: u32) -> &str {
        &self.files[src as usize].0
    }

    /// 1-based line and column of a byte offset.
    pub fn line_col(&self, src: u32, offset: u32) -> (usize, usize) {
        let text = self.text(src);
        let offset = (offset as usize).min(text.len());
        let before = &text[..offset];
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        (line, before[line_start..].chars().count() + 1)
    }

    /// `file:line:col: error: message`, the offending line, and a marker under
    /// the faulty span.
    pub fn render(&self, d: &Diagnostic) -> String {
        let text = self.text(d.span.src);
        let start = (d.span.start as usize).min(text.len());
        let end = (d.span.end as usize).clamp(start, text.len());
        let (line, col) = self.line_col(d.span.src, start as u32);
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[start..].find('\n').map_or(text.len(), |i| start + i);
        let source_line = text[line_start..line_end].trim_end_matches('\r');
        let width = text[start..end.min(line_end)].chars().count().max(1);
        let gutter = line.to_string().len();

        let mut out = format!(
            "{}:{}:{}: error: {}\n",
            self.name(d.span.src),
            line,
            col,
            d.message
        );
        out.push_str(&format!("{line:>gutter$} | {source_line}\n"));
        out.push_str(&format!(
            "{:gutter$} | {}{}\n",
            "",
            " ".repeat(col - 1),
            "^".repeat(width)
        ));
        if let Some(help) = &d.help {
            out.push_str(&format!("{:gutter$} = help: {}\n", "", help));
        }
        out
    }
}
