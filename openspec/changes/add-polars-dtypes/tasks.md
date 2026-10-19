# Tasks: The Polars dtypes become the representation

## 0. Settle the open questions

- [ ] 0.1 P25: does a result past `i64` fail, or widen to `Decimal`? `2 ** 64`
      and the `huge` row of `examples/types.omx` both depend on the answer
- [ ] 0.2 What `--exact` writes for a value rounded at 38 digits, now that
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
- [ ] 1.4 `Dec::from_f64`, replacing `convert::fraction_of`
- [ ] 1.5 Scientific-notation display, and the cut-and-marked form for a value
      too wide to show plainly; a scale at the 38-digit limit stands in for
      "does not end", which is what `convert::is_decimal` decided before

## 2. Drop what has no dtype (`numerics`, `omx-expressions`)

- [x] 2.1 `Complex`: the type, imaginary literals in the lexer, `re`, `im`,
      `conj`, `arg`, and `Func::Complex`
- [x] 2.2 `Fraction`: the type and the mixed-number display, keeping `1/7` and
      `2 3/4` as literal syntax read as exact `Decimal`s
- [x] 2.3 The `Complexes` and `Fractions` sheets of `examples/display.omx` and
      the `Mixes` sheet of `examples/types.omx`

## 3. The names (`file-format`)

- [x] 3.1 `Bool` → `Boolean`, `Text` → `String`, `Num` → `Float` across `S`,
      `Value`, `Lit`, `Tok`, `S::from_name`, the `Func::To` conversions and the
      function directory
- [x] 3.2 `Value::Empty` → `Value::Null`
- [x] 3.3 Every schema line in `examples/`, the type lists in diagnostics and
      `entry_hint`, and the four documentation files

## 4. The representation (`numerics`)

- [ ] 4.1 `Lit` and `Tok`: `Int(i64)`, `Decimal(Dec)`, `Percent(Dec)`
- [ ] 4.2 `Value`: `Int(i64)`, `Decimal(Dec)`, `Percent(Dec)`
- [ ] 4.3 `value.rs` arithmetic, comparison and equality on `Dec`; overflow
      becomes a diagnostic
- [ ] 4.4 The display stack — `format_rat`, `shown_decimal`, `cut_decimal`,
      `scientific`, `power_of_ten`, `magnitude`, `long_decimal`, `grouped` —
      rebuilt on `Dec`. `convert::is_decimal` goes: every `Dec` terminates
- [ ] 4.5 `lexer.rs number()` and `convert.rs` produce `i64` and `Dec`
- [ ] 4.6 `check.rs` literal fitting, and the `S::shift` and `wider` rules
- [ ] 4.7 `omasheet-interop`: the `xlsx` number path and `xlsx/write.rs exact`
- [ ] 4.8 Remove `num-bigint`, `num-rational` and `num-traits` from
      `Cargo.toml`
- [ ] 4.9 Regenerate every `examples/*.expected`; rewrite the `huge` row and
      the prose of `examples/types.omx`

## 5. The new types (`numerics`, `file-format`)

- [ ] 5.1 `Binary`, over `Rc<[u8]>`
- [ ] 5.2 `Duration`, with no literal, and `S::shift` giving it for the
      difference of two dates or times, with `Date ± Duration → Date`
- [ ] 5.3 `DateTime` subsumes `Zoned`: one type carrying an optional zone, and
      `to_zone`, `utc`, `local`, `offset` and `zone` returning it
- [ ] 5.4 Unsigned dtypes read and preserved, with a notice for a `UInt64`
      above `i64::MAX`

## 6. Column lists (`omx-expressions`)

- [ ] 6.1 Promote `Ty::Vector` to a nameable column list, and let `ColSel::Many`
      carry one, so the Polars operations that take several columns can be
      expressed

## 7. The documents

- [ ] 7.1 `docs/format.md`: the `Null` line reverses — there *is* a null now
- [ ] 7.2 `docs/expressions.md` and `SKILL.md`: the advice that a fraction in a
      result is correct and should not be "fixed" no longer holds
- [ ] 7.3 `README.md`: "exact arithmetic as a semantic guarantee" and "their
      exact decimal or rational meaning" both need rewording
- [ ] 7.4 `openspec/project.md`: Design Principle 3, the Tech Stack numerics
      and Polars lines, and the Roadmap
- [ ] 7.5 Mark D11 superseded in `add-sheet-operations/design.md`
- [ ] 7.6 Version bump, and a note that the change is breaking

## 8. Verification

- [x] 8.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace --no-fail-fast`
- [ ] 8.2 Tests pinning the new losses, so they are deliberate: `(1/3) * 3`,
      `1/7 * 7`, `Date - Date`, and whatever 0.1 settles for `2 ** 64`
- [ ] 8.3 Property-test the scalar path against a one-row Polars series for
      each dtype pair, once `add-dataframe-interop` brings the crate in
