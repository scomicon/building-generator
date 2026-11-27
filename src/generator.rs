use std::collections::BTreeMap;

pub use crate::gen_common::{GeneratedBlueprint, PlannedBlock};
use crate::gen_common::{
    LocalGrid, compute_tight_bounds, fill_rect, is_fp, is_lantern, is_solid_or_occupied,
    make_beam_block_at, make_block_with_axis, make_frame_block_at, make_pillar_block_at,
    make_slab, make_stair, resolve_connections_and_shapes, set_uv, terrain_rel_at,
};
use crate::gen_exterior::{
    build_bay_windows, build_courtyard_and_landscaping, build_custom_yard_trees,
    build_downspouts_and_signs, build_entrance_and_porch, build_foundation_and_basement,
    build_lantern_cranes, build_market_awning_stall, build_windows_and_balcony,
    decorate_living_exteriors,
};
use crate::gen_interior::{
    build_fireplace_and_chimney, build_interior_zoning, build_staircases,
    decorate_living_interiors, wood_prefix_for_palette,
};
use crate::gen_roof::{
    build_first_floor_mokoshi_skirt, build_roof_and_dormers, build_wing_cross_gable_roof,
    build_wing_sloped_roof,
};
use crate::model::{BuildCategory, BuildRequest, BuildStyle, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, StructureTheme};

fn ensure_valid_front_door_spot(
    grid: LocalGrid,
    fp: &mut [bool],
    min_u: i32,
    max_u: i32,
    min_v: i32,
    max_v: i32,
) {
    for v in min_v..=max_v {
        for u in (min_u + 1)..=(max_u - 1) {
            if is_fp(grid, fp, u, v)
                && !is_fp(grid, fp, u, v + 1)
                && is_fp(grid, fp, u - 1, v)
                && is_fp(grid, fp, u + 1, v)
            {
                return;
            }
        }
    }
    let cu = (min_u + max_u) / 2;
    let fv = (max_v - 1).max(min_v + 2);
    fill_rect(fp, grid, cu - 1, fv - 2, cu + 1, fv, true);
}

/// Sculpt architectural 2D footprint using noise + category + style rules, reserving outer rim for depth trim & landscaping.
fn build_footprint(
    grid: LocalGrid,
    spec: &BlueprintSpec,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
    rng: &mut SplitMix64,
) -> (Vec<bool>, [i32; 4]) {
    let mut fp = vec![false; (grid.w * grid.d) as usize];
    let outer_trim = if spec.exterior_depth && grid.w >= 11 && grid.d >= 10 {
        2
    } else if grid.w >= 9 {
        1
    } else {
        0
    };
    let pad_side = outer_trim
        .min((grid.w - 6) / 2)
        .max(if grid.w >= 9 { 1 } else { 0 });
    let pad_back = if spec.overhang_jetty || spec.roof_overhang {
        if grid.d >= 11 {
            2
        } else if grid.d >= 9 {
            1
        } else {
            0
        }
    } else {
        0
    };
    let front_slopes_down = terrain_offsets.is_some_and(|_| {
        let mid_u = grid.w / 2;
        terrain_rel_at(terrain_offsets, grid, mid_u, grid.d - 2) <= -2
    });
    let pad_front = if (spec.courtyard_well && grid.d >= 18) || (front_slopes_down && grid.d >= 14) {
        5
    } else if (spec.courtyard_well && grid.d >= 16)
        || (spec.porch && spec.style == BuildStyle::Classical && grid.d >= 11)
    {
        4
    } else if spec.porch && grid.d >= 11 {
        3
    } else if spec.porch && grid.d >= 9 {
        2
    } else if grid.d >= 9 {
        1
    } else {
        0
    };

    let min_u = pad_side;
    let max_u = (grid.w - 1 - pad_side).max(min_u + 4);
    let min_v = pad_back;
    let max_v = (grid.d - 1 - pad_front).max(min_v + 4);
    let span_u = max_u - min_u + 1;
    let span_v = max_v - min_v + 1;

    match spec.category {
        BuildCategory::Tower => {
            let cu = (min_u + max_u) as f64 * 0.5;
            let cv = (min_v + max_v) as f64 * 0.5;
            let scale = 0.88 + rng.next_f64() * 0.14;
            let r_base = (((span_u.min(span_v) as f64) * 0.48) * scale).max(2.4);
            let tower_shape = if matches!(spec.theme, StructureTheme::Windmill | StructureTheme::Lighthouse) {
                rng.range_i32(0, 1)
            } else {
                rng.range_i32(0, 3)
            };
            for v in min_v..=max_v {
                for u in min_u..=max_u {
                    let du = (u as f64 - cu).abs() / r_base;
                    let dv = (v as f64 - cv).abs() / r_base;
                    let n = (noise.fbm_2d(u as f64 * 0.35, v as f64 * 0.35, 2) - 0.5)
                        * 0.20
                        * f64::from(spec.asymmetry);
                    let inside = match tower_shape {
                        0 => (du * du + dv * dv).sqrt() <= 1.04 + n,
                        1 => du.max(dv) <= 1.02 + n * 0.4 && (du + dv) <= 1.44 + n,
                        2 => du <= 0.55 || dv <= 0.55, // Cruciform buttressed keep tower.
                        _ => du.max(dv) <= 0.96 + n * 0.4,
                    };
                    if inside {
                        fp[grid.idx(u, v)] = true;
                    }
                }
            }
            ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
            let b = compute_tight_bounds(grid, &fp);
            return (fp, b);
        }
        BuildCategory::Pavilion => {
            if spec.theme == StructureTheme::Greenhouse {
                fill_rect(&mut fp, grid, min_u, min_v, max_u, max_v, true);
            } else {
                let cu = (min_u + max_u) as f64 * 0.5;
                let cv = (min_v + max_v) as f64 * 0.5;
                let rx = ((span_u as f64) * 0.48).max(2.5);
                let rz = ((span_v as f64) * 0.48).max(2.5);
                let pav_shape = rng.range_i32(0, 3);
                for v in min_v..=max_v {
                    for u in min_u..=max_u {
                        let du = (u as f64 - cu).abs() / rx;
                        let dv = (v as f64 - cv).abs() / rz;
                        let inside = match pav_shape {
                            0 => true,
                            1 => (du + dv) <= 1.48,
                            2 => (du * du + dv * dv).sqrt() <= 1.04,
                            _ => du <= 0.52 || dv <= 0.52,
                        };
                        if inside {
                            fp[grid.idx(u, v)] = true;
                        }
                    }
                }
            }
            ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
            let b = compute_tight_bounds(grid, &fp);
            return (fp, b);
        }
        BuildCategory::Castle => {
            // Mix castle keep silhouette using NoiseField so House/Castle RNG stream remains unbroken.
            let castle_shape = if span_u >= 15 && span_v >= 13 {
                ((noise.cell_hash(min_u, 11, max_v) * 3.0) as i32).clamp(0, 2)
            } else {
                0
            };
            let inset_u = if span_u >= 11 { 1 } else { 0 };
            let inset_v = if span_v >= 9 { 1 } else { 0 };
            let core_u0 = min_u + inset_u;
            let core_u1 = max_u - inset_u;
            let core_v0 = min_v + inset_v;
            let core_v1 = max_v - inset_v;
            fill_rect(&mut fp, grid, core_u0, core_v0, core_u1, core_v1, true);

            if span_u >= 9 && span_v >= 7 {
                for &(cu, cv) in &[
                    (min_u + 1, min_v + 1),
                    (max_u - 1, min_v + 1),
                    (min_u + 1, max_v - 1),
                    (max_u - 1, max_v - 1),
                ] {
                    fill_rect(&mut fp, grid, cu - 1, cv - 1, cu + 1, cv + 1, true);
                }
            }
            if castle_shape == 1 {
                // Royal Barbican Keep: protruding front gatehouse pavilion between front towers.
                let mu = (core_u0 + core_u1) / 2;
                fill_rect(&mut fp, grid, mu - 2, core_v1, mu + 2, max_v, true);
            } else if castle_shape == 2 {
                // Star-Bastion Citadel: axial rear apse tower & protruding flank bastions.
                let mu = (core_u0 + core_u1) / 2;
                let mv = (core_v0 + core_v1) / 2;
                fill_rect(&mut fp, grid, min_u, mv - 2, core_u0, mv + 2, true);
                fill_rect(&mut fp, grid, core_u1, mv - 2, max_u, mv + 2, true);
                fill_rect(&mut fp, grid, mu - 2, min_v, mu + 2, core_v0, true);
            }
            ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
            let b = compute_tight_bounds(grid, &fp);
            return (fp, b);
        }
        BuildCategory::Temple => {
            let temple_shape = if span_u >= 11 && span_v >= 9 {
                rng.range_i32(0, 3)
            } else {
                0
            };
            match temple_shape {
                1 => {
                    // Classical Parthenon / Peripteral Basilica stylobate rectangle.
                    let inset_u = if span_u >= 13 { 1 } else { 0 };
                    fill_rect(&mut fp, grid, min_u + inset_u, min_v, max_u - inset_u, max_v, true);
                }
                2 => {
                    // Sacred Octagonal Rotunda Temple.
                    let cu = (min_u + max_u) as f64 * 0.5;
                    let cv = (min_v + max_v) as f64 * 0.5;
                    let r = ((span_u.min(span_v) as f64) * 0.48).max(2.5);
                    for v in min_v..=max_v {
                        for u in min_u..=max_u {
                            let du = (u as f64 - cu).abs() / r;
                            let dv = (v as f64 - cv).abs() / r;
                            if (du + dv) <= 1.46 && du <= 1.0 && dv <= 1.0 {
                                fp[grid.idx(u, v)] = true;
                            }
                        }
                    }
                }
                3 if span_v >= 8 => {
                    // T-Shaped Rear Choir Transept Temple.
                    let inset_u = (span_u / 5).max(1);
                    fill_rect(&mut fp, grid, min_u + inset_u, min_v, max_u - inset_u, max_v, true);
                    fill_rect(&mut fp, grid, min_u, min_v + 1, max_u, (min_v + span_v / 3).min(max_v), true);
                }
                _ => {
                    let inset_u = (span_u / 5).max(1);
                    fill_rect(
                        &mut fp,
                        grid,
                        min_u + inset_u,
                        min_v,
                        max_u - inset_u,
                        max_v,
                        true,
                    );
                    if span_v >= 7 {
                        let tv0 = min_v + (span_v / 4);
                        let tv1 = (max_v - (span_v / 4)).max(tv0 + 2);
                        fill_rect(&mut fp, grid, min_u, tv0, max_u, tv1, true);
                    }
                }
            }
            ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
            let b = compute_tight_bounds(grid, &fp);
            return (fp, b);
        }
        BuildCategory::Barn | BuildCategory::Forge => {
            let inset_u = if span_u >= 11 && rng.next_f64() < 0.4 { 1 } else { 0 };
            fill_rect(&mut fp, grid, min_u + inset_u, min_v, max_u - inset_u, max_v, true);
            if span_u >= 10 && span_v >= 7 {
                let left = rng.next_f64() < 0.5;
                let cut_w = 2;
                let cut_front = spec.category == BuildCategory::Forge || rng.next_f64() < 0.5;
                let cu0 = if left {
                    min_u + inset_u
                } else {
                    max_u - inset_u - cut_w + 1
                };
                let cu1 = cu0 + cut_w - 1;
                if cut_front {
                    fill_rect(&mut fp, grid, cu0, max_v - 2, cu1, max_v, false);
                } else {
                    fill_rect(&mut fp, grid, cu0, min_v, cu1, min_v + 1, false);
                }
            }
            ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
            let b = compute_tight_bounds(grid, &fp);
            return (fp, b);
        }
        BuildCategory::Bridge
        | BuildCategory::Settlement
        | BuildCategory::Dungeon
        | BuildCategory::Estate
        | BuildCategory::Engineering
        | BuildCategory::Agricultural
        | BuildCategory::DecorArt => {
            fill_rect(&mut fp, grid, 0, 0, grid.w - 1, grid.d - 1, true);
            return (fp, [0, 0, grid.w - 1, grid.d - 1]);
        }
        BuildCategory::House => {}
    }

    let archetype = if spec.symmetric_modern {
        if span_u >= 12 && span_v >= 8 && rng.next_f64() < 0.45 {
            4 // Symmetric U-Shaped Courtyard Villa
        } else if span_u >= 10 && span_v >= 6 && rng.next_f64() < 0.55 {
            1
        } else {
            3
        }
    } else {
        let roll = rng.next_f64();
        if span_u >= 12 && span_v >= 8 && roll < 0.16 {
            4 // U-Shaped Villa
        } else if span_u >= 11 && span_v >= 11 && roll < 0.28 {
            5 // Chamfered Octagonal Manor
        } else if span_u >= 8 && span_v >= 6 && spec.wing_bias > 0.35 {
            if roll < 0.52 {
                0
            } else if roll < 0.72 {
                1
            } else if roll < 0.90 {
                2
            } else {
                3
            }
        } else if roll < 0.58 {
            2
        } else {
            3
        }
    };

    match archetype {
        0 => {
            fill_rect(&mut fp, grid, min_u, min_v, max_u, max_v, true);
            let cut_left = rng.next_f64() < 0.5;
            let cut_front = rng.next_f64() < 0.45;
            let max_cut_u = (span_u - 5).max(2).min(span_u / 2);
            let max_cut_v = (span_v - 4).max(2).min(span_v / 2);
            let cu = rng.range_i32(2, max_cut_u);
            let cv = rng.range_i32(2, max_cut_v);
            let u0 = if cut_left { min_u } else { max_u - cu + 1 };
            let u1 = u0 + cu - 1;
            let v0 = if cut_front { max_v - cv + 1 } else { min_v };
            let v1 = v0 + cv - 1;
            fill_rect(&mut fp, grid, u0, v0, u1, v1, false);
        }
        1 => {
            let wing_on_front = rng.next_f64() < 0.5;
            let spine_inset_l = if spec.symmetric_modern {
                (span_u / 5).max(1)
            } else {
                rng.range_i32(1, (span_u / 3).max(1))
            };
            let spine_inset_r = if spec.symmetric_modern {
                spine_inset_l
            } else {
                rng.range_i32(1, (span_u / 3).max(1))
            };
            let su0 = min_u + spine_inset_l;
            let su1 = (max_u - spine_inset_r).max(su0 + 3).min(max_u);
            fill_rect(&mut fp, grid, su0, min_v, su1, max_v, true);

            let bar_depth = rng.range_i32(3.min(span_v - 2), (span_v * 2 / 3).max(3));
            if wing_on_front {
                fill_rect(&mut fp, grid, min_u, max_v - bar_depth + 1, max_u, max_v, true);
            } else {
                fill_rect(&mut fp, grid, min_u, min_v, max_u, min_v + bar_depth - 1, true);
            }
        }
        2 => {
            let core_w = crate::gen_common::clamp_i32(span_u * 2 / 3, 5, span_u);
            let core_d = crate::gen_common::clamp_i32(span_v * 2 / 3, 4, span_v);
            let shift_u = rng.range_i32(0, (span_u - core_w).max(0));
            let shift_v = rng.range_i32(0, (span_v - core_d).max(0));
            let cu0 = min_u + shift_u;
            let cv0 = min_v + shift_v;
            let cu1 = cu0 + core_w - 1;
            let cv1 = cv0 + core_d - 1;
            fill_rect(&mut fp, grid, cu0, cv0, cu1, cv1, true);

            let center_u = (cu0 + cu1) as f64 * 0.5;
            let center_v = (cv0 + cv1) as f64 * 0.5;
            for v in min_v..=max_v {
                for u in min_u..=max_u {
                    let n = noise.fbm_2d(u as f64 * 0.36, v as f64 * 0.36, 3);
                    let dx = (u as f64 - center_u).abs() / ((span_u as f64 * 0.5).max(1.0));
                    let dv = (v as f64 - center_v).abs() / ((span_v as f64 * 0.5).max(1.0));
                    let falloff = (dx * 0.45 + dv * 0.45).min(0.65);
                    if n - falloff * 0.38 > 0.44 {
                        fp[grid.idx(u, v)] = true;
                    }
                }
            }
            for _ in 0..2 {
                let prev = fp.clone();
                for v in min_v..=max_v {
                    for u in min_u..=max_u {
                        let orth = [
                            is_fp(grid, &prev, u - 1, v),
                            is_fp(grid, &prev, u + 1, v),
                            is_fp(grid, &prev, u, v - 1),
                            is_fp(grid, &prev, u, v + 1),
                        ];
                        let cnt = orth.into_iter().filter(|&x| x).count();
                        let h_pair = is_fp(grid, &prev, u - 1, v) && is_fp(grid, &prev, u + 1, v);
                        let v_pair = is_fp(grid, &prev, u, v - 1) && is_fp(grid, &prev, u, v + 1);
                        if prev[grid.idx(u, v)] && cnt <= 1 {
                            fp[grid.idx(u, v)] = false;
                        } else if !prev[grid.idx(u, v)] && (cnt >= 3 || (h_pair && v_pair)) {
                            fp[grid.idx(u, v)] = true;
                        }
                    }
                }
            }
        }
        4 => {
            // U-shaped Courtyard Villa embracing a front garden entry court.
            fill_rect(&mut fp, grid, min_u, min_v, max_u, max_v, true);
            let arm_w = (span_u / 3).clamp(3, 4);
            let court_v0 = min_v + (span_v / 2).max(3);
            let cu0 = min_u + arm_w;
            let cu1 = max_u - arm_w;
            if cu1 >= cu0 + 2 && court_v0 <= max_v {
                fill_rect(&mut fp, grid, cu0, court_v0, cu1, max_v, false);
            }
        }
        5 => {
            // Chamfered Octagonal Manor footprint.
            let cu = (min_u + max_u) as f64 * 0.5;
            let cv = (min_v + max_v) as f64 * 0.5;
            let rx = ((span_u as f64) * 0.5).max(3.0);
            let rz = ((span_v as f64) * 0.5).max(3.0);
            for v in min_v..=max_v {
                for u in min_u..=max_u {
                    let du = (u as f64 - cu).abs() / rx;
                    let dv = (v as f64 - cv).abs() / rz;
                    if du + dv <= 1.48 {
                        fp[grid.idx(u, v)] = true;
                    }
                }
            }
        }
        _ => {
            let shrink_l = if span_u >= 10 && !spec.symmetric_modern {
                rng.range_i32(0, 2)
            } else {
                0
            };
            let shrink_r = if span_u >= 10 && !spec.symmetric_modern {
                rng.range_i32(0, 2)
            } else {
                0
            };
            let shrink_b = if span_v >= 8 && !spec.symmetric_modern {
                rng.range_i32(0, 1)
            } else {
                0
            };
            let u0 = min_u + shrink_l;
            let u1 = (max_u - shrink_r).max(u0 + 5).min(max_u);
            let v0 = min_v + shrink_b;
            fill_rect(&mut fp, grid, u0, v0, u1, max_v, true);

            if !spec.symmetric_modern && v0 > min_v {
                let bump_u = rng.range_i32(u0 + 2, (u1 - 2).max(u0 + 2));
                fill_rect(&mut fp, grid, bump_u - 1, v0 - 1, bump_u + 1, v0 - 1, true);
            }
        }
    }

    ensure_valid_front_door_spot(grid, &mut fp, min_u, max_u, min_v, max_v);
    let bounds = compute_tight_bounds(grid, &fp);
    (fp, bounds)
}

