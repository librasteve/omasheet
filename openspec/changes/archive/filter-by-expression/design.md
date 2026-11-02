# Design: filter by expression

## Context

Source: the conversation of 2026-11-02, on the `speedup` branch, after
D158 (a filter and its list read a value per group of rows) and the commit
that keeps a filter's rows in Rust and has the grid ask for them one at a
time. D numbers continue from D159.

How the filter works now. `values_matching` → `Document::distinct_shown`
lists the texts a column shows, for the popup's checklist. `filter_rows`
→ `Document::shown_rows` hides the rows whose text is one that was
unticked. `win.filters[sheet][column]` holds those texts. Both reads cover
the whole column, and both turn values into text.

## Decisions

| | | |
|---|---|---|
| D160 | Decided | A column's filter is an OMX expression in which `*` stands for the cell's value. The checklist is removed, from every kind of sheet | Owner decision, 2026-11-02. With the checklist gone, opening the popup reads nothing. One way to filter, and one popup layout |
| D161 | Decided | `*` needs no new syntax: the parser already reads a bare `*` as `ExprKind::Cursor`, the current row (`parser.rs:404`). In a filter it means this row's cell in the filtered column, which is what the cursor means everywhere else. A filter is checked in a filter mode that accepts `Cursor` as a value and allows no other name, no sheet and no index | Found while planning. `* == 3`, `*.starts_with('A')` and `* in 0..10` all parse today |
| D162 | Decided | The filter syntax is limited to what lowers to Polars: literals, `*`, comparisons and their chains, `in` with a range or a vector, `and`/`or`/`not`, the date and time parts, and the functions of the table under *The filter functions*. The check rejects anything else, so a filter on a table always runs as one Polars `Expr` | Owner decision, 2026-11-02. A table's frame is filtered in a single vectorised pass. When the table is scanned lazily, the expression can also become the scan's predicate (not done here) |
| D163 | Decided | The OMX evaluator runs a filter in two cases only: a sheet the text holds, which is small, and a computed column of a table that Polars does not hold (where `frame_values` returns `None`). It runs on values, never on display texts | |
| D164 | Decided | A filter compares values, not what the grid shows. `* == 3` matches a cell shown as `3.00`, and `* >= 2025-01-01` compares dates | Follows from D160. The checklist matched texts because texts were what it listed |
| D165 | Decided | An empty cell fails every comparison, as a null does in Polars: `* != 3` hides empty cells too. `* == ''` and `*.is_null()` match empty cells, and `* != 3 or * == ''` keeps them | Owner decision, 2026-11-02. Lowers to the plain expression with no extra terms |
| D166 | Decided | Comparisons chain, in all of OMX: `a < b <= c` is `a < b and b <= c`. All six comparison operators chain; `in` does not. The middle operand is written twice rather than held: an expression has no effects, and in Polars it is the one column named twice | Raku does it. Today the second operator is a parse error (`comparison` reads a single operator), so no file changes meaning |
| D167 | Decided | `starts_with(text, prefix)`, `ends_with`, `contains` (a literal match, not a regex) and `is_null(x)` are ordinary OMX functions, called as methods or as functions. `starts_with('A'..'C')` takes a range of single characters and means the first character is in it. On a table it lowers to `col >= 'A' and col < 'D'` (the character after the upper bound), a plain comparison | Owner decision, 2026-11-02: `starts_with` rather than `begins_with`. The range form needs no slicing, and comparisons can use a Parquet row group's min/max statistics |
| D173 | Decided | `x.is_in(list)` is `x in list`, the operator spelled as Polars spells it, taking a vector or a range as `in` does. It lowers to `is_in`, or to `is_between` for a range | Owner request, 2026-11-02. `is_in` was already in the checker as a spelling of `in`; what is new is its lowering |
| D174 | Decided | The full set of Polars operations that give a boolean is in OMX, under Polars' own names: `is_between` (by chains and ranges), `is_in`, `is_null`/`is_not_null`, `is_nan`/`is_not_nan`, `is_finite`/`is_infinite`, `starts_with`, `ends_with`, `contains`, `matches`, `contains_any`, `len_chars`, `to_lowercase`, `to_uppercase`, `is_leap_year`, `is_unique`, `is_duplicated`, `is_first_distinct`, `is_last_distinct` | Owner decision, 2026-11-02: the full set, and Polars names, as `starts_with` and `is_in` already are |
| D175 | Decided | `contains(part)` is a literal match and `matches(re)` is the regex, which Polars calls `str.contains` | Owner decision, 2026-11-02. A `.` in a plain search does not surprise anyone |
| D176 | Decided | No `eq_missing`/`neq_missing`: `* == ''`, `*.is_null()` and `* != 3 or *.is_null()` already say it | Owner decision, 2026-11-02 |
| D177 | Decided | The Polars features `strings`, `regex`, `find_many`, `is_in`, `is_between`, `is_unique`, `is_first_distinct` and `is_last_distinct` are turned on | What D174 needs. They cost compile time and disk in `target/` |
| D178 | Decided | In a filter, `x == ''` is `x.is_null()` and `x != ''` is `not x.is_null()`, whatever the column holds, so `* == ''` asks for an empty cell of a number column without a type error. The column's own name means `*`; any other name is an error | Found by the test of D165: `* == ''` on an Int column failed to check |
| D179 | Decided | `Engine::filter_mask` lowers a filter whenever it can, however few the rows, and otherwise evaluates it a row at a time. `Document::shown_rows` ANDs the columns' masks in file order and lists the rows in the grid's order | The row threshold is for column formulas, where the engine's own path is the reference; a filter has no cells to keep |
| D180 | Decided | A Parquet `Datetime` in nanoseconds or milliseconds is lowered as the microseconds a sheet holds, as `dtype::cell` reads each value: nanoseconds floored (Polars' cast stepped down one where it went up, which needs no `round_series` feature), milliseconds cast | Found by the timing (3.6): the 2019 FHVHV file is in nanoseconds, so its date filters fell to the row path, 7.7 s for `.day` on 24M rows. Tested with a time one nanosecond before 1970, held and paged |
| D168 | Decided | Strings in OMX code are in single quotes; text in a table cell keeps double quotes. Code is everything outside a table cell: formulas, consts, `dataframe['…']`, `from '…'`, `{{ }}`, `omx` blocks and the filter box. The escapes are `\'`, `\\`, `\n` and `\t`; `\"` is no longer an escape in code, since `"` needs none there | Owner decision, 2026-11-02. Code and text look different at a glance. A cell's `"…"` is read by the file reader, not the expression lexer, so it is untouched |
| D169 | Decided | A double-quoted string in code is an error, not a warning: "strings in OMX are in single quotes", with the single-quoted text as the suggested fix. There is no transitional double form | Owner decision, 2026-11-02. The language keeps one form. Everything in the repo is migrated by this change |
| D170 | Decided | In a number format, `'` is the flag for grouping digits (`%'.2f`). Inside a single-quoted string it is written `\'`: `'%\'.2f'`. No other flag is added for it | It is the one place the new quotes collide with existing content. Rare enough that an escape is the cheaper answer |
| D171 | Decided | The popup is 16.5 font-sizes wide, half again its current 11. Under the sort entries: the expression field, prefilled with the column's filter, with the hint `e.g. * >= 3` while empty; then Clear filter, Apply, OK and Cancel. An error outlines the field in red and is said in its tooltip, not in a line of its own. Apply is enabled when the expression passes and differs from the filter in force, is then drawn filled (OK is filled otherwise), and applies it with the popup kept open; Clear filter clears and keeps it open; OK and Enter apply and close; Esc and Cancel close. Padding 8, spacing 4 | Owner decisions, 2026-11-02: the width; then, after trying it, no help text in so small a space, only the hint, and Apply beside OK |
| D172 | Decided | `win.filters[sheet][column]` holds the expression's text. The spec given to Rust is `name U+001F expr`, columns separated by U+001E. `filter_rows` returns the row count, `-1` when no filter is set, or `-2` when an expression fails, and the old filter stays in force. `check_filter(sheet, col, expr)` parses and type-checks without reading any rows, so it can run on every keystroke | |

## Lowering

The filtered column is `c = col(name)`, taken in the grid's order (the
same `orders` take that `groups_by_value` uses now).

