# CLTU

A Communications Link Transmission Unit (CLTU) wraps a TC transfer
frame for uplink. The structure differs from CADU because uplink
commands must be received with very high reliability — an
incorrect command could endanger the mission.

The CLTU begins with a 2-byte start sequence (`0xEB90`) that the
receiver uses to detect the beginning of a command. The TC frame
is then split into 7-byte blocks, and each block is appended with
a 1-byte parity computed using a BCH(63,56) code — a type of
error-detecting code that can detect (but not correct) errors
within the block. The CLTU ends with an 8-byte tail sequence
(`0xC5C5C5C5C5C5C5C5`).

The receiver processes each 8-byte block independently. If the BCH
check fails, the receiver knows the block is corrupted and can
reject the entire CLTU. This per-block error detection complements
the FEC: even if the FEC introduces a miscorrection, the BCH check
provides a second line of defense.
