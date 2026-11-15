// Copyright (c) 2026 Stephen Roe

//! The registry: every unit, prefix and quantity type, loaded once from the
//! YAML resource files and never changed afterwards.
//!
//! It is normalised. Units, prefixes and types are separate tables addressed
//! by id; `symbol -> id` is one map per table; a prefixed unit such as `km`
//! is never stored, it is a prefix and a unit found at parse time. At load
//! every prefix x unit combination is checked for ambiguity, so a symbol has
//! one reading.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use crate::dim::{Base, Dim};
use crate::error::{Error, Result};
use crate::quantity::QuantityType;
use crate::unit::{Term, Unit};

const PREFIXES_YAML: &str = include_str!("../resources/prefixes.yaml");
const UNITS_YAML: &str = include_str!("../resources/units.yaml");
const TYPES_YAML: &str = include_str!("../resources/quantity_types.yaml");

/// Which unit a [`Term`] names: an index into the registry's unit table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitId(pub(crate) u32);

/// Which prefix a [`Term`] has: an index into the registry's prefix table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrefixId(pub(crate) u32);

/// Which symbols a parse may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Cell text: only units marked `live`.
    Live,
    /// A column unit or a definition: any unit in the registry.
    Any,
}

/// Which prefixes a unit takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Prefixable {
    #[default]
    None,
    Si,
    Binary,
    Both,
}

/// The headings a menu files units and quantity types under, in order. A
/// resource file names one of them in its `group` field; the default is the
/// last.
pub const GROUPS: [&str; 10] = [
    "SI Base",
    "Data",
    "Angle",
    "Light",
    "Size",
    "Mechanical",
    "Electromagnetic",
    "Thermodynamic",
    "Radiation",
    "Other",
];

fn group_of(what: &str, group: Option<&str>, default: &str) -> Result<String> {
    let group = group.unwrap_or(default);
    if GROUPS.contains(&group) {
        Ok(group.to_string())
    } else {
        resource(
            what,
            format!("`{group}` is not a group: {}", GROUPS.join(", ")),
        )
    }
}

/// Whether a prefix is a power of ten or of two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixKind {
    Si,
    Binary,
}

impl Prefixable {
    pub(crate) fn allows(self, kind: PrefixKind) -> bool {
        matches!(
            (self, kind),
            (Prefixable::Both, _)
                | (Prefixable::Si, PrefixKind::Si)
                | (Prefixable::Binary, PrefixKind::Binary)
        )
    }
}

pub(crate) struct PrefixDef {
    pub symbol: String,
    pub name: String,
    pub alts: Vec<String>,
    pub base: u8,
    pub exp: i16,
    pub scale: f64,
    pub kind: PrefixKind,
}

impl PrefixDef {
    pub(crate) fn all_symbols(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.symbol.as_str()).chain(self.alts.iter().map(String::as_str))
    }
}

pub(crate) struct UnitDef {
    pub symbol: String,
    pub name: String,
    pub alts: Vec<String>,
    pub dims: Dim,
    /// The size of the unit in the coherent SI unit of its dimension.
    pub factor: f64,
    /// Added in the unit's own scale before scaling.
    pub offset: f64,
    pub prefixable: Prefixable,
    pub live: bool,
    /// What the unit is defined as; a base unit is defined as itself.
    pub defn: Vec<Term>,
    /// Is it a base unit, one defined by a dimension and not by other units?
    pub is_base: bool,
    /// The heading a menu files it under, one of [`GROUPS`].
    pub group: String,
}

impl UnitDef {
    pub(crate) fn all_symbols(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.symbol.as_str()).chain(self.alts.iter().map(String::as_str))
    }
}

pub(crate) struct TypeDef {
    pub name: String,
    pub dims: Dim,
    pub unit: Option<Vec<Term>>,
    pub group: String,
}

