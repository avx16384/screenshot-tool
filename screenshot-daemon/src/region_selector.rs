/// Region selector + annotation tool.
/// Used by screenshot-daemon through the C API.
///
/// 1. Captures fullscreen immediately on startup
/// 2. Shows fullscreen overlay — drag to select region
/// 3. Floating draggable toolbar (positioned near selection) with icon buttons + tooltips
/// 4. Annotate: Rect, Ellipse (rect-drag), Circle (center+radius), Line, Arrow, Text
/// 5. Save (Enter or button) → crops captured image, draws annotations, saves PNG
/// 6. Cancel (Esc) → prints "cancelled"
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Move,
    Rect,
    Ellipse,
    Circle,
    Line,
    Arrow,
    Text,
}

#[derive(Clone)]
enum Annotation {
    Rect { x1: f32, y1: f32, x2: f32, y2: f32 },
    Ellipse { x1: f32, y1: f32, x2: f32, y2: f32 },
    Circle { cx: f32, cy: f32, r: f32 },
    Line { x1: f32, y1: f32, x2: f32, y2: f32 },
    Arrow { x1: f32, y1: f32, x2: f32, y2: f32 },
    Text { x: f32, y: f32, content: String },
}

struct SharedState {
    selection: Option<egui::Rect>,
    annotations: Vec<Annotation>,
    finished: bool,
    cancelled: bool,
    fullscreen: Option<image::RgbaImage>,
    pixels_per_point: f32,
}

struct App {
    #[allow(dead_code)]
    fullscreen: Option<image::RgbaImage>,
    bg_texture: Option<egui::TextureHandle>,
    icons: Option<ToolIcons>,
    selection: Option<egui::Rect>,
    select_start: Option<egui::Pos2>,
    select_current: Option<egui::Pos2>,
    phase: Phase,
    tool: Tool,
    annotations: Vec<Annotation>,
    draw_start: Option<egui::Pos2>,
    draw_current: Option<egui::Pos2>,
    text_input: String,
    text_pos: Option<egui::Pos2>,
    toolbar_pos: Option<egui::Pos2>,
    toolbar_dragging: bool,
    move_dragging: bool,
    #[allow(dead_code)]
    output_path: Option<String>,
    shared: Arc<Mutex<SharedState>>,
    record_mode: bool,
}

struct ToolIcons {
    mv: egui::TextureHandle,
    rect: egui::TextureHandle,
    ellipse: egui::TextureHandle,
    circle: egui::TextureHandle,
    line: egui::TextureHandle,
    arrow: egui::TextureHandle,
    text: egui::TextureHandle,
    save: egui::TextureHandle,
    cancel: egui::TextureHandle,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Selecting,
    Annotating,
}

// ── Icon drawing helpers ──────────────────────────────────────────────

fn make_icon(
    cc: &eframe::CreationContext<'_>,
    name: &str,
    draw_fn: impl FnOnce(&mut [u8], u32),
) -> egui::TextureHandle {
    let size = 24u32;
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    draw_fn(&mut pixels, size);
    let img = egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], &pixels);
    cc.egui_ctx
        .load_texture(name, img, egui::TextureOptions::LINEAR)
}

fn set_px(px: &mut [u8], w: u32, x: u32, y: u32, c: [u8; 4]) {
    if x < w && y < w {
        let i = ((y * w + x) * 4) as usize;
        px[i] = c[0];
        px[i + 1] = c[1];
        px[i + 2] = c[2];
        px[i + 3] = c[3];
    }
}

fn draw_icon_rect(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    for x in 4..w - 4 {
        set_px(px, w, x, 4, c);
        set_px(px, w, x, w - 5, c);
    }
    for y in 4..w - 4 {
        set_px(px, w, 4, y, c);
        set_px(px, w, w - 5, y, c);
    }
}

fn draw_icon_ellipse(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    let cx = w as f32 / 2.0;
    let cy = w as f32 / 2.0;
    let rx = 9.0f32;
    let ry = 6.0f32;
    for a in 0..360 {
        let r = (a as f32).to_radians();
        let x = (cx + r.cos() * rx) as u32;
        let y = (cy + r.sin() * ry) as u32;
        set_px(px, w, x, y, c);
    }
}

fn draw_icon_circle(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    let cx = w as i32 / 2;
    let cy = w as i32 / 2;
    let r = 8i32;
    for a in 0..360 {
        let rad = (a as f64).to_radians();
        set_px(
            px,
            w,
            (cx + (rad.cos() * r as f64) as i32) as u32,
            (cy + (rad.sin() * r as f64) as i32) as u32,
            c,
        );
    }
    // Small crosshair at center
    for d in -2..=2i32 {
        set_px(px, w, (cx + d) as u32, cy as u32, c);
        set_px(px, w, cx as u32, (cy + d) as u32, c);
    }
}

fn draw_icon_line(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    for i in 4..w - 4 {
        set_px(px, w, i, i, c);
    }
}

fn draw_icon_arrow(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    for i in 4..w - 4 {
        set_px(px, w, i, w - 1 - i, c);
    }
    for i in 0..5 {
        set_px(px, w, w - 5 - i, 5 + i, c);
        set_px(px, w, w - 5 + i, 5 + i, c);
    }
}

fn draw_icon_text(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    for x in 6..18 {
        set_px(px, w, x, 6, c);
        set_px(px, w, x, 7, c);
    }
    for y in 7..18 {
        set_px(px, w, 11, y, c);
        set_px(px, w, 12, y, c);
    }
}

