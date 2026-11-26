use crate::model::{BuildCategory, BuildStyle, MaterialPalette, MixMode, NoiseLevel, SlotExtraList, UiLang};

/// Maximum number of saved favorite builds persisted on disk.
pub const MAX_SAVED_FAVORITES: usize = 24;

/// Persistent build configuration convertible to/from `BG2:...` share codes and favorites file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPreset {
    pub name: String,
    pub width: u16,
    pub depth: u16,
    pub height: u16,
    pub category: BuildCategory,
    pub style: BuildStyle,
    pub roof_raw: u8,
    pub toggles: [u8; 5],
    pub noise_level: NoiseLevel,
    pub adapt_terrain: bool,
    pub air_only: bool,
    pub wear_pct: u8,
    pub overgrowth_pct: u8,
    pub description: String,
    pub seed: u64,
    pub palette_manual: bool,
    pub palette: MaterialPalette,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatFilter {
    All,
    Wood,
    Stone,
    ColorGlass,
    Special,
}

impl MatFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Wood,
        Self::Stone,
        Self::ColorGlass,
        Self::Special,
    ];

    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::All => "All",
                Self::Wood => "Wood",
                Self::Stone => "Stone",
                Self::ColorGlass => "Color",
                Self::Special => "Decor",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::All => "Все",
            Self::Wood => "Дерево",
            Self::Stone => "Камень",
            Self::ColorGlass => "Цвет",
            Self::Special => "Декор",
        }
    }
}

fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash = 0x811C_9DC5u32;
    for &b in data {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 4).div_ceil(3));
    let mut i = 0;
    while i < input.len() {
        let b0 = u32::from(input[i]);
        let b1 = if i + 1 < input.len() { u32::from(input[i + 1]) } else { 0 };
        let b2 = if i + 2 < input.len() { u32::from(input[i + 2]) } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_URL[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_URL[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < input.len() {
            out.push(B64_URL[((triple >> 6) & 0x3F) as usize] as char);
        }
        if i + 2 < input.len() {
            out.push(B64_URL[(triple & 0x3F) as usize] as char);
        }
        i += 3;
    }
    out
}

fn b64_val(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a' + 26)),
        b'0'..=b'9' => Some(u32::from(b - b'0' + 52)),
        b'-' | b'+' => Some(62),
        b'_' | b'/' => Some(63),
        _ => None,
    }
}

fn err_msg(lang: UiLang, en: &str, ru: &str) -> String {
    match lang {
        UiLang::En => en.to_owned(),
        UiLang::Ru => ru.to_owned(),
    }
}

fn b64_decode(s: &str, lang: UiLang) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = s
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace() && b != b'=')
        .collect();
    if bytes.len() % 4 == 1 {
        return Err(err_msg(lang, "Invalid Share code length", "Некорректная длина Share-кода"));
    }
    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    let mut i = 0;
    while i < bytes.len() {
        let v0 = b64_val(bytes[i]).ok_or_else(|| err_msg(lang, "Invalid character in Share code", "Недопустимый символ в Share-коде"))?;
        let v1 = b64_val(*bytes.get(i + 1).ok_or_else(|| err_msg(lang, "Truncated Share code", "Обрезанный Share-код"))?)
            .ok_or_else(|| err_msg(lang, "Invalid character in Share code", "Недопустимый символ в Share-коде"))?;
        let v2 = if let Some(&c) = bytes.get(i + 2) {
            Some(b64_val(c).ok_or_else(|| err_msg(lang, "Invalid character in Share code", "Недопустимый символ в Share-коде"))?)
        } else {
            None
        };
        let v3 = if let Some(&c) = bytes.get(i + 3) {
            Some(b64_val(c).ok_or_else(|| err_msg(lang, "Invalid character in Share code", "Недопустимый символ в Share-коде"))?)
        } else {
            None
        };
        let triple = (v0 << 18) | (v1 << 12) | (v2.unwrap_or(0) << 6) | v3.unwrap_or(0);
        out.push(((triple >> 16) & 0xFF) as u8);
        if v2.is_some() {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if v3.is_some() {
            out.push((triple & 0xFF) as u8);
        }
        i += 4;
    }
    Ok(out)
}

