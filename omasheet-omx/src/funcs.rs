// Copyright (c) 2026 Stephen Roe

//! The functions of OMX: what each is called, and what a person browsing
//! them is told. The checker and the function directory both read this.

use crate::types::S;

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
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Radians,
    Degrees,
    /// The real part of a complex number.
    Re,
    /// The imaginary part.
    Im,
    /// The complex conjugate.
    Conj,
    /// The angle of a complex number, in radians.
    Arg,
}

impl MathFn {
    const ALL: [(&'static str, MathFn); 25] = [
        ("abs", MathFn::Abs),
        ("sign", MathFn::Sign),
        ("round", MathFn::Round),
        ("floor", MathFn::Floor),
        ("ceil", MathFn::Ceil),
        ("sqrt", MathFn::Sqrt),
        ("exp", MathFn::Exp),
        ("ln", MathFn::Ln),
        ("log10", MathFn::Log10),
        ("log2", MathFn::Log2),
        ("sin", MathFn::Sin),
        ("cos", MathFn::Cos),
        ("tan", MathFn::Tan),
        ("asin", MathFn::Asin),
        ("acos", MathFn::Acos),
        ("atan", MathFn::Atan),
        ("sinh", MathFn::Sinh),
        ("cosh", MathFn::Cosh),
        ("tanh", MathFn::Tanh),
        ("radians", MathFn::Radians),
        ("degrees", MathFn::Degrees),
        ("re", MathFn::Re),
        ("im", MathFn::Im),
        ("conj", MathFn::Conj),
        ("arg", MathFn::Arg),
    ];

    pub fn from_name(name: &str) -> Option<MathFn> {
        MathFn::ALL
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, m)| *m)
    }

    pub fn name(self) -> &'static str {
        MathFn::ALL
            .iter()
            .find(|(_, m)| *m == self)
            .map_or("", |(n, _)| n)
    }

    /// Whether an exact number gives an exact result.
    pub fn is_exact(self) -> bool {
        matches!(
            self,
            MathFn::Abs
                | MathFn::Sign
                | MathFn::Round
                | MathFn::Floor
                | MathFn::Ceil
                | MathFn::Re
                | MathFn::Im
                | MathFn::Conj
        )
    }

    /// Whether the result is always a whole number, given an exact one.
    pub fn is_whole(self) -> bool {
        self.is_exact() && !matches!(self, MathFn::Abs | MathFn::Re | MathFn::Conj)
    }

    /// The type of the result for a complex number, or `None` if the
    /// function does not take one.
    pub fn of_complex(self) -> Option<S> {
        match self {
            MathFn::Abs | MathFn::Re | MathFn::Im | MathFn::Arg => Some(S::Num),
            MathFn::Conj | MathFn::Sqrt | MathFn::Exp | MathFn::Ln => Some(S::Complex),
            _ => None,
        }
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
            MathFn::Asin => x.asin(),
            MathFn::Acos => x.acos(),
            MathFn::Atan => x.atan(),
            MathFn::Sinh => x.sinh(),
            MathFn::Cosh => x.cosh(),
            MathFn::Tanh => x.tanh(),
            MathFn::Radians => x.to_radians(),
            MathFn::Degrees => x.to_degrees(),
            MathFn::Re | MathFn::Conj => x,
            MathFn::Im => 0.0,
            MathFn::Arg if x < 0.0 => std::f64::consts::PI,
            MathFn::Arg => 0.0,
        }
    }
}

/// One entry of the function directory.
#[derive(Clone, Copy, Debug)]
pub struct FuncDoc {
    pub name: &'static str,
    pub category: &'static str,
    /// How it is written, such as `Table.Column.sum()`.
    pub usage: &'static str,
    pub summary: &'static str,
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
    }
}

const AGG: &str = "Aggregate";
const TABLE: &str = "Table";
const CONVERT: &str = "Convert";
const MATH: &str = "Math";
const LOG: &str = "Log & Exp";
const TRIG: &str = "Trigonometry";
const COMPLEX: &str = "Complex";
const TIME: &str = "Time & Date";

