# Proposal: Hex, Octal and Binary

> **Archived**: built, verified and shipped; every task is done.

## Why

A column of register values, flags or permissions is written in hex, octal or
binary, and omasheet reads only decimal. A format, `Integer[#x]`, shows the
numbers in hex, but they can't be typed that way, and nothing in the column
says what they are.

## What Changes

- **numerics**: ADDED `Hex`, `Octal` and `Binary`, three subtypes of
  `Integer`. Each holds the same numbers as `Integer` and shows them in its own
  base. `0xFF`, `0o17` and `0b101` are literals of these types, in formulas
  and in cells. A negative one is shown as its 64-bit two's complement. Conversions `Hex(x)`,
  `Octal(x)` and `Binary(x)` are added.
- **file-format**: MODIFIED. A schema line may declare `Hex`, `Octal` or `Binary`.
  When a column is undeclared, its type is inferred from its cells, so a column
  of `0x` cells is written as `: Hex`.
- **app**: MODIFIED. A value typed with a `0x`, `0o` or `0b` prefix is read as
  that subtype. A new column of such values is inferred and saved as that
  type, and its heading shows it.

## Impact

- `omasheet-omx`: the lexer reads the prefixes; `S` gains `Hex`, `Octal` and
  `Binary`; `Lit` holds which base an integer was written in; `join`,
  `assignable_to`, `converts_to` and `convert` know the new types
- `omasheet-engine`: `Value` keeps the base, `dtype` maps all three to
  `Int64`, and display is by base
- `omasheet-ui`: cell entry, inference and the column heading
- `omasheet-cli`, `omasheet-md`: shown by base, through the engine
- A file with no prefixed integers reads and shows as before

## Out of Scope

- Bases other than 2, 8 and 16
- Fractions or floats in hex (`0x1.8p3`)
- XLSX, CSV and JSON exports, which write the plain integer
