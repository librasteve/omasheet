# Tasks: Bitwise Operations

## 1. Reading (`omx-expressions`, `file-format`)
- [x] 1.1 Lexer: `+&`, `+|`, `+^`, `+<` and `+>`, each one token
- [x] 1.2 Parser: the levels of D139, and the prefix `+^`
- [x] 1.3 Checker: whole numbers only (D138), with the base of the result
      (D134)
- [x] 1.4 Cell splitter: `+|` in a formula cell stays in the cell (D140)

## 2. Calculating (`numerics`)
- [x] 2.1 Row path: the four operations and the two shifts, with the shift
      errors of D137
- [x] 2.2 Lowering to Polars, or refusal for the shifts
- [x] 2.3 The complement shown as two's complement in a based column (D135)
- [x] 2.4 `docs/expressions.md`, the operators and their precedence;
      `examples/bitwise.omx` with `.expected`

## 3. Export (`xlsx-interop`)
- [x] 3.1 The values of a bitwise formula, with a note, in place of
      `BITAND` and the rest, which take only 0 to 2^48 - 1
