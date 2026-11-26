use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, is_lantern, make_beam_block_at,
    make_frame_block_at, make_leaves, make_pillar_block_at, make_slab, make_stair,
    prune_map_unsupported, resolve_connections_and_shapes, terrain_rel_at,
};
use crate::model::{BuildRequest, BuildStyle, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, BridgeVariant};

/// Rotated horizontal frame aligned along the longer axis of the bridge selection.
#[derive(Clone, Copy)]
struct BridgeAxes {
    grid: LocalGrid,
    span_along_u: bool,
    cross_len: i32,
    along_len: i32,
    height: i32,
}

impl BridgeAxes {
    fn new(grid: LocalGrid, height: u16) -> Self {
        let span_along_u = grid.w > grid.d + 4;
        let (cross_len, along_len) = if span_along_u {
            (grid.d, grid.w)
        } else {
            (grid.w, grid.d)
        };
        Self {
            grid,
            span_along_u,
            cross_len,
            along_len,
            height: i32::from(height),
        }
    }

    fn uv(&self, cross: i32, along: i32) -> (i32, i32) {
        if self.span_along_u {
            (along, cross)
        } else {
            (cross, along)
        }
    }

    fn in_bounds(&self, cross: i32, along: i32, y: i32) -> bool {
        let (u, v) = self.uv(cross, along);
        self.grid.in_bounds(u, v) && y >= -6 && y < self.height
    }

    fn set(
        &self,
        blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
        cross: i32,
        along: i32,
        y: i32,
        block: PlannedBlock,
    ) {
        if !self.in_bounds(cross, along, y) {
            return;
        }
        let (u, v) = self.uv(cross, along);
        let [lx, lz] = self.grid.to_local_xz(u, v);
        blocks.insert([lx, y, lz], block);
    }

    fn terr(&self, terrain_offsets: Option<&[i32]>, cross: i32, along: i32) -> i32 {
        let (u, v) = self.uv(cross, along);
        terrain_rel_at(terrain_offsets, self.grid, u, v)
    }

    fn dir_along_fwd(&self) -> &'static str {
        if self.span_along_u {
            self.grid.dir_right()
        } else {
            self.grid.dir_front()
        }
    }

    fn dir_along_back(&self) -> &'static str {
        if self.span_along_u {
            self.grid.dir_left()
        } else {
            self.grid.dir_back()
        }
    }

    fn dir_cross_pos(&self) -> &'static str {
        if self.span_along_u {
            self.grid.dir_front()
        } else {
            self.grid.dir_right()
        }
    }

    fn dir_cross_neg(&self) -> &'static str {
        if self.span_along_u {
            self.grid.dir_back()
        } else {
            self.grid.dir_left()
        }
    }

    fn axis_along(&self) -> &'static str {
        if self.span_along_u {
            self.grid.axis_u()
        } else {
            self.grid.axis_v()
        }
    }

    fn axis_cross(&self) -> &'static str {
        if self.span_along_u {
            self.grid.axis_v()
        } else {
            self.grid.axis_u()
        }
    }
}

fn make_bridge_light(id: String, hanging: bool) -> PlannedBlock {
    if is_lantern(&id) {
        PlannedBlock::with_props(id, vec![("hanging", if hanging { "true" } else { "false" })])
    } else if id == "minecraft:end_rod" {
        PlannedBlock::with_props(id, vec![("facing", if hanging { "down" } else { "up" })])
    } else {
        PlannedBlock::simple(id)
    }
}

fn pick_stair_for_slot(
    pal: &MaterialPalette,
    slot: usize,
    seed: u64,
    u: i32,
    y: i32,
    v: i32,
    facing: &'static str,
    half: &'static str,
) -> PlannedBlock {
    let blk_id = pal.pick(slot, seed, u, y, v);
    let fallback_slot = if slot == MaterialPalette::SLOT_ROOF {
        MaterialPalette::SLOT_ROOF
    } else {
        MaterialPalette::SLOT_STAIR_BLOCK
    };
    let stair_id = MaterialPalette::resolve_stair_id(&blk_id)
        .or_else(|| {
            MaterialPalette::resolve_stair_id(&pal.pick(
                MaterialPalette::SLOT_FOUNDATION,
                seed,
                u,
                y,
                v,
            ))
        })
        .unwrap_or_else(|| pal.pick_stair(fallback_slot, seed, u, y, v));
    make_stair(&stair_id, facing, half)
}

fn sample_bank_terr(
    ax: BridgeAxes,
    terrain_offsets: Option<&[i32]>,
    c0: i32,
    c1: i32,
    along: i32,
) -> Option<i32> {
    let offs = terrain_offsets?;
    let lo = (c0 + 1).min(c1);
    let hi = (c1 - 1).max(lo);
    let mut ys = Vec::with_capacity((hi - lo + 1) as usize);
    for c in lo..=hi {
        ys.push(ax.terr(Some(offs), c, along).clamp(-5, 6));
    }
    if ys.is_empty() {
        ys.push(ax.terr(Some(offs), (c0 + c1) / 2, along).clamp(-5, 6));
    }
    ys.sort_unstable();
    Some(ys[ys.len() / 2])
}

fn variant_headroom_reserve(variant: BridgeVariant) -> i32 {
    match variant {
        BridgeVariant::FortifiedGate
        | BridgeVariant::CoveredTimber
        | BridgeVariant::Aqueduct
        | BridgeVariant::Suspension => 6,
        BridgeVariant::TaikoMoon
        | BridgeVariant::PontoonPier
        | BridgeVariant::MasonryArch
        | BridgeVariant::GothicRibbedArch
        | BridgeVariant::GrandSingleArch => 5,
    }
}

fn compute_walk_top_profile(deck_profile: &[i32]) -> Vec<i32> {
    let n = deck_profile.len();
    let mut top = Vec::with_capacity(n);
    for i in 0..n {
        let dy = deck_profile[i];
        let prev = if i > 0 { deck_profile[i - 1] } else { dy };
        let next = if i + 1 < n { deck_profile[i + 1] } else { dy };
        let has_step = next > dy || prev > dy;
        top.push(dy + i32::from(has_step));
    }
    top
}

fn max_walk_window(walk_top: &[i32], center: i32, radius: i32) -> i32 {
    let lo = (center - radius).max(0) as usize;
    let hi = (center + radius).min(walk_top.len() as i32 - 1) as usize;
    walk_top[lo..=hi].iter().copied().max().unwrap_or(0)
}

/// Computes bridge cross-section bounds `c0..=c1` utilizing large requested widths proportionally.
fn compute_bridge_width(cross_len: i32, variant: BridgeVariant, rng: &mut SplitMix64) -> (i32, i32) {
    let mid = cross_len / 2;
    let raw_w = if cross_len <= 9 {
        (cross_len - 2).max(3)
    } else if cross_len <= 15 {
        let lo = (cross_len - 4).max(5);
        let hi = (cross_len - 2).min(13);
        rng.range_i32(lo, hi)
    } else {
        let lo = match variant {
            BridgeVariant::PontoonPier | BridgeVariant::CoveredTimber => (cross_len - 6).max(9),
            BridgeVariant::Aqueduct => (cross_len - 6).max(11),
            _ => (cross_len - 4).max(11),
        };
        let hi = (cross_len - 2).min(17);
        rng.range_i32(lo.min(hi), hi)
    };
    let odd_w = if raw_w % 2 == 0 { raw_w - 1 } else { raw_w }.max(3);
    let hw = (odd_w / 2).min(mid);
    let c0 = (mid - hw).max(0);
    let c1 = (mid + hw).min(cross_len - 1);
    (c0, c1)
}

