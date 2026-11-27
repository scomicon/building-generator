use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static CMD_BUILD_SEQ: AtomicU64 = AtomicU64::new(1);

use mod_api::{
    BlockEdit, BlockKey, BlockRun, BlockState, BossBarColor, BossBarRef, Command,
    CommandArgumentValue, CommandSource, ConfigPath, Direction, Flow, Heightmap, HostBlockStates,
    IPayloadContext, PlayerRef, RegisterCommandsEvent, RunModCommandEvent, ScheduledTaskEvent,
    ServerHost, StateConstraint, TabCompleteEvent, Tag, Text,
};

use crate::gen_common::{
    BannerStyle, LecternBookKind, LivingEntityKind, LivingPlan, SignPurpose,
};
use crate::generator::{GeneratedBlueprint, PlannedBlock};
use crate::model::{
    BuildCategory, BuildRequest, BuildResponse, CardinalFacing, MaterialPalette, NoiseLevel,
    RedoRequest, RerollRequest, RoadConnectRequest, SetLangRequest, UiLang, UndoRequest,
};
use crate::network::channels;
use crate::noise::SplitMix64;
use crate::parser::StructureTheme;

pub const BATCH_TASK_TOKEN: u64 = 0x4247_5F42_4154_4348; // "BG_BATCH"
const MAX_RUNS_PER_BATCH: usize = 3000;
const MAX_UNDO_HISTORY_PER_PLAYER: usize = 5;
const MIN_NETWORK_BUILD_INTERVAL: Duration = Duration::from_millis(350);

