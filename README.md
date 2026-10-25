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
| `crates/lol-data` | Leser für das CD-Abbild (ISO 9660) und die Dateiformate (PAK, LCW, XOR-Delta, CPS, Paletten, SHP, WSA, VCN, VOC), ohne Grafikabhängigkeiten, mit Tests. |
| `crates/lol-sound` | Musik und Klänge: Westwoods AdLib-Treiber (Version 4) auf einem nachgebauten OPL2-Chip, aus EOBRS übernommen. |
| `crates/lol-export` | Alle Bilder, Klänge, die Sprachausgabe und die Musik als PNG und WAV, zur Sichtung und als Vorlage für HD-Inhalte. |

Die Formate sind in [docs/FORMATE.md](docs/FORMATE.md) beschrieben.

## Spieldaten finden

1. Umgebungsvariable `LOL_DATA` (ein Ordner oder das Abbild selbst).
2. Neben dem Programm und im Arbeitsordner.
3. Übliche GOG-Orte auf allen Laufwerken (`GOG Games\Lands Of Lore` …).

Erkannt wird das Abbild `GAME.DAT` oder ein Ordner mit den Dateien der CD (`DATA\GENERAL.PAK`).

## Exportieren

```powershell
cargo run --offline --release -p lol-export -- alles
```

Einzeln: `liste`, `info [de|en|fr]`, `bilder`, `klaenge`, `musik`; ein Zielordner als zweites
Argument (sonst `extract`). Danach liegen in `extract/` (etwa 1,3 GB):

- `bilder/allgemein/<ARCHIV>/` und `bilder/de/<ARCHIV>/`: Bilder (`NAME.CPS.png`), Figuren
  (`NAME.SHP.png` als Bogen, einzeln in `NAME.SHP/`), Animationen (`NAME.WSA.png` als Bogen,
  einzeln in `NAME.WSA/`), Wandblöcke (`NAME.VCN.png`) und Paletten. `bilder/en` und
  `bilder/fr` enthalten nur, was sich von der deutschen Fassung unterscheidet.
- `klaenge/`: die Geräusche (`VOC.PAK`) und die Stimmen des Vorspanns.
- `sprache/00` … `sprache/29`: die Sprachausgabe.
- `musik/`: die Musik der AdLib-Karte, `musik/klaenge/` ihre Geräusche.
- `uebersicht.html`: alles auf einer Seite, mit Abspielknöpfen.

Der Ordner enthält Originalgrafik und -ton und wird nie eingecheckt. Ersatzgrafiken für den
Modern-Look sollen später in `textures/` liegen, benannt nach dem Original
(`PLAYFLD.CPS.png`).
