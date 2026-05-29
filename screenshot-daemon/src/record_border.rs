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
                let painter = ui.painter();
                let border = egui::Rect::from_min_max(
                    egui::pos2(self.x as f32, self.y as f32),
                    egui::pos2((self.x + self.w) as f32, (self.y + self.h) as f32),
                );
                painter.rect_stroke(
                    border.shrink(1.5),
                    0.0,
                    egui::Stroke::new(
                        3.0,
                        egui::Color32::from_rgba_unmultiplied(255, 50, 50, 180),
                    ),
                );
                let corner_len = 12.0;
                let corner_thick = 4.0;
                let corner_color =
                    egui::Color32::from_rgba_unmultiplied(255, 80, 80, 220);
                let tl = border.min;
                let tr = egui::pos2(border.max.x, border.min.y);
                let bl = egui::pos2(border.min.x, border.max.y);
                let br = border.max;
                painter.line_segment(
                    [tl, tl + egui::vec2(corner_len, 0.0)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [tl, tl + egui::vec2(0.0, corner_len)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [tr, tr + egui::vec2(-corner_len, 0.0)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [tr, tr + egui::vec2(0.0, corner_len)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [bl, bl + egui::vec2(corner_len, 0.0)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [bl, bl + egui::vec2(0.0, -corner_len)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [br, br + egui::vec2(-corner_len, 0.0)],
                    egui::Stroke::new(corner_thick, corner_color),
                );
                painter.line_segment(
                    [br, br + egui::vec2(0.0, -corner_len)],
                    egui::Stroke::new(corner_thick, corner_color),
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
            .with_fullscreen(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_transparent(true)
            .with_mouse_passthrough(true)
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
