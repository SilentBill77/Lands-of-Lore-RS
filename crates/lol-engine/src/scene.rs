// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
// Ported from ScummVM's Kyra engine, Copyright (C) The ScummVM Team (www.scummvm.org).
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the original draws over the walls of the view (ScummVM's `LoLEngine::drawSceneShapes`):
//! block by block, the farthest first, each block's decorations and its door, cut to the part
//! of the view where that block can be seen (`setLevelShapesDim`). Items, monsters and the
//! marks on the floor are still missing.

use lol_data::tables::{
    BASE_DSC_Y, DSC_BLOCK_MAP, DSC_DIM_DATA1, DSC_DIM_DATA2, DSC_DIM_MAP, DSC_DOOR_FRAME_Y1,
    DSC_DOOR_FRAME_Y2, DSC_DOOR_SCALE, DSC_DOOR_SCALE_OFFS, DSC_DOOR_SHAPE_INDEX, DSC_DOOR_X,
    DSC_DOOR_Y, DSC_DOOR_Y2, DSC_DOOR4, DSC_OVL_INDEX, DSC_OVL_MAP, DSC_SCALE_HEIGHT_DATA,
    DSC_SCALE_WIDTH_DATA, DSC_SHAPE_INDEX, DSC_TILE_INDEX, DSC_WALLS, DSC_X,
};
use lol_data::view::{SCREEN_X, SEEN, background_mirrored, visible_blocks};

use crate::draw::{Clip, Draw, draw_shape, scaled_height, scaled_width};
use crate::level::Level;

/// The view's left edge in columns of 8 pixels.
const VIEW_COLUMN: i32 = (SCREEN_X / 8) as i32;

/// Draws the decorations and doors of the blocks in sight into the screen.
pub fn draw_scene_shapes(screen: &mut [u8], level: &Level, block: usize, direction: usize) {
    let direction = direction & 3;
    let visible = visible_blocks(block, direction);
    let down = DSC_BLOCK_MAP[direction] as usize;
    let mirrored = background_mirrored(block, direction);
    let ctx = Context {
        level,
        visible,
        down,
        direction,
        mirrored,
    };
    for &tile in DSC_TILE_INDEX.iter() {
        let t = tile as usize;
        let s = level.maze.walls[visible[t]][down];
        let (x1, x2) = ctx.shapes_dim(t);
        if x2 <= x1 {
            continue;
        }
        let clip = Clip {
            x1: x1 * 8,
            y1: 0,
            x2: x2 * 8,
            y2: 120,
        };
        ctx.draw_decorations(screen, t, clip);
        if level.walls.flags[s as usize] & 8 == 0 {
            continue;
        }
        // A door in its frame: walls 3–22 are doors in five stages of opening.
        let si = s as usize;
        let offset = if si < 23 {
            DSC_DOOR_SCALE_OFFS[si] as i32
        } else {
            0
        };
        let v = (20 * (s as i32 - offset)) as u16;
        let v = v.min(80) as i32;
        let a = DSC_DIM_MAP[t] as usize;
        let door_clip = Clip {
            y1: DSC_DOOR_FRAME_Y1[a] as i32,
            y2: DSC_DOOR_FRAME_Y2[a] as i32,
            ..clip
        };
        let which = if si < 23 {
            DSC_DOOR_SHAPE_INDEX[si] as usize
        } else {
            0
        };
        ctx.draw_door(screen, which.min(1), t, 10, 0, -v, door_clip);
    }
}

struct Context<'a> {
    level: &'a Level,
    visible: [usize; SEEN],
    down: usize,
    direction: usize,
    /// Ceiling and floor mirrored (`_wllProcessFlag`).
    mirrored: bool,
}