/// Splits large multi-story footprints into an asymmetric lower wing + taller main hall.
fn compute_stepped_wing_masks(
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    spec: &BlueprintSpec,
    rng: &mut SplitMix64,
) -> Option<(Vec<bool>, bool)> {
    if !spec.stepped_wing || spec.floors < 2 {
        return None;
    }
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let span_u = b_u1 - b_u0 + 1;
    let span_v = b_v1 - b_v0 + 1;
    if span_u < 9 || span_v < 6 {
        return None;
    }

    let cut_w = ((span_u * 38) / 100).clamp(3, 6).min(span_u - 6);
    let left_wing = rng.next_f64() < 0.5;
    let (cut_u0, cut_u1) = if left_wing {
        (b_u0, b_u0 + cut_w - 1)
    } else {
        (b_u1 - cut_w + 1, b_u1)
    };

    let mut tall_fp = fp.to_vec();
    let mut lowered = 0usize;
    let mut remained = 0usize;
    for v in b_v0..=b_v1 {
        for u in b_u0..=b_u1 {
            let idx = grid.idx(u, v);
            if !tall_fp[idx] {
                continue;
            }
            if u >= cut_u0 && u <= cut_u1 {
                tall_fp[idx] = false;
                lowered += 1;
            } else {
                remained += 1;
            }
        }
    }
    if lowered < 6 || remained < 24 {
        return None;
    }
    let terrace_mode = spec.style == BuildStyle::Modern
        || matches!(spec.style, BuildStyle::Japanese | BuildStyle::Cottage | BuildStyle::Medieval)
            && rng.next_f64() < 0.58;
    Some((tall_fp, terrace_mode))
}

/// Crowns a stepped 1st-story side wing with an upper balcony garden terrace or a tucked wing roof.
#[allow(clippy::too_many_arguments)]
fn build_stepped_wing_top(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    tall_fp: &[bool],
    roof_fp: &[bool],
    terrace_on_lower: bool,
    cross_gable: bool,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
) {
    let lower_roof_y = i32::from(spec.foundation_height) + i32::from(spec.floor_height);
    let lower_only: Vec<bool> = (0..fp.len()).map(|i| fp[i] && !tall_fp[i]).collect();

    if !terrace_on_lower {
        if cross_gable {
            build_wing_cross_gable_roof(blocks, grid, &lower_only, tall_fp, pal, seed, lower_roof_y, height);
        } else {
            build_wing_sloped_roof(blocks, grid, &lower_only, tall_fp, pal, seed, lower_roof_y, height);
        }
        return;
    }

    let mut door_done = false;
    let mut planter_slots = 0usize;
    let mid_v = grid.d / 2;

    for v in 0..grid.d {
        for u in 0..grid.w {
            if !lower_only[grid.idx(u, v)] {
                continue;
            }
            set_uv(
                blocks,
                grid,
                u,
                lower_roof_y,
                v,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, lower_roof_y, v)),
            );

            // Connect Floor 2 of the tall main hall onto this open terrace with a full doorway!
            let shared_left = is_fp(grid, tall_fp, u - 1, v);
            let shared_right = is_fp(grid, tall_fp, u + 1, v);
            if !door_done
                && (shared_left || shared_right)
                && (v - mid_v).abs() <= 1
                && lower_roof_y + 3 < i32::from(height)
            {
                let tall_u = if shared_left { u - 1 } else { u + 1 };
                let face_out = if shared_left {
                    grid.dir_right()
                } else {
                    grid.dir_left()
                };
                let terrace_door = pal.pick(MaterialPalette::SLOT_DOOR, seed, tall_u, lower_roof_y + 1, v);
                set_uv(
                    blocks,
                    grid,
                    tall_u,
                    lower_roof_y + 1,
                    v,
                    PlannedBlock::with_props(
                        terrace_door.clone(),
                        vec![
                            ("facing", face_out),
                            ("half", "lower"),
                            ("hinge", "left"),
                            ("open", "false"),
                        ],
                    ),
                );
                set_uv(
                    blocks,
                    grid,
                    tall_u,
                    lower_roof_y + 2,
                    v,
                    PlannedBlock::with_props(
                        terrace_door,
                        vec![
                            ("facing", face_out),
                            ("half", "upper"),
                            ("hinge", "left"),
                            ("open", "false"),
                        ],
                    ),
                );
                door_done = true;
                continue;
            }

            let adj_out_n = !is_fp(grid, fp, u, v - 1);
            let adj_out_s = !is_fp(grid, fp, u, v + 1);
            let adj_out_w = !is_fp(grid, fp, u - 1, v);
            let adj_out_e = !is_fp(grid, fp, u + 1, v);
            let out_edges = [adj_out_n, adj_out_s, adj_out_w, adj_out_e]
                .into_iter()
                .filter(|&x| x)
                .count();
            if out_edges >= 1 && lower_roof_y + 2 < i32::from(height) {
                if out_edges >= 2 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        lower_roof_y + 1,
                        v,
                        make_frame_block_at(pal, "y", seed, u, lower_roof_y + 1, v),
                    );
                    let t_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, lower_roof_y + 2, v);
                    let lantern_blk = if is_lantern(&t_light) {
                        PlannedBlock::with_props(t_light, vec![("hanging", "false")])
                    } else {
                        PlannedBlock::simple(t_light)
                    };
                    set_uv(blocks, grid, u, lower_roof_y + 2, v, lantern_blk);
                } else {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        lower_roof_y + 1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, lower_roof_y + 1, v)),
                    );
                }
            } else if !shared_left
                && !shared_right
                && !is_fp(grid, roof_fp, u, v)
                && !is_solid_or_occupied(blocks, grid, u, lower_roof_y + 1, v)
                && !(lower_roof_y + 2..=lower_roof_y + 4)
                    .any(|cy| is_solid_or_occupied(blocks, grid, u, cy, v))
                && lower_roof_y + 2 < i32::from(height)
            {
                if planter_slots < 3 && (u + v) % 2 == 0 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        lower_roof_y + 1,
                        v,
                        PlannedBlock::simple("minecraft:moss_block"),
                    );
                    let pot = if planter_slots % 2 == 0 {
                        "minecraft:flowering_azalea"
                    } else {
                        "minecraft:azalea"
                    };
                    set_uv(
                        blocks,
                        grid,
                        u,
                        lower_roof_y + 2,
                        v,
                        PlannedBlock::simple(pot),
                    );
                    planter_slots += 1;
                } else if (u + v) % 4 == 1 {
                    set_uv(
                        blocks,
                        grid,
                        u,
                        lower_roof_y + 1,
                        v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, lower_roof_y + 1, v), grid.dir_front(), "bottom"),
                    );
                }
            }
        }
    }
}

