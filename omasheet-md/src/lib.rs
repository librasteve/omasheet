// Copyright (c) 2026 Stephen Roe

//! Omasheet in Markdown.
//!
//! A Markdown document may hold sheets and quote their values:
//!
//! - a fenced `omx` block is `.omx` source, and is shown as its tables;
//! - `{{ expression }}` in the text is replaced by the expression's value;
//! - `sheets:` in the front matter names `.omx` files to read as well.
//!
//! Everything the document names is one sheet. [`render`] calculates it and
//! gives the document back as Markdown with the values in place, or as HTML.

use omasheet_engine::omx::sheet::{is_zone_name, parse_sheet};
use omasheet_engine::omx::{Diagnostic, Sources, Span, compile, compile_expr};
use omasheet_engine::value::View;
use omasheet_engine::{Engine, Options, Outcome, Value};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::ops::Range;
use std::rc::Rc;

/// What a rendered document is written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Markdown, with values and tables in place: plain text to read in a
    /// terminal, and a document for any Markdown tool.
    Markdown,
    /// A whole HTML page.
    Html,
}

fn extensions() -> pulldown_cmark::Options {
    use pulldown_cmark::Options as O;
    O::ENABLE_TABLES
        | O::ENABLE_STRIKETHROUGH
        | O::ENABLE_FOOTNOTES
        | O::ENABLE_TASKLISTS
        | O::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

/// What the front matter says: the sheets it names, each with where it is
/// named, and the title.
#[derive(Default)]
struct Front {
    sheets: Vec<(String, Range<usize>)>,
    title: Option<String>,
}

/// A YAML value with its quotes and surrounding space taken off, and where
/// it is in `text`.
fn scalar(text: &str, range: Range<usize>) -> Option<(String, Range<usize>)> {
    let raw = &text[range.clone()];
    let lead = raw.len() - raw.trim_start().len();
    let mut value = raw.trim();
    let mut start = range.start + lead;
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            value = &value[1..value.len() - 1];
            start += 1;
        }
    }
    (!value.is_empty()).then(|| (value.to_string(), start..start + value.len()))
}

/// Read the front matter: the lines between a first line of `---` and the
/// next. Only `sheets` and `title` are looked at.
fn front_matter(text: &str) -> Front {
    let mut front = Front::default();
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return front;
    }
    let mut at = text.find('\n').map_or(text.len(), |i| i + 1);
    let mut in_sheets = false;
    for line in lines {
        let here = at;
        at += line.len();
        let body = line.trim_end();
        if body == "---" || body == "..." {
            return front;
        }
        if let Some(item) = body.trim_start().strip_prefix("- ").filter(|_| in_sheets) {
            let start = here + body.len() - item.len();
            front.sheets.extend(scalar(text, start..start + item.len()));
            continue;
        }
        in_sheets = false;
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        let start = here + key.len() + 1;
        let range = start..start + value.len();
        match key {
            "title" => front.title = scalar(text, range).map(|(title, _)| title),
            "sheets" | "sheet" => {
                let inner = value.trim();
                if let Some(list) = inner.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    let mut from = start + value.find('[').unwrap_or(0) + 1;
                    for part in list.split(',') {
                        front.sheets.extend(scalar(text, from..from + part.len()));
                        from += part.len() + 1;
                    }
                } else if inner.is_empty() {
                    in_sheets = true;
                } else {
                    front.sheets.extend(scalar(text, range));
                }
            }
            _ => {}
        }
    }
    // It never closed: it was not front matter.
    Front::default()
}

/// A fenced `omx` block: all of it, and the parts of it that are source.
struct Block {
    range: Range<usize>,
    source: Vec<Range<usize>>,
}

/// The `omx` blocks of a document, and the parts of it where `{{` is not
/// an interpolation: code, HTML and the front matter.
fn scan(text: &str) -> (Vec<Block>, Vec<Range<usize>>) {
    let mut blocks = Vec::new();
    let mut skip = Vec::new();
    let mut open: Option<Block> = None;
    for (event, range) in Parser::new_ext(text, extensions()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let omx = matches!(&kind, CodeBlockKind::Fenced(info)
                    if info.split_whitespace().next() == Some("omx"));
                skip.push(range.clone());
                open = omx.then_some(Block {
                    range,
                    source: Vec::new(),
                });
            }
            Event::End(TagEnd::CodeBlock) => blocks.extend(open.take()),
            Event::Text(_) => {
                if let Some(block) = open.as_mut() {
                    block.source.push(range);
                }
            }
            Event::Start(Tag::MetadataBlock(_) | Tag::HtmlBlock)
            | Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_) => skip.push(range),
            _ => {}
        }
    }
    (blocks, skip)
}

