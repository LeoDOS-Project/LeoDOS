# Rice

Lossless compression for sensor data. Uses adaptive entropy coding
that is efficient for data with exponential-like distributions
(e.g. prediction residuals from a linear predictor). The encoder
splits each sample into a most-significant and least-significant
part, encoding the former with a unary code and the latter with a
fixed-length binary code. This is optimal when the prediction
residuals follow a geometric distribution.
