# siquant

SI units, prefixes and quantities with uncertainty, defined by YAML resource files. A second-generation, much simpler Rust successor to the Raku `Physics::Unit` and `Physics::Measure` modules.

Status: SI units and prefixes, data, angles, durations, non-metric units and currency are implemented. The staged plan is in [docs/plan.md](docs/plan.md).

## Cell text

A cell holds a number followed directly by a unit: `5kg`, `300cm`, `9.81m.s^-2`, `5kg.m^2.s^-2`, `20°C`. SI units only, `.` between terms, `^` and a signed integer for an exponent, no spaces, no `/`. The crate reads the unit; the host splits the number off.

A prefix and a trailing `.` on their own is a dimensionless scale: `1k.` is 1000, `5m.` is 0.005 (`Quantity::to_number()` applies it). The dot makes it a prefix and nothing else, so `m.` is milli and `T.` is tera, not the metre and the tesla. Note that a stray dot after `m` or `T` therefore reads as a scale rather than failing.

```rust
use siquant::{Quantity, Scope, Unit};

let m = Unit::parse("m", Scope::Live)?;
let cm = Unit::parse("cm", Scope::Live)?;
let sum = Quantity::exact(5.0, m).add(&Quantity::exact(300.0, cm))?;
assert_eq!(sum.value(), 8.0);
```

## Currency

Money is a base dimension of its own, `$`, with the US dollar, `USD` (alternate `$`), as its unit, and `Currency` is a quantity type. Every currency of [currency-api](https://latest.currency-api.pages.dev/v1/currencies/usd.json), fiat and crypto (about 340 codes), is a unit by its code, and some also by a sign (`€`, `£`, `¥`, `A$`). Money mixes with every other unit: `KiB.$^-1` is data for the money.

A currency is worth what the rates say. A snapshot of them is compiled in, so there is always a rate; `Registry::set_rates` replaces it with live ones (how many of a code make a dollar, and the date they are as of), and a `Unit` parsed afterwards uses them. A currency with no rate is `Error::NoRate`. The crate never fetches: that is the host's. `tools/currency.py` regenerates the currency resources and the snapshot.

```rust
use siquant::{Quantity, Registry, Scope, Unit};

Registry::builtin().set_rates("2026-11-18", [("eur", 0.86), ("gbp", 0.75)]);
let gbp = Quantity::exact(1.0, Unit::parse("GBP", Scope::Live)?);
let eur = Unit::parse("€", Scope::Live)?;
let in_eur = gbp.convert(&eur)?; // about 1.147 EUR
```

## Development

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Licence

MIT. See [LICENSE](LICENSE).
