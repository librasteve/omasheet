# Proposal: Angles (Phase 4, stage 3)

## Why

An angle is not a plain number: a sheet of bearings, slopes and sights needs `45°30′30″`, a way to add `90°` to `0.5rad`, and a sine that knows what it is given. Stages 1 and 2 made a unit part of a value; this stage gives the angle a dimension of its own, so a frequency in `Hz` and a turning speed in `rad.s^-1` are not mixed up, and `sin(5m)` is an error.

## What Changes

- **units** — ADDED: the radian `rad` (the base unit of the angle dimension), the steradian `sr` (a radian squared), the degree `°`, the minute `′` and the second `″` of arc (live, not prefixable), the lumen `lm` and the lux `lx`; the quantity types `Angle`, `SolidAngle`, `AngularSpeed`, `LuminousFlux` and `Illuminance`; the menu groups `Angle` and `Light`
- **expressions** — ADDED: the literal `45°30′30″` (alternates `º`, `'`, `"`), `sin`, `cos` and `tan` of an angle, and `dms`, an angle written out as degrees, minutes and seconds

## Impact

- `siquant` reads and writes the form (`Quantity::from_dms`, `to_dms`); the lexer finds it after a number whose degree sign is followed by a digit, so `45°` and `20°C` are as they were
- A quantity in its own unit is no longer rounded to 15 digits
- `asin`, `acos` and `atan` still give a plain number of radians

## Out of Scope

- A DMS display format for a column, the inverse trigonometric functions returning an angle, and compass bearings
