use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "forma",
    version,
    about = "Freedom-first photo/video editor with CLI, GUI, MCP, PSD/RAW support, and ffmpeg workflows"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Headless photo editing pipeline
    Cli {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long = "op")]
        ops: Vec<String>,
        #[arg(long)]
        ai: Option<String>,
        #[arg(long)]
        copilot_suggest: bool,
        /// Run supported pixel adjustments on a GPU (requires a compatible adapter)
        #[arg(long)]
        gpu: bool,
    },
    /// Advanced video editing pipeline powered by ffmpeg
    Video {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        trim_start: Option<String>,
        #[arg(long)]
        trim_end: Option<String>,
        #[arg(long)]
        resize: Option<String>,
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long)]
        speed: Option<f32>,
        #[arg(long)]
        mute: bool,
        #[arg(long)]
        overlay_image: Option<PathBuf>,
        #[arg(long, default_value = "20:20")]
        overlay_pos: String,
        #[arg(long)]
        color_grade: Option<String>,
        #[arg(long, default_value = "libx264")]
        codec: String,
        #[arg(long, default_value_t = 20)]
        crf: u8,
        #[arg(long, default_value = "medium")]
        preset: String,
    },
    /// Native desktop editor (compile with --features gui)
    Gui {
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// MCP-compatible JSON-RPC server over stdio
    Mcp,
    /// Extract a frame from video
    FfmpegExtractFrame {
        #[arg(long)]
        input_video: PathBuf,
        #[arg(long)]
        output_image: PathBuf,
        #[arg(long, default_value = "00:00:01")]
        timestamp: String,
    },
    /// Apply a raw ffmpeg filter graph
    FfmpegFilter {
        #[arg(long)]
        input_video: PathBuf,
        #[arg(long)]
        output_video: PathBuf,
        #[arg(long)]
        vf: String,
    },
    /// Show available image operations
    Ops,
    /// Inspect PSD metadata support
    PsdInfo {
        #[arg(long)]
        input: PathBuf,
    },
}
