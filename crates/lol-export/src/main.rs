//! `lol-export`: the pictures, sounds and music of an installed Lands of Lore as PNG and WAV
//! files, for looking at them and as the source of replacement graphics and sounds.
//!
//! - `lol-export liste`: the copy found, its languages and archives.
//! - `lol-export info [SPRACHE]`: every file of every archive with its kind.
//! - `lol-export skript DATEI`: what each function of a script asks of the game.
//! - `lol-export bilder [ZIEL]`: pictures, figures, animations and palettes.
//! - `lol-export klaenge [ZIEL]`: the sounds and the speech (`VOC`).
//! - `lol-export musik [ZIEL]`: the music and sounds of the AdLib card (`ADL`).
//! - `lol-export ansichten [ZIEL]`: views of every level as the original draws them.
//! - `lol-export alles [ZIEL]`: all of it, and `uebersicht.html`.
//!
//! `ZIEL` defaults to `extract`. Files shared by all languages go to `ZIEL/allgemein`, those
//! of a language to `ZIEL/de`, `ZIEL/en`, `ZIEL/fr`.

use std::path::PathBuf;

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Installation, Language};

mod export;
mod music;
mod overview;
mod sounds;
mod views;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(copy) = paths::find() else {
        eprintln!(
            "Keine Installation gefunden (Umgebungsvariable {}: Ordner oder GAME.DAT).",
            paths::LOL_ENV
        );
        std::process::exit(1);
    };
    let target = || PathBuf::from(args.get(1).map_or("extract", String::as_str));
    let result = match args.first().map(String::as_str) {
        Some("liste") | None => list(&copy),
        Some("info") => info(
            &copy,
            args.get(1)
                .and_then(|a| Language::parse(a))
                .unwrap_or(Language::German),
        ),
        Some("skript") => script(&copy, args.get(1).map_or("LEVEL1.INI", String::as_str)),
        Some("bilder") => export::all(&copy, &target()),
        Some("klaenge") => sounds::all(&copy, &target()),
        Some("ansichten") => views::all(&copy, &target()),
        Some("musik") => music::all(&copy, &target()),
        Some("alles") => {
            let target = target();
            export::all(&copy, &target)
                .and_then(|_| sounds::all(&copy, &target))
                .and_then(|_| music::all(&copy, &target))
                .and_then(|_| views::all(&copy, &target))
                .and_then(|_| overview::write(&target))
        }
        Some(other) => {
            eprintln!(
                "Unbekannter Befehl: {other} (liste, info, skript, bilder, klaenge, musik, ansichten, alles)"
            );
            std::process::exit(2);
        }
    };
    if let Err(err) = result {
        eprintln!("Fehler: {err}");
        std::process::exit(1);
    }
}

fn list(copy: &Installation) -> lol_data::Result<()> {
    println!("{} ({:?})", copy.name(), copy.medium);
    for language in Language::ALL {
        let store = AssetStore::open(copy, language)?;
        let own: Vec<&str> = store
            .archives()
            .iter()
            .filter(|a| a.language.is_some())
            .map(|a| a.name.as_str())
            .collect();
        println!("  {}: {}", language.label(), own.join(" "));
    }
    let store = AssetStore::open(copy, Language::German)?;
    let shared: Vec<String> = store
        .archives()
        .iter()
        .filter(|a| a.language.is_none())
        .map(|a| format!("{} ({})", a.name, a.pak.entries.len()))
        .collect();
    println!("  Gemeinsam: {}", shared.join(" "));
    let loose: Vec<&str> = store.names().filter(|n| !n.ends_with(".PAK")).collect();
    println!("  Lose: {}", loose.join(" "));
    Ok(())
}

/// Runs every function of a script once, printing the game's functions it calls.
fn script(copy: &Installation, name: &str) -> lol_data::Result<()> {
    let store = AssetStore::open(copy, Language::German)?;
    let script = lol_data::emc::Script::parse(name, &store.read(name)?)?;
    println!("{name}: {}", script.texts().join(" | "));
    for function in (0..2048).filter(|&f| script.has_function(f)) {
        println!("Funktion {function}:");
        for call in lol_engine::trace::trace(&script, function) {
            println!("  {call}");
        }
    }
    Ok(())
}

/// Every file of every archive, with its size and what it seems to be.
fn info(copy: &Installation, language: Language) -> lol_data::Result<()> {
    let store = AssetStore::open(copy, language)?;
    for archive in store.archives() {
        println!(
            "== {} ({} Dateien)",
            archive.name,
            archive.pak.entries.len()
        );
        for (name, data) in archive.files() {
            println!("  {name:14} {:8} Byte  {}", data.len(), kind(name, data));
        }
    }
    Ok(())
}

/// A short description of a file's contents, for `info`.
fn kind(name: &str, data: &[u8]) -> String {
    let ext = name.rsplit('.').next().unwrap_or("");
    let word = |i: usize| {
        data.get(i..i + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    match ext {
        "CPS" | "VCN" | "VMP" => match (word(0), word(2), word(8)) {
            (Some(size), Some(c), Some(pal)) if size + 2 == data.len() => {
                format!("Bildkopf: Kompression {c}, Palette {pal}")
            }
            _ => "ohne Bildkopf".into(),
        },
        "SHP" => match lol_data::shp::parse_file(name, data) {
            Ok(shapes) => format!("{} Figuren", shapes.len()),
            Err(err) => format!("FEHLER {err}"),
        },
        "WSA" => match lol_data::wsa::Wsa::parse(name, data) {
            Ok(w) => format!(
                "{} Bilder {}×{}{}",
                w.frames.len(),
                w.width,
                w.height,
                if w.palette.is_some() { ", Palette" } else { "" }
            ),
            Err(err) => format!("FEHLER {err}"),
        },
        "VOC" => match lol_data::voc::parse(name, data) {
            Ok(s) => format!(
                "{} Hz, {:.1} s",
                s.rate,
                s.samples.len() as f32 / s.rate as f32
            ),
            Err(err) => format!("FEHLER {err}"),
        },
        _ => String::new(),
    }
}
