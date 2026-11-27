use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, TextField, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientTickPreEvent, CrosshairTarget, Flow, Heightmap, Identifier, InputKeyEvent,
    KeyAction, MouseButtonPreEvent, RenderGuiPostEvent, ScreenCharTypedPreEvent,
    ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseButtonReleasedPreEvent, ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, SubmitCustomGeometryEvent, Text, WorldDepth, WorldLine, WorldQuad,
};

use crate::model::{
    BuildCategory, BuildRequest, BuildResponse, BuildStyle, CardinalFacing, MaterialPalette,
    NoiseLevel, RedoRequest, RerollRequest, RoadConnectRequest, SetLangRequest, UiLang,
    UndoRequest,
};
use crate::network::channels;
use crate::persist::{
    MatFilter, MAX_SAVED_FAVORITES, SavedPreset, apply_block_to_slot, decode_share_code_lang,
    encode_share_code, filtered_picker_choices, parse_favorites, remove_block_from_slot_at,
    replace_block_in_slot_at, serialize_favorites, toggle_block_in_slot,
};
use mod_api::ConfigPath;

pub const SCREEN_ID: &str = "building_generator";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiTab {
    SizeArea,
    DescNoise,
    Materials,
    WorldDecor,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TriToggle {
    Auto,
    On,
    Off,
}

impl TriToggle {
    fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::On,
            2 => Self::Off,
            _ => Self::Auto,
        }
    }

    fn to_raw(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::On => 1,
            Self::Off => 2,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Auto => Self::On,
            Self::On => Self::Off,
            Self::Off => Self::Auto,
        }
    }

    fn badge(self, lang: UiLang) -> &'static str {
        match self {
            Self::Auto => match lang {
                UiLang::En => "A",
                UiLang::Ru => "А",
            },
            Self::On => "+",
            Self::Off => "-",
        }
    }
}

fn current_saved_preset(st: &ClientUiState, name: String) -> SavedPreset {
    SavedPreset {
        name,
        width: st.width,
        depth: st.depth,
        height: st.height,
        category: st.category,
        style: st.style,
        roof_raw: st.roof_choice as u8,
        toggles: st.toggles.map(TriToggle::to_raw),
        noise_level: st.noise_level,
        adapt_terrain: st.adapt_terrain,
        air_only: st.air_only,
        wear_pct: st.wear_pct,
        overgrowth_pct: st.overgrowth_pct,
        description: st.description.clone(),
        seed: st.seed,
        palette_manual: st.palette_manual,
        palette: st.palette.clone(),
        living_decor: st.living_decor,
        spawn_npcs: st.spawn_npcs,
        blend_radius: st.blend_radius,
    }
}

fn apply_saved_preset(st: &mut ClientUiState, preset: SavedPreset) {
    st.width = preset.width;
    st.depth = preset.depth;
    st.height = preset.height;
    st.category = preset.category;
    st.style = preset.style;
    st.roof_choice = RoofChoice::from_raw(preset.roof_raw);
    for i in 0..5 {
        st.toggles[i] = TriToggle::from_raw(preset.toggles[i]);
    }
    st.noise_level = preset.noise_level;
    st.adapt_terrain = preset.adapt_terrain;
    st.air_only = preset.air_only;
    st.wear_pct = preset.wear_pct.min(100);
    st.overgrowth_pct = preset.overgrowth_pct.min(100);
    st.description = preset.description;
    st.seed = preset.seed;
    st.seed_text = preset.seed.to_string();
    st.auto_seed = false;
    st.palette_manual = preset.palette_manual;
    st.palette = preset.palette;
    st.living_decor = preset.living_decor;
    st.spawn_npcs = preset.spawn_npcs;
    st.blend_radius = preset.blend_radius.min(10);
    sync_palette_from_ui(st);
    st.mat_list_scroll = 0;
    st.picker_scroll = 0;
    st.bom_scroll = 0;
    st.bp_cache_key = None;
    st.preview_key = None;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RoofChoice {
    Auto,
    Gable,
    Hip,
    Steep,
    Pagoda,
    Dome,
    Mansard,
    Flat,
}

impl RoofChoice {
    fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::Gable,
            2 => Self::Hip,
            3 => Self::Steep,
            4 => Self::Pagoda,
            5 => Self::Dome,
            6 => Self::Mansard,
            7 => Self::Flat,
            _ => Self::Auto,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Auto => Self::Gable,
            Self::Gable => Self::Hip,
            Self::Hip => Self::Steep,
            Self::Steep => Self::Pagoda,
            Self::Pagoda => Self::Dome,
            Self::Dome => Self::Mansard,
            Self::Mansard => Self::Flat,
            Self::Flat => Self::Auto,
        }
    }

    fn label(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::Auto => "Auto",
                Self::Gable => "Gable",
                Self::Hip => "Hip",
                Self::Steep => "Steep",
                Self::Pagoda => "Curved",
                Self::Dome => "Dome",
                Self::Mansard => "Mansard",
                Self::Flat => "Flat",
            },
            UiLang::Ru => match self {
                Self::Auto => "Авто",
                Self::Gable => "Двускат",
                Self::Hip => "Шатёр",
                Self::Steep => "Острая",
                Self::Pagoda => "Изогнут.",
                Self::Dome => "Купол",
                Self::Mansard => "Мансарда",
                Self::Flat => "Плоская",
            },
        }
    }

    fn tag(self, lang: UiLang) -> &'static str {
        match lang {
            UiLang::En => match self {
                Self::Auto => "",
                Self::Gable => "gable roof",
                Self::Hip => "hip roof",
                Self::Steep => "steep roof",
                Self::Pagoda => "pagoda roof",
                Self::Dome => "dome",
                Self::Mansard => "mansard roof",
                Self::Flat => "flat roof",
            },
            UiLang::Ru => match self {
                Self::Auto => "",
                Self::Gable => "двускатная крыша",
                Self::Hip => "шатровая крыша",
                Self::Steep => "крутая острая крыша",
                Self::Pagoda => "изогнутая крыша",
                Self::Dome => "купол",
                Self::Mansard => "мансардная крыша",
                Self::Flat => "плоская крыша",
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorldSelectMode {
    Inactive,
    PickingCorner1,
    PickingCorner2 { c1: [i32; 3] },
    MovePreview,
    PickingRoadStart,
    PickingRoadEnd { start: [i32; 3] },
}

#[derive(Clone)]
enum PendingAction {
    OpenMenu,
    CloseMenu,
    StartWorldSelect,
    StartMovePreview,
    StartRoadSelect,
    PlaceInFront,
    CopyShareCode(SavedPreset),
    PasteShareCode,
    CopyBomText(String),
    SendBuild(BuildRequest),
    SendRoad(RoadConnectRequest),
    SendReroll(String),
    SendUndo(String),
    SendRedo(String),
}

#[derive(Clone, Copy)]
struct ScrollZone {
    viewport: [f32; 4],
    track: [f32; 4],
    total_rows: usize,
    vis_rows: usize,
    scroll: usize,
}

struct WidgetHandles {
    bom_btn: WidgetId,
    lang_btn: WidgetId,
    tab_size: Option<WidgetId>,
    tab_desc: Option<WidgetId>,
    tab_mats: Option<WidgetId>,
    tab_world: Option<WidgetId>,

    // Tab 1: Size & Area
    w_dec: Option<WidgetId>,
    w_inc: Option<WidgetId>,
    d_dec: Option<WidgetId>,
    d_inc: Option<WidgetId>,
    h_dec: Option<WidgetId>,
    h_inc: Option<WidgetId>,
    preset_small: Option<WidgetId>,
    preset_med: Option<WidgetId>,
    preset_large: Option<WidgetId>,
    preset_tower: Option<WidgetId>,
    place_front: Option<WidgetId>,
    pick_world: Option<WidgetId>,
    rotate_btn: Option<WidgetId>,
    y_dec_btn: Option<WidgetId>,
    y_inc_btn: Option<WidgetId>,
    terrain_btn: Option<WidgetId>,
    preview_btn: Option<WidgetId>,
    air_only_btn: Option<WidgetId>,
    slice_toggle_btn: Option<WidgetId>,
    slice_dec_btn: Option<WidgetId>,
    slice_inc_btn: Option<WidgetId>,
    frame_btn: Option<WidgetId>,
    collision_btn: Option<WidgetId>,
    move_preview_btn: Option<WidgetId>,
    auto_hide_btn: Option<WidgetId>,
    fav_save_btn: Option<WidgetId>,
    fav_load_btn: Option<WidgetId>,
    fav_del_btn: Option<WidgetId>,
    share_copy_btn: Option<WidgetId>,
    share_paste_btn: Option<WidgetId>,

    // Tab 2: Description & Noise
    cat_btn: Option<WidgetId>,
    style_btn: Option<WidgetId>,
    roof_btn: Option<WidgetId>,
    desc_field: Option<WidgetId>,
    seed_field: Option<WidgetId>,
    prev_seed_btn: Option<WidgetId>,
    new_seed_btn: Option<WidgetId>,
    auto_seed_btn: Option<WidgetId>,
    archetype_prev_btn: Option<WidgetId>,
    archetype_btn: Option<WidgetId>,
    archetype_next_btn: Option<WidgetId>,
    noise_btn: Option<WidgetId>,
    biome_btn: Option<WidgetId>,
    wear_dec_btn: Option<WidgetId>,
    wear_btn: Option<WidgetId>,
    wear_inc_btn: Option<WidgetId>,
    over_dec_btn: Option<WidgetId>,
    over_btn: Option<WidgetId>,
    over_inc_btn: Option<WidgetId>,
    tag_buttons: Vec<(WidgetId, &'static str)>,
    feature_toggles: Vec<(WidgetId, usize)>,

    // Dedicated Preset Catalog Menu (`preset_picker_open == true`)
    preset_back_btn: Option<WidgetId>,
    preset_cat_btn: Option<WidgetId>,
    preset_search_field: Option<WidgetId>,
    preset_search_clear_btn: Option<WidgetId>,
    preset_up_btn: Option<WidgetId>,
    preset_down_btn: Option<WidgetId>,
    preset_grid_btns: Vec<(WidgetId, usize)>,

    // Dedicated Bill of Materials Modal (`bom_open == true`)
    bom_back_btn: Option<WidgetId>,
    bom_copy_btn: Option<WidgetId>,
    bom_up_btn: Option<WidgetId>,
    bom_down_btn: Option<WidgetId>,

    // Tab 3: Materials & Per-Slot Block Lists
    mat_preset_btn: Option<WidgetId>,
    mat_infer_btn: Option<WidgetId>,
    mat_sync_wood_btn: Option<WidgetId>,
    mat_mix_mode_btn: Option<WidgetId>,
    mat_slot_up_btn: Option<WidgetId>,
    mat_slot_down_btn: Option<WidgetId>,
    mat_slot_btns: Vec<(WidgetId, usize)>,
    list_clear_btn: Option<WidgetId>,
    list_up_btn: Option<WidgetId>,
    list_down_btn: Option<WidgetId>,
    list_item_btns: Vec<(WidgetId, usize)>,
    list_remove_btns: Vec<(WidgetId, usize)>,
    picker_add_open_btn: Option<WidgetId>,
    list_pop_btn: Option<WidgetId>,

    // Dedicated Selection Menu (`picker_open == true`)
    picker_back_btn: Option<WidgetId>,
    picker_mode_btn: Option<WidgetId>,
    picker_catalog_btn: Option<WidgetId>,
    mat_filter_btns: Vec<(WidgetId, MatFilter)>,
    mat_search_field: Option<WidgetId>,
    mat_search_clear_btn: Option<WidgetId>,
    picker_up_btn: Option<WidgetId>,
    picker_down_btn: Option<WidgetId>,
    picker_grid_btns: Vec<(WidgetId, String)>,
    picker_chip_btns: Vec<(WidgetId, usize)>,

    // Tab 4: World & Decor
    living_decor_btn: Option<WidgetId>,
    spawn_npcs_btn: Option<WidgetId>,
    blend_radius_btn: Option<WidgetId>,
    road_width_btn: Option<WidgetId>,
    road_auto_bridge_btn: Option<WidgetId>,
    road_style_btn: Option<WidgetId>,
    road_connect_btn: Option<WidgetId>,

    // Persistent Footer
    generate_btn: WidgetId,
    reroll_btn: WidgetId,
    undo_btn: WidgetId,
    redo_btn: WidgetId,
    close_btn: WidgetId,
}

#[derive(Clone, Debug)]
struct TooltipEntry {
    id: WidgetId,
    rect: [f32; 4],
    title: String,
    desc: String,
}

struct BuiltScreen {
    size: [f32; 2],
    panel: [f32; 4],
    widgets: Widgets,
    handles: WidgetHandles,
    tooltips: Vec<TooltipEntry>,
    slot_scroll_zone: Option<ScrollZone>,
    list_scroll_zone: Option<ScrollZone>,
    picker_scroll_zone: Option<ScrollZone>,
    preset_scroll_zone: Option<ScrollZone>,
    bom_scroll_zone: Option<ScrollZone>,
    tab3_slot_rows: Vec<(usize, [f32; 4])>,
    tab3_list_rows: Vec<(usize, String, [f32; 4])>,
    tab3_header_pos: Option<[f32; 2]>,
    picker_tiles: Vec<(String, bool, [f32; 4])>,
    picker_chips: Vec<(usize, String, [f32; 4])>,
    picker_strip_pos: Option<[f32; 2]>,
    bom_rows: Vec<(String, usize, [f32; 4])>,
}

#[derive(Clone, PartialEq, Eq)]
struct BpCacheKey {
    width: u16,
    depth: u16,
    height: u16,
    category: BuildCategory,
    style: BuildStyle,
    facing: CardinalFacing,
    noise_level: NoiseLevel,
    eff_desc: String,
    seed: u64,
    roof_raw: u8,
    toggles_raw: [u8; 5],
    adapt_terrain: bool,
    terrain_fp: u64,
    palette_fp: u64,
    wear_pct: u8,
    overgrowth_pct: u8,
    living_decor: bool,
    biome_hint: String,
}

#[derive(Clone, PartialEq, Eq)]
struct PreviewKey {
    bp_fp: u64,
    preview_slice_y: Option<i32>,
    air_only: bool,
    show_collisions: bool,
    world_mask_fp: u64,
}

struct ClientUiState {
    lang: UiLang,
    lang_loaded: bool,
    was_playing: bool,
    pending_save_lang: bool,
    pending_send_lang: Option<UiLang>,
    tab: UiTab,
    width: u16,
    depth: u16,
    height: u16,
    origin: Option<[i32; 3]>,
    dimension: String,
    facing: CardinalFacing,
    adapt_terrain: bool,
    air_only: bool,
    wear_pct: u8,
    overgrowth_pct: u8,
    living_decor: bool,
    spawn_npcs: bool,
    blend_radius: u8,
    road_width: u8,
    road_auto_bridge: bool,
    use_biome_auto: bool,
    detected_biome: String,
    last_biome_query: Option<(String, i32, i32, i32)>,
    show_preview_box: bool,
    show_frame_box: bool,
    show_collisions: bool,
    auto_hide_on_build: bool,
    saved_overlay_vis: Option<(bool, bool)>,
    hud_toast: Option<(String, u64)>,
    preview_slice_y: Option<i32>,
    category: BuildCategory,
    style: BuildStyle,
    roof_choice: RoofChoice,
    toggles: [TriToggle; 5],
    favorites: Vec<SavedPreset>,
    fav_idx: usize,
    favorites_loaded: bool,
    pending_save_favorites: bool,
    description: String,
    seed: u64,
    seed_text: String,
    seed_history: Vec<u64>,
    auto_seed: bool,
    undo_count: usize,
    redo_count: usize,
    build_seq: u64,
    noise_level: NoiseLevel,
    palette_preset_idx: usize,
    archetype_idx: usize,
    palette_manual: bool,
    palette: MaterialPalette,
    selected_mat_slot: usize,
    mat_slot_scroll: usize,
    mat_list_scroll: usize,
    picker_open: bool,
    picker_replace_idx: Option<usize>,
    picker_all_blocks: bool,
    picker_scroll: usize,
    preset_picker_open: bool,
    preset_scroll: usize,
    preset_search: String,
    preset_cat_filter: Option<BuildCategory>,
    bom_open: bool,
    bom_scroll: usize,
    world_blocks: Vec<String>,
    mat_filter: MatFilter,
    mat_search: String,
    auto_sync_wood: bool,
    world_select: WorldSelectMode,
    aimed_block: Option<[i32; 3]>,
    road_preview_cache: Option<([i32; 3], [i32; 3], Vec<f64>)>,
    terrain_cache_key: Option<(bool, i32, i32, i32, u16, u16)>,
    cached_terrain_offsets: Option<Vec<i32>>,
    cached_water_depths: Option<Vec<i32>>,
    cached_terrain_fp: u64,
    last_terrain_sample_ms: u64,
    bp_cache_key: Option<BpCacheKey>,
    cached_bp: Option<crate::generator::GeneratedBlueprint>,
    cached_bp_fp: u64,
    world_mask_key: Option<(String, i32, i32, i32, u16, u16, u16, u64, bool, bool)>,
    cached_world_mask: Option<crate::gen_preview::PreviewWorldMask>,
    cached_world_mask_fp: u64,
    last_world_mask_ms: u64,
    preview_key: Option<PreviewKey>,
    preview_mesh: crate::gen_preview::HologramMesh,
    preview_bom: Vec<(String, usize)>,
    preview_total_blocks: usize,
    build_progress: Option<(u8, usize, usize, String)>,
    status: String,
    pending: Option<PendingAction>,
    dirty: bool,
    screen: Option<BuiltScreen>,
    hover_widget: Option<WidgetId>,
    hover_since_ms: u64,
}

impl Default for ClientUiState {
    fn default() -> Self {
        let seed = crate::noise::fresh_entropy(1, 0);
        Self {
            lang: UiLang::En,
            lang_loaded: false,
            was_playing: false,
            pending_save_lang: false,
            pending_send_lang: None,
            tab: UiTab::SizeArea,
            width: 13,
            depth: 11,
            height: 12,
            origin: None,
            dimension: "minecraft:overworld".into(),
            facing: CardinalFacing::South,
            adapt_terrain: true,
            air_only: false,
            wear_pct: 0,
            overgrowth_pct: 0,
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 3,
            road_width: 3,
            road_auto_bridge: true,
            use_biome_auto: true,
            detected_biome: String::new(),
            last_biome_query: None,
            show_preview_box: true,
            show_frame_box: true,
            show_collisions: true,
            auto_hide_on_build: false,
            saved_overlay_vis: None,
            hud_toast: None,
            preview_slice_y: None,
            category: BuildCategory::House,
            style: BuildStyle::Auto,
            roof_choice: RoofChoice::Auto,
            toggles: [TriToggle::Auto; 5],
            favorites: Vec::new(),
            fav_idx: 0,
            favorites_loaded: false,
            pending_save_favorites: false,
            description: "cozy house".into(),
            seed,
            seed_text: seed.to_string(),
            seed_history: Vec::new(),
            auto_seed: true,
            undo_count: 0,
            redo_count: 0,
            build_seq: 1,
            noise_level: NoiseLevel::Balanced,
            palette_preset_idx: 0,
            archetype_idx: 0,
            palette_manual: false,
            palette: MaterialPalette::preset(0),
            selected_mat_slot: 1,
            mat_slot_scroll: 0,
            mat_list_scroll: 0,
            picker_open: false,
            picker_replace_idx: None,
            picker_all_blocks: false,
            picker_scroll: 0,
            preset_picker_open: false,
            preset_scroll: 0,
            preset_search: String::new(),
            preset_cat_filter: None,
            bom_open: false,
            bom_scroll: 0,
            world_blocks: Vec::new(),
            mat_filter: MatFilter::All,
            mat_search: String::new(),
            auto_sync_wood: true,
            world_select: WorldSelectMode::Inactive,
            aimed_block: None,
            road_preview_cache: None,
            terrain_cache_key: None,
            cached_terrain_offsets: None,
            cached_water_depths: None,
            cached_terrain_fp: 0,
            last_terrain_sample_ms: 0,
            bp_cache_key: None,
            cached_bp: None,
            cached_bp_fp: 0,
            world_mask_key: None,
            cached_world_mask: None,
            cached_world_mask_fp: 0,
            last_world_mask_ms: 0,
            preview_key: None,
            preview_mesh: crate::gen_preview::HologramMesh::default(),
            preview_bom: Vec::new(),
            preview_total_blocks: 0,
            build_progress: None,
            status: "F8 — toggle menu. Set size, area, description & materials.".into(),
            pending: None,
            dirty: true,
            screen: None,
            hover_widget: None,
            hover_since_ms: 0,
        }
    }
}

static CLIENT_STATE: Mutex<Option<ClientUiState>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut ClientUiState) -> R) -> R {
    let mut guard = CLIENT_STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(ClientUiState::default);
    f(st)
}

const TOGGLE_META_EN: [(&str, &str, &str); 5] = [
    ("Porch", "with porch", "no porch"),
    ("Balcony", "with balcony", "no balcony"),
    ("Cellar", "with basement", "no basement"),
    ("Awning", "with awning", "no awning"),
    ("Garden", "with garden", "no garden"),
];

const TOGGLE_META_RU: [(&str, &str, &str); 5] = [
    ("Крыльцо", "с крыльцом", "без крыльца"),
    ("Балкон", "с балконом", "без балкона"),
    ("Подвал", "с подвалом", "без подвала"),
    ("Навес", "с лавкой", "без навеса"),
    ("Сад", "с садом", "без сада"),
];

fn toggle_meta(lang: UiLang) -> &'static [(&'static str, &'static str, &'static str); 5] {
    match lang {
        UiLang::En => &TOGGLE_META_EN,
        UiLang::Ru => &TOGGLE_META_RU,
    }
}

#[inline]
fn tr<'a>(lang: UiLang, en: &'a str, ru: &'a str) -> &'a str {
    match lang {
        UiLang::En => en,
        UiLang::Ru => ru,
    }
}

fn effective_description(st: &ClientUiState) -> String {
    let mut parts: Vec<String> = Vec::new();
    let trimmed = st.description.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_owned());
    }
    let rtag = st.roof_choice.tag(st.lang);
    if !rtag.is_empty() {
        parts.push(rtag.to_owned());
    }
    for (idx, (_, on_kw, off_kw)) in toggle_meta(st.lang).iter().enumerate() {
        match st.toggles[idx] {
            TriToggle::On => parts.push((*on_kw).to_owned()),
            TriToggle::Off => parts.push((*off_kw).to_owned()),
            TriToggle::Auto => {}
        }
    }
    parts.join(", ")
}

fn translate_known_description(desc: &str, to_lang: UiLang) -> Option<String> {
    let trimmed = desc.trim();
    if trimmed.eq_ignore_ascii_case("cozy house") || trimmed == "уютный дом" {
        return Some(match to_lang {
            UiLang::En => "cozy house".to_owned(),
            UiLang::Ru => "уютный дом".to_owned(),
        });
    }
    for (idx, preset) in CLI_PRESETS.iter().enumerate() {
        if trimmed == preset.6 || trimmed == CLI_PRESET_DESCS_EN[idx] {
            return Some(cli_preset_desc(idx, to_lang).to_owned());
        }
    }
    None
}

fn apply_ui_lang(st: &mut ClientUiState, next_lang: UiLang, notify_server: bool) {
    if st.lang == next_lang {
        if notify_server {
            st.pending_send_lang = Some(next_lang);
        }
        return;
    }
    st.lang = next_lang;
    if let Some(mapped) = translate_known_description(&st.description, next_lang) {
        st.description = mapped;
    }
    st.status = match next_lang {
        UiLang::En => "Language: English".into(),
        UiLang::Ru => "Язык: Русский".into(),
    };
    st.pending_save_lang = true;
    if notify_server {
        st.pending_send_lang = Some(next_lang);
    }
    st.dirty = true;
}

fn push_seed_history(st: &mut ClientUiState, old_seed: u64) {
    if st.seed_history.last().copied() != Some(old_seed) {
        if st.seed_history.len() >= 24 {
            st.seed_history.remove(0);
        }
        st.seed_history.push(old_seed);
    }
}

fn parse_counter_marker(msg: &str, keys: &[&str]) -> Option<usize> {
    for &key in keys {
        if let Some(pos) = msg.find(key) {
            let tail = &msg[pos + key.len()..];
            if let Some(slash) = tail.find('/')
                && let Ok(n) = tail[..slash].trim().parse::<usize>()
            {
                return Some(n);
            }
        }
    }
    None
}

pub fn on_server_response(resp: BuildResponse) {
    with_state(|st| {
        if resp.message == "__OPEN_GUI__" {
            st.pending = Some(PendingAction::OpenMenu);
        } else if let Some(code) = resp.message.strip_prefix("__LANG__:") {
            if let Some(parsed) = UiLang::parse(code) {
                apply_ui_lang(st, parsed, false);
            }
        } else if let Some(rest) = resp.message.strip_prefix("__HISTORY__:") {
            let mut parts = rest.splitn(2, ':');
            let u = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let r = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            st.undo_count = u;
            st.redo_count = r;
            st.dirty = true;
        } else if let Some(rest) = resp.message.strip_prefix("__PROGRESS__:") {
            let mut parts = rest.splitn(4, ':');
            let pct = parts.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
            let done = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let total = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
            let fallback = match st.lang {
                UiLang::En => "Build",
                UiLang::Ru => "Стройка",
            };
            let label = parts.next().unwrap_or(fallback).to_owned();
            st.build_progress = Some((pct.min(100), done, total.max(1), label.clone()));
            st.status = match st.lang {
                UiLang::En => format!("Building {label}: {pct}% ({done}/{total})"),
                UiLang::Ru => format!("Стройка {label}: {pct}% ({done}/{total})"),
            };
        } else {
            st.build_progress = None;
            if let Some(u) = parse_counter_marker(&resp.message, &["[undo:", "[отмен:"]) {
                st.undo_count = u;
                st.dirty = true;
            }
            if let Some(r) = parse_counter_marker(&resp.message, &["redo:", "возврат:"]) {
                st.redo_count = r;
                st.dirty = true;
            } else if resp.ok && (resp.message.starts_with("Ready:") || resp.message.starts_with("Готово:") || resp.message.contains("seed ")) {
                st.redo_count = 0;
                st.dirty = true;
            }
            let finished_build = resp.ok
                && (resp.message.starts_with("Ready:")
                    || resp.message.starts_with("Готово:")
                    || resp.message.contains("Build complete")
                    || resp.message.contains("Завершена постройка"))
                && !resp.message.starts_with("Large build")
                && !resp.message.starts_with("Крупная постройка");
            if finished_build && st.auto_hide_on_build && (st.show_frame_box || st.show_preview_box) {
                st.saved_overlay_vis = Some((st.show_frame_box, st.show_preview_box));
                st.show_frame_box = false;
                st.show_preview_box = false;
                st.dirty = true;
            }
            st.status = resp.message;
        }
    });
}

pub fn on_biome_response(resp: crate::model::BiomeQueryResponse) {
    with_state(|st| {
        let next = resp.biome.trim().to_owned();
        if st.detected_biome != next {
            st.detected_biome = next;
            if st.use_biome_auto && !st.palette_manual {
                sync_palette_from_ui(st);
            }
            st.preview_key = None;
            st.dirty = true;
        }
    });
}

fn short_block_name(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

const PICKER_COLS: usize = 3;

const SLOT_LABELS_EN: [&str; 30] = [
    "Found.",
    "Wall 1",
    "Accent",
    "Frame",
    "Roof",
    "Floor 1",
    "Glass",
    "Door",
    "Shutter",
    "Walling",
    "Leaves",
    "Light",
    "Wall 2",
    "Trim",
    "Beams",
    "Pillar",
    "Edge",
    "Floor 2",
    "Ceiling",
    "Stained",
    "In.Door",
    "Fence",
    "Gate",
    "Stairs",
    "Chimney",
    "Leaf 2",
    "Sconce",
    "Carpet",
    "Path",
    "Awning",
];

const SLOT_LABELS_RU: [&str; 30] = [
    "Фунд.",
    "Стены 1",
    "Акцент",
    "Каркас",
    "Крыша",
    "Пол 1",
    "Окна",
    "Вх. дв.",
    "Ставни",
    "Ограда",
    "Листва",
    "Свет",
    "Стены 2",
    "Бордюр",
    "Балки",
    "Колонн.",
    "Кромка",
    "Пол 2",
    "Потол.",
    "Витраж",
    "Комн.дв",
    "Перила",
    "Калитка",
    "Лестн.",
    "Дымох.",
    "Лист. 2",
    "Бра",
    "Ковёр",
    "Дорожка",
    "Тент",
];

fn slot_label(slot: usize, lang: UiLang) -> &'static str {
    let idx = slot.min(29);
    match lang {
        UiLang::En => SLOT_LABELS_EN[idx],
        UiLang::Ru => SLOT_LABELS_RU[idx],
    }
}

fn slot_block_id(pal: &MaterialPalette, slot: usize) -> &str {
    match slot {
        0 => &pal.foundation,
        1 => &pal.wall,
        2 => &pal.wall_accent,
        3 => &pal.frame,
        4 => &pal.roof_block,
        5 => &pal.floor,
        6 => &pal.window,
        7 => &pal.door,
        8 => &pal.trapdoor,
        9 => &pal.trim_wall,
        10 => &pal.leaves,
        11 => &pal.light,
        12 => &pal.wall_upper,
        13 => &pal.wall_trim,
        14 => &pal.beam,
        15 => &pal.pillar,
        16 => &pal.roof_edge,
        17 => &pal.floor_upper,
        18 => &pal.ceiling,
        19 => &pal.window_accent,
        20 => &pal.interior_door,
        21 => &pal.fence,
        22 => &pal.fence_gate,
        23 => &pal.stair_block,
        24 => &pal.chimney_block,
        25 => &pal.leaves_alt,
        26 => &pal.light_accent,
        27 => &pal.carpet,
        28 => &pal.paving,
        _ => &pal.awning,
    }
}

fn current_picker_choices(st: &ClientUiState) -> Vec<String> {
    filtered_picker_choices(
        st.selected_mat_slot,
        st.picker_all_blocks,
        &st.world_blocks,
        st.mat_filter,
        &st.mat_search,
    )
}

fn palette_fingerprint(pal: &MaterialPalette) -> u64 {
    fn feed(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x0100_0000_01b3);
        }
        *h ^= 0xff;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }

    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ u64::from(pal.mix_mode.to_idx());
    for s in 0..MaterialPalette::SLOT_COUNT {
        for blk in pal.slot_blocks(s) {
            feed(&mut h, blk.as_bytes());
        }
        h ^= 0xfe;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub(crate) const HOVER_TOOLTIP_DELAY_MS: u64 = 1000;

fn point_in_rect(pt: [f32; 2], rect: [f32; 4]) -> bool {
    pt[0] >= rect[0]
        && pt[1] >= rect[1]
        && pt[0] < rect[0] + rect[2]
        && pt[1] < rect[1] + rect[3]
}

pub(crate) fn category_tooltip_desc(cat: BuildCategory, lang: UiLang) -> &'static str {
    match (lang, cat) {
        (UiLang::En, BuildCategory::House) => "Residential homes, cottages, villas, taverns & multi-room living quarters with full interiors.",
        (UiLang::En, BuildCategory::Tower) => "Vertical landmarks: watchtowers, mage spires, pagodas, lighthouses, windmills & clock towers.",
        (UiLang::En, BuildCategory::Barn) => "Rural barns, grain storehouses, lofts, stables & covered timber storage halls.",
        (UiLang::En, BuildCategory::Pavilion) => "Open-air market stalls, gazebos, teahouses, train stations, ships, airships & civic halls.",
        (UiLang::En, BuildCategory::Castle) => "Fortified citadels, bailey keeps, bastions, colosseums, city gates & defensive walls.",
        (UiLang::En, BuildCategory::Forge) => "Blacksmith workshops, smelteries, watermills, steam foundries & crafting yards.",
        (UiLang::En, BuildCategory::Temple) => "Cathedrals, shrines, pyramids, pantheons, mosques, monasteries & monumental arches.",
        (UiLang::En, BuildCategory::Bridge) => "River and ravine bridges across 9 archetypes with walkable ramps, steps & full headroom.",
        (UiLang::En, BuildCategory::Settlement) => "Entire terrain-following walled cities with plazas, avenues, canals, bridges & lots.",
        (UiLang::En, BuildCategory::Dungeon) => "Multi-tier subterranean dungeons & mineshafts with switchback stairs, vaults & loot.",
        (UiLang::En, BuildCategory::Estate) => "Full country estates combining manor house, barn, irrigated crops, windmill & paddock.",
        (UiLang::En, BuildCategory::Engineering) => "Canal locks, railway roundhouses, gantry drydocks & electrical power substations.",
        (UiLang::En, BuildCategory::Agricultural) => "Hillside vineyards, masonry grain silos, orchard apiaries & royal horse stables.",
        (UiLang::En, BuildCategory::DecorArt) => "Tiered plaza fountains, guardian statues, evergreen hedge mazes & zen karesansui gardens.",
        (UiLang::Ru, BuildCategory::House) => "Жилые дома, коттеджи, виллы, таверны и особняки с многокомнатной меблировкой.",
        (UiLang::Ru, BuildCategory::Tower) => "Высокие башни, шпили магов, пагоды, маяки, ветряные мельницы и башни с часами.",
        (UiLang::Ru, BuildCategory::Barn) => "Амбары, сеновалы, склады припасов и деревянные хозяйственные постройки.",
        (UiLang::Ru, BuildCategory::Pavilion) => "Павильоны, беседки, вокзалы, корабли, дирижабли, базары и общественные залы.",
        (UiLang::Ru, BuildCategory::Castle) => "Цитадели, замки, бастионы, колизеи, городские ворота и крепостные стены.",
        (UiLang::Ru, BuildCategory::Forge) => "Кузницы, плавильни, водяные мельницы, мастерские и ремесленные дворы.",
        (UiLang::Ru, BuildCategory::Temple) => "Соборы, святилища, пирамиды, пантеоны, мечети, монастыри и триумфальные арки.",
        (UiLang::Ru, BuildCategory::Bridge) => "Мосты и виадуки 9 типов через реки и ущелья со ступенями и высоким проходом.",
        (UiLang::Ru, BuildCategory::Settlement) => "Целые города и деревни по рельефу с улицами, каналами, площадями, стенами и домами.",
        (UiLang::Ru, BuildCategory::Dungeon) => "Многоуровневые подземные данжи и шахты с лестницами, ловушками и добычей.",
        (UiLang::Ru, BuildCategory::Estate) => "Поместья: усадебный дом, амбар, мельница, орошаемые грядки и загон со скотом.",
        (UiLang::Ru, BuildCategory::Engineering) => "Судоходные шлюзы, веерные депо поездов, стапели с краном и электроподстанции.",
        (UiLang::Ru, BuildCategory::Agricultural) => "Виноградники с прессом, зерновые силосы, пасеки в цветущем саду и конюшни.",
        (UiLang::Ru, BuildCategory::DecorArt) => "Фонтаны, монументальные статуи стражей, садовые лабиринты и дзен-сады камней.",
    }
}

