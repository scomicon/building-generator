use std::collections::BTreeMap;

use crate::gen_common::{
    LocalGrid, PlannedBlock, get_uv, is_fp, is_lantern, is_solid_or_occupied,
    make_frame_block_at, make_leaves, make_pillar_block_at, make_slab, make_stair, make_trapdoor,
    set_uv, style_pot, terrain_rel_at, water_depth_at,
};
use crate::model::{BuildCategory, BuildStyle, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, StructureTheme};

/// Places terrain-adaptive plinth, optional basement vault, lakeside/hillside stilts, & Japanese Engawa deck.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_foundation_and_basement(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    noise: NoiseField,
    adapt_terrain: bool,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) {
    let y_top = i32::from(spec.foundation_height);
    if spec.on_stilts {
        for v in 0..grid.d {
            for u in 0..grid.w {
                if !fp[grid.idx(u, v)] {
                    continue;
                }
                let is_boundary = !is_fp(grid, fp, u - 1, v)
                    || !is_fp(grid, fp, u + 1, v)
                    || !is_fp(grid, fp, u, v - 1)
                    || !is_fp(grid, fp, u, v + 1);
                let is_corner = (!is_fp(grid, fp, u - 1, v) || !is_fp(grid, fp, u + 1, v))
                    && (!is_fp(grid, fp, u, v - 1) || !is_fp(grid, fp, u, v + 1));

                // Platform joists & timber deck directly beneath the first floor.
                let deck_y = y_top - 1;
                let deck_blk = if is_boundary {
                    make_frame_block_at(
                        pal,
                        if !is_fp(grid, fp, u, v - 1) || !is_fp(grid, fp, u, v + 1) {
                            grid.axis_u()
                        } else {
                            grid.axis_v()
                        },
                        seed,
                        u,
                        deck_y,
                        v,
                    )
                } else {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, deck_y, v))
                };
                set_uv(blocks, grid, u, deck_y, v, deck_blk);

                let is_pier_col = is_corner
                    || (is_boundary && (u % 4 == 0 || v % 4 == 0))
                    || (u % 4 == 0 && v % 4 == 0);
                let terr_rel = terrain_rel_at(terrain_offsets, grid, u, v);
                let wdepth = water_depth_at(water_depths, grid, u, v);
                let solid_ground = terr_rel - wdepth;

                if is_pier_col {
                    let stilt_bottom = (solid_ground - 1).clamp(-14, deck_y - 1);
                    for y in stilt_bottom..=deck_y {
                        let blk = if y <= solid_ground + 1 {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v))
                        } else {
                            make_frame_block_at(pal, "y", seed, u, y, v)
                        };
                        set_uv(blocks, grid, u, y, v, blk);
                    }
                    // Decorative under-deck knee braces flanking pier pilings.
                    if is_boundary && deck_y - 1 > solid_ground {
                        for (du, dv) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            if is_fp(grid, fp, u + du, v + dv) && !is_solid_or_occupied(blocks, grid, u + du, deck_y - 1, v + dv) {
                                set_uv(
                                    blocks,
                                    grid,
                                    u + du,
                                    deck_y - 1,
                                    v + dv,
                                    make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u + du, deck_y - 1, v + dv), "top"),
                                );
                            }
                        }
                    }
                } else if is_boundary && deck_y - 1 > solid_ground {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        deck_y - 1,
                        v,
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, deck_y - 1, v), "top"),
                    );
                }
            }
        }
    } else {
        for v in 0..grid.d {
            for u in 0..grid.w {
                if !fp[grid.idx(u, v)] {
                    continue;
                }
                let terrain_rel = terrain_rel_at(terrain_offsets, grid, u, v);
                let wdepth = water_depth_at(water_depths, grid, u, v);
                let seabed_rel = terrain_rel - wdepth;
                let is_boundary = !is_fp(grid, fp, u - 1, v)
                    || !is_fp(grid, fp, u + 1, v)
                    || !is_fp(grid, fp, u, v - 1)
                    || !is_fp(grid, fp, u, v + 1);
                let skirt_bottom = if spec.basement {
                    seabed_rel.clamp(-10, -4).min(-4)
                } else if adapt_terrain {
                    seabed_rel.clamp(-10, 0)
                } else {
                    0
                };

                for y in skirt_bottom..y_top {
                    if spec.basement && !is_boundary && (-3..0).contains(&y) {
                        set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                        continue;
                    }
                    let n = noise.fbm_3d(u as f64 * 0.38, y as f64 * 0.42, v as f64 * 0.38, 3);
                    let blk = if n > (0.90 - f64::from(spec.weathering) * 0.60) {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v))
                    };
                    set_uv(blocks, grid, u, y, v, blk);
                }
            }
        }
    }

    // Place basement storage props & ceiling lanterns when interior depth >= 3.
    if spec.basement {
        for v in 1..(grid.d - 1) {
            for u in 1..(grid.w - 1) {
                let inner = is_fp(grid, fp, u, v)
                    && is_fp(grid, fp, u - 1, v)
                    && is_fp(grid, fp, u + 1, v)
                    && is_fp(grid, fp, u, v - 1)
                    && is_fp(grid, fp, u, v + 1);
                if !inner {
                    continue;
                }
                let bas_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, -1, v);
                if u % 4 == 2 && v % 4 == 2 && is_lantern(&bas_light) {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        -1,
                        v,
                        PlannedBlock::with_props(bas_light, vec![("hanging", "true")]),
                    );
                } else if !is_fp(grid, fp, u - 2, v) && (u + v) % 3 == 0 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        -3,
                        v,
                        PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]),
                    );
                }
            }
        }
    }

    // Japanese wrap-around Engawa veranda deck.
    if spec.style == BuildStyle::Japanese && spec.category != BuildCategory::Tower {
        let engawa_y = i32::from(spec.foundation_height);
        for v in 0..grid.d {
            for u in 0..grid.w {
                if fp[grid.idx(u, v)] {
                    continue;
                }
                let adj = is_fp(grid, fp, u - 1, v)
                    || is_fp(grid, fp, u + 1, v)
                    || is_fp(grid, fp, u, v - 1)
                    || is_fp(grid, fp, u, v + 1);
                if adj {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        engawa_y,
                        v,
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, engawa_y, v), "bottom"),
                    );
                }
            }
        }
    }
}

