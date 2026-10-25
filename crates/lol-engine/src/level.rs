//! Loading a level as ScummVM's `LoLEngine::loadLevel`: the meaning of its wall numbers
//! (`LEVELn.WLL`), then its init script (`LEVELn.INI`: function 0, and function 1 on the first
//! visit), which names the wall set, the map, the doors, the monsters and the decorations, and
//! the level's script (`LEVELn.INF`) that the game runs on events.
//!
//! So far the init script's calls that put the level's look together are carried out
//! ([`IniHost`]: wall set, map, doors, decorations); the others are noted in
//! [`Level::unhandled`].

use std::collections::BTreeSet;

use lol_data::Result;
use lol_data::assets::AssetStore;
use lol_data::emc::{Args, Host, Script, State};
use lol_data::error::Error;
use lol_data::level::{Maze, Vmp, WallData};
use lol_data::palette::Palette;
use lol_data::shp::{self, Shape};
use lol_data::tables::{LEVEL_DAT_LIST, LEVEL_SHP_LIST, SCRIPT_FUNCTIONS};
use lol_data::vcn::WallSet;

use crate::decorations::Decorations;
use crate::draw::level_overlays;

/// Words a script may run before it counts as stuck.
pub const MAX_STEPS: usize = 200_000;

#[derive(Debug, Clone)]
pub struct Level {
    pub index: usize,
    pub maze: Maze,
    pub walls: WallData,
    /// The wall set's name (`KEEP`), its blocks and how its kinds of wall are put together.
    pub graphics: String,
    pub wall_set: WallSet,
    pub vmp: Vmp,
    /// The game's palette while in this level (colours 0–127 the wall set's).
    pub palette: Palette,
    /// The colour and weight of the darkening with distance (`_lastSpecialColor`, …).
    pub special_color: i16,
    pub special_weight: i16,
    /// The tables that darken the walls' colours with distance (`getLevelOverlay`).
    pub overlays: [[u8; 256]; 8],
    /// The doors' file and their two figures.
    pub doors: Option<(String, i16, i16)>,
    pub door_shapes: [Option<Shape>; 2],
    pub decorations: Decorations,
    /// Per wall number its decoration in `decorations` (`_wllShapeMap`; 0 or less none).
    pub shape_map: [i8; 256],
    pub music: i16,
    pub script: Option<Script>,
    /// The init script's calls not carried out yet (their names).
    pub unhandled: BTreeSet<&'static str>,
}

impl Level {
    /// Loads level `index` (1–29), the game's palette's upper half from `base`.
    pub fn load(store: &AssetStore, index: usize, base: &Palette) -> Result<Self> {
        let name = |ext: &str| format!("LEVEL{index}.{ext}");
        let walls = WallData::parse(&name("WLL"), &store.read(&name("WLL"))?)?;
        // `loadLevelWallData`: the decorations of the region the walls name, and per wall its
        // decoration.
        let mut decorations = Decorations::default();
        let list = walls.decorations_file as usize;
        let (shp_file, dat_file) = LEVEL_SHP_LIST
            .get(list)
            .zip(LEVEL_DAT_LIST.get(list))
            .ok_or_else(|| Error::format(name("WLL"), format!("Verzierungen {list} unbekannt")))?;
        decorations.load_files(store, shp_file, dat_file, false)?;
        let mut shape_map = [0i8; 256];
        for (wall, entry) in shape_map.iter_mut().enumerate() {
            let shape = walls.shape[wall];
            *entry = if shape > 0 {
                decorations.assign(shape as u16) as i8
            } else {
                shape as u8 as i8
            };
        }
        let ini = Script::parse(&name("INI"), &store.read(&name("INI"))?)?;
        let mut host = IniHost {
            store,
            walls,
            base: base.clone(),
            maze: None,
            graphics: None,
            doors: None,
            door_shapes: [None, None],
            decorations,
            shape_map,
            music: 0,
            unhandled: BTreeSet::new(),
            error: None,
        };
        for function in [0, 1] {
            let mut state = State::default();
            if ini.start(&mut state, function) {
                ini.run(&mut state, &mut host, MAX_STEPS);
            }
            if let Some(err) = host.error.take() {
                return Err(err);
            }
        }
        let missing = |what: &str| Error::format(name("INI"), format!("lädt keine {what}"));
        let (graphics, wall_set, vmp, palette, special_color, special_weight) =
            host.graphics.ok_or_else(|| missing("Wandgrafik"))?;
        let script = store
            .read(&name("INF"))
            .ok()
            .map(|bytes| Script::parse(&name("INF"), &bytes))
            .transpose()?;
        let overlays = level_overlays(&palette, special_color as u8, special_weight as i32);
        Ok(Self {
            index,
            maze: host.maze.ok_or_else(|| missing("Karte"))?,
            walls: host.walls,
            graphics,
            wall_set,
            vmp,
            palette,
            special_color,
            special_weight,
            overlays,
            doors: host.doors,
            door_shapes: host.door_shapes,
            decorations: host.decorations,
            shape_map: host.shape_map,
            music: host.music,
            script,
            unhandled: host.unhandled,
        })
    }
}

