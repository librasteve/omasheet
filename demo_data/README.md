# demo_data

Big files for trying how Omasheet opens big data, and the `.omx` files that
open them. The data files are not in Git (see `.gitignore`): they are kept as
assets of the [`demo-data`](https://github.com/librasteve/omasheet/releases/tag/demo-data)
release, and `./fetch.sh` downloads them into this folder.

```sh
demo_data/fetch.sh
omasheet demo_data/short.omx
```

| File | Size | Rows | Source |
|---|---:|---:|---|
| `yellow_tripdata_2026-01.parquet` | 64 MB | 3,724,889, 20 columns | NYC TLC yellow taxi trips, January 2026 |
| `fhvhv_tripdata_2019-03.parquet` | 611 MB | 23,864,598, 24 columns | NYC TLC ride-hail trips, March 2019 (the largest month TLC publishes) |
| `fhvhv_tripdata_2021-06.csv` | 921 MB (176 MB gzipped in the release) | 14,961,892 + header, 7 columns | NYC TLC ride-hail trips, June 2021, mirrored by DataTalksClub |

| `.omx` | Opens |
|---|---|
| `short.omx` | the ride-hail Parquet, as a table |
| `short2.omx` | the yellow taxi Parquet, as a table |

The CSV is opened directly, as a table read through a Parquet copy in the
cache: `omasheet demo_data/fhvhv_tripdata_2021-06.csv`.

### Synthetic CSVs

`open-a-big-csv` is measured against seeded synthetic files of `id,date,region,product,qty,price,amount`, made by `./generate.sh` (10 MB, 100 MB and 1.1 GB; each is a prefix of the next, and a rerun is byte-identical). `./generate.sh json` also makes the same rows as JSON lists of objects (23 MB, 233 MB and 2.4 GB, for the same row counts), which open as tables in the same way, a sheet named by `file.json#Sheet` where a book has several. `./generate.sh xlsx` makes them as one sheet of a workbook (8 MB for 211,000 rows, and 42 MB for the 1,048,575 a sheet can hold), read by a `dataframe['synthetic-100mb.xlsx#Sales']` source; `-formulas.xlsx` is the same with `amount` as a formula filled down, which opens as a table with a calculated `amount`. They are not in Git. The harness is the `the_targets_on_a_synthetic_csv` test in `omasheet-engine/tests/timings.rs`; run `sudo -v` first if you want the cold-cache rows.

The original sources, should the release be gone:

```sh
cd demo_data
curl -L -o yellow_tripdata_2026-01.parquet https://d37ci6vzurychx.cloudfront.net/trip-data/yellow_tripdata_2026-01.parquet
curl -L -o fhvhv_tripdata_2019-03.parquet https://d37ci6vzurychx.cloudfront.net/trip-data/fhvhv_tripdata_2019-03.parquet
curl -L https://github.com/DataTalksClub/nyc-tlc-data/releases/download/fhvhv/fhvhv_tripdata_2021-06.csv.gz | gunzip -c > fhvhv_tripdata_2021-06.csv
```

No public tabular Parquet file found is a single ~1 GB file: TLC months top
out at ~611 MB, and Hugging Face shards are 100–300 MB, or text-heavy
(FineWeb-Edu `sample/10BT/000_00000.parquet` is 2.15 GB).