#[derive(Default)]
pub(crate) struct Inner {
    pub prefixes: Vec<PrefixDef>,
    pub units: Vec<UnitDef>,
    pub types: Vec<TypeDef>,
    pub unit_by_symbol: HashMap<String, UnitId>,
    pub prefix_by_symbol: HashMap<String, PrefixId>,
    /// The dimensionless unit a bare prefix scales: `k.` is prefix `k` and
    /// this. It has no symbol, so it is reached only by that form.
    pub unity: UnitId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefixRow {
    symbol: String,
    name: String,
    base: u8,
    exp: i16,
    #[serde(default)]
    alt: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitRow {
    symbol: String,
    name: String,
    #[serde(default)]
    alt: Vec<String>,
    base: Option<String>,
    defn: Option<String>,
    factor: Option<String>,
    offset: Option<String>,
    #[serde(default)]
    prefixable: Prefixable,
    #[serde(default = "yes")]
    live: bool,
    group: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeRow {
    name: String,
    dims: BTreeMap<String, i8>,
    unit: Option<String>,
    group: Option<String>,
}

fn resource<T>(what: &str, e: impl fmt::Display) -> Result<T> {
    Err(Error::Resource(format!("{what}: {e}")))
}

/// A symbol is written in cell text, so it cannot hold the notation itself.
fn check_symbol(symbol: &str) -> Result<()> {
    let bad = symbol.is_empty()
        || symbol
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '.' | '^' | '/' | '*' | '(' | ')'));
    if bad {
        return resource(
            "symbol",
            format!("`{symbol}` is empty or holds a space, `.`, `^`, `/`, `*` or a bracket"),
        );
    }
    Ok(())
}

fn parse_decimal(what: &str, text: &str) -> Result<f64> {
    match text.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => resource(what, format!("`{text}` is not a finite decimal number")),
    }
}

impl Inner {
    pub(crate) fn unit_def(&self, id: UnitId) -> &UnitDef {
        &self.units[id.0 as usize]
    }

    pub(crate) fn prefix_def(&self, id: PrefixId) -> &PrefixDef {
        &self.prefixes[id.0 as usize]
    }

    /// Makes `unity`, which must be the first unit (id 0).
    fn add_unity(&mut self) {
        let id = UnitId(self.units.len() as u32);
        self.unity = id;
        self.units.push(UnitDef {
            symbol: String::new(),
            name: "unity".to_string(),
            alts: Vec::new(),
            dims: Dim::NONE,
            factor: 1.0,
            offset: 0.0,
            prefixable: Prefixable::None,
            live: true,
            defn: vec![Term {
                prefix: None,
                unit: id,
                exp: 1,
            }],
            is_base: true,
            group: "Other".to_string(),
        });
    }

    /// The size of one term in the coherent unit: (prefix x unit) ^ exponent.
    pub(crate) fn term_factor(&self, t: &Term) -> f64 {
        let scale = t.prefix.map_or(1.0, |p| self.prefix_def(p).scale);
        (scale * self.unit_def(t.unit).factor).powi(i32::from(t.exp))
    }

    /// The dimension, factor and offset of a normalised list of terms.
    pub(crate) fn eval(&self, terms: &[Term]) -> Result<(Dim, f64, f64)> {
        let mut dims = Dim::NONE;
        let mut factor = 1.0;
        for t in terms {
            let scaled = self
                .unit_def(t.unit)
                .dims
                .checked_scale(t.exp)
                .ok_or(Error::Overflow)?;
            dims = dims.checked_add(scaled).ok_or(Error::Overflow)?;
            factor *= self.term_factor(t);
        }
        if !factor.is_finite() || factor == 0.0 {
            return Err(Error::Overflow);
        }
        let offset = match terms {
            [t] if t.exp == 1 && t.prefix.is_none() => self.unit_def(t.unit).offset,
            _ => 0.0,
        };
        Ok((dims, factor, offset))
    }

