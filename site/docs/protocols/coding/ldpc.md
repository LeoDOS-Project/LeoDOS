# LDPC Error Correction

## The Problem

A satellite sends 1024 bits of telemetry to Earth. The channel is
noisy --- individual bits can flip with some probability. We want
the receiver to correct as many flipped bits as possible, without
retransmission.

Reed-Solomon (covered in [coding](coding)) corrects _byte_ errors and
works best against short bursts. LDPC codes work at the _bit_ level
and are designed for channels where errors are spread randomly
across the frame. They get closer to the theoretical Shannon limit
--- meaning they can correct more errors for the same amount of
redundancy.

## The Idea

Instead of building a polynomial over a Galois field (like RS),
LDPC uses a large, sparse _parity-check matrix_ $H$. Each row of
$H$ defines one parity-check equation: the XOR of a small subset of
codeword bits must equal zero. If the codeword is valid, every
equation is satisfied. If bits are flipped, some equations fail, and
the pattern of failures reveals which bits are wrong.

"Low-density" means most entries in $H$ are zero --- each check
involves only a few bits, and each bit participates in only a few
checks. This sparsity is what makes decoding tractable: instead of
solving a huge linear system, the decoder passes messages between
checks and bits along the edges of a sparse graph.

## Parity-Check Matrix

A codeword $\mathbf{c}$ of $n$ bits is valid if and only if:

$$H \cdot \mathbf{c}^T = \mathbf{0} \pmod{2}$$

For a code with $k$ information bits and $n$ transmitted bits, $H$
has $(n - k)$ rows and $n$ columns (plus some extra columns for
punctured bits --- more on that later). Each row is one check
equation. Each column corresponds to one bit.

### Example: A Tiny Code

Consider a toy $(7, 4)$ code with 4 info bits and 3 checks:

$$H = \begin{pmatrix} 1 & 1 & 0 & 1 & 1 & 0 & 0 \\ 0 & 1 & 1 & 1 & 0 & 1 & 0 \\ 1 & 0 & 1 & 1 & 0 & 0 & 1 \end{pmatrix}$$

Row 0 says: $c_0 \oplus c_1 \oplus c_3 \oplus c_4 = 0$. If the received
bits satisfy all three equations, the codeword is valid. If not, the
decoder uses the failed checks to figure out which bits flipped.

## AR4JA: The CCSDS LDPC Code

CCSDS 131.0-B-5 specifies a family of _Accumulate-Repeat-4-Jagged-Accumulate_ (AR4JA) codes for telemetry. They come in three rates:

| Rate | $k$ | $n$ | Parity | Punctured |
|---|---|---|---|---|
| 1/2 | 1024 | 2048 | 1024 | 512 |
| 2/3 | 1024 | 1536 | 512 | 256 |
| 4/5 | 1024 | 1280 | 256 | 128 |

The $k = 4096$ variants use the same structure with larger matrices.

The code is _systematic_: the first $k$ bits of the codeword are the
original data, unchanged. Only parity bits are appended.

### Block Structure

The $H$ matrix is not stored as a dense array of bits. Instead it is
defined as a grid of $M \times M$ sub-matrix blocks, where $M$ is the
_submatrix size_ (e.g. 512 for rate 1/2 with $k = 1024$).

For rate 1/2, the base matrix has 3 block-rows and 5 block-columns.
Each block is one of three types:

- **Zero block** ($H_Z$): an $M \times M$ all-zeros matrix --- this
  check doesn't connect to any bit in this column.
- **Identity block** ($H_I$): the $M \times M$ identity matrix ---
  check $i$ connects to bit $i$ in this column.
- **Permutation block** ($H_{P_k}$): an $M \times M$ matrix defined by
  the $\pi_k$ permutation --- check $i$ connects to bit $\pi_k(i)$.

The base matrices for the three rates are:

**Rate 1/2** (3 x 5 blocks):

| | Col 0 | Col 1 | Col 2 | Col 3 | Col 4 |
|---|---|---|---|---|---|
| Row 0 | $H_Z$ | $H_Z$ | $H_I$ | $H_Z$ | $H_I + H_{P_0}$ |
| Row 1 | $H_I$ | $H_I$ | $H_Z$ | $H_I$ | $H_{P_1} + H_{P_2} + H_{P_3}$ |
| Row 2 | $H_I$ | $H_{P_4} + H_{P_5}$ | $H_Z$ | $H_{P_6} + H_{P_7}$ | $H_I$ |

Each $H_{P_k}$ uses a different permutation. The `+` denotes XOR
(mod-2 addition of matrices), so $H_I + H_{P_0}$ means one matrix
where row $i$ has 1s at both column $i$ (identity) and column
$\pi_0(i)$ (permutation).

The last column (column 4 for rate 1/2) corresponds to _punctured_
bits: they are part of the code's internal structure but are not
transmitted. The transmitted codeword comes from columns 0--3.

