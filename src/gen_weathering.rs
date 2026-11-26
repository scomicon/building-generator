use std::collections::HashSet;

use crate::gen_common::{PlannedBlock, make_leaves};
use crate::generator::GeneratedBlueprint;
use crate::model::BuildRequest;
use crate::noise::{NoiseField, SplitMix64};

fn is_door_or_bed(id: &str) -> bool {
    id.ends_with("_door") || id.ends_with("_bed")
}

fn is_solid_structural(blk: &PlannedBlock) -> bool {
    let id = blk.block_id.as_str();
    !matches!(
        id,
        "minecraft:air"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:cobweb"
            | "minecraft:vine"
            | "minecraft:torch"
            | "minecraft:wall_torch"
            | "minecraft:soul_torch"
            | "minecraft:soul_wall_torch"
            | "minecraft:fern"
            | "minecraft:short_grass"
            | "minecraft:dead_bush"
            | "minecraft:pink_petals"
            | "minecraft:wildflowers"
            | "minecraft:leaf_litter"
            | "minecraft:azalea"
            | "minecraft:flowering_azalea"
            | "minecraft:lily_pad"
            | "minecraft:ladder"
            | "minecraft:sugar_cane"
            | "minecraft:wheat"
            | "minecraft:flower_pot"
    ) && !id.ends_with("_carpet")
        && !id.ends_with("_door")
        && !id.ends_with("_bed")
        && !id.ends_with("_leaves")
        && !id.starts_with("minecraft:potted_")
}

fn is_erodable_or_weathered_support(id: &str) -> bool {
    !matches!(
        id,
        "minecraft:air"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:bedrock"
            | "minecraft:chest"
            | "minecraft:barrel"
            | "minecraft:ender_chest"
            | "minecraft:beacon"
            | "minecraft:enchanting_table"
    ) && !is_door_or_bed(id)
}

fn near_protected_door(pos: [i32; 3], door_pos: Option<[i32; 3]>) -> bool {
    let Some(dp) = door_pos else {
        return false;
    };
    (pos[0] - dp[0]).abs() <= 1
        && (pos[2] - dp[2]).abs() <= 1
        && (pos[1] >= dp[1] - 1 && pos[1] <= dp[1] + 2)
}

fn copper_oxidation_stage(id: &str, wear: f64, roll: f64) -> Option<(&'static str, bool)> {
    let stage = ((wear * 2.8) + roll * 0.9).floor() as i32;
    if stage <= 0 {
        return None;
    }
    match id {
        "minecraft:cut_copper" => Some((
            match stage {
                1 => "minecraft:exposed_cut_copper",
                2 => "minecraft:weathered_cut_copper",
                _ => "minecraft:oxidized_cut_copper",
            },
            false,
        )),
        "minecraft:exposed_cut_copper" => Some((
            if stage >= 2 {
                "minecraft:oxidized_cut_copper"
            } else {
                "minecraft:weathered_cut_copper"
            },
            false,
        )),
        "minecraft:weathered_cut_copper" => Some(("minecraft:oxidized_cut_copper", false)),
        "minecraft:cut_copper_stairs" => Some((
            match stage {
                1 => "minecraft:exposed_cut_copper_stairs",
                2 => "minecraft:weathered_cut_copper_stairs",
                _ => "minecraft:oxidized_cut_copper_stairs",
            },
            true,
        )),
        "minecraft:exposed_cut_copper_stairs" => Some(("minecraft:weathered_cut_copper_stairs", true)),
        "minecraft:cut_copper_slab" => Some((
            match stage {
                1 => "minecraft:exposed_cut_copper_slab",
                2 => "minecraft:weathered_cut_copper_slab",
                _ => "minecraft:oxidized_cut_copper_slab",
            },
            true,
        )),
        "minecraft:exposed_cut_copper_slab" => Some(("minecraft:weathered_cut_copper_slab", true)),
        _ => None,
    }
}