fn draw_icon_save(px: &mut [u8], w: u32) {
    let c = [100u8, 255, 100, 255];
    for i in 0..6 {
        set_px(px, w, 6 + i, 14 + i, c);
    }
    for i in 0..12 {
        set_px(px, w, 12 + i, 20 - i, c);
    }
}

fn draw_icon_cancel(px: &mut [u8], w: u32) {
    let c = [255u8, 100, 100, 255];
    for i in 0..12 {
        set_px(px, w, 6 + i, 6 + i, c);
        set_px(px, w, 17 - i, 6 + i, c);
    }
}

fn draw_icon_move(px: &mut [u8], w: u32) {
    let c = [255u8, 255, 255, 255];
    let cx = w as i32 / 2;
    let cy = w as i32 / 2;
    // Four arrows from center: up, down, left, right
    // Up arrow
    for i in 0..5 {
        set_px(px, w, (cx - 2 + i) as u32, (cy - 8) as u32, c);
    }
    for i in 0..4 {
        set_px(px, w, (cx - 1 + i) as u32, (cy - 7) as u32, c);
    }
    for i in 0..2 {
        set_px(px, w, (cx + i) as u32, (cy - 6) as u32, c);
    }
    set_px(px, w, cx as u32, (cy - 5) as u32, c);
    // Down arrow
    for i in 0..5 {
        set_px(px, w, (cx - 2 + i) as u32, (cy + 7) as u32, c);
    }
    for i in 0..4 {
        set_px(px, w, (cx - 1 + i) as u32, (cy + 6) as u32, c);
    }
    for i in 0..2 {
        set_px(px, w, (cx + i) as u32, (cy + 5) as u32, c);
    }
    set_px(px, w, cx as u32, (cy + 4) as u32, c);
    // Left arrow
    for i in 0..5 {
        set_px(px, w, (cx - 8) as u32, (cy - 2 + i) as u32, c);
    }
    for i in 0..4 {
        set_px(px, w, (cx - 7) as u32, (cy - 1 + i) as u32, c);
    }
    for i in 0..2 {
        set_px(px, w, (cx - 6) as u32, (cy + i) as u32, c);
    }
    set_px(px, w, (cx - 5) as u32, cy as u32, c);
    // Right arrow
    for i in 0..5 {
        set_px(px, w, (cx + 7) as u32, (cy - 2 + i) as u32, c);
    }
    for i in 0..4 {
        set_px(px, w, (cx + 6) as u32, (cy - 1 + i) as u32, c);
    }
    for i in 0..2 {
        set_px(px, w, (cx + 5) as u32, (cy + i) as u32, c);
    }
    set_px(px, w, (cx + 4) as u32, cy as u32, c);
    // Center dot
    set_px(px, w, cx as u32, cy as u32, c);
}

// ── App impl ──────────────────────────────────────────────────────────

impl App {
    fn new(
        cc: Option<&eframe::CreationContext<'_>>,
        output_path: Option<String>,
        shared: Arc<Mutex<SharedState>>,
        fullscreen: Option<image::RgbaImage>,
        record_mode: bool,
    ) -> Self {
        let icons = cc.map(|cc| ToolIcons {
            mv: make_icon(cc, "move", draw_icon_move),
            rect: make_icon(cc, "rect", draw_icon_rect),
            ellipse: make_icon(cc, "ellipse", draw_icon_ellipse),
            circle: make_icon(cc, "circle", draw_icon_circle),
            line: make_icon(cc, "line", draw_icon_line),
            arrow: make_icon(cc, "arrow", draw_icon_arrow),
            text: make_icon(cc, "text", draw_icon_text),
            save: make_icon(cc, "save", draw_icon_save),
            cancel: make_icon(cc, "cancel", draw_icon_cancel),
        });
        Self {
            fullscreen,
            bg_texture: None,
            icons,
            selection: None,
            select_start: None,
            select_current: None,
            phase: Phase::Selecting,
            tool: Tool::Rect,
            annotations: Vec::new(),
            draw_start: None,
            draw_current: None,
            text_input: String::new(),
            text_pos: None,
            toolbar_pos: None,
            toolbar_dragging: false,
            move_dragging: false,
            output_path,
            shared,
            record_mode,
        }
    }

