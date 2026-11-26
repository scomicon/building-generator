use std::collections::BTreeMap;

use crate::gen_common::{
    GeneratedBlueprint, LocalGrid, PlannedBlock, is_lantern, make_beam_block_at,
    make_frame_block_at, make_leaves, make_pillar_block_at, make_slab, make_stair,
    prune_map_unsupported, resolve_connections_and_shapes, set_uv, terrain_rel_at,
};
use crate::model::{BuildRequest, MaterialPalette};
use crate::noise::{NoiseField, SplitMix64};
use crate::parser::{BlueprintSpec, StructureTheme};

/// Dispatches specialized full-volume landmark generators when a rich architectural theme is active.
pub(crate) fn try_generate_landmark(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> Option<GeneratedBlueprint> {
    match spec.theme {
        StructureTheme::Pyramid => Some(generate_pyramid_ziggurat(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::TriumphalArch => Some(generate_triumphal_arch(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Observatory => Some(generate_observatory(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::ClockTower => Some(generate_clock_tower(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Watermill => Some(generate_watermill(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Cathedral => Some(generate_cathedral(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::JapaneseCastle => Some(generate_japanese_castle(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Bazaar => Some(generate_bazaar_market(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Treehouse => Some(generate_treehouse(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::OnsenBath => Some(generate_onsen_bath(req, spec, grid, height, noise, terrain_offsets)),
        // --- Ancient & Eastern (gen_ancient) ---
        StructureTheme::Pantheon => Some(crate::gen_ancient::generate_pantheon(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Parthenon => Some(crate::gen_ancient::generate_parthenon(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::HangingGardens => Some(crate::gen_ancient::generate_hanging_gardens(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::MosquePalace => Some(crate::gen_ancient::generate_mosque_palace(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::IshtarGate => Some(crate::gen_ancient::generate_ishtar_gate(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::AngkorTemple => Some(crate::gen_ancient::generate_angkor_temple(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::WaterShrine => Some(crate::gen_ancient::generate_water_shrine(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Sphinx => Some(crate::gen_ancient::generate_sphinx(req, spec, grid, height, noise, terrain_offsets)),
        // --- Fortress & Guilds (gen_fortress) ---
        StructureTheme::WalledBaileyCastle => Some(crate::gen_fortress::generate_walled_bailey_castle(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::CityGatehouse => Some(crate::gen_fortress::generate_city_gatehouse(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::MonasteryCloister => Some(crate::gen_fortress::generate_monastery_cloister(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::JoustingArena => Some(crate::gen_fortress::generate_jousting_arena(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::MilitaryOutpost => Some(crate::gen_fortress::generate_military_outpost(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::StarFort => Some(crate::gen_fortress::generate_star_fort(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Apothecary => Some(crate::gen_fortress::generate_apothecary(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::WineryBakery => Some(crate::gen_fortress::generate_winery_bakery(req, spec, grid, height, noise, terrain_offsets)),
        // --- Civic, Transport & Maritime (gen_civic_transport) ---
        StructureTheme::TownHall => Some(crate::gen_civic_transport::generate_town_hall(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::OperaHouse => Some(crate::gen_civic_transport::generate_opera_house(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Museum => Some(crate::gen_civic_transport::generate_museum(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::BankVault => Some(crate::gen_civic_transport::generate_bank_vault(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Academy => Some(crate::gen_civic_transport::generate_academy(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::SailingShip => Some(crate::gen_civic_transport::generate_sailing_ship(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::ShipyardDock => Some(crate::gen_civic_transport::generate_shipyard_dock(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::TrainStation => Some(crate::gen_civic_transport::generate_train_station(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::AirshipDock => Some(crate::gen_civic_transport::generate_airship_dock(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::DamBarrage => Some(crate::gen_civic_transport::generate_dam_barrage(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::CircusTent => Some(crate::gen_civic_transport::generate_circus_tent(req, spec, grid, height, noise, terrain_offsets)),
        // --- Industrial, Modern & Sci-Fi (gen_modern_industrial) ---
        StructureTheme::Skyscraper => Some(crate::gen_modern_industrial::generate_skyscraper(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::SteampunkFactory => Some(crate::gen_modern_industrial::generate_steampunk_factory(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::ModernStadium => Some(crate::gen_modern_industrial::generate_modern_stadium(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::Bunker => Some(crate::gen_modern_industrial::generate_bunker(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::LaunchpadRocket => Some(crate::gen_modern_industrial::generate_launchpad_rocket(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::OffshoreRig => Some(crate::gen_modern_industrial::generate_offshore_rig(req, spec, grid, height, noise, terrain_offsets)),
        // --- Fantasy, Mythic & Regional Houses (gen_fantasy_folk) ---
        StructureTheme::MageTower => Some(crate::gen_fantasy_folk::generate_mage_tower(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::HobbitHole => Some(crate::gen_fantasy_folk::generate_hobbit_hole(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::MushroomHouse => Some(crate::gen_fantasy_folk::generate_mushroom_house(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::MausoleumCrypt => Some(crate::gen_fantasy_folk::generate_mausoleum_crypt(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::DwarvenHall => Some(crate::gen_fantasy_folk::generate_dwarven_hall(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::DragonSanctuary => Some(crate::gen_fantasy_folk::generate_dragon_sanctuary(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::StoneCirclePortal => Some(crate::gen_fantasy_folk::generate_stone_circle_portal(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::VikingLonghouse => Some(crate::gen_fantasy_folk::generate_viking_longhouse(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::AlpineChalet => Some(crate::gen_fantasy_folk::generate_alpine_chalet(req, spec, grid, height, noise, terrain_offsets)),
        StructureTheme::DesertKasbah => Some(crate::gen_fantasy_folk::generate_desert_kasbah(req, spec, grid, height, noise, terrain_offsets)),
        _ => None,
    }
}

pub(crate) fn make_light(id: String, hanging: bool) -> PlannedBlock {
    if is_lantern(&id) {
        PlannedBlock::with_props(id, vec![("hanging", if hanging { "true" } else { "false" })])
    } else if id == "minecraft:end_rod" {
        PlannedBlock::with_props(id, vec![("facing", if hanging { "down" } else { "up" })])
    } else {
        PlannedBlock::simple(id)
    }
}

pub(crate) fn pick_stair(
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
    let stair_id = MaterialPalette::resolve_stair_id(&blk_id)
        .unwrap_or_else(|| pal.pick_stair(MaterialPalette::SLOT_ROOF, seed, u, y, v));
    make_stair(&stair_id, facing, half)
}

pub(crate) fn anchor_plinth(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    pal: &MaterialPalette,
    seed: u64,
    u0: i32,
    u1: i32,
    v0: i32,
    v1: i32,
    top_y: i32,
    terrain_offsets: Option<&[i32]>,
) {
    for v in v0..=v1 {
        for u in u0..=u1 {
            let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-8, 1);
            let bot = (terr - 2).max(-6);
            for y in bot..=top_y {
                let blk = if y == top_y {
                    pal.pick(MaterialPalette::SLOT_PAVING, seed, u, y, v)
                } else {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                };
                set_uv(blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
        }
    }
}

pub(crate) fn place_door_stack(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    pal: &MaterialPalette,
    u: i32,
    base_y: i32,
    v: i32,
) -> [i32; 3] {
    set_uv(
        blocks,
        grid,
        u,
        base_y,
        v,
        PlannedBlock::with_props(
            pal.door.clone(),
            vec![
                ("facing", grid.dir_front()),
                ("half", "lower"),
                ("hinge", "left"),
                ("open", "false"),
            ],
        ),
    );
    set_uv(
        blocks,
        grid,
        u,
        base_y + 1,
        v,
        PlannedBlock::with_props(
            pal.door.clone(),
            vec![
                ("facing", grid.dir_front()),
                ("half", "upper"),
                ("hinge", "left"),
                ("open", "false"),
            ],
        ),
    );
    let [lx, lz] = grid.to_local_xz(u, v);
    [lx, base_y, lz]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_dome_shell(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    pal: &MaterialPalette,
    seed: u64,
    cu: i32,
    cv: i32,
    radius: i32,
    base_y: i32,
    dome_h: i32,
    has_oculus: bool,
    slot: usize,
) {
    for dy in 1..=dome_h {
        let y = base_y + dy;
        let frac = f64::from(dy) / f64::from(dome_h + 1);
        let cur_r = (f64::from(radius) * (1.0 - frac * frac).max(0.0).sqrt()).max(0.0);
        for v in (cv - radius)..=(cv + radius) {
            for u in (cu - radius)..=(cu + radius) {
                let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
                if has_oculus && dy >= dome_h - 1 && d <= 1.3 {
                    continue;
                }
                if ((d - cur_r).abs() <= 0.85 && cur_r >= 0.6) || (!has_oculus && dy == dome_h && d <= 1.2) {
                    let is_rib = u == cu || v == cv;
                    let blk = if is_rib {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    } else {
                        pal.pick(slot, seed, u, y, v)
                    };
                    set_uv(blocks, grid, u, y, v, PlannedBlock::simple(blk));
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_gable_roof(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    pal: &MaterialPalette,
    seed: u64,
    u0: i32,
    u1: i32,
    v0: i32,
    v1: i32,
    eave_y: i32,
    max_h: i32,
    ridge_along_v: bool,
) {
    let mid_u = (u0 + u1) / 2;
    let mid_v = (v0 + v1) / 2;
    let half = if ridge_along_v { (u1 - u0) / 2 } else { (v1 - v0) / 2 };
    for v in v0..=v1 {
        for u in u0..=u1 {
            let dist = if ridge_along_v { (u - mid_u).abs() } else { (v - mid_v).abs() };
            let rise = (half - dist).max(0);
            let ry = (eave_y + 1 + rise / 2).min(max_h - 1);
            if dist == 0 {
                set_uv(blocks, grid, u, ry, v, make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v), "bottom"));
            } else {
                let face = if ridge_along_v {
                    if u < mid_u { grid.dir_right() } else { grid.dir_left() }
                } else if v < mid_v {
                    grid.dir_front()
                } else {
                    grid.dir_back()
                };
                set_uv(blocks, grid, u, ry, v, pick_stair(pal, MaterialPalette::SLOT_ROOF, seed, u, ry, v, face, "bottom"));
            }
        }
    }
}

pub(crate) fn finish_landmark(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    mut blocks: BTreeMap<[i32; 3], PlannedBlock>,
    _noise: NoiseField,
    door_pos: Option<[i32; 3]>,
) -> GeneratedBlueprint {
    prune_map_unsupported(&mut blocks);
    resolve_connections_and_shapes(&mut blocks);
    GeneratedBlueprint {
        spec,
        origin: [req.origin_x, req.origin_y, req.origin_z],
        width: grid.orig_w as u16,
        depth: grid.orig_d as u16,
        height,
        blocks,
        door_pos,
    }
}

/// 1. Egyptian Pyramid, Stepped Mayan Ziggurat with 4 axial stairs, or Gothic Obsidian Monolith.
fn generate_pyramid_ziggurat(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0x5059_5241_4D49_4401);
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let cv = grid.d / 2;
    let max_half = ((grid.w.min(grid.d) - 4) / 2).max(4);
    let mode = rng.range_i32(0, 2); // 0=Egyptian Smooth+Obelisks, 1=Mayan Ziggurat+Temple, 2=Obsidian Monolith

    anchor_plinth(
        &mut blocks,
        grid,
        pal,
        seed,
        cu - max_half - 1,
        cu + max_half + 1,
        cv - max_half - 1,
        cv + max_half + 1,
        0,
        terrain_offsets,
    );

    if mode == 1 {
        // Stepped Mayan Ziggurat with 3-5 monumental flat terraces + summit altar sanctuary + 4 axial stairs!
        let tier_h = if h >= 18 { 3 } else { 2 };
        let summit_h = 4.min((h - 5).max(3));
        let max_tiers = ((h - summit_h - 1) / tier_h).clamp(2, 5);
        let mut r = max_half;
        let step_in = (r / (max_tiers + 1)).clamp(2, 3);
        let mut top_y = 1;
        for t in 0..max_tiers {
            let y0 = 1 + t * tier_h;
            let y1 = (y0 + tier_h - 1).min(h - summit_h - 1);
            if r < 3 || y1 < y0 {
                break;
            }
            for y in y0..=y1 {
                for v in (cv - r)..=(cv + r) {
                    for u in (cu - r)..=(cu + r) {
                        let edge = u == cu - r || u == cu + r || v == cv - r || v == cv + r;
                        if edge || y == y1 {
                            let blk = if edge && y == y1 {
                                pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
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
            // Axial ceremonial stairways up all 4 faces.
            for (du, dv, face) in [
                (0, 1, grid.dir_back()),
                (0, -1, grid.dir_front()),
                (1, 0, grid.dir_left()),
                (-1, 0, grid.dir_right()),
            ] {
                for y in y0..=y1 {
                    let out = (y1 - y) / 1;
                    let sx = cu + du * (r + 1 + out);
                    let sz = cv + dv * (r + 1 + out);
                    for w in -1..=1 {
                        let (wu, wv) = if du == 0 { (sx + w, sz) } else { (sx, sz + w) };
                        let fill_blk = if (wu == sx && wv == sz) || w.abs() <= 1 {
                            for fy in 1..y {
                                set_uv(
                                    &mut blocks,
                                    grid,
                                    wu,
                                    fy,
                                    wv,
                                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, wu, fy, wv)),
                                );
                            }
                            pick_stair(pal, MaterialPalette::SLOT_FOUNDATION, seed, wu, y, wv, face, "bottom")
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, wu, y, wv))
                        };
                        set_uv(&mut blocks, grid, wu, y, wv, fill_blk);
                    }
                }
            }
            top_y = y1;
            r -= step_in;
        }

        // Summit Sanctuary Temple crowning the ziggurat!
        let sr = r.clamp(2, 4);
        let sy0 = top_y + 1;
        let sy1 = (sy0 + summit_h - 1).min(h - 2);
        for y in sy0..=sy1 {
            for v in (cv - sr)..=(cv + sr) {
                for u in (cu - sr)..=(cu + sr) {
                    let shell = u == cu - sr || u == cu + sr || v == cv - sr || v == cv + sr;
                    let doorway = ((u == cu && (v == cv + sr || v == cv - sr))
                        || (v == cv && (u == cu + sr || u == cu - sr)))
                        && y < sy0 + 2;
                    if doorway {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    } else if shell || y == sy1 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
        if sy1 + 1 < h {
            for v in (cv - sr)..=(cv + sr) {
                for u in (cu - sr)..=(cu + sr) {
                    if u == cu - sr || u == cu + sr || v == cv - sr || v == cv + sr {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            sy1 + 1,
                            v,
                            make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, sy1 + 1, v), "bottom"),
                        );
                    }
                }
            }
        }
        set_uv(&mut blocks, grid, cu, sy0, cv, PlannedBlock::simple("minecraft:enchanting_table"));
        set_uv(
            &mut blocks,
            grid,
            cu,
            sy1 - 1,
            cv,
            make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, sy1 - 1, cv), true),
        );
    } else {
        // Smooth Cladded Pyramid (mode 0 = sandstone + gold apex + flanking obelisks; mode 2 = monolith tomb).
        let peak_y = (max_half + 1).min(h - 2);
        for level in 0..=peak_y {
            let y = 1 + level;
            let r = (max_half - level).max(0);
            if r == 0 || y == 1 + peak_y {
                set_uv(
                    &mut blocks,
                    grid,
                    cu,
                    y,
                    cv,
                    PlannedBlock::simple(if mode == 0 {
                        "minecraft:gold_block"
                    } else {
                        "minecraft:gilded_blackstone"
                    }),
                );
                if y + 1 < h && mode == 2 {
                    set_uv(&mut blocks, grid, cu, y + 1, cv, PlannedBlock::simple("minecraft:lightning_rod"));
                }
                break;
            }
            for v in (cv - r)..=(cv + r) {
                for u in (cu - r)..=(cu + r) {
                    let on_shell = u == cu - r || u == cu + r || v == cv - r || v == cv + r;
                    if on_shell {
                        let corner = (u == cu - r || u == cu + r) && (v == cv - r || v == cv + r);
                        let is_band = (level % 4 == 0 && !corner) || (level >= peak_y - 2 && mode == 0);
                        let blk = if is_band {
                            pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                        } else {
                            pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                        };
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                    } else if level >= peak_y - 1 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }

        // Flanking Twin Entrance Obelisks on front plaza!
        if max_half >= 6 {
            let ob_v = (cv + max_half).min(grid.d - 2);
            for ob_u in [(cu - 3).max(1), (cu + 3).min(grid.w - 2)] {
                for y in 1..=(h / 2).clamp(4, 7) {
                    set_uv(
                        &mut blocks,
                        grid,
                        ob_u,
                        y,
                        ob_v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, ob_u, y, ob_v)),
                    );
                }
                let ob_top = (h / 2).clamp(4, 7) + 1;
                if ob_top < h {
                    set_uv(&mut blocks, grid, ob_u, ob_top, ob_v, PlannedBlock::simple("minecraft:gold_block"));
                }
            }
        }
    }

    // Inner Buried Tomb Sanctuary & Illuminated Portal Corridor!
    let tomb_r = (max_half - 2).clamp(2, 5);
    for v in (cv - tomb_r + 1)..=(cv + tomb_r - 1) {
        for u in (cu - tomb_r + 1)..=(cu + tomb_r - 1) {
            set_uv(
                &mut blocks,
                grid,
                u,
                0,
                v,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, 0, v)),
            );
            for y in 1..=3 {
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
            }
        }
    }
    // Sarcophagus altar + gold treasure pile + torches.
    set_uv(&mut blocks, grid, cu, 1, cv - 1, PlannedBlock::simple("minecraft:gold_block"));
    set_uv(&mut blocks, grid, cu, 1, cv, PlannedBlock::simple("minecraft:chiseled_sandstone"));
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::simple("minecraft:enchanting_table"));
    for &(lu, lv) in &[
        (cu - 2, cv - 2),
        (cu + 2, cv - 2),
        (cu - 2, cv + 2),
        (cu + 2, cv + 2),
    ] {
        set_uv(
            &mut blocks,
            grid,
            lu,
            1,
            lv,
            make_pillar_block_at(pal, "y", seed, lu, 1, lv),
        );
        set_uv(
            &mut blocks,
            grid,
            lu,
            2,
            lv,
            make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, lu, 2, lv), false),
        );
    }

    // Front portal tunnel from outer face into tomb.
    let front_v = (cv + max_half).min(grid.d - 2);
    for v in cv..=front_v {
        for u in (cu - 1)..=(cu + 1) {
            if u == cu {
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::simple("minecraft:air"));
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:air"));
            } else {
                for y in 1..=3 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)),
                    );
                }
            }
        }
        set_uv(
            &mut blocks,
            grid,
            cu,
            3,
            v,
            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, cu, 3, v)),
        );
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 1, front_v));

    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 2. Roman Triumphal Triple/Single Arch or Memorial Obelisk Colonnade with Roof Viewing Terrace.
fn generate_triumphal_arch(
    req: &BuildRequest,
    spec: BlueprintSpec,
    grid: LocalGrid,
    height: u16,
    noise: NoiseField,
    terrain_offsets: Option<&[i32]>,
) -> GeneratedBlueprint {
    let mut blocks = BTreeMap::new();
    let seed = req.seed as u64;
    let mut rng = SplitMix64::new(seed ^ 0x4152_4348_0000_0002);
    let pal = &req.palette;
    let h = i32::from(height);
    let cu = grid.w / 2;
    let cv = grid.d / 2;
    let mode = rng.range_i32(0, 2); // 0=Triple Arch, 1=Grand Single Portal, 2=Obelisk & Colonnade

    if mode == 2 {
        let plaza_r = ((grid.w.min(grid.d) - 4) / 2).clamp(4, 14);
        for v in (cv - plaza_r)..=(cv + plaza_r) {
            for u in (cu - plaza_r)..=(cu + plaza_r) {
                let d2 = ((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64;
                if d2 <= f64::from(plaza_r * plaza_r) + 0.5 {
                    let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-6, 0);
                    for y in (terr - 2)..=0 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)),
                        );
                    }
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        1,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, u, 1, v)),
                    );
                }
            }
        }
        // Central tall tapered Obelisk up to height - 2.
        let ob_top = (h - 2).max(8);
        for y in 2..=ob_top {
            let r: i32 = if y <= 4 {
                2
            } else if y <= ob_top - 3 {
                1
            } else {
                0
            };
            for dv in -r..=r {
                for du in -r..=r {
                    let blk = if y == ob_top || (y == ob_top - 1 && du == 0 && dv == 0) {
                        "minecraft:gold_block".to_owned()
                    } else if r == 2 && (du.abs() == 2 || dv.abs() == 2) {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, cu + du, y, cv + dv)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, cu + du, y, cv + dv)
                    };
                    set_uv(&mut blocks, grid, cu + du, y, cv + dv, PlannedBlock::simple(blk));
                }
            }
        }
        // Rear Semicircular Colonnade of Corinthian Pillars + Pediment Architrave.
        let col_r = (plaza_r - 1).max(3);
        let col_top = (h / 2 + 2).clamp(6, ob_top - 2);
        for v in (cv - col_r)..=cv {
            for u in (cu - col_r)..=(cu + col_r) {
                let dist = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
                if (dist - f64::from(col_r)).abs() <= 0.65 {
                    let is_col = (u + v).abs() % 2 == 0;
                    if is_col {
                        for y in 2..col_top {
                            set_uv(&mut blocks, grid, u, y, v, make_pillar_block_at(pal, "y", seed, u, y, v));
                        }
                    }
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        col_top,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, col_top, v)),
                    );
                    if col_top + 1 < h {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            col_top + 1,
                            v,
                            make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, col_top + 1, v), "bottom"),
                        );
                    }
                }
            }
        }
        let [lx, lz] = grid.to_local_xz(cu, cv + plaza_r);
        return finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]));
    }

    // Modes 0 & 1: Grand Monumental Triumphal Arch (spanning wide along `u`, thick piers along `v`).
    let half_w = ((grid.w - 4) / 2).clamp(4, 14);
    let half_t = (grid.d / 5).clamp(2, 4);
    let attic_top = (h - 3).clamp(9, h - 2);
    let main_arch_hw = if mode == 0 && half_w >= 8 {
        (half_w / 3).clamp(2, 3)
    } else {
        (half_w / 2).clamp(2, 4)
    };
    let main_arch_top = (attic_top * 65 / 100).clamp(5, attic_top - 3);
    let side_arch_c = (half_w * 2 / 3).max(main_arch_hw + 3);
    let side_arch_top = (main_arch_top - 2).clamp(4, main_arch_top);

    anchor_plinth(
        &mut blocks,
        grid,
        pal,
        seed,
        cu - half_w - 1,
        cu + half_w + 1,
        cv - half_t - 1,
        cv + half_t + 1,
        1,
        terrain_offsets,
    );

    for v in (cv - half_t)..=(cv + half_t) {
        for u in (cu - half_w)..=(cu + half_w) {
            let du_abs = (u - cu).abs();
            let in_main = du_abs <= main_arch_hw;
            let in_side = mode == 0 && half_w >= 8 && (du_abs - side_arch_c).abs() <= 1;
            let is_front_or_back = v == cv - half_t || v == cv + half_t;
            let is_end = u == cu - half_w || u == cu + half_w;

            for y in 2..=attic_top {
                let mut open_vault = false;
                if in_main {
                    let dx = du_abs as f64 / (main_arch_hw as f64 + 0.5);
                    let dy_max = main_arch_top as f64 - (1.0 - (1.0 - dx * dx).max(0.0).sqrt()) * 2.5;
                    if (y as f64) <= dy_max {
                        open_vault = true;
                    }
                } else if in_side {
                    let dx = (du_abs - side_arch_c).abs();
                    let dy_max = side_arch_top - if dx == 1 { 1 } else { 0 };
                    if y <= dy_max {
                        open_vault = true;
                    }
                }

                if open_vault {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                } else {
                    let is_cornice = y == main_arch_top + 1 || y == attic_top || y == attic_top - 2;
                    let is_column = is_front_or_back
                        && y < main_arch_top + 1
                        && (is_end || du_abs == main_arch_hw + 1 || (mode == 0 && du_abs == half_w));
                    let blk = if is_cornice {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
                    } else if is_column {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if y == attic_top - 1 && (u + v).abs() % 3 == 0 && is_front_or_back {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                }
            }

            // Rooftop parapet balustrade + corner lantern statues!
            if (is_front_or_back || is_end) && attic_top + 1 < h {
                let is_plinth = is_front_or_back && (du_abs == main_arch_hw + 1 || is_end);
                let blk = if is_plinth {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, attic_top + 1, v))
                } else {
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, attic_top + 1, v))
                };
                set_uv(&mut blocks, grid, u, attic_top + 1, v, blk);
                if is_plinth && attic_top + 2 < h {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        attic_top + 2,
                        v,
                        make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, attic_top + 2, v), false),
                    );
                }
            }
        }
    }

    // Keystone lanterns under vault soffits.
    set_uv(
        &mut blocks,
        grid,
        cu,
        main_arch_top,
        cv,
        make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, main_arch_top, cv), true),
    );
    let [lx, lz] = grid.to_local_xz(cu, cv + half_t);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 3. Astronomical Observatory Tower with Copper Slotted Dome & Angled Star Telescope.
fn generate_observatory(
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
    let radius = ((grid.w.min(grid.d) - 4) / 2).clamp(4, 11);
    let drum_top = ((h * 56) / 100).clamp(6, (h - 5).max(6));
    let dome_h = (h - 1 - drum_top).clamp(4, radius + 2);

    // Cylindrical stepped stone foundation drum + circular gallery floors.
    for v in (cv - radius - 1)..=(cv + radius + 1) {
        for u in (cu - radius - 1)..=(cu + radius + 1) {
            let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
            if d > f64::from(radius) + 1.2 {
                continue;
            }
            let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-6, 0);
            if d <= f64::from(radius) + 0.35 {
                for y in (terr - 2)..=0 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)),
                    );
                }
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, 1, v)),
                );
            }
        }
    }

    for y in 2..=drum_top {
        let is_floor = y == drum_top || y == drum_top / 2 + 1;
        let is_balcony = y == drum_top;
        let r_max = if is_balcony { radius + 1 } else { radius };
        for v in (cv - r_max)..=(cv + r_max) {
            for u in (cu - r_max)..=(cu + r_max) {
                let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
                let on_wall = (d >= f64::from(radius) - 0.65) && (d <= f64::from(radius) + 0.35);
                if is_balcony && d > f64::from(radius) + 0.35 && d <= f64::from(radius) + 1.35 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)),
                    );
                    if y + 1 < h {
                        let fence_or_lamp = if (u == cu || v == cv) && y + 2 < h {
                            set_uv(
                                &mut blocks,
                                grid,
                                u,
                                y + 2,
                                v,
                                make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, y + 2, v), false),
                            );
                            pal.pick(MaterialPalette::SLOT_TRIM_WALL, seed, u, y + 1, v)
                        } else {
                            pal.pick(MaterialPalette::SLOT_FENCE, seed, u, y + 1, v)
                        };
                        set_uv(&mut blocks, grid, u, y + 1, v, PlannedBlock::simple(fence_or_lamp));
                    }
                } else if on_wall {
                    let is_window = (y % 4 == 3) && ((u - cu).abs() <= 1 || (v - cv).abs() <= 1);
                    let blk = if is_floor || y == 2 {
                        pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                    } else if is_window {
                        pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if d < f64::from(radius) - 0.65 {
                    if is_floor {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, y, v)),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
    }

    // Slotted Oxidized Copper Observatory Dome! (Leaves an open observation slit toward `u > cu`).
    let slit_v_half = 1;
    let apex_y = (drum_top + dome_h).min(h - 2);
    for dy in 1..=(apex_y - drum_top) {
        let y = drum_top + dy;
        let frac = f64::from(dy) / f64::from((apex_y - drum_top) + 1);
        let cur_r = (f64::from(radius) * (1.0 - frac * frac).max(0.0).sqrt()).max(0.0);
        for v in (cv - radius)..=(cv + radius) {
            for u in (cu - radius)..=(cu + radius) {
                let d = (((u - cu) * (u - cu) + (v - cv) * (v - cv)) as f64).sqrt();
                let in_slit = y < apex_y && v >= cv - slit_v_half && v <= cv + slit_v_half && u >= cu + 1;
                if in_slit {
                    continue;
                }
                if (d - cur_r).abs() <= 0.85 && cur_r >= 0.6 {
                    let is_rib = u == cu || v == cv;
                    let blk = if is_rib {
                        pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_ROOF, seed, u, y, v)
                    };
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                } else if (cur_r < 0.7 || y == apex_y) && d <= 1.2 {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, y, v)),
                    );
                }
            }
        }
    }
    if apex_y + 1 < h {
        set_uv(
            &mut blocks,
            grid,
            cu,
            apex_y + 1,
            cv,
            PlannedBlock::simple("minecraft:lightning_rod"),
        );
    }

    // Angled Telescope Brass & Glass Lens Mount rising from the upper dome floor!
    let base_tel_y = drum_top + 1;
    set_uv(&mut blocks, grid, cu, base_tel_y, cv, PlannedBlock::simple("minecraft:anvil"));
    let reach = (radius + 2).min(h - base_tel_y - 1).clamp(3, 7);
    for step in 1..=reach {
        let tu = cu + step;
        let ty = base_tel_y + (step * 3) / 4;
        if !grid.in_bounds(tu, cv) || ty >= h {
            break;
        }
        let id = if step == reach {
            "minecraft:tinted_glass"
        } else if step == 1 {
            "minecraft:cut_copper"
        } else {
            "minecraft:copper_block"
        };
        set_uv(&mut blocks, grid, tu, ty, cv, PlannedBlock::simple(id));
        if step < reach {
            set_uv(&mut blocks, grid, tu, ty + 1, cv, PlannedBlock::simple("minecraft:cut_copper_slab"));
        }
    }

    // Interior astronomy library + central spiral ladder/stair column.
    set_uv(&mut blocks, grid, cu - 1, 2, cv - 1, PlannedBlock::simple("minecraft:bookshelf"));
    set_uv(&mut blocks, grid, cu + 1, 2, cv - 1, PlannedBlock::simple("minecraft:lectern"));
    for y in 2..=drum_top {
        set_uv(&mut blocks, grid, cu - 1, y, cv, make_frame_block_at(pal, "y", seed, cu - 1, y, cv));
        set_uv(
            &mut blocks,
            grid,
            cu - 2,
            y,
            cv,
            PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_left())]),
        );
    }

    // Door at front perimeter (`v = cv + radius`).
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + radius));
    set_uv(&mut blocks, grid, cu, 1, cv + radius + 1, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_PAVING, seed, cu, 1, cv + radius + 1)));

    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 4. Towering Stepped Clock Tower / Big Ben Campanile with 4 Illuminated Clockfaces & Belfry Bell.
fn generate_clock_tower(
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
    let r = ((grid.w.min(grid.d) - 4) / 2).clamp(3, 6);

    let shaft_top = ((h * 52) / 100).clamp(5, (h - 7).max(5));
    let clock_bot = shaft_top + 1;
    let clock_top = (clock_bot + 4).min(h - 5).max(clock_bot + 3);
    let belfry_top = (clock_top + 3).min(h - 3).max(clock_top + 2);

    anchor_plinth(&mut blocks, grid, pal, seed, cu - r - 1, cu + r + 1, cv - r - 1, cv + r + 1, 1, terrain_offsets);

    // Tapered shaft with buttressed corner pilasters.
    for y in 2..=shaft_top {
        for v in (cv - r)..=(cv + r) {
            for u in (cu - r)..=(cu + r) {
                let shell = u == cu - r || u == cu + r || v == cv - r || v == cv + r;
                let corner = (u == cu - r || u == cu + r) && (v == cv - r || v == cv + r);
                if shell {
                    let is_slit = !corner && (u == cu || v == cv) && (y % 4 == 3);
                    let blk = if corner {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else if is_slit {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else if y == shaft_top {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, y, v)),
                    );
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // 4-Sided Illuminated Clockfaces!
    let clock_mid_y = (clock_bot + clock_top) / 2;
    for y in clock_bot..=clock_top {
        for v in (cv - r - 1)..=(cv + r + 1) {
            for u in (cu - r - 1)..=(cu + r + 1) {
                let on_frame_ring = ((u - cu).abs() == r + 1 && (v - cv).abs() <= r)
                    || ((v - cv).abs() == r + 1 && (u - cu).abs() <= r);
                let in_body = (u - cu).abs() <= r && (v - cv).abs() <= r;
                if on_frame_ring {
                    // Projecting ornamental stone clock-hood cornice at top and dial rim around the center.
                    let du = (u - cu).abs();
                    let dv = (v - cv).abs();
                    let in_dial = (du == r + 1 && dv <= 1 && (y - clock_mid_y).abs() <= 1)
                        || (dv == r + 1 && du <= 1 && (y - clock_mid_y).abs() <= 1);
                    if in_dial {
                        let is_hand = y == clock_mid_y && (du == 0 || dv == 0)
                            || (y == clock_mid_y + 1 && (du == 0 || dv == 0));
                        let blk = if is_hand {
                            "minecraft:polished_blackstone".to_owned()
                        } else {
                            pal.pick(MaterialPalette::SLOT_LIGHT, seed, u, y, v)
                        };
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                    } else if y == clock_top {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)),
                        );
                    }
                } else if in_body {
                    let shell = (u - cu).abs() == r || (v - cv).abs() == r;
                    if shell {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)),
                        );
                    } else if y == clock_top {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, y, v)),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
    }

    // Open Belfry Lantern Stage with Hanging Bell!
    let br = (r - 1).max(2);
    for y in (clock_top + 1)..=belfry_top {
        for v in (cv - br)..=(cv + br) {
            for u in (cu - br)..=(cu + br) {
                let corner = (u == cu - br || u == cu + br) && (v == cv - br || v == cv + br);
                if corner || y == belfry_top {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)),
                    );
                }
            }
        }
    }
    set_uv(
        &mut blocks,
        grid,
        cu,
        belfry_top - 1,
        cv,
        PlannedBlock::with_props("minecraft:bell", vec![("facing", "north"), ("attachment", "ceiling")]),
    );

    // Tapered Copper/Slate Spire crowning above belfry!
    let spire_h = (h - 1 - belfry_top).max(2);
    for sy in 1..=spire_h {
        let y = belfry_top + sy;
        let sr = (br - sy / 2).max(0);
        for v in (cv - sr)..=(cv + sr) {
            for u in (cu - sr)..=(cu + sr) {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    y,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, y, v)),
                );
            }
        }
    }
    if belfry_top + spire_h + 1 < h {
        set_uv(
            &mut blocks,
            grid,
            cu,
            belfry_top + spire_h + 1,
            cv,
            PlannedBlock::simple("minecraft:lightning_rod"),
        );
    }

    // Interior winding ladder core & entrance door.
    for y in 2..=clock_top {
        set_uv(&mut blocks, grid, cu, y, cv, make_frame_block_at(pal, "y", seed, cu, y, cv));
        set_uv(
            &mut blocks,
            grid,
            cu,
            y,
            cv - 1,
            PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_back())]),
        );
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, cv + r));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 5. Riverside Watermill with Vertical Rotating Paddle Wheel, Flume Aqueduct & Flour Millstone.
fn generate_watermill(
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
    let cu = grid.w / 2 + 1;
    let cv = grid.d / 2;
    let hw = ((grid.w - 6) / 2).clamp(3, 6);
    let hd = ((grid.d - 4) / 2).clamp(3, 7);
    let u0 = (cu - hw).max(4);
    let u1 = (u0 + hw * 2).min(grid.w - 2);
    let v0 = (cv - hd).max(1);
    let v1 = (v0 + hd * 2).min(grid.d - 2);
    let eave_y = (h / 2 + 2).clamp(6, h - 4);

    // Stone tailrace water channel along left side `u = u0 - 3 .. u0 - 1`!
    let wheel_u = u0 - 2;
    for v in (v0 - 1).max(0)..=(v1 + 1).min(grid.d - 1) {
        for u in (u0 - 3).max(0)..=(u0 - 1) {
            let wall = u == (u0 - 3).max(0) || v == (v0 - 1).max(0) || v == (v1 + 1).min(grid.d - 1);
            set_uv(
                &mut blocks,
                grid,
                u,
                0,
                v,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 0, v)),
            );
            if wall {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 1, v)),
                );
            } else {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::with_props("minecraft:water", vec![("level", "0")]),
                );
            }
        }
    }

    // Mill House base + timber-framed upper + steep Gable roof!
    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 1, terrain_offsets);
    for y in 2..=eave_y {
        for v in v0..=v1 {
            for u in u0..=u1 {
                let shell = u == u0 || u == u1 || v == v0 || v == v1;
                if shell {
                    let corner = (u == u0 || u == u1) && (v == v0 || v == v1);
                    let blk = if corner {
                        make_frame_block_at(pal, "y", seed, u, y, v)
                    } else if y <= 4 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v))
                    } else if y == eave_y {
                        make_beam_block_at(pal, grid.axis_u(), seed, u, y, v)
                    } else if (v - cv).abs() == 1 && y == 4 {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_UPPER, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                } else {
                    set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                }
            }
        }
    }

    // Gable Roof spanning along `v`!
    let roof_mid_u = (u0 + u1) / 2;
    let roof_half = (u1 - u0) / 2 + 1;
    for v in (v0 - 1).max(0)..=(v1 + 1).min(grid.d - 1) {
        for du in -roof_half..=roof_half {
            let u = roof_mid_u + du;
            if !grid.in_bounds(u, v) {
                continue;
            }
            let edge_rise = roof_half - du.abs();
            let ry = (eave_y + 1 + edge_rise / 2).min(h - 1);
            if du == 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v), "bottom"),
                );
            } else {
                let face = if du < 0 { grid.dir_right() } else { grid.dir_left() };
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    pick_stair(pal, MaterialPalette::SLOT_ROOF, seed, u, ry, v, face, "bottom"),
                );
            }
        }
    }

    // Vertical Exterior Paddle Waterwheel centered at `(wheel_u, hub_y, cv)` + Drive Axle + Elevated Flume!
    let wheel_r = (eave_y - 2).clamp(3, 5);
    let hub_y = (wheel_r + 1).min(eave_y - 1);
    for u in wheel_u..=u0 {
        set_uv(&mut blocks, grid, u, hub_y, cv, make_frame_block_at(pal, grid.axis_u(), seed, u, hub_y, cv));
    }
    for dv in -wheel_r..=wheel_r {
        for dy in -wheel_r..=wheel_r {
            let dist = ((dv * dv + dy * dy) as f64).sqrt();
            let y = hub_y + dy;
            let v = cv + dv;
            if y < 1 || y >= h || !grid.in_bounds(wheel_u, v) {
                continue;
            }
            if (dist - f64::from(wheel_r)).abs() <= 0.65 {
                let is_paddle = dv == 0 || dy == 0 || (dv.abs() == dy.abs());
                let blk = if is_paddle {
                    pal.pick(MaterialPalette::SLOT_TRAPDOOR, seed, wheel_u, y, v)
                } else {
                    pal.pick(MaterialPalette::SLOT_FRAME, seed, wheel_u, y, v)
                };
                if is_paddle {
                    set_uv(
                        &mut blocks,
                        grid,
                        wheel_u,
                        y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, wheel_u, y, v)),
                    );
                } else {
                    set_uv(&mut blocks, grid, wheel_u, y, v, PlannedBlock::simple(blk));
                }
            } else if (dv == 0 || dy == 0 || dv.abs() == dy.abs()) && dist < f64::from(wheel_r) {
                set_uv(
                    &mut blocks,
                    grid,
                    wheel_u,
                    y,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, wheel_u, y, v)),
                );
            }
        }
    }

    // Elevated overhead wooden Mill-Race Aqueduct feeding the top of the wheel!
    let flume_y = (hub_y + wheel_r + 1).min(h - 2);
    for v in 0..=cv {
        if grid.in_bounds(wheel_u, v) {
            set_uv(
                &mut blocks,
                grid,
                wheel_u,
                flume_y,
                v,
                make_slab(&pal.pick(MaterialPalette::SLOT_FLOOR, seed, wheel_u, flume_y, v), "bottom"),
            );
            if v % 4 == 0 {
                for sy in 1..flume_y {
                    set_uv(&mut blocks, grid, wheel_u, sy, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, wheel_u, sy, v)));
                }
            }
        }
    }

    // Interior Grindstone & Grain Barrels!
    set_uv(&mut blocks, grid, u0 + 2, 2, cv, PlannedBlock::simple("minecraft:smooth_stone"));
    set_uv(&mut blocks, grid, u0 + 2, 3, cv, PlannedBlock::simple("minecraft:hay_block"));
    set_uv(&mut blocks, grid, u1 - 1, 2, v0 + 1, PlannedBlock::simple("minecraft:barrel"));
    set_uv(&mut blocks, grid, u1 - 1, 2, v0 + 2, PlannedBlock::simple("minecraft:hay_block"));
    set_uv(
        &mut blocks,
        grid,
        roof_mid_u,
        eave_y,
        cv,
        make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, roof_mid_u, eave_y, cv), true),
    );

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, roof_mid_u, 2, v1));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 6. Cruciform Gothic/Nordic Cathedral Basilica with Twin Spires, Rose Window, Flying Buttresses, Pews & Bell.
fn generate_cathedral(
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
    let total_hw = ((grid.w - 4) / 2).clamp(5, 11);
    let nave_hw = (total_hw / 2).clamp(2, 5);
    let v_front = grid.d - 3;
    let v_back = 2;
    let transept_v = (v_back + (v_front - v_back) / 3).max(v_back + 4);
    let nave_wall_h = ((h * 56) / 100).clamp(7, h - 5);
    let aisle_wall_h = (nave_wall_h * 58 / 100).clamp(4, nave_wall_h - 2);
    let tower_top = (h - 5).clamp(nave_wall_h + 2, h - 2);

    // Cruciform + Aisles Footprint mask cleanly anchors the whole cathedral!
    for v in v_back..=v_front {
        for u in (cu - total_hw)..=(cu + total_hw) {
            let du = (u - cu).abs();
            let in_transept = (v - transept_v).abs() <= nave_hw;
            let in_footprint = du <= total_hw && (in_transept || du <= total_hw - 1);
            if !in_footprint {
                continue;
            }
            let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-6, 0);
            for y in (terr - 2)..=1 {
                let blk = if y == 1 {
                    if (u + v).abs() % 2 == 0 {
                        pal.pick(MaterialPalette::SLOT_PAVING, seed, u, y, v)
                    } else {
                        pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, y, v)
                    }
                } else {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
        }
    }

    // Outer Aisle & Transept Walls + Tall Upper Clerestory Nave Walls + Flying Buttresses!
    for v in v_back..=v_front {
        let in_transept = (v - transept_v).abs() <= nave_hw;
        let outer_hw = if in_transept { total_hw } else { total_hw - 1 };
        let aisle_top = if in_transept { nave_wall_h } else { aisle_wall_h };

        for u in (cu - outer_hw)..=(cu + outer_hw) {
            let du = (u - cu).abs();
            let on_outer_wall = u == cu - outer_hw || u == cu + outer_hw || v == v_front || v == v_back;
            let on_nave_arcade = du == nave_hw && !in_transept && v > v_back && v < v_front;

            if on_outer_wall {
                let is_pier = (v % 3 == 0) || v == v_front || v == v_back;
                for y in 2..=aisle_top {
                    let is_window = !is_pier && y >= 3 && y < aisle_top && (v - v_back) % 3 != 0 && v < v_front;
                    let blk = if is_window {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
                    } else if is_pier {
                        make_pillar_block_at(pal, "y", seed, u, y, v)
                    } else {
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v))
                    };
                    set_uv(&mut blocks, grid, u, y, v, blk);
                }
                // Sloped aisle roof bridging inward to nave wall.
                if !in_transept && v > v_back && v < v_front && du > nave_hw {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        aisle_top + 1,
                        v,
                        make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, u, aisle_top + 1, v), "bottom"),
                    );
                }
            } else if du > nave_hw && du < outer_hw && !in_transept {
                let ry = (aisle_top + (outer_hw - du).min(2)).min(nave_wall_h - 1);
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v)),
                );
            }

            // Flying Buttress arches at rhythmic bay intervals jumping from outer pier to nave wall!
            if !in_transept && v > v_back + 2 && v < v_front - 2 && (v % 4 == 0) && du > nave_hw && du <= outer_hw {
                let by = (aisle_top + 2 + (outer_hw - du)).min(nave_wall_h);
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    by,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, by, v)),
                );
            }

            // Tall Clerestory Upper Nave Walls!
            if on_nave_arcade {
                for y in 2..=nave_wall_h {
                    let is_arcade_open = y <= aisle_top - 1 && (v % 3 != 0);
                    let is_clerestory = y > aisle_top + 1 && y < nave_wall_h && (v % 3 != 0);
                    if is_arcade_open {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    } else if is_clerestory {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, u, y, v)),
                        );
                    } else {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)),
                        );
                    }
                }
            }
        }
    }

    // Steep Vaulted Nave Roof along `v`!
    for v in v_back..=v_front {
        for du in -(nave_hw + 1)..=(nave_hw + 1) {
            let u = cu + du;
            if !grid.in_bounds(u, v) {
                continue;
            }
            let lift = (nave_hw + 1 - du.abs()).max(0);
            let ry = (nave_wall_h + 1 + lift).min(h - 1);
            if du == 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, ry, v)),
                );
            } else {
                let face = if du < 0 { grid.dir_right() } else { grid.dir_left() };
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    ry,
                    v,
                    pick_stair(pal, MaterialPalette::SLOT_ROOF, seed, u, ry, v, face, "bottom"),
                );
            }
        }
    }

    // Twin West Front Spired Bell Towers + Stained-Glass Rose Window centered over Portal!
    let tw_r = 2.min((total_hw - 1) / 2).max(1);
    for tower_side in [-1, 1] {
        let tc = cu + tower_side * (nave_hw + tw_r).min(total_hw - 1);
        for dv in -1..=1 {
            let tv = v_front - 1 + dv;
            for du in -1..=1 {
                let tu = tc + du;
                for y in 2..=tower_top {
                    let shell = du.abs() == 1 || dv.abs() == 1;
                    let is_window_open = (du == 0 || dv == 0) && y >= tower_top - 3 && y < tower_top;
                    if (shell || y == tower_top) && !is_window_open {
                        set_uv(
                            &mut blocks,
                            grid,
                            tu,
                            y,
                            tv,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, tu, y, tv)),
                        );
                    } else if is_window_open {
                        set_uv(&mut blocks, grid, tu, y, tv, PlannedBlock::simple("minecraft:air"));
                    } else if du == 0 && dv == 0 && y < tower_top {
                        set_uv(&mut blocks, grid, tu, y, tv, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }
        // Spire tip!
        for sy in 1..=(h - 1 - tower_top).clamp(2, 5) {
            let y = tower_top + sy;
            let sr = if sy <= 2 { 1 } else { 0 };
            for dv in -sr..=sr {
                for du in -sr..=sr {
                    set_uv(
                        &mut blocks,
                        grid,
                        tc + du,
                        y,
                        v_front - 1 + dv,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, tc + du, y, v_front - 1 + dv)),
                    );
                }
            }
        }
        // Hang a real Cathedral Bell in left west tower!
        if tower_side < 0 {
            set_uv(
                &mut blocks,
                grid,
                tc,
                tower_top - 1,
                v_front - 1,
                PlannedBlock::with_props("minecraft:bell", vec![("facing", "north"), ("attachment", "ceiling")]),
            );
        }
    }

    // Front Rose Stained-Glass Window between the West Towers!
    let rose_y = (nave_wall_h - 1).max(6).min(h - 3);
    for dy in -1..=1 {
        for du in -1..=1 {
            set_uv(
                &mut blocks,
                grid,
                cu + du,
                rose_y + dy,
                v_front,
                PlannedBlock::simple(if du == 0 && dy == 0 {
                    pal.pick(MaterialPalette::SLOT_LIGHT_ACCENT, seed, cu, rose_y, v_front)
                } else {
                    pal.pick(MaterialPalette::SLOT_WINDOW_ACCENT, seed, cu + du, rose_y + dy, v_front)
                }),
            );
        }
    }

    // Interior Cathedral Furnishings: Sanctuary Altar + Rows of Oak/Spruce Pews + Chandeliers!
    set_uv(&mut blocks, grid, cu, 2, v_back + 2, PlannedBlock::simple("minecraft:lectern"));
    set_uv(&mut blocks, grid, cu - 1, 2, v_back + 2, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT_ACCENT, seed, cu - 1, 2, v_back + 2)));
    set_uv(&mut blocks, grid, cu + 1, 2, v_back + 2, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_LIGHT_ACCENT, seed, cu + 1, 2, v_back + 2)));
    for v in (transept_v + 2..=v_front - 4).step_by(2) {
        for du in [-(nave_hw - 1), -(nave_hw - 2), nave_hw - 2, nave_hw - 1] {
            if du != 0 && (cu + du).abs() > 0 {
                set_uv(
                    &mut blocks,
                    grid,
                    cu + du,
                    2,
                    v,
                    pick_stair(pal, MaterialPalette::SLOT_FLOOR, seed, cu + du, 2, v, grid.dir_back(), "bottom"),
                );
            }
        }
    }
    for v in (v_back + 4..v_front - 3).step_by(5) {
        set_uv(
            &mut blocks,
            grid,
            cu,
            nave_wall_h - 1,
            v,
            make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cu, nave_wall_h - 1, v), true),
        );
    }

    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, 2, v_front));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 7. Authentic Japanese Tenshu / Himeji Castle with Battered Ishigaki Plinth & Receding Pagoda Tiers.
