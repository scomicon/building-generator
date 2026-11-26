use std::collections::{BTreeMap, BTreeSet};

use crate::gen_common::{
    LocalGrid, PlannedBlock, get_uv, is_fp, is_lantern, is_solid_or_occupied, make_beam_block_at,
    make_frame_block_at, make_slab, make_stair, make_trapdoor, make_wall_sconce, set_uv,
    style_carpet, style_pot,
};
use crate::model::{BuildCategory, BuildStyle, MaterialPalette};
use crate::noise::SplitMix64;
use crate::parser::{BlueprintSpec, StructureTheme};

fn shelf_id_for_palette(pal: &MaterialPalette) -> &'static str {
    let key = format!("{} {} {}", pal.door, pal.fence, pal.floor);
    if key.contains("cherry") {
        "minecraft:cherry_shelf"
    } else if key.contains("birch") || key.contains("bamboo") {
        "minecraft:birch_shelf"
    } else if key.contains("pale_oak") || key.contains("quartz") || key.contains("sandstone") {
        "minecraft:pale_oak_shelf"
    } else {
        "minecraft:dark_oak_shelf"
    }
}

fn bed_id_for_style(style: BuildStyle) -> &'static str {
    match style {
        BuildStyle::Modern | BuildStyle::Japanese => "minecraft:white_bed",
        BuildStyle::Gothic => "minecraft:purple_bed",
        BuildStyle::Cottage => "minecraft:green_bed",
        BuildStyle::Nordic => "minecraft:light_blue_bed",
        BuildStyle::Desert | BuildStyle::Steampunk => "minecraft:yellow_bed",
        BuildStyle::Medieval | BuildStyle::Classical | BuildStyle::Auto => "minecraft:red_bed",
    }
}

