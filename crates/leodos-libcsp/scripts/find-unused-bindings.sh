#!/bin/bash

# A script to find and report the usage of FFI bindings.

set -e
set -u
set -o pipefail

if [ "$#" -ne 2 ];
then
    echo "Error: Invalid number of arguments."
    echo "Usage: $0 <path/to/bindings.rs> <path/to/src/>"
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

found_unused=false
unused_count=0

awk '
/^ *pub\(crate\) (const|enum|struct|fn|type)/ {
    binding_type = $2
    binding_name = $3
    gsub(/[:(;].*/, "", binding_name)
    print binding_type, binding_name
}' "$BINDINGS_FILE" | while read -r binding_type binding_name; do
    # For each binding, search for files where it is used.
    usage_files=$(grep -r -l -w "$binding_name" "$SRC_DIR" || true)

    # Check if the `usage_files` variable is empty or not.
    if [ -n "$usage_files" ]; then
        # The binding is used. Iterate over each file where it was found.
        while IFS= read -r file; do
            # Use printf for nicely aligned output. %-5s pads the string to 5 characters.
            printf "✅ USED:   %+6s %s in %s\n" "$binding_type" "$binding_name" "$file"
        done <<< "$usage_files"
    else
        # The binding is not used.
        printf "❌ UNUSED: %+6s %s\n" "$binding_type" "$binding_name"
        found_unused=true
        ((unused_count++))
    fi
done