fn generate_japanese_castle(
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
    let base_r = ((grid.w.min(grid.d) - 4) / 2).clamp(4, 10);
    let plinth_h = (h / 5).clamp(3, 5);

    // 1. Sloped Ishigaki stone foundation fortress base (`r` tapers cleanly inward by 1-2 blocks up to `plinth_h`).
    anchor_plinth(&mut blocks, grid, pal, seed, cu - base_r, cu + base_r, cv - base_r, cv + base_r, 0, terrain_offsets);
    for y in 1..=plinth_h {
        let inset = (y / 2).min(2);
        let r = (base_r - inset).max(3);
        for v in (cv - r)..=(cv + r) {
            for u in (cu - r)..=(cu + r) {
                let shell = u == cu - r || u == cu + r || v == cv - r || v == cv + r;
                let corner = (u == cu - r || u == cu + r) && (v == cv - r || v == cv + r);
                let blk = if corner {
                    pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, u, y, v)
                } else if y == plinth_h {
                    pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, y, v)
                } else if shell {
                    pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                } else {
                    continue;
                };
                set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
            }
        }
    }

    // 2. Stacked Receding Tenshu Pagoda Keep Tiers (`2`..`4` stories)!
    let start_r = (base_r - (plinth_h / 2).min(2)).max(3);
    let tier_h = 4;
    let max_tiers = ((h - plinth_h - 3) / tier_h).clamp(2, 4);
    let mut cur_r = start_r;
    let mut cur_y = plinth_h + 1;
    let first_r = start_r;

    for t in 0..max_tiers {
        let wall_top = (cur_y + tier_h - 2).min(h - 3);
        if wall_top < cur_y + 1 || cur_r < 2 {
            break;
        }
        // Plaster white / framed walls for tier `t`.
        for y in cur_y..=wall_top {
            for v in (cv - cur_r)..=(cv + cur_r) {
                for u in (cu - cur_r)..=(cu + cur_r) {
                    let shell = u == cu - cur_r || u == cu + cur_r || v == cv - cur_r || v == cv + cur_r;
                    let corner = (u == cu - cur_r || u == cu + cur_r) && (v == cv - cur_r || v == cv + cur_r);
                    if shell {
                        let win = !corner && y == cur_y + 1 && (u == cu || v == cv);
                        let blk = if corner {
                            make_frame_block_at(pal, "y", seed, u, y, v)
                        } else if win {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v))
                        } else {
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_UPPER, seed, u, y, v))
                        };
                        set_uv(&mut blocks, grid, u, y, v, blk);
                    } else if y == cur_y && t > 0 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR_UPPER, seed, u, y, v)),
                        );
                    } else {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple("minecraft:air"));
                    }
                }
            }
        }

        // Upturned Cantilevered Pagoda Eaves Ring protruding +1/+2 around this tier!
        let eave_y = wall_top + 1;
        let eave_r = (cur_r + 2).min(base_r + 1);
        for v in (cv - eave_r)..=(cv + eave_r) {
            for u in (cu - eave_r)..=(cu + eave_r) {
                let d_inf = (u - cu).abs().max((v - cv).abs());
                if d_inf < cur_r {
                    continue;
                }
                let is_tip = (u - cu).abs() == eave_r && (v - cv).abs() == eave_r;
                if d_inf == eave_r {
                    if eave_y + 1 < h {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            eave_y,
                            v,
                            make_slab(&pal.pick(MaterialPalette::SLOT_ROOF_EDGE, seed, u, eave_y, v), "top"),
                        );
                        if is_tip {
                            set_uv(
                                &mut blocks,
                                grid,
                                u,
                                eave_y + 1,
                                v,
                                make_slab(&pal.pick(MaterialPalette::SLOT_ROOF_EDGE, seed, u, eave_y + 1, v), "bottom"),
                            );
                        }
                    }
                } else {
                    set_uv(
                        &mut blocks,
                        grid,
                        u,
                        eave_y,
                        v,
                        PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, u, eave_y, v)),
                    );
                }
            }
        }

        cur_y = eave_y + 1;
        cur_r = (cur_r - 1).max(2);
    }

    // Crown Gable Ridge & Shachihoko Roof Finials!
    let cap_top = cur_y.min(h - 2);
    for dv in -(cur_r + 1)..=(cur_r + 1) {
        for du in -(cur_r + 1)..=(cur_r + 1) {
            set_uv(
                &mut blocks,
                grid,
                cu + du,
                cap_top,
                cv + dv,
                PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_ROOF, seed, cu + du, cap_top, cv + dv)),
            );
        }
    }
    if cap_top + 1 < h {
        for du in -(cur_r)..=cur_r {
            let blk = if du.abs() == cur_r {
                "minecraft:gold_block".to_owned()
            } else {
                pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, cu + du, cap_top + 1, cv)
            };
            set_uv(&mut blocks, grid, cu + du, cap_top + 1, cv, PlannedBlock::simple(blk));
        }
    }

    // Central ladder access + entrance stairs up Ishigaki plinth!
    for y in 2..cap_top {
        set_uv(&mut blocks, grid, cu, y, cv, make_frame_block_at(pal, "y", seed, cu, y, cv));
        set_uv(
            &mut blocks,
            grid,
            cu,
            y,
            cv - 1,
            PlannedBlock::with_props("minecraft:ladder", vec![("facing", grid.dir_back())]),
        );
    }
    for y in 1..=plinth_h {
        let sv = (cv + first_r + 1 + (plinth_h - y)).min(grid.d - 1);
        set_uv(
            &mut blocks,
            grid,
            cu,
            y,
            sv,
            pick_stair(pal, MaterialPalette::SLOT_FOUNDATION, seed, cu, y, sv, grid.dir_back(), "bottom"),
        );
    }
    let door_pos = Some(place_door_stack(&mut blocks, grid, pal, cu, plinth_h + 1, cv + first_r));
    finish_landmark(req, spec, grid, height, blocks, noise, door_pos)
}