fn push_str(buf: &mut Vec<u8>, s: &str) {
    let raw = s.as_bytes();
    let len = raw.len().min(u16::MAX as usize) as u16;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&raw[..usize::from(len)]);
}

fn read_str(data: &[u8], cursor: &mut usize, lang: UiLang) -> Result<String, String> {
    if *cursor + 2 > data.len() {
        return Err(err_msg(lang, "Corrupted string in Share code", "Повреждённая строка в Share-коде"));
    }
    let len = u16::from_le_bytes([data[*cursor], data[*cursor + 1]]) as usize;
    *cursor += 2;
    if *cursor + len > data.len() {
        return Err(err_msg(lang, "Truncated string in Share code", "Обрезанная строка в Share-коде"));
    }
    let s = String::from_utf8(data[*cursor..*cursor + len].to_vec())
        .map_err(|_| err_msg(lang, "Invalid UTF-8 string in Share code", "Неверная UTF-8 строка в Share-коде"))?;
    *cursor += len;
    Ok(s)
}

/// Encodes a preset configuration into a compact `BG2:...` share code with FNV-1a checksum (`v4` includes wear & overgrowth %).
pub fn encode_share_code(p: &SavedPreset) -> String {
    let mut buf = Vec::with_capacity(228);
    buf.extend_from_slice(b"BG2\x04");
    buf.extend_from_slice(&p.width.to_le_bytes());
    buf.extend_from_slice(&p.depth.to_le_bytes());
    buf.extend_from_slice(&p.height.to_le_bytes());
    buf.push(p.category.to_idx());
    buf.push(p.style.to_idx());
    buf.push(p.roof_raw);
    buf.push(p.noise_level.to_idx());
    let flags = u8::from(p.adapt_terrain)
        | (u8::from(p.palette_manual) << 1)
        | (u8::from(p.air_only) << 2);
    buf.push(flags);
    buf.extend_from_slice(&p.toggles);
    buf.extend_from_slice(&p.seed.to_le_bytes());
    push_str(&mut buf, &p.description);
    push_str(&mut buf, &p.palette.name);
    if p.palette_manual {
        for field in [
            &p.palette.foundation,
            &p.palette.wall,
            &p.palette.wall_accent,
            &p.palette.frame,
            &p.palette.trim_wall,
            &p.palette.roof_block,
            &p.palette.roof_stairs,
            &p.palette.roof_slab,
            &p.palette.floor,
            &p.palette.window,
            &p.palette.door,
            &p.palette.fence,
            &p.palette.trapdoor,
            &p.palette.leaves,
            &p.palette.light,
            &p.palette.chimney_block,
            &p.palette.wall_upper,
            &p.palette.wall_trim,
            &p.palette.beam,
            &p.palette.pillar,
            &p.palette.roof_edge,
            &p.palette.floor_upper,
            &p.palette.ceiling,
            &p.palette.window_accent,
            &p.palette.interior_door,
            &p.palette.fence_gate,
            &p.palette.stair_block,
            &p.palette.leaves_alt,
            &p.palette.light_accent,
            &p.palette.carpet,
            &p.palette.paving,
            &p.palette.awning,
        ] {
            push_str(&mut buf, field);
        }
        buf.push(p.palette.mix_mode.to_idx());
        let non_empty: Vec<&SlotExtraList> = p
            .palette
            .extra_lists
            .iter()
            .filter(|e| !e.blocks.is_empty() && usize::from(e.slot) < MaterialPalette::SLOT_COUNT)
            .collect();
        let entry_cnt = non_empty.len().min(u8::MAX as usize);
        buf.push(entry_cnt as u8);
        for entry in non_empty.into_iter().take(entry_cnt) {
            buf.push(entry.slot);
            let bcnt = entry.blocks.len().min(u8::MAX as usize);
            buf.push(bcnt as u8);
            for b in entry.blocks.iter().take(bcnt) {
                push_str(&mut buf, b);
            }
        }
    }
    buf.push(p.wear_pct.min(100));
    buf.push(p.overgrowth_pct.min(100));
    let sum = fnv1a_32(&buf);
    buf.extend_from_slice(&sum.to_le_bytes());
    format!("BG2:{}", b64_encode(&buf))
}

