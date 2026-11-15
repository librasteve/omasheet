// Copyright (c) 2026 Stephen Roe

//! Stage 3: angles. The radian, the steradian, the degree and its minutes
//! and seconds, the lumen and the lux, and the `45°30′30″` entry form.

use siquant::{Error, Quantity, Registry, Scope, Unit};

fn u(text: &str) -> Unit {
    Unit::parse(text, Scope::Live).unwrap()
}

fn q(value: f64, unit: &str) -> Quantity {
    Quantity::exact(value, u(unit))
}

fn to(value: f64, from: &str, into: &str) -> f64 {
    q(value, from).convert(&u(into)).unwrap().value()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * b.abs().max(1.0)
}

#[test]
fn a_half_turn_is_pi_radians_and_180_degrees() {
    assert!(near(to(180.0, "°", "rad"), std::f64::consts::PI));
    assert!(near(to(1.0, "rad", "°"), 180.0 / std::f64::consts::PI));
    assert!(near(to(1.0, "°", "′"), 60.0));
    assert!(near(to(1.0, "′", "″"), 60.0));
    assert!(near(to(1.0, "°", "″"), 3600.0));
    assert!(near(to(1000.0, "mrad", "rad"), 1.0));
}

#[test]
fn the_alternates_are_the_same_units() {
    assert_eq!(u("º"), u("°"));
    assert_eq!(u("'"), u("′"));
    assert_eq!(u("\""), u("″"));
}

#[test]
fn a_degree_is_not_a_degree_celsius() {
    assert_eq!(u("°").to_string(), "°");
    assert_eq!(u("°C").to_string(), "°C");
    assert!(Unit::parse("°C", Scope::Live).unwrap().is_affine());
    assert!(!u("°").is_affine());
    assert!(Unit::parse("k°", Scope::Live).is_err());
}

#[test]
fn an_angle_is_a_dimension_of_its_own() {
    // A frequency and an angular speed differ, and a solid angle is an angle
    // squared.
    assert_ne!(u("Hz").dims(), u("rad.s^-1").dims());
    assert_eq!(u("sr").dims(), u("rad^2").dims());
    assert!(u("rad").convert_value(1.0, &u("Hz")).is_err());
    assert_eq!(u("lm").dims(), u("cd.sr").dims());
    assert_eq!(u("lx").dims(), u("lm.m^-2").dims());
    assert!(near(to(1.0, "lx", "lm.m^-2"), 1.0));
}

#[test]
fn the_angle_types_name_their_dimensions() {
    let r = Registry::builtin();
    for (name, unit) in [
        ("Angle", "rad"),
        ("SolidAngle", "sr"),
        ("AngularSpeed", "rad.s^-1"),
        ("LuminousFlux", "lm"),
        ("Illuminance", "lx"),
    ] {
        let t = r.quantity_type(name).unwrap_or_else(|| panic!("{name}"));
        assert!(t.admits(&u(unit)), "{name} admits {unit}");
        assert!(t.admits(&u(unit)), "{name}");
    }
    let angle = r.quantity_type("Angle").unwrap();
    assert!(angle.admits(&u("°")) && angle.admits(&u("″")));
    assert!(!angle.admits(&u("m")));
}

#[test]
fn degrees_minutes_and_seconds_are_read() {
    let a = Quantity::from_dms("45°30′30″").unwrap();
    assert_eq!(a.unit(), &u("°"));
    assert!(near(a.value(), 45.0 + 30.0 / 60.0 + 30.0 / 3600.0));
    for same in ["45°30′30″", "45º30'30\"", "45°30'30''", "+45°30′30″"] {
        assert!(
            near(Quantity::from_dms(same).unwrap().value(), a.value()),
            "{same}"
        );
    }
    assert!(near(Quantity::from_dms("-12°30′").unwrap().value(), -12.5));
    assert!(near(
        Quantity::from_dms("0°0′1.5″").unwrap().value(),
        1.5 / 3600.0
    ));
    assert!(near(
        Quantity::from_dms("45°30.5′").unwrap().value(),
        45.0 + 30.5 / 60.0
    ));
    assert_eq!(Quantity::from_dms("90°").unwrap().value(), 90.0);
}