/// Carves entrance doorway, optional double gate, covered porch, and terrain-reaching stair descent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_entrance_and_porch(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    adapt_terrain: bool,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
    rng: &mut SplitMix64,
) -> (Option<(i32, i32)>, Option<[i32; 3]>) {
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let span_top = (b_u1 - b_u0 - 2).max(1);
    let preferred_u = if matches!(
        spec.category,
        BuildCategory::Temple | BuildCategory::Castle | BuildCategory::Pavilion
    ) || matches!(spec.theme, StructureTheme::Tavern | StructureTheme::Greenhouse)
    {
        (b_u0 + b_u1) / 2
    } else {
        match rng.range_i32(0, 2) {
            0 => b_u0 + 2.min(span_top),
            1 => (b_u0 + b_u1) / 2,
            _ => (b_u1 - 2).max(b_u0 + 1),
        }
    };

    let mut best_door: Option<(i32, i32)> = None;
    let mut best_score = i32::MAX;
    for u in (b_u0 + 1)..=(b_u1 - 1) {
        for v in (b_v0..=b_v1).rev() {
            if is_fp(grid, fp, u, v) && !is_fp(grid, fp, u, v + 1) {
                let is_side_wall = is_fp(grid, fp, u - 1, v)
                    && is_fp(grid, fp, u + 1, v)
                    && !is_fp(grid, fp, u - 1, v + 1)
                    && !is_fp(grid, fp, u + 1, v + 1);
                if is_side_wall {
                    let score = (u - preferred_u).abs() + (b_v1 - v) * 2;
                    if score < best_score {
                        best_score = score;
                        best_door = Some((u, v));
                    }
                }
                break;
            }
        }
    }

    let mut door_pos = None;
    if let Some((du, dv)) = best_door {
        let door_base_y = i32::from(spec.foundation_height) + 1;
        let is_wide_door = matches!(
            spec.category,
            BuildCategory::Barn | BuildCategory::Castle | BuildCategory::Temple
        ) && is_fp(grid, fp, du + 1, dv)
            && is_fp(grid, fp, du + 2, dv)
            && is_fp(grid, fp, du + 1, dv - 1)
            && !is_fp(grid, fp, du + 1, dv + 1)
            && !is_fp(grid, fp, du + 2, dv + 1);
        let recessed_entry = !is_wide_door
            && spec.porch
            && (b_u1 - b_u0) >= 8
            && (b_v1 - b_v0) >= 8
            && matches!(
                spec.style,
                BuildStyle::Japanese | BuildStyle::Medieval | BuildStyle::Cottage
            )
            && is_fp(grid, fp, du, dv - 1)
            && is_fp(grid, fp, du, dv - 2)
            && is_fp(grid, fp, du - 1, dv - 1)
            && is_fp(grid, fp, du + 1, dv - 1)
            && is_fp(grid, fp, du - 1, dv)
            && is_fp(grid, fp, du + 1, dv);

        let actual_dv = if recessed_entry { dv - 1 } else { dv };
        let right_jamb_u = if is_wide_door { du + 2 } else { du + 1 };
        if recessed_entry {
            set_uv(
                blocks,
                grid,
                du,
                door_base_y - 1,
                dv,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, du, door_base_y - 1, dv)),
            );
            set_uv(blocks, grid, du, door_base_y, dv, PlannedBlock::simple("minecraft:air"));
            set_uv(blocks, grid, du, door_base_y + 1, dv, PlannedBlock::simple("minecraft:air"));
            set_uv(
                blocks,
                grid,
                du,
                door_base_y + 2,
                dv,
                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du, door_base_y + 2, dv), grid.dir_front(), "top"),
            );
            for dy in 0..=1 {
                set_uv(blocks, grid, du - 1, door_base_y + dy, dv, make_frame_block_at(pal, "y", seed, du - 1, door_base_y + dy, dv));
                set_uv(blocks, grid, du + 1, door_base_y + dy, dv, make_frame_block_at(pal, "y", seed, du + 1, door_base_y + dy, dv));
                set_uv(blocks, grid, du - 1, door_base_y + dy, actual_dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, du - 1, door_base_y + dy, actual_dv)));
                set_uv(blocks, grid, du + 1, door_base_y + dy, actual_dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, du + 1, door_base_y + dy, actual_dv)));
            }
            set_uv(blocks, grid, du - 1, door_base_y + 2, actual_dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, du - 1, door_base_y + 2, actual_dv)));
            set_uv(blocks, grid, du + 1, door_base_y + 2, actual_dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, du + 1, door_base_y + 2, actual_dv)));
        } else {
            for dy in 0..=1 {
                if !is_solid_or_occupied(blocks, grid, du - 1, door_base_y + dy, actual_dv) {
                    set_uv(blocks, grid, du - 1, door_base_y + dy, actual_dv, make_frame_block_at(pal, "y", seed, du - 1, door_base_y + dy, actual_dv));
                }
                if !is_solid_or_occupied(blocks, grid, right_jamb_u, door_base_y + dy, actual_dv) {
                    set_uv(blocks, grid, right_jamb_u, door_base_y + dy, actual_dv, make_frame_block_at(pal, "y", seed, right_jamb_u, door_base_y + dy, actual_dv));
                }
            }
        }

        let entrance_door_id = pal.pick(MaterialPalette::SLOT_DOOR, seed, du, door_base_y, actual_dv);
        set_uv(
            blocks,
            grid,
            du,
            door_base_y,
            actual_dv,
            PlannedBlock::with_props(
                entrance_door_id.clone(),
                vec![
                    ("facing", grid.dir_front()),
                    ("half", "lower"),
                    ("hinge", "left"),
                    ("open", "false"),
                ],
            ),
        );
        set_uv(
            blocks,
            grid,
            du,
            door_base_y + 1,
            actual_dv,
            PlannedBlock::with_props(
                entrance_door_id.clone(),
                vec![
                    ("facing", grid.dir_front()),
                    ("half", "upper"),
                    ("hinge", "left"),
                    ("open", "false"),
                ],
            ),
        );
        set_uv(
            blocks,
            grid,
            du,
            door_base_y + 2,
            actual_dv,
            make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du, door_base_y + 2, actual_dv), grid.dir_front(), "top"),
        );

        // Wide double entrance for Barn, Castle, and Temple.
        if is_wide_door {
            set_uv(
                blocks,
                grid,
                du + 1,
                door_base_y,
                dv,
                PlannedBlock::with_props(
                    entrance_door_id.clone(),
                    vec![
                        ("facing", grid.dir_front()),
                        ("half", "lower"),
                        ("hinge", "right"),
                        ("open", "false"),
                    ],
                ),
            );
            set_uv(
                blocks,
                grid,
                du + 1,
                door_base_y + 1,
                dv,
                PlannedBlock::with_props(
                    entrance_door_id,
                    vec![
                        ("facing", grid.dir_front()),
                        ("half", "upper"),
                        ("hinge", "right"),
                        ("open", "false"),
                    ],
                ),
            );
            for dy in 0..=1 {
                set_uv(blocks, grid, du + 1, door_base_y + dy, dv - 1, PlannedBlock::simple("minecraft:air"));
                set_uv(blocks, grid, du + 1, door_base_y + dy, dv + 1, PlannedBlock::simple("minecraft:air"));
            }
            if spec.category == BuildCategory::Castle {
                set_uv(blocks, grid, du, door_base_y + 2, dv, PlannedBlock::simple("minecraft:iron_bars"));
                set_uv(blocks, grid, du + 1, door_base_y + 2, dv, PlannedBlock::simple("minecraft:iron_bars"));
            }
        }

        for dy in 0..=1 {
            set_uv(
                blocks,
                grid,
                du,
                door_base_y + dy,
                actual_dv - 1,
                PlannedBlock::simple("minecraft:air"),
            );
            set_uv(blocks, grid, du, door_base_y + dy, dv + 1, PlannedBlock::simple("minecraft:air"));
        }

        let [dlx, dlz] = grid.to_local_xz(du, actual_dv);
        door_pos = Some([dlx, door_base_y, dlz]);

        let step_y = i32::from(spec.foundation_height);
        let door_u1 = if is_wide_door { du + 1 } else { du };
        let use_classical_portico = spec.style == BuildStyle::Classical
            && (b_u1 - b_u0) >= 8
            && grid.in_bounds(du, dv + 2)
            && !is_fp(grid, fp, du, dv + 2)
            && !is_fp(grid, fp, du, dv + 1);
        let porch_depth = if use_classical_portico { 2 } else { 1 };
        let stair_start_v = if (spec.porch || use_classical_portico)
            && grid.in_bounds(du, dv + 1)
            && !is_fp(grid, fp, du, dv + 1)
        {
            for pv_off in 1..=porch_depth {
                let pv = dv + pv_off;
                for pu in (du - 1)..=(door_u1 + 1) {
                    if !grid.in_bounds(pu, pv) || is_fp(grid, fp, pu, pv) {
                        continue;
                    }
                    set_uv(blocks, grid, pu, step_y, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, pu, step_y, pv)));
                    set_uv(blocks, grid, pu, step_y + 1, pv, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, pu, step_y + 2, pv, PlannedBlock::simple("minecraft:air"));
                    let is_outer_post = (pu < du || pu > door_u1) && pv_off == porch_depth;
                    let porch_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, pu, step_y + 2, pv);
                    if is_outer_post {
                        if spec.style == BuildStyle::Classical {
                            set_uv(blocks, grid, pu, step_y + 1, pv, make_pillar_block_at(pal, "y", seed, pu, step_y + 1, pv));
                            set_uv(blocks, grid, pu, step_y + 2, pv, make_pillar_block_at(pal, "y", seed, pu, step_y + 2, pv));
                        } else {
                            set_uv(blocks, grid, pu, step_y + 1, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, pu, step_y + 1, pv)));
                            if spec.garden_lanterns && is_lantern(&porch_light) {
                                set_uv(blocks, grid, pu, step_y + 2, pv, PlannedBlock::simple(porch_light));
                            } else {
                                set_uv(blocks, grid, pu, step_y + 2, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, pu, step_y + 2, pv)));
                            }
                        }
                    } else if (pu < du || pu > door_u1) && spec.style != BuildStyle::Classical {
                        set_uv(blocks, grid, pu, step_y + 1, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, pu, step_y + 1, pv)));
                    }

                    if use_classical_portico {
                        set_uv(blocks, grid, pu, step_y + 3, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, pu, step_y + 3, pv)));
                        if (du..=door_u1).contains(&pu) {
                            set_uv(blocks, grid, pu, step_y + 4, pv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, pu, step_y + 4, pv), "bottom"));
                        }
                    } else if spec.garden_lanterns && !is_lantern(&pal.light) && (du..=door_u1).contains(&pu) {
                        set_uv(blocks, grid, pu, step_y + 3, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, pu, step_y + 3, pv)));
                    } else {
                        set_uv(blocks, grid, pu, step_y + 3, pv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_CEILING, seed, pu, step_y + 3, pv), "top"));
                    }
                }
            }
            dv + porch_depth + 1
        } else {
            dv + 1
        };

        // Adaptive stair cascade (or waterfront boardwalk dock with mooring bollards in stilt/water mode).
        let front_is_water = water_depth_at(water_depths, grid, du, stair_start_v) > 0;
        if spec.on_stilts && (front_is_water || water_depth_at(water_depths, grid, du, dv) > 0) {
            let dock_y = step_y;
            let dock_end_v = (stair_start_v + 3).min(grid.d - 1);
            for pv in stair_start_v..=dock_end_v {
                for pu in (du - 1)..=(door_u1 + 1) {
                    if !grid.in_bounds(pu, pv) || is_fp(grid, fp, pu, pv) {
                        continue;
                    }
                    set_uv(blocks, grid, pu, dock_y, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, pu, dock_y, pv)));
                    set_uv(blocks, grid, pu, dock_y + 1, pv, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, pu, dock_y + 2, pv, PlannedBlock::simple("minecraft:air"));
                    if pu == du - 1 || pu == door_u1 + 1 {
                        if pv == dock_end_v {
                            let wd = water_depth_at(water_depths, grid, pu, pv);
                            let bed = terrain_rel_at(terrain_offsets, grid, pu, pv) - wd;
                            for py in (bed - 1).clamp(-14, dock_y - 1)..=dock_y {
                                set_uv(blocks, grid, pu, py, pv, make_frame_block_at(pal, "y", seed, pu, py, pv));
                            }
                            set_uv(blocks, grid, pu, dock_y + 1, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, pu, dock_y + 1, pv)));
                            let dock_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, pu, dock_y + 2, pv);
                            if is_lantern(&dock_light) {
                                set_uv(blocks, grid, pu, dock_y + 2, pv, PlannedBlock::simple(dock_light));
                            }
                        } else if pv < dock_end_v {
                            set_uv(blocks, grid, pu, dock_y + 1, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, pu, dock_y + 1, pv)));
                        }
                    }
                }
            }
        } else {
            let mut sy = if spec.porch && step_y >= 2 { step_y - 1 } else { step_y };
            let mut sv = stair_start_v;
            while sv < grid.d && !is_fp(grid, fp, du, sv) {
                let ground_rel = if adapt_terrain {
                    terrain_rel_at(terrain_offsets, grid, du, sv).clamp(-8, 4)
                } else {
                    0
                };
                if sy <= ground_rel {
                    break;
                }
                for step_u in du..=door_u1 {
                    if !grid.in_bounds(step_u, sv) || is_fp(grid, fp, step_u, sv) {
                        continue;
                    }
                    set_uv(
                        blocks,
                        grid,
                        step_u,
                        sy,
                        sv,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, step_u, sy, sv), grid.dir_back(), "bottom"),
                    );
                    set_uv(blocks, grid, step_u, sy + 1, sv, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, step_u, sy + 2, sv, PlannedBlock::simple("minecraft:air"));
                    set_uv(blocks, grid, step_u, sy + 3, sv, PlannedBlock::simple("minecraft:air"));
                    if sy - 1 > ground_rel {
                        if spec.on_stilts {
                            if (sv - stair_start_v) % 2 == 0 {
                                for fill_y in (ground_rel.max(-8))..sy {
                                    set_uv(blocks, grid, step_u, fill_y, sv, make_frame_block_at(pal, "y", seed, step_u, fill_y, sv));
                                }
                            }
                        } else {
                            for fill_y in (ground_rel.max(-8))..sy {
                                set_uv(
                                    blocks,
                                    grid,
                                    step_u,
                                    fill_y,
                                    sv,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, step_u, fill_y, sv)),
                                );
                            }
                        }
                    }
                }
                sy -= 1;
                sv += 1;
            }
        }
    }

    (best_door, door_pos)
}