/// Builds balanced multi-span intervals `[start, end]` leaving space at both ends for shore abutments.
fn compute_arch_spans(
    along_len: i32,
    variant: BridgeVariant,
    rng: &mut SplitMix64,
) -> Vec<(i32, i32)> {
    let end_pad = match variant {
        BridgeVariant::GrandSingleArch => (along_len / 9).clamp(3, 5),
        BridgeVariant::Aqueduct => 2,
        BridgeVariant::FortifiedGate => (along_len / 8).clamp(3, 6),
        BridgeVariant::TaikoMoon => (along_len / 8).clamp(2, 5),
        _ => (along_len / 10).clamp(2, 4),
    };
    let s0 = end_pad;
    let s1 = (along_len - 1 - end_pad).max(s0 + 4);
    let usable = (s1 - s0 + 1).max(5);

    let max_spans = (usable / 7).clamp(1, 6);
    let desired_spans = match variant {
        BridgeVariant::GrandSingleArch | BridgeVariant::TaikoMoon => 1,
        BridgeVariant::CoveredTimber => {
            if usable >= 18 && rng.next_f64() < 0.55 { 2 } else { 1 }
        }
        BridgeVariant::Aqueduct => (usable / 7).clamp(2, max_spans),
        BridgeVariant::Suspension => if usable >= 21 { 3 } else { 1 },
        BridgeVariant::PontoonPier => (usable / 5).clamp(2, 7),
        BridgeVariant::MasonryArch | BridgeVariant::GothicRibbedArch | BridgeVariant::FortifiedGate => {
            let min_sp = if usable >= 22 { 2 } else { 1 };
            let hi_sp = max_spans.min(5);
            rng.range_i32(min_sp.min(hi_sp), hi_sp)
        }
    };

    if desired_spans <= 1 {
        return vec![(s0, s1)];
    }

    let pier_thick = if variant == BridgeVariant::PontoonPier {
        1
    } else if usable >= 32 {
        3
    } else {
        2
    };
    let total_piers = (desired_spans - 1) * pier_thick;
    let open_total = (usable - total_piers).max(desired_spans * 4);
    let base_span = open_total / desired_spans;
    let extra = open_total % desired_spans;

    let mut spans = Vec::with_capacity(desired_spans as usize);
    let mut cur = s0;
    for i in 0..desired_spans {
        let len = base_span + if i < extra { 1 } else { 0 };
        let a_end = (cur + len - 1).min(s1);
        spans.push((cur, a_end));
        cur = a_end + pier_thick + 1;
        if cur >= s1 {
            break;
        }
    }
    if spans.is_empty() {
        spans.push((s0, s1));
    }
    spans
}

/// Computes deck elevation curve `deck_y[along]` anchored cleanly to each bank's ground level.
fn compute_deck_profile(
    ax: BridgeAxes,
    variant: BridgeVariant,
    spans: &[(i32, i32)],
    c0: i32,
    c1: i32,
    terrain_offsets: Option<&[i32]>,
    rng: &mut SplitMix64,
) -> Vec<i32> {
    let along_len = ax.along_len;
    let reserve = variant_headroom_reserve(variant);
    let max_deck_ceiling = (ax.height - reserve).max(1).min(ax.height - 3).max(-2);

    let raw_a = sample_bank_terr(ax, terrain_offsets, c0, c1, 0).unwrap_or(0);
    let raw_b = sample_bank_terr(ax, terrain_offsets, c0, c1, along_len - 1).unwrap_or(0);
    let mut end_y_a = raw_a.clamp(-4, max_deck_ceiling);
    let mut end_y_b = raw_b.clamp(-4, max_deck_ceiling);
    let max_diff = (along_len - 1).max(0);
    if (end_y_b - end_y_a).abs() > max_diff {
        let mid_end = (end_y_a + end_y_b) / 2;
        let half_d = max_diff / 2;
        end_y_a = end_y_a.clamp(mid_end - half_d, mid_end + half_d);
        end_y_b = end_y_b.clamp(mid_end - half_d, mid_end + half_d);
    }
    let bank_hi = end_y_a.max(end_y_b);

    let base_high = match variant {
        BridgeVariant::PontoonPier => (bank_hi + 1).clamp(bank_hi, (bank_hi + 2).min(max_deck_ceiling)),
        BridgeVariant::Aqueduct => {
            (bank_hi + ((ax.height * 22) / 100).clamp(2, 5)).min(max_deck_ceiling)
        }
        BridgeVariant::TaikoMoon => (bank_hi + 1).min(max_deck_ceiling),
        BridgeVariant::GrandSingleArch => {
            let lo = (bank_hi + 2).min(max_deck_ceiling);
            let hi = (bank_hi + 5).clamp(lo, max_deck_ceiling);
            rng.range_i32(lo, hi)
        }
        BridgeVariant::Suspension => {
            (bank_hi + ((ax.height * 22) / 100).clamp(2, 5)).min(max_deck_ceiling)
        }
        BridgeVariant::CoveredTimber => {
            let lo = (bank_hi + 1).min(max_deck_ceiling);
            let hi = (bank_hi + 3).clamp(lo, max_deck_ceiling);
            rng.range_i32(lo, hi)
        }
        BridgeVariant::MasonryArch | BridgeVariant::GothicRibbedArch | BridgeVariant::FortifiedGate => {
            let lo = (bank_hi + 2).min(max_deck_ceiling);
            let hi = (bank_hi + 4).clamp(lo, max_deck_ceiling);
            rng.range_i32(lo, hi)
        }
    };

    let headroom_left = (max_deck_ceiling - base_high).max(0);
    let camber = match variant {
        BridgeVariant::PontoonPier | BridgeVariant::Aqueduct => 0,
        BridgeVariant::TaikoMoon => headroom_left.min(6).max(if headroom_left >= 3 { 3 } else { headroom_left }),
        BridgeVariant::GrandSingleArch => headroom_left.min(4),
        BridgeVariant::Suspension => headroom_left.min((ax.height / 7).clamp(1, 3)),
        BridgeVariant::CoveredTimber => headroom_left.min(rng.range_i32(1, 2)),
        BridgeVariant::MasonryArch | BridgeVariant::GothicRibbedArch | BridgeVariant::FortifiedGate => {
            if headroom_left > 0 && rng.next_f64() < 0.68 {
                rng.range_i32(1, headroom_left.min(3))
            } else {
                0
            }
        }
    };

    let span_start = spans.first().map_or(2, |s| s.0);
    let span_end = spans.last().map_or(along_len - 3, |s| s.1);
    let flat_ends = along_len >= 9 && (end_y_a - end_y_b).abs() <= along_len - 3;
    let mut profile = vec![base_high; along_len as usize];

    for along in 0..along_len {
        let mut y = if along == 0 {
            end_y_a
        } else if along == along_len - 1 {
            end_y_b
        } else if flat_ends && along == 1 {
            end_y_a
        } else if flat_ends && along == along_len - 2 {
            end_y_b
        } else if variant == BridgeVariant::TaikoMoon || camber >= 3 {
            let ramp0 = if flat_ends { 1 } else { 0 };
            let ramp1 = if flat_ends { along_len - 2 } else { along_len - 1 };
            let denom = (ramp1 - ramp0).max(1) as f64;
            let t = ((along - ramp0) as f64 / denom).clamp(0.0, 1.0);
            let edge_y = (end_y_a as f64) * (1.0 - t) + (end_y_b as f64) * t;
            let arch_curve = (std::f64::consts::PI * t).sin().powf(0.88);
            let target_peak = (base_high + camber).min(max_deck_ceiling);
            (edge_y + (target_peak as f64 - edge_y) * arch_curve).round() as i32
        } else if along < span_start {
            let start_i = if flat_ends { 1 } else { 0 };
            let d = (span_start - along) as f64 / (span_start - start_i).max(1) as f64;
            let ramp_y = (base_high as f64 - d * (base_high - end_y_a).max(0) as f64).round() as i32;
            ramp_y.max(end_y_a)
        } else if along > span_end {
            let end_i = if flat_ends { along_len - 2 } else { along_len - 1 };
            let rem = (end_i - span_end).max(1);
            let d = (along - span_end) as f64 / rem as f64;
            let ramp_y = (base_high as f64 - d * (base_high - end_y_b).max(0) as f64).round() as i32;
            ramp_y.max(end_y_b)
        } else {
            let t = (along - span_start) as f64 / (span_end - span_start).max(1) as f64;
            let hump = (std::f64::consts::PI * t).sin();
            base_high + (hump * f64::from(camber)).round() as i32
        };

        let (dist_a, dist_b) = if flat_ends {
            ((along - 1).max(0), (along_len - 2 - along).max(0))
        } else {
            (along, along_len - 1 - along)
        };
        let lo_env = (end_y_a - dist_a).max(end_y_b - dist_b).max(-5);
        let hi_env = (end_y_a + dist_a).min(end_y_b + dist_b).min(max_deck_ceiling);
        y = y.clamp(lo_env.min(hi_env), hi_env);
        profile[along as usize] = y;
    }

    profile[0] = end_y_a;
    if let Some(last) = profile.last_mut() {
        *last = end_y_b;
    }
    for i in 1..profile.len() {
        let prev = profile[i - 1];
        if profile[i] > prev + 1 {
            profile[i] = prev + 1;
        } else if profile[i] < prev - 1 {
            profile[i] = prev - 1;
        }
    }
    for i in (0..profile.len().saturating_sub(1)).rev() {
        let next = profile[i + 1];
        if profile[i] > next + 1 {
            profile[i] = next + 1;
        } else if profile[i] < next - 1 {
            profile[i] = next - 1;
        }
    }

    profile
}

