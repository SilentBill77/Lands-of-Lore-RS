# Lands-of-Lore-RS

Eigene Engine in Rust für **Lands of Lore: The Throne of Chaos** (Westwood, 1993), für
Windows 11. Ziel: das ganze Spiel originalgetreu spielbar, mit moderner Grafik (Umschalten
mit F2 zwischen Original-Look und Modern-Look, wie bei EOBRS).

Lands-of-Lore-RS enthält **keine** Spieldaten. Die Engine liest die Dateien einer vorhandenen
Installation (der CD-Fassung, wie GOG sie verkauft) zur Laufzeit. Lizenz: GPL-3.0 oder
später (siehe `LICENSE`).

## Aufbau

| Crate | Zweck |
| --- | --- |
| `crates/lol-data` | Leser für das CD-Abbild (ISO 9660) und die Dateiformate (PAK, LCW, XOR-Delta, CPS, Paletten, SHP, WSA, VCN, VOC, Karten, Wände, Skripte) und das Sichtfenster wie im Original, ohne Grafikabhängigkeiten, mit Tests. |
| `crates/lol-engine` | Das Spiel ohne Fenster: Ebenen laden (mit ihrem Init-Skript), die Gruppe bewegen, den Bildschirm zeichnen wie das Original (320×200 Farbindizes). |
| `crates/lol-game` | Das Spiel (Bevy 0.20). Bisher: die Ebenen im Original-Look begehen. |
| `crates/lol-sound` | Musik und Klänge: Westwoods AdLib-Treiber (Version 4) auf einem nachgebauten OPL2-Chip, aus EOBRS übernommen. |
| `crates/lol-export` | Alle Bilder, Klänge, die Sprachausgabe und die Musik als PNG und WAV, zur Sichtung und als Vorlage für HD-Inhalte. |
| `tools/` | `tables_from_scummvm.py`: die Tabellen des Originalprogramms aus ScummVM (GPL) nach `crates/lol-data/src/tables.rs`. |

Die Formate sind in [docs/FORMATE.md](docs/FORMATE.md) beschrieben.

## Spieldaten finden

1. Umgebungsvariable `LOL_DATA` (ein Ordner oder das Abbild selbst).
2. Neben dem Programm und im Arbeitsordner.
3. Übliche GOG-Orte auf allen Laufwerken (`GOG Games\Lands Of Lore` …).

Erkannt wird das Abbild `GAME.DAT` oder ein Ordner mit den Dateien der CD (`DATA\GENERAL.PAK`).

## Spielen

```powershell
cargo run --offline -p lol-game
cargo run --offline -p lol-game -- --ebene 11 --pos 400 --richtung N
```

Bisher lassen sich die Ebenen begehen (Bildschirm 320×200, auf 4:3 gestreckt wie unter DOS): die
Spielfläche des Originals mit dem Sichtfenster, Wände, Decke und Boden aus dem Wandsatz der
Gegend; Wände versperren den Weg. Ein neues Spiel beginnt wie das Original in Gladstone Keep,
Ebene 1, Block 272, mit Blick nach Osten. Noch fehlen Türen, Verzierungen, Gegenstände, Monster,
Gruppe, Skripte der Ebenen und alles Weitere.

Tasten: W/↑/8 vor, S/↓/5 zurück, A/4 und D/6 seitwärts, Q/←/7 und E/→/9 drehen; Bild↑/Bild↓
wechseln die Ebene zum Ausprobieren.

Optionen: `--sprache de|en|fr`, `--ebene N`, `--pos BLOCK`, `--richtung N|O|S|W`,
`--wege VZADLR…` (Züge gleich nach dem Start: V vor, Z zurück, A/D seitwärts, L/R drehen),
`--screenshot DATEI` (schreibt den Bildschirm und endet, ohne Fenster), `--groesse BREITExHÖHE`.

## Exportieren

```powershell
cargo run --offline --release -p lol-export -- alles
```

Einzeln: `liste`, `info [de|en|fr]`, `skript DATEI` (was ein Skript vom Spiel will), `bilder`,
`klaenge`, `musik`, `ansichten`; ein Zielordner als zweites Argument (sonst `extract`). Danach liegen in `extract/` (etwa 1,3 GB):

- `bilder/allgemein/<ARCHIV>/` und `bilder/de/<ARCHIV>/`: Bilder (`NAME.CPS.png`), Figuren
  (`NAME.SHP.png` als Bogen, einzeln in `NAME.SHP/`), Animationen (`NAME.WSA.png` als Bogen,
  einzeln in `NAME.WSA/`), Wandblöcke (`NAME.VCN.png`) und Paletten. `bilder/en` und
  `bilder/fr` enthalten nur, was sich von der deutschen Fassung unterscheidet.
- `klaenge/`: die Geräusche (`VOC.PAK`) und die Stimmen des Vorspanns.
- `sprache/00` … `sprache/29`: die Sprachausgabe.
- `musik/`: die Musik der AdLib-Karte, `musik/klaenge/` ihre Geräusche.
- `ansichten/`: der ganze Bildschirm jeder Ebene von einigen Stellen aus, zum Vergleich mit dem
  Original.
- `uebersicht.html`: alles auf einer Seite, mit Abspielknöpfen.

Der Ordner enthält Originalgrafik und -ton und wird nie eingecheckt. Ersatzgrafiken für den
Modern-Look sollen später in `textures/` liegen, benannt nach dem Original
(`PLAYFLD.CPS.png`).
