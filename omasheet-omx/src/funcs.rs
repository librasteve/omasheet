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
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Radians,
    Degrees,
}

impl MathFn {
    const ALL: [(&'static str, MathFn); 21] = [
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
            MathFn::Asin => x.asin(),
            MathFn::Acos => x.acos(),
            MathFn::Atan => x.atan(),
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
        "avg",
        "Sheet.Column.avg()",
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
        "How many cells are not empty, of one vector or several; for a sheet, how many rows.",
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
        MATH,
        "succ",
        "succ(x)",
        "The next value: a number plus one, or the next text in the way letters and digits count up, AZ to BA. The step a sequence takes when its terms give no other.",
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
        "round(x, scale)",
        "The nearest whole number, or with a scale the nearest multiple of it: round(x, 0.01) is to the hundredth, round(x, 100) to the hundred. A half rounds away from zero.",
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
        "The square root, as a Float. A negative x needs a complex number: sqrt(-4+0i).",
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
        "The year of a Date or DateTime.",
    ),
    doc(
        TIME,
        "month",
        "Sheet.Column.month()",
        "The month, 1 to 12, of a Date or DateTime.",
    ),
    doc(
        TIME,
        "day",
        "Sheet.Column.day()",
        "The day of the month of a Date or DateTime.",
    ),
    doc(
        TIME,
        "add_months",
        "add_months(x, n)",
        "A date n months on, keeping the day of the month, or the last day of a month too short for it: a month after 2025-01-31 is 2025-02-28. Write it as x + n.month, and x + n.year for years.",
    ),
    doc(
        TIME,
        "eom",
        "eom(x)",
        "The last day of the month a date is in.",
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
        "The hour, 0 to 23, of a Time or DateTime.",
    ),
    doc(
        TIME,
        "minute",
        "Sheet.Column.minute()",
        "The minute of a Time or DateTime.",
    ),
    doc(
        TIME,
        "second",
        "Sheet.Column.second()",
        "The second of a Time or DateTime.",
    ),
    doc(
        TIME,
        "date",
        "Sheet.Column.date()",
        "The Date half of a DateTime.",
    ),
    doc(
        TIME,
        "time",
        "Sheet.Column.time()",
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
