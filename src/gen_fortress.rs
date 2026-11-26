use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, make_beam_block_at, make_frame_block_at,
    make_leaves, make_pillar_block_at, make_slab, set_uv,
};
use crate::gen_landmarks::{
    anchor_plinth, build_gable_roof, finish_landmark, make_light, pick_stair, place_door_stack,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::NoiseField;
use crate::parser::BlueprintSpec;

/// 9. Concentric Walled Bailey Castle: outer crenellated curtain wall, 4 corner towers, courtyard well & tall central Keep.
pub(crate) fn generate_walled_bailey_castle(
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
    let wall_h = ((h * 38) / 100).clamp(5, (h / 2).max(5));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Outer Curtain Wall + Catwalk Parapet + Crenellations.
    for v in v0..=v1 {
        for u in u0..=u1 {
            let on_wall = u == u0 || u == u1 || v == v0 || v == v1;
            let on_walk = u == u0 + 1 || u == u1 - 1 || v == v0 + 1 || v == v1 - 1;
            let in_gate = v == v1 && (u - cu).abs() <= 1;
            if on_wall {
                for y in 2..=wall_h {
                    if in_gate && y <= 4 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(if y == 4 { "minecraft:iron_bars" } else { "minecraft:air" }),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                    }
                }
                if wall_h + 1 < h && (u + v) % 2 == 0 {
                    set_uv(&mut blocks, grid, u, wall_h + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, wall_h + 1, v)));
                }
            } else if on_walk {
                set_uv(&mut blocks, grid, u, wall_h, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, wall_h, v)));
            }
        }
    }

    // 4 Corner Bastion Watch Towers taller than curtain wall.
    let tow_top = (wall_h + 4).min(h - 3);
    for &(tu, tv) in &[(u0 + 1, v0 + 1), (u1 - 1, v0 + 1), (u0 + 1, v1 - 1), (u1 - 1, v1 - 1)] {
        for dv in -1..=1 {
            for du in -1..=1 {
                let u = tu + du;
                let v = tv + dv;
                let rim = du.abs() == 1 || dv.abs() == 1;
                for y in 2..=tow_top {
                    if rim || y == tow_top {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
                if rim && (du + dv).abs() % 2 == 1 && tow_top + 1 < h {
                    set_uv(&mut blocks, grid, u, tow_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, tow_top + 1, v)));
                }
            }
        }
    }

    // Towering Central Donjon Keep rising cleanly inside the bailey courtyard!
    let k_hw = ((u1 - u0) / 5).clamp(2, 4);
    let k_hd = ((v1 - v0) / 5).clamp(2, 4);
    let keep_top = crate::gen_common::clamp_i32(h - 3, tow_top + 2, (h - 2).max(3));
    for v in (cv - k_hd)..=(cv + k_hd) {
        for u in (cu - k_hw)..=(cu + k_hw) {
            let shell = u == cu - k_hw || u == cu + k_hw || v == cv - k_hd || v == cv + k_hd;
            for y in 2..=keep_top {
                if shell || y == keep_top || y == (keep_top / 2 + 1) {
                    let is_win = shell && y % 4 == 0 && (u == cu || v == cv);
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
            if shell && (u + v) % 2 == 0 && keep_top + 1 < h {
                set_uv(&mut blocks, grid, u, keep_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, keep_top + 1, v)));
            }
        }
    }
    for y in 2..keep_top {
        set_uv(&mut blocks, grid, cu, y, cv - k_hd + 1, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + k_hd));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 10. Monumental City Gatehouse / Barbican & Curtain Wall Section with Portcullis & Machicolations.
