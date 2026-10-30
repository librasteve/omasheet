# Tasks: Number formats

## 1. Reading (`file-format`)
- [x] 1.1 The format grammar and its reader, with errors at the bracket
- [x] 1.2 `Type[format]` on a schema line; `SchemaLine` holds the format
- [x] 1.3 A format on a type other than `Integer`, `Decimal`, `Float` is an error
- [x] 1.4 Files written out keep the format; a retype keeps it where the new
      type takes it and drops it where not

## 2. Showing (`numerics`)
- [x] 2.1 Renderer of a conversion on an exact value and on an `f64`
- [x] 2.2 The locale's point and `'` grouping from `Style`
- [x] 2.3 `Engine::show_in` takes a column's format in place of the adaptive
      rules; the app's cells use it, and a cell its format rounds is not
      marked as shown short
- [x] 2.4 `x X o b B` on an `Integer`, with `#`, and a negative one as its 64-bit two's complement
- [x] 2.5 Tests against `libc::snprintf` for every flag and conversion

      One difference is kept: glibc writes `%#.3g` of 999.5 as `1.e+03`,
      dropping the noughts `#` keeps where rounding carries into a new
      digit; the C standard has `1.00e+03`, and that is what is shown
- [x] 2.6 Formats in `docs/format.md`, under Types

## 3. The app (`app`)
- [x] 3.1 The column heading shows `Type[format]`, or `Type[]` where the
      column is too narrow; a column is always wide enough for `Type[]`
- [x] 3.2 Set a format from the app: `Set format…` in the column menu, for
      Integer, Decimal and Float columns; text agreed with the owner 2026-10-30
- [x] 3.3 The cell editor still shows the value in full

## 4. Rendering (`cli`, `markdown-integration`)
- [x] 4.1 `print` shows a number by its column's format, in the locale with
      or without `--locale` (`Options::numbers`); `examples/formats.omx`
- [x] 4.2 Markdown `{{ }}` shows a number by its column's format where it
      reads one column, a cell of it or a row's field; a placed sheet too
