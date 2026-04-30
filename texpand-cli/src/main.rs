use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use texpand_core::{
    expander::{ExpandOptions, expand},
    resolver::FileResolver,
};

mod config;

/// Internal argument used to spawn a clipboard daemon child process on Linux.
/// The daemon reads text from stdin, sets the clipboard, and blocks with `wait()`
/// until the clipboard is overwritten, then exits silently.
const CLIPBOARD_DAEMON_ARG: &str = "__texpand_clipboard_daemon";

#[derive(Parser)]
#[command(
    name = "texpand",
    version,
    about = "Expand C/C++ #include dependencies into a single file"
)]
struct Cli {
    /// Input C/C++ source file to expand
    input: PathBuf,

    /// Enable output compression (overrides config)
    #[arg(short = 'c', long = "compress", overrides_with = "no_compress")]
    compress: bool,

    /// Disable output compression (overrides config)
    #[arg(long = "no-compress", overrides_with = "compress")]
    no_compress: bool,

    /// Add an include search path (repeatable, overrides config file)
    #[arg(short = 'i', long = "include")]
    include: Vec<String>,

    /// Write output to a file (instead of stdout)
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Copy output to clipboard
    #[arg(short = 'C', long = "clipboard", conflicts_with = "output")]
    clipboard: bool,

    /// Path to config file (default: ~/.config/texpand.toml)
    #[arg(long = "config")]
    config: Option<PathBuf>,
}

struct FsResolver {
    include_paths: Vec<PathBuf>,
}

impl FsResolver {
    fn new(include_paths: Vec<PathBuf>) -> Self {
        Self { include_paths }
    }
}

impl FileResolver for FsResolver {
    fn resolve_and_read(
        &self,
        includer_path: &str,
        include_path: &str,
    ) -> Result<(String, String)> {
        let path = Path::new(include_path);

        if path.is_absolute() {
            let content = std::fs::read_to_string(path)?;
            let canonical = std::fs::canonicalize(path)?;
            return Ok((canonical.to_string_lossy().to_string(), content));
        }

        if let Some(includer_dir) = Path::new(includer_path).parent() {
            let candidate = includer_dir.join(path);
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                let canonical = std::fs::canonicalize(&candidate)?;
                return Ok((canonical.to_string_lossy().to_string(), content));
            }
        }

        for inc_path in &self.include_paths {
            let candidate = inc_path.join(path);
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                let canonical = std::fs::canonicalize(&candidate)?;
                return Ok((canonical.to_string_lossy().to_string(), content));
            }
        }

        anyhow::bail!("include file not found: {include_path}")
    }
}

/// Run as a clipboard daemon: read text from stdin, set clipboard with `wait()`,
/// block until overwritten, then exit.
#[cfg(target_os = "linux")]
fn run_clipboard_daemon() -> Result<()> {
    use arboard::SetExtLinux;
    use std::io::Read;

    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .context("failed to read clipboard text from stdin")?;

    let mut clipboard = arboard::Clipboard::new().context("failed to open clipboard")?;
    clipboard
        .set()
        .wait()
        .text(text)
        .context("failed to set clipboard text")?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn run_clipboard_daemon() -> Result<()> {
    unreachable!("clipboard daemon is only used on Linux")
}

/// Spawn a detached child process that holds clipboard contents alive on Linux.
/// Returns immediately — the child process outlives the parent.
#[cfg(target_os = "linux")]
fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::process::{Command, Stdio};

    let exe = std::env::current_exe().context("failed to get current executable path")?;

    let mut child = Command::new(exe)
        .arg(CLIPBOARD_DAEMON_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir("/")
        .spawn()
        .context("failed to spawn clipboard daemon process")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .context("failed to write clipboard data to daemon")?;
        // Drop stdin to close the pipe so the daemon can proceed
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open clipboard")?;
    clipboard
        .set_text(text)
        .context("failed to set clipboard text")?;
    Ok(())
}

fn main() -> Result<()> {
    // On Linux, if invoked as a clipboard daemon, handle and exit early.
    if std::env::args().nth(1).as_deref() == Some(CLIPBOARD_DAEMON_ARG) {
        return run_clipboard_daemon();
    }

    let args = Cli::parse();

    let config = config::TexpandConfig::load(args.config.as_deref())?;

    let cli_include_paths: Vec<PathBuf> = if args.include.is_empty() {
        config.include_paths.iter().map(PathBuf::from).collect()
    } else {
        args.include.iter().map(PathBuf::from).collect()
    };

    let compress = if args.compress {
        true
    } else if args.no_compress {
        false
    } else {
        config.default_compress
    };

    let entry_path = std::fs::canonicalize(&args.input)
        .with_context(|| format!("cannot access '{}'", args.input.display()))?;
    let entry_source = std::fs::read_to_string(&entry_path)
        .with_context(|| format!("cannot read '{}'", args.input.display()))?;

    let mut resolver_paths = Vec::new();
    resolver_paths.push(entry_path.parent().unwrap_or(Path::new(".")).to_path_buf());
    resolver_paths.extend(cli_include_paths);
    let resolver = FsResolver::new(resolver_paths);

    let opts = ExpandOptions { compress };
    let result = expand(
        &entry_path.to_string_lossy(),
        &entry_source,
        &resolver,
        &opts,
    )?;

    if args.clipboard {
        copy_to_clipboard(&result)?;
    } else if let Some(output_path) = args.output {
        std::fs::write(&output_path, &result)
            .with_context(|| format!("failed to write '{}'", output_path.display()))?;
    } else {
        print!("{result}");
    }

    Ok(())
}