/// Builds varied windows, arches, shutters, planter flower boxes, and bracketed 2nd-floor balcony.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_windows_and_balcony(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    upper_fp: Option<&[bool]>,
    best_door: Option<(i32, i32)>,
    stairwell_uv: &[(i32, i32)],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    noise: NoiseField,
    rng: &mut SplitMix64,
) {
    let win_rhythm = rng.range_i32(0, 2);
    if spec.window_density > 0.01 && spec.category != BuildCategory::Pavilion {
        for f in 0..i32::from(spec.floors) {
            let cur_fp = if f >= 1 {
                upper_fp.unwrap_or(fp)
            } else {
                fp
            };
            let floor_y = i32::from(spec.foundation_height) + f * i32::from(spec.floor_height);
            let win_y0 = floor_y + 2;
            let win_y1 = if (spec.wide_windows || win_rhythm == 2 || spec.style == BuildStyle::Gothic)
                && spec.floor_height >= 4
            {
                floor_y + 3
            } else {
                floor_y + 2
            };

            for v in 0..grid.d {
                for u in 0..grid.w {
                    if !cur_fp[grid.idx(u, v)] {
                        continue;
                    }
                    let n_out_n = !is_fp(grid, cur_fp, u, v - 1);
                    let n_out_s = !is_fp(grid, cur_fp, u, v + 1);
                    let n_out_w = !is_fp(grid, cur_fp, u - 1, v);
                    let n_out_e = !is_fp(grid, cur_fp, u + 1, v);
                    let out_count = [n_out_n, n_out_s, n_out_w, n_out_e]
                        .into_iter()
                        .filter(|&x| x)
                        .count();
                    if out_count != 1 {
                        continue;
                    }
                    if let Some((du, dv)) = best_door
                        && (u - du).abs() <= 1
                        && (v - dv).abs() <= 1
                    {
                        continue;
                    }
                    // Keep perimeter walls directly supporting interior ladders or stairwell shafts solid.
                    if stairwell_uv
                        .iter()
                        .any(|&(su, sv)| (su - u).abs() + (sv - v).abs() <= 1)
                    {
                        continue;
                    }
                    let along = if n_out_n || n_out_s { u } else { v };
                    let neighbor_a = if n_out_n || n_out_s {
                        is_fp(grid, cur_fp, u - 1, v) && is_fp(grid, cur_fp, u + 1, v)
                    } else {
                        is_fp(grid, cur_fp, u, v - 1) && is_fp(grid, cur_fp, u, v + 1)
                    };
                    if !neighbor_a {
                        continue;
                    }

                    let (tan_u, tan_v) = if n_out_n || n_out_s { (1, 0) } else { (0, 1) };
                    let (odu, odv) = if n_out_n {
                        (0, -1)
                    } else if n_out_s {
                        (0, 1)
                    } else if n_out_w {
                        (-1, 0)
                    } else {
                        (1, 0)
                    };
                    let ou = u + odu;
                    let ov = v + odv;
                    let max_wy = (floor_y + i32::from(spec.floor_height) - 2).max(win_y0);
                    let clamped_y1 = win_y1.min(max_wy);
                    let window_blocked_outside = grid.in_bounds(ou, ov)
                        && (win_y0..=clamped_y1)
                            .any(|wy| is_solid_or_occupied(blocks, grid, ou, wy, ov));
                    let pier_or_special_wall = (win_y0..=clamped_y1).any(|wy| {
                        get_uv(blocks, grid, u, wy, v).is_none_or(|b| {
                            let bid = b.block_id.as_str();
                            bid == "minecraft:air"
                                || pal.slot_contains(MaterialPalette::SLOT_FRAME, bid)
                                || pal.slot_contains(MaterialPalette::SLOT_DOOR, bid)
                                || pal.slot_contains(MaterialPalette::SLOT_CHIMNEY, bid)
                                || bid.ends_with("_door")
                        })
                    });
                    if window_blocked_outside || pier_or_special_wall {
                        continue;
                    }

                    // Architectural facade clearance: keep corners & wing joints clean by avoiding
                    // windows immediately touching an adjacent wall corner or obstructed bay.
                    let adj_straight_wall = is_fp(grid, cur_fp, u - tan_u, v - tan_v)
                        && !is_fp(grid, cur_fp, u - tan_u + odu, v - tan_v + odv)
                        && is_fp(grid, cur_fp, u + tan_u, v + tan_v)
                        && !is_fp(grid, cur_fp, u + tan_u + odu, v + tan_v + odv)
                        && (win_y0..=clamped_y1).all(|wy| {
                            !is_solid_or_occupied(blocks, grid, ou - tan_u, wy, ov - tan_v)
                                && !is_solid_or_occupied(blocks, grid, ou + tan_u, wy, ov + tan_v)
                        });

                    let wn = noise.fbm_2d(u as f64 * 0.48 + f as f64 * 7.3, v as f64 * 0.48, 2);
                    let period = match win_rhythm {
                        1 => (along % 4 == 1) || (along % 4 == 2),
                        2 => along % 3 != 0,
                        _ => along % 2 == 0,
                    };
                    if !period || !adj_straight_wall || wn < (1.0 - f64::from(spec.window_density)) * 0.72 {
                        // Replace clean blind wall bays (with empty adjacent spans) with timber shelf louvers.
                        if spec.exterior_depth
                            && adj_straight_wall
                            && matches!(
                                spec.style,
                                BuildStyle::Japanese | BuildStyle::Medieval | BuildStyle::Cottage
                            )
                            && (u + v + f) % 4 == 0
                            && grid.in_bounds(ou, ov)
                            && !is_fp(grid, cur_fp, ou, ov)
                            && !(f == 0 && is_fp(grid, fp, ou, ov))
                            && !is_solid_or_occupied(blocks, grid, ou, win_y0, ov)
                            && !is_solid_or_occupied(blocks, grid, ou, win_y0 + 1, ov)
                            && get_uv(blocks, grid, u - tan_u, win_y0, v - tan_v)
                                .is_some_and(|b| !pal.slot_contains(MaterialPalette::SLOT_WINDOW, &b.block_id) && !b.block_id.contains("glass"))
                            && get_uv(blocks, grid, u + tan_u, win_y0, v + tan_v)
                                .is_some_and(|b| !pal.slot_contains(MaterialPalette::SLOT_WINDOW, &b.block_id) && !b.block_id.contains("glass"))
                        {
                            let out_dir = grid.uv_to_mc_dir(odu, odv);
                            let shelf_id = if spec.style == BuildStyle::Japanese || spec.style == BuildStyle::Medieval {
                                "minecraft:spruce_shelf"
                            } else {
                                "minecraft:oak_shelf"
                            };
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                win_y0,
                                ov,
                                PlannedBlock::with_props(
                                    shelf_id,
                                    vec![
                                        ("facing", out_dir),
                                        ("powered", "false"),
                                        ("side_chain", "unconnected"),
                                        ("waterlogged", "false"),
                                    ],
                                ),
                            );
                            if spec.flower_boxes && (u + f) % 2 == 0 {
                                set_uv(
                                    blocks,
                                    grid,
                                    ou,
                                    win_y0 + 1,
                                    ov,
                                    PlannedBlock::simple(style_pot(spec.style)),
                                );
                            }
                        }
                        continue;
                    }

                    let max_wy = (floor_y + i32::from(spec.floor_height) - 2).max(win_y0);
                    let clamped_y1 = win_y1.min(max_wy);
                    let is_kumiko_lattice = matches!(
                        spec.style,
                        BuildStyle::Japanese | BuildStyle::Medieval | BuildStyle::Cottage
                    ) && clamped_y1 > win_y0
                        && (along + f) % 4 == 1;
                    let win_pane_id = pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, win_y0, v);
                    for wy in win_y0..=clamped_y1 {
                        let wblk = if spec.style == BuildStyle::Gothic && wy == clamped_y1 {
                            PlannedBlock::simple("minecraft:iron_bars")
                        } else if is_kumiko_lattice && wy == win_y0 {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, wy, v))
                        } else {
                            PlannedBlock::simple(win_pane_id.clone())
                        };
                        set_uv(blocks, grid, u, wy, v, wblk);
                    }

                    if !grid.in_bounds(ou, ov)
                        || is_fp(grid, cur_fp, ou, ov)
                        || (f == 0 && is_fp(grid, fp, ou, ov))
                    {
                        continue;
                    }
                    let out_dir = grid.uv_to_mc_dir(odu, odv);
                    let in_dir = grid.uv_to_mc_dir(-odu, -odv);
                    let td_id = pal.pick(MaterialPalette::SLOT_TRAPDOOR, seed, ou, win_y0, ov);

                    let wall_base_solid = get_uv(blocks, grid, u, win_y0 - 1, v).is_some_and(|b| {
                        let bid = b.block_id.as_str();
                        bid != "minecraft:air" && !bid.contains("glass") && !bid.ends_with("_door")
                    });
                    if spec.flower_boxes
                        && !matches!(spec.style, BuildStyle::Gothic | BuildStyle::Modern | BuildStyle::Classical)
                        && td_id != "minecraft:iron_trapdoor"
                        && wall_base_solid
                        && !is_solid_or_occupied(blocks, grid, ou, win_y0 - 1, ov)
                        && !is_solid_or_occupied(blocks, grid, ou, win_y0, ov)
                    {
                        if ((along / 2) + f) % 2 == 0 {
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                win_y0 - 1,
                                ov,
                                make_trapdoor(&td_id, out_dir, "top", false),
                            );
                            let pot_blk = if spec.style == BuildStyle::Japanese && (u + f) % 2 == 0 {
                                "minecraft:potted_bamboo"
                            } else if noise.cell_hash(u + 3, f, v) > 0.55 {
                                "minecraft:potted_flowering_azalea_bush"
                            } else {
                                style_pot(spec.style)
                            };
                            set_uv(blocks, grid, ou, win_y0, ov, PlannedBlock::simple(pot_blk));
                        } else {
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                win_y0 - 1,
                                ov,
                                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou, win_y0 - 1, ov), in_dir, "top"),
                            );
                        }
                    } else if spec.exterior_depth
                        && wall_base_solid
                        && !matches!(spec.style, BuildStyle::Modern)
                        && !is_solid_or_occupied(blocks, grid, ou, win_y0 - 1, ov)
                    {
                        let sill_blk = if matches!(spec.style, BuildStyle::Gothic | BuildStyle::Classical)
                            || td_id == "minecraft:iron_trapdoor"
                        {
                            make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou, win_y0 - 1, ov), in_dir, "top")
                        } else {
                            make_trapdoor(&td_id, out_dir, "top", false)
                        };
                        set_uv(blocks, grid, ou, win_y0 - 1, ov, sill_blk);
                    }

                    if spec.exterior_depth && !is_solid_or_occupied(blocks, grid, ou, clamped_y1 + 1, ov) {
                        if matches!(spec.style, BuildStyle::Gothic | BuildStyle::Classical) {
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                clamped_y1 + 1,
                                ov,
                                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou, clamped_y1 + 1, ov), in_dir, "top"),
                            );
                        } else if spec.style == BuildStyle::Japanese {
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                clamped_y1 + 1,
                                ov,
                                make_slab(&pal.pick_slab(MaterialPalette::SLOT_FLOOR, seed, ou, clamped_y1 + 1, ov), "top"),
                            );
                        } else if spec.shutters
                            && td_id != "minecraft:iron_trapdoor"
                            && !matches!(spec.style, BuildStyle::Modern)
                        {
                            for side in [-1, 1] {
                                let su = u + tan_u * side;
                                let sv = v + tan_v * side;
                                let sou = ou + tan_u * side;
                                let sov = ov + tan_v * side;
                                let side_solid_wall = is_fp(grid, cur_fp, su, sv)
                                    && !is_fp(grid, cur_fp, sou, sov)
                                    && !(f == 0 && is_fp(grid, fp, sou, sov))
                                    && get_uv(blocks, grid, su, win_y0, sv).is_some_and(|b| {
                                        let bid = b.block_id.as_str();
                                        bid != "minecraft:air"
                                            && !bid.contains("glass")
                                            && !bid.ends_with("_door")
                                            && bid != "minecraft:iron_bars"
                                    });
                                if side_solid_wall
                                    && !is_solid_or_occupied(blocks, grid, sou, win_y0, sov)
                                {
                                    set_uv(
                                        blocks,
                                        grid,
                                        sou,
                                        win_y0,
                                        sov,
                                        make_trapdoor(&pal.pick(MaterialPalette::SLOT_TRAPDOOR, seed, sou, win_y0, sov), out_dir, "top", true),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Supported 2nd-floor Balcony with side rail returns, planters & under-brackets!
    if spec.balcony
        && spec.floors >= 2
        && let Some((du, dv)) = best_door
    {
        let upper_mask = upper_fp.unwrap_or(fp);
        if is_fp(grid, upper_mask, du, dv) {
            let balc_y = i32::from(spec.foundation_height) + i32::from(spec.floor_height);
            for bu in (du - 2)..=(du + 2) {
                let bv = dv + 1;
                if grid.in_bounds(bu, bv)
                    && !is_fp(grid, fp, bu, bv)
                    && !is_fp(grid, upper_mask, bu, bv)
                {
                    set_uv(blocks, grid, bu, balc_y, bv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, bu, balc_y, bv)));
                    set_uv(
                        blocks,
                        grid,
                        bu,
                        balc_y - 1,
                        bv,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, bu, balc_y - 1, bv), grid.dir_back(), "top"),
                    );
                    set_uv(
                        blocks,
                        grid,
                        bu,
                        balc_y + 1,
                        bv,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, bu, balc_y + 1, bv)),
                    );
                    if (bu == du - 2 || bu == du + 2)
                        && !is_solid_or_occupied(blocks, grid, bu, balc_y + 2, bv)
                    {
                        let balc_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, bu, balc_y + 2, bv);
                        let top_pot = if bu < du {
                            "minecraft:potted_flowering_azalea_bush".to_owned()
                        } else if is_lantern(&balc_light) {
                            balc_light
                        } else {
                            style_pot(spec.style).to_owned()
                        };
                        set_uv(blocks, grid, bu, balc_y + 2, bv, PlannedBlock::simple(top_pot));
                    }
                }
            }
            set_uv(blocks, grid, du, balc_y + 1, dv, PlannedBlock::simple("minecraft:air"));
            set_uv(blocks, grid, du, balc_y + 2, dv, PlannedBlock::simple("minecraft:air"));
            set_uv(blocks, grid, du, balc_y + 1, dv + 1, PlannedBlock::simple("minecraft:air"));
        }
    }
}

