// Copyright (c) 2026 Stephen Roe

//! The scenarios of `openspec/changes/add-si-units`: a number with an SI unit,
//! written `5kg`, `10.5(3)e-3kg` or `9.81m.s^-2`, and a scale, `1k.`.

use omasheet_engine::{Options, eval, eval_with, lint, view};

/// Evaluate an expression as the grid shows it.
fn shown(expr: &str) -> String {
    let out = eval(None, "<expression>", expr);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

/// Evaluate an expression with every number in full, as a file writes it.
fn full(expr: &str) -> String {
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = eval_with(None, "<expression>", expr, options);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

fn fails(expr: &str) -> String {
    let out = eval(None, "<expression>", expr);
    assert!(!out.ok(), "`{expr}` should fail but gave {}", out.output);
    out.errors.join("\n")
}

/// The value, the uncertainty and the unit of a quantity, from its full text.
fn parts(expr: &str) -> (f64, f64, String) {
    quantity_parts(&full(expr))
}

/// The same from the full text of a quantity: `10.5 ± 0.3kg`.
fn quantity_parts(text: &str) -> (f64, f64, String) {
    // The unit starts at the first letter that is not an exponent's `e`.
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let split = chars
        .iter()
        .enumerate()
        .find(|(i, (_, c))| {
            let exponent = matches!(c, 'e' | 'E')
                && chars
                    .get(i + 1)
                    .is_some_and(|(_, n)| n.is_ascii_digit() || matches!(n, '-' | '+'));
            (c.is_alphabetic() && !exponent) || *c == '°'
        })
        .map(|(_, (at, _))| *at)
        .unwrap_or_else(|| panic!("`{text}` is not a quantity"));
    let (number, unit) = text.split_at(split);
    let (v, s) = match number.split_once(" ± ") {
        Some((v, s)) => (v.parse().unwrap(), s.parse().unwrap()),
        None => (number.parse().unwrap(), 0.0),
    };
    (v, s, unit.to_string())
}

fn near(a: f64, b: f64) {
    assert!(
        a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()),
        "{a} is not {b}"
    );
}

// ---- literals --------------------------------------------------------------

#[test]
fn a_unit_after_a_number() {
    assert_eq!(full("5kg"), "5kg");
    assert_eq!(full("300cm"), "300cm");
    assert_eq!(full("9.81m.s^-2"), "9.81m.s^-2");
    assert_eq!(full("5kg.m^2.s^-2"), "5kg.m^2.s^-2");
    assert_eq!(full("20°C"), "20°C");
    // A compound is written in its own order, whichever it was read in.
    assert_eq!(full("5s^-2.m.kg"), "5kg.m.s^-2");
    // A named unit stays as it is written.
    assert_eq!(full("2kPa"), "2kPa");
}

#[test]
fn with_an_uncertainty() {
    let (v, s, u) = parts("10.5(3)e-3kg");
    near(v, 0.0105);
    near(s, 0.0003);
    assert_eq!(u, "kg");
    let (v, s, u) = parts("10.5(3)g");
    assert_eq!((v, s, u.as_str()), (10.5, 0.3, "g"));
}

#[test]
fn what_is_not_a_unit_is_an_error() {
    for bad in ["5 kg", "5km/s", "5m2", "5ft", "5L", "5min", "5kgx"] {
        let out = eval(None, "<expression>", bad);
        assert!(!out.ok(), "`{bad}` should fail, gave {}", out.output);
    }
}

#[test]
fn a_method_after_a_unit_is_a_method() {
    assert_eq!(full("5kg.abs()"), "5kg");
    assert_eq!(full("(-5kg).abs()"), "5kg");
}

// ---- scales ----------------------------------------------------------------

#[test]
fn a_scale_is_a_number() {
    assert_eq!(full("1k."), "1000");
    assert_eq!(full("1k. == 1000"), "true");
    assert_eq!(full("1.5k."), "1500");
    assert_eq!(full("1.5k. * 2"), "3000");
    assert_eq!(full("5m."), "0.005");
    assert_eq!(full("5m"), "5m");
    assert_eq!(full("2M. + 1k."), "2001000");
    assert_eq!(full("1T."), "1000000000000");
}

#[test]
fn a_scale_keeps_its_uncertainty() {
    assert_eq!(full("2.5(3)k."), "2500 ± 300");
}

// ---- arithmetic ------------------------------------------------------------

#[test]
fn units_convert_in_a_sum() {
    assert_eq!(full("5kg + 300g"), "5.3kg");
    assert_eq!(full("300g + 5kg"), "5300g");
    assert_eq!(full("5kg - 300g"), "4.7kg");
    assert_eq!(full("5m + 300cm == 8m"), "true");
    assert_eq!(full("5m + 300cm"), "8m");
}

#[test]
fn mismatched_dimensions_are_an_error() {
    let m = fails("5kg + 3m");
    assert!(
        m.contains("cannot add kg and m: they measure Mass and Length"),
        "{m}"
    );
    assert!(fails("5kg + 3").contains("cannot apply"));
    assert!(fails("5kg < 3m").contains("cannot compare kg and m"));
}

#[test]
fn products_and_quotients_combine_units() {
    assert_eq!(full("2m * 3m"), "6m^2");
    assert_eq!(full("2m * 3s"), "6m.s");
    assert_eq!(full("10m / 4s"), "2.5m.s^-1");
    assert_eq!(full("5kg * 2"), "10kg");
    assert_eq!(full("2 * 5kg"), "10kg");
    assert_eq!(full("10kg / 4"), "2.5kg");
    assert_eq!(full("1 / 4s"), "0.25s^-1");
    // Different prefixes of one unit stay apart; no rebasing.
    assert_eq!(full("5km * 2s"), "10km.s");
}

#[test]
fn powers_and_roots() {
    assert_eq!(full("2m ** 3"), "8m^3");
    assert_eq!(full("sqrt(4m^2 * 1)"), "2m");
    assert_eq!(full("sqrt(9 * 4m ** 2)"), "12m");
    assert!(fails("sqrt(4m)").contains("not divisible"));
    assert!(fails("2m ** 0.5").contains("whole power"));
}

#[test]
fn no_dimension_is_a_plain_number() {
    assert_eq!(full("6m / 3m"), "2");
    assert_eq!(full("6m / 3cm"), "200");
    assert_eq!(full("(6kg / 3kg) + 1"), "3");
    // With an uncertainty it is a Scientific.
    assert_eq!(full("6.0(1)m / 3m"), "2 ± 0.03333333333333333");
}

#[test]
fn temperatures_go_through_kelvin() {
    assert_eq!(full("20°C - 10°C"), "10K");
    let (v, s, u) = parts("20°C + 5K");
    near(v, 298.15);
    assert_eq!((s, u.as_str()), (0.0, "K"));
}

#[test]
fn negation_keeps_the_unit() {
    assert_eq!(full("-5kg"), "-5kg");
    assert_eq!(full("-(5kg)"), "-5kg");
    assert_eq!(full("-5kg - 300g"), "-5.3kg");
    // A number is not a quantity, so it has no place in a sum with one.
    assert!(fails("0 - 5kg").contains("cannot apply"));
}

// ---- the uncertainty --------------------------------------------------------

#[test]
fn uncertainty_propagates() {
    // In quadrature for a sum, 0.3 and 0.4 together are 0.5.
    let (v, s, u) = parts("3.0(3)m + 4.0(4)m");
    near(v, 7.0);
    near(s, 0.5);
    assert_eq!(u, "m");
    // A product: the partial derivatives.
    let (v, s, u) = parts("2.0(1)m * 3.0(3)s");
    near(v, 6.0);
    near(s, 0.3f64.hypot(0.2 * 3.0));
    assert_eq!(u, "m.s");
    // A conversion scales it.
    let (v, s, u) = parts("(2.0(1)km).to('m')");
    near(v, 2000.0);
    near(s, 100.0);
    assert_eq!(u, "m");
}

#[test]
fn plus_minus_on_a_quantity() {
    let (v, s, u) = parts("5kg ± 100g");
    near(v, 5.0);
    near(s, 0.1);
    assert_eq!(u, "kg");
    let (v, s, u) = parts("5kg ± 0.2");
    near(v, 5.0);
    near(s, 0.2);
    assert_eq!(u, "kg");
    assert!(fails("5kg ± 3m").contains("they measure Mass and Length"));
    assert!(fails("5kg ± (-1kg)").contains("negative"));
    assert!(fails("5.0(1)kg ± 1kg").contains("no uncertainty of its own"));
}

#[test]
fn quantities_are_equal_within_their_uncertainties() {
    assert_eq!(shown("5.0(1)kg == 5.1kg"), "true");
    assert_eq!(shown("5kg == 5.1kg"), "false");
    assert_eq!(shown("5kg < 6kg"), "true");
    assert_eq!(shown("5kg < 6000g"), "true");
    assert_eq!(shown("5kg >= 5000g"), "true");
    assert_eq!(shown("100cm == 1m"), "true");
}

// ---- conversion -------------------------------------------------------------

#[test]
fn to_another_unit() {
    assert_eq!(full("5kg.to('g')"), "5000g");
    assert_eq!(full("to(5kg, 'g')"), "5000g");
    assert_eq!(full("1.5km.to('m')"), "1500m");
    assert_eq!(full("100cm.to('m')"), "1m");
    assert_eq!(full("1N.to('kg.m.s^-2')"), "1kg.m.s^-2");
    let (v, _, u) = parts("39°C.to('K')");
    near(v, 312.15);
    assert_eq!(u, "K");
    let (v, _, u) = parts("(312.15K).to('°C')");
    near(v, 39.0);
    assert_eq!(u, "°C");
}

#[test]
fn a_conversion_to_the_wrong_dimension_is_an_error() {
    assert!(fails("5kg.to('m')").contains("kg is a Mass and m a Length"));
    assert!(fails("5kg.to('furlong')").contains("not a unit"));
    assert!(fails("5.to('g')").contains("quantity"));
    assert!(fails("'abc'.to('g')").contains("quantity"));
}

// ---- types and text ---------------------------------------------------------

#[test]
fn a_quantity_as_text_and_number() {
    assert_eq!(full("String(5kg) == '5kg'"), "true");
    assert_eq!(full("(6kg / 3kg).Float"), "2");
    assert!(fails("5kg.Float").contains("not a number"));
}

#[test]
fn math_that_needs_a_number_refuses_a_unit() {
    assert!(fails("sin(5m)").contains("no unit"));
    assert_eq!(full("abs(-5m)"), "5m");
}

// ---- columns -----------------------------------------------------------------

const PANTRY: &str = "\
sheet Pantry

Item   : String
Weight : Mass

| Item  | Weight  |
| ----- | ------- |
| flour | 5kg     |
| sugar | 300g    |
| salt  | 5       |
| yeast | 7.5(2)g |
";

/// An expression against the pantry, in full.
fn pantry(expr: &str) -> String {
    let options = Options {
        exact: true,
        ..Options::default()
    };
    let out = eval_with(Some(("test.omx", PANTRY)), "<expression>", expr, options);
    assert!(out.ok(), "`{expr}` failed:\n{}", out.errors.join("\n"));
    out.output.trim_end().to_string()
}

fn errors(file: &str) -> Vec<String> {
    lint("test.omx", file).errors
}

#[test]
fn a_cell_is_held_in_the_columns_unit() {
    let out = view("test.omx", PANTRY);
    assert!(out.ok(), "{:?}", out.errors);
    // 300g is 0.3kg, and a bare 5 is 5kg.
    for shown in ["5kg", "0.3kg", "7.50(20)e-3kg"] {
        assert!(out.output.contains(shown), "{shown} in\n{}", out.output);
    }
    assert!(!out.output.contains("300g"), "{}", out.output);
    assert_eq!(pantry("Pantry.Weight[1]"), "0.3kg");
    assert_eq!(pantry("Pantry.Weight[2]"), "5kg");
}

#[test]
fn a_cell_of_another_dimension_is_an_error() {
    let e = errors("sheet S\n\nWeight : Mass\n\n| Weight |\n| --- |\n| 3m |\n");
    assert!(
        e.iter()
            .any(|m| m.contains("not a `Mass`") && m.contains("Length")),
        "{e:?}"
    );
    // So is a formula that gives one, once it is worked out.
    let out = view(
        "test.omx",
        "sheet S\n\nWeight : Mass\n\n| Weight |\n| --- |\n| = 3m |\n",
    );
    assert!(!out.ok());
    let e = out.errors.join("\n");
    assert!(e.contains("Length") && e.contains("Mass"), "{e}");
    // A number from a formula is read in the column's unit, as a cell is.
    let out = view(
        "test.omx",
        "sheet S\n\nWeight : Mass\n\n| Weight |\n| --- |\n| = 2 + 3 |\n",
    );
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("5kg"), "{}", out.output);
}

