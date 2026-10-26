# Tasks: The Polars dtypes become the representation

The first of two stages. Both changes are on the `polars` branch:
`add-dataframe-interop`, the second, is built on this one. Sections 1 to 4 are
what the Parquet boundary needed, and are done; the rest goes on here while
that change goes on beside it.

## 0. Settle the open questions

- [x] 0.1 P25: does a result past `i64` fail, or widen to `Decimal`? `2 ** 64`
      and the `huge` row of `examples/types.omx` both depend on the answer
- [x] 0.2 What `--exact` writes for a value rounded at 38 digits, now that
      there is no `1/3` to fall back on

## 1. The decimal kernel (`numerics`)

- [x] 1.1 `Dec` in `omasheet-omx/src/decimal.rs`: an `i128` of unscaled digits
      and a scale, 38 digits wide, with `checked_add`, `checked_sub`,
      `checked_mul`, `checked_div`, `powi`, `rescale`, ordering, `parse` and a
      plain-string display
- [x] 1.2 Exact for adding, subtracting and multiplying; division rounds half
      away from zero at the widest scale the result will hold
- [x] 1.3 Unit tests, including `0.1 + 0.2`, `1/3 + 1/6`, the `(1/3) * 3` loss,
      and the 38-digit ceiling
- [x] 1.4 `Dec::from_f64`, replacing `convert::fraction_of`
- [x] 1.5 Scientific-notation display, and the cut-and-marked form for a value
      too wide to show plainly; a scale at the 38-digit limit stands in for
      "does not end", which is what `convert::is_decimal` decided before

## 2. Drop what has no dtype (`numerics`, `omx-expressions`)

- [x] 2.1 `Complex`: the type, imaginary literals in the lexer, `re`, `im`,
      `conj`, `arg`, and `Func::Complex`
- [x] 2.2 `Fraction`: the type and the mixed-number display, keeping `1/7` and
      `2 3/4` as literal syntax read as exact `Decimal`s. Reversed by section
      9: the type is back as a display of a `Decimal`
- [x] 2.3 The `Complexes` and `Fractions` sheets of `examples/display.omx` and
      the `Mixes` sheet of `examples/types.omx`. `Fractions` and `Mixes` are
      back, with section 9

## 3. The names (`file-format`)

- [x] 3.1 `Bool` → `Boolean`, `Text` → `String`, `Num` → `Float` across `S`,
      `Value`, `Lit`, `Tok`, `S::from_name`, the `Func::To` conversions and the
      function directory
- [x] 3.2 `Value::Empty` → `Value::Null`
- [x] 3.3 Every schema line in `examples/`, the type lists in diagnostics and
      `entry_hint`, and the four documentation files

## 4. The representation (`numerics`)

- [x] 4.1 `Lit` and `Tok`: `Int(i64)`, `Decimal(Dec)`, `Percent(Dec)`
- [x] 4.2 `Value`: `Int(i64)`, `Decimal(Dec)`, `Percent(Dec)`
- [x] 4.3 `value.rs` arithmetic, comparison and equality on `Dec`; overflow
      becomes a diagnostic
- [x] 4.4 The display stack — `format_rat`, `shown_decimal`, `cut_decimal`,
      `scientific`, `power_of_ten`, `magnitude`, `long_decimal`, `grouped` —
      rebuilt on `Dec`. `convert::is_decimal` goes: every `Dec` terminates
- [x] 4.5 `lexer.rs number()` and `convert.rs` produce `i64` and `Dec`
- [x] 4.6 `check.rs` literal fitting, and the `S::shift` and `wider` rules
- [x] 4.7 `omasheet-interop`: the `xlsx` number path and `xlsx/write.rs exact`
- [x] 4.8 Remove `num-bigint`, `num-rational` and `num-traits` from
      `Cargo.toml`
- [x] 4.9 Regenerate every `examples/*.expected`; rewrite the `huge` row and
      the prose of `examples/types.omx`

## 5. The new types (`numerics`, `file-format`)

- [ ] 5.1 `Binary`, over `Rc<[u8]>`
- [ ] 5.2 `Duration`, and `S::shift` giving it for the difference of two
      dates or times, with `Date ± Duration → Date`
- [ ] 5.2a The ISO 8601 literal (D22, amended): `PT30M`, `PT1H30M`, `P1DT6H`,
      `PT0.5S`, `P2W`, and `-PT30M` for a negative one. Fixed-length units
      only -- `W`, `D`, `H`, `M`, `S` -- with `Y` and a calendar `M` a
      diagnostic, since a `Duration` holds a count of time units and nothing
      else. The lexer must not take `P1DT6H` for a name, and the `-` must not
      be read as subtraction
- [ ] 5.2b The shown form, the same ISO 8601 the literal is written in, so a
      computed duration round-trips through the file. A selectable display
      format is later work and out of this change
- [ ] 5.3 `DateTime` subsumes `Zoned`: one type carrying an optional zone,
      with `convert_time_zone`, `utc`, `local`, `base_utc_offset` and
      `time_zone` returning it