/// Computes arch soffit height for a single column `along` inside span `[s0, s1]`.
fn compute_arch_void_ceiling(
    variant: BridgeVariant,
    along: i32,
    s0: i32,
    s1: i32,
    deck_y: i32,
    base_floor_y: i32,
    spandrel_thickness: i32,
) -> i32 {
    let span = (s1 - s0).max(2) as f64;
    let t = ((along - s0) as f64 / span).clamp(0.0, 1.0);
    let shape = match variant {
        BridgeVariant::GothicRibbedArch => {
            let dist = (2.0 * t - 1.0).abs();
            (1.0 - dist).clamp(0.0, 1.0).powf(0.72)
        }
        BridgeVariant::GrandSingleArch | BridgeVariant::TaikoMoon => {
            (std::f64::consts::PI * t).sin().powf(0.78)
        }
        _ => {
            let u = 2.0 * t - 1.0;
            (1.0 - u * u).max(0.0).sqrt()
        }
    };
    let min_clear = base_floor_y + 1;
    let max_void_top = (deck_y - spandrel_thickness).max(min_clear);
    let rise = (max_void_top - min_clear).max(0) as f64;
    (min_clear as f64 + shape * rise).round() as i32
}

/// Places a decorative/structural bridge pylon or castle gatehouse turret spanning `[c0..=c1]` with guaranteed headroom.
#[allow(clippy::too_many_arguments)]
fn build_bridge_tower(
    ax: BridgeAxes,
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    seed: u64,
    c0: i32,
    c1: i32,
    along_center: i32,
    deck_profile: &[i32],
    walk_top: &[i32],
    top_y: i32,
    portal_half_w: i32,
    point_spire: bool,
) {
    let mid_c = (c0 + c1) / 2;
    let center_dy = deck_profile[along_center as usize];
    let min_dy = (-1..=1)
        .filter_map(|da| {
            let a = along_center + da;
            (0..ax.along_len).contains(&a).then(|| deck_profile[a as usize])
        })
        .min()
        .unwrap_or(center_dy);
    let max_walk = max_walk_window(walk_top, along_center, 1);
    let eff_top_y = top_y.max(max_walk + 6).min(ax.height - 2);
    let portal_top = (max_walk + 5).min(eff_top_y - 2);
    let has_arch_roof = portal_top >= max_walk + 4 && eff_top_y >= portal_top + 1;
    let pier_width_c0 = (c0 - 1).max(0);
    let pier_width_c1 = (c1 + 1).min(ax.cross_len - 1);
    let base_anchor = (min_dy - 4).max(-6);

    for da in -1..=1 {
        let a = along_center + da;
        if a < 0 || a >= ax.along_len {
            continue;
        }
        let slice_dy = deck_profile[a as usize];
        let slice_walk = walk_top[a as usize];

        for c in pier_width_c0..=pier_width_c1 {
            if c < 0 || c >= ax.cross_len || (c - mid_c).abs() <= portal_half_w {
                continue;
            }
            for y in base_anchor..=eff_top_y {
                let blk = if y < slice_dy {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, a))
                } else if y == portal_top || y == eff_top_y {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, a))
                } else if c == pier_width_c0 || c == pier_width_c1 {
                    make_frame_block_at(pal, "y", seed, c, y, a)
                } else {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, c, y, a))
                };
                ax.set(blocks, c, a, y, blk);
            }
            if da == 0 && (c == c0 || c == c1) {
                ax.set(
                    blocks,
                    c,
                    a,
                    eff_top_y + 1,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, c, eff_top_y + 1, a)),
                );
                if point_spire && eff_top_y + 2 < ax.height {
                    ax.set(
                        blocks,
                        c,
                        a,
                        eff_top_y + 2,
                        make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, c, eff_top_y + 2, a), false),
                    );
                }
            } else if da != 0 && (c == pier_width_c0 || c == pier_width_c1) {
                ax.set(
                    blocks,
                    c,
                    a,
                    eff_top_y + 1,
                    make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, c, eff_top_y + 1, a), "bottom"),
                );
            }
        }

        for c in (mid_c - portal_half_w)..=(mid_c + portal_half_w) {
            if c < c0 || c > c1 {
                continue;
            }
            let edge_dist = portal_half_w - (c - mid_c).abs();
            let local_arch_y = if edge_dist == 0 && portal_top - 1 >= max_walk + 4 {
                portal_top - 1
            } else {
                portal_top
            };
            let clear_end = if has_arch_roof { local_arch_y } else { eff_top_y + 1 };
            for y in (slice_walk + 1)..clear_end {
                ax.set(blocks, c, a, y, PlannedBlock::simple("minecraft:air"));
            }
            if has_arch_roof {
                for y in local_arch_y..=eff_top_y {
                    if y == local_arch_y {
                        if da == 0 && edge_dist == 0 && local_arch_y < portal_top {
                            let face = if c < mid_c {
                                ax.dir_cross_pos()
                            } else {
                                ax.dir_cross_neg()
                            };
                            ax.set(
                                blocks,
                                c,
                                a,
                                y,
                                pick_stair_for_slot(pal, MaterialPalette::SLOT_FOUNDATION, seed, c, y, a, face, "top"),
                            );
                        } else if da.abs() == 1 {
                            ax.set(blocks, c, a, y, PlannedBlock::simple("minecraft:iron_bars"));
                        } else {
                            ax.set(blocks, c, a, y, PlannedBlock::simple("minecraft:air"));
                        }
                    } else {
                        let is_window = da == 0 && c == mid_c && y == eff_top_y - 1 && eff_top_y - local_arch_y >= 2;
                        let blk = if is_window {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, c, y, a))
                        } else if y == eff_top_y {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, a))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, a))
                        };
                        ax.set(blocks, c, a, y, blk);
                    }
                }
                if eff_top_y + 1 < ax.height && ((c + da).abs() % 2 == 0 || point_spire) {
                    if point_spire {
                        let ridge_h = (portal_half_w - (c - mid_c).abs()).min(2) + 1;
                        for ry in 1..=ridge_h {
                            if eff_top_y + ry < ax.height {
                                ax.set(
                                    blocks,
                                    c,
                                    a,
                                    eff_top_y + ry,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, eff_top_y + ry, a)),
                                );
                            }
                        }
                    } else if (c + a).abs() % 2 == 0 {
                        ax.set(
                            blocks,
                            c,
                            a,
                            eff_top_y + 1,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, c, eff_top_y + 1, a)),
                        );
                    } else {
                        ax.set(
                            blocks,
                            c,
                            a,
                            eff_top_y + 1,
                            make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, eff_top_y + 1, a), "bottom"),
                        );
                    }
                }
            }
        }
    }
}

