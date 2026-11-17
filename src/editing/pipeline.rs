use crate::editing::ai::{AiPreset, ai_operations, recommend_copilot_pipeline};
use crate::editing::io::load_image_with_psd_and_raw_support;
use crate::editing::ops::{Operation, apply_operation};
use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use std::path::Path;

#[cfg(feature = "gpu")]
fn apply_operations(
    mut image: DynamicImage,
    operations: &[Operation],
    gpu: Option<&super::gpu::GpuProcessor>,
    accelerated: &mut usize,
) -> Result<DynamicImage> {
    for operation in operations {
        if let Some(processor) = gpu.filter(|_| super::gpu::GpuProcessor::supports(operation)) {
            image = processor.apply(&image, operation)?;
            *accelerated += 1;
        } else {
            image = apply_operation(image, operation)?;
        }
    }
    Ok(image)
}

#[cfg(not(feature = "gpu"))]
fn apply_operations(
    mut image: DynamicImage,
    operations: &[Operation],
    _gpu: Option<&()>,
    _accelerated: &mut usize,
) -> Result<DynamicImage> {
    for operation in operations {
        image = apply_operation(image, operation)?;
    }
    Ok(image)
}

pub fn apply_pipeline(
    input: &Path,
    output: &Path,
    ops: &[Operation],
    ai: Option<AiPreset>,
    copilot_suggest: bool,
    use_gpu: bool,
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
    #[cfg(feature = "gpu")]
    let gpu = if use_gpu {
        let processor = super::gpu::GpuProcessor::new()?;
        eprintln!("Using GPU adapter: {}", processor.adapter_name());
        Some(processor)
    } else {
        None
    };
    #[cfg(not(feature = "gpu"))]
    let gpu = {
        if use_gpu {
            return Err(anyhow!(
                "GPU acceleration is not compiled in; rebuild with the `gpu` feature"
            ));
        }
        None::<()>
    };

    let mut effective = Vec::new();
    let mut accelerated = 0;

    if copilot_suggest {
        let rec = recommend_copilot_pipeline(&img);
        img = apply_operations(img, &rec, gpu.as_ref(), &mut accelerated)?;
        effective.extend(rec);
    }

    if let Some(preset) = ai {
        let preset_ops = ai_operations(preset, Some(&img));
        img = apply_operations(img, &preset_ops, gpu.as_ref(), &mut accelerated)?;
        effective.extend(preset_ops);
    }

    img = apply_operations(img, ops, gpu.as_ref(), &mut accelerated)?;
    effective.extend_from_slice(ops);

    img.save(output)
        .with_context(|| format!("failed to save {}", output.display()))?;
    if use_gpu {
        eprintln!("GPU accelerated {accelerated} operation(s); unsupported operations ran on CPU.");
    }
    Ok(effective)
}
