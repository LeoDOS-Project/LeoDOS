//! Three backends behind the same [`ReceiverBackend`] trait:
//!
//! |                 | Fast            | Lite           | Packed      |
//! | --------------- | --------------- | -------------- | ----------- |
//! | OOO insert      | O(1)            | O(WIN)         | O(1)        |
//! | Delivery        | O(1) advance    | O(REASM) shift | O(MSG) copy |
//! | Per-segment use | MTU (fixed)     | MTU (fixed)    | payload len |
//! | Static memory   | WIN×MTU + REASM | REASM          | BUF + REASM |
//!
//! [`ReceiverMachine`] is a type alias for [`PackedReceiver`].
/// Fastest backend — O(1) insert and delivery.
pub mod fast;
/// Half-memory backend — single shared buffer.
pub mod lite;
/// Packed backend — efficient for small payloads.
pub mod packed;
