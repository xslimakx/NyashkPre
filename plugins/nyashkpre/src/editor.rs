//! NyashkPre GUI: modern analog faceplate (pre-rendered in tools/render_ui.py, see hw.rs).
use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, epaint::PathStroke, Align2, Color32, FontId, Pos2, Stroke},
};
use nyashk_common::ui::{resize_grip, set_b, set_e};
use std::sync::Arc;
use std::time::Duration;

use crate::hw::{self, Hw};
use crate::hw_layout as lay;
use crate::{dsp, Character, Meters, PreParams};

const BASE_W: f32 = lay::W;
const BASE_H: f32 = lay::H;

struct Gui {
    hw: Option<Hw>,
    spec: nyashk_common::analyzer::Spectrum,
    trace: Vec<f32>,
    in_pk: f32,
    out_pk: f32,
    in_hold: f32,
    out_hold: f32,
    color: f32,
}

fn k(id: &str) -> &'static hw::KnobL {
    hw::find(lay::KNOBS, id, |x| x.id)
}
fn b(id: &str) -> &'static hw::BtnL {
    hw::find(lay::BUTTONS, id, |x| x.id)
}
fn sc(id: &str) -> &'static hw::ScreenL {
    hw::find(lay::SCREENS, id, |x| x.id)
}