    /// Terms with an affine unit only ever alone, like terms merged, zero
    /// exponents dropped, and in the canonical order: positive exponents
    /// first, named derived units before base units (`N.m`, not `m.N`), then
    /// the order of the resource file, then no prefix before a prefix.
    pub(crate) fn normalise(&self, terms: Vec<Term>) -> Result<Vec<Term>> {
        if let Some(t) = terms
            .iter()
            .find(|t| self.unit_def(t.unit).offset != 0.0)
            .filter(|t| terms.len() != 1 || t.exp != 1)
        {
            return Err(Error::AffineInCompound(
                self.unit_def(t.unit).symbol.clone(),
            ));
        }
        // A scale (`k.`) is a whole unit too: `k.m` would be unreadable.
        if let Some(t) = terms
            .iter()
            .find(|t| t.unit == self.unity)
            .filter(|t| terms.len() != 1 || t.exp != 1 || t.prefix.is_none())
        {
            let prefix = t.prefix.map_or("", |p| self.prefix_def(p).symbol.as_str());
            return Err(Error::ScaleInCompound(format!("{prefix}.")));
        }
        let mut out: Vec<Term> = Vec::with_capacity(terms.len());
        for t in terms {
            match out
                .iter_mut()
                .find(|o| o.prefix == t.prefix && o.unit == t.unit)
            {
                Some(o) => o.exp = o.exp.checked_add(t.exp).ok_or(Error::Overflow)?,
                None => out.push(t),
            }
        }
        out.retain(|t| t.exp != 0);
        out.sort_by_key(|t| (t.exp < 0, self.unit_def(t.unit).is_base, t.unit, t.prefix));
        Ok(out)
    }

    fn add_prefix(&mut self, row: PrefixRow) -> Result<()> {
        let what = format!("prefix `{}`", row.symbol);
        let (scale, kind) = match row.base {
            10 => (
                parse_decimal(&what, &format!("1e{}", row.exp))?,
                PrefixKind::Si,
            ),
            2 => (2f64.powi(i32::from(row.exp)), PrefixKind::Binary),
            b => return resource(&what, format!("base must be 10 or 2, not {b}")),
        };
        if !scale.is_finite() || scale == 0.0 {
            return resource(&what, "the scale is out of range");
        }
        let id = PrefixId(self.prefixes.len() as u32);
        let symbols = std::iter::once(row.symbol.clone()).chain(row.alt.iter().cloned());
        for s in symbols {
            check_symbol(&s)?;
            if self.prefix_by_symbol.insert(s.clone(), id).is_some() {
                return resource(&what, format!("symbol `{s}` is used twice"));
            }
        }
        self.prefixes.push(PrefixDef {
            symbol: row.symbol,
            name: row.name,
            alts: row.alt,
            base: row.base,
            exp: row.exp,
            scale,
            kind,
        });
        Ok(())
    }

    fn add_unit(&mut self, row: UnitRow) -> Result<()> {
        let what = format!("unit `{}`", row.symbol);
        let id = UnitId(self.units.len() as u32);
        let row_factor = parse_decimal(&what, row.factor.as_deref().unwrap_or("1"))?;
        let offset = parse_decimal(&what, row.offset.as_deref().unwrap_or("0"))?;
        if row_factor <= 0.0 {
            return resource(&what, "the factor must be positive");
        }
        if offset != 0.0 && row.prefixable != Prefixable::None {
            return resource(&what, "an affine unit cannot take prefixes");
        }
        let (dims, factor, defn) = match (&row.base, &row.defn) {
            (Some(b), None) => {
                let Some(base) = Base::from_name(b) else {
                    return resource(&what, format!("`{b}` is not a base dimension"));
                };
                let me = Term {
                    prefix: None,
                    unit: id,
                    exp: 1,
                };
                (Dim::of(base), row_factor, vec![me])
            }
            (None, Some(d)) => {
                let terms = self
                    .parse_terms(d, Scope::Any)
                    .or_else(|e| resource(&what, format!("defn `{d}`: {e}")))?;
                let (dims, f, off) = self.eval(&terms)?;
                if off != 0.0 {
                    return resource(&what, "defn must not be an affine unit");
                }
                (dims, f * row_factor, terms)
            }
            _ => return resource(&what, "needs exactly one of `base` and `defn`"),
        };
        let symbols = std::iter::once(row.symbol.clone()).chain(row.alt.iter().cloned());
        for s in symbols {
            check_symbol(&s)?;
            if self.unit_by_symbol.insert(s.clone(), id).is_some() {
                return resource(&what, format!("symbol `{s}` is used twice"));
            }
        }
        self.units.push(UnitDef {
            symbol: row.symbol,
            name: row.name,
            alts: row.alt,
            dims,
            factor,
            offset,
            prefixable: row.prefixable,
            live: row.live,
            defn,
            is_base: row.base.is_some(),
            group: group_of(
                &what,
                row.group.as_deref(),
                if row.base.is_some() {
                    "SI Base"
                } else {
                    "Other"
                },
            )?,
        });
        Ok(())
    }

