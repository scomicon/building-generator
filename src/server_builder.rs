use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static CMD_BUILD_SEQ: AtomicU64 = AtomicU64::new(1);

use mod_api::{
    BlockEdit, BlockKey, BlockRun, BlockState, BossBarColor, BossBarRef, Command,
    CommandArgumentValue, CommandSource, Flow, Heightmap, HostBlockStates, IPayloadContext,
    PlayerRef, RegisterCommandsEvent, RunModCommandEvent, ScheduledTaskEvent, ServerHost,
    StateConstraint, TabCompleteEvent, Tag, Text,
};

use crate::generator::{GeneratedBlueprint, PlannedBlock};
use crate::model::{
    BuildCategory, BuildRequest, BuildResponse, CardinalFacing, MaterialPalette, NoiseLevel,
    RedoRequest, RerollRequest, SetLangRequest, UiLang, UndoRequest,
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
}

static UNDO_STACK: Mutex<Option<HashMap<u128, Vec<UndoEntry>>>> = Mutex::new(None);
static REDO_STACK: Mutex<Option<HashMap<u128, Vec<UndoEntry>>>> = Mutex::new(None);
static LAST_REQUESTS: Mutex<Option<HashMap<u128, BuildRequest>>> = Mutex::new(None);
static PENDING_JOBS: Mutex<Option<VecDeque<PendingBatchJob>>> = Mutex::new(None);
static PLAYER_LANG: Mutex<Option<HashMap<u128, UiLang>>> = Mutex::new(None);
static LAST_BUILD_AT: Mutex<Option<HashMap<u128, Instant>>> = Mutex::new(None);

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
        || id.ends_with("_shelf")
        || id.starts_with("minecraft:potted_")
        || matches!(
            id,
            "minecraft:flower_pot"
                | "minecraft:cake"
                | "minecraft:sea_pickle"
                | "minecraft:bell"
                | "minecraft:ladder"
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
            Err(_) => BlockState::of(&states, block_key, &[])
                .map_err(|e| format!("Fallback error for {}: {e:?}", planned.block_id)),
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
        retain_complete_doors(&mut bp.blocks);
    }

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

    let owner_key = player.map_or(0, PlayerRef::uuid);
    let undo_snapshot =
        prepare_undo_and_filter_blueprint(host, &req.dimension, &mut bp, req.air_only);
    let post_containers = collect_post_build_containers(&bp, req.seed as u64, lang);

    if req.air_only && bp.blocks.is_empty() {
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

    let (palette, phase1_runs, phase2_runs) = match blueprint_to_phased_runs(host, &bp) {
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
    with_last_requests(|map| {
        map.insert(owner_key, req.clone());
    });

    if total_runs <= MAX_RUNS_PER_BATCH {
        let mut changed = 0u64;
        if !phase1_runs.is_empty() {
            let edit1 = BlockEdit {
                palette: palette.clone(),
                filters: Vec::new(),
                runs: phase1_runs,
            };
            match host.edit_blocks(&req.dimension, &edit1) {
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
            match host.edit_blocks(&req.dimension, &edit2) {
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
        apply_post_build_containers(host, &req.dimension, &post_containers);
        let undo_count = undo_depth(owner_key);
        BuildResponse {
            ok: true,
            blocks_changed: changed.min(i32::MAX as u64) as i32,
            message: match lang {
                UiLang::En => format!(
                    "Ready: {} ({}) {}x{}x{} (seed {}, blocks changed: {}) [undo: {}/{} | redo: 0/{}].",
                    bp.spec.category.label(UiLang::En),
                    bp.spec.style.label(UiLang::En),
                    bp.width,
                    bp.depth,
                    bp.height,
                    req.seed,
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
                    req.seed,
                    changed,
                    undo_count,
                    MAX_UNDO_HISTORY_PER_PLAYER,
                    MAX_UNDO_HISTORY_PER_PLAYER
                ),
            },
        }
    } else {
        let undo_count = undo_depth(owner_key);
        let mut chunks: VecDeque<Vec<BlockRun>> = phase1_runs
            .chunks(MAX_RUNS_PER_BATCH)
            .map(|c| c.to_vec())
            .collect();
        for c in phase2_runs.chunks(MAX_RUNS_PER_BATCH) {
            chunks.push_back(c.to_vec());
        }
        let first = chunks.pop_front().unwrap_or_default();
        let first_count = first.len();
        let edit = BlockEdit {
            palette: palette.clone(),
            filters: Vec::new(),
            runs: first,
        };
        match host.edit_blocks(&req.dimension, &edit) {
            Ok(first_changed) => {
                if !chunks.is_empty() {
                    let job_seq = CMD_BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
                    let label = format!(
                        "{} {}x{}",
                        bp.spec.category.label(lang),
                        bp.width,
                        bp.depth
                    );
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
                            dimension: req.dimension.clone(),
                            palette,
                            chunks,
                            completed_runs: first_count,
                            total_runs,
                            player_uuid: player.map(PlayerRef::uuid),
                            boss_bar,
                            label,
                            lang,
                            post_containers,
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
                            total_runs, req.seed, undo_count, MAX_UNDO_HISTORY_PER_PLAYER, MAX_UNDO_HISTORY_PER_PLAYER
                        ),
                        UiLang::Ru => format!(
                            "Крупная постройка ({} блоков/сегментов) строится по тикам (сид {}) [отмен: {}/{} | возврат: 0/{}].",
                            total_runs, req.seed, undo_count, MAX_UNDO_HISTORY_PER_PLAYER, MAX_UNDO_HISTORY_PER_PLAYER
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

    with_pending_jobs(|q| {
        while let Some(job) = q.front_mut() {
            if !host.server_has_time() {
                need_reschedule = true;
                break;
            }
            let Some(chunk) = job.chunks.pop_front() else {
                let completed = q.pop_front().unwrap();
                if !completed.post_containers.is_empty() {
                    finished_containers.push((completed.dimension, completed.post_containers));
                }
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
                    finished_containers.push((completed.dimension, completed.post_containers));
                }
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

    if let Some(inv) = inverse {
        if is_redo {
            with_undo_map(|map| push_history_entry(map, owner_key, inv));
        } else {
            with_redo_map(|map| push_history_entry(map, owner_key, inv));
        }
    }

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

pub fn handle_lang_payload(req: SetLangRequest, cx: &mut dyn IPayloadContext) -> Result<(), String> {
    let player = cx.player().map_err(|e| format!("{e:?}"))?;
    set_player_lang(Some(player), req.lang);
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
];

pub const CLI_PRESET_DESCS_EN: [&str; 72] = [
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
        let ox = cx - i32::from(w / 2);
        let oz = cz - i32::from(d / 2);
        let dim = player
            .dimension(host)
            .map_or_else(|| "minecraft:overworld".to_owned(), |id| id.to_string());
        let mut oy = (pos[1].floor() as i32) - 1;
        if let Ok(heights) = host.surface_heights(&dim, &[(cx, cz)], Heightmap::MotionBlockingNoLeaves)
            && let Some(Some(sy)) = heights.first()
        {
            oy = sy - 1;
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
        for sub in ["help", "undo", "redo", "reroll", "lang", "preset", "generate", "size"] {
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
                    "Building Generator v{}: F8 or /buildgen | /buildgen lang <en|ru> | /buildgen preset <name> | /buildgen generate <desc> | /buildgen size <w> <d> <h> <desc> | /buildgen reroll | /buildgen undo | /buildgen redo",
                    crate::model::MOD_VERSION
                ),
                UiLang::Ru => format!(
                    "Building Generator v{}: F8 или /buildgen | /buildgen lang <en|ru> | /buildgen preset <имя> | /buildgen generate <описание> | /buildgen size <w> <d> <h> <описание> | /buildgen reroll | /buildgen undo | /buildgen redo",
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
        _ => Flow::Continue,
    }
}