/// 8. Colorful Striped Market Bazaar Square with 4 Awnings, Merchandise Barrels & Central Fountain.
fn generate_bazaar_market(
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

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 0, terrain_offsets);
    for v in v0..=v1 {
        for u in u0..=u1 {
            let pattern = (u + v) % 4 == 0;
            set_uv(
                &mut blocks,
                grid,
                u,
                1,
                v,
                PlannedBlock::simple(pal.pick(
                    if pattern { MaterialPalette::SLOT_WALL_ACCENT } else { MaterialPalette::SLOT_PAVING },
                    seed,
                    u,
                    1,
                    v,
                )),
            );
        }
    }

    // Central Octagonal Oasis Fountain (`minecraft:water` enclosed cleanly)!
    let f_r = 2.min((grid.w.min(grid.d) - 8) / 4).max(1);
    for v in (cv - f_r - 1)..=(cv + f_r + 1) {
        for u in (cu - f_r - 1)..=(cu + f_r + 1) {
            let rim = (u - cu).abs() == f_r + 1 || (v - cv).abs() == f_r + 1;
            if rim {
                set_uv(&mut blocks, grid, u, 2, v, make_slab(&pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, 2, v), "bottom"));
            } else {
                set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
            }
        }
    }
    set_uv(&mut blocks, grid, cu, 2, cv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, cu, 2, cv)));
    set_uv(&mut blocks, grid, cu, 3, cv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_WALL_ACCENT, seed, cu, 3, cv)));
    set_uv(&mut blocks, grid, cu, 4, cv, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));

    // 4 Colorful Corner Market Pavilion Stalls!
    let tent_colors = [
        ("minecraft:red_wool", "minecraft:white_wool"),
        ("minecraft:blue_wool", "minecraft:yellow_wool"),
        ("minecraft:cyan_wool", "minecraft:white_wool"),
        ("minecraft:orange_wool", "minecraft:yellow_wool"),
    ];
    let corner_centers = [
        (u0 + 2, v0 + 2, 0),
        (u1 - 2, v0 + 2, 1),
        (u0 + 2, v1 - 2, 2),
        (u1 - 2, v1 - 2, 3),
    ];
    let stall_roof_y = 6.min(h - 2);
    for (su, sv, idx) in corner_centers {
        let (col_a, col_b) = tent_colors[idx];
        for dv in -2..=2 {
            for du in -2..=2 {
                let u = su + du;
                let v = sv + dv;
                if !grid.in_bounds(u, v) || ((u - cu).abs() <= f_r + 1 && (v - cv).abs() <= f_r + 1) {
                    continue;
                }
                if du.abs() == 2 && dv.abs() == 2 {
                    for y in 2..stall_roof_y {
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FENCE, seed, u, y, v)));
                    }
                }
                let ry = stall_roof_y + if du.abs() <= 1 && dv.abs() <= 1 { 1 } else { 0 };
                let cloth = if (u + idx as i32) % 2 == 0 { col_a } else { col_b };
                set_uv(&mut blocks, grid, u, ry.min(h - 1), v, PlannedBlock::simple(cloth));
                if (du == 0 || dv == 0) && du.abs() + dv.abs() == 1 {
                    set_uv(&mut blocks, grid, u, 2, v, PlannedBlock::simple("minecraft:barrel"));
                }
            }
        }
        set_uv(
            &mut blocks,
            grid,
            su,
            stall_roof_y,
            sv,
            make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, su, stall_roof_y, sv), true),
        );
    }

    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}