#[test]
fn a_column_adds_up_and_orders() {
    let (v, s, u) = quantity_parts(&pantry("Pantry.Weight.sum()"));
    near(v, 10.3075);
    near(s, 0.0002);
    assert_eq!(u, "kg");
    let (v, _, u) = quantity_parts(&pantry("Pantry.Weight.mean()"));
    near(v, 10.3075 / 4.0);
    assert_eq!(u, "kg");
    assert_eq!(pantry("Pantry.Weight.max()"), "5kg");
    let (v, s, _) = quantity_parts(&pantry("Pantry.Weight.min()"));
    near(v, 0.0075);
    near(s, 0.0002);
    assert_eq!(pantry("Pantry.Weight.median()"), "2.65kg");
    let (v, _, u) = quantity_parts(&pantry("Pantry.Weight.sum().to('g')"));
    near(v, 10307.5);
    assert_eq!(u, "g");
}

#[test]
fn a_spread_of_quantities_is_not_taken() {
    for f in ["std", "var"] {
        let e = fails_in(PANTRY, &format!("Pantry.Weight.{f}()"));
        assert!(e.contains(f) && e.contains("Mass"), "{e}");
    }
}

fn fails_in(file: &str, expr: &str) -> String {
    let out = eval_with(
        Some(("test.omx", file)),
        "<expression>",
        expr,
        Options::default(),
    );
    assert!(!out.ok(), "`{expr}` should fail but gave {}", out.output);
    out.errors.join("\n")
}

