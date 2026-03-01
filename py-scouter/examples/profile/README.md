# Profile Examples

Demonstrates computing a `DataProfile` from a DataFrame. A `DataProfile` captures per-feature statistics (mean, stddev, quantiles, histogram bins, cardinality) and is the starting point for setting up drift monitoring.

## Examples

### `pandas_dataframe.py`

Builds a 10,000-row synthetic DataFrame with four numeric features and one categorical feature, then profiles it.

```bash
cd py-scouter
uv run python examples/profile/pandas_dataframe.py
```

**What it shows:**

- Constructing a `DataProfiler`
- Calling `create_data_profile(df)` to produce a `DataProfile`
- Accessing per-feature statistics from the result

**Key classes:**

| Class | Purpose |
|-------|---------|
| `DataProfiler` | Computes statistics from a DataFrame |
| `DataProfile` | Contains per-feature statistics and histograms |

**Accepted inputs:** Pandas DataFrame, Polars DataFrame, NumPy 2D array.

## Next step

Once you have a `DataProfile`, you can use it to create a drift profile — see the [`monitor/` examples](../monitor/README.md).
