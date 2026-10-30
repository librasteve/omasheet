// Copyright (c) 2026 Stephen Roe

//! The document snapshot as JSON, for the QML side to parse.

use omasheet_engine::doc::{FuncSnap, Snapshot};
use omasheet_engine::omx::funcs::FuncDoc;

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn field(out: &mut String, name: &str, value: &str) {
    string(out, name);
    out.push(':');
    string(out, value);
}

fn list<T>(out: &mut String, items: &[T], mut each: impl FnMut(&mut String, &T)) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        each(out, item);
    }
    out.push(']');
}

/// Row numbers, as a list.
pub fn numbers(items: &[usize]) -> String {
    let mut out = String::new();
    list(&mut out, items, |out, n| out.push_str(&n.to_string()));
    out
}

/// Texts, as a list.
pub fn texts(items: &[String]) -> String {
    let mut out = String::new();
    list(&mut out, items, |out, t| string(out, t));
    out
}

/// Cells are `{d: display, s: source, n: numeric, f: formula, e: error,
/// x: the value in full when the display is shortened, h: text that is
/// Markdown as HTML}`.
pub fn snapshot(snap: &Snapshot) -> String {
    let mut out = String::from("{\"sheets\":");
    list(&mut out, &snap.sheets, |out, t| {
        out.push('{');
        field(out, "name", &t.name);
        out.push(',');
        // What a sheet's rows come from, where they are not its own: the
        // expression of a sheet calculated from one, or a table's file.
        field(out, "view", t.view.as_deref().unwrap_or(""));
        out.push(',');
        field(out, "from", t.source.as_deref().unwrap_or(""));
        out.push_str(",\"columns\":");
        list(out, &t.columns, |out, c| {
            out.push('{');
            field(out, "name", &c.name);
            out.push(',');
            field(out, "type", &c.ty);
            out.push(',');
            field(out, "formula", c.formula.as_deref().unwrap_or(""));
            out.push(',');
            field(out, "format", c.format.as_deref().unwrap_or(""));
            out.push_str(&format!(
                ",\"computed\":{},\"declared\":{}}}",
                c.formula.is_some(),
                c.declared
            ));
        });
        out.push_str(",\"rows\":");
        list(out, &t.rows, |out, row| {
            list(out, row, |out, cell| {
                out.push('{');
                field(out, "d", &cell.display);
                out.push(',');
                field(out, "s", &cell.source);
                out.push(',');
                field(out, "e", cell.error.as_deref().unwrap_or(""));
                out.push(',');
                // A decimal that does not end is given to more places.
                let long = omasheet_engine::value::long_decimal(&cell.value);
                let full = long
                    .as_deref()
                    .or(cell.exact.as_deref().filter(|_| cell.shortened));
                field(out, "x", full.unwrap_or(""));
                if cell.value.is_markdown() {
                    out.push(',');
                    // The grid draws `<i>`, and has no `<em>`.
                    let html = omasheet_md::inline_html(&cell.display)
                        .replace("<em>", "<i>")
                        .replace("</em>", "</i>");
                    field(out, "h", &html);
                }
                out.push_str(&format!(",\"n\":{},\"f\":{}}}", cell.numeric, cell.formula));
            });
        });
        out.push('}');
    });
    out.push_str(",\"consts\":");
    list(&mut out, &snap.consts, |out, c| {
        out.push('{');
        field(out, "name", &c.name);
        out.push(',');
        field(out, "source", &c.source);
        out.push(',');
        field(out, "display", &c.display);
        out.push(',');
        field(out, "exact", c.exact.as_deref().unwrap_or(""));
        out.push(',');
        field(out, "error", c.error.as_deref().unwrap_or(""));
        out.push('}');
    });
    out.push_str(",\"problems\":");
    list(&mut out, &snap.problems, |out, p| {
        out.push_str(&format!("{{\"line\":{},\"column\":{},", p.line, p.column));
        field(out, "message", &p.message);
        // Where to go to reach it, or -1 where no cell holds it.
        let (sheet, row, col) = match p.cell {
            Some((t, r, c)) => (t as i64, r as i64, c as i64),
            None => (-1, -1, -1),
        };
        out.push_str(&format!(",\"sheet\":{sheet},\"row\":{row},\"col\":{col}}}"));
    });
    out.push('}');
    out
}

