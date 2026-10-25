"""Writes crates/lol-data/src/tables.rs from ScummVM's create_kyradat resources.

The tables the original program (LANDS.EXE of the CD version) holds -- where the view puts
walls and decorations, the wall sets of the levels, monsters' scaling, spells, the
characters to choose from, buttons -- are not in the game's data files. ScummVM keeps them
in devtools/create_kyradat/resources (GPL, like this project). Run from the project folder:

    python -I tools/tables_from_scummvm.py ref/scummvm
"""
import os
import re
import sys

SOURCE = "devtools/create_kyradat/resources"
LANGUAGES = [("German", "german"), ("English", "english"), ("French", "french")]

# Element types: Rust type of a plain table.
PLAIN = {"byte": "u8", "uint16": "u16", "uint32": "u32", "char": "&str"}

# Structs: (Rust name, fields (name, Rust type, count: None for one value)).
STRUCTS = {
    "SpellProperty": ("SpellProperty", [
        ("spell_name_code", "u16", None), ("mp_required", "u16", 4), ("field_a", "u16", None),
        ("field_c", "u16", None), ("hp_required", "u16", 4), ("field_16", "u16", None),
        ("field_18", "u16", None), ("flags", "u16", None)]),
    "CompassDef": ("CompassDef", [
        ("shape_index", "u8", None), ("x", "i8", None), ("y", "i8", None), ("flags", "u8", None)]),
    "FlyingObjectShape": ("FlyingObjectShape", [
        ("shape_front", "u8", None), ("shape_back", "u8", None), ("shape_left", "u8", None),
        ("draw_flags", "u8", None), ("flip_flags", "u8", None)]),
    "LoLButtonDef": ("ButtonDef", [
        ("flags", "u16", None), ("key_code", "u16", None), ("key_code2", "u16", None),
        ("x", "i16", None), ("y", "i16", None), ("w", "u16", None), ("h", "u16", None),
        ("index", "u16", None), ("screen_dim", "u16", None)]),
    "LoLCharacter": ("CharacterDef", [
        ("flags", "u16", None), ("name", "&'static str", None), ("race_class_sex", "u8", None),
        ("id", "i16", None), ("cur_face_frame", "u8", None), ("temp_face_frame", "u8", None),
        ("scream_sfx", "u8", None), ("items_might", "u16", 8),
        ("protection_against_items", "u16", 8), ("item_protection", "u16", None),
        ("hit_points_cur", "i16", None), ("hit_points_max", "u16", None),
        ("magic_points_cur", "i16", None), ("magic_points_max", "u16", None),
        ("field_41", "u8", None), ("damage_suffered", "u16", None), ("weapon_hit", "u16", None),
        ("total_might_modifier", "u16", None), ("total_protection_modifier", "u16", None),
        ("might", "u16", None), ("protection", "u16", None),
        ("next_anim_update_countdown", "i16", None), ("items", "u16", 11),
        ("skill_levels", "u8", 3), ("skill_modifiers", "u8", 3), ("experience_pts", "u32", 3),
        ("character_update_events", "u8", 5), ("character_update_delay", "u8", 5)]),
}

TABLE = re.compile(
    r"static const (\w+) (?:\*const )?(kLoL(\w+?)DOSCD(\w*))\[(\d+)\] = (\{.*?\n\});", re.S)


def tokens(text):
    return re.findall(r'"(?:[^"\\]|\\.)*"|-?0x[0-9A-Fa-f]+|-?\d+|[{},]', text)


def parse(toks, i=0):
    """A C initializer from token i: a value or a nested list. Returns (value, next i)."""
    if toks[i] == "{":
        items, i = [], i + 1
        while toks[i] != "}":
            item, i = parse(toks, i)
            items.append(item)
            if toks[i] == ",":
                i += 1
        return items, i + 1
    return toks[i], i + 1


def number(tok, rust):
    value = int(tok, 0)
    if rust.startswith("i"):
        bits = int(rust[1:])
        if value >= 1 << (bits - 1):
            value -= 1 << bits
    return str(value)


def snake(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name).upper()


def literal(value, rust):
    if rust in ("&str", "&'static str"):
        return value
    return number(value, rust)


def struct_literal(kind, value):
    name, fields = STRUCTS[kind]
    parts = []
    for (field, rust, count), item in zip(fields, value):
        if count is None:
            parts.append(f"{field}: {literal(item, rust)}")
        else:
            parts.append(f"{field}: [{', '.join(literal(v, rust) for v in item)}]")
    return f"{name} {{ {', '.join(parts)} }}"


