pub use crate::audio::pcm_to_wav_bytes;
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use std::f32::consts::TAU;

/// 16-step arpeggiator note scale in Hz (D minor industrial dark-synth pentatonic)
pub const DARK_SYNTH_ARP_SCALE: [f32; 16] = [
    146.83, // D3
    174.61, // F3
    220.00, // A3
    261.63, // C4
    293.66, // D4
    261.63, // C4
    220.00, // A3
    174.61, // F3
    164.81, // E3
    196.00, // G3
    246.94, // B3
    293.66, // D4
    349.23, // F4
    293.66, // D4
    220.00, // A3
    196.00, // G3
];

/// Chord roots for atmospheric pad drone (D minor, Bb major, G minor, A minor)
pub const PAD_CHORD_FREQS: [[f32; 3]; 4] = [
    [146.83, 174.61, 220.00], // Dm: D3, F3, A3
    [116.54, 146.83, 174.61], // Bb: Bb2, D3, F3
    [98.00, 116.54, 146.83],  // Gm: G2, Bb2, D3
    [110.00, 130.81, 164.81], // Am: A2, C3, E3
];

/// Noise generator for percussion & snare transients
#[derive(Clone, Copy)]
pub struct SynthNoiseGen {
    seed: u32,
}

impl SynthNoiseGen {
    pub const fn new(seed: u32) -> Self {
        Self { seed }
    }

    #[inline(always)]
    pub fn next_f32(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) as f32 / 16777216.0) * 2.0 - 1.0
    }
}

/// Dynamic musical reactivity parameters
#[derive(Debug, Clone, Copy)]
pub struct MusicState {
    pub stress_ratio: f32,       // 0.0 = calm, 1.0 = catastrophic mega-collapse
    pub is_bullet_time: bool,    // Slow-motion tactical dilation active
    pub bullet_time_speed: f32,  // Typically 0.05 to 0.40
    pub master_volume: f32,      // 0.0 to 1.0
    pub bass_boost: f32,         // 1.0 to 2.5
    pub muted: bool,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            stress_ratio: 0.0,
            is_bullet_time: false,
            bullet_time_speed: 0.20,
            master_volume: 0.65,
            bass_boost: 1.30,
            muted: false,
        }
    }
}

/// Algorithmic 4-Channel Dark-Synth Synthesizer
pub struct ProceduralDarkSynth {
    pub sample_rate: u32,
    pub base_bpm: f32,
}

impl ProceduralDarkSynth {
    pub fn new(sample_rate: u32, base_bpm: f32) -> Self {
        Self {
            sample_rate,
            base_bpm,
        }
    }

    /// Calculates effective BPM scaled by structural stress (+0% to +25%)
    pub fn calculate_bpm(&self, stress_ratio: f32, is_bullet_time: bool) -> f32 {
        if is_bullet_time {
            // Half-time slow-mo pulse
            self.base_bpm * 0.50
        } else {
            let stress = stress_ratio.clamp(0.0, 1.0);
            self.base_bpm * (1.0 + 0.25 * stress)
        }
    }

    /// Channel 1: Sub-Bass Generator (Resonant FM saw/triangle with pitch drop & drive)
    pub fn synthesize_sub_bass(
        &self,
        t: f32,
        beat: f32,
        stress: f32,
        bass_boost: f32,
        is_bullet_time: bool,
    ) -> f32 {
        let f0 = if is_bullet_time {
            40.0 // Ultra-deep sub-resonant 40Hz drone
        } else {
            // Bassline sequence root changes every 4 beats: D1 (36.7Hz), F1 (43.6Hz), G1 (49.0Hz), A1 (55.0Hz)
            let bar = (beat / 4.0).floor() as usize % 4;
            match bar {
                0 => 36.71, // D1
                1 => 43.65, // F1
                2 => 48.99, // G1
                _ => 55.00, // A1
            }
        };

        let note_fraction = beat.fract();
        let amp_env = (1.0 - (-note_fraction * 18.0).exp()) * (-note_fraction * 2.8).exp();

        // 2-operator FM: carrier + modulator
        let mod_ratio = if is_bullet_time { 0.5 } else { 1.5 };
        let mod_index = if is_bullet_time { 0.8 } else { 1.2 + 0.8 * stress };
        let modulator = (t * f0 * mod_ratio * TAU).sin() * mod_index;

        // Carrier: saw-ish blend
        let phase = (t * f0) % 1.0;
        let saw = 2.0 * phase - 1.0;
        let sine = (t * f0 * TAU + modulator).sin();
        let raw_bass = saw * 0.45 + sine * 0.55;

        // Saturation drive
        let drive = 1.0 + stress * 0.8;
        (raw_bass * amp_env * drive * bass_boost).tanh() * 0.70
    }

