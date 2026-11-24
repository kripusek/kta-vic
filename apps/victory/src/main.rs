//! `victory`: the command-line front end.
//!
//! Every command reads the user's own files and writes only to stdout or to
//! the user's cache directory (outside the repository).

#[cfg(all(windows, feature = "agility"))]
#[unsafe(no_mangle)]
pub static D3D12SDKVersion: u32 = 620;

#[cfg(all(windows, feature = "agility"))]
#[unsafe(no_mangle)]
pub static D3D12SDKPath: &[u8; 9] = b".\\D3D12\\\0";

mod census;
mod inspect;
mod run;
mod shader;
mod translate;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "victory",
    version,
    about = "VI Causeway runs an Xbox Series title natively on Windows"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Summarise one PE image: headers, sections, imports, TLS, build id.
    Inspect {
        file: PathBuf,
        /// List every imported function.
        #[arg(long)]
        imports: bool,
        /// List every exported function.
        #[arg(long)]
        exports: bool,
    },
    /// Aggregate the imports of every PE image under the given paths and
    /// show which ones our runtime covers. Defaults to the configured
    /// game and test-title directories.
    Census {
        paths: Vec<PathBuf>,
        /// Do not count host pass-through as covered.
        #[arg(long)]
        strict: bool,
        /// List every missing `dll!function`, most used first.
        #[arg(long)]
        missing: bool,
    },
    /// List the `dll!export` pairs an import library describes.
    LibExports {
        lib: PathBuf,
        /// List every export, not just per-DLL counts.
        #[arg(long)]
        list: bool,
    },
    /// Find shader containers in files or directories (also inside engine
    /// archives, if uncompressed) and report their layout.
    ShaderInfo {
        paths: Vec<PathBuf>,
        /// Print one line per container found.
        #[arg(long)]
        verbose: bool,
        /// Also list the decoded native instructions (implies --verbose).
        #[arg(long)]
        disasm: bool,
    },
    /// Run every shader found (containers, SGD5 files) through the
    /// translator and report what it cannot handle yet.
    Translate {
        paths: Vec<PathBuf>,
        /// Check emitted DXIL with dxil.dll's validator (`VICTORY_DXIL`,
        /// else the newest Windows SDK's).
        #[arg(long)]
        validate: bool,
        /// Also create a host D3D12 pipeline for each emitted shader.
        #[arg(long)]
        pipelines: bool,
        /// Print one line per program and outcome.
        #[arg(long)]
        list: bool,
        /// Worker threads (default: every logical core).
        #[arg(long)]
        jobs: Option<usize>,
    },
    /// Prepare, map and bind a title executable; with --execute, run it.
    Run {
        exe: PathBuf,
        /// What unimplemented imports do when called.
        #[arg(long, value_enum, default_value_t = Unimplemented::Trap)]
        unimplemented: Unimplemented,
        /// Disable host pass-through (everything must be implemented by us).
        #[arg(long)]
        strict: bool,
        /// Actually start the title. Without it, stop after binding.
        #[arg(long)]
        execute: bool,
        /// The console model the title sees (device type, memory).
        #[arg(long, value_enum, default_value_t = Console::SeriesX)]
        console: Console,
        /// Override the title's 30/60 Hz frame pacing (30 or 60 fps).
        /// Omit to keep the title's requested intervals.
        #[arg(long)]
        fps: Option<victory_hle::FrameRate>,
        /// Maximum height of the host presentation backbuffer (overrides VICTORY_OUTPUT_HEIGHT).
        #[arg(long)]
        output_height: Option<std::num::NonZeroU32>,
        /// Limit background pipeline compilation threads for this launch.
        #[arg(long)]
        max_pipeline_workers: Option<std::num::NonZeroU8>,
        /// Arguments for the title, after `--` (it reads the whole command
        /// line; a title that relaunches itself passes some).
        #[arg(last = true)]
        title_args: Vec<String>,
    },
    /// Show the resolved configuration and check it.
    Config,
}

#[derive(Clone, Copy, ValueEnum)]
enum Unimplemented {
    Trap,
    Zero,
}

#[derive(Clone, Copy, ValueEnum)]
enum Console {
    SeriesX,
    SeriesS,
}

fn main() -> Result<()> {
    victory_core::logging::init("info");
    // The audio recorder `run` starts for `VICTORY_AUDIO_CAPTURE`.
    #[cfg(all(windows, feature = "diagnostics"))]
    if victory_hle::modules::audio::capture::child() {
        return Ok(());
    }
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect {
            file,
            imports,
            exports,
        } => inspect::run(&file, imports, exports),
        Command::Census {
            paths,
            strict,
            missing,
        } => census::run(paths, strict, missing),
        Command::LibExports { lib, list } => inspect::lib_exports(&lib, list),
        Command::ShaderInfo {
            paths,
            verbose,
            disasm,
        } => shader::run(paths, verbose || disasm, disasm),
        Command::Translate {
            paths,
            validate,
            pipelines,
            list,
            jobs,
        } => translate::run(paths, validate, pipelines, list, jobs),
        Command::Run {
            exe,
            unimplemented,
            strict,
            execute,
            console,
            fps,
            output_height,
            max_pipeline_workers,
            title_args: _,
        } => run::run(
            &exe,
            unimplemented,
            strict,
            execute,
            console,
            fps,
            output_height,
            max_pipeline_workers,
        ),
        Command::Config => show_config(),
    }
}

