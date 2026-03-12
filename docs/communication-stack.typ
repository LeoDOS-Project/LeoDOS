#set page(paper: "a4", margin: 2cm)
#set text(font: "Helvetica Neue", size: 10.5pt)
#set heading(numbering: "1.1")
#set par(justify: true)

= LeoDOS Communication Stack

#let layer(name, ..children) = {
  rect(
    width: 100%,
    stroke: 0.75pt + black,
    inset: 4pt,
    [
      #text(weight: "bold", size: 8.5pt)[#name]
      #v(1pt)
      #children.pos().join()
    ]
  )
}

#let sublayer(body, width: 100%) = {
  rect(
    width: width,
    stroke: 0.5pt + luma(120),
    inset: 3pt,
    fill: luma(245),
    text(weight: "bold", size: 7.5pt)[#body]
  )
}

#let alt(..items) = {
  grid(
    columns: (1fr,) * items.pos().len(),
    column-gutter: 2pt,
    ..items.pos().map(c => sublayer(c, width: 100%))
  )
}

#let seq(..items) = {
  stack(
    dir: ttb,
    spacing: 1pt,
    ..items.pos().map(c => sublayer(c))
  )
}

#let group(name, content) = {
  rect(
    width: 100%,
    stroke: 0.5pt + luma(160),
    inset: 3pt,
    fill: luma(252),
    [
      #text(size: 7pt, style: "italic")[#name]
      #v(-2pt)
      #content
    ]
  )
}

#layer("Application")[
  #alt([SpaceCoMP], [ColonyOS])
]
#v(-5pt)
#layer("Transport")[
  #alt([SRSPP], [CFDP])
]
#v(-5pt)
#layer("Network")[
  #group("cFE Headers")[
    #alt([TM], [TC])
  ]
  #v(-4pt)
  #group("Routing")[
    #alt([ISL Router], [PassThrough], [Gossip])
  ]
]
#v(-5pt)
#layer("Data Link")[
  #group("Packet Protocol")[
    #seq([SPP])
  ]
  #v(-4pt)
  #group("Transfer Frame Protocols")[
    #alt([TM], [TC], [AOS], [Proximity-1], [USLP])
  ]
  #v(-4pt)
  #group("Security")[
    #seq([SDLS])
  ]
  #v(-4pt)
  #group("Reliability")[
    #seq([COP-1])
  ]
]
#v(-5pt)
#layer("Coding")[
  #group("Randomization")[
    #seq([Randomization])
  ]
  #v(-4pt)
  #group("Forward Error Correction")[
    #alt([RS(255,223)], [LDPC (AR4JA)], [Convolutional])
  ]
  #v(-4pt)
  #group("Framing")[
    #alt([ASM / CADU], [CLTU])
  ]
  #v(-4pt)
  #group("Data Compression (payload)")[
    #alt([Rice], [DWT], [Hyperspectral])
  ]
]
#v(-5pt)
#layer("Physical")[
  #group("Modulation")[
    #alt([BPSK], [QPSK], [OQPSK], [8PSK], [GMSK])
  ]
  #v(-4pt)
  #group("Hardware")[
    #alt([UART], [SPI], [I2C], [CAN], [GPIO], [UDP], [TCP])
  ]
]

#pagebreak()

= Application Layer

The application layer contains the end-user protocols that use the
communication stack to accomplish mission objectives.

== SpaceCoMP

The Space Computing Platform enables distributed computation
across the satellite constellation. The motivation is that LEO
satellites collect large volumes of data (e.g. Earth observation
imagery) but have limited downlink bandwidth. By processing data
on-orbit and only downlinking results, SpaceCoMP reduces the
communication bottleneck.

SpaceCoMP implements a map-reduce model:

+ A ground station submits a *job* defining the computation to
  perform and the geographic area of interest.
+ The *coordinator* (a designated satellite or ground station)
  plans the job: it identifies which satellites are over the area
  of interest, estimates the cost of assigning each satellite
  (based on link quality, battery state, orbital position), and
  solves an assignment problem to select participants. Assignment
  uses either the Hungarian algorithm or the LAPJV algorithm.
+ *Collectors* gather the raw input data (e.g. from on-board
  instruments).
+ *Mappers* process their assigned data partitions.
+ *Reducers* aggregate the partial results into a final output.
+ The final result is downlinked to the ground station.

Communication between roles uses the transport layer (SRSPP or
CFDP depending on the data size). The job planning and role
assignment messages use a defined packet format that is carried as
Space Packet payloads.

== ColonyOS

ColonyOS integration allows an external orchestration platform to
schedule compute jobs onto the constellation without understanding
the satellite communication topology.

A ColonyOS executor runs on each participating satellite. It
periodically polls a ColonyOS backend (via the transport layer) for
new job assignments. When a job is assigned, the executor processes
it locally and reports the result. A client library provides the
interface for submitting jobs and receiving results.

The message protocol uses length-value encoded payloads within
Space Packets, providing a simple serialization format that is
efficient on constrained hardware.

#pagebreak()

= Transport Layer

The transport layer provides end-to-end reliable delivery. Below
this layer, reliability is per-hop (COP-1) and packets can still
be lost at intermediate routers. The transport layer guarantees
that data arrives at the destination application complete, in
order, and without duplicates, regardless of how many hops the path
traverses.

== SRSPP

The Simple Reliable Space Packet Protocol provides reliable
delivery of variable-size messages. It is designed for the
request-response patterns typical of command/telemetry and
distributed computing (SpaceCoMP).

On the send side, SRSPP segments a message into Space Packets that
fit within the MTU (Maximum Transfer Unit --- the largest packet
the network layer can carry in one piece), assigns each a sequence
number, and transmits them through the network layer. It maintains
a retransmission buffer and a timer for each outstanding packet.

On the receive side, SRSPP collects incoming packets, reorders
them by sequence number, detects gaps, and reassembles the
original message. It sends acknowledgments back to the sender:
cumulative ACKs confirm all packets up to a sequence number, and
selective ACKs identify specific packets received beyond a gap.

When the sender receives an ACK, it removes acknowledged packets
from the retransmission buffer. When a timer expires without
acknowledgment, it retransmits the unacknowledged packet. The
retransmission timeout adapts to observed round-trip times.

Three receiver backends trade off memory and performance:

- *Fast*: Optimized for throughput. Uses more memory to allow
  rapid insertion and retrieval.
- *Lite*: Minimizes memory usage. Suitable for resource-constrained
  satellites.
- *Packed*: Uses compact in-place storage for a balance between
  the two.

SRSPP has platform-specific async APIs for both the Tokio runtime
(used in ground stations and simulation) and the cFS runtime (used
on flight software).

== CFDP (727.0-B-5)

The CCSDS File Delivery Protocol (727.0-B-5) provides reliable
file transfer. Unlike SRSPP which delivers messages, CFDP
transfers named files of arbitrary size with metadata.

CFDP Class 2 (acknowledged mode) uses a state machine with the
following phases:

+ *Metadata*: The sender transmits a Metadata PDU (Protocol Data
  Unit --- the CFDP term for a single message exchanged between
  sender and receiver) containing the file name, size, and options.
+ *File Data*: The sender transmits File Data PDUs containing
  successive chunks of the file.
+ *EOF*: The sender transmits an EOF PDU with a checksum of the
  complete file.
+ *NAK*: If the receiver detects missing chunks, it sends NAK
  (Negative Acknowledgment) PDUs listing the gaps. The sender
  retransmits the missing data.
+ *Finished*: The receiver confirms the file is complete and
  intact. The sender sends a final ACK.

CFDP manages concurrent file transfers using transaction IDs.
Each transfer is an independent state machine, and multiple
transfers can proceed in parallel over the same link.

The file I/O is abstracted behind a platform-independent filestore
trait, allowing CFDP to work on any system that can read and write
files --- whether that is a Linux filesystem on the ground or a
flash-based filesystem on a satellite.

#pagebreak()

= Network Layer

The network layer provides addressing and multi-hop forwarding.
Below this layer, communication is point-to-point between
neighbours. Above it, any node can send to any other node in the
constellation.

== cFE Headers

The core Flight Executive (cFE) defines mission-specific secondary
headers that extend the generic SPP primary header. These headers
occupy the SPP secondary header field and add metadata needed by
the cFE Software Bus for message routing and validation.

A *Telemetry* (TM) packet carries a secondary header containing a
timestamp (6 bytes) used by the ground system to correlate
telemetry with on-board time.

A *Telecommand* (TC) packet carries a secondary header containing
a function code (1 byte) that identifies the specific command
within the target application, and a checksum (1 byte) computed
over the entire packet to detect corruption.

The cFE Software Bus routes packets using a composite Message ID
derived from the SPP APID and packet type fields. This allows TM
and TC packets to share the APID space without ambiguity.

== ISL Router

The LeoDOS constellation is arranged as a _torus mesh_: satellites
are organized in a rectangular grid, and the edges wrap around
(the rightmost satellite in a row is linked to the leftmost, and
the topmost in a column to the bottommost --- like the surface of
a doughnut). Each satellite
has four inter-satellite links (north, south, east, west), one
ground link, and one local loopback for packets destined for itself.

The Router is the core of the network layer. It receives packets
from all five directional links concurrently and either delivers
them locally or forwards them toward their destination. Routing
decisions are time-dependent: the Router uses a monotonic clock to
account for the changing geometry of the constellation.

Destination addresses can be satellite grid positions, ground
stations, or service areas (multicast to an orbital plane). For
ground station addresses, the routing algorithm resolves the
destination to a _gateway satellite_ --- the satellite currently
overhead with line-of-sight to the station. The gateway is
determined by computing each satellite's position in Earth-centred
coordinates (accounting for orbital motion and Earth rotation) and
selecting the one with the highest elevation angle above the
station's horizon.

Routing algorithms are pluggable:

- *Distance Minimizing*: Physics-aware routing that considers
  orbital mechanics. Near the poles, orbital planes converge and
  cross-plane ISL distances become short, so the algorithm
  prefers east/west hops there. Near the equator, where planes
  are far apart, it prefers north/south hops along the orbit.
  The decision accounts for the satellite's current orbital
  position and how it changes over time.
- *Manhattan*: Routes along the torus grid using taxicab distance.
  Minimizes hop count on the grid, which may differ from geographic
  distance when orbits are not uniformly spaced.

The Router also supports gossip broadcast: a special flooding
protocol that delivers a message to every satellite in the
constellation. Gossip uses _epidemic forwarding_: each node that receives a
gossip message rebroadcasts it to all its neighbours, much like a
rumour spreading through a crowd. To prevent messages from
circulating forever, each gossip message carries an epoch number,
and each node tracks which epochs it has already seen. If a
message arrives with an epoch the node has already processed, it
is silently dropped.

== PassThrough

A trivial network layer for links that do not need routing --- for
example, a ground station with a single uplink to one satellite.
PassThrough forwards all packets directly to the underlying data
link without inspecting headers or making routing decisions.

#pagebreak()

= Data Link Layer

The data link layer imposes structure on the data passing between
the network layer above and the coding layer below: frame
boundaries, addressing, multiplexing, security, and optional
per-hop reliability. It operates on individual point-to-point
links --- each ISL and each ground link has its own independent
data link layer instance.

== Space Packet Protocol --- SPP (133.0-B-2)

CCSDS SPP (133.0-B-2) defines the packet format used by all higher
layers. Each Space Packet has a 6-byte primary header containing:

- *APID* (Application Process Identifier): an 11-bit value that
  identifies which application or service the packet belongs to.
  The receiver uses the APID to dispatch incoming packets to the
  correct handler.
- *Sequence Count*: a 14-bit counter that increments per packet
  per APID. Used by the transport layer to detect gaps and
  reorder.
- *Sequence Flags*: indicate whether the packet is unsegmented, or
  the first/continuation/last segment of a larger payload.
- *Data Length*: the number of bytes in the data field.

When a payload is too large for a single Space Packet (constrained
by the maximum frame size), the segmenter splits it into multiple
packets with appropriate sequence flags. The reassembler on the
receiving side collects the segments and reconstructs the original
payload.

The Encapsulation Packet Protocol (CCSDS 133.1-B-3) extends SPP
with encapsulation packets that can wrap non-CCSDS data or serve
as idle fill when the link has no data to send but must maintain
frame synchronization.

Space Packets are extracted from transfer frames by the link
reader, which parses the data field using the First Header Pointer
and packet length fields to find packet boundaries even when
packets span multiple frames.

== Transfer Frame Protocols

Each point-to-point link uses one transfer frame protocol. The
choice depends on the link direction and type.

=== TM --- Telemetry Transfer Frame (132.0-B-3)

Used for the downlink (satellite to ground). A TM frame has a
fixed length configured at link setup time. The header contains:

- *Spacecraft ID (SCID)*: identifies the satellite.
- *Virtual Channel ID (VCID)*: multiplexes multiple data streams
  over a single physical link. For example, real-time telemetry
  and stored science data can share a downlink on separate virtual
  channels.
- *Master Channel Frame Counter*: increments for every frame on
  the physical link, across all virtual channels.
- *Virtual Channel Frame Counter*: increments for every frame on
  this virtual channel specifically. COP-1 uses this counter to
  detect lost frames.
- *First Header Pointer*: the byte offset within the data field
  where the first Space Packet begins. This allows the receiver
  to find packet boundaries even when packets span multiple frames.

The fixed frame length simplifies the coding layer (RS and ASM
operate on fixed-size blocks) and enables the receiver to achieve
frame synchronization without delimiters.

=== TC --- Telecommand Transfer Frame (232.0-B-4)

Used for the uplink (ground to satellite). TC frames are
variable-length (up to a configured maximum). The header contains
SCID, VCID, frame sequence number, and two flags:

- *Bypass flag*: when set, the frame bypasses the COP-1 sequence
  check and is delivered immediately. Used for emergency commands
  when the COP-1 state may be out of sync.
- *Control flag*: distinguishes command frames (carrying Space
  Packets) from control frames (carrying COP-1 directives).

TC frames are typically much shorter than TM frames because
commands are small and uplink bandwidth is limited.

=== AOS --- Advanced Orbiting Systems (732.0-B-4)

An extension of TM for missions that need higher data rates or
additional multiplexing. AOS adds an "insert zone" for
path-level metadata (e.g. time stamps inserted by the ground
station) and supports both packet-mode and bitstream-mode virtual
channels.

=== Proximity-1 (211.2-B-3)

Defined by CCSDS 211.2-B-3 for short-range inter-spacecraft links.
Proximity-1 frames include a Physical Layer Control Word (PLCW)
that carries physical-layer status (e.g. signal quality, link
mode) within the frame itself, enabling the two spacecraft to
coordinate link parameters without a separate control channel. An
optional segment header supports segmentation at the data link
layer for time-critical data.

=== USLP --- Unified Space Data Link Protocol (732.1-B-3)

CCSDS 732.1-B-3 defines a next-generation frame format that
replaces TC, TM, and AOS with a single protocol. USLP supports
variable-length frames, larger SCID and VCID spaces, and a
flexible header that can carry any combination of the features
from the older protocols. It is intended for future missions where
a single protocol simplifies implementation and testing.

== Security: SDLS (355.0-B-2)

Space Data Link Security (CCSDS 355.0-B-2) provides
confidentiality and integrity protection at the frame level. Each
frame can be encrypted, authenticated, or both.

A Security Association (SA) binds a Security Parameter Index (SPI)
to a set of cryptographic parameters: algorithm, key, initialization
vector (IV --- a unique value used in each encryption operation to
ensure that encrypting the same data twice produces different
ciphertext), and service type. The sender and receiver must share
the same SA configuration.

LeoDOS implements AES-GCM (Advanced Encryption Standard in Galois/
Counter Mode), an authenticated encryption algorithm that
simultaneously encrypts data and computes a MAC (Message
Authentication Code --- a cryptographic checksum that proves the
data has not been tampered with). AES-GCM is used with 128-bit or
256-bit keys. On the send side, `apply_security()` takes a
plaintext frame, encrypts the data field, computes the MAC over
the header and ciphertext, and appends a security trailer
containing the MAC. On the receive
side, `process_security()` verifies the MAC, decrypts the data
field, and returns the plaintext frame. If the MAC verification
fails, the frame is discarded.

SDLS is applied after the transfer frame is constructed but before
it is passed to the coding layer. This means the FEC protects the
encrypted data, and an attacker who can corrupt the signal cannot
cause the receiver to accept a forged frame even if the FEC
"corrects" the corruption into valid-looking ciphertext.

== Reliability: COP-1 (232.1-B-2)

Communications Operation Procedure-1 (CCSDS 232.1-B-2) provides
reliable frame delivery on a single hop. It is an ARQ (Automatic
Repeat reQuest) protocol --- a scheme where the receiver
acknowledges each successfully received frame, and the sender
retransmits anything that is not acknowledged within a timeout. ARQ
uses a _sliding window_: the sender can have multiple frames
in flight simultaneously (up to the window size) without waiting
for each to be acknowledged individually.

The sender side is called FOP-1 (Frame Operation Procedure). It
assigns a sequence number to each TC frame, places it in a
retransmission buffer, and starts a timer. If the timer expires
before the receiver acknowledges the frame, FOP-1 retransmits it.

The receiver side is called FARM-1 (Frame Acceptance and Reporting
Mechanism). It maintains a window of expected sequence numbers. If
a frame arrives with the expected sequence number, FARM-1 accepts
it and advances the window. If a frame arrives out of order or
with a gap, FARM-1 can either reject it or buffer it depending on
configuration.

FARM-1 communicates its state back to the sender via the CLCW
(Command Link Control Word), a 32-bit field piggybacked on TM
frames. The CLCW reports the next expected sequence number and
status flags (e.g. "no RF available", "retransmit"). FOP-1 parses
the CLCW to determine which frames have been received and which
need retransmission.

COP-1 is essential for link-level recovery. Without it, every
frame lost due to uncorrectable bit errors would propagate up to
the transport layer, which would have to retransmit across the
entire multi-hop path. With COP-1, most losses are recovered in a
single link round-trip time.

#pagebreak()

= Coding Layer

The coding layer protects transfer frames against bit errors
introduced by the RF channel. Without forward error correction,
even a single flipped bit would corrupt the frame and force a
retransmission at a higher layer. The coding layer applies three
operations in sequence: randomization, forward error correction,
and framing.

== Randomization (131.0-B-5)

The transmitted bitstream must contain frequent transitions between
0 and 1 so that the receiver's clock recovery circuit can stay
synchronized to the signal. The receiver has no independent clock;
it infers the sender's bit timing from the transitions in the
incoming signal. A phase-locked loop (PLL) tracks these transitions
and adjusts its internal clock to stay aligned. If the data
happens to contain long runs of the same bit (e.g. a block of
zeros), there are no transitions and the PLL drifts, causing the
receiver to misinterpret subsequent bits.

Randomization prevents this by XOR-ing the frame data with a
deterministic pseudo-random sequence generated by a linear feedback
shift register (LFSR) --- a simple shift register whose input bit
is a function of its current state, producing a sequence that
appears random but is entirely predictable. The same sequence is
known to both sender and receiver. Because XOR is its own inverse
(applying it twice returns the original value), the receiver
applies the identical operation to recover the original data.

CCSDS defines three randomizer variants. The TC randomizer uses a
255-byte sequence generated by the polynomial
$x^8 + x^6 + x^4 + x^3 + x^2 + x + 1$. The TM randomizer uses
the polynomial $x^8 + x^7 + x^5 + x^3 + 1$, available in a
255-byte (legacy) or 131071-byte (recommended) variant. The longer
sequence is preferred because it avoids correlation with periodic
frame content.

Randomization is always applied first, before FEC encoding, because
the FEC parity symbols are computed over the randomized data and
therefore do not need separate randomization.

== Forward Error Correction

FEC (Forward Error Correction) adds redundant data so that the
receiver can detect and correct errors without requesting
retransmission. The sender computes extra _parity_ bytes from the
original data using a mathematical code, and appends them. The
receiver uses the parity to detect which bytes were corrupted and
compute their original values. The three FEC schemes offer
different trade-offs between correction capability, complexity, and
latency. Only one is used per link.

=== Reed-Solomon RS(255,223) (131.0-B-5)