#[test]
fn a_badly_written_angle_is_refused() {
    for text in [
        "",
        "45",
        "°",
        "30′",
        "45°60′",
        "45°30′60″",
        "45.5°30′",
        "45°30′30",
        "45°30″30′",
        "45°45°",
        "45 °",
        "-",
        "4 5°",
        "45°30′30″5",
        "45.°",
    ] {
        assert!(
            matches!(Quantity::from_dms(text), Err(Error::Syntax { .. })),
            "`{text}` should be refused"
        );
    }
}

#[test]
fn an_angle_is_written_as_degrees_minutes_seconds() {
    assert_eq!(
        q(45.508_333_333_333_33, "°").to_dms(0).unwrap(),
        "45°30′30″"
    );
    assert_eq!(q(-12.5, "°").to_dms(0).unwrap(), "-12°30′0″");
    assert_eq!(q(1.0, "″").to_dms(2).unwrap(), "0°0′1.00″");
    assert_eq!(
        q(std::f64::consts::PI, "rad").to_dms(1).unwrap(),
        "180°0′0.0″"
    );
    // Rounding carries: 59.999″ at two places is a whole minute.
    assert_eq!(q(0.016666666, "°").to_dms(2).unwrap(), "0°1′0.00″");
    assert_eq!(q(-0.0, "°").to_dms(0).unwrap(), "0°0′0″");
    assert!(matches!(
        q(1.0, "m").to_dms(0),
        Err(Error::DimensionMismatch { .. })
    ));
}

#[test]
fn written_and_read_they_round_trip() {
    for text in ["45°30′30″", "0°0′1.5″", "-12°30′0″", "179°59′59.999″"] {
        let a = Quantity::from_dms(text).unwrap();
        let places = text
            .split_once('.')
            .map_or(0, |(_, f)| f.len() - '″'.len_utf8());
        let back = a.to_dms(places).unwrap();
        // Written whole: `-12°30′` is read as `-12°30′0″`.
        assert_eq!(back, text);
    }
}

#[test]
fn angles_add_and_compare_across_units() {
    let sum = q(90.0, "°")
        .add(&q(std::f64::consts::FRAC_PI_2, "rad"))
        .unwrap();
    assert!(near(sum.value(), 180.0));
    assert_eq!(sum.unit(), &u("°"));
    // An angle with an uncertainty keeps it through the conversion.
    let a = Quantity::new(30.0, 0.5, u("′"))
        .unwrap()
        .convert(&u("°"))
        .unwrap();
    assert!(near(a.value(), 0.5) && near(a.sigma(), 0.5 / 60.0));
}

#[test]
fn a_quantity_in_its_own_unit_is_not_rounded() {
    // 16 digits are kept when the unit does not change.
    let a = Quantity::from_dms("45°30′30″").unwrap();
    assert_eq!(a.convert(&u("°")).unwrap().value(), a.value());
    assert_eq!(a.sub(&a).unwrap().value(), 0.0);
}

#[test]
fn a_conversion_keeps_the_digits_it_has() {
    // 90° is 1.5707963267948966 rad: rounded to 15 digits it would be off by 3e-15.
    let half_pi = q(90.0, "°").convert(&u("rad")).unwrap().value();
    assert_eq!(half_pi, std::f64::consts::FRAC_PI_2);
    let back = q(half_pi, "rad").convert(&u("°")).unwrap().value();
    assert!((back - 90.0).abs() < 1e-13, "{back}");
    // And the noise of a sum is still cleaned up.
    assert_eq!(to_value(2.0, "g", "mg"), 2000.0);
}

fn to_value(value: f64, from: &str, into: &str) -> f64 {
    q(value, from).convert(&u(into)).unwrap().value()
}
