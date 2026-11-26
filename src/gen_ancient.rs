use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, make_beam_block_at, make_frame_block_at,
    make_leaves, make_pillar_block_at, make_slab, set_uv, terrain_rel_at,
};
use crate::gen_landmarks::{
    anchor_plinth, build_dome_shell, build_gable_roof, finish_landmark, make_light,
    place_door_stack,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::NoiseField;
use crate::parser::BlueprintSpec;

/// 1. Roman Pantheon: deep columned portico + wide rotunda drum + coffered dome with open central oculus.
pub(crate) fn generate_pantheon(
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
    let max_r = ((grid.w - 4) / 2).clamp(4, 12);
    let drum_cv = crate::gen_common::clamp_i32((grid.d - 2) / 2 - 2, max_r + 1, (grid.d - max_r - 2).max(4));
    let drum_cv = drum_cv.min(grid.d - 5);
    let r = max_r.min(drum_cv - 1).min(grid.d - 5 - drum_cv).max(3);
    let drum_top = crate::gen_common::clamp_i32((h * 52) / 100, 6, (h - 5).max(3));
    let dome_h = crate::gen_common::clamp_i32(h - 1 - drum_top, 4, (r + 2).max(2));

    // Rotunda base drum + coffered interior floor & wall niches.
    for v in (drum_cv - r)..=(drum_cv + r) {
        for u in (cu - r)..=(cu + r) {
            let d = (((u - cu) * (u - cu) + (v - drum_cv) * (v - drum_cv)) as f64).sqrt();
            if d > f64::from(r) + 0.45 {
                continue;
            }
            let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-6, 0);
            for y in (terr - 2)..=1 {
                let blk = if y == 1 {
                    if (u + v).abs() % 2 == 0 {
                        "minecraft:smooth_quartz".to_owned()
                    } else {
                        "minecraft:polished_diorite".to_owned()
                    }
                } else {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
            let is_shell = d >= f64::from(r) - 0.85;
            for y in 2..=drum_top {
                if is_shell {
                    let is_cornice = y == drum_top || y == 2 || y == drum_top / 2 + 1;
                    let blk = if is_cornice {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    } else if (u == cu || v == drum_cv) && y <= 5 {
                        pal.pick(MaterialPalette::SLOT_PILLAR, seed, u, y, v)
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

    // Coffered hemispherical dome with central open oculus (`r >= 5`).
    build_dome_shell(
        &mut blocks,
        grid,
        pal,
        seed,
        cu,
        drum_cv,
        r,
        drum_top,
        dome_h,
        true,
        MaterialPalette::SLOT_ROOF,
    );

    // Altar sanctuary & golden braziers inside rotunda.
    set_uv(&mut blocks, grid, cu, 2, drum_cv - r + 2, PlannedBlock::simple("minecraft:gold_block"));
    set_uv(&mut blocks, grid, cu, 3, drum_cv - r + 2, PlannedBlock::simple("minecraft:lectern"));
    for &(lu, lv) in &[(cu - 2, drum_cv - 2), (cu + 2, drum_cv - 2), (cu - 2, drum_cv + 2), (cu + 2, drum_cv + 2)] {
        if (((lu - cu) * (lu - cu) + (lv - drum_cv) * (lv - drum_cv)) as f64).sqrt() < f64::from(r) - 1.5 {
            set_uv(&mut blocks, grid, lu, 2, lv, PlannedBlock::simple("minecraft:quartz_pillar"));
            set_uv(&mut blocks, grid, lu, 3, lv, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lu, 3, lv), false));
        }
    }

    // Deep Classical Columned Front Portico + Triangular Pediment facing `v = grid.d - 2`!
    let pv0 = drum_cv + r;
    let pv1 = (grid.d - 2).max(pv0 + 3).min(grid.d - 1);
    let phw = (r - 1).clamp(3, 5);
    anchor_plinth(&mut blocks, grid, pal, seed, cu - phw, cu + phw, pv0, pv1, 1, terrain_offsets);
    let portico_top = crate::gen_common::clamp_i32(drum_top - 1, 5, drum_top.max(3));
    for v in (pv0 + 1)..=pv1 {
        for u in (cu - phw)..=(cu + phw) {
            let is_col = (v == pv1 || v == pv0 + 2) && ((u - cu).abs() == phw || (u - cu).abs() == phw - 2 || u == cu);
            if is_col {
                for y in 2..portico_top {
                    set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                }
            }
            set_uv(
                &mut blocks,
                grid,
                u,
                portico_top,
                v,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, portico_top, v)),
            );
        }
    }
    build_gable_roof(
        &mut blocks,
        grid,
        pal,
        seed,
        cu - phw,
        cu + phw,
        pv0,
        pv1,
        portico_top,
        h,
        true,
    );

    // Clear monumental arched portal entry through rotunda front wall.
    for v in (pv0 - 1)..=(pv0 + 1) {
        for u in (cu - 1)..=(cu + 1) {
            for y in 2..=4 {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
        }
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, pv0));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 2. Parthenon / Classical Peripteral Temple: 3-step stylobate, 4-sided colonnade, cella & gable pediments.
pub(crate) fn generate_parthenon(
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

    // 3-stepped marble stylobate podium!
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    let step2_u0 = u0 + 1;
    let step2_u1 = u1 - 1;
    let step2_v0 = v0 + 1;
    let step2_v1 = v1 - 1;
    for v in step2_v0..=step2_v1 {
        for u in step2_u0..=step2_u1 {
            set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:smooth_quartz"));
        }
    }

    let col_u0 = step2_u0;
    let col_u1 = step2_u1;
    let col_v0 = step2_v0;
    let col_v1 = step2_v1;
    let eave_y = crate::gen_common::clamp_i32(h - 5, 7, (h - 3).max(3));

    // Outer 4-sided Peripteral Colonnade (`quartz_pillar` spaced cleanly every 2 blocks).
    for v in col_v0..=col_v1 {
        for u in col_u0..=col_u1 {
            let on_perim = u == col_u0 || u == col_u1 || v == col_v0 || v == col_v1;
            if on_perim && ((u - col_u0) % 2 == 0 && (v - col_v0) % 2 == 0 || (u == col_u0 || u == col_u1) && (v - col_v0) % 2 == 0 || (v == col_v0 || v == col_v1) && (u - col_u0) % 2 == 0) {
                for y in 3..=eave_y {
                    set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                }
            }
            if on_perim {
                set_uv(&mut blocks, grid, u, eave_y, v, PlannedBlock::simple("minecraft:chiseled_quartz_block"));
            }
        }
    }

    // Inner Cella Sanctuary Chamber + Statue of Athena (`gold_block` + braziers).
    let cel_u0 = (col_u0 + 2).min(cu - 2);
    let cel_u1 = (col_u1 - 2).max(cu + 2);
    let cel_v0 = (col_v0 + 2).min(grid.d / 2 - 2);
    let cel_v1 = (col_v1 - 2).max(grid.d / 2 + 2);
    for v in cel_v0..=cel_v1 {
        for u in cel_u0..=cel_u1 {
            let shell = u == cel_u0 || u == cel_u1 || v == cel_v0 || v == cel_v1;
            if shell {
                for y in 3..=eave_y {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:smooth_quartz"));
                }
            }
        }
    }
    for y in 3..=(eave_y - 2).max(5) {
        set_uv(&mut blocks, grid, cu, y, cel_v0 + 2, PlannedBlock::simple("minecraft:gold_block"));
    }
    set_uv(&mut blocks, grid, cu - 1, 3, cel_v0 + 2, PlannedBlock::simple("minecraft:sea_lantern"));
    set_uv(&mut blocks, grid, cu + 1, 3, cel_v0 + 2, PlannedBlock::simple("minecraft:sea_lantern"));

    // Triangular Pediment & Gabled Marble Roof spanning length `v`.
    build_gable_roof(&mut blocks, grid, pal, seed, col_u0, col_u1, col_v0, col_v1, eave_y, h, true);
    // Gold pediment central medallion.
    if eave_y + 2 < h {
        set_uv(&mut blocks, grid, cu, eave_y + 1, col_v1, PlannedBlock::simple("minecraft:gold_block"));
    }

    for y in 3..=5 {
        set_uv(&mut blocks, grid, cu, y, cel_v1, PlannedBlock::simple("minecraft:air"));
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 3, cel_v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 3. Hanging Gardens of Babylon: cascading sandstone terraces, waterfalls, lush greenery & summit kiosk.
pub(crate) fn generate_hanging_gardens(
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
    let max_r = ((grid.w.min(grid.d) - 4) / 2).clamp(5, 13);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - max_r, cu + max_r, cv - max_r, cv + max_r, 0, terrain_offsets);

    // Lower reflection water basin ring around tier 1.
    for v in (cv - max_r)..=(cv + max_r) {
        for u in (cu - max_r)..=(cu + max_r) {
            let d = (u - cu).abs().max((v - cv).abs());
            if d == max_r {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 1, v)));
            } else if d == max_r - 1 {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
            }
        }
    }

    let tier_h = 4;
    let tiers = ((h - 4) / tier_h).clamp(2, 4);
    let mut r = (max_r - 1).max(4);
    let mut cur_y = 1;
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, cur_y, cv);

    for t in 0..tiers {
        let top_y = (cur_y + tier_h - 1).min(h - 4);
        if r < 2 || top_y <= cur_y + 1 {
            break;
        }
        for v in (cv - r)..=(cv + r) {
            for u in (cu - r)..=(cu + r) {
                let d_edge = (u - cu).abs().max((v - cv).abs());
                let is_col = (u - cu).abs() == r && (v - cv).abs() == r || ((u == cu || v == cv) && d_edge == r);
                let is_waterfall = u == cu && v == cv + r;
                if d_edge == r {
                    if is_waterfall {
                        for y in (cur_y + 1)..=top_y {
                            set_uv(&mut blocks, grid, u, y, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                        }
                    } else if is_col || (u + v) % 3 == 0 {
                        for y in (cur_y + 1)..=top_y {
                            set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                        }
                    } else {
                        set_uv(&mut blocks, grid, u, top_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, top_y, v)));
                    }
                    // Draped hanging foliage along terrace parapets!
                    if !is_waterfall && ((u + v + t) % 2 == 0) {
                        set_uv(&mut blocks, grid, u, top_y + 1, v, make_leaves(&leaf_id));
                    }
                } else if d_edge < r {
                    set_uv(&mut blocks, grid, u, top_y, v, PlannedBlock::simple("minecraft:grass_block"));
                    if d_edge == r - 1 && (u + v) % 4 == 0 {
                        set_uv(&mut blocks, grid, u, top_y + 1, v, make_leaves(&leaf_id));
                    }
                }
            }
        }
        cur_y = top_y;
        r = (r - 2).max(2);
    }

    // Summit Canopy Pergola Pavilion with Golden Spring!
    let k_top = (cur_y + 3).min(h - 1);
    for dv in -2..=2 {
        for du in -2..=2 {
            let u = cu + du;
            let v = cv + dv;
            if du.abs() == 2 && dv.abs() == 2 {
                for y in (cur_y + 1)..k_top {
                    set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                }
            }
            set_uv(&mut blocks, grid, u, k_top, v, make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, u, k_top, v), "bottom"));
        }
    }
    set_uv(&mut blocks, grid, cu, cur_y + 1, cv, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
    let [lx, lz] = grid.to_local_xz(cu, cv + max_r);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 4. Grand Mosque / Sultan Palace: turquoise-gold bulbous dome, 4 corner minarets & iwan arched portal.
pub(crate) fn generate_mosque_palace(
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
    let hw = ((grid.w - 4) / 2).clamp(5, 11);
    let hd = ((grid.d - 4) / 2).clamp(5, 11);
    let u0 = cu - hw;
    let u1 = cu + hw;
    let v0 = cv - hd;
    let v1 = cv + hd;
    let hall_top = crate::gen_common::clamp_i32((h * 45) / 100, 6, (h - 6).max(3));
    let dome_r = (hw.min(hd) - 2).clamp(3, 8);
    let dome_h = crate::gen_common::clamp_i32(h - 3 - hall_top, 4, (dome_r + 2).max(2));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=hall_top {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                let portal = v == v1 && (u - cu).abs() <= 1 && y <= 5;
                if portal {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                } else if shell {
                    let is_win = y >= 4 && y <= hall_top - 1 && ((u + v) % 3 == 0);
                    let blk = if is_win {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    } else if y == hall_top || y == 3 {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if y == hall_top {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Main Turquoise Bulbous Dome + Golden Crescent Finial!
    build_dome_shell(
        &mut blocks,
        grid,
        pal,
        seed,
        cu,
        cv,
        dome_r,
        hall_top,
        dome_h,
        false,
        MaterialPalette::SLOT_ROOF,
    );
    let apex_y = (hall_top + dome_h).min(h - 3);
    set_uv(&mut blocks, grid, cu, apex_y + 1, cv, PlannedBlock::simple("minecraft:gold_block"));
    if apex_y + 2 < h {
        set_uv(&mut blocks, grid, cu, apex_y + 2, cv, PlannedBlock::simple("minecraft:lightning_rod"));
    }

    // 4 Corner Tall Slender Minarets with Encircling Balconies!
    let min_top = crate::gen_common::clamp_i32(h - 4, hall_top + 4, (h - 2).max(3));
    for &(mu, mv) in &[(u0, v0), (u1, v0), (u0, v1), (u1, v1)] {
        for y in 2..=min_top {
            for dv in -1..=1 {
                for du in -1..=1 {
                    let u = (mu + du).clamp(0, grid.w - 1);
                    let v = (mv + dv).clamp(0, grid.d - 1);
                    let is_balc = y == hall_top + 2 && (du != 0 || dv != 0);
                    if (du == 0 && dv == 0) || y <= hall_top || is_balc {
                        let blk = if is_balc || y == min_top {
                            pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                        } else {
                            pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                        };
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                    }
                }
            }
        }
        set_uv(&mut blocks, grid, mu, min_top + 1, mv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, mu, min_top + 1, mv)));
        if min_top + 2 < h {
            set_uv(&mut blocks, grid, mu, min_top + 2, mv, PlannedBlock::simple("minecraft:gold_block"));
        }
    }

    // Rich patterned prayer carpet interior & chandelier.
    for v in (v0 + 2)..=(v1 - 2) {
        for u in (u0 + 2)..=(u1 - 2) {
            set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_CARPET, seed, u, 2, v)));
        }
    }
    set_uv(&mut blocks, grid, cu, hall_top - 1, cv, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, hall_top - 1, cv), true));

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 5. Ishtar Gate of Babylon: lapis-lazuli glazed twin pylons with golden bands & arched portal tunnel.
pub(crate) fn generate_ishtar_gate(
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
    let hw = ((grid.w - 4) / 2).clamp(4, 11);
    let hd = (grid.d / 3).clamp(3, 6);
    let u0 = cu - hw;
    let u1 = cu + hw;
    let v0 = grid.d / 2 - hd;
    let v1 = grid.d / 2 + hd;
    let arch_hw = (hw / 3).clamp(2, 3);
    let arch_h = crate::gen_common::clamp_i32(h * 55 / 100, 5, (h - 4).max(3));
    let top_y = crate::gen_common::clamp_i32(h - 2, arch_h + 3, (h - 1).max(4));

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);

    for v in v0..=v1 {
        for u in u0..=u1 {
            let du = (u - cu).abs();
            let in_portal = du <= arch_hw;
            let in_tower = du >= arch_hw + 1 && du <= hw - 1;
            let is_outer_tower = !in_portal && (v == v0 || v == v1 || du >= arch_hw + 1);
            if !in_portal && !is_outer_tower && !in_tower {
                continue;
            }
            for y in 2..=top_y {
                let open_arch = in_portal && y <= arch_h - if du == arch_hw { 1 } else { 0 };
                let tower_only_above = in_portal && y > top_y - 2;
                if open_arch || tower_only_above {
                    if open_arch {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                    continue;
                }
                let is_gold_frieze = y == 4 || y == arch_h + 1 || y == top_y - 1;
                let blk = if is_gold_frieze {
                    "minecraft:gold_block"
                } else if (u + v) % 4 == 0 {
                    "minecraft:lapis_block"
                } else {
                    "minecraft:blue_concrete"
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
            // Stepped Babylonian battlement merlons atop towers & bridge parapet!
            let merlon_y = if in_portal { top_y - 1 } else { top_y + 1 };
            if merlon_y < h && ((u + v) % 2 == 0) {
                set_uv(&mut blocks, grid, u, merlon_y, v, PlannedBlock::simple("minecraft:gold_block"));
            }
        }
    }

    set_uv(&mut blocks, grid, cu, arch_h, (v0 + v1) / 2, make_light("minecraft:lantern".into(), true));
    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 6. Angkor Wat / Hindu-Buddhist Stupa Temple: stepped mossy stone plinth & 5 lotus-bud prang towers.
pub(crate) fn generate_angkor_temple(
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
    let max_r = ((grid.w.min(grid.d) - 4) / 2).clamp(5, 12);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - max_r, cu + max_r, cv - max_r, cv + max_r, 2, terrain_offsets);

    // 2 Stepped Mossy Stone Gallery Platforms!
    let base_top = 4;
    for r in [max_r - 1, max_r - 3] {
        if r < 3 {
            continue;
        }
        let y_step = if r == max_r - 1 { 3 } else { base_top };
        for v in (cv - r)..=(cv + r) {
            for u in (cu - r)..=(cu + r) {
                let d_edge = (u - cu).abs().max((v - cv).abs());
                let blk = if d_edge == r && (u + v) % 3 == 0 {
                    "minecraft:mossy_stone_bricks"
                } else {
                    "minecraft:stone_bricks"
                };
                set_uv(&mut blocks, grid, u, y_step, v, PlannedBlock::simple(blk));
            }
        }
    }

    // Helper for tapered Lotus-Bud Prang Tower (corner prangs + huge central Meru tower).
    let off = (max_r - 3).clamp(3, 6);
    let towers = [
        (cu, cv, crate::gen_common::clamp_i32(h - 2, 11, (h - 1).max(3)), 2),
        (cu - off, cv - off, crate::gen_common::clamp_i32(h * 62 / 100, 7, (h - 4).max(2)), 1),
        (cu + off, cv - off, crate::gen_common::clamp_i32(h * 62 / 100, 7, (h - 4).max(2)), 1),
        (cu - off, cv + off, crate::gen_common::clamp_i32(h * 62 / 100, 7, (h - 4).max(2)), 1),
        (cu + off, cv + off, crate::gen_common::clamp_i32(h * 62 / 100, 7, (h - 4).max(2)), 1),
    ];
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, 4, cv);

    for (tu, tv, t_top, base_w) in towers {
        for y in (base_top + 1)..=t_top {
            let frac = (y - base_top) as f64 / (t_top - base_top).max(1) as f64;
            let tr: i32 = if frac < 0.45 {
                base_w
            } else if frac < 0.80 {
                (base_w - 1).max(1)
            } else {
                0
            };
            for dv in -tr..=tr {
                for du in -tr..=tr {
                    let is_inner = tu == cu && tv == cv && y <= base_top + 3 && (du.abs() + dv.abs() <= 1);
                    if is_inner && y < base_top + 3 {
                        set_uv(&mut blocks, grid, tu + du, y, tv + dv, PlannedBlock::simple("minecraft:air"));
                        continue;
                    }
                    let blk = if y == t_top {
                        "minecraft:gold_block"
                    } else if (tu + du + y) % 5 == 0 {
                        "minecraft:mossy_stone_bricks"
                    } else if (du + dv).abs() == tr * 2 {
                        "minecraft:chiseled_stone_bricks"
                    } else {
                        "minecraft:stone_bricks"
                    };
                    set_uv(&mut blocks, grid, tu + du, y, tv + dv, PlannedBlock::simple(blk));
                }
            }
        }
        // Clusters of jungle moss & leaves clinging to lower slopes.
        if t_top > 8 {
            set_uv(&mut blocks, grid, tu + base_w, base_top + 1, tv, make_leaves(&leaf_id));
        }
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, base_top + 1, cv + 2));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 7. Japanese Floating Water Shrine (Itsukushima / Fushimi Torii Avenue over water mirror).
pub(crate) fn generate_water_shrine(
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

    // Shallow reflective lagoon + stone outer border rim!
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 0, terrain_offsets);
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                if rim {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 1, v))
                } else {
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")])
                },
            );
        }
    }

    // Elevated Stilt Timber Pier Boardwalk down the central axis (`v = v0+3 .. v1`) + Repeated Vermilion Torii Tunnel!
    let pier_y = 2;
    let sanct_v0 = v0 + 2;
    let sanct_v1 = (sanct_v0 + 5).min(grid.d / 2 + 1);
    for v in sanct_v1..=v1 {
        for u in (cu - 1)..=(cu + 1) {
            if (v % 3 == 0) && (u == cu - 1 || u == cu + 1) {
                set_uv(&mut blocks, grid, u, 1, v, make_frame_block_at(pal, "y", seed, u, 1, v));
            }
            set_uv(&mut blocks, grid, u, pier_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, pier_y, v)));
        }
        // Rhythmically spaced Crimson Torii Gates along the approach pier!
        if v > sanct_v1 + 1 && (v - sanct_v1) % 3 == 0 {
            let gate_top = (pier_y + 4).min(h - 2);
            for gu in [cu - 2, cu + 2] {
                for y in 1..=gate_top {
                    set_uv(&mut blocks, grid, gu, y, v, make_frame_block_at(pal, "y", seed, gu, y, v));
                }
            }
            for gu in (cu - 3).max(0)..=(cu + 3).min(grid.w - 1) {
                set_uv(&mut blocks, grid, gu, gate_top, v, make_beam_block_at(pal, grid.axis_u(), seed, gu, gate_top, v));
            }
            set_uv(&mut blocks, grid, cu, gate_top - 1, v, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, gate_top - 1, v), true));
        }
    }

    // Rear Over-Water Sanctuary Pavilion (Honden) with Pagoda Roof!
    let shw = ((grid.w - 6) / 2).clamp(3, 5);
    let wall_top = (pier_y + 4).min(h - 4);
    for v in sanct_v0..=sanct_v1 {
        for u in (cu - shw)..=(cu + shw) {
            set_uv(&mut blocks, grid, u, pier_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, pier_y, v)));
            let shell = u == cu - shw || u == cu + shw || v == sanct_v0 || v == sanct_v1;
            let open_front = v == sanct_v1 && (u - cu).abs() <= 1;
            if shell && !open_front {
                for y in (pier_y + 1)..=wall_top {
                    let is_pillar = (u == cu - shw || u == cu + shw) && (v == sanct_v0 || v == sanct_v1);
                    let blk = if is_pillar {
                        make_frame_block_at(pal, "y", seed, u, y, v)
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                }
            }
        }
    }
    build_gable_roof(&mut blocks, grid, pal, seed, cu - shw - 1, cu + shw + 1, sanct_v0 - 1, sanct_v1 + 1, wall_top, h, true);
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, pier_y + 1, sanct_v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 8. Monumental Sandstone Sphinx with outstretched paws, Altar Shrine between paws & internal Tomb.
pub(crate) fn generate_sphinx(
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
    let body_u0 = (cu - 4).max(2);
    let body_u1 = (cu + 4).min(grid.w - 3);
    let back_v = 3;
    let chest_v = crate::gen_common::clamp_i32(grid.d * 55 / 100, back_v + 4, (grid.d - 6).max(3));
    let paw_v1 = (grid.d - 2).max(chest_v + 3).min(grid.d - 1);
    let back_h = crate::gen_common::clamp_i32((h * 45) / 100, 5, (h - 6).max(3));
    let head_top = crate::gen_common::clamp_i32(h - 2, back_h + 4, (h - 1).max(4));

    anchor_plinth(&mut blocks, grid, pal, seed, body_u0 - 1, body_u1 + 1, back_v - 1, paw_v1, 1, terrain_offsets);

    // Sculpted Crouching Lion Body (hollow internal tomb + smooth sandstone hide).
    for v in back_v..=chest_v {
        for u in body_u0..=body_u1 {
            let on_edge = u == body_u0 || u == body_u1 || v == back_v || v == chest_v;
            let body_h = if v == back_v || v == chest_v { back_h - 1 } else { back_h };
            for y in 2..=body_h {
                if !on_edge && y > 2 && y < body_h && (u - cu).abs() <= 2 && v > back_v + 1 && v < chest_v - 1 {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:smooth_sandstone"));
                }
            }
        }
    }

    // Two Outstretched Forepaws toward `paw_v1` framing a sacred Altar Stele!
    for &(pu0, pu1) in &[(body_u0, cu - 2), (cu + 2, body_u1)] {
        for v in (chest_v + 1)..=paw_v1 {
            for u in pu0..=pu1 {
                for y in 2..=3 {
                    let is_claw = v == paw_v1 && (u == pu0 || u == pu1) && y == 3;
                    if !is_claw {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:sandstone"));
                    }
                }
            }
        }
    }

    // Towering Pharaoh Nemes Head & Headdress above the chest (`chest_v - 2 ..= chest_v + 1`).
    let hv0 = (chest_v - 3).max(back_v + 1);
    let hv1 = (chest_v + 1).min(paw_v1 - 1);
    let head_hw = 3.min(cu - body_u0);
    for y in back_h..=head_top {
        let inset = if y >= head_top - 1 { 1 } else { 0 };
        for v in hv0..=hv1 {
            for u in (cu - head_hw + inset)..=(cu + head_hw - inset) {
                let is_crown = y == head_top || ((y - back_h) % 2 == 0 && v == hv1 && (u - cu).abs() == head_hw - inset);
                let is_eye = y == head_top - 2 && v == hv1 && (u - cu).abs() == 1;
                let blk = if is_eye {
                    "minecraft:lapis_block"
                } else if is_crown {
                    "minecraft:gold_block"
                } else {
                    "minecraft:smooth_sandstone"
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
        }
    }

    // Shrine Altar & Entrance between the front paws into the Sphinx's heart!
    set_uv(&mut blocks, grid, cu, 2, chest_v + 2, PlannedBlock::simple("minecraft:chiseled_sandstone"));
    set_uv(&mut blocks, grid, cu, 3, chest_v + 2, PlannedBlock::simple("minecraft:lantern"));
    set_uv(&mut blocks, grid, cu, 2, (back_v + chest_v) / 2, PlannedBlock::simple("minecraft:gold_block"));
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, chest_v));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}
