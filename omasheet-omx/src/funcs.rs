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
const PIPE: &str = "Pipe";
const CONVERT: &str = "Convert";
const MATH: &str = "Math";
const LOG: &str = "Log & Exp";
const TRIG: &str = "Trigonometry";
const TIME: &str = "Time & Date";
const BITWISE: &str = "Bitwise";

/// What the function directory says first: how a function is written.
pub const INTRO: &str = "Functions are written =sum(x), many can also be used a method applied to an object e.g. \
     =x.sum().";

/// What the functions of each category are applied to, said under its heading.
pub const ABOUT: &[(&str, &str)] = &[
    (
        AGG,
        "Object: any collection - Sheet, Column, vector or cell block. For several: sum(A, B). \
         Empty(null) cells usually skipped.",
    ),
    (
        PIPE,
        "Left-hand side of pipe operator(|>): a Sheet or Column.",
    ),
    (
        MATH,
        "Object: any - a single cell, Sheet, Column, vector or a cell block. Usually must contain \
         a number.",
    ),
    (CONVERT, "Object: a Value."),
    (LOG, "Takes: a number."),
    (TRIG, "Takes: a number as an angle in radians."),
    (TIME, "Object: Date, Time or Datetime."),
    (
        BITWISE,
        "Operators, not functions: written between two whole numbers, or before one. and, or \
         and not stay Boolean.",
    ),
];