/// Finds a clear `rw x rd` courtyard rectangle `(left_u, top_v)` outside walls, occupied props, and the front walkway.
fn find_clear_rect(
    blocks: &BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    best_door: Option<(i32, i32)>,
    b_v1: i32,
    rw: i32,
    rd: i32,
) -> Option<(i32, i32)> {
    let mut best: Option<(i32, i32, i32)> = None;
    for v0 in 1..=(grid.d - rd - 1) {
        for u0 in 1..=(grid.w - rw - 1) {
            if let Some((du, dv)) = best_door
                && ((u0..=(u0 + rw - 1)).any(|u| (u - du).abs() <= 2))
                && (v0 + rd - 1) >= dv
            {
                continue;
            }
            let ok = (0..rd).all(|dv| {
                (0..rw).all(|du| {
                    !is_fp(grid, fp, u0 + du, v0 + dv)
                        && !is_solid_or_occupied(blocks, grid, u0 + du, 0, v0 + dv)
                        && !is_solid_or_occupied(blocks, grid, u0 + du, 1, v0 + dv)
                        && !is_solid_or_occupied(blocks, grid, u0 + du, 2, v0 + dv)
                })
            });
            if ok {
                let center_u = u0 + rw / 2;
                let edge_bias = if u0 == 1 || u0 + rw >= grid.w - 1 || v0 == 1 { 2 } else { 0 };
                let score = if v0 > b_v1 { 0 } else { 10 } + edge_bias + (center_u - grid.w / 2).abs();
                if best.is_none_or(|(_, _, bs)| score < bs) {
                    best = Some((u0, v0, score));
                }
            }
        }
    }
    best.map(|(u, v, _)| (u, v))
}

/// Finds a clear 3x3 courtyard pad center outside walls and front door walkway.
fn find_clear_3x3(
    blocks: &BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    best_door: Option<(i32, i32)>,
    b_v1: i32,
) -> Option<(i32, i32)> {
    find_clear_rect(blocks, grid, fp, best_door, b_v1, 3, 3).map(|(u, v)| (u + 1, v + 1))
}

/// Finds a clear `3x2` horizontal strip `(left_u..=left_u+2, v0..=v0+1)` outside walls and door path.
fn find_clear_3x2(
    blocks: &BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    best_door: Option<(i32, i32)>,
    b_v1: i32,
) -> Option<(i32, i32)> {
    find_clear_rect(blocks, grid, fp, best_door, b_v1, 3, 2)
}