The CCSDS standard Reed-Solomon code operates over a Galois field
--- a finite set of 256 values ($2^8$, one per byte) with specially
defined addition and multiplication that ensure every non-zero
value has an inverse. This algebraic structure is what makes the
error correction mathematics work. The code appends 32 parity
bytes to 223 data bytes, producing a 255-byte codeword. It can
correct up to 16 corrupted bytes per codeword. In Reed-Solomon
terminology, each byte is called a "symbol", and the key property
is that it does not matter how badly a byte is damaged --- whether
one bit is flipped or all eight, it counts as a single symbol
error.

For burst error resilience, the standard supports interleaving
depths $I = 1$ to $5$. With interleaving, $I$ codewords are
symbol-interleaved into a single block of $I times 255$ bytes. A
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
$g(x) = product_(i=0)^(31) (x - alpha^(112+i))$; the remainder of
this division becomes the 32 parity bytes. Decoding uses the
Berlekamp--Massey algorithm to find the error locator polynomial,
Chien search to find error positions, and the Forney algorithm to
compute error magnitudes.

RS is the most widely deployed FEC in space communications.
It is computationally inexpensive, has deterministic decoding
latency, and its byte-level correction is well-suited to the
symbol-level errors that occur after demodulation.

=== LDPC --- AR4JA (131.0-B-5)

CCSDS specifies a family of Accumulate Repeat-by-4 Jagged
Accumulate (AR4JA) LDPC codes at six code rates: 1/2, 2/3, 4/5,
and 7/8 at three information block sizes (1024, 4096, 16384 bits).
LDPC codes achieve error correction performance close to the
Shannon limit --- the theoretical maximum rate at which information
can be transmitted over a noisy channel with arbitrarily low error
rate. In practice, this means LDPC can operate at lower
signal-to-noise ratios than RS for the same error rate.

Encoding multiplies the information bits by a sparse generator
matrix. Decoding uses iterative belief propagation on the
parity-check matrix: each bit node and check node exchange
messages about the likelihood of each bit being 0 or 1, refining
the estimates over multiple iterations until convergence. The
decoder accepts soft-decision input (LLRs from the demodulator),
which provides several dB (decibels --- a logarithmic measure of
signal power ratio) of additional gain compared to hard-decision
decoding where each bit is simply 0 or 1 with no confidence
information.

LDPC is preferred when the link budget is tight --- for example,
on long-range ISL links or during periods of high atmospheric
attenuation. The trade-off is higher computational cost and
variable decoding latency.

=== Convolutional Code (131.0-B-5)

The CCSDS convolutional code uses rate 1/2 (one output bit for
every input bit, doubling the data rate) and constraint length 7
(each output bit depends on the current input bit and the previous
6). Decoding uses the Viterbi algorithm, which finds the most
probable transmitted sequence. It works by maintaining $2^6 = 64$
possible encoder states (a _trellis_ --- a graph of states over
time) and pruning unlikely paths as each new bit arrives, keeping
only the best path into each state.

Convolutional coding provides the highest _coding gain_ (the
reduction in required SNR for a given error rate) of the three
options, at the cost of halving the effective data rate. It is
specified for deep-space missions where the signal arrives
extremely weak and every fraction of a dB matters. It is often
concatenated with RS: the convolutional code handles random bit
errors while RS handles the residual burst errors that the Viterbi
decoder occasionally produces.

== Framing

After FEC encoding, the coded data must be framed so that the
receiver can find the start of each block in the continuous
bitstream. CCSDS uses different framing for the TM (downlink) and
TC (uplink) directions.

=== ASM / CADU --- TM direction (131.0-B-5)

A Channel Access Data Unit (CADU) consists of an Attached Sync
Marker (ASM) followed by the coded transfer frame. The standard TM
ASM is the 32-bit pattern `0x1ACFFC1D`, chosen because it has a
sharp _autocorrelation peak_: when the receiver compares this
pattern against the incoming bitstream (sliding it one bit at a
time), the match score jumps sharply at the correct position and
is low everywhere else. This makes it easy to detect the exact
frame boundary even in noisy conditions. Proximity-1 links use a shorter
24-bit ASM (`0xFAF320`).

The receiver's frame synchronizer searches for the ASM pattern,
then knows that the next $N$ bytes are the coded frame. By
checking for the ASM at the expected offset after the current
frame, the receiver can achieve flywheel synchronization: once
locked, it expects frames at regular intervals and can tolerate
brief signal dropouts.

=== CLTU --- TC direction (231.0-B-4)

A Communications Link Transmission Unit (CLTU) wraps a TC transfer
frame for uplink. The structure differs from CADU because uplink
commands must be received with very high reliability --- an
incorrect command could endanger the mission.

The CLTU begins with a 2-byte start sequence (`0xEB90`) that the
receiver uses to detect the beginning of a command. The TC frame
is then split into 7-byte blocks, and each block is appended with
a 1-byte parity computed using a BCH(63,56) code --- a type of
error-detecting code that can detect (but not correct) errors
within the block. The CLTU ends with an 8-byte tail sequence
(`0xC5C5C5C5C5C5C5C5`).

The receiver processes each 8-byte block independently. If the BCH
check fails, the receiver knows the block is corrupted and can
reject the entire CLTU. This per-block error detection complements
the FEC: even if the FEC introduces a miscorrection, the BCH check
provides a second line of defense.

== Data Compression

In addition to error correction, the coding layer provides data
compression for payload data (not transfer frames). These
algorithms reduce the volume of data that must traverse the
communication stack, which is critical when downlink bandwidth is
the bottleneck.

=== Rice Coding (121.0-B-3)

Lossless compression for sensor data. Uses adaptive entropy coding
that is efficient for data with exponential-like distributions
(e.g. prediction residuals from a linear predictor). The encoder
splits each sample into a most-significant and least-significant
part, encoding the former with a unary code and the latter with a
fixed-length binary code. This is optimal when the prediction
residuals follow a geometric distribution.

=== Image Data Compression (122.0-B-2)

Wavelet-based image compression using the integer 5/3 discrete
wavelet transform (DWT). The DWT decomposes the image into
frequency sub-bands; a bit-plane encoder then transmits the most
significant bits first. Compression can be lossless (all bit
planes transmitted) or lossy (truncated at a target bit rate).

=== Multispectral/Hyperspectral Compression (123.0-B-2)

Lossless compression specifically designed for multispectral and
hyperspectral image cubes. Exploits both spatial correlation
(neighbouring pixels in the same band) and spectral correlation
(the same pixel across bands) using a linear prediction model. The
prediction residuals are then entropy-coded.

#pagebreak()

= Physical Layer

The physical layer moves raw bits between the flight computer and
the radio. Everything above this layer operates on bytes and
frames; the physical layer is responsible for the boundary between
digital data and the analog RF signal.

== Modulation

Modulation converts bits into _symbols_ --- discrete waveform
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

=== BPSK

Binary Phase Shift Keying uses two phases (0° and 180°), carrying
1 bit per symbol. Phase-shift keying (PSK) works by varying the
_phase_ of the carrier signal --- the position of the waveform
within its cycle. The receiver compares the received phase to the
expected phase to determine which symbol was sent. BPSK is the
simplest and most noise-tolerant PSK scheme. Used on links where
the signal is weak relative to noise (low signal-to-noise ratio,
or SNR).