/// Builds a Japanese scarlet/timber Torii gate at road position `along_center` with guaranteed headroom.
#[allow(clippy::too_many_arguments)]
fn build_torii_portal(
    ax: BridgeAxes,
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    seed: u64,
    c0: i32,
    c1: i32,
    along_center: i32,
    deck_y: i32,
    walk_top: &[i32],
) {
    let mid_c = (c0 + c1) / 2;
    let col_off = ((c1 - c0) / 2 - 1).clamp(2, 4);
    let col_left = (mid_c - col_off).max(c0);
    let col_right = (mid_c + col_off).min(c1);
    let local_walk = max_walk_window(walk_top, along_center, 1);
    let gate_h = (local_walk + 6).min(ax.height - 2);
    if gate_h <= local_walk + 2 {
        return;
    }

    for col_c in [col_left, col_right] {
        ax.set(
            blocks,
            col_c,
            along_center,
            deck_y + 1,
            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, col_c, deck_y + 1, along_center)),
        );
        for y in (deck_y + 2)..=gate_h {
            ax.set(
                blocks,
                col_c,
                along_center,
                y,
                make_frame_block_at(pal, "y", seed, col_c, y, along_center),
            );
        }
    }

    if gate_h - 1 < local_walk + 4 {
        if gate_h + 1 < ax.height {
            for col_c in [col_left, col_right] {
                ax.set(
                    blocks,
                    col_c,
                    along_center,
                    gate_h + 1,
                    make_bridge_light(
                        pal.pick(MaterialPalette::SLOT_LIGHT, seed, col_c, gate_h + 1, along_center),
                        false,
                    ),
                );
            }
        }
        return;
    }

    let beam_axis = ax.axis_cross();
    for c in (col_left - 1).max(0)..=(col_right + 1).min(ax.cross_len - 1) {
        ax.set(
            blocks,
            c,
            along_center,
            gate_h - 1,
            make_beam_block_at(pal, beam_axis, seed, c, gate_h - 1, along_center),
        );
    }
    ax.set(
        blocks,
        mid_c,
        along_center,
        gate_h,
        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, mid_c, gate_h, along_center)),
    );
    if gate_h - 2 >= local_walk + 4 {
        ax.set(
            blocks,
            mid_c,
            along_center,
            gate_h - 2,
            make_bridge_light(
                pal.pick(MaterialPalette::SLOT_LIGHT, seed, mid_c, gate_h - 2, along_center),
                true,
            ),
        );
    }

    let wing_l = (col_left - 2).max(0);
    let wing_r = (col_right + 2).min(ax.cross_len - 1);
    for c in wing_l..=wing_r {
        if c == wing_l || c == wing_r {
            let face = if c == wing_l {
                ax.dir_cross_pos()
            } else {
                ax.dir_cross_neg()
            };
            ax.set(
                blocks,
                c,
                along_center,
                gate_h,
                pick_stair_for_slot(pal, MaterialPalette::SLOT_ROOF, seed, c, gate_h, along_center, face, "bottom"),
            );
            if gate_h + 1 < ax.height {
                ax.set(
                    blocks,
                    c,
                    along_center,
                    gate_h + 1,
                    make_slab(&pal.pick(MaterialPalette::SLOT_ROOF_EDGE, seed, c, gate_h + 1, along_center), "bottom"),
                );
            }
        } else {
            ax.set(
                blocks,
                c,
                along_center,
                gate_h,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, gate_h, along_center)),
            );
            if gate_h + 1 < ax.height {
                ax.set(
                    blocks,
                    c,
                    along_center,
                    gate_h + 1,
                    make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, c, gate_h + 1, along_center), "bottom"),
                );
            }
        }
    }
}

/// Builds tall paired suspension H-pylons, sweeping catenary main cables, and vertical chain suspenders.
#[allow(clippy::too_many_arguments)]
fn build_suspension_rig(
    ax: BridgeAxes,
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    seed: u64,
    c0: i32,
    c1: i32,
    deck_profile: &[i32],
    walk_top: &[i32],
    rng: &mut SplitMix64,
) {
    let along_len = ax.along_len;
    let pylon_a = (along_len / 4).clamp(4, (along_len / 2 - 4).max(4));
    let pylon_b = (along_len - 1 - pylon_a).max(pylon_a + 6).min(along_len - 4);
    let mid_along = (pylon_a + pylon_b) / 2;
    let peak_walk = walk_top.iter().copied().max().unwrap_or(5);
    let pylon_top = (ax.height - 2).max(peak_walk + 5).min(ax.height - 1);
    let sag_min = (deck_profile[mid_along as usize] + 2).min(pylon_top - 2);
    let use_copper_cable = rng.next_f64() < 0.40;
    let hanger_interval = if along_len >= 40 { 3 } else { 2 };

    for a_end in [1, along_len - 2] {
        let dy = deck_profile[a_end as usize];
        for &c in &[c0, c1] {
            for y in dy..=dy + 2 {
                ax.set(
                    blocks,
                    c,
                    a_end,
                    y,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, a_end)),
                );
            }
        }
    }

    for pylon in [pylon_a, pylon_b] {
        let dy = deck_profile[pylon as usize];
        let local_walk = max_walk_window(walk_top, pylon, 1);
        for da in -1..=0 {
            let a = (pylon + da).clamp(0, along_len - 1);
            let slice_dy = deck_profile[a as usize];
            for &c in &[c0, c1] {
                for y in (slice_dy - 4).max(-6)..=pylon_top {
                    let blk = if y <= slice_dy {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, a))
                    } else if y == pylon_top {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, y, a))
                    } else {
                        make_frame_block_at(pal, "y", seed, c, y, a)
                    };
                    ax.set(blocks, c, a, y, blk);
                }
            }
        }
        let low_brace = (local_walk + 4).max(dy + 5);
        let top_brace = pylon_top - 1;
        for c in (c0 + 1)..c1 {
            for &by in &[low_brace, top_brace] {
                if by <= pylon_top && by >= local_walk + 4 {
                    ax.set(
                        blocks,
                        c,
                        pylon,
                        by,
                        make_beam_block_at(pal, ax.axis_cross(), seed, c, by, pylon),
                    );
                }
            }
        }
        if pylon_top + 1 < ax.height {
            for &c in &[c0, c1] {
                ax.set(
                    blocks,
                    c,
                    pylon,
                    pylon_top + 1,
                    make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, c, pylon_top + 1, pylon), false),
                );
            }
        }
    }

    let cable_y_at = |a: i32| -> i32 {
        if a <= pylon_a {
            let anchor_y = deck_profile[1] + 2;
            let t = ((a - 1) as f64 / (pylon_a - 1).max(1) as f64).clamp(0.0, 1.0);
            (anchor_y as f64 + (pylon_top - anchor_y) as f64 * t.powf(1.45)).round() as i32
        } else if a >= pylon_b {
            let anchor_y = deck_profile[(along_len - 2) as usize] + 2;
            let t = ((along_len - 2 - a) as f64 / (along_len - 2 - pylon_b).max(1) as f64).clamp(0.0, 1.0);
            (anchor_y as f64 + (pylon_top - anchor_y) as f64 * t.powf(1.45)).round() as i32
        } else {
            let half = ((pylon_b - pylon_a) as f64 / 2.0).max(1.0);
            let x = (((a - pylon_a) as f64 - half) / half).clamp(-1.0, 1.0);
            (sag_min as f64 + (pylon_top - sag_min) as f64 * (x * x)).round() as i32
        }
    };

    for &c in &[c0, c1] {
        let mut prev_y: Option<i32> = None;
        for a in 1..=(along_len - 2) {
            let cy = cable_y_at(a).min(pylon_top);
            let cable_blk = if use_copper_cable {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, cy, a))
            } else {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FRAME, seed, c, cy, a))
            };
            ax.set(blocks, c, a, cy, cable_blk.clone());
            if let Some(py) = prev_y {
                let lo = py.min(cy);
                let hi = py.max(cy);
                for fy in lo..=hi {
                    ax.set(blocks, c, a, fy, cable_blk.clone());
                }
            }
            prev_y = Some(cy);

            if a != pylon_a
                && a != pylon_b
                && a != pylon_a - 1
                && a != pylon_b - 1
                && (a % hanger_interval == 0)
            {
                let dy = deck_profile[a as usize];
                for hy in (dy + 1)..cy {
                    ax.set(blocks, c, a, hy, PlannedBlock::simple("minecraft:iron_chain"));
                }
            }
        }
    }
}