pub(crate) fn style_tooltip_desc(style: BuildStyle, lang: UiLang) -> &'static str {
    match (lang, style) {
        (UiLang::En, BuildStyle::Auto) => "Infers architecture & road style from prompt text, seed noise, or detected world biome.",
        (UiLang::En, BuildStyle::Medieval) => "Timber-framed stone, half-timber facades, cobble roads & spruce roofs.",
        (UiLang::En, BuildStyle::Cottage) => "Warm rustic wood, mossy stone, packed-mud garden paths & lush flowerboxes.",
        (UiLang::En, BuildStyle::Japanese) => "Curved pagoda eaves, engawa verandas, bamboo, sakura & andesite paths.",
        (UiLang::En, BuildStyle::Gothic) => "Pointed arches, deep slate masonry, dark oak, stained glass & soul lanterns.",
        (UiLang::En, BuildStyle::Nordic) => "Heavy spruce logs, steep A-frame roofs, tuff bricks & cobbled highland roads.",
        (UiLang::En, BuildStyle::Modern) => "Clean concrete volumes, panoramic glass, iron bars & striped asphalt roads.",
        (UiLang::En, BuildStyle::Desert) => "Carved sandstone terraces, domes, arches & terracotta-accented roads.",
        (UiLang::En, BuildStyle::Steampunk) => "Oxidized copper domes, brass pipes, mud bricks, grates & copper lanterns.",
        (UiLang::En, BuildStyle::Classical) => "Greco-Roman quartz columns, portico pediments, domes & marble avenues.",
        (UiLang::Ru, BuildStyle::Auto) => "Автоподбор архитектуры и стиля дорог по тексту описания, сиду или биому мира.",
        (UiLang::Ru, BuildStyle::Medieval) => "Средневековый фахверк, каменная кладка, брусчатка и еловые крыши.",
        (UiLang::Ru, BuildStyle::Cottage) => "Уютное дерево, кирпич, мшистый камень, тёплые садовые тропы и цветники.",
        (UiLang::Ru, BuildStyle::Japanese) => "Изогнутые кровли пагод, веранды энгава, бамбук, сакура и андезитовые аллеи.",
        (UiLang::Ru, BuildStyle::Gothic) => "Стрельчатые своды, глубинный сланец, тёмный дуб, витражи и фонари душ.",
        (UiLang::Ru, BuildStyle::Nordic) => "Северный сруб из ели, крутые двускатные крыши, туф и булыжные дороги.",
        (UiLang::Ru, BuildStyle::Modern) => "Бетон, панорамное остекление, металлические перила и асфальт с разметкой.",
        (UiLang::Ru, BuildStyle::Desert) => "Восточный песчаник, купола, террасы и дороги с терракотовым бордюром.",
        (UiLang::Ru, BuildStyle::Steampunk) => "Окисленная медь, решётки, туф, медные фонари и индустриальный шарм.",
        (UiLang::Ru, BuildStyle::Classical) => "Античный мрамор и кварцевые колоннады, портики, купола и мощёные проспекты.",
    }
}

fn roof_tooltip_desc(roof: RoofChoice, lang: UiLang) -> &'static str {
    match (lang, roof) {
        (UiLang::En, RoofChoice::Auto) => "Automatically chooses a roof geometry matching the current category and style.",
        (UiLang::En, RoofChoice::Gable) => "Classic pitched two-slope roof with sealed triangular gable ends.",
        (UiLang::En, RoofChoice::Hip) => "Four-sided sloped pyramid hip roof tapering cleanly to a ridge.",
        (UiLang::En, RoofChoice::Steep) => "High-pitched Alpine/Gothic roof with dramatic vertical silhouette.",
        (UiLang::En, RoofChoice::Pagoda) => "Flared East Asian curved eaves with upturned corner tips.",
        (UiLang::En, RoofChoice::Dome) => "Circular vaulted stone/copper dome crowned with a cupola lantern.",
        (UiLang::En, RoofChoice::Mansard) => "Two-slope French mansard crown framing spacious attic rooms.",
        (UiLang::En, RoofChoice::Flat) => "Clean parapet rooftop terrace suited for Modern and Desert builds.",
        (UiLang::Ru, RoofChoice::Auto) => "Автоматически выбирает форму крыши под текущую категорию и стиль.",
        (UiLang::Ru, RoofChoice::Gable) => "Классическая двускатная кровля с зашитыми треугольными фронтонами.",
        (UiLang::Ru, RoofChoice::Hip) => "Четырёхскатная шатровая крыша с плавным схождением к коньку.",
        (UiLang::Ru, RoofChoice::Steep) => "Крутая высокая кровля в альпийском или готическом стиле.",
        (UiLang::Ru, RoofChoice::Pagoda) => "Вогнутые восточные скаты с приподнятыми уголками карниза.",
        (UiLang::Ru, RoofChoice::Dome) => "Сводчатый купол с венчающим световым фонарём на вершине.",
        (UiLang::Ru, RoofChoice::Mansard) => "Французская мансардная крыша с просторным чердачным этажом.",
        (UiLang::Ru, RoofChoice::Flat) => "Плоская террасная крыша с парапетом для модерна и пустыни.",
    }
}

fn noise_tooltip_desc(noise: NoiseLevel, lang: UiLang) -> &'static str {
    match (lang, noise) {
        (UiLang::En, NoiseLevel::Calm) => "Clean, symmetrical walls and minimal organic wing asymmetry.",
        (UiLang::En, NoiseLevel::Balanced) => "Natural mix of wings, porches, bays & varied surface accents.",
        (UiLang::En, NoiseLevel::Wild) => "Bold asymmetric wings, towers & rich procedural silhouette variations.",
        (UiLang::Ru, NoiseLevel::Calm) => "Строгие симметричные фасады с минимальными случайными выступами.",
        (UiLang::Ru, NoiseLevel::Balanced) => "Сбалансированное сочетание флигелей, эркеров и фактуры стен.",
        (UiLang::Ru, NoiseLevel::Wild) => "Смелая асимметрия, пристройки, башенки и богатая вариативность форм.",
    }
}

fn filter_tooltip_desc(filt: MatFilter, lang: UiLang) -> &'static str {
    match (lang, filt) {
        (UiLang::En, MatFilter::All) => "Filter category: show every block matching the selected slot.",
        (UiLang::En, MatFilter::Wood) => "Filter category: logs, planks, doors, fences, bamboo & timber trim.",
        (UiLang::En, MatFilter::Stone) => "Filter category: stone bricks, deepslate, tuff, sandstone & masonry.",
        (UiLang::En, MatFilter::ColorGlass) => "Filter category: concrete, terracotta, wool & stained glass panes.",
        (UiLang::En, MatFilter::Special) => "Filter category: lanterns, leaves, grates, bars, copper & special decor.",
        (UiLang::Ru, MatFilter::All) => "Категория фильтра: показать все подходящие блоки для слота.",
        (UiLang::Ru, MatFilter::Wood) => "Категория фильтра: брёвна, доски, двери, заборы, бамбук и люки.",
        (UiLang::Ru, MatFilter::Stone) => "Категория фильтра: каменная кладка, сланец, туф, песчаник и кирпич.",
        (UiLang::Ru, MatFilter::ColorGlass) => "Категория фильтра: бетон, терракота, шерсть и цветные витражи.",
        (UiLang::Ru, MatFilter::Special) => "Категория фильтра: фонари, листва, решётки, медь и декор.",
    }
}

fn toggle_tooltip_desc(idx: usize, tri: TriToggle, lang: UiLang) -> String {
    let base = match (lang, idx) {
        (UiLang::En, 0) => "Toggle entrance porch stairs & canopy: Auto (A), Force ON (+), or OFF (-).",
        (UiLang::En, 1) => "Toggle upper-floor protruding balcony: Auto (A), Force ON (+), or OFF (-).",
        (UiLang::En, 2) => "Toggle subterranean stone cellar/basement room: Auto (A), Force ON (+), or OFF (-).",
        (UiLang::En, 3) => "Toggle market awning & roadside stall outside: Auto (A), Force ON (+), or OFF (-).",
        (UiLang::En, _) => "Toggle courtyard trees, planters, well & fences: Auto (A), Force ON (+), or OFF (-).",
        (UiLang::Ru, 0) => "Переключатель крыльца у входа: Авто (А), ВКЛ (+) или ВЫКЛ (-).",
        (UiLang::Ru, 1) => "Переключатель балкона на верхнем этаже: Авто (А), ВКЛ (+) или ВЫКЛ (-).",
        (UiLang::Ru, 2) => "Переключатель подземного каменного подвала: Авто (А), ВКЛ (+) или ВЫКЛ (-).",
        (UiLang::Ru, 3) => "Переключатель уличного навеса-лавки: Авто (А), ВКЛ (+) или ВЫКЛ (-).",
        (UiLang::Ru, _) => "Переключатель двора с деревьями, клумбами и оградой: Авто (А), ВКЛ (+) или ВЫКЛ (-).",
    };
    let state = match (lang, tri) {
        (UiLang::En, TriToggle::Auto) => "State: Auto",
        (UiLang::En, TriToggle::On) => "State: Forced ON",
        (UiLang::En, TriToggle::Off) => "State: Forced OFF",
        (UiLang::Ru, TriToggle::Auto) => "Сейчас: Авто",
        (UiLang::Ru, TriToggle::On) => "Сейчас: Включено",
        (UiLang::Ru, TriToggle::Off) => "Сейчас: Выключено",
    };
    format!("{base} {state}.")
}

fn add_tip_btn(
    w: &mut Widgets,
    tooltips: &mut Vec<TooltipEntry>,
    rect: [f32; 4],
    label: impl Into<String>,
    active: bool,
    title: impl Into<String>,
    desc: impl Into<String>,
) -> WidgetId {
    let mut b = Button::new(rect, Text::literal(label.into()));
    b.active = active;
    let id = w.add_button(b);
    let d = desc.into();
    if !d.is_empty() {
        tooltips.push(TooltipEntry {
            id,
            rect,
            title: title.into(),
            desc: d,
        });
    }
    id
}