/// Decodes a `BG2:...` share code (`v1` 16-field, `v2` 32-field, `v3` with slot block lists), validating checksum.
pub fn decode_share_code(raw: &str, name_override: Option<&str>) -> Result<SavedPreset, String> {
    decode_share_code_lang(raw, name_override, UiLang::En)
}

/// Decodes a `BG2:...` share code with localized validation errors.
pub fn decode_share_code_lang(
    raw: &str,
    name_override: Option<&str>,
    lang: UiLang,
) -> Result<SavedPreset, String> {
    let trimmed = raw
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | '[' | ']'));
    let payload = trimmed
        .strip_prefix("BG2:")
        .or_else(|| trimmed.strip_prefix("bg2:"))
        .ok_or_else(|| err_msg(lang, "Share code must start with BG2:", "Код должен начинаться с BG2:"))?;
    let bytes = b64_decode(payload, lang)?;
    if bytes.len() < 4 + 24 + 4
        || (&bytes[0..4] != b"BG2\x01"
            && &bytes[0..4] != b"BG2\x02"
            && &bytes[0..4] != b"BG2\x03"
            && &bytes[0..4] != b"BG2\x04")
    {
        return Err(err_msg(lang, "Unsupported Share code version", "Неподдерживаемая версия Share-кода"));
    }
    let is_v2 = &bytes[0..4] == b"BG2\x02";
    let is_v3_or_v4 = &bytes[0..4] == b"BG2\x03" || &bytes[0..4] == b"BG2\x04";
    let is_v4 = &bytes[0..4] == b"BG2\x04";
    let body_len = bytes.len() - 4;
    let stored_sum = u32::from_le_bytes([
        bytes[body_len],
        bytes[body_len + 1],
        bytes[body_len + 2],
        bytes[body_len + 3],
    ]);
    let actual_sum = fnv1a_32(&bytes[..body_len]);
    if stored_sum != actual_sum {
        return Err(err_msg(lang, "Share code checksum mismatch", "Контрольная сумма Share-кода не совпала"));
    }

    let body = &bytes[..body_len];
    let width = u16::from_le_bytes([body[4], body[5]]).clamp(7, 96);
    let depth = u16::from_le_bytes([body[6], body[7]]).clamp(7, 96);
    let height = u16::from_le_bytes([body[8], body[9]]).clamp(5, 48);
    let category = BuildCategory::from_idx(body[10]);
    let style = BuildStyle::from_idx(body[11]);
    let roof_raw = body[12].min(7);
    let noise_level = NoiseLevel::from_idx(body[13]);
    let flags = body[14];
    let adapt_terrain = (flags & 1) != 0;
    let palette_manual = (flags & 2) != 0;
    let air_only = (flags & 4) != 0;
    let toggles = [
        body[15].min(2),
        body[16].min(2),
        body[17].min(2),
        body[18].min(2),
        body[19].min(2),
    ];
    let seed = u64::from_le_bytes([
        body[20], body[21], body[22], body[23], body[24], body[25], body[26], body[27],
    ]);

    let mut cur = 28;
    let description = read_str(body, &mut cur, lang)?;
    let pal_name = read_str(body, &mut cur, lang)?;
    let palette = if palette_manual {
        let foundation = read_str(body, &mut cur, lang)?;
        let wall = read_str(body, &mut cur, lang)?;
        let wall_accent = read_str(body, &mut cur, lang)?;
        let frame = read_str(body, &mut cur, lang)?;
        let trim_wall = read_str(body, &mut cur, lang)?;
        let roof_block = read_str(body, &mut cur, lang)?;
        let roof_stairs = read_str(body, &mut cur, lang)?;
        let roof_slab = read_str(body, &mut cur, lang)?;
        let floor = read_str(body, &mut cur, lang)?;
        let window = read_str(body, &mut cur, lang)?;
        let door = read_str(body, &mut cur, lang)?;
        let fence = read_str(body, &mut cur, lang)?;
        let trapdoor = read_str(body, &mut cur, lang)?;
        let leaves = read_str(body, &mut cur, lang)?;
        let light = read_str(body, &mut cur, lang)?;
        let chimney_block = read_str(body, &mut cur, lang)?;
        if is_v2 || is_v3_or_v4 {
            let mut pal = MaterialPalette {
                name: pal_name,
                foundation,
                wall,
                wall_accent,
                frame,
                trim_wall,
                roof_block,
                roof_stairs,
                roof_slab,
                floor,
                window,
                door,
                fence,
                trapdoor,
                leaves,
                light,
                chimney_block,
                wall_upper: read_str(body, &mut cur, lang)?,
                wall_trim: read_str(body, &mut cur, lang)?,
                beam: read_str(body, &mut cur, lang)?,
                pillar: read_str(body, &mut cur, lang)?,
                roof_edge: read_str(body, &mut cur, lang)?,
                floor_upper: read_str(body, &mut cur, lang)?,
                ceiling: read_str(body, &mut cur, lang)?,
                window_accent: read_str(body, &mut cur, lang)?,
                interior_door: read_str(body, &mut cur, lang)?,
                fence_gate: read_str(body, &mut cur, lang)?,
                stair_block: read_str(body, &mut cur, lang)?,
                leaves_alt: read_str(body, &mut cur, lang)?,
                light_accent: read_str(body, &mut cur, lang)?,
                carpet: read_str(body, &mut cur, lang)?,
                paving: read_str(body, &mut cur, lang)?,
                awning: read_str(body, &mut cur, lang)?,
                mix_mode: MixMode::Patches,
                extra_lists: Vec::new(),
            };
            if is_v3_or_v4 && cur < body.len() {
                pal.mix_mode = MixMode::from_idx(body[cur]);
                cur += 1;
                if cur < body.len() {
                    let entry_cnt = usize::from(body[cur]);
                    cur += 1;
                    for _ in 0..entry_cnt {
                        if cur + 2 > body.len() {
                            return Err(err_msg(lang, "Truncated material list in Share code", "Обрезанный список материалов в Share-коде"));
                        }
                        let slot = body[cur];
                        let bcnt = usize::from(body[cur + 1]);
                        cur += 2;
                        let mut blocks = Vec::with_capacity(bcnt.min(MaterialPalette::MAX_BLOCKS_PER_SLOT));
                        for _ in 0..bcnt {
                            let blk = read_str(body, &mut cur, lang)?;
                            if blocks.len() < MaterialPalette::MAX_BLOCKS_PER_SLOT {
                                blocks.push(blk);
                            }
                        }
                        if !blocks.is_empty() && usize::from(slot) < MaterialPalette::SLOT_COUNT {
                            pal.extra_lists.push(SlotExtraList { slot, blocks });
                        }
                    }
                }
            }
            pal
        } else {
            let mut base = MaterialPalette {
                name: pal_name,
                wall_upper: wall.clone(),
                wall_trim: wall_accent.clone(),
                beam: frame.clone(),
                pillar: frame.clone(),
                roof_edge: roof_block.clone(),
                floor_upper: floor.clone(),
                ceiling: roof_slab.clone(),
                window_accent: window.clone(),
                interior_door: door.clone(),
                fence_gate: "minecraft:oak_fence_gate".into(),
                stair_block: roof_stairs.clone(),
                leaves_alt: leaves.clone(),
                light_accent: if light.contains("soul") {
                    "minecraft:soul_torch".into()
                } else if light.contains("copper") {
                    "minecraft:copper_torch".into()
                } else {
                    "minecraft:torch".into()
                },
                carpet: "minecraft:moss_carpet".into(),
                paving: foundation.clone(),
                awning: "minecraft:red_wool".into(),
                mix_mode: MixMode::Patches,
                extra_lists: Vec::new(),
                foundation,
                wall,
                wall_accent,
                frame,
                trim_wall,
                roof_block,
                roof_stairs,
                roof_slab,
                floor,
                window,
                door,
                fence,
                trapdoor,
                leaves,
                light,
                chimney_block,
            };
            base.fence_gate = base.fence_gate_for_fence().into();
            base
        }
    } else {
        let mut inferred = crate::parser::infer_palette_with_style_and_seed(style, &description, seed);
        if !pal_name.is_empty() {
            inferred.name = pal_name;
        }
        inferred
    };

    let (wear_pct, overgrowth_pct) = if is_v4 && cur + 2 <= body.len() {
        (body[cur].min(100), body[cur + 1].min(100))
    } else {
        (0, 0)
    };

    let name = name_override
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{} ({})", category.label_ru(), style.label_ru()));

    Ok(SavedPreset {
        name,
        width,
        depth,
        height,
        category,
        style,
        roof_raw,
        toggles,
        noise_level,
        adapt_terrain,
        air_only,
        wear_pct,
        overgrowth_pct,
        description,
        seed,
        palette_manual,
        palette,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0F) as usize] as char);
    }
    s
}