impl Context<'_> {
    /// `setLevelShapesDim`: the columns of the view (in 8 pixels, on the screen) in which block
    /// `index` can be seen past the walls in front of it.
    fn shapes_dim(&self, index: usize) -> (i32, i32) {
        let walls = &self.level.walls;
        let (mut x1, mut x2) = (0i32, 22i32);
        let m = index * SEEN;
        for i in 0..SEEN {
            let d = self.level.maze.walls[self.visible[i]][self.down] as usize;
            let a = walls.flags[d];
            if a & 8 != 0 {
                let t = DSC_DIM_DATA2[(m + i) << 1] as i8 as i32;
                if t > x1 {
                    x1 = t;
                }
                let t = DSC_DIM_DATA2[((m + i) << 1) + 1] as i8 as i32;
                if t < x2 {
                    x2 = t;
                }
            } else {
                let t = DSC_DIM_DATA1[m + i] as i8 as i32;
                if walls.vmp_map[d] == 0 || t == -40 {
                    continue;
                }
                if t == -41 {
                    x1 = 22;
                    x2 = 0;
                    break;
                }
                if t > 0 && x2 > t {
                    x2 = t;
                }
                if t < 0 && x1 < -t {
                    x1 = -t;
                }
            }
            if x2 < x1 {
                break;
            }
        }
        (x1 + VIEW_COLUMN, x2 + VIEW_COLUMN)
    }

    /// `drawDecorations`: the decorations on the walls of block `index` facing the party and
    /// to the sides, each at the size and place the program's tables give for that place.
    fn draw_decorations(&self, screen: &mut [u8], index: usize, clip: Clip) {
        let level = self.level;
        for i in [1usize, 0] {
            let s = index * 2 + i;
            let mut scale_w = DSC_SCALE_WIDTH_DATA[s];
            let mut scale_h = DSC_SCALE_HEIGHT_DATA[s];
            let mut ix = DSC_SHAPE_INDEX[s] as i8 as i32;
            let mut shp_ix = ix.unsigned_abs() as usize;
            let ovl_index =
                (DSC_OVL_INDEX[4 + DSC_DIM_MAP[index] as usize * 5] as usize + 2).min(7);
            if scale_w == 0 || scale_h == 0 {
                continue;
            }
            let d = (self.direction as i32 + DSC_WALLS[s] as i8 as i32) as usize & 3;
            let wall = level.maze.walls[self.visible[index]][d];
            let mut l = level.shape_map[wall as usize] as i32;
            let mut guard = 0;
            while l > 0 && guard < 32 {
                guard += 1;
                let Some(prop) = level.decorations.get(l as usize).copied() else {
                    break;
                };
                if prop.flags & 8 != 0 && index != 3 && index != 9 && index != 13 {
                    l = prop.next as i32;
                    continue;
                }
                if DSC_OVL_MAP.get(shp_ix) == Some(&1)
                    && (prop.flags & 2 != 0 || (prop.flags & 4 != 0 && self.mirrored))
                {
                    ix = -ix;
                }
                let (mut x_offs, mut y_offs) = (0, 0);
                let mut fade = None;
                if prop.scale_flag.get(shp_ix).is_some_and(|f| f & 1 != 0) {
                    x_offs = prop.shape_x[shp_ix] as i32;
                    y_offs = prop.shape_y[shp_ix] as i32;
                    shp_ix = DSC_OVL_MAP.get(shp_ix).copied().unwrap_or(0) as usize;
                    fade = Some(&level.overlays[ovl_index]);
                } else if prop.shape_index.get(shp_ix).is_some_and(|&v| v != 0xffff) {
                    scale_w = 0x100;
                    scale_h = 0x100;
                    fade = Some(&level.overlays[7]);
                }
                let shape_index = prop.shape_index.get(shp_ix).copied().unwrap_or(0xffff);
                if shape_index != 0xffff
                    && let Some(shape) = level.decorations.shape(shape_index)
                {
                    let sx = prop.shape_x[shp_ix] as i32;
                    let sy = prop.shape_y[shp_ix] as i32;
                    let base_x = DSC_X[s] as i16 as i32;
                    let mut x = base_x + x_offs + ((sx * scale_w as i32) >> 8);
                    let mut flip = false;
                    if ix < 0 {
                        if ix == DSC_SHAPE_INDEX[s] as i8 as i32 {
                            x = base_x
                                - ((sx * scale_w as i32) >> 8)
                                - scaled_width(shape, scale_w)
                                - x_offs;
                        }
                        flip = true;
                    }
                    let y = BASE_DSC_Y[s] as i8 as i32 + y_offs + ((sy * scale_h as i32) >> 8);
                    let how = Draw {
                        flip,
                        scale: Some((scale_w, scale_h)),
                        table: None,
                        fade,
                    };
                    draw_shape(screen, shape, x + SCREEN_X as i32, y, clip, how);
                    if prop.flags & 1 != 0 && shp_ix < 4 {
                        // Drawn again, mirrored, beside itself.
                        x += scaled_width(shape, scale_w);
                        let how = Draw { flip: !flip, ..how };
                        draw_shape(screen, shape, x + SCREEN_X as i32, y, clip, how);
                    }
                }
                l = prop.next as i8 as i32;
                shp_ix = (DSC_SHAPE_INDEX[s] as i8 as i32).unsigned_abs() as usize;
            }
        }
    }

    /// `drawDoor`: the door figure of block `index`, scaled for its distance and raised by `h`
    /// (opening), centred where the program's tables place it.
    fn draw_door(
        &self,
        screen: &mut [u8],
        which: usize,
        index: usize,
        column: usize,
        w: i32,
        h: i32,
        clip: Clip,
    ) {
        let level = self.level;
        let Some(shape) = level.door_shapes[which].as_ref() else {
            return;
        };
        let c = DSC_DOOR_Y2[(self.direction << 5) + column] as usize;
        let dim = DSC_DIM_MAP[index] as usize;
        let r = c / 5 + 5 * dim;
        let mut d = DSC_OVL_INDEX.get(r).copied().unwrap_or(0) as usize;
        let t = (index << 5) + c;
        let mut y = DSC_DOOR_Y[t] as i16 as i32 + 120;
        let scale_w = DSC_DOOR_SCALE[dim << 1];
        let scale_h = DSC_DOOR_SCALE[(dim << 1) + 1];
        let u = DSC_DOOR4[dim] as i32;
        d += 2;
        if scale_w == 0 || scale_h == 0 {
            return;
        }
        let s = scaled_height(shape, scale_h) >> 1;
        let w = (w * scale_w as i32) >> 8;
        let h = (h * scale_h as i32) >> 8;
        let mut x = DSC_DOOR_X[t] as i16 as i32 + w + 200;
        y = y + 4 - s + h - u;
        let fade = &level.overlays[d.min(7)];
        x -= scaled_width(shape, scale_w) >> 1;
        y -= s;
        let how = Draw {
            flip: false,
            scale: Some((scale_w, scale_h)),
            table: None,
            fade: Some(fade),
        };
        draw_shape(screen, shape, x, y, clip, how);
    }
}
