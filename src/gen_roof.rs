use std::collections::BTreeMap;

use crate::gen_common::{
    LocalGrid, PlannedBlock, compute_edge_distance, is_fp, is_solid_or_occupied,
    make_frame_block_at, make_slab, make_stair, set_uv,
};
use crate::model::{BuildCategory, BuildStyle, MaterialPalette, RoofStyle};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, StructureTheme};

/// Builds distance-field, cross-gable, vaulted greenhouse, lighthouse lantern, windmill sails, and dormer roofs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_roof_and_dormers(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    bounds: [i32; 4],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
    roof_base_y: i32,
    noise: NoiseField,
    rng: &mut SplitMix64,
) -> i32 {
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let roof_edge_dist = compute_edge_distance(grid, roof_fp);
    let max_roof_span = ((i32::from(height) - 1) - roof_base_y).max(2);
    let mut max_placed_roof_y = roof_base_y;

    // Specialized vaulted glass roof for Greenhouse conservatories.
    if spec.theme == StructureTheme::Greenhouse {
        return build_greenhouse_roof(
            blocks, grid, roof_fp, bounds, pal, seed, height, roof_base_y, max_roof_span,
        );
    }

    // Specialized glowing lighthouse beacon chamber on top of tower!
    if spec.theme == StructureTheme::Lighthouse {
        return build_lighthouse_lantern(
            blocks, grid, roof_fp, bounds, pal, seed, height, roof_base_y,
        );
    }

    let flip_ridge = ((b_u1 - b_u0) - (b_v1 - b_v0)).abs() <= 3 && rng.next_f64() < 0.45;
    match spec.roof_style {
        RoofStyle::Flat => {
            let crenel = matches!(spec.category, BuildCategory::Tower | BuildCategory::Castle)
                || spec.style == BuildStyle::Desert;
            for v in 0..grid.d {
                for u in 0..grid.w {
                    if !roof_fp[grid.idx(u, v)] {
                        continue;
                    }
                    set_uv(
                        blocks,
                        grid,
                        u,
                        roof_base_y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, roof_base_y, v)),
                    );
                    if roof_edge_dist[grid.idx(u, v)] == 1 {
                        if crenel {
                            if (u + v) % 2 == 0 {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    roof_base_y + 1,
                                    v,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, roof_base_y + 1, v)),
                                );
                                if spec.category == BuildCategory::Castle && roof_base_y + 2 < i32::from(height) {
                                    set_uv(
                                        blocks,
                                        grid,
                                        u,
                                        roof_base_y + 2,
                                        v,
                                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, roof_base_y + 2, v), "bottom"),
                                    );
                                    max_placed_roof_y = max_placed_roof_y.max(roof_base_y + 2);
                                } else {
                                    max_placed_roof_y = max_placed_roof_y.max(roof_base_y + 1);
                                }
                            } else {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    roof_base_y + 1,
                                    v,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, roof_base_y + 1, v)),
                                );
                                max_placed_roof_y = max_placed_roof_y.max(roof_base_y + 1);
                            }
                        } else if spec.style == BuildStyle::Modern {
                            let is_rim_corner = !is_fp(grid, roof_fp, u - 1, v) as u8
                                + !is_fp(grid, roof_fp, u + 1, v) as u8
                                + !is_fp(grid, roof_fp, u, v - 1) as u8
                                + !is_fp(grid, roof_fp, u, v + 1) as u8
                                >= 2;
                            let parapet_blk = if is_rim_corner {
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, roof_base_y + 1, v))
                            } else {
                                PlannedBlock::simple("minecraft:glass_pane")
                            };
                            set_uv(blocks, grid, u, roof_base_y + 1, v, parapet_blk);
                            max_placed_roof_y = max_placed_roof_y.max(roof_base_y + 1);
                        } else {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                roof_base_y + 1,
                                v,
                                make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, roof_base_y + 1, v), "bottom"),
                            );
                            max_placed_roof_y = max_placed_roof_y.max(roof_base_y + 1);
                        }
                    } else if spec.style == BuildStyle::Modern
                        && noise.fbm_2d(u as f64 * 0.5, v as f64 * 0.5, 2) > 0.65
                    {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            roof_base_y,
                            v,
                            PlannedBlock::simple("minecraft:glass"),
                        );
                    }
                }
            }
            // Modern penthouse roof lounge seat & azalea planter on spacious accessible flat roofs.
            if spec.style == BuildStyle::Modern
                && (b_u1 - b_u0) >= 7
                && (b_v1 - b_v0) >= 7
                && roof_base_y + 2 < i32::from(height)
            {
                let lu = (b_u0 + b_u1) / 2;
                let lv = (b_v0 + b_v1) / 2;
                if roof_edge_dist[grid.idx(lu, lv)] >= 2 && roof_edge_dist[grid.idx(lu + 1, lv)] >= 2 {
                    set_uv(blocks, grid, lu, roof_base_y, lv, PlannedBlock::simple(pal.roof_block.clone()));
                    set_uv(blocks, grid, lu + 1, roof_base_y, lv, PlannedBlock::simple(pal.roof_block.clone()));
                    set_uv(
                        blocks,
                        grid,
                        lu,
                        roof_base_y + 1,
                        lv,
                        make_stair(&pal.roof_stairs, grid.dir_front(), "bottom"),
                    );
                    set_uv(
                        blocks,
                        grid,
                        lu + 1,
                        roof_base_y + 1,
                        lv,
                        PlannedBlock::simple("minecraft:potted_azalea_bush"),
                    );
                }
            }
        }
        RoofStyle::Gable => {
            max_placed_roof_y = build_gable_roof(
                blocks,
                grid,
                roof_fp,
                bounds,
                spec,
                pal,
                seed,
                height,
                roof_base_y,
                max_roof_span,
                flip_ridge,
            );
        }
        RoofStyle::Hip | RoofStyle::Steep | RoofStyle::Pagoda => {
            let y_mult = if spec.roof_style == RoofStyle::Steep { 2 } else { 1 };
            let n_cells = (grid.w * grid.d) as usize;
            let mut hip_ry = vec![None; n_cells];
            for v in 0..grid.d {
                for u in 0..grid.w {
                    let d = roof_edge_dist[grid.idx(u, v)];
                    let near_orth = is_fp(grid, roof_fp, u - 1, v)
                        || is_fp(grid, roof_fp, u + 1, v)
                        || is_fp(grid, roof_fp, u, v - 1)
                        || is_fp(grid, roof_fp, u, v + 1);
                    let near_diag = is_fp(grid, roof_fp, u - 1, v - 1)
                        || is_fp(grid, roof_fp, u + 1, v - 1)
                        || is_fp(grid, roof_fp, u - 1, v + 1)
                        || is_fp(grid, roof_fp, u + 1, v + 1);
                    let on_eave = spec.roof_overhang && d == 0 && (near_orth || near_diag);
                    if d == 0 && !on_eave {
                        continue;
                    }
                    let layer = if d == 0 {
                        0
                    } else {
                        (d - 1).min(max_roof_span / y_mult)
                    };
                    let ry = (roof_base_y + layer * y_mult).min(i32::from(height) - 1);
                    hip_ry[grid.idx(u, v)] = Some(ry);
                    max_placed_roof_y = max_placed_roof_y.max(ry);
                }
            }

            for v in 0..grid.d {
                for u in 0..grid.w {
                    let Some(ry) = hip_ry[grid.idx(u, v)] else {
                        continue;
                    };
                    let d = roof_edge_dist[grid.idx(u, v)];
                    let near_orth = is_fp(grid, roof_fp, u - 1, v)
                        || is_fp(grid, roof_fp, u + 1, v)
                        || is_fp(grid, roof_fp, u, v - 1)
                        || is_fp(grid, roof_fp, u, v + 1);
                    let near_diag = is_fp(grid, roof_fp, u - 1, v - 1)
                        || is_fp(grid, roof_fp, u + 1, v - 1)
                        || is_fp(grid, roof_fp, u - 1, v + 1)
                        || is_fp(grid, roof_fp, u + 1, v + 1);

                    // Seal vertical step & diagonal corner gaps cleanly below steep/hip roof slopes.
                    if d > 0 && ry > roof_base_y + 1 {
                        let mut min_nb = ry;
                        for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                            let nu = u + du;
                            let nv = v + dv;
                            if grid.in_bounds(nu, nv) {
                                min_nb = min_nb.min(hip_ry[grid.idx(nu, nv)].unwrap_or(roof_base_y));
                            }
                        }
                        for fill_y in min_nb.max(roof_base_y + 1)..ry {
                            if !is_solid_or_occupied(blocks, grid, u, fill_y, v) {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    fill_y,
                                    v,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, fill_y, v)),
                                );
                            }
                        }
                    }

                    if spec.roof_style == RoofStyle::Pagoda && d == 0 {
                        let diag_corner = !near_orth && near_diag;
                        if diag_corner && ry + 1 < i32::from(height) {
                            set_uv(blocks, grid, u, ry + 1, v, make_slab(&pal.roof_slab, "bottom"));
                            if spec.style == BuildStyle::Japanese
                                && ry > i32::from(spec.foundation_height) + 2
                                && (pal.light == "minecraft:lantern"
                                    || pal.light == "minecraft:soul_lantern"
                                    || pal.light == "minecraft:copper_lantern")
                                && !is_solid_or_occupied(blocks, grid, u, ry, v)
                            {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    ry,
                                    v,
                                    PlannedBlock::with_props(pal.light.clone(), vec![("hanging", "true")]),
                                );
                            }
                            max_placed_roof_y = max_placed_roof_y.max(ry + 1);
                        } else {
                            set_uv(blocks, grid, u, ry, v, make_slab(&pal.roof_slab, "top"));
                        }
                        continue;
                    }

                    let dn = if grid.in_bounds(u, v - 1) {
                        roof_edge_dist[grid.idx(u, v - 1)]
                    } else {
                        0
                    };
                    let ds = if grid.in_bounds(u, v + 1) {
                        roof_edge_dist[grid.idx(u, v + 1)]
                    } else {
                        0
                    };
                    let dw = if grid.in_bounds(u - 1, v) {
                        roof_edge_dist[grid.idx(u - 1, v)]
                    } else {
                        0
                    };
                    let de = if grid.in_bounds(u + 1, v) {
                        roof_edge_dist[grid.idx(u + 1, v)]
                    } else {
                        0
                    };
                    let min_n = dn.min(ds).min(dw).min(de);
                    let max_n = dn.max(ds).max(dw).max(de);

                    if d > 0 && max_n <= d {
                        let ridge_slab = if pal.roof_slab.contains("deepslate")
                            || pal.roof_slab.contains("blackstone")
                        {
                            "minecraft:stone_brick_slab"
                        } else {
                            pal.roof_slab.as_str()
                        };
                        if ry + 1 < i32::from(height) {
                            set_uv(blocks, grid, u, ry, v, PlannedBlock::simple(pal.roof_block.clone()));
                            set_uv(blocks, grid, u, ry + 1, v, make_slab(ridge_slab, "bottom"));
                            let is_apex_center = u == (b_u0 + b_u1) / 2 && v == (b_v0 + b_v1) / 2;
                            if spec.roof_style == RoofStyle::Pagoda
                                && is_apex_center
                                && ry + 3 < i32::from(height)
                            {
                                // Ornate Japanese `sorin` finial spire crowning pagoda apex.
                                set_uv(blocks, grid, u, ry + 1, v, PlannedBlock::simple(pal.roof_block.clone()));
                                set_uv(blocks, grid, u, ry + 2, v, PlannedBlock::simple("minecraft:iron_bars"));
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    ry + 3,
                                    v,
                                    PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
                                );
                                max_placed_roof_y = max_placed_roof_y.max(ry + 3);
                            } else {
                                max_placed_roof_y = max_placed_roof_y.max(ry + 1);
                            }
                        } else {
                            if ry - 1 > roof_base_y {
                                set_uv(
                                    blocks,
                                    grid,
                                    u,
                                    ry - 1,
                                    v,
                                    PlannedBlock::simple(pal.roof_block.clone()),
                                );
                            }
                            set_uv(blocks, grid, u, ry, v, make_slab(ridge_slab, "bottom"));
                        }
                    } else if d == 0 && !near_orth && near_diag {
                        // Diagonal outer eave corner stair oriented cleanly along diagonal quadrant!
                        let face = if is_fp(grid, roof_fp, u, v + 1) || is_fp(grid, roof_fp, u + 1, v + 1) || is_fp(grid, roof_fp, u - 1, v + 1) {
                            grid.dir_front()
                        } else {
                            grid.dir_back()
                        };
                        set_uv(
                            blocks,
                            grid,
                            u,
                            ry,
                            v,
                            make_stair(&pal.roof_stairs, face, "bottom"),
                        );
                    } else {
                        let face = if min_n == ds {
                            grid.dir_back()
                        } else if min_n == dn {
                            grid.dir_front()
                        } else if min_n == de {
                            grid.dir_left()
                        } else {
                            grid.dir_right()
                        };
                        let picked_stair = pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                        let slope_stair = if !pal.has_extra(MaterialPalette::SLOT_ROOF)
                            && picked_stair == "minecraft:deepslate_tile_stairs"
                            && (u * 7 + v * 13).abs() % 5 == 0
                        {
                            "minecraft:cobbled_deepslate_stairs".to_owned()
                        } else {
                            picked_stair
                        };
                        set_uv(
                            blocks,
                            grid,
                            u,
                            ry,
                            v,
                            make_stair(&slope_stair, face, "bottom"),
                        );
                    }
                }
            }
            // Add dormer onto sloped hip/steep roof if large enough!
            if spec.dormers && (b_u1 - b_u0) >= 8 && (b_v1 - b_v0) >= 7 {
                place_dormer_at(
                    blocks,
                    grid,
                    roof_fp,
                    (b_u0 + b_u1) / 2,
                    b_v1 - 1,
                    true,
                    roof_base_y,
                    height,
                    pal,
                    seed,
                );
            }
        }
        RoofStyle::Dome => {
            let cu = (b_u0 + b_u1) as f64 * 0.5;
            let cv = (b_v0 + b_v1) as f64 * 0.5;
            let rx = ((b_u1 - b_u0) as f64 * 0.5).max(2.0);
            let rz = ((b_v1 - b_v0) as f64 * 0.5).max(2.0);
            let dome_h = ((rx.min(rz) * 0.95).round() as i32).clamp(2, max_roof_span.max(2));
            let mut dome_y = vec![None; (grid.w * grid.d) as usize];

            for v in 0..grid.d {
                for u in 0..grid.w {
                    let d = roof_edge_dist[grid.idx(u, v)];
                    let near_orth = is_fp(grid, roof_fp, u - 1, v)
                        || is_fp(grid, roof_fp, u + 1, v)
                        || is_fp(grid, roof_fp, u, v - 1)
                        || is_fp(grid, roof_fp, u, v + 1);
                    let near_diag = is_fp(grid, roof_fp, u - 1, v - 1)
                        || is_fp(grid, roof_fp, u + 1, v - 1)
                        || is_fp(grid, roof_fp, u - 1, v + 1)
                        || is_fp(grid, roof_fp, u + 1, v + 1);
                    if d == 0 {
                        if spec.roof_overhang && (near_orth || near_diag) {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                roof_base_y,
                                v,
                                make_slab(&pal.roof_slab, "bottom"),
                            );
                            max_placed_roof_y = max_placed_roof_y.max(roof_base_y);
                        }
                        continue;
                    }
                    let nx = ((u as f64 - cu) / rx).abs();
                    let nz = ((v as f64 - cv) / rz).abs();
                    let norm_sq = (nx * nx + nz * nz).min(1.0);
                    let curve = (1.0 - norm_sq).max(0.0).sqrt();
                    let raw_ry = roof_base_y + (curve * f64::from(dome_h)).round() as i32;
                    let max_y = i32::from(height) - 1;
                    let ry = crate::gen_common::clamp_i32(raw_ry, roof_base_y.min(max_y), max_y);
                    dome_y[grid.idx(u, v)] = Some((ry, norm_sq));
                    max_placed_roof_y = max_placed_roof_y.max(ry);
                }
            }

            for v in 0..grid.d {
                for u in 0..grid.w {
                    let Some((ry, norm_sq)) = dome_y[grid.idx(u, v)] else {
                        continue;
                    };
                    let mut min_nb = ry;
                    for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let nu = u + du;
                        let nv = v + dv;
                        if grid.in_bounds(nu, nv) {
                            if let Some((nry, _)) = dome_y[grid.idx(nu, nv)] {
                                min_nb = min_nb.min(nry);
                            } else {
                                min_nb = min_nb.min(roof_base_y);
                            }
                        }
                    }
                    for y in min_nb.max(roof_base_y)..ry {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, y, v)),
                        );
                    }
                    let top_block = if ry > roof_base_y && norm_sq < 0.14 {
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry, v), "bottom")
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v))
                    };
                    set_uv(blocks, grid, u, ry, v, top_block);
                }
            }

            // Crown cupola lantern / open 3x3 rotunda atop dome summit.
            let mid_u = (b_u0 + b_u1) / 2;
            let mid_v = (b_v0 + b_v1) / 2;
            if let Some((summit_y, _)) = dome_y[grid.idx(mid_u, mid_v)] {
                if b_u1 - b_u0 >= 6
                    && b_v1 - b_v0 >= 6
                    && summit_y + 4 < i32::from(height)
                {
                    for dv in -1..=1 {
                        for du in -1..=1 {
                            let u = mid_u + du;
                            let v = mid_v + dv;
                            if !grid.in_bounds(u, v) {
                                continue;
                            }
                            if du == 0 && dv == 0 {
                                set_uv(blocks, grid, u, summit_y + 1, v, PlannedBlock::simple(pal.light.clone()));
                                set_uv(blocks, grid, u, summit_y + 2, v, PlannedBlock::simple(pal.roof_block.clone()));
                                set_uv(blocks, grid, u, summit_y + 3, v, make_slab(&pal.roof_slab, "bottom"));
                                if summit_y + 4 < i32::from(height) {
                                    set_uv(
                                        blocks,
                                        grid,
                                        u,
                                        summit_y + 4,
                                        v,
                                        PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
                                    );
                                }
                            } else if du.abs() == 1 && dv.abs() == 1 {
                                set_uv(blocks, grid, u, summit_y + 1, v, PlannedBlock::simple(pal.fence.clone()));
                                set_uv(blocks, grid, u, summit_y + 2, v, make_slab(&pal.roof_slab, "bottom"));
                            } else {
                                set_uv(blocks, grid, u, summit_y + 2, v, make_slab(&pal.roof_slab, "top"));
                            }
                        }
                    }
                    max_placed_roof_y = max_placed_roof_y.max((summit_y + 4).min(i32::from(height) - 1));
                } else if summit_y + 2 < i32::from(height) {
                    set_uv(blocks, grid, mid_u, summit_y, mid_v, PlannedBlock::simple(pal.roof_block.clone()));
                    set_uv(blocks, grid, mid_u, summit_y + 1, mid_v, PlannedBlock::simple(pal.trim_wall.clone()));
                    set_uv(blocks, grid, mid_u, summit_y + 2, mid_v, PlannedBlock::simple(pal.light.clone()));
                    max_placed_roof_y = max_placed_roof_y.max(summit_y + 2);
                }
            }
        }
        RoofStyle::Mansard => {
            for v in 0..grid.d {
                for u in 0..grid.w {
                    let d = roof_edge_dist[grid.idx(u, v)];
                    let near_orth = is_fp(grid, roof_fp, u - 1, v)
                        || is_fp(grid, roof_fp, u + 1, v)
                        || is_fp(grid, roof_fp, u, v - 1)
                        || is_fp(grid, roof_fp, u, v + 1);
                    let near_diag = is_fp(grid, roof_fp, u - 1, v - 1)
                        || is_fp(grid, roof_fp, u + 1, v - 1)
                        || is_fp(grid, roof_fp, u - 1, v + 1)
                        || is_fp(grid, roof_fp, u + 1, v + 1);
                    let on_eave = spec.roof_overhang && d == 0 && (near_orth || near_diag);
                    if d == 0 && !on_eave {
                        continue;
                    }

                    let dn = if grid.in_bounds(u, v - 1) {
                        roof_edge_dist[grid.idx(u, v - 1)]
                    } else {
                        0
                    };
                    let ds = if grid.in_bounds(u, v + 1) {
                        roof_edge_dist[grid.idx(u, v + 1)]
                    } else {
                        0
                    };
                    let dw = if grid.in_bounds(u - 1, v) {
                        roof_edge_dist[grid.idx(u - 1, v)]
                    } else {
                        0
                    };
                    let de = if grid.in_bounds(u + 1, v) {
                        roof_edge_dist[grid.idx(u + 1, v)]
                    } else {
                        0
                    };
                    let min_n = dn.min(ds).min(dw).min(de);
                    let outward_face = if d == 0 {
                        if is_fp(grid, roof_fp, u, v + 1)
                            || is_fp(grid, roof_fp, u + 1, v + 1)
                            || is_fp(grid, roof_fp, u - 1, v + 1)
                        {
                            grid.dir_front()
                        } else if is_fp(grid, roof_fp, u, v - 1) {
                            grid.dir_back()
                        } else if is_fp(grid, roof_fp, u - 1, v) {
                            grid.dir_right()
                        } else {
                            grid.dir_left()
                        }
                    } else if min_n == ds {
                        grid.dir_back()
                    } else if min_n == dn {
                        grid.dir_front()
                    } else if min_n == de {
                        grid.dir_left()
                    } else {
                        grid.dir_right()
                    };

                    if d == 0 {
                        let eave_y = roof_base_y.min(i32::from(height) - 1);
                        set_uv(
                            blocks,
                            grid,
                            u,
                            eave_y,
                            v,
                            make_stair(&pal.roof_stairs, outward_face, "bottom"),
                        );
                        max_placed_roof_y = max_placed_roof_y.max(eave_y);
                    } else if d == 1 {
                        // Steep 2-block-tall French mansard lower apron.
                        let y1 = (roof_base_y + 1).min(i32::from(height) - 1);
                        let y2 = (roof_base_y + 2).min(i32::from(height) - 1);
                        set_uv(
                            blocks,
                            grid,
                            u,
                            roof_base_y,
                            v,
                            PlannedBlock::simple(pal.roof_block.clone()),
                        );
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y1,
                            v,
                            make_stair(&pal.roof_stairs, outward_face, "bottom"),
                        );
                        if y2 > y1 {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                y2,
                                v,
                                PlannedBlock::simple(pal.trim_wall.clone()),
                            );
                        }
                        max_placed_roof_y = max_placed_roof_y.max(y2);
                    } else if d == 2 {
                        // Upper mansard deck curb + shallow terrace slope.
                        let y2 = (roof_base_y + 2).min(i32::from(height) - 1);
                        let y3 = (roof_base_y + 3).min(i32::from(height) - 1);
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y2,
                            v,
                            PlannedBlock::simple(pal.roof_block.clone()),
                        );
                        if y3 > y2 {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                y3,
                                v,
                                make_slab(&pal.roof_slab, "bottom"),
                            );
                        }
                        max_placed_roof_y = max_placed_roof_y.max(y3);
                    } else {
                        let y3 = (roof_base_y + 3).min(i32::from(height) - 1);
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y3,
                            v,
                            PlannedBlock::simple(pal.roof_block.clone()),
                        );
                        max_placed_roof_y = max_placed_roof_y.max(y3);
                    }
                }
            }
            if spec.dormers && (b_u1 - b_u0) >= 8 && (b_v1 - b_v0) >= 7 {
                place_dormer_at(
                    blocks,
                    grid,
                    roof_fp,
                    (b_u0 + b_u1) / 2,
                    b_v1 - 1,
                    true,
                    roof_base_y,
                    height,
                    pal,
                    seed,
                );
            }
        }
    }

    // Temple front bell steeple arch holding a hanging `minecraft:bell`!
    if spec.category == BuildCategory::Temple && (b_u1 - b_u0) >= 6 {
        let su = (b_u0 + b_u1) / 2;
        if let Some(sv) = (b_v0..=b_v1)
            .rev()
            .skip(1)
            .find(|&v| is_fp(grid, roof_fp, su, v))
        {
            let roof_at_bell = (roof_base_y..i32::from(height))
                .rev()
                .find(|&y| crate::gen_common::is_solid_or_occupied(blocks, grid, su, y, sv))
                .unwrap_or(roof_base_y);
            let base = roof_at_bell.min(i32::from(height) - 5);
            if base >= roof_base_y && base + 4 < i32::from(height) {
                for dy in 1..=3 {
                    set_uv(blocks, grid, su - 1, base + dy, sv, make_frame_block_at(pal, "y", seed, su - 1, base + dy, sv));
                    set_uv(blocks, grid, su + 1, base + dy, sv, make_frame_block_at(pal, "y", seed, su + 1, base + dy, sv));
                    set_uv(blocks, grid, su, base + dy, sv, PlannedBlock::simple("minecraft:air"));
                }
                set_uv(blocks, grid, su, base + 3, sv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, su, base + 3, sv)));
                set_uv(blocks, grid, su - 1, base + 4, sv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, su - 1, base + 4, sv), "bottom"));
                set_uv(blocks, grid, su + 1, base + 4, sv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, su + 1, base + 4, sv), "bottom"));
                set_uv(blocks, grid, su, base + 4, sv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, su, base + 4, sv)));
                set_uv(
                    blocks,
                    grid,
                    su,
                    base + 2,
                    sv,
                    PlannedBlock::with_props(
                        "minecraft:bell",
                        vec![
                            ("attachment", "ceiling"),
                            ("facing", grid.dir_front()),
                            ("powered", "false"),
                        ],
                    ),
                );
                max_placed_roof_y = max_placed_roof_y.max(base + 4);
            }
        }
    }

    // Windmill axle and 4 diagonal timber/wool wind-sails on the front facade!
    if spec.theme == StructureTheme::Windmill {
        attach_windmill_sails(blocks, grid, roof_fp, bounds, roof_base_y, pal, seed, height);
    }

    max_placed_roof_y
}

