# ASM / CADU

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
