use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use crate::gen_common::{
    GeneratedBlueprint, LivingEntityKind, LivingPlan, PlannedBlock, PlannedEntity, PlannedSign,
    SignPurpose, clamp_i32, resolve_connections_and_shapes,
};
use crate::gen_interior::wood_prefix_for_palette;
use crate::generator::apply_radial_terrain_column;
use crate::model::{
    BuildCategory, BuildStyle, CardinalFacing, MaterialPalette, NoiseLevel, RoadConnectRequest,
};
use crate::noise::NoiseField;
use crate::parser::{biome_style_suggestion, parse_description_with_biome};

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

struct RoadStyleTheme {
    center_primary: String,
    center_secondary: String,
    center_tertiary: String,
    center_stripe: Option<String>,
    stripe_period: usize,
    curb: String,
    foundation: String,
    bridge_deck: String,
    bridge_edge: String,
    fence: String,
    wall_trim: String,
    bridge_light: String,
    sign_wood: String,
    custom_mix_paving: bool,
    custom_mix_foundation: bool,
}

fn infer_style_from_palette_hint(pal: &MaterialPalette) -> Option<BuildStyle> {
    let joined = format!(
        "{} {} {} {}",
        pal.name.to_lowercase(),
        pal.paving,
        pal.wall,
        pal.foundation
    );
    if joined.contains("sandstone") || joined.contains("terracotta") || joined.contains("acacia") {
        Some(BuildStyle::Desert)
    } else if joined.contains("concrete") || joined.contains("iron_bars") {
        Some(BuildStyle::Modern)
    } else if joined.contains("quartz") || joined.contains("calcite") && !joined.contains("япон") {
        Some(BuildStyle::Classical)
    } else if joined.contains("cherry") || joined.contains("bamboo") || joined.contains("япон") || joined.contains("сакур") {
        Some(BuildStyle::Japanese)
    } else if joined.contains("blackstone") || joined.contains("nether") || joined.contains("готик") || joined.contains("магич") {
        Some(BuildStyle::Gothic)
    } else if joined.contains("copper") || joined.contains("стимпанк") {
        Some(BuildStyle::Steampunk)
    } else if joined.contains("tuff") || joined.contains("spruce") || joined.contains("сканд") || joined.contains("снег") {
        Some(BuildStyle::Nordic)
    } else if joined.contains("bricks") || joined.contains("коттедж") || joined.contains("берёз") {
        Some(BuildStyle::Cottage)
    } else {
        None
    }
}

fn resolve_road_style(req: &RoadConnectRequest) -> BuildStyle {
    if req.style != BuildStyle::Auto {
        return req.style;
    }
    if let Some(biome_style) = biome_style_suggestion(&req.biome_hint) {
        return biome_style;
    }
    if req.palette != MaterialPalette::preset(0)
        && let Some(pal_style) = infer_style_from_palette_hint(&req.palette)
    {
        return pal_style;
    }
    BuildStyle::Medieval
}

