# Tasks: Parquet and dataframe interop (later phase)

## 0. Settle the open questions
- [ ] 0.1 Decide what, if anything, Polars integration means beyond Parquet
- [ ] 0.2 Decide whether the `parquet` feature is on by default, and which codecs

## 1. Parquet (`parquet-interop`)
- [ ] 1.1 `parquet` Cargo feature in `omasheet-interop`; CI builds with and without it
- [ ] 1.2 Export: choose each column's type from its values, exact decimals for terminating `Ratio`
- [ ] 1.3 Import: column types from the schema, `zone` line from timestamps
- [ ] 1.4 Round-trip test over the `examples/` corpus

## 2. CLI (`cli`)
- [ ] 2.1 `omasheet import` for `.parquet`
- [ ] 2.2 `omasheet export --parquet`
