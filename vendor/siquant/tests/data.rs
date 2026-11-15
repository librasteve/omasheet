// Copyright (c) 2026 Stephen Roe

//! Stage 2: data units. The bit `b` and the byte `B`, with the SI prefixes
//! (`kB` is 1000 B) and the binary ones (`KiB` is 1024 B).

use siquant::{Error, Quantity, Registry, Scope, Unit};

fn u(text: &str) -> Unit {
    Unit::parse(text, Scope::Live).unwrap()
}

fn q(value: f64, unit: &str) -> Quantity {
    Quantity::new(value, 0.0, u(unit)).unwrap()
}

fn to(value: f64, from: &str, into: &str) -> f64 {
    q(value, from).convert(&u(into)).unwrap().value()
}

#[test]
fn kilo_is_a_thousand_and_kibi_is_1024() {
    assert_eq!(to(1.0, "kB", "B"), 1000.0);
    assert_eq!(to(1.0, "KiB", "B"), 1024.0);
    assert_eq!(to(1.0, "MiB", "KiB"), 1024.0);
    assert_eq!(to(1.0, "MB", "kB"), 1000.0);
    assert_eq!(to(1.0, "GiB", "B"), 1073741824.0);
    assert_eq!(to(1.0, "TiB", "GiB"), 1024.0);
}

#[test]
fn a_byte_is_eight_bits() {
    assert_eq!(to(1.0, "B", "b"), 8.0);
    assert_eq!(to(16.0, "b", "B"), 2.0);
    assert_eq!(to(1.0, "kB", "kb"), 8.0);
    assert_eq!(to(1.0, "Mb", "kB"), 125.0);
    assert_eq!(to(1.0, "MiB", "Mib"), 8.0);
}

#[test]
fn data_adds_and_compares_across_prefixes() {
    let sum = q(4.0, "GiB").add(&q(512.0, "MiB")).unwrap();
    assert_eq!(sum.unit().to_string(), "GiB");
    assert_eq!(sum.value(), 4.5);
    assert!(q(1.0, "kB").convert(&u("KiB")).unwrap().value() < 1.0);
}

#[test]
fn rates_need_no_entry_of_their_own() {
    assert_eq!(u("MiB.s^-1").to_string(), "MiB.s^-1");
    assert_eq!(to(1.0, "MB.s^-1", "kb.s^-1"), 8000.0);
    assert_eq!(to(1.0, "Mb.s^-1", "MB.s^-1"), 0.125);
    // A rate times a time is an amount of data.
    let got = q(10.0, "MB.s^-1").mul(&q(3.0, "s")).unwrap();
    assert_eq!(got.unit().to_string(), "MB");
    assert_eq!(got.value(), 30.0);
}

#[test]
fn data_has_its_own_dimension() {
    let b = u("B");
    assert!(!b.is_dimensionless());
    assert_eq!(b.dims(), u("kb").dims());
    assert_ne!(b.dims(), u("g").dims());
    let err = q(1.0, "B").add(&q(1.0, "g")).unwrap_err();
    assert!(matches!(err, Error::DimensionMismatch { .. }));
}

#[test]
fn the_binary_prefixes_are_for_data_alone() {
    for bad in ["Kim", "KiB.", "Kig", "Mis", "kiB", "KIB", "Kib.s"] {
        let r = Unit::parse(bad, Scope::Live);
        // `Kib.s` is a bit-second, a legal compound; the rest are not units.
        if bad == "Kib.s" {
            assert!(r.is_ok(), "{bad}");
        } else {
            assert!(r.is_err(), "`{bad}` should not read");
        }
    }
}

#[test]
fn a_binary_scale_is_a_power_of_two() {
    let reg = Registry::builtin();
    assert_eq!(reg.scale("Ki."), Some((2, 10)));
    assert_eq!(reg.scale("Mi."), Some((2, 20)));
    assert_eq!(reg.scale("Gi."), Some((2, 30)));
    assert_eq!(reg.scale("k."), Some((10, 3)));
    let kibi = u("Ki.");
    assert!(kibi.is_scale());
    assert_eq!(kibi.factor(), 1024.0);
}

#[test]
fn the_data_types() {
    let reg = Registry::builtin();
    let info = reg.quantity_type("Data").unwrap();
    assert!(info.admits(&u("MiB")) && info.admits(&u("kb")));
    assert!(!info.admits(&u("m")));
    assert_eq!(info.preferred_unit().unwrap().to_string(), "B");
    assert_eq!(info.group(), "Data");
    let rate = reg.quantity_type("DataRate").unwrap();
    assert!(rate.admits(&u("MiB.s^-1")));
    assert_eq!(rate.preferred_unit().unwrap().to_string(), "b.s^-1");
}

#[test]
fn information_density_is_data_over_area() {
    let reg = Registry::builtin();
    let t = reg.quantity_type("DataDensity").unwrap();
    assert_eq!(t.preferred_unit().unwrap().to_string(), "b.m^-2");
    assert_eq!(t.group(), "Data");
    assert!(t.admits(&u("Gb.m^-2")) && t.admits(&u("TiB.m^-2")));
    assert!(!t.admits(&u("B.s^-1")));
    // A disk: 1 TB on 1 cm^2 is 1e12 B over 1e-4 m^2.
    let per_area = q(1.0, "TB").div(&q(1.0, "cm^2")).unwrap();
    let got = per_area.convert(&u("b.m^-2")).unwrap().value();
    assert!(((got - 8e16) / 8e16).abs() < 1e-12, "{got}");
    assert_eq!(per_area.unit().dims(), t.dims());
}
