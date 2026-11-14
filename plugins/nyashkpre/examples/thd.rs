//! THD / harmonic / aliasing report of the preamp (developer tool).
use nyashkpre::dsp::{Char, Params, Pre};

fn analyse(ch: Char, drive: f32, gain_db: f32, level_db: f32, f: f32) -> (f32, f32, f32, f32, f32) {
    let fs = 48000.0f32;
    let mut pre = Pre::new(fs);
    let p = Params { gain_db, drive, ch, auto_level: true, ..Params::default() };
    let n = 1 << 16;
    let a = 10f32.powf(level_db / 20.0);
    let mut out = Vec::with_capacity(n);
    for i in 0..(n + 8192) {
        let v = a * (2.0 * std::f32::consts::PI * f * i as f32 / fs).sin();
        let mut x = [v, v];
        pre.process(&p, &mut x, 1);
        if i >= 8192 {
            out.push(x[0] as f64);
        }
    }
    // DFT at harmonics (Blackman-Harris) + total energy
    let w = |i: usize| {
        let t = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        0.35875 - 0.48829 * t.cos() + 0.14128 * (2.0 * t).cos() - 0.01168 * (3.0 * t).cos()
    };
    let amp = |fr: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in out.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * fr * i as f64 / fs as f64;
            re += v * w(i) * ph.cos();
            im += v * w(i) * ph.sin();
        }
        (re * re + im * im).sqrt()
    };
    let h1 = amp(f as f64);
    let mut hs = vec![];
    let mut harm_e = 0.0;
    for k in 2..=((20000.0 / f) as usize).min(40) {
        let h = amp(f as f64 * k as f64);
        harm_e += h * h;
        hs.push(h);
    }
    // total energy (Parseval with window): approximate via time domain
    let tot: f64 = out.iter().enumerate().map(|(i, v)| (v * w(i)).powi(2)).sum::<f64>();
    let fund_e = h1 * h1 * 2.0 / out.iter().enumerate().map(|(i, _)| w(i).powi(2)).sum::<f64>();
    let _ = fund_e;
    let thd = (harm_e.sqrt() / h1 * 100.0) as f32;
    let h2 = (20.0 * (hs[0] / h1).log10()) as f32;
    let h3 = (20.0 * (hs.get(1).copied().unwrap_or(1e-12) / h1).log10()) as f32;
    // high harmonics (above 5 kHz) relative: the "fizz"
    let hi: f64 = hs.iter().enumerate().filter(|(k, _)| (k + 2) as f32 * f > 5000.0).map(|(_, h)| h * h).sum();
    let fizz = (10.0 * (hi / (h1 * h1) + 1e-20).log10()) as f32;
    let _ = tot;
    let out_db = (20.0 * (out.iter().map(|v| v * v).sum::<f64>() / out.len() as f64).sqrt().log10() + 3.01) as f32;
    (thd, h2, h3, fizz, out_db)
}

fn main() {
    if std::env::var("COLOR").is_ok() {
        for lvl in [-24.0f32, -18.0, -12.0, -6.0] {
            for d in [0.0f32, 0.3] {
                let mut pre = Pre::new(48000.0);
                let p = Params { gain_db: 6.0, drive: d, ch: Char::Tube, auto_level: true, ..Params::default() };
                let a = 10f32.powf(lvl / 20.0);
                for i in 0..48000 {
                    let v = a * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48000.0).sin();
                    let mut x = [v, v];
                    pre.process(&p, &mut x, 1);
                }
                println!("in {lvl} drive {d}: colour {:.1} dB", pre.color_db);
            }
        }
        return;
    }
    if let Ok(f) = std::env::var("F") {
        let f: f32 = f.parse().unwrap();
        for ch in [Char::Tube, Char::Iron, Char::Clean] {
            for drive in [0.3f32, 1.0] {
                let (thd, h2, h3, fizz, od) = analyse(ch, drive, 6.0, -12.0, f);
                println!("{ch:?} drive {:3.0}% {f} Hz -12 dBFS: THD {thd:6.2}%  H2 {h2:6.1} H3 {h3:6.1} dB  out {od:+.1}", drive * 100.0);
                let _ = fizz;
            }
        }
        return;
    }
    for ch in [Char::Tube, Char::Iron, Char::Clean] {
        for drive in [0.0f32, 0.3, 0.6, 1.0] {
            for lvl in [-18.0f32, -12.0, -6.0] {
                let (thd, h2, h3, fizz, od) = analyse(ch, drive, 6.0, lvl, 220.0);
                println!("{ch:?} drive {:3.0}% in {lvl:+} dBFS: THD {thd:6.2}%  H2 {h2:6.1} H3 {h3:6.1} dB  >5k {fizz:6.1} dB  out {od:+.1} dBFS", drive * 100.0);
            }
        }
    }
}

#[allow(dead_code)]
pub fn color_check() {}
