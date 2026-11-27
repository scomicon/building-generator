use std::collections::BTreeMap;

use crate::gen_common::{
    BannerStyle, GeneratedBlueprint, LecternBookKind, LivingEntityKind, LivingPlan, LocalGrid,
    PlannedBanner, PlannedBlock, PlannedEntity, PlannedLectern, PlannedSign, SignPurpose, clamp_i32,
    get_uv, make_leaves, make_slab, make_stair, set_uv,
};
use crate::gen_landmarks::{
    anchor_plinth, build_gable_roof, finish_landmark_with_living, make_light, place_door_stack,
};
use crate::model::{BuildCategory, BuildRequest, CardinalFacing, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, StructureTheme};

/// Dispatches the 12 Engineering, Agricultural, and Decor & Art themes (plus deterministic category fallback).
pub(crate) fn try_generate_new_category(
    req: &BuildRequest,
    spec: &BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> Option<GeneratedBlueprint> {
    let resolved_theme = resolve_theme(req.seed as u64, spec.category, spec.theme)?;
    let mut resolved_spec = spec.clone();
    resolved_spec.theme = resolved_theme;

    let bp = match resolved_theme {
        StructureTheme::CanalLock => {
            generate_canal_lock(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::RailwayRoundhouse => {
            generate_railway_roundhouse(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::GantryDrydock => {
            generate_gantry_drydock(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::PowerSubstation => {
            generate_power_substation(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::VineyardPress => {
            generate_vineyard_press(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::GrainSiloFarm => {
            generate_grain_silo_farm(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::OrchardApiary => {
            generate_orchard_apiary(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::HorseStables => {
            generate_horse_stables(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::PlazaFountain => {
            generate_plaza_fountain(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::GuardianStatue => {
            generate_guardian_statue(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::HedgeMazePark => {
            generate_hedge_maze_park(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        StructureTheme::ZenKaresansui => {
            generate_zen_karesansui(req, resolved_spec, grid, height, noise, terrain_offsets)
        }
        _ => return None,
    };
    Some(bp)
}

fn resolve_theme(
    seed: u64,
    category: BuildCategory,
    theme: StructureTheme,
) -> Option<StructureTheme> {
    match theme {
        StructureTheme::CanalLock
        | StructureTheme::RailwayRoundhouse
        | StructureTheme::GantryDrydock
        | StructureTheme::PowerSubstation
        | StructureTheme::VineyardPress
        | StructureTheme::GrainSiloFarm
        | StructureTheme::OrchardApiary
        | StructureTheme::HorseStables
        | StructureTheme::PlazaFountain
        | StructureTheme::GuardianStatue
        | StructureTheme::HedgeMazePark
        | StructureTheme::ZenKaresansui => Some(theme),
        StructureTheme::Standard => {
            let mut rng = SplitMix64::new(seed ^ 0xCA7E_9012_3456_789B);
            let idx = rng.range_i32(0, 3) as usize;
            match category {
                BuildCategory::Engineering => Some(
                    [
                        StructureTheme::CanalLock,
                        StructureTheme::RailwayRoundhouse,
                        StructureTheme::GantryDrydock,
                        StructureTheme::PowerSubstation,
                    ][idx],
                ),
                BuildCategory::Agricultural => Some(
                    [
                        StructureTheme::VineyardPress,
                        StructureTheme::GrainSiloFarm,
                        StructureTheme::OrchardApiary,
                        StructureTheme::HorseStables,
                    ][idx],
                ),
                BuildCategory::DecorArt => Some(
                    [
                        StructureTheme::PlazaFountain,
                        StructureTheme::GuardianStatue,
                        StructureTheme::HedgeMazePark,
                        StructureTheme::ZenKaresansui,
                    ][idx],
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

#[inline]
fn local_pos(grid: LocalGrid, u: i32, y: i32, v: i32) -> [i32; 3] {
    let [lx, lz] = grid.to_local_xz(u, v);
    [lx, y, lz]
}

fn clear_air_box(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u0: i32,
    u1: i32,
    v0: i32,
    v1: i32,
    y0: i32,
    y1: i32,
) {
    for y in y0..=y1 {
        for v in v0..=v1 {
            for u in u0..=u1 {
                set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
        }
    }
}

fn rail_along_u(grid: LocalGrid) -> PlannedBlock {
    let shape = if grid.axis_u() == "x" {
        "east_west"
    } else {
        "north_south"
    };
    PlannedBlock::with_props("minecraft:rail", vec![("shape", shape)])
}

fn rail_along_v(grid: LocalGrid) -> PlannedBlock {
    let shape = if grid.axis_v() == "x" {
        "east_west"
    } else {
        "north_south"
    };
    PlannedBlock::with_props("minecraft:rail", vec![("shape", shape)])
}

fn water_source() -> PlannedBlock {
    PlannedBlock::with_props("minecraft:water", vec![("level", "0")])
}

fn place_daylight_lamp(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u: i32,
    base_y: i32,
    v: i32,
    post_h: i32,
    post_id: &str,
    max_h: i32,
) {
    if !grid.in_bounds(u, v) || base_y + post_h + 1 >= max_h {
        return;
    }
    for dy in 0..post_h {
        set_uv(blocks, grid, u, base_y + dy, v, PlannedBlock::simple(post_id));
    }
    set_uv(
        blocks,
        grid,
        u,
        base_y + post_h,
        v,
        PlannedBlock::simple("minecraft:redstone_lamp"),
    );
    set_uv(
        blocks,
        grid,
        u,
        base_y + post_h + 1,
        v,
        PlannedBlock::with_props("minecraft:daylight_detector", vec![("inverted", "true")]),
    );
}

fn place_stone_lantern_post(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u: i32,
    base_y: i32,
    v: i32,
    wall_id: &str,
    lantern_id: &str,
    slab_id: &str,
    max_h: i32,
) {
    if !grid.in_bounds(u, v) || base_y + 2 >= max_h {
        return;
    }
    set_uv(blocks, grid, u, base_y, v, PlannedBlock::simple(wall_id));
    set_uv(blocks, grid, u, base_y + 1, v, make_light(lantern_id.to_owned(), false));
    if base_y + 2 < max_h && wall_id == "minecraft:tuff_brick_wall" {
        // Only add cap slab if resting on full block/wall; lanterns support slabs above via solid top?
        // Actually in MC lantern isn't full solid in prune, so cap sits neatly on tall post or skip if unsupported.
        let _ = slab_id;
    }
}

fn place_tree(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    cu: i32,
    base_y: i32,
    cv: i32,
    trunk_id: &str,
    leaf_id: &str,
    trunk_h: i32,
    canopy_r: i32,
    lean_du: i32,
    max_h: i32,
) {
    if !grid.in_bounds(cu, cv) || base_y + 2 >= max_h {
        return;
    }
    let th = trunk_h.clamp(2, (max_h - base_y - 2).max(2));
    let mut top_u = cu;
    for dy in 0..th {
        let u = if lean_du != 0 && dy >= th / 2 {
            (cu + lean_du).clamp(1, grid.w - 2)
        } else {
            cu
        };
        if dy == th / 2 && u != cu {
            set_uv(
                blocks,
                grid,
                cu,
                base_y + dy,
                cv,
                PlannedBlock::simple(trunk_id),
            );
        }
        set_uv(blocks, grid, u, base_y + dy, cv, PlannedBlock::simple(trunk_id));
        top_u = u;
    }
    let top_y = base_y + th - 1;
    let c_top = (top_y + 2).min(max_h - 1);
    for y in (top_y - 1).max(base_y + 1)..=c_top {
        let r = if y == c_top {
            (canopy_r - 1).max(1)
        } else {
            canopy_r
        };
        for dv in -r..=r {
            for du in -r..=r {
                let u = top_u + du;
                let v = cv + dv;
                if !grid.in_bounds(u, v) {
                    continue;
                }
                if du * du + dv * dv <= r * r + 1 {
                    let occupied_trunk = get_uv(blocks, grid, u, y, v)
                        .is_some_and(|b| b.block_id.ends_with("_log"));
                    if !occupied_trunk {
                        set_uv(blocks, grid, u, y, v, make_leaves(leaf_id));
                    }
                }
            }
        }
    }
}

fn push_sign(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    living: &mut LivingPlan,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
    purpose: SignPurpose,
) {
    if !grid.in_bounds(u, v) || y <= 0 {
        return;
    }
    set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
    living.signs.push(PlannedSign {
        pos: local_pos(grid, u, y, v),
        purpose,
        custom_en: None,
        custom_ru: None,
    });
}

fn push_lectern(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    living: &mut LivingPlan,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
    facing: &'static str,
    book: LecternBookKind,
) {
    if !grid.in_bounds(u, v) {
        return;
    }
    set_uv(
        blocks,
        grid,
        u,
        y,
        v,
        PlannedBlock::with_props(
            "minecraft:lectern",
            vec![("facing", facing), ("has_book", "true")],
        ),
    );
    living.lecterns.push(PlannedLectern {
        pos: local_pos(grid, u, y, v),
        book,
    });
}

fn push_banner(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    living: &mut LivingPlan,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
    style: BannerStyle,
    primary_color: &'static str,
    accent_color: &'static str,
) {
    if !grid.in_bounds(u, v) || y <= 0 {
        return;
    }
    set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
    living.banners.push(PlannedBanner {
        pos: local_pos(grid, u, y, v),
        style,
        primary_color,
        accent_color,
    });
}

fn push_entity(
    living: &mut LivingPlan,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
    kind: LivingEntityKind,
    facing: CardinalFacing,
    detail: &str,
    sub_detail: &str,
    is_decor: bool,
) {
    if !grid.in_bounds(u, v) {
        return;
    }
    living.entities.push(PlannedEntity {
        pos: local_pos(grid, u, y, v),
        kind,
        facing,
        detail: detail.to_owned(),
        sub_detail: sub_detail.to_owned(),
        is_decor,
    });
}

/// Builds a compact solid-walled masonry/timber booth with gable/sloped roof and complete `_door`.
#[allow(clippy::too_many_arguments)]
fn build_service_booth(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    pal: &MaterialPalette,
    seed: u64,
    u0: i32,
    u1: i32,
    v0: i32,
    v1: i32,
    floor_y: i32,
    wall_top: i32,
    max_h: i32,
    wall_id: &str,
    trim_id: &str,
    roof_mat: &str,
) -> Option<[i32; 3]> {
    if u1 - u0 < 2 || v1 - v0 < 2 || wall_top < floor_y + 3 || wall_top >= max_h {
        return None;
    }
    let door_u = (u0 + u1) / 2;
    for v in v0..=v1 {
        for u in u0..=u1 {
            set_uv(
                blocks,
                grid,
                u,
                floor_y,
                v,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, floor_y, v)),
            );
        }
    }
    for y in (floor_y + 1)..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if !shell {
                    set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    continue;
                }
                let corner = (u == u0 || u == u1) && (v == v0 || v == v1);
                let near_door = v == v1 && (u - door_u).abs() <= 1;
                let is_win = !corner && !near_door && y == floor_y + 2 && u1 - u0 >= 4;
                let blk = if corner {
                    PlannedBlock::simple(trim_id)
                } else if is_win {
                    PlannedBlock::simple("minecraft:glass_pane")
                } else {
                    PlannedBlock::simple(wall_id)
                };
                set_uv(blocks, grid, u, y, v, blk);
            }
        }
    }
    let roof_y = (wall_top + 1).min(max_h - 1);
    for v in (v0 - 1).max(0)..=(v1 + 1).min(grid.d - 1) {
        for u in (u0 - 1).max(0)..=(u1 + 1).min(grid.w - 1) {
            let center = u >= u0 && u <= u1 && v >= v0 && v <= v1 && roof_y + 1 < max_h;
            let blk = if center {
                PlannedBlock::simple(roof_mat)
            } else {
                make_slab(roof_mat, "bottom")
            };
            set_uv(blocks, grid, u, roof_y, v, blk);
        }
    }
    if roof_y + 1 < max_h && u1 - u0 >= 3 {
        for v in v0..=v1 {
            for u in (u0 + 1)..=(u1 - 1) {
                set_uv(blocks, grid, u, roof_y + 1, v, make_slab(roof_mat, "bottom"));
            }
        }
    }
    if floor_y + 3 <= wall_top {
        set_uv(
            blocks,
            grid,
            door_u,
            floor_y + 3,
            v1,
            PlannedBlock::simple(trim_id),
        );
    }
    Some(place_door_stack(blocks, grid, pal, door_u, floor_y + 1, v1))
}

// ============================================================================
// 1. ENGINEERING & TRANSPORT
// ============================================================================

/// 1. Canal Lock: stepped two-level flooded canal basin, double timber/iron sluice gates,
/// balance beams, brick lock-keeper booth, daylight streetlamps & nautical signage.
fn generate_canal_lock(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 2, terrain_offsets);

    let lock_hw = if grid.w < 11 {
        1
    } else {
        clamp_i32((grid.w - 6) / 6, 2, 3)
    };
    let canal_cu = clamp_i32(grid.w / 2 - 2, u0 + lock_hw + 1, u1 - lock_hw - 4);
    let ch_u0 = (canal_cu - lock_hw).max(u0 + 1);
    let ch_u1 = (canal_cu + lock_hw).min(u1 - 2);
    let mid_v = clamp_i32((v0 + v1) / 2, v0 + 3, v1 - 3);
    let low_gate_v = (v1 - 1).max(mid_v + 2);

    // Canal floor bed (y=0 solid masonry) & watertight two-level chambers.
    for v in v0..=v1 {
        for u in ch_u0..=ch_u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                0,
                v,
                PlannedBlock::simple("minecraft:mossy_stone_bricks"),
            );
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:air"));
            set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:air"));
        }
    }

    // Rear weir headwall (v0) & front submerged sill (v1) to contain water cleanly.
    for u in ch_u0..=ch_u1 {
        set_uv(
            &mut blocks,
            grid,
            u,
            1,
            v0,
            PlannedBlock::simple("minecraft:stone_bricks"),
        );
        set_uv(
            &mut blocks,
            grid,
            u,
            2,
            v0,
            PlannedBlock::simple("minecraft:chiseled_stone_bricks"),
        );
        set_uv(
            &mut blocks,
            grid,
            u,
            1,
            v1,
            PlannedBlock::simple("minecraft:stone_bricks"),
        );
    }

    // Upper chamber water (y=1..=2) and lower chamber water (y=1).
    for v in (v0 + 1)..mid_v {
        for u in ch_u0..=ch_u1 {
            set_uv(&mut blocks, grid, u, 1, v, water_source());
            set_uv(&mut blocks, grid, u, 2, v, water_source());
        }
    }
    for v in (mid_v + 1)..low_gate_v {
        for u in ch_u0..=ch_u1 {
            set_uv(&mut blocks, grid, u, 1, v, water_source());
        }
    }

    // Lock chamber side retaining walls + ladders + bollards.
    for v in v0..=v1 {
        for side_u in [ch_u0 - 1, ch_u1 + 1] {
            if !grid.in_bounds(side_u, v) {
                continue;
            }
            for y in 0..=2 {
                let id = if y == 2 {
                    "minecraft:smooth_stone"
                } else if (v + y) % 3 == 0 {
                    "minecraft:mossy_stone_bricks"
                } else {
                    "minecraft:stone_bricks"
                };
                set_uv(&mut blocks, grid, side_u, y, v, PlannedBlock::simple(id));
            }
            if v % 3 == 0 && h >= 6 {
                set_uv(
                    &mut blocks,
                    grid,
                    side_u,
                    3,
                    v,
                    PlannedBlock::simple("minecraft:polished_basalt"),
                );
            }
        }
    }
    // Ladder inside lower chamber wall for lock maintenance.
    let lad_u = ch_u0;
    let lad_v = clamp_i32(mid_v + 2, v0 + 1, low_gate_v - 1);
    if grid.in_bounds(lad_u - 1, lad_v) {
        set_uv(
            &mut blocks,
            grid,
            lad_u,
            2,
            lad_v,
            PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_right())]),
        );
    }

    // Double Wooden & Iron Sluice Gates + Overhanging Balance Beams + Footbridge.
    for (gate_v, top_y) in [(mid_v, 3), (low_gate_v, 3)] {
        for u in ch_u0..=ch_u1 {
            let is_center = u == canal_cu;
            for y in 1..=top_y {
                let blk = if y == top_y {
                    PlannedBlock::simple("minecraft:spruce_planks")
                } else if is_center && y == 2 {
                    PlannedBlock::simple("minecraft:iron_bars")
                } else {
                    PlannedBlock::simple("minecraft:dark_oak_planks")
                };
                set_uv(&mut blocks, grid, u, y, gate_v, blk);
            }
        }
        // Timber walk-deck & railing across the gate top.
        for u in (ch_u0 - 1)..=(ch_u1 + 1) {
            set_uv(
                &mut blocks,
                grid,
                u,
                4,
                gate_v,
                PlannedBlock::simple("minecraft:oak_fence"),
            );
        }
        // Balance beams projecting out onto quays with iron chain drop rods.
        for beam_side in [-1, 1] {
            let post_u = if beam_side < 0 { ch_u0 - 1 } else { ch_u1 + 1 };
            let tail_u = (post_u + beam_side * 2).clamp(u0, u1);
            let min_bu = post_u.min(tail_u);
            let max_bu = post_u.max(tail_u);
            if h >= 7 {
                for bu in min_bu..=max_bu {
                    set_uv(
                        &mut blocks,
                        grid,
                        bu,
                        4,
                        gate_v,
                        PlannedBlock::with_props(
                            "minecraft:oak_log",
                            vec![("axis", grid.axis_u())],
                        ),
                    );
                }
                set_uv(
                    &mut blocks,
                    grid,
                    tail_u,
                    3,
                    gate_v,
                    PlannedBlock::simple("minecraft:oak_fence"),
                );
            }
        }
    }

    // Front quay ground stairs so quays are walkable from terrain y=0.
    for u in [u0 + 1, (ch_u1 + 2).min(u1)] {
        if grid.in_bounds(u, v1) && u != canal_cu {
            set_uv(
                &mut blocks,
                grid,
                u,
                2,
                v1,
                make_stair("minecraft:stone_brick_stairs", grid.dir_back(), "bottom"),
            );
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v1,
                PlannedBlock::simple("minecraft:stone_bricks"),
            );
        }
    }

    // Lock-keeper brick booth on right quay.
    let b_u0 = (ch_u1 + 2).min(u1 - 2);
    let b_u1 = u1;
    let b_v0 = (v0 + 1).min(mid_v - 1);
    let b_v1 = (b_v0 + 3).min(v1 - 2);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        b_u0,
        b_u1,
        b_v0,
        b_v1,
        2,
        5.min(h - 3),
        h,
        "minecraft:bricks",
        "minecraft:stone_bricks",
        "minecraft:spruce_planks",
    );
    if b_u1 - b_u0 >= 2 && b_v1 - b_v0 >= 2 {
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            b_u0 + 1,
            3,
            b_v0 + 1,
            grid.dir_front(),
            LecternBookKind::EngineeringLog,
        );
    }

    // Daylight streetlamps, harbor sign, nautical banner & lock-keeper NPC.
    place_daylight_lamp(
        &mut blocks,
        grid,
        u0,
        3,
        mid_v - 1,
        2,
        "minecraft:stone_brick_wall",
        h,
    );
    place_daylight_lamp(
        &mut blocks,
        grid,
        (ch_u1 + 2).min(u1),
        3,
        (v1 - 2).max(mid_v + 1),
        2,
        "minecraft:stone_brick_wall",
        h,
    );
    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (ch_u0 - 1).max(u0),
        3,
        low_gate_v,
        SignPurpose::Harbor,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (b_u0 - 1).max(ch_u1 + 1),
        3,
        b_v1,
        BannerStyle::HarborNautical,
        "blue",
        "white",
    );
    push_entity(
        &mut living,
        grid,
        (ch_u1 + 2).min(u1),
        3,
        mid_v,
        LivingEntityKind::Villager,
        grid.uv_to_facing(-1, 0),
        "fisherman",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 2. Railway Roundhouse: recessed circular turntable pit with rail bridge, brick arched engine
/// bays with campfire smoke vents, coal hopper tower, and tool booth.
fn generate_railway_roundhouse(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Yard surface: gravel ballast & stone-brick border.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            let mat = if rim {
                "minecraft:stone_bricks"
            } else {
                "minecraft:gravel"
            };
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(mat));
        }
    }

    // Partition rear into arched engine bays and front into recessed turntable pit.
    let bay_v1 = clamp_i32(v0 + (grid.d * 34) / 100, v0 + 2, v1 - 5);
    let tr = clamp_i32((v1 - bay_v1 - 2) / 2, 1, 4);
    let tv = clamp_i32(bay_v1 + tr + 1, bay_v1 + 2, v1 - tr - 1);
    let tu = cu;

    // 1. Brick Arched Engine Bays at rear (v0..=bay_v1).
    let bay_top = clamp_i32(h - 4, 6, 9);
    let shed_u0 = u0;
    let shed_u1 = (u1 - 3).max(u0 + 4);
    for y in 2..=bay_top {
        for v in v0..=bay_v1 {
            for u in shed_u0..=shed_u1 {
                let back_or_side = v == v0 || u == shed_u0 || u == shed_u1;
                let front_face = v == bay_v1;
                if back_or_side {
                    let is_pier = (u - shed_u0) % 4 == 0;
                    let is_win = v == v0 && !is_pier && y == 4;
                    let blk = if is_pier {
                        PlannedBlock::simple("minecraft:deepslate_bricks")
                    } else if is_win {
                        PlannedBlock::simple("minecraft:iron_bars")
                    } else {
                        PlannedBlock::simple("minecraft:bricks")
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else if front_face {
                    let bay_rel = (u - shed_u0) % 4;
                    let in_arch = bay_rel != 0 && (y <= 4 || (bay_rel == 2 && y == 5));
                    if !in_arch {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(if bay_rel == 0 {
                                "minecraft:deepslate_bricks"
                            } else {
                                "minecraft:bricks"
                            }),
                        );
                    }
                }
            }
        }
    }
    build_gable_roof(
        &mut blocks,
        grid,
        pal,
        seed,
        shed_u0,
        shed_u1,
        v0,
        bay_v1,
        bay_top,
        h,
        false,
    );

    // Smoke vents with lit campfire chimneys above each bay!
    for u in ((shed_u0 + 2)..shed_u1).step_by(4) {
        let vent_v = (v0 + 1).min(bay_v1 - 1);
        let top_y = (bay_top + 2).min(h - 2);
        for y in bay_top..=top_y {
            set_uv(
                &mut blocks,
                grid,
                u,
                y,
                vent_v,
                PlannedBlock::simple("minecraft:bricks"),
            );
        }
        if top_y + 1 < h {
            set_uv(
                &mut blocks,
                grid,
                u,
                top_y + 1,
                vent_v,
                PlannedBlock::with_props(
                    "minecraft:campfire",
                    vec![("lit", "true"), ("signal_fire", "true")],
                ),
            );
        }
        // Track running into engine bay.
        for tv_bay in (v0 + 1)..=bay_v1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                tv_bay,
                PlannedBlock::simple("minecraft:smooth_stone"),
            );
            set_uv(&mut blocks, grid, u, 2, tv_bay, rail_along_v(grid));
        }
    }
    // Mini locomotive boiler parked in center bay.
    if bay_v1 - v0 >= 2 && shed_u1 - shed_u0 >= 4 {
        let loc_u = (shed_u0 + shed_u1) / 2;
        set_uv(
            &mut blocks,
            grid,
            loc_u,
            3,
            v0 + 1,
            PlannedBlock::simple("minecraft:black_concrete"),
        );
        set_uv(
            &mut blocks,
            grid,
            loc_u,
            3,
            v0 + 2,
            PlannedBlock::simple("minecraft:blast_furnace"),
        );
    }

    // 2. Recessed Circular Turntable Pit & Rotating Rail Bridge.
    for dv in -(tr + 1)..=(tr + 1) {
        for du in -(tr + 1)..=(tr + 1) {
            let u = tu + du;
            let v = tv + dv;
            if !grid.in_bounds(u, v) {
                continue;
            }
            let d2 = du * du + dv * dv;
            if d2 <= tr * tr {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    0,
                    v,
                    PlannedBlock::simple("minecraft:cobbled_deepslate"),
                );
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:air"));
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:air"));
            } else if d2 <= (tr + 1) * (tr + 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple("minecraft:stone_bricks"),
                );
            }
        }
    }
    // Turntable rotating deck & rail across center + outbound lead track.
    for dv in -tr..=tr {
        let v = tv + dv;
        set_uv(
            &mut blocks,
            grid,
            tu,
            1,
            v,
            PlannedBlock::simple("minecraft:spruce_planks"),
        );
        set_uv(&mut blocks, grid, tu, 2, v, rail_along_v(grid));
    }
    for v in (bay_v1 + 1)..=(tv - tr - 1) {
        set_uv(&mut blocks, grid, tu, 2, v, rail_along_v(grid));
    }
    for v in (tv + tr + 1)..=v1 {
        set_uv(&mut blocks, grid, tu, 2, v, rail_along_v(grid));
    }
    // Left radial spur track toward coal hopper.
    for u in (u0 + 2)..=(tu - tr) {
        set_uv(&mut blocks, grid, u, 2, tv, rail_along_u(grid));
    }

    // 3. Elevated Coal Hopper Tower on left apron.
    let hop_u = u0 + 1;
    let hop_v = (tv - 1).max(bay_v1 + 1);
    if grid.in_bounds(hop_u, hop_v) && h >= 7 {
        set_uv(
            &mut blocks,
            grid,
            hop_u,
            2,
            hop_v,
            PlannedBlock::simple("minecraft:oak_fence"),
        );
        set_uv(
            &mut blocks,
            grid,
            hop_u,
            3,
            hop_v,
            PlannedBlock::with_props("minecraft:hopper", vec![("facing", "down")]),
        );
        set_uv(
            &mut blocks,
            grid,
            hop_u,
            4,
            hop_v,
            PlannedBlock::simple("minecraft:deepslate_coal_ore"),
        );
        set_uv(
            &mut blocks,
            grid,
            hop_u,
            5,
            hop_v,
            PlannedBlock::simple("minecraft:black_concrete"),
        );
    }

    // 4. Brick Tool & Dispatch Booth on right apron with complete door, lectern & ItemFrame tools.
    let tb_u0 = (tu + tr + 1).min(u1 - 3);
    let tb_u1 = u1;
    let tb_v0 = (v0 + 1).max(bay_v1 - 1);
    let tb_v1 = (tb_v0 + 3).min(v1 - 1);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        tb_u0,
        tb_u1,
        tb_v0,
        tb_v1,
        1,
        4.min(h - 3),
        h,
        "minecraft:bricks",
        "minecraft:deepslate_bricks",
        "minecraft:dark_oak_planks",
    );
    if tb_u1 - tb_u0 >= 2 && tb_v1 - tb_v0 >= 2 {
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            tb_u0 + 1,
            2,
            tb_v0 + 1,
            grid.dir_front(),
            LecternBookKind::EngineeringLog,
        );
        push_entity(
            &mut living,
            grid,
            tb_u0 + 1,
            3,
            tb_v0 + 1,
            LivingEntityKind::ItemFrame,
            grid.uv_to_facing(0, 1),
            "minecraft:iron_pickaxe",
            "",
            true,
        );
        push_entity(
            &mut living,
            grid,
            tb_u0 + 2,
            3,
            tb_v0 + 1,
            LivingEntityKind::ItemFrame,
            grid.uv_to_facing(0, 1),
            "minecraft:compass",
            "",
            true,
        );
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (tu - 2).max(u0),
        2,
        v1,
        SignPurpose::Station,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (tu + 2).min(u1),
        2,
        v1,
        BannerStyle::EngineerCog,
        "red",
        "yellow",
    );
    push_entity(
        &mut living,
        grid,
        (tu + 1).min(u1),
        2,
        (tv + tr + 1).min(v1),
        LivingEntityKind::Villager,
        grid.uv_to_facing(-1, 0),
        "toolsmith",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 3. Gantry Drydock: drained stepped stone drydock basin down into terrain, timber keel blocks &
/// partial oak/spruce ship ribs, quay rails carrying a tall straddle gantry crane with iron_chain hoist.
fn generate_gantry_drydock(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    let quay_y = if h >= 14 { 3 } else { 2 };
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, quay_y, terrain_offsets);

    let dock_hw = if grid.w < 11 {
        1
    } else {
        clamp_i32((grid.w - 6) / 6, 2, 3)
    };
    let dock_cu = clamp_i32(grid.w / 2 - 1, u0 + dock_hw + 1, u1 - dock_hw - 3);
    let d_u0 = (dock_cu - dock_hw).max(u0 + 1);
    let d_u1 = (dock_cu + dock_hw).min(u1 - 2);
    let d_v0 = v0 + 2;
    let d_v1 = (v1 - 2).max(d_v0 + 3);

    // Carve stepped drydock basin down to y=0 stone floor!
    for v in d_v0..=d_v1 {
        for u in d_u0..=d_u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                0,
                v,
                PlannedBlock::simple("minecraft:mossy_stone_bricks"),
            );
            let on_step = u == d_u0 || u == d_u1 || v == d_v0;
            for y in 1..=quay_y {
                if on_step && y == 1 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple("minecraft:stone_bricks"),
                    );
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    // Caisson sea-gate sealing front end of drydock.
    for u in d_u0..=d_u1 {
        for y in 1..=quay_y {
            set_uv(
                &mut blocks,
                grid,
                u,
                y,
                d_v1 + 1,
                PlannedBlock::simple("minecraft:deepslate_bricks"),
            );
        }
    }

    // Timber keel blocks & curved oak/spruce ship ribs inside drydock basin!
    for v in (d_v0 + 1)..=(d_v1 - 1) {
        if (v - d_v0) % 2 == 1 {
            set_uv(
                &mut blocks,
                grid,
                dock_cu,
                1,
                v,
                PlannedBlock::simple("minecraft:dark_oak_log"),
            );
        }
        set_uv(
            &mut blocks,
            grid,
            dock_cu,
            2,
            v,
            PlannedBlock::simple("minecraft:oak_planks"),
        );
        if (v - d_v0) % 2 == 0 && d_u1 - d_u0 >= 2 {
            for side in [-1, 1] {
                let ru = dock_cu + side;
                set_uv(
                    &mut blocks,
                    grid,
                    ru,
                    2,
                    v,
                    PlannedBlock::simple("minecraft:spruce_planks"),
                );
                set_uv(
                    &mut blocks,
                    grid,
                    ru,
                    3,
                    v,
                    PlannedBlock::simple("minecraft:oak_log"),
                );
                if dock_hw >= 2 && grid.in_bounds(dock_cu + side * 2, v) && quay_y + 1 < h {
                    set_uv(
                        &mut blocks,
                        grid,
                        dock_cu + side * 2,
                        3,
                        v,
                        PlannedBlock::simple("minecraft:spruce_stairs"),
                    );
                }
            }
        }
    }

    // Quay crane rails along both drydock shoulders.
    let rail_u0 = (d_u0 - 1).max(u0);
    let rail_u1 = (d_u1 + 1).min(u1);
    for v in v0..=v1 {
        set_uv(&mut blocks, grid, rail_u0, quay_y + 1, v, rail_along_v(grid));
        set_uv(&mut blocks, grid, rail_u1, quay_y + 1, v, rail_along_v(grid));
    }

    // Tall Yellow/Copper/Iron Straddle Gantry Crane spanning the basin!
    let crane_v0 = clamp_i32((d_v0 + d_v1) / 2 - 1, v0 + 1, v1 - 3);
    let crane_v1 = (crane_v0 + 2).min(v1 - 1);
    let crane_top = clamp_i32(h - 2, quay_y + 5, 16);
    for cv_leg in [crane_v0, crane_v1] {
        for cu_leg in [rail_u0, rail_u1] {
            for y in (quay_y + 1)..=crane_top {
                let blk = if y % 3 == 0 {
                    "minecraft:cut_copper"
                } else {
                    "minecraft:yellow_concrete"
                };
                set_uv(&mut blocks, grid, cu_leg, y, cv_leg, PlannedBlock::simple(blk));
            }
        }
    }
    // Cross gantry bridge girders & iron_bars catwalk guardrails.
    for u in rail_u0..=rail_u1 {
        for v in crane_v0..=crane_v1 {
            let is_beam = v == crane_v0 || v == crane_v1 || u == dock_cu;
            if is_beam {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    crane_top,
                    v,
                    PlannedBlock::simple("minecraft:yellow_terracotta"),
                );
                if crane_top + 1 < h && (v == crane_v0 || v == crane_v1) {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        crane_top + 1,
                        v,
                        PlannedBlock::simple("minecraft:iron_bars"),
                    );
                }
            }
        }
    }
    // Overhead trolley + iron_chain hoist lowering a cargo crate into the ship hull!
    let mid_cv = (crane_v0 + crane_v1) / 2;
    set_uv(
        &mut blocks,
        grid,
        dock_cu,
        crane_top,
        mid_cv,
        PlannedBlock::simple("minecraft:iron_block"),
    );
    let hoist_bot = (crane_top - 3).max(quay_y + 2);
    for y in (hoist_bot + 1)..crane_top {
        set_uv(
            &mut blocks,
            grid,
            dock_cu,
            y,
            mid_cv,
            PlannedBlock::with_props("minecraft:iron_chain", vec![("axis", "y")]),
        );
    }
    set_uv(
        &mut blocks,
        grid,
        dock_cu,
        hoist_bot,
        mid_cv,
        PlannedBlock::simple("minecraft:barrel"),
    );

    // Quay entry stairs on front edge.
    for u in [u0, (rail_u1 + 1).min(u1)] {
        if grid.in_bounds(u, v1) {
            set_uv(
                &mut blocks,
                grid,
                u,
                quay_y,
                v1,
                make_stair("minecraft:stone_brick_stairs", grid.dir_back(), "bottom"),
            );
        }
    }

    // Control booth on right quay.
    let cb_u0 = (rail_u1 + 1).min(u1 - 2);
    let cb_u1 = u1;
    let cb_v0 = v0;
    let cb_v1 = (v0 + 3).min(v1 - 2);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        cb_u0,
        cb_u1,
        cb_v0,
        cb_v1,
        quay_y,
        (quay_y + 3).min(h - 3),
        h,
        "minecraft:bricks",
        "minecraft:cut_copper",
        "minecraft:smooth_stone",
    );

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        rail_u0,
        quay_y + 1,
        v1 - 1,
        SignPurpose::Harbor,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        rail_u1,
        quay_y + 1,
        v1 - 1,
        BannerStyle::HarborNautical,
        "yellow",
        "blue",
    );
    push_entity(
        &mut living,
        grid,
        (rail_u1 + 1).min(u1),
        quay_y + 1,
        (cb_v1 + 1).min(v1 - 1),
        LivingEntityKind::Villager,
        grid.uv_to_facing(-1, 0),
        "armorer",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 4. Power Substation: tall tapered wind turbine mast with 3-blade rotor + lightning rod finial,
/// copper-coil transformer yard with quartz porcelain insulators, brick switch house & daylight lamps.
fn generate_power_substation(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Gravel yard with iron_bars security perimeter and front gate opening.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple(if rim {
                    "minecraft:smooth_stone"
                } else {
                    "minecraft:gravel"
                }),
            );
            if rim && !(v == v1 && (u - cu).abs() <= 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    v,
                    PlannedBlock::simple("minecraft:iron_bars"),
                );
            }
        }
    }

    // 1. Brick Switch House in back-right quadrant.
    let sw_u0 = clamp_i32(cu + 1, u0 + 4, u1 - 4);
    let sw_u1 = u1 - 1;
    let sw_v0 = v0 + 1;
    let sw_v1 = clamp_i32(v0 + 4, sw_v0 + 2, cv + 1);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        sw_u0,
        sw_u1,
        sw_v0,
        sw_v1,
        1,
        5.min(h - 3),
        h,
        "minecraft:bricks",
        "minecraft:deepslate_bricks",
        "minecraft:smooth_stone",
    );
    if sw_u1 - sw_u0 >= 2 && sw_v1 - sw_v0 >= 2 {
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            sw_u0 + 1,
            2,
            sw_v0 + 1,
            grid.dir_front(),
            LecternBookKind::EngineeringLog,
        );
    }

    // 2. Tall Tapered Wind Turbine Mast with 3-Blade Rotor + Lightning Rod Finial.
    let tu = clamp_i32(u0 + 3, u0 + 2, (sw_u0 - 2).max(u0 + 2));
    let tv = clamp_i32(v0 + 3, v0 + 2, cv);
    let hub_y = clamp_i32(h - 4, 7, 24);
    // Grounded 3x3 concrete plinth.
    for dv in -1..=1 {
        for du in -1..=1 {
            set_uv(
                &mut blocks,
                grid,
                tu + du,
                2,
                tv + dv,
                PlannedBlock::simple("minecraft:smooth_quartz"),
            );
            if du == 0 || dv == 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    tu + du,
                    3,
                    tv + dv,
                    PlannedBlock::simple("minecraft:iron_bars"),
                );
            }
        }
    }
    for y in 3..=hub_y {
        let mast_id = if y <= hub_y / 2 {
            "minecraft:quartz_block"
        } else {
            "minecraft:quartz_pillar"
        };
        set_uv(&mut blocks, grid, tu, y, tv, PlannedBlock::simple(mast_id));
    }
    // Nacelle & Lightning Rod Finial.
    for dv in -1..=1 {
        let nv = (tv + dv).clamp(v0, v1);
        let mat = if dv < 0 {
            "minecraft:copper_block"
        } else {
            "minecraft:white_concrete"
        };
        set_uv(&mut blocks, grid, tu, hub_y, nv, PlannedBlock::simple(mat));
    }
    if hub_y + 1 < h {
        set_uv(
            &mut blocks,
            grid,
            tu,
            hub_y + 1,
            tv,
            PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
        );
    }
    // 3-Blade Rotor at front of nacelle (orthogonally step-connected so zero pruning).
    let rv = (tv + 2).min(v1);
    set_uv(
        &mut blocks,
        grid,
        tu,
        hub_y,
        rv,
        PlannedBlock::simple("minecraft:iron_block"),
    );
    let blade_r = (hub_y - 3)
        .min(tu - u0)
        .min(u1 - tu)
        .min(h - 1 - hub_y)
        .clamp(2, 5);
    for i in 1..=blade_r {
        set_uv(
            &mut blocks,
            grid,
            tu,
            hub_y + i,
            rv,
            PlannedBlock::simple("minecraft:white_concrete"),
        );
        for side in [-1, 1] {
            let bx = tu + side * i;
            let by = hub_y - i;
            if grid.in_bounds(bx, rv) && by >= 3 {
                set_uv(
                    &mut blocks,
                    grid,
                    bx,
                    by + 1,
                    rv,
                    PlannedBlock::simple("minecraft:smooth_quartz_slab"),
                );
                set_uv(
                    &mut blocks,
                    grid,
                    bx,
                    by,
                    rv,
                    PlannedBlock::simple("minecraft:white_concrete"),
                );
            }
        }
    }

    // 3. Copper-Coil Transformer Yard with Porcelain Quartz Insulators & Overhead Busbars.
    let xf_v = (v1 - 3).max(sw_v1 + 2);
    for (idx, xf_u) in [u0 + 2, cu].into_iter().enumerate() {
        if !grid.in_bounds(xf_u, xf_v) || xf_u >= u1 - 1 {
            continue;
        }
        set_uv(
            &mut blocks,
            grid,
            xf_u,
            2,
            xf_v,
            PlannedBlock::simple(if idx % 2 == 0 {
                "minecraft:copper_block"
            } else {
                "minecraft:cut_copper"
            }),
        );
        set_uv(
            &mut blocks,
            grid,
            xf_u,
            3,
            xf_v,
            PlannedBlock::simple("minecraft:quartz_block"),
        );
        set_uv(
            &mut blocks,
            grid,
            xf_u,
            4,
            xf_v,
            PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
        );
        if xf_u + 1 < u1 {
            set_uv(
                &mut blocks,
                grid,
                xf_u + 1,
                2,
                xf_v,
                PlannedBlock::simple("minecraft:copper_block"),
            );
            set_uv(
                &mut blocks,
                grid,
                xf_u + 1,
                3,
                xf_v,
                PlannedBlock::simple("minecraft:quartz_block"),
            );
        }
    }

    // Daylight detector + redstone lamp yard floodlights.
    place_daylight_lamp(&mut blocks, grid, u0 + 1, 2, v1 - 1, 2, "minecraft:iron_bars", h);
    place_daylight_lamp(&mut blocks, grid, u1 - 1, 2, v1 - 1, 2, "minecraft:iron_bars", h);

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (cu - 2).max(u0 + 1),
        2,
        v1,
        SignPurpose::PowerPlant,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + 2).min(u1 - 1),
        2,
        v1,
        BannerStyle::EngineerCog,
        "yellow",
        "gray",
    );
    push_entity(
        &mut living,
        grid,
        cu,
        2,
        (v1 - 2).max(sw_v1 + 1),
        LivingEntityKind::Villager,
        grid.uv_to_facing(0, 1),
        "toolsmith",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

// ============================================================================
// 2. AGRICULTURAL
// ============================================================================

/// 5. Vineyard & Wine Press: tiered grape trellises with persistent azalea_leaves & sweet_berry_bush,
/// stone grape-crushing vat & wooden screw press, barrel aging cellar pergola.
fn generate_vineyard_press(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Grassy courtyard with cobbled path & low perimeter stone wall.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple(if rim || (u - cu).abs() <= 1 {
                    "minecraft:cobblestone"
                } else {
                    "minecraft:grass_block"
                }),
            );
            if rim && !(v == v1 && (u - cu).abs() <= 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    v,
                    PlannedBlock::simple("minecraft:cobblestone_wall"),
                );
            }
        }
    }

    // 1. Barrel Aging Cellar / Press House at rear-right.
    let cell_u0 = clamp_i32(cu + 1, u0 + 3, u1 - 4);
    let cell_u1 = u1 - 1;
    let cell_v0 = v0 + 1;
    let cell_v1 = clamp_i32(v0 + 4, cell_v0 + 2, v1 - 3);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        cell_u0,
        cell_u1,
        cell_v0,
        cell_v1,
        1,
        5.min(h - 3),
        h,
        "minecraft:stone_bricks",
        "minecraft:oak_log",
        "minecraft:terracotta",
    );
    if cell_u1 - cell_u0 >= 2 && cell_v1 - cell_v0 >= 2 {
        set_uv(
            &mut blocks,
            grid,
            cell_u1 - 1,
            2,
            cell_v0 + 1,
            PlannedBlock::simple("minecraft:barrel"),
        );
        if cell_u1 - cell_u0 >= 3 {
            set_uv(
                &mut blocks,
                grid,
                cell_u1 - 1,
                3,
                cell_v0 + 1,
                PlannedBlock::simple("minecraft:barrel"),
            );
        }
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            cell_u0 + 1,
            2,
            cell_v0 + 1,
            grid.dir_front(),
            LecternBookKind::FarmAlmanac,
        );
    }

    // 2. Tiered Grape Trellises on Left & Rear Terraces (farmland/grass alleys + azalea vine canopies + sweet_berry_bush).
    let trel_u1 = (cell_u0 - 1).max(u0 + 2);
    for (row_idx, tv) in ((v0 + 2)..=(v1 - 2)).step_by(3).enumerate() {
        let tier_y = if row_idx == 0 && grid.d >= 13 { 2 } else { 1 };
        for u in (u0 + 1)..=trel_u1 {
            if tier_y == 2 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    tv,
                    PlannedBlock::simple("minecraft:stone_bricks"),
                );
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    tv,
                    PlannedBlock::simple("minecraft:grass_block"),
                );
            } else {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    tv,
                    PlannedBlock::simple(if u % 2 == 0 {
                        "minecraft:grass_block"
                    } else {
                        "minecraft:farmland"
                    }),
                );
            }
            if u == u0 + 1 || u == trel_u1 || (u - (u0 + 1)) % 3 == 0 {
                for dy in 1..=2 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        tier_y + dy,
                        tv,
                        PlannedBlock::simple("minecraft:oak_fence"),
                    );
                }
            } else {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    tier_y + 1,
                    tv,
                    PlannedBlock::with_props("minecraft:sweet_berry_bush", vec![("age", "3")]),
                );
            }
            if tier_y + 3 < h {
                let leaf = if (u + row_idx as i32) % 3 == 0 {
                    "minecraft:flowering_azalea_leaves"
                } else {
                    "minecraft:azalea_leaves"
                };
                set_uv(&mut blocks, grid, u, tier_y + 3, tv, make_leaves(leaf));
            }
        }
    }

    // 3. Stone Grape-Crushing Vat & Wooden Screw Press on Front-Right Courtyard.
    let vat_u = clamp_i32(cu + 2, trel_u1 + 2, u1 - 2);
    let vat_v = clamp_i32(v1 - 3, cell_v1 + 2, v1 - 2);
    if grid.in_bounds(vat_u, vat_v) && vat_u < u1 {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = vat_u + du;
                let v = vat_v + dv;
                if !grid.in_bounds(u, v) {
                    continue;
                }
                if du == 0 && dv == 0 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        2,
                        v,
                        PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]),
                    );
                } else {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        2,
                        v,
                        PlannedBlock::simple("minecraft:stone_brick_slab"),
                    );
                }
            }
        }
        if vat_u - 1 >= u0 + 1 && vat_u + 1 <= u1 - 1 && h >= 7 {
            for y in 2..=4 {
                set_uv(
                    &mut blocks,
                    grid,
                    vat_u - 1,
                    y,
                    vat_v,
                    PlannedBlock::simple("minecraft:stripped_oak_log"),
                );
                set_uv(
                    &mut blocks,
                    grid,
                    vat_u + 1,
                    y,
                    vat_v,
                    PlannedBlock::simple("minecraft:stripped_oak_log"),
                );
            }
            for u in (vat_u - 1)..=(vat_u + 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    5,
                    vat_v,
                    PlannedBlock::simple("minecraft:oak_planks"),
                );
            }
            set_uv(
                &mut blocks,
                grid,
                vat_u,
                4,
                vat_v,
                PlannedBlock::simple("minecraft:oak_fence"),
            );
            set_uv(
                &mut blocks,
                grid,
                vat_u,
                3,
                vat_v,
                make_slab("minecraft:spruce_planks", "top"),
            );
        }
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (cu - 2).max(u0 + 1),
        2,
        v1,
        SignPurpose::Vineyard,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + 2).min(u1 - 1),
        2,
        v1,
        BannerStyle::ParkBotanical,
        "purple",
        "lime",
    );
    push_entity(
        &mut living,
        grid,
        cu,
        2,
        (v1 - 2).max(v0 + 2),
        LivingEntityKind::Villager,
        grid.uv_to_facing(0, 1),
        "farmer",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 6. Grain Silo Farm: gambrel-roofed timber barn with pulley hoist & hay bales joined to a
/// cylindrical masonry/copper-domed grain silo with ladder, unloading chute & hydrated wheat patch.
fn generate_grain_silo_farm(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in v0..=v1 {
        for u in u0..=u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:grass_block"),
            );
        }
    }

    // 1. Cylindrical Masonry & Copper-Domed Grain Silo on Right Side.
    let silo_r = if grid.w >= 13 { 2 } else { 1 };
    let silo_cu = (u1 - silo_r - 1).max(u0 + 5);
    let silo_cv = clamp_i32(v0 + silo_r + 1, v0 + 2, v1 - silo_r - 2);
    let barn_u0 = u0;
    let barn_u1 = (silo_cu - silo_r - 1).max(barn_u0 + 4);
    let barn_v0 = v0;
    let barn_v1 = clamp_i32(v0 + (grid.d * 48) / 100, barn_v0 + 4, v1 - 4);
    let barn_cu = (barn_u0 + barn_u1) / 2;

    // Barn floor & walls.
    let eave_y = clamp_i32(h / 2, 4, 6);
    for v in barn_v0..=barn_v1 {
        for u in barn_u0..=barn_u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:spruce_planks"),
            );
            for y in 2..=eave_y {
                let shell = u == barn_u0 || u == barn_u1 || v == barn_v0 || v == barn_v1;
                if !shell {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    continue;
                }
                let corner = (u == barn_u0 || u == barn_u1) && (v == barn_v0 || v == barn_v1);
                let win = !corner && v != barn_v1 && y == 3 && (u + v) % 3 == 0;
                let blk = if corner {
                    PlannedBlock::simple("minecraft:oak_log")
                } else if win {
                    PlannedBlock::simple("minecraft:oak_fence")
                } else {
                    PlannedBlock::simple("minecraft:red_terracotta")
                };
                set_uv(&mut blocks, grid, u, y, v, blk);
            }
        }
    }

    // Authentic Gambrel Roof along `v`!
    let half_w = ((barn_u1 - barn_u0) / 2).max(2);
    for v in (barn_v0 - 1).max(0)..=(barn_v1 + 1).min(grid.d - 1) {
        for u in (barn_u0 - 1).max(0)..=(barn_u1 + 1).min(grid.w - 1) {
            let dist = (u - barn_cu).abs();
            let (dy, steep) = if dist >= half_w {
                (1, true)
            } else if dist * 2 >= half_w {
                (2, true)
            } else if dist > 0 {
                (3, false)
            } else {
                (4, false)
            };
            let ry = (eave_y + dy).min(h - 1);
            // Fill gable end triangle under roof.
            if (v == barn_v0 || v == barn_v1) && u >= barn_u0 && u <= barn_u1 {
                for wy in (eave_y + 1)..ry {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        wy,
                        v,
                        PlannedBlock::simple("minecraft:red_terracotta"),
                    );
                }
            }
            if dist == 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_slab("minecraft:dark_oak_planks", "bottom"),
                );
            } else {
                let face = if u < barn_cu {
                    grid.dir_right()
                } else {
                    grid.dir_left()
                };
                let blk = if steep {
                    make_stair("minecraft:dark_oak_stairs", face, "bottom")
                } else {
                    make_slab("minecraft:dark_oak_planks", "bottom")
                };
                set_uv(&mut blocks, grid, u, ry, v, blk);
            }
        }
    }
    // Hayloft pulley boom + iron_chain hoist over hay bales!
    let boom_y = (eave_y + 3).min(h - 2);
    if barn_v1 + 1 < grid.d && boom_y > 4 {
        set_uv(
            &mut blocks,
            grid,
            barn_cu,
            boom_y,
            barn_v1,
            PlannedBlock::simple("minecraft:oak_log"),
        );
        set_uv(
            &mut blocks,
            grid,
            barn_cu,
            boom_y,
            barn_v1 + 1,
            PlannedBlock::simple("minecraft:oak_log"),
        );
        set_uv(
            &mut blocks,
            grid,
            barn_cu,
            boom_y - 1,
            barn_v1 + 1,
            PlannedBlock::with_props("minecraft:iron_chain", vec![("axis", "y")]),
        );
        set_uv(
            &mut blocks,
            grid,
            barn_cu,
            boom_y - 2,
            barn_v1 + 1,
            PlannedBlock::simple("minecraft:hay_block"),
        );
    }

    // Interior hay bales & FarmAlmanac lectern.
    if barn_u1 - barn_u0 >= 3 && barn_v1 - barn_v0 >= 3 {
        set_uv(
            &mut blocks,
            grid,
            barn_u0 + 1,
            2,
            barn_v0 + 1,
            PlannedBlock::simple("minecraft:hay_block"),
        );
        set_uv(
            &mut blocks,
            grid,
            barn_u0 + 1,
            3,
            barn_v0 + 1,
            PlannedBlock::simple("minecraft:hay_block"),
        );
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            barn_u1 - 1,
            2,
            barn_v0 + 1,
            grid.dir_front(),
            LecternBookKind::FarmAlmanac,
        );
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, barn_cu, 2, barn_v1));

    // 2. Cylindrical Masonry Silo + Copper Dome + Ladder + Hopper Unloading Chute!
    let silo_top = clamp_i32(h - 4, eave_y + 3, 16);
    for y in 2..=silo_top {
        let band = y % 3 == 0;
        for dv in -silo_r..=silo_r {
            for du in -silo_r..=silo_r {
                let u = silo_cu + du;
                let v = silo_cv + dv;
                if !grid.in_bounds(u, v) {
                    continue;
                }
                let d2 = du * du + dv * dv;
                if d2 <= silo_r * silo_r + 1 {
                    let shell = d2 >= (silo_r - 1).max(0) * (silo_r - 1).max(0);
                    let blk = if shell {
                        if band {
                            "minecraft:deepslate_bricks"
                        } else {
                            "minecraft:stone_bricks"
                        }
                    } else {
                        "minecraft:hay_block"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            }
        }
    }
    // Copper Dome atop Silo + Lightning Rod Finial.
    for dy in 1..=2 {
        let dome_y = silo_top + dy;
        if dome_y >= h {
            break;
        }
        let dr = if dy == 1 { silo_r } else { (silo_r - 1).max(0) };
        for dv in -dr..=dr {
            for du in -dr..=dr {
                if du * du + dv * dv <= dr * dr + 1 {
                    set_uv(
                        &mut blocks,
                        grid,
                        silo_cu + du,
                        dome_y,
                        silo_cv + dv,
                        PlannedBlock::simple(if dy == 2 {
                            "minecraft:cut_copper"
                        } else {
                            "minecraft:oxidized_copper"
                        }),
                    );
                }
            }
        }
    }
    if silo_top + 3 < h {
        set_uv(
            &mut blocks,
            grid,
            silo_cu,
            silo_top + 3,
            silo_cv,
            PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
        );
    }
    // Vertical ladder up front of silo (solid masonry directly behind at silo_cv + silo_r!).
    let lad_v = silo_cv + silo_r + 1;
    if grid.in_bounds(silo_cu, lad_v) {
        for y in 2..=silo_top {
            set_uv(
                &mut blocks,
                grid,
                silo_cu,
                y,
                lad_v,
                PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]),
            );
        }
    }
    // Unloading hopper chute & grain cart on silo side!
    let chute_u = (silo_cu - silo_r - 1).max(barn_u1);
    if grid.in_bounds(chute_u, silo_cv) {
        set_uv(
            &mut blocks,
            grid,
            chute_u,
            3,
            silo_cv,
            PlannedBlock::with_props("minecraft:hopper", vec![("facing", "down")]),
        );
        set_uv(
            &mut blocks,
            grid,
            chute_u,
            2,
            silo_cv,
            PlannedBlock::simple("minecraft:hay_block"),
        );
    }

    // 3. Hydrated Wheat Patch & Fenced Livestock Pen in Front Yard.
    let farm_v0 = (barn_v1 + 2).min(v1 - 2);
    let farm_u0 = u0 + 1;
    let farm_u1 = (u0 + (grid.w / 2).max(4)).min(u1 - 2);
    for v in farm_v0..=v1 - 1 {
        for u in farm_u0..=farm_u1 {
            if u == farm_u0 && v == farm_v0 {
                set_uv(&mut blocks, grid, u, 1, v, water_source());
            } else {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::with_props("minecraft:farmland", vec![("moisture", "7")]),
                );
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    v,
                    PlannedBlock::with_props("minecraft:wheat", vec![("age", "7")]),
                );
            }
        }
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (barn_cu + 2).min(u1),
        2,
        barn_v1 + 1,
        SignPurpose::FarmBarn,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (barn_cu - 2).max(u0),
        2,
        barn_v1 + 1,
        BannerStyle::ParkBotanical,
        "yellow",
        "brown",
    );
    let pen_u = (farm_u1 + 2).min(u1 - 1);
    push_entity(
        &mut living,
        grid,
        pen_u,
        2,
        v1 - 1,
        LivingEntityKind::Cow,
        grid.uv_to_facing(0, 1),
        "",
        "",
        false,
    );
    push_entity(
        &mut living,
        grid,
        (pen_u - 1).max(farm_u1 + 1),
        2,
        (farm_v0 + 1).min(v1 - 1),
        LivingEntityKind::Sheep,
        grid.uv_to_facing(-1, 0),
        "",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 7. Orchard & Apiary: spaced flowering oak/cherry trees (persistent=true leaves), raised
/// wooden beehive/bee_nest boxes atop posts ringed by wildflowers, honey cabin, Bee + Villager NPCs.
fn generate_orchard_apiary(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in v0..=v1 {
        for u in u0..=u1 {
            let on_path = (u - cu).abs() == 0 && v >= v0 + 3;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple(if on_path {
                    "minecraft:dirt_path"
                } else {
                    "minecraft:grass_block"
                }),
            );
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            if rim && !(v == v1 && (u - cu).abs() <= 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    v,
                    PlannedBlock::simple("minecraft:oak_fence"),
                );
            }
        }
    }

    // 1. Honey-Extraction Cabin in back-right corner.
    let cab_u0 = clamp_i32(cu + 1, u0 + 3, u1 - 4);
    let cab_u1 = u1 - 1;
    let cab_v0 = v0 + 1;
    let cab_v1 = clamp_i32(v0 + 4, cab_v0 + 2, v1 - 3);
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        cab_u0,
        cab_u1,
        cab_v0,
        cab_v1,
        1,
        4.min(h - 3),
        h,
        "minecraft:spruce_planks",
        "minecraft:oak_log",
        "minecraft:dark_oak_planks",
    );
    if cab_u1 - cab_u0 >= 2 && cab_v1 - cab_v0 >= 2 {
        set_uv(
            &mut blocks,
            grid,
            cab_u0 + 1,
            2,
            cab_v0 + 1,
            PlannedBlock::simple("minecraft:barrel"),
        );
        set_uv(
            &mut blocks,
            grid,
            cab_u1 - 1,
            2,
            cab_v0 + 1,
            PlannedBlock::simple("minecraft:smoker"),
        );
        set_uv(
            &mut blocks,
            grid,
            (cab_u0 + cab_u1) / 2,
            4,
            cab_v0 + 1,
            make_light("minecraft:lantern".into(), true),
        );
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            cab_u1 - 1,
            2,
            cab_v0 + 1.min(cab_v1 - 1 - cab_v0),
            grid.dir_front(),
            LecternBookKind::FarmAlmanac,
        );
    }

    // 2. Spaced Flowering Orchard Trees (cherry + flowering azalea/oak).
    let tree_spots = [
        (u0 + 2, v0 + 2, "minecraft:cherry_log", "minecraft:cherry_leaves"),
        (
            u0 + 2,
            (v1 - 3).max(v0 + 5),
            "minecraft:oak_log",
            "minecraft:flowering_azalea_leaves",
        ),
        (
            (u1 - 2).max(cu + 2),
            (v1 - 2).max(cab_v1 + 3),
            "minecraft:cherry_log",
            "minecraft:flowering_azalea_leaves",
        ),
    ];
    for &(tu, tv, trunk, leaf) in &tree_spots {
        if grid.in_bounds(tu, tv) && (tu < cab_u0 - 1 || tv > cab_v1 + 2) {
            place_tree(&mut blocks, grid, tu, 2, tv, trunk, leaf, 3, 2, 0, h);
        }
    }

    // 3. Raised Wooden Beehive / Bee Nest Posts + Wildflower Meadows.
    let flowers = [
        "minecraft:poppy",
        "minecraft:dandelion",
        "minecraft:cornflower",
        "minecraft:oxeye_daisy",
        "minecraft:wildflowers",
        "minecraft:pink_petals",
    ];
    let hive_spots = [
        (cu - 2, v0 + 3, true),
        (cu - 2, (v1 - 2).max(v0 + 4), false),
        ((cu + 2).min(u1 - 2), (v1 - 3).max(cab_v1 + 2), true),
    ];
    for (hi, &(hu, hv, is_hive)) in hive_spots.iter().enumerate() {
        if !grid.in_bounds(hu, hv) || hu <= u0 || hu >= u1 || hv <= v0 || hv >= v1 {
            continue;
        }
        set_uv(
            &mut blocks,
            grid,
            hu,
            2,
            hv,
            PlannedBlock::simple("minecraft:oak_fence"),
        );
        let box_id = if is_hive {
            "minecraft:beehive"
        } else {
            "minecraft:bee_nest"
        };
        set_uv(
            &mut blocks,
            grid,
            hu,
            3,
            hv,
            PlannedBlock::with_props(
                box_id,
                vec![("facing", grid.dir_front()), ("honey_level", "5")],
            ),
        );
        if 4 < h {
            set_uv(
                &mut blocks,
                grid,
                hu,
                4,
                hv,
                make_slab("minecraft:spruce_planks", "bottom"),
            );
        }
        for (fi, (du, dv)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
            let fu = hu + du;
            let fv = hv + dv;
            if grid.in_bounds(fu, fv) && get_uv(&blocks, grid, fu, 2, fv).is_none_or(|b| b.block_id == "minecraft:air") {
                set_uv(
                    &mut blocks,
                    grid,
                    fu,
                    2,
                    fv,
                    PlannedBlock::simple(flowers[(hi + fi) % flowers.len()]),
                );
            }
        }
        push_entity(
            &mut living,
            grid,
            hu,
            4,
            (hv + 1).min(v1 - 1),
            LivingEntityKind::Bee,
            grid.uv_to_facing(0, 1),
            "",
            "",
            false,
        );
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (cu - 1).max(u0 + 1),
        2,
        v1,
        SignPurpose::Apiary,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + 1).min(u1 - 1),
        2,
        v1,
        BannerStyle::ParkBotanical,
        "yellow",
        "brown",
    );
    push_entity(
        &mut living,
        grid,
        cu,
        2,
        (v1 - 2).max(v0 + 2),
        LivingEntityKind::Villager,
        grid.uv_to_facing(0, 1),
        "farmer",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

/// 8. Horse Stables: multi-stall spruce/oak stable wing with gated stalls, hay mangers, water_cauldron
/// troughs, tack room with ArmorStand + ItemFrame, fenced oval riding paddock & Horse NPCs.
fn generate_horse_stables(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in v0..=v1 {
        for u in u0..=u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:grass_block"),
            );
        }
    }

    // 1. Multi-Stall Stable Wing + Tack Room along Rear (`v0 ..= stall_front_v`).
    let stall_front_v = clamp_i32(v0 + 4, v0 + 3, v1 - 4);
    let tack_u0 = clamp_i32(u1 - 3, u0 + 4, u1 - 3);
    let stable_top = 5.min(h - 3);

    // Stable stalls (`u0 .. tack_u0`).
    for v in v0..=stall_front_v {
        for u in u0..tack_u0 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:coarse_dirt"),
            );
            let rel_u = (u - u0) % 3;
            if v == v0 || u == u0 {
                for y in 2..=stable_top {
                    let blk = if rel_u == 0 {
                        "minecraft:oak_log"
                    } else {
                        "minecraft:spruce_planks"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            } else if v == stall_front_v {
                if rel_u == 0 {
                    for y in 2..=stable_top {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple("minecraft:oak_log"),
                        );
                    }
                } else {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        2,
                        v,
                        PlannedBlock::with_props(
                            "minecraft:spruce_fence_gate",
                            vec![("facing", grid.dir_front()), ("open", "false")],
                        ),
                    );
                    if stable_top >= 4 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            stable_top,
                            v,
                            PlannedBlock::simple("minecraft:spruce_planks"),
                        );
                    }
                }
            } else if rel_u == 0 {
                // Stall partition wall.
                for y in 2..=3 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(if y == 2 {
                            "minecraft:spruce_planks"
                        } else {
                            "minecraft:spruce_fence"
                        }),
                    );
                }
            }
        }
    }
    // Stall mangers (`hay_block`), `water_cauldron` troughs & Horse NPCs inside stalls.
    for su in ((u0 + 1)..(tack_u0 - 1)).step_by(3) {
        set_uv(
            &mut blocks,
            grid,
            su,
            2,
            v0 + 1,
            PlannedBlock::simple("minecraft:hay_block"),
        );
        if su + 1 < tack_u0 {
            set_uv(
                &mut blocks,
                grid,
                su + 1,
                2,
                v0 + 1,
                PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]),
            );
        }
        push_entity(
            &mut living,
            grid,
            su,
            2,
            (v0 + 2).min(stall_front_v - 1),
            LivingEntityKind::Horse,
            grid.uv_to_facing(0, 1),
            "",
            "",
            false,
        );
    }

    // Tack Room at right end of stable wing (`tack_u0 ..= u1`).
    let door_pos = build_service_booth(
        &mut blocks,
        grid,
        pal,
        seed,
        tack_u0,
        u1,
        v0,
        stall_front_v,
        1,
        stable_top,
        h,
        "minecraft:spruce_planks",
        "minecraft:oak_log",
        "minecraft:dark_oak_planks",
    );
    // Unify gable roof across entire stable wing + tack room!
    build_gable_roof(
        &mut blocks,
        grid,
        pal,
        seed,
        u0,
        u1,
        v0,
        stall_front_v,
        stable_top,
        h,
        false,
    );
    if u1 - tack_u0 >= 2 && stall_front_v - v0 >= 2 {
        push_entity(
            &mut living,
            grid,
            tack_u0 + 1,
            2,
            v0 + 1,
            LivingEntityKind::ArmorStand,
            grid.uv_to_facing(0, 1),
            "minecraft:iron_sword",
            "leather",
            true,
        );
        push_entity(
            &mut living,
            grid,
            u1 - 1,
            3,
            v0 + 1,
            LivingEntityKind::ItemFrame,
            grid.uv_to_facing(0, 1),
            "minecraft:saddle",
            "",
            true,
        );
        push_lectern(
            &mut blocks,
            &mut living,
            grid,
            u1 - 1,
            2,
            v0 + 1,
            grid.dir_front(),
            LecternBookKind::FarmAlmanac,
        );
    }

    // 2. Fenced Oval Riding Paddock in front courtyard (`stall_front_v + 2 ..= v1`).
    let pad_v0 = (stall_front_v + 2).min(v1 - 2);
    let pad_v1 = v1;
    let pad_cu = cu;
    let pad_cv = (pad_v0 + pad_v1) / 2;
    let rx = ((u1 - u0) as f64 * 0.48).max(2.5);
    let rz = ((pad_v1 - pad_v0) as f64 * 0.50).max(1.5);
    for v in pad_v0..=pad_v1 {
        for u in u0..=u1 {
            let dx = (u as f64 - pad_cu as f64) / rx;
            let dz = (v as f64 - pad_cv as f64) / rz;
            let r = (dx * dx + dz * dz).sqrt();
            if r <= 1.05 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(if r <= 0.80 {
                        "minecraft:sand"
                    } else {
                        "minecraft:coarse_dirt"
                    }),
                );
                if r >= 0.82 {
                    let is_gate = v >= pad_cv && (u - pad_cu).abs() == 0 && (v == pad_v1 || r >= 0.85);
                    if is_gate && v == pad_v1 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            2,
                            v,
                            PlannedBlock::with_props(
                                "minecraft:spruce_fence_gate",
                                vec![("facing", grid.dir_front()), ("open", "false")],
                            ),
                        );
                    } else {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            2,
                            v,
                            PlannedBlock::simple("minecraft:oak_fence"),
                        );
                    }
                }
            }
        }
    }
    // Paddock jump hurdle & riding Horse NPC.
    if pad_v1 - pad_v0 >= 3 && u1 - u0 >= 7 {
        set_uv(
            &mut blocks,
            grid,
            pad_cu - 1,
            2,
            pad_cv,
            PlannedBlock::simple("minecraft:oak_fence"),
        );
        set_uv(
            &mut blocks,
            grid,
            pad_cu,
            2,
            pad_cv,
            make_slab("minecraft:spruce_planks", "top"),
        );
        set_uv(
            &mut blocks,
            grid,
            pad_cu + 1,
            2,
            pad_cv,
            PlannedBlock::simple("minecraft:oak_fence"),
        );
    }
    push_entity(
        &mut living,
        grid,
        (pad_cu + 2).min(u1 - 1),
        2,
        pad_cv,
        LivingEntityKind::Horse,
        grid.uv_to_facing(-1, 0),
        "",
        "",
        false,
    );
    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (pad_cu - 2).max(u0),
        2,
        pad_v1,
        SignPurpose::Stables,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, door_pos, living)
}

