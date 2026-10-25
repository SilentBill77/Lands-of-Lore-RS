//! The view as the original puts it together from the wall set's blocks: 22×15 blocks of 8×8
//! pixels (176×120), first ceiling and floor, then the walls of the 18 blocks in sight, the
//! farthest first, each from the piece of its kind of wall for that place. Follows ScummVM's
//! `generateBlockDrawingBuffer` and `drawVcnBlocks` (Kyra engine, shared with Eye of the
//! Beholder).
//!
//! The blocks in sight, seen from the party (P), looking up:
//!
//! ```text
//!  00 01 02 03 04 05 06    three steps ahead
//!     07 08 09 10 11       two
//!        12 13 14          one
//!        15 16 17          the party's row (16 is the party's own block)
//! ```

use crate::level::{BLOCKS, Maze, Vmp, WallData};
use crate::tables::{DSC_BLOCK_INDEX, DSC_BLOCK_MAP};
use crate::vcn::WallSet;

pub const COLUMNS: usize = 22;
pub const ROWS: usize = 15;
pub const WIDTH: usize = COLUMNS * 8;
pub const HEIGHT: usize = ROWS * 8;
/// Where the view lies on the screen.
pub const SCREEN_X: usize = 112;
pub const SCREEN_Y: usize = 0;
/// Entries of a kind of wall in the `VMP`, and the ceiling and floor before them.
pub const WALL_ENTRIES: usize = 431;
pub const BACKGROUND: usize = COLUMNS * ROWS;
/// Where each piece of a kind of wall begins among its entries (`_vmpOffsetsDefault`).
pub const PIECE_OFFSETS: [usize; 9] = [102, 97, 129, 117, 81, 159, 45, 239, 0];
pub const SEEN: usize = 18;

/// The blocks in sight from `block`, looking in `direction` (`assignVisibleBlocks`).
pub fn visible_blocks(block: usize, direction: usize) -> [usize; SEEN] {
    std::array::from_fn(|i| {
        let offset = DSC_BLOCK_INDEX[(direction & 3) * SEEN + i] as i8 as isize;
        (block as isize + offset).rem_euclid(BLOCKS as isize) as usize
    })
}

/// Whether ceiling and floor are drawn mirrored (on every other block and turn, so that walking
/// seems to move them).
pub fn background_mirrored(block: usize, direction: usize) -> bool {
    ((block >> 5) + (block & 31) + direction) & 1 != 0
}

/// The view's blocks (`_blockDrawingBuffer`): 330 entries for the walls (0 where the ceiling
/// and floor show), then the 330 of ceiling and floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scene {
    pub blocks: [u16; 2 * BACKGROUND],
    pub visible: [usize; SEEN],
}

impl Scene {
    pub fn compose(
        maze: &Maze,
        walls: &WallData,
        vmp: &Vmp,
        block: usize,
        direction: usize,
    ) -> Self {
        let direction = direction & 3;
        let down = DSC_BLOCK_MAP[direction] as usize;
        let right = DSC_BLOCK_MAP[direction + 4] as usize;
        let left = DSC_BLOCK_MAP[direction + 8] as usize;
        let visible = visible_blocks(block, direction);
        let mut scene = Self {
            blocks: [0; 2 * BACKGROUND],
            visible,
        };
        let v = |i: usize, side: usize| maze.walls[visible[i]][side];
        let o = |i: usize| PIECE_OFFSETS[i] as isize;
        let mut put =
            |x: i32, y: usize, wall: u8, offset: isize, w: usize, h: usize, flip: bool| {
                scene.place(walls, vmp, x, y, wall, offset, w, h, flip);
            };

        // Ceiling and floor.
        put(
            0,
            15,
            1,
            -(BACKGROUND as isize),
            22,
            15,
            background_mirrored(block, direction),
        );

        put(-2, 3, v(0, right), o(0), 3, 5, false);
        put(21, 3, v(6, left), o(0), 3, 5, true);
        let (t, t2) = (v(1, right), v(2, down));
        if walls.has_wall(t) && !walls.see_through(t2) {
            put(2, 3, t, o(0), 3, 5, false);
        } else if t != 0 && walls.see_through(t2) {
            put(2, 3, t2, o(0), 3, 5, false);
        }
        let (t, t2) = (v(5, left), v(4, down));
        if walls.has_wall(t) && !walls.see_through(t2) {
            put(17, 3, t, o(0), 3, 5, true);
        } else if t != 0 && walls.see_through(t2) {
            put(17, 3, t2, o(0), 3, 5, true);
        }
        put(8, 3, v(2, right), o(1), 1, 5, false);
        put(13, 3, v(4, left), o(1), 1, 5, true);
        for (i, x) in [(1, -4), (5, 20), (2, 2), (4, 14)] {
            if walls.has_wall(v(i, down)) {
                put(x, 3, v(i, down), o(2), 6, 5, false);
            }
        }
        put(8, 3, v(3, down), o(2), 6, 5, false);

        put(0, 3, v(7, right), o(3), 2, 6, false);
        put(20, 3, v(11, left), o(3), 2, 6, true);
        put(6, 2, v(8, right), o(4), 2, 8, false);
        put(14, 2, v(10, left), o(4), 2, 8, true);
        for (i, x) in [(8, -4), (10, 16)] {
            if walls.has_wall(v(i, down)) {
                put(x, 2, v(i, down), o(5), 10, 8, false);
            }
        }
        put(6, 2, v(9, down), o(5), 10, 8, false);

        put(3, 1, v(12, right), o(6), 3, 12, false);
        put(16, 1, v(14, left), o(6), 3, 12, true);
        for (i, x) in [(12, -13), (14, 19)] {
            if !walls.see_through(v(i, down)) {
                put(x, 1, v(i, down), o(7), 16, 12, false);
            }
        }
        put(3, 1, v(13, down), o(7), 16, 12, false);

        put(0, 0, v(15, right), o(8), 3, 15, false);
        put(19, 0, v(17, left), o(8), 3, 15, true);
        scene
    }

