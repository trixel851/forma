use anyhow::{Result, anyhow};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Grayscale,
    Invert,
    Brighten(i32),
    Contrast(f32),
    Blur(f32),
    HueRotate(i32),
    Saturate(f32),
    Gamma(f32),
    AutoLevels,
    Unsharpen {
        sigma: f32,
        threshold: i32,
    },
    Rotate90,
    Rotate180,
    Rotate270,
    FlipHorizontal,
    FlipVertical,
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Resize {
        width: u32,
        height: u32,
        filter: FilterType,
    },
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Grayscale => write!(f, "grayscale"),
            Operation::Invert => write!(f, "invert"),
            Operation::Brighten(v) => write!(f, "brighten:{v}"),
            Operation::Contrast(v) => write!(f, "contrast:{v}"),
            Operation::Blur(v) => write!(f, "blur:{v}"),
            Operation::HueRotate(v) => write!(f, "huerotate:{v}"),
            Operation::Saturate(v) => write!(f, "saturate:{v}"),
            Operation::Gamma(v) => write!(f, "gamma:{v}"),
            Operation::AutoLevels => write!(f, "autolevels"),
            Operation::Unsharpen { sigma, threshold } => write!(f, "unsharpen:{sigma},{threshold}"),
            Operation::Rotate90 => write!(f, "rotate90"),
            Operation::Rotate180 => write!(f, "rotate180"),
            Operation::Rotate270 => write!(f, "rotate270"),
            Operation::FlipHorizontal => write!(f, "flip-horizontal"),
            Operation::FlipVertical => write!(f, "flip-vertical"),
            Operation::Crop {
                x,
                y,
                width,
                height,
            } => write!(f, "crop:{x},{y},{width},{height}"),
            Operation::Resize {
                width,
                height,
                filter,
            } => write!(f, "resize:{width},{height},{}", filter_name(*filter)),
        }
    }
}

pub fn filter_name(filter: FilterType) -> &'static str {
    match filter {
        FilterType::Nearest => "nearest",
        FilterType::Triangle => "triangle",
        FilterType::CatmullRom => "catmullrom",
        FilterType::Gaussian => "gaussian",
        FilterType::Lanczos3 => "lanczos3",
    }
}

pub fn parse_filter(name: &str) -> Result<FilterType> {
    match name.trim().to_ascii_lowercase().as_str() {
        "nearest" => Ok(FilterType::Nearest),
        "triangle" => Ok(FilterType::Triangle),
        "catmullrom" => Ok(FilterType::CatmullRom),
        "gaussian" => Ok(FilterType::Gaussian),
        "lanczos3" => Ok(FilterType::Lanczos3),
        _ => Err(anyhow!(
            "unknown resize filter '{name}'. Use: nearest, triangle, catmullrom, gaussian, lanczos3"
        )),
    }
}

fn apply_saturation(img: DynamicImage, factor: f32) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;
        let luminance = (0.2126 * rf) + (0.7152 * gf) + (0.0722 * bf);
        let nr = (luminance + (rf - luminance) * factor).clamp(0.0, 255.0) as u8;
        let ng = (luminance + (gf - luminance) * factor).clamp(0.0, 255.0) as u8;
        let nb = (luminance + (bf - luminance) * factor).clamp(0.0, 255.0) as u8;
        pixel.0 = [nr, ng, nb, a];
    }
    DynamicImage::ImageRgba8(rgba)
}

fn apply_gamma(img: DynamicImage, gamma: f32) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    let inv = 1.0 / gamma;
    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let map = |c: u8| -> u8 {
            (((c as f32 / 255.0).powf(inv)) * 255.0)
                .clamp(0.0, 255.0)
                .round() as u8
        };
        pixel.0 = [map(r), map(g), map(b), a];
    }
    DynamicImage::ImageRgba8(rgba)
}

fn apply_autolevels(img: DynamicImage) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    let (mut min_r, mut min_g, mut min_b) = (u8::MAX, u8::MAX, u8::MAX);
    let (mut max_r, mut max_g, mut max_b) = (u8::MIN, u8::MIN, u8::MIN);
    for pixel in rgba.pixels() {
        let [r, g, b, _] = pixel.0;
        min_r = min_r.min(r);
        min_g = min_g.min(g);
        min_b = min_b.min(b);
        max_r = max_r.max(r);
        max_g = max_g.max(g);
        max_b = max_b.max(b);
    }
    let stretch = |v: u8, min: u8, max: u8| -> u8 {
        if min >= max {
            v
        } else {
            (((v.saturating_sub(min)) as f32 / (max - min) as f32) * 255.0)
                .clamp(0.0, 255.0)
                .round() as u8
        }
    };
    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        pixel.0 = [
            stretch(r, min_r, max_r),
            stretch(g, min_g, max_g),
            stretch(b, min_b, max_b),
            a,
        ];
    }
    DynamicImage::ImageRgba8(rgba)
}

pub fn apply_operation(img: DynamicImage, operation: &Operation) -> Result<DynamicImage> {
    let next = match operation {
        Operation::Grayscale => img.grayscale(),
        Operation::Invert => {
            let mut working = img;
            working.invert();
            working
        }
        Operation::Brighten(value) => img.brighten(*value),
        Operation::Contrast(value) => img.adjust_contrast(*value),
        Operation::Blur(sigma) => img.blur(*sigma),
        Operation::HueRotate(degrees) => img.huerotate(*degrees),
        Operation::Saturate(factor) => apply_saturation(img, *factor),
        Operation::Gamma(value) => apply_gamma(img, *value),
        Operation::AutoLevels => apply_autolevels(img),
        Operation::Unsharpen { sigma, threshold } => img.unsharpen(*sigma, *threshold),
        Operation::Rotate90 => img.rotate90(),
        Operation::Rotate180 => img.rotate180(),
        Operation::Rotate270 => img.rotate270(),
        Operation::FlipHorizontal => img.fliph(),
        Operation::FlipVertical => img.flipv(),
        Operation::Crop {
            x,
            y,
            width,
            height,
        } => {
            let (img_w, img_h) = img.dimensions();
            if *x >= img_w || *y >= img_h {
                return Err(anyhow!(
                    "crop origin ({x},{y}) is outside image bounds ({img_w}x{img_h})"
                ));
            }
            let max_w = img_w - *x;
            let max_h = img_h - *y;
            if *width > max_w || *height > max_h {
                return Err(anyhow!(
                    "crop rectangle ({width}x{height}) from ({x},{y}) exceeds bounds ({img_w}x{img_h})"
                ));
            }
            img.crop_imm(*x, *y, *width, *height)
        }
        Operation::Resize {
            width,
            height,
            filter,
        } => img.resize_exact(*width, *height, *filter),
    };
    Ok(next)
}

pub fn list_operations() -> &'static str {
    "Operations:
  grayscale
  invert
  autolevels
  brighten:<int>
  contrast:<float>
  blur:<float>
  huerotate:<int>
  saturate:<float>
  gamma:<float>
  unsharpen:<sigma>,<threshold>
  rotate90 | rotate180 | rotate270
  flip-horizontal | flip-vertical
  crop:<x>,<y>,<w>,<h>
  resize:<w>,<h>[,<filter>]

AI presets: enhance, denoise, dramatic, copilot-smart"
}

#[cfg(test)]
mod tests {
    use super::parse_filter;
    use image::imageops::FilterType;

    #[test]
    fn parses_filter() {
        let f = parse_filter("gaussian").expect("filter should parse");
        assert_eq!(f, FilterType::Gaussian);
    }
}
