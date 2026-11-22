//! Scales: a word after a number that is a power of ten and no unit, as `ppm`
//! (`resources/scales.yaml`).

use serde::Deserialize;
use std::sync::OnceLock;

const SCALES_YAML: &str = include_str!("../resources/scales.yaml");

/// A scale: `symbol` stands for `base` to the `exp`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Scale {
    pub symbol: String,
    pub name: String,
    pub base: u8,
    pub exp: i16,
}

/// Every scale, in the order of the file.
pub fn scales() -> &'static [Scale] {
    static ALL: OnceLock<Vec<Scale>> = OnceLock::new();
    ALL.get_or_init(|| {
        serde_yaml_ng::from_str(SCALES_YAML).expect("the embedded scales are valid")
    })
}

/// The scale written `symbol`, if there is one.
pub fn scale_word(symbol: &str) -> Option<&'static Scale> {
    scales().iter().find(|s| s.symbol == symbol)
}
