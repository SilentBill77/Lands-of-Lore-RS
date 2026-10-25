//! The command line and the settings kept between runs.

use std::path::PathBuf;

use bevy::prelude::Resource;
use lol_data::paths::Language;
use lol_engine::lol::main_loop::Settings;

#[derive(Resource, Debug, Clone)]
pub struct Options {
    pub language: Language,
    /// A saved game to start with (its slot).
    pub load: Option<i32>,
    pub window_size: (u32, u32),
    /// The window as large as the screen.
    pub fullscreen: bool,
    /// The installed copy's folder (or its `GAME.DAT`), when it is not found by itself.
    pub data: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            language: Language::German,
            load: None,
            window_size: (1280, 960),
            fullscreen: false,
            data: None,
        }
    }
}

pub const USAGE: &str = "Aufruf: lol-game [--daten ORDNER] [--sprache de|en|fr] [--laden NR] [--groesse BxH] [--vollbild]";

impl Options {
    pub fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{arg} braucht einen Wert.\n{USAGE}"))
            };
            match arg.as_str() {
                "--sprache" => {
                    let v = value()?;
                    options.language =
                        Language::parse(&v).ok_or_else(|| format!("Unbekannte Sprache: {v}"))?;
                }
                "--laden" => options.load = Some(number(&value()?, 0, 989)? as i32),
                "--groesse" => {
                    let v = value()?;
                    let (w, h) = v
                        .split_once(['x', 'X'])
                        .ok_or_else(|| format!("Größe als BREITExHÖHE, nicht {v}"))?;
                    options.window_size = (
                        number(w, 320, 10_000)? as u32,
                        number(h, 200, 10_000)? as u32,
                    );
                }
                "--vollbild" => options.fullscreen = true,
                "--daten" => options.data = Some(PathBuf::from(value()?)),
                "--hilfe" | "-h" | "--help" => return Err(USAGE.to_string()),
                other => return Err(format!("Unbekannte Option: {other}\n{USAGE}")),
            }
        }
        Ok(options)
    }
}

fn number(text: &str, min: usize, max: usize) -> Result<usize, String> {
    text.parse::<usize>()
        .ok()
        .filter(|n| (min..=max).contains(n))
        .ok_or_else(|| format!("Zahl von {min} bis {max} erwartet, nicht {text}"))
}

/// Where the program keeps its files: `%APPDATA%\Lands-of-Lore-RS` (saved games, settings).
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("Lands-of-Lore-RS"))
}

/// The game's settings as `key=value` lines.
pub fn read_settings(path: &PathBuf) -> Option<Settings> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut s = Settings {
        monster_difficulty: 1,
        smooth_scrolling: true,
        floating_cursors: false,
        config_voice: 2,
        music_volume: 192,
        sfx_volume: 192,
        speech_volume: 255,
    };
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let n = v.trim().parse::<i32>().unwrap_or(0);
        match k.trim() {
            "monster_difficulty" => s.monster_difficulty = n,
            "smooth_scrolling" => s.smooth_scrolling = n != 0,
            "floating_cursors" => s.floating_cursors = n != 0,
            "voice" => s.config_voice = n,
            "music_volume" => s.music_volume = n.clamp(0, 255) as u8,
            "sfx_volume" => s.sfx_volume = n.clamp(0, 255) as u8,
            "speech_volume" => s.speech_volume = n.clamp(0, 255) as u8,
            _ => {}
        }
    }
    Some(s)
}

pub fn write_settings(path: &PathBuf, s: &Settings) {
    let text = format!(
        "monster_difficulty={}\nsmooth_scrolling={}\nfloating_cursors={}\nvoice={}\nmusic_volume={}\nsfx_volume={}\nspeech_volume={}\n",
        s.monster_difficulty,
        s.smooth_scrolling as i32,
        s.floating_cursors as i32,
        s.config_voice,
        s.music_volume,
        s.sfx_volume,
        s.speech_volume
    );
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_command_line() {
        let args = [
            "--sprache",
            "en",
            "--laden",
            "3",
            "--groesse",
            "800x600",
            "--daten",
            r"E:\LoL",
        ];
        let options = Options::from_args(args.iter().map(|s| s.to_string())).unwrap();
        assert_eq!(options.language, Language::English);
        assert_eq!(options.load, Some(3));
        assert_eq!(options.window_size, (800, 600));
        assert_eq!(options.data, Some(PathBuf::from(r"E:\LoL")));
        assert!(Options::from_args(["--laden", "x"].iter().map(|s| s.to_string())).is_err());
    }
}
