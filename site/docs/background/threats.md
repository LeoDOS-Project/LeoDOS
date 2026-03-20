# Threats

The space environment poses physical threats to satellites that do not exist in ground-based computing.

## The Downlink Wall

LEO Earth observation satellites generate 1–2 TB of sensor data per day. Ground contact windows allow only a fraction of this to be downlinked. This is the fundamental problem LeoDOS addresses: process data onboard and downlink only the results.

| What | Size | Example |
|---|---|---|
| Raw SAR strip | ~2 GB | Full resolution radar image over a dam |
| Alert packet | ~2 KB | "Displacement exceeds 5 mm at these coordinates" |
| Reduction factor | ~10⁶ | Processing onboard avoids downlinking data the ground doesn't need |

## Radiation

LEO satellites are exposed to ionizing radiation from three sources:

- **Van Allen radiation belts** — regions of trapped charged particles (protons and electrons) held by Earth's magnetic field. The inner belt (1,000–6,000 km) contains high-energy protons; the outer belt (13,000–60,000 km) contains electrons. LEO satellites fly below the inner belt but pass through the **South Atlantic Anomaly (SAA)**, where the inner belt dips to ~200 km altitude due to the offset between Earth's geographic and magnetic poles. The SAA is the primary radiation concern for LEO missions.
- **Galactic cosmic rays** — high-energy particles from outside the solar system. Low flux but very penetrating. Cannot be shielded effectively.
- **Solar particle events** — bursts of protons from solar flares. Intermittent but can deliver a large dose in hours.

Radiation causes:
- **Single-event upsets (SEUs)** — a particle flips a bit in memory or a register. Addressed by ECC memory and software checksums (see [fault tolerance](/cfs/mission/fault-tolerance)).
- **Total ionizing dose (TID)** — cumulative damage to transistors over the mission lifetime. Addressed by radiation-hardened processor design.
- **Single-event latchups** — a particle triggers a short circuit that can only be cleared by a power cycle.

## Atmospheric Drag

At LEO altitudes, especially below 500 km, residual atmosphere exerts drag on the satellite. Drag lowers the orbit over time — without periodic orbit-raising maneuvers (using thrusters), the satellite eventually reenters the atmosphere.

Drag depends on:
- **Altitude** — drag decreases roughly exponentially with altitude. At 200 km, a satellite reenters in days; at 600 km, it can last decades.
- **Solar activity** — the Sun heats the upper atmosphere, causing it to expand. During solar maximum, drag at 400 km can be 10× higher than during solar minimum.
- **Ballistic coefficient** — the satellite's mass-to-area ratio. A large, lightweight satellite (like one with deployed solar panels) experiences more drag.

## Orbital Debris

There are over 30,000 tracked debris objects in orbit, and millions of smaller untracked fragments. A collision at orbital velocity (7–8 km/s in LEO) can destroy a satellite and generate hundreds of new fragments, each capable of destroying another satellite. This cascading effect is called the **Kessler syndrome**.

Conjunction assessment — predicting close approaches between objects — is a routine part of constellation operations. When a close approach is predicted, the satellite can perform a collision avoidance maneuver (raising or lowering its orbit slightly to increase the miss distance).
