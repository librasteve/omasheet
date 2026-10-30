# Tasks: Hex, Octal and Binary

## 1. Reading (`numerics`, `file-format`)
- [x] 1.1 Lexer: `0x`, `0o` and `0b` integers, with errors at the bad digit
- [x] 1.2 Lexer: `_` among the digits of a based integer (D131); a decimal
      one already takes it, and an exponent does not (`digit_separators`)
- [x] 1.3 `S::Hex`, `S::Octal` and `S::Binary`; names on the schema line;
      `join`, `assignable_to` and `converts_to`
- [x] 1.4 `Lit::Int` and `Value::Int` keep the base; `lit_type` gives the subtype
- [x] 1.5 Conversions `Hex(x)`, `Octal(x)` and `Binary(x)`
- [x] 1.6 Inference of an undeclared column writes the subtype into the file

## 2. Showing (`numerics`)
- [x] 2.1 Show by base with a lower-case prefix, and a negative one as its
      64-bit two's complement; the file and the editor write `-0x…`
- [x] 2.2 A format on a subtype column replaces the default display
- [x] 2.3 `dtype`: all three are `Int64`, and the base is from the column
- [x] 2.4 `docs/format.md`, under Types; `examples/radix.omx` with `.expected`

## 3. The app (`app`)
- [x] 3.1 A prefixed entry is read as its subtype
- [x] 3.2 A new column of prefixed values is inferred and saved as the
      subtype; the heading shows it
- [x] 3.3 The cell editor shows the value with its prefix
