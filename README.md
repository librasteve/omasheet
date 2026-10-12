# Omasheet

A text-native spreadsheet: what Markdown is to a word processor, Omasheet is
to a spreadsheet. A sheet is a plain-text `.omx` file of named, typed tables,
with formulas in OMX, a small array language with exact arithmetic. Automatically 
follows system dark/light mode.

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

<img src="Screenshot.png" alt="Project screenshot">

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/librasteve/omasheet/main/install.sh | bash
```

**Update:** run the same line again.
**Uninstall:** `sudo pacman -R omasheet`.

## Run

```sh
omasheet                                # the app, on a new sheet
omasheet examples/budget.omx            # the app, on a .omx, .csv, .json or .xlsx file
omasheet print examples/budget.omx      # print the calculated sheet
omasheet eval '1/3 + 1/6'               # 0.5, exactly
omasheet export examples/budget.omx --xlsx
omasheet render examples/report.md --html
```

## AI Skill

Prompt your favorite AI with:

```text
Please load the Omasheet skill from https://raw.githubusercontent.com/librasteve/omasheet/main/.claude/skills/omasheet/SKILL.md and follow its instructions when creating, checking, or converting spreadsheets.
```

## Docs

- [The sheet format](docs/format.md)
- [Expressions](docs/expressions.md)
- [The app and the command line](docs/app.md)
- [Workbooks, CSV, JSON and Markdown](docs/interop.md)
- [`examples/`](examples) — worked sheets, each with its expected output
- [`openspec/`](openspec) — the design and the specs

Early and changing. MAY CONTAIN ERRORS.

Copyright (c) 2026 Stephen Roe. MIT licence; see [LICENSE](LICENSE).