fn resolve_wall_gradient(
    base_wall: &str,
    foundation: &str,
    accent: &str,
    weathering: f32,
    y_frac: f64,
    tex_n: f64,
) -> String {
    let w = f64::from(weathering);
    let bottom_bias = (1.0 - y_frac).clamp(0.0, 1.0);
    let top_bias = y_frac.clamp(0.0, 1.0);

    match base_wall {
        "minecraft:stone_bricks" => {
            if y_frac < 0.38 && tex_n < 0.42 + w * 0.28 * bottom_bias {
                "minecraft:mossy_stone_bricks".into()
            } else if tex_n > 0.72 - w * 0.36 {
                "minecraft:cracked_stone_bricks".into()
            } else if y_frac > 0.70 && tex_n < 0.36 + 0.14 * top_bias {
                "minecraft:andesite".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:cobblestone" => {
            if y_frac < 0.40 && tex_n < 0.42 + w * 0.28 * bottom_bias {
                "minecraft:mossy_cobblestone".into()
            } else if y_frac > 0.68 && tex_n < 0.38 {
                "minecraft:andesite".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:deepslate_bricks" | "minecraft:deepslate_tiles" => {
            if y_frac < 0.34 && tex_n < 0.42 + w * 0.24 {
                "minecraft:cobbled_deepslate".into()
            } else if tex_n > 0.72 - w * 0.34 {
                "minecraft:cracked_deepslate_bricks".into()
            } else if y_frac > 0.68 && tex_n < 0.40 {
                "minecraft:deepslate_tiles".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:bricks" => {
            if y_frac < 0.32 && tex_n < 0.40 + w * 0.20 {
                "minecraft:terracotta".into()
            } else if tex_n > 0.72 - w * 0.30 {
                "minecraft:granite".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:tuff_bricks" => {
            if y_frac < 0.32 && tex_n < 0.42 {
                "minecraft:polished_tuff".into()
            } else if tex_n > 0.72 - w * 0.28 {
                "minecraft:chiseled_tuff_bricks".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:smooth_sandstone" | "minecraft:sandstone" => {
            if y_frac < 0.28 && tex_n < 0.42 {
                "minecraft:cut_sandstone".into()
            } else if y_frac > 0.70 && tex_n > 0.64 - w * 0.22 {
                "minecraft:smooth_sandstone".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:white_terracotta" => {
            if y_frac < 0.28 && tex_n < 0.38 {
                foundation.to_owned()
            } else if tex_n < 0.38 + 0.14 * top_bias {
                "minecraft:calcite".into()
            } else {
                base_wall.to_owned()
            }
        }
        "minecraft:calcite" => {
            if tex_n < 0.36 + 0.12 * bottom_bias {
                "minecraft:diorite".into()
            } else if tex_n > 0.72 - w * 0.30 {
                accent.to_owned()
            } else {
                base_wall.to_owned()
            }
        }
        _ => {
            if tex_n > (0.74 - w * 0.38) && !base_wall.ends_with("_planks") {
                accent.to_owned()
            } else if tex_n > (0.78 - w * 0.28) {
                accent.to_owned()
            } else {
                base_wall.to_owned()
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_walls_and_buttresses(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    tall_fp_opt: Option<&[bool]>,
    bounds: [i32; 4],
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    noise: NoiseField,
    rng: &mut SplitMix64,
) -> Option<Vec<bool>> {
    let [b_u0, b_v0, _b_u1, _b_v1] = bounds;
    let mut upper_perimeter_mask: Option<Vec<bool>> = tall_fp_opt.map(|t| t.to_vec());
    let beam_every_floor = rng.next_f64() < 0.85;
    let total_wall_span = (i32::from(spec.floors) * i32::from(spec.floor_height)).max(1) as f64;
    let base_wall_y = i32::from(spec.foundation_height);

    for f in 0..i32::from(spec.floors) {
        let floor_y = base_wall_y + f * i32::from(spec.floor_height);
        let top_wall_y = floor_y + i32::from(spec.floor_height) - 1;
        let floor_primary_wall = if f == 0 {
            spec.ground_wall.as_deref().unwrap_or(pal.wall.as_str())
        } else {
            spec.upper_wall.as_deref().unwrap_or(pal.wall_upper.as_str())
        };

        let base_upper = tall_fp_opt.unwrap_or(fp);
        let use_fp = if f >= 1 && spec.overhang_jetty {
            if upper_perimeter_mask.as_deref() == tall_fp_opt || upper_perimeter_mask.is_none() {
                let mut ext = base_upper.to_vec();
                for v in 1..(grid.d - 1) {
                    for u in 1..(grid.w - 1) {
                        if !fp[grid.idx(u, v)]
                            && !base_upper[grid.idx(u, v)]
                            && (is_fp(grid, base_upper, u - 1, v)
                                || is_fp(grid, base_upper, u + 1, v)
                                || is_fp(grid, base_upper, u, v - 1)
                                || is_fp(grid, base_upper, u, v + 1))
                        {
                            ext[grid.idx(u, v)] = true;
                        }
                    }
                }
                upper_perimeter_mask = Some(ext);
            }
            upper_perimeter_mask.as_deref().unwrap_or(base_upper)
        } else if f >= 1 {
            base_upper
        } else {
            fp
        };

        for v in 0..grid.d {
            for u in 0..grid.w {
                if !use_fp[grid.idx(u, v)] {
                    continue;
                }
                let n_out_n = !is_fp(grid, use_fp, u, v - 1);
                let n_out_s = !is_fp(grid, use_fp, u, v + 1);
                let n_out_w = !is_fp(grid, use_fp, u - 1, v);
                let n_out_e = !is_fp(grid, use_fp, u + 1, v);
                let on_boundary = n_out_n || n_out_s || n_out_w || n_out_e;
                let is_corner = (n_out_n || n_out_s) && (n_out_w || n_out_e);

                let floor_blk = if f == 0
                    && matches!(
                        spec.category,
                        BuildCategory::Pavilion
                            | BuildCategory::Temple
                            | BuildCategory::Castle
                            | BuildCategory::Forge
                    ) {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, floor_y, v))
                } else if f >= 1 {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR_UPPER, seed, u, floor_y, v))
                } else {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, floor_y, v))
                };
                set_uv(blocks, grid, u, floor_y, v, floor_blk);

                if f >= 1 && spec.overhang_jetty && !fp[grid.idx(u, v)] {
                    let (du, dv) = if is_fp(grid, base_upper, u, v - 1) {
                        (0, -1)
                    } else if is_fp(grid, base_upper, u, v + 1) {
                        (0, 1)
                    } else if is_fp(grid, base_upper, u - 1, v) {
                        (-1, 0)
                    } else {
                        (1, 0)
                    };
                    set_uv(
                        blocks,
                        grid,
                        u,
                        floor_y - 1,
                        v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, floor_y - 1, v), grid.uv_to_mc_dir(du, dv), "top"),
                    );
                }

                if !on_boundary {
                    continue;
                }

                // Glazed Pavilion walls for Greenhouses vs open colonnades for regular Pavilions.
                if spec.category == BuildCategory::Pavilion {
                    let is_pillar = ((u - b_u0) % 3 == 0 && (v - b_v0) % 3 == 0) || is_corner;
                    for y in (floor_y + 1)..=top_wall_y {
                        if is_pillar {
                            set_uv(blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                        } else if y == floor_y + 1 {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                y,
                                v,
                                PlannedBlock::simple(if spec.theme == StructureTheme::Greenhouse {
                                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                                } else {
                                    pal.pick(MaterialPalette::SLOT_FENCE, seed, u, y, v)
                                }),
                            );
                        } else if y == top_wall_y {
                            let axis = if n_out_n || n_out_s {
                                grid.axis_u()
                            } else {
                                grid.axis_v()
                            };
                            set_uv(blocks, grid, u, y, v, make_beam_block_at(pal, axis, seed, u, y, v));
                        } else if spec.theme == StructureTheme::Greenhouse {
                            set_uv(blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)));
                        }
                    }
                    continue;
                }

                let is_pier = (n_out_n || n_out_s) && (u - b_u0) % 4 == 0
                    || (n_out_w || n_out_e) && (v - b_v0) % 4 == 0;
                for y in (floor_y + 1)..=top_wall_y {
                    if spec.theme == StructureTheme::Lighthouse {
                        let stripe = (y / 3) % 2 == 1;
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(if stripe {
                                pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                            } else {
                                pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                            }),
                        );
                    } else if is_corner
                        || (spec.exterior_depth && is_pier && spec.style != BuildStyle::Modern)
                    {
                        set_uv(blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                    } else if y == top_wall_y && (beam_every_floor || f < i32::from(spec.floors) - 1) {
                        let axis = if n_out_n || n_out_s {
                            grid.axis_u()
                        } else {
                            grid.axis_v()
                        };
                        set_uv(blocks, grid, u, y, v, make_beam_block_at(pal, axis, seed, u, y, v));
                    } else if f >= 1
                        && y == floor_y + 1
                        && spec.exterior_depth
                        && spec.style != BuildStyle::Modern
                        && spec.theme != StructureTheme::Greenhouse
                        && (pal.wall_trim != pal.wall_upper || pal.has_extra(MaterialPalette::SLOT_WALL_TRIM))
                    {
                        let axis = if n_out_n || n_out_s {
                            grid.axis_u()
                        } else {
                            grid.axis_v()
                        };
                        set_uv(blocks, grid, u, y, v, make_block_with_axis(&pal.pick(MaterialPalette::SLOT_WALL_TRIM, seed, u, y, v), axis));
                    } else if f == 0
                        && y == floor_y + 1
                        && !matches!(spec.style, BuildStyle::Modern)
                        && spec.theme != StructureTheme::Greenhouse
                    {
                        // Ground-floor masonry base wainscot beneath plastered/timber walls.
                        let tex_n = noise.fbm_3d(u as f64 * 0.34, y as f64 * 0.40, v as f64 * 0.34, 3);
                        let base_blk = if spec.ground_wall.is_some() {
                            resolve_wall_gradient(
                                floor_primary_wall,
                                &pal.foundation,
                                &pal.wall_accent,
                                spec.weathering,
                                0.05,
                                tex_n,
                            )
                        } else if spec.weathering >= 0.45
                            && !pal.has_extra(MaterialPalette::SLOT_FOUNDATION)
                            && !pal.has_extra(MaterialPalette::SLOT_WALL_ACCENT)
                            && matches!(pal.foundation.as_str(), "minecraft:stone_bricks" | "minecraft:cobblestone")
                        {
                            resolve_wall_gradient(
                                &pal.foundation,
                                &pal.foundation,
                                &pal.wall_accent,
                                spec.weathering,
                                0.05,
                                tex_n,
                            )
                        } else if noise.cell_hash(u, y, v) > 0.30 {
                            pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                        } else {
                            pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                        };
                        set_uv(blocks, grid, u, y, v, PlannedBlock::simple(base_blk));
                    } else if f == 0 && spec.ground_wall.is_none() && pal.has_extra(MaterialPalette::SLOT_WALL) {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)),
                        );
                    } else if f >= 1 && spec.upper_wall.is_none() && pal.has_extra(MaterialPalette::SLOT_WALL_UPPER) {
                        set_uv(
                            blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_UPPER, seed, u, y, v)),
                        );
                    } else {
                        let tex_n = noise.fbm_3d(u as f64 * 0.34, y as f64 * 0.40, v as f64 * 0.34, 3);
                        let y_frac = f64::from(y - base_wall_y) / total_wall_span;
                        let blk_id = resolve_wall_gradient(
                            floor_primary_wall,
                            &pal.foundation,
                            &pal.wall_accent,
                            spec.weathering,
                            y_frac,
                            tex_n,
                        );
                        set_uv(blocks, grid, u, y, v, PlannedBlock::simple(blk_id));
                    }
                }

                if spec.exterior_depth && (is_corner || is_pier) {
                    let out_dirs = [
                        (0, -1, n_out_n),
                        (0, 1, n_out_s),
                        (-1, 0, n_out_w),
                        (1, 0, n_out_e),
                    ];
                    for (du, dv, is_out) in out_dirs {
                        if !is_out || (!is_corner && spec.style == BuildStyle::Modern) {
                            continue;
                        }
                        let ou = u + du;
                        let ov = v + dv;
                        if !grid.in_bounds(ou, ov) || is_fp(grid, use_fp, ou, ov) {
                            continue;
                        }
                        if matches!(spec.style, BuildStyle::Gothic | BuildStyle::Medieval)
                            || matches!(spec.category, BuildCategory::Castle | BuildCategory::Temple)
                        {
                            let y_start = if f == 0 { 0 } else { floor_y };
                            for y in y_start..=floor_y {
                                set_uv(
                                    blocks,
                                    grid,
                                    ou,
                                    y,
                                    ov,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, ou, y, ov)),
                                );
                            }
                            for y in (floor_y + 1)..top_wall_y {
                                set_uv(
                                    blocks,
                                    grid,
                                    ou,
                                    y,
                                    ov,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, ou, y, ov)),
                                );
                            }
                            set_uv(
                                blocks,
                                grid,
                                ou,
                                top_wall_y,
                                ov,
                                make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou, top_wall_y, ov), grid.uv_to_mc_dir(-du, -dv), "top"),
                            );
                            // Stepped cathedral flying buttress piers on spacious side/rear walls.
                            if spec.rose_window && !is_corner && (du != 0 || dv < 0) {
                                let ou2 = u + du * 2;
                                let ov2 = v + dv * 2;
                                if grid.in_bounds(ou2, ov2) && !is_fp(grid, use_fp, ou2, ov2) {
                                    let y2_start = if f == 0 { 0 } else { floor_y };
                                    for y in y2_start..=(floor_y + 1) {
                                        set_uv(
                                            blocks,
                                            grid,
                                            ou2,
                                            y,
                                            ov2,
                                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, ou2, y, ov2)),
                                        );
                                    }
                                    set_uv(
                                        blocks,
                                        grid,
                                        ou2,
                                        floor_y + 2,
                                        ov2,
                                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, ou2, floor_y + 2, ov2)),
                                    );
                                    if top_wall_y > floor_y + 2 {
                                        let buttress_face = grid.uv_to_mc_dir(du, dv);
                                        set_uv(
                                            blocks,
                                            grid,
                                            ou,
                                            top_wall_y,
                                            ov,
                                            make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou, top_wall_y, ov), buttress_face, "bottom"),
                                        );
                                        set_uv(
                                            blocks,
                                            grid,
                                            ou2,
                                            floor_y + 3,
                                            ov2,
                                            make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, ou2, floor_y + 3, ov2), buttress_face, "bottom"),
                                        );
                                    }
                                }
                            }
                        } else if spec.style == BuildStyle::Steampunk && is_corner {
                            let y_start = if f == 0 { 0 } else { floor_y };
                            for y in y_start..=floor_y {
                                set_uv(
                                    blocks,
                                    grid,
                                    ou,
                                    y,
                                    ov,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, ou, y, ov)),
                                );
                            }
                            for y in (floor_y + 1)..top_wall_y {
                                set_uv(blocks, grid, ou, y, ov, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, ou, y, ov)));
                            }
                            set_uv(blocks, grid, ou, top_wall_y, ov, PlannedBlock::simple("minecraft:copper_bulb"));
                        }
                    }
                }
            }
        }

        // Japanese Pagoda multi-tier upturned eave skirts between tower stories!
        if spec.style == BuildStyle::Japanese && spec.category == BuildCategory::Tower && f >= 1 {
            for v in 0..grid.d {
                for u in 0..grid.w {
                    if use_fp[grid.idx(u, v)] {
                        continue;
                    }
                    let near_o = is_fp(grid, use_fp, u - 1, v)
                        || is_fp(grid, use_fp, u + 1, v)
                        || is_fp(grid, use_fp, u, v - 1)
                        || is_fp(grid, use_fp, u, v + 1);
                    let near_d = is_fp(grid, use_fp, u - 1, v - 1)
                        || is_fp(grid, use_fp, u + 1, v - 1)
                        || is_fp(grid, use_fp, u - 1, v + 1)
                        || is_fp(grid, use_fp, u + 1, v + 1);
                    if near_o {
                        set_uv(blocks, grid, u, floor_y, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, floor_y, v), "top"));
                    } else if near_d {
                        set_uv(blocks, grid, u, floor_y + 1, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, floor_y + 1, v), "bottom"));
                    }
                }
            }
        }
    }

    upper_perimeter_mask
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn build_corner_turret(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    roof_fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    stairwell_uv: &mut Vec<(i32, i32)>,
    stair_headroom: &mut Vec<(i32, i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    max_placed_roof_y: i32,
    height: u16,
    adapt_terrain: bool,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
    rng: &mut SplitMix64,
) {
    if !spec.corner_turret {
        return;
    }
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let turret_candidates = [
        (b_u0 + 1, b_v0 + 1),
        (b_u1 - 1, b_v0 + 1),
        (b_u0 + 1, b_v1 - 1),
        (b_u1 - 1, b_v1 - 1),
    ];

    let turret_top = (max_placed_roof_y + rng.range_i32(2, 4)).min(i32::from(height) - 3);
    let spire = spec.category != BuildCategory::Castle && rng.next_f64() < 0.70;
    let use_oct_radius2 = (grid.w >= 17
        && grid.d >= 15
        && (b_u1 - b_u0) >= 10
        && (b_v1 - b_v0) >= 8
        && matches!(spec.category, BuildCategory::Castle | BuildCategory::Temple))
        || (matches!(spec.style, BuildStyle::Gothic | BuildStyle::Medieval)
            && grid.w >= 19
            && grid.d >= 17);
    let rad: i32 = if use_oct_radius2 { 2 } else { 1 };

    let chosen: Vec<(i32, i32)> = if spec.category == BuildCategory::Castle {
        turret_candidates
            .into_iter()
            .filter(|&(tu, tv)| {
                is_fp(grid, fp, tu, tv)
                    && best_door.is_none_or(|(du, dv)| (tu - du).abs() + (tv - dv).abs() > rad + 3)
                    && !stairwell_uv
                        .iter()
                        .any(|&(su, sv)| (su - tu).abs() <= rad + 1 && (sv - tv).abs() <= rad + 1)
            })
            .collect()
    } else {
        let start_idx = rng.range_i32(0, 3) as usize;
        (0..4)
            .map(|k| turret_candidates[(start_idx + k) % 4])
            .find(|&(tu, tv)| {
                is_fp(grid, fp, tu, tv)
                    && best_door.is_none_or(|(du, dv)| (tu - du).abs() + (tv - dv).abs() > rad + 3)
                    && !stairwell_uv
                        .iter()
                        .any(|&(su, sv)| (su - tu).abs() <= rad + 1 && (sv - tv).abs() <= rad + 1)
            })
            .into_iter()
            .collect()
    };
    if chosen.is_empty() {
        return;
    }

    let center_u = (b_u0 + b_u1) / 2;
    let center_v = (b_v0 + b_v1) / 2;
    let floor_base = i32::from(spec.foundation_height);
    let top_floor_y = floor_base + (i32::from(spec.floors) - 1) * i32::from(spec.floor_height);

    for (tu_center, tv_center) in chosen {
        let step_u = if center_u >= tu_center { 1 } else { -1 };
        let step_v = if center_v >= tv_center { 1 } else { -1 };
        let lad_u = tu_center - step_u * (rad - 1).max(0);
        let lad_v = tv_center;
        let wall_u = tu_center - step_u * rad;

        for du in -rad..=rad {
            for dv in -rad..=rad {
                if rad == 2 && du.abs() == 2 && dv.abs() == 2 {
                    continue;
                }
                let u = tu_center + du;
                let v = tv_center + dv;
                if !grid.in_bounds(u, v) {
                    continue;
                }
                let is_rim = if rad == 2 {
                    du.abs() == 2 || dv.abs() == 2 || (du.abs() == 1 && dv.abs() == 1)
                } else {
                    du.abs() == 1 || dv.abs() == 1
                };
                let is_corner_post = if rad == 2 {
                    (du.abs() == 2 && dv.abs() == 1) || (du.abs() == 1 && dv.abs() == 2)
                } else {
                    du.abs() == 1 && dv.abs() == 1
                };
                // Ground corner turret solidly into terrain/water even where its 3x3 base projects outside `fp`.
                let terr = terrain_rel_at(terrain_offsets, grid, u, v);
                let wd = crate::gen_common::water_depth_at(water_depths, grid, u, v);
                let seabed = terr - wd;
                if spec.on_stilts {
                    if !is_fp(grid, fp, u, v) && is_corner_post {
                        let post_bot = (seabed - 1).clamp(-14, floor_base - 1);
                        for y in post_bot..floor_base {
                            if !is_solid_or_occupied(blocks, grid, u, y, v) {
                                let blk = if y <= seabed + 1 {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v))
                                } else {
                                    make_frame_block_at(pal, "y", seed, u, y, v)
                                };
                                set_uv(blocks, grid, u, y, v, blk);
                            }
                        }
                    }
                } else {
                    let skirt_bot = if adapt_terrain { seabed.clamp(-16, 0) } else { 0 };
                    for y in skirt_bot..floor_base {
                        if (is_rim || !is_fp(grid, fp, u, v) || !spec.basement || y >= 0)
                            && !is_solid_or_occupied(blocks, grid, u, y, v)
                        {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                y,
                                v,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)),
                            );
                        }
                    }
                }
                for y in floor_base..=turret_top {
                    if y == floor_base {
                        set_uv(blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                    } else if is_rim {
                        let blk = if is_corner_post {
                            make_frame_block_at(pal, "y", seed, u, y, v)
                        } else if y == turret_top - 1 && spec.exterior_depth {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                        };
                        set_uv(blocks, grid, u, y, v, blk);
                    } else {
                        stairwell_uv.push((u, v));
                        set_uv(blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
                if spire {
                    let ring_dist = du.abs().max(dv.abs());
                    let step_off = rad + 1 - ring_dist;
                    let roof_y = (turret_top + step_off).min(i32::from(height) - 1);
                    if ring_dist == rad && roof_y > turret_top {
                        set_uv(blocks, grid, u, roof_y, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, roof_y, v), "bottom"));
                    } else if ring_dist > 0 {
                        set_uv(blocks, grid, u, roof_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, roof_y, v)));
                    } else {
                        set_uv(blocks, grid, u, roof_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, roof_y, v)));
                        if roof_y + 1 < i32::from(height) {
                            set_uv(blocks, grid, u, roof_y + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, roof_y + 1, v)));
                        }
                    }
                } else {
                    // Open walkable battlement watch deck at `turret_top`!
                    if !is_rim && !(u == lad_u && v == lad_v) {
                        set_uv(blocks, grid, u, turret_top, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, turret_top, v)));
                        for cy in (turret_top + 1)..=(turret_top + 2).min(i32::from(height) - 1) {
                            set_uv(blocks, grid, u, cy, v, PlannedBlock::simple("minecraft:air"));
                        }
                    }
                    let exit_rim_u = tu_center + step_u * rad;
                    if is_rim && !(u == exit_rim_u && v == tv_center) {
                        if (du + dv).abs() % 2 == 1 {
                            set_uv(
                                blocks,
                                grid,
                                u,
                                turret_top + 1,
                                v,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, turret_top + 1, v)),
                            );
                        } else if is_corner_post && turret_top + 1 < i32::from(height) {
                            set_uv(blocks, grid, u, turret_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, turret_top + 1, v)));
                        }
                    } else if is_rim && u == exit_rim_u && v == tv_center {
                        set_uv(blocks, grid, u, turret_top, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, turret_top, v)));
                        for cy in (turret_top + 1)..=(turret_top + 2).min(i32::from(height) - 1) {
                            set_uv(blocks, grid, u, cy, v, PlannedBlock::simple("minecraft:air"));
                        }
                    }
                }
            }
        }

        // Carve walkable arched entries connecting house stories into the hollow turret and keep approaches clear of furniture.
        let entry_uv = [
            (tu_center + step_u * rad, tv_center, step_u, 0),
            (tu_center, tv_center + step_v * rad, 0, step_v),
        ]
        .into_iter()
        .find(|&(eu, ev, _, _)| grid.in_bounds(eu, ev) && is_fp(grid, fp, eu + step_u, ev + step_v))
        .unwrap_or((tu_center + step_u * rad, tv_center, step_u, 0));

        let (eu, ev, ed_u, ed_v) = entry_uv;
        stairwell_uv.push((eu, ev));
        if grid.in_bounds(eu + ed_u, ev + ed_v) {
            stairwell_uv.push((eu + ed_u, ev + ed_v));
        }
        stairwell_uv.push((lad_u, lad_v));

        for f in 0..i32::from(spec.floors) {
            let fy = floor_base + f * i32::from(spec.floor_height);
            if fy >= turret_top {
                break;
            }
            let floor_exists = if f == 0 {
                true
            } else {
                grid.in_bounds(eu + ed_u, ev + ed_v) && is_fp(grid, roof_fp, eu + ed_u, ev + ed_v)
            };
            let near_entrance = f == 0
                && best_door.is_some_and(|(du, dv)| (eu - du).abs() <= 2 && (ev - dv).abs() <= 1);
            let has_door = (fy..=(fy + 2)).any(|dy| {
                crate::gen_common::get_uv(blocks, grid, eu, dy, ev)
                    .is_some_and(|b| b.block_id.ends_with("_door"))
            });
            if floor_exists && grid.in_bounds(eu, ev) && !near_entrance && !has_door {
                set_uv(blocks, grid, eu, fy, ev, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, eu, fy, ev)));
                set_uv(blocks, grid, eu, fy + 1, ev, PlannedBlock::simple("minecraft:air"));
                set_uv(blocks, grid, eu, fy + 2, ev, PlannedBlock::simple("minecraft:air"));
                stair_headroom.push((eu, fy, ev));
                if f > 0 && rad == 2 {
                    for du in -1..=1 {
                        for dv in -1..=1 {
                            let pu = tu_center + du;
                            let pv = tv_center + dv;
                            if !(pu == lad_u && pv == lad_v) && !(du.abs() == 1 && dv.abs() == 1) {
                                set_uv(blocks, grid, pu, fy, pv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, pu, fy, pv)));
                            }
                        }
                    }
                }
            }
        }

        let ladder_top_opt = if !spire {
            Some(turret_top)
        } else if spec.floors >= 2 && top_floor_y < turret_top && is_fp(grid, roof_fp, eu, ev) {
            Some(top_floor_y)
        } else {
            None
        };
        if let Some(lad_top) = ladder_top_opt
            && grid.in_bounds(lad_u, lad_v)
            && grid.in_bounds(wall_u, lad_v)
        {
            let lad_face = grid.uv_to_mc_dir(step_u, 0);
            for ly in (floor_base + 1)..=lad_top {
                set_uv(blocks, grid, wall_u, ly, lad_v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, wall_u, ly, lad_v)));
                set_uv(
                    blocks,
                    grid,
                    lad_u,
                    ly,
                    lad_v,
                    PlannedBlock::with_props("minecraft:ladder", vec![("facing", lad_face)]),
                );
            }
            for cy in (lad_top + 1)..=(lad_top + 2).min(i32::from(height) - 1) {
                set_uv(blocks, grid, lad_u, cy, lad_v, PlannedBlock::simple("minecraft:air"));
            }
            stair_headroom.push((lad_u, lad_top, lad_v));
        } else {
            let turret_light = pal.pick(MaterialPalette::SLOT_LIGHT, seed, tu_center, turret_top - 1, tv_center);
            if is_lantern(&turret_light) && grid.in_bounds(tu_center, tv_center) && turret_top > floor_base + 3 {
                set_uv(
                    blocks,
                    grid,
                    tu_center,
                    turret_top - 1,
                    tv_center,
                    PlannedBlock::with_props(turret_light, vec![("hanging", "true")]),
                );
            }
        }

        let spire_top = (turret_top + rad + 2).min(i32::from(height) - 1);
        if spire && grid.in_bounds(tu_center, tv_center) && spire_top < i32::from(height) {
            set_uv(
                blocks,
                grid,
                tu_center,
                spire_top,
                tv_center,
                PlannedBlock::with_props("minecraft:lightning_rod", vec![("facing", "up")]),
            );
        }
    }
}