/// Computes local span along ridge & orthogonal direction so L/T-wings form natural intersecting gables + under-eave brackets.
#[allow(clippy::too_many_arguments)]
fn build_gable_roof(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    bounds: [i32; 4],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
    roof_base_y: i32,
    max_roof_span: i32,
    flip_ridge: bool,
) -> i32 {
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let gable_along_u = ((b_u1 - b_u0) >= (b_v1 - b_v0)) ^ flip_ridge;
    let span = if gable_along_u {
        b_v1 - b_v0 + 1
    } else {
        b_u1 - b_u0 + 1
    };
    let half_span = ((span + 1) / 2).min(max_roof_span);
    let ridge_mid_u = (b_u0 + b_u1) / 2;
    let ridge_mid_v = (b_v0 + b_v1) / 2;
    let mut max_y = roof_base_y;

    let n_cells = (grid.w * grid.d) as usize;
    let mut in_fp_vec = vec![false; n_cells];
    let mut on_eave_vec = vec![false; n_cells];
    let mut d_left_vec = vec![0i32; n_cells];
    let mut d_right_vec = vec![0i32; n_cells];
    let mut raw_step_vec = vec![0i32; n_cells];
    let mut step_vec = vec![0i32; n_cells];
    let mut ry_vec = vec![roof_base_y; n_cells];
    let mut half_hip_vec = vec![false; n_cells];
    let mut is_gable_outer_vec = vec![false; n_cells];

    // First pass: compute exact local gable span & height for every cell in `roof_fp`.
    for v in 0..grid.d {
        for u in 0..grid.w {
            let idx = grid.idx(u, v);
            let in_fp = roof_fp[idx];
            if !in_fp {
                continue;
            }

            // Trace local continuous span along cross-axis for natural L/T wing hips!
            let (mut loc_min, mut loc_max, pos_across) = if gable_along_u {
                (v, v, v)
            } else {
                (u, u, u)
            };
            while if gable_along_u {
                is_fp(grid, roof_fp, u, loc_min - 1)
            } else {
                is_fp(grid, roof_fp, loc_min - 1, v)
            } {
                loc_min -= 1;
            }
            while if gable_along_u {
                is_fp(grid, roof_fp, u, loc_max + 1)
            } else {
                is_fp(grid, roof_fp, loc_max + 1, v)
            } {
                loc_max += 1;
            }
            let (min_a, max_a) = if loc_max - loc_min >= 3 {
                (loc_min, loc_max)
            } else if gable_along_u {
                (b_v0, b_v1)
            } else {
                (b_u0, b_u1)
            };

            let d_left = (pos_across - min_a).max(0);
            let d_right = (max_a - pos_across).max(0);
            let raw_step = d_left.min(d_right).min(half_span);
            let is_gable_outer_end = if gable_along_u {
                !is_fp(grid, roof_fp, u - 1, v) || !is_fp(grid, roof_fp, u + 1, v)
            } else {
                !is_fp(grid, roof_fp, u, v - 1) || !is_fp(grid, roof_fp, u, v + 1)
            };
            // Half-hip chamfer cuts off top 1-2 blocks of steep gable ends cleanly.
            let half_hip_bevel = spec.half_hip
                && half_span >= 3
                && is_gable_outer_end
                && raw_step >= half_span - 1;
            let step = if half_hip_bevel {
                (half_span - 2).max(1)
            } else {
                raw_step
            };
            let ry = (roof_base_y + step).min(i32::from(height) - 1);
            max_y = max_y.max(ry);

            in_fp_vec[idx] = true;
            d_left_vec[idx] = d_left;
            d_right_vec[idx] = d_right;
            raw_step_vec[idx] = raw_step;
            step_vec[idx] = step;
            ry_vec[idx] = ry;
            half_hip_vec[idx] = half_hip_bevel;
            is_gable_outer_vec[idx] = is_gable_outer_end;
        }
    }

    // Second half of Pass 1: overhang eave cells inherit exact height & slope from the wing they overhang.
    if spec.roof_overhang {
        for v in 0..grid.d {
            for u in 0..grid.w {
                let idx = grid.idx(u, v);
                if in_fp_vec[idx] {
                    continue;
                }
                let ridge_nb = if gable_along_u {
                    [(u - 1, v), (u + 1, v)]
                        .into_iter()
                        .find(|&(nu, nv)| is_fp(grid, roof_fp, nu, nv))
                } else {
                    [(u, v - 1), (u, v + 1)]
                        .into_iter()
                        .find(|&(nu, nv)| is_fp(grid, roof_fp, nu, nv))
                };
                if let Some((nu, nv)) = ridge_nb {
                    let nidx = grid.idx(nu, nv);
                    on_eave_vec[idx] = true;
                    d_left_vec[idx] = d_left_vec[nidx];
                    d_right_vec[idx] = d_right_vec[nidx];
                    raw_step_vec[idx] = raw_step_vec[nidx];
                    step_vec[idx] = step_vec[nidx];
                    ry_vec[idx] = ry_vec[nidx];
                    half_hip_vec[idx] = half_hip_vec[nidx];
                    is_gable_outer_vec[idx] = true;
                } else {
                    let side_neg_in = if gable_along_u {
                        is_fp(grid, roof_fp, u, v + 1)
                    } else {
                        is_fp(grid, roof_fp, u + 1, v)
                    };
                    let side_pos_in = if gable_along_u {
                        is_fp(grid, roof_fp, u, v - 1)
                    } else {
                        is_fp(grid, roof_fp, u - 1, v)
                    };
                    if side_neg_in || side_pos_in {
                        on_eave_vec[idx] = true;
                        d_left_vec[idx] = if side_neg_in { 0 } else { 1 };
                        d_right_vec[idx] = if side_neg_in { 1 } else { 0 };
                        raw_step_vec[idx] = 0;
                        step_vec[idx] = 0;
                        ry_vec[idx] = roof_base_y.min(i32::from(height) - 1);
                    }
                }
            }
        }
    }

    for v in 0..grid.d {
        for u in 0..grid.w {
            let idx = grid.idx(u, v);
            let in_fp = in_fp_vec[idx];
            let on_eave = on_eave_vec[idx];
            if !in_fp && !on_eave {
                continue;
            }
            let d_left = d_left_vec[idx];
            let d_right = d_right_vec[idx];
            let raw_step = raw_step_vec[idx];
            let step = step_vec[idx];
            let ry = ry_vec[idx];
            let half_hip_bevel = half_hip_vec[idx];
            let is_gable_outer_end = is_gable_outer_vec[idx];

            let (adj_ry_a, adj_ry_b) = if gable_along_u {
                let a = if is_fp(grid, roof_fp, u - 1, v) {
                    ry_vec[grid.idx(u - 1, v)]
                } else {
                    roof_base_y
                };
                let b = if is_fp(grid, roof_fp, u + 1, v) {
                    ry_vec[grid.idx(u + 1, v)]
                } else {
                    roof_base_y
                };
                (a, b)
            } else {
                let a = if is_fp(grid, roof_fp, u, v - 1) {
                    ry_vec[grid.idx(u, v - 1)]
                } else {
                    roof_base_y
                };
                let b = if is_fp(grid, roof_fp, u, v + 1) {
                    ry_vec[grid.idx(u, v + 1)]
                } else {
                    roof_base_y
                };
                (a, b)
            };
            let min_adj_ridge_ry = adj_ry_a.min(adj_ry_b);
            let min_cross_ry = if gable_along_u {
                let c0 = if is_fp(grid, roof_fp, u, v - 1) {
                    ry_vec[grid.idx(u, v - 1)]
                } else {
                    roof_base_y
                };
                let c1 = if is_fp(grid, roof_fp, u, v + 1) {
                    ry_vec[grid.idx(u, v + 1)]
                } else {
                    roof_base_y
                };
                c0.min(c1)
            } else {
                let c0 = if is_fp(grid, roof_fp, u - 1, v) {
                    ry_vec[grid.idx(u - 1, v)]
                } else {
                    roof_base_y
                };
                let c1 = if is_fp(grid, roof_fp, u + 1, v) {
                    ry_vec[grid.idx(u + 1, v)]
                } else {
                    roof_base_y
                };
                c0.min(c1)
            };
            let is_ridge_step_wall = !is_gable_outer_end && min_adj_ridge_ry < ry;
            let is_cross_step_wall = !is_gable_outer_end && min_cross_ry < ry - 1;
            let is_step_wall = is_ridge_step_wall || is_cross_step_wall;

            if in_fp && step > 0 && (is_gable_outer_end || is_step_wall) {
                let center_dist = (d_left - d_right).abs();
                let use_rose = is_gable_outer_end
                    && !half_hip_bevel
                    && spec.rose_window
                    && half_span >= 4
                    && step >= 4
                    && (ry - roof_base_y) >= 5;
                let has_side_attic_win = is_gable_outer_end
                    && !use_rose
                    && !half_hip_bevel
                    && (ry - roof_base_y) >= 5
                    && (roof_base_y + 3) < ry
                    && center_dist == 2;
                let step_min_ry = if is_ridge_step_wall && is_cross_step_wall {
                    min_adj_ridge_ry.min(min_cross_ry)
                } else if is_ridge_step_wall {
                    min_adj_ridge_ry
                } else {
                    min_cross_ry + 1
                };
                let wy_start = if is_gable_outer_end {
                    roof_base_y + 1
                } else {
                    step_min_ry.max(roof_base_y + 1)
                };
                for wy in wy_start..ry {
                    let dy_rose = wy - (roof_base_y + 3);
                    let blk = if use_rose && center_dist <= 1 && dy_rose.abs() <= 1 {
                        if center_dist == 0 || dy_rose == 0 {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, u, wy, v))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, wy, v))
                        }
                    } else if has_side_attic_win && wy == roof_base_y + 2 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, u, wy, v))
                    } else if center_dist == 0 {
                        make_frame_block_at(pal, "y", seed, u, wy, v)
                    } else if wy == roof_base_y + 1 && step >= 2 {
                        make_frame_block_at(
                            pal,
                            if gable_along_u {
                                grid.axis_u()
                            } else {
                                grid.axis_v()
                            },
                            seed,
                            u,
                            wy,
                            v,
                        )
                    } else if (u + wy) % 3 == 0 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, wy, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, wy, v))
                    };
                    set_uv(blocks, grid, u, wy, v, blk);
                }
            }

            if half_hip_bevel {
                let cap_face = if gable_along_u {
                    if !is_fp(grid, roof_fp, u - 1, v) {
                        grid.dir_right()
                    } else {
                        grid.dir_left()
                    }
                } else if !is_fp(grid, roof_fp, u, v - 1) {
                    grid.dir_front()
                } else {
                    grid.dir_back()
                };
                let hip_blk = if raw_step == half_span && ry + 1 < i32::from(height) {
                    set_uv(blocks, grid, u, ry + 1, v, make_slab(&pal.roof_slab, "bottom"));
                    PlannedBlock::simple(pal.roof_block.clone())
                } else {
                    make_stair(&pal.roof_stairs, cap_face, "bottom")
                };
                set_uv(blocks, grid, u, ry, v, hip_blk);
            } else if d_left == d_right && step >= 1 {
                let ridge_slab = if pal.has_extra(MaterialPalette::SLOT_ROOF_EDGE) || pal.roof_edge != pal.roof_block {
                    crate::gen_common::resolve_slab_id(&pal.pick(MaterialPalette::SLOT_ROOF_EDGE, seed, u, ry, v))
                } else {
                    let s = pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                    if s.contains("deepslate") || s.contains("blackstone") {
                        "minecraft:stone_brick_slab".to_owned()
                    } else {
                        s
                    }
                };
                set_uv(blocks, grid, u, ry, v, make_slab(&ridge_slab, "bottom"));
                // Sculpted upward horn finials (`onigawara` / Nordic ridge peaks) on gable tips!
                if on_eave && ry + 1 < i32::from(height) {
                    let horn_blk = if spec.style == BuildStyle::Nordic {
                        make_frame_block_at(pal, "y", seed, u, ry + 1, v)
                    } else if pal.roof_stairs == "minecraft:deepslate_tile_stairs" && !pal.has_extra(MaterialPalette::SLOT_ROOF) {
                        make_slab("minecraft:stone_brick_slab", "bottom")
                    } else {
                        let inward = if gable_along_u {
                            if !is_fp(grid, roof_fp, u - 1, v) {
                                grid.dir_right()
                            } else {
                                grid.dir_left()
                            }
                        } else if !is_fp(grid, roof_fp, u, v - 1) {
                            grid.dir_front()
                        } else {
                            grid.dir_back()
                        };
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry + 1, v), inward, "bottom")
                    };
                    set_uv(blocks, grid, u, ry + 1, v, horn_blk);
                    max_y = max_y.max(ry + 1);
                }
            } else if d_left < d_right {
                let face = if gable_along_u {
                    grid.dir_front()
                } else {
                    grid.dir_right()
                };
                let picked_stair = pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                let slope_stair = if !pal.has_extra(MaterialPalette::SLOT_ROOF)
                    && picked_stair == "minecraft:deepslate_tile_stairs"
                    && (u * 7 + v * 13).abs() % 5 == 0
                {
                    "minecraft:cobbled_deepslate_stairs".to_owned()
                } else {
                    picked_stair
                };
                set_uv(
                    blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_stair(&slope_stair, face, "bottom"),
                );
                if in_fp && ry - 1 > roof_base_y && !is_solid_or_occupied(blocks, grid, u, ry - 1, v) {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v)),
                    );
                }
                if on_eave && ry > roof_base_y {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v), face, "top"),
                    );
                } else if on_eave && ry == roof_base_y {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v), "top"),
                    );
                }
            } else {
                let face = if gable_along_u {
                    grid.dir_back()
                } else {
                    grid.dir_left()
                };
                let picked_stair = pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                let slope_stair = if !pal.has_extra(MaterialPalette::SLOT_ROOF)
                    && picked_stair == "minecraft:deepslate_tile_stairs"
                    && (u * 7 + v * 13).abs() % 5 == 0
                {
                    "minecraft:cobbled_deepslate_stairs".to_owned()
                } else {
                    picked_stair
                };
                set_uv(
                    blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_stair(&slope_stair, face, "bottom"),
                );
                if in_fp && ry - 1 > roof_base_y && !is_solid_or_occupied(blocks, grid, u, ry - 1, v) {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v)),
                    );
                }
                if on_eave && ry > roof_base_y {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v), face, "top"),
                    );
                } else if on_eave && ry == roof_base_y {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v), "top"),
                    );
                }
            }
        }
    }

    if spec.dormers && half_span >= 3 {
        if gable_along_u {
            place_dormer_at(
                blocks,
                grid,
                roof_fp,
                ridge_mid_u,
                b_v1 - 1,
                true,
                roof_base_y,
                height,
                pal,
                seed,
            );
        } else {
            place_dormer_at(
                blocks,
                grid,
                roof_fp,
                b_u1 - 1,
                ridge_mid_v,
                false,
                roof_base_y,
                height,
                pal,
                seed,
            );
        }
    }

    max_y
}

