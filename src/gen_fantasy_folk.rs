use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, make_leaves, set_uv,
};
use crate::gen_landmarks::{
    anchor_plinth, build_gable_roof, finish_landmark, make_light, pick_stair, place_door_stack,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::NoiseField;
use crate::parser::BlueprintSpec;

/// 34. Archmage Wizard Tower: flared upper observatory, floating-style braced crystal rings & enchanting library.
pub(crate) fn generate_mage_tower(
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
    let base_r = 3.min((grid.w.min(grid.d) - 4) / 2).max(2);
    let waist_top = crate::gen_common::clamp_i32((h * 45) / 100, 5, (h - 7).max(2));
    let top_chamber = crate::gen_common::clamp_i32(h - 5, waist_top + 4, (h - 4).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, cu - base_r - 1, cu + base_r + 1, cv - base_r - 1, cv + base_r + 1, 1, terrain_offsets);

    for y in 2..=top_chamber {
        // Flared Gothic Silhouette: wider base -> slender waist -> dramatic cantilevered upper library!
        let r = if y <= waist_top {
            base_r
        } else if y == top_chamber {
            base_r + 1
        } else {
            base_r + 1
        };
        let shift_u = if y >= waist_top - 1 && y <= waist_top + 2 { 1 } else { 0 };
        let c_cur_u = (cu + shift_u).min(grid.w - r - 2);
        let is_floor = y == 2 || y == waist_top + 1 || y == top_chamber;
        for v in (cv - r)..=(cv + r) {
            for u in (c_cur_u - r)..=(c_cur_u + r) {
                let d = (u - c_cur_u).abs().max((v - cv).abs());
                if d == r && (u - c_cur_u).abs() == r && (v - cv).abs() == r && r >= 3 {
                    continue;
                }
                if d == r {
                    let win = y % 4 == 0 && (u == c_cur_u || v == cv);
                    let blk = if win {
                        "minecraft:purple_stained_glass"
                    } else if y == waist_top + 1 {
                        "minecraft:amethyst_block"
                    } else {
                        "minecraft:deepslate_bricks"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if is_floor {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:purpur_block"));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Soaring Steep Purpur Wizard Roof + Braced Amethyst Crystal Crown!
    let cone_h = (h - 1 - top_chamber).clamp(3, 6);
    for dy in 1..=cone_h {
        let y = top_chamber + dy;
        let r = (base_r - dy / 2).max(0);
        for v in (cv - r)..=(cv + r) {
            for u in (cu - r)..=(cu + r) {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if r == 0 { "minecraft:amethyst_block" } else { "minecraft:purpur_block" }));
            }
        }
    }
    if top_chamber + cone_h + 1 < h {
        set_uv(&mut blocks, grid, cu, top_chamber + cone_h + 1, cv, PlannedBlock::simple("minecraft:end_rod"));
    }

    // Enchanting Table + Bookshelves + Wall Ladder Access!
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::simple("minecraft:enchanting_table"));
    for y in 2..=top_chamber {
        set_uv(&mut blocks, grid, cu - 1, y, cv - 1, PlannedBlock::simple("minecraft:purpur_pillar"));
        set_uv(&mut blocks, grid, cu - 1, y, cv, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + base_r));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 35. Cozy Hobbit Hole embedded inside a grassy flowery Shire hillside with round facade & portal door!
pub(crate) fn generate_hobbit_hole(
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
    let hill_r = ((grid.w.min(grid.d) - 4) / 2).clamp(4, 10);
    let hill_h = (hill_r + 2).min((h - 2).max(2)).clamp(2, 9);
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, hill_h, cv);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - hill_r, cu + hill_r, cv - hill_r, cv + hill_r, 0, terrain_offsets);

    // Sculpted Domed Green Hill with Hollow Cozy Wooden Cabin Interior (`r <= hill_r - 2`) & Front Cutaway Facade!
    let front_face_v = (cv + hill_r - 2).min(grid.d - 3);
    for v in (cv - hill_r)..=front_face_v {
        for u in (cu - hill_r)..=(cu + hill_r) {
            let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
            if d > f64::from(hill_r) + 0.35 {
                continue;
            }
            let dome_top = (1.0 + f64::from(hill_h) * (1.0 - (d / f64::from(hill_r)).powi(2)).max(0.0).sqrt()).round() as i32;
            let in_cabin = d <= f64::from(hill_r - 2) && v <= front_face_v - 1;
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:oak_planks"));
            for y in 2..=dome_top {
                if in_cabin && y <= 4 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                } else if v == front_face_v && y <= 4 && (u - cu).abs() <= 4 {
                    // Cozy Hobbit Circular Wooden Facade Embedded into Hillside!
                    let is_round_win = (u - cu).abs() == 3 && y == 3;
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(if is_round_win { "minecraft:yellow_stained_glass" } else { "minecraft:oak_planks" }),
                    );
                } else if y == dome_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:grass_block"));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:dirt"));
                }
            }
            if (u + v) % 4 == 0 && dome_top + 1 < h {
                set_uv(&mut blocks, grid, u, dome_top + 1, v, make_leaves(&leaf_id));
            }
        }
    }

    // Brick Chimney poking through the grassy hill roof + Cozy Hearth & Bookshelf inside!
    for y in 2..=(hill_h + 2).min(h - 1) {
        set_uv(&mut blocks, grid, cu - 2, y, cv - 1, PlannedBlock::simple("minecraft:bricks"));
    }
    set_uv(&mut blocks, grid, cu - 2, 2, cv - 1, PlannedBlock::simple("minecraft:campfire"));
    set_uv(&mut blocks, grid, cu + 2, 2, cv - 1, PlannedBlock::simple("minecraft:bookshelf"));
    set_uv(&mut blocks, grid, cu, 4, cv, make_light("minecraft:lantern".into(), true));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, front_face_v));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 36. Giant Mushroom House Village: twin crimson & brown mushroom towers joined by glowing rope skybridge!
