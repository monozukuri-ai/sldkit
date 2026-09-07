use std::{fs::OpenOptions, io::Write, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use sldkit_core::{
    DrawingStructureStatus, ExtractionMode, ExtractionStatus, GeometryStatus, InventoryStatus,
    LimitProfile, ParseStatus, ProbeStatus, ProjectScanStatus, ResourceLimits,
};

#[derive(Debug, Parser)]
#[command(
    name = "sldkit-rs",
    version,
    about = "Inspect and decode supported SolidWorks source data"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Detect a candidate container without semantic parsing.
    Probe {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
    /// Walk the outer container and print its deterministic stream inventory.
    Inspect {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
    /// Extract one inventory entry to an explicitly selected output path.
    Extract {
        path: PathBuf,
        entry_id: String,
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = ModeArg::Decoded)]
        mode: ModeArg,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
        /// Permit replacing an existing output file.
        #[arg(long)]
        force: bool,
    },
    /// Return source-native facts, diagnostics, and semantic coverage.
    Parse {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
    /// Decode modern Part B-Rep topology, geometry carriers, and tessellation.
    Geometry {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
    /// Inventory modern Drawing source records without assigning renderable semantics.
    Drawing {
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
    /// Resolve a bounded `SolidWorks` document graph within explicit local roots.
    Scan {
        path: PathBuf,
        #[arg(long)]
        project_root: Option<PathBuf>,
        #[arg(long)]
        configuration: Option<String>,
        #[arg(long = "search-dir")]
        search_directories: Vec<PathBuf>,
        #[arg(long = "windows-prefix-map", value_name = "SOURCE=TARGET")]
        windows_prefix_mappings: Vec<String>,
        #[arg(long)]
        follow_suppressed: bool,
        /// Emit only the path-free compatibility aggregate.
        #[arg(long)]
        summary: bool,
        #[arg(long, value_enum, default_value_t = LimitArg::Desktop)]
        limits: LimitArg,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LimitArg {
    Desktop,
    Service,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ModeArg {
    Stored,
    Decoded,
}

impl ModeArg {
    const fn mode(self) -> ExtractionMode {
        match self {
            Self::Stored => ExtractionMode::Stored,
            Self::Decoded => ExtractionMode::Decoded,
        }
    }
}

impl LimitArg {
    const fn limits(self) -> ResourceLimits {
        match self {
            Self::Desktop => LimitProfile::Desktop.limits(),
            Self::Service => LimitProfile::Service.limits(),
        }
    }
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("sldkit-rs failed: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Args) -> Result<ExitCode, Box<dyn std::error::Error>> {
    match args.command {
        Command::Probe { path, limits } => {
            let result = sldkit_parser::probe_path(path, &limits.limits());
            write_json(&result)?;
            Ok(match result.status {
                ProbeStatus::Recognized => ExitCode::SUCCESS,
                ProbeStatus::Unrecognized => ExitCode::from(1),
                ProbeStatus::Malformed | ProbeStatus::Rejected => ExitCode::from(2),
            })
        }
        Command::Inspect { path, limits } => {
            let result = sldkit_parser::inspect_path(path, &limits.limits());
            write_json(&result)?;
            Ok(match result.status {
                InventoryStatus::Complete | InventoryStatus::Partial => ExitCode::SUCCESS,
                InventoryStatus::Unsupported => ExitCode::from(1),
                InventoryStatus::Malformed | InventoryStatus::Rejected => ExitCode::from(2),
            })
        }
        Command::Extract {
            path,
            entry_id,
            output,
            mode,
            limits,
            force,
        } => {
            let extraction =
                sldkit_parser::extract_path(path, &entry_id, mode.mode(), &limits.limits());
            write_json(&extraction.result)?;
            if extraction.result.status == ExtractionStatus::Extracted {
                if let Some(data) = extraction.data {
                    let mut options = OpenOptions::new();
                    options.write(true);
                    if force {
                        options.create(true).truncate(true);
                    } else {
                        options.create_new(true);
                    }
                    options.open(output)?.write_all(&data)?;
                }
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(match extraction.result.status {
                    ExtractionStatus::NotFound | ExtractionStatus::Unavailable => ExitCode::from(1),
                    ExtractionStatus::Malformed | ExtractionStatus::Rejected => ExitCode::from(2),
                    ExtractionStatus::Extracted => ExitCode::SUCCESS,
                })
            }
        }
        Command::Parse { path, limits } => {
            let result = sldkit_parser::parse_path(path, &limits.limits());
            write_json(&result)?;
            Ok(match result.status {
                ParseStatus::Parsed | ParseStatus::Partial => ExitCode::SUCCESS,
                ParseStatus::Unsupported => ExitCode::from(1),
                ParseStatus::Malformed | ParseStatus::Rejected => ExitCode::from(2),
            })
        }
        Command::Geometry { path, limits } => run_geometry(path, limits),
        Command::Drawing { path, limits } => run_drawing(path, limits),
        Command::Scan {
            path,
            project_root,
            configuration,
            search_directories,
            windows_prefix_mappings,
            follow_suppressed,
            summary,
            limits,
        } => {
            let mappings = windows_prefix_mappings
                .iter()
                .map(|value| parse_windows_prefix_mapping(value))
                .collect::<Result<Vec<_>, _>>()?;
            let options = sldkit_parser::ProjectScanOptions {
                project_root,
                root_configuration: configuration,
                search_directories,
                windows_prefix_mappings: mappings,
                follow_suppressed,
            };
            let result = sldkit_parser::scan_project_path(path, &options, &limits.limits());
            if summary {
                write_json(&result.compatibility_report)?;
            } else {
                write_json(&result)?;
            }
            Ok(match result.status {
                ProjectScanStatus::Complete => ExitCode::SUCCESS,
                ProjectScanStatus::Partial => ExitCode::from(1),
                ProjectScanStatus::Rejected => ExitCode::from(2),
            })
        }
    }
}

fn run_drawing(path: PathBuf, limits: LimitArg) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let result = sldkit_parser::decode_drawing_structure_path(path, &limits.limits());
    write_json(&result)?;
    Ok(match result.status {
        DrawingStructureStatus::Inventoried | DrawingStructureStatus::Partial => ExitCode::SUCCESS,
        DrawingStructureStatus::Unsupported => ExitCode::from(1),
        DrawingStructureStatus::Malformed | DrawingStructureStatus::Rejected => ExitCode::from(2),
    })
}

fn run_geometry(path: PathBuf, limits: LimitArg) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let result = sldkit_parser::decode_geometry_path(path, &limits.limits());
    write_json(&result)?;
    Ok(match result.status {
        GeometryStatus::Decoded | GeometryStatus::Partial => ExitCode::SUCCESS,
        GeometryStatus::Unsupported => ExitCode::from(1),
        GeometryStatus::Malformed | GeometryStatus::Rejected => ExitCode::from(2),
    })
}

fn parse_windows_prefix_mapping(
    value: &str,
) -> Result<sldkit_parser::WindowsPrefixMapping, String> {
    let Some((source_prefix, target_directory)) = value.split_once('=') else {
        return Err(format!(
            "invalid Windows prefix mapping {value:?}; expected SOURCE=TARGET"
        ));
    };
    if source_prefix.is_empty() || target_directory.is_empty() {
        return Err(format!(
            "invalid Windows prefix mapping {value:?}; SOURCE and TARGET must be non-empty"
        ));
    }
    Ok(sldkit_parser::WindowsPrefixMapping {
        source_prefix: source_prefix.to_owned(),
        target_directory: PathBuf::from(target_directory),
    })
}

fn write_json(value: &impl Serialize) -> Result<(), serde_json::Error> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, value)?;
    writeln!(output).map_err(serde_json::Error::io)
}
