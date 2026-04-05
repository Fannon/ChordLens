# ChordLens Build Script (Bash)
# This script builds properly bundled plugin artifacts using NIH-plug's bundler.

set -e

echo "Bundling ChordLens in Release mode..."
cargo xtask bundle chord-lens --release

mkdir -p bin
mkdir -p tmp

rm -rf "bin/ChordLens.vst3" "bin/ChordLens.clap"
cp -R "target/bundled/ChordLens.vst3" "bin/ChordLens.vst3"
cp -R "target/bundled/ChordLens.clap" "bin/ChordLens.clap"

echo "Build complete! Plugins are located in the bin/ directory."

echo "Creating timestamped release in tmp/..."
dt=$(date +%Y%m%d_%H%M%S)
dir="tmp/release_$dt"
mkdir -p "$dir"
cp -R "target/bundled/ChordLens.vst3" "$dir/ChordLens.vst3"
cp -R "target/bundled/ChordLens.clap" "$dir/ChordLens.clap"
echo "Snapshot saved to $dir"
