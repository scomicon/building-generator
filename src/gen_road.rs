use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::gen_common::{
    GeneratedBlueprint, LivingEntityKind, LivingPlan, PlannedBlock, PlannedEntity, PlannedSign,
    SignPurpose, clamp_i32, make_slab, prune_map_unsupported, resolve_connections_and_shapes,
};
use crate::generator::apply_radial_terrain_column;
use crate::model::{
    BuildCategory, BuildStyle, CardinalFacing, MaterialPalette, NoiseLevel, RoadConnectRequest,
};
use crate::noise::NoiseField;
use crate::parser::parse_description_with_biome;

#[derive(Clone, Copy, Eq, PartialEq)]
struct NavNode {
    cost: i64,
    idx: usize,
}

impl Ord for NavNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost).then_with(|| self.idx.cmp(&other.idx))
    }
}

impl PartialOrd for NavNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn road_palette_for_style(style: BuildStyle) -> MaterialPalette {
    let mut pal = MaterialPalette::preset(0);
    match style {
        BuildStyle::Desert => {
            pal.paving = "minecraft:smooth_sandstone".into();
            pal.foundation = "minecraft:sandstone".into();
            pal.trim_wall = "minecraft:cut_sandstone".into();
            pal.roof_slab = "minecraft:sandstone_slab".into();
            pal.fence = "minecraft:birch_fence".into();
            pal.pillar = "minecraft:cut_sandstone".into();
            pal.wall_trim = "minecraft:sandstone_wall".into();
        }
        BuildStyle::Modern => {
            pal.paving = "minecraft:gray_concrete".into();
            pal.foundation = "minecraft:deepslate_tiles".into();
            pal.trim_wall = "minecraft:smooth_stone".into();
            pal.roof_slab = "minecraft:smooth_stone_slab".into();
            pal.fence = "minecraft:iron_bars".into();
            pal.pillar = "minecraft:polished_deepslate".into();
            pal.wall_trim = "minecraft:polished_deepslate_wall".into();
        }
        BuildStyle::Japanese => {
            pal.paving = "minecraft:andesite".into();
            pal.foundation = "minecraft:stone_bricks".into();
            pal.trim_wall = "minecraft:polished_andesite".into();
            pal.roof_slab = "minecraft:crimson_slab".into();
            pal.fence = "minecraft:crimson_fence".into();
            pal.pillar = "minecraft:crimson_planks".into();
            pal.wall_trim = "minecraft:stone_brick_wall".into();
        }
        BuildStyle::Gothic | BuildStyle::Nordic | BuildStyle::Steampunk => {
            pal.paving = "minecraft:deepslate_bricks".into();
            pal.foundation = "minecraft:cobbled_deepslate".into();
            pal.trim_wall = "minecraft:polished_basalt".into();
            pal.roof_slab = "minecraft:dark_oak_slab".into();
            pal.fence = "minecraft:dark_oak_fence".into();
            pal.pillar = "minecraft:dark_oak_planks".into();
            pal.wall_trim = "minecraft:deepslate_brick_wall".into();
        }
        BuildStyle::Classical => {
            pal.paving = "minecraft:quartz_bricks".into();
            pal.foundation = "minecraft:smooth_stone".into();
            pal.trim_wall = "minecraft:calcite".into();
            pal.roof_slab = "minecraft:quartz_slab".into();
            pal.fence = "minecraft:pale_oak_fence".into();
            pal.pillar = "minecraft:quartz_pillar".into();
            pal.wall_trim = "minecraft:stone_brick_wall".into();
        }
        _ => {
            pal.paving = "minecraft:cobblestone".into();
            pal.foundation = "minecraft:stone_bricks".into();
            pal.trim_wall = "minecraft:mossy_stone_bricks".into();
            pal.roof_slab = "minecraft:spruce_slab".into();
            pal.fence = "minecraft:spruce_fence".into();
            pal.pillar = "minecraft:spruce_planks".into();
            pal.wall_trim = "minecraft:stone_brick_wall".into();
        }
    }
    pal
}