=== QPSK

Quadrature PSK uses four phases (0°, 90°, 180°, 270°), carrying
2 bits per symbol. Doubles throughput relative to BPSK at a modest
noise penalty. The signal has two independent components: in-phase
(I) and quadrature (Q), each carrying one bit.

=== OQPSK

Offset QPSK is a variant of QPSK that staggers the I and Q
components by half a symbol period. This prevents the signal
amplitude from dropping to zero during phase transitions, which
matters because spacecraft power amplifiers are non-linear:
amplitude drops cause distortion. OQPSK avoids this, which is why
CCSDS specifies it for Proximity-1 inter-spacecraft links.

=== 8PSK

Uses eight phases, carrying 3 bits per symbol. The phases are
assigned using Gray coding, meaning adjacent phase states differ by
only one bit --- so if noise causes the receiver to pick a
neighbouring phase, only one bit is wrong. Higher throughput, but
requires a stronger signal (better _link budget_ --- the overall
margin between transmitted power and minimum receivable power).

=== GMSK

Gaussian Minimum Shift Keying keeps the signal amplitude constant
(constant envelope), which allows the power amplifier to operate at
its most efficient point (saturation). Used where electrical power
is scarce, which is common on small satellites.

== Hardware Interfaces

The flight computer communicates with peripherals (radios, sensors,
actuators) over hardware buses. NOS3 hwlib provides drivers for
seven bus types, all of which are link-time substituted in
simulation: the same FSW binary runs on real hardware or in NOS3,
with only the linked driver library changing.

=== UART

Universal Asynchronous Receiver/Transmitter. A serial bus that
sends bytes one bit at a time over a wire. The standard interface
between the flight computer and the radio. The FSW writes bytes to
the UART transmit register; the radio modulates them onto the
carrier. In the receive direction, the radio demodulates the
incoming signal and presents bytes on the UART receive register.

=== SPI

Serial Peripheral Interface. A synchronous full-duplex bus with
separate clock and data lines. Higher throughput than UART. Used
for sensors (IMU, magnetometer) and could serve as the interface
to an optical transceiver.

=== I2C

Inter-Integrated Circuit. A two-wire synchronous bus supporting
multiple devices on a shared bus with addressing. Lower throughput
than SPI but requires fewer pins. Used for low-rate sensors.

=== CAN

Controller Area Network. A differential bus designed for reliable
communication in noisy environments. Supports message
prioritization and error detection. Used in some spacecraft for
internal subsystem communication.

=== GPIO

General Purpose I/O. Direct pin-level control for simple signals
(enable lines, status flags, interrupt triggers).

=== UDP / TCP

Network sockets used in simulation and ground station links. UDP
provides low-latency datagram delivery; TCP provides reliable byte
streams. In NOS3, the radio simulator uses UDP to forward data
between the flight software and the ground station software.

#pagebreak()

= Layered Recovery

The reliability mechanisms at different layers are complementary,
not redundant. Each layer handles a class of failure that the
layers below cannot see.

== Why COP-1 alone is not sufficient

COP-1 runs independently on each hop. Consider a three-hop path:

#align(center)[
  Sat A #sym.arrow.r Sat B #sym.arrow.r Sat C #sym.arrow.r Sat D
]

COP-1 on the A--B link confirms that Sat B received the frame. But
Sat B's router may drop the packet before forwarding it to Sat C
--- due to queue overflow, a software fault, or a route change.
COP-1 on A--B has already reported success. Neither the sender (A)
nor the final receiver (D) knows the packet was lost.

Only an end-to-end protocol (SRSPP) between A and D can detect and
recover from this. SRSPP's sequence numbers span the entire path,
so D knows exactly which packets it has received and can request
retransmission from A.

== Why SRSPP alone is not sufficient

Without per-hop reliability, SRSPP must retransmit every packet
lost to bit errors. On a lossy RF link this means:

- Each retransmission traverses the full multi-hop path, consuming
  bandwidth on every intermediate link.
- Each retransmission is itself subject to the same per-hop loss
  rate.
- Throughput degrades geometrically with hop count: a 1% frame
  loss rate per hop becomes $(1 - 0.99^n)$ end-to-end loss for an
  $n$-hop path.

With COP-1 on each hop, frame losses are recovered locally in one
link round-trip time. SRSPP only needs to retransmit when an
intermediate router drops a packet --- a much rarer event than a
bit error.

== Recovery summary

+ A bit error on the RF link is corrected by the coding layer's
  FEC. No retransmission occurs.
+ If FEC cannot correct the damage, the corrupted frame is
  discarded. COP-1 detects the gap in the frame sequence and
  retransmits the frame on the same hop.
+ If a packet survives all hops but is dropped at an intermediate
  router, SRSPP detects the missing sequence number and
  retransmits end-to-end.

Each layer handles only the residual failures of the layer below.
The result is that SRSPP retransmissions are rare, but they remain
necessary for correctness in a multi-hop network.

#pagebreak()

= Time Codes (301.0-B-4)

CCSDS 301.0-B-4 defines standard time formats for space missions.
Time stamps appear in cFE telemetry secondary headers (6 bytes),
in protocol metadata, and in science data annotations. Two formats
are used in LeoDOS: CUC and CDS.

Both formats use a two-part encoding. The _P-field_ (preamble) is a
1-byte descriptor that identifies the time format, epoch, and field
sizes. The _T-field_ contains the actual time value. When sender and
receiver agree on the format in advance, the P-field can be omitted
and the T-field is interpreted using the implicit configuration.

Both formats reference the CCSDS epoch: *1958-01-01T00:00:00 TAI*.
TAI (International Atomic Time) is a monotonic time scale that does
not include leap seconds, which is important for spacecraft where
a discontinuity in the time reference could cause control loops to
misbehave.

== CUC --- Unsegmented Code

CUC encodes time as a binary count of whole seconds (the _coarse_
field) and fractional seconds (the _fine_ field) since the epoch.
The number of bytes for each is configurable: 1--4 bytes for coarse
and 0--3 bytes for fine. This flexibility allows trading off range
against resolution and encoded size.

The P-field encodes:

- *Time code ID* (3 bits): `001` for agency-defined epoch, `010`
  for the CCSDS epoch.
- *Coarse octets* (2 bits): the number of coarse bytes minus one
  (0--3, meaning 1--4 bytes).
- *Fine octets* (2 bits): the number of fine bytes (0--3).

Common configurations:

- *4+2* (standard): 4 coarse bytes give a range of ~136 years from
  epoch. 2 fine bytes give ~15 µs resolution
  ($1 / 2^(16) approx 15.3 "µs"$). Total T-field: 6 bytes.
- *4+0*: 4 coarse bytes, no fractional part. 1-second resolution.
  Total T-field: 4 bytes.

