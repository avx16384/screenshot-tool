use std::sync::{Arc, Mutex};
use std::time::Instant;

struct ControlState {
    recording: bool,
    paused: bool,
    stopped: bool,
    start_time: Option<Instant>,
    paused_elapsed: std::time::Duration,
}

struct ControlBar {
    state: Arc<Mutex<ControlState>>,
    drag_offset: Option<egui::Pos2>,
}

impl ControlBar {
    fn new(state: Arc<Mutex<ControlState>>) -> Self {
        Self { state, drag_offset: None }
    }
}

impl eframe::App for ControlBar {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut state = self.state.lock().unwrap();

        if state.stopped {
            drop(state);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let elapsed = if let Some(start) = state.start_time {
            if state.paused {
                state.paused_elapsed
            } else {
                state.paused_elapsed + start.elapsed()
            }
        } else {
            std::time::Duration::ZERO
        };

        let secs = elapsed.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        let hours = mins / 60;
        let time_str = if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins % 60, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        };

        let is_paused = state.paused;

        let response = egui::Area::new(egui::Id::new("control_bar"))
            .movable(false)
            .interactable(true)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 40, 230))
                        .rounding(egui::Rounding::same(8.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 100)))
                        .inner_margin(egui::Margin::symmetric(8.0, 4.0));

                    frame.show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            let dot_color = if is_paused {
                                egui::Color32::from_rgb(255, 200, 50)
                            } else {
                                egui::Color32::from_rgb(255, 60, 60)
                            };
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            let painter = ui.painter();
                            let center = rect.center();
                            painter.circle_filled(center, 5.0, dot_color);

                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new(&time_str)
                                    .font(egui::FontId::monospace(16.0))
                                    .color(egui::Color32::WHITE),
                            );

                            ui.add_space(8.0);

                            let pause_label = if is_paused { "▶" } else { "⏸" };
                            let pause_tooltip = if is_paused { "Resume" } else { "Pause" };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(pause_label)
                                            .font(egui::FontId::proportional(16.0))
                                            .color(egui::Color32::WHITE),
                                    )
                                    .frame(false)
                                    .min_size(egui::vec2(28.0, 24.0)),
                                )
                                .on_hover_text(pause_tooltip)
                                .clicked()
                            {
                                if state.paused {
                                    state.start_time = Some(Instant::now());
                                    state.paused = false;
                                    println!("resumed");
                                } else {
                                    let elapsed = state.start_time.unwrap().elapsed();
                                    state.paused_elapsed += elapsed;
                                    state.paused = true;
                                    println!("paused");
                                }
                            }

                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("⏹")
                                            .font(egui::FontId::proportional(16.0))
                                            .color(egui::Color32::from_rgb(255, 100, 100)),
                                    )
                                    .frame(false)
                                    .min_size(egui::vec2(28.0, 24.0)),
                                )
                                .on_hover_text("Stop recording")
                                .clicked()
                            {
                                state.recording = false;
                                state.stopped = true;
                                println!("stopped");
                            }
                        });
                    });
                });
            })
            .response;

        if response.dragged() {
            if self.drag_offset.is_none() {
                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                    let screen_rect = ctx.input(|i| i.screen_rect);
                    self.drag_offset = Some(egui::pos2(
                        pos.x - screen_rect.left(),
                        pos.y - screen_rect.top(),
                    ));
                }
            }
            if let Some(offset) = self.drag_offset {
                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                        egui::pos2(pos.x - offset.x, pos.y - offset.y),
                    ));
                }
            }
        } else {
            self.drag_offset = None;
        }

        drop(state);
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let state = Arc::new(Mutex::new(ControlState {
        recording: true,
        paused: false,
        stopped: false,
        start_time: Some(Instant::now()),
        paused_elapsed: std::time::Duration::ZERO,
    }));

    let state_after = state.clone();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(200.0, 40.0))
            .with_decorations(false)
            .with_always_on_top()
            .with_transparent(true)
            .with_resizable(false)
            .with_title("Screen Recorder"),
        ..Default::default()
    };

    eframe::run_native(
        "Screen Recorder Control",
        native_options,
        Box::new(move |_cc| Ok(Box::new(ControlBar::new(state)))),
    )?;

    let s = state_after.lock().unwrap();
    if s.stopped {
        println!("stopped");
    } else {
        println!("cancelled");
    }

    Ok(())
}
