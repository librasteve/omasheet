#!/bin/sh
# Download the big demo files into demo_data/, from the `demo-data` release
# of librasteve/omasheet. A file already here is left alone.
set -eu
cd "$(dirname "$0")"
base=https://github.com/librasteve/omasheet/releases/download/demo-data
for file in \
    yellow_tripdata_2026-01.parquet \
    fhvhv_tripdata_2019-03.parquet \
    fhvhv_tripdata_2021-06.csv.gz
do
    target=${file%.gz}
    if [ -e "$target" ]; then
        echo "$target: already here"
        continue
    fi
    echo "$target: downloading"
    case $file in
        *.gz) curl -fL "$base/$file" | gunzip -c > "$target.part" ;;
        *) curl -fL -o "$target.part" "$base/$file" ;;
    esac
    mv "$target.part" "$target"
done
