// Copyright (c) 2026 Stephen Roe

//! The functions of OMX: what each is called, and what a person browsing
//! them is told. The checker and the function directory both read this.

/// A function of one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathFn {
    Abs,
    Sign,
    Round,
    Floor,
    Ceil,
    Sqrt,
    Exp,
    Ln,
    Log10,
    Log2,
    Sin,
    Cos,
    Tan,
    Arcsin,
    Arccos,
    Arctan,
    Sinh,
    Cosh,
    Tanh,
    Radians,
    Degrees,
}

impl MathFn {
    /// The function directory is the one name table: an entry made with
    /// [`math_doc`] carries the `MathFn` it names.
    pub fn from_name(name: &str) -> Option<MathFn> {
        FUNCTIONS.iter().find(|f| f.name == name)?.math
    }

    pub fn name(self) -> &'static str {
        FUNCTIONS
            .iter()
            .find(|f| f.math == Some(self))
            .map_or("", |f| f.name)
    }

    /// Whether an exact number gives an exact result.
    pub fn is_exact(self) -> bool {
        matches!(
            self,
            MathFn::Abs | MathFn::Sign | MathFn::Round | MathFn::Floor | MathFn::Ceil
        )
    }

    /// Whether the result is always a whole number, given an exact one.
    pub fn is_whole(self) -> bool {
        self.is_exact() && !matches!(self, MathFn::Abs)
    }

    pub fn apply(self, x: f64) -> f64 {
        match self {
            MathFn::Abs => x.abs(),
            MathFn::Sign if x == 0.0 => 0.0,
            MathFn::Sign => x.signum(),
            MathFn::Round => x.round(),
            MathFn::Floor => x.floor(),
            MathFn::Ceil => x.ceil(),
            MathFn::Sqrt => x.sqrt(),
            MathFn::Exp => x.exp(),
            MathFn::Ln => x.ln(),
            MathFn::Log10 => x.log10(),
            MathFn::Log2 => x.log2(),
            MathFn::Sin => x.sin(),
            MathFn::Cos => x.cos(),
            MathFn::Tan => x.tan(),
            MathFn::Arcsin => x.asin(),
            MathFn::Arccos => x.acos(),
            MathFn::Arctan => x.atan(),
            MathFn::Sinh => x.sinh(),
            MathFn::Cosh => x.cosh(),
            MathFn::Tanh => x.tanh(),
            MathFn::Radians => x.to_radians(),
            MathFn::Degrees => x.to_degrees(),
        }
    }
}

/// One entry of the function directory.
#[derive(Clone, Copy, Debug)]
pub struct FuncDoc {
    pub name: &'static str,
    pub category: &'static str,
    /// How it is written, such as `Sheet.Column.sum()`.
    pub usage: &'static str,
    pub summary: &'static str,
    /// The function of one real number this is the name of, where it is one.
    pub math: Option<MathFn>,
}

const fn doc(
    category: &'static str,
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
) -> FuncDoc {
    FuncDoc {
        name,
        category,
        usage,
        summary,
        math: None,
    }
}

/// One entry for a function of one real number, which is also how
/// [`MathFn::from_name`] finds it.
const fn math_doc(
    category: &'static str,
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
    math: MathFn,
) -> FuncDoc {
    FuncDoc {
        name,
        category,
        usage,
        summary,
        math: Some(math),
    }
}

const AGG: &str = "Aggregate";
const SHEET: &str = "Sheet";
const CONVERT: &str = "Convert";
const MATH: &str = "Math";
const LOG: &str = "Log & Exp";
const TRIG: &str = "Trigonometry";
const TIME: &str = "Time & Date";