    /// Channel 2: Cyber Lead Arpeggio (16-step industrial sequence with resonant filter sweeps)
    pub fn synthesize_lead_arp(&self, t: f32, step: usize, beat: f32, stress: f32, is_bullet_time: bool) -> f32 {
        if is_bullet_time {
            // In bullet-time, arpeggio dissolves into an ethereal detuned shimmer
            let shimmer = (t * 220.0 * TAU).sin() * 0.15 + (t * 221.5 * TAU).sin() * 0.15;
            return shimmer * 0.40;
        }

        let note_freq = DARK_SYNTH_ARP_SCALE[step % 16];
        let step_frac = (beat * 4.0).fract();
        let env = (-step_frac * 6.5).exp();

        // Dual detuned saw oscillators
        let phase1 = (t * note_freq) % 1.0;
        let phase2 = (t * (note_freq * 1.004)) % 1.0;
        let saw1 = 2.0 * phase1 - 1.0;
        let saw2 = 2.0 * phase2 - 1.0;
        let raw_lead = (saw1 + saw2) * 0.5;

        // Resonant dynamic filter sweep influenced by stress
        let sweep_lfo = (t * 0.5 * TAU).sin() * 0.5 + 0.5;
        let cutoff_mult = 0.5 + 0.5 * sweep_lfo + 0.3 * stress;

        (raw_lead * env * cutoff_mult).tanh() * 0.45
    }

    /// Channel 3: Industrial Percussion Engine (Punchy pitch-drop Kick, Noise Snare, Crisp Hi-Hat)
    pub fn synthesize_percussion(
        &self,
        step: usize,
        beat_frac: f32,
        sixteenth_frac: f32,
        noise_gen: &mut SynthNoiseGen,
        stress: f32,
        is_bullet_time: bool,
    ) -> f32 {
        let mut sample = 0.0f32;

        // Kick on beats 0, 1, 2, 3 (every 4 sixteenths)
        let is_kick_step = step.is_multiple_of(4) || (stress > 0.6 && step % 4 == 3 && beat_frac > 0.75);
        if is_kick_step {
            let kick_t = beat_frac;
            let kick_env = (-kick_t * 9.0).exp();
            // Pitch drop from 160Hz down to 42Hz
            let kick_freq = 42.0 + 118.0 * (-kick_t * 32.0).exp();
            let click = if kick_t < 0.015 { noise_gen.next_f32() * 0.35 } else { 0.0 };
            let kick_body = (kick_t * kick_freq * TAU).sin();
            let kick_vol = if is_bullet_time { 0.50 } else { 0.85 };
            sample += ((kick_body + click) * kick_env * kick_vol).tanh();
        }

        // Snare on beats 1 and 3 (steps 4 and 12 in a 16-step bar)
        let is_snare_step = !is_bullet_time && (step == 4 || step == 12);
        if is_snare_step {
            let snare_t = beat_frac;
            let snare_env = (-snare_t * 11.0).exp();
            let tone = (snare_t * 185.0 * TAU).sin() * 0.40;
            let noise = noise_gen.next_f32() * 0.60;
            sample += ((tone + noise) * snare_env * 0.60).tanh();
        }

        // Hi-Hat on every 16th step (crisp filtered noise pulses)
        let hat_env = (-sixteenth_frac * 28.0).exp();
        let hat_noise = noise_gen.next_f32();
        let hat_vol = if step.is_multiple_of(2) { 0.22 } else { 0.14 };
        sample += hat_noise * hat_env * hat_vol;

        sample
    }

