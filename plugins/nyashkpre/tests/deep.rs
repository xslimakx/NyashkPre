//! Deeper behaviour checks of the vocal preamp.
use nyashkpre::dsp::{Char, Params, Pre, Tone};
use std::f32::consts::PI;

fn tone_gain(p: &Params, f: f32, amp: f32, fs: f32) -> f32 {
    let mut pre = Pre::new(fs);
    let n = (fs * 0.5) as usize;
    let (mut re, mut im, mut ri, mut ii) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let s = amp * (2.0 * PI * f * i as f32 / fs).sin();
        let mut x = [s, s];
        pre.process(p, &mut x, 2);
        if i > n / 3 {
            let ph = 2.0 * std::f64::consts::PI * f as f64 * i as f64 / fs as f64;
            re += x[0] as f64 * ph.cos();
            im += x[0] as f64 * ph.sin();
            ri += s as f64 * ph.cos();
            ii += s as f64 * ph.sin();
        }
    }
    10.0 * ((re * re + im * im) / (ri * ri + ii * ii)).log10() as f32
}

fn thd(p: &Params, amp: f32) -> f32 {
    let fs = 48000.0;
    let f = 220.0;
    let mut pre = Pre::new(fs);
    let mut y = vec![];
    for i in 0..48000 {
        let s = amp * (2.0 * PI * f * i as f32 / fs).sin();
        let mut x = [s, s];
        pre.process(p, &mut x, 2);
        if i >= 24000 {
            y.push(x[0]);
        }
    }
    let h = |k: f32| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in y.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * (k * f) as f64 * (i + 24000) as f64 / fs as f64;
            re += *v as f64 * ph.cos();
            im += *v as f64 * ph.sin();
        }
        (re * re + im * im).sqrt()
    };
    let fund = h(1.0);
    ((2..10).map(|k| h(k as f32).powi(2)).sum::<f64>().sqrt() / fund * 100.0) as f32
}

#[test]
fn drive_raises_distortion_steadily_for_every_character() {
    for ch in [Char::Tube, Char::Iron, Char::Clean] {
        let mut last = -1.0;
        let mut row = vec![];
        for d in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let t = thd(&Params { drive: d, ch, gain_db: 6.0, ..Params::default() }, 0.125);
            row.push(format!("{t:.3}"));
            assert!(t >= last * 0.98, "{ch:?}: THD not rising at drive {d}: {t} after {last}");
            last = t;
        }
        println!("{ch:?} THD % at -12 dBFS (+6 dB gain), drive 0..100: {}", row.join(" "));
        assert!(last > 0.5 && last < 15.0, "{ch:?} full drive {last}");
    }
}

#[test]
fn parallel_mix_has_no_comb() {
    // half wet, half dry with the saturation on: the two paths must stay in phase
    for f in [1000.0f32, 6000.0, 10000.0, 16000.0] {
        let g = tone_gain(&Params { drive: 0.5, mix: 0.5, ..Params::default() }, f, 0.003, 48000.0);
        println!("mix 50 %, drive 50 %: {f} Hz {g:+.2} dB");
        assert!(g.abs() < 0.5, "comb at {f}: {g}");
    }
}

#[test]
fn tone_controls_match_their_curve() {
    let fs = 48000.0;
    for p in [
        Params { warmth_db: 6.0, ..Params::default() },
        Params { presence_db: -6.0, ..Params::default() },
        Params { silk_db: 6.0, ..Params::default() },
        Params { lowcut: 120.0, ..Params::default() },
    ] {
        let t = Tone::new(fs, &p);
        for f in [60.0f32, 120.0, 500.0, 3500.0, 10000.0, 15000.0] {
            let meas = tone_gain(&p, f, 0.01, fs);
            let shown = t.response_db(fs, f);
            assert!((meas - shown).abs() < 0.3, "at {f}: sound {meas:.2}, curve {shown:.2}");
        }
    }
}

#[test]
fn auto_level_keeps_the_level() {
    // a vocal-like level; Auto Level should keep the loudness within ~1.5 dB from 0 to +24 dB gain
    for ch in [Char::Tube, Char::Iron, Char::Clean] {
        let mut row = vec![];
        for gdb in [0.0f32, 6.0, 12.0, 18.0, 24.0] {
            let g = tone_gain(&Params { gain_db: gdb, drive: 0.5, ch, auto_level: true, ..Params::default() }, 300.0, 0.1, 48000.0);
            row.push(g);
        }
        println!("{ch:?} auto level, gain 0..24: {:?}", row.iter().map(|v| format!("{v:+.2}")).collect::<Vec<_>>());
        let (lo, hi) = (row.iter().cloned().fold(9.0f32, f32::min), row.iter().cloned().fold(-9.0f32, f32::max));
        assert!(hi - lo < 3.0, "{ch:?}: level moves {:.2} dB", hi - lo);
    }
}

#[test]
fn turning_drive_on_and_off_does_not_click() {
    let fs = 48000.0;
    let mut pre = Pre::new(fs);
    let mut p = Params { drive: 0.0, ..Params::default() };
    let mut prev = 0.0f32;
    let mut worst = 0.0f32;
    let f = 3000.0;
    for i in 0..48000 {
        if i % 6000 == 0 && i > 0 {
            p.drive = if p.drive > 0.0 { 0.0 } else { 0.4 };
        }
        let s = 0.1 * (2.0 * PI * f * i as f32 / fs).sin();
        let mut x = [s, s];
        pre.process(&p, &mut x, 2);
        if i > 1000 {
            worst = worst.max((x[0] - prev).abs());
        }
        prev = x[0];
    }
    let slope = 0.1 * 2.0 * PI * f / fs;
    println!("largest step {worst:.4} (tone slope {slope:.4})");
    assert!(worst < slope * 1.15, "drive switch clicks: {worst}");
}
