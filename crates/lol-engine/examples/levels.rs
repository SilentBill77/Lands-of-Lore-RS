//! Loads every level in turn as the game does (scripts, graphics, monsters), lets it run a few
//! seconds of virtual time and saves a picture: `levels <out-dir> [first] [last]`.

use std::path::PathBuf;

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_engine::lol::Lol;
use lol_engine::lol::res::Res;
use lol_engine::platform::{Shared, System};
use lol_engine::screen::Screen;

fn save(shared: &Shared, path: PathBuf) {
    let frame = shared.frame.lock().unwrap().clone();
    let img = image::RgbaImage::from_raw(320, 200, frame.rgba(None)).unwrap();
    let _ = img.save(path);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&dir).unwrap();
    let first: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let last: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(29);
    let copy = paths::find().expect("installation");
    let store = AssetStore::open(&copy, Language::German).expect("store");
    let shared = Shared::new(22050);
    let system = System::with_virtual_time(shared.clone());
    let mut lol = Lol::new(Screen::new(system), Res::new(store));
    lol.init();
    lol.res.load_pak("GENERAL.PAK");
    lol.pre_init();
    lol.res.load_file_list("FILEDATA.FDT");
    lol.startup();
    lol.char_selection = 2;
    lol.startup_new();
    lol.enable_sys_timer(2);
    for level in first..=last {
        lol.load_level(level);
        // `LOL_BLOCK=n`: the walls of a block (type, flags, special type, shape).
        if let Some(b) = std::env::var("LOL_BLOCK")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
        {
            for (d, &w) in lol.level_block_properties[b].walls.iter().enumerate() {
                println!(
                    "Block {b} Seite {d}: Wand {w} Flags {:#x} Sonder {} Form {}",
                    lol.wll_wall_flags[w as usize],
                    lol.special_wall_types[w as usize],
                    lol.wll_shape_map[w as usize]
                );
            }
        }
        lol.scene_update_required = true;
        let start = lol.millis();
        while lol.millis() < start + 3000 {
            lol.timer_update();
            lol.update();
            if lol.scene_update_required {
                lol.gui_draw_scene(0);
            }
            lol.delay(16, false);
        }
        lol.gui_draw_scene(0);
        lol.screen.update_screen();
        let blocks: Vec<u16> = lol
            .monsters
            .iter()
            .filter(|m| m.hit_points > 0)
            .map(|m| m.obj.block)
            .collect();
        println!(
            "Ebene {level}: Block {} Richtung {}, Monster in {blocks:?}",
            lol.current_block, lol.current_direction
        );
        save(&shared, dir.join(format!("level{level:02}.png")));
    }
}