fn hex_decode(hex: &str) -> Option<String> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)? as u8;
        let lo = (pair[1] as char).to_digit(16)? as u8;
        out.push((hi << 4) | lo);
    }
    String::from_utf8(out).ok()
}

/// Serializes favorites list for `host.data_files()` storage (`favorites.txt`).
pub fn serialize_favorites(list: &[SavedPreset]) -> String {
    let mut out = String::from("BG_FAVORITES_V1\n");
    for item in list.iter().take(MAX_SAVED_FAVORITES) {
        let code = encode_share_code(item);
        let hex_name = hex_encode(item.name.as_bytes());
        out.push_str(&format!("FAV\t{code}\t{hex_name}\n"));
    }
    out
}

/// Parses favorites list persisted by `serialize_favorites`, ignoring malformed lines gracefully.
pub fn parse_favorites(raw: &str) -> Vec<SavedPreset> {
    let mut list = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("FAV\t") {
            let mut parts = rest.splitn(2, '\t');
            let code = parts.next().unwrap_or("");
            let name_opt = parts.next().and_then(hex_decode);
            if let Ok(preset) = decode_share_code(code, name_opt.as_deref()) {
                list.push(preset);
                if list.len() >= MAX_SAVED_FAVORITES {
                    break;
                }
            }
        }
    }
    list
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn block_matches_filter(slot: usize, id: &str, filter: MatFilter) -> bool {
    let stem = id.strip_prefix("minecraft:").unwrap_or(id);
    match filter {
        MatFilter::All => true,
        MatFilter::Wood => contains_any(
            stem,
            &[
                "oak",
                "pale_oak",
                "spruce",
                "birch",
                "jungle",
                "acacia",
                "mangrove",
                "cherry",
                "bamboo",
                "crimson",
                "warped",
                "planks",
                "log",
                "wood",
                "leaves",
                "azalea",
                "moss",
            ],
        ),
        MatFilter::Stone => contains_any(
            stem,
            &[
                "stone",
                "cobble",
                "deepslate",
                "tuff",
                "andesite",
                "diorite",
                "granite",
                "blackstone",
                "basalt",
                "brick",
                "quartz",
                "sandstone",
                "gravel",
                "mud",
                "obsidian",
            ],
        ),
        MatFilter::ColorGlass => contains_any(
            stem,
            &[
                "terracotta",
                "concrete",
                "glass",
                "carpet",
                "wool",
                "calcite",
                "snow",
                "ice",
                "amethyst",
                "purpur",
                "resin",
            ],
        ),
        MatFilter::Special => {
            matches!(slot, 11 | 26 | 29)
                || contains_any(
                    stem,
                    &[
                        "copper",
                        "lantern",
                        "torch",
                        "lamp",
                        "bulb",
                        "glow",
                        "froglight",
                        "shroomlight",
                        "end_rod",
                        "nether",
                        "prismarine",
                        "end_stone",
                        "iron",
                        "bars",
                        "grate",
                        "amethyst",
                        "azalea",
                        "gilded",
                        "honeycomb",
                    ],
                )
        }
    }
}

