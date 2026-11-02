# Tasks: filter by expression

In order: the language first, because the filter is written in it.

## 1. Quotes (D168–D170)

- [x] 1.1 Lexer: `'…'` is a string in code, with escapes `\'` `\\` `\n`
      `\t`. `"…"` in code is an error, "strings in OMX are in single
      quotes", with the single-quoted form as its help
- [x] 1.2 A table cell's `"…"` lexes as `Tok::Text`, which only
      `parse_literal` (a cell) takes; a cell `'quoted'` stays text. Tested
- [x] 1.3 Everything that writes OMX code writes single quotes: the Excel
      formula converter (`omasheet-interop/src/formula.rs`), `sort_view`,
      `convert.rs`, the CSV and Parquet importers' `dataframe[…]` and
      `from …` lines
- [x] 1.4 Migrate `examples/*.omx`, `demo_data/*.omx`, `report.md` and
      the showcase. The `.expected` outputs must not change
- [x] 1.5 Migrate the string literals in tests across the crates
- [x] 1.6 `docs/expressions.md`, `docs/format.md`, `help.txt` and the
      `omasheet` skill: code is in single quotes, a cell in double, and
      the `'%\'.2f'` escape

## 2. The language (D166, D167)

- [x] 2.1 Chained comparisons in `Parser::comparison`, with the middle
      operand evaluated once. Tests: `0 < x <= 10`, a chain of three,
      and `a in b in c` still an error
- [x] 2.2 Turn on the Polars features of D177
- [x] 2.3 The functions of D174 in `funcs.rs`, typed by `check`, answered
      by the evaluator: text (`starts_with` with a prefix or a range,
      `ends_with`, `contains`, `matches`, `contains_any`, `len_chars`,
      `to_lowercase`, `to_uppercase`), membership (`is_in`), empty and
      float (`is_not_null`, `is_not_nan`, `is_finite`, `is_infinite`),
      `is_leap_year`, and the column functions (`is_unique`,
      `is_duplicated`, `is_first_distinct`, `is_last_distinct`)
- [x] 2.4 Lower each in `lower.rs`, and `in` (`is_in`, `is_between`), and
      the comparisons of a String or Date/Datetime column with a literal
      cast to its dtype if not already there
- [x] 2.5 `docs/expressions.md`: chains and the new functions

## 3. Filter mode (D161–D165)

- [x] 3.1 `omasheet_omx::check_filter(text, column type)`: parse, accept
      `Cursor` as a value of the column's type, and reject other names,
      indexes, sheets and anything outside D162's list, giving its
      position
- [x] 3.2 Engine: lower a filter to a Polars `Expr` over the column in
      grid order, and run it over the frame for a mask
- [x] 3.3 Evaluator path for a sheet the text holds and for a computed
      column of a table that Polars does not hold (D163)
- [x] 3.4 `Document::shown_rows(sheet, &[(name, expr)])` returns its rows
      or the first error. Remove `distinct_shown`, and `groups_by_value`
      if nothing else uses it
- [x] 3.5 Tests: the same filter on a sheet held in the text and on a
      Parquet table gives the same rows. Cover numbers, a chain, a date,
      `starts_with` with a prefix and with a range, `ends_with`,
      `contains`, empty cells (D165), a sorted table, and a type error
- [x] 3.6 Timing in `tests/timings.rs` on the 611 MB Parquet: what
      applying a filter costs, against the old list-and-apply. Release,
      `a_filter_on_a_big_table`, ms (first apply, then again). Checking is
      under 0.1 ms everywhere and opening the popup reads nothing (the old
      list took ~3 s on 3.7M rows, and applying 288 ms).
      Yellow 2026-01, 3.7M rows: comparisons and chains 10–30, `in` 35–200,
      text 20–45, `*.day == 15` 140–330.
      FHVHV 2019-03, 24M rows: comparisons 110–650 (`* != 1` keeps 23.9M
      rows, and listing them is most of it), `starts_with('B'..'C')` 135,
      `contains` 450, `matches` 1,050, a date-time comparison 700, `.day`
      1,500–2,200. That file's date-times are nanoseconds, which were not
      lowered until D180; before it `.day` took 7,700 by the row path

## 4. The app (D171, D172)

- [x] 4.1 `file.rs`: `filter_rows` takes expressions and returns `-2` on
      error; `check_filter` invokable; remove `values_matching`, the
      `distinct` cache and `json::matching`
- [x] 4.2 `Main.qml`: remove the probe timers left in the working tree.
      In the popup: 16.5 font-sizes wide, an expression field with its
      hint `e.g. * >= 3`, Clear filter, Apply (applies, stays open), OK
      and Cancel. Enter is OK, Esc cancels; an error outlines the field in
      red with the reason in its tooltip, and disables Apply and OK. Remove the checklist,
      the search box, "Show all" and the "First N of M" note
- [x] 4.3 `win.filters` holds expressions; `filtered(col)` means a
      non-empty one; a `-2` is shown in `win.notice`
- [x] 4.4 `docs/app.md`: a Filtering section with the syntax table and
      D164/D165. `help.txt`: one line
- [x] 4.5 Run the app on the taxi Parquet: the popup opens at once, and
      `* > 10`, `0 < * <= 5` and `*.starts_with('B')` filter. Done by the
      owner in the app, 2026-11-02, on short2 and the ride-hail files; what
      it turned up is in the popup now (the hint alone, Apply that lights
      when an expression passes, Clear filter that stays open) and in
      open-a-big-csv 2b.6a (filters over 200,000 rows on a worker)
