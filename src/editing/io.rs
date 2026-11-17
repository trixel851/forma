use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use std::path::Path;
use std::process::Command;

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn decode_with_ffmpeg(path: &Path) -> Result<DynamicImage> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            path.to_string_lossy().as_ref(),
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        .output()
        .context("failed to execute ffmpeg for fallback decode")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("failed to decode file via ffmpeg: {stderr}"));
    }

    image::load_from_memory(&output.stdout).context("ffmpeg produced unreadable image bytes")
}

pub fn load_image_with_psd_and_raw_support(path: &Path) -> Result<DynamicImage> {
    if !path.exists() {
        return Err(anyhow!("input not found: {}", path.display()));
    }

    let ext = extension(path);
    if ext == "psd" {
        return decode_with_ffmpeg(path).with_context(|| {
            format!(
                "failed to read Photoshop file {} (ensure ffmpeg has PSD support)",
                path.display()
            )
        });
    }

    match image::open(path) {
        Ok(img) => Ok(img),
        Err(_) => decode_with_ffmpeg(path).with_context(|| {
            format!(
                "standard decode failed for {}; ffmpeg fallback also failed",
                path.display()
            )
        }),
    }
}

pub fn psd_info(path: &Path) -> Result<String> {
    if extension(path) != "psd" {
        return Err(anyhow!("{} is not a .psd file", path.display()));
    }
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,pix_fmt",
            "-of",
            "default=noprint_wrappers=1",
            path.to_string_lossy().as_ref(),
        ])
        .output()
        .context("failed to run ffprobe for PSD metadata")?;
    if !probe.status.success() {
        let stderr = String::from_utf8_lossy(&probe.stderr);
        return Err(anyhow!("ffprobe failed for PSD: {stderr}"));
    }
    Ok(format!(
        "PSD metadata for {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&probe.stdout)
    ))
}
