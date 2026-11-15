// Copyright (c) 2026 Stephen Roe

//! What cell text may say, and what it may not: the unit notation is strict.

use siquant::{Error, Scope, Unit};

fn live(text: &str) -> Result<Unit, Error> {
    Unit::parse(text, Scope::Live)
}

fn unit(text: &str) -> Unit {
    live(text).unwrap_or_else(|e| panic!("`{text}`: {e}"))
}

#[test]
fn accepts_si_units_in_the_one_notation() {
    for text in [
        "kg",
        "g",
        "cm",
        "km",
        "m^2",
        "m^-1",
        "m^1",
        "m.s^-2",
        "kg.m^2.s^-2",
        "A.s",
        "kPa",
        "MHz",
        "daN",
        "Ym",
        "qg",
        "Ω",
        "kΩ",
        "μm",
        "mol.s^-1",
        "cm^2",
        "°C",
        "kat",
        "mT",
    ] {
        assert!(
            live(text).is_ok(),
            "should accept `{text}`: {:?}",
            live(text)
        );
    }
}

#[test]
fn accepts_typographic_alternates() {
    // U+2126 OHM SIGN, U+00B5 MICRO SIGN and U+00BA MASCULINE ORDINAL.
    assert_eq!(live("\u{2126}").unwrap(), live("Ω").unwrap());
    assert_eq!(live("\u{b5}m").unwrap(), live("μm").unwrap());
    assert_eq!(live("\u{ba}C").unwrap(), live("°C").unwrap());
    // They print as the canonical symbol.
    assert_eq!(live("\u{b5}m").unwrap().to_string(), "μm");
    assert_eq!(live("\u{ba}C").unwrap().to_string(), "°C");
}

#[test]
fn rejects_everything_else() {
    let cases: &[(&str, &str)] = &[
        ("", "empty"),
        (" kg", "leading space"),
        ("kg ", "trailing space"),
        ("k g", "space inside"),
        ("m s", "space as multiply"),
        ("m s^-1", "space as multiply"),
        ("km/s", "slash"),
        ("m^2.s^-1/kg", "slash"),
        ("m^", "no exponent"),
        ("m^x", "bad exponent"),
        ("m^+2", "plus in exponent"),
        ("m^0", "zero exponent"),
        ("m^2^3", "two exponents"),
        ("m^2.5", "fractional exponent"),
        ("^2", "no symbol"),
        ("m2", "trailing-digit exponent"),
        ("s-1", "trailing-digit exponent"),
        ("m..s", "empty term"),
        (".m", "leading dot"),
        ("kg.", "trailing dot after a unit"),
        ("s.", "trailing dot after a unit"),
        ("N.", "trailing dot after a unit"),
        ("k..", "two dots"),
        (".", "a dot alone"),
        ("k.m", "a scale in a compound"),
        ("m.k.", "a scale in a compound"),
        ("k.^2", "an exponent on a scale"),
        ("k^2.", "an exponent on a scale"),
        ("x.", "not a prefix"),
        ("m*s", "star"),
        ("m\u{b7}s", "middle dot"),
        ("m\u{22c5}s", "dot operator"),
        ("(m)", "brackets"),
        ("m\u{b2}", "superscript"),
        ("ft", "non-SI, unknown"),
        ("L", "litre"),
        ("min", "minute"),
        ("kilogram", "a name"),
        ("metres", "a plural"),
        ("Kg", "wrong case"),
        ("KG", "wrong case"),
        ("Mm.", "trailing dot"),
        ("k", "a prefix alone"),
        ("da", "a prefix alone"),
        ("kk", "two prefixes"),
        ("kkg", "two prefixes"),
        ("1", "a bare number"),
    ];
    for (text, why) in cases {
        assert!(live(text).is_err(), "should reject `{text}` ({why})");
    }
}

#[test]
fn a_prefix_and_a_dot_is_a_scale() {
    for (text, factor) in [
        ("k.", 1e3),
        ("M.", 1e6),
        ("G.", 1e9),
        ("T.", 1e12),
        ("Q.", 1e30),
        ("h.", 1e2),
        ("da.", 10.0),
        ("d.", 1e-1),
        ("c.", 1e-2),
        ("m.", 1e-3),
        ("μ.", 1e-6),
        ("\u{b5}.", 1e-6),
        ("n.", 1e-9),
        ("q.", 1e-30),
    ] {
        let u = unit(text);
        assert!(u.is_scale() && u.is_dimensionless(), "{text}");
        assert!(close(u.factor(), factor), "{text}: {}", u.factor());
        assert_eq!(u.offset(), 0.0);
        // It prints as the canonical symbol and a dot, and parses back.
        let shown = u.to_string();
        assert_eq!(unit(&shown), u, "{text} -> {shown}");
    }
    assert_eq!(unit("\u{b5}.").to_string(), "μ.");
    assert_eq!(unit("k.").to_string(), "k.");
}

