# Reed-Solomon

## The Problem

A satellite sends 223 bytes to Earth. Some bytes get corrupted by
noise on the way down. We want the receiver to detect and fix the
corruption automatically, without retransmission.

Reed-Solomon (RS) coding adds 32 extra _parity_ bytes to the 223
data bytes, making 255 total. If up to 16 of those 255 bytes are
corrupted in transit, the receiver can figure out which bytes are
wrong and what they should have been.

## Why We Need Arithmetic

A simple check like XOR-ing all bytes together can tell you
_whether_ something changed, but not _which_ byte changed or _what
it changed to_. To recover both the position and the value of a
corrupted byte, you need more information.

The idea is to build multiple independent equations that relate the
parity bytes to the data bytes. Each equation multiplies every data
byte by a different coefficient and sums the results. For example,
one parity byte might be:

$$p_0 = d_0 \cdot \alpha^2 + d_1 \cdot \alpha + d_2$$

and another:

$$p_1 = d_0 \cdot \alpha^4 + d_1 \cdot \alpha^2 + d_2$$

If a single byte $d_1$ gets corrupted, both $p_0$ and $p_1$ will
be wrong by amounts that depend on $d_1$'s position (because each
equation weighted $d_1$ differently). The receiver can solve these
two equations to find both _where_ and _by how much_.

More errors need more equations, which means more parity bytes.
Correcting up to 16 errors requires 32 parity bytes (2 per error:
one for position, one for magnitude).

This is why arithmetic --- multiplication and addition --- is
unavoidable.

## A Special Number System

Since we're doing arithmetic on bytes, we need every operation to
produce a result that is also a byte. In normal arithmetic this
fails: $200 \times 200 = 40000$, which doesn't fit. If intermediate
results grow beyond one byte, the parity symbols become a different
size than the data symbols, and the fixed-size structure (255
symbols $\times$ 1 byte each) falls apart.

So we use a number system called GF($2^8$) --- a _Galois field_
with 256 elements (0--255) --- where every operation on two bytes
always produces exactly one byte:

- **Addition** is XOR: $200 + 200 = 0$ (everything cancels itself)
- **Multiplication** uses special rules that always stay within
  0--255

The field has a special element called $\alpha$ (alpha), defined as
$\alpha = 2$. Its powers generate every non-zero value in the field:

$$\alpha^0 = 1, \quad \alpha^1 = 2, \quad \alpha^2 = 4, \quad \ldots, \quad \alpha^{254} = 142, \quad \alpha^{255} = 1 \text{ (wraps around)}$$

So every non-zero byte can be written as $\alpha^k$ for some $k$.
This is useful because it turns multiplication into addition of
exponents: $\alpha^a \times \alpha^b = \alpha^{a+b}$.

We store these powers in a lookup table called EXP (and the reverse
mapping in LOG), so a multiplication becomes two table lookups and
one addition --- effectively free on any CPU.

## Bytes as a Polynomial

Take the 223 data bytes, say $[d_0, d_1, d_2, \ldots]$. We treat
them as coefficients of a polynomial:

$$d_0 \cdot x^{222} + d_1 \cdot x^{221} + d_2 \cdot x^{220} + \cdots + d_{222}$$

This isn't something we "solve for x." It's a way of representing
the byte sequence so we can do algebra on it. Each byte is a
coefficient; its position in the array determines the power of $x$.

## The Generator Polynomial

We want a polynomial $g(x)$ that is zero at 32 specific points.
Start with the simplest polynomial that is zero at one point ---
$\alpha^{112} = 2^{112}$:

$$(x - 2^{112})$$

This is zero when $x = 2^{112}$ and non-zero everywhere else. To
also be zero at $2^{113}$, multiply by another factor:

$$(x - 2^{112})(x - 2^{113})$$

Continue for all 32 points:

$$g(x) = (x - 2^{112})(x - 2^{113}) \cdots (x - 2^{143})$$

Multiplying these 32 factors out (in GF($2^8$) arithmetic, where
subtraction is the same as addition/XOR) gives a degree-32
polynomial with 33 concrete byte-valued coefficients. It is fixed
--- the same for every message, computed once.

The key property: $g(2^{112}) = 0$, $g(2^{113}) = 0$, ...,
$g(2^{143}) = 0$, by construction.

## Encoding