/// Builds upper Roman Aqueduct arcade tier with sealed water trough along the top deck.
#[allow(clippy::too_many_arguments)]
fn build_aqueduct_upper_tier(
    ax: BridgeAxes,
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    seed: u64,
    c0: i32,
    c1: i32,
    walk_top: &[i32],
    spans: &[(i32, i32)],
) {
    let max_walk = walk_top.iter().copied().max().unwrap_or(4);
    let min_floor_y = max_walk + 4;
    if min_floor_y + 1 >= ax.height || ax.height - 2 < min_floor_y {
        return;
    }
    let channel_floor_y = (ax.height - 3).clamp(min_floor_y, ax.height - 2);
    let mid_c = (c0 + c1) / 2;
    let (aq_c0, aq_c1) = if c1 - c0 < 8 {
        (c0, c1)
    } else {
        let channel_hw = ((c1 - c0) / 4).clamp(2, 3);
        let a0 = (mid_c - channel_hw - 1).max(c0);
        let a1 = (mid_c + channel_hw + 1).min(c1);
        if a1 - a0 >= 4 {
            (a0, a1)
        } else {
            (c0, c1)
        }
    };

    for a in 0..ax.along_len {
        let local_walk = walk_top[a as usize];
        let in_main_span = spans.iter().any(|&(s0, s1)| a >= s0 && a <= s1);
        let sub_arch_open = in_main_span && (a % 4 != 0) && channel_floor_y - local_walk >= 4;
        for c in [aq_c0, aq_c1] {
            for y in (local_walk + 1)..channel_floor_y {
                let open_here = sub_arch_open && y < channel_floor_y - 1;
                if !open_here {
                    let blk = if y == channel_floor_y - 1 || y == local_walk + 1 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, a))
                    } else {
                        make_pillar_block_at(pal, "y", seed, c, y, a)
                    };
                    ax.set(blocks, c, a, y, blk);
                }
            }
        }
        for c in aq_c0..=aq_c1 {
            ax.set(
                blocks,
                c,
                a,
                channel_floor_y,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, channel_floor_y, a)),
            );
            let is_wall = c == aq_c0 || c == aq_c1 || a == 0 || a == ax.along_len - 1;
            if is_wall {
                ax.set(
                    blocks,
                    c,
                    a,
                    channel_floor_y + 1,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, c, channel_floor_y + 1, a)),
                );
                if channel_floor_y + 2 < ax.height && a % 3 == 0 {
                    ax.set(
                        blocks,
                        c,
                        a,
                        channel_floor_y + 2,
                        make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, channel_floor_y + 2, a), "bottom"),
                    );
                }
            } else {
                ax.set(
                    blocks,
                    c,
                    a,
                    channel_floor_y + 1,
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")]),
                );
            }
        }
    }
}

/// Builds Covered Timber bridge gallery posts, view bays/windows & gabled/pagoda shingle canopy roof.
#[allow(clippy::too_many_arguments)]
fn build_covered_canopy(
    ax: BridgeAxes,
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    spec: &BlueprintSpec,
    seed: u64,
    c0: i32,
    c1: i32,
    deck_profile: &[i32],
    walk_top: &[i32],
    rng: &mut SplitMix64,
) {
    let along_len = ax.along_len;
    let cov_start = 2;
    let cov_end = (along_len - 3).max(cov_start + 3);
    let mid_c = (c0 + c1) / 2;
    let eave_half = (c1 - mid_c + 1).max(2);
    let open_gallery = rng.next_f64() < 0.45;
    let post_step = if cov_end - cov_start >= 16 { 4 } else { 3 };
    let is_japanese = spec.style == BuildStyle::Japanese;

    for a in cov_start..=cov_end {
        let dy = deck_profile[a as usize];
        let local_walk = max_walk_window(walk_top, a, 1);
        let wall_top = (dy + 4).max(local_walk + 4).min(ax.height - 2);
        if wall_top < local_walk + 4 {
            continue;
        }
        let is_post = a == cov_start || a == cov_end || ((a - cov_start) % post_step == 0);

        for &c in &[c0, c1] {
            if is_post {
                for y in (dy + 1)..=wall_top {
                    ax.set(blocks, c, a, y, make_frame_block_at(pal, "y", seed, c, y, a));
                }
            } else {
                ax.set(
                    blocks,
                    c,
                    a,
                    dy + 1,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, c, dy + 1, a)),
                );
                if !open_gallery && dy + 2 < wall_top {
                    ax.set(
                        blocks,
                        c,
                        a,
                        dy + 2,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, c, dy + 2, a)),
                    );
                }
                ax.set(
                    blocks,
                    c,
                    a,
                    wall_top,
                    make_beam_block_at(pal, ax.axis_along(), seed, c, wall_top, a),
                );
            }
        }

        if is_post && wall_top < ax.height - 1 {
            for c in (c0 + 1)..c1 {
                ax.set(
                    blocks,
                    c,
                    a,
                    wall_top,
                    make_beam_block_at(pal, ax.axis_cross(), seed, c, wall_top, a),
                );
            }
            if wall_top - 1 >= local_walk + 4 {
                ax.set(
                    blocks,
                    mid_c,
                    a,
                    wall_top - 1,
                    make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, mid_c, wall_top - 1, a), true),
                );
            }
        }

        let r0 = (mid_c - eave_half).max(0);
        let r1 = (mid_c + eave_half).min(ax.cross_len - 1);
        for c in r0..=r1 {
            let dc = (c - mid_c).abs();
            let edge_rise = eave_half - dc;
            let ry = (wall_top + 1 + edge_rise / 2).min(ax.height - 1);
            if c > c0 && c < c1 && ry < local_walk + 4 {
                continue;
            }
            if dc == 0 {
                let ridge = if is_japanese {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, ry, a))
                } else {
                    make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, c, ry, a), "bottom")
                };
                ax.set(blocks, c, a, ry, ridge);
                if ry > wall_top + 1 && ry - 1 >= local_walk + 4 {
                    ax.set(
                        blocks,
                        c,
                        a,
                        ry - 1,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, ry - 1, a)),
                    );
                }
            } else {
                let face = if c < mid_c {
                    ax.dir_cross_pos()
                } else {
                    ax.dir_cross_neg()
                };
                if is_japanese && dc == eave_half {
                    ax.set(
                        blocks,
                        c,
                        a,
                        ry,
                        make_slab(&pal.pick(MaterialPalette::SLOT_ROOF_EDGE, seed, c, ry, a), "top"),
                    );
                } else if edge_rise % 2 == 0 {
                    ax.set(
                        blocks,
                        c,
                        a,
                        ry,
                        pick_stair_for_slot(pal, MaterialPalette::SLOT_ROOF, seed, c, ry, a, face, "bottom"),
                    );
                } else {
                    ax.set(
                        blocks,
                        c,
                        a,
                        ry,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, ry, a)),
                    );
                }
            }
        }
    }
}

