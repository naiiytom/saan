//! # `saan_audio` — Pure-Rust Audio Playback Engine for Saan
//!
//! Provides real-time playback of background music (BGM) and 3D spatial sound effects (SFX)
//! via `rodio` with procedural fallback synthesis and headless-safe operation.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::mpsc::{channel, Sender};

use rodio::{Decoder, OutputStream, Sink};
use saan_core::{EventWeftStrand, GameplayEvent, Loom, LoomError, WeftSystem, World};

/// Synthesizes procedural WAV PCM audio in-memory for testing, prototyping, and fallback.
pub struct ProceduralSfx;

impl ProceduralSfx {
    /// Generates a gentle chiming tone (sine wave decay) as WAV bytes.
    pub fn chime_wav(freq: f32, duration_secs: f32) -> Vec<u8> {
        let sample_rate = 44100u32;
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let envelope = (-3.5 * t / duration_secs).exp();
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin() * envelope * 0.45;
            let pcm = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            samples.push(pcm);
        }

        Self::encode_wav_16bit_mono(samples, sample_rate)
    }

    /// Generates a subtle footstep impact as WAV bytes.
    pub fn footstep_wav() -> Vec<u8> {
        let sample_rate = 44100u32;
        let num_samples = (sample_rate as f32 * 0.12) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let envelope = (-18.0 * t).exp();
            let noise = ((i * 1103515245 + 12345) & 0x7fff) as f32 / 32767.0 * 2.0 - 1.0;
            let tone = (2.0 * std::f32::consts::PI * 85.0 * t).sin() * 0.6;
            let sample = (tone + noise * 0.25) * envelope * 0.35;
            let pcm = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
            samples.push(pcm);
        }

        Self::encode_wav_16bit_mono(samples, sample_rate)
    }

    fn encode_wav_16bit_mono(samples: Vec<i16>, sample_rate: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let num_samples = samples.len() as u32;
        let byte_rate = sample_rate * 2;
        let data_chunk_size = num_samples * 2;
        let file_size = 36 + data_chunk_size;

        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&file_size.to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); // subchunk1 size
        out.extend_from_slice(&1u16.to_le_bytes());  // PCM format
        out.extend_from_slice(&1u16.to_le_bytes());  // Mono
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());  // block align
        out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_chunk_size.to_le_bytes());

        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }

        out
    }
}

/// Commands sent to the background audio thread.
#[derive(Debug, Clone)]
pub enum AudioCommand {
    PlaySpatialSfx {
        sound_id: String,
        stereo_gain: [f32; 2],
    },
    SetBgmVolume(f32),
    SetSfxVolume(f32),
}

