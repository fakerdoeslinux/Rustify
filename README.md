# Rustify

Headless Spotify Connect sink manager written in Rust. Thin, fast, no Electron.
librespot does the audio; Rustify owns config, process lifecycle, logs, and startup.

## What it does

- `rustify init` — writes `config.toml` in your platform config dir
- `rustify config` — prints the active config
- `rustify doctor` — checks binary, config, password, backends
- `rustify start` / `rustify stop` / `rustify status` — process control
- `rustify open spotify:track:...` — hands a URI to the Spotify client
- `rustify install-service` — systemd user unit (Linux) or logon task (Windows)

## Install — Linux

1. Open a terminal.
2. Paste the whole block:

```bash
git clone https://github.com/fakerdoeslinux/Rustify.git
cd Rustify
bash scripts/install.sh
```

3. Edit the config:

```bash
nano ~/.config/rustify/config.toml
```

Put your real username in `username`. Leave `password` empty. Use the env var.

4. Set the password for this shell:

```bash
export RUSTIFY_SPOTIFY_PASSWORD='your password'
```

5. Check it:

```bash
rustify doctor
rustify start
rustify status
```

6. Open your phone, Spotify, Connect, pick the device named `Rustify`.

7. Boot persistence:

```bash
rustify install-service
systemctl --user enable --now ~/.config/rustify/rustify.service
```

## Install — Windows

1. Open PowerShell. Not Administrator.
2. Paste the whole block:

```powershell
git clone https://github.com/fakerdoeslinux/Rustify.git
cd Rustify
.\scripts\install.ps1
```

3. Edit the config:

```powershell
notepad "$env:APPDATA\rustify\config.toml"
```

4. Set the password in that same PowerShell window:

```powershell
$env:RUSTIFY_SPOTIFY_PASSWORD='your password'
```

5. Check it:

```powershell
rustify doctor
rustify start
rustify status
```

6. Phone, Spotify, Connect, device `Rustify`.

7. Boot persistence:

```powershell
rustify install-service
```

Run `rustify uninstall-service` to undo.

## Backends

`rodio` is the default. Swap in `alsa`, `pulseaudio`, `portaudio`, or `cubeb` in config.
Linux package deps: `libasound2-dev libpulse-dev portaudio19-dev libcubeb-dev`.
Windows stays on `rodio`.

## Build from source

```bash
cargo build --release
cargo install --path .
```

CI runs on Ubuntu and Windows.

## Why this is faster

librespot is native Rust audio decode and streaming, no browser shell, no Electron.
Rustify adds a ~few ms process wrapper with a config file and a pid file. The
speed comes from the sink, not from a rewrite of Spotify's servers.