/// Builds front pathway, Torii/courtyard gates, flanking lantern posts, roofed well, wheat farmland, pergola, and bushes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_courtyard_and_landscaping(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    noise: NoiseField,
) {
    if spec.on_stilts {
        return;
    }
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let base_y = i32::from(spec.foundation_height);

    if let Some((du, dv)) = best_door {
        for pv in (dv + 2)..grid.d {
            let sway = if pv >= dv + 4 && grid.w >= 13 && spec.style != BuildStyle::Modern {
                match (pv + du) % 4 {
                    1 => 1,
                    3 => -1,
                    _ => 0,
                }
            } else {
                0
            };
            let pu = (du + sway).clamp(1, grid.w - 2);
            if !is_fp(grid, fp, pu, pv) && !is_solid_or_occupied(blocks, grid, pu, 1, pv) {
                let p_blk = if spec.style != BuildStyle::Modern && spec.style != BuildStyle::Desert {
                    match (pv - dv).abs() % 3 {
                        0 => pal.pick(MaterialPalette::SLOT_PAVING, seed, pu, 0, pv),
                        1 => pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, pu, 0, pv),
                        _ => "minecraft:dirt_path".to_owned(),
                    }
                } else {
                    pal.pick(MaterialPalette::SLOT_PAVING, seed, pu, 0, pv)
                };
                set_uv(blocks, grid, pu, 0, pv, PlannedBlock::simple(p_blk));
            }
        }
        if spec.garden_lanterns {
            for side in [-1, 1] {
                let lu = du + side * 3;
                let lv = (dv + 2).min(grid.d - 1);
                if grid.in_bounds(lu, lv)
                    && !is_fp(grid, fp, lu, lv)
                    && !is_solid_or_occupied(blocks, grid, lu, 1, lv)
                    && !is_solid_or_occupied(blocks, grid, lu, 2, lv)
                    && !is_solid_or_occupied(blocks, grid, lu, 3, lv)
                {
                    let light_id = pal.pick(MaterialPalette::SLOT_LIGHT, seed, lu, 3, lv);
                    set_uv(blocks, grid, lu, 1, lv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lu, 1, lv)));
                    set_uv(
                        blocks,
                        grid,
                        lu,
                        2,
                        lv,
                        PlannedBlock::simple(if is_lantern(&light_id) {
                            pal.pick(MaterialPalette::SLOT_FENCE, seed, lu, 2, lv)
                        } else {
                            pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lu, 2, lv)
                        }),
                    );
                    set_uv(blocks, grid, lu, 3, lv, PlannedBlock::simple(light_id));
                }
            }
        }

        // Japanese Crimson Torii gate framing the front walkway when yard space allows.
        let tv = (dv + 3).min(grid.d - 2);
        if spec.style == BuildStyle::Japanese
            && tv >= dv + 2
            && du >= 2
            && du + 2 < grid.w
            && (-1..=1).all(|off| {
                !is_fp(grid, fp, du + off, tv)
                    && (1..=4).all(|y| !is_solid_or_occupied(blocks, grid, du + off, y, tv))
            })
        {
            for side in [-1, 1] {
                set_uv(
                    blocks,
                    grid,
                    du + side,
                    1,
                    tv,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, du + side, 1, tv)),
                );
                for y in 2..=3 {
                    set_uv(
                        blocks,
                        grid,
                        du + side,
                        y,
                        tv,
                        PlannedBlock::simple("minecraft:crimson_planks"),
                    );
                }
            }
            set_uv(
                blocks,
                grid,
                du,
                3,
                tv,
                PlannedBlock::simple("minecraft:crimson_planks"),
            );
            for off in -2..=2 {
                if grid.in_bounds(du + off, tv)
                    && !is_solid_or_occupied(blocks, grid, du + off, 4, tv)
                {
                    let lintel = if off.abs() == 2 {
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, du + off, 4, tv), "bottom")
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, du + off, 4, tv))
                    };
                    set_uv(blocks, grid, du + off, 4, tv, lintel);
                }
            }
        }
    }

    // Perimeter courtyard fence with matching front gate (or trimmed hedges / stone piers on Modern & Classical).
    if b_u0 >= 2
        && b_u1 <= grid.w - 3
        && b_v0 >= 2
        && b_v1 <= grid.d - 3
        && grid.w >= 19
        && grid.d >= 17
        && !matches!(spec.category, BuildCategory::Bridge)
    {
        if matches!(spec.style, BuildStyle::Modern | BuildStyle::Classical) {
            for &(hu, hv) in &[
                (1, grid.d - 2),
                (2, grid.d - 2),
                (grid.w - 3, grid.d - 2),
                (grid.w - 2, grid.d - 2),
                (1, 1),
                (grid.w - 2, 1),
            ] {
                if !is_fp(grid, fp, hu, hv)
                    && best_door.is_none_or(|(du, dv)| (hu - du).abs() > 2 || hv < dv)
                    && !is_solid_or_occupied(blocks, grid, hu, 1, hv)
                {
                    let hedge = if (hu == 1 || hu == grid.w - 2) && spec.style == BuildStyle::Classical {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, hu, 1, hv))
                    } else if (hu + hv) % 2 == 0 {
                        make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, hu, 1, hv))
                    } else {
                        make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES_ALT, seed, hu, 1, hv))
                    };
                    set_uv(blocks, grid, hu, 1, hv, hedge);
                }
            }
        } else {
            for v in 1..=(grid.d - 2) {
                for u in 1..=(grid.w - 2) {
                    let on_ring = u == 1 || u == grid.w - 2 || v == 1 || v == grid.d - 2;
                    if !on_ring
                        || is_fp(grid, fp, u, v)
                        || is_fp(grid, fp, u - 1, v)
                        || is_fp(grid, fp, u + 1, v)
                        || is_fp(grid, fp, u, v - 1)
                        || is_fp(grid, fp, u, v + 1)
                        || is_solid_or_occupied(blocks, grid, u, 1, v)
                        || is_solid_or_occupied(blocks, grid, u, 2, v)
                    {
                        continue;
                    }
                    if v == grid.d - 2
                        && let Some((du, _)) = best_door
                        && u == du
                    {
                        let gate_id = if pal.has_extra(MaterialPalette::SLOT_FENCE_GATE) {
                            pal.pick(MaterialPalette::SLOT_FENCE_GATE, seed, u, 1, v)
                        } else {
                            pal.fence_gate_for_fence().to_owned()
                        };
                        set_uv(
                            blocks,
                            grid,
                            u,
                            1,
                            v,
                            PlannedBlock::with_props(
                                gate_id,
                                vec![
                                    ("facing", grid.dir_front()),
                                    ("in_wall", "false"),
                                    ("open", "false"),
                                    ("powered", "false"),
                                ],
                            ),
                        );
                        continue;
                    }
                    let is_corner = (u == 1 || u == grid.w - 2) && (v == 1 || v == grid.d - 2);
                    if is_corner {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, 1, v)));
                        set_uv(blocks, grid, u, 2, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, 2, v), "bottom"));
                    } else {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, 1, v)));
                    }
                }
            }
        }
    }

    // Ceremonial flanking approach braziers on Classical & Temple estates.
    if (spec.style == BuildStyle::Classical || spec.category == BuildCategory::Temple)
        && let Some((du, dv)) = best_door
    {
        for side in [-3, 3] {
            let bu = du + side;
            let bv = (dv + 3).min(grid.d - 2);
            if grid.in_bounds(bu, bv)
                && !is_fp(grid, fp, bu, bv)
                && !is_solid_or_occupied(blocks, grid, bu, 1, bv)
                && !is_solid_or_occupied(blocks, grid, bu, 2, bv)
            {
                set_uv(blocks, grid, bu, 1, bv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, bu, 1, bv)));
                set_uv(
                    blocks,
                    grid,
                    bu,
                    2,
                    bv,
                    PlannedBlock::with_props(
                        "minecraft:campfire",
                        vec![("lit", "true"), ("signal_fire", "false"), ("facing", grid.dir_front())],
                    ),
                );
            }
        }
    }

    // Roofed stone well or Modern reflecting pool + poolside lounge seat in clear yard space!
    if spec.courtyard_well
        && let Some((wu, wv)) = find_clear_3x3(blocks, grid, fp, best_door, b_v1)
    {
        if spec.style == BuildStyle::Modern {
            for dv in -1..=1 {
                for du in -1..=1 {
                    let u = wu + du;
                    let v = wv + dv;
                    if (du == 0 && dv == 0) || (du == -1 && dv == 0) {
                        set_uv(blocks, grid, u, 0, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                        set_uv(blocks, grid, u, -1, v, PlannedBlock::simple("minecraft:sea_lantern"));
                    } else {
                        set_uv(blocks, grid, u, 0, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 0, v)));
                    }
                }
            }
            set_uv(
                blocks,
                grid,
                wu + 1,
                1,
                wv,
                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, wu + 1, 1, wv), grid.dir_right(), "bottom"),
            );
            set_uv(
                blocks,
                grid,
                wu + 1,
                1,
                wv - 1,
                PlannedBlock::simple("minecraft:potted_azalea_bush"),
            );
        } else {
            for dv in -1..=1 {
                for du in -1..=1 {
                    let u = wu + du;
                    let v = wv + dv;
                    if du == 0 && dv == 0 {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            1,
                            v,
                            PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]),
                        );
                        set_uv(
                            blocks,
                            grid,
                            u,
                            2,
                            v,
                            PlannedBlock::with_props("minecraft:iron_chain", vec![("axis", "y")]),
                        );
                        set_uv(blocks, grid, u, 3, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, 3, v)));
                    } else if du.abs() == 1 && dv.abs() == 1 {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, 1, v)));
                        set_uv(blocks, grid, u, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, 2, v)));
                        set_uv(blocks, grid, u, 3, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, 3, v), "bottom"));
                    } else {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, 1, v)));
                        set_uv(blocks, grid, u, 3, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, 3, v), "bottom"));
                    }
                }
            }
        }
    }

    // Irrigated wheat farmland patch beside barns and windmills.
    if (spec.category == BuildCategory::Barn || spec.theme == StructureTheme::Windmill)
        && let Some((fu, fv)) = find_clear_3x3(blocks, grid, fp, best_door, b_v1)
    {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = fu + du;
                let v = fv + dv;
                if du == 0 && dv == 0 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        0,
                        v,
                        PlannedBlock::with_props("minecraft:water", vec![("level", "0")]),
                    );
                } else {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        0,
                        v,
                        PlannedBlock::with_props("minecraft:farmland", vec![("moisture", "7")]),
                    );
                    let age = match (u + v).abs() % 3 {
                        0 => "7",
                        1 => "6",
                        _ => "5",
                    };
                    set_uv(
                        blocks,
                        grid,
                        u,
                        1,
                        v,
                        PlannedBlock::with_props("minecraft:wheat", vec![("age", age)]),
                    );
                }
            }
        }
    }

    // Flower pergola arbor on spacious Cottage, Medieval, or Pavilion plots.
    if grid.w >= 19
        && grid.d >= 17
        && (matches!(spec.style, BuildStyle::Cottage | BuildStyle::Medieval)
            || spec.category == BuildCategory::Pavilion)
        && let Some((pu, pv)) = find_clear_3x3(blocks, grid, fp, best_door, b_v1)
    {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = pu + du;
                let v = pv + dv;
                if du.abs() == 1 && dv.abs() == 1 {
                    set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, 1, v)));
                    set_uv(blocks, grid, u, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, 2, v)));
                    set_uv(blocks, grid, u, 3, v, make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, u, 3, v)));
                } else if du == 0 || dv == 0 {
                    set_uv(blocks, grid, u, 3, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, 3, v), "top"));
                }
            }
        }
        set_uv(blocks, grid, pu, 1, pv, PlannedBlock::simple(style_pot(spec.style)));
        set_uv(
            blocks,
            grid,
            pu - 1,
            1,
            pv,
            make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, pu - 1, 1, pv), grid.dir_right(), "bottom"),
        );
    }

    // Forge Outdoor Smithing Workyard vs Barn/Windmill Hay & Trough Farmstead vs Tavern Beer Garden.
    if spec.category == BuildCategory::Forge {
        if let Some((su, sv)) = find_clear_3x2(blocks, grid, fp, best_door, b_v1) {
            for dv in 0..=1 {
                for du in 0..=2 {
                    set_uv(blocks, grid, su + du, 0, sv + dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, su + du, 0, sv + dv)));
                }
            }
            set_uv(blocks, grid, su, 1, sv + 1, PlannedBlock::with_props("minecraft:anvil", vec![("facing", grid.dir_front())]));
            set_uv(blocks, grid, su + 1, 1, sv + 1, PlannedBlock::simple("minecraft:lava_cauldron"));
            set_uv(blocks, grid, su + 2, 1, sv + 1, PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]));
            set_uv(blocks, grid, su, 1, sv, PlannedBlock::with_props("minecraft:grindstone", vec![("face", "floor"), ("facing", grid.dir_front())]));
            set_uv(blocks, grid, su + 1, 1, sv, PlannedBlock::with_props("minecraft:dark_oak_log", vec![("axis", "x")]));
            set_uv(blocks, grid, su + 2, 1, sv, PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
        } else if let Some((su, sv)) = find_clear_rect(blocks, grid, fp, best_door, b_v1, 2, 2) {
            set_uv(blocks, grid, su, 1, sv, PlannedBlock::with_props("minecraft:anvil", vec![("facing", grid.dir_front())]));
            set_uv(blocks, grid, su + 1, 1, sv, PlannedBlock::simple("minecraft:lava_cauldron"));
            set_uv(blocks, grid, su, 1, sv + 1, PlannedBlock::with_props("minecraft:grindstone", vec![("face", "floor"), ("facing", grid.dir_front())]));
            set_uv(blocks, grid, su + 1, 1, sv + 1, PlannedBlock::with_props("minecraft:dark_oak_log", vec![("axis", "x")]));
        }
    } else if spec.category == BuildCategory::Barn || spec.theme == StructureTheme::Windmill {
        if let Some((su, sv)) = find_clear_rect(blocks, grid, fp, best_door, b_v1, 2, 2) {
            set_uv(blocks, grid, su, 1, sv, PlannedBlock::simple("minecraft:composter"));
            set_uv(blocks, grid, su + 1, 1, sv, PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]));
            set_uv(blocks, grid, su, 1, sv + 1, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su, 2, sv + 1, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su + 1, 1, sv + 1, PlannedBlock::simple("minecraft:hay_block"));
        } else if let Some((su, sv)) = find_clear_rect(blocks, grid, fp, best_door, b_v1, 3, 1) {
            set_uv(blocks, grid, su, 1, sv, PlannedBlock::simple("minecraft:composter"));
            set_uv(blocks, grid, su + 1, 1, sv, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su + 1, 2, sv, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su + 2, 1, sv, PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]));
        } else if let Some((su, sv)) = find_clear_rect(blocks, grid, fp, best_door, b_v1, 1, 3) {
            set_uv(blocks, grid, su, 1, sv + 2, PlannedBlock::simple("minecraft:composter"));
            set_uv(blocks, grid, su, 1, sv + 1, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su, 2, sv + 1, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, su, 1, sv, PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]));
        } else if let Some((hu, hv)) = find_clear_rect(blocks, grid, fp, best_door, b_v1, 1, 1) {
            set_uv(blocks, grid, hu, 1, hv, PlannedBlock::simple("minecraft:hay_block"));
            set_uv(blocks, grid, hu, 2, hv, PlannedBlock::simple("minecraft:hay_block"));
        }
    } else if (spec.theme == StructureTheme::Tavern || matches!(spec.category, BuildCategory::House | BuildCategory::Pavilion))
        && grid.w >= 17
        && grid.d >= 15
        && !matches!(spec.style, BuildStyle::Modern | BuildStyle::Desert)
        && let Some((tu0, tv0)) = find_clear_3x2(blocks, grid, fp, best_door, b_v1)
    {
        // Outdoor beer-garden table or shaded courtyard bistro bench!
        let tu = tu0 + 1;
        let tv = tv0 + 1;
        set_uv(blocks, grid, tu, 1, tv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, tu, 1, tv)));
        set_uv(
            blocks,
            grid,
            tu,
            2,
            tv,
            if spec.theme == StructureTheme::Tavern {
                PlannedBlock::with_props("minecraft:sea_pickle", vec![("pickles", "2"), ("waterlogged", "false")])
            } else {
                PlannedBlock::simple(style_pot(spec.style))
            },
        );
        set_uv(blocks, grid, tu - 1, 1, tv, make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, tu - 1, 1, tv), grid.dir_left(), "bottom"));
        set_uv(blocks, grid, tu + 1, 1, tv, make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, tu + 1, 1, tv), grid.dir_right(), "bottom"));
        if spec.theme == StructureTheme::Tavern {
            set_uv(blocks, grid, tu, 1, tv - 1, PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
            set_uv(blocks, grid, tu, 2, tv - 1, PlannedBlock::with_props("minecraft:cake", vec![("bites", "0")]));
        }
    }

    let _ = base_y;

    // Zen garden spring / pond with lily pad, stepping stones & sugar cane reeds on remaining clear 3x3 plot.
    if grid.w >= 19
        && grid.d >= 17
        && spec.style != BuildStyle::Modern
        && spec.category != BuildCategory::Bridge
        && (spec.courtyard_garden || matches!(spec.style, BuildStyle::Japanese | BuildStyle::Cottage))
        && let Some((pu, pv)) = find_clear_3x3(blocks, grid, fp, best_door, b_v1)
    {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = pu + du;
                let v = pv + dv;
                if du == 0 && dv == 0 {
                    set_uv(blocks, grid, u, -1, v, PlannedBlock::simple("minecraft:sea_lantern"));
                    set_uv(
                        blocks,
                        grid,
                        u,
                        0,
                        v,
                        PlannedBlock::with_props("minecraft:water", vec![("level", "0")]),
                    );
                    set_uv(blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:lily_pad"));
                } else {
                    let bank = if spec.style == BuildStyle::Japanese && (du == 0 || dv == 0) {
                        "minecraft:stone_bricks"
                    } else if (du + dv) % 2 == 0 {
                        "minecraft:moss_block"
                    } else {
                        "minecraft:gravel"
                    };
                    set_uv(blocks, grid, u, 0, v, PlannedBlock::simple(bank));
                    if spec.style == BuildStyle::Japanese && du == -1 && dv == -1 && !is_solid_or_occupied(blocks, grid, u, 1, v) {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, 1, v)));
                        let pond_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, 2, v);
                        if is_lantern(&pond_light) {
                            set_uv(blocks, grid, u, 2, v, PlannedBlock::simple(pond_light));
                        }
                    } else if du == 1 && dv == 1 && !is_solid_or_occupied(blocks, grid, u, 1, v) {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            1,
                            v,
                            PlannedBlock::with_props("minecraft:sugar_cane", vec![("age", "0")]),
                        );
                        if !is_solid_or_occupied(blocks, grid, u, 2, v) {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                2,
                                v,
                                PlannedBlock::with_props("minecraft:sugar_cane", vec![("age", "0")]),
                            );
                        }
                    }
                }
            }
        }
    }

    // Lush planter clusters (moss bases + azalea / bamboo / potted bushes), wildflowers, grass tufts & hanging vines!
    if (spec.courtyard_garden || spec.overgrown)
        && spec.ruin_erosion < 0.1
        && spec.category != BuildCategory::Bridge
    {
        let mut planter_count = 0;
        let mut vine_count = 0;
        for v in 1..(grid.d - 1) {
            for u in 1..(grid.w - 1) {
                let only_engawa_slab = spec.style == BuildStyle::Japanese
                    && get_uv(blocks, grid, u, base_y, v).is_some_and(|b| b.block_id.ends_with("_slab"))
                    && !is_solid_or_occupied(blocks, grid, u, base_y + 1, v)
                    && !is_solid_or_occupied(blocks, grid, u, base_y + 2, v);
                let ground_free = !is_solid_or_occupied(blocks, grid, u, 1, v)
                    && !is_solid_or_occupied(blocks, grid, u, base_y + 1, v);
                if is_fp(grid, fp, u, v) || (!ground_free && !only_engawa_slab) {
                    continue;
                }
                let near_wall = is_fp(grid, fp, u - 1, v)
                    || is_fp(grid, fp, u + 1, v)
                    || is_fp(grid, fp, u, v - 1)
                    || is_fp(grid, fp, u, v + 1);
                if near_wall {
                    if let Some((du, dv)) = best_door
                        && (u - du).abs() <= 2
                        && v >= dv
                    {
                        continue;
                    }
                    if vine_count < 6
                        && (spec.overgrown || spec.style == BuildStyle::Cottage)
                        && (u + v) % 3 == 1
                        && !is_solid_or_occupied(blocks, grid, u, base_y + 2, v)
                    {
                        let wall_adj = [
                            (0, -1, grid.dir_back()),
                            (0, 1, grid.dir_front()),
                            (-1, 0, grid.dir_left()),
                            (1, 0, grid.dir_right()),
                        ]
                        .into_iter()
                        .find(|&(wdu, wdv, _)| {
                            is_fp(grid, fp, u + wdu, v + wdv)
                                && get_uv(blocks, grid, u + wdu, base_y + 2, v + wdv).is_some_and(|b| {
                                    b.block_id != "minecraft:air"
                                        && !b.block_id.contains("glass")
                                        && !b.block_id.ends_with("_door")
                                        && !b.block_id.ends_with("_fence")
                                })
                        });
                        if let Some((_, _, vdir)) = wall_adj {
                            let props = vec![
                                ("north", if vdir == "north" { "true" } else { "false" }),
                                ("south", if vdir == "south" { "true" } else { "false" }),
                                ("east", if vdir == "east" { "true" } else { "false" }),
                                ("west", if vdir == "west" { "true" } else { "false" }),
                                ("up", "false"),
                            ];
                            set_uv(
                                blocks,
                                grid,
                                u,
                                base_y + 2,
                                v,
                                PlannedBlock::with_props("minecraft:vine", props.clone()),
                            );
                            if !is_solid_or_occupied(blocks, grid, u, base_y + 1, v) {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    base_y + 1,
                                    v,
                                    PlannedBlock::with_props("minecraft:vine", props),
                                );
                            }
                            vine_count += 1;
                            continue;
                        }
                    }
                    if planter_count < 6
                        && (only_engawa_slab || noise.cell_hash(u, 5, v) > 0.72)
                        && !is_solid_or_occupied(blocks, grid, u, 2, v)
                        && !is_solid_or_occupied(blocks, grid, u, base_y + 2, v)
                    {
                        let py = if only_engawa_slab { base_y } else { 1 };
                        if !is_solid_or_occupied(blocks, grid, u, py + 1, v) {
                            set_uv(blocks, grid, u, py, v, PlannedBlock::simple("minecraft:moss_block"));
                            if spec.style == BuildStyle::Japanese && planter_count % 2 == 0 {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    py + 1,
                                    v,
                                    PlannedBlock::with_props(
                                        "minecraft:bamboo",
                                        vec![("age", "0"), ("leaves", "small"), ("stage", "0")],
                                    ),
                                );
                                if !is_solid_or_occupied(blocks, grid, u, py + 2, v) {
                                    set_uv(
                                        blocks,
                                        grid,
                                        u,
                                        py + 2,
                                        v,
                                        PlannedBlock::with_props(
                                            "minecraft:bamboo",
                                            vec![("age", "0"), ("leaves", "large"), ("stage", "0")],
                                        ),
                                    );
                                }
                            } else {
                                let shrub = if (u + v) % 2 == 0 {
                                    "minecraft:flowering_azalea"
                                } else {
                                    "minecraft:azalea"
                                };
                                set_uv(blocks, grid, u, py + 1, v, PlannedBlock::simple(shrub));
                            }
                            planter_count += 1;
                        }
                    } else if spec.overgrown
                        && ground_free
                        && noise.fbm_2d(u as f64 * 0.55, v as f64 * 0.55, 2) > 0.58
                    {
                        set_uv(blocks, grid, u, 1, v, make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, u, 1, v)));
                        if noise.cell_hash(u, 2, v) > 0.65 && !is_solid_or_occupied(blocks, grid, u, 2, v) {
                            set_uv(blocks, grid, u, 2, v, make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, u, 2, v)));
                        }
                    }
                } else if ground_free
                    && !is_solid_or_occupied(blocks, grid, u, 0, v)
                    && best_door.is_none_or(|(du, dv)| (u - du).abs() > 2 || v < dv)
                {
                    let fh = noise.cell_hash(u, 7, v);
                    if fh > 0.91 {
                        let dec = if spec.style == BuildStyle::Japanese {
                            PlannedBlock::with_props(
                                "minecraft:pink_petals",
                                vec![("facing", grid.dir_front()), ("flower_amount", "3")],
                            )
                        } else if matches!(spec.style, BuildStyle::Cottage | BuildStyle::Medieval) {
                            PlannedBlock::with_props(
                                "minecraft:wildflowers",
                                vec![("facing", grid.dir_front()), ("flower_amount", "3")],
                            )
                        } else {
                            PlannedBlock::with_props(
                                "minecraft:leaf_litter",
                                vec![("facing", grid.dir_front()), ("segment_amount", "3")],
                            )
                        };
                        set_uv(blocks, grid, u, 1, v, dec);
                    } else if fh > 0.85 {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:fern"));
                    } else if fh > 0.77 {
                        set_uv(blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:short_grass"));
                    }
                }
            }
        }
    }

    let _ = get_uv(blocks, grid, 0, 0, 0);
}

