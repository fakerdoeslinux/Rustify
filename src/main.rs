use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_BITRATE: u16 = 320;
const DEFAULT_VOLUME: u8 = 100;
const DEFAULT_DEVICE: &str = "Rustify";
const DEFAULT_BACKEND: &str = "rodio";
const VALID_BITRATES: [u16; 3] = [96, 160, 320];
const VALID_DEVICE_TYPES: [&str; 15] = [
    "computer",
    "tablet",
    "smartphone",
    "speaker",
    "tv",
    "avr",
    "stb",
    "audiodongle",
    "gameconsole",
    "castaudio",
    "castvideo",
    "automobile",
    "smartwatch",
    "chromebook",
    "carthing",
];

#[derive(Parser)]
#[command(
    name = "rustify",
    version,
    about = "Headless Spotify Connect sink manager written in Rust"
)]
struct Cli {
    #[command(subcommand)]
    cmd: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    /// Write the default config file
    Init,
    /// Print the active config
    Config,
    /// Check the install and print useful diagnostics
    Doctor,
    /// One-time browser sign-in (OAuth), cached for later runs
    Login {
        /// OAuth redirect port. Use 0 for headless/manual redirect.
        #[arg(long)]
        oauth_port: Option<u16>,
    },
    /// Start the Spotify Connect sink
    Start,
    /// Stop the Spotify Connect sink
    Stop,
    /// Show whether the sink is running
    Status,
    /// Hand a spotify: URI to the default desktop client (Spicetify-friendly)
    Open { uri: String },
    /// Install a systemd user unit (Linux) or logon task (Windows)
    InstallService,
    /// Remove the systemd user unit (Linux) or logon task (Windows)
    UninstallService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    /// Only needed when reusing cached credentials for a specific account.
    #[serde(default)]
    username: String,
    #[serde(default = "default_device")]
    device_name: String,
    #[serde(default = "default_backend")]
    backend: String,
    #[serde(default = "default_bitrate")]
    bitrate: u16,
    #[serde(default = "default_volume")]
    initial_volume: u8,
    #[serde(default = "default_device_type")]
    device_type: String,
    /// Where librespot stores OAuth credentials. Defaults to <config dir>/cache.
    #[serde(default)]
    cache_dir: Option<PathBuf>,
    /// Optional absolute path to the librespot binary.
    #[serde(default)]
    librespot_path: Option<PathBuf>,
}

fn default_device() -> String {
    DEFAULT_DEVICE.into()
}
fn default_backend() -> String {
    DEFAULT_BACKEND.into()
}
fn default_bitrate() -> u16 {
    DEFAULT_BITRATE
}
fn default_volume() -> u8 {
    DEFAULT_VOLUME
}
fn default_device_type() -> String {
    "computer".into()
}

fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|p| p.join("rustify"))
        .ok_or_else(|| anyhow!("no config dir on this platform"))
}

fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

fn pid_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("rustify.pid"))
}

fn log_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("librespot.log"))
}

fn cache_dir(cfg: &Config) -> Result<PathBuf> {
    match &cfg.cache_dir {
        Some(p) => Ok(p.clone()),
        None => Ok(config_dir()?.join("cache")),
    }
}

fn load_config() -> Result<Config> {
    let path = config_path()?;
    let text = fs::read_to_string(&path)
        .with_context(|| format!("missing config at {} - run `rustify init`", path.display()))?;
    let cfg: Config = toml::from_str(&text).context("bad config.toml")?;
    validate_config(&cfg)?;
    Ok(cfg)
}

fn validate_config(cfg: &Config) -> Result<()> {
    if !VALID_BITRATES.contains(&cfg.bitrate) {
        bail!(
            "bitrate must be one of {:?}, got {}",
            VALID_BITRATES,
            cfg.bitrate
        );
    }
    if cfg.initial_volume > 100 {
        bail!("initial_volume must be 0-100, got {}", cfg.initial_volume);
    }
    if cfg.device_name.trim().is_empty() {
        bail!("device_name cannot be empty");
    }
    if cfg.backend.trim().is_empty() {
        bail!("backend cannot be empty");
    }
    if !VALID_DEVICE_TYPES.contains(&cfg.device_type.as_str()) {
        bail!(
            "device_type must be one of {:?}, got {}",
            VALID_DEVICE_TYPES,
            cfg.device_type
        );
    }
    Ok(())
}