### The $\pi_k$ Permutation

Each permutation $\pi_k$ maps row position $i$ (in $0 \ldots M-1$) to
a column position within the same block. It is defined by two lookup
tables, $\theta$ and $\phi$, from the CCSDS standard:

$$\pi_k(i) = \frac{M}{4} \cdot ((\theta_k + \lfloor 4i / M \rfloor) \bmod 4) + (\phi_{\lfloor 4i/M \rfloor, k} + i) \bmod \frac{M}{4}$$

The formula splits $M$ into 4 quadrants. $\theta_k$ rotates which
quadrant the output lands in; $\phi_{j,k}$ offsets within the
quadrant. Together they produce a bijection (every output appears
exactly once), which is essential for $H$ to have full rank.

For $M = 512$ and $k = 0$: $\theta_0 = 3$, $\phi_{0,0} = 16$. So:

$$\pi_0(0) = 128 \cdot ((3 + 0) \bmod 4) + (16 + 0) \bmod 128 = 384 + 16 = 400$$

$$\pi_0(200) = 128 \cdot ((3 + 1) \bmod 4) + (16 + 200) \bmod 128 = 0 + 88 = 88$$

The CCSDS standard defines 26 permutations ($k = 0 \ldots 25$). How
many are used depends on the rate: rate 1/2 uses $k = 0 \ldots 7$,
rate 2/3 uses $k = 0 \ldots 13$, rate 4/5 uses $k = 0 \ldots 25$.

## Encoding

Encoding computes the parity bits from the information bits. CCSDS
AR4JA codes use a precomputed _compact generator matrix_ $P$ such
that:

$$\text{parity} = \text{info} \times P \pmod{2}$$

The codeword is then $[\text{info} | \text{parity}]$, with $k$ info bits
followed by $n - k$ parity bits.

### Circulant Structure

The matrix $P$ has dimensions $k \times (n - k)$. Storing it
directly would require $1024 \times 1024 = 131072$ bits for
rate 1/2. Instead, $P$ is decomposed into _circulant blocks_ of
size $b \times b$, where $b$ is the circulant size ($b = M/4$).

A circulant block is fully defined by its first row --- every
subsequent row is the previous row rotated right by one position.
So we only store one row per block:

$$P_\text{compact} = k/b \text{ rows} \times (n-k)/64 \text{ u64 values per row}$$

For rate 1/2: $1024 / 128 = 8$ rows of $1024 / 64 = 16$ u64 each
$= 128$ u64 values total (1024 bytes to represent a million-bit
matrix).

### Encoding Algorithm

The encoder processes info bits in groups aligned to the circulant
structure:

1. For each rotation offset $o = 0, 1, \ldots, b - 1$:
   1. For each compact row $r = 0, 1, \ldots, k/b - 1$:
      - The info bit at position $r \cdot b + o$: if it is 1, XOR the
        compact generator row $r$ into the parity accumulator.
   2. Left-rotate each parity block by 1 bit (simulating the
      circulant shift for the next offset).

After $b$ rotations, every info bit has contributed its generator
row at the correct rotation, and the parity accumulator holds the
final parity bits.

## Syndrome Check

To verify a received codeword, we check $H \cdot \mathbf{c}^T = \mathbf{0}$.
But the transmitted codeword has $n$ bits, while $H$ has $n + M$
columns (the extra $M$ are the punctured bits). The punctured bits
are not transmitted, so they must be recovered first.

### Recovering Punctured Bits

For all AR4JA rates, block-row 2 of the punctured column (the last
column) is a pure identity block. This means check equation
$(2, i)$ connects to punctured bit $i$ via identity, plus some
transmitted bits via other columns. So:

$$\text{punctured}[i] = \bigoplus_{\text{all other connections in row 2, pos } i} \text{codeword}[\text{bit}]$$

We compute this for every $i \in 0 \ldots M - 1$, giving us all $M$
punctured bits.

### Verifying Rows 0 and 1

With the punctured bits recovered, we check rows 0 and 1 of $H$.
For each check position, XOR all connected codeword bits (from the
transmitted columns) and all connected punctured bits (from the
recovered column). If every check evaluates to zero, the codeword is
valid.

Row 2 is not checked separately --- it was used to _define_ the
punctured bits, so it passes by construction. Row 3 of the base
matrix is all zeros and contributes no checks.

## Decoding: Belief Propagation

_Not yet implemented._ The decoder will use layered min-sum belief
propagation with fixed-point (i16) log-likelihood ratios.

The idea: each bit starts with a "soft" confidence value from the
channel (positive = probably 0, negative = probably 1). The decoder
iteratively passes messages between check nodes and variable nodes:

1. **Variable -> Check:** each bit tells each of its checks
   "here's my current belief, excluding what you told me last time."