/// The game as a level's init script sees it.
struct IniHost<'a> {
    store: &'a AssetStore,
    walls: WallData,
    base: Palette,
    maze: Option<Maze>,
    graphics: Option<(String, WallSet, Vmp, Palette, i16, i16)>,
    doors: Option<(String, i16, i16)>,
    door_shapes: [Option<Shape>; 2],
    decorations: Decorations,
    shape_map: [i8; 256],
    music: i16,
    unhandled: BTreeSet<&'static str>,
    error: Option<Error>,
}

impl IniHost<'_> {
    /// `loadLevelGraphics`: the wall set `NAME.VCN` and `NAME.VMP`, the palette's lower half
    /// from the wall set or from a file of its own.
    fn load_graphics(&mut self, args: &Args) -> Result<()> {
        let file = args.string(0).to_ascii_uppercase();
        let vcn_name = format!("{file}.VCN");
        let wall_set = WallSet::parse(&vcn_name, &self.store.read(&vcn_name)?)?;
        let vmp_name = format!("{file}.VMP");
        let vmp = Vmp::parse(&vmp_name, &self.store.read(&vmp_name)?)?;
        let low = if args.pos(5) == -1 {
            wall_set.palette_low.clone()
        } else {
            let own = self.store.read(args.string(5))?;
            own[..own.len().min(384)].to_vec()
        };
        let palette = self.base.with_low(&low);
        self.graphics = Some((file, wall_set, vmp, palette, args.pos(1), args.pos(2)));
        Ok(())
    }

    /// `loadDoorShapes`: the doors' two figures, and which doors stop the party, as the
    /// original sets their flags.
    fn load_doors(&mut self, args: &Args) -> Result<()> {
        let file = args.string(0).to_string();
        let shapes = shp::parse_file(&file, &self.store.read(&file)?)?;
        let pick = |i: i16| shapes.get(i.max(0) as usize).cloned().flatten();
        self.door_shapes = [pick(args.pos(1)), pick(args.pos(2))];
        let flags = &mut self.walls.flags;
        for i in 0..20 {
            flags[i + 3] |= 7;
            match i % 5 {
                4 => flags[i + 3] &= 0xf8,
                3 => flags[i + 3] &= 0xfd,
                _ => {}
            }
        }
        if args.pos(3) != 0 {
            flags[3..13].iter_mut().for_each(|f| *f &= 0xfd);
        }
        if args.pos(4) != 0 {
            flags[13..23].iter_mut().for_each(|f| *f &= 0xfd);
        }
        self.doors = Some((file, args.pos(1), args.pos(2)));
        Ok(())
    }
}

impl Host for IniHost<'_> {
    fn sys_call(&mut self, id: u8, args: &Args) -> i16 {
        let name = SCRIPT_FUNCTIONS.get(id as usize).copied().unwrap_or("");
        let result = match name {
            "loadLevelGraphics" => self.load_graphics(args),
            "loadBlockProperties" => {
                let file = args.string(0).to_string();
                self.store
                    .read(&file)
                    .and_then(|bytes| Maze::parse(&file, &bytes))
                    .map(|maze| self.maze = Some(maze))
            }
            "loadDoorShapes" => self.load_doors(args),
            "loadLevelShapes" => {
                let (shp, dat) = (args.string(0).to_string(), args.string(1).to_string());
                self.decorations.load_files(self.store, &shp, &dat, true)
            }
            "assignLevelDecorationShape" => {
                return self.decorations.assign(args.pos(0) as u16) as i16;
            }
            "resetBlockShapeAssignment" => {
                let v = args.pos(0) as u8 as i8;
                self.shape_map[3..8].fill(v);
                self.shape_map[13..18].fill(v);
                Ok(())
            }
            "closeLevelShapeFile" => Ok(()),
            "setMusicTrack" => {
                self.music = args.pos(0);
                Ok(())
            }
            _ => {
                self.unhandled.insert(name);
                return 0;
            }
        };
        if let Err(err) = result {
            self.error.get_or_insert(err);
        }
        1
    }
}