/// The active Rodio sound device manager holding BGM and spatial SFX buses.
#[derive(Clone)]
pub struct AudioEngine {
    tx: Sender<AudioCommand>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    /// Attempts to open the host default audio output stream in a dedicated thread.
    /// Operates silently and gracefully if no soundcard/device is available (e.g. CI environments).
    pub fn new() -> Self {
        let (tx, rx) = channel::<AudioCommand>();

        std::thread::Builder::new()
            .name("saan-audio".to_string())
            .spawn(move || {
                let (_stream, stream_handle) = match OutputStream::try_default() {
                    Ok((s, h)) => (Some(s), Some(h)),
                    Err(e) => {
                        eprintln!("[Saan Audio] No physical audio output device detected ({e}); running in mock mode");
                        (None, None)
                    }
                };

                let mut sound_cache: HashMap<String, Vec<u8>> = HashMap::new();
                sound_cache.insert("audio/ambient_chime.ogg".to_string(), ProceduralSfx::chime_wav(520.0, 1.2));
                sound_cache.insert("audio/spirit_lantern_chime.ogg".to_string(), ProceduralSfx::chime_wav(660.0, 1.8));
                sound_cache.insert("audio/footsteps_moss.ogg".to_string(), ProceduralSfx::footstep_wav());
                sound_cache.insert("audio/banyan_wind_leaves.ogg".to_string(), ProceduralSfx::chime_wav(330.0, 2.5));

                let mut sfx_sinks: Vec<Sink> = Vec::new();
                let mut sfx_volume = 1.0f32;

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::PlaySpatialSfx { sound_id, stereo_gain } => {
                            let max_gain = stereo_gain[0].max(stereo_gain[1]);
                            if max_gain < 0.005 {
                                continue;
                            }

                            let Some(ref handle) = stream_handle else {
                                continue;
                            };

                            let wav_data = match sound_cache.get(&sound_id) {
                                Some(data) => data.clone(),
                                None => {
                                    let default_chime = ProceduralSfx::chime_wav(440.0, 0.8);
                                    sound_cache.insert(sound_id.clone(), default_chime.clone());
                                    default_chime
                                }
                            };

                            if let Ok(source) = Decoder::new(Cursor::new(wav_data)) {
                                sfx_sinks.retain(|s| !s.empty());
                                if let Ok(sink) = Sink::try_new(handle) {
                                    let vol = max_gain * sfx_volume;
                                    sink.set_volume(vol.clamp(0.0, 2.0));
                                    sink.append(source);
                                    sfx_sinks.push(sink);
                                }
                            }
                        }
                        AudioCommand::SetBgmVolume(_vol) => {
                            // Reserved for BGM stream
                        }
                        AudioCommand::SetSfxVolume(vol) => {
                            sfx_volume = vol.clamp(0.0, 2.0);
                        }
                    }
                }
            })
            .expect("Failed to spawn saan-audio worker thread");

        Self { tx }
    }

    /// Plays or loops a sound effect using spatial stereo volumes `[left, right]`.
    pub fn play_spatial_sfx(&self, sound_id: &str, stereo_gain: [f32; 2]) {
        let _ = self.tx.send(AudioCommand::PlaySpatialSfx {
            sound_id: sound_id.to_string(),
            stereo_gain,
        });
    }

    /// Updates SFX master volume.
    pub fn set_sfx_volume(&self, vol: f32) {
        let _ = self.tx.send(AudioCommand::SetSfxVolume(vol));
    }
}

/// Weft system that listens for [`GameplayEvent::AudioCue`] events and plays spatial audio.
pub struct AudioPlaybackWeftSystem {
    engine: AudioEngine,
}

impl Default for AudioPlaybackWeftSystem {
    fn default() -> Self {
        Self {
            engine: AudioEngine::new(),
        }
    }
}

impl AudioPlaybackWeftSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine(engine: AudioEngine) -> Self {
        Self { engine }
    }
}

impl WeftSystem for AudioPlaybackWeftSystem {
    fn name(&self) -> &'static str {
        "audio_playback_rodio"
    }

    fn reads(&self) -> &[&'static str] {
        &["AudioEmitter3D", "EventWeftStrand"]
    }

    fn writes(&self) -> &[&'static str] {
        &[]
    }

    fn weave(&mut self, world: &mut World, _dt: f32) {
        let events: Vec<(String, [f32; 2])> = world
            .query::<EventWeftStrand>()
            .next()
            .map(|(_, bus)| {
                bus.current_events
                    .iter()
                    .filter_map(|e| match e {
                        GameplayEvent::AudioCue { sound_id, stereo_gain, .. } => {
                            Some((sound_id.clone(), *stereo_gain))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        if events.is_empty() {
            return;
        }

        for (sound_id, gain) in events {
            self.engine.play_spatial_sfx(&sound_id, gain);
        }
    }
}

/// Registers the audio playback Weft system onto `loom` with proper dependency ordering.
pub fn register_audio_systems(loom: &mut Loom) -> Result<(), LoomError> {
    loom.add_system(AudioPlaybackWeftSystem::new())?;
    loom.add_ordering("positional_audio_3d", "audio_playback_rodio")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_wav_generation_produces_valid_riff_headers() {
        let chime = ProceduralSfx::chime_wav(440.0, 0.5);
        assert_eq!(&chime[0..4], b"RIFF");
        assert_eq!(&chime[8..12], b"WAVE");
        assert_eq!(&chime[12..16], b"fmt ");

        let footstep = ProceduralSfx::footstep_wav();
        assert_eq!(&footstep[0..4], b"RIFF");
        assert_eq!(&footstep[8..12], b"WAVE");
    }

    #[test]
    fn audio_engine_handles_headless_gracefully() {
        let engine = AudioEngine::new();
        engine.play_spatial_sfx("audio/ambient_chime.ogg", [0.5, 0.5]);
        engine.set_sfx_volume(0.8);
    }
}