/// Builds a 2-block-deep covered arcade/colonnade gallery with stair arches along an open side wall.
#[allow(clippy::too_many_arguments)]
fn build_arcade_gallery(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    bounds: [i32; 4],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    adapt_terrain: bool,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) {
    if !spec.arcade_gallery || spec.ruin_erosion >= 0.1 {
        return;
    }
    let [b_u0, b_v0, b_u1, b_v1] = bounds;
    let ground_y = i32::from(spec.foundation_height);
    let top_y = ground_y + i32::from(spec.floor_height) - 1;
    if top_y <= ground_y + 2 || b_v1 - b_v0 < 7 {
        return;
    }

    for &(wall_u, dir_u, arch_face) in &[
        (b_u0, -1, grid.dir_right()),
        (b_u1, 1, grid.dir_left()),
    ] {
        let u1 = wall_u + dir_u;
        let u2 = wall_u + dir_u * 2;
        if !grid.in_bounds(u2, b_v0 + 1) {
            continue;
        }
        let v_start = b_v0 + 2;
        let v_end = (b_v1 - 2).min(v_start + 7);
        if v_end - v_start < 4 {
            continue;
        }
        let clear_side = (v_start..=v_end).all(|v| {
            is_fp(grid, fp, wall_u, v)
                && !is_fp(grid, fp, u1, v)
                && !is_fp(grid, fp, u2, v)
                && best_door.is_none_or(|(du, dv)| (u2 - du).abs() > 2 || (v - dv).abs() > 2)
                && (ground_y..=top_y + 1).all(|y| {
                    !crate::gen_common::is_solid_or_occupied(blocks, grid, u1, y, v)
                        && !crate::gen_common::is_solid_or_occupied(blocks, grid, u2, y, v)
                })
        });
        if !clear_side {
            continue;
        }

        let mid_v = (v_start + v_end) / 2;
        let ridge_out = grid.uv_to_mc_dir(dir_u, 0);
        let _ = arch_face;
        for v in v_start..=v_end {
            if !spec.on_stilts {
                for col_u in [u1, u2] {
                    let terr = terrain_rel_at(terrain_offsets, grid, col_u, v);
                    let wd = crate::gen_common::water_depth_at(water_depths, grid, col_u, v);
                    let skirt_bot = if adapt_terrain {
                        (terr - wd).clamp(-16, 0)
                    } else {
                        0
                    };
                    for fy in skirt_bot..ground_y {
                        if !is_solid_or_occupied(blocks, grid, col_u, fy, v) {
                            set_uv(
                                blocks,
                                grid,
                                col_u,
                                fy,
                                v,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, col_u, fy, v)),
                            );
                        }
                    }
                }
            }
            set_uv(blocks, grid, u1, ground_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u1, ground_y, v)));
            set_uv(blocks, grid, u2, ground_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u2, ground_y, v)));
            set_uv(blocks, grid, u1, top_y + 1, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_CEILING, seed, u1, top_y + 1, v), "top"));
            set_uv(blocks, grid, u2, top_y + 1, v, make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u2, top_y + 1, v), ridge_out, "bottom"));

            let is_pillar = (v - v_start) % 3 == 0 || v == v_end;
            if is_pillar {
                set_uv(blocks, grid, u2, ground_y + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u2, ground_y + 1, v)));
                for y in (ground_y + 2)..=top_y {
                    set_uv(blocks, grid, u2, y, v, make_pillar_block_at(pal, "y", seed, u2, y, v));
                }
            } else {
                set_uv(blocks, grid, u2, top_y, v, make_beam_block_at(pal, grid.axis_v(), seed, u2, top_y, v));
                let arch_dir = if (v - v_start) % 3 == 1 {
                    grid.dir_front()
                } else {
                    grid.dir_back()
                };
                if top_y - 1 > ground_y + 1 {
                    set_uv(
                        blocks,
                        grid,
                        u2,
                        top_y - 1,
                        v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u2, top_y - 1, v), arch_dir, "top"),
                    );
                }
            }
            let near_door = best_door.is_some_and(|(du, dv)| (wall_u - du).abs() <= 2 && (v - dv).abs() <= 2)
                || crate::gen_common::get_uv(blocks, grid, wall_u, ground_y + 1, v)
                    .is_some_and(|b| b.block_id.ends_with("_door"));
            if v == mid_v && !near_door {
                set_uv(blocks, grid, wall_u, ground_y + 1, v, PlannedBlock::simple("minecraft:air"));
                set_uv(blocks, grid, wall_u, ground_y + 2, v, PlannedBlock::simple("minecraft:air"));
            }
        }
        break;
    }
}

