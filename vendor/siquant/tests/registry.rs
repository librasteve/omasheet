// Copyright (c) 2026 Stephen Roe

//! The registry: it loads, nothing in it is ambiguous, every prefix x unit
//! reads back as itself, and the named quantity types are right.

use siquant::{Base, Dim, Error, PrefixKind, Prefixable, Registry, Scope, Unit};

fn live(text: &str) -> Unit {
    Unit::parse(text, Scope::Live).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
}

#[test]
fn the_builtin_registry_loads_and_is_unambiguous() {
    let reg = Registry::builtin();
    assert_eq!(reg.prefixes().count(), 34);
    assert!(reg.units().count() >= 25);
    assert!(reg.units().all(|u| u.live));
    // The bit and the byte are there, with the data prefixes.
    assert!(reg.units().any(|u| u.symbol == "B"));
}

#[test]
fn every_prefix_and_unit_reads_back_as_itself() {
    let reg = Registry::builtin();
    let mut checked = 0;
    for u in reg.units().filter(|u| u.prefixable != Prefixable::None) {
        for p in reg.prefixes() {
            let kind = if p.base == 10 {
                PrefixKind::Si
            } else {
                PrefixKind::Binary
            };
            let allowed = matches!(
                (u.prefixable, kind),
                (Prefixable::Both, _)
                    | (Prefixable::Si, PrefixKind::Si)
                    | (Prefixable::Binary, PrefixKind::Binary)
            );
            if !allowed {
                continue;
            }
            let text = format!("{}{}", p.symbol, u.symbol);
            let unit = Unit::parse(&text, Scope::Live)
                .unwrap_or_else(|e| panic!("`{text}` should parse: {e}"));
            assert_eq!(unit.to_string(), text, "`{text}` prints as itself");
            assert_eq!(unit.dims(), u.dims, "`{text}` keeps the dimension");
            // One of it, in the bare unit, is base^exp of the bare unit (10^exp
            // for an SI prefix, 2^exp for a binary one).
            let bare = Unit::parse(u.symbol, Scope::Live).unwrap();
            let ratio = unit.convert_value(1.0, &bare).unwrap();
            let want: f64 = if p.base == 2 {
                2f64.powi(i32::from(p.exp))
            } else {
                format!("1e{}", p.exp).parse().unwrap()
            };
            assert!(
                close(ratio, want),
                "`{text}` -> `{}`: {ratio} vs {want}",
                u.symbol
            );
            checked += 1;
        }
    }
    // 24 SI prefixes on each SI unit, and the binary ones too on the data units.
    assert!(
        checked >= 24 * 24 + 34 * 2,
        "only {checked} combinations checked"
    );
}

#[test]
fn a_bare_unit_symbol_beats_a_prefix_reading() {
    // `T` is the tesla, not the prefix; `cd` is the candela, not centi-d.
    assert_eq!(live("T").to_string(), "T");
    assert_eq!(live("cd").to_string(), "cd");
    assert_eq!(live("mT").to_string(), "mT");
    assert_eq!(live("Pa").to_string(), "Pa");
}

#[test]
fn the_kilogram_is_a_prefixed_gram() {
    let kg = live("kg");
    assert_eq!(kg.terms().len(), 1);
    assert!(close(kg.factor(), 1.0));
    assert!(close(live("g").factor(), 1e-3));
    assert!(close(live("mg").factor(), 1e-6));
}