    fn finish(&mut self, ctx: &egui::Context) {
        let mut s = self.shared.lock().unwrap();
        s.selection = self.selection;
        s.annotations = self.annotations.clone();
        s.fullscreen = self.fullscreen.clone();
        s.pixels_per_point = ctx.pixels_per_point();
        s.finished = true;
        drop(s);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn cancel(&mut self, ctx: &egui::Context) {
        self.shared.lock().unwrap().cancelled = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn draw_ann(&self, painter: &egui::Painter, ann: &Annotation) {
        let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 50, 50));
        match ann {
            Annotation::Rect { x1, y1, x2, y2 } => {
                painter.rect_stroke(
                    egui::Rect::from_two_pos(egui::pos2(*x1, *y1), egui::pos2(*x2, *y2)),
                    0.0,
                    stroke,
                );
            }
            Annotation::Ellipse { x1, y1, x2, y2 } => {
                let r = egui::Rect::from_two_pos(egui::pos2(*x1, *y1), egui::pos2(*x2, *y2));
                painter.circle_stroke(r.center(), r.width().min(r.height()) / 2.0, stroke);
            }
            Annotation::Circle { cx, cy, r } => {
                painter.circle_stroke(egui::pos2(*cx, *cy), *r, stroke);
            }
            Annotation::Line { x1, y1, x2, y2 } => {
                painter.line_segment([egui::pos2(*x1, *y1), egui::pos2(*x2, *y2)], stroke);
            }
            Annotation::Arrow { x1, y1, x2, y2 } => {
                let s = egui::pos2(*x1, *y1);
                let e = egui::pos2(*x2, *y2);
                painter.line_segment([s, e], stroke);
                let d = (e - s).normalized();
                let p = egui::vec2(-d.y, d.x);
                let hl = 12.0;
                painter.line_segment([e, e - d * hl + p * (hl * 0.4)], stroke);
                painter.line_segment([e, e - d * hl - p * (hl * 0.4)], stroke);
            }
            Annotation::Text { x, y, content } => {
                painter.text(
                    egui::pos2(*x, *y),
                    egui::Align2::LEFT_TOP,
                    content,
                    egui::FontId::proportional(18.0),
                    egui::Color32::from_rgb(255, 50, 50),
                );
            }
        }
    }