| OMX | Polars |
|---|---|
| `* == v`, `!=`, `<`, `<=`, `>`, `>=` | `c.eq(lit(v))`, `.neq`, `.lt`, `.lt_eq`, `.gt`, `.gt_eq`, with `v` cast to the column's dtype (a date to the Datetime unit and time zone; a string to a categorical's String) |
| `a < * <= b` | `c.gt(lit(a)).and(c.lt_eq(lit(b)))` |
| `* in a..b`, `^..`, `..^` | `c.is_between(lit(a), lit(b), ClosedInterval::…)` |
| `* in [x, y]`, `*.is_in([x, y])` | `c.is_in(lit(series))` |
| `*.starts_with(s)`, `ends_with`, `contains` | `c.str().starts_with(lit(s))`, `.ends_with`, `.contains_literal` |
| `*.starts_with('A'..'C')` | `c.gt_eq(lit("A")).and(c.lt(lit("D")))` |
| `* == ''`, `*.is_null()` | `c.is_null()`, with `.or(c.eq(lit("")))` on a String column |
| `and`, `or`, `not` | `.and`, `.or`, `.not` |

The mask is evaluated over the frame, its true places become the rows
shown, and each column's mask is ANDed with the others.

## The filter functions

Every one is an ordinary OMX function, called as a method or as a function,
outside filters too. The evaluator answers each for a sheet the text holds;
the lowering is what a table runs.

