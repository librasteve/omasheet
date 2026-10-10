# Omasheet

A text-native spreadsheet: what Markdown is to a word processor, Omasheet is
to a spreadsheet. A sheet is a plain-text `.omx` file of named, typed tables,
with formulas in OMX, a small array language with exact arithmetic.

```omx
const TaxRate = 20%

table Sales

Month | Revenue | Cost | Profit | Tax | Running
Jan   | 10000   | 6000 | *      | *   | *
Feb   | 12000   | 7000 | *      | *   | *
Mar   | 15000   | 8000 | *      | *   | *

Profit  := Revenue - Cost
Tax     := Profit * TaxRate
Running := [Revenue; 0..*].sum()
```

## Run

```sh
cargo run -p omasheet-ui -- examples/budget.omx      # the app (needs Qt 6)
cargo run -p omasheet-cli -- examples/budget.omx     # print the calculated sheet
cargo run -p omasheet-cli -- eval '1/3 + 1/6'        # 0.5, exactly
cargo run -p omasheet-cli -- export examples/budget.omx --xlsx
cargo run -p omasheet-cli -- render examples/report.md --html
```

## Install

```sh
cargo install --git https://github.com/librasteve/omasheet omasheet-cli   # the omasheet command
```

## With Claude

Omasheet comes with a [skill](.claude/skills/omasheet/SKILL.md) that teaches
[Claude Code](https://claude.com/claude-code) to write, check and convert
sheets. In this repository it is there already. Anywhere else, ask Claude to
install it:

> Install the omasheet skill: save
> https://raw.githubusercontent.com/librasteve/omasheet/main/.claude/skills/omasheet/SKILL.md
> as ~/.claude/skills/omasheet/SKILL.md

or do it yourself:

```sh
mkdir -p ~/.claude/skills/omasheet
curl -fsSL https://raw.githubusercontent.com/librasteve/omasheet/main/.claude/skills/omasheet/SKILL.md \
  -o ~/.claude/skills/omasheet/SKILL.md
```

Start a new Claude session, and ask for a sheet: "make me a budget as an
omasheet". `/omasheet` calls the skill by name.

## Docs

- [The sheet format](docs/format.md)
- [Expressions](docs/expressions.md)
- [The app and the command line](docs/app.md)
- [Workbooks, CSV and Markdown](docs/interop.md)
- [`examples/`](examples) — worked sheets, each with its expected output
- [`openspec/`](openspec) — the design and the specs

Early and changing. MAY CONTAIN ERRORS.

Copyright (c) 2026 Stephen Roe. MIT licence; see [LICENSE](LICENSE).