    fn dim_outside(&self, painter: &egui::Painter, screen: egui::Rect, sel: egui::Rect) {
        let dim = egui::Color32::from_black_alpha(80);
        painter.rect_filled(
            egui::Rect::from_min_max(screen.left_top(), egui::pos2(screen.right(), sel.top())),
            0.0,
            dim,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(screen.left(), sel.bottom()),
                screen.right_bottom(),
            ),
            0.0,
            dim,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(screen.left(), sel.top()), sel.left_bottom()),
            0.0,
            dim,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(sel.right_top(), egui::pos2(screen.right(), sel.bottom())),
            0.0,
            dim,
        );
    }

    fn icon_btn(&self, tex: &egui::TextureHandle, tool: Tool) -> egui::ImageButton<'_> {
        let selected = self.tool == tool;
        let btn = egui::ImageButton::new(egui::load::SizedTexture::new(
            tex.id(),
            egui::vec2(20.0, 20.0),
        ));
        if selected {
            btn.tint(egui::Color32::from_rgb(100, 149, 237))
        } else {
            btn
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Upload fullscreen image as background texture ────────────
        if self.bg_texture.is_none() {
            if let Some(ref img) = self.fullscreen {
                let size = [img.width() as usize, img.height() as usize];
                let pixels = img.as_raw().clone();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                let tex = ctx.load_texture("bg", color_image, egui::TextureOptions::LINEAR);
                self.bg_texture = Some(tex);
            }
        }

        // ── Background image is rendered per-phase on the same painter as dim overlay ──

        // ── Global keys ─────────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.phase == Phase::Annotating {
                self.phase = Phase::Selecting;
                self.selection = None;
                self.select_start = None;
                self.select_current = None;
                self.annotations.clear();
                self.text_input.clear();
                self.text_pos = None;
            } else {
                self.cancel(ctx);
            }
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            if self.record_mode {
                self.finish(ctx);
            } else if self.tool == Tool::Text
                && self.text_pos.is_some()
                && !self.text_input.is_empty()
            {
                let p = self.text_pos.unwrap();
                self.annotations.push(Annotation::Text {
                    x: p.x,
                    y: p.y,
                    content: self.text_input.clone(),
                });
                self.text_input.clear();
                self.text_pos = None;
            } else if self.phase == Phase::Annotating {
                self.finish(ctx);
            }
        }

        if self.phase == Phase::Annotating
            && ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.ctrl)
        {
            self.annotations.pop();
        }

        // Tool shortcuts
        if self.phase == Phase::Annotating {
            let new_tool = ctx.input(|i| {
                if i.key_pressed(egui::Key::M) {
                    Some(Tool::Move)
                } else if i.key_pressed(egui::Key::R) {
                    Some(Tool::Rect)
                } else if i.key_pressed(egui::Key::E) {
                    Some(Tool::Ellipse)
                } else if i.key_pressed(egui::Key::C) {
                    Some(Tool::Circle)
                } else if i.key_pressed(egui::Key::L) {
                    Some(Tool::Line)
                } else if i.key_pressed(egui::Key::A) {
                    Some(Tool::Arrow)
                } else if i.key_pressed(egui::Key::T) {
                    Some(Tool::Text)
                } else {
                    None
                }
            });
            if let Some(t) = new_tool {
                self.tool = t;
                self.text_input.clear();
                self.text_pos = None;
            }
        }

        // ── Phase: Selecting ──────────────────────────────────────────
        if self.phase == Phase::Selecting {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                .show(ctx, |ui| {
                    // Interaction first (needs mutable borrow)
                    let resp = ui.allocate_response(ui.available_size(), egui::Sense::drag());
                    if resp.drag_started() {
                        self.select_start =
                            Some(ui.input(|i| i.pointer.latest_pos().unwrap_or_default()));
                        self.select_current = self.select_start;
                    }
                    if resp.dragged() {
                        self.select_current = ui.input(|i| i.pointer.latest_pos());
                    }
                    if resp.drag_stopped() {
                        if let (Some(s), Some(e)) = (self.select_start, self.select_current) {
                            let r = egui::Rect::from_two_pos(s, e);
                            if r.width() > 10.0 && r.height() > 10.0 {
                                self.selection = Some(r);
                                if self.record_mode {
                                    self.finish(ctx);
                                    return;
                                }
                                self.phase = Phase::Annotating;
                                self.tool = Tool::Rect;
                                // Position toolbar smartly near selection
                                let tb_w = 240.0;
                                let tb_h = 36.0;
                                let screen = ui.max_rect();
                                let tb_x = (r.center().x - tb_w / 2.0)
                                    .max(screen.left() + 4.0)
                                    .min(screen.right() - tb_w - 4.0);
                                let mut tb_y = (r.top() - tb_h - 4.0).max(screen.top() + 4.0);
                                if tb_y < screen.top() + 4.0 {
                                    tb_y = (r.bottom() + 4.0).min(screen.bottom() - tb_h - 4.0);
                                }
                                self.toolbar_pos = Some(egui::pos2(tb_x, tb_y));
                            }
                        }
                    }

                    // Drawing (immutable borrow of painter)
                    let painter = ui.painter();
                    let screen = ui.max_rect();

                    // Draw background image
                    if let Some(ref tex) = self.bg_texture {
                        let img_size = tex.size_vec2();
                        let scale = (screen.width() / img_size.x).max(screen.height() / img_size.y);
                        let draw_size = img_size * scale;
                        let offset = screen.min + (screen.size() - draw_size) * 0.5;
                        let draw_rect = egui::Rect::from_min_size(offset, draw_size);
                        painter.image(
                            tex.id(),
                            draw_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    if let (Some(s), Some(c)) = (self.select_start, self.select_current) {
                        let r = egui::Rect::from_two_pos(s, c);
                        // Dim outside selection, selection area stays bright
                        self.dim_outside(&painter, screen, r);
                        painter.rect_stroke(
                            r,
                            0.0,
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 149, 237)),
                        );
                        painter.text(
                            r.right_bottom() + egui::vec2(4.0, 0.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}x{}", r.width() as u32, r.height() as u32),
                            egui::FontId::proportional(14.0),
                            egui::Color32::WHITE,
                        );
                    } else {
                        // No selection yet — dim entire screen
                        painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(100));
                        if self.record_mode {
                            let hint = "Select recording area (Enter = fullscreen)";
                            painter.text(
                                screen.center(),
                                egui::Align2::CENTER_CENTER,
                                hint,
                                egui::FontId::proportional(18.0),
                                egui::Color32::WHITE,
                            );
                        }
                    }
                });
            ctx.request_repaint();
            return;
        }

        // ── Phase: Annotating ─────────────────────────────────────────
        let sel = self.selection.unwrap_or(egui::Rect::ZERO);

        // Draggable floating toolbar — hidden while moving region to avoid interference
        // We detect drag via ctx.input() pointer: if pointer is over the toolbar
        // and dragging, we move it. Buttons still work on click (not drag).
        if self.move_dragging {
            // Skip toolbar rendering while dragging region
        } else {
            let mut area = egui::Area::new(egui::Id::new("toolbar"))
                .movable(false)
                .constrain(true)
                .interactable(true);
            if let Some(pos) = self.toolbar_pos {
                area = area.fixed_pos(pos);
            }
            let toolbar_inner = area.show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_black_alpha(230))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80)))
                    .rounding(6.0)
                    .show(ui, |ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                            if let Some(icons) = &self.icons {
                                let tools: [(Tool, &egui::TextureHandle, &str); 7] = [
                                    (Tool::Move, &icons.mv, "Move selection (M)"),
                                    (Tool::Rect, &icons.rect, "Rectangle (R)"),
                                    (Tool::Ellipse, &icons.ellipse, "Ellipse (E)"),
                                    (
                                        Tool::Circle,
                                        &icons.circle,
                                        "Circle: click center, drag radius (C)",
                                    ),
                                    (Tool::Line, &icons.line, "Line (L)"),
                                    (Tool::Arrow, &icons.arrow, "Arrow (A)"),
                                    (Tool::Text, &icons.text, "Text (T)"),
                                ];
                                let save_id = icons.save.id();
                                let cancel_id = icons.cancel.id();
                                for (t, tex, tip) in tools {
                                    let btn = self.icon_btn(tex, t);
                                    if ui.add(btn).on_hover_text(tip).clicked() {
                                        self.tool = t;
                                        self.text_input.clear();
                                        self.text_pos = None;
                                    }
                                }
                                ui.separator();
                                let save_btn = egui::ImageButton::new(
                                    egui::load::SizedTexture::new(save_id, egui::vec2(20.0, 20.0)),
                                );
                                if ui.add(save_btn).on_hover_text("Save (Enter)").clicked() {
                                    self.finish(ctx);
                                }
                                let cancel_btn =
                                    egui::ImageButton::new(egui::load::SizedTexture::new(
                                        cancel_id,
                                        egui::vec2(20.0, 20.0),
                                    ));
                                if ui.add(cancel_btn).on_hover_text("Cancel (Esc)").clicked() {
                                    self.cancel(ctx);
                                }
                            } else {
                                for (t, label) in [
                                    (Tool::Move, "Move"),
                                    (Tool::Rect, "Rect"),
                                    (Tool::Ellipse, "Ellipse"),
                                    (Tool::Circle, "Circle"),
                                    (Tool::Line, "Line"),
                                    (Tool::Arrow, "Arrow"),
                                    (Tool::Text, "Text"),
                                ] {
                                    if ui.selectable_label(self.tool == t, label).clicked() {
                                        self.tool = t;
                                        self.text_input.clear();
                                        self.text_pos = None;
                                    }
                                }
                                ui.separator();
                                if ui.button("Save").clicked() {
                                    self.finish(ctx);
                                }
                                if ui.button("Cancel").clicked() {
                                    self.cancel(ctx);
                                }
                            }
                        });
                    });
                ui.min_rect()
            });
            let toolbar_rect = toolbar_inner.inner;

            // Move toolbar when pointer drags over it, clamped to screen
            let screen = ctx.screen_rect();
            let pointer = ctx.input(|i| i.pointer.clone());
            if pointer.is_decidedly_dragging() {
                if let Some(pos) = pointer.latest_pos() {
                    if toolbar_rect.contains(pos) || self.toolbar_dragging {
                        self.toolbar_dragging = true;
                        let delta = pointer.delta();
                        if let Some(p) = self.toolbar_pos {
                            let new_p = p + delta;
                            // Clamp so toolbar stays on screen
                            let tw = toolbar_rect.width();
                            let th = toolbar_rect.height();
                            let cx = new_p
                                .x
                                .max(screen.left() + 4.0)
                                .min(screen.right() - tw - 4.0);
                            let cy = new_p
                                .y
                                .max(screen.top() + 4.0)
                                .min(screen.bottom() - th - 4.0);
                            self.toolbar_pos = Some(egui::pos2(cx, cy));
                        }
                    }
                }
            } else {
                self.toolbar_dragging = false;
            }
        } // end else (toolbar visible)

        // Canvas — use Area instead of CentralPanel so it doesn't block toolbar drag
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("canvas"),
        ));
        // Draw background image
        if let Some(ref tex) = self.bg_texture {
            let img_size = tex.size_vec2();
            let scale = (screen.width() / img_size.x).max(screen.height() / img_size.y);
            let draw_size = img_size * scale;
            let offset = screen.min + (screen.size() - draw_size) * 0.5;
            let draw_rect = egui::Rect::from_min_size(offset, draw_size);
            painter.image(
                tex.id(),
                draw_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        self.dim_outside(&painter, screen, sel);
        painter.rect_stroke(
            sel,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 149, 237)),
        );

        for ann in &self.annotations {
            self.draw_ann(&painter, ann);
        }

        // In-progress preview
        if let (Some(s), Some(c)) = (self.draw_start, self.draw_current) {
            let temp = match self.tool {
                Tool::Rect => Some(Annotation::Rect {
                    x1: s.x,
                    y1: s.y,
                    x2: c.x,
                    y2: c.y,
                }),
                Tool::Ellipse => Some(Annotation::Ellipse {
                    x1: s.x,
                    y1: s.y,
                    x2: c.x,
                    y2: c.y,
                }),
                Tool::Circle => Some(Annotation::Circle {
                    cx: s.x,
                    cy: s.y,
                    r: (c - s).length(),
                }),
                Tool::Line => Some(Annotation::Line {
                    x1: s.x,
                    y1: s.y,
                    x2: c.x,
                    y2: c.y,
                }),
                Tool::Arrow => Some(Annotation::Arrow {
                    x1: s.x,
                    y1: s.y,
                    x2: c.x,
                    y2: c.y,
                }),
                _ => None,
            };
            if let Some(a) = &temp {
                self.draw_ann(&painter, a);
            }
        }

        // Text preview
        if self.tool == Tool::Text && self.text_pos.is_some() && !self.text_input.is_empty() {
            let p = self.text_pos.unwrap();
            painter.text(
                p,
                egui::Align2::LEFT_TOP,
                &self.text_input,
                egui::FontId::proportional(18.0),
                egui::Color32::from_rgb(255, 50, 50),
            );
            let tw = self.text_input.len() as f32 * 10.0;
            painter.line_segment(
                [egui::pos2(p.x + tw, p.y), egui::pos2(p.x + tw, p.y + 18.0)],
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }

        // Input — handle via ctx.input() so nothing blocks toolbar Area drag
        // Skip all drawing/move input if toolbar is being dragged
        let pointer = ctx.input(|i| i.pointer.clone());
        if self.toolbar_dragging {
            // toolbar has priority, don't process any canvas input
        } else if self.tool == Tool::Move {
            // Move tool: drag inside selection to reposition it
            if pointer.primary_down() && !self.move_dragging {
                let pos = pointer.latest_pos().unwrap_or_default();
                if sel.contains(pos) {
                    self.move_dragging = true;
                }
            }
            if pointer.primary_down() && self.move_dragging {
                let delta = pointer.delta();
                if delta != egui::Vec2::ZERO {
                    let new_sel = sel.translate(delta);
                    self.selection = Some(new_sel);
                }
            }
            if pointer.primary_released() {
                self.move_dragging = false;
            }
        } else if self.tool == Tool::Text {
            if pointer.primary_clicked() {
                let pos = pointer.latest_pos().unwrap_or_default();
                if sel.contains(pos) {
                    if !self.text_input.is_empty() && self.text_pos.is_some() {
                        let tp = self.text_pos.unwrap();
                        self.annotations.push(Annotation::Text {
                            x: tp.x,
                            y: tp.y,
                            content: self.text_input.clone(),
                        });
                    }
                    self.text_pos = Some(pos);
                    self.text_input.clear();
                }
            }
        } else {
            if pointer.primary_clicked() {
                let pos = pointer.latest_pos().unwrap_or_default();
                if sel.contains(pos) {
                    self.draw_start = Some(pos);
                    self.draw_current = Some(pos);
                }
            }
            if pointer.primary_down() && self.draw_start.is_some() {
                self.draw_current = pointer.latest_pos();
            }
            if pointer.primary_released() && self.draw_start.is_some() {
                if let (Some(s), Some(c)) = (self.draw_start, self.draw_current) {
                    if sel.contains(s) {
                        let ann = match self.tool {
                            Tool::Rect => Annotation::Rect {
                                x1: s.x,
                                y1: s.y,
                                x2: c.x,
                                y2: c.y,
                            },
                            Tool::Ellipse => Annotation::Ellipse {
                                x1: s.x,
                                y1: s.y,
                                x2: c.x,
                                y2: c.y,
                            },
                            Tool::Circle => Annotation::Circle {
                                cx: s.x,
                                cy: s.y,
                                r: (c - s).length(),
                            },
                            Tool::Line => Annotation::Line {
                                x1: s.x,
                                y1: s.y,
                                x2: c.x,
                                y2: c.y,
                            },
                            Tool::Arrow => Annotation::Arrow {
                                x1: s.x,
                                y1: s.y,
                                x2: c.x,
                                y2: c.y,
                            },
                            _ => unreachable!(),
                        };
                        self.annotations.push(ann);
                    }
                }
                self.draw_start = None;
                self.draw_current = None;
            }
        }

        // Text keyboard
        if self.tool == Tool::Text && self.text_pos.is_some() {
            ctx.input(|i| {
                for event in &i.raw.events {
                    match event {
                        egui::Event::Text(ch) => {
                            self.text_input.push_str(ch);
                        }
                        egui::Event::Key {
                            key: egui::Key::Backspace,
                            pressed: true,
                            ..
                        } => {
                            self.text_input.pop();
                        }
                        _ => {}
                    }
                }
            });
        }

        ctx.request_repaint();
    }
}