/// Adds a striped cloth street market awning and open counter stall beside the main entrance.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_market_awning_stall(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
) {
    if !spec.market_awning || spec.ruin_erosion >= 0.1 || spec.category == BuildCategory::Bridge {
        return;
    }
    let ground_y = i32::from(spec.foundation_height);
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    if b_v1 + 2 >= grid.d {
        return;
    }

    let mut stall_anchor: Option<(i32, i32, i32)> = None;
    for width_try in [4, 3] {
        for v in (b_v0 + 2..=b_v1).rev() {
            if v + 2 >= grid.d {
                continue;
            }
            for su in (b_u0 + 1)..=(b_u1 - width_try) {
                let valid_wall = (0..width_try).all(|k| {
                    let u = su + k;
                    is_fp(grid, fp, u, v)
                        && !is_fp(grid, fp, u, v + 1)
                        && !is_fp(grid, fp, u, v + 2)
                        && best_door.is_none_or(|(du, dv)| (u - du).abs() >= 2 || (v - dv).abs() >= 2)
                });
                if !valid_wall {
                    continue;
                }
                let front_clear = (0..width_try).all(|k| {
                    let u = su + k;
                    !is_solid_or_occupied(blocks, grid, u, ground_y + 2, v + 1)
                        && !is_solid_or_occupied(blocks, grid, u, ground_y + 3, v + 1)
                        && !is_solid_or_occupied(blocks, grid, u, ground_y + 2, v + 2)
                        && !is_solid_or_occupied(blocks, grid, u, ground_y + 3, v + 2)
                });
                if front_clear {
                    stall_anchor = Some((su, v, width_try));
                    break;
                }
            }
            if stall_anchor.is_some() {
                break;
            }
        }
        if stall_anchor.is_some() {
            break;
        }
    }

    if let Some((su, sv, aw_w)) = stall_anchor {
        for k in 0..aw_w {
            let u = su + k;
            let wool1 = if pal.has_extra(MaterialPalette::SLOT_AWNING) {
                pal.pick(MaterialPalette::SLOT_AWNING, seed, u, ground_y + 3, sv + 1)
            } else if k % 2 == 0 {
                pal.awning.clone()
            } else {
                "minecraft:white_wool".to_owned()
            };
            let wool2 = if pal.has_extra(MaterialPalette::SLOT_AWNING) {
                pal.pick(MaterialPalette::SLOT_AWNING, seed, u, ground_y + 3, sv + 2)
            } else if k % 2 == 0 {
                pal.awning.clone()
            } else {
                "minecraft:white_wool".to_owned()
            };
            set_uv(blocks, grid, u, ground_y + 3, sv + 1, PlannedBlock::simple(wool1));
            set_uv(blocks, grid, u, ground_y + 3, sv + 2, PlannedBlock::simple(wool2));
            if k == 0 || k == aw_w - 1 {
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 1,
                    sv + 2,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, ground_y + 1, sv + 2)),
                );
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 2,
                    sv + 2,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, ground_y + 2, sv + 2)),
                );
            } else {
                // Open shop pass-through counter along ground wall + hanging awning lanterns + stools!
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 1,
                    sv,
                    make_slab(&pal.pick_slab(MaterialPalette::SLOT_FLOOR, seed, u, ground_y + 1, sv), "top"),
                );
                set_uv(blocks, grid, u, ground_y + 2, sv, PlannedBlock::simple("minecraft:air"));
                let awning_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, ground_y + 2, sv + 1);
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 2,
                    sv + 1,
                    if k == 1 && is_lantern(&awning_light) {
                        PlannedBlock::with_props(awning_light, vec![("hanging", "true")])
                    } else {
                        PlannedBlock::simple("minecraft:spore_blossom")
                    },
                );
                set_uv(
                    blocks,
                    grid,
                    u,
                    ground_y + 1,
                    sv + 1,
                    if k == 1 {
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_FLOOR, seed, u, ground_y + 1, sv + 1), "bottom")
                    } else {
                        PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")])
                    },
                );
            }
        }
    }
}

