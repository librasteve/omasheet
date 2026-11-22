// Copyright (c) 2026 Stephen Roe

//! Plots of sheet data as SVG: line, bar, pie and candlestick.
//!
//! The marks (lines, bars, candles, error bars) are drawn by `plotters`, in
//! the data's own coordinates. Everything with text in it (the axes' labels,
//! the title, the legend) is written here as SVG `<text>`, because
//! `plotters` measures text with a system font and this crate has none: the
//! text is set by whatever shows the SVG. The ink is `currentColor` and the
//! background is clear, so a plot follows the page's light or dark.

use plotters::coord::Shift;
use plotters::coord::types::RangedCoordf64;
use plotters::prelude::*;
use std::fmt::Write as _;

/// What kind of plot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Line,
    Bar,
    Pie,
    /// Of an uncertain value (`value ± σ`), or of open, high, low and close.
    Candle,
}

impl Kind {
    pub const NAMES: &'static str = "line, bar, pie, candle";

    pub fn parse(name: &str) -> Option<Kind> {
        Some(match name {
            "line" => Kind::Line,
            "bar" => Kind::Bar,
            "pie" => Kind::Pie,
            "candle" => Kind::Candle,
            _ => return None,
        })
    }
}

/// How a plot is drawn.
#[derive(Clone, Debug)]
pub struct Spec {
    pub kind: Kind,
    pub title: Option<String>,
    pub width: u32,
    pub height: u32,
    /// The multiple of the uncertainty a candle's body covers; its wick is
    /// twice as far.
    pub k: f64,
}

impl Spec {
    pub fn new(kind: Kind) -> Spec {
        Spec {
            kind,
            title: None,
            width: 640,
            height: 360,
            k: 1.0,
        }
    }
}

/// One column of values. A gap is `None`; `sigma` is the uncertainty of each
/// value, 0 where there is none.
#[derive(Clone, Debug, Default)]
pub struct Series {
    pub name: String,
    pub values: Vec<Option<f64>>,
    pub sigma: Vec<f64>,
}

/// What is plotted: a label for each row, and the columns of values.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub labels: Vec<String>,
    pub series: Vec<Series>,
}

const PALETTE: [(u8, u8, u8); 8] = [
    (0x4E, 0x79, 0xA7),
    (0xF2, 0x8E, 0x2B),
    (0x59, 0xA1, 0x4F),
    (0xE1, 0x57, 0x59),
    (0x76, 0xB7, 0xB2),
    (0xB0, 0x7A, 0xA1),
    (0xED, 0xC9, 0x48),
    (0x9C, 0x75, 0x5F),
];

fn colour(i: usize) -> RGBColor {
    let (r, g, b) = PALETTE[i % PALETTE.len()];
    RGBColor(r, g, b)
}

