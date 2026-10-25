//! Readers for the data files of Lands of Lore: The Throne of Chaos (Westwood, 1993), read
//! from the player's own copy (the CD version, as GOG sells it). See `docs/FORMATE.md`.

pub mod assets;
pub mod compression;
pub mod cps;
pub mod error;
pub mod iso;
pub mod pak;
pub mod palette;
pub mod paths;
pub mod shp;
pub mod vcn;
pub mod voc;
pub mod wsa;

pub use error::{Error, Result};
