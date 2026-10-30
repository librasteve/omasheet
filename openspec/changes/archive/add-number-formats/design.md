# Design: Number formats

## Context

`format_value` (`omasheet-engine/src/value.rs`) shows every number by the
adaptive rules: `WIDTH = 23`, plain from `1e-15` to `1e15`, grouping from
five digits, `…` past the width. The locale's point and separator come from
`Style`. This change lets a column replace those rules with a `printf`
conversion. D/P/Q numbers continue the project's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D97 | Decided | A format is a C `printf` conversion without its `%`, after the type in brackets: `Integer['d]` | Owner, 2026-10-30 |
| D98 | Decided | Only `Integer`, `Decimal` and `Float` columns take a format | Owner |
| D99 | Decided | A format follows the locale: its point is the locale's point, and the `'` flag groups by the locale's separator | Owner |
| D100 | Decided | Where a column has a format, the adaptive rules are bypassed: no 23-character limit, no switch to e notation, no `…` | Owner |
| D101 | Decided | A format in a hand-typed `.omx` file is checked as the file is read, and one that does not read is an error at the bracket | Owner |
| D102 | Decided | The column heading in the app shows the format, without its `%`, after the type | Owner |
| D103 | Decided | An `Integer` column takes `d i x X o b B`; a `Decimal` or `Float` column takes `f F e E g G`; any other is an error | Owner, Q13 |
| D104 | Decided | An `Integer` and a `Decimal` are shown by omasheet's own renderer on the exact value, not through `f64` | Owner, Q14 |
| D105 | Decided | The cell editor shows the value in full, as now, not the formatted text | Owner, Q15 |
| D106 | Decided | `print` and Markdown `{{ }}` show a number by its column's format, in the locale; XLSX, CSV and JSON exports are unchanged | Owner, Q16 |
| D108 | Decided | `x X o b B` show a negative `Integer` as its 64-bit two's complement, as C does | Owner |
| D109 | Decided | An `Integer` takes binary, `b` and `B`, as C23 has it | Owner |
| D107 | Decided | A format can be set from the app as well as typed in the file; any dialog text is agreed with the owner first | Owner, Q17 |

## The grammar

```
format     = flags* width? ("." precision?)? conversion
flags      = "+" | "-" | " " | "0" | "'" | "#"
width      = digit+
precision  = digit*
conversion = "d" | "i" | "x" | "X" | "o" | "b" | "B"   (Integer)
           | "f" | "F" | "e" | "E" | "g" | "G"      (Decimal, Float)
```

Read as C reads it: `-` left-justifies, `0` pads with zeros and is ignored
with `-`, `+` signs every number, ` ` puts a space where there is no sign,
`#` keeps the point, or prefixes `0x`, `0X`, `0`, `0b` or `0B` to hex, octal and binary, `'` groups the whole part. A width is a minimum, never
a cut. `*` widths, length modifiers (`l`, `ll`, `h`) and positional `n$`
arguments are refused: there is one value and its type is the column's.

## Checking

The format is parsed by omasheet's own reader of the grammar above, not by
handing it to `snprintf`: a C library takes `%n`, `%s` and the rest as
undefined behaviour or a crash rather than an error. The tests show the
same value through the reader and through `libc::snprintf` and require the
same text, so "valid" means what C means.

## Showing

A `Float` is shown by the conversion directly. An `Integer` and a `Decimal`
are exact, and an `f64` cannot hold every one of their digits, so they are
shown by omasheet's own renderer of the conversion on the exact value,
rounding half to even as C does (D104). A negative `Integer` under
`x X o b B` is shown as its 64-bit two's complement, as C shows an
`int64_t`: `-255` in hex is `ffffffffffffff01` (D108).

## Open Questions

None. Q13–Q17 are settled as D103–D107.

## Risks

- **`'` in a C library depends on `LC_NUMERIC`.** Grouping is done by
  omasheet from `Style`, never by the C library, so the shown text does not
  depend on which locale the process last set.
