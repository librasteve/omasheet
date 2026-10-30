# Tasks: Date and time formats

## 1. Reading (`file-format`)
- [x] 1.1 The `strftime` grammar and its reader, with `%]`, `%f`/`%3f`/`%6f`/`%9f` and
      the GNU flags, and errors at the bracket
- [x] 1.2 `Date[...]`, `Time[...]`, `Datetime[...]` on a schema line; `SchemaLine` holds the format
- [x] 1.3 A conversion its type does not take is an error
- [x] 1.4 Files written out keep the format; a retype keeps it where the new
      type takes it and drops it where not

## 2. Showing (`datetime`)
- [x] 2.1 Renderer of a format on a `Date`, a `Time` and a `Datetime`
- [x] 2.2 Names and `%c %x %X` in the locale's language and forms, from `nl_langinfo`
- [x] 2.3 `%z %Z` from the file's zone, and from a moved date-time's own
- [x] 2.4 `Engine::show_in` takes a column's format in place of the locale's form
- [x] 2.5 Tests against `libc::strftime` for every conversion

      glibc pads `%C`, `%G`, `%Y` and `%s` to no width of their own, and
      `-` with a width pads with spaces; both are as glibc has them
- [x] 2.6 Formats in `docs/format.md`, under Types; `examples/dateformats.omx`

## 3. The app (`app`)
- [x] 3.1 The column heading shows `Type[format]`
- [x] 3.2 `Set format…` for `Date`, `Time` and `Datetime` columns; its example and
      error text agreed with the owner 2026-10-30
- [x] 3.3 The cell editor still shows and reads the locale's form

## 4. Rendering (`cli`, `markdown-integration`)
- [x] 4.1 `print` shows a date or time by its column's format
- [x] 4.2 Markdown `{{ }}` shows a date or time by its column's format
