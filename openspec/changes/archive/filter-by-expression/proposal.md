# Proposal: filter by expression

## Why

A column's filter is a checklist of the texts the column shows, and making
that checklist means reading every row of the column, sorting and grouping
it, and formatting each value it has. Applying the filter does the same
work again, to compare each row's text with the texts that were unticked.
On a table of millions of rows both steps take seconds (D158 made them a
pass over groups of values rather than over rows, but each is still a full
read and a format per value). On a column of unique values such as an id
or a timestamp, the checklist runs to millions of entries that nobody could
scroll through, which is why it shows "First 1,000 of N — search to narrow".

A filter written as an expression avoids all of that. Opening the popup
reads nothing, and applying it is a single Polars pass over the column's
own values, with no text made.

While writing out the syntax, the quotes came up. OMX strings are in double
quotes, which are also what a table cell uses to keep its text as written.
Code and cell text are better told apart at a glance: from now on code uses
single quotes and a cell uses double quotes.

## What Changes

- **omx-expressions**
  - MODIFIED: a string in OMX code is written in single quotes, `'Unknown'`.
    A double-quoted string in code is an error that suggests the
    single-quoted form. Text in a table cell keeps its double quotes,
    `"2 * 3 * 4"`. Code here means everything outside a table cell: column
    formulas, consts, `table … := dataframe['…']`, `from '…'` on a column
    line, `{{ }}` and `omx` blocks in Markdown, and the filter box.
  - ADDED: comparisons chain, as in Raku: `0 < x <= 10` is
    `0 < x and x <= 10`. Today the second operator is a parse error.
  - ADDED: the full set of Polars operations that give a boolean, under
    Polars' names: `is_in`, `is_not_null`, `is_not_nan`, `is_finite`,
    `is_infinite`, `starts_with` (also with a range of single characters,
    `starts_with('A'..'C')`), `ends_with`, `contains` (literal), `matches`
    (regex), `contains_any`, `len_chars`, `to_lowercase`, `to_uppercase`,
    `is_leap_year`, `is_unique`, `is_duplicated`, `is_first_distinct` and
    `is_last_distinct`.
- **evaluation** — ADDED: a filter expression, an OMX expression in which
  `*` is the value of the filtered column's cell in each row. On a table
  it always lowers to a single Polars expression.
- **app** — ADDED: a column's filter is an expression typed in its popup,
  which is half again as wide as now and no longer lists the column's
  values.

## Impact

- `omasheet-omx`: lexer (the quotes), parser (chained comparisons), funcs
  (the four functions), check (filter mode).
- `omasheet-engine`: `lower.rs` (the string functions and `is_null`),
  `doc.rs` (`shown_rows` by expression; `distinct_shown` removed).
- `omasheet-ui`: `file.rs` (`filter_rows` takes expressions, plus
  `check_filter`; `values_matching` and its cache removed),
  `Main.qml` (the popup).
- `omasheet-interop`: Excel formulas convert to single-quoted strings.
- **BREAKING** for `.omx` files outside the repo that have double-quoted
  strings in code. They fail with a message that suggests the fix. Every
  example, test and doc in the repo is migrated by this change, and so are
  the `omasheet` skill's notes.
