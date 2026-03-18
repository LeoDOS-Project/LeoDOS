# Overview

Modulation converts bits into _symbols_ — discrete waveform
states that can be transmitted over the radio channel. Each symbol
represents one or more bits. The modulator maps groups of bits to
symbols for transmission; the demodulator on the receiving end
observes the (possibly noisy) signal and determines which symbol
was most likely sent.

The choice of modulation scheme determines two things: how many
bits each symbol carries (throughput), and how much noise the
signal can tolerate before the receiver makes errors (robustness).
More bits per symbol means higher throughput but less margin
against noise.

On real spacecraft hardware the radio performs modulation
internally. In LeoDOS these modulation schemes are implemented as
software models for two purposes: (1) simulating the channel to
test the full stack without hardware, and (2) computing
soft-decision log-likelihood ratios (LLRs) from noisy received
symbols. An LLR is a number indicating how confident the
demodulator is that a given bit is 0 or 1. Positive means "likely
1", negative means "likely 0", and the magnitude indicates
confidence. This soft information is needed as input to the LDPC
and Viterbi decoders, which use it to make better correction
decisions than they could from just the raw bit values (hard
decisions).

- [BPSK](bpsk) — 1 bit/symbol, most robust
- [QPSK](qpsk) — 2 bits/symbol, doubles throughput
- [OQPSK](oqpsk) — offset QPSK for non-linear amplifiers
- [8PSK](8psk) — 3 bits/symbol, high throughput
- [GMSK](gmsk) — constant envelope for power efficiency