CUC is used in the cFE telemetry secondary header (6-byte
timestamp). It is the natural choice when the on-board clock
provides a seconds-and-ticks counter.

== CDS --- Day Segmented

CDS encodes time as a day count since the epoch plus milliseconds
within the day. It optionally includes a sub-millisecond field for
higher resolution.

The P-field encodes:

- *Time code ID* (3 bits): `100` for CDS.
- *Epoch ID* (1 bit): 0 = CCSDS epoch, 1 = agency-defined.
- *Day segment length* (1 bit): 0 = 16-bit day count (range
  ~179 years), 1 = 24-bit day count (range ~45,000 years).
- *Sub-millisecond resolution* (2 bits): `00` = none, `01` = 16-bit
  microseconds (0--999 µs), `10` = 32-bit picoseconds
  (0--999,999,999 ps).

Common configurations:

- *16-bit day, no sub-ms*: 2 day + 4 ms = 6-byte T-field.
  Millisecond resolution, ~179 year range.
- *16-bit day, µs*: 2 day + 4 ms + 2 µs = 8-byte T-field.
  Microsecond resolution.
- *24-bit day, ps*: 3 day + 4 ms + 4 ps = 11-byte T-field.
  Picosecond resolution, extended range.

CDS is suited for ground systems and science data where wall-clock
time (day + time-of-day) is more natural than a raw second count.

== Choosing Between CUC and CDS

CUC is compact and maps directly to hardware tick counters. CDS is
human-readable (day + milliseconds) and convenient for correlating
events with calendar dates. In practice, flight software uses CUC
for telemetry timestamps and CDS for ground-originated time
references and science data annotations.

#pagebreak()

= Type System Composition

The communication stack is implemented as a set of Rust traits and
generic types. Each layer defines a trait boundary, and concrete
types compose by holding an inner type that implements the trait of
the layer below. This section describes every trait, every type
that implements it, and how they nest.

== Physical Layer

=== Traits

```rust
trait AsyncPhysicalWriter {
    type Error;
    async fn write(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
}

trait AsyncPhysicalReader {
    type Error;
    async fn read(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Raw byte I/O with hardware.

=== Implementations

```rust
impl AsyncPhysicalWriter for UartChannel { .. }
impl AsyncPhysicalReader for UartChannel { .. }
```

`UartChannel` (behind the `cfs` feature) wraps the hwlib UART
driver. In simulation, the UART is routed through NOS Engine to
the radio simulator; on real hardware, the same code talks to the
physical radio.

== Coding Layer

=== Traits

The coding layer currently has no trait boundary of its own. It
needs two: one facing the data link layer above (accepting transfer
frames) and one facing the physical layer below (writing/reading
coded bytes). These traits do not exist yet.

```rust
// Needed: accepts a transfer frame, applies the full
// coding chain (randomize → FEC → framing), and writes
// the result to the physical layer.
trait CodedFrameSender {
    type Error;
    async fn send(&mut self, frame: &[u8])
        -> Result<(), Self::Error>;
}

