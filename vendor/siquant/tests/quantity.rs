// Copyright (c) 2026 Stephen Roe

//! Conversion and arithmetic on quantities, with their uncertainty.

use siquant::{Error, Quantity, Scope, Unit};

fn u(text: &str) -> Unit {
    Unit::parse(text, Scope::Live).unwrap()
}

fn q(value: f64, sigma: f64, unit: &str) -> Quantity {
    Quantity::new(value, sigma, u(unit)).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

fn assert_q(got: &Quantity, value: f64, sigma: f64, unit: &str) {
    assert_eq!(got.unit().to_string(), unit, "unit of {got:?}");
    assert!(
        close(got.value(), value),
        "value {} vs {value}",
        got.value()
    );
    assert!(
        (got.sigma() - sigma).abs() <= 1e-12 * sigma.abs().max(1e-300) + 1e-300,
        "sigma {} vs {sigma}",
        got.sigma()
    );
}

#[test]
fn conversions() {
    for (value, from, to, want) in [
        (5.0, "km", "m", 5000.0),
        (300.0, "cm", "m", 3.0),
        (1.0, "kg", "g", 1000.0),
        (1.0, "g", "kg", 0.001),
        (250.0, "mg", "kg", 2.5e-4),
        (1.0, "mm", "μm", 1000.0),
        (1.0, "MJ", "J", 1e6),
        (1.0, "kW", "W", 1000.0),
        (27.0, "kg.m.s^-2", "N", 27.0),
        (1.0, "N.m", "J", 1.0),
        (1.0, "Pa", "N.m^-2", 1.0),
        (1.0, "kPa", "N.m^-2", 1000.0),
        (1.0, "J", "kg.m^2.s^-2", 1.0),
        (1.0, "km.s^-1", "m.s^-1", 1000.0),
        (1.0, "cm^2", "m^2", 1e-4),
        (1.0, "km^2", "m^2", 1e6),
        (2.5, "kg.m^-3", "g.cm^-3", 0.0025),
        (1.0, "Gy", "J.kg^-1", 1.0),
        (1.0, "kat", "mol.s^-1", 1.0),
        (1.0, "Mm", "km", 1000.0),
        (1.0, "qm", "ym", 1e-6),
    ] {
        let got = q(value, 0.0, from).convert(&u(to)).unwrap();
        assert!(
            close(got.value(), want),
            "{value} {from} -> {to}: {} vs {want}",
            got.value()
        );
        // And back again.
        let back = got.convert(&u(from)).unwrap();
        assert!(close(back.value(), value), "{from} <-> {to}");
    }
}

#[test]
fn celsius_and_kelvin() {
    // The Raku goldens: 39°C is 312.15K.
    assert_q(
        &q(39.0, 0.0, "°C").convert(&u("K")).unwrap(),
        312.15,
        0.0,
        "K",
    );
    assert_q(
        &q(312.15, 0.0, "K").convert(&u("°C")).unwrap(),
        39.0,
        0.0,
        "°C",
    );
    assert_q(
        &q(0.0, 0.0, "°C").convert(&u("K")).unwrap(),
        273.15,
        0.0,
        "K",
    );
    assert_q(
        &q(100.0, 0.0, "°C").convert(&u("K")).unwrap(),
        373.15,
        0.0,
        "K",
    );
    assert_q(
        &q(0.0, 0.0, "°C").convert(&u("mK")).unwrap(),
        273_150.0,
        0.0,
        "mK",
    );
    assert_q(
        &q(-273.15, 0.0, "°C").convert(&u("K")).unwrap(),
        0.0,
        0.0,
        "K",
    );
}

#[test]
fn offset_applies_to_the_value_and_not_the_uncertainty() {
    assert_q(
        &q(20.0, 0.5, "°C").convert(&u("K")).unwrap(),
        293.15,
        0.5,
        "K",
    );
    assert_q(
        &q(293.15, 0.5, "K").convert(&u("°C")).unwrap(),
        20.0,
        0.5,
        "°C",
    );
}

#[test]
fn convert_scales_the_uncertainty_with_the_unit() {
    assert_q(
        &q(2.0, 0.1, "km").convert(&u("m")).unwrap(),
        2000.0,
        100.0,
        "m",
    );
    assert_q(
        &q(5.0, 0.2, "kg.m^-3").convert(&u("g.cm^-3")).unwrap(),
        0.005,
        0.0002,
        "g.cm^-3",
    );
}

#[test]
fn five_metres_plus_three_hundred_centimetres_is_eight_metres() {
    let sum = q(5.0, 0.0, "m").add(&q(300.0, 0.0, "cm")).unwrap();
    assert_eq!(sum.value(), 8.0);
    assert_eq!(sum.unit().to_string(), "m");
    // The left unit wins.
    let sum = q(300.0, 0.0, "cm").add(&q(5.0, 0.0, "m")).unwrap();
    assert_q(&sum, 800.0, 0.0, "cm");
}

#[test]
fn a_sum_combines_uncertainties_in_quadrature() {
    let sum = q(3.0, 0.4, "m").add(&q(4.0, 0.3, "m")).unwrap();
    assert_q(&sum, 7.0, 0.5, "m");
    let diff = q(3.0, 0.4, "m").sub(&q(4.0, 0.3, "m")).unwrap();
    assert_q(&diff, -1.0, 0.5, "m");
    // The other side's uncertainty is converted too: 0.3 cm is 0.003 m.
    let mixed = q(1.0, 0.0, "m").add(&q(50.0, 0.3, "cm")).unwrap();
    assert!(close(mixed.sigma(), 0.003));
}

#[test]
fn a_product_keeps_both_units_and_propagates_the_spread() {
    let p = q(2.0, 0.1, "m").mul(&q(3.0, 0.3, "s")).unwrap();
    assert_q(&p, 6.0, 0.3f64.hypot(0.2 * 3.0), "m.s");
    // No rebasing: kilometres stay kilometres.
    let p = q(5.0, 0.0, "km").mul(&q(2.0, 0.0, "s")).unwrap();
    assert_q(&p, 10.0, 0.0, "km.s");
}

#[test]
fn a_quotient_and_the_result_it_leaves() {
    let v = q(10.0, 0.2, "m").div(&q(4.0, 0.1, "s")).unwrap();
    let want = (0.2f64 / 4.0).hypot(10.0 * 0.1 / 16.0);
    assert_q(&v, 2.5, want, "m.s^-1");
    // A unit cancelling itself leaves a pure number.
    let ratio = q(6.0, 0.0, "m").div(&q(3.0, 0.0, "m")).unwrap();
    assert!(ratio.unit().is_dimensionless());
    assert_eq!(ratio.value(), 2.0);
    assert!(matches!(
        q(1.0, 0.0, "m").div(&q(0.0, 0.0, "s")),
        Err(Error::DivideByZero)
    ));
}

#[test]
fn powers_and_roots() {
    let cube = q(2.0, 0.1, "m").pow(3).unwrap();
    assert_q(&cube, 8.0, 3.0 * 4.0 * 0.1, "m^3");
    let root = q(4.0, 0.2, "m^2").root(2).unwrap();
    assert_q(&root, 2.0, 0.5 * 0.25 * 0.2 * 2.0, "m");
    let inverse = q(4.0, 0.0, "s").pow(-1).unwrap();
    assert_q(&inverse, 0.25, 0.0, "s^-1");
    // m has no square root as a unit.
    assert!(matches!(q(4.0, 0.0, "m").root(2), Err(Error::BadRoot(_))));
    assert!(matches!(
        q(-4.0, 0.0, "m^2").root(2),
        Err(Error::BadRoot(_))
    ));
    // But an odd root of a negative value is fine.
    assert_q(&q(-8.0, 0.0, "m^3").root(3).unwrap(), -2.0, 0.0, "m");
}

#[test]
fn a_difference_of_celsius_temperatures_is_in_celsius() {
    // An interval is as big in `°C` as in K, and keeps the left's unit.
    let d = q(20.0, 0.3, "°C").sub(&q(10.0, 0.4, "°C")).unwrap();
    assert_q(&d, 10.0, 0.5, "°C");
    assert_q(
        &q(10.0, 0.0, "°C").sub(&q(5.0, 0.0, "°C")).unwrap(),
        5.0,
        0.0,
        "°C",
    );
    // A Celsius value moved by a kelvin step stays Celsius.
    let up = q(20.0, 0.0, "°C").add(&q(5.0, 0.0, "K")).unwrap();
    assert_q(&up, 25.0, 0.0, "°C");
    let down = q(20.0, 0.0, "°C").sub(&q(5.0, 0.0, "K")).unwrap();
    assert_q(&down, 15.0, 0.0, "°C");
    // A product goes through kelvin.
    let p = q(20.0, 0.0, "°C").mul(&q(2.0, 0.0, "m")).unwrap();
    assert_q(&p, 586.3, 0.0, "m.K");
}

#[test]
fn rebase_leaves_other_units_alone() {
    assert_q(&q(20.0, 0.5, "°C").rebase().unwrap(), 293.15, 0.5, "K");
    assert_q(&q(20.0, 0.5, "km").rebase().unwrap(), 20.0, 0.5, "km");
}

#[test]
fn different_dimensions_do_not_mix() {
    let e = q(5.0, 0.0, "m").add(&q(3.0, 0.0, "s")).unwrap_err();
    assert!(matches!(e, Error::DimensionMismatch { .. }), "{e}");
    assert!(e.to_string().contains("L") && e.to_string().contains("T"));
    assert!(q(5.0, 0.0, "m").convert(&u("kg")).is_err());
    assert!(q(1.0, 0.0, "J").convert(&u("W")).is_err());
}

#[test]
fn an_uncertainty_is_not_negative() {
    assert!(matches!(
        Quantity::new(1.0, -0.1, u("m")),
        Err(Error::NegativeSigma)
    ));
    assert!(matches!(
        Quantity::new(1.0, f64::NAN, u("m")),
        Err(Error::NegativeSigma)
    ));
    assert!(Quantity::new(1.0, 0.0, u("m")).is_ok());
}

#[test]
fn a_quantity_is_its_types_dimension() {
    let reg = siquant::Registry::builtin();
    let length = reg.quantity_type("Length").unwrap();
    let area = reg.quantity_type("Area").unwrap();
    let side = q(3.0, 0.1, "cm");
    assert!(length.admits_quantity(&side));
    assert!(!area.admits_quantity(&side));
    assert!(area.admits_quantity(&side.mul(&side).unwrap()));
}

fn one() -> Unit {
    Unit::one(siquant::Registry::builtin())
}

#[test]
fn a_scale_is_a_plain_number() {
    // 1k. == 1000, and the uncertainty is scaled with it.
    assert_eq!(q(1.0, 0.0, "k.").to_number().unwrap(), (1000.0, 0.0));
    let (v, s) = q(2.0, 0.1, "k.").to_number().unwrap();
    assert!(close(v, 2000.0) && close(s, 100.0), "{v} {s}");
    let (v, _) = q(5.0, 0.0, "m.").to_number().unwrap();
    assert!(close(v, 0.005));
    let (v, _) = q(1.0, 0.0, "T.").to_number().unwrap();
    assert!(close(v, 1e12));
    let (v, _) = q(1.0, 0.0, "da.").to_number().unwrap();
    assert!(close(v, 10.0));
    // A plain number and a quantity with no unit terms are the same thing.
    assert_eq!(Quantity::exact(7.0, one()).to_number().unwrap(), (7.0, 0.0));
    let ratio = q(6.0, 0.0, "m").div(&q(3.0, 0.0, "m")).unwrap();
    assert_eq!(ratio.to_number().unwrap(), (2.0, 0.0));
}

#[test]
fn only_a_dimensionless_quantity_is_a_number() {
    let e = q(1.0, 0.0, "m").to_number().unwrap_err();
    assert!(matches!(e, Error::DimensionMismatch { .. }), "{e}");
    assert!(q(1.0, 0.0, "kg").to_number().is_err());
}

#[test]
fn a_scale_adds_to_a_number_and_to_another_scale() {
    // 5k. + 300 is 5.3k.; the left unit wins.
    let sum = q(5.0, 0.0, "k.")
        .add(&Quantity::exact(300.0, one()))
        .unwrap();
    assert_q(&sum, 5.3, 0.0, "k.");
    assert!(close(sum.to_number().unwrap().0, 5300.0));
    // 1M. + 500k. is 1.5M.
    let sum = q(1.0, 0.0, "M.").add(&q(500.0, 0.0, "k.")).unwrap();
    assert_q(&sum, 1.5, 0.0, "M.");
    // A scale is not a length.
    assert!(matches!(
        q(1.0, 0.0, "k.").add(&q(1.0, 0.0, "m")),
        Err(Error::DimensionMismatch { .. })
    ));
}

#[test]
fn a_scale_multiplies_into_the_value() {
    // 2k. * 3m is 6000m; the scale does not stay in the unit.
    let p = q(2.0, 0.1, "k.").mul(&q(3.0, 0.0, "m")).unwrap();
    assert_q(&p, 6000.0, 300.0, "m");
    // 3m / 2k. is 0.0015m.
    let d = q(3.0, 0.0, "m").div(&q(2.0, 0.0, "k.")).unwrap();
    assert_q(&d, 0.0015, 0.0, "m");
    // (2k.)^2 is 4e6, a plain number.
    let sq = q(2.0, 0.0, "k.").pow(2).unwrap();
    assert!(sq.unit().is_dimensionless() && !sq.unit().is_scale());
    assert!(close(sq.value(), 4e6));
    // The square root of 4M. is 2000.
    let r = q(4.0, 0.0, "M.").root(2).unwrap();
    assert!(close(r.value(), 2000.0));
}

#[test]
fn a_scale_converts_to_other_scales_and_to_one() {
    let n = q(1.0, 0.1, "M.").convert(&u("k.")).unwrap();
    assert_q(&n, 1000.0, 100.0, "k.");
    let plain = q(3.0, 0.0, "k.").convert(&one()).unwrap();
    assert_eq!(plain.value(), 3000.0);
    assert!(plain.unit().is_dimensionless());
    assert!(q(1.0, 0.0, "k.").convert(&u("m")).is_err());
}

#[test]
fn the_dimensionless_type_admits_scales_and_cancelled_units() {
    let reg = siquant::Registry::builtin();
    let none = reg.quantity_type("Dimensionless").unwrap();
    assert!(none.admits(&u("k.")));
    assert!(none.admits(&u("m.m^-1")));
    assert!(none.admits(&one()));
    assert!(!none.admits(&u("m")));
}

#[test]
fn the_unity_unit_is_not_listed() {
    let reg = siquant::Registry::builtin();
    assert!(reg.units().all(|u| !u.symbol.is_empty()));
}

#[test]
fn a_conversion_has_no_rounding_noise() {
    assert_q(
        &q(2.0, 0.0, "g").convert(&u("mg")).unwrap(),
        2000.0,
        0.0,
        "mg",
    );
    assert_q(
        &q(5.0, 0.1, "kg").convert(&u("g")).unwrap(),
        5000.0,
        100.0,
        "g",
    );
    assert_q(
        &q(0.3, 0.0, "m").convert(&u("cm")).unwrap(),
        30.0,
        0.0,
        "cm",
    );
}
