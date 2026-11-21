# Release notes and migration

Omasheet is early and changing. This chapter says where the release notes are, and lists the breaking changes a file written for an older version may need to follow.

## Release notes

Each release's notes are in `docs/releases/`:

| Notes | Covers |
|---|---|
| [Next release](releases/unreleased.md) | what has changed since 0.1.12, kept until it is cut |
| [0.1.8 to 0.1.12](releases/0.1.12.md) | five releases: units, `Decimal` scale and exact `Fraction`, windowed columns in Polars, `Boolean` formats and logic, built-in constants |
| [0.1.11](releases/0.1.11.md), [0.1.10](releases/0.1.10.md), [0.1.9](releases/0.1.9.md), [0.1.8](releases/0.1.8.md) | each release on its own |

The releases page on GitHub has the installers: <https://github.com/librasteve/omasheet/releases>. To update, run the install line again.

## Breaking changes

A file written for an older version may need the changes below. `omasheet lint file.omx` reports what no longer reads, with the line and usually a `help:` line that says what to write.

### In the next release

- **A comment line is `//`, not `#`.** A `#` line is a heading in Markdown, so an `.omx` read as Markdown showed its comments as headings. There is no compatibility form: change `# note` to `// note`.
- **`Decimal(n)` is `Decimal[n]`**, and takes a format after a `;`, `Decimal[2; .4f]`. A declared `Decimal[n]` shows `n` places by default.
- **`Duration` is a kind of `Quantity`, for time.** A bare `Duration` column now shows seconds, `Duration[s]`, where it showed `PT1H30M`. Ask for the old look with `Duration[iso]`.
- **`.Quantity` and `.Duration` are gone.** A `Duration` is already a quantity of time, so there is nothing to convert. There is no compatibility form.
- **An uncertainty on a `Duration`** (`10.5(3)s`) is held as seconds and the column is calculated by the engine.

### In 0.1.6

Numbers became the Polars dtypes, so exactness narrowed from rational to a 38-digit decimal: `Int` is 64-bit rather than arbitrary-precision, and a result that does not fit is reported. `Complex` was dropped and has since returned, as the most infectious number type, stored in Parquet as a struct of `re` and `im`. The type names follow Polars: `Bool` is `Boolean`, `Text` is `String`, `Num` is `Float`, and an empty cell is `Null`. Files written for 0.1.5 and earlier may need their schema lines renamed.

## Reading the notes

The notes for the next release list new features as well as breaking changes: results that have a kind (`Force * Length` is an `Energy`), powers and roots of quantities, new metric units, and the display of durations.
