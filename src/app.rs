use crate::args::{Commands, PluginCommands};
use crate::editing::{
    AiPreset, apply_pipeline_with_plugins, list_operations, list_plugins, parse_operation, psd_info,
};
use crate::integrations::gui::run_gui;
use crate::integrations::mcp::run_mcp_server;
use crate::integrations::video::{
    VideoEditOptions, run_ffmpeg_extract_frame, run_ffmpeg_filter, run_video_edit,
};
use anyhow::Result;

pub fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Cli {
            input,
            output,
            ops,
            ai,
            copilot_suggest,
            gpu,
            plugins,
            plugin_dirs,
        } => {
            let parsed_ops = ops
                .iter()
                .map(|op| parse_operation(op))
                .collect::<Result<Vec<_>>>()?;
            let ai_preset = ai.as_deref().map(AiPreset::parse).transpose()?;
            let effective = apply_pipeline_with_plugins(
                &input,
                &output,
                &parsed_ops,
                ai_preset,
                copilot_suggest,
                gpu,
                &plugins,
                &plugin_dirs,
            )?;
            println!(
                "Saved {} with {} operation(s): {}",
                output.display(),
                effective.len() + plugins.len(),
                effective
                    .iter()
                    .map(ToString::to_string)
                    .chain(plugins.iter().map(|id| format!("plugin:{id}")))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            Ok(())
        }
        Commands::Video {
            input,
            output,
            trim_start,
            trim_end,
            resize,
            fps,
            speed,
            mute,
            overlay_image,
            overlay_pos,
            color_grade,
            codec,
            crf,
            preset,
        } => run_video_edit(VideoEditOptions {
            input,
            output,
            trim_start,
            trim_end,
            resize,
            fps,
            speed,
            mute,
            overlay_image,
            overlay_pos,
            color_grade,
            codec,
            crf,
            preset,
        }),
        Commands::Gui { input } => run_gui(input),
        Commands::Mcp => run_mcp_server(),
        Commands::FfmpegExtractFrame {
            input_video,
            output_image,
            timestamp,
        } => run_ffmpeg_extract_frame(input_video, output_image, timestamp),
        Commands::FfmpegFilter {
            input_video,
            output_video,
            vf,
        } => run_ffmpeg_filter(input_video, output_video, vf),
        Commands::Ops => {
            println!("{}", list_operations());
            Ok(())
        }
        Commands::PsdInfo { input } => {
            let info = psd_info(&input)?;
            println!("{info}");
            Ok(())
        }
        Commands::Plugins { command } => match command {
            PluginCommands::List { plugin_dirs } => {
                let plugins = list_plugins(&plugin_dirs)?;
                if plugins.is_empty() {
                    println!("No JavaScript plugins found.");
                } else {
                    for plugin in plugins {
                        println!(
                            "{}\t{}\t{}\t{}",
                            plugin.id,
                            plugin.name,
                            plugin.version,
                            plugin.path.display()
                        );
                    }
                }
                Ok(())
            }
        },
    }
}
