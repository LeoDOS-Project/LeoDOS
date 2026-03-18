# Table Services

Table Services (TBL) separates runtime configuration data from application code. Instead of compiling constants into the binary, applications load configuration from tables that can be updated from the ground without restarting the app or uploading new software.

## Key Concepts

- **Table registration** — at startup, an application registers each table it uses, specifying its size, name, and whether it is single-buffered or double-buffered.
- **Load and dump** — the ground can upload a new table image (load) or request the current contents (dump). Loads go through a validation step before they take effect.
- **Validation callbacks** — when a new table image is loaded, TBL calls the application's validation function. The application checks that the new data is consistent (ranges, checksums, cross-references) and accepts or rejects the load.

## Single vs Double Buffered

A single-buffered table uses one memory region. When a new image is loaded, the application must explicitly release its pointer, let TBL swap in the new data, and re-acquire the pointer. This is simple but means the application cannot access the table during the swap.

A double-buffered table maintains two copies. TBL writes the new image into the inactive copy and swaps the pointers atomically. The application always has a valid table pointer — it sees either the old or new data, never a partial update. Double buffering costs twice the memory but is essential for tables accessed in tight loops.

## Critical Tables

Tables can be marked as critical, which means their contents are persisted in the [Critical Data Store](/cfs/cfe/es) (CDS) across processor resets. When the application restarts after a reset, it recovers the table from CDS rather than loading the default image. This ensures that ground-uploaded configuration survives faults.

## Ground Operations

The ground workflow for updating a table is: upload the new image file, send a load command, wait for the validation result, and send an activate command if validation passed. This two-step process prevents invalid configurations from reaching the application. The ground can also dump any table at any time to verify its contents.