// ============================================================================
// 3. DECOR & ART
// ============================================================================

/// 9. Plaza Fountain: sculpted multi-tier octagonal stone/quartz fountain with watertight stepped
/// basins, perimeter stair seating benches, trimmed persistent hedges, daylight lamps, Villager + Cat.
fn generate_plaza_fountain(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Checkerboard stone & quartz civic plaza pavement.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let mat = if u == u0 || u == u1 || v == v0 || v == v1 {
                "minecraft:stone_bricks"
            } else if (u + v) % 2 == 0 {
                "minecraft:smooth_quartz"
            } else {
                "minecraft:polished_andesite"
            };
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(mat));
        }
    }

    // 1. Multi-Tier Octagonal Fountain Basin (strictly contained water, zero spillover!).
    let r_out = clamp_i32((grid.w.min(grid.d) - 6) / 4, 2, 4);
    for dv in -r_out..=r_out {
        for du in -r_out..=r_out {
            let u = cu + du;
            let v = cv + dv;
            if !grid.in_bounds(u, v) {
                continue;
            }
            let oct = du.abs().max(dv.abs()) + (du.abs().min(dv.abs()) / 2);
            if oct == r_out {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    2,
                    v,
                    PlannedBlock::simple("minecraft:smooth_quartz"),
                );
            } else if oct < r_out {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(if (du + dv).abs() == 1 {
                        "minecraft:sea_lantern"
                    } else {
                        "minecraft:prismarine_bricks"
                    }),
                );
                set_uv(&mut blocks, grid, u, 2, v, water_source());
            }
        }
    }
    // Central pedestal & upper scalloped catch-basin (`y=4` floor, `y=5` rim + water).
    let ped_h = clamp_i32(h - 4, 4, 7);
    for y in 2..=ped_h {
        set_uv(
            &mut blocks,
            grid,
            cu,
            y,
            cv,
            PlannedBlock::simple(if y == ped_h {
                "minecraft:sea_lantern"
            } else {
                "minecraft:quartz_pillar"
            }),
        );
    }
    if ped_h + 1 < h {
        set_uv(
            &mut blocks,
            grid,
            cu,
            ped_h + 1,
            cv,
            make_slab("minecraft:smooth_quartz", "bottom"),
        );
    }
    if r_out >= 3 && h >= 8 {
        for dv in -2..=2 {
            for du in -2..=2 {
                let u = cu + du;
                let v = cv + dv;
                if du.abs() + dv.abs() > 3 || !grid.in_bounds(u, v) {
                    continue;
                }
                if du.abs() <= 1 && dv.abs() <= 1 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        4,
                        v,
                        PlannedBlock::simple("minecraft:smooth_quartz"),
                    );
                    if du != 0 || dv != 0 {
                        set_uv(&mut blocks, grid, u, 5, v, water_source());
                    }
                } else {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        5,
                        v,
                        make_slab("minecraft:smooth_quartz", "bottom"),
                    );
                }
            }
        }
    }

    // 2. Perimeter Seating Benches, Trimmed Persistent Hedges & Daylight Lamp Posts.
    for &(bu, bv, face) in &[
        (cu, v0 + 1, grid.dir_front()),
        (cu, v1 - 1, grid.dir_back()),
        (u0 + 1, cv, grid.dir_right()),
        (u1 - 1, cv, grid.dir_left()),
    ] {
        if grid.in_bounds(bu, bv) && (bu - cu).abs().max((bv - cv).abs()) > r_out {
            set_uv(
                &mut blocks,
                grid,
                bu,
                2,
                bv,
                make_stair("minecraft:spruce_stairs", face, "bottom"),
            );
        }
    }
    for (cu_corner, cv_corner) in [(u0, v0), (u1, v0), (u0, v1), (u1, v1)] {
        place_daylight_lamp(
            &mut blocks,
            grid,
            cu_corner,
            2,
            cv_corner,
            2,
            "minecraft:stone_brick_wall",
            h,
        );
        for (du, dv) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let hu = cu_corner + du;
            let hv = cv_corner + dv;
            if grid.in_bounds(hu, hv) && hu >= u0 && hu <= u1 && hv >= v0 && hv <= v1 {
                set_uv(
                    &mut blocks,
                    grid,
                    hu,
                    2,
                    hv,
                    make_leaves("minecraft:azalea_leaves"),
                );
                if h >= 6 {
                    set_uv(
                        &mut blocks,
                        grid,
                        hu,
                        3,
                        hv,
                        make_leaves("minecraft:flowering_azalea_leaves"),
                    );
                }
            }
        }
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        cu,
        2,
        (cv + r_out + 1).min(v1),
        SignPurpose::FountainPlaque,
    );
    push_entity(
        &mut living,
        grid,
        (cu - r_out - 1).max(u0),
        2,
        (cv + 1).min(v1 - 1),
        LivingEntityKind::Villager,
        grid.uv_to_facing(1, 0),
        "librarian",
        "",
        false,
    );
    push_entity(
        &mut living,
        grid,
        (cu + r_out + 1).min(u1),
        2,
        (cv - 1).max(v0 + 1),
        LivingEntityKind::Cat,
        grid.uv_to_facing(-1, 0),
        "",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, None, living)
}

