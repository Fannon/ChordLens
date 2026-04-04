@echo off
setlocal

echo Building ChordLens in Release mode...
cargo build --release

if not exist "bin" mkdir "bin"

echo Deploying VST3 and CLAP...
copy "target\release\chord_lens.dll" "bin\ChordLens.vst3" /Y
copy "target\release\chord_lens.dll" "bin\ChordLens.clap" /Y

echo Build complete! Plugins are located in the bin/ directory.
pause