#[derive(Clone)]
struct UndoEntry {
    dimension: String,
    previous_blocks: Vec<([i32; 3], BlockState)>,
    spawned_entities: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContainerSlotItem {
    pub slot: i8,
    pub item_id: &'static str,
    pub count: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PostBuildContainer {
    pub pos: [i32; 3],
    pub custom_name: String,
    pub items: Vec<ContainerSlotItem>,
}

struct PendingBatchJob {
    dimension: String,
    palette: Vec<BlockState>,
    chunks: VecDeque<Vec<BlockRun>>,
    completed_runs: usize,
    total_runs: usize,
    player_uuid: Option<u128>,
    boss_bar: Option<BossBarRef>,
    label: String,
    lang: UiLang,
    post_containers: Vec<PostBuildContainer>,
    post_living: LivingPlan,
    post_origin: [i32; 3],
}

static UNDO_STACK: Mutex<Option<HashMap<u128, Vec<UndoEntry>>>> = Mutex::new(None);
static REDO_STACK: Mutex<Option<HashMap<u128, Vec<UndoEntry>>>> = Mutex::new(None);
static HISTORY_WORLD_DIR: Mutex<Option<Option<PathBuf>>> = Mutex::new(None);
static LOADED_HISTORY_OWNERS: Mutex<Option<HashSet<u128>>> = Mutex::new(None);
static LAST_REQUESTS: Mutex<Option<HashMap<u128, BuildRequest>>> = Mutex::new(None);
static PENDING_JOBS: Mutex<Option<VecDeque<PendingBatchJob>>> = Mutex::new(None);
static PLAYER_LANG: Mutex<Option<HashMap<u128, UiLang>>> = Mutex::new(None);
static LAST_BUILD_AT: Mutex<Option<HashMap<u128, Instant>>> = Mutex::new(None);
static ROAD_CONNECT_ANCHORS: Mutex<Option<HashMap<u128, (String, [i32; 3])>>> = Mutex::new(None);

#[cfg(test)]
pub(crate) fn clear_history_memory_cache() {
    with_undo_map(|m| m.clear());
    with_redo_map(|m| m.clear());
    with_last_requests(|m| m.clear());
    *HISTORY_WORLD_DIR.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if let Ok(mut g) = LOADED_HISTORY_OWNERS.lock() {
        g.get_or_insert_with(HashSet::new).clear();
    }
    if let Ok(mut r) = ROAD_CONNECT_ANCHORS.lock()
        && let Some(m) = r.as_mut()
    {
        m.clear();
    }
}

fn with_undo_map<R>(f: impl FnOnce(&mut HashMap<u128, Vec<UndoEntry>>) -> R) -> R {
    let mut guard = UNDO_STACK.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

fn with_redo_map<R>(f: impl FnOnce(&mut HashMap<u128, Vec<UndoEntry>>) -> R) -> R {
    let mut guard = REDO_STACK.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

fn with_last_requests<R>(f: impl FnOnce(&mut HashMap<u128, BuildRequest>) -> R) -> R {
    let mut guard = LAST_REQUESTS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    f(map)
}

fn with_pending_jobs<R>(f: impl FnOnce(&mut VecDeque<PendingBatchJob>) -> R) -> R {
    let mut guard = PENDING_JOBS.lock().unwrap_or_else(|e| e.into_inner());
    let q = guard.get_or_insert_with(VecDeque::new);
    f(q)
}

#[inline]
fn tr<'a>(lang: UiLang, en: &'a str, ru: &'a str) -> &'a str {
    match lang {
        UiLang::En => en,
        UiLang::Ru => ru,
    }
}

pub fn player_has_build_permission(host: &dyn ServerHost, player: PlayerRef) -> bool {
    player.permission_level(host).is_some_and(|lvl| lvl >= 2)
}

fn deny_permission_response(lang: UiLang) -> BuildResponse {
    BuildResponse {
        ok: false,
        blocks_changed: 0,
        message: tr(
            lang,
            "Permission denied: operator level 2+ (OP) required.",
            "Недостаточно прав: требуется уровень оператора (OP 2+).",
        )
        .to_owned(),
    }
}

fn check_network_action_allowed(
    host: &dyn ServerHost,
    player: PlayerRef,
    lang: UiLang,
    check_build_rate: bool,
) -> Result<(), BuildResponse> {
    if !player_has_build_permission(host, player) {
        return Err(deny_permission_response(lang));
    }
    if check_build_rate {
        let uid = player.uuid();
        let active_job = with_pending_jobs(|q| q.iter().any(|j| j.player_uuid == Some(uid)));
        if active_job {
            return Err(BuildResponse {
                ok: false,
                blocks_changed: 0,
                message: tr(
                    lang,
                    "Previous large build is still finishing. Please wait...",
                    "Предыдущая крупная постройка ещё строится, подождите...",
                )
                .to_owned(),
            });
        }
        let mut guard = LAST_BUILD_AT.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        let now = Instant::now();
        if let Some(&prev) = map.get(&uid)
            && now.saturating_duration_since(prev) < MIN_NETWORK_BUILD_INTERVAL
        {
            return Err(BuildResponse {
                ok: false,
                blocks_changed: 0,
                message: tr(
                    lang,
                    "Too many requests: wait a moment before next build.",
                    "Слишком часто: подождите немного перед следующей постройкой.",
                )
                .to_owned(),
            });
        }
        map.insert(uid, now);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn clear_rate_limit_for_test() {
    if let Ok(mut guard) = LAST_BUILD_AT.lock()
        && let Some(map) = guard.as_mut()
    {
        map.clear();
    }
}

pub(crate) fn sanitize_build_request(mut req: BuildRequest) -> BuildRequest {
    req.width = req.width.clamp(7, 96);
    req.depth = req.depth.clamp(7, 96);
    req.height = req.height.clamp(5, 48);
    req.origin_x = req.origin_x.clamp(-29_999_900, 29_999_900);
    req.origin_y = req.origin_y.clamp(-2048, 2048);
    req.origin_z = req.origin_z.clamp(-29_999_900, 29_999_900);
    if req.dimension.chars().count() > 128 || req.dimension.trim().is_empty() {
        req.dimension = "minecraft:overworld".into();
    }
    req.description = req.description.chars().take(512).collect();
    req.biome_hint = req.biome_hint.trim().chars().take(96).collect();
    req.wear_pct = req.wear_pct.min(100);
    req.overgrowth_pct = req.overgrowth_pct.min(100);
    req.blend_radius = req.blend_radius.min(10);
    req.palette.name = req.palette.name.chars().take(64).collect();
    let max_extra = MaterialPalette::MAX_BLOCKS_PER_SLOT.saturating_sub(1);
    req.palette
        .extra_lists
        .retain(|e| usize::from(e.slot) < MaterialPalette::SLOT_COUNT);
    req.palette.extra_lists.truncate(MaterialPalette::SLOT_COUNT);
    for entry in &mut req.palette.extra_lists {
        entry
            .blocks
            .retain(|b| !b.is_empty() && b.len() <= 128);
        entry.blocks.truncate(max_extra);
    }
    req.palette.extra_lists.retain(|e| !e.blocks.is_empty());
    req
}

pub fn player_lang(player: Option<PlayerRef>) -> UiLang {
    let key = player.map_or(0, PlayerRef::uuid);
    let mut guard = PLAYER_LANG.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.get(&key).copied().unwrap_or(UiLang::En)
}

pub fn set_player_lang(player: Option<PlayerRef>, lang: UiLang) {
    let key = player.map_or(0, PlayerRef::uuid);
    let mut guard = PLAYER_LANG.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(key, lang);
}

/// Returns `true` for delicate/attached/multi-part blocks that should be placed after structural solids.
pub fn is_dependent_block(planned: &PlannedBlock) -> bool {
    let id = planned.block_id.as_str();
    id.ends_with("_door")
        || id.ends_with("_bed")
        || id.ends_with("_carpet")
        || id.ends_with("_fence_gate")
        || id.ends_with("_pressure_plate")
        || id.ends_with("_hanging_sign")
        || id.ends_with("_wall_sign")
        || id.ends_with("_sign")
        || id.ends_with("_wall_banner")
        || id.ends_with("_banner")
        || id.ends_with("_shelf")
        || id.starts_with("minecraft:potted_")
        || matches!(
            id,
            "minecraft:flower_pot"
                | "minecraft:cake"
                | "minecraft:sea_pickle"
                | "minecraft:bell"
                | "minecraft:ladder"
                | "minecraft:lever"
                | "minecraft:tripwire_hook"
                | "minecraft:daylight_detector"
                | "minecraft:torch"
                | "minecraft:wall_torch"
                | "minecraft:soul_wall_torch"
                | "minecraft:candle"
                | "minecraft:wheat"
                | "minecraft:bamboo"
                | "minecraft:azalea"
                | "minecraft:flowering_azalea"
                | "minecraft:fern"
                | "minecraft:spore_blossom"
                | "minecraft:vine"
                | "minecraft:glow_lichen"
                | "minecraft:hanging_roots"
                | "minecraft:lily_pad"
                | "minecraft:sugar_cane"
                | "minecraft:pink_petals"
                | "minecraft:wildflowers"
                | "minecraft:leaf_litter"
                | "minecraft:short_grass"
                | "minecraft:brewing_stand"
                | "minecraft:lantern"
                | "minecraft:soul_lantern"
                | "minecraft:copper_lantern"
        )
}

/// Converts a `PlannedBlock` into an engine `BlockState`, falling back cleanly to default state if constraints fail.
fn resolve_planned_state(
    host: &dyn ServerHost,
    cache: &mut HashMap<String, Option<BlockKey>>,
    planned: &PlannedBlock,
) -> Result<BlockState, String> {
    let key = if let Some(cached) = cache.get(&planned.block_id) {
        *cached
    } else {
        let resolved = host.block_key(&planned.block_id);
        cache.insert(planned.block_id.clone(), resolved);
        resolved
    };

    let Some(block_key) = key else {
        return Err(format!("Unknown registry block: {}", planned.block_id));
    };

    let states = host.block_states();
    if planned.props.is_empty() {
        BlockState::of(&states, block_key, &[])
            .map_err(|e| format!("State error for {}: {e:?}", planned.block_id))
    } else {
        let constraints: Vec<StateConstraint> = planned
            .props
            .iter()
            .map(|(k, v)| StateConstraint::new(*k, *v))
            .collect();
        match BlockState::of(&states, block_key, &constraints) {
            Ok(state) => Ok(state),
            Err(_) => {
                // Preserve critical structural properties like `half` ("upper"/"lower") & `facing`
                // if an optional property constraint fails, before falling back to bare default state.
                let essential: Vec<StateConstraint> = planned
                    .props
                    .iter()
                    .filter(|&&(k, _)| matches!(k, "half" | "facing" | "type"))
                    .map(|(k, v)| StateConstraint::new(*k, *v))
                    .collect();
                if !essential.is_empty()
                    && let Ok(state) = BlockState::of(&states, block_key, &essential)
                {
                    return Ok(state);
                }
                BlockState::of(&states, block_key, &[])
                    .map_err(|e| format!("Fallback error for {}: {e:?}", planned.block_id))
            }
        }
    }
}

fn collect_runs_for_columns(columns: HashMap<(i32, i32), Vec<(i32, u32)>>) -> Vec<BlockRun> {
    let mut runs = Vec::new();
    let mut col_keys: Vec<(i32, i32)> = columns.keys().copied().collect();
    col_keys.sort_unstable();

    for (wx, wz) in col_keys {
        let mut col = columns.get(&(wx, wz)).cloned().unwrap_or_default();
        col.sort_unstable_by_key(|&(y, _)| y);

        let mut i = 0;
        while i < col.len() {
            let (start_y, block_idx) = col[i];
            let mut h = 1u32;
            while i + (h as usize) < col.len()
                && col[i + (h as usize)].0 == start_y + (h as i32)
                && col[i + (h as usize)].1 == block_idx
            {
                h += 1;
            }
            runs.push(BlockRun {
                x: wx,
                z: wz,
                y: start_y,
                height: h,
                surface: None,
                block: block_idx,
                only: None,
            });
            i += h as usize;
        }
    }
    runs
}

/// Converts generated voxel map into a deduplicated palette + two-phase vertically coalesced `BlockRun`s
/// (`phase1`: terrain/air/structural solids, `phase2`: doors/beds/carpets/pots/lanterns/ladders).
pub fn blueprint_to_phased_runs(
    host: &dyn ServerHost,
    bp: &GeneratedBlueprint,
) -> Result<(Vec<BlockState>, Vec<BlockRun>, Vec<BlockRun>), String> {
    let mut key_cache: HashMap<String, Option<BlockKey>> = HashMap::new();
    let mut palette: Vec<BlockState> = Vec::new();
    let mut state_to_idx: HashMap<BlockState, u32> = HashMap::new();

    let mut phase1_cols: HashMap<(i32, i32), Vec<(i32, u32)>> = HashMap::new();
    let mut phase2_cols: HashMap<(i32, i32), Vec<(i32, u32)>> = HashMap::new();

    for (&[lx, ly, lz], planned) in &bp.blocks {
        let state = match resolve_planned_state(host, &mut key_cache, planned) {
            Ok(s) => s,
            Err(_) => {
                // Fallback to stone/air if user typed an unknown block id.
                let fallback = if planned.block_id == "minecraft:air" {
                    PlannedBlock::simple("minecraft:air")
                } else {
                    PlannedBlock::simple("minecraft:cobblestone")
                };
                resolve_planned_state(host, &mut key_cache, &fallback)?
            }
        };
        let idx = if let Some(&existing) = state_to_idx.get(&state) {
            existing
        } else {
            let idx = palette.len() as u32;
            palette.push(state);
            state_to_idx.insert(state, idx);
            idx
        };

        let wx = bp.origin[0] + lx;
        let wy = bp.origin[1] + ly;
        let wz = bp.origin[2] + lz;
        let target = if is_dependent_block(planned) {
            &mut phase2_cols
        } else {
            &mut phase1_cols
        };
        target.entry((wx, wz)).or_default().push((wy, idx));
    }

    Ok((
        palette,
        collect_runs_for_columns(phase1_cols),
        collect_runs_for_columns(phase2_cols),
    ))
}

/// Converts generated voxel map into a deduplicated palette + vertically coalesced `BlockRun`s (Phase 1 followed by Phase 2).
pub fn blueprint_to_runs(
    host: &dyn ServerHost,
    bp: &GeneratedBlueprint,
) -> Result<(Vec<BlockState>, Vec<BlockRun>), String> {
    let (palette, mut phase1, phase2) = blueprint_to_phased_runs(host, bp)?;
    phase1.extend(phase2);
    Ok((palette, phase1))
}

fn collect_air_keys(host: &dyn ServerHost) -> Vec<BlockKey> {
    let mut keys = Vec::with_capacity(3);
    for id in ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"] {
        if let Some(k) = host.block_key(id) {
            keys.push(k);
        }
    }
    keys
}

fn retain_complete_doors(blocks: &mut std::collections::BTreeMap<[i32; 3], PlannedBlock>) {
    let drop_coords: Vec<[i32; 3]> = blocks
        .iter()
        .filter_map(|(&[x, y, z], blk)| {
            if !blk.block_id.ends_with("_door") {
                return None;
            }
            let is_upper = blk.props.iter().any(|&(k, v)| k == "half" && v == "upper");
            let partner = if is_upper { [x, y - 1, z] } else { [x, y + 1, z] };
            if blocks
                .get(&partner)
                .is_some_and(|p| p.block_id.ends_with("_door"))
            {
                None
            } else {
                Some([x, y, z])
            }
        })
        .collect();
    for pos in drop_coords {
        blocks.remove(&pos);
    }
}

/// Curates deterministic thematic supplies and names for placed chests, barrels, furnaces, smokers, and brewing stands.
pub(crate) fn collect_post_build_containers(
    bp: &GeneratedBlueprint,
    seed: u64,
    lang: UiLang,
) -> Vec<PostBuildContainer> {
    const MAX_CONTAINERS: usize = 256;
    let mut out = Vec::new();
    let upper_floor_y = i32::from(bp.spec.foundation_height) + i32::from(bp.spec.floor_height);

    for (&[lx, ly, lz], blk) in &bp.blocks {
        if out.len() >= MAX_CONTAINERS {
            break;
        }
        let bid = blk.block_id.as_str();
        if !matches!(
            bid,
            "minecraft:chest"
                | "minecraft:barrel"
                | "minecraft:smoker"
                | "minecraft:furnace"
                | "minecraft:blast_furnace"
                | "minecraft:brewing_stand"
        ) {
            continue;
        }
        let pos = [bp.origin[0] + lx, bp.origin[1] + ly, bp.origin[2] + lz];
        let pos_mix = ((lx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            ^ ((ly as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
            ^ ((lz as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
        let mut rng = SplitMix64::new(seed ^ pos_mix ^ 0x517C_C1B7_2722_0A95);

        let (custom_name, items) = match bid {
            "minecraft:smoker" => {
                let foods = [
                    "minecraft:beef",
                    "minecraft:porkchop",
                    "minecraft:chicken",
                    "minecraft:potato",
                    "minecraft:cod",
                ];
                let f = foods[rng.range_i32(0, (foods.len() - 1) as i32) as usize];
                let fc = rng.range_i32(4, 10) as i8;
                let cc = rng.range_i32(2, 6) as i8;
                (
                    tr(lang, "Kitchen Smoker", "Коптильня").to_owned(),
                    vec![
                        ContainerSlotItem { slot: 0, item_id: f, count: fc },
                        ContainerSlotItem { slot: 1, item_id: "minecraft:coal", count: cc },
                    ],
                )
            }
            "minecraft:furnace" | "minecraft:blast_furnace" => {
                let ores = if bid == "minecraft:blast_furnace" || bp.spec.category == BuildCategory::Forge {
                    &["minecraft:raw_iron", "minecraft:raw_copper", "minecraft:raw_gold"][..]
                } else {
                    &["minecraft:raw_iron", "minecraft:cobblestone", "minecraft:sand"][..]
                };
                let ore = ores[rng.range_i32(0, (ores.len() - 1) as i32) as usize];
                let oc = rng.range_i32(6, 14) as i8;
                let cc = rng.range_i32(4, 9) as i8;
                let title = if bid == "minecraft:blast_furnace" {
                    tr(lang, "Forge Blast Furnace", "Плавильня кузницы")
                } else {
                    tr(lang, "Stone Furnace", "Печь")
                };
                (
                    title.to_owned(),
                    vec![
                        ContainerSlotItem { slot: 0, item_id: ore, count: oc },
                        ContainerSlotItem { slot: 1, item_id: "minecraft:coal", count: cc },
                    ],
                )
            }
            "minecraft:brewing_stand" => {
                let reagents = [
                    "minecraft:nether_wart",
                    "minecraft:glowstone_dust",
                    "minecraft:sugar",
                    "minecraft:spider_eye",
                ];
                let reagent = reagents[rng.range_i32(0, (reagents.len() - 1) as i32) as usize];
                let fuel = rng.range_i32(1, 3) as i8;
                (
                    tr(lang, "Alchemist Stand", "Стойка алхимика").to_owned(),
                    vec![
                        ContainerSlotItem { slot: 0, item_id: "minecraft:glass_bottle", count: 1 },
                        ContainerSlotItem { slot: 1, item_id: "minecraft:glass_bottle", count: 1 },
                        ContainerSlotItem { slot: 2, item_id: "minecraft:glass_bottle", count: 1 },
                        ContainerSlotItem { slot: 3, item_id: reagent, count: rng.range_i32(2, 4) as i8 },
                        ContainerSlotItem { slot: 4, item_id: "minecraft:blaze_powder", count: fuel },
                    ],
                )
            }
            _ => {
                // Chest or Barrel: contextual thematic pool.
                let is_barrel = bid == "minecraft:barrel";
                let (title, pool): (&str, &[(&str, i32, i32)]) = if bp.spec.theme == StructureTheme::MageTower
                    || bp.spec.theme == StructureTheme::Apothecary
                    || bp.spec.theme == StructureTheme::Observatory
                {
                    (
                        if is_barrel {
                            tr(lang, "Reagent Barrel", "Бочка реагентов")
                        } else {
                            tr(lang, "Arcane Archive", "Тайный архив")
                        },
                        &[
                            ("minecraft:glass_bottle", 2, 5),
                            ("minecraft:amethyst_shard", 2, 6),
                            ("minecraft:glowstone_dust", 2, 6),
                            ("minecraft:lapis_lazuli", 3, 8),
                            ("minecraft:book", 1, 3),
                            ("minecraft:blaze_powder", 1, 2),
                            ("minecraft:ender_pearl", 1, 2),
                        ],
                    )
                } else if bp.spec.theme == StructureTheme::Library || bp.spec.theme == StructureTheme::Academy {
                    (
                        if is_barrel {
                            tr(lang, "Scroll Barrel", "Бочка со свитками")
                        } else {
                            tr(lang, "Scholar's Chest", "Сундук учёного")
                        },
                        &[
                            ("minecraft:book", 2, 5),
                            ("minecraft:paper", 4, 12),
                            ("minecraft:writable_book", 1, 2),
                            ("minecraft:lapis_lazuli", 2, 6),
                            ("minecraft:compass", 1, 1),
                            ("minecraft:candle", 2, 4),
                            ("minecraft:emerald", 1, 3),
                        ],
                    )
                } else if bp.spec.theme == StructureTheme::Tavern || bp.spec.theme == StructureTheme::WineryBakery {
                    (
                        if is_barrel {
                            tr(lang, "Tavern Ale Keg", "Бочка таверны")
                        } else {
                            tr(lang, "Tavern Larder", "Кладовая таверны")
                        },
                        &[
                            ("minecraft:bread", 3, 8),
                            ("minecraft:cooked_beef", 2, 5),
                            ("minecraft:pumpkin_pie", 1, 3),
                            ("minecraft:honey_bottle", 1, 4),
                            ("minecraft:apple", 3, 7),
                            ("minecraft:baked_potato", 3, 8),
                            ("minecraft:wheat", 4, 10),
                        ],
                    )
                } else if bp.spec.theme == StructureTheme::SailingShip || bp.spec.theme == StructureTheme::ShipyardDock || bp.spec.category == BuildCategory::Bridge {
                    (
                        if is_barrel {
                            tr(lang, "Harbor Cargo Keg", "Портовая бочка")
                        } else {
                            tr(lang, "Navigator's Chest", "Сундук морехода")
                        },
                        &[
                            ("minecraft:cod", 3, 8),
                            ("minecraft:salmon", 2, 6),
                            ("minecraft:compass", 1, 1),
                            ("minecraft:spyglass", 1, 1),
                            ("minecraft:lantern", 1, 3),
                            ("minecraft:emerald", 1, 4),
                            ("minecraft:bread", 2, 6),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Forge {
                    (
                        if is_barrel {
                            tr(lang, "Coal & Oil Barrel", "Бочка угля и смазки")
                        } else {
                            tr(lang, "Master Smith's Chest", "Сундук кузнеца")
                        },
                        &[
                            ("minecraft:iron_ingot", 3, 9),
                            ("minecraft:coal", 6, 16),
                            ("minecraft:copper_ingot", 3, 8),
                            ("minecraft:raw_iron", 3, 8),
                            ("minecraft:iron_pickaxe", 1, 1),
                            ("minecraft:iron_sword", 1, 1),
                            ("minecraft:shield", 1, 1),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Castle || bp.spec.theme == StructureTheme::BankVault {
                    (
                        if is_barrel {
                            tr(lang, "Garrison Provisions", "Припасы гарнизона")
                        } else {
                            tr(lang, "Citadel Treasury", "Сокровищница цитадели")
                        },
                        &[
                            ("minecraft:iron_sword", 1, 1),
                            ("minecraft:arrow", 6, 16),
                            ("minecraft:bread", 3, 8),
                            ("minecraft:gold_ingot", 2, 6),
                            ("minecraft:emerald", 1, 4),
                            ("minecraft:shield", 1, 1),
                            ("minecraft:torch", 4, 10),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Barn {
                    (
                        if is_barrel {
                            tr(lang, "Grain Barrel", "Бочка с зерном")
                        } else {
                            tr(lang, "Stable Tack Chest", "Сундук конюшни")
                        },
                        &[
                            ("minecraft:wheat", 5, 14),
                            ("minecraft:carrot", 4, 10),
                            ("minecraft:potato", 4, 10),
                            ("minecraft:saddle", 1, 1),
                            ("minecraft:lead", 1, 2),
                            ("minecraft:leather", 2, 6),
                            ("minecraft:apple", 3, 7),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Temple {
                    (
                        if is_barrel {
                            tr(lang, "Incense & Wax Barrel", "Бочка с воском")
                        } else {
                            tr(lang, "Sanctuary Reliquary", "Ларец святилища")
                        },
                        &[
                            ("minecraft:gold_ingot", 2, 5),
                            ("minecraft:lapis_lazuli", 3, 7),
                            ("minecraft:candle", 2, 6),
                            ("minecraft:amethyst_shard", 2, 5),
                            ("minecraft:emerald", 1, 3),
                            ("minecraft:honey_bottle", 1, 3),
                            ("minecraft:book", 1, 3),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Dungeon {
                    (
                        if is_barrel {
                            tr(lang, "Miner's Supply Barrel", "Шахтёрская бочка")
                        } else {
                            tr(lang, "Crypt Reliquary", "Ларец подземелья")
                        },
                        &[
                            ("minecraft:gold_ingot", 2, 6),
                            ("minecraft:iron_ingot", 3, 8),
                            ("minecraft:emerald", 1, 4),
                            ("minecraft:amethyst_shard", 2, 6),
                            ("minecraft:coal", 5, 14),
                            ("minecraft:torch", 4, 12),
                            ("minecraft:golden_apple", 1, 2),
                            ("minecraft:bread", 2, 5),
                        ],
                    )
                } else if bp.spec.category == BuildCategory::Estate {
                    (
                        if is_barrel {
                            tr(lang, "Vineyard Cask", "Усадебная бочка")
                        } else {
                            tr(lang, "Estate Provision Chest", "Усадебный сундук")
                        },
                        &[
                            ("minecraft:wheat", 5, 14),
                            ("minecraft:apple", 3, 8),
                            ("minecraft:bread", 3, 7),
                            ("minecraft:sweet_berries", 3, 8),
                            ("minecraft:honey_bottle", 1, 3),
                            ("minecraft:carrot", 3, 9),
                            ("minecraft:iron_ingot", 1, 4),
                        ],
                    )
                } else if ly < 0 {
                    (
                        if is_barrel {
                            tr(lang, "Cellar Cask", "Бочка погреба")
                        } else {
                            tr(lang, "Cellar Supplies", "Запасы погреба")
                        },
                        &[
                            ("minecraft:potato", 4, 10),
                            ("minecraft:bread", 2, 6),
                            ("minecraft:honey_bottle", 1, 3),
                            ("minecraft:apple", 2, 6),
                            ("minecraft:coal", 4, 10),
                            ("minecraft:sweet_berries", 3, 8),
                        ],
                    )
                } else if ly >= upper_floor_y {
                    (
                        if is_barrel {
                            tr(lang, "Study Keg", "Бочонок кабинета")
                        } else {
                            tr(lang, "Traveler's Trunk", "Дорожный сундук")
                        },
                        &[
                            ("minecraft:book", 1, 3),
                            ("minecraft:paper", 3, 8),
                            ("minecraft:compass", 1, 1),
                            ("minecraft:lantern", 1, 2),
                            ("minecraft:emerald", 1, 3),
                            ("minecraft:spyglass", 1, 1),
                        ],
                    )
                } else {
                    (
                        if is_barrel {
                            tr(lang, "Pantry Barrel", "Кладовая бочка")
                        } else {
                            tr(lang, "Homestead Chest", "Домашний сундук")
                        },
                        &[
                            ("minecraft:bread", 2, 6),
                            ("minecraft:apple", 2, 6),
                            ("minecraft:baked_potato", 2, 6),
                            ("minecraft:torch", 3, 8),
                            ("minecraft:iron_ingot", 1, 4),
                            ("minecraft:coal", 3, 8),
                            ("minecraft:paper", 2, 6),
                        ],
                    )
                };

                let item_count = if is_barrel {
                    rng.range_i32(3, 5) as usize
                } else {
                    rng.range_i32(4, 7) as usize
                };
                let mut used_slots = [false; 27];
                let mut items = Vec::with_capacity(item_count);
                for _ in 0..item_count {
                    let &(item_id, min_c, max_c) = &pool[rng.range_i32(0, (pool.len() - 1) as i32) as usize];
                    let mut slot = rng.range_i32(0, 26) as usize;
                    for _ in 0..27 {
                        if !used_slots[slot] {
                            break;
                        }
                        slot = (slot + 5) % 27;
                    }
                    if used_slots[slot] {
                        continue;
                    }
                    used_slots[slot] = true;
                    let count = rng.range_i32(min_c, max_c).clamp(1, 64) as i8;
                    items.push(ContainerSlotItem {
                        slot: slot as i8,
                        item_id,
                        count,
                    });
                }
                (title.to_owned(), items)
            }
        };

        out.push(PostBuildContainer {
            pos,
            custom_name,
            items,
        });
    }

    out
}

fn apply_post_build_containers(
    host: &mut dyn ServerHost,
    dimension: &str,
    containers: &[PostBuildContainer],
) {
    if containers.is_empty() {
        return;
    }
    let Some(level) = host.level(dimension) else {
        return;
    };
    for c in containers {
        let Some(be) = host.block_entity(level, c.pos) else {
            continue;
        };
        let list_tags: Vec<Tag> = c
            .items
            .iter()
            .map(|it| {
                Tag::compound([
                    ("Slot", Tag::Byte(it.slot)),
                    ("id", Tag::String(it.item_id.into())),
                    ("count", Tag::Int(i32::from(it.count))),
                ])
            })
            .collect();
        let Tag::Compound(root) = Tag::compound([("Items", Tag::List(list_tags))]) else {
            continue;
        };
        let _ = be.merge_data(host, root);
        let _ = be.set_name(host, Some(&c.custom_name));
    }
}

fn sign_lines_for_purpose(purpose: SignPurpose, lang: UiLang) -> [String; 4] {
    let raw: [&str; 4] = match (purpose, lang) {
        (SignPurpose::Tavern, UiLang::Ru) => ["", "ТАВЕРНА", "«Тёплый очаг»", "Открыто 24ч"],
        (SignPurpose::Tavern, UiLang::En) => ["", "THE HEARTH", "& BARREL TAVERN", "Warm Ale & Beds"],
        (SignPurpose::Bakery, UiLang::Ru) => ["", "ПЕКАРНЯ", "Свежий хлеб", "и выпечка"],
        (SignPurpose::Bakery, UiLang::En) => ["", "ARTISAN", "BAKERY & MILL", "Fresh Daily"],
        (SignPurpose::Library, UiLang::Ru) => ["", "БИБЛИОТЕКА", "Тишина и", "древние фолианты"],
        (SignPurpose::Library, UiLang::En) => ["", "GRAND LIBRARY", "Archive & Lore", "Please Keep Quiet"],
        (SignPurpose::Forge, UiLang::Ru) => ["", "КУЗНИЦА", "Клинки, щиты", "и наковальни"],
        (SignPurpose::Forge, UiLang::En) => ["", "MASTER FORGE", "Blades & Armor", "Tempered Steel"],
        (SignPurpose::Apothecary, UiLang::Ru) => ["", "АПТЕКАРЬ", "Зелья, травы", "и эликсиры"],
        (SignPurpose::Apothecary, UiLang::En) => ["", "APOTHECARY", "Potions, Herbs", "& Rare Tonics"],
        (SignPurpose::Station, UiLang::Ru) => ["", "ВОКЗАЛ", "Платформа 1", "Поезда по распис."],
        (SignPurpose::Station, UiLang::En) => ["", "CENTRAL DEPOT", "Platform 1", "All Lines Board"],
        (SignPurpose::Harbor, UiLang::Ru) => ["", "ГАВАНЬ", "Причал и шлюзы", "Счастливого пути"],
        (SignPurpose::Harbor, UiLang::En) => ["", "HARBOR & LOCK", "Port Authority", "Fair Winds"],
        (SignPurpose::PowerPlant, UiLang::Ru) => ["", "ОСТОРОЖНО!", "Подстанция", "Высокое напряж."],
        (SignPurpose::PowerPlant, UiLang::En) => ["", "DANGER!", "HIGH VOLTAGE", "Substation Grid"],
        (SignPurpose::FarmBarn, UiLang::Ru) => ["", "ФЕРМА", "Свежее зерно", "и урожай"],
        (SignPurpose::FarmBarn, UiLang::En) => ["", "HOMESTEAD FARM", "Grain & Harvest", "Storehouse"],
        (SignPurpose::Vineyard, UiLang::Ru) => ["", "ВИНОДЕЛЬНЯ", "Виноградник", "и прессовый дом"],
        (SignPurpose::Vineyard, UiLang::En) => ["", "VINEYARD", "Estate Winery", "& Cellar Press"],
        (SignPurpose::Apiary, UiLang::Ru) => ["", "ПАСЕКА", "Цветочный мёд", "и фруктовый сад"],
        (SignPurpose::Apiary, UiLang::En) => ["", "ORCHARD APIARY", "Wildflower Honey", "& Blossom Grove"],
        (SignPurpose::Stables, UiLang::Ru) => ["", "КОНЮШНЯ", "Рысаки, сёдла", "и тёплые стойла"],
        (SignPurpose::Stables, UiLang::En) => ["", "ROYAL STABLES", "Horses, Feed", "& Saddlery"],
        (SignPurpose::StatuePlaque, UiLang::Ru) => ["", "ПАМЯТНИК", "Страж этих", "древних земель"],
        (SignPurpose::StatuePlaque, UiLang::En) => ["", "MONUMENT", "Guardian of the", "Realm Plaza"],
        (SignPurpose::FountainPlaque, UiLang::Ru) => ["", "ФОНТАН", "Площадь мира", "и вдохновения"],
        (SignPurpose::FountainPlaque, UiLang::En) => ["", "PLAZA FOUNTAIN", "Waters of Peace", "& Prosperity"],
        (SignPurpose::StreetCorner, UiLang::Ru) => ["", "УЛ. РАССВЕТНАЯ", "К площади ->", "<- К воротам"],
        (SignPurpose::StreetCorner, UiLang::En) => ["", "MARKET ROW", "Square ->", "<- City Gate"],
        (SignPurpose::DungeonWarning, UiLang::Ru) => ["ОПАСНОСТЬ!", "Вход в подземелье", "Проверьте щит", "и факелы!"],
        (SignPurpose::DungeonWarning, UiLang::En) => ["DANGER AHEAD!", "Crypt & Vaults", "Bring Shields,", "Food & Torches"],
        (SignPurpose::Waypost, UiLang::Ru) => ["", "УКАЗАТЕЛЬ", "Безопасный путь", "построен модом"],
        (SignPurpose::Waypost, UiLang::En) => ["", "WAYPOST", "Traveler's Road", "Keep to the Path"],
    };
    raw.map(str::to_owned)
}

fn lectern_book_content(kind: LecternBookKind, lang: UiLang) -> (&'static str, &'static str, Vec<&'static str>) {
    match (kind, lang) {
        (LecternBookKind::LibraryChronicle, UiLang::Ru) => (
            "Хроники Архива",
            "Главный Архивариус",
            vec![
                "Глава I\nВеликая летопись возведения городов, храмов и чудес света в этом краю.",
                "Глава II\nБерегите древние манускрипты, изучайте ремёсла и поддерживайте свет в фонарях.",
            ],
        ),
        (LecternBookKind::LibraryChronicle, UiLang::En) => (
            "Chronicle of the Realm",
            "Grand Archivist",
            vec![
                "Chapter I\nA record of ancient halls, soaring spires, and timber estates across these lands.",
                "Chapter II\nPreserve old manuscripts, tend the shelves, and keep the reading lamps lit.",
            ],
        ),
        (LecternBookKind::MageGrimoire, UiLang::Ru) => (
            "Гримуар Арканы",
            "Архимаг",
            vec![
                "Трактат о кристаллах\nАметистовые жилы резонируют с чарами на рассвете.",
                "Алхимическая заметка\nНе оставляйте зелья ускорения без присмотра рядом с варочной стойкой.",
            ],
        ),
        (LecternBookKind::MageGrimoire, UiLang::En) => (
            "Arcane Grimoire",
            "Archmage",
            vec![
                "On Amethyst Harmonics\nCrystal shards resonate with enchanting tables at dusk.",
                "Brewing Caution\nKeep fermented spider eyes sealed inside the apothecary cellar.",
            ],
        ),
        (LecternBookKind::ObservatoryStarChart, UiLang::Ru) => (
            "Звёздный Атлас",
            "Астроном",
            vec![
                "Наблюдение 42\nСозвездие Кузнеца восходит точно над северным хребтом.",
                "Навигация\nКалибруйте медную подзорную трубу каждую новолунную ночь.",
            ],
        ),
        (LecternBookKind::ObservatoryStarChart, UiLang::En) => (
            "Celestial Star Chart",
            "Royal Astronomer",
            vec![
                "Observation XLII\nThe Anvil constellation rises true above the northern ridge.",
                "Calibration Log\nRealign the copper spyglass cradle after every eclipse.",
            ],
        ),
        (LecternBookKind::TempleCodex, UiLang::Ru) => (
            "Священный Кодекс",
            "Хранитель Храма",
            vec![
                "Завет Света\nВсякий путник найдёт здесь укрытие, тишину и тепло очага.",
                "Устав\nЗвоните в колокол лишь на рассвете или при приближении грозы.",
            ],
        ),
        (LecternBookKind::TempleCodex, UiLang::En) => (
            "Sacred Sanctuary Codex",
            "Temple Keeper",
            vec![
                "Vow of Lanterns\nEvery traveler may find shelter, peace, and warm bread within.",
                "Bell Rule\nRing the belfry at dawn or when gathering rainclouds approach.",
            ],
        ),
        (LecternBookKind::CastleDecree, UiLang::Ru) => (
            "Королевский Указ",
            "Кастелян Цитадели",
            vec![
                "Гарнизонный приказ\nУкреплять стены, дежурить у ворот и поддерживать оружейную в чистоте.",
                "Снабжение\nБочонки с припасами распределять на рассвете сменным стражам.",
            ],
        ),
        (LecternBookKind::CastleDecree, UiLang::En) => (
            "Royal Garrison Decree",
            "Citadel Castellan",
            vec![
                "Standing Orders\nMan the gatehouse battlements and inspect the armory racks daily.",
                "Quartermaster Note\nIssue iron rations and torches from the barrel stores at dawn.",
            ],
        ),
        (LecternBookKind::TownCharter, UiLang::Ru) => (
            "Городская Хартия",
            "Бургомистр",
            vec![
                "Устав города\nМощёные улицы и ночные фонари поддерживаются общиной в порядке.",
                "Ярмарочные дни\nТорговые ряды открываются на центральной площади по субботам.",
            ],
        ),
        (LecternBookKind::TownCharter, UiLang::En) => (
            "Municipal Town Charter",
            "Mayor of the District",
            vec![
                "Civic By-Laws\nAll cobbled avenues and daylight lanterns are kept clear by the guild.",
                "Market Days\nMerchants gather around the central plaza each morning.",
            ],
        ),
        (LecternBookKind::EngineeringLog, UiLang::Ru) => (
            "Инженерный Журнал",
            "Главный Механик",
            vec![
                "Схема механизмов\nРедукторы, шлюзовые затворы и рельсовые пути прошли проверку нагрузки.",
                "Техника безопасности\nПроверяйте редстоун-датчики освещения и предохранители.",
            ],
        ),
        (LecternBookKind::EngineeringLog, UiLang::En) => (
            "Engineering Field Log",
            "Chief Machinist",
            vec![
                "Inspection Report\nLock gates, gantry hoists, and rail turntables passed load checks.",
                "Safety Protocol\nVerify redstone daylight relays and insulator coils before nightfall.",
            ],
        ),
        (LecternBookKind::FarmAlmanac, UiLang::Ru) => (
            "Альманах Фермера",
            "Староста Хутора",
            vec![
                "Посевной календарь\nОрошайте грядки каждые четыре блока и вовремя собирайте мёд из ульев.",
                "Конюшня и скот\nНаполняйте поилки водой и пополняйте запас сена до зимы.",
            ],
        ),
        (LecternBookKind::FarmAlmanac, UiLang::En) => (
            "Homestead Farm Almanac",
            "Master Grower",
            vec![
                "Harvest Calendar\nKeep irrigation ditches flowing and tend orchard beehives gently.",
                "Livestock Care\nRefill water cauldrons and stack dry hay bales inside the barn loft.",
            ],
        ),
        (LecternBookKind::DungeonJournal, UiLang::Ru) => (
            "Дневник Исследователя",
            "Пропавший Следопыт",
            vec![
                "Запись 7\nНиже второго яруса слышен звон костей и скрип каменных плит.",
                "Предупреждение\nОстерегайтесь нажимных плит и раздатчиков у алтаря сокровищ!",
            ],
        ),
        (LecternBookKind::DungeonJournal, UiLang::En) => (
            "Explorer's Field Notes",
            "Missing Delver",
            vec![
                "Entry VII\nBeyond the switchback stairs, echoes stir beneath the deeper deepslate vaults.",
                "Trap Warning\nWatch for stone pressure plates and wall dispensers near the lower sanctum!",
            ],
        ),
        (LecternBookKind::StudyNotes, UiLang::Ru) => (
            "Заметки Мастера",
            "Зодчий",
            vec![
                "Чертежи дома\nКаждый этаж спланирован с учётом дневного света, уюта и тёплого очага.",
            ],
        ),
        (LecternBookKind::StudyNotes, UiLang::En) => (
            "Architect's Notebook",
            "Master Builder",
            vec![
                "Floor Plan Notes\nBuilt with warm hearths, sturdy timber frames, and sunlit windows.",
            ],
        ),
    }
}

fn banner_layers_for_style(style: BannerStyle, accent: &str) -> Vec<(&'static str, &'static str)> {
    match style {
        BannerStyle::RoyalCastle => vec![
            ("minecraft:border", "block.minecraft.banner.border"),
            ("minecraft:flower", "block.minecraft.banner.flower"),
        ],
        BannerStyle::TempleSacred => vec![
            ("minecraft:cross", "block.minecraft.banner.cross"),
            ("minecraft:border", "block.minecraft.banner.border"),
        ],
        BannerStyle::GuildCraft => vec![
            ("minecraft:bricks", "block.minecraft.banner.bricks"),
            ("minecraft:stripe_center", "block.minecraft.banner.stripe_center"),
        ],
        BannerStyle::HarborNautical => vec![
            ("minecraft:stripe_bottom", "block.minecraft.banner.stripe_bottom"),
            ("minecraft:rhombus", "block.minecraft.banner.rhombus"),
        ],
        BannerStyle::ParkBotanical => vec![
            ("minecraft:gradient_up", "block.minecraft.banner.gradient_up"),
            ("minecraft:flower", "block.minecraft.banner.flower"),
        ],
        BannerStyle::EngineerCog => vec![
            ("minecraft:creeper", "block.minecraft.banner.creeper"),
            ("minecraft:border", "block.minecraft.banner.border"),
        ],
    }
    .into_iter()
    .map(|(id, tk)| {
        let _ = accent;
        (id, tk)
    })
    .collect()
}

fn facing_yaw(facing: CardinalFacing) -> f32 {
    match facing {
        CardinalFacing::South => 0.0,
        CardinalFacing::West => 90.0,
        CardinalFacing::North => 180.0,
        CardinalFacing::East => -90.0,
    }
}

fn mc_direction_from_facing(facing: CardinalFacing) -> Direction {
    match facing {
        CardinalFacing::North => Direction::North,
        CardinalFacing::South => Direction::South,
        CardinalFacing::East => Direction::East,
        CardinalFacing::West => Direction::West,
    }
}

fn biome_villager_type(biome_tag: &str) -> &'static str {
    let b = biome_tag.to_ascii_lowercase();
    if b.contains("desert") || b.contains("badlands") {
        "minecraft:desert"
    } else if b.contains("snow") || b.contains("ice") || b.contains("frozen") {
        "minecraft:snow"
    } else if b.contains("taiga") {
        "minecraft:taiga"
    } else if b.contains("savanna") {
        "minecraft:savanna"
    } else if b.contains("swamp") || b.contains("mangrove") {
        "minecraft:swamp"
    } else if b.contains("jungle") {
        "minecraft:jungle"
    } else {
        "minecraft:plains"
    }
}

fn apply_post_build_extras(
    host: &mut dyn ServerHost,
    dimension: &str,
    origin: [i32; 3],
    living: &LivingPlan,
    lang: UiLang,
) -> Vec<u32> {
    let mut spawned_ids = Vec::new();
    let Some(level) = host.level(dimension) else {
        return spawned_ids;
    };

    // 1. Populate Lecterns with bilingual written books
    for lec in &living.lecterns {
        let wpos = [origin[0] + lec.pos[0], origin[1] + lec.pos[1], origin[2] + lec.pos[2]];
        if let Some(be) = host.block_entity(level, wpos) {
            let (title, author, pages) = lectern_book_content(lec.book, lang);
            let page_tags: Vec<Tag> = pages.into_iter().map(|p| Tag::String(p.into())).collect();
            let book_content = Tag::compound([
                ("title", Tag::String(title.into())),
                ("author", Tag::String(author.into())),
                ("pages", Tag::List(page_tags)),
            ]);
            let book_item = Tag::compound([
                ("id", Tag::String("minecraft:written_book".into())),
                ("count", Tag::Int(1)),
                (
                    "components",
                    Tag::compound([("minecraft:written_book_content", book_content)]),
                ),
            ]);
            if let Tag::Compound(root) = Tag::compound([("Book", book_item), ("Page", Tag::Int(0))]) {
                let _ = be.merge_data(host, root);
            }
        }
    }

    // 2. Populate Wall & Hanging Signs with localized text
    for sign in &living.signs {
        let wpos = [origin[0] + sign.pos[0], origin[1] + sign.pos[1], origin[2] + sign.pos[2]];
        if let Some(be) = host.block_entity(level, wpos) {
            let lines = match (lang, &sign.custom_en, &sign.custom_ru) {
                (UiLang::Ru, _, Some(ru)) => ru.clone(),
                (UiLang::En, Some(en), _) => en.clone(),
                _ => sign_lines_for_purpose(sign.purpose, lang),
            };
            let msg_tags: Vec<Tag> = lines.iter().map(|s| Tag::String(s.as_str().into())).collect();
            let front = Tag::compound([
                ("messages", Tag::List(msg_tags)),
                ("color", Tag::String("black".into())),
                ("has_glowing_text", Tag::Byte(0)),
            ]);
            if let Tag::Compound(root) =
                Tag::compound([("front_text", front), ("is_waxed", Tag::Byte(1))])
            {
                let _ = be.merge_data(host, root);
            }
        }
    }

    // 3. Populate Banners with Heraldic Inline Patterns
    for ban in &living.banners {
        let wpos = [origin[0] + ban.pos[0], origin[1] + ban.pos[1], origin[2] + ban.pos[2]];
        if let Some(be) = host.block_entity(level, wpos) {
            let layers = banner_layers_for_style(ban.style, ban.accent_color);
            let pattern_list: Vec<Tag> = layers
                .into_iter()
                .map(|(asset_id, tkey)| {
                    Tag::compound([
                        (
                            "pattern",
                            Tag::compound([
                                ("asset_id", Tag::String(asset_id.into())),
                                ("translation_key", Tag::String(tkey.into())),
                            ]),
                        ),
                        ("color", Tag::String(ban.accent_color.into())),
                    ])
                })
                .collect();
            if let Tag::Compound(root) = Tag::compound([("patterns", Tag::List(pattern_list))]) {
                let _ = be.merge_data(host, root);
            }
        }
    }

    // 4. Summon Living Entities & Decor Entities (ArmorStands, ItemFrames, Paintings, Villagers, Mobs)
    for ent in &living.entities {
        let wx = origin[0] + ent.pos[0];
        let wy = origin[1] + ent.pos[1];
        let wz = origin[2] + ent.pos[2];
        let spawn_xyz = [
            f64::from(wx) + 0.5,
            f64::from(wy) + match ent.kind {
                LivingEntityKind::Painting
                | LivingEntityKind::ItemFrame
                | LivingEntityKind::GlowItemFrame => 0.5,
                _ => 0.05,
            },
            f64::from(wz) + 0.5,
        ];

        let (kind_id, root_tag, set_dir) = match ent.kind {
            LivingEntityKind::ArmorStand => {
                let mat = if ent.detail.is_empty() { "iron" } else { ent.detail.as_str() };
                let mut eq = vec![
                    ("head", Tag::compound([("id", Tag::String(format!("minecraft:{mat}_helmet").as_str().into())), ("count", Tag::Int(1))])),
                    ("chest", Tag::compound([("id", Tag::String(format!("minecraft:{mat}_chestplate").as_str().into())), ("count", Tag::Int(1))])),
                    ("legs", Tag::compound([("id", Tag::String(format!("minecraft:{mat}_leggings").as_str().into())), ("count", Tag::Int(1))])),
                    ("feet", Tag::compound([("id", Tag::String(format!("minecraft:{mat}_boots").as_str().into())), ("count", Tag::Int(1))])),
                ];
                if !ent.sub_detail.is_empty() {
                    let wpn_id = if ent.sub_detail.starts_with("minecraft:") {
                        ent.sub_detail.clone()
                    } else {
                        format!("minecraft:{}", ent.sub_detail)
                    };
                    eq.push(("mainhand", Tag::compound([("id", Tag::String(wpn_id.as_str().into())), ("count", Tag::Int(1))])));
                }
                let yaw = facing_yaw(ent.facing);
                let nbt = Tag::compound([
                    ("ShowArms", Tag::Byte(1)),
                    ("Rotation", Tag::List(vec![Tag::Float(yaw.to_bits()), Tag::Float(0.0f32.to_bits())])),
                    ("equipment", Tag::compound(eq)),
                ]);
                ("minecraft:armor_stand", nbt, false)
            }
            LivingEntityKind::ItemFrame | LivingEntityKind::GlowItemFrame => {
                let id = if ent.kind == LivingEntityKind::GlowItemFrame {
                    "minecraft:glow_item_frame"
                } else {
                    "minecraft:item_frame"
                };
                let item = if ent.detail.is_empty() {
                    "minecraft:compass"
                } else {
                    ent.detail.as_str()
                };
                let nbt = Tag::compound([
                    ("Fixed", Tag::Byte(1)),
                    ("block_pos", Tag::IntArray(vec![wx, wy, wz])),
                    (
                        "Item",
                        Tag::compound([("id", Tag::String(item.into())), ("count", Tag::Int(1))]),
                    ),
                ]);
                (id, nbt, true)
            }
            LivingEntityKind::Painting => {
                let variant = if ent.detail.is_empty() {
                    "minecraft:kebab"
                } else {
                    ent.detail.as_str()
                };
                let nbt = Tag::compound([
                    ("variant", Tag::String(variant.into())),
                    ("block_pos", Tag::IntArray(vec![wx, wy, wz])),
                ]);
                ("minecraft:painting", nbt, true)
            }
            LivingEntityKind::Villager => {
                let prof = if ent.detail.is_empty() {
                    "minecraft:mason".to_owned()
                } else if ent.detail.starts_with("minecraft:") {
                    ent.detail.clone()
                } else {
                    format!("minecraft:{}", ent.detail)
                };
                let vtype = biome_villager_type(&ent.sub_detail);
                let nbt = Tag::compound([
                    (
                        "VillagerData",
                        Tag::compound([
                            ("level", Tag::Int(2)),
                            ("profession", Tag::String(prof.as_str().into())),
                            ("type", Tag::String(vtype.into())),
                        ]),
                    ),
                    ("VillagerDataFinalized", Tag::Byte(1)),
                ]);
                ("minecraft:villager", nbt, false)
            }
            LivingEntityKind::IronGolem => {
                ("minecraft:iron_golem", Tag::compound([("PlayerCreated", Tag::Byte(1))]), false)
            }
            LivingEntityKind::Horse => {
                ("minecraft:horse", Tag::compound([("Tame", Tag::Byte(1))]), false)
            }
            LivingEntityKind::Cow => ("minecraft:cow", Tag::Compound(Default::default()), false),
            LivingEntityKind::Sheep => ("minecraft:sheep", Tag::Compound(Default::default()), false),
            LivingEntityKind::Bee => ("minecraft:bee", Tag::Compound(Default::default()), false),
            LivingEntityKind::Cat => ("minecraft:cat", Tag::Compound(Default::default()), false),
            LivingEntityKind::Skeleton => ("minecraft:skeleton", Tag::Compound(Default::default()), false),
            LivingEntityKind::Zombie => ("minecraft:zombie", Tag::Compound(Default::default()), false),
        };

        let Tag::Compound(comp) = root_tag else {
            continue;
        };
        if let Some(eid) = host.summon(dimension, kind_id, spawn_xyz, comp) {
            if set_dir && let Some(eref) = host.entity(eid) {
                let _ = eref.set_hanging_direction(host, mc_direction_from_facing(ent.facing));
            }
            spawned_ids.push(eid);
        }
    }

    spawned_ids
}

/// Extends radial biome-aware terrain blending outside the `[0..w, 0..d]` footprint (`blend_radius` 1..=10)
/// into `bp.blocks` before `prepare_undo_and_filter_blueprint`, so Undo/Redo restores the full blended apron.
fn apply_outer_halo_blend(
    host: &dyn ServerHost,
    req: &BuildRequest,
    bp: &mut GeneratedBlueprint,
) {
    if !req.adapt_terrain
        || req.blend_radius == 0
        || bp.spec.on_stilts
        || matches!(bp.spec.category, BuildCategory::Bridge)
    {
        return;
    }
    let r = i32::from(req.blend_radius.min(10));
    let w = i32::from(bp.width);
    let d = i32::from(bp.depth);
    let h_lim = i32::from(bp.height) + 2;

    // Map boundary columns of the generated structure to anchor height around base level.
    let mut edge_anchors: HashMap<(i32, i32), i32> = HashMap::new();
    for (&[lx, ly, lz], blk) in &bp.blocks {
        if blk.block_id != "minecraft:air"
            && (-6..=8).contains(&ly)
            && (lx <= 1 || lx >= w - 2 || lz <= 1 || lz >= d - 2)
        {
            edge_anchors
                .entry((lx, lz))
                .and_modify(|y| *y = (*y).min(ly))
                .or_insert(ly);
        }
    }
    if edge_anchors.is_empty() {
        return;
    }

    let mut halo_cols = Vec::new();
    let mut query_xz = Vec::new();
    for lz in (-r)..(d + r) {
        for lx in (-r)..(w + r) {
            if (0..w).contains(&lx) && (0..d).contains(&lz) {
                continue;
            }
            if let Some([dx, _, dz]) = bp.door_pos
                && (lx - dx).abs() <= 1
                && (lz - dz).abs() <= 2
            {
                continue;
            }
            let mut best: Option<(i32, i32)> = None;
            for dz in -r..=r {
                for dx in -r..=r {
                    if let Some(&ay) = edge_anchors.get(&(lx + dx, lz + dz)) {
                        let d2 = dx * dx + dz * dz;
                        if best.is_none_or(|(bd2, _)| d2 < bd2) {
                            best = Some((d2, ay.clamp(-6, 6)));
                        }
                    }
                }
            }
            let Some((d2, anchor_y)) = best else {
                continue;
            };
            let dist = (d2 as f64).sqrt();
            if dist <= f64::from(r) + 0.35 {
                halo_cols.push((lx, lz, dist, anchor_y));
                query_xz.push((bp.origin[0] + lx, bp.origin[2] + lz));
            }
        }
    }
    if query_xz.is_empty() {
        return;
    }

    let Ok(surf_heights) =
        host.surface_heights(&req.dimension, &query_xz, Heightmap::MotionBlockingNoLeaves)
    else {
        return;
    };
    let floor_heights = host
        .surface_heights(&req.dimension, &query_xz, Heightmap::OceanFloor)
        .ok();
    let noise = crate::noise::NoiseField::new(req.seed as u64);

    for (idx, (lx, lz, dist, anchor_y)) in halo_cols.into_iter().enumerate() {
        let Some(Some(sy)) = surf_heights.get(idx).copied() else {
            continue;
        };
        if let Some(Some(fy)) = floor_heights.as_ref().and_then(|v| v.get(idx).copied())
            && sy > fy
        {
            continue;
        }
        let terr = ((sy - 1) - bp.origin[1]).clamp(-16, 16);
        crate::generator::apply_radial_terrain_column(
            &mut bp.blocks,
            lx,
            lz,
            dist,
            r,
            terr,
            anchor_y,
            h_lim,
            &bp.spec.biome_tag,
            bp.spec.style,
            &req.palette,
            req.seed as u64,
            noise.cell_hash(lx, 0, lz),
        );
    }
}

fn prepare_undo_and_filter_blueprint(
    host: &dyn ServerHost,
    dimension: &str,
    bp: &mut GeneratedBlueprint,
    air_only: bool,
) -> Option<UndoEntry> {
    let level = host.level(dimension)?;
    if air_only {
        let air_keys = collect_air_keys(host);
        let orig = bp.origin;
        bp.blocks.retain(|&[lx, ly, lz], planned| {
            if matches!(
                planned.block_id.as_str(),
                "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
            ) {
                return false;
            }
            let pos = [orig[0] + lx, orig[1] + ly, orig[2] + lz];
            level
                .block_state(host, pos)
                .is_some_and(|st| air_keys.contains(&st.block()))
        });
    }
    retain_complete_doors(&mut bp.blocks);

    let mut previous_blocks = Vec::with_capacity(bp.blocks.len());
    for &[lx, ly, lz] in bp.blocks.keys() {
        let pos = [bp.origin[0] + lx, bp.origin[1] + ly, bp.origin[2] + lz];
        if let Some(prev) = level.block_state(host, pos) {
            previous_blocks.push((pos, prev));
        }
    }
    if previous_blocks.is_empty() {
        None
    } else {
        Some(UndoEntry {
            dimension: dimension.to_owned(),
            previous_blocks,
            spawned_entities: Vec::new(),
        })
    }
}

fn capture_positions_snapshot(
    host: &dyn ServerHost,
    dimension: &str,
    entries: &[([i32; 3], BlockState)],
) -> Option<UndoEntry> {
    let level = host.level(dimension)?;
    let mut current = Vec::with_capacity(entries.len());
    for &(pos, _) in entries {
        if let Some(st) = level.block_state(host, pos) {
            current.push((pos, st));
        }
    }
    if current.is_empty() {
        None
    } else {
        Some(UndoEntry {
            dimension: dimension.to_owned(),
            previous_blocks: current,
            spawned_entities: Vec::new(),
        })
    }
}

fn apply_state_snapshot(
    host: &mut dyn ServerHost,
    dimension: &str,
    entries: &[([i32; 3], BlockState)],
) -> Result<u64, String> {
    let mut palette: Vec<BlockState> = Vec::new();
    let mut state_to_idx: HashMap<BlockState, u32> = HashMap::new();
    let mut runs = Vec::with_capacity(entries.len());

    for &(pos, state) in entries {
        let idx = if let Some(&existing) = state_to_idx.get(&state) {
            existing
        } else {
            let idx = palette.len() as u32;
            palette.push(state);
            state_to_idx.insert(state, idx);
            idx
        };
        runs.push(BlockRun {
            x: pos[0],
            z: pos[2],
            y: pos[1],
            height: 1,
            surface: None,
            block: idx,
            only: None,
        });
    }

    let mut total_changed = 0u64;
    for chunk in runs.chunks(MAX_RUNS_PER_BATCH) {
        let edit = BlockEdit {
            palette: palette.clone(),
            filters: Vec::new(),
            runs: chunk.to_vec(),
        };
        total_changed += host.edit_blocks(dimension, &edit)?;
    }
    Ok(total_changed)
}

fn push_history_entry(
    stack_map: &mut HashMap<u128, Vec<UndoEntry>>,
    owner_key: u128,
    entry: UndoEntry,
) {
    let stack = stack_map.entry(owner_key).or_default();
    if stack.len() >= MAX_UNDO_HISTORY_PER_PLAYER {
        stack.remove(0);
    }
    stack.push(entry);
}

const HISTORY_MAGIC: &[u8; 4] = b"BGH1";
const MAX_PERSIST_BYTES: usize = 48 * 1024 * 1024;
const MAX_DECODE_CELLS_PER_ENTRY: usize = 2_000_000;

fn history_config_path(kind: &str, owner_key: u128) -> Option<ConfigPath> {
    ConfigPath::new(&format!("{kind}_{owner_key:032x}.bin")).ok()
}

fn sync_history_world(host: &dyn ServerHost) {
    let cur_world = host.world_dir();
    let changed = {
        let mut guard = HISTORY_WORLD_DIR.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(prev) if *prev == cur_world => false,
            _ => {
                *guard = Some(cur_world);
                true
            }
        }
    };
    if changed {
        with_undo_map(|m| m.clear());
        with_redo_map(|m| m.clear());
        with_last_requests(|m| m.clear());
        let mut loaded = LOADED_HISTORY_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        loaded.get_or_insert_with(HashSet::new).clear();
    }
}

fn encode_single_entry(host: &dyn ServerHost, entry: &UndoEntry) -> Option<Vec<u8>> {
    let mut state_to_pal: HashMap<BlockState, u16> = HashMap::new();
    let mut name_to_pal: HashMap<String, u16> = HashMap::new();
    let mut palette: Vec<String> = Vec::new();
    let mut cells: Vec<(i32, i32, i32, u16)> = Vec::with_capacity(entry.previous_blocks.len());

    for &(pos, state) in &entry.previous_blocks {
        let pal_idx = if let Some(&idx) = state_to_pal.get(&state) {
            idx
        } else {
            let Some(name) = host.block_state_string(state) else {
                continue;
            };
            let idx = if let Some(&idx) = name_to_pal.get(&name) {
                idx
            } else {
                let Ok(idx) = u16::try_from(palette.len()) else {
                    continue;
                };
                palette.push(name.clone());
                name_to_pal.insert(name, idx);
                idx
            };
            state_to_pal.insert(state, idx);
            idx
        };
        cells.push((pos[0], pos[2], pos[1], pal_idx));
    }

    if cells.is_empty() {
        return None;
    }

    // Sort by (x, z, y) so vertical runs collapse cleanly into compact RLE segments.
    cells.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2)));

    let mut runs: Vec<(i32, i32, i32, u16, u16)> = Vec::with_capacity(cells.len() / 2 + 1);
    let mut i = 0;
    while i < cells.len() {
        let (x, z, y0, pal_idx) = cells[i];
        let mut len: u16 = 1;
        while i + usize::from(len) < cells.len() && len < u16::MAX {
            let (nx, nz, ny, np) = cells[i + usize::from(len)];
            if nx == x && nz == z && np == pal_idx && ny == y0.saturating_add(i32::from(len)) {
                len += 1;
            } else {
                break;
            }
        }
        runs.push((x, y0, z, len, pal_idx));
        i += usize::from(len);
    }

    let dim_bytes = entry.dimension.as_bytes();
    let dim_len = u16::try_from(dim_bytes.len()).ok()?;
    let pal_len = u32::try_from(palette.len()).ok()?;
    let run_len = u32::try_from(runs.len()).ok()?;

    let mut out = Vec::with_capacity(2 + dim_bytes.len() + 4 + palette.len() * 24 + runs.len() * 16);
    out.extend_from_slice(&dim_len.to_le_bytes());
    out.extend_from_slice(dim_bytes);
    out.extend_from_slice(&pal_len.to_le_bytes());
    for name in &palette {
        let nb = name.as_bytes();
        let nl = u16::try_from(nb.len()).ok()?;
        out.extend_from_slice(&nl.to_le_bytes());
        out.extend_from_slice(nb);
    }
    out.extend_from_slice(&run_len.to_le_bytes());
    for (x, y0, z, len, pal_idx) in runs {
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y0.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&pal_idx.to_le_bytes());
    }
    Some(out)
}

fn encode_history_stack(host: &dyn ServerHost, entries: &[UndoEntry]) -> Vec<u8> {
    let start = entries.len().saturating_sub(MAX_UNDO_HISTORY_PER_PLAYER);
    let mut encoded_entries: Vec<Vec<u8>> = entries[start..]
        .iter()
        .filter_map(|e| encode_single_entry(host, e))
        .collect();
    // Respect horizon ConfigFiles 64 MiB hard cap comfortably by pruning oldest entries if ever needed.
    while !encoded_entries.is_empty()
        && 5 + encoded_entries.iter().map(Vec::len).sum::<usize>() > MAX_PERSIST_BYTES
    {
        encoded_entries.remove(0);
    }
    let total_len = 5 + encoded_entries.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(HISTORY_MAGIC);
    out.push(encoded_entries.len() as u8);
    for chunk in encoded_entries {
        out.extend_from_slice(&chunk);
    }
    out
}

fn decode_history_stack(host: &dyn ServerHost, bytes: &[u8]) -> Option<Vec<UndoEntry>> {
    if bytes.len() < 5 || &bytes[0..4] != HISTORY_MAGIC {
        return None;
    }
    let count = (bytes[4] as usize).min(MAX_UNDO_HISTORY_PER_PLAYER);
    let mut cur = 5usize;

    let take = |cur: &mut usize, n: usize| -> Option<&[u8]> {
        let end = cur.checked_add(n)?;
        let slice = bytes.get(*cur..end)?;
        *cur = end;
        Some(slice)
    };
    let take_u16 = |cur: &mut usize| -> Option<u16> {
        let s = take(cur, 2)?;
        Some(u16::from_le_bytes([s[0], s[1]]))
    };
    let take_u32 = |cur: &mut usize| -> Option<u32> {
        let s = take(cur, 4)?;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let take_i32 = |cur: &mut usize| -> Option<i32> {
        let s = take(cur, 4)?;
        Some(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let dim_len = usize::from(take_u16(&mut cur)?);
        let dim = std::str::from_utf8(take(&mut cur, dim_len)?).ok()?.to_owned();
        let pal_len = take_u32(&mut cur)? as usize;
        if pal_len > 65_536 {
            return None;
        }
        let mut palette: Vec<Option<BlockState>> = Vec::with_capacity(pal_len);
        for _ in 0..pal_len {
            let nl = usize::from(take_u16(&mut cur)?);
            let name = std::str::from_utf8(take(&mut cur, nl)?).ok()?;
            palette.push(host.block_state_id(name));
        }
        let run_len = take_u32(&mut cur)? as usize;
        if run_len > MAX_DECODE_CELLS_PER_ENTRY {
            return None;
        }
        let mut previous_blocks = Vec::with_capacity(run_len.min(65_536));
        for _ in 0..run_len {
            let x = take_i32(&mut cur)?;
            let y0 = take_i32(&mut cur)?;
            let z = take_i32(&mut cur)?;
            let len = take_u16(&mut cur)?;
            let pal_idx = usize::from(take_u16(&mut cur)?);
            if previous_blocks.len().saturating_add(usize::from(len)) > MAX_DECODE_CELLS_PER_ENTRY {
                return None;
            }
            if let Some(Some(state)) = palette.get(pal_idx).copied() {
                for dy in 0..i32::from(len) {
                    previous_blocks.push(([x, y0 + dy, z], state));
                }
            }
        }
        if !previous_blocks.is_empty() {
            out.push(UndoEntry {
                dimension: dim,
                previous_blocks,
                spawned_entities: Vec::new(),
            });
        }
    }
    Some(out)
}

fn ensure_history_loaded(host: &dyn ServerHost, owner_key: u128) {
    sync_history_world(host);
    {
        let mut loaded = LOADED_HISTORY_OWNERS.lock().unwrap_or_else(|e| e.into_inner());
        let set = loaded.get_or_insert_with(HashSet::new);
        if !set.insert(owner_key) {
            return;
        }
    }
    let files = host.world_data_files();
    if let Some(p) = history_config_path("undo", owner_key)
        && let Ok(bytes) = files.read(&p)
        && let Some(mut entries) = decode_history_stack(host, &bytes)
    {
        if entries.len() > MAX_UNDO_HISTORY_PER_PLAYER {
            entries.drain(..entries.len() - MAX_UNDO_HISTORY_PER_PLAYER);
        }
        if !entries.is_empty() {
            with_undo_map(|map| {
                map.entry(owner_key).or_insert(entries);
            });
        }
    }
    if let Some(p) = history_config_path("redo", owner_key)
        && let Ok(bytes) = files.read(&p)
        && let Some(mut entries) = decode_history_stack(host, &bytes)
    {
        if entries.len() > MAX_UNDO_HISTORY_PER_PLAYER {
            entries.drain(..entries.len() - MAX_UNDO_HISTORY_PER_PLAYER);
        }
        if !entries.is_empty() {
            with_redo_map(|map| {
                map.entry(owner_key).or_insert(entries);
            });
        }
    }
}

fn persist_owner_history(host: &dyn ServerHost, owner_key: u128) {
    let undo_entries = with_undo_map(|map| map.get(&owner_key).cloned().unwrap_or_default());
    let redo_entries = with_redo_map(|map| map.get(&owner_key).cloned().unwrap_or_default());
    let files = host.world_data_files();
    if let Some(p) = history_config_path("undo", owner_key) {
        let encoded = encode_history_stack(host, &undo_entries);
        let _ = files.write(&p, &encoded);
    }
    if let Some(p) = history_config_path("redo", owner_key) {
        let encoded = encode_history_stack(host, &redo_entries);
        let _ = files.write(&p, &encoded);
    }
}

fn sample_terrain_info(
    host: &dyn ServerHost,
    req: &BuildRequest,
) -> (Option<Vec<i32>>, Option<Vec<i32>>) {
    if !req.adapt_terrain {
        return (None, None);
    }
    let w = i32::from(req.width.clamp(7, 96));
    let d = i32::from(req.depth.clamp(7, 96));
    let mut cols = Vec::with_capacity((w * d) as usize);
    for lz in 0..d {
        for lx in 0..w {
            cols.push((req.origin_x + lx, req.origin_z + lz));
        }
    }
    let Ok(surf_heights) = host.surface_heights(&req.dimension, &cols, Heightmap::MotionBlockingNoLeaves) else {
        return (None, None);
    };
    let floor_heights = host
        .surface_heights(&req.dimension, &cols, Heightmap::OceanFloor)
        .ok();
    let offsets: Vec<i32> = surf_heights
        .iter()
        .map(|opt| opt.map_or(0, |sy| (sy - 1) - req.origin_y))
        .collect();
    let water_depths = floor_heights.map(|floors| {
        surf_heights
            .iter()
            .zip(floors.iter())
            .map(|(s, f)| match (s, f) {
                (Some(sy), Some(fy)) if sy > fy => (sy - fy).clamp(0, 16),
                _ => 0,
            })
            .collect()
    });
    (Some(offsets), water_depths)
}

fn undo_depth(owner_key: u128) -> usize {
    with_undo_map(|map| map.get(&owner_key).map_or(0, Vec::len))
}

fn redo_depth(owner_key: u128) -> usize {
    with_redo_map(|map| map.get(&owner_key).map_or(0, Vec::len))
}

/// Core server execution used by both typed payload requests and `/buildgen` chat commands.
pub fn execute_build(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    req: BuildRequest,
) -> BuildResponse {
    let lang = player_lang(player);
    if let Some(p) = player
        && !player_has_build_permission(host, p)
    {
        return deny_permission_response(lang);
    }

    let mut req = sanitize_build_request(req);
    if host.level(&req.dimension).is_none() {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: match lang {
                UiLang::En => format!("Unknown dimension: {}", req.dimension),
                UiLang::Ru => format!("Неизвестное измерение: {}", req.dimension),
            },
        };
    }

    if req.biome_hint.is_empty() && req.style == crate::model::BuildStyle::Auto {
        let cx = req.origin_x + i32::from(req.width) / 2;
        let cz = req.origin_z + i32::from(req.depth) / 2;
        let sampled = sample_biome_at(host, &req.dimension, cx, req.origin_y, cz);
        if !sampled.is_empty() {
            req.biome_hint = sampled;
            if req.palette == MaterialPalette::preset(0)
                && !crate::parser::description_names_explicit_material(&req.description)
            {
                req.palette = crate::parser::infer_palette_with_biome(
                    req.style,
                    &req.description,
                    req.seed as u64,
                    &req.biome_hint,
                );
            }
        }
    }

    let (terrain_offsets, water_depths) = sample_terrain_info(host, &req);
    let mut bp = crate::generator::generate_blueprint_with_water(
        &req,
        terrain_offsets.as_deref(),
        water_depths.as_deref(),
    );
    apply_outer_halo_blend(host, &req, &mut bp);
    with_last_requests(|map| {
        map.insert(player.map_or(0, PlayerRef::uuid), req.clone());
    });
    place_blueprint_with_history(host, player, &req.dimension, req.seed, req.air_only, &mut bp, lang, None)
}

#[allow(clippy::too_many_arguments)]
fn place_blueprint_with_history(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    dimension: &str,
    seed: i64,
    air_only: bool,
    bp: &mut GeneratedBlueprint,
    lang: UiLang,
    custom_label: Option<String>,
) -> BuildResponse {
    let owner_key = player.map_or(0, PlayerRef::uuid);
    ensure_history_loaded(host, owner_key);
    let undo_snapshot = prepare_undo_and_filter_blueprint(host, dimension, bp, air_only);
    let post_containers = collect_post_build_containers(bp, seed as u64, lang);
    let post_living = bp.living.clone();
    let post_origin = bp.origin;

    if air_only && bp.blocks.is_empty() {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: tr(
                lang,
                "No air blocks available inside target area (Air Only).",
                "В выбранной области нет воздушных блоков (режим «Только воздух»).",
            )
            .to_owned(),
        };
    }

    let (palette, phase1_runs, phase2_runs) = match blueprint_to_phased_runs(host, bp) {
        Ok(res) => res,
        Err(err) => {
            return BuildResponse {
                ok: false,
                blocks_changed: 0,
                message: match lang {
                    UiLang::En => format!("Palette error: {err}"),
                    UiLang::Ru => format!("Ошибка палитры: {err}"),
                },
            };
        }
    };

    let total_runs = phase1_runs.len() + phase2_runs.len();
    if total_runs == 0 {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: match lang {
                UiLang::En => "Generator produced no blocks.".into(),
                UiLang::Ru => "Генератор не создал блоков.".into(),
            },
        };
    }

    if let Some(snapshot) = undo_snapshot {
        with_undo_map(|map| push_history_entry(map, owner_key, snapshot));
    }
    with_redo_map(|map| {
        map.remove(&owner_key);
    });
    persist_owner_history(host, owner_key);

    if total_runs <= MAX_RUNS_PER_BATCH {
        let mut changed = 0u64;
        if !phase1_runs.is_empty() {
            let edit1 = BlockEdit {
                palette: palette.clone(),
                filters: Vec::new(),
                runs: phase1_runs,
            };
            match host.edit_blocks(dimension, &edit1) {
                Ok(c) => changed += c,
                Err(err) => {
                    return BuildResponse {
                        ok: false,
                        blocks_changed: 0,
                        message: match lang {
                            UiLang::En => format!("Failed to place structure: {err}"),
                            UiLang::Ru => format!("Не удалось разместить постройку: {err}"),
                        },
                    };
                }
            }
        }
        if !phase2_runs.is_empty() {
            let edit2 = BlockEdit {
                palette,
                filters: Vec::new(),
                runs: phase2_runs,
            };
            match host.edit_blocks(dimension, &edit2) {
                Ok(c) => changed += c,
                Err(err) => {
                    return BuildResponse {
                        ok: false,
                        blocks_changed: changed.min(i32::MAX as u64) as i32,
                        message: match lang {
                            UiLang::En => format!("Failed to place decor/doors: {err}"),
                            UiLang::Ru => format!("Не удалось разместить декор/двери: {err}"),
                        },
                    };
                }
            }
        }
        apply_post_build_containers(host, dimension, &post_containers);
        let spawned = apply_post_build_extras(host, dimension, post_origin, &post_living, lang);
        if !spawned.is_empty() {
            with_undo_map(|map| {
                if let Some(last) = map.get_mut(&owner_key).and_then(|s| s.last_mut()) {
                    last.spawned_entities.extend(spawned);
                }
            });
        }
        let undo_count = undo_depth(owner_key);
        BuildResponse {
            ok: true,
            blocks_changed: changed.min(i32::MAX as u64) as i32,
            message: if let Some(lbl) = custom_label {
                match lang {
                    UiLang::En => format!("{lbl} ({changed} blocks) [undo: {undo_count}/{MAX_UNDO_HISTORY_PER_PLAYER} | redo: 0/{MAX_UNDO_HISTORY_PER_PLAYER}]."),
                    UiLang::Ru => format!("{lbl} ({changed} бл.) [отмен: {undo_count}/{MAX_UNDO_HISTORY_PER_PLAYER} | возврат: 0/{MAX_UNDO_HISTORY_PER_PLAYER}]."),
                }
            } else {
                match lang {
                    UiLang::En => format!(
                        "Ready: {} ({}) {}x{}x{} (seed {}, blocks changed: {}) [undo: {}/{} | redo: 0/{}].",
                        bp.spec.category.label(UiLang::En),
                        bp.spec.style.label(UiLang::En),
                        bp.width,
                        bp.depth,
                        bp.height,
                        seed,
                        changed,
                        undo_count,
                        MAX_UNDO_HISTORY_PER_PLAYER,
                        MAX_UNDO_HISTORY_PER_PLAYER
                    ),
                    UiLang::Ru => format!(
                        "Готово: {} ({}) {}x{}x{} (сид {}, изменено блоков: {}) [отмен: {}/{} | возврат: 0/{}].",
                        bp.spec.category.label_ru(),
                        bp.spec.style.label_ru(),
                        bp.width,
                        bp.depth,
                        bp.height,
                        seed,
                        changed,
                        undo_count,
                        MAX_UNDO_HISTORY_PER_PLAYER,
                        MAX_UNDO_HISTORY_PER_PLAYER
                    ),
                }
            },
        }
    } else {
        let undo_count = undo_depth(owner_key);
        let mut chunks: VecDeque<Vec<BlockRun>> = phase1_runs
            .chunks(MAX_RUNS_PER_BATCH)
            .map(|c| c.to_vec())
            .collect();
        // Keep vertical pairs in the same (x, z) column (such as door lower + upper halves) in one batch.
        let mut p2_idx = 0;
        while p2_idx < phase2_runs.len() {
            let mut end = (p2_idx + MAX_RUNS_PER_BATCH).min(phase2_runs.len());
            if end < phase2_runs.len() {
                let split_x = phase2_runs[end - 1].x;
                let split_z = phase2_runs[end - 1].z;
                while end < phase2_runs.len()
                    && phase2_runs[end].x == split_x
                    && phase2_runs[end].z == split_z
                {
                    end += 1;
                }
            }
            chunks.push_back(phase2_runs[p2_idx..end].to_vec());
            p2_idx = end;
        }
        let first = chunks.pop_front().unwrap_or_default();
        let first_count = first.len();
        let edit = BlockEdit {
            palette: palette.clone(),
            filters: Vec::new(),
            runs: first,
        };
        match host.edit_blocks(dimension, &edit) {
            Ok(first_changed) => {
                if chunks.is_empty() {
                    apply_post_build_containers(host, dimension, &post_containers);
                    let spawned = apply_post_build_extras(host, dimension, post_origin, &post_living, lang);
                    if !spawned.is_empty() {
                        with_undo_map(|map| {
                            if let Some(last) = map.get_mut(&owner_key).and_then(|s| s.last_mut()) {
                                last.spawned_entities.extend(spawned);
                            }
                        });
                    }
                } else {
                    let job_seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
                    let label = custom_label.unwrap_or_else(|| {
                        format!(
                            "{} {}x{}",
                            bp.spec.category.label(lang),
                            bp.width,
                            bp.depth
                        )
                    });
                    let pct = ((first_count * 100) / total_runs.max(1)).min(99) as i32;
                    let boss_bar = if let Some(p) = player {
                        let bar_id = format!("building_generator:build_{job_seq}");
                        let bar_opt = host
                            .create_boss_bar(
                                &bar_id,
                                Text::literal(format!("Building Generator: {label} ({pct}%)")),
                            )
                            .ok()
                            .flatten();
                        if let Some(b) = bar_opt {
                            let _ = b.set_color(host, BossBarColor::Blue);
                            let _ = b.set_max(host, 100);
                            let _ = b.set_value(host, pct);
                            let _ = b.add_player(host, p);
                        }
                        bar_opt
                    } else {
                        None
                    };
                    if let Some(p) = player
                        && let Some(ch) = channels()
                    {
                        let _ = ch.to_client_reply.send(
                            host,
                            p,
                            &BuildResponse {
                                ok: true,
                                blocks_changed: first_changed.min(i32::MAX as u64) as i32,
                                message: format!("__PROGRESS__:{pct}:{first_count}:{total_runs}:{label}"),
                            },
                        );
                    }
                    with_pending_jobs(|q| {
                        q.push_back(PendingBatchJob {
                            dimension: dimension.to_owned(),
                            palette,
                            chunks,
                            completed_runs: first_count,
                            total_runs,
                            player_uuid: player.map(PlayerRef::uuid),
                            boss_bar,
                            label,
                            lang,
                            post_containers,
                            post_living,
                            post_origin,
                        });
                    });
                    let _ = host.schedule_task(1, BATCH_TASK_TOKEN);
                }
                BuildResponse {
                    ok: true,
                    blocks_changed: first_changed.min(i32::MAX as u64) as i32,
                    message: match lang {
                        UiLang::En => format!(
                            "Large build ({} blocks/segments) building per tick (seed {}) [undo: {}/{} | redo: 0/{}].",
                            total_runs, seed, undo_count, MAX_UNDO_HISTORY_PER_PLAYER, MAX_UNDO_HISTORY_PER_PLAYER
                        ),
                        UiLang::Ru => format!(
                            "Крупная постройка ({} блоков/сегментов) строится по тикам (сид {}) [отмен: {}/{} | возврат: 0/{}].",
                            total_runs, seed, undo_count, MAX_UNDO_HISTORY_PER_PLAYER, MAX_UNDO_HISTORY_PER_PLAYER
                        ),
                    },
                }
            }
            Err(err) => BuildResponse {
                ok: false,
                blocks_changed: 0,
                message: match lang {
                    UiLang::En => format!("Placement error: {err}"),
                    UiLang::Ru => format!("Ошибка размещения: {err}"),
                },
            },
        }
    }
}

pub fn process_scheduled_task(host: &mut dyn ServerHost, event: &mut ScheduledTaskEvent<'_>) {
    if event.owner != "building_generator" || event.token != BATCH_TASK_TOKEN {
        return;
    }

    let mut need_reschedule = false;
    let mut progress_updates: Vec<(Option<u128>, i32, usize, usize, String)> = Vec::new();
    let mut bar_updates: Vec<(BossBarRef, i32, String)> = Vec::new();
    let mut bars_to_remove: Vec<BossBarRef> = Vec::new();
    let mut finished_notifications: Vec<(Option<u128>, String)> = Vec::new();
    let mut finished_containers: Vec<(String, Vec<PostBuildContainer>)> = Vec::new();
    let mut finished_living: Vec<(Option<u128>, String, [i32; 3], LivingPlan, UiLang)> = Vec::new();

    with_pending_jobs(|q| {
        while let Some(job) = q.front_mut() {
            if !host.server_has_time() {
                need_reschedule = true;
                break;
            }
            let Some(chunk) = job.chunks.pop_front() else {
                let completed = q.pop_front().unwrap();
                if !completed.post_containers.is_empty() {
                    finished_containers.push((completed.dimension.clone(), completed.post_containers));
                }
                finished_living.push((
                    completed.player_uuid,
                    completed.dimension,
                    completed.post_origin,
                    completed.post_living,
                    completed.lang,
                ));
                if let Some(bar) = completed.boss_bar {
                    bars_to_remove.push(bar);
                }
                let msg = match completed.lang {
                    UiLang::En => format!(
                        "[Building Generator] Build complete ({} vertical segments).",
                        completed.total_runs
                    ),
                    UiLang::Ru => format!(
                        "[Building Generator] Завершена постройка ({} вертикальных сегментов).",
                        completed.total_runs
                    ),
                };
                finished_notifications.push((completed.player_uuid, msg));
                continue;
            };
            let chunk_len = chunk.len();
            let edit = BlockEdit {
                palette: job.palette.clone(),
                filters: Vec::new(),
                runs: chunk,
            };
            let _ = host.edit_blocks(&job.dimension, &edit);
            job.completed_runs = (job.completed_runs + chunk_len).min(job.total_runs);
            let pct = ((job.completed_runs * 100) / job.total_runs.max(1)).min(100) as i32;
            if job.chunks.is_empty() {
                let completed = q.pop_front().unwrap();
                if !completed.post_containers.is_empty() {
                    finished_containers.push((completed.dimension.clone(), completed.post_containers));
                }
                finished_living.push((
                    completed.player_uuid,
                    completed.dimension,
                    completed.post_origin,
                    completed.post_living,
                    completed.lang,
                ));
                if let Some(bar) = completed.boss_bar {
                    bars_to_remove.push(bar);
                }
                let msg = match completed.lang {
                    UiLang::En => format!(
                        "[Building Generator] Build complete ({} vertical segments).",
                        completed.total_runs
                    ),
                    UiLang::Ru => format!(
                        "[Building Generator] Завершена постройка ({} вертикальных сегментов).",
                        completed.total_runs
                    ),
                };
                finished_notifications.push((completed.player_uuid, msg));
            } else {
                if let Some(bar) = job.boss_bar {
                    bar_updates.push((bar, pct, job.label.clone()));
                }
                progress_updates.push((
                    job.player_uuid,
                    pct,
                    job.completed_runs,
                    job.total_runs,
                    job.label.clone(),
                ));
            }
        }
    });

    for (dim, containers) in finished_containers {
        apply_post_build_containers(host, &dim, &containers);
    }
    for (uuid_opt, dim, origin, living, lang) in finished_living {
        let spawned = apply_post_build_extras(host, &dim, origin, &living, lang);
        if !spawned.is_empty() {
            let owner_key = uuid_opt.unwrap_or(0);
            with_undo_map(|map| {
                if let Some(last) = map.get_mut(&owner_key).and_then(|s| s.last_mut()) {
                    last.spawned_entities.extend(spawned);
                }
            });
        }
    }

    for (bar, pct, label) in bar_updates {
        let _ = bar.set_value(host, pct);
        let _ = bar.set_name(
            host,
            Text::literal(format!("Building Generator: {label} ({pct}%)")),
        );
    }
    for bar in bars_to_remove {
        let _ = bar.remove(host);
    }
    for (uuid_opt, pct, done, total, label) in progress_updates {
        if let Some(uuid) = uuid_opt
            && let Some(player) = host.player(uuid)
            && let Some(ch) = channels()
        {
            let _ = ch.to_client_reply.send(
                host,
                player,
                &BuildResponse {
                    ok: true,
                    blocks_changed: done.min(i32::MAX as usize) as i32,
                    message: format!("__PROGRESS__:{pct}:{done}:{total}:{label}"),
                },
            );
        }
    }

    for (uuid_opt, msg) in finished_notifications {
        if let Some(uuid) = uuid_opt
            && let Some(player) = host.player(uuid)
        {
            let _ = player.tell(host, Text::literal(&msg));
            if let Some(ch) = channels() {
                let _ = ch.to_client_reply.send(
                    host,
                    player,
                    &BuildResponse {
                        ok: true,
                        blocks_changed: 0,
                        message: msg.clone(),
                    },
                );
            }
        }
    }

    if need_reschedule {
        let _ = host.schedule_task(1, BATCH_TASK_TOKEN);
    }
}

fn execute_history_step(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    dimension: &str,
    is_redo: bool,
) -> BuildResponse {
    let lang = player_lang(player);
    if let Some(p) = player
        && !player_has_build_permission(host, p)
    {
        return deny_permission_response(lang);
    }

    let owner_key = player.map_or(0, PlayerRef::uuid);
    ensure_history_loaded(host, owner_key);
    let popped = if is_redo {
        with_redo_map(|map| map.get_mut(&owner_key).and_then(Vec::pop))
    } else {
        with_undo_map(|map| map.get_mut(&owner_key).and_then(Vec::pop))
    };
    let Some(entry) = popped else {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: if is_redo {
                tr(
                    lang,
                    "No undone builds to redo.",
                    "Нет отменённых построек для возврата (Redo).",
                )
                .to_owned()
            } else {
                tr(
                    lang,
                    "No saved builds to undo.",
                    "Нет сохранённых построек для отмены.",
                )
                .to_owned()
            },
        };
    };

    let target_dim = if entry.dimension.is_empty() {
        dimension
    } else {
        &entry.dimension
    };

    let inverse = capture_positions_snapshot(host, target_dim, &entry.previous_blocks);
    let total_changed = match apply_state_snapshot(host, target_dim, &entry.previous_blocks) {
        Ok(c) => c,
        Err(err) => {
            if is_redo {
                with_redo_map(|map| push_history_entry(map, owner_key, entry));
            } else {
                with_undo_map(|map| push_history_entry(map, owner_key, entry));
            }
            return BuildResponse {
                ok: false,
                blocks_changed: 0,
                message: match (lang, is_redo) {
                    (UiLang::En, false) => format!("Undo error: {err}"),
                    (UiLang::Ru, false) => format!("Ошибка отката Undo: {err}"),
                    (UiLang::En, true) => format!("Redo error: {err}"),
                    (UiLang::Ru, true) => format!("Ошибка повтора Redo: {err}"),
                },
            };
        }
    };

    if !is_redo {
        for eid in &entry.spawned_entities {
            if let Some(eref) = host.entity(*eid) {
                let _ = eref.discard(host);
            }
        }
    }

    if let Some(inv) = inverse {
        if is_redo {
            with_undo_map(|map| push_history_entry(map, owner_key, inv));
        } else {
            with_redo_map(|map| push_history_entry(map, owner_key, inv));
        }
    }
    persist_owner_history(host, owner_key);

    let u = undo_depth(owner_key);
    let r = redo_depth(owner_key);
    BuildResponse {
        ok: true,
        blocks_changed: total_changed.min(i32::MAX as u64) as i32,
        message: match (lang, is_redo) {
            (UiLang::En, false) => format!(
                "Undo complete: restored {total_changed} blocks [undo: {u}/{MAX_UNDO_HISTORY_PER_PLAYER} | redo: {r}/{MAX_UNDO_HISTORY_PER_PLAYER}]."
            ),
            (UiLang::Ru, false) => format!(
                "Откат выполнен: восстановлено блоков — {total_changed} [отмен: {u}/{MAX_UNDO_HISTORY_PER_PLAYER} | возврат: {r}/{MAX_UNDO_HISTORY_PER_PLAYER}]."
            ),
            (UiLang::En, true) => format!(
                "Redo complete: re-applied {total_changed} blocks [undo: {u}/{MAX_UNDO_HISTORY_PER_PLAYER} | redo: {r}/{MAX_UNDO_HISTORY_PER_PLAYER}]."
            ),
            (UiLang::Ru, true) => format!(
                "Повтор выполнен: возвращено блоков — {total_changed} [отмен: {u}/{MAX_UNDO_HISTORY_PER_PLAYER} | возврат: {r}/{MAX_UNDO_HISTORY_PER_PLAYER}]."
            ),
        },
    }
}

pub fn execute_undo(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    dimension: &str,
) -> BuildResponse {
    execute_history_step(host, player, dimension, false)
}

pub fn execute_redo(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    dimension: &str,
) -> BuildResponse {
    execute_history_step(host, player, dimension, true)
}

pub fn execute_reroll(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    dimension: &str,
) -> BuildResponse {
    let lang = player_lang(player);
    if let Some(p) = player
        && !player_has_build_permission(host, p)
    {
        return deny_permission_response(lang);
    }

    let owner_key = player.map_or(0, PlayerRef::uuid);
    ensure_history_loaded(host, owner_key);
    let prev_opt = with_last_requests(|map| map.get(&owner_key).cloned());
    let Some(mut next_req) = prev_opt else {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: match lang {
                UiLang::En => "No previous build to reroll.".into(),
                UiLang::Ru => "Нет предыдущей постройки для перегенерации (reroll).".into(),
            },
        };
    };

    let target_dim = if next_req.dimension.is_empty() {
        dimension.to_owned()
    } else {
        next_req.dimension.clone()
    };
    let has_undo = with_undo_map(|map| map.get(&owner_key).is_some_and(|s| !s.is_empty()));
    if has_undo {
        let undo_resp = execute_undo(host, player, &target_dim);
        if !undo_resp.ok {
            return undo_resp;
        }
    }

    let seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut new_seed = crate::noise::fresh_entropy(
        seq,
        (next_req.seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    ) as i64;
    if new_seed == next_req.seed {
        new_seed = new_seed.wrapping_add(1);
    }
    next_req.seed = new_seed;
    execute_build(host, player, next_req)
}

fn run_network_action(
    cx: &mut dyn IPayloadContext,
    check_build_rate: bool,
    op: impl FnOnce(&mut dyn ServerHost, PlayerRef) -> BuildResponse,
) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    let resp = {
        let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
        let lang = player_lang(Some(player));
        let r = match check_network_action_allowed(host, player, lang, check_build_rate) {
            Ok(()) => op(host, player),
            Err(deny) => deny,
        };
        let _ = player.tell(host, Text::literal(format!("[Building Generator] {}", r.message)));
        r
    };
    if let Some(ch) = channels() {
        let _ = ch.to_client_reply.reply(cx, &resp);
    }
    Ok(())
}

pub fn handle_build_payload(req: BuildRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    run_network_action(cx, true, |host, player| execute_build(host, Some(player), req))
}

pub fn handle_undo_payload(req: UndoRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    run_network_action(cx, false, |host, player| {
        execute_undo(host, Some(player), &req.dimension)
    })
}

pub fn handle_redo_payload(req: RedoRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    run_network_action(cx, false, |host, player| {
        execute_redo(host, Some(player), &req.dimension)
    })
}

pub fn handle_reroll_payload(req: RerollRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    run_network_action(cx, true, |host, player| {
        execute_reroll(host, Some(player), &req.dimension)
    })
}

/// Executes 2-point terrain-conforming road and automatic bridge construction between `start` and `end`.
pub fn execute_road_connect(
    host: &mut dyn ServerHost,
    player: Option<PlayerRef>,
    mut req: RoadConnectRequest,
) -> BuildResponse {
    let lang = player_lang(player);
    if let Some(p) = player
        && !player_has_build_permission(host, p)
    {
        return deny_permission_response(lang);
    }
    if req.dimension.chars().count() > 128 || req.dimension.trim().is_empty() {
        req.dimension = "minecraft:overworld".into();
    }
    if host.level(&req.dimension).is_none() {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: match lang {
                UiLang::En => format!("Unknown dimension: {}", req.dimension),
                UiLang::Ru => format!("Неизвестное измерение: {}", req.dimension),
            },
        };
    }

    req.road_width = req.road_width.clamp(3, 5);
    req.blend_radius = req.blend_radius.min(10);
    let span_x = (req.end_x - req.start_x).abs();
    let span_z = (req.end_z - req.start_z).abs();
    if span_x > 160 || span_z > 160 {
        return BuildResponse {
            ok: false,
            blocks_changed: 0,
            message: tr(
                lang,
                "Road span exceeds 160 blocks; pick closer endpoints.",
                "Длина дороги превышает 160 блоков; выберите точки ближе.",
            )
            .to_owned(),
        };
    }

    let margin = i32::from(req.road_width) / 2 + i32::from(req.blend_radius) + 3;
    let min_x = req.start_x.min(req.end_x) - margin;
    let max_x = req.start_x.max(req.end_x) + margin;
    let min_z = req.start_z.min(req.end_z) - margin;
    let max_z = req.start_z.max(req.end_z) + margin;
    let bw = (max_x - min_x + 1).clamp(7, 192) as u16;
    let bd = (max_z - min_z + 1).clamp(7, 192) as u16;
    let base_y = req.start_y.min(req.end_y);
    let box_origin = [min_x, base_y, min_z];

    let mut cols = Vec::with_capacity(usize::from(bw) * usize::from(bd));
    for lz in 0..i32::from(bd) {
        for lx in 0..i32::from(bw) {
            cols.push((box_origin[0] + lx, box_origin[2] + lz));
        }
    }
    let offsets = if let Ok(surf) =
        host.surface_heights(&req.dimension, &cols, Heightmap::MotionBlockingNoLeaves)
    {
        surf.iter()
            .map(|opt| opt.map_or(0, |sy| (sy - 1) - base_y))
            .collect::<Vec<i32>>()
    } else {
        vec![0; cols.len()]
    };
    let depths = if let (Ok(surf), Ok(floor)) = (
        host.surface_heights(&req.dimension, &cols, Heightmap::MotionBlockingNoLeaves),
        host.surface_heights(&req.dimension, &cols, Heightmap::OceanFloor),
    ) {
        surf.iter()
            .zip(floor.iter())
            .map(|(s, f)| match (s, f) {
                (Some(sy), Some(fy)) if sy > fy => (sy - fy).clamp(0, 20),
                _ => 0,
            })
            .collect::<Vec<i32>>()
    } else {
        vec![0; cols.len()]
    };

    let mut bp = crate::gen_road::generate_road_connector(&req, &offsets, &depths, box_origin, bw, bd);
    let label = match lang {
        UiLang::En => format!("Road ({},{})->({},{}) [w={}]", req.start_x, req.start_z, req.end_x, req.end_z, req.road_width),
        UiLang::Ru => format!("Дорога ({},{})->({},{}) [ш={}]", req.start_x, req.start_z, req.end_x, req.end_z, req.road_width),
    };
    place_blueprint_with_history(
        host,
        player,
        &req.dimension,
        req.seed,
        req.air_only,
        &mut bp,
        lang,
        Some(label),
    )
}

pub fn handle_road_payload(
    req: RoadConnectRequest,
    cx: &mut dyn IPayloadContext,
) -> Result<(), String> {
    run_network_action(cx, true, |host, player| {
        execute_road_connect(host, Some(player), req)
    })
}

pub fn handle_lang_payload(req: SetLangRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    set_player_lang(Some(player), req.lang);
    let (u, r) = {
        let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
        let owner_key = player.uuid();
        ensure_history_loaded(host, owner_key);
        (undo_depth(owner_key), redo_depth(owner_key))
    };
    if let Some(ch) = channels() {
        let _ = ch.to_client_reply.reply(
            cx,
            &BuildResponse {
                ok: true,
                blocks_changed: 0,
                message: format!("__HISTORY__:{u}:{r}"),
            },
        );
    }
    Ok(())
}

pub(crate) fn sample_biome_at(host: &dyn ServerHost, dimension: &str, x: i32, y: i32, z: i32) -> String {
    let Some(level) = host.level(dimension) else {
        return String::new();
    };
    level.biome_at(host, [x, y, z]).unwrap_or_default()
}

pub fn handle_biome_payload(
    req: crate::model::BiomeQueryRequest,
    cx: &mut dyn IPayloadContext,
) -> Result<(), String> {
    let _player = cx.player().map_err(|e| format!("{e:?}"))?;
    let dim = if req.dimension.trim().is_empty() || req.dimension.chars().count() > 128 {
        "minecraft:overworld".to_owned()
    } else {
        req.dimension
    };
    let biome = {
        let host = cx.server_host().map_err(|e| format!("{e:?}"))?;
        sample_biome_at(host, &dim, req.x, req.y, req.z)
    };
    if let Some(ch) = channels() {
        let _ = ch
            .to_client_biome
            .reply(cx, &crate::model::BiomeQueryResponse { biome });
    }
    Ok(())
}

pub const CLI_PRESETS: &[(&str, BuildCategory, crate::model::BuildStyle, u16, u16, u16, &str)] = &[
    ("teahouse", BuildCategory::House, crate::model::BuildStyle::Japanese, 19, 17, 16, "японский двухэтажный чайный дом с террасой, лавкой, полосатым навесом и фонарями"),
    ("tavern", BuildCategory::House, crate::model::BuildStyle::Medieval, 19, 17, 16, "средневековая двухэтажная таверна с камином и подвалом"),
    ("pagoda", BuildCategory::Tower, crate::model::BuildStyle::Japanese, 13, 13, 20, "многоярусная японская пагода с садом сакуры"),
    ("castle", BuildCategory::Castle, crate::model::BuildStyle::Gothic, 21, 19, 18, "готический замок цитадель с башнями, подвалом и колодцем во дворе"),
    ("windmill", BuildCategory::Tower, crate::model::BuildStyle::Medieval, 15, 15, 18, "деревянная ветряная мельница с лопастями и амбаром"),
    ("lighthouse", BuildCategory::Tower, crate::model::BuildStyle::Modern, 13, 13, 22, "высокий береговой маяк с фонарём и винтовой лестницей"),
    ("greenhouse", BuildCategory::Pavilion, crate::model::BuildStyle::Cottage, 15, 13, 12, "стеклянная теплица оранжерея с зеленью и цветами"),
    ("forge", BuildCategory::Forge, crate::model::BuildStyle::Steampunk, 17, 15, 14, "кузница в стиле стимпанк с медной крышей и наковальней"),
    ("temple", BuildCategory::Temple, crate::model::BuildStyle::Nordic, 19, 17, 18, "нордический каменный собор храм с колоколом и витражами"),
    ("villa", BuildCategory::House, crate::model::BuildStyle::Modern, 21, 19, 14, "современная вилла модерн с плоской крышей, панорамными окнами и бассейном во дворе"),
    ("bridge", BuildCategory::Bridge, crate::model::BuildStyle::Medieval, 15, 35, 14, "арочный каменный мост с аркой и фонарями"),
    ("observatory", BuildCategory::Tower, crate::model::BuildStyle::Steampunk, 13, 13, 20, "обсерватория с куполом и стимпанк башней"),
    ("mansard", BuildCategory::House, crate::model::BuildStyle::Medieval, 15, 13, 14, "французский двухэтажный особняк с мансардной крышей и люкарнами"),
    ("colosseum", BuildCategory::Castle, crate::model::BuildStyle::Classical, 25, 25, 16, "античный римский колизей амфитеатр с арками, колоннами и ареной"),
    ("village", BuildCategory::Settlement, crate::model::BuildStyle::Medieval, 48, 48, 20, "средневековый квартал деревня с таверной, кузницей и площадью"),
    ("suspension_bridge", BuildCategory::Bridge, crate::model::BuildStyle::Modern, 19, 44, 18, "вантовый подвесной мост с пилонами и канатами"),
    ("aqueduct", BuildCategory::Bridge, crate::model::BuildStyle::Classical, 15, 42, 16, "римский двухъярусный акведук мост с каналом воды"),
    ("taiko_bridge", BuildCategory::Bridge, crate::model::BuildStyle::Japanese, 13, 29, 14, "японский горбатый лунный мост тайко с воротами тории"),
    ("gate_bridge", BuildCategory::Bridge, crate::model::BuildStyle::Medieval, 15, 36, 16, "крепостной разводной мост со сторожевыми башнями"),
    ("pyramid", BuildCategory::Temple, crate::model::BuildStyle::Desert, 25, 25, 22, "египетская ступенчатая пирамида зиккурат с гробницей и обелисками"),
    ("triumphal_arch", BuildCategory::Temple, crate::model::BuildStyle::Classical, 21, 13, 18, "триумфальная античная арка с колоннами и террасой"),
    ("clocktower", BuildCategory::Tower, crate::model::BuildStyle::Steampunk, 13, 13, 28, "часовая башня биг-бен с курантами и колоколом"),
    ("watermill", BuildCategory::Forge, crate::model::BuildStyle::Cottage, 19, 17, 15, "водяная мельница у ручья с колесом и лотком воды"),
    ("cathedral", BuildCategory::Temple, crate::model::BuildStyle::Gothic, 23, 31, 24, "готический кафедральный собор с аркбутанами, башнями и окном розой"),
    ("tenshu", BuildCategory::Castle, crate::model::BuildStyle::Japanese, 21, 21, 26, "японский замок тенсю с каменным цоколем и ярусной пагодой"),
    ("bazaar", BuildCategory::Pavilion, crate::model::BuildStyle::Desert, 23, 23, 12, "восточный базар ярмарка с цветными шатрами и фонтаном"),
    ("treehouse", BuildCategory::House, crate::model::BuildStyle::Cottage, 21, 21, 24, "большой дом на дереве с мостиками и пышной кроной"),
    ("onsen", BuildCategory::Pavilion, crate::model::BuildStyle::Japanese, 19, 17, 11, "японский горячий источник онсэн баня с водопадом и павильоном"),
    ("pantheon", BuildCategory::Temple, crate::model::BuildStyle::Classical, 21, 21, 18, "римский пантеон ротонда с куполом и портиком"),
    ("parthenon", BuildCategory::Temple, crate::model::BuildStyle::Classical, 19, 25, 14, "греческий парфенон периптер с колоннадой и фронтоном"),
    ("hanging_gardens", BuildCategory::Temple, crate::model::BuildStyle::Desert, 23, 23, 20, "вавилонские висячие сады семирамиды с террасами и водопадами"),
    ("mosque", BuildCategory::Temple, crate::model::BuildStyle::Desert, 23, 23, 22, "восточная мечеть дворец с бирюзовым куполом и минаретами"),
    ("ishtar_gate", BuildCategory::Castle, crate::model::BuildStyle::Desert, 19, 17, 16, "лазурные вавилонские ворота иштар из глазурованного кирпича"),
    ("angkor", BuildCategory::Temple, crate::model::BuildStyle::Cottage, 23, 23, 22, "древний кхмерский храм ангкор с башнями прангами в джунглях"),
    ("water_shrine", BuildCategory::Temple, crate::model::BuildStyle::Japanese, 19, 31, 14, "японская аллея красных тории на воде святилище ицукусима"),
    ("sphinx", BuildCategory::Temple, crate::model::BuildStyle::Desert, 15, 29, 16, "великий египетский сфинкс страж пирамид из песчаника"),
    ("bailey_castle", BuildCategory::Castle, crate::model::BuildStyle::Medieval, 33, 33, 22, "средневековый концентрический замок мотт и бейли со стенами и донжоном"),
    ("city_gate", BuildCategory::Castle, crate::model::BuildStyle::Medieval, 21, 19, 18, "укреплённые городские ворота барбакан с решёткой и дозорными башнями"),
    ("monastery", BuildCategory::Temple, crate::model::BuildStyle::Medieval, 29, 29, 18, "средневековый монастырь аббатство с клуатром и двориком"),
    ("jousting", BuildCategory::Castle, crate::model::BuildStyle::Medieval, 25, 35, 14, "рыцарское турнирное ристалище с барьером и королевской трибуной"),
    ("outpost", BuildCategory::Castle, crate::model::BuildStyle::Medieval, 25, 25, 14, "военный форпост лагерь за частоколом с дозорной вышкой и требушетом"),
    ("star_fort", BuildCategory::Castle, crate::model::BuildStyle::Classical, 31, 31, 16, "звёздчатый бастионный форт вобана с ромбовидными редутами"),
    ("apothecary", BuildCategory::House, crate::model::BuildStyle::Medieval, 15, 15, 15, "средневековая аптека лавка алхимика зельевара с оранжереей"),
    ("bakery", BuildCategory::House, crate::model::BuildStyle::Cottage, 17, 15, 14, "уютная винодельня пекарня с каменной печью и винным погребом"),
    ("town_hall", BuildCategory::House, crate::model::BuildStyle::Classical, 23, 21, 22, "торжественная городская ратуша мэрия с портиком и часовой башней"),
    ("opera", BuildCategory::Pavilion, crate::model::BuildStyle::Classical, 23, 27, 18, "классический оперный театр со сценой, ложами и куполом"),
    ("museum", BuildCategory::Pavilion, crate::model::BuildStyle::Classical, 23, 25, 18, "музей естественной истории со стеклянным атриумом и скелетом"),
    ("bank", BuildCategory::House, crate::model::BuildStyle::Classical, 19, 21, 16, "мраморный банк сокровищница с колоннами и бронированным сейфом"),
    ("academy", BuildCategory::Castle, crate::model::BuildStyle::Gothic, 25, 29, 22, "магическая академия университет с большим готическим залом и башнями"),
    ("ship", BuildCategory::Pavilion, crate::model::BuildStyle::Medieval, 13, 35, 20, "трёхмачтовый парусный корабль галеон на воде с пушками"),
    ("shipyard", BuildCategory::Settlement, crate::model::BuildStyle::Medieval, 27, 29, 18, "морская гавань верфь стапель с краном, пирсом и строящимся судном"),
    ("station", BuildCategory::Pavilion, crate::model::BuildStyle::Steampunk, 25, 31, 18, "викторианский железнодорожный вокзал с арочным дебаркадером и паровозом"),
    ("airship", BuildCategory::Tower, crate::model::BuildStyle::Steampunk, 21, 31, 26, "причал дирижаблей цеппелинов с высокой мачтой и небесным аэростатом"),
    ("dam", BuildCategory::Bridge, crate::model::BuildStyle::Modern, 31, 19, 18, "бетонная плотина гидроэлектростанция дамба с водосбросом и озером"),
    ("circus", BuildCategory::Pavilion, crate::model::BuildStyle::Cottage, 23, 23, 18, "праздничный шатёр цирк шапито с красно-белым куполом и ареной"),
    ("skyscraper", BuildCategory::Tower, crate::model::BuildStyle::Modern, 19, 19, 44, "современный стеклянный небоскрёб высотка бизнес-центр со шпилем"),
    ("factory", BuildCategory::Forge, crate::model::BuildStyle::Steampunk, 23, 25, 20, "стимпанк завод фабрика с пилообразной крышей, трубами и краном"),
    ("stadium", BuildCategory::Pavilion, crate::model::BuildStyle::Modern, 33, 41, 18, "современный футбольный стадион с трибунами, полем и прожекторами"),
    ("bunker", BuildCategory::Castle, crate::model::BuildStyle::Modern, 21, 21, 14, "укреплённый подземный бункер убежище из железобетона со шлюзом"),
    ("rocket", BuildCategory::Tower, crate::model::BuildStyle::Modern, 21, 21, 36, "космодром со стартовой фермой и высокой космической ракетой"),
    ("oil_rig", BuildCategory::Forge, crate::model::BuildStyle::Modern, 23, 23, 28, "морская буровая нефтяная платформа на опорах с вертолётной площадкой"),
    ("mage_tower", BuildCategory::Tower, crate::model::BuildStyle::Gothic, 15, 15, 30, "башня архимага волшебника с парящими кристаллами и рунами"),
    ("hobbit", BuildCategory::House, crate::model::BuildStyle::Cottage, 17, 17, 11, "уютная нора хоббита в холме шира с круглой дверью и садом"),
    ("mushroom", BuildCategory::House, crate::model::BuildStyle::Cottage, 15, 15, 16, "волшебный грибной дом мухомор со светящимися спорами и крыльцом"),
    ("catacombs", BuildCategory::Temple, crate::model::BuildStyle::Gothic, 19, 21, 14, "мрачный склеп мавзолей с подземными катакомбами и саркофагом"),
    ("dwarven", BuildCategory::Castle, crate::model::BuildStyle::Nordic, 23, 21, 20, "величественный дварфийский горный чертог эребора с троном и колоннами"),
    ("dragon", BuildCategory::Temple, crate::model::BuildStyle::Gothic, 25, 29, 20, "древнее святилище дракона с гигантским костяным скелетом и алтарём"),
    ("stonehenge", BuildCategory::Temple, crate::model::BuildStyle::Nordic, 21, 21, 14, "древний кромлех стоунхендж из мегалитов с порталом в центре"),
    ("longhouse", BuildCategory::House, crate::model::BuildStyle::Nordic, 17, 29, 14, "скандинавский длинный дом зал викингов с резным коньком крыши и очагом"),
    ("chalet", BuildCategory::House, crate::model::BuildStyle::Cottage, 17, 19, 15, "тёплое альпийское горное шале из бруса со снежной крышей и дровницей"),
    ("kasbah", BuildCategory::Castle, crate::model::BuildStyle::Desert, 21, 21, 18, "восточная касба риад с глинобитными башнями бадгир и оазисом во дворе"),
    ("venice", BuildCategory::Settlement, crate::model::BuildStyle::Classical, 48, 48, 18, "венецианский город на воде на сваях с каналами и деревянными мостиками"),
    ("dungeon", BuildCategory::Dungeon, crate::model::BuildStyle::Gothic, 27, 27, 18, "многоуровневое подземное подземелье темница с залами, лестницами, сундуками и наземным входом"),
    ("mine", BuildCategory::Dungeon, crate::model::BuildStyle::Medieval, 29, 29, 18, "глубокая заброшенная шахта штольня с рельсами, деревянными крепями и рудными жилами"),
    ("estate", BuildCategory::Estate, crate::model::BuildStyle::Cottage, 35, 33, 20, "загородная усадьба поместье с хозяйским домом, амбаром, грядками, загоном и садом"),
    ("vineyard_estate", BuildCategory::Estate, crate::model::BuildStyle::Medieval, 31, 29, 18, "средневековая загородная усадьба хутор с хозяйским домом, бочками, садом и огородом"),
    ("canal_lock", BuildCategory::Engineering, crate::model::BuildStyle::Medieval, 23, 25, 16, "судоходный шлюз канал с двойными воротами и будкой смотрителя"),
    ("roundhouse", BuildCategory::Engineering, crate::model::BuildStyle::Steampunk, 25, 25, 16, "железнодорожное веерное депо с поворотным кругом, рельсами и паровозными стойлами"),
    ("drydock", BuildCategory::Engineering, crate::model::BuildStyle::Medieval, 21, 31, 18, "корабельный сухой док с портальным краном и кильблоками"),
    ("substation", BuildCategory::Engineering, crate::model::BuildStyle::Modern, 21, 19, 16, "трансформаторная электроподстанция с громоотводами, катушками и датчиками"),
    ("vineyard", BuildCategory::Agricultural, crate::model::BuildStyle::Cottage, 25, 23, 14, "террасный виноградник со шпалерами и прессовым домом с бочками"),
    ("silo_farm", BuildCategory::Agricultural, crate::model::BuildStyle::Medieval, 23, 21, 20, "фермерский элеватор с высокой зерновой башней силосом и мельницей"),
    ("apiary", BuildCategory::Agricultural, crate::model::BuildStyle::Cottage, 21, 19, 13, "пасека и фруктовый сад с цветущими деревьями и пчелиными ульями"),
    ("stables", BuildCategory::Agricultural, crate::model::BuildStyle::Medieval, 23, 19, 14, "королевская конюшня и пастбище с денниками, водопоем и сеновалом"),
    ("fountain", BuildCategory::DecorArt, crate::model::BuildStyle::Classical, 17, 17, 14, "торжественный городской фонтан на площади со скамьями и светильниками"),
    ("statue", BuildCategory::DecorArt, crate::model::BuildStyle::Classical, 15, 15, 22, "монументальная статуя стража на гранитном постаменте с мечом"),
    ("hedge_maze", BuildCategory::DecorArt, crate::model::BuildStyle::Cottage, 25, 25, 11, "парковый зелёный лабиринт из живой изгороди с беседкой в центре"),
    ("zen_garden", BuildCategory::DecorArt, crate::model::BuildStyle::Japanese, 21, 19, 12, "японский сад камней карэсансуй с бонсай сакуры и мостиком"),
];

pub const CLI_PRESET_DESCS_EN: [&str; 88] = [
    "japanese two-story teahouse with terrace, stall, striped awning and lanterns",
    "medieval two-story tavern with fireplace and basement",
    "multi-tiered japanese pagoda with cherry blossom garden",
    "gothic citadel castle with towers, basement and courtyard well",
    "wooden windmill with sails and barn",
    "tall coastal stone lighthouse with beacon lantern and spiral stairs",
    "glass greenhouse conservatory with greenery and flowers",
    "steampunk blacksmith forge with copper roof and anvil",
    "nordic stone cathedral temple with bell and stained glass",
    "modern concrete villa with flat roof, panoramic windows and courtyard pool",
    "arched stone bridge with lanterns",
    "observatory tower with dome and steampunk telescope",
    "french two-story manor house with mansard roof and dormers",
    "classical roman colosseum amphitheater with arches, columns and arena",
    "medieval village district with tavern, forge and town square",
    "cable-stayed suspension bridge with pylons and cables",
    "two-tier roman aqueduct bridge with water channel",
    "japanese arched taiko moon bridge with torii gates",
    "fortified castle drawbridge with watchtowers",
    "egyptian stepped pyramid ziggurat with tomb and obelisks",
    "classical triumphal arch monument with columns and terrace",
    "big ben clocktower with chimes and bell",
    "cottage watermill by stream with waterwheel and flume",
    "gothic cathedral with flying buttresses, towers and rose window",
    "japanese tenshu castle with stone plinth and tiered pagoda",
    "oriental bazaar market square with colorful tents and fountain",
    "tall treehouse in lush canopy with suspension walkways",
    "japanese hot spring onsen bathhouse with waterfall and pavilion",
    "roman pantheon rotunda with coffered dome and portico",
    "greek parthenon peripteral temple with colonnade and pediment",
    "babylonian hanging gardens with terraced greenery and waterfalls",
    "oriental mosque palace with turquoise dome and minarets",
    "azure babylonian ishtar gate of glazed brick and gold",
    "ancient khmer angkor stupa temple with prang towers in jungle",
    "japanese crimson torii avenue over water itsukushima shrine",
    "great egyptian sandstone sphinx guardian of pyramids",
    "medieval concentric motte and bailey castle with walls and keep",
    "fortified barbican city gatehouse with portcullis and watchtowers",
    "medieval abbey monastery with cloister and courtyard",
    "medieval knight jousting tournament arena with royal stand",
    "military outpost camp behind palisade with watchtower and trebuchet",
    "star bastion vauban fort with diamond redoubts",
    "medieval apothecary alchemy shop with herbal garden",
    "cozy winery and bakery cottage with stone oven and wine cellar",
    "grand classical town hall with portico and bell tower",
    "classical opera house theater with stage, boxes and dome",
    "classical natural history museum with glass atrium and skeleton",
    "classical marble bank vault treasury with columns and safe",
    "gothic magic academy university with great hall and towers",
    "medieval three-masted galleon sailing ship on water with cannons",
    "medieval harbor shipyard with crane, pier and hull on ways",
    "victorian steampunk railway train station with arched shed and locomotive",
    "steampunk airship zeppelin dock with mooring mast and sky vessel",
    "modern concrete hydroelectric dam barrage with spillway and lake",
    "festive red-and-white striped circus big top tent with arena",
    "modern glass curtain skyscraper highrise tower with spire",
    "steampunk industrial factory with saw-tooth roof, smokestacks and crane",
    "modern football stadium arena with grandstands, pitch and floodlights",
    "reinforced concrete underground bunker shelter with blast airlock",
    "modern spaceport launchpad with service gantry tower and tall rocket",
    "offshore steel oil rig drilling platform on stilts with helipad",
    "gothic archmage wizard tower with floating crystals and runes",
    "cozy hobbit hole in shire hillside with round door and garden",
    "enchanted mushroom cottage with glowing spores and porch",
    "gothic mausoleum crypt with subterranean catacombs and sarcophagus",
    "grand nordic dwarven mountain hall of erebor with throne and pillars",
    "ancient dragon sanctuary with giant bone skeleton and altar",
    "ancient megalithic stonehenge stone circle with central portal",
    "nordic viking longhouse mead hall with carved gable and hearth",
    "warm alpine timber chalet with snow roof and woodshed",
    "oriental desert kasbah riad with adobe towers and oasis courtyard",
    "venetian floating village on stilts with canals and timber bridges",
    "multi-level subterranean gothic dungeon chambers with stairs, chests, torches and stone entrance",
    "deep abandoned timber mineshaft dungeon with rails, ore veins and supply crates",
    "rural country manor estate compound with house, barn, irrigated crop fields, livestock paddock and orchard",
    "medieval countryside homestead estate with manor house, orchard well, irrigated fields and paddock",
    "engineering canal navigation lock with stepped basin, double sluice gates and lockkeeper cabin",
    "steampunk railway roundhouse depot with rotating turntable, radial rails and locomotive bays",
    "maritime gantry drydock shipyard with overhead beam crane, scaffold cradles and slipway",
    "modern electrical power substation with transformer coils, copper busbars and lightning rods",
    "cottage hillside vineyard with grape trellises, stone winepress house and barrel cellar",
    "medieval grain farmstead with tall circular masonry grain silos, loading chute and barn",
    "peaceful blossom orchard apiary with fruit trees, raised flower beds and wooden beehives",
    "timber royal horse stables and paddock with stall boxes, hay loft and watering troughs",
    "grand classical tiered stone plaza fountain with spouts, perimeter benches and lanterns",
    "monumental stone guardian statue on tiered plinth holding sword and raised lantern",
    "botanical garden hedge maze park with evergreen topiary corridors and central gazebo",
    "tranquil japanese zen garden karesansui with raked gravel waves, crooked sakura and torii bridge",
];

pub fn cli_preset_desc(idx: usize, lang: UiLang) -> &'static str {
    let i = idx % CLI_PRESETS.len();
    match lang {
        UiLang::En => CLI_PRESET_DESCS_EN[i],
        UiLang::Ru => CLI_PRESETS[i].6,
    }
}

pub fn find_cli_preset(query: &str) -> Option<&'static (&'static str, BuildCategory, crate::model::BuildStyle, u16, u16, u16, &'static str)> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    CLI_PRESETS
        .iter()
        .find(|p| p.0 == q)
        .or_else(|| CLI_PRESETS.iter().find(|p| p.0.starts_with(&q)))
        .or_else(|| {
            CLI_PRESETS.iter().enumerate().find_map(|(idx, p)| {
                if p.6.to_lowercase().contains(&q) || CLI_PRESET_DESCS_EN[idx].contains(&q) {
                    Some(p)
                } else {
                    None
                }
            })
        })
}

fn spawn_command_build(
    host: &mut dyn ServerHost,
    event: &RunModCommandEvent<'_>,
    player_opt: Option<PlayerRef>,
    w: u16,
    d: u16,
    h: u16,
    category: BuildCategory,
    style: crate::model::BuildStyle,
    desc: String,
) -> BuildResponse {
    let (origin, dim, facing) = if let Some(player) = player_opt {
        let pos = player
            .entity()
            .position(host)
            .unwrap_or(event.command.position);
        let rot = player.entity().rotation(host).unwrap_or([0.0, 0.0]);
        let facing = CardinalFacing::from_yaw(rot[0]);
        let yaw_rad = (f64::from(rot[0])).to_radians();
        let fx = -yaw_rad.sin();
        let fz = yaw_rad.cos();
        let dist = (f64::from(w.max(d)) * 0.5) + 4.0;
        let cx = (pos[0] + fx * dist).round() as i32;
        let cz = (pos[2] + fz * dist).round() as i32;
        let iw = i32::from(w.clamp(7, 96));
        let id = i32::from(d.clamp(7, 96));
        let ox = cx - iw / 2;
        let oz = cz - id / 2;
        let dim = player
            .dimension(host)
            .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
        let mut sample_cols = Vec::with_capacity(25);
        for gz in 0..5 {
            let lz = (gz * (id - 1)) / 4;
            for gx in 0..5 {
                let lx = (gx * (iw - 1)) / 4;
                sample_cols.push((ox + lx, oz + lz));
            }
        }
        let mut oy = (pos[1].floor() as i32) - 1;
        if let Ok(heights) = host.surface_heights(&dim, &sample_cols, Heightmap::MotionBlockingNoLeaves) {
            let mut ys: Vec<i32> = heights.into_iter().flatten().map(|sy| sy - 1).collect();
            if !ys.is_empty() {
                ys.sort_unstable();
                oy = ys[(ys.len() * 35) / 100];
            }
        }
        ([ox, oy, oz], dim, facing)
    } else {
        let p = event.command.position;
        (
            [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32],
            "minecraft:overworld".to_owned(),
            CardinalFacing::South,
        )
    };

    let seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
    let salt = ((origin[0] as i64).wrapping_mul(0x9E37_79B9)
        ^ (origin[2] as i64).wrapping_mul(0x517C_C1B7)) as u64;
    let seed = crate::noise::fresh_entropy(seq, salt) as i64;
    let biome_hint = sample_biome_at(
        host,
        &dim,
        origin[0] + i32::from(w / 2),
        origin[1],
        origin[2] + i32::from(d / 2),
    );
    let palette = crate::parser::infer_palette_with_biome(style, &desc, seed as u64, &biome_hint);

    let req = BuildRequest {
        dimension: dim,
        origin_x: origin[0],
        origin_y: origin[1],
        origin_z: origin[2],
        width: w,
        depth: d,
        height: h,
        category,
        style,
        facing,
        noise_level: NoiseLevel::Balanced,
        seed,
        description: desc,
        adapt_terrain: true,
        air_only: false,
        wear_pct: 0,
        overgrowth_pct: 0,
        biome_hint,
        palette,
        living_decor: true,
        spawn_npcs: false,
        blend_radius: 4,
    };
    execute_build(host, player_opt, req)
}

pub fn register_commands(event: &mut RegisterCommandsEvent<'_>) {
    event.commands.push(Command::new("buildgen"));
    event.commands.push(Command::new("buildgen help"));
    event.commands.push(Command::op("buildgen undo", 2));
    event.commands.push(Command::op("buildgen redo", 2));
    event.commands.push(Command::op("buildgen reroll", 2));
    event.commands.push(Command::new("buildgen lang <lang:text>"));
    event.commands.push(Command::op("buildgen preset <preset:text>", 2));
    event.commands.push(Command::op("buildgen generate <description:text>", 2));
    event.commands.push(Command::op(
        "buildgen size <width:integer> <depth:integer> <height:integer> <description:text>",
        2,
    ));
    event.commands.push(Command::op("buildgen connect", 2));
    event.commands.push(Command::op(
        "buildgen road <x1:integer> <z1:integer> <x2:integer> <z2:integer>",
        2,
    ));
    event.commands.push(Command::op(
        "buildgen road <x1:integer> <z1:integer> <x2:integer> <z2:integer> <width:integer>",
        2,
    ));
}

pub fn tab_complete(event: &mut TabCompleteEvent<'_>) {
    if event.name != "buildgen" {
        return;
    }
    let trimmed = event.command.trim_start().trim_start_matches('/');
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let ends_with_space = event.command.ends_with(' ');
    let range = (event.range[0], event.range[1]);

    if parts.len() <= 1 && ends_with_space || (parts.len() == 2 && !ends_with_space) {
        let prefix = if parts.len() == 2 && !ends_with_space {
            parts[1]
        } else {
            ""
        };
        for sub in ["help", "undo", "redo", "reroll", "lang", "preset", "generate", "size", "road", "connect"] {
            if sub.starts_with(prefix) && !event.suggestions.iter().any(|s| s.text == sub) {
                event.suggestions.push(mod_api::Suggestion::new(range, sub));
            }
        }
    } else if parts.get(1).copied() == Some("lang") && (ends_with_space || parts.len() >= 2) {
        let prefix = if parts.len() >= 3 && !ends_with_space {
            parts[2].to_lowercase()
        } else {
            String::new()
        };
        for code in ["en", "ru"] {
            if code.starts_with(&prefix) && !event.suggestions.iter().any(|s| s.text == code) {
                event.suggestions.push(mod_api::Suggestion::new(range, code));
            }
        }
    } else if parts.get(1).copied() == Some("preset") && (ends_with_space || parts.len() >= 2) {
        let prefix = if parts.len() >= 3 && !ends_with_space {
            parts[2].to_lowercase()
        } else {
            String::new()
        };
        for &(slug, _, _, _, _, _, _) in CLI_PRESETS {
            if slug.starts_with(&prefix) && !event.suggestions.iter().any(|s| s.text == slug) {
                event.suggestions.push(mod_api::Suggestion::new(range, slug));
            }
        }
    } else if parts.get(1).copied() == Some("generate") && (ends_with_space || parts.len() >= 2) {
        let samples = match player_lang(None) {
            UiLang::En => &[
                "cozy two-story medieval house with porch and fireplace",
                "japanese pagoda with cherry blossom garden",
                "gothic citadel castle with towers and basement",
                "steampunk blacksmith forge with copper roof and anvil",
                "cable-stayed suspension bridge with pylons",
                "roman pantheon rotunda with dome and portico",
                "glass curtain skyscraper highrise with spire",
                "venetian floating village on stilts with canals",
                "subterranean gothic dungeon catacombs with chests",
                "rural country estate with manor, barn and crop fields",
            ][..],
            UiLang::Ru => &[
                "уютный двухэтажный средневековый дом с крыльцом и камином",
                "японский дом пагода с садом сакуры и террасой",
                "готический замок с высокими башнями и подвалом",
                "кузница в стиле стимпанк с медной крышей и наковальней",
                "вантовый подвесной мост с пилонами и канатами",
                "римский пантеон ротонда с куполом и портиком",
                "современный стеклянный небоскрёб со шпилем",
                "венецианский город на воде на сваях с каналами",
                "многоуровневое подземелье катакомбы с сундуками и порталом",
                "загородная усадьба с особняком, амбаром, грядками и мельницей",
            ][..],
        };
        for sample in samples {
            if !event.suggestions.iter().any(|s| s.text == *sample) {
                event.suggestions.push(mod_api::Suggestion::new(range, *sample));
            }
        }
    }
}

fn require_command_op(
    host: &mut dyn ServerHost,
    event: &RunModCommandEvent<'_>,
    player_opt: Option<PlayerRef>,
    lang: UiLang,
) -> bool {
    let ok = event.source.permission().level() >= 2
        && player_opt.is_none_or(|p| player_has_build_permission(host, p));
    if !ok {
        let _ = host
            .commands()
            .feedback(Text::literal(deny_permission_response(lang).message));
    }
    ok
}

pub fn handle_command(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let lits = &event.command.literals;
    if lits.first().map(String::as_str) != Some("buildgen") {
        return Flow::Continue;
    }

    let sub = lits.get(1).map(String::as_str);
    let player_opt = match event.source {
        CommandSource::Player { player, .. } => Some(*player),
        _ => None,
    };
    let lang = player_lang(player_opt);

    match sub {
        None => {
            if let Some(player) = player_opt {
                if let Some(ch) = channels() {
                    let _ = ch.to_client_reply.send(
                        host,
                        player,
                        &BuildResponse {
                            ok: true,
                            blocks_changed: 0,
                            message: "__OPEN_GUI__".into(),
                        },
                    );
                }
                let _ = player.tell(
                    host,
                    Text::literal(match lang {
                        UiLang::En => "Opening Building Generator menu (hotkey: F8).",
                        UiLang::Ru => "Открываю меню Building Generator (горячая клавиша: F8).",
                    }),
                );
            } else {
                let _ = host.commands().feedback(Text::literal(match lang {
                    UiLang::En => "Usage: /buildgen preset <name> | generate <description> | reroll | undo | redo | lang <en|ru>",
                    UiLang::Ru => "Использование: /buildgen preset <имя> | generate <описание> | reroll | undo | redo | lang <en|ru>",
                }));
            }
            Flow::Cancel
        }
        Some("help") => {
            let _ = host.commands().feedback(Text::literal(match lang {
                UiLang::En => format!(
                    "Building Generator v{}: F8 or /buildgen | /buildgen lang <en|ru> | /buildgen preset <name> | /buildgen generate <desc> | /buildgen size <w> <d> <h> <desc> | /buildgen road <x1> <z1> <x2> <z2> [width] | /buildgen connect | /buildgen reroll | /buildgen undo | /buildgen redo",
                    crate::model::MOD_VERSION
                ),
                UiLang::Ru => format!(
                    "Building Generator v{}: F8 или /buildgen | /buildgen lang <en|ru> | /buildgen preset <имя> | /buildgen generate <описание> | /buildgen size <w> <d> <h> <описание> | /buildgen road <x1> <z1> <x2> <z2> [шир] | /buildgen connect | /buildgen reroll | /buildgen undo | /buildgen redo",
                    crate::model::MOD_VERSION
                ),
            }));
            Flow::Cancel
        }
        Some("lang") => {
            let raw = event
                .command
                .arguments
                .iter()
                .find_map(|arg| match (&arg.name[..], &arg.value) {
                    ("lang", CommandArgumentValue::String(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            if let Some(next_lang) = UiLang::parse(raw) {
                set_player_lang(player_opt, next_lang);
                if let Some(player) = player_opt
                    && let Some(ch) = channels()
                {
                    let _ = ch.to_client_reply.send(
                        host,
                        player,
                        &BuildResponse {
                            ok: true,
                            blocks_changed: 0,
                            message: format!("__LANG__:{}", next_lang.code()),
                        },
                    );
                }
                let _ = host.commands().feedback(Text::literal(match next_lang {
                    UiLang::En => "Building Generator language switched to English.",
                    UiLang::Ru => "Язык Building Generator переключён на русский.",
                }));
            } else {
                let _ = host.commands().feedback(Text::literal(match lang {
                    UiLang::En => "Usage: /buildgen lang <en|ru>",
                    UiLang::Ru => "Использование: /buildgen lang <en|ru>",
                }));
            }
            Flow::Cancel
        }
        Some("undo") => {
            if require_command_op(host, event, player_opt, lang) {
                let dim = player_opt
                    .and_then(|p| p.dimension(host))
                    .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
                let resp = execute_undo(host, player_opt, &dim);
                let _ = host.commands().feedback(Text::literal(resp.message));
            }
            Flow::Cancel
        }
        Some("redo") => {
            if require_command_op(host, event, player_opt, lang) {
                let dim = player_opt
                    .and_then(|p| p.dimension(host))
                    .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
                let resp = execute_redo(host, player_opt, &dim);
                let _ = host.commands().feedback(Text::literal(resp.message));
            }
            Flow::Cancel
        }
        Some("reroll") => {
            if require_command_op(host, event, player_opt, lang) {
                let dim = player_opt
                    .and_then(|p| p.dimension(host))
                    .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
                let resp = execute_reroll(host, player_opt, &dim);
                let _ = host.commands().feedback(Text::literal(resp.message));
            }
            Flow::Cancel
        }
        Some("preset") => {
            if !require_command_op(host, event, player_opt, lang) {
                return Flow::Cancel;
            }
            let preset_name = event
                .command
                .arguments
                .iter()
                .find_map(|arg| match (&arg.name[..], &arg.value) {
                    ("preset", CommandArgumentValue::String(s)) => Some(s.as_str()),
                    _ => None,
                })
                .unwrap_or("");
            if let Some(&(_, cat, style, w, d, h, desc)) = find_cli_preset(preset_name) {
                let resp = spawn_command_build(
                    host,
                    event,
                    player_opt,
                    w,
                    d,
                    h,
                    cat,
                    style,
                    desc.to_owned(),
                );
                let _ = host.commands().feedback(Text::literal(resp.message));
            } else {
                let names: Vec<&str> = CLI_PRESETS.iter().map(|p| p.0).collect();
                let _ = host.commands().feedback(Text::literal(match lang {
                    UiLang::En => format!("Preset not found. Available: {}", names.join(", ")),
                    UiLang::Ru => format!("Пресет не найден. Доступны: {}", names.join(", ")),
                }));
            }
            Flow::Cancel
        }
        Some("generate") | Some("size") => {
            if !require_command_op(host, event, player_opt, lang) {
                return Flow::Cancel;
            }
            let mut w = 13u16;
            let mut d = 11u16;
            let mut h = 12u16;
            let mut desc = String::from(match lang {
                UiLang::En => "cozy house with porch",
                UiLang::Ru => "уютный дом с крыльцом",
            });

            for arg in &event.command.arguments {
                match (arg.name.as_str(), &arg.value) {
                    ("width", CommandArgumentValue::Integer(v)) => {
                        w = (*v).clamp(7, 96) as u16;
                    }
                    ("depth", CommandArgumentValue::Integer(v)) => {
                        d = (*v).clamp(7, 96) as u16;
                    }
                    ("height", CommandArgumentValue::Integer(v)) => {
                        h = (*v).clamp(5, 48) as u16;
                    }
                    ("description", CommandArgumentValue::String(s)) => {
                        desc = s.clone();
                    }
                    _ => {}
                }
            }

            let resp = spawn_command_build(
                host,
                event,
                player_opt,
                w,
                d,
                h,
                BuildCategory::House,
                crate::model::BuildStyle::Auto,
                desc,
            );
            let _ = host.commands().feedback(Text::literal(resp.message));
            Flow::Cancel
        }
        Some("road") => {
            if !require_command_op(host, event, player_opt, lang) {
                return Flow::Cancel;
            }
            let mut x1 = 0i32;
            let mut z1 = 0i32;
            let mut x2 = 0i32;
            let mut z2 = 0i32;
            let mut width = 3u8;
            for arg in &event.command.arguments {
                match (arg.name.as_str(), &arg.value) {
                    ("x1", CommandArgumentValue::Integer(v)) => x1 = *v as i32,
                    ("z1", CommandArgumentValue::Integer(v)) => z1 = *v as i32,
                    ("x2", CommandArgumentValue::Integer(v)) => x2 = *v as i32,
                    ("z2", CommandArgumentValue::Integer(v)) => z2 = *v as i32,
                    ("width", CommandArgumentValue::Integer(v)) => width = (*v).clamp(3, 5) as u8,
                    _ => {}
                }
            }
            let dim = player_opt
                .and_then(|p| p.dimension(host))
                .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
            let base_y = player_opt
                .and_then(|p| p.entity().position(host))
                .map_or(event.command.position[1].floor() as i32, |pos| pos[1].floor() as i32 - 1);
            let seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
            let biome_hint = sample_biome_at(host, &dim, x1, base_y, z1);
            let req = RoadConnectRequest {
                dimension: dim,
                start_x: x1,
                start_y: base_y,
                start_z: z1,
                end_x: x2,
                end_y: base_y,
                end_z: z2,
                road_width: width,
                auto_bridge: true,
                style: crate::model::BuildStyle::Medieval,
                seed: crate::noise::fresh_entropy(seq, (x1 ^ z2) as u64) as i64,
                air_only: false,
                blend_radius: 4,
                living_decor: true,
                spawn_npcs: false,
                biome_hint,
                palette: MaterialPalette::preset(0),
            };
            let resp = execute_road_connect(host, player_opt, req);
            let _ = host.commands().feedback(Text::literal(resp.message));
            Flow::Cancel
        }
        Some("connect") => {
            if !require_command_op(host, event, player_opt, lang) {
                return Flow::Cancel;
            }
            let Some(player) = player_opt else {
                let _ = host.commands().feedback(Text::literal(tr(
                    lang,
                    "/buildgen connect must be run by a player.",
                    "/buildgen connect должен запускаться игроком.",
                )));
                return Flow::Cancel;
            };
            let dim = player
                .dimension(host)
                .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
            let pos = player.entity().position(host).unwrap_or(event.command.position);
            let cur = [pos[0].floor() as i32, pos[1].floor() as i32 - 1, pos[2].floor() as i32];
            let uid = player.uuid();
            let prev = {
                let mut guard = ROAD_CONNECT_ANCHORS.lock().unwrap_or_else(|e| e.into_inner());
                let map = guard.get_or_insert_with(HashMap::new);
                if let Some((pdim, ppos)) = map.get(&uid).cloned()
                    && pdim == dim
                {
                    map.remove(&uid);
                    Some(ppos)
                } else {
                    map.insert(uid, (dim.clone(), cur));
                    None
                }
            };
            if let Some(start) = prev {
                let seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
                let biome_hint = sample_biome_at(host, &dim, start[0], start[1], start[2]);
                let req = RoadConnectRequest {
                    dimension: dim,
                    start_x: start[0],
                    start_y: start[1],
                    start_z: start[2],
                    end_x: cur[0],
                    end_y: cur[1],
                    end_z: cur[2],
                    road_width: 3,
                    auto_bridge: true,
                    style: crate::model::BuildStyle::Medieval,
                    seed: crate::noise::fresh_entropy(seq, (start[0] ^ cur[2]) as u64) as i64,
                    air_only: false,
                    blend_radius: 4,
                    living_decor: true,
                    spawn_npcs: false,
                    biome_hint,
                    palette: MaterialPalette::preset(0),
                };
                let resp = execute_road_connect(host, player_opt, req);
                let _ = host.commands().feedback(Text::literal(resp.message));
            } else {
                let _ = host.commands().feedback(Text::literal(match lang {
                    UiLang::En => format!("Road start pinned at ({}, {}, {}). Walk to end and run /buildgen connect again.", cur[0], cur[1], cur[2]),
                    UiLang::Ru => format!("Начало дороги зафиксировано ({}, {}, {}). Дойдите до второй точки и снова введите /buildgen connect.", cur[0], cur[1], cur[2]),
                }));
            }
            Flow::Cancel
        }
        _ => Flow::Continue,
    }
}
