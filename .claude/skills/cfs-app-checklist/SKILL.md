---
name: cfs-app-checklist
description: NASA cFS application development checklist. Apply when writing, reviewing, or auditing a cFS app to verify compliance with the cFE Application Developers Guide.
---

# cFS Application Development Checklist

Apply when writing, reviewing, or auditing a cFS application built on `leodos-libcfs`.

## Sources

- `cfe/docs/cFE Application Developers Guide.md` — official NASA cFE developer guide (shipped with cFE source)
- `cfe/docs/cFS_IdentifierNamingConvention.md` — NASA cFS naming conventions
- `apps/sample_app/` — NASA reference cFS application implementation
- `cfe/docs/cfe requirements.docx` — cFE functional requirements

Status legend:
- **AUTO** -- handled by `leodos-libcfs` (App builder, Runtime, RAII guards)
- **APP** -- application developer's responsibility
- **PARTIAL** -- framework provides the primitive; app must wire it up
- **N/A** -- not yet implemented or not applicable to Rust cFS apps

---

## 1. Application Lifecycle

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 1.1 | Entry point is `extern "C" fn XX_AppMain()` exported with `#[no_mangle]` | **APP** | Rust function naming; must match `cfe_es_startup.scr` |
| 1.2 | Application registration with ES (automatic since cFE 6.8+) | **AUTO** | cFE calls your entry point after registration |
| 1.3 | Check reset type via `CFE_ES_GetResetType` during init | **APP** | `cfe::es::system::get_reset_type()` available; app decides behavior |
| 1.4 | Call `CFE_ES_RunLoop` each cycle to honor shutdown commands | **AUTO** | `Runtime::poll_until_done` calls `app::run_loop()` each iteration |
| 1.5 | Exit with `CFE_ES_ExitApp` and correct `RunStatus` | **AUTO** | `Runtime::run` calls `app::exit_app(status)` after future completes |
| 1.6 | Drop all resources before exit (pipes, tables, tasks) | **AUTO** | Rust RAII -- `Pipe`, `Table`, `ChildTask`, `SendBuffer` all impl `Drop` |
| 1.7 | Support restartability -- no unrecoverable global state | **APP** | App must use CDS or re-derive state on startup |
| 1.8 | Panic handler that logs to System Log and exits | **APP** | `default_panic_handler` provided; app must wire it with `#[panic_handler]` |
| 1.9 | Use `#![no_std]` -- no standard library | **APP** | All apps must declare this |
| 1.10 | Wait for startup sync as last init step | **APP** | `cfe::es::system::wait_for_startup_sync()` available |

## 2. Event Services (EVS)

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 2.1 | Register with EVS via `CFE_EVS_Register` before sending events | **AUTO** | `App::new` calls `event::register(&[])` |
| 2.2 | Define unique Event IDs per event across the entire app (including child tasks) | **APP** | App defines its own event ID constants |
| 2.3 | Use appropriate event types: DEBUG, INFORMATION, ERROR, CRITICAL | **APP** | `event::debug/info/error/critical` and `info!/warn!/err!` macros available |
| 2.4 | Configure binary filters for high-frequency events | **PARTIAL** | `BinFilter` and `event::register(filters)` available; `App::new` passes `&[]` |
| 2.5 | Reset filters when needed (`CFE_EVS_ResetFilter`) | **APP** | `event::reset_filter()` and `event::reset_all_filters()` available |
| 2.6 | NoOp event includes app version string | **AUTO** | `App::recv` sends `event::info(EVT_NOOP, self.version)` |
| 2.7 | Startup event with version | **AUTO** | `App::new` sends `event::info(EVT_STARTUP, version)` |
| 2.8 | Child task events should identify the child task in the message text | **APP** | EVS identifies the app, not the task; text must disambiguate |
| 2.9 | Critical events should also go to ES System Log | **APP** | Use `log!()` macro alongside `event::critical()` |
| 2.10 | No unprintable or control characters in event text | **APP** | Developer responsibility |
| 2.11 | Use `CFE_ES_WriteToSysLog` if EVS registration fails or app is exiting | **PARTIAL** | `log!()` macro wraps `CFE_ES_WriteToSysLog`; `Runtime` uses it on exit |