#[test]
fn derived_units_have_the_right_dimensions() {
    let l = Dim::of(Base::Length);
    let m = Dim::of(Base::Mass);
    let t = Dim::of(Base::Time);
    let i = Dim::of(Base::Current);
    let dim = |parts: &[(Dim, i16)]| {
        parts.iter().fold(Dim::NONE, |acc, (d, k)| {
            acc.checked_add(d.checked_scale(*k).unwrap()).unwrap()
        })
    };
    let cases: &[(&str, Dim)] = &[
        ("Hz", dim(&[(t, -1)])),
        ("N", dim(&[(m, 1), (l, 1), (t, -2)])),
        ("Pa", dim(&[(m, 1), (l, -1), (t, -2)])),
        ("J", dim(&[(m, 1), (l, 2), (t, -2)])),
        ("W", dim(&[(m, 1), (l, 2), (t, -3)])),
        ("C", dim(&[(i, 1), (t, 1)])),
        ("V", dim(&[(m, 1), (l, 2), (t, -3), (i, -1)])),
        ("F", dim(&[(m, -1), (l, -2), (t, 4), (i, 2)])),
        ("Ω", dim(&[(m, 1), (l, 2), (t, -3), (i, -2)])),
        ("S", dim(&[(m, -1), (l, -2), (t, 3), (i, 2)])),
        ("Wb", dim(&[(m, 1), (l, 2), (t, -2), (i, -1)])),
        ("T", dim(&[(m, 1), (t, -2), (i, -1)])),
        ("H", dim(&[(m, 1), (l, 2), (t, -2), (i, -2)])),
        ("Gy", dim(&[(l, 2), (t, -2)])),
        ("Sv", dim(&[(l, 2), (t, -2)])),
        ("Bq", dim(&[(t, -1)])),
        ("°C", Dim::of(Base::Temperature)),
    ];
    for (symbol, want) in cases {
        assert_eq!(live(symbol).dims(), *want, "{symbol}");
    }
}

#[test]
fn the_derived_unit_and_its_definition_are_interchangeable() {
    for (symbol, defn) in [
        ("N", "kg.m.s^-2"),
        ("Pa", "N.m^-2"),
        ("J", "N.m"),
        ("W", "J.s^-1"),
        ("V", "W.A^-1"),
        ("Ω", "V.A^-1"),
        ("C", "A.s"),
        ("T", "Wb.m^-2"),
        ("Gy", "J.kg^-1"),
        ("kat", "mol.s^-1"),
    ] {
        let (a, b) = (live(symbol), live(defn));
        assert_eq!(a.dims(), b.dims(), "{symbol} = {defn}");
        assert!(
            close(a.convert_value(1.0, &b).unwrap(), 1.0),
            "{symbol} = {defn}"
        );
    }
}

#[test]
fn quantity_types_name_dimensions() {
    let reg = Registry::builtin();
    let types = [
        ("Hz", "Frequency"),
        ("N", "Force"),
        ("Pa", "Pressure"),
        ("J", "Energy"),
        ("W", "Power"),
        ("C", "Charge"),
        ("V", "Potential"),
        ("F", "Capacitance"),
        ("Ω", "Resistance"),
        ("S", "Conductance"),
        ("Wb", "MagneticFlux"),
        ("T", "MagneticField"),
        ("H", "Inductance"),
        ("°C", "Temperature"),
        ("Bq", "Radioactivity"),
        ("Gy", "Dose"),
        ("kat", "CatalyticActivity"),
        ("m", "Length"),
        ("kg", "Mass"),
        ("m.s^-1", "Speed"),
        ("m.s^-2", "Acceleration"),
    ];
    for (symbol, name) in types {
        let ty = reg
            .quantity_type(name)
            .unwrap_or_else(|| panic!("no type {name}"));
        assert!(ty.admits(&live(symbol)), "{name} admits {symbol}");
    }
}

#[test]
fn a_type_admits_by_dimension_only() {
    let reg = Registry::builtin();
    let length = reg.quantity_type("Length").unwrap();
    for u in ["m", "cm", "km", "μm", "qm"] {
        assert!(length.admits(&live(u)), "{u}");
    }
    for u in ["s", "kg", "m^2", "m.s^-1", "N"] {
        assert!(!length.admits(&live(u)), "{u}");
    }
    // Every Length is a Quantity of that dimension, and a Torque is an
    // Energy: the name is not inferred from the dimension.
    let energy = reg.quantity_type("Energy").unwrap();
    let torque = reg.quantity_type("Torque").unwrap();
    assert!(energy.admits(&live("N.m")) && torque.admits(&live("N.m")));
    assert!(energy.admits(&live("J")) && torque.admits(&live("J")));
    assert!(reg.quantity_type("Nonsense").is_none());
}

