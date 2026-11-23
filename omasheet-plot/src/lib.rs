// Copyright (c) 2026 Stephen Roe

//! Plots of sheet data as SVG: line, bar, pie, candlestick, scatter, area,
//! stacked bars, steps, histogram, box, heatmap, waterfall, bubble and donut.
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
    /// Points at a number along `x`, with error bars for an uncertain value.
    Scatter,
    /// A line with the area under it filled.
    Area,
    /// Bars of the columns, one on another.
    Stacked,
    /// A line that holds its value until the next row.
    Step,
    /// The counts of a column's values in equal bins.
    Histogram,
    /// A box for each column: quartiles, whiskers and outliers.
    Box,
    /// A cell for each value, as dark as it is large.
    Heatmap,
    /// Changes, one after another, and the total they come to.
    Waterfall,
    /// Points sized by a second column.
    Bubble,
    /// A pie with a hole.
    Donut,
}

impl Kind {
    pub const NAMES: &'static str = "line, bar, pie, candle, scatter, area, stacked, step, histogram, box, heatmap, waterfall, bubble, donut";

    pub fn parse(name: &str) -> Option<Kind> {
        Some(match name {
            "line" => Kind::Line,
            "bar" => Kind::Bar,
            "pie" => Kind::Pie,
            "candle" => Kind::Candle,
            "scatter" => Kind::Scatter,
            "area" => Kind::Area,
            "stacked" => Kind::Stacked,
            "step" => Kind::Step,
            "histogram" => Kind::Histogram,
            "box" => Kind::Box,
            "heatmap" => Kind::Heatmap,
            "waterfall" => Kind::Waterfall,
            "bubble" => Kind::Bubble,
            "donut" => Kind::Donut,
            _ => return None,
        })
    }
}

/// A mark of a plot as it is drawn, for a click on it: a rectangle in the
/// pixels of the drawing, and the row (and column of values) it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub row: usize,
    /// Which column of values, counting from the first.
    pub series: usize,
}

/// The mark under pixel (`x`, `y`): of those that hold it, the smallest.
pub fn hit_at(hits: &[Hit], x: f64, y: f64) -> Option<&Hit> {
    hits.iter()
        .filter(|h| x >= h.x && x <= h.x + h.w && y >= h.y && y <= h.y + h.h)
        .min_by(|a, b| (a.w * a.h).total_cmp(&(b.w * b.h)))
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
    /// For a histogram: how many bins (by default from the number of values).
    pub bins: Option<usize>,
}