pub(crate) fn wrap_tooltip_lines(
    text: &str,
    max_w: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if cur.is_empty() {
            if measure(word) <= max_w {
                cur.push_str(word);
            } else {
                for ch in word.chars() {
                    let mut cand = cur.clone();
                    cand.push(ch);
                    if !cur.is_empty() && measure(&cand) > max_w {
                        lines.push(std::mem::take(&mut cur));
                    }
                    cur.push(ch);
                }
            }
        } else {
            let cand = format!("{cur} {word}");
            if measure(&cand) <= max_w {
                cur = cand;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur.push_str(word);
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

pub(crate) fn clamp_tooltip_pos(
    mouse: [f32; 2],
    box_w: f32,
    box_h: f32,
    screen_size: [f32; 2],
) -> [f32; 2] {
    let margin = 4.0;
    let mut tx = mouse[0] + 9.0;
    let mut ty = mouse[1] + 11.0;
    if tx + box_w > screen_size[0] - margin {
        tx = mouse[0] - box_w - 6.0;
    }
    if ty + box_h > screen_size[1] - margin {
        ty = mouse[1] - box_h - 6.0;
    }
    let max_x = (screen_size[0] - box_w - margin).max(margin);
    let max_y = (screen_size[1] - box_h - margin).max(margin);
    [tx.clamp(margin, max_x), ty.clamp(margin, max_y)]
}

fn resolve_hovered_tooltip(tooltips: &[TooltipEntry], mouse: [f32; 2]) -> Option<&TooltipEntry> {
    tooltips.iter().rev().find(|t| point_in_rect(mouse, t.rect))
}

pub(crate) fn hover_tooltip_ready(
    hover_widget: &mut Option<WidgetId>,
    hover_since_ms: &mut u64,
    hovered_id: Option<WidgetId>,
    now_ms: u64,
) -> bool {
    let Some(id) = hovered_id else {
        *hover_widget = None;
        return false;
    };
    if *hover_widget != Some(id) {
        *hover_widget = Some(id);
        *hover_since_ms = now_ms;
        return false;
    }
    now_ms.saturating_sub(*hover_since_ms) >= HOVER_TOOLTIP_DELAY_MS
}

fn draw_tooltip_popup(
    draw: &mut dyn mod_api::DrawContext,
    mouse: [f32; 2],
    screen_size: [f32; 2],
    title: &str,
    desc: &str,
) {
    let max_text_w = (screen_size[0] - 24.0).clamp(110.0, 196.0);
    let body_lines = wrap_tooltip_lines(desc, max_text_w, |s| draw.text_width(s));
    let mut content_w = if title.is_empty() {
        0.0
    } else {
        draw.text_width(title) + 8.0
    };
    for line in &body_lines {
        content_w = content_w.max(draw.text_width(line));
    }
    if content_w <= 1.0 {
        return;
    }
    let has_title = !title.is_empty();
    let title_h = if has_title { 13.0 } else { 0.0 };
    let line_h = 10.0;
    let box_w = (content_w + 12.0).min(screen_size[0] - 8.0);
    let box_h = 8.0 + title_h + (body_lines.len() as f32) * line_h;
    let [tx, ty] = clamp_tooltip_pos(mouse, box_w, box_h, screen_size);

    draw.fill([tx + 2.0, ty + 2.0, box_w, box_h], 0x7702_0617);
    draw.fill([tx, ty, box_w, box_h], 0xF20B_1120);
    draw.fill([tx, ty, box_w, 1.0], 0xFF38_BDF8);
    draw.fill([tx, ty + box_h - 1.0, box_w, 1.0], 0xFF38_BDF8);
    draw.fill([tx, ty, 1.0, box_h], 0xFF38_BDF8);
    draw.fill([tx + box_w - 1.0, ty, 1.0, box_h], 0xFF38_BDF8);

    let mut cur_y = ty + 4.5;
    if has_title {
        draw.text(title, [tx + 6.0, cur_y], 0xFF38_BDF8, true);
        draw.fill([tx + 5.0, cur_y + 10.0, box_w - 10.0, 1.0], 0x5538_BDF8);
        cur_y += title_h;
    }
    for line in &body_lines {
        draw.text(line, [tx + 6.0, cur_y], 0xFFE2_E8F0, false);
        cur_y += line_h;
    }
}

const QUICK_TAGS_EN: [(&str, &str); 8] = [
    ("+2 Floors", "two-story"),
    ("+Cellar", "with basement"),
    ("+Hearth", "with fireplace"),
    ("+Gothic", "gothic with tower"),
    ("+Japan", "japanese pagoda with cherry"),
    ("+Steam", "steampunk with copper"),
    ("+Tavern", "cozy tavern with porch and garden"),
    ("+Beacon", "tall lighthouse with lantern"),
];

const QUICK_TAGS_RU: [(&str, &str); 8] = [
    ("+2 этажа", "двухэтажный"),
    ("+Подвал", "с подвалом"),
    ("+Камин", "с камином"),
    ("+Готика", "готический с башней"),
    ("+Япония", "японский пагода с сакурой"),
    ("+Стимпанк", "стимпанк с медью"),
    ("+Таверна", "уютная таверна с крыльцом и садом"),
    ("+Маяк", "высокий маяк с фонарём"),
];

fn quick_tags(lang: UiLang) -> &'static [(&'static str, &'static str); 8] {
    match lang {
        UiLang::En => &QUICK_TAGS_EN,
        UiLang::Ru => &QUICK_TAGS_RU,
    }
}

fn append_tag_to_description(st: &mut ClientUiState, phrase: &str) {
    if !st.description.to_lowercase().contains(phrase) {
        if !st.description.trim().is_empty() {
            st.description.push_str(", ");
        }
        st.description.push_str(phrase);
    }
}

use crate::server_builder::{CLI_PRESETS, CLI_PRESET_DESCS_EN, cli_preset_desc};

fn sync_palette_from_ui(st: &mut ClientUiState) {
    if !st.palette_manual {
        let biome = if st.use_biome_auto {
            st.detected_biome.as_str()
        } else {
            ""
        };
        st.palette = crate::parser::infer_palette_with_biome(
            st.style,
            &effective_description(st),
            st.seed,
            biome,
        );
    }
}

fn next_preset_cat_filter(cur: Option<BuildCategory>) -> Option<BuildCategory> {
    match cur {
        None => Some(BuildCategory::ALL[0]),
        Some(cat) => {
            let pos = BuildCategory::ALL.iter().position(|&c| c == cat).unwrap_or(0);
            if pos + 1 < BuildCategory::ALL.len() {
                Some(BuildCategory::ALL[pos + 1])
            } else {
                None
            }
        }
    }
}

fn filtered_preset_indices(cat_filter: Option<BuildCategory>, query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();
    CLI_PRESETS
        .iter()
        .enumerate()
        .filter(|&(i, &(slug, cat, style, _, _, _, desc_ru))| {
            if let Some(req_cat) = cat_filter
                && cat != req_cat
            {
                return false;
            }
            if terms.is_empty() {
                return true;
            }
            let desc_en = CLI_PRESET_DESCS_EN[i];
            let hay = format!(
                "{slug} {} {} {} {} {} {desc_ru} {desc_en}",
                cat.label(UiLang::En).to_lowercase(),
                cat.label_ru().to_lowercase(),
                cat.slug(),
                style.label(UiLang::En).to_lowercase(),
                style.label_ru().to_lowercase(),
            );
            terms.iter().all(|t| hay.contains(t))
        })
        .map(|(i, _)| i)
        .collect()
}

fn preset_card_label(preset_idx: usize, active_idx: usize, lang: UiLang) -> String {
    let (_, _, _, w, d, _, _) = CLI_PRESETS[preset_idx];
    let desc = cli_preset_desc(preset_idx, lang);
    let mark = if preset_idx == active_idx { "✓ " } else { "" };
    let words: String = desc
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|s| !s.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let mut short: String = words.chars().take(21).collect();
    if short.is_empty() {
        short = CLI_PRESETS[preset_idx].0.to_owned();
    }
    format!("{mark}{}. {short} ({w}x{d})", preset_idx + 1)
}

fn apply_preset_by_idx(st: &mut ClientUiState, raw_idx: usize) {
    let len = CLI_PRESETS.len();
    let idx = raw_idx % len;
    st.archetype_idx = idx;
    let (_, cat, style, w, d, h, _) = CLI_PRESETS[idx];
    let desc = cli_preset_desc(idx, st.lang);
    st.category = cat;
    st.style = style;
    st.roof_choice = RoofChoice::Auto;
    st.toggles = [TriToggle::Auto; 5];
    st.width = w;
    st.depth = d;
    st.height = h;
    st.description = desc.to_owned();
    st.palette_manual = false;
    sync_palette_from_ui(st);
    st.preview_key = None;
    st.status = match st.lang {
        UiLang::En => format!("Structure {}/{}: {} ({}x{}x{})", idx + 1, len, cat.label(UiLang::En), w, d, h),
        UiLang::Ru => format!("Сооружение {}/{}: {} ({}x{}x{})", idx + 1, len, cat.label_ru(), w, d, h),
    };
}

fn apply_prev_archetype(st: &mut ClientUiState) {
    let len = CLI_PRESETS.len();
    let prev = (st.archetype_idx + len - 1) % len;
    apply_preset_by_idx(st, prev);
}

fn apply_next_archetype(st: &mut ClientUiState) {
    let next = (st.archetype_idx + 1) % CLI_PRESETS.len();
    apply_preset_by_idx(st, next);
}

fn build_screen(st: &ClientUiState, size: [f32; 2]) -> BuiltScreen {
    let lang = st.lang;
    let pw = (size[0] - 16.0).clamp(396.0, 504.0);
    let ph = (size[1] - 12.0).clamp(264.0, 304.0);
    let px = ((size[0] - pw) * 0.5).floor();
    let py = ((size[1] - ph) * 0.5).floor();
    let panel = [px, py, pw, ph];

    let mut w = Widgets::new();
    let mut tooltips: Vec<TooltipEntry> = Vec::new();
    let pad = 10.0;
    let inner_w = pw - pad * 2.0;

    let bom_lbl = match lang {
        UiLang::En => format!("Blocks: {}", st.preview_total_blocks),
        UiLang::Ru => format!("Смета: {}", st.preview_total_blocks),
    };
    let bom_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pw - pad - 184.0, py + 2.0, 82.0, 16.0],
        bom_lbl,
        true,
        tr(lang, "Material Estimate (BOM)", "Смета блоков (BOM)"),
        tr(
            lang,
            "Open detailed block count breakdown with stacks and clipboard export.",
            "Открыть смету блоков с подсчётом стаков и копированием списка.",
        ),
    );
    let lang_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pw - pad - 98.0, py + 2.0, 56.0, 16.0],
        lang.button_label(),
        true,
        tr(lang, "Interface Language", "Язык интерфейса"),
        tr(
            lang,
            "Switch GUI labels, presets & /buildgen messages between English and Russian.",
            "Переключить язык меню, каталога и команд /buildgen (EN / RU).",
        ),
    );

    let mut tab_size = None;
    let mut tab_desc = None;
    let mut tab_mats = None;
    let mut tab_world = None;
    let tab_y = py + 24.0;
    let body_y = tab_y + 24.0;

    if !st.picker_open && !st.preset_picker_open && !st.bom_open {
        let tab_w = ((inner_w - 12.0) / 4.0).floor();
        let tab_w4 = inner_w - (tab_w + 4.0) * 3.0;
        tab_size = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [px + pad, tab_y, tab_w, 18.0],
            tr(lang, "1. Size & Area", "1. Размер"),
            st.tab != UiTab::SizeArea,
            tr(lang, "Tab 1: Size & Area", "Вкладка 1: Размер и область"),
            tr(
                lang,
                "Area dimensions (7-96), world selection, 3D hologram, cutaway slice, favorites & BG2 codes.",
                "Габариты (7-96), выделение области, 3D-голограмма, срез, избранное и коды BG2.",
            ),
        ));
        tab_desc = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [px + pad + tab_w + 4.0, tab_y, tab_w, 18.0],
            tr(lang, "2. Style & Noise", "2. Стиль"),
            st.tab != UiTab::DescNoise,
            tr(lang, "Tab 2: Style & Catalog", "Вкладка 2: Стиль и каталог"),
            tr(
                lang,
                "Categories, styles, roof profile, text prompt, seed, 88 presets, biome, wear & moss.",
                "Категории, стили, форма крыши, описание, сид, 88 пресетов, биом, износ и зелень.",
            ),
        ));
        tab_mats = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [px + pad + (tab_w + 4.0) * 2.0, tab_y, tab_w, 18.0],
            tr(lang, "3. Materials", "3. Материалы"),
            st.tab != UiTab::Materials,
            tr(lang, "Tab 3: Palette & Mix", "Вкладка 3: Палитра и микс"),
            tr(
                lang,
                "Architectural palette presets, 30 building slots, mixing modes & custom block lists.",
                "Пресеты палитр, 30 слотов архитектуры, режимы смешивания и списки блоков.",
            ),
        ));
        tab_world = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [px + pad + (tab_w + 4.0) * 3.0, tab_y, tab_w4, 18.0],
            tr(lang, "4. World & Decor", "4. Мир и Декор"),
            st.tab != UiTab::WorldDecor,
            tr(lang, "Tab 4: World & Decor", "Вкладка 4: Мир и декор"),
            tr(
                lang,
                "Living Interiors 2.0, NPC/livestock spawning, terrain smoothing & 2-point roads.",
                "Живые интерьеры 2.0, спавн NPC/животных, сглаживание краёв и дороги по 2 точкам.",
            ),
        ));
    }

    let mut w_dec = None;
    let mut w_inc = None;
    let mut d_dec = None;
    let mut d_inc = None;
    let mut h_dec = None;
    let mut h_inc = None;
    let mut preset_small = None;
    let mut preset_med = None;
    let mut preset_large = None;
    let mut preset_tower = None;
    let mut place_front = None;
    let mut pick_world = None;
    let mut rotate_btn = None;
    let mut y_dec_btn = None;
    let mut y_inc_btn = None;
    let mut terrain_btn = None;
    let mut preview_btn = None;
    let mut air_only_btn = None;
    let mut slice_toggle_btn = None;
    let mut slice_dec_btn = None;
    let mut slice_inc_btn = None;
    let mut fav_save_btn = None;
    let mut fav_load_btn = None;
    let mut fav_del_btn = None;
    let mut share_copy_btn = None;
    let mut share_paste_btn = None;

    let mut cat_btn = None;
    let mut style_btn = None;
    let mut frame_btn = None;
    let mut collision_btn = None;
    let mut move_preview_btn = None;
    let mut auto_hide_btn = None;
    let mut roof_btn = None;
    let mut desc_field = None;
    let mut seed_field = None;
    let mut prev_seed_btn = None;
    let mut new_seed_btn = None;
    let mut auto_seed_btn = None;
    let mut archetype_prev_btn = None;
    let mut archetype_btn = None;
    let mut archetype_next_btn = None;
    let mut noise_btn = None;
    let mut biome_btn = None;
    let mut wear_dec_btn = None;
    let mut wear_btn = None;
    let mut wear_inc_btn = None;
    let mut over_dec_btn = None;
    let mut over_btn = None;
    let mut over_inc_btn = None;
    let mut tag_buttons = Vec::new();
    let mut feature_toggles = Vec::new();

    let mut preset_back_btn = None;
    let mut preset_cat_btn = None;
    let mut preset_search_field = None;
    let mut preset_search_clear_btn = None;
    let mut preset_up_btn = None;
    let mut preset_down_btn = None;
    let mut preset_grid_btns = Vec::new();

    let mut bom_back_btn = None;
    let mut bom_copy_btn = None;
    let mut bom_up_btn = None;
    let mut bom_down_btn = None;

    let mut mat_preset_btn = None;
    let mut mat_infer_btn = None;
    let mut mat_sync_wood_btn = None;
    let mut mat_mix_mode_btn = None;
    let mut mat_slot_up_btn = None;
    let mut mat_slot_down_btn = None;
    let mut mat_slot_btns = Vec::new();
    let mut list_clear_btn = None;
    let mut list_up_btn = None;
    let mut list_down_btn = None;
    let mut list_item_btns = Vec::new();
    let mut list_remove_btns = Vec::new();
    let mut picker_add_open_btn = None;
    let mut list_pop_btn = None;

    let mut picker_back_btn = None;
    let mut picker_mode_btn = None;
    let mut picker_catalog_btn = None;
    let mut mat_filter_btns = Vec::new();
    let mut mat_search_field = None;
    let mut mat_search_clear_btn = None;
    let mut picker_up_btn = None;
    let mut picker_down_btn = None;
    let mut picker_grid_btns = Vec::new();
    let mut picker_chip_btns = Vec::new();

    let mut living_decor_btn = None;
    let mut spawn_npcs_btn = None;
    let mut blend_radius_btn = None;
    let mut road_width_btn = None;
    let mut road_auto_bridge_btn = None;
    let mut road_style_btn = None;
    let mut road_connect_btn = None;

    let mut slot_scroll_zone = None;
    let mut list_scroll_zone = None;
    let mut picker_scroll_zone = None;
    let mut preset_scroll_zone = None;
    let mut bom_scroll_zone = None;
    let mut tab3_slot_rows = Vec::new();
    let mut tab3_list_rows = Vec::new();
    let mut tab3_header_pos = None;
    let mut picker_tiles = Vec::new();
    let mut picker_chips = Vec::new();
    let mut picker_strip_pos = None;
    let mut bom_rows = Vec::new();

    let foot_y = py + ph - 30.0;

    if st.picker_open {
        let lx = px + pad;
        let r1_y = py + 23.0;
        let back_w = 64.0;
        let mode_w = (inner_w * 0.28).floor();
        let cat_w = (inner_w * 0.30).floor();
        let mix_w = inner_w - back_w - mode_w - cat_w - 9.0;

        picker_back_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx, r1_y, back_w, 17.0],
            tr(lang, "← Back", "← Назад"),
            true,
            tr(lang, "Back to Materials", "Назад к материалам"),
            tr(
                lang,
                "Close the block picker and return to architectural slots.",
                "Закрыть окно выбора блоков и вернуться ко вкладке материалов.",
            ),
        ));
        let mode_lbl = if let Some(idx) = st.picker_replace_idx {
            match lang {
                UiLang::En => format!("Mode: Replace #{}", idx + 1),
                UiLang::Ru => format!("Режим: Замена #{}", idx + 1),
            }
        } else {
            tr(lang, "Mode: + Add to list", "Режим: + В список").to_owned()
        };
        picker_mode_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + back_w + 3.0, r1_y, mode_w, 17.0],
            mode_lbl,
            true,
            tr(lang, "Selection Mode", "Режим выбора"),
            tr(
                lang,
                "Switch between adding blocks to the slot list or replacing a chosen block chip.",
                "Переключение между добавлением блоков в список и заменой выбранного элемента.",
            ),
        ));
        let cat_lbl = if st.picker_all_blocks {
            if st.world_blocks.is_empty() {
                tr(lang, "Catalog: All", "Каталог: Все").to_owned()
            } else {
                match lang {
                    UiLang::En => format!("Catalog: All ({})", st.world_blocks.len()),
                    UiLang::Ru => format!("Каталог: Все ({})", st.world_blocks.len()),
                }
            }
        } else {
            tr(lang, "Catalog: Rec.", "Каталог: Рекоменд.").to_owned()
        };
        picker_catalog_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + back_w + mode_w + 6.0, r1_y, cat_w, 17.0],
            cat_lbl,
            true,
            tr(lang, "Block Source Catalog", "Источник блоков"),
            tr(
                lang,
                "Toggle between curated recommended blocks for this slot and all registered world blocks.",
                "Переключает список между рекомендованными блоками слота и всеми блоками игры.",
            ),
        ));
        mat_mix_mode_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + back_w + mode_w + cat_w + 9.0, r1_y, mix_w, 17.0],
            match lang {
                UiLang::En => format!("Mix: {}", st.palette.mix_mode.label(lang)),
                UiLang::Ru => format!("Микс: {}", st.palette.mix_mode.label(lang)),
            },
            true,
            tr(lang, "Multi-Block Mix Pattern", "Алгоритм смешивания"),
            tr(
                lang,
                "Cycles how multiple blocks in one slot blend: Patches, Scatter, Horizontal Stripes, or Checker.",
                "Режим чередования блоков внутри слота: Пятна, Россыпь, Полосы или Шахматка.",
            ),
        ));

        // Row 2: Filters (left 54%) + Search box (right 46%)
        let r2_y = r1_y + 20.0;
        let filters_w = (inner_w * 0.54).floor();
        let fw = ((filters_w - 8.0) / 5.0).floor();
        for (fi, &filt) in MatFilter::ALL.iter().enumerate() {
            let id = add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx + fi as f32 * (fw + 2.0), r2_y, fw, 15.0],
                filt.label(lang),
                st.mat_filter != filt,
                filt.label(lang),
                filter_tooltip_desc(filt, lang),
            );
            mat_filter_btns.push((id, filt));
        }

        let sx = lx + filters_w + 6.0;
        let sw = inner_w - filters_w - 6.0;
        let clr_w = 17.0;
        let mut sf = TextField::new([sx, r2_y, sw - clr_w - 2.0, 15.0], 32);
        sf.set_value(&st.mat_search);
        sf.suggestion = Some(tr(lang, "search EN/RU: oak, glass...", "поиск RU/EN: сланец, glass...").into());
        mat_search_field = Some(w.add_text_field(sf));
        mat_search_clear_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sx + sw - clr_w, r2_y, clr_w, 15.0],
            "✕",
            !st.mat_search.is_empty(),
            tr(lang, "Clear Search", "Сброс поиска"),
            tr(
                lang,
                "Clear block search filter text.",
                "Очистить строку поиска по блокам.",
            ),
        ));

        // Bottom active slot block list strip above status & footer
        let strip_y = foot_y - 43.0;
        picker_strip_pos = Some([lx, strip_y]);
        let chip_y = strip_y + 11.0;
        let slot_blocks = st.palette.slot_blocks(st.selected_mat_slot);
        let reset_w = 68.0;
        let chips_area_w = inner_w - reset_w - 4.0;
        let max_chips = 5usize;
        let show_cnt = slot_blocks.len().min(max_chips);
        let chip_slot_w = ((chips_area_w - (max_chips.saturating_sub(1) as f32) * 3.0)
            / max_chips as f32)
            .floor();
        for idx in 0..show_cnt {
            let bid = &slot_blocks[idx];
            let cx = lx + (idx as f32) * (chip_slot_w + 3.0);
            picker_chips.push((idx, bid.clone(), [cx, chip_y, chip_slot_w, 16.0]));
            let short = short_block_name(bid);
            let lbl = if st.picker_replace_idx == Some(idx) {
                format!("▸{}.{}", idx + 1, short)
            } else if slot_blocks.len() > 1 {
                format!("{}.{} ✕", idx + 1, short)
            } else {
                format!("{}.{}", idx + 1, short)
            };
            let btn = w.add_button(Button::new(
                [cx + 14.0, chip_y, chip_slot_w - 14.0, 16.0],
                Text::literal(lbl),
            ));
            picker_chip_btns.push((btn, idx));
        }
        list_clear_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + chips_area_w + 4.0, chip_y, reset_w, 16.0],
            tr(lang, "Reset to 1", "Сброс до 1"),
            slot_blocks.len() > 1,
            tr(lang, "Reset Slot List", "Сбросить список"),
            tr(
                lang,
                "Keep only the primary block in this slot and remove extra variants.",
                "Оставить в слоте только главный блок и удалить дополнительные варианты.",
            ),
        ));

        // Main 3-col scrollable block icon grid (NO tooltips on block tiles)
        let grid_y = r2_y + 19.0;
        let grid_avail_h = (strip_y - 3.0 - grid_y).max(74.0);
        let row_h = 18.0;
        let vis_rows = ((grid_avail_h / row_h).floor() as usize).clamp(4, 9);
        let sb_w = 10.0;
        let grid_inner_w = inner_w - sb_w - 3.0;
        let tile_gap = 3.0;
        let tile_w = ((grid_inner_w - ((PICKER_COLS - 1) as f32) * tile_gap)
            / PICKER_COLS as f32)
            .floor();

        let choices = current_picker_choices(st);
        let total_rows = choices.len().div_ceil(PICKER_COLS).max(1);
        let max_scroll = total_rows.saturating_sub(vis_rows);
        let scroll = st.picker_scroll.min(max_scroll);

        for r in 0..vis_rows {
            for c in 0..PICKER_COLS {
                let g_idx = (scroll + r) * PICKER_COLS + c;
                let tx = lx + (c as f32) * (tile_w + tile_gap);
                let ty = grid_y + (r as f32) * row_h;
                if let Some(cand_id) = choices.get(g_idx) {
                    let in_list = st.palette.slot_contains(st.selected_mat_slot, cand_id);
                    let short = short_block_name(cand_id);
                    let lbl = if in_list {
                        format!("✓ {short}")
                    } else {
                        short.to_owned()
                    };
                    let tile_rect = [tx, ty, tile_w, 16.8];
                    picker_tiles.push((cand_id.clone(), in_list, tile_rect));
                    let btn = w.add_button(Button::new(
                        [tx + 15.0, ty, tile_w - 15.0, 16.8],
                        Text::literal(lbl),
                    ));
                    picker_grid_btns.push((btn, cand_id.clone()));
                } else {
                    let tile_rect = [tx, ty, tile_w, 16.8];
                    picker_tiles.push((String::new(), false, tile_rect));
                    let mut btn_spec = Button::new(
                        [tx + 15.0, ty, tile_w - 15.0, 16.8],
                        Text::literal(""),
                    );
                    btn_spec.active = false;
                    picker_grid_btns.push((w.add_button(btn_spec), String::new()));
                }
            }
        }

        let sb_x = lx + grid_inner_w + 3.0;
        let sb_h = (vis_rows as f32) * row_h - 1.0;
        picker_up_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y, sb_w, 12.0],
            "▲",
            scroll > 0,
            tr(lang, "Scroll Up", "Вверх"),
            tr(lang, "Scroll picker grid up.", "Прокрутить список вверх."),
        ));
        picker_down_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y + sb_h - 12.0, sb_w, 12.0],
            "▼",
            scroll < max_scroll,
            tr(lang, "Scroll Down", "Вниз"),
            tr(lang, "Scroll picker grid down.", "Прокрутить список вниз."),
        ));
        picker_scroll_zone = Some(ScrollZone {
            viewport: [lx, grid_y, inner_w, sb_h],
            track: [sb_x, grid_y + 13.0, sb_w, (sb_h - 26.0).max(8.0)],
            total_rows,
            vis_rows,
            scroll,
        });
    } else if st.preset_picker_open {
        let lx = px + pad;
        let r1_y = py + 23.0;
        let back_w = 60.0;
        let cat_w = 104.0;
        let clr_w = 17.0;
        let search_x = lx + back_w + cat_w + 6.0;
        let search_w = (inner_w - back_w - cat_w - clr_w - 10.0).max(80.0);

        preset_back_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx, r1_y, back_w, 17.0],
            tr(lang, "← Back", "← Назад"),
            true,
            tr(lang, "Back to Style Tab", "Назад в меню"),
            tr(
                lang,
                "Close the preset catalog and keep current settings.",
                "Закрыть каталог пресетов без сброса настроек.",
            ),
        ));
        let cat_lbl = match (lang, st.preset_cat_filter) {
            (UiLang::En, None) => "Type: All".to_owned(),
            (UiLang::Ru, None) => "Тип: Все".to_owned(),
            (UiLang::En, Some(c)) => format!("Type: {}", c.label(lang)),
            (UiLang::Ru, Some(c)) => format!("Тип: {}", c.label(lang)),
        };
        let cat_tip = if let Some(c) = st.preset_cat_filter {
            format!(
                "{} — {}",
                c.label(lang),
                category_tooltip_desc(c, lang)
            )
        } else {
            tr(
                lang,
                "Filter presets across all 14 architectural categories (click to cycle).",
                "Фильтр по всем 14 категориям построек (нажмите для перебора).",
            )
            .to_owned()
        };
        preset_cat_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + back_w + 3.0, r1_y, cat_w, 17.0],
            cat_lbl,
            true,
            tr(lang, "Category Filter", "Фильтр категорий"),
            cat_tip,
        ));
        let mut psf = TextField::new([search_x, r1_y, search_w, 17.0], 40);
        psf.set_value(&st.preset_search);
        psf.suggestion = Some(tr(lang, "search: bridge, castle, ship...", "поиск: мост, замок, храм, ship...").into());
        preset_search_field = Some(w.add_text_field(psf));
        preset_search_clear_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [search_x + search_w + 3.0, r1_y, clr_w, 17.0],
            "✕",
            !st.preset_search.is_empty(),
            tr(lang, "Clear Filter", "Сброс поиска"),
            tr(lang, "Clear preset search query.", "Очистить строку поиска по каталогу."),
        ));

        let grid_y = r1_y + 21.0;
        let grid_avail_h = (foot_y - 26.0 - grid_y).max(96.0);
        let row_h = 19.0;
        let vis_rows = ((grid_avail_h / row_h).floor() as usize).clamp(5, 8);
        let sb_w = 10.0;
        let grid_inner_w = inner_w - sb_w - 3.0;
        let col_gap = 4.0;
        let col_w = ((grid_inner_w - col_gap) * 0.5).floor();

        let matches = filtered_preset_indices(st.preset_cat_filter, &st.preset_search);
        let total_rows = matches.len().div_ceil(2).max(1);
        let max_scroll = total_rows.saturating_sub(vis_rows);
        let scroll = st.preset_scroll.min(max_scroll);
        let active_idx = st.archetype_idx % CLI_PRESETS.len();

        for r in 0..vis_rows {
            for c in 0..2usize {
                let m_idx = (scroll + r) * 2 + c;
                let tx = lx + (c as f32) * (col_w + col_gap);
                let ty = grid_y + (r as f32) * row_h;
                if let Some(&preset_idx) = matches.get(m_idx) {
                    let (slug, cat, style, pw_u, pd_u, ph_u, _) = CLI_PRESETS[preset_idx];
                    let btn = add_tip_btn(
                        &mut w,
                        &mut tooltips,
                        [tx, ty, col_w, 17.5],
                        preset_card_label(preset_idx, active_idx, lang),
                        true,
                        format!("#{} {slug} ({pw_u}x{pd_u}x{ph_u})", preset_idx + 1),
                        format!(
                            "{} · {}: {}",
                            cat.label(lang),
                            style.label(lang),
                            cli_preset_desc(preset_idx, lang)
                        ),
                    );
                    preset_grid_btns.push((btn, preset_idx));
                } else {
                    let mut btn_spec = Button::new([tx, ty, col_w, 17.5], Text::literal(""));
                    btn_spec.active = false;
                    preset_grid_btns.push((w.add_button(btn_spec), usize::MAX));
                }
            }
        }

        let sb_x = lx + grid_inner_w + 3.0;
        let sb_h = (vis_rows as f32) * row_h - 1.5;
        preset_up_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y, sb_w, 12.0],
            "▲",
            scroll > 0,
            tr(lang, "Scroll Up", "Вверх"),
            tr(lang, "Scroll preset catalog up.", "Прокрутить каталог пресетов вверх."),
        ));
        preset_down_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y + sb_h - 12.0, sb_w, 12.0],
            "▼",
            scroll < max_scroll,
            tr(lang, "Scroll Down", "Вниз"),
            tr(lang, "Scroll preset catalog down.", "Прокрутить каталог пресетов вниз."),
        ));
        preset_scroll_zone = Some(ScrollZone {
            viewport: [lx, grid_y, inner_w, sb_h],
            track: [sb_x, grid_y + 13.0, sb_w, (sb_h - 26.0).max(8.0)],
            total_rows,
            vis_rows,
            scroll,
        });
    } else if st.bom_open {
        let lx = px + pad;
        let r1_y = py + 23.0;
        let back_w = 64.0;
        let copy_w = 108.0;
        bom_back_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx, r1_y, back_w, 17.0],
            tr(lang, "← Back", "← Назад"),
            true,
            tr(lang, "Close Estimate", "Закрыть смету"),
            tr(lang, "Return to main Building Generator menu.", "Вернуться в главное меню генератора."),
        ));
        bom_copy_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [lx + inner_w - copy_w, r1_y, copy_w, 17.0],
            tr(lang, "Copy List", "Копир. смету"),
            true,
            tr(lang, "Copy Estimate", "Копировать смету"),
            tr(
                lang,
                "Copy full block count breakdown with stacks to your clipboard.",
                "Скопировать список всех требуемых блоков со стаками в буфер обмена.",
            ),
        ));

        let grid_y = r1_y + 22.0;
        let grid_avail_h = (foot_y - 26.0 - grid_y).max(90.0);
        let row_h = 18.0;
        let vis_rows = ((grid_avail_h / row_h).floor() as usize).clamp(5, 8);
        let sb_w = 10.0;
        let grid_inner_w = inner_w - sb_w - 3.0;
        let col_gap = 6.0;
        let col_w = ((grid_inner_w - col_gap) * 0.5).floor();

        let total_rows = st.preview_bom.len().div_ceil(2).max(1);
        let max_scroll = total_rows.saturating_sub(vis_rows);
        let scroll = st.bom_scroll.min(max_scroll);

        for r in 0..vis_rows {
            for c in 0..2usize {
                let idx = (scroll + r) * 2 + c;
                if let Some((bid, cnt)) = st.preview_bom.get(idx) {
                    let tx = lx + (c as f32) * (col_w + col_gap);
                    let ty = grid_y + (r as f32) * row_h;
                    bom_rows.push((bid.clone(), *cnt, [tx, ty, col_w, 16.5]));
                }
            }
        }

        let sb_x = lx + grid_inner_w + 3.0;
        let sb_h = (vis_rows as f32) * row_h - 1.5;
        bom_up_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y, sb_w, 12.0],
            "▲",
            scroll > 0,
            tr(lang, "Scroll Up", "Вверх"),
            tr(lang, "Scroll block estimate list up.", "Прокрутить смету блоков вверх."),
        ));
        bom_down_btn = Some(add_tip_btn(
            &mut w,
            &mut tooltips,
            [sb_x, grid_y + sb_h - 12.0, sb_w, 12.0],
            "▼",
            scroll < max_scroll,
            tr(lang, "Scroll Down", "Вниз"),
            tr(lang, "Scroll block estimate list down.", "Прокрутить смету блоков вниз."),
        ));
        bom_scroll_zone = Some(ScrollZone {
            viewport: [lx, grid_y, inner_w, sb_h],
            track: [sb_x, grid_y + 13.0, sb_w, (sb_h - 26.0).max(8.0)],
            total_rows,
            vis_rows,
            scroll,
        });
    } else {
    match st.tab {
        UiTab::SizeArea => {
            let col_w = ((inner_w - 10.0) * 0.5).floor();
            let lx = px + pad;
            let rx = px + pad + col_w + 10.0;

            // Left column: W, D, H steppers + Presets + Disk Favorites + Share Code
            w_dec = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 86.0, body_y, 22.0, 18.0], "-", true, tr(lang, "Decrease Width", "Уменьшить ширину"), tr(lang, "Shrink X width by 2 blocks (min 7).", "Уменьшить ширину (X) на 2 блока (мин. 7).")));
            w_inc = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 144.0, body_y, 22.0, 18.0], "+", true, tr(lang, "Increase Width", "Увеличить ширину"), tr(lang, "Expand X width by 2–4 blocks (max 96).", "Увеличить ширину (X) на 2–4 блока (макс. 96).")));

            d_dec = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 86.0, body_y + 22.0, 22.0, 18.0], "-", true, tr(lang, "Decrease Depth", "Уменьшить глубину"), tr(lang, "Shrink Z depth by 2 blocks (min 7).", "Уменьшить глубину (Z) на 2 блока (мин. 7).")));
            d_inc = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 144.0, body_y + 22.0, 22.0, 18.0], "+", true, tr(lang, "Increase Depth", "Увеличить глубину"), tr(lang, "Expand Z depth by 2–4 blocks (max 96).", "Увеличить глубину (Z) на 2–4 блока (макс. 96).")));

            h_dec = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 86.0, body_y + 44.0, 22.0, 18.0], "-", true, tr(lang, "Decrease Height", "Уменьшить высоту"), tr(lang, "Lower Y height limit by 1 block (min 5).", "Уменьшить высоту (Y) на 1 блок (мин. 5).")));
            h_inc = Some(add_tip_btn(&mut w, &mut tooltips, [lx + 144.0, body_y + 44.0, 22.0, 18.0], "+", true, tr(lang, "Increase Height", "Увеличить высоту"), tr(lang, "Raise Y height limit by 1 block (max 48).", "Увеличить высоту (Y) на 1 блок (макс. 48).")));

            let half_p = ((col_w - 4.0) * 0.5).floor();
            preset_small = Some(add_tip_btn(&mut w, &mut tooltips, [lx, body_y + 68.0, half_p, 17.0], tr(lang, "Small 9x9x8", "Малый 9x9x8"), true, tr(lang, "Compact Footprint", "Компактный размер"), tr(lang, "Set area dimensions to 9x9x8 (starter cabin, stall or hut).", "Установить габариты 9x9x8 (небольшой дом или лавка).")));
            preset_med = Some(add_tip_btn(&mut w, &mut tooltips, [lx + half_p + 4.0, body_y + 68.0, half_p, 17.0], tr(lang, "Med 13x11x12", "Средн. 13x11x12"), true, tr(lang, "Standard House Size", "Стандартный размер"), tr(lang, "Set area dimensions to 13x11x12 (balanced house or shop).", "Установить габариты 13x11x12 (средний жилой дом).")));
            preset_large = Some(add_tip_btn(&mut w, &mut tooltips, [lx, body_y + 88.0, half_p, 17.0], tr(lang, "Large 19x15x16", "Больш. 19x15x16"), true, tr(lang, "Grand Hall Footprint", "Крупное сооружение"), tr(lang, "Set area dimensions to 19x15x16 (mansion, temple or castle).", "Установить габариты 19x15x16 (особняк, храм или замок).")));
            preset_tower = Some(add_tip_btn(&mut w, &mut tooltips, [lx + half_p + 4.0, body_y + 88.0, half_p, 17.0], tr(lang, "Tower 9x9x22", "Башня 9x9x22"), true, tr(lang, "Tall Tower Size", "Высокая башня"), tr(lang, "Set area dimensions to 9x9x22 (pagoda, spire or lighthouse).", "Установить габариты 9x9x22 (высокая башня, маяк или пагода).")));

            let del_w = 22.0;
            let rem_fav = col_w - del_w - 6.0;
            let fav_s_w = (rem_fav * 0.44).floor();
            let fav_l_w = rem_fav - fav_s_w;
            fav_save_btn = Some(add_tip_btn(&mut w, &mut tooltips, [lx, body_y + 109.0, fav_s_w, 16.0], tr(lang, "★ Save", "★ Сохр."), true, tr(lang, "Save Favorite", "В избранное"), tr(lang, "Save current size, style, prompt, wear & palette to favorites.txt.", "Сохранить текущие параметры, стиль и палитру в favorites.txt.")));
            let fav_lbl = if st.favorites.is_empty() {
                tr(lang, "Favs (0)", "Избр. (0)").into()
            } else {
                match lang {
                    UiLang::En => format!("Favs {}/{}", (st.fav_idx % st.favorites.len()) + 1, st.favorites.len()),
                    UiLang::Ru => format!("Избр. {}/{}", (st.fav_idx % st.favorites.len()) + 1, st.favorites.len()),
                }
            };
            fav_load_btn = Some(add_tip_btn(&mut w, &mut tooltips, [lx + fav_s_w + 3.0, body_y + 109.0, fav_l_w, 16.0], fav_lbl, true, tr(lang, "Cycle Favorites", "Загрузить избранное"), tr(lang, "Cycle and load saved presets from favorites.txt.", "Переключать и применять сохранённые пресеты из favorites.txt.")));
            fav_del_btn = Some(add_tip_btn(&mut w, &mut tooltips, [lx + fav_s_w + fav_l_w + 6.0, body_y + 109.0, del_w, 16.0], "✕", !st.favorites.is_empty(), tr(lang, "Delete Favorite", "Удалить избранное"), tr(lang, "Delete the currently selected saved favorite.", "Удалить текущий выбранный пресет из списка избранного.")));

            share_copy_btn = Some(add_tip_btn(&mut w, &mut tooltips, [lx, body_y + 128.0, half_p, 16.0], tr(lang, "Copy Code", "Копир. код"), true, tr(lang, "Copy BG2 Share Code", "Копировать код BG2"), tr(lang, "Copy full build configuration as a BG2 v5 share code to clipboard.", "Скопировать настройки постройки в виде кода обмена BG2 v5.")));
            share_paste_btn = Some(add_tip_btn(&mut w, &mut tooltips, [lx + half_p + 4.0, body_y + 128.0, half_p, 16.0], tr(lang, "Paste Code", "Вставить код"), true, tr(lang, "Paste BG2 Share Code", "Вставить код BG2"), tr(lang, "Import and apply a BG2 share code from your clipboard.", "Загрузить настройки постройки из кода BG2 в буфере обмена.")));

            // Right column: Y nudge + Area buttons + Facing + Terrain + Preview + Slice 3D
            y_dec_btn = Some(add_tip_btn(&mut w, &mut tooltips, [rx + col_w - 46.0, body_y, 21.0, 16.0], "-Y", true, tr(lang, "Lower Y Elevation", "Опустить уровень Y"), tr(lang, "Move origin down 1 block along Y.", "Сместить точку постройки на 1 блок вниз по вертикали.")));
            y_inc_btn = Some(add_tip_btn(&mut w, &mut tooltips, [rx + col_w - 22.0, body_y, 22.0, 16.0], "+Y", true, tr(lang, "Raise Y Elevation", "Поднять уровень Y"), tr(lang, "Move origin up 1 block along Y.", "Сместить точку постройки на 1 блок вверх по вертикали.")));
            place_front = Some(add_tip_btn(&mut w, &mut tooltips, [rx, body_y + 19.0, col_w, 18.0], tr(lang, "Place in Front", "Поставить перед собой"), true, tr(lang, "Place Box in Front", "Поставить перед собой"), tr(lang, "Anchor build area right in front of you using 5x5 terrain height sampling.", "Установить область перед игроком с замером высоты рельефа 5x5.")));
            pick_world = Some(add_tip_btn(&mut w, &mut tooltips, [rx, body_y + 39.0, col_w, 18.0], tr(lang, "Select Area in World", "Выделить область в мире"), true, tr(lang, "Select Area in World", "Выделить область в мире"), tr(lang, "Pick two corner blocks interactively in the world to size & place the box.", "Выделить два противоположных угла зоны постройки прямо в мире.")));
            rotate_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 60.0, half_p, 18.0],
                match lang {
                    UiLang::En => format!("Front: {}", st.facing.label(lang)),
                    UiLang::Ru => format!("Вход: {}", st.facing.label(lang)),
                },
                true,
                tr(lang, "Entrance Facing", "Сторона фасада"),
                tr(lang, "Rotate main entrance orientation (North, East, South, West).", "Повернуть главный вход здания по сторонам света (С/В/Ю/З)."),
            ));
            terrain_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + half_p + 4.0, body_y + 60.0, half_p, 18.0],
                match (lang, st.adapt_terrain) {
                    (UiLang::En, true) => "Terrain: ON",
                    (UiLang::En, false) => "Terrain: OFF",
                    (UiLang::Ru, true) => "Рельеф: ВКЛ",
                    (UiLang::Ru, false) => "Рельеф: ВЫКЛ",
                },
                true,
                tr(lang, "Terrain Adaptation", "Адаптация к рельефу"),
                tr(lang, "Extend foundation skirts down slopes, add water stilts & step doorway approaches.", "Опускать фундамент и сваи к склону/воде и подгонять ступени у входа."),
            ));
            preview_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 81.0, half_p, 18.0],
                match (lang, st.show_preview_box) {
                    (UiLang::En, true) => "3D View: ON",
                    (UiLang::En, false) => "3D View: OFF",
                    (UiLang::Ru, true) => "3D-обзор: ВКЛ",
                    (UiLang::Ru, false) => "3D-обзор: ВЫКЛ",
                },
                true,
                tr(lang, "3D Hologram Preview", "3D-голограмма"),
                tr(lang, "Toggle real-time semi-transparent 3D preview before building.", "Включить или выключить полупрозрачную 3D-голограмму будущей постройки."),
            ));
            air_only_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + half_p + 4.0, body_y + 81.0, half_p, 18.0],
                match (lang, st.air_only) {
                    (UiLang::En, true) => "Air Only: ON",
                    (UiLang::En, false) => "Air Only: OFF",
                    (UiLang::Ru, true) => "Т.возд: ВКЛ",
                    (UiLang::Ru, false) => "Т.возд: ВЫКЛ",
                },
                true,
                tr(lang, "Air Only Filter", "Только в воздух"),
                tr(lang, "Only place blocks into air — preserves terrain and existing structures.", "Ставить блоки только в воздух, не разрушая рельеф и соседние постройки."),
            ));

            let slice_lbl = match (lang, st.preview_slice_y) {
                (UiLang::En, Some(sy)) => format!("3D Slice: Y<={sy}"),
                (UiLang::En, None) => "3D Slice: Full".into(),
                (UiLang::Ru, Some(sy)) => format!("Срез 3D: Y<={sy}"),
                (UiLang::Ru, None) => "Срез 3D: Весь".into(),
            };
            let slice_w = col_w - 48.0;
            slice_toggle_btn = Some(add_tip_btn(&mut w, &mut tooltips, [rx, body_y + 102.0, slice_w, 17.0], slice_lbl, true, tr(lang, "Hologram Y Slice", "Поэтажный срез 3D"), tr(lang, "Cut the 3D hologram horizontally to inspect interior rooms and floors.", "Включить горизонтальный срез голограммы для просмотра комнат и этажей.")));
            slice_dec_btn = Some(add_tip_btn(&mut w, &mut tooltips, [rx + slice_w + 3.0, body_y + 102.0, 21.0, 17.0], "-", st.preview_slice_y.is_some(), tr(lang, "Lower Slice Y", "Срез ниже"), tr(lang, "Move 3D cutaway height down by 1 block.", "Опустить уровень горизонтального среза 3D-голограммы на 1 блок.")));
            slice_inc_btn = Some(add_tip_btn(&mut w, &mut tooltips, [rx + slice_w + 26.0, body_y + 102.0, 22.0, 17.0], "+", st.preview_slice_y.is_some(), tr(lang, "Raise Slice Y", "Срез выше"), tr(lang, "Move 3D cutaway height up by 1 block.", "Поднять уровень горизонтального среза 3D-голограммы на 1 блок.")));

            frame_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 121.0, half_p, 16.0],
                match (lang, st.show_frame_box) {
                    (UiLang::En, true) => "Frame: ON",
                    (UiLang::En, false) => "Frame: OFF",
                    (UiLang::Ru, true) => "Рамка: ВКЛ",
                    (UiLang::Ru, false) => "Рамка: ВЫКЛ",
                },
                true,
                tr(lang, "Wireframe Box [H]", "Синяя рамка [H]"),
                tr(lang, "Show or hide bounding wireframe outline independently of 3D preview.", "Показать или скрыть синюю рамку габаритов независимо от 3D-обзора."),
            ));
            collision_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + half_p + 4.0, body_y + 121.0, half_p, 16.0],
                match (lang, st.show_collisions) {
                    (UiLang::En, true) => "Colls: ON",
                    (UiLang::En, false) => "Colls: OFF",
                    (UiLang::Ru, true) => "Коллиз: ВКЛ",
                    (UiLang::Ru, false) => "Коллиз: ВЫКЛ",
                },
                true,
                tr(lang, "Collision Highlights", "Подсветка коллизий"),
                tr(lang, "Highlight existing solid blocks that collide with the planned build.", "Подсвечивать коралловым цветом блоки мира, пересекающиеся с постройкой."),
            ));
            move_preview_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 139.0, half_p, 16.0],
                tr(lang, "Move 3D [World]", "Двигать 3D"),
                true,
                tr(lang, "Move 3D Hologram", "Сдвиг 3D-превью"),
                tr(lang, "Nudge hologram in-world with Arrows/IJKL, PgUp/Dn, R, [/], C & Enter to build.", "Сдвигать превью в мире стрелками/IJKL, PgUp/Dn, R, [/], C и Enter для стройки."),
            ));
            auto_hide_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + half_p + 4.0, body_y + 139.0, half_p, 16.0],
                match (lang, st.auto_hide_on_build) {
                    (UiLang::En, true) => "Hide@Build: ON",
                    (UiLang::En, false) => "Hide@Build: OFF",
                    (UiLang::Ru, true) => "Автоскрыт: ВКЛ",
                    (UiLang::Ru, false) => "Автоскрыт: ВЫКЛ",
                },
                true,
                tr(lang, "Auto-Hide After Build", "Скрывать при стройке"),
                tr(lang, "Automatically hide frame & 3D preview when starting a build.", "Автоматически скрывать рамку и 3D-голограмму после нажатия «Сгенерировать»."),
            ));
        }
        UiTab::DescNoise => {
            let b_gap = 3.0;
            let w0 = (inner_w * 0.24).floor();
            let w1 = (inner_w * 0.27).floor();
            let w2 = (inner_w * 0.26).floor();
            let w3 = inner_w - w0 - w1 - w2 - b_gap * 3.0;
            cat_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [px + pad, body_y, w0, 18.0],
                match lang {
                    UiLang::En => format!("Type: {}", st.category.label(lang)),
                    UiLang::Ru => format!("Тип: {}", st.category.label(lang)),
                },
                true,
                format!("{}: {}", tr(lang, "Category", "Категория"), st.category.label(lang)),
                category_tooltip_desc(st.category, lang),
            ));
            style_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [px + pad + w0 + b_gap, body_y, w1, 18.0],
                match lang {
                    UiLang::En => format!("Style: {}", st.style.label(lang)),
                    UiLang::Ru => format!("Стиль: {}", st.style.label(lang)),
                },
                true,
                format!("{}: {}", tr(lang, "Style", "Стиль"), st.style.label(lang)),
                style_tooltip_desc(st.style, lang),
            ));
            roof_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [px + pad + w0 + w1 + b_gap * 2.0, body_y, w2, 18.0],
                match lang {
                    UiLang::En => format!("Roof: {}", st.roof_choice.label(lang)),
                    UiLang::Ru => format!("Крыша: {}", st.roof_choice.label(lang)),
                },
                true,
                format!("{}: {}", tr(lang, "Roof Shape", "Форма крыши"), st.roof_choice.label(lang)),
                roof_tooltip_desc(st.roof_choice, lang),
            ));
            noise_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [px + pad + w0 + w1 + w2 + b_gap * 3.0, body_y, w3, 18.0],
                match lang {
                    UiLang::En => format!("Noise: {}", st.noise_level.label(lang)),
                    UiLang::Ru => format!("Шум: {}", st.noise_level.label(lang)),
                },
                true,
                format!("{}: {}", tr(lang, "Variation", "Вариативность"), st.noise_level.label(lang)),
                noise_tooltip_desc(st.noise_level, lang),
            ));

            let mut df = TextField::new([px + pad, body_y + 34.0, inner_w, 18.0], 200);
            df.set_value(&st.description);
            df.suggestion = Some(
                tr(
                    lang,
                    "e.g.: 1st floor stone, 2nd floor dark oak, with tower",
                    "например: 1 этаж камень, 2 этаж темный дуб, с башней",
                )
                .into(),
            );
            desc_field = Some(w.add_text_field(df));

            let chip_w = ((inner_w - 12.0) / 4.0).floor();
            for (i, (label, phrase)) in quick_tags(lang).iter().enumerate() {
                let col = (i % 4) as f32;
                let row = (i / 4) as f32;
                let id = add_tip_btn(
                    &mut w,
                    &mut tooltips,
                    [px + pad + col * (chip_w + 4.0), body_y + 55.0 + row * 19.0, chip_w, 17.0],
                    *label,
                    true,
                    *label,
                    match lang {
                        UiLang::En => format!("Append \"{phrase}\" to the text prompt for instant theme/detail."),
                        UiLang::Ru => format!("Добавить «{phrase}» в текстовое описание постройки."),
                    },
                );
                tag_buttons.push((id, *phrase));
            }

            let sf_w = (inner_w * 0.17).floor();
            let ns_w = (inner_w * 0.15).floor();
            let as_w = (inner_w * 0.19).floor();
            let prev_x = px + pad + 24.0;
            let sf_x = prev_x + 23.0;
            let ns_x = sf_x + sf_w + 3.0;
            let as_x = ns_x + ns_w + 3.0;
            let arch_x = as_x + as_w + 3.0;
            let arch_area_w = (px + pad + inner_w - arch_x).max(86.0);
            let nav_w = 16.0;
            let cat_btn_w = (arch_area_w - nav_w * 2.0 - 4.0).max(50.0);

            prev_seed_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [prev_x, body_y + 95.0, 21.0, 17.0],
                "<",
                true,
                tr(lang, "Previous Seed", "Предыдущий сид"),
                tr(lang, "Restore the previous seed from recent roll history.", "Вернуть предыдущий сид из истории генераций."),
            ));
            let mut sf = TextField::new([sf_x, body_y + 95.0, sf_w, 17.0], 20);
            sf.set_value(&st.seed_text);
            seed_field = Some(w.add_text_field(sf));
            new_seed_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [ns_x, body_y + 95.0, ns_w, 17.0],
                tr(lang, "New", "Новый"),
                true,
                tr(lang, "Roll Random Seed", "Новый случайный сид"),
                tr(lang, "Generate a fresh random seed to get a different layout & variation.", "Сгенерировать новый случайный сид для другой планировки."),
            ));
            auto_seed_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [as_x, body_y + 95.0, as_w, 17.0],
                match (lang, st.auto_seed) {
                    (UiLang::En, true) => "Seed: AUTO",
                    (UiLang::En, false) => "Seed: FIX",
                    (UiLang::Ru, true) => "Сид: АВТО",
                    (UiLang::Ru, false) => "Сид: ФИКС",
                },
                true,
                tr(lang, "Seed Lock Mode", "Режим сида"),
                tr(lang, "AUTO rolls a fresh seed on every build; FIX keeps this exact seed.", "АВТО меняет сид при каждой стройке; ФИКС закрепляет текущее число."),
            ));
            archetype_prev_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [arch_x, body_y + 95.0, nav_w, 17.0],
                "◀",
                true,
                tr(lang, "Previous Preset", "Предыдущий пресет"),
                tr(lang, "Step backward through all 88 built-in structure presets.", "Перейти к предыдущему пресету из встроенного каталога (88 шт.)."),
            ));
            let active_idx = st.archetype_idx % CLI_PRESETS.len();
            archetype_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [arch_x + nav_w + 2.0, body_y + 95.0, cat_btn_w, 17.0],
                match lang {
                    UiLang::En => format!("Catalog {}/{}", active_idx + 1, CLI_PRESETS.len()),
                    UiLang::Ru => format!("Каталог {}/{}", active_idx + 1, CLI_PRESETS.len()),
                },
                true,
                tr(lang, "Open Preset Catalog", "Открыть каталог (88)"),
                match lang {
                    UiLang::En => format!("Browse all {} curated presets. Active: #{}: {}", CLI_PRESETS.len(), active_idx + 1, CLI_PRESETS[active_idx].0),
                    UiLang::Ru => format!("Открыть каталог из {} готовых построек. Текущий: #{}: {}", CLI_PRESETS.len(), active_idx + 1, CLI_PRESETS[active_idx].0),
                },
            ));
            archetype_next_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [arch_x + nav_w + 2.0 + cat_btn_w + 2.0, body_y + 95.0, nav_w, 17.0],
                "▶",
                true,
                tr(lang, "Next Preset", "Следующий пресет"),
                tr(lang, "Step forward through all 88 built-in structure presets.", "Перейти к следующему пресету из встроенного каталога (88 шт.)."),
            ));

            let tog_w = ((inner_w - 12.0) / 5.0).floor();
            for (idx, (lbl, _, _)) in toggle_meta(lang).iter().enumerate() {
                let id = add_tip_btn(
                    &mut w,
                    &mut tooltips,
                    [px + pad + idx as f32 * (tog_w + 3.0), body_y + 115.0, tog_w, 17.0],
                    format!("{}:{}", lbl, st.toggles[idx].badge(lang)),
                    true,
                    *lbl,
                    toggle_tooltip_desc(idx, st.toggles[idx], lang),
                );
                feature_toggles.push((id, idx));
            }

            // Row: [Biome Auto] + [-][Wear %][+] + [-][Moss %][+]
            let row_y = body_y + 136.0;
            let gap3 = 4.0;
            let bw = (inner_w * 0.34).floor();
            let ww = ((inner_w - bw - gap3 * 2.0) * 0.5).floor();
            let ow = inner_w - bw - ww - gap3 * 2.0;

            let biome_txt = if !st.use_biome_auto {
                tr(lang, "Biome: OFF", "Биом: ВЫКЛ").to_owned()
            } else {
                let bname = crate::parser::biome_short_label(&st.detected_biome, lang);
                match lang {
                    UiLang::En => format!("Biome: {bname} ✓"),
                    UiLang::Ru => format!("Биом: {bname} ✓"),
                }
            };
            biome_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [px + pad, row_y, bw, 17.0],
                biome_txt,
                true,
                tr(lang, "Live World Biome Sync", "Адаптация к биому"),
                tr(lang, "Inspect your current biome so Auto style & roads match local stone/wood/sand.", "Определяет текущий биом мира для стиля «Авто», дорог и палитры."),
            ));

            let step_w = 15.0;
            let wx = px + pad + bw + gap3;
            wear_dec_btn = Some(add_tip_btn(&mut w, &mut tooltips, [wx, row_y, step_w, 17.0], "-", true, tr(lang, "Less Wear", "Меньше износа"), tr(lang, "Decrease ruin aging & erosion by 15%.", "Уменьшить процент обветшания и руин на 15%.")));
            let wear_lbl = match lang {
                UiLang::En => format!("Wear: {}%", st.wear_pct),
                UiLang::Ru => format!("Износ: {}%", st.wear_pct),
            };
            wear_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [wx + step_w + 1.0, row_y, (ww - step_w * 2.0 - 2.0).max(40.0), 17.0],
                wear_lbl,
                true,
                tr(lang, "Wear & Ruin Aging", "Износ и разрушение"),
                tr(lang, "Cracked stone, oxidized copper, shattered glass, rubble & cobwebs (0-100%).", "Трещины, окисление меди, битые стёкла, обрушения и паутина (0-100%)."),
            ));
            wear_inc_btn = Some(add_tip_btn(&mut w, &mut tooltips, [wx + ww - step_w, row_y, step_w, 17.0], "+", true, tr(lang, "More Wear", "Больше износа"), tr(lang, "Increase ruin aging & erosion by 15%.", "Увеличить процент обветшания и руин на 15%.")));

            let ox = wx + ww + gap3;
            over_dec_btn = Some(add_tip_btn(&mut w, &mut tooltips, [ox, row_y, step_w, 17.0], "-", true, tr(lang, "Less Overgrowth", "Меньше зелени"), tr(lang, "Decrease vines, moss & wild flora by 15%.", "Уменьшить зарастание мхом и лианами на 15%.")));
            let over_lbl = match lang {
                UiLang::En => format!("Moss: {}%", st.overgrowth_pct),
                UiLang::Ru => format!("Зелень: {}%", st.overgrowth_pct),
            };
            over_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [ox + step_w + 1.0, row_y, (ow - step_w * 2.0 - 2.0).max(40.0), 17.0],
                over_lbl,
                true,
                tr(lang, "Overgrowth & Flora", "Зарастание зеленью"),
                tr(lang, "Climbing vines, biome-aware moss, flowering leaves & foliage (0-100%).", "Плети лоз, биомный мох, цветущая листва и кустарники (0-100%)."),
            ));
            over_inc_btn = Some(add_tip_btn(&mut w, &mut tooltips, [ox + ow - step_w, row_y, step_w, 17.0], "+", true, tr(lang, "More Overgrowth", "Больше зелени"), tr(lang, "Increase vines, moss & wild flora by 15%.", "Увеличить зарастание мхом и лианами на 15%.")));
        }
        UiTab::Materials => {
            let lx = px + pad;
            let top_w0 = (inner_w * 0.34).floor();
            let top_w1 = (inner_w * 0.21).floor();
            let top_w2 = (inner_w * 0.22).floor();
            let top_w3 = inner_w - top_w0 - top_w1 - top_w2 - 9.0;
            mat_preset_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y, top_w0, 17.0],
                match lang {
                    UiLang::En => format!("Palette: {}", st.palette.display_name(lang)),
                    UiLang::Ru => format!("Палитра: {}", st.palette.display_name(lang)),
                },
                true,
                tr(lang, "Palette Preset", "Пресет палитры"),
                tr(
                    lang,
                    "Cycle curated architectural material presets across all 30 slots.",
                    "Перебирать готовые цветовые наборы архитектуры для всех 30 слотов.",
                ),
            ));
            let infer_lbl = match (lang, st.palette_manual) {
                (UiLang::En, true) => "From Text",
                (UiLang::En, false) => "From Text ✓",
                (UiLang::Ru, true) => "По тексту",
                (UiLang::Ru, false) => "По тексту ✓",
            };
            mat_infer_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx + top_w0 + 3.0, body_y, top_w1, 17.0],
                infer_lbl,
                true,
                tr(lang, "Infer from Prompt & Style", "Подбор по тексту и стилю"),
                tr(
                    lang,
                    "Reset manual slot tweaks and auto-pick materials from your description, style & biome.",
                    "Сбросить ручные правки и автоматически подобрать блоки по тексту, стилю и биому.",
                ),
            ));
            mat_sync_wood_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx + top_w0 + top_w1 + 6.0, body_y, top_w2, 17.0],
                match (lang, st.auto_sync_wood) {
                    (UiLang::En, true) => "Wood ✓",
                    (UiLang::En, false) => "Wood ✗",
                    (UiLang::Ru, true) => "Дерево ✓",
                    (UiLang::Ru, false) => "Дерево ✗",
                },
                true,
                tr(lang, "Sync Matching Wood Set", "Синхронизация древесины"),
                tr(
                    lang,
                    "When picking a plank or log, auto-match doors, fences, slabs, gates & trapdoors.",
                    "При смене досок или брёвен автоматически подбирать двери, заборы, люки и плиты той же породы.",
                ),
            ));
            mat_mix_mode_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx + top_w0 + top_w1 + top_w2 + 9.0, body_y, top_w3, 17.0],
                match lang {
                    UiLang::En => format!("Mix: {}", st.palette.mix_mode.label(lang)),
                    UiLang::Ru => format!("Микс: {}", st.palette.mix_mode.label(lang)),
                },
                true,
                tr(lang, "Multi-Block Mix Mode", "Режим смешивания"),
                tr(
                    lang,
                    "Cycle how multi-block lists blend: Patches, Scatter, Stripes, or Checker.",
                    "Выбор узора смешивания блоков из списка слота: Пятна, Россыпь, Полосы, Шахматка.",
                ),
            ));

            let body_h = (foot_y - 25.0 - body_y).max(162.0);
            let cols_y = body_y + 20.0;
            let cols_h = body_h - 20.0;
            let left_w = (inner_w * 0.46).floor();
            let gap = 8.0;
            let rx = lx + left_w + gap;
            let right_w = inner_w - left_w - gap;
            let sb_w = 10.0;

            // Left column: Smooth scrollable list of all 30 slots (NO tooltips on materials!)
            let slot_row_h = 16.0;
            let slot_vis_rows = ((cols_h / slot_row_h).floor() as usize).clamp(7, 11);
            let slot_max_scroll = MaterialPalette::SLOT_COUNT.saturating_sub(slot_vis_rows);
            let slot_scroll = st.mat_slot_scroll.min(slot_max_scroll);
            let slot_area_w = left_w - sb_w - 2.0;
            for r in 0..slot_vis_rows {
                let slot = slot_scroll + r;
                if slot >= MaterialPalette::SLOT_COUNT {
                    break;
                }
                let ry = cols_y + (r as f32) * slot_row_h;
                tab3_slot_rows.push((slot, [lx, ry, slot_area_w, 15.0]));
                let cnt = st.palette.slot_blocks(slot).len();
                let marker = if slot == st.selected_mat_slot { ">" } else { "" };
                let badge = if cnt > 1 {
                    format!(" [{cnt}]")
                } else {
                    String::new()
                };
                let label = format!(
                    "{}{}: {}{}",
                    marker,
                    slot_label(slot, lang),
                    short_block_name(slot_block_id(&st.palette, slot)),
                    badge
                );
                let btn = w.add_button(Button::new(
                    [lx + 15.0, ry, slot_area_w - 15.0, 15.0],
                    Text::literal(label),
                ));
                mat_slot_btns.push((btn, slot));
            }
            let s_sb_x = lx + slot_area_w + 2.0;
            let s_sb_h = (slot_vis_rows as f32) * slot_row_h - 1.0;
            mat_slot_up_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [s_sb_x, cols_y, sb_w, 12.0],
                "▲",
                slot_scroll > 0,
                tr(lang, "Scroll Slots Up", "Прокрутка вверх"),
                tr(lang, "Scroll architectural slot list up.", "Прокрутить список слотов вверх."),
            ));
            mat_slot_down_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [s_sb_x, cols_y + s_sb_h - 12.0, sb_w, 12.0],
                "▼",
                slot_scroll < slot_max_scroll,
                tr(lang, "Scroll Slots Down", "Прокрутка вниз"),
                tr(lang, "Scroll architectural slot list down.", "Прокрутить список слотов вниз."),
            ));
            slot_scroll_zone = Some(ScrollZone {
                viewport: [lx, cols_y, left_w, s_sb_h],
                track: [s_sb_x, cols_y + 13.0, sb_w, (s_sb_h - 26.0).max(8.0)],
                total_rows: MaterialPalette::SLOT_COUNT,
                vis_rows: slot_vis_rows,
                scroll: slot_scroll,
            });

            // Right column: Editable ordered block list for selected slot + Separate Picker launcher
            let slot_blocks = st.palette.slot_blocks(st.selected_mat_slot);
            tab3_header_pos = Some([rx, cols_y + 3.0]);
            let reset_w = 64.0;
            list_clear_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + right_w - reset_w, cols_y, reset_w, 15.0],
                tr(lang, "Reset to 1", "Сброс до 1"),
                slot_blocks.len() > 1,
                tr(lang, "Reset Slot to 1 Block", "Сбросить до 1 блока"),
                tr(
                    lang,
                    "Remove all extra blocks from this slot list and keep only #1.",
                    "Удалить дополнительные варианты блоков в этом слоте, оставив только первый.",
                ),
            ));

            let act_y = cols_y + cols_h - 16.0;
            let add_w = (right_w * 0.62).floor();
            let pop_w = right_w - add_w - 3.0;
            picker_add_open_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, act_y, add_w, 16.0],
                tr(lang, "+ Expand (Menu)", "+ Расширить (Меню)"),
                true,
                tr(lang, "Open Block Picker", "Открыть каталог блоков"),
                tr(
                    lang,
                    "Open dedicated searchable block menu to add variants into this slot.",
                    "Открыть окно поиска и выбора блоков для добавления в текущий слот.",
                ),
            ));
            list_pop_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx + add_w + 3.0, act_y, pop_w, 16.0],
                tr(lang, "- Remove", "- Убрать"),
                slot_blocks.len() > 1,
                tr(lang, "Remove Last Block", "Убрать последний блок"),
                tr(
                    lang,
                    "Pop the last block out of this slot's mix list.",
                    "Удалить последний добавленный блок из списка этого слота.",
                ),
            ));

            let list_y = cols_y + 18.0;
            let list_avail_h = (act_y - 3.0 - list_y).max(64.0);
            let list_row_h = 16.0;
            let list_vis_rows = ((list_avail_h / list_row_h).floor() as usize).clamp(4, 8);
            let list_max_scroll = slot_blocks.len().saturating_sub(list_vis_rows);
            let list_scroll = st.mat_list_scroll.min(list_max_scroll);
            let item_area_w = right_w - sb_w - 2.0;
            let rm_w = 17.0;

            for r in 0..list_vis_rows {
                let idx = list_scroll + r;
                if let Some(bid) = slot_blocks.get(idx) {
                    let ry = list_y + (r as f32) * list_row_h;
                    tab3_list_rows.push((idx, bid.clone(), [rx, ry, item_area_w, 15.0]));
                    let short = short_block_name(bid);
                    let tag = if idx == 0 { tr(lang, " (main)", " (осн)") } else { "" };
                    let btn = w.add_button(Button::new(
                        [rx + 15.0, ry, item_area_w - 15.0 - rm_w - 2.0, 15.0],
                        Text::literal(format!("{}. {}{}", idx + 1, short, tag)),
                    ));
                    list_item_btns.push((btn, idx));

                    let mut rmb =
                        Button::new([rx + item_area_w - rm_w, ry, rm_w, 15.0], Text::literal("✕"));
                    rmb.active = slot_blocks.len() > 1;
                    list_remove_btns.push((w.add_button(rmb), idx));
                }
            }
            let l_sb_x = rx + item_area_w + 2.0;
            let l_sb_h = (list_vis_rows as f32) * list_row_h - 1.0;
            list_up_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [l_sb_x, list_y, sb_w, 12.0],
                "▲",
                list_scroll > 0,
                tr(lang, "Scroll List Up", "Прокрутка вверх"),
                tr(lang, "Scroll slot block list up.", "Прокрутить список блоков слота вверх."),
            ));
            list_down_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [l_sb_x, list_y + l_sb_h - 12.0, sb_w, 12.0],
                "▼",
                list_scroll < list_max_scroll,
                tr(lang, "Scroll List Down", "Прокрутка вниз"),
                tr(lang, "Scroll slot block list down.", "Прокрутить список блоков слота вниз."),
            ));
            list_scroll_zone = Some(ScrollZone {
                viewport: [rx, list_y, right_w, l_sb_h],
                track: [l_sb_x, list_y + 13.0, sb_w, (l_sb_h - 26.0).max(8.0)],
                total_rows: slot_blocks.len().max(1),
                vis_rows: list_vis_rows,
                scroll: list_scroll,
            });
        }
        UiTab::WorldDecor => {
            let lx = px + pad;
            let half_w = ((inner_w - 6.0) * 0.5).floor();
            let rx = lx + half_w + 6.0;
            let right_w = inner_w - half_w - 6.0;

            // Section 1: Living Interiors 2.0 + Outer Terrain Smoothing
            living_decor_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y + 17.0, half_w, 19.0],
                match (lang, st.living_decor) {
                    (UiLang::En, true) => "Living Decor: ON",
                    (UiLang::En, false) => "Living Decor: OFF",
                    (UiLang::Ru, true) => "Живой декор: ВКЛ",
                    (UiLang::Ru, false) => "Живой декор: ВЫКЛ",
                },
                true,
                tr(lang, "Living Interiors 2.0", "Живые интерьеры 2.0"),
                tr(
                    lang,
                    "Add heraldic banners, readable lectern books, streetlamps, signs, pressure plates & dungeon traps.",
                    "Добавляет геральдические знамёна, читаемые книги на кафедрах, ночные фонари, таблички и механизмы.",
                ),
            ));
            spawn_npcs_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 17.0, right_w, 19.0],
                match (lang, st.spawn_npcs) {
                    (UiLang::En, true) => "Spawn NPCs/Mobs: ON",
                    (UiLang::En, false) => "Spawn NPCs/Mobs: OFF",
                    (UiLang::Ru, true) => "Спавн NPC/Мобов: ВКЛ",
                    (UiLang::Ru, false) => "Спавн NPC/Мобов: ВЫКЛ",
                },
                true,
                tr(lang, "NPC & Livestock Spawning", "Спавн жителей и мобов"),
                tr(
                    lang,
                    "Populate villagers, golems, barn animals & dungeon guards (cleanly removed on Undo).",
                    "Заселяет жителей, големов, скот на фермы и стражей в данжи (полностью удаляются при Undo).",
                ),
            ));

            let blend_lbl = match (lang, st.blend_radius) {
                (UiLang::En, 0) => "Edge Blend: Off".to_owned(),
                (UiLang::En, r) => format!("Edge Blend: {r}b"),
                (UiLang::Ru, 0) => "Сглаж. краёв: Выкл".to_owned(),
                (UiLang::Ru, r) => format!("Сглаж. краёв: {r}"),
            };
            blend_radius_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y + 40.0, half_w, 19.0],
                blend_lbl,
                true,
                tr(lang, "Smooth Radial Edge Blend", "Радиальное сглаживание краёв"),
                tr(
                    lang,
                    "Smoothly merge ground & roads into surrounding terrain (0–10 blocks radius) using biome blocks.",
                    "Плавно вписывает основание постройки и дорог в окружающий ландшафт в радиусе 0–10 блоков.",
                ),
            ));
            terrain_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 40.0, right_w, 19.0],
                match (lang, st.adapt_terrain) {
                    (UiLang::En, true) => "Terrain: ON",
                    (UiLang::En, false) => "Terrain: OFF",
                    (UiLang::Ru, true) => "Рельеф: ВКЛ",
                    (UiLang::Ru, false) => "Рельеф: ВЫКЛ",
                },
                true,
                tr(lang, "Terrain Adaptation", "Адаптация к рельефу"),
                tr(
                    lang,
                    "Extend masonry skirts, road beds & water stilts seamlessly down onto uneven terrain.",
                    "Вытягивает фундаменты, насыпи дорог и сваи к реальным перепадам высоты и воде.",
                ),
            ));

            // Section 2: 2-Point Road & Auto-Bridge Connector
            road_width_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y + 94.0, half_w, 18.0],
                match lang {
                    UiLang::En => format!("Road Width: {}", st.road_width),
                    UiLang::Ru => format!("Ширина дороги: {}", st.road_width),
                },
                true,
                tr(lang, "Road Width (3 / 5 blocks)", "Ширина дороги (3 / 5)"),
                tr(
                    lang,
                    "Switch between 3-block village lane and wide 5-block highway/avenue.",
                    "Переключает ширину дороги между узкой (3 бл.) и широким проспектом (5 бл.).",
                ),
            ));
            road_auto_bridge_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [rx, body_y + 94.0, right_w, 18.0],
                match (lang, st.road_auto_bridge) {
                    (UiLang::En, true) => "Auto-Bridge: ON",
                    (UiLang::En, false) => "Auto-Bridge: OFF",
                    (UiLang::Ru, true) => "Авто-мост: ВКЛ",
                    (UiLang::Ru, false) => "Авто-мост: ВЫКЛ",
                },
                true,
                tr(lang, "Automatic River Bridges", "Авто-мост через воду"),
                tr(
                    lang,
                    "Automatically span rivers and channels with arched piers, railings & style lanterns.",
                    "Автоматически строит арочный мост со сваями, перилами и фонарями над водой.",
                ),
            ));
            road_style_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y + 115.0, half_w, 18.0],
                match lang {
                    UiLang::En => format!("Style: {}", st.style.label(lang)),
                    UiLang::Ru => format!("Стиль: {}", st.style.label(lang)),
                },
                true,
                format!("{}: {}", tr(lang, "Road Style", "Стиль дороги"), st.style.label(lang)),
                style_tooltip_desc(st.style, lang),
            ));

            road_connect_btn = Some(add_tip_btn(
                &mut w,
                &mut tooltips,
                [lx, body_y + 136.0, inner_w, 21.0],
                tr(lang, "Connect Road (2 Points)", "Проложить дорогу (2 точки)"),
                true,
                tr(lang, "Connect Two Points by Road", "Проложить дорогу по 2 точкам"),
                tr(
                    lang,
                    "Pick Start and End points in world to pathfind a smooth styled road & auto-bridge.",
                    "Укажите Старт и Финиш в мире — алгоритм проложит дорогу выбранного стиля с мостами.",
                ),
            ));
        }
    }
    }

    // Persistent bottom footer
    let gap = 3.0;
    let gen_w = (inner_w * 0.29).floor();
    let reroll_w = (inner_w * 0.17).floor();
    let undo_w = (inner_w * 0.19).floor();
    let redo_w = (inner_w * 0.19).floor();
    let close_w = inner_w - gen_w - reroll_w - undo_w - redo_w - gap * 4.0;

    let generate_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pad, foot_y, gen_w, 20.0],
        tr(lang, "GENERATE", "СГЕНЕРИРОВАТЬ"),
        true,
        tr(lang, "Generate Structure", "Построить здание"),
        tr(
            lang,
            "Build the configured structure in the selected world area (prompts area pick if unset).",
            "Возвести постройку в выбранной зоне мира (если зона не задана, откроет выделение).",
        ),
    );
    let reroll_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pad + gen_w + gap, foot_y, reroll_w, 20.0],
        tr(lang, "Reroll", "Заново"),
        true,
        tr(lang, "Reroll at Same Spot", "Перегенерировать здесь же"),
        tr(
            lang,
            "Undo the last structure and rebuild it in the exact same spot with a new seed.",
            "Откатить последнюю постройку и сразу возвести новую версию на том же месте.",
        ),
    );
    let undo_label = if st.undo_count > 0 {
        match lang {
            UiLang::En => format!("Undo ({})", st.undo_count),
            UiLang::Ru => format!("Отмена ({})", st.undo_count),
        }
    } else {
        tr(lang, "Undo", "Отмена").into()
    };
    let undo_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pad + gen_w + reroll_w + gap * 2.0, foot_y, undo_w, 20.0],
        undo_label,
        true,
        tr(lang, "Undo Last Build", "Отменить постройку"),
        tr(
            lang,
            "Restore previous terrain blocks and remove spawned NPCs/decor (persists across restarts, up to 5 steps).",
            "Восстановить исходные блоки и убрать заспавненных NPC (история до 5 шагов сохраняется на диск).",
        ),
    );
    let redo_label = if st.redo_count > 0 {
        match lang {
            UiLang::En => format!("Redo ({})", st.redo_count),
            UiLang::Ru => format!("Возврат ({})", st.redo_count),
        }
    } else {
        tr(lang, "Redo", "Возврат").into()
    };
    let redo_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [px + pad + gen_w + reroll_w + undo_w + gap * 3.0, foot_y, redo_w, 20.0],
        redo_label,
        true,
        tr(lang, "Redo Undone Build", "Вернуть постройку"),
        tr(
            lang,
            "Re-apply the last undone structure or road build from history.",
            "Повторно применить отменённую постройку или дорогу из истории.",
        ),
    );
    let close_btn = add_tip_btn(
        &mut w,
        &mut tooltips,
        [
            px + pad + gen_w + reroll_w + undo_w + redo_w + gap * 4.0,
            foot_y,
            close_w,
            20.0,
        ],
        tr(lang, "Close", "Закрыть"),
        true,
        tr(lang, "Close Menu [F8 / Esc]", "Закрыть меню [F8 / Esc]"),
        tr(
            lang,
            "Close Building Generator menu while keeping your area, settings & 3D preview active.",
            "Скрыть меню; выделенная область, настройки и 3D-превью останутся активны.",
        ),
    );

    BuiltScreen {
        size,
        panel,
        widgets: w,
        handles: WidgetHandles {
            bom_btn,
            lang_btn,
            tab_size,
            tab_desc,
            tab_mats,
            tab_world,
            w_dec,
            w_inc,
            d_dec,
            d_inc,
            h_dec,
            h_inc,
            preset_small,
            preset_med,
            preset_large,
            preset_tower,
            place_front,
            pick_world,
            rotate_btn,
            y_dec_btn,
            y_inc_btn,
            terrain_btn,
            preview_btn,
            air_only_btn,
            slice_toggle_btn,
            slice_dec_btn,
            slice_inc_btn,
            frame_btn,
            collision_btn,
            move_preview_btn,
            auto_hide_btn,
            fav_save_btn,
            fav_load_btn,
            fav_del_btn,
            share_copy_btn,
            share_paste_btn,
            cat_btn,
            style_btn,
            roof_btn,
            desc_field,
            seed_field,
            prev_seed_btn,
            new_seed_btn,
            auto_seed_btn,
            archetype_prev_btn,
            archetype_btn,
            archetype_next_btn,
            noise_btn,
            biome_btn,
            wear_dec_btn,
            wear_btn,
            wear_inc_btn,
            over_dec_btn,
            over_btn,
            over_inc_btn,
            tag_buttons,
            feature_toggles,
            preset_back_btn,
            preset_cat_btn,
            preset_search_field,
            preset_search_clear_btn,
            preset_up_btn,
            preset_down_btn,
            preset_grid_btns,
            bom_back_btn,
            bom_copy_btn,
            bom_up_btn,
            bom_down_btn,
            mat_preset_btn,
            mat_infer_btn,
            mat_sync_wood_btn,
            mat_mix_mode_btn,
            mat_slot_up_btn,
            mat_slot_down_btn,
            mat_slot_btns,
            list_clear_btn,
            list_up_btn,
            list_down_btn,
            list_item_btns,
            list_remove_btns,
            picker_add_open_btn,
            list_pop_btn,
            picker_back_btn,
            picker_mode_btn,
            picker_catalog_btn,
            mat_filter_btns,
            mat_search_field,
            mat_search_clear_btn,
            picker_up_btn,
            picker_down_btn,
            picker_grid_btns,
            picker_chip_btns,
            living_decor_btn,
            spawn_npcs_btn,
            blend_radius_btn,
            road_width_btn,
            road_auto_bridge_btn,
            road_style_btn,
            road_connect_btn,
            generate_btn,
            reroll_btn,
            undo_btn,
            redo_btn,
            close_btn,
        },
        tooltips,
        slot_scroll_zone,
        list_scroll_zone,
        picker_scroll_zone,
        preset_scroll_zone,
        bom_scroll_zone,
        tab3_slot_rows,
        tab3_list_rows,
        tab3_header_pos,
        picker_tiles,
        picker_chips,
        picker_strip_pos,
        bom_rows,
    }
}

