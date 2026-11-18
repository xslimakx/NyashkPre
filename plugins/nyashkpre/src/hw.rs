//! Photo-real hardware widgets drawn over the pre-rendered faceplate (tools/render_ui.py).
//! The background picture carries everything that never moves (chassis, print, knob bodies,
//! buttons in their off state, meter faces); here only the moving parts are drawn: knob pointers,
//! LED rings, lit buttons, needles, LED bars and the glass over screens.
#![allow(dead_code)]
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Align2, Color32, ColorImage, FontId, Painter, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2};
use std::collections::HashMap;

pub struct KnobL {
    pub id: &'static str,
    pub c: (f32, f32),
    pub r: f32,
    pub ptr0: f32,
    pub ptr1: f32,
    pub ptr_w: f32,
    pub ptr_col: [u8; 4],
    pub engraved: bool,
    pub skirt: (f32, f32),
    pub led_r: f32,
    pub led_n: u32,
    pub bip: bool,
}
pub struct BtnL {
    pub id: &'static str,
    pub rect: (f32, f32, f32, f32),
    pub tex: &'static str,
    pub tex_size: (f32, f32),
}
pub struct ScreenL {
    pub id: &'static str,
    pub rect: (f32, f32, f32, f32),
    pub glass: &'static str,
}
pub struct VuL {
    pub id: &'static str,
    pub rect: (f32, f32, f32, f32),
    pub pivot: (f32, f32),
    pub r: f32,
    pub glass: &'static str,
}
pub struct LadderL {
    pub id: &'static str,
    pub rect: (f32, f32, f32, f32),
    pub n: u32,
    pub colors: &'static [(f32, [u8; 3])],
}
pub struct TextL {
    pub id: &'static str,
    pub pos: (f32, f32),
    pub anchor: &'static str,
    pub size: f32,
    pub col: [u8; 3],
}

const UV: Rect = Rect { min: Pos2 { x: 0.0, y: 0.0 }, max: Pos2 { x: 1.0, y: 1.0 } };

fn placeholder() -> ColorImage {
    ColorImage::new([4, 4], Color32::from_gray(40))
}
pub fn jpeg(data: &[u8]) -> ColorImage {
    let mut dec = zune_jpeg::JpegDecoder::new(data);
    if let Ok(px) = dec.decode() {
        if let Some((w, h)) = dec.dimensions() {
            if px.len() == w * h * 3 {
                return ColorImage::from_rgb([w, h], &px);
            }
        }
    }
    placeholder()
}
pub fn png(data: &[u8]) -> ColorImage {
    let mut dec = zune_png::PngDecoder::new(data);
    if let Ok(px) = dec.decode_raw() {
        if let Some((w, h)) = dec.get_dimensions() {
            if px.len() == w * h * 4 {
                return ColorImage::from_rgba_unmultiplied([w, h], &px);
            }
            if px.len() == w * h * 3 {
                return ColorImage::from_rgb([w, h], &px);
            }
        }
    }
    placeholder()
}

/// All textures of one faceplate, plus its origin / scale on screen.
pub struct Hw {
    pub tex: HashMap<&'static str, TextureHandle>,
    pub o: Pos2,
}
impl Hw {
    pub fn load(ctx: &egui::Context, assets: &[(&'static str, &'static [u8])]) -> Self {
        let opt = TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: Some(egui::TextureFilter::Linear),
        };
        let mut tex = HashMap::new();
        for (name, data) in assets {
            let img = if data.starts_with(&[0xff, 0xd8]) { jpeg(data) } else { png(data) };
            tex.insert(*name, ctx.load_texture(*name, img, opt));
        }
        Self { tex, o: Pos2::ZERO }
    }
    pub fn at(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(self.o.x + x, self.o.y + y)
    }
    pub fn rect(&self, r: (f32, f32, f32, f32)) -> Rect {
        Rect::from_min_size(self.at(r.0, r.1), Vec2::new(r.2, r.3))
    }
    pub fn image(&self, p: &Painter, name: &str, r: Rect, tint: Color32) {
        if let Some(t) = self.tex.get(name) {
            p.image(t.id(), r, UV, tint);
        }
    }
    pub fn background(&self, p: &Painter, w: f32, h: f32) {
        self.image(p, "bg", Rect::from_min_size(self.o, Vec2::new(w, h)), Color32::WHITE);
    }
    /// soft glowing LED dot (glow sprite + bright core)
    pub fn led(&self, p: &Painter, c: Pos2, col: Color32, size: f32, level: f32) {
        if level <= 0.01 {
            return;
        }
        let g = Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), (90.0 * level) as u8);
        self.image(p, "glow", Rect::from_center_size(c, Vec2::splat(size * 4.0)), g);
        p.circle_filled(c, size * 0.55, col.gamma_multiply(0.6 + 0.4 * level));
        p.circle_filled(c + Vec2::new(-size * 0.12, -size * 0.15), size * 0.25, Color32::from_rgba_unmultiplied(255, 255, 240, (160.0 * level) as u8));
    }
}

