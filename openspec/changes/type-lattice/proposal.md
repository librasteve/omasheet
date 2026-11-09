# Proposal: the type lattice

## Why

The number types are a family tree, and the rules for what two of them make
were written one type at a time. `Fraction` ranked above `Decimal`, so a
column of `1/2` and `0.5` was a `Fraction` one, a fraction times 2 was still a
fraction, and a power with a fractional exponent was an error. The owner's
type specification (2026-11-03) states the whole lattice in three tables
instead.

## What Changes

- **numerics**: MODIFIED. `Integer`, `Decimal` and `Float` are each more
  infectious than the one before. `Hex`, `Octal` and `Binary` are `Integer`s
  shown another way, and `Fraction` and `Percent` are `Decimal`s shown
  another way. Two of one type give that type; two different members of a
  family, or one with its parent, give the parent. Arithmetic returns the
  join of its two sides, so `Fraction + Fraction` is a `Fraction`, and
  `Fraction * Integer` and `Fraction + Percent` are `Decimal`.
- **numerics**: MODIFIED. A power is exact only with a whole exponent; with
  any other (`2 ** 0.5`) it is a `Float`, as a root is. It was an error.
- **file-format**: MODIFIED. A column with no type whose cells are all
  written with a `/` is `Fraction`; with any other number among them it is
  `Decimal`. A cell is stored as it was entered, a number literal or the text
  of a formula (this was already so, and is now the rule).

## Impact

- `omasheet-omx`: `S::wider` and `rank`, and the result type of arithmetic in
  `check.rs`
- `omasheet-engine`: `arith` in `value.rs` gives a `Fraction` or a `Percent`
  only where both sides are one, and a power with a non-whole exponent is a
  `Float`
- `docs/format.md`, `docs/expressions.md`, the skill, `examples/types.omx`
- A file whose `Fraction` column is worked out with `* 2` and the like now
  shows a `Decimal`; declare the column `Fraction` to see a fraction

## Out of Scope

- Remembering the form a cell was written in (`1e0` against `1`): the file
  keeps the entered text and the type shows it, nothing more
- A type that joins a number with text, which stays `Any`