fn show_config() -> Result<()> {
    let cfg = victory_core::Config::load()?;
    let status = |p: &Option<PathBuf>| match p {
        Some(p) if p.exists() => format!("{}", p.display()),
        Some(p) => format!("{}  (does not exist)", p.display()),
        None => "(not set)".to_owned(),
    };
    println!(
        "config file : {}",
        cfg.source
            .as_ref()
            .map_or("(none)".into(), |p| p.display().to_string())
    );
    println!("game_dir    : {}", status(&cfg.game_dir));
    println!("bcpack_tables: {}", status(&cfg.bcpack_tables));
    println!("cache_dir   : {}", cfg.cache_dir.display());
    for t in &cfg.test_titles {
        println!("test_title  : {}", status(&Some(t.clone())));
    }
    for p in cfg
        .game_dir
        .iter()
        .chain(&cfg.bcpack_tables)
        .chain(&cfg.test_titles)
        .chain([&cfg.cache_dir])
    {
        if let Err(e) = victory_core::ensure_outside_repo(p) {
            println!("POLICY VIOLATION: {e}");
        }
    }
    Ok(())
}

/// Paths given on the command line, or the configured game + test titles.
pub(crate) fn default_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    if !paths.is_empty() {
        return Ok(paths);
    }
    let cfg = victory_core::Config::load()?;
    let all: Vec<PathBuf> = cfg.game_dir.into_iter().chain(cfg.test_titles).collect();
    anyhow::ensure!(
        !all.is_empty(),
        "no paths given and none configured (see victory.example.toml)"
    );
    Ok(all)
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use victory_hle::FrameRate;

    #[test]
    fn run_frame_rate_is_optional_and_validated() {
        for (args, expected) in [
            (vec!["victory", "run", "game.exe"], None),
            (
                vec!["victory", "run", "game.exe", "--fps", "30"],
                Some(FrameRate::Fps30),
            ),
            (
                vec!["victory", "run", "game.exe", "--fps", "60"],
                Some(FrameRate::Fps60),
            ),
        ] {
            let Command::Run { fps, .. } = Cli::try_parse_from(args).unwrap().command else {
                panic!("expected run subcommand");
            };
            assert_eq!(fps, expected);
        }
        assert!(Cli::try_parse_from(["victory", "run", "game.exe", "--fps", "120"]).is_err());
    }

    #[test]
    fn run_output_height_is_optional_and_positive() {
        for (value, expected) in [
            (None, None),
            (Some("720"), Some(720)),
            (Some("1080"), Some(1080)),
        ] {
            let mut args = vec!["victory", "run", "game.exe"];
            if let Some(value) = value {
                args.extend(["--output-height", value]);
            }
            let Command::Run { output_height, .. } = Cli::try_parse_from(args).unwrap().command
            else {
                panic!("expected run subcommand");
            };
            assert_eq!(output_height.map(|height| height.get()), expected);
        }
        for invalid in ["0", "-1", "abc", "4294967296"] {
            assert!(
                Cli::try_parse_from(["victory", "run", "game.exe", "--output-height", invalid])
                    .is_err()
            );
        }
    }

    #[test]
    fn run_output_height_flag_overrides_environment() {
        use std::num::NonZeroU32;
        assert_eq!(
            run::resolve_output_height(None, Ok("1080".into())).unwrap(),
            NonZeroU32::new(1080)
        );
        assert_eq!(
            run::resolve_output_height(NonZeroU32::new(720), Ok("1080".into())).unwrap(),
            NonZeroU32::new(720)
        );
        assert_eq!(
            run::resolve_output_height(None, Err(std::env::VarError::NotPresent)).unwrap(),
            None
        );
        assert!(run::resolve_output_height(None, Ok("0".into())).is_err());
    }

    #[test]
    fn run_accepts_a_positive_pipeline_worker_limit_with_fps() {
        let args = [
            "victory", "run", "game.exe", "--max-pipeline-workers", "2", "--fps", "60",
        ];
        let Command::Run {
            max_pipeline_workers, fps, ..
        } = Cli::try_parse_from(args).unwrap().command
        else {
            panic!("expected run subcommand");
        };
        assert_eq!(max_pipeline_workers.map(|n| n.get()), Some(2));
        assert_eq!(fps, Some(FrameRate::Fps60));
        let zero = ["victory", "run", "game.exe", "--max-pipeline-workers", "0"];
        assert!(Cli::try_parse_from(zero).is_err());
    }
}
