---
name: verify-docs
description: Verify that documentation files match the actual code implementation
---

## Usage

```
/verify-docs [doc-file]
```

## Arguments

- `doc-file` (optional): Path to a specific doc file to verify. If omitted,
  verifies all docs in `docs/`.

## Instructions

Read the documentation file and the corresponding source code. Compare them and
report any discrepancies between what the documentation claims and what the code
actually implements.

Use checkmarks (✓) for matches and crosses (✗) for mismatches. Suggest fixes
when discrepancies are found.