fn compute_origin_in_front(host: &dyn ClientHost, st: &mut ClientUiState) {
    let Some(cam) = host.camera() else {
        return;
    };
    st.dimension = cam.dimension.clone();
    st.facing = CardinalFacing::from_yaw(cam.yaw);
    let yaw_rad = f64::from(cam.yaw).to_radians();
    let fx = -yaw_rad.sin();
    let fz = yaw_rad.cos();
    let dist = f64::from(st.width.max(st.depth)) * 0.5 + 4.5;
    let cx = (cam.position[0] + fx * dist).round() as i32;
    let cz = (cam.position[2] + fz * dist).round() as i32;
    let iw = i32::from(st.width.clamp(7, 96));
    let id = i32::from(st.depth.clamp(7, 96));
    let ox = cx - iw / 2;
    let oz = cz - id / 2;

    // Sample a 5x5 grid across the entire footprint instead of a single center point so local
    // pillars, fountains, trees, or preexisting roofs do not launch the building into mid-air.
    let mut sample_cols = Vec::with_capacity(25);
    for gz in 0..5 {
        let lz = (gz * (id - 1)) / 4;
        for gx in 0..5 {
            let lx = (gx * (iw - 1)) / 4;
            sample_cols.push((ox + lx, oz + lz));
        }
    }

    let mut oy = (cam.position[1] - f64::from(cam.eye_height)).floor() as i32 - 1;
    if let Ok(heights) = host.client_surface_heights(&sample_cols, Heightmap::MotionBlockingNoLeaves) {
        let mut ys: Vec<i32> = heights.into_iter().flatten().map(|sy| sy - 1).collect();
        if !ys.is_empty() {
            ys.sort_unstable();
            oy = ys[(ys.len() * 35) / 100];
        }
    }
    st.origin = Some([ox, oy, oz]);
}

fn current_aim_block(host: &dyn ClientHost) -> Option<[i32; 3]> {
    if let Some(target) = host.crosshair_target() {
        match target {
            CrosshairTarget::Block { pos, hit, .. } => {
                // Anchor directly on the clicked surface block when aiming at its top face so foundations
                // sit flush on terrain instead of hovering 1 block up in the air.
                let fy = if hit.face[1] > 0 { 0 } else { hit.face[1] };
                return Some([
                    pos.x + hit.face[0],
                    pos.y + fy,
                    pos.z + hit.face[2],
                ]);
            }
            CrosshairTarget::Entity { point, .. } => {
                return Some([
                    point[0].floor() as i32,
                    point[1].floor() as i32,
                    point[2].floor() as i32,
                ]);
            }
            _ => {}
        }
    }
    if let Some((_, hit)) = host.view_hit() {
        return Some([hit.block_x, hit.block_y, hit.block_z]);
    }
    None
}

fn refresh_picker_search_widgets(
    screen: &mut BuiltScreen,
    slot: usize,
    all_blocks: bool,
    world_blocks: &[String],
    filter: MatFilter,
    query: &str,
    pal: &MaterialPalette,
) {
    let choices = filtered_picker_choices(slot, all_blocks, world_blocks, filter, query);
    let total_rows = choices.len().div_ceil(PICKER_COLS).max(1);
    for ci in 0..screen.handles.picker_grid_btns.len() {
        let bid = screen.handles.picker_grid_btns[ci].0;
        if let Some(cand_id) = choices.get(ci) {
            screen.handles.picker_grid_btns[ci].1 = cand_id.clone();
            let in_list = pal.slot_contains(slot, cand_id);
            if let Some(tile) = screen.picker_tiles.get_mut(ci) {
                tile.0 = cand_id.clone();
                tile.1 = in_list;
            }
            let short = short_block_name(cand_id);
            let lbl = if in_list {
                format!("✓ {short}")
            } else {
                short.to_owned()
            };
            if let Some(b) = screen.widgets.button_mut(bid) {
                b.label = Text::literal(lbl);
                b.active = true;
            }
        } else {
            screen.handles.picker_grid_btns[ci].1.clear();
            if let Some(tile) = screen.picker_tiles.get_mut(ci) {
                tile.0.clear();
                tile.1 = false;
            }
            if let Some(b) = screen.widgets.button_mut(bid) {
                b.label = Text::literal("");
                b.active = false;
            }
        }
    }
    if let Some(cid) = screen.handles.mat_search_clear_btn
        && let Some(b) = screen.widgets.button_mut(cid)
    {
        b.active = !query.is_empty();
    }
    if let Some(z) = screen.picker_scroll_zone.as_mut() {
        z.total_rows = total_rows;
        z.scroll = 0;
        if let Some(uid) = screen.handles.picker_up_btn
            && let Some(b) = screen.widgets.button_mut(uid)
        {
            b.active = false;
        }
        if let Some(did) = screen.handles.picker_down_btn
            && let Some(b) = screen.widgets.button_mut(did)
        {
            b.active = total_rows > z.vis_rows;
        }
    }
}

