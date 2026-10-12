---
title: First quarter
files: [budget.omx]
---

# First quarter

Revenue for the quarter was {{ Sales.Revenue.sum() }}, and profit
{{ Sales.Profit.sum() }} after costs. The best month was
{{ Sales[; Profit == Sales.Profit.max()].Month }}, and the books closed on
{{ Sales.Closed.max().date() }}.

Tax is charged at {{ TaxRate }}, which comes to
{{ Sales.Tax.sum() }}.

## Months over target

A sheet can be shown where it is asked for:

{{ Sales |> filter(Profit > Target) |> select(Month, Revenue, Profit) }}

## Targets

A document can hold files of its own. This block is a file like any other,
and it reads the one named at the top:

```omx
const Target = 4500

sheet Targets

Month | Goal | Profit | Over | Met | Note
Jan   | 4200 | *      | *    | *   | a **slow** start
Feb   | 4500 | *      | *    | *   | see the [file format](../docs/format.md)
Mar   | 5000 | *      | *    | *   | the _best_ month, `Mar`

Profit := Sales[Profit; *]
Over   := Profit - Goal
Met    := Over >= 0
```

{{ Targets[; Met].Month.count() }} of {{ Targets.Month.count() }} months met
their goal.

To show the braces themselves, put them in backticks: `{{ 1 + 1 }}` is not
worked out.