/// 10. Guardian Statue: monumental inscribed plinth grounded via anchor_plinth, corner braziers,
/// sculpted winged stone/copper guardian sentinel wielding glowing sword & shield, RoyalCastle banners.
fn generate_guardian_statue(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in v0..=v1 {
        for u in u0..=u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple(if (u + v) % 2 == 0 {
                    "minecraft:polished_deepslate"
                } else {
                    "minecraft:stone_bricks"
                }),
            );
        }
    }

    // 1. Monumental Stepped Plinth Pedestal & Corner Flame Braziers.
    let ped_r = clamp_i32((grid.w.min(grid.d) - 4) / 4, 2, 3);
    let ped_top = clamp_i32(h / 4, 3, 6);
    for y in 2..=ped_top {
        let r = if y == 2 { ped_r } else { (ped_r - 1).max(1) };
        for dv in -r..=r {
            for du in -r..=r {
                let u = cu + du;
                let v = cv + dv;
                if !grid.in_bounds(u, v) {
                    continue;
                }
                let blk = if y == ped_top {
                    "minecraft:chiseled_stone_bricks"
                } else if y == 2 {
                    "minecraft:deepslate_bricks"
                } else {
                    "minecraft:stone_bricks"
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
        }
    }
    // Front ceremonial stairs onto pedestal base.
    for u in (cu - 1)..=(cu + 1) {
        if grid.in_bounds(u, cv + ped_r + 1) {
            set_uv(
                &mut blocks,
                grid,
                u,
                2,
                cv + ped_r + 1,
                make_stair("minecraft:stone_brick_stairs", grid.dir_back(), "bottom"),
            );
        }
    }
    // Corner soul/flame braziers around plinth.
    for (bu, bv) in [
        (cu - ped_r, cv - ped_r),
        (cu + ped_r, cv - ped_r),
        (cu - ped_r, cv + ped_r),
        (cu + ped_r, cv + ped_r),
    ] {
        if grid.in_bounds(bu, bv) && ped_top >= 4 {
            set_uv(
                &mut blocks,
                grid,
                bu,
                3,
                bv,
                PlannedBlock::simple("minecraft:stone_brick_wall"),
            );
            set_uv(
                &mut blocks,
                grid,
                bu,
                4,
                bv,
                PlannedBlock::with_props("minecraft:soul_campfire", vec![("lit", "true")]),
            );
        }
    }

    // 2. Sculpted Guardian Knight / Winged Sentinel.
    let mut rng = SplitMix64::new(seed ^ 0x57A7_0001);
    let armor_mat = if rng.range_i32(0, 1) == 0 {
        "minecraft:oxidized_copper"
    } else {
        "minecraft:stone_bricks"
    };
    let trim_mat = "minecraft:gold_block";
    let base = ped_top + 1;
    let statue_h = (h - 1 - base).max(5);
    let waist_y = base + (statue_h * 35 / 100).max(1);
    let shoulder_y = base + (statue_h * 72 / 100).max(3);
    let head_top = (h - 2).max(shoulder_y + 2).min(h - 1);

    // Legs & Boots.
    for y in base..waist_y {
        set_uv(&mut blocks, grid, cu - 1, y, cv, PlannedBlock::simple("minecraft:polished_deepslate"));
        set_uv(&mut blocks, grid, cu + 1, y, cv, PlannedBlock::simple("minecraft:polished_deepslate"));
    }
    // Armored Torso & Glowing Chest Core.
    for y in waist_y..=shoulder_y {
        for du in -1..=1 {
            let blk = if y == waist_y && du == 0 {
                trim_mat
            } else if y == (waist_y + shoulder_y) / 2 && du == 0 {
                "minecraft:sea_lantern"
            } else {
                armor_mat
            };
            set_uv(&mut blocks, grid, cu + du, y, cv, PlannedBlock::simple(blk));
            if grid.in_bounds(cu + du, cv - 1) && y < shoulder_y {
                set_uv(&mut blocks, grid, cu + du, y, cv - 1, PlannedBlock::simple(armor_mat));
            }
        }
    }
    // Outstretched Sculpted Wings (connected cleanly to shoulder back).
    if grid.w >= 11 && statue_h >= 6 {
        for side in [-1, 1] {
            for step in 2..=3 {
                let wu = cu + side * step;
                let wy = (shoulder_y + step - 2).min(h - 2);
                if grid.in_bounds(wu, cv - 1) {
                    set_uv(
                        &mut blocks,
                        grid,
                        wu,
                        wy,
                        cv - 1,
                        PlannedBlock::simple("minecraft:smooth_quartz"),
                    );
                    if wy - 1 >= waist_y {
                        set_uv(
                            &mut blocks,
                            grid,
                            wu,
                            wy - 1,
                            cv - 1,
                            PlannedBlock::simple("minecraft:smooth_quartz"),
                        );
                    }
                }
            }
        }
    }
    // Left Arm + Monumental Tower Shield (`cu + 2`).
    if grid.in_bounds(cu + 2, cv) {
        set_uv(&mut blocks, grid, cu + 2, shoulder_y, cv, PlannedBlock::simple(armor_mat));
        set_uv(&mut blocks, grid, cu + 2, shoulder_y - 1, cv, PlannedBlock::simple(armor_mat));
        for sy in (waist_y - 1).max(base)..=shoulder_y {
            if grid.in_bounds(cu + 2, cv + 1) {
                set_uv(
                    &mut blocks,
                    grid,
                    cu + 2,
                    sy,
                    cv + 1,
                    PlannedBlock::simple(if sy == waist_y {
                        trim_mat
                    } else {
                        "minecraft:deepslate_bricks"
                    }),
                );
            }
        }
    }
    // Right Arm + Grounded Glowing Greatsword (`cu - 2`).
    if grid.in_bounds(cu - 2, cv) {
        set_uv(&mut blocks, grid, cu - 2, shoulder_y, cv, PlannedBlock::simple(armor_mat));
        if grid.in_bounds(cu - 2, cv + 1) {
            for sy in base..=(shoulder_y + 1).min(h - 1) {
                let blk = if sy == waist_y + 1 {
                    trim_mat
                } else if sy > waist_y + 1 {
                    "minecraft:ochre_froglight"
                } else {
                    "minecraft:iron_block"
                };
                set_uv(&mut blocks, grid, cu - 2, sy, cv + 1, PlannedBlock::simple(blk));
            }
        }
    }
    // Helmeted Head, Glowing Visor & Crown Crest.
    for y in (shoulder_y + 1)..=head_top {
        set_uv(
            &mut blocks,
            grid,
            cu,
            y,
            cv,
            PlannedBlock::simple(if y == head_top {
                trim_mat
            } else {
                armor_mat
            }),
        );
    }
    if shoulder_y + 1 <= head_top && grid.in_bounds(cu, cv + 1) {
        set_uv(
            &mut blocks,
            grid,
            cu,
            shoulder_y + 1,
            cv + 1,
            PlannedBlock::simple("minecraft:sea_lantern"),
        );
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        cu,
        3,
        (cv + ped_r).min(v1),
        SignPurpose::StatuePlaque,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu - ped_r + 1).max(u0),
        3,
        (cv + ped_r).min(v1),
        BannerStyle::RoyalCastle,
        "red",
        "yellow",
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + ped_r - 1).min(u1),
        3,
        (cv + ped_r).min(v1),
        BannerStyle::RoyalCastle,
        "red",
        "yellow",
    );
    push_entity(
        &mut living,
        grid,
        (cu + 2).min(u1),
        2,
        v1 - 1,
        LivingEntityKind::IronGolem,
        grid.uv_to_facing(0, 1),
        "",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, None, living)
}

