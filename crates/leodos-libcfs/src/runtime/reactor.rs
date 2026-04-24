//! Reactor: a per-task registry of fds waiting for IO.
//!
//! Leaf futures that wrap an OSAL selectable handle (UDP sockets,
//! TCP sockets, files opened O_NONBLOCK) call
//! [`register_read`] when they return `Poll::Pending`. The
//! runtime then blocks the task in `OS_SelectMultiple` until any
//! registered fd becomes readable or a timeout elapses.
//!
//! The reactor is installed into the `Waker` passed to polls by
//! [`crate::runtime::Runtime`]. Leaf futures find it by reading
//! the waker's data pointer.

use crate::os::id::OsalId;
use crate::os::net::select_multiple;
use crate::os::net::FdSet;
use core::cell::RefCell;
use core::task::RawWaker;
use core::task::RawWakerVTable;
use core::task::Waker;

struct State {
    read_set: FdSet,
    write_set: FdSet,
    has_reads: bool,
    has_writes: bool,
    woken: bool,
}

impl State {
    fn new() -> Self {
        Self {
            read_set: FdSet::new(),
            write_set: FdSet::new(),
            has_reads: false,
            has_writes: false,
            woken: false,
        }
    }
}

/// Per-task reactor state.
pub struct Reactor {
    state: RefCell<State>,
}

impl Reactor {
    pub(crate) fn new() -> Self {
        Self { state: RefCell::new(State::new()) }
    }

    fn register_read(&self, id: OsalId) {
        let mut s = self.state.borrow_mut();
        s.read_set.add(id);
        s.has_reads = true;
    }

    fn register_write(&self, id: OsalId) {
        let mut s = self.state.borrow_mut();
        s.write_set.add(id);
        s.has_writes = true;
    }

    fn take_woken(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let prev = s.woken;
        s.woken = false;
        prev
    }

    fn set_woken(&self) {
        self.state.borrow_mut().woken = true;
    }

    /// Blocks the task until any registered fd is readable /
    /// writable or the timeout elapses. Clears the registration
    /// sets. If no fds are registered, sleeps for `timeout_ms`
    /// milliseconds so apps that only use non-fd primitives
    /// (e.g. SB pipes) still yield the CPU.
    ///
    /// TODO: per-leaf wakers + persistent FdSet so unrelated
    /// leaves aren't re-polled on every wake. See CLAUDE.md.
    pub(crate) fn block(&self, timeout_ms: i32) {
        let Ok(mut s) = self.state.try_borrow_mut() else {
            return;
        };
        if !s.has_reads && !s.has_writes {
            drop(s);
            let duration = core::time::Duration::from_millis(timeout_ms.max(0) as u64);
            let _ = crate::os::task::delay(duration);
            return;
        }
        // Move the sets out so `select_multiple` can receive
        // `&mut FdSet` while we drop the RefCell borrow. The
        // ownership transfer is effectively a fd-set reset.
        let mut read_set = core::mem::replace(&mut s.read_set, FdSet::new());
        let mut write_set = core::mem::replace(&mut s.write_set, FdSet::new());
        s.has_reads = false;
        s.has_writes = false;
        drop(s);
        let _ = select_multiple(Some(&mut read_set), Some(&mut write_set), timeout_ms);
    }

    pub(crate) fn was_woken(&self) -> bool {
        self.take_woken()
    }
}

/// Build a `Waker` whose data pointer is a `&Reactor`.
///
/// SAFETY: the caller must ensure the `Reactor` outlives every
/// clone of the returned waker.
pub(crate) unsafe fn waker_from_reactor(reactor: &Reactor) -> Waker {
    unsafe {
        Waker::from_raw(RawWaker::new(
            reactor as *const Reactor as *const (),
            &VTABLE,
        ))
    }
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(w_clone, w_wake, w_wake_by_ref, w_drop);

unsafe fn w_clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
}

unsafe fn w_wake(data: *const ()) {
    unsafe { w_wake_by_ref(data) };
}

unsafe fn w_wake_by_ref(data: *const ()) {
    if data.is_null() {
        return;
    }
    let reactor = unsafe { &*(data as *const Reactor) };
    reactor.set_woken();
}

unsafe fn w_drop(_data: *const ()) {}

/// Registers a read interest for `id` using the reactor carried
/// in `waker`. No-op if the waker was not produced by a
/// reactor-backed runtime.
pub fn register_read(waker: &Waker, id: OsalId) {
    let data = waker.data();
    if data.is_null() {
        return;
    }
    let reactor = unsafe { &*(data as *const Reactor) };
    reactor.register_read(id);
}

/// Registers a write interest for `id` using the reactor carried
/// in `waker`. No-op if the waker was not produced by a
/// reactor-backed runtime.
pub fn register_write(waker: &Waker, id: OsalId) {
    let data = waker.data();
    if data.is_null() {
        return;
    }
    let reactor = unsafe { &*(data as *const Reactor) };
    reactor.register_write(id);
}
