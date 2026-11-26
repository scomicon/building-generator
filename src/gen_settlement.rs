use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, PlannedBlock, make_leaves, make_slab, prune_map_unsupported,
    resolve_connections_and_shapes,
};
use crate::model::{BuildCategory, BuildRequest, BuildStyle, CardinalFacing, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::BlueprintSpec;

fn terrain_at_wxz(arr: Option<&[i32]>, w: i32, lx: i32, lz: i32) -> i32 {
    arr.and_then(|s| s.get((lz * w + lx) as usize).copied()).unwrap_or(0)
}

fn slice_map_2d(
    src: Option<&[i32]>,
    full_w: i32,
    x0: i32,
    z0: i32,
    sw: i32,
    sd: i32,
    base_rel: i32,
    is_offset: bool,
) -> Option<Vec<i32>> {
    let s = src?;
    let mut out = Vec::with_capacity((sw * sd) as usize);
    for dz in 0..sd {
        for dx in 0..sw {
            let v = s.get(((z0 + dz) * full_w + (x0 + dx)) as usize).copied().unwrap_or(0);
            out.push(if is_offset { v - base_rel } else { v });
        }
    }
    Some(out)
}

#[derive(Clone, Copy)]
struct LotPlan {
    x0: i32,
    z0: i32,
    w: u16,
    d: u16,
    facing: CardinalFacing,
    cat: BuildCategory,
    desc: &'static str,
}

fn pave_block_for_style(
    style: BuildStyle,
    pal: &MaterialPalette,
    seed: u64,
    x: i32,
    y: i32,
    z: i32,
    hash: f64,
    is_border: bool,
) -> PlannedBlock {
    match style {
        BuildStyle::Modern | BuildStyle::Classical => {
            if is_border {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, y, z))
            } else if !pal.has_extra(MaterialPalette::SLOT_PAVING) && hash > 0.72 {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, x, y, z))
            } else {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, x, y, z))
            }
        }
        BuildStyle::Desert => {
            if is_border && !pal.has_extra(MaterialPalette::SLOT_PAVING) {
                PlannedBlock::simple("minecraft:cut_sandstone")
            } else if pal.has_extra(MaterialPalette::SLOT_PAVING) || hash > 0.65 {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, x, y, z))
            } else {
                PlannedBlock::simple("minecraft:sandstone")
            }
        }
        _ => {
            if is_border && hash < 0.45 && !pal.has_extra(MaterialPalette::SLOT_PAVING) {
                PlannedBlock::simple("minecraft:dirt_path")
            } else if hash > 0.76 && !pal.has_extra(MaterialPalette::SLOT_PAVING) {
                PlannedBlock::simple("minecraft:mossy_cobblestone")
            } else if pal.has_extra(MaterialPalette::SLOT_PAVING) || hash > 0.48 || hash < 0.18 {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, x, y, z))
            } else {
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, x, y, z))
            }
        }
    }
}

fn sanitize_sub_description(desc: &str) -> String {
    let strip_roots = [
        "квартал",
        "деревн",
        "поселен",
        "посёлок",
        "поселок",
        "усадьб",
        "городок",
        "village",
        "town",
        "hamlet",
        "settlement",
        "district",
        "compound",
    ];
    let filtered: Vec<&str> = desc
        .split_whitespace()
        .filter(|w| {
            let low = w.to_lowercase();
            !strip_roots.iter().any(|r| low.contains(r))
        })
        .collect();
    filtered.join(" ")
}