## 3. Software Bus

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 3.1 | Create at least one pipe during init | **AUTO** | `App::new` creates a pipe |
| 3.2 | Name pipes with app prefix to avoid collisions | **AUTO** | `App::new` uses the app name |
| 3.3 | Subscribe to command MID | **AUTO** | `App::new` subscribes to `cmd_topic` |
| 3.4 | Subscribe to HK wakeup (Send HK) MID | **AUTO** | `App::new` subscribes to `send_hk_topic` |
| 3.5 | Pend on pipe in main loop (`CFE_SB_ReceiveBuffer`) | **AUTO** | `Pipe::recv` (async poll) used by `App::recv` |
| 3.6 | Use Message API to read/write header fields -- never manipulate raw header | **AUTO** | `MessageRef`/`MessageMut` wrap all CFE_MSG APIs |
| 3.7 | Time-stamp telemetry messages before sending | **AUTO** | `MessageMut::timestamp()` called in `App::send_hk` |
| 3.8 | Use zero-copy `SendBuffer` for large messages | **PARTIAL** | `SendBuffer` available; app chooses when to use |
| 3.9 | Pipe depth appropriate for the system | **PARTIAL** | `App::builder().pipe_depth(N)` -- default 16 |
| 3.10 | All inter-app communication via SB -- no direct function calls | **APP** | Architectural discipline |
| 3.11 | Pipe is single-thread only -- each thread needs its own pipe | **APP** | Enforced by `&mut self` on `Pipe::recv` |
| 3.12 | Delete pipe on cleanup (or let cFE clean up on exit) | **AUTO** | `Pipe::drop` calls `CFE_SB_DeletePipe` |

## 4. Command Handling

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 4.1 | Support NoOp command (function code 0) | **AUTO** | `App::recv` handles FCN_NOOP |
| 4.2 | Support Reset Counters command (function code 1) | **AUTO** | `App::recv` handles FCN_RESET, zeros cmd/err counters |
| 4.3 | Validate message length for every command | **PARTIAL** | `App::recv` validates NoOp/Reset length; app must validate app-specific commands |
| 4.4 | Reject unknown command codes with error event | **PARTIAL** | `App::reject()` available; app must call it for unrecognized codes |
| 4.5 | Increment command counter on success | **PARTIAL** | `App::ack()` available; auto for NoOp/Reset, app calls for others |
| 4.6 | Increment error counter on failure | **PARTIAL** | `App::reject()` increments; app can also manipulate counters directly |
| 4.7 | Dispatch table / match on function code for app-specific commands | **APP** | App implements match on `msg.fcn_code()` |
| 4.8 | Verify MsgId before processing | **AUTO** | `App::recv` checks `msg_id == cmd_msg_id` |

## 5. Housekeeping Telemetry

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 5.1 | Define HK telemetry struct with `#[repr(C)]` | **PARTIAL** | `HkTlm` (cmd_count, err_count) provided; app extends with own struct |
| 5.2 | Include command counter in HK | **AUTO** | `HkTlm::cmd_count` / `App::cmd_count()` |
| 5.3 | Include error counter in HK | **AUTO** | `HkTlm::err_count` / `App::err_count()` |
| 5.4 | Respond to Send HK wakeup message | **AUTO** | `App::recv` returns `Event::Hk` |
| 5.5 | Publish HK telemetry on wakeup | **PARTIAL** | `App::send_hk(&payload)` available; app calls with its full HK struct |
| 5.6 | Timestamp the HK packet | **AUTO** | `MessageMut::timestamp()` called in `send_hk` |
| 5.7 | Initialize the HK message once during init | **AUTO** | `SendBuffer::new` + `msg.init()` called each send |
| 5.8 | Include app-specific telemetry fields (mode, status, etc.) | **APP** | App defines its own HK struct and calls `app.send_hk(&my_hk)` |

## 6. Table Services

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 6.1 | Register tables during init with `CFE_TBL_Register` | **PARTIAL** | `Table::<T>::new(name, options)` wraps registration |
| 6.2 | Provide a validation function | **PARTIAL** | Implement `Validate` trait on table struct; trampoline auto-registered |
| 6.3 | Load default table data during init | **PARTIAL** | `Table::new` loads `T::default()`; file load via `load_from_file()` |
| 6.4 | Call `CFE_TBL_Manage` periodically (e.g. during HK cycle) | **APP** | `table.manage()` available; app must call it |
| 6.5 | Acquire table pointer with `CFE_TBL_GetAddress` | **AUTO** | `TableAccessor` RAII guard via `table.get()` |
| 6.6 | Release table pointer with `CFE_TBL_ReleaseAddress` | **AUTO** | `TableAccessor::drop` releases automatically |
| 6.7 | Release address before blocking on SB or calling Update | **APP** | App must scope `TableAccessor` lifetime correctly |
| 6.8 | Handle `CFE_TBL_INFO_UPDATED` return from GetAddress | **PARTIAL** | `TableAccessor::new` treats it as success |
| 6.9 | Use `Table::share` for shared tables from other apps | **PARTIAL** | Available; shared handles do not unregister on drop |
| 6.10 | Unregister tables on cleanup | **AUTO** | `Table::drop` calls `CFE_TBL_Unregister` for owned tables |
| 6.11 | Use tables for runtime configuration parameters | **APP** | None of the current apps use tables yet |