2. **Check -> Variable:** each check combines messages from all
   its other bits and sends back "based on everyone else, I think
   you should be..."

After enough iterations (typically 20--50), the bits converge to
their corrected values. The min-sum variant approximates the optimal
sum-product algorithm using only additions and comparisons ---
no multiplications or transcendental functions --- making it
suitable for embedded and FPGA implementations.

## Why These Numbers

**Why three rates?** Different missions have different channel
conditions. A deep-space probe with a weak signal uses rate 1/2
(sends 2 bits for every 1 bit of data --- maximum protection). A
low-Earth-orbit satellite with a strong link uses rate 4/5 (sends
only 1.25 bits per data bit --- maximum throughput).

**Why is a column punctured?** The AR4JA code's internal structure
needs an extra set of $M$ "accumulator seed" bits to make the
encoder work. These bits are determined by the code constraints and
don't carry independent information, so transmitting them would
waste bandwidth. The decoder recovers them from the parity checks.

**Why $M = 512$ for rate 1/2?** The base matrix has $n_b = 5$
columns. The transmitted codeword has $(n_b - 1) \cdot M = 4M$ bits.
For $n = 2048$: $M = 2048 / 4 = 512$.

**Why circulant size $b = M/4$?** The $\pi_k$ permutation splits each
$M$-row block into 4 quadrants. The circulant structure of the
generator aligns with these quadrants, so $b = M/4$.

## CCSDS Parameters

| Parameter | Value |
|---|---|
| Standard | CCSDS 131.0-B-5 |
| Code family | AR4JA (Accumulate-Repeat-4-Jagged-Accumulate) |
| Rates | 1/2, 2/3, 4/5 |
| Info lengths | $k = 1024$ or $k = 4096$ |
| Base matrix rows | 3 (plus 1 all-zero row) |
| Base matrix cols | 5 (r1/2), 7 (r2/3), 11 (r4/5) |
| Block types | $H_Z$ (zero), $H_I$ (identity), $H_{P_k}$ (permutation) |
| Permutations | $\pi_k$ with $\theta$/$\phi$ lookup tables |
| Encoding | Compact generator with circulant rotation |
| Decoding | Layered min-sum belief propagation |

## End-to-End Example

We encode 1024 bits using the rate 1/2 code ($n = 2048$), corrupt
some bits, and verify detection.

### 1. Encoding

The 1024 info bits (128 bytes) are the input. For this example,
we use the test vector `info[i] = i` for $i = 0 \ldots 127$:

```
Info (128 bytes):
  [ 0x00, 0x01, 0x02, 0x03, ..., 0x7E, 0x7F ]
```

The encoder computes 1024 parity bits (128 bytes) using the compact
generator matrix. The first few parity bytes are:

```
Parity (128 bytes):
  [ 0xEE, 0xA9, 0xAA, 0xAF, 0x98, 0xD9, 0x16, 0xCE, ... ]
```

The codeword is systematic --- info followed by parity:

```
Codeword (256 bytes = 2048 bits):
  [ 0x00, 0x01, ..., 0x7F, 0xEE, 0xA9, 0xAA, 0xAF, ... ]
    |---- 128 info bytes ---|---- 128 parity bytes ------|
```

### 2. Syndrome Check (Valid)

The syndrome checker first recovers the 512 punctured bits from
row 2 of the $H$ matrix, then verifies rows 0 and 1. For this
valid codeword, all $2 \times 512 = 1024$ check equations evaluate
to zero.

### 3. Corruption

Flip bit 0 of the codeword (the MSB of the first byte):

```
Received:
  [ 0x80, 0x01, 0x02, ..., 0x7F, 0xEE, 0xA9, ... ]
    ^^^^
    was 0x00, now 0x80
```

### 4. Syndrome Check (Invalid)

The recovered punctured bits are now different (because the
transmitted bits changed), and at least one check in rows 0 or 1
evaluates to 1. The syndrome check returns false --- the codeword
is corrupted.

### 5. Correction (Future)

A belief propagation decoder would take the received bits with soft
channel information, run iterative message passing, and converge on
the corrected codeword. This is not yet implemented.

## Comparison with Reed-Solomon

| | Reed-Solomon | LDPC (AR4JA) |
|---|---|---|
| **Error model** | Byte errors | Bit errors |
| **Arithmetic** | GF($2^8$) field ops | Binary XOR only |
| **Matrix** | Dense (syndrome) | Sparse (parity-check) |
| **Decoding** | Algebraic (BM + Forney) | Iterative (belief prop.) |
| **Best for** | Burst errors | Random bit errors |
| **Performance** | ~2 dB from Shannon | ~0.5 dB from Shannon |
| **Complexity** | Low (255 symbols) | Higher (thousands of bits) |

In the CCSDS stack, both are used: RS protects the Transfer Frame
layer, while LDPC provides the physical-layer FEC. They can be
concatenated for extra robustness.