fn weathered_masonry_variant(id: &str, wear: f64, roll: f64) -> Option<(&'static str, bool)> {
    if let Some(c) = copper_oxidation_stage(id, wear, roll) {
        return Some(c);
    }
    if roll > 0.14 + wear * 0.56 {
        return None;
    }
    match id {
        "minecraft:stone_bricks" => Some((
            if roll < wear * 0.24 {
                "minecraft:cobblestone"
            } else {
                "minecraft:cracked_stone_bricks"
            },
            false,
        )),
        "minecraft:stone_brick_stairs" => Some(("minecraft:cobblestone_stairs", true)),
        "minecraft:stone_brick_slab" => Some(("minecraft:cobblestone_slab", true)),
        "minecraft:stone_brick_wall" => Some(("minecraft:cobblestone_wall", true)),
        "minecraft:deepslate_bricks" => Some((
            if roll < wear * 0.24 {
                "minecraft:cobbled_deepslate"
            } else {
                "minecraft:cracked_deepslate_bricks"
            },
            false,
        )),
        "minecraft:deepslate_tiles" => Some((
            if roll < wear * 0.22 {
                "minecraft:cobbled_deepslate"
            } else {
                "minecraft:cracked_deepslate_tiles"
            },
            false,
        )),
        "minecraft:deepslate_brick_stairs" => Some(("minecraft:cobbled_deepslate_stairs", true)),
        "minecraft:nether_bricks" => Some(("minecraft:cracked_nether_bricks", false)),
        "minecraft:polished_blackstone_bricks" => Some((
            if roll < wear * 0.20 {
                "minecraft:blackstone"
            } else {
                "minecraft:cracked_polished_blackstone_bricks"
            },
            false,
        )),
        "minecraft:mud_bricks" => Some(("minecraft:packed_mud", false)),
        "minecraft:smooth_sandstone" | "minecraft:cut_sandstone" => Some((
            if roll < wear * 0.18 {
                "minecraft:sandstone"
            } else {
                "minecraft:chiseled_sandstone"
            },
            false,
        )),
        "minecraft:cut_red_sandstone" => Some(("minecraft:red_sandstone", false)),
        "minecraft:bricks" if wear >= 0.30 && roll < wear * 0.25 => {
            Some(("minecraft:terracotta", false))
        }
        "minecraft:glass" | "minecraft:glass_pane" if wear >= 0.22 => Some((
            if roll < wear * 0.32 {
                "minecraft:air"
            } else {
                "minecraft:iron_bars"
            },
            false,
        )),
        _ if id.ends_with("_stained_glass_pane") && wear >= 0.25 && roll < wear * 0.45 => {
            Some(("minecraft:air", false))
        }
        _ => None,
    }
}

fn mossy_variant(id: &str, arid: bool, over: f64, height_bias: f64, roll: f64) -> Option<(&'static str, bool)> {
    if arid && over < 0.70 {
        return None;
    }
    let thresh = (0.12 + over * 0.58) * height_bias;
    if roll > thresh {
        return None;
    }
    match id {
        "minecraft:cobblestone" => Some(("minecraft:mossy_cobblestone", false)),
        "minecraft:cobblestone_stairs" => Some(("minecraft:mossy_cobblestone_stairs", true)),
        "minecraft:cobblestone_slab" => Some(("minecraft:mossy_cobblestone_slab", true)),
        "minecraft:cobblestone_wall" => Some(("minecraft:mossy_cobblestone_wall", true)),
        "minecraft:stone_bricks" | "minecraft:cracked_stone_bricks" => {
            Some(("minecraft:mossy_stone_bricks", false))
        }
        "minecraft:stone_brick_stairs" => Some(("minecraft:mossy_stone_brick_stairs", true)),
        "minecraft:stone_brick_slab" => Some(("minecraft:mossy_stone_brick_slab", true)),
        "minecraft:stone_brick_wall" => Some(("minecraft:mossy_stone_brick_wall", true)),
        _ => None,
    }
}

