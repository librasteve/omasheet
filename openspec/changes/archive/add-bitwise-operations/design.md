# Design: Bitwise Operations

## Context

OMX has `and`, `or` and `not` for Booleans, and since `add-radix-integers`
whole numbers can be written and shown in hex, octal and binary. Bitwise
operations are rare beside Boolean logic, so they do not take the bare
words. The C symbols are not free: `|` divides a table's cells, `|>` is the
pipe, and `^` starts a range, `^5`. Raku's numeric bitwise operators, `+`
followed by the C symbol, say that they work on numbers and clash with
nothing in OMX but the `|` of a table, which is dealt with below.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D133 | Decided | Bitwise operations are spelled as Raku spells them: `+&`, `+\|` and `+^` between two whole numbers, `+^` before one for its complement; `and`, `or` and `not` stay Boolean | Owner, 2026-10-30, replacing the bare words first proposed. No new reserved word |
| D134 | Decided | `+&`, `+\|` and `+^` of two whole numbers keep their base where the two share one, and are an `Integer` otherwise; the complement keeps its operand's base, and a shift the base of the number it shifts | Owner, 2026-10-30. As arithmetic does (D128), so `0xF0 +& 0x3C` is `0x30`. The shifts were first written into the same rule; a shift's right side is a count, so `0xff +> 4` is `0xf` |
| D135 | Decided | The complement of a whole number is its 64-bit complement, `-n - 1`, shown as D126 shows a negative: `+^0xff` is `0xffffffffffffff00` | Owner, 2026-10-30. What Polars, Raku and every two's complement machine give. The other way, flipping only the digits written, would need each value to keep its written width |
| D136 | Decided | Shifts are `+<` and `+>`, as in Raku | Owner, 2026-10-30, with D133, replacing `<<` and `>>` |
| D137 | Decided | `+>` of a negative number fills with its sign, so `x +> n` is `x` divided by `2 ** n` and rounded down; a shift of less than 0 or more than 63 is an error, and a `+<` that leaves 64 bits is too large, as a product is | Owner, 2026-10-30. As Raku, Python and Rust's `i64` do |
| D138 | Decided | The bitwise operators take whole numbers only: a Boolean or any other number beside one is an error, not a coercion | Owner, 2026-10-30. `true +& 6` is more likely a mistake than a mask |
| D139 | Decided | The operators bind as Raku binds them: `+&`, `+<` and `+>` as `*` does, `+\|` and `+^` as `+` does, and the prefix `+^` as `-` does | Owner, 2026-10-30. So a flag test needs no brackets, `Flags +& 0b100 == 0b100` |
| D140 | Decided | In a table, `+\|` in a cell that starts with `=` stays in the cell, as `\|>` does; in any other cell `\|` divides as now | Owner, 2026-10-30. A compact text cell such as `A+\|B` still splits |

## Precedence

From loosest to tightest, with the new operators in bold:

| Level | Operators |
|-------|-----------|
| | `\|>`, `or`, `and`, `not`, `//`, comparisons, ranges, as now |
| additive | `+`, `-`, **`+\|`**, **`+^`** |
| multiplicative | `*`, `/`, **`+&`**, **`+<`**, **`+>`** |
| unary | `-`, **prefix `+^`** |
| | `**` and tighter, as now |

## Reading

Each operator is one token, `+` and the next character with no space
between: `a + ^5` is a sum and a range, `a +^ 5` an xor. Nothing in OMX can
follow a binary `+` with `&`, `|`, `<` or `>`, so only `+^` reads
differently from before, and only where it is written without a space.

## Lowering

Polars' `and`, `or`, `xor` and `not` on `Int64` are bitwise and lower as
they are. A shift is refused and left to the row path, as a conversion is,
which is what says where a shift goes too far (D137).

## Risks

- **Excel's BIT functions take 0 to 2^48 - 1.** A formula cannot say
  beforehand that every row's numbers are among them, so an XLSX Workbook
  export writes the values of a bitwise formula, with a note, rather than a
  `BITAND` that shows `#NUM!` on some rows.
- **`+^5` written to mean `+ ^5`.** A sum of a number and a range is a type
  error today, so no formula that works changes meaning.