fn refresh_preset_search_widgets(
    screen: &mut BuiltScreen,
    cat_filter: Option<BuildCategory>,
    query: &str,
    active_idx: usize,
    lang: UiLang,
) {
    let matches = filtered_preset_indices(cat_filter, query);
    let total_rows = matches.len().div_ceil(2).max(1);
    for ci in 0..screen.handles.preset_grid_btns.len() {
        let bid = screen.handles.preset_grid_btns[ci].0;
        if let Some(&preset_idx) = matches.get(ci) {
            screen.handles.preset_grid_btns[ci].1 = preset_idx;
            if let Some(b) = screen.widgets.button_mut(bid) {
                b.label = Text::literal(preset_card_label(preset_idx, active_idx, lang));
                b.active = true;
            }
        } else {
            screen.handles.preset_grid_btns[ci].1 = usize::MAX;
            if let Some(b) = screen.widgets.button_mut(bid) {
                b.label = Text::literal("");
                b.active = false;
            }
        }
    }
    if let Some(cid) = screen.handles.preset_search_clear_btn
        && let Some(b) = screen.widgets.button_mut(cid)
    {
        b.active = !query.is_empty();
    }
    if let Some(z) = screen.preset_scroll_zone.as_mut() {
        z.total_rows = total_rows;
        z.scroll = 0;
        if let Some(uid) = screen.handles.preset_up_btn
            && let Some(b) = screen.widgets.button_mut(uid)
        {
            b.active = false;
        }
        if let Some(did) = screen.handles.preset_down_btn
            && let Some(b) = screen.widgets.button_mut(did)
        {
            b.active = total_rows > z.vis_rows;
        }
    }
}

fn handle_widget_input(host: &dyn ClientHost, input: impl FnOnce(&mut Widgets) -> Outcome) -> Flow {
    if host.mod_screen() != Some(SCREEN_ID) {
        return Flow::Continue;
    }
    with_state(|st| {
        let mat_slot = st.selected_mat_slot;
        let all_blocks = st.picker_all_blocks;
        let world_blocks = st.world_blocks.clone();
        let mat_filter = st.mat_filter;
        let pal_snapshot = st.palette.clone();
        let preset_cat_filter = st.preset_cat_filter;
        let active_preset = st.archetype_idx % CLI_PRESETS.len();
        let cur_lang = st.lang;
        let Some(screen) = st.screen.as_mut() else {
            return Flow::Continue;
        };
        let outcome = input(&mut screen.widgets);
        let h = &screen.handles;

        match outcome {
            Outcome::Changed(id) => {
                if Some(id) == h.desc_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    st.description = tf.value().to_owned();
                    sync_palette_from_ui(st);
                    st.preview_key = None;
                } else if Some(id) == h.seed_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let val = tf.value().to_owned();
                    st.seed_text = val.clone();
                    if let Ok(parsed) = val.trim().parse::<u64>() {
                        if parsed != st.seed {
                            push_seed_history(st, st.seed);
                            st.seed = parsed;
                        }
                        st.auto_seed = false;
                    }
                } else if Some(id) == h.mat_search_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let query = tf.value().to_owned();
                    st.mat_search = query.clone();
                    st.picker_scroll = 0;
                    refresh_picker_search_widgets(
                        screen,
                        mat_slot,
                        all_blocks,
                        &world_blocks,
                        mat_filter,
                        &query,
                        &pal_snapshot,
                    );
                } else if Some(id) == h.preset_search_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let query = tf.value().to_owned();
                    st.preset_search = query.clone();
                    st.preset_scroll = 0;
                    refresh_preset_search_widgets(
                        screen,
                        preset_cat_filter,
                        &query,
                        active_preset,
                        cur_lang,
                    );
                }
            }
            Outcome::Submitted(id) => {
                if Some(id) == h.desc_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    st.description = tf.value().to_owned();
                    sync_palette_from_ui(st);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.seed_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let val = tf.value().to_owned();
                    st.seed_text = val.clone();
                    if let Ok(parsed) = val.trim().parse::<u64>() {
                        if parsed != st.seed {
                            push_seed_history(st, st.seed);
                            st.seed = parsed;
                        }
                        st.auto_seed = false;
                        st.dirty = true;
                    }
                } else if Some(id) == h.mat_search_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let query = tf.value().to_owned();
                    st.mat_search = query.clone();
                    st.picker_scroll = 0;
                    refresh_picker_search_widgets(
                        screen,
                        mat_slot,
                        all_blocks,
                        &world_blocks,
                        mat_filter,
                        &query,
                        &pal_snapshot,
                    );
                } else if Some(id) == h.preset_search_field
                    && let Some(tf) = screen.widgets.text_field(id)
                {
                    let query = tf.value().to_owned();
                    st.preset_search = query.clone();
                    st.preset_scroll = 0;
                    refresh_preset_search_widgets(
                        screen,
                        preset_cat_filter,
                        &query,
                        active_preset,
                        cur_lang,
                    );
                }
            }
            Outcome::Pressed(id) => {
                if id == h.lang_btn {
                    let next_lang = st.lang.toggle();
                    apply_ui_lang(st, next_lang, true);
                } else if id == h.bom_btn {
                    st.bom_open = !st.bom_open;
                    st.picker_open = false;
                    st.preset_picker_open = false;
                    st.bom_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.bom_back_btn {
                    st.bom_open = false;
                    st.bom_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.bom_copy_btn {
                    let txt = format_bom_clipboard(st);
                    st.pending = Some(PendingAction::CopyBomText(txt));
                } else if Some(id) == h.bom_up_btn {
                    st.bom_scroll = st.bom_scroll.saturating_sub(2);
                    st.dirty = true;
                } else if Some(id) == h.bom_down_btn {
                    let max_s = screen
                        .bom_scroll_zone
                        .map_or(0, |z| z.total_rows.saturating_sub(z.vis_rows));
                    st.bom_scroll = (st.bom_scroll + 2).min(max_s);
                    st.dirty = true;
                } else if Some(id) == h.tab_size {
                    st.tab = UiTab::SizeArea;
                    st.dirty = true;
                } else if Some(id) == h.tab_desc {
                    st.tab = UiTab::DescNoise;
                    st.dirty = true;
                } else if Some(id) == h.tab_mats {
                    st.tab = UiTab::Materials;
                    st.dirty = true;
                } else if Some(id) == h.tab_world {
                    st.tab = UiTab::WorldDecor;
                    st.dirty = true;
                } else if Some(id) == h.living_decor_btn {
                    st.living_decor = !st.living_decor;
                    st.bp_cache_key = None;
                    st.preview_key = None;
                    st.status = match (st.lang, st.living_decor) {
                        (UiLang::En, true) => "Living Decor: ON (stands, frames, banners, signs, lamps)".into(),
                        (UiLang::En, false) => "Living Decor: OFF".into(),
                        (UiLang::Ru, true) => "Живой декор: ВКЛ (стойки, рамки, флаги, таблички, лампы)".into(),
                        (UiLang::Ru, false) => "Живой декор: ВЫКЛ".into(),
                    };
                    st.dirty = true;
                } else if Some(id) == h.spawn_npcs_btn {
                    st.spawn_npcs = !st.spawn_npcs;
                    st.bp_cache_key = None;
                    st.preview_key = None;
                    st.status = match (st.lang, st.spawn_npcs) {
                        (UiLang::En, true) => "Spawn NPCs/Mobs: ON".into(),
                        (UiLang::En, false) => "Spawn NPCs/Mobs: OFF".into(),
                        (UiLang::Ru, true) => "Спавн NPC/Мобов: ВКЛ".into(),
                        (UiLang::Ru, false) => "Спавн NPC/Мобов: ВЫКЛ".into(),
                    };
                    st.dirty = true;
                } else if Some(id) == h.blend_radius_btn {
                    st.blend_radius = match st.blend_radius {
                        0 => 3,
                        1..=3 => 6,
                        4..=6 => 8,
                        _ => 0,
                    };
                    st.bp_cache_key = None;
                    st.preview_key = None;
                    st.status = match (st.lang, st.blend_radius) {
                        (UiLang::En, 0) => "Outer Edge Blend: OFF".into(),
                        (UiLang::En, r) => format!("Outer Edge Blend: {r} blocks"),
                        (UiLang::Ru, 0) => "Сглаживание краёв: ВЫКЛ".into(),
                        (UiLang::Ru, r) => format!("Сглаживание краёв: {r} бл."),
                    };
                    st.dirty = true;
                } else if Some(id) == h.road_width_btn {
                    st.road_width = if st.road_width == 3 { 5 } else { 3 };
                    st.road_preview_cache = None;
                    st.status = match st.lang {
                        UiLang::En => format!("Road Width: {} blocks", st.road_width),
                        UiLang::Ru => format!("Ширина дороги: {} бл.", st.road_width),
                    };
                    st.dirty = true;
                } else if Some(id) == h.road_auto_bridge_btn {
                    st.road_auto_bridge = !st.road_auto_bridge;
                    st.road_preview_cache = None;
                    st.status = match (st.lang, st.road_auto_bridge) {
                        (UiLang::En, true) => "Auto-Bridge: ON (spans water & gaps)".into(),
                        (UiLang::En, false) => "Auto-Bridge: OFF (terrain hugging)".into(),
                        (UiLang::Ru, true) => "Авто-мост: ВКЛ (через воду и обрывы)".into(),
                        (UiLang::Ru, false) => "Авто-мост: ВЫКЛ (по рельефу)".into(),
                    };
                    st.dirty = true;
                } else if Some(id) == h.road_style_btn {
                    st.style = st.style.next();
                    if st.style != BuildStyle::Auto {
                        st.palette_manual = false;
                    }
                    sync_palette_from_ui(st);
                    st.road_preview_cache = None;
                    st.preview_key = None;
                    st.status = match st.lang {
                        UiLang::En => format!("Road Style: {} ({})", st.style.label(st.lang), st.palette.display_name(st.lang)),
                        UiLang::Ru => format!("Стиль дороги: {} ({})", st.style.label(st.lang), st.palette.display_name(st.lang)),
                    };
                    st.dirty = true;
                } else if Some(id) == h.road_connect_btn {
                    st.pending = Some(PendingAction::StartRoadSelect);
                } else if Some(id) == h.w_dec {
                    st.width = st.width.saturating_sub(2).max(7);
                    st.dirty = true;
                } else if Some(id) == h.w_inc {
                    let step = if st.width >= 48 { 4 } else { 2 };
                    st.width = (st.width + step).min(96);
                    st.dirty = true;
                } else if Some(id) == h.d_dec {
                    st.depth = st.depth.saturating_sub(2).max(7);
                    st.dirty = true;
                } else if Some(id) == h.d_inc {
                    let step = if st.depth >= 48 { 4 } else { 2 };
                    st.depth = (st.depth + step).min(96);
                    st.dirty = true;
                } else if Some(id) == h.h_dec {
                    st.height = st.height.saturating_sub(1).max(5);
                    st.dirty = true;
                } else if Some(id) == h.h_inc {
                    st.height = (st.height + 1).min(48);
                    st.dirty = true;
                } else if Some(id) == h.preset_small {
                    (st.width, st.depth, st.height) = (9, 9, 8);
                    st.dirty = true;
                } else if Some(id) == h.preset_med {
                    (st.width, st.depth, st.height) = (13, 11, 12);
                    st.dirty = true;
                } else if Some(id) == h.preset_large {
                    (st.width, st.depth, st.height) = (19, 15, 16);
                    st.dirty = true;
                } else if Some(id) == h.preset_tower {
                    (st.width, st.depth, st.height) = (9, 9, 22);
                    st.category = BuildCategory::Tower;
                    st.dirty = true;
                } else if Some(id) == h.place_front {
                    st.pending = Some(PendingAction::PlaceInFront);
                    st.dirty = true;
                } else if Some(id) == h.pick_world {
                    st.pending = Some(PendingAction::StartWorldSelect);
                } else if Some(id) == h.rotate_btn {
                    st.facing = st.facing.next();
                    st.dirty = true;
                } else if Some(id) == h.y_dec_btn {
                    if st.origin.is_none() {
                        compute_origin_in_front(host, st);
                    }
                    if let Some(o) = &mut st.origin {
                        o[1] -= 1;
                    }
                    st.dirty = true;
                } else if Some(id) == h.y_inc_btn {
                    if st.origin.is_none() {
                        compute_origin_in_front(host, st);
                    }
                    if let Some(o) = &mut st.origin {
                        o[1] += 1;
                    }
                    st.dirty = true;
                } else if Some(id) == h.terrain_btn {
                    st.adapt_terrain = !st.adapt_terrain;
                    st.dirty = true;
                } else if Some(id) == h.preview_btn {
                    st.show_preview_box = !st.show_preview_box;
                    st.saved_overlay_vis = None;
                    st.dirty = true;
                } else if Some(id) == h.air_only_btn {
                    st.air_only = !st.air_only;
                    st.world_mask_key = None;
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.frame_btn {
                    st.show_frame_box = !st.show_frame_box;
                    st.saved_overlay_vis = None;
                    st.dirty = true;
                } else if Some(id) == h.collision_btn {
                    st.show_collisions = !st.show_collisions;
                    st.world_mask_key = None;
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.move_preview_btn {
                    st.pending = Some(PendingAction::StartMovePreview);
                } else if Some(id) == h.auto_hide_btn {
                    st.auto_hide_on_build = !st.auto_hide_on_build;
                    st.dirty = true;
                } else if Some(id) == h.slice_toggle_btn {
                    st.preview_slice_y = match st.preview_slice_y {
                        None => Some((i32::from(st.height) / 3).clamp(2, 10)),
                        Some(_) => None,
                    };
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.slice_dec_btn {
                    if let Some(sy) = &mut st.preview_slice_y {
                        *sy = (*sy - 1).max(0);
                        st.preview_key = None;
                        st.dirty = true;
                    }
                } else if Some(id) == h.slice_inc_btn {
                    if let Some(sy) = &mut st.preview_slice_y {
                        *sy = (*sy + 1).min(i32::from(st.height).saturating_sub(1));
                        st.preview_key = None;
                        st.dirty = true;
                    }
                } else if Some(id) == h.fav_save_btn {
                    if let Some(df_id) = h.desc_field
                        && let Some(tf) = screen.widgets.text_field(df_id)
                    {
                        st.description = tf.value().to_owned();
                    }
                    let name = format!("{} ({})", st.category.label(st.lang), st.style.label(st.lang));
                    let fav = current_saved_preset(st, name);
                    if st.favorites.len() >= MAX_SAVED_FAVORITES {
                        st.favorites.remove(0);
                    }
                    st.favorites.push(fav);
                    st.fav_idx = st.favorites.len().saturating_sub(1);
                    st.pending_save_favorites = true;
                    st.status = match st.lang {
                        UiLang::En => format!("Saved to disk (#{}) — seed {}", st.favorites.len(), st.seed),
                        UiLang::Ru => format!("Сохранено на диск (#{}) — сид {}", st.favorites.len(), st.seed),
                    };
                    st.dirty = true;
                } else if Some(id) == h.fav_load_btn {
                    if st.favorites.is_empty() {
                        st.status = tr(
                            st.lang,
                            "Favorites list is empty (click \"★ Save\").",
                            "Список избранного пока пуст (нажмите «★ Сохр.»).",
                        )
                        .into();
                    } else {
                        let idx = st.fav_idx % st.favorites.len();
                        let fav = st.favorites[idx].clone();
                        st.fav_idx = (idx + 1) % st.favorites.len();
                        apply_saved_preset(st, fav);
                        st.status = match st.lang {
                            UiLang::En => format!("Loaded favorite #{}/{} (seed {})", idx + 1, st.favorites.len(), st.seed),
                            UiLang::Ru => format!("Загружено избр. #{}/{} (сид {})", idx + 1, st.favorites.len(), st.seed),
                        };
                        st.dirty = true;
                    }
                } else if Some(id) == h.fav_del_btn {
                    if !st.favorites.is_empty() {
                        let idx = st.fav_idx % st.favorites.len();
                        st.favorites.remove(idx);
                        if st.fav_idx >= st.favorites.len() && !st.favorites.is_empty() {
                            st.fav_idx = st.favorites.len() - 1;
                        } else if st.favorites.is_empty() {
                            st.fav_idx = 0;
                        }
                        st.pending_save_favorites = true;
                        st.status = match st.lang {
                            UiLang::En => format!("Deleted favorite ({} remaining).", st.favorites.len()),
                            UiLang::Ru => format!("Запись удалена (осталось {}).", st.favorites.len()),
                        };
                        st.dirty = true;
                    }
                } else if Some(id) == h.share_copy_btn {
                    if let Some(df_id) = h.desc_field
                        && let Some(tf) = screen.widgets.text_field(df_id)
                    {
                        st.description = tf.value().to_owned();
                    }
                    let preset = current_saved_preset(
                        st,
                        format!("{} ({})", st.category.label(st.lang), st.style.label(st.lang)),
                    );
                    st.pending = Some(PendingAction::CopyShareCode(preset));
                } else if Some(id) == h.share_paste_btn {
                    st.pending = Some(PendingAction::PasteShareCode);
                } else if Some(id) == h.cat_btn {
                    st.category = st.category.next();
                    if st.category == BuildCategory::Settlement && (st.width < 36 || st.depth < 36) {
                        st.width = st.width.max(41);
                        st.depth = st.depth.max(41);
                        st.height = st.height.max(18);
                        st.status = tr(
                            st.lang,
                            "District: area expanded to 41x41 (max 96x96).",
                            "Квартал: размер расширен до 41x41 (макс. 96x96).",
                        )
                        .into();
                    }
                    st.dirty = true;
                } else if Some(id) == h.style_btn {
                    st.style = st.style.next();
                    if st.style != BuildStyle::Auto {
                        st.palette_manual = false;
                    }
                    sync_palette_from_ui(st);
                    st.dirty = true;
                } else if Some(id) == h.roof_btn {
                    st.roof_choice = st.roof_choice.next();
                    st.dirty = true;
                } else if Some(id) == h.noise_btn {
                    st.noise_level = st.noise_level.next();
                    st.dirty = true;
                } else if Some(id) == h.biome_btn {
                    st.use_biome_auto = !st.use_biome_auto;
                    st.last_biome_query = None;
                    sync_palette_from_ui(st);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.wear_dec_btn {
                    st.wear_pct = st.wear_pct.saturating_sub(15);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.wear_inc_btn {
                    st.wear_pct = (st.wear_pct + 15).min(100);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.wear_btn {
                    st.wear_pct = match st.wear_pct {
                        0..=24 => 25,
                        25..=49 => 50,
                        50..=74 => 75,
                        75..=99 => 100,
                        _ => 0,
                    };
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.over_dec_btn {
                    st.overgrowth_pct = st.overgrowth_pct.saturating_sub(15);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.over_inc_btn {
                    st.overgrowth_pct = (st.overgrowth_pct + 15).min(100);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.over_btn {
                    st.overgrowth_pct = match st.overgrowth_pct {
                        0..=24 => 25,
                        25..=49 => 50,
                        50..=74 => 75,
                        75..=99 => 100,
                        _ => 0,
                    };
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.prev_seed_btn {
                    if let Some(prev) = st.seed_history.pop() {
                        st.seed = prev;
                        st.seed_text = prev.to_string();
                        st.auto_seed = false;
                        st.status = match st.lang {
                            UiLang::En => format!("Previous seed: {prev} (locked)"),
                            UiLang::Ru => format!("Предыдущий сид: {prev} (сид зафиксирован)"),
                        };
                    } else {
                        st.status = tr(st.lang, "Seed history is empty.", "История сидов пуста.").into();
                    }
                    st.dirty = true;
                } else if Some(id) == h.new_seed_btn {
                    push_seed_history(st, st.seed);
                    st.build_seq = st.build_seq.wrapping_add(1);
                    st.seed = crate::noise::fresh_entropy(st.build_seq, host.millis());
                    st.seed_text = st.seed.to_string();
                    sync_palette_from_ui(st);
                    st.preview_key = None;
                    st.dirty = true;
                } else if Some(id) == h.auto_seed_btn {
                    st.auto_seed = !st.auto_seed;
                    st.dirty = true;
                } else if Some(id) == h.archetype_prev_btn {
                    apply_prev_archetype(st);
                    st.dirty = true;
                } else if Some(id) == h.archetype_btn {
                    st.preset_picker_open = true;
                    st.preset_scroll = 0;
                    st.preset_search.clear();
                    st.dirty = true;
                } else if Some(id) == h.archetype_next_btn {
                    apply_next_archetype(st);
                    st.dirty = true;
                } else if let Some(&(_, phrase)) = h.tag_buttons.iter().find(|&&(bid, _)| bid == id) {
                    append_tag_to_description(st, phrase);
                    sync_palette_from_ui(st);
                    st.dirty = true;
                } else if let Some(&(_, idx)) = h.feature_toggles.iter().find(|&&(bid, _)| bid == id) {
                    st.toggles[idx] = st.toggles[idx].next();
                    st.dirty = true;
                } else if Some(id) == h.mat_preset_btn {
                    st.palette_preset_idx = (st.palette_preset_idx + 1) % MaterialPalette::PRESET_COUNT;
                    st.palette = MaterialPalette::preset(st.palette_preset_idx);
                    st.palette_manual = true;
                    st.mat_list_scroll = 0;
                    st.status = match st.lang {
                        UiLang::En => format!("Palette #{}: {}", st.palette_preset_idx + 1, st.palette.display_name(st.lang)),
                        UiLang::Ru => format!("Палитра #{}: {}", st.palette_preset_idx + 1, st.palette.display_name(st.lang)),
                    };
                    st.dirty = true;
                } else if Some(id) == h.mat_infer_btn {
                    st.palette_manual = false;
                    sync_palette_from_ui(st);
                    st.mat_list_scroll = 0;
                    st.preview_key = None;
                    st.status = match st.lang {
                        UiLang::En => format!("Blocks inferred from text: {}", st.palette.display_name(st.lang)),
                        UiLang::Ru => format!("Блоки подобраны по тексту: {}", st.palette.display_name(st.lang)),
                    };
                    st.dirty = true;
                } else if Some(id) == h.mat_sync_wood_btn {
                    st.auto_sync_wood = !st.auto_sync_wood;
                    st.dirty = true;
                } else if Some(id) == h.mat_mix_mode_btn {
                    st.palette.mix_mode = st.palette.mix_mode.next();
                    st.palette_manual = true;
                    st.preview_key = None;
                    st.status = match st.lang {
                        UiLang::En => format!("Mix mode: {}", st.palette.mix_mode.label(st.lang)),
                        UiLang::Ru => format!("Режим смешивания: {}", st.palette.mix_mode.label(st.lang)),
                    };
                    st.dirty = true;
                } else if Some(id) == h.mat_slot_up_btn {
                    st.mat_slot_scroll = st.mat_slot_scroll.saturating_sub(3);
                    st.dirty = true;
                } else if Some(id) == h.mat_slot_down_btn {
                    let max_s = screen
                        .slot_scroll_zone
                        .map_or(20, |z| z.total_rows.saturating_sub(z.vis_rows));
                    st.mat_slot_scroll = (st.mat_slot_scroll + 3).min(max_s);
                    st.dirty = true;
                } else if let Some(&(_, slot)) = h.mat_slot_btns.iter().find(|&&(bid, _)| bid == id) {
                    st.selected_mat_slot = slot;
                    st.mat_list_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.list_up_btn {
                    st.mat_list_scroll = st.mat_list_scroll.saturating_sub(1);
                    st.dirty = true;
                } else if Some(id) == h.list_down_btn {
                    let max_s = screen
                        .list_scroll_zone
                        .map_or(0, |z| z.total_rows.saturating_sub(z.vis_rows));
                    st.mat_list_scroll = (st.mat_list_scroll + 1).min(max_s);
                    st.dirty = true;
                } else if Some(id) == h.list_clear_btn {
                    st.palette.clear_slot_extras(st.selected_mat_slot);
                    st.palette.name = "Custom Palette".into();
                    st.palette_manual = true;
                    st.mat_list_scroll = 0;
                    st.preview_key = None;
                    st.status = match st.lang {
                        UiLang::En => format!("{}: trimmed list to 1 main block", slot_label(st.selected_mat_slot, st.lang)),
                        UiLang::Ru => format!("{}: список сокращён до 1 основного блока", slot_label(st.selected_mat_slot, st.lang)),
                    };
                    st.dirty = true;
                } else if Some(id) == h.list_pop_btn {
                    let len = st.palette.slot_blocks(st.selected_mat_slot).len();
                    if len > 1 && remove_block_from_slot_at(&mut st.palette, st.selected_mat_slot, len - 1) {
                        st.palette_manual = true;
                        st.preview_key = None;
                        let rem = st.palette.slot_blocks(st.selected_mat_slot).len();
                        st.mat_list_scroll = st.mat_list_scroll.min(rem.saturating_sub(1));
                        st.status = match st.lang {
                            UiLang::En => format!("{}: removed last block ({rem} left)", slot_label(st.selected_mat_slot, st.lang)),
                            UiLang::Ru => format!("{}: убран последний блок (осталось {rem})", slot_label(st.selected_mat_slot, st.lang)),
                        };
                        st.dirty = true;
                    }
                } else if let Some(&(_, idx)) = h.list_remove_btns.iter().find(|&&(bid, _)| bid == id) {
                    if remove_block_from_slot_at(&mut st.palette, st.selected_mat_slot, idx) {
                        st.palette_manual = true;
                        st.preview_key = None;
                        let rem = st.palette.slot_blocks(st.selected_mat_slot).len();
                        st.mat_list_scroll = st.mat_list_scroll.min(rem.saturating_sub(1));
                        st.status = match st.lang {
                            UiLang::En => format!("{}: removed item #{} ({rem} left)", slot_label(st.selected_mat_slot, st.lang), idx + 1),
                            UiLang::Ru => format!("{}: удалена позиция #{} (осталось {rem})", slot_label(st.selected_mat_slot, st.lang), idx + 1),
                        };
                        st.dirty = true;
                    }
                } else if let Some(&(_, idx)) = h.list_item_btns.iter().find(|&&(bid, _)| bid == id) {
                    st.picker_open = true;
                    st.picker_replace_idx = Some(idx);
                    st.picker_scroll = 0;
                    st.mat_search.clear();
                    st.dirty = true;
                } else if Some(id) == h.picker_add_open_btn {
                    st.picker_open = true;
                    st.picker_replace_idx = None;
                    st.picker_scroll = 0;
                    st.mat_search.clear();
                    st.dirty = true;
                } else if Some(id) == h.picker_back_btn {
                    st.picker_open = false;
                    st.picker_scroll = 0;
                    st.mat_search.clear();
                    st.dirty = true;
                } else if Some(id) == h.picker_mode_btn {
                    st.picker_replace_idx = if st.picker_replace_idx.is_some() {
                        None
                    } else {
                        Some(0)
                    };
                    st.dirty = true;
                } else if Some(id) == h.picker_catalog_btn {
                    st.picker_all_blocks = !st.picker_all_blocks;
                    st.picker_scroll = 0;
                    st.dirty = true;
                } else if let Some(&(_, filt)) = h.mat_filter_btns.iter().find(|&&(bid, _)| bid == id) {
                    st.mat_filter = filt;
                    st.picker_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.mat_search_clear_btn {
                    st.mat_search.clear();
                    st.picker_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.picker_up_btn {
                    st.picker_scroll = st.picker_scroll.saturating_sub(2);
                    st.dirty = true;
                } else if Some(id) == h.picker_down_btn {
                    let max_s = screen
                        .picker_scroll_zone
                        .map_or(0, |z| z.total_rows.saturating_sub(z.vis_rows));
                    st.picker_scroll = (st.picker_scroll + 2).min(max_s);
                    st.dirty = true;
                } else if let Some(&(_, idx)) = h.picker_chip_btns.iter().find(|&&(bid, _)| bid == id) {
                    if st.picker_replace_idx.is_some() {
                        st.picker_replace_idx = Some(idx);
                        st.dirty = true;
                    } else {
                        let len = st.palette.slot_blocks(st.selected_mat_slot).len();
                        if len > 1 && remove_block_from_slot_at(&mut st.palette, st.selected_mat_slot, idx) {
                            st.palette_manual = true;
                            st.preview_key = None;
                            st.status = match st.lang {
                                UiLang::En => format!("{}: removed item #{} ({} left)", slot_label(st.selected_mat_slot, st.lang), idx + 1, len - 1),
                                UiLang::Ru => format!("{}: убран из списка #{} (осталось {})", slot_label(st.selected_mat_slot, st.lang), idx + 1, len - 1),
                            };
                            st.dirty = true;
                        } else {
                            st.picker_replace_idx = Some(idx);
                            st.dirty = true;
                        }
                    }
                } else if let Some((_, cand_id)) = h.picker_grid_btns.iter().find(|&&(bid, _)| bid == id) {
                    if !cand_id.is_empty() {
                        let chosen = cand_id.clone();
                        if let Some(rep_idx) = st.picker_replace_idx {
                            if rep_idx == 0 {
                                apply_block_to_slot(
                                    &mut st.palette,
                                    st.selected_mat_slot,
                                    &chosen,
                                    st.auto_sync_wood,
                                );
                            } else {
                                replace_block_in_slot_at(
                                    &mut st.palette,
                                    st.selected_mat_slot,
                                    rep_idx,
                                    &chosen,
                                    st.auto_sync_wood,
                                );
                            }
                            st.palette_manual = true;
                            st.preview_key = None;
                            st.picker_open = false;
                            st.picker_scroll = 0;
                            st.mat_search.clear();
                            st.status = format!(
                                "{} #{} -> {}",
                                slot_label(st.selected_mat_slot, st.lang),
                                rep_idx + 1,
                                short_block_name(&chosen)
                            );
                            st.dirty = true;
                        } else {
                            let before = st.palette.slot_blocks(st.selected_mat_slot).len();
                            let _ = toggle_block_in_slot(
                                &mut st.palette,
                                st.selected_mat_slot,
                                &chosen,
                                st.auto_sync_wood,
                            );
                            let after = st.palette.slot_blocks(st.selected_mat_slot).len();
                            st.palette_manual = after != before || st.palette_manual;
                            st.preview_key = None;
                            st.status = if after > before {
                                match st.lang {
                                    UiLang::En => format!("+ {}: added {} ({after} total)", slot_label(st.selected_mat_slot, st.lang), short_block_name(&chosen)),
                                    UiLang::Ru => format!("+ {}: добавлен {} (всего {after})", slot_label(st.selected_mat_slot, st.lang), short_block_name(&chosen)),
                                }
                            } else if after < before {
                                match st.lang {
                                    UiLang::En => format!("- {}: removed {} ({after} left)", slot_label(st.selected_mat_slot, st.lang), short_block_name(&chosen)),
                                    UiLang::Ru => format!("- {}: убран {} (осталось {after})", slot_label(st.selected_mat_slot, st.lang), short_block_name(&chosen)),
                                }
                            } else if before >= MaterialPalette::MAX_BLOCKS_PER_SLOT {
                                match st.lang {
                                    UiLang::En => format!("Limit {} blocks in slot {}", MaterialPalette::MAX_BLOCKS_PER_SLOT, slot_label(st.selected_mat_slot, st.lang)),
                                    UiLang::Ru => format!("Лимит {} блоков в слоте {}", MaterialPalette::MAX_BLOCKS_PER_SLOT, slot_label(st.selected_mat_slot, st.lang)),
                                }
                            } else {
                                tr(st.lang, "At least 1 block must remain in the list", "В списке должен остаться минимум 1 блок").into()
                            };
                            st.dirty = true;
                        }
                    }
                } else if Some(id) == h.preset_back_btn {
                    st.preset_picker_open = false;
                    st.preset_scroll = 0;
                    st.preset_search.clear();
                    st.dirty = true;
                } else if Some(id) == h.preset_cat_btn {
                    st.preset_cat_filter = next_preset_cat_filter(st.preset_cat_filter);
                    st.preset_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.preset_search_clear_btn {
                    st.preset_search.clear();
                    st.preset_scroll = 0;
                    st.dirty = true;
                } else if Some(id) == h.preset_up_btn {
                    st.preset_scroll = st.preset_scroll.saturating_sub(2);
                    st.dirty = true;
                } else if Some(id) == h.preset_down_btn {
                    let max_s = screen
                        .preset_scroll_zone
                        .map_or(0, |z| z.total_rows.saturating_sub(z.vis_rows));
                    st.preset_scroll = (st.preset_scroll + 2).min(max_s);
                    st.dirty = true;
                } else if let Some(&(_, preset_idx)) = h.preset_grid_btns.iter().find(|&&(bid, _)| bid == id) {
                    if preset_idx < CLI_PRESETS.len() {
                        apply_preset_by_idx(st, preset_idx);
                        st.preset_picker_open = false;
                        st.preset_scroll = 0;
                        st.preset_search.clear();
                        st.dirty = true;
                    }
                } else if id == h.generate_btn {
                    if let Some(df_id) = h.desc_field
                        && let Some(tf) = screen.widgets.text_field(df_id)
                    {
                        st.description = tf.value().to_owned();
                    }
                    if let Some(sf_id) = h.seed_field
                        && let Some(tf) = screen.widgets.text_field(sf_id)
                    {
                        let val = tf.value().to_owned();
                        if !st.auto_seed
                            && let Ok(parsed) = val.trim().parse::<u64>()
                        {
                            st.seed = parsed;
                            st.seed_text = val;
                        }
                    }
                    queue_build_from_state(host, st);
                } else if id == h.reroll_btn {
                    st.status = tr(st.lang, "Rerolling with a new seed...", "Перегенерация с новым шумом...").into();
                    st.pending = Some(PendingAction::SendReroll(st.dimension.clone()));
                } else if id == h.undo_btn {
                    st.status = tr(st.lang, "Undoing last build...", "Отмена последней постройки...").into();
                    st.pending = Some(PendingAction::SendUndo(st.dimension.clone()));
                } else if id == h.redo_btn {
                    st.status = tr(st.lang, "Redoing undone build...", "Возврат отменённой постройки...").into();
                    st.pending = Some(PendingAction::SendRedo(st.dimension.clone()));
                } else if id == h.close_btn {
                    st.pending = Some(PendingAction::CloseMenu);
                }
            }
            _ => {}
        }

        if outcome.handled() {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    })
}

fn facing_forward_xz(f: CardinalFacing) -> [i32; 2] {
    match f {
        CardinalFacing::North => [0, -1],
        CardinalFacing::South => [0, 1],
        CardinalFacing::East => [1, 0],
        CardinalFacing::West => [-1, 0],
    }
}

fn queue_build_from_state(host: &dyn ClientHost, st: &mut ClientUiState) {
    st.build_seq = st.build_seq.wrapping_add(1);
    if st.auto_seed {
        push_seed_history(st, st.seed);
        st.seed = crate::noise::fresh_entropy(st.build_seq, host.millis());
        st.seed_text = st.seed.to_string();
        st.dirty = true;
    }
    sync_palette_from_ui(st);
    st.preview_key = None;
    if let Some(cam) = host.camera() {
        st.dimension = cam.dimension.clone();
    }
    if st.origin.is_none() {
        compute_origin_in_front(host, st);
    }
    let origin = st.origin.unwrap_or([0, 64, 0]);
    let req = BuildRequest {
        dimension: st.dimension.clone(),
        origin_x: origin[0],
        origin_y: origin[1],
        origin_z: origin[2],
        width: st.width,
        depth: st.depth,
        height: st.height,
        category: st.category,
        style: st.style,
        facing: st.facing,
        noise_level: st.noise_level,
        seed: st.seed as i64,
        description: effective_description(st),
        adapt_terrain: st.adapt_terrain,
        air_only: st.air_only,
        wear_pct: st.wear_pct,
        overgrowth_pct: st.overgrowth_pct,
        biome_hint: if st.use_biome_auto {
            st.detected_biome.clone()
        } else {
            String::new()
        },
        palette: st.palette.clone(),
        living_decor: st.living_decor,
        spawn_npcs: st.spawn_npcs,
        blend_radius: if st.adapt_terrain { st.blend_radius } else { 0 },
    };
    st.status = match st.lang {
        UiLang::En => format!("Build (seed {}) sent...", st.seed),
        UiLang::Ru => format!("Генерация (сид {}) отправлена...", st.seed),
    };
    st.pending = Some(PendingAction::SendBuild(req));
}

pub fn on_input_key(host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
    if event.action != KeyAction::Press {
        return;
    }

    if event.key == "key.keyboard.f8" {
        if host.mod_screen() == Some(SCREEN_ID) {
            host.mod_screen_close(SCREEN_ID);
            return;
        }
        if host.playing() {
            with_state(|st| {
                st.world_select = WorldSelectMode::Inactive;
                if st.origin.is_none() {
                    compute_origin_in_front(host, st);
                }
                st.dirty = true;
            });
            host.mod_screen_open(SCREEN_ID);
            return;
        }
    }

    if !host.playing() {
        return;
    }

    let shift_down = (event.modifiers & 1) != 0;

    if event.key == "key.keyboard.h" && host.mod_screen().is_none() {
        let now = host.millis();
        with_state(|st| {
            if shift_down {
                st.show_frame_box = !st.show_frame_box;
                st.hud_toast = Some((
                    match (st.lang, st.show_frame_box) {
                        (UiLang::En, true) => "Blue frame: ON".into(),
                        (UiLang::En, false) => "Blue frame: OFF (3D View kept)".into(),
                        (UiLang::Ru, true) => "Синяя рамка: ВКЛ".into(),
                        (UiLang::Ru, false) => "Синяя рамка: ВЫКЛ (3D-обзор сохранён)".into(),
                    },
                    now + 2200,
                ));
            } else if st.show_frame_box || st.show_preview_box {
                st.saved_overlay_vis = Some((st.show_frame_box, st.show_preview_box));
                st.show_frame_box = false;
                st.show_preview_box = false;
                st.hud_toast = Some((
                    tr(
                        st.lang,
                        "Frame & 3D hidden — [H] to restore",
                        "Рамка и 3D скрыты — [H] вернуть",
                    )
                    .into(),
                    now + 2200,
                ));
            } else {
                let (rf, rp) = st.saved_overlay_vis.take().unwrap_or((true, true));
                st.show_frame_box = if !rf && !rp { true } else { rf };
                st.show_preview_box = rp;
                if st.origin.is_none() {
                    compute_origin_in_front(host, st);
                }
                st.hud_toast = Some((
                    tr(
                        st.lang,
                        "Frame & 3D restored ([Shift+H] frame only)",
                        "Рамка и 3D включены ([Shift+H] только рамка)",
                    )
                    .into(),
                    now + 2200,
                ));
            }
            st.dirty = true;
        });
        return;
    }

    let mode = with_state(|st| st.world_select);
    if mode == WorldSelectMode::Inactive {
        return;
    }

    if mode == WorldSelectMode::MovePreview {
        let cam_face = host
            .camera()
            .map(|c| CardinalFacing::from_yaw(c.yaw))
            .unwrap_or(CardinalFacing::South);
        let [fx, fz] = facing_forward_xz(cam_face);
        let step = if shift_down { 4 } else { 1 };
        let nudge: Option<[i32; 3]> = match event.key {
            "key.keyboard.up" | "key.keyboard.i" => Some([fx * step, 0, fz * step]),
            "key.keyboard.down" | "key.keyboard.k" => Some([-fx * step, 0, -fz * step]),
            "key.keyboard.left" | "key.keyboard.j" => Some([fz * step, 0, -fx * step]),
            "key.keyboard.right" | "key.keyboard.l" => Some([-fz * step, 0, fx * step]),
            "key.keyboard.page.up" | "key.keyboard.page_up" | "key.keyboard.equal" => Some([0, step, 0]),
            "key.keyboard.page.down" | "key.keyboard.page_down" | "key.keyboard.minus" => Some([0, -step, 0]),
            _ => None,
        };
        if let Some([dx, dy, dz]) = nudge {
            with_state(|st| {
                if st.origin.is_none() {
                    compute_origin_in_front(host, st);
                }
                if let Some(o) = &mut st.origin {
                    o[0] += dx;
                    o[1] += dy;
                    o[2] += dz;
                    st.world_mask_key = None;
                    st.terrain_cache_key = None;
                    st.status = format!("Pos: {}, {}, {}", o[0], o[1], o[2]);
                }
                st.dirty = true;
            });
            return;
        }
        if event.key == "key.keyboard.c" {
            with_state(|st| {
                st.show_collisions = !st.show_collisions;
                st.world_mask_key = None;
                st.preview_key = None;
                st.status = tr(
                    st.lang,
                    if st.show_collisions { "Collisions: ON" } else { "Collisions: OFF" },
                    if st.show_collisions { "Коллизии: ВКЛ" } else { "Коллизии: ВЫКЛ" },
                )
                .into();
                st.dirty = true;
            });
            return;
        }
        if event.key == "key.keyboard.enter" || event.key == "key.keyboard.keypad.enter" {
            with_state(|st| {
                st.world_select = WorldSelectMode::Inactive;
                queue_build_from_state(host, st);
                st.dirty = true;
            });
            return;
        }
    }

    if matches!(
        mode,
        WorldSelectMode::PickingRoadStart | WorldSelectMode::PickingRoadEnd { .. }
    ) {
        if event.key == "key.keyboard.escape" {
            with_state(|st| {
                st.world_select = WorldSelectMode::Inactive;
                st.road_preview_cache = None;
                st.dirty = true;
            });
            host.mod_screen_open(SCREEN_ID);
        } else if event.key == "key.keyboard.r" {
            let now = host.millis();
            with_state(|st| {
                st.road_width = if st.road_width == 3 { 5 } else { 3 };
                st.road_preview_cache = None;
                let msg = match st.lang {
                    UiLang::En => format!("Road width: {} blocks", st.road_width),
                    UiLang::Ru => format!("Ширина дороги: {} бл.", st.road_width),
                };
                st.status = msg.clone();
                st.hud_toast = Some((msg, now + 1800));
                st.dirty = true;
            });
        } else if matches!(
            event.key,
            "key.keyboard.left.bracket"
                | "key.keyboard.left_bracket"
                | "key.keyboard.right.bracket"
                | "key.keyboard.right_bracket"
        ) {
            let next_dir = matches!(
                event.key,
                "key.keyboard.right.bracket" | "key.keyboard.right_bracket"
            );
            let now = host.millis();
            with_state(|st| {
                st.style = if next_dir { st.style.next() } else { st.style.prev() };
                if st.style != BuildStyle::Auto {
                    st.palette_manual = false;
                }
                sync_palette_from_ui(st);
                st.road_preview_cache = None;
                st.preview_key = None;
                let msg = match st.lang {
                    UiLang::En => format!("Road style: {}", st.style.label(st.lang)),
                    UiLang::Ru => format!("Стиль дороги: {}", st.style.label(st.lang)),
                };
                st.status = msg.clone();
                st.hud_toast = Some((msg, now + 1800));
                st.dirty = true;
            });
        }
        return;
    }

    if event.key == "key.keyboard.r" {
        with_state(|st| {
            st.facing = st.facing.next();
            st.status = match st.lang {
                UiLang::En => format!("Entrance facing: {}", st.facing.label(st.lang)),
                UiLang::Ru => format!("Направление входа: {}", st.facing.label(st.lang)),
            };
        });
    } else if matches!(event.key, "key.keyboard.page.up" | "key.keyboard.page_up") {
        with_state(|st| {
            st.height = (st.height + 1).min(48);
            st.status = match st.lang {
                UiLang::En => format!("Area height: {}", st.height),
                UiLang::Ru => format!("Высота области: {}", st.height),
            };
        });
    } else if matches!(event.key, "key.keyboard.page.down" | "key.keyboard.page_down") {
        with_state(|st| {
            st.height = st.height.saturating_sub(1).max(5);
            st.status = match st.lang {
                UiLang::En => format!("Area height: {}", st.height),
                UiLang::Ru => format!("Высота области: {}", st.height),
            };
        });
    } else if matches!(event.key, "key.keyboard.left.bracket" | "key.keyboard.left_bracket") {
        with_state(|st| {
            let next = st.preview_slice_y.map_or(Some((i32::from(st.height) / 3).max(2)), |y| {
                Some((y - 1).max(0))
            });
            st.preview_slice_y = next;
            st.preview_key = None;
            st.status = match st.lang {
                UiLang::En => format!("3D hologram slice: Y<={}", next.unwrap_or(0)),
                UiLang::Ru => format!("Срез 3D-голограммы: Y<={}", next.unwrap_or(0)),
            };
        });
    } else if matches!(event.key, "key.keyboard.right.bracket" | "key.keyboard.right_bracket") {
        with_state(|st| {
            let cur = st.preview_slice_y.unwrap_or(2) + 1;
            if cur >= i32::from(st.height) {
                st.preview_slice_y = None;
                st.status = tr(st.lang, "3D hologram slice: Full volume", "Срез 3D-голограммы: Весь объём").into();
            } else {
                st.preview_slice_y = Some(cur);
                st.status = match st.lang {
                    UiLang::En => format!("3D hologram slice: Y<={cur}"),
                    UiLang::Ru => format!("Срез 3D-голограммы: Y<={cur}"),
                };
            }
            st.preview_key = None;
        });
    } else if event.key == "key.keyboard.slash" {
        with_state(|st| {
            st.preview_slice_y = match st.preview_slice_y {
                None => Some((i32::from(st.height) / 3).clamp(2, 10)),
                Some(_) => None,
            };
            st.preview_key = None;
        });
    } else if event.key == "key.keyboard.escape" {
        with_state(|st| {
            st.world_select = WorldSelectMode::Inactive;
            st.dirty = true;
        });
        host.mod_screen_open(SCREEN_ID);
    }
}

pub fn on_mouse_button_pre(host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
    if !event.pressed || !host.playing() {
        return Flow::Continue;
    }
    let mode = with_state(|st| st.world_select);
    if mode == WorldSelectMode::Inactive {
        return Flow::Continue;
    }

    let Some(hit_pos) = current_aim_block(host) else {
        return Flow::Cancel;
    };

    let reopen = with_state(|st| {
        if let Some(cam) = host.camera() {
            st.dimension = cam.dimension.clone();
        }
        match st.world_select {
            WorldSelectMode::Inactive => false,
            WorldSelectMode::PickingCorner1 => {
                if event.button == 1 {
                    // Right-click: quick anchor placement keeping W x D x H!
                    st.origin = Some([
                        hit_pos[0] - i32::from(st.width / 2),
                        hit_pos[1],
                        hit_pos[2] - i32::from(st.depth / 2),
                    ]);
                    st.world_select = WorldSelectMode::Inactive;
                    st.status = match st.lang {
                        UiLang::En => format!("Area set: {:?}", st.origin.unwrap()),
                        UiLang::Ru => format!("Область выбрана: {:?}", st.origin.unwrap()),
                    };
                    st.dirty = true;
                    true
                } else if event.button == 0 {
                    // Left-click: set Corner 1 for 2-point area stretch!
                    st.world_select = WorldSelectMode::PickingCorner2 { c1: hit_pos };
                    false
                } else {
                    false
                }
            }
            WorldSelectMode::PickingCorner2 { c1 } => {
                if event.button == 0 || event.button == 1 {
                    let min_x = c1[0].min(hit_pos[0]);
                    let min_z = c1[2].min(hit_pos[2]);
                    let max_x = c1[0].max(hit_pos[0]);
                    let max_z = c1[2].max(hit_pos[2]);
                    let min_y = c1[1].min(hit_pos[1]);
                    st.width = ((max_x - min_x + 1) as u16).clamp(7, 96);
                    st.depth = ((max_z - min_z + 1) as u16).clamp(7, 96);
                    st.origin = Some([min_x, min_y, min_z]);
                    st.world_select = WorldSelectMode::Inactive;
                    st.status = match st.lang {
                        UiLang::En => format!("Area {}x{} @ ({}, {}, {})", st.width, st.depth, min_x, min_y, min_z),
                        UiLang::Ru => format!("Область {}x{} @ ({}, {}, {})", st.width, st.depth, min_x, min_y, min_z),
                    };
                    st.dirty = true;
                    true
                } else {
                    false
                }
            }
            WorldSelectMode::MovePreview => {
                if event.button == 0 || event.button == 1 {
                    st.origin = Some([
                        hit_pos[0] - i32::from(st.width / 2),
                        hit_pos[1],
                        hit_pos[2] - i32::from(st.depth / 2),
                    ]);
                    st.world_mask_key = None;
                    st.terrain_cache_key = None;
                    st.dirty = true;
                }
                false
            }
            WorldSelectMode::PickingRoadStart => {
                if event.button == 0 || event.button == 1 {
                    let now = host.millis();
                    st.world_select = WorldSelectMode::PickingRoadEnd { start: hit_pos };
                    st.road_preview_cache = None;
                    st.hud_toast = Some((
                        tr(
                            st.lang,
                            "Pick road end point [LMB/RMB] or [Esc] cancel",
                            "Выберите конец дороги [ЛКМ/ПКМ] или [Esc] отмена",
                        )
                        .into(),
                        now + 2600,
                    ));
                    st.status = match st.lang {
                        UiLang::En => format!(
                            "Road start @ ({}, {}, {}) — pick end point",
                            hit_pos[0], hit_pos[1], hit_pos[2]
                        ),
                        UiLang::Ru => format!(
                            "Начало дороги @ ({}, {}, {}) — выберите конец",
                            hit_pos[0], hit_pos[1], hit_pos[2]
                        ),
                    };
                }
                false
            }
            WorldSelectMode::PickingRoadEnd { start } => {
                if event.button == 0 || event.button == 1 {
                    if st.auto_seed {
                        st.build_seq = st.build_seq.wrapping_add(1);
                        push_seed_history(st, st.seed);
                        st.seed = crate::noise::fresh_entropy(st.build_seq, host.millis());
                        st.seed_text = st.seed.to_string();
                    }
                    sync_palette_from_ui(st);
                    let road_palette = if st.palette_manual || st.style == BuildStyle::Auto {
                        st.palette.clone()
                    } else {
                        MaterialPalette::preset_for_style(st.style)
                    };
                    let req = RoadConnectRequest {
                        dimension: st.dimension.clone(),
                        start_x: start[0],
                        start_y: start[1],
                        start_z: start[2],
                        end_x: hit_pos[0],
                        end_y: hit_pos[1],
                        end_z: hit_pos[2],
                        road_width: st.road_width,
                        auto_bridge: st.road_auto_bridge,
                        style: st.style,
                        seed: st.seed as i64,
                        air_only: st.air_only,
                        blend_radius: st.blend_radius,
                        living_decor: st.living_decor,
                        spawn_npcs: st.spawn_npcs,
                        biome_hint: if st.use_biome_auto {
                            st.detected_biome.clone()
                        } else {
                            String::new()
                        },
                        palette: road_palette,
                    };
                    st.world_select = WorldSelectMode::Inactive;
                    st.road_preview_cache = None;
                    let now = host.millis();
                    let style_lbl = st.style.label(st.lang);
                    let msg = match st.lang {
                        UiLang::En => format!(
                            "Road [{style_lbl}] ({},{},{}) -> ({},{},{}) [w={}] sent...",
                            start[0], start[1], start[2], hit_pos[0], hit_pos[1], hit_pos[2], st.road_width
                        ),
                        UiLang::Ru => format!(
                            "Дорога [{style_lbl}] ({},{},{}) -> ({},{},{}) [шир={}] отправлена...",
                            start[0], start[1], start[2], hit_pos[0], hit_pos[1], hit_pos[2], st.road_width
                        ),
                    };
                    st.hud_toast = Some((msg.clone(), now + 2600));
                    st.status = msg;
                    st.pending = Some(PendingAction::SendRoad(req));
                    st.dirty = true;
                    true
                } else {
                    false
                }
            }
        }
    });

    if reopen {
        host.mod_screen_open(SCREEN_ID);
    }
    Flow::Cancel
}

pub fn on_client_tick_pre(host: &mut dyn ClientHost, _: &mut ClientTickPreEvent) {
    if host.playing() {
        let need_lang_load = with_state(|st| !st.lang_loaded);
        if need_lang_load {
            let loaded_lang = ConfigPath::new("lang.txt")
                .ok()
                .and_then(|p| host.data_files().read(&p).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|raw| UiLang::parse(&raw));
            with_state(|st| {
                st.lang_loaded = true;
                if let Some(l) = loaded_lang {
                    st.lang = l;
                    if let Some(mapped) = translate_known_description(&st.description, l) {
                        st.description = mapped;
                    }
                    st.status = tr(
                        l,
                        "F8 — toggle menu. Set size, area, description & materials.",
                        "F8 — открыть/закрыть. Выберите размер, область, описание и материалы.",
                    )
                    .into();
                    st.pending_send_lang = Some(l);
                    st.dirty = true;
                }
            });
        }

        // Reset stale counters and ask the server to load persisted world Undo/Redo history on world entry.
        with_state(|st| {
            if !st.was_playing {
                st.was_playing = true;
                st.undo_count = 0;
                st.redo_count = 0;
                if st.pending_send_lang.is_none() {
                    st.pending_send_lang = Some(st.lang);
                }
                st.dirty = true;
            }
        });

        let save_lang = with_state(|st| {
            if st.pending_save_lang {
                st.pending_save_lang = false;
                Some(st.lang)
            } else {
                None
            }
        });
        if let Some(l) = save_lang
            && let Ok(p) = ConfigPath::new("lang.txt")
        {
            let _ = host.data_files().write(&p, l.code().as_bytes());
        }

        let send_lang = with_state(|st| st.pending_send_lang.take());
        if let Some(l) = send_lang
            && let Some(ch) = channels()
        {
            let _ = ch.to_server_lang.send(host, &SetLangRequest { lang: l });
        }

        let need_load = with_state(|st| !st.favorites_loaded);
        if need_load {
            let loaded = ConfigPath::new("favorites.txt")
                .ok()
                .and_then(|p| host.data_files().read(&p).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(|raw| parse_favorites(&raw))
                .unwrap_or_default();
            with_state(|st| {
                st.favorites_loaded = true;
                if !loaded.is_empty() {
                    st.favorites = loaded;
                    st.fav_idx = 0;
                    st.dirty = true;
                }
            });
        }

        let to_save = with_state(|st| {
            if st.pending_save_favorites {
                st.pending_save_favorites = false;
                Some(serialize_favorites(&st.favorites))
            } else {
                None
            }
        });
        if let Some(raw) = to_save
            && let Ok(p) = ConfigPath::new("favorites.txt")
        {
            let _ = host.data_files().write(&p, raw.as_bytes());
        }

        let need_world_blocks = with_state(|st| st.world_blocks.is_empty());
        if need_world_blocks
            && let Some(entries) = host.registry_entries("minecraft:block")
        {
            let mut blocks = Vec::with_capacity(entries.len());
            for ident in entries {
                if host.item_key(&ident).is_some() {
                    blocks.push(ident.to_string());
                }
            }
            blocks.sort();
            blocks.dedup();
            if !blocks.is_empty() {
                with_state(|st| {
                    if st.world_blocks.is_empty() {
                        st.world_blocks = blocks;
                        if st.picker_open && st.picker_all_blocks {
                            st.dirty = true;
                        }
                    }
                });
            }
        }

        let biome_query = if host.mod_screen() == Some(SCREEN_ID) {
            with_state(|st| {
                if !st.use_biome_auto {
                    return None;
                }
                let (dim, qx, qy, qz, sx, sy, sz) = if let Some(o) = st.origin {
                    let cx = o[0] + i32::from(st.width / 2);
                    let cz = o[2] + i32::from(st.depth / 2);
                    (st.dimension.clone(), cx >> 4, o[1] >> 4, cz >> 4, cx, o[1], cz)
                } else if let Some(cam) = host.camera() {
                    let cx = cam.position[0].floor() as i32;
                    let cy = cam.position[1].floor() as i32;
                    let cz = cam.position[2].floor() as i32;
                    (cam.dimension.clone(), cx >> 4, cy >> 4, cz >> 4, cx, cy, cz)
                } else {
                    return None;
                };
                let key = (dim.clone(), qx, qy, qz);
                if st.last_biome_query.as_ref() == Some(&key) {
                    None
                } else {
                    st.last_biome_query = Some(key);
                    Some(crate::model::BiomeQueryRequest {
                        dimension: dim,
                        x: sx,
                        y: sy,
                        z: sz,
                    })
                }
            })
        } else {
            None
        };
        if let Some(bq) = biome_query
            && let Some(ch) = channels()
        {
            let _ = ch.to_server_biome.send(host, &bq);
        }
    } else {
        with_state(|st| {
            if st.was_playing {
                st.was_playing = false;
                st.undo_count = 0;
                st.redo_count = 0;
                st.pending_send_lang = None;
                st.terrain_cache_key = None;
                st.cached_terrain_offsets = None;
                st.cached_water_depths = None;
                st.bp_cache_key = None;
                st.cached_bp = None;
                st.world_mask_key = None;
                st.world_blocks.clear();
                st.detected_biome.clear();
                st.last_biome_query = None;
                st.build_progress = None;
                st.dirty = true;
            }
        });
    }

    let pending = with_state(|st| {
        if st.world_select != WorldSelectMode::Inactive {
            st.aimed_block = current_aim_block(host);
        }
        st.pending.take()
    });

    let Some(action) = pending else {
        return;
    };

    match action {
        PendingAction::OpenMenu => {
            with_state(|st| {
                if st.origin.is_none() {
                    compute_origin_in_front(host, st);
                }
                st.dirty = true;
            });
            host.mod_screen_open(SCREEN_ID);
        }
        PendingAction::CloseMenu => {
            host.mod_screen_close(SCREEN_ID);
        }
        PendingAction::StartWorldSelect => {
            host.mod_screen_close(SCREEN_ID);
            with_state(|st| {
                st.world_select = WorldSelectMode::PickingCorner1;
                st.aimed_block = current_aim_block(host);
            });
        }
        PendingAction::StartMovePreview => {
            host.mod_screen_close(SCREEN_ID);
            with_state(|st| {
                if st.origin.is_none() {
                    compute_origin_in_front(host, st);
                }
                if !st.show_preview_box {
                    st.show_preview_box = true;
                }
                st.saved_overlay_vis = None;
                st.world_select = WorldSelectMode::MovePreview;
                st.aimed_block = current_aim_block(host);
            });
        }
        PendingAction::StartRoadSelect => {
            host.mod_screen_close(SCREEN_ID);
            with_state(|st| {
                st.world_select = WorldSelectMode::PickingRoadStart;
                st.road_preview_cache = None;
                st.aimed_block = current_aim_block(host);
            });
        }
        PendingAction::PlaceInFront => {
            with_state(|st| {
                compute_origin_in_front(host, st);
                if let Some((rf, rp)) = st.saved_overlay_vis.take() {
                    st.show_frame_box = rf || !rp;
                    st.show_preview_box = rp;
                }
                st.status = match st.lang {
                    UiLang::En => format!("Area in front of you: {:?}", st.origin.unwrap_or([0, 64, 0])),
                    UiLang::Ru => format!("Область перед вами: {:?}", st.origin.unwrap_or([0, 64, 0])),
                };
                st.dirty = true;
            });
        }
        PendingAction::CopyShareCode(preset) => {
            let code = encode_share_code(&preset);
            let copied = host.set_clipboard_text(&code);
            with_state(|st| {
                st.status = if copied {
                    match st.lang {
                        UiLang::En => format!("Share code copied ({} chars): {}...", code.len(), &code[..code.len().min(20)]),
                        UiLang::Ru => format!("Share-код скопирован ({} симв.): {}...", code.len(), &code[..code.len().min(20)]),
                    }
                } else {
                    match st.lang {
                        UiLang::En => format!("Code: {code}"),
                        UiLang::Ru => format!("Код: {code}"),
                    }
                };
            });
        }
        PendingAction::PasteShareCode => {
            let clip = host.clipboard_text().unwrap_or_default();
            let lang = with_state(|st| st.lang);
            match decode_share_code_lang(&clip, None, lang) {
                Ok(preset) => {
                    with_state(|st| {
                        apply_saved_preset(st, preset);
                        st.status = match st.lang {
                            UiLang::En => format!("Share code applied: {}x{}x{} (seed {})", st.width, st.depth, st.height, st.seed),
                            UiLang::Ru => format!("Share-код применён: {}x{}x{} (сид {})", st.width, st.depth, st.height, st.seed),
                        };
                        st.dirty = true;
                    });
                }
                Err(err) => {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Clipboard: {err}"),
                            UiLang::Ru => format!("Буфер обмена: {err}"),
                        };
                    });
                }
            }
        }
        PendingAction::CopyBomText(txt) => {
            let copied = host.set_clipboard_text(&txt);
            with_state(|st| {
                st.status = if copied {
                    match st.lang {
                        UiLang::En => format!("Material list copied ({} types, {} blocks).", st.preview_bom.len(), st.preview_total_blocks),
                        UiLang::Ru => format!("Смета скопирована ({} видов, {} бл.).", st.preview_bom.len(), st.preview_total_blocks),
                    }
                } else {
                    tr(st.lang, "Clipboard unavailable.", "Буфер обмена недоступен.").into()
                };
            });
        }
        PendingAction::SendBuild(req) => {
            if let Some(ch) = channels() {
                if let Err(e) = ch.to_server_build.send(host, &req) {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Network error: {e:?}"),
                            UiLang::Ru => format!("Ошибка сети: {e:?}"),
                        };
                    });
                }
            }
        }
        PendingAction::SendReroll(dim) => {
            if let Some(ch) = channels() {
                if let Err(e) = ch.to_server_reroll.send(host, &RerollRequest { dimension: dim }) {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Network error: {e:?}"),
                            UiLang::Ru => format!("Ошибка сети: {e:?}"),
                        };
                    });
                }
            }
        }
        PendingAction::SendUndo(dim) => {
            if let Some(ch) = channels() {
                if let Err(e) = ch.to_server_undo.send(host, &UndoRequest { dimension: dim }) {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Network error: {e:?}"),
                            UiLang::Ru => format!("Ошибка сети: {e:?}"),
                        };
                    });
                }
            }
        }
        PendingAction::SendRedo(dim) => {
            if let Some(ch) = channels() {
                if let Err(e) = ch.to_server_redo.send(host, &RedoRequest { dimension: dim }) {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Network error: {e:?}"),
                            UiLang::Ru => format!("Ошибка сети: {e:?}"),
                        };
                    });
                }
            }
        }
        PendingAction::SendRoad(req) => {
            if let Some(ch) = channels() {
                if let Err(e) = ch.to_server_road.send(host, &req) {
                    with_state(|st| {
                        st.status = match st.lang {
                            UiLang::En => format!("Network error: {e:?}"),
                            UiLang::Ru => format!("Ошибка сети: {e:?}"),
                        };
                    });
                }
            }
        }
    }
}

