---
title: First quarter
sheets: [budget.omx]
---

# First quarter

Revenue for the quarter was {{ Sales.Revenue.sum() }}, and profit
{{ Sales.Profit.sum() }} after costs. The best month was
{{ Sales[; Profit == Sales.Profit.max()].Month }}, and the books closed on
{{ Sales.Closed.max().date() }}.

Tax is charged at {{ TaxRate }}, which comes to
{{ Sales.Tax.sum() }}.

## Months over target

A table can be shown where it is asked for:

{{ Sales |> filter(Profit > Target) |> select(Month, Revenue, Profit) }}

## Targets

A document can hold sheets of its own. This block is a sheet like any other,
and it reads the one named at the top:

```omx
const Target = 4500

table Targets

Month | Goal | Profit | Over | Met
Jan   | 4200 | *      | *    | *
Feb   | 4500 | *      | *    | *
Mar   | 5000 | *      | *    | *

Profit := Sales[Profit; *]
Over   := Profit - Goal
Met    := Over >= 0
```

{{ Targets[; Met].Month.count() }} of {{ Targets.Month.count() }} months met
their goal. Written in code, `{{ 1 + 1 }}` is left as it is.