fn expand_query_tokens(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase();
    let mut mapped = Vec::new();
    for (ru, en) in [
        ("дуб", "oak"),
        ("тёмн", "dark"),
        ("темн", "dark"),
        ("бледн", "pale_oak"),
        ("ель", "spruce"),
        ("ел", "spruce"),
        ("берёз", "birch"),
        ("берез", "birch"),
        ("вишн", "cherry"),
        ("сакур", "cherry"),
        ("акац", "acacia"),
        ("джунг", "jungle"),
        ("мангр", "mangrove"),
        ("бамбук", "bamboo"),
        ("багр", "crimson"),
        ("искаж", "warped"),
        ("камен", "stone"),
        ("булыж", "cobble"),
        ("мшист", "mossy"),
        ("мох", "moss"),
        ("слан", "deepslate"),
        ("кирпич", "brick"),
        ("медь", "copper"),
        ("медн", "copper"),
        ("кварц", "quartz"),
        ("песч", "sandstone"),
        ("туф", "tuff"),
        ("смол", "resin"),
        ("глин", "mud"),
        ("саман", "mud"),
        ("бетон", "concrete"),
        ("террак", "terracotta"),
        ("стекл", "glass"),
        ("фонар", "lantern"),
        ("факел", "torch"),
        ("свет", "lantern"),
        ("ламп", "lamp"),
        ("ковер", "carpet"),
        ("ковёр", "carpet"),
        ("шерст", "wool"),
        ("ткань", "wool"),
        ("тент", "wool"),
        ("калит", "fence_gate"),
        ("лестн", "stairs"),
        ("плит", "slab"),
        ("дорож", "gravel"),
        ("черн", "black"),
        ("сер", "gray"),
        ("бел", "white"),
        ("зелен", "green"),
        ("зел", "green"),
        ("оранж", "orange"),
        ("желт", "yellow"),
        ("жёлт", "yellow"),
        ("корич", "brown"),
        ("красн", "red"),
        ("син", "blue"),
        ("голуб", "light_blue"),
        ("бирюз", "cyan"),
        ("розов", "pink"),
        ("фиол", "purple"),
        ("листв", "leaves"),
        ("призм", "prismarine"),
        ("пурпур", "purpur"),
        ("незер", "nether"),
        ("ад", "nether"),
        ("энд", "end_"),
        ("базальт", "basalt"),
        ("андезит", "andesite"),
        ("диорит", "diorite"),
        ("гранит", "granite"),
        ("кальцит", "calcite"),
        ("аметист", "amethyst"),
        ("обсидиан", "obsidian"),
        ("снег", "snow"),
        ("снеж", "snow"),
        ("лед", "ice"),
        ("лёд", "ice"),
        ("песок", "sand"),
        ("двер", "door"),
        ("люк", "trapdoor"),
        ("оград", "wall"),
        ("забор", "fence"),
        ("решет", "bars"),
        ("решёт", "bars"),
        ("timber", "planks"),
        ("lumber", "planks"),
        ("marble", "quartz"),
        ("travertine", "sandstone"),
        ("slate", "deepslate"),
        ("masonry", "stone"),
    ] {
        if q.contains(ru) {
            mapped.push(en);
        }
    }
    mapped
}

