//! The game's sound to the speakers: Bevy's audio pulls the samples the engine's mixer makes
//! (the AdLib music and effects, the voices).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use lol_engine::audio::Mixer;

/// Samples rendered at a time (about 23 ms at 44.1 kHz).
const BLOCK: usize = 1024;

#[derive(Asset, TypePath)]
pub struct MixerStream {
    mixer: Arc<Mutex<Mixer>>,
}

pub struct MixerDecoder {
    mixer: Arc<Mutex<Mixer>>,
    rate: u32,
    buffer: Vec<i16>,
    next: usize,
}

impl Iterator for MixerDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.next == self.buffer.len() {
            match self.mixer.lock() {
                Ok(mut m) => m.render(&mut self.buffer),
                Err(_) => self.buffer.fill(0),
            }
            self.next = 0;
        }
        let s = self.buffer[self.next];
        self.next += 1;
        Some(s as f32 / 32768.0)
    }
}

impl Source for MixerDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("one channel")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.rate).expect("non-zero rate")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for MixerStream {
    type Decoder = MixerDecoder;

    fn decoder(&self) -> MixerDecoder {
        let rate = self.mixer.lock().map_or(44_100, |m| m.rate());
        MixerDecoder {
            mixer: self.mixer.clone(),
            rate,
            buffer: vec![0; BLOCK],
            next: BLOCK,
        }
    }
}

pub struct SoundPlugin(pub Arc<Mutex<Mixer>>);

#[derive(Resource)]
struct MixerHandle(Arc<Mutex<Mixer>>);

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<MixerStream>()
            .insert_resource(MixerHandle(self.0.clone()))
            .add_systems(Startup, setup);
    }
}

fn setup(
    mut commands: Commands,
    mixer: Res<MixerHandle>,
    mut streams: ResMut<Assets<MixerStream>>,
) {
    let stream = streams.add(MixerStream {
        mixer: mixer.0.clone(),
    });
    commands.spawn(AudioPlayer(stream));
}
