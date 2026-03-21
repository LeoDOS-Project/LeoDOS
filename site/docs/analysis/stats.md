# Statistics

Basic image statistics and histogram analysis. All functions
operate on `&[f32]` slices with no heap allocation.

## Basic Statistics

`compute` returns a `Stats` struct from a data slice:

| Field | Description |
|-------|-------------|
| `count` | Number of samples |
| `min` | Minimum value |
| `max` | Maximum value |
| `mean` | Arithmetic mean |
| `variance` | Population variance |

Accumulation uses `f64` internally to reduce floating-point
error on large arrays.

## Histogram

`histogram` bins a data slice into `n_bins` uniform bins over a
caller-specified [min, max] range. The output is a `&mut [u32]`
slice whose length determines the number of bins. Values at or
above `max` are placed in the last bin.

```rust
let mut bins = [0u32; 256];
histogram(&data, 0.0, 1.0, &mut bins);
```

## Percentiles

`percentile_from_histogram` estimates a percentile value from a
pre-computed histogram. The `percentile` parameter is in [0.0,
1.0]. It walks the cumulative distribution and returns the bin
center where the target count is reached.

```rust
let median = percentile_from_histogram(&bins, 0.0, 1.0, 0.5);
let p95 = percentile_from_histogram(&bins, 0.0, 1.0, 0.95);
```

Resolution is limited by the number of bins. For 256 bins over
[0, 1], the resolution is ~0.004.
