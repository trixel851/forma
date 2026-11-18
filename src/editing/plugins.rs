use anyhow::{Context, Result, anyhow, bail};
use image::DynamicImage;
use rquickjs::{Context as JsContext, Function, Object, Runtime, TypedArray};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_PLUGIN_BYTES: u64 = 256 * 1024;
const JS_MEMORY_LIMIT: usize = 256 * 1024 * 1024;
const JS_STACK_LIMIT: usize = 512 * 1024;
const JS_TIME_LIMIT: Duration = Duration::from_secs(2);
const MAX_IMAGE_PLUGIN_TIME: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

fn default_plugin_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("plugins")];
    if cfg!(target_os = "windows") {
        if let Some(app_data) = env::var_os("APPDATA") {
            dirs.push(PathBuf::from(app_data).join("forma").join("plugins"));
        }
    } else if cfg!(target_os = "macos") {
        if let Some(home) = env::var_os("HOME") {
            dirs.push(
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("forma")
                    .join("plugins"),
            );
        }
    } else if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(config_home).join("forma").join("plugins"));
    } else if let Some(home) = env::var_os("HOME") {
        dirs.push(
            PathBuf::from(home)
                .join(".config")
                .join("forma")
                .join("plugins"),
        );
    }
    dirs
}

fn plugin_dirs(extra_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = default_plugin_dirs();
    dirs.extend(extra_dirs.iter().cloned());
    dirs
}

