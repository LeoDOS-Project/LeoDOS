# Router split + output queues

## Problem

The Router's `read()` loop forwards non-local packets inline:
while writing a forwarded packet to an outgoing link, it cannot
receive from any other link. This causes:

1. **Head-of-line blocking**: a slow outgoing link stalls the
   entire router. A packet destined north blocks packets
   arriving for all other directions.
2. **Deadlock**: two adjacent routers could each be trying to
   forward a packet to each other. Both blocked on write,
   neither reading. Classic circular wait.

With UDP links this doesn't happen in practice (UDP writes
don't block), but it breaks with any backpressure-aware link.

## Design

### Split links into read/write halves

New trait:

```rust
pub trait DatalinkSplit {
    type Reader: DatalinkRead;
    type Writer: DatalinkWrite;
    fn split(self) -> (Self::Reader, Self::Writer);
}
```

For `UdpDatalink`: `send()` and `recv()` both take `&self`
on `UdpSocket`, so both halves share the same socket via a
borrow. The existing `UdpFrameWriter`/`UdpFrameReader` in
`datalink/link/cfs/udp.rs` already demonstrate this pattern.

```rust
impl UdpDatalink {
    pub fn split(&self) -> (UdpReader<'_>, UdpWriter<'_>) {
        (
            UdpReader { socket: &self.socket },
            UdpWriter { socket: &self.socket,
                        remote: self.remote },
        )
    }
}
```

The socket lives on the caller's stack, both halves borrow
it — same pattern as `LocalChannel::split()`.

### Router struct

The Router stores separate reader/writer per direction, plus
per-direction output queues and staging buffers:

```rust
pub struct Router<'a, N, S, E, W, G, R, C,
    const MTU: usize = 1024,
    const QUEUE: usize = 4,
> where
    N: DatalinkSplit, S: DatalinkSplit,
    E: DatalinkSplit, W: DatalinkSplit,
    G: DatalinkSplit,
{
    // Split link halves
    north_r: N::Reader, north_w: N::Writer,
    south_r: S::Reader, south_w: S::Writer,
    east_r:  E::Reader, east_w:  E::Writer,
    west_r:  W::Reader, west_w:  W::Writer,
    ground_r: G::Reader, ground_w: G::Writer,

    // Input buffers (one per direction)
    north_buf: [u8; MTU],
    south_buf: [u8; MTU],
    east_buf:  [u8; MTU],
    west_buf:  [u8; MTU],
    ground_buf: [u8; MTU],

    // Output ring buffers (one per direction)
    // Write future borrows directly from ring tail — no
    // separate staging buffer needed.
    north_out: PacketRing<OUT_BUF>,
    south_out: PacketRing<OUT_BUF>,
    east_out:  PacketRing<OUT_BUF>,
    west_out:  PacketRing<OUT_BUF>,
    ground_out: PacketRing<OUT_BUF>,

    address: Address,
    algorithm: R,
    clock: C,
}
```

### read() event loop

Destructure `self` so each future borrows independent fields.
`select_biased!` polls all 10 futures (5 reads + 5 writes):

```rust
async fn read(&mut self, buffer: &mut [u8]) -> ... {
    loop {
        let Self {
            north_r, north_w, north_buf, north_out,
            north_stage,
            south_r, south_w, south_buf, south_out,
            south_stage,
            // ...
        } = self;

        // Stage output: copy queue front into staging buf
        let n_out = stage_write(north_w, north_out,
                                north_stage);
        let s_out = stage_write(south_w, south_out,
                                south_stage);
        // ...

        let n_in = north_r.read(north_buf).fuse();
        let s_in = south_r.read(south_buf).fuse();
        // ...

        pin_mut!(n_in, s_in, ..., n_out, s_out, ...);

        select_biased! {
            // Inputs — classify and enqueue
            r = n_in => enqueue(r, North, ...),
            r = s_in => enqueue(r, South, ...),
            r = e_in => enqueue(r, East, ...),
            r = w_in => enqueue(r, West, ...),
            r = g_in => enqueue(r, Ground, ...),
            // Outputs — drain one packet each
            _ = n_out => { north_out.pop_front(); }
            _ = s_out => { south_out.pop_front(); }
            _ = e_out => { east_out.pop_front(); }
            _ = w_out => { west_out.pop_front(); }
            _ = g_out => { ground_out.pop_front(); }
        }

        // Check if local packet is ready
        if let Some(pkt) = local_packet {
            buffer[..pkt.len].copy_from_slice(...);
            return Ok(pkt.len);
        }
    }
}
```

`stage_write` either starts a real write or returns
`pending()`:

```rust
fn stage_write(writer, queue, stage) -> impl Future {
    match queue.front() {
        Some(pkt) => {
            stage[..pkt.len].copy_from_slice(&pkt.data);
            writer.write(&stage[..pkt.len]).left_future()
        }
        None => pending().right_future(),
    }
}
```

### Output queue: byte ring buffer

Using `Deque<Packet<MTU>, QUEUE>` wastes memory — every slot
reserves MTU bytes even if the actual packet is 50 bytes.
At MTU=1024, QUEUE=4 that's 4 KB per direction of padding.

A byte ring buffer stores packets contiguously with a length
prefix, only paying for actual packet sizes:

```rust
struct PacketRing<const N: usize> {
    buf: [u8; N],
    head: usize,  // next write position
    tail: usize,  // next read position
}
```

Each packet stored as `[len: u16][data: len bytes]`. Push
writes at head, pop reads at tail, both wrap modulo N.

If a packet doesn't fit between head and the buffer end,
waste the gap and write at position 0. Worst case wastes
MTU bytes once — same as one Deque slot. A 2 KB ring per
direction can hold dozens of small packets or a couple
large ones.

The ring also eliminates the staging buffer — the write
future borrows directly from the ring at the tail position.
No extra copy needed.

### Drop policy

When the ring is full, drop the packet. Transport layer
(SRSPP/CFDP) handles retransmission. This matches how IP
routers work (tail-drop).

### App code (spacecomp example)

```rust
let north = UdpDatalink::bind(local_n, remote_n)?;
let south = UdpDatalink::bind(local_s, remote_s)?;
// ...
let (nr, nw) = north.split();
let (sr, sw) = south.split();
// ...

let mut router = Router::builder()
    .north(nr, nw)
    .south(sr, sw)
    .east(er, ew)
    .west(wr, ww)
    .ground(gr, gw)
    .address(address)
    .algorithm(algorithm)
    .clock(MetClock::new())
    .build();
```

The links live on the caller's stack. The Router gains a
lifetime `'a` tying it to the link storage.

### Memory cost

5 input buffers (MTU each) + 5 output ring buffers (OUT_BUF
each). At MTU=1024, OUT_BUF=2048: ~15 KB total. The ring
buffers are much more memory-efficient than Deque since they
only consume actual packet bytes, not MTU per slot. Tunable
via const generics.

## Files to modify

| File | Change |
|------|--------|
| `datalink/mod.rs` | Add `DatalinkSplit` trait |
| `datalink/link/cfs/udp.rs` | Impl split for UdpDatalink |
| `network/isl/routing/mod.rs` | Restructure Router with split halves + output queues |
| `network/isl/routing/service.rs` | Update RouterService for new Router API |
| `network/isl/routing/local.rs` | Impl split for LocalChannel handles |
| `apps/spacecomp/fsw/src/lib.rs` | Update link construction |