/// Maps biome hints and architectural style to natural `(top, alt_top, sub_fill)` terrain blocks.
pub(crate) fn biome_surface_blocks(
    biome_tag: &str,
    style: BuildStyle,
) -> (&'static str, &'static str, &'static str) {
    let b = biome_tag.to_ascii_lowercase();
    if b.contains("badlands") {
        ("minecraft:red_sand", "minecraft:red_sandstone", "minecraft:terracotta")
    } else if b.contains("desert") || (b.is_empty() && style == BuildStyle::Desert) {
        ("minecraft:sand", "minecraft:sandstone", "minecraft:sandstone")
    } else if b.contains("swamp") || b.contains("mangrove") {
        ("minecraft:grass_block", "minecraft:mud", "minecraft:mud")
    } else if b.contains("snow")
        || b.contains("ice")
        || b.contains("frozen")
        || b.contains("grove")
        || b.contains("jagged")
    {
        ("minecraft:snow_block", "minecraft:grass_block", "minecraft:dirt")
    } else if b.contains("taiga") {
        ("minecraft:podzol", "minecraft:grass_block", "minecraft:dirt")
    } else if b.contains("savanna") {
        ("minecraft:grass_block", "minecraft:coarse_dirt", "minecraft:dirt")
    } else if b.contains("mushroom") {
        ("minecraft:mycelium", "minecraft:moss_block", "minecraft:dirt")
    } else if b.contains("stony") {
        ("minecraft:stone", "minecraft:gravel", "minecraft:stone")
    } else if b.contains("beach") {
        ("minecraft:sand", "minecraft:sandstone", "minecraft:sand")
    } else {
        ("minecraft:grass_block", "minecraft:moss_block", "minecraft:dirt")
    }
}

/// Applies radial smoothstep terrain blending for a single local column `[lx, lz]`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_radial_terrain_column(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    lx: i32,
    lz: i32,
    dist: f64,
    radius: i32,
    terr: i32,
    base_y: i32,
    height_lim: i32,
    biome_tag: &str,
    style: BuildStyle,
    pal: &MaterialPalette,
    seed: u64,
    noise_hash: f64,
) {
    if radius <= 0 || dist > f64::from(radius) + 0.35 {
        return;
    }
    let delta = terr - base_y;
    if (-1..=1).contains(&delta) {
        return;
    }

    let t = (dist / f64::from(radius)).clamp(0.0, 1.0);
    let smooth_w = t * t * (3.0 - 2.0 * t);
    let jitter = (noise_hash - 0.5) * 0.45 * (1.0 - (2.0 * t - 1.0).abs());
    let (top_blk, alt_blk, sub_blk) = biome_surface_blocks(biome_tag, style);
    let chosen_top = if noise_hash > 0.72 { alt_blk } else { top_blk };

    if delta < -1 {
        let target_top = ((base_y as f64) + (delta as f64) * smooth_w + jitter)
            .round()
            .clamp((terr + 1) as f64, (base_y - 1) as f64) as i32;
        let lo_fill = (terr + 1).max(-16);
        for y in lo_fill..target_top {
            if blocks.get(&[lx, y, lz]).is_none_or(|b| b.block_id == "minecraft:air") {
                let fill_blk = if dist <= 1.55 && delta <= -4 && y >= target_top - 1 {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, y, lz)
                } else {
                    sub_blk.to_owned()
                };
                blocks.insert([lx, y, lz], PlannedBlock::simple(fill_blk));
            }
        }
        if target_top > terr
            && blocks
                .get(&[lx, target_top, lz])
                .is_none_or(|b| b.block_id == "minecraft:air")
        {
            blocks.insert([lx, target_top, lz], PlannedBlock::simple(chosen_top));
        }
    } else if delta >= 2 {
        let target_top = ((base_y as f64) + (delta as f64) * smooth_w + jitter)
            .round()
            .clamp(base_y as f64, (terr - 1) as f64) as i32;
        for y in (target_top + 1)..=(terr + 2).min(height_lim) {
            if blocks
                .get(&[lx, y, lz])
                .is_none_or(|b| b.block_id == "minecraft:air")
            {
                blocks.insert([lx, y, lz], PlannedBlock::simple("minecraft:air"));
            }
        }
        if target_top < terr
            && blocks
                .get(&[lx, target_top, lz])
                .is_none_or(|b| b.block_id == "minecraft:air")
        {
            if target_top > base_y && dist <= 1.55 && delta >= 4 {
                blocks.insert(
                    [lx, target_top - 1, lz],
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, target_top - 1, lz)),
                );
            }
            blocks.insert([lx, target_top, lz], PlannedBlock::simple(chosen_top));
        }
    }
}

/// Smooths jagged hillside cuts and dropoffs around the footprint using Euclidean radial smoothstep & biome terrain.
#[allow(clippy::too_many_arguments)]
pub(crate) fn blend_surrounding_terrain(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    fp: &[bool],
    best_door: Option<(i32, i32)>,
    spec: &BlueprintSpec,
    pal: &MaterialPalette,
    seed: u64,
    height: u16,
    terrain_offsets: &[i32],
    water_depths: Option<&[i32]>,
    noise: NoiseField,
    blend_radius: u8,
) {
    if spec.on_stilts || blend_radius == 0 {
        return;
    }
    let radius = i32::from(blend_radius.clamp(1, 10));
    let height_lim = i32::from(height) + 2;
    for v in 0..grid.d {
        for u in 0..grid.w {
            if is_fp(grid, fp, u, v) || crate::gen_common::water_depth_at(water_depths, grid, u, v) > 0 {
                continue;
            }
            if let Some((du, dv)) = best_door
                && (u - du).abs() <= 1
                && v >= dv
                && v <= dv + 2
            {
                continue;
            }
            if (0..i32::from(height)).any(|y| is_solid_or_occupied(blocks, grid, u, y, v)) {
                continue;
            }

            let mut min_d2 = i32::MAX;
            for dv in -radius..=radius {
                for du in -radius..=radius {
                    if is_fp(grid, fp, u + du, v + dv) {
                        min_d2 = min_d2.min(du * du + dv * dv);
                    }
                }
            }
            if min_d2 == i32::MAX {
                continue;
            }
            let dist = (min_d2 as f64).sqrt();
            if dist > f64::from(radius) + 0.35 {
                continue;
            }
            let terr = terrain_rel_at(Some(terrain_offsets), grid, u, v);
            let [lx, lz] = grid.to_local_xz(u, v);
            let h = noise.cell_hash(u, 0, v);
            apply_radial_terrain_column(
                blocks,
                lx,
                lz,
                dist,
                radius,
                terr,
                0,
                height_lim,
                &spec.biome_tag,
                spec.style,
                pal,
                seed,
                h,
            );
        }
    }
}

/// Applies radial Euclidean terrain blending around landmarks/monuments that sit inside a bounding box without a courtyard apron.
pub(crate) fn smooth_landmark_edges(
    bp: &mut GeneratedBlueprint,
    req: &BuildRequest,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
    noise: NoiseField,
) {
    if !req.adapt_terrain || bp.spec.on_stilts || req.blend_radius == 0 {
        return;
    }
    let Some(offsets) = terrain_offsets else {
        return;
    };
    let w = i32::from(bp.width);
    let d = i32::from(bp.depth);
    let h = i32::from(bp.height);
    let radius = i32::from(req.blend_radius.clamp(1, 10));

    let mut solid_cols: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for (&[lx, ly, lz], blk) in &bp.blocks {
        if blk.block_id != "minecraft:air" && ly >= -6 && ly <= 6 {
            solid_cols
                .entry((lx, lz))
                .and_modify(|min_y| *min_y = (*min_y).min(ly))
                .or_insert(ly);
        }
    }

    for lz in 0..d {
        for lx in 0..w {
            if solid_cols.contains_key(&(lx, lz)) {
                continue;
            }
            let idx = (lz * w + lx) as usize;
            if water_depths.and_then(|wd| wd.get(idx).copied()).unwrap_or(0) > 0 {
                continue;
            }
            let mut best: Option<(i32, i32)> = None; // (d2, anchor_y)
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if let Some(&anchor_y) = solid_cols.get(&(lx + dx, lz + dz)) {
                        let d2 = dx * dx + dz * dz;
                        if best.is_none_or(|(bd2, _)| d2 < bd2) {
                            best = Some((d2, anchor_y.clamp(-4, 3)));
                        }
                    }
                }
            }
            let Some((d2, anchor_y)) = best else {
                continue;
            };
            let dist = (d2 as f64).sqrt();
            if dist > f64::from(radius) + 0.35 {
                continue;
            }
            let terr = offsets.get(idx).copied().unwrap_or(0).clamp(-14, 14);
            apply_radial_terrain_column(
                &mut bp.blocks,
                lx,
                lz,
                dist,
                radius,
                terr,
                anchor_y,
                h + 2,
                &bp.spec.biome_tag,
                bp.spec.style,
                &req.palette,
                req.seed as u64,
                noise.cell_hash(lx, 0, lz),
            );
        }
    }
}