fn find_road_path(
    w: i32,
    d: i32,
    start: (i32, i32),
    end: (i32, i32),
    terrain_offsets: &[i32],
    water_depths: &[i32],
    auto_bridge: bool,
) -> Vec<(i32, i32)> {
    let total = (w * d) as usize;
    let to_idx = |x: i32, z: i32| (z * w + x) as usize;
    let surf = |x: i32, z: i32| -> i32 {
        terrain_offsets
            .get(to_idx(x, z))
            .copied()
            .unwrap_or(0)
            .clamp(-24, 24)
    };
    let wd = |x: i32, z: i32| -> i32 { water_depths.get(to_idx(x, z)).copied().unwrap_or(0) };

    let s_idx = to_idx(start.0, start.1);
    let e_idx = to_idx(end.0, end.1);
    if s_idx == e_idx {
        return vec![start];
    }

    let mut dist = vec![i64::MAX / 4; total];
    let mut prev = vec![usize::MAX; total];
    let mut heap = BinaryHeap::new();
    dist[s_idx] = 0;
    heap.push(NavNode { cost: 0, idx: s_idx });

    let line_dx = (end.0 - start.0) as f64;
    let line_dz = (end.1 - start.1) as f64;
    let line_len = (line_dx * line_dx + line_dz * line_dz).sqrt().max(1.0);

    let dirs = [
        (1, 0, 10i64),
        (-1, 0, 10),
        (0, 1, 10),
        (0, -1, 10),
        (1, 1, 14),
        (1, -1, 14),
        (-1, 1, 14),
        (-1, -1, 14),
    ];

    while let Some(NavNode { cost, idx }) = heap.pop() {
        if cost > dist[idx] {
            continue;
        }
        if idx == e_idx {
            break;
        }
        let cx = (idx as i32) % w;
        let cz = (idx as i32) / w;
        let cs = surf(cx, cz);

        for &(dx, dz, base_cost) in &dirs {
            let nx = cx + dx;
            let nz = cz + dz;
            if nx < 1 || nx >= w - 1 || nz < 1 || nz >= d - 1 {
                if (nx, nz) != end {
                    continue;
                }
            }
            if !(0..w).contains(&nx) || !(0..d).contains(&nz) {
                continue;
            }
            let n_idx = to_idx(nx, nz);
            let ns = surf(nx, nz);
            let n_wd = wd(nx, nz);

            let dy = (ns - cs).abs() as i64;
            let slope_pen = match dy {
                0 => 0,
                1 => 3,
                2 => 14,
                _ => 42 * dy,
            };
            let water_pen = if n_wd > 0 {
                if auto_bridge { 16 } else { 220 }
            } else {
                0
            };

            // Mild centerline bias so flat plains stay straight rather than meandering.
            let cross = (((nx - start.0) as f64) * line_dz - ((nz - start.1) as f64) * line_dx).abs()
                / line_len;
            let center_pen = (cross * 1.5) as i64;

            let next_cost = cost + base_cost + slope_pen + water_pen + center_pen;
            if next_cost < dist[n_idx] {
                dist[n_idx] = next_cost;
                prev[n_idx] = idx;
                heap.push(NavNode {
                    cost: next_cost,
                    idx: n_idx,
                });
            }
        }
    }

    if prev[e_idx] == usize::MAX {
        // Fallback Bresenham line
        let mut pts = Vec::new();
        let steps = (end.0 - start.0).abs().max((end.1 - start.1).abs()).max(1);
        for s in 0..=steps {
            let t = f64::from(s) / f64::from(steps);
            let x = (f64::from(start.0) + line_dx * t).round() as i32;
            let z = (f64::from(start.1) + line_dz * t).round() as i32;
            if pts.last().copied() != Some((x, z)) {
                pts.push((x, z));
            }
        }
        return pts;
    }

    let mut path = Vec::new();
    let mut cur = e_idx;
    while cur != usize::MAX {
        path.push(((cur as i32) % w, (cur as i32) / w));
        if cur == s_idx {
            break;
        }
        cur = prev[cur];
    }
    path.reverse();
    path
}

