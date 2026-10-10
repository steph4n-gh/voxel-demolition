use std::collections::HashMap;
use std::cell::Cell;
use std::sync::Mutex;
use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use self::pcm_to_wav_bytes as create_wav;

#[allow(dead_code)]
pub struct SoundSystem {
    pub explosion: Option<Sound>,
    pub big_boom: Option<Sound>,
    pub railgun: Option<Sound>,
    pub singularity_hum: Option<Sound>,
    pub singularity_implode: Option<Sound>,
    pub plasma_buzz: Option<Sound>,
    pub cryo_freeze: Option<Sound>,
    pub cryo_shatter: Option<Sound>,
    pub ram_impact: Option<Sound>,
    pub debris_clatter: Option<Sound>,
    pub ui_click: Option<Sound>,
    pub ui_switch: Option<Sound>,
    pub structural_groan: Option<Sound>,
    pub mortar_launch: Option<Sound>,
    pub subsonic_artillery_boom: Option<Sound>,
    pub hydraulic_slam: Option<Sound>,
    pub railgun_boom: Option<Sound>,
    pub singularity_launch: Option<Sound>,
    pub vacuum_sweep: Option<Sound>,
    pub stress_creak: Option<Sound>,
    pub jet_dash: Option<Sound>,
    pub land_impact: Option<Sound>,
    pub jump: Option<Sound>,
    pub hit_tick: Option<Sound>,
    pub hit_crit: Option<Sound>,
    pub seismic_pulse: Option<Sound>,
    pub tesla_arc: Option<Sound>,
    pub plasma_fire: Option<Sound>,
    pub bullet_time_drone: Option<Sound>,
    pub capacitor_whine: Option<Sound>,
    pub recent_plays: Mutex<HashMap<usize, f64>>,
    pub recent_timestamps: Mutex<Vec<f64>>,
    master_volume: Cell<f32>,
    siege_volume: Cell<f32>,
    stress_creak_volume: Cell<f32>,
    anti_stacking_seconds: Cell<f64>,
}

/// Helper function to package synthesized f32 mono PCM samples into standard 16-bit RIFF WAV bytes
pub fn pcm_to_wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;
    let bits_per_sample = 16u16;
    let subchunk2_size = num_samples * 2;
    let chunk_size = 36 + subchunk2_size;

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt subchunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size
    wav.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (PCM)
    wav.extend_from_slice(&1u16.to_le_bytes());  // NumChannels (1 = Mono)
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&subchunk2_size.to_le_bytes());

    // Convert f32 [-1.0, 1.0] to 16-bit signed PCM
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        wav.extend_from_slice(&sample_i16.to_le_bytes());
    }

    wav
}

// Simple deterministic pseudo-random generator for noise
struct NoiseGen {
    seed: u32,
}
impl NoiseGen {
    fn new(seed: u32) -> Self { Self { seed } }
    fn next_f32(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) as f32 / 16777216.0) * 2.0 - 1.0
    }
}

