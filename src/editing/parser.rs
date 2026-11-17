use crate::editing::ops::{Operation, parse_filter};
use anyhow::{Result, anyhow};
use image::imageops::FilterType;

fn parse_csv_numbers<T: std::str::FromStr>(
    payload: &str,
    expected: usize,
    label: &str,
) -> Result<Vec<T>> {
    let parts = payload
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    if parts.len() != expected {
        return Err(anyhow!("{label} expects {expected} comma-separated values"));
    }
    parts
        .into_iter()
        .map(|raw| {
            raw.parse::<T>()
                .map_err(|_| anyhow!("invalid number: {raw}"))
        })
        .collect()
}

pub fn parse_operation(value: &str) -> Result<Operation> {
    let (name, payload) = match value.split_once(':') {
        Some((name, payload)) => (name.trim().to_ascii_lowercase(), Some(payload.trim())),
        None => (value.trim().to_ascii_lowercase(), None),
    };

    match name.as_str() {
        "grayscale" => Ok(Operation::Grayscale),
        "invert" => Ok(Operation::Invert),
        "rotate90" => Ok(Operation::Rotate90),
        "rotate180" => Ok(Operation::Rotate180),
        "rotate270" => Ok(Operation::Rotate270),
        "fliph" | "flip-horizontal" => Ok(Operation::FlipHorizontal),
        "flipv" | "flip-vertical" => Ok(Operation::FlipVertical),
        "autolevels" | "auto-levels" => Ok(Operation::AutoLevels),
        "brighten" => Ok(Operation::Brighten(
            payload
                .ok_or_else(|| anyhow!("missing brighten amount"))?
                .parse::<i32>()
                .map_err(|_| anyhow!("brighten amount must be an integer"))?,
        )),
        "contrast" => Ok(Operation::Contrast(
            payload
                .ok_or_else(|| anyhow!("missing contrast amount"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("contrast amount must be a number"))?,
        )),
        "blur" => {
            let sigma = payload
                .ok_or_else(|| anyhow!("missing blur sigma"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("blur sigma must be a number"))?;
            if sigma < 0.0 {
                return Err(anyhow!("blur sigma must be >= 0"));
            }
            Ok(Operation::Blur(sigma))
        }
        "huerotate" | "hue" => Ok(Operation::HueRotate(
            payload
                .ok_or_else(|| anyhow!("missing hue degrees"))?
                .parse::<i32>()
                .map_err(|_| anyhow!("hue degrees must be an integer"))?,
        )),
        "saturate" => Ok(Operation::Saturate(
            payload
                .ok_or_else(|| anyhow!("missing saturation amount"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("saturation amount must be a number"))?,
        )),
        "gamma" => {
            let gamma = payload
                .ok_or_else(|| anyhow!("missing gamma value"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("gamma must be a number"))?;
            if !gamma.is_finite() || gamma <= 0.0 {
                return Err(anyhow!("gamma must be > 0"));
            }
            Ok(Operation::Gamma(gamma))
        }
        "exposure" => {
            let ev = payload
                .ok_or_else(|| anyhow!("missing exposure value"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("exposure must be a number"))?;
            if !ev.is_finite() || !(-16.0..=16.0).contains(&ev) {
                return Err(anyhow!("exposure must be between -16 and 16 EV"));
            }
            Ok(Operation::Exposure(ev))
        }
        "vignette" => {
            let strength = payload
                .ok_or_else(|| anyhow!("missing vignette strength"))?
                .parse::<f32>()
                .map_err(|_| anyhow!("vignette strength must be a number"))?;
            if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
                return Err(anyhow!("vignette strength must be between 0 and 1"));
            }
            Ok(Operation::Vignette(strength))
        }
        "unsharpen" => {
            let nums = parse_csv_numbers::<f32>(
                payload.ok_or_else(|| anyhow!("missing unsharpen args"))?,
                2,
                "unsharpen",
            )?;
            Ok(Operation::Unsharpen {
                sigma: nums[0],
                threshold: nums[1] as i32,
            })
        }
        "crop" => {
            let nums = parse_csv_numbers::<u32>(
                payload.ok_or_else(|| anyhow!("missing crop args"))?,
                4,
                "crop",
            )?;
            if nums[2] == 0 || nums[3] == 0 {
                return Err(anyhow!("crop width and height must be > 0"));
            }
            Ok(Operation::Crop {
                x: nums[0],
                y: nums[1],
                width: nums[2],
                height: nums[3],
            })
        }
        "resize" => {
            let raw = payload.ok_or_else(|| anyhow!("missing resize args"))?;
            let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
            if !(parts.len() == 2 || parts.len() == 3) {
                return Err(anyhow!("resize expects width,height[,filter]"));
            }
            let width = parts[0]
                .parse::<u32>()
                .map_err(|_| anyhow!("resize width must be an unsigned integer"))?;
            let height = parts[1]
                .parse::<u32>()
                .map_err(|_| anyhow!("resize height must be an unsigned integer"))?;
            if width == 0 || height == 0 {
                return Err(anyhow!("resize width and height must be > 0"));
            }
            let filter = if parts.len() == 3 {
                parse_filter(parts[2])?
            } else {
                FilterType::Lanczos3
            };
            Ok(Operation::Resize {
                width,
                height,
                filter,
            })
        }
        _ => Err(anyhow!("unknown operation '{name}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_operation;
    use crate::editing::ops::Operation;
    use image::imageops::FilterType;

    #[test]
    fn parses_resize_with_default_filter() {
        let op = parse_operation("resize:1920,1080").expect("resize should parse");
        assert_eq!(
            op,
            Operation::Resize {
                width: 1920,
                height: 1080,
                filter: FilterType::Lanczos3,
            }
        );
    }

    #[test]
    fn parses_unsharpen() {
        let op = parse_operation("unsharpen:0.8,2").expect("unsharpen should parse");
        assert_eq!(
            op,
            Operation::Unsharpen {
                sigma: 0.8,
                threshold: 2,
            }
        );
    }

    #[test]
    fn rejects_bad_crop_shape() {
        let err = parse_operation("crop:10,20,30").expect_err("bad crop should fail");
        assert!(err.to_string().contains("crop expects 4"));
    }

    #[test]
    fn validates_exposure_and_vignette_ranges() {
        assert!(parse_operation("exposure:17").is_err());
        assert!(parse_operation("vignette:-0.1").is_err());
        assert_eq!(
            parse_operation("exposure:-1.5").expect("valid exposure"),
            Operation::Exposure(-1.5)
        );
    }
}