fn write_default_config() -> Result<PathBuf> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)?;
    let path = config_path()?;
    if path.exists() {
        bail!("config already exists at {}", path.display());
    }
    let cfg = Config {
        username: String::new(),
        device_name: default_device(),
        backend: default_backend(),
        bitrate: DEFAULT_BITRATE,
        initial_volume: DEFAULT_VOLUME,
        device_type: default_device_type(),
        cache_dir: None,
        librespot_path: None,
    };
    let body = toml::to_string_pretty(&cfg)?;
    let header = concat!(
        "# Rustify configuration\n",
        "# Run `rustify login` once to sign in, then `rustify start`.\n",
        "# Leave username empty unless you want cached credentials for one account.\n",
        "# bitrate: 96, 160 or 320. backend: rodio, alsa, pulseaudio, ...\n",
        "\n",
    );
    fs::write(&path, format!("{header}{body}"))?;
    Ok(path)
}

fn librespot_bin(cfg: &Config) -> Result<PathBuf> {
    if let Some(p) = &cfg.librespot_path {
        if p.is_file() {
            return Ok(p.clone());
        }
        bail!("librespot_path is not a file: {}", p.display());
    }
    which::which("librespot").map_err(|_| {
        anyhow!("librespot not found on PATH - install it with: cargo install librespot --locked")
    })
}

fn librespot_args(cfg: &Config) -> Result<Vec<String>> {
    let cache = cache_dir(cfg)?;
    let mut args = vec![
        "--name".into(),
        cfg.device_name.clone(),
        "--bitrate".into(),
        cfg.bitrate.to_string(),
        "--backend".into(),
        cfg.backend.clone(),
        "--initial-volume".into(),
        cfg.initial_volume.to_string(),
        "--device-type".into(),
        cfg.device_type.clone(),
        "--cache".into(),
        cache.to_string_lossy().into_owned(),
    ];
    if !cfg.username.is_empty() {
        args.push("--username".into());
        args.push(cfg.username.clone());
    }
    // Password login was removed in librespot 0.5. Access tokens are optional;
    // OAuth (via `rustify login`) or Spotify Connect discovery are preferred.
    if let Ok(token) = env::var("RUSTIFY_SPOTIFY_ACCESS_TOKEN") {
        if !token.is_empty() {
            args.push("--access-token".into());
            args.push(token);
        }
    }
    Ok(args)
}

fn prepare_cache(cfg: &Config) -> Result<PathBuf> {
    let dir = cache_dir(cfg)?;
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

fn cache_has_credentials(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|mut entries| {
            entries.any(|entry| {
                entry
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_lowercase();
                        name.contains("cred") || name.contains("auth") || name.contains("token")
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn read_pid(path: &Path) -> Option<String> {
    let pid = fs::read_to_string(path).ok()?.trim().to_string();
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(pid)
}

fn process_exists(pid: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .to_ascii_lowercase()
                    .contains(&format!(",{pid},"))
            })
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn process_is_librespot(pid: &str) -> bool {
    if !process_exists(pid) {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        fs::read(format!("/proc/{pid}/cmdline"))
            .map(|bytes| {
                let cmdline = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
                cmdline.contains("librespot")
            })
            .unwrap_or(false)
    }
    // On Windows tasklist already reported the image name; check it explicitly.
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .to_ascii_lowercase()
                    .contains("librespot.exe")
            })
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        true
    }
}

fn start() -> Result<()> {
    let cfg = load_config()?;
    if status()? {
        bail!("rustify sink is already running");
    }
    let bin = librespot_bin(&cfg)?;
    let cache = prepare_cache(&cfg)?;
    let args = librespot_args(&cfg)?;

    let log = log_path()?;
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)?;
    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Detach from the controlling terminal so closing the shell doesn't kill audio.
        cmd.process_group(0);
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("failed to launch {}", bin.display()))?;
    let pid = child.id().to_string();
    fs::write(pid_path()?, &pid)?;

    println!("started, pid {pid}");
    println!("log: {}", log.display());
    if !cache_has_credentials(&cache) && env::var_os("RUSTIFY_SPOTIFY_ACCESS_TOKEN").is_none() {
        println!("no cached login found - run `rustify login`, or pick `{}` in Spotify Connect to sign in", cfg.device_name);
    }
    Ok(())
}

fn stop() -> Result<()> {
    let path = pid_path()?;
    let pid = read_pid(&path).context("no pid file - nothing to stop")?;
    if !process_is_librespot(&pid) {
        let _ = fs::remove_file(&path);
        println!("stale pid file removed; sink was not running");
        return Ok(());
    }
    let ok = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .status()?
            .success()
    } else {
        Command::new("kill").arg(&pid).status()?.success()
    };
    let _ = fs::remove_file(&path);
    if !ok {
        bail!("process {pid} did not stop cleanly");
    }
    println!("stopped pid {pid}");
    Ok(())
}