/// Generates a terrain-relaxed 2-point road & automatic arched bridge blueprint inside `box_origin` `[bw x bd]`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_road_connector(
    req: &RoadConnectRequest,
    terrain_offsets: &[i32],
    water_depths: &[i32],
    box_origin: [i32; 3],
    bw: u16,
    bd: u16,
) -> GeneratedBlueprint {
    let w = i32::from(bw.max(7));
    let d = i32::from(bd.max(7));
    let seed = req.seed as u64;
    let pal = road_palette_for_style(req.style);
    let noise = NoiseField::new(seed);

    let sx = clamp_i32(req.start_x - box_origin[0], 0, w - 1);
    let sz = clamp_i32(req.start_z - box_origin[2], 0, d - 1);
    let ex = clamp_i32(req.end_x - box_origin[0], 0, w - 1);
    let ez = clamp_i32(req.end_z - box_origin[2], 0, d - 1);

    let surf_at = |x: i32, z: i32| -> i32 {
        let cx = clamp_i32(x, 0, w - 1);
        let cz = clamp_i32(z, 0, d - 1);
        terrain_offsets
            .get((cz * w + cx) as usize)
            .copied()
            .unwrap_or(0)
            .clamp(-24, 24)
    };
    let wd_at = |x: i32, z: i32| -> i32 {
        let cx = clamp_i32(x, 0, w - 1);
        let cz = clamp_i32(z, 0, d - 1);
        water_depths
            .get((cz * w + cx) as usize)
            .copied()
            .unwrap_or(0)
            .clamp(0, 20)
    };

    let path = find_road_path(w, d, (sx, sz), (ex, ez), terrain_offsets, water_depths, req.auto_bridge);
    let n = path.len().max(1);

    let mut is_bridge_pt: Vec<bool> = path
        .iter()
        .map(|&(x, z)| req.auto_bridge && wd_at(x, z) > 0)
        .collect();
    // Fill single dry gaps between bridge spans so bridges never split unnecessarily.
    if n >= 3 {
        for i in 1..(n - 1) {
            if !is_bridge_pt[i] && is_bridge_pt[i - 1] && is_bridge_pt[i + 1] {
                is_bridge_pt[i] = true;
            }
        }
    }

    let mut raw_y: Vec<i32> = path
        .iter()
        .enumerate()
        .map(|(i, &(x, z))| {
            let s = surf_at(x, z);
            if is_bridge_pt[i] {
                s + 1
            } else {
                s
            }
        })
        .collect();

    // Multi-pass smoothing + monotonic <=1 slope constraint along the road spine.
    let mut smooth_y = raw_y.clone();
    for _pass in 0..3 {
        let prev_pass = smooth_y.clone();
        for i in 1..(n.saturating_sub(1)) {
            if !is_bridge_pt[i] {
                let avg = (prev_pass[i - 1] + prev_pass[i] * 2 + prev_pass[i + 1] + 2) / 4;
                smooth_y[i] = avg;
            }
        }
    }
    for i in 1..n {
        if smooth_y[i] > smooth_y[i - 1] + 1 {
            smooth_y[i] = smooth_y[i - 1] + 1;
        } else if smooth_y[i] < smooth_y[i - 1] - 1 {
            smooth_y[i] = smooth_y[i - 1] - 1;
        }
    }
    for i in (0..(n - 1)).rev() {
        if smooth_y[i] > smooth_y[i + 1] + 1 {
            smooth_y[i] = smooth_y[i + 1] + 1;
        } else if smooth_y[i] < smooth_y[i + 1] - 1 {
            smooth_y[i] = smooth_y[i + 1] - 1;
        }
    }

    // Apply graceful parabolic arches across contiguous water bridge spans.
    let mut i = 0;
    while i < n {
        if !is_bridge_pt[i] {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && is_bridge_pt[i] {
            i += 1;
        }
        let e = i - 1;
        let left_y = if s > 0 { smooth_y[s - 1] } else { raw_y[s] };
        let right_y = if e + 1 < n { smooth_y[e + 1] } else { raw_y[e] };
        let water_top = (s..=e).map(|k| surf_at(path[k].0, path[k].1)).max().unwrap_or(0);
        let abut_y = left_y.max(right_y).max(water_top + 1);
        let span = (e - s + 1) as f64;
        let arch_rise = if span >= 9.0 {
            2.0
        } else if span >= 4.0 {
            1.0
        } else {
            0.0
        };
        for k in s..=e {
            let t = if e > s {
                (k - s) as f64 / (e - s) as f64
            } else {
                0.5
            };
            let arch = (4.0 * t * (1.0 - t) * arch_rise).round() as i32;
            smooth_y[k] = abut_y + arch;
        }
    }
    raw_y.clear();

    // Rasterize corridor columns cleanly with Euclidean radial ownership.
    let half_w = f64::from((req.road_width.clamp(3, 5) - 1) / 2);
    let r_ceil = half_w.ceil() as i32 + 1;
    // Value: (best_dist2, road_y, is_bridge, is_edge, path_idx)
    let mut road_cols: BTreeMap<(i32, i32), (i32, i32, bool, bool, usize)> = BTreeMap::new();
    for (p_idx, &(px, pz)) in path.iter().enumerate() {
        let py = smooth_y[p_idx];
        let br = is_bridge_pt[p_idx];
        for dz in -r_ceil..=r_ceil {
            for dx in -r_ceil..=r_ceil {
                let d2 = dx * dx + dz * dz;
                let dist = (d2 as f64).sqrt();
                if dist > half_w + 0.35 {
                    continue;
                }
                let x = px + dx;
                let z = pz + dz;
                if !(0..w).contains(&x) || !(0..d).contains(&z) {
                    continue;
                }
                let is_edge = dist >= half_w - 0.35;
                road_cols
                    .entry((x, z))
                    .and_modify(|e| {
                        if d2 < e.0 {
                            *e = (d2, py, br, is_edge, p_idx);
                        }
                    })
                    .or_insert((d2, py, br, is_edge, p_idx));
            }
        }
    }

    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();
    let mut living = LivingPlan::default();
    let mut max_rel_y = 6i32;
    let mut min_rel_y = 0i32;

    let style_center_blk = |x: i32, y: i32, z: i32, h: f64| -> PlannedBlock {
        match req.style {
            BuildStyle::Desert => PlannedBlock::simple(if h > 0.72 {
                "minecraft:cut_sandstone"
            } else {
                "minecraft:smooth_sandstone"
            }),
            BuildStyle::Modern => PlannedBlock::simple(if h > 0.78 {
                "minecraft:polished_deepslate"
            } else {
                "minecraft:gray_concrete"
            }),
            BuildStyle::Japanese => PlannedBlock::simple(if h > 0.68 {
                "minecraft:polished_andesite"
            } else {
                "minecraft:andesite"
            }),
            BuildStyle::Gothic | BuildStyle::Nordic | BuildStyle::Steampunk => {
                PlannedBlock::simple(if h > 0.74 {
                    "minecraft:cracked_deepslate_bricks"
                } else {
                    "minecraft:deepslate_bricks"
                })
            }
            BuildStyle::Classical => PlannedBlock::simple(if h > 0.75 {
                "minecraft:calcite"
            } else {
                "minecraft:quartz_bricks"
            }),
            _ => PlannedBlock::simple(if h > 0.74 {
                "minecraft:mossy_cobblestone".to_owned()
            } else if h < 0.22 {
                "minecraft:gravel".to_owned()
            } else {
                pal.pick(MaterialPalette::SLOT_PAVING, seed, x, y, z)
            }),
        }
    };

    for (&(x, z), &(_, ry, is_bridge, is_edge, p_idx)) in &road_cols {
        let terr = surf_at(x, z);
        let wd = wd_at(x, z);
        max_rel_y = max_rel_y.max(ry + 6).max(terr + 4);
        min_rel_y = min_rel_y.min(terr - wd - 2);

        let h = noise.cell_hash(x, 0, z);
        if is_bridge {
            // Arched bridge deck + stone piers over water + parapet rails!
            let deck_blk = if is_edge {
                PlannedBlock::simple(&pal.foundation)
            } else {
                PlannedBlock::simple(&pal.pillar)
            };
            blocks.insert([x, ry, z], deck_blk);
            for ay in (ry + 1)..=(ry + 4) {
                blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
            }
            if is_edge {
                if p_idx % 4 == 0 {
                    let bot = (terr - wd - 1).clamp(-26, ry - 1);
                    for py in bot..ry {
                        blocks.insert(
                            [x, py, z],
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, py, z)),
                        );
                    }
                    blocks.insert([x, ry + 1, z], PlannedBlock::simple(&pal.wall_trim));
                    if req.living_decor {
                        blocks.insert(
                            [x, ry + 2, z],
                            PlannedBlock::with_props("minecraft:lantern", vec![("hanging", "false")]),
                        );
                    }
                } else {
                    blocks.insert([x, ry + 1, z], PlannedBlock::simple(&pal.fence));
                }
            }
        } else {
            // Terrain-conforming road surface + solid skirt if elevated + clean headroom!
            let lo_fill = (terr - wd - 1).clamp(-26, ry - 1);
            for fy in lo_fill..ry {
                blocks.insert(
                    [x, fy, z],
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, fy, z)),
                );
            }
            let surf_blk = if is_edge {
                PlannedBlock::simple(&pal.trim_wall)
            } else {
                style_center_blk(x, ry, z, h)
            };
            blocks.insert([x, ry, z], surf_blk);
            for ay in (ry + 1)..=(terr.max(ry) + 4) {
                blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Add automated night streetlights along dry road shoulders every 10 blocks + waypost signs at endpoints.
    let mut shoulder_lamps: BTreeSet<(i32, i32)> = BTreeSet::new();
    let offset_shoulder = (half_w.round() as i32) + 1;
    for (idx, &(px, pz)) in path.iter().enumerate() {
        let is_endpoint = idx == 0 || idx + 1 == n;
        if !is_endpoint && (idx % 10 != 5 || is_bridge_pt[idx]) {
            continue;
        }
        let next_p = path[(idx + 1).min(n - 1)];
        let prev_p = path[idx.saturating_sub(1)];
        let tx = (next_p.0 - prev_p.0).signum();
        let _tz = (next_p.1 - prev_p.1).signum();
        // Perpendicular shoulder
        let (ox, oz) = if tx != 0 { (0, offset_shoulder) } else { (offset_shoulder, 0) };
        let sx_sh = px + ox;
        let sz_sh = pz + oz;
        if !(1..(w - 1)).contains(&sx_sh)
            || !(1..(d - 1)).contains(&sz_sh)
            || road_cols.contains_key(&(sx_sh, sz_sh))
            || wd_at(sx_sh, sz_sh) > 0
            || shoulder_lamps.contains(&(sx_sh, sz_sh))
        {
            continue;
        }
        let ty = surf_at(sx_sh, sz_sh);
        let post_y = ty.max(smooth_y[idx]);
        for fy in (ty.min(post_y) - 1)..=post_y {
            blocks.insert([sx_sh, fy, sz_sh], PlannedBlock::simple(&pal.foundation));
        }
        if is_endpoint && req.living_decor {
            blocks.insert([sx_sh, post_y + 1, sz_sh], PlannedBlock::simple(&pal.fence));
            blocks.insert(
                [sx_sh, post_y + 2, sz_sh],
                PlannedBlock::with_props("minecraft:oak_sign", vec![("rotation", "0"), ("waterlogged", "false")]),
            );
            living.signs.push(PlannedSign {
                pos: [sx_sh, post_y + 2, sz_sh],
                purpose: SignPurpose::Waypost,
                custom_en: None,
                custom_ru: None,
            });
            shoulder_lamps.insert((sx_sh, sz_sh));
        } else if req.living_decor {
            blocks.insert([sx_sh, post_y + 1, sz_sh], PlannedBlock::simple(&pal.wall_trim));
            blocks.insert([sx_sh, post_y + 2, sz_sh], PlannedBlock::simple(&pal.fence));
            blocks.insert([sx_sh, post_y + 3, sz_sh], PlannedBlock::simple(&pal.fence));
            blocks.insert(
                [sx_sh, post_y + 4, sz_sh],
                PlannedBlock::with_props("minecraft:redstone_lamp", vec![("lit", "false")]),
            );
            blocks.insert(
                [sx_sh, post_y + 5, sz_sh],
                PlannedBlock::with_props(
                    "minecraft:daylight_detector",
                    vec![("inverted", "true"), ("power", "0")],
                ),
            );
            shoulder_lamps.insert((sx_sh, sz_sh));
        }
    }

    // Radial biome-aware terrain shoulder smoothing around dry road edges.
    let blend_r = i32::from(req.blend_radius.min(10));
    if blend_r > 0 {
        for lz in 0..d {
            for lx in 0..w {
                if road_cols.contains_key(&(lx, lz))
                    || shoulder_lamps.contains(&(lx, lz))
                    || wd_at(lx, lz) > 0
                {
                    continue;
                }
                let mut best: Option<(i32, i32)> = None;
                for dz in -blend_r..=blend_r {
                    for dx in -blend_r..=blend_r {
                        if let Some(&(_, ry, is_br, _, _)) = road_cols.get(&(lx + dx, lz + dz))
                            && !is_br
                        {
                            let d2 = dx * dx + dz * dz;
                            if best.is_none_or(|(bd2, _)| d2 < bd2) {
                                best = Some((d2, ry));
                            }
                        }
                    }
                }
                let Some((d2, ry)) = best else {
                    continue;
                };
                let dist = (d2 as f64).sqrt();
                if dist > f64::from(blend_r) + 0.35 {
                    continue;
                }
                let terr = surf_at(lx, lz);
                apply_radial_terrain_column(
                    &mut blocks,
                    lx,
                    lz,
                    dist,
                    blend_r,
                    terr,
                    ry,
                    max_rel_y.max(terr + 3),
                    &req.biome_hint,
                    req.style,
                    &pal,
                    seed,
                    noise.cell_hash(lx, 0, lz),
                );
            }
        }
    }

    // Optional traveler/cartographer villager along route (`spawn_npcs`).
    if req.spawn_npcs && n >= 3 {
        let (vx, vz) = path[n / 2];
        let vy = smooth_y[n / 2];
        if blocks.get(&[vx, vy, vz]).is_some_and(|b| b.block_id != "minecraft:water") {
            living.entities.push(PlannedEntity {
                pos: [vx, vy + 1, vz],
                kind: LivingEntityKind::Villager,
                facing: CardinalFacing::South,
                detail: "cartographer".into(),
                sub_detail: "plains".into(),
                is_decor: false,
            });
        }
    }

    living.apply_filters(req.living_decor, req.spawn_npcs);
    crate::generator::prune_unsupported_floating_blocks_with_terrain(
        &mut blocks,
        Some((w, d)),
        Some(terrain_offsets),
        Some(water_depths),
        false,
    );
    let _ = prune_map_unsupported;
    resolve_connections_and_shapes(&mut blocks);
    let _ = make_slab; // keep clean if unused in style branch

    let mut dummy_spec = parse_description_with_biome(
        BuildCategory::Engineering,
        req.style,
        "road connector",
        bw.clamp(7, 96),
        bd.clamp(7, 96),
        16,
        NoiseLevel::Balanced,
        seed,
        "",
    );
    dummy_spec.living_decor = req.living_decor;
    dummy_spec.spawn_npcs = req.spawn_npcs;
    dummy_spec.blend_radius = req.blend_radius;

    GeneratedBlueprint {
        spec: dummy_spec,
        origin: box_origin,
        width: bw,
        depth: bd,
        height: ((max_rel_y - min_rel_y).clamp(8, 48)) as u16,
        blocks,
        door_pos: Some([sx, smooth_y[0] + 1, sz]),
        living,
    }
}