#[allow(clippy::too_many_arguments)]
fn place_dormer_at(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    du: i32,
    dv: i32,
    facing_front: bool,
    roof_base_y: i32,
    height: u16,
    pal: &MaterialPalette,
    seed: u64,
) {
    if !is_fp(grid, roof_fp, du, dv) || roof_base_y + 4 >= i32::from(height) {
        return;
    }
    if facing_front {
        if !is_fp(grid, roof_fp, du, dv + 1)
            || !is_fp(grid, roof_fp, du - 1, dv)
            || !is_fp(grid, roof_fp, du + 1, dv)
            || !is_fp(grid, roof_fp, du, dv - 1)
            || !is_fp(grid, roof_fp, du, dv - 2)
        {
            return;
        }
        // 3-wide gabled mini-dormer jutting proud from front roof slope.
        for side in [-1, 1] {
            set_uv(
                blocks,
                grid,
                du + side,
                roof_base_y + 1,
                dv,
                make_frame_block_at(pal, "y", seed, du + side, roof_base_y + 1, dv),
            );
            set_uv(
                blocks,
                grid,
                du + side,
                roof_base_y + 1,
                dv + 1,
                make_frame_block_at(pal, "y", seed, du + side, roof_base_y + 1, dv + 1),
            );
            set_uv(
                blocks,
                grid,
                du + side,
                roof_base_y + 2,
                dv,
                make_stair(
                    &pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du + side, roof_base_y + 2, dv),
                    if side < 0 { grid.dir_right() } else { grid.dir_left() },
                    "bottom",
                ),
            );
            set_uv(
                blocks,
                grid,
                du + side,
                roof_base_y + 2,
                dv + 1,
                make_stair(
                    &pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du + side, roof_base_y + 2, dv + 1),
                    if side < 0 { grid.dir_right() } else { grid.dir_left() },
                    "bottom",
                ),
            );
        }
        set_uv(blocks, grid, du, roof_base_y + 1, dv, PlannedBlock::simple("minecraft:air"));
        set_uv(blocks, grid, du, roof_base_y + 1, dv + 1, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, du, roof_base_y + 1, dv + 1)));
        set_uv(blocks, grid, du, roof_base_y + 2, dv + 1, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, du, roof_base_y + 2, dv + 1)));
        set_uv(blocks, grid, du, roof_base_y + 2, dv, PlannedBlock::simple("minecraft:air"));
        set_uv(blocks, grid, du, roof_base_y + 3, dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, du, roof_base_y + 3, dv)));
        set_uv(blocks, grid, du, roof_base_y + 3, dv + 1, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, du, roof_base_y + 3, dv + 1), "bottom"));
        if !is_solid_or_occupied(blocks, grid, du, roof_base_y + 2, dv - 1) {
            set_uv(blocks, grid, du, roof_base_y + 2, dv - 1, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, du, roof_base_y + 2, dv - 1)));
        }
    } else {
        if !is_fp(grid, roof_fp, du + 1, dv)
            || !is_fp(grid, roof_fp, du, dv - 1)
            || !is_fp(grid, roof_fp, du, dv + 1)
            || !is_fp(grid, roof_fp, du - 1, dv)
            || !is_fp(grid, roof_fp, du - 2, dv)
        {
            return;
        }
        for side in [-1, 1] {
            set_uv(
                blocks,
                grid,
                du,
                roof_base_y + 1,
                dv + side,
                make_frame_block_at(pal, "y", seed, du, roof_base_y + 1, dv + side),
            );
            set_uv(
                blocks,
                grid,
                du + 1,
                roof_base_y + 1,
                dv + side,
                make_frame_block_at(pal, "y", seed, du + 1, roof_base_y + 1, dv + side),
            );
            set_uv(
                blocks,
                grid,
                du,
                roof_base_y + 2,
                dv + side,
                make_stair(
                    &pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du, roof_base_y + 2, dv + side),
                    if side < 0 { grid.dir_front() } else { grid.dir_back() },
                    "bottom",
                ),
            );
            set_uv(
                blocks,
                grid,
                du + 1,
                roof_base_y + 2,
                dv + side,
                make_stair(
                    &pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, du + 1, roof_base_y + 2, dv + side),
                    if side < 0 { grid.dir_front() } else { grid.dir_back() },
                    "bottom",
                ),
            );
        }
        set_uv(blocks, grid, du, roof_base_y + 1, dv, PlannedBlock::simple("minecraft:air"));
        set_uv(blocks, grid, du + 1, roof_base_y + 1, dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, du + 1, roof_base_y + 1, dv)));
        set_uv(blocks, grid, du + 1, roof_base_y + 2, dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, du + 1, roof_base_y + 2, dv)));
        set_uv(blocks, grid, du, roof_base_y + 2, dv, PlannedBlock::simple("minecraft:air"));
        set_uv(blocks, grid, du, roof_base_y + 3, dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, du, roof_base_y + 3, dv)));
        set_uv(blocks, grid, du + 1, roof_base_y + 3, dv, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, du + 1, roof_base_y + 3, dv), "bottom"));
        if !is_solid_or_occupied(blocks, grid, du - 1, roof_base_y + 2, dv) {
            set_uv(blocks, grid, du - 1, roof_base_y + 2, dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, du - 1, roof_base_y + 2, dv)));
        }
    }
}