pub fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Knob: drag (shift = fine), wheel, double-click = default, right-click = type a value.
/// Draws the pointer and LED ring; the value pops up while hovered or turned.
/// Returns how much the knob moved this frame (normalized), 0 when untouched.
pub fn knob<P: Param>(ui: &mut Ui, hw: &Hw, setter: &ParamSetter, p: &P, k: &KnobL, accent: Color32, tip: &str) -> f32 {
    let c = hw.at(k.c.0, k.c.1);
    let hit = Rect::from_center_size(c, Vec2::splat(k.r * 2.0 + 6.0));
    let id = egui::Id::new(("hwknob", k.id));
    let resp = ui.interact(hit, id, Sense::click_and_drag());
    let norm = p.unmodulated_normalized_value();
    let norm0 = norm;
    let raw_id = id.with("raw");
    let mut changed = false;
    let mut newv = norm;
    if resp.drag_started() {
        setter.begin_set_parameter(p);
        ui.data_mut(|d| d.insert_temp(raw_id, norm));
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let d = -resp.drag_delta().y / if fine { 1200.0 } else { 220.0 };
        let raw = ui.data(|dd| dd.get_temp::<f32>(raw_id)).unwrap_or(norm) + d;
        ui.data_mut(|dd| dd.insert_temp(raw_id, raw));
        newv = raw.clamp(0.0, 1.0);
        setter.set_parameter_normalized(p, newv);
        changed = true;
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    if resp.double_clicked() {
        setter.begin_set_parameter(p);
        newv = p.default_normalized_value();
        setter.set_parameter_normalized(p, newv);
        setter.end_set_parameter(p);
        changed = true;
    }
    if resp.hovered() {
        let sc = ui.input(|i| i.smooth_scroll_delta.y);
        if sc != 0.0 {
            setter.begin_set_parameter(p);
            newv = (norm + sc / 1500.0).clamp(0.0, 1.0);
            setter.set_parameter_normalized(p, newv);
            setter.end_set_parameter(p);
            changed = true;
        }
    }
    let edit_id = id.with("edit");
    if resp.secondary_clicked() {
        let v = p.normalized_value_to_string(p.unmodulated_normalized_value(), false);
        ui.data_mut(|d| d.insert_temp(edit_id, v));
    }
    let norm = if changed { newv } else { p.unmodulated_normalized_value() };
    let painter = ui.painter();
    // LED ring
    if k.led_n > 1 {
        let from = if k.bip { 0.5 } else { 0.0 };
        let (lo, hi) = if norm < from { (norm, from) } else { (from, norm) };
        for i in 0..k.led_n {
            let t = i as f32 / (k.led_n - 1) as f32;
            let a = (-135.0 + 270.0 * t).to_radians();
            let pt = c + Vec2::new(k.led_r * a.sin(), -k.led_r * a.cos());
            let step = 0.5 / (k.led_n - 1) as f32;
            let lvl = if t >= lo - step && t <= hi + step {
                // partial brightness at the moving end, so the ring glides instead of jumping
                let d = if t < lo { (lo - t) / step } else if t > hi { (t - hi) / step } else { 0.0 };
                (1.0 - d).clamp(0.0, 1.0)
            } else {
                0.0
            };
            hw.led(painter, pt, accent, 2.8, lvl);
        }
    }
    // hover / grab: a faint rim of light around the knob
    if resp.hovered() || resp.dragged() {
        let al = if resp.dragged() { 70 } else { 40 };
        painter.circle_stroke(c, k.r * 0.86, Stroke::new(1.2_f32, Color32::from_white_alpha(al)));
    }
    // pointer
    let a = (-135.0 + 270.0 * norm).to_radians();
    let dir = Vec2::new(a.sin(), -a.cos());
    let col = Color32::from_rgba_unmultiplied(k.ptr_col[0], k.ptr_col[1], k.ptr_col[2], k.ptr_col[3]);
    let seg = |r0: f32, r1: f32| [c + dir * r0, c + dir * r1];
    if k.engraved {
        // a groove: dark line with a light lip below it
        painter.line_segment(seg(k.ptr0, k.ptr1).map(|p| p + Vec2::new(0.5, 0.8)), Stroke::new(k.ptr_w, Color32::from_rgba_unmultiplied(255, 255, 255, 70)));
        painter.line_segment(seg(k.ptr0, k.ptr1), Stroke::new(k.ptr_w, col));
    } else {
        painter.line_segment(seg(k.ptr0, k.ptr1).map(|p| p + Vec2::new(0.4, 0.9)), Stroke::new(k.ptr_w + 0.6, Color32::from_rgba_unmultiplied(0, 0, 0, 90)));
        painter.line_segment(seg(k.ptr0, k.ptr1), Stroke::new(k.ptr_w, col));
        if k.skirt.1 > 0.0 {
            painter.line_segment(seg(k.skirt.0, k.skirt.1), Stroke::new(k.ptr_w * 0.9, Color32::from_rgb(25, 25, 25)));
        }
    }
    // value: popup while hovered / dragged, text field on right-click
    let editing: Option<String> = ui.data(|d| d.get_temp::<String>(edit_id));
    let vr = Rect::from_center_size(c + Vec2::new(0.0, k.r + 12.0), Vec2::new(84.0, 18.0));
    if let Some(mut buf) = editing {
        let te = egui::TextEdit::singleline(&mut buf).font(FontId::monospace(11.0)).horizontal_align(egui::Align::Center).desired_width(vr.width());
        let r_te = ui.put(vr, te);
        if !r_te.has_focus() && !r_te.lost_focus() {
            r_te.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else if enter || r_te.lost_focus() {
            if let Some(nv) = p.string_to_normalized_value(buf.trim().replace(',', ".").as_str()) {
                setter.begin_set_parameter(p);
                newv = nv.clamp(0.0, 1.0);
                setter.set_parameter_normalized(p, newv);
                setter.end_set_parameter(p);
                changed = true;
            }
            ui.data_mut(|d| d.remove::<String>(edit_id));
        } else {
            ui.data_mut(|d| d.insert_temp(edit_id, buf));
        }
    } else if resp.hovered() || resp.dragged() {
        let txt = p.normalized_value_to_string(norm, true);
        bubble(ui.painter(), vr.center(), &txt);
        if !tip.is_empty() && !resp.dragged() {
            resp.on_hover_text(tip);
        }
    }
    if changed { newv - norm0 } else { 0.0 }
}

/// small dark value bubble
pub fn bubble(p: &Painter, c: Pos2, txt: &str) {
    let font = FontId::monospace(11.0);
    let g = p.layout_no_wrap(txt.to_string(), font, Color32::WHITE);
    let r = Rect::from_center_size(c, g.size() + Vec2::new(12.0, 6.0));
    p.rect_filled(r.translate(Vec2::new(0.0, 1.5)), 4.0, Color32::from_black_alpha(90));
    p.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(12, 12, 14, 235));
    p.rect_stroke(r, 4.0, Stroke::new(1.0_f32, Color32::from_white_alpha(30)), egui::StrokeKind::Inside);
    p.galley(r.center() - g.size() * 0.5, g, Color32::WHITE);
}

/// Push button: the lit picture when `on`; returns true when clicked.
pub fn button(ui: &mut Ui, hw: &Hw, b: &BtnL, on: bool, tip: &str) -> bool {
    let r = hw.rect(b.rect);
    let resp = ui.interact(r, egui::Id::new(("hwbtn", b.id)), Sense::click());
    let p = ui.painter();
    let pressed = resp.is_pointer_button_down_on();
    if on {
        let tr = Rect::from_center_size(r.center() + Vec2::new(0.0, if pressed { 0.6 } else { 0.0 }), Vec2::new(b.tex_size.0, b.tex_size.1));
        hw.image(p, b.tex, tr, Color32::WHITE);
    }
    if pressed {
        p.rect_filled(r.shrink(1.0), 4.0, Color32::from_black_alpha(50));
    } else if resp.hovered() {
        p.rect_filled(r.shrink(1.0), 4.0, Color32::from_white_alpha(10));
    }
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    clicked
}

/// Needle on a VU face; `t` is 0..1 across the scale (-48..+48 degrees).
pub fn needle(p: &Painter, hw: &Hw, v: &VuL, t: f32, col: Color32) {
    let clip = p.with_clip_rect(hw.rect(v.rect).shrink(2.0));
    let pv = hw.at(v.pivot.0, v.pivot.1);
    let a = (-48.0 + 96.0 * t.clamp(-0.03, 1.03)).to_radians();
    let dir = Vec2::new(a.sin(), -a.cos());
    let (r0, r1) = (v.r * 0.22, v.r + 6.0);
    clip.line_segment([pv + dir * r0 + Vec2::new(3.0, 4.0), pv + dir * r1 + Vec2::new(3.0, 4.0)], Stroke::new(2.2_f32, Color32::from_black_alpha(110)));
    clip.line_segment([pv + dir * r0, pv + dir * r1], Stroke::new(1.6_f32, col));
    hw.image(p, v.glass, hw.rect(v.rect), Color32::WHITE);
}

/// VU deflection for a dB value on a classic VU scale (-20..+3 VU)
pub fn vu_t(db: f32) -> f32 {
    let lo = 10f32.powf(-20.0 / 20.0);
    let hi = 10f32.powf(3.0 / 20.0);
    ((10f32.powf(db.clamp(-40.0, 6.0) / 20.0) - lo) / (hi - lo)).clamp(-0.04, 1.04)
}

/// LED bar: `level` 0..1 lights segments from the bottom (or from the top when `down`).
pub fn ladder(p: &Painter, hw: &Hw, l: &LadderL, level: f32, down: bool) {
    let r = hw.rect(l.rect);
    let seg = r.height() / l.n as f32;
    let lit = level.clamp(0.0, 1.0) * l.n as f32;
    for k in 0..l.n {
        let on = if down { ((l.n - 1 - k) as f32) < lit } else { (k as f32) < lit };
        if !on {
            continue;
        }
        let t = k as f32 / l.n as f32;
        let col = l.colors.iter().rev().find(|(f, _)| t >= *f).map(|(_, c)| c32(*c)).unwrap_or(Color32::GREEN);
        let y1 = r.bottom() - k as f32 * seg - 1.2;
        let sr = Rect::from_min_max(Pos2::new(r.left() + 2.0, y1 - seg + 2.2), Pos2::new(r.right() - 2.0, y1));
        hw.image(p, "glow", sr.expand(5.0), Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 70));
        p.rect_filled(sr, 1.0, col);
    }
}

