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

/// The sign of a currency where it has one, `£` for `GBP`: as a column of money
/// is shown; any other unit as it is.
fn money_sign(unit: &str) -> String {
    match unit {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "CNY" => "￥",
        "INR" => "₹",
        "KRW" => "₩",
        "THB" => "฿",
        "RUB" => "₽",
        "TRY" => "₺",
        "ILS" => "₪",
        "PHP" => "₱",
        "UAH" => "₴",
        "VND" => "₫",
        "NGN" => "₦",
        other => other,
    }
    .to_string()
}

/// The mark of a column that shows money in pence or cents, `p`, `¢` or `c`, if
/// it does: its values are plotted in that hundredth, as it shows them.
fn sub_mark(col: &omasheet_omx::check::Column, unit: &str) -> Option<&'static str> {
    use omasheet_omx::colfmt::Format;
    let sub = match col.shown_format().as_deref() {
        Some(Format::Quantity(q)) => q.number.and_then(|n| n.sub)?,
        _ => return None,
    };
    Some(match (sub, unit.split('.').next().unwrap_or(unit)) {
        ('p', _) => "p",
        (_, "USD") => "¢",
        _ => "c",
    })
}

/// The palettes a plot may be drawn in, each with its colours as `#rrggbb`.
pub fn palettes() -> Vec<(&'static str, Vec<String>)> {
    omasheet_plot::palettes()
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

/// The columns of that sheet whose values can be plotted: numbers, with
/// empty cells, as far as the first rows say.
pub fn numeric_columns(engine: &Engine, program: &Program, plot: &Plot) -> Vec<String> {
    match engine.eval_top(&plot.data) {
        Some(Value::Sheet(view)) => view
            .cols
            .iter()
            .filter(|&&c| {
                view.rows
                    .iter()
                    .take(2000)
                    .all(|&r| number(&engine.cell_shown(view.sheet, c, r)).is_ok())
            })
            .map(|&c| program.sheets[view.sheet].cols[c].name.clone())
            .collect(),
        _ => {
            engine.take_diags();
            Vec::new()
        }
    }
}

/// The columns of that sheet that have a unit to show: quantities, money and
/// durations.
pub fn unit_columns(engine: &Engine, program: &Program, plot: &Plot) -> Vec<String> {
    match engine.eval_top(&plot.data) {
        Some(Value::Sheet(view)) => view
            .cols
            .iter()
            .filter(|&&c| {
                view.rows.iter().take(2000).any(|&r| match engine.cell_shown(view.sheet, c, r) {
                    Value::Quantity(q) => !q.unit().to_string().is_empty(),
                    Value::Duration(_) => true,
                    _ => false,
                })
            })
            .map(|&c| program.sheets[view.sheet].cols[c].name.clone())
            .collect(),
        _ => {
            engine.take_diags();
            Vec::new()
        }
    }
}

/// Whether the origin is in view on its own along each axis, `x` then `y`: the
/// values there go from below zero to above it, or the kind of plot starts
/// its bars from zero. Then the plot's `origin` has nothing to add.
pub fn origin_in(engine: &Engine, program: &Program, plot: &Plot) -> (bool, bool) {
    let Some(Value::Sheet(view)) = engine.eval_top(&plot.data) else {
        engine.take_diags();
        return (false, false);
    };
    let cols = &program.sheets[view.sheet].cols;
    let find = |name: &str| view.cols.iter().copied().find(|&c| cols[c].name == name);
    let x_col = plot.x.as_deref().and_then(find).or(view.cols.first().copied());
    let y_cols: Vec<usize> = if plot.y.is_empty() {
        view.cols.iter().copied().filter(|&c| Some(c) != x_col).collect()
    } else {
        plot.y.iter().filter_map(|n| find(n)).collect()
    };
    // The reach of a column's numbers: none where one is not.
    let reach = |cs: &[usize]| -> Option<(f64, f64)> {
        let mut r: Option<(f64, f64)> = None;
        for &c in cs {
            for &row in view.rows.iter() {
                match number(&engine.cell_shown(view.sheet, c, row)) {
                    Ok(Some((v, _))) => {
                        r = Some(r.map_or((v, v), |(a, b)| (a.min(v), b.max(v))));
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        }
        r
    };
    let spans = |r: Option<(f64, f64)>| r.is_some_and(|(a, b)| a <= 0.0 && b >= 0.0);
    let from_zero = matches!(plot.kind.as_str(), "bar" | "area" | "stacked" | "histogram" | "waterfall");
    (
        x_col.is_some_and(|c| spans(reach(&[c]))),
        from_zero || spans(reach(&y_cols)),
    )
}

/// A mark of a drawn plot, in its pixels, and the cell it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotHit {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// The sheet, the row of it and the column.
    pub sheet: usize,
    pub row: usize,
    pub col: usize,
}

/// The SVG of a plot of the program, once the engine has run. `size` is the
/// width and height to draw at, in place of the plot's own.
pub fn svg(
    engine: &Engine,
    program: &Program,
    plot: &Plot,
    size: Option<(u32, u32)>,
) -> Result<String, Vec<Diagnostic>> {
    draw(engine, program, plot, size).map(|(svg, _)| svg)
}

/// The SVG of a plot, and the cell each of its marks stands for.
pub fn draw(
    engine: &Engine,
    program: &Program,
    plot: &Plot,
    size: Option<(u32, u32)>,
) -> Result<(String, Vec<PlotHit>), Vec<Diagnostic>> {
    draw_as(engine, program, plot, &plot.kind, size)
}

/// As [`draw`], of kind `kind` in place of the plot's own: to find which kinds
/// its data suits.
pub fn draw_as(
    engine: &Engine,
    program: &Program,
    plot: &Plot,
    kind: &str,
    size: Option<(u32, u32)>,
) -> Result<(String, Vec<PlotHit>), Vec<Diagnostic>> {
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
        // Left unsaid, the columns of numbers: text beside them is no value.
        let rest: Vec<usize> = view.cols.iter().copied().filter(|&c| c != x_col).collect();
        let numbers: Vec<usize> = rest
            .iter()
            .copied()
            .filter(|&c| {
                view.rows.iter().take(2000).all(|&r| number(&engine.cell_shown(view.sheet, c, r)).is_ok())
            })
            .collect();
        if numbers.is_empty() { rest } else { numbers }
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
        Kind::parse(kind).ok_or_else(|| one(plot.span, format!("`{kind}` is not a kind of plot")))?,
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
    spec.labels = plot.labels.into();
    spec.ticks = plot.ticks.into();
    spec.units = plot.units.into();
    spec.origin = plot.origin.into();
    spec.logx = plot.logx;
    spec.logy = plot.logy;
    spec.fit = match plot.fit.as_str() {
        "straight" => omasheet_plot::Fit::Straight,
        "quadratic" => omasheet_plot::Fit::Quadratic,
        "bezier" => omasheet_plot::Fit::Bezier,
        "basic" => omasheet_plot::Fit::None,
        // Left unsaid, a line is smoothed and any other plot is as it is.
        _ if kind == "line" => omasheet_plot::Fit::Bezier,
        _ => omasheet_plot::Fit::None,
    };
    spec.legend = plot.legend;
    spec.palette = plot.palette.clone();
    // The unit of a column: that of its first cell that has one.
    let unit_of = |engine: &Engine, c: usize| -> Option<String> {
        view.rows.iter().find_map(|&r| match engine.cell_shown(view.sheet, c, r) {
            Value::Quantity(q) => {
                Some(money_sign(&omasheet_omx::colfmt::pretty_unit(&q.unit().to_string())))
                    .filter(|u| !u.is_empty())
            }
            Value::Duration(_) => Some("s".to_string()),
            _ => None,
        })
    };

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
        x_unit: if plot.units.0 || plot.units.1 { unit_of(engine, x_col) } else { None },
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
        // Pence or cents, where the column shows them: plotted so, in `p⋅kWh⁻¹`.
        let raw_unit = view
            .rows
            .iter()
            .find_map(|&r| match engine.cell_shown(view.sheet, c, r) {
                Value::Quantity(q) => Some(q.unit().to_string()),
                _ => None,
            })
            .unwrap_or_default();
        let sub = sub_mark(&cols[c], &raw_unit);
        let scale = if sub.is_some() { 100.0 } else { 1.0 };
        let mut series = Series {
            name: cols[c].name.clone(),
            unit: if plot.units.0 || plot.units.1 {
                match (sub, raw_unit.split_once('.')) {
                    (Some(mark), Some((_, rest))) if rest.ends_with("^-1") => {
                        Some(format!("{mark}⋅{}", omasheet_omx::colfmt::pretty_unit(rest)))
                    }
                    (Some(mark), _) => Some(mark.to_string()),
                    _ => unit_of(engine, c),
                }
            } else {
                None
            },
            ..Series::default()
        };
        for &r in view.rows.iter() {
            match number(&engine.cell_shown(view.sheet, c, r)) {
                Ok(cell) => {
                    let (v, s) = cell.map_or((None, 0.0), |(v, s)| (Some(v), s));
                    series.values.push(v.map(|v| v * scale));
                    series.sigma.push(s * scale);
                    series.shown.push(if v.is_some() && plot.labels.1 {
                        let cell = engine.cell_shown(view.sheet, c, r);
                        engine.show_in(&cell, false, cols[c].shown_format().as_deref())
                    } else {
                        String::new()
                    });
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
    let (svg, hits) = omasheet_plot::render_hits(&spec, &table).map_err(|why| one(plot.span, why))?;
    let hits = hits
        .into_iter()
        .filter_map(|h| {
            Some(PlotHit {
                x: h.x,
                y: h.y,
                w: h.w,
                h: h.h,
                sheet: view.sheet,
                row: *view.rows.get(h.row)?,
                col: *y_cols.get(h.series).or(y_cols.first())?,
            })
        })
        .collect();
    Ok((svg, hits))
}