fn hex(i: usize) -> String {
    let (r, g, b) = PALETTE[i % PALETTE.len()];
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A number as an axis shows it: no more places than its step needs.
fn tick_text(v: f64, step: f64) -> String {
    if v != 0.0 && (v.abs() >= 1e9 || v.abs() < 1e-6) {
        return format!("{v:e}");
    }
    let places = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    format!("{v:.places$}")
}

/// Round numbers from `lo` to `hi`, about `n` of them, and the step.
fn nice_ticks(lo: f64, hi: f64, n: usize) -> (Vec<f64>, f64) {
    let span = (hi - lo).max(f64::MIN_POSITIVE);
    let raw = span / n.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let first = (lo / step).ceil() as i64;
    let last = (hi / step).floor() as i64;
    ((first..=last).map(|i| i as f64 * step).collect(), step)
}

type Chart<'a> = ChartContext<'a, SVGBackend<'a>, Cartesian2d<RangedCoordf64, RangedCoordf64>>;

/// The SVG of a plot, or why there is none.
pub fn render(spec: &Spec, table: &Table) -> Result<String, String> {
    if table.labels.is_empty() {
        return Err("there are no rows to plot".to_string());
    }
    if table.series.is_empty() {
        return Err("there is no column of values to plot".to_string());
    }
    let (w, h) = (spec.width.max(120), spec.height.max(90));
    let mut buf = String::new();
    let mut text = String::new();
    {
        let root = SVGBackend::with_string(&mut buf, (w, h)).into_drawing_area();
        match spec.kind {
            Kind::Pie => pie(spec, table, &mut text)?,
            _ => cartesian(spec, table, &root, &mut text)?,
        }
        root.present().map_err(|e| e.to_string())?;
    }
    let at = buf.rfind("</svg>").ok_or("the SVG was not written")?;
    let mut out = buf[..at].to_string();
    if let Some(title) = &spec.title {
        let _ = writeln!(
            text,
            "<text x=\"{}\" y=\"20\" text-anchor=\"middle\" font-size=\"15\" font-weight=\"600\">{}</text>",
            w / 2,
            escape(title)
        );
    }
    let _ = writeln!(
        out,
        "<g font-family=\"sans-serif\" font-size=\"12\" fill=\"#000000\">\n{text}</g>"
    );
    out.push_str("</svg>");
    // Ink follows the page; and no blank line, which would end an HTML
    // block in Markdown.
    let out = out.replace("#000000", "currentColor");
    Ok(out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// The legend: a swatch and a name for each, along the top.
fn legend(names: &[(String, String)], width: f64, text: &mut String) {
    let size = |n: &str| 26.0 + n.chars().count() as f64 * 6.5;
    let total: f64 = names.iter().map(|(n, _)| size(n)).sum();
    let mut x = ((width - total) / 2.0).max(8.0);
    for (name, fill) in names {
        let _ = writeln!(
            text,
            "<rect x=\"{x:.1}\" y=\"30\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{fill}\"/>\n<text x=\"{:.1}\" y=\"40\">{}</text>",
            x + 17.0,
            escape(name)
        );
        x += size(name);
    }
}

fn cartesian(
    spec: &Spec,
    table: &Table,
    root: &DrawingArea<SVGBackend, Shift>,
    text: &mut String,
) -> Result<(), String> {
    let e = |e: &dyn std::fmt::Display| e.to_string();
    let n = table.labels.len();
    let series = &table.series;
    let ohlc = spec.kind == Kind::Candle && series.len() == 4;
    if spec.kind == Kind::Candle && !ohlc && series.len() != 1 {
        return Err(
            "a candle plot takes one uncertain column, or four: open, high, low and close"
                .to_string(),
        );
    }

    // What the vertical axis must cover.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for s in series {
        for (i, v) in s.values.iter().enumerate() {
            let Some(v) = v else { continue };
            let sigma = s.sigma.get(i).copied().unwrap_or(0.0);
            let reach = if spec.kind == Kind::Candle {
                2.0 * spec.k * sigma
            } else {
                sigma
            };
            lo = lo.min(v - reach);
            hi = hi.max(v + reach);
        }
    }
    if !lo.is_finite() {
        return Err("there are no numbers to plot".to_string());
    }
    if spec.kind == Kind::Bar {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    if lo == hi {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = if spec.kind == Kind::Bar {
        0.0
    } else {
        (hi - lo) * 0.05
    };
    let (ylo, yhi) = (lo - if lo < 0.0 || spec.kind != Kind::Bar { pad } else { 0.0 }, hi + pad);
    let (ticks, step) = nice_ticks(ylo, yhi, 6);

    let (width, height) = (spec.width as f64, spec.height as f64);
    let multi = series.len() > 1 && spec.kind != Kind::Candle;
    let left = 18.0
        + ticks
            .iter()
            .map(|t| tick_text(*t, step).chars().count())
            .max()
            .unwrap_or(2) as f64
            * 6.5;
    let (right, top, bottom) = (
        16.0,
        match (multi, spec.title.is_some()) {
            (true, _) => 52.0,
            (false, true) => 34.0,
            _ => 12.0,
        },
        30.0,
    );
    let (x0, x1) = match spec.kind {
        Kind::Bar | Kind::Candle => (-0.5, n as f64 - 0.5),
        _ if n == 1 => (-0.5, 0.5),
        _ => (0.0, (n - 1) as f64),
    };
    let area = root.margin(top as i32, bottom as i32, left as i32, right as i32);
    let mut chart: Chart = ChartBuilder::on(&area)
        .build_cartesian_2d(x0..x1, ylo..yhi)
        .map_err(|x| e(&x))?;
    // Pixels in the whole drawing, not in the margin's area.
    let px = |chart: &Chart, x: f64, y: f64| {
        let (a, b) = chart.backend_coord(&(x, y));
        (a as f64, b as f64)
    };

    // The grid, and the vertical axis' labels.
    for t in &ticks {
        chart
            .draw_series(std::iter::once(PathElement::new(
                vec![(x0, *t), (x1, *t)],
                BLACK.mix(0.12).stroke_width(1),
            )))
            .map_err(|x| e(&x))?;
        let (_, y) = px(&chart, x0, *t);
        let _ = writeln!(
            text,
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" opacity=\"0.75\">{}</text>",
            left - 6.0,
            y + 4.0,
            tick_text(*t, step)
        );
    }
    // The axis along the bottom, at zero where that is in view.
    let base = if ylo <= 0.0 && yhi >= 0.0 { 0.0 } else { ylo };
    chart
        .draw_series(std::iter::once(PathElement::new(
            vec![(x0, base), (x1, base)],
            BLACK.mix(0.55).stroke_width(1),
        )))
        .map_err(|x| e(&x))?;

    // The labels along it, thinned to what fits.
    let longest = table
        .labels
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1);
    let every = ((longest as f64 * 6.5 + 10.0) * n as f64 / (width - left - right))
        .ceil()
        .max(1.0) as usize;
    for (i, label) in table.labels.iter().enumerate().step_by(every) {
        let (x, _) = px(&chart, i as f64, base);
        let _ = writeln!(
            text,
            "<text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" opacity=\"0.75\">{}</text>",
            height - bottom + 16.0,
            escape(label)
        );
    }

    match spec.kind {
        Kind::Line => {
            for (si, s) in series.iter().enumerate() {
                let c = colour(si);
                // A gap breaks the line.
                let mut runs: Vec<Vec<(f64, f64)>> = vec![Vec::new()];
                for (i, v) in s.values.iter().enumerate() {
                    match v {
                        Some(v) => runs.last_mut().unwrap().push((i as f64, *v)),
                        None => runs.push(Vec::new()),
                    }
                }
                for run in runs {
                    if run.len() > 1 {
                        chart
                            .draw_series(LineSeries::new(run.clone(), c.stroke_width(2)))
                            .map_err(|x| e(&x))?;
                    }
                    chart
                        .draw_series(run.iter().map(|p| Circle::new(*p, 3, c.filled())))
                        .map_err(|x| e(&x))?;
                }
                error_bars(&mut chart, s, |i| i as f64, c.mix(1.0))?;
            }
        }
        Kind::Bar => {
            let slot = 0.8 / series.len() as f64;
            for (si, s) in series.iter().enumerate() {
                let c = colour(si);
                let centre = |i: usize| i as f64 - 0.4 + (si as f64 + 0.5) * slot;
                chart
                    .draw_series(s.values.iter().enumerate().filter_map(|(i, v)| {
                        let v = (*v)?;
                        Some(Rectangle::new(
                            [
                                (centre(i) - slot * 0.44, 0.0),
                                (centre(i) + slot * 0.44, v),
                            ],
                            c.filled(),
                        ))
                    }))
                    .map_err(|x| e(&x))?;
                error_bars(&mut chart, s, centre, BLACK.mix(0.7))?;
            }
        }
        Kind::Candle => {
            let (up, down) = (colour(2), colour(3));
            // A body a third of its slot wide, and the value's tick across it.
            let unit = (width - left - right) / (x1 - x0);
            let body = (unit / 3.0).clamp(6.0, 40.0).round();
            let half = body / 2.0 / unit;
            for i in 0..n {
                let get = |s: usize| series[s].values.get(i).copied().flatten();
                let (open, high, low, close, gain, loss, centre) = if ohlc {
                    let (Some(o), Some(hh), Some(l), Some(c)) = (get(0), get(1), get(2), get(3))
                    else {
                        continue;
                    };
                    (o, hh, l, c, up.filled(), down.filled(), None)
                } else {
                    let Some(v) = get(0) else { continue };
                    let sigma = series[0].sigma.get(i).copied().unwrap_or(0.0);
                    let c = colour(0).filled();
                    (
                        v - spec.k * sigma,
                        v + 2.0 * spec.k * sigma,
                        v - 2.0 * spec.k * sigma,
                        v + spec.k * sigma,
                        c,
                        c,
                        Some(v),
                    )
                };
                chart
                    .draw_series(std::iter::once(CandleStick::new(
                        i as f64, open, high, low, close, gain, loss, body as u32,
                    )))
                    .map_err(|x| e(&x))?;
                if let Some(v) = centre {
                    // The value itself, across the body.
                    chart
                        .draw_series(std::iter::once(PathElement::new(
                            vec![(i as f64 - half, v), (i as f64 + half, v)],
                            WHITE.mix(0.9).stroke_width(2),
                        )))
                        .map_err(|x| e(&x))?;
                }
            }
        }
        Kind::Pie => unreachable!("a pie is not drawn on axes"),
    }

    if multi {
        let names: Vec<(String, String)> = series
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.clone(), hex(i)))
            .collect();
        legend(&names, width, text);
    } else if spec.kind == Kind::Candle {
        let note = if ohlc {
            "open, high, low, close".to_string()
        } else {
            format!(
                "body ±{}σ, wick ±{}σ",
                spec.k,
                2.0 * spec.k
            )
        };
        let _ = writeln!(
            text,
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" opacity=\"0.6\" font-size=\"11\">{}</text>",
            width - right,
            14.0,
            escape(&note)
        );
    }
    Ok(())
}

/// An error bar for each value of a series that has an uncertainty.
fn error_bars(
    chart: &mut Chart,
    s: &Series,
    x_of: impl Fn(usize) -> f64,
    ink: RGBAColor,
) -> Result<(), String> {
    for (i, v) in s.values.iter().enumerate() {
        let (Some(v), Some(sigma)) = (v, s.sigma.get(i)) else {
            continue;
        };
        if *sigma <= 0.0 {
            continue;
        }
        chart
            .draw_series(std::iter::once(ErrorBar::new_vertical(
                x_of(i),
                v - sigma,
                *v,
                v + sigma,
                ink.stroke_width(1),
                6,
            )))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn pie(spec: &Spec, table: &Table, text: &mut String) -> Result<(), String> {
    let s = &table.series[0];
    let items: Vec<(&str, f64)> = table
        .labels
        .iter()
        .zip(&s.values)
        .filter_map(|(l, v)| Some((l.as_str(), (*v)?)))
        .collect();
    if let Some((l, v)) = items.iter().find(|(_, v)| *v < 0.0) {
        return Err(format!("a pie cannot show a negative value: `{l}` is {v}"));
    }
    let total: f64 = items.iter().map(|(_, v)| v).sum();
    if total <= 0.0 {
        return Err("a pie needs values that add to more than zero".to_string());
    }
    let (w, h) = (spec.width as f64, spec.height as f64);
    let top = if spec.title.is_some() { 32.0 } else { 8.0 };
    // The pie on the left, the key on the right.
    let key_w = items
        .iter()
        .map(|(l, _)| l.chars().count() as f64 * 6.5 + 64.0)
        .fold(0.0, f64::max);
    let r = ((w - key_w - 40.0).min(h - top - 16.0) / 2.0).max(20.0);
    let (cx, cy) = (16.0 + r, top + (h - top) / 2.0);
    let mut angle = -std::f64::consts::FRAC_PI_2;
    for (i, (_, v)) in items.iter().enumerate() {
        let sweep = v / total * std::f64::consts::TAU;
        let (x0, y0) = (cx + r * angle.cos(), cy + r * angle.sin());
        let end = angle + sweep;
        let (x1, y1) = (cx + r * end.cos(), cy + r * end.sin());
        if sweep >= std::f64::consts::TAU - 1e-9 {
            let _ = writeln!(
                text,
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{r:.1}\" fill=\"{}\"/>",
                hex(i)
            );
        } else if sweep > 0.0 {
            let large = u8::from(sweep > std::f64::consts::PI);
            let _ = writeln!(
                text,
                "<path d=\"M{cx:.1},{cy:.1} L{x0:.1},{y0:.1} A{r:.1},{r:.1} 0 {large} 1 {x1:.1},{y1:.1} Z\" fill=\"{}\" stroke=\"#ffffff\" stroke-opacity=\"0.6\"/>",
                hex(i)
            );
        }
        angle = end;
    }
    let x = 16.0 + 2.0 * r + 24.0;
    let mut y = (cy - items.len() as f64 * 9.0).max(top);
    for (i, (l, v)) in items.iter().enumerate() {
        let _ = writeln!(
            text,
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"{}\"/>\n<text x=\"{:.1}\" y=\"{:.1}\">{} <tspan opacity=\"0.65\">{:.1}%</tspan></text>",
            hex(i),
            x + 18.0,
            y + 10.0,
            escape(l),
            v / total * 100.0
        );
        y += 18.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table {
            labels: ["Jan", "Feb", "Mar"].map(String::from).to_vec(),
            series: vec![
                Series {
                    name: "Revenue".into(),
                    values: vec![Some(100.0), Some(120.0), Some(150.0)],
                    sigma: vec![0.0; 3],
                },
                Series {
                    name: "Cost".into(),
                    values: vec![Some(60.0), None, Some(90.0)],
                    sigma: vec![0.0; 3],
                },
            ],
        }
    }

    fn svg(kind: Kind, t: &Table) -> String {
        let mut spec = Spec::new(kind);
        spec.title = Some("A & B".into());
        render(&spec, t).unwrap()
    }

    #[test]
    fn line_is_a_polyline_for_each_run_with_a_legend() {
        let s = svg(Kind::Line, &table());
        assert!(s.starts_with("<svg"), "{s}");
        assert!(s.ends_with("</svg>"));
        assert!(s.contains("A &amp; B"));
        assert!(s.contains(">Revenue<") && s.contains(">Cost<"));
        assert!(s.contains("currentColor") && !s.contains("#000000"));
        assert!(!s.contains("\n\n"));
        // Revenue is one run; Cost is two single points, which are no line.
        assert_eq!(s.matches("stroke-width=\"2\"").count(), 1, "{s}");
        assert_eq!(s.matches("<circle").count(), 5);
    }

    #[test]
    fn bar_is_a_rect_for_each_value() {
        let s = svg(Kind::Bar, &table());
        // Five values, and two legend swatches.
        assert_eq!(s.matches("<rect").count(), 5 + 2, "{s}");
    }

    #[test]
    fn pie_is_a_slice_for_each_value_with_percentages() {
        let mut t = table();
        t.series.truncate(1);
        let s = svg(Kind::Pie, &t);
        assert_eq!(s.matches("<path").count(), 3);
        assert!(s.contains("27.0%"), "{s}");
        t.series[0].values[0] = Some(-1.0);
        assert!(render(&Spec::new(Kind::Pie), &t).is_err());
    }

    #[test]
    fn candle_of_an_uncertain_column() {
        let mut t = table();
        t.series.truncate(1);
        t.series[0].sigma = vec![5.0, 8.0, 10.0];
        let s = svg(Kind::Candle, &t);
        assert!(s.contains("body ±1σ, wick ±2σ"), "{s}");
        assert!(s.matches("<rect").count() >= 3, "{s}");
    }

    #[test]
    fn candle_needs_one_column_or_four() {
        assert!(render(&Spec::new(Kind::Candle), &table()).is_err());
    }

    #[test]
    fn nothing_to_plot_is_an_error() {
        assert!(render(&Spec::new(Kind::Line), &Table::default()).is_err());
    }
}
