//! Parser for the line-oriented `.omx` sheet format.
//!
//! ```omx
//! # a comment
//! const TaxRate = 20%
//!
//! table Sales
//!
//! Revenue : Rat
//!
//! Month | Revenue | Cost
//! Jan   | 10000   | 6000
//!
//! Profit := Revenue - Cost
//! ```
//!
//! Cells are kept as raw text here; whether a cell is a literal or a formula
//! depends on the column type and is decided by the checker.

use crate::ast::Expr;
use crate::diag::{Diagnostic, Span};
use crate::parser::parse_expr;
use crate::types::S;

#[derive(Debug, Default)]
pub struct SheetAst {
    pub consts: Vec<ConstDecl>,
    pub tables: Vec<TableDecl>,
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

#[derive(Debug)]
pub struct TableDecl {
    pub name: String,
    pub span: Span,
    pub schema: Vec<SchemaLine>,
    pub header: Vec<(String, Span)>,
    pub rows: Vec<Vec<CellSrc>>,
    /// The header line, the separator line if any, and each kept row's line
    /// (without line endings), for tools that rewrite a table in place.
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

fn is_separator(line: &str) -> bool {
    line.contains('-')
        && line
            .chars()
            .all(|c| matches!(c, '-' | '|' | ':' | ' ' | '\t'))
}

pub fn parse_sheet(text: &str, src: u32, diags: &mut Vec<Diagnostic>) -> SheetAst {
    let mut lines = Vec::new();
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        lines.push(Line {
            start: offset,
            text: raw.trim_end_matches(['\n', '\r']),
        });
        offset += raw.len();
    }

    let mut ast = SheetAst::default();
    let mut table: Option<TableDecl> = None;
    let mut in_body = false; // the header row has been read
    let mut after_header = false; // the next line may be a separator row
    let mut i = 0;

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
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // table <Name>
        if let Some(rest) = after_keyword(trimmed, "table") {
            let n = ident_len(rest);
            if n > 0 && rest[n..].trim().is_empty() {
                ast.tables.extend(table.take());
                let name_at = at + (trimmed.len() - rest.len());
                table = Some(TableDecl {
                    name: rest[..n].to_string(),
                    span: Span::new(src, name_at, name_at + n),
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

        // const <Name> = <expr>
        if let Some(rest) = after_keyword(trimmed, "const") {
            let n = ident_len(rest);
            let tail = rest[n..].trim_start();
            if n > 0 && tail.starts_with('=') && !tail.starts_with("==") {
                ast.tables.extend(table.take());
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

        let Some(t) = table.as_mut() else {
            diags.push(
                Diagnostic::new(
                    Span::new(src, at, at + trimmed.len()),
                    "expected `table <Name>` or `const <Name> = <expression>`",
                )
                .with_help("rows of data belong under a `table` line"),
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
            let ty_text = tail[1..].trim();
            let ty_at = at
                + (trimmed.len() - tail.len())
                + 1
                + (tail[1..].len() - tail[1..].trim_start().len());
            match S::from_name(ty_text) {
                Some(ty) => t.schema.push(SchemaLine {
                    name: trimmed[..n].to_string(),
                    span: Span::new(src, at, at + n),
                    ty,
                }),
                None => diags.push(
                    Diagnostic::new(
                        Span::new(src, ty_at, ty_at + ty_text.len()),
                        format!("unknown type `{ty_text}`"),
                    )
                    .with_help("the types are Int, Rat, Num, Text, Date and Bool"),
                ),
            }
            i += 1;
            continue;
        }

        if !in_body {
            // The header row.
            for cell in split_cells(line.text, line.start, src) {
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

        let cells = split_cells(line.text, line.start, src);
        if cells.len() != t.header.len() {
            diags.push(Diagnostic::new(
                Span::new(src, at, at + trimmed.len()),
                format!(
                    "this row has {} cell{} but table `{}` has {} column{}",
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
    ast.tables.extend(table.take());
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
    fn parses_a_table() {
        let mut diags = Vec::new();
        let src = "const Rate = 20%\n\ntable Sales\n\nRevenue : Rat\n\nMonth | Revenue\n------|--------\nJan | 100\nFeb | 120\n\nTax := Revenue * Rate\n";
        let ast = parse_sheet(src, 0, &mut diags);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(ast.consts.len(), 1);
        let t = &ast.tables[0];
        assert_eq!((t.header.len(), t.rows.len(), t.computed.len()), (2, 2, 1));
        assert_eq!(t.schema[0].ty, S::Rat);
    }

    #[test]
    fn ragged_row_is_an_error() {
        let mut diags = Vec::new();
        parse_sheet("table T\n\nA | B\n1 | 2 | 3\n", 0, &mut diags);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("table `T`"));
    }
}
