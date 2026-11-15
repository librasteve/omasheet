# siquant

SI units, prefixes and quantities with uncertainty, defined by YAML resource files. A second-generation, much simpler Rust successor to the Raku `Physics::Unit` and `Physics::Measure` modules.

Status: Stage 1 (SI units and SI prefixes) is implemented. Data prefixes, angles and durations are next. The staged plan is in [docs/plan.md](docs/plan.md).

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

## Development

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Licence

MIT. See [LICENSE](LICENSE).
