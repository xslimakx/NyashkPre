//! Vocal preamp: input gain, three saturation characters, tone section, parallel mix.
use nyashk_common::dsp::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Char {
    /// Asymmetric triode-style stage: even harmonics (2nd), smooth.
    Tube,
    /// Transformer: lows saturate first (density and weight), highs stay open.
    Iron,
    /// Symmetric, tight and clean; odd harmonics only at high drive.
    Clean,
}

#[derive(Clone, Copy)]
pub struct Params {
    pub gain_db: f32,
    pub drive: f32, // 0..1
    pub ch: Char,
    pub lowcut: f32, // Hz, 0 = off
    pub warmth_db: f32,
    pub presence_db: f32,
    pub silk_db: f32,
    pub mix: f32, // 0..1
    pub out_db: f32,
    pub auto_level: bool,
    pub phase: bool,
    pub power: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params { gain_db: 0.0, drive: 0.0, ch: Char::Tube, lowcut: 0.0, warmth_db: 0.0, presence_db: 0.0, silk_db: 0.0, mix: 1.0, out_db: 0.0, auto_level: false, phase: false, power: true }
    }
}

pub const WARMTH_F: f32 = 120.0;
/// The top end goes into the saturation this much quieter and is lifted back after it: sibilants
/// and breaths make far fewer harsh high harmonics, the tone is unchanged.
pub const EMPH_F: f32 = 4500.0;
pub const EMPH_DB: f32 = 6.0;
pub const PRES_F: f32 = 3500.0;
pub const SILK_F: f32 = 10000.0;

#[derive(Clone, Copy)]
pub struct Tone {
    pub hp: [Coef; 2],
    pub hp_on: bool,
    pub warm: Coef,
    pub pres: Coef,
    pub silk: Coef,
}

impl Tone {
    pub fn new(fs: f32, p: &Params) -> Self {
        let q = butter_q::<2>();
        let sh = |c: Coef, g: f32| if g.abs() < 0.01 { Coef::THRU } else { c };
        Tone {
            hp: [Coef::highpass(fs, p.lowcut.max(10.0), q[0]), Coef::highpass(fs, p.lowcut.max(10.0), q[1])],
            hp_on: p.lowcut > 10.0,
            warm: sh(Coef::low_shelf(fs, WARMTH_F, p.warmth_db, 0.7), p.warmth_db),
            pres: sh(Coef::peaking(fs, PRES_F, p.presence_db, 0.8), p.presence_db),
            silk: sh(Coef::high_shelf(fs, SILK_F, p.silk_db, 0.7), p.silk_db),
        }
    }
    pub fn response_db(&self, fs: f32, f: f32) -> f32 {
        let mut d = self.warm.response_db(fs, f) + self.pres.response_db(fs, f) + self.silk.response_db(fs, f);
        if self.hp_on {
            d += self.hp[0].response_db(fs, f) + self.hp[1].response_db(fs, f);
        }
        d
    }
}

/// How hard the stage is driven: the slope of the curve. Drive 0 % keeps a vocal at -18 dBFS
/// (after the gain) practically clean (~0.05 % THD), 100 % gives a few per cent there - the
/// range of a real vocal preamp pushed from clean to warm, not a guitar amp. Louder peaks bend
/// more, as in hardware.
pub fn drive_k(ch: Char, drive: f32) -> f32 {
    let (k0, k1) = match ch {
        Char::Tube => (0.4f32, 2.6f32),
        Char::Iron => (0.45, 2.6),
        Char::Clean => (0.35, 2.2),
    };
    k0 * (k1 / k0).powf(drive.clamp(0.0, 1.0))
}

/// Bias of the tube curve: the asymmetry that makes the even (2nd) harmonic.
const TUBE_BIAS: f32 = 0.15;

/// The static transfer curve of the saturation stage, `v` = signal after the gain stage.
/// Small-signal gain is 1 and the curve is bounded, so the stage stays level-neutral for quiet
/// material and never folds over on hot peaks.
pub fn shaper(ch: Char, drive: f32, v: f32) -> f32 {
    let k = drive_k(ch, drive);
    match ch {
        // a biased soft curve: smooth 2nd harmonic first, odd ones only when pushed
        Char::Tube => {
            let t0 = TUBE_BIAS.tanh();
            let slope = 1.0 - t0 * t0;
            ((k * v + TUBE_BIAS).tanh() - t0) / (k * slope)
        }
        // symmetric: odd harmonics, the low band of Iron is driven harder in the processor
        Char::Clean | Char::Iron => (k * v).tanh() / k,
    }
}

