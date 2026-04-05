@echo off
setlocal

echo Bundling ChordLens in Release mode...
cargo xtask bundle chord-lens --release

if not exist "bin" mkdir "bin"
if not exist "tmp" mkdir "tmp"

echo Deploying bundled VST3 and CLAP...
if exist "bin\ChordLens.vst3" rmdir /S /Q "bin\ChordLens.vst3"
if exist "bin\ChordLens.clap" del /F /Q "bin\ChordLens.clap"
xcopy "target\bundled\ChordLens.vst3" "bin\ChordLens.vst3\" /E /I /Y >nul || echo Warning: bin\ChordLens.vst3 is busy, skipping deploy.
copy "target\bundled\ChordLens.clap" "bin\ChordLens.clap" /Y || echo Warning: bin\ChordLens.clap is busy, skipping deploy.

echo Build complete! Plugins are located in the bin/ directory.

echo Creating timestamped release in tmp/...
for /f "tokens=2-4 delims=/ " %%a in ('date /t') do (set mydate=%%c%%a%%b)
set mytime=%time: =0%
set mytime=%mytime::=%
set mytime=%mytime:~0,6%
set dt=%mydate%_%mytime%
set dir=tmp\release_%dt%
if not exist %dir% mkdir %dir%
if exist "%dir%\ChordLens.vst3" rmdir /S /Q "%dir%\ChordLens.vst3"
xcopy "target\bundled\ChordLens.vst3" "%dir%\ChordLens.vst3\" /E /I /Y >nul
copy "target\bundled\ChordLens.clap" "%dir%\ChordLens.clap" /Y >nul
echo Snapshot saved to %dir%

pause