impl SoundSystem {
    pub async fn new() -> Self {
        let sample_rate = 44100;

        // 1. Explosion (Punchy transient + warm bass drop + filtered noise tail)
        let explosion = {
            let dur = 0.65;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(42);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 6.5).exp();
                let freq = 115.0 * (-t * 14.0).exp() + 45.0;
                let sine = (t * freq * std::f32::consts::TAU).sin();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.14;
                let mixed = (sine * 0.75 + filter_state * 0.30) * env;
                samples.push(mixed.tanh() * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 2. Big Bass Boom (Nuke / Structural Collapse)
        let big_boom = {
            let dur = 1.4;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(99);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 2.6).exp();
                let freq = 68.0 * (-t * 4.2).exp() + 40.0;
                let sub = (t * freq * std::f32::consts::TAU).sin();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.10;
                let rumble = filter_state * 0.40 * (-t * 3.5).exp();
                let mixed = (sub * 0.8 + rumble * 0.30) * env;
                samples.push(mixed.clamp(-1.0, 1.0) * 0.42);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 3. Railgun (Instant hypersonic crack + sub thump)
        let railgun = {
            let dur = 0.45;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(1337);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 9.5).exp();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.22;
                let snap = filter_state * (-t * 50.0).exp() * 0.35;
                let hum = (t * 260.0 * std::f32::consts::TAU).sin() * 0.28
                    + (t * 520.0 * std::f32::consts::TAU).sin() * 0.10;
                let sub = (t * 68.0 * (-t * 16.0).exp() * std::f32::consts::TAU).sin() * 0.4;
                let mixed = snap + hum * env + sub * env;
                samples.push(mixed.tanh() * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 4. Singularity Hum (Mysterious oscillating vortex drone)
        let singularity_hum = {
            let dur = 0.55;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (std::f32::consts::PI * (t / dur)).sin().powf(0.8);
                let lfo = (t * 11.0 * std::f32::consts::TAU).sin() * 12.0;
                let sine = (t * (155.0 + lfo) * std::f32::consts::TAU).sin();
                let sub = (t * 54.0 * std::f32::consts::TAU).sin();
                samples.push((sine * 0.30 + sub * 0.30) * env * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 5. Singularity Implosion (Reverse suction sweep into colossal pop)
        let singularity_implode = {
            let dur = 0.80;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(777);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let val = if t < 0.45 {
                    let p = t / 0.45;
                    let freq = 65.0 + p * p * 420.0;
                    (t * freq * std::f32::consts::TAU).sin() * p * 0.45
                } else {
                    let dt = t - 0.45;
                    let env = (-dt * 9.5).exp();
                    let bass = (dt * 48.0 * std::f32::consts::TAU).sin();
                    let raw_n = noise.next_f32();
                    filter_state += (raw_n - filter_state) * 0.20;
                    (bass * 0.65 + filter_state * 0.28) * env
                };
                samples.push(val.clamp(-1.0, 1.0) * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 6. Plasma Beam Buzz (Sizzling high energy laser)
        let plasma_buzz = {
            let dur = 0.16;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(2024);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (std::f32::consts::PI * (t / dur)).sin().powf(0.6);
                let hum = (t * 200.0 * std::f32::consts::TAU).sin() * 0.32;
                let buzz = (t * 380.0 * std::f32::consts::TAU).sin() * 0.18;
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.16;
                samples.push((hum + buzz + filter_state * 0.15) * env * 0.35);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 7. Cryo Freeze (Eerie crystal chime)
        let cryo_freeze = {
            let dur = 0.35;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 7.5).exp();
                let chime1 = (t * 760.0 * std::f32::consts::TAU).sin();
                let chime2 = (t * 1140.0 * std::f32::consts::TAU).sin();
                let chime3 = (t * 1330.0 * std::f32::consts::TAU).sin();
                samples.push((chime1 * 0.35 + chime2 * 0.25 + chime3 * 0.12) * env * 0.38);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 8. Cryo Shatter (Violent crystalline glass breakup)
        let cryo_shatter = {
            let dur = 0.40;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(555);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 10.5).exp();
                let chime = (t * 1120.0 * std::f32::consts::TAU).sin() * 0.20;
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.25;
                let crumble = filter_state * 0.38;
                samples.push((crumble + chime) * env * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 9. Breaching Ram Heavy Impact
        let ram_impact = {
            let dur = 0.38;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(888);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 12.5).exp();
                let thud = (t * 80.0 * (-t * 16.0).exp() * std::f32::consts::TAU).sin();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.18;
                let crunch = filter_state * 0.35;
                samples.push((thud * 0.80 + crunch * 0.28) * env * 0.42);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 10. Debris Clatter (Staccato impacts of falling blocks)
        let debris_clatter = {
            let dur = 0.25;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(3141);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 15.0).exp();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.20;
                let click = filter_state * 0.35;
                let ring = (t * 540.0 * std::f32::consts::TAU).sin() * 0.16;
                samples.push((click + ring) * env * 0.30);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 11. UI Click
        let ui_click = {
            let dur = 0.04;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 85.0).exp();
                let tone = (t * 1050.0 * std::f32::consts::TAU).sin();
                samples.push(tone * env * 0.25);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 12. UI Switch
        let ui_switch = {
            let dur = 0.07;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 45.0).exp();
                let tone = (t * (520.0 + t * 2800.0) * std::f32::consts::TAU).sin();
                samples.push(tone * env * 0.25);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 13. Structural Groan (Low eerie metal stress warning before building collapses)
        let structural_groan = {
            let dur = 1.0;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (std::f32::consts::PI * (t / dur)).sin().powf(0.85);
                let freq = 98.0 + (t * 4.5 * std::f32::consts::TAU).sin() * 10.0;
                let groan = (t * freq * std::f32::consts::TAU).sin();
                let sub = (t * 50.0 * std::f32::consts::TAU).sin();
                samples.push((groan * 0.45 + sub * 0.35) * env * 0.38);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 14. Mortar Launch (Heavy artillery launch thunk + gas blast)
        let mortar_launch = {
            let dur = 0.34;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(101);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 16.5).exp();
                let freq = 145.0 * (-t * 22.0).exp() + 45.0;
                let thunk = (t * freq * std::f32::consts::TAU).sin();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.16;
                let gas = filter_state * (-t * 28.0).exp() * 0.38;
                let clang = (t * 460.0 * std::f32::consts::TAU).sin() * (-t * 20.0).exp() * 0.16;
                let mixed = (thunk * 0.75 + gas + clang) * env;
                samples.push(mixed.tanh() * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 15. Subsonic Artillery Detonation (Deep cratering 45-60Hz punch + acoustic tail)
        let subsonic_artillery_boom = {
            let dur = 1.5;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(202);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 2.4).exp();
                let freq = 58.0 * (-t * 3.8).exp() + 42.0;
                let sub = (t * freq * std::f32::consts::TAU).sin();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.09;
                let rumble = filter_state * 0.35 * (-t * 2.8).exp();
                let mid_thud = (t * 88.0 * (-t * 12.0).exp() * std::f32::consts::TAU).sin() * 0.25;
                let mixed = (sub * 0.75 + rumble + mid_thud) * env;
                samples.push(mixed.tanh() * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 16. Hydraulic Slam (Pneumatic hiss + massive hydraulic ram punch)
        let hydraulic_slam = {
            let dur = 0.40;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(303);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 11.5).exp();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.18;
                let hiss = filter_state * (-(t - 0.03).powi(2) * 900.0).exp() * 0.28;
                let piston_freq = 95.0 * (-t * 18.0).exp() + 40.0;
                let punch = (t * piston_freq * std::f32::consts::TAU).sin();
                let metal_crunch = filter_state * (-t * 20.0).exp() * 0.28;
                let mixed = (punch * 0.75 + hiss + metal_crunch) * env;
                samples.push(mixed.tanh() * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 17. Railgun Sonic Crack & Sub-bass Boom (Hypersonic crack + shockwave)
        let railgun_boom = {
            let dur = 0.70;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(404);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 5.8).exp();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.20;
                let snap = filter_state * (-t * 55.0).exp() * 0.38;
                let sub = (t * (75.0 * (-t * 14.0).exp() + 42.0) * std::f32::consts::TAU).sin();
                let ping = (t * 780.0 * std::f32::consts::TAU).sin() * (-t * 26.0).exp() * 0.18;
                let mixed = snap + sub * 0.70 * env + ping;
                samples.push(mixed.tanh() * 0.45);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 18. Singularity Launch (Dark matter spatial whoosh + magnetic charge)
        let singularity_launch = {
            let dur = 0.45;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(505);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 7.5).exp();
                let whoosh_freq = 80.0 + (t * 20.0 * std::f32::consts::TAU).sin() * 26.0;
                let whoosh = (t * whoosh_freq * std::f32::consts::TAU).sin();
                let charge = (t * 340.0 * (1.0 + t * 3.0) * std::f32::consts::TAU).sin() * 0.20;
                let raw_n = noise.next_f32();
                filter_state += (raw_n - filter_state) * 0.16;
                let n = filter_state * 0.20 * env;
                let mixed = (whoosh * 0.55 + charge + n) * env;
                samples.push(mixed.tanh() * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 19. Singularity Vacuum Sweep (Deep vacuum sweep & spatial drag)
        let vacuum_sweep = {
            let dur = 0.90;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let p = (t / dur).clamp(0.0, 1.0);
                let sweep_freq = 42.0 + (p * p) * 140.0;
                let lfo = (t * 18.0 * std::f32::consts::TAU).sin() * 8.0;
                let tone = (t * (sweep_freq + lfo) * std::f32::consts::TAU).sin();
                let sub = (t * 46.0 * std::f32::consts::TAU).sin();
                let env = (std::f32::consts::PI * p).sin().powf(0.8);
                let mixed = (tone * 0.40 + sub * 0.40) * env;
                samples.push(mixed.clamp(-1.0, 1.0) * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 20. Stress Creak (High-tension strain groan & mortar fracture)
        let stress_creak = {
            let dur = 1.0;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(606);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (std::f32::consts::PI * (t / dur)).sin().powf(0.85);
                let jitter = (t * 24.0 * std::f32::consts::TAU).sin() * 24.0;
                let creak1 = (t * (210.0 + jitter) * std::f32::consts::TAU).sin();
                let creak2 = (t * (420.0 + jitter * 1.3) * std::f32::consts::TAU).sin() * 0.25;
                let raw_n = noise.next_f32();
                filter_state += (raw_n - filter_state) * 0.16;
                let grit = filter_state * 0.18 * (-((t * 5.0).sin().abs()) * 1.2).exp();
                let mixed = (creak1 * 0.45 + creak2 * 0.25 + grit) * env;
                samples.push(mixed.tanh() * 0.38);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 21. Jet Dash Thruster (Pneumatic gas rush + rocket surge)
        let jet_dash = {
            let dur = 0.30;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(909);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 9.5).exp() * (1.0 - (-t * 80.0).exp());
                let raw_n = noise.next_f32();
                filter_state += (raw_n - filter_state) * 0.20;
                let hiss = filter_state * 0.40;
                let flame = (t * 68.0 * std::f32::consts::TAU).sin() * 0.35;
                let surge = (t * 160.0 * (-t * 14.0).exp() * std::f32::consts::TAU).sin() * 0.30;
                let mixed = (hiss + flame + surge) * env;
                samples.push(mixed.tanh() * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 22. Land Impact (Heavy bedrock/concrete body slam thud)
        let land_impact = {
            let dur = 0.25;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(1010);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 16.0).exp();
                let freq = 55.0 * (-t * 20.0).exp() + 40.0;
                let thud = (t * freq * std::f32::consts::TAU).sin();
                let raw_n = noise.next_f32();
                filter_state += (raw_n - filter_state) * 0.16;
                let grit = filter_state * (-t * 30.0).exp() * 0.28;
                let mixed = (thud * 0.75 + grit) * env;
                samples.push(mixed.tanh() * 0.40);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 23. Jump (Springy concrete foot push)
        let jump = {
            let dur = 0.11;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 32.0).exp();
                let freq = 100.0 + t * 300.0;
                let spring = (t * freq * std::f32::consts::TAU).sin();
                samples.push(spring * env * 0.30);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 24. Hit Confirm Tick (Subtle tactile blip)
        let hit_tick = {
            let dur = 0.035;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 90.0).exp();
                let tone = (t * 1250.0 * std::f32::consts::TAU).sin() * 0.75
                    + (t * 1850.0 * std::f32::consts::TAU).sin() * 0.25;
                samples.push(tone * env * 0.28);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 25. Critical Hit Confirm Chime (Two-tone resonant warm chime)
        let hit_crit = {
            let dur = 0.08;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 40.0).exp();
                let chime1 = (t * 880.0 * std::f32::consts::TAU).sin() * 0.60;
                let chime2 = (t * 1760.0 * std::f32::consts::TAU).sin() * 0.30;
                samples.push((chime1 + chime2) * env * 0.35);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 26. Seismic Pulse (Deep ground tremor resonance)
        let seismic_pulse = {
            let dur = 0.55;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 5.2).exp();
                let freq = 50.0 * (-t * 2.5).exp() + 38.0;
                let tremor = (t * freq * std::f32::consts::TAU).sin() * 0.70;
                let sub = (t * 42.0 * std::f32::consts::TAU).sin() * 0.40;
                samples.push((tremor + sub) * env * 0.42);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 27. Tesla Arc (Electric snap & low-pass filtered sizzle)
        let tesla_arc = {
            let dur = 0.20;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(717);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 18.0).exp();
                let zap = ((t * 720.0 * std::f32::consts::TAU).sin() * 0.5
                    + (t * 1440.0 * std::f32::consts::TAU).sin() * 0.25).tanh();
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.20;
                let spark = filter_state * 0.35 * (-t * 30.0).exp();
                samples.push((zap * 0.55 + spark) * env * 0.36);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 28. Plasma Flamer Whoosh (Rushing thermal stream)
        let plasma_fire = {
            let dur = 0.22;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut noise = NoiseGen::new(828);
            let mut filter_state = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (-t * 11.0).exp();
                let hum = (t * 165.0 * std::f32::consts::TAU).sin() * 0.30;
                let raw_noise = noise.next_f32();
                filter_state += (raw_noise - filter_state) * 0.18;
                let hiss = filter_state * 0.35;
                samples.push((hum + hiss) * env * 0.35);
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 29. Bullet-Time Resonant Drone (40Hz deep hum with resonant slow modulation)
        let bullet_time_drone = {
            let dur = 1.4;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (1.0 - (-t * 8.0).exp()) * (-(t - dur).max(0.0) * 4.0).exp();
                let f0 = 40.0;
                let drone = (t * f0 * std::f32::consts::TAU).sin() * 0.60
                    + (t * 80.0 * std::f32::consts::TAU).sin() * 0.25
                    + (t * 120.0 * std::f32::consts::TAU).sin() * 0.12;
                let pulse = 0.85 + 0.15 * (t * 3.0 * std::f32::consts::TAU).sin();
                samples.push((drone * pulse * env * 0.45).tanh());
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 30. Capacitor Whine (Escalating pitch from 350Hz to 1400Hz over 0.85s)
        let capacitor_whine = {
            let dur = 0.85;
            let len = (sample_rate as f32 * dur) as usize;
            let mut samples = Vec::with_capacity(len);
            let mut phase = 0.0f32;
            for i in 0..len {
                let t = i as f32 / sample_rate as f32;
                let env = (t / 0.1).min(1.0) * (1.0 - (t - dur + 0.1).max(0.0) / 0.1).clamp(0.0, 1.0);
                let freq = 350.0 + 1050.0 * (t / dur).powi(2);
                phase += freq * std::f32::consts::TAU / sample_rate as f32;
                let s = phase.sin() * 0.45 + (phase * 2.0).sin() * 0.15;
                samples.push((s * env).tanh());
            }
            let wav = create_wav(&samples, sample_rate);
            load_sound_from_bytes(&wav).await.ok()
        };

        Self {
            explosion,
            big_boom,
            railgun,
            singularity_hum,
            singularity_implode,
            plasma_buzz,
            cryo_freeze,
            cryo_shatter,
            ram_impact,
            debris_clatter,
            ui_click,
            ui_switch,
            structural_groan,
            mortar_launch,
            subsonic_artillery_boom,
            hydraulic_slam,
            railgun_boom,
            singularity_launch,
            vacuum_sweep,
            stress_creak,
            jet_dash,
            land_impact,
            jump,
            hit_tick,
            hit_crit,
            seismic_pulse,
            tesla_arc,
            plasma_fire,
            bullet_time_drone,
            capacitor_whine,
            recent_plays: Mutex::new(HashMap::new()),
            recent_timestamps: Mutex::new(Vec::new()),
            master_volume: Cell::new(1.0),
            siege_volume: Cell::new(1.2),
            stress_creak_volume: Cell::new(1.0),
            anti_stacking_seconds: Cell::new(0.075),
        }
    }

    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            explosion: None,
            big_boom: None,
            railgun: None,
            singularity_hum: None,
            singularity_implode: None,
            plasma_buzz: None,
            cryo_freeze: None,
            cryo_shatter: None,
            ram_impact: None,
            debris_clatter: None,
            ui_click: None,
            ui_switch: None,
            structural_groan: None,
            mortar_launch: None,
            subsonic_artillery_boom: None,
            hydraulic_slam: None,
            railgun_boom: None,
            singularity_launch: None,
            vacuum_sweep: None,
            stress_creak: None,
            jet_dash: None,
            land_impact: None,
            jump: None,
            hit_tick: None,
            hit_crit: None,
            seismic_pulse: None,
            tesla_arc: None,
            plasma_fire: None,
            bullet_time_drone: None,
            capacitor_whine: None,
            recent_plays: Mutex::new(HashMap::new()),
            recent_timestamps: Mutex::new(Vec::new()),
            master_volume: Cell::new(1.0),
            siege_volume: Cell::new(1.2),
            stress_creak_volume: Cell::new(1.0),
            anti_stacking_seconds: Cell::new(0.075),
        }
    }

    /// Applies the workbench's effect gains to every gameplay and audition sound.
    pub fn configure(&self, master_volume: f32, siege_volume: f32, anti_stacking_ms: f32, stress_creak_volume: f32) {
        self.master_volume.set(master_volume.clamp(0.0, 2.0));
        self.siege_volume.set(siege_volume.clamp(0.5, 2.5));
        self.stress_creak_volume.set(stress_creak_volume.clamp(0.1, 2.0));
        self.anti_stacking_seconds.set(anti_stacking_ms.clamp(20.0, 200.0) as f64 / 1000.0);
    }

    fn effect_gain(&self, sound: &Sound) -> f32 {
        let is = |candidate: &Option<Sound>| candidate.as_ref().is_some_and(|s| std::ptr::eq(s, sound));
        let mut gain = self.master_volume.get();
        if is(&self.explosion) || is(&self.big_boom) || is(&self.mortar_launch)
            || is(&self.subsonic_artillery_boom) || is(&self.railgun_boom)
            || is(&self.singularity_implode) || is(&self.hydraulic_slam)
        {
            gain *= self.siege_volume.get();
        }
        if is(&self.stress_creak) || is(&self.structural_groan) {
            gain *= self.stress_creak_volume.get();
        }
        gain
    }

    fn allow_play(&self, id: usize, now: f64) -> bool {
        if let Ok(mut map) = self.recent_plays.lock() {
            let guard = self.anti_stacking_seconds.get();
            // Entries only need to survive for the debounce window, even for temporary sounds.
            map.retain(|_, last| now - *last < guard);
            if map.contains_key(&id) {
                return false;
            }
            map.insert(id, now);
        }
        true
    }

    pub fn play(&self, sound: Option<&Sound>, volume: f32) {
        if let Some(s) = sound {
            let volume = volume * self.effect_gain(s);
            if !volume.is_finite() || volume <= 0.0 {
                return;
            }
            let id = s as *const Sound as usize;
            let now = macroquad::time::get_time();

            // Anti-stacking concurrency guard:
            // Prevents duplicates within the configured window (75ms by default).
            if !self.allow_play(id, now) {
                return;
            }

            // Global voice-counting energy limiter:
            // Prevents massive simultaneous bursts (e.g. 5+ simultaneous explosions
            // or dozens of shattering voxels) from stacking into deafening volume spikes.
            let mut ducking = 1.0f32;
            if let Ok(mut timestamps) = self.recent_timestamps.lock() {
                timestamps.retain(|&t| now - t < 0.160);
                ducking = Self::calculate_concurrency_ducking(timestamps.len());
                timestamps.push(now);
            }

            // Master volume scaling (0.30x) ensures all sound elements stay pleasantly balanced and never deafen
            let effective_volume = (volume * 0.30 * ducking).clamp(0.0, 1.0);

            play_sound(
                s,
                PlaySoundParams {
                    looped: false,
                    volume: effective_volume,
                },
            );
        }
    }

    pub fn calculate_concurrency_ducking(recent_count: usize) -> f32 {
        1.0 / (1.0 + recent_count as f32 * 0.50)
    }

    pub fn calculate_distance_attenuation(source_pos: macroquad::prelude::Vec3, listener_pos: macroquad::prelude::Vec3) -> f32 {
        let dist = (source_pos - listener_pos).length();
        (1.0 / (1.0 + (dist / 8.0).powf(1.8))).clamp(0.005, 1.0)
    }

    pub fn play_spatial(&self, sound: Option<&Sound>, source_pos: macroquad::prelude::Vec3, listener_pos: macroquad::prelude::Vec3, base_volume: f32) {
        let atten = Self::calculate_distance_attenuation(source_pos, listener_pos);
        self.play(sound, base_volume * atten);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anti_stacking_guard_throttles_rapid_duplicates() {
        let audio = SoundSystem::empty();
        let dummy_id = 0x12345usize;
        assert!(audio.allow_play(dummy_id, 1.000));
        assert!(!audio.allow_play(dummy_id, 1.030));
        assert!(audio.allow_play(dummy_id, 1.085));
        audio.configure(0.0, 2.0, 150.0, 1.5);
        assert!(!audio.allow_play(dummy_id, 1.200), "Configured 150ms guard must affect real playback");
        assert!(audio.allow_play(dummy_id, 1.240));
        assert_eq!(audio.master_volume.get(), 0.0);
        assert_eq!(audio.siege_volume.get(), 2.0);
        assert_eq!(audio.stress_creak_volume.get(), 1.5);
    }

    #[test]
    fn test_debounce_expires_temporary_sound_entries() {
        let audio = SoundSystem::empty();
        for id in 0..1000 {
            assert!(audio.allow_play(id, id as f64));
        }
        assert_eq!(audio.recent_plays.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_concurrency_energy_limiter_attenuates_bursts() {
        // Zero recent voices: full energy (1.0)
        let duck0 = SoundSystem::calculate_concurrency_ducking(0);
        assert!((duck0 - 1.0).abs() < 0.001);

        // 3 recent voices in rapid succession: smoothly ducked to ~0.40
        let duck3 = SoundSystem::calculate_concurrency_ducking(3);
        assert!((duck3 - 0.40).abs() < 0.01, "3 simultaneous voices must be ducked to ~0.40, got {}", duck3);

        // 8 recent voices (major chain explosion): ducked to ~0.20
        let duck8 = SoundSystem::calculate_concurrency_ducking(8);
        assert!((duck8 - 0.20).abs() < 0.01, "8 simultaneous voices must prevent volume blowout, got {}", duck8);
    }

    #[test]
    fn test_distance_attenuation_drops_with_distance() {
        use macroquad::prelude::Vec3;
        let listener = Vec3::new(0.0, 5.0, 20.0);

        // 0m away: full volume
        let atten_close = SoundSystem::calculate_distance_attenuation(listener, listener);
        assert!((atten_close - 1.0).abs() < 0.001);

        // 8m away: ~0.50 volume
        let source_8m = listener + Vec3::new(8.0, 0.0, 0.0);
        let atten_8m = SoundSystem::calculate_distance_attenuation(source_8m, listener);
        assert!((atten_8m - 0.50).abs() < 0.05, "8m attenuation expected ~0.5, got {}", atten_8m);

        // 32m away: ~0.076 volume
        let source_32m = listener + Vec3::new(32.0, 0.0, 0.0);
        let atten_32m = SoundSystem::calculate_distance_attenuation(source_32m, listener);
        assert!((0.05..0.10).contains(&atten_32m), "32m attenuation expected ~0.076, got {}", atten_32m);

        // 100m away: muffled distant tail ~0.010
        let source_100m = listener + Vec3::new(100.0, 0.0, 0.0);
        let atten_100m = SoundSystem::calculate_distance_attenuation(source_100m, listener);
        assert!((0.005..0.025).contains(&atten_100m), "100m attenuation must be gentle muffled rumble, got {}", atten_100m);
    }
}