// ── X11 fullscreen capture ────────────────────────────────────────────

fn capture_fullscreen() -> anyhow::Result<image::RgbaImage> {
    match capture_fullscreen_x11() {
        Ok(img) => {
            eprintln!("[region-selector] X11 capture OK");
            Ok(img)
        }
        Err(e) => {
            eprintln!(
                "[region-selector] X11 capture failed: {}, trying wayshot",
                e
            );
            capture_fullscreen_wayshot()
        }
    }
}

fn capture_fullscreen_x11() -> anyhow::Result<image::RgbaImage> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;
    use x11rb::rust_connection::RustConnection;
    let (conn, sn) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[sn];
    let w = screen.width_in_pixels as u32;
    let h = screen.height_in_pixels as u32;
    let reply = get_image(
        &conn,
        ImageFormat::Z_PIXMAP,
        screen.root,
        0,
        0,
        w as u16,
        h as u16,
        u32::MAX,
    )?
    .reply()?;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for chunk in reply.data.chunks_exact(4) {
        rgba.push(chunk[2]);
        rgba.push(chunk[1]);
        rgba.push(chunk[0]);
        rgba.push(chunk[3]);
    }
    image::RgbaImage::from_raw(w, h, rgba).ok_or_else(|| anyhow::anyhow!("failed to create image"))
}