/// Pitches a cozy lean-to wing roof over a lower stepped wing abutting the taller main core.
pub(crate) fn build_wing_sloped_roof(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    wing_fp: &[bool],
    tall_fp: &[bool],
    pal: &MaterialPalette,
    seed: u64,
    base_y: i32,
    height: u16,
) {
    let mut tall_dist = vec![99i32; (grid.w * grid.d) as usize];
    for v in 0..grid.d {
        for u in 0..grid.w {
            if is_fp(grid, tall_fp, u, v) {
                tall_dist[grid.idx(u, v)] = 0;
            }
        }
    }
    for _pass in 0..8 {
        for v in 0..grid.d {
            for u in 0..grid.w {
                if tall_dist[grid.idx(u, v)] == 0 {
                    continue;
                }
                let mut m = tall_dist[grid.idx(u, v)];
                for (du, dv) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    if grid.in_bounds(u + du, v + dv) {
                        m = m.min(tall_dist[grid.idx(u + du, v + dv)] + 1);
                    }
                }
                tall_dist[grid.idx(u, v)] = m;
            }
        }
    }

    let max_d = wing_fp
        .iter()
        .enumerate()
        .filter(|&(idx, &in_w)| in_w && tall_dist[idx] < 90)
        .map(|(idx, _)| tall_dist[idx])
        .max()
        .unwrap_or(2);

    // Slope upward from outer wing margin toward shared tall wall (`d == 1`), plus 1-block eave!
    for v in 0..grid.d {
        for u in 0..grid.w {
            let in_w = wing_fp[grid.idx(u, v)];
            let near_w = !in_w
                && !is_fp(grid, tall_fp, u, v)
                && (is_fp(grid, wing_fp, u - 1, v)
                    || is_fp(grid, wing_fp, u + 1, v)
                    || is_fp(grid, wing_fp, u, v - 1)
                    || is_fp(grid, wing_fp, u, v + 1));
            if !in_w && !near_w {
                continue;
            }
            let d = tall_dist[grid.idx(u, v)].min(max_d + 1);
            let step = (max_d + 1 - d).max(0).min(3);
            let ry = (base_y + step).min(i32::from(height) - 1);

            if in_w {
                for wy in base_y..ry {
                    set_uv(blocks, grid, u, wy, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, wy, v)));
                }
            }
            let (odu, odv) = if grid.in_bounds(u - 1, v) && tall_dist[grid.idx(u - 1, v)] < d {
                (1, 0)
            } else if grid.in_bounds(u + 1, v) && tall_dist[grid.idx(u + 1, v)] < d {
                (-1, 0)
            } else if grid.in_bounds(u, v - 1) && tall_dist[grid.idx(u, v - 1)] < d {
                (0, 1)
            } else {
                (0, -1)
            };
            let face = grid.uv_to_mc_dir(-odu, -odv);
            if d == 1 {
                let picked_roof = pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                let crest_blk = if !pal.has_extra(MaterialPalette::SLOT_ROOF) && picked_roof == "minecraft:deepslate_tiles" {
                    "minecraft:cracked_deepslate_tiles".to_owned()
                } else {
                    picked_roof
                };
                set_uv(blocks, grid, u, ry, v, PlannedBlock::simple(crest_blk));
                if ry + 1 < i32::from(height) && !is_fp(grid, wing_fp, u, v - 1) {
                    let s = pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry + 1, v);
                    let horn = if !pal.has_extra(MaterialPalette::SLOT_ROOF) && (s.contains("deepslate") || s.contains("blackstone")) {
                        "minecraft:stone_brick_slab".to_owned()
                    } else {
                        s
                    };
                    set_uv(blocks, grid, u, ry + 1, v, make_slab(&horn, "bottom"));
                }
            } else if near_w {
                set_uv(blocks, grid, u, ry, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry, v), "top"));
            } else {
                set_uv(blocks, grid, u, ry, v, make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v), face, "bottom"));
            }
        }
    }
}

