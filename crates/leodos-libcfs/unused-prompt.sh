#!/bin/bash
BINDINGS="$(cat-repo.sh target/debug/build/libcfs-*/out/bindings.rs)"
CODE="$(cat-repo.sh src/)"
STATUS="$(./find-unused-bindings.sh target/debug/build/libcfs-*/out/bindings.rs src/|grep 'UNUSED: *fn')"

echo "I have this code:"
echo
echo "$CODE"
echo
echo "And this FFI:"
echo
echo "$BINDINGS"
echo
echo "Currently, these bindings are unused:"
echo
echo "$STATUS"
echo 
echo "Could you help me integrate the remaining ones? You can show only the added code and where I should put it."
