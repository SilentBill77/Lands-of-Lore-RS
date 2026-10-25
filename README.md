# Lands-of-Lore-RS

Eigene Engine in Rust für **Lands of Lore: The Throne of Chaos** (Westwood, 1993), für
Windows 11. Ziel: das ganze Spiel originalgetreu spielbar, später mit moderner Grafik
(Umschalten zwischen Original-Look und Modern-Look wie bei EOBRS; F2 bleibt hier dem Spiel).

Lands-of-Lore-RS enthält **keine** Spieldaten. Die Engine liest die Dateien einer vorhandenen
Installation (der CD-Fassung, wie GOG sie verkauft) zur Laufzeit. Lizenz: GPL-3.0 oder
später (siehe `LICENSE`).

## Aufbau

| Crate | Zweck |
| --- | --- |
| `crates/lol-data` | Leser für das CD-Abbild (ISO 9660) und die Dateiformate (PAK, LCW, XOR-Delta, CPS, Paletten, SHP, WSA, VCN, VOC, Karten, Wände, Skripte) und das Sichtfenster wie im Original, ohne Grafikabhängigkeiten, mit Tests. |
| `crates/lol-engine` | Das Spiel ohne Fenster, nach ScummVMs Kyra/LoL-Engine portiert (`src/lol/`): Vorspann, Titel und Heldenwahl, Ebenen mit ihren Skripten (EMC) und Szenen (TIM), Gruppe, Gegenstände, Monster, Kampf, Zauber, Zauberatlas, Menüs, Spielstände, Abspann. Es zeichnet einen Bildschirm von 320×200 Farbindizes wie das Original und mischt Musik, Geräusche und Sprache. |
| `crates/lol-game` | Das Fenster (Bevy 0.20): zeigt den Bildschirm des Spiels, gibt Tasten und Maus weiter, spielt den Ton. |
| `crates/lol-sound` | Musik und Klänge: Westwoods AdLib-Treiber (Version 4) auf einem nachgebauten OPL2-Chip, aus EOBRS übernommen. |
| `crates/lol-export` | Alle Bilder, Klänge, die Sprachausgabe und die Musik als PNG und WAV, zur Sichtung und als Vorlage für HD-Inhalte. |
| `tools/` | `tables_from_scummvm.py`: die Tabellen des Originalprogramms aus ScummVM (GPL) nach `crates/lol-data/src/tables.rs`. |

Die Formate sind in [docs/FORMATE.md](docs/FORMATE.md) beschrieben.

## Spieldaten finden

1. Die Option `--daten ORDNER` oder die Umgebungsvariable `LOL_DATA` (ein Ordner oder das
   Abbild selbst); dann nur dieser Ort.
2. Wo der GOG-Installer das Spiel in die Registry eingetragen hat (`GOG.com\Games\1207662513`,
   Wert `PATH`), also auch bei einem frei gewählten Installationsordner.
3. Neben dem Programm und im Arbeitsordner.
4. Übliche Orte auf allen Laufwerken (`GOG Games`, `GOG Galaxy\Games`, `Games`, `Spiele` …,
   Ordner mit „Lands of Lore“ oder „LOL…“ im Namen).

Erkannt wird das Abbild `GAME.DAT` oder ein Ordner mit den Dateien der CD (`DATA\GENERAL.PAK`).

## Spielen

```powershell
cargo run --offline --profile spiel -p lol-game
cargo run --offline -p lol-game -- --sprache en --laden 0 --groesse 1600x1200 --vollbild
```

Das ganze Spiel läuft wie das Original (CD-Fassung mit Sprachausgabe): Vorspann, Titelmenü mit
„Lore of the Lands“, König Richards Wahl des Helden, die 29 Ebenen mit ihren Skripten, Szenen und
Gesprächen, Kampf, Zauber, Gegenstände, Händler, Zauberatlas, Kompass und Lampe, die Spielmenüs
(Laden, Sichern, Löschen, Spielsteuerung, Lautstärken) und das Finale mit Abspann. Das Spiel
läuft in einem eigenen Faden und wartet wie das Original; das Fenster zeigt seinen Bildschirm,
auf 4:3 gestreckt wie unter DOS.

Bedienung wie im Original: Maus (links, rechts), Pfeiltasten und Ziffernblock zum Gehen,
Pos1/Bild↑ drehen, F1–F4 Angriff der Figuren, M Karte, R Rast, O Optionen, / Zauber, Esc Menü.
Alle Tasten wirken wie im Original; F2 ist die Angriffstaste der zweiten Figur. Die
Umschaltung zur HD-Grafik ist vorbereitet (`lol-game/src/renderer.rs`) und bekommt später eine
Taste, die das Spiel nicht belegt.

Optionen: `--daten ORDNER`, `--sprache de|en|fr`, `--laden NR` (Spielstand), `--groesse BREITExHÖHE`,
`--vollbild`.

Spielstände und Einstellungen liegen in `%APPDATA%\Lands-of-Lore-RS` (`spielstaende\lol.NNN`,
`einstellungen.txt`).

### Testen ohne Fenster

Das Spiel läuft auch in virtueller Zeit (warten lässt die Zeit sofort verstreichen), gesteuert
durch ein Skript zeitgesteuerter Eingaben, und legt Bilder des Bildschirms ab:

```powershell
cargo run --offline -p lol-engine --example headless -- skript.txt ausgabe
```

Skriptzeilen: `<ms> key <taste>`, `<ms> char <z>`, `<ms> click <x> <y>`, `<ms> rclick <x> <y>`,
`<ms> snap <name>`, `<ms> every <ms>`, `<ms> quit`. Umgebungsvariablen: `LOL_QUICK=0..3` (neues
Spiel mit diesem Helden ohne Vorspann), `LOL_START=ebene,block,richtung` (ein negativer Block -n
ist das n-te freie Feld der Ebene), `LOL_GEAR=1` (Lampe,
Atlas, Kompass), `LOL_LOAD=nr`, `LOL_OUTRO=held`, `LOL_DEBUG=1`, `LOL_TRACE=1` (jeder Aufruf der
Spielfunktionen durch die Skripte). Weitere Werkzeuge: `--example levels` (jede Ebene laden und
abbilden), `--example emcdump DATEI [FUNKTION]` (ein Skript lesbar), `--example paklist ARCHIV`,
`--example fuzz SAAT MS [ORDNER]` (zufällige Eingaben ab `LOL_START`, um Abstürze und Hänger
zu finden).

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