    /// `generateVmpTileData` and its mirrored twin: a piece of a wall's kind laid into the
    /// blocks, its top left at block `(x, y)`; parts outside the view are cut off, empty
    /// entries leave what is there.
    fn place(
        &mut self,
        walls: &WallData,
        vmp: &Vmp,
        x: i32,
        y: usize,
        wall: u8,
        offset: isize,
        columns: usize,
        rows: usize,
        flipped: bool,
    ) {
        let kind = walls.vmp_map[wall as usize] as isize;
        if kind == 0 {
            return;
        }
        let start = (kind - 1) * WALL_ENTRIES as isize + offset + BACKGROUND as isize;
        for row in 0..rows {
            for column in 0..columns {
                let bx = x + column as i32;
                if !(0..COLUMNS as i32).contains(&bx) {
                    continue;
                }
                let source = if flipped {
                    columns - 1 - column
                } else {
                    column
                };
                let index = start + (row * columns + source) as isize;
                let Some(&raw) = usize::try_from(index).ok().and_then(|i| vmp.entries.get(i))
                else {
                    continue;
                };
                if raw == 0 {
                    continue;
                }
                let value = if flipped {
                    if raw & 0x4000 != 0 {
                        raw - 0x4000
                    } else {
                        raw | 0x4000
                    }
                } else {
                    raw
                };
                if let Some(slot) = self.blocks.get_mut((y + row) * COLUMNS + bx as usize) {
                    *slot = value;
                }
            }
        }
    }

    /// The view as colour indices, 176×120 (`drawVcnBlocks`): a wall's block, else the
    /// ceiling's or floor's; a wall laid over them (bit 15) leaves its colour 0 out.
    pub fn render(&self, set: &WallSet) -> Vec<u8> {
        let mut picture = vec![0u8; WIDTH * HEIGHT];
        for i in 0..BACKGROUND {
            let (x, y) = ((i % COLUMNS) * 8, (i / COLUMNS) * 8);
            let mut entry = self.blocks[i];
            let mut over = 0;
            if entry & 0x8000 != 0 {
                over = entry - 0x8000;
                entry = 0;
            }
            if entry == 0 {
                entry = self.blocks[i + BACKGROUND];
            }
            draw_block(set, entry, false, &mut picture, x, y);
            if over != 0 {
                draw_block(set, over, true, &mut picture, x, y);
            }
        }
        picture
    }
}

/// One block (bit 14 mirrors it) into the picture; `transparent` leaves colour 0 out.
fn draw_block(
    set: &WallSet,
    entry: u16,
    transparent: bool,
    picture: &mut [u8],
    x: usize,
    y: usize,
) {
    let flipped = entry & 0x4000 != 0;
    let index = (entry & 0x3fff) as usize;
    let pixels = set.block_pixels(index);
    for row in 0..8 {
        for column in 0..8 {
            let source = if flipped { 7 - column } else { column };
            let color = pixels[row * 8 + source];
            if transparent && color == 0 {
                continue;
            }
            picture[(y + row) * WIDTH + x + column] = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_the_block_ahead() {
        // Facing north from block 33 (1, 1), the block ahead (13) is 1 (1, 0).
        let seen = visible_blocks(33, 0);
        assert_eq!(seen[16], 33);
        assert_eq!(seen[13], 1);
        // Facing east, the block ahead is 34.
        assert_eq!(visible_blocks(33, 1)[13], 34);
    }

    #[test]
    fn pieces_fill_a_kind_of_wall() {
        let sizes = [
            (3, 5),
            (1, 5),
            (6, 5),
            (2, 6),
            (2, 8),
            (10, 8),
            (3, 12),
            (16, 12),
            (3, 15),
        ];
        let total: usize = sizes.iter().map(|(w, h)| w * h).sum();
        assert_eq!(total, WALL_ENTRIES);
        for (i, (w, h)) in sizes.iter().enumerate() {
            let end = PIECE_OFFSETS[i] + w * h;
            assert!(end <= WALL_ENTRIES, "Stück {i}");
        }
    }
}