fn base_road_theme(style: BuildStyle, biome_hint: &str) -> RoadStyleTheme {
    let biome_lower = biome_hint.to_lowercase();
    match style {
        BuildStyle::Desert => {
            if biome_lower.contains("badlands") {
                RoadStyleTheme {
                    center_primary: "minecraft:smooth_red_sandstone".into(),
                    center_secondary: "minecraft:cut_red_sandstone".into(),
                    center_tertiary: "minecraft:terracotta".into(),
                    center_stripe: None,
                    stripe_period: 4,
                    curb: "minecraft:cut_red_sandstone".into(),
                    foundation: "minecraft:red_sandstone".into(),
                    bridge_deck: "minecraft:smooth_red_sandstone".into(),
                    bridge_edge: "minecraft:terracotta".into(),
                    fence: "minecraft:acacia_fence".into(),
                    wall_trim: "minecraft:red_sandstone_wall".into(),
                    bridge_light: "minecraft:lantern".into(),
                    sign_wood: "acacia".into(),
                    custom_mix_paving: false,
                    custom_mix_foundation: false,
                }
            } else {
                RoadStyleTheme {
                    center_primary: "minecraft:smooth_sandstone".into(),
                    center_secondary: "minecraft:cut_sandstone".into(),
                    center_tertiary: "minecraft:sandstone".into(),
                    center_stripe: None,
                    stripe_period: 4,
                    curb: "minecraft:cut_sandstone".into(),
                    foundation: "minecraft:sandstone".into(),
                    bridge_deck: "minecraft:smooth_sandstone".into(),
                    bridge_edge: "minecraft:cut_sandstone".into(),
                    fence: "minecraft:birch_fence".into(),
                    wall_trim: "minecraft:sandstone_wall".into(),
                    bridge_light: "minecraft:lantern".into(),
                    sign_wood: "acacia".into(),
                    custom_mix_paving: false,
                    custom_mix_foundation: false,
                }
            }
        }
        BuildStyle::Modern => RoadStyleTheme {
            center_primary: "minecraft:gray_concrete".into(),
            center_secondary: "minecraft:black_concrete".into(),
            center_tertiary: "minecraft:gray_concrete".into(),
            center_stripe: Some("minecraft:white_concrete".into()),
            stripe_period: 4,
            curb: "minecraft:smooth_stone".into(),
            foundation: "minecraft:polished_deepslate".into(),
            bridge_deck: "minecraft:smooth_stone".into(),
            bridge_edge: "minecraft:gray_concrete".into(),
            fence: "minecraft:iron_bars".into(),
            wall_trim: "minecraft:polished_deepslate_wall".into(),
            bridge_light: "minecraft:sea_lantern".into(),
            sign_wood: "pale_oak".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Japanese => RoadStyleTheme {
            center_primary: "minecraft:andesite".into(),
            center_secondary: "minecraft:polished_andesite".into(),
            center_tertiary: "minecraft:gravel".into(),
            center_stripe: Some("minecraft:polished_andesite".into()),
            stripe_period: 3,
            curb: "minecraft:stone_bricks".into(),
            foundation: "minecraft:stone_bricks".into(),
            bridge_deck: "minecraft:dark_oak_planks".into(),
            bridge_edge: "minecraft:crimson_planks".into(),
            fence: "minecraft:bamboo_fence".into(),
            wall_trim: "minecraft:polished_deepslate_wall".into(),
            bridge_light: "minecraft:lantern".into(),
            sign_wood: "bamboo".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Gothic => RoadStyleTheme {
            center_primary: "minecraft:deepslate_bricks".into(),
            center_secondary: "minecraft:cracked_deepslate_bricks".into(),
            center_tertiary: "minecraft:cobbled_deepslate".into(),
            center_stripe: None,
            stripe_period: 4,
            curb: "minecraft:polished_blackstone_bricks".into(),
            foundation: "minecraft:cobbled_deepslate".into(),
            bridge_deck: "minecraft:deepslate_tiles".into(),
            bridge_edge: "minecraft:polished_blackstone_bricks".into(),
            fence: "minecraft:nether_brick_fence".into(),
            wall_trim: "minecraft:deepslate_brick_wall".into(),
            bridge_light: "minecraft:soul_lantern".into(),
            sign_wood: "dark_oak".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Nordic => RoadStyleTheme {
            center_primary: "minecraft:cobbled_deepslate".into(),
            center_secondary: "minecraft:tuff_bricks".into(),
            center_tertiary: "minecraft:gravel".into(),
            center_stripe: None,
            stripe_period: 4,
            curb: "minecraft:tuff_bricks".into(),
            foundation: "minecraft:tuff_bricks".into(),
            bridge_deck: "minecraft:spruce_planks".into(),
            bridge_edge: "minecraft:spruce_log".into(),
            fence: "minecraft:spruce_fence".into(),
            wall_trim: "minecraft:tuff_brick_wall".into(),
            bridge_light: "minecraft:lantern".into(),
            sign_wood: "spruce".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Steampunk => RoadStyleTheme {
            center_primary: "minecraft:mud_bricks".into(),
            center_secondary: "minecraft:polished_tuff".into(),
            center_tertiary: "minecraft:tuff_bricks".into(),
            center_stripe: Some("minecraft:cut_copper".into()),
            stripe_period: 5,
            curb: "minecraft:oxidized_cut_copper".into(),
            foundation: "minecraft:tuff_bricks".into(),
            bridge_deck: "minecraft:dark_oak_planks".into(),
            bridge_edge: "minecraft:exposed_cut_copper".into(),
            fence: "minecraft:iron_bars".into(),
            wall_trim: "minecraft:brick_wall".into(),
            bridge_light: "minecraft:copper_lantern".into(),
            sign_wood: "dark_oak".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Classical => RoadStyleTheme {
            center_primary: "minecraft:quartz_bricks".into(),
            center_secondary: "minecraft:calcite".into(),
            center_tertiary: "minecraft:smooth_sandstone".into(),
            center_stripe: Some("minecraft:chiseled_quartz_block".into()),
            stripe_period: 4,
            curb: "minecraft:chiseled_quartz_block".into(),
            foundation: "minecraft:smooth_stone".into(),
            bridge_deck: "minecraft:smooth_quartz".into(),
            bridge_edge: "minecraft:quartz_pillar".into(),
            fence: "minecraft:pale_oak_fence".into(),
            wall_trim: "minecraft:sandstone_wall".into(),
            bridge_light: "minecraft:lantern".into(),
            sign_wood: "pale_oak".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        BuildStyle::Cottage => RoadStyleTheme {
            center_primary: "minecraft:packed_mud".into(),
            center_secondary: "minecraft:coarse_dirt".into(),
            center_tertiary: "minecraft:mossy_cobblestone".into(),
            center_stripe: None,
            stripe_period: 4,
            curb: "minecraft:bricks".into(),
            foundation: "minecraft:cobblestone".into(),
            bridge_deck: "minecraft:oak_planks".into(),
            bridge_edge: "minecraft:cobblestone".into(),
            fence: "minecraft:oak_fence".into(),
            wall_trim: "minecraft:cobblestone_wall".into(),
            bridge_light: "minecraft:lantern".into(),
            sign_wood: "oak".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
        _ => RoadStyleTheme {
            center_primary: "minecraft:cobblestone".into(),
            center_secondary: "minecraft:mossy_cobblestone".into(),
            center_tertiary: "minecraft:gravel".into(),
            center_stripe: None,
            stripe_period: 4,
            curb: "minecraft:mossy_stone_bricks".into(),
            foundation: "minecraft:stone_bricks".into(),
            bridge_deck: "minecraft:spruce_planks".into(),
            bridge_edge: "minecraft:stone_bricks".into(),
            fence: "minecraft:spruce_fence".into(),
            wall_trim: "minecraft:stone_brick_wall".into(),
            bridge_light: "minecraft:lantern".into(),
            sign_wood: "spruce".into(),
            custom_mix_paving: false,
            custom_mix_foundation: false,
        },
    }
}

fn build_road_theme_and_palette(
    req: &RoadConnectRequest,
    effective_style: BuildStyle,
) -> (RoadStyleTheme, MaterialPalette) {
    let mut theme = base_road_theme(effective_style, &req.biome_hint);
    let auto_default = MaterialPalette::preset(0);
    let style_default = MaterialPalette::preset_for_style(effective_style);
    let has_explicit_palette = req.palette != auto_default && req.palette != style_default;

    let mut pal = style_default.clone();
    pal.paving = theme.center_primary.clone();
    pal.foundation = theme.foundation.clone();
    pal.trim_wall = theme.curb.clone();
    pal.wall_trim = theme.wall_trim.clone();
    pal.fence = theme.fence.clone();
    pal.pillar = theme.bridge_deck.clone();
    pal.light = theme.bridge_light.clone();

    if has_explicit_palette {
        if req.palette.paving != style_default.paving {
            theme.center_primary = req.palette.paving.clone();
            theme.center_secondary = req.palette.paving.clone();
            theme.center_stripe = None;
            pal.paving = req.palette.paving.clone();
        }
        if req.palette.foundation != style_default.foundation {
            theme.foundation = req.palette.foundation.clone();
            theme.bridge_edge = req.palette.foundation.clone();
            pal.foundation = req.palette.foundation.clone();
        }
        if req.palette.wall_accent != style_default.wall_accent {
            theme.curb = req.palette.wall_accent.clone();
            pal.trim_wall = req.palette.wall_accent.clone();
        }
        if req.palette.trim_wall != style_default.trim_wall {
            theme.wall_trim = req.palette.trim_wall.clone();
            pal.wall_trim = req.palette.trim_wall.clone();
        }
        if req.palette.fence != style_default.fence {
            theme.fence = req.palette.fence.clone();
            pal.fence = req.palette.fence.clone();
        }
        if req.palette.floor != style_default.floor {
            theme.bridge_deck = req.palette.floor.clone();
            pal.pillar = req.palette.floor.clone();
        }
        if req.palette.light != style_default.light {
            theme.bridge_light = req.palette.light.clone();
            pal.light = req.palette.light.clone();
        }
        if req.palette.door != style_default.door || req.palette.fence != style_default.fence {
            theme.sign_wood = wood_prefix_for_palette(&req.palette).to_owned();
        }
        if req.palette.has_extra(MaterialPalette::SLOT_PAVING) {
            theme.custom_mix_paving = true;
            theme.center_stripe = None;
        }
        if req.palette.has_extra(MaterialPalette::SLOT_FOUNDATION) {
            theme.custom_mix_foundation = true;
        }
        pal.mix_mode = req.palette.mix_mode;
        pal.extra_lists = req.palette.extra_lists.clone();
    }

    (theme, pal)
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
    let effective_style = resolve_road_style(req);
    let (rtheme, pal) = build_road_theme_and_palette(req, effective_style);
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

    let style_center_blk = |x: i32, y: i32, z: i32, h: f64, is_centerline: bool, p_idx: usize| -> PlannedBlock {
        if rtheme.custom_mix_paving {
            return PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, x, y, z));
        }
        if is_centerline
            && let Some(ref stripe) = rtheme.center_stripe
            && (p_idx % rtheme.stripe_period) < if rtheme.stripe_period == 4 && effective_style == BuildStyle::Modern { 2 } else { 1 }
        {
            return PlannedBlock::simple(stripe);
        }
        if h > 0.74 {
            PlannedBlock::simple(&rtheme.center_secondary)
        } else if h < 0.20 {
            PlannedBlock::simple(&rtheme.center_tertiary)
        } else {
            PlannedBlock::simple(&rtheme.center_primary)
        }
    };

    for (&(x, z), &(d2, ry, is_bridge, is_edge, p_idx)) in &road_cols {
        let terr = surf_at(x, z);
        let wd = wd_at(x, z);
        max_rel_y = max_rel_y.max(ry + 6).max(terr + 4);
        min_rel_y = min_rel_y.min(terr - wd - 2);

        let h = noise.cell_hash(x, 0, z);
        if is_bridge {
            // Arched bridge deck + style masonry/timber piers over water + parapet rails!
            let deck_blk = if is_edge {
                PlannedBlock::simple(&rtheme.bridge_edge)
            } else {
                PlannedBlock::simple(&rtheme.bridge_deck)
            };
            blocks.insert([x, ry, z], deck_blk);
            for ay in (ry + 1)..=(ry + 4) {
                blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
            }
            if is_edge {
                if p_idx % 4 == 0 {
                    let bot = (terr - wd - 1).clamp(-26, ry - 1);
                    for py in bot..ry {
                        let pier_blk = if rtheme.custom_mix_foundation {
                            pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, py, z)
                        } else {
                            rtheme.foundation.clone()
                        };
                        blocks.insert([x, py, z], PlannedBlock::simple(pier_blk));
                    }
                    blocks.insert([x, ry + 1, z], PlannedBlock::simple(&rtheme.wall_trim));
                    if req.living_decor {
                        let light_pb = if crate::gen_common::is_lantern(&rtheme.bridge_light) {
                            PlannedBlock::with_props(&rtheme.bridge_light, vec![("hanging", "false")])
                        } else {
                            PlannedBlock::simple(&rtheme.bridge_light)
                        };
                        blocks.insert([x, ry + 2, z], light_pb);
                    }
                } else {
                    blocks.insert([x, ry + 1, z], PlannedBlock::simple(&rtheme.fence));
                }
            }
        } else {
            // Terrain-conforming road surface + solid skirt if elevated + clean headroom!
            let lo_fill = (terr - wd - 1).clamp(-26, ry - 1);
            for fy in lo_fill..ry {
                let fill_blk = if rtheme.custom_mix_foundation {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, fy, z)
                } else {
                    rtheme.foundation.clone()
                };
                blocks.insert([x, fy, z], PlannedBlock::simple(fill_blk));
            }
            let surf_blk = if is_edge {
                PlannedBlock::simple(&rtheme.curb)
            } else {
                style_center_blk(x, ry, z, h, d2 == 0, p_idx)
            };
            blocks.insert([x, ry, z], surf_blk);
            for ay in (ry + 1)..=(terr.max(ry) + 4) {
                blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Add automated night streetlights along dry road shoulders every 10 blocks + style-matched waypost signs at endpoints.
    let mut shoulder_lamps: BTreeSet<(i32, i32)> = BTreeSet::new();
    let offset_shoulder = (half_w.round() as i32) + 1;
    let sign_block_id = format!("minecraft:{}_sign", rtheme.sign_wood);
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
            blocks.insert([sx_sh, fy, sz_sh], PlannedBlock::simple(&rtheme.foundation));
        }
        if is_endpoint && req.living_decor {
            blocks.insert([sx_sh, post_y + 1, sz_sh], PlannedBlock::simple(&rtheme.fence));
            blocks.insert(
                [sx_sh, post_y + 2, sz_sh],
                PlannedBlock::with_props(sign_block_id.clone(), vec![("rotation", "0"), ("waterlogged", "false")]),
            );
            living.signs.push(PlannedSign {
                pos: [sx_sh, post_y + 2, sz_sh],
                purpose: SignPurpose::Waypost,
                custom_en: None,
                custom_ru: None,
            });
            shoulder_lamps.insert((sx_sh, sz_sh));
        } else if req.living_decor {
            blocks.insert([sx_sh, post_y + 1, sz_sh], PlannedBlock::simple(&rtheme.wall_trim));
            blocks.insert([sx_sh, post_y + 2, sz_sh], PlannedBlock::simple(&rtheme.fence));
            blocks.insert([sx_sh, post_y + 3, sz_sh], PlannedBlock::simple(&rtheme.fence));
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
                    effective_style,
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
    resolve_connections_and_shapes(&mut blocks);

    let mut dummy_spec = parse_description_with_biome(
        BuildCategory::Engineering,
        effective_style,
        "road connector",
        bw.clamp(7, 96),
        bd.clamp(7, 96),
        16,
        NoiseLevel::Balanced,
        seed,
        &req.biome_hint,
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