/// Generates a cohesive multi-building village / town district / estate (`<= 96x96`)
/// with paved street axes, central fountain/well plaza, streetlights, shops/houses, and perimeter walling.
pub fn generate_settlement(
    req: &BuildRequest,
    spec: BlueprintSpec,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
    water_depths: Option<&[i32]>,
) -> GeneratedBlueprint {
    let w = i32::from(req.width.clamp(7, 96));
    let d = i32::from(req.depth.clamp(7, 96));
    let height = req.height.clamp(5, 48);
    let pal = &req.palette;
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0x5E77_1E00_9911_2233);
    let mut blocks: BTreeMap<[i32; 3], PlannedBlock> = BTreeMap::new();

    let cx = w / 2;
    let cz = d / 2;
    let is_floating = spec.theme == crate::parser::StructureTheme::FloatingVillage;
    let town_mode = if is_floating {
        4 // 4 = Floating Stilt Canal City / Venetian Water Quarter
    } else if w >= 32 && d >= 32 {
        rng.range_i32(0, 3) // 0=fountain crossroads, 1=canal + arched stone bridge, 2=landmark clocktower/arch plaza, 3=bazaar boulevard
    } else {
        0
    };
    let plaza_r = if w >= 52 && d >= 52 { 4 } else if w >= 29 && d >= 29 { 3 } else { 2 };
    let road_half = if w >= 48 && d >= 48 { 2 } else { 1 };

    let mut is_road = vec![false; (w * d) as usize];
    let mut is_plaza = vec![false; (w * d) as usize];
    let mut is_canal = vec![false; (w * d) as usize];
    for lz in 1..(d - 1) {
        for lx in 1..(w - 1) {
            let on_main_x = (lx - cx).abs() <= road_half;
            let on_main_z = (lz - cz).abs() <= road_half;
            let in_plaza = (lx - cx).abs() <= plaza_r && (lz - cz).abs() <= plaza_r;
            if is_floating && (lz - cz).abs() > road_half && (lx - cx).abs() > road_half {
                is_canal[(lz * w + lx) as usize] = true;
            } else if town_mode == 1 && (lz - cz).abs() <= 1 && !on_main_x {
                is_canal[(lz * w + lx) as usize] = true;
            } else if in_plaza {
                is_plaza[(lz * w + lx) as usize] = true;
                is_road[(lz * w + lx) as usize] = true;
            } else if on_main_x || on_main_z {
                is_road[(lz * w + lx) as usize] = true;
            }
        }
    }

    // Grade roads, canals & central plaza smoothly to surrounding terrain.
    for lz in 0..d {
        for lx in 0..w {
            let idx = (lz * w + lx) as usize;
            if is_canal[idx] {
                let bed_y = if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-4, 2) - 1
                } else {
                    -1
                };
                blocks.insert([lx, bed_y, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, bed_y, lz)));
                blocks.insert([lx, bed_y + 1, lz], PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                for cy in (bed_y + 2)..=(bed_y + 5) {
                    blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
                }
                continue;
            }
            if !is_road[idx] {
                continue;
            }
            let surf_y = if req.adapt_terrain {
                terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-4, 3)
            } else {
                0
            };
            let wd = terrain_at_wxz(water_depths, w, lx, lz);
            let h = noise.cell_hash(lx, 0, lz);
            let is_border = (lx - cx).abs() == road_half || (lz - cz).abs() == road_half;
            let road_y = if is_floating {
                1
            } else if wd > 0 {
                1
            } else {
                surf_y
            };
            if is_floating {
                let bed_y = (surf_y - wd).clamp(-10, 0) - 1;
                blocks.insert(
                    [lx, bed_y, lz],
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, bed_y, lz)),
                );
                if (lx - cx).abs() % 3 == 0 && (lz - cz).abs() % 3 == 0 {
                    for fill_y in (bed_y + 1)..road_y {
                        blocks.insert(
                            [lx, fill_y, lz],
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PILLAR, seed, lx, fill_y, lz)),
                        );
                    }
                } else {
                    blocks.insert(
                        [lx, 0, lz],
                        PlannedBlock::with_props("minecraft:water", vec![("level", "0")]),
                    );
                }
                let pier_slot = if is_border {
                    MaterialPalette::SLOT_WALL_TRIM
                } else {
                    MaterialPalette::SLOT_FLOOR
                };
                blocks.insert(
                    [lx, road_y, lz],
                    PlannedBlock::simple(pal.pick(pier_slot, seed, lx, road_y, lz)),
                );
            } else {
                if wd > 0 || road_y > surf_y {
                    for fill_y in (surf_y - wd).clamp(-10, road_y - 1)..road_y {
                        blocks.insert(
                            [lx, fill_y, lz],
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fill_y, lz)),
                        );
                    }
                }
                blocks.insert([lx, road_y, lz], pave_block_for_style(spec.style, pal, seed, lx, road_y, lz, h, is_border));
            }
            for cy in (road_y + 1)..=(road_y + 5) {
                blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Central Town Plaza Monument: Canal Stone Bridge (mode 1), Monumental Pillar (mode 2), Market Kiosk (mode 3), Floating Pier Plaza (mode 4), or Fountain/Well!
    let plaza_y = if is_floating {
        1
    } else if req.adapt_terrain {
        terrain_at_wxz(terrain_offsets, w, cx, cz).clamp(-3, 2)
    } else {
        0
    };
    if town_mode == 1 {
        // Canal embankment kerbs + central arched stone footbridge at `(cx, cz)`.
        for lx in 1..(w - 1) {
            if (lx - cx).abs() <= road_half + 1 {
                continue;
            }
            for &bank_z in &[cz - 2, cz + 2] {
                blocks.insert([lx, plaza_y + 1, bank_z], PlannedBlock::simple(pal.trim_wall.clone()));
            }
        }
        let bw = ((road_half * 2) + 3).clamp(5, 9) as u16;
        let bridge_req = BuildRequest {
            dimension: req.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: bw,
            depth: 9,
            height: height.clamp(8, 12),
            category: BuildCategory::Bridge,
            style: spec.style,
            facing: CardinalFacing::South,
            noise_level: req.noise_level,
            seed: (req.seed as u64 ^ 0xB41D_6E00_1122).cast_signed(),
            description: "арочный каменный мост через канал".into(),
            adapt_terrain: req.adapt_terrain,
            palette: pal.clone(),
        };
        let bridge_bp = crate::generator::generate_blueprint_with_water(&bridge_req, None, None);
        let bx0 = cx - i32::from(bw) / 2;
        let bz0 = cz - 4;
        for ([bx, by, bz], blk) in bridge_bp.blocks {
            let wx = bx0 + bx;
            let wz = bz0 + bz;
            if (0..w).contains(&wx) && (0..d).contains(&wz) {
                blocks.insert([wx, plaza_y + by - 2, wz], blk);
            }
        }
    } else if town_mode == 2 {
        // Monumental Central Clock / Obelisk Statue Spire towering over plaza!
        let tower_h = height.clamp(11, 18);
        let mon_desc = if spec.style == BuildStyle::Classical || spec.style == BuildStyle::Desert {
            "монументальный обелиск с аркой и золотом"
        } else {
            "часовая башня с курантами и колоколом"
        };
        let mon_req = BuildRequest {
            dimension: req.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: 9,
            depth: 9,
            height: tower_h,
            category: BuildCategory::Tower,
            style: spec.style,
            facing: CardinalFacing::South,
            noise_level: req.noise_level,
            seed: (req.seed as u64 ^ 0xC10C_4700_0001).cast_signed(),
            description: mon_desc.into(),
            adapt_terrain: false,
            palette: pal.clone(),
        };
        let mon_bp = crate::generator::generate_blueprint_with_water(&mon_req, None, None);
        let mx0 = cx - 4;
        let mz0 = cz - 4;
        for ([mx, my, mz], blk) in mon_bp.blocks {
            if my >= 0 {
                let wx = mx0 + mx;
                let wz = mz0 + mz;
                if (0..w).contains(&wx) && (0..d).contains(&wz) {
                    blocks.insert([wx, plaza_y + my, wz], blk);
                }
            }
        }
    } else if town_mode == 3 {
        // Open-air Festive Market Stalls framing the central avenue!
        let tent_colors = ["minecraft:red_wool", "minecraft:yellow_wool", "minecraft:blue_wool", "minecraft:white_wool"];
        for (idx, &(sx, sz)) in [
            (cx - plaza_r + 1, cz - plaza_r + 1),
            (cx + plaza_r - 1, cz - plaza_r + 1),
            (cx - plaza_r + 1, cz + plaza_r - 1),
            (cx + plaza_r - 1, cz + plaza_r - 1),
        ]
        .iter()
        .enumerate()
        {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let px = sx + dx;
                    let pz = sz + dz;
                    if (0..w).contains(&px) && (0..d).contains(&pz) {
                        if dx.abs() == 1 && dz.abs() == 1 {
                            blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple(pal.fence.clone()));
                            blocks.insert([px, plaza_y + 2, pz], PlannedBlock::simple(pal.fence.clone()));
                        }
                        blocks.insert(
                            [px, plaza_y + 3, pz],
                            PlannedBlock::simple(if (px + pz) % 2 == 0 {
                                tent_colors[idx % tent_colors.len()]
                            } else {
                                "minecraft:white_wool"
                            }),
                        );
                    }
                }
            }
            blocks.insert([sx, plaza_y + 1, sz], PlannedBlock::simple("minecraft:barrel"));
        }
    } else if plaza_r >= 3 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let px = cx + dx;
                let pz = cz + dz;
                let max_d = dx.abs().max(dz.abs());
                blocks.insert([px, plaza_y, pz], PlannedBlock::simple(pal.foundation.clone()));
                if max_d == 2 {
                    if dx.abs() == 2 && dz.abs() == 2 {
                        blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple(pal.foundation.clone()));
                        blocks.insert([px, plaza_y + 2, pz], PlannedBlock::simple(pal.light.clone()));
                    } else {
                        blocks.insert([px, plaza_y + 1, pz], make_slab(&pal.roof_slab, "bottom"));
                    }
                } else if dx == 0 && dz == 0 {
                    blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple(pal.foundation.clone()));
                    blocks.insert([px, plaza_y + 2, pz], PlannedBlock::simple(pal.trim_wall.clone()));
                    blocks.insert([px, plaza_y + 3, pz], PlannedBlock::simple(pal.light.clone()));
                } else {
                    blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple("minecraft:water"));
                }
            }
        }
    } else {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let px = cx + dx;
                let pz = cz + dz;
                if dx == 0 && dz == 0 {
                    blocks.insert([px, plaza_y, pz], PlannedBlock::simple("minecraft:water"));
                    blocks.insert([px, plaza_y + 3, pz], PlannedBlock::simple(pal.roof_block.clone()));
                } else if dx.abs() == 1 && dz.abs() == 1 {
                    blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple(pal.trim_wall.clone()));
                    blocks.insert([px, plaza_y + 2, pz], PlannedBlock::simple(pal.fence.clone()));
                    blocks.insert([px, plaza_y + 3, pz], make_slab(&pal.roof_slab, "bottom"));
                } else {
                    blocks.insert([px, plaza_y + 1, pz], PlannedBlock::simple(pal.foundation.clone()));
                }
            }
        }
    }

    // Build varied quadrant sub-lots facing the main avenues according to seed.
    let gap = road_half + 2;
    let margin = if w >= 32 && d >= 32 { 2 } else { 1 };
    let nw_w = (cx - gap - margin).max(0);
    let ne_w = ((w - margin) - (cx + gap)).max(0);
    let n_d = (cz - gap - margin).max(0);
    let s_d = ((d - margin) - (cz + gap)).max(0);

    let lot_roll = rng.range_i32(0, 2);
    let lot_specs = match lot_roll {
        1 => [
            (margin, margin, nw_w, n_d, CardinalFacing::South, BuildCategory::Tower, "ветряная мельница с лопастями"),
            (cx + gap, margin, ne_w, n_d, CardinalFacing::South, BuildCategory::Pavilion, "стеклянная теплица с цветами"),
            (margin, cz + gap, nw_w, s_d, CardinalFacing::North, BuildCategory::House, "таверна с крыльцом и садом"),
            (cx + gap, cz + gap, ne_w, s_d, CardinalFacing::North, BuildCategory::House, "двухэтажный особняк с мансардной крышей"),
        ],
        2 => [
            (margin, margin, nw_w, n_d, CardinalFacing::South, BuildCategory::Temple, "соборный храм с колоколом и витражами"),
            (cx + gap, margin, ne_w, n_d, CardinalFacing::South, BuildCategory::House, "библиотека архив с балконом"),
            (margin, cz + gap, nw_w, s_d, CardinalFacing::North, BuildCategory::Forge, "водяная мельница мастерская с колесом"),
            (cx + gap, cz + gap, ne_w, s_d, CardinalFacing::North, BuildCategory::Pavilion, "восточный базар ярмарка с навесами"),
        ],
        _ => [
            (margin, margin, nw_w, n_d, CardinalFacing::South, BuildCategory::House, "таверна с крыльцом и садом"),
            (cx + gap, margin, ne_w, n_d, CardinalFacing::South, BuildCategory::Forge, "кузница с горном и наковальней"),
            (margin, cz + gap, nw_w, s_d, CardinalFacing::North, BuildCategory::House, "уютный двухэтажный дом с балконом"),
            (
                cx + gap,
                cz + gap,
                ne_w,
                s_d,
                CardinalFacing::North,
                match spec.style {
                    BuildStyle::Gothic | BuildStyle::Steampunk | BuildStyle::Nordic => BuildCategory::Tower,
                    BuildStyle::Classical | BuildStyle::Desert => BuildCategory::Temple,
                    _ => BuildCategory::Barn,
                },
                match spec.style {
                    BuildStyle::Gothic | BuildStyle::Steampunk | BuildStyle::Nordic => "обсерватория с куполом и башней",
                    BuildStyle::Classical | BuildStyle::Desert => "малый храм колоннада",
                    _ => "фермерский амбар со стогами",
                },
            ),
        ],
    };

    let mut lots = Vec::new();
    for (x0, z0, raw_w, raw_d, facing, cat, desc) in lot_specs {
        if raw_w >= 9 && raw_d >= 9 {
            let cap_w = raw_w.min(21) as u16;
            let cap_d = raw_d.min(19) as u16;
            let off_x = (raw_w - i32::from(cap_w)) / 2;
            let off_z = (raw_d - i32::from(cap_d)) / 2;
            lots.push(LotPlan {
                x0: x0 + off_x,
                z0: z0 + off_z,
                w: cap_w,
                d: cap_d,
                facing,
                cat,
                desc,
            });
        }
    }

    // Guarantee at least one cozy cottage/house even on small plot tests (e.g., 19x19 or 13x11).
    if lots.is_empty() && w >= 11 && d >= 11 {
        let sw = ((w - 4).clamp(7, 13)) as u16;
        let sd = ((d - 4).clamp(7, 11)) as u16;
        lots.push(LotPlan {
            x0: (w - i32::from(sw)) / 2,
            z0: 1,
            w: sw,
            d: sd,
            facing: CardinalFacing::South,
            cat: BuildCategory::House,
            desc: "уютный деревенский дом с крыльцом",
        });
    }

    for (li, lot) in lots.iter().enumerate() {
        let lw = i32::from(lot.w);
        let ld = i32::from(lot.d);
        let door_edge_x = lot.x0 + lw / 2;
        let door_edge_z = match lot.facing {
            CardinalFacing::South => lot.z0 + ld - 1,
            CardinalFacing::North => lot.z0,
            _ => lot.z0 + ld / 2,
        };
        let lot_base_y = if is_floating {
            0
        } else if req.adapt_terrain {
            terrain_at_wxz(terrain_offsets, w, door_edge_x, door_edge_z).clamp(-4, 4)
        } else {
            0
        };

        let sub_off = slice_map_2d(terrain_offsets, w, lot.x0, lot.z0, lw, ld, lot_base_y, true);
        let sub_wd = if is_floating {
            Some(vec![1i32; (lw * ld) as usize])
        } else {
            slice_map_2d(water_depths, w, lot.x0, lot.z0, lw, ld, lot_base_y, false)
        };

        let sub_seed = (req.seed as u64) ^ ((li as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let clean_extra = sanitize_sub_description(&req.description);
        let full_desc = if is_floating {
            if clean_extra.is_empty() {
                format!("{}, на сваях на воде", lot.desc)
            } else {
                format!("{}, на сваях на воде, {}", lot.desc, clean_extra)
            }
        } else if clean_extra.is_empty() {
            lot.desc.to_owned()
        } else {
            format!("{}, {}", lot.desc, clean_extra)
        };
        let sub_req = BuildRequest {
            dimension: req.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: lot.w,
            depth: lot.d,
            height: height.clamp(10, 22),
            category: lot.cat,
            style: spec.style,
            facing: lot.facing,
            noise_level: req.noise_level,
            seed: sub_seed as i64,
            description: full_desc,
            adapt_terrain: req.adapt_terrain,
            palette: pal.clone(),
        };
        let sub_bp = crate::generator::generate_blueprint_with_water(
            &sub_req,
            sub_off.as_deref(),
            sub_wd.as_deref(),
        );

        for ([sx, sy, sz], blk) in sub_bp.blocks {
            let wx = lot.x0 + sx;
            let wy = lot_base_y + sy;
            let wz = lot.z0 + sz;
            if is_floating && wy <= 0 && blk.block_id == "minecraft:air" {
                continue;
            }
            if (0..w).contains(&wx) && (0..d).contains(&wz) {
                blocks.insert([wx, wy, wz], blk);
            }
        }

        // Link house entrance cleanly to main street with cobblestone/dirt path (or timber pier walkway on water).
        if let Some([dlx, _, dlz]) = sub_bp.door_pos {
            let dx = lot.x0 + dlx;
            let dz = lot.z0 + dlz;
            let (step_z, lim_z) = if dz < cz { (1, cz) } else { (-1, cz) };
            let mut cur_z = dz + step_z;
            while (0..d).contains(&cur_z) && cur_z != lim_z && !is_road[(cur_z * w + dx) as usize] {
                let sy = if is_floating {
                    1
                } else if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, dx, cur_z).clamp(-4, 3)
                } else {
                    0
                };
                if blocks
                    .get(&[dx, sy + 1, cur_z])
                    .is_none_or(|b| b.block_id == "minecraft:air")
                {
                    if is_floating {
                        if cur_z % 2 == 0 {
                            blocks.insert([dx, 0, cur_z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PILLAR, seed, dx, 0, cur_z)));
                        }
                        blocks.insert([dx, sy, cur_z], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, dx, sy, cur_z)));
                    } else {
                        blocks.insert(
                            [dx, sy, cur_z],
                            pave_block_for_style(spec.style, pal, seed, dx, sy, cur_z, rng.next_f64(), false),
                        );
                    }
                }
                cur_z += step_z;
            }
        }
    }

    // Streetlights flanking main roads every 8 blocks.
    for dist in (plaza_r + 3..cx.max(cz)).step_by(8) {
        let spots = [
            (cx - road_half - 1, cz - dist),
            (cx + road_half + 1, cz + dist),
            (cx - dist, cz + road_half + 1),
            (cx + dist, cz - road_half - 1),
        ];
        for (lx, lz) in spots {
            if !(2..(w - 2)).contains(&lx) || !(2..(d - 2)).contains(&lz) {
                continue;
            }
            if is_plaza[(lz * w + lx) as usize] {
                continue;
            }
            if (1..=4).any(|y| blocks.get(&[lx, y, lz]).is_some_and(|b| b.block_id != "minecraft:air")) {
                continue;
            }
            let base_y = if is_floating {
                1
            } else if req.adapt_terrain {
                terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-4, 3)
            } else {
                0
            };
            blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 1, lz)));
            blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 2, lz)));
            blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 3, lz)));
            blocks.insert([lx, base_y + 4, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 4, lz)));
        }
    }

    // Perimeter Village Fence / Crenellated Town Wall with Gate Piers at road exits.
    if !is_floating && w >= 25 && d >= 25 {
        let stone_town_wall = matches!(
            spec.style,
            BuildStyle::Gothic | BuildStyle::Classical | BuildStyle::Steampunk
        );
        for lz in 0..d {
            for lx in 0..w {
                let on_rim = lx == 0 || lx == w - 1 || lz == 0 || lz == d - 1;
                if !on_rim {
                    continue;
                }
                let in_gate = (lx - cx).abs() <= road_half + 1 || (lz - cz).abs() <= road_half + 1;
                let is_gate_pier = ((lx - cx).abs() == road_half + 2 && (lz == 0 || lz == d - 1))
                    || ((lz - cz).abs() == road_half + 2 && (lx == 0 || lx == w - 1));
                if in_gate && !is_gate_pier {
                    continue;
                }
                let base_y = if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-4, 3)
                } else {
                    0
                };
                if terrain_at_wxz(water_depths, w, lx, lz) > 0 {
                    continue;
                }
                if is_gate_pier || (lx == 0 || lx == w - 1) && (lz == 0 || lz == d - 1) {
                    blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 1, lz)));
                    blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 2, lz)));
                    blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 3, lz)));
                } else if stone_town_wall {
                    blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 1, lz)));
                    blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 2, lz)));
                    if (lx + lz) % 2 == 0 {
                        blocks.insert([lx, base_y + 3, lz], make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, lx, base_y + 3, lz), "bottom"));
                    }
                } else if (lx + lz) % 3 == 0 {
                    blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 1, lz)));
                    blocks.insert([lx, base_y + 2, lz], make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, lx, base_y + 2, lz)));
                } else {
                    blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 1, lz)));
                }
            }
        }
    }

    prune_map_unsupported(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);

    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: w as u16,
        depth: d as u16,
        height,
        blocks,
        door_pos: Some([cx, plaza_y + 1, cz + plaza_r]),
    }
}
