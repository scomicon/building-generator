use mod_api::PayloadCodec;

/// Mod version string, falling back when Horizon's prebuilt-api rustc compiles without Cargo env vars.
pub const MOD_VERSION: &str = match option_env!("CARGO_PKG_VERSION") {
    Some(ver) => ver,
    None => "0.4.1",
};

/// UI and chat feedback language supported by the mod (`En` default, switchable to `Ru`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PayloadCodec)]
pub enum UiLang {
    #[default]
    En,
    Ru,
}

impl UiLang {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "eng" | "english" | "анг" | "англ" | "английский" => Some(Self::En),
            "ru" | "rus" | "russian" | "ру" | "рус" | "русский" => Some(Self::Ru),
            _ => None,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::Ru,
            Self::Ru => Self::En,
        }
    }

    pub fn button_label(self) -> &'static str {
        match self {
            Self::En => "Lang: EN",
            Self::Ru => "Язык: RU",
        }
    }
}

/// Supported high-level building categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PayloadCodec)]
pub enum BuildCategory {
    House,
    Tower,
    Barn,
    Pavilion,
    Castle,
    Forge,
    Temple,
    Bridge,
    Settlement,
}

impl BuildCategory {
    pub const ALL: [Self; 9] = [
        Self::House,
        Self::Tower,
        Self::Barn,
        Self::Pavilion,
        Self::Castle,
        Self::Forge,
        Self::Temple,
        Self::Bridge,
        Self::Settlement,
    ];

    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::House => "House",
                Self::Tower => "Tower",
                Self::Barn => "Barn",
                Self::Pavilion => "Pavilion",
                Self::Castle => "Castle",
                Self::Forge => "Forge",
                Self::Temple => "Temple",
                Self::Bridge => "Bridge",
                Self::Settlement => "District",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::House => "Дом",
            Self::Tower => "Башня",
            Self::Barn => "Амбар",
            Self::Pavilion => "Павильон",
            Self::Castle => "Цитадель",
            Self::Forge => "Кузница",
            Self::Temple => "Храм",
            Self::Bridge => "Мост",
            Self::Settlement => "Квартал",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::House => "house",
            Self::Tower => "tower",
            Self::Barn => "barn",
            Self::Pavilion => "pavilion",
            Self::Castle => "castle",
            Self::Forge => "forge",
            Self::Temple => "temple",
            Self::Bridge => "bridge",
            Self::Settlement => "settlement",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "house" | "дом" => Some(Self::House),
            "tower" | "башня" => Some(Self::Tower),
            "barn" | "амбар" | "склад" => Some(Self::Barn),
            "pavilion" | "павильон" | "беседка" => Some(Self::Pavilion),
            "castle" | "citadel" | "замок" | "цитадель" | "крепость" => Some(Self::Castle),
            "forge" | "smithy" | "кузница" | "мастерская" => Some(Self::Forge),
            "temple" | "shrine" | "храм" | "святилище" => Some(Self::Temple),
            "bridge" | "viaduct" | "мост" | "виадук" => Some(Self::Bridge),
            "settlement" | "village" | "town" | "estate" | "квартал" | "деревня" | "поселение" | "усадьба" => {
                Some(Self::Settlement)
            }
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::House => Self::Tower,
            Self::Tower => Self::Barn,
            Self::Barn => Self::Pavilion,
            Self::Pavilion => Self::Castle,
            Self::Castle => Self::Forge,
            Self::Forge => Self::Temple,
            Self::Temple => Self::Bridge,
            Self::Bridge => Self::Settlement,
            Self::Settlement => Self::House,
        }
    }

    pub fn to_idx(self) -> u8 {
        match self {
            Self::House => 0,
            Self::Tower => 1,
            Self::Barn => 2,
            Self::Pavilion => 3,
            Self::Castle => 4,
            Self::Forge => 5,
            Self::Temple => 6,
            Self::Bridge => 7,
            Self::Settlement => 8,
        }
    }

    pub fn from_idx(idx: u8) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or(Self::House)
    }
}

/// Architectural style grammar governing trim, arches, eaves, overhangs, and detailing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PayloadCodec)]
pub enum BuildStyle {
    Auto,
    Medieval,
    Cottage,
    Japanese,
    Gothic,
    Nordic,
    Modern,
    Desert,
    Steampunk,
    Classical,
}

impl BuildStyle {
    pub const ALL: [Self; 10] = [
        Self::Auto,
        Self::Medieval,
        Self::Cottage,
        Self::Japanese,
        Self::Gothic,
        Self::Nordic,
        Self::Modern,
        Self::Desert,
        Self::Steampunk,
        Self::Classical,
    ];

    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::Auto => "Auto (text/noise)",
                Self::Medieval => "Medieval",
                Self::Cottage => "Cottage/Garden",
                Self::Japanese => "Japanese",
                Self::Gothic => "Gothic",
                Self::Nordic => "Nordic",
                Self::Modern => "Modern",
                Self::Desert => "Orient/Desert",
                Self::Steampunk => "Steampunk",
                Self::Classical => "Classical/Rome",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Auto => "Авто (текст/шум)",
            Self::Medieval => "Средневековье",
            Self::Cottage => "Коттедж/Сад",
            Self::Japanese => "Японский",
            Self::Gothic => "Готика",
            Self::Nordic => "Скандинавия",
            Self::Modern => "Модерн",
            Self::Desert => "Восток/Пустыня",
            Self::Steampunk => "Стимпанк",
            Self::Classical => "Античность/Рим",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Medieval,
            Self::Medieval => Self::Cottage,
            Self::Cottage => Self::Japanese,
            Self::Japanese => Self::Gothic,
            Self::Gothic => Self::Nordic,
            Self::Nordic => Self::Modern,
            Self::Modern => Self::Desert,
            Self::Desert => Self::Steampunk,
            Self::Steampunk => Self::Classical,
            Self::Classical => Self::Auto,
        }
    }

    pub fn to_idx(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Medieval => 1,
            Self::Cottage => 2,
            Self::Japanese => 3,
            Self::Gothic => 4,
            Self::Nordic => 5,
            Self::Modern => 6,
            Self::Desert => 7,
            Self::Steampunk => 8,
            Self::Classical => 9,
        }
    }

    pub fn from_idx(idx: u8) -> Self {
        Self::ALL.get(idx as usize).copied().unwrap_or(Self::Auto)
    }
}

/// Cardinal orientation of the building front entrance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PayloadCodec)]
pub enum CardinalFacing {
    North,
    East,
    South,
    West,
}

impl CardinalFacing {
    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::North => "North (-Z)",
                Self::East => "East (+X)",
                Self::South => "South (+Z)",
                Self::West => "West (-X)",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::North => "Север (-Z)",
            Self::East => "Восток (+X)",
            Self::South => "Юг (+Z)",
            Self::West => "Запад (-X)",
        }
    }

    pub fn as_mc(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::East => "east",
            Self::South => "south",
            Self::West => "west",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    pub fn from_yaw(yaw_deg: f32) -> Self {
        let norm = yaw_deg.rem_euclid(360.0);
        if (45.0..135.0).contains(&norm) {
            Self::East
        } else if (135.0..225.0).contains(&norm) {
            Self::South
        } else if (225.0..315.0).contains(&norm) {
            Self::West
        } else {
            Self::North
        }
    }
}

/// Roof geometry style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoofStyle {
    Gable,
    Hip,
    Flat,
    Steep,
    Pagoda,
    Dome,
    Mansard,
}

/// Noise intensity presets selectable in UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PayloadCodec)]
pub enum NoiseLevel {
    Calm,
    Balanced,
    Wild,
}

impl NoiseLevel {
    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::Calm => "Calm",
                Self::Balanced => "Balanced",
                Self::Wild => "Wild Noise",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Calm => "Спокойный",
            Self::Balanced => "Сбалансир.",
            Self::Wild => "Смелый шум",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Calm => Self::Balanced,
            Self::Balanced => Self::Wild,
            Self::Wild => Self::Calm,
        }
    }

    pub fn strength(self) -> f32 {
        match self {
            Self::Calm => 0.25,
            Self::Balanced => 0.65,
            Self::Wild => 0.98,
        }
    }

    pub fn to_idx(self) -> u8 {
        match self {
            Self::Calm => 0,
            Self::Balanced => 1,
            Self::Wild => 2,
        }
    }

    pub fn from_idx(idx: u8) -> Self {
        match idx {
            0 => Self::Calm,
            2 => Self::Wild,
            _ => Self::Balanced,
        }
    }
}

/// Pattern used when a material slot contains multiple blocks in its custom build list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PayloadCodec)]
pub enum MixMode {
    Patches,
    Scatter,
    Stripes,
    Checker,
}

impl MixMode {
    pub fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::Patches => "Patches",
                Self::Scatter => "Scatter",
                Self::Stripes => "Stripes",
                Self::Checker => "Checker",
            },
            UiLang::Ru => self.label_ru(),
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Patches => "Пятна",
            Self::Scatter => "Шум",
            Self::Stripes => "Слои",
            Self::Checker => "Шахмат",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Patches => Self::Scatter,
            Self::Scatter => Self::Stripes,
            Self::Stripes => Self::Checker,
            Self::Checker => Self::Patches,
        }
    }

    pub fn to_idx(self) -> u8 {
        match self {
            Self::Patches => 0,
            Self::Scatter => 1,
            Self::Stripes => 2,
            Self::Checker => 3,
        }
    }

    pub fn from_idx(idx: u8) -> Self {
        match idx {
            1 => Self::Scatter,
            2 => Self::Stripes,
            3 => Self::Checker,
            _ => Self::Patches,
        }
    }
}

/// Extra blocks added to a specific material slot (`0..30`) beyond its primary block.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct SlotExtraList {
    pub slot: u8,
    pub blocks: Vec<String>,
}

/// Complete 30-slot architectural block palette supporting composable multi-block lists per slot.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct MaterialPalette {
    pub name: String,
    // Core slots 0..12
    pub foundation: String,
    pub wall: String,
    pub wall_accent: String,
    pub frame: String,
    pub trim_wall: String,
    pub roof_block: String,
    pub roof_stairs: String,
    pub roof_slab: String,
    pub floor: String,
    pub window: String,
    pub door: String,
    pub fence: String,
    pub trapdoor: String,
    pub leaves: String,
    pub light: String,
    pub chimney_block: String,
    // Extended slots 12..30
    pub wall_upper: String,
    pub wall_trim: String,
    pub beam: String,
    pub pillar: String,
    pub roof_edge: String,
    pub floor_upper: String,
    pub ceiling: String,
    pub window_accent: String,
    pub interior_door: String,
    pub fence_gate: String,
    pub stair_block: String,
    pub leaves_alt: String,
    pub light_accent: String,
    pub carpet: String,
    pub paving: String,
    pub awning: String,
    /// Mixing mode and additional per-slot block lists (`slot -> extra block IDs`).
    pub mix_mode: MixMode,
    pub extra_lists: Vec<SlotExtraList>,
}

impl Default for MaterialPalette {
    fn default() -> Self {
        Self::preset(0)
    }
}

impl MaterialPalette {
    pub const PRESET_COUNT: usize = 17;