/// 9. Giant Ancient Treehouse with Buttress Roots, Elevated Multi-Cabins, Rope Bridge & Lush Canopy.
fn generate_treehouse(
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
    let trunk_top = (h - 4).clamp(9, h - 2);
    let platform_y = (h / 2).clamp(5, trunk_top - 4);
    let leaf_id = pal.pick(MaterialPalette::SLOT_LEAVES, seed, cu, trunk_top, cv);

    // Ancient Buttress Roots & 3x3 Living Bark Trunk with spiral ladder inside/outside.
    for v in (cv - 3)..=(cv + 3) {
        for u in (cu - 3)..=(cu + 3) {
            let d = (u - cu).abs().max((v - cv).abs());
            if d <= 1 || ((u == cu || v == cv) && d <= 3) {
                let terr = terrain_rel_at(terrain_offsets, grid, u, v).clamp(-6, 0);
                let top = if d <= 1 { trunk_top } else { (4 - d).max(1) };
                for y in (terr - 2)..=top {
                    set_uv(&mut blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                }
            }
        }
    }

    // Spiral Wooden Staircase wrapping up the trunk to the main elevated deck!
    let dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    for y in 1..=platform_y {
        let dir = dirs[(y as usize) % 4];
        let su = cu + dir.0 * 2;
        let sv = cv + dir.1 * 2;
        if grid.in_bounds(su, sv) {
            set_uv(
                &mut blocks,
                grid,
                su,
                y,
                sv,
                make_slab(&pal.pick(MaterialPalette::SLOT_FLOOR, seed, su, y, sv), "bottom"),
            );
            set_uv(&mut blocks, grid, su, y, sv, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, su, y, sv)));
        }
    }

    // Two Elevated Branch-Supported Treehouse Cabins linked by a Hanging Plank Skybridge!
    let c_off = ((grid.w - 8) / 4).clamp(2, 4);
    let cabin_centers = [((cu - c_off).max(3), cv - 1, 2), ((cu + c_off).min(grid.w - 4), cv + 1, 2)];
    for &(cx, cz, cr) in &cabin_centers {
        // Horizontal wooden branch supporting platform back to main trunk.
        for u in cx.min(cu)..=cx.max(cu) {
            set_uv(&mut blocks, grid, u, platform_y - 1, cz, make_beam_block_at(pal, "x", seed, u, platform_y - 1, cz));
        }
        let cab_top = (platform_y + 3).min(h - 3);
        for v in (cz - cr)..=(cz + cr) {
            for u in (cx - cr)..=(cx + cr) {
                set_uv(&mut blocks, grid, u, platform_y, v, PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, platform_y, v)));
                let wall = u == cx - cr || u == cx + cr || v == cz - cr || v == cz + cr;
                for y in (platform_y + 1)..=cab_top {
                    if wall && !(u == cx && v == cz + cr && y <= platform_y + 2) {
                        let win = y == platform_y + 2 && (u == cx || v == cz);
                        let blk = if win {
                            pal.pick(MaterialPalette::SLOT_WINDOW, seed, u, y, v)
                        } else {
                            pal.pick(MaterialPalette::SLOT_WALL, seed, u, y, v)
                        };
                        set_uv(&mut blocks, grid, u, y, v, PlannedBlock::simple(blk));
                    }
                }
                set_uv(&mut blocks, grid, u, cab_top + 1, v, make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, u, cab_top + 1, v), "bottom"));
            }
        }
        set_uv(&mut blocks, grid, cx, platform_y + 1, cz - 1, PlannedBlock::simple("minecraft:bookshelf"));
        set_uv(&mut blocks, grid, cx, cab_top, cz, make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, cx, cab_top, cz), true));
    }

    // Lush Multi-Sphere Canopy Crown (`persistent=true` leaves)!
    let canopy_r = ((grid.w.min(grid.d) - 2) / 2).clamp(4, 9);
    for y in (trunk_top - 2).max(platform_y + 2)..=(h - 1) {
        let dy = y - trunk_top;
        let r_at_y = (canopy_r - dy.abs()).max(2);
        for v in (cv - r_at_y)..=(cv + r_at_y) {
            for u in (cu - r_at_y)..=(cu + r_at_y) {
                let d2 = (u - cu) * (u - cu) + (v - cv) * (v - cv);
                if d2 <= r_at_y * r_at_y + 1 && !((u - cu).abs() <= 1 && (v - cv).abs() <= 1 && y <= trunk_top) {
                    set_uv(&mut blocks, grid, u, y, v, make_leaves(&leaf_id));
                }
            }
        }
    }

    let [lx, lz] = grid.to_local_xz(cu, cv + 3);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 1, lz]))
}

