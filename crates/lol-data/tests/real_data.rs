//! Tests against the player's own copy; each passes quietly when none is installed.

use lol_data::assets::AssetStore;
use lol_data::cps::Cps;
use lol_data::paths::{self, Language};
use lol_data::vcn::WallSet;
use lol_data::wsa::Wsa;
use lol_data::{shp, voc};

fn store(language: Language) -> Option<AssetStore> {
    let Some(copy) = paths::find() else {
        eprintln!("Keine Installation gefunden, Test übersprungen.");
        return None;
    };
    Some(AssetStore::open(&copy, language).expect("Installation lesbar"))
}

#[test]
fn every_language_has_its_archives() {
    for language in Language::ALL {
        let Some(store) = store(language) else { return };
        for name in [
            "GENERAL.PAK",
            "STARTUP.PAK",
            "INTRO1.PAK",
            "L01.PAK",
            "MUSIC.PAK",
        ] {
            assert!(store.archive(name).is_some(), "{name} fehlt ({language:?})");
        }
        let general = store.archive("GENERAL.PAK").unwrap();
        assert_eq!(general.language, Some(language));
        assert_eq!(store.names_with("TLK").len(), 30);
    }
}

#[test]
fn every_picture_figure_and_animation_reads() {
    let Some(store) = store(Language::German) else {
        return;
    };
    let mut counts = [0usize; 5];
    for archive in store.archives() {
        for (name, data) in archive.files() {
            let what = format!("{}/{name}", archive.name);
            match name.rsplit('.').next().unwrap_or("") {
                "CPS" => {
                    let cps = Cps::parse(name, data).expect(&what);
                    assert_eq!(cps.pixels.len(), 320 * 200, "{what}");
                    counts[0] += 1;
                }
                "SHP" => {
                    let shapes = shp::parse_file(name, data).expect(&what);
                    assert!(shapes.iter().any(Option::is_some), "{what}");
                    counts[1] += 1;
                }
                "WSA" => {
                    let wsa = Wsa::parse(name, data).expect(&what);
                    assert!(!wsa.frames.is_empty(), "{what}");
                    counts[2] += 1;
                }
                "VCN" => {
                    let set = WallSet::parse(name, data).expect(&what);
                    assert!(!set.blocks.is_empty(), "{what}");
                    counts[3] += 1;
                }
                "VOC" => {
                    let sound = voc::parse(name, data).expect(&what);
                    assert!(sound.rate > 4000, "{what}");
                    counts[4] += 1;
                }
                _ => {}
            }
        }
    }
    // As many as the German copy holds.
    assert_eq!(counts, [135, 119, 298, 13, 342]);
}

#[test]
fn the_playfield_brings_its_palette() {
    let Some(store) = store(Language::German) else {
        return;
    };
    let field = Cps::parse("PLAYFLD.CPS", &store.read("PLAYFLD.CPS").unwrap()).unwrap();
    assert!(field.palette.is_some());
}

#[test]
fn the_speech_reads() {
    let Some(store) = store(Language::German) else {
        return;
    };
    let tlk = store.open_archive("00.TLK").unwrap();
    let (name, data) = tlk.files().nth(1).unwrap();
    let sound = voc::parse(name, data).unwrap();
    assert_eq!(sound.rate, 22_222);
    assert!(!sound.samples.is_empty());
}
