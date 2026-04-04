@echo off
setlocal

echo Building ChordLens in Release mode...
cargo build --release

if not exist "bin" mkdir "bin"

echo Deploying VST3 and CLAP...
echo Deploying VST3 and CLAP...
copy "target\release\chord_lens.dll" "bin\ChordLens.vst3" /Y || echo Warning: bin\ChordLens.vst3 is busy, skipping deploy.
copy "target\release\chord_lens.dll" "bin\ChordLens.clap" /Y || echo Warning: bin\ChordLens.clap is busy, skipping deploy.

echo Build complete! Plugins are located in the bin/ directory.

echo Creating timestamped release in tmp/...
for /f "tokens=2-4 delims=/ " %%a in ('date /t') do (set mydate=%%c%%a%%b)
set mytime=%time: =0%
set mytime=%mytime::=%
set mytime=%mytime:~0,6%
set dt=%mydate%_%mytime%
set dir=tmp\release_%dt%
if not exist %dir% mkdir %dir%
copy "target\release\chord_lens.dll" "%dir%\ChordLens.vst3" /Y
copy "target\release\chord_lens.dll" "%dir%\ChordLens.clap" /Y
echo Snapshot saved to %dir%

pause
