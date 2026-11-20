---
title: Opening a big file
files: [benchmark.omx]
---

<link rel="stylesheet" href="technical.css">

# Opening a big file

How long does it take to put the first screen of rows up? Every figure below is worked out from the sheet in [`benchmark.omx`](benchmark.omx): a CSV, two workbooks and Parquet files of two sizes, measured on a release build on 2026-11-07, against a target of {{ Target }}.

## The runs

Warm is the file as it was left, Cold is with the disk cache purged first, which was not taken for the workbooks. Change is the warm time against the cold one, and the last column says whether the cold time keeps to the target.

{{ Benchmark |> select(Run, FileSize, Rows, Warm, Cold, Change, Beats_50ms) }}

## What it shows

The CSV of {{ Benchmark.FileSize[0] }} takes {{ Benchmark.Cold[0] }} cold, over the {{ Target }} target, and {{ Benchmark.Warm[0] }} warm. A Parquet file of {{ Benchmark.FileSize[3] }} is {{ Benchmark.Cold[3] }} cold, and the smaller one of {{ Benchmark.FileSize[2] }} just {{ Benchmark.Cold[2] }}.