/// Peak-hold segment on an LED bar (0..1).
pub fn ladder_hold(p: &Painter, hw: &Hw, l: &LadderL, hold: f32) {
    if hold <= 0.02 {
        return;
    }
    let r = hw.rect(l.rect);
    let seg = r.height() / l.n as f32;
    let k = ((hold.clamp(0.0, 1.0) * l.n as f32).ceil() as u32).clamp(1, l.n) - 1;
    let t = k as f32 / l.n as f32;
    let col = l.colors.iter().rev().find(|(f, _)| t >= *f).map(|(_, c)| c32(*c)).unwrap_or(Color32::GREEN);
    let y1 = r.bottom() - k as f32 * seg - 1.2;
    let sr = Rect::from_min_max(Pos2::new(r.left() + 2.0, y1 - seg + 2.2), Pos2::new(r.right() - 2.0, y1));
    p.rect_filled(sr, 1.0, col.gamma_multiply(0.8));
}

/// Glass over a screen (after drawing its content).
pub fn glass(p: &Painter, hw: &Hw, s: &ScreenL) {
    hw.image(p, s.glass, hw.rect(s.rect), Color32::WHITE);
}

pub fn text(p: &Painter, hw: &Hw, t: &TextL, s: &str) {
    let al = match t.anchor {
        "lm" => Align2::LEFT_CENTER,
        "rm" => Align2::RIGHT_CENTER,
        _ => Align2::CENTER_CENTER,
    };
    p.text(hw.at(t.pos.0, t.pos.1), al, s, FontId::proportional(t.size), c32(t.col));
}

pub fn find<'a, T>(list: &'a [T], id: &str, key: impl Fn(&T) -> &str) -> &'a T {
    list.iter().find(|x| key(x) == id).unwrap_or_else(|| panic!("hw layout: no element {id}"))
}