## 7. Critical Data Store (CDS)

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 7.1 | Register CDS block during init | **PARTIAL** | `CdsBlock::<T>::new(name)` wraps `CFE_ES_RegisterCDS` |
| 7.2 | Check if block was newly created or restored | **PARTIAL** | `CdsInfo::Created` vs `CdsInfo::Restored` returned by `new()` |
| 7.3 | Restore and validate contents on processor reset | **PARTIAL** | `cds.restore()` available; `restore_or_default()` convenience method |
| 7.4 | Initialize with defaults on power-on reset or if restore fails | **PARTIAL** | `restore_or_default()` handles this pattern |
| 7.5 | Periodically store working copy to CDS | **APP** | `cds.store(&data)` available; app must call it (e.g. each HK cycle) |
| 7.6 | Logically validate CDS contents (not just CRC) | **APP** | CRC is checked by cFE; app must validate semantic correctness |
| 7.7 | Store CDS before exit if possible | **APP** | App should store in error/shutdown path |

## 8. Performance Monitoring

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 8.1 | Define unique performance ID(s) in platform config | **APP** | App defines `XX_APPMAIN_PERF_ID` constant |
| 8.2 | Log entry at start of processing cycle | **AUTO** | `Runtime::perf_id(id)` creates `PerfMarker` at each poll cycle |
| 8.3 | Log exit before blocking on SB | **AUTO** | `PerfMarker::drop` logs exit when scope ends |
| 8.4 | Use `PerfMarker` RAII guard for additional code sections | **PARTIAL** | `PerfMarker::new(id)` available for custom sections |
| 8.5 | Multiple perf IDs for different sections if needed | **APP** | App creates additional `PerfMarker` instances |

## 9. Error Handling

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 9.1 | Check return codes of all cFE API calls | **AUTO** | All wrappers call `check(status)?` and return `Result` |
| 9.2 | Cascade init failures -- if any init step fails, exit with `APP_ERROR` | **PARTIAL** | `Runtime::run` exits on error; `?` operator propagates within the async block |
| 9.3 | Log to System Log if EVS registration fails | **APP** | Use `log!()` before EVS is available |
| 9.4 | Send error event for rejected commands | **PARTIAL** | `App::reject()` sends error event |
| 9.5 | Use typed error enums, not raw status codes | **AUTO** | `CfsError` with sub-enums: `EvsError`, `EsError`, `SbError`, `TblError`, etc. |
| 9.6 | Error status codes follow cFE bit format (severity, service, code) | **AUTO** | `status.rs` maps raw i32 to typed `Status` enum |
| 9.7 | Write to System Log on unrecoverable errors | **APP** | `log!()` macro available |
| 9.8 | Exit app on unrecoverable errors with `CFE_ES_ExitApp(APP_ERROR)` | **AUTO** | Panic handler calls `exit_app(Error)` |

## 10. Memory Management

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 10.1 | No heap allocation -- `#![no_std]`, no global allocator | **APP** | App must declare `#![no_std]` |
| 10.2 | Consolidate all resource allocations to init | **APP** | cFE best practice; buffers, pipes, tables during init |
| 10.3 | Use stack arrays for buffers | **APP** | `let buf = [0u8; N]` -- preferred approach |
| 10.4 | Use `heapless` containers for bounded collections | **PARTIAL** | `heapless::String`, `heapless::Vec` used throughout libcfs |
| 10.5 | Use statics for large or persistent buffers | **APP** | Requires `unsafe` for mutable statics |
| 10.6 | Memory pool available for pseudo-dynamic allocation | **PARTIAL** | `MemPool` and `PoolBuffer` RAII wrappers available; not used by current apps |
| 10.7 | Pool allocations return RAII guards | **AUTO** | `PoolBuffer` auto-returns to pool on drop |

## 11. Child Tasks

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 11.1 | Create child tasks only from the main task | **APP** | `ChildTask::new()` wraps `CFE_ES_CreateChildTask` |
| 11.2 | Main task must safely stop all child tasks before exit | **AUTO** | `ChildTask::drop` calls `CFE_ES_DeleteChildTask` |
| 11.3 | Child tasks call `CFE_ES_ExitChildTask` (not `ExitApp`) | **PARTIAL** | `task::exit_child_task()` available |
| 11.4 | Increment task counter in child tasks for liveness | **PARTIAL** | `task::increment_task_counter()` available |
| 11.5 | Child task events identify the child in message text | **APP** | Developer responsibility in event messages |
| 11.6 | Each thread needs its own pipe | **APP** | Enforced by Rust ownership (`&mut self` on recv) |
| 11.7 | Specify actual stack size (0 is non-portable) | **APP** | Always pass explicit size to `ChildTask::new` |
| 11.8 | Consolidate child task creation to init | **APP** | cFE best practice |
| 11.9 | Prefer async tasks (`join!`) over OS-level child tasks | **PARTIAL** | `Runtime` + `join!` macro for cooperative multitasking within main task |

