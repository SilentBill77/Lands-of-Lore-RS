# Dateiformate von Lands of Lore (CD-Fassung)

✔ = gelesen in `lol-data`. Quellen: eigene Untersuchung der GOG-Fassung und ScummVM
(`ref/scummvm/engines/kyra`).

## Die GOG-Fassung

`D:\GOG Games\Lands Of Lore\GAME.DAT` ist das Abbild der CD (ISO 9660, Datenträger
`LOL_V102`), das DOSBox als Laufwerk D: einbindet. Auf der CD:

- `DATA\`: alles, was alle Sprachen teilen: Ebenen `L01.PAK` … `L29.PAK` (Karte `LEVELn.CMZ`,
  Skripte `LEVELn.INF`/`.INI`, Texte `LEVELnn.ENG/.GER/.FRE`, Wandsatz `LEVELn.WLL`,
  Monster `*.SHP` …), Szenen `O00A.PAK` … `O29A.PAK` (`TIM`-Skripte, Animationen), die
  Wandsätze der Gegenden (`KEEP.PAK`, `CIMMERIA.PAK`, `FOREST1.PAK` …: `VCN`, `VMP`, Türen),
  `MONSTER.PAK`, `MUSIC.PAK` (Musik als `ADL`, `XMI`, `C55`, `PCS`), `VOC.PAK` (Geräusche),
  die Sprachausgabe `00.TLK` … `29.TLK`, `FILEDATA.FDT`.
- `DATA\GER`, `DATA\ENG`, `DATA\FRE`: je Sprache das Programm (`LANDS.EXE`, `FINALE.EXE`),
  `GENERAL.PAK` (Spielfläche, Gesichter, Zauber), `STARTUP.PAK`, Vorspann `INTRO1–9.PAK`
  und `INTROVOC.PAK`, Vorgeschichte `HISTORY.PAK`, Abspann `FINALE*.PAK`, Schriften.
  Eine Datei der Sprache geht einer gleichnamigen in `DATA` vor.

## ISO 9660 ✔ (`iso.rs`)

Sektoren zu 2048 Byte. Sektor 16: `01 CD001`, ab Byte 156 der Eintrag des Wurzelverzeichnisses.
Ein Verzeichniseintrag: Länge (u8), Länge der Zusatzattribute (u8), erster Sektor (u32 LE, dann
BE), Größe (u32 LE, dann BE), Datum (7 Byte), Flags (Bit 1: Verzeichnis, Byte 25), Länge des
Namens (Byte 32), Name (`GENERAL.PAK;1`; `00`/`01` für das Verzeichnis selbst und das
übergeordnete). Kein Eintrag überschreitet ein Sektorende; ein Längenbyte 0 heißt: weiter im
nächsten Sektor.

## PAK-Archiv ✔ (`pak.rs`)

Inhaltsverzeichnis: je Datei Beginn ihrer Daten (u32) und Name (mit NUL). Der erste Beginn ist
das Ende des Verzeichnisses. Lands of Lore schließt mit der Archivgröße als letztem Beginn,
einem leeren Namen und Füllbytes ab; Eye of the Beholder mit vier bedeutungslosen Bytes direkt
vor den Daten. Die `TLK`-Dateien sind ebensolche Archive voller `VOC`-Dateien.

## LCW („Format 80“) ✔ und XOR-Delta („Format 40“) ✔ (`compression.rs`)

LCW wie in EOBRS. Das XOR-Delta der Animationen: `00 n v` n-mal v; `0nnnnnnn` die nächsten
n Byte; `1nnnnnnn` n überspringen; `80 nnnn` (u16): 0 Ende, sonst `10…` n überspringen,
`1000…` die nächsten n Byte, `1100… v` n-mal v. Alles wird auf das vorige Bild ge-XOR-t.

## CPS: Bilder ✔ (`cps.rs`)

Wie in EOBRS: Größe − 2 (u16), Kompression (u16: 0, 3 RLE, 4 LCW), entpackte Größe (u32),
Palettengröße (u16), Palette, Daten. Fast alle Bilder von Lands of Lore bringen ihre Palette
mit. Auch `SHP`, `VCN` und `VMP` stecken als Ganzes hinter diesem Kopf.

## Paletten: COL, PAL ✔ (`palette.rs`)

256 × 3 Byte, je 0–63. Im Spiel kommen die Farben 0–127 vom Wandsatz der Gegend (`VCN`), die
Farben 128–255 von `PLAYFLD.CPS`. `MONSTERS.PAL` (3360 Byte) ist keine einfache Palette.

## SHP: Figuren ✔ (`shp.rs`, nach `Screen_v2::getPtrToShape`, `Screen::drawShape`)

Meist hinter einem CPS-Kopf gepackt. Entpackt: Anzahl (u16), je Figur ein Versatz (u32, ab
Byte 2 gezählt). Eine Figur: Flags (u16; Bit 0 Farbtabelle, Bit 1 ungepackt, Bit 2 Länge der
Tabelle folgt), Höhe (u8), Breite (u16), Höhe (u8), Größe (u16), entpackte Größe der Zeilen
(u16), [Tabellenlänge (u8), sonst 16], [Farbtabelle], Zeilen (LCW, außer Bit 1). In den
Zeilen ist `00 n` ein durchsichtiger Lauf von n Pixeln. Mit Tabelle sind die Pixel Plätze in
ihr. Monster verwenden ihre eigene Tabelle (ScummVM `drawItemOrMonster`, Flags `0x8124`).

## WSA: Animationen ✔ (`wsa.rs`, nach `WSAMovie_v2`)

Bilder (u16), x, y (i16), Breite, Höhe, Größe des Delta-Puffers, Flags (u16; Bit 0
Palette). Dann Bilder + 2 Versätze (u32; der vorletzte führt vom letzten Bild zurück zum ersten,
der letzte ist 0, wenn es das nicht gibt). Ist der erste 0, hat die Animation kein eigenes erstes
Bild: sie liegt über einem Bild auf dem Schirm (`HIST02.WSA` über `HIST02.CPS`). Dann die Palette
(768 Byte) und je Bild LCW-gepackt ein XOR-Delta.

## VCN: Blöcke eines Wandsatzes ✔ (`vcn.rs`, nach `loadLevelGraphics`, `drawVcnBlocks`)

Hinter einem CPS-Kopf. Anzahl der Blöcke (u16), je Block eine Verschiebung (u8), eine
Farbtabelle (128 Byte), die Farben 0–127 der Palette (384 Byte), die Blöcke (je 32 Byte, 8×8
Pixel zu 4 Bit, das linke im oberen Halbbyte). Farbe eines Pixels:
`tabelle[(pixel + wll) | verschiebung]`.

## VOC: Klänge und Sprache ✔ (`voc.rs`)

Creative Voice File. Die Geräusche haben 11 111 Hz, die Sprache 22 222 Hz, 8 Bit ohne
Vorzeichen. Die Namen der Sprachdateien (`019A0.00`) setzt ScummVM in
`snd_playCharacterSpeech` zusammen: Satznummer, Sprecher, laufende Nummer, Endung = TLK-Datei.

## ADL: Musik und Klänge der AdLib-Karte ✔ (`lol-sound`)

Westwoods AdLib-Treiber, Version 4: vor den Klangdaten 250 Liednummern (u16, `FFFF` keine),
dann 500 Programme und die Instrumente. Musik: `LORE01A.ADL` … `LORE08B.ADL` in `MUSIC.PAK`,
`LOREINTR.ADL` (Vorspann), `LOREFINL.ADL` (Abspann); Geräusche `LORESFX.ADL`.

## Tabellen aus dem Programm ✔ (`tables.rs`)

Was `LANDS.EXE` an Tabellen mitbringt (Sichtfenster, Wandsätze der Ebenen, Monster, Zauber,
Figuren zur Wahl, Knöpfe, die Namen der Spielfunktionen der Skripte), liest
`python -I tools/tables_from_scummvm.py ref/scummvm` aus ScummVMs
`devtools/create_kyradat/resources/lol_dos_cd*.h` und `engines/kyra/script/script_lol.cpp`.

## EMC2: Skripte ✔ (`emc.rs`, nach `EMCInterpreter`)

IFF `FORM` vom Typ `EMC2` mit `TEXT` (Versätze u16 BE, dann Texte), `ORDR` (je Funktion ihr
Beginn in `DATA`, u16 BE, `FFFF` keine) und `DATA` (Code, u16 BE). Ein Wort: Bit 15 Sprung;
sonst Befehl in Bit 8–12, Parameter im unteren Byte (Bit 14, mit Vorzeichen), im nächsten Wort
(Bit 13) oder keiner. 19 Befehle (Sprung, Stapel, Register, Rechnen, Aufruf des Spiels). Die
Spielfunktionen lesen ihre Argumente oben vom Stapel; ein Text-Argument ist ein Index in `TEXT`.
Die Funktionen von `LEVELn.INF` tragen die Nummer des Blocks, für den sie gelten; das Wort davor
sagt, bei welchen Ereignissen sie laufen (`runLevelScriptCustom`). `LEVELn.INI` richtet die Ebene
ein (Funktion 0, beim ersten Besuch auch 1): `loadLevelGraphics("KEEP", …)`,
`loadBlockProperties("LEVEL1.CMZ")`, `loadDoorShapes`, `loadMonsterShapes`, `loadLevelShapes`.

## TIM: Szenenskripte (teilweise)

IFF `FORM` mit `TEXT` und `AVTL` (u16 LE): die ersten zehn Wörter sind die Anfänge von zehn
Funktionen; ein Befehl ist Länge, Wartezeit, Befehlsnummer, Parameter. Befehl 25 ruft eine
Spielfunktion (`TIM_INGAME_FUNCTIONS`). Die Audienz beim König (`KING1.TIM`) setzt die Gruppe auf
Block 272 mit Blick nach Osten: dort beginnt ein neues Spiel.

## CMZ: Karte ✔ (`level.rs`)

Hinter einem CPS-Kopf. Sechs Byte Kopf (Byte 4–5: Größe eines Eintrags), dann 32×32 Einträge;
deren erste vier Byte sind die Wandnummern nach Norden, Osten, Süden, Westen.

## WLL: Bedeutung der Wandnummern ✔ (`level.rs`)

Index in die Listen der Verzierungsdateien des Programms (u16), dann je 12 Byte: Wandnummer,
Art im Wandsatz (`VMP`, 0 keine), Verzierung (i16), besondere Art, Flags (Bit 0 versperrt den
Weg, Bit 3 durchsichtig), Zeichen auf der Karte, je zwei Byte. `loadDoorShapes` setzt die
Flags der Türen 3–22.

## VMP: Zusammensetzung der Ansicht ✔ (`level.rs`, `view.rs`)

Hinter einem CPS-Kopf: Anzahl (u16), dann Einträge (u16): 330 für Decke und Boden (22×15), dann
431 je Wandart, in neun Stücken (Versätze 102, 97, 129, 117, 81, 159, 45, 239, 0). Ein Eintrag ist
eine Blocknummer im `VCN`; Bit 14 spiegelt, Bit 15 legt den Block über Decke und Boden (Farbe 0
durchsichtig). Das Sichtfenster (176×120 bei x 112) setzt sich wie in Eye of the Beholder aus
den 18 sichtbaren Blöcken zusammen, Decke und Boden auf jedem zweiten Block gespiegelt.

## DAT: Verzierungen eines Wandsatzes ✔ (`decor.rs`, nach `loadLevelShpDat`)

Anzahl (u16), dann je 72 Byte: für zehn Ansichtsplätze die Figur in der zugehörigen `SHP`
(u16, `FFFF` keine), ob der Platz über die Tabellen des Programms skaliert wird (u8, Bit 0), x
und y (i16); dann die mitgezeichnete Verzierung (u8) und Flags (u8: 1 gespiegelt daneben, 2/4
gespiegelt, 8 nur geradeaus). Die Wandnummern zeigen über `WLL` auf diese Verzierungen; das Spiel
übernimmt die gebrauchten in eine eigene Liste der Ebene (`assignLevelDecorationShapes`), auch
aus weiteren Dateipaaren, die das Init-Skript lädt.

## Figuren im Sichtfenster ✔ (Teil, `lol-engine/src/scene.rs`, nach `drawSceneShapes`)

Block für Block, der fernste zuerst: der sichtbare Ausschnitt (`setLevelShapesDim`), die
Verzierungen (`drawDecorations`), dann die Tür (Wände 3–22: fünf Stufen des Öffnens, die Tür um
20 Punkte je Stufe angehoben). Skaliert in 256steln und abgedunkelt über acht Tabellen je Ebene
(`generateOverlay`: Farben 1–127 zur „Sonderfarbe“ des Init-Skripts hin). Noch fehlen
Gegenstände, Monster, Flecken und Eis auf dem Boden.

## Noch offen

Die meisten Spielfunktionen der Skripte, `TIM` als Ganzes, Gegenstände und Monster im
Sichtfenster, Texte
`LEVELnn.GER` und `LANDS.EXE`-Texte, `FNT`, `XMI`/`C55` (General MIDI / MT-32), `FILEDATA.FDT`.
