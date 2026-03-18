# OQPSK

Offset QPSK is a variant of QPSK that staggers the I and Q
components by half a symbol period. This prevents the signal
amplitude from dropping to zero during phase transitions, which
matters because spacecraft power amplifiers are non-linear:
amplitude drops cause distortion. OQPSK avoids this, which is why
CCSDS specifies it for Proximity-1 inter-spacecraft links.