fn format_bom_stack(count: usize, lang: UiLang) -> String {
    if count < 64 {
        format!("x{count}")
    } else {
        let stacks = count / 64;
        let rem = count % 64;
        match (lang, rem) {
            (UiLang::En, 0) => format!("x{count} ({stacks}st)"),
            (UiLang::En, r) => format!("x{count} ({stacks}st+{r})"),
            (UiLang::Ru, 0) => format!("x{count} ({stacks}ст)"),
            (UiLang::Ru, r) => format!("x{count} ({stacks}ст+{r})"),
        }
    }
}

fn format_bom_clipboard(st: &ClientUiState) -> String {
    let mut out = match st.lang {
        UiLang::En => format!(
            "Building Generator — {} ({}) {}x{}x{} ({} blocks, {} types):\n",
            st.category.label(st.lang),
            st.style.label(st.lang),
            st.width,
            st.depth,
            st.height,
            st.preview_total_blocks,
            st.preview_bom.len()
        ),
        UiLang::Ru => format!(
            "Building Generator — {} ({}) {}x{}x{} ({} блоков, {} видов):\n",
            st.category.label(st.lang),
            st.style.label(st.lang),
            st.width,
            st.depth,
            st.height,
            st.preview_total_blocks,
            st.preview_bom.len()
        ),
    };
    for (id, cnt) in &st.preview_bom {
        out.push_str(&format!("- {id}: {}\n", format_bom_stack(*cnt, st.lang)));
    }
    out
}

fn draw_scrollbar(
    draw: &mut dyn mod_api::DrawContext,
    z: ScrollZone,
    mouse: [f32; 2],
) {
    let [tx, ty, tw, th] = z.track;
    draw.fill(z.track, 0xAA0F_172A);
    if z.total_rows > z.vis_rows && th > 4.0 {
        let max_s = z.total_rows - z.vis_rows;
        let thumb_h = ((z.vis_rows as f32 / z.total_rows as f32) * th).clamp(10.0f32.min(th), th);
        let frac = (z.scroll.min(max_s) as f32) / (max_s as f32);
        let thumb_y = ty + (th - thumb_h) * frac;
        let hovered = point_in_rect(mouse, z.track);
        let col = if hovered { 0xFF7D_D3FC } else { 0xFF38_BDF8 };
        draw.fill([tx + 1.0, thumb_y, (tw - 2.0).max(2.0), thumb_h], col);
    } else {
        draw.fill([tx + 1.0, ty, (tw - 2.0).max(2.0), th], 0x4447_5569);
    }
}