    /// Fails if any prefix x unit symbol reads as another, or as a unit.
    fn validate_symbols(&self) -> Result<()> {
        let mut seen: HashMap<String, String> = self
            .unit_by_symbol
            .keys()
            .map(|s| (s.clone(), format!("the unit `{s}`")))
            .collect();
        for p in &self.prefixes {
            for u in &self.units {
                if !u.prefixable.allows(p.kind) {
                    continue;
                }
                for ps in p.all_symbols() {
                    for us in u.all_symbols() {
                        let s = format!("{ps}{us}");
                        let this = format!("`{ps}` + `{us}`");
                        if let Some(prev) = seen.insert(s.clone(), this.clone()) {
                            return resource(
                                "symbols",
                                format!("`{s}` is ambiguous: {prev} and {this}"),
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn add_type(&mut self, row: TypeRow) -> Result<()> {
        let what = format!("quantity type `{}`", row.name);
        if self.types.iter().any(|t| t.name == row.name) {
            return resource(&what, "the name is used twice");
        }
        let mut dims = Dim::NONE;
        for (k, v) in &row.dims {
            let Some(base) = Base::from_name(k) else {
                return resource(&what, format!("`{k}` is not a base dimension"));
            };
            dims = dims.with(base, *v);
        }
        let unit = match &row.unit {
            None => None,
            Some(text) => {
                let terms = self
                    .parse_terms(text, Scope::Any)
                    .or_else(|e| resource(&what, format!("unit `{text}`: {e}")))?;
                if self.eval(&terms)?.0 != dims {
                    return resource(&what, format!("unit `{text}` has a different dimension"));
                }
                Some(terms)
            }
        };
        self.types.push(TypeDef {
            name: row.name,
            dims,
            unit,
            group: group_of(&what, row.group.as_deref(), "Other")?,
        });
        Ok(())
    }
}

/// The registry of units, prefixes and quantity types: a cheap handle to
/// immutable data.
#[derive(Clone)]
pub struct Registry(pub(crate) Arc<Inner>);

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Registry {{ units: {}, prefixes: {}, types: {} }}",
            self.0.units.len(),
            self.0.prefixes.len(),
            self.0.types.len()
        )
    }
}

/// What a caller may know of one unit.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitInfo<'a> {
    pub symbol: &'a str,
    pub name: &'a str,
    pub dims: Dim,
    pub prefixable: Prefixable,
    pub live: bool,
    /// The heading a menu files it under, one of [`GROUPS`].
    pub group: &'a str,
}

/// What a caller may know of one prefix.
#[derive(Clone, Debug, PartialEq)]
pub struct PrefixInfo<'a> {
    pub symbol: &'a str,
    pub name: &'a str,
    pub base: u8,
    pub exp: i16,
}

impl Registry {
    /// The registry built from the resource files compiled into the crate.
    pub fn builtin() -> &'static Registry {
        static BUILTIN: OnceLock<Registry> = OnceLock::new();
        BUILTIN.get_or_init(|| {
            Registry::from_yaml(PREFIXES_YAML, UNITS_YAML, TYPES_YAML)
                .expect("the embedded resource files are valid")
        })
    }

    /// A registry from three YAML documents, in the schema of `resources/`.
    /// Units may only be defined over units listed before them.
    pub fn from_yaml(prefixes: &str, units: &str, types: &str) -> Result<Registry> {
        let prefix_rows: Vec<PrefixRow> =
            serde_yaml_ng::from_str(prefixes).or_else(|e| resource("prefixes.yaml", e))?;
        let unit_rows: Vec<UnitRow> =
            serde_yaml_ng::from_str(units).or_else(|e| resource("units.yaml", e))?;
        let type_rows: Vec<TypeRow> =
            serde_yaml_ng::from_str(types).or_else(|e| resource("quantity_types.yaml", e))?;
        let mut inner = Inner::default();
        inner.add_unity();
        for row in prefix_rows {
            inner.add_prefix(row)?;
        }
        for row in unit_rows {
            inner.add_unit(row)?;
        }
        inner.validate_symbols()?;
        for row in type_rows {
            inner.add_type(row)?;
        }
        Ok(Registry(Arc::new(inner)))
    }

    /// The unit written as `text`, in the notation of cell text.
    pub fn parse_unit(&self, text: &str, scope: Scope) -> Result<Unit> {
        let terms = self.0.parse_terms(text, scope)?;
        Unit::from_terms(self, terms)
    }

    /// The quantity type called `name`.
    pub fn quantity_type(&self, name: &str) -> Option<QuantityType> {
        let index = self.0.types.iter().position(|t| t.name == name)?;
        Some(QuantityType::new(self.clone(), index))
    }

    /// The quantity type with this id (see [`QuantityType::id`]).
    pub fn quantity_type_by_id(&self, id: u16) -> Option<QuantityType> {
        let index = usize::from(id);
        (index < self.0.types.len()).then(|| QuantityType::new(self.clone(), index))
    }

    /// Every quantity type, in the order of the resource file, for menus
    /// and help.
    pub fn quantity_types(&self) -> impl Iterator<Item = QuantityType> + '_ {
        (0..self.0.types.len()).map(|i| QuantityType::new(self.clone(), i))
    }