/// Pitches a perpendicular cross-gable roof across a lower stepped wing toward the tall core.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_wing_cross_gable_roof(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    wing_fp: &[bool],
    tall_fp: &[bool],
    pal: &MaterialPalette,
    seed: u64,
    base_y: i32,
    height: u16,
) {
    let mut wu0 = grid.w;
    let mut wv0 = grid.d;
    let mut wu1 = 0;
    let mut wv1 = 0;
    for v in 0..grid.d {
        for u in 0..grid.w {
            if wing_fp[grid.idx(u, v)] {
                wu0 = wu0.min(u);
                wv0 = wv0.min(v);
                wu1 = wu1.max(u);
                wv1 = wv1.max(v);
            }
        }
    }
    if wu1 <= wu0 || wv1 - wv0 < 3 {
        build_wing_sloped_roof(blocks, grid, wing_fp, tall_fp, pal, seed, base_y, height);
        return;
    }

    let wing_on_left = is_fp(grid, tall_fp, wu1 + 1, (wv0 + wv1) / 2);
    let span_v = wv1 - wv0 + 1;
    let half_v = ((span_v + 1) / 2).min(4);
    let outer_u = if wing_on_left { wu0 } else { wu1 };
    let eave_u = if wing_on_left { wu0 - 1 } else { wu1 + 1 };

    let ridge_slab_unused = if pal.roof_slab.contains("deepslate") || pal.roof_slab.contains("blackstone") {
        "minecraft:stone_brick_slab"
    } else {
        pal.roof_slab.as_str()
    };
    let _ = ridge_slab_unused;

    for v in (wv0 - 1).max(0)..=(wv1 + 1).min(grid.d - 1) {
        let dl = (v - wv0).max(0);
        let dr = (wv1 - v).max(0);
        let in_v = v >= wv0 && v <= wv1;
        let step = if !in_v { 0 } else { dl.min(dr).min(half_v) };
        let ry = (base_y + step).min(i32::from(height) - 1);

        let u_start = if wing_on_left { (wu0 - 1).max(0) } else { wu0 };
        let u_end = if wing_on_left { wu1 } else { (wu1 + 1).min(grid.w - 1) };
        for u in u_start..=u_end {
            let in_wing = is_fp(grid, wing_fp, u, v);
            let is_end = u == outer_u || u == eave_u;
            if is_fp(grid, tall_fp, u, v) {
                continue;
            }
            if !in_wing && !(u == eave_u && in_v) && !(in_wing == false && !in_v && u >= wu0 && u <= wu1) {
                continue;
            }
            if u == outer_u && in_v && step > 0 {
                let center_dist = (dl - dr).abs();
                let side_attic = center_dist == 2 && step >= 4 && (base_y + 3) < ry;
                for wy in (base_y + 1)..ry {
                    let blk = if side_attic && wy == base_y + 2 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, wy, v))
                    } else if center_dist == 0 {
                        make_frame_block_at(pal, "y", seed, u, wy, v)
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, wy, v))
                    };
                    set_uv(blocks, grid, u, wy, v, blk);
                }
            }
            if in_v && dl == dr && step >= 1 {
                let r_sl = pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, ry, v);
                set_uv(blocks, grid, u, ry, v, make_slab(&r_sl, "bottom"));
                if is_end && ry + 1 < i32::from(height) {
                    set_uv(blocks, grid, u, ry + 1, v, make_slab(&r_sl, "bottom"));
                }
            } else if v <= (wv0 + wv1) / 2 {
                set_uv(
                    blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v), grid.dir_front(), "bottom"),
                );
                if in_wing
                    && u != outer_u
                    && ry - 1 > base_y
                    && !crate::gen_common::is_solid_or_occupied(blocks, grid, u, ry - 1, v)
                {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v)),
                    );
                }
            } else {
                set_uv(
                    blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, ry, v), grid.dir_back(), "bottom"),
                );
                if in_wing
                    && u != outer_u
                    && ry - 1 > base_y
                    && !crate::gen_common::is_solid_or_occupied(blocks, grid, u, ry - 1, v)
                {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        ry - 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry - 1, v)),
                    );
                }
            }
        }
    }
}

