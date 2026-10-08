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

/// Cells are `{d: display, s: source, n: numeric, f: formula, e: error}`.
pub fn snapshot(snap: &Snapshot) -> String {
    let mut out = String::from("{\"tables\":");
    list(&mut out, &snap.tables, |out, t| {
        out.push('{');
        field(out, "name", &t.name);
        out.push_str(",\"columns\":");
        list(out, &t.columns, |out, c| {
            out.push('{');
            field(out, "name", &c.name);
            out.push(',');
            field(out, "type", &c.ty);
            out.push(',');
            field(out, "formula", c.formula.as_deref().unwrap_or(""));
            out.push_str(&format!(",\"computed\":{}}}", c.formula.is_some()));
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
        field(out, "error", c.error.as_deref().unwrap_or(""));
        out.push('}');
    });
    out.push_str(",\"problems\":");
    list(&mut out, &snap.problems, |out, p| {
        out.push_str(&format!("{{\"line\":{},\"column\":{},", p.line, p.column));
        field(out, "message", &p.message);
        out.push('}');
    });
    out.push('}');
    out
}

/// The function directory: `[{name, category, usage, summary}]`. The
/// functions the sheet defines come first, under `Custom`.
pub fn functions(custom: &[FuncSnap], docs: &[FuncDoc]) -> String {
    let mut entries: Vec<[String; 4]> = Vec::new();
    for f in custom {
        // What it does, in the author's words if there are any.
        let summary = if f.doc.is_empty() {
            format!("= {}", f.source)
        } else {
            f.doc.clone()
        };
        entries.push([f.name.clone(), "Custom".into(), f.usage.clone(), summary]);
    }
    for f in docs {
        entries.push([f.name, f.category, f.usage, f.summary].map(String::from));
    }
    let mut out = String::new();
    list(
        &mut out,
        &entries,
        |out, [name, category, usage, summary]| {
            out.push('{');
            field(out, "name", name);
            out.push(',');
            field(out, "category", category);
            out.push(',');
            field(out, "usage", usage);
            out.push(',');
            field(out, "summary", summary);
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
        let doc = Document::from_text("table T\n\nA | B\n\"say \\\"hi\\\"\" | 2\n\nC := B * 2\n");
        let json = super::snapshot(doc.snapshot());
        assert!(json.starts_with("{\"tables\":[{\"name\":\"T\",\"columns\":[{\"name\":\"A\""));
        assert!(json.contains("\"d\":\"say \\\"hi\\\"\""), "{json}");
        assert!(json.contains("\"formula\":\"B * 2\",\"computed\":true"));
        assert!(json.contains("\"d\":\"4\""));
        assert!(json.ends_with("\"consts\":[],\"problems\":[]}"));
    }

    #[test]
    fn the_sheets_own_functions_lead_the_directory() {
        let doc =
            Document::from_text("# Twice over.\nfunc Twice(x) = x * 2\nfunc Half(x) = x / 2\n");
        let json = super::functions(
            &doc.snapshot().funcs,
            omasheet_engine::omx::funcs::FUNCTIONS,
        );
        assert!(json.starts_with(
            "[{\"name\":\"Twice\",\"category\":\"Custom\",\"usage\":\"Twice(x)\",\"summary\":\"Twice over.\"},"
        ));
        assert!(json.contains("\"usage\":\"Half(x)\",\"summary\":\"= x / 2\"},{\"name\":\"sum\""));
    }
}