| Group | OMX | Polars | Type |
|---|---|---|---|
| Compare | `* == v` `!=` `<` `<=` `>` `>=` | `eq` `neq` `lt` `lt_eq` `gt` `gt_eq` | any ordered |
| | `a <= * <= b`, `* in a..b` | `is_between(a, b, closed)` | any ordered |
| Member | `* in [..]`, `*.is_in([..])`, `*.is_in(a..b)` | `is_in`, `is_between` | any |
| Empty, float | `is_null()` `is_not_null()` | `is_null` `is_not_null` | any |
| | `is_nan()` `is_not_nan()` `is_finite()` `is_infinite()` | the same | `Float`; exact numbers are never NaN or infinite, so a constant answer |
| Text | `starts_with(s)`, `starts_with('A'..'C')` | `str.starts_with`, or `gt_eq`/`lt` | `String` |
| | `ends_with(s)` | `str.ends_with` | `String` |
| | `contains(s)` | `str.contains_literal` | `String` |
| | `matches(re)` | `str.contains(re, strict)` | `String`; a bad regex is an error when checked |
| | `contains_any([s, …])` | `str.contains_any` | `String` |
| | `len_chars()` | `str.len_chars`, cast to `Int64` | `String` → `Int` |
| | `to_lowercase()` `to_uppercase()` | `str.to_lowercase` `str.to_uppercase` | `String` → `String` |
| Dates | `year` `month` `day` `weekday` `quarter` `week` `ordinal_day` `hour` … | `dt.*` | already lowered |
| | `is_leap_year()` | `dt.is_leap_year` | `Date`, `Datetime` |
| Column | `is_unique()` `is_duplicated()` | `is_unique` `is_duplicated` | any; over the whole column |
| | `is_first_distinct()` `is_last_distinct()` | the same | any; over the whole column, in grid order |
| Logic | `and` `or` `not` | `and` `or` `not` | `Boolean` |

The four column functions ask about a value's place among all of the
column's values, so the evaluator answers them over the column rather than
a cell at a time, and the check accepts them only on `*` or a column name.

## Risks

- **The quote migration is wide.** Formulas in 7 example and demo files,
  string literals in tests across six crates, two docs, the Excel
  converter's output and anything the app writes back (`|> sort(…)`).
  The `.expected` outputs show values, not code, so they should not
  change. The examples' runs pin it: before and after must match.
- **A text cell starting with `'`.** A cell is read by the file reader, so
  `'quoted'` in a cell stays text, as it is today. A test pins it.
- **Type mismatches.** `* == 'x'` on an Int column is an error when the
  filter is checked; it must not be left for Polars to fail on during
  the pass.
