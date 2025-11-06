#!/bin/bash
BINDINGS="$(cat-repo.sh target/debug/build/libcfs-*/out/bindings.rs)"
CODE="$(cat-repo.sh src/)"

echo "I have this code:"
echo
echo "$CODE"
echo
echo "And this FFI:"
echo
echo "$BINDINGS"
echo
echo "I want the API to not use the raw FFI types directly. Are there any parts of the API that need to be fixed? If so, could you help me fix it? You can show only the updated code and where I should put it."
