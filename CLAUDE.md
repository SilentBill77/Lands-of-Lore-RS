# Lands-of-Lore-RS – Hinweise für die Arbeit am Projekt

Remake von Lands of Lore: The Throne of Chaos (Westwood, 1993) in Rust für Windows 11, nach
dem Vorbild von EOBRS (`G:\EOBRS`, gleiche Bauweise: Datenkrate mit Tests, Export, Sound,
Spiel auf Bevy 0.20, F2 Original/Modern). Lizenz GPL-3.0-or-later: ScummVM (Kyra-Engine)
darf als Referenz dienen.

## Arbeitsweise

- Antworten auf Deutsch (natürliches Deutsch, Anglizismen meiden).
  Code, Bezeichner und Commit-Messages auf Englisch, Kommentare im Stil von EOBRS.
- Committen und pushen nur auf Anweisung. Commits enden mit `Co-Authored-By: Claude …`.
- Nie einchecken: Originaldaten und alles daraus (`extract/`), große Assets (Modelle,
  Texturen, Musik), Spielstände, Einstellungsdateien, `ref/`.
- Keine Downloads ohne Erlaubnis. Cargo mit `--offline` (die Registry liegt lokal).
- Vor schwerer GPU-Arbeit (ComfyUI, lange Messreihen) fragen.
- Tests dürfen keine Einstellungsdateien verändern.
- Vorlagen und Porträts: nichts dazuerfinden, treu am Original bleiben.

## Spieldaten

GOG: `D:\GOG Games\Lands Of Lore`. Fast alles liegt in `GAME.DAT`, dem ISO-Abbild der
CD-Fassung (Sprachausgabe; Deutsch, Englisch, Französisch). Das Programm liest das Abbild
direkt (`lol-data/src/iso.rs`). Formate: `docs/FORMATE.md`.
Export: `cargo run --offline --release -p lol-export -- alles` (nach `extract/`).
`extract/` nie löschen, nur überschreiben (der Nutzer legt dort manchmal Dateien ab).
Referenz: `ref/scummvm` (Kopie aus EOBRS: `engines/kyra`, `devtools/create_kyradat`, nicht
eingecheckt). Lands of Lore steht dort in `engine/*_lol.cpp`, `graphics/screen_lol.cpp`,
`gui/gui_lol.cpp`, `script/script_lol.cpp`, `sequence/sequences_lol.cpp`,
`sound/sound_lol.cpp`, `resource/staticres_lol.cpp`.

## Prüfen

```
cargo fmt --all
cargo clippy --offline --all-targets
cargo test --offline
```
