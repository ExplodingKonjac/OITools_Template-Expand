use std::path::{Path, PathBuf};

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
    fn resolve_and_read(
        &self,
        includer_path: &str,
        include_path: &str,
    ) -> Result<(String, String)> {
        let path = Path::new(include_path);

        // Absolute path — use directly
        if path.is_absolute() {
            let content = std::fs::read_to_string(path)?;
            let canonical = std::fs::canonicalize(path)?;
            return Ok((canonical.to_string_lossy().to_string(), content));
        }

        // Try relative to the includer's directory first (standard C preprocessor behavior)
        if let Some(includer_dir) = Path::new(includer_path).parent() {
            let candidate = includer_dir.join(path);
            if candidate.exists() {
                let content = std::fs::read_to_string(&candidate)?;
                let canonical = std::fs::canonicalize(&candidate)?;
                return Ok((canonical.to_string_lossy().to_string(), content));
            }
        }

        // Then search configured include paths in order
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

fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = arboard::Clipboard::new().context("failed to open clipboard")?;
    clipboard
        .set_text(text)
        .context("failed to set clipboard text")?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();

    // Load config
    let config = config::TexpandConfig::load(args.config.as_deref())?;

    // Determine include paths: CLI overrides config
    let cli_include_paths: Vec<PathBuf> = if args.include.is_empty() {
        config.include_paths.iter().map(PathBuf::from).collect()
    } else {
        args.include.iter().map(PathBuf::from).collect()
    };

    // Determine compression: CLI flag > config > default (false)
    let compress = if args.compress {
        true
    } else if args.no_compress {
        false
    } else {
        config.default_compress
    };

    // Read entry file
    let entry_path = std::fs::canonicalize(&args.input)
        .with_context(|| format!("cannot access '{}'", args.input.display()))?;
    let entry_source = std::fs::read_to_string(&entry_path)
        .with_context(|| format!("cannot read '{}'", args.input.display()))?;

    // Build resolver: entry file's directory searched first, then include paths
    let mut resolver_paths = Vec::new();
    resolver_paths.push(entry_path.parent().unwrap_or(Path::new(".")).to_path_buf());
    resolver_paths.extend(cli_include_paths);
    let resolver = FsResolver::new(resolver_paths);

    // Expand
    let opts = ExpandOptions { compress };
    let result = expand(
        &entry_path.to_string_lossy(),
        &entry_source,
        &resolver,
        &opts,
    )?;

    // Output
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