    /// Builds extended slots coherently from core palette entries when not explicitly customized.
    #[allow(clippy::too_many_arguments)]
    fn build_full(
        name: &str,
        foundation: &str,
        wall: &str,
        wall_upper: &str,
        wall_accent: &str,
        wall_trim: &str,
        frame: &str,
        beam: &str,
        pillar: &str,
        trim_wall: &str,
        roof_block: &str,
        roof_stairs: &str,
        roof_slab: &str,
        roof_edge: &str,
        floor: &str,
        floor_upper: &str,
        ceiling: &str,
        window: &str,
        window_accent: &str,
        door: &str,
        interior_door: &str,
        fence: &str,
        fence_gate: &str,
        trapdoor: &str,
        stair_block: &str,
        chimney_block: &str,
        leaves: &str,
        leaves_alt: &str,
        light: &str,
        light_accent: &str,
        carpet: &str,
        paving: &str,
        awning: &str,
    ) -> Self {
        Self {
            name: name.into(),
            foundation: foundation.into(),
            wall: wall.into(),
            wall_upper: wall_upper.into(),
            wall_accent: wall_accent.into(),
            wall_trim: wall_trim.into(),
            frame: frame.into(),
            beam: beam.into(),
            pillar: pillar.into(),
            trim_wall: trim_wall.into(),
            roof_block: roof_block.into(),
            roof_stairs: roof_stairs.into(),
            roof_slab: roof_slab.into(),
            roof_edge: roof_edge.into(),
            floor: floor.into(),
            floor_upper: floor_upper.into(),
            ceiling: ceiling.into(),
            window: window.into(),
            window_accent: window_accent.into(),
            door: door.into(),
            interior_door: interior_door.into(),
            fence: fence.into(),
            fence_gate: fence_gate.into(),
            trapdoor: trapdoor.into(),
            stair_block: stair_block.into(),
            chimney_block: chimney_block.into(),
            leaves: leaves.into(),
            leaves_alt: leaves_alt.into(),
            light: light.into(),
            light_accent: light_accent.into(),
            carpet: carpet.into(),
            paving: paving.into(),
            awning: awning.into(),
            mix_mode: MixMode::Patches,
            extra_lists: Vec::new(),
        }
    }

    pub const SLOT_COUNT: usize = 30;
    pub const MAX_BLOCKS_PER_SLOT: usize = 24;

    pub const SLOT_FOUNDATION: usize = 0;
    pub const SLOT_WALL: usize = 1;
    pub const SLOT_WALL_ACCENT: usize = 2;
    pub const SLOT_FRAME: usize = 3;
    pub const SLOT_ROOF: usize = 4;
    pub const SLOT_FLOOR: usize = 5;
    pub const SLOT_WINDOW: usize = 6;
    pub const SLOT_DOOR: usize = 7;
    pub const SLOT_TRAPDOOR: usize = 8;
    pub const SLOT_TRIM_WALL: usize = 9;
    pub const SLOT_LEAVES: usize = 10;
    pub const SLOT_LIGHT: usize = 11;
    pub const SLOT_WALL_UPPER: usize = 12;
    pub const SLOT_WALL_TRIM: usize = 13;
    pub const SLOT_BEAM: usize = 14;
    pub const SLOT_PILLAR: usize = 15;
    pub const SLOT_ROOF_EDGE: usize = 16;
    pub const SLOT_FLOOR_UPPER: usize = 17;
    pub const SLOT_CEILING: usize = 18;
    pub const SLOT_WINDOW_ACCENT: usize = 19;
    pub const SLOT_INTERIOR_DOOR: usize = 20;
    pub const SLOT_FENCE: usize = 21;
    pub const SLOT_FENCE_GATE: usize = 22;
    pub const SLOT_STAIR_BLOCK: usize = 23;
    pub const SLOT_CHIMNEY: usize = 24;
    pub const SLOT_LEAVES_ALT: usize = 25;
    pub const SLOT_LIGHT_ACCENT: usize = 26;
    pub const SLOT_CARPET: usize = 27;
    pub const SLOT_PAVING: usize = 28;
    pub const SLOT_AWNING: usize = 29;

    /// Returns primary block ID for UI slot `0..30`.
    pub fn slot_primary(&self, slot: usize) -> &str {
        match slot {
            0 => &self.foundation,
            1 => &self.wall,
            2 => &self.wall_accent,
            3 => &self.frame,
            4 => &self.roof_block,
            5 => &self.floor,
            6 => &self.window,
            7 => &self.door,
            8 => &self.trapdoor,
            9 => &self.trim_wall,
            10 => &self.leaves,
            11 => &self.light,
            12 => &self.wall_upper,
            13 => &self.wall_trim,
            14 => &self.beam,
            15 => &self.pillar,
            16 => &self.roof_edge,
            17 => &self.floor_upper,
            18 => &self.ceiling,
            19 => &self.window_accent,
            20 => &self.interior_door,
            21 => &self.fence,
            22 => &self.fence_gate,
            23 => &self.stair_block,
            24 => &self.chimney_block,
            25 => &self.leaves_alt,
            26 => &self.light_accent,
            27 => &self.carpet,
            28 => &self.paving,
            _ => &self.awning,
        }
    }

    /// Updates the primary block ID for UI slot `0..30`, syncing `roof_stairs`/`roof_slab` when `slot == 4`.
    pub fn set_slot_primary(&mut self, slot: usize, block_id: String) {
        match slot {
            0 => self.foundation = block_id,
            1 => self.wall = block_id,
            2 => self.wall_accent = block_id,
            3 => self.frame = block_id,
            4 => {
                if let Some(&(b, s, sl)) = Self::ROOF_CHOICES.iter().find(|&&(b, _, _)| b == block_id)
                {
                    self.roof_block = b.to_owned();
                    self.roof_stairs = s.to_owned();
                    self.roof_slab = sl.to_owned();
                } else {
                    self.roof_block = block_id.clone();
                    if let Some(st) = Self::resolve_stair_id(&block_id) {
                        self.roof_stairs = st;
                    }
                    self.roof_slab = Self::resolve_slab_id(&block_id);
                }
            }
            5 => self.floor = block_id,
            6 => self.window = block_id,
            7 => self.door = block_id,
            8 => self.trapdoor = block_id,
            9 => self.trim_wall = block_id,
            10 => self.leaves = block_id,
            11 => self.light = block_id,
            12 => self.wall_upper = block_id,
            13 => self.wall_trim = block_id,
            14 => self.beam = block_id,
            15 => self.pillar = block_id,
            16 => self.roof_edge = block_id,
            17 => self.floor_upper = block_id,
            18 => self.ceiling = block_id,
            19 => self.window_accent = block_id,
            20 => self.interior_door = block_id,
            21 => self.fence = block_id,
            22 => self.fence_gate = block_id,
            23 => self.stair_block = block_id,
            24 => self.chimney_block = block_id,
            25 => self.leaves_alt = block_id,
            26 => self.light_accent = block_id,
            27 => self.carpet = block_id,
            28 => self.paving = block_id,
            _ => self.awning = block_id,
        }
    }

    fn extras_for(&self, slot: usize) -> &[String] {
        let s = slot as u8;
        self.extra_lists
            .iter()
            .find(|e| e.slot == s)
            .map_or(&[], |e| e.blocks.as_slice())
    }

    fn extras_for_mut(&mut self, slot: usize) -> &mut Vec<String> {
        let s = slot as u8;
        if let Some(pos) = self.extra_lists.iter().position(|e| e.slot == s) {
            &mut self.extra_lists[pos].blocks
        } else {
            self.extra_lists.push(SlotExtraList {
                slot: s,
                blocks: Vec::new(),
            });
            let last = self.extra_lists.len() - 1;
            &mut self.extra_lists[last].blocks
        }
    }

    pub fn has_extra(&self, slot: usize) -> bool {
        !self.extras_for(slot).is_empty()
    }

    pub fn slot_extra_count(&self, slot: usize) -> usize {
        self.extras_for(slot).len()
    }

    /// Returns the full list of blocks configured for `slot` (`primary` followed by all extra entries).
    pub fn slot_blocks(&self, slot: usize) -> Vec<String> {
        let extras = self.extras_for(slot);
        let mut out = Vec::with_capacity(1 + extras.len());
        out.push(self.slot_primary(slot).to_owned());
        out.extend(extras.iter().cloned());
        out
    }

    pub fn slot_contains(&self, slot: usize, block_id: &str) -> bool {
        self.slot_primary(slot) == block_id || self.extras_for(slot).iter().any(|b| b == block_id)
    }

    /// Appends a block ID to the list of blocks for `slot` (up to `MAX_BLOCKS_PER_SLOT`).
    pub fn add_slot_block(&mut self, slot: usize, block_id: &str) -> bool {
        if self.slot_extra_count(slot) + 1 >= Self::MAX_BLOCKS_PER_SLOT {
            return false;
        }
        self.extras_for_mut(slot).push(block_id.to_owned());
        true
    }

    /// Replaces entry at `idx` in `slot`'s block list.
    pub fn replace_slot_block_at(&mut self, slot: usize, idx: usize, block_id: &str) {
        if idx == 0 {
            self.set_slot_primary(slot, block_id.to_owned());
        } else {
            let extra = self.extras_for_mut(slot);
            if let Some(target) = extra.get_mut(idx - 1) {
                *target = block_id.to_owned();
            }
        }
    }

    /// Removes entry `idx` from `slot`'s block list; shrinks extras or promotes entry #1 when removing #0. Returns `false` if only 1 block remains.
    pub fn remove_slot_block_at(&mut self, slot: usize, idx: usize) -> bool {
        let s = slot as u8;
        let Some(pos) = self.extra_lists.iter().position(|e| e.slot == s) else {
            return false;
        };
        if self.extra_lists[pos].blocks.is_empty() {
            self.extra_lists.remove(pos);
            return false;
        }
        if idx == 0 {
            let promoted = self.extra_lists[pos].blocks.remove(0);
            self.set_slot_primary(slot, promoted);
        } else if idx - 1 < self.extra_lists[pos].blocks.len() {
            self.extra_lists[pos].blocks.remove(idx - 1);
        } else {
            return false;
        }
        if self.extra_lists[pos].blocks.is_empty() {
            self.extra_lists.remove(pos);
        }
        true
    }

    pub fn clear_slot_extras(&mut self, slot: usize) {
        let s = slot as u8;
        self.extra_lists.retain(|e| e.slot != s);
    }

    /// Selects a block ID from `slot`'s block list at `(u, y, v)` according to `self.mix_mode`.
    pub fn pick(&self, slot: usize, seed: u64, u: i32, y: i32, v: i32) -> String {
        let extras = self.extras_for(slot);
        let primary = self.slot_primary(slot);
        if extras.is_empty() {
            return primary.to_owned();
        }
        let len = 1 + extras.len();
        let idx = match self.mix_mode {
            MixMode::Patches => {
                let n = smooth_mix_01(seed ^ ((slot as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)), u, y, v);
                ((n * len as f64).floor() as usize).min(len - 1)
            }
            MixMode::Scatter => {
                let h = mix_hash(seed ^ ((slot as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)), u, y, v);
                (h as usize) % len
            }
            MixMode::Stripes => (y.rem_euclid(len as i32)) as usize,
            MixMode::Checker => ((u + y + v).rem_euclid(len as i32)) as usize,
        };
        if idx == 0 {
            primary.to_owned()
        } else {
            extras[idx - 1].clone()
        }
    }