fn capture_fullscreen_wayshot() -> anyhow::Result<image::RgbaImage> {
    let conn = libwayshot::WayshotConnection::new()?;
    let img = conn.screenshot_all(false)?;
    eprintln!(
        "[region-selector] wayshot image loaded: {}x{}",
        img.width(),
        img.height()
    );
    Ok(img.to_rgba8())
}

// ── Draw annotations onto image ───────────────────────────────────────

fn draw_annotations_on_image(
    img: &mut image::RgbaImage,
    annotations: &[Annotation],
    sel: egui::Rect,
) {
    let red = image::Rgba([255, 50, 50, 255]);
    for ann in annotations {
        match ann {
            Annotation::Rect { x1, y1, x2, y2 } => {
                let r = egui::Rect::from_two_pos(egui::pos2(*x1, *y1), egui::pos2(*x2, *y2));
                draw_rect_outline(
                    img,
                    (r.min.x - sel.min.x) as i32,
                    (r.min.y - sel.min.y) as i32,
                    r.width() as i32,
                    r.height() as i32,
                    red,
                );
            }
            Annotation::Ellipse { x1, y1, x2, y2 } => {
                let r = egui::Rect::from_two_pos(egui::pos2(*x1, *y1), egui::pos2(*x2, *y2));
                let cx = (r.center().x - sel.min.x) as i32;
                let cy = (r.center().y - sel.min.y) as i32;
                draw_ellipse_outline(
                    img,
                    cx,
                    cy,
                    (r.width() / 2.0) as i32,
                    (r.height() / 2.0) as i32,
                    red,
                );
            }
            Annotation::Circle { cx, cy, r } => {
                let icx = (*cx - sel.min.x) as i32;
                let icy = (*cy - sel.min.y) as i32;
                let ir = *r as i32;
                draw_ellipse_outline(img, icx, icy, ir, ir, red);
            }
            Annotation::Line { x1, y1, x2, y2 } => {
                draw_line(
                    img,
                    (*x1 - sel.min.x) as i32,
                    (*y1 - sel.min.y) as i32,
                    (*x2 - sel.min.x) as i32,
                    (*y2 - sel.min.y) as i32,
                    red,
                );
            }
            Annotation::Arrow { x1, y1, x2, y2 } => {
                let sx = (*x1 - sel.min.x) as f32;
                let sy = (*y1 - sel.min.y) as f32;
                let ex = (*x2 - sel.min.x) as f32;
                let ey = (*y2 - sel.min.y) as f32;
                draw_line(img, sx as i32, sy as i32, ex as i32, ey as i32, red);
                let dx = ex - sx;
                let dy = ey - sy;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.0 {
                    let dx = dx / len;
                    let dy = dy / len;
                    let hl = 12.0;
                    let px = -dy;
                    let py = dx;
                    draw_line(
                        img,
                        ex as i32,
                        ey as i32,
                        (ex - dx * hl + px * hl * 0.4) as i32,
                        (ey - dy * hl + py * hl * 0.4) as i32,
                        red,
                    );
                    draw_line(
                        img,
                        ex as i32,
                        ey as i32,
                        (ex - dx * hl - px * hl * 0.4) as i32,
                        (ey - dy * hl - py * hl * 0.4) as i32,
                        red,
                    );
                }
            }
            Annotation::Text { x, y, content } => {
                draw_text(
                    img,
                    (*x - sel.min.x) as u32,
                    (*y - sel.min.y) as u32,
                    content,
                    red,
                );
            }
        }
    }
}