We have the data polynomial $D(x)$ (223 bytes) and the generator
$g(x)$ (degree 32). The goal is to find 32 parity bytes such that
the complete 255-byte codeword is divisible by $g(x)$.

First, shift $D(x)$ up by 32 positions to make room for parity:

$$D(x) \cdot x^{32}$$

This is like writing the 223 data bytes followed by 32 zeros.
Now divide by $g(x)$:

$$D(x) \cdot x^{32} = Q(x) \cdot g(x) + R(x)$$

where $R(x)$ is the remainder (degree < 32, so exactly 32 bytes).
The codeword is:

$$C(x) = D(x) \cdot x^{32} + R(x)$$

In GF($2^8$), addition is XOR, so adding $R(x)$ replaces those 32
trailing zeros with the parity bytes. Now check --- is $C(x)$
divisible by $g(x)$?

$$C(x) = D(x) \cdot x^{32} + R(x) = Q(x) \cdot g(x) + R(x) + R(x) = Q(x) \cdot g(x)$$

Yes --- because $R(x) + R(x) = 0$ (XOR with itself). So $C(x)$
is exactly divisible by $g(x)$, which means evaluating $C(x)$
at any root of $g(x)$ gives zero:

$$C(2^{112}) = Q(2^{112}) \cdot \underbrace{g(2^{112})}_{= 0} = 0$$

The 223 data bytes sit at the front, unchanged. Only 32 parity
bytes are appended. This is called _systematic_ encoding.

## Decoding

### Syndrome Check

The receiver evaluates the received 255-byte polynomial at the same
32 special values. If the data arrived intact, all 32 results are
zero (because a valid codeword is divisible by $g(x)$).

If any byte was corrupted, at least some results will be non-zero.
These 32 numbers are the _syndromes_ --- a fingerprint of the
damage.

### Finding Error Positions: Berlekamp-Massey

For a single error, the ratio $S_1 / S_0$ directly reveals the
error position (as shown in the example). For multiple errors, we
need to find _all_ error positions simultaneously.

The idea is to build an _error locator polynomial_ $\sigma(x)$ whose
roots correspond to the error positions. If there are $t$ errors at
positions $p_1, p_2, \ldots, p_t$, define $X_k = 2^{254 - p_k}$ for
each error and:

$$\sigma(x) = (1 - X_1 x)(1 - X_2 x) \cdots (1 - X_t x) = 1 + \sigma_1 x + \sigma_2 x^2 + \cdots + \sigma_t x^t$$

Each $X_k^{-1}$ is a root of $\sigma(x)$, and from $X_k$ we can
recover $p_k$.

The _Berlekamp-Massey_ algorithm finds $\sigma(x)$ from the
syndromes. It works iteratively, processing one syndrome at a time:

1. Start with $\sigma(x) = 1$ (no errors assumed).
2. For each syndrome $S_n$ ($n = 0, 1, \ldots, 31$), compute a
   _discrepancy_:
   $$\Delta_n = S_n + \sigma_1 \cdot S_{n-1} + \sigma_2 \cdot S_{n-2} + \cdots + \sigma_l \cdot S_{n-l}$$
   where $l$ is the current number of estimated errors.
3. If $\Delta_n = 0$, the current $\sigma$ already predicts $S_n$
   correctly --- no update needed.
4. If $\Delta_n \neq 0$, the current $\sigma$ is wrong. Update it
   using a correction term scaled by $\Delta_n$. If this increases
   the estimated error count ($2l \leq n$), also update $l$.
5. After all 32 syndromes, $l$ is the number of errors and $\sigma(x)$
   has degree $l$. If $l > 16$, the codeword has too many errors to
   correct.

The algorithm is efficient because it reuses a saved "old" version
of $\sigma$ as the correction term, so each step is just a few
multiplications.

### Finding Error Positions: Chien Search

Berlekamp-Massey gives us $\sigma(x)$, but we need the actual roots.
The _Chien search_ simply evaluates $\sigma(x)$ at every possible
position:

1. For each $m = 0, 1, \ldots, 254$, compute $\sigma(2^m)$ using the
   GF($2^8$) lookup tables.
2. If $\sigma(2^m) = 0$, then $X = 2^m$ is a root, meaning there is
   an error at position $p = (m + 254) \bmod 255$.
3. Collect all positions where $\sigma$ evaluates to zero.