/// Builds walkable straight stairs, spiral tower stairs, upper guardrails, and basement descent.
/// Returns `(reserved_uv_cells, stair_headroom_columns)` where each `(u, step_top_y, v)` guarantees
/// at least 3 blocks of unobstructed headroom above `step_top_y`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_staircases(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    roof_base_y: i32,
    rng: &mut SplitMix64,
) -> (Vec<(i32, i32)>, Vec<(i32, i32, i32)>) {
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let mut stairwell_uv: Vec<(i32, i32)> = Vec::new();
    let mut stair_headroom: Vec<(i32, i32, i32)> = Vec::new();
    let ground_y = i32::from(spec.foundation_height);
    let mut basement_u_opt: Option<i32> = None;

    // Pre-compute straight multi-floor staircase column so compact basement ladder shafts never collide with stair runs.
    let rand_stair_right = rng.next_f64() < 0.5;
    let place_stair_right_pre = match best_door {
        Some((du, _)) => du < (b_u0 + b_u1) / 2,
        None => rand_stair_right,
    };
    let margin_in_pre = if (b_u1 - b_u0) >= 9 { 3 } else { 2 };
    let reserved_stair_u = if spec.floors >= 2 && spec.category != BuildCategory::Tower {
        Some(if place_stair_right_pre {
            (b_u1 - margin_in_pre).max(b_u0 + 2)
        } else {
            (b_u0 + margin_in_pre).min(b_u1 - 2)
        })
    } else {
        None
    };

    // Optional basement descent: spacious walkable stone staircase on wide houses, ladder shaft on compact plots.
    if spec.basement {
        let is_inner = |u: i32, v: i32| {
            is_fp(grid, fp, u, v)
                && is_fp(grid, fp, u - 1, v)
                && is_fp(grid, fp, u + 1, v)
                && is_fp(grid, fp, u, v - 1)
                && is_fp(grid, fp, u, v + 1)
        };
        let wide_stair = if (b_u1 - b_u0) >= 7 && (b_v1 - b_v0) >= 8 {
            (b_v0 + 2..=b_v1 - 6)
                .flat_map(|v| (b_u0 + 3..=b_u1 - 3).map(move |u| (u, v)))
                .find(|&(u, v)| {
                    (0..=5).all(|step| is_inner(u, v + step))
                        && best_door.is_none_or(|(du, dv)| (u - du).abs() > 2 || v + 5 < dv)
                })
                .or_else(|| {
                    (b_v0 + 2..=b_v1 - 5)
                        .flat_map(|v| (b_u0 + 3..=b_u1 - 3).map(move |u| (u, v)))
                        .find(|&(u, v)| {
                            (0..=4).all(|step| is_inner(u, v + step))
                                && best_door.is_none_or(|(du, dv)| (u - du).abs() > 2 || v + 4 < dv)
                        })
                })
        } else {
            None
        };
        if let Some((bu, bv)) = wide_stair {
            basement_u_opt = Some(bu);
            // Keep top entry landing `(bu, bv - 1)` spacious & open.
            stairwell_uv.push((bu, bv - 1));
            stair_headroom.push((bu, ground_y, bv - 1));

            let max_s = (ground_y + 3).clamp(3, 6);
            for s in 0..=max_s {
                let sv = bv + s;
                if !is_fp(grid, fp, bu, sv) {
                    break;
                }
                let step_y = ground_y - s;
                if step_y < -3 {
                    break;
                }
                stairwell_uv.push((bu, sv));
                stair_headroom.push((bu, step_y, sv));
                set_uv(blocks, grid, bu, -4, sv, PlannedBlock::simple(pal.foundation.clone()));
                for cy in (step_y + 1)..=(ground_y + 3) {
                    set_uv(blocks, grid, bu, cy, sv, PlannedBlock::simple("minecraft:air"));
                }
                set_uv(
                    blocks,
                    grid,
                    bu,
                    step_y,
                    sv,
                    make_stair("minecraft:cobblestone_stairs", grid.dir_back(), "bottom"),
                );
            }
            let landing_v = bv + max_s + 1;
            if is_inner(bu, landing_v) {
                stairwell_uv.push((bu, landing_v));
                set_uv(blocks, grid, bu, -4, landing_v, PlannedBlock::simple(pal.foundation.clone()));
                for cy in -3..=0 {
                    set_uv(blocks, grid, bu, cy, landing_v, PlannedBlock::simple("minecraft:air"));
                }
                stair_headroom.push((bu, -4, landing_v));
            }
        } else {
            let avoids_main_stairs = |u: i32| reserved_stair_u != Some(u);
            let shaft = (b_v0 + 1..b_v1)
                .flat_map(|v| (b_u0 + 1..b_u1).map(move |u| (u, v)))
                .find(|&(u, v)| {
                    avoids_main_stairs(u)
                        && is_fp(grid, fp, u, v)
                        && is_fp(grid, fp, u, v - 1)
                        && !is_fp(grid, fp, u, v - 2)
                        && is_inner(u, v + 1)
                        && !is_solid_or_occupied(blocks, grid, u, ground_y + 1, v + 1)
                        && !is_solid_or_occupied(blocks, grid, u, ground_y + 2, v + 1)
                        && best_door.is_none_or(|(du, dv)| (u - du).abs() + (v - dv).abs() > 3)
                })
                .or_else(|| {
                    (b_v0 + 1..b_v1)
                        .flat_map(|v| (b_u0 + 1..b_u1).map(move |u| (u, v)))
                        .find(|&(u, v)| {
                            avoids_main_stairs(u)
                                && is_inner(u, v)
                                && is_inner(u, v + 1)
                                && !is_solid_or_occupied(blocks, grid, u, ground_y + 1, v + 1)
                                && !is_solid_or_occupied(blocks, grid, u, ground_y + 2, v + 1)
                                && best_door.is_none_or(|(du, dv)| (u - du).abs() + (v - dv).abs() > 3)
                        })
                })
                .or_else(|| {
                    (b_v0 + 1..b_v1)
                        .flat_map(|v| (b_u0 + 1..b_u1).map(move |u| (u, v)))
                        .find(|&(u, v)| {
                            is_inner(u, v)
                                && is_inner(u, v + 1)
                                && !is_solid_or_occupied(blocks, grid, u, ground_y + 1, v + 1)
                                && !is_solid_or_occupied(blocks, grid, u, ground_y + 2, v + 1)
                                && best_door.is_none_or(|(du, dv)| (u - du).abs() + (v - dv).abs() > 3)
                        })
                });
            if let Some((bu, bv)) = shaft {
                stairwell_uv.push((bu, bv));
                stairwell_uv.push((bu, bv + 1));
                stair_headroom.push((bu, ground_y, bv));
                stair_headroom.push((bu, ground_y, bv + 1));
                if is_inner(bu, bv - 1) {
                    // Solid masonry pier behind ladder shaft inside basement so ladder is never floating in air.
                    for py in -3..ground_y {
                        set_uv(blocks, grid, bu, py, bv - 1, PlannedBlock::simple(pal.foundation.clone()));
                    }
                }
                set_uv(
                    blocks,
                    grid,
                    bu,
                    ground_y,
                    bv,
                    make_trapdoor(&pal.trapdoor, grid.dir_front(), "top", true),
                );
                for cy in (ground_y + 1)..=(ground_y + 3) {
                    set_uv(blocks, grid, bu, cy, bv, PlannedBlock::simple("minecraft:air"));
                    if cy <= ground_y + 2 {
                        set_uv(blocks, grid, bu, cy, bv + 1, PlannedBlock::simple("minecraft:air"));
                    }
                }
                for ly in -3..ground_y {
                    set_uv(
                        blocks,
                        grid,
                        bu,
                        ly,
                        bv,
                        PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]),
                    );
                    if ly < 0 {
                        set_uv(blocks, grid, bu, ly, bv + 1, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
    }

    if spec.floors <= 1 || matches!(spec.category, BuildCategory::Pavilion | BuildCategory::Bridge) {
        return (stairwell_uv, stair_headroom);
    }

    // Tower spiral stairs with full 3x3 diagonal corner turning headroom.
    if spec.category == BuildCategory::Tower && (b_u1 - b_u0) >= 4 && (b_v1 - b_v0) >= 4 {
        let cu = (b_u0 + b_u1) / 2;
        let cv = (b_v0 + b_v1) / 2;
        let ring: [(i32, i32, i32, i32); 4] = [
            (cu + 1, cv, 0, -1),
            (cu, cv - 1, -1, 0),
            (cu - 1, cv, 0, 1),
            (cu, cv + 1, 1, 0),
        ];
        if ring.iter().all(|&(u, v, _, _)| is_fp(grid, fp, u, v)) {
            for f in 0..(i32::from(spec.floors) - 1) {
                let floor_y = ground_y + f * i32::from(spec.floor_height);
                let next_floor_y = floor_y + i32::from(spec.floor_height);
                for s in 0..i32::from(spec.floor_height) {
                    let (su, sv, fdu, fdv) = ring[(s as usize) % 4];
                    let sy = floor_y + 1 + s;
                    stairwell_uv.push((su, sv));
                    stair_headroom.push((su, sy, sv));
                    set_uv(blocks, grid, cu, sy, cv, make_frame_block_at(pal, "y", seed, cu, sy, cv));
                    set_uv(
                        blocks,
                        grid,
                        su,
                        sy,
                        sv,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, su, sy, sv), grid.uv_to_mc_dir(fdu, fdv), "bottom"),
                    );
                    for clear_y in (sy + 1)..=(sy + 3).max(next_floor_y + 2) {
                        set_uv(blocks, grid, su, clear_y, sv, PlannedBlock::simple("minecraft:air"));
                    }
                    // Open adjacent diagonal turning corner in the 3x3 ring so turns never bump shoulders.
                    for &(du_c, dv_c) in &[(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                        let diag_u = cu + du_c;
                        let diag_v = cv + dv_c;
                        if (diag_u - su).abs() + (diag_v - sv).abs() == 1 && is_fp(grid, fp, diag_u, diag_v) {
                            stairwell_uv.push((diag_u, diag_v));
                            stair_headroom.push((diag_u, floor_y, diag_v));
                            for clear_y in (floor_y + 1)..=(next_floor_y + 3) {
                                set_uv(blocks, grid, diag_u, clear_y, diag_v, PlannedBlock::simple("minecraft:air"));
                            }
                        }
                    }
                }
            }
            return (stairwell_uv, stair_headroom);
        }
    }

    // Standard multi-floor straight staircase tucked cleanly under central ridge headroom with guardrails.
    let place_stair_right = match (best_door, basement_u_opt) {
        (_, Some(bu)) => bu < (b_u0 + b_u1) / 2,
        (Some((du, _)), None) => du < (b_u0 + b_u1) / 2,
        _ => rand_stair_right,
    };
    let margin_in = if (b_u1 - b_u0) >= 9 { 3 } else { 2 };
    let stair_u = if place_stair_right {
        (b_u1 - margin_in).max(b_u0 + 2)
    } else {
        (b_u0 + margin_in).min(b_u1 - 2)
    };
    let rail_u = if place_stair_right {
        stair_u - 1
    } else {
        stair_u + 1
    };
    let stair_v0 = b_v0 + 2;
    let max_run = (b_v1 - 1 - stair_v0).max(0);

    for f in 0..(i32::from(spec.floors) - 1) {
        let floor_y = ground_y + f * i32::from(spec.floor_height);
        let next_floor_y = floor_y + i32::from(spec.floor_height);
        let steps = i32::from(spec.floor_height);
        if max_run >= steps
            && (0..=steps).all(|s| is_fp(grid, fp, stair_u, stair_v0 + s))
            && is_fp(grid, fp, stair_u, stair_v0 - 1)
        {
            stairwell_uv.push((stair_u, stair_v0 - 1));
            stair_headroom.push((stair_u, floor_y, stair_v0 - 1));
            for clear_y in (floor_y + 1)..=(floor_y + 3) {
                set_uv(blocks, grid, stair_u, clear_y, stair_v0 - 1, PlannedBlock::simple("minecraft:air"));
            }

            for s in 0..steps {
                let sv = stair_v0 + s;
                let sy = floor_y + 1 + s;
                stairwell_uv.push((stair_u, sv));
                stair_headroom.push((stair_u, sy, sv));
                set_uv(
                    blocks,
                    grid,
                    stair_u,
                    sy,
                    sv,
                    make_stair(&pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, stair_u, sy, sv), grid.dir_front(), "bottom"),
                );
                // Solid inverted stair stringer underneath steps s>=1 prevents thin zigzag gaps & ensures orthogonal support.
                if s >= 1 && sy - 1 > floor_y {
                    set_uv(
                        blocks,
                        grid,
                        stair_u,
                        sy - 1,
                        sv,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, stair_u, sy - 1, sv), grid.dir_back(), "top"),
                    );
                }
                for clear_y in (sy + 1)..=(sy + 3).max(next_floor_y + 2) {
                    set_uv(blocks, grid, stair_u, clear_y, sv, PlannedBlock::simple("minecraft:air"));
                }
                if s + 1 < steps
                    && is_fp(grid, fp, rail_u, sv)
                    && next_floor_y < roof_base_y
                    && !is_solid_or_occupied(blocks, grid, rail_u, next_floor_y + 1, sv)
                {
                    set_uv(
                        blocks,
                        grid,
                        rail_u,
                        next_floor_y + 1,
                        sv,
                        PlannedBlock::simple(pal.fence.clone()),
                    );
                }
            }
            if is_fp(grid, fp, rail_u, stair_v0)
                && !is_solid_or_occupied(blocks, grid, rail_u, floor_y + 1, stair_v0)
            {
                set_uv(
                    blocks,
                    grid,
                    rail_u,
                    floor_y + 1,
                    stair_v0,
                    PlannedBlock::simple(pal.fence.clone()),
                );
                if floor_y + 2 < next_floor_y
                    && !is_solid_or_occupied(blocks, grid, rail_u, floor_y + 2, stair_v0)
                {
                    set_uv(
                        blocks,
                        grid,
                        rail_u,
                        floor_y + 2,
                        stair_v0,
                        PlannedBlock::with_props(
                            "minecraft:candle",
                            vec![("candles", "1"), ("lit", "true"), ("waterlogged", "false")],
                        ),
                    );
                }
            }
            let exit_v = stair_v0 + steps;
            stairwell_uv.push((stair_u, exit_v));
            set_uv(blocks, grid, stair_u, next_floor_y, exit_v, PlannedBlock::simple(pal.floor.clone()));
            stair_headroom.push((stair_u, next_floor_y, exit_v));
            for clear_y in (next_floor_y + 1)..=(next_floor_y + 3) {
                set_uv(blocks, grid, stair_u, clear_y, exit_v, PlannedBlock::simple("minecraft:air"));
            }
        } else {
            let ladder_uv = (b_v0 + 1..b_v1)
                .flat_map(|v| (b_u0 + 1..b_u1).map(move |u| (u, v)))
                .find(|&(u, v)| {
                    is_fp(grid, fp, u, v)
                        && is_fp(grid, fp, u, v + 1)
                        && is_fp(grid, fp, u, v - 1)
                        && !is_fp(grid, fp, u, v - 2)
                        && !stairwell_uv.contains(&(u, v))
                        && !stairwell_uv.contains(&(u, v + 1))
                        && best_door.is_none_or(|(du, dv)| (u - du).abs() + (v - dv).abs() > 2)
                });
            if let Some((lu, lv)) = ladder_uv {
                stairwell_uv.push((lu, lv));
                stairwell_uv.push((lu, lv + 1));
                stair_headroom.push((lu, next_floor_y, lv));
                stair_headroom.push((lu, next_floor_y, lv + 1));
                for ly in (floor_y + 1)..=next_floor_y {
                    set_uv(
                        blocks,
                        grid,
                        lu,
                        ly,
                        lv,
                        PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]),
                    );
                }
                set_uv(blocks, grid, lu, next_floor_y, lv + 1, PlannedBlock::simple(pal.floor.clone()));
                for clear_y in (next_floor_y + 1)..=(next_floor_y + 3) {
                    set_uv(blocks, grid, lu, clear_y, lv, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, lu, clear_y, lv + 1, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    (stairwell_uv, stair_headroom)
}

/// Builds inward-facing stone fireplace hearth, chimney flue, and smoking campfire cap.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_fireplace_and_chimney(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    stairwell_uv: &[(i32, i32)],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    max_placed_roof_y: i32,
    height: u16,
    rng: &mut SplitMix64,
) -> Option<(i32, i32)> {
    if !spec.chimney {
        return None;
    }
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let candidates = [
        (b_u1 - 2, b_v0 + 1, 0, 1),
        (b_u0 + 2, b_v0 + 1, 0, 1),
        (b_u0 + 1, (b_v0 + b_v1) / 2, 1, 0),
        (b_u1 - 1, (b_v0 + b_v1) / 2, -1, 0),
    ];
    let idx0 = rng.range_i32(0, 3) as usize;
    let (cu, cv, idu, idv) = (0..4)
        .map(|k| candidates[(idx0 + k) % 4])
        .find(|&(cu, cv, _, _)| {
            is_fp(grid, fp, cu, cv)
                && !stairwell_uv.contains(&(cu, cv))
                && best_door.is_none_or(|(du, dv)| (cu - du).abs() + (cv - dv).abs() > 2)
        })?;

    let top_y = (max_placed_roof_y + rng.range_i32(1, 2)).min(i32::from(height) - 2);
    let base_y = i32::from(spec.foundation_height);
    for y in base_y..=top_y {
        set_uv(
            blocks,
            grid,
            cu,
            y,
            cv,
            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_CHIMNEY, seed, cu, y, cv)),
        );
    }

    // Place campfire hearth visible toward the room interior!
    let hu = if is_fp(grid, fp, cu + idu, cv + idv) {
        cu + idu
    } else {
        cu
    };
    let hv = if is_fp(grid, fp, cu + idu, cv + idv) {
        cv + idv
    } else {
        cv
    };
    let face_room = grid.uv_to_mc_dir(idu, idv);
    set_uv(
        blocks,
        grid,
        hu,
        base_y + 1,
        hv,
        PlannedBlock::with_props(
            if spec.style == BuildStyle::Gothic {
                "minecraft:soul_campfire"
            } else {
                "minecraft:campfire"
            },
            vec![("lit", "true"), ("signal_fire", "false"), ("facing", face_room)],
        ),
    );
    set_uv(
        blocks,
        grid,
        hu,
        base_y + 2,
        hv,
        make_stair(&pal.roof_stairs, grid.uv_to_mc_dir(-idu, -idv), "top"),
    );
    if is_fp(grid, fp, cu, cv) && base_y + 3 < i32::from(height) {
        set_uv(
            blocks,
            grid,
            cu,
            base_y + 3,
            cv,
            make_trapdoor(&pal.trapdoor, face_room, "bottom", true),
        );
    }

    if top_y + 1 < i32::from(height) {
        // Hay block tucked inside trapdoor collar boosts rising chimney smoke plume!
        set_uv(blocks, grid, cu, top_y, cv, PlannedBlock::simple("minecraft:hay_block"));
        set_uv(
            blocks,
            grid,
            cu,
            top_y + 1,
            cv,
            PlannedBlock::with_props(
                "minecraft:campfire",
                vec![("lit", "true"), ("signal_fire", "true"), ("facing", grid.dir_front())],
            ),
        );
        for (tdu, tdv) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            if grid.in_bounds(cu + tdu, cv + tdv) {
                set_uv(
                    blocks,
                    grid,
                    cu + tdu,
                    top_y + 1,
                    cv + tdv,
                    make_trapdoor(&pal.trapdoor, grid.uv_to_mc_dir(tdu, tdv), "bottom", true),
                );
            }
        }
    }
    Some((cu, cv))
}

