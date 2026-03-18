# Overview

FEC (Forward Error Correction) adds redundant data so that the
receiver can detect and correct errors without requesting
retransmission. The sender computes extra _parity_ bytes from the
original data using a mathematical code, and appends them. The
receiver uses the parity to detect which bytes were corrupted and
compute their original values. The three FEC schemes offer
different trade-offs between correction capability, complexity, and
latency. Only one is used per link.

## Reed-Solomon RS(255,223) (131.0-B-5)

The CCSDS standard Reed-Solomon code operates over a Galois field
— a finite set of 256 values ($2^8$, one per byte) with specially
defined addition and multiplication that ensure every non-zero
value has an inverse. This algebraic structure is what makes the
error correction mathematics work. The code appends 32 parity
bytes to 223 data bytes, producing a 255-byte codeword. It can
correct up to 16 corrupted bytes per codeword. In Reed-Solomon
terminology, each byte is called a "symbol", and the key property
is that it does not matter how badly a byte is damaged — whether
one bit is flipped or all eight, it counts as a single symbol
error.

For burst error resilience, the standard supports interleaving
depths $I = 1$ to $5$. With interleaving, $I$ codewords are
symbol-interleaved into a single block of $I \times 255$ bytes. A
burst of errors that would overwhelm a single codeword is spread
across $I$ codewords, each of which sees only a fraction of the
damage. At depth 5 the code tolerates bursts of up to 80
consecutive corrupted bytes.

Encoding uses _systematic_ polynomial division: the original data
bytes are preserved unchanged in the output, and the 32 parity
bytes are appended at the end. (A non-systematic code would
intermix data and parity, making it harder to extract the data.)
The parity is computed by treating the data as a polynomial over
the Galois field and dividing by the generator polynomial
$g(x) = \prod_{i=0}^{31} (x - \alpha^{112+i})$; the remainder of
this division becomes the 32 parity bytes. Decoding uses the
Berlekamp--Massey algorithm to find the error locator polynomial,
Chien search to find error positions, and the Forney algorithm to
compute error magnitudes.

RS is the most widely deployed FEC in space communications.
It is computationally inexpensive, has deterministic decoding
latency, and its byte-level correction is well-suited to the
symbol-level errors that occur after demodulation.

See the [detailed Reed-Solomon page](reed-solomon) for the full
mathematical treatment and end-to-end example.

## LDPC — AR4JA (131.0-B-5)

CCSDS specifies a family of Accumulate Repeat-by-4 Jagged
Accumulate (AR4JA) LDPC codes at six code rates: 1/2, 2/3, 4/5,
and 7/8 at three information block sizes (1024, 4096, 16384 bits).
LDPC codes achieve error correction performance close to the
Shannon limit — the theoretical maximum rate at which information
can be transmitted over a noisy channel with arbitrarily low error
rate. In practice, this means LDPC can operate at lower
signal-to-noise ratios than RS for the same error rate.

Encoding multiplies the information bits by a sparse generator
matrix. Decoding uses iterative belief propagation on the
parity-check matrix: each bit node and check node exchange
messages about the likelihood of each bit being 0 or 1, refining
the estimates over multiple iterations until convergence. The
decoder accepts soft-decision input (LLRs from the demodulator),
which provides several dB (decibels — a logarithmic measure of
signal power ratio) of additional gain compared to hard-decision
decoding where each bit is simply 0 or 1 with no confidence
information.

LDPC is preferred when the link budget is tight — for example,
on long-range ISL links or during periods of high atmospheric
attenuation. The trade-off is higher computational cost and
variable decoding latency.

See the [detailed LDPC page](ldpc) for the full mathematical
treatment and end-to-end example.

## Convolutional Code (131.0-B-5)

See the [Convolutional page](convolutional) for the full
description.