/// The function directory: `[{name, category, about, usage, summary, code}]`.
/// The functions the file defines come first, under `Custom`, each with the
/// expression it stands for as its `code`. `about` is what the functions of
/// the category are applied to, from `about`.
pub fn functions(custom: &[FuncSnap], docs: &[FuncDoc], about: &[(&str, &str)]) -> String {
    let mut entries: Vec<[String; 6]> = Vec::new();
    for f in custom {
        let code = format!("= {}", f.source);
        entries.push([
            f.name.clone(),
            "Custom".into(),
            String::new(),
            f.usage.clone(),
            f.doc.clone(),
            code,
        ]);
    }
    for f in docs {
        let note = about.iter().find(|(c, _)| *c == f.category).map_or("", |(_, a)| a);
        entries.push([f.name, f.category, note, f.usage, f.summary, ""].map(String::from));
    }
    let mut out = String::new();
    list(
        &mut out,
        &entries,
        |out, [name, category, about, usage, summary, code]| {
            out.push('{');
            field(out, "name", name);
            out.push(',');
            field(out, "category", category);
            out.push(',');
            field(out, "about", about);
            out.push(',');
            field(out, "usage", usage);
            out.push(',');
            field(out, "summary", summary);
            out.push(',');
            field(out, "code", code);
            out.push('}');
        },
    );
    out
}

#[cfg(test)]
mod tests {
    use omasheet_engine::Document;

    #[test]
    fn escapes_and_structure() {
        let doc =
            Document::from_text("sheet T\n\nA | B | C\n\"say \\\"hi\\\"\" | 2 | *\n\nC := B * 2\n");
        let json = super::snapshot(doc.snapshot());
        // A sheet says what its rows come from before its columns: the
        // expression of one calculated from an expression, or a table's
        // file. Both empty for a sheet whose rows are its own.
        assert!(json.starts_with(
            "{\"sheets\":[{\"name\":\"T\",\"view\":\"\",\"from\":\"\",\"columns\":[{\"name\":\"A\""
        ));
        assert!(json.contains("\"d\":\"say \\\"hi\\\"\""), "{json}");
        assert!(json.contains(
            "\"formula\":\"B * 2\",\"format\":\"\",\"computed\":true,\"declared\":false"
        ));
        assert!(json.contains("\"d\":\"4\""));
        assert!(json.ends_with("\"consts\":[],\"problems\":[]}"));
    }

    #[test]
    fn a_calculated_sheet_says_what_it_is_calculated_from() {
        let doc =
            Document::from_text("sheet T\n\nA | B\n1 | 2\n3 | 4\n\nsheet Top := T |> head(1)\n");
        let json = super::snapshot(doc.snapshot());
        // The tab marks it and the bar says it, both from this one field.
        assert!(
            json.contains("\"name\":\"Top\",\"view\":\"T |> head(1)\""),
            "{json}"
        );
        // And it holds the row the view keeps, no more.
        let top = json.split("\"name\":\"Top\"").nth(1).unwrap();
        assert!(top.contains("\"d\":\"1\""), "{top}");
        assert!(!top.contains("\"d\":\"3\""), "{top}");
    }

    #[test]
    fn the_files_own_functions_lead_the_directory() {
        let doc = Document::from_text("# Twice over.\nfn Twice(x) = x * 2\nfn Half(x) = x / 2\n");
        let json = super::functions(
            &doc.snapshot().funcs,
            omasheet_engine::omx::funcs::FUNCTIONS,
            omasheet_engine::omx::funcs::ABOUT,
        );
        assert!(json.starts_with(
            "[{\"name\":\"Twice\",\"category\":\"Custom\",\"about\":\"\",\"usage\":\"Twice(x)\",\"summary\":\"Twice over.\",\"code\":\"= x * 2\"},"
        ));
        // The code is there with or without a comment, and only for these.
        assert!(json.contains(
            "\"usage\":\"Half(x)\",\"summary\":\"\",\"code\":\"= x / 2\"},{\"name\":\"sum\""
        ));
        assert!(json.contains("\"name\":\"sum\",\"category\":\"Aggregate\",\"about\":\"Object:"));
        assert!(json.ends_with("\"code\":\"\"}]"));
    }
}