fn biome_overgrowth_palette(biome_tag: &str) -> (&'static str, &'static str, &'static str, &'static str, &'static str, bool) {
    // Returns: (leaves_id, trunk_log, carpet_id, flora_primary, flora_secondary, is_arid)
    if biome_tag.contains("pale_garden") {
        (
            "minecraft:pale_oak_leaves",
            "minecraft:pale_oak_log",
            "minecraft:pale_moss_carpet",
            "minecraft:fern",
            "minecraft:pale_moss_carpet",
            false,
        )
    } else if biome_tag.contains("cherry") {
        (
            "minecraft:cherry_leaves",
            "minecraft:cherry_log",
            "minecraft:moss_carpet",
            "minecraft:pink_petals",
            "minecraft:flowering_azalea",
            false,
        )
    } else if biome_tag.contains("desert") || biome_tag.contains("badlands") || biome_tag.contains("savanna") {
        (
            "minecraft:acacia_leaves",
            "minecraft:acacia_log",
            "minecraft:dead_bush",
            "minecraft:dead_bush",
            "minecraft:short_grass",
            true,
        )
    } else if biome_tag.contains("snow")
        || biome_tag.contains("ice")
        || biome_tag.contains("frozen")
        || biome_tag.contains("taiga")
        || biome_tag.contains("grove")
    {
        (
            "minecraft:spruce_leaves",
            "minecraft:spruce_log",
            "minecraft:moss_carpet",
            "minecraft:fern",
            "minecraft:short_grass",
            false,
        )
    } else if biome_tag.contains("jungle") || biome_tag.contains("bamboo") || biome_tag.contains("mangrove") {
        (
            "minecraft:jungle_leaves",
            "minecraft:jungle_log",
            "minecraft:moss_carpet",
            "minecraft:fern",
            "minecraft:flowering_azalea",
            false,
        )
    } else if biome_tag.contains("dark_forest") || biome_tag.contains("swamp") {
        (
            "minecraft:dark_oak_leaves",
            "minecraft:dark_oak_log",
            "minecraft:moss_carpet",
            "minecraft:fern",
            "minecraft:azalea",
            false,
        )
    } else {
        (
            "minecraft:oak_leaves",
            "minecraft:oak_log",
            "minecraft:moss_carpet",
            "minecraft:wildflowers",
            "minecraft:flowering_azalea",
            false,
        )
    }
}

fn rubble_block_for(original_id: &str, arid: bool, over: f64, roll: f64) -> &'static str {
    if original_id.contains("deepslate") || original_id.contains("blackstone") {
        if roll < 0.55 {
            "minecraft:cobbled_deepslate"
        } else {
            "minecraft:cobbled_deepslate_slab"
        }
    } else if original_id.contains("sandstone") || arid {
        if roll < 0.55 {
            "minecraft:sandstone_slab"
        } else if original_id.contains("red") {
            "minecraft:red_sand"
        } else {
            "minecraft:sand"
        }
    } else if original_id.contains("planks") || original_id.contains("log") {
        if roll < 0.55 {
            "minecraft:oak_slab"
        } else {
            "minecraft:cobblestone"
        }
    } else if over >= 0.20 && roll < 0.38 {
        "minecraft:mossy_cobblestone"
    } else if roll < 0.72 {
        "minecraft:cobblestone"
    } else {
        "minecraft:stone_brick_slab"
    }
}

