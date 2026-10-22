// Copyright (c) 2026 Stephen Roe

//! Parser for the line-oriented `.omx` file format.
//!
//! ```omx
//! # a comment
//! zone Europe/London
//! const TaxRate = 20%
//! fn Margin(revenue, cost) = (revenue - cost) / revenue
//!
//! sheet Sales
//!
//! Revenue : Decimal
//!
//! Month | Revenue | Cost | Profit
//! Jan   | 10000   | 6000 | *
//!
//! Profit := Revenue - Cost
//! ```
//!
//! Cells are kept as raw text here; whether a cell is a literal or a formula
//! depends on the column type and is decided by the checker, as is whether a
//! column of the header row is a formula column, with `*` for its cells.

use crate::ast::Expr;
use crate::diag::{Diagnostic, Span};
use crate::parser::parse_expr;
use crate::types::S;

#[derive(Debug, Default)]
pub struct FileAst {
    /// The time zone the file's date-times are in, if it names one.
    pub zone: Option<(String, Span)>,
    pub consts: Vec<ConstDecl>,
    pub funcs: Vec<FuncDecl>,
    pub sheets: Vec<SheetDecl>,
}

#[derive(Debug)]
pub struct ConstDecl {
    pub name: String,
    pub span: Span,
    /// `None` if the expression did not parse.
    pub expr: Option<Expr>,
    /// The source text of the expression, parsed or not.
    pub expr_span: Span,
}

/// `fn <Name>(<parameters>) = <expr>`
#[derive(Debug)]
pub struct FuncDecl {
    pub name: String,
    pub span: Span,
    pub params: Vec<(String, Span)>,
    /// `None` if the expression did not parse.
    pub expr: Option<Expr>,
    pub expr_span: Span,
    /// The comment lines directly above the definition, joined.
    pub doc: String,
}

/// `table <Name> := dataframe["<path>"]`: where a sheet's rows come from,
/// when they are not in the file.
#[derive(Debug)]
pub struct SourceDecl {
    /// `None` if the expression did not parse.
    pub expr: Option<Expr>,
    /// The source text of the expression, parsed or not.
    pub expr_span: Span,
}

#[derive(Debug)]
pub struct SheetDecl {
    pub name: String,
    pub span: Span,
    /// `Some` for a `table`: its rows live outside the file.
    pub source: Option<SourceDecl>,
    pub schema: Vec<SchemaLine>,
    pub header: Vec<(String, Span)>,
    pub rows: Vec<Vec<CellSrc>>,
    /// The header line, the separator line if any, and each kept row's line
    /// (without line endings), for tools that rewrite a sheet in place.
    pub header_line: Span,
    pub separator_line: Option<Span>,
    pub row_lines: Vec<Span>,
    pub computed: Vec<ComputedDecl>,
}

#[derive(Debug)]
pub struct SchemaLine {
    pub name: String,
    pub span: Span,
    pub ty: S,
    /// The name the file holds this column under, when it is not the name
    /// used here: `Siblings : Int from "Siblings/Spouses Aboard"`. Only a
    /// table's column is read from a file, so only a table's line has one.
    pub from: Option<String>,
    /// The quoted name, for a diagnostic about it; default without one.
    pub from_span: Span,
}

#[derive(Debug)]
pub struct ComputedDecl {
    pub name: String,
    pub span: Span,
    pub expr: Option<Expr>,
    pub expr_span: Span,
}

/// One cell: its text with surrounding whitespace removed, and where it is.
#[derive(Debug)]
pub struct CellSrc {
    pub text: String,
    pub span: Span,
}

struct Line<'a> {
    start: usize,
    text: &'a str,
}

pub fn ident_len(s: &str) -> usize {
    let b = s.as_bytes();
    if b.is_empty() || !(b[0].is_ascii_alphabetic() || b[0] == b'_') {
        return 0;
    }
    b.iter()
        .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_')
        .count()
}

