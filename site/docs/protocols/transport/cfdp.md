# CFDP

The CCSDS File Delivery Protocol (727.0-B-5) provides reliable
file transfer. Unlike SRSPP which delivers messages, CFDP
transfers named files of arbitrary size with metadata.

CFDP Class 2 (acknowledged mode) uses a state machine with the
following phases:

1. **Metadata**: The sender transmits a Metadata PDU (Protocol Data
   Unit --- the CFDP term for a single message exchanged between
   sender and receiver) containing the file name, size, and options.
2. **File Data**: The sender transmits File Data PDUs containing
   successive chunks of the file.
3. **EOF**: The sender transmits an EOF PDU with a checksum of the
   complete file.
4. **NAK**: If the receiver detects missing chunks, it sends NAK
   (Negative Acknowledgment) PDUs listing the gaps. The sender
   retransmits the missing data.
5. **Finished**: The receiver confirms the file is complete and
   intact. The sender sends a final ACK.

CFDP manages concurrent file transfers using transaction IDs.
Each transfer is an independent state machine, and multiple
transfers can proceed in parallel over the same link.

The file I/O is abstracted behind a platform-independent filestore
trait, allowing CFDP to work on any system that can read and write
files --- whether that is a Linux filesystem on the ground or a
flash-based filesystem on a satellite.
