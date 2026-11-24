---
title: More plots
files: [plots.omx]
---

# Some plots

## Line, at numbers

A line is drawn at its `x` where those are numbers, here the time.

{{ Damped }}

## Complex numbers

A complex number is plotted by its real and imaginary parts, here as a spiral on the complex plane.

{{ Argand }}

## Pence per kWh

A price for each unit of energy is kept in pounds and shown in pence with `$p`; the plot reads `p⋅kWh⁻¹` on its axis and in its labels.

{{ UnitRate }}

## Candle for uncertainty

A body of one standard deviation and a wick of two.

{{ Labs }}

## Scatter with error bars

Each value with an uncertainty, `98.2(15)`, has an error bar of one standard deviation.

{{ Ohm }}

## One log axis

A sample that halves every ten minutes falls on a straight line when only `y` is logarithmic. `fit` draws the line and writes it: the first number is the count at the start, the second what it is multiplied by each minute.

{{ HalfLife }}

## Both log axes

A power law is a straight line on log-log axes, and the power is the slope of the fit: a year grows as the distance to the power 1.5.

{{ Kepler }}

## Area

A line with the area under it filled.

{{ Season }}

## Stacked bars

The columns, one on another.

{{ Mix }}

## Step

The value holds until the next row.

{{ Day }}

## Histogram

Counts in bins shared by the columns; `bins:` sets how many.

{{ Distribution }}

## Box

Quartiles, whiskers, and the outliers beyond them.

{{ Groups }}

## Heatmap

A cell for each value, as dark as it is large.

{{ Intensity }}

## Waterfall

Each change starts where the last ended.

{{ Bridge }}

## Bubble

A point for each row, as large as the second column.

{{ Orbits }}

## Donut

A pie with a hole.

{{ Atmosphere }}
