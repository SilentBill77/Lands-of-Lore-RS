//! `uebersicht.html` in the export's folder: the pictures (whole files and sheets, not the
//! single figures and frames) with sharp pixels at twice their size, and players for the
//! music, the sounds and the speech, grouped by folder.

use std::collections::BTreeMap;
use std::path::Path;

use lol_data::Result;

/// Folders whose files are left out: the single figures and frames beside each sheet.
fn single_frames(group: &str) -> bool {
    group.ends_with(".SHP") || group.ends_with(".WSA")
}

pub fn write(target: &Path) -> Result<()> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    collect(target, target, &mut found);
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"de\"><head><meta charset=\"utf-8\">\
         <title>Lands of Lore – Übersicht</title><style>\
         body{background:#202028;color:#ddd;font:14px sans-serif;margin:16px}\
         h2{margin:24px 0 8px;font-size:16px}summary{cursor:pointer;font-size:15px;margin:6px 0}\
         figure{display:inline-block;margin:4px;vertical-align:top;text-align:center}\
         img{image-rendering:pixelated;zoom:2;background:#f0f;display:block}\
         figcaption{font-size:11px;max-width:320px;overflow-wrap:anywhere}\
         .ton{display:inline-flex;align-items:center;gap:6px;margin:2px 12px 2px 0;font-size:12px}\
         audio{height:28px}\
         </style></head><body><h1>Lands of Lore – Bilder und Klänge</h1>\n\
         <p>Magenta zeigt durchsichtige Stellen. Einzelne Figuren und Bilder der Animationen \
         liegen in den Ordnern neben den Bögen.</p>\n",
    );
    for (group, files) in &found {
        let sound = files[0].ends_with(".wav");
        // The speech has thousands of files: folded away per folder.
        let folded = group.starts_with("sprache");
        if folded {
            html.push_str(&format!(
                "<details><summary>{group} ({})</summary>\n",
                files.len()
            ));
        } else {
            html.push_str(&format!("<h2>{group} ({})</h2>\n", files.len()));
        }
        for file in files {
            let name = file.rsplit('/').next().unwrap_or(file);
            if sound {
                html.push_str(&format!(
                    "<span class=\"ton\"><audio controls preload=\"none\" src=\"{file}\"></audio>{name}</span>\n"
                ));
            } else {
                html.push_str(&format!(
                    "<figure><img src=\"{file}\" loading=\"lazy\"><figcaption>{name}</figcaption></figure>\n"
                ));
            }
        }
        if folded {
            html.push_str("</details>\n");
        }
    }
    html.push_str("</body></html>\n");
    let path = target.join("uebersicht.html");
    std::fs::write(&path, html).map_err(|e| lol_data::Error::io(path.display(), e))?;
    println!("Übersicht: {}", path.display());
    Ok(())
}

fn collect(dir: &Path, base: &Path, found: &mut BTreeMap<String, Vec<String>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, base, found);
            continue;
        }
        if !path.extension().is_some_and(|e| e == "png" || e == "wav") {
            continue;
        }
        let relative = path.strip_prefix(base).unwrap_or(&path);
        let group = relative
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if single_frames(&group) {
            continue;
        }
        found
            .entry(group)
            .or_default()
            .push(relative.to_string_lossy().replace('\\', "/"));
    }
}