#[test]
fn the_dot_makes_it_a_prefix_so_m_and_t_do_not_clash() {
    // `m` and `T` are units; `m.` and `T.` are scales.
    assert!(!unit("m").is_scale());
    assert!(!unit("T").is_scale());
    assert!(unit("m.").is_scale());
    assert!(unit("T.").is_scale());
    assert_eq!(unit("m").dims(), siquant::Dim::of(siquant::Base::Length));
    assert_eq!(unit("m.").dims(), siquant::Dim::NONE);
    assert!(close(unit("T.").factor(), 1e12));
}

#[test]
fn a_scale_is_not_part_of_a_compound() {
    let k = unit("k.");
    let m = unit("m");
    assert!(matches!(k.mul(&m), Err(Error::ScaleInCompound(s)) if s == "k."));
    assert!(matches!(m.div(&k), Err(Error::ScaleInCompound(_))));
    assert!(matches!(k.pow(2), Err(Error::ScaleInCompound(_))));
    assert!(matches!(
        unit("k.").invert(),
        Err(Error::ScaleInCompound(_))
    ));
    // But a scale times a plain number is fine, and so is its own power one.
    assert!(k.pow(1).is_ok());
    assert!(k.mul(&Unit::one(k.registry())).is_ok());
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

#[test]
fn rejects_affine_units_inside_a_compound() {
    for text in ["°C.s^-1", "m.°C", "°C^2", "°C^-1", "°C.°C"] {
        assert!(
            matches!(live(text), Err(Error::AffineInCompound(_))),
            "`{text}`: {:?}",
            live(text)
        );
    }
    // And a prefix is not a unit of the affine unit.
    assert!(matches!(live("k°C"), Err(Error::UnknownUnit(_))));
}

#[test]
fn errors_say_where_and_why() {
    assert!(matches!(live("km/s"), Err(Error::Syntax { at: 2, .. })));
    assert!(matches!(live("m.s^"), Err(Error::Syntax { at: 4, .. })));
    assert!(matches!(live("m..s"), Err(Error::Syntax { at: 2, .. })));
    assert!(matches!(live("furlong"), Err(Error::UnknownUnit(s)) if s == "furlong"));
    assert!(matches!(live("m^99999"), Err(Error::Overflow)));
}

#[test]
fn printing_is_canonical_and_parses_back() {
    for (written, canonical) in [
        ("s^-2.m.kg", "kg.m.s^-2"),
        ("kg.m^2.s^-2", "kg.m^2.s^-2"),
        ("m.m", "m^2"),
        ("m^2.m^-1", "m"),
        ("m^1", "m"),
        ("s^-1.m", "m.s^-1"),
        ("A.s", "A.s"),
        ("kg.m^-3", "kg.m^-3"),
        ("cm^2", "cm^2"),
        // Two terms of one unit with different prefixes stay apart; no
        // prefix sorts before a prefix.
        ("km.m", "m.km"),
        // Positive exponents first; derived units before base units.
        ("s^-1.m", "m.s^-1"),
        ("m^-3.kg", "kg.m^-3"),
        ("m.N", "N.m"),
        ("s.N", "N.s"),
        ("K^-1.J", "J.K^-1"),
        ("s.K.mol", "K.mol.s"),
        ("A^-1.m^2.kg.s^-3", "kg.m^2.A^-1.s^-3"),
    ] {
        let unit = live(written).unwrap();
        assert_eq!(unit.to_string(), canonical, "printing `{written}`");
        assert_eq!(live(canonical).unwrap(), unit, "parsing `{canonical}`");
    }
}

#[test]
fn cancelling_units_leave_no_terms() {
    let u = live("m.m^-1").unwrap();
    assert!(u.is_dimensionless());
    assert_eq!(u.to_string(), "1");
}

#[test]
fn scope_live_refuses_a_column_only_unit() {
    let prefixes = "- {symbol: k, name: kilo, base: 10, exp: 3}";
    let units = "\
- {symbol: m, name: metre, base: Length, prefixable: si}
- {symbol: ft, name: foot, defn: m, factor: \"0.3048\", live: false}
";
    let reg = siquant::Registry::from_yaml(prefixes, units, "[]").unwrap();
    assert!(matches!(
        reg.parse_unit("ft", Scope::Live),
        Err(Error::NotLive(s)) if s == "ft"
    ));
    let ft = reg.parse_unit("ft", Scope::Any).unwrap();
    let m = reg.parse_unit("m", Scope::Live).unwrap();
    assert!((ft.convert_value(1.0, &m).unwrap() - 0.3048).abs() < 1e-15);
    // Compounds work in the wider scope too.
    assert!(reg.parse_unit("ft.s^-1", Scope::Any).is_err()); // no `s` here
    assert_eq!(
        reg.parse_unit("ft^2", Scope::Any).unwrap().to_string(),
        "ft^2"
    );
}
