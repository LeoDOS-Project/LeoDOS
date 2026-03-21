# leodos-libcfs: doc deviations from C headers

Audit of Rust doc comments against the cFE/OSAL/PSP C header
doxygen. Three categories: factual errors (wrong), missing
safety caveats, and missing behavioral details.

---

## Factual errors (fix immediately)

### 1. `evs/event.rs` `info()` — wrong severity level

Doc says "Sends a debug-level software event" but the function
sends info-level. Copy-paste from `debug()`.

### 2. `es/cds.rs` `restore()` — doc/code mismatch

Doc says corrupted bytes are still copied into `T` and the
caller must decide how to handle it. But the code does
`check(status)?` which returns `Err` before `assume_init()`,
so the caller never gets the data. Either fix the doc to match
the code, or fix the code to match the doc (return the data
alongside the error).

### 3. `os/id.rs` `to_index()` — claims validity checking

Doc says "Returns `Error::OsErrInvalidId` if this ID does not
map to a valid index for its type." The C header explicitly
says `OS_ConvertToArrayIndex` does NOT verify validity.

---

## Missing safety/blocking caveats

### 4. `es/app.rs` `restart()` / `reload()`

C warns: if the file is missing or corrupt, the application
may be permanently deleted and unrecoverable except via
`ES_STARTAPP` command.

### 5. `tbl.rs` `Table::new()`, `load_from_file()`, `load_from_slice()`

These calls can block. Must not be called from ISR context.

### 6. `tbl.rs` `TableAccessor::new()` (wraps `CFE_TBL_GetAddress`)

- Can block on shared single-buffered tables
- Address must be released before any `Update` call or
  blocking call (e.g. pending on SB message)
- Returns zeroed table pointer if never loaded
  (`CFE_TBL_ERR_NEVER_LOADED`)

### 7. `es/system.rs` `process_async_event()`

Must not be invoked directly from ISR/signal context. PSP must
guarantee it runs from a context that can use OSAL primitives.

### 8. `os/app.rs` `api_init()`

C warns: failure means subsequent OSAL calls have undefined
behavior. Typical response is to abort.

### 9. `os/app.rs` `api_teardown()`

C says this is best-effort — may not recover all resources.
Rust doc says "will release all OS resources" which is
misleading.

### 10. `os/fs.rs` `remove()`, `rename()`, `cp()`, `mv()`

C warns: behavior on an open file is not defined at the OSAL
level. Applications should ensure the file is closed first.

### 11. `os/fs.rs` `unmount()`

C warns: all open file descriptors become useless after
unmount. Should close them first.

### 12. `os/timer.rs` — all timer APIs

Every timer API (`Timer::new`, `set`, `delete`,
`get_id_by_name`, `get_info`) has the note: "Must not be used
from the context of a timer callback."

### 13. `os/timebase.rs` `TimeBase::new()`, `TimeBase::set()`

Same "must not call from timer callback context" caveat.
Also `new()` creates a servicing task at elevated priority
that will interrupt user tasks. Kernel must be configured
for `OS_MAX_TASKS + OS_MAX_TIMEBASES` threads.

---

## Missing behavioral details

### 14. `es/app.rs` `AppId::this()`

Child tasks return the same app ID as their parent. Important
for understanding the semantics.

### 15. `es/cds.rs` `CdsBlock::new()`

- Does NOT clear or initialize the data in the block
- If a block existed with a different size, it is replaced
  and the new one contains uninitialized data (returns
  `Created`, not `Restored`)

### 16. `es/system.rs` `wait_for_startup_sync()`

- Timeout must be ≥ 1000ms (lower values rounded up)
- Should only be called as the last item of app init
- Should only be called by apps started from the ES
  startup file

### 17. `es/system.rs` `reset_cfe()`

Can return `CFE_ES_BAD_ARGUMENT` or `CFE_ES_NOT_IMPLEMENTED`
for invalid reset types. Doc says "does not return" but it
can on error (Rust code loops, which is correct but doc is
slightly misleading).

### 18. `es/system.rs` `background_wakeup()`

Work is pro-rated based on elapsed time since last wakeup.
Waking early does not cause extra work.

### 19. `es/counter.rs` module doc

Says "atomically incremented" but C header does not guarantee
atomicity for `CFE_ES_IncrementGenCounter`.

### 20. `es/pool.rs` `MemPool::new()`

- Pool size must be an integral number of 32-bit words
- Start address must be 32-bit aligned
- 168 bytes reserved for internal bookkeeping

### 21. `es/pool.rs` `MemPool::get_buf()`

Actual allocation is at least 12 bytes more than requested.

### 22. `evs/event.rs` `register()`

Calling more than once wipes all previous filters. Filter
registration is NOT cumulative.

### 23. `evs/event.rs` `send()`

Only works within the context of a registered application.
For messages outside that context (e.g. early in init),
use `CFE_ES_WriteToSysLog` instead.

### 24. `sb/pipe.rs` `subscribe()`

