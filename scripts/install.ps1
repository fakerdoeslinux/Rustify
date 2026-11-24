$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
  & "$env:TEMP\rustup-init.exe" -y
  $env:Path += ";$env:USERPROFILE\.cargo\bin"
}

cargo install librespot
cargo install --path .
rustify init

Write-Host "edit $env:APPDATA\rustify\config.toml, then:"
Write-Host "  `$env:RUSTIFY_SPOTIFY_PASSWORD='your password'"
Write-Host "  rustify doctor"
Write-Host "  rustify start"