fn push_unique(out: &mut Vec<String>, items: &[&str]) {
    for &id in items {
        if !out.iter().any(|existing| existing == id) {
            out.push(id.to_owned());
        }
    }
}

fn slot_catalog_raw(slot: usize) -> Vec<String> {
    let mut raw: Vec<String> = Vec::new();
    match slot {
        0 => push_unique(&mut raw, MaterialPalette::FOUNDATION_CHOICES),
        1 | 12 => push_unique(&mut raw, MaterialPalette::WALL_CHOICES),
        2 => push_unique(&mut raw, MaterialPalette::ACCENT_CHOICES),
        3 | 14 => push_unique(&mut raw, MaterialPalette::FRAME_CHOICES),
        4 => {
            for &(b, _, _) in MaterialPalette::ROOF_CHOICES {
                if !raw.iter().any(|x| x == b) {
                    raw.push(b.to_owned());
                }
            }
        }
        5 | 17 => push_unique(&mut raw, MaterialPalette::FLOOR_CHOICES),
        6 | 19 => push_unique(&mut raw, MaterialPalette::WINDOW_CHOICES),
        7 | 20 => push_unique(&mut raw, MaterialPalette::DOOR_CHOICES),
        8 => push_unique(&mut raw, MaterialPalette::TRAPDOOR_CHOICES),
        9 => push_unique(&mut raw, MaterialPalette::TRIM_WALL_CHOICES),
        10 | 25 => push_unique(&mut raw, MaterialPalette::LEAVES_CHOICES),
        11 | 26 => push_unique(&mut raw, MaterialPalette::LIGHT_CHOICES),
        13 => {
            push_unique(&mut raw, MaterialPalette::ACCENT_CHOICES);
            push_unique(&mut raw, MaterialPalette::FRAME_CHOICES);
            push_unique(&mut raw, MaterialPalette::FOUNDATION_CHOICES);
        }
        15 => {
            push_unique(&mut raw, MaterialPalette::FRAME_CHOICES);
            push_unique(&mut raw, MaterialPalette::FENCE_CHOICES);
            push_unique(&mut raw, MaterialPalette::TRIM_WALL_CHOICES);
        }
        16 => {
            for &(b, _, _) in MaterialPalette::ROOF_CHOICES {
                if !raw.iter().any(|x| x == b) {
                    raw.push(b.to_owned());
                }
            }
            push_unique(&mut raw, MaterialPalette::FRAME_CHOICES);
            push_unique(&mut raw, MaterialPalette::ACCENT_CHOICES);
        }
        18 => push_unique(&mut raw, MaterialPalette::CEILING_CHOICES),
        21 => push_unique(&mut raw, MaterialPalette::FENCE_CHOICES),
        22 => push_unique(&mut raw, MaterialPalette::FENCE_GATE_CHOICES),
        23 => {
            for &(_, s, _) in MaterialPalette::ROOF_CHOICES {
                if !raw.iter().any(|x| x == s) {
                    raw.push(s.to_owned());
                }
            }
        }
        24 => push_unique(&mut raw, MaterialPalette::CHIMNEY_CHOICES),
        27 => push_unique(&mut raw, MaterialPalette::CARPET_CHOICES),
        28 => push_unique(&mut raw, MaterialPalette::PAVING_CHOICES),
        _ => push_unique(&mut raw, MaterialPalette::AWNING_CHOICES),
    }
    raw
}