/// Adds a first-floor horizontal waist eave skirt (`mokoshi`) along multi-story facades.
pub(crate) fn build_first_floor_mokoshi_skirt(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    base_fp: &[bool],
    upper_fp: &[bool],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    skirt_y: i32,
) {
    if spec.floors < 2
        || matches!(
            spec.style,
            BuildStyle::Modern | BuildStyle::Desert | BuildStyle::Gothic | BuildStyle::Steampunk | BuildStyle::Classical
        )
        || matches!(spec.category, BuildCategory::Pavilion | BuildCategory::Bridge)
    {
        return;
    }
    let mut diag_corners = Vec::new();
    for v in 0..grid.d {
        for u in 0..grid.w {
            if is_fp(grid, base_fp, u, v) || is_fp(grid, upper_fp, u, v) {
                continue;
            }
            if is_solid_or_occupied(blocks, grid, u, skirt_y, v) {
                continue;
            }
            let orth = is_fp(grid, base_fp, u - 1, v)
                || is_fp(grid, base_fp, u + 1, v)
                || is_fp(grid, base_fp, u, v - 1)
                || is_fp(grid, base_fp, u, v + 1);
            let diag = is_fp(grid, base_fp, u - 1, v - 1)
                || is_fp(grid, base_fp, u + 1, v - 1)
                || is_fp(grid, base_fp, u - 1, v + 1)
                || is_fp(grid, base_fp, u + 1, v + 1);
            if orth {
                let face = if is_fp(grid, base_fp, u, v - 1) {
                    grid.dir_back()
                } else if is_fp(grid, base_fp, u, v + 1) {
                    grid.dir_front()
                } else if is_fp(grid, base_fp, u - 1, v) {
                    grid.dir_left()
                } else {
                    grid.dir_right()
                };
                set_uv(
                    blocks,
                    grid,
                    u,
                    skirt_y,
                    v,
                    make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, skirt_y, v), face, "bottom"),
                );
            } else if diag && spec.style == BuildStyle::Japanese {
                diag_corners.push((u, v));
            }
        }
    }
    for (u, v) in diag_corners {
        let touches_eave = [(-1, 0), (1, 0), (0, -1), (0, 1)]
            .into_iter()
            .any(|(du, dv)| is_solid_or_occupied(blocks, grid, u + du, skirt_y, v + dv));
        if touches_eave {
            set_uv(blocks, grid, u, skirt_y, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, skirt_y, v), "bottom"));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_greenhouse_roof(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    bounds: [i32; 4],
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
    roof_base_y: i32,
    max_roof_span: i32,
) -> i32 {
    let [b_u0, _b_v0, b_u1, _b_v1] = bounds;
    let span_u = (b_u1 - b_u0 + 1).max(3);
    let half = ((span_u + 1) / 2).min(max_roof_span);
    let mut max_y = roof_base_y;

    for v in 0..grid.d {
        for u in 0..grid.w {
            if !roof_fp[grid.idx(u, v)] {
                continue;
            }
            let dist_u = (u - b_u0).min(b_u1 - u);
            let step = dist_u.min(half);
            let ry = (roof_base_y + step).min(i32::from(height) - 1);
            max_y = max_y.max(ry);
            let is_rib = (u - b_u0) == 0 || (b_u1 - u) == 0 || v % 3 == 0;
            let blk = if is_rib {
                make_frame_block_at(pal, grid.axis_u(), seed, u, ry, v)
            } else {
                PlannedBlock::simple("minecraft:glass")
            };
            set_uv(blocks, grid, u, ry, v, blk);
        }
    }
    max_y
}