fn draw_rect_outline(
    img: &mut image::RgbaImage,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    c: image::Rgba<u8>,
) {
    for i in 0..w {
        put_pixel_safe(img, x + i, y, c);
        put_pixel_safe(img, x + i, y + h - 1, c);
    }
    for j in 0..h {
        put_pixel_safe(img, x, y + j, c);
        put_pixel_safe(img, x + w - 1, y + j, c);
    }
}

fn draw_ellipse_outline(
    img: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    c: image::Rgba<u8>,
) {
    if rx == 0 || ry == 0 {
        return;
    }
    let mut x = 0i32;
    let mut y = ry;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let mut d1 = ry2 - rx2 * ry + rx2 / 4;
    while ry2 * x <= rx2 * y {
        put_pixel_safe(img, cx + x, cy + y, c);
        put_pixel_safe(img, cx - x, cy + y, c);
        put_pixel_safe(img, cx + x, cy - y, c);
        put_pixel_safe(img, cx - x, cy - y, c);
        if d1 < 0 {
            x += 1;
            d1 += 2 * ry2 * x + ry2;
        } else {
            x += 1;
            y -= 1;
            d1 += 2 * ry2 * x - 2 * rx2 * y + ry2;
        }
    }
    let mut d2 = ry2 * (x * 2 + 1).pow(2) / 4 + rx2 * (y - 1).pow(2) - rx2 * ry2;
    while y >= 0 {
        put_pixel_safe(img, cx + x, cy + y, c);
        put_pixel_safe(img, cx - x, cy + y, c);
        put_pixel_safe(img, cx + x, cy - y, c);
        put_pixel_safe(img, cx - x, cy - y, c);
        if d2 > 0 {
            y -= 1;
            d2 -= 2 * rx2 * y + rx2;
        } else {
            y -= 1;
            x += 1;
            d2 += 2 * ry2 * x - 2 * rx2 * y + rx2;
        }
    }
}

fn draw_line(img: &mut image::RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, c: image::Rgba<u8>) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;
    loop {
        put_pixel_safe(img, cx, cy, c);
        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

fn draw_text(img: &mut image::RgbaImage, x: u32, y: u32, text: &str, c: image::Rgba<u8>) {
    const FONT: &[u8] = include_bytes!("font_5x7.bin");
    let mut cx = x;
    for ch in text.chars() {
        if ch < ' ' || ch > '~' {
            continue;
        }
        let off = ((ch as u32) - 32) * 7;
        for row in 0..7u32 {
            if off as usize + row as usize >= FONT.len() {
                break;
            }
            let bits = FONT[off as usize + row as usize];
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) != 0 {
                    let px = cx + col;
                    let py = y + row;
                    if px < img.width() && py < img.height() {
                        img.put_pixel(px, py, c);
                    }
                }
            }
        }
        cx += 6;
    }
}

fn put_pixel_safe(img: &mut image::RgbaImage, x: i32, y: i32, c: image::Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, c);
    }
}

// ── Main ──────────────────────────────────────────────────────────────

pub struct RegionSelectorOptions {
    pub output_path: Option<String>,
    pub background_path: Option<String>,
    pub record_mode: bool,
}

pub enum RegionSelectorOutcome {
    Cancelled,
    Saved(String),
    Fullscreen,
    Region(i32, i32, u32, u32),
    Noop,
}