pub(crate) fn generate_city_gatehouse(
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
    let wall_d = 2.min((grid.d - 4) / 3).max(1);
    let gate_hw = 2.min((u1 - u0) / 5).max(1);
    let wall_h = crate::gen_common::clamp_i32((h * 52) / 100, 6, (h - 5).max(3));
    let tower_top = crate::gen_common::clamp_i32(h - 3, wall_h + 3, (h - 2).max(3));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, (cv - wall_d - 2).max(1), (cv + wall_d + 2).min(grid.d - 2), 1, terrain_offsets);

    // Thick Flanking City Wall Wings + Fighting Wall-Walk.
    for v in (cv - wall_d)..=(cv + wall_d) {
        for u in u0..=u1 {
            let du = (u - cu).abs();
            let in_gate = du <= gate_hw;
            for y in 2..=wall_h {
                if in_gate && y <= 5 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(if y == 5 && (v == cv - wall_d || v == cv + wall_d) {
                            "minecraft:iron_bars"
                        } else {
                            "minecraft:air"
                        }),
                    );
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                }
            }
            if (v == cv - wall_d || v == cv + wall_d) && (u % 2 == 0) && wall_h + 1 < h {
                set_uv(&mut blocks, grid, u, wall_h + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, wall_h + 1, v)));
            }
        }
    }

    // Twin Heavy Projecting Drum Gatehouse Towers!
    let t_off = (gate_hw + 2).min((u1 - cu - 1).max(gate_hw + 1));
    for tc in [cu - t_off, cu + t_off] {
        for v in (cv - wall_d - 1).max(1)..=(cv + wall_d + 1).min(grid.d - 2) {
            for u in (tc - 1).max(1)..=(tc + 1).min(grid.w - 2) {
                for y in 2..=tower_top {
                    let center_core = u == tc && v == cv;
                    if center_core && y < tower_top {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                    }
                }
                if (u - tc).abs() == 1 || (v - cv).abs() >= wall_d {
                    if (u + v) % 2 == 0 && tower_top + 1 < h {
                        set_uv(&mut blocks, grid, u, tower_top + 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, tower_top + 1, v)));
                    }
                }
            }
        }
    }

    set_uv(&mut blocks, grid, cu, 5, cv, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, 5, cv), true));
    let [lx, lz] = grid.to_local_xz(cu, cv + wall_d);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 11. Monastery / Cloister: 4-sided vaulted stone arcade surrounding a serene garden fountain + chapel bell.
pub(crate) fn generate_monastery_cloister(
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
    let gallery_d = 3.min((grid.w.min(grid.d) - 7) / 2).max(2);
    let cloister_top = 5.min(h - 4);

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Outer Wing Walls & Inner Square Cloister Arcade Pillars + Covered Roof!
    for v in v0..=v1 {
        for u in u0..=u1 {
            let in_garden = u > u0 + gallery_d && u < u1 - gallery_d && v > v0 + gallery_d && v < v1 - gallery_d;
            if in_garden {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:grass_block"));
                continue;
            }
            let outer_wall = u == u0 || u == u1 || v == v0 || v == v1;
            let inner_arcade = u == u0 + gallery_d || u == u1 - gallery_d || v == v0 + gallery_d || v == v1 - gallery_d;
            for y in 2..=cloister_top {
                if outer_wall {
                    let gate = v == v1 && (u - cu).abs() <= 1 && y <= 4;
                    let win = !gate && y == 3 && ((u + v) % 3 == 0);
                    let blk = if gate {
                        "minecraft:air".to_owned()
                    } else if win {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if inner_arcade {
                    if (u + v) % 2 == 0 || y == cloister_top {
                        set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                    }
                } else if y == cloister_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, y, v)));
                }
            }
            if !in_garden && cloister_top + 1 < h {
                set_uv(&mut blocks, grid, u, cloister_top + 1, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, cloister_top + 1, v), "bottom"));
            }
        }
    }

    // Center Courtyard Fountain + Rear Chapel Spire with Bell!
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, cu, 2, cv)));
    for &(fu, fv) in &[(cu - 1, cv), (cu + 1, cv), (cu, cv - 1), (cu, cv + 1)] {
        set_uv(&mut blocks, grid, fu, 2, fv, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
    }
    let bell_top = crate::gen_common::clamp_i32(h - 2, cloister_top + 3, (h - 1).max(3));
    for dv in -1i32..=1 {
        for du in -1i32..=1 {
            for y in (cloister_top + 1)..=bell_top {
                if du.abs() == 1 && dv.abs() == 1 || y == bell_top {
                    set_uv(&mut blocks, grid, cu + du, y, v0 + 1 + dv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, cu + du, y, v0 + 1 + dv)));
                }
            }
        }
    }
    set_uv(&mut blocks, grid, cu, bell_top - 1, v0 + 1, PlannedBlock::with_props("minecraft:bell", vec![("facing", "north"), ("attachment", "ceiling")]));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 12. Knightly Jousting Arena: elongated sand tilt course, central barrier, royal pavilion & spectator stands.
