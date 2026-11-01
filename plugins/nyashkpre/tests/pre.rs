use nyashkpre::dsp::*;

const FS: f32 = 48000.0;

fn run(p: &Params, f: f32, amp: f32) -> (Vec<f32>, Vec<f32>) {
    let mut pre = Pre::new(FS);
    let (mut i, mut o) = (vec![], vec![]);
    for n in 0..48000 {
        let s = amp * (2.0 * std::f32::consts::PI * f * n as f32 / FS).sin();
        let mut x = [s, s];
        pre.process(p, &mut x, 2);
        assert!(x[0].is_finite());
        if n >= 24000 {
            i.push(s);
            o.push(x[0]);
        }
    }
    (i, o)
}

/// Amplitude of frequency `f` (single-bin DFT over whole periods).
fn amp(sig: &[f32], f: f32) -> f32 {
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (n, &v) in sig.iter().enumerate() {
        let w = 2.0 * std::f64::consts::PI * f as f64 * (n + 24000) as f64 / FS as f64;
        re += v as f64 * w.cos();
        im += v as f64 * w.sin();
    }
    (2.0 * (re * re + im * im).sqrt() / sig.len() as f64) as f32
}

#[test]
fn neutral_settings_are_transparent() {
    let p = Params::default();
    let (i, o) = run(&p, 1000.0, 0.5);
    let r = o.iter().map(|v| v * v).sum::<f32>() / i.iter().map(|v| v * v).sum::<f32>();
    assert!((r - 1.0).abs() < 1e-3, "{r}");
}

#[test]
fn tube_makes_second_harmonic_clean_does_not() {
    let mut p = Params { drive: 0.35, gain_db: 0.0, ch: Char::Tube, ..Default::default() };
    let (_, o) = run(&p, 1000.0, 0.3);
    let (h1, h2, h3) = (amp(&o, 1000.0), amp(&o, 2000.0), amp(&o, 3000.0));
    assert!(h2 > 0.01 && h2 > h3 * 0.8, "tube: h2 {h2} h3 {h3}");
    p.ch = Char::Clean;
    let (_, o) = run(&p, 1000.0, 0.3);
    let (c2, c3) = (amp(&o, 2000.0), amp(&o, 3000.0));
    assert!(c2 < 0.002 && c3 > c2 * 3.0, "clean: h2 {c2} h3 {c3} (fundamental {h1})");
}

#[test]
fn iron_saturates_lows_more_than_highs() {
    let p = Params { drive: 0.3, gain_db: 0.0, ch: Char::Iron, ..Default::default() };
    let thd = |f: f32| {
        let (_, o) = run(&p, f, 0.3);
        let h1 = amp(&o, f);
        let h3 = amp(&o, 3.0 * f);
        h3 / h1
    };
    let (lo, hi) = (thd(60.0), thd(1500.0));
    assert!(lo > hi * 1.3, "lows {lo} highs {hi}");
}

#[test]
fn no_dc_from_the_tube_stage() {
    let p = Params { drive: 0.9, gain_db: 12.0, ch: Char::Tube, ..Default::default() };
    let (_, o) = run(&p, 440.0, 0.4);
    let mean = o.iter().sum::<f32>() / o.len() as f32;
    assert!(mean.abs() < 0.003, "dc {mean}");
}

#[test]
fn tone_controls_move_the_right_band() {
    let p = Params { warmth_db: 6.0, ..Default::default() };
    let t = Tone::new(FS, &p);
    assert!((t.response_db(FS, 40.0) - 6.0).abs() < 1.2);
    assert!(t.response_db(FS, 3000.0).abs() < 0.3);
    let p = Params { presence_db: -5.0, silk_db: 4.0, lowcut: 150.0, ..Default::default() };
    let t = Tone::new(FS, &p);
    assert!((t.response_db(FS, 3500.0) + 5.0).abs() < 1.0);
    assert!((t.response_db(FS, 18000.0) - 4.0).abs() < 1.5);
    assert!(t.response_db(FS, 40.0) < -20.0);
}

#[test]
fn auto_level_compensates_gain() {
    let mut p = Params { gain_db: 12.0, drive: 0.0, ..Default::default() };
    let (_, a) = run(&p, 1000.0, 0.05);
    p.auto_level = true;
    let (_, b) = run(&p, 1000.0, 0.05);
    assert!(amp(&b, 1000.0) < amp(&a, 1000.0) * 0.6);
}

#[test]
fn power_off_is_bypass() {
    let p = Params { gain_db: 20.0, drive: 1.0, power: false, ..Default::default() };
    let (i, o) = run(&p, 500.0, 0.3);
    let d: f32 = i.iter().zip(o.iter()).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
    assert!(d < 0.01, "{d}");
}