/// Each `{{ expression }}` outside `skip`: all of it, and the expression.
/// A `{{` that is not closed in its paragraph, or follows a backslash, is
/// text.
fn interpolations(text: &str, skip: &[Range<usize>]) -> Vec<(Range<usize>, Range<usize>)> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(open) = text[at..].find("{{").map(|i| at + i) {
        if let Some(inside) = skip.iter().find(|r| r.contains(&open)) {
            at = inside.end.max(open + 2);
            continue;
        }
        let from = open + 2;
        let close = text[from..].find("}}").map(|i| from + i);
        let escaped = text[..open].ends_with('\\');
        match close.filter(|close| !escaped && !text[from..*close].contains("\n\n")) {
            Some(close) => {
                found.push((open..close + 2, from..close));
                at = close + 2;
            }
            None => at = from,
        }
    }
    found
}

/// A part of the one sheet a document is, and the source it came from.
struct Segment {
    at: usize,
    len: usize,
    src: u32,
    base: usize,
}

/// The one sheet of a document, put together from its blocks and the files
/// it names, with what is needed to say where in them a diagnostic is.
#[derive(Default)]
struct Joined {
    text: String,
    segments: Vec<Segment>,
}

impl Joined {
    fn push(&mut self, part: &str, src: u32, base: usize) {
        self.segments.push(Segment {
            at: self.text.len(),
            len: part.len(),
            src,
            base,
        });
        self.text.push_str(part);
        self.text.push_str("\n\n");
    }

    /// A place in the joined sheet as a place in the source it came from.
    fn locate(&self, span: Span) -> Span {
        let start = span.start as usize;
        match self
            .segments
            .iter()
            .find(|s| (s.at..=s.at + s.len).contains(&start))
        {
            Some(s) => Span::new(
                s.src,
                start - s.at + s.base,
                (span.end as usize).min(s.at + s.len) - s.at + s.base,
            ),
            None => Span::new(0, 0, 0),
        }
    }
}

/// Take the `zone` line out of a sheet's text, if it has one before its
/// first table: its name and where that is. A document's sheets are joined
/// into one, and only the first lines of that one may name the zone.
fn take_zone(text: &mut String) -> Option<(String, usize)> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("table ") {
            return None;
        }
        if let Some(name) = trimmed.strip_prefix("zone ").map(str::trim)
            && is_zone_name(name)
        {
            let start = at + line.find(name)?;
            let (from, to) = (at, at + line.trim_end_matches(['\n', '\r']).len());
            let name = name.to_string();
            text.replace_range(from..to, &" ".repeat(to - from));
            return Some((name, start));
        }
        at += line.len();
    }
    None
}

/// A value's text with the characters Markdown reads as markup escaped.
fn escape(text: &str, format: Format) -> String {
    let marks: &[char] = match format {
        // A table still needs its `|` and line breaks kept out of cells.
        Format::Markdown => &[],
        Format::Html => &['\\', '`', '*', '_', '[', ']', '<', '>', '&', '#', '~'],
    };
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if marks.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Rows and columns of a table as a Markdown table, numbers to the right.
fn table(
    engine: &Engine,
    program: &omasheet_engine::omx::Program,
    view: &View,
    format: Format,
) -> String {
    let cols = &program.tables[view.table].cols;
    let mut grid: Vec<Vec<String>> =
        vec![view.cols.iter().map(|&c| cols[c].name.clone()).collect()];
    let mut numeric = vec![true; view.cols.len()];
    for &r in view.rows.iter() {
        let mut line = Vec::with_capacity(view.cols.len());
        for (k, &c) in view.cols.iter().enumerate() {
            let value = engine.cell_shown(view.table, c, r);
            if !matches!(value, Value::Empty | Value::Error) && !value.is_numeric() {
                numeric[k] = false;
            }
            let shown = escape(&engine.show(&value, false), format);
            line.push(shown.replace('|', "\\|").replace('\n', " "));
        }
        grid.push(line);
    }
    let widths: Vec<usize> = (0..view.cols.len())
        .map(|k| {
            grid.iter()
                .map(|row| row[k].chars().count())
                .max()
                .unwrap_or(0)
                .max(3)
        })
        .collect();
    let mut out = String::new();
    for (i, row) in grid.iter().enumerate() {
        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(k, text)| {
                let pad = " ".repeat(widths[k] - text.chars().count());
                if numeric[k] {
                    format!("{pad}{text}")
                } else {
                    format!("{text}{pad}")
                }
            })
            .collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
        if i == 0 {
            let rule: Vec<String> = (0..widths.len())
                .map(|k| {
                    let dashes = "-".repeat(widths[k] - 1);
                    if numeric[k] {
                        format!("{dashes}:")
                    } else {
                        format!(":{dashes}")
                    }
                })
                .collect();
            out.push_str(&format!("| {} |\n", rule.join(" | ")));
        }
    }
    out
}

