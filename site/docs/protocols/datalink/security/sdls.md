# SDLS

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
