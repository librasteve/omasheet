# Design: Hex, Octal and Binary

## Context

`Fraction` and `Percent` are exact numbers shown in their own way: they are
assignable to and from `Decimal`, and `Value::Percent` keeps the way a value
is shown. `Url`, `Email` and `Uuid` (`bdd4323`) did the same for text. This
change does the same for `Integer`, with one difference: the prefix is
unambiguous syntax, so an inferred `Hex` is written into the file, as an
inferred `Percent` is, and not left as a suggestion in the header, as `Url`
is. D/P/Q numbers continue the project's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D123 | Decided | `Hex`, `Octal` and `Binary` are subtypes of `Integer`, as `Percent` is of the exact numbers | Owner, 2026-10-30 |
| D124 | Decided | `0x`, `0o` and `0b` prefixed integers are literals in formulas and in cells | Owner |
| D125 | Decided | The app infers the type from prefixed values and writes it into the schema line | Owner |
| D126 | Decided | A negative value is shown as its 64-bit two's complement, as the `x`, `o` and `b` formats show it (D108): `-255` is `0xffffffffffffff01` | Owner, 2026-10-30, reversing sign-then-prefix |
| D132 | Decided | The file and the cell editor write a negative one as `-0xff`, so it reads back as the same number; only display is two's complement | Owner, 2026-10-30 |
| D127 | Decided | The prefix is lower case, `0x`, `0o`, `0b`. Either case is read; hex digits are shown in lower case and read in either | Owner, 2026-10-30 |
| D128 | Decided | Assignable both ways with `Integer`, as `Decimal` and `Percent` are; `join` of two of the same base is that base, and of any other `Integer` is `Integer` | Owner, 2026-10-30 |
| D129 | Decided | A subtype column takes the `Integer` formats; a format replaces the default display, so `Hex[08X]` is `000000FF` | Owner, 2026-10-30 |
| D130 | Decided | `0o` only: a leading `0` stays decimal, `017` is 17 | Owner, 2026-10-30 |
| D131 | Decided | `_` may stand anywhere among the digits of a number, before or after the point and in a based integer, `0xFF_FF`, but not in an exponent; as the lexer already reads a decimal one | Owner, 2026-10-30 |

## Holding

`Lit::Int(i64)` becomes `Lit::Int(i64, Base)`, with `Base::Dec` for every
literal written today, and `Value::Int` the same, so `lit_type` gives the
subtype and nothing that only does arithmetic changes. In Polars all four are
`Int64`. The base is the column's, from its declared type, as `Percent`'s is.

## Reading

The lexer reads `0` followed by `x`, `o` or `b` in either case as the start of
a based integer. At least one digit of that base must follow, and a digit
outside the base is an error at that digit. A value that doesn't fit `i64` is
an error, as a decimal one is.