If the number of roots found does not match $l$ from
Berlekamp-Massey, the codeword is uncorrectable.

This is a brute-force search, but with only 255 positions and each
evaluation being a few table lookups, it is fast.

### Finding Error Magnitudes: Forney Algorithm

Now we know _where_ the errors are. The _Forney algorithm_ computes
_how much_ each corrupted byte is off by.

First, build the _error evaluator polynomial_:

$$\Omega(x) = S(x) \cdot \sigma(x) \bmod x^{32}$$

where $S(x) = S_0 + S_1 x + S_2 x^2 + \cdots + S_{31} x^{31}$ is
the syndrome polynomial. This multiplication and truncation combines
the syndrome information with the error locations.

Next, compute the _formal derivative_ of $\sigma(x)$. In GF($2^8$),
the derivative has a simplification: since $2 = 0$ in GF($2$), all
even-power terms vanish. Only the odd-index coefficients survive:

$$\sigma'(x) = \sigma_1 + \sigma_3 x^2 + \sigma_5 x^4 + \cdots$$

The error magnitude at position $k$ is then:

$$e_k = X_k^{1 - 112} \cdot \frac{\Omega(X_k^{-1})}{\sigma'(X_k^{-1})}$$

where $X_k = 2^{254 - p_k}$ is the error locator value for position
$p_k$, and the $X_k^{1-112}$ factor adjusts for the first
consecutive root being $\alpha^{112}$ instead of $\alpha^0$.

### Correction

XOR each corrupted byte with its computed error magnitude. The
original data is restored. As a final check, recompute the 32
syndromes --- if they are all zero, the correction succeeded.

### When Correction Fails

If more than 16 bytes are corrupted, the decoder detects the
failure at one of three points:

1. **Berlekamp-Massey** finds $l > 16$: the syndrome equations imply
   more errors than the code can handle.
2. **Chien search** finds fewer roots than $l$: the error locator
   polynomial has no valid solution within the 255 positions.
3. **Verification** after correction: the recomputed syndromes are
   still non-zero, meaning the applied corrections were wrong.

In all three cases, the decoder reports failure. The data remains
corrupted and must be recovered by retransmission or a higher-level
protocol.

## Why These Numbers

**Why 32 parity bytes for 16 errors?** Each error has two unknowns:
_where_ it is and _what_ changed. That's 2 unknowns per error, and
each syndrome gives 1 equation. 32 syndromes -> at most 16
solvable errors.

**Why 255 total?** In GF($2^8$) there are 255 non-zero values. Each
one maps to a position in the codeword. You can't have more
positions than field elements, so 255 is the maximum codeword
length for byte-sized symbols.

**Why 223 data bytes?** $255 - 32 = 223$. It's simply whatever room
is left after reserving space for 32 parity bytes.

## Interleaving

Burst errors (e.g. a brief signal dropout) corrupt consecutive
bytes. With interleaving depth $I$, the transmitter shuffles $I$
independent codewords together so that consecutive bytes belong to
_different_ codewords.

A burst of $16 \cdot I$ corrupted bytes gets spread across $I$
codewords, each seeing at most 16 errors --- exactly within the
correction capability. CCSDS supports $I = 1$ through $5$.

## CCSDS Parameters

The specific parameters used in this implementation follow
CCSDS 131.0-B-5:

| Parameter | Value |
|---|---|
| Field polynomial | $x^8 + x^7 + x^2 + x + 1$ (`0x187`) |
| Primitive element | $\alpha = 2$ |
| Codeword length | 255 symbols |
| Data length | 223 symbols |
| Parity symbols | 32 |
| Error correction | Up to 16 symbol errors |
| First consecutive root | $\alpha^{112}$ |
| Interleave depth | $I = 1$ to $5$ |

## End-to-End Example

We encode the message `"HELLO"` (bytes `72, 69, 76, 76, 79`),
corrupt one byte in transit, and recover the original.

### 1. Encoding

The 5-byte message is padded with zeros to fill all 223 data
positions:

```
Data (223 bytes):
  [ 72, 69, 76, 76, 79, 0, 0, 0, ... , 0 ]
    H   E   L   L   O
```

The data bytes become a polynomial (each byte is a coefficient,
highest power first):

$$D(x) = 72 x^{222} + 69 x^{221} + 76 x^{220} + 76 x^{219} + 79 x^{218}$$

(The remaining 218 coefficients are zero.)

Shift it up by 32 to make room for parity:

$$D(x) \cdot x^{32} = 72 x^{254} + 69 x^{253} + 76 x^{252} + 76 x^{251} + 79 x^{250}$$

The generator $g(x) = (x - 2^{112})(x - 2^{113}) \cdots (x - 2^{143})$
is a fixed degree-32 polynomial.

Divide $D(x) \cdot x^{32}$ by $g(x)$. The remainder $R(x)$ has 32
coefficients --- these become the parity bytes
`[243, 147, 197, 58, ...]`. XOR them into the trailing zeros:

```
Codeword (255 bytes):
  [ 72, 69, 76, 76, 79, 0, ..., 0, 243, 147, 197, 58, 154, 156, 250, 218, ... ]
    |--- 223 data bytes ----------|  |-------- 32 parity bytes --------------|
```

The data is unchanged at the front. Only parity was added.

### 2. Corruption

During transmission, byte 2 gets corrupted --- the `L` (76)
becomes 255:

```
Received:
  [ 72, 69, 255, 76, 79, 0, ..., 0, 243, 147, 197, ... ]
            ^^^
        was 76, now 255
```

### 3. Syndrome Check

The received 255 bytes $[r_0, r_1, \ldots, r_{254}]$ form a
polynomial:

$$R(x) = r_0 \cdot x^{254} + r_1 \cdot x^{253} + \cdots + r_{254}$$

The receiver computes each syndrome by replacing $x$ with one of
the 32 special values. Since $\alpha = 2$, the first syndrome $S_0$
uses $x = \alpha^{112} = 2^{112}$. In GF($2^8$), exponents wrap
mod 255, so $2^{112}$ is just a single byte (looked up from the
EXP table):

$$S_0 = r_0 \cdot (2^{112})^{254} + r_1 \cdot (2^{112})^{253} + r_2 \cdot (2^{112})^{252} + \cdots + r_{254}$$

Each term like $(2^{112})^{254}$ simplifies to $2^{(112 \cdot 254) \bmod 255} = 2^{93}$,
which is just another byte from the EXP table. Plugging in the
received bytes ($r_0 = 72$, $r_1 = 69$, $r_2 = 255$, ...):

$$S_0 = 72 \cdot 2^{93} + 69 \cdot 2^{236} + 255 \cdot 2^{124} + \cdots = 219$$

All arithmetic is in GF($2^8$): every multiplication uses the
lookup tables, every addition is XOR, and the result is always a
single byte.

In general, each syndrome $S_j$ uses a different power of 2:

$$S_j = r_0 \cdot 2^{(112+j) \cdot 254 \bmod 255} + r_1 \cdot 2^{(112+j) \cdot 253 \bmod 255} + \cdots + r_{254}$$

for $j = 0, 1, \ldots, 31$. The same bytes, but different
coefficients each time --- that's what gives the receiver 32
independent equations to work with.

For the original (uncorrupted) codeword, every syndrome is zero.
Recall from the encoding step that the codeword is
$C(x) = Q(x) \cdot g(x)$, where
$g(x) = (x - 2^{112})(x - 2^{113}) \cdots (x - 2^{143})$.
To compute $S_0$, we evaluate $C(x)$ at $x = 2^{112}$:

$$S_0 = C(2^{112}) = Q(2^{112}) \cdot g(2^{112})$$

Expanding $g(2^{112})$:

$$g(2^{112}) = (2^{112} - 2^{112})(2^{112} - 2^{113}) \cdots (2^{112} - 2^{143})$$

The first factor is $2^{112} - 2^{112} = 0$ (any value XOR'd
with itself is zero). A product with a zero factor is zero, so
$g(2^{112}) = 0$ and:

$$S_0 = Q(2^{112}) \cdot 0 = 0$$

The same applies to $S_1$ through $S_{31}$: each evaluates at
$x = 2^{113}$ through $x = 2^{143}$, and each is a root of $g(x)$
by construction, so every factor chain has a zero term:

$$S_0 = 0, \quad S_1 = 0, \quad S_2 = 0, \quad \ldots, \quad S_{31} = 0$$

After the corruption at byte 2, the syndromes become non-zero:

$$S_0 = 219, \quad S_1 = 232, \quad S_2 = 29, \quad S_3 = 145, \quad S_4 = 67, \quad \ldots$$

These 32 values encode both _where_ the error is and _how large_
it is. The decoder's job is to extract that information.

### 4. Error Location (Berlekamp-Massey)

The decoder builds an _error locator polynomial_ $\sigma(x)$ whose
roots reveal the corrupted positions. It processes the syndromes
one at a time, updating $\sigma(x)$ whenever the current guess fails
to predict the next syndrome.

Start with $\sigma(x) = 1$ and $l = 0$ (no errors assumed).

**Iteration $n = 0$:** Compute the discrepancy $\Delta_0 = S_0 = 219$.
This is non-zero, so the current $\sigma$ is wrong. Update:
$\sigma(x) \leftarrow 1 + 219 x$, $l \leftarrow 1$.

**Iteration $n = 1$:** $\Delta_1 = S_1 \oplus \sigma_1 \cdot S_0 = 232 \oplus \text{mul}(219, 219) = 232 \oplus 101 = 141$.
Non-zero, so update $\sigma$. Since $l$ doesn't increase ($2 \cdot 1 > 1$),
only the coefficients change: $\sigma(x) \leftarrow 1 + 81 x$.

**Iteration $n = 2$:** $\Delta_2 = S_2 \oplus \sigma_1 \cdot S_1 = 29 \oplus \text{mul}(81, 232) = 29 \oplus 29 = 0$.
Zero --- $\sigma$ already predicts $S_2$ correctly. No update.

**Iterations $n = 3$ through $31$:** All discrepancies are zero.
The polynomial has converged.

Result: $\sigma(x) = 1 + 81 x$ with $l = 1$ (one error). Since
$81 = \alpha^{252}$, the error locator coefficient directly encodes
the error position.

### 4b. Error Location (Chien Search)

The Chien search finds the roots of $\sigma(x) = 1 + 81 x$ by
evaluating it at every possible value $x = \alpha^m$:

$$\sigma(\alpha^0) = 1 \oplus \text{mul}(81, 1) = 1 \oplus 81 = 80$$

$$\sigma(\alpha^1) = 1 \oplus \text{mul}(81, 2) = 1 \oplus 162 = 163$$

$$\sigma(\alpha^2) = 1 \oplus \text{mul}(81, 4) = 1 \oplus 195 = 194$$

$$\sigma(\alpha^3) = 1 \oplus \text{mul}(81, 8) = 1 \oplus 1 = 0 \quad \leftarrow \text{root!}$$

$\sigma(\alpha^3) = 0$, so $\alpha^3$ is a root. The error position is
$(3 + 254) \bmod 255 = 2$. That is byte 2 --- exactly where the `L`
was corrupted.

### 5. Error Magnitude (Forney)

The Forney algorithm computes the error evaluator polynomial
$\Omega(x) = S(x) \cdot \sigma(x) \bmod x^{32}$, which starts:

$$\Omega_0 = S_0 \cdot \sigma_0 = 219 \cdot 1 = 219$$
$$\Omega_1 = S_1 \cdot \sigma_0 \oplus S_0 \cdot \sigma_1 = 232 \oplus \text{mul}(219, 81) = 232 \oplus 232 = 0$$

So $\Omega(x) = 219$ (a constant). The formal derivative of $\sigma$
is also a constant: $\sigma'(x) = \sigma_1 = 81$.

The error magnitude uses $X = \alpha^{254-p} = \alpha^{252} = 81$ and
$X^{-1} = \alpha^3 = 8$:

$$e = X^{1 - 112} \cdot \frac{\Omega(X^{-1})}{\sigma'(X^{-1})} = \alpha^{78} \cdot \frac{219}{81} = 19 \cdot 196 = 179$$

where $X^{1 - 112} = \alpha^{252 \cdot 144 \bmod 255} = \alpha^{78} = 19$ adjusts for
the first consecutive root, and $219 / 81 = 196$ in GF($2^8$).

Correction is XOR:

$$255 \oplus 179 = 76 \quad (\text{the original } \texttt{L}\text{!})$$

### 6. Result

The corrected codeword matches the original:

```
Corrected:
  [ 72, 69, 76, 76, 79, 0, ..., 0, 243, 147, 197, ... ]
    H   E   L   L   O              (parity intact)

  Errors corrected: 1
```
