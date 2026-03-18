# Convolutional

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
