# OSAL

The Operating System Abstraction Layer (OSAL) sits between the PSP and the cFE, abstracting the real-time operating system. It provides a single API for task management, inter-task communication, synchronization, timers, and file system access that works identically across VxWorks, RTEMS, and Linux.

## Purpose

Flight software must be portable across operating systems. A mission might prototype on Linux, test on RTEMS, and fly on VxWorks. Without OSAL, every task creation, mutex lock, and file open would need OS-specific code. OSAL wraps these into a common interface so that application code never calls OS-native APIs directly.

## Key Abstractions

- **Tasks** — priority-scheduled threads with configurable stack sizes. Each cFS application runs as one or more OSAL tasks. The scheduler is preemptive: a higher-priority task interrupts a lower-priority one immediately.
- **Message queues** — fixed-depth, fixed-size queues for passing messages between tasks. The [Software Bus](/cfs/cfe/sb) is built on top of OSAL queues.
- **Synchronization** — binary semaphores (event signaling), counting semaphores (resource pools), and mutexes (mutual exclusion with priority inheritance to prevent priority inversion).
- **Timers** — one-shot and periodic timers backed by the OS timer facility. Used by [Time Services](/cfs/cfe/time) and by applications that need periodic wakeups.
- **File system** — open, read, write, close, mkdir, stat, and directory listing. OSAL maps these to the OS-native file system (or to a RAM disk in environments without persistent storage).

## LeoDOS Context

Development and testing use the Linux OSAL, which maps tasks to pthreads and semaphores to POSIX primitives. Flight targets use VxWorks or RTEMS OSAL implementations. Because all LeoDOS code goes through OSAL rather than calling OS APIs directly, switching between these environments requires no code changes — only a different OSAL build configuration.
