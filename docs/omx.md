# OMX: the language in brief

OMX is the expression language of Omasheet. An expression has no effects: it reads cells and constants and gives a value. This page is the grammar. What each value, type and function does is in [Expressions](expressions.md), and where an expression may be written is in [The file format](format.md).

## Where an expression is written

| Form | Meaning |
|---|---|
| `= expr` in a cell | one formula, for that cell |
| `Name := expr` | a computed column: one formula, every row |
| `sheet Name := expr` | a sheet calculated from an expression |
| `const Name = expr` | a named value |
| `fn Name(a, b) = expr` | a function of your own |
| `{{ expr }}` in Markdown | the value, in a rendered document |

## Lexical

- A comment is `#` to the end of the line. White space between tokens is free.
- A name is letters, digits and `_`, not starting with a digit. The words `if then else and or not in by` are reserved. `true`, `false`, `Inf` and `NaN` are values.
- Text is `'single quoted'` in code. `"double quoted"` is for table cells only.
- `_` may separate digits: `1_000_000`.

| Literal | Written | Type |
|---|---|---|
| whole number | `42` `0xFF` `0o17` `0b101` | `Integer`, `Hex`, `Octal`, `Binary` |
| decimal | `1.5` | `Decimal` |
| percent | `20%` | `Percent` |
| float | `1e5` `2.5e-3` | `Float` |
| fraction | `<1/10>` `<2 3/4>` | `Fraction` |
| imaginary | `4i` | `Complex` |
| uncertain | `10 ± 0.1` `10.5(3)` `7.297(11)e-3` | `Scientific` |
| quantity | `5kg` `300cm` `9.81m.s^-2` `10.5(3)e-3kg` `20°C` | `Quantity` |
| scale | `1k.` `1.5M.` `5m.` | a number: 1000, 1500000, 0.005 |
| date, time | `2025-01-31` `09:30` `09:30:15.5` | `Date`, `Time` |
| date and time | `2025-01-31T09:30` `2025-03-01T09:30+01:00` | `Datetime` |
| duration | `PT1H30M` | `Duration` |
| text | `'abc'` | `String` |
| Boolean | `true` `false` | `Boolean` |
| list | `[1, 2, 3]` | a vector |

## Operators

From the loosest to the tightest:

| Level | Operators | Notes |
|---|---|---|
| 1 | `x \|> f(args)` | left to right; `x` becomes the first argument |
| 2 | `or` | |
| 3 | `and` | |
| 4 | `not` | prefix |
| 5 | `a // b` | `a`, or `b` where `a` is empty |
| 6 | `== != < > <= >= in` | comparisons chain, as in Raku: `0 < x <= 10` |
| 7 | `a..b` `a..^b` `a^..b` `a^..^b` `^n` | ranges; `^n` is `0..^n` |
| 8 | `+ - +\| +^` | `+\|` and `+^` are bitwise or and xor |
| 9 | `* / % +& +< +>` | `+&` is bitwise and, `+<` and `+>` shift |
| 10 | `-x` `+^x` | prefix: negation, bitwise complement |
| 11 | `a ** b` | right to left: `2 ** 3 ** 2` is `2 ** 9`, and `-2 ** 2` is `-4` |
| 12 | `a ± b` | one value and one uncertainty, each a single term: `10 ± 0.1 ** 2` is `(10 ± 0.1) ** 2` |
| 13 | `x[…]` `x.name` `x.f(args)` | postfix |

`if c then a else b` is an expression of its own, and `( … )` groups.

## Names and selection

| Written | Means |
|---|---|
| `Sales` | the sheet |
| `Sales.Revenue` | the column, as a vector |
| `Revenue` | inside the sheet, this row's value |
| `TaxRate` | a constant |
| `Sales[Revenue; 0..2]` | `[columns; rows]`: a missing slot is all of it, positions count from 0 and `-1` is the last |
| `Sales[; Revenue > 100]` | the rows where a condition is true |
| `*` | this row, so `[Revenue; *-1]` is the row before, and `[…; *-1 by Region]` stays in a partition |
| `$-1` | in a sequence, the term before this one |

## Calls

`f(x, y)`, `x.f(y)` and `x |> f(y)` are the same call. A type name converts: `Integer(x)`, `x.Integer`, `x.Integer()`. The standard functions are listed under [Functions](expressions.md#functions).

## Sequences

A computed column may be written `seeds ... limit`: `1, 3 ... Inf`, `1, 2, 4 ... 100`, `0, 1, $-1 + $-2 ... Inf`. A last term that reads `$-n` is the rule, and otherwise the step is taken from the seeds. See [Sequences](expressions.md#sequences-with-).

## Types

Arithmetic between types gives the wider one, along `Integer` → `Decimal` → `Float` → `Scientific` → `Complex`. Exact numbers stay exact until a `Float` joins them, and a `Scientific` carries its uncertainty through `+ - * / **` and the math functions, treating operands as independent. An empty cell is a missing value: it passes through arithmetic as empty, `is_null(x)` tests for it and `x // y` replaces it.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
