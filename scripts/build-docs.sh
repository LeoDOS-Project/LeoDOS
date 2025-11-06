#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Preparing cFS Bindings ---"
docker compose build cfs-build
docker compose run --rm cfs-build bash -c "make SIMULATION=native prep"
echo "✅ Bindings prepared."

CRATES=(
    "apps/demo-rust-app/fsw"
    "crates/leodos-libcfs"
    "crates/leodos-spacepacket"
    "tools/leodos-cli"
)
OUTPUT_DIR="gh-pages-docs"

echo "--- Cleaning up old documentation ---"
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
echo "✅ Output directory '$OUTPUT_DIR' is clean."

echo "--- Building documentation for all crates (inside Docker) ---"
for crate_path in "${CRATES[@]}"; do
    crate_name=$(basename "$crate_path")
    echo "Building docs for $crate_name..."
    
    docker compose run --rm \
      cfs-build \
      cargo doc --manifest-path "$crate_path/Cargo.toml" --no-deps --all-features
    
    rsync -a --exclude '.lock' "$crate_path/target/doc/" "$OUTPUT_DIR/$crate_name"
done
echo "✅ All crate documentation built and collected."

echo "--- Creating main index.html ---"
# ... the rest of the script remains exactly the same ...
cat > "$OUTPUT_DIR/index.html" <<EOF
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Leodos Project Documentation</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; line-height: 1.6; padding: 2em; }
    h1 { font-size: 2em; }
    ul { list-style: none; padding: 0; }
    li { margin: 0.5em 0; }
    a { text-decoration: none; color: #0366d6; font-size: 1.2em; }
    a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <h1>Leodos Project Documentation</h1>
  <h2>Crates</h2>
  <ul>
EOF

for crate_path in "${CRATES[@]}"; do
    crate_name=$(basename "$crate_path")
    echo "    <li><a href=\"./$crate_name/\">$crate_name</a></li>" >> "$OUTPUT_DIR/index.html"
done

cat >> "$OUTPUT_DIR/index.html" <<EOF
  </ul>
</body>
</html>
EOF
echo "✅ Main index.html created."

echo "--- Documentation is ready in the '$OUTPUT_DIR' directory! ---"
