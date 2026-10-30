# Design: Date and time formats

## Context

A `Date`, `Time` or `Datetime` is shown by `Style` (`omasheet-omx/src/date.rs`):
the locale's order and separator, its clock, ISO where the style is ISO.
`Style::from_strftime` already reads the locale's own `strftime` formats to
find these. This change lets a column replace the locale's form with a
`strftime` format of its own, as `add-number-formats` did for numbers with
`printf` (D97–D109). D/P/Q numbers continue the project's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D110 | Decided | A format is a C `strftime` format after the type in brackets: `Date[%d %b %Y]` | Owner, 2026-10-30 |
| D111 | Decided | Only `Date`, `Time` and `Datetime` columns take one, alongside D98's number types | Owner, 2026-10-30 |
| D112 | Decided | Unlike a number format, the `%` stays: a `strftime` format mixes conversions with literal text; `%]` is a literal `]` | Owner, Q18 |
| D113 | Decided | A `Date` takes the date conversions, a `Time` the time ones, a `Datetime` both and `%c %s %z %Z`; any other is an error at the bracket, as D103 | Owner, 2026-10-30 |
| D114 | Decided | Where a column has a format, the locale's form is bypassed for showing; the file stays ISO | Owner, 2026-10-30 |
| D115 | Decided | A format in a hand-typed `.omx` file is checked as the file is read, as D101 | Owner, 2026-10-30 |
| D116 | Decided | The cell editor shows and reads the locale's form, as now, not the format, as D105 | Owner, Q21 |
| D117 | Decided | `print` and `{{ }}` show by the format; XLSX, CSV and JSON exports are unchanged, as D106 | Owner, 2026-10-30 |
| D118 | Decided | `%z` and `%Z` give the file's `zone` (or the machine's) for a `Datetime`; a `Datetime` moved to another zone gives that zone | Owner, 2026-10-30 |
| D119 | Decided | The heading shows the format after the type; `Set format…` takes the three types, dialog text agreed with the owner first, as D102/D107 | Owner, 2026-10-30 |
| D120 | Decided | `%f` shows every digit of the fraction held, `%3f`, `%6f` and `%9f` that many, as `jiff` and `chrono` do | Owner, Q19 |
| D121 | Decided | The GNU flags `- _ 0 ^ #` and a width are taken, as glibc and `jiff` take them | Owner, Q20 |
| D122 | Decided | `%a %A %b %B %p` and `%c %x %X` are in the locale's language and forms, from `nl_langinfo`, as D99 | Owner, Q22 |

## The grammar

```
format     = (text | "%%" | "%]" | "%" flag? width? modifier? conversion)*
flag       = "-" | "_" | "0" | "^" | "#"          (D121)
width      = digit+
modifier   = "E" | "O"                           (read and ignored, as in the C locale)
conversion = date | time | instant | "n" | "t"
date       = "a" | "A" | "b" | "h" | "B" | "C" | "d" | "D" | "e" | "F"
           | "g" | "G" | "j" | "m" | "u" | "U" | "V" | "w" | "W" | "x" | "y" | "Y"
time       = "H" | "I" | "k" | "l" | "M" | "p" | "P" | "r" | "R" | "S" | "T" | "X" | f
instant    = "c" | "s" | "z" | "Z"                (Datetime only, D113)
f          = ("3" | "6" | "9")? "f"               (fraction of a second, D120)
```

A bare `]` closes the bracket; `%]` is a `]` in the shown text (D112).

## Checking

As with numbers, the format is read by omasheet's own reader of the grammar,
not handed to the C library, which takes an unknown conversion as undefined
behaviour. Tests show the same value through the reader and through
`libc::strftime` in the C locale and require the same text, except for the
conversions C does not have (`%f`).

## Showing

Rendered by omasheet from the exact value: the day count of a `Date`, the
nanoseconds of a `Time`, the microseconds of a `Datetime`. `jiff` (already a
dependency) has a `strftime` renderer, but its names are English only;
D122 wants the locale's, so the names and `%c %x %X` come from
`nl_langinfo`.

## Open Questions

None. Q18–Q22 are settled as D112, D120, D121, D116 and D122.

## Risks

- **Names in the locale tie the shown text to the machine.** As with
  grouping (D99), what is shown may differ between machines; the file
  never does.
