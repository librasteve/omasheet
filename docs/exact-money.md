# Exact money

Money needs two things a spreadsheet often gets wrong: amounts that add up to the penny, and conversion between currencies. Omasheet has a `Decimal` for the first and a `Currency` kind of quantity for the second.

## Amounts

A `Decimal` is exact. `0.1 + 0.2 == 0.3` is true, and a column of prices adds to the same total however many rows there are. A column that holds money is declared with a currency, and a format that says how to show it:

```omx
const TaxRate = 20%

sheet Sales

Revenue : Currency[USD; $.2f]
Tax     : Currency[USD; $.2f]

| Month | Revenue | Tax |
| ----- | ------- | --- |
| Jan   | 10000   | *   |
| Feb   | 12000   | *   |

Tax := Revenue * TaxRate
```

`Currency[USD; $.2f]` says the column holds US dollars, shows two places, and puts the dollar sign in front. A bare number written in the column is read as dollars. [The file format](format.md#types) has the rest of the format letters.

## A currency is a unit

Money is a kind of quantity of its own dimension, with the US dollar `USD` as its base unit. Every currency in [currency-api](https://latest.currency-api.pages.dev/v1/currencies/usd.json), about 340 of them including the crypto ones (`BTC`, `ETH`), is a unit by its code: `5GBP`. The signs `$ € £ ¥ ￥ ₹ ₩ ฿ ₽ ₺ ₪ ₱ ₴ ₫ ₦` and `A$ C$ HK$ NZ$ S$` are one code each, written after the number, `5€`, or before it, `€5.50`. A negative amount is written `-$5`.

## Converting

A sum converts its right side to the unit of the left, and `.to` converts a value on its own:

```text
> $100 + €50
154.837USD·
> ($100).to('€')
91.180EUR·
```

The `·` says the figure is not exact: a rate is a measurement, so a converted amount is shown cut, as a `Float` is. Money mixes with other units too, and `1GiB / $5` is `0.2GiB⋅USD⁻¹`.

Strings in OMX are in single quotes, so the target is `'€'` or `'EUR'`.

## The rates

Rates change about once a day. The program has a snapshot of its own, keeps a copy of the last fetch in its cache directory, and:

```sh
omasheet rates          # what date the rates are as of, and where they came from
omasheet rates update   # fetch the latest
```

```text
rates 2026-11-19 (cached, fetched 2026-11-19T09:21:01Z)
```

The app fetches by itself while a file with money in it is open, and says `rates 2026-11-18 (today)` in its status bar, or `(cached)` or `(bundled)` for the rates of an earlier day. A file is never calculated on a fetch it did not ask for. Converted amounts depend on the rates in use, so a report that shows one should say what date they were as of.

## A column of euros

`Price : Currency[EUR]` shows and holds euros, reads a bare number in them, and shows two places, or more for a unit worth more. The format can give places, `Currency[EUR; .4f]`, and `$` in the format puts each amount down by its sign, `Currency[EUR; $]` being `€4.50`, or by its code where it has no sign, `3.00CHF`.