def tables(path):
    with open(path, encoding="latin-1") as f:
        text = f.read()
    for kind, _full, name, suffix, count, body in TABLE.findall(text):
        value, _ = parse(tokens(body))
        yield kind, name, suffix, int(count), value


def table_code(kind, name, count, value):
    const = snake(name)
    if kind in PLAIN:
        rust = PLAIN[kind]
        typ = "&str" if rust == "&str" else rust
        items = [literal(v, rust) for v in value]
        lines = []
        for k in range(0, len(items), 12 if rust != "&str" else 1):
            lines.append("    " + ", ".join(items[k:k + (12 if rust != "&str" else 1)]) + ",")
        return f"pub const {const}: [{typ}; {count}] = [\n" + "\n".join(lines) + "\n];\n"
    rust_name = STRUCTS[kind][0]
    lines = [f"    {struct_literal(kind, v)}," for v in value]
    return f"pub const {const}: [{rust_name}; {count}] = [\n" + "\n".join(lines) + "\n];\n"


def opcode_tables(root):
    """The names of the program's functions as the scripts call them, in their order
    (ScummVM's `setupOpcodeTable`); empty where ScummVM has none."""
    with open(os.path.join(root, "engines/kyra/script/script_lol.cpp"), encoding="latin-1") as f:
        text = f.read()
    body = text[text.index("void LoLEngine::setupOpcodeTable()"):]
    tables, current = {}, None
    for line in body.splitlines():
        line = line.strip()
        m = re.match(r"Set(?:Tim)?OpcodeTable\((\w+)\);", line)
        if m:
            current = tables.setdefault(m.group(1), [])
            continue
        m = re.match(r"Opcode(?:Tim)?\((?:olol|tlol|o1)_(\w+)\);", line)
        if m and current is not None:
            current.append(m.group(1))
        elif re.match(r"Opcode(?:Tim)?UnImpl\(\);", line) and current is not None:
            current.append("")
        if line == "}":
            break
    return tables


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "ref/scummvm"
    source = os.path.join(root, SOURCE)
    out = [
        "//! Tables of the original program (`LANDS.EXE`, CD version), from ScummVM's",
        "//! `devtools/create_kyradat/resources/lol_dos_cd*.h` (GPL). Written by",
        "//! `tools/tables_from_scummvm.py`; do not edit by hand.",
        "",
        "#![allow(clippy::unreadable_literal)]",
        "",
        "use crate::paths::Language;",
        "",
    ]
    for kind, (name, fields) in STRUCTS.items():
        out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
        out.append(f"pub struct {name} {{")
        for field, rust, count in fields:
            typ = rust if count is None else f"[{rust}; {count}]"
            out.append(f"    pub {field}: {typ},")
        out.append("}")
        out.append("")
    for kind, name, suffix, count, value in tables(os.path.join(source, "lol_dos_cd.h")):
        if suffix:
            continue
        out.append(table_code(kind, name, count, value))
    # Tables each language has its own of.
    per_language = {}
    for language, file in LANGUAGES:
        path = os.path.join(source, f"lol_dos_cd_{file}.h")
        for kind, name, _suffix, count, value in tables(path):
            const = f"{snake(name)}_{language.upper()}"
            out.append(table_code(kind, name, count, value).replace(
                f"pub const {snake(name)}:", f"pub const {const}:"))
            per_language.setdefault((name, kind, count), []).append((language, const))
    for (name, kind, count), consts in per_language.items():
        rust = STRUCTS[kind][0] if kind in STRUCTS else PLAIN[kind]
        fn = snake(name).lower()
        out.append(f"pub fn {fn}(language: Language) -> &'static [{rust}] {{")
        out.append("    match language {")
        for language, const in consts:
            out.append(f"        Language::{language} => &{const},")
        out.append("    }")
        out.append("}")
        out.append("")
    names = {"_opcodes": "SCRIPT_FUNCTIONS", "_timIntroOpcodes": "TIM_INTRO_FUNCTIONS",
             "_timOutroOpcodes": "TIM_OUTRO_FUNCTIONS", "_timIngameOpcodes": "TIM_INGAME_FUNCTIONS"}
    for table, functions in opcode_tables(root).items():
        const = names.get(table, snake(table.strip("_")))
        out.append(f"/// From `engines/kyra/script/script_lol.cpp` (`{table}`).")
        out.append(f"pub const {const}: [&str; {len(functions)}] = [")
        out.extend(f'    "{f}",' for f in functions)
        out.append("];")
        out.append("")
    path = os.path.join("crates", "lol-data", "src", "tables.rs")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out).rstrip() + "\n")
    print(f"{path} geschrieben")


if __name__ == "__main__":
    main()
