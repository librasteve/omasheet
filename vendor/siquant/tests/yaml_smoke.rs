// Copyright (c) 2026 Stephen Roe

//! Proves the chosen YAML crate reads the shape the resource files will use:
//! a list of maps, decimal strings kept as text, and non-ASCII symbols.

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
struct Row {
    symbol: String,
    name: String,
    factor: String,
}

#[test]
fn resource_shaped_yaml_loads() {
    let text = "\
- {symbol: μ, name: micro, factor: '1e-6'}
- {symbol: Ω, name: ohm, factor: '1'}
";
    let rows: Vec<Row> = serde_yaml_ng::from_str(text).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].symbol, "μ");
    assert_eq!(rows[0].factor.parse::<f64>().unwrap(), 1e-6);
    assert_eq!(rows[1].name, "ohm");
}