Subscriptions are added to the head of a linked list.
Messages are delivered in LIFO order (last subscriber
receives first).

### 25. `sb/send_buf.rs` `SendBuffer::send()`

On failure, the caller still owns the buffer (state is
unchanged). The Rust code handles this correctly via
`mem::forget` only on success, but the doc should state it.

### 26. `time.rs` `now_tai()` / `now_utc()`

- Not portable to all missions
- TAI maintenance in flight is not guaranteed
- UTC can jump backward on leap second events

### 27. `time.rs` `microseconds_to_subseconds()`

Returns `0xFFFFFFFF` if input > 999,999 (saturation).

### 28. `time.rs` `register_synch_callback()`

- Only one callback per application
- Should only be called from the app's main thread
- Distribute timing to child tasks internally

### 29. `os/app.rs` `application_shutdown()`

Passing `false` cancels a previously-initiated shutdown.
Doc only describes the `true` case.

### 30. `os/sync.rs` `Mutex::new()`

OSAL mutexes are always created in the unlocked (full) state.

### 31. `os/sync.rs` `CountSem::new()`

For portability, keep values within `short int` range
(0–32767). Some RTOS impose upper limits.

### 32. `os/sync.rs` `CondVar::wait()` — API mismatch

Takes an external `MutexGuard` but OSAL condvars have their
own internal mutex via `OS_CondVarLock`/`OS_CondVarUnlock`.
Those functions are not wrapped. The doc is misleading about
which mutex is released. Needs either an API fix or a doc
clarification.

### 33. `os/task.rs` `Task::new()`

`stack_size=0` is non-portable. Some RTOS use a default,
others create a task with no stack. Always specify actual size.

### 34. `os/timer.rs` `Timer::new()`

Creates a dedicated hidden time base object (consumes a
resource slot), created and deleted with the timer itself.

### 35. `os/timer.rs` `Timer::set()`

- Both `start_time` and `interval_time` being zero is an error
- Values below clock accuracy are rounded up to the timer's
  resolution

### 36. `os/fs.rs` `make_fs()`

- RAM disk `volname` must begin with "RAM" (e.g. "RAM0")
- `address == 0` means the OS allocates the memory

### 37. `os/fs.rs` `close_file_by_name()`

Only works if the name matches the one used to open the file.

### 38. `os/module.rs` `symbol_table_dump()`

Not all RTOS support this. Returns `OS_ERR_NOT_IMPLEMENTED`
if not available.

### 39. `os/module.rs` `Module::load()`

`GLOBAL_SYMBOLS` is the default. Use `LOCAL_SYMBOLS` for safer
unloading and `OS_SymbolLookupInModule` for local symbols.

### 40. `psp/eeprom.rs` — write functions

`write_u8/u16/u32` can return `TIMEOUT` (write did not
complete), `ADDRESS_MISALIGNED`, or `NOT_IMPLEMENTED`.

### 41. `psp/eeprom.rs` — enable/disable/power functions

`write_enable`, `write_disable`, `power_up`, `power_down`
can all return `NOT_IMPLEMENTED`.

### 42. `psp/mem.rs` `get_reset_area()`

Area is preserved during processor resets. Stores ER Log,
System Log, and reset-related variables.

### 43. `psp/mem.rs` `get_cfe_text_segment_info()`

Missing "may not be implemented on all architectures" caveat.
The sister function `get_kernel_text_segment_info` has it.

### 44. `psp/mem.rs` `mem_validate_range()`

Three specific error codes: `INVALID_MEM_ADDR` (bad start),
`INVALID_MEM_TYPE` (type mismatch), `INVALID_MEM_RANGE`
(range too small for address + size).

### 45. `psp/exception.rs` `get_summary()`

- The call **pops** the entry from the queue (destructive read)
- Success doesn't guarantee output fields have valid data,
  just that they are initialized

### 46. `psp/exception.rs` `copy_context()`

Can return `NO_EXCEPTION_DATA` if data has expired from the
circular memory log.

### 47. `psp/watchdog.rs` `init()`

Configures timer resolution and platform-specific settings,
not just a generic "initialize."

### 48. `psp/watchdog.rs` `service()`

Reloads the timer with the value from `WatchdogSet`.
Platform quirk: value of 0 → 4.5s reset, all others → 5.5s.

### 49. `psp/version.rs` `get_version_number()`

MissionRevision semantics: 0 = official release,
1–254 = local patch (mission use), 255 = development build.

### 50. `psp/version.rs` `get_build_number()`

Monotonically increasing number reflecting commits since
epoch release. Fixed at compile time.

### 51. `psp/time.rs` `get_timer_ticks_per_second()`

C guarantees at least 1 MHz resolution (1 µs per tick).

---

## Priority

1. Fix the 3 factual errors (#1–#3) first
2. Add safety/blocking caveats (#4–#13) — these affect
   correctness and real-time behavior
3. Add behavioral details (#14–#51) as time permits