fn discover_files(extra_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for dir in plugin_dirs(extra_dirs) {
        if !dir.exists() {
            continue;
        }
        if !dir.is_dir() {
            bail!("plugin search path is not a directory: {}", dir.display());
        }
        for entry in fs::read_dir(&dir)
            .with_context(|| format!("failed to read plugin directory {}", dir.display()))?
        {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("js"))
            {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn read_source(path: &Path) -> Result<String> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.len() > MAX_PLUGIN_BYTES {
        bail!(
            "plugin {} exceeds the {} KiB source limit",
            path.display(),
            MAX_PLUGIN_BYTES / 1024
        );
    }
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        || !id.as_bytes()[0].is_ascii_lowercase()
    {
        bail!(
            "plugin ID '{id}' is invalid; use 1-64 lowercase letters, digits, '.', '_' or '-' and start with a letter"
        );
    }
    Ok(())
}

fn with_plugin_context<T>(
    source: &str,
    path: &Path,
    time_limit: Duration,
    f: impl for<'js> FnOnce(rquickjs::Ctx<'js>, Object<'js>) -> Result<T>,
) -> Result<T> {
    let runtime = Runtime::new().context("failed to create JavaScript plugin runtime")?;
    runtime.set_memory_limit(JS_MEMORY_LIMIT);
    runtime.set_max_stack_size(JS_STACK_LIMIT);
    let deadline = Instant::now() + time_limit;
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = JsContext::full(&runtime).context("failed to create JavaScript context")?;
    let executable_source = format!("{source}\n;globalThis.formaPlugin");
    context
        .with(|ctx| {
            let plugin: Object = ctx
                .eval(executable_source.as_bytes())
                .with_context(|| format!("plugin initialization failed: {}", path.display()))?;
            f(ctx, plugin)
        })
        .map_err(|error| anyhow!("JavaScript plugin error in {}: {error}", path.display()))
}

fn plugin_info(path: &Path) -> Result<PluginInfo> {
    let source = read_source(path)?;
    with_plugin_context(&source, path, JS_TIME_LIMIT, |_ctx, plugin| {
        let id: String = plugin
            .get("id")
            .context("plugin must define formaPlugin.id")?;
        validate_id(&id)?;
        let name: String = plugin
            .get("name")
            .context("plugin must define formaPlugin.name")?;
        let version: String = plugin
            .get("version")
            .context("plugin must define formaPlugin.version")?;
        let _: Function = plugin
            .get("apply")
            .context("plugin must define formaPlugin.apply(image)")?;
        if name.trim().is_empty() || name.len() > 128 {
            bail!("plugin name must contain 1-128 non-whitespace characters");
        }
        if version.trim().is_empty() || version.len() > 64 {
            bail!("plugin version must contain 1-64 non-whitespace characters");
        }
        Ok(PluginInfo {
            id,
            name,
            version,
            path: path.to_path_buf(),
        })
    })
}

pub fn list_plugins(extra_dirs: &[PathBuf]) -> Result<Vec<PluginInfo>> {
    let mut plugins = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for path in discover_files(extra_dirs)? {
        let info = plugin_info(&path)
            .with_context(|| format!("invalid plugin file {}", path.display()))?;
        if !ids.insert(info.id.clone()) {
            bail!("duplicate JavaScript plugin ID '{}'", info.id);
        }
        plugins.push(info);
    }
    Ok(plugins)
}

pub fn apply_plugins(
    mut image: DynamicImage,
    selected_ids: &[String],
    extra_dirs: &[PathBuf],
) -> Result<DynamicImage> {
    if selected_ids.is_empty() {
        return Ok(image);
    }

    let plugins = list_plugins(extra_dirs)?;
    for selected_id in selected_ids {
        validate_id(selected_id)?;
        let plugin = plugins
            .iter()
            .find(|plugin| plugin.id == *selected_id)
            .ok_or_else(|| anyhow!("JavaScript plugin '{selected_id}' was not found"))?;
        let rgba = image.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        let path = &plugin.path;
        let source = read_source(path)?;
        let pixel_count = u64::from(width) * u64::from(height);
        let time_limit = (JS_TIME_LIMIT
            + Duration::from_millis((pixel_count / 500).min(10_000) as u64))
        .min(MAX_IMAGE_PLUGIN_TIME);
        let result = with_plugin_context(&source, path, time_limit, |ctx, plugin_object| {
            let apply: Function = plugin_object.get("apply")?;
            let input = Object::new(ctx.clone())?;
            input.set("width", width)?;
            input.set("height", height)?;
            input.set("data", TypedArray::<u8>::new_copy(ctx, rgba.as_raw())?)?;
            let output: Object = apply
                .call((input,))
                .context("plugin apply(image) must return an image object")?;

            let output_width: u32 = output.get("width")?;
            let output_height: u32 = output.get("height")?;
            if output_width != width || output_height != height {
                bail!("plugins must preserve image width and height");
            }
            let output_data: TypedArray<u8> = output.get("data")?;
            if output_data.len() != rgba.as_raw().len() {
                bail!("plugin output pixel buffer has the wrong length");
            }
            let data = unsafe {
                output_data
                    .as_bytes()
                    .ok_or_else(|| anyhow!("plugin returned a detached pixel buffer"))?
                    .to_vec()
            };
            image::RgbaImage::from_raw(width, height, data)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| anyhow!("plugin returned an invalid image buffer"))
        })
        .with_context(|| format!("failed to apply plugin '{}'", plugin.id))?;
        image = result;
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::{Duration, Instant, apply_plugins, list_plugins};
    use image::{DynamicImage, Rgba, RgbaImage};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestPluginDir(PathBuf);

    impl TestPluginDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("forma-plugin-test-{unique}"));
            fs::create_dir_all(&path).expect("create plugin dir");
            Self(path)
        }

        fn write(&self, name: &str, source: &str) {
            fs::write(self.0.join(name), source).expect("write plugin");
        }
    }

    impl Drop for TestPluginDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn discovers_and_applies_javascript_plugin() {
        let dir = TestPluginDir::new();
        dir.write(
            "invert.js",
            r#"globalThis.formaPlugin = {
                id: "invert-extra",
                name: "Invert extra",
                version: "1.0.0",
                apply(image) {
                    if (typeof process !== "undefined" ||
                        typeof require !== "undefined" ||
                        typeof fetch !== "undefined") {
                        throw new Error("host APIs must be unavailable");
                    }
                    for (let i = 0; i < image.data.length; i += 4) {
                        image.data[i] = 255 - image.data[i];
                        image.data[i + 1] = 255 - image.data[i + 1];
                        image.data[i + 2] = 255 - image.data[i + 2];
                    }
                    return image;
                }
            };"#,
        );
        let plugins = list_plugins(std::slice::from_ref(&dir.0)).expect("list plugins");
        assert!(plugins.iter().any(|plugin| plugin.id == "invert-extra"));

        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([20, 40, 80, 77])));
        let result = apply_plugins(
            image,
            &["invert-extra".to_string()],
            std::slice::from_ref(&dir.0),
        )
        .expect("apply plugin")
        .to_rgba8();
        assert_eq!(result.get_pixel(0, 0), &Rgba([235, 215, 175, 77]));
    }

    #[test]
    fn rejects_plugins_that_change_dimensions() {
        let dir = TestPluginDir::new();
        dir.write(
            "resize.js",
            r#"globalThis.formaPlugin = {
                id: "bad-size",
                name: "Bad size",
                version: "1",
                apply(image) { image.width += 1; return image; }
            };"#,
        );
        let image = DynamicImage::ImageRgba8(RgbaImage::new(1, 1));
        let error = apply_plugins(image, &["bad-size".to_string()], &[dir.0.clone()])
            .expect_err("dimension changes must fail");
        assert!(format!("{error:#}").contains("preserve image width and height"));
    }

    #[test]
    fn interrupts_non_terminating_plugins() {
        let dir = TestPluginDir::new();
        dir.write(
            "loop.js",
            r#"globalThis.formaPlugin = {
                id: "infinite-loop",
                name: "Infinite loop",
                version: "1",
                apply(image) { while (true) {} }
            };"#,
        );
        let image = DynamicImage::ImageRgba8(RgbaImage::new(1, 1));
        let started = Instant::now();
        let error = apply_plugins(image, &["infinite-loop".to_string()], &[dir.0.clone()])
            .expect_err("non-terminating plugin must be interrupted");
        assert!(format!("{error:#}").contains("plugin"));
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
