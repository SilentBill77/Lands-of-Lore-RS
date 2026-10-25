//! The music and sounds of Lands of Lore as the AdLib card played them: Westwood's driver
//! ([`adlib`], version 4) on an emulated OPL2 ([`opl`]), taken over from EOBRS.
//!
//! Which files: the music in `LORE01A.ADL` … `LORE08B.ADL` and `LOREINTR.ADL` (ScummVM's
//! `snd_loadSoundFile` picks them through `_musicTrackMap`), the sounds in `LORESFX.ADL`.

pub mod adlib;
pub mod opl;

use adlib::{AdLibDriver, CALLBACKS_PER_SECOND};

/// A rendered sound.
pub struct Rendered {
    pub samples: Vec<i16>,
    /// The music started over; it was played once through, then faded out.
    pub repeats: bool,
    /// It did not end by itself before the longest allowed time (an endless sound).
    pub endless: bool,
}

/// Seconds a sound keeps sounding after its programs end (the notes' release).
const TAIL: f32 = 1.5;
/// Seconds of the repeat played (and faded out over) after a piece was heard once through.
const FADE: f32 = 8.0;

/// Plays one sound (program number) of a sound file's data, for a driver of that `version`
/// (4 for Lands of Lore), at `rate` samples a second: until it ends, or until music has been played
/// once through (then a few seconds of its repeat, faded out), at most `max_seconds`.
pub fn render(version: u8, data: &[u8], sound: u16, rate: u32, max_seconds: f32) -> Rendered {
    let mut driver = AdLibDriver::new(version, rate);
    driver.set_sound_data(data);
    driver.start_sound(sound, 0xff);

    let tick_seconds = 1.0 / CALLBACKS_PER_SECOND as f32;
    let mut samples: Vec<i16> = Vec::new();
    let mut fraction = 0u32;
    let mut started = false;
    let mut tail: Option<f32> = None;
    let mut repeat_at: Option<usize> = None;
    let mut seconds = 0.0f32;
    let mut buffer = Vec::new();
    while seconds < max_seconds {
        driver.callback();
        fraction += rate;
        let n = (fraction / CALLBACKS_PER_SECOND) as usize;
        fraction %= CALLBACKS_PER_SECOND;
        buffer.resize(n, 0);
        driver.opl.generate(&mut buffer);
        samples.extend_from_slice(&buffer);
        seconds += tick_seconds;

        let playing = driver.playing();
        started |= playing;
        if !started {
            if seconds > 1.0 {
                return Rendered {
                    samples: Vec::new(),
                    repeats: false,
                    endless: false,
                };
            }
            continue;
        }
        if !playing {
            let left = tail.get_or_insert(TAIL);
            *left -= tick_seconds;
            if *left <= 0.0 {
                break;
            }
            continue;
        }
        // Once every channel still playing has started over at least once, the piece has been
        // heard once through.
        if repeat_at.is_none() {
            let active = driver.active();
            let jumped = driver.jumps_back;
            let any = (0..10).any(|c| active[c] && jumped[c] > 0);
            let all = (0..10).all(|c| !active[c] || jumped[c] > 0 || c == 9);
            if any && all {
                repeat_at = Some(samples.len());
            }
        }
        if let Some(at) = repeat_at
            && samples.len() >= at + (FADE * rate as f32) as usize
        {
            break;
        }
    }
    // Cut off at the longest allowed (an endless sound): faded out as well.
    let cut = seconds >= max_seconds && repeat_at.is_none() && tail.is_none();
    let fade_from = repeat_at
        .or_else(|| cut.then(|| samples.len().saturating_sub((FADE * rate as f32) as usize)));
    if let Some(at) = fade_from {
        let length = samples.len().saturating_sub(at).max(1) as f32;
        for (i, s) in samples[at..].iter_mut().enumerate() {
            *s = (*s as f32 * (1.0 - i as f32 / length)) as i16;
        }
    }
    Rendered {
        samples,
        repeats: repeat_at.is_some(),
        endless: cut,
    }
}

/// A WAV file (16-bit mono) of these samples.
pub fn wav(samples: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_wav_header() {
        let file = wav(&[1, -1], 44_100);
        assert_eq!(&file[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(file[4..8].try_into().unwrap()), 40);
        assert_eq!(u32::from_le_bytes(file[24..28].try_into().unwrap()), 44_100);
        assert_eq!(&file[44..48], &[1, 0, 0xff, 0xff]);
    }

    #[test]
    fn reads_the_track_table_of_version_4() {
        let mut file = vec![0xffu8; 500];
        file[2..4].copy_from_slice(&300u16.to_le_bytes());
        file.extend_from_slice(&[1, 2]);
        let (tracks, data) = adlib::split_sound_file(4, &file).unwrap();
        assert_eq!(tracks.len(), 250);
        assert_eq!((tracks[0], tracks[1]), (None, Some(300)));
        assert_eq!(data, &[1, 2]);
    }

    #[test]
    fn a_tiny_program_plays_a_note() {
        // Program 0 on channel 0, priority 1: instrument 0, note C (octave 4) for 20 ticks,
        // stop. Instrument 0 (program table of the first game: 150 entries).
        let programs = 150;
        let table = (programs + 1) * 2;
        let program = table;
        let instrument = program + 9;
        let mut data = vec![0u8; table];
        data[0..2].copy_from_slice(&(program as u16).to_le_bytes());
        data[programs * 2..programs * 2 + 2].copy_from_slice(&(instrument as u16).to_le_bytes());
        data.extend_from_slice(&[0, 1, 0x90, 0, 0x40, 20, 0x88, 0, 0]);
        data.extend_from_slice(&[
            0x01, 0x21, 0x00, 0x00, 0x00, 0x3f, 0x00, 0xff, 0xf4, 0x0f, 0x0f,
        ]);
        let rendered = render(1, &data, 0, 44_100, 5.0);
        assert!(!rendered.repeats);
        let loud = rendered
            .samples
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(loud > 3000, "zu leise: {loud}");
        // 20 ticks of note and the release: about a second and a half.
        let seconds = rendered.samples.len() as f32 / 44_100.0;
        assert!(seconds > 1.0 && seconds < 3.0, "{seconds} s");
    }
}
