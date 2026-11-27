use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, clamp_i32, make_beam_block_at, make_frame_block_at,
    make_leaves, make_pillar_block_at, make_slab, make_stair, prune_map_unsupported,
    resolve_connections_and_shapes, set_uv, terrain_rel_at,
};
use crate::model::{BuildCategory, BuildRequest, BuildStyle, CardinalFacing, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::BlueprintSpec;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DungeonFlavor {
    CryptCatacombs,
    Mineshaft,
    DwarvenSanctum,
}

fn detect_dungeon_flavor(desc_lower: &str, style: BuildStyle) -> DungeonFlavor {
    if desc_lower.contains("шахт")
        || desc_lower.contains("штольн")
        || desc_lower.contains("рудник")
        || desc_lower.contains("mine")
        || desc_lower.contains("quarry")
        || style == BuildStyle::Steampunk
    {
        DungeonFlavor::Mineshaft
    } else if desc_lower.contains("дварф")
        || desc_lower.contains("гном")
        || desc_lower.contains("лава")
        || desc_lower.contains("бездна")
        || desc_lower.contains("dwarv")
        || desc_lower.contains("foundry")
        || style == BuildStyle::Nordic
    {
        DungeonFlavor::DwarvenSanctum
    } else {
        DungeonFlavor::CryptCatacombs
    }
}

fn light_block_for_dungeon(flavor: DungeonFlavor, pal: &MaterialPalette, hanging: bool) -> PlannedBlock {
    let id = if flavor == DungeonFlavor::CryptCatacombs && pal.light == "minecraft:lantern" {
        "minecraft:soul_lantern".to_owned()
    } else {
        pal.light.clone()
    };
    if id.ends_with("lantern") {
        PlannedBlock::with_props(id, vec![("hanging", if hanging { "true" } else { "false" })])
    } else {
        PlannedBlock::simple(id)
    }
}

fn dungeon_ore_block(seed: u64, u: i32, y: i32, v: i32, tier: usize) -> &'static str {
    let h = ((u as u64).wrapping_mul(0x9E37_79B9)
        ^ (y as u64).wrapping_mul(0xBF58_476D)
        ^ (v as u64).wrapping_mul(0x94D0_49BB)
        ^ seed)
        % 29;
    if tier >= 2 {
        match h {
            0 => "minecraft:deepslate_diamond_ore",
            1..=3 => "minecraft:deepslate_gold_ore",
            4..=7 => "minecraft:deepslate_redstone_ore",
            8..=13 => "minecraft:deepslate_iron_ore",
            _ => "minecraft:deepslate_coal_ore",
        }
    } else {
        match h {
            0..=2 => "minecraft:gold_ore",
            3..=8 => "minecraft:copper_ore",
            9..=16 => "minecraft:iron_ore",
            _ => "minecraft:coal_ore",
        }
    }
}