/// Returns all deduplicated architectural catalog blocks plus any world blocks discovered on the client.
pub fn all_catalog_blocks(world_blocks: &[String]) -> Vec<String> {
    let mut raw: Vec<String> = Vec::new();
    for list in [
        MaterialPalette::WALL_CHOICES,
        MaterialPalette::FOUNDATION_CHOICES,
        MaterialPalette::ACCENT_CHOICES,
        MaterialPalette::FRAME_CHOICES,
        MaterialPalette::FLOOR_CHOICES,
        MaterialPalette::CEILING_CHOICES,
        MaterialPalette::WINDOW_CHOICES,
        MaterialPalette::DOOR_CHOICES,
        MaterialPalette::TRAPDOOR_CHOICES,
        MaterialPalette::FENCE_CHOICES,
        MaterialPalette::FENCE_GATE_CHOICES,
        MaterialPalette::TRIM_WALL_CHOICES,
        MaterialPalette::CHIMNEY_CHOICES,
        MaterialPalette::LEAVES_CHOICES,
        MaterialPalette::LIGHT_CHOICES,
        MaterialPalette::CARPET_CHOICES,
        MaterialPalette::PAVING_CHOICES,
        MaterialPalette::AWNING_CHOICES,
    ] {
        push_unique(&mut raw, list);
    }
    for &(b, s, sl) in MaterialPalette::ROOF_CHOICES {
        for id in [b, s, sl] {
            if !raw.iter().any(|existing| existing == id) {
                raw.push(id.to_owned());
            }
        }
    }
    for wb in world_blocks {
        let stem = wb.strip_prefix("minecraft:").unwrap_or(wb);
        if matches!(
            stem,
            "air" | "cave_air" | "void_air" | "water" | "lava" | "barrier" | "structure_void"
                | "light" | "command_block" | "chain_command_block" | "repeating_command_block"
                | "structure_block" | "jigsaw" | "bedrock"
        ) {
            continue;
        }
        if !raw.iter().any(|existing| existing == wb) {
            raw.push(wb.clone());
        }
    }
    raw
}

/// Returns candidate block IDs for a picker screen (either tailored to `slot` or all blocks), filtered by category & RU/EN query.
pub fn filtered_picker_choices(
    slot: usize,
    all_blocks: bool,
    world_blocks: &[String],
    filter: MatFilter,
    query: &str,
) -> Vec<String> {
    let mut raw = slot_catalog_raw(slot);
    if all_blocks {
        for id in all_catalog_blocks(world_blocks) {
            if !raw.iter().any(|existing| existing == &id) {
                raw.push(id);
            }
        }
    }

    let q_lower = query.trim().to_lowercase();
    let en_tokens = expand_query_tokens(&q_lower);

    let out: Vec<String> = raw
        .into_iter()
        .filter(|id| block_matches_filter(slot, id, filter))
        .filter(|id| {
            if q_lower.is_empty() {
                true
            } else {
                let stem = id.strip_prefix("minecraft:").unwrap_or(id);
                stem.contains(&q_lower) || en_tokens.iter().any(|tok| stem.contains(tok))
            }
        })
        .collect();

    if out.is_empty() && filter != MatFilter::All && !q_lower.is_empty() {
        return filtered_picker_choices(slot, all_blocks, world_blocks, MatFilter::All, query);
    }
    out
}