/// Every function, grouped by category in the order the directory shows them.
pub const FUNCTIONS: &[FuncDoc] = &[
    doc(
        AGG,
        "sum",
        "Table.Column.sum()",
        "The total of a column or vector. Empty cells are skipped.",
    ),
    doc(
        AGG,
        "avg",
        "Table.Column.avg()",
        "The mean of a column or vector, as an exact fraction where it can be.",
    ),
    doc(
        AGG,
        "min",
        "Table.Column.min()",
        "The smallest value. Works on numbers, text, dates and times.",
    ),
    doc(
        AGG,
        "max",
        "Table.Column.max()",
        "The largest value. Works on numbers, text, dates and times.",
    ),
    doc(
        AGG,
        "count",
        "Table.Column.count()",
        "How many cells are not empty; for a table, how many rows.",
    ),
    doc(
        TABLE,
        "filter",
        "Table |> filter(Column == value)",
        "The rows of a table for which a condition holds.",
    ),
    doc(
        TABLE,
        "select",
        "Table |> select(Column1, Column2)",
        "A table with only the named columns.",
    ),
    doc(
        MATH,
        "abs",
        "abs(x)",
        "The size of a number without its sign, or the modulus of a complex number.",
    ),
    doc(
        MATH,
        "sign",
        "sign(x)",
        "-1, 0 or 1 according to the sign of a number.",
    ),
    doc(
        MATH,
        "round",
        "round(x)",
        "The nearest whole number; a half rounds away from zero.",
    ),
    doc(
        MATH,
        "floor",
        "floor(x)",
        "The largest whole number not above x.",
    ),
    doc(
        MATH,
        "ceil",
        "ceil(x)",
        "The smallest whole number not below x.",
    ),
    doc(
        MATH,
        "sqrt",
        "sqrt(x)",
        "The square root, as a Num. A negative x needs a complex number: sqrt(-4+0i).",
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
        "An exact number that ends as a decimal: a Num as the decimal it is shown as, and a \
         Ratio such as 1/8 as 0.125. One that does not end, such as 1/3, is an error. Also \
         written Decimal(x).",
    ),
    doc(
        CONVERT,
        "Ratio",
        "x.Ratio",
        "An exact number: a Num as the decimal it is shown as. Also written Ratio(x).",
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
        "Num",
        "x.Num",
        "A floating-point Num from an exact number. Also written Num(x).",
    ),
    doc(
        CONVERT,
        "Text",
        "x.Text",
        "Any value as text, written the way a sheet writes it. Also written Text(x).",
    ),
    doc(
        CONVERT,
        "Bool",
        "x.Bool",
        "true for a number that is not zero, false for zero. Also written Bool(x).",
    ),
    doc(
        CONVERT,
        "Date",
        "x.Date",
        "The Date of a DateTime, or of text such as \"2025-01-31\". Also written Date(x).",
    ),
    doc(
        CONVERT,
        "Time",
        "x.Time",
        "The Time of a DateTime, or of text such as \"09:30\". Also written Time(x).",
    ),
    doc(
        CONVERT,
        "DateTime",
        "x.DateTime",
        "A Date at midnight, or text such as \"2025-01-31T09:30\". Also written DateTime(x).",
    ),
    doc(LOG, "exp", "exp(x)", "e raised to the power x."),
    doc(
        LOG,
        "e",
        "e()",
        "The base of the natural logarithm, 2.71828...",
    ),
    doc(
        LOG,
        "ln",
        "ln(x)",
        "The natural logarithm. x must be positive.",
    ),
    doc(
        LOG,
        "log10",
        "log10(x)",
        "The logarithm to base 10. x must be positive.",
    ),
    doc(
        LOG,
        "log2",
        "log2(x)",
        "The logarithm to base 2. x must be positive.",
    ),
    doc(TRIG, "sin", "sin(x)", "The sine of an angle in radians."),
    doc(TRIG, "cos", "cos(x)", "The cosine of an angle in radians."),
    doc(TRIG, "tan", "tan(x)", "The tangent of an angle in radians."),
    doc(
        TRIG,
        "asin",
        "asin(x)",
        "The angle in radians whose sine is x, for x from -1 to 1.",
    ),
    doc(
        TRIG,
        "acos",
        "acos(x)",
        "The angle in radians whose cosine is x, for x from -1 to 1.",
    ),
    doc(
        TRIG,
        "atan",
        "atan(x)",
        "The angle in radians whose tangent is x.",
    ),
    doc(TRIG, "sinh", "sinh(x)", "The hyperbolic sine."),
    doc(TRIG, "cosh", "cosh(x)", "The hyperbolic cosine."),
    doc(TRIG, "tanh", "tanh(x)", "The hyperbolic tangent."),
    doc(
        TRIG,
        "radians",
        "radians(180)",
        "An angle in degrees, in radians.",
    ),
    doc(
        TRIG,
        "degrees",
        "degrees(pi())",
        "An angle in radians, in degrees.",
    ),
    doc(
        TRIG,
        "pi",
        "pi()",
        "The ratio of a circle's circumference to its diameter, 3.14159...",
    ),
    doc(
        COMPLEX,
        "Complex",
        "Complex(re, im)",
        "A complex number from its real and imaginary parts, or from a real number: x.Complex. \
         Also written 3+4i.",
    ),
    doc(COMPLEX, "re", "re(z)", "The real part of a complex number."),
    doc(
        COMPLEX,
        "im",
        "im(z)",
        "The imaginary part of a complex number.",
    ),
    doc(
        COMPLEX,
        "conj",
        "conj(z)",
        "The complex conjugate: the imaginary part with its sign changed.",
    ),
    doc(
        COMPLEX,
        "arg",
        "arg(z)",
        "The angle of a complex number, in radians from -pi to pi.",
    ),
    doc(
        TIME,
        "today",
        "today()",
        "The date now, in the sheet's time zone.",
    ),
    doc(
        TIME,
        "now",
        "now()",
        "The date and time now, in the sheet's time zone.",
    ),
    doc(
        TIME,
        "year",
        "Table.Column.year()",
        "The year of a Date or DateTime.",
    ),
    doc(
        TIME,
        "month",
        "Table.Column.month()",
        "The month, 1 to 12, of a Date or DateTime.",
    ),
    doc(
        TIME,
        "day",
        "Table.Column.day()",
        "The day of the month of a Date or DateTime.",
    ),
    doc(
        TIME,
        "weekday",
        "Table.Column.weekday()",
        "The day of the week, 1 for Monday to 7 for Sunday.",
    ),
    doc(
        TIME,
        "hour",
        "Table.Column.hour()",
        "The hour, 0 to 23, of a Time or DateTime.",
    ),
    doc(
        TIME,
        "minute",
        "Table.Column.minute()",
        "The minute of a Time or DateTime.",
    ),
    doc(
        TIME,
        "second",
        "Table.Column.second()",
        "The second of a Time or DateTime.",
    ),
    doc(
        TIME,
        "date",
        "Table.Column.date()",
        "The Date half of a DateTime.",
    ),
    doc(
        TIME,
        "time",
        "Table.Column.time()",
        "The Time half of a DateTime.",
    ),
    doc(
        TIME,
        "to_zone",
        "Column.to_zone(\"Australia/Adelaide\")",
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
        "offset",
        "Column.offset()",
        "The seconds a DateTime is ahead of UTC.",
    ),
    doc(
        TIME,
        "zone",
        "Column.zone()",
        "The name of a DateTime's time zone.",
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
        for (name, m) in MathFn::ALL {
            assert_eq!(MathFn::from_name(name), Some(m));
            assert!(FUNCTIONS.iter().any(|f| f.name == name), "{name}");
        }
    }
}