/// Every function, grouped by category in the order the directory shows them.
pub const FUNCTIONS: &[FuncDoc] = &[
    doc(AGG, "sum", ".sum()", "The total."),
    doc(AGG, "mean", ".mean()", "The average."),
    doc(
        AGG,
        "min",
        ".min()",
        "The smallest. Works on numbers, text order, dates and times.",
    ),
    doc(
        AGG,
        "max",
        ".max()",
        "The largest. Works on numbers, text order, dates and times.",
    ),
    doc(AGG, "count", ".count()", "Cells in a column or cell block."),
    doc(
        AGG,
        "len",
        ".len()",
        "Rows in a sheet, or cells in a column: empty ones included.",
    ),
    doc(
        AGG,
        "median",
        ".median()",
        "The middle value, or the mean of the two middle ones.",
    ),
    doc(
        AGG,
        "product",
        ".product()",
        "Every value multiplied together.",
    ),
    doc(AGG, "first", ".first()", "The first value, empty or not."),
    doc(AGG, "last", ".last()", "The last value, empty or not."),
    doc(
        AGG,
        "std",
        ".std()",
        "The standard deviation, of a sample. std(x, 0) is of a whole population.",
    ),
    doc(
        AGG,
        "var",
        ".var()",
        "The variance, of a sample. var(x, 0) is of a whole population.",
    ),
    doc(
        AGG,
        "quantile",
        ".quantile(0.9)",
        "The value a fraction of the way through the values in order: quantile(x, 0.5) is the \
         median.",
    ),
    doc(
        PIPE,
        "filter",
        "|> filter(Column == value)",
        "The rows for which a condition holds.",
    ),
    doc(
        PIPE,
        "select",
        "|> select(Column1, Column2)",
        "A sheet with only the named columns.",
    ),
    doc(
        PIPE,
        "head",
        "|> head(5)",
        "The first rows of a sheet, or cells of a column.",
    ),
    doc(
        PIPE,
        "tail",
        "|> tail(5)",
        "The last rows of a sheet, or cells of a column.",
    ),
    doc(
        PIPE,
        "shift",
        ".shift(1)",
        "The value a number of rows back, empty where there is no such row.",
    ),
    doc(
        PIPE,
        "sort",
        "|> sort(Column, desc)",
        "The rows of a sheet in the order of one or more columns, or the cells of a column in \
         their own order. Ascending unless the last word is desc; an empty cell comes first.",
    ),
    doc(
        PIPE,
        "group_by",
        "sheet ByRegion := Sales |> group_by(Region) |> agg(Revenue.sum())",
        "The rows of a sheet in groups, one for each value of the key columns, for agg to work \
         out. It ends a sheet's formula, with agg after it.",
    ),
    doc(
        PIPE,
        "join",
        "sheet Both := Sales |> join(Targets, Month, left)",
        "The rows of two sheets paired where their key columns are equal, a row for every pair. \
         Inner unless the last word is left, which keeps a row that matches none. An empty key \
         matches nothing.",
    ),
    doc(
        PIPE,
        "agg",
        "Sales |> group_by(Region) |> agg(Revenue.sum(), Orders := Item.len())",
        "A row for each group: the key columns, then an aggregate of each group's rows. \
         Revenue.sum() makes the column Revenue_sum, and Name := names it. The groups are in the \
         order of their keys.",
    ),
    doc(MATH, "is_null", ".is_null()", "Whether a cell is empty."),
    doc(
        MATH,
        "is_not_null",
        ".is_not_null()",
        "Whether a cell holds a value.",
    ),
    doc(
        MATH,
        "is_nan",
        ".is_nan()",
        "Whether a cell holds NaN (Not a Number).",
    ),
    doc(
        MATH,
        "is_not_nan",
        ".is_not_nan()",
        "Whether a cell is not a NaN.",
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
        "The next value: a number plus one, or the text in alphabetical order, AZ to BA.",
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
        "The nearest whole number, or the nearest at count decimal places. A half rounds away from \
         zero.",
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
    doc(CONVERT, "Integer", ".Integer", "A whole number: truncates."),
    doc(
        CONVERT,
        "Hex",
        ".Hex",
        "A whole number shown in hex, as Integer(x) gives it: 255 gives 0xff. Also written \
         Hex(x).",
    ),
    doc(
        CONVERT,
        "Octal",
        ".Octal",
        "A whole number shown in octal, as Integer(x) gives it: 15 gives 0o17. Also written \
         Octal(x).",
    ),
    doc(
        CONVERT,
        "Binary",
        ".Binary",
        "A whole number shown in binary, as Integer(x) gives it: 5 gives 0b101. Also written \
         Binary(x).",
    ),
    doc(
        CONVERT,
        "Decimal",
        ".Decimal",
        "An exact number shown as a decimal.",
    ),
    doc(
        CONVERT,
        "Fraction",
        ".Fraction",
        "A Decimal shown as a fraction.",
    ),
    doc(
        CONVERT,
        "Percent",
        ".Percent",
        "A Decimal shown as a percentage (%).",
    ),
    doc(CONVERT, "Float", ".Float", "A floating-point number."),
    doc(CONVERT, "String", ".String", "Any value as text."),
    doc(
        CONVERT,
        "Boolean",
        ".Boolean",
        "true for a number that is not zero, false for zero.",
    ),
    doc(
        CONVERT,
        "Date",
        ".Date",
        "The Date of a Datetime, or of text such as \"2025-01-31\".",
    ),
    doc(
        CONVERT,
        "Time",
        ".Time",
        "The Time of a Datetime, or of text such as \"09:30\".",
    ),
    doc(
        CONVERT,
        "Datetime",
        ".Datetime",
        "A Date at midnight, or text such as \"2025-01-31T09:30\".",
    ),
    doc(
        CONVERT,
        "cast",
        "cast(x, Int)",
        "A value as another type: the same as Int(x) and x.Int.",
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
    math_doc(TRIG, "sin", "sin(x)", "The sine.", MathFn::Sin),
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
    doc(TIME, "year", ".year()", "The year of a Date or Datetime."),
    doc(TIME, "month", ".month()", "The month, 1 to 12."),
    doc(TIME, "day", ".day()", "The day."),
    doc(
        TIME,
        "add_months",
        "add_months(x, n)",
        "A date n months on, keeping the day of the month, or the last day of a month too short. \
         Write it as x + n.month, and x + n.year for years.",
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
        ".quarter()",
        "The quarter of the year, 1 to 4.",
    ),
    doc(
        TIME,
        "week",
        ".week()",
        "The ISO week of the year, 1 to 53. A week belongs to the year its Thursday is in.",
    ),
    doc(
        TIME,
        "ordinal_day",
        ".ordinal_day()",
        "The day of the year, 1 to 366.",
    ),
    doc(
        TIME,
        "weekday",
        ".weekday()",
        "The day of the week, 1 for Monday to 7 for Sunday.",
    ),
    doc(TIME, "hour", ".hour()", "The hour, 0 to 23."),
    doc(TIME, "minute", ".minute()", "The minute."),
    doc(TIME, "second", ".second()", "The second."),
    doc(
        TIME,
        "millisecond",
        ".millisecond()",
        "The milliseconds left over after the second: 500 of 09:30:00.5.",
    ),
    doc(
        TIME,
        "microsecond",
        ".microsecond()",
        "The microseconds left over after the second: 500000 of 09:30:00.5.",
    ),
    doc(
        TIME,
        "nanosecond",
        ".nanosecond()",
        "The nanoseconds left over after the second of a Time: 500000000 of 09:30:00.5. A Datetime \
         is held to the microsecond, ends in 000.",
    ),
    doc(TIME, "date", ".date()", "The Date part of a Datetime."),
    doc(TIME, "time", ".time()", "The Time partf of a Datetime."),
    doc(
        TIME,
        "convert_time_zone",
        ".convert_time_zone(\"Australia/Adelaide\")",
        "The same instant on the clocks of a named time zone.",
    ),
    doc(
        TIME,
        "utc",
        ".utc()",
        "The same instant on the clocks of UTC.",
    ),
    doc(
        TIME,
        "local",
        ".local()",
        "The same instant on the clocks of this machine.",
    ),
    doc(
        TIME,
        "base_utc_offset",
        ".base_utc_offset()",
        "The seconds a Datetime is ahead of UTC.",
    ),
    doc(
        TIME,
        "time_zone",
        ".time_zone()",
        "The name of a Datetime's time zone.",
    ),
    // Operators, listed by their symbols, which no function can be named.
    doc(
        BITWISE,
        "+&",
        "a +& b",
        "Bitwise and: the bits set in both.",
    ),
    doc(
        BITWISE,
        "+|",
        "a +| b",
        "Bitwise or: the bits set in either.",
    ),
    doc(
        BITWISE,
        "+^",
        "a +^ b",
        "Bitwise xor: the bits set in one but not both.",
    ),
    doc(
        BITWISE,
        "+^a",
        "+^a",
        "The complement: every bit flipped, so +^0xff gives 0xffffffffffffff00.",
    ),
    doc(BITWISE, "+<", "a +< n", "Shift left by n bits, 0 to 63."),
    doc(
        BITWISE,
        "+>",
        "a +> n",
        "Shift right by n bits, keeping the sign: -16 +> 2 gives -4.",
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