#[test]
fn several_names_may_share_a_dimension() {
    let reg = Registry::builtin();
    let names = |symbol: &str| -> Vec<String> {
        let mut v: Vec<String> = reg
            .quantity_types_for(live(symbol).dims())
            .iter()
            .map(|t| t.name().to_string())
            .collect();
        v.sort();
        v
    };
    assert_eq!(names("Hz"), ["Frequency", "Radioactivity"]);
    assert_eq!(names("J"), ["Energy", "Torque"]);
    assert_eq!(names("Gy"), ["Dose", "SpecificEnergy"]);
}

#[test]
fn a_preferred_unit_has_the_dimension() {
    let reg = Registry::builtin();
    let force = reg.quantity_type("Force").unwrap();
    assert_eq!(force.preferred_unit().unwrap().to_string(), "N");
    let speed = reg.quantity_type("Speed").unwrap();
    assert_eq!(speed.preferred_unit().unwrap().to_string(), "m.s^-1");
    for t in [
        "Length",
        "Mass",
        "Area",
        "Energy",
        "Torque",
        "Resistance",
        "SpecificHeat",
    ] {
        let ty = reg.quantity_type(t).unwrap();
        assert!(ty.admits(&ty.preferred_unit().unwrap()), "{t}");
    }
}

mod bad_resources {
    use super::*;

    const PREFIXES: &str = "\
- {symbol: k, name: kilo, base: 10, exp: 3}
- {symbol: m, name: milli, base: 10, exp: -3}
";

    fn load(units: &str) -> Result<Registry, Error> {
        Registry::from_yaml(PREFIXES, units, "[]")
    }

    fn message(r: Result<Registry, Error>) -> String {
        match r {
            Err(Error::Resource(m)) => m,
            Err(e) => panic!("wrong error: {e}"),
            Ok(_) => panic!("should have failed"),
        }
    }

