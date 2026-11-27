use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, PlannedBlock, make_leaves, make_slab, make_stair,
    prune_map_unsupported, resolve_connections_and_shapes,
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
        "город",
        "руин",
        "заброш",
        "разруш",
        "village",
        "town",
        "hamlet",
        "settlement",
        "district",
        "compound",
        "ruin",
        "abandoned",
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

fn district_role_pool(style: BuildStyle) -> &'static [(BuildCategory, &'static str)] {
    match style {
        BuildStyle::Classical => &[
            (BuildCategory::House, "торжественная ратуша с колоннами"),
            (BuildCategory::Temple, "античный малый храм колоннада"),
            (BuildCategory::Pavilion, "оперный павильон с портиком"),
            (BuildCategory::House, "мраморный банк с арками"),
            (BuildCategory::House, "музей античная библиотека с балконом"),
            (BuildCategory::Pavilion, "базар торговая площадь с навесами"),
            (BuildCategory::Forge, "ремесленная мастерская с горном"),
            (BuildCategory::House, "особняк с садом и галереей"),
        ],
        BuildStyle::Japanese => &[
            (BuildCategory::House, "чайный дом с террасой энгава"),
            (BuildCategory::Temple, "святилище пагода с тории"),
            (BuildCategory::Pavilion, "горячий источник онсэн баня"),
            (BuildCategory::Tower, "многоярусная деревянная пагода"),
            (BuildCategory::Forge, "мастерская ремесленника кузница"),
            (BuildCategory::House, "традиционный дом самурая с садом"),
            (BuildCategory::Pavilion, "рынок фонарей с цветным навесом"),
            (BuildCategory::Barn, "деревянный склад с навесом"),
        ],
        BuildStyle::Desert => &[
            (BuildCategory::Pavilion, "восточный базар с цветными шатрами"),
            (BuildCategory::Temple, "купольный храм минарет"),
            (BuildCategory::House, "глинобитная касба с террасой"),
            (BuildCategory::Forge, "оазис мастерская с печью"),
            (BuildCategory::House, "торговый дом риад с садом"),
            (BuildCategory::Tower, "дозорная башня с арками"),
            (BuildCategory::House, "пекарня лавка с навесом"),
            (BuildCategory::Barn, "караван сарай склад"),
        ],
        _ => &[
            (BuildCategory::House, "таверна с крыльцом и камином"),
            (BuildCategory::Forge, "кузница с горном и наковальней"),
            (BuildCategory::House, "уютная пекарня лавка с навесом"),
            (BuildCategory::House, "аптека алхимика с оранжереей"),
            (BuildCategory::Temple, "городской соборный храм с колоколом"),
            (BuildCategory::Tower, "ветряная мельница с лопастями"),
            (BuildCategory::House, "двухэтажный особняк с мансардной крышей"),
            (BuildCategory::Pavilion, "стеклянная теплица оранжерея с цветами"),
            (BuildCategory::Barn, "фермерский амбар со стогами"),
            (BuildCategory::Forge, "водяная мельница мастерская с колесом"),
            (BuildCategory::Tower, "обсерватория с куполом и башней"),
        ],
    }
}

