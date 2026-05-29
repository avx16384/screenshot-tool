struct RedBorderOverlay {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl eframe::App for RedBorderOverlay {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter();
                painter.rect_stroke(
                    rect.shrink(2.0),
                    0.0,
                    egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(255, 50, 50, 180)),
                );
            });
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (x, y, w, h) = if args.len() >= 5 {
        (
            args[1].parse::<i32>().unwrap_or(0),
            args[2].parse::<i32>().unwrap_or(0),
            args[3].parse::<i32>().unwrap_or(1920),
            args[4].parse::<i32>().unwrap_or(1080),
        )
    } else {
        (0, 0, 1920, 1080)
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_position(egui::Pos2::new(x as f32, y as f32))
            .with_inner_size(egui::vec2(w as f32, h as f32))
            .with_decorations(false)
            .with_always_on_top()
            .with_transparent(true)
            .with_resizable(false)
            .with_title("Recording Border"),
        ..Default::default()
    };

    eframe::run_native(
        "Recording Border",
        native_options,
        Box::new(move |_cc| Ok(Box::new(RedBorderOverlay { x, y, w, h }))),
    )
}
