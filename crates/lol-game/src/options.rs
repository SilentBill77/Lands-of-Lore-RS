//! The command line: where to begin and how to run (for trying things out and test runs).

use bevy::prelude::Resource;
use lol_data::paths::Language;

#[derive(Resource, Debug, Clone)]
pub struct Options {
    pub language: Language,
    pub level: Option<usize>,
    pub block: Option<usize>,
    pub direction: Option<usize>,
    /// Moves right after the start (`V` ahead, `Z` back, `A`/`D` aside, `L`/`R` turn).
    pub moves: String,
    /// Writes the screen as it would show to this file, then ends (after `frames` frames).
    pub screenshot: Option<String>,
    pub frames: u32,
    pub window_size: (u32, u32),
}

impl Default for Options {
    fn default() -> Self {
        Self {
            language: Language::German,
            level: None,
            block: None,
            direction: None,
            moves: String::new(),
            screenshot: None,
            frames: 3,
            window_size: (1280, 960),
        }
    }
}

pub const USAGE: &str = "Aufruf: lol-game [--sprache de|en|fr] [--ebene N] [--pos BLOCK] \
[--richtung N|O|S|W] [--wege VZADLR…] [--screenshot DATEI] [--frames N] [--groesse BxH]";

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
                "--ebene" => options.level = Some(number(&value()?, 1, 29)?),
                "--pos" => options.block = Some(number(&value()?, 0, 1023)?),
                "--richtung" => {
                    let v = value()?.to_ascii_uppercase();
                    options.direction = Some(
                        ["N", "O", "S", "W"]
                            .iter()
                            .position(|d| *d == v)
                            .ok_or_else(|| format!("Richtung N, O, S oder W, nicht {v}"))?,
                    );
                }
                "--wege" => options.moves = value()?.to_ascii_uppercase(),
                "--screenshot" => options.screenshot = Some(value()?),
                "--frames" => options.frames = number(&value()?, 1, 100_000)? as u32,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_command_line() {
        let args = [
            "--ebene",
            "3",
            "--pos",
            "300",
            "--richtung",
            "w",
            "--wege",
            "vvl",
        ];
        let options = Options::from_args(args.iter().map(|s| s.to_string())).unwrap();
        assert_eq!(options.level, Some(3));
        assert_eq!(options.block, Some(300));
        assert_eq!(options.direction, Some(3));
        assert_eq!(options.moves, "VVL");
        assert!(Options::from_args(["--ebene", "30"].iter().map(|s| s.to_string())).is_err());
    }
}