/// Adds cantilevered upper lantern outrigger cranes extending asymmetrically from upper walls.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lantern_cranes(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    tall_fp: &[bool],
    tall_bounds: [i32; 4],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    roof_base_y: i32,
    height: u16,
) {
    if spec.ruin_erosion >= 0.1
        || spec.category == BuildCategory::Bridge
        || matches!(spec.style, BuildStyle::Modern | BuildStyle::Desert)
        || !matches!(
            spec.category,
            BuildCategory::House | BuildCategory::Forge | BuildCategory::Castle | BuildCategory::Temple
        )
        || (spec.floors < 2 && height < 12)
    {
        return;
    }

    let ground_y = i32::from(spec.foundation_height);
    let [t_u0, t_v0, t_u1, t_v1] = tall_bounds;
    let crane_y = (roof_base_y - 2).clamp(ground_y + 4, i32::from(height) - 3);
    let mid_v = (t_v0 + t_v1) / 2;

    for (idx, &(wu, dir_u, pref_v, alt_lantern)) in [
        (t_u0, -1, (t_v0 + 2).min(t_v1), "minecraft:soul_lantern"),
        (t_u1, 1, (t_v1 - 2).max(t_v0), "minecraft:copper_lantern"),
    ]
    .iter()
    .enumerate()
    {
        let mut candidates_v: Vec<i32> = (t_v0..=t_v1).collect();
        candidates_v.sort_by_key(|&v| (v - pref_v).abs() + (v - mid_v).abs() / 3);

        for wv in candidates_v {
            if !is_fp(grid, tall_fp, wu, wv) || is_fp(grid, tall_fp, wu + dir_u, wv) {
                continue;
            }
            let anchor_solid = (crane_y - 1..=crane_y).all(|y| {
                get_uv(blocks, grid, wu, y, wv).is_some_and(|b| {
                    let bid = b.block_id.as_str();
                    bid != "minecraft:air" && !bid.contains("glass") && !bid.ends_with("_door")
                })
            });
            if !anchor_solid {
                continue;
            }
            let max_arm = if grid.in_bounds(wu + dir_u * 3, wv)
                && !is_fp(grid, fp, wu + dir_u * 3, wv)
                && (1..=3).all(|s| {
                    (crane_y - 2..=crane_y)
                        .all(|y| !is_solid_or_occupied(blocks, grid, wu + dir_u * s, y, wv))
                }) {
                3
            } else if grid.in_bounds(wu + dir_u * 2, wv)
                && !is_fp(grid, fp, wu + dir_u * 2, wv)
                && (1..=2).all(|s| {
                    (crane_y - 2..=crane_y)
                        .all(|y| !is_solid_or_occupied(blocks, grid, wu + dir_u * s, y, wv))
                }) {
                2
            } else {
                continue;
            };

            let brace_dir = grid.uv_to_mc_dir(-dir_u, 0);
            set_uv(
                blocks,
                grid,
                wu + dir_u,
                crane_y - 1,
                wv,
                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, wu + dir_u, crane_y - 1, wv), brace_dir, "top"),
            );
            for s in 1..max_arm {
                set_uv(
                    blocks,
                    grid,
                    wu + dir_u * s,
                    crane_y,
                    wv,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, wu + dir_u * s, crane_y, wv)),
                );
            }
            let tip_u = wu + dir_u * max_arm;
            set_uv(blocks, grid, tip_u, crane_y, wv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_FLOOR, seed, tip_u, crane_y, wv), "bottom"));
            set_uv(
                blocks,
                grid,
                tip_u,
                crane_y - 1,
                wv,
                PlannedBlock::with_props("minecraft:iron_chain", vec![("axis", "y")]),
            );
            let lantern_id = if pal.has_extra(MaterialPalette::SLOT_LIGHT) {
                pal.pick(MaterialPalette::SLOT_LIGHT, seed, tip_u, crane_y - 2, wv)
            } else if spec.style == BuildStyle::Japanese && idx == 0 {
                alt_lantern.to_owned()
            } else if spec.style == BuildStyle::Japanese || spec.style == BuildStyle::Steampunk {
                "minecraft:copper_lantern".to_owned()
            } else {
                pal.light.clone()
            };
            let lantern_blk = if is_lantern(&lantern_id) {
                PlannedBlock::with_props(lantern_id, vec![("hanging", "true")])
            } else {
                PlannedBlock::simple(lantern_id)
            };
            set_uv(blocks, grid, tip_u, crane_y - 2, wv, lantern_blk);
            break;
        }
    }
}

/// Projects a walkable 2nd-story 3-wide bay window (`oriel` / эркер) with corbels & cap.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_bay_windows(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    upper_fp: &[bool],
    upper_bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    stairwell_uv: &[(i32, i32)],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
) {
    if spec.floors < 2
        || !spec.exterior_depth
        || spec.ruin_erosion >= 0.1
        || matches!(spec.style, BuildStyle::Modern | BuildStyle::Desert)
        || spec.category == BuildCategory::Pavilion
    {
        return;
    }
    let floor_y = i32::from(spec.foundation_height) + i32::from(spec.floor_height);
    let [u0, _v0, u1, v1] = upper_bounds;
    if u1 - u0 < 6 {
        return;
    }

    // Try finding a 3-wide straight bay on front upper wall (`v == v1`) away from doorway & stairs.
    for bu in (u0 + 2)..=(u1 - 4) {
        let ov = v1 + 1;
        if ov >= grid.d {
            continue;
        }
        let valid_bay = (0..3).all(|k| {
            let u = bu + k;
            is_fp(grid, upper_fp, u, v1)
                && !is_fp(grid, upper_fp, u, ov)
                && !is_fp(grid, fp, u, ov)
                && !stairwell_uv.contains(&(u, v1))
                && best_door.is_none_or(|(du, _)| (u - du).abs() > 2)
                && (floor_y..=floor_y + 3)
                    .all(|y| !is_solid_or_occupied(blocks, grid, u, y, ov))
        });
        if !valid_bay {
            continue;
        }
        for k in 0..3 {
            let u = bu + k;
            set_uv(
                blocks,
                grid,
                u,
                floor_y - 1,
                ov,
                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, floor_y - 1, ov), grid.dir_back(), "top"),
            );
            set_uv(blocks, grid, u, floor_y, ov, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, floor_y, ov)));
            let bay_win = pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, floor_y + 1, ov);
            if k == 1 {
                set_uv(blocks, grid, u, floor_y + 1, v1, PlannedBlock::simple("minecraft:air"));
                set_uv(blocks, grid, u, floor_y + 2, v1, PlannedBlock::simple("minecraft:air"));
                set_uv(blocks, grid, u, floor_y + 1, ov, PlannedBlock::simple(bay_win.clone()));
                set_uv(blocks, grid, u, floor_y + 2, ov, PlannedBlock::simple(bay_win));
                set_uv(
                    blocks,
                    grid,
                    u,
                    floor_y + 3,
                    ov,
                    make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, floor_y + 3, ov), "bottom"),
                );
            } else {
                set_uv(blocks, grid, u, floor_y + 1, ov, make_frame_block_at(pal, "y", seed, u, floor_y + 1, ov));
                set_uv(blocks, grid, u, floor_y + 2, ov, PlannedBlock::simple(bay_win));
                set_uv(
                    blocks,
                    grid,
                    u,
                    floor_y + 3,
                    ov,
                    make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, floor_y + 3, ov), grid.dir_front(), "bottom"),
                );
            }
        }
        break;
    }
}

/// Adds vertical rainwater downspouts into cauldrons and hanging facade signs for taverns/shops.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_downspouts_and_signs(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
) {
    if spec.ruin_erosion >= 0.1 || spec.category == BuildCategory::Bridge {
        return;
    }
    let ground_y = i32::from(spec.foundation_height);
    let top_spout_y = ground_y + i32::from(spec.floor_height);
    let [b_u0, b_v0, b_u1, b_v1] = bounds;

    // 1. Rainwater downspout rod into `water_cauldron` beside a wall corner.
    let mut spout_done = false;
    for &(su, sv) in &[(b_u0 - 1, b_v0 + 1), (b_u1 + 1, b_v0 + 1), (b_u0 - 1, b_v1 - 1)] {
        if spout_done
            || !grid.in_bounds(su, sv)
            || is_fp(grid, fp, su, sv)
            || best_door.is_some_and(|(du, dv)| (su - du).abs() <= 2 && sv >= dv)
        {
            continue;
        }
        if (1..=top_spout_y).all(|y| !is_solid_or_occupied(blocks, grid, su, y, sv)) {
            set_uv(
                blocks,
                grid,
                su,
                1,
                sv,
                PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]),
            );
            for y in 2..top_spout_y {
                set_uv(
                    blocks,
                    grid,
                    su,
                    y,
                    sv,
                    PlannedBlock::with_props(
                        "minecraft:lightning_rod",
                        vec![("facing", "down"), ("powered", "false"), ("waterlogged", "false")],
                    ),
                );
            }
            spout_done = true;
        }
    }

    // 2. Hanging tavern / forge / shop / estate wooden swing sign beside doorway.
    if (spec.market_awning
        || (spec.courtyard_garden && spec.style != BuildStyle::Modern)
        || matches!(spec.category, BuildCategory::Forge)
        || matches!(spec.theme, StructureTheme::Tavern | StructureTheme::Library))
        && let Some((du, dv)) = best_door
    {
        let door_choice = pal.pick(MaterialPalette::SLOT_DOOR, seed, du, ground_y + 1, dv);
        let sign_id = match door_choice.as_str() {
            "minecraft:spruce_door" => "minecraft:spruce_hanging_sign",
            "minecraft:dark_oak_door" => "minecraft:dark_oak_hanging_sign",
            "minecraft:cherry_door" => "minecraft:cherry_hanging_sign",
            _ => "minecraft:oak_hanging_sign",
        };
        for side in [-2, 2, -3, 3] {
            let su = du + side;
            let sv = dv + 1;
            let anchor_wall_ok = get_uv(blocks, grid, su, ground_y + 3, dv).is_some_and(|b| {
                let bid = b.block_id.as_str();
                bid != "minecraft:air" && !bid.contains("glass") && !bid.ends_with("_door")
            });
            if grid.in_bounds(su, sv)
                && !is_fp(grid, fp, su, sv)
                && is_fp(grid, fp, su, dv)
                && anchor_wall_ok
                && !is_solid_or_occupied(blocks, grid, su, ground_y + 2, sv)
            {
                if !is_solid_or_occupied(blocks, grid, su, ground_y + 3, sv) {
                    set_uv(
                        blocks,
                        grid,
                        su,
                        ground_y + 3,
                        sv,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, su, ground_y + 3, sv)),
                    );
                }
                set_uv(
                    blocks,
                    grid,
                    su,
                    ground_y + 2,
                    sv,
                    PlannedBlock::with_props(
                        sign_id,
                        vec![
                            ("attached", "true"),
                            ("rotation", "0"),
                            ("waterlogged", "false"),
                        ],
                    ),
                );
                break;
            }
        }
    }
}