const STYLE: &str = "\
:root { color-scheme: light dark; }
body { font: 16px/1.6 system-ui, sans-serif; max-width: 46rem; margin: 2rem auto; padding: 0 1rem; }
table { border-collapse: collapse; margin: 1rem 0; font-variant-numeric: tabular-nums; }
th, td { padding: 0.25rem 0.75rem; border-bottom: 1px solid color-mix(in srgb, currentColor 20%, transparent); }
th { border-bottom-width: 2px; }
code, pre { font-family: ui-monospace, monospace; font-size: 0.9em; }
pre { overflow-x: auto; padding: 0.75rem; border: 1px solid color-mix(in srgb, currentColor 20%, transparent); }
";

/// A Markdown document as an HTML page.
fn page(markdown: &str, title: &str) -> String {
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, Parser::new_ext(markdown, extensions()));
    let safe = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{safe}</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n{body}</body>\n</html>\n"
    )
}

/// The first heading of a document, as plain text.
fn first_heading(markdown: &str) -> Option<String> {
    let mut inside = false;
    let mut title = String::new();
    for event in Parser::new_ext(markdown, extensions()) {
        match event {
            Event::Start(Tag::Heading { .. }) => inside = true,
            Event::End(TagEnd::Heading(_)) => return Some(title),
            Event::Text(t) | Event::Code(t) if inside => title.push_str(&t),
            _ => {}
        }
    }
    None
}

/// Reads a sheet the front matter names, by the path written there: its
/// name for diagnostics and its text, or why it cannot be read.
pub type Load<'a> = dyn Fn(&str) -> Result<(String, String), String> + 'a;