fn status() -> Result<bool> {
    let path = pid_path()?;
    let pid = match read_pid(&path) {
        Some(p) => p,
        None => {
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
            return Ok(false);
        }
    };
    if process_is_librespot(&pid) {
        println!("running, pid {pid}");
        Ok(true)
    } else {
        println!("not running (stale pid file removed)");
        let _ = fs::remove_file(&path);
        Ok(false)
    }
}

fn login(cfg: &Config, oauth_port: Option<u16>) -> Result<()> {
    let bin = librespot_bin(cfg)?;
    let cache = prepare_cache(cfg)?;
    let mut args = vec![
        "--cache".into(),
        cache.to_string_lossy().into_owned(),
        "--enable-oauth".into(),
    ];
    if let Some(port) = oauth_port {
        args.push("--oauth-port".into());
        args.push(port.to_string());
    }
    println!("Opening OAuth sign-in. Follow the URL printed below in your browser.");
    println!("Credentials are cached in {}", cache.display());
    let status = Command::new(&bin)
        .args(&args)
        .status()
        .with_context(|| format!("failed to launch {}", bin.display()))?;
    if !status.success() {
        bail!("OAuth login exited with {status}");
    }
    println!("login finished - try `rustify doctor` then `rustify start`");
    Ok(())
}

fn doctor() -> Result<()> {
    let mut bad = false;
    let cfg = match load_config() {
        Ok(cfg) => {
            println!("[ok] config: {}", config_path()?.display());
            Some(cfg)
        }
        Err(err) => {
            println!("[no] config: {err:#}");
            bad = true;
            None
        }
    };

    match cfg.as_ref().map(librespot_bin) {
        Some(Ok(path)) => {
            let version = Command::new(&path)
                .arg("--version")
                .output()
                .ok()
                .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
                .unwrap_or_default();
            println!("[ok] librespot: {} {}", path.display(), version);
        }
        Some(Err(err)) => {
            println!("[no] librespot: {err:#}");
            bad = true;
        }
        None => {}
    }

    if let Some(cfg) = &cfg {
        let cache = cache_dir(cfg)?;
        if cache_has_credentials(&cache) {
            println!("[ok] cached login: {}", cache.display());
        } else {
            println!(
                "[--] no cached login in {} - run `rustify login`",
                cache.display()
            );
        }
        println!(
            "[ok] config checks out ({} backend, {} kbps)",
            cfg.backend, cfg.bitrate
        );

        match librespot_bin(cfg).and_then(|bin| {
            Command::new(bin)
                .args(["--backend", "?"])
                .output()
                .context("failed to list librespot backends")
        }) {
            Ok(out) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                let backends: Vec<&str> = text
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                if backends.is_empty() {
                    println!("[--] backend list unavailable; run `librespot --backend ?`");
                } else {
                    println!("[ok] compiled backends: {}", backends.join(", "));
                }
            }
            Err(_) => println!("[--] backend list unavailable; run `librespot --backend ?`"),
        }
    }

    if env::var_os("RUSTIFY_SPOTIFY_ACCESS_TOKEN").is_some() {
        println!("[ok] access token env set");
    } else {
        println!("[--] access token env unset (OAuth cache or Connect discovery is fine)");
    }
    match which::which("spicetify") {
        Ok(path) => println!("[ok] spicetify (optional): {}", path.display()),
        Err(_) => println!("[--] spicetify not found (optional; themes stay in the desktop app)"),
    }
    println!("[!!] needs Spotify Premium and an account eligible for audio keys");
    println!("     (new accounts can fail with `audio key error 0 1` - that is Spotify-side)");

    if bad {
        bail!("doctor found problems");
    }
    Ok(())
}