pub fn create(params: Arc<PreParams>, meters: Arc<Meters>) -> Option<Box<dyn Editor>> {
    let editor_state = params.editor_state.clone();
    let size_state = editor_state.clone();
    create_egui_editor(
        editor_state,
        Gui { hw: None, spec: nyashk_common::analyzer::Spectrum::new(4096), trace: vec![0.0; 1024], in_pk: -90.0, out_pk: -90.0, in_hold: -90.0, out_hold: -90.0, color: -90.0 },
        |ctx, st| {
            st.hw = None;
            let mut v = egui::Visuals::dark();
            v.override_text_color = Some(Color32::from_rgb(236, 238, 240));
            ctx.set_visuals(v);
        },
        move |ctx, setter, st| {
            ctx.request_repaint_after(Duration::from_millis(33));
            if st.hw.is_none() {
                st.hw = Some(Hw::load(ctx, lay::ASSETS));
            }
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
            let accent = hw::c32(lay::ACCENT);
            let ink = Color32::from_rgb(236, 238, 240);
            let dim = Color32::from_rgb(130, 140, 152);

            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(14, 14, 15))).show(ctx, |ui| {
                let full = ui.max_rect();
                st.hw.as_mut().unwrap().o = full.min;
                let hwr = st.hw.as_ref().unwrap();
                let painter = ui.painter().clone();
                hwr.background(&painter, BASE_W, BASE_H);
                let on = p.power.value();
                if hw::button(ui, hwr, b("power"), on, "Switch the preamp on or off (smooth, click-free).") {
                    set_b(setter, &p.power, !on);
                }
                let ph = p.phase.value();
                if hw::button(ui, hwr, b("phase"), ph, "Invert the polarity.") {
                    set_b(setter, &p.phase, !ph);
                }
                let pr = dsp::Params {
                    drive: p.drive.value() / 100.0,
                    ch: p.dsp_char(),
                    lowcut: if p.lowcut.value() <= 20.5 { 0.0 } else { p.lowcut.value() },
                    warmth_db: p.warmth.value(),
                    presence_db: p.presence.value(),
                    silk_db: p.silk.value(),
                    ..Default::default()
                };

                // ---- saturation curve with the live signal on it
                let s1 = sc("sat");
                let tb = hwr.rect(s1.rect).shrink(6.0);
                let grid = Color32::from_rgb(28, 30, 34);
                painter.hline(tb.left()..=tb.right(), tb.center().y, Stroke::new(1.0_f32, grid));
                painter.vline(tb.center().x, tb.top()..=tb.bottom(), Stroke::new(1.0_f32, grid));
                painter.line_segment([tb.left_bottom(), tb.right_top()], Stroke::new(1.0_f32, Color32::from_rgb(52, 54, 58)));
                let range = 1.5f32;
                let pts: Vec<Pos2> = (0..=120)
                    .map(|i| {
                        let v = -range + 2.0 * range * i as f32 / 120.0;
                        let y = dsp::shaper(pr.ch, pr.drive, v).clamp(-range, range);
                        Pos2::new(tb.center().x + v / range * tb.width() * 0.5, tb.center().y - y / range * tb.height() * 0.5)
                    })
                    .collect();
                painter.with_clip_rect(tb).add(egui::Shape::line(pts, PathStroke::new(2.2_f32, if on { accent } else { dim })));
                if on {
                    let g_in = 10f32.powf(p.gain.value() / 20.0);
                    let clip = painter.with_clip_rect(tb);
                    let n = st.trace.len();
                    for (i, s) in st.trace.iter().enumerate().step_by(2) {
                        let v = (s * g_in).clamp(-range, range);
                        let y = dsp::shaper(pr.ch, pr.drive, v).clamp(-range, range);
                        let a = (30.0 + 180.0 * i as f32 / n as f32) as u8;
                        let pos = Pos2::new(tb.center().x + v / range * tb.width() * 0.5, tb.center().y - y / range * tb.height() * 0.5);
                        clip.circle_filled(pos, 1.5, Color32::from_rgba_unmultiplied(255, 236, 220, a));
                    }
                }
                hw::glass(&painter, hwr, s1);

                // ---- tone response over the live spectrum
                let s2 = sc("tone");
                let tg = hwr.rect(s2.rect).shrink(6.0);
                let (f_lo, f_hi, d_r) = (20.0f32, 20000.0f32, 9.0f32);
                let fx = |f: f32| tg.left() + tg.width() * (f / f_lo).ln() / (f_hi / f_lo).ln();
                let dy = |d: f32| tg.center().y - tg.height() * 0.5 * (d / d_r).clamp(-1.0, 1.0);
                for d in [-6.0f32, -3.0, 0.0, 3.0, 6.0] {
                    painter.hline(tg.left()..=tg.right(), dy(d), Stroke::new(1.0_f32, if d == 0.0 { Color32::from_rgb(52, 54, 58) } else { grid }));
                }
                for f in [100.0f32, 1000.0, 10000.0] {
                    painter.vline(fx(f), tg.top()..=tg.bottom(), Stroke::new(1.0_f32, grid));
                    painter.text(Pos2::new(fx(f) + 3.0, tg.bottom() - 7.0), Align2::LEFT_CENTER, if f >= 1000.0 { format!("{}k", f / 1000.0) } else { format!("{}", f) }, FontId::monospace(8.5), dim);
                }
                let spy = |d: f32| tg.bottom() - tg.height() * ((d + 90.0) / 90.0).clamp(-0.05, 1.05);
                let sp = st.spec.points(fs, f_lo, f_hi.min(fs * 0.49), 160, 4.5, fx, spy);
                nyashk_common::analyzer::draw_spectrum(&painter, tg, &sp, Color32::from_rgb(26, 30, 38), Color32::from_rgb(70, 78, 92));
                let tone = dsp::Tone::new(fs, &pr);
                let cpts: Vec<Pos2> = (0..=200)
                    .map(|i| {
                        let f = f_lo * (f_hi / f_lo).powf(i as f32 / 200.0);
                        Pos2::new(fx(f), dy(if on { tone.response_db(fs, f) } else { 0.0 }))
                    })
                    .collect();
                painter.with_clip_rect(tg).add(egui::Shape::line(cpts, PathStroke::new(2.0_f32, accent)));
                let col = if on && st.color > -70.0 { format!("COLOR {:.0} dB", st.color.round() + 0.0) } else { "COLOR -- dB".into() };
                painter.text(Pos2::new(tg.right() - 4.0, tg.top() + 8.0), Align2::RIGHT_CENTER, col, FontId::monospace(9.5), ink)
                    ;
                hw::glass(&painter, hwr, s2);

                // ---- meters (-48..0 dBFS)
                let lv = |d: f32| ((d + 48.0) / 48.0).clamp(0.0, 1.0);
                hw::ladder(&painter, hwr, hw::find(lay::LADDERS, "in", |x| x.id), lv(st.in_pk), false);
                hw::ladder(&painter, hwr, hw::find(lay::LADDERS, "out", |x| x.id), lv(st.out_pk), false);
                hw::ladder_hold(&painter, hwr, hw::find(lay::LADDERS, "in", |x| x.id), lv(st.in_hold));
                hw::ladder_hold(&painter, hwr, hw::find(lay::LADDERS, "out", |x| x.id), lv(st.out_hold));

                // ---- controls
                hw::knob(ui, hwr, setter, &p.gain, k("gain"), accent, "GAIN: input gain in dB. Pushes the signal into the saturation stage, like turning up a mic pre.");
                hw::knob(ui, hwr, setter, &p.drive, k("drive"), accent, "DRIVE: how strongly the stage saturates. 0 = clean and level-neutral.");
                for (id, c, tip) in [
                    ("tube", Character::Tube, "TUBE: warm, smooth, adds 2nd harmonic (the classic vocal sweetener)."),
                    ("iron", Character::Iron, "IRON: transformer weight, the low end saturates first. Dense and full."),
                    ("clean", Character::Clean, "CLEAN: tight, transparent solid-state; only adds odd harmonics when pushed hard."),
                ] {
                    if hw::button(ui, hwr, b(id), p.character.value() == c, tip) {
                        set_e(setter, &p.character, c);
                    }
                }
                hw::knob(ui, hwr, setter, &p.lowcut, k("lowcut"), accent, "LOW CUT: high-pass against rumble and proximity boom, 20 to 300 Hz. Far left = off.");
                hw::knob(ui, hwr, setter, &p.warmth, k("warmth"), accent, "WARMTH: low shelf at 120 Hz, +/- 6 dB: body and weight.");
                hw::knob(ui, hwr, setter, &p.presence, k("presence"), accent, "PRESENCE: bell at 3.5 kHz, +/- 6 dB: intelligibility and forwardness.");
                hw::knob(ui, hwr, setter, &p.silk, k("silk"), accent, "SILK: high shelf at 10 kHz, +/- 6 dB: smooth air on the top.");
                hw::knob(ui, hwr, setter, &p.mix, k("mix"), accent, "MIX: parallel blend, 0 = dry, 100 = fully processed.");
                hw::knob(ui, hwr, setter, &p.out, k("out"), accent, "OUTPUT: output level in dB.");
                let au = p.auto_level.value();
                if hw::button(ui, hwr, b("auto"), au, "AUTO LEVEL: turns the output down as you turn up GAIN, so you judge the sound and not the loudness.") {
                    set_b(setter, &p.auto_level, !au);
                }
                resize_grip(ui, ctx, &size_state, full, BASE_W, BASE_H, 520.0);
            });
        },
    )
}
