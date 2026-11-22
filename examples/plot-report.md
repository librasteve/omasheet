---
title: Plots
files: [plots.omx]
---

# Plots

A `plot` is declared in the file, as a sheet is, and `{{ Name }}` draws it where it is written.

## Line

{{ Trend }}

## Bar

{{ ByMonth }}

## Pie

{{ Share }}

## Candlestick, for uncertainty

A column with an uncertainty is drawn with its value `k` standard deviations either way in the body, and twice that in the wick.

{{ Spread }}

## In a block

A block shows its plots as well as its sheets:

```omx
sheet Top := Sales |> sort(Revenue, desc) |> head(2)

plot Best
type: bar
data: Top
x: Month
y: Revenue
```
