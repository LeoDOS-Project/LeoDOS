# Rice

Lossless compression for sensor data (CCSDS 121.0-B-3). Rice coding is designed for the kind of data that spacecraft sensors produce: sequences of measurements where consecutive values are close together. Instead of transmitting each measurement directly, the compressor predicts each value from its neighbors and transmits only the prediction error — the difference between the predicted and actual value.

## Why It Works

Sensor data has a property that makes it compressible: neighboring samples are correlated. A temperature reading is likely close to the previous reading. A pixel in an image is likely similar to the pixel next to it. After prediction, the errors are small numbers clustered around zero. Small numbers take fewer bits to encode than the full-range original values.

## How It Works

The compressor processes data in blocks. For each block:

1. **Preprocess** — apply a linear predictor to convert raw samples into prediction residuals (small values near zero).
2. **Select coding option** — choose the most efficient encoding for this block based on the distribution of residuals. The compressor picks from several options automatically, adapting to changing data statistics.
3. **Encode** — write the residuals using the selected code. The core technique (Golomb-Rice coding) splits each value into two parts: a quotient encoded with a variable-length prefix (small values get short codes) and a remainder encoded with a fixed number of bits.

The result is a bitstream that is typically 30–60% smaller than the raw data, with no information lost — the original samples are perfectly recoverable.

## Adaptive Block Selection

The compressor evaluates multiple coding options for each block and picks the one that produces the shortest output. This means it adapts automatically to changes in the data — quiet regions with small residuals get highly compressed, while noisy regions with large residuals are encoded less aggressively but still losslessly.

## Use in LeoDOS

Rice coding is the default compression for instrument telemetry and sensor readout data. It runs on the flight [processor](/cfs/mission/processor) within the [bounded memory model](/cfs/mission/memory) — each block is compressed independently with a fixed-size working buffer, no dynamic allocation required.
