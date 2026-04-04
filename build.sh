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

# Deploy VST3
cp "target/release/chord_lens.$EXT" "bin/ChordLens.vst3"
# Deploy CLAP
cp "target/release/chord_lens.$EXT" "bin/ChordLens.clap"

echo "Build complete! Plugins are located in the bin/ directory."
