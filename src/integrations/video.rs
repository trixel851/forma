use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct VideoEditOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub trim_start: Option<String>,
    pub trim_end: Option<String>,
    pub resize: Option<String>,
    pub fps: Option<u32>,
    pub speed: Option<f32>,
    pub mute: bool,
    pub overlay_image: Option<PathBuf>,
    pub overlay_pos: String,
    pub color_grade: Option<String>,
    pub codec: String,
    pub crf: u8,
    pub preset: String,
}

fn ensure_ffmpeg(bin: &str) -> Result<()> {
    let status = Command::new(bin)
        .arg("-version")
        .output()
        .with_context(|| format!("{bin} is not installed or not available on PATH"))?;
    if !status.status.success() {
        return Err(anyhow!("{bin} exists but returned a failure status"));
    }
    Ok(())
}

fn color_grade_filter(grade: &str) -> Result<&'static str> {
    match grade.trim().to_ascii_lowercase().as_str() {
        "cinematic" => Ok("eq=contrast=1.14:saturation=1.12:brightness=-0.01"),
        "vivid" => Ok("eq=contrast=1.10:saturation=1.25"),
        "flat" | "neutral" => Ok("eq=contrast=1.0:saturation=1.0"),
        other => Err(anyhow!(
            "unknown color grade '{other}'. Use cinematic, vivid, neutral"
        )),
    }
}

fn parse_resize(resize: &str) -> Result<(u32, u32)> {
    let mut parts = resize.split(':');
    let w = parts
        .next()
        .ok_or_else(|| anyhow!("resize must be WIDTH:HEIGHT"))?
        .parse::<u32>()
        .map_err(|_| anyhow!("resize width must be a positive integer"))?;
    let h = parts
        .next()
        .ok_or_else(|| anyhow!("resize must be WIDTH:HEIGHT"))?
        .parse::<u32>()
        .map_err(|_| anyhow!("resize height must be a positive integer"))?;
    if parts.next().is_some() {
        return Err(anyhow!("resize must be WIDTH:HEIGHT"));
    }
    if w == 0 || h == 0 {
        return Err(anyhow!("resize width/height must be > 0"));
    }
    Ok((w, h))
}

pub fn run_video_edit(opts: VideoEditOptions) -> Result<()> {
    ensure_ffmpeg("ffmpeg")?;

    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
    ];

    if let Some(start) = &opts.trim_start {
        args.extend(["-ss".into(), start.clone()]);
    }
    args.extend(["-i".into(), opts.input.to_string_lossy().to_string()]);

    let has_overlay = opts.overlay_image.is_some();
    if let Some(overlay) = &opts.overlay_image {
        args.extend(["-i".into(), overlay.to_string_lossy().to_string()]);
    }

    if let Some(end) = &opts.trim_end {
        args.extend(["-to".into(), end.clone()]);
    }

    let mut vf_parts = Vec::new();

    if let Some(resize) = &opts.resize {
        let (w, h) = parse_resize(resize)?;
        vf_parts.push(format!("scale={w}:{h}"));
    }
    if let Some(fps) = opts.fps {
        vf_parts.push(format!("fps={fps}"));
    }
    if let Some(speed) = opts.speed {
        if speed <= 0.0 {
            return Err(anyhow!("speed must be > 0"));
        }
        vf_parts.push(format!("setpts=PTS/{speed}"));
        if !opts.mute {
            args.extend(["-af".into(), format!("atempo={speed}")]);
        }
    }
    if let Some(grade) = &opts.color_grade {
        vf_parts.push(color_grade_filter(grade)?.to_string());
    }
    if has_overlay {
        vf_parts.push(format!("overlay={}", opts.overlay_pos));
    }

    if !vf_parts.is_empty() {
        args.extend(["-vf".into(), vf_parts.join(",")]);
    }
    if opts.mute {
        args.push("-an".into());
    }

    args.extend([
        "-c:v".into(),
        opts.codec,
        "-preset".into(),
        opts.preset,
        "-crf".into(),
        opts.crf.to_string(),
        opts.output.to_string_lossy().to_string(),
    ]);

    let status = Command::new("ffmpeg")
        .args(args)
        .status()
        .context("failed to run ffmpeg video edit command")?;

    if !status.success() {
        return Err(anyhow!("ffmpeg failed to process video"));
    }

    println!("Saved edited video to {}", opts.output.display());
    Ok(())
}

pub fn run_ffmpeg_extract_frame(
    input_video: PathBuf,
    output_image: PathBuf,
    timestamp: String,
) -> Result<()> {
    ensure_ffmpeg("ffmpeg")?;
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &timestamp,
            "-i",
            input_video.to_string_lossy().as_ref(),
            "-frames:v",
            "1",
            output_image.to_string_lossy().as_ref(),
        ])
        .status()
        .context("failed to run ffmpeg extract frame command")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg failed to extract frame"));
    }
    println!(
        "Extracted frame at {timestamp} -> {}",
        output_image.display()
    );
    Ok(())
}

pub fn run_ffmpeg_filter(input_video: PathBuf, output_video: PathBuf, vf: String) -> Result<()> {
    ensure_ffmpeg("ffmpeg")?;
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            input_video.to_string_lossy().as_ref(),
            "-vf",
            &vf,
            output_video.to_string_lossy().as_ref(),
        ])
        .status()
        .context("failed to run ffmpeg filter command")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg failed to process video"));
    }
    println!("Saved ffmpeg output to {}", output_video.display());
    Ok(())
}
