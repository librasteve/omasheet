# Omasheet

A text-native spreadsheet: what Markdown is to a word processor, Omasheet is
to a spreadsheet. A sheet is a plain-text `.omx` file of named, typed tables,
with formulas in OMX, a small array language with exact arithmetic.

```omx
const TaxRate = 20%

table Sales

Month | Revenue | Cost
Jan   | 10000   | 6000
Feb   | 12000   | 7000
Mar   | 15000   | 8000

Profit  := Revenue - Cost
Tax     := Profit * TaxRate
Running := [0..*; Revenue].sum()
```

## Run

```sh
cargo run -p omasheet-ui -- examples/budget.omx      # the app (needs Qt 6)
cargo run -p omasheet-cli -- examples/budget.omx     # print the calculated sheet
cargo run -p omasheet-cli -- eval '1/3 + 1/6'        # 0.5, exactly
```

## Docs

- [The sheet format](docs/format.md)
- [Expressions](docs/expressions.md)
- [The app and the command line](docs/app.md)
- [`examples/`](examples) — worked sheets, each with its expected output
- [`openspec/`](openspec) — the design and the specs

Early and changing. MIT licence.
