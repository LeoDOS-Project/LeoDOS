# Operation

## Sender Operation

The sender maintains a window of unacknowledged packets. Each packet in the window
is in one of two states:

- **Pending Transmit**: Queued but not yet sent, or marked for retransmission
- **Awaiting ACK**: Transmitted and waiting for acknowledgment

### Send Flow

1. Application submits a message to send
2. If message exceeds MTU, segment it into multiple packets (FIRST/CONTINUATION/LAST)
3. For each packet:
   - Assign the next sequence number
   - Store packet in send buffer
   - Mark as Pending Transmit
4. Transmit Pending Transmit packets up to the window limit — if the window is full, remaining packets stay queued until ACKs free window slots
5. Start retransmission timer for each transmitted packet
6. Mark transmitted packets as Awaiting ACK

### ACK Processing

When an ACK arrives:

1. For each packet covered by the cumulative ACK (seq ≤ cumulative\_ack):
   - Stop its retransmission timer
   - Remove from send buffer
2. For each bit set in the selective bitmap: bit N means the receiver has packet N positions beyond the cumulative ACK (i.e., sequence number cumulative\_ack + 1 + N). For each such packet:
   - Stop its retransmission timer
   - Remove from send buffer
3. Slide the window forward

### Timeout Handling

When a retransmission timer expires:

1. If retransmit count has not reached the maximum:
   - Increment retransmit count
   - Mark packet as Pending Transmit
   - Retransmit the packet
   - Restart timer
2. Otherwise:
   - Declare packet lost
   - Remove from send buffer
   - Signal error to application

### Stream Termination

To signal end of transmission:

1. Application requests stream close
2. Send EOS packet with next sequence number
3. Wait for ACK covering the EOS sequence
4. Transfer is complete when EOS is acknowledged

The EOS is retransmitted like DATA if no ACK arrives.

## Receiver Operation

The receiver maintains the expected sequence number and a reorder buffer for
out-of-order packets.

### Receive Flow

When a DATA or EOS packet arrives:

1. Compare packet sequence to expected sequence
2. If sequence matches expected (in-order):
   - If DATA: deliver payload to reassembly
   - If EOS: signal stream complete to application
   - Advance expected sequence
   - Check reorder buffer for now-deliverable packets
   - Repeat until no more consecutive packets
3. If sequence is ahead of expected but within the window (out-of-order):
   - Store in reorder buffer
   - Set corresponding bit in selective bitmap
4. If sequence is behind expected (duplicate):
   - Ignore
5. Schedule or send ACK (see ACK Generation)

### Reassembly

As packets are delivered in order:

1. Check sequence flag:
   - UNSEGMENTED: Complete message, deliver to application
   - FIRST: Start new reassembly buffer
   - CONTINUATION: Append to reassembly buffer
   - LAST: Append and deliver complete message to application

### ACK Generation

After processing each DATA packet:

1. If immediate_ack mode:
   - Send ACK immediately
2. If delayed_ack mode:
   - If no ACK timer running, start one
   - When timer expires, send ACK

The ACK contains:
- Cumulative ACK = expected_seq - 1 (highest in-order seq received)
- Selective bitmap = bits for each buffered out-of-order packet

## Version Handling

The SRSPP header includes a 2-bit version field. When a packet arrives with an
unrecognized version:

1. Discard the packet silently
2. Do not send an ACK (sender will retransmit or timeout)
3. Optionally log the event for diagnostics

This allows future protocol versions to coexist during transitions. Endpoints
should be upgraded to matching versions for reliable communication.

## Error Handling

### Sender Errors

- **Send buffer full** — block or reject new messages until space is available
- **Window full** — block until ACKs free window slots
- **Packet lost** (max retransmits exceeded) — signal error to the application and remove the packet

### Receiver Errors

- **Reorder buffer full** — drop the out-of-order packet (the sender will retransmit)
- **Message too large for REASM** — discard segments and signal reassembly error to the application
- **CONTINUATION without FIRST** — discard the segment and signal reassembly error
- **Unknown packet type** — discard silently
- **Unknown version** — discard silently

Receiver errors do not generate negative acknowledgments. The sender retransmits based on timeouts.
