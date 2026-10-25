//! A level's decorations as the original gathers them (`loadLevelShpDat`,
//! `assignLevelDecorationShapes`): from one or more pairs of files (`KEEP.SHP` with
//! `KEEP.DAT` …), each decoration a wall uses is taken over into the level's own list, with its
//! figures and the decorations drawn with it; the wall numbers then point into that list.

use std::collections::HashMap;

use lol_data::Result;
use lol_data::assets::AssetStore;
use lol_data::decor::{self, Decoration};
use lol_data::shp::{self, Shape};

#[derive(Debug, Clone, Default)]
pub struct Decorations {
    /// The level's decorations (0 none), their figures pointing into `shapes`.
    pub list: Vec<Decoration>,
    /// The level's figures (0 none).
    pub shapes: Vec<Option<Shape>>,
    file_shapes: Vec<Option<Shape>>,
    file_list: Vec<Decoration>,
    /// Taken over so far from the current files: figure → place in `shapes`, decoration →
    /// place in `list`.
    shape_places: HashMap<u16, u16>,
    list_places: HashMap<u16, u16>,
}

impl Decorations {
    /// Reads a pair of files; `keep` adds to what the level has, else it begins anew.
    pub fn load_files(
        &mut self,
        store: &AssetStore,
        shp: &str,
        dat: &str,
        keep: bool,
    ) -> Result<()> {
        self.file_shapes = shp::parse_file(shp, &store.read(shp)?)?;
        self.file_list = decor::parse(dat, &store.read(dat)?)?;
        self.shape_places.clear();
        self.list_places.clear();
        if !keep || self.list.is_empty() {
            self.list = vec![Decoration::default()];
            self.shapes = vec![None];
        }
        Ok(())
    }

    /// Takes decoration `index` of the current files into the level's list; returns its place.
    pub fn assign(&mut self, index: u16) -> u16 {
        if let Some(&place) = self.list_places.get(&index) {
            return place;
        }
        let place = self.list.len() as u16;
        let mut decoration = self
            .file_list
            .get(index as usize)
            .copied()
            .unwrap_or_default();
        for shape in decoration.shape_index.iter_mut() {
            if *shape == 0xffff {
                continue;
            }
            *shape = match self.shape_places.get(shape) {
                Some(&taken) => taken,
                None => {
                    let taken = self.shapes.len() as u16;
                    self.shapes
                        .push(self.file_shapes.get(*shape as usize).cloned().flatten());
                    self.shape_places.insert(*shape, taken);
                    taken
                }
            };
        }
        self.list.push(decoration);
        self.list_places.insert(index, place);
        if decoration.next != 0 {
            let next = self.assign(decoration.next as u16);
            self.list[place as usize].next = next as u8;
        }
        place
    }

    pub fn get(&self, place: usize) -> Option<&Decoration> {
        self.list.get(place)
    }

    pub fn shape(&self, index: u16) -> Option<&Shape> {
        self.shapes.get(index as usize).and_then(Option::as_ref)
    }
}