fn open_uri(uri: &str) -> Result<()> {
    if !uri.starts_with("spotify:") {
        bail!("uri must start with `spotify:`");
    }
    if cfg!(windows) {
        Command::new("cmd").args(["/C", "start", "", uri]).spawn()?;
    } else {
        Command::new("xdg-open").arg(uri).spawn()?;
    }
    println!("handed {uri} to the default handler (your Spicetify-themed Spotify client)");
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(target_os = "linux")]
fn unit_body(bin: &Path, args: &[String]) -> String {
    let command = std::iter::once(bin.to_string_lossy().to_string())
        .chain(args.iter().cloned())
        .map(|arg| systemd_quote(&arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "[Unit]\n\
         Description=Rustify Spotify Connect sink\n\
         After=network-online.target sound.target\n\
         Wants=network-online.target\n\n\
         [Service]\n\
         Type=simple\n\
         ExecStart={command}\n\
         Restart=on-failure\n\
         RestartSec=5\n\n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

#[cfg(target_os = "linux")]
fn service_unit_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow!("no config dir on this platform"))?;
    Ok(base.join("systemd").join("user").join("rustify.service"))
}

fn install_service() -> Result<()> {
    let _cfg = load_config()?;

    #[cfg(target_os = "linux")]
    {
        let bin = librespot_bin(&_cfg)?;
        let args = librespot_args(&_cfg)?;
        let unit = service_unit_path()?;
        fs::create_dir_all(unit.parent().expect("unit has a parent"))?;
        fs::write(&unit, unit_body(&bin, &args))?;
        println!("unit written: {}", unit.display());
        println!("now run:");
        println!("  systemctl --user daemon-reload");
        println!("  systemctl --user enable --now rustify.service");
    }
    #[cfg(target_os = "windows")]
    {
        let exe = env::current_exe()?;
        let task = format!("\"{}\" start", exe.display());
        let out = Command::new("schtasks")
            .args([
                "/Create", "/TN", "Rustify", "/TR", &task, "/SC", "ONLOGON", "/F",
            ])
            .output()?;
        if !out.status.success() {
            bail!("schtasks failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        println!("installed logon task: Rustify");
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = cfg;
        bail!("install-service is only supported on Linux and Windows");
    }
    Ok(())
}

fn uninstall_service() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let unit = service_unit_path()?;
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "rustify.service"])
            .status();
        if unit.exists() {
            fs::remove_file(&unit)?;
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        println!("removed unit: {}", unit.display());
    }
    #[cfg(target_os = "windows")]
    {
        let out = Command::new("schtasks")
            .args(["/Delete", "/TN", "Rustify", "/F"])
            .output()?;
        if !out.status.success() {
            bail!(
                "schtasks delete failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        println!("removed logon task: Rustify");
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        bail!("uninstall-service is only supported on Linux and Windows");
    }
    Ok(())
}

fn show_config() -> Result<()> {
    let path = config_path()?;
    let mut text = fs::read_to_string(&path)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    std::io::stdout().write_all(text.as_bytes())?;
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.cmd {
        CommandKind::Init => write_default_config().map(|p| println!("wrote {}", p.display())),
        CommandKind::Config => show_config(),
        CommandKind::Doctor => doctor(),
        CommandKind::Login { oauth_port } => load_config().and_then(|cfg| login(&cfg, oauth_port)),
        CommandKind::Start => start(),
        CommandKind::Stop => stop(),
        CommandKind::Status => status().map(|_| ()),
        CommandKind::Open { uri } => open_uri(&uri),
        CommandKind::InstallService => install_service(),
        CommandKind::UninstallService => uninstall_service(),
    };
    if let Err(err) = result {
        eprintln!("rustify: {err:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> Config {
        Config {
            username: String::new(),
            device_name: default_device(),
            backend: default_backend(),
            bitrate: DEFAULT_BITRATE,
            initial_volume: DEFAULT_VOLUME,
            device_type: default_device_type(),
            cache_dir: Some(PathBuf::from("/tmp/rustify-test-cache")),
            librespot_path: None,
        }
    }

    #[test]
    fn minimal_config_gets_defaults() {
        let cfg: Config = toml::from_str("username = \"me\"").unwrap();
        assert_eq!(cfg.device_name, "Rustify");
        assert_eq!(cfg.backend, "rodio");
        assert_eq!(cfg.bitrate, 320);
        assert_eq!(cfg.initial_volume, 100);
        assert_eq!(cfg.device_type, "computer");
    }

    #[test]
    fn args_use_current_librespot_flags() {
        let args = librespot_args(&test_cfg()).unwrap();
        for flag in [
            "--name",
            "--bitrate",
            "--backend",
            "--initial-volume",
            "--cache",
        ] {
            assert!(args.iter().any(|a| a == flag), "missing {flag}");
        }
        assert!(!args
            .iter()
            .any(|a| a == "--device-name" || a == "--volume" || a == "--password"));
        assert!(args.iter().any(|a| a == "320"));
    }

    #[test]
    fn config_validation_rejects_bad_values() {
        let mut cfg = test_cfg();
        cfg.bitrate = 123;
        assert!(validate_config(&cfg).is_err());
        cfg.bitrate = 160;
        cfg.initial_volume = 101;
        assert!(validate_config(&cfg).is_err());
        cfg.initial_volume = 80;
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn systemd_unit_contains_quoted_execstart() {
        let unit = unit_body(
            Path::new("/opt/my bin/librespot"),
            &["--name".into(), "my speaker".into()],
        );
        assert!(unit.contains("ExecStart=\"/opt/my bin/librespot\" \"--name\" \"my speaker\""));
    }
}
