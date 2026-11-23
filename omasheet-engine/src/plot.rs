// Copyright (c) 2026 Stephen Roe

//! A `plot` of a file, drawn: the sheet its `data` gives, as the SVG of
//! `omasheet-plot`. A report and the app both draw through [`svg`].

use crate::value::{View, to_float};
use crate::{Engine, Value};
use omasheet_omx::check::Plot;
use omasheet_omx::{Diagnostic, Program};
use omasheet_plot::{Kind, Series, Spec, Table};

/// A cell as a number and its uncertainty: `None` for an empty one, and the
/// kind of what is not a number as an error.
fn number(v: &Value) -> Result<Option<(f64, f64)>, &'static str> {
    Ok(match v {
        Value::Null | Value::Error => None,
        Value::Scientific(x, s) => Some((*x, *s)),
        Value::Quantity(q) => Some((q.value(), q.sigma())),
        // A duration is in seconds.
        Value::Duration(us) => Some((*us as f64 / 1e6, 0.0)),
        other => match to_float(other) {
            Some(x) if x.is_finite() => Some((x, 0.0)),
            Some(_) => None,
            None => return Err(other.kind()),
        },
    })
}

/// The plot named `name`, if the program has one.
pub fn find<'a>(program: &'a Program, name: &str) -> Option<&'a Plot> {
    program.plots.iter().find(|p| p.name == name)
}

/// The names of the columns of the sheet a plot is drawn from, once the
/// engine has run; none where its `data` does not give a sheet.
pub fn columns(engine: &Engine, program: &Program, plot: &Plot) -> Vec<String> {
    match engine.eval_top(&plot.data) {
        Some(Value::Sheet(view)) => view
            .cols
            .iter()
            .map(|&c| program.sheets[view.sheet].cols[c].name.clone())
            .collect(),
        _ => {
            engine.take_diags();
            Vec::new()
        }
    }
}

/// The SVG of a plot of the program, once the engine has run. `size` is the
/// width and height to draw at, in place of the plot's own.
pub fn svg(
    engine: &Engine,
    program: &Program,
    plot: &Plot,
    size: Option<(u32, u32)>,
) -> Result<String, Vec<Diagnostic>> {
    let one = |span, message: String| vec![Diagnostic::new(span, message)];
    let view: View = match engine.eval_top(&plot.data) {
        Some(Value::Sheet(view)) => view,
        Some(other) => {
            return Err(one(
                plot.data_span,
                format!("`data` gives {}, and a plot needs a sheet", other.kind()),
            ));
        }
        None => {
            let diags = engine.take_diags();
            return Err(if diags.is_empty() {
                one(plot.data_span, "`data` could not be calculated".to_string())
            } else {
                diags
            });
        }
    };

    let cols = &program.sheets[view.sheet].cols;
    let find = |name: &str| view.cols.iter().copied().find(|&c| cols[c].name == name);
    let x_col = match &plot.x {
        Some(name) => find(name),
        None => view.cols.first().copied(),
    };
    let Some(x_col) = x_col else {
        return Err(one(plot.span, "the plot has no column for its labels".to_string()));
    };
    let y_cols: Vec<usize> = if plot.y.is_empty() {
        view.cols.iter().copied().filter(|&c| c != x_col).collect()
    } else {
        plot.y.iter().filter_map(|n| find(n)).collect()
    };
    if y_cols.is_empty() {
        return Err(one(
            plot.data_span,
            "`data` has no column of values besides `x`".to_string(),
        ));
    }

    let mut spec = Spec::new(
        Kind::parse(&plot.kind).ok_or_else(|| one(plot.span, format!("`{}` is not a kind of plot", plot.kind)))?,
    );
    spec.title = plot.title.clone();
    if let Some((w, h)) = size {
        (spec.width, spec.height) = (w, h);
    } else {
        spec.width = plot.width.unwrap_or(spec.width);
        spec.height = plot.height.unwrap_or(spec.height);
    }
    spec.k = plot.k.unwrap_or(spec.k);
    spec.bins = plot.bins;

    let mut table = Table {
        labels: view
            .rows
            .iter()
            .map(|&r| {
                let v = engine.cell_shown(view.sheet, x_col, r);
                engine.show_in(&v, false, cols[x_col].shown_format().as_deref())
            })
            .collect(),
        series: Vec::new(),
        xs: None,
    };
    // The labels are numbers where every filled one is.
    let xs: Vec<Option<f64>> = view
        .rows
        .iter()
        .map(|&r| number(&engine.cell_shown(view.sheet, x_col, r)).ok().flatten().map(|n| n.0))
        .collect();
    let numeric = view.rows.iter().zip(&xs).all(|(&r, x)| {
        x.is_some() || matches!(engine.cell_shown(view.sheet, x_col, r), Value::Null)
    });
    if numeric && xs.iter().any(Option::is_some) {
        table.xs = Some(xs);
    }
    for &c in &y_cols {
        let mut series = Series {
            name: cols[c].name.clone(),
            ..Series::default()
        };
        for &r in view.rows.iter() {
            match number(&engine.cell_shown(view.sheet, c, r)) {
                Ok(cell) => {
                    let (v, s) = cell.map_or((None, 0.0), |(v, s)| (Some(v), s));
                    series.values.push(v);
                    series.sigma.push(s);
                }
                Err(kind) => {
                    return Err(one(
                        plot.span,
                        format!("column `{}` holds {kind}, which cannot be plotted", cols[c].name),
                    ));
                }
            }
        }
        table.series.push(series);
    }
    omasheet_plot::render(&spec, &table).map_err(|why| one(plot.span, why))
}
