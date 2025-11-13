#!/bin/bash

# Find and report the usage of FFI bindings in a Rust crate.
# This script analyzes bindgen-generated bindings and checks which ones
# are actually used in the crate's source code.

set -e
set -u
set -o pipefail

print_usage() {
    echo "Usage: $0 [--all] <path/to/bindings.rs> <path/to/src/>"
    echo ""
    echo "Arguments:"
    echo "  --all        Show both used and unused bindings (default: unused only)"
    echo "  bindings.rs  Path to the bindgen-generated bindings file"
    echo "  src/         Path to the crate's source directory"
    echo ""
    echo "Example:"
    echo "  $0 target/debug/build/leodos-libcsp-*/out/bindings.rs src/"
    echo "  $0 --all target/debug/build/leodos-libcsp-*/out/bindings.rs src/"
}

SHOW_ALL=false

if [ "$#" -ge 1 ] && [ "$1" = "--all" ]; then
    SHOW_ALL=true
    shift
fi

if [ "$#" -ne 2 ]; then
    echo "Error: Invalid number of arguments."
    print_usage
    exit 1
fi

BINDINGS_FILE="$1"
SRC_DIR="$2"

if [ ! -f "$BINDINGS_FILE" ]; then
    echo "Error: Bindings file not found at '$BINDINGS_FILE'"
    exit 1
fi

if [ ! -d "$SRC_DIR" ]; then
    echo "Error: Source directory not found at '$SRC_DIR'"
    exit 1
fi

echo "Analyzing bindings: $BINDINGS_FILE"
echo "Source directory:   $SRC_DIR"
echo "----------------------------------------"
echo ""

used_count=0
unused_count=0

# Extract binding definitions and check usage
awk '
/^ *pub\(crate\) (const|enum|struct|fn|type|union)/ {
    binding_type = $2
    binding_name = $3
    gsub(/[:(;<].*/, "", binding_name)
    if (binding_name != "" && binding_name !~ /^_/) {
        print binding_type, binding_name
    }
}' "$BINDINGS_FILE" | sort -u | while read -r binding_type binding_name; do
    # Skip empty names
    if [ -z "$binding_name" ]; then
        continue
    fi

    # Search for usage in source files (excluding ffi.rs which just includes bindings)
    usage_files=$(grep -r -l -w "$binding_name" "$SRC_DIR" --include="*.rs" 2>/dev/null | grep -v "ffi.rs" || true)

    if [ -n "$usage_files" ]; then
        if [ "$SHOW_ALL" = true ]; then
            file_list=$(echo "$usage_files" | tr '\n' ', ' | sed 's/,$//')
            printf "✅ USED:   %-6s %-40s in %s\n" "$binding_type" "$binding_name" "$file_list"
        fi
        ((used_count++)) || true
    else
        printf "❌ UNUSED: %-6s %s\n" "$binding_type" "$binding_name"
        ((unused_count++)) || true
    fi
done

echo ""
echo "----------------------------------------"
echo "Summary:"
echo "  Used:   $used_count"
echo "  Unused: $unused_count"
