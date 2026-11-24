use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DEFAULT_BITRATE: u16 = 320;
const DEFAULT_VOLUME: u8 = 100;

#[derive(Parser)]
#[command(name = "rustify", about = "Headless Spotify Connect sink manager")]
struct Cli {
    #[command(subcommand)]
    cmd: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    Init,
    Config,
    Start,
    Stop,
    Status,
    Doctor,
    InstallService,
    UninstallService,
    Open { uri: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    username: String,
    #[serde(default)]
    password: String,
    #[serde(default = "default_device")]
    device_name: String,
    #[serde(default = "default_backend")]
    backend: String,
    #[serde(default = "default_bitrate")]
    bitrate: u16,
    #[serde(default = "default_volume")]
    volume: u8,
    #[serde(default)]
    client_id: String,
}

fn default_device() -> String {
    "Rustify".into()
}
fn default_backend() -> String {
    "rodio".into()
}
fn default_bitrate() -> u16 {
    DEFAULT_BITRATE
}
fn default_volume() -> u8 {
    DEFAULT_VOLUME
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

fn load_config() -> Result<Config> {
    let path = config_path()?;
    let text = fs::read_to_string(&path)
        .with_context(|| format!("missing config at {}", path.display()))?;
    toml::from_str(&text).context("bad config.toml")
}

fn write_default_config() -> Result<PathBuf> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)?;
    let path = config_path()?;
    if path.exists() {
        bail!("config already exists at {}", path.display());
    }
    let cfg = Config {
        username: "your_spotify_username".into(),
        password: String::new(),
        device_name: default_device(),
        backend: default_backend(),
        bitrate: DEFAULT_BITRATE,
        volume: DEFAULT_VOLUME,
        client_id: String::new(),
    };
    let text = toml::to_string_pretty(&cfg)?;
    fs::write(&path, text)?;
    Ok(path)
}

fn find_librespot() -> Result<String> {
    which("librespot").map_err(|_| anyhow!("librespot not found on PATH - cargo install librespot"))
}

fn which(name: &str) -> Result<String> {
    let out = Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(name)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("not found"));
    }
    let line = String::from_utf8_lossy(&out.stdout);
    line.lines()
        .next()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| anyhow!("empty lookup result"))
}

fn librespot_args(cfg: &Config) -> Vec<String> {
    let mut args = vec![
        "--username".into(),
        cfg.username.clone(),
        "--device-name".into(),
        cfg.device_name.clone(),
        "--backend".into(),
        cfg.backend.clone(),
        "--bitrate".into(),
        cfg.bitrate.to_string(),
        "--volume".into(),
        cfg.volume.to_string(),
    ];
    if let Ok(pass) = std::env::var("RUSTIFY_SPOTIFY_PASSWORD") {
        args.push("--password".into());
        args.push(pass);
    } else if !cfg.password.is_empty() {
        args.push("--password".into());
        args.push(cfg.password.clone());
    }
    if !cfg.client_id.is_empty() {
        args.push("--client-id".into());
        args.push(cfg.client_id.clone());
    }
    args
}

fn start() -> Result<()> {
    let cfg = load_config()?;
    if cfg.username == "your_spotify_username" {
        bail!("edit config.toml before starting");
    }
    if pid_path()?.exists() && status().unwrap_or(false) {
        bail!("rustify sink is already running");
    }
    let bin = find_librespot()?;
    let log = log_path()?;
    let file = fs::File::create(&log)?;
    let child = Command::new(bin)
        .args(librespot_args(&cfg))
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        .spawn()?;
    fs::write(pid_path()?, child.id().to_string())?;
    println!("started, pid {}", child.id());
    println!("log: {}", log.display());
    Ok(())
}

fn stop() -> Result<()> {
    let path = pid_path()?;
    let pid = fs::read_to_string(&path)
        .context("no pid file - nothing to stop")?
        .trim()
        .to_string();
    let ok = if cfg!(windows) {
        Command::new("taskkill").args(["/PID", &pid, "/F"]).status()?.success()
    } else {
        Command::new("kill").arg(&pid).status()?.success()
    };
    fs::remove_file(&path)?;
    if !ok {
        bail!("process {} did not stop cleanly", pid);
    }
    println!("stopped pid {}", pid);
    Ok(())
}