/// Removes blocks whose structural support or attachment face is air/missing (prevents floating debris).
pub(crate) fn prune_unsupported_floating_blocks(blocks: &mut BTreeMap<[i32; 3], PlannedBlock>) {
    fn dir_offset(dir: &str) -> [i32; 3] {
        match dir {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            _ => [-1, 0, 0],
        }
    }

    fn prop(b: &PlannedBlock, k: &str) -> Option<&'static str> {
        b.props.iter().find(|&&(pk, _)| pk == k).map(|&(_, pv)| pv)
    }

    fn is_non_air(blocks: &BTreeMap<[i32; 3], PlannedBlock>, p: [i32; 3]) -> bool {
        match blocks.get(&p) {
            Some(b) => b.block_id != "minecraft:air",
            None => p[1] <= 0,
        }
    }

    fn is_solid_support(blocks: &BTreeMap<[i32; 3], PlannedBlock>, p: [i32; 3]) -> bool {
        let Some(b) = blocks.get(&p) else {
            return p[1] <= 0;
        };
        let id = b.block_id.as_str();
        !matches!(
            id,
            "minecraft:air"
                | "minecraft:water"
                | "minecraft:lava"
                | "minecraft:torch"
                | "minecraft:wall_torch"
                | "minecraft:soul_wall_torch"
                | "minecraft:vine"
                | "minecraft:fern"
                | "minecraft:short_grass"
                | "minecraft:pink_petals"
                | "minecraft:wildflowers"
                | "minecraft:leaf_litter"
                | "minecraft:candle"
                | "minecraft:flower_pot"
                | "minecraft:cake"
                | "minecraft:sea_pickle"
                | "minecraft:lily_pad"
                | "minecraft:sugar_cane"
                | "minecraft:wheat"
                | "minecraft:azalea"
                | "minecraft:flowering_azalea"
                | "minecraft:lever"
                | "minecraft:tripwire_hook"
                | "minecraft:daylight_detector"
        ) && !id.ends_with("_carpet")
            && !id.ends_with("_banner")
            && !id.ends_with("_wall_banner")
            && !id.ends_with("_sign")
            && !id.ends_with("_wall_sign")
            && !id.ends_with("_pressure_plate")
            && !id.starts_with("minecraft:potted_")
    }

    for _pass in 0..3 {
        let positions: Vec<[i32; 3]> = blocks.keys().copied().collect();
        let mut to_remove = Vec::new();
        for pos in positions {
            let Some(b) = blocks.get(&pos) else {
                continue;
            };
            let id = b.block_id.as_str();
            if id == "minecraft:air" {
                continue;
            }
            let [x, y, z] = pos;
            let above = [x, y + 1, z];
            let below = [x, y - 1, z];

            let needs_above = matches!(
                id,
                "minecraft:iron_chain" | "minecraft:spore_blossom" | "minecraft:hanging_roots"
            ) || (is_lantern(id) && prop(b, "hanging") == Some("true"))
                || (id == "minecraft:bell" && prop(b, "attachment") == Some("ceiling"))
                || (id.ends_with("_hanging_sign") && prop(b, "attached") == Some("true"));
            if needs_above && !is_non_air(blocks, above) {
                to_remove.push(pos);
                continue;
            }

            if matches!(id, "minecraft:wall_torch" | "minecraft:soul_wall_torch" | "minecraft:tripwire_hook")
                || id.ends_with("_shelf")
                || id.ends_with("_wall_banner")
                || id.ends_with("_wall_sign")
                || id == "minecraft:ladder"
                || (id == "minecraft:lever" && prop(b, "face") == Some("wall"))
            {
                let d = dir_offset(prop(b, "facing").unwrap_or("north"));
                let attach = [x - d[0], y, z - d[2]];
                let back_ok = is_solid_support(blocks, attach)
                    && (id != "minecraft:ladder"
                        || blocks.get(&attach).is_some_and(|ab| {
                            let aid = ab.block_id.as_str();
                            !aid.contains("glass")
                                && !aid.ends_with("_door")
                                && !aid.ends_with("_stairs")
                                && !aid.ends_with("_slab")
                                && !aid.ends_with("_fence")
                                && aid != "minecraft:iron_bars"
                        }));
                if !back_ok {
                    to_remove.push(pos);
                    continue;
                }
            }

            if id == "minecraft:vine" {
                let attached = is_solid_support(blocks, above)
                    || is_solid_support(blocks, [x, y, z - 1])
                    || is_solid_support(blocks, [x, y, z + 1])
                    || is_solid_support(blocks, [x - 1, y, z])
                    || is_solid_support(blocks, [x + 1, y, z]);
                if !attached {
                    to_remove.push(pos);
                    continue;
                }
            }

            if y > 0 {
                if id == "minecraft:lily_pad" {
                    let on_water = blocks
                        .get(&below)
                        .is_some_and(|sb| sb.block_id == "minecraft:water");
                    if !on_water {
                        to_remove.push(pos);
                        continue;
                    }
                }
                let needs_floor = matches!(
                    id,
                    "minecraft:torch"
                        | "minecraft:campfire"
                        | "minecraft:soul_campfire"
                        | "minecraft:candle"
                        | "minecraft:cake"
                        | "minecraft:sea_pickle"
                        | "minecraft:flower_pot"
                        | "minecraft:bamboo"
                        | "minecraft:wheat"
                        | "minecraft:azalea"
                        | "minecraft:flowering_azalea"
                        | "minecraft:fern"
                        | "minecraft:short_grass"
                        | "minecraft:pink_petals"
                        | "minecraft:wildflowers"
                        | "minecraft:leaf_litter"
                        | "minecraft:sugar_cane"
                        | "minecraft:brewing_stand"
                        | "minecraft:lectern"
                        | "minecraft:anvil"
                        | "minecraft:enchanting_table"
                        | "minecraft:jukebox"
                        | "minecraft:ender_chest"
                        | "minecraft:crafting_table"
                        | "minecraft:furnace"
                        | "minecraft:smoker"
                        | "minecraft:blast_furnace"
                        | "minecraft:stonecutter"
                        | "minecraft:cartography_table"
                        | "minecraft:loom"
                        | "minecraft:smithing_table"
                        | "minecraft:composter"
                        | "minecraft:cauldron"
                        | "minecraft:water_cauldron"
                        | "minecraft:lava_cauldron"
                        | "minecraft:beacon"
                        | "minecraft:chest"
                        | "minecraft:barrel"
                        | "minecraft:daylight_detector"
                ) || id.starts_with("minecraft:potted_")
                    || id.ends_with("_carpet")
                    || id.ends_with("_pressure_plate")
                    || id.ends_with("_bed")
                    || id.ends_with("_fence_gate")
                    || (!id.ends_with("_wall_banner") && id.ends_with("_banner"))
                    || (!id.ends_with("_wall_sign")
                        && !id.ends_with("_hanging_sign")
                        && id.ends_with("_sign"))
                    || (id == "minecraft:lever" && prop(b, "face") == Some("floor"))
                    || (id == "minecraft:grindstone" && prop(b, "face") == Some("floor"))
                    || (is_lantern(id) && prop(b, "hanging") != Some("true"))
                    || (id.ends_with("_door") && prop(b, "half") == Some("lower"));
                if needs_floor && !is_solid_support(blocks, below) {
                    to_remove.push(pos);
                    continue;
                }
                if id.ends_with("_door")
                    && prop(b, "half") == Some("upper")
                    && blocks.get(&below).is_none_or(|sb| !sb.block_id.ends_with("_door"))
                {
                    to_remove.push(pos);
                    continue;
                }
                if id.ends_with("_wall") {
                    let supported = is_solid_support(blocks, below)
                        || (is_non_air(blocks, [x - 1, y, z]) && is_non_air(blocks, [x + 1, y, z]))
                        || (is_non_air(blocks, [x, y, z - 1]) && is_non_air(blocks, [x, y, z + 1]));
                    if !supported {
                        to_remove.push(pos);
                        continue;
                    }
                }
                if id == "minecraft:lightning_rod" {
                    let facing = prop(b, "facing").unwrap_or("up");
                    let ok = if facing == "down" {
                        is_non_air(blocks, above) || is_non_air(blocks, below)
                    } else {
                        is_non_air(blocks, below)
                    };
                    if !ok {
                        to_remove.push(pos);
                        continue;
                    }
                }
                let has_diag_stair = id.ends_with("_stairs")
                    && [
                        [x - 1, y - 1, z],
                        [x + 1, y - 1, z],
                        [x, y - 1, z - 1],
                        [x, y - 1, z + 1],
                        [x - 1, y + 1, z],
                        [x + 1, y + 1, z],
                        [x, y + 1, z - 1],
                        [x, y + 1, z + 1],
                    ]
                    .into_iter()
                    .any(|np| {
                        blocks
                            .get(&np)
                            .is_some_and(|nb| nb.block_id.ends_with("_stairs"))
                    });
                let has_neighbor = has_diag_stair
                    || [
                        [x - 1, y, z],
                        [x + 1, y, z],
                        [x, y - 1, z],
                        [x, y + 1, z],
                        [x, y, z - 1],
                        [x, y, z + 1],
                    ]
                    .into_iter()
                    .any(|np| is_non_air(blocks, np));
                if !has_neighbor {
                    to_remove.push(pos);
                }
            }
        }
        if to_remove.is_empty() {
            break;
        }
        for p in to_remove {
            blocks.insert(p, PlannedBlock::simple("minecraft:air"));
        }
    }
}