    #[test]
    fn an_ambiguous_symbol_is_refused() {
        // milli + inch reads as `min`, which is also the minute.
        let m = message(load(
            "\
- {symbol: m, name: metre, base: Length, prefixable: si}
- {symbol: in, name: inch, defn: m, factor: \"0.0254\", prefixable: si}
- {symbol: min, name: minute, base: Time}
",
        ));
        assert!(m.contains("`min` is ambiguous"), "{m}");
    }

    #[test]
    fn a_duplicate_symbol_is_refused() {
        let m = message(load(
            "\
- {symbol: m, name: metre, base: Length}
- {symbol: m, name: mile, base: Length}
",
        ));
        assert!(m.contains("used twice"), "{m}");
    }

    #[test]
    fn a_forward_reference_is_refused() {
        let m = message(load(
            "\
- {symbol: ft, name: foot, defn: m, factor: \"0.3048\"}
- {symbol: m, name: metre, base: Length}
",
        ));
        assert!(m.contains("unknown unit `m`"), "{m}");
    }

    #[test]
    fn an_affine_unit_cannot_take_prefixes() {
        let m = message(load(
            "\
- {symbol: K, name: kelvin, base: Temperature}
- {symbol: C, name: celsius, defn: K, offset: \"273.15\", prefixable: si}
",
        ));
        assert!(m.contains("affine"), "{m}");
    }

    #[test]
    fn a_unit_needs_a_base_or_a_defn() {
        assert!(message(load("- {symbol: x, name: x}")).contains("exactly one"));
        assert!(
            message(load("- {symbol: x, name: x, base: Length, defn: m}")).contains("exactly one")
        );
        assert!(message(load("- {symbol: x, name: x, base: Nope}")).contains("not a base"));
    }

    #[test]
    fn a_symbol_cannot_hold_the_notation() {
        assert!(message(load("- {symbol: \"a.b\", name: x, base: Length}")).contains("symbol"));
        assert!(message(load("- {symbol: \"a b\", name: x, base: Length}")).contains("symbol"));
    }

    #[test]
    fn unknown_fields_are_refused() {
        assert!(matches!(
            load("- {symbol: x, name: x, base: Length, colour: red}"),
            Err(Error::Resource(_))
        ));
    }

    #[test]
    fn a_type_unit_must_match_its_dimension() {
        let units = "- {symbol: m, name: metre, base: Length, prefixable: si}";
        let r = Registry::from_yaml(PREFIXES, units, "- {name: Wrong, dims: {Mass: 1}, unit: m}");
        assert!(message(r).contains("different dimension"));
    }
}

#[test]
fn quantity_types_have_stable_ids() {
    let reg = Registry::builtin();
    let all: Vec<_> = reg.quantity_types().collect();
    assert!(all.len() >= 40);
    for (i, t) in all.iter().enumerate() {
        assert_eq!(usize::from(t.id()), i);
        assert_eq!(reg.quantity_type_by_id(t.id()).unwrap(), *t);
        assert_eq!(reg.quantity_type(t.name()).unwrap(), *t);
    }
    assert!(reg.quantity_type_by_id(u16::MAX).is_none());
    // Names are unique.
    let mut names: Vec<_> = all.iter().map(|t| t.name().to_string()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), all.len());
}

#[test]
fn a_scale_is_a_base_and_an_exponent() {
    let reg = Registry::builtin();
    assert_eq!(reg.scale("k."), Some((10, 3)));
    assert_eq!(reg.scale("M."), Some((10, 6)));
    assert_eq!(reg.scale("m."), Some((10, -3)));
    assert_eq!(reg.scale("T."), Some((10, 12)));
    assert_eq!(reg.scale("da."), Some((10, 1)));
    assert_eq!(reg.scale("μ."), Some((10, -6)));
    assert_eq!(reg.scale("\u{b5}."), Some((10, -6)));
    assert_eq!(reg.scale("q."), Some((10, -30)));
    assert_eq!(reg.scale("Ki."), Some((2, 10)));
    assert_eq!(reg.scale("Gi."), Some((2, 30)));
    for not in ["k", "kg.", "k..", ".", "", "m", "x.", "k.m"] {
        assert_eq!(reg.scale(not), None, "{not}");
    }
    // It agrees with the unit a scale parses to.
    for p in reg.prefixes() {
        let text = format!("{}.", p.symbol);
        assert_eq!(reg.scale(&text), Some((p.base, p.exp)), "{text}");
        let unit = reg.parse_unit(&text, Scope::Live).unwrap();
        let want: f64 = if p.base == 2 {
            2f64.powi(i32::from(p.exp))
        } else {
            format!("1e{}", p.exp).parse().unwrap()
        };
        assert!(close(unit.factor(), want), "{text}");
    }
}

#[test]
fn types_and_units_are_grouped_for_menus() {
    use siquant::Registry;
    let reg = Registry::builtin();
    let group = |n: &str| reg.quantity_type(n).unwrap().group().to_string();
    // A type is filed under the heading of its preferred unit.
    assert_eq!(group("Mass"), "SI Base");
    assert_eq!(group("Area"), "Magnitude");
    assert_eq!(group("Force"), "Physical");
    assert_eq!(group("Resistance"), "Electromagnetic");
    assert_eq!(group("SpecificHeat"), "Thermal");
    assert_eq!(group("Dose"), "Radiation");
    assert_eq!(group("Dimensionless"), "Other");
    let unit = |s: &str| {
        reg.units()
            .find(|u| u.symbol == s)
            .unwrap()
            .group
            .to_string()
    };
    // The gram is not an SI base unit: the kilogram is, which is `Mass`'s.
    assert_eq!(unit("g"), "Magnitude");
    assert_eq!(unit("au"), "Astronomical");
    assert_eq!(unit("ct"), "Contextual");
    assert_eq!(unit("rpm"), "Contextual");
    assert_eq!(unit("m"), "SI Base");
    assert_eq!(unit("N"), "Physical");
    assert_eq!(unit("°C"), "Thermal");
    assert_eq!(unit("kat"), "Chemical");
    // The headings and what is under them are those of `resources/menu.txt`,
    // in its order.
    let menu: Vec<(&str, &[String])> = reg.menu().collect();
    assert_eq!(menu[0].0, "SI Base");
    assert_eq!(menu[0].1, ["m", "kg", "s", "A", "K", "mol", "cd"]);
    let contextual = menu.iter().find(|(h, _)| *h == "Contextual").unwrap();
    let at = |s: &str| contextual.1.iter().position(|u| u == s).unwrap();
    assert_eq!(at("rpm"), at("rev") + 1);
    // `Other` is added last for what the menu does not name, a type of no unit.
    assert_eq!(reg.groups().last(), Some(&"Other"));
    // Every group named is one of the headings.
    let groups = reg.groups();
    assert!(reg.quantity_types().all(|t| groups.contains(&t.group())));
    assert!(reg.units().all(|u| groups.contains(&u.group)));
    // The base types are the seven with a base unit of their own, `Duration`
    // being the one for time.
    assert_eq!(
        reg.quantity_types()
            .filter(|t| t.group() == "SI Base")
            .count(),
        7
    );
}

#[test]
fn every_live_unit_and_preferred_unit_is_in_the_menu() {
    use siquant::Registry;
    // A menu that leaves one out is refused, naming it.
    let units = "- {symbol: m, name: metre, base: Length}\n- {symbol: ft, name: foot, defn: m, factor: \"0.3048\"}";
    let err = Registry::from_yaml_with_menu("[]", units, "[]", "== A\nm | the metre\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("`ft` is in no group"), "{err}");
    // And one that names a unit twice, or one that is not a unit.
    let err = Registry::from_yaml_with_menu("[]", units, "[]", "== A\nm\nft\n== B\nm\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("twice"), "{err}");
    let err = Registry::from_yaml_with_menu("[]", units, "[]", "== A\nm\nft\nfurlong\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("furlong"), "{err}");
    let ok = Registry::from_yaml_with_menu("[]", units, "[]", "== A\nm\n== B\nft | foot\n");
    assert!(ok.is_ok());
}

#[test]
fn a_dimension_with_several_types_is_inferred_as_the_fixed_one() {
    use siquant::{Registry, Scope, Unit};
    let reg = Registry::builtin();
    let of = |unit: &str| {
        let u = Unit::parse(unit, Scope::Live).unwrap();
        reg.infer_type(u.dims()).map(|t| t.name().to_string())
    };
    // One type with the dimension: that one.
    assert_eq!(of("m.s^-1").as_deref(), Some("Speed"));
    assert_eq!(of("s").as_deref(), Some("Duration"));
    // Several: the fixed one, whichever the resource file lists first.
    assert_eq!(of("J").as_deref(), Some("Energy"));
    assert_eq!(of("N.m").as_deref(), Some("Energy"));
    assert_eq!(of("Hz").as_deref(), Some("Frequency"));
    assert_eq!(of("kg.m.s^-1").as_deref(), Some("Momentum"));
    assert_eq!(of("J.kg^-1").as_deref(), Some("SpecificEnergy"));
    assert_eq!(of("m^2").as_deref(), Some("Area"));
    // None: a plain quantity.
    assert_eq!(of("kg.m^5"), None);
}

#[test]
fn the_astronomical_and_metric_units_have_their_values() {
    use siquant::{Quantity, Registry, Scope, Unit};
    let reg = Registry::builtin();
    let _ = reg;
    let to = |v: f64, from: &str, into: &str| {
        let a = Quantity::exact(v, Unit::parse(from, Scope::Live).unwrap());
        a.convert(&Unit::parse(into, Scope::Live).unwrap())
            .unwrap()
            .value()
    };
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs();
    // A light year is a Julian year at the speed of light.
    assert!(near(to(1.0, "ly", "m"), 299_792_458.0 * 31_557_600.0));
    assert!(near(to(1.0, "ld", "m"), 299_792_458.0 * 86_400.0));
    assert!(near(to(1.0, "au", "km"), 149_597_870.7));
    assert!(near(to(1.0, "pc", "ly"), 3.261_563_777_167_433));
    assert!(near(to(1.0, "pc", "au"), 648_000.0 / std::f64::consts::PI));
    assert!(near(to(1.0, "kpc", "pc"), 1000.0) && near(to(1.0, "Gyr", "yr"), 1e9));
    assert!(near(to(1.0, "yr", "d"), 365.25) && near(to(1.0, "Å", "pm"), 100.0));
    assert!(near(to(1.0, "nmi", "m"), 1852.0) && near(to(1.0, "kn", "m.s^-1"), 1852.0 / 3600.0));
    assert!(near(to(1.0, "are", "m^2"), 100.0) && near(to(1.0, "barn", "m^2"), 1e-28));
    assert!(near(to(1.0, "mbarn", "barn"), 1e-3));
    assert!(near(to(1.0, "ct", "mg"), 200.0));
    assert!(near(to(1.0, "kt", "t"), 1000.0) && near(to(1.0, "Mt", "kt"), 1000.0));
    assert!(near(to(1.0, "rev", "°"), 360.0) && near(to(100.0, "gon", "°"), 90.0));
    assert!(near(to(1.0, "atm", "Pa"), 101_325.0) && near(to(760.0, "torr", "atm"), 1.0));
    assert!(near(to(1.0, "mhp", "W"), 735.498_75));
    assert!(near(to(100.0, "rem", "Sv"), 1.0) && near(to(1.0, "Ci", "Bq"), 3.7e10));
    assert!(near(to(1.0, "R", "C.kg^-1"), 2.58e-4) && near(to(1.0, "mCi", "Bq"), 3.7e7));
    assert!(near(to(1.0, "Jy", "W.m^-2.Hz^-1"), 1e-26) && near(to(1.0, "tog", "K.m^2.W^-1"), 0.1));
    assert!(near(to(1.0, "sverdrup", "m^3.s^-1"), 1e6));
    // `PS` is a petasiemens, and `b` the bit: neither is a horsepower or a barn.
    assert!(
        Unit::parse("PS", Scope::Live).unwrap().dims()
            == Unit::parse("S", Scope::Live).unwrap().dims()
    );
    assert_eq!(to(1.0, "B", "b"), 8.0);
}

#[test]
fn the_non_metric_units_have_their_values() {
    use siquant::{Registry, Scope, Unit};
    let reg = Registry::builtin();
    let to = |v: f64, from: &str, into: &str| {
        let a = Unit::parse(from, Scope::Live).unwrap();
        let b = Unit::parse(into, Scope::Live).unwrap();
        a.convert_value(v, &b).unwrap()
    };
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1.0);
    assert!(near(to(1.0, "ft", "m"), 0.3048));
    assert!(near(to(1.0, "mi", "m"), 1609.344));
    assert!(near(to(1.0, "lb", "g"), 453.592_37));
    assert!(near(to(1.0, "ton", "kg"), 907.184_74));
    assert!(near(to(1.0, "gal", "l"), 3.785_411_784));
    assert!(near(to(1.0, "ac", "m^2"), 4_046.856_422_4));
    assert!(near(to(1.0, "ft^2", "m^2"), 0.092_903_04));
    assert!(near(to(1.0, "psi", "Pa"), 6_894.757_293_168_361));
    assert!(near(to(1.0, "hp", "W"), 745.699_871_582_270_2));
    assert!(near(to(1.0, "mph", "m.s^-1"), 0.447_04));
    assert!(near(to(212.0, "°F", "°C"), 100.0));
    assert!(near(to(32.0, "°F", "K"), 273.15));
    // The gram per square metre, which is metric.
    assert!(near(to(80.0, "gsm", "kg.m^-2"), 0.08));
    let metric = |s: &str| reg.units().find(|u| u.symbol == s).unwrap().metric;
    assert!(metric("gsm") && metric("mhp") && !metric("ft") && !metric("°F"));
    assert!(
        reg.units()
            .filter(|u| u.currency)
            .all(|u| u.dims == Dim::of(Base::Currency))
    );
}

#[test]
fn every_menu_entry_is_written_as_the_registry_writes_it() {
    use siquant::{Registry, Scope, Unit};
    let reg = Registry::builtin();
    for (_, entries) in reg.menu() {
        for e in entries {
            let u = Unit::parse(e, Scope::Live).unwrap();
            assert_eq!(&u.to_string(), e, "`{e}` is written another way");
        }
    }
}

#[test]
fn a_currency_is_a_dimension_of_its_own() {
    use siquant::{Registry, Scope, Unit};
    let reg = Registry::builtin();
    // The dollar is the base, and its symbol is an alternate of `USD`.
    let usd = Unit::parse("USD", Scope::Live).unwrap();
    assert_eq!(Unit::parse("$", Scope::Live).unwrap(), usd);
    assert_eq!(usd.to_string(), "USD");
    for alt in ["€", "£", "¥", "￥", "A$", "NZ$"] {
        assert!(Unit::parse(alt, Scope::Live).is_ok(), "{alt}");
    }
    assert_eq!(
        Unit::parse("￥", Scope::Live).unwrap(),
        Unit::parse("JPY", Scope::Live).unwrap()
    );
    // Money over data is its own dimension: `KiB/$`.
    let per = Unit::parse("KiB.$^-1", Scope::Live).unwrap();
    assert_eq!(
        per.dims(),
        Dim::NONE.with(Base::Data, 1).with(Base::Currency, -1)
    );
    // Every currency, crypto included, is in the registry and in the menu.
    assert!(reg.units().filter(|u| u.currency).count() > 300);
    for region in ["Europe", "Americas", "Africa", "Asia-Pacific", "Digital"] {
        assert!(reg.groups().contains(&region), "{region}");
    }
    let sector = |code: &str| {
        reg.units()
            .find(|u| u.symbol == code)
            .unwrap()
            .group
            .to_string()
    };
    assert_eq!(sector("EUR"), "Europe");
    assert_eq!(sector("USD"), "Americas");
    assert_eq!(sector("ZAR"), "Africa");
    assert_eq!(sector("JPY"), "Asia-Pacific");
    assert_eq!(sector("BTC"), "Digital");
    assert_eq!(sector("XAU"), "Other");
    for code in ["EUR", "BTC", "ETH", "XAU"] {
        assert!(reg.rate(code).is_some(), "{code}");
    }
}

#[test]
fn a_unit_says_the_other_symbols_it_is_written_with() {
    let reg = siquant::Registry::builtin();
    let alts = |code: &str| {
        reg.units()
            .find(|u| u.symbol == code)
            .unwrap()
            .alts
            .to_vec()
    };
    assert_eq!(alts("EUR"), ["€"]);
    assert_eq!(alts("JPY"), ["¥", "￥"]);
    assert!(alts("m").is_empty());
}

#[test]
fn a_currency_has_a_sign_to_show_it_by() {
    let reg = siquant::Registry::builtin();
    assert_eq!(reg.currency_sign("USD"), Some("$"));
    assert_eq!(reg.currency_sign("EUR"), Some("€"));
    assert_eq!(reg.currency_sign("AUD"), Some("A$"));
    // No sign, or no currency.
    assert_eq!(reg.currency_sign("CHF"), None);
    assert_eq!(reg.currency_sign("kg"), None);
}

#[test]
fn rates_set_what_a_currency_is_worth() {
    use siquant::{Quantity, Registry, Scope, Unit};
    let reg = Registry::builtin();
    reg.set_rates(
        "2030-01-01",
        [("eur", 0.8), ("GBP", 0.5), ("nonsense", 2.0), ("usd", 9.0)],
    );
    assert_eq!(reg.rates_as_of(), "2030-01-01");
    // The dollar is the base: it is always one, and no rate moves it.
    assert_eq!(reg.rate("USD"), Some(1.0));
    assert_eq!(reg.rate("EUR"), Some(0.8));
    let eur = Unit::parse("EUR", Scope::Live).unwrap();
    let gbp = Unit::parse("GBP", Scope::Live).unwrap();
    // 0.8 EUR make a dollar and 0.5 GBP make one, so 1 GBP is 1.6 EUR.
    let q = Quantity::new(1.0, 0.0, gbp).unwrap().convert(&eur).unwrap();
    assert!((q.value() - 1.6).abs() < 1e-12, "{}", q.value());
    // A bad rate is left out.
    assert_eq!(
        reg.set_rates("x", [("EUR", -1.0), ("EUR", f64::NAN), ("EUR", 0.0)]),
        0
    );
    assert_eq!(reg.rates_as_of(), "2030-01-01");
}