fn find_water_runs(
    water_depths: Option<&[i32]>,
    w: i32,
    d: i32,
    fixed_coord: i32,
    along_z: bool,
    min_len: i32,
) -> Vec<(i32, i32)> {
    let Some(wd) = water_depths else {
        return Vec::new();
    };
    let max_i = if along_z { d } else { w };
    let mut out = Vec::new();
    let mut run_start: Option<i32> = None;
    for i in 2..(max_i - 2) {
        let (x, z) = if along_z { (fixed_coord, i) } else { (i, fixed_coord) };
        let depth = wd.get((z * w + x) as usize).copied().unwrap_or(0);
        if depth > 0 {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(s) = run_start.take()
            && i - s >= min_len
        {
            out.push((s, i - 1));
        }
    }
    if let Some(s) = run_start
        && (max_i - 2) - s >= min_len
    {
        out.push((s, max_i - 3));
    }
    out
}

fn build_town_tree(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    pal: &MaterialPalette,
    biome_tag: &str,
    seed: u64,
    tx: i32,
    ty: i32,
    tz: i32,
    tall: bool,
) {
    let (trunk, leaves) = if biome_tag.contains("cherry") {
        ("minecraft:cherry_log".to_owned(), "minecraft:cherry_leaves".to_owned())
    } else if biome_tag.contains("taiga") || biome_tag.contains("snow") || biome_tag.contains("frozen") {
        ("minecraft:spruce_log".to_owned(), "minecraft:spruce_leaves".to_owned())
    } else if biome_tag.contains("birch") {
        ("minecraft:birch_log".to_owned(), "minecraft:birch_leaves".to_owned())
    } else if biome_tag.contains("desert") || biome_tag.contains("savanna") || biome_tag.contains("badlands") {
        ("minecraft:acacia_log".to_owned(), "minecraft:acacia_leaves".to_owned())
    } else if biome_tag.contains("dark_forest") || biome_tag.contains("pale_garden") {
        ("minecraft:dark_oak_log".to_owned(), "minecraft:dark_oak_leaves".to_owned())
    } else {
        (
            pal.pick(MaterialPalette::SLOT_FRAME, seed, tx, ty, tz),
            pal.pick(MaterialPalette::SLOT_LEAVES, seed, tx, ty, tz),
        )
    };

    blocks.insert([tx, ty + 1, tz], PlannedBlock::simple(trunk.clone()));
    blocks.insert([tx, ty + 2, tz], PlannedBlock::simple(trunk.clone()));
    if tall {
        blocks.insert([tx, ty + 3, tz], PlannedBlock::simple(trunk));
    }
    let crown_base = if tall { ty + 3 } else { ty + 2 };
    for dy in 0..=2 {
        let rad: i32 = if dy == 2 { 0 } else if dy == 1 { 1 } else { 2 };
        for dz in -rad..=rad {
            for dx in -rad..=rad {
                if rad == 2 && dx.abs() == 2 && dz.abs() == 2 {
                    continue;
                }
                let pos = [tx + dx, crown_base + dy, tz + dz];
                if blocks.get(&pos).is_none_or(|b| b.block_id == "minecraft:air") {
                    blocks.insert(pos, make_leaves(&leaves));
                }
            }
        }
    }
}

/// Generates a multi-district town / walled city / village (`<= 96x96`) with secondary ring avenues,
/// smooth sloped streets, automatic bridges over natural rivers, corner bastions & gatehouses,
/// up to 8 oriented building lots, pocket gardens, and lakeside mooring piers.
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
    let lower_desc = req.description.to_lowercase();
    let wants_plaza_fountain = lower_desc.contains("площад")
        || lower_desc.contains("фонтан")
        || lower_desc.contains("fountain");
    let town_mode = if is_floating {
        4 // Floating Stilt Canal City
    } else if wants_plaza_fountain
        && !lower_desc.contains("канал")
        && !lower_desc.contains("рынок")
        && !lower_desc.contains("базар")
    {
        0
    } else if w >= 32 && d >= 32 {
        rng.range_i32(0, 3) // 0=fountain crossroads, 1=canal+bridge, 2=monument/obelisk/clocktower, 3=bazaar plaza
    } else {
        0
    };
    let fortified_walls = !is_floating
        && w >= 36
        && d >= 36
        && (matches!(
            spec.style,
            BuildStyle::Medieval
                | BuildStyle::Gothic
                | BuildStyle::Classical
                | BuildStyle::Steampunk
                | BuildStyle::Desert
                | BuildStyle::Japanese
        ) || lower_desc.contains("стена")
            || lower_desc.contains("крепост")
            || lower_desc.contains("walled")
            || lower_desc.contains("fortified")
            || lower_desc.contains("город"));

    let plaza_r = if w >= 52 && d >= 52 {
        4
    } else if w >= 29 && d >= 29 {
        3
    } else {
        2
    };
    let road_half = if w >= 48 && d >= 48 { 2 } else { 1 };
    let wall_margin = if fortified_walls { 3 } else if w >= 32 && d >= 32 { 2 } else { 1 };

    // Secondary ring/district avenues on larger settlements (`>= 56x56`).
    let ring_xs: Vec<i32> = if !is_floating && w >= 60 {
        vec![(wall_margin + cx - plaza_r) / 2, (cx + plaza_r + w - wall_margin) / 2]
    } else {
        Vec::new()
    };
    let ring_zs: Vec<i32> = if !is_floating && d >= 60 {
        vec![(wall_margin + cz - plaza_r) / 2, (cz + plaza_r + d - wall_margin) / 2]
    } else {
        Vec::new()
    };

    let mut is_road = vec![false; (w * d) as usize];
    let mut is_plaza = vec![false; (w * d) as usize];
    let mut is_canal = vec![false; (w * d) as usize];
    let mut is_nat_bridge_road = vec![false; (w * d) as usize];

    for lz in 1..(d - 1) {
        for lx in 1..(w - 1) {
            let on_main_x = (lx - cx).abs() <= road_half;
            let on_main_z = (lz - cz).abs() <= road_half;
            let on_sec_x = ring_xs.iter().any(|&rx| (lx - rx).abs() <= 1);
            let on_sec_z = ring_zs.iter().any(|&rz| (lz - rz).abs() <= 1);
            let in_plaza = (lx - cx).abs() <= plaza_r && (lz - cz).abs() <= plaza_r;
            if is_floating && (lz - cz).abs() > road_half && (lx - cx).abs() > road_half {
                is_canal[(lz * w + lx) as usize] = true;
            } else if town_mode == 1 && (lz - cz).abs() <= 1 && !on_main_x {
                is_canal[(lz * w + lx) as usize] = true;
            } else if in_plaza {
                is_plaza[(lz * w + lx) as usize] = true;
                is_road[(lz * w + lx) as usize] = true;
            } else if on_main_x || on_main_z || on_sec_x || on_sec_z {
                is_road[(lz * w + lx) as usize] = true;
            }
        }
    }

    // Detect contiguous natural water crossings along main road axes so bridges span rivers instead of burying them.
    let mut water_bridges: Vec<(bool, i32, i32, i32)> = Vec::new(); // (along_z, fixed_coord, start, end)
    if !is_floating && town_mode != 1 {
        for (s, e) in find_water_runs(water_depths, w, d, cx, true, 3) {
            for z in s..=e {
                for dx in -road_half..=road_half {
                    let x = cx + dx;
                    if (1..(w - 1)).contains(&x) && (z - cz).abs() > plaza_r {
                        is_nat_bridge_road[(z * w + x) as usize] = true;
                    }
                }
            }
            if (s - cz).abs() > plaza_r && (e - cz).abs() > plaza_r {
                water_bridges.push((true, cx, s, e));
            }
        }
        for (s, e) in find_water_runs(water_depths, w, d, cz, false, 3) {
            for x in s..=e {
                for dz in -road_half..=road_half {
                    let z = cz + dz;
                    if (1..(d - 1)).contains(&z) && (x - cx).abs() > plaza_r {
                        is_nat_bridge_road[(z * w + x) as usize] = true;
                    }
                }
            }
            if (s - cx).abs() > plaza_r && (e - cx).abs() > plaza_r {
                water_bridges.push((false, cz, s, e));
            }
        }
    }

    // Anchor plaza center elevation cleanly up front so canal, streets, and gatehouses align vertically.
    let plaza_y = if is_floating {
        1
    } else if req.adapt_terrain {
        terrain_at_wxz(terrain_offsets, w, cx, cz).clamp(-12, 12)
    } else {
        0
    };

    // Compute and relax smooth road heights so slopes never step sharply (>1 vertical block per step).
    let mut road_y_grid = vec![0i32; (w * d) as usize];
    for lz in 0..d {
        for lx in 0..w {
            let idx = (lz * w + lx) as usize;
            if is_floating {
                road_y_grid[idx] = 1;
            } else if is_road[idx] {
                let wd = terrain_at_wxz(water_depths, w, lx, lz);
                let sy = if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-12, 12)
                } else {
                    0
                };
                road_y_grid[idx] = if wd > 0 { 1 } else { sy };
            }
        }
    }
    if !is_floating && req.adapt_terrain {
        for _pass in 0..6 {
            let prev = road_y_grid.clone();
            for lz in 1..(d - 1) {
                for lx in 1..(w - 1) {
                    let idx = (lz * w + lx) as usize;
                    if !is_road[idx] || is_plaza[idx] || is_nat_bridge_road[idx] {
                        continue;
                    }
                    let mut sum = prev[idx] * 2;
                    let mut cnt = 2;
                    for (nx, nz) in [(lx + 1, lz), (lx - 1, lz), (lx, lz + 1), (lx, lz - 1)] {
                        let nidx = (nz * w + nx) as usize;
                        if is_road[nidx] {
                            sum += prev[nidx];
                            cnt += 1;
                        }
                    }
                    let avg = (sum as f64 / cnt as f64).round() as i32;
                    road_y_grid[idx] = avg.clamp(prev[idx] - 1, prev[idx] + 1);
                }
            }
        }
    }

    // Shared smoothed canal water surface along X so canal water & banks remain flat or step cleanly.
    let mut canal_water_y = vec![plaza_y; w as usize];
    if !is_floating && town_mode == 1 && req.adapt_terrain {
        for lx in 0..w {
            if (lx - cx).abs() <= road_half + 1 {
                canal_water_y[lx as usize] = plaza_y;
            } else {
                canal_water_y[lx as usize] = terrain_at_wxz(terrain_offsets, w, lx, cz).clamp(-12, 12);
            }
        }
        for _pass in 0..5 {
            let prev = canal_water_y.clone();
            for lx in 1..(w - 1) {
                if (lx - cx).abs() <= road_half + 1 {
                    canal_water_y[lx as usize] = plaza_y;
                    continue;
                }
                let avg = ((prev[(lx - 1) as usize] + prev[lx as usize] * 2 + prev[(lx + 1) as usize]) as f64 / 4.0)
                    .round() as i32;
                canal_water_y[lx as usize] = avg.clamp(prev[lx as usize] - 1, prev[lx as usize] + 1);
            }
        }
        // Keep step transitions spaced at least 2 blocks apart and bounded to <= 1 block per column.
        for lx in (cx + road_half + 2)..w {
            let prev_y = canal_water_y[(lx - 1) as usize];
            canal_water_y[lx as usize] = canal_water_y[lx as usize].clamp(prev_y - 1, prev_y + 1);
        }
        for lx in (0..(cx - road_half - 1)).rev() {
            let next_y = canal_water_y[(lx + 1) as usize];
            canal_water_y[lx as usize] = canal_water_y[lx as usize].clamp(next_y - 1, next_y + 1);
        }
    }

    // Grade roads, canals & central plaza smoothly to surrounding terrain.
    for lz in 0..d {
        for lx in 0..w {
            let idx = (lz * w + lx) as usize;
            if is_canal[idx] {
                let surf_y = if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-14, 14)
                } else {
                    0
                };
                let wd = terrain_at_wxz(water_depths, w, lx, lz);
                let water_y = if is_floating {
                    0
                } else if req.adapt_terrain {
                    canal_water_y[lx as usize]
                } else {
                    0
                };
                let bed_y = water_y - 1;
                let lo_y = (surf_y - wd - 2).clamp(-16, bed_y);
                for fy in lo_y..=bed_y {
                    blocks.insert(
                        [lx, fy, lz],
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, lz)),
                    );
                }
                blocks.insert([lx, water_y, lz], PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
                for cy in (water_y + 1)..=(surf_y.max(water_y) + 5) {
                    blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
                }
                continue;
            }
            if !is_road[idx] || is_nat_bridge_road[idx] {
                continue;
            }
            let surf_y = if req.adapt_terrain {
                terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-14, 14)
            } else {
                0
            };
            let wd = terrain_at_wxz(water_depths, w, lx, lz);
            let h = noise.cell_hash(lx, 0, lz);
            let is_border = (lx - cx).abs() == road_half
                || (lz - cz).abs() == road_half
                || ring_xs.iter().any(|&rx| (lx - rx).abs() == 1)
                || ring_zs.iter().any(|&rz| (lz - rz).abs() == 1);
            let road_y = road_y_grid[idx];

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
                    let lo_y = (surf_y - wd - 1).clamp(-16, (road_y - 1).max(-16));
                    for fill_y in lo_y..road_y {
                        blocks.insert(
                            [lx, fill_y, lz],
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fill_y, lz)),
                        );
                    }
                }
                blocks.insert(
                    [lx, road_y, lz],
                    pave_block_for_style(spec.style, pal, seed, lx, road_y, lz, h, is_border),
                );
            }
            for cy in (road_y + 1)..=(surf_y.max(road_y) + 5) {
                blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
            }
        }
    }

    // Place arched bridges across natural water channels intersected by avenues!
    for (bi, &(along_z, fixed_coord, s, e)) in water_bridges.iter().enumerate() {
        let span_len = ((e - s + 5) as u16).clamp(9, 32);
        let bw = ((road_half * 2) + 3).clamp(7, 11) as u16;
        let bridge_desc = match spec.style {
            BuildStyle::Japanese => "японский горбатый лунный мост",
            BuildStyle::Cottage | BuildStyle::Nordic => "деревянный крытый мост",
            _ => "арочный каменный мост",
        };
        let (rw, rd, facing) = if along_z {
            (bw, span_len, CardinalFacing::South)
        } else {
            (span_len, bw, CardinalFacing::East)
        };
        let mid = (s + e) / 2;
        let bx0 = if along_z {
            fixed_coord - i32::from(rw) / 2
        } else {
            mid - i32::from(rw) / 2
        };
        let bz0 = if along_z {
            mid - i32::from(rd) / 2
        } else {
            fixed_coord - i32::from(rd) / 2
        };
        let bank_y_a = if along_z {
            let z_edge = (bz0 - 1).clamp(1, d - 2);
            road_y_grid[(z_edge * w + fixed_coord.clamp(1, w - 2)) as usize]
        } else {
            let x_edge = (bx0 - 1).clamp(1, w - 2);
            road_y_grid[(fixed_coord.clamp(1, d - 2) * w + x_edge) as usize]
        };
        let bank_y_b = if along_z {
            let z_edge = (bz0 + i32::from(rd)).clamp(1, d - 2);
            road_y_grid[(z_edge * w + fixed_coord.clamp(1, w - 2)) as usize]
        } else {
            let x_edge = (bx0 + i32::from(rw)).clamp(1, w - 2);
            road_y_grid[(fixed_coord.clamp(1, d - 2) * w + x_edge) as usize]
        };
        let mut bridge_terrain = vec![0i32; usize::from(rw) * usize::from(rd)];
        for bz in 0..i32::from(rd) {
            for bx in 0..i32::from(rw) {
                let t = if along_z {
                    bz as f64 / i32::from(rd).saturating_sub(1).max(1) as f64
                } else {
                    bx as f64 / i32::from(rw).saturating_sub(1).max(1) as f64
                };
                bridge_terrain[(bz * i32::from(rw) + bx) as usize] =
                    (bank_y_a as f64 + t * (bank_y_b - bank_y_a) as f64).round() as i32;
            }
        }
        let bridge_req = BuildRequest {
            dimension: req.dimension.clone(),
            origin_x: 0,
            origin_y: 0,
            origin_z: 0,
            width: rw,
            depth: rd,
            height: height.clamp(10, 15),
            category: BuildCategory::Bridge,
            style: spec.style,
            facing,
            noise_level: req.noise_level,
            seed: (req.seed as u64 ^ (0xA91E_5500 + bi as u64)).cast_signed(),
            description: bridge_desc.into(),
            adapt_terrain: true,
            air_only: req.air_only,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: req.biome_hint.clone(),
            palette: pal.clone(),
        };
        let bbp = crate::generator::generate_blueprint_with_water(
            &bridge_req,
            Some(&bridge_terrain),
            None,
        );
        for ([bx, by, bz], blk) in bbp.blocks {
            if blk.block_id == "minecraft:air" && by <= 0 {
                continue;
            }
            let wx = bx0 + bx;
            let wz = bz0 + bz;
            if (1..(w - 1)).contains(&wx) && (1..(d - 1)).contains(&wz) {
                blocks.insert([wx, by, wz], blk);
            }
        }
    }

    // Central Town Plaza Monument: Canal Stone Bridge (mode 1), Monumental Pillar (mode 2), Market Kiosk (mode 3), or Fountain/Well!
    if !is_floating {
        for pz in (cz - plaza_r).max(1)..=(cz + plaza_r).min(d - 2) {
            for px in (cx - plaza_r).max(1)..=(cx + plaza_r).min(w - 2) {
                let surf = terrain_at_wxz(terrain_offsets, w, px, pz);
                let lo_y = (surf - 1).clamp(-16, plaza_y - 1);
                for fy in lo_y..plaza_y {
                    blocks.insert(
                        [px, fy, pz],
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, px, fy, pz)),
                    );
                }
                for cy in (plaza_y + 1)..=(surf.max(plaza_y) + 5) {
                    blocks.insert([px, cy, pz], PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }
    if town_mode == 1 {
        for lx in 1..(w - 1) {
            if (lx - cx).abs() <= road_half + 1 {
                continue;
            }
            let water_y = canal_water_y[lx as usize];
            for &bank_z in &[cz - 2, cz + 2] {
                let surf = terrain_at_wxz(terrain_offsets, w, lx, bank_z);
                let bot = (surf - 2).clamp(-16, water_y);
                for fy in bot..=water_y {
                    blocks.insert(
                        [lx, fy, bank_z],
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, bank_z)),
                    );
                }
                blocks.insert([lx, water_y + 1, bank_z], PlannedBlock::simple(pal.trim_wall.clone()));
                for cy in (water_y + 2)..=(surf.max(water_y) + 5) {
                    blocks.insert([lx, cy, bank_z], PlannedBlock::simple("minecraft:air"));
                }
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
            height: height.clamp(9, 13),
            category: BuildCategory::Bridge,
            style: spec.style,
            facing: CardinalFacing::South,
            noise_level: req.noise_level,
            seed: (req.seed as u64 ^ 0xB41D_6E00_1122).cast_signed(),
            description: "арочный каменный мост через канал".into(),
            adapt_terrain: false,
            air_only: req.air_only,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: req.biome_hint.clone(),
            palette: pal.clone(),
        };
        let bridge_bp = crate::generator::generate_blueprint_with_water(&bridge_req, None, None);
        let bx0 = cx - i32::from(bw) / 2;
        let bz0 = cz - 4;
        for ([bx, by, bz], blk) in bridge_bp.blocks {
            if blk.block_id == "minecraft:air" && by <= 0 {
                continue;
            }
            let wx = bx0 + bx;
            let wz = bz0 + bz;
            if (0..w).contains(&wx) && (0..d).contains(&wz) {
                blocks.insert([wx, plaza_y + by, wz], blk);
            }
        }
    } else if town_mode == 2 {
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
            air_only: false,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: req.biome_hint.clone(),
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
        let tent_colors = [
            "minecraft:red_wool",
            "minecraft:yellow_wool",
            "minecraft:blue_wool",
            "minecraft:white_wool",
        ];
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
                        blocks.insert(
                            [px, plaza_y, pz],
                            pave_block_for_style(spec.style, pal, seed, px, plaza_y, pz, noise.cell_hash(px, 0, pz), false),
                        );
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

    // Build X & Z zone bands bounded by perimeter wall margin, main streets, and secondary ring avenues.
    let gap = road_half + 2;
    let mut x_bands: Vec<(i32, i32, bool)> = Vec::new(); // (start, end_excl, face_east_preferred)
    if let Some((&lx_w, &lx_e)) = ring_xs.first().zip(ring_xs.get(1)) {
        x_bands.push((wall_margin, lx_w - 2, true));
        x_bands.push((lx_w + 3, cx - gap, true));
        x_bands.push((cx + gap, lx_e - 2, false));
        x_bands.push((lx_e + 3, w - wall_margin, false));
    } else {
        x_bands.push((wall_margin, cx - gap, true));
        x_bands.push((cx + gap, w - wall_margin, false));
    }
    let mut z_bands: Vec<(i32, i32, CardinalFacing)> = Vec::new();
    if let Some((&lz_n, &lz_s)) = ring_zs.first().zip(ring_zs.get(1)) {
        z_bands.push((wall_margin, lz_n - 2, CardinalFacing::South));
        z_bands.push((lz_n + 3, cz - gap, CardinalFacing::South));
        z_bands.push((cz + gap, lz_s - 2, CardinalFacing::North));
        z_bands.push((lz_s + 3, d - wall_margin, CardinalFacing::North));
    } else {
        z_bands.push((wall_margin, cz - gap, CardinalFacing::South));
        z_bands.push((cz + gap, d - wall_margin, CardinalFacing::North));
    }

    let pool = district_role_pool(spec.style);
    let lot_offset = rng.range_i32(0, pool.len() as i32 - 1) as usize;
    let mut lots = Vec::new();
    let mut small_pockets: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut role_cursor = 0usize;

    for (zi, &(z0, z1, facing_z)) in z_bands.iter().enumerate() {
        let raw_d = z1 - z0;
        if raw_d < 5 {
            continue;
        }
        for (xi, &(x0, x1, _east)) in x_bands.iter().enumerate() {
            let raw_w = x1 - x0;
            if raw_w < 5 {
                continue;
            }
            let intentional_park = z_bands.len() >= 4
                && x_bands.len() >= 4
                && ((xi == 0 && zi == 1) || (xi == 3 && zi == 2));
            if !intentional_park && raw_w >= 9 && raw_d >= 9 && lots.len() < 12 {
                let cap_w = raw_w.min(21) as u16;
                let cap_d = raw_d.min(19) as u16;
                let off_x = (raw_w - i32::from(cap_w)) / 2;
                let off_z = (raw_d - i32::from(cap_d)) / 2;
                let (cat, desc) = pool[(lot_offset + role_cursor) % pool.len()];
                role_cursor += 1;
                let facing = if (xi + zi) % 4 == 3 && raw_w >= 11 && raw_d >= 11 {
                    if x0 < cx { CardinalFacing::East } else { CardinalFacing::West }
                } else {
                    facing_z
                };
                lots.push(LotPlan {
                    x0: x0 + off_x,
                    z0: z0 + off_z,
                    w: cap_w,
                    d: cap_d,
                    facing,
                    cat,
                    desc,
                });
            } else {
                small_pockets.push((x0, z0, raw_w, raw_d));
            }
        }
    }

    // Guarantee at least one cozy house even on small plot tests (e.g., 19x19 or 13x11).
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
        let (door_edge_x, door_edge_z) = match lot.facing {
            CardinalFacing::South => (lot.x0 + lw / 2, lot.z0 + ld - 1),
            CardinalFacing::North => (lot.x0 + lw / 2, lot.z0),
            CardinalFacing::East => (lot.x0 + lw - 1, lot.z0 + ld / 2),
            CardinalFacing::West => (lot.x0, lot.z0 + ld / 2),
        };
        let lot_base_y = if is_floating {
            0
        } else if req.adapt_terrain {
            let samples = [
                (door_edge_x.clamp(0, w - 1), door_edge_z.clamp(0, d - 1)),
                ((lot.x0 + lw / 2).clamp(0, w - 1), (lot.z0 + ld / 2).clamp(0, d - 1)),
                (lot.x0.clamp(0, w - 1), lot.z0.clamp(0, d - 1)),
                ((lot.x0 + lw - 1).clamp(0, w - 1), (lot.z0 + ld - 1).clamp(0, d - 1)),
                (door_edge_x.clamp(0, w - 1), door_edge_z.clamp(0, d - 1)),
            ];
            let mut vals: Vec<i32> = samples
                .into_iter()
                .map(|(sx, sz)| terrain_at_wxz(terrain_offsets, w, sx, sz).clamp(-12, 12))
                .collect();
            vals.sort_unstable();
            vals[vals.len() / 2]
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
            air_only: req.air_only,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: req.biome_hint.clone(),
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

        // Link building entrance smoothly to nearest street along facing axis (including gentle terrain steps).
        if let Some([dlx, dl_y, dlz]) = sub_bp.door_pos {
            let dx = lot.x0 + dlx;
            let dz = lot.z0 + dlz;
            let (sx_step, sz_step) = match lot.facing {
                CardinalFacing::South => (0, 1),
                CardinalFacing::North => (0, -1),
                CardinalFacing::East => (1, 0),
                CardinalFacing::West => (-1, 0),
            };
            let mut prev_y = lot_base_y + dl_y - 1;
            let mut px = dx + sx_step;
            let mut pz = dz + sz_step;
            let mut steps = 0;
            while (1..(w - 1)).contains(&px)
                && (1..(d - 1)).contains(&pz)
                && !is_road[(pz * w + px) as usize]
                && steps < 18
            {
                let has_lot_stair_or_porch = (prev_y - 4..=prev_y + 2).any(|y| {
                    blocks.get(&[px, y, pz]).is_some_and(|b| {
                        b.block_id.ends_with("_stairs") || b.block_id.ends_with("_fence")
                    })
                });
                if has_lot_stair_or_porch {
                    if let Some(top_y) = (prev_y - 4..=prev_y + 2)
                        .rev()
                        .find(|&y| {
                            blocks.get(&[px, y, pz]).is_some_and(|b| {
                                b.block_id != "minecraft:air" && !b.block_id.ends_with("_fence")
                            })
                        })
                    {
                        prev_y = top_y;
                    }
                    px += sx_step;
                    pz += sz_step;
                    steps += 1;
                    continue;
                }
                let local_surf = if req.adapt_terrain {
                    terrain_at_wxz(terrain_offsets, w, px, pz).clamp(-14, 14)
                } else {
                    0
                };
                let target_y = if is_floating {
                    1
                } else if req.adapt_terrain {
                    local_surf.clamp(-12, 12)
                } else {
                    0
                };
                let sy = if target_y > prev_y {
                    prev_y + 1
                } else if target_y < prev_y {
                    prev_y - 1
                } else {
                    target_y
                };
                if blocks
                    .get(&[px, sy + 1, pz])
                    .is_none_or(|b| b.block_id == "minecraft:air" || !b.block_id.ends_with("_door"))
                {
                    if is_floating {
                        if steps % 2 == 0 {
                            blocks.insert([px, 0, pz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PILLAR, seed, px, 0, pz)));
                        }
                        blocks.insert([px, sy, pz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, px, sy, pz)));
                    } else {
                        let lo_y = (local_surf - 2).clamp(-16, (sy - 1).max(-16));
                        for fy in lo_y..sy {
                            blocks.insert([px, fy, pz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, px, fy, pz)));
                        }
                        blocks.insert(
                            [px, sy, pz],
                            pave_block_for_style(spec.style, pal, seed, px, sy, pz, rng.next_f64(), false),
                        );
                        for cy in (sy + 1)..=(local_surf.max(sy) + 5) {
                            blocks.insert([px, cy, pz], PlannedBlock::simple("minecraft:air"));
                        }
                    }
                }
                prev_y = sy;
                px += sx_step;
                pz += sz_step;
                steps += 1;
            }
        }
    }

    // Pocket green squares / parks, custom biome trees, hay bales & lakeside timber docks.
    let is_arid_biome = spec.style == BuildStyle::Desert
        || spec.biome_tag.contains("desert")
        || spec.biome_tag.contains("badlands");
    for (pi, &(px0, pz0, pw, pd)) in small_pockets.iter().enumerate() {
        let tx = px0 + pw / 2;
        let tz = pz0 + pd / 2;
        if !(2..(w - 2)).contains(&tx) || !(2..(d - 2)).contains(&tz) {
            continue;
        }
        let wd = terrain_at_wxz(water_depths, w, tx, tz);
        let sy = if req.adapt_terrain {
            terrain_at_wxz(terrain_offsets, w, tx, tz).clamp(-12, 12)
        } else {
            0
        };
        if wd > 0 {
            // Timber shoreline mooring dock over natural water pocket!
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let wx = tx + dx;
                    let wz = tz + dz;
                    if (2..(w - 2)).contains(&wx) && (2..(d - 2)).contains(&wz) {
                        if dx.abs() == 1 && dz.abs() == 1 {
                            for fy in (sy - wd).clamp(-16, 0)..=1 {
                                blocks.insert([wx, fy, wz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PILLAR, seed, wx, fy, wz)));
                            }
                            if dx == -1 && dz == -1 {
                                blocks.insert([wx, 2, wz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, wx, 2, wz)));
                                blocks.insert([wx, 3, wz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, wx, 3, wz)));
                            }
                        }
                        blocks.insert([wx, 1, wz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, wx, 1, wz)));
                        for cy in 2..=5 {
                            if blocks.get(&[wx, cy, wz]).is_none() {
                                blocks.insert([wx, cy, wz], PlannedBlock::simple("minecraft:air"));
                            }
                        }
                    }
                }
            }
        } else {
            // Clear overburden dunes/hills cleanly over the pocket park pad so trees, benches & fences never get buried.
            let pk_x0 = (px0 + 1).max(2);
            let pk_x1 = (px0 + pw - 2).min(w - 3);
            let pk_z0 = (pz0 + 1).max(2);
            let pk_z1 = (pz0 + pd - 2).min(d - 3);
            for pz in pk_z0..=pk_z1 {
                for px in pk_x0..=pk_x1 {
                    if is_road[(pz * w + px) as usize] || is_canal[(pz * w + px) as usize] {
                        continue;
                    }
                    let has_structure = (sy..=sy + 6).any(|y| {
                        blocks.get(&[px, y, pz]).is_some_and(|b| b.block_id != "minecraft:air")
                    });
                    if has_structure {
                        continue;
                    }
                    let cell_terr = if req.adapt_terrain {
                        terrain_at_wxz(terrain_offsets, w, px, pz).clamp(-14, 14)
                    } else {
                        0
                    };
                    let lo_y = (cell_terr - 1).clamp(-16, sy - 1);
                    for fy in lo_y..sy {
                        blocks.insert(
                            [px, fy, pz],
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, px, fy, pz)),
                        );
                    }
                    let pad_blk = if is_arid_biome {
                        pave_block_for_style(spec.style, pal, seed, px, sy, pz, noise.cell_hash(px, 1, pz), false)
                    } else {
                        PlannedBlock::simple(if (px + pz) % 3 == 0 { "minecraft:moss_block" } else { "minecraft:grass_block" })
                    };
                    blocks.insert([px, sy, pz], pad_blk);
                    for cy in (sy + 1)..=(cell_terr.max(sy) + 5) {
                        blocks.insert([px, cy, pz], PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
            if (1..=4).all(|dy| blocks.get(&[tx, sy + dy, tz]).is_none_or(|b| b.block_id == "minecraft:air")) {
                build_town_tree(&mut blocks, pal, &spec.biome_tag, seed, tx, sy, tz, pi % 2 == 0);
                if tx + 2 < px0 + pw {
                    blocks.insert(
                        [tx + 2, sy + 1, tz],
                        make_stair(
                            &pal.pick(MaterialPalette::SLOT_STAIR_BLOCK, seed, tx + 2, sy + 1, tz),
                            "west",
                            "bottom",
                        ),
                    );
                    blocks.insert([tx + 2, sy + 1, tz + 1], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, tx + 2, sy + 1, tz + 1)));
                    blocks.insert([tx + 2, sy + 2, tz + 1], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, tx + 2, sy + 2, tz + 1)));
                }
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
            if is_plaza[(lz * w + lx) as usize] || is_road[(lz * w + lx) as usize] {
                continue;
            }
            if (1..=4).any(|y| blocks.get(&[lx, y, lz]).is_some_and(|b| b.block_id != "minecraft:air")) {
                continue;
            }
            let surf_y = if req.adapt_terrain {
                terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-14, 14)
            } else {
                0
            };
            let base_y = if is_floating {
                1
            } else if req.adapt_terrain {
                surf_y.clamp(-12, 12)
            } else {
                0
            };
            if !is_floating {
                let lo_y = (surf_y - 1).clamp(-16, base_y);
                for fy in lo_y..=base_y {
                    blocks.insert([lx, fy, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, lz)));
                }
            }
            blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 1, lz)));
            blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 2, lz)));
            blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 3, lz)));
            blocks.insert([lx, base_y + 4, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 4, lz)));
        }
    }

    // Perimeter Fortified Town Ramparts, Corner Bastion Watchtowers & Arched Gatehouses (or Cozy Hedgerows).
    if !is_floating && w >= 25 && d >= 25 {
        let gate_axis_x = |lx: i32| {
            (lx - cx).abs() <= road_half + 1 || ring_xs.iter().any(|&rx| (lx - rx).abs() <= 1)
        };
        let gate_axis_z = |lz: i32| {
            (lz - cz).abs() <= road_half + 1 || ring_zs.iter().any(|&rz| (lz - rz).abs() <= 1)
        };
        let rim_base_at = |lx: i32, lz: i32| -> i32 {
            if !req.adapt_terrain {
                return 0;
            }
            let on_ns = lz <= 1 || lz >= d - 2;
            let on_ew = lx <= 1 || lx >= w - 2;
            if (on_ns && gate_axis_x(lx)) || (on_ew && gate_axis_z(lz)) {
                let rz = if lz <= 1 { 2.min(d - 1) } else if lz >= d - 2 { (d - 3).max(0) } else { lz };
                let rx = if lx <= 1 { 2.min(w - 1) } else if lx >= w - 2 { (w - 3).max(0) } else { lx };
                let r_idx = (rz * w + rx) as usize;
                if is_road[r_idx] {
                    return road_y_grid[r_idx];
                }
            }
            terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-12, 12)
        };

        if fortified_walls {
            // Walkable outer rampart (`0` & `1` rim around N/S/E/W edges) + crenellations + arched road gatehouses!
            for lz in 0..d {
                for lx in 0..w {
                    let dist_edge = lx.min(w - 1 - lx).min(lz).min(d - 1 - lz);
                    if dist_edge > 1 {
                        continue;
                    }
                    let wd = terrain_at_wxz(water_depths, w, lx, lz);
                    if wd > 0 {
                        continue;
                    }
                    let terr_y = terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-14, 14);
                    let base_y = rim_base_at(lx, lz);
                    let skirt_bot = (terr_y - wd - 2).clamp(-16, base_y);
                    let fill_skirt = |blocks: &mut BTreeMap<[i32; 3], PlannedBlock>, top_incl: i32| {
                        for fy in skirt_bot..=top_incl {
                            blocks.insert(
                                [lx, fy, lz],
                                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, lz)),
                            );
                        }
                    };

                    let on_ns_edge = lz <= 1 || lz >= d - 2;
                    let on_ew_edge = lx <= 1 || lx >= w - 2;
                    let in_gate_gap = (on_ns_edge && gate_axis_x(lx)) || (on_ew_edge && gate_axis_z(lz));
                    let is_corner_bastion = (lx <= 2 || lx >= w - 3) && (lz <= 2 || lz >= d - 3);

                    if is_corner_bastion {
                        fill_skirt(&mut blocks, base_y);
                        for dy in 1..=5 {
                            let slot = if dy == 1 {
                                MaterialPalette::SLOT_FOUNDATION
                            } else if dy <= 4 {
                                MaterialPalette::SLOT_WALL_UPPER
                            } else {
                                MaterialPalette::SLOT_TRIM_WALL
                            };
                            blocks.insert([lx, base_y + dy, lz], PlannedBlock::simple(pal.pick(slot, seed, lx, base_y + dy, lz)));
                        }
                        if (lx == 0 || lx == w - 1) && (lz == 0 || lz == d - 1) {
                            blocks.insert([lx, base_y + 6, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 6, lz)));
                        } else if dist_edge == 0 && (lx + lz) % 2 == 0 {
                            blocks.insert([lx, base_y + 6, lz], make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, lx, base_y + 6, lz), "bottom"));
                        }
                        continue;
                    }

                    if in_gate_gap {
                        // Solid paved gateway approach + open arch clearance + overhead lintel aligned with road!
                        fill_skirt(&mut blocks, base_y - 1);
                        let is_border = (lx - cx).abs() == road_half + 1
                            || (lz - cz).abs() == road_half + 1
                            || ring_xs.iter().any(|&rx| (lx - rx).abs() == 1)
                            || ring_zs.iter().any(|&rz| (lz - rz).abs() == 1);
                        blocks.insert(
                            [lx, base_y, lz],
                            pave_block_for_style(spec.style, pal, seed, lx, base_y, lz, noise.cell_hash(lx, 0, lz), is_border),
                        );
                        for cy in (base_y + 1)..=(terr_y.max(base_y) + 4) {
                            blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
                        }
                        if dist_edge == 0 {
                            blocks.insert([lx, base_y + 5, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_BEAM, seed, lx, base_y + 5, lz)));
                            blocks.insert([lx, base_y + 6, lz], make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, lx, base_y + 6, lz), "bottom"));
                        }
                        continue;
                    }

                    let gate_pier = (on_ns_edge && ((lx - cx).abs() == road_half + 2 || ring_xs.iter().any(|&rx| (lx - rx).abs() == 2)))
                        || (on_ew_edge && ((lz - cz).abs() == road_half + 2 || ring_zs.iter().any(|&rz| (lz - rz).abs() == 2)));

                    fill_skirt(&mut blocks, base_y);
                    if gate_pier {
                        for dy in 1..=5 {
                            blocks.insert([lx, base_y + dy, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PILLAR, seed, lx, base_y + dy, lz)));
                        }
                        if dist_edge == 0 {
                            blocks.insert([lx, base_y + 6, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 6, lz)));
                        }
                    } else if dist_edge == 0 {
                        // Outer curtain masonry wall + battlement crenels.
                        blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 1, lz)));
                        blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 2, lz)));
                        blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 3, lz)));
                        if (lx + lz) % 2 == 0 {
                            blocks.insert([lx, base_y + 4, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 4, lz)));
                            if (lx + lz) % 8 == 0 {
                                blocks.insert([lx, base_y + 5, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 5, lz)));
                            }
                        } else {
                            blocks.insert([lx, base_y + 4, lz], make_slab(&pal.pick_slab(MaterialPalette::SLOT_ROOF, seed, lx, base_y + 4, lz), "bottom"));
                        }
                    } else {
                        // Inner rampart sentry walkway at `base_y + 3`.
                        blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 1, lz)));
                        blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_BEAM, seed, lx, base_y + 2, lz)));
                        blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, lx, base_y + 3, lz)));
                        for cy in (base_y + 4)..=(terr_y.max(base_y) + 5) {
                            blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
                        }
                    }
                }
            }
        } else {
            for lz in 0..d {
                for lx in 0..w {
                    let on_rim = lx == 0 || lx == w - 1 || lz == 0 || lz == d - 1;
                    if !on_rim {
                        continue;
                    }
                    let wd = terrain_at_wxz(water_depths, w, lx, lz);
                    if wd > 0 {
                        continue;
                    }
                    let terr_y = terrain_at_wxz(terrain_offsets, w, lx, lz).clamp(-14, 14);
                    let base_y = rim_base_at(lx, lz);
                    let in_gate = gate_axis_x(lx) && (lz == 0 || lz == d - 1)
                        || gate_axis_z(lz) && (lx == 0 || lx == w - 1);
                    let is_gate_pier = ((lx - cx).abs() == road_half + 2 && (lz == 0 || lz == d - 1))
                        || ((lz - cz).abs() == road_half + 2 && (lx == 0 || lx == w - 1));
                    let lo_y = (terr_y - wd - 2).clamp(-16, base_y);
                    if in_gate && !is_gate_pier {
                        for fy in lo_y..base_y {
                            blocks.insert([lx, fy, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, lz)));
                        }
                        blocks.insert(
                            [lx, base_y, lz],
                            pave_block_for_style(spec.style, pal, seed, lx, base_y, lz, noise.cell_hash(lx, 0, lz), false),
                        );
                        for cy in (base_y + 1)..=(terr_y.max(base_y) + 4) {
                            blocks.insert([lx, cy, lz], PlannedBlock::simple("minecraft:air"));
                        }
                        continue;
                    }
                    for fy in lo_y..=base_y {
                        blocks.insert([lx, fy, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, fy, lz)));
                    }
                    if is_gate_pier || (lx == 0 || lx == w - 1) && (lz == 0 || lz == d - 1) {
                        blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, lx, base_y + 1, lz)));
                        blocks.insert([lx, base_y + 2, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 2, lz)));
                        blocks.insert([lx, base_y + 3, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lx, base_y + 3, lz)));
                    } else if (lx + lz) % 3 == 0 {
                        blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, lx, base_y + 1, lz)));
                        blocks.insert([lx, base_y + 2, lz], make_leaves(&pal.pick(MaterialPalette::SLOT_LEAVES, seed, lx, base_y + 2, lz)));
                    } else {
                        blocks.insert([lx, base_y + 1, lz], PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, lx, base_y + 1, lz)));
                    }
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