    /// Resolves matching slab ID for a full block ID.
    pub fn resolve_slab_id(block_id: &str) -> String {
        if block_id.ends_with("_slab") {
            return block_id.to_owned();
        }
        if let Some(stem) = block_id.strip_suffix("_planks") {
            return format!("{stem}_slab");
        }
        if let Some(&(_, _, sl)) = Self::ROOF_CHOICES.iter().find(|&&(b, _, _)| b == block_id) {
            return sl.to_owned();
        }
        match block_id {
            "minecraft:stone_bricks" => "minecraft:stone_brick_slab".to_owned(),
            "minecraft:mossy_stone_bricks" => "minecraft:mossy_stone_brick_slab".to_owned(),
            "minecraft:deepslate_bricks" => "minecraft:deepslate_brick_slab".to_owned(),
            "minecraft:deepslate_tiles" => "minecraft:deepslate_tile_slab".to_owned(),
            "minecraft:polished_blackstone_bricks" => "minecraft:polished_blackstone_brick_slab".to_owned(),
            "minecraft:tuff_bricks" => "minecraft:tuff_brick_slab".to_owned(),
            "minecraft:mud_bricks" => "minecraft:mud_brick_slab".to_owned(),
            "minecraft:resin_bricks" => "minecraft:resin_brick_slab".to_owned(),
            "minecraft:bricks" => "minecraft:brick_slab".to_owned(),
            "minecraft:prismarine" => "minecraft:prismarine_slab".to_owned(),
            "minecraft:prismarine_bricks" => "minecraft:prismarine_brick_slab".to_owned(),
            "minecraft:dark_prismarine" => "minecraft:dark_prismarine_slab".to_owned(),
            "minecraft:purpur_block" => "minecraft:purpur_slab".to_owned(),
            "minecraft:quartz_block" | "minecraft:quartz_bricks" | "minecraft:quartz_pillar" => {
                "minecraft:quartz_slab".to_owned()
            }
            "minecraft:smooth_quartz" => "minecraft:smooth_quartz_slab".to_owned(),
            "minecraft:sandstone" | "minecraft:cut_sandstone" => "minecraft:sandstone_slab".to_owned(),
            "minecraft:smooth_sandstone" => "minecraft:smooth_sandstone_slab".to_owned(),
            "minecraft:red_sandstone" | "minecraft:cut_red_sandstone" => "minecraft:red_sandstone_slab".to_owned(),
            "minecraft:smooth_red_sandstone" => "minecraft:smooth_red_sandstone_slab".to_owned(),
            "minecraft:polished_deepslate" => "minecraft:polished_deepslate_slab".to_owned(),
            "minecraft:cobbled_deepslate" => "minecraft:cobbled_deepslate_slab".to_owned(),
            "minecraft:polished_andesite" => "minecraft:polished_andesite_slab".to_owned(),
            "minecraft:andesite" => "minecraft:andesite_slab".to_owned(),
            "minecraft:polished_diorite" => "minecraft:polished_diorite_slab".to_owned(),
            "minecraft:diorite" => "minecraft:diorite_slab".to_owned(),
            "minecraft:polished_granite" => "minecraft:polished_granite_slab".to_owned(),
            "minecraft:granite" => "minecraft:granite_slab".to_owned(),
            "minecraft:polished_tuff" => "minecraft:polished_tuff_slab".to_owned(),
            "minecraft:tuff" => "minecraft:tuff_slab".to_owned(),
            "minecraft:cobblestone" => "minecraft:cobblestone_slab".to_owned(),
            "minecraft:mossy_cobblestone" => "minecraft:mossy_cobblestone_slab".to_owned(),
            "minecraft:blackstone" => "minecraft:blackstone_slab".to_owned(),
            "minecraft:polished_blackstone" => "minecraft:polished_blackstone_slab".to_owned(),
            "minecraft:nether_bricks" => "minecraft:nether_brick_slab".to_owned(),
            "minecraft:red_nether_bricks" => "minecraft:red_nether_brick_slab".to_owned(),
            "minecraft:end_stone_bricks" => "minecraft:end_stone_brick_slab".to_owned(),
            "minecraft:bamboo_mosaic" => "minecraft:bamboo_mosaic_slab".to_owned(),
            "minecraft:cut_copper" | "minecraft:copper_block" => "minecraft:cut_copper_slab".to_owned(),
            "minecraft:exposed_cut_copper" | "minecraft:exposed_copper" => {
                "minecraft:exposed_cut_copper_slab".to_owned()
            }
            "minecraft:weathered_cut_copper" | "minecraft:weathered_copper" => {
                "minecraft:weathered_cut_copper_slab".to_owned()
            }
            "minecraft:oxidized_cut_copper" | "minecraft:oxidized_copper" => {
                "minecraft:oxidized_cut_copper_slab".to_owned()
            }
            _ => "minecraft:smooth_stone_slab".to_owned(),
        }
    }

    /// Resolves matching stair ID for a block ID if a verified stair variant exists.
    pub fn resolve_stair_id(block_id: &str) -> Option<String> {
        if block_id.ends_with("_stairs") {
            return Some(block_id.to_owned());
        }
        if let Some(stem) = block_id.strip_suffix("_planks") {
            return Some(format!("{stem}_stairs"));
        }
        Self::ROOF_CHOICES
            .iter()
            .find(|&&(b, _, _)| b == block_id)
            .map(|&(_, st, _)| st.to_owned())
    }

    /// Picks a stair block from `slot` (`4` for roof, `23` for stairs) at `(u, y, v)`.
    pub fn pick_stair(&self, slot: usize, seed: u64, u: i32, y: i32, v: i32) -> String {
        let raw = self.pick(slot, seed, u, y, v);
        if !self.has_extra(slot) && slot == 4 {
            return self.roof_stairs.clone();
        }
        if !self.has_extra(slot) && slot == 23 {
            return self.stair_block.clone();
        }
        Self::resolve_stair_id(&raw).unwrap_or_else(|| {
            if slot == 23 {
                self.stair_block.clone()
            } else {
                self.roof_stairs.clone()
            }
        })
    }

    /// Picks a slab block from `slot` (`4` for roof, `18` for ceiling) at `(u, y, v)`.
    pub fn pick_slab(&self, slot: usize, seed: u64, u: i32, y: i32, v: i32) -> String {
        if !self.has_extra(slot) {
            return match slot {
                4 => self.roof_slab.clone(),
                18 => self.ceiling.clone(),
                _ => Self::resolve_slab_id(self.slot_primary(slot)),
            };
        }
        let raw = self.pick(slot, seed, u, y, v);
        Self::resolve_slab_id(&raw)
    }

