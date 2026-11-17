use crate::editing::ops::Operation;
use anyhow::{Result, anyhow};
use image::DynamicImage;

#[derive(Debug, Clone, Copy)]
pub enum AiPreset {
    Enhance,
    Denoise,
    Dramatic,
    CopilotSmart,
}

impl AiPreset {
    pub fn parse(input: &str) -> Result<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "enhance" => Ok(Self::Enhance),
            "denoise" => Ok(Self::Denoise),
            "dramatic" => Ok(Self::Dramatic),
            "copilot-smart" | "copilot" => Ok(Self::CopilotSmart),
            other => Err(anyhow!(
                "unknown AI preset '{other}'. Use enhance, denoise, dramatic, copilot-smart"
            )),
        }
    }
}

pub fn ai_operations(preset: AiPreset, source: Option<&DynamicImage>) -> Vec<Operation> {
    match preset {
        AiPreset::Enhance => vec![
            Operation::AutoLevels,
            Operation::Contrast(8.0),
            Operation::Saturate(1.10),
            Operation::Unsharpen {
                sigma: 0.6,
                threshold: 1,
            },
        ],
        AiPreset::Denoise => vec![
            Operation::Blur(0.7),
            Operation::Unsharpen {
                sigma: 0.7,
                threshold: 1,
            },
            Operation::Contrast(4.0),
        ],
        AiPreset::Dramatic => vec![
            Operation::Contrast(15.0),
            Operation::Saturate(1.20),
            Operation::Gamma(0.95),
            Operation::Unsharpen {
                sigma: 0.8,
                threshold: 1,
            },
        ],
        AiPreset::CopilotSmart => source
            .map(recommend_copilot_pipeline)
            .unwrap_or_else(|| vec![Operation::AutoLevels, Operation::Contrast(6.0)]),
    }
}

pub fn recommend_copilot_pipeline(img: &DynamicImage) -> Vec<Operation> {
    let thumb = img.thumbnail(256, 256).to_rgb8();
    let mut mean_luma = 0.0f32;
    let mut min_luma = 255.0f32;
    let mut max_luma = 0.0f32;
    let mut count = 0f32;
    for px in thumb.pixels() {
        let [r, g, b] = px.0;
        let l = (0.2126 * r as f32) + (0.7152 * g as f32) + (0.0722 * b as f32);
        mean_luma += l;
        min_luma = min_luma.min(l);
        max_luma = max_luma.max(l);
        count += 1.0;
    }
    mean_luma /= count.max(1.0);
    let dynamic_range = max_luma - min_luma;

    let mut ops = vec![Operation::AutoLevels];
    if mean_luma < 90.0 {
        ops.push(Operation::Brighten(12));
    } else if mean_luma > 175.0 {
        ops.push(Operation::Brighten(-8));
    }
    if dynamic_range < 95.0 {
        ops.push(Operation::Contrast(10.0));
    } else {
        ops.push(Operation::Contrast(5.0));
    }
    ops.push(Operation::Saturate(1.08));
    ops.push(Operation::Unsharpen {
        sigma: 0.6,
        threshold: 1,
    });
    ops
}

#[cfg(test)]
mod tests {
    use super::AiPreset;

    #[test]
    fn parses_ai_preset() {
        let preset = AiPreset::parse("copilot-smart").expect("preset should parse");
        assert!(matches!(preset, AiPreset::CopilotSmart));
    }
}