fn status() -> Result<bool> {
    let path = pid_path()?;
    let pid = match fs::read_to_string(&path) {
        Ok(p) => p.trim().to_string(),
        Err(_) => return Ok(false),
    };
    let alive = if cfg!(windows) {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()?
            .stdout
            .windows(pid.len())
            .any(|w| w == pid.as_bytes())
    } else {
        Command::new("kill").args(["-0", &pid]).status()?.success()
    };
    if alive {
        println!("running, pid {}", pid);
    } else {
        println!("stale pid file, sink not running");
        let _ = fs::remove_file(path);
    }
    Ok(alive)
}

fn doctor() -> Result<()> {
    let mut bad = false;
    match find_librespot() {
        Ok(p) => println!("[ok] librespot: {}", p),
        Err(_) => {
            println!("[no] librespot - cargo install librespot --locked");
            bad = true;
        }
    }
    match config_path() {
        Ok(p) if p.exists() => println!("[ok] config: {}", p.display()),
        _ => {
            println!("[no] config - rustify init");
            bad = true;
        }
    }
    match std::env::var("RUSTIFY_SPOTIFY_PASSWORD") {
        Ok(_) => println!("[ok] password env set"),
        Err(_) => println!("[--] password env unset (config password may be used)"),
    }
    println!("[--] audio backends: rodio, alsa, pulseaudio, portaudio, cubeb");
    if bad {
        bail!("doctor found problems");
    }
    Ok(())
}

fn open_uri(uri: &str) -> Result<()> {
    if !uri.starts_with("spotify:") {
        bail!("uri must start with spotify:");
    }
    if cfg!(windows) {
        Command::new("cmd").args(["/C", "start", "", uri]).spawn()?;
    } else {
        Command::new("xdg-open").arg(uri).spawn()?;
    }
    println!("handed {} to the spotify handler", uri);
    Ok(())
}

fn install_service() -> Result<()> {
    let exe = std::env::current_exe()?;
    if cfg!(windows) {
        let task = format!("\"{}\" start", exe.display());
        let out = Command::new("schtasks")
            .args(["/Create", "/TN", "Rustify", "/TR", &task, "/SC", "ONLOGON", "/F"])
            .output()?;
        if !out.status.success() {
            bail!("schtasks failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        println!("installed logon task: Rustify");
    } else {
        let dir = config_dir()?;
        fs::create_dir_all(&dir)?;
        let unit = dir.join("rustify.service");
        let body = format!(
            "[Unit]\nDescription=Rustify Spotify Connect sink\nAfter=network-online.target sound.target\nWants=network-online.target\n\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart=\"{}\" start\nExecStop=\"{}\" stop\n\n[Install]\nWantedBy=default.target\n",
            exe.display(),
            exe.display()
        );
        fs::write(&unit, body)?;
        println!("unit written: {}", unit.display());
        println!("enable with: systemctl --user enable --now {}", unit.display());
    }
    Ok(())
}

fn uninstall_service() -> Result<()> {
    if cfg!(windows) {
        let out = Command::new("schtasks")
            .args(["/Delete", "/TN", "Rustify", "/F"])
            .output()?;
        if !out.status.success() {
            bail!("schtasks delete failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        println!("removed logon task: Rustify");
    } else {
        let unit = config_dir()?.join("rustify.service");
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", &unit.to_string_lossy()])
            .status();
        fs::remove_file(&unit)?;
        println!("removed unit: {}", unit.display());
    }
    Ok(())
}

fn show_config() -> Result<()> {
    let path = config_path()?;
    println!("{}", fs::read_to_string(path)?);
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.cmd {
        CommandKind::Init => write_default_config().map(|p| println!("wrote {}", p.display())),
        CommandKind::Config => show_config(),
        CommandKind::Start => start(),
        CommandKind::Stop => stop(),
        CommandKind::Status => status().map(|_| ()),
        CommandKind::Doctor => doctor(),
        CommandKind::InstallService => install_service(),
        CommandKind::UninstallService => uninstall_service(),
        CommandKind::Open { uri } => open_uri(&uri),
    };
    if let Err(err) = result {
        eprintln!("rustify: {:#}", err);
        std::process::exit(1);
    }
}