/// Generates a multi-level subterranean dungeon, branching catacomb vault, or deep mineshaft complex
/// anchored cleanly to a surface gatehouse with walkable switchback descending staircases (`y = -18..surface`).
pub(crate) fn generate_dungeon_complex(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0xD046_E040_1234_5678);
    let pal = &req.palette;
    let desc_lower = req.description.to_lowercase();
    let flavor = detect_dungeon_flavor(&desc_lower, spec.style);

    let cu = grid.w / 2;
    let cv = grid.d / 2;
    let h = i32::from(height).max(5);

    // Subterranean depth tiers fit comfortably inside preview horizon limits (`min_y >= -24`).
    let tier_count: usize = if grid.w >= 21 && grid.d >= 21 && h >= 16 {
        3
    } else if grid.w >= 13 && grid.d >= 13 && h >= 9 {
        2
    } else {
        1
    };

    let surface_base_y = if req.adapt_terrain {
        terrain_rel_at(terrain_offsets, grid, cu, cv).clamp(-3, 2)
    } else {
        0
    };

    // 1. Subterranean Tier Excavation & Structural Shells (`tier = 0..tier_count`).
    let min_x = if grid.w >= 11 { 1 } else { 0 };
    let max_x = if grid.w >= 11 { grid.w - 2 } else { grid.w - 1 };
    let min_z = if grid.d >= 11 { 1 } else { 0 };
    let max_z = if grid.d >= 11 { grid.d - 2 } else { grid.d - 1 };

    let tier_floors: Vec<i32> = (0..tier_count)
        .map(|t| surface_base_y - 6 * (t as i32 + 1))
        .collect();

    for (t_idx, &floor_y) in tier_floors.iter().enumerate() {
        let ceil_y = floor_y + 4;
        let roof_y = floor_y + 5;
        let wall_mat = |u: i32, y: i32, v: i32| -> String {
            if flavor == DungeonFlavor::Mineshaft && (noise.cell_hash(u, y, v) > 0.84) {
                dungeon_ore_block(seed, u, y, v, t_idx).to_owned()
            } else if t_idx >= 1 || flavor == DungeonFlavor::DwarvenSanctum {
                if (u + y + v).abs() % 7 == 0 {
                    "minecraft:cracked_deepslate_bricks".to_owned()
                } else {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                }
            } else if (u + v + y).abs() % 6 == 0 {
                "minecraft:cracked_stone_bricks".to_owned()
            } else {
                pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
            }
        };

        // Determine walkable rooms & cross corridors for this subterranean level.
        let hub_ru = clamp_i32((max_x - min_x) / 5, 1, 4);
        let hub_rv = clamp_i32((max_z - min_z) / 5, 1, 4);
        let corr_hw = if grid.w >= 15 && grid.d >= 15 { 1 } else { 0 };

        // Optional corner chambers carved on spacious footprints (`>= 17x17`).
        let corner_rooms = grid.w >= 17 && grid.d >= 17;
        let room_rad = clamp_i32((max_x - min_x).min(max_z - min_z) / 6, 2, 4);
        let room_centers = [
            (min_x + room_rad + 1, min_z + room_rad + 1, 0usize), // NW: Library / Study Vault
            (max_x - room_rad - 1, min_z + room_rad + 1, 1usize), // NE: Armory / Smithy Crypt
            (min_x + room_rad + 1, max_z - room_rad - 1, 2usize), // SW: Lush Mossy Fountain Shrine
            (max_x - room_rad - 1, max_z - room_rad - 1, 3usize), // SE: Boss Treasury / Alchemy Sanctum
        ];

        let is_carved = |u: i32, v: i32| -> bool {
            if u <= min_x || u >= max_x || v <= min_z || v >= max_z {
                return false;
            }
            if (u - cu).abs() <= hub_ru && (v - cv).abs() <= hub_rv {
                return true;
            }
            // Cardinal branching corridors from hub out toward walls / end rooms.
            let n_reach = (min_z + 2).max(cv - 2);
            let s_reach = (max_z - 2).min(cv + 2);
            let w_reach = (min_x + 2).max(cu - 2);
            let e_reach = (max_x - 2).min(cu + 2);
            let in_ns = (u - cu).abs() <= corr_hw
                && v >= if corner_rooms { min_z + 2 } else { n_reach }
                && v <= if corner_rooms { max_z - 2 } else { s_reach };
            let in_ew = (v - cv).abs() <= corr_hw
                && u >= if corner_rooms { min_x + 2 } else { w_reach }
                && u <= if corner_rooms { max_x - 2 } else { e_reach };
            if in_ns || in_ew {
                return true;
            }
            if corner_rooms {
                for &(rx, rz, r_id) in &room_centers {
                    let active_on_tier = (t_idx + r_id) % 2 == 0 || t_idx == tier_count - 1;
                    if !active_on_tier {
                        continue;
                    }
                    if (u - rx).abs() <= room_rad && (v - rz).abs() <= room_rad {
                        return true;
                    }
                    // Branch tunnel linking main cardinal avenue to each corner vault room.
                    let in_branch = (u >= rx.min(cu) && u <= rx.max(cu) && (v - rz).abs() <= 1)
                        || (v >= rz.min(cv) && v <= rz.max(cv) && (u - rx).abs() <= 1);
                    if in_branch {
                        return true;
                    }
                }
            }
            false
        };

        // Pass A: Enclose solid underground tier volume + excavate rooms & vaulted ceiling coffers.
        for v in min_z..=max_z {
            for u in min_x..=max_x {
                let open = is_carved(u, v);
                if open {
                    let floor_blk = if (u + v) % 4 == 0 {
                        pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, floor_y, v)
                    } else if t_idx >= 1 {
                        "minecraft:polished_deepslate".to_owned()
                    } else {
                        "minecraft:stone_bricks".to_owned()
                    };
                    set_uv(&mut blocks, grid, u, floor_y, v, PlannedBlock::simple(floor_blk));
                    set_uv(&mut blocks, grid, u, roof_y, v, PlannedBlock::simple(wall_mat(u, roof_y, v)));

                    // Vaulted bevel at ceiling perimeter & solid air inside.
                    let edge_adj = !is_carved(u - 1, v) || !is_carved(u + 1, v) || !is_carved(u, v - 1) || !is_carved(u, v + 1);
                    for y in (floor_y + 1)..=ceil_y {
                        if y == ceil_y && edge_adj && flavor != DungeonFlavor::Mineshaft {
                            set_uv(&mut blocks, grid, u, y, v, make_slab(&pal.roof_slab, "top"));
                        } else {
                            set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                        }
                    }

                    // Mineshaft timber arches & rails every 3 tiles along straight tunnels!
                    if flavor == DungeonFlavor::Mineshaft {
                        let on_ns = (u - cu).abs() <= corr_hw && (v - cv).abs() > hub_rv;
                        let on_ew = (v - cv).abs() <= corr_hw && (u - cu).abs() > hub_ru;
                        if (on_ns && v % 3 == 0) || (on_ew && u % 3 == 0) {
                            if (on_ns && (u - cu).abs() == corr_hw) || (on_ew && (v - cv).abs() == corr_hw) {
                                for y in (floor_y + 1)..ceil_y {
                                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.fence.clone()));
                                }
                            }
                            set_uv(&mut blocks, grid, u, ceil_y, v, make_beam_block_at(pal, if on_ns { grid.axis_u() } else { grid.axis_v() }, seed, u, ceil_y, v));
                            if (u == cu || v == cv) && ceil_y - 1 >= floor_y + 3 {
                                set_uv(&mut blocks, grid, u, ceil_y - 1, v, light_block_for_dungeon(flavor, pal, true));
                            }
                        } else if on_ns && u == cu {
                            set_uv(&mut blocks, grid, u, floor_y + 1, v, PlannedBlock::simple("minecraft:rail"));
                        } else if on_ew && v == cv {
                            set_uv(&mut blocks, grid, u, floor_y + 1, v, PlannedBlock::simple("minecraft:rail"));
                        }
                    } else if (u == cu || v == cv) && ((u - cu).abs() + (v - cv).abs()) % 5 == 4 {
                        set_uv(&mut blocks, grid, u, ceil_y, v, light_block_for_dungeon(flavor, pal, true));
                    }
                } else {
                    for y in floor_y..=roof_y {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(wall_mat(u, y, v)));
                    }
                }
            }
        }

        // Hub corner pillars + Dwarven caged lava vents + cozy ceiling cobwebs in corners.
        for &(pu, pv) in &[
            (cu - hub_ru, cv - hub_rv),
            (cu + hub_ru, cv - hub_rv),
            (cu - hub_ru, cv + hub_rv),
            (cu + hub_ru, cv + hub_rv),
        ] {
            if is_carved(pu, pv) && (pu - cu).abs() >= 2 && (pv - cv).abs() >= 2 {
                for y in (floor_y + 1)..=ceil_y {
                    set_uv(&mut blocks, grid, pu, y, pv, make_pillar_block_at(pal, "y", seed, pu, y, pv));
                }
                if (pu + pv) % 2 == 0 {
                    set_uv(&mut blocks, grid, pu, floor_y + 2, pv, light_block_for_dungeon(flavor, pal, false));
                }
            }
        }

        if flavor == DungeonFlavor::DwarvenSanctum && corner_rooms {
            for &(mu, mv) in &[(cu - 2, cv), (cu + 2, cv)] {
                if is_carved(mu, mv) {
                    set_uv(&mut blocks, grid, mu, floor_y - 2, mv, PlannedBlock::simple(wall_mat(mu, floor_y - 2, mv)));
                    set_uv(&mut blocks, grid, mu, floor_y - 1, mv, PlannedBlock::simple("minecraft:lava"));
                    set_uv(&mut blocks, grid, mu, floor_y, mv, PlannedBlock::simple("minecraft:iron_bars"));
                }
            }
        }

        // Decorate Terminal Corner Rooms (Loot Chests, Barrels, Enchanter, Brewing, Shrine, Traps).
        if corner_rooms {
            for &(rx, rz, r_id) in &room_centers {
                if !is_carved(rx, rz) {
                    continue;
                }
                match r_id {
                    0 => {
                        // Secret Library & Arcane Vault
                        if is_carved(rx, rz) {
                            set_uv(&mut blocks, grid, rx, floor_y + 1, rz, PlannedBlock::simple("minecraft:enchanting_table"));
                        }
                        if is_carved(rx - 1, rz - 1) {
                            set_uv(&mut blocks, grid, rx - 1, floor_y + 1, rz - 1, PlannedBlock::simple("minecraft:bookshelf"));
                            set_uv(&mut blocks, grid, rx - 1, floor_y + 2, rz - 1, PlannedBlock::simple("minecraft:bookshelf"));
                        }
                        if is_carved(rx + 1, rz - 1) {
                            set_uv(&mut blocks, grid, rx + 1, floor_y + 1, rz - 1, PlannedBlock::with_props("minecraft:chest", vec![("facing", grid.dir_front()), ("type", "single")]));
                        }
                    }
                    1 => {
                        // Guard Armory & Dwarven Forge Crypt
                        set_uv(&mut blocks, grid, rx, floor_y + 1, rz, PlannedBlock::with_props("minecraft:anvil", vec![("facing", grid.dir_front())]));
                        if is_carved(rx + 1, rz - 1) {
                            set_uv(&mut blocks, grid, rx + 1, floor_y + 1, rz - 1, PlannedBlock::with_props("minecraft:blast_furnace", vec![("facing", grid.dir_front()), ("lit", "false")]));
                        }
                        if is_carved(rx - 1, rz) {
                            set_uv(&mut blocks, grid, rx - 1, floor_y + 1, rz, PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
                        }
                        if is_carved(rx, rz - 1) {
                            set_uv(&mut blocks, grid, rx, floor_y + 1, rz - 1, PlannedBlock::with_props("minecraft:chest", vec![("facing", grid.dir_front()), ("type", "single")]));
                        }
                    }
                    2 => {
                        // Subterranean Glow-Moss Water Cistern
                        set_uv(&mut blocks, grid, rx, floor_y, rz, PlannedBlock::simple("minecraft:water"));
                        set_uv(&mut blocks, grid, rx, floor_y - 1, rz, PlannedBlock::simple("minecraft:sea_lantern"));
                        if is_carved(rx - 1, rz) {
                            set_uv(&mut blocks, grid, rx - 1, floor_y, rz, PlannedBlock::simple("minecraft:moss_block"));
                        }
                        if is_carved(rx + 1, rz) {
                            set_uv(&mut blocks, grid, rx + 1, floor_y + 1, rz, PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
                        }
                    }
                    _ => {
                        // Deep Boss Sanctum & Alchemist Pedestal with Safe Trial Floor!
                        set_uv(&mut blocks, grid, rx, floor_y + 1, rz, PlannedBlock::simple("minecraft:gilded_blackstone"));
                        set_uv(&mut blocks, grid, rx, floor_y + 2, rz, PlannedBlock::with_props("minecraft:brewing_stand", vec![("has_bottle_0", "true"), ("has_bottle_1", "true"), ("has_bottle_2", "true")]));
                        if is_carved(rx + 1, rz) {
                            set_uv(&mut blocks, grid, rx + 1, floor_y + 1, rz, PlannedBlock::with_props("minecraft:chest", vec![("facing", grid.dir_left()), ("type", "single")]));
                        }
                        if is_carved(rx - 1, rz) {
                            set_uv(&mut blocks, grid, rx - 1, floor_y + 1, rz, PlannedBlock::with_props("minecraft:chest", vec![("facing", grid.dir_right()), ("type", "single")]));
                        }
                        // Flavor pressure plate warning step on approach!
                        if is_carved(rx, rz - 1) {
                            set_uv(&mut blocks, grid, rx, floor_y + 1, rz - 1, PlannedBlock::simple("minecraft:stone_pressure_plate"));
                        }
                    }
                }
                set_uv(&mut blocks, grid, rx, ceil_y, rz, light_block_for_dungeon(flavor, pal, true));
                if flavor != DungeonFlavor::DwarvenSanctum && is_carved(rx - 1, rz + 1) && rng.next_f64() < 0.65 {
                    set_uv(&mut blocks, grid, rx - 1, ceil_y, rz + 1, PlannedBlock::simple("minecraft:cobweb"));
                }
            }
        } else {
            // Compact dungeon chest & barrel right inside main chamber!
            let c_u = (cu + 1).min(max_x - 1);
            let c_v = (cv - 1).max(min_z + 1);
            if is_carved(c_u, c_v) {
                set_uv(&mut blocks, grid, c_u, floor_y + 1, c_v, PlannedBlock::with_props("minecraft:chest", vec![("facing", grid.dir_front()), ("type", "single")]));
            }
            if is_carved(cu - 1, c_v) {
                set_uv(&mut blocks, grid, cu - 1, floor_y + 1, c_v, PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
            }
        }
    }

    // 2. Surface Entrance Gatehouse / Mine Pithead Canopy (`y = surface_base_y ..= surface_base_y + gate_h`).
    let gate_h = (h - 2).clamp(4, 7);
    let gate_ru = ((max_x - min_x) / 4).clamp(2, 3);
    let gate_rv = ((max_z - min_z) / 4).clamp(2, 3);
    for v in (cv - gate_rv).max(0)..=(cv + gate_rv).min(grid.d - 1) {
        for u in (cu - gate_ru).max(0)..=(cu + gate_ru).min(grid.w - 1) {
            let shell = u == (cu - gate_ru).max(0)
                || u == (cu + gate_ru).min(grid.w - 1)
                || v == (cv - gate_rv).max(0)
                || v == (cv + gate_rv).min(grid.d - 1);
            set_uv(&mut blocks, grid, u, surface_base_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, surface_base_y, v)));
            if req.adapt_terrain {
                let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-8, surface_base_y);
                for fy in terr..surface_base_y {
                    set_uv(&mut blocks, grid, u, fy, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, fy, v)));
                }
            }
            for y in (surface_base_y + 1)..=(surface_base_y + gate_h) {
                if y == surface_base_y + gate_h {
                    set_uv(&mut blocks, grid, u, y, v, make_slab(&pal.roof_slab, "bottom"));
                } else if shell {
                    let is_corner = (u == (cu - gate_ru).max(0) || u == (cu + gate_ru).min(grid.w - 1))
                        && (v == (cv - gate_rv).max(0) || v == (cv + gate_rv).min(grid.d - 1));
                    let blk = if is_corner {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if flavor == DungeonFlavor::Mineshaft && y == surface_base_y + 2 {
                        PlannedBlock::simple(pal.fence.clone())
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Carve South doorway opening at `cv + gate_rv` cleanly to outside world!
    let front_v = (cv + gate_rv).min(grid.d - 1);
    for dy in 1..=2 {
        set_uv(&mut blocks, grid, cu, surface_base_y + dy, front_v, PlannedBlock::simple("minecraft:air"));
        if front_v + 1 < grid.d {
            set_uv(&mut blocks, grid, cu, surface_base_y + dy, front_v + 1, PlannedBlock::simple("minecraft:air"));
        }
    }
    set_uv(&mut blocks, grid, cu, surface_base_y + 3, front_v, make_stair(&pal.roof_stairs, grid.dir_front(), "top"));
    for side in [-1, 1] {
        let lu = cu + side;
        if grid.in_bounds(lu, front_v + 1) {
            set_uv(&mut blocks, grid, lu, surface_base_y + 1, front_v + 1, PlannedBlock::simple(pal.fence.clone()));
            set_uv(&mut blocks, grid, lu, surface_base_y + 2, front_v + 1, light_block_for_dungeon(flavor, pal, false));
        }
    }
    if surface_base_y + gate_h - 1 >= surface_base_y + 3 {
        set_uv(&mut blocks, grid, cu, surface_base_y + gate_h - 1, cv, light_block_for_dungeon(flavor, pal, true));
    }

    // 3. Walkable Switchback Stone Staircase Linking Surface -> All Subterranean Tiers!
    // Uses safe zig-zag stair flight along `u = cu` / `cu - 1` so there is ALWAYS 3-block headroom (`air`).
    let can_switchback = (max_x - min_x) >= 6;
    for t_idx in 0..tier_count {
        let top_y = if t_idx == 0 {
            surface_base_y
        } else {
            tier_floors[t_idx - 1]
        };
        let bot_y = tier_floors[t_idx];
        let dir_u: i32 = if t_idx % 2 == 0 { 1 } else { -1 };
        let stair_face = grid.uv_to_mc_dir(-dir_u, 0);
        let stair_v = (cv - 1).max(min_z + 1);

        if can_switchback {
            let steps = top_y - bot_y; // 6 steps
            let start_u = if dir_u > 0 {
                (cu - 2).max(min_x + 1)
            } else {
                (cu + 2).min(max_x - 1)
            };
            for s in 0..=steps {
                let cur_u = clamp_i32(start_u + dir_u * s.min(5), min_x + 1, max_x - 1);
                let cur_y = top_y - s;
                if s > 0 && s < steps {
                    set_uv(
                        &mut blocks,
                        grid,
                        cur_u,
                        cur_y,
                        stair_v,
                        make_stair(&pal.pick_stair(MaterialPalette::SLOT_STAIR_BLOCK, seed, cur_u, cur_y, stair_v), stair_face, "bottom"),
                    );
                } else {
                    set_uv(
                        &mut blocks,
                        grid,
                        cur_u,
                        cur_y,
                        stair_v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, cur_u, cur_y, stair_v)),
                    );
                }
                set_uv(
                    &mut blocks,
                    grid,
                    cur_u,
                    cur_y - 1,
                    stair_v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, cur_u, cur_y - 1, stair_v)),
                );
                for cy in (cur_y + 1)..=(cur_y + 3) {
                    set_uv(&mut blocks, grid, cur_u, cy, stair_v, PlannedBlock::simple("minecraft:air"));
                }
                if stair_v + 1 <= max_z - 1 && cur_y == top_y {
                    set_uv(&mut blocks, grid, cur_u, top_y + 1, stair_v + 1, PlannedBlock::simple(pal.fence.clone()));
                }
            }
        } else {
            // Compact vertical ladder shaft with landing for minimum `7x7x5` dimensions!
            let lad_v = (cv - 1).max(min_z + 1);
            for y in (bot_y + 1)..=(top_y + 1) {
                set_uv(&mut blocks, grid, cu, y, lad_v - 1, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, cu, y, lad_v - 1)));
                set_uv(&mut blocks, grid, cu, y, lad_v, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
                if lad_v + 1 <= max_z {
                    set_uv(&mut blocks, grid, cu, y, lad_v + 1, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    let _ = rng;
    prune_map_unsupported(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);
    let [dlx, dlz] = grid.to_local_xz(cu, front_v);

    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: grid.orig_w as u16,
        depth: grid.orig_d as u16,
        height,
        blocks,
        door_pos: Some([dlx, surface_base_y + 1, dlz]),
    }
}

/// Generates a cohesive rural manor compound / farmstead ensemble:
/// perimeter fence & gate, cobbled estate lanes, Manor House, Barn, irrigated tilled crop plots,
/// fenced livestock paddock, and Windmill or orchard pond when footprint permits (`>= 26x26`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_estate_compound(
    req: &BuildRequest,
    spec: BlueprintSpec,
    _noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) -> GeneratedBlueprint {
    let w = i32::from(req.width.clamp(7, 96));
    let d = i32::from(req.depth.clamp(7, 96));
    let h = req.height.clamp(5, 48);
    let seed = req.seed as u64;
    let rng = SplitMix64::new(seed ^ 0xE57A_7E00_1122_3344);
    let pal = &req.palette;

    let terr_at = |x: i32, z: i32| -> i32 {
        if !req.adapt_terrain {
            return 0;
        }
        let Some(off) = terrain_offsets else {
            return 0;
        };
        let ix = clamp_i32(x, 0, w - 1) as usize;
        let iz = clamp_i32(z, 0, d - 1) as usize;
        off.get(iz * (w as usize) + ix).copied().unwrap_or(0).clamp(-4, 4)
    };
    let slice_2d = |src: Option<&[i32]>, rx0: i32, rz0: i32, rw: i32, rd: i32, base: i32, rel: bool| -> Option<Vec<i32>> {
        let data = src?;
        let mut out = Vec::with_capacity((rw * rd) as usize);
        for dz in 0..rd {
            for dx in 0..rw {
                let wx = clamp_i32(rx0 + dx, 0, w - 1) as usize;
                let wz = clamp_i32(rz0 + dz, 0, d - 1) as usize;
                let val = data.get(wz * (w as usize) + wx).copied().unwrap_or(0);
                out.push(if rel { val - base } else { val });
            }
        }
        Some(out)
    };

    // Compact `< 21x21` estate: generate cozy farmhouse + attached fenced vegetable plot cleanly!
    if w < 21 || d < 21 {
        let mut sub_req = req.clone();
        sub_req.category = BuildCategory::House;
        // Strip outer estate keywords so child sub-buildings never recurse back into `BuildCategory::Estate`.
        sub_req.description = "cozy farmhouse cottage with porch and chimney".to_owned();
        sub_req.wear_pct = 0;
        sub_req.overgrowth_pct = 0;
        if w >= 13 && d >= 13 {
            sub_req.width = ((w - 4).clamp(7, 13)) as u16;
            sub_req.depth = ((d - 4).clamp(7, 11)) as u16;
            let ox = (w - i32::from(sub_req.width)) / 2;
            let base_y = terr_at(ox + i32::from(sub_req.width) / 2, i32::from(sub_req.depth) - 1);
            let sub_off = slice_2d(terrain_offsets, ox, 0, i32::from(sub_req.width), i32::from(sub_req.depth), base_y, true);
            let sub_wd = slice_2d(water_depths, ox, 0, i32::from(sub_req.width), i32::from(sub_req.depth), base_y, false);
            let bp = crate::generator::generate_blueprint_with_water(&sub_req, sub_off.as_deref(), sub_wd.as_deref());
            let mut blocks = BTreeMap::new();
            for ([sx, sy, sz], blk) in bp.blocks {
                blocks.insert([ox + sx, base_y + sy, sz], blk);
            }
            // Add front garden plot along south yard!
            let plot_z0 = i32::from(sub_req.depth) + 1;
            if plot_z0 + 1 < d {
                for px in 1..(w - 1) {
                    let py = terr_at(px, plot_z0);
                    if (px - w / 2).abs() <= 1 {
                        blocks.insert([px, py, plot_z0], PlannedBlock::simple(pal.paving.clone()));
                        continue;
                    }
                    let is_water = px == 2 || px == w - 3;
                    if is_water {
                        blocks.insert([px, py, plot_z0], PlannedBlock::simple("minecraft:water"));
                    } else {
                        blocks.insert([px, py, plot_z0], PlannedBlock::with_props("minecraft:farmland", vec![("moisture", "7")]));
                        blocks.insert([px, py + 1, plot_z0], PlannedBlock::with_props("minecraft:wheat", vec![("age", "7")]));
                    }
                }
            }
            prune_map_unsupported(&mut blocks);
            resolve_connections_and_shapes(&mut blocks);
            return GeneratedBlueprint {
                spec,
                origin: [req.origin_x, req.origin_y, req.origin_z],
                width: w as u16,
                depth: d as u16,
                height: h,
                blocks,
                door_pos: bp.door_pos.map(|[dx, dy, dz]| [ox + dx, base_y + dy, dz]),
            };
        }
        let mut fallback = crate::generator::generate_blueprint_with_water(&sub_req, terrain_offsets, water_depths);
        fallback.spec = spec;
        return fallback;
    }

    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();

    let cx = w / 2;
    let cz = d / 2;

    // 1. Rural Estate Perimeter Fence & Arched Carriage Gate (`South`, z = d - 1).
    for z in 0..d {
        for x in 0..w {
            let on_edge = x == 0 || x == w - 1 || z == 0 || z == d - 1;
            if !on_edge {
                continue;
            }
            let by = terr_at(x, z);
            let is_south_gate = z == d - 1 && (x - cx).abs() <= 1;
            if is_south_gate {
                blocks.insert([x, by, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, x, by, z)));
                for ay in 1..=3 {
                    blocks.insert([x, by + ay, z], PlannedBlock::simple("minecraft:air"));
                }
                if (x - cx).abs() == 0 {
                    blocks.insert([x, by + 4, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_BEAM, seed, x, by + 4, z)));
                }
                continue;
            }
            let is_post = (x == 0 || x == w - 1) && (z == 0 || z == d - 1)
                || (z == d - 1 && (x - cx).abs() == 2)
                || (x + z) % 6 == 0;
            if is_post {
                blocks.insert([x, by + 1, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, x, by + 1, z)));
                blocks.insert([x, by + 2, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, x, by + 2, z)));
                if (z == d - 1 && (x - cx).abs() == 2) || ((x == 0 || x == w - 1) && (z == 0 || z == d - 1)) {
                    blocks.insert([x, by + 3, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, x, by + 3, z)));
                }
            } else {
                blocks.insert([x, by + 1, z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, x, by + 1, z)));
            }
        }
    }

    // 2. Winding Main Estate Lane (`cx - 1 ..= cx + 1` North-South + East-West Crossroads at `cz`).
    let mut prev_ry = terr_at(cx, d - 1);
    for z in (3..(d - 1)).rev() {
        let raw_y = terr_at(cx, z);
        let ry = if raw_y > prev_ry {
            prev_ry + 1
        } else if raw_y < prev_ry {
            prev_ry - 1
        } else {
            raw_y
        };
        prev_ry = ry;
        for dx in -1..=1 {
            let rx = cx + dx;
            let fill_col = if (rx + z) % 5 == 0 { "minecraft:coarse_dirt".to_owned() } else { pal.pick(MaterialPalette::SLOT_PAVING, seed, rx, ry, z) };
            blocks.insert([rx, ry, z], PlannedBlock::simple(fill_col));
            for ay in 1..=3 {
                blocks.insert([rx, ry + ay, z], PlannedBlock::simple("minecraft:air"));
            }
        }
    }
    for x in 3..(w - 3) {
        let ry = terr_at(x, cz);
        for dz in -1..=1 {
            let rz = cz + dz;
            let fill_col = if (x + rz) % 5 == 0 { "minecraft:gravel".to_owned() } else { pal.pick(MaterialPalette::SLOT_PAVING, seed, x, ry, rz) };
            blocks.insert([x, ry, rz], PlannedBlock::simple(fill_col));
            for ay in 1..=3 {
                blocks.insert([x, ry + ay, rz], PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Helper closure to stamp a full procedural sub-building cleanly into an estate quadrant.
    let mut stamp_building = |bx0: i32, bz0: i32, bw: u16, bd: u16, bh: u16, cat: BuildCategory, facing: CardinalFacing, prompt: &str, seed_salt: u64| {
        let ilw = i32::from(bw);
        let ild = i32::from(bd);
        let base_y = terr_at(bx0 + ilw / 2, bz0 + ild - 1);
        let sub_off = slice_2d(terrain_offsets, bx0, bz0, ilw, ild, base_y, true);
        let sub_wd = slice_2d(water_depths, bx0, bz0, ilw, ild, base_y, false);
        let sub_req = BuildRequest {
            dimension: req.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: bw,
            depth: bd,
            height: bh,
            category: cat,
            style: spec.style,
            facing,
            noise_level: req.noise_level,
            seed: (seed ^ seed_salt) as i64,
            description: prompt.to_owned(),
            adapt_terrain: req.adapt_terrain,
            air_only: req.air_only,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: req.biome_hint.clone(),
            palette: pal.clone(),
        };
        let _keep_desc_ref = req.description.len();
        let sub_bp = crate::generator::generate_blueprint_with_water(&sub_req, sub_off.as_deref(), sub_wd.as_deref());
        for ([sx, sy, sz], blk) in sub_bp.blocks {
            let wx = bx0 + sx;
            let wy = base_y + sy;
            let wz = bz0 + sz;
            if (1..(w - 1)).contains(&wx) && (1..(d - 1)).contains(&wz) {
                blocks.insert([wx, wy, wz], blk);
            }
        }
        // Connect building door to nearest estate avenue along facing axis!
        if let Some([dlx, dly, dlz]) = sub_bp.door_pos {
            let mut px = bx0 + dlx;
            let mut pz = bz0 + dlz;
            let (sx_step, sz_step) = match facing {
                CardinalFacing::South => (0, 1),
                CardinalFacing::North => (0, -1),
                CardinalFacing::East => (1, 0),
                CardinalFacing::West => (-1, 0),
            };
            let mut path_y = base_y + dly - 1;
            for _ in 0..10 {
                px += sx_step;
                pz += sz_step;
                if !(2..(w - 2)).contains(&px) || !(2..(d - 2)).contains(&pz) {
                    break;
                }
                let ty = terr_at(px, pz);
                path_y = if ty > path_y { path_y + 1 } else if ty < path_y { path_y - 1 } else { ty };
                if blocks.get(&[px, path_y + 1, pz]).is_none_or(|b| b.block_id == "minecraft:air") {
                    blocks.insert([px, path_y, pz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, px, path_y, pz)));
                    blocks.insert([px, path_y + 1, pz], PlannedBlock::simple("minecraft:air"));
                    blocks.insert([px, path_y + 2, pz], PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    };

    // 3. Manor House (NW/North Zone) + Barn (NE Zone) + Optional Windmill (Far NE/NW Edge).
    let manor_w = ((cx - 3).clamp(7, 17)) as u16;
    let manor_d = ((cz - 3).clamp(7, 15)) as u16;
    let manor_h = h.clamp(11, 20);
    let manor_x = ((cx - 1 - i32::from(manor_w)).max(2)) as i32;
    let manor_z = ((cz - 1 - i32::from(manor_d)).max(2)) as i32;
    stamp_building(
        manor_x,
        manor_z,
        manor_w,
        manor_d,
        manor_h,
        BuildCategory::House,
        CardinalFacing::South,
        "manor house with porch fireplace and kitchen",
        0x1111_2222_3333_4444,
    );

    let barn_avail_w = w - 2 - (cx + 2);
    let barn_avail_d = cz - 3;
    let built_barn = barn_avail_w >= 9 && barn_avail_d >= 7;
    if built_barn {
        let barn_w = (barn_avail_w.clamp(8, 15)) as u16;
        let barn_d = (barn_avail_d.clamp(7, 13)) as u16;
        stamp_building(
            cx + 2,
            (cz - 1 - i32::from(barn_d)).max(2),
            barn_w,
            barn_d,
            h.clamp(10, 15),
            BuildCategory::Barn,
            CardinalFacing::South,
            "timber farm barn with loft and stables",
            0x5555_6666_7777_8888,
        );
    }

    // Windmill overlooking the compound (SE East flank) when footprint >= 34x34!
    let mut windmill_x: Option<i32> = None;
    if w >= 34 && d >= 34 {
        let wm_w = 9u16;
        let wm_d = 9u16;
        let wm_x = w - 10;
        let wm_z = cz + 2;
        if wm_x - (cx + 2) >= 5 && wm_z + i32::from(wm_d) <= d - 2 {
            stamp_building(
                wm_x,
                wm_z,
                wm_w,
                wm_d,
                h.clamp(15, 24),
                BuildCategory::Tower,
                CardinalFacing::North,
                "windmill with sails",
                0x9999_AAAA_BBBB_CCCC,
            );
            windmill_x = Some(wm_x);
        }
    }

    if !built_barn && barn_avail_w >= 4 && barn_avail_d >= 4 {
        // Cozy stone garden well / orchard courtyard in NE when footprint is compact (`21..24`).
        let wx = (cx + 2 + w - 2) / 2;
        let wz = (2 + cz - 2) / 2;
        let wy = terr_at(wx, wz);
        for dz in -1_i32..=1 {
            for dx in -1_i32..=1 {
                let is_center = dx == 0 && dz == 0;
                if is_center {
                    blocks.insert([wx, wy, wz], PlannedBlock::simple("minecraft:water"));
                    blocks.insert([wx, wy + 1, wz], PlannedBlock::simple("minecraft:air"));
                    blocks.insert([wx, wy + 3, wz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, wx, wy + 3, wz)));
                } else {
                    blocks.insert([wx + dx, wy + 1, wz + dz], make_slab(&pal.roof_slab, "bottom"));
                    if dx.abs() == 1 && dz.abs() == 1 {
                        blocks.insert([wx + dx, wy + 2, wz + dz], PlannedBlock::simple(pal.fence.clone()));
                        blocks.insert([wx + dx, wy + 3, wz + dz], make_slab(&pal.roof_slab, "bottom"));
                    }
                }
            }
        }
    }

    // 4. Irrigated Crop Fields (SW Quadrant) with Water Channels, Crops, Composters & Scarecrow!
    let field_x0 = 2;
    let field_x1 = cx - 2;
    let field_z0 = cz + 2;
    let field_z1 = d - 3;
    if field_x1 - field_x0 >= 4 && field_z1 - field_z0 >= 4 {
        let crop_types = ["minecraft:wheat", "minecraft:carrots", "minecraft:potatoes", "minecraft:beetroots"];
        for z in field_z0..=field_z1 {
            let is_water_row = (z - field_z0) % 5 == 2;
            for x in field_x0..=field_x1 {
                let fy = terr_at(field_x0, field_z0);
                if req.adapt_terrain {
                    let cur_t = terr_at(x, z);
                    for ddy in cur_t.min(fy - 2)..fy {
                        blocks.insert([x, ddy, z], PlannedBlock::simple("minecraft:dirt"));
                    }
                    for ay in (fy + 1)..=(fy + 3) {
                        blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
                    }
                }
                if is_water_row && x > field_x0 && x < field_x1 {
                    blocks.insert([x, fy, z], PlannedBlock::simple("minecraft:water"));
                    if (x - field_x0) % 3 == 0 {
                        blocks.insert([x, fy + 1, z], make_slab(&pal.roof_slab, "bottom"));
                    }
                } else if z == field_z0 || x == field_x0 {
                    blocks.insert([x, fy, z], make_beam_block_at(pal, "x", seed, x, fy, z));
                } else {
                    blocks.insert([x, fy, z], PlannedBlock::with_props("minecraft:farmland", vec![("moisture", "7")]));
                    let crop = crop_types[((z - field_z0) / 5) as usize % crop_types.len()];
                    let age = if crop == "minecraft:beetroots" { "3" } else { "7" };
                    blocks.insert([x, fy + 1, z], PlannedBlock::with_props(crop, vec![("age", age)]));
                }
            }
        }
        // Scarecrow & Farmer Supply Basket at corner of crop field!
        let scx = (field_x0 + field_x1) / 2;
        let scz = field_z0 + 1;
        let scy = terr_at(field_x0, field_z0);
        if scx > field_x0 && scz <= field_z1 {
            blocks.insert([scx, scy, scz], PlannedBlock::simple("minecraft:hay_block"));
            blocks.insert([scx, scy + 1, scz], PlannedBlock::simple(pal.fence.clone()));
            blocks.insert([scx, scy + 2, scz], PlannedBlock::with_props("minecraft:jack_o_lantern", vec![("facing", "south")]));
        }
        if field_z0 <= field_z1 {
            let cy = terr_at(field_x0, field_z0);
            blocks.insert([field_x0, cy + 1, field_z0], PlannedBlock::simple("minecraft:composter"));
            if field_x0 + 1 <= field_x1 {
                blocks.insert([field_x0 + 1, cy + 1, field_z0], PlannedBlock::with_props("minecraft:barrel", vec![("facing", "up")]));
            }
        }
    }

    // 5. Fenced Animal Livestock Paddock (SE Quadrant) with Shelter Roof, Water Trough & Hay Feeder!
    let pen_x0 = cx + 2;
    let pen_x1 = match windmill_x {
        Some(wx) => (wx - 2).min(w - 3),
        None => w - 3,
    };
    let pen_z0 = cz + 2;
    let pen_z1 = d - 3;
    if pen_x1 - pen_x0 >= 3 && pen_z1 - pen_z0 >= 4 {
        let pen_y = terr_at((pen_x0 + pen_x1) / 2, (pen_z0 + pen_z1) / 2);
        for z in pen_z0..=pen_z1 {
            for x in pen_x0..=pen_x1 {
                let border = x == pen_x0 || x == pen_x1 || z == pen_z0 || z == pen_z1;
                blocks.insert(
                    [x, pen_y, z],
                    PlannedBlock::simple(if !border && (x + z) % 3 == 0 {
                        "minecraft:coarse_dirt"
                    } else {
                        "minecraft:grass_block"
                    }),
                );
                for ay in (pen_y + 1)..=(pen_y + 4) {
                    blocks.insert([x, ay, z], PlannedBlock::simple("minecraft:air"));
                }
                if border {
                    if x == pen_x0 && z == (pen_z0 + pen_z1) / 2 {
                        blocks.insert([x, pen_y + 1, z], PlannedBlock::with_props(pal.fence_gate.clone(), vec![("facing", "east"), ("open", "false")]));
                    } else {
                        blocks.insert([x, pen_y + 1, z], PlannedBlock::simple(pal.fence.clone()));
                        if (x == pen_x0 || x == pen_x1) && (z == pen_z0 || z == pen_z1) {
                            blocks.insert([x, pen_y + 2, z], light_block_for_dungeon(DungeonFlavor::CryptCatacombs, pal, false));
                        }
                    }
                }
            }
        }
        // Timber Livestock Lean-to Shelter + Cauldron Water Trough + Hay Stack!
        let sh_x1 = (pen_x0 + 3).min(pen_x1 - 1);
        let sh_z1 = (pen_z0 + 2).min(pen_z1 - 1);
        for z in (pen_z0 + 1)..=sh_z1 {
            for x in (pen_x0 + 1)..=sh_x1 {
                blocks.insert([x, pen_y + 3, z], make_slab(&pal.roof_slab, "bottom"));
                if (x == pen_x0 + 1 || x == sh_x1) && (z == pen_z0 + 1 || z == sh_z1) {
                    blocks.insert([x, pen_y + 1, z], PlannedBlock::simple(pal.fence.clone()));
                    blocks.insert([x, pen_y + 2, z], PlannedBlock::simple(pal.fence.clone()));
                }
            }
        }
        blocks.insert([pen_x1 - 1, pen_y + 1, pen_z1 - 1], PlannedBlock::with_props("minecraft:water_cauldron", vec![("level", "3")]));
        if pen_x1 - 2 > pen_x0 {
            blocks.insert([pen_x1 - 2, pen_y + 1, pen_z1 - 1], PlannedBlock::simple("minecraft:hay_block"));
        }
    }

    // 6. Central Crossroads Estate Lantern Post or Orchard Tree!
    if w >= 23 && d >= 23 {
        let wy = terr_at(cx, cz);
        blocks.insert([cx, wy + 1, cz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, cx, wy + 1, cz)));
        blocks.insert([cx, wy + 2, cz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, cx, wy + 2, cz)));
        blocks.insert([cx, wy + 3, cz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, cx, wy + 3, cz)));
        blocks.insert([cx, wy + 4, cz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cx, wy + 4, cz)));
    }
    let orchard_spot = [w - 4, 2];
    if w >= 29 && d >= 29 && blocks.get(&[orchard_spot[0], terr_at(orchard_spot[0], orchard_spot[1]) + 1, orchard_spot[1]]).is_none() {
        let oy = terr_at(orchard_spot[0], orchard_spot[1]);
        for ty in (oy + 1)..=(oy + 3) {
            blocks.insert([orchard_spot[0], ty, orchard_spot[1]], make_frame_block_at(pal, "y", seed, orchard_spot[0], ty, orchard_spot[1]));
        }
        for dx in -1_i32..=1 {
            for dz in -1_i32..=1 {
                for dy in 3_i32..=4 {
                    if dy == 4 && dx.abs() == 1 && dz.abs() == 1 {
                        continue;
                    }
                    if !(dx == 0 && dz == 0 && dy == 3) {
                        blocks.insert([orchard_spot[0] + dx, oy + dy, orchard_spot[1] + dz], make_leaves(&pal.leaves));
                    }
                }
            }
        }
    }

    let _ = rng;
    prune_map_unsupported(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);

    let gate_y = terr_at(cx, d - 1) + 1;
    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: w as u16,
        depth: d as u16,
        height: h,
        blocks,
        door_pos: Some([cx, gate_y, d - 1]),
    }
}
