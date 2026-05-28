struct DepDialog {
    report: String,
    close_requested: bool,
}

impl eframe::App for DepDialog {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                    .rounding(egui::Rounding::same(8.0))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 80, 80)))
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("⚠ Missing Dependencies")
                            .font(egui::FontId::proportional(20.0))
                            .color(egui::Color32::from_rgb(255, 180, 60))
                            .strong(),
                    );
                    ui.add_space(12.0);
                });

                ui.add_space(4.0);

                for line in self.report.lines() {
                    if line.contains("MISSING") {
                        ui.label(
                            egui::RichText::new(line)
                                .font(egui::FontId::monospace(13.0))
                                .color(egui::Color32::from_rgb(255, 100, 100)),
                        );
                    } else if line.contains("OK") {
                        ui.label(
                            egui::RichText::new(line)
                                .font(egui::FontId::monospace(13.0))
                                .color(egui::Color32::from_rgb(100, 255, 100)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(line)
                                .font(egui::FontId::monospace(13.0))
                                .color(egui::Color32::LIGHT_GRAY),
                        );
                    }
                }

                ui.add_space(16.0);

                ui.vertical_centered(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("OK")
                                    .font(egui::FontId::proportional(14.0))
                                    .color(egui::Color32::WHITE),
                            )
                            .min_size(egui::vec2(80.0, 28.0)),
                        )
                        .clicked()
                    {
                        self.close_requested = true;
                    }
                });
            });

        if self.close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let report = args.get(1).cloned().unwrap_or_default();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(480.0, 360.0))
            .with_resizable(false)
            .with_title("Screenshot Daemon - Dependency Warning"),
        ..Default::default()
    };

    eframe::run_native(
        "Dependency Warning",
        native_options,
        Box::new(move |_cc| Ok(Box::new(DepDialog { report, close_requested: false }))),
    )
}
