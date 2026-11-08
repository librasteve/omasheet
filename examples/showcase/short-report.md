# Fruit report

Here are the fruit sales for {{ today() }}.

```omx
sheet Fruit

| Item   | Price | Qty | Total |
| ------ | ----- | --- | ----- |
| Apple  | 0.5   | 5   | *     |
| Pear   | 0.75  | 3   | *     |

Total := Price * Qty
```

Altogether that comes to ${{ Fruit.Total.sum() }} of sales, across {{ Fruit.Item.count() }} kinds of fruit.