/// Every function, grouped by category in the order the directory shows them.
pub const FUNCTIONS: &[FuncDoc] = &[
    doc(
        AGG,
        "sum",
        "Sheet.Column.sum()",
        "The total of a column or vector, a block of cells, or several taken together: sum(A, B). Empty cells are skipped.",
    ),
    doc(
        AGG,
        "mean",
        "Sheet.Column.mean()",
        "The mean of a column or vector, a block of cells, or several taken together, as an exact fraction where it can be.",
    ),
    doc(
        AGG,
        "min",
        "Sheet.Column.min()",
        "The smallest value, of one vector or several. Works on numbers, text, dates and times.",
    ),
    doc(
        AGG,
        "max",
        "Sheet.Column.max()",
        "The largest value, of one vector or several. Works on numbers, text, dates and times.",
    ),
    doc(
        AGG,
        "count",
        "Sheet.Column.count()",
        "How many cells are not empty, of one vector or several. For how many rows a sheet has, see len.",
    ),
    doc(
        AGG,
        "len",
        "Sheet.len()",
        "How many rows a sheet has, or how many cells a column or vector has, empty ones included.",
    ),
    doc(
        AGG,
        "median",
        "Sheet.Column.median()",
        "The middle value of a column or vector, or the mean of the two middle ones. Empty cells are skipped.",
    ),
    doc(
        AGG,
        "product",
        "Sheet.Column.product()",
        "Every value multiplied together. Empty cells are skipped.",
    ),
    doc(
        AGG,
        "first",
        "Sheet.Column.first()",
        "The first value, empty or not.",
    ),
    doc(
        AGG,
        "last",
        "Sheet.Column.last()",
        "The last value, empty or not.",
    ),
    doc(
        AGG,
        "std",
        "Sheet.Column.std()",
        "The standard deviation, of a sample. std(x, 0) is of a whole population.",
    ),
    doc(
        AGG,
        "var",
        "Sheet.Column.var()",
        "The variance, of a sample. var(x, 0) is of a whole population.",
    ),
    doc(
        AGG,
        "quantile",
        "Sheet.Column.quantile(0.9)",
        "The value a fraction of the way through the values in order: quantile(x, 0.5) is the median.",
    ),
    doc(
        SHEET,
        "filter",
        "Sheet |> filter(Column == value)",
        "The rows of a sheet for which a condition holds.",
    ),
    doc(
        SHEET,
        "select",
        "Sheet |> select(Column1, Column2)",
        "A sheet with only the named columns.",
    ),
    doc(
        SHEET,
        "head",
        "Sheet |> head(5)",
        "The first rows of a sheet, or cells of a column.",
    ),
    doc(
        SHEET,
        "tail",
        "Sheet |> tail(5)",
        "The last rows of a sheet, or cells of a column.",
    ),
    doc(
        SHEET,
        "shift",
        "Sheet.Column.shift(1)",
        "The value a number of rows back, empty where there is no such row.",
    ),
    doc(
        CONVERT,
        "cast",
        "cast(x, Int)",
        "A value as another type, named by the type: the same as Int(x) and x.Int.",
    ),
    doc(
        MATH,
        "is_null",
        "Sheet.Column.is_null()",
        "Whether a cell is empty.",
    ),
    doc(
        MATH,
        "is_not_null",
        "Sheet.Column.is_not_null()",
        "Whether a cell holds a value.",
    ),
    doc(
        MATH,
        "fill_null",
        "fill_null(x, 0)",
        "The value, or something else where it is empty. The same as x // 0.",
    ),
    doc(
        MATH,
        "is_in",
        "is_in(x, [1, 2, 3])",
        "Whether a value is one of several. The same as x in [1, 2, 3].",
    ),
    doc(
        MATH,
        "succ",
        "succ(x)",
        "The next value: a number plus one, or the next text in the way letters and digits count up, AZ to BA. The step a sequence takes when its terms give no other.",
    ),
    math_doc(
        MATH,
        "abs",
        "abs(x)",
        "The size of a number without its sign, or the modulus of a complex number.",
        MathFn::Abs,
    ),
    math_doc(
        MATH,
        "sign",
        "sign(x)",
        "-1, 0 or 1 according to the sign of a number.",
        MathFn::Sign,
    ),
    math_doc(
        MATH,
        "round",
        "round(x, 2)",
        "The nearest whole number, or with a count of places the nearest at that many decimal places. A half rounds away from zero. For the nearest multiple of something, round(x / 100) * 100.",
        MathFn::Round,
    ),
    math_doc(
        MATH,
        "floor",
        "floor(x)",
        "The largest whole number not above x.",
        MathFn::Floor,
    ),
    math_doc(
        MATH,
        "ceil",
        "ceil(x)",
        "The smallest whole number not below x.",
        MathFn::Ceil,
    ),
    math_doc(
        MATH,
        "sqrt",
        "sqrt(x)",
        "The square root, as a Float. A negative x needs a complex number: sqrt(-4+0i).",
        MathFn::Sqrt,
    ),
    doc(
        CONVERT,
        "Int",
        "x.Int",
        "A whole number: a fraction loses its fractional part, so 19.99 gives 19 and -19.99 \
         gives -19. Also written Int(x).",
    ),
    doc(
        CONVERT,
        "Decimal",
        "x.Decimal",
        "The same exact number shown as a decimal: a Num as the decimal it is shown as, and \
         1/8 as 0.125. One that does not end, such as 1/3, is shown cut and marked, 0.333…, \
         and held in full. Also written Decimal(x).",
    ),
    doc(
        CONVERT,
        "Fraction",
        "x.Fraction",
        "The same exact number shown as a fraction of whole numbers: 0.125 gives 1/8, 1/7 stays \
         1/7, and 2.75 gives 2 3/4. One with no fraction of a small enough denominator is shown \
         as a decimal. Also written Fraction(x).",
    ),
    doc(
        CONVERT,
        "Percent",
        "x.Percent",
        "The same number as a percentage: 0.4 gives 40% and 5/12 gives 41.67…%. Also written \
         Percent(x).",
    ),
    doc(
        CONVERT,
        "Float",
        "x.Float",
        "A floating-point Float from an exact number. Also written Float(x).",
    ),
    doc(
        CONVERT,
        "String",
        "x.String",
        "Any value as text, written the way a file writes it. Also written String(x).",
    ),
    doc(
        CONVERT,
        "Boolean",
        "x.Boolean",
        "true for a number that is not zero, false for zero. Also written Boolean(x).",
    ),
    doc(
        CONVERT,
        "Date",
        "x.Date",
        "The Date of a Datetime, or of text such as \"2025-01-31\". Also written Date(x).",
    ),
    doc(
        CONVERT,
        "Time",
        "x.Time",
        "The Time of a Datetime, or of text such as \"09:30\". Also written Time(x).",
    ),
    doc(
        CONVERT,
        "Datetime",
        "x.Datetime",
        "A Date at midnight, or text such as \"2025-01-31T09:30\". Also written Datetime(x).",
    ),
    math_doc(
        LOG,
        "exp",
        "exp(x)",
        "e raised to the power x.",
        MathFn::Exp,
    ),
    doc(
        LOG,
        "e",
        "e()",
        "The base of the natural logarithm, 2.71828...",
    ),
    math_doc(
        LOG,
        "ln",
        "ln(x)",
        "The natural logarithm. x must be positive.",
        MathFn::Ln,
    ),
    math_doc(
        LOG,
        "log10",
        "log10(x)",
        "The logarithm to base 10. x must be positive.",
        MathFn::Log10,
    ),
    math_doc(
        LOG,
        "log2",
        "log2(x)",
        "The logarithm to base 2. x must be positive.",
        MathFn::Log2,
    ),
    math_doc(
        TRIG,
        "sin",
        "sin(x)",
        "The sine of an angle in radians.",
        MathFn::Sin,
    ),
    math_doc(
        TRIG,
        "cos",
        "cos(x)",
        "The cosine of an angle in radians.",
        MathFn::Cos,
    ),
    math_doc(
        TRIG,
        "tan",
        "tan(x)",
        "The tangent of an angle in radians.",
        MathFn::Tan,
    ),
    math_doc(
        TRIG,
        "arcsin",
        "arcsin(x)",
        "The angle in radians whose sine is x, for x from -1 to 1.",
        MathFn::Arcsin,
    ),
    math_doc(
        TRIG,
        "arccos",
        "arccos(x)",
        "The angle in radians whose cosine is x, for x from -1 to 1.",
        MathFn::Arccos,
    ),
    math_doc(
        TRIG,
        "arctan",
        "arctan(x)",
        "The angle in radians whose tangent is x.",
        MathFn::Arctan,
    ),
    math_doc(
        TRIG,
        "sinh",
        "sinh(x)",
        "The hyperbolic sine.",
        MathFn::Sinh,
    ),
    math_doc(
        TRIG,
        "cosh",
        "cosh(x)",
        "The hyperbolic cosine.",
        MathFn::Cosh,
    ),
    math_doc(
        TRIG,
        "tanh",
        "tanh(x)",
        "The hyperbolic tangent.",
        MathFn::Tanh,
    ),
    math_doc(
        TRIG,
        "radians",
        "radians(180)",
        "An angle in degrees, in radians.",
        MathFn::Radians,
    ),
    math_doc(
        TRIG,
        "degrees",
        "degrees(pi())",
        "An angle in radians, in degrees.",
        MathFn::Degrees,
    ),
    doc(
        TRIG,
        "pi",
        "pi()",
        "The ratio of a circle's circumference to its diameter, 3.14159...",
    ),
    doc(
        TIME,
        "today",
        "today()",
        "The date now, in the file's time zone.",
    ),
    doc(
        TIME,
        "now",
        "now()",
        "The date and time now, in the file's time zone.",
    ),
    doc(
        TIME,
        "year",
        "Sheet.Column.year()",
        "The year of a Date or Datetime.",
    ),
    doc(
        TIME,
        "month",
        "Sheet.Column.month()",
        "The month, 1 to 12, of a Date or Datetime.",
    ),
    doc(
        TIME,
        "day",
        "Sheet.Column.day()",
        "The day of the month of a Date or Datetime.",
    ),
    doc(
        TIME,
        "add_months",
        "add_months(x, n)",
        "A date n months on, keeping the day of the month, or the last day of a month too short for it: a month after 2025-01-31 is 2025-02-28. Write it as x + n.month, and x + n.year for years.",
    ),
    doc(
        TIME,
        "month_end",
        "month_end(x)",
        "The last day of the month a date is in.",
    ),
    doc(
        TIME,
        "month_start",
        "month_start(x)",
        "The first day of the month a date is in.",
    ),
    doc(
        TIME,
        "quarter",
        "Sheet.Column.quarter()",
        "The quarter of the year, 1 to 4.",
    ),
    doc(
        TIME,
        "week",
        "Sheet.Column.week()",
        "The ISO week of the year, 1 to 53. A week belongs to the year its Thursday is in.",
    ),
    doc(
        TIME,
        "ordinal_day",
        "Sheet.Column.ordinal_day()",
        "The day of the year, 1 to 366.",
    ),
    doc(
        TIME,
        "weekday",
        "Sheet.Column.weekday()",
        "The day of the week, 1 for Monday to 7 for Sunday.",
    ),
    doc(
        TIME,
        "hour",
        "Sheet.Column.hour()",
        "The hour, 0 to 23, of a Time or Datetime.",
    ),
    doc(
        TIME,
        "minute",
        "Sheet.Column.minute()",
        "The minute of a Time or Datetime.",
    ),
    doc(
        TIME,
        "second",
        "Sheet.Column.second()",
        "The second of a Time or Datetime.",
    ),
    doc(
        TIME,
        "date",
        "Sheet.Column.date()",
        "The Date half of a Datetime.",
    ),
    doc(
        TIME,
        "time",
        "Sheet.Column.time()",
        "The Time half of a Datetime.",
    ),
    doc(
        TIME,
        "convert_time_zone",
        "Column.convert_time_zone(\"Australia/Adelaide\")",
        "The same instant on the clocks of a named time zone.",
    ),
    doc(
        TIME,
        "utc",
        "Column.utc()",
        "The same instant on the clocks of UTC.",
    ),
    doc(
        TIME,
        "local",
        "Column.local()",
        "The same instant on the clocks of this machine.",
    ),
    doc(
        TIME,
        "base_utc_offset",
        "Column.base_utc_offset()",
        "The seconds a Datetime is ahead of UTC.",
    ),
    doc(
        TIME,
        "time_zone",
        "Column.time_zone()",
        "The name of a Datetime's time zone.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_math_is_listed() {
        for (i, f) in FUNCTIONS.iter().enumerate() {
            assert!(
                !FUNCTIONS[..i].iter().any(|g| g.name == f.name),
                "{}",
                f.name
            );
            assert!(f.usage.contains(f.name), "{}", f.name);
        }
        // One table: a `MathFn` is found by the name it is listed under,
        // and is listed exactly once.
        for f in FUNCTIONS {
            if let Some(m) = f.math {
                assert_eq!(MathFn::from_name(f.name), Some(m), "{}", f.name);
                assert_eq!(m.name(), f.name);
            }
        }
        for m in [MathFn::Abs, MathFn::Round, MathFn::Arcsin, MathFn::Degrees] {
            assert!(!m.name().is_empty());
        }
    }
}
