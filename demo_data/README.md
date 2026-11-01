# demo_data

Large files for testing how Omasheet opens big data. They are not in Git
(see `.gitignore`); fetch them again with the commands below.

| File | Size | Rows | Source |
|---|---:|---:|---|
| `fhvhv_tripdata_2021-06.csv` | 921 MB | 14,961,892 + header, 7 columns | NYC TLC ride-hail trips, June 2021, mirrored by DataTalksClub |
| `fhvhv_tripdata_2019-03.parquet` | 611 MB | 23,864,598, 24 columns | NYC TLC ride-hail trips, March 2019 (the largest month TLC publishes) |

```sh
cd demo_data
curl -L https://github.com/DataTalksClub/nyc-tlc-data/releases/download/fhvhv/fhvhv_tripdata_2021-06.csv.gz | gunzip -c > fhvhv_tripdata_2021-06.csv
curl -L -o fhvhv_tripdata_2019-03.parquet https://d37ci6vzurychx.cloudfront.net/trip-data/fhvhv_tripdata_2019-03.parquet
```

No public tabular Parquet file found is a single ~1 GB file: TLC months top
out at ~611 MB, and Hugging Face shards are 100–300 MB, or text-heavy
(FineWeb-Edu `sample/10BT/000_00000.parquet` is 2.15 GB).
