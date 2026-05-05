use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use texpand_core::{
    expander::{ExpandOptions, expand},
    resolver::FileResolver,
};

mod config;

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
    fn resolve(&self, includer_path: &Path, include_path: &str) -> Result<PathBuf> {
        let path = Path::new(include_path);
        if path.is_absolute() {
            return std::fs::canonicalize(path).with_context(|| "failed to canonicalize path");
        }

        if let Some(includer_dir) = Path::new(includer_path).parent() {
            let candidate = includer_dir.join(path);
            if candidate.exists() {
                return std::fs::canonicalize(candidate)
                    .with_context(|| "failed to canonicalize path");
            }
        }

        for inc_path in &self.include_paths {
            let candidate = inc_path.join(path);
            if candidate.exists() {
                return std::fs::canonicalize(candidate)
                    .with_context(|| "failed to canonicalize path");
            }
        }

        anyhow::bail!("include file not found: {include_path}")
    }

    fn read_content(&self, resolved_path: &Path) -> Result<String> {
        std::fs::read_to_string(resolved_path).with_context(|| "failed to read content")
    }
}

/// Fork a child process that holds clipboard contents alive on Linux.
/// Returns immediately — the child process outlives the parent.
#[cfg(target_os = "linux")]
fn copy_to_clipboard(text: &str) -> Result<()> {
    use arboard::SetExtLinux;
    use nix::unistd::{ForkResult, fork};

    let text = text.to_owned();

    match unsafe { fork() }.context("failed to fork clipboard daemon")? {
        ForkResult::Child => {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set().wait().text(text);
            }
            std::process::exit(0);
        }
        ForkResult::Parent { child: _ } => Ok(()),
    }
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

    let (entry_path, entry_source, source_dir) = if args.input.as_os_str() == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .context("failed to read source from stdin")?;
        let cwd = std::env::current_dir().context("failed to get current directory")?;
        ("-".into(), source, cwd)
    } else {
        let path = std::fs::canonicalize(&args.input)
            .with_context(|| format!("cannot access '{}'", args.input.display()))?;
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read '{}'", args.input.display()))?;
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        (path, source, dir)
    };

    let mut resolver_paths = Vec::new();
    resolver_paths.push(source_dir);
    resolver_paths.extend(cli_include_paths);
    let resolver = FsResolver::new(resolver_paths);

    let opts = ExpandOptions { compress };
    let result = expand(&entry_path, &entry_source, &resolver, &opts)?;

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