/// 11. Hedge Maze Park: symmetrical topiary hedge maze (`persistent=true` leaves framed by stone trim
/// posts, guaranteed walkable >= 2-block-tall corridors), rose/flower parterres & central vine pergola.
fn generate_hedge_maze_park(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Base gravel walkways across park.
    for v in v0..=v1 {
        for u in u0..=u1 {
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:gravel"),
            );
        }
    }

    let c_r = if grid.w.min(grid.d) >= 17 { 3 } else { 2 };
    let max_ring = ((cu - u0).min(cv - v0)).max(c_r + 2);
    let hedge_h = if h >= 7 { 3 } else { 2 };

    // Concentric alternating-gate topiary hedge rings (guaranteed walkable from outer gates to center!).
    for v in v0..=v1 {
        for u in u0..=u1 {
            let du = (u - cu).abs();
            let dv = (v - cv).abs();
            let cheb = du.max(dv);
            if cheb <= c_r {
                continue;
            }
            // Corner flower parterre alcoves!
            let in_corner_parterre = du >= max_ring - 2 && dv >= max_ring - 2 && max_ring >= 5;
            if in_corner_parterre {
                if du < max_ring && dv < max_ring {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        1,
                        v,
                        PlannedBlock::simple("minecraft:grass_block"),
                    );
                    let flower = match (u + v) % 4 {
                        0 => "minecraft:poppy",
                        1 => "minecraft:pink_petals",
                        2 => "minecraft:cornflower",
                        _ => "minecraft:oxeye_daisy",
                    };
                    set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple(flower));
                }
                continue;
            }

            let offset_from_outer = max_ring - cheb;
            if offset_from_outer >= 0 && offset_from_outer % 2 == 0 {
                let ring_idx = offset_from_outer / 2;
                let on_axial = du == 0 || dv == 0;
                let on_diag_gate = (du - dv).abs() <= 1 && du > 1 && dv > 1;
                let is_gate = if ring_idx % 2 == 0 {
                    on_axial
                } else {
                    on_diag_gate
                };
                if !is_gate {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        1,
                        v,
                        PlannedBlock::simple("minecraft:grass_block"),
                    );
                    let is_corner_post = du == dv || (ring_idx == 0 && (du <= 1 || dv <= 1));
                    for dy in 1..=hedge_h {
                        let blk = if is_corner_post {
                            PlannedBlock::simple("minecraft:stone_brick_wall")
                        } else if dy == hedge_h && (u + v) % 3 == 0 {
                            make_leaves("minecraft:flowering_azalea_leaves")
                        } else {
                            make_leaves("minecraft:azalea_leaves")
                        };
                        set_uv(&mut blocks, grid, u, 1 + dy, v, blk);
                    }
                    if is_corner_post && 2 + hedge_h < h && (du + dv) % 4 == 0 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            2 + hedge_h,
                            v,
                            make_light("minecraft:lantern".into(), false),
                        );
                    }
                }
            }
        }
    }

    // Central Octagonal Vine Pergola & Mini-Fountain!
    for dv in -c_r..=c_r {
        for du in -c_r..=c_r {
            let u = cu + du;
            let v = cv + dv;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple("minecraft:smooth_stone"),
            );
        }
    }
    set_uv(
        &mut blocks,
        grid,
        cu,
        1,
        cv,
        PlannedBlock::simple("minecraft:sea_lantern"),
    );
    set_uv(&mut blocks, grid, cu, 2, cv, water_source());
    for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        set_uv(
            &mut blocks,
            grid,
            cu + du,
            2,
            cv + dv,
            PlannedBlock::simple("minecraft:stone_brick_slab"),
        );
    }
    let perg_top = (hedge_h + 2).min(h - 2);
    for dv in [-c_r + 1, c_r - 1] {
        for du in [-c_r + 1, c_r - 1] {
            for y in 2..=perg_top {
                set_uv(
                    &mut blocks,
                    grid,
                    cu + du,
                    y,
                    cv + dv,
                    PlannedBlock::simple("minecraft:quartz_pillar"),
                );
            }
        }
    }
    for dv in (-c_r + 1)..=(c_r - 1) {
        for du in (-c_r + 1)..=(c_r - 1) {
            if perg_top + 1 < h {
                set_uv(
                    &mut blocks,
                    grid,
                    cu + du,
                    perg_top + 1,
                    cv + dv,
                    make_leaves("minecraft:flowering_azalea_leaves"),
                );
            }
        }
    }

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (cu - 1).max(u0),
        2,
        v1,
        SignPurpose::Waypost,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + 1).min(u1),
        2,
        v1,
        BannerStyle::ParkBotanical,
        "green",
        "pink",
    );
    push_entity(
        &mut living,
        grid,
        cu + 1,
        2,
        (cv + c_r).min(v1 - 1),
        LivingEntityKind::Cat,
        grid.uv_to_facing(0, 1),
        "",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, None, living)
}