    /// A bare prefix and a `.`, `k.`, as the power it stands for: the base
    /// (10, or 2 for a binary prefix) and the exponent, so `k.` is `(10, 3)`.
    /// A host that reads `1k.` as the number 1000 uses this to shift an exact
    /// decimal. `None` if the text is not a prefix and a trailing `.`.
    pub fn scale(&self, text: &str) -> Option<(u8, i16)> {
        let body = text.strip_suffix('.')?;
        let p = self.0.prefix_by_symbol.get(body)?;
        let def = self.0.prefix_def(*p);
        Some((def.base, def.exp))
    }

    /// Every quantity type with exactly this dimension, to suggest a column
    /// type; never used to infer one.
    pub fn quantity_types_for(&self, dims: Dim) -> Vec<QuantityType> {
        (0..self.0.types.len())
            .filter(|&i| self.0.types[i].dims == dims)
            .map(|i| QuantityType::new(self.clone(), i))
            .collect()
    }

    /// Every unit, in the order of the resource file. (Not the unity that a
    /// bare prefix scales; it has no symbol.)
    pub fn units(&self) -> impl Iterator<Item = UnitInfo<'_>> {
        self.0
            .units
            .iter()
            .filter(|u| !u.symbol.is_empty())
            .map(|u| UnitInfo {
                symbol: &u.symbol,
                name: &u.name,
                dims: u.dims,
                prefixable: u.prefixable,
                live: u.live,
                group: &u.group,
            })
    }

    /// Every prefix, in the order of the resource file.
    pub fn prefixes(&self) -> impl Iterator<Item = PrefixInfo<'_>> {
        self.0.prefixes.iter().map(|p| PrefixInfo {
            symbol: &p.symbol,
            name: &p.name,
            base: p.base,
            exp: p.exp,
        })
    }

    pub(crate) fn same(&self, other: &Registry) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