pub(crate) fn generate_mushroom_house(
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
    let cv = grid.d / 2;
    let u_a = crate::gen_common::clamp_i32(grid.w / 3, 3, (grid.w - 7).max(2));
    let u_b = (grid.w * 2 / 3).max(u_a + 5).min(grid.w - 4);
    let top_a = crate::gen_common::clamp_i32(h - 4, 9, (h - 3).max(2));
    let top_b = crate::gen_common::clamp_i32(h - 6, 7, (top_a - 1).max(2));

    anchor_plinth(&mut blocks, grid, pal, seed, 1, grid.w - 2, 1, grid.d - 2, 0, terrain_offsets);

    for (mu, m_top, is_red) in [(u_a, top_a, true), (u_b, top_b, false)] {
        let cap_blk = if is_red { "minecraft:red_mushroom_block" } else { "minecraft:brown_mushroom_block" };
        // Hollow Mushroom Stem Tower!
        for y in 1..m_top {
            for dv in -2i32..=2 {
                for du in -2i32..=2 {
                    let shell = du.abs() == 2 || dv.abs() == 2;
                    let win = shell && y == 4 && (du == 0 || dv == 0);
                    if du.abs() == 2 && dv.abs() == 2 {
                        continue;
                    }
                    if shell || y == 1 || y == 5 {
                        set_uv(
                            &mut blocks,
                            grid,
                            mu + du,
                            y,
                            cv + dv,
                            PlannedBlock::simple(if win {
                                "minecraft:glass"
                            } else if y == 5 {
                                "minecraft:oak_planks"
                            } else {
                                "minecraft:mushroom_stem"
                            }),
                        );
                    } else {
                        set_uv(&mut blocks, grid, mu + du, y, cv + dv, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
        // Bulbous Mushroom Cap Canopy + Glowing Shroomlights underneath!
        let cap_r = 4;
        for dy in 0..=3 {
            let y = (m_top + dy).min(h - 1);
            let r_here = (cap_r - dy).max(1);
            for dv in -r_here..=r_here {
                for du in -r_here..=r_here {
                    if du * du + dv * dv <= r_here * r_here + 1 {
                        let dot = is_red && (mu + du + y) % 3 == 0 && (cv + dv) % 3 == 0;
                        set_uv(&mut blocks, grid, mu + du, y, cv + dv, PlannedBlock::simple(if dot { "minecraft:white_wool" } else { cap_blk }));
                    }
                }
            }
        }
        set_uv(&mut blocks, grid, mu, m_top - 1, cv, PlannedBlock::simple("minecraft:shroomlight"));
        for y in 2..m_top {
            set_uv(&mut blocks, grid, mu, y, cv - 1, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
        }
    }

    // Elevated Wooden Plank Bridge connecting Mushroom A & B at `y = 5`!
    for u in (u_a + 2)..=(u_b - 2) {
        set_uv(&mut blocks, grid, u, 5, cv, PlannedBlock::simple("minecraft:oak_planks"));
        set_uv(&mut blocks, grid, u, 6, cv - 1, PlannedBlock::simple("minecraft:oak_fence"));
        set_uv(&mut blocks, grid, u, 6, cv + 1, PlannedBlock::simple("minecraft:oak_fence"));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, u_a, 2, cv + 2));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 37. Gothic Mausoleum & Subterranean Catacombs (-5..0): stone portico above + vaulted sarcophagus crypt below!
pub(crate) fn generate_mausoleum_crypt(
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
    let hw = ((grid.w - 4) / 2).clamp(3, 7);
    let hd = ((grid.d - 4) / 2).clamp(4, 8);
    let u0 = cu - hw;
    let u1 = cu + hw;
    let v0 = cv - hd;
    let v1 = cv + hd;
    let hall_top = crate::gen_common::clamp_i32(h / 2, 5, (h - 4).max(2));

    // Underground Vaulted Catacombs Chamber (`y = -5 ..= 0`)!
    for v in v0..=v1 {
        for u in u0..=u1 {
            let shell = u == u0 || u == u1 || v == v0 || v == v1;
            set_uv(&mut blocks, grid, u, -5, v, PlannedBlock::simple("minecraft:polished_blackstone_bricks"));
            for y in -4..=0 {
                if shell || y == 0 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if y % 2 == 0 { "minecraft:deepslate_bricks" } else { "minecraft:cracked_stone_bricks" }));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
            if !shell && v % 3 == 0 && (u == u0 + 1 || u == u1 - 1) {
                set_uv(&mut blocks, grid, u, -4, v, PlannedBlock::simple("minecraft:chiseled_polished_blackstone"));
                set_uv(&mut blocks, grid, u, -3, v, make_light("minecraft:soul_lantern".into(), false));
            }
        }
    }

    // Surface Gothic Mausoleum Shrine + Steep Roof!
    for y in 1..=hall_top {
        for v in (cv - 2)..=(cv + 3).min(v1) {
            for u in (cu - 3).max(u0)..=(cu + 3).min(u1) {
                let shell = u == (cu - 3).max(u0) || u == (cu + 3).min(u1) || v == (cv - 2) || v == (cv + 3).min(v1);
                if shell {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:polished_blackstone_bricks"));
                } else if y > 1 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, (cu - 4).max(u0), (cu + 4).min(u1), cv - 3, (cv + 4).min(v1), hall_top, h, true);

    // Descent Stairway/Ladder down from Mausoleum Floor into Catacomb Vault (`-4..1`)!
    set_uv(&mut blocks, grid, cu, 0, cv, PlannedBlock::simple("minecraft:air"));
    for y in -4..=1 {
        set_uv(&mut blocks, grid, cu, y, cv - 1, PlannedBlock::simple("minecraft:polished_blackstone_bricks"));
        set_uv(&mut blocks, grid, cu, y, cv, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }
    set_uv(&mut blocks, grid, cu, -4, v0 + 2, PlannedBlock::simple("minecraft:gold_block"));
    let _ = terrain_offsets;
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, (cv + 3).min(v1)));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 38. Dwarven Mountain Gate / Throne Hall (Erebor/Moria): towering basalt pylons, caged lava falls & throne.
pub(crate) fn generate_dwarven_hall(
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
    let top_y = crate::gen_common::clamp_i32(h - 2, 10, (h - 1).max(3));
    let portal_hw = ((u1 - u0) / 4).clamp(2, 4);

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Massive Geometric Dwarven Stone Gate Pylons + Caged Magma/Glow Channels on Either Side!
    for v in v0..=v1 {
        for u in u0..=u1 {
            let du = (u - cu).abs();
            let in_hall = du <= portal_hw + 1 && v < v1 - 2 && v > v0 + 1;
            let in_gate_open = du <= portal_hw && v >= v1 - 2;
            for y in 2..=top_y {
                if (in_gate_open && y <= (top_y - 3).clamp(5, 9)) || (in_hall && y <= top_y - 2) {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                } else {
                    let is_gilded = y == 5 || y == top_y - 1 || (du == portal_hw + 1 && y % 4 == 0);
                    let blk = if is_gilded {
                        "minecraft:gilded_blackstone"
                    } else if (u + v + y) % 5 == 0 {
                        "minecraft:polished_basalt"
                    } else {
                        "minecraft:deepslate_bricks"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            }
        }
    }

    // Glowing Lava-Grate Flank Columns framing Gate (`du == portal_hw + 2` at front)!
    for fu in [cu - portal_hw - 2, cu + portal_hw + 2] {
        if grid.in_bounds(fu, v1) {
            for y in 2..=(top_y - 2) {
                set_uv(&mut blocks, grid, fu, y, v1 - 1, PlannedBlock::simple("minecraft:magma_block"));
                set_uv(&mut blocks, grid, fu, y, v1, PlannedBlock::simple("minecraft:iron_bars"));
            }
        }
    }
    // Golden Dwarven King Throne on Stepped Dais inside Hall!
    set_uv(&mut blocks, grid, cu, 2, v0 + 3, PlannedBlock::simple("minecraft:gold_block"));
    set_uv(&mut blocks, grid, cu, 3, v0 + 3, pick_stair(pal, MaterialPalette::SLOT_FLOOR, seed, cu, 3, v0 + 3, grid.dir_front(), "bottom"));
    set_uv(&mut blocks, grid, cu - 1, 2, v0 + 3, PlannedBlock::simple("minecraft:anvil"));
    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 39. Dragon Sanctuary / Leviathan Ribcage Shrine: ruined stone dais arched over by huge bone ribcage & skull!
pub(crate) fn generate_dragon_sanctuary(
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

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Cracked Ruined Temple Podium + Gold Dragon Hoard in center!
    for v in (v0 + 2)..=(v1 - 2) {
        for u in (u0 + 2)..=(u1 - 2) {
            if (u - cu).abs() <= 2 && (v - grid.d / 2).abs() <= 2 {
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:gold_block"));
            }
        }
    }

    // Giant Curved Bone Block Spine & 4 Pairs of Arching Dragon Ribs spanning the Sanctuary!
    let spine_top = (h - 3).clamp(7, 13);
    for v in v0..=v1 {
        let tv = (v - v0) as f64 / (v1 - v0).max(1) as f64;
        let arch_y = (3.0 + (std::f64::consts::PI * tv).sin() * f64::from(spine_top - 3)).round() as i32;
        set_uv(&mut blocks, grid, cu, arch_y, v, PlannedBlock::simple("minecraft:bone_block"));
        if (v - v0) % 3 == 0 && v > v0 + 1 && v < v1 - 2 {
            // Sweeping Left & Right Ribs down to stone floor!
            for s in 1..=3.min(cu - u0) {
                let rib_u_l = cu - s;
                let rib_u_r = cu + s;
                let rib_y = (arch_y - s).max(2);
                set_uv(&mut blocks, grid, rib_u_l, rib_y, v, PlannedBlock::simple("minecraft:bone_block"));
                set_uv(&mut blocks, grid, rib_u_r, rib_y, v, PlannedBlock::simple("minecraft:bone_block"));
                if s == 3 {
                    for fy in 2..rib_y {
                        set_uv(&mut blocks, grid, rib_u_l, fy, v, PlannedBlock::simple("minecraft:bone_block"));
                        set_uv(&mut blocks, grid, rib_u_r, fy, v, PlannedBlock::simple("minecraft:bone_block"));
                    }
                }
            }
        }
    }
    // Horned Dragon Skull perched at front (`v = v1 - 1`) with Glowing Soul Lantern Eyes!
    for y in 2..=5 {
        for du in -1..=1 {
            set_uv(&mut blocks, grid, cu + du, y, v1 - 1, PlannedBlock::simple("minecraft:bone_block"));
        }
    }
    set_uv(&mut blocks, grid, cu - 1, 4, v1 - 1, PlannedBlock::simple("minecraft:sea_lantern"));
    set_uv(&mut blocks, grid, cu + 1, 4, v1 - 1, PlannedBlock::simple("minecraft:sea_lantern"));
    set_uv(&mut blocks, grid, cu - 2, 6, v1 - 2, PlannedBlock::simple("minecraft:bone_block"));
    set_uv(&mut blocks, grid, cu + 2, 6, v1 - 2, PlannedBlock::simple("minecraft:bone_block"));

    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 40. Megalithic Stonehenge Circle & Rune Portal Altar: moss mound, trilithon arches & glowing amethyst core!
pub(crate) fn generate_stone_circle_portal(
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
    let rad = ((grid.w.min(grid.d) - 4) / 2).clamp(4, 11);
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, 2, cv);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - rad, cu + rad, cv - rad, cv + rad, 0, terrain_offsets);
    for v in (cv - rad)..=(cv + rad) {
        for u in (cu - rad)..=(cu + rad) {
            let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
            if d <= f64::from(rad) {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(if (u + v) % 3 == 0 { "minecraft:moss_block" } else { "minecraft:grass_block" }));
            }
        }
    }

    // Ring of Stonehenge Trilithons!
    let tril_h = 5.min(h - 3);
    for angle_deg in (0..360).step_by(45) {
        let ang = f64::from(angle_deg).to_radians();
        let tu = cu + (ang.cos() * f64::from(rad - 1)).round() as i32;
        let tv = cv + (ang.sin() * f64::from(rad - 1)).round() as i32;
        let tu2 = cu + ((ang + 0.25).cos() * f64::from(rad - 1)).round() as i32;
        let tv2 = cv + ((ang + 0.25).sin() * f64::from(rad - 1)).round() as i32;
        for (pu, pv) in [(tu, tv), (tu2, tv2)] {
            for y in 2..tril_h {
                set_uv(&mut blocks, grid, pu, y, pv, PlannedBlock::simple("minecraft:cobblestone"));
            }
            set_uv(&mut blocks, grid, pu, tril_h, pv, PlannedBlock::simple("minecraft:mossy_cobblestone"));
        }
        set_uv(&mut blocks, grid, (tu + tu2) / 2, tril_h, (tv + tv2) / 2, PlannedBlock::simple("minecraft:mossy_cobblestone"));
    }

    // Central Rune Portal Monolith (`crying_obsidian` + `amethyst_block`)!
    let port_top = (h - 2).clamp(7, 10);
    for v in (cv - 1)..=(cv + 1) {
        for u in (cu - 1)..=(cu + 1) {
            set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:stone_bricks"));
        }
    }
    for y in 3..=port_top {
        set_uv(&mut blocks, grid, cu - 2, y, cv, PlannedBlock::simple("minecraft:crying_obsidian"));
        set_uv(&mut blocks, grid, cu + 2, y, cv, PlannedBlock::simple("minecraft:crying_obsidian"));
        if y == port_top {
            for u in (cu - 2)..=(cu + 2) {
                set_uv(&mut blocks, grid, u, y, cv, PlannedBlock::simple("minecraft:crying_obsidian"));
            }
        }
    }
    set_uv(&mut blocks, grid, cu, 3, cv, PlannedBlock::simple("minecraft:amethyst_block"));
    set_uv(&mut blocks, grid, cu, 2, cv + rad - 2, make_leaves(&leaf_id));

    let [lx, lz] = grid.to_local_xz(cu, cv + rad);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 41. Viking Longhouse / Mead Hall: curved boat-shaped hall, central long hearth fire, dragon gables & shields!
pub(crate) fn generate_viking_longhouse(
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
    let max_hw = ((grid.w - 4) / 2).clamp(3, 7);
    let v0 = 2;
    let v1 = grid.d - 3;
    let eave_y = 5.min(h - 5);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - max_hw, cu + max_hw, v0, v1, 1, terrain_offsets);

    // Tapered Boat-Bellied Timber Walls (`wider waist -> narrow ends`)!
    let len = (v1 - v0).max(4) as f64;
    for v in v0..=v1 {
        let tv = ((v - v0) as f64 / len * 2.0 - 1.0).abs();
        let hw = (f64::from(max_hw) * (1.0 - 0.35 * tv * tv)).round() as i32;
        for u in (cu - hw)..=(cu + hw) {
            let on_wall = u == cu - hw || u == cu + hw || v == v0 || v == v1;
            if on_wall {
                for y in 2..=eave_y {
                    let blk = if y == 2 {
                        "minecraft:cobblestone"
                    } else if (v - v0) % 3 == 0 {
                        "minecraft:spruce_log"
                    } else {
                        "minecraft:spruce_planks"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            } else {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:spruce_planks"));
            }
        }
        // Central Burning Long Hearth Pit along axis `cu`!
        if v > v0 + 2 && v < v1 - 2 && (v % 2 == 0) {
            set_uv(&mut blocks, grid, cu, 2, v, PlannedBlock::simple("minecraft:campfire"));
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, cu - max_hw - 1, cu + max_hw + 1, v0 - 1, v1 + 1, eave_y, h, true);
    // Dragon Head Gable Finials crowning front & back roof peaks!
    let apex_y = (eave_y + (max_hw + 1) / 2 + 1).min(h - 2);
    for gv in [v0 - 1, v1 + 1] {
        if grid.in_bounds(cu, gv) {
            set_uv(&mut blocks, grid, cu, apex_y, gv, PlannedBlock::simple("minecraft:dark_oak_log"));
            if apex_y + 1 < h {
                set_uv(&mut blocks, grid, cu, apex_y + 1, gv, PlannedBlock::simple("minecraft:gold_block"));
            }
        }
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 42. Alpine Chalet: stone basement base, cantilevered timber stories, wide low-pitch snow roof & wrap balcony.
pub(crate) fn generate_alpine_chalet(
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
    let u0 = 3;
    let u1 = (grid.w - 4).max(7);
    let v0 = 3;
    let v1 = (grid.d - 4).max(7);
    let wall_top = crate::gen_common::clamp_i32(h * 60 / 100, 8, (h - 4).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let is_base = y <= 4;
                    let win = !is_base && y == 6 && ((u + v) % 3 == 0);
                    let blk = if is_base {
                        "minecraft:cobblestone"
                    } else if win {
                        "minecraft:glass_pane"
                    } else {
                        "minecraft:spruce_planks"
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if y == 5 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:spruce_planks"));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Wraparound Wooden Front Balcony at 2nd floor (`v1 + 1`, `y = 5`)!
    for u in (u0 - 1)..=(u1 + 1) {
        set_uv(&mut blocks, grid, u, 5, v1 + 1, PlannedBlock::simple("minecraft:spruce_planks"));
        set_uv(&mut blocks, grid, u, 6, v1 + 1, PlannedBlock::simple("minecraft:spruce_fence"));
    }
    // Wide Low-Pitch Overhanging Alpine Roof (+2 blocks out) + Tall Exterior Stone Hearth Chimney!
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 2, u1 + 2, v0 - 2, v1 + 2, wall_top, h, true);
    for y in 2..=(h - 2) {
        set_uv(&mut blocks, grid, u0 - 1, y, (v0 + v1) / 2, PlannedBlock::simple("minecraft:cobblestone"));
    }
    set_uv(&mut blocks, grid, u0 + 2, 2, v0 + 2, PlannedBlock::simple("minecraft:campfire"));
    for y in 2..=5 {
        set_uv(&mut blocks, grid, u1 - 1, y, v0 + 1, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 43. Desert Kasbah & Riad Oasis Courtyard: stepped sandstone volumes, windcatcher badgir towers & palm court.
pub(crate) fn generate_desert_kasbah(
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
    let ring_top = crate::gen_common::clamp_i32(h * 52 / 100, 6, (h - 5).max(2));
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, 4, cv);

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Fortified Mud/Sandstone Perimeter enclosing an Oasis Courtyard!
    for v in v0..=v1 {
        for u in u0..=u1 {
            let edge = u == u0 || u == u1 || v == v0 || v == v1;
            let in_court = u >= u0 + 3 && u <= u1 - 3 && v >= v0 + 3 && v <= v1 - 3;
            if edge {
                for y in 2..=ring_top {
                    let is_mashrab = y == 5 && ((u + v) % 3 == 0);
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if is_mashrab { "minecraft:oak_fence" } else { "minecraft:smooth_sandstone" }));
                }
            } else if !in_court {
                set_uv(&mut blocks, grid, u, ring_top, v, PlannedBlock::simple("minecraft:cut_sandstone"));
            }
        }
    }

    // Twin Windcatcher (Badgir) Cooling Towers with open vented louvers on opposite corners!
    let badgir_top = crate::gen_common::clamp_i32(h - 2, ring_top + 4, (h - 1).max(3));
    for &(bu, bv) in &[(u0 + 1, v0 + 1), (u1 - 1, v1 - 1)] {
        for dv in -1i32..=1 {
            for du in -1i32..=1 {
                for y in 2..=badgir_top {
                    let vent = y >= badgir_top - 2 && y < badgir_top && (du == 0 || dv == 0) && (du != 0 || dv != 0);
                    if vent {
                        continue;
                    }
                    if du.abs() == 1 || dv.abs() == 1 || y == badgir_top {
                        set_uv(&mut blocks, grid, bu + du, y, bv + dv, PlannedBlock::simple("minecraft:terracotta"));
                    }
                }
            }
        }
    }

    // Lush Riad Oasis Fountain + Palm Tree inside the Courtyard!
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
    for y in 2..=5 {
        set_uv(&mut blocks, grid, (cu + 2).min(u1 - 2), y, cv - 1, PlannedBlock::simple("minecraft:jungle_log"));
    }
    let pu = (cu + 2).min(u1 - 2);
    for dv in -1..=1 {
        for du in -1..=1 {
            set_uv(&mut blocks, grid, pu + du, 6, cv - 1 + dv, make_leaves(&leaf_id));
        }
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}