/// Generates ceiling chandeliers, arched room partitions, themed furniture zones, and bedrooms.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_interior_zoning(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    tall_fp_opt: Option<&[bool]>,
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    stairwell_uv: &[(i32, i32)],
    chimney_uv: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    rng: &mut SplitMix64,
) {
    if !spec.furnished
        || spec.ruin_erosion >= 0.1
        || (spec.category == BuildCategory::Pavilion && spec.theme != StructureTheme::Greenhouse)
    {
        return;
    }

    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let interior_cells: BTreeSet<(i32, i32)> = (b_v0 + 1..b_v1)
        .flat_map(|v| (b_u0 + 1..b_u1).map(move |u| (u, v)))
        .filter(|&(u, v)| {
            is_fp(grid, fp, u, v)
                && is_fp(grid, fp, u - 1, v)
                && is_fp(grid, fp, u + 1, v)
                && is_fp(grid, fp, u, v - 1)
                && is_fp(grid, fp, u, v + 1)
        })
        .collect();
    let upper_fp = tall_fp_opt.unwrap_or(fp);
    let upper_cells: BTreeSet<(i32, i32)> = (0..grid.d)
        .flat_map(|v| (0..grid.w).map(move |u| (u, v)))
        .filter(|&(u, v)| {
            is_fp(grid, upper_fp, u, v)
                && is_fp(grid, upper_fp, u - 1, v)
                && is_fp(grid, upper_fp, u + 1, v)
                && is_fp(grid, upper_fp, u, v - 1)
                && is_fp(grid, upper_fp, u, v + 1)
        })
        .collect();

    let is_clear_prop_spot = |u: i32, v: i32| -> bool {
        if !interior_cells.contains(&(u, v)) {
            return false;
        }
        if stairwell_uv.contains(&(u, v))
            || stairwell_uv
                .iter()
                .any(|&(su, sv)| (su - u).abs() <= 1 && (sv - v).abs() <= 1)
        {
            return false;
        }
        if chimney_uv == Some((u, v)) {
            return false;
        }
        if let Some((du, dv)) = best_door
            && (du - u).abs() <= 2
            && (dv - v).abs() <= 2
        {
            return false;
        }
        true
    };

    let beam_axis = if matches!(grid.uv_to_mc_dir(1, 0), "east" | "west") {
        "x"
    } else {
        "z"
    };
    let effective_sconce = if !pal.light_accent.is_empty() {
        pal.light_accent.as_str()
    } else if matches!(spec.style, BuildStyle::Gothic | BuildStyle::Japanese) {
        "minecraft:soul_torch"
    } else {
        "minecraft:torch"
    };

    // Chandeliers on `iron_chain`, exposed ceiling rafters, and warm perimeter wall sconces.
    for f in 0..i32::from(spec.floors) {
        let floor_cells = if f == 0 { &interior_cells } else { &upper_cells };
        let floor_fp = if f == 0 { fp } else { upper_fp };
        let floor_y = i32::from(spec.foundation_height) + f * i32::from(spec.floor_height);
        let ceil_y = floor_y + i32::from(spec.floor_height) - 1;
        let cu = (b_u0 + b_u1) / 2;
        let cv = (b_v0 + b_v1) / 2;
        if floor_cells.contains(&(cu, cv)) && ceil_y > floor_y + 2 {
            let light_id = pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, ceil_y, cv);
            if spec.floor_height >= 5 && is_lantern(&light_id) {
                set_uv(
                    blocks,
                    grid,
                    cu,
                    ceil_y,
                    cv,
                    PlannedBlock::with_props("minecraft:iron_chain", vec![("axis", "y")]),
                );
                set_uv(
                    blocks,
                    grid,
                    cu,
                    ceil_y - 1,
                    cv,
                    PlannedBlock::with_props(light_id, vec![("hanging", "true")]),
                );
            } else {
                let light_blk = if is_lantern(&light_id) {
                    PlannedBlock::with_props(light_id, vec![("hanging", "true")])
                } else {
                    PlannedBlock::simple(light_id)
                };
                set_uv(blocks, grid, cu, ceil_y, cv, light_blk);
            }
        }

        if spec.floor_height >= 5
            && !matches!(spec.style, BuildStyle::Modern | BuildStyle::Desert)
            && spec.theme != StructureTheme::Greenhouse
            && ceil_y > floor_y + 3
        {
            for rv in (b_v0 + 2)..=(b_v1 - 2) {
                if (rv - b_v0) % 2 != 0 {
                    continue;
                }
                for ru in (b_u0 + 1)..b_u1 {
                    if floor_cells.contains(&(ru, rv))
                        && !(ru == cu && rv == cv)
                        && !stairwell_uv.contains(&(ru, rv))
                        && !is_solid_or_occupied(blocks, grid, ru, ceil_y, rv)
                    {
                        set_uv(blocks, grid, ru, ceil_y, rv, make_beam_block_at(pal, beam_axis, seed, ru, ceil_y, rv));
                    }
                }
            }
        }

        let sconce_y = (floor_y + 3).min(ceil_y - 1);
        if sconce_y > floor_y + 1 {
            let mut placed_sconces = 0;
            for &(u, v) in floor_cells {
                if placed_sconces >= 4 || (u + v) % 4 != 0 || stairwell_uv.contains(&(u, v)) {
                    continue;
                }
                if !is_solid_or_occupied(blocks, grid, u, floor_y, v)
                    || is_solid_or_occupied(blocks, grid, u, sconce_y, v)
                {
                    continue;
                }
                let candidates = [
                    (u, v - 1, grid.dir_front()),
                    (u - 1, v, grid.dir_right()),
                    (u + 1, v, grid.dir_left()),
                ];
                for (wu, wv, face) in candidates {
                    if !floor_cells.contains(&(wu, wv)) && is_fp(grid, floor_fp, wu, wv) {
                        let solid_wall = get_uv(blocks, grid, wu, sconce_y, wv).is_some_and(|b| {
                            let bid = b.block_id.as_str();
                            bid != "minecraft:air"
                                && !bid.contains("glass")
                                && !bid.ends_with("_door")
                                && !bid.ends_with("_trapdoor")
                                && !bid.ends_with("_stairs")
                                && !bid.ends_with("_slab")
                                && !bid.ends_with("_fence")
                                && bid != "minecraft:iron_bars"
                        });
                        if solid_wall {
                            let sconce_id = if !pal.light_accent.is_empty() {
                                pal.pick(MaterialPalette::SLOT_LIGHT_ACCENT, seed, u, sconce_y, v)
                            } else {
                                effective_sconce.to_owned()
                            };
                            set_uv(
                                blocks,
                                grid,
                                u,
                                sconce_y,
                                v,
                                make_wall_sconce(&sconce_id, face),
                            );
                            placed_sconces += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    let ground_y = i32::from(spec.foundation_height);

    // Room partition walls on ground floor (2-3 rooms with open interior doors!) cleanly avoiding stairs & exterior doors.
    let mut split_u_opt = None;
    let mut split_v_opt = None;
    let mut door_clearance_uv: Vec<(i32, i32)> = Vec::new();
    let check_partition_safe = |u: i32, v: i32| -> bool {
        !stairwell_uv
            .iter()
            .any(|&(su, sv)| (su - u).abs() <= 1 && (sv - v).abs() <= 1)
            && best_door.is_none_or(|(du, dv)| (du - u).abs() > 2 || (dv - v).abs() > 2)
    };
    let is_walk_clear = |u: i32, v: i32| -> bool {
        !stairwell_uv.contains(&(u, v))
            && chimney_uv != Some((u, v))
            && best_door != Some((u, v))
    };
    let find_partition_door_v = |blocks: &BTreeMap<[i32; 3], PlannedBlock>,
                                 u: i32,
                                 v_lo: i32,
                                 v_hi: i32,
                                 pref_v: i32,
                                 floor_y: i32,
                                 cells: &BTreeSet<(i32, i32)>|
     -> Option<i32> {
        let mut candidates = vec![pref_v];
        for delta in 1..=((v_hi - v_lo) / 2 + 2) {
            candidates.push(pref_v - delta);
            candidates.push(pref_v + delta);
        }
        candidates.into_iter().find(|&dv| {
            dv > v_lo
                && dv < v_hi
                && cells.contains(&(u, dv - 1))
                && cells.contains(&(u, dv))
                && cells.contains(&(u, dv + 1))
                && cells.contains(&(u - 1, dv))
                && cells.contains(&(u + 1, dv))
                && is_solid_or_occupied(blocks, grid, u - 1, floor_y, dv)
                && is_solid_or_occupied(blocks, grid, u + 1, floor_y, dv)
                && chimney_uv != Some((u, dv - 1))
                && chimney_uv != Some((u, dv))
                && chimney_uv != Some((u, dv + 1))
                && check_partition_safe(u, dv - 1)
                && check_partition_safe(u, dv)
                && check_partition_safe(u, dv + 1)
                && is_walk_clear(u - 1, dv)
                && is_walk_clear(u + 1, dv)
        })
    };
    let find_partition_door_u = |blocks: &BTreeMap<[i32; 3], PlannedBlock>,
                                 v: i32,
                                 u_lo: i32,
                                 u_hi: i32,
                                 pref_u: i32,
                                 floor_y: i32,
                                 cells: &BTreeSet<(i32, i32)>|
     -> Option<i32> {
        let mut candidates = vec![pref_u];
        for delta in 1..=((u_hi - u_lo) / 2 + 2) {
            candidates.push(pref_u - delta);
            candidates.push(pref_u + delta);
        }
        candidates.into_iter().find(|&du| {
            du > u_lo
                && du < u_hi
                && cells.contains(&(du - 1, v))
                && cells.contains(&(du, v))
                && cells.contains(&(du + 1, v))
                && cells.contains(&(du, v - 1))
                && cells.contains(&(du, v + 1))
                && is_solid_or_occupied(blocks, grid, du, floor_y, v - 1)
                && is_solid_or_occupied(blocks, grid, du, floor_y, v + 1)
                && chimney_uv != Some((du - 1, v))
                && chimney_uv != Some((du, v))
                && chimney_uv != Some((du + 1, v))
                && check_partition_safe(du - 1, v)
                && check_partition_safe(du, v)
                && check_partition_safe(du + 1, v)
                && is_walk_clear(du, v - 1)
                && is_walk_clear(du, v + 1)
        })
    };

    if spec.room_partitions && (b_u1 - b_u0) >= 9 && (b_v1 - b_v0) >= 6 {
        let base_u = ((b_u0 + b_u1) / 2 + rng.range_i32(-1, 1)).clamp(b_u0 + 3, b_u1 - 3);
        let pref_v = (b_v0 + b_v1) / 2;
        if let Some((split_u, arch_v)) = [base_u, base_u - 1, base_u + 1, base_u - 2, base_u + 2]
            .into_iter()
            .filter(|&u| {
                u >= b_u0 + 3
                    && u <= b_u1 - 3
                    && !stairwell_uv.iter().any(|&(su, _)| (su - u).abs() <= 1)
            })
            .find_map(|u| {
                find_partition_door_v(blocks, u, b_v0 + 1, b_v1 - 1, pref_v, ground_y, &interior_cells)
                    .map(|dv| (u, dv))
            })
        {
            split_u_opt = Some(split_u);
            for dv_cl in -1..=1 {
                door_clearance_uv.push((split_u, arch_v + dv_cl));
            }
            door_clearance_uv.push((split_u - 1, arch_v));
            door_clearance_uv.push((split_u + 1, arch_v));
            for v in (b_v0 + 1)..b_v1 {
                let is_jamb = (v - arch_v).abs() == 1;
                if v == arch_v {
                    let idoor = pal.pick(MaterialPalette::SLOT_INTERIOR_DOOR, seed, split_u, ground_y + 1, v);
                    set_uv(
                        blocks,
                        grid,
                        split_u,
                        ground_y + 1,
                        v,
                        PlannedBlock::with_props(
                            idoor.clone(),
                            vec![
                                ("facing", grid.dir_right()),
                                ("half", "lower"),
                                ("hinge", "left"),
                                ("open", "true"),
                            ],
                        ),
                    );
                    set_uv(
                        blocks,
                        grid,
                        split_u,
                        ground_y + 2,
                        v,
                        PlannedBlock::with_props(
                            idoor,
                            vec![
                                ("facing", grid.dir_right()),
                                ("half", "upper"),
                                ("hinge", "left"),
                                ("open", "true"),
                            ],
                        ),
                    );
                    for pass_u in [split_u - 1, split_u + 1] {
                        for dy in 1..=2 {
                            set_uv(
                                blocks,
                                grid,
                                pass_u,
                                ground_y + dy,
                                v,
                                PlannedBlock::simple("minecraft:air"),
                            );
                        }
                    }
                    for y in (ground_y + 3)..(ground_y + i32::from(spec.floor_height)) {
                        let b = if y == ground_y + 3 {
                            make_frame_block_at(pal, grid.axis_v(), seed, split_u, y, v)
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, split_u, y, v))
                        };
                        set_uv(blocks, grid, split_u, y, v, b);
                    }
                } else if is_jamb || is_clear_prop_spot(split_u, v) {
                    for y in (ground_y + 1)..(ground_y + i32::from(spec.floor_height)) {
                        set_uv(blocks, grid, split_u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, split_u, y, v)));
                    }
                }
            }

            // Secondary perpendicular partition dividing larger side wing into a 3rd distinct room.
            if (b_v1 - b_v0) >= 10 && (b_u1 - b_u0) >= 11 {
                let sv = (b_v0 + b_v1) / 2;
                let left_room = b_u0 + 1..split_u;
                let right_room = (split_u + 1)..b_u1;
                let side_range = if best_door.is_some_and(|(du, _)| du < split_u) {
                    right_room
                } else {
                    left_room
                };
                let can_cross = side_range.clone().all(|u| {
                    interior_cells.contains(&(u, sv)) && check_partition_safe(u, sv)
                });
                let pref_du = (side_range.start + side_range.end) / 2;
                if can_cross
                    && let Some(door_u) = find_partition_door_u(
                        blocks,
                        sv,
                        side_range.start,
                        side_range.end - 1,
                        pref_du,
                        ground_y,
                        &interior_cells,
                    )
                {
                    split_v_opt = Some(sv);
                    for du_cl in -1..=1 {
                        door_clearance_uv.push((door_u + du_cl, sv));
                    }
                    door_clearance_uv.push((door_u, sv - 1));
                    door_clearance_uv.push((door_u, sv + 1));
                    for u in side_range {
                        let is_jamb = (u - door_u).abs() == 1;
                        if u == door_u {
                            let idoor = pal.pick(MaterialPalette::SLOT_INTERIOR_DOOR, seed, u, ground_y + 1, sv);
                            set_uv(
                                blocks,
                                grid,
                                u,
                                ground_y + 1,
                                sv,
                                PlannedBlock::with_props(
                                    idoor.clone(),
                                    vec![
                                        ("facing", grid.dir_front()),
                                        ("half", "lower"),
                                        ("hinge", "left"),
                                        ("open", "true"),
                                    ],
                                ),
                            );
                            set_uv(
                                blocks,
                                grid,
                                u,
                                ground_y + 2,
                                sv,
                                PlannedBlock::with_props(
                                    idoor,
                                    vec![
                                        ("facing", grid.dir_front()),
                                        ("half", "upper"),
                                        ("hinge", "left"),
                                        ("open", "true"),
                                    ],
                                ),
                            );
                            for pass_v in [sv - 1, sv + 1] {
                                for dy in 1..=2 {
                                    set_uv(
                                        blocks,
                                        grid,
                                        u,
                                        ground_y + dy,
                                        pass_v,
                                        PlannedBlock::simple("minecraft:air"),
                                    );
                                }
                            }
                            for y in (ground_y + 3)..(ground_y + i32::from(spec.floor_height)) {
                                set_uv(blocks, grid, u, y, sv, make_frame_block_at(pal, grid.axis_u(), seed, u, y, sv));
                            }
                        } else if is_jamb || is_clear_prop_spot(u, sv) {
                            for y in (ground_y + 1)..(ground_y + i32::from(spec.floor_height)) {
                                set_uv(blocks, grid, u, y, sv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, sv)));
                            }
                        }
                    }
                }
            }
        }
    }

    // Greenhouse indoor botanical garden beds + central fountain pool!
    if spec.theme == StructureTheme::Greenhouse {
        let gcu = (b_u0 + b_u1) / 2;
        let gcv = (b_v0 + b_v1) / 2;
        if is_clear_prop_spot(gcu, gcv) {
            set_uv(blocks, grid, gcu, ground_y, gcv, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
            set_uv(blocks, grid, gcu, ground_y + 1, gcv, PlannedBlock::simple("minecraft:lily_pad"));
        }
        for &(u, v) in &interior_cells {
            if !is_clear_prop_spot(u, v) || (u == gcu && v == gcv) {
                continue;
            }
            if (u + v) % 3 == 0 {
                set_uv(blocks, grid, u, ground_y + 1, v, PlannedBlock::simple("minecraft:moss_block"));
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 2,
                    v,
                    PlannedBlock::simple(style_pot(spec.style)),
                );
            }
        }
        return;
    }

    // Center living rug / Japanese woven tatami mats & cozy hearth/tavern zones!
    let carpet_id = if !pal.carpet.is_empty() {
        pal.carpet.as_str()
    } else {
        style_carpet(spec.style)
    };
    let cu = (b_u0 + b_u1) / 2;
    let cv = (b_v0 + b_v1) / 2;
    let rug_rad = if spec.style == BuildStyle::Japanese { 2 } else { 1 };
    for dv in -rug_rad..=rug_rad {
        for du in -rug_rad..=rug_rad {
            let u = cu + du;
            let v = cv + dv;
            if is_clear_prop_spot(u, v)
                && split_u_opt != Some(u)
                && split_v_opt != Some(v)
                && !is_solid_or_occupied(blocks, grid, u, ground_y + 1, v)
            {
                let rug = if spec.style == BuildStyle::Japanese && !pal.has_extra(MaterialPalette::SLOT_CARPET) {
                    if (u + v) % 2 == 0 {
                        "minecraft:lime_carpet".to_owned()
                    } else {
                        carpet_id.to_owned()
                    }
                } else {
                    pal.pick(MaterialPalette::SLOT_CARPET, seed, u, ground_y + 1, v)
                };
                set_uv(blocks, grid, u, ground_y + 1, v, PlannedBlock::simple(rug));
            }
        }
    }

    // Temple altar & pews lining nave aisle | Forge workshop hearth | House & Tavern banquet dining scene.
    if spec.category == BuildCategory::Temple {
        if (b_v1 - b_v0) >= 7
            && is_clear_prop_spot(cu, b_v0 + 2)
            && !is_solid_or_occupied(blocks, grid, cu, ground_y + 1, b_v0 + 2)
        {
            for du in -1..=1 {
                if is_clear_prop_spot(cu + du, b_v0 + 2) {
                    set_uv(
                        blocks,
                        grid,
                        cu + du,
                        ground_y + 1,
                        b_v0 + 2,
                        make_slab(&pal.roof_slab, "bottom"),
                    );
                    if du.abs() == 1 {
                        set_uv(
                            blocks,
                            grid,
                            cu + du,
                            ground_y + 2,
                            b_v0 + 2,
                            PlannedBlock::with_props(
                                "minecraft:candle",
                                vec![("candles", "3"), ("lit", "true"), ("waterlogged", "false")],
                            ),
                        );
                    }
                }
            }
            let altar_blk = if (b_u1 - b_u0) >= 9 {
                "minecraft:beacon"
            } else {
                "minecraft:enchanting_table"
            };
            set_uv(blocks, grid, cu, ground_y + 2, b_v0 + 2, PlannedBlock::simple(altar_blk));
        }
        for pv in (b_v0 + 4)..=(b_v1 - 3) {
            if (pv - (b_v0 + 4)) % 2 != 0 {
                continue;
            }
            for &side_u in &[cu - 2, cu + 2] {
                if is_clear_prop_spot(side_u, pv) {
                    set_uv(
                        blocks,
                        grid,
                        side_u,
                        ground_y + 1,
                        pv,
                        make_stair(&pal.roof_stairs, grid.dir_back(), "bottom"),
                    );
                }
            }
        }
    } else if spec.category == BuildCategory::Forge
        && is_clear_prop_spot(cu + 2, cv)
        && !is_solid_or_occupied(blocks, grid, cu + 2, ground_y + 2, cv)
    {
        set_uv(blocks, grid, cu + 2, ground_y + 1, cv, PlannedBlock::simple("minecraft:magma_block"));
        set_uv(blocks, grid, cu + 2, ground_y + 2, cv, PlannedBlock::simple("minecraft:iron_bars"));
        if is_clear_prop_spot(cu + 2, cv + 1) {
            set_uv(blocks, grid, cu + 2, ground_y + 1, cv + 1, PlannedBlock::simple("minecraft:lava_cauldron"));
        }
        if is_clear_prop_spot(cu + 1, cv + 1) {
            set_uv(
                blocks,
                grid,
                cu + 1,
                ground_y + 1,
                cv + 1,
                PlannedBlock::with_props("minecraft:stonecutter", vec![("facing", grid.dir_front())]),
            );
        }
        if is_clear_prop_spot(cu + 1, cv) && !is_solid_or_occupied(blocks, grid, cu + 1, ground_y + 1, cv) {
            set_uv(blocks, grid, cu + 1, ground_y + 1, cv, PlannedBlock::simple("minecraft:smithing_table"));
        }
        if is_clear_prop_spot(cu + 1, cv - 1) && !is_solid_or_occupied(blocks, grid, cu + 1, ground_y + 1, cv - 1) {
            set_uv(
                blocks,
                grid,
                cu + 1,
                ground_y + 1,
                cv - 1,
                PlannedBlock::with_props("minecraft:grindstone", vec![("face", "floor"), ("facing", grid.dir_front())]),
            );
        }
    } else if matches!(spec.category, BuildCategory::House | BuildCategory::Castle) {
        let mut table_center: Option<(i32, i32)> = None;
        for dv_try in [2, -2, 1, -1, 0] {
            for du_try in [-2, 2, -3, 3, 0] {
                let tu = cu + du_try;
                let tv = cv + dv_try;
                if tu <= b_u0 + 1 || tu >= b_u1 - 1 || tv <= b_v0 + 1 || tv >= b_v1 - 1 {
                    continue;
                }
                let room_ok = (-1..=1).all(|du| {
                    let xu = tu + du;
                    is_clear_prop_spot(xu, tv)
                        && split_u_opt != Some(xu)
                        && split_v_opt != Some(tv)
                        && !door_clearance_uv.contains(&(xu, tv))
                        && (-1..=1).all(|dv| is_clear_prop_spot(xu, tv + dv))
                });
                if room_ok {
                    table_center = Some((tu, tv));
                    break;
                }
            }
            if table_center.is_some() {
                break;
            }
        }
        if let Some((tu, tv)) = table_center {
            // 3-wide feast dining table (`fence` legs + `top` slab deck + cake + goblet pickles + chairs).
            for (k, du) in (-1..=1).enumerate() {
                let u = tu + du;
                if k == 1 {
                    set_uv(blocks, grid, u, ground_y + 1, tv, make_slab(&pal.roof_slab, "top"));
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ground_y + 2,
                        tv,
                        PlannedBlock::with_props("minecraft:cake", vec![("bites", "0")]),
                    );
                } else {
                    set_uv(blocks, grid, u, ground_y + 1, tv, PlannedBlock::simple(pal.fence.clone()));
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ground_y + 2,
                        tv,
                        if k == 0 {
                            PlannedBlock::with_props(
                                "minecraft:sea_pickle",
                                vec![("pickles", "2"), ("waterlogged", "false")],
                            )
                        } else {
                            PlannedBlock::with_props(
                                "minecraft:candle",
                                vec![("candles", "2"), ("lit", "true"), ("waterlogged", "false")],
                            )
                        },
                    );
                }
                if !door_clearance_uv.contains(&(u, tv - 1)) && split_v_opt != Some(tv - 1) {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ground_y + 1,
                        tv - 1,
                        make_stair(&pal.roof_stairs, grid.dir_front(), "bottom"),
                    );
                }
                if !door_clearance_uv.contains(&(u, tv + 1)) && split_v_opt != Some(tv + 1) && k != 1 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ground_y + 1,
                        tv + 1,
                        make_stair(&pal.roof_stairs, grid.dir_back(), "bottom"),
                    );
                }
            }
        } else if is_clear_prop_spot(cu, cv)
            && split_u_opt != Some(cu)
            && is_clear_prop_spot(cu - 1, cv)
            && is_clear_prop_spot(cu + 1, cv)
        {
            set_uv(blocks, grid, cu, ground_y + 1, cv, PlannedBlock::simple(pal.fence.clone()));
            set_uv(
                blocks,
                grid,
                cu,
                ground_y + 2,
                cv,
                PlannedBlock::with_props("minecraft:cake", vec![("bites", "0")]),
            );
            set_uv(
                blocks,
                grid,
                cu - 1,
                ground_y + 1,
                cv,
                make_stair(&pal.roof_stairs, grid.dir_right(), "bottom"),
            );
            set_uv(
                blocks,
                grid,
                cu + 1,
                ground_y + 1,
                cv,
                make_stair(&pal.roof_stairs, grid.dir_left(), "bottom"),
            );
        }

        // Cozy music nook beside hearth/living corner if clear.
        let juke_u = b_u0 + 2;
        let juke_v = b_v1 - 2;
        if juke_u < b_u1 - 1
            && is_clear_prop_spot(juke_u, juke_v)
            && split_u_opt != Some(juke_u)
            && !is_solid_or_occupied(blocks, grid, juke_u, ground_y + 1, juke_v)
        {
            set_uv(
                blocks,
                grid,
                juke_u,
                ground_y + 1,
                juke_v,
                PlannedBlock::with_props("minecraft:jukebox", vec![("has_record", "false")]),
            );
        }
    }

    if spec.theme == StructureTheme::Tavern {
        let best_bar_row = [b_v0 + 2, b_v1 - 3, b_v0 + 3]
            .into_iter()
            .filter(|&v| v > b_v0 + 1 && v < b_v1 - 1 && split_v_opt != Some(v))
            .max_by_key(|&v| {
                ((b_u0 + 2)..=(b_u1 - 2))
                    .filter(|&u| {
                        is_clear_prop_spot(u, v)
                            && split_u_opt != Some(u)
                            && !door_clearance_uv.contains(&(u, v))
                    })
                    .count()
            });
        if let Some(bar_v) = best_bar_row {
            let mut bar_slot = 0usize;
            for bu in (b_u0 + 2)..=(b_u1 - 2) {
                if bar_slot >= 5 {
                    break;
                }
                if !is_clear_prop_spot(bu, bar_v)
                    || split_u_opt == Some(bu)
                    || door_clearance_uv.contains(&(bu, bar_v))
                {
                    continue;
                }
                set_uv(blocks, grid, bu, ground_y + 1, bar_v, make_slab(&pal.roof_slab, "top"));
                match bar_slot % 3 {
                    0 => set_uv(
                        blocks,
                        grid,
                        bu,
                        ground_y + 2,
                        bar_v,
                        PlannedBlock::with_props(
                            "minecraft:sea_pickle",
                            vec![("pickles", "2"), ("waterlogged", "false")],
                        ),
                    ),
                    1 => set_uv(
                        blocks,
                        grid,
                        bu,
                        ground_y + 2,
                        bar_v,
                        PlannedBlock::with_props("minecraft:cake", vec![("bites", "0")]),
                    ),
                    _ => {}
                }
                if is_clear_prop_spot(bu, bar_v + 1)
                    && !door_clearance_uv.contains(&(bu, bar_v + 1))
                    && split_v_opt != Some(bar_v + 1)
                    && bar_slot % 2 == 0
                    && !is_solid_or_occupied(blocks, grid, bu, ground_y + 1, bar_v + 1)
                {
                    set_uv(blocks, grid, bu, ground_y + 1, bar_v + 1, PlannedBlock::simple(pal.fence.clone()));
                }
                if is_clear_prop_spot(bu, bar_v - 1)
                    && !door_clearance_uv.contains(&(bu, bar_v - 1))
                    && split_v_opt != Some(bar_v - 1)
                    && bar_slot % 2 == 0
                {
                    set_uv(
                        blocks,
                        grid,
                        bu,
                        ground_y + 1,
                        bar_v - 1,
                        PlannedBlock::with_props("minecraft:barrel", vec![("facing", grid.dir_front())]),
                    );
                    if spec.floor_height >= 4 {
                        set_uv(
                            blocks,
                            grid,
                            bu,
                            ground_y + 2,
                            bar_v - 1,
                            PlannedBlock::with_props("minecraft:barrel", vec![("facing", grid.dir_front())]),
                        );
                    }
                }
                bar_slot += 1;
            }
        }
    }

    // Wall-backed zoned furnishings on Ground Floor.
    let mut wall_spots: Vec<(i32, i32, &'static str)> = Vec::new();
    for &(u, v) in &interior_cells {
        if !is_clear_prop_spot(u, v)
            || split_u_opt == Some(u)
            || split_v_opt == Some(v)
            || door_clearance_uv.contains(&(u, v))
        {
            continue;
        }
        if !interior_cells.contains(&(u, v - 1)) && is_fp(grid, fp, u, v - 1) {
            wall_spots.push((u, v, grid.dir_front()));
        } else if !interior_cells.contains(&(u - 1, v)) && is_fp(grid, fp, u - 1, v) {
            wall_spots.push((u, v, grid.dir_right()));
        } else if !interior_cells.contains(&(u + 1, v)) && is_fp(grid, fp, u + 1, v) {
            wall_spots.push((u, v, grid.dir_left()));
        }
    }

    let prop_set: &[(&str, bool)] = match (spec.theme, spec.category) {
        (StructureTheme::Library, _) => &[
            ("minecraft:bookshelf", false),
            ("minecraft:chiseled_bookshelf", true),
            ("minecraft:lectern", true),
            ("minecraft:enchanting_table", false),
            ("minecraft:bookshelf", false),
            ("minecraft:chest", true),
        ],
        (StructureTheme::Tavern, _) => &[
            ("minecraft:barrel", true),
            ("minecraft:smoker", true),
            ("minecraft:water_cauldron", false),
            ("minecraft:crafting_table", false),
            ("minecraft:chest", true),
            ("minecraft:barrel", true),
        ],
        (_, BuildCategory::Forge) => &[
            ("minecraft:blast_furnace", true),
            ("minecraft:anvil", true),
            ("minecraft:water_cauldron", false),
            ("minecraft:barrel", true),
            ("minecraft:chest", true),
            ("minecraft:crafting_table", false),
        ],
        (_, BuildCategory::Barn) => &[
            ("minecraft:hay_block", false),
            ("minecraft:barrel", true),
            ("minecraft:composter", false),
            ("minecraft:hay_block", false),
            ("minecraft:chest", true),
        ],
        (_, BuildCategory::Temple) => &[
            ("minecraft:lectern", true),
            ("minecraft:enchanting_table", false),
            ("minecraft:bookshelf", false),
            ("minecraft:decorated_pot", true),
        ],
        (_, BuildCategory::Castle) => &[
            ("minecraft:lectern", true),
            ("minecraft:chest", true),
            ("minecraft:chiseled_bookshelf", true),
            ("minecraft:anvil", true),
            ("minecraft:barrel", true),
        ],
        _ => &[
            ("minecraft:crafting_table", false),
            ("minecraft:furnace", true),
            ("minecraft:smoker", true),
            ("minecraft:barrel", true),
            ("minecraft:bookshelf", false),
            ("minecraft:chest", true),
        ],
    };

    for (i, &(wu, wv, face)) in wall_spots.iter().enumerate() {
        if i % 2 != 0 || i / 2 >= prop_set.len() {
            continue;
        }
        let (blk_id, needs_facing) = prop_set[i / 2];
        let props = match blk_id {
            "minecraft:lectern" => vec![("facing", face), ("has_book", "true")],
            "minecraft:chest" => vec![("facing", face), ("type", "single")],
            "minecraft:barrel" => vec![("facing", "up")],
            "minecraft:water_cauldron" => vec![("level", "3")],
            "minecraft:furnace"
            | "minecraft:smoker"
            | "minecraft:blast_furnace"
            | "minecraft:anvil"
            | "minecraft:decorated_pot"
            | "minecraft:chiseled_bookshelf" => vec![("facing", face)],
            _ if needs_facing => vec![("facing", face)],
            _ => vec![],
        };
        set_uv(
            blocks,
            grid,
            wu,
            ground_y + 1,
            wv,
            PlannedBlock::with_props(blk_id, props),
        );
        if (blk_id == "minecraft:bookshelf" || blk_id == "minecraft:chiseled_bookshelf")
            && spec.floor_height >= 4
        {
            set_uv(
                blocks,
                grid,
                wu,
                ground_y + 2,
                wv,
                PlannedBlock::simple("minecraft:bookshelf"),
            );
            if spec.theme == StructureTheme::Library && spec.floor_height >= 5 {
                set_uv(
                    blocks,
                    grid,
                    wu,
                    ground_y + 3,
                    wv,
                    PlannedBlock::simple("minecraft:bookshelf"),
                );
            }
        }
    }

    let roof_base_y =
        i32::from(spec.foundation_height) + i32::from(spec.floors) * i32::from(spec.floor_height);
    let shelf_id = shelf_id_for_palette(pal);

    // Optional 2nd-floor partition dividing Master Bedroom from Study/Guest suite on spacious houses/towers.
    let mut upper_split_u_opt: Option<i32> = None;
    let mut upper_door_clearance: Vec<(i32, i32)> = Vec::new();
    if spec.floors >= 2
        && spec.room_partitions
        && !matches!(spec.category, BuildCategory::Castle | BuildCategory::Temple)
        && upper_cells.len() >= 20
        && (b_u1 - b_u0) >= 9
    {
        let upper_y = ground_y + i32::from(spec.floor_height);
        let upper_ceil = (upper_y + i32::from(spec.floor_height) - 1).min(roof_base_y);
        let cand_split = [cu, cu + 1, cu - 1, cu + 2, cu - 2]
            .into_iter()
            .filter(|&u| {
                u >= b_u0 + 3
                    && u <= b_u1 - 3
                    && !stairwell_uv.iter().any(|&(su, _)| (su - u).abs() <= 1)
                    && chimney_uv.is_none_or(|(chu, _)| (chu - u).abs() > 1)
            })
            .find_map(|u| {
                find_partition_door_v(blocks, u, b_v0 + 1, b_v1 - 1, cv, upper_y, &upper_cells)
                    .filter(|&dv| {
                        is_solid_or_occupied(blocks, grid, u, upper_y, dv - 1)
                            && is_solid_or_occupied(blocks, grid, u, upper_y, dv)
                            && is_solid_or_occupied(blocks, grid, u, upper_y, dv + 1)
                    })
                    .map(|dv| (u, dv))
            });
        if let Some((us_u, arch_v)) = cand_split
            && upper_y + 2 < upper_ceil
        {
            upper_split_u_opt = Some(us_u);
            for dv_cl in -1..=1 {
                upper_door_clearance.push((us_u, arch_v + dv_cl));
            }
            upper_door_clearance.push((us_u - 1, arch_v));
            upper_door_clearance.push((us_u + 1, arch_v));

            for v in (b_v0 + 1)..b_v1 {
                let is_jamb = (v - arch_v).abs() == 1;
                if v != arch_v
                    && !is_jamb
                    && (!upper_cells.contains(&(us_u, v))
                        || stairwell_uv
                            .iter()
                            .any(|&(su, sv)| (su - us_u).abs() <= 1 && (sv - v).abs() <= 1)
                        || chimney_uv == Some((us_u, v))
                        || !is_solid_or_occupied(blocks, grid, us_u, upper_y, v))
                {
                    continue;
                }
                if v == arch_v {
                    let idoor = pal.pick(MaterialPalette::SLOT_INTERIOR_DOOR, seed, us_u, upper_y + 1, v);
                    set_uv(
                        blocks,
                        grid,
                        us_u,
                        upper_y + 1,
                        v,
                        PlannedBlock::with_props(
                            idoor.clone(),
                            vec![
                                ("facing", grid.dir_right()),
                                ("half", "lower"),
                                ("hinge", "left"),
                                ("open", "true"),
                            ],
                        ),
                    );
                    set_uv(
                        blocks,
                        grid,
                        us_u,
                        upper_y + 2,
                        v,
                        PlannedBlock::with_props(
                            idoor,
                            vec![
                                ("facing", grid.dir_right()),
                                ("half", "upper"),
                                ("hinge", "left"),
                                ("open", "true"),
                            ],
                        ),
                    );
                    for pass_u in [us_u - 1, us_u + 1] {
                        for dy in 1..=2 {
                            set_uv(
                                blocks,
                                grid,
                                pass_u,
                                upper_y + dy,
                                v,
                                PlannedBlock::simple("minecraft:air"),
                            );
                        }
                    }
                    if upper_y + 3 < upper_ceil {
                        set_uv(
                            blocks,
                            grid,
                            us_u,
                            upper_y + 3,
                            v,
                            make_frame_block_at(pal, grid.axis_v(), seed, us_u, upper_y + 3, v),
                        );
                    }
                } else {
                    for y in (upper_y + 1)..upper_ceil {
                        if is_jamb || !is_solid_or_occupied(blocks, grid, us_u, y, v) {
                            set_uv(blocks, grid, us_u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_UPPER, seed, us_u, y, v)));
                        }
                    }
                }
            }
        }
    }

    // Master bedroom suite on upper floor (or ground corner on single-story builds):
    // style-matched double/single bed, mounted headboard shelf, and bedside lamp/pot table.
    let mut master_bed_uv: Option<(i32, i32)> = None;
    if matches!(
        spec.category,
        BuildCategory::House | BuildCategory::Tower | BuildCategory::Castle
    ) {
        let bed_floor_y = if spec.floors >= 2 {
            ground_y + i32::from(spec.floor_height)
        } else {
            ground_y
        };
        let bed_ceil = (bed_floor_y + i32::from(spec.floor_height) - 1).min(roof_base_y);
        let bed_id = bed_id_for_style(spec.style);
        let active_cells = if spec.floors >= 2 { &upper_cells } else { &interior_cells };
        let bed_cell_ok = |u: i32, v: i32| -> bool {
            active_cells.contains(&(u, v))
                && !stairwell_uv
                    .iter()
                    .any(|&(su, sv)| (su - u).abs() <= 1 && (sv - v).abs() <= 1)
                && chimney_uv != Some((u, v))
                && !door_clearance_uv.contains(&(u, v))
                && !upper_door_clearance.contains(&(u, v))
                && split_u_opt != Some(u)
                && split_v_opt != Some(v)
                && upper_split_u_opt != Some(u)
                && is_solid_or_occupied(blocks, grid, u, bed_floor_y, v)
                && !is_solid_or_occupied(blocks, grid, u, bed_floor_y + 1, v)
                && (spec.floors >= 2
                    || (is_clear_prop_spot(u, v)
                        && !wall_spots.iter().any(|&(wu, wv, _)| wu == u && wv == v)))
        };

        if bed_floor_y + 2 < bed_ceil
            && let Some(&(bu, bv)) = active_cells
                .iter()
                .find(|&&(u, v)| bed_cell_ok(u, v) && bed_cell_ok(u, v - 1))
        {
            master_bed_uv = Some((bu, bv));
            let bface = grid.dir_back();
            let double_bed = bed_cell_ok(bu + 1, bv) && bed_cell_ok(bu + 1, bv - 1);
            let bed_cols = if double_bed { 2 } else { 1 };
            let stand_u = if bed_cell_ok(bu + bed_cols, bv - 1) {
                Some(bu + bed_cols)
            } else if bed_cell_ok(bu - 1, bv - 1) {
                Some(bu - 1)
            } else {
                None
            };
            for du in 0..bed_cols {
                set_uv(
                    blocks,
                    grid,
                    bu + du,
                    bed_floor_y + 1,
                    bv,
                    PlannedBlock::with_props(
                        bed_id,
                        vec![("facing", bface), ("part", "foot"), ("occupied", "false")],
                    ),
                );
                set_uv(
                    blocks,
                    grid,
                    bu + du,
                    bed_floor_y + 1,
                    bv - 1,
                    PlannedBlock::with_props(
                        bed_id,
                        vec![("facing", bface), ("part", "head"), ("occupied", "false")],
                    ),
                );
                let back_wall_solid = get_uv(blocks, grid, bu + du, bed_floor_y + 2, bv - 2).is_some_and(|b| {
                    let bid = b.block_id.as_str();
                    bid != "minecraft:air" && !bid.contains("glass") && !bid.ends_with("_door")
                });
                if back_wall_solid
                    && bed_floor_y + 3 <= bed_ceil
                    && !is_solid_or_occupied(blocks, grid, bu + du, bed_floor_y + 2, bv - 1)
                {
                    set_uv(
                        blocks,
                        grid,
                        bu + du,
                        bed_floor_y + 2,
                        bv - 1,
                        PlannedBlock::with_props(
                            shelf_id,
                            vec![("facing", grid.dir_front()), ("waterlogged", "false")],
                        ),
                    );
                }
            }

            if let Some(su) = stand_u {
                set_uv(
                    blocks,
                    grid,
                    su,
                    bed_floor_y + 1,
                    bv - 1,
                    PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]),
                );
                if bed_floor_y + 2 < bed_ceil && !is_solid_or_occupied(blocks, grid, su, bed_floor_y + 2, bv - 1) {
                    set_uv(
                        blocks,
                        grid,
                        su,
                        bed_floor_y + 2,
                        bv - 1,
                        PlannedBlock::simple(style_pot(spec.style)),
                    );
                }
            }
        }
    }

    // Furnish upper stories: Study/Library wing (cartography, loom, lectern, ender_chest, shelves) + Guest Bed.
    for f in 1..i32::from(spec.floors) {
        let upper_y = ground_y + f * i32::from(spec.floor_height);
        let upper_ceil = (upper_y + i32::from(spec.floor_height) - 1).min(roof_base_y);
        if upper_y + 2 >= upper_ceil {
            continue;
        }
        for dv in -1..=0 {
            for du in -1..=0 {
                let ru = cu + du;
                let rv = cv + dv;
                if upper_cells.contains(&(ru, rv))
                    && !stairwell_uv.contains(&(ru, rv))
                    && upper_split_u_opt != Some(ru)
                    && !upper_door_clearance.contains(&(ru, rv))
                    && is_solid_or_occupied(blocks, grid, ru, upper_y, rv)
                    && !is_solid_or_occupied(blocks, grid, ru, upper_y + 1, rv)
                {
                    let uc = if !pal.carpet.is_empty() {
                        pal.pick(MaterialPalette::SLOT_CARPET, seed, ru, upper_y + 1, rv)
                    } else {
                        carpet_id.to_owned()
                    };
                    set_uv(blocks, grid, ru, upper_y + 1, rv, PlannedBlock::simple(uc));
                }
            }
        }

        let upper_props: &[(&str, &str)] = if f == 1 {
            &[
                ("minecraft:cartography_table", "none"),
                ("minecraft:chiseled_bookshelf", "facing"),
                ("minecraft:loom", "facing"),
                ("minecraft:lectern", "lectern"),
                ("minecraft:bookshelf", "none"),
                ("minecraft:chest", "chest"),
            ]
        } else {
            &[
                ("minecraft:enchanting_table", "none"),
                ("minecraft:ender_chest", "water_facing"),
                ("minecraft:chiseled_bookshelf", "facing"),
                ("minecraft:cartography_table", "none"),
                ("minecraft:chest", "chest"),
            ]
        };

        let mut placed = 0usize;
        let mut candle_placed = false;
        for &(wu, wv) in &upper_cells {
            if placed >= upper_props.len() {
                break;
            }
            if stairwell_uv.contains(&(wu, wv))
                || stairwell_uv
                    .iter()
                    .any(|&(su, sv)| (su - wu).abs() <= 1 && (sv - wv).abs() <= 1)
                || chimney_uv == Some((wu, wv))
                || upper_split_u_opt == Some(wu)
                || upper_door_clearance.contains(&(wu, wv))
                || !is_solid_or_occupied(blocks, grid, wu, upper_y, wv)
                || is_solid_or_occupied(blocks, grid, wu, upper_y + 1, wv)
            {
                continue;
            }
            let face_opt = if !upper_cells.contains(&(wu, wv - 1)) && is_fp(grid, upper_fp, wu, wv - 1) {
                Some(grid.dir_front())
            } else if !upper_cells.contains(&(wu - 1, wv)) && is_fp(grid, upper_fp, wu - 1, wv) {
                Some(grid.dir_right())
            } else if !upper_cells.contains(&(wu + 1, wv)) && is_fp(grid, upper_fp, wu + 1, wv) {
                Some(grid.dir_left())
            } else {
                None
            };
            let Some(face) = face_opt else {
                continue;
            };
            let (blk_id, mode) = upper_props[placed];
            let props = match mode {
                "lectern" => vec![("facing", face), ("has_book", "true")],
                "chest" => vec![("facing", face), ("type", "single")],
                "water_facing" => vec![("facing", face), ("waterlogged", "false")],
                "facing" => vec![("facing", face)],
                _ => vec![],
            };
            set_uv(
                blocks,
                grid,
                wu,
                upper_y + 1,
                wv,
                PlannedBlock::with_props(blk_id, props),
            );
            if !candle_placed
                && matches!(
                    blk_id,
                    "minecraft:bookshelf" | "minecraft:chiseled_bookshelf" | "minecraft:chest"
                )
                && upper_y + 2 < upper_ceil
                && !is_solid_or_occupied(blocks, grid, wu, upper_y + 2, wv)
            {
                set_uv(
                    blocks,
                    grid,
                    wu,
                    upper_y + 2,
                    wv,
                    PlannedBlock::with_props(
                        "minecraft:candle",
                        vec![
                            ("candles", if f >= 2 { "3" } else { "2" }),
                            ("lit", "true"),
                            ("waterlogged", "false"),
                        ],
                    ),
                );
                candle_placed = true;
            }
            placed += 1;
        }

        // Secondary Guest Bed in separate upper wing on spacious House / Castle 2nd floor.
        if f == 1
            && upper_cells.len() >= 24
            && matches!(spec.category, BuildCategory::House | BuildCategory::Castle)
            && let Some((mbu, mbv)) = master_bed_uv
        {
            let guest_cell = upper_cells.iter().copied().find(|&(gu, gv)| {
                let on_other_wing = upper_split_u_opt.map_or((gu - mbu).abs() >= 3 || (gv - mbv).abs() >= 3, |us_u| {
                    (mbu < us_u && gu > us_u) || (mbu > us_u && gu < us_u)
                });
                on_other_wing
                    && upper_cells.contains(&(gu, gv - 1))
                    && upper_split_u_opt != Some(gu)
                    && !upper_door_clearance.contains(&(gu, gv))
                    && !upper_door_clearance.contains(&(gu, gv - 1))
                    && !stairwell_uv
                        .iter()
                        .any(|&(su, sv)| (su - gu).abs() <= 1 && ((sv - gv).abs() <= 1 || (sv - (gv - 1)).abs() <= 1))
                    && chimney_uv != Some((gu, gv))
                    && chimney_uv != Some((gu, gv - 1))
                    && is_solid_or_occupied(blocks, grid, gu, upper_y, gv)
                    && is_solid_or_occupied(blocks, grid, gu, upper_y, gv - 1)
                    && !is_solid_or_occupied(blocks, grid, gu, upper_y + 1, gv)
                    && !is_solid_or_occupied(blocks, grid, gu, upper_y + 1, gv - 1)
            });
            if let Some((gu, gv)) = guest_cell {
                let gface = grid.dir_back();
                set_uv(
                    blocks,
                    grid,
                    gu,
                    upper_y + 1,
                    gv,
                    PlannedBlock::with_props(
                        "minecraft:white_bed",
                        vec![("facing", gface), ("part", "foot"), ("occupied", "false")],
                    ),
                );
                set_uv(
                    blocks,
                    grid,
                    gu,
                    upper_y + 1,
                    gv - 1,
                    PlannedBlock::with_props(
                        "minecraft:white_bed",
                        vec![("facing", gface), ("part", "head"), ("occupied", "false")],
                    ),
                );
            }
        }
    }

    // Cohesive Ground-Floor Kitchen Counter + Overhead Cupboard Shelves (Houses) + Mezzanine & Basements.
    if spec.category == BuildCategory::House {
        let mut linear_placed = false;
        'counter: for idx in 0..wall_spots.len() {
            let (u0, v0, face0) = wall_spots[idx];
            if face0 != grid.dir_front() {
                continue;
            }
            let run: Vec<(i32, i32)> = (0..4)
                .filter_map(|du| {
                    let nu = u0 + du;
                    wall_spots
                        .iter()
                        .any(|&(wu, wv, wf)| wu == nu && wv == v0 && wf == face0)
                        .then_some((nu, v0))
                })
                .filter(|&(nu, nv)| !is_solid_or_occupied(blocks, grid, nu, ground_y + 1, nv))
                .collect();
            if run.len() >= 3 {
                for (step, &(ku, kv)) in run.iter().enumerate() {
                    let (blk, props) = match step {
                        0 => ("minecraft:smoker", vec![("facing", face0), ("lit", "false")]),
                        1 => ("minecraft:crafting_table", vec![]),
                        2 => ("minecraft:water_cauldron", vec![("level", "3")]),
                        _ => ("minecraft:barrel", vec![("facing", "up")]),
                    };
                    set_uv(
                        blocks,
                        grid,
                        ku,
                        ground_y + 1,
                        kv,
                        PlannedBlock::with_props(blk, props),
                    );
                    if step == 3 && !is_solid_or_occupied(blocks, grid, ku, ground_y + 2, kv) {
                        set_uv(
                            blocks,
                            grid,
                            ku,
                            ground_y + 2,
                            kv,
                            PlannedBlock::with_props("minecraft:cake", vec![("bites", "0")]),
                        );
                    }
                    let back_solid = get_uv(blocks, grid, ku, ground_y + 3, kv - 1).is_some_and(|b| {
                        let bid = b.block_id.as_str();
                        bid != "minecraft:air" && !bid.contains("glass") && !bid.ends_with("_door")
                    });
                    if spec.floor_height >= 4
                        && back_solid
                        && (step == 0 || step == 2)
                        && !is_solid_or_occupied(blocks, grid, ku, ground_y + 3, kv)
                    {
                        set_uv(
                            blocks,
                            grid,
                            ku,
                            ground_y + 3,
                            kv,
                            PlannedBlock::with_props(
                                shelf_id,
                                vec![("facing", face0), ("waterlogged", "false")],
                            ),
                        );
                    }
                }
                linear_placed = true;
                break 'counter;
            }
        }
        if !linear_placed {
            let mut kitchen_placed = 0usize;
            for (i, &(wu, wv, face)) in wall_spots.iter().enumerate() {
                if i % 2 == 0 || is_solid_or_occupied(blocks, grid, wu, ground_y + 1, wv) {
                    continue;
                }
                let (blk, props) = match kitchen_placed {
                    0 => ("minecraft:smoker", vec![("facing", face), ("lit", "false")]),
                    1 => ("minecraft:water_cauldron", vec![("level", "3")]),
                    2 => ("minecraft:barrel", vec![("facing", "up")]),
                    _ => break,
                };
                set_uv(
                    blocks,
                    grid,
                    wu,
                    ground_y + 1,
                    wv,
                    PlannedBlock::with_props(blk, props),
                );
                kitchen_placed += 1;
            }
        }
    }

    if matches!(spec.category, BuildCategory::Temple | BuildCategory::Castle)
        && spec.floors >= 2
        && spec.floor_height >= 5
        && upper_cells.len() >= 24
    {
        let mez_y = ground_y + i32::from(spec.floor_height);
        for dv in -1..=0 {
            for du in -1..=0 {
                let u = cu + du;
                let v = cv + dv;
                if upper_cells.contains(&(u, v)) && !stairwell_uv.contains(&(u, v)) {
                    set_uv(blocks, grid, u, mez_y, v, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, u, mez_y + 1, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
        for v in (cv - 2)..=(cv + 1) {
            for u in (cu - 2)..=(cu + 1) {
                let on_rim = u == cu - 2 || u == cu + 1 || v == cv - 2 || v == cv + 1;
                if on_rim
                    && upper_cells.contains(&(u, v))
                    && !stairwell_uv.contains(&(u, v))
                    && is_solid_or_occupied(blocks, grid, u, mez_y, v)
                    && !is_solid_or_occupied(blocks, grid, u, mez_y + 1, v)
                {
                    set_uv(blocks, grid, u, mez_y + 1, v, PlannedBlock::simple(pal.fence.clone()));
                }
            }
        }
    }

    if spec.basement {
        let cellar_mode = match (spec.style, spec.category) {
            (BuildStyle::Gothic, _) | (_, BuildCategory::Temple) => 1, // Alchemy Vault
            (_, BuildCategory::Forge | BuildCategory::Castle) | (BuildStyle::Nordic, _) => 2, // Armory
            _ => rng.range_i32(0, 2),
        };
        let cellar_props: &[(&str, &str)] = match cellar_mode {
            1 => &[
                ("minecraft:brewing_stand", "none"),
                ("minecraft:water_cauldron", "cauldron"),
                ("minecraft:bookshelf", "none"),
                ("minecraft:chest", "chest"),
            ],
            2 => &[
                ("minecraft:smithing_table", "none"),
                ("minecraft:grindstone", "grind"),
                ("minecraft:anvil", "facing"),
                ("minecraft:chest", "chest"),
            ],
            _ => &[
                ("minecraft:barrel", "side_barrel"),
                ("minecraft:barrel", "side_barrel"),
                ("minecraft:chest", "chest"),
                ("minecraft:barrel", "side_barrel"),
            ],
        };
        let mut bp_idx = 0usize;
        for &(wu, wv, face) in &wall_spots {
            if bp_idx >= cellar_props.len() {
                break;
            }
            if stairwell_uv.iter().any(|&(su, sv)| (su - wu).abs() <= 1 && (sv - wv).abs() <= 1)
                || is_solid_or_occupied(blocks, grid, wu, -3, wv)
            {
                continue;
            }
            let (id, mode) = cellar_props[bp_idx];
            let props = match mode {
                "cauldron" => vec![("level", "3")],
                "chest" => vec![("facing", face), ("type", "single")],
                "grind" => vec![("face", "floor"), ("facing", face)],
                "side_barrel" => vec![("facing", face)],
                "facing" => vec![("facing", face)],
                _ => vec![],
            };
            set_uv(blocks, grid, wu, -3, wv, PlannedBlock::with_props(id, props));
            if mode == "side_barrel" && !is_solid_or_occupied(blocks, grid, wu, -2, wv) {
                set_uv(
                    blocks,
                    grid,
                    wu,
                    -2,
                    wv,
                    PlannedBlock::with_props("minecraft:barrel", vec![("facing", face)]),
                );
            }
            bp_idx += 1;
        }
    }
}