/// Returns candidate block IDs for a given UI material slot (`0..30`), filtered by category & RU/EN query.
pub fn filtered_slot_choices(slot: usize, filter: MatFilter, query: &str) -> Vec<String> {
    filtered_picker_choices(slot, false, &[], filter, query)
}

fn sync_wood_trim_if_needed(pal: &mut MaterialPalette, slot: usize, block_id: &str, sync_wood: bool) {
    if !sync_wood {
        return;
    }
    let stem = block_id.strip_prefix("minecraft:").unwrap_or(block_id);
    let wood = [
        "dark_oak",
        "pale_oak",
        "spruce",
        "birch",
        "jungle",
        "acacia",
        "mangrove",
        "cherry",
        "bamboo",
        "crimson",
        "warped",
        "oak",
    ]
    .into_iter()
    .find(|w| stem.contains(w));
    if let Some(w) = wood
        && matches!(slot, 1 | 3 | 4 | 5 | 7 | 12 | 14 | 17 | 20)
    {
        pal.door = format!("minecraft:{w}_door");
        pal.interior_door = format!("minecraft:{w}_door");
        pal.trapdoor = format!("minecraft:{w}_trapdoor");
        pal.fence = format!("minecraft:{w}_fence");
        pal.fence_gate = format!("minecraft:{w}_fence_gate");
    }
}

/// Sets the primary block ID of slot `slot` (`0..30`), syncing roof stairs/slab & wooden trim when appropriate.
pub fn apply_block_to_slot(pal: &mut MaterialPalette, slot: usize, block_id: &str, sync_wood: bool) {
    pal.set_slot_primary(slot, block_id.to_owned());
    sync_wood_trim_if_needed(pal, slot, block_id, sync_wood);
    pal.name = "Custom Palette".into();
}

/// Appends `block_id` to `slot`'s build list without disturbing the primary block.
pub fn add_block_to_slot(pal: &mut MaterialPalette, slot: usize, block_id: &str, sync_wood: bool) -> bool {
    if pal.add_slot_block(slot, block_id) {
        if !pal.has_extra(slot) {
            sync_wood_trim_if_needed(pal, slot, block_id, sync_wood);
        }
        pal.name = "Custom Palette".into();
        true
    } else {
        false
    }
}

/// Toggles `block_id` inside `slot`'s build list: adds if absent, or removes if present (when >=2 blocks). Returns `true` if now present.
pub fn toggle_block_in_slot(pal: &mut MaterialPalette, slot: usize, block_id: &str, sync_wood: bool) -> bool {
    let blocks = pal.slot_blocks(slot);
    if let Some(pos) = blocks.iter().position(|b| b == block_id) {
        if blocks.len() > 1 {
            pal.remove_slot_block_at(slot, pos);
            pal.name = "Custom Palette".into();
            return false;
        }
        return true;
    }
    add_block_to_slot(pal, slot, block_id, sync_wood)
}

/// Replaces entry `idx` inside `slot`'s build list.
pub fn replace_block_in_slot_at(
    pal: &mut MaterialPalette,
    slot: usize,
    idx: usize,
    block_id: &str,
    sync_wood: bool,
) {
    pal.replace_slot_block_at(slot, idx, block_id);
    if idx == 0 {
        sync_wood_trim_if_needed(pal, slot, block_id, sync_wood);
    }
    pal.name = "Custom Palette".into();
}

/// Removes entry `idx` from `slot`'s build list (down to 1 block minimum).
pub fn remove_block_from_slot_at(pal: &mut MaterialPalette, slot: usize, idx: usize) -> bool {
    let removed = pal.remove_slot_block_at(slot, idx);
    if removed {
        pal.name = "Custom Palette".into();
    }
    removed
}