pub(crate) fn generate_jousting_arena(
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

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in (v0 + 2)..=(v1 - 2) {
        for u in (u0 + 2)..=(u1 - 2) {
            set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:sand"));
        }
    }
    // Central Timber Tilt Barrier + Crimson/Blue Pavilion Tents at ends + Side Bleachers!
    for v in (v0 + 4)..=(v1 - 4) {
        set_uv(&mut blocks, grid, cu, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, cu, 2, v)));
    }
    for v in (v0 + 3)..=(v1 - 3) {
        for &(su, face) in &[(u0 + 1, grid.dir_right()), (u1 - 1, grid.dir_left())] {
            set_uv(&mut blocks, grid, su, 2, v, pick_stair(pal, MaterialPalette::SLOT_FLOOR, seed, su, 2, v, face, "bottom"));
        }
        set_uv(&mut blocks, grid, u0, 2, v, PlannedBlock::simple("minecraft:oak_planks"));
        set_uv(&mut blocks, grid, u0, 3, v, PlannedBlock::simple("minecraft:oak_fence"));
        set_uv(&mut blocks, grid, u1, 2, v, PlannedBlock::simple("minecraft:oak_planks"));
        set_uv(&mut blocks, grid, u1, 3, v, PlannedBlock::simple("minecraft:oak_fence"));
    }

    // Royal Tent Canopy in middle of Left Bleacher!
    let r_top = 6.min(h - 2);
    for dv in -2..=2 {
        for du in 0..=2 {
            let u = u0 + du;
            let v = cv + dv;
            if du == 2 && dv.abs() == 2 {
                for y in 2..r_top {
                    set_uv(&mut blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                }
            }
            set_uv(&mut blocks, grid, u, r_top, v, PlannedBlock::simple(if (u + v) % 2 == 0 { "minecraft:red_wool" } else { "minecraft:white_wool" }));
        }
    }
    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 13. Military Outpost / Siege Camp: spiked timber palisade, corner watchtower, command tents & trebuchet!
pub(crate) fn generate_military_outpost(
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

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Pointed Log Palisade Wall (`log` + `spruce_fence` spikes on top).
    for v in v0..=v1 {
        for u in u0..=u1 {
            let edge = u == u0 || u == u1 || v == v0 || v == v1;
            let gate = v == v1 && (u - cu).abs() <= 1;
            if edge && !gate {
                let top_p = 3 + ((u + v) % 2);
                for y in 2..=top_p {
                    set_uv(&mut blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                }
                set_uv(&mut blocks, grid, u, top_p + 1, v, PlannedBlock::simple("minecraft:spruce_fence"));
            }
        }
    }

    // Corner Timber Watchtower (`u0+2, v0+2`) + Ladder Access!
    let t_top = 8.min(h - 3);
    for dv in -1..=1 {
        for du in -1..=1 {
            let u = u0 + 2 + du;
            let v = v0 + 2 + dv;
            if du.abs() == 1 && dv.abs() == 1 {
                for y in 2..=t_top {
                    set_uv(&mut blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                }
            }
            set_uv(&mut blocks, grid, u, t_top, v, PlannedBlock::simple("minecraft:spruce_planks"));
            if du.abs() == 1 || dv.abs() == 1 {
                set_uv(&mut blocks, grid, u, t_top + 1, v, PlannedBlock::simple("minecraft:spruce_fence"));
            }
        }
    }
    for y in 2..t_top {
        set_uv(&mut blocks, grid, u0 + 2, y, v0 + 3, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }

    // Wooden Siege Trebuchet inside courtyard!
    let tx = (u1 - 3).max(cu + 1);
    let tz = cv;
    for y in 2..=6 {
        set_uv(&mut blocks, grid, tx - 1, y, tz, make_frame_block_at(pal, "y", seed, tx - 1, y, tz));
        set_uv(&mut blocks, grid, tx + 1, y, tz, make_frame_block_at(pal, "y", seed, tx + 1, y, tz));
    }
    for u in (tx - 1)..=(tx + 1) {
        set_uv(&mut blocks, grid, u, 6, tz, make_beam_block_at(pal, grid.axis_u(), seed, u, 6, tz));
    }
    for dv in -2..=2 {
        set_uv(&mut blocks, grid, tx, 6, tz + dv, make_beam_block_at(pal, grid.axis_v(), seed, tx, 6, tz + dv));
    }
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::simple("minecraft:campfire"));

    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 14. Vauban Star-Bastion Fort: thick polygonal sloped walls, projecting diamond corner ravelins & cannons.
pub(crate) fn generate_star_fort(
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
    let r = ((grid.w.min(grid.d) - 4) / 2).clamp(5, 13);
    let wall_h = (h * 45 / 100).clamp(5, 8);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - r, cu + r, cv - r, cv + r, 1, terrain_offsets);

    // 8-Point Star / Diamond Overlap Geometry!
    for v in (cv - r)..=(cv + r) {
        for u in (cu - r)..=(cu + r) {
            let du = (u - cu).abs();
            let dv = (v - cv).abs();
            let in_rect = du <= r * 7 / 10 && dv <= r * 7 / 10;
            let in_diam = du + dv <= r + 1;
            let inside = in_rect || in_diam;
            if !inside {
                continue;
            }
            let near_edge = !(du + 1 <= r * 7 / 10 && dv + 1 <= r * 7 / 10) && !(du + dv + 1 <= r + 1);
            if near_edge {
                for y in 2..=wall_h {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)));
                }
                if wall_h + 1 < h && (u + v) % 2 == 0 {
                    set_uv(&mut blocks, grid, u, wall_h + 1, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_FOUNDATION, seed, u, wall_h + 1, v), "bottom"));
                }
            }
        }
    }

    // Central Casemate Powder Magazine Bunker!
    let cr = 2.min(r / 3).max(1);
    for v in (cv - cr)..=(cv + cr) {
        for u in (cu - cr)..=(cu + cr) {
            let shell = u == cu - cr || u == cu + cr || v == cv - cr || v == cv + cr;
            for y in 2..=wall_h {
                if shell || y == wall_h {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                }
            }
        }
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + cr));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 15. Alchemist / Apothecary / Hospital: herb garden, brewing stands, cauldrons & gabled infirmary.
pub(crate) fn generate_apothecary(
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
    let u0 = 2;
    let u1 = (grid.w - 3).max(8);
    let v0 = 2;
    let v1 = (grid.d - 3).max(8);
    let wall_top = crate::gen_common::clamp_i32(h / 2 + 1, 6, (h - 4).max(3));
    let cu = (u0 + u1) / 2;
    let cv = (v0 + v1) / 2;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let corner = (u == u0 || u == u1) && (v == v0 || v == v1);
                    let win = !corner && y == 4 && ((u + v) % 2 == 1);
                    let blk = if corner {
                        make_frame_block_at(pal, "y", seed, u, y, v)
                    } else if win {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
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
    build_gable_roof(&mut blocks, grid, pal, seed, u0 - 1, u1 + 1, v0 - 1, v1 + 1, wall_top, h, true);

    // Alchemy Brewing Tables, Water Cauldrons & Herb Flower Pots inside + outside!
    set_uv(&mut blocks, grid, u0 + 1, 2, v0 + 1, PlannedBlock::simple("minecraft:bookshelf"));
    set_uv(&mut blocks, grid, u0 + 2, 2, v0 + 1, PlannedBlock::simple("minecraft:oak_planks"));
    set_uv(&mut blocks, grid, u0 + 2, 3, v0 + 1, PlannedBlock::simple("minecraft:brewing_stand"));
    set_uv(&mut blocks, grid, u1 - 1, 2, v0 + 1, PlannedBlock::simple("minecraft:water_cauldron"));
    set_uv(&mut blocks, grid, u0 + 1, 2, cv, PlannedBlock::simple("minecraft:flower_pot"));
    set_uv(&mut blocks, grid, u1 - 1, 2, cv, make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, u1 - 1, 2, cv)));
    set_uv(&mut blocks, grid, cu, wall_top, cv, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, wall_top, cv), true));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 16. Winery / Bakery with Brick Domed Oven, Vine Pergola & Deep Barrel Cellar (-4..0).