#[allow(clippy::too_many_arguments)]
fn build_lighthouse_lantern(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    bounds: [i32; 4],
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
    roof_base_y: i32,
) -> i32 {
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let cu = (b_u0 + b_u1) / 2;
    let cv = (b_v0 + b_v1) / 2;
    let mut max_y = roof_base_y;

    // Observation deck floor & gallery perimeter fence.
    for v in (b_v0 - 1)..=(b_v1 + 1) {
        for u in (b_u0 - 1)..=(b_u1 + 1) {
            if !grid.in_bounds(u, v) {
                continue;
            }
            let near_fp = is_fp(grid, roof_fp, u, v)
                || is_fp(grid, roof_fp, u - 1, v)
                || is_fp(grid, roof_fp, u + 1, v)
                || is_fp(grid, roof_fp, u, v - 1)
                || is_fp(grid, roof_fp, u, v + 1);
            if near_fp {
                set_uv(
                    blocks,
                    grid,
                    u,
                    roof_base_y,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, roof_base_y, v)),
                );
                if !is_fp(grid, roof_fp, u, v) && roof_base_y + 1 < i32::from(height) {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        roof_base_y + 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, roof_base_y + 1, v)),
                    );
                }
            }
        }
    }

    // Glowing lantern chamber (`glass` enclosure around bright `sea_lantern` / `ochre_froglight` beacon).
    let chamber_h = (i32::from(height) - 2 - roof_base_y).clamp(1, 3);
    for du in -1..=1 {
        for dv in -1..=1 {
            let u = cu + du;
            let v = cv + dv;
            if !grid.in_bounds(u, v) {
                continue;
            }
            for dy in 1..=chamber_h {
                let y = roof_base_y + dy;
                let blk = if du == 0 && dv == 0 {
                    PlannedBlock::simple(if pal.has_extra(MaterialPalette::SLOT_LIGHT) {
                        pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, y, v)
                    } else if dy == 2 {
                        "minecraft:ochre_froglight".to_owned()
                    } else {
                        "minecraft:sea_lantern".to_owned()
                    })
                } else if du.abs() == 1 && dv.abs() == 1 {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, y, v))
                } else {
                    PlannedBlock::simple("minecraft:glass")
                };
                set_uv(blocks, grid, u, y, v, blk);
                max_y = max_y.max(y);
            }
            let cap_y = (roof_base_y + chamber_h + 1).min(i32::from(height) - 1);
            let cap = if du == 0 && dv == 0 {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, cap_y, v))
            } else {
                make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, cap_y, v), "bottom")
            };
            set_uv(blocks, grid, u, cap_y, v, cap);
            max_y = max_y.max(cap_y);
        }
    }
    if max_y + 1 < i32::from(height) {
        set_uv(
            blocks,
            grid,
            cu,
            max_y + 1,
            cv,
            PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
        );
        max_y += 1;
    }
    max_y
}