    /// Returns localized display name for any built-in/custom palette name across EN and RU.
    pub fn display_name<'a>(&'a self, lang: UiLang) -> &'a str {
        translate_palette_name(&self.name, lang)
    }

    pub fn preset_for_style(style: BuildStyle) -> Self {
        match style {
            BuildStyle::Auto | BuildStyle::Cottage => Self::preset(0),
            BuildStyle::Medieval => Self::preset(1),
            BuildStyle::Japanese => Self::preset(2),
            BuildStyle::Gothic => Self::preset(3),
            BuildStyle::Modern => Self::preset(4),
            BuildStyle::Desert => Self::preset(5),
            BuildStyle::Nordic => Self::preset(6),
            BuildStyle::Steampunk => Self::preset(7),
            BuildStyle::Classical => Self::preset(16),
        }
    }

    pub fn preset(index: usize) -> Self {
        match index % Self::PRESET_COUNT {
            0 => Self::build_full(
                "Уютный коттедж (дуб и сланец)",
                "minecraft:cobblestone",
                "minecraft:oak_planks",
                "minecraft:oak_planks",
                "minecraft:stripped_oak_log",
                "minecraft:stripped_spruce_log",
                "minecraft:spruce_log",
                "minecraft:spruce_log",
                "minecraft:oak_log",
                "minecraft:cobblestone_wall",
                "minecraft:spruce_planks",
                "minecraft:spruce_stairs",
                "minecraft:spruce_slab",
                "minecraft:dark_oak_planks",
                "minecraft:oak_planks",
                "minecraft:spruce_planks",
                "minecraft:oak_slab",
                "minecraft:glass_pane",
                "minecraft:light_blue_stained_glass_pane",
                "minecraft:oak_door",
                "minecraft:spruce_door",
                "minecraft:oak_fence",
                "minecraft:oak_fence_gate",
                "minecraft:spruce_trapdoor",
                "minecraft:spruce_stairs",
                "minecraft:bricks",
                "minecraft:flowering_azalea_leaves",
                "minecraft:azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:moss_carpet",
                "minecraft:gravel",
                "minecraft:red_wool",
            ),
            1 => Self::build_full(
                "Средневековый фахверк (Тюдор)",
                "minecraft:stone_bricks",
                "minecraft:white_terracotta",
                "minecraft:white_terracotta",
                "minecraft:calcite",
                "minecraft:stone_bricks",
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_log",
                "minecraft:stone_brick_wall",
                "minecraft:deepslate_tiles",
                "minecraft:deepslate_tile_stairs",
                "minecraft:deepslate_tile_slab",
                "minecraft:stone_bricks",
                "minecraft:spruce_planks",
                "minecraft:spruce_planks",
                "minecraft:spruce_slab",
                "minecraft:glass_pane",
                "minecraft:yellow_stained_glass_pane",
                "minecraft:dark_oak_door",
                "minecraft:dark_oak_door",
                "minecraft:dark_oak_fence",
                "minecraft:dark_oak_fence_gate",
                "minecraft:dark_oak_trapdoor",
                "minecraft:spruce_stairs",
                "minecraft:bricks",
                "minecraft:oak_leaves",
                "minecraft:azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:red_carpet",
                "minecraft:cobblestone",
                "minecraft:red_wool",
            ),
            2 => Self::build_full(
                "Японский чайный дом / пагода",
                "minecraft:stone_bricks",
                "minecraft:calcite",
                "minecraft:calcite",
                "minecraft:diorite",
                "minecraft:stripped_dark_oak_log",
                "minecraft:stripped_dark_oak_log",
                "minecraft:dark_oak_log",
                "minecraft:stripped_dark_oak_log",
                "minecraft:polished_deepslate_wall",
                "minecraft:deepslate_tiles",
                "minecraft:deepslate_tile_stairs",
                "minecraft:deepslate_tile_slab",
                "minecraft:stone_bricks",
                "minecraft:spruce_planks",
                "minecraft:spruce_planks",
                "minecraft:spruce_slab",
                "minecraft:white_stained_glass_pane",
                "minecraft:pink_stained_glass_pane",
                "minecraft:cherry_door",
                "minecraft:cherry_door",
                "minecraft:bamboo_fence",
                "minecraft:bamboo_fence_gate",
                "minecraft:spruce_trapdoor",
                "minecraft:spruce_stairs",
                "minecraft:polished_basalt",
                "minecraft:cherry_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:lantern",
                "minecraft:soul_lantern",
                "minecraft:white_carpet",
                "minecraft:gravel",
                "minecraft:orange_wool",
            ),
            3 => Self::build_full(
                "Готика (сланец и тёмный дуб)",
                "minecraft:deepslate_bricks",
                "minecraft:deepslate_tiles",
                "minecraft:deepslate_bricks",
                "minecraft:cracked_deepslate_bricks",
                "minecraft:chiseled_deepslate",
                "minecraft:polished_deepslate",
                "minecraft:polished_blackstone",
                "minecraft:polished_deepslate",
                "minecraft:deepslate_brick_wall",
                "minecraft:polished_blackstone_bricks",
                "minecraft:polished_blackstone_brick_stairs",
                "minecraft:polished_blackstone_brick_slab",
                "minecraft:polished_deepslate",
                "minecraft:dark_oak_planks",
                "minecraft:dark_oak_planks",
                "minecraft:dark_oak_slab",
                "minecraft:purple_stained_glass_pane",
                "minecraft:magenta_stained_glass_pane",
                "minecraft:dark_oak_door",
                "minecraft:dark_oak_door",
                "minecraft:nether_brick_fence",
                "minecraft:dark_oak_fence_gate",
                "minecraft:dark_oak_trapdoor",
                "minecraft:polished_blackstone_brick_stairs",
                "minecraft:polished_deepslate",
                "minecraft:spruce_leaves",
                "minecraft:pale_oak_leaves",
                "minecraft:soul_lantern",
                "minecraft:soul_torch",
                "minecraft:purple_carpet",
                "minecraft:cobbled_deepslate",
                "minecraft:purple_wool",
            ),
            4 => Self::build_full(
                "Модерн (белый бетон и кварц)",
                "minecraft:polished_andesite",
                "minecraft:white_concrete",
                "minecraft:white_concrete",
                "minecraft:light_gray_concrete",
                "minecraft:gray_concrete",
                "minecraft:gray_concrete",
                "minecraft:black_concrete",
                "minecraft:quartz_pillar",
                "minecraft:polished_deepslate_wall",
                "minecraft:smooth_quartz",
                "minecraft:smooth_quartz_stairs",
                "minecraft:smooth_quartz_slab",
                "minecraft:gray_concrete",
                "minecraft:pale_oak_planks",
                "minecraft:smooth_quartz",
                "minecraft:smooth_quartz_slab",
                "minecraft:light_blue_stained_glass_pane",
                "minecraft:glass_pane",
                "minecraft:pale_oak_door",
                "minecraft:pale_oak_door",
                "minecraft:iron_bars",
                "minecraft:pale_oak_fence_gate",
                "minecraft:iron_trapdoor",
                "minecraft:smooth_quartz_stairs",
                "minecraft:smooth_stone",
                "minecraft:azalea_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:sea_lantern",
                "minecraft:end_rod",
                "minecraft:white_carpet",
                "minecraft:polished_andesite",
                "minecraft:light_blue_wool",
            ),
            5 => Self::build_full(
                "Восток и пустыня (песчаник)",
                "minecraft:cut_sandstone",
                "minecraft:sandstone",
                "minecraft:smooth_sandstone",
                "minecraft:smooth_sandstone",
                "minecraft:chiseled_sandstone",
                "minecraft:stripped_acacia_log",
                "minecraft:acacia_log",
                "minecraft:cut_sandstone",
                "minecraft:sandstone_wall",
                "minecraft:terracotta",
                "minecraft:red_sandstone_stairs",
                "minecraft:red_sandstone_slab",
                "minecraft:smooth_red_sandstone",
                "minecraft:acacia_planks",
                "minecraft:smooth_sandstone",
                "minecraft:sandstone_slab",
                "minecraft:yellow_stained_glass_pane",
                "minecraft:orange_stained_glass_pane",
                "minecraft:acacia_door",
                "minecraft:acacia_door",
                "minecraft:acacia_fence",
                "minecraft:acacia_fence_gate",
                "minecraft:acacia_trapdoor",
                "minecraft:smooth_sandstone_stairs",
                "minecraft:mud_bricks",
                "minecraft:flowering_azalea_leaves",
                "minecraft:jungle_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:orange_carpet",
                "minecraft:smooth_sandstone",
                "minecraft:yellow_wool",
            ),
            6 => Self::build_full(
                "Скандинавский сруб (ель и туф)",
                "minecraft:tuff_bricks",
                "minecraft:spruce_planks",
                "minecraft:spruce_planks",
                "minecraft:stripped_spruce_log",
                "minecraft:tuff_bricks",
                "minecraft:spruce_log",
                "minecraft:spruce_log",
                "minecraft:spruce_log",
                "minecraft:tuff_brick_wall",
                "minecraft:cobbled_deepslate",
                "minecraft:cobbled_deepslate_stairs",
                "minecraft:cobbled_deepslate_slab",
                "minecraft:spruce_log",
                "minecraft:oak_planks",
                "minecraft:spruce_planks",
                "minecraft:spruce_slab",
                "minecraft:glass_pane",
                "minecraft:light_gray_stained_glass_pane",
                "minecraft:spruce_door",
                "minecraft:spruce_door",
                "minecraft:spruce_fence",
                "minecraft:spruce_fence_gate",
                "minecraft:spruce_trapdoor",
                "minecraft:spruce_stairs",
                "minecraft:tuff_bricks",
                "minecraft:spruce_leaves",
                "minecraft:azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:moss_carpet",
                "minecraft:cobbled_deepslate",
                "minecraft:blue_wool",
            ),
            7 => Self::build_full(
                "Стимпанк (медь и туфовый кирпич)",
                "minecraft:tuff_bricks",
                "minecraft:bricks",
                "minecraft:mud_bricks",
                "minecraft:mud_bricks",
                "minecraft:chiseled_copper",
                "minecraft:dark_oak_log",
                "minecraft:dark_oak_log",
                "minecraft:polished_tuff",
                "minecraft:brick_wall",
                "minecraft:oxidized_cut_copper",
                "minecraft:oxidized_cut_copper_stairs",
                "minecraft:oxidized_cut_copper_slab",
                "minecraft:cut_copper",
                "minecraft:dark_oak_planks",
                "minecraft:dark_oak_planks",
                "minecraft:dark_oak_slab",
                "minecraft:copper_grate",
                "minecraft:orange_stained_glass_pane",
                "minecraft:copper_door",
                "minecraft:exposed_copper_door",
                "minecraft:nether_brick_fence",
                "minecraft:dark_oak_fence_gate",
                "minecraft:copper_trapdoor",
                "minecraft:tuff_brick_stairs",
                "minecraft:polished_tuff",
                "minecraft:azalea_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:copper_lantern",
                "minecraft:copper_torch",
                "minecraft:brown_carpet",
                "minecraft:polished_tuff",
                "minecraft:orange_wool",
            ),
            8 => Self::build_full(
                "Берёза, кирпич и медь",
                "minecraft:stone_bricks",
                "minecraft:birch_planks",
                "minecraft:birch_planks",
                "minecraft:white_terracotta",
                "minecraft:bricks",
                "minecraft:oak_log",
                "minecraft:oak_log",
                "minecraft:birch_log",
                "minecraft:mossy_stone_brick_wall",
                "minecraft:cut_copper",
                "minecraft:cut_copper_stairs",
                "minecraft:cut_copper_slab",
                "minecraft:stone_bricks",
                "minecraft:birch_planks",
                "minecraft:birch_planks",
                "minecraft:birch_slab",
                "minecraft:glass_pane",
                "minecraft:green_stained_glass_pane",
                "minecraft:birch_door",
                "minecraft:birch_door",
                "minecraft:birch_fence",
                "minecraft:birch_fence_gate",
                "minecraft:birch_trapdoor",
                "minecraft:oak_stairs",
                "minecraft:bricks",
                "minecraft:birch_leaves",
                "minecraft:oak_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:green_carpet",
                "minecraft:cobblestone",
                "minecraft:green_wool",
            ),
            9 => Self::build_full(
                "Вишнёвый сад (сакура и кальцит)",
                "minecraft:stone_bricks",
                "minecraft:cherry_planks",
                "minecraft:calcite",
                "minecraft:calcite",
                "minecraft:stripped_cherry_log",
                "minecraft:cherry_log",
                "minecraft:cherry_log",
                "minecraft:cherry_log",
                "minecraft:diorite_wall",
                "minecraft:deepslate_tiles",
                "minecraft:deepslate_tile_stairs",
                "minecraft:deepslate_tile_slab",
                "minecraft:cherry_planks",
                "minecraft:cherry_planks",
                "minecraft:cherry_planks",
                "minecraft:cherry_slab",
                "minecraft:pink_stained_glass_pane",
                "minecraft:white_stained_glass_pane",
                "minecraft:cherry_door",
                "minecraft:cherry_door",
                "minecraft:cherry_fence",
                "minecraft:cherry_fence_gate",
                "minecraft:cherry_trapdoor",
                "minecraft:cherry_stairs",
                "minecraft:stone_bricks",
                "minecraft:cherry_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:pink_carpet",
                "minecraft:polished_diorite",
                "minecraft:pink_wool",
            ),
            10 => Self::build_full(
                "Океан и призмарин",
                "minecraft:dark_prismarine",
                "minecraft:prismarine_bricks",
                "minecraft:calcite",
                "minecraft:calcite",
                "minecraft:dark_prismarine",
                "minecraft:warped_stem",
                "minecraft:warped_stem",
                "minecraft:dark_prismarine",
                "minecraft:prismarine_wall",
                "minecraft:dark_prismarine",
                "minecraft:dark_prismarine_stairs",
                "minecraft:dark_prismarine_slab",
                "minecraft:prismarine_bricks",
                "minecraft:warped_planks",
                "minecraft:warped_planks",
                "minecraft:warped_slab",
                "minecraft:cyan_stained_glass_pane",
                "minecraft:light_blue_stained_glass_pane",
                "minecraft:warped_door",
                "minecraft:warped_door",
                "minecraft:warped_fence",
                "minecraft:warped_fence_gate",
                "minecraft:warped_trapdoor",
                "minecraft:prismarine_brick_stairs",
                "minecraft:polished_basalt",
                "minecraft:azalea_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:sea_lantern",
                "minecraft:end_rod",
                "minecraft:cyan_carpet",
                "minecraft:prismarine_bricks",
                "minecraft:cyan_wool",
            ),
            11 => Self::build_full(
                "Тропическая вилла (бамбук и джунгли)",
                "minecraft:mossy_cobblestone",
                "minecraft:bamboo_planks",
                "minecraft:bamboo_planks",
                "minecraft:bamboo_mosaic",
                "minecraft:stripped_bamboo_block",
                "minecraft:jungle_log",
                "minecraft:jungle_log",
                "minecraft:bamboo_block",
                "minecraft:mossy_cobblestone_wall",
                "minecraft:jungle_planks",
                "minecraft:jungle_stairs",
                "minecraft:jungle_slab",
                "minecraft:bamboo_mosaic",
                "minecraft:bamboo_mosaic",
                "minecraft:jungle_planks",
                "minecraft:bamboo_slab",
                "minecraft:yellow_stained_glass_pane",
                "minecraft:lime_stained_glass_pane",
                "minecraft:bamboo_door",
                "minecraft:jungle_door",
                "minecraft:bamboo_fence",
                "minecraft:bamboo_fence_gate",
                "minecraft:bamboo_trapdoor",
                "minecraft:bamboo_mosaic_stairs",
                "minecraft:mud_bricks",
                "minecraft:jungle_leaves",
                "minecraft:flowering_azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:lime_carpet",
                "minecraft:mossy_cobblestone",
                "minecraft:lime_wool",
            ),
            12 => Self::build_full(
                "Магическая обсерватория (аметист)",
                "minecraft:end_stone_bricks",
                "minecraft:calcite",
                "minecraft:end_stone_bricks",
                "minecraft:amethyst_block",
                "minecraft:purpur_pillar",
                "minecraft:purpur_pillar",
                "minecraft:purpur_pillar",
                "minecraft:purpur_pillar",
                "minecraft:end_stone_brick_wall",
                "minecraft:purpur_block",
                "minecraft:purpur_stairs",
                "minecraft:purpur_slab",
                "minecraft:amethyst_block",
                "minecraft:dark_oak_planks",
                "minecraft:purpur_block",
                "minecraft:purpur_slab",
                "minecraft:purple_stained_glass_pane",
                "minecraft:magenta_stained_glass_pane",
                "minecraft:pale_oak_door",
                "minecraft:pale_oak_door",
                "minecraft:pale_oak_fence",
                "minecraft:pale_oak_fence_gate",
                "minecraft:cherry_trapdoor",
                "minecraft:purpur_stairs",
                "minecraft:polished_deepslate",
                "minecraft:flowering_azalea_leaves",
                "minecraft:cherry_leaves",
                "minecraft:pearlescent_froglight",
                "minecraft:end_rod",
                "minecraft:magenta_carpet",
                "minecraft:end_stone_bricks",
                "minecraft:magenta_wool",
            ),
            13 => Self::build_full(
                "Незер-цитадель (багровый камень)",
                "minecraft:nether_bricks",
                "minecraft:red_nether_bricks",
                "minecraft:nether_bricks",
                "minecraft:polished_basalt",
                "minecraft:chiseled_nether_bricks",
                "minecraft:crimson_stem",
                "minecraft:crimson_stem",
                "minecraft:polished_basalt",
                "minecraft:nether_brick_wall",
                "minecraft:nether_bricks",
                "minecraft:nether_brick_stairs",
                "minecraft:nether_brick_slab",
                "minecraft:red_nether_bricks",
                "minecraft:crimson_planks",
                "minecraft:crimson_planks",
                "minecraft:crimson_slab",
                "minecraft:red_stained_glass_pane",
                "minecraft:orange_stained_glass_pane",
                "minecraft:crimson_door",
                "minecraft:crimson_door",
                "minecraft:nether_brick_fence",
                "minecraft:crimson_fence_gate",
                "minecraft:crimson_trapdoor",
                "minecraft:red_nether_brick_stairs",
                "minecraft:blackstone",
                "minecraft:flowering_azalea_leaves",
                "minecraft:azalea_leaves",
                "minecraft:shroomlight",
                "minecraft:soul_torch",
                "minecraft:red_carpet",
                "minecraft:blackstone",
                "minecraft:red_wool",
            ),
            14 => Self::build_full(
                "Бледный особняк (туф и белый дуб)",
                "minecraft:tuff_bricks",
                "minecraft:pale_oak_planks",
                "minecraft:pale_oak_planks",
                "minecraft:calcite",
                "minecraft:chiseled_tuff_bricks",
                "minecraft:pale_oak_log",
                "minecraft:pale_oak_log",
                "minecraft:pale_oak_log",
                "minecraft:tuff_brick_wall",
                "minecraft:deepslate_bricks",
                "minecraft:deepslate_brick_stairs",
                "minecraft:deepslate_brick_slab",
                "minecraft:tuff_bricks",
                "minecraft:spruce_planks",
                "minecraft:pale_oak_planks",
                "minecraft:pale_oak_slab",
                "minecraft:gray_stained_glass_pane",
                "minecraft:white_stained_glass_pane",
                "minecraft:pale_oak_door",
                "minecraft:pale_oak_door",
                "minecraft:pale_oak_fence",
                "minecraft:pale_oak_fence_gate",
                "minecraft:pale_oak_trapdoor",
                "minecraft:tuff_brick_stairs",
                "minecraft:tuff_bricks",
                "minecraft:pale_oak_leaves",
                "minecraft:spruce_leaves",
                "minecraft:soul_lantern",
                "minecraft:soul_torch",
                "minecraft:pale_moss_carpet",
                "minecraft:polished_tuff",
                "minecraft:light_gray_wool",
            ),
            15 => Self::build_full(
                "Зимнее шале (снег, лёд и ель)",
                "minecraft:stone_bricks",
                "minecraft:snow_block",
                "minecraft:snow_block",
                "minecraft:packed_ice",
                "minecraft:stripped_spruce_log",
                "minecraft:spruce_log",
                "minecraft:spruce_log",
                "minecraft:spruce_log",
                "minecraft:stone_brick_wall",
                "minecraft:spruce_planks",
                "minecraft:spruce_stairs",
                "minecraft:spruce_slab",
                "minecraft:snow_block",
                "minecraft:spruce_planks",
                "minecraft:spruce_planks",
                "minecraft:spruce_slab",
                "minecraft:light_blue_stained_glass_pane",
                "minecraft:white_stained_glass_pane",
                "minecraft:spruce_door",
                "minecraft:spruce_door",
                "minecraft:spruce_fence",
                "minecraft:spruce_fence_gate",
                "minecraft:spruce_trapdoor",
                "minecraft:spruce_stairs",
                "minecraft:cobblestone",
                "minecraft:spruce_leaves",
                "minecraft:pale_oak_leaves",
                "minecraft:lantern",
                "minecraft:soul_torch",
                "minecraft:light_blue_carpet",
                "minecraft:packed_ice",
                "minecraft:white_wool",
            ),
            _ => Self::build_full(
                "Античность (травертин и мрамор)",
                "minecraft:cut_sandstone",
                "minecraft:smooth_sandstone",
                "minecraft:quartz_bricks",
                "minecraft:quartz_bricks",
                "minecraft:chiseled_quartz_block",
                "minecraft:quartz_pillar",
                "minecraft:smooth_quartz",
                "minecraft:quartz_pillar",
                "minecraft:sandstone_wall",
                "minecraft:terracotta",
                "minecraft:red_sandstone_stairs",
                "minecraft:smooth_quartz_slab",
                "minecraft:smooth_quartz",
                "minecraft:smooth_sandstone",
                "minecraft:smooth_quartz",
                "minecraft:smooth_quartz_slab",
                "minecraft:iron_bars",
                "minecraft:yellow_stained_glass_pane",
                "minecraft:oak_door",
                "minecraft:oak_door",
                "minecraft:sandstone_wall",
                "minecraft:oak_fence_gate",
                "minecraft:oak_trapdoor",
                "minecraft:smooth_sandstone_stairs",
                "minecraft:bricks",
                "minecraft:flowering_azalea_leaves",
                "minecraft:azalea_leaves",
                "minecraft:lantern",
                "minecraft:torch",
                "minecraft:red_carpet",
                "minecraft:smooth_sandstone",
                "minecraft:red_wool",
            ),
        }
    }

    pub const FOUNDATION_CHOICES: &'static [&'static str] = &[
        "minecraft:cobblestone",
        "minecraft:stone_bricks",
        "minecraft:mossy_cobblestone",
        "minecraft:mossy_stone_bricks",
        "minecraft:cracked_stone_bricks",
        "minecraft:chiseled_stone_bricks",
        "minecraft:tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:chiseled_tuff_bricks",
        "minecraft:deepslate_bricks",
        "minecraft:deepslate_tiles",
        "minecraft:cobbled_deepslate",
        "minecraft:polished_deepslate",
        "minecraft:polished_blackstone_bricks",
        "minecraft:polished_blackstone",
        "minecraft:blackstone",
        "minecraft:gilded_blackstone",
        "minecraft:polished_andesite",
        "minecraft:andesite",
        "minecraft:polished_granite",
        "minecraft:granite",
        "minecraft:polished_diorite",
        "minecraft:diorite",
        "minecraft:bricks",
        "minecraft:mud_bricks",
        "minecraft:packed_mud",
        "minecraft:resin_bricks",
        "minecraft:cut_sandstone",
        "minecraft:smooth_sandstone",
        "minecraft:cut_red_sandstone",
        "minecraft:smooth_red_sandstone",
        "minecraft:nether_bricks",
        "minecraft:red_nether_bricks",
        "minecraft:dark_prismarine",
        "minecraft:prismarine_bricks",
        "minecraft:end_stone_bricks",
        "minecraft:quartz_bricks",
        "minecraft:smooth_quartz",
        "minecraft:polished_basalt",
        "minecraft:smooth_stone",
        "minecraft:cut_copper",
        "minecraft:oxidized_cut_copper",
        "minecraft:obsidian",
    ];

    pub const WALL_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_planks",
        "minecraft:spruce_planks",
        "minecraft:birch_planks",
        "minecraft:jungle_planks",
        "minecraft:acacia_planks",
        "minecraft:dark_oak_planks",
        "minecraft:mangrove_planks",
        "minecraft:cherry_planks",
        "minecraft:pale_oak_planks",
        "minecraft:bamboo_planks",
        "minecraft:bamboo_mosaic",
        "minecraft:crimson_planks",
        "minecraft:warped_planks",
        "minecraft:white_terracotta",
        "minecraft:terracotta",
        "minecraft:light_gray_terracotta",
        "minecraft:gray_terracotta",
        "minecraft:black_terracotta",
        "minecraft:brown_terracotta",
        "minecraft:red_terracotta",
        "minecraft:orange_terracotta",
        "minecraft:yellow_terracotta",
        "minecraft:lime_terracotta",
        "minecraft:green_terracotta",
        "minecraft:cyan_terracotta",
        "minecraft:light_blue_terracotta",
        "minecraft:blue_terracotta",
        "minecraft:purple_terracotta",
        "minecraft:magenta_terracotta",
        "minecraft:pink_terracotta",
        "minecraft:white_concrete",
        "minecraft:light_gray_concrete",
        "minecraft:gray_concrete",
        "minecraft:black_concrete",
        "minecraft:brown_concrete",
        "minecraft:red_concrete",
        "minecraft:orange_concrete",
        "minecraft:yellow_concrete",
        "minecraft:lime_concrete",
        "minecraft:green_concrete",
        "minecraft:cyan_concrete",
        "minecraft:light_blue_concrete",
        "minecraft:blue_concrete",
        "minecraft:purple_concrete",
        "minecraft:magenta_concrete",
        "minecraft:pink_concrete",
        "minecraft:calcite",
        "minecraft:diorite",
        "minecraft:stone_bricks",
        "minecraft:mossy_stone_bricks",
        "minecraft:tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:bricks",
        "minecraft:mud_bricks",
        "minecraft:packed_mud",
        "minecraft:resin_bricks",
        "minecraft:sandstone",
        "minecraft:smooth_sandstone",
        "minecraft:red_sandstone",
        "minecraft:smooth_red_sandstone",
        "minecraft:smooth_quartz",
        "minecraft:quartz_bricks",
        "minecraft:deepslate_tiles",
        "minecraft:deepslate_bricks",
        "minecraft:polished_blackstone_bricks",
        "minecraft:prismarine_bricks",
        "minecraft:dark_prismarine",
        "minecraft:red_nether_bricks",
        "minecraft:nether_bricks",
        "minecraft:end_stone_bricks",
        "minecraft:purpur_block",
        "minecraft:snow_block",
        "minecraft:glass",
        "minecraft:moss_block",
        "minecraft:pale_moss_block",
    ];

    pub const ACCENT_CHOICES: &'static [&'static str] = &[
        "minecraft:stripped_oak_log",
        "minecraft:stripped_spruce_log",
        "minecraft:stripped_birch_log",
        "minecraft:stripped_jungle_log",
        "minecraft:stripped_acacia_log",
        "minecraft:stripped_dark_oak_log",
        "minecraft:stripped_mangrove_log",
        "minecraft:stripped_cherry_log",
        "minecraft:stripped_pale_oak_log",
        "minecraft:stripped_bamboo_block",
        "minecraft:stripped_crimson_stem",
        "minecraft:stripped_warped_stem",
        "minecraft:calcite",
        "minecraft:diorite",
        "minecraft:cracked_stone_bricks",
        "minecraft:mossy_stone_bricks",
        "minecraft:chiseled_stone_bricks",
        "minecraft:chiseled_tuff_bricks",
        "minecraft:chiseled_tuff",
        "minecraft:mud_bricks",
        "minecraft:resin_bricks",
        "minecraft:chiseled_resin_bricks",
        "minecraft:light_gray_concrete",
        "minecraft:white_terracotta",
        "minecraft:cracked_deepslate_bricks",
        "minecraft:cracked_deepslate_tiles",
        "minecraft:chiseled_deepslate",
        "minecraft:chiseled_polished_blackstone",
        "minecraft:cracked_nether_bricks",
        "minecraft:chiseled_nether_bricks",
        "minecraft:gilded_blackstone",
        "minecraft:smooth_sandstone",
        "minecraft:chiseled_sandstone",
        "minecraft:quartz_bricks",
        "minecraft:chiseled_quartz_block",
        "minecraft:bamboo_mosaic",
        "minecraft:amethyst_block",
        "minecraft:copper_block",
        "minecraft:exposed_copper",
        "minecraft:weathered_copper",
        "minecraft:oxidized_copper",
        "minecraft:chiseled_copper",
        "minecraft:exposed_chiseled_copper",
        "minecraft:oxidized_chiseled_copper",
        "minecraft:polished_basalt",
        "minecraft:packed_ice",
        "minecraft:blue_ice",
        "minecraft:dark_prismarine",
        "minecraft:bookshelf",
        "minecraft:hay_block",
        "minecraft:bone_block",
        "minecraft:honeycomb_block",
        "minecraft:white_glazed_terracotta",
        "minecraft:cyan_glazed_terracotta",
        "minecraft:blue_glazed_terracotta",
        "minecraft:red_glazed_terracotta",
        "minecraft:yellow_glazed_terracotta",
    ];

    pub const FRAME_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_log",
        "minecraft:spruce_log",
        "minecraft:dark_oak_log",
        "minecraft:stripped_dark_oak_log",
        "minecraft:birch_log",
        "minecraft:jungle_log",
        "minecraft:acacia_log",
        "minecraft:mangrove_log",
        "minecraft:cherry_log",
        "minecraft:pale_oak_log",
        "minecraft:bamboo_block",
        "minecraft:crimson_stem",
        "minecraft:warped_stem",
        "minecraft:oak_wood",
        "minecraft:spruce_wood",
        "minecraft:birch_wood",
        "minecraft:dark_oak_wood",
        "minecraft:pale_oak_wood",
        "minecraft:stripped_oak_wood",
        "minecraft:stripped_spruce_wood",
        "minecraft:stripped_dark_oak_wood",
        "minecraft:stripped_pale_oak_wood",
        "minecraft:gray_concrete",
        "minecraft:black_concrete",
        "minecraft:white_concrete",
        "minecraft:polished_deepslate",
        "minecraft:deepslate_bricks",
        "minecraft:polished_blackstone",
        "minecraft:polished_tuff",
        "minecraft:tuff_bricks",
        "minecraft:stone_bricks",
        "minecraft:bricks",
        "minecraft:mud_bricks",
        "minecraft:resin_bricks",
        "minecraft:quartz_pillar",
        "minecraft:smooth_quartz",
        "minecraft:purpur_pillar",
        "minecraft:polished_basalt",
        "minecraft:dark_prismarine",
        "minecraft:cut_sandstone",
    ];

    pub const ROOF_CHOICES: &'static [(&'static str, &'static str, &'static str)] = &[
        ("minecraft:spruce_planks", "minecraft:spruce_stairs", "minecraft:spruce_slab"),
        ("minecraft:oak_planks", "minecraft:oak_stairs", "minecraft:oak_slab"),
        ("minecraft:birch_planks", "minecraft:birch_stairs", "minecraft:birch_slab"),
        ("minecraft:jungle_planks", "minecraft:jungle_stairs", "minecraft:jungle_slab"),
        ("minecraft:acacia_planks", "minecraft:acacia_stairs", "minecraft:acacia_slab"),
        ("minecraft:dark_oak_planks", "minecraft:dark_oak_stairs", "minecraft:dark_oak_slab"),
        ("minecraft:mangrove_planks", "minecraft:mangrove_stairs", "minecraft:mangrove_slab"),
        ("minecraft:cherry_planks", "minecraft:cherry_stairs", "minecraft:cherry_slab"),
        ("minecraft:pale_oak_planks", "minecraft:pale_oak_stairs", "minecraft:pale_oak_slab"),
        ("minecraft:bamboo_planks", "minecraft:bamboo_stairs", "minecraft:bamboo_slab"),
        ("minecraft:bamboo_mosaic", "minecraft:bamboo_mosaic_stairs", "minecraft:bamboo_mosaic_slab"),
        ("minecraft:crimson_planks", "minecraft:crimson_stairs", "minecraft:crimson_slab"),
        ("minecraft:warped_planks", "minecraft:warped_stairs", "minecraft:warped_slab"),
        ("minecraft:deepslate_tiles", "minecraft:deepslate_tile_stairs", "minecraft:deepslate_tile_slab"),
        ("minecraft:deepslate_bricks", "minecraft:deepslate_brick_stairs", "minecraft:deepslate_brick_slab"),
        ("minecraft:cobbled_deepslate", "minecraft:cobbled_deepslate_stairs", "minecraft:cobbled_deepslate_slab"),
        ("minecraft:polished_deepslate", "minecraft:polished_deepslate_stairs", "minecraft:polished_deepslate_slab"),
        ("minecraft:polished_blackstone_bricks", "minecraft:polished_blackstone_brick_stairs", "minecraft:polished_blackstone_brick_slab"),
        ("minecraft:polished_blackstone", "minecraft:polished_blackstone_stairs", "minecraft:polished_blackstone_slab"),
        ("minecraft:blackstone", "minecraft:blackstone_stairs", "minecraft:blackstone_slab"),
        ("minecraft:stone_bricks", "minecraft:stone_brick_stairs", "minecraft:stone_brick_slab"),
        ("minecraft:mossy_stone_bricks", "minecraft:mossy_stone_brick_stairs", "minecraft:mossy_stone_brick_slab"),
        ("minecraft:cobblestone", "minecraft:cobblestone_stairs", "minecraft:cobblestone_slab"),
        ("minecraft:mossy_cobblestone", "minecraft:mossy_cobblestone_stairs", "minecraft:mossy_cobblestone_slab"),
        ("minecraft:tuff_bricks", "minecraft:tuff_brick_stairs", "minecraft:tuff_brick_slab"),
        ("minecraft:polished_tuff", "minecraft:polished_tuff_stairs", "minecraft:polished_tuff_slab"),
        ("minecraft:tuff", "minecraft:tuff_stairs", "minecraft:tuff_slab"),
        ("minecraft:polished_andesite", "minecraft:polished_andesite_stairs", "minecraft:polished_andesite_slab"),
        ("minecraft:polished_diorite", "minecraft:polished_diorite_stairs", "minecraft:polished_diorite_slab"),
        ("minecraft:polished_granite", "minecraft:polished_granite_stairs", "minecraft:polished_granite_slab"),
        ("minecraft:bricks", "minecraft:brick_stairs", "minecraft:brick_slab"),
        ("minecraft:mud_bricks", "minecraft:mud_brick_stairs", "minecraft:mud_brick_slab"),
        ("minecraft:resin_bricks", "minecraft:resin_brick_stairs", "minecraft:resin_brick_slab"),
        ("minecraft:cut_copper", "minecraft:cut_copper_stairs", "minecraft:cut_copper_slab"),
        ("minecraft:exposed_cut_copper", "minecraft:exposed_cut_copper_stairs", "minecraft:exposed_cut_copper_slab"),
        ("minecraft:weathered_cut_copper", "minecraft:weathered_cut_copper_stairs", "minecraft:weathered_cut_copper_slab"),
        ("minecraft:oxidized_cut_copper", "minecraft:oxidized_cut_copper_stairs", "minecraft:oxidized_cut_copper_slab"),
        ("minecraft:smooth_quartz", "minecraft:smooth_quartz_stairs", "minecraft:smooth_quartz_slab"),
        ("minecraft:quartz_block", "minecraft:quartz_stairs", "minecraft:quartz_slab"),
        ("minecraft:sandstone", "minecraft:sandstone_stairs", "minecraft:sandstone_slab"),
        ("minecraft:smooth_sandstone", "minecraft:smooth_sandstone_stairs", "minecraft:smooth_sandstone_slab"),
        ("minecraft:terracotta", "minecraft:red_sandstone_stairs", "minecraft:red_sandstone_slab"),
        ("minecraft:smooth_red_sandstone", "minecraft:smooth_red_sandstone_stairs", "minecraft:smooth_red_sandstone_slab"),
        ("minecraft:nether_bricks", "minecraft:nether_brick_stairs", "minecraft:nether_brick_slab"),
        ("minecraft:red_nether_bricks", "minecraft:red_nether_brick_stairs", "minecraft:red_nether_brick_slab"),
        ("minecraft:dark_prismarine", "minecraft:dark_prismarine_stairs", "minecraft:dark_prismarine_slab"),
        ("minecraft:prismarine_bricks", "minecraft:prismarine_brick_stairs", "minecraft:prismarine_brick_slab"),
        ("minecraft:prismarine", "minecraft:prismarine_stairs", "minecraft:prismarine_slab"),
        ("minecraft:purpur_block", "minecraft:purpur_stairs", "minecraft:purpur_slab"),
        ("minecraft:end_stone_bricks", "minecraft:end_stone_brick_stairs", "minecraft:end_stone_brick_slab"),
    ];

    pub const FLOOR_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_planks",
        "minecraft:spruce_planks",
        "minecraft:birch_planks",
        "minecraft:jungle_planks",
        "minecraft:acacia_planks",
        "minecraft:dark_oak_planks",
        "minecraft:mangrove_planks",
        "minecraft:cherry_planks",
        "minecraft:pale_oak_planks",
        "minecraft:bamboo_planks",
        "minecraft:bamboo_mosaic",
        "minecraft:crimson_planks",
        "minecraft:warped_planks",
        "minecraft:polished_andesite",
        "minecraft:polished_diorite",
        "minecraft:polished_granite",
        "minecraft:polished_deepslate",
        "minecraft:deepslate_tiles",
        "minecraft:deepslate_bricks",
        "minecraft:tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:stone_bricks",
        "minecraft:cobblestone",
        "minecraft:mossy_cobblestone",
        "minecraft:smooth_stone",
        "minecraft:smooth_quartz",
        "minecraft:quartz_bricks",
        "minecraft:smooth_sandstone",
        "minecraft:smooth_red_sandstone",
        "minecraft:bricks",
        "minecraft:mud_bricks",
        "minecraft:packed_mud",
        "minecraft:resin_bricks",
        "minecraft:polished_blackstone_bricks",
        "minecraft:prismarine_bricks",
        "minecraft:dark_prismarine",
        "minecraft:end_stone_bricks",
        "minecraft:purpur_block",
        "minecraft:moss_block",
        "minecraft:pale_moss_block",
        "minecraft:hay_block",
        "minecraft:calcite",
        "minecraft:white_concrete",
    ];

    pub const CEILING_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_slab",
        "minecraft:spruce_slab",
        "minecraft:birch_slab",
        "minecraft:jungle_slab",
        "minecraft:acacia_slab",
        "minecraft:dark_oak_slab",
        "minecraft:mangrove_slab",
        "minecraft:cherry_slab",
        "minecraft:pale_oak_slab",
        "minecraft:bamboo_slab",
        "minecraft:bamboo_mosaic_slab",
        "minecraft:crimson_slab",
        "minecraft:warped_slab",
        "minecraft:smooth_stone_slab",
        "minecraft:stone_brick_slab",
        "minecraft:deepslate_tile_slab",
        "minecraft:deepslate_brick_slab",
        "minecraft:tuff_brick_slab",
        "minecraft:polished_tuff_slab",
        "minecraft:polished_andesite_slab",
        "minecraft:polished_diorite_slab",
        "minecraft:polished_granite_slab",
        "minecraft:smooth_quartz_slab",
        "minecraft:quartz_slab",
        "minecraft:sandstone_slab",
        "minecraft:smooth_sandstone_slab",
        "minecraft:mud_brick_slab",
        "minecraft:resin_brick_slab",
        "minecraft:brick_slab",
        "minecraft:polished_blackstone_brick_slab",
        "minecraft:dark_prismarine_slab",
        "minecraft:purpur_slab",
        "minecraft:cut_copper_slab",
        "minecraft:oxidized_cut_copper_slab",
    ];

    pub const WINDOW_CHOICES: &'static [&'static str] = &[
        "minecraft:glass_pane",
        "minecraft:white_stained_glass_pane",
        "minecraft:light_gray_stained_glass_pane",
        "minecraft:gray_stained_glass_pane",
        "minecraft:black_stained_glass_pane",
        "minecraft:brown_stained_glass_pane",
        "minecraft:red_stained_glass_pane",
        "minecraft:orange_stained_glass_pane",
        "minecraft:yellow_stained_glass_pane",
        "minecraft:lime_stained_glass_pane",
        "minecraft:green_stained_glass_pane",
        "minecraft:cyan_stained_glass_pane",
        "minecraft:light_blue_stained_glass_pane",
        "minecraft:blue_stained_glass_pane",
        "minecraft:purple_stained_glass_pane",
        "minecraft:magenta_stained_glass_pane",
        "minecraft:pink_stained_glass_pane",
        "minecraft:tinted_glass",
        "minecraft:glass",
        "minecraft:white_stained_glass",
        "minecraft:light_blue_stained_glass",
        "minecraft:cyan_stained_glass",
        "minecraft:purple_stained_glass",
        "minecraft:yellow_stained_glass",
        "minecraft:copper_grate",
        "minecraft:exposed_copper_grate",
        "minecraft:weathered_copper_grate",
        "minecraft:oxidized_copper_grate",
        "minecraft:iron_bars",
        "minecraft:copper_bars",
        "minecraft:exposed_copper_bars",
        "minecraft:weathered_copper_bars",
        "minecraft:oxidized_copper_bars",
        "minecraft:oak_fence",
        "minecraft:spruce_fence",
        "minecraft:dark_oak_fence",
        "minecraft:bamboo_fence",
    ];

    pub const DOOR_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_door",
        "minecraft:spruce_door",
        "minecraft:birch_door",
        "minecraft:jungle_door",
        "minecraft:acacia_door",
        "minecraft:dark_oak_door",
        "minecraft:mangrove_door",
        "minecraft:cherry_door",
        "minecraft:pale_oak_door",
        "minecraft:bamboo_door",
        "minecraft:crimson_door",
        "minecraft:warped_door",
        "minecraft:copper_door",
        "minecraft:exposed_copper_door",
        "minecraft:weathered_copper_door",
        "minecraft:oxidized_copper_door",
        "minecraft:iron_door",
    ];

    pub const TRAPDOOR_CHOICES: &'static [&'static str] = &[
        "minecraft:spruce_trapdoor",
        "minecraft:oak_trapdoor",
        "minecraft:birch_trapdoor",
        "minecraft:jungle_trapdoor",
        "minecraft:acacia_trapdoor",
        "minecraft:dark_oak_trapdoor",
        "minecraft:mangrove_trapdoor",
        "minecraft:cherry_trapdoor",
        "minecraft:pale_oak_trapdoor",
        "minecraft:bamboo_trapdoor",
        "minecraft:crimson_trapdoor",
        "minecraft:warped_trapdoor",
        "minecraft:copper_trapdoor",
        "minecraft:exposed_copper_trapdoor",
        "minecraft:weathered_copper_trapdoor",
        "minecraft:oxidized_copper_trapdoor",
        "minecraft:iron_trapdoor",
    ];

    pub const FENCE_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_fence",
        "minecraft:spruce_fence",
        "minecraft:birch_fence",
        "minecraft:jungle_fence",
        "minecraft:acacia_fence",
        "minecraft:dark_oak_fence",
        "minecraft:mangrove_fence",
        "minecraft:cherry_fence",
        "minecraft:pale_oak_fence",
        "minecraft:bamboo_fence",
        "minecraft:crimson_fence",
        "minecraft:warped_fence",
        "minecraft:nether_brick_fence",
        "minecraft:iron_bars",
        "minecraft:copper_bars",
        "minecraft:exposed_copper_bars",
        "minecraft:weathered_copper_bars",
        "minecraft:oxidized_copper_bars",
        "minecraft:cobblestone_wall",
        "minecraft:stone_brick_wall",
        "minecraft:deepslate_brick_wall",
        "minecraft:tuff_brick_wall",
        "minecraft:brick_wall",
        "minecraft:mud_brick_wall",
        "minecraft:resin_brick_wall",
        "minecraft:sandstone_wall",
        "minecraft:polished_blackstone_brick_wall",
        "minecraft:prismarine_wall",
    ];

    pub const FENCE_GATE_CHOICES: &'static [&'static str] = &[
        "minecraft:oak_fence_gate",
        "minecraft:spruce_fence_gate",
        "minecraft:birch_fence_gate",
        "minecraft:jungle_fence_gate",
        "minecraft:acacia_fence_gate",
        "minecraft:dark_oak_fence_gate",
        "minecraft:mangrove_fence_gate",
        "minecraft:cherry_fence_gate",
        "minecraft:pale_oak_fence_gate",
        "minecraft:bamboo_fence_gate",
        "minecraft:crimson_fence_gate",
        "minecraft:warped_fence_gate",
    ];

    pub const LEAVES_CHOICES: &'static [&'static str] = &[
        "minecraft:flowering_azalea_leaves",
        "minecraft:azalea_leaves",
        "minecraft:oak_leaves",
        "minecraft:spruce_leaves",
        "minecraft:birch_leaves",
        "minecraft:jungle_leaves",
        "minecraft:acacia_leaves",
        "minecraft:dark_oak_leaves",
        "minecraft:mangrove_leaves",
        "minecraft:cherry_leaves",
        "minecraft:pale_oak_leaves",
        "minecraft:moss_block",
        "minecraft:pale_moss_block",
        "minecraft:flowering_azalea",
        "minecraft:azalea",
    ];

    pub const LIGHT_CHOICES: &'static [&'static str] = &[
        "minecraft:lantern",
        "minecraft:soul_lantern",
        "minecraft:copper_lantern",
        "minecraft:exposed_copper_lantern",
        "minecraft:weathered_copper_lantern",
        "minecraft:oxidized_copper_lantern",
        "minecraft:torch",
        "minecraft:soul_torch",
        "minecraft:copper_torch",
        "minecraft:sea_lantern",
        "minecraft:redstone_lamp",
        "minecraft:glowstone",
        "minecraft:shroomlight",
        "minecraft:ochre_froglight",
        "minecraft:verdant_froglight",
        "minecraft:pearlescent_froglight",
        "minecraft:copper_bulb",
        "minecraft:exposed_copper_bulb",
        "minecraft:weathered_copper_bulb",
        "minecraft:oxidized_copper_bulb",
        "minecraft:end_rod",
    ];

    pub const TRIM_WALL_CHOICES: &'static [&'static str] = &[
        "minecraft:cobblestone_wall",
        "minecraft:mossy_cobblestone_wall",
        "minecraft:stone_brick_wall",
        "minecraft:mossy_stone_brick_wall",
        "minecraft:andesite_wall",
        "minecraft:diorite_wall",
        "minecraft:granite_wall",
        "minecraft:tuff_wall",
        "minecraft:polished_tuff_wall",
        "minecraft:tuff_brick_wall",
        "minecraft:cobbled_deepslate_wall",
        "minecraft:deepslate_brick_wall",
        "minecraft:polished_deepslate_wall",
        "minecraft:blackstone_wall",
        "minecraft:polished_blackstone_wall",
        "minecraft:polished_blackstone_brick_wall",
        "minecraft:brick_wall",
        "minecraft:mud_brick_wall",
        "minecraft:resin_brick_wall",
        "minecraft:sandstone_wall",
        "minecraft:red_sandstone_wall",
        "minecraft:nether_brick_wall",
        "minecraft:red_nether_brick_wall",
        "minecraft:prismarine_wall",
        "minecraft:end_stone_brick_wall",
    ];

    pub const CHIMNEY_CHOICES: &'static [&'static str] = &[
        "minecraft:bricks",
        "minecraft:cobblestone",
        "minecraft:mossy_cobblestone",
        "minecraft:stone_bricks",
        "minecraft:mossy_stone_bricks",
        "minecraft:tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:polished_deepslate",
        "minecraft:deepslate_bricks",
        "minecraft:deepslate_tiles",
        "minecraft:blackstone",
        "minecraft:polished_blackstone_bricks",
        "minecraft:polished_basalt",
        "minecraft:smooth_stone",
        "minecraft:mud_bricks",
        "minecraft:resin_bricks",
        "minecraft:nether_bricks",
        "minecraft:red_nether_bricks",
        "minecraft:cut_copper",
        "minecraft:oxidized_cut_copper",
        "minecraft:polished_andesite",
        "minecraft:terracotta",
    ];

    pub const CARPET_CHOICES: &'static [&'static str] = &[
        "minecraft:moss_carpet",
        "minecraft:pale_moss_carpet",
        "minecraft:red_carpet",
        "minecraft:white_carpet",
        "minecraft:light_gray_carpet",
        "minecraft:gray_carpet",
        "minecraft:black_carpet",
        "minecraft:brown_carpet",
        "minecraft:orange_carpet",
        "minecraft:yellow_carpet",
        "minecraft:lime_carpet",
        "minecraft:green_carpet",
        "minecraft:cyan_carpet",
        "minecraft:light_blue_carpet",
        "minecraft:blue_carpet",
        "minecraft:purple_carpet",
        "minecraft:magenta_carpet",
        "minecraft:pink_carpet",
    ];

    pub const PAVING_CHOICES: &'static [&'static str] = &[
        "minecraft:gravel",
        "minecraft:dirt_path",
        "minecraft:coarse_dirt",
        "minecraft:cobblestone",
        "minecraft:mossy_cobblestone",
        "minecraft:stone_bricks",
        "minecraft:mossy_stone_bricks",
        "minecraft:cracked_stone_bricks",
        "minecraft:andesite",
        "minecraft:polished_andesite",
        "minecraft:polished_diorite",
        "minecraft:polished_granite",
        "minecraft:tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:cobbled_deepslate",
        "minecraft:polished_deepslate",
        "minecraft:deepslate_tiles",
        "minecraft:bricks",
        "minecraft:mud_bricks",
        "minecraft:packed_mud",
        "minecraft:resin_bricks",
        "minecraft:sandstone",
        "minecraft:smooth_sandstone",
        "minecraft:red_sandstone",
        "minecraft:smooth_red_sandstone",
        "minecraft:smooth_quartz",
        "minecraft:prismarine_bricks",
        "minecraft:end_stone_bricks",
        "minecraft:polished_blackstone_bricks",
        "minecraft:oak_planks",
        "minecraft:spruce_planks",
        "minecraft:bamboo_mosaic",
    ];

    pub const AWNING_CHOICES: &'static [&'static str] = &[
        "minecraft:red_wool",
        "minecraft:white_wool",
        "minecraft:light_gray_wool",
        "minecraft:gray_wool",
        "minecraft:black_wool",
        "minecraft:brown_wool",
        "minecraft:orange_wool",
        "minecraft:yellow_wool",
        "minecraft:lime_wool",
        "minecraft:green_wool",
        "minecraft:cyan_wool",
        "minecraft:light_blue_wool",
        "minecraft:blue_wool",
        "minecraft:purple_wool",
        "minecraft:magenta_wool",
        "minecraft:pink_wool",
        "minecraft:hay_block",
        "minecraft:moss_block",
        "minecraft:terracotta",
        "minecraft:white_terracotta",
    ];

    /// Picks matching fence gate block id for courtyard gates.
    pub fn fence_gate_for_fence(&self) -> &'static str {
        if !self.fence_gate.is_empty() && self.fence_gate.ends_with("_fence_gate") {
            for &g in Self::FENCE_GATE_CHOICES {
                if g == self.fence_gate {
                    return g;
                }
            }
        }
        match self.fence.as_str() {
            "minecraft:spruce_fence" => "minecraft:spruce_fence_gate",
            "minecraft:dark_oak_fence" => "minecraft:dark_oak_fence_gate",
            "minecraft:birch_fence" => "minecraft:birch_fence_gate",
            "minecraft:jungle_fence" => "minecraft:jungle_fence_gate",
            "minecraft:mangrove_fence" => "minecraft:mangrove_fence_gate",
            "minecraft:cherry_fence" => "minecraft:cherry_fence_gate",
            "minecraft:pale_oak_fence" => "minecraft:pale_oak_fence_gate",
            "minecraft:acacia_fence" => "minecraft:acacia_fence_gate",
            "minecraft:bamboo_fence" => "minecraft:bamboo_fence_gate",
            "minecraft:crimson_fence" => "minecraft:crimson_fence_gate",
            "minecraft:warped_fence" => "minecraft:warped_fence_gate",
            "minecraft:nether_brick_fence" => "minecraft:dark_oak_fence_gate",
            _ => "minecraft:oak_fence_gate",
        }
    }
}