pub fn run_region_selector(
    options: RegionSelectorOptions,
) -> eframe::Result<RegionSelectorOutcome> {
    let output_path = options.output_path;
    let output_path_clone = output_path.clone();
    let bg_path = options.background_path;
    let record_mode = options.record_mode;

    // Capture fullscreen BEFORE creating the overlay window
    // If --background is provided, use that; otherwise capture ourselves
    let fullscreen = if let Some(ref p) = bg_path {
        eprintln!("[region-selector] using provided background: {}", p);
        image::open(p).ok().map(|i| i.to_rgba8())
    } else {
        capture_fullscreen().ok()
    };

    let shared = Arc::new(Mutex::new(SharedState {
        selection: None,
        annotations: Vec::new(),
        finished: false,
        cancelled: false,
        fullscreen: None,
        pixels_per_point: 1.0,
    }));
    let shared_clone = shared.clone();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "Screenshot Region Select",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                Some(cc),
                output_path_clone,
                shared_clone,
                fullscreen,
                record_mode,
            )))
        }),
    )?;

    let s = shared.lock().unwrap();
    if s.cancelled {
        return Ok(RegionSelectorOutcome::Cancelled);
    }

    if s.finished {
        if record_mode {
            if let Some(sel) = s.selection {
                let scale = s.pixels_per_point;
                let x = (sel.min.x * scale).max(0.0) as i32;
                let y = (sel.min.y * scale).max(0.0) as i32;
                let w = (sel.width() * scale).round() as u32;
                let h = (sel.height() * scale).round() as u32;
                return Ok(RegionSelectorOutcome::Region(x, y, w, h));
            } else {
                return Ok(RegionSelectorOutcome::Fullscreen);
            }
        }

        if let Some(path) = &output_path {
            if let Some(mut img) = s.fullscreen.clone() {
                if let Some(sel) = s.selection {
                    // Scale from egui logical pixels to physical pixels
                    let scale = s.pixels_per_point;
                    let x = (sel.min.x * scale).max(0.0) as u32;
                    let y = (sel.min.y * scale).max(0.0) as u32;
                    let w = (sel.width() * scale).min(img.width() as f32 - x as f32) as u32;
                    let h = (sel.height() * scale).min(img.height() as f32 - y as f32) as u32;
                    // Scale annotations to physical pixels too
                    let scaled_annotations: Vec<Annotation> = s
                        .annotations
                        .iter()
                        .map(|a| match a {
                            Annotation::Rect { x1, y1, x2, y2 } => Annotation::Rect {
                                x1: x1 * scale,
                                y1: y1 * scale,
                                x2: x2 * scale,
                                y2: y2 * scale,
                            },
                            Annotation::Ellipse { x1, y1, x2, y2 } => Annotation::Ellipse {
                                x1: x1 * scale,
                                y1: y1 * scale,
                                x2: x2 * scale,
                                y2: y2 * scale,
                            },
                            Annotation::Circle { cx, cy, r } => Annotation::Circle {
                                cx: cx * scale,
                                cy: cy * scale,
                                r: r * scale,
                            },
                            Annotation::Line { x1, y1, x2, y2 } => Annotation::Line {
                                x1: x1 * scale,
                                y1: y1 * scale,
                                x2: x2 * scale,
                                y2: y2 * scale,
                            },
                            Annotation::Arrow { x1, y1, x2, y2 } => Annotation::Arrow {
                                x1: x1 * scale,
                                y1: y1 * scale,
                                x2: x2 * scale,
                                y2: y2 * scale,
                            },
                            Annotation::Text { x, y, content } => Annotation::Text {
                                x: x * scale,
                                y: y * scale,
                                content: content.clone(),
                            },
                        })
                        .collect();
                    // Use physical-pixel selection rect for annotation offset
                    let phys_sel = egui::Rect::from_min_max(
                        egui::pos2(x as f32, y as f32),
                        egui::pos2((x + w) as f32, (y + h) as f32),
                    );
                    draw_annotations_on_image(&mut img, &scaled_annotations, phys_sel);
                    let mut cropped = image::RgbaImage::new(w, h);
                    for row in 0..h {
                        for col in 0..w {
                            if x + col < img.width() && y + row < img.height() {
                                cropped.put_pixel(
                                    col,
                                    row,
                                    img.get_pixel(x + col, y + row).clone(),
                                );
                            }
                        }
                    }
                    if cropped.save(path).is_ok() {
                        // Copy image data to clipboard (must happen here, not in daemon —
                        // Wayland clipboard requires an active compositor connection)
                        if let Ok(mut clip) = arboard::Clipboard::new() {
                            let img_data = arboard::ImageData {
                                width: cropped.width() as usize,
                                height: cropped.height() as usize,
                                bytes: cropped.as_raw().clone().into(),
                            };
                            if let Err(e) = clip.set_image(img_data) {
                                eprintln!("[region-selector] clipboard copy failed: {}", e);
                            }
                        }
                        return Ok(RegionSelectorOutcome::Saved(path.clone()));
                    }
                }
            }
        }
    }

    Ok(RegionSelectorOutcome::Noop)
}

#[allow(dead_code)]
fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let output_path = if args.len() >= 3 && args[1] == "--output" {
        Some(args[2].clone())
    } else {
        None
    };
    let background_path = if let Some(idx) = args.iter().position(|a| a == "--background") {
        args.get(idx + 1).cloned()
    } else {
        None
    };
    let record_mode = args.contains(&"--record".to_string());

    match run_region_selector(RegionSelectorOptions {
        output_path,
        background_path,
        record_mode,
    })? {
        RegionSelectorOutcome::Cancelled => println!("cancelled"),
        RegionSelectorOutcome::Saved(path) => println!("saved:{path}"),
        RegionSelectorOutcome::Fullscreen => println!("fullscreen"),
        RegionSelectorOutcome::Region(x, y, w, h) => println!("region:{x},{y},{w},{h}"),
        RegionSelectorOutcome::Noop => {}
    }

    Ok(())
}
