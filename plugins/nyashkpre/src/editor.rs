//! NyashkPre GUI.
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, epaint::PathStroke, Align2, Color32, Pos2, Rect, Stroke, Vec2},
};
use nyashk_common::ui::*;
use std::sync::Arc;
use std::time::Duration;

use crate::{dsp, Character, Meters, PreParams};

const BASE_W: f32 = 700.0;
const BASE_H: f32 = 450.0;

struct Gui {
    spec: nyashk_common::analyzer::Spectrum,
    trace: Vec<f32>,
    in_pk: f32,
    out_pk: f32,
    in_hold: f32,
    out_hold: f32,
    color: f32,
}

pub fn create(params: Arc<PreParams>, meters: Arc<Meters>) -> Option<Box<dyn Editor>> {
    let editor_state = params.editor_state.clone();
    let size_state = editor_state.clone();
    create_egui_editor(
        editor_state,
        Gui { spec: nyashk_common::analyzer::Spectrum::new(4096), trace: vec![0.0; 1024], in_pk: -90.0, out_pk: -90.0, in_hold: -90.0, out_hold: -90.0, color: -90.0 },
        |ctx, _| apply_theme(ctx),
        move |ctx, setter, st| {
            ctx.request_repaint_after(Duration::from_millis(33));
            let p = &*params;
            let fs = Meters::get(&meters.fs).max(8000.0);
            let fall = |cur: f32, t: f32| if t > cur { t } else { cur - 1.2 };
            st.in_pk = fall(st.in_pk, Meters::get(&meters.in_db));
            st.out_pk = fall(st.out_pk, Meters::get(&meters.out_db));
            st.in_hold = st.in_hold.max(st.in_pk) - 0.12;
            st.out_hold = st.out_hold.max(st.out_pk) - 0.12;
            st.spec.update(&meters.tap_out);
            meters.tap_in.snapshot(&mut st.trace);
            let c = Meters::get(&meters.color_db);
            st.color += (c - st.color) * 0.2;

            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG)).show(ctx, |ui| {
                let full = ui.max_rect();
                let o = full.min;
                let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(o.x + x, o.y + y), Vec2::new(w, h));
                header(ui, o, BASE_W, "Pre", "VOCAL PREAMP / SATURATOR");
                let on = p.power.value();
                if pill_tip(ui, rect(BASE_W - 14.0 - 76.0, 22.0, 76.0, 24.0), "POWER", on, "Switch the preamp on or off (smooth, click-free).") {
                    set_b(setter, &p.power, !on);
                }
                let ph = p.phase.value();
                if pill_tip(ui, rect(BASE_W - 14.0 - 76.0 - 56.0, 22.0, 50.0, 24.0), "Ø", ph, "Invert the polarity.") {
                    set_b(setter, &p.phase, !ph);
                }

                // ---------------- displays
                let dp = rect(14.0, 70.0, BASE_W - 28.0, 178.0);
                panel(ui, dp);
                let painter = ui.painter().clone();
                let pr = dsp::Params {
                    drive: p.drive.value() / 100.0,
                    ch: p.dsp_char(),
                    lowcut: if p.lowcut.value() <= 20.5 { 0.0 } else { p.lowcut.value() },
                    warmth_db: p.warmth.value(),
                    presence_db: p.presence.value(),
                    silk_db: p.silk.value(),
                    ..Default::default()
                };
                // transfer curve
                let tb = Rect::from_min_size(Pos2::new(dp.left() + 14.0, dp.top() + 14.0), Vec2::splat(dp.height() - 28.0));
                painter.rect_filled(tb, 4.0, Color32::from_rgb(0x07, 0x07, 0x07));
                painter.hline(tb.left()..=tb.right(), tb.center().y, Stroke::new(1.0, LINE));
                painter.vline(tb.center().x, tb.top()..=tb.bottom(), Stroke::new(1.0, LINE));
                painter.line_segment([tb.left_bottom(), tb.right_top()], Stroke::new(1.0, Color32::from_rgb(0x34, 0x34, 0x34)));
                let range = 1.5f32;
                let pts: Vec<Pos2> = (0..=120)
                    .map(|i| {
                        let v = -range + 2.0 * range * i as f32 / 120.0;
                        let y = dsp::shaper(pr.ch, pr.drive, v).clamp(-range, range);
                        Pos2::new(tb.center().x + v / range * tb.width() * 0.5, tb.center().y - y / range * tb.height() * 0.5)
                    })
                    .collect();
                painter.with_clip_rect(tb).add(egui::Shape::line(pts, PathStroke::new(2.2, if on { WHITE } else { DIM })));
                // live trace: where the signal sits on the curve right now (newest samples brightest)
                if on {
                    let g_in = 10f32.powf(p.gain.value() / 20.0);
                    let clip = painter.with_clip_rect(tb);
                    let n = st.trace.len();
                    for (i, s) in st.trace.iter().enumerate().step_by(2) {
                        let v = (s * g_in).clamp(-range, range);
                        let y = dsp::shaper(pr.ch, pr.drive, v).clamp(-range, range);
                        let a = (40.0 + 200.0 * i as f32 / n as f32) as u8;
                        let pos = Pos2::new(tb.center().x + v / range * tb.width() * 0.5, tb.center().y - y / range * tb.height() * 0.5);
                        clip.circle_filled(pos, 1.6, Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, a));
                    }
                }
                painter.text(Pos2::new(tb.left() + 6.0, tb.top() + 8.0), Align2::LEFT_CENTER, "SATURATION CURVE", mono(9.0), DIM);
                painter.text(Pos2::new(tb.center().x, tb.bottom() + 0.0), Align2::CENTER_BOTTOM, "in", mono(9.0), DIM);

                // tone response
                let tg = Rect::from_min_max(Pos2::new(tb.right() + 52.0, dp.top() + 14.0), Pos2::new(dp.right() - 130.0, dp.bottom() - 28.0));
                painter.rect_filled(tg, 4.0, Color32::from_rgb(0x07, 0x07, 0x07));
                let (f_lo, f_hi, d_r) = (20.0f32, 20000.0f32, 9.0f32);
                let fx = |f: f32| tg.left() + tg.width() * (f / f_lo).ln() / (f_hi / f_lo).ln();
                let dy = |d: f32| tg.center().y - tg.height() * 0.5 * (d / d_r).clamp(-1.0, 1.0);
                for d in [-6.0f32, -3.0, 0.0, 3.0, 6.0] {
                    painter.hline(tg.left()..=tg.right(), dy(d), Stroke::new(1.0, if d == 0.0 { MID } else { LINE }));
                    painter.text(Pos2::new(tg.left() - 5.0, dy(d)), Align2::RIGHT_CENTER, format!("{:+.0}", d), mono(9.0), DIM);
                }
                for f in [50.0f32, 100.0, 500.0, 1000.0, 5000.0, 10000.0] {
                    painter.vline(fx(f), tg.top()..=tg.bottom(), Stroke::new(1.0, LINE));
                    painter.text(Pos2::new(fx(f), tg.bottom() + 10.0), Align2::CENTER_CENTER, if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{}", f) }, mono(9.0), DIM);
                }
                // live output spectrum behind the tone curve
                let spy = |d: f32| tg.bottom() - tg.height() * ((d + 90.0) / 90.0).clamp(-0.05, 1.05);
                let sp = st.spec.points(fs, f_lo, f_hi.min(fs * 0.49), 160, 4.5, fx, spy);
                nyashk_common::analyzer::draw_spectrum(&painter, tg, &sp, Color32::from_gray(26), Color32::from_gray(72));
                for d in [-6.0f32, -3.0, 3.0, 6.0] {
                    painter.with_clip_rect(tg).hline(tg.left()..=tg.right(), dy(d), Stroke::new(1.0, Color32::from_white_alpha(6)));
                }
                let tone = dsp::Tone::new(fs, &pr);
                let cpts: Vec<Pos2> = (0..=200)
                    .map(|i| {
                        let f = f_lo * (f_hi / f_lo).powf(i as f32 / 200.0);
                        Pos2::new(fx(f), dy(if on { tone.response_db(fs, f) } else { 0.0 }))
                    })
                    .collect();
                painter.with_clip_rect(tg).add(egui::Shape::line(cpts, PathStroke::new(2.0, WHITE)));
                painter.text(Pos2::new(tg.left() + 8.0, tg.top() + 8.0), Align2::LEFT_CENTER, "TONE  ·  SPECTRUM", mono(9.0), DIM);

                // meters
                let mx = dp.right() - 108.0;
                let bt = dp.top() + 16.0;
                let bh = dp.height() - 52.0;
                for (k, (name, lvl, hold)) in [("IN", st.in_pk, st.in_hold), ("OUT", st.out_pk, st.out_hold)].into_iter().enumerate() {
                    let r = Rect::from_min_size(Pos2::new(mx + 36.0 * k as f32, bt), Vec2::new(14.0, bh));
                    vbar(ui, r, lvl, hold, -48.0, 0.0);
                    painter.text(Pos2::new(r.center().x, r.bottom() + 11.0), Align2::CENTER_CENTER, name, mono(9.5), DIM);
                }
                painter.text(Pos2::new(dp.right() - 14.0, bt + 8.0), Align2::RIGHT_CENTER, "COLOR", mono(9.0), DIM);
                let col = if on && st.color > -70.0 { format!("{:.0} dB", st.color.round() + 0.0) } else { "-- dB".into() };
                painter.text(Pos2::new(dp.right() - 14.0, bt + 26.0), Align2::RIGHT_CENTER, col, mono(11.0), WHITE);
                painter.text(Pos2::new(dp.right() - 14.0, bt + 46.0), Align2::RIGHT_CENTER, "harmonics", mono(9.0), DIM);
                painter.text(Pos2::new(dp.right() - 14.0, bt + 58.0), Align2::RIGHT_CENTER, "vs signal", mono(9.0), DIM);

                // ---------------- controls
                let kp = rect(14.0, 260.0, BASE_W - 28.0, 178.0);
                panel(ui, kp);
                let cw = 70.0;
                let r = 26.0;
                let top = kp.top() + 34.0;
                let cell = |x: f32| Rect::from_min_size(Pos2::new(o.x + x, top), Vec2::new(cw, 100.0));
                let title = |t: &str, x0: f32, w: f32| {
                    painter.text(Pos2::new(o.x + x0 + w * 0.5, kp.top() + 14.0), Align2::CENTER_CENTER, t, mono(10.5), WHITE);
                    painter.hline(o.x + x0 + 4.0..=o.x + x0 + w - 4.0, kp.top() + 24.0, Stroke::new(1.0, LINE));
                };
                let mut x = 22.0;
                title("PREAMP", x, cw * 2.0);
                knob_r(ui, setter, &p.gain, "GAIN", cell(x), "Input gain in dB. Pushes the signal into the saturation stage, like turning up a mic pre.", false, r);
                knob_r(ui, setter, &p.drive, "DRIVE", cell(x + cw), "How strongly the stage saturates. 0 = clean and level-neutral.", false, r);
                x += cw * 2.0 + 10.0;
                // character pills
                painter.text(Pos2::new(o.x + x + 40.0, kp.top() + 14.0), Align2::CENTER_CENTER, "CHARACTER", mono(10.5), WHITE);
                painter.hline(o.x + x + 4.0..=o.x + x + 76.0, kp.top() + 24.0, Stroke::new(1.0, LINE));
                for (i, (c, tip)) in [
                    (Character::Tube, "TUBE: warm, smooth, adds 2nd harmonic (the classic vocal sweetener)."),
                    (Character::Iron, "IRON: transformer weight, the low end saturates first. Dense and full."),
                    (Character::Clean, "CLEAN: tight, transparent solid-state; only adds odd harmonics when pushed hard."),
                ]
                .into_iter()
                .enumerate()
                {
                    if pill_tip(ui, Rect::from_min_size(Pos2::new(o.x + x + 4.0, top + 6.0 + 34.0 * i as f32), Vec2::new(72.0, 26.0)), ["TUBE", "IRON", "CLEAN"][i], p.character.value() == c, tip) {
                        set_e(setter, &p.character, c);
                    }
                }
                x += 80.0 + 10.0;
                title("TONE", x, cw * 4.0);
                knob_r(ui, setter, &p.lowcut, "LOW CUT", cell(x), "High-pass filter against rumble and proximity boom, 20 to 300 Hz. At the far left it is off.", false, r);
                knob_ac(ui, setter, &p.warmth, "WARMTH", cell(x + cw), "Low shelf at 120 Hz, +/- 6 dB: body and weight.", true, r, C_LF);
                knob_ac(ui, setter, &p.presence, "PRESENCE", cell(x + cw * 2.0), "Bell at 3.5 kHz, +/- 6 dB: intelligibility and forwardness.", true, r, C_HMF);
                knob_ac(ui, setter, &p.silk, "SILK", cell(x + cw * 3.0), "High shelf at 10 kHz, +/- 6 dB: smooth air on the top.", true, r, C_HF);
                x += cw * 4.0 + 10.0;
                title("OUT", x, cw * 2.0 - 20.0);
                knob_r(ui, setter, &p.mix, "MIX", cell(x - 6.0), "Parallel blend: 0 = dry, 100 = fully processed.", false, r);
                knob_r(ui, setter, &p.out, "OUTPUT", cell(x - 6.0 + cw - 4.0), "Output level in dB.", true, r);
                let au = p.auto_level.value();
                if pill_tip(ui, Rect::from_min_size(Pos2::new(o.x + 22.0, kp.bottom() - 34.0), Vec2::new(128.0, 24.0)), "AUTO LEVEL", au, "Turns the output down as you turn up GAIN, so you judge the sound and not the loudness.") {
                    set_b(setter, &p.auto_level, !au);
                }

                resize_grip(ui, ctx, &size_state, full, BASE_W, BASE_H, 520.0);
            });
        },
    )
}