fn mix_hash(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as i64 as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ (y as i64 as u64).wrapping_mul(0xA5A3_564E_27F8_862D)
        ^ (z as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

fn lattice_01(seed: u64, gx: i32, gy: i32, gz: i32) -> f64 {
    ((mix_hash(seed, gx, gy, gz) >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
}

fn smooth_mix_01(seed: u64, u: i32, y: i32, v: i32) -> f64 {
    let fx = f64::from(u) * 0.42;
    let fy = f64::from(y) * 0.42;
    let fz = f64::from(v) * 0.42;
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let z0 = fz.floor() as i32;
    let tx = fx - f64::from(x0);
    let ty = fy - f64::from(y0);
    let tz = fz - f64::from(z0);
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let sz = tz * tz * (3.0 - 2.0 * tz);

    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let c00 = lerp(lattice_01(seed, x0, y0, z0), lattice_01(seed, x0 + 1, y0, z0), sx);
    let c10 = lerp(lattice_01(seed, x0, y0 + 1, z0), lattice_01(seed, x0 + 1, y0 + 1, z0), sx);
    let c01 = lerp(lattice_01(seed, x0, y0, z0 + 1), lattice_01(seed, x0 + 1, y0, z0 + 1), sx);
    let c11 = lerp(lattice_01(seed, x0, y0 + 1, z0 + 1), lattice_01(seed, x0 + 1, y0 + 1, z0 + 1), sx);
    let c0 = lerp(c00, c10, sy);
    let c1 = lerp(c01, c11, sy);
    lerp(c0, c1, sz)
}

/// Client-to-server build request sent over typed payload channel.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct BuildRequest {
    pub dimension: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    pub width: u16,
    pub depth: u16,
    pub height: u16,
    pub category: BuildCategory,
    pub style: BuildStyle,
    pub facing: CardinalFacing,
    pub noise_level: NoiseLevel,
    pub seed: i64,
    pub description: String,
    pub adapt_terrain: bool,
    pub air_only: bool,
    pub wear_pct: u8,
    pub overgrowth_pct: u8,
    pub biome_hint: String,
    pub palette: MaterialPalette,
}

/// Client-to-server biome lookup request for live GUI Auto-Biome style/palette inference.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct BiomeQueryRequest {
    pub dimension: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Server-to-client biome lookup response (canonical biome identifier, e.g. `minecraft:cherry_grove`).
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct BiomeQueryResponse {
    pub biome: String,
}

/// Client-to-server undo request.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct UndoRequest {
    pub dimension: String,
}

/// Client-to-server redo request: re-apply the last undone build edit.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct RedoRequest {
    pub dimension: String,
}

/// Client-to-server reroll request: undo last build and rebuild at the same spot with a new seed.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct RerollRequest {
    pub dimension: String,
}

