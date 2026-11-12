//! NyashkPre: light vocal preamp / saturator (VST3).

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub mod dsp;
mod editor;

pub struct Meters {
    pub in_db: AtomicU32,
    pub out_db: AtomicU32,
    pub color_db: AtomicU32,
    pub fs: AtomicU32,
    /// live input (mono) for the saturation trace, live output for the spectrum
    pub tap_in: nyashk_common::tap::Tap,
    pub tap_out: nyashk_common::tap::Tap,
}
impl Meters {
    fn new() -> Self {
        let m = (-90f32).to_bits();
        Self { in_db: AtomicU32::new(m), out_db: AtomicU32::new(m), color_db: AtomicU32::new(m),
            fs: AtomicU32::new(48000f32.to_bits()),
            tap_in: nyashk_common::tap::Tap::new(8192),
            tap_out: nyashk_common::tap::Tap::new(8192),
        }
    }
    pub fn get(a: &AtomicU32) -> f32 {
        f32::from_bits(a.load(Ordering::Relaxed))
    }
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Character {
    #[name = "TUBE"]
    Tube,
    #[name = "IRON"]
    Iron,
    #[name = "CLEAN"]
    Clean,
}

fn db_param(name: &str, min: f32, max: f32, def: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Linear { min, max })
        .with_step_size(0.1)
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_value_to_string(Arc::new(|v| format!("{:+.1}", v)))
}

#[derive(Params)]
pub struct PreParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "power"]
    pub power: BoolParam,
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "char"]
    pub character: EnumParam<Character>,
    #[id = "lowcut"]
    pub lowcut: FloatParam,
    #[id = "warmth"]
    pub warmth: FloatParam,
    #[id = "pres"]
    pub presence: FloatParam,
    #[id = "silk"]
    pub silk: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "out"]
    pub out: FloatParam,
    #[id = "auto"]
    pub auto_level: BoolParam,
    #[id = "phase"]
    pub phase: BoolParam,
}

impl Default for PreParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(700, 450),
            power: BoolParam::new("Power", true),
            gain: db_param("Gain", 0.0, 36.0, 6.0),
            drive: FloatParam::new("Drive", 30.0, FloatRange::Linear { min: 0.0, max: 100.0 }).with_step_size(0.1).with_smoother(SmoothingStyle::Linear(20.0)),
            character: EnumParam::new("Character", Character::Tube),
            lowcut: FloatParam::new("Low Cut", 20.0, FloatRange::Skewed { min: 20.0, max: 300.0, factor: FloatRange::skew_factor(-1.5) })
                .with_smoother(SmoothingStyle::Logarithmic(25.0))
                .with_value_to_string(Arc::new(|v| if v <= 20.5 { "OFF".to_string() } else { format!("{:.0}", v) }))
                .with_string_to_value(Arc::new(|s| {
                    let t = s.trim().to_lowercase().replace("hz", "");
                    if t.trim() == "off" { Some(20.0) } else { t.trim().parse().ok() }
                })),
            warmth: db_param("Warmth", -6.0, 6.0, 0.0),
            presence: db_param("Presence", -6.0, 6.0, 0.0),
            silk: db_param("Silk", -6.0, 6.0, 0.0),
            mix: FloatParam::new("Mix", 100.0, FloatRange::Linear { min: 0.0, max: 100.0 }).with_unit(" %").with_step_size(0.1).with_smoother(SmoothingStyle::Linear(20.0)),
            out: db_param("Output", -18.0, 18.0, 0.0),
            auto_level: BoolParam::new("Auto Level", true),
            phase: BoolParam::new("Phase", false),
        }
    }
}

impl PreParams {
    pub fn dsp_char(&self) -> dsp::Char {
        match self.character.value() {
            Character::Tube => dsp::Char::Tube,
            Character::Iron => dsp::Char::Iron,
            Character::Clean => dsp::Char::Clean,
        }
    }
}

pub struct NyashkPre {
    params: Arc<PreParams>,
    meters: Arc<Meters>,
    pre: Option<dsp::Pre>,
}

impl Default for NyashkPre {
    fn default() -> Self {
        Self { params: Arc::new(PreParams::default()), meters: Arc::new(Meters::new()), pre: None }
    }
}

impl Plugin for NyashkPre {
    const NAME: &'static str = "NyashkPre";
    const VENDOR: &'static str = "Nyashk";
    const URL: &'static str = "https://github.com/xslimakx";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout { main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() },
        AudioIOLayout { main_input_channels: NonZeroU32::new(1), main_output_channels: NonZeroU32::new(1), ..AudioIOLayout::const_default() },
    ];
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _e: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.meters.clone())
    }

    fn initialize(&mut self, _l: &AudioIOLayout, config: &BufferConfig, _c: &mut impl InitContext<Self>) -> bool {
        self.pre = Some(dsp::Pre::new(config.sample_rate));
        self.meters.fs.store(config.sample_rate.to_bits(), Ordering::Relaxed);
        true
    }

    fn reset(&mut self) {
        if let Some(p) = self.pre.as_mut() {
            p.reset();
        }
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, _c: &mut impl ProcessContext<Self>) -> ProcessStatus {
        nyashk_common::demo::fill(buffer, f32::from_bits(self.meters.fs.load(Ordering::Relaxed)));
        nyashk_common::demo::sanitize(buffer);
        let Some(pre) = self.pre.as_mut() else { return ProcessStatus::Normal };
        let p = &self.params;
        let (mut ipk, mut opk) = (0.0f32, 0.0f32);
        for mut frame in buffer.iter_samples() {
            let dp = dsp::Params {
                gain_db: p.gain.smoothed.next(),
                drive: p.drive.smoothed.next() / 100.0,
                ch: p.dsp_char(),
                lowcut: { let v = p.lowcut.smoothed.next(); if v <= 20.5 { 0.0 } else { v } },
                warmth_db: p.warmth.smoothed.next(),
                presence_db: p.presence.smoothed.next(),
                silk_db: p.silk.smoothed.next(),
                mix: p.mix.smoothed.next() / 100.0,
                out_db: p.out.smoothed.next(),
                auto_level: p.auto_level.value(),
                phase: p.phase.value(),
                power: p.power.value(),
            };
            let n = frame.len().min(2);
            let mut x = [0.0f32; 2];
            for (i, s) in frame.iter_mut().enumerate().take(n) {
                x[i] = *s;
            }
            self.meters.tap_in.push(if n == 2 { 0.5 * (x[0] + x[1]) } else { x[0] });
            pre.process(&dp, &mut x, n);
            self.meters.tap_out.push(if n == 2 { 0.5 * (x[0] + x[1]) } else { x[0] });
            ipk = ipk.max(pre.in_peak);
            opk = opk.max(pre.out_peak);
            for (i, s) in frame.iter_mut().enumerate().take(n) {
                *s = x[i];
            }
        }
        let db = |v: f32| 20.0 * (v + 1e-9).log10();
        self.meters.in_db.store(db(ipk).to_bits(), Ordering::Relaxed);
        self.meters.out_db.store(db(opk).to_bits(), Ordering::Relaxed);
        self.meters.color_db.store(pre.color_db.to_bits(), Ordering::Relaxed);
        ProcessStatus::Normal
    }
}

impl Vst3Plugin for NyashkPre {
    const VST3_CLASS_ID: [u8; 16] = *b"NyashkPreVST3!!!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_vst3!(NyashkPre);