    /// Channel 4: Atmospheric Drone & Pad (Phase-shifted dystopian chords)
    pub fn synthesize_drone_pad(&self, t: f32, beat: f32, stress: f32, is_bullet_time: bool) -> f32 {
        let chord_idx = (beat / 8.0).floor() as usize % 4;
        let freqs = PAD_CHORD_FREQS[chord_idx];

        let mut pad = 0.0f32;
        for (i, &f) in freqs.iter().enumerate() {
            let detune = 1.0 + (i as f32 - 1.0) * 0.003;
            let lfo = (t * 0.25 + i as f32 * 0.4).sin() * 0.25 + 0.75;
            let wave = (t * f * detune * TAU).sin();
            pad += wave * lfo;
        }
        pad *= 0.33;

        if is_bullet_time {
            // Emphasize deep 40Hz resonant drone
            let sub_hum = (t * 40.0 * TAU).sin() * 0.70;
            pad = pad * 0.40 + sub_hum * 0.60;
        }

        let pad_drive = 1.0 + stress * 0.5;
        (pad * pad_drive).tanh() * 0.35
    }

    /// Synthesizes a contiguous seamless looping chunk of 4-channel audio
    pub fn synthesize_loop_chunk(
        &self,
        num_bars: usize,
        state: &MusicState,
    ) -> Vec<f32> {
        let bpm = self.calculate_bpm(state.stress_ratio, state.is_bullet_time);
        let beats_per_second = bpm / 60.0;
        let seconds_per_bar = 4.0 / beats_per_second;
        let total_seconds = seconds_per_bar * num_bars as f32;
        let total_samples = (self.sample_rate as f32 * total_seconds) as usize;

        let mut samples = Vec::with_capacity(total_samples);
        let mut noise_gen = SynthNoiseGen::new(0xDEAD_BEEF);

        for i in 0..total_samples {
            let t = i as f32 / self.sample_rate as f32;
            let current_beat = t * beats_per_second;
            let step = (current_beat * 4.0).floor() as usize % 16;
            let beat_frac = current_beat.fract();
            let sixteenth_frac = (current_beat * 4.0).fract();

            // 1. Sub-Bass Channel
            let sub_bass = self.synthesize_sub_bass(
                t,
                current_beat,
                state.stress_ratio,
                state.bass_boost,
                state.is_bullet_time,
            );

            // 2. Lead Arpeggio Channel
            let lead_arp = self.synthesize_lead_arp(
                t,
                step,
                current_beat,
                state.stress_ratio,
                state.is_bullet_time,
            );

            // 3. Percussion Engine Channel
            let percussion = self.synthesize_percussion(
                step,
                beat_frac,
                sixteenth_frac,
                &mut noise_gen,
                state.stress_ratio,
                state.is_bullet_time,
            );

            // 4. Atmospheric Drone Pad Channel
            let pad = self.synthesize_drone_pad(
                t,
                current_beat,
                state.stress_ratio,
                state.is_bullet_time,
            );

            // Mix all 4 channels
            let master_mix = sub_bass * 0.90 + lead_arp * 0.65 + percussion * 0.80 + pad * 0.60;
            let driven = (master_mix * 1.15).tanh();

            // Loop crossfade smoothing at start & end of chunk (20ms window)
            let fade_len = (self.sample_rate as f32 * 0.02) as usize;
            let fade = if i < fade_len {
                i as f32 / fade_len as f32
            } else if i >= total_samples - fade_len {
                (total_samples - 1 - i) as f32 / fade_len as f32
            } else {
                1.0
            };

            samples.push(driven * fade);
        }

        samples
    }
}

