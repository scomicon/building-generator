use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, make_beam_block_at, set_uv,
};
use crate::gen_landmarks::{
    anchor_plinth, finish_landmark, place_door_stack,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::NoiseField;
use crate::parser::BlueprintSpec;

/// 28. Stepped Art-Deco / Glass Curtain Skyscraper with lobby, repetitive office floors, stairwell & helipad spire.
pub(crate) fn generate_skyscraper(
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
    let base_hw = ((grid.w - 4) / 2).clamp(4, 9);
    let base_hd = ((grid.d - 4) / 2).clamp(4, 9);
    let roof_y = (h - 5).clamp(11, h - 3);
    let story_h = 4;

    anchor_plinth(&mut blocks, grid, pal, seed, cu - base_hw - 1, cu + base_hw + 1, cv - base_hd - 1, cv + base_hd + 1, 1, terrain_offsets);

    // 3 Stepped Setback Tiers from ground up to roof helipad/crown!
    for y in 2..=roof_y {
        let setback = if y > (roof_y * 78) / 100 && base_hw >= 6 {
            2
        } else if y > (roof_y * 48) / 100 && base_hw >= 5 {
            1
        } else {
            0
        };
        let hw = (base_hw - setback).max(3);
        let hd = (base_hd - setback).max(3);
        let is_floor = y == 2 || ((y - 2) % story_h == 0) || y == roof_y;

        for v in (cv - hd)..=(cv + hd) {
            for u in (cu - hw)..=(cu + hw) {
                let shell = u == cu - hw || u == cu + hw || v == cv - hd || v == cv + hd;
                let corner = (u == cu - hw || u == cu + hw) && (v == cv - hd || v == cv + hd);
                if shell {
                    let is_col = corner || (u - cu).abs() % 3 == 0 && (v == cv - hd || v == cv + hd) || (v - cv).abs() % 3 == 0 && (u == cu - hw || u == cu + hw);
                    let blk = if is_floor {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                    } else if is_col {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if is_floor {
                    let is_helipad_h = y == roof_y && (((u - cu).abs() == 1 && (v - cv).abs() <= 1) || (u == cu && v == cv));
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(if is_helipad_h {
                            "minecraft:yellow_concrete"
                        } else if y == roof_y {
                            "minecraft:gray_concrete"
                        } else {
                            "minecraft:smooth_quartz"
                        }),
                    );
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Core elevator & ladder shaft + Beacon Antenna Spire atop crown!
    for y in 2..=roof_y {
        set_uv(&mut blocks, grid, cu, y, cv - 1, PlannedBlock::simple("minecraft:iron_block"));
        set_uv(&mut blocks, grid, cu, y, cv, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }
    for y in (roof_y + 1)..=(h - 1) {
        set_uv(&mut blocks, grid, cu, y, cv - 1, PlannedBlock::simple(if y == h - 1 { "minecraft:sea_lantern" } else { "minecraft:iron_bars" }));
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + base_hd));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 29. Steampunk Industrial Factory: saw-tooth shed roof, twin smoking brick stacks & copper boiler vats.
pub(crate) fn generate_steampunk_factory(
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
    let wall_top = (h / 2 + 2).clamp(6, h - 5);

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=wall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let is_win = y >= 4 && y <= wall_top - 1 && (u + v) % 3 != 0;
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if is_win { "minecraft:glass_pane" } else { "minecraft:bricks" }));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Saw-Tooth Industrial Skylight Shed Roof (`period = 4` along `v`)!
    for v in v0..=v1 {
        let tooth = (v - v0) % 4;
        for u in u0..=u1 {
            if tooth == 0 {
                for dy in 1..=2 {
                    set_uv(&mut blocks, grid, u, (wall_top + dy).min(h - 2), v, PlannedBlock::simple("minecraft:glass"));
                }
            } else {
                let ry = (wall_top + (3 - tooth)).min(h - 2);
                set_uv(&mut blocks, grid, u, ry, v, PlannedBlock::simple("minecraft:cut_copper"));
            }
        }
    }

    // Twin Towering Brick Smokestacks with Campfire Smoke Plumes + Interior Copper Boilers & Furnace!
    let stack_top = (h - 2).clamp(wall_top + 4, h - 1);
    for su in [u0 + 2, (u1 - 2).max(u0 + 4)] {
        for y in 2..=stack_top {
            set_uv(&mut blocks, grid, su, y, v0 + 1, PlannedBlock::simple("minecraft:bricks"));
        }
        set_uv(&mut blocks, grid, su, stack_top + 1, v0 + 1, PlannedBlock::with_props("minecraft:campfire", vec![("lit", "true"), ("signal_fire", "true")]));
    }
    for y in 2..=5 {
        set_uv(&mut blocks, grid, u0 + 2, y, v0 + 3, PlannedBlock::simple("minecraft:copper_block"));
    }
    set_uv(&mut blocks, grid, u1 - 2, 2, v0 + 3, PlannedBlock::simple("minecraft:blast_furnace"));
    set_uv(&mut blocks, grid, u1 - 3, 2, v0 + 3, PlannedBlock::simple("minecraft:anvil"));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 30. Modern Football Stadium Arena: tiered concrete seating bowl, cantilevered canopy, floodlight towers & pitch.
pub(crate) fn generate_modern_stadium(
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
    let rx = ((grid.w - 4) / 2).clamp(6, 16) as f64;
    let rz = ((grid.d - 4) / 2).clamp(6, 16) as f64;
    let mast_top = (h - 2).clamp(10, h - 1);

    anchor_plinth(&mut blocks, grid, pal, seed, 1, grid.w - 2, 1, grid.d - 2, 0, terrain_offsets);

    // Striped Green Pitch Turf + Stepped Bleacher Tiers + Canopy!
    for v in 1..(grid.d - 1) {
        for u in 1..(grid.w - 1) {
            let du = (u as f64 - cu as f64) / rx;
            let dv = (v as f64 - cv as f64) / rz;
            let r_norm = (du * du + dv * dv).sqrt();
            if r_norm > 1.04 {
                continue;
            }
            if r_norm < 0.62 {
                let is_center_line = v == cv || (du.abs() < 0.08 && dv.abs() < 0.08);
                let turf = if is_center_line {
                    "minecraft:white_concrete"
                } else if (v % 2) == 0 {
                    "minecraft:lime_concrete"
                } else {
                    "minecraft:green_concrete"
                };
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(turf));
            } else {
                let tier = ((r_norm - 0.62) / 0.12).floor() as i32;
                let step_y = (2 + tier).min(6);
                for y in 1..=step_y {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if y == step_y { "minecraft:light_gray_concrete" } else { "minecraft:gray_concrete" }));
                }
                if r_norm >= 0.88 {
                    let can_y = (7 + ((r_norm - 0.88) * 10.0).round() as i32).min(h - 4);
                    set_uv(&mut blocks, grid, u, can_y, v, PlannedBlock::simple("minecraft:white_concrete"));
                }
            }
        }
    }

    // 4 Corner Floodlight Masts illuminating the Pitch!
    for &(mu, mv) in &[
        ((cu as f64 - rx * 0.72).round() as i32, (cv as f64 - rz * 0.72).round() as i32),
        ((cu as f64 + rx * 0.72).round() as i32, (cv as f64 - rz * 0.72).round() as i32),
        ((cu as f64 - rx * 0.72).round() as i32, (cv as f64 + rz * 0.72).round() as i32),
        ((cu as f64 + rx * 0.72).round() as i32, (cv as f64 + rz * 0.72).round() as i32),
    ] {
        if grid.in_bounds(mu, mv) {
            for y in 2..=mast_top {
                set_uv(&mut blocks, grid, mu, y, mv, PlannedBlock::simple("minecraft:iron_block"));
            }
            for du in -1..=1 {
                if grid.in_bounds(mu + du, mv) {
                    set_uv(&mut blocks, grid, mu + du, mast_top, mv, PlannedBlock::simple("minecraft:sea_lantern"));
                }
            }
        }
    }

    let [lx, lz] = grid.to_local_xz(cu, cv);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 31. Reinforced Concrete Military Bunker: sloped casemate walls, radar tower, iron blast door & command cellar.
pub(crate) fn generate_bunker(
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
    let bunker_top = 5.min(h - 4);
    let cv = (v0 + v1) / 2;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Underground Armored Generator Room (`-4..0`) + Surface Sloped Pillbox!
    for v in (v0 + 1)..=(v1 - 1) {
        for u in (u0 + 1)..=(u1 - 1) {
            set_uv(&mut blocks, grid, u, -4, v, PlannedBlock::simple("minecraft:gray_concrete"));
            for y in -3..=0 {
                let shell = u == u0 + 1 || u == u1 - 1 || v == v0 + 1 || v == v1 - 1;
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(if shell { "minecraft:polished_andesite" } else { "minecraft:air" }));
            }
        }
    }

    for y in 2..=bunker_top {
        let inset = if y >= bunker_top - 1 { 1 } else { 0 };
        for v in (v0 + inset)..=(v1 - inset) {
            for u in (u0 + inset)..=(u1 - inset) {
                let shell = u == u0 + inset || u == u1 - inset || v == v0 + inset || v == v1 - inset;
                let slit = y == 3 && ((u - cu).abs() % 3 == 0 && (v == v0 + inset || v == v1 - inset));
                if slit {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:iron_bars"));
                } else if shell || y == bunker_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:gray_concrete"));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Rotating Concrete Roof Radar Dish + Vertical Access Ladder!
    let radar_top = (bunker_top + 4).min(h - 1);
    for y in (bunker_top + 1)..=radar_top {
        set_uv(&mut blocks, grid, cu, y, cv, PlannedBlock::simple("minecraft:iron_bars"));
    }
    if radar_top < h {
        set_uv(&mut blocks, grid, cu, radar_top, cv, PlannedBlock::simple("minecraft:iron_block"));
        set_uv(&mut blocks, grid, cu, radar_top, (cv + 1).min(v1), PlannedBlock::simple("minecraft:white_concrete"));
    }
    for y in -3..=1 {
        set_uv(&mut blocks, grid, u1 - 2, y, v0 + 2, PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_front())]));
    }
    set_uv(&mut blocks, grid, u0 + 2, 2, cv, PlannedBlock::simple("minecraft:sea_lantern"));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 32. Spaceport Launchpad & Multi-Stage Rocket: launch gantry truss tower, umbilical bridge & white-orange rocket!
pub(crate) fn generate_launchpad_rocket(
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
    let pad_y = 3;

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    // Concrete Launch Pad Deck with Hazard Stripes!
    for v in (v0 + 1)..=(v1 - 1) {
        for u in (u0 + 1)..=(u1 - 1) {
            let hazard = u == u0 + 1 || u == u1 - 1 || v == v0 + 1 || v == v1 - 1;
            set_uv(
                &mut blocks,
                grid,
                u,
                pad_y,
                v,
                PlannedBlock::simple(if hazard && (u + v) % 2 == 0 { "minecraft:yellow_concrete" } else { "minecraft:gray_concrete" }),
            );
        }
    }

    // Steel Gantry Service Tower at Left `(u_g = cu - 4)` + Boarding Umbilical Swing Arm!
    let ug = (cu - 4).max(u0 + 2);
    let rocket_top = (h - 2).clamp(12, h - 1);
    for y in (pad_y + 1)..=(rocket_top - 2) {
        for dv in -1i32..=1 {
            for du in -1i32..=1 {
                if du.abs() == 1 && dv.abs() == 1 {
                    set_uv(&mut blocks, grid, ug + du, y, cv + dv, PlannedBlock::simple("minecraft:iron_block"));
                } else if y % 4 == 0 && (du.abs() == 1 || dv.abs() == 1) {
                    set_uv(&mut blocks, grid, ug + du, y, cv + dv, make_beam_block_at(pal, grid.axis_u(), seed, ug + du, y, cv + dv));
                }
            }
        }
    }
    // High Crew Access Umbilical Arm connecting Gantry Tower directly to Rocket Payload Fairing!
    let arm_y = (rocket_top * 2 / 3).clamp(pad_y + 4, rocket_top - 3);
    for u in ug..=(cu - 1) {
        set_uv(&mut blocks, grid, u, arm_y, cv, PlannedBlock::simple("minecraft:iron_block"));
    }

    // Multi-Stage Launch Vehicle Rocket centered at `(cu, cv)` with Side Boosters, Nozzle Flame & Fairing!
    for (du, dv) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
        if grid.in_bounds(cu + du, cv + dv) {
            set_uv(&mut blocks, grid, cu + du, pad_y + 1, cv + dv, PlannedBlock::simple("minecraft:gray_concrete"));
            for y in (pad_y + 2)..=(pad_y + 6).min(h - 3) {
                set_uv(&mut blocks, grid, cu + du, y, cv + dv, PlannedBlock::simple("minecraft:white_concrete"));
            }
        }
    }
    for y in (pad_y + 1)..=rocket_top {
        let nose = y >= rocket_top - 2;
        let orange_tank = y >= pad_y + 3 && y <= arm_y;
        let body_r: i32 = if nose { 0 } else { 1 };
        for dv in -body_r..=body_r {
            for du in -body_r..=body_r {
                if body_r == 1 && du.abs() == 1 && dv.abs() == 1 {
                    continue;
                }
                let blk = if nose {
                    "minecraft:white_concrete"
                } else if orange_tank {
                    "minecraft:orange_terracotta"
                } else {
                    "minecraft:white_concrete"
                };
                set_uv(&mut blocks, grid, cu + du, y, cv + dv, PlannedBlock::simple(blk));
            }
        }
    }
    if rocket_top + 1 < h {
        set_uv(&mut blocks, grid, cu, rocket_top + 1, cv, PlannedBlock::simple("minecraft:lightning_rod"));
    }

    let [lx, lz] = grid.to_local_xz(cu, v1 - 1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, pad_y + 1, lz]))
}

/// 33. Offshore Oil Rig Platform: deep steel truss legs over sea, drilling derrick, helipad & crew deck!
pub(crate) fn generate_offshore_rig(
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
    let u0 = 2;
    let u1 = grid.w - 3;
    let v0 = 2;
    let v1 = grid.d - 3;
    let deck_y = 6.min(h - 7).max(4);

    anchor_plinth(&mut blocks, grid, pal, seed, 1, grid.w - 2, 1, grid.d - 2, 0, terrain_offsets);
    // Deep Sea Base Pool around the Rig!
    for v in 1..(grid.d - 1) {
        for u in 1..(grid.w - 1) {
            let rim = u == 1 || u == grid.w - 2 || v == 1 || v == grid.d - 2;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                if rim {
                    PlannedBlock::simple("minecraft:stone_bricks")
                } else {
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")])
                },
            );
        }
    }

    // 4 Giant Yellow/Steel Truss Jacket Legs rising cleanly out of sea to Deck (`deck_y`)!
    for &(lu, lv) in &[(u0 + 1, v0 + 1), (u1 - 1, v0 + 1), (u0 + 1, v1 - 1), (u1 - 1, v1 - 1)] {
        for y in 1..=deck_y {
            set_uv(&mut blocks, grid, lu, y, lv, PlannedBlock::simple("minecraft:yellow_concrete"));
        }
    }

    // Steel Main Working Platform + Perimeter Guardrails!
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            set_uv(&mut blocks, grid, u, deck_y, v, PlannedBlock::simple(if rim { "minecraft:yellow_concrete" } else { "minecraft:gray_concrete" }));
            if rim {
                set_uv(&mut blocks, grid, u, deck_y + 1, v, PlannedBlock::simple("minecraft:iron_bars"));
            }
        }
    }

    // Tall Steel Lattice Drilling Derrick Tower centered + Crew Cabin Module!
    let derrick_top = (h - 2).clamp(deck_y + 5, h - 1);
    for y in (deck_y + 1)..=derrick_top {
        let inset: i32 = if y > derrick_top - 3 { 1 } else { 0 };
        for dv in (-1 + inset)..=(1 - inset) {
            for du in (-1 + inset)..=(1 - inset) {
                if du.abs() == dv.abs() {
                    set_uv(&mut blocks, grid, cu + du, y, cv + dv, PlannedBlock::simple("minecraft:iron_block"));
                }
            }
        }
    }
    // Yellow Helipad Pad on Corner!
    for v in (v1 - 2)..=v1 {
        for u in u0..=(u0 + 2) {
            set_uv(&mut blocks, grid, u, deck_y + 1, v, PlannedBlock::simple("minecraft:yellow_carpet"));
        }
    }

    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, deck_y + 1, lz]))
}
