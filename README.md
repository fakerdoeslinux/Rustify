# Rustify

Turn your Linux or Windows box into a Spotify Connect speaker with a tiny Rust binary. No Electron. No browser tab pretending to be an app. No 500 MB runtime for playing MP3s.

Rustify is a small manager around [librespot](https://github.com/librespot-org/librespot) — the open-source Rust implementation of Spotify's Connect receiver. librespot does the streaming and decoding; Rustify owns config, login, logs, process lifecycle, and boot startup.

> **Read this first:** this is not "Spotify rewritten from scratch". Spotify's servers, app, and account system stay exactly where they are. What's fast is the *player on your machine*: native audio decode, milliseconds to start, tiny footprint.

---

## Reality check before we install anything

- You need **Spotify Premium**. librespot has never supported free accounts.
- Spotify changed key delivery: **accounts created after an apparent cutoff can fail with `audio key error 0 1`**. That's Spotify-side, and there is no known workaround in librespot or Rustify. If your main account is old enough, you're golden.
- This is unofficial and probably violates Spotify's ToS. Use it at your own risk.
- "Spiceify" almost certainly means **Spicetify** — see [the Spicetify section](#spicetify-yes-that-one) below. Short version: they coexist just fine.

---

## Install — the Chris Titus version

Alright, gather round. We're gonna do the thing. Open a terminal, paste one block, hit Enter, go get a coffee. Rust compiles for a minute. That's normal. We're not paying for it.

### Linux

```bash
curl -fsSL https://raw.githubusercontent.com/fakerdoeslinux/Rustify/master/scripts/install.sh | bash
```

Hate piping random things into bash? Respect. Read the script first — it's short — or do it manually:

```bash
git clone https://github.com/fakerdoeslinux/Rustify.git
cd Rustify
bash scripts/install.sh
```

The script installs Rust if missing, pulls ALSA headers, installs `librespot` and `rustify`, and writes your first config. Then:

```bash
rustify login      # one-time browser sign-in; credentials get cached
rustify doctor     # checks everything, tells you what's wrong
rustify start      # become a speaker
rustify status     # yes, it's actually running
```

Now grab your phone, open Spotify, tap **Connect**, pick **Rustify**. Done. That's the whole trick.

**Boot persistence** (because of course you want it to just be there):

```bash
rustify install-service
systemctl --user daemon-reload
systemctl --user enable --now rustify.service
```

`rustify install-service` writes a systemd user unit that runs librespot directly, with restart-on-failure. If you change `config.toml`, run `rustify install-service` again and `systemctl --user restart rustify.service`.

### Windows

Open **PowerShell** (regular user, no Administrator needed) and paste:

```powershell
irm https://raw.githubusercontent.com/fakerdoeslinux/Rustify/master/scripts/install.ps1 | iex
```

Manual route, if you prefer seeing what you're running:

```powershell
git clone https://github.com/fakerdoeslinux/Rustify.git
cd Rustify
.\scripts\install.ps1
```

Then, in that same PowerShell window:

```powershell
rustify login
rustify doctor
rustify start
rustify status
```

Phone → Spotify → Connect → **Rustify**. Same party, different OS.

**Boot persistence:**

```powershell
rustify install-service
```

That creates a logon task called `Rustify`. Undo it with `rustify uninstall-service`.

> Headless server, no browser? Run `rustify login --oauth-port 0`, copy the redirect URL from any browser into the waiting prompt. Or just start Rustify and pick the device from Spotify Connect on your phone — discovery can authenticate too.

---

## Spicetify (yes, that one)

Good news: nothing to configure.

- **Spicetify** customizes the official Spotify desktop client — themes, extensions, custom apps.
- **Rustify** is a separate Spotify Connect receiver. It doesn't patch, replace, or inspect the desktop app.
- Both can run at the same time. There is no shared config, no port fight, and no "Spicetify update broke my audio sink" situation.
- `rustify open spotify:track:4uLU6hMCjMI75M1A2tKUQC` hands the link to your default handler, so it opens in your Spicetify-themed Spotify client where you can see what's playing.
- `rustify doctor` prints `[ok] spicetify: ...` if it finds Spicetify on your PATH. It's optional — no Spicetify, no problem.

So: Spicetify makes the app look good. Rustify makes your machine a speaker. Different lanes, same Spotify account.

---

## Commands

| Command | What it does |
| --- | --- |
| `rustify init` | Write `config.toml` in your platform config dir |
| `rustify config` | Print the active config |
| `rustify doctor` | Check binary, config, login cache, backends, Spicetify |
| `rustify login` | One-time interactive OAuth sign-in (`--oauth-port 0` for headless) |
| `rustify start` / `stop` / `status` | Run the sink, kill it, or ask if it's alive |
| `rustify open <spotify:uri>` | Open a track/album URI in the desktop client |
| `rustify install-service` / `uninstall-service` | systemd user unit (Linux) or logon task (Windows) |

Config lives at `~/.config/rustify/config.toml` on Linux and `%APPDATA%\rustify\config.toml` on Windows. Logs: `librespot.log` next to it.

```toml
username = ""            # optional; only for cached credentials of one account
device_name = "Rustify"  # what shows up in Spotify Connect
backend = "rodio"        # rodio, alsa, pulseaudio, portaudio, gstreamer, ...
bitrate = 320            # 96, 160, 320
initial_volume = 100
device_type = "computer"
# cache_dir = "/custom/path"       # OAuth credentials live here (chmod 700)
# librespot_path = "/path/to/bin"  # if librespot isn't on PATH
```

Auth options, in order of sanity: cached OAuth from `rustify login`, `RUSTIFY_SPOTIFY_ACCESS_TOKEN` (needs `streaming` scope), or Spotify Connect discovery. **There is no password option** — librespot removed password login in v0.5, and Rustify won't pretend otherwise.

---

## Troubleshooting, aka "it's making a red text"

- **No device shows up in Connect** — same network? mDNS (UDP 5353) blocked by a firewall or guest Wi-Fi? Try `librespot --name Test --backend ?` to confirm librespot itself works, then check `~/.config/rustify/librespot.log`.
- **`audio key error 0 1`** — that's the Spotify account restriction from the top. Nothing to fix here.
- **`backend not compiled in`** — your librespot was built without that backend. `rodio` is the default and works on both OSes. Run `librespot --backend ?` to see what you actually have; rebuild with features if you need ALSA/Pulse.
- **`stale pid file`** — Rustify checked the PID wasn't librespot and cleaned it up itself. Just `rustify start` again.
- **`librespot not found on PATH`** — `cargo install librespot --locked`, or set `librespot_path` in the config.
- **Windows task didn't start audio** — the logon task calls `rustify start`; make sure the OAuth cache exists (`rustify login` once, as your user).

---

## Build from source

```bash
git clone https://github.com/fakerdoeslinux/Rustify.git
cd Rustify
cargo test
cargo build --release
cargo install --path . --locked
```

CI builds and tests on Ubuntu and Windows on every push. Linux uses ALSA headers for the default backend; Windows uses rodio and needs no extra SDK.

## License

MIT. librespot is also MIT — see their repo for their own license and disclaimer.