#[allow(clippy::too_many_arguments)]
fn attach_windmill_sails(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    roof_fp: &[bool],
    bounds: [i32; 4],
    roof_base_y: i32,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
) {
    let [b_u0, _b_v0, b_u1, _b_v1] = bounds;
    let hu = (b_u0 + b_u1) / 2;
    let hub_hi = (i32::from(height) - 2).max(2);
    let hub_y = crate::gen_common::clamp_i32(roof_base_y - 2, 4, hub_hi);
    // Find front wall surface at hub u.
    let front_v = (0..grid.d)
        .rev()
        .find(|&v| is_fp(grid, roof_fp, hu, v))
        .unwrap_or(grid.d - 2);
    let axle_v = front_v + 1;
    let sail_v = front_v + 2;
    if !grid.in_bounds(hu, axle_v) {
        return;
    }
    set_uv(blocks, grid, hu, hub_y, front_v, make_frame_block_at(pal, grid.axis_v(), seed, hu, hub_y, front_v));
    set_uv(blocks, grid, hu, hub_y, axle_v, make_frame_block_at(pal, grid.axis_v(), seed, hu, hub_y, axle_v));

    let use_v = if grid.in_bounds(hu, sail_v) { sail_v } else { axle_v };
    let arm_len = ((grid.w - 1) / 3).clamp(3, 5);
    set_uv(blocks, grid, hu, hub_y, use_v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, hu, hub_y, use_v)));

    for arm in 0..arm_len {
        let off = arm + 1;
        for &(du, dy, su, sy, su2, sy2) in &[
            (off, off, 1, 0, 2, 0),
            (-off, off, -1, 0, -2, 0),
            (off, -off, 0, -1, 0, -2),
            (-off, -off, 0, 1, 0, 2),
        ] {
            let u = hu + du;
            let y = hub_y + dy;
            if grid.in_bounds(u, use_v) && y >= 2 && y < i32::from(height) {
                set_uv(blocks, grid, u, y, use_v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, y, use_v)));
            }
            for (si, (wu, wy)) in [(u + su, y + sy), (u + su2, y + sy2)].into_iter().enumerate() {
                if grid.in_bounds(wu, use_v) && wy >= 2 && wy < i32::from(height) && arm >= 1 {
                    let sail_blk = if si == 0 && !pal.has_extra(MaterialPalette::SLOT_AWNING) {
                        "minecraft:white_wool".to_owned()
                    } else {
                        pal.pick(MaterialPalette::SLOT_AWNING, seed, wu, wy, use_v)
                    };
                    set_uv(
                        blocks,
                        grid,
                        wu,
                        wy,
                        use_v,
                        PlannedBlock::simple(sail_blk),
                    );
                }
            }
        }
    }
}