/// Grows a stylized crooked sakura, gnarled oak, or spire pine tree in spacious yard corners.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_custom_yard_trees(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
) {
    if grid.w < 19
        || grid.d < 17
        || !(spec.courtyard_garden || spec.overgrown)
        || spec.ruin_erosion >= 0.1
        || matches!(spec.category, BuildCategory::Bridge | BuildCategory::Pavilion)
        || height < 10
    {
        return;
    }
    let [_b_u0, _b_v0, _b_u1, b_v1] = bounds;
    let max_clear_y = (i32::from(height) - 2).min(7);
    if max_clear_y < 5 {
        return;
    }

    let mut chosen: Option<(i32, i32)> = None;
    let mut best_score = i32::MAX;
    for tv in 1..(grid.d - 1) {
        for tu in 1..(grid.w - 1) {
            if is_fp(grid, fp, tu, tv) {
                continue;
            }
            if let Some((du, dv)) = best_door
                && (tu - du).abs() <= 2
                && tv >= dv
            {
                continue;
            }
            let base_ok = get_uv(blocks, grid, tu, 0, tv)
                .is_none_or(|b| b.block_id != "minecraft:water")
                && get_uv(blocks, grid, tu, 1, tv).is_none_or(|b| {
                    matches!(
                        b.block_id.as_str(),
                        "minecraft:short_grass"
                            | "minecraft:fern"
                            | "minecraft:wildflowers"
                            | "minecraft:pink_petals"
                            | "minecraft:leaf_litter"
                            | "minecraft:moss_carpet"
                    )
                });
            let trunk_air_ok = (2..=max_clear_y)
                .all(|y| !is_solid_or_occupied(blocks, grid, tu, y, tv));
            let ring_cardinal_clear = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .filter(|&(du, dv)| {
                    !is_fp(grid, fp, tu + du, tv + dv)
                        && !is_solid_or_occupied(blocks, grid, tu + du, 5, tv + dv)
                })
                .count();
            if base_ok && trunk_air_ok && ring_cardinal_clear >= 2 {
                let clear_4 = if ring_cardinal_clear == 4 { 0 } else { 15 };
                let score = clear_4 + if tv > b_v1 { 0 } else { 8 } + (tu - grid.w / 2).abs();
                if score < best_score {
                    best_score = score;
                    chosen = Some((tu, tv));
                }
            }
        }
    }
    let Some((tu, tv)) = chosen else {
        return;
    };

    enum TreeStyle {
        SpruceCone,
        CypressPillar,
        OrnamentalFlat,
        CrookedCanopy,
    }

    let (trunk_id, fixed_leaf, tree_kind) = match spec.style {
        BuildStyle::Japanese => ("minecraft:cherry_log", Some("minecraft:cherry_leaves"), TreeStyle::CrookedCanopy),
        BuildStyle::Nordic => ("minecraft:spruce_log", Some("minecraft:spruce_leaves"), TreeStyle::SpruceCone),
        BuildStyle::Desert | BuildStyle::Classical => ("minecraft:oak_log", Some("minecraft:spruce_leaves"), TreeStyle::CypressPillar),
        BuildStyle::Modern => ("minecraft:birch_log", Some("minecraft:azalea_leaves"), TreeStyle::OrnamentalFlat),
        BuildStyle::Gothic | BuildStyle::Steampunk => ("minecraft:dark_oak_log", None, TreeStyle::CrookedCanopy),
        _ => ("minecraft:oak_log", None, TreeStyle::CrookedCanopy),
    };
    let pick_leaf = |cu: i32, ly: i32, cv: i32| -> String {
        match fixed_leaf {
            Some(fixed) if !pal.has_extra(MaterialPalette::SLOT_LEAVES) => fixed.to_owned(),
            _ => pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, ly, cv),
        }
    };

    match tree_kind {
        TreeStyle::SpruceCone => {
            for y in 1..=6.min(max_clear_y) {
                set_uv(
                    blocks,
                    grid,
                    tu,
                    y,
                    tv,
                    PlannedBlock::with_props(trunk_id, vec![("axis", "y")]),
                );
            }
            for (ly, rad) in [(3i32, 2i32), (4, 1), (5, 1), (6, 1)] {
                if ly > max_clear_y {
                    continue;
                }
                for dv in -rad..=rad {
                    for du in -rad..=rad {
                        if (du == 0 && dv == 0) || (rad == 2 && du.abs() == 2 && dv.abs() == 2) {
                            continue;
                        }
                        if ly == 6 && (du.abs() + dv.abs() > 1) {
                            continue;
                        }
                        let (cu, cv) = (tu + du, tv + dv);
                        if grid.in_bounds(cu, cv)
                            && !is_fp(grid, fp, cu, cv)
                            && !is_solid_or_occupied(blocks, grid, cu, ly, cv)
                        {
                            set_uv(blocks, grid, cu, ly, cv, make_leaves(&pick_leaf(cu, ly, cv)));
                        }
                    }
                }
            }
            if 7 <= max_clear_y && !is_solid_or_occupied(blocks, grid, tu, 7, tv) {
                set_uv(blocks, grid, tu, 7, tv, make_leaves(&pick_leaf(tu, 7, tv)));
            }
        }
        TreeStyle::CypressPillar => {
            // Tall slender Mediterranean cypress spire beside Classical & Desert builds.
            let top_y = 6.min(max_clear_y);
            for y in 1..=top_y.min(5) {
                set_uv(
                    blocks,
                    grid,
                    tu,
                    y,
                    tv,
                    PlannedBlock::with_props(trunk_id, vec![("axis", "y")]),
                );
            }
            for ly in 2..=top_y {
                for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (cu, cv) = (tu + du, tv + dv);
                    if grid.in_bounds(cu, cv)
                        && !is_fp(grid, fp, cu, cv)
                        && !is_solid_or_occupied(blocks, grid, cu, ly, cv)
                    {
                        set_uv(blocks, grid, cu, ly, cv, make_leaves(&pick_leaf(cu, ly, cv)));
                    }
                }
            }
            if top_y + 1 <= max_clear_y && !is_solid_or_occupied(blocks, grid, tu, top_y + 1, tv) {
                set_uv(blocks, grid, tu, top_y + 1, tv, make_leaves(&pick_leaf(tu, top_y + 1, tv)));
            }
        }
        TreeStyle::OrnamentalFlat => {
            for y in 1..=4.min(max_clear_y) {
                set_uv(
                    blocks,
                    grid,
                    tu,
                    y,
                    tv,
                    PlannedBlock::with_props(trunk_id, vec![("axis", "y")]),
                );
            }
            let crown_y = 5.min(max_clear_y);
            for dv in -1..=1 {
                for du in -1..=1 {
                    let (cu, cv) = (tu + du, tv + dv);
                    if grid.in_bounds(cu, cv)
                        && !is_fp(grid, fp, cu, cv)
                        && !is_solid_or_occupied(blocks, grid, cu, crown_y, cv)
                    {
                        let l_blk = if du == 0 && dv == 0 && !pal.has_extra(MaterialPalette::SLOT_LEAVES) {
                            "minecraft:flowering_azalea_leaves".to_owned()
                        } else {
                            pick_leaf(cu, crown_y, cv)
                        };
                        set_uv(blocks, grid, cu, crown_y, cv, make_leaves(&l_blk));
                    }
                }
            }
        }
        TreeStyle::CrookedCanopy => {
            let lean_u = if !is_fp(grid, fp, tu - 1, tv)
                && !is_solid_or_occupied(blocks, grid, tu - 1, 4, tv)
                && (tu >= grid.w / 2 || is_fp(grid, fp, tu + 1, tv))
            {
                -1
            } else if !is_fp(grid, fp, tu + 1, tv)
                && !is_solid_or_occupied(blocks, grid, tu + 1, 4, tv)
            {
                1
            } else {
                0
            };
            for y in 1..=3 {
                set_uv(
                    blocks,
                    grid,
                    tu,
                    y,
                    tv,
                    PlannedBlock::with_props(trunk_id, vec![("axis", "y")]),
                );
            }
            set_uv(
                blocks,
                grid,
                tu + lean_u,
                4,
                tv,
                PlannedBlock::with_props(trunk_id, vec![("axis", if lean_u == 0 { "y" } else { "x" })]),
            );
            for dv in -1..=1 {
                for du in -1..=1 {
                    let cu = tu + lean_u + du;
                    let cv = tv + dv;
                    if grid.in_bounds(cu, cv) && !is_fp(grid, fp, cu, cv) {
                        if !is_solid_or_occupied(blocks, grid, cu, 5, cv) {
                            set_uv(blocks, grid, cu, 5, cv, make_leaves(&pick_leaf(cu, 5, cv)));
                        }
                        if du.abs() + dv.abs() <= 1
                            && 6 <= max_clear_y
                            && !is_solid_or_occupied(blocks, grid, cu, 6, cv)
                        {
                            set_uv(blocks, grid, cu, 6, cv, make_leaves(&pick_leaf(cu, 6, cv)));
                        }
                    }
                }
            }
            let tree_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, tu + lean_u, 4, tv + 1);
            if grid.in_bounds(tu + lean_u, tv + 1)
                && !is_fp(grid, fp, tu + lean_u, tv + 1)
                && !is_solid_or_occupied(blocks, grid, tu + lean_u, 4, tv + 1)
                && is_solid_or_occupied(blocks, grid, tu + lean_u, 5, tv + 1)
                && is_lantern(&tree_light)
            {
                set_uv(
                    blocks,
                    grid,
                    tu + lean_u,
                    4,
                    tv + 1,
                    PlannedBlock::with_props(tree_light, vec![("hanging", "true")]),
                );
            }
        }
    }

    // Scattered petals / wildflowers / leaf litter collar beneath tree roots over clear ground.
    for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let cu = tu + du;
        let cv = tv + dv;
        if grid.in_bounds(cu, cv)
            && !is_fp(grid, fp, cu, cv)
            && !is_solid_or_occupied(blocks, grid, cu, 1, cv)
            && get_uv(blocks, grid, cu, 0, cv).is_none_or(|b| b.block_id != "minecraft:water")
        {
            let dec = match spec.style {
                BuildStyle::Japanese => PlannedBlock::with_props(
                    "minecraft:pink_petals",
                    vec![("facing", grid.dir_front()), ("flower_amount", "3")],
                ),
                BuildStyle::Cottage | BuildStyle::Medieval => PlannedBlock::with_props(
                    "minecraft:wildflowers",
                    vec![("facing", grid.dir_front()), ("flower_amount", "3")],
                ),
                _ => PlannedBlock::with_props(
                    "minecraft:leaf_litter",
                    vec![("facing", grid.dir_front()), ("segment_amount", "3")],
                ),
            };
            set_uv(blocks, grid, cu, 1, cv, dec);
        }
    }
}