/// Builds a monumental, full-scale, geometrically varied Bridge / Viaduct blueprint.
pub(crate) fn generate_bridge(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();
    let ax = BridgeAxes::new(grid, height);
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0x4252_4944_4745_9911);
    let pal = &req.palette;
    let variant = spec.bridge_variant;

    let (c0, c1) = compute_bridge_width(ax.cross_len, variant, &mut rng);
    let mid_c = (c0 + c1) / 2;
    let spans = compute_arch_spans(ax.along_len, variant, &mut rng);
    let deck_profile = compute_deck_profile(ax, variant, &spans, c0, c1, terrain_offsets, &mut rng);
    let walk_top = compute_walk_top_profile(&deck_profile);

    let cutwater_prow = rng.next_f64() < 0.72;
    let open_spandrels = !matches!(variant, BridgeVariant::PontoonPier | BridgeVariant::Suspension)
        && rng.next_f64() < 0.48;
    let balcony_mode = if ax.cross_len - (c1 - c0 + 1) >= 4
        && ax.along_len >= 19
        && variant != BridgeVariant::CoveredTimber
    {
        rng.range_i32(0, 2)
    } else {
        0
    };
    let parapet_style = rng.range_i32(0, 3);
    let spandrel_thick = if variant == BridgeVariant::GrandSingleArch || variant == BridgeVariant::TaikoMoon {
        1
    } else {
        2
    };
    let lamp_spacing = if ax.along_len >= 34 { 6 } else { 4 };

    let mut pier_centers: Vec<i32> = Vec::new();
    for pair in spans.windows(2) {
        let p_start = pair[0].1 + 1;
        let p_end = pair[1].0 - 1;
        if p_end >= p_start {
            pier_centers.push((p_start + p_end) / 2);
        }
    }

    let step_slot = if matches!(
        variant,
        BridgeVariant::CoveredTimber | BridgeVariant::PontoonPier | BridgeVariant::TaikoMoon
    ) {
        MaterialPalette::SLOT_FLOOR
    } else {
        MaterialPalette::SLOT_PAVING
    };

    // 1. Substructure: Abutments, Cutwater Piers, Carved Arch Vaults & Smooth Stepped Roadway Deck.
    for along in 0..ax.along_len {
        let dy = deck_profile[along as usize];
        let prev_dy = if along > 0 { deck_profile[(along - 1) as usize] } else { dy };
        let prev2_dy = if along > 1 { deck_profile[(along - 2) as usize] } else { prev_dy };
        let next_dy = if along + 1 < ax.along_len {
            deck_profile[(along + 1) as usize]
        } else {
            dy
        };
        let next2_dy = if along + 2 < ax.along_len {
            deck_profile[(along + 2) as usize]
        } else {
            next_dy
        };

        let matching_span = spans.iter().find(|&&(s0, s1)| along >= s0 && along <= s1).copied();
        let is_pier_zone = pier_centers.iter().any(|&pc| (along - pc).abs() <= 1);
        let is_cutwater_center = pier_centers.contains(&along);

        for c in c0..=c1 {
            let terr = ax.terr(terrain_offsets, c, along).clamp(-5, 3);
            let floor_anchor = (terr - 4).min(dy - 2).max(-6);

            if variant == BridgeVariant::PontoonPier {
                let is_bents = matching_span.is_none() || (along % 4 == 0);
                let is_piling_col = is_bents && (c == c0 || c == c1 || c == mid_c || (c - mid_c).abs() == 3);
                if is_piling_col {
                    for y in floor_anchor..dy {
                        ax.set(&mut blocks, c, along, y, make_frame_block_at(pal, "y", seed, c, y, along));
                    }
                } else if is_bents && dy - 1 >= floor_anchor {
                    ax.set(
                        &mut blocks,
                        c,
                        along,
                        dy - 1,
                        make_beam_block_at(pal, ax.axis_cross(), seed, c, dy - 1, along),
                    );
                }
            } else if let Some((s0, s1)) = matching_span {
                let arch_void = compute_arch_void_ceiling(
                    variant,
                    along,
                    s0,
                    s1,
                    dy,
                    terr.min(0),
                    spandrel_thick,
                );
                let is_rib_col = c == c0 || c == c1 || (c == mid_c && (c1 - c0) >= 8);
                if arch_void < dy {
                    let t_mid = (s0 + s1) / 2;
                    let stair_face = if along < t_mid {
                        ax.dir_along_fwd()
                    } else {
                        ax.dir_along_back()
                    };

                    if variant == BridgeVariant::Suspension {
                        if c == c0 || c == c1 || ((c + along).abs() % 2 == 0) {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                dy - 1,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FRAME, seed, c, dy - 1, along)),
                            );
                        } else if dy - 1 >= floor_anchor {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                dy - 1,
                                make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, dy - 1, along), "top"),
                            );
                        }
                    } else if variant == BridgeVariant::TaikoMoon {
                        let beam_y = dy - 1;
                        if beam_y < dy && beam_y >= floor_anchor {
                            if is_rib_col {
                                ax.set(
                                    &mut blocks,
                                    c,
                                    along,
                                    beam_y,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, beam_y, along)),
                                );
                                if beam_y - 1 >= floor_anchor {
                                    ax.set(
                                        &mut blocks,
                                        c,
                                        along,
                                        beam_y - 1,
                                        pick_stair_for_slot(pal, MaterialPalette::SLOT_ROOF, seed, c, beam_y - 1, along, stair_face, "top"),
                                    );
                                }
                            } else if along % 3 == 0 {
                                ax.set(
                                    &mut blocks,
                                    c,
                                    along,
                                    beam_y,
                                    make_beam_block_at(pal, ax.axis_cross(), seed, c, beam_y, along),
                                );
                            }
                        }
                    } else {
                        let fill_bot = arch_void;
                        if is_rib_col || c == c0 + 1 || c == c1 - 1 {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                fill_bot,
                                pick_stair_for_slot(pal, MaterialPalette::SLOT_FOUNDATION, seed, c, fill_bot, along, stair_face, "top"),
                            );
                            for y in (fill_bot + 1)..dy {
                                let open_hole = open_spandrels
                                    && (dy - fill_bot >= 4)
                                    && (along % 2 == 1)
                                    && y < dy - 1
                                    && (c == c0 || c == c1);
                                let blk = if open_hole {
                                    PlannedBlock::simple("minecraft:air")
                                } else if is_rib_col && (y == fill_bot + 1 || y == dy - 1) {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, along))
                                } else {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, along))
                                };
                                ax.set(&mut blocks, c, along, y, blk);
                            }
                        } else if open_spandrels && dy - arch_void >= 3 {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                fill_bot,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, fill_bot, along)),
                            );
                            if along % 3 == 0 {
                                for y in (fill_bot + 1)..dy {
                                    ax.set(
                                        &mut blocks,
                                        c,
                                        along,
                                        y,
                                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, c, y, along)),
                                    );
                                }
                            } else {
                                ax.set(
                                    &mut blocks,
                                    c,
                                    along,
                                    dy - 1,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, dy - 1, along)),
                                );
                            }
                        } else {
                            for y in fill_bot..dy {
                                let blk = if y == fill_bot && variant == BridgeVariant::GothicRibbedArch {
                                    make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, along), "top")
                                } else {
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, along))
                                };
                                ax.set(&mut blocks, c, along, y, blk);
                            }
                        }
                    }
                }
            } else {
                for y in floor_anchor..dy {
                    let is_accent = y == dy - 1 || y == terr + 1 || (y == dy - 3 && is_pier_zone);
                    let blk = if is_accent {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, along))
                    } else if is_pier_zone && (c == c0 || c == c1) {
                        make_pillar_block_at(pal, "y", seed, c, y, along)
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, y, along))
                    };
                    ax.set(&mut blocks, c, along, y, blk);
                }
            }

            if is_cutwater_center && cutwater_prow && variant != BridgeVariant::PontoonPier {
                let pier_top = (dy - 1).min(terr + 4).clamp(-5, dy - 1);
                if pier_top >= -4 {
                    for side in [-1, 1] {
                        for dist in 1..=2 {
                            let pc = if side < 0 { c0 - dist } else { c1 + dist };
                            if pc < 0 || pc >= ax.cross_len {
                                continue;
                            }
                            let local_top = pier_top - (dist - 1);
                            let pterr = ax.terr(terrain_offsets, pc, along).clamp(-5, 2);
                            let pbot = (pterr - 3).max(-6);
                            if dist == 2 && local_top <= pbot + 1 {
                                continue;
                            }
                            let da_max = if dist == 1 { 1 } else { 0 };
                            for da in -da_max..=da_max {
                                let pa = along + da;
                                if pa < 0 || pa >= ax.along_len {
                                    continue;
                                }
                                let pterr_a = ax.terr(terrain_offsets, pc, pa).clamp(-5, 2);
                                for y in (pterr_a - 3).max(-6)..=local_top {
                                    let blk = if y == local_top {
                                        make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, pc, y, pa), "bottom")
                                    } else {
                                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, pc, y, pa))
                                    };
                                    ax.set(&mut blocks, pc, pa, y, blk);
                                }
                            }
                        }
                    }
                }
            }

            let is_curb = c == c0 || c == c1;
            let deck_blk = if is_curb {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, dy, along))
            } else if matches!(
                variant,
                BridgeVariant::CoveredTimber | BridgeVariant::PontoonPier | BridgeVariant::TaikoMoon
            ) {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, c, dy, along))
            } else if c == mid_c && (c1 - c0) >= 8 && variant == BridgeVariant::Suspension && along % 2 == 0 {
                PlannedBlock::simple("minecraft:smooth_quartz")
            } else if (c - c0 == 1 || c1 - c == 1) && (c1 - c0) >= 6 {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, dy, along))
            } else {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, c, dy, along))
            };
            ax.set(&mut blocks, c, along, dy, deck_blk);

            if !is_curb {
                if next_dy == dy + 1 && prev_dy != dy + 1 {
                    let blk = if next2_dy <= dy + 1 {
                        make_slab(&pal.pick(step_slot, seed, c, dy + 1, along), "bottom")
                    } else {
                        pick_stair_for_slot(
                            pal,
                            step_slot,
                            seed,
                            c,
                            dy + 1,
                            along,
                            ax.dir_along_fwd(),
                            "bottom",
                        )
                    };
                    ax.set(&mut blocks, c, along, dy + 1, blk);
                } else if prev_dy == dy + 1 && next_dy != dy + 1 {
                    let blk = if prev2_dy <= dy + 1 {
                        make_slab(&pal.pick(step_slot, seed, c, dy + 1, along), "bottom")
                    } else {
                        pick_stair_for_slot(
                            pal,
                            step_slot,
                            seed,
                            c,
                            dy + 1,
                            along,
                            ax.dir_along_back(),
                            "bottom",
                        )
                    };
                    ax.set(&mut blocks, c, along, dy + 1, blk);
                } else if next_dy == dy + 1 && prev_dy == dy + 1 {
                    ax.set(
                        &mut blocks,
                        c,
                        along,
                        dy + 1,
                        make_slab(&pal.pick(step_slot, seed, c, dy + 1, along), "bottom"),
                    );
                }
            }
        }

        let skip_parapet =
            variant == BridgeVariant::CoveredTimber && along >= 2 && along <= ax.along_len - 3;
        if !skip_parapet {
            let is_major_post = along == 0
                || along == ax.along_len - 1
                || is_cutwater_center
                || (along % lamp_spacing == 0);

            for &c in &[c0, c1] {
                if is_major_post {
                    let post_h = if variant == BridgeVariant::GothicRibbedArch && is_cutwater_center {
                        3
                    } else if variant == BridgeVariant::PontoonPier {
                        1
                    } else {
                        2
                    };
                    for py in 1..=post_h {
                        let y = dy + py;
                        let blk = if py == 1 && post_h >= 2 {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, c, y, along))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, c, y, along))
                        };
                        ax.set(&mut blocks, c, along, y, blk);
                    }
                    let light_y = dy + post_h + 1;
                    if light_y < ax.height && variant != BridgeVariant::Aqueduct {
                        ax.set(
                            &mut blocks,
                            c,
                            along,
                            light_y,
                            make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, c, light_y, along), false),
                        );
                    }
                } else {
                    match parapet_style {
                        1 if variant != BridgeVariant::TaikoMoon && variant != BridgeVariant::PontoonPier => {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                dy + 1,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, c, dy + 1, along)),
                            );
                            if along % 2 == 0 {
                                ax.set(
                                    &mut blocks,
                                    c,
                                    along,
                                    dy + 2,
                                    make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, c, dy + 2, along), "bottom"),
                                );
                            }
                        }
                        2 => {
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                dy + 1,
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, c, dy + 1, along)),
                            );
                        }
                        3 => {
                            let rail_face = if c == c0 {
                                ax.dir_cross_neg()
                            } else {
                                ax.dir_cross_pos()
                            };
                            ax.set(
                                &mut blocks,
                                c,
                                along,
                                dy + 1,
                                pick_stair_for_slot(pal, MaterialPalette::SLOT_FOUNDATION, seed, c, dy + 1, along, rail_face, "bottom"),
                            );
                        }
                        _ => {
                            let id = if matches!(
                                variant,
                                BridgeVariant::TaikoMoon | BridgeVariant::PontoonPier | BridgeVariant::CoveredTimber
                            ) {
                                pal.pick(MaterialPalette::SLOT_FENCE, seed, c, dy + 1, along)
                            } else {
                                pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, c, dy + 1, along)
                            };
                            ax.set(&mut blocks, c, along, dy + 1, PlannedBlock::simple(id));
                        }
                    }
                }
            }
        }
    }

    // 3. Variant-Specific Monumental Superstructures!
    let mid_along = ax.along_len / 2;
    let portal_hw = ((c1 - c0) / 2 - 1).clamp(1, 4);
    let mut tower_zones: Vec<i32> = Vec::new();

    match variant {
        BridgeVariant::Suspension => {
            build_suspension_rig(ax, &mut blocks, pal, seed, c0, c1, &deck_profile, &walk_top, &mut rng);
        }
        BridgeVariant::Aqueduct => {
            build_aqueduct_upper_tier(ax, &mut blocks, pal, seed, c0, c1, &walk_top, &spans);
        }
        BridgeVariant::CoveredTimber => {
            build_covered_canopy(ax, &mut blocks, pal, &spec, seed, c0, c1, &deck_profile, &walk_top, &mut rng);
        }
        BridgeVariant::TaikoMoon => {
            let g1 = 1.min(ax.along_len - 2);
            let g2 = (ax.along_len - 2).max(g1 + 2);
            build_torii_portal(ax, &mut blocks, pal, seed, c0, c1, g1, deck_profile[g1 as usize], &walk_top);
            if g2 != g1 {
                build_torii_portal(ax, &mut blocks, pal, seed, c0, c1, g2, deck_profile[g2 as usize], &walk_top);
            }
        }
        BridgeVariant::FortifiedGate => {
            let mode = rng.range_i32(0, 2);
            if mode == 0 || mode == 2 {
                for ga in [2, (ax.along_len - 3).max(4)] {
                    let dy = deck_profile[ga as usize];
                    let top_y = (dy + 6).min(ax.height - 2);
                    build_bridge_tower(ax, &mut blocks, pal, seed, c0, c1, ga, &deck_profile, &walk_top, top_y, portal_hw, false);
                    tower_zones.push(ga);
                }
            }
            if mode == 1 || mode == 2 {
                let dy = deck_profile[mid_along as usize];
                let top_y = (dy + 7).min(ax.height - 2);
                build_bridge_tower(ax, &mut blocks, pal, seed, c0, c1, mid_along, &deck_profile, &walk_top, top_y, portal_hw, false);
                tower_zones.push(mid_along);
            }
        }
        BridgeVariant::GothicRibbedArch => {
            if rng.next_f64() < 0.55 {
                let dy = deck_profile[mid_along as usize];
                let top_y = (dy + 6).min(ax.height - 3);
                build_bridge_tower(ax, &mut blocks, pal, seed, c0, c1, mid_along, &deck_profile, &walk_top, top_y, portal_hw, true);
                tower_zones.push(mid_along);
            }
        }
        BridgeVariant::GrandSingleArch | BridgeVariant::MasonryArch => {
            let deco = rng.range_i32(0, 2);
            if deco == 1 {
                let dy = deck_profile[mid_along as usize];
                let top_y = (dy + 6).min(ax.height - 2);
                build_bridge_tower(ax, &mut blocks, pal, seed, c0, c1, mid_along, &deck_profile, &walk_top, top_y, portal_hw, true);
                tower_zones.push(mid_along);
            } else if deco == 2 {
                for &ea in &[1, (ax.along_len - 2).max(2)] {
                    let dy = deck_profile[ea as usize];
                    for &ec in &[c0, c1] {
                        for y in (dy + 1)..=(dy + 4).min(ax.height - 2) {
                            let blk = if y == dy + 4 {
                                make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, ec, y, ea), "bottom")
                            } else {
                                make_pillar_block_at(pal, "y", seed, ec, y, ea)
                            };
                            ax.set(&mut blocks, ec, ea, y, blk);
                        }
                    }
                }
            }
        }
        BridgeVariant::PontoonPier => {
            let dy = deck_profile[mid_along as usize];
            let g_a0 = (mid_along - 2).max(1);
            let g_a1 = (mid_along + 2).min(ax.along_len - 2);
            let max_gaz_walk = max_walk_window(&walk_top, mid_along, 2);
            let gazebo_top = (max_gaz_walk + 4).max(dy + 5).min(ax.height - 2);
            if gazebo_top >= max_gaz_walk + 4 && ax.along_len >= 13 {
                for &gc in &[c0, c1] {
                    for &ga in &[g_a0, g_a1] {
                        for y in (deck_profile[ga as usize] + 1)..gazebo_top {
                            ax.set(&mut blocks, gc, ga, y, make_frame_block_at(pal, "y", seed, gc, y, ga));
                        }
                    }
                }
                for a in (g_a0 - 1).max(0)..=(g_a1 + 1).min(ax.along_len - 1) {
                    for c in (c0 - 1).max(0)..=(c1 + 1).min(ax.cross_len - 1) {
                        let edge = a < g_a0 || a > g_a1 || c < c0 || c > c1;
                        let ry = if edge { gazebo_top } else { (gazebo_top + 1).min(ax.height - 1) };
                        ax.set(
                            &mut blocks,
                            c,
                            a,
                            ry,
                            if edge {
                                make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, c, ry, a), "bottom")
                            } else {
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, c, ry, a))
                            },
                        );
                    }
                }
                if gazebo_top >= max_gaz_walk + 4 {
                    ax.set(
                        &mut blocks,
                        mid_c,
                        mid_along,
                        gazebo_top,
                        make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, mid_c, gazebo_top, mid_along), true),
                    );
                }
            }
        }
    }

    // 4. Protruding Balcony Overlooks with Benches & Streetlamps when extra side margin exists!
    if balcony_mode > 0 {
        let mut bay_centers = Vec::new();
        if balcony_mode == 1 {
            bay_centers.push(mid_along);
        } else {
            bay_centers.extend(pier_centers.iter().copied());
        }
        for bay_a in bay_centers {
            if tower_zones.iter().any(|&ta| (bay_a - ta).abs() <= 2) {
                continue;
            }
            let dy = deck_profile[bay_a as usize];
            for side in [-1, 1] {
                let bc = if side < 0 { c0 - 1 } else { c1 + 1 };
                let outer_c = if side < 0 { c0 - 2 } else { c1 + 2 };
                if outer_c < 0 || outer_c >= ax.cross_len {
                    continue;
                }
                for da in -2..=2 {
                    let a = bay_a + da;
                    if a < 1 || a >= ax.along_len - 1 {
                        continue;
                    }
                    let reach_outer = da.abs() <= 1;
                    let edge_c = if side < 0 { c0 } else { c1 };
                    let edge_wt = walk_top[a as usize];
                    ax.set(
                        &mut blocks,
                        edge_c,
                        a,
                        dy,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, edge_c, dy, a)),
                    );
                    for cy in (dy + 1)..=(edge_wt + 3).min(ax.height - 1) {
                        if da.abs() <= 1 || cy <= dy + 1 {
                            ax.set(&mut blocks, edge_c, a, cy, PlannedBlock::simple("minecraft:air"));
                        }
                    }
                    ax.set(
                        &mut blocks,
                        bc,
                        a,
                        dy,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, bc, dy, a)),
                    );
                    let corbel_face = if side < 0 {
                        ax.dir_cross_pos()
                    } else {
                        ax.dir_cross_neg()
                    };
                    ax.set(
                        &mut blocks,
                        bc,
                        a,
                        dy - 1,
                        pick_stair_for_slot(pal, MaterialPalette::SLOT_FOUNDATION, seed, bc, dy - 1, a, corbel_face, "top"),
                    );
                    if reach_outer {
                        ax.set(
                            &mut blocks,
                            outer_c,
                            a,
                            dy,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, outer_c, dy, a)),
                        );
                        ax.set(
                            &mut blocks,
                            outer_c,
                            a,
                            dy + 1,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, outer_c, dy + 1, a)),
                        );
                    } else {
                        ax.set(
                            &mut blocks,
                            bc,
                            a,
                            dy + 1,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, bc, dy + 1, a)),
                        );
                    }
                }
                ax.set(
                    &mut blocks,
                    bc,
                    bay_a,
                    dy + 1,
                    pick_stair_for_slot(
                        pal,
                        MaterialPalette::SLOT_FLOOR,
                        seed,
                        bc,
                        dy + 1,
                        bay_a,
                        if side < 0 { ax.dir_cross_pos() } else { ax.dir_cross_neg() },
                        "bottom",
                    ),
                );
                if dy + 2 < ax.height {
                    ax.set(
                        &mut blocks,
                        outer_c,
                        bay_a,
                        dy + 2,
                        make_bridge_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, outer_c, dy + 2, bay_a), false),
                    );
                }
            }
        }
    }

    // 5. Optional Overgrown Ivy & Ruin Erosion.
    if spec.overgrown {
        for along in 2..(ax.along_len - 2) {
            if along % 3 != 0 {
                continue;
            }
            let dy = deck_profile[along as usize];
            for &c in &[c0, c1] {
                if noise.cell_hash(c, along, 99) > 0.45 && dy - 1 >= -4 {
                    let leaf = make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, c, dy - 1, along));
                    ax.set(&mut blocks, c, along, dy - 1, leaf);
                }
            }
        }
    }

    // 6. Universal Bridge Walkthrough & Overhead Clearance Invariant: guarantee 3 clear blocks above every deck/step.
    for along in 0..ax.along_len {
        let in_tower = tower_zones.iter().any(|&ta| (along - ta).abs() <= 1);
        let (clear_c0, clear_c1) = if in_tower {
            ((mid_c - portal_hw).max(c0 + 1), (mid_c + portal_hw).min(c1 - 1))
        } else {
            (c0 + 1, c1 - 1)
        };
        let clear_lo = walk_top[along as usize] + 1;
        let clear_hi = (walk_top[along as usize] + 3).min(ax.height - 1);
        for c in clear_c0..=clear_c1 {
            for y in clear_lo..=clear_hi {
                ax.set(&mut blocks, c, along, y, PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    prune_map_unsupported(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);

    let (door_u, door_v) = ax.uv(mid_c, ax.along_len - 1);
    let [dlx, dlz] = grid.to_local_xz(door_u, door_v);
    let door_y = walk_top[(ax.along_len - 1) as usize] + 1;

    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: grid.orig_w as u16,
        depth: grid.orig_d as u16,
        height,
        blocks,
        door_pos: Some([dlx, door_y, dlz]),
    }
}