- [ ] 5.3a A column may **declare** its zone, which today only a computed
      column can have (owner, 2026-10-26): `Logged : Datetime("Europe/London")`,
      mirroring how Polars prints the dtype, and unambiguous because `Time`
      and `Duration` take no argument. The rejected alternative was a clause,
      `Datetime zone Europe/London`, which reads better in a schema block but
      adds a keyword.

      The zone lives in the schema and is not repeated per cell: a cell holds
      the wall clock of that column's zone, as `Lit::DateTime(z.wall)` already
      writes one. What is typed into the column is taken in the column's zone,
      so a cell never shifts by being re-entered -- the rule the file's own
      zone already follows.

      It is shown with its offset, as a converted value is today
      (`value.rs:892`, the wall date-time then `format_offset`), and the
      offset follows the zone's own daylight saving: `+00:00` in March and
      `+01:00` in July for `Europe/London`. A column in the file's own zone
      shows none. Comparison and subtraction stay on the instant, so a
      declared-zone column and a plain one holding the same instant are equal.

      `file.rs`'s `is_zone_name` checks only the shape of the name, so a zone
      the engine cannot find is a diagnostic at check time, as a `zone`
      declaration's is
- [ ] 5.4 Unsigned dtypes read and preserved, with a notice for a `UInt64`
      above `i64::MAX`

Each of these is then carried through Parquet, as task 2.9 of
`add-dataframe-interop`.

## 6. Column lists (`omx-expressions`)

- [ ] 6.1 Promote `Ty::Vector` to a nameable column list, and let `ColSel::Many`
      carry one, so the Polars operations that take several columns can be
      expressed

## 7. The documents

- [x] 7.1 `docs/format.md`: the `Null` line reverses — there *is* a null now
- [x] 7.2 `docs/expressions.md` and `SKILL.md`: the advice that a fraction in a
      result is correct and should not be "fixed" no longer holds
- [x] 7.3 `README.md`: "exact arithmetic as a semantic guarantee" and "their
      exact decimal or rational meaning" both need rewording
- [ ] 7.4 `openspec/project.md`: Design Principle 3, the Tech Stack numerics
      and Polars lines, and the Roadmap
- [ ] 7.5 Mark D11 superseded in `add-sheet-operations/design.md`
- [ ] 7.6 Version bump, and a note that the change is breaking

## 8. Verification

- [x] 8.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace --no-fail-fast`
- [x] 8.2 Tests pinning the new losses, so they are deliberate: `(1/3) * 3`,
      `1/7 * 7`, `Date - Date`, and whatever 0.1 settles for `2 ** 64`
- [ ] 8.3 Property-test the scalar path against a one-row Polars series for
      each dtype pair, once `add-dataframe-interop` brings the crate in

## 9. Guard digits, and `Fraction` as a display (D37, D38)

Built 2026-10-21, after sections 1 to 4. Both are display-and-comparison
work over the representation that is already there: no dtype changes, so
nothing here touches what `add-dataframe-interop` lowers to Polars.

- [x] 9.1 `GUARD_DIGITS`, `SHOWN_PRECISION`, `Dec::precision` and
      `Dec::guarded` in `decimal.rs`; `is_long` becomes "holds more digits
      than it shows"
- [x] 9.2 `cmp_dec` compares guarded values, so `Eq` and `Ord` — and every
      sort, group and lookup over them — agree
- [x] 9.3 `format_rat`, `shown_decimal`, `shown_percent` and `long_decimal`
      show guarded values, so what is written, shown and compared is one
      number. A literal of more than 30 digits is shown at 30
- [x] 9.4 `Dec::to_fraction`: the convergents of the continued fraction,
      stopping at `MAX_DENOMINATOR`
- [x] 9.5 `S::Fraction` and `Value::Fraction`, with `wider`, `rank`,
      `is_exact`, `assignable_to` and `converts_to` as they were before D17
- [x] 9.6 `format_fraction` and `format_mixed`: `n/d` in full, `2 3/4`
      shown, falling back to the decimal where no fraction is found
- [x] 9.7 The column coercions: a literal cell and a `:=` result in a
      `Fraction` column become fractions, and a fraction in a `Decimal`
      column becomes a decimal
- [x] 9.8 `x.Fraction` in the function directory, `Fraction` in the schema
      type list, and the entry hint for a `Fraction` cell
- [x] 9.9 The interop writers: a `Fraction` goes to CSV, JSON and XLSX as
      the number it holds, never as `1/7`
- [x] 9.10 Tests: the identities (`2/7 + 3/7 == 5/7`, `1/7 + 1/7 == 2/7`,
      `1/3 * 3 == 1`), that `1e-38` survives guarding, fraction recovery
      including the denominator limit, and the type end to end
- [x] 9.11 The documents: `format.md`, `expressions.md`, `interop.md`,
      `SKILL.md`, `README.md`, and the `Fractions` and `Mixes` example sheets
- [ ] 9.12 Once `add-dataframe-interop` section 4 lands: a `group_by`, join
      or `unique` pushed to Polars must round to `SHOWN_PRECISION` first, or
      two cells that compare equal here land in different groups there