/// Applies unified Wear / Ruin and Biome-Aware Overgrowth post-processing across all building categories.
/// Returns `true` if any blueprint block was added, mutated, or eroded.
pub(crate) fn apply_weathering_and_overgrowth(
    bp: &mut GeneratedBlueprint,
    req: &BuildRequest,
    noise: NoiseField,
) -> bool {
    let lower_desc = req.description.to_lowercase();
    let explicit_overgrown_kw = lower_desc.contains("зарос")
        || lower_desc.contains("плющ")
        || lower_desc.contains("мхом")
        || lower_desc.contains("мшист")
        || lower_desc.contains("overgrown")
        || lower_desc.contains("mossy")
        || lower_desc.contains("ivy");

    let wear = (f64::from(req.wear_pct) / 100.0).max(f64::from(bp.spec.ruin_erosion));
    let mut over = f64::from(req.overgrowth_pct) / 100.0;
    if req.overgrowth_pct == 0 && explicit_overgrown_kw {
        over = 0.36;
    } else if req.overgrowth_pct == 0 && bp.spec.ruin_erosion > 0.05 && bp.spec.overgrown {
        // Light mossy stonework only for keyword ruins unless explicit overgrowth % is requested.
        over = 0.14;
    }

    if wear <= 0.04 && over <= 0.04 {
        return false;
    }

    let min_keep_y = i32::from(bp.spec.foundation_height) + 1;
    let w = i32::from(bp.width);
    let d = i32::from(bp.depth);
    let h = i32::from(bp.height).max(6);
    let (leaves_id, log_id, carpet_id, flora_a, flora_b, arid) =
        biome_overgrowth_palette(&bp.spec.biome_tag);

    let mut changed = false;
    let mut rng = SplitMix64::new((req.seed as u64) ^ 0xAE47_9102_55CC_0019);

    // 1. Structural Ruin Erosion + Weathering Material Swap (keeps doorways & base plinths safe).
    let mut eroded_cols: Vec<[i32; 3]> = Vec::new();
    if wear > 0.04 {
        let positions: Vec<[i32; 3]> = bp.blocks.keys().copied().collect();
        for pos in positions {
            let Some(orig) = bp.blocks.get(&pos).cloned() else {
                continue;
            };
            let id = orig.block_id.as_str();
            if id == "minecraft:air" || near_protected_door(pos, bp.door_pos) || !is_erodable_or_weathered_support(id) {
                continue;
            }
            let r1 = noise.cell_hash(pos[0], pos[1], pos[2]);
            let r2 = noise.cell_hash(pos[0] + 73, pos[1] + 19, pos[2] - 41);

            // Height-biased skyline & roof collapse when wear >= 0.14
            if wear >= 0.14 && pos[1] > min_keep_y {
                let height_ratio = ((pos[1] - min_keep_y) as f64) / ((h - min_keep_y).max(1) as f64);
                let n = noise.fbm_3d(
                    pos[0] as f64 * 0.36,
                    pos[1] as f64 * 0.36,
                    pos[2] as f64 * 0.36,
                    3,
                );
                let erode_thresh = 0.78 - height_ratio * wear * 0.68 - (wear * 0.14);
                if n > erode_thresh {
                    bp.blocks.insert(pos, PlannedBlock::simple("minecraft:air"));
                    if is_solid_structural(&orig) {
                        eroded_cols.push(pos);
                    }
                    changed = true;
                    continue;
                }
            }

            if let Some((new_id, keep_props)) = weathered_masonry_variant(id, wear, r1) {
                let next_blk = if keep_props {
                    PlannedBlock::with_props(new_id, orig.props.clone())
                } else {
                    PlannedBlock::simple(new_id)
                };
                bp.blocks.insert(pos, next_blk);
                changed = true;
                continue;
            }

            // Chipped plank roofs/walls at heavy wear: replace some solid blocks with matching weathered slabs.
            if wear >= 0.45 && pos[1] > min_keep_y + 1 && r2 < (wear - 0.35) * 0.22 {
                let slab_id = if id.ends_with("stone_bricks") {
                    Some("minecraft:stone_brick_slab")
                } else if id.ends_with("cobblestone") {
                    Some("minecraft:cobblestone_slab")
                } else if id.ends_with("spruce_planks") {
                    Some("minecraft:spruce_slab")
                } else if id.ends_with("oak_planks") {
                    Some("minecraft:oak_slab")
                } else if id.ends_with("dark_oak_planks") {
                    Some("minecraft:dark_oak_slab")
                } else {
                    None
                };
                if let Some(sid) = slab_id {
                    bp.blocks.insert(pos, PlannedBlock::with_props(sid, vec![("type", "bottom")]));
                    changed = true;
                }
            }
        }

        // Drop partial rubble onto exposed floor/ground below eroded upper blocks (keep bridges walkable below 70% wear).
        if wear >= 0.20
            && (bp.spec.category != crate::model::BuildCategory::Bridge || wear >= 0.70)
        {
            let mut placed_rubble: HashSet<[i32; 2]> = HashSet::new();
            for ep in eroded_cols {
                if !placed_rubble.insert([ep[0], ep[2]]) {
                    continue;
                }
                let r = noise.cell_hash(ep[0] + 11, ep[1], ep[2] + 37);
                if r > (0.12 + wear * 0.18).min(0.28) {
                    continue;
                }
                let mut drop_y = None;
                for dy in 1..=10 {
                    let cy = ep[1] - dy;
                    if cy < 1 {
                        break;
                    }
                    let check = [ep[0], cy - 1, ep[2]];
                    let here = bp.blocks.get(&[ep[0], cy, ep[2]]);
                    let here_is_air = here.is_none_or(|b| b.block_id == "minecraft:air");
                    let sup_solid = bp.blocks.get(&check).is_some_and(is_solid_structural);
                    if here_is_air && sup_solid {
                        drop_y = Some([ep[0], cy, ep[2]]);
                        break;
                    }
                }
                if let Some(dp_pos) = drop_y
                    && !near_protected_door(dp_pos, bp.door_pos)
                {
                    let rub = rubble_block_for("", arid, over, rng.next_f64());
                    let blk = if rub.ends_with("_slab") {
                        PlannedBlock::with_props(rub, vec![("type", "bottom")])
                    } else {
                        PlannedBlock::simple(rub)
                    };
                    bp.blocks.insert(dp_pos, blk);
                    changed = true;
                }
            }
        }

        // Sheltered interior cobwebs under ceiling/roof beams.
        if wear >= 0.30 {
            let candidates: Vec<[i32; 3]> = bp
                .blocks
                .iter()
                .filter_map(|(&pos, blk)| {
                    if blk.block_id != "minecraft:air" || pos[1] <= min_keep_y || near_protected_door(pos, bp.door_pos) {
                        return None;
                    }
                    let above_solid = bp.blocks.get(&[pos[0], pos[1] + 1, pos[2]]).is_some_and(is_solid_structural);
                    let side_solid = [
                        [pos[0] + 1, pos[1], pos[2]],
                        [pos[0] - 1, pos[1], pos[2]],
                        [pos[0], pos[1], pos[2] + 1],
                        [pos[0], pos[1], pos[2] - 1],
                    ]
                    .iter()
                    .any(|p| bp.blocks.get(p).is_some_and(is_solid_structural));
                    if above_solid && side_solid {
                        Some(pos)
                    } else {
                        None
                    }
                })
                .collect();
            for pos in candidates {
                if noise.cell_hash(pos[0] * 3, pos[1] * 5, pos[2] * 7) < 0.04 + wear * 0.09 {
                    bp.blocks.insert(pos, PlannedBlock::simple("minecraft:cobweb"));
                    changed = true;
                }
            }
        }
    }

    // 2. Biome-Aware Overgrowth Pass (mossy stone, climbing leaves, vine curtains, lush ground flora).
    if over > 0.04 {
        // A. Mossy stone/cobble variants weighted toward damp lower walls.
        let positions: Vec<[i32; 3]> = bp.blocks.keys().copied().collect();
        for pos in positions {
            let Some(orig) = bp.blocks.get(&pos).cloned() else {
                continue;
            };
            let id = orig.block_id.as_str();
            let dist_from_base = (pos[1] - min_keep_y).max(0) as f64;
            let height_bias = (1.15 - dist_from_base * 0.07).clamp(0.35, 1.15);
            let r = noise.cell_hash(pos[0] - 29, pos[1] + 13, pos[2] + 53);
            if let Some((mid, keep_props)) = mossy_variant(id, arid, over, height_bias, r) {
                let next_blk = if keep_props {
                    PlannedBlock::with_props(mid, orig.props)
                } else {
                    PlannedBlock::simple(mid)
                };
                bp.blocks.insert(pos, next_blk);
                changed = true;
            }
        }

        // B. Exterior wall climbing leaf mounds & ground-hugging bushes (keep bridge decks clear).
        if over >= 0.22 && bp.spec.category != crate::model::BuildCategory::Bridge {
            let dirs = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
            let mut leaf_spots = Vec::new();
            for (&pos, blk) in &bp.blocks {
                if !is_solid_structural(blk) || !(1..(h - 1)).contains(&pos[1]) {
                    continue;
                }
                if pos[0] < 1 || pos[0] >= w - 1 || pos[2] < 1 || pos[2] >= d - 1 {
                    continue;
                }
                let n = noise.fbm_3d(pos[0] as f64 * 0.42, pos[1] as f64 * 0.42, pos[2] as f64 * 0.42, 2);
                let low_bonus = if pos[1] <= min_keep_y + 3 { 0.14 } else { 0.0 };
                if n > 0.72 - over * 0.34 - low_bonus {
                    for d_off in dirs {
                        let cand = [pos[0] + d_off[0], pos[1], pos[2] + d_off[2]];
                        if (0..w).contains(&cand[0])
                            && (0..d).contains(&cand[2])
                            && !near_protected_door(cand, bp.door_pos)
                            && bp.blocks.get(&cand).is_none_or(|b| b.block_id == "minecraft:air")
                        {
                            leaf_spots.push(cand);
                        }
                    }
                }
            }
            for lp in leaf_spots {
                if rng.next_f64() < 0.45 + over * 0.35 {
                    bp.blocks.insert(lp, make_leaves(leaves_id));
                    changed = true;
                }
            }
        }

        // C. Hanging directional vine ribbons down exterior masonry & leaves!
        if over >= 0.18 && !arid {
            let vine_dirs: [(&str, i32, i32); 4] = [
                ("north", 0, -1),
                ("south", 0, 1),
                ("east", 1, 0),
                ("west", -1, 0),
            ];
            let anchors: Vec<([i32; 3], Vec<(&'static str, &'static str)>)> = bp
                .blocks
                .iter()
                .filter_map(|(&pos, blk)| {
                    if blk.block_id != "minecraft:air" && blk.block_id != leaves_id {
                        return None;
                    }
                    None.or_else(|| {
                        if blk.block_id != "minecraft:air" || pos[1] <= 2 || near_protected_door(pos, bp.door_pos) {
                            return None;
                        }
                        if !(0..w).contains(&pos[0]) || !(0..d).contains(&pos[2]) {
                            return None;
                        }
                        let mut attached_props: Vec<(&'static str, &'static str)> = Vec::new();
                        for &(face, dx, dz) in &vine_dirs {
                            let adj = [pos[0] + dx, pos[1], pos[2] + dz];
                            if bp.blocks.get(&adj).is_some_and(is_solid_structural) {
                                attached_props.push((face, "true"));
                            }
                        }
                        if attached_props.is_empty() {
                            return None;
                        }
                        let h_vine = noise.cell_hash(pos[0] + 17, pos[1] * 2, pos[2] - 23);
                        if h_vine < 0.05 + over * 0.24 {
                            Some((pos, attached_props))
                        } else {
                            None
                        }
                    })
                })
                .collect();

            for (start, props) in anchors {
                let len = 1 + (over * 4.0 * noise.cell_hash(start[0], 0, start[2])).floor() as i32;
                for dy in 0..len {
                    let p = [start[0], start[1] - dy, start[2]];
                    if p[1] < 1 || near_protected_door(p, bp.door_pos) {
                        break;
                    }
                    let is_air = bp.blocks.get(&p).is_none_or(|b| b.block_id == "minecraft:air");
                    if !is_air {
                        break;
                    }
                    bp.blocks.insert(p, PlannedBlock::with_props("minecraft:vine", props.clone()));
                    changed = true;
                }
            }
        }

        // D. Flat ground/roof moss carpets, grasses, ferns & biome wildflowers.
        if over >= 0.25 {
            let floor_spots: Vec<[i32; 3]> = bp
                .blocks
                .iter()
                .filter_map(|(&pos, blk)| {
                    if !is_solid_structural(blk) || pos[1] >= h - 1 || pos[1] < 0 {
                        return None;
                    }
                    let above = [pos[0], pos[1] + 1, pos[2]];
                    if near_protected_door(above, bp.door_pos) {
                        return None;
                    }
                    let above_air = bp.blocks.get(&above).is_none_or(|b| b.block_id == "minecraft:air");
                    if above_air { Some(above) } else { None }
                })
                .collect();

            for pos in floor_spots {
                let r = noise.cell_hash(pos[0] - 51, pos[1] + 7, pos[2] + 91);
                if r > 0.06 + over * 0.34 {
                    continue;
                }
                let kind = rng.range_i32(0, 9);
                let flora = match kind {
                    0..=4 => PlannedBlock::simple(carpet_id),
                    5..=6 => PlannedBlock::simple(flora_a),
                    7 if flora_a == "minecraft:wildflowers" || flora_a == "minecraft:pink_petals" => {
                        PlannedBlock::with_props(
                            flora_a,
                            vec![("facing", "south"), ("flower_amount", "3")],
                        )
                    }
                    _ => PlannedBlock::simple(flora_b),
                };
                bp.blocks.insert(pos, flora);
                changed = true;
            }
        }

        // E. Heavy overgrowth (>= 65%): occasional compact wild shrubs/trees reclaiming open ruin ground (skip narrow bridge roadways).
        if over >= 0.65
            && !arid
            && w >= 11
            && d >= 11
            && bp.spec.category != crate::model::BuildCategory::Bridge
        {
            let shrub_budget = ((w * d) / 140).clamp(1, 5) as usize;
            let mut placed_shrubs = 0usize;
            for _attempt in 0..(shrub_budget * 6) {
                if placed_shrubs >= shrub_budget {
                    break;
                }
                let sx = rng.range_i32(2, w - 3);
                let sz = rng.range_i32(2, d - 3);
                if let Some(dp) = bp.door_pos
                    && (sx - dp[0]).abs() <= 3
                    && (sz - dp[2]).abs() <= 3
                {
                    continue;
                }
                let ground_y = bp
                    .blocks
                    .iter()
                    .filter(|(p, b)| p[0] == sx && p[2] == sz && p[1] <= min_keep_y + 3 && is_solid_structural(b))
                    .map(|(&[_, y, _], _)| y)
                    .max();
                let Some(gy) = ground_y else {
                    continue;
                };
                if gy + 4 >= h {
                    continue;
                }
                let clear = (1..=4).all(|dy| {
                    bp.blocks
                        .get(&[sx, gy + dy, sz])
                        .is_none_or(|b| b.block_id == "minecraft:air")
                });
                if !clear {
                    continue;
                }
                bp.blocks.insert([sx, gy + 1, sz], PlannedBlock::simple(log_id));
                bp.blocks.insert([sx, gy + 2, sz], PlannedBlock::simple(log_id));
                for dz in -1i32..=1 {
                    for dx in -1i32..=1 {
                        for dy in 2..=3 {
                            if dy == 3 && dx.abs() == 1 && dz.abs() == 1 {
                                continue;
                            }
                            let cp = [sx + dx, gy + dy, sz + dz];
                            if bp.blocks.get(&cp).is_none_or(|b| b.block_id == "minecraft:air") {
                                bp.blocks.insert(cp, make_leaves(leaves_id));
                            }
                        }
                    }
                }
                bp.blocks.insert([sx, gy + 4, sz], make_leaves(leaves_id));
                placed_shrubs += 1;
                changed = true;
            }
        }
    }

    changed
}
