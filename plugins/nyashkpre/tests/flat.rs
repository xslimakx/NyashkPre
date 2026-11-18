//! Neutral settings must not colour the top end at 44.1 / 48 kHz (oversampling, filters).
use nyashkpre::dsp::*;

fn gain(fs: f32, f: f32, mut run: impl FnMut(&mut [f32; 2])) -> f32 {
    let n = (fs * 0.6) as usize;
    let (mut ei, mut eo) = (0.0f64, 0.0f64);
    for i in 0..n {
        let s = 0.05 * (2.0 * std::f32::consts::PI * f * i as f32 / fs).sin();
        let mut x = [s, s];
        run(&mut x);
        if i > n / 2 {
            ei += (s * s) as f64;
            eo += (x[0] * x[0]) as f64;
        }
    }
    10.0 * (eo / ei).log10() as f32
}

#[test]
fn neutral_settings_are_flat_to_20k() {
    for fs in [44100.0f32, 48000.0] {
        for (name, p) in [("default", Params::default()), ("half mix", Params { mix: 0.5, ..Params::default() })] {
            let mut line = vec![];
            let mut g1 = 0.0;
            for f in [1000.0f32, 10000.0, 15000.0, 18000.0, 20000.0] {
                let mut d = Pre::new(fs);
                let g = gain(fs, f, |x| d.process(&p, x, 2));
                if f == 1000.0 {
                    g1 = g;
                }
                line.push(format!("{f}: {g:.2}"));
                assert!((g - g1).abs() < 0.3, "{name} fs {fs} f {f}: {g} dB vs {g1} at 1 kHz");
            }
            println!("{name} fs {fs}: {}", line.join("  "));
        }
    }
}
