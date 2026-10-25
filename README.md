# Lands-of-Lore-RS

A fan-made reimplementation of the engine of **Lands of Lore: The Throne of Chaos**
(Westwood Studios, 1993), written in Rust for Windows 11.

Lands-of-Lore-RS lets you play the original game on a modern PC without DOSBox: it reads
the data files of your own copy of the game and runs them with a new engine that behaves like
the original program. The whole game is playable: the intro, the title menu, the choice of
hero, all 29 levels with their scripts, scenes and dialogues, combat, magic, merchants and the
finale with credits, including the CD version's speech.

> [!IMPORTANT]
> **This is an unofficial fan project.** It is not made, endorsed, supported or approved by
> Westwood Studios, Virgin Interactive, Electronic Arts, GOG.com or any other rights holder or
> vendor of Lands of Lore. "Lands of Lore", "Westwood Studios" and all related names are
> trademarks of their respective owners.

## You need your own copy of the game

Lands-of-Lore-RS contains **no game assets at all**: no graphics, sounds, music, speech,
texts, maps or scripts of the original game. It only contains the engine.

To play, you need a **legally purchased copy of Lands of Lore: The Throne of Chaos**,
available on [GOG.com](https://www.gog.com/). The engine reads the files of that installation
at run time, directly from the CD image `GAME.DAT` that GOG ships. Nothing is copied,
converted or changed in your installation, and nothing is downloaded. A folder with the
files of the original CD (`DATA\GENERAL.PAK` …) works as well.

Without an installed copy of the game, the program does nothing but tell you that it could not
find one.

## Status

| Part | State |
| --- | --- |
| Intro, title menu, "Lore of the Lands", choice of hero | playable |
| All 29 levels with scripts, scene animations and dialogues | playable |
| Party, inventory, items, merchants (buying and selling) | playable |
| Monsters, combat, magic and spell scrolls | playable |
| Map (atlas), compass, lamp, resting | playable |
| Game menus: load, save, delete, game controls, volumes | playable |
| Music (AdLib, emulated OPL2), sound effects, CD speech | working |
| Finale, outro and credits | playable |
| Languages: German, English, French (as on the CD) | working |
| HD graphics ("modern look") | prepared, not yet available |

The screen is the original's 320×200 picture, stretched to 4:3 as on a DOS monitor and scaled
to the window. The game logic follows the original program as closely as possible; the keys and
the mouse work exactly as in the original.

## How the game data is found

1. The option `--daten FOLDER` or the environment variable `LOL_DATA` (a folder or the image
   `GAME.DAT` itself). If given, only this place is used.
2. The place where GOG's installer registered the game in the Windows registry
   (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\1207662513`, value `PATH`). This also finds the
   game when it was installed to a folder of your choice.
3. The program's own folder and the working folder.
4. Common places on all drives (`GOG Games`, `GOG Galaxy\Games`, `Games`, `Spiele`, the drive
   root …): folders whose name contains "Lands of Lore" or starts with "LOL".

## Building and playing

You need Windows 11 and a recent stable [Rust](https://www.rust-lang.org/) toolchain
(1.88 or newer).

```powershell
git clone https://github.com/SilentBill77/Lands-of-Lore-RS.git
cd Lands-of-Lore-RS
cargo run --profile spiel -p lol-game
```

The `spiel` profile ("play") optimises everything including the Bevy engine; the first build
takes several minutes. Afterwards the program is `target\spiel\lol-game.exe` and can be started
directly. For quicker builds while developing, use `cargo run --release -p lol-game`.

### Options

| Option | Meaning |
| --- | --- |
| `--daten FOLDER` | Where the game is installed, if it is not found automatically |
| `--sprache de\|en\|fr` | Language of texts and speech (default: German) |
| `--laden N` | Load saved game number N right away |
| `--groesse WIDTHxHEIGHT` | Window size (default: 1280x960) |
| `--vollbild` | Fullscreen |

Example: `lol-game.exe --sprache en --vollbild`

### Controls

As in the original: the mouse (left and right button) for nearly everything, the arrow keys or
the number pad to walk, Home and Page Up to turn, F1–F4 for the characters' attacks, M for the
map, R to rest, O for the options, / for magic, Esc for the menu.

### Saved games and settings

Saved games and settings are kept in `%APPDATA%\Lands-of-Lore-RS` (`spielstaende\lol.NNN`
and `einstellungen.txt`), never in the game's installation. The saved games use their own
format and are not compatible with those of the original DOS game.

## Project layout

| Crate | Purpose |
| --- | --- |
| `crates/lol-data` | Readers for the CD image (ISO 9660) and the game's file formats (PAK, LCW, XOR delta, CPS, palettes, SHP, WSA, VCN/VMP, VOC, maps, walls, EMC scripts), without graphics dependencies, with tests. |
| `crates/lol-engine` | The game itself, without a window (`src/lol/`): intro, title and choice of hero, levels with their scripts (EMC) and scene animations (TIM), party, items, monsters, combat, magic, map, menus, saved games, outro. It draws a 320×200 screen of colour indices like the original and mixes music, sound effects and speech. |
| `crates/lol-game` | The window (Bevy 0.20): shows the game's screen, passes on keyboard and mouse, plays the sound. |
| `crates/lol-sound` | Music and sound effects: Westwood's AdLib driver on an emulated OPL2 chip. |
| `crates/lol-export` | Exports pictures, sounds, speech and music of your installation as PNG and WAV files, for inspection and as a basis for HD graphics. |
| `tools/` | `tables_from_scummvm.py`: generates the original program's data tables (`crates/lol-data/src/tables.rs`) from ScummVM's sources. |

The file formats are described in [docs/FORMATE.md](docs/FORMATE.md) (German).

## Development and testing

`cargo fmt --all`, `cargo clippy --all-targets` and `cargo test` check the code. Tests that need
the game data pass quietly when no installation is found.

The engine can also run without a window in virtual time (waiting lets the time pass at once),
driven by a script of timed input, and saves pictures of the screen:

```powershell
cargo run -p lol-engine --example headless -- script.txt output
```

Script lines: `<ms> key <key>`, `<ms> char <c>`, `<ms> click <x> <y>`, `<ms> rclick <x> <y>`,
`<ms> move <x> <y>`, `<ms> snap <name>`, `<ms> every <ms>`, `<ms> quit`.

Environment variables for test runs:

| Variable | Effect |
| --- | --- |
| `LOL_QUICK=0..3` | Start a new game with this hero, without the intro |
| `LOL_START=level,block,direction` | Start somewhere else (a negative block `-n` is the n-th open block of the level) |
| `LOL_GEAR=1` | Start with lamp, atlas and compass |
| `LOL_LOAD=n` | Load saved game n |
| `LOL_OUTRO=hero` | Play the outro |
| `LOL_DEBUG=1` | Print diagnostics |
| `LOL_TRACE=1` | Print every call of a game function by the scripts |

More tools:

| Example | Purpose |
| --- | --- |
| `--example levels OUT [FIRST] [LAST]` | Load every level and save a picture of it |
| `--example emcdump FILE [FUNCTION]` | Disassemble a script of the game |
| `--example paklist ARCHIVE` | List the files in a PAK archive |
| `--example fuzz SEED MS [OUT]` | Play with random input to find crashes and hangs |

## Exporting the game's assets

```powershell
cargo run --release -p lol-export -- alles
```

This writes everything from your installation to `extract/` (about 1.3 GB). Single parts:
`liste` (list files), `info [de|en|fr]`, `skript FILE` (what a script needs from the game),
`bilder` (pictures), `klaenge` (sounds), `musik` (music), `ansichten` (views of every level);
an output folder can be given as the second argument.

- `bilder/`: pictures, sprites (as sheets and single frames), animations, wall blocks and
  palettes; `bilder/en` and `bilder/fr` only hold what differs from the German version.
- `klaenge/`: the sound effects and the voices of the intro.
- `sprache/00` … `sprache/29`: the speech.
- `musik/`: the AdLib music.
- `ansichten/`: the whole screen of every level from a few places.
- `uebersicht.html`: everything on one page, with play buttons.

The exported files are the original game's assets. They are for your own use and are never
part of this repository; do not distribute them. Replacement graphics for the planned modern
look will live in `textures/`, named after the original files (`PLAYFLD.CPS.png`).

## Credits

- **Westwood Studios** created Lands of Lore: The Throne of Chaos (1993). All game content
  belongs to its rights holders.
- The engine is a port of the Lands of Lore engine of **[ScummVM](https://www.scummvm.org/)**
  (the Kyra engine, GPL), whose developers reverse-engineered the original program. Without
  their work this project would not exist.
- **Claude** (Anthropic's AI model, working in [Claude Code](https://claude.com/claude-code))
  took a substantial part in the project: under the direction of the project's author it wrote
  most of the code (the file readers, the port of the engine, the window and the sound), the
  tools for testing and exporting, and the documentation, and it tested the game.

## License

The source code of Lands-of-Lore-RS is licensed under the
**GNU General Public License v3.0 or later** (see [LICENSE](LICENSE)), as it is derived from
ScummVM's GPL-licensed code. This license covers only the code in this repository, not the
game Lands of Lore or any of its assets.
