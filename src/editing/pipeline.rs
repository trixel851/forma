use crate::editing::ai::{AiPreset, ai_operations, recommend_copilot_pipeline};
use crate::editing::io::load_image_with_psd_and_raw_support;
use crate::editing::ops::{Operation, apply_operation};
use anyhow::{Context, Result, anyhow};
use std::path::Path;

pub fn apply_pipeline(
    input: &Path,
    output: &Path,
    ops: &[Operation],
    ai: Option<AiPreset>,
    copilot_suggest: bool,
) -> Result<Vec<Operation>> {
    if output
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("psd"))
    {
        return Err(anyhow!(
            "PSD export is not implemented yet; output a non-PSD format (png/jpg/tiff/webp/avif)"
        ));
    }

    let mut img = load_image_with_psd_and_raw_support(input)?;
    let mut effective = Vec::new();

    if copilot_suggest {
        let rec = recommend_copilot_pipeline(&img);
        for op in &rec {
            img = apply_operation(img, op)?;
        }
        effective.extend(rec);
    }

    if let Some(preset) = ai {
        let preset_ops = ai_operations(preset, Some(&img));
        for op in &preset_ops {
            img = apply_operation(img, op)?;
        }
        effective.extend(preset_ops);
    }

    for op in ops {
        img = apply_operation(img, op)?;
    }
    effective.extend_from_slice(ops);

    img.save(output)
        .with_context(|| format!("failed to save {}", output.display()))?;
    Ok(effective)
}