/// Dedicated monumental elliptical Roman Colosseum / amphitheater with multi-tier arcades, seating bowl & arena.
#[allow(clippy::too_many_arguments)]
fn generate_colosseum(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let pal = &req.palette;
    let seed = req.seed as u64;
    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();
    let cu_f = (grid.w - 1) as f64 * 0.5;
    let cv_f = (grid.d - 1) as f64 * 0.5;
    let cu_i = cu_f.round() as i32;
    let cv_i = cv_f.round() as i32;

    let rx = ((grid.w - 3) as f64 * 0.5).max(4.5);
    let rz = ((grid.d - 3) as f64 * 0.5).max(4.5);
    let r_arena = (2.2 / rx.min(rz)).max(0.40).min(0.52);
    let r_shell_inner = 0.84;

    let max_h = i32::from(height);
    let tier_h = if max_h >= 12 { 4 } else { 3 };
    let num_tiers = ((max_h - 3) / tier_h).clamp(2, 4);
    let rim_top = (1 + num_tiers * tier_h).min(max_h - 2).max(5);

    let radius_at = |u: i32, v: i32| -> f64 {
        let dx = (u as f64 - cu_f) / rx;
        let dz = (v as f64 - cv_f) / rz;
        (dx * dx + dz * dz).sqrt()
    };

    let mut col_fp = vec![false; (grid.w * grid.d) as usize];
    for v in 0..grid.d {
        for u in 0..grid.w {
            if radius_at(u, v) <= 1.03 {
                col_fp[grid.idx(u, v)] = true;
            }
            for y in 1..max_h {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    let seat_stairs = if pal.wall == "minecraft:smooth_quartz" || pal.wall == "minecraft:calcite" {
        "minecraft:smooth_quartz_stairs"
    } else {
        "minecraft:sandstone_stairs"
    };

    let dir_toward_center = |u: i32, v: i32| -> &'static str {
        let du = cu_i - u;
        let dv = cv_i - v;
        if du.abs() >= dv.abs() {
            if du >= 0 { grid.dir_left() } else { grid.dir_right() }
        } else if dv >= 0 {
            grid.dir_back()
        } else {
            grid.dir_front()
        }
    };

    for v in 0..grid.d {
        for u in 0..grid.w {
            if !col_fp[grid.idx(u, v)] {
                continue;
            }
            let r = radius_at(u, v);
            let terr = if req.adapt_terrain {
                terrain_rel_at(terrain_offsets, grid, u, v).clamp(-10, 0)
            } else {
                0
            };
            for y in terr..0 {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
            }

            let du = u - cu_i;
            let dv = v - cv_i;
            let in_gate_corridor = (du.abs() <= 1 && r > r_arena) || (dv.abs() <= 1 && r > r_arena);

            if r <= r_arena {
                // Central battle arena floor + decorative stone battle circle.
                let ring_center = r <= r_arena * 0.30 || (r >= r_arena * 0.88 && (u + v) % 2 == 0);
                let floor_blk = if spec.basement && r <= r_arena * 0.45 && (u + v) % 3 == 0 {
                    PlannedBlock::with_props(
                        "minecraft:oak_trapdoor",
                        vec![("facing", grid.dir_front()), ("half", "top"), ("open", "false")],
                    )
                } else if ring_center {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, 0, v))
                } else {
                    PlannedBlock::simple("minecraft:sand")
                };
                set_uv(&mut blocks, grid, u, 0, v, floor_blk);
                if spec.basement {
                    let is_hypogeum_wall = r >= r_arena * 0.85;
                    for y in -3..0 {
                        let b = if is_hypogeum_wall {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v))
                        } else {
                            PlannedBlock::simple("minecraft:air")
                        };
                        set_uv(&mut blocks, grid, u, y, v, b);
                    }
                    set_uv(&mut blocks, grid, u, -4, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, -4, v)));
                }
                continue;
            }

            set_uv(&mut blocks, grid, u, 0, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 0, v)));

            let is_outer_ring = !is_fp(grid, &col_fp, u - 1, v)
                || !is_fp(grid, &col_fp, u + 1, v)
                || !is_fp(grid, &col_fp, u, v - 1)
                || !is_fp(grid, &col_fp, u, v + 1);

            if r >= r_shell_inner {
                // Multi-tier arched colonnade facade shell + attic velarium crown.
                let along = if du.abs() <= dv.abs() { du.abs() } else { dv.abs() };
                let is_col_pier = along >= 2 && along % 3 == 2;
                let is_gate_post = ((du.abs() == 2 && dv.abs() > du.abs())
                    || (dv.abs() == 2 && du.abs() > dv.abs()))
                    && is_outer_ring;

                if in_gate_corridor {
                    let gate_h = 3.min(rim_top - 1);
                    for y in 1..=gate_h {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                    if is_outer_ring && gate_h >= 3 && (du == 0 || dv == 0) {
                        set_uv(&mut blocks, grid, u, gate_h, v, PlannedBlock::simple("minecraft:iron_bars"));
                    }
                    for y in (gate_h + 1)..rim_top {
                        let blk = if y == gate_h + 1 {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                        };
                        set_uv(&mut blocks, grid, u, y, v, blk);
                    }
                } else {
                    for tier in 0..num_tiers {
                        let t0 = 1 + tier * tier_h;
                        let t1 = (t0 + tier_h - 1).min(rim_top - 1);
                        if t0 > rim_top - 1 {
                            break;
                        }
                        set_uv(&mut blocks, grid, u, t0, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, t0, v)));
                        for y in (t0 + 1)..=t1 {
                            let is_entablature = y == t1 && tier_h >= 4;
                            let blk = if is_gate_post || (is_outer_ring && is_col_pier) {
                                make_frame_block_at(pal, "y", seed, u, y, v)
                            } else if is_entablature || !is_outer_ring {
                                if y == t0 + 1 && !is_outer_ring {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                                } else if is_entablature {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                                } else if y == t0 + 1 && tier >= 1 && is_outer_ring {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, y, v))
                                } else {
                                    PlannedBlock::simple("minecraft:air")
                                }
                            } else if y == t0 + 1 && tier >= 1 {
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, y, v))
                            } else {
                                PlannedBlock::simple("minecraft:air")
                            };
                            set_uv(&mut blocks, grid, u, y, v, blk);
                        }
                    }
                }

                // Attic cornice & rhythmic parapet merlons + parapet lanterns.
                set_uv(&mut blocks, grid, u, rim_top, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, rim_top, v)));
                if is_outer_ring && rim_top + 1 < max_h {
                    if is_col_pier || is_gate_post {
                        set_uv(&mut blocks, grid, u, rim_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, rim_top + 1, v)));
                        if rim_top + 2 < max_h {
                            let top_dec = if along % 6 == 2 {
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, rim_top + 2, v))
                            } else {
                                make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, rim_top + 2, v), "bottom")
                            };
                            set_uv(&mut blocks, grid, u, rim_top + 2, v, top_dec);
                        }
                    } else if !in_gate_corridor {
                        set_uv(&mut blocks, grid, u, rim_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, rim_top + 1, v)));
                    }
                } else if !is_outer_ring && rim_top + 1 < max_h && !in_gate_corridor && (u + v) % 2 == 0 {
                    // Velarium crimson/ivory sun-canopy anchored atop inner attic wall.
                    let sail = if pal.has_extra(MaterialPalette::SLOT_AWNING) {
                        pal.pick(MaterialPalette::SLOT_AWNING, seed, u, rim_top + 1, v)
                    } else if (u + v) % 4 == 0 {
                        pal.awning.clone()
                    } else {
                        "minecraft:white_wool".to_owned()
                    };
                    set_uv(&mut blocks, grid, u, rim_top + 1, v, PlannedBlock::simple(sail));
                }
            } else {
                // Stepped Spectator Cavea Bowl & 4 Vomitoria Arena Corridors.
                if in_gate_corridor {
                    set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, 1, v)));
                    continue;
                }
                let is_podium_lip = r <= r_arena + 0.09;
                if is_podium_lip {
                    set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, 1, v)));
                    if 2 < rim_top {
                        set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, 2, v)));
                    }
                    continue;
                }
                let radial_t = ((r - (r_arena + 0.09)) / (r_shell_inner - (r_arena + 0.09))).clamp(0.0, 1.0);
                let seat_top = (1 + (radial_t * (rim_top - 4).max(2) as f64).round() as i32)
                    .min(rim_top - 2)
                    .max(1);
                for y in 1..seat_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                }
                let seat_blk = if seat_top == 1 {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, seat_top, v))
                } else {
                    let chosen_seat = if pal.has_extra(MaterialPalette::SLOT_STAIR_BLOCK) {
                        pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, u, seat_top, v)
                    } else {
                        seat_stairs.to_owned()
                    };
                    make_stair(&chosen_seat, dir_toward_center(u, v), "bottom")
                };
                set_uv(&mut blocks, grid, u, seat_top, v, seat_blk);
            }
        }
    }

    // Imperial Box (Pulvinar) canopy on far (-v) podium overlooking the arena!
    if rim_top >= 6 {
        let box_u0 = cu_i - 2;
        let box_u1 = cu_i + 2;
        let box_v = (0..cv_i)
            .rev()
            .find(|&v| radius_at(cu_i, v) >= r_arena + 0.06)
            .unwrap_or(cv_i / 2);
        if grid.in_bounds(box_u0, box_v - 1) && grid.in_bounds(box_u1, box_v + 1) {
            for u in (box_u0 + 1)..box_u1 {
                for v in (box_v - 1)..=box_v {
                    if !col_fp[grid.idx(u, v)] || radius_at(u, v) <= r_arena {
                        continue;
                    }
                    set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, 1, v)));
                    set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, 2, v)));
                    set_uv(&mut blocks, grid, u, 3, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_CARPET, seed, u, 3, v)));
                    set_uv(&mut blocks, grid, u, 4, v, PlannedBlock::simple("minecraft:air"));
                    if u == box_u0 + 1 || u == box_u1 - 1 {
                        set_uv(&mut blocks, grid, u, 3, v, make_frame_block_at(pal, "y", seed, u, 3, v));
                        set_uv(&mut blocks, grid, u, 4, v, make_frame_block_at(pal, "y", seed, u, 4, v));
                    }
                    if 5 < max_h {
                        set_uv(&mut blocks, grid, u, 5, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, 5, v), "bottom"));
                    }
                }
            }
        }
    }

    // South triumphal gate coordinates & flanking entrance bollards.
    let gate_v = (cv_i..grid.d)
        .rev()
        .find(|&v| is_fp(grid, &col_fp, cu_i, v))
        .unwrap_or(grid.d - 2);
    let [dlx, dlz] = grid.to_local_xz(cu_i, gate_v);
    let door_pos = Some([dlx, 1, dlz]);
    for side in [-2, 2] {
        let lu = cu_i + side;
        let lv = (gate_v + 1).min(grid.d - 1);
        if grid.in_bounds(lu, lv) && !is_fp(grid, &col_fp, lu, lv) {
            set_uv(&mut blocks, grid, lu, 1, lv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lu, 1, lv)));
            set_uv(&mut blocks, grid, lu, 2, lv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lu, 2, lv)));
        }
    }

    if req.adapt_terrain
        && let Some(offsets) = terrain_offsets
    {
        blend_surrounding_terrain(
            &mut blocks,
            grid,
            &col_fp,
            Some((cu_i, gate_v)),
            &spec,
            pal,
            seed,
            height,
            offsets,
            None,
            noise,
            req.blend_radius,
        );
    }
    prune_unsupported_floating_blocks(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);

    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: grid.orig_w as u16,
        depth: grid.orig_d as u16,
        height,
        blocks,
        door_pos,
        living: crate::gen_common::LivingPlan::default(),
    }
}

/// Synthesizes a complete 3D voxel blueprint across structural, roof, exterior, interior, and courtyard passes.
pub fn generate_blueprint(
    req: &BuildRequest,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    generate_blueprint_with_water(req, terrain_offsets, None)
}

/// Synthesizes a complete 3D voxel blueprint with optional water/seabed depth telemetry for stilt & dock adaptation.
pub fn generate_blueprint_with_water(
    req: &BuildRequest,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) -> GeneratedBlueprint {
    let width = req.width.clamp(7, 96);
    let depth = req.depth.clamp(7, 96);
    let height = req.height.clamp(5, 48);
    let grid = LocalGrid::new(width, depth, req.facing);

    let mut spec = crate::parser::parse_description_with_biome(
        req.category,
        req.style,
        &req.description,
        grid.w as u16,
        grid.d as u16,
        height,
        req.noise_level,
        req.seed as u64,
        &req.biome_hint,
    );
    spec.living_decor &= req.living_decor;
    spec.spawn_npcs |= req.spawn_npcs;
    spec.blend_radius = if req.adapt_terrain { req.blend_radius.min(10) } else { 0 };
    let noise = NoiseField::new(req.seed as u64);

    let is_landmark_or_colosseum;
    let mut bp = if spec.category == BuildCategory::Dungeon {
        is_landmark_or_colosseum = false;
        crate::gen_dungeon_estate::generate_dungeon_complex(
            req,
            spec,
            grid,
            height,
            noise,
            terrain_offsets,
        )
    } else if spec.category == BuildCategory::Estate {
        is_landmark_or_colosseum = false;
        crate::gen_dungeon_estate::generate_estate_compound(
            req,
            spec,
            noise,
            terrain_offsets,
            water_depths,
        )
    } else if spec.theme == StructureTheme::Colosseum {
        is_landmark_or_colosseum = true;
        generate_colosseum(req, spec, grid, height, noise, terrain_offsets)
    } else if let Some(landmark) = crate::gen_landmarks::try_generate_landmark(
        req,
        spec.clone(),
        grid,
        height,
        noise,
        terrain_offsets,
    ) {
        is_landmark_or_colosseum = true;
        landmark
    } else if spec.category == BuildCategory::Bridge {
        is_landmark_or_colosseum = false;
        crate::gen_bridge::generate_bridge(req, spec, grid, height, noise, terrain_offsets)
    } else if spec.category == BuildCategory::Settlement {
        is_landmark_or_colosseum = false;
        crate::gen_settlement::generate_settlement(
            req,
            spec,
            noise,
            terrain_offsets,
            water_depths,
        )
    } else {
        is_landmark_or_colosseum = false;
        generate_standard_structure(req, spec, grid, height, noise, terrain_offsets, water_depths)
    };

    if is_landmark_or_colosseum {
        smooth_landmark_edges(&mut bp, req, terrain_offsets, water_depths, noise);
    }

    if crate::gen_weathering::apply_weathering_and_overgrowth(&mut bp, req, noise) {
        prune_unsupported_floating_blocks(&mut bp.blocks);
        crate::gen_common::resolve_connections_and_shapes(&mut bp.blocks);
    }
    bp
}

