//! The game's records (ScummVM's `lol.h`, `kyra_rpg.h`, `gui.h`): characters, items, monsters,
//! flying objects, the level's blocks, buttons.

use lol_data::tables::CharacterDef;

pub type Item = u16;

#[derive(Debug, Clone, Default)]
pub struct Character {
    pub flags: u16,
    pub name: Vec<u8>,
    pub race_class_sex: u8,
    pub id: i16,
    pub cur_face_frame: u8,
    pub temp_face_frame: u8,
    pub scream_sfx: u8,
    /// Which of the program's tables of default modifiers (0 man, 1 woman, 2 Kieran, 3 Ak'shel).
    pub default_modifiers: usize,
    pub items_might: [u16; 8],
    pub protection_against_items: [u16; 8],
    pub item_protection: u16,
    pub hit_points_cur: i16,
    pub hit_points_max: u16,
    pub magic_points_cur: i16,
    pub magic_points_max: u16,
    pub field_41: u8,
    pub damage_suffered: u16,
    pub weapon_hit: u16,
    pub total_might_modifier: u16,
    pub total_protection_modifier: u16,
    pub might: u16,
    pub protection: u16,
    pub next_anim_update_countdown: i16,
    pub items: [Item; 11],
    pub skill_levels: [u8; 3],
    pub skill_modifiers: [i8; 3],
    pub experience_pts: [i32; 3],
    pub character_update_events: [u8; 5],
    pub character_update_delay: [u8; 5],
}

