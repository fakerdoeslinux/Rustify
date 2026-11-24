# Rustify installer - Windows
# Safe to re-run. Run in PowerShell (not Administrator required).
$ErrorActionPreference = "Stop"

$RepoUrl = "https://github.com/fakerdoeslinux/Rustify.git"
$SrcDir = Join-Path $env:LOCALAPPDATA "Rustify\src"

function Say($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Say "Rust is missing. Installing rustup."
  $installer = Join-Path $env:TEMP "rustup-init.exe"
  Invoke-WebRequest "https://win.rustup.rs/x86_64" -OutFile $installer
  & $installer -y --profile minimal
  $env:Path += ";$env:USERPROFILE\.cargo\bin"
}

# Running from a checkout? Use it. Otherwise clone/fetch the repo.
$here = Join-Path (Get-Location) "Cargo.toml"
if ((Test-Path $here) -and (Select-String -Path $here -Pattern '^name = "rustify"')) {
  Say "Using local checkout: $(Get-Location)"
} else {
  Say "Getting the source."
  if (-not (Test-Path (Join-Path $SrcDir ".git"))) {
    New-Item -ItemType Directory -Force -Path (Split-Path $SrcDir) | Out-Null
    git clone $RepoUrl $SrcDir
  }
  Set-Location $SrcDir
}

Say "Installing librespot (the actual audio engine)."
cargo install librespot --locked

Say "Installing rustify."
cargo install --path . --locked

$config = Join-Path $env:APPDATA "rustify\config.toml"
if (-not (Test-Path $config)) {
  Say "Writing your first config."
  rustify init
}

Say "Done. Next:"
Write-Host "  rustify login    # one-time browser sign-in"
Write-Host "  rustify doctor   # check everything"
Write-Host "  rustify start    # become a Spotify Connect speaker"
Write-Host "Then open Spotify on your phone and pick 'Rustify' under Connect."
