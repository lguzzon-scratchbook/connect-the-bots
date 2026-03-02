//! CLI binary for running and validating Attractor pipelines.

mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use commands::{cmd_decompose, cmd_info, cmd_plan, cmd_run, cmd_scaffold, cmd_validate, validate_decomposition};

#[derive(Parser)]
#[command(name = "attractor", version, about = "DOT-based pipeline runner for AI workflows")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a pipeline from a .dot file
    Run {
        /// Path to the pipeline .dot file
        pipeline: PathBuf,

        /// Working directory for tool execution
        #[arg(short, long)]
        workdir: Option<PathBuf>,

        /// Logs output directory (default: .attractor/logs/<pipeline>-<hash>)
        #[arg(short, long)]
        logs: Option<PathBuf>,

        /// Don't actually call LLMs (dry run)
        #[arg(long)]
        dry_run: bool,

        /// Maximum total spend across all nodes (USD). Pipeline aborts if exceeded.
        #[arg(long)]
        max_budget_usd: Option<f64>,

        /// Maximum number of node executions before aborting. Prevents runaway loops. Default: 200.
        #[arg(long, default_value = "200")]
        max_steps: u64,
    },

    /// Validate a pipeline .dot file
    Validate {
        /// Path to the pipeline .dot file
        pipeline: PathBuf,
    },

    /// Show information about a pipeline
    Info {
        /// Path to the pipeline .dot file
        pipeline: PathBuf,
    },

    /// Generate PRD or spec documents from templates
    Plan {
        /// Generate a PRD document
        #[arg(long, conflicts_with = "spec")]
        prd: bool,

        /// Generate a spec document
        #[arg(long, conflicts_with = "prd")]
        spec: bool,

        /// Generate from a prompt description (uses Claude CLI)
        #[arg(long)]
        from_prompt: Option<String>,

        /// Output file path (defaults: .attractor/prd.md or .attractor/spec.md)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Decompose a spec into beads epic and tasks
    Decompose {
        /// Path to the spec markdown file
        spec_path: PathBuf,

        /// Print the generated shell commands without executing them
        #[arg(long, conflicts_with = "validate")]
        dry_run: bool,

        /// Validate existing tickets against spec (skip LLM, just check coverage)
        #[arg(long, conflicts_with = "dry_run")]
        validate: Option<String>,
    },

    /// Scaffold a pipeline from a beads epic
    Scaffold {
        /// Beads epic ID (e.g., beads-xxx)
        epic_id: String,

        /// Output file path (default: pipelines/<epic-id>.dot)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Setup tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    match cli.command {
        Commands::Run {
            pipeline,
            workdir,
            logs,
            dry_run,
            max_budget_usd,
            max_steps,
        } => {
            cmd_run(&pipeline, workdir.as_deref(), logs.as_deref(), dry_run, max_budget_usd, max_steps).await?;
        }
        Commands::Validate { pipeline } => {
            cmd_validate(&pipeline)?;
        }
        Commands::Info { pipeline } => {
            cmd_info(&pipeline)?;
        }
        Commands::Plan { prd, spec, from_prompt, output } => {
            cmd_plan(prd, spec, from_prompt.as_deref(), output.as_deref()).await?;
        }
        Commands::Decompose { spec_path, dry_run, validate } => {
            if let Some(epic_id) = validate {
                let spec_content = std::fs::read_to_string(&spec_path)?;
                validate_decomposition(&spec_content, Some(&epic_id)).await?;
            } else {
                cmd_decompose(&spec_path, dry_run).await?;
            }
        }
        Commands::Scaffold { epic_id, output } => {
            cmd_scaffold(&epic_id, output.as_deref()).await?;
        }
    }

    Ok(())
}

pub(crate) fn load_pipeline(path: &std::path::Path) -> anyhow::Result<attractor_pipeline::PipelineGraph> {
    let source = std::fs::read_to_string(path)?;
    let dot = attractor_dot::parse(&source)?;
    let graph = attractor_pipeline::PipelineGraph::from_dot(dot)?;
    Ok(graph)
}
