# Closure-based write API

## Idea

Replace `write(&mut self, data: &[u8])` with a
closure-based "fill my buffer" pattern across the
stack:

```rust
fn push_with(
    &mut self,
    f: impl FnOnce(&mut [u8]) -> usize,
) -> Result<(), PushError>;
```

The writer hands the caller its internal buffer. The
caller writes directly into it and returns the length.
`buf.len()` tells the caller how much space is
available.

## Origin

Came up while designing the Router output queues
(see `router-split-output-queues.md`). The RingBuffer
stores packets that can wrap around the buffer boundary,
producing two slices. Sending those through the current
`write(&[u8])` API requires copying into a contiguous
staging buffer first. A closure-based API would let the
ring buffer write both halves directly into the frame
writer's buffer — but changing `FrameWrite::push` and
every frame type is a large refactor, so we deferred it.

## Motivation

- Avoids requiring data in a contiguous `&[u8]` before
  sending — enables zero-copy from split sources (e.g.
  ring buffer wrapping around, ISL header + payload).
- Enables in-place serialization directly into the
  frame data field.
- Current `&[u8]` API forces an extra copy when the
  source isn't contiguous.

## Scope

Affects the entire write path top-to-bottom:

| Trait | Current | New |
|-------|---------|-----|
| `FrameWrite::push` | `&[u8]` | closure |
| `DatalinkWrite::write` | `&[u8]` | closure |
| `NetworkWrite::write` | `&[u8]` | closure |

All frame types (TC, TM, AOS, Proximity-1, USLP) and
their writers would need updating.

## Open questions

- Keep the `&[u8]` method as a convenience alongside
  the closure version? (Default impl that calls the
  closure variant.)
- Error handling: should the closure return
  `Result<usize, E>` to signal serialization failure?
- How does this interact with COP-1 / FARM which need
  to inspect the data before deciding whether to accept
  it?