/// 12. Zen Karesansui: raked gravel & sand wave garden, mossy cobblestone/tuff rock islands,
/// crooked sakura/pine bonsai trees (persistent=true leaves), crimson arched footbridge & stone lanterns.
fn generate_zen_karesansui(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let mut living = LivingPlan::default();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    clear_air_box(&mut blocks, grid, 0, grid.w - 1, 0, grid.d - 1, 1, h - 1);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 0, terrain_offsets);

    // 1. Perimeter Garden Wall + Front Wooden Gate.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            if rim {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple("minecraft:stone_bricks"),
                );
                if !(v == v1 && (u - cu).abs() <= 1) {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        2,
                        v,
                        PlannedBlock::simple("minecraft:bamboo_fence"),
                    );
                }
            }
        }
    }

    // 2. Curved Streamlet (`y=1` water on solid `y=0` bed) & Raked Gravel/Sand Wave Grooves.
    let island1 = (clamp_i32(u0 + (grid.w / 3), u0 + 2, cu - 1), clamp_i32(v0 + 3, v0 + 2, cv));
    let island2 = (clamp_i32(u1 - (grid.w / 4), cu + 1, u1 - 2), clamp_i32(v1 - 3, cv, v1 - 2));
    let stream_center_v = |u: i32| -> i32 {
        let wave = ((f64::from(u - u0) * 0.45).sin() * 1.4).round() as i32;
        clamp_i32(cv + wave, v0 + 2, v1 - 2)
    };

    for v in (v0 + 1)..=(v1 - 1) {
        for u in (u0 + 1)..=(u1 - 1) {
            let sv = stream_center_v(u);
            if (v - sv).abs() <= 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    0,
                    v,
                    PlannedBlock::simple("minecraft:mossy_cobblestone"),
                );
                set_uv(&mut blocks, grid, u, 1, v, water_source());
            } else {
                let d1 = (u - island1.0).abs().max((v - island1.1).abs());
                let d2 = (u - island2.0).abs().max((v - island2.1).abs());
                let d_min = d1.min(d2);
                let groove = if d_min <= 3 {
                    d_min % 2 == 0
                } else {
                    (v + ((f64::from(u) * 0.6).sin().round() as i32)).rem_euclid(2) == 0
                };
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(if groove {
                        "minecraft:gravel"
                    } else {
                        "minecraft:sand"
                    }),
                );
            }
        }
    }

    // 3. Mossy Cobblestone & Tuff Rock Islands.
    for (idx, &(iu, iv)) in [island1, island2].iter().enumerate() {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = iu + du;
                let v = iv + dv;
                if !grid.in_bounds(u, v) || u <= u0 || u >= u1 || v <= v0 || v >= v1 {
                    continue;
                }
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple("minecraft:moss_block"),
                );
                if du == 0 && dv == 0 {
                    let rock_h = if idx == 0 { 3 } else { 2 };
                    for y in 2..=(1 + rock_h).min(h - 1) {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(if y % 2 == 0 {
                                "minecraft:mossy_cobblestone"
                            } else {
                                "minecraft:tuff"
                            }),
                        );
                    }
                } else if (du + dv).abs() == 1 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        2,
                        v,
                        PlannedBlock::simple(if idx == 0 {
                            "minecraft:tuff_bricks"
                        } else {
                            "minecraft:azalea"
                        }),
                    );
                }
            }
        }
    }

    // 4. Crimson Arched Footbridge across the Streamlet at `u = cu - 1 ..= cu + 1`.
    let b_cv = stream_center_v(cu);
    for dv in -2..=2 {
        let v = (b_cv + dv).clamp(v0 + 1, v1 - 1);
        for du in -1..=1 {
            let u = cu + du;
            let deck_y = if dv.abs() <= 1 { 3 } else { 2 };
            let blk = if dv == 0 {
                PlannedBlock::simple("minecraft:crimson_planks")
            } else if dv.abs() == 1 {
                make_slab("minecraft:crimson_planks", "bottom")
            } else {
                make_stair(
                    "minecraft:crimson_stairs",
                    if dv < 0 {
                        grid.dir_front()
                    } else {
                        grid.dir_back()
                    },
                    "bottom",
                )
            };
            set_uv(&mut blocks, grid, u, deck_y, v, blk);
            if du.abs() == 1 && deck_y + 1 < h {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    deck_y + 1,
                    v,
                    PlannedBlock::simple("minecraft:crimson_fence"),
                );
            }
        }
    }

    // 5. Sculpted Crooked Sakura & Pine Bonsai Trees (persistent=true leaves).
    let sak_u = (u1 - 2).max(cu + 2);
    let sak_v = (v0 + 2).min(b_cv - 1);
    place_tree(
        &mut blocks,
        grid,
        sak_u,
        2,
        sak_v,
        "minecraft:cherry_log",
        "minecraft:cherry_leaves",
        4,
        2,
        -1,
        h,
    );
    if grid.w >= 13 {
        place_tree(
            &mut blocks,
            grid,
            u0 + 2,
            2,
            (v1 - 2).max(b_cv + 2),
            "minecraft:spruce_log",
            "minecraft:spruce_leaves",
            3,
            1,
            1,
            h,
        );
    }

    // 6. Stone Toro Lanterns, Waypost & Peaceful Living Entities.
    place_stone_lantern_post(
        &mut blocks,
        grid,
        (cu - 2).max(u0 + 1),
        2,
        (b_cv + 2).min(v1 - 1),
        "minecraft:tuff_brick_wall",
        "minecraft:lantern",
        "minecraft:stone_brick_slab",
        h,
    );
    place_stone_lantern_post(
        &mut blocks,
        grid,
        (cu + 2).min(u1 - 1),
        2,
        (b_cv - 2).max(v0 + 1),
        "minecraft:tuff_brick_wall",
        "minecraft:lantern",
        "minecraft:stone_brick_slab",
        h,
    );

    push_sign(
        &mut blocks,
        &mut living,
        grid,
        (cu - 2).max(u0),
        2,
        v1,
        SignPurpose::Waypost,
    );
    push_banner(
        &mut blocks,
        &mut living,
        grid,
        (cu + 2).min(u1),
        2,
        v1,
        BannerStyle::ParkBotanical,
        "pink",
        "white",
    );
    push_entity(
        &mut living,
        grid,
        cu,
        2,
        (v1 - 1).max(b_cv + 2),
        LivingEntityKind::Cat,
        grid.uv_to_facing(0, 1),
        "",
        "",
        false,
    );

    finish_landmark_with_living(req, spec, grid, height, blocks, noise, None, living)
}