#[test]
fn a_computed_column_of_a_named_type_is_held_in_its_unit() {
    let file = "\
sheet Boxes

Width  : Length
Height : Length
Area   : Area

| Width | Height | Area |
| ----- | ------ | ---- |
| 20cm  | 30cm   | *    |
| 2m    | 150cm  | *    |

Area := Width * Height
";
    let out = view("test.omx", file);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("0.06m^2"), "{}", out.output);
    assert!(out.output.contains("3m^2"), "{}", out.output);
}

#[test]
fn a_quantity_column_holds_what_it_is_given() {
    let file = "\
sheet S

Reading : Quantity

| Reading |
| ------- |
| 5kg     |
| 300cm   |
| 9.81m.s^-2 |
";
    let out = view("test.omx", file);
    assert!(out.ok(), "{:?}", out.errors);
    for shown in ["5kg", "300cm", "9.81m.s^-2"] {
        assert!(out.output.contains(shown), "{shown} in\n{}", out.output);
    }
}

#[test]
fn a_column_without_a_type_keeps_its_units() {
    let file = "\
sheet S

| Reading |
| ------- |
| 5kg     |
| 300g    |
";
    let out = view("test.omx", file);
    assert!(out.ok(), "{:?}", out.errors);
    assert!(out.output.contains("300g"), "{}", out.output);
}

#[test]
fn the_names_of_the_types() {
    for name in [
        "Quantity", "Length", "Mass", "Force", "Energy", "Torque", "Pressure",
    ] {
        let file = format!("sheet S\n\nX : {name}\n\n| X |\n| --- |\n| |\n");
        assert!(errors(&file).is_empty(), "{name}: {:?}", errors(&file));
    }
    // A number with no dimension is a number, and a typo is not a type.
    for name in ["Dimensionless", "Mess", "quantity"] {
        let file = format!("sheet S\n\nX : {name}\n\n| X |\n| --- |\n| 1 |\n");
        assert!(
            errors(&file).iter().any(|e| e.contains("unknown type")),
            "{name}: {:?}",
            errors(&file)
        );
    }
}