pub fn on_screen_render_post(host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
    if host.mod_screen() != Some(SCREEN_ID) {
        return;
    }
    let size = event.draw.size();

    let active = with_state(|st| {
        effective_preview_box(st.world_select, st.aimed_block, st.origin, st.width, st.depth, st.height)
    });
    if let Some((bmin, _bmax, eff_w, eff_d, _eff_h)) = active {
        sync_client_terrain_for_preview(host, bmin, eff_w, eff_d, true);
    }

    with_state(|st| {
        let (_, _, eff_w, eff_d, eff_h) = active.unwrap_or(([0, 0, 0], [0, 0, 0], st.width, st.depth, st.height));
        ensure_preview_cached(st, eff_w, eff_d, eff_h);
        if st.dirty
            || st
                .screen
                .as_ref()
                .is_none_or(|s| (s.size[0] - size[0]).abs() > 0.5 || (s.size[1] - size[1]).abs() > 0.5)
        {
            st.screen = Some(build_screen(st, size));
            st.hover_widget = None;
            st.dirty = false;
        }

        let Some(screen) = st.screen.as_mut() else {
            return;
        };
        let [px, py, pw, ph] = screen.panel;
        let pad = 10.0;
        let body_y = py + 48.0;

        // Backdrop & border
        event.draw.fill([px, py, pw, ph], 0xD814_1820);
        event.draw.fill([px, py, pw, 20.0], 0xEE1F_2937);
        event.draw.fill([px, py, pw, 1.0], 0xFF38_BDF8);
        event.draw.fill([px, py + ph - 1.0, pw, 1.0], 0xFF38_BDF8);
        event.draw.fill([px, py, 1.0, ph], 0xFF38_BDF8);
        event.draw.fill([px + pw - 1.0, py, 1.0, ph], 0xFF38_BDF8);

        let lang = st.lang;
        let title = if st.picker_open {
            match lang {
                UiLang::En => format!(
                    "Pick: {} ({})",
                    slot_label(st.selected_mat_slot, lang),
                    st.palette.slot_blocks(st.selected_mat_slot).len()
                ),
                UiLang::Ru => format!(
                    "Блоки: {} ({})",
                    slot_label(st.selected_mat_slot, lang),
                    st.palette.slot_blocks(st.selected_mat_slot).len()
                ),
            }
        } else if st.preset_picker_open {
            let total = filtered_preset_indices(st.preset_cat_filter, &st.preset_search).len();
            match lang {
                UiLang::En => format!("Catalog: {}/{}", total, CLI_PRESETS.len()),
                UiLang::Ru => format!("Каталог: {}/{}", total, CLI_PRESETS.len()),
            }
        } else if st.bom_open {
            match lang {
                UiLang::En => format!("Material Estimate ({} types)", st.preview_bom.len()),
                UiLang::Ru => format!("Смета блоков ({} видов)", st.preview_bom.len()),
            }
        } else {
            match lang {
                UiLang::En => "Building Generator".to_owned(),
                UiLang::Ru => "Генератор построек".to_owned(),
            }
        };
        event.draw.text(&title, [px + pad, py + 6.0], 0xFFF8_FAFC, true);
        let ver_badge = format!("v{}", crate::model::MOD_VERSION);
        let ver_w = event.draw.text_width(&ver_badge);
        event.draw.text(
            &ver_badge,
            [px + pw - pad - ver_w, py + 6.0],
            0xFF38_BDF8,
            true,
        );

        let _ = screen
            .widgets
            .draw(&mut *event.draw, host, event.screen.mouse);

        if st.picker_open {
            for (cand_id, in_list, rect) in &screen.picker_tiles {
                if cand_id.is_empty() {
                    continue;
                }
                if *in_list {
                    event.draw.fill([rect[0], rect[1], rect[2], 1.0], 0xFF22_C55E);
                    event.draw.fill([rect[0], rect[1] + rect[3] - 1.0, rect[2], 1.0], 0xFF22_C55E);
                    event.draw.fill([rect[0], rect[1], 1.0, rect[3]], 0xFF22_C55E);
                    event.draw.fill([rect[0] + rect[2] - 1.0, rect[1], 1.0, rect[3]], 0xFF22_C55E);
                }
                if let Ok(ident) = Identifier::parse(cand_id)
                    && let Some(item_key) = host.item_key(&ident)
                {
                    event.draw.item(item_key, [rect[0] + 1.5, rect[1] + 2.4], 12.0);
                }
            }
            if let Some(z) = screen.picker_scroll_zone {
                draw_scrollbar(&mut *event.draw, z, event.screen.mouse);
            }
            if let Some([sx, sy]) = screen.picker_strip_pos {
                let total_in_slot = st.palette.slot_blocks(st.selected_mat_slot).len();
                let sname = slot_label(st.selected_mat_slot, lang);
                let header = if total_in_slot > 5 {
                    match lang {
                        UiLang::En => format!("List \"{sname}\" ({total_in_slot}, showing 5 — click chip or ✕):"),
                        UiLang::Ru => format!("Список «{sname}» ({total_in_slot}, показано 5 — жмите чип или крестик):"),
                    }
                } else {
                    match lang {
                        UiLang::En => format!("List \"{sname}\" ({total_in_slot}/{}):", MaterialPalette::MAX_BLOCKS_PER_SLOT),
                        UiLang::Ru => format!("Список «{sname}» ({total_in_slot}/{}):", MaterialPalette::MAX_BLOCKS_PER_SLOT),
                    }
                };
                event.draw.text(&header, [sx, sy + 1.0], 0xFFCB_D5E1, false);
            }
            for (idx, bid, rect) in &screen.picker_chips {
                let active_replace = st.picker_replace_idx == Some(*idx);
                let col = if active_replace { 0xFFFB_BF24 } else { 0xFF38_BDF8 };
                event.draw.fill([rect[0], rect[1], rect[2], 1.0], col);
                event.draw.fill([rect[0], rect[1] + rect[3] - 1.0, rect[2], 1.0], col);
                if let Ok(ident) = Identifier::parse(bid)
                    && let Some(item_key) = host.item_key(&ident)
                {
                    event.draw.item(item_key, [rect[0] + 1.5, rect[1] + 2.5], 11.0);
                }
            }
        } else if st.preset_picker_open {
            if let Some(z) = screen.preset_scroll_zone {
                draw_scrollbar(&mut *event.draw, z, event.screen.mouse);
            }
            let active = st.archetype_idx % CLI_PRESETS.len();
            let active_slug = CLI_PRESETS[active].0;
            let foot_y = py + ph - 30.0;
            let sel_txt = match lang {
                UiLang::En => format!("Selected #{}/{}: {} [{}x{}x{}]", active + 1, CLI_PRESETS.len(), active_slug, st.width, st.depth, st.height),
                UiLang::Ru => format!("Выбран #{}/{}: {} [{}x{}x{}]", active + 1, CLI_PRESETS.len(), active_slug, st.width, st.depth, st.height),
            };
            event.draw.text(
                &sel_txt,
                [px + pad, foot_y - 23.0],
                0xFF94_A3B8,
                false,
            );
        } else if st.bom_open {
            let hdr_txt = match (lang, st.air_only) {
                (UiLang::En, false) => format!("Total: {} blocks ({} types)", st.preview_total_blocks, st.preview_bom.len()),
                (UiLang::En, true) => format!("Total: {} blocks ({} types | Air Only)", st.preview_total_blocks, st.preview_bom.len()),
                (UiLang::Ru, false) => format!("Всего: {} бл. ({} видов)", st.preview_total_blocks, st.preview_bom.len()),
                (UiLang::Ru, true) => format!("Всего: {} бл. ({} видов | Т.возд)", st.preview_total_blocks, st.preview_bom.len()),
            };
            event.draw.text(&hdr_txt, [px + pad + 68.0, py + 27.5], 0xFFE2_E8F0, false);

            for (bid, cnt, rect) in &screen.bom_rows {
                event.draw.fill(*rect, 0x551E_293B);
                if let Ok(ident) = Identifier::parse(bid)
                    && let Some(item_key) = host.item_key(&ident)
                {
                    event.draw.item(item_key, [rect[0] + 2.0, rect[1] + 2.2], 12.0);
                }
                let short: String = short_block_name(bid).chars().take(16).collect();
                event.draw.text(&short, [rect[0] + 17.0, rect[1] + 4.5], 0xFFE2_E8F0, false);
                let qty = format_bom_stack(*cnt, lang);
                let qw = event.draw.text_width(&qty);
                event.draw.text(&qty, [rect[0] + rect[2] - qw - 3.0, rect[1] + 4.5], 0xFF38_BDF8, false);
            }
            if let Some(z) = screen.bom_scroll_zone {
                draw_scrollbar(&mut *event.draw, z, event.screen.mouse);
            }
            let foot_y = py + ph - 30.0;
            let foot_note = tr(
                lang,
                "Door/bed items count 1 per 2 blocks. Scroll wheel or drag bar.",
                "Двери/кровати считаются 1 шт. за 2 блока. Колёсико/ползунок.",
            );
            event.draw.text(foot_note, [px + pad, foot_y - 23.0], 0xFF94_A3B8, false);
        } else {
            // Draw static labels & item icons over tabs.
            match st.tab {
                UiTab::SizeArea => {
                    let col_w = ((pw - pad * 2.0 - 10.0) * 0.5).floor();
                    let lx = px + pad;
                    let rx = px + pad + col_w + 10.0;

                    event.draw.text(tr(lang, "Width (X):", "Ширина (X):"), [lx, body_y + 5.0], 0xFFE2_E8F0, false);
                    event.draw.text(&format!("{}", st.width), [lx + 114.0, body_y + 5.0], 0xFF38_BDF8, true);

                    event.draw.text(tr(lang, "Depth (Z):", "Глубина (Z):"), [lx, body_y + 27.0], 0xFFE2_E8F0, false);
                    event.draw.text(&format!("{}", st.depth), [lx + 114.0, body_y + 27.0], 0xFF38_BDF8, true);

                    event.draw.text(tr(lang, "Height (Y):", "Высота (Y):"), [lx, body_y + 49.0], 0xFFE2_E8F0, false);
                    event.draw.text(&format!("{}", st.height), [lx + 114.0, body_y + 49.0], 0xFF38_BDF8, true);

                    let orig_text = match st.origin {
                        Some([ox, oy, oz]) => format!("{} {ox},{oy},{oz}", tr(lang, "Pos:", "Поз:")),
                        None => tr(lang, "Pos: not set", "Поз: не выбрана").into(),
                    };
                    event.draw.text(&orig_text, [rx, body_y + 5.0], 0xFFA5_F3FC, false);
                }
                UiTab::DescNoise => {
                    event.draw.text(
                        tr(
                            lang,
                            "Describe structure (style, floors, porch, balcony, hearth, tower, garden):",
                            "Опишите постройку (стиль, этажи, крыльцо, балкон, камин, башня, сад):",
                        ),
                        [px + pad, body_y + 22.0],
                        0xFFCB_D5E1,
                        false,
                    );
                    event.draw.text(tr(lang, "Seed:", "Сид:"), [px + pad, body_y + 100.0], 0xFFE2_E8F0, false);
                }
                UiTab::Materials => {
                    for &(slot, rect) in &screen.tab3_slot_rows {
                        if slot == st.selected_mat_slot {
                            event.draw.fill([rect[0], rect[1], rect[2], 1.0], 0xFF38_BDF8);
                            event.draw.fill([rect[0], rect[1] + rect[3] - 1.0, rect[2], 1.0], 0xFF38_BDF8);
                            event.draw.fill([rect[0], rect[1], 1.0, rect[3]], 0xFF38_BDF8);
                            event.draw.fill([rect[0] + rect[2] - 1.0, rect[1], 1.0, rect[3]], 0xFF38_BDF8);
                        }
                        let block_id = slot_block_id(&st.palette, slot);
                        if let Ok(ident) = Identifier::parse(block_id)
                            && let Some(item_key) = host.item_key(&ident)
                        {
                            event.draw.item(item_key, [rect[0] + 1.5, rect[1] + 1.5], 12.0);
                        }
                    }
                    if let Some(z) = screen.slot_scroll_zone {
                        draw_scrollbar(&mut *event.draw, z, event.screen.mouse);
                    }
                    if let Some([hx, hy]) = screen.tab3_header_pos {
                        let cnt = st.palette.slot_blocks(st.selected_mat_slot).len();
                        let htxt = format!(
                            "{} {} ({cnt})",
                            tr(lang, "List:", "Список:"),
                            slot_label(st.selected_mat_slot, lang)
                        );
                        event.draw.text(&htxt, [hx, hy], 0xFFE2_E8F0, false);
                    }
                    for (idx, bid, rect) in &screen.tab3_list_rows {
                        if *idx == 0 {
                            event.draw.fill([rect[0], rect[1], 1.5, rect[3]], 0xFF38_BDF8);
                        }
                        if let Ok(ident) = Identifier::parse(bid)
                            && let Some(item_key) = host.item_key(&ident)
                        {
                            event.draw.item(item_key, [rect[0] + 2.0, rect[1] + 1.5], 12.0);
                        }
                    }
                    if let Some(z) = screen.list_scroll_zone {
                        draw_scrollbar(&mut *event.draw, z, event.screen.mouse);
                    }
                }
                UiTab::WorldDecor => {
                    let lx = px + pad;
                    event.draw.text(
                        tr(
                            lang,
                            "Living Interiors 2.0 & Smooth Terrain Blending:",
                            "Живые интерьеры 2.0 и сглаживание ландшафта:",
                        ),
                        [lx, body_y + 4.0],
                        0xFFE2_E8F0,
                        false,
                    );
                    event.draw.text(
                        tr(
                            lang,
                            "2-Point Road & Auto-Bridge Connector:",
                            "Дорога между 2 точками и авто-мосты:",
                        ),
                        [lx, body_y + 70.0],
                        0xFFE2_E8F0,
                        false,
                    );
                    event.draw.text(
                        tr(
                            lang,
                            "Pick Start -> End in world or /buildgen connect [R: width, [/]: style]",
                            "Точки Старт -> Финиш в мире или /buildgen connect",
                        ),
                        [lx, body_y + 81.0],
                        0xFF94_A3B8,
                        false,
                    );
                    let rx = lx + (((pw - pad * 2.0) - 6.0) * 0.5).floor() + 6.0;
                    let pal_note = if st.palette_manual {
                        format!(
                            "{}: {}",
                            tr(lang, "Mat", "Мат"),
                            st.palette.display_name(lang)
                        )
                    } else if st.style == BuildStyle::Auto && st.use_biome_auto && !st.detected_biome.is_empty() {
                        format!("{}: {}", tr(lang, "Auto", "Авто"), short_block_name(&st.detected_biome))
                    } else {
                        match lang {
                            UiLang::En => "Tied to current Style".to_owned(),
                            UiLang::Ru => "По выбранному стилю".to_owned(),
                        }
                    };
                    event.draw.text(&pal_note, [rx + 4.0, body_y + 119.0], 0xFF94_A3B8, false);
                }
            }
        }

        // Summary + Status / Live Progress Bar above footer
        let foot_y = py + ph - 30.0;
        if !st.picker_open && !st.preset_picker_open && !st.bom_open {
            let air_tag = if st.air_only {
                tr(lang, " | AirOnly", " | Т.возд")
            } else {
                ""
            };
            let aging_tag = match (st.wear_pct, st.overgrowth_pct) {
                (0, 0) => String::new(),
                (w, 0) => format!(" | {}:{w}%", tr(lang, "W", "И")),
                (0, m) => format!(" | {}:{m}%", tr(lang, "M", "З")),
                (w, m) => format!(" | {}:{w}% {}:{m}%", tr(lang, "W", "И"), tr(lang, "M", "З")),
            };
            let summary = format!(
                "{} ({}) {}x{}x{} | {} {} ({}) | {} [{}]{air_tag}{aging_tag}",
                st.category.label(lang),
                st.style.label(lang),
                st.width,
                st.depth,
                st.height,
                tr(lang, "Seed:", "Сид:"),
                st.seed,
                if st.auto_seed {
                    tr(lang, "auto", "авто")
                } else {
                    tr(lang, "fix", "фикс")
                },
                st.palette.display_name(lang),
                if st.palette_manual {
                    tr(lang, "manual", "ручн.")
                } else {
                    tr(lang, "from text", "по тексту")
                }
            );
            event.draw.text(&summary, [px + pad, foot_y - 23.0], 0xFF94_A3B8, false);
        }
        if let Some((pct, _, _, _)) = &st.build_progress {
            let bar_w = ((pw - pad * 2.0) * 0.36).floor();
            let bar_x = px + pw - pad - bar_w;
            let bar_y = foot_y - 10.5;
            event.draw.fill([bar_x, bar_y, bar_w, 6.0], 0xAA0F_172A);
            let fill_w = (bar_w * (f32::from(*pct) / 100.0)).clamp(2.0, bar_w);
            event.draw.fill([bar_x, bar_y, fill_w, 6.0], 0xFF38_BDF8);
        }
        event.draw.text(&st.status, [px + pad, foot_y - 12.0], 0xFF34_D399, false);

        let hovered_tip = resolve_hovered_tooltip(&screen.tooltips, event.screen.mouse)
            .map(|t| (t.id, t.title.clone(), t.desc.clone()));
        let hovered_id = hovered_tip.as_ref().map(|(id, _, _)| *id);
        if hover_tooltip_ready(
            &mut st.hover_widget,
            &mut st.hover_since_ms,
            hovered_id,
            host.millis(),
        ) && let Some((_, title, desc)) = hovered_tip
        {
            draw_tooltip_popup(&mut *event.draw, event.screen.mouse, size, &title, &desc);
        }
    });
}

fn handle_scrollbar_track_at(host: &dyn ClientHost, at: [f32; 2]) -> bool {
    if host.mod_screen() != Some(SCREEN_ID) {
        return false;
    }
    with_state(|st| {
        let Some(screen) = st.screen.as_ref() else {
            return false;
        };
        let map_scroll = |z: ScrollZone| -> usize {
            if z.total_rows <= z.vis_rows || z.track[3] <= 0.0 {
                return 0;
            }
            let frac = ((at[1] - z.track[1]) / z.track[3]).clamp(0.0, 1.0);
            let max_s = z.total_rows - z.vis_rows;
            ((frac * max_s as f32).round() as usize).min(max_s)
        };
        if st.picker_open {
            if let Some(z) = screen.picker_scroll_zone
                && point_in_rect(at, z.track)
            {
                st.picker_scroll = map_scroll(z);
                st.dirty = true;
                return true;
            }
        } else if st.preset_picker_open {
            if let Some(z) = screen.preset_scroll_zone
                && point_in_rect(at, z.track)
            {
                st.preset_scroll = map_scroll(z);
                st.dirty = true;
                return true;
            }
        } else if st.bom_open {
            if let Some(z) = screen.bom_scroll_zone
                && point_in_rect(at, z.track)
            {
                st.bom_scroll = map_scroll(z);
                st.dirty = true;
                return true;
            }
        } else if st.tab == UiTab::Materials {
            if let Some(z) = screen.slot_scroll_zone
                && point_in_rect(at, z.track)
            {
                st.mat_slot_scroll = map_scroll(z);
                st.dirty = true;
                return true;
            }
            if let Some(z) = screen.list_scroll_zone
                && point_in_rect(at, z.track)
            {
                st.mat_list_scroll = map_scroll(z);
                st.dirty = true;
                return true;
            }
        }
        false
    })
}

pub fn on_screen_mouse_pressed(
    host: &dyn ClientHost,
    event: &mut ScreenMouseButtonPressedPreEvent<'_>,
) -> Flow {
    let (at, button, mods) = (event.screen.mouse, event.button, event.modifiers);
    if button == 0 && handle_scrollbar_track_at(host, at) {
        return Flow::Cancel;
    }
    handle_widget_input(host, |w| w.mouse_pressed(host, at, button, mods))
}

pub fn on_screen_mouse_released(
    host: &dyn ClientHost,
    event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
) -> Flow {
    let button = event.button;
    handle_widget_input(host, |w| w.mouse_released(button))
}

pub fn on_screen_mouse_dragged(
    host: &dyn ClientHost,
    event: &mut ScreenMouseDraggedPreEvent<'_>,
) -> Flow {
    let (at, button) = (event.screen.mouse, event.button);
    if button == 0 && handle_scrollbar_track_at(host, at) {
        return Flow::Cancel;
    }
    handle_widget_input(host, |w| w.mouse_dragged(host, at, button))
}

pub fn on_screen_mouse_scrolled(
    host: &dyn ClientHost,
    event: &mut ScreenMouseScrolledPreEvent<'_>,
) -> Flow {
    if host.mod_screen() != Some(SCREEN_ID) {
        return Flow::Continue;
    }
    let at = event.screen.mouse;
    let delta_y = event.scroll[1];
    if delta_y.abs() < 1e-4 {
        return Flow::Continue;
    }
    with_state(|st| {
        let Some(screen) = st.screen.as_ref() else {
            return Flow::Continue;
        };
        if !point_in_rect(at, screen.panel) {
            return Flow::Continue;
        }
        let step = if delta_y.abs() >= 2.0 { 2usize } else { 1usize };
        let up = delta_y > 0.0;
        if st.picker_open {
            if let Some(z) = screen.picker_scroll_zone
                && z.total_rows > z.vis_rows
            {
                let max_s = z.total_rows - z.vis_rows;
                let next = if up {
                    st.picker_scroll.saturating_sub(step)
                } else {
                    (st.picker_scroll + step).min(max_s)
                };
                if next != st.picker_scroll {
                    st.picker_scroll = next;
                    st.dirty = true;
                }
            }
            return Flow::Cancel;
        }
        if st.preset_picker_open {
            if let Some(z) = screen.preset_scroll_zone
                && z.total_rows > z.vis_rows
            {
                let max_s = z.total_rows - z.vis_rows;
                let next = if up {
                    st.preset_scroll.saturating_sub(step)
                } else {
                    (st.preset_scroll + step).min(max_s)
                };
                if next != st.preset_scroll {
                    st.preset_scroll = next;
                    st.dirty = true;
                }
            }
            return Flow::Cancel;
        }
        if st.bom_open {
            if let Some(z) = screen.bom_scroll_zone
                && z.total_rows > z.vis_rows
            {
                let max_s = z.total_rows - z.vis_rows;
                let next = if up {
                    st.bom_scroll.saturating_sub(step)
                } else {
                    (st.bom_scroll + step).min(max_s)
                };
                if next != st.bom_scroll {
                    st.bom_scroll = next;
                    st.dirty = true;
                }
            }
            return Flow::Cancel;
        }
        if st.tab == UiTab::Materials {
            if let Some(z) = screen.slot_scroll_zone
                && point_in_rect(at, z.viewport)
            {
                if z.total_rows > z.vis_rows {
                    let max_s = z.total_rows - z.vis_rows;
                    let next = if up {
                        st.mat_slot_scroll.saturating_sub(step)
                    } else {
                        (st.mat_slot_scroll + step).min(max_s)
                    };
                    if next != st.mat_slot_scroll {
                        st.mat_slot_scroll = next;
                        st.dirty = true;
                    }
                }
                return Flow::Cancel;
            }
            if let Some(z) = screen.list_scroll_zone
                && point_in_rect(at, z.viewport)
            {
                if z.total_rows > z.vis_rows {
                    let max_s = z.total_rows - z.vis_rows;
                    let next = if up {
                        st.mat_list_scroll.saturating_sub(step)
                    } else {
                        (st.mat_list_scroll + step).min(max_s)
                    };
                    if next != st.mat_list_scroll {
                        st.mat_list_scroll = next;
                        st.dirty = true;
                    }
                }
                return Flow::Cancel;
            }
        }
        Flow::Continue
    })
}

pub fn on_screen_key_pressed(
    host: &dyn ClientHost,
    event: &mut ScreenKeyPressedPreEvent<'_>,
) -> Flow {
    let (key, mods) = (event.key, event.modifiers);
    if key == "key.keyboard.escape" && host.mod_screen() == Some(SCREEN_ID) {
        let closed_picker = with_state(|st| {
            if st.picker_open {
                st.picker_open = false;
                st.picker_scroll = 0;
                st.mat_search.clear();
                st.dirty = true;
                true
            } else if st.preset_picker_open {
                st.preset_picker_open = false;
                st.preset_scroll = 0;
                st.preset_search.clear();
                st.dirty = true;
                true
            } else if st.bom_open {
                st.bom_open = false;
                st.bom_scroll = 0;
                st.dirty = true;
                true
            } else {
                false
            }
        });
        if closed_picker {
            return Flow::Cancel;
        }
    }
    handle_widget_input(host, |w| w.key_pressed(host, key, mods))
}

pub fn on_screen_char_typed(
    host: &dyn ClientHost,
    event: &mut ScreenCharTypedPreEvent<'_>,
) -> Flow {
    let c = event.c;
    handle_widget_input(host, |w| w.char_typed(host, c))
}

pub fn on_render_gui_post(host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
    let now = host.millis();
    let (lang, mode, style, facing, origin, aimed, road_width, show_frame, show_preview, progress, toast) = with_state(|st| {
        if let Some((_, exp)) = st.hud_toast
            && now >= exp
        {
            st.hud_toast = None;
        }
        (
            st.lang,
            st.world_select,
            st.style,
            st.facing,
            st.origin,
            st.aimed_block,
            st.road_width,
            st.show_frame_box,
            st.show_preview_box,
            st.build_progress.clone(),
            st.hud_toast.as_ref().map(|(msg, _)| msg.clone()),
        )
    });
    if !host.playing() {
        return;
    }
    let [sw, _] = event.draw.size();

    if let Some((pct, done, total, label)) = progress {
        let txt = match lang {
            UiLang::En => format!("Building {label}: {pct}% ({done}/{total})"),
            UiLang::Ru => format!("Стройка {label}: {pct}% ({done}/{total})"),
        };
        let bw = 200.0_f32.max(event.draw.text_width(&txt) + 16.0);
        let bx = ((sw - bw) * 0.5).floor();
        let by = if mode != WorldSelectMode::Inactive || toast.is_some() {
            32.0
        } else {
            10.0
        };
        event.draw.fill([bx, by, bw, 20.0], 0xC80F_172A);
        event.draw.fill([bx, by, bw, 1.0], 0xFF38_BDF8);
        event.draw.text(&txt, [bx + 8.0, by + 3.0], 0xFFE0_F2FE, true);
        let bar_w = bw - 16.0;
        let fill_w = (bar_w * (f32::from(pct) / 100.0)).clamp(2.0, bar_w);
        event.draw.fill([bx + 8.0, by + 13.5, bar_w, 4.0], 0xFF1E_293B);
        event.draw.fill([bx + 8.0, by + 13.5, fill_w, 4.0], 0xFF38_BDF8);
    }

    if mode == WorldSelectMode::Inactive {
        if let Some(tmsg) = toast {
            let tw = event.draw.text_width(&tmsg) + 16.0;
            let bx = ((sw - tw) * 0.5).floor();
            event.draw.fill([bx, 10.0, tw, 17.0], 0xC811_1827);
            event.draw.fill([bx, 10.0, tw, 1.0], 0xFF34_D399);
            event.draw.text(&tmsg, [bx + 8.0, 14.5], 0xFFD1_FAE5, true);
        } else if host.mod_screen().is_none() && origin.is_some() && (show_frame || show_preview) {
            let hint = match (lang, show_frame, show_preview) {
                (UiLang::En, true, true) => "[F8] Menu | [H] Hide Frame & 3D | [Shift+H] Hide Frame Only",
                (UiLang::En, false, true) => "[F8] Menu | [H] Hide 3D | [Shift+H] Show Frame",
                (UiLang::En, true, false) => "[F8] Menu | [H] Hide Frame",
                (UiLang::Ru, true, true) => "[F8] Меню | [H] Скрыть рамку и 3D | [Shift+H] Только рамку",
                (UiLang::Ru, false, true) => "[F8] Меню | [H] Скрыть 3D | [Shift+H] Показать рамку",
                (UiLang::Ru, true, false) => "[F8] Меню | [H] Скрыть рамку",
                _ => "",
            };
            if !hint.is_empty() {
                let hw = event.draw.text_width(hint) + 14.0;
                let hx = ((sw - hw) * 0.5).floor();
                event.draw.fill([hx, 8.0, hw, 14.0], 0x880F_172A);
                event.draw.text(hint, [hx + 7.0, 11.0], 0xFFCB_D5E1, false);
            }
        }
        return;
    }
    let msg = match mode {
        WorldSelectMode::Inactive => return,
        WorldSelectMode::PickingCorner1 => match lang {
            UiLang::En => format!(
                "Area: [RMB] center | [LMB] 1st corner | [R] front ({}) | [PgUp/Dn] height | [[/]] slice | [Esc]",
                facing.label(lang)
            ),
            UiLang::Ru => format!(
                "Область: [ПКМ] центр | [ЛКМ] 1-й угол | [R] вход ({}) | [PgUp/Dn] высота | [[/]] срез | [Esc]",
                facing.label(lang)
            ),
        },
        WorldSelectMode::PickingCorner2 { .. } => match lang {
            UiLang::En => format!(
                "2nd corner: [LMB/RMB] done | [R] front ({}) | [PgUp/Dn] height | [[/]] slice | [Esc]",
                facing.label(lang)
            ),
            UiLang::Ru => format!(
                "2-й угол: [ЛКМ/ПКМ] готово | [R] вход ({}) | [PgUp/Dn] высота | [[/]] срез | [Esc]",
                facing.label(lang)
            ),
        },
        WorldSelectMode::MovePreview => {
            let [ox, oy, oz] = origin.unwrap_or([0, 64, 0]);
            match lang {
                UiLang::En => format!(
                    "Move 3D ({ox},{oy},{oz}) [{}]: [Arrows/IJKL] Move | [PgUp/Dn] Y | [R] Rot | [C] Colls | [H] Frame | [Click] Snap | [Enter] Build | [Esc]",
                    facing.label(lang)
                ),
                UiLang::Ru => format!(
                    "3D ({ox},{oy},{oz}) [{}]: [Стрелки/IJKL] Сдвиг | [PgUp/Dn] Y | [R] Вращ | [C] Колл | [H] Рамка | [Клик] Точка | [Enter] Строить | [Esc]",
                    facing.label(lang)
                ),
            }
        }
        WorldSelectMode::PickingRoadStart => {
            let s_lbl = style.label(lang);
            match lang {
                UiLang::En => format!(
                    "Road [1/2] ({s_lbl}): [LMB/RMB] START | [R] width ({road_width}b) | [[/]] style | [Esc]"
                ),
                UiLang::Ru => format!(
                    "Дорога [1/2] ({s_lbl}): [ЛКМ/ПКМ] СТАРТ | [R] шир ({road_width}б) | [[/]] стиль | [Esc]"
                ),
            }
        }
        WorldSelectMode::PickingRoadEnd { start } => {
            let s_lbl = style.label(lang);
            let end = aimed.unwrap_or(start);
            let dist = (((end[0] - start[0]).pow(2) + (end[2] - start[2]).pow(2)) as f64)
                .sqrt()
                .round() as i32;
            match lang {
                UiLang::En => format!(
                    "Road [2/2] ({s_lbl}) ({},{},{}) ~{}m: [LMB/RMB] build | [R] {}b | [[/]] style | [Esc]",
                    start[0], start[1], start[2], dist, road_width
                ),
                UiLang::Ru => format!(
                    "Дорога [2/2] ({s_lbl}) ({},{},{}) ~{}м: [ЛКМ/ПКМ] строить | [R] {}б | [[/]] стиль | [Esc]",
                    start[0], start[1], start[2], dist, road_width
                ),
            }
        }
    };
    let tw = event.draw.text_width(&msg) + 16.0;
    let bx = ((sw - tw) * 0.5).floor();
    event.draw.fill([bx, 10.0, tw, 18.0], 0xC811_1827);
    event.draw.fill([bx, 10.0, tw, 1.0], 0xFF38_BDF8);
    event.draw.text(&msg, [bx + 8.0, 15.0], 0xFFE0_F2FE, true);
}

fn push_box_lines(lines: &mut Vec<WorldLine>, min: [f64; 3], max: [f64; 3], color: u32, width: f32) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let edges = [
        ([x0, y0, z0], [x1, y0, z0]),
        ([x1, y0, z0], [x1, y0, z1]),
        ([x1, y0, z1], [x0, y0, z1]),
        ([x0, y0, z1], [x0, y0, z0]),
        ([x0, y1, z0], [x1, y1, z0]),
        ([x1, y1, z0], [x1, y1, z1]),
        ([x1, y1, z1], [x0, y1, z1]),
        ([x0, y1, z1], [x0, y1, z0]),
        ([x0, y0, z0], [x0, y1, z0]),
        ([x1, y0, z0], [x1, y1, z0]),
        ([x1, y0, z1], [x1, y1, z1]),
        ([x0, y0, z1], [x0, y1, z1]),
    ];
    for (a, b) in edges {
        lines.push(WorldLine::new(a, b, color, width, WorldDepth::Tested));
    }
}

fn effective_preview_box(
    mode: WorldSelectMode,
    aimed: Option<[i32; 3]>,
    origin: Option<[i32; 3]>,
    w: u16,
    d: u16,
    h: u16,
) -> Option<([i32; 3], [i32; 3], u16, u16, u16)> {
    match mode {
        WorldSelectMode::PickingCorner2 { c1 } => {
            let p2 = aimed.unwrap_or(c1);
            let min_x = c1[0].min(p2[0]);
            let min_y = c1[1].min(p2[1]);
            let min_z = c1[2].min(p2[2]);
            let eff_w = ((c1[0].max(p2[0]) - min_x + 1) as u16).clamp(7, 96);
            let eff_d = ((c1[2].max(p2[2]) - min_z + 1) as u16).clamp(7, 96);
            let min = [min_x, min_y, min_z];
            let max = [
                min_x + i32::from(eff_w),
                min_y + i32::from(h),
                min_z + i32::from(eff_d),
            ];
            Some((min, max, eff_w, eff_d, h))
        }
        WorldSelectMode::PickingCorner1 => aimed.map(|p| {
            let min = [p[0] - i32::from(w / 2), p[1], p[2] - i32::from(d / 2)];
            let max = [min[0] + i32::from(w), min[1] + i32::from(h), min[2] + i32::from(d)];
            (min, max, w, d, h)
        }),
        WorldSelectMode::Inactive | WorldSelectMode::MovePreview => origin.map(|o| {
            (o, [o[0] + i32::from(w), o[1] + i32::from(h), o[2] + i32::from(d)], w, d, h)
        }),
        WorldSelectMode::PickingRoadStart | WorldSelectMode::PickingRoadEnd { .. } => None,
    }
}