// Needed: reads coded bytes from the physical layer,
// applies the inverse chain (sync → FEC decode →
// derandomize), and returns the transfer frame.
trait CodedFrameReceiver {
    type Error;
    async fn recv(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Additionally, the `Randomizer` trait exists for the randomization
step:

```rust
trait Randomizer {
    fn apply(&self, data: &mut [u8]);
    fn table(&self) -> &[u8];
}
```

=== Implementations

Randomizer implementors:

```rust
impl Randomizer for TcRandomizer { .. }
impl Randomizer for Tm255Randomizer { .. }
impl Randomizer for Tm131071Randomizer { .. }
```

Standalone coding functions (no trait, called directly):

```rust
fn reed_solomon::encode(..);
fn reed_solomon::decode(..);
fn cadu::encode_cadu(..);
fn cadu::decode_cadu(..);
fn cltu::encode_cltu(..);
fn ldpc::encode(..);
fn ldpc::decode(..);
fn convolutional::encode(..);
fn convolutional::viterbi_decode(..);
fn rice::compress(..);
fn rice::decompress(..);
fn ccsds122::compress(..);
fn ccsds122::decompress(..);
fn ccsds123::compress(..);
fn ccsds123::decompress(..);
```

No types implement `CodedFrameSender` or `CodedFrameReceiver` yet.
The TM sender driver calls `Randomizer::apply` directly; the other
coding functions are not called anywhere in the pipeline.

== Data Link Layer

=== Traits

```rust
trait FrameSender {
    type Error: core::error::Error;
    async fn send(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
}

trait FrameReceiver {
    type Error: core::error::Error;
    async fn recv(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

`FrameSender` and `FrameReceiver` are the lower boundary of the
data link layer --- the interface through which frame drivers write
to and read from the link.

```rust
trait DataLink {
    type Error: core::error::Error;
    async fn send(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
    async fn recv(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

`DataLink` combines both directions into what the network layer
holds per link.

=== Implementations

`FrameSender` implementors:

```rust
impl FrameSender for UdpFrameSender { .. }
impl FrameSender for PipeFrameSender { .. }
impl FrameSender for TmSenderHandle { .. }
impl FrameSender for TcSenderHandle { .. }
```

`UdpFrameSender` sends one UDP datagram per frame.
`PipeFrameSender` publishes to a cFS Software Bus message ID.
`TmSenderHandle` and `TcSenderHandle` enqueue into a shared
channel (see drivers below).

`FrameReceiver` implementors:

```rust
impl FrameReceiver for UdpFrameReceiver { .. }
impl FrameReceiver for PipeFrameReceiver { .. }
impl FrameReceiver for TmReceiverHandle { .. }
impl FrameReceiver for TcReceiverHandle { .. }
```

`DataLink` implementors:

```rust
impl DataLink for UdpDataLink { .. }
impl DataLink for AsymmetricLink { .. }
impl DataLink for LocalRouterHandle { .. }
```

`UdpDataLink` wraps a single bidirectional UDP socket.
`AsymmetricLink` holds separate sender and receiver halves.
`LocalRouterHandle` is the router-side of an in-process channel.

Background drivers (no trait --- run via `run()` method):

```rust
TmSenderDriver<W: FrameSender>
TcSenderDriver<W: FrameSender>
TmReceiverDriver<R: FrameReceiver>
TcReceiverDriver<R: FrameReceiver>
```

The TM/TC channels use a handle/driver split to allow send and
receive loops to run concurrently on a single-threaded async
executor (as required by cFS). The handle is held by the
application task; the driver runs as a separate task. They share
state through a `RefCell`-guarded queue. The TM driver builds a
`TelemetryTransferFrame`, applies `Randomizer::apply`, and calls
`FrameSender::send`. The TC driver builds a
`TelecommandTransferFrame` and calls `FrameSender::send`.

== Network Layer

=== Traits

```rust
trait NetworkLayer {
    type Error: core::error::Error;
    async fn send(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
    async fn recv(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

=== Implementations

```rust
impl<L: DataLink> NetworkLayer for PassThrough<L> { .. }
impl NetworkLayer for LocalAppHandle { .. }
```

`PassThrough` forwards directly to one `DataLink` without routing.
`LocalAppHandle` is the app-side of an in-process channel to the
router.

The `Router` does not implement `NetworkLayer` itself. It holds
six `DataLink` instances and a routing algorithm, and runs as a
background task that polls all links and forwards packets. The
application communicates with the router through a `LocalChannel`,
whose `LocalAppHandle` implements `NetworkLayer`.

```rust
Router<N, S, E, W, G, L, R>
// N, S, E, W, G, L: DataLink; R: RoutingAlgorithm
```

== Transport Layer

=== Traits

```rust
trait TransportSender {
    type Error;
    async fn send(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
}

trait TransportReceiver {
    type Error;
    async fn recv(&mut self, buf: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

These traits are defined but have no implementors today.

=== Implementations

The SRSPP and CFDP types provide their own `send`/`recv` methods
without going through the transport traits.

```rust
SrsppSender<L: NetworkLayer, P: RtoPolicy>
SrsppReceiver<L: NetworkLayer, R: ReceiverBackend>
```

Both have `pub async fn send()`/`recv()` but do not implement
`TransportSender`/`TransportReceiver`.

On cFS, SRSPP uses the same handle/driver split as the data link
layer:

```rust
SrsppTxHandle
SrsppSenderDriver<L: NetworkLayer>
SrsppRxHandle
SrsppReceiverDriver<L: NetworkLayer>
```

CFDP operates as independent state machines:

```rust
SendingMachine
ReceivingMachine
```

== Concrete Composition

The TM (downlink) send path illustrates how the types nest:

```
SrsppSender<LocalAppHandle, ..>
     │
     ├─ holds LocalAppHandle (impl NetworkLayer)
     │       │
     │       └─ communicates with Router via LocalChannel
     │
     └─ Router runs in background
             │
             ├─ holds AsymmetricLink per direction
             │       (impl DataLink)
             │       │
             │       └─ holds TmSenderHandle (FrameSender)
             │               │
             │               └─ enqueues into TmSenderChannel
             │
             └─ TmSenderDriver runs in background
                     │
                     ├─ dequeues from TmSenderChannel
                     ├─ builds TelemetryTransferFrame
                     ├─ applies Randomizer::apply
                     └─ calls FrameSender::send
                             │
                             └─ UdpFrameSender (today)
```

== What Does Not Compose Yet

Three gaps prevent a complete end-to-end pipeline:

=== FrameSender to physical layer

The `TmSenderDriver` and `TcSenderDriver` hold a generic
`W: FrameSender`. At the bottom of the stack, something must
implement `FrameSender` by writing bytes to the physical channel.
Today this role is filled by `UdpFrameSender` and `UdpDataLink`
(which send UDP datagrams). There is no adapter that implements
`FrameSender` by calling `AsyncPhysicalWriter::write`, so the
UART-based physical channel cannot be used as a frame sender
without manual glue.

=== Coding in the pipeline

The coding functions (Reed-Solomon, CADU framing, CLTU encoding)
exist as standalone `encode`/`decode` calls. The TM driver calls
the randomizer directly, but Reed-Solomon encoding, ASM framing,
and CLTU encoding are not called anywhere in the send/receive path.
To fully encode a downlink frame, the driver would need to:

+ Randomize the frame (already done).
+ Reed-Solomon encode the randomized frame.
+ Prepend the ASM to form a CADU.
+ Write the CADU to the physical channel.

A symmetric decode path is needed on the receive side.

=== COP-1 and SDLS integration

The COP-1 state machines (FOP-1 sender, FARM-1 receiver) and the
SDLS `apply_security`/`process_security` functions are fully
implemented but are not called from the TC/TM drivers. The frame
drivers currently send frames directly without sequence checking,
retransmission, or encryption. Integrating these requires inserting
COP-1 and SDLS as stages in the driver's send/receive path, between
frame construction and the call to `FrameSender::send`.

#pagebreak()

= Wanted Design

The current design has traits only at layer boundaries and uses
inconsistent names (`NetworkLayer`, `DataLink`, `FrameSender`,
`AsyncPhysicalWriter`). The wanted design uses a uniform naming
convention and adds traits for each _group_ within the physical,
coding, and data link layers, matching the groups in the stack
diagram.

Every layer boundary uses the same pattern: the layer name with a
`Writer`/`Reader` suffix for the send/receive directions. The
combined traits (`NetworkLayer`, `DataLink`) are dropped --- only
the directional variants exist. The `TransportSender`/
`TransportReceiver` traits are removed since they have no
implementors.

```
Transport  holds  W: NetworkWriter,  R: NetworkReader
Network    holds  W: DataLinkWriter, R: DataLinkReader
DataLink   holds  W: CodingWriter,   R: CodingReader
Coding     holds  W: PhysicalWriter, R: PhysicalReader
```

All four trait pairs have the same shape:

```rust
trait <Layer>Writer {
    type Error;
    async fn write(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
}

trait <Layer>Reader {
    type Error;
    async fn read(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

== Physical Layer

=== Hardware

```rust
trait PhysicalWriter {
    type Error;
    async fn write(&mut self, data: &[u8])
        -> Result<(), Self::Error>;
}

trait PhysicalReader {
    type Error;
    async fn read(&mut self, buffer: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

One implementor per hwlib bus type:

```rust
impl PhysicalWriter for UartChannel { .. }
impl PhysicalReader for UartChannel { .. }

impl PhysicalWriter for SpiChannel { .. }
impl PhysicalReader for SpiChannel { .. }

impl PhysicalWriter for I2cChannel { .. }
impl PhysicalReader for I2cChannel { .. }

impl PhysicalWriter for CanChannel { .. }
impl PhysicalReader for CanChannel { .. }

impl PhysicalWriter for UdpChannel { .. }
impl PhysicalReader for UdpChannel { .. }

impl PhysicalWriter for TcpChannel { .. }
impl PhysicalReader for TcpChannel { .. }
```

Today only `UartChannel` exists. The others are needed to
support all hwlib bus types. GPIO is excluded because it is
pin-level (single bits, not byte streams).

=== Modulation

```rust
trait Modulator {
    fn modulate(&self, bits: &[u8], symbols: &mut [f32]);
}

trait Demodulator {
    fn demodulate(&self, symbols: &[f32], bits: &mut [u8]);
    fn demodulate_soft(&self, symbols: &[f32],
        llrs: &mut [f32]);
}
```

Swappable modulation schemes. `demodulate_soft` produces
log-likelihood ratios for soft-decision FEC decoding.

```rust
impl Modulator for Bpsk { .. }
impl Modulator for Qpsk { .. }
impl Modulator for Oqpsk { .. }
impl Modulator for EightPsk { .. }
impl Modulator for Gmsk { .. }

impl Demodulator for Bpsk { .. }
impl Demodulator for Qpsk { .. }
impl Demodulator for Oqpsk { .. }
impl Demodulator for EightPsk { .. }
impl Demodulator for Gmsk { .. }
```

On real hardware modulation is handled by the radio. These are
used for software simulation and for computing LLRs.

== Coding Layer

=== Randomization

```rust
trait Randomizer {
    fn apply(&self, data: &mut [u8]);
}
```

XORs data with a deterministic pseudo-random sequence. Self-inverse
(apply twice to recover original).

```rust
impl Randomizer for TcRandomizer { .. }
impl Randomizer for Tm255Randomizer { .. }
impl Randomizer for Tm131071Randomizer { .. }
```

=== Forward Error Correction

```rust
trait FecEncoder {
    type Error;
    fn encode(&self, data: &[u8], output: &mut [u8])
        -> Result<usize, Self::Error>;
}

trait FecDecoder {
    type Error;
    fn decode(&self, data: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Swappable FEC schemes. Only one is used per link.

```rust
impl FecEncoder for ReedSolomon { .. }
impl FecDecoder for ReedSolomon { .. }

impl FecEncoder for LdpcEncoder { .. }
impl FecDecoder for LdpcDecoder { .. }

impl FecEncoder for ConvolutionalEncoder { .. }
impl FecDecoder for ViterbiDecoder { .. }
```

=== Framing

```rust
trait Framer {
    type Error;
    fn frame(&self, data: &[u8], output: &mut [u8])
        -> Result<usize, Self::Error>;
}

trait Deframer {
    type Error;
    fn deframe(&self, data: &[u8], output: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Wraps coded data for transmission (ASM for TM, CLTU for TC).

```rust
impl Framer for AsmFramer { .. }
impl Deframer for AsmDeframer { .. }

impl Framer for CltuFramer { .. }
```

=== Data Compression

```rust
trait Compressor {
    type Error;
    fn compress(&self, input: &[u8], output: &mut [u8])
        -> Result<usize, Self::Error>;
}

trait Decompressor {
    type Error;
    fn decompress(&self, input: &[u8], output: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Applied to payload data, not transfer frames. Independent of the
frame pipeline.

```rust
impl Compressor for RiceCompressor { .. }
impl Decompressor for RiceDecompressor { .. }

impl Compressor for DwtCompressor { .. }
impl Decompressor for DwtDecompressor { .. }

impl Compressor for HyperspectralCompressor { .. }
impl Decompressor for HyperspectralDecompressor { .. }
```

=== Coding Pipeline

The groups compose into a coding pipeline. A `CodingPipeline` type
holds one of each and implements `CodingWriter`/`CodingReader` so
the data link layer can use it:

```rust
CodingPipeline<R: Randomizer, F: FecEncoder, M: Framer,
               W: PhysicalWriter>

impl CodingWriter for CodingPipeline { .. }
```

```
Send:  frame → Randomizer → FecEncoder → Framer
                                            → PhysicalWriter

Recv:  PhysicalReader → Deframer → FecDecoder
                                       → Randomizer → frame
```

== Data Link Layer

=== Transfer Frame Protocols

```rust
trait FrameBuilder {
    type Error;
    fn build(&mut self, data: &[u8],
        output: &mut [u8]) -> Result<usize, Self::Error>;
}

trait FrameParser {
    type Error;
    fn parse<'a>(&mut self, frame: &'a [u8])
        -> Result<&'a [u8], Self::Error>;
}
```

Swappable frame formats. The builder wraps payload data in a
transfer frame; the parser extracts the payload.

```rust
impl FrameBuilder for TmFrameBuilder { .. }
impl FrameParser for TmFrameParser { .. }

impl FrameBuilder for TcFrameBuilder { .. }
impl FrameParser for TcFrameParser { .. }

impl FrameBuilder for AosFrameBuilder { .. }
impl FrameParser for AosFrameParser { .. }

impl FrameBuilder for Proximity1FrameBuilder { .. }
impl FrameParser for Proximity1FrameParser { .. }

impl FrameBuilder for UslpFrameBuilder { .. }
impl FrameParser for UslpFrameParser { .. }
```

=== Security

```rust
trait SecurityProcessor {
    type Error;
    fn apply(&mut self, frame: &mut [u8])
        -> Result<usize, Self::Error>;
    fn process(&mut self, frame: &mut [u8])
        -> Result<usize, Self::Error>;
}
```

Encrypts/authenticates frames after building, decrypts/verifies
before parsing. Can be a no-op for unsecured links.

```rust
impl SecurityProcessor for SdlsProcessor { .. }
impl SecurityProcessor for NoSecurity { .. }
```

=== Reliability

```rust
trait ReliabilitySender {
    type Error;
    fn send(&mut self, frame: &[u8])
        -> FopAction;
}

trait ReliabilityReceiver {
    type Error;
    fn receive(&mut self, frame: &[u8])
        -> FarmAction;
}
```

Wraps the COP-1 state machines. The sender (FOP-1) assigns
sequence numbers and manages retransmission. The receiver (FARM-1)
checks sequence numbers and generates CLCWs. Can be bypassed for
links that don't need per-hop reliability.

```rust
impl ReliabilitySender for Fop { .. }
impl ReliabilityReceiver for Farm { .. }

impl ReliabilitySender for NoReliability { .. }
impl ReliabilityReceiver for NoReliability { .. }
```

=== Data Link Pipeline

The three groups compose into the data link send/receive path.
A `DataLinkPipeline` type holds one of each and implements
`DataLinkWriter`/`DataLinkReader`:

```rust
DataLinkPipeline<B: FrameBuilder, S: SecurityProcessor,
                 R: ReliabilitySender, W: CodingWriter>

impl DataLinkWriter for DataLinkPipeline { .. }
```

```
Send:  packet → FrameBuilder → SecurityProcessor
                  → ReliabilitySender → CodingWriter

Recv:  CodingReader → ReliabilityReceiver
         → SecurityProcessor → FrameParser → packet
```

== Full Pipeline

With all group traits in place, the complete TM downlink path
from application to hardware:

```
Application (SpaceCoMP)
  → SrsppSender            (transport)
  ┄┄┄┄┄┄┄ NetworkWriter ┄┄┄┄┄┄┄
    → Router              (network: background task)
    ┄┄┄┄┄ DataLinkWriter ┄┄┄┄┄┄
      → FrameBuilder      (datalink: TM frame)
      → SecurityProcessor (datalink: SDLS)
      → ReliabilitySender (datalink: COP-1)
      ┄┄┄┄┄ CodingWriter ┄┄┄┄┄┄
        → Randomizer         (coding)
        → FecEncoder         (coding: RS)
        → Framer             (coding: ASM)
        ┄┄┄ PhysicalWriter ┄┄┄┄
          → UartChannel        (physical: UART)
```

Each `→` is a trait boundary. Any component can be swapped by
providing a different implementor of the same trait.