## 12. Naming Conventions

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 12.1 | Entry point: `XX_AppMain` (matches startup script) | **APP** | Rust function with `#[no_mangle]` and `extern "C"` |
| 12.2 | Pipe names prefixed with app abbreviation | **AUTO** | `App::new` uses the `name` parameter |
| 12.3 | Event IDs unique within app (all tasks) | **APP** | Define constants or use `line!()` via `info!/warn!/err!` macros |
| 12.4 | Performance IDs defined in platform config header | **APP** | Typically in `xx_perfids.h` or Rust constants |
| 12.5 | Message IDs in separate header/module from app logic | **APP** | Keep topic IDs in config bindings (`bindings::` module) |
| 12.6 | Source files prefixed with app/component name | **APP** | Standard cFS convention; `xx_app.c`, `xx_cmds.c`, etc. |
| 12.7 | Config headers follow `default_xx_*` pattern for overridability | **APP** | CMake/build system convention |
| 12.8 | Table names are auto-prefixed with "AppName." by TBL services | **AUTO** | cFE handles this transparently |

## 13. Portability

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
| 13.1 | Avoid endianness dependencies in data extraction | **APP** | Use `from_be_bytes`/`to_be_bytes` for wire formats |
| 13.2 | Use CFE_MSG API for all header manipulation | **AUTO** | `MessageRef`/`MessageMut` enforce this |
| 13.3 | Use OSAL abstractions, not direct OS calls | **AUTO** | All OS wrappers in `os::` module use OSAL |
| 13.4 | Use `CFE_TIME_GetTime` (not TAI/UTC directly) for portable time | **PARTIAL** | `SysTime::now()` wraps `CFE_TIME_GetTime`; app should prefer this |
| 13.5 | Use `CFE_ES_` functions over `OS_` when both exist | **AUTO** | libcfs exposes the correct API level |
| 13.6 | Message IDs are opaque -- do not interpret bit patterns | **AUTO** | `MsgId` is an opaque wrapper; construction via topic-ID helpers |
| 13.7 | Use PSP/OSAL for hardware access, not direct I/O | **PARTIAL** | PSP wrappers in `psp::` module; NOS3 hardware drivers in `nos3::` |
| 13.8 | No floating-point assumptions (not all targets have FPU) | **APP** | Use `TaskFlags::FP_ENABLED` if needed |

---

## Quick Reference: What App::builder Handles

```rust
let mut app = App::builder()
    .name("MY_APP")           // pipe name prefix
    .cmd_topic(MY_CMD_TOPIC)  // subscribes to command MID
    .send_hk_topic(SEND_HK)  // subscribes to HK wakeup MID
    .hk_tlm_topic(MY_HK_TLM) // used for publishing HK
    .version("1.0.0")         // sent in NoOp/startup events
    .pipe_depth(16)           // optional, default 16
    .build()?;
```

Automatically handled:
- EVS registration (no filters)
- Pipe creation and subscription (cmd + HK wakeup)
- Startup info event with version
- NoOp command (fcn 0) with length validation and version event
- Reset Counters command (fcn 1) with length validation
- Command/error counter management
- HK telemetry publishing (`app.send_hk(&payload)`)
- Invalid command code rejection (`app.reject(msg)`)

## Quick Reference: What Runtime Handles

```rust
Runtime::new()
    .perf_id(MY_PERF_ID)  // optional performance monitoring
    .run(async {
        // init + main loop here
    });
```

Automatically handled:
- `CFE_ES_RunLoop` check each cycle
- `CFE_ES_ExitApp` on completion or shutdown request
- Performance entry/exit logging around each poll cycle
- System Log message on exit
- Drop of all resources before exit

## What the App Must Still Do

1. **Define the entry point** with `#[no_mangle] pub extern "C" fn XX_AppMain()`
2. **Wire the panic handler** with `#[panic_handler]`
3. **Validate app-specific commands** (length, parameters, range checks)
4. **Publish app-specific HK fields** (define struct, call `app.send_hk()`)
5. **Register and manage tables** if using runtime configuration
6. **Register and maintain CDS** if state must survive resets
7. **Define unique event IDs** for app-specific events
8. **Define performance IDs** for additional monitored sections
9. **Handle reset type** if behavior differs between power-on and processor reset
10. **Call `table.manage()`** during HK cycle for each owned table
11. **Store CDS periodically** (e.g. each HK cycle or on state change)
12. **Consolidate allocations to init** -- avoid runtime resource creation
