use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[cfg(feature = "gui")]
pub fn run_gui(initial: Option<PathBuf>) -> Result<()> {
    use crate::editing::{
        AiPreset, Operation, PluginInfo, ai_operations, apply_operation,
        apply_pipeline_with_plugins, apply_plugins, list_plugins,
        load_image_with_psd_and_raw_support, parse_operation, recommend_copilot_pipeline,
    };
    use eframe::egui;
    use image::DynamicImage;
    use std::path::Path;

    fn suggested_output(input: &Path) -> String {
        let stem = input
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("forma-image");
        let mut output = input.to_path_buf();
        output.set_file_name(format!("{stem}-edited.png"));
        output.to_string_lossy().to_string()
    }

    struct FormaGui {
        input: String,
        output: String,
        operation_input: String,
        operations: Vec<String>,
        redo: Vec<String>,
        plugins: Vec<PluginInfo>,
        enabled_plugins: Vec<String>,
        ai: String,
        copilot: bool,
        gpu: bool,
        dark_mode: bool,
        exposure: f32,
        contrast: f32,
        saturation: f32,
        vignette: f32,
        source: Option<DynamicImage>,
        texture: Option<egui::TextureHandle>,
        preview_dirty: bool,
        focus_input: bool,
        status: String,
    }

    impl FormaGui {
        fn refresh_preview(&mut self, ctx: &egui::Context) {
            let Some(source) = &self.source else {
                self.status = "Open an image to start editing.".to_string();
                self.preview_dirty = false;
                return;
            };
            let original_width = source.width();
            let original_height = source.height();

            let preview = (|| {
                let mut image = source.thumbnail(1400, 1000);
                let scale_x = image.width() as f32 / original_width as f32;
                let scale_y = image.height() as f32 / original_height as f32;
                if self.copilot {
                    for operation in recommend_copilot_pipeline(&image) {
                        image = apply_operation(image, &operation)?;
                    }
                }
                if !self.ai.trim().is_empty() {
                    for operation in ai_operations(AiPreset::parse(self.ai.trim())?, Some(&image)) {
                        image = apply_operation(image, &operation)?;
                    }
                }
                let operations = self
                    .operations
                    .iter()
                    .map(|raw| parse_operation(raw))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .chain(self.quick_operations())
                    .map(|operation| match operation {
                        Operation::Crop {
                            x,
                            y,
                            width,
                            height,
                        } => Operation::Crop {
                            x: (x as f32 * scale_x).round() as u32,
                            y: (y as f32 * scale_y).round() as u32,
                            width: (width as f32 * scale_x).round().max(1.0) as u32,
                            height: (height as f32 * scale_y).round().max(1.0) as u32,
                        },
                        Operation::Resize {
                            width,
                            height,
                            filter,
                        } => Operation::Resize {
                            width: (width as f32 * scale_x).round().max(1.0) as u32,
                            height: (height as f32 * scale_y).round().max(1.0) as u32,
                            filter,
                        },
                        Operation::Blur(sigma) => Operation::Blur(sigma * scale_x.min(scale_y)),
                        Operation::Unsharpen { sigma, threshold } => Operation::Unsharpen {
                            sigma: sigma * scale_x.min(scale_y),
                            threshold,
                        },
                        other => other,
                    });
                for operation in operations {
                    image = apply_operation(image, &operation)?;
                }
                image = apply_plugins(image, &self.enabled_plugins, &[])?;
                Ok::<_, anyhow::Error>(image)
            })();
            match preview {
                Ok(image) => {
                    let rgba = image.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    if let Some(texture) = &mut self.texture {
                        texture.set(color_image, egui::TextureOptions::LINEAR);
                    } else {
                        self.texture = Some(ctx.load_texture(
                            "forma-preview",
                            color_image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                    self.status = format!(
                        "{} x {} px | {} edit(s) | fast preview",
                        original_width,
                        original_height,
                        self.operations.len() + self.quick_operations().len()
                    );
                }
                Err(error) => {
                    self.status = format!("Preview error: {error:#}");
                }
            }
            self.preview_dirty = false;
        }

        fn open_image(&mut self, ctx: &egui::Context) {
            match load_image_with_psd_and_raw_support(Path::new(&self.input)) {
                Ok(image) => {
                    self.source = Some(image);
                    self.operations.clear();
                    self.redo.clear();
                    self.texture = None;
                    self.exposure = 0.0;
                    self.contrast = 0.0;
                    self.saturation = 1.0;
                    self.vignette = 0.0;
                    if self.output.trim().is_empty() {
                        self.output = suggested_output(Path::new(&self.input));
                    }
                    self.preview_dirty = false;
                    self.refresh_preview(ctx);
                }
                Err(error) => self.status = format!("Open failed: {error:#}"),
            }
        }

        fn save_image(&mut self) {
            if self.source.is_none() {
                self.status = "Open an image before exporting.".to_string();
                return;
            }
            if self.output.trim().is_empty() {
                self.status = "Choose an output path before exporting.".to_string();
                return;
            }
            let parsed = self
                .operations
                .iter()
                .map(|raw| parse_operation(raw))
                .collect::<Result<Vec<_>>>();
            let result = parsed.and_then(|operations| {
                let ai = if self.ai.trim().is_empty() {
                    None
                } else {
                    Some(AiPreset::parse(self.ai.trim())?)
                };
                apply_pipeline_with_plugins(
                    Path::new(&self.input),
                    Path::new(&self.output),
                    &operations
                        .into_iter()
                        .chain(self.quick_operations())
                        .collect::<Vec<_>>(),
                    ai,
                    self.copilot,
                    self.gpu,
                    &self.enabled_plugins,
                    &[],
                )
            });
            self.status = match result {
                Ok(operations) => {
                    format!("Exported {} edit(s) to {}.", operations.len(), self.output)
                }
                Err(error) => format!("Export failed: {error:#}"),
            };
        }

        fn add_operation(&mut self, operation: String, ctx: &egui::Context) {
            if parse_operation(&operation).is_err() {
                self.status = format!("Invalid operation: {operation}");
                return;
            }
            self.operations.push(operation);
            self.redo.clear();
            self.refresh_preview(ctx);
        }

        fn undo(&mut self, ctx: &egui::Context) {
            if let Some(operation) = self.operations.pop() {
                self.redo.push(operation);
                self.refresh_preview(ctx);
            }
        }

        fn redo(&mut self, ctx: &egui::Context) {
            if let Some(operation) = self.redo.pop() {
                self.operations.push(operation);
                self.refresh_preview(ctx);
            }
        }

        fn suggest(&mut self, ctx: &egui::Context) {
            let Some(source) = &self.source else {
                self.status = "Open an image before requesting a suggestion.".to_string();
                return;
            };
            let suggested = recommend_copilot_pipeline(source)
                .into_iter()
                .map(|operation| operation.to_string())
                .collect::<Vec<_>>();
            self.operations.extend(suggested);
            self.redo.clear();
            self.refresh_preview(ctx);
        }

        fn reload_plugins(&mut self, ctx: &egui::Context) {
            match list_plugins(&[]) {
                Ok(plugins) => {
                    let ids = plugins
                        .iter()
                        .map(|plugin| plugin.id.clone())
                        .collect::<std::collections::BTreeSet<_>>();
                    self.enabled_plugins.retain(|id| ids.contains(id));
                    self.plugins = plugins;
                    self.refresh_preview(ctx);
                    self.status = format!("Loaded {} JavaScript plugin(s).", self.plugins.len());
                }
                Err(error) => self.status = format!("Plugin discovery failed: {error:#}"),
            }
        }

        fn quick_operations(&self) -> Vec<Operation> {
            let mut operations = Vec::new();
            if self.exposure != 0.0 {
                operations.push(Operation::Exposure(self.exposure));
            }
            if self.contrast != 0.0 {
                operations.push(Operation::Contrast(self.contrast));
            }
            if (self.saturation - 1.0).abs() > f32::EPSILON {
                operations.push(Operation::Saturate(self.saturation));
            }
            if self.vignette > 0.0 {
                operations.push(Operation::Vignette(self.vignette));
            }
            operations
        }

        fn handle_shortcuts(&mut self, ctx: &egui::Context) {
            let (open, save, undo, redo) = ctx.input(|input| {
                let command = input.modifiers.command;
                (
                    command && input.key_pressed(egui::Key::O),
                    command && input.key_pressed(egui::Key::S),
                    command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
                    (command && input.key_pressed(egui::Key::Y))
                        || (command && input.modifiers.shift && input.key_pressed(egui::Key::Z)),
                )
            });
            if open {
                self.focus_input = true;
                self.status = "Enter an image path and press Open.".to_string();
            }
            if save {
                self.save_image();
            }
            if undo {
                self.undo(ctx);
            }
            if redo {
                self.redo(ctx);
            }
        }
    }

    impl eframe::App for FormaGui {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            ctx.set_visuals(if self.dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            self.handle_shortcuts(ctx);
            if self.preview_dirty {
                self.refresh_preview(ctx);
            }

            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("forma");
                    if ui.button("Open  Ctrl+O").clicked() {
                        self.focus_input = true;
                    }
                    if ui.button("Undo  Ctrl+Z").clicked() {
                        self.undo(ctx);
                    }
                    if ui.button("Redo  Ctrl+Shift+Z").clicked() {
                        self.redo(ctx);
                    }
                    if ui.button("Export  Ctrl+S").clicked() {
                        self.save_image();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.dark_mode, "Dark");
                    ui.checkbox(&mut self.gpu, "GPU");
                });
            });

            egui::SidePanel::left("editing_tools")
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.heading("Document");
                    ui.horizontal(|ui| {
                        ui.label("Input");
                        let response = ui.text_edit_singleline(&mut self.input);
                        if self.focus_input {
                            response.request_focus();
                            self.focus_input = false;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Output");
                        ui.text_edit_singleline(&mut self.output);
                    });
                    if ui.button("Open image").clicked() {
                        self.open_image(ctx);
                    }
                    ui.small("Supports standard images, PSD input, and RAW via ffmpeg.");

                    ui.separator();
                    ui.heading("Quick adjustments");
                    let mut changed = false;
                    changed |= ui
                        .add(egui::Slider::new(&mut self.exposure, -4.0..=4.0).text("Exposure"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut self.contrast, -80.0..=100.0).text("Contrast"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut self.saturation, 0.0..=2.0).text("Saturation"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut self.vignette, 0.0..=1.0).text("Vignette"))
                        .changed();
                    if changed {
                        self.refresh_preview(ctx);
                    }
                    if ui.button("Commit quick adjustments to history").clicked() {
                        self.operations.extend(
                            self.quick_operations()
                                .into_iter()
                                .map(|operation| operation.to_string()),
                        );
                        self.exposure = 0.0;
                        self.contrast = 0.0;
                        self.saturation = 1.0;
                        self.vignette = 0.0;
                        self.redo.clear();
                        self.refresh_preview(ctx);
                    }

                    ui.separator();
                    ui.heading("Effects");
                    ui.horizontal_wrapped(|ui| {
                        for (label, operation) in [
                            ("B&W", "grayscale"),
                            ("Invert", "invert"),
                            ("Auto levels", "autolevels"),
                            ("Sharpen", "unsharpen:0.8,2"),
                            ("Rotate 90", "rotate90"),
                            ("Flip H", "flip-horizontal"),
                        ] {
                            if ui.button(label).clicked() {
                                self.add_operation(operation.to_string(), ctx);
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.operation_input);
                        if ui.button("Add").clicked() {
                            let operation = self.operation_input.trim().to_string();
                            self.add_operation(operation, ctx);
                            self.operation_input.clear();
                        }
                    });
                    ui.small("Add any operation from `forma ops`, e.g. crop:10,10,800,600.");

                    ui.separator();
                    ui.heading("Local assist");
                    if ui.checkbox(&mut self.copilot, "Apply preset on export").changed() {
                        self.refresh_preview(ctx);
                    }
                    ui.horizontal(|ui| {
                        ui.label("Preset");
                        if ui.text_edit_singleline(&mut self.ai).changed() {
                            self.refresh_preview(ctx);
                        }
                    });
                    if ui.button("Suggest edits from image").clicked() {
                        self.suggest(ctx);
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.heading("JavaScript plugins");
                        if ui.button("Reload").clicked() {
                            self.reload_plugins(ctx);
                        }
                    });
                    for plugin in self.plugins.clone() {
                        let mut enabled = self.enabled_plugins.contains(&plugin.id);
                        if ui.checkbox(
                            &mut enabled,
                            format!("{} ({})", plugin.name, plugin.id),
                        )
                        .changed()
                        {
                            if enabled {
                                self.enabled_plugins.push(plugin.id.clone());
                            } else {
                                self.enabled_plugins.retain(|id| id != &plugin.id);
                            }
                            self.refresh_preview(ctx);
                        }
                    }
                    if self.plugins.is_empty() {
                        ui.small("Add .js plugins to the plugins folder, then reload.");
                    }

                    ui.separator();
                    ui.heading(format!("History ({})", self.operations.len()));
                    egui::ScrollArea::vertical()
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for (index, operation) in self.operations.iter().enumerate() {
                                ui.label(format!("{:02}. {}", index + 1, operation));
                            }
                        });
                    ui.separator();
                    ui.label(&self.status);
                    ui.small("Ctrl/Cmd+O open path · Ctrl/Cmd+S export · Ctrl/Cmd+Z undo · Ctrl/Cmd+Shift+Z redo");
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Preview");
                if let Some(texture) = &self.texture {
                    let available = ui.available_size();
                    let image_size = texture.size_vec2();
                    let scale = (available.x / image_size.x)
                        .min(available.y / image_size.y)
                        .min(1.0);
                    ui.centered_and_justified(|ui| {
                        ui.image((texture.id(), image_size * scale));
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Open a photo to preview and edit it.");
                    });
                }
            });
        }
    }

    let initial_source = initial
        .as_deref()
        .map(load_image_with_psd_and_raw_support)
        .transpose()?;
    let initial_output = initial
        .as_deref()
        .map(|path| suggested_output(path))
        .unwrap_or_default();
    let (plugins, plugin_status) = match list_plugins(&[]) {
        Ok(plugins) => (plugins, "Open an image to start editing.".to_string()),
        Err(error) => (Vec::new(), format!("Plugin discovery failed: {error:#}")),
    };
    let app = FormaGui {
        input: initial
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        output: initial_output,
        operation_input: String::new(),
        operations: Vec::new(),
        redo: Vec::new(),
        plugins,
        enabled_plugins: Vec::new(),
        ai: String::new(),
        copilot: false,
        gpu: false,
        dark_mode: true,
        exposure: 0.0,
        contrast: 0.0,
        saturation: 1.0,
        vignette: 0.0,
        source: initial_source,
        texture: None,
        preview_dirty: true,
        focus_input: false,
        status: plugin_status,
    };

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "forma editor",
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .map_err(|error| anyhow!("failed to run GUI: {error}"))
}

#[cfg(not(feature = "gui"))]
pub fn run_gui(_initial: Option<PathBuf>) -> Result<()> {
    Err(anyhow!(
        "GUI is not enabled in this build. Rebuild with: cargo run --features gui -- gui"
    ))
}