/// Translates known built-in/inferred palette names bidirectionally between English and Russian.
pub fn translate_palette_name<'a>(name: &'a str, lang: UiLang) -> &'a str {
    const PAIRS: [(&str, &str); 33] = [
        ("Cozy Cottage (Oak & Slate)", "Уютный коттедж (дуб и сланец)"),
        ("Medieval Timber-Frame (Tudor)", "Средневековый фахверк (Тюдор)"),
        ("Japanese Teahouse / Pagoda", "Японский чайный дом / пагода"),
        ("Gothic (Deepslate & Dark Oak)", "Готика (сланец и тёмный дуб)"),
        ("Modern (White Concrete & Quartz)", "Модерн (белый бетон и кварц)"),
        ("Orient & Desert (Sandstone)", "Восток и пустыня (песчаник)"),
        ("Nordic Timber (Spruce & Tuff)", "Скандинавский сруб (ель и туф)"),
        ("Steampunk (Copper & Tuff Bricks)", "Стимпанк (медь и туфовый кирпич)"),
        ("Birch, Bricks & Copper", "Берёза, кирпич и медь"),
        ("Cherry Garden (Sakura & Calcite)", "Вишнёвый сад (сакура и кальцит)"),
        ("Ocean & Prismarine", "Океан и призмарин"),
        ("Tropical Villa (Bamboo & Jungle)", "Тропическая вилла (бамбук и джунгли)"),
        ("Arcane Observatory (Amethyst)", "Магическая обсерватория (аметист)"),
        ("Nether Citadel (Crimson Stone)", "Незер-цитадель (багровый камень)"),
        ("Pale Manor (Tuff & Pale Oak)", "Бледный особняк (туф и белый дуб)"),
        ("Winter Chalet (Snow, Ice & Spruce)", "Зимнее шале (снег, лёд и ель)"),
        ("Classical (Travertine & Marble)", "Античность (травертин и мрамор)"),
        ("Lighthouse (Concrete & Stone)", "Маяк (белый бетон и камень)"),
        ("Obsidian Ziggurat", "Обсидиановый зиккурат"),
        ("Pyramid (Sandstone & Gold)", "Пирамида (песчаник и золото)"),
        ("Observatory (Copper, Quartz & Slate)", "Обсерватория (медь, кварц и сланец)"),
        ("Clock Tower (Brick & Copper)", "Часовая башня (кирпич и медь)"),
        ("Hot Spring Onsen", "Горячий источник Онсэн"),
        ("Ishtar Gate (Lapis & Gold)", "Ворота Иштар (лазурит и золото)"),
        ("Domed Palace / Mosque", "Купольный дворец / Мечеть"),
        ("Hi-Tech Steel & Glass", "Хай-тек сталь и стекло"),
        ("Spaceport / Rocket", "Космодром / Ракета"),
        ("Archmage Tower (Amethyst & Purpur)", "Башня Архимага (аметист и пурпур)"),
        ("Fairytale Cottage", "Сказочный коттедж"),
        ("Dwarven Halls", "Дварфийские чертоги"),
        ("Water Torii Shrine", "Святилище Тории на воде"),
        ("Glass Greenhouse", "Стеклянная теплица"),
        ("Custom Palette", "Своя палитра"),
    ];
    for &(en, ru) in &PAIRS {
        if name == en || name == ru {
            return match lang {
                UiLang::En => en,
                UiLang::Ru => ru,
            };
        }
    }
    name
}

/// Client-to-server UI/command language synchronization payload.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct SetLangRequest {
    pub lang: UiLang,
}

/// Server-to-client build or undo feedback response.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct BuildResponse {
    pub ok: bool,
    pub blocks_changed: i32,
    pub message: String,
}