pub(crate) fn generate_winery_bakery(
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
    let u1 = (grid.w - 4).max(7);
    let v0 = 2;
    let v1 = (grid.d - 3).max(7);
    let wall_top = crate::gen_common::clamp_i32(h / 2 + 1, 5, (h - 4).max(3));
    let cv = (v0 + v1) / 2;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Underground Wine Cellar (`-3..0`) with Stacked Oak Barrels & Ladder!
    for v in (v0 + 1)..=(v1 - 1) {
        for u in (u0 + 1)..=(u1 - 1) {
            set_uv(&mut blocks, grid, u, -3, v, PlannedBlock::simple("minecraft:stone_bricks"));
            for y in -2..=0 {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
            if u == u0 + 1 && v > v0 + 1 {
                set_uv(&mut blocks, grid, u, -2, v, PlannedBlock::simple("minecraft:barrel"));
                set_uv(&mut blocks, grid, u, -1, v, PlannedBlock::simple("minecraft:barrel"));
            }
        }
    }

    // Main Stone & Timber Tasting Hall / Bakery.
    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let blk = if y == 3 && ((u + v) % 3 == 0) {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    } else if y <= 3 {
                        pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
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

    // Projecting Brick Baking Oven + Tall Chimney Stack on Right Side (`u1 + 1`)!
    if u1 + 2 < grid.w {
        for y in 1..=(wall_top + 3).min(h - 2) {
            set_uv(&mut blocks, grid, u1 + 1, y, cv, PlannedBlock::simple("minecraft:bricks"));
        }
        set_uv(&mut blocks, grid, u1 + 1, 2, cv, PlannedBlock::simple("minecraft:furnace"));
        set_uv(&mut blocks, grid, u1 + 1, (wall_top + 4).min(h - 1), cv, PlannedBlock::simple("minecraft:campfire"));
    }

    // Cellar Access Ladder + Baker Display Counter with Cake!
    for y in -2..=1 {
        set_uv(&mut blocks, grid, u1 - 1, y, v0 + 1, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }
    set_uv(&mut blocks, grid, u0 + 1, 2, cv, PlannedBlock::simple("minecraft:crafting_table"));
    set_uv(&mut blocks, grid, u0 + 2, 2, cv, PlannedBlock::simple("minecraft:oak_slab"));
    set_uv(&mut blocks, grid, u0 + 2, 3, cv, PlannedBlock::simple("minecraft:cake"));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}
