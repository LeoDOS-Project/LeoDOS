# Oil Spill

Early detection of oil spills at sea enables faster response and
reduces environmental damage.

**Sensor:** SAR (C-band or X-band).

**Pipeline:**
- _Collect:_ SAR image over monitored shipping lanes or offshore
  platforms.
- _Map:_ detect dark spots on ocean surface (oil dampens capillary
  waves, reducing SAR backscatter). Apply adaptive threshold relative
  to surrounding sea state.
- _Reduce:_ classify dark spots by shape (elongated = likely spill,
  circular = natural slick/low-wind zone). Filter by area (> minimum
  spill size). Generate alert with spill extent estimate.

**Alert payload:** ~2 KB (centroid, estimated area km^2, elongation
ratio, heading, wind speed context, timestamp).

**Feasibility:**
- Dark-spot detection in SAR is computationally cheap (thresholding +
  connected components).
- False positive discrimination (oil vs lookalikes like algae, low-wind
  zones) is the hard part; simple shape heuristics help, ML-based
  classifiers need more compute.
- No baseline needed --- each image is self-contained.