impl Spec {
    pub fn new(kind: Kind) -> Spec {
        Spec {
            kind,
            title: None,
            width: 640,
            height: 360,
            k: 1.0,
            bins: None,
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
    /// The labels as numbers, where every one is: a scatter or a bubble plot
    /// needs them, and a line or area is drawn at them.
    pub xs: Option<Vec<Option<f64>>>,
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
    render_hits(spec, table).map(|(svg, _)| svg)
}

/// The SVG of a plot, and where its marks are.
pub fn render_hits(spec: &Spec, table: &Table) -> Result<(String, Vec<Hit>), String> {
    if table.labels.is_empty() {
        return Err("there are no rows to plot".to_string());
    }
    if table.series.is_empty() {
        return Err("there is no column of values to plot".to_string());
    }
    let (w, h) = (spec.width.max(120), spec.height.max(90));
    let mut buf = String::new();
    let mut text = String::new();
    let mut hits = Vec::new();
    {
        let root = SVGBackend::with_string(&mut buf, (w, h)).into_drawing_area();
        match spec.kind {
            Kind::Pie => pie(spec, table, 0.0, &mut text)?,
            Kind::Donut => pie(spec, table, 0.55, &mut text)?,
            Kind::Heatmap => heatmap(spec, table, &mut text, &mut hits)?,
            _ => {
                let prep = prepare(spec, table)?;
                cartesian(spec, &prep, &root, &mut text, &mut hits)?;
            }
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
    let svg = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok((svg, hits))
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

/// The quartiles of one column, and the whiskers (the furthest values within
/// 1.5 times the range between the quartiles) and the values beyond them.
struct BoxStat {
    lo: f64,
    q1: f64,
    med: f64,
    q3: f64,
    hi: f64,
    outliers: Vec<f64>,
}

/// What a plot is drawn from, once the kind has made of the table what it
/// needs.
struct Prep {
    table: Table,
    /// The labels as numbers, for the kinds drawn at them.
    xv: Option<Vec<f64>>,
    boxes: Vec<BoxStat>,
    /// A waterfall's bars: where each starts and ends.
    bars: Vec<(f64, f64)>,
    /// A line in the corner, about what is drawn.
    note: Option<String>,
}

fn quantile(sorted: &[f64], p: f64) -> f64 {
    let at = p * (sorted.len() - 1) as f64;
    let (a, b) = (at.floor() as usize, at.ceil() as usize);
    sorted[a] + (sorted[b] - sorted[a]) * (at - a as f64)
}

fn prepare(spec: &Spec, table: &Table) -> Result<Prep, String> {
    let mut prep = Prep {
        table: table.clone(),
        xv: None,
        boxes: Vec::new(),
        bars: Vec::new(),
        note: None,
    };
    let numbers = |s: &Series| -> Vec<f64> {
        s.values
            .iter()
            .flatten()
            .copied()
            .filter(|v| v.is_finite())
            .collect()
    };
    match spec.kind {
        Kind::Scatter | Kind::Bubble | Kind::Line | Kind::Step | Kind::Area => {
            prep.xv = table
                .xs
                .as_ref()
                .map(|xs| xs.iter().map(|x| x.unwrap_or(f64::NAN)).collect());
            if prep.xv.is_none() && matches!(spec.kind, Kind::Scatter | Kind::Bubble) {
                return Err(format!(
                    "a {} plot needs numbers in `x`, and the labels are not",
                    if spec.kind == Kind::Scatter { "scatter" } else { "bubble" }
                ));
            }
            if spec.kind == Kind::Bubble && table.series.len() < 2 {
                return Err("a bubble plot needs two columns of values: the height, then the size".to_string());
            }
        }
        Kind::Histogram => {
            let all: Vec<f64> = table.series.iter().flat_map(numbers).collect();
            if all.is_empty() {
                return Err("there are no numbers to plot".to_string());
            }
            let n = all.len();
            let bins = spec
                .bins
                .unwrap_or_else(|| (((n as f64).log2().ceil() as usize) + 1).clamp(3, 40))
                .max(1);
            let (mut lo, mut hi) = all
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
            if lo == hi {
                lo -= 0.5;
                hi += 0.5;
            }
            let width = (hi - lo) / bins as f64;
            let mut series = Vec::new();
            for s in &table.series {
                let mut counts = vec![0.0; bins];
                for v in numbers(s) {
                    let b = (((v - lo) / width) as usize).min(bins - 1);
                    counts[b] += 1.0;
                }
                series.push(Series {
                    name: s.name.clone(),
                    values: counts.into_iter().map(Some).collect(),
                    sigma: vec![0.0; bins],
                });
            }
            prep.table = Table {
                labels: (0..bins)
                    .map(|b| tick_text(lo + (b as f64 + 0.5) * width, width))
                    .collect(),
                series,
                xs: None,
            };
            prep.note = Some(format!("{n} values, {bins} bins of {}", tick_text(width, width / 10.0)));
        }
        Kind::Box => {
            let mut labels = Vec::new();
            for s in &table.series {
                let mut v = numbers(s);
                if v.is_empty() {
                    continue;
                }
                v.sort_by(|a, b| a.total_cmp(b));
                let (q1, med, q3) = (quantile(&v, 0.25), quantile(&v, 0.5), quantile(&v, 0.75));
                let reach = 1.5 * (q3 - q1);
                let inside = |x: &&f64| **x >= q1 - reach && **x <= q3 + reach;
                let lo = v.iter().filter(inside).fold(f64::INFINITY, |a, b| a.min(*b));
                let hi = v.iter().filter(inside).fold(f64::NEG_INFINITY, |a, b| a.max(*b));
                let outliers = v.iter().filter(|x| !inside(x)).copied().collect();
                prep.boxes.push(BoxStat { lo, q1, med, q3, hi, outliers });
                labels.push(s.name.clone());
            }
            if prep.boxes.is_empty() {
                return Err("there are no numbers to plot".to_string());
            }
            prep.table = Table { labels, series: Vec::new(), xs: None };
            prep.note = Some("box: quartiles, whiskers to 1.5 × the range".to_string());
        }
        Kind::Waterfall => {
            let s = &table.series[0];
            let mut level = 0.0;
            for v in &s.values {
                let d = v.unwrap_or(0.0);
                prep.bars.push((level, level + d));
                level += d;
            }
            prep.bars.push((0.0, level));
            let mut labels = table.labels.clone();
            labels.push("Total".to_string());
            let n = labels.len();
            prep.table = Table {
                labels,
                series: vec![Series {
                    name: s.name.clone(),
                    values: prep.bars.iter().map(|b| Some(b.1)).collect(),
                    sigma: vec![0.0; n],
                }],
                xs: None,
            };
        }
        _ => {}
    }
    Ok(prep)
}

fn cartesian(
    spec: &Spec,
    prep: &Prep,
    root: &DrawingArea<SVGBackend, Shift>,
    text: &mut String,
    hits: &mut Vec<Hit>,
) -> Result<(), String> {
    let e = |e: &dyn std::fmt::Display| e.to_string();
    let kind = spec.kind;
    let table = &prep.table;
    let n = table.labels.len();
    let series = &table.series;
    let ohlc = kind == Kind::Candle && series.len() == 4;
    if kind == Kind::Candle && !ohlc && series.len() != 1 {
        return Err(
            "a candle plot takes one uncertain column, or four: open, high, low and close"
                .to_string(),
        );
    }
    // Bars start at zero, and an area is filled down to it.
    let zero = matches!(
        kind,
        Kind::Bar | Kind::Area | Kind::Stacked | Kind::Histogram | Kind::Waterfall
    );
    let numeric_x = prep.xv.is_some();
    let xat = |i: usize| prep.xv.as_ref().map_or(i as f64, |v| v[i]);

    // What the vertical axis must cover.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut see = |v: f64| {
        lo = lo.min(v);
        hi = hi.max(v);
    };
    let ys = if kind == Kind::Bubble { &series[..1] } else { &series[..] };
    match kind {
        Kind::Stacked => {
            for i in 0..n {
                let (mut up, mut down) = (0.0, 0.0);
                for s in series {
                    let v = s.values.get(i).copied().flatten().unwrap_or(0.0);
                    if v >= 0.0 { up += v } else { down += v }
                }
                see(up);
                see(down);
            }
        }
        Kind::Box => {
            for b in &prep.boxes {
                see(b.lo);
                see(b.hi);
                b.outliers.iter().for_each(|v| see(*v));
            }
        }
        Kind::Waterfall => prep.bars.iter().for_each(|b| {
            see(b.0);
            see(b.1);
        }),
        _ => {
            for s in ys {
                for (i, v) in s.values.iter().enumerate() {
                    let Some(v) = v else { continue };
                    let sigma = s.sigma.get(i).copied().unwrap_or(0.0);
                    let reach = match kind {
                        Kind::Candle => 2.0 * spec.k * sigma,
                        Kind::Line | Kind::Bar | Kind::Scatter => sigma,
                        _ => 0.0,
                    };
                    see(v - reach);
                    see(v + reach);
                }
            }
        }
    }
    if !lo.is_finite() {
        return Err("there are no numbers to plot".to_string());
    }
    if zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    if lo == hi {
        lo -= 1.0;
        hi += 1.0;
    }
    let (below, above) = match kind {
        Kind::Bar => (0.0, 0.0),
        Kind::Bubble => ((hi - lo) * 0.12, (hi - lo) * 0.12),
        _ if zero => (0.0, (hi - lo) * 0.04),
        _ => ((hi - lo) * 0.05, (hi - lo) * 0.05),
    };
    let (ylo, yhi) = (lo - below, hi + above);
    let (ticks, step) = nice_ticks(ylo, yhi, 6);

    let (width, height) = (spec.width as f64, spec.height as f64);
    let multi = series.len() > 1
        && matches!(
            kind,
            Kind::Line | Kind::Bar | Kind::Area | Kind::Stacked | Kind::Step | Kind::Scatter | Kind::Histogram
        );
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
    let (x0, x1) = if numeric_x {
        let xs: Vec<f64> = (0..n).map(xat).filter(|x| x.is_finite()).collect();
        let (a, b) = xs
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| (a.min(*x), b.max(*x)));
        if !a.is_finite() {
            return Err("there are no numbers along `x`".to_string());
        }
        let span = if b > a { b - a } else { 2.0 };
        let pad = span * if matches!(kind, Kind::Scatter | Kind::Bubble) { 0.08 } else { 0.0 };
        if b > a { (a - pad, b + pad) } else { (a - 1.0, a + 1.0) }
    } else {
        match kind {
            Kind::Bar | Kind::Candle | Kind::Stacked | Kind::Histogram | Kind::Box | Kind::Waterfall => {
                (-0.5, n as f64 - 0.5)
            }
            _ if n == 1 => (-0.5, 0.5),
            _ => (0.0, (n - 1) as f64),
        }
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
    if numeric_x {
        let (xt, xstep) = nice_ticks(x0, x1, ((width - left - right) / 70.0).max(2.0) as usize);
        for t in xt {
            chart
                .draw_series(std::iter::once(PathElement::new(
                    vec![(t, ylo), (t, yhi)],
                    BLACK.mix(0.08).stroke_width(1),
                )))
                .map_err(|x| e(&x))?;
            let (x, _) = px(&chart, t, base);
            let _ = writeln!(
                text,
                "<text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" opacity=\"0.75\">{}</text>",
                height - bottom + 16.0,
                tick_text(t, xstep)
            );
        }
    } else {
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
    }

    match kind {
        Kind::Line | Kind::Step | Kind::Area => {
            for (si, s) in series.iter().enumerate() {
                let c = colour(si);
                // A gap breaks the line.
                let mut runs: Vec<Vec<(f64, f64)>> = vec![Vec::new()];
                for (i, v) in s.values.iter().enumerate() {
                    match v {
                        Some(v) if xat(i).is_finite() => runs.last_mut().unwrap().push((xat(i), *v)),
                        _ => runs.push(Vec::new()),
                    }
                }
                for run in runs {
                    match kind {
                        Kind::Step => {
                            let mut path = Vec::new();
                            for (j, p) in run.iter().enumerate() {
                                path.push(*p);
                                if let Some(next) = run.get(j + 1) {
                                    path.push((next.0, p.1));
                                }
                            }
                            if path.len() > 1 {
                                chart
                                    .draw_series(LineSeries::new(path, c.stroke_width(2)))
                                    .map_err(|x| e(&x))?;
                            }
                        }
                        Kind::Area => {
                            if run.len() > 1 {
                                let mut shape = vec![(run[0].0, base)];
                                shape.extend(run.iter().copied());
                                shape.push((run[run.len() - 1].0, base));
                                chart
                                    .draw_series(std::iter::once(Polygon::new(shape, c.mix(0.3).filled())))
                                    .map_err(|x| e(&x))?;
                                chart
                                    .draw_series(LineSeries::new(run.clone(), c.stroke_width(2)))
                                    .map_err(|x| e(&x))?;
                            }
                        }
                        _ => {
                            if run.len() > 1 {
                                chart
                                    .draw_series(LineSeries::new(run.clone(), c.stroke_width(2)))
                                    .map_err(|x| e(&x))?;
                            }
                            chart
                                .draw_series(run.iter().map(|p| Circle::new(*p, 3, c.filled())))
                                .map_err(|x| e(&x))?;
                        }
                    }
                }
                if kind == Kind::Line {
                    error_bars(&mut chart, s, xat, c.mix(1.0))?;
                }
            }
        }
        Kind::Scatter => {
            for (si, s) in series.iter().enumerate() {
                let c = colour(si);
                chart
                    .draw_series(s.values.iter().enumerate().filter_map(|(i, v)| {
                        let v = (*v)?;
                        xat(i).is_finite().then(|| Circle::new((xat(i), v), 4, c.mix(0.85).filled()))
                    }))
                    .map_err(|x| e(&x))?;
                error_bars(&mut chart, s, xat, c.mix(1.0))?;
            }
        }
        Kind::Bubble => {
            let size = &series[1];
            let sizes: Vec<f64> = size.values.iter().flatten().copied().collect();
            let (smin, smax) = sizes
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
            let reach = ((width - left - right).min(height - top - bottom) / 12.0).max(10.0);
            for i in 0..n {
                let (Some(v), Some(sz)) = (series[0].values[i], size.values[i]) else {
                    continue;
                };
                if !xat(i).is_finite() {
                    continue;
                }
                let frac = if smax > smin { ((sz - smin) / (smax - smin)).sqrt() } else { 0.5 };
                let (x, y) = px(&chart, xat(i), v);
                let r = 4.0 + (reach - 4.0) * frac;
                hits.push(Hit { x: x - r, y: y - r, w: 2.0 * r, h: 2.0 * r, row: i, series: 0 });
                let _ = writeln!(
                    text,
                    "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"{r:.1}\" fill=\"{}\" fill-opacity=\"0.55\" stroke=\"{}\"/>",
                    hex(0),
                    hex(0)
                );
            }
            let _ = (&series[1].name, ());
        }
        Kind::Bar | Kind::Histogram => {
            let slot = if kind == Kind::Histogram { 1.0 } else { 0.8 } / series.len() as f64;
            let fill = if kind == Kind::Histogram { 0.5 } else { 0.44 };
            for (si, s) in series.iter().enumerate() {
                let c = colour(si);
                let origin = if kind == Kind::Histogram { 0.5 } else { 0.4 };
                let centre = |i: usize| i as f64 - origin + (si as f64 + 0.5) * slot;
                chart
                    .draw_series(s.values.iter().enumerate().filter_map(|(i, v)| {
                        let v = (*v)?;
                        Some(Rectangle::new(
                            [
                                (centre(i) - slot * fill, 0.0),
                                (centre(i) + slot * fill, v),
                            ],
                            if kind == Kind::Histogram { c.mix(0.8).filled() } else { c.filled() },
                        ))
                    }))
                    .map_err(|x| e(&x))?;
                if kind == Kind::Bar {
                    error_bars(&mut chart, s, centre, BLACK.mix(0.7))?;
                }
            }
        }
        Kind::Stacked => {
            for i in 0..n {
                let (mut up, mut down) = (0.0, 0.0);
                for (si, s) in series.iter().enumerate() {
                    let v = s.values.get(i).copied().flatten().unwrap_or(0.0);
                    let (from, to) = if v >= 0.0 {
                        up += v;
                        (up - v, up)
                    } else {
                        down += v;
                        (down - v, down)
                    };
                    chart
                        .draw_series(std::iter::once(Rectangle::new(
                            [(i as f64 - 0.4, from), (i as f64 + 0.4, to)],
                            colour(si).filled(),
                        )))
                        .map_err(|x| e(&x))?;
                }
            }
        }
        Kind::Waterfall => {
            let last = prep.bars.len() - 1;
            for (i, (from, to)) in prep.bars.iter().enumerate() {
                let c = if i == last {
                    colour(0)
                } else if to >= from {
                    colour(2)
                } else {
                    colour(3)
                };
                chart
                    .draw_series(std::iter::once(Rectangle::new(
                        [(i as f64 - 0.35, *from), (i as f64 + 0.35, *to)],
                        c.filled(),
                    )))
                    .map_err(|x| e(&x))?;
                if i < last {
                    chart
                        .draw_series(std::iter::once(PathElement::new(
                            vec![(i as f64 + 0.35, *to), (i as f64 + 0.65, *to)],
                            BLACK.mix(0.5).stroke_width(1),
                        )))
                        .map_err(|x| e(&x))?;
                }
            }
        }
        Kind::Box => {
            for (i, b) in prep.boxes.iter().enumerate() {
                let c = colour(i);
                let x = i as f64;
                let ink = BLACK.mix(0.8).stroke_width(1);
                let lines = [
                    vec![(x, b.lo), (x, b.q1)],
                    vec![(x, b.q3), (x, b.hi)],
                    vec![(x - 0.12, b.lo), (x + 0.12, b.lo)],
                    vec![(x - 0.12, b.hi), (x + 0.12, b.hi)],
                ];
                for l in lines {
                    chart
                        .draw_series(std::iter::once(PathElement::new(l, ink)))
                        .map_err(|x| e(&x))?;
                }
                chart
                    .draw_series(std::iter::once(Rectangle::new(
                        [(x - 0.25, b.q1), (x + 0.25, b.q3)],
                        c.mix(0.6).filled(),
                    )))
                    .map_err(|x| e(&x))?;
                chart
                    .draw_series(std::iter::once(Rectangle::new(
                        [(x - 0.25, b.q1), (x + 0.25, b.q3)],
                        c.stroke_width(1),
                    )))
                    .map_err(|x| e(&x))?;
                chart
                    .draw_series(std::iter::once(PathElement::new(
                        vec![(x - 0.25, b.med), (x + 0.25, b.med)],
                        BLACK.stroke_width(2),
                    )))
                    .map_err(|x| e(&x))?;
                chart
                    .draw_series(b.outliers.iter().map(|v| Circle::new((x, *v), 3, c.stroke_width(1))))
                    .map_err(|x| e(&x))?;
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
        Kind::Pie | Kind::Donut | Kind::Heatmap => unreachable!("not drawn on axes"),
    }

    // Where the marks are, for a click on one.
    let cell = |chart: &Chart, xa: f64, ya: f64, xb: f64, yb: f64, row: usize, series: usize| {
        let (p, q) = (px(chart, xa, ya), px(chart, xb, yb));
        Hit {
            x: p.0.min(q.0),
            y: p.1.min(q.1),
            w: (p.0 - q.0).abs().max(1.0),
            h: (p.1 - q.1).abs().max(1.0),
            row,
            series,
        }
    };
    let dot = |chart: &Chart, x: f64, y: f64, row: usize, series: usize| {
        let (a, b) = px(chart, x, y);
        Hit { x: a - 8.0, y: b - 8.0, w: 16.0, h: 16.0, row, series }
    };
    match kind {
        Kind::Line | Kind::Step | Kind::Area | Kind::Scatter => {
            for (si, s) in series.iter().enumerate() {
                for (i, v) in s.values.iter().enumerate() {
                    if let (Some(v), true) = (v, xat(i).is_finite()) {
                        hits.push(dot(&chart, xat(i), *v, i, si));
                    }
                }
            }
        }
        Kind::Bar => {
            let slot = 0.8 / series.len() as f64;
            for (si, s) in series.iter().enumerate() {
                for (i, v) in s.values.iter().enumerate() {
                    if let Some(v) = v {
                        let c = i as f64 - 0.4 + (si as f64 + 0.5) * slot;
                        hits.push(cell(&chart, c - slot * 0.44, 0.0, c + slot * 0.44, *v, i, si));
                    }
                }
            }
        }
        Kind::Stacked => {
            for i in 0..n {
                let (mut up, mut down) = (0.0, 0.0);
                for (si, s) in series.iter().enumerate() {
                    let v = s.values.get(i).copied().flatten().unwrap_or(0.0);
                    let (from, to) = if v >= 0.0 {
                        up += v;
                        (up - v, up)
                    } else {
                        down += v;
                        (down - v, down)
                    };
                    if s.values.get(i).copied().flatten().is_some() {
                        hits.push(cell(&chart, i as f64 - 0.4, from, i as f64 + 0.4, to, i, si));
                    }
                }
            }
        }
        Kind::Candle => {
            for i in 0..n {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for s in series {
                    if let Some(v) = s.values.get(i).copied().flatten() {
                        let reach = if ohlc { 0.0 } else { 2.0 * spec.k * s.sigma.get(i).copied().unwrap_or(0.0) };
                        lo = lo.min(v - reach);
                        hi = hi.max(v + reach);
                    }
                }
                if lo.is_finite() {
                    hits.push(cell(&chart, i as f64 - 0.3, lo, i as f64 + 0.3, hi, i, 0));
                }
            }
        }
        Kind::Waterfall => {
            for (i, (from, to)) in prep.bars.iter().enumerate().take(prep.bars.len() - 1) {
                hits.push(cell(&chart, i as f64 - 0.35, *from, i as f64 + 0.35, *to, i, 0));
            }
        }
        _ => {}
    }

    if multi {
        let names: Vec<(String, String)> = series
            .iter()
            .enumerate()
            .map(|(i, s)| (s.name.clone(), hex(i)))
            .collect();
        legend(&names, width, text);
    }
    let note = if kind == Kind::Candle {
        Some(if ohlc {
            "open, high, low, close".to_string()
        } else {
            format!("body ±{}σ, wick ±{}σ", spec.k, 2.0 * spec.k)
        })
    } else if kind == Kind::Bubble {
        Some(format!("size: {}", series[1].name))
    } else {
        prep.note.clone()
    };
    if let (Some(note), false) = (note, multi) {
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
        if *sigma <= 0.0 || !x_of(i).is_finite() {
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

/// A number in a cell of a heatmap: three significant figures.
fn short(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if v.abs() >= 1e5 || v.abs() < 1e-3 {
        return format!("{v:.1e}");
    }
    let places = (1 - v.abs().log10().floor() as i32).clamp(0, 6) as usize;
    format!("{v:.places$}")
}

/// A cell for each value, a row for each row of the sheet and a column for
/// each column of values, as dark as the value is large.
fn heatmap(spec: &Spec, table: &Table, text: &mut String, hits: &mut Vec<Hit>) -> Result<(), String> {
    let all: Vec<f64> = table
        .series
        .iter()
        .flat_map(|s| s.values.iter().flatten().copied())
        .filter(|v| v.is_finite())
        .collect();
    if all.is_empty() {
        return Err("there are no numbers to plot".to_string());
    }
    let (lo, hi) = all
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| (a.min(*v), b.max(*v)));
    let frac = |v: f64| if hi > lo { (v - lo) / (hi - lo) } else { 0.5 };
    let (w, h) = (spec.width as f64, spec.height as f64);
    let rows = table.labels.len();
    let cols = table.series.len();
    let label_w = table
        .labels
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1) as f64
        * 6.5
        + 14.0;
    let top = if spec.title.is_some() { 52.0 } else { 30.0 };
    let (left, right, bottom) = (label_w, 64.0, 12.0);
    let (cw, ch) = ((w - left - right) / cols as f64, (h - top - bottom) / rows as f64);
    if cw < 2.0 || ch < 2.0 {
        return Err("there is no room for so many cells: make the plot larger".to_string());
    }
    let every = ((26.0 / cw).ceil() as usize).max(1);
    for (j, s) in table.series.iter().enumerate().step_by(every) {
        let _ = writeln!(
            text,
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" opacity=\"0.75\">{}</text>",
            left + (j as f64 + 0.5) * cw,
            top - 6.0,
            escape(&s.name)
        );
    }
    let numbers = cw >= 30.0 && ch >= 14.0;
    for (i, label) in table.labels.iter().enumerate() {
        let y = top + i as f64 * ch;
        if ch >= 12.0 {
            let _ = writeln!(
                text,
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" opacity=\"0.75\">{}</text>",
                left - 6.0,
                y + ch / 2.0 + 4.0,
                escape(label)
            );
        }
        for (j, s) in table.series.iter().enumerate() {
            let Some(v) = s.values.get(i).copied().flatten().filter(|v| v.is_finite()) else {
                continue;
            };
            let f = frac(v);
            let x = left + j as f64 * cw;
            hits.push(Hit { x, y, w: cw, h: ch, row: i, series: j });
            let _ = writeln!(
                text,
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{cw:.1}\" height=\"{ch:.1}\" fill=\"{}\" fill-opacity=\"{:.2}\" stroke=\"#ffffff\" stroke-opacity=\"0.25\"/>",
                hex(0),
                0.08 + 0.92 * f
            );
            if numbers {
                let ink = if f > 0.55 { "#ffffff" } else { "#000000" };
                let _ = writeln!(
                    text,
                    "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-size=\"10\" fill=\"{ink}\">{}</text>",
                    x + cw / 2.0,
                    y + ch / 2.0 + 3.5,
                    short(v)
                );
            }
        }
    }
    // The scale: the lightest at the bottom, the darkest at the top.
    let (sx, steps) = (w - right + 18.0, 24);
    let (sy, sh) = (top, (h - top - bottom).min(160.0));
    for k in 0..steps {
        let f = 1.0 - (k as f64 + 0.5) / steps as f64;
        let _ = writeln!(
            text,
            "<rect x=\"{sx:.1}\" y=\"{:.1}\" width=\"12\" height=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.2}\"/>",
            sy + sh * k as f64 / steps as f64,
            sh / steps as f64 + 0.5,
            hex(0),
            0.08 + 0.92 * f
        );
    }
    let _ = writeln!(
        text,
        "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" opacity=\"0.75\">{}</text>\n<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" opacity=\"0.75\">{}</text>",
        sx + 16.0,
        sy + 8.0,
        short(hi),
        sx + 16.0,
        sy + sh,
        short(lo)
    );
    Ok(())
}

fn pie(spec: &Spec, table: &Table, hole: f64, text: &mut String) -> Result<(), String> {
    let s = &table.series[0];
    let items: Vec<(&str, f64)> = table
        .labels
        .iter()
        .zip(&s.values)
        .filter_map(|(l, v)| Some((l.as_str(), (*v)?)))
        .collect();
    if let Some((l, v)) = items.iter().find(|(_, v)| *v < 0.0) {
        return Err(format!("a {} cannot show a negative value: `{l}` is {v}", if hole > 0.0 { "donut" } else { "pie" }));
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
        let ri = r * hole;
        if sweep >= std::f64::consts::TAU - 1e-9 {
            if hole > 0.0 {
                let _ = writeln!(
                    text,
                    "<path fill-rule=\"evenodd\" d=\"M{:.1},{cy:.1} a{r:.1},{r:.1} 0 1,0 {:.1},0 a{r:.1},{r:.1} 0 1,0 -{:.1},0 Z M{:.1},{cy:.1} a{ri:.1},{ri:.1} 0 1,0 {:.1},0 a{ri:.1},{ri:.1} 0 1,0 -{:.1},0 Z\" fill=\"{}\"/>",
                    cx - r, 2.0 * r, 2.0 * r, cx - ri, 2.0 * ri, 2.0 * ri,
                    hex(i)
                );
            } else {
                let _ = writeln!(
                    text,
                    "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{r:.1}\" fill=\"{}\"/>",
                    hex(i)
                );
            }
        } else if sweep > 0.0 && hole > 0.0 {
            let large = u8::from(sweep > std::f64::consts::PI);
            let (ix0, iy0) = (cx + ri * angle.cos(), cy + ri * angle.sin());
            let (ix1, iy1) = (cx + ri * end.cos(), cy + ri * end.sin());
            let _ = writeln!(
                text,
                "<path d=\"M{x0:.1},{y0:.1} A{r:.1},{r:.1} 0 {large} 1 {x1:.1},{y1:.1} L{ix1:.1},{iy1:.1} A{ri:.1},{ri:.1} 0 {large} 0 {ix0:.1},{iy0:.1} Z\" fill=\"{}\" stroke=\"#ffffff\" stroke-opacity=\"0.6\"/>",
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
            xs: None,
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

    fn numeric() -> Table {
        let mut t = table();
        t.xs = Some(vec![Some(1.0), Some(2.0), Some(4.0)]);
        t.series[1].values[1] = Some(80.0);
        t.series[0].sigma = vec![5.0, 5.0, 5.0];
        t
    }

    #[test]
    fn scatter_is_a_circle_for_each_value_with_error_bars() {
        let s = svg(Kind::Scatter, &numeric());
        // Six points, and a mark at the centre of each of three error bars.
        assert_eq!(s.matches("<circle").count(), 6 + 3, "{s}");
        assert!(render(&Spec::new(Kind::Scatter), &table()).is_err());
    }

    #[test]
    fn a_line_is_drawn_at_numeric_labels() {
        let a = svg(Kind::Line, &numeric());
        let b = svg(Kind::Line, &table());
        assert_ne!(a, b);
        assert!(a.contains(">4.0<"), "{a}");
    }

    #[test]
    fn area_step_and_stacked_are_drawn() {
        assert!(svg(Kind::Area, &table()).contains("<polygon"));
        assert!(svg(Kind::Step, &table()).contains("<polyline"));
        // Five values and two legend swatches.
        assert_eq!(svg(Kind::Stacked, &table()).matches("<rect").count(), 6 + 2);
    }

    #[test]
    fn histogram_counts_into_bins() {
        let mut t = table();
        t.series.truncate(1);
        let mut spec = Spec::new(Kind::Histogram);
        spec.bins = Some(3);
        let s = render(&spec, &t).unwrap();
        assert!(s.contains("3 values, 3 bins"), "{s}");
        assert_eq!(s.matches("<rect").count(), 3, "{s}");
    }

    #[test]
    fn box_has_a_box_for_each_column_and_outliers() {
        let mut t = table();
        t.series[0].values = (0..11).map(|i| Some(if i == 10 { 90.0 } else { 10.0 + i as f64 })).collect();
        t.series[1].values = (0..11).map(|i| Some(i as f64)).collect();
        let s = svg(Kind::Box, &t);
        // The box is a fill and an outline each, and one outlier.
        assert_eq!(s.matches("<circle").count(), 1, "{s}");
        assert!(s.contains("1.5 × the range"));
    }

    #[test]
    fn heatmap_has_a_cell_for_each_value() {
        let s = svg(Kind::Heatmap, &table());
        // Five values (one is a gap) and the scale's 24 steps.
        assert_eq!(s.matches("<rect").count(), 5 + 24, "{s}");
    }

    #[test]
    fn waterfall_ends_in_the_total() {
        let mut t = table();
        t.series.truncate(1);
        let s = svg(Kind::Waterfall, &t);
        assert!(s.contains(">Total<"), "{s}");
        assert_eq!(s.matches("<rect").count(), 4, "{s}");
    }

    #[test]
    fn bubble_is_sized_by_its_second_column() {
        let s = svg(Kind::Bubble, &numeric());
        assert_eq!(s.matches("<circle").count(), 3, "{s}");
        assert!(s.contains("size: Cost"));
        let mut t = numeric();
        t.series.truncate(1);
        assert!(render(&Spec::new(Kind::Bubble), &t).is_err());
    }

    #[test]
    fn donut_is_a_ring_for_each_value() {
        let mut t = table();
        t.series.truncate(1);
        let s = svg(Kind::Donut, &t);
        assert_eq!(s.matches("<path").count(), 3);
    }
}