/// Dynamic Music Player & Reactivity Manager
pub struct MusicEngine {
    pub synth: ProceduralDarkSynth,
    pub state: MusicState,
    pub normal_track: Option<Sound>,
    pub intense_track: Option<Sound>,
    pub slowmo_track: Option<Sound>,
    pub is_playing: bool,
    pub active_track_kind: u8, // 0 = normal, 1 = intense, 2 = slowmo
}

impl MusicEngine {
    pub async fn new() -> Self {
        let synth = ProceduralDarkSynth::new(44100, 126.0);
        let state = MusicState::default();

        // 1. Synthesize Normal Atmosphere Loop (4 bars)
        let normal_track = {
            let pcm = synth.synthesize_loop_chunk(4, &state);
            let wav = pcm_to_wav_bytes(&pcm, 44100);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 2. Synthesize High-Stress Combat Loop (4 bars with high stress)
        let mut intense_state = state;
        intense_state.stress_ratio = 0.95;
        let intense_track = {
            let pcm = synth.synthesize_loop_chunk(4, &intense_state);
            let wav = pcm_to_wav_bytes(&pcm, 44100);
            load_sound_from_bytes(&wav).await.ok()
        };

        // 3. Synthesize Slow-Mo Resonant Drone Loop (4 bars with bullet-time active)
        let mut slowmo_state = state;
        slowmo_state.is_bullet_time = true;
        let slowmo_track = {
            let pcm = synth.synthesize_loop_chunk(4, &slowmo_state);
            let wav = pcm_to_wav_bytes(&pcm, 44100);
            load_sound_from_bytes(&wav).await.ok()
        };

        Self {
            synth,
            state,
            normal_track,
            intense_track,
            slowmo_track,
            is_playing: false,
            active_track_kind: 0,
        }
    }

    #[allow(dead_code)]
    pub fn empty() -> Self {
        Self {
            synth: ProceduralDarkSynth::new(44100, 126.0),
            state: MusicState::default(),
            normal_track: None,
            intense_track: None,
            slowmo_track: None,
            is_playing: false,
            active_track_kind: 0,
        }
    }

    pub fn stop_all(&mut self) {
        if let Some(sound) = &self.normal_track {
            stop_sound(sound);
        }
        if let Some(sound) = &self.intense_track {
            stop_sound(sound);
        }
        if let Some(sound) = &self.slowmo_track {
            stop_sound(sound);
        }
        self.is_playing = false;
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.state.muted = !self.state.muted;
        if self.state.muted {
            self.stop_all();
        }
        self.state.muted
    }

    pub fn update(&mut self, stress_ratio: f32, is_bullet_time: bool) {
        self.state.stress_ratio = stress_ratio.clamp(0.0, 1.0);
        self.state.is_bullet_time = is_bullet_time;

        if self.state.muted || self.state.master_volume <= 0.0 {
            if self.is_playing {
                self.stop_all();
            }
            return;
        }

        let desired_kind = if is_bullet_time {
            2
        } else if self.state.stress_ratio >= 0.45 {
            1
        } else {
            0
        };

        let target_vol = (self.state.master_volume * 0.35).clamp(0.0, 1.0);

        if !self.is_playing || desired_kind != self.active_track_kind {
            self.stop_all();
            self.active_track_kind = desired_kind;

            let chosen_sound = match desired_kind {
                2 => self.slowmo_track.as_ref(),
                1 => self.intense_track.as_ref(),
                _ => self.normal_track.as_ref(),
            };

            if let Some(sound) = chosen_sound {
                play_sound(
                    sound,
                    PlaySoundParams {
                        looped: true,
                        volume: target_vol,
                    },
                );
                self.is_playing = true;
            }
        } else {
            let active_sound = match self.active_track_kind {
                2 => self.slowmo_track.as_ref(),
                1 => self.intense_track.as_ref(),
                _ => self.normal_track.as_ref(),
            };
            if let Some(sound) = active_sound {
                set_sound_volume(sound, target_vol);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_synth_bpm_scaling_with_stress_and_bullet_time() {
        let synth = ProceduralDarkSynth::new(44100, 120.0);
        let calm_bpm = synth.calculate_bpm(0.0, false);
        assert_eq!(calm_bpm, 120.0);

        let high_stress_bpm = synth.calculate_bpm(1.0, false);
        assert_eq!(high_stress_bpm, 150.0, "Stress should increase BPM by exactly 25%");

        let bullet_time_bpm = synth.calculate_bpm(0.5, true);
        assert_eq!(bullet_time_bpm, 60.0, "Bullet-time should cut BPM in half");
    }

    #[test]
    fn test_sub_bass_synthesis_bounds() {
        let synth = ProceduralDarkSynth::new(44100, 120.0);
        for i in 0..100 {
            let t = i as f32 * 0.01;
            let sample = synth.synthesize_sub_bass(t, t * 2.0, 0.5, 1.5, false);
            assert!(sample.is_finite());
            assert!((-1.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn test_lead_arpeggio_cycle_frequency_and_bounds() {
        let synth = ProceduralDarkSynth::new(44100, 120.0);
        for (step, &freq) in DARK_SYNTH_ARP_SCALE.iter().enumerate() {
            assert!((140.0..=360.0).contains(&freq));
            let s = synth.synthesize_lead_arp(0.1, step, 1.25, 0.5, false);
            assert!(s.is_finite());
            assert!((-1.0..=1.0).contains(&s));
        }
    }

    #[test]
    fn test_percussion_engine_generates_transients() {
        let synth = ProceduralDarkSynth::new(44100, 120.0);
        let mut noise = SynthNoiseGen::new(42);
        let kick_sample = synth.synthesize_percussion(0, 0.005, 0.02, &mut noise, 0.8, false);
        assert!(kick_sample.abs() > 0.05, "Kick should deliver punchy transient");

        let snare_sample = synth.synthesize_percussion(4, 0.01, 0.04, &mut noise, 0.5, false);
        assert!(snare_sample.abs() > 0.05, "Snare should deliver noise burst");
    }

    #[test]
    fn test_drone_pad_synthesis_and_slowmo_sub_resonance() {
        let synth = ProceduralDarkSynth::new(44100, 120.0);
        let pad_normal = synth.synthesize_drone_pad(0.5, 2.0, 0.2, false);
        assert!(pad_normal.is_finite());

        let pad_slowmo = synth.synthesize_drone_pad(0.5, 2.0, 0.2, true);
        assert!(pad_slowmo.is_finite());
    }

    #[test]
    fn test_loop_chunk_synthesis_and_wav_generation() {
        let synth = ProceduralDarkSynth::new(44100, 140.0);
        let state = MusicState::default();
        let samples = synth.synthesize_loop_chunk(1, &state);
        assert!(!samples.is_empty());

        let wav = pcm_to_wav_bytes(&samples, 44100);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        let payload_len = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(payload_len as usize, samples.len() * 2);
    }

    #[test]
    fn test_music_engine_mute_toggle() {
        let mut engine = MusicEngine::empty();
        assert!(!engine.state.muted);
        assert!(engine.toggle_mute());
        assert!(engine.state.muted);
        assert!(!engine.toggle_mute());
        assert!(!engine.state.muted);
    }

    #[test]
    fn test_music_engine_stop_all_and_track_switching() {
        let mut engine = MusicEngine::empty();
        engine.is_playing = true;
        engine.stop_all();
        assert!(!engine.is_playing);

        // When muted, update should remain stopped
        engine.state.muted = true;
        engine.update(0.8, false);
        assert!(!engine.is_playing);
    }

    #[test]
    fn test_missing_tracks_never_claim_to_be_playing() {
        let mut engine = MusicEngine::empty();
        for (stress, slowmo) in [(0.0, false), (0.8, false), (0.8, true)] {
            engine.update(stress, slowmo);
            assert!(!engine.is_playing);
        }
    }
}