impl Character {
    pub fn from_def(d: &CharacterDef) -> Self {
        Self {
            flags: d.flags,
            name: d.name.as_bytes().to_vec(),
            race_class_sex: d.race_class_sex,
            id: d.id,
            cur_face_frame: d.cur_face_frame,
            temp_face_frame: d.temp_face_frame,
            scream_sfx: d.scream_sfx,
            default_modifiers: 0,
            items_might: d.items_might,
            protection_against_items: d.protection_against_items,
            item_protection: d.item_protection,
            hit_points_cur: d.hit_points_cur,
            hit_points_max: d.hit_points_max,
            magic_points_cur: d.magic_points_cur,
            magic_points_max: d.magic_points_max,
            field_41: d.field_41,
            damage_suffered: d.damage_suffered,
            weapon_hit: d.weapon_hit,
            total_might_modifier: d.total_might_modifier,
            total_protection_modifier: d.total_protection_modifier,
            might: d.might,
            protection: d.protection,
            next_anim_update_countdown: d.next_anim_update_countdown,
            items: d.items,
            skill_levels: d.skill_levels,
            skill_modifiers: d.skill_modifiers.map(|v| v as i8),
            experience_pts: d.experience_pts.map(|v| v as i32),
            character_update_events: d.character_update_events,
            character_update_delay: d.character_update_delay,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MonsterProperty {
    pub shape_index: u8,
    pub max_width: u8,
    pub fighting_stats: [u16; 9],
    pub items_might: [u16; 8],
    pub protection_against_items: [u16; 8],
    pub item_protection: u16,
    pub hit_points: u16,
    pub speed_total_wait_ticks: u8,
    pub skill_level: u8,
    pub flags: u16,
    pub unk5: u16,
    pub num_dist_attacks: u16,
    pub num_dist_weapons: u16,
    pub dist_weapons: [u16; 3],
    pub attack_skill_chance: u8,
    pub attack_skill_type: u8,
    pub defense_skill_chance: u8,
    pub defense_skill_type: u8,
    pub sounds: [u8; 3],
}

/// What items and monsters share (`LoLObject`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Object {
    pub next_assigned_object: u16,
    pub next_draw_object: u16,
    pub flying_height: u8,
    pub block: u16,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Monster {
    pub obj: Object,
    pub dest_direction: u8,
    pub shift_step: i8,
    pub dest_x: u16,
    pub dest_y: u16,
    pub hit_offs_x: i8,
    pub hit_offs_y: i8,
    pub current_sub_frame: u8,
    pub mode: u8,
    pub fight_cur_tick: i8,
    pub id: u8,
    pub direction: u8,
    pub facing: u8,
    pub flags: u16,
    pub damage_received: u16,
    pub hit_points: i16,
    pub speed_tick: u8,
    pub type_: u8,
    /// Index into the monster properties (`properties` pointer).
    pub properties: usize,
    pub num_dist_attacks: u8,
    pub cur_dist_weapon: u8,
    pub dist_attack_tick: i8,
    pub assigned_items: u16,
    pub equipment_shapes: [u8; 4],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LolItem {
    pub obj: Object,
    pub level: i8,
    pub item_property_index: u16,
    pub shp_cur_frame_flg: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ItemProperty {
    pub name_string_id: u16,
    pub shp_index: u8,
    pub flags: u16,
    pub type_: u16,
    pub item_script_func: u8,
    pub might: i8,
    pub skill: u8,
    pub protection: u8,
    pub unk_b: u16,
    pub unk_d: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActiveSpell {
    pub spell: u8,
    pub char_num: u8,
    pub level: u8,
    pub target: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FlyingObject {
    pub enable: u8,
    pub object_type: u8,
    pub attacker_id: u16,
    pub item: Item,
    pub x: u16,
    pub y: u16,
    pub flying_height: u8,
    pub direction: u8,
    pub distance: u8,
    pub field_d: i8,
    pub c: u8,
    pub flags: u8,
    pub wall_flags: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MapLegendData {
    pub shape_index: u8,
    pub enable: bool,
    pub y: i8,
    pub string_id: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LevelBlockProperty {
    pub walls: [u8; 4],
    pub assigned_objects: u16,
    pub draw_objects: u16,
    pub direction: u8,
    pub flags: u16,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenDoorState {
    pub block: u16,
    pub wall: i8,
    pub state: i8,
}

/// What a button does when used (ScummVM's member function pointers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Callback {
    #[default]
    None,
    /// One of the play field's (`_buttonCallbacks[index]`, the index of `LoLButtonDef`).
    Playfield(u8),
    /// A menu's (index of the menu item).
    Menu(MenuCb),
    ScrollUp,
    ScrollDown,
    RedrawButton,
    RedrawShadedButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCb {
    MainMenu,
    LoadMenu,
    SaveMenu,
    DeleteMenu,
    OptionsMenu,
    AudioMenu,
    DeathMenu,
    SavenameMenu,
    ChoiceMenu,
}

#[derive(Debug, Clone, Default)]
pub struct Button {
    pub index: u16,
    pub key_code: u16,
    pub key_code2: u16,
    pub data0_val1: u8,
    pub data1_val1: u8,
    pub data2_val1: u8,
    pub data3_val1: u8,
    pub flags: u16,
    pub data0_shape: Option<Vec<u8>>,
    pub data1_shape: Option<Vec<u8>>,
    pub data2_shape: Option<Vec<u8>>,
    pub data0_callback: Callback,
    pub data1_callback: Callback,
    pub data2_callback: Callback,
    pub dim_table_index: u16,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub data0_val2: u8,
    pub data0_val3: u8,
    pub data1_val2: u8,
    pub data1_val3: u8,
    pub data2_val2: u8,
    pub data2_val3: u8,
    pub data3_val2: u8,
    pub data3_val3: u8,
    pub flags2: u16,
    pub mouse_wheel: i8,
    pub button_callback: Callback,
    pub arg: u16,
}

impl Button {
    /// A button with the defaults `gui_initButton` and the menus set.
    pub fn blank() -> Self {
        Self {
            data0_val2: 0xfe,
            data1_val2: 0xfe,
            data2_val2: 0xfe,
            data0_val3: 1,
            data1_val3: 1,
            data2_val3: 1,
            ..Self::default()
        }
    }
}

/// A printf argument (`%d`, `%s`, `%c` …).
#[derive(Debug, Clone)]
pub enum Arg {
    Int(i32),
    Str(Vec<u8>),
}

impl From<i32> for Arg {
    fn from(v: i32) -> Self {
        Arg::Int(v)
    }
}

impl From<&[u8]> for Arg {
    fn from(v: &[u8]) -> Self {
        Arg::Str(v.to_vec())
    }
}

impl From<Vec<u8>> for Arg {
    fn from(v: Vec<u8>) -> Self {
        Arg::Str(v)
    }
}

impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Str(v.as_bytes().to_vec())
    }
}

/// `vsnprintf` for the game's strings: `%d %i %u %x %c %s %%`, with flags, width and precision
/// as C has them (enough for the game's formats).
pub fn sprintf(format: &[u8], args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut next = 0;
    while i < format.len() {
        let c = format[i];
        if c == 0 {
            break;
        }
        if c != b'%' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        let mut left = false;
        let mut zero = false;
        let mut plus = false;
        while i < format.len() && matches!(format[i], b'-' | b'0' | b'+' | b' ' | b'#') {
            match format[i] {
                b'-' => left = true,
                b'0' => zero = true,
                b'+' => plus = true,
                _ => {}
            }
            i += 1;
        }
        let mut width = 0usize;
        while i < format.len() && format[i].is_ascii_digit() {
            width = width * 10 + (format[i] - b'0') as usize;
            i += 1;
        }
        let mut precision = None;
        if i < format.len() && format[i] == b'.' {
            i += 1;
            let mut p = 0usize;
            while i < format.len() && format[i].is_ascii_digit() {
                p = p * 10 + (format[i] - b'0') as usize;
                i += 1;
            }
            precision = Some(p);
        }
        while i < format.len() && matches!(format[i], b'l' | b'h') {
            i += 1;
        }
        let Some(&conv) = format.get(i) else { break };
        i += 1;
        let arg = args.get(next).cloned();
        let text: Vec<u8> = match conv {
            b'%' => {
                out.push(b'%');
                continue;
            }
            b'd' | b'i' | b'u' => {
                next += 1;
                let v = match arg {
                    Some(Arg::Int(v)) => v,
                    _ => 0,
                };
                let v = if conv == b'u' {
                    v as u32 as i64
                } else {
                    v as i64
                };
                let mut s = v.abs().to_string().into_bytes();
                if let Some(p) = precision {
                    while s.len() < p {
                        s.insert(0, b'0');
                    }
                }
                if v < 0 {
                    s.insert(0, b'-');
                } else if plus {
                    s.insert(0, b'+');
                }
                s
            }
            b'x' | b'X' => {
                next += 1;
                let v = match arg {
                    Some(Arg::Int(v)) => v as u32,
                    _ => 0,
                };
                if conv == b'x' {
                    format!("{v:x}").into_bytes()
                } else {
                    format!("{v:X}").into_bytes()
                }
            }
            b'c' => {
                next += 1;
                match arg {
                    Some(Arg::Int(v)) => vec![v as u8],
                    Some(Arg::Str(s)) => s.first().copied().into_iter().collect(),
                    None => Vec::new(),
                }
            }
            b's' => {
                next += 1;
                let mut s = match arg {
                    Some(Arg::Str(s)) => s,
                    Some(Arg::Int(v)) => v.to_string().into_bytes(),
                    None => Vec::new(),
                };
                if let Some(end) = s.iter().position(|&b| b == 0) {
                    s.truncate(end);
                }
                if let Some(p) = precision {
                    s.truncate(p);
                }
                s
            }
            other => vec![b'%', other],
        };
        let pad = width.saturating_sub(text.len());
        if left {
            out.extend_from_slice(&text);
            out.extend(std::iter::repeat_n(b' ', pad));
        } else {
            let fill = if zero && conv != b's' { b'0' } else { b' ' };
            if fill == b'0' && text.first() == Some(&b'-') {
                out.push(b'-');
                out.extend(std::iter::repeat_n(b'0', pad));
                out.extend_from_slice(&text[1..]);
            } else {
                out.extend(std::iter::repeat_n(fill, pad));
                out.extend_from_slice(&text);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_c() {
        let out = sprintf(
            b"%s hat %d/%d LP%c",
            &["Kieran".into(), 7.into(), 20.into(), Arg::Int(b'.' as i32)],
        );
        assert_eq!(out, b"Kieran hat 7/20 LP.");
        assert_eq!(
            sprintf(b"%03d|%-4s|%%", &[5.into(), "ab".into()]),
            b"005|ab  |%"
        );
    }
}
