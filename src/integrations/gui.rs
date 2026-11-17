use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[cfg(feature = "gui")]
pub fn run_gui(initial: Option<PathBuf>) -> Result<()> {
    use crate::editing::{AiPreset, apply_pipeline, parse_operation, recommend_copilot_pipeline};
    use eframe::egui;
    use std::path::Path;

    struct FormaGui {
        input: String,
        output: String,
        ops_multiline: String,
        ai: String,
        copilot: bool,
        status: String,
    }

    impl eframe::App for FormaGui {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("forma desktop editor");
                ui.label("Supports standard images, PSD input, RAW fallback, and AI/Copilot suggestions.");

                ui.horizontal(|ui| {
                    ui.label("Input:");
                    ui.text_edit_singleline(&mut self.input);
                });
                ui.horizontal(|ui| {
                    ui.label("Output:");
                    ui.text_edit_singleline(&mut self.output);
                });
                ui.horizontal(|ui| {
                    ui.label("AI preset:");
                    ui.text_edit_singleline(&mut self.ai);
                });
                ui.checkbox(&mut self.copilot, "Apply Copilot smart suggestion");

                ui.add(
                    egui::TextEdit::multiline(&mut self.ops_multiline)
                        .hint_text("autolevels\ncontrast:8\nsaturate:1.1")
                        .desired_rows(10),
                );

                if ui.button("Run Edit Pipeline").clicked() {
                    let op_lines = self
                        .ops_multiline
                        .lines()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    let parsed_ops = op_lines
                        .iter()
                        .map(|v| parse_operation(v))
                        .collect::<Result<Vec<_>>>();
                    let result = parsed_ops.and_then(|ops| {
                        let ai = if self.ai.trim().is_empty() {
                            None
                        } else {
                            Some(AiPreset::parse(self.ai.trim())?)
                        };
                        apply_pipeline(
                            Path::new(&self.input),
                            Path::new(&self.output),
                            &ops,
                            ai,
                            self.copilot,
                        )
                    });

                    self.status = match result {
                        Ok(effective) => format!("Success: {} operation(s) applied.", effective.len()),
                        Err(err) => format!("Error: {err:#}"),
                    };
                }

                if ui
                    .button("Generate Copilot suggestion from input image")
                    .clicked()
                {
                    let suggestion = crate::editing::load_image_with_psd_and_raw_support(Path::new(&self.input))
                        .map(|img| recommend_copilot_pipeline(&img))
                        .map(|ops| {
                            ops.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n")
                        });
                    match suggestion {
                        Ok(s) => {
                            self.ops_multiline = s;
                            self.status = "Suggestion generated.".to_string();
                        }
                        Err(err) => {
                            self.status = format!("Error: {err:#}");
                        }
                    }
                }

                ui.separator();
                ui.label(&self.status);
            });
        }
    }

    let app = FormaGui {
        input: initial
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        output: String::new(),
        ops_multiline: "autolevels\ncontrast:8\nsaturate:1.1".to_string(),
        ai: "enhance".to_string(),
        copilot: false,
        status: "Ready.".to_string(),
    };

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "forma editor",
        native_options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .map_err(|err| anyhow!("failed to run GUI: {err}"))
}

#[cfg(not(feature = "gui"))]
pub fn run_gui(_initial: Option<PathBuf>) -> Result<()> {
    Err(anyhow!(
        "GUI is not enabled in this build. Rebuild with: cargo run --features gui -- gui"
    ))
}