/// Render the Markdown document `text`, called `name`, with its Omasheet
/// content calculated.
///
/// Any diagnostic means the document was not rendered.
pub fn render(name: &str, text: &str, load: &Load, format: Format, options: Options) -> Outcome {
    let mut out = Outcome::default();
    let mut sources = Sources::new();
    let md = sources.add(name, text);
    let front = front_matter(text);
    let (blocks, skip) = scan(text);

    // The files the front matter names, then the document's own blocks:
    // the text of the document with all but the blocks blanked out, so
    // that every place in a block is the same place in the document.
    let mut parts: Vec<(String, u32)> = Vec::new();
    let mut unread = Vec::new();
    for (path, range) in &front.sheets {
        match load(path) {
            Ok((name, sheet)) => {
                let src = sources.add(name, sheet.clone());
                parts.push((sheet, src));
            }
            Err(why) => unread.push(Diagnostic::new(
                Span::new(md, range.start, range.end),
                format!("cannot read `{path}`: {why}"),
            )),
        }
    }
    if !unread.is_empty() {
        out.errors.extend(unread.iter().map(|d| sources.render(d)));
        return out;
    }
    let mut own: Vec<u8> = text
        .bytes()
        .map(|b| if b == b'\n' { b'\n' } else { b' ' })
        .collect();
    for range in blocks.iter().flat_map(|b| b.source.iter()) {
        own[range.clone()].copy_from_slice(&text.as_bytes()[range.clone()]);
    }
    parts.push((String::from_utf8(own).unwrap_or_default(), md));

    let mut joined = Joined::default();
    let mut zone: Option<String> = None;
    let mut clashes = Vec::new();
    let mut zones = Vec::new();
    for (part, src) in &mut parts {
        if let Some((name, at)) = take_zone(part) {
            let span = Span::new(*src, at, at + name.len());
            match &zone {
                Some(first) if *first != name => clashes.push(
                    Diagnostic::new(
                        span,
                        format!("the document is already in the zone `{first}`"),
                    )
                    .with_help("the sheets of one document share a time zone"),
                ),
                Some(_) => {}
                None => {
                    zones.push((name.clone(), *src, at));
                    zone = Some(name);
                }
            }
        }
    }
    if !clashes.is_empty() {
        out.errors.extend(clashes.iter().map(|d| sources.render(d)));
        return out;
    }
    if let Some((name, src, at)) = zones.first() {
        joined.text.push_str("zone ");
        joined.push(name, *src, *at);
    }
    for (part, src) in &parts {
        joined.push(part, *src, 0);
    }

    // Diagnostics name places in the joined sheet (source 0) or in an
    // expression (source 1) that starts at `base` in the document.
    let locate = |d: &Diagnostic, base: usize| {
        let mut d = d.clone();
        d.span = match d.span.src {
            0 => joined.locate(d.span),
            _ => Span::new(md, d.span.start as usize + base, d.span.end as usize + base),
        };
        sources.render(&d)
    };
    let (program, diags) = compile(&joined.text, 0);
    if !diags.is_empty() {
        out.errors.extend(diags.iter().map(|d| locate(d, 0)));
        return out;
    }
    let engine = Engine::with_options(&program, options);
    engine.run();
    let diags = engine.take_diags();
    if !diags.is_empty() {
        out.errors.extend(diags.iter().map(|d| locate(d, 0)));
        return out;
    }

    // What takes the place of each block and each interpolation.
    let mut changes: Vec<(Range<usize>, String)> = Vec::new();
    for block in &blocks {
        let source: String = block.source.iter().map(|r| &text[r.clone()]).collect();
        let declared = parse_sheet(&source, 0, &mut Vec::new());
        let tables: Vec<String> = declared
            .tables
            .iter()
            .filter_map(|decl| program.tables.iter().position(|t| t.name == decl.name))
            .map(|t| {
                let view = View {
                    table: t,
                    rows: Rc::new((0..program.tables[t].nrows).collect()),
                    cols: Rc::new((0..program.tables[t].cols.len()).collect()),
                };
                table(&engine, &program, &view, format)
            })
            .collect();
        let mut shown = tables.join("\n");
        let mut range = block.range.clone();
        if shown.is_empty() {
            // A block of constants shows nothing, and leaves no gap.
            let rest = &text[range.end..];
            range.end += rest.len() - rest.trim_start_matches(['\n', '\r']).len();
        } else if !text[range.clone()].ends_with('\n') {
            shown.truncate(shown.trim_end().len());
        }
        changes.push((range, shown));
    }
    for (whole, inner) in interpolations(text, &skip) {
        let expr = &text[inner.clone()];
        if expr.trim().is_empty() {
            let d = Diagnostic::new(
                Span::new(md, whole.start, whole.end),
                "there is nothing to evaluate",
            )
            .with_help("write an expression, such as `{{ Sales.Revenue.sum() }}`");
            out.errors.push(sources.render(&d));
            continue;
        }
        let node = match compile_expr(&program, expr, 1) {
            Ok((node, _)) => node,
            Err(diags) => {
                out.errors
                    .extend(diags.iter().map(|d| locate(d, inner.start)));
                continue;
            }
        };
        // A table stands apart from the text around it.
        let apart = |view: &View| {
            let shown = table(&engine, &program, view, format);
            let before = &text[..whole.start];
            let lead = match before {
                b if b.is_empty() || b.ends_with("\n\n") => "",
                b if b.ends_with('\n') => "\n",
                _ => "\n\n",
            };
            let after = &text[whole.end..];
            let tail = if after.is_empty() || after.starts_with('\n') {
                ""
            } else {
                "\n\n"
            };
            format!("{lead}{}{tail}", shown.trim_end())
        };
        let shown = match engine.eval_top(&node) {
            Some(Value::Table(view)) => apart(&view),
            Some(Value::Row(row)) => apart(&View {
                table: row.table,
                rows: Rc::new(vec![row.row]),
                cols: row.cols.clone(),
            }),
            // Several values read as a list: `Jan, Feb`.
            Some(Value::Vector(items)) => {
                let shown: Vec<String> = items.iter().map(|v| engine.show(v, false)).collect();
                escape(&shown.join(", "), format)
            }
            Some(value) => escape(&engine.show(&value, false), format),
            None => {
                let diags = engine.take_diags();
                out.errors
                    .extend(diags.iter().map(|d| locate(d, inner.start)));
                continue;
            }
        };
        changes.push((whole, shown));
    }
    if !out.errors.is_empty() {
        return out;
    }

    changes.sort_by_key(|(range, _)| range.start);
    let mut markdown = String::with_capacity(text.len());
    let mut at = 0;
    for (range, shown) in &changes {
        if range.start < at {
            continue;
        }
        markdown.push_str(&text[at..range.start]);
        markdown.push_str(shown);
        at = range.end;
    }
    markdown.push_str(&text[at..]);

    out.output = match format {
        Format::Markdown => markdown,
        Format::Html => {
            let title = front
                .title
                .or_else(|| first_heading(&markdown))
                .unwrap_or_else(|| name.to_string());
            page(&markdown, &title)
        }
    };
    out
}