#[allow(clippy::too_many_arguments)]
fn generate_standard_structure(
    req: &BuildRequest,
    mut spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) -> GeneratedBlueprint {
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0xCAFE_BABE_1234_9876);
    let pal = &req.palette;

    let (fp, bounds) = build_footprint(grid, &spec, noise, terrain_offsets, &mut rng);

    // Auto-detect deep water / floodplains so aquatic builds rise cleanly on pilings;
    // dry slopes extend solid stone foundation skirts down cleanly instead of hovering on stilts.
    if req.adapt_terrain
        && !matches!(spec.category, BuildCategory::Castle)
        && !spec.on_stilts
    {
        let mut fp_cells = 0usize;
        let mut water_cells = 0usize;
        for v in 0..grid.d {
            for u in 0..grid.w {
                if !fp[grid.idx(u, v)] {
                    continue;
                }
                fp_cells += 1;
                if crate::gen_common::water_depth_at(water_depths, grid, u, v) > 0 {
                    water_cells += 1;
                }
            }
        }
        if fp_cells > 0 && water_cells * 3 >= fp_cells {
            spec.on_stilts = true;
            spec.foundation_height = spec.foundation_height.max(2);
            spec.basement = false;
        }
    } else if spec.on_stilts {
        spec.foundation_height = spec.foundation_height.max(2);
        spec.basement = false;
    }

    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();
    let roof_base_y =
        i32::from(spec.foundation_height) + i32::from(spec.floors) * i32::from(spec.floor_height);

    // 1. Clear working airspace envelope around footprint (wider 3-block skirt for cranes & overhangs).
    let clear_margin = if height >= 12 { 3 } else { 2 };
    for v in 0..grid.d {
        for u in 0..grid.w {
            let near_fp = (-clear_margin..=clear_margin)
                .any(|du| (-clear_margin..=clear_margin).any(|dv| is_fp(grid, &fp, u + du, v + dv)));
            if !near_fp {
                continue;
            }
            for y in 1..i32::from(height) {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // 2. Plinth, Sloped Skirt, Optional Basement, Stilts & Japanese Engawa Deck.
    build_foundation_and_basement(
        &mut blocks,
        grid,
        &fp,
        &spec,
        pal,
        seed,
        noise,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
    );

    let stepped_wing_opt = compute_stepped_wing_masks(grid, &fp, bounds, &spec, &mut rng);
    let tall_fp_ref: &[bool] = stepped_wing_opt.as_ref().map_or(&fp, |(t, _)| t.as_slice());
    let tall_bounds = compute_tight_bounds(grid, tall_fp_ref);

    // 3. Structural Floors, Framed Walls, Buttresses & Jetty Overhangs.
    let upper_fp = build_walls_and_buttresses(
        &mut blocks,
        grid,
        &fp,
        stepped_wing_opt.as_ref().map(|(t, _)| t.as_slice()),
        bounds,
        &spec,
        pal,
        seed,
        noise,
        &mut rng,
    );

    let roof_fp = upper_fp.as_deref().unwrap_or(tall_fp_ref);
    let roof_bounds = compute_tight_bounds(grid, roof_fp);
    if !matches!(spec.category, BuildCategory::Pavilion) && spec.theme != StructureTheme::Greenhouse {
        for v in 0..grid.d {
            for u in 0..grid.w {
                if roof_fp[grid.idx(u, v)] {
                    set_uv(&mut blocks, grid, u, roof_base_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, roof_base_y, v)));
                }
            }
        }
    }

    if let Some((ref tall_fp, terrace_on_lower)) = stepped_wing_opt {
        let cross_gable = rng.next_f64() < 0.50;
        build_stepped_wing_top(
            &mut blocks,
            grid,
            &fp,
            tall_fp,
            roof_fp,
            terrace_on_lower,
            cross_gable,
            &spec,
            pal,
            seed,
            height,
        );
    }

    // First-floor horizontal waist eave skirt (`mokoshi`) below upper stories.
    build_first_floor_mokoshi_skirt(
        &mut blocks,
        grid,
        &fp,
        roof_fp,
        &spec,
        pal,
        seed,
        i32::from(spec.foundation_height) + i32::from(spec.floor_height),
    );

    // 4. Main Entrance, Porch & Adaptive Slope Steps / Dock Pier.
    let (best_door, door_pos) = build_entrance_and_porch(
        &mut blocks,
        grid,
        &fp,
        bounds,
        &spec,
        pal,
        seed,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
        &mut rng,
    );

    // 4b. Striped Cloth Market Awning & Open Street Counter Stall.
    build_market_awning_stall(&mut blocks, grid, &fp, bounds, best_door, &spec, pal, seed);

    // 5. Spiral / Multi-floor Staircases, Guardrails & Basement Shaft.
    let (mut stairwell_uv, mut stair_headroom) = build_staircases(
        &mut blocks,
        grid,
        tall_fp_ref,
        tall_bounds,
        best_door,
        &spec,
        pal,
        seed,
        roof_base_y,
        &mut rng,
    );

    // 6. Windows, Shutters, Flower Boxes, Bay Windows & Supported Balcony.
    build_windows_and_balcony(
        &mut blocks,
        grid,
        &fp,
        Some(roof_fp),
        best_door,
        &stairwell_uv,
        &spec,
        pal,
        seed,
        noise,
        &mut rng,
    );
    build_bay_windows(
        &mut blocks,
        grid,
        &fp,
        roof_fp,
        roof_bounds,
        best_door,
        &stairwell_uv,
        &spec,
        pal,
        seed,
    );

    // 7. Roofs, Cross-Gables, Dormers, Greenhouse Glass Vault, Lighthouse Lantern & Windmill Sails.
    let max_placed_roof_y = build_roof_and_dormers(
        &mut blocks,
        grid,
        roof_fp,
        roof_bounds,
        &spec,
        pal,
        seed,
        height,
        roof_base_y,
        noise,
        &mut rng,
    );

    // 8. Corner Turrets / Lightning Finials.
    build_corner_turret(
        &mut blocks,
        grid,
        tall_fp_ref,
        roof_fp,
        tall_bounds,
        best_door,
        &mut stairwell_uv,
        &mut stair_headroom,
        &spec,
        pal,
        seed,
        max_placed_roof_y,
        height,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
        &mut rng,
    );

    // 9. Fireplace Hearth & Chimney Stack.
    let chimney_uv = build_fireplace_and_chimney(
        &mut blocks,
        grid,
        tall_fp_ref,
        tall_bounds,
        best_door,
        &stairwell_uv,
        &spec,
        pal,
        seed,
        max_placed_roof_y,
        height,
        &mut rng,
    );

    // 10. Interior Rooms, Chandeliers & Functional Props.
    build_interior_zoning(
        &mut blocks,
        grid,
        &fp,
        Some(roof_fp),
        bounds,
        best_door,
        &stairwell_uv,
        chimney_uv,
        &spec,
        pal,
        seed,
        &mut rng,
    );

    // 11. Courtyard Path, Roofed Well, Streetlamps & Foliage.
    build_courtyard_and_landscaping(
        &mut blocks,
        grid,
        &fp,
        bounds,
        best_door,
        &spec,
        pal,
        seed,
        noise,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
    );

    // 11a. Cantilevered upper lantern cranes, custom yard trees & downspouts/signs.
    build_lantern_cranes(
        &mut blocks,
        grid,
        &fp,
        roof_fp,
        roof_bounds,
        &spec,
        pal,
        seed,
        roof_base_y,
        height,
    );
    build_downspouts_and_signs(&mut blocks, grid, &fp, bounds, best_door, &spec, pal, seed);
    build_arcade_gallery(
        &mut blocks,
        grid,
        &fp,
        bounds,
        best_door,
        &spec,
        pal,
        seed,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
    );
    if !spec.on_stilts {
        build_custom_yard_trees(&mut blocks, grid, &fp, bounds, best_door, &spec, pal, seed, height);
    }

    // 11b. Smooth perimeter hillside terraces & radial biome blending outside footprint.
    if req.adapt_terrain
        && let Some(offsets) = terrain_offsets
    {
        blend_surrounding_terrain(
            &mut blocks,
            grid,
            &fp,
            best_door,
            &spec,
            pal,
            seed,
            height,
            offsets,
            water_depths,
            noise,
            req.blend_radius,
        );
    }

    // 11c. Final doorway corridor & staircase headroom guarantee so player never gets blocked.
    if let Some((du, dv)) = best_door {
        let door_y = i32::from(spec.foundation_height) + 1;
        let is_wide = matches!(
            spec.category,
            BuildCategory::Barn | BuildCategory::Castle | BuildCategory::Temple
        ) && is_fp(grid, &fp, du + 1, dv)
            && is_fp(grid, &fp, du + 2, dv);
        let door_u1 = if is_wide { du + 1 } else { du };
        for door_u in du..=door_u1 {
            for sv in (dv + 1)..=(dv + 3).min(grid.d - 1) {
                if !is_fp(grid, &fp, door_u, sv) {
                    if sv <= dv + 2 {
                        for cy in door_y..=(door_y + 1) {
                            let is_step = crate::gen_common::get_uv(&blocks, grid, door_u, cy, sv)
                                .is_some_and(|b| b.block_id.ends_with("_stairs"));
                            if !is_step {
                                set_uv(&mut blocks, grid, door_u, cy, sv, PlannedBlock::simple("minecraft:air"));
                            }
                        }
                    }
                    if let Some(above_blk) = crate::gen_common::get_uv(&blocks, grid, door_u, door_y + 2, sv)
                        && (above_blk.block_id.ends_with("_leaves")
                            || above_blk.block_id == "minecraft:grass_block"
                            || above_blk.block_id == "minecraft:dirt"
                            || above_blk.block_id == "minecraft:coarse_dirt"
                            || above_blk.block_id == "minecraft:moss_block")
                    {
                        set_uv(&mut blocks, grid, door_u, door_y + 2, sv, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
    }
    for &(su, sy, sv) in &stair_headroom {
        let head_clear = if sy >= roof_base_y { 2 } else { 3 };
        for cy in (sy + 1)..=(sy + head_clear).min(i32::from(height) - 1) {
            if cy >= roof_base_y + 1 {
                let roof_shell_above = crate::gen_common::get_uv(&blocks, grid, su, cy + 1, sv)
                    .is_some_and(|b| {
                        b.block_id.ends_with("_stairs")
                            || b.block_id.ends_with("_slab")
                            || pal.slot_contains(MaterialPalette::SLOT_ROOF, &b.block_id)
                    });
                if roof_shell_above && cy >= sy + 2 {
                    continue;
                }
            }
            let protected_door_frame = best_door.is_some_and(|(du, dv)| {
                cy <= i32::from(spec.foundation_height) + 3
                    && (su - du).abs() <= 2
                    && sv == dv
            });
            let blocked_by_non_stair = !protected_door_frame
                && crate::gen_common::get_uv(&blocks, grid, su, cy, sv).is_some_and(|b| {
                    b.block_id != "minecraft:air"
                        && b.block_id != "minecraft:ladder"
                        && !b.block_id.ends_with("_trapdoor")
                        && !b.block_id.ends_with("_stairs")
                        && !b.block_id.ends_with("_door")
                });
            if blocked_by_non_stair {
                set_uv(&mut blocks, grid, su, cy, sv, PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Enforce complete 2-high doors, solid supports, solid lintels, and non-window jambs.
    enforce_door_invariants(&mut blocks, &pal, seed, &spec);

    // 11d. Living Interiors 2.0 (banners, lecterns, signs, paintings, armor stands, item frames, redstone & optional NPCs).
    let mut living = decorate_living_interiors(
        &mut blocks,
        grid,
        &fp,
        bounds,
        best_door,
        &stairwell_uv,
        &spec,
        pal,
        seed,
    );
    let ext_living = decorate_living_exteriors(
        &mut blocks,
        grid,
        &fp,
        best_door,
        &spec,
        pal,
        seed,
        req.adapt_terrain,
        terrain_offsets,
        water_depths,
    );
    living.merge_offset(ext_living, 0, 0, 0);
    living.apply_filters(spec.living_decor, spec.spawn_npcs);
    if spec.living_decor {
        living.materialize_signs_and_banners(&mut blocks, wood_prefix_for_palette(pal));
    }

    // 12. Structural Anti-Floating Sanitizer Pass.
    prune_unsupported_floating_blocks(&mut blocks);

    // 13. Connect glass panes, fences, stone walls, iron bars & miter stair corners cleanly.
    resolve_connections_and_shapes(&mut blocks);

    living
        .lecterns
        .retain(|l| blocks.get(&l.pos).is_some_and(|b| b.block_id == "minecraft:lectern"));
    living.banners.retain(|b| {
        blocks
            .get(&b.pos)
            .is_some_and(|blk| blk.block_id.ends_with("_banner"))
    });
    living.signs.retain(|s| {
        blocks
            .get(&s.pos)
            .is_some_and(|blk| blk.block_id.ends_with("_sign"))
    });

    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: grid.orig_w as u16,
        depth: grid.orig_d as u16,
        height,
        blocks,
        door_pos,
        living,
    }
}

fn block_needs_solid_floor(bid: &str) -> bool {
    bid == "minecraft:air"
        || bid == "minecraft:water"
        || bid == "minecraft:lava"
        || bid.contains("glass")
        || bid.ends_with("_grate")
        || bid == "minecraft:iron_bars"
        || bid.ends_with("_door")
        || bid.ends_with("_fence")
        || bid.ends_with("_slab")
        || bid.ends_with("_stairs")
        || bid.ends_with("_trapdoor")
}

fn is_window_or_open(bid: &str) -> bool {
    bid == "minecraft:air"
        || bid.contains("glass")
        || bid.ends_with("_grate")
        || bid == "minecraft:iron_bars"
}

fn is_transparent_window_block(bid: &str) -> bool {
    bid.contains("glass") || bid.ends_with("_grate") || bid == "minecraft:iron_bars"
}

/// Repairs half-doors, missing door supports, transparent lintels, and glass/grate door jambs.
pub(crate) fn enforce_door_invariants(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    seed: u64,
    spec: &BlueprintSpec,
) {
    let door_entries: Vec<([i32; 3], PlannedBlock)> = blocks
        .iter()
        .filter(|(_, b)| b.block_id.ends_with("_door"))
        .map(|(&pos, b)| (pos, b.clone()))
        .collect();

    for (pos, pb) in &door_entries {
        let half = pb
            .props
            .iter()
            .find(|&&(k, _)| k == "half")
            .map(|&(_, v)| v);
        if half != Some("upper") {
            continue;
        }
        let below = [pos[0], pos[1] - 1, pos[2]];
        let below_is_door = blocks
            .get(&below)
            .is_some_and(|b| b.block_id.ends_with("_door"));
        if !below_is_door {
            let below_open = blocks
                .get(&below)
                .is_none_or(|b| is_window_or_open(&b.block_id));
            if below_open {
                let mut lower_props: Vec<(&'static str, &'static str)> = pb
                    .props
                    .iter()
                    .copied()
                    .filter(|&(k, _)| k != "half")
                    .collect();
                lower_props.push(("half", "lower"));
                blocks.insert(
                    below,
                    PlannedBlock::with_props(pb.block_id.clone(), lower_props),
                );
            } else {
                blocks.remove(pos);
            }
        }
    }

    let lower_doors: Vec<([i32; 3], PlannedBlock)> = blocks
        .iter()
        .filter(|(_, b)| {
            b.block_id.ends_with("_door")
                && !b.props.iter().any(|&(k, v)| k == "half" && v == "upper")
        })
        .map(|(&pos, b)| (pos, b.clone()))
        .collect();

    for (pos, pb) in lower_doors {
        let [x, y, z] = pos;
        let floor_pos = [x, y - 1, z];
        if blocks
            .get(&floor_pos)
            .is_none_or(|b| block_needs_solid_floor(&b.block_id))
        {
            blocks.insert(
                floor_pos,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, x, y - 1, z)),
            );
        }

        let upper_pos = [x, y + 1, z];
        let mut upper_props: Vec<(&'static str, &'static str)> = pb
            .props
            .iter()
            .copied()
            .filter(|&(k, _)| k != "half")
            .collect();
        upper_props.push(("half", "upper"));
        blocks.insert(
            upper_pos,
            PlannedBlock::with_props(pb.block_id.clone(), upper_props),
        );

        let lintel_pos = [x, y + 2, z];
        let lintel_bad = blocks.get(&lintel_pos).is_none_or(|b| {
            let bid = b.block_id.as_str();
            if spec.category == BuildCategory::Castle && bid == "minecraft:iron_bars" {
                false
            } else {
                is_window_or_open(bid)
            }
        });
        if lintel_bad {
            blocks.insert(
                lintel_pos,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, x, y + 2, z)),
            );
        }

        let facing = pb
            .props
            .iter()
            .find(|&&(k, _)| k == "facing")
            .map(|&(_, v)| v)
            .unwrap_or("south");
        let (jx, jz) = match facing {
            "east" | "west" => (0, 1),
            _ => (1, 0),
        };
        for sign in [-1, 1] {
            for dy in 0..=2 {
                let jp = [x + sign * jx, y + dy, z + sign * jz];
                let bad_jamb = blocks.get(&jp).is_some_and(|b| {
                    let bid = b.block_id.as_str();
                    !bid.ends_with("_door")
                        && is_transparent_window_block(bid)
                        && !(matches!(spec.category, BuildCategory::Castle | BuildCategory::Dungeon)
                            && bid == "minecraft:iron_bars")
                });
                if bad_jamb {
                    let repl = if dy <= 1 {
                        pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, jp[0], jp[1], jp[2])
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, jp[0], jp[1], jp[2])
                    };
                    blocks.insert(jp, PlannedBlock::simple(repl));
                }
            }
        }
    }
}