/// The text of a double-quoted string at the start of `s`, and the length
/// of the string as written. `\\"` and `\\\\` stand for what they quote, as in
/// an expression. `None` if `s` does not open with a quote or does not close.
fn quoted(s: &str) -> Option<(String, usize)> {
    let b = s.as_bytes();
    if b.first() != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'"' => return Some((out, i + 1)),
            b'\\' if i + 1 < b.len() => {
                out.push(b[i + 1] as char);
                i += 2;
            }
            _ => {
                let c = s[i..].chars().next()?;
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    None
}

/// `keyword` followed by whitespace: the rest of the line, trimmed at the left.
fn after_keyword<'a>(s: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = s.strip_prefix(keyword)?;
    rest.starts_with([' ', '\t']).then(|| rest.trim_start())
}

/// Net count of unclosed `(` and `[` outside strings and comments.
fn open_brackets(s: &str) -> i32 {
    let mut depth = 0;
    let mut in_string = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => in_string = !in_string,
            '#' if !in_string => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '(' | '[' if !in_string => depth += 1,
            ')' | ']' if !in_string => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// Split a row at `|`, ignoring `|` inside strings and the `|>` operator.
pub fn split_cells(line: &str, start: usize, src: u32) -> Vec<CellSrc> {
    let b = line.as_bytes();
    let mut cells = Vec::new();
    let mut from = 0;
    let mut in_string = false;
    let mut i = 0;
    let mut push = |from: usize, to: usize| {
        let raw = &line[from..to];
        let lead = raw.len() - raw.trim_start().len();
        let text = raw.trim();
        cells.push(CellSrc {
            text: text.to_string(),
            span: Span::new(src, start + from + lead, start + from + lead + text.len()),
        });
    };
    while i < b.len() {
        match b[i] {
            b'\\' if in_string => i += 1,
            b'"' => in_string = !in_string,
            b'|' if !in_string && b.get(i + 1) != Some(&b'>') => {
                push(from, i);
                from = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    push(from, b.len());
    cells
}

/// Split a parameter list at `,`.
fn split_params(inner: &str, start: usize, src: u32) -> Vec<CellSrc> {
    let mut out = Vec::new();
    let mut from = 0;
    for part in inner.split(',') {
        let lead = part.len() - part.trim_start().len();
        let text = part.trim();
        out.push(CellSrc {
            text: text.to_string(),
            span: Span::new(src, start + from + lead, start + from + lead + text.len()),
        });
        from += part.len() + 1;
    }
    out
}

/// A line of a sheet written with outer pipes, `| a | b |`, without its
/// first and last `|`: where what is left starts in the line, and it.
fn strip_outer(line: &str) -> (usize, &str) {
    let lead = line.len() - line.trim_start().len();
    let Some(inner) = line.trim().strip_prefix('|') else {
        return (0, line);
    };
    (lead + 1, inner.strip_suffix('|').unwrap_or(inner))
}

fn is_separator(line: &str) -> bool {
    line.contains('-')
        && line
            .chars()
            .all(|c| matches!(c, '-' | '|' | ':' | ' ' | '\t'))
}

/// Whether `s` could be the name of a time zone, such as `Europe/London`,
/// `UTC` or `Etc/GMT+5`. Whether there is such a zone is for the engine.
pub fn is_zone_name(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/_-+".contains(c))
}

pub fn parse_file(text: &str, src: u32, diags: &mut Vec<Diagnostic>) -> FileAst {
    let mut lines = Vec::new();
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        lines.push(Line {
            start: offset,
            text: raw.trim_end_matches(['\n', '\r']),
        });
        offset += raw.len();
    }

    let mut ast = FileAst::default();
    let mut sheet: Option<SheetDecl> = None;
    let mut in_body = false; // the header row has been read
    let mut after_header = false; // the next line may be a separator row
    let mut outer = false; // the sheet's lines start and end with `|`
    let mut i = 0;
    // The comment lines directly above the line being read.
    let mut doc: Vec<&str> = Vec::new();

    // An expression that starts at `from` on line `i`, plus continuation
    // lines: indented lines, and any lines while a bracket is still open.
    let expression = |i: &mut usize, from: usize, diags: &mut Vec<Diagnostic>| {
        let mut end = lines[*i].start + lines[*i].text.len();
        while *i + 1 < lines.len() {
            let next = &lines[*i + 1];
            let open = open_brackets(&text[from..end]) > 0;
            let indented = next.text.starts_with([' ', '\t']) && !next.text.trim().is_empty();
            if !(open || indented) {
                break;
            }
            *i += 1;
            end = next.start + next.text.len();
        }
        let lead = text[from..end].len() - text[from..end].trim_start().len();
        let span = Span::new(
            src,
            from + lead,
            from + text[from..end].trim_end().len().max(lead),
        );
        let expr = match parse_expr(&text[from..end], src, from) {
            Ok(e) => Some(e),
            Err(d) => {
                diags.push(d);
                None
            }
        };
        (expr, span)
    };

    while i < lines.len() {
        let line = &lines[i];
        let trimmed = line.text.trim();
        let lead = line.text.len() - line.text.trim_start().len();
        let at = line.start + lead;
        if let Some(comment) = trimmed.strip_prefix('#') {
            doc.push(comment.trim());
            i += 1;
            continue;
        }
        let above = std::mem::take(&mut doc);
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // sheet <Name>
        if let Some(rest) = after_keyword(trimmed, "sheet") {
            let n = ident_len(rest);
            if n > 0 && rest[n..].trim().is_empty() {
                ast.sheets.extend(sheet.take());
                let name_at = at + (trimmed.len() - rest.len());
                sheet = Some(SheetDecl {
                    name: rest[..n].to_string(),
                    span: Span::new(src, name_at, name_at + n),
                    source: None,
                    schema: Vec::new(),
                    header: Vec::new(),
                    header_line: Span::default(),
                    separator_line: None,
                    row_lines: Vec::new(),
                    rows: Vec::new(),
                    computed: Vec::new(),
                });
                in_body = false;
                after_header = false;
                i += 1;
                continue;
            }
        }

        // table <Name> := <expr>
        if let Some(rest) = after_keyword(trimmed, "table") {
            let n = ident_len(rest);
            let tail = rest[n..].trim_start();
            if n > 0 && tail.starts_with(":=") {
                ast.sheets.extend(sheet.take());
                let name_at = at + (trimmed.len() - rest.len());
                let expr_at = at + (trimmed.len() - tail.len()) + 2;
                let (expr, expr_span) = expression(&mut i, expr_at, diags);
                sheet = Some(SheetDecl {
                    name: rest[..n].to_string(),
                    span: Span::new(src, name_at, name_at + n),
                    source: Some(SourceDecl { expr, expr_span }),
                    schema: Vec::new(),
                    header: Vec::new(),
                    header_line: Span::default(),
                    separator_line: None,
                    row_lines: Vec::new(),
                    rows: Vec::new(),
                    computed: Vec::new(),
                });
                in_body = false;
                after_header = false;
                i += 1;
                continue;
            }
        }

        // zone <Area/City>, before the first sheet
        if let Some(rest) = after_keyword(trimmed, "zone").filter(|_| sheet.is_none())
            && ast.sheets.is_empty()
            && is_zone_name(rest.trim())
        {
            let name = rest.trim();
            let name_at = at + (trimmed.len() - rest.len());
            let span = Span::new(src, name_at, name_at + name.len());
            if ast.zone.is_some() {
                diags.push(Diagnostic::new(span, "the file already has a `zone`"));
            } else {
                ast.zone = Some((name.to_string(), span));
            }
            i += 1;
            continue;
        }

        // const <Name> = <expr>
        if let Some(rest) = after_keyword(trimmed, "const") {
            let n = ident_len(rest);
            let tail = rest[n..].trim_start();
            if n > 0 && tail.starts_with('=') && !tail.starts_with("==") {
                ast.sheets.extend(sheet.take());
                let name_at = at + (trimmed.len() - rest.len());
                let expr_at = at + (trimmed.len() - tail.len()) + 1;
                let (expr, expr_span) = expression(&mut i, expr_at, diags);
                ast.consts.push(ConstDecl {
                    name: rest[..n].to_string(),
                    span: Span::new(src, name_at, name_at + n),
                    expr,
                    expr_span,
                });
                i += 1;
                continue;
            }
        }

        // fn <Name>(<parameters>) = <expr>
        if let Some(rest) = after_keyword(trimmed, "fn") {
            let n = ident_len(rest);
            if n > 0 && rest[n..].trim_start().starts_with('(') {
                let name_at = at + (trimmed.len() - rest.len());
                let open = name_at + n + rest[n..].find('(').unwrap_or(0);
                let close = rest[n..].find(')').map(|k| name_at + n + k);
                let tail = close.map_or("", |k| {
                    text[k + 1..line.start + line.text.len()].trim_start()
                });
                let mut params = Vec::new();
                let mut ok = tail.starts_with('=') && !tail.starts_with("==");
                if let Some(close) = close.filter(|_| ok) {
                    let inner = &text[open + 1..close];
                    if !inner.trim().is_empty() {
                        for cell in split_params(inner, open + 1, src) {
                            ok &= ident_len(&cell.text) == cell.text.len() && !cell.text.is_empty();
                            params.push((cell.text, cell.span));
                        }
                    }
                }
                if !ok {
                    diags.push(
                        Diagnostic::new(
                            Span::new(src, at, at + trimmed.len()),
                            "expected `fn <Name>(<parameters>) = <expression>`",
                        )
                        .with_help(
                            "for example `fn Margin(revenue, cost) = (revenue - cost) / revenue`",
                        ),
                    );
                    i += 1;
                    continue;
                }
                ast.sheets.extend(sheet.take());
                let expr_at = line.start + line.text.len() - tail.len() + 1;
                let (expr, expr_span) = expression(&mut i, expr_at, diags);
                ast.funcs.push(FuncDecl {
                    name: rest[..n].to_string(),
                    span: Span::new(src, name_at, name_at + n),
                    params,
                    expr,
                    expr_span,
                    doc: above.join(" "),
                });
                i += 1;
                continue;
            }
        }

        let Some(t) = sheet.as_mut() else {
            diags.push(
                Diagnostic::new(
                    Span::new(src, at, at + trimmed.len()),
                    "expected `sheet <Name>`, `table <Name> := dataframe[\"<path>\"]`, \
                     `const <Name> = <expression>`, \
                     `fn <Name>(<parameters>) = <expression>` or `zone <Area/City>`",
                )
                .with_help("rows of data belong under a `sheet` line"),
            );
            i += 1;
            continue;
        };

        let n = ident_len(trimmed);
        let tail = trimmed[n..].trim_start();

        // <Column> := <expr>
        if n > 0 && tail.starts_with(":=") {
            let expr_at = at + (trimmed.len() - tail.len()) + 2;
            let (expr, expr_span) = expression(&mut i, expr_at, diags);
            t.computed.push(ComputedDecl {
                name: trimmed[..n].to_string(),
                span: Span::new(src, at, at + n),
                expr,
                expr_span,
            });
            after_header = false;
            i += 1;
            continue;
        }

        // <Column> : <Type>, before the header row only
        if !in_body && n > 0 && tail.starts_with(':') && !trimmed.contains('|') {
            let body = tail[1..].trim();
            let ty_at = at
                + (trimmed.len() - tail.len())
                + 1
                + (tail[1..].len() - tail[1..].trim_start().len());
            // `Int from "Siblings/Spouses Aboard"`: the type, and the name
            // the file holds the column under. A name Polars allows and
            // omasheet does not is reached this way and no other.
            let k = ident_len(body);
            let (ty_text, after) = match after_keyword(body[k..].trim_start(), "from") {
                Some(rest) if k > 0 => (&body[..k], Some(rest)),
                _ => (body, None),
            };
            let mut from = None;
            let mut from_span = Span::default();
            if let Some(rest) = after {
                let rest_at = ty_at + (body.len() - rest.len());
                match quoted(rest) {
                    Some((name, used)) if rest[used..].trim().is_empty() => {
                        from_span = Span::new(src, rest_at, rest_at + used);
                        from = Some(name);
                    }
                    _ => diags.push(
                        Diagnostic::new(
                            Span::new(src, rest_at, rest_at + rest.trim_end().len()),
                            "a `from` names the column in the file".to_string(),
                        )
                        .with_help("the name is in quotes, as `from \"Siblings/Spouses Aboard\"`"),
                    ),
                }
            }
            match S::from_name(ty_text) {
                Some(ty) => t.schema.push(SchemaLine {
                    name: trimmed[..n].to_string(),
                    span: Span::new(src, at, at + n),
                    ty,
                    from,
                    from_span,
                }),
                None => diags.push(
                    Diagnostic::new(
                        Span::new(src, ty_at, ty_at + ty_text.len()),
                        format!("unknown type `{ty_text}`"),
                    )
                    .with_help(
                        "the types are Int, Decimal, Fraction, Percent, Float, String, Date, Time, DateTime and Boolean",
                    ),
                ),
            }
            i += 1;
            continue;
        }

        // The cells of a line of the sheet, inside its outer pipes if it
        // is written with them. A header row cannot start with an empty
        // cell, so a `|` at the start of it says so for the whole sheet.
        if !in_body {
            outer = trimmed.starts_with('|');
        }
        let (inside, cells_text) = if outer {
            strip_outer(line.text)
        } else {
            (0, line.text)
        };

        if !in_body {
            // The header row.
            for cell in split_cells(cells_text, line.start + inside, src) {
                if ident_len(&cell.text) == cell.text.len() && !cell.text.is_empty() {
                    t.header.push((cell.text, cell.span));
                } else {
                    diags.push(
                        Diagnostic::new(
                            cell.span,
                            format!("`{}` is not a valid column name", cell.text),
                        )
                        .with_help("column names use letters, digits and `_`"),
                    );
                    t.header.push((format!("_{}", t.header.len()), cell.span));
                }
            }
            t.header_line = Span::new(src, line.start, line.start + line.text.len());
            in_body = true;
            after_header = true;
            i += 1;
            continue;
        }

        if after_header && is_separator(trimmed) {
            t.separator_line = Some(Span::new(src, line.start, line.start + line.text.len()));
            after_header = false;
            i += 1;
            continue;
        }
        after_header = false;

        let cells = split_cells(cells_text, line.start + inside, src);
        if cells.len() != t.header.len() {
            diags.push(Diagnostic::new(
                Span::new(src, at, at + trimmed.len()),
                format!(
                    "this row has {} cell{} but sheet `{}` has {} column{}",
                    cells.len(),
                    if cells.len() == 1 { "" } else { "s" },
                    t.name,
                    t.header.len(),
                    if t.header.len() == 1 { "" } else { "s" },
                ),
            ));
        } else {
            t.rows.push(cells);
            t.row_lines
                .push(Span::new(src, line.start, line.start + line.text.len()));
        }
        i += 1;
    }
    ast.sheets.extend(sheet.take());
    ast
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_cells_around_strings_and_pipes() {
        let cells = split_cells("a | \"x | y\" | = T |> sum() ", 0, 0);
        let texts: Vec<_> = cells.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["a", "\"x | y\"", "= T |> sum()"]);
        assert_eq!(cells[1].span.start, 4);
    }

    #[test]
    fn parses_a_sheet() {
        let mut diags = Vec::new();
        let src = "const Rate = 20%\n\nsheet Sales\n\nRevenue : Decimal\n\nMonth | Revenue | Tax\n------|---------|----\nJan | 100 | *\nFeb | 120 | *\n\nTax := Revenue * Rate\n";
        let ast = parse_file(src, 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(ast.consts.len(), 1);
        let t = &ast.sheets[0];
        assert_eq!((t.header.len(), t.rows.len(), t.computed.len()), (3, 2, 1));
        assert_eq!(t.schema[0].ty, S::Decimal);
    }

    #[test]
    fn parses_a_sheet_between_outer_pipes() {
        let mut diags = Vec::new();
        let src = "sheet T\n\n| A | B |\n| --- | --- |\n| 1 | \"x | y\" |\n| 2 |  |\n\nsheet One\n\n| Name |\n| Ana |\n|  |\n";
        let ast = parse_file(src, 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        let texts = |t: &SheetDecl| -> Vec<Vec<String>> {
            t.rows
                .iter()
                .map(|r| r.iter().map(|c| c.text.clone()).collect())
                .collect()
        };
        let t = &ast.sheets[0];
        assert_eq!(t.header[1].0, "B");
        assert_eq!(texts(t), [["1", "\"x | y\""], ["2", ""]]);
        assert!(t.separator_line.is_some());
        // Spans still point into the file.
        let span = t.rows[0][1].span;
        assert_eq!(&src[span.start as usize..span.end as usize], "\"x | y\"");
        assert_eq!(texts(&ast.sheets[1]), [["Ana"], [""]]);
        // Without outer pipes, a `|` at the start of a row is an empty cell.
        let ast = parse_file("sheet T\n\nA | B\n| 2\n", 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(texts(&ast.sheets[0]), [["", "2"]]);
    }

    #[test]
    fn parses_a_from_clause() {
        let mut diags = Vec::new();
        let src = "table T := dataframe[\"x.parquet\"]\n\nSiblings : Int from \"Siblings/Spouses Aboard\"\nAge : Float\n";
        let ast = parse_file(src, 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        let schema = &ast.sheets[0].schema;
        assert_eq!(schema[0].ty, S::Int);
        assert_eq!(schema[0].from.as_deref(), Some("Siblings/Spouses Aboard"));
        // The span covers the quoted name, for a diagnostic about it.
        let span = schema[0].from_span;
        assert_eq!(
            &src[span.start as usize..span.end as usize],
            "\"Siblings/Spouses Aboard\""
        );
        // A column of its own name has none.
        assert_eq!(schema[1].from, None);

        // A `from` with no name in quotes says so, and the type still reads.
        let mut diags = Vec::new();
        let ast = parse_file(
            "table T := dataframe[\"x\"]\n\nA : Int from B\n",
            0,
            &mut diags,
        );
        assert_eq!(ast.sheets[0].schema[0].ty, S::Int);
        assert!(
            diags.iter().any(|d| d.message.contains("names the column")),
            "{diags:?}"
        );
    }

    #[test]
    fn parses_a_function() {
        let mut diags = Vec::new();
        let src = "# Profit as a share\n# of revenue.\nfn Margin(revenue, cost) =\n  (revenue - cost) / revenue\nfn Vat() = 20%\n";
        let ast = parse_file(src, 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        let f = &ast.funcs[0];
        assert_eq!(f.name, "Margin");
        assert_eq!(f.doc, "Profit as a share of revenue.");
        let names: Vec<_> = f.params.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(names, ["revenue", "cost"]);
        assert_eq!(
            &src[f.params[1].1.start as usize..f.params[1].1.end as usize],
            "cost"
        );
        assert!(f.expr.is_some());
        assert!(ast.funcs[1].params.is_empty() && ast.funcs[1].doc.is_empty());

        parse_file("fn Bad(a b) = 1\n", 0, &mut diags);
        parse_file("fn Bad(a) 1\n", 0, &mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn ragged_row_is_an_error() {
        let mut diags = Vec::new();
        parse_file("sheet T\n\nA | B\n1 | 2 | 3\n", 0, &mut diags);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("sheet `T`"));
    }
}