#[derive(Clone, Default)]
struct Chn {
    hp: Chain<2>,
    warm: Bq,
    pres: Bq,
    silk: Bq,
    split: Bq,
    dc: Bq,
    pre: Bq,
    de: Bq,
}

pub struct Pre {
    fs: f32,
    ch: [Chn; 2],
    os: [Os2; 2],
    tone: Tone,
    split: Coef,
    dc: Coef,
    emph: Coef,
    deemph: Coef,
    tick: u32,
    pub in_peak: f32,
    pub out_peak: f32,
    pub color_db: f32,
    c_n: f32,
    c_d: f32,
    sm_gain: Smooth,
    sm_out: Smooth,
    sm_mix: Smooth,
    sm_pow: Smooth,
}

impl Pre {
    pub fn new(fs: f32) -> Self {
        Self {
            fs,
            ch: Default::default(),
            os: [Os2::new(fs), Os2::new(fs)],
            tone: Tone::new(fs, &Params::default()),
            split: Coef::lowpass(fs, 180.0, 0.7071),
            dc: Coef::highpass(fs, 12.0, 0.7071),
            emph: Coef::high_shelf(fs, EMPH_F, -EMPH_DB, 0.7),
            deemph: Coef::high_shelf(fs, EMPH_F, EMPH_DB, 0.7),
            tick: 0,
            in_peak: 0.0,
            out_peak: 0.0,
            color_db: -90.0,
            c_n: 0.0,
            c_d: 0.0,
            sm_gain: Smooth::new(1.0, fs, 15.0),
            sm_out: Smooth::new(1.0, fs, 15.0),
            sm_mix: Smooth::new(1.0, fs, 15.0),
            sm_pow: Smooth::new(1.0, fs, 8.0),
        }
    }

    pub fn reset(&mut self) {
        self.ch = Default::default();
        for o in self.os.iter_mut() {
            o.reset();
        }
    }

    pub fn process(&mut self, p: &Params, x: &mut [f32; 2], n: usize) {
        if self.tick % 16 == 0 {
            self.tone = Tone::new(self.fs, p);
        }
        self.tick = self.tick.wrapping_add(1);
        // Auto Level: the gain comes back off at the output, so Gain only sets how hard the stage
        // is hit (a little less than all of it: what the saturation itself shaves off the peaks)
        let comp = if p.auto_level { -0.9 * p.gain_db } else { 0.0 };
        let g = self.sm_gain.tick(db_to_lin(p.gain_db));
        let og = self.sm_out.tick(db_to_lin(p.out_db + comp)) * if p.phase { -1.0 } else { 1.0 };
        let mix = self.sm_mix.tick(p.mix);
        let pw = self.sm_pow.tick(if p.power { 1.0 } else { 0.0 });
        let (mut ipk, mut opk) = (0.0f32, 0.0f32);
        let t = self.tone;
        for i in 0..n {
            let dry = x[i];
            ipk = ipk.max(dry.abs());
            let st = &mut self.ch[i];
            let v = dry * g;
            // saturation (oversampled only when it does something)
            let wet = if p.drive > 0.001 {
                let split = self.split;
                let dc = self.dc;
                let ch = p.ch;
                let d = p.drive;
                let (emph, deemph) = (self.emph, self.deemph);
                let ve = st.pre.tick(&emph, v);
                let mut y = self.os[i].process(ve, |a| shaper(ch, d, a));
                y = st.de.tick(&deemph, y);
                if ch == Char::Iron {
                    // extra weight: the low band is driven harder than the rest
                    let lo = st.split.tick(&split, v);
                    let lo_s = shaper(Char::Iron, (d + 0.35).min(1.0), lo);
                    y += lo_s - lo;
                }
                if ch == Char::Tube {
                    y = st.dc.tick(&dc, y);
                }
                y
            } else {
                v
            };
            self.c_n += (wet - v) * (wet - v);
            self.c_d += v * v;
            // tone
            let mut y = wet;
            if t.hp_on {
                y = st.hp.tick(&t.hp, y);
            }
            y = st.warm.tick(&t.warm, y);
            y = st.pres.tick(&t.pres, y);
            y = st.silk.tick(&t.silk, y);
            let o = (dry * g * (1.0 - mix) + y * mix) * og;
            let o = dry + (o - dry) * pw;
            opk = opk.max(o.abs());
            x[i] = o;
        }
        self.c_n *= 0.9995;
        self.c_d *= 0.9995;
        self.color_db = 10.0 * ((self.c_n + 1e-14) / (self.c_d + 1e-14)).log10();
        self.in_peak = ipk;
        self.out_peak = opk;
    }
}