fn terrain_fingerprint(offsets: Option<&[i32]>, waters: Option<&[i32]>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    if let Some(offs) = offsets {
        for &v in offs {
            for b in v.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x1000_0000_01b3);
            }
        }
    }
    if let Some(ws) = waters {
        h ^= 0x9e37_79b9_7f4a_7c15;
        for &v in ws {
            for b in v.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x1000_0000_01b3);
            }
        }
    }
    h
}

fn sync_client_terrain_for_preview(
    host: &dyn ClientHost,
    origin: [i32; 3],
    w: u16,
    d: u16,
    force: bool,
) {
    let now = host.millis();
    let sample_plan = with_state(|st| {
        let key = (st.adapt_terrain, origin[0], origin[1], origin[2], w, d);
        if st.terrain_cache_key.as_ref() == Some(&key) {
            return None;
        }
        if !st.adapt_terrain {
            st.terrain_cache_key = Some(key);
            st.cached_terrain_offsets = None;
            st.cached_water_depths = None;
            st.cached_terrain_fp = 0;
            return None;
        }
        if !force && st.terrain_cache_key.is_some() && now.saturating_sub(st.last_terrain_sample_ms) < 60 {
            return None;
        }
        Some(key)
    });

    let Some(key) = sample_plan else {
        return;
    };

    let iw = i32::from(w.clamp(7, 96));
    let id = i32::from(d.clamp(7, 96));
    let mut cols = Vec::with_capacity((iw * id) as usize);
    for lz in 0..id {
        for lx in 0..iw {
            cols.push((origin[0] + lx, origin[2] + lz));
        }
    }

    let surf = host.client_surface_heights(&cols, Heightmap::MotionBlockingNoLeaves).ok();
    let floor = surf
        .as_ref()
        .and_then(|_| host.client_surface_heights(&cols, Heightmap::OceanFloor).ok());

    let (offsets, waters) = if let Some(surf_h) = surf {
        let off: Vec<i32> = surf_h
            .iter()
            .map(|opt| opt.map_or(0, |sy| (sy - 1) - origin[1]))
            .collect();
        let wat = floor.map(|fl| {
            surf_h
                .iter()
                .zip(fl.iter())
                .map(|(s, f)| match (s, f) {
                    (Some(sy), Some(fy)) if sy > fy => (sy - fy).clamp(0, 16),
                    _ => 0,
                })
                .collect::<Vec<i32>>()
        });
        (Some(off), wat)
    } else {
        (None, None)
    };

    let fp = terrain_fingerprint(offsets.as_deref(), waters.as_deref());
    with_state(|st| {
        st.terrain_cache_key = Some(key);
        st.cached_terrain_offsets = offsets;
        st.cached_water_depths = waters;
        st.cached_terrain_fp = fp;
        st.last_terrain_sample_ms = now;
    });
}

fn ensure_preview_data(
    host: Option<&dyn ClientHost>,
    bmin: Option<[i32; 3]>,
    st: &mut ClientUiState,
    eff_w: u16,
    eff_d: u16,
    eff_h: u16,
    max_quads: usize,
    max_lines: usize,
) -> (Vec<crate::gen_preview::PreviewQuad>, Vec<crate::gen_preview::PreviewLine>, Option<i32>) {
    ensure_preview_cached_with_host(host, bmin, st, eff_w, eff_d, eff_h);
    let (qs, ls) = st.preview_mesh.fit_budget(max_quads, max_lines);
    (qs.to_vec(), ls.to_vec(), st.preview_slice_y)
}

fn ensure_preview_cached(st: &mut ClientUiState, eff_w: u16, eff_d: u16, eff_h: u16) {
    ensure_preview_cached_with_host(None, None, st, eff_w, eff_d, eff_h);
}

fn blueprint_block_hash(bp: &crate::generator::GeneratedBlueprint) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for (&[x, y, z], planned) in &bp.blocks {
        for v in [x, y, z] {
            for b in v.to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x1000_0000_01b3);
            }
        }
        for b in planned.block_id.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}

fn sample_world_collision_mask(
    host: &dyn ClientHost,
    st: &mut ClientUiState,
    orig: [i32; 3],
    eff_w: u16,
    eff_d: u16,
    eff_h: u16,
) {
    if !st.air_only && !st.show_collisions {
        return;
    }
    let Some(bp) = st.cached_bp.as_ref() else {
        return;
    };
    if !st.air_only && bp.blocks.len() > 45_000 {
        st.cached_world_mask = None;
        st.cached_world_mask_fp = 0;
        st.world_mask_key = None;
        return;
    }
    let wm_key = (
        st.dimension.clone(),
        orig[0],
        orig[1],
        orig[2],
        eff_w,
        eff_d,
        eff_h,
        st.cached_bp_fp,
        st.air_only,
        st.show_collisions,
    );
    if st.world_mask_key.as_ref() == Some(&wm_key) {
        return;
    }
    let now = host.millis();
    if st.world_mask_key.is_some()
        && matches!(st.world_select, WorldSelectMode::PickingCorner1 | WorldSelectMode::PickingCorner2 { .. })
        && now.saturating_sub(st.last_world_mask_ms) < 80
    {
        return;
    }
    let Some(level) = host.level(&st.dimension) else {
        return;
    };

    let collect_keys = |names: &[&str]| -> Vec<mod_api::BlockKey> {
        names.iter().filter_map(|n| host.block_key(n)).collect()
    };
    let air_keys = collect_keys(&["minecraft:air", "minecraft:cave_air", "minecraft:void_air"]);
    let soft_keys = collect_keys(&[
        "minecraft:water",
        "minecraft:lava",
        "minecraft:short_grass",
        "minecraft:tall_grass",
        "minecraft:fern",
        "minecraft:large_fern",
        "minecraft:dead_bush",
        "minecraft:vine",
        "minecraft:snow",
        "minecraft:seagrass",
        "minecraft:tall_seagrass",
        "minecraft:kelp",
        "minecraft:kelp_plant",
        "minecraft:pink_petals",
        "minecraft:wildflowers",
        "minecraft:leaf_litter",
        "minecraft:sugar_cane",
        "minecraft:dandelion",
        "minecraft:poppy",
        "minecraft:azure_bluet",
        "minecraft:oxeye_daisy",
        "minecraft:cornflower",
    ]);
    let natural_ground = collect_keys(&[
        "minecraft:grass_block",
        "minecraft:dirt",
        "minecraft:coarse_dirt",
        "minecraft:podzol",
        "minecraft:rooted_dirt",
        "minecraft:sand",
        "minecraft:red_sand",
        "minecraft:gravel",
        "minecraft:clay",
        "minecraft:stone",
        "minecraft:deepslate",
        "minecraft:netherrack",
        "minecraft:end_stone",
        "minecraft:mud",
        "minecraft:snow_block",
        "minecraft:mycelium",
        "minecraft:terracotta",
        "minecraft:red_sandstone",
        "minecraft:sandstone",
        "minecraft:basalt",
        "minecraft:blackstone",
        "minecraft:soul_sand",
        "minecraft:soul_soil",
        "minecraft:tuff",
        "minecraft:andesite",
        "minecraft:diorite",
        "minecraft:granite",
    ]);

    let min_y = bp.blocks.keys().map(|k| k[1]).min().unwrap_or(0).clamp(-24, 0);
    let max_y = bp
        .blocks
        .keys()
        .map(|k| k[1])
        .max()
        .unwrap_or(i32::from(bp.height).saturating_sub(1))
        .max(i32::from(bp.height).saturating_sub(1));
    let mask_h = (max_y - min_y + 1).clamp(1, 96);
    let mut mask = crate::gen_preview::PreviewWorldMask::new(
        i32::from(bp.width),
        min_y,
        mask_h,
        i32::from(bp.depth),
    );
    let mut wm_hash = 0xcbf2_9ce4_8422_2325u64 ^ u64::from(st.air_only) ^ (u64::from(st.show_collisions) << 1);

    for (&[lx, ly, lz], planned) in &bp.blocks {
        if matches!(
            planned.block_id.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        ) {
            continue;
        }
        let wpos = [orig[0] + lx, orig[1] + ly, orig[2] + lz];
        let Some(bk) = level.block(host, wpos) else {
            continue;
        };
        let is_air = air_keys.contains(&bk);
        let blocked = if st.air_only {
            !is_air
        } else {
            let is_soft = is_air || soft_keys.contains(&bk);
            let natural_sub = ly <= 0 && natural_ground.contains(&bk);
            !is_soft && !natural_sub
        };
        if blocked {
            mask.set_blocked(lx, ly, lz, true);
            for v in [lx, ly, lz] {
                for b in v.to_le_bytes() {
                    wm_hash ^= u64::from(b);
                    wm_hash = wm_hash.wrapping_mul(0x1000_0000_01b3);
                }
            }
        }
    }

    st.cached_world_mask = Some(mask);
    st.cached_world_mask_fp = wm_hash;
    st.world_mask_key = Some(wm_key);
    st.last_world_mask_ms = now;
}

fn ensure_preview_cached_with_host(
    host: Option<&dyn ClientHost>,
    bmin: Option<[i32; 3]>,
    st: &mut ClientUiState,
    eff_w: u16,
    eff_d: u16,
    eff_h: u16,
) {
    let eff_desc = effective_description(st);
    let biome_key = if st.use_biome_auto {
        st.detected_biome.clone()
    } else {
        String::new()
    };
    let toggles_raw = st.toggles.map(|t| t as u8);
    let bp_key = BpCacheKey {
        width: eff_w,
        depth: eff_d,
        height: eff_h,
        category: st.category,
        style: st.style,
        facing: st.facing,
        noise_level: st.noise_level,
        eff_desc: eff_desc.clone(),
        seed: st.seed,
        roof_raw: st.roof_choice as u8,
        toggles_raw,
        adapt_terrain: st.adapt_terrain,
        terrain_fp: if st.adapt_terrain { st.cached_terrain_fp } else { 0 },
        palette_fp: palette_fingerprint(&st.palette),
        wear_pct: st.wear_pct,
        overgrowth_pct: st.overgrowth_pct,
        living_decor: st.living_decor,
        biome_hint: biome_key.clone(),
    };
    if st.bp_cache_key.as_ref() != Some(&bp_key) || st.cached_bp.is_none() {
        let req = BuildRequest {
            dimension: st.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: eff_w,
            depth: eff_d,
            height: eff_h,
            category: st.category,
            style: st.style,
            facing: st.facing,
            noise_level: st.noise_level,
            seed: st.seed as i64,
            description: eff_desc,
            adapt_terrain: st.adapt_terrain,
            air_only: false,
            wear_pct: st.wear_pct,
            overgrowth_pct: st.overgrowth_pct,
            biome_hint: biome_key,
            palette: st.palette.clone(),
            living_decor: st.living_decor,
            spawn_npcs: st.spawn_npcs,
            blend_radius: 0,
        };
        let terrain_off = if st.adapt_terrain {
            st.cached_terrain_offsets.as_deref()
        } else {
            None
        };
        let water_dep = if st.adapt_terrain {
            st.cached_water_depths.as_deref()
        } else {
            None
        };
        let bp = crate::generator::generate_blueprint_with_water(&req, terrain_off, water_dep);
        let (bom_total, bom_list) = bp.material_breakdown();
        if st.preview_total_blocks != bom_total || st.preview_bom != bom_list {
            st.preview_total_blocks = bom_total;
            st.preview_bom = bom_list;
            st.dirty = true;
        }
        st.cached_bp_fp = blueprint_block_hash(&bp);
        st.cached_bp = Some(bp);
        st.bp_cache_key = Some(bp_key);
        st.world_mask_key = None;
        st.preview_key = None;
    }

    if let (Some(h), Some(orig)) = (host, bmin) {
        sample_world_collision_mask(h, st, orig, eff_w, eff_d, eff_h);
    }

    let wm_fp = if st.air_only || st.show_collisions {
        st.cached_world_mask_fp
    } else {
        0
    };
    let key = PreviewKey {
        bp_fp: st.cached_bp_fp,
        preview_slice_y: st.preview_slice_y,
        air_only: st.air_only,
        show_collisions: st.show_collisions,
        world_mask_fp: wm_fp,
    };
    if st.preview_key.as_ref() == Some(&key) {
        return;
    }

    if let Some(bp) = st.cached_bp.as_ref() {
        let clip_max_y = st.preview_slice_y.map(|sy| sy.clamp(0, i32::from(bp.height)));
        let wm_ref = if st.air_only || st.show_collisions {
            st.cached_world_mask.as_ref()
        } else {
            None
        };
        st.preview_mesh = crate::gen_preview::build_hologram_mesh_with_world(
            bp,
            clip_max_y,
            wm_ref,
            st.air_only,
            st.show_collisions,
        );
    }
    st.preview_key = Some(key);
}

fn road_preview_colors(style: BuildStyle, biome: &str) -> (u32, u32, u32) {
    let eff = if style == BuildStyle::Auto {
        crate::parser::biome_style_suggestion(biome).unwrap_or(BuildStyle::Auto)
    } else {
        style
    };
    match eff {
        BuildStyle::Desert => (0x44F5_9E0B, 0xE0FB_BF24, 0x99FE_F3C7),
        BuildStyle::Japanese => (0x44F4_72B6, 0xE0F4_3F5E, 0x99FD_A4AF),
        BuildStyle::Modern => (0x4494_A3B8, 0xE0E2_E8F0, 0xAAFF_FFFF),
        BuildStyle::Gothic => (0x448B_5CF6, 0xE0A8_55F7, 0x99C4_B5FD),
        BuildStyle::Classical => (0x44FB_BF24, 0xE0FE_F08A, 0xAAFF_FFFF),
        BuildStyle::Steampunk => (0x44EA_580C, 0xE014_B8A6, 0x99FD_BA74),
        BuildStyle::Cottage => (0x4422_C55E, 0xE084_CC16, 0x99BB_F7D0),
        BuildStyle::Nordic => (0x4460_A5FA, 0xE093_C5FD, 0x99BF_DBFE),
        BuildStyle::Auto | BuildStyle::Medieval => (0x4438_BDF8, 0xE038_BDF8, 0x99A5_F3FC),
    }
}

pub fn on_submit_custom_geometry(
    host: &dyn ClientHost,
    event: &mut SubmitCustomGeometryEvent<'_>,
) {
    let snapshot = with_state(|st| {
        let biome = if st.use_biome_auto {
            st.detected_biome.as_str()
        } else {
            ""
        };
        (
            st.world_select,
            st.aimed_block,
            st.origin,
            st.width,
            st.depth,
            st.height,
            st.facing,
            st.style,
            road_preview_colors(st.style, biome),
            st.road_width,
            st.road_auto_bridge,
            st.show_preview_box,
            st.show_frame_box,
        )
    });

    let (
        mode,
        aimed,
        origin,
        w,
        d,
        h,
        facing,
        _road_style,
        (road_quad_col, road_edge_col, road_center_col),
        road_width,
        road_auto_bridge,
        show_preview,
        show_frame,
    ) = snapshot;

    if matches!(
        mode,
        WorldSelectMode::PickingRoadStart | WorldSelectMode::PickingRoadEnd { .. }
    ) {
        let mut lines = Vec::new();
        let mut quads = Vec::new();
        match mode {
            WorldSelectMode::PickingRoadStart => {
                if let Some(p) = aimed {
                    let bmin = [p[0] as f64, p[1] as f64, p[2] as f64];
                    let bmax = [bmin[0] + 1.0, bmin[1] + 1.0, bmin[2] + 1.0];
                    push_box_lines(&mut lines, bmin, bmax, 0xFF38_BDF8, 2.5);
                    lines.push(WorldLine::new(
                        [bmin[0] + 0.5, bmax[1], bmin[2] + 0.5],
                        [bmin[0] + 0.5, bmax[1] + 4.0, bmin[2] + 0.5],
                        0xFF38_BDF8,
                        2.5,
                        WorldDepth::ThroughWalls,
                    ));
                    quads.push(WorldQuad::new(
                        [
                            [bmin[0], bmax[1] + 0.03, bmin[2]],
                            [bmin[0], bmax[1] + 0.03, bmax[2]],
                            [bmax[0], bmax[1] + 0.03, bmax[2]],
                            [bmax[0], bmax[1] + 0.03, bmin[2]],
                        ],
                        0x5538_BDF8,
                        WorldDepth::Tested,
                    ));
                }
            }
            WorldSelectMode::PickingRoadEnd { start } => {
                let smin = [start[0] as f64, start[1] as f64, start[2] as f64];
                push_box_lines(
                    &mut lines,
                    smin,
                    [smin[0] + 1.0, smin[1] + 1.0, smin[2] + 1.0],
                    0xFF34_D399,
                    2.6,
                );
                let end = aimed.unwrap_or(start);
                let emin = [end[0] as f64, end[1] as f64, end[2] as f64];
                push_box_lines(
                    &mut lines,
                    emin,
                    [emin[0] + 1.0, emin[1] + 1.0, emin[2] + 1.0],
                    0xFFFACC15,
                    2.4,
                );

                let sx = smin[0] + 0.5;
                let sy = smin[1] + 1.05;
                let sz = smin[2] + 0.5;
                let ex = emin[0] + 0.5;
                let ey = emin[1] + 1.05;
                let ez = emin[2] + 0.5;
                let dx = ex - sx;
                let dz = ez - sz;
                let len = (dx * dx + dz * dz).sqrt().max(0.01);
                let hw = f64::from(road_width.clamp(3, 5)) * 0.5;
                let nx = -dz / len * hw;
                let nz = dx / len * hw;
                let steps = ((len / 2.0).ceil() as usize).clamp(1, 32);

                let mut prev_l = [sx + nx, sy, sz + nz];
                let mut prev_r = [sx - nx, sy, sz - nz];
                let mut prev_c = [sx, sy + 0.08, sz];
                for step in 1..=steps {
                    let t = step as f64 / steps as f64;
                    let arch = if road_auto_bridge && len >= 10.0 {
                        4.0 * t * (1.0 - t) * 0.5
                    } else {
                        0.0
                    };
                    let px = sx + dx * t;
                    let py = sy + (ey - sy) * t + arch;
                    let pz = sz + dz * t;
                    let cur_l = [px + nx, py, pz + nz];
                    let cur_r = [px - nx, py, pz - nz];
                    let cur_c = [px, py + 0.08, pz];

                    quads.push(WorldQuad::new(
                        [prev_l, cur_l, cur_r, prev_r],
                        road_quad_col,
                        WorldDepth::Tested,
                    ));
                    lines.push(WorldLine::new(prev_l, cur_l, road_edge_col, 2.5, WorldDepth::Tested));
                    lines.push(WorldLine::new(prev_r, cur_r, road_edge_col, 2.5, WorldDepth::Tested));
                    lines.push(WorldLine::new(prev_c, cur_c, road_center_col, 1.8, WorldDepth::Tested));

                    prev_l = cur_l;
                    prev_r = cur_r;
                    prev_c = cur_c;
                }
            }
            _ => {}
        }
        if !lines.is_empty() {
            let _ = event.draw.lines(&lines);
        }
        if !quads.is_empty() {
            let _ = event.draw.quads(&quads);
        }
        return;
    }

    if mode == WorldSelectMode::Inactive && ((!show_preview && !show_frame) || origin.is_none()) {
        return;
    }

    let active_box = effective_preview_box(mode, aimed, origin, w, d, h);
    let Some((bmin, bmax, eff_w, eff_d, eff_h)) = active_box else {
        return;
    };

    // Refresh real client surface terrain heights on hover/relocation when Terrain adaptation is ON.
    sync_client_terrain_for_preview(host, bmin, eff_w, eff_d, false);

    let usage = event.draw.usage();
    let avail_quads = (usage.quad_capacity.saturating_sub(usage.quads) as usize).saturating_sub(8);
    let avail_lines = (usage.line_capacity.saturating_sub(usage.lines) as usize).saturating_sub(48);

    let mut lines = Vec::new();
    let mut quads = Vec::new();

    let fmin = [bmin[0] as f64, bmin[1] as f64, bmin[2] as f64];
    let fmax = [bmax[0] as f64, bmax[1] as f64, bmax[2] as f64];

    let draw_frame = show_frame || matches!(mode, WorldSelectMode::PickingCorner1 | WorldSelectMode::PickingCorner2 { .. });
    let draw_mesh = show_preview || mode != WorldSelectMode::Inactive;

    if draw_frame {
        // Gentle living pulse on outer hologram frame so users clearly see live preview bounds.
        let phase = ((host.millis() / 45) % 40) as i32;
        let pulse = if phase <= 20 { phase } else { 40 - phase }; // 0..=20
        let frame_col = if pulse > 10 { 0xE860_D5FF } else { 0xD038_BDF8 };
        push_box_lines(&mut lines, fmin, fmax, frame_col, 2.2);

        // Subtle translucent ground footprint highlight.
        quads.push(WorldQuad::new(
            [
                [fmin[0], fmin[1] + 0.02, fmin[2]],
                [fmin[0], fmin[1] + 0.02, fmax[2]],
                [fmax[0], fmin[1] + 0.02, fmax[2]],
                [fmax[0], fmin[1] + 0.02, fmin[2]],
            ],
            0x1C38_BDF8,
            WorldDepth::Tested,
        ));
    }

    if draw_mesh {
        let (preview_q, preview_l, slice_y) = with_state(|st| {
            ensure_preview_data(Some(host), Some(bmin), st, eff_w, eff_d, eff_h, avail_quads, avail_lines)
        });

        for pq in preview_q {
            quads.push(WorldQuad::new(
                [
                    [fmin[0] + pq.corners[0][0], fmin[1] + pq.corners[0][1], fmin[2] + pq.corners[0][2]],
                    [fmin[0] + pq.corners[1][0], fmin[1] + pq.corners[1][1], fmin[2] + pq.corners[1][2]],
                    [fmin[0] + pq.corners[2][0], fmin[1] + pq.corners[2][1], fmin[2] + pq.corners[2][2]],
                    [fmin[0] + pq.corners[3][0], fmin[1] + pq.corners[3][1], fmin[2] + pq.corners[3][2]],
                ],
                pq.color,
                WorldDepth::Tested,
            ));
        }

        for pl in preview_l {
            lines.push(WorldLine::new(
                [fmin[0] + pl.a[0], fmin[1] + pl.a[1], fmin[2] + pl.a[2]],
                [fmin[0] + pl.b[0], fmin[1] + pl.b[1], fmin[2] + pl.b[2]],
                pl.color,
                pl.width,
                WorldDepth::Tested,
            ));
        }

        if let Some(sy) = slice_y {
            let cut_y = fmin[1] + f64::from(sy.clamp(0, i32::from(eff_h)) + 1) + 0.03;
            let scol = crate::gen_preview::SLICE_CAP_ARGB | 0xFF00_0000;
            for (p0, p1) in [
                ([fmin[0], cut_y, fmin[2]], [fmax[0], cut_y, fmin[2]]),
                ([fmax[0], cut_y, fmin[2]], [fmax[0], cut_y, fmax[2]]),
                ([fmax[0], cut_y, fmax[2]], [fmin[0], cut_y, fmax[2]]),
                ([fmin[0], cut_y, fmax[2]], [fmin[0], cut_y, fmin[2]]),
            ] {
                lines.push(WorldLine::new(p0, p1, scol, 2.2, WorldDepth::Tested));
            }
        }
    }

    if draw_frame {
        // Golden directional arrow on front entrance side!
        let cx = (fmin[0] + fmax[0]) * 0.5;
        let cz = (fmin[2] + fmax[2]) * 0.5;
        let y = fmin[1] + 0.14;
        let (ax, az, dx, dz) = match facing {
            CardinalFacing::North => (cx, fmin[2], 0.0, -2.0),
            CardinalFacing::South => (cx, fmax[2], 0.0, 2.0),
            CardinalFacing::East => (fmax[0], cz, 2.0, 0.0),
            CardinalFacing::West => (fmin[0], cz, -2.0, 0.0),
        };
        lines.push(WorldLine::new(
            [ax, y, az],
            [ax + dx, y, az + dz],
            0xFFFBBF24,
            3.5,
            WorldDepth::ThroughWalls,
        ));
    }

    if mode != WorldSelectMode::Inactive
        && let Some(p) = aimed
    {
        push_box_lines(
            &mut lines,
            [p[0] as f64, p[1] as f64, p[2] as f64],
            [(p[0] + 1) as f64, (p[1] + 1) as f64, (p[2] + 1) as f64],
            0xFFFACC15,
            2.0,
        );
    }

    let final_line_cap = usage.line_capacity.saturating_sub(usage.lines) as usize;
    let final_quad_cap = usage.quad_capacity.saturating_sub(usage.quads) as usize;
    if lines.len() > final_line_cap {
        lines.truncate(final_line_cap);
    }
    if quads.len() > final_quad_cap {
        quads.truncate(final_quad_cap);
    }

    if !lines.is_empty() {
        let _ = event.draw.lines(&lines);
    }
    if !quads.is_empty() {
        let _ = event.draw.quads(&quads);
    }
}

#[cfg(test)]
pub(crate) fn test_audit_menu_tooltips() {
    let screen_sz = [480.0, 290.0];
    let has_tip = |scr: &BuiltScreen, id: Option<WidgetId>| -> bool {
        let Some(wid) = id else { return true };
        scr.tooltips.iter().any(|t| t.id == wid && !t.desc.is_empty())
    };
    let assert_has_tip = |scr: &BuiltScreen, id: WidgetId, name: &str| {
        assert!(
            scr.tooltips.iter().any(|t| t.id == id && !t.desc.is_empty()),
            "Missing tooltip on menu button '{name}'"
        );
    };

    for lang in [UiLang::En, UiLang::Ru] {
        // 1. All 4 main tabs
        for tab in [UiTab::SizeArea, UiTab::DescNoise, UiTab::Materials, UiTab::WorldDecor] {
            let mut st = ClientUiState::default();
            st.lang = lang;
            st.tab = tab;
            st.favorites.push(current_saved_preset(&st, "fav1".into()));
            let scr = build_screen(&st, screen_sz);
            let h = &scr.handles;

            assert_has_tip(&scr, h.bom_btn, "bom_btn");
            assert_has_tip(&scr, h.lang_btn, "lang_btn");
            assert_has_tip(&scr, h.tab_size.unwrap(), "tab_size");
            assert_has_tip(&scr, h.tab_desc.unwrap(), "tab_desc");
            assert_has_tip(&scr, h.tab_mats.unwrap(), "tab_mats");
            assert_has_tip(&scr, h.tab_world.unwrap(), "tab_world");
            assert_has_tip(&scr, h.generate_btn, "generate_btn");
            assert_has_tip(&scr, h.reroll_btn, "reroll_btn");
            assert_has_tip(&scr, h.undo_btn, "undo_btn");
            assert_has_tip(&scr, h.redo_btn, "redo_btn");
            assert_has_tip(&scr, h.close_btn, "close_btn");

            for opt in [
                h.w_dec, h.w_inc, h.d_dec, h.d_inc, h.h_dec, h.h_inc,
                h.preset_small, h.preset_med, h.preset_large, h.preset_tower,
                h.place_front, h.pick_world, h.rotate_btn, h.y_dec_btn, h.y_inc_btn,
                h.terrain_btn, h.preview_btn, h.air_only_btn,
                h.slice_toggle_btn, h.slice_dec_btn, h.slice_inc_btn,
                h.frame_btn, h.collision_btn, h.move_preview_btn, h.auto_hide_btn,
                h.fav_save_btn, h.fav_load_btn, h.fav_del_btn,
                h.share_copy_btn, h.share_paste_btn,
                h.cat_btn, h.style_btn, h.roof_btn,
                h.prev_seed_btn, h.new_seed_btn, h.auto_seed_btn,
                h.archetype_prev_btn, h.archetype_btn, h.archetype_next_btn,
                h.noise_btn, h.biome_btn,
                h.wear_dec_btn, h.wear_btn, h.wear_inc_btn,
                h.over_dec_btn, h.over_btn, h.over_inc_btn,
                h.mat_preset_btn, h.mat_infer_btn, h.mat_sync_wood_btn, h.mat_mix_mode_btn,
                h.mat_slot_up_btn, h.mat_slot_down_btn,
                h.list_clear_btn, h.list_up_btn, h.list_down_btn,
                h.picker_add_open_btn, h.list_pop_btn,
                h.living_decor_btn, h.spawn_npcs_btn, h.blend_radius_btn,
                h.road_width_btn, h.road_auto_bridge_btn, h.road_style_btn, h.road_connect_btn,
            ] {
                assert!(has_tip(&scr, opt), "Missing tooltip on active tab {:?} button", tab);
            }
            for &(tid, _) in &h.tag_buttons {
                assert_has_tip(&scr, tid, "tag_button");
            }
            for &(fid, _) in &h.feature_toggles {
                assert_has_tip(&scr, fid, "feature_toggle");
            }
            // Materials & blocks MUST NOT have hover tooltips!
            for &(mid, _) in &h.mat_slot_btns {
                assert!(scr.tooltips.iter().all(|t| t.id != mid), "Material slot button must NOT have tooltip");
            }
            for &(lid, _) in &h.list_item_btns {
                assert!(scr.tooltips.iter().all(|t| t.id != lid), "Slot block list button must NOT have tooltip");
            }
            for &(rid, _) in &h.list_remove_btns {
                assert!(scr.tooltips.iter().all(|t| t.id != rid), "Slot block remove button must NOT have tooltip");
            }
        }

        // 2. Block picker modal: functional/category buttons have tooltips; block tiles & chips do NOT!
        let mut st_pick = ClientUiState::default();
        st_pick.lang = lang;
        st_pick.picker_open = true;
        st_pick.selected_mat_slot = 1;
        let scr_pick = build_screen(&st_pick, screen_sz);
        let hp = &scr_pick.handles;
        assert_has_tip(&scr_pick, hp.picker_back_btn.unwrap(), "picker_back_btn");
        assert_has_tip(&scr_pick, hp.picker_mode_btn.unwrap(), "picker_mode_btn");
        assert_has_tip(&scr_pick, hp.picker_catalog_btn.unwrap(), "picker_catalog_btn");
        assert_has_tip(&scr_pick, hp.mat_mix_mode_btn.unwrap(), "mat_mix_mode_btn");
        assert_has_tip(&scr_pick, hp.mat_search_clear_btn.unwrap(), "mat_search_clear_btn");
        assert_has_tip(&scr_pick, hp.list_clear_btn.unwrap(), "list_clear_btn");
        assert_has_tip(&scr_pick, hp.picker_up_btn.unwrap(), "picker_up_btn");
        assert_has_tip(&scr_pick, hp.picker_down_btn.unwrap(), "picker_down_btn");
        for &(fbid, _) in &hp.mat_filter_btns {
            assert_has_tip(&scr_pick, fbid, "mat_filter_btn");
        }
        for (gbid, _) in &hp.picker_grid_btns {
            assert!(scr_pick.tooltips.iter().all(|t| t.id != *gbid), "Block tile button must NOT have tooltip");
        }
        for &(cbid, _) in &hp.picker_chip_btns {
            assert!(scr_pick.tooltips.iter().all(|t| t.id != cbid), "Slot block chip button must NOT have tooltip");
        }

        // 3. Preset catalog & BOM modals
        let mut st_cat = ClientUiState::default();
        st_cat.lang = lang;
        st_cat.preset_picker_open = true;
        let scr_cat = build_screen(&st_cat, screen_sz);
        let hc = &scr_cat.handles;
        assert_has_tip(&scr_cat, hc.preset_back_btn.unwrap(), "preset_back_btn");
        assert_has_tip(&scr_cat, hc.preset_cat_btn.unwrap(), "preset_cat_btn");
        assert_has_tip(&scr_cat, hc.preset_search_clear_btn.unwrap(), "preset_search_clear_btn");
        assert_has_tip(&scr_cat, hc.preset_up_btn.unwrap(), "preset_up_btn");
        assert_has_tip(&scr_cat, hc.preset_down_btn.unwrap(), "preset_down_btn");
        for &(pbid, pidx) in &hc.preset_grid_btns {
            if pidx < CLI_PRESETS.len() {
                assert_has_tip(&scr_cat, pbid, "preset_card");
            }
        }

        let mut st_bom = ClientUiState::default();
        st_bom.lang = lang;
        st_bom.bom_open = true;
        let scr_bom = build_screen(&st_bom, screen_sz);
        let hb = &scr_bom.handles;
        assert_has_tip(&scr_bom, hb.bom_back_btn.unwrap(), "bom_back_btn");
        assert_has_tip(&scr_bom, hb.bom_copy_btn.unwrap(), "bom_copy_btn");
        assert_has_tip(&scr_bom, hb.bom_up_btn.unwrap(), "bom_up_btn");
        assert_has_tip(&scr_bom, hb.bom_down_btn.unwrap(), "bom_down_btn");
    }
}
