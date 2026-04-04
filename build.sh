# ChordLens Build Script (Bash)
# This script builds the plugin and moves it to the bin/ directory.

set -e

echo "Building ChordLens in Release mode..."
cargo build --release

# Determine extension based on OS
OS_NAME=$(uname -s)
EXT="so"
if [[ "$OS_NAME" == *"NT"* || "$OS_NAME" == "MINGW"* || "$OS_NAME" == "MSYS"* ]]; then
    EXT="dll"
elif [[ "$OS_NAME" == "Darwin" ]]; then
    EXT="dylib"
fi

mkdir -p bin

# Deploy VST3 (might fail if DAW has it locked)
cp "target/release/chord_lens.$EXT" "bin/ChordLens.vst3" || echo "Warning: bin/ChordLens.vst3 is busy, skipping deploy."
# Deploy CLAP (might fail if DAW has it locked)
cp "target/release/chord_lens.$EXT" "bin/ChordLens.clap" || echo "Warning: bin/ChordLens.clap is busy, skipping deploy."

echo "Build complete! Plugins are located in the bin/ directory."

echo "Creating timestamped release in tmp/..."
dt=$(date +%Y%m%d_%H%M%S)
dir="tmp/release_$dt"
mkdir -p "$dir"
cp "target/release/chord_lens.$EXT" "$dir/ChordLens.vst3"
cp "target/release/chord_lens.$EXT" "$dir/ChordLens.clap"
echo "Snapshot saved to $dir"
