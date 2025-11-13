---
name: find-unused-bindings
description: Analyzes FFI binding usage in Rust bindgen crates. Use when the user asks to check unused bindings, find bindings needing wrappers, or analyze binding coverage in leodos-libcfs, leodos-libcsp, or similar crates.
user-invocable: false
---

# Find Unused Bindings

Analyzes which bindgen-generated FFI bindings are used or unused in a Rust crate.

## How to use

1. Build the crate to ensure bindings exist:
   ```bash
   cargo build -p <crate-name>
   ```

2. Find the bindings file:
   ```bash
   find crates/<crate-name>/target -name "bindings.rs" | head -1
   ```

3. Run the script:
   ```bash
   .claude/skills/find-unused-bindings/scripts/find-unused-bindings.sh [--all] <bindings-path> crates/<crate-name>/src/
   ```

## Flags

- Default: Show only unused bindings
- `--all`: Show both used and unused bindings

## Output

- `❌ UNUSED:` - Binding not referenced in source
- `✅ USED:` - Binding referenced (only with `--all`)

## Follow-up

After identifying unused bindings, if implementation is requested:
1. Prioritize structs and enums over constants
2. Create safe wrappers following existing crate patterns
3. Re-run to verify bindings are now used
