//! Lists the files of an archive of the game: `paklist <archive> [filter]`.

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let copy = paths::find().expect("installation");
    let store = AssetStore::open(&copy, Language::German).expect("store");
    let filter = args
        .get(2)
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let archive = match store.archive(&args[1]) {
        Some(a) => a.clone(),
        None => store.open_archive(&args[1]).expect("archive"),
    };
    for (name, data) in archive.files() {
        if name.contains(&filter) {
            println!("{name} {}", data.len());
        }
    }
}
