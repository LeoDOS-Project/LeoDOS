# Satellite ↔ Satellite

Satellites in a constellation communicate directly with each other via inter-satellite links (ISL). In a [Walker Delta constellation](/spacecomp/constellation), each satellite maintains links to four neighbors forming a 2D torus mesh. Data can be routed across multiple hops to reach any satellite in the constellation.

## ISL Technologies

- **Optical (laser)** — 10–100+ Gbps bandwidth, low power for the data rate, but requires precise pointing between moving platforms (beam divergence is very small). Used by Starlink and planned for most new constellations.
- **RF (radio)** — lower bandwidth (hundreds of Mbps to low Gbps) but wider beam width, making acquisition and tracking simpler. More tolerant of pointing errors.

ISL latency within LEO is 1–5 ms per hop, depending on the distance between satellites. A path across 10 hops takes tens of milliseconds — comparable to terrestrial internet latency.

## Distance and Throughput

ISL link quality degrades with distance. The signal weakens according to free-space path loss, which grows with the square of distance. A link that is twice as long has four times the path loss, resulting in lower signal-to-noise ratio and reduced channel capacity. This means longer links carry less data per second, not just with more delay.

In a Walker Delta constellation, cross-plane link distances vary by ~40% over each orbit — shortest near the poles (where orbital planes converge) and longest near the equator (where they diverge). This variation directly affects throughput: the same cross-plane hop carries more data near the poles than near the equator. [Distance-minimizing routing](/spacecomp/routing) exploits this by scheduling cross-plane hops when they are shortest.

Both RF and optical ISLs are subject to free-space path loss — it applies to any electromagnetic signal in free space, regardless of wavelength. Optical links handle it better because their narrow beam (microradians) concentrates more power on the receiver, achieving higher SNR at the same distance. But the narrow beam requires sub-microradian pointing accuracy between two platforms moving at 7.5 km/s.

## Optical ISLs vs Fiber

Optical ISLs and terrestrial fiber are both photonic links — laser light carrying data. But they operate in fundamentally different regimes.

**Advantages of optical ISLs over fiber:**

- **Faster propagation** — light travels at full speed in vacuum (299,792 km/s), about 47% faster than in glass fiber (~200,000 km/s). For latency-sensitive paths, an ISL mesh can beat a terrestrial fiber route even if the physical distance is longer.
- **No infrastructure** — no cables to lay, no landing rights, no undersea repeaters. A constellation creates a global mesh network by launching satellites. Adding capacity means launching more satellites, not digging trenches.
- **No weather or terrain constraints** — fiber routes must avoid mountains, oceans, and political boundaries. ISLs operate in vacuum and are unaffected by weather.

**Disadvantages of optical ISLs compared to fiber:**

- **Signal diverges with distance** — in fiber, light is guided inside a glass core and barely spreads, regardless of how far it travels. In free space, the beam diverges and the power spreads over a larger area. The receiver captures a smaller fraction as distance increases, following the inverse square law. This means ISL capacity drops with distance, while fiber capacity does not.
- **No amplification** — fiber networks place repeaters every 80–100 km to restore the signal. An ISL must cover the full 1,000–5,000 km between satellites without amplification. The signal arrives weaker than it would over the same distance in fiber.
- **Pointing precision** — a fiber connector is physically mated and stays aligned. An optical ISL must aim a microradian-wide beam at a receiver on another satellite thousands of kilometers away, while both platforms move at 7.5 km/s. Acquiring and maintaining this lock is a significant engineering challenge.
- **Variable link quality** — fiber capacity between two nodes is constant (the cable doesn't change length). ISL capacity varies as orbital geometry changes — cross-plane links are stronger near the poles and weaker near the equator.
- **More overhead per bit** — lower SNR on longer links means more forward error correction (FEC) is needed to achieve the same bit error rate. A short link with high SNR can use a light FEC code and devote most of the channel bandwidth to data. A long link with low SNR needs a heavier code — more parity bits per data bit — so the effective data throughput drops even beyond what the raw capacity suggests. The [coding layer](/protocols/coding/fec/overview) (Reed-Solomon, LDPC) handles this, but the cost is real: longer links spend more bandwidth on error correction and less on useful data.