/// 10. Japanese Hot Spring Onsen Bathhouse with Steaming Pool, Waterfall Rockery & Timber Pavilion.
fn generate_onsen_bath(
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

    anchor_plinth(&mut blocks, grid, pal, seed, u0, u1, v0, v1, 0, terrain_offsets);

    // Garden Perimeter Slatted Privacy Fence + Stone Walkway + Pool Basin!
    let pool_u1 = (cu + 2).min(u1 - 4);
    let pv_pav_start = (pool_u1 + 2).min(u1 - 3);
    for v in v0..=v1 {
        for u in u0..=u1 {
            let rim = u == u0 || u == u1 || v == v0 || v == v1;
            let in_pool = u >= u0 + 2 && u <= pool_u1 - 1 && v >= v0 + 2 && v <= v1 - 2;
            if rim {
                let is_gate = v == v1 && (u - cu).abs() <= 1;
                if !is_gate {
                    for y in 1..=3 {
                        set_uv(
                            &mut blocks,
                            grid,
                            u,
                            y,
                            v,
                            PlannedBlock::simple(if y == 1 {
                                pal.pick(MaterialPalette::SLOT_FOUNDATION, seed, u, y, v)
                            } else {
                                "minecraft:bamboo_fence".to_owned()
                            }),
                        );
                    }
                }
            } else if in_pool {
                set_uv(&mut blocks, grid, u, 0, v, PlannedBlock::simple("minecraft:sea_lantern"));
                set_uv(&mut blocks, grid, u, 1, v, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));
            } else {
                set_uv(
                    &mut blocks,
                    grid,
                    u,
                    1,
                    v,
                    PlannedBlock::simple(pal.pick(MaterialPalette::SLOT_FLOOR, seed, u, 1, v)),
                );
            }
        }
    }

    // Back Boulder Waterfall pouring into Onsen Pool!
    for u in (u0 + 2)..=(u0 + 4).min(pool_u1) {
        for y in 1..=4 {
            set_uv(
                &mut blocks,
                grid,
                u,
                y,
                v0 + 1,
                PlannedBlock::simple("minecraft:mossy_cobblestone"),
            );
        }
    }
    set_uv(&mut blocks, grid, u0 + 3, 4, v0 + 1, PlannedBlock::with_props("minecraft:water", vec![("level", "0")]));

    // Side Timber Bathhouse Changing Pavilion with Pagoda Canopy!
    let pav_top = 5.min(h - 3);
    for v in (v0 + 1)..=(v1 - 1) {
        for u in pv_pav_start..=(u1 - 1) {
            let corner = (u == pv_pav_start || u == u1 - 1) && (v == v0 + 1 || v == v1 - 1);
            if corner {
                for y in 2..=pav_top {
                    set_uv(&mut blocks, grid, u, y, v, make_frame_block_at(pal, "y", seed, u, y, v));
                }
            }
            set_uv(
                &mut blocks,
                grid,
                u,
                pav_top + 1,
                v,
                make_slab(&pal.pick(MaterialPalette::SLOT_ROOF, seed, u, pav_top + 1, v), "bottom"),
            );
        }
    }
    let pav_mid_u = (pv_pav_start + u1 - 1) / 2;
    set_uv(&mut blocks, grid, pav_mid_u, 2, cv, PlannedBlock::simple("minecraft:barrel"));
    set_uv(
        &mut blocks,
        grid,
        pav_mid_u,
        pav_top,
        cv,
        make_light(pal.pick(MaterialPalette::SLOT_LIGHT, seed, pav_mid_u, pav_top, cv), true),
    );

    let [lx, lz] = grid.to_local_xz(cu, v1);
    finish_landmark(req, spec, grid, height, blocks, noise, Some([lx, 2, lz]))
}
