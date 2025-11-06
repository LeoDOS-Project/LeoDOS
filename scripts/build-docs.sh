#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status.

echo "--- Preparing cFS Bindings ---"
docker compose build cfs-build
docker compose run --rm cfs-build bash -c "make SIMULATION=native prep"
echo "✅ Bindings prepared."

# Define the crates to document
# Note: Use an array to handle spaces and special characters correctly
CRATES=(
    "apps/demo-rust-app/fsw"
    "crates/leodos-libcfs"
    "crates/leodos-spacepacket"
    "tools/leodos-cli"
)

# This is the final directory that will be deployed to GitHub Pages
OUTPUT_DIR="gh-pages-docs"

echo "--- Cleaning up old documentation ---"
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
echo "✅ Output directory '$OUTPUT_DIR' is clean."

echo "--- Building documentation for all crates ---"
for crate_path in "${CRATES[@]}"; do
    # Extract the crate's name for the subdirectory
    crate_name=$(basename "$crate_path")
    echo "Building docs for $crate_name..."
    
    # Build the docs
    $(cd $crate_path; cargo doc --no-deps --all-features)
    
    # Copy the generated docs to our final output directory
    cp -r "$crate_path/target/doc" "$OUTPUT_DIR/$crate_name"
done
echo "✅ All crate documentation built and collected."

echo "--- Creating main index.html ---"
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

# Add a link for each crate to the index file
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
