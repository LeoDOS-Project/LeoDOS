# Deployment

A cFS mission can be updated while running. Apps can be loaded, unloaded, and restarted. Configuration tables can be replaced. These operations are performed by ground commands — no reboot is required unless the core executive itself changes.

## Updating Apps

Apps are separate loadable modules, not part of the core executive binary. Updating an app follows three steps:

1. **Upload** — the new app binary is transferred to the spacecraft's file system, typically via [CFDP](/protocols/transport/cfdp) (the file delivery protocol in the communication stack).
2. **Unload** — a ground command tells the executive to stop the running app. Its bus subscriptions are removed, its pipes are deleted, and its resources are freed. Other apps are unaffected — they were never coupled to it directly.
3. **Load** — a ground command tells the executive to load the new binary, specifying its entry point, stack size, and priority. The app starts, registers with the executive, recovers any persistent state, and begins processing messages.

These steps can also add an entirely new app that was not part of the original mission, or remove one that is no longer needed. The bus-based communication model means no other app needs to be modified or restarted when an app is swapped.

## Updating Tables

Tables can be updated without touching the app that owns them:

1. **Upload** — the new table image is transferred to the file system.
2. **Load** — a ground command tells [Table Services](/cfs/cfe/tbl) to load the new image. The app's validation callback checks that the data is consistent (ranges, checksums, cross-references) and accepts or rejects the load.
3. **Activate** — if validation passes, a second command activates the new table. For double-buffered tables, the swap is atomic — the app sees either the old or new data, never a partial update.

This two-step validate-then-activate process prevents invalid configuration from reaching a running app. The ground can also dump any table at any time to inspect its current contents.

## Updating the Startup Script

The startup script — which lists which apps to load, in what order, with what priority — is a file on the file system. Uploading a new startup script changes what happens on the next boot without rebuilding any software. This is how missions add or retire apps across processor resets.

## What Cannot Be Updated at Runtime

The core executive ([PSP](/cfs/psp) + [OSAL](/cfs/osal) + [cFE](/cfs/cfe/overview)) is a single binary that boots with the processor. Updating it requires uploading the new binary, commanding a processor reset, and booting into the new image. Missions typically maintain two boot banks — one with the current image, one with the update — so a failed update can fall back to the known-good image.
