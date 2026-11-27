use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, make_frame_block_at, make_pillar_block_at, set_uv,
};
use crate::gen_landmarks::{
    anchor_plinth, build_dome_shell, build_gable_roof, finish_landmark, make_light, pick_stair,
    place_door_stack,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::NoiseField;
use crate::parser::BlueprintSpec;

/// 17. Town Hall / City Hall: broad entry staircase, portico balcony, assembly hall & central clock spire.
pub(crate) fn generate_town_hall(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 2;
    let u1 = grid.w - 3;
    let v0 = 2;
    let v1 = (grid.d - 4).max(v0 + 6);
    let wall_top = crate::gen_common::clamp_i32((h * 55) / 100, 8, (h - 6).max(3));
    let tower_top = crate::gen_common::clamp_i32(h - 2, wall_top + 4, (h - 1).max(4));

    anchor_plinth(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 2, 2, terrain_offsets);
    // Broad Ceremonial Entry Stairs at front `v1 + 1 .. v1 + 2`.
    for u in (cu - 3).max(u0)..=(cu + 3).min(u1) {
        set_uv(&mut blocks, grid, u, 1, v1 + 2, pick_stair(pal, MaterialPalette::SLOT_FOUNDATION, seed, u, 1, v1 + 2, grid.dir_back(), "bottom"));
        set_uv(&mut blocks, grid, u, 2, v1 + 1, pick_stair(pal, MaterialPalette::SLOT_FOUNDATION, seed, u, 2, v1 + 1, grid.dir_back(), "bottom"));
    }

    for y in 3..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let is_pilaster = (u - u0) % 4 == 0 || (v - v0) % 4 == 0;
                    let is_win = !is_pilaster && (y == 4 || y == 5 || y == wall_top - 2);
                    let blk = if is_pilaster {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if is_win {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else if y == 7 && wall_top >= 9 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR_UPPER, seed, u, y, v)));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 1, wall_top, h - 2, false);

    // Central Clock Bell Tower rising proudly above facade!
    let tv = (v0 + v1) / 2;
    for dv in -2..=2 {
        for du in -2..=2 {
            let u = cu + du;
            let v = tv + dv;
            if !grid.in_bounds(u, v) {
                continue;
            }
            for y in (wall_top + 1)..=tower_top {
                let inset = if y >= tower_top - 2 { 1 } else { 0 };
                if du.abs() <= 2 - inset && dv.abs() <= 2 - inset {
                    let is_face = y == wall_top + 3 && ((du == 0 && dv.abs() == 2) || (dv == 0 && du.abs() == 2));
                    let blk = if y == tower_top {
                        "minecraft:gold_block".to_owned()
                    } else if is_face {
                        pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            }
        }
    }
    set_uv(&mut blocks, grid, cu, 3, v0 + 2, PlannedBlock::simple("minecraft:lectern"));
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 3, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 18. Grand Opera House / Theatre: red carpet foyer, tiered horseshoe balconies, orchestra pit & curtained stage.
pub(crate) fn generate_opera_house(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let wall_top = crate::gen_common::clamp_i32(h - 4, 8, (h - 3).max(3));
    let stage_v = v0 + (v1 - v0) / 3;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let front_col = v == v1 && (u - cu).abs() % 2 == 0 && y < wall_top;
                    let blk = if front_col {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if y == 3 || y == wall_top {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
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

    // Raised Wooden Stage & Crimson Velvet Curtains at rear!
    for v in (v0 + 1)..=stage_v {
        for u in (u0 + 1)..=(u1 - 1) {
            set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:spruce_planks"));
            if v == stage_v && ((u - u0 <= 2) || (u1 - u <= 2)) {
                for y in 3..(wall_top - 1) {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:red_wool"));
                }
            }
            if v == stage_v {
                set_uv(&mut blocks, grid, u, wall_top - 1, v, PlannedBlock::simple("minecraft:red_wool"));
            }
        }
    }

    // Orchestra Red Velvet Spectator Seating Rows & Side Tiered Box Balconies!
    for v in ((stage_v + 2)..=(v1 - 3)).step_by(2) {
        for u in (u0 + 3)..=(u1 - 3) {
            set_uv(&mut blocks, grid, u, 2, v, pick_stair(pal, MaterialPalette::SLOT_ROOF, seed, u, 2, v, grid.dir_back(), "bottom"));
        }
    }
    if wall_top >= 8 {
        for v in (stage_v + 1)..=(v1 - 2) {
            for u in [u0 + 1, u1 - 1] {
                set_uv(&mut blocks, grid, u, 5, v, PlannedBlock::simple("minecraft:red_wool"));
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 1, wall_top, h, true);
    set_uv(&mut blocks, grid, cu, wall_top - 1, (stage_v + v1) / 2, make_light("minecraft:sea_lantern".into(), true));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 19. Natural History & Art Museum: skylit domed marble atrium with centerpiece Dinosaur Bone Skeleton!
pub(crate) fn generate_museum(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let cv = grid.d / 2;
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let wall_top = crate::gen_common::clamp_i32((h * 58) / 100, 7, (h - 5).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let is_col = (v == v1 && (u - cu).abs() % 2 == 0) || ((u == u0 || u == u1) && (v - v0) % 3 == 0);
                    let blk = if is_col {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if y >= 4 && y <= wall_top - 1 && ((u + v) % 3 == 1) {
                        PlannedBlock::simple("minecraft:glass")
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else if y == wall_top {
                    let glass_roof = (u - cu).abs() <= 4 && (v - cv).abs() <= 4;
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if glass_roof { "minecraft:glass" } else { "minecraft:smooth_quartz" }));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Centerpiece Skylit Glass Atrium Dome + Dinosaur Fossil Skeleton (`bone_block`)!
    let r_dome = ((u1 - u0).min(v1 - v0) / 4).clamp(3, 5);
    build_dome_shell(&mut blocks, grid, pal, seed, cu, cv, r_dome, wall_top, (h - wall_top - 1).clamp(3, 5), false, MaterialPalette::SLOT_WINDOW);

    for v in (cv - 3).max(v0 + 2)..=(cv + 3).min(v1 - 2) {
        let spine_y = if v == cv - 3 { 5 } else if v >= cv + 2 { 3 } else { 4 };
        set_uv(&mut blocks, grid, cu, spine_y, v, PlannedBlock::simple("minecraft:bone_block"));
        if (v - cv).abs() <= 1 {
            set_uv(&mut blocks, grid, cu - 1, spine_y - 1, v, PlannedBlock::simple("minecraft:bone_block"));
            set_uv(&mut blocks, grid, cu + 1, spine_y - 1, v, PlannedBlock::simple("minecraft:bone_block"));
        }
    }
    for y in 2..=3 {
        set_uv(&mut blocks, grid, cu - 1, y, cv, PlannedBlock::simple("minecraft:bone_block"));
        set_uv(&mut blocks, grid, cu + 1, y, cv, PlannedBlock::simple("minecraft:bone_block"));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 20. Classical Marble Bank / Vault: monumental columns, teller cages & gold-filled iron safe chamber.
pub(crate) fn generate_bank_vault(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 2;
    let u1 = grid.w - 3;
    let v0 = 2;
    let v1 = grid.d - 3;
    let wall_top = crate::gen_common::clamp_i32(h - 4, 7, (h - 3).max(3));
    let vault_v = v0 + (v1 - v0) / 3;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let outer = u == u0 || u == u1 || v == v0 || v == v1;
                let vault_wall = v == vault_v && u > u0 && u < u1;
                if outer {
                    let is_col = v == v1 && (u - cu).abs() % 2 == 0;
                    let blk = if is_col {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else if vault_wall {
                    let open_door = (u - cu).abs() <= 1 && y <= 4;
                    let blk = if open_door {
                        "minecraft:air"
                    } else if (u - cu).abs() <= 2 && y <= 5 {
                        "minecraft:iron_block"
                    } else {
                        "minecraft:polished_deepslate"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 1, wall_top, h, true);

    // Gold & Emerald Ingot Piles inside Armored Vault!
    for v in (v0 + 1)..vault_v {
        for u in (u0 + 1)..=(u1 - 1) {
            if (u + v) % 2 == 0 && (u - cu).abs() >= 2 {
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:gold_block"));
                if (u + v) % 4 == 0 {
                    set_uv(&mut blocks, grid, u, 3, v, PlannedBlock::simple("minecraft:emerald_block"));
                }
            }
        }
    }
    // Iron-barred Teller Counters in lobby.
    for u in (u0 + 2)..=(u1 - 2) {
        if (u - cu).abs() >= 2 {
            set_uv(&mut blocks, grid, u, 2, vault_v + 2, PlannedBlock::simple("minecraft:smooth_stone"));
            set_uv(&mut blocks, grid, u, 3, vault_v + 2, PlannedBlock::simple("minecraft:iron_bars"));
        }
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 21. Magic Academy Great Hall: long house dining tables with candles, banners, mezzanine & stained windows.
pub(crate) fn generate_academy(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 2;
    let u1 = grid.w - 3;
    let v0 = 2;
    let v1 = grid.d - 3;
    let wall_top = crate::gen_common::clamp_i32(h - 5, 8, (h - 4).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let is_win = y >= 4 && y <= wall_top - 2 && ((u == u0 || u == u1) && (v - v0) % 3 == 1);
                    let blk = if is_win {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 1, wall_top, h, true);

    // 4 Long House Banquet Tables + High Professor Dais + Faculty Banners!
    let banner_colors = ["minecraft:red_wool", "minecraft:blue_wool", "minecraft:yellow_wool", "minecraft:green_wool"];
    for (idx, tu) in [(cu - 3).max(u0 + 2), (cu + 3).min(u1 - 2)].into_iter().enumerate() {
        for v in (v0 + 4)..=(v1 - 3) {
            set_uv(&mut blocks, grid, tu, 2, v, PlannedBlock::simple("minecraft:dark_oak_planks"));
            if v % 3 == 0 {
                set_uv(&mut blocks, grid, tu, 3, v, PlannedBlock::with_props("minecraft:candle", vec![("candles", "2"), ("lit", "true"), ("waterlogged", "false")]));
            }
        }
        for y in (wall_top - 3)..(wall_top) {
            set_uv(&mut blocks, grid, tu, y, v0 + 2, PlannedBlock::simple(banner_colors[idx % banner_colors.len()]));
        }
    }
    set_uv(&mut blocks, grid, cu, 2, v0 + 2, PlannedBlock::simple("minecraft:lectern"));
    for v in (v0 + 4..v1 - 2).step_by(4) {
        set_uv(&mut blocks, grid, cu, wall_top - 1, v, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, wall_top - 1, v), true));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 22. Detailed Wooden Sailing Galleon / Pirate Ship with curved hull, bowsprit, 2 tall masts & billowing sails!
pub(crate) fn generate_sailing_ship(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let hw = ((grid.w - 4) / 2).clamp(3, 7);
    let v_stern = 2;
    let v_bow = (grid.d - 3).max(v_stern + 10);
    let water_y = 1;
    let deck_y = 4;

    // Gentle harbor water pool basin along keel footprint.
    anchor_plinth(&mut blocks, grid, pal, seed, cu - hw - 1, cu + hw + 1, v_stern - 1, v_bow + 1, 0, terrain_offsets);
    for v in (v_stern - 1)..=(v_bow + 1) {
        for u in (cu - hw - 1)..=(cu + hw + 1) {
            let rim = u == cu - hw - 1 || u == cu + hw + 1 || v == v_stern - 1 || v == v_bow + 1;
            set_uv(
                &mut blocks,
                grid,
                u,
                water_y,
                v,
                if rim {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, water_y, v))
                } else {
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")])
                },
            );
        }
    }

    // Tapered Elliptical Wooden Ship Hull + Poop-Deck Stern Cabin!
    let len = (v_bow - v_stern) as f64;
    for v in v_stern..=v_bow {
        let t = (v - v_stern) as f64 / len.max(1.0);
        // Taper dramatically toward bow (`t -> 1`), broad across waist (`0.2..0.7`), moderate stern.
        let local_hw = if t > 0.72 {
            ((f64::from(hw) * ((1.0 - t) / 0.28)).round() as i32).max(1)
        } else if t < 0.18 {
            ((f64::from(hw) * (0.65 + t * 1.9)).round() as i32).clamp(2, hw)
        } else {
            hw
        };
        let rail_y = if t < 0.25 { deck_y + 3 } else { deck_y + 1 };
        for u in (cu - local_hw)..=(cu + local_hw) {
            let du = (u - cu).abs();
            for y in 1..=deck_y {
                let keel_inset = if y == 1 {
                    (local_hw - 2).max(0)
                } else if y == 2 {
                    (local_hw - 1).max(0)
                } else {
                    local_hw
                };
                if du <= keel_inset && (y == deck_y || y == 1 || du == keel_inset || v == v_stern) {
                    let hull_blk = if y == 1 { "minecraft:dark_oak_planks" } else { "minecraft:spruce_planks" };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(hull_blk));
                }
            }
            if du == local_hw || (v == v_stern && t < 0.25) {
                for y in (deck_y + 1)..=rail_y {
                    let is_cab_win = v == v_stern && y == deck_y + 2 && du <= 1;
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if is_cab_win { "minecraft:yellow_stained_glass" } else { "minecraft:dark_oak_planks" }));
                }
                set_uv(&mut blocks, grid, u, rail_y + 1, v, PlannedBlock::simple("minecraft:oak_fence"));
            }
        }
    }
    // Bowsprit jutting forward from `v_bow`!
    for s in 1..=3.min(grid.d - 1 - v_bow) {
        set_uv(&mut blocks, grid, cu, deck_y + s.min(2), v_bow + s, make_frame_block_at(pal, grid.axis_v(), seed, cu, deck_y + 1, v_bow + s));
    }

    // Twin Tall Masts (`main` & `fore`) + Cross Yardarms + Billowing White Canvas Sails!
    let v_mid = (v_stern + v_bow) / 2;
    let masts = [
        (v_mid, crate::gen_common::clamp_i32(h - 2, deck_y + 7, (h - 1).max(3))),
        (v_bow - (v_bow - v_mid) / 2, crate::gen_common::clamp_i32(h - 4, deck_y + 6, (h - 3).max(3))),
    ];
    for (mv, m_top) in masts {
        for y in deck_y..=m_top {
            set_uv(&mut blocks, grid, cu, y, mv, PlannedBlock::simple("minecraft:spruce_log"));
        }
        // Horizontal yardarm + attached full-width wool sail connected cleanly down!
        let sail_bot = deck_y + 3;
        let sail_top = (m_top - 2).max(sail_bot + 2);
        let sail_hw = (hw + 1).min(cu - 1);
        for u in (cu - sail_hw)..=(cu + sail_hw) {
            set_uv(&mut blocks, grid, u, sail_top, mv, PlannedBlock::simple("minecraft:oak_fence"));
            for y in sail_bot..=sail_top {
                let belly_v = if y > sail_bot && y < sail_top { mv + 1 } else { mv };
                if grid.in_bounds(u, belly_v) {
                    set_uv(&mut blocks, grid, u, y, belly_v, PlannedBlock::simple("minecraft:white_wool"));
                }
            }
        }
    }

    set_uv(&mut blocks, grid, cu, deck_y + 4, v_stern + 1, PlannedBlock::simple("minecraft:lantern"));
    finish_landmark(req, spec, grid, height, blocks, noise, None)
}

/// 23. Harbor Shipyard Dock: cobblestone quay, storage warehouse, towering cargo crane & moored boat.
pub(crate) fn generate_shipyard_dock(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let quay_v = crate::gen_common::clamp_i32(grid.d * 48 / 100, 6, (v1 - 5).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, quay_v, 1, terrain_offsets);
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, quay_v + 1, v1, 0, terrain_offsets);

    // Harbor Water Basin in front half + Wooden Finger Pier jutting into water!
    for v in (quay_v + 1)..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v1;
            let on_pier = (u - cu + 3).abs() <= 1 && v <= v1 - 2;
            if rim {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 1, v)));
            } else {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                if on_pier {
                    set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:spruce_planks"));
                }
            }
        }
    }

    // Timber Harbor Warehouse on Left Half of Quay (`u0 + 1 .. cu - 1`) + Cargo Crates & Barrels.
    let wh_u1 = (cu - 1).max(u0 + 4);
    let wh_top = 6.min(h - 4);
    for y in 2..=wh_top {
        for v in (v0 + 1)..=(quay_v - 1) {
            for u in (u0 + 1)..=wh_u1 {
                let shell = u == u0 + 1 || u == wh_u1 || v == v0 + 1 || v == quay_v - 1;
                if shell {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0, wh_u1 + 1, v0, quay_v, wh_top, h, false);
    set_uv(&mut blocks, grid, u1 - 2, 2, quay_v - 1, PlannedBlock::simple("minecraft:barrel"));
    set_uv(&mut blocks, grid, u1 - 3, 2, quay_v - 1, PlannedBlock::simple("minecraft:barrel"));

    // Tall Harbor Cargo Crane overhanging pier + chain hanging cargo crate!
    let crane_u = (cu + 2).min(u1 - 2);
    let crane_v = quay_v;
    let crane_top = (h - 2).clamp(8, 12);
    for y in 2..=crane_top {
        set_uv(&mut blocks, grid, crane_u, y, crane_v, PlannedBlock::simple("minecraft:oak_log"));
    }
    for dv in 0..=4.min(v1 - crane_v - 1) {
        set_uv(&mut blocks, grid, crane_u, crane_top, crane_v + dv, PlannedBlock::simple("minecraft:spruce_planks"));
    }
    for y in (crane_top - 3)..crane_top {
        set_uv(&mut blocks, grid, crane_u, y, crane_v + 3, PlannedBlock::simple("minecraft:iron_chain"));
    }
    set_uv(&mut blocks, grid, crane_u, crane_top - 3, crane_v + 3, PlannedBlock::simple("minecraft:barrel"));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, (u0 + 1 + wh_u1) / 2, 2, quay_v - 1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 24. Victorian Train Station: clock tower ticket hall, arched glass train-shed vault & steam locomotive on rails!
pub(crate) fn generate_train_station(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let cu = (u0 + u1) / 2;
    let track_cv = (v0 + v1) / 2 - 2;
    let lobby_v0 = (track_cv + 3).min(v1 - 4);
    let lobby_top = 7.min(h - 4);

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Arched Copper & Glass Victorian Train-Shed Vault spanning over railway tracks along `u`!
    let shed_hw = (grid.d / 4).clamp(3, 5);
    let shed_top = (h - 3).clamp(8, 12);
    for u in u0..=u1 {
        let is_rib = u == u0 || u == u1 || (u - u0) % 4 == 0;
        for dv in -shed_hw..=shed_hw {
            let v = track_cv + dv;
            if v < v0 || v > lobby_v0 {
                continue;
            }
            if is_rib && dv.abs() == shed_hw {
                for y in 2..=shed_top - 2 {
                    set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                }
            }
            let arch_y = (shed_top - (dv * dv) / 3).max(6).min(h - 1);
            let blk = if is_rib {
                pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, arch_y, v)
            } else {
                "minecraft:glass".to_owned()
            };
            set_uv(&mut blocks, grid, u, arch_y, v, PlannedBlock::simple(blk));
        }
        // Gravel Ballast + Railway Track (`rail`) + Retro Black Steam Locomotive!
        if grid.in_bounds(u, track_cv) {
            set_uv(&mut blocks, grid, u, 1, track_cv, PlannedBlock::simple("minecraft:gravel"));
            set_uv(&mut blocks, grid, u, 2, track_cv, PlannedBlock::with_props("minecraft:rail", vec![("shape", if grid.axis_u() == "x" { "east_west" } else { "north_south" })]));
        }
    }

    // Steam Locomotive parked at platform (`u = cu - 3 .. cu + 3`)!
    for u in (cu - 3).max(u0 + 2)..=(cu + 3).min(u1 - 2) {
        let is_cab = u >= cu + 1;
        let loc_h = if is_cab { 5 } else { 4 };
        for y in 3..=loc_h {
            set_uv(&mut blocks, grid, u, y, track_cv, PlannedBlock::simple(if y == 5 { "minecraft:red_terracotta" } else { "minecraft:black_concrete" }));
        }
        if u == cu - 2 {
            set_uv(&mut blocks, grid, u, 5, track_cv, PlannedBlock::simple("minecraft:black_concrete"));
            set_uv(&mut blocks, grid, u, 6, track_cv, PlannedBlock::simple("minecraft:campfire"));
        }
    }

    // Brick Front Station Lobby + Clock Tower above entry.
    for y in 2..=lobby_top {
        for v in lobby_v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == lobby_v0 || v == v1;
                if shell {
                    let is_win = y == 4 && ((u - u0) % 3 != 0);
                    let blk = if is_win { "minecraft:glass" } else { "minecraft:bricks" };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, u0, u1, lobby_v0, v1, lobby_top, h, false);

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 25. Steampunk Airship Dock: tall stone/copper mooring tower linked via sky-gangway to moored Dirigible!
pub(crate) fn generate_airship_dock(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let tower_v = grid.d - 5;
    let deck_y = crate::gen_common::clamp_i32((h * 55) / 100, 7, (h - 7).max(3));
    let tower_top = crate::gen_common::clamp_i32(h - 2, deck_y + 4, (h - 1).max(4));

    anchor_plinth(&mut blocks, grid, pal, seed, cu - 3, cu + 3, tower_v - 3, tower_v + 3, 1, terrain_offsets);

    // Tall Mooring Tower & Cantilevered Boarding Pier Platform!
    for y in 2..=tower_top {
        for dv in -2..=2 {
            for du in -2..=2 {
                let u = cu + du;
                let v = tower_v + dv;
                let shell = du.abs() == 2 || dv.abs() == 2;
                if shell || y == deck_y || y == tower_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    for y in 2..=deck_y {
        set_uv(&mut blocks, grid, cu, y, tower_v - 1, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }

    // Mooring Sky-Gangway connecting Tower to Dirigible Gondola!
    let ship_cv = crate::gen_common::clamp_i32(grid.d / 3, 4, (tower_v - 5).max(2));
    for v in ship_cv..=(tower_v - 2) {
        for u in (cu - 1)..=(cu + 1) {
            set_uv(&mut blocks, grid, u, deck_y, v, PlannedBlock::simple("minecraft:spruce_planks"));
            if u == cu - 1 || u == cu + 1 {
                set_uv(&mut blocks, grid, u, deck_y + 1, v, PlannedBlock::simple("minecraft:oak_fence"));
            }
        }
    }

    // Gondola anchored onto Boarding Bridge (`deck_y + 1 .. deck_y + 3`) + Balloon Connected Solidly by Timber Rigging!
    let gond_v0 = (ship_cv - 3).max(2);
    let gond_v1 = (ship_cv + 2).min(tower_v - 3);
    for v in gond_v0..=gond_v1 {
        for u in (cu - 1)..=(cu + 1) {
            set_uv(&mut blocks, grid, u, deck_y, v, PlannedBlock::simple("minecraft:dark_oak_planks"));
            if u == cu - 1 || u == cu + 1 || v == gond_v0 || v == gond_v1 {
                set_uv(&mut blocks, grid, u, deck_y + 1, v, PlannedBlock::simple("minecraft:spruce_fence"));
            }
        }
    }
    // Structural copper/timber rigging struts connecting gondola cleanly up to the Dirigible Balloon!
    let ball_cy = (deck_y + 5).min(h - 4);
    for v in [gond_v0 + 1, gond_v1 - 1] {
        for u in [cu - 1, cu + 1] {
            for y in (deck_y + 2)..=ball_cy {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:oak_fence"));
            }
        }
    }

    // Ellipsoid Copper-Trimmed Gas Balloon Hull (`v = 2 .. tower_v - 2`)!
    let b_v0 = 2;
    let b_v1 = (tower_v - 2).max(b_v0 + 6);
    let b_len = (b_v1 - b_v0) as f64;
    for v in b_v0..=b_v1 {
        let tv = ((v - b_v0) as f64 / b_len * 2.0 - 1.0).abs();
        let rad = (3.2 * (1.0 - tv * tv).max(0.0).sqrt()).round() as i32;
        for dy in -rad..=rad {
            for du in -rad..=rad {
                let d = ((du * du + dy * dy) as f64).sqrt();
                if d <= f64::from(rad) + 0.45 && (ball_cy + dy) < h && grid.in_bounds(cu + du, v) {
                    let stripe = (v - b_v0) % 4 == 0 || du.abs() == rad;
                    set_uv(
                        &mut blocks,
                        grid,
                        cu + du,
                        ball_cy + dy,
                        v,
                        PlannedBlock::simple(if stripe { "minecraft:cut_copper" } else { "minecraft:white_wool" }),
                    );
                }
            }
        }
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, tower_v + 2));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 26. Hydroelectric Concrete Dam / Barrage: massive curved retaining wall, upper reservoir & roaring spillways.
pub(crate) fn generate_dam_barrage(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let u0 = 1;
    let u1 = grid.w - 2;
    let v0 = 1;
    let v1 = grid.d - 2;
    let crest_y = crate::gen_common::clamp_i32((h * 72) / 100, 6, (h - 3).max(3));
    let dam_v = (v0 + v1) / 2;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 0, terrain_offsets);

    // Sloped Massive Curved Dam Wall + Upper Reservoir Water Behind (`v < dam_v - 1`) + Plunge Pool!
    for u in u0..=u1 {
        let is_spill = (u - cu).abs() == 2 || (u - cu).abs() == 3;
        for v in v0..=(dam_v - 2) {
            let edge = u == u0 || u == u1 || v == v0;
            for y in 1..=(crest_y - 1) {
                let blk = if edge {
                    PlannedBlock::simple("minecraft:stone_bricks")
                } else {
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")])
                };
                set_uv(&mut blocks, grid, u, y, v, blk);
            }
        }
        for y in 1..=crest_y {
            let slope_back = ((crest_y - y) / 3).clamp(0, 2);
            for dv in -1..=(1 + slope_back) {
                let v = dam_v + dv;
                if v > v1 {
                    continue;
                }
                if is_spill && y < crest_y && dv == 1 + slope_back {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:smooth_stone"));
                }
            }
        }
        // Bridge walkway & streetlamps along dam crest!
        set_uv(&mut blocks, grid, u, crest_y + 1, dam_v, PlannedBlock::simple("minecraft:stone_brick_wall"));
        if u % 4 == 0 && crest_y + 2 < h {
            set_uv(&mut blocks, grid, u, crest_y + 2, dam_v, make_light("minecraft:sea_lantern".into(), false));
        }
        for v in (dam_v + 3)..=(v1 - 1) {
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
        }
    }

    let [lx, lz] = grid.to_local_xz(u0, dam_v);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, crest_y + 1, lz]))
}

/// 27. Big Top Circus Tent (Chapiteau): striped red/white conical canopy, sand ring & amphitheater seats.
pub(crate) fn generate_circus_tent(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let cv = grid.d / 2;
    let rad = ((grid.w.min(grid.d) - 4) / 2).clamp(5, 13);
    let eave_y = 5.min(h - 5).max(2);
    let peak_y = crate::gen_common::clamp_i32(h - 2, eave_y + 4, (h - 1).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, cu - rad, cu + rad, cv - rad, cv + rad, 1, terrain_offsets);

    // Sand Performer Ring + Circular Tent Wall + Sloped Striped Conical Roof!
    for v in (cv - rad)..=(cv + rad) {
        for u in (cu - rad)..=(cu + rad) {
            let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
            if d > f64::from(rad) + 0.35 {
                continue;
            }
            let stripe = ((u as f64 - cu as f64).atan2(v as f64 - cv as f64) * 6.0 / std::f64::consts::PI).floor() as i32;
            let cloth = if stripe.abs() % 2 == 0 { "minecraft:red_wool" } else { "minecraft:white_wool" };
            if d <= f64::from(rad) * 0.45 {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:sand"));
            } else if d <= f64::from(rad) * 0.78 {
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:spruce_planks"));
            }
            if (d - f64::from(rad)).abs() <= 0.75 {
                let is_entry = v > cv && (u - cu).abs() <= 1;
                if !is_entry {
                    for y in 2..=eave_y {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(cloth));
                    }
                }
            }
            let t = (1.0 - (d / f64::from(rad))).clamp(0.0, 1.0);
            let ry = (eave_y + ((peak_y - eave_y) as f64 * t).round() as i32).min(h - 1);
            set_uv(&mut blocks, grid, u, ry, v, PlannedBlock::simple(cloth));
        }
    }
    // Central Circus Mast & Hanging Star Lights!
    for y in 2..=peak_y {
        set_uv(&mut blocks, grid, cu, y, cv, make_frame_block_at(pal, "y", seed, cu, y, cv));
    }
    set_uv(&mut blocks, grid, cu, (eave_y + peak_y) / 2, cv + 2, make_light("minecraft:lantern".into(), true));
    let [lx, lz] = grid.to_local_xz(cu, cv + rad);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}
