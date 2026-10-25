//! Tests against the player's own copy; each passes quietly when none is installed.

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_engine::game::{Game, START};

fn new_game() -> Option<Game> {
    let Some(copy) = paths::find() else {
        eprintln!("Keine Installation gefunden, Test übersprungen.");
        return None;
    };
    let store = AssetStore::open(&copy, Language::German).expect("Installation lesbar");
    Some(Game::new_game(store).expect("Ebene 1 lädt"))
}

#[test]
fn the_first_level_is_gladstone_keep() {
    let Some(game) = new_game() else { return };
    assert_eq!(game.level.graphics, "KEEP");
    assert_eq!((game.level.index, game.block, game.direction), START);
    assert!(game.level.doors.is_some());
    assert!(game.level.script.is_some());
    let screen = game.render();
    assert_eq!(screen.pixels.len(), 320 * 200);
    // The view shows more than a few colours: walls, floor and ceiling were drawn.
    let mut colours: Vec<u8> = (0..120)
        .flat_map(|y| screen.pixels[y * 320 + 112..y * 320 + 288].to_vec())
        .collect();
    colours.sort_unstable();
    colours.dedup();
    assert!(colours.len() > 16, "{} Farben", colours.len());
}

#[test]
fn every_level_loads() {
    let Some(mut game) = new_game() else { return };
    for index in 1..=29 {
        game.change_level(index)
            .unwrap_or_else(|err| panic!("Ebene {index}: {err}"));
        assert_eq!(game.level.index, index);
        let _ = game.view();
    }
}

#[test]
fn walls_stop_the_party() {
    let Some(mut game) = new_game() else { return };
    // Turning round four times comes back; walking into the wall ahead does not move.
    for _ in 0..4 {
        game.turn(true);
    }
    assert_eq!(game.direction, START.2);
    let mut moved = 0;
    for _ in 0..40 {
        if game.walk(0) {
            moved += 1;
        }
    }
    assert!(moved < 32, "40 Schritte ohne Wand");
}
