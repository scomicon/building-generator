//! Building Generator: procedural noise-driven building generator mod for Horizon (Minecraft 26.3).
pub mod client_ui;
mod gen_ancient;
mod gen_bridge;
mod gen_civic_transport;
mod gen_common;
mod gen_dungeon_estate;
mod gen_fantasy_folk;
mod gen_fortress;
mod gen_landmarks;
mod gen_modern_industrial;
mod gen_new_categories;
pub mod gen_preview;
mod gen_road;
mod gen_exterior;
mod gen_interior;
mod gen_roof;
mod gen_settlement;
mod gen_weathering;
pub mod generator;
pub mod model;
pub mod network;
pub mod noise;
pub mod parser;
pub mod persist;
pub mod server_builder;

use mod_api::{
    self, ClientHost, ClientTickPreEvent, Flow, Hooks, InputKeyEvent, MouseButtonPreEvent,
    RegisterCommandsEvent, Registrar, RegistrationContext, RenderGuiPostEvent, RunModCommandEvent,
    ScheduledTaskEvent, ScreenCharTypedPreEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, ServerHost,
    SubmitCustomGeometryEvent, TabCompleteEvent,
};

#[derive(Default)]
pub struct BuildingGeneratorMod;

#[mod_api::horizon_mod]
impl Hooks for BuildingGeneratorMod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        network::register_channels(r);
    }

    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        server_builder::register_commands(event);
    }

    fn run_mod_command(&self, host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
        server_builder::handle_command(host, event)
    }

    fn tab_complete(&self, _host: &mut dyn ServerHost, event: &mut TabCompleteEvent<'_>) {
        server_builder::tab_complete(event);
    }

    fn scheduled_task(&self, host: &mut dyn ServerHost, event: &mut ScheduledTaskEvent<'_>) {
        server_builder::process_scheduled_task(host, event);
    }

    fn input_key(&self, host: &mut dyn ClientHost, event: &mut InputKeyEvent<'_>) {
        client_ui::on_input_key(host, event);
    }

    fn mouse_button_pre(&self, host: &mut dyn ClientHost, event: &mut MouseButtonPreEvent) -> Flow {
        client_ui::on_mouse_button_pre(host, event)
    }

    fn client_tick_pre(&self, host: &mut dyn ClientHost, event: &mut ClientTickPreEvent) {
        client_ui::on_client_tick_pre(host, event);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        client_ui::on_screen_render_post(host, event);
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_mouse_pressed(host, event)
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_mouse_released(host, event)
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_mouse_dragged(host, event)
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_mouse_scrolled(host, event)
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_key_pressed(host, event)
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        client_ui::on_screen_char_typed(host, event)
    }

    fn render_gui_post(&self, host: &dyn ClientHost, event: &mut RenderGuiPostEvent<'_>) {
        client_ui::on_render_gui_post(host, event);
    }

    fn submit_custom_geometry(
        &self,
        host: &dyn ClientHost,
        event: &mut SubmitCustomGeometryEvent<'_>,
    ) {
        client_ui::on_submit_custom_geometry(host, event);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};

    use mod_api::__private::game::{BackendMutRefOf, BackendRefOf};
    use mod_api::prelude::*;
    use mod_api::testing::{FakeHost, with_mods};
    use mod_api::{
        Backend, BackendMut, BackendMutRef, BackendRef, BlockEdit, BlockKey, BlockMetadata,
        BlockState, BlockStateError, BlockStateRecord, ClientHost, CommandArgument,
        CommandArgumentValue, CommandInvocation, CommandSource, ConfigEntry, ConfigFileError,
        ConfigFiles, ConfigPath, ConfigStat, Flow, FluidKey, GameRuleReadError, GameRuleValue,
        Heightmap, Host, Hooks, ItemKey, ItemStack, LevelRef, MobTickCost, ModEntry,
        PermissionLevel, Registrar, RegistrationContext, RunModCommandEvent, ServerHost,
        ServerReloadResources, StateConstraint, decode_payload, encode_payload,
    };

    use crate::generator::generate_blueprint;
    use crate::model::{
        BuildCategory, BuildRequest, BuildStyle, CardinalFacing, MaterialPalette, NoiseLevel,
        RerollRequest, RoofStyle,
    };
    use crate::noise::NoiseField;
    use crate::parser::{StructureTheme, infer_palette_from_text, parse_description};
    use crate::server_builder::{
        CLI_PRESETS, blueprint_to_phased_runs, execute_build, execute_redo, execute_reroll,
        execute_undo, find_cli_preset, handle_command,
    };
    use crate::BuildingGeneratorMod;

    fn sample_request(desc: &str, seed: i64) -> BuildRequest {
        BuildRequest {
            dimension: "minecraft:overworld".into(),
            origin_x: 10,
            origin_y: 64,
            origin_z: 20,
            width: 13,
            depth: 11,
            height: 12,
            category: BuildCategory::House,
            style: BuildStyle::Auto,
            facing: CardinalFacing::South,
            noise_level: NoiseLevel::Balanced,
            seed,
            description: desc.into(),
            adapt_terrain: true,
            air_only: false,
            wear_pct: 0,
            overgrowth_pct: 0,
            biome_hint: String::new(),
            palette: MaterialPalette::preset(0),
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 4,
        }
    }

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let a = NoiseField::new(999);
        let b = NoiseField::new(999);
        let c = NoiseField::new(1000);
        let mut diff_count = 0;
        for x in -10..10 {
            for z in -10..10 {
                let va = a.fbm_2d(x as f64 * 0.23, z as f64 * 0.23, 3);
                let vb = b.fbm_2d(x as f64 * 0.23, z as f64 * 0.23, 3);
                let vc = c.fbm_2d(x as f64 * 0.23, z as f64 * 0.23, 3);
                assert!((0.0..=1.0).contains(&va));
                assert_eq!(va.to_bits(), vb.to_bits());
                if (va - vc).abs() > 1e-6 {
                    diff_count += 1;
                }
            }
        }
        assert!(diff_count > 100);
    }

    #[test]
    fn parser_extracts_ru_and_en_architectural_features() {
        let s1 = parse_description(
            BuildCategory::House,
            BuildStyle::Auto,
            "уютный двухэтажный дом с крыльцом, балконом, камином и высокой крышей",
            15,
            13,
            14,
            NoiseLevel::Balanced,
            42,
        );
        assert_eq!(s1.category, BuildCategory::House);
        assert_eq!(s1.floors, 2);
        assert!(s1.porch);
        assert!(s1.balcony);
        assert!(s1.chimney);

        let s2 = parse_description(
            BuildCategory::House,
            BuildStyle::Auto,
            "modern 2 story house with flat roof and panoramic windows",
            15,
            13,
            14,
            NoiseLevel::Calm,
            42,
        );
        assert_eq!(s2.roof_style, RoofStyle::Flat);
        assert!(s2.symmetric_modern);
        assert!(s2.wide_windows);

        let pal =
            infer_palette_from_text("мрачный готический дом из глубокого сланца с синим огнем");
        assert!(pal.foundation.contains("deepslate"));
        assert_eq!(pal.light, "minecraft:soul_lantern");
    }

    #[test]
    fn generator_produces_valid_walkable_house_across_all_facings() {
        for facing in [
            CardinalFacing::North,
            CardinalFacing::East,
            CardinalFacing::South,
            CardinalFacing::West,
        ] {
            let mut req = sample_request("двухэтажный дом с крыльцом и балконом", 12345);
            req.facing = facing;
            let bp = generate_blueprint(&req, None);
            assert!(bp.non_air_count() > 150, "expected solid house blocks");
            let door = bp.door_pos.expect("house must always place an entrance door");
            let lower = bp.blocks.get(&door).expect("lower door block");
            let upper = bp
                .blocks
                .get(&[door[0], door[1] + 1, door[2]])
                .expect("upper door block");
            assert_eq!(lower.block_id, req.palette.door);
            assert_eq!(upper.block_id, req.palette.door);
            assert!(lower.props.contains(&("half", "lower")));
            assert!(upper.props.contains(&("half", "upper")));
            assert!(lower.props.contains(&("facing", facing.as_mc())));

            // All placed local XZ coordinates must stay inside [0..W, 0..D].
            for &[lx, _ly, lz] in bp.blocks.keys() {
                assert!((0..i32::from(bp.width)).contains(&lx));
                assert!((0..i32::from(bp.depth)).contains(&lz));
            }
        }
    }

    #[test]
    fn generator_produces_distinct_shapes_for_different_seeds_and_categories() {
        let mut seen_blocks = std::collections::HashSet::new();
        let mut seen_footprints = std::collections::HashSet::new();
        for seed in [101i64, 202, 303, 404, 505, 606] {
            let r = sample_request("уютный дом", seed);
            let bp = generate_blueprint(&r, None);
            let fp: Vec<[i32; 2]> = bp
                .blocks
                .iter()
                .filter(|(_, b)| b.block_id != "minecraft:air")
                .map(|(&[lx, _, lz], _)| [lx, lz])
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            seen_footprints.insert(fp);
            let sig: Vec<([i32; 3], String)> = bp
                .blocks
                .iter()
                .map(|(&pos, b)| (pos, b.block_id.clone()))
                .collect();
            seen_blocks.insert(sig);
        }
        assert!(
            seen_blocks.len() >= 5,
            "expected varied buildings across 6 seeds, got {} unique",
            seen_blocks.len()
        );
        assert!(
            seen_footprints.len() >= 3,
            "expected distinct 2D footprints on default 13x11 house, got {}",
            seen_footprints.len()
        );

        for cat in BuildCategory::ALL {
            let mut r = sample_request("красивая постройка", 777);
            r.category = cat;
            let bp = generate_blueprint(&r, None);
            assert!(bp.non_air_count() > 60, "category {:?} too empty", cat);
        }

        let intact =
            generate_blueprint(&sample_request("каменный дом с высокой крышей", 55), None);
        let ruined =
            generate_blueprint(&sample_request("заброшенные руины каменного дома", 55), None);
        assert!(ruined.non_air_count() < intact.non_air_count());
    }

    #[test]
    fn payload_codec_roundtrip_preserves_request_and_response() {
        let req = sample_request("дом из кирпича и березы", 8888);
        let bytes = encode_payload(&req, 65_536).expect("encode BuildRequest");
        let decoded: BuildRequest = decode_payload(&bytes).expect("decode BuildRequest");
        assert_eq!(req, decoded);

        let resp = crate::model::BuildResponse {
            ok: true,
            blocks_changed: 412,
            message: "Готово!".into(),
        };
        let rbytes = encode_payload(&resp, 8192).expect("encode BuildResponse");
        let rdecoded: crate::model::BuildResponse = decode_payload(&rbytes).expect("decode");
        assert_eq!(resp, rdecoded);
    }

    const TEST_BLOCK_IDS: &[&str] = &[
        "minecraft:air",
        "minecraft:grass_block",
        "minecraft:dirt",
        "minecraft:dirt_path",
        "minecraft:stone",
        "minecraft:cobblestone",
        "minecraft:cobblestone_wall",
        "minecraft:stone_bricks",
        "minecraft:stone_brick_wall",
        "minecraft:stone_brick_stairs",
        "minecraft:stone_brick_slab",
        "minecraft:mossy_stone_bricks",
        "minecraft:mossy_stone_brick_wall",
        "minecraft:cracked_stone_bricks",
        "minecraft:deepslate_bricks",
        "minecraft:deepslate_brick_wall",
        "minecraft:deepslate_brick_stairs",
        "minecraft:deepslate_brick_slab",
        "minecraft:polished_deepslate",
        "minecraft:polished_deepslate_wall",
        "minecraft:cobbled_deepslate",
        "minecraft:polished_blackstone_bricks",
        "minecraft:polished_blackstone_brick_wall",
        "minecraft:polished_blackstone_brick_stairs",
        "minecraft:polished_blackstone_brick_slab",
        "minecraft:tuff_bricks",
        "minecraft:tuff_brick_wall",
        "minecraft:tuff_brick_stairs",
        "minecraft:tuff_brick_slab",
        "minecraft:bricks",
        "minecraft:brick_wall",
        "minecraft:brick_stairs",
        "minecraft:brick_slab",
        "minecraft:sandstone",
        "minecraft:sandstone_wall",
        "minecraft:sandstone_stairs",
        "minecraft:sandstone_slab",
        "minecraft:cut_sandstone",
        "minecraft:smooth_sandstone",
        "minecraft:terracotta",
        "minecraft:white_terracotta",
        "minecraft:white_concrete",
        "minecraft:light_gray_concrete",
        "minecraft:gray_concrete",
        "minecraft:black_concrete",
        "minecraft:quartz_block",
        "minecraft:smooth_quartz",
        "minecraft:smooth_quartz_stairs",
        "minecraft:smooth_quartz_slab",
        "minecraft:cut_copper",
        "minecraft:cut_copper_stairs",
        "minecraft:cut_copper_slab",
        "minecraft:oxidized_copper",
        "minecraft:oxidized_cut_copper_stairs",
        "minecraft:oxidized_cut_copper_slab",
        "minecraft:copper_bulb",
        "minecraft:copper_grate",
        "minecraft:calcite",
        "minecraft:tuff",
        "minecraft:moss_block",
        "minecraft:prismarine_bricks",
        "minecraft:dark_prismarine",
        "minecraft:oak_planks",
        "minecraft:spruce_planks",
        "minecraft:dark_oak_planks",
        "minecraft:birch_planks",
        "minecraft:cherry_planks",
        "minecraft:pale_oak_planks",
        "minecraft:bamboo_planks",
        "minecraft:bamboo_mosaic",
        "minecraft:acacia_planks",
        "minecraft:crimson_planks",
        "minecraft:warped_planks",
        "minecraft:oak_log",
        "minecraft:stripped_oak_log",
        "minecraft:stripped_oak_wood",
        "minecraft:spruce_log",
        "minecraft:stripped_spruce_log",
        "minecraft:dark_oak_log",
        "minecraft:stripped_dark_oak_log",
        "minecraft:birch_log",
        "minecraft:cherry_log",
        "minecraft:stripped_cherry_log",
        "minecraft:pale_oak_log",
        "minecraft:stripped_pale_oak_log",
        "minecraft:bamboo_block",
        "minecraft:stripped_bamboo_block",
        "minecraft:oak_stairs",
        "minecraft:oak_slab",
        "minecraft:spruce_stairs",
        "minecraft:spruce_slab",
        "minecraft:dark_oak_stairs",
        "minecraft:dark_oak_slab",
        "minecraft:birch_stairs",
        "minecraft:birch_slab",
        "minecraft:cherry_stairs",
        "minecraft:cherry_slab",
        "minecraft:pale_oak_stairs",
        "minecraft:pale_oak_slab",
        "minecraft:bamboo_stairs",
        "minecraft:bamboo_slab",
        "minecraft:oak_door",
        "minecraft:spruce_door",
        "minecraft:dark_oak_door",
        "minecraft:birch_door",
        "minecraft:cherry_door",
        "minecraft:pale_oak_door",
        "minecraft:bamboo_door",
        "minecraft:acacia_door",
        "minecraft:iron_door",
        "minecraft:copper_door",
        "minecraft:oak_trapdoor",
        "minecraft:spruce_trapdoor",
        "minecraft:dark_oak_trapdoor",
        "minecraft:birch_trapdoor",
        "minecraft:cherry_trapdoor",
        "minecraft:pale_oak_trapdoor",
        "minecraft:bamboo_trapdoor",
        "minecraft:acacia_trapdoor",
        "minecraft:copper_trapdoor",
        "minecraft:iron_trapdoor",
        "minecraft:oak_fence",
        "minecraft:spruce_fence",
        "minecraft:dark_oak_fence",
        "minecraft:birch_fence",
        "minecraft:cherry_fence",
        "minecraft:pale_oak_fence",
        "minecraft:bamboo_fence",
        "minecraft:nether_brick_fence",
        "minecraft:oak_leaves",
        "minecraft:spruce_leaves",
        "minecraft:birch_leaves",
        "minecraft:cherry_leaves",
        "minecraft:azalea_leaves",
        "minecraft:flowering_azalea_leaves",
        "minecraft:pale_oak_leaves",
        "minecraft:glass",
        "minecraft:glass_pane",
        "minecraft:white_stained_glass_pane",
        "minecraft:gray_stained_glass_pane",
        "minecraft:black_stained_glass_pane",
        "minecraft:magenta_stained_glass_pane",
        "minecraft:purple_stained_glass_pane",
        "minecraft:blue_stained_glass_pane",
        "minecraft:iron_bars",
        "minecraft:lantern",
        "minecraft:soul_lantern",
        "minecraft:copper_lantern",
        "minecraft:torch",
        "minecraft:sea_lantern",
        "minecraft:glowstone",
        "minecraft:end_rod",
        "minecraft:shroomlight",
        "minecraft:ochre_froglight",
        "minecraft:iron_chain",
        "minecraft:campfire",
        "minecraft:soul_campfire",
        "minecraft:potted_oak_sapling",
        "minecraft:potted_fern",
        "minecraft:potted_cherry_sapling",
        "minecraft:potted_red_tulip",
        "minecraft:flower_pot",
        "minecraft:red_bed",
        "minecraft:white_bed",
        "minecraft:blue_bed",
        "minecraft:red_carpet",
        "minecraft:white_carpet",
        "minecraft:moss_carpet",
        "minecraft:crafting_table",
        "minecraft:furnace",
        "minecraft:smoker",
        "minecraft:blast_furnace",
        "minecraft:anvil",
        "minecraft:cauldron",
        "minecraft:water_cauldron",
        "minecraft:barrel",
        "minecraft:chest",
        "minecraft:bookshelf",
        "minecraft:lectern",
        "minecraft:enchanting_table",
        "minecraft:decorated_pot",
        "minecraft:hay_block",
        "minecraft:cobblestone_slab",
        "minecraft:cobblestone_stairs",
        "minecraft:mossy_cobblestone",
        "minecraft:redstone_lamp",
        "minecraft:ladder",
        "minecraft:white_wool",
        "minecraft:red_wool",
        "minecraft:snow_block",
        "minecraft:packed_ice",
        "minecraft:chiseled_bookshelf",
        "minecraft:composter",
        "minecraft:lightning_rod",
        "minecraft:potted_azalea_bush",
        "minecraft:bell",
        "minecraft:magma_block",
        "minecraft:water",
        "minecraft:oak_pressure_plate",
        "minecraft:farmland",
        "minecraft:wheat",
        "minecraft:candle",
        "minecraft:wall_torch",
        "minecraft:soul_wall_torch",
        "minecraft:oak_fence_gate",
        "minecraft:spruce_fence_gate",
        "minecraft:dark_oak_fence_gate",
        "minecraft:birch_fence_gate",
        "minecraft:cherry_fence_gate",
        "minecraft:pale_oak_fence_gate",
        "minecraft:acacia_fence_gate",
        "minecraft:bamboo_fence_gate",
        "minecraft:diorite",
        "minecraft:orange_wool",
        "minecraft:blue_wool",
        "minecraft:orange_carpet",
        "minecraft:azalea",
        "minecraft:flowering_azalea",
        "minecraft:bamboo",
        "minecraft:potted_bamboo",
        "minecraft:potted_flowering_azalea_bush",
        "minecraft:spore_blossom",
        "minecraft:fern",
        "minecraft:cobbled_deepslate_stairs",
        "minecraft:cracked_deepslate_tiles",
        "minecraft:spruce_shelf",
        "minecraft:oak_shelf",
        "minecraft:lily_pad",
        "minecraft:sugar_cane",
        "minecraft:gravel",
        "minecraft:vine",
        "minecraft:pink_petals",
        "minecraft:wildflowers",
        "minecraft:leaf_litter",
        "minecraft:short_grass",
        "minecraft:lime_carpet",
        "minecraft:brewing_stand",
        "minecraft:grindstone",
        "minecraft:smithing_table",
        "minecraft:oak_hanging_sign",
        "minecraft:spruce_hanging_sign",
        "minecraft:dark_oak_hanging_sign",
        "minecraft:cherry_hanging_sign",
        "minecraft:cherry_log",
        "minecraft:jungle_planks",
        "minecraft:jungle_log",
        "minecraft:stripped_jungle_log",
        "minecraft:jungle_door",
        "minecraft:jungle_trapdoor",
        "minecraft:jungle_fence",
        "minecraft:jungle_fence_gate",
        "minecraft:jungle_leaves",
        "minecraft:jungle_stairs",
        "minecraft:jungle_slab",
        "minecraft:mangrove_planks",
        "minecraft:mangrove_log",
        "minecraft:stripped_mangrove_log",
        "minecraft:mangrove_door",
        "minecraft:mangrove_trapdoor",
        "minecraft:mangrove_fence",
        "minecraft:mangrove_fence_gate",
        "minecraft:mangrove_leaves",
        "minecraft:mangrove_stairs",
        "minecraft:mangrove_slab",
        "minecraft:acacia_log",
        "minecraft:stripped_acacia_log",
        "minecraft:acacia_fence",
        "minecraft:acacia_leaves",
        "minecraft:acacia_stairs",
        "minecraft:acacia_slab",
        "minecraft:crimson_stem",
        "minecraft:crimson_door",
        "minecraft:crimson_trapdoor",
        "minecraft:crimson_fence",
        "minecraft:crimson_fence_gate",
        "minecraft:crimson_stairs",
        "minecraft:crimson_slab",
        "minecraft:warped_stem",
        "minecraft:warped_door",
        "minecraft:warped_trapdoor",
        "minecraft:warped_fence",
        "minecraft:warped_fence_gate",
        "minecraft:warped_stairs",
        "minecraft:warped_slab",
        "minecraft:dark_oak_leaves",
        "minecraft:mud_bricks",
        "minecraft:mud_brick_stairs",
        "minecraft:mud_brick_slab",
        "minecraft:mud_brick_wall",
        "minecraft:resin_bricks",
        "minecraft:resin_brick_stairs",
        "minecraft:resin_brick_slab",
        "minecraft:resin_brick_wall",
        "minecraft:chiseled_tuff_bricks",
        "minecraft:polished_tuff",
        "minecraft:red_nether_bricks",
        "minecraft:red_nether_brick_stairs",
        "minecraft:red_nether_brick_slab",
        "minecraft:red_nether_brick_wall",
        "minecraft:nether_bricks",
        "minecraft:nether_brick_stairs",
        "minecraft:nether_brick_slab",
        "minecraft:nether_brick_wall",
        "minecraft:purpur_block",
        "minecraft:purpur_pillar",
        "minecraft:purpur_stairs",
        "minecraft:purpur_slab",
        "minecraft:end_stone_bricks",
        "minecraft:end_stone_brick_stairs",
        "minecraft:end_stone_brick_slab",
        "minecraft:end_stone_brick_wall",
        "minecraft:amethyst_block",
        "minecraft:quartz_bricks",
        "minecraft:quartz_pillar",
        "minecraft:polished_basalt",
        "minecraft:polished_andesite",
        "minecraft:polished_diorite",
        "minecraft:polished_granite",
        "minecraft:andesite_wall",
        "minecraft:diorite_wall",
        "minecraft:granite_wall",
        "minecraft:mossy_cobblestone_wall",
        "minecraft:red_sandstone",
        "minecraft:cut_red_sandstone",
        "minecraft:red_sandstone_stairs",
        "minecraft:red_sandstone_slab",
        "minecraft:red_sandstone_wall",
        "minecraft:dark_prismarine_stairs",
        "minecraft:dark_prismarine_slab",
        "minecraft:prismarine_wall",
        "minecraft:exposed_cut_copper",
        "minecraft:exposed_cut_copper_stairs",
        "minecraft:exposed_cut_copper_slab",
        "minecraft:weathered_cut_copper",
        "minecraft:weathered_cut_copper_stairs",
        "minecraft:weathered_cut_copper_slab",
        "minecraft:oxidized_cut_copper",
        "minecraft:copper_block",
        "minecraft:tinted_glass",
        "minecraft:light_blue_stained_glass_pane",
        "minecraft:cyan_stained_glass_pane",
        "minecraft:pink_stained_glass_pane",
        "minecraft:red_stained_glass_pane",
        "minecraft:orange_stained_glass_pane",
        "minecraft:yellow_stained_glass_pane",
        "minecraft:lime_stained_glass_pane",
        "minecraft:green_stained_glass_pane",
        "minecraft:brown_stained_glass_pane",
        "minecraft:verdant_froglight",
        "minecraft:pearlescent_froglight",
        "minecraft:blackstone",
        "minecraft:cartography_table",
        "minecraft:loom",
        "minecraft:stonecutter",
        "minecraft:lava_cauldron",
        "minecraft:cake",
        "minecraft:sea_pickle",
        "minecraft:jukebox",
        "minecraft:ender_chest",
        "minecraft:beacon",
        "minecraft:light_blue_bed",
        "minecraft:green_bed",
        "minecraft:purple_bed",
        "minecraft:yellow_bed",
        "minecraft:dark_oak_shelf",
        "minecraft:cherry_shelf",
        "minecraft:birch_shelf",
        "minecraft:pale_oak_shelf",
        "minecraft:sand",
        "minecraft:red_sand",
        "minecraft:cobweb",
        "minecraft:cracked_deepslate_bricks",
        "minecraft:cracked_nether_bricks",
        "minecraft:cracked_polished_blackstone_bricks",
        "minecraft:mossy_stone_brick_stairs",
        "minecraft:mossy_stone_brick_slab",
        "minecraft:mossy_cobblestone_stairs",
        "minecraft:mossy_cobblestone_slab",
        "minecraft:stone_brick_slab",
        "minecraft:cobblestone_stairs",
        "minecraft:cobblestone_slab",
        "minecraft:packed_mud",
        "minecraft:dead_bush",
        "minecraft:pale_moss_carpet",
        "minecraft:pale_moss_block",
        "minecraft:red_mushroom_block",
        "minecraft:brown_mushroom_block",
        "minecraft:cobbled_deepslate_slab",
        "minecraft:chiseled_sandstone",
        "minecraft:carrots",
        "minecraft:potatoes",
        "minecraft:beetroots",
        "minecraft:iron_ore",
        "minecraft:gold_ore",
        "minecraft:coal_ore",
        "minecraft:copper_ore",
        "minecraft:deepslate_iron_ore",
        "minecraft:deepslate_gold_ore",
        "minecraft:deepslate_coal_ore",
        "minecraft:deepslate_redstone_ore",
        "minecraft:deepslate_diamond_ore",
        "minecraft:lava",
        "minecraft:coarse_dirt",
        "minecraft:stone_pressure_plate",
        "minecraft:gilded_blackstone",
        "minecraft:jack_o_lantern",
        "minecraft:daylight_detector",
        "minecraft:tripwire_hook",
        "minecraft:dispenser",
        "minecraft:lever",
        "minecraft:heavy_weighted_pressure_plate",
        "minecraft:spruce_pressure_plate",
        "minecraft:dark_oak_pressure_plate",
        "minecraft:birch_pressure_plate",
        "minecraft:cherry_pressure_plate",
        "minecraft:bamboo_pressure_plate",
        "minecraft:pale_oak_pressure_plate",
        "minecraft:oak_sign",
        "minecraft:oak_wall_sign",
        "minecraft:dark_oak_wall_sign",
        "minecraft:birch_wall_sign",
        "minecraft:spruce_wall_sign",
        "minecraft:cherry_wall_sign",
        "minecraft:red_wall_banner",
        "minecraft:red_banner",
        "minecraft:blue_wall_banner",
        "minecraft:blue_banner",
        "minecraft:white_wall_banner",
        "minecraft:green_wall_banner",
        "minecraft:green_banner",
        "minecraft:gray_wall_banner",
        "minecraft:gray_banner",
        "minecraft:purple_wall_banner",
        "minecraft:cyan_wall_banner",
        "minecraft:black_wall_banner",
        "minecraft:brown_wall_banner",
        "minecraft:rail",
        "minecraft:powered_rail",
        "minecraft:redstone_block",
        "minecraft:beehive",
        "minecraft:sweet_berry_bush",
        "minecraft:melon",
        "minecraft:oxeye_daisy",
        "minecraft:cornflower",
        "minecraft:rose_bush",
        "minecraft:peony",
        "minecraft:lilac",
        "minecraft:sunflower",
        "minecraft:allium",
        "minecraft:blue_orchid",
        "minecraft:lily_of_the_valley",
        "minecraft:hopper",
        "minecraft:cracked_bricks",
        "minecraft:andesite",
        "minecraft:smooth_stone",
        "minecraft:smooth_stone_slab",
        "minecraft:deepslate_tiles",
        "minecraft:quartz_slab",
        "minecraft:mud",
        "minecraft:podzol",
        "minecraft:mycelium",
        "minecraft:iron_block",
        "minecraft:scaffolding",
        "minecraft:amethyst_cluster",
    ];

    static REGISTERED_KEYS: OnceLock<Mutex<HashMap<String, BlockKey>>> = OnceLock::new();

    struct TestRegistryMod;
    #[mod_api::__subscriptions]
    impl Hooks for TestRegistryMod {
        fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
            let map = REGISTERED_KEYS.get_or_init(|| Mutex::new(HashMap::new()));
            let mut guard = map.lock().unwrap();
            for &id in TEST_BLOCK_IDS {
                guard.insert(id.to_owned(), r.block_key(id));
            }
        }
    }

    /// Test double combining `FakeHost` world/player state with in-memory `WorldEditServerHost` & `BlockMetadata`.
    #[derive(Clone, Default)]
    struct MemWorldFiles {
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    }

    impl ConfigFiles for MemWorldFiles {
        fn read(&self, file: &ConfigPath) -> Result<Vec<u8>, ConfigFileError> {
            self.files
                .lock()
                .unwrap()
                .get(file.as_str())
                .cloned()
                .ok_or(ConfigFileError::NotFound)
        }

        fn write(&self, file: &ConfigPath, bytes: &[u8]) -> Result<(), ConfigFileError> {
            self.files
                .lock()
                .unwrap()
                .insert(file.as_str().to_owned(), bytes.to_vec());
            Ok(())
        }

        fn list(
            &self,
            _folder: &ConfigPath,
            _after: Option<&str>,
            _limit: u32,
        ) -> Result<Vec<ConfigEntry>, ConfigFileError> {
            Ok(Vec::new())
        }

        fn stat(&self, _path: &ConfigPath) -> Result<Option<ConfigStat>, ConfigFileError> {
            Ok(None)
        }
    }

    struct TestWorldHost {
        fake: FakeHost,
        key_to_id: HashMap<BlockKey, String>,
        id_to_key: HashMap<String, BlockKey>,
        world_blocks: BTreeMap<[i32; 3], BlockState>,
        air_key: BlockKey,
        sent_payloads: Vec<(String, Vec<u8>)>,
        player_perms: HashMap<u128, u8>,
        biome_id: Option<String>,
        world_dir_path: Option<PathBuf>,
        world_files: MemWorldFiles,
        summoned_entities: Vec<(u32, String, [f64; 3], mod_api::Compound, bool)>,
        hanging_dirs: HashMap<u32, mod_api::Direction>,
        next_entity_id: u32,
    }

    impl TestWorldHost {
        fn new(keys: HashMap<String, BlockKey>) -> Self {
            let mut key_to_id = HashMap::new();
            for (id, &k) in &keys {
                key_to_id.insert(k, id.clone());
            }
            let air_key = keys["minecraft:air"];
            let grass_key = keys["minecraft:grass_block"];
            let mut host = Self {
                fake: FakeHost::new(),
                key_to_id,
                id_to_key: keys,
                world_blocks: BTreeMap::new(),
                air_key,
                sent_payloads: Vec::new(),
                player_perms: HashMap::new(),
                biome_id: None,
                world_dir_path: None,
                world_files: MemWorldFiles::default(),
                summoned_entities: Vec::new(),
                hanging_dirs: HashMap::new(),
                next_entity_id: 1000,
            };
            let grass = BlockStateRecord::new(grass_key, 0).into();
            for x in 0..40 {
                for z in 0..40 {
                    host.world_blocks.insert([x, 63, z], grass);
                }
            }
            host
        }

        fn set_player_perm(&mut self, uuid: u128, level: u8) {
            self.player_perms.insert(uuid, level);
        }

        fn set_biome(&mut self, biome: Option<&str>) {
            self.biome_id = biome.map(str::to_owned);
        }
    }

    impl BlockMetadata for TestWorldHost {
        fn name(&self, block: BlockKey) -> Result<&str, BlockStateError> {
            self.key_to_id
                .get(&block)
                .map(String::as_str)
                .ok_or(BlockStateError::Unavailable)
        }
        fn property(
            &self,
            _: BlockStateRecord,
            _: &str,
        ) -> Result<Option<&str>, BlockStateError> {
            Ok(None)
        }
        fn with(
            &self,
            state: BlockStateRecord,
            _: &str,
            _: &str,
        ) -> Result<BlockStateRecord, BlockStateError> {
            Ok(state)
        }
        fn of(
            &self,
            block: BlockKey,
            _: &[StateConstraint],
        ) -> Result<BlockStateRecord, BlockStateError> {
            if self.key_to_id.contains_key(&block) {
                Ok(BlockStateRecord::new(block, 0))
            } else {
                Err(BlockStateError::Unavailable)
            }
        }
    }

    impl Host for TestWorldHost {
        fn backend(&self) -> Option<BackendRef<'_>> {
            Some(<BackendRef<'_> as BackendRefOf>::of(self))
        }
    }
    mod_api::impl_host_areas!(TestWorldHost, except(entities, keys, payload_tokens, players, block_entities, block_states));

    impl BlockStatesHost for TestWorldHost {
        fn lit_block_name(&self, pos: [i32; 3]) -> Option<BlockKey> {
            <FakeHost as BlockStatesHost>::lit_block_name(&self.fake, pos)
        }
        fn fluid_key(&self, id: &str) -> Option<FluidKey> {
            <FakeHost as BlockStatesHost>::fluid_key(&self.fake, id)
        }
        fn block_state_id(&self, name: &str) -> Option<BlockState> {
            let base = name.split('[').next()?;
            let key = *self.id_to_key.get(base)?;
            Some(BlockStateRecord::new(key, 0).into())
        }
        fn block_state_string(&self, state: BlockState) -> Option<String> {
            self.key_to_id.get(&state.block()).cloned()
        }
        fn block_state_default(&self, block: BlockKey) -> Option<BlockState> {
            if self.key_to_id.contains_key(&block) {
                Some(BlockStateRecord::new(block, 0).into())
            } else {
                None
            }
        }
        fn item_crafting_remainder(&self, stack: &ItemStack) -> Option<ItemStack> {
            <FakeHost as BlockStatesHost>::item_crafting_remainder(&self.fake, stack)
        }
        fn block_item(&self, block: BlockKey) -> Option<ItemKey> {
            <FakeHost as BlockStatesHost>::block_item(&self.fake, block)
        }
        fn item_block(&self, item: ItemKey) -> Option<BlockKey> {
            <FakeHost as BlockStatesHost>::item_block(&self.fake, item)
        }
    }

    impl PayloadTokensHost for TestWorldHost {
        fn check_payload_token(
            &self,
            _: &mod_api::PayloadToken,
        ) -> Result<(), mod_api::PayloadSendError> {
            Ok(())
        }
    }

    impl KeysHost for TestWorldHost {
        fn block_key(&self, id: &str) -> Option<BlockKey> {
            self.id_to_key.get(id).copied()
        }
    }
    impl EntitiesHost for TestWorldHost {
        fn entity(&self, id: u32) -> Option<mod_api::EntityRef> {
            let alive = self
                .summoned_entities
                .iter()
                .any(|(eid, _, _, _, ok)| *eid == id && *ok);
            if !alive {
                return None;
            }
            let overworld = self.fake.level("minecraft:overworld")?;
            Some(mod_api::EntityRef::new(overworld.epoch(), id))
        }
        fn level(&self, dimension: &str) -> Option<LevelRef> {
            self.fake.level(dimension)
        }
    }
    impl PlayersHost for TestWorldHost {
        fn player(&self, uuid: u128) -> Option<mod_api::PlayerRef> {
            self.fake.player(uuid)
        }
    }
    impl BlockEntitiesHost for TestWorldHost {
        fn block_entity(
            &self,
            _level: LevelRef,
            _pos: [i32; 3],
        ) -> Option<mod_api::BlockEntityRef> {
            None
        }

        fn block_entity_identity(
            &self,
            _level: LevelRef,
            _pos: [i32; 3],
        ) -> Option<mod_api::BlockEntityRef> {
            None
        }
    }
    impl BlockEntitiesServerHost for TestWorldHost {
        fn block_entity_sync(&mut self, _pos: [i32; 3]) {}
    }

    impl ServerHost for TestWorldHost {
        fn backend_mut(&mut self) -> Option<BackendMutRef<'_>> {
            Some(<BackendMutRef<'_> as BackendMutRefOf>::of(self))
        }
    }
    mod_api::impl_server_host_areas!(TestWorldHost, except(world_edit, services, block_entities, config, entities));

    impl EntitiesServerHost for TestWorldHost {
        fn summon(
            &mut self,
            _dimension: &str,
            kind: &str,
            pos: [f64; 3],
            nbt: mod_api::Compound,
        ) -> Option<u32> {
            let id = self.next_entity_id;
            self.next_entity_id += 1;
            self.summoned_entities
                .push((id, kind.to_owned(), pos, nbt, true));
            Some(id)
        }
    }

    impl ConfigServerHost for TestWorldHost {
        fn game_tests_enabled(&self) -> bool {
            <FakeHost as ConfigServerHost>::game_tests_enabled(&self.fake)
        }
        fn entity_range_percent(&self) -> i32 {
            <FakeHost as ConfigServerHost>::entity_range_percent(&self.fake)
        }
        fn tick_stats(&self) -> (f32, f64) {
            <FakeHost as ConfigServerHost>::tick_stats(&self.fake)
        }
        fn tick_times(&self) -> Vec<u64> {
            <FakeHost as ConfigServerHost>::tick_times(&self.fake)
        }
        fn mob_cost_sampling(&mut self, on: bool) -> bool {
            <FakeHost as ConfigServerHost>::mob_cost_sampling(&mut self.fake, on)
        }
        fn take_mob_costs(&mut self) -> Vec<MobTickCost> {
            <FakeHost as ConfigServerHost>::take_mob_costs(&mut self.fake)
        }
        fn count_packets(&mut self, on: bool) -> bool {
            <FakeHost as ConfigServerHost>::count_packets(&mut self.fake, on)
        }
        fn server_view_distance(&self) -> Option<i32> {
            <FakeHost as ConfigServerHost>::server_view_distance(&self.fake)
        }
        fn server_simulation_distance(&self) -> Option<i32> {
            <FakeHost as ConfigServerHost>::server_simulation_distance(&self.fake)
        }
        fn server_set_view_distance(&mut self, distance: i32) -> bool {
            <FakeHost as ConfigServerHost>::server_set_view_distance(&mut self.fake, distance)
        }
        fn server_set_simulation_distance(&mut self, distance: i32) -> bool {
            <FakeHost as ConfigServerHost>::server_set_simulation_distance(&mut self.fake, distance)
        }
        fn set_tick_rate(&mut self, rate: f32) -> bool {
            <FakeHost as ConfigServerHost>::set_tick_rate(&mut self.fake, rate)
        }
        fn world_dir(&self) -> Option<PathBuf> {
            self.world_dir_path.clone()
        }
        fn world_data_files(&self) -> &dyn ConfigFiles {
            &self.world_files
        }
        fn game_rules(&self) -> Vec<(String, GameRuleValue)> {
            <FakeHost as ConfigServerHost>::game_rules(&self.fake)
        }
        fn game_rule(&self, id: &str) -> Result<GameRuleValue, GameRuleReadError> {
            <FakeHost as ConfigServerHost>::game_rule(&self.fake, id)
        }
        fn reload_resources(&self) -> &dyn ServerReloadResources {
            <FakeHost as ConfigServerHost>::reload_resources(&self.fake)
        }
    }

    impl ServicesServerHost for TestWorldHost {
        fn commands(&mut self) -> &mut dyn mod_api::Commands {
            &mut self.fake.commands
        }
        fn messages(&self) -> &dyn mod_api::Messages {
            &self.fake.messages
        }
        fn players(&self) -> &dyn mod_api::Players {
            &self.fake.players
        }
    }

    impl WorldEditServerHost for TestWorldHost {
        fn edit_blocks(&mut self, dimension: &str, edit: &BlockEdit) -> Result<u64, String> {
            if dimension != "minecraft:overworld" {
                return Err("unknown dimension".into());
            }
            let mut changed = 0u64;
            for run in &edit.runs {
                let state = edit.palette[run.block as usize];
                for dy in 0..(run.height as i32) {
                    let pos = [run.x, run.y + dy, run.z];
                    if state.block() == self.air_key {
                        if self.world_blocks.remove(&pos).is_some() {
                            changed += 1;
                        }
                    } else {
                        self.world_blocks.insert(pos, state);
                        changed += 1;
                    }
                }
            }
            Ok(changed)
        }

        fn surface_heights(
            &self,
            _dimension: &str,
            columns: &[(i32, i32)],
            _heightmap: Heightmap,
        ) -> Result<Vec<Option<i32>>, String> {
            Ok(columns
                .iter()
                .map(|&(x, z)| {
                    self.world_blocks
                        .keys()
                        .filter(|p| p[0] == x && p[2] == z)
                        .map(|p| p[1] + 1)
                        .max()
                        .or(Some(64))
                })
                .collect())
        }
    }

    impl ClientHost for TestWorldHost {}
    mod_api::impl_client_host_areas!(TestWorldHost);

    impl Backend for TestWorldHost {}
    mod_api::impl_backend_areas!(TestWorldHost, except(block_states, entities, levels, players));

    impl PlayersClientBackend for TestWorldHost {}

    impl EntitiesBackend for TestWorldHost {
        fn entity_position(&self, _entity: mod_api::EntityRef) -> Option<[f64; 3]> {
            Some([10.0, 64.0, 10.0])
        }
        fn entity_rotation(&self, _entity: mod_api::EntityRef) -> Option<[f32; 2]> {
            Some([0.0, 0.0])
        }
    }

    impl BlockStatesBackend for TestWorldHost {
        fn block_metadata_name(&self, block: BlockKey) -> Result<&str, BlockStateError> {
            BlockMetadata::name(self, block)
        }
        fn block_metadata_of(
            &self,
            block: BlockKey,
            properties: &[StateConstraint],
        ) -> Result<BlockStateRecord, BlockStateError> {
            BlockMetadata::of(self, block, properties)
        }
    }
    impl LevelsBackend for TestWorldHost {
        fn level_block_state(&self, _level: LevelRef, pos: [i32; 3]) -> Option<BlockState> {
            Some(
                self.world_blocks
                    .get(&pos)
                    .copied()
                    .unwrap_or_else(|| BlockStateRecord::new(self.air_key, 0).into()),
            )
        }

        fn level_biome_at(&self, _level: LevelRef, _pos: [i32; 3]) -> Option<String> {
            self.biome_id.clone()
        }
    }
    impl PlayersBackend for TestWorldHost {
        fn player_dimension(&self, player: mod_api::PlayerRef) -> Option<mod_api::Identifier> {
            self.fake
                .players
                .get(player.uuid())
                .and_then(|_| mod_api::Identifier::parse("minecraft:overworld").ok())
        }

        fn player_permission_level(&self, player: mod_api::PlayerRef) -> Option<u8> {
            self.fake.players.get(player.uuid())?;
            Some(self.player_perms.get(&player.uuid()).copied().unwrap_or(4))
        }
    }

    impl BackendMut for TestWorldHost {}
    mod_api::impl_backend_mut_areas!(TestWorldHost, except(entities, payloads, players));

    impl EntitiesBackendMut for TestWorldHost {
        fn entity_discard(&mut self, entity: mod_api::EntityRef) -> bool {
            let id = entity.id();
            if let Some(entry) = self.summoned_entities.iter_mut().find(|e| e.0 == id && e.4) {
                entry.4 = false;
                true
            } else {
                false
            }
        }
        fn entity_set_hanging_direction(
            &mut self,
            entity: mod_api::EntityRef,
            direction: mod_api::Direction,
        ) -> bool {
            self.hanging_dirs.insert(entity.id(), direction);
            true
        }
    }

    impl PlayersBackendMut for TestWorldHost {
        fn player_tell(
            &mut self,
            _player: mod_api::PlayerRef,
            _line: mod_api::Text,
        ) -> Result<bool, mod_api::TextFormError> {
            Ok(true)
        }
    }

    impl PayloadsBackendMut for TestWorldHost {
        fn player_send_payload(
            &mut self,
            _: mod_api::PlayerRef,
            id: &mod_api::PayloadId,
            bytes: mod_api::PayloadBytes,
        ) -> Result<mod_api::PayloadAccepted, mod_api::PayloadSendError> {
            self.sent_payloads.push((id.0.clone(), bytes.0));
            Ok(mod_api::PayloadAccepted::Queued)
        }
    }

    #[test]
    fn server_execute_build_undo_and_command_work_end_to_end() {
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();

            let mut host = TestWorldHost::new(keys);
            host.fake.join(77, 100);
            let player = host.player(77).unwrap();

            let before_snapshot = host.world_blocks.clone();
            let req = sample_request("уютный двухэтажный дом с крыльцом и камином", 4242);
            let resp = execute_build(&mut host, Some(player), req);
            assert!(resp.ok, "build failed: {}", resp.message);
            assert!(resp.blocks_changed > 100);
            assert_ne!(host.world_blocks, before_snapshot);

            let undo_resp = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(undo_resp.ok, "undo failed: {}", undo_resp.message);
            assert_eq!(host.world_blocks, before_snapshot);

            // Also test `/buildgen generate` and `/buildgen undo` command dispatch!
            let level_dim = host.level("minecraft:overworld").unwrap().dimension();
            let cmd = CommandInvocation::new(
                vec!["buildgen".into(), "generate".into()],
                vec![CommandArgument::new(
                    "description".into(),
                    CommandArgumentValue::String("уютный дом с крыльцом".into()),
                )],
                level_dim,
                [15.0, 64.0, 15.0],
            );
            let mut ev = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut ev), Flow::Cancel);
            assert_ne!(host.world_blocks, before_snapshot);

            let undo_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "undo".into()],
                vec![],
                level_dim,
                [15.0, 64.0, 15.0],
            );
            let mut undo_ev = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &undo_cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut undo_ev), Flow::Cancel);
            assert_eq!(host.world_blocks, before_snapshot);

            let preset_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "preset".into()],
                vec![CommandArgument::new(
                    "preset".into(),
                    CommandArgumentValue::String("observatory".into()),
                )],
                level_dim,
                [15.0, 64.0, 15.0],
            );
            let mut preset_ev = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &preset_cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut preset_ev), Flow::Cancel);
            assert_ne!(host.world_blocks, before_snapshot);

            let mut undo_ev2 = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &undo_cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut undo_ev2), Flow::Cancel);
            assert_eq!(host.world_blocks, before_snapshot);
        });
    }

    #[test]
    fn terrain_adaptation_builds_foundation_skirt_on_slopes() {
        let mut req = sample_request("каменный дом", 777);
        req.adapt_terrain = true;
        // Simulate a sloped hillside dipping 4 blocks below origin_y on one side.
        let mut offsets = vec![0i32; usize::from(req.width) * usize::from(req.depth)];
        for lz in 0..usize::from(req.depth) {
            for lx in 0..usize::from(req.width) {
                offsets[lz * usize::from(req.width) + lx] = if lx > 6 { -4 } else { 0 };
            }
        }
        let bp = generate_blueprint(&req, Some(&offsets));
        let has_skirt = bp.blocks.keys().any(|&[lx, ly, _lz]| lx > 6 && ly < 0 && ly >= -4);
        assert!(has_skirt, "expected foundation skirt extending down on sloping terrain");
    }

    #[test]
    fn command_registration_and_tab_complete_cover_all_subcommands() {
        let mut cmds = Vec::new();
        let mut reg_ev = mod_api::RegisterCommandsEvent { commands: &mut cmds };
        crate::server_builder::register_commands(&mut reg_ev);
        let usages: Vec<&str> = cmds.iter().map(|c| c.usage.as_str()).collect();
        assert!(usages.contains(&"buildgen"));
        assert!(usages.contains(&"buildgen undo"));
        assert!(usages.contains(&"buildgen redo"));
        assert!(usages.contains(&"buildgen lang <lang:text>"));
        assert!(usages.iter().any(|u| u.starts_with("buildgen generate")));
        assert!(usages.iter().any(|u| u.starts_with("buildgen size")));
        for cmd in &cmds {
            if matches!(
                cmd.usage.as_str(),
                "buildgen undo"
                    | "buildgen redo"
                    | "buildgen reroll"
                    | "buildgen preset <preset:text>"
                    | "buildgen generate <description:text>"
            ) || cmd.usage.starts_with("buildgen size ")
            {
                assert!(cmd.level >= 2, "expected OP level >= 2 for {}", cmd.usage);
            }
        }

        let mut suggestions = Vec::new();
        let mut tab_ev = mod_api::TabCompleteEvent {
            player: 1,
            name: "buildgen",
            command: "buildgen ",
            range: [9, 9],
            suggestions: &mut suggestions,
        };
        crate::server_builder::tab_complete(&mut tab_ev);
        assert!(suggestions.iter().any(|s| s.text == "generate"));
        assert!(suggestions.iter().any(|s| s.text == "lang"));
        assert!(suggestions.iter().any(|s| s.text == "undo"));
        assert!(suggestions.iter().any(|s| s.text == "redo"));

        let mut lang_suggestions = Vec::new();
        let mut tab_lang = mod_api::TabCompleteEvent {
            player: 1,
            name: "buildgen",
            command: "buildgen lang ",
            range: [14, 14],
            suggestions: &mut lang_suggestions,
        };
        crate::server_builder::tab_complete(&mut tab_lang);
        assert!(lang_suggestions.iter().any(|s| s.text == "en"));
        assert!(lang_suggestions.iter().any(|s| s.text == "ru"));
    }

    #[test]
    fn all_styles_and_categories_produce_detailed_architecture() {
        let styles = [
            BuildStyle::Auto,
            BuildStyle::Medieval,
            BuildStyle::Cottage,
            BuildStyle::Japanese,
            BuildStyle::Gothic,
            BuildStyle::Nordic,
            BuildStyle::Modern,
            BuildStyle::Desert,
            BuildStyle::Steampunk,
            BuildStyle::Classical,
        ];
        for style in styles {
            let mut req = sample_request("двухэтажный дом с камином, садом и балконом", 9001);
            req.width = 17;
            req.depth = 15;
            req.height = 16;
            req.style = style;
            req.palette = MaterialPalette::preset_for_style(style);
            let bp = generate_blueprint(&req, None);
            assert!(
                bp.non_air_count() > 200,
                "style {:?} produced too few blocks: {}",
                style,
                bp.non_air_count()
            );
            assert!(
                !bp.blocks
                    .values()
                    .any(|p| p.block_id == "minecraft:chain"),
                "style {:?} emitted legacy minecraft:chain instead of minecraft:iron_chain",
                style
            );
            for p in bp.blocks.values() {
                if p.block_id.ends_with("_leaves") {
                    assert!(
                        p.props.iter().any(|(k, v)| *k == "persistent" && *v == "true"),
                        "leaf {} missing persistent=true in style {:?}",
                        p.block_id,
                        style
                    );
                }
            }
        }

        for cat in [
            BuildCategory::House,
            BuildCategory::Tower,
            BuildCategory::Barn,
            BuildCategory::Pavilion,
            BuildCategory::Castle,
            BuildCategory::Forge,
            BuildCategory::Temple,
            BuildCategory::Bridge,
        ] {
            let mut req = sample_request("детализированная постройка с освещением", 4422);
            req.width = 19;
            req.depth = 17;
            req.height = 16;
            req.category = cat;
            let bp = generate_blueprint(&req, None);
            assert!(
                bp.non_air_count() > 140,
                "category {:?} too empty: {}",
                cat,
                bp.non_air_count()
            );
        }

        // Verify Japanese + Castle + Forge thematic richness & interior furnishings.
        let mut jap_req = sample_request("японский дом с пагодой и садом сакуры", 31415);
        jap_req.width = 19;
        jap_req.depth = 17;
        jap_req.height = 18;
        jap_req.style = BuildStyle::Japanese;
        jap_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let jap_bp = generate_blueprint(&jap_req, None);
        assert!(
            jap_bp
                .blocks
                .values()
                .any(|p| p.block_id == "minecraft:cherry_leaves"),
            "Japanese pagoda should place cherry leaves"
        );

        let mut forge_req = sample_request("кузница в стиле стимпанк с наковальней", 27182);
        forge_req.width = 17;
        forge_req.depth = 15;
        forge_req.height = 14;
        forge_req.category = BuildCategory::Forge;
        forge_req.style = BuildStyle::Steampunk;
        forge_req.palette = MaterialPalette::preset_for_style(BuildStyle::Steampunk);
        let forge_bp = generate_blueprint(&forge_req, None);
        assert!(
            forge_bp
                .blocks
                .values()
                .any(|p| matches!(p.block_id.as_str(), "minecraft:anvil" | "minecraft:blast_furnace")),
            "Forge should contain smithing blocks (anvil / blast_furnace)"
        );
    }

    #[test]
    fn specialized_themes_basements_wells_and_slope_steps_work() {
        // 1. Windmill (blades), Lighthouse (beacon), Greenhouse (glass vault), Tavern, Library.
        let mut windmill_req = sample_request("деревянная мельница с лопастями", 1111);
        windmill_req.width = 15;
        windmill_req.depth = 15;
        windmill_req.height = 18;
        let windmill = generate_blueprint(&windmill_req, None);
        assert_eq!(windmill.spec.theme, StructureTheme::Windmill);
        assert!(
            windmill.blocks.values().any(|p| p.block_id == "minecraft:white_wool"),
            "Windmill must place white_wool sail blades"
        );

        let mut lighthouse_req = sample_request("высокий каменный маяк", 2222);
        lighthouse_req.width = 13;
        lighthouse_req.depth = 13;
        lighthouse_req.height = 22;
        lighthouse_req.palette = infer_palette_from_text(&lighthouse_req.description);
        let lighthouse = generate_blueprint(&lighthouse_req, None);
        assert_eq!(lighthouse.spec.theme, StructureTheme::Lighthouse);
        assert!(lighthouse.blocks.values().any(|p| p.block_id == "minecraft:sea_lantern"));
        assert!(lighthouse.blocks.values().any(|p| p.block_id == "minecraft:lightning_rod"));

        let mut gh_req = sample_request("стеклянная теплица оранжерея с зеленью", 3333);
        gh_req.width = 15;
        gh_req.depth = 13;
        gh_req.height = 12;
        gh_req.palette = infer_palette_from_text(&gh_req.description);
        let gh = generate_blueprint(&gh_req, None);
        assert_eq!(gh.spec.theme, StructureTheme::Greenhouse);
        assert!(gh.blocks.values().any(|p| p.block_id == "minecraft:glass"));
        assert!(gh.blocks.values().any(|p| p.block_id == "minecraft:moss_block"));

        let mut lib_req = sample_request("большая библиотека архив", 4444);
        lib_req.width = 15;
        lib_req.depth = 13;
        lib_req.height = 12;
        let lib_bp = generate_blueprint(&lib_req, None);
        assert_eq!(lib_bp.spec.theme, StructureTheme::Library);
        assert!(
            lib_bp
                .blocks
                .values()
                .any(|p| matches!(p.block_id.as_str(), "minecraft:bookshelf" | "minecraft:chiseled_bookshelf"))
        );

        // 2. Basement + Large plot Courtyard Well + Slope steps!
        let mut cellar_req = sample_request("уютный двухэтажный дом с подвалом и колодцем во дворе", 5555);
        cellar_req.width = 21;
        cellar_req.depth = 19;
        cellar_req.height = 16;
        let cellar_bp = generate_blueprint(&cellar_req, None);
        assert!(cellar_bp.spec.basement);
        assert!(cellar_bp.spec.courtyard_well);
        assert!(
            cellar_bp.blocks.iter().any(|(&[_, ly, _], b)| ly <= -2 && b.block_id == "minecraft:air"),
            "Basement must carve hollow underground room at ly <= -2"
        );
        assert!(
            cellar_bp.blocks.values().any(|b| b.block_id == "minecraft:water_cauldron"),
            "Large estate with courtyard_well should include water_cauldron"
        );

        // 3. Adaptive stairs descending down front hillside slope!
        let mut hill_req = sample_request("дом с крыльцом", 6666);
        hill_req.width = 15;
        hill_req.depth = 15;
        hill_req.height = 14;
        let offsets = vec![-4i32; usize::from(hill_req.width) * usize::from(hill_req.depth)];
        let hill_bp = generate_blueprint(&hill_req, Some(&offsets));
        assert!(
            hill_bp.blocks.iter().any(|(&[_, ly, _], b)| ly < 0 && b.block_id.ends_with("_stairs")),
            "Porch stairs must extend below y=0 onto dipping terrain slope"
        );
    }

    #[test]
    fn two_phase_placement_and_reroll_work_end_to_end() {
        let rr = RerollRequest {
            dimension: "minecraft:overworld".into(),
        };
        let rbytes = encode_payload(&rr, 1024).expect("encode RerollRequest");
        let decoded: RerollRequest = decode_payload(&rbytes).expect("decode RerollRequest");
        assert_eq!(rr, decoded);

        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();

            let mut host = TestWorldHost::new(keys);
            host.fake.join(88, 100);
            let player = host.player(88).unwrap();

            let req = sample_request("двухэтажный дом с крыльцом, камином и подвалом", 123456);
            let bp = generate_blueprint(&req, None);
            let (palette, phase1, phase2) =
                blueprint_to_phased_runs(&host, &bp).expect("phased runs");
            assert!(!palette.is_empty());
            assert!(!phase1.is_empty());
            assert!(!phase2.is_empty(), "expected Phase 2 dependent props (door/bed/lantern/pot)");

            let before = host.world_blocks.clone();
            let b1 = execute_build(&mut host, Some(player), req);
            assert!(b1.ok, "initial build failed: {}", b1.message);
            assert!(
                b1.message.contains("[undo: 1/5") || b1.message.contains("[отмен: 1/5"),
                "Build message should expose undo stack depth: {}",
                b1.message
            );
            let after_first = host.world_blocks.clone();
            assert_ne!(after_first, before);

            // Reroll restores prior ground and places a fresh distinct seed variation!
            let reroll_resp = execute_reroll(&mut host, Some(player), "minecraft:overworld");
            assert!(reroll_resp.ok, "reroll failed: {}", reroll_resp.message);
            assert!(
                reroll_resp.message.contains("[undo: 1/5") || reroll_resp.message.contains("[отмен: 1/5"),
                "Reroll should keep stack at 1/5: {}",
                reroll_resp.message
            );
            let after_reroll = host.world_blocks.clone();
            assert_ne!(after_reroll, before);
            assert_ne!(after_reroll, after_first, "reroll must produce a fresh seed variant");

            let undo_resp = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(undo_resp.ok, "undo after reroll failed: {}", undo_resp.message);
            assert!(
                undo_resp.message.contains("[undo: 0/5") || undo_resp.message.contains("[отмен: 0/5"),
                "Undo message should report 0/5 remaining: {}",
                undo_resp.message
            );
            assert_eq!(host.world_blocks, before);

            // Also verify all 12 CLI presets resolve all blocks cleanly through TestWorldHost!
            for (idx, &(slug, cat, style, w, d, h, desc)) in CLI_PRESETS.iter().enumerate() {
                let pal = MaterialPalette::preset_for_style(style);
                let preq = BuildRequest {
                    dimension: "minecraft:overworld".into(),
                    origin_x: 0,
                    origin_y: 64,
                    origin_z: 0,
                    width: w,
                    depth: d,
                    height: h,
                    category: cat,
                    style,
                    facing: CardinalFacing::South,
                    noise_level: NoiseLevel::Balanced,
                    seed: 1000 + idx as i64,
                    description: desc.into(),
                    adapt_terrain: true,
                    air_only: false,
                    wear_pct: 0,
                    overgrowth_pct: 0,
                    biome_hint: String::new(),
                    palette: pal,
                    living_decor: true,
                    spawn_npcs: false,
                    blend_radius: 0,
                };
                let pbp = generate_blueprint(&preq, None);
                let res = blueprint_to_phased_runs(&host, &pbp);
                assert!(
                    res.is_ok(),
                    "Preset '{slug}' emitted unknown/unregistered block IDs: {:?}",
                    res.err()
                );
            }
        });
    }

    #[test]
    fn connected_states_mitered_stairs_temple_bell_castle_pool_and_pagoda_work() {
        // 1. Connected panes/fences/walls & mitered outer/inner hip roof stair shapes!
        let mut hip_req = sample_request("средневековый дом с шатровой крышей и камином", 7777);
        hip_req.width = 17;
        hip_req.depth = 15;
        hip_req.height = 16;
        hip_req.style = BuildStyle::Medieval;
        let hip_bp = generate_blueprint(&hip_req, None);

        let has_outer_miter = hip_bp.blocks.values().any(|b| {
            b.block_id.ends_with("_stairs")
                && b.props
                    .iter()
                    .any(|(k, v)| *k == "shape" && v.starts_with("outer_"))
        });
        assert!(has_outer_miter, "Hip roof must emit mitered outer_left/outer_right corner stairs");

        for b in hip_bp.blocks.values() {
            if b.block_id.ends_with("glass_pane") || b.block_id.ends_with("_fence") {
                assert!(
                    b.props.iter().any(|(k, _)| *k == "north")
                        && b.props.iter().any(|(k, _)| *k == "east"),
                    "{} must compute directional connection properties",
                    b.block_id
                );
            }
            if b.block_id.ends_with("_wall") {
                assert!(
                    b.props.iter().any(|(k, _)| *k == "up")
                        && b.props.iter().any(|(k, _)| *k == "north"),
                    "wall {} must compute north/up properties",
                    b.block_id
                );
            }
        }
        assert!(
            hip_bp.blocks.values().any(|b| {
                b.block_id == "minecraft:campfire"
                    && b.props.iter().any(|(k, v)| *k == "signal_fire" && *v == "true")
            }),
            "Chimney cap should enable signal_fire=true smoke plume"
        );

        // 2. Temple bell steeple + nave pews!
        let mut temple_req = sample_request("нордический каменный собор храм", 8888);
        temple_req.width = 19;
        temple_req.depth = 17;
        temple_req.height = 18;
        temple_req.category = BuildCategory::Temple;
        let temple_bp = generate_blueprint(&temple_req, None);
        assert!(
            temple_bp.blocks.values().any(|b| b.block_id == "minecraft:bell"),
            "Temple must include a hanging bell steeple"
        );

        // 3. Castle portcullis & Forge molten magma!
        let mut castle_req = sample_request("готический замок цитадель", 9999);
        castle_req.width = 21;
        castle_req.depth = 19;
        castle_req.height = 18;
        castle_req.category = BuildCategory::Castle;
        let castle_bp = generate_blueprint(&castle_req, None);
        assert!(
            castle_bp.blocks.values().any(|b| b.block_id == "minecraft:iron_bars"),
            "Castle must include iron_bars portcullis / bars"
        );

        let mut forge_req = sample_request("кузница", 4242);
        forge_req.width = 17;
        forge_req.depth = 15;
        forge_req.height = 14;
        forge_req.category = BuildCategory::Forge;
        let forge_bp = generate_blueprint(&forge_req, None);
        assert!(
            forge_bp.blocks.values().any(|b| b.block_id == "minecraft:magma_block"),
            "Forge must include magma_block hearth glow"
        );

        // 4. Modern courtyard reflecting pool!
        let mut mod_req = sample_request("современный дом с двором и колодцем", 1212);
        mod_req.width = 21;
        mod_req.depth = 19;
        mod_req.height = 14;
        mod_req.style = BuildStyle::Modern;
        let mod_bp = generate_blueprint(&mod_req, None);
        assert!(
            mod_bp.blocks.values().any(|b| b.block_id == "minecraft:water"),
            "Large Modern estate should feature water reflecting pool"
        );

        // 5. Japanese multi-tier pagoda!
        let mut pag_req = sample_request("высокая японская пагода", 3434);
        pag_req.width = 13;
        pag_req.depth = 13;
        pag_req.height = 20;
        pag_req.category = BuildCategory::Tower;
        pag_req.style = BuildStyle::Japanese;
        pag_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let pag_bp = generate_blueprint(&pag_req, None);
        assert!(
            pag_bp.blocks.values().filter(|b| b.block_id.ends_with("_slab")).count() > 25,
            "Multi-tier Japanese Pagoda should place multiple upturned eave slab skirts"
        );
    }

    #[test]
    fn dome_and_mansard_roofs_build_distinct_crowned_architectures() {
        let mut dome_req = sample_request("обсерватория с куполом", 5151);
        dome_req.width = 15;
        dome_req.depth = 15;
        dome_req.height = 20;
        dome_req.category = BuildCategory::Tower;
        let dome_bp = generate_blueprint(&dome_req, None);
        assert_eq!(dome_bp.spec.roof_style, RoofStyle::Dome);
        let dome_max_y = dome_bp
            .blocks
            .iter()
            .filter(|(_, b)| b.block_id != "minecraft:air")
            .map(|(p, _)| p[1])
            .max()
            .unwrap_or(0);
        assert!(dome_max_y >= 11, "Dome should curve cleanly above base roof level");

        let mut mans_req = sample_request("французский двухэтажный особняк с мансардной крышей", 6161);
        mans_req.width = 17;
        mans_req.depth = 15;
        mans_req.height = 16;
        let mans_bp = generate_blueprint(&mans_req, None);
        assert_eq!(mans_bp.spec.roof_style, RoofStyle::Mansard);
        assert!(
            mans_bp.blocks.values().any(|b| {
                b.block_id.ends_with("_stairs")
                    && b.props
                        .iter()
                        .any(|(k, v)| *k == "shape" && (*v == "outer_left" || *v == "outer_right"))
            }),
            "Mansard apron stairs should miter corners cleanly"
        );
    }

    #[test]
    fn terrain_smoothing_terraces_hills_and_ramps_slopes() {
        let mut req = sample_request("уютный дом", 7272);
        req.width = 19;
        req.depth = 17;
        req.height = 12;
        req.adapt_terrain = true;
        // Steep hill (+4) on rear/left (z < 4) and deep slope (-4) on right margin (x >= 16).
        let mut offsets = vec![0i32; usize::from(req.width) * usize::from(req.depth)];
        for z in 0..usize::from(req.depth) {
            for x in 0..usize::from(req.width) {
                offsets[z * usize::from(req.width) + x] = if z < 4 {
                    4
                } else if x >= 16 {
                    -4
                } else {
                    0
                };
            }
        }
        let bp = generate_blueprint(&req, Some(&offsets));
        assert!(
            bp.blocks.iter().any(|(&[_x, y, _z], b)| y < 0 && b.block_id == "minecraft:grass_block"),
            "Downhill perimeter margin should smoothly ramp grass blocks down"
        );
        assert!(
            bp.blocks.iter().any(|(&[_x, y, z], b)| z < 4 && y >= 1 && b.block_id == "minecraft:grass_block"),
            "Uphill hillside perimeter should terrace stepped grass blocks"
        );
    }

    #[test]
    fn multi_floor_interiors_and_courtyard_ensembles_place_details() {
        let mut manor_req = sample_request("большой трехэтажный готический замок с высокими потолками", 8383);
        manor_req.width = 23;
        manor_req.depth = 21;
        manor_req.height = 24;
        manor_req.category = BuildCategory::House;
        manor_req.style = BuildStyle::Gothic;
        let manor_bp = generate_blueprint(&manor_req, None);
        assert!(
            manor_bp.blocks.values().any(|b| b.block_id == "minecraft:candle"),
            "Upper stories should include lit candles"
        );
        assert!(
            manor_bp
                .blocks
                .values()
                .any(|b| b.block_id == "minecraft:soul_wall_torch" || b.block_id == "minecraft:wall_torch"),
            "Interior perimeter solid walls should host wall sconces"
        );

        let mut jp_req = sample_request("японский дом пагода с садом сакуры и двором", 9494);
        jp_req.width = 21;
        jp_req.depth = 19;
        jp_req.height = 18;
        jp_req.style = BuildStyle::Japanese;
        jp_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let jp_bp = generate_blueprint(&jp_req, None);
        assert!(
            jp_bp.blocks.values().any(|b| b.block_id == "minecraft:crimson_planks"),
            "Spacious Japanese estate should erect a Crimson Torii entrance gate"
        );
        assert!(
            jp_bp.blocks.values().any(|b| b.block_id.ends_with("_fence_gate")),
            "Spacious courtyard should include matching front fence gate"
        );

        let mut farm_req = sample_request("деревянная ветряная мельница с амбаром", 1357);
        farm_req.width = 21;
        farm_req.depth = 19;
        farm_req.height = 18;
        farm_req.category = BuildCategory::Barn;
        let farm_bp = generate_blueprint(&farm_req, None);
        assert!(
            farm_bp.blocks.values().any(|b| b.block_id == "minecraft:farmland")
                && farm_bp.blocks.values().any(|b| b.block_id == "minecraft:wheat"),
            "Barn/Windmill estates should generate irrigated farmland + wheat crop plots"
        );
    }

    #[test]
    fn cli_presets_resolve_cleanly_by_slug_and_russian_keywords() {
        assert_eq!(find_cli_preset("teahouse").map(|p| p.0), Some("teahouse"));
        assert_eq!(find_cli_preset("чайн").map(|p| p.0), Some("teahouse"));
        assert_eq!(find_cli_preset("observatory").map(|p| p.0), Some("observatory"));
        assert_eq!(find_cli_preset("купол").map(|p| p.0), Some("observatory"));
        assert_eq!(find_cli_preset("мансард").map(|p| p.0), Some("mansard"));
        assert_eq!(find_cli_preset("pagoda").map(|p| p.0), Some("pagoda"));
        assert_eq!(find_cli_preset("colosseum").map(|p| p.0), Some("colosseum"));
        assert_eq!(find_cli_preset("колиз").map(|p| p.0), Some("colosseum"));
        assert_eq!(find_cli_preset("village").map(|p| p.0), Some("village"));
        assert_eq!(find_cli_preset("квартал").map(|p| p.0), Some("village"));
        assert!(find_cli_preset("nonexistent_preset_xyz").is_none());
        assert_eq!(CLI_PRESETS.len(), 88);
    }

    #[test]
    fn layered_master_house_generates_cranes_awning_stepped_wing_and_planters() {
        let mut req = sample_request(
            "японский двухэтажный чайный дом с террасой, лавкой, полосатым навесом и фонарями",
            4040,
        );
        req.width = 21;
        req.depth = 19;
        req.height = 18;
        req.style = BuildStyle::Japanese;
        req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let bp = generate_blueprint(&req, None);
        assert!(bp.spec.stepped_wing, "Large multi-story tea house should use stepped wing massing");
        assert!(bp.spec.market_awning, "Shop/tea house should enable striped cloth awning");

        let has_wool_awning = bp
            .blocks
            .values()
            .any(|b| b.block_id == "minecraft:orange_wool" || b.block_id == "minecraft:red_wool")
            && bp.blocks.values().any(|b| b.block_id == "minecraft:white_wool");
        assert!(has_wool_awning, "Tea house must emit striped wool street awning");

        let has_planter_flora = bp
            .blocks
            .values()
            .any(|b| matches!(b.block_id.as_str(), "minecraft:flowering_azalea" | "minecraft:azalea" | "minecraft:bamboo"));
        assert!(has_planter_flora, "Yard and terraces should contain lush azalea/bamboo greenery");

        let has_hanging_lantern = bp.blocks.values().any(|b| {
            matches!(b.block_id.as_str(), "minecraft:lantern" | "minecraft:copper_lantern" | "minecraft:soul_lantern")
                && b.props.iter().any(|(k, v)| *k == "hanging" && *v == "true")
        });
        assert!(has_hanging_lantern, "Outrigger cranes and awnings must hang lanterns");
    }

    #[test]
    fn koi_pond_tatami_shelves_petals_vines_and_kegs_enrich_estates() {
        let mut jp_req = sample_request(
            "японский двухэтажный чайный дом с террасой, садом сакуры и лавкой",
            2525,
        );
        jp_req.width = 23;
        jp_req.depth = 21;
        jp_req.height = 18;
        jp_req.style = BuildStyle::Japanese;
        jp_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let jp_bp = generate_blueprint(&jp_req, None);
        assert!(
            jp_bp.blocks.values().any(|b| b.block_id == "minecraft:stone_brick_slab"),
            "Dark slate Japanese roofs should carry pale contrasting stone_brick_slab ridge crest"
        );
        assert!(
            jp_bp.blocks.values().any(|b| b.block_id == "minecraft:lime_carpet"),
            "Japanese houses must lay woven lime/moss tatami mats indoors"
        );
        assert!(
            jp_bp.blocks.values().any(|b| b.block_id == "minecraft:water")
                && jp_bp.blocks.values().any(|b| b.block_id == "minecraft:lily_pad"),
            "Spacious Japanese estate yards should generate a zen pond with lily pads"
        );

        let mut cot_req = sample_request(
            "уютный двухэтажный коттедж с плющом, цветами, балконом и двором",
            6767,
        );
        cot_req.width = 21;
        cot_req.depth = 19;
        cot_req.height = 16;
        cot_req.style = BuildStyle::Cottage;
        cot_req.palette = MaterialPalette::preset_for_style(BuildStyle::Cottage);
        let cot_bp = generate_blueprint(&cot_req, None);
        assert!(
            cot_bp.blocks.values().any(|b| b.block_id == "minecraft:vine"),
            "Overgrown cottage facades should drape climbing wall vines"
        );
        assert!(
            cot_bp.blocks.values().any(|b| {
                matches!(b.block_id.as_str(), "minecraft:wildflowers" | "minecraft:short_grass" | "minecraft:fern")
            }),
            "Cottage garden lawns should scatter wildflowers/short_grass tufts"
        );

        let mut tav_req = sample_request("уютная средневековая таверна с камином и бочками", 3131);
        tav_req.width = 19;
        tav_req.depth = 17;
        tav_req.height = 16;
        let tav_bp = generate_blueprint(&tav_req, None);
        assert!(
            tav_bp.blocks.values().filter(|b| b.block_id == "minecraft:barrel").count() >= 4,
            "Tavern interior should stack wine barrels and bar counter kegs"
        );
    }

    #[test]
    fn custom_trees_bay_windows_signs_downspouts_basements_and_negative_rules_work() {
        let mut manor_req = sample_request(
            "средневековый двухэтажный дом с подвалом, балконом и садом",
            1919,
        );
        manor_req.width = 23;
        manor_req.depth = 21;
        manor_req.height = 18;
        manor_req.style = BuildStyle::Medieval;
        manor_req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);
        let manor_bp = generate_blueprint(&manor_req, None);

        assert!(
            manor_bp.blocks.values().any(|b| b.block_id == "minecraft:oak_log"),
            "Spacious overgrown/garden estate must grow custom procedural yard trees"
        );
        assert!(
            manor_bp
                .blocks
                .values()
                .any(|b| b.block_id == "minecraft:water_cauldron"),
            "Exterior walls should collect rainwater from downspouts into water_cauldron"
        );
        assert!(
            manor_bp
                .blocks
                .values()
                .any(|b| b.block_id.ends_with("_hanging_sign")),
            "Houses with shop/garden facade details should hang swinging signage"
        );
        assert!(
            manor_bp
                .blocks
                .iter()
                .any(|(&[_x, y, _z], b)| y <= -2 && b.block_id == "minecraft:cobblestone_stairs"),
            "Large homes with basement should carve walkable stone cellar stairs"
        );
        assert!(
            manor_bp.blocks.iter().any(|(&[_x, y, _z], b)| {
                y == -3
                    && matches!(
                        b.block_id.as_str(),
                        "minecraft:barrel"
                            | "minecraft:brewing_stand"
                            | "minecraft:smithing_table"
                            | "minecraft:grindstone"
                    )
            }),
            "Basement at y=-3 should be furnished with cellar/alchemy/armory fittings"
        );

        let neg_spec = parse_description(
            BuildCategory::House,
            BuildStyle::Medieval,
            "дом без подвала, без балкона, без крыльца, без навеса, без сада",
            15,
            13,
            14,
            NoiseLevel::Balanced,
            1234,
        );
        assert!(!neg_spec.basement);
        assert!(!neg_spec.balcony);
        assert!(!neg_spec.porch);
        assert!(!neg_spec.market_awning);
        assert!(!neg_spec.courtyard_garden);
    }

    #[test]
    fn doorways_and_staircases_have_full_player_clearance_across_styles() {
        let mut modern_req = sample_request("современный дом с крыльцом и фонарями", 2838086);
        modern_req.style = BuildStyle::Modern;
        modern_req.palette = MaterialPalette::preset_for_style(BuildStyle::Modern);

        let mut steam_req = sample_request("стимпанк дом с террасой, крыльцом и фонарями", 8244012);
        steam_req.width = 19;
        steam_req.depth = 18;
        steam_req.height = 16;
        steam_req.style = BuildStyle::Steampunk;
        steam_req.palette = MaterialPalette::preset_for_style(BuildStyle::Steampunk);

        let mut cellar_req = sample_request("средневековый двухэтажный дом с подвалом и крыльцом", 554433);
        cellar_req.width = 19;
        cellar_req.depth = 17;
        cellar_req.height = 16;
        cellar_req.style = BuildStyle::Medieval;
        cellar_req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);

        let mut tower_req = sample_request("высокая каменная башня со спиральной лестницей", 9012);
        tower_req.category = BuildCategory::Tower;
        tower_req.width = 11;
        tower_req.depth = 11;
        tower_req.height = 20;

        for req in [&modern_req, &steam_req, &cellar_req, &tower_req] {
            let bp = generate_blueprint(req, None);
            if let Some([dx, dy, dz]) = bp.door_pos {
                let (fx, fz) = match req.facing {
                    CardinalFacing::South => (0, 1),
                    CardinalFacing::North => (0, -1),
                    CardinalFacing::East => (1, 0),
                    CardinalFacing::West => (-1, 0),
                };
                for dist in 1..=2 {
                    for head_off in 0..=1 {
                        let p = [dx + fx * dist, dy + head_off, dz + fz * dist];
                        let blk_id = bp.blocks.get(&p).map_or("minecraft:air", |b| b.block_id.as_str());
                        assert_eq!(
                            blk_id, "minecraft:air",
                            "Doorway corridor at dist={dist}, head_off={head_off} blocked by {blk_id} for {:?}",
                            req.style
                        );
                    }
                }
            }

            // Every interior basement cobblestone_stairs step must have >= 2 blocks of air above it.
            for (&[x, y, z], blk) in &bp.blocks {
                if blk.block_id == "minecraft:cobblestone_stairs" && y <= 1 {
                    for cy in 1..=2 {
                        let up_id = bp
                            .blocks
                            .get(&[x, y + cy, z])
                            .map_or("minecraft:air", |b| b.block_id.as_str());
                        assert_eq!(
                            up_id, "minecraft:air",
                            "Basement staircase step at [{x}, {y}, {z}] blocked at +{cy} by {up_id}"
                        );
                    }
                }
            }
        }

        // Steampunk corner details must not leave bare dangling iron_chain wall segments outside walls.
        let steam_bp = generate_blueprint(&steam_req, None);
        assert!(
            !steam_bp
                .blocks
                .iter()
                .any(|(&[_x, y, _z], b)| y <= 4 && b.block_id == "minecraft:iron_chain"),
            "Ground floor outside walls should not dangle loose iron_chain Steampunk pipes"
        );
    }

    #[test]
    fn rich_text_palette_inference_and_expanded_materials_work() {
        use crate::parser::{infer_palette_with_style, infer_style_from_text};

        let birch_copper = infer_palette_with_style(
            BuildStyle::Auto,
            "уютный дом из березы с медной крышей и голубыми окнами",
        );
        assert_eq!(birch_copper.wall, "minecraft:birch_planks");
        assert_eq!(birch_copper.roof_block, "minecraft:cut_copper");
        assert_eq!(birch_copper.window, "minecraft:light_blue_stained_glass_pane");

        let brick_sand = infer_palette_with_style(
            BuildStyle::Auto,
            "дом с кирпичной крышей, стенами из красного песчаника и синим огнем",
        );
        assert_eq!(brick_sand.roof_block, "minecraft:bricks");
        assert_eq!(brick_sand.wall, "minecraft:red_sandstone");
        assert_eq!(brick_sand.light, "minecraft:soul_lantern");

        let pale_ox = infer_palette_with_style(
            BuildStyle::Auto,
            "бледный дуб с окисленной медной крышей и тонированным стеклом",
        );
        assert_eq!(pale_ox.wall, "minecraft:pale_oak_planks");
        assert_eq!(pale_ox.roof_block, "minecraft:oxidized_cut_copper");
        assert_eq!(pale_ox.window, "minecraft:tinted_glass");

        assert_eq!(
            infer_style_from_text(BuildStyle::Steampunk, "японский чайный дом", 42),
            BuildStyle::Steampunk
        );

        assert!(MaterialPalette::PRESET_COUNT >= 16);
        assert!(MaterialPalette::WALL_CHOICES.len() >= 30);
        assert!(MaterialPalette::ROOF_CHOICES.len() >= 30);
        for idx in 0..MaterialPalette::PRESET_COUNT {
            let p = MaterialPalette::preset(idx);
            assert!(p.wall.starts_with("minecraft:"));
            assert!(p.fence_gate_for_fence().starts_with("minecraft:"));
        }
    }

    #[test]
    fn phase1_composite_turrets_arcades_half_hip_rose_windows_and_dome_rotunda_work() {
        let mut cath_req = sample_request(
            "готический собор с башнями, розеткой-витражом, аркбутанами и колоколом",
            7171,
        );
        cath_req.category = BuildCategory::Temple;
        cath_req.style = BuildStyle::Gothic;
        cath_req.width = 21;
        cath_req.depth = 19;
        cath_req.height = 20;
        cath_req.palette = MaterialPalette::preset_for_style(BuildStyle::Gothic);
        let cath_bp = generate_blueprint(&cath_req, None);
        assert!(cath_bp.spec.rose_window);
        assert!(cath_bp.spec.arcade_gallery);
        assert!(
            cath_bp.blocks.values().any(|b| b.block_id == "minecraft:bell"),
            "Cathedral should build bell steeple"
        );
        assert!(
            cath_bp
                .blocks
                .values()
                .filter(|b| b.block_id == cath_req.palette.window)
                .count()
                >= 8,
            "Cathedral gable rose window and side bays should emit stained glass windows"
        );

        let mut mansion_req = sample_request(
            "средневековый двухэтажный дом с полувальмовой крышей, аркадой и угловой башней",
            8282,
        );
        mansion_req.width = 21;
        mansion_req.depth = 17;
        mansion_req.height = 18;
        mansion_req.style = BuildStyle::Medieval;
        mansion_req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);
        let mansion_bp = generate_blueprint(&mansion_req, None);
        assert!(mansion_bp.spec.half_hip);
        assert!(mansion_bp.spec.arcade_gallery);
        assert!(
            mansion_bp.blocks.values().any(|b| b.block_id == "minecraft:ladder"),
            "Octagonal corner turret should include interior climbing ladder"
        );

        let mut dome_req = sample_request("обсерватория с куполом", 9393);
        dome_req.width = 15;
        dome_req.depth = 15;
        dome_req.height = 20;
        let dome_bp = generate_blueprint(&dome_req, None);
        assert_eq!(dome_bp.spec.roof_style, RoofStyle::Dome);
        assert!(
            dome_bp
                .blocks
                .values()
                .any(|b| b.block_id == "minecraft:lightning_rod"),
            "Open cupola rotunda atop dome should finish with spire lightning_rod"
        );
    }

    #[test]
    fn phase2_multi_room_floorplan_doors_and_rich_furniture_work() {
        let mut house_req = sample_request(
            "большой средневековый двухэтажный дом с кухней, столовой и спальней",
            4141,
        );
        house_req.width = 17;
        house_req.depth = 15;
        house_req.height = 14;
        house_req.style = BuildStyle::Medieval;
        house_req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);
        let house_bp = generate_blueprint(&house_req, None);
        assert!(house_bp.spec.room_partitions);
        let open_doors = house_bp
            .blocks
            .values()
            .filter(|b| {
                (b.block_id == house_req.palette.door || b.block_id == house_req.palette.interior_door)
                    && b.props.iter().any(|&(k, v)| k == "half" && v == "lower")
                    && b.props.iter().any(|&(k, v)| k == "open" && v == "true")
            })
            .count();
        assert!(
            open_doors >= 2,
            "Spacious multi-story partitioned house should have open interior doors, got {open_doors}"
        );
        for needed in [
            "minecraft:cake",
            "minecraft:sea_pickle",
            "minecraft:candle",
            "minecraft:jukebox",
            "minecraft:smoker",
            "minecraft:crafting_table",
            "minecraft:water_cauldron",
            "minecraft:cartography_table",
            "minecraft:loom",
            "minecraft:red_bed",
        ] {
            assert!(
                house_bp.blocks.values().any(|b| b.block_id == needed),
                "Phase 2 spacious house should contain {needed}"
            );
        }
        assert!(
            house_bp
                .blocks
                .values()
                .any(|b| b.block_id.ends_with("_shelf")),
            "House kitchen/bedroom should mount a wooden shelf cupboard or headboard shelf"
        );

        let mut nordic_req = sample_request("скандинавский уютный двухэтажный дом", 7474);
        nordic_req.width = 15;
        nordic_req.depth = 13;
        nordic_req.height = 13;
        nordic_req.style = BuildStyle::Nordic;
        nordic_req.palette = MaterialPalette::preset_for_style(BuildStyle::Nordic);
        let nordic_bp = generate_blueprint(&nordic_req, None);
        assert!(
            nordic_bp
                .blocks
                .values()
                .any(|b| b.block_id == "minecraft:light_blue_bed"),
            "Nordic house should furnish upper floor with light_blue_bed"
        );

        let mut forge_req = sample_request("большая каменная кузница с горном и наковальней", 5252);
        forge_req.width = 15;
        forge_req.depth = 13;
        forge_req.height = 10;
        let forge_bp = generate_blueprint(&forge_req, None);
        for needed in [
            "minecraft:lava_cauldron",
            "minecraft:stonecutter",
            "minecraft:blast_furnace",
            "minecraft:anvil",
        ] {
            assert!(
                forge_bp.blocks.values().any(|b| b.block_id == needed),
                "Phase 2 Forge workshop should contain {needed}"
            );
        }

        let mut temple_req = sample_request("высокий готический храм с алтарем", 6363);
        temple_req.width = 15;
        temple_req.depth = 15;
        temple_req.height = 16;
        temple_req.style = BuildStyle::Gothic;
        temple_req.palette = MaterialPalette::preset_for_style(BuildStyle::Gothic);
        let temple_bp = generate_blueprint(&temple_req, None);
        assert!(
            temple_bp.blocks.values().any(|b| b.block_id == "minecraft:beacon"),
            "Grand temple sanctuary should feature a beacon altar"
        );

        let mut tavern_req = sample_request("средневековая таверна с барной стойкой", 8585);
        tavern_req.width = 17;
        tavern_req.depth = 15;
        tavern_req.height = 13;
        tavern_req.style = BuildStyle::Medieval;
        tavern_req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);
        let tavern_bp = generate_blueprint(&tavern_req, None);
        for needed in ["minecraft:sea_pickle", "minecraft:cake", "minecraft:barrel"] {
            assert!(
                tavern_bp.blocks.values().any(|b| b.block_id == needed),
                "Phase 2 Tavern bar counter should include {needed}"
            );
        }
    }

    #[test]
    fn colosseum_classical_architecture_and_no_floating_blocks_work() {
        let desc = "античный колизей";
        let seed = 819_489i64;
        let pal = crate::parser::infer_palette_with_style_and_seed(BuildStyle::Auto, desc, seed as u64);
        assert_eq!(pal.wall, "minecraft:smooth_sandstone");
        assert_eq!(pal.frame, "minecraft:quartz_pillar");

        let mut req = sample_request(desc, seed);
        req.width = 25;
        req.depth = 25;
        req.height = 16;
        req.style = BuildStyle::Auto;
        req.palette = pal;
        let col_bp = generate_blueprint(&req, None);
        assert_eq!(col_bp.spec.theme, StructureTheme::Colosseum);
        assert_eq!(col_bp.spec.style, BuildStyle::Classical);
        assert_eq!(col_bp.spec.roof_style, RoofStyle::Flat);
        assert!(!col_bp.spec.chimney);
        assert!(!col_bp.spec.shutters);
        for needed in [
            "minecraft:sand",
            "minecraft:quartz_pillar",
            "minecraft:sandstone_stairs",
            "minecraft:sandstone_wall",
            "minecraft:red_wool",
            "minecraft:white_wool",
        ] {
            assert!(
                col_bp.blocks.values().any(|b| b.block_id == needed),
                "Classical Colosseum should contain {needed}"
            );
        }

        let mut parthenon_req = sample_request("античный греческий храм парфенон с колоннадой", 303);
        parthenon_req.width = 19;
        parthenon_req.depth = 17;
        parthenon_req.height = 14;
        parthenon_req.palette = crate::parser::infer_palette_with_style_and_seed(
            BuildStyle::Auto,
            &parthenon_req.description,
            303,
        );
        let parthenon_bp = generate_blueprint(&parthenon_req, None);
        assert_eq!(parthenon_bp.spec.category, BuildCategory::Temple);
        assert_eq!(parthenon_bp.spec.style, BuildStyle::Classical);
        assert!(parthenon_bp.spec.arcade_gallery);

        let mut pantheon_req = sample_request("римский пантеон с куполом", 404);
        pantheon_req.width = 19;
        pantheon_req.depth = 19;
        pantheon_req.height = 18;
        pantheon_req.palette = crate::parser::infer_palette_with_style_and_seed(
            BuildStyle::Auto,
            &pantheon_req.description,
            404,
        );
        let pantheon_bp = generate_blueprint(&pantheon_req, None);
        assert_eq!(pantheon_bp.spec.category, BuildCategory::Temple);
        assert_eq!(pantheon_bp.spec.roof_style, RoofStyle::Dome);

        // Verify structural no-floating-block invariant across Modern, Steampunk (2F tall), Gothic Cathedral, Ruins, and Colosseum.
        let mut modern_req = sample_request("современный дом модерн", 819_489);
        modern_req.width = 19;
        modern_req.depth = 17;
        modern_req.height = 14;
        modern_req.style = BuildStyle::Modern;
        modern_req.palette = MaterialPalette::preset_for_style(BuildStyle::Modern);

        let mut steam_req = sample_request("стимпанк особняк 2 этажа с башней", 2_838_086);
        steam_req.width = 19;
        steam_req.depth = 17;
        steam_req.height = 16;
        steam_req.style = BuildStyle::Steampunk;
        steam_req.palette = MaterialPalette::preset_for_style(BuildStyle::Steampunk);

        let mut ruin_req = sample_request("разрушенный готический замок руины", 5555);
        ruin_req.width = 21;
        ruin_req.depth = 19;
        ruin_req.height = 18;
        ruin_req.style = BuildStyle::Gothic;
        ruin_req.palette = MaterialPalette::preset_for_style(BuildStyle::Gothic);

        for bp in [
            col_bp,
            parthenon_bp,
            generate_blueprint(&modern_req, None),
            generate_blueprint(&steam_req, None),
            generate_blueprint(&ruin_req, None),
        ] {
            for (&[x, y, z], blk) in &bp.blocks {
                let id = blk.block_id.as_str();
                if id == "minecraft:air" || y <= 0 {
                    continue;
                }
                let is_non_air = |p: [i32; 3]| match bp.blocks.get(&p) {
                    Some(b) => b.block_id != "minecraft:air",
                    None => p[1] <= 0,
                };
                let has_neighbor = [
                    [x - 1, y, z],
                    [x + 1, y, z],
                    [x, y - 1, z],
                    [x, y + 1, z],
                    [x, y, z - 1],
                    [x, y, z + 1],
                ]
                .into_iter()
                .any(is_non_air);
                assert!(
                    has_neighbor,
                    "Orphan floating block {id} at [{x}, {y}, {z}] in {:?}",
                    bp.spec.style
                );
                if id == "minecraft:iron_chain"
                    || (id.ends_with("lantern")
                        && id != "minecraft:sea_lantern"
                        && blk.props.iter().any(|&(k, v)| k == "hanging" && v == "true"))
                {
                    assert!(
                        is_non_air([x, y + 1, z]),
                        "Hanging {id} at [{x}, {y}, {z}] has air above!"
                    );
                }
                if id == "minecraft:copper_bulb" || id.ends_with("_bed") || id.ends_with("_carpet") {
                    assert!(
                        is_non_air([x, y - 1, z]),
                        "{id} at [{x}, {y}, {z}] has air below!"
                    );
                }
                if id.ends_with("_wall") {
                    let supported = is_non_air([x, y - 1, z])
                        || (is_non_air([x - 1, y, z]) && is_non_air([x + 1, y, z]))
                        || (is_non_air([x, y, z - 1]) && is_non_air([x, y, z + 1]));
                    assert!(supported, "Wall {id} at [{x}, {y}, {z}] is dangling over air!");
                }
            }
        }
    }

    #[test]
    fn phase3_living_yards_outdoor_workshops_cypress_and_terraces_work() {
        let mut forge_req = sample_request("кузница с большим двором и садом", 9090);
        forge_req.category = BuildCategory::Forge;
        forge_req.width = 19;
        forge_req.depth = 17;
        forge_req.height = 12;
        let forge_bp = generate_blueprint(&forge_req, None);
        assert!(
            forge_bp.blocks.values().any(|b| b.block_id == "minecraft:dark_oak_log")
                && forge_bp.blocks.values().any(|b| b.block_id == "minecraft:grindstone"),
            "Spacious Forge should furnish outdoor smithing yard (logs + grindstone)"
        );

        let mut barn_req = sample_request("деревянный амбар с сеном и колодцем", 9191);
        barn_req.category = BuildCategory::Barn;
        barn_req.width = 19;
        barn_req.depth = 17;
        barn_req.height = 12;
        let barn_bp = generate_blueprint(&barn_req, None);
        assert!(
            barn_bp
                .blocks
                .iter()
                .any(|(&[_, y, _], b)| y == 2 && b.block_id == "minecraft:hay_block"),
            "Barn farmstead should stack hay_block bales up to y=2"
        );

        let mut classical_req = sample_request("античный храм вилла с кипарисами и садом", 9292);
        classical_req.width = 21;
        classical_req.depth = 19;
        classical_req.height = 14;
        classical_req.palette = crate::parser::infer_palette_with_style_and_seed(
            BuildStyle::Auto,
            &classical_req.description,
            9292,
        );
        let classical_bp = generate_blueprint(&classical_req, None);
        assert_eq!(classical_bp.spec.style, BuildStyle::Classical);
        assert!(
            classical_bp.blocks.values().any(|b| b.block_id == "minecraft:campfire"),
            "Classical courtyard entrance should flank ceremonial campfire braziers"
        );
        assert!(
            classical_bp.blocks.values().any(|b| b.block_id == "minecraft:spruce_leaves"),
            "Classical courtyard should plant slender cypress trees"
        );

        let mut villa_req = sample_request(
            "современная вилла модерн с бассейном, колодцем и садом",
            9393,
        );
        villa_req.width = 21;
        villa_req.depth = 19;
        villa_req.height = 14;
        villa_req.style = BuildStyle::Modern;
        villa_req.palette = MaterialPalette::preset_for_style(BuildStyle::Modern);
        let villa_bp = generate_blueprint(&villa_req, None);
        assert!(
            villa_bp.blocks.values().any(|b| b.block_id == "minecraft:birch_log"),
            "Modern courtyard should plant ornamental birch/azalea centerpiece tree"
        );
        assert!(
            villa_bp.blocks.values().any(|b| b.block_id == "minecraft:potted_azalea_bush"),
            "Modern pool deck should place poolside potted_azalea_bush"
        );

        let mut tavern_req = sample_request("большая таверна с двором и бочками", 9494);
        tavern_req.width = 19;
        tavern_req.depth = 17;
        tavern_req.height = 14;
        let tavern_bp = generate_blueprint(&tavern_req, None);
        assert!(
            tavern_bp.blocks.values().any(|b| b.block_id == "minecraft:sea_pickle")
                && tavern_bp.blocks.values().any(|b| b.block_id == "minecraft:cake"),
            "Tavern beer garden should furnish mugs & cake outdoors/indoors"
        );
    }

    #[test]
    fn phase4_stairs_and_ladders_have_unobstructed_access_backing_and_support() {
        let mut issues = Vec::new();
        let prompts: &[(&str, BuildCategory, BuildStyle, u16, u16, u16)] = &[
            ("античная библиотека вилла классика", BuildCategory::House, BuildStyle::Classical, 15, 13, 14),
            ("классический дом с террасой и библиотекой", BuildCategory::House, BuildStyle::Classical, 17, 15, 16),
            ("большой античный храм с этажами", BuildCategory::Temple, BuildStyle::Classical, 17, 15, 16),
            ("готический особняк замок с башнями и подвалом", BuildCategory::House, BuildStyle::Gothic, 19, 17, 16),
            ("замок с угловыми башнями и лестницами", BuildCategory::Castle, BuildStyle::Medieval, 19, 17, 16),
            ("сторожевая башня со спиральной лестницей", BuildCategory::Tower, BuildStyle::Medieval, 11, 11, 18),
            ("уютный компактный дом с подвалом", BuildCategory::House, BuildStyle::Cottage, 11, 11, 12),
            ("стимпанк дом кузница", BuildCategory::Forge, BuildStyle::Steampunk, 15, 13, 14),
            ("японский двухэтажный чайный дом", BuildCategory::House, BuildStyle::Japanese, 15, 13, 14),
            ("современная вилла модерн", BuildCategory::House, BuildStyle::Modern, 17, 15, 14),
        ];

        let dir_vec = |facing: &str| -> [i32; 3] {
            match facing {
                "north" => [0, 0, -1],
                "south" => [0, 0, 1],
                "east" => [1, 0, 0],
                "west" => [-1, 0, 0],
                _ => [0, 0, 1],
            }
        };
        let prop = |b: &crate::gen_common::PlannedBlock, k: &str| -> Option<&'static str> {
            b.props.iter().find(|&&(pk, _)| pk == k).map(|&(_, pv)| pv)
        };
        let is_passable = |b: Option<&crate::gen_common::PlannedBlock>| -> bool {
            match b {
                None => true,
                Some(blk) => matches!(
                    blk.block_id.as_str(),
                    "minecraft:air" | "minecraft:ladder" | "minecraft:torch" | "minecraft:wall_torch" | "minecraft:soul_wall_torch"
                ) || blk.block_id.ends_with("_carpet") || blk.block_id.ends_with("_trapdoor"),
            }
        };

        for (idx, &(desc, cat, style, w, d, h)) in prompts.iter().enumerate() {
            for seed in [101i64, 4242, 7777, 1699156, 8244012] {
                let mut req = sample_request(desc, seed + idx as i64);
                req.category = cat;
                req.style = style;
                req.width = w;
                req.depth = d;
                req.height = h;
                req.palette = MaterialPalette::preset_for_style(style);
                let bp = generate_blueprint(&req, None);

                // 1) Inspect every ladder column!
                let mut ladder_cols: std::collections::BTreeMap<(i32, i32), Vec<(i32, &'static str)>> =
                    std::collections::BTreeMap::new();
                for (&[x, y, z], blk) in &bp.blocks {
                    if blk.block_id == "minecraft:ladder" {
                        let facing = prop(blk, "facing").unwrap_or("south");
                        ladder_cols.entry((x, z)).or_default().push((y, facing));
                        let dv = dir_vec(facing);
                        let back = [x - dv[0], y, z - dv[2]];
                        let back_ok = bp.blocks.get(&back).is_some_and(|bb| {
                            bb.block_id != "minecraft:air"
                                && !bb.block_id.contains("glass")
                                && !bb.block_id.ends_with("_door")
                                && !bb.block_id.ends_with("_stairs")
                                && !bb.block_id.ends_with("_slab")
                        });
                        if !back_ok {
                            issues.push(format!(
                                "{desc} seed={seed}: ladder at [{x},{y},{z}] facing={facing} lacks solid wall behind it ({back:?})"
                            ));
                        }
                    }
                }
                for ((lx, lz), mut ys) in ladder_cols {
                    ys.sort_by_key(|&(y, _)| y);
                    let (bot_y, face) = ys[0];
                    let &(top_y, _) = ys.last().unwrap();
                    let dv = dir_vec(face);
                    // Front entry at bottom must be passable at bottom & chest height!
                    for ey in [bot_y, bot_y + 1] {
                        let front = [lx + dv[0], ey, lz + dv[2]];
                        if !is_passable(bp.blocks.get(&front)) {
                            let hit = bp.blocks.get(&front).map_or("none", |b| b.block_id.as_str());
                            issues.push(format!(
                                "{desc} seed={seed}: ladder [{lx},{bot_y}..={top_y},{lz}] facing={face} blocked in front {front:?} by {hit}"
                            ));
                        }
                    }
                    // Top of ladder must allow player exit & headroom!
                    let ground_y = i32::from(bp.spec.foundation_height);
                    let is_basement_hatch = bot_y < 0
                        && top_y == ground_y - 1
                        && bp
                            .blocks
                            .get(&[lx, ground_y, lz])
                            .is_some_and(|b| b.block_id.ends_with("_trapdoor"));
                    let can_exit = if is_basement_hatch {
                        let hatch_head = [lx, ground_y + 1, lz];
                        let hatch_head2 = [lx, ground_y + 2, lz];
                        let step_out = [lx + dv[0], ground_y + 1, lz + dv[2]];
                        let step_head = [lx + dv[0], ground_y + 2, lz + dv[2]];
                        let step_floor = [lx + dv[0], ground_y, lz + dv[2]];
                        is_passable(bp.blocks.get(&hatch_head))
                            && is_passable(bp.blocks.get(&hatch_head2))
                            && is_passable(bp.blocks.get(&step_out))
                            && is_passable(bp.blocks.get(&step_head))
                            && bp.blocks.get(&step_floor).is_some_and(|b| b.block_id != "minecraft:air")
                    } else {
                        let above_top = [lx, top_y + 1, lz];
                        let above_top2 = [lx, top_y + 2, lz];
                        let front_top = [lx + dv[0], top_y + 1, lz + dv[2]];
                        let back_top = [lx - dv[0], top_y + 1, lz - dv[2]];
                        let side_l = [lx - dv[2], top_y + 1, lz + dv[0]];
                        let side_r = [lx + dv[2], top_y + 1, lz - dv[0]];
                        is_passable(bp.blocks.get(&above_top))
                            && is_passable(bp.blocks.get(&above_top2))
                            && [front_top, back_top, side_l, side_r].iter().any(|exit_p| {
                                let floor_p = [exit_p[0], exit_p[1] - 1, exit_p[2]];
                                let head_p = [exit_p[0], exit_p[1] + 1, exit_p[2]];
                                is_passable(bp.blocks.get(exit_p))
                                    && is_passable(bp.blocks.get(&head_p))
                                    && bp.blocks.get(&floor_p).is_some_and(|b| b.block_id != "minecraft:air")
                            })
                    };
                    if !can_exit {
                        issues.push(format!(
                            "{desc} seed={seed}: ladder [{lx},{bot_y}..={top_y},{lz}] facing={face} sealed/unexitable at top!"
                        ));
                    }
                }

                // 2) Check interior staircase steps above floor level for dangling/ungrounded steps!
                for (&[x, y, z], blk) in &bp.blocks {
                    if !blk.block_id.ends_with("_stairs") {
                        continue;
                    }
                    let half = prop(blk, "half").unwrap_or("bottom");
                    let min_in = 3;
                    let max_x = bp.width as i32 - 3;
                    let max_z = bp.depth as i32 - 3;
                    let roof_base = i32::from(bp.spec.foundation_height)
                        + i32::from(bp.spec.floors) * i32::from(bp.spec.floor_height);
                    if x >= min_in
                        && x < max_x
                        && z >= min_in
                        && z < max_z
                        && y > i32::from(bp.spec.foundation_height) + 1
                        && y < roof_base
                    {
                        if half == "top" {
                            // Top stairs (stringers, porch brackets, window lintels) must touch a neighboring support!
                            let has_any_support = [
                                [x - 1, y, z],
                                [x + 1, y, z],
                                [x, y, z - 1],
                                [x, y, z + 1],
                                [x, y + 1, z],
                            ]
                            .into_iter()
                            .any(|np| bp.blocks.get(&np).is_some_and(|b| b.block_id != "minecraft:air"));
                            if !has_any_support {
                                issues.push(format!(
                                    "TOP_DETACHED {desc} seed={seed}: dangling top stair {} at [{x},{y},{z}]",
                                    blk.block_id
                                ));
                            }
                            continue;
                        }
                        let has_below = bp.blocks.get(&[x, y - 1, z]).is_some_and(|b| b.block_id != "minecraft:air");
                        let has_lower_step = [
                            [x - 1, y - 1, z],
                            [x + 1, y - 1, z],
                            [x, y - 1, z - 1],
                            [x, y - 1, z + 1],
                        ]
                        .into_iter()
                        .any(|np| bp.blocks.get(&np).is_some_and(|b| b.block_id != "minecraft:air"));
                        let has_same_y_wall = [
                            [x - 1, y, z],
                            [x + 1, y, z],
                            [x, y, z - 1],
                            [x, y, z + 1],
                        ]
                        .into_iter()
                        .any(|np| {
                            bp.blocks
                                .get(&np)
                                .is_some_and(|b| b.block_id != "minecraft:air" && !b.block_id.ends_with("_stairs"))
                        });
                        if !has_below && !has_lower_step && !has_same_y_wall {
                            issues.push(format!(
                                "ORPHAN_STAIR {desc} seed={seed}: orphan/floating interior stair {} at [{x},{y},{z}]",
                                blk.block_id
                            ));
                        }
                        let under_is_opp_top_stringer = bp.blocks.get(&[x, y - 1, z]).is_some_and(|b| {
                            b.block_id.ends_with("_stairs")
                                && prop(b, "half") == Some("top")
                                && prop(b, "facing") != prop(blk, "facing")
                        });
                        let next_step_has_top_stringer = [
                            [x - 1, y + 1, z],
                            [x + 1, y + 1, z],
                            [x, y + 1, z - 1],
                            [x, y + 1, z + 1],
                        ]
                        .into_iter()
                        .any(|np| {
                            bp.blocks.get(&np).is_some_and(|b| b.block_id.ends_with("_stairs"))
                                && bp.blocks.get(&[np[0], y, np[2]]).is_some_and(|ub| {
                                    ub.block_id.ends_with("_stairs")
                                        && prop(ub, "half") == Some("top")
                                        && prop(ub, "facing") != prop(blk, "facing")
                                })
                        });
                        let is_tower_spiral = bp.spec.category == BuildCategory::Tower
                            && [
                                [x - 1, y - 1, z],
                                [x + 1, y - 1, z],
                                [x, y - 1, z - 1],
                                [x, y - 1, z + 1],
                            ]
                            .into_iter()
                            .any(|np| bp.blocks.get(&np).is_some_and(|b| b.block_id.ends_with("_stairs")));
                        let is_flight_step = under_is_opp_top_stringer
                            || (has_below && next_step_has_top_stringer)
                            || is_tower_spiral;
                        if is_flight_step {
                            for cy in [y + 1, y + 2] {
                                if let Some(hb) = bp.blocks.get(&[x, cy, z])
                                    && hb.block_id != "minecraft:air"
                                    && !hb.block_id.ends_with("_stairs")
                                {
                                    issues.push(format!(
                                        "STAIR_HEADROOM {desc} seed={seed}: stair at [{x},{y},{z}] has blocked headroom at y={cy} ({})",
                                        hb.block_id
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut by_tag: std::collections::BTreeMap<&str, Vec<&String>> = std::collections::BTreeMap::new();
        for msg in &issues {
            let tag = msg.split_whitespace().next().unwrap_or("OTHER");
            by_tag.entry(tag).or_default().push(msg);
        }
        let summary: Vec<String> = by_tag
            .iter()
            .map(|(k, v)| {
                let sample: Vec<&str> = v.iter().take(12).map(|s| s.as_str()).collect();
                format!("=== {k} ({}) ===\n{}", v.len(), sample.join("\n"))
            })
            .collect();
        assert!(
            issues.is_empty(),
            "Found {} stair/ladder issues:\n{}",
            issues.len(),
            summary.join("\n\n")
        );
    }

    #[test]
    fn phase5_porticos_modern_penthouse_pagoda_finials_and_bridges_work() {
        // 1. Classical Temple Portico uses quartz columns (not wooden fences) and stepped portico depth.
        let mut temple_req = sample_request("большой античный храм с крыльцом", 5050);
        temple_req.category = BuildCategory::Temple;
        temple_req.style = BuildStyle::Classical;
        temple_req.width = 19;
        temple_req.depth = 17;
        temple_req.height = 16;
        temple_req.palette = MaterialPalette::preset_for_style(BuildStyle::Classical);
        let temple_bp = generate_blueprint(&temple_req, None);
        assert!(
            temple_bp.blocks.values().any(|b| b.block_id == "minecraft:quartz_pillar"),
            "Classical portico temple should place quartz_pillar columns"
        );

        // 2. Modern spacious flat roof gets glass_pane balustrade + rooftop azalea lounge.
        let mut modern_req = sample_request("современная вилла модерн с плоской крышей", 5151);
        modern_req.category = BuildCategory::House;
        modern_req.style = BuildStyle::Modern;
        modern_req.width = 19;
        modern_req.depth = 17;
        modern_req.height = 14;
        modern_req.palette = MaterialPalette::preset_for_style(BuildStyle::Modern);
        let modern_bp = generate_blueprint(&modern_req, None);
        assert!(
            modern_bp.blocks.values().any(|b| b.block_id == "minecraft:glass_pane"),
            "Modern flat-roof villa should frame rooftop balustrade with glass_pane"
        );
        assert!(
            modern_bp.blocks.values().any(|b| b.block_id == "minecraft:potted_azalea_bush"),
            "Modern spacious rooftop should include rooftop potted_azalea_bush lounge"
        );

        // 3. Japanese Pagoda crowns roof summit with metal sorin finial (iron_bars + lightning_rod).
        let mut pagoda_req = sample_request("японская пагода с садом", 5252);
        pagoda_req.category = BuildCategory::House;
        pagoda_req.style = BuildStyle::Japanese;
        pagoda_req.width = 15;
        pagoda_req.depth = 15;
        pagoda_req.height = 18;
        pagoda_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let pagoda_bp = generate_blueprint(&pagoda_req, None);
        assert!(
            pagoda_bp.blocks.values().any(|b| b.block_id == "minecraft:lightning_rod")
                && pagoda_bp.blocks.values().any(|b| b.block_id == "minecraft:iron_bars"),
            "Japanese Pagoda should feature sorin iron_bars + lightning_rod spire finial"
        );

        // 4. Lighthouse lantern includes warm ochre_froglight + sea_lantern core.
        let mut light_req = sample_request("высокий маяк", 5353);
        light_req.category = BuildCategory::Tower;
        light_req.width = 11;
        light_req.depth = 11;
        light_req.height = 22;
        let light_bp = generate_blueprint(&light_req, None);
        assert!(
            light_bp.blocks.values().any(|b| b.block_id == "minecraft:ochre_froglight")
                && light_bp.blocks.values().any(|b| b.block_id == "minecraft:sea_lantern"),
            "Lighthouse chamber should layer ochre_froglight and sea_lantern"
        );

        // 5. Covered Japanese Bridge builds covered timber canopy & smooth slab crest ramps.
        let mut bridge_req = sample_request("японский крытый мост через реку с крыльцом", 5454);
        bridge_req.category = BuildCategory::Bridge;
        bridge_req.style = BuildStyle::Japanese;
        bridge_req.width = 9;
        bridge_req.depth = 19;
        bridge_req.height = 14;
        bridge_req.palette = MaterialPalette::preset_for_style(BuildStyle::Japanese);
        let bridge_bp = generate_blueprint(&bridge_req, None);
        assert!(
            bridge_bp.blocks.values().any(|b| b.block_id.ends_with("_slab") && b.props.iter().any(|&(k, v)| k == "type" && v == "bottom")),
            "Covered arched bridge should place walkable bottom slab ramps & roof canopy"
        );
    }

    #[test]
    fn phase6_gui_persistence_stilts_gradients_and_settlements_work() {
        use crate::persist::{
            MatFilter, SavedPreset, apply_block_to_slot, decode_share_code, encode_share_code,
            filtered_slot_choices, parse_favorites, serialize_favorites,
        };

        // 1. Persistence, Share-Codes & Bilingual Material Filter 2.0.
        let preset = SavedPreset {
            name: "Римский квартал".into(),
            width: 56,
            depth: 52,
            height: 24,
            category: BuildCategory::Settlement,
            style: BuildStyle::Classical,
            roof_raw: 1,
            toggles: [1, 2, 0, 1, 1],
            noise_level: NoiseLevel::Wild,
            adapt_terrain: true,
            air_only: true,
            wear_pct: 25,
            overgrowth_pct: 40,
            description: "квартал античный с колоннами и садом".into(),
            seed: 987_654_321,
            palette_manual: true,
            palette: MaterialPalette::preset_for_style(BuildStyle::Classical),
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 4,
        };
        let code = encode_share_code(&preset);
        assert!(code.starts_with("BG2:"), "Share code must use BG2 prefix");
        let decoded = decode_share_code(&code, Some(&preset.name)).expect("valid share code roundtrip");
        assert_eq!(decoded, preset);
        assert!(decode_share_code("BG2:CORRUPTED_DATA", None).is_err());

        let fav_text = serialize_favorites(&[preset.clone()]);
        let parsed_favs = parse_favorites(&fav_text);
        assert_eq!(parsed_favs, vec![preset]);

        let copper_roofs = filtered_slot_choices(4, MatFilter::Special, "медь");
        assert!(
            copper_roofs.iter().any(|id| id == "minecraft:cut_copper"),
            "RU query 'медь' should find copper roof choices: {copper_roofs:?}"
        );
        let mut sync_pal = MaterialPalette::preset(0);
        apply_block_to_slot(&mut sync_pal, 1, "minecraft:cherry_planks", true);
        assert_eq!(sync_pal.door, "minecraft:cherry_door");
        assert_eq!(sync_pal.fence, "minecraft:cherry_fence");

        // 2. Water Stilts & Pier Adaptation.
        let mut stilt_req = sample_request("рыбацкий коттедж с крыльцом", 6161);
        stilt_req.width = 15;
        stilt_req.depth = 15;
        stilt_req.height = 12;
        let cols = usize::from(stilt_req.width) * usize::from(stilt_req.depth);
        let offsets = vec![0i32; cols];
        let water_depths = vec![4i32; cols];
        let stilt_bp = crate::generator::generate_blueprint_with_water(
            &stilt_req,
            Some(&offsets),
            Some(&water_depths),
        );
        assert!(stilt_bp.spec.on_stilts, "House over water should activate stilt pier mode");
        assert!(!stilt_bp.spec.basement, "Underwater stilts should disable flooded basement");
        let deep_piers = stilt_bp
            .blocks
            .iter()
            .filter(|&(&[_, y, _], b)| y <= -3 && b.block_id != "minecraft:air")
            .count();
        assert!(deep_piers > 0 && deep_piers < 90, "Expected open underwater pilings (not solid cube): {deep_piers}");

        // 3. Floor-by-Floor Material Overrides & Weathering Gradients.
        let mut split_req = sample_request("двухэтажный дом, 1 этаж из булыжника, 2 этаж из тёмного дуба", 6262);
        split_req.width = 15;
        split_req.depth = 13;
        split_req.height = 14;
        split_req.palette = crate::parser::infer_palette_with_style_and_seed(
            BuildStyle::Medieval,
            &split_req.description,
            split_req.seed as u64,
        );
        let split_bp = generate_blueprint(&split_req, None);
        assert!(split_bp.spec.floors >= 2);
        assert!(
            split_bp.blocks.iter().any(|(&[_, y, _], b)| y <= 4 && (b.block_id == "minecraft:cobblestone" || b.block_id == "minecraft:mossy_cobblestone")),
            "1st floor should keep cobblestone masonry"
        );
        assert!(
            split_bp.blocks.iter().any(|(&[_, y, _], b)| y >= 6 && b.block_id == "minecraft:dark_oak_planks"),
            "2nd floor should switch to dark_oak_planks"
        );

        let mut grad_req = sample_request("древняя каменная башня с мхом", 6363);
        grad_req.category = BuildCategory::Tower;
        grad_req.width = 11;
        grad_req.depth = 11;
        grad_req.height = 18;
        grad_req.palette = crate::parser::infer_palette_with_style_and_seed(
            BuildStyle::Medieval,
            &grad_req.description,
            grad_req.seed as u64,
        );
        let grad_bp = generate_blueprint(&grad_req, None);
        assert!(
            grad_bp.blocks.values().any(|b| b.block_id == "minecraft:mossy_stone_bricks"),
            "Weathered stone walls should include mossy_stone_bricks base gradient"
        );
        assert!(
            grad_bp.blocks.values().any(|b| b.block_id == "minecraft:cracked_stone_bricks"),
            "Weathered stone walls should include cracked_stone_bricks accent gradient"
        );

        // 4. Settlement / Village Mode (`<= 96x96`) with multiple oriented buildings & fountain.
        let mut village_req = sample_request("средневековый квартал деревня с площадью", 6464);
        village_req.category = BuildCategory::Settlement;
        village_req.style = BuildStyle::Medieval;
        village_req.width = 56;
        village_req.depth = 56;
        village_req.height = 22;
        village_req.palette = MaterialPalette::preset(1);
        let village_bp = generate_blueprint(&village_req, None);
        assert_eq!(village_bp.width, 56);
        assert_eq!(village_bp.depth, 56);
        assert!(village_bp.non_air_count() > 2000, "Village should populate multi-building town: {}", village_bp.non_air_count());
        assert!(
            village_bp.blocks.values().any(|b| b.block_id == "minecraft:water")
                && village_bp.blocks.values().any(|b| b.block_id.ends_with("_door")),
            "Village should include plaza fountain water and multiple building entrances"
        );
    }

    #[test]
    fn phase7_fixes_roof_doors_windows_and_30_slot_materials() {
        use crate::persist::{
            MatFilter, SavedPreset, apply_block_to_slot, decode_share_code, encode_share_code,
            filtered_slot_choices,
        };

        // 1. All 30 UI material slots have valid expanded choice catalogs & RU/EN search.
        assert!(MaterialPalette::WALL_CHOICES.len() >= 70);
        assert!(MaterialPalette::FOUNDATION_CHOICES.len() >= 40);
        assert!(MaterialPalette::ROOF_CHOICES.len() >= 45);
        for slot in 0..30 {
            let all = filtered_slot_choices(slot, MatFilter::All, "");
            assert!(
                !all.is_empty(),
                "Material slot {slot} must return candidate choices"
            );
            for id in &all {
                assert!(id.starts_with("minecraft:"), "Invalid ID in slot {slot}: {id}");
            }
        }
        assert!(filtered_slot_choices(23, MatFilter::All, "").len() >= 45);
        assert!(filtered_slot_choices(27, MatFilter::All, "").len() >= 18);
        assert!(filtered_slot_choices(28, MatFilter::All, "").len() >= 30);
        assert!(filtered_slot_choices(29, MatFilter::All, "").len() >= 18);
        assert!(
            filtered_slot_choices(23, MatFilter::All, "медь")
                .iter()
                .any(|id| id == "minecraft:cut_copper_stairs")
        );
        assert!(
            filtered_slot_choices(27, MatFilter::All, "мох")
                .iter()
                .any(|id| id == "minecraft:moss_carpet")
        );

        // 2. All 30 slots apply cleanly & roundtrip via Share-Code BG2 v2.
        let mut custom_pal = MaterialPalette::preset(0);
        apply_block_to_slot(&mut custom_pal, 12, "minecraft:yellow_terracotta", false);
        apply_block_to_slot(&mut custom_pal, 14, "minecraft:cherry_log", false);
        apply_block_to_slot(&mut custom_pal, 15, "minecraft:purpur_pillar", false);
        apply_block_to_slot(&mut custom_pal, 16, "minecraft:cut_copper", false);
        apply_block_to_slot(&mut custom_pal, 17, "minecraft:birch_planks", false);
        apply_block_to_slot(&mut custom_pal, 18, "minecraft:dark_oak_slab", false);
        apply_block_to_slot(&mut custom_pal, 19, "minecraft:magenta_stained_glass_pane", false);
        apply_block_to_slot(&mut custom_pal, 20, "minecraft:crimson_door", false);
        apply_block_to_slot(&mut custom_pal, 22, "minecraft:bamboo_fence_gate", false);
        apply_block_to_slot(&mut custom_pal, 23, "minecraft:cut_copper_stairs", false);
        apply_block_to_slot(&mut custom_pal, 24, "minecraft:tuff_bricks", false);
        apply_block_to_slot(&mut custom_pal, 25, "minecraft:cherry_leaves", false);
        apply_block_to_slot(&mut custom_pal, 26, "minecraft:copper_torch", false);
        apply_block_to_slot(&mut custom_pal, 27, "minecraft:magenta_carpet", false);
        apply_block_to_slot(&mut custom_pal, 28, "minecraft:smooth_red_sandstone", false);
        apply_block_to_slot(&mut custom_pal, 29, "minecraft:cyan_wool", false);

        let p = SavedPreset {
            name: "Тест 30 слотов".into(),
            width: 17,
            depth: 15,
            height: 16,
            category: BuildCategory::House,
            style: BuildStyle::Medieval,
            roof_raw: 1,
            toggles: [1, 1, 1, 1, 1],
            noise_level: NoiseLevel::Balanced,
            adapt_terrain: true,
            air_only: false,
            wear_pct: 0,
            overgrowth_pct: 0,
            description: "двухэтажная уютная таверна лавка с крыльцом и садом".into(),
            seed: 4242,
            palette_manual: true,
            palette: custom_pal.clone(),
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 4,
        };
        let code = encode_share_code(&p);
        let decoded = decode_share_code(&code, Some(&p.name)).expect("BG2 v2 share code roundtrip");
        assert_eq!(decoded.palette, custom_pal);

        // Customized extended slots render into generated building blocks.
        let mut custom_req = sample_request("двухэтажная уютная таверна лавка с крыльцом и садом", 7171);
        custom_req.width = 21;
        custom_req.depth = 19;
        custom_req.height = 16;
        custom_req.style = BuildStyle::Cottage;
        custom_req.palette = custom_pal.clone();
        let custom_bp = generate_blueprint(&custom_req, None);
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:yellow_terracotta"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:crimson_door"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:magenta_carpet"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:smooth_red_sandstone"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:cut_copper_stairs"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:cyan_wool"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:copper_wall_torch"));
        assert!(custom_bp.blocks.values().any(|b| b.block_id == "minecraft:cherry_log"));

        // 3. Regressions across 8 seeds: properly jambed doors, no 1x1 pier/gable window shards, no roof-step holes!
        let mut checked_doors = 0usize;
        let is_solid_jamb = |bp: &crate::generator::GeneratedBlueprint, x: i32, y: i32, z: i32| -> bool {
            bp.blocks.get(&[x, y, z]).is_some_and(|b| {
                let id = b.block_id.as_str();
                id != "minecraft:air"
                    && id != "minecraft:ladder"
                    && !id.contains("glass")
                    && !id.ends_with("_door")
            })
        };
        let is_clear_pass = |bp: &crate::generator::GeneratedBlueprint, x: i32, y: i32, z: i32| -> bool {
            bp.blocks.get(&[x, y, z]).is_none_or(|b| {
                let id = b.block_id.as_str();
                id == "minecraft:air" || id.ends_with("_carpet") || id == "minecraft:torch"
            })
        };

        for seed in 100..=107 {
            let mut req = sample_request("двухэтажный просторный дом с крыльцом и садом", seed);
            req.width = 17;
            req.depth = 15;
            req.height = 17;
            req.style = BuildStyle::Medieval;
            req.palette = MaterialPalette::preset_for_style(BuildStyle::Medieval);
            let bp = generate_blueprint(&req, None);

            // (a) Every lower door must have solid wall jambs on both left/right flanks AND clear passage!
            for (&[x, y, z], blk) in &bp.blocks {
                if blk.block_id.ends_with("_door")
                    && blk.props.iter().any(|&(k, v)| k == "half" && v == "lower")
                {
                    checked_doors += 1;
                    let along_x_jambs = (is_solid_jamb(&bp, x - 1, y, z)
                        || bp.blocks.get(&[x - 1, y, z]).is_some_and(|b| b.block_id.ends_with("_door")))
                        && (is_solid_jamb(&bp, x + 1, y, z)
                            || bp.blocks.get(&[x + 1, y, z]).is_some_and(|b| b.block_id.ends_with("_door")));
                    let along_z_jambs = (is_solid_jamb(&bp, x, y, z - 1)
                        || bp.blocks.get(&[x, y, z - 1]).is_some_and(|b| b.block_id.ends_with("_door")))
                        && (is_solid_jamb(&bp, x, y, z + 1)
                            || bp.blocks.get(&[x, y, z + 1]).is_some_and(|b| b.block_id.ends_with("_door")));
                    assert!(
                        along_x_jambs || along_z_jambs,
                        "Seed {seed}: Door at ({x},{y},{z}) lacks solid flanking jambs"
                    );
                    if blk.props.iter().any(|&(k, v)| k == "open" && v == "true") {
                        let pass_z = is_clear_pass(&bp, x, y, z - 1) && is_clear_pass(&bp, x, y, z + 1);
                        let pass_x = is_clear_pass(&bp, x - 1, y, z) && is_clear_pass(&bp, x + 1, y, z);
                        assert!(
                            (along_x_jambs && pass_z) || (along_z_jambs && pass_x),
                            "Seed {seed}: Open interior door at ({x},{y},{z}) obstructed by wall/ladder"
                        );
                    }
                }
            }

            // (b) No lone glass block trapped inside a vertical frame pillar (`y-1 == frame && y+1 == frame`).
            let frame_id = req.palette.frame.as_str();
            for (&[x, y, z], blk) in &bp.blocks {
                if blk.block_id.contains("glass") {
                    let below_frame = bp.blocks.get(&[x, y - 1, z]).is_some_and(|b| b.block_id == frame_id);
                    let above_frame = bp.blocks.get(&[x, y + 1, z]).is_some_and(|b| b.block_id == frame_id);
                    assert!(
                        !(below_frame && above_frame),
                        "Seed {seed}: Glass shard trapped between frame logs at ({x},{y},{z})"
                    );
                }
            }

            // (c) Attic roof enclosure: above roof_base_y + 1, no interior attic air voxel inside the
            // structural roof/gable envelope should horizontally see outside sky through an unsealed step drop.
            let roof_base_y = i32::from(bp.spec.foundation_height)
                + i32::from(bp.spec.floors) * i32::from(bp.spec.floor_height);
            let is_roof_envelope = |id: &str| -> bool {
                id == req.palette.roof_block
                    || id == req.palette.wall
                    || id == req.palette.wall_upper
                    || id == req.palette.wall_accent
                    || id == req.palette.frame
                    || id == req.palette.roof_stairs
                    || id == req.palette.roof_slab
                    || id.ends_with("_stairs")
                    || id.ends_with("_slab")
            };
            let mut col_roof_top: BTreeMap<(i32, i32), i32> = BTreeMap::new();
            for (&[x, y, z], blk) in &bp.blocks {
                if y >= roof_base_y && is_roof_envelope(blk.block_id.as_str()) {
                    let e = col_roof_top.entry((x, z)).or_insert(y);
                    if y > *e {
                        *e = y;
                    }
                }
            }
            for (&(x, z), &top_y) in &col_roof_top {
                // Only inspect columns inside the actual building attic (solid floor/plate at roof_base_y),
                // skipping exterior 1-block overhanging eaves outside the walls.
                if bp.blocks.get(&[x, roof_base_y, z]).is_none_or(|b| b.block_id == "minecraft:air") {
                    continue;
                }
                for y in (roof_base_y + 1)..top_y {
                    let is_air_here = bp.blocks.get(&[x, y, z]).is_none_or(|b| b.block_id == "minecraft:air");
                    if !is_air_here {
                        continue;
                    }
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let nb_top = col_roof_top.get(&(x + dx, z + dz)).copied().unwrap_or(0);
                        if nb_top < y {
                            let nb_cur_air = bp.blocks.get(&[x + dx, y, z + dz]).is_none_or(|b| b.block_id == "minecraft:air");
                            let nb_above_air = bp.blocks.get(&[x + dx, y + 1, z + dz]).is_none_or(|b| b.block_id == "minecraft:air");
                            let col_dump: Vec<(i32, &str)> = (roof_base_y..=top_y)
                                .map(|yy| {
                                    (
                                        yy,
                                        bp.blocks
                                            .get(&[x, yy, z])
                                            .map(|b| b.block_id.as_str())
                                            .unwrap_or("air"),
                                    )
                                })
                                .collect();
                            let nb_cols: Vec<_> = [(6,12),(7,11),(7,12),(8,12),(7,13)]
                                .into_iter()
                                .map(|(cx, cz)| {
                                    let items: Vec<_> = (7..=13)
                                        .map(|yy| (yy, bp.blocks.get(&[cx, yy, cz]).map(|b| b.block_id.as_str()).unwrap_or("air")))
                                        .collect();
                                    ((cx, cz), items)
                                })
                                .collect();
                            assert!(
                                !(nb_cur_air && nb_above_air),
                                "Seed {seed} ({:?}, dormer={}, turret={}): Unsealed attic roof gap at ({x},{y},{z}) [top={top_y}] facing ({},{}) [nb_top={nb_top}, roof_base_y={roof_base_y}] col={col_dump:?} nbs={nb_cols:?}",
                                bp.spec.roof_style,
                                bp.spec.dormers,
                                bp.spec.corner_turret,
                                x + dx,
                                z + dz
                            );
                        }
                    }
                }
            }
        }
        for seed in 200..=205 {
            let mut g_req = sample_request("двухэтажный большой дом с двускатной крышей и флигелем", seed);
            g_req.width = 19;
            g_req.depth = 17;
            g_req.height = 18;
            g_req.style = BuildStyle::Cottage;
            let g_bp = generate_blueprint(&g_req, None);
            let roof_base_y = i32::from(g_bp.spec.foundation_height)
                + i32::from(g_bp.spec.floors) * i32::from(g_bp.spec.floor_height);
            let mut col_top: BTreeMap<(i32, i32), i32> = BTreeMap::new();
            for (&[x, y, z], blk) in &g_bp.blocks {
                if y >= roof_base_y
                    && blk.block_id != "minecraft:air"
                    && blk.block_id != g_req.palette.chimney_block
                    && !blk.block_id.ends_with("_trapdoor")
                    && !blk.block_id.ends_with("_leaves")
                {
                    let e = col_top.entry((x, z)).or_insert(y);
                    if y > *e {
                        *e = y;
                    }
                }
            }
            for (&(x, z), &top_y) in &col_top {
                if g_bp.blocks.get(&[x, roof_base_y, z]).is_none_or(|b| b.block_id == "minecraft:air") {
                    continue;
                }
                for y in (roof_base_y + 1)..top_y {
                    if g_bp.blocks.get(&[x, y, z]).is_none_or(|b| b.block_id == "minecraft:air") {
                        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            let nb_top = col_top.get(&(x + dx, z + dz)).copied().unwrap_or(0);
                            if nb_top < y {
                                let nb_cur_air = g_bp.blocks.get(&[x + dx, y, z + dz]).is_none_or(|b| b.block_id == "minecraft:air");
                                let nb_above_air = g_bp.blocks.get(&[x + dx, y + 1, z + dz]).is_none_or(|b| b.block_id == "minecraft:air");
                                assert!(
                                    !(nb_cur_air && nb_above_air),
                                    "Gable seed {seed}: Unsealed step/gable gap at ({x},{y},{z}) facing ({},{})",
                                    x + dx,
                                    z + dz
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(checked_doors >= 16, "Expected doors checked across 8 multi-story seeds, got {checked_doors}");
    }

    #[test]
    fn test_material_palette_block_lists_expand_shrink_and_mix_modes() {
        use std::collections::BTreeSet;
        use crate::model::MixMode;

        let mut pal = MaterialPalette::preset(0);
        assert_eq!(pal.slot_blocks(MaterialPalette::SLOT_WALL).len(), 1);
        assert!(!pal.remove_slot_block_at(MaterialPalette::SLOT_WALL, 0));

        assert!(pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:mossy_stone_bricks"));
        assert!(pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:andesite"));
        // Weighted repeat entry allowed up to MAX_BLOCKS_PER_SLOT
        assert!(pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:mossy_stone_bricks"));
        assert_eq!(pal.slot_blocks(MaterialPalette::SLOT_WALL).len(), 4);
        assert!(pal.remove_slot_block_at(MaterialPalette::SLOT_WALL, 3));
        assert_eq!(pal.slot_blocks(MaterialPalette::SLOT_WALL).len(), 3);

        // Replace middle entry and test shrinking back down to 1 minimum
        pal.replace_slot_block_at(MaterialPalette::SLOT_WALL, 1, "minecraft:cracked_stone_bricks");
        assert_eq!(
            pal.slot_blocks(MaterialPalette::SLOT_WALL)[1],
            "minecraft:cracked_stone_bricks"
        );
        for mode in [MixMode::Patches, MixMode::Scatter, MixMode::Stripes, MixMode::Checker] {
            pal.mix_mode = mode;
            let mut seen = BTreeSet::new();
            for u in 0..10 {
                for y in 0..6 {
                    for v in 0..10 {
                        seen.insert(pal.pick(MaterialPalette::SLOT_WALL, 42, u, y, v));
                    }
                }
            }
            assert!(
                seen.len() >= 2,
                "MixMode {:?} should emit multiple materials from slot list, got {:?}",
                mode,
                seen
            );
        }

        assert!(pal.remove_slot_block_at(MaterialPalette::SLOT_WALL, 1));
        assert!(pal.remove_slot_block_at(MaterialPalette::SLOT_WALL, 1));
        assert_eq!(pal.slot_blocks(MaterialPalette::SLOT_WALL).len(), 1);
        assert!(!pal.remove_slot_block_at(MaterialPalette::SLOT_WALL, 0));

        // Verify upper cap MAX_BLOCKS_PER_SLOT
        for i in 1..MaterialPalette::MAX_BLOCKS_PER_SLOT {
            assert!(
                pal.add_slot_block(MaterialPalette::SLOT_WALL, &format!("minecraft:custom_block_{i}")),
                "Expected add up to cap at {i}"
            );
        }
        assert_eq!(
            pal.slot_blocks(MaterialPalette::SLOT_WALL).len(),
            MaterialPalette::MAX_BLOCKS_PER_SLOT
        );
        assert!(!pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:overflow_block"));
    }

    #[test]
    fn test_share_code_v3_and_picker_filtering() {
        use crate::model::MixMode;
        use crate::persist::{
            MatFilter, SavedPreset, add_block_to_slot, decode_share_code, encode_share_code,
            filtered_picker_choices, parse_favorites, remove_block_from_slot_at,
            replace_block_in_slot_at, serialize_favorites, toggle_block_in_slot,
        };

        let mut pal = MaterialPalette::preset(1);
        pal.mix_mode = MixMode::Checker;
        assert!(add_block_to_slot(&mut pal, MaterialPalette::SLOT_WALL, "minecraft:calcite", false));
        assert!(add_block_to_slot(
            &mut pal,
            MaterialPalette::SLOT_WALL_UPPER,
            "minecraft:birch_planks",
            false
        ));
        assert!(add_block_to_slot(&mut pal, MaterialPalette::SLOT_ROOF, "minecraft:blackstone", false));
        assert!(add_block_to_slot(
            &mut pal,
            MaterialPalette::SLOT_AWNING,
            "minecraft:cyan_wool",
            false
        ));

        let preset = SavedPreset {
            name: "ListMixTest".into(),
            width: 15,
            depth: 13,
            height: 14,
            category: BuildCategory::House,
            style: BuildStyle::Medieval,
            roof_raw: 1,
            toggles: [0, 1, 2, 0, 1],
            noise_level: NoiseLevel::Balanced,
            adapt_terrain: true,
            air_only: false,
            wear_pct: 0,
            overgrowth_pct: 0,
            description: "дом с разными блоками".into(),
            seed: 987654,
            palette_manual: true,
            palette: pal.clone(),
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 4,
        };
        let code = encode_share_code(&preset);
        assert!(code.starts_with("BG2"));
        let decoded = decode_share_code(&code, None).expect("decode BG2 v3 share code");
        assert_eq!(decoded.palette.mix_mode, MixMode::Checker);
        assert_eq!(
            decoded.palette.slot_blocks(MaterialPalette::SLOT_WALL),
            pal.slot_blocks(MaterialPalette::SLOT_WALL)
        );
        assert_eq!(
            decoded.palette.slot_blocks(MaterialPalette::SLOT_WALL_UPPER),
            pal.slot_blocks(MaterialPalette::SLOT_WALL_UPPER)
        );
        assert_eq!(
            decoded.palette.slot_blocks(MaterialPalette::SLOT_AWNING),
            pal.slot_blocks(MaterialPalette::SLOT_AWNING)
        );

        let fav_raw = serialize_favorites(&[decoded]);
        let fav_back = parse_favorites(&fav_raw);
        assert_eq!(fav_back.len(), 1);
        assert_eq!(fav_back[0].palette.mix_mode, MixMode::Checker);

        // Curated vs All-World block catalog + RU/EN filter search
        let world_blocks = vec![
            "minecraft:tuff_bricks".to_owned(),
            "testmod:custom_beam".to_owned(),
            "minecraft:mossy_stone_bricks".to_owned(),
        ];
        let all_tuff = filtered_picker_choices(
            MaterialPalette::SLOT_WALL,
            true,
            &world_blocks,
            MatFilter::All,
            "tuFf",
        );
        assert!(all_tuff.contains(&"minecraft:tuff_bricks".to_owned()));
        let ru_slate = filtered_picker_choices(
            MaterialPalette::SLOT_WALL,
            true,
            &world_blocks,
            MatFilter::Stone,
            "сланец",
        );
        assert!(!ru_slate.is_empty());
        let mod_custom = filtered_picker_choices(
            MaterialPalette::SLOT_WALL,
            true,
            &world_blocks,
            MatFilter::All,
            "custom_beam",
        );
        assert_eq!(mod_custom, vec!["testmod:custom_beam".to_owned()]);

        // Persist UI mutators (`toggle` returns false when removed)
        assert!(!toggle_block_in_slot(&mut pal, MaterialPalette::SLOT_WALL, "minecraft:calcite", false));
        assert!(!pal.slot_contains(MaterialPalette::SLOT_WALL, "minecraft:calcite"));
        assert!(add_block_to_slot(&mut pal, MaterialPalette::SLOT_WALL, "minecraft:granite", false));
        replace_block_in_slot_at(&mut pal, MaterialPalette::SLOT_WALL, 1, "minecraft:diorite", false);
        assert!(pal.slot_contains(MaterialPalette::SLOT_WALL, "minecraft:diorite"));
        assert!(remove_block_from_slot_at(&mut pal, MaterialPalette::SLOT_WALL, 1));
    }

    #[test]
    fn test_multi_block_list_generation_and_door_consistency() {
        use std::collections::BTreeSet;
        use crate::model::MixMode;

        for mode in [MixMode::Patches, MixMode::Scatter, MixMode::Stripes, MixMode::Checker] {
            let mut req = sample_request("двухэтажный уютный дом с крыльцом", 777 + i64::from(mode.to_idx()));
            req.width = 15;
            req.depth = 13;
            req.height = 14;
            let pal = &mut req.palette;
            pal.mix_mode = mode;
            pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:birch_planks");
            pal.add_slot_block(MaterialPalette::SLOT_WALL, "minecraft:spruce_planks");
            pal.add_slot_block(MaterialPalette::SLOT_ROOF, "minecraft:cobbled_deepslate");
            pal.add_slot_block(MaterialPalette::SLOT_ROOF, "minecraft:blackstone");
            pal.add_slot_block(MaterialPalette::SLOT_DOOR, "minecraft:spruce_door");
            pal.add_slot_block(MaterialPalette::SLOT_DOOR, "minecraft:dark_oak_door");
            pal.add_slot_block(MaterialPalette::SLOT_FENCE, "minecraft:spruce_fence");

            let bp = generate_blueprint(&req, None);
            let mut wall_ids_seen = BTreeSet::new();
            let mut roof_ids_seen = BTreeSet::new();
            let mut doors_found = 0usize;

            for (&[x, y, z], blk) in &bp.blocks {
                if req.palette.slot_contains(MaterialPalette::SLOT_WALL, &blk.block_id) {
                    wall_ids_seen.insert(blk.block_id.clone());
                }
                if req.palette.slot_contains(MaterialPalette::SLOT_ROOF, &blk.block_id)
                    || blk.block_id.contains("deepslate_tile")
                    || blk.block_id.contains("cobbled_deepslate")
                    || blk.block_id.contains("blackstone")
                {
                    roof_ids_seen.insert(blk.block_id.clone());
                }
                if blk.block_id.ends_with("_door")
                    && blk.props.iter().any(|(k, v)| *k == "half" && *v == "lower")
                {
                    let upper = bp
                        .blocks
                        .get(&[x, y + 1, z])
                        .expect("Upper half of door must exist");
                    assert_eq!(
                        upper.block_id, blk.block_id,
                        "Upper & lower door halves must share exact block_id at ({x},{y},{z}) in {:?}",
                        mode
                    );
                    doors_found += 1;
                }
            }
            assert!(doors_found >= 1, "Expected >=1 doorway in {:?}", mode);
            assert!(
                wall_ids_seen.len() >= 2,
                "Expected multiple wall materials generated in {:?}, got {:?}",
                mode,
                wall_ids_seen
            );
            assert!(
                roof_ids_seen.len() >= 2,
                "Expected multiple roof materials generated in {:?}, got {:?}",
                mode,
                roof_ids_seen
            );
        }
    }

    #[test]
    fn phase9_bridge_diversity_full_scale_and_no_clones() {
        use std::collections::BTreeSet;
        use crate::parser::{BridgeVariant, infer_palette_with_style_and_seed};

        // Reproduces exact user screenshot scenario: Gothic arched bridge at 21x70x12 across different seeds.
        let seeds = [2_504_692_i64, 600_562, 101_010, 202_020, 303_030, 404_040, 505_050, 606_060];
        let mut variants_seen = BTreeSet::new();
        let mut fingerprints: Vec<(i64, usize, BTreeMap<[i32; 3], String>)> = Vec::new();

        for &seed in &seeds {
            let mut req = sample_request("арочный мост", seed);
            req.category = BuildCategory::Bridge;
            req.style = BuildStyle::Gothic;
            req.width = 21;
            req.depth = 70;
            req.height = 12;
            req.palette = infer_palette_with_style_and_seed(BuildStyle::Gothic, &req.description, seed as u64);

            let bp = generate_blueprint(&req, None);
            variants_seen.insert(format!("{:?}", bp.spec.bridge_variant));

            let min_x = bp.blocks.iter().filter(|(_, b)| b.block_id != "minecraft:air").map(|(p, _)| p[0]).min().unwrap_or(0);
            let max_x = bp.blocks.iter().filter(|(_, b)| b.block_id != "minecraft:air").map(|(p, _)| p[0]).max().unwrap_or(0);
            let span_x = max_x - min_x + 1;
            assert!(
                span_x >= 11,
                "Expected 21-wide bridge (seed {seed}) to scale past old 9-block cap, got span_x={span_x}"
            );

            let map: BTreeMap<[i32; 3], String> = bp
                .blocks
                .iter()
                .filter(|(_, b)| b.block_id != "minecraft:air")
                .map(|(&p, b)| (p, b.block_id.clone()))
                .collect();
            assert!(map.len() > 800, "Bridge seed {seed} too empty: {}", map.len());
            fingerprints.push((seed, map.len(), map));
        }

        assert!(
            variants_seen.len() >= 3,
            "Expected 'арочный мост' to produce >=3 distinct bridge archetypes across seeds, got {:?}",
            variants_seen
        );

        let (_, _, ref base_map) = fingerprints[0];
        let (_, _, ref second_map) = fingerprints[1];
        let mut diff_count = 0usize;
        for (pos, id) in base_map {
            if second_map.get(pos) != Some(id) {
                diff_count += 1;
            }
        }
        for pos in second_map.keys() {
            if !base_map.contains_key(pos) {
                diff_count += 1;
            }
        }
        assert!(
            diff_count >= base_map.len() / 4,
            "Expected >=25% structural diff between seeds 2504692 and 600562, got only {diff_count} blocks (base={})",
            base_map.len()
        );

        // Verify all explicit specialized BridgeVariants produce rich distinct builds.
        for (prompt, expected_variant, w, d, h) in [
            ("вантовый подвесной мост с пилонами и канатами", BridgeVariant::Suspension, 19, 44, 18),
            ("римский двухъярусный акведук мост с каналом воды", BridgeVariant::Aqueduct, 15, 42, 16),
            ("японский горбатый лунный мост тайко с воротами тории", BridgeVariant::TaikoMoon, 13, 29, 14),
            ("крепостной разводной мост со сторожевыми башнями", BridgeVariant::FortifiedGate, 15, 36, 16),
            ("деревянный понтонный пирс пристань мост", BridgeVariant::PontoonPier, 13, 30, 12),
        ] {
            let mut req = sample_request(prompt, 7777);
            req.category = BuildCategory::Bridge;
            req.width = w;
            req.depth = d;
            req.height = h;
            let bp = generate_blueprint(&req, None);
            assert_eq!(bp.spec.bridge_variant, expected_variant);
            assert!(
                bp.non_air_count() > 400,
                "Bridge variant {:?} too small: {}",
                expected_variant,
                bp.non_air_count()
            );
            assert!(
                !bp.blocks.values().any(|b| b.block_id == "minecraft:chain"),
                "Found invalid legacy minecraft:chain in {:?}",
                expected_variant
            );
        }
    }

    #[test]
    fn phase9_new_landmarks_and_all_28_presets_generate_cleanly() {
        use std::collections::BTreeSet;
        use crate::parser::{StructureTheme, infer_palette_with_style_and_seed};

        let mut themes_seen = BTreeSet::new();
        for &(slug, cat, style, w, d, h, desc) in CLI_PRESETS {
            let mut req = sample_request(desc, 9191);
            req.category = cat;
            req.style = style;
            req.width = w;
            req.depth = d;
            req.height = h;
            req.palette = infer_palette_with_style_and_seed(style, desc, 9191);
            let bp = generate_blueprint(&req, None);
            themes_seen.insert(format!("{:?}", bp.spec.theme));
            assert!(
                bp.non_air_count() > 200,
                "Preset '{slug}' generated too few blocks: {}",
                bp.non_air_count()
            );
            assert!(
                !bp.blocks.values().any(|b| b.block_id == "minecraft:chain"),
                "Preset '{slug}' emitted forbidden minecraft:chain"
            );
            for b in bp.blocks.values() {
                if b.block_id.ends_with("_leaves") {
                    assert!(
                        b.props.iter().any(|&(k, v)| k == "persistent" && v == "true"),
                        "Preset '{slug}' emitted non-persistent leaves"
                    );
                }
            }
        }

        for expected in [
            StructureTheme::Pyramid,
            StructureTheme::TriumphalArch,
            StructureTheme::Observatory,
            StructureTheme::ClockTower,
            StructureTheme::Watermill,
            StructureTheme::Cathedral,
            StructureTheme::JapaneseCastle,
            StructureTheme::Bazaar,
            StructureTheme::Treehouse,
            StructureTheme::OnsenBath,
        ] {
            assert!(
                themes_seen.contains(&format!("{expected:?}")),
                "Expected theme {:?} to be exercised by presets",
                expected
            );
        }

        // Clocktower and Cathedral both include a hanging bell.
        for slug in ["clocktower", "cathedral"] {
            let p = find_cli_preset(slug).unwrap();
            let mut req = sample_request(p.6, 5151);
            req.category = p.1;
            req.style = p.2;
            req.width = p.3;
            req.depth = p.4;
            req.height = p.5;
            let bp = generate_blueprint(&req, None);
            assert!(
                bp.blocks.values().any(|b| b.block_id == "minecraft:bell"),
                "Expected minecraft:bell inside preset '{slug}'"
            );
        }

        // Canal + Arched Bridge & Varied Settlement Layouts across seeds.
        let mut saw_water_canal_or_fountain = false;
        let mut town_counts = Vec::new();
        for seed in 10..18 {
            let mut req = sample_request("большой город квартал", seed);
            req.category = BuildCategory::Settlement;
            req.width = 48;
            req.depth = 48;
            req.height = 20;
            let bp = generate_blueprint(&req, None);
            if bp.blocks.values().any(|b| b.block_id == "minecraft:water") {
                saw_water_canal_or_fountain = true;
            }
            town_counts.push(bp.non_air_count());
        }
        assert!(saw_water_canal_or_fountain);
        let min_c = *town_counts.iter().min().unwrap();
        let max_c = *town_counts.iter().max().unwrap();
        assert!(
            max_c - min_c > 500,
            "Expected settlements across seeds to vary markedly in block massing, got {town_counts:?}"
        );
    }

    #[test]
    fn phase10_all_44_new_structures_and_72_presets_build_cleanly() {
        use std::collections::HashMap;
        use crate::model::RoofStyle;
        use crate::parser::StructureTheme;
        use crate::server_builder::{CLI_PRESETS, find_cli_preset};

        assert_eq!(CLI_PRESETS.len(), 88);
        let mut seen_new_themes: HashMap<StructureTheme, &'static str> = HashMap::new();

        for (idx, &(slug, cat, style, w, d, h, desc)) in CLI_PRESETS.iter().enumerate() {
            assert_eq!(find_cli_preset(slug).map(|p| p.0), Some(slug), "Preset slug '{slug}' must resolve");

            let mut req = sample_request(desc, 9001 + idx as i64);
            req.category = cat;
            req.style = style;
            req.width = w;
            req.depth = d;
            req.height = h;
            req.palette = crate::parser::infer_palette_with_style_and_seed(style, desc, req.seed as u64);
            let bp = generate_blueprint(&req, None);

            assert!(
                bp.non_air_count() > 180,
                "Preset '{slug}' generated too few blocks ({})",
                bp.non_air_count()
            );

            for (pos, blk) in &bp.blocks {
                assert_ne!(
                    blk.block_id, "minecraft:chain",
                    "Preset '{slug}' used removed 26.3 block minecraft:chain at {pos:?}"
                );
                if blk.block_id.ends_with("_leaves") {
                    assert!(
                        blk.props.iter().any(|(k, v)| *k == "persistent" && *v == "true"),
                        "Preset '{slug}' emitted decaying leaves {} at {pos:?}",
                        blk.block_id
                    );
                }
                if blk.block_id.ends_with("_door") && !blk.block_id.ends_with("trapdoor") {
                    let has_half = blk.props.iter().any(|(k, v)| *k == "half" && *v == "lower");
                    if has_half {
                        let upper = bp.blocks.get(&[pos[0], pos[1] + 1, pos[2]]);
                        assert!(
                            upper.is_some_and(|u| u.block_id == blk.block_id
                                && u.props.iter().any(|(k, v)| *k == "half" && *v == "upper")),
                            "Preset '{slug}' has orphan lower door half at {pos:?}"
                        );
                    }
                }
            }

            if (28..72).contains(&idx) {
                assert_ne!(
                    bp.spec.theme,
                    StructureTheme::Standard,
                    "New preset '{slug}' must trigger a dedicated StructureTheme"
                );
                if let Some(prev) = seen_new_themes.insert(bp.spec.theme, slug) {
                    panic!("Theme collision: presets '{prev}' and '{slug}' both mapped to {:?}", bp.spec.theme);
                }
            }
        }

        assert_eq!(
            seen_new_themes.len(),
            44,
            "All 44 new StructureTheme variants must be exercised across new presets, got {}",
            seen_new_themes.len()
        );

        let check_has_id = |slug: &str, expected_id: &str| {
            let p = find_cli_preset(slug).unwrap();
            let mut req = sample_request(p.6, 4242);
            req.category = p.1;
            req.style = p.2;
            req.width = p.3;
            req.depth = p.4;
            req.height = p.5;
            req.palette = crate::parser::infer_palette_with_style_and_seed(p.2, p.6, 4242);
            let bp = generate_blueprint(&req, None);
            assert!(
                bp.blocks.values().any(|b| b.block_id == expected_id),
                "Preset '{slug}' should contain '{expected_id}'"
            );
            bp
        };

        let pantheon_bp = check_has_id("pantheon", "minecraft:quartz_pillar");
        assert_eq!(pantheon_bp.spec.roof_style, RoofStyle::Dome);

        let parthenon_bp = check_has_id("parthenon", "minecraft:quartz_pillar");
        assert!(parthenon_bp.spec.arcade_gallery);

        let _ = check_has_id("ishtar_gate", "minecraft:lapis_block");
        let _ = check_has_id("ishtar_gate", "minecraft:gold_block");
        let _ = check_has_id("ship", "minecraft:white_wool");
        let _ = check_has_id("station", "minecraft:rail");
        let _ = check_has_id("mushroom", "minecraft:red_mushroom_block");
        let _ = check_has_id("dragon", "minecraft:bone_block");

        let catacombs_bp = check_has_id("catacombs", "minecraft:soul_lantern");
        assert!(
            catacombs_bp.blocks.keys().any(|p| p[1] <= -4),
            "Mausoleum/catacombs must carve subterranean chamber at y <= -4"
        );

        let venice_bp = check_has_id("venice", "minecraft:water");
        assert!(
            venice_bp.non_air_count() > 1200,
            "Floating Venice canal town should have extensive stilt walkways and waterfront lots"
        );
    }

    #[test]
    fn bilingual_ui_commands_and_presets_work_in_en_and_ru() {
        use crate::model::{SetLangRequest, UiLang, translate_palette_name};
        use crate::persist::MatFilter;
        use crate::server_builder::{CLI_PRESET_DESCS_EN, cli_preset_desc, player_lang, set_player_lang};

        assert_eq!(UiLang::default(), UiLang::En);
        assert_eq!(UiLang::En.toggle(), UiLang::Ru);
        assert_eq!(UiLang::Ru.toggle(), UiLang::En);
        assert_eq!(UiLang::parse("en"), Some(UiLang::En));
        assert_eq!(UiLang::parse("русский"), Some(UiLang::Ru));

        let lang_req = SetLangRequest { lang: UiLang::Ru };
        let bytes = encode_payload(&lang_req, 1024).expect("encode SetLangRequest");
        let dec_lang: SetLangRequest = decode_payload(&bytes).expect("decode SetLangRequest");
        assert_eq!(dec_lang, lang_req);

        assert_ne!(MatFilter::All.label(UiLang::En), MatFilter::All.label(UiLang::Ru));
        for idx in 0..MaterialPalette::PRESET_COUNT {
            let p = MaterialPalette::preset(idx);
            let en_name = p.display_name(UiLang::En);
            let ru_name = p.display_name(UiLang::Ru);
            assert_ne!(en_name, ru_name, "Preset palette #{idx} must have distinct EN and RU names");
            assert_eq!(translate_palette_name(en_name, UiLang::Ru), ru_name);
            assert_eq!(translate_palette_name(ru_name, UiLang::En), en_name);
        }
        assert_eq!(translate_palette_name("Своя палитра", UiLang::En), "Custom Palette");
        assert_eq!(translate_palette_name("Custom Palette", UiLang::Ru), "Своя палитра");

        assert_eq!(CLI_PRESETS.len(), CLI_PRESET_DESCS_EN.len());
        for (idx, preset) in CLI_PRESETS.iter().enumerate() {
            let en = cli_preset_desc(idx, UiLang::En);
            let ru = cli_preset_desc(idx, UiLang::Ru);
            assert!(!en.is_empty() && !ru.is_empty());
            let spec_en = parse_description(preset.1, preset.2, en, preset.3, preset.4, preset.5, NoiseLevel::Balanced, 555);
            let spec_ru = parse_description(preset.1, preset.2, ru, preset.3, preset.4, preset.5, NoiseLevel::Balanced, 555);
            assert_eq!(
                spec_en.theme, spec_ru.theme,
                "EN/RU preset #{idx} ('{}') should resolve to same StructureTheme",
                preset.0
            );
        }

        assert_eq!(find_cli_preset("spaceport").map(|p| p.0), Some("rocket"));
        assert_eq!(find_cli_preset("pantheon").map(|p| p.0), Some("pantheon"));

        // Verify per-player language switching via `/buildgen lang ru|en` on dedicated test player 99.
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            host.fake.join(99, 100);
            let player = host.player(99).unwrap();
            let level_dim = host.level("minecraft:overworld").unwrap().dimension();

            set_player_lang(Some(player), UiLang::En);
            assert_eq!(player_lang(Some(player)), UiLang::En);

            let ru_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "lang".into()],
                vec![CommandArgument::new(
                    "lang".into(),
                    CommandArgumentValue::String("ru".into()),
                )],
                level_dim,
                [0.0, 64.0, 0.0],
            );
            let mut ru_ev = RunModCommandEvent {
                command: &ru_cmd,
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut ru_ev), Flow::Cancel);
            assert_eq!(player_lang(Some(player)), UiLang::Ru);
            if let Some((_, raw)) = host.sent_payloads.last()
                && let Ok(resp) = decode_payload::<crate::model::BuildResponse>(raw)
            {
                assert_eq!(resp.message, "__LANG__:ru");
            }
            let ru_build = execute_build(&mut host, Some(player), sample_request("cozy house", 12345));
            assert!(ru_build.ok);
            assert!(ru_build.message.contains("[отмен:"), "Expected RU feedback: {}", ru_build.message);

            let en_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "lang".into()],
                vec![CommandArgument::new(
                    "lang".into(),
                    CommandArgumentValue::String("en".into()),
                )],
                level_dim,
                [0.0, 64.0, 0.0],
            );
            let mut en_ev = RunModCommandEvent {
                command: &en_cmd,
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut en_ev), Flow::Cancel);
            assert_eq!(player_lang(Some(player)), UiLang::En);
            if let Some((_, raw)) = host.sent_payloads.last()
                && let Ok(resp) = decode_payload::<crate::model::BuildResponse>(raw)
            {
                assert_eq!(resp.message, "__LANG__:en");
            }
            let en_build = execute_build(&mut host, Some(player), sample_request("cozy house", 12346));
            assert!(en_build.ok);
            assert!(en_build.message.contains("[undo:"), "Expected EN feedback: {}", en_build.message);
        });
    }

    struct TestPayloadContext<'a> {
        host: &'a mut TestWorldHost,
        player: mod_api::PlayerRef,
        replies: Vec<(String, Vec<u8>)>,
    }

    impl mod_api::IPayloadContext for TestPayloadContext<'_> {
        fn player(&self) -> Result<mod_api::PlayerRef, mod_api::PayloadContextError> {
            Ok(self.player)
        }
        fn flow(&self) -> mod_api::PayloadDirection {
            mod_api::PayloadDirection::ToServer
        }
        fn protocol(&self) -> mod_api::PayloadProtocol {
            mod_api::PayloadProtocol::Play
        }
        fn server_host(&mut self) -> Result<&mut dyn ServerHost, mod_api::PayloadContextError> {
            Ok(self.host)
        }
        fn client_host(&mut self) -> Result<&mut dyn ClientHost, mod_api::PayloadContextError> {
            Err(mod_api::PayloadContextError::WrongSide)
        }
        fn reply(
            &mut self,
            id: &mod_api::PayloadId,
            bytes: mod_api::PayloadBytes,
        ) -> Result<mod_api::PayloadAccepted, mod_api::PayloadSendError> {
            self.replies.push((id.0.clone(), bytes.0));
            Ok(mod_api::PayloadAccepted::Queued)
        }
        fn check_payload_token(
            &self,
            _: &mod_api::PayloadToken,
        ) -> Result<(), mod_api::PayloadSendError> {
            Ok(())
        }
        fn disconnect(&mut self, _: mod_api::Text) {}
        fn enqueue_work(
            &mut self,
            _: std::sync::Arc<dyn mod_api::PayloadWork>,
        ) -> Result<mod_api::PayloadWorkStatus, mod_api::PayloadContextError> {
            Ok(mod_api::PayloadWorkStatus::Complete)
        }
    }

    #[test]
    fn v030_redo_air_only_bom_and_server_security_work() {
        use crate::model::{RedoRequest, SetLangRequest, UiLang, UndoRequest};
        use crate::server_builder::{
            clear_rate_limit_for_test, handle_build_payload, handle_redo_payload,
            handle_reroll_payload, handle_undo_payload, player_lang, set_player_lang,
        };

        // 1. RedoRequest + BuildRequest(air_only=true) codec roundtrip + BOM check.
        let redo_req = RedoRequest {
            dimension: "minecraft:overworld".into(),
        };
        let rbytes = encode_payload(&redo_req, 1024).expect("encode RedoRequest");
        assert_eq!(decode_payload::<RedoRequest>(&rbytes).unwrap(), redo_req);

        let mut air_req = sample_request("cozy oak house with porch", 31337);
        air_req.air_only = true;
        let bbytes = encode_payload(&air_req, 65_536).expect("encode BuildRequest");
        assert_eq!(decode_payload::<BuildRequest>(&bbytes).unwrap(), air_req);

        let bom_bp = generate_blueprint(&air_req, None);
        let (total_non_air, bom) = bom_bp.material_breakdown();
        assert_eq!(total_non_air, bom_bp.non_air_count());
        assert!(!bom.is_empty());
        assert!(bom.windows(2).all(|w| w[0].1 >= w[1].1), "BOM must sort descending");

        // 2. End-to-end Undo -> Redo, Air Only preservation, OP security, sanitization, rate limit.
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            host.fake.join(201, 100);
            host.fake.join(202, 100);
            let non_op = host.player(201).unwrap();
            let op_player = host.player(202).unwrap();
            host.set_player_perm(201, 0);
            host.set_player_perm(202, 2);
            set_player_lang(Some(non_op), UiLang::En);
            set_player_lang(Some(op_player), UiLang::En);
            clear_rate_limit_for_test();

            // Place protective stone pillar inside build box [10..23, 64..76, 20..31].
            let stone_key = host.id_to_key["minecraft:cobblestone"];
            let stone_state: BlockState = BlockStateRecord::new(stone_key, 0).into();
            let mut protected = Vec::new();
            for y in 64..=69 {
                for x in 15..=17 {
                    for z in 24..=26 {
                        host.world_blocks.insert([x, y, z], stone_state);
                        protected.push([x, y, z]);
                    }
                }
            }
            let before_air = host.world_blocks.clone();

            // Non-OP player cannot build/undo/redo via command or payload, though can switch lang.
            let level_dim = host.level("minecraft:overworld").unwrap().dimension();
            let gen_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "generate".into()],
                vec![CommandArgument::new(
                    "description".into(),
                    CommandArgumentValue::String("cozy house".into()),
                )],
                level_dim,
                [15.0, 64.0, 15.0],
            );
            let mut deny_ev = RunModCommandEvent {
                command: &gen_cmd,
                source: &CommandSource::Player {
                    player: non_op,
                    permission: PermissionLevel::All,
                },
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut deny_ev), Flow::Cancel);
            assert_eq!(host.world_blocks, before_air, "Non-OP command must not modify world");

            let lang_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "lang".into()],
                vec![CommandArgument::new(
                    "lang".into(),
                    CommandArgumentValue::String("ru".into()),
                )],
                level_dim,
                [15.0, 64.0, 15.0],
            );
            let mut lang_ev = RunModCommandEvent {
                command: &lang_cmd,
                source: &CommandSource::Player {
                    player: non_op,
                    permission: PermissionLevel::All,
                },
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut lang_ev), Flow::Cancel);
            assert_eq!(player_lang(Some(non_op)), UiLang::Ru);

            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player: non_op,
                    replies: Vec::new(),
                };
                handle_build_payload(sample_request("cozy house", 111), &mut cx).unwrap();
                handle_undo_payload(UndoRequest { dimension: "minecraft:overworld".into() }, &mut cx).unwrap();
                handle_redo_payload(RedoRequest { dimension: "minecraft:overworld".into() }, &mut cx).unwrap();
                handle_reroll_payload(crate::model::RerollRequest { dimension: "minecraft:overworld".into() }, &mut cx).unwrap();
            }
            assert_eq!(host.world_blocks, before_air, "Non-OP payloads must never modify world");

            // Air-only build by OP player preserves all pre-existing cobblestone blocks!
            let b_resp = execute_build(&mut host, Some(op_player), air_req);
            assert!(b_resp.ok, "air-only build failed: {}", b_resp.message);
            assert!(b_resp.message.contains("redo: 0/5"), "expected redo counter: {}", b_resp.message);
            for pos in &protected {
                assert_eq!(
                    host.world_blocks.get(pos).copied(),
                    Some(stone_state),
                    "air_only mode overwrote existing block at {:?}",
                    pos
                );
            }
            let built_snapshot = host.world_blocks.clone();
            assert_ne!(built_snapshot, before_air);

            // Undo restores exact pre-build state, then Redo restores built snapshot!
            let u_resp = execute_undo(&mut host, Some(op_player), "minecraft:overworld");
            assert!(u_resp.ok && u_resp.message.contains("redo: 1/5"), "unexpected undo msg: {}", u_resp.message);
            assert_eq!(host.world_blocks, before_air);

            let r_resp = execute_redo(&mut host, Some(op_player), "minecraft:overworld");
            assert!(r_resp.ok && r_resp.message.contains("undo: 1/5 | redo: 0/5"), "unexpected redo msg: {}", r_resp.message);
            assert_eq!(host.world_blocks, built_snapshot);

            // Undo again, then a fresh build clears Redo stack.
            assert!(execute_undo(&mut host, Some(op_player), "minecraft:overworld").ok);
            let mut normal_req = sample_request("cozy oak house", 4242);
            normal_req.width = 11;
            normal_req.depth = 9;
            assert!(execute_build(&mut host, Some(op_player), normal_req).ok);
            assert!(!execute_redo(&mut host, Some(op_player), "minecraft:overworld").ok);

            // Request bounds sanitization + network rate limiter + Undo/Redo payloads.
            clear_rate_limit_for_test();
            let mut bloated = sample_request(&"a".repeat(900), 9090);
            bloated.width = 500;
            bloated.depth = 500;
            bloated.height = 500;
            bloated.origin_x = 99_999_999;
            bloated.origin_y = -9999;
            bloated.dimension = "".into();
            let sanitized = crate::server_builder::sanitize_build_request(bloated);
            assert_eq!(sanitized.width, 96);
            assert_eq!(sanitized.depth, 96);
            assert_eq!(sanitized.height, 48);
            assert_eq!(sanitized.origin_x, 29_999_900);
            assert_eq!(sanitized.origin_y, -2048);
            assert_eq!(sanitized.dimension, "minecraft:overworld");
            assert_eq!(sanitized.description.chars().count(), 512);

            let mut net_req = sample_request(&"b".repeat(800), 9090);
            net_req.width = 13;
            net_req.depth = 11;
            net_req.height = 10;
            net_req.dimension = "   ".into();
            let before_net = host.world_blocks.clone();
            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player: op_player,
                    replies: Vec::new(),
                };
                handle_build_payload(net_req.clone(), &mut cx).unwrap();
            }
            let after_net = host.world_blocks.clone();
            assert_ne!(after_net, before_net);
            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player: op_player,
                    replies: Vec::new(),
                };
                // Immediate second build triggers rate limiter without modifying world again.
                handle_build_payload(net_req, &mut cx).unwrap();
            }
            assert_eq!(host.world_blocks, after_net, "rate-limited build must not mutate world");
            clear_rate_limit_for_test();
            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player: op_player,
                    replies: Vec::new(),
                };
                handle_undo_payload(UndoRequest { dimension: "minecraft:overworld".into() }, &mut cx).unwrap();
            }
            assert_eq!(host.world_blocks, before_net, "Undo payload must restore world");
            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player: op_player,
                    replies: Vec::new(),
                };
                handle_redo_payload(RedoRequest { dimension: "minecraft:overworld".into() }, &mut cx).unwrap();
            }
            assert_eq!(host.world_blocks, after_net, "Redo payload must re-apply build");
            let _ = SetLangRequest { lang: UiLang::En };
        });
    }

    #[test]
    fn v040_settlement2_biome_autostyle_and_weathering_work() {
        use crate::model::{BiomeQueryRequest, BiomeQueryResponse, UiLang};
        use crate::parser::{
            biome_short_label, infer_palette_with_biome, infer_style_with_biome,
        };
        use crate::persist::{
            SavedPreset, decode_share_code, encode_share_code, parse_favorites,
            serialize_favorites,
        };
        use crate::server_builder::handle_biome_payload;

        // 1. BiomeQuery codec roundtrip & Biome Auto-Style / Palette inference.
        let bq = BiomeQueryRequest {
            dimension: "minecraft:overworld".into(),
            x: 120,
            y: 64,
            z: -80,
        };
        let bq_bytes = encode_payload(&bq, 1024).unwrap();
        assert_eq!(decode_payload::<BiomeQueryRequest>(&bq_bytes).unwrap(), bq);
        let br = BiomeQueryResponse {
            biome: "minecraft:cherry_grove".into(),
        };
        let br_bytes = encode_payload(&br, 1024).unwrap();
        assert_eq!(decode_payload::<BiomeQueryResponse>(&br_bytes).unwrap(), br);

        assert_eq!(
            biome_short_label("minecraft:cherry_grove", UiLang::En),
            "Cherry"
        );
        assert_eq!(
            biome_short_label("minecraft:cherry_grove", UiLang::Ru),
            "Сакура"
        );
        assert_eq!(
            infer_style_with_biome(BuildStyle::Auto, "cozy house", 42, "minecraft:cherry_grove"),
            BuildStyle::Japanese
        );
        assert_eq!(
            infer_style_with_biome(BuildStyle::Auto, "cozy house", 42, "minecraft:desert"),
            BuildStyle::Desert
        );
        assert_eq!(
            infer_style_with_biome(BuildStyle::Auto, "cozy house", 42, "minecraft:snowy_taiga"),
            BuildStyle::Nordic
        );
        assert_eq!(
            infer_style_with_biome(BuildStyle::Auto, "cozy house", 42, "minecraft:pale_garden"),
            BuildStyle::Gothic
        );
        // Explicit prompt style overrides world biome!
        assert_eq!(
            infer_style_with_biome(
                BuildStyle::Auto,
                "готический замок",
                42,
                "minecraft:cherry_grove"
            ),
            BuildStyle::Gothic
        );
        let cherry_pal =
            infer_palette_with_biome(BuildStyle::Auto, "cozy house", 42, "minecraft:cherry_grove");
        assert!(
            cherry_pal.wall.contains("cherry") || cherry_pal.leaves.contains("cherry"),
            "cherry_grove biome should drive cherry palette: {:?}",
            cherry_pal.wall
        );

        // 2. BG2 v4 Share-Code preserves wear_pct & overgrowth_pct cleanly.
        let preset_v4 = SavedPreset {
            name: "Ruined Keep".into(),
            width: 21,
            depth: 19,
            height: 18,
            category: BuildCategory::Castle,
            style: BuildStyle::Gothic,
            roof_raw: 2,
            toggles: [1, 1, 1, 0, 1],
            noise_level: NoiseLevel::Balanced,
            adapt_terrain: true,
            air_only: true,
            wear_pct: 65,
            overgrowth_pct: 45,
            description: "gothic castle keep".into(),
            seed: 123_456,
            palette_manual: false,
            palette: infer_palette_with_biome(BuildStyle::Gothic, "gothic castle keep", 123_456, ""),
            living_decor: true,
            spawn_npcs: false,
            blend_radius: 4,
        };
        let v4_code = encode_share_code(&preset_v4);
        assert_eq!(
            decode_share_code(&v4_code, Some("Ruined Keep")).unwrap(),
            preset_v4
        );
        assert_eq!(parse_favorites(&serialize_favorites(&[preset_v4.clone()])), vec![preset_v4]);

        // 3. Weathering (Wear + Overgrowth) & Copper Oxidation + Biome Flora + Server Biome Lookup.
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            host.fake.join(301, 100);
            let player = host.player(301).unwrap();
            host.set_biome(Some("minecraft:cherry_grove"));

            {
                let mut cx = TestPayloadContext {
                    host: &mut host,
                    player,
                    replies: Vec::new(),
                };
                handle_biome_payload(bq, &mut cx).unwrap();
            }

            // Server execute_build with Auto style samples cherry_grove from TestWorldHost!
            let mut cherry_build = sample_request("cozy house", 8888);
            cherry_build.origin_x = 12;
            cherry_build.origin_z = 12;
            let resp = execute_build(&mut host, Some(player), cherry_build);
            assert!(resp.ok, "cherry biome auto-build failed: {}", resp.message);
            let cherry_planks_key = host.id_to_key["minecraft:cherry_planks"];
            assert!(
                host.world_blocks
                    .values()
                    .any(|st| st.block() == cherry_planks_key),
                "Auto biome build in cherry_grove should place cherry_planks"
            );

            // Universal Wear + Overgrowth on stone + copper castle.
            let mut intact = sample_request("steampunk castle with copper roof", 54321);
            intact.width = 21;
            intact.depth = 19;
            intact.height = 18;
            intact.category = BuildCategory::Castle;
            intact.style = BuildStyle::Steampunk;
            intact.palette = MaterialPalette::preset(7);
            let mut aged = intact.clone();
            aged.wear_pct = 75;
            aged.overgrowth_pct = 80;
            aged.biome_hint = "minecraft:pale_garden".into();
            let intact_bp = generate_blueprint(&intact, None);
            let aged_bp = generate_blueprint(&aged, None);
            assert_ne!(intact_bp.non_air_count(), aged_bp.non_air_count());
            assert!(
                aged_bp.blocks.values().any(|b| b.block_id.contains("exposed_cut_copper")
                    || b.block_id.contains("weathered_cut_copper")
                    || b.block_id.contains("oxidized_cut_copper")
                    || b.block_id == "minecraft:cracked_stone_bricks"),
                "75% wear must oxidize copper and crack stone bricks"
            );
            assert!(
                aged_bp.blocks.values().any(|b| b.block_id == "minecraft:vine"),
                "80% overgrowth must drape directional vines"
            );
            assert!(
                aged_bp.blocks.values().any(|b| b.block_id == "minecraft:pale_moss_carpet"
                    || b.block_id == "minecraft:pale_oak_leaves"),
                "Pale garden overgrowth must use pale flora"
            );
            assert!(
                blueprint_to_phased_runs(&host, &aged_bp).is_ok(),
                "All weathered/overgrown blocks must resolve in world registry"
            );

            // 4. Settlement 2.0: 72x72 fortified town with natural river crossing bridge + multi-district entrances.
            let mut river_wd = vec![0i32; 72 * 72];
            for lz in 18..=22 {
                for lx in 0..72 {
                    river_wd[lz * 72 + lx] = 3;
                }
            }
            let mut town_req = sample_request("большой средневековый город с площадью", 77777);
            town_req.width = 72;
            town_req.depth = 72;
            town_req.height = 24;
            town_req.category = BuildCategory::Settlement;
            town_req.style = BuildStyle::Medieval;
            town_req.palette = MaterialPalette::preset(1);
            let town_bp =
                crate::generator::generate_blueprint_with_water(&town_req, None, Some(&river_wd));
            assert!(
                blueprint_to_phased_runs(&host, &town_bp).is_ok(),
                "Settlement 2.0 must emit only valid block IDs"
            );
            let door_cols: std::collections::HashSet<[i32; 2]> = town_bp
                .blocks
                .iter()
                .filter(|(_, b)| b.block_id.ends_with("_door"))
                .map(|(&[x, _, z], _)| [x, z])
                .collect();
            assert!(
                door_cols.len() >= 4,
                "Settlement 2.0 72x72 town should build at least 4 distinct building entrances, got {}",
                door_cols.len()
            );
            assert!(
                town_bp.blocks.iter().any(|(&[x, y, z], b)| {
                    (33..=39).contains(&x) && (18..=22).contains(&z) && y >= 1 && b.block_id != "minecraft:air" && b.block_id != "minecraft:water"
                }),
                "Settlement 2.0 should span a bridge across natural river water at z=18..22"
            );
        });
    }

    #[test]
    fn v041_bridge_walkability_and_headroom_work() {
        let cases: &[(&str, BuildStyle, u16, u16, u16, i32)] = &[
            ("крепостной разводной мост со сторожевыми башнями", BuildStyle::Desert, 13, 27, 10, -1),
            ("крепостной разводной мост со сторожевыми башнями", BuildStyle::Medieval, 15, 36, 12, 0),
            ("деревянный крытый мост", BuildStyle::Cottage, 11, 25, 10, -1),
            ("деревянный крытый мост", BuildStyle::Japanese, 9, 19, 12, 0),
            ("японский горбатый лунный мост тайко с воротами тории", BuildStyle::Japanese, 13, 29, 12, -1),
            ("римский двухъярусный акведук мост с каналом воды", BuildStyle::Classical, 9, 25, 11, -1),
            ("римский двухъярусный акведук мост с каналом воды", BuildStyle::Classical, 15, 35, 16, 0),
            ("вантовый подвесной мост с пилонами и канатами", BuildStyle::Modern, 15, 35, 14, -1),
            ("деревянный понтонный пирс пристань мост", BuildStyle::Nordic, 11, 25, 10, -1),
            ("арочный каменный мост", BuildStyle::Gothic, 13, 29, 11, -1),
        ];

        for &(desc, style, w, d, h, bank_terr) in cases {
            for &use_terrain in &[true, false] {
                let offs = use_terrain.then(|| vec![bank_terr; usize::from(w) * usize::from(d)]);
                for &seed in &[101_i64, 4242, 99_999] {
                    let mut req = sample_request(desc, seed);
                    req.category = BuildCategory::Bridge;
                    req.style = style;
                    req.width = w;
                    req.depth = d;
                    req.height = h;
                    req.adapt_terrain = use_terrain;
                    req.biome_hint = if style == BuildStyle::Desert {
                        "minecraft:desert".into()
                    } else {
                        String::new()
                    };
                    req.palette = MaterialPalette::preset_for_style(style);

                    let bp = generate_blueprint(&req, offs.as_deref());
                    let cx = i32::from(w) / 2;

                    let walkway_candidates = |lx: i32, lz: i32| -> Vec<f64> {
                        let mut cands = Vec::new();
                        for y in -6..i32::from(h) {
                            let Some(blk) = bp.blocks.get(&[lx, y, lz]) else {
                                continue;
                            };
                            let id = blk.block_id.as_str();
                            if id == "minecraft:air" || id == "minecraft:water" {
                                continue;
                            }
                            let clear_1 = bp.blocks.get(&[lx, y + 1, lz]).is_none_or(|b| b.block_id == "minecraft:air");
                            let clear_2 = bp.blocks.get(&[lx, y + 2, lz]).is_none_or(|b| b.block_id == "minecraft:air");
                            if clear_1 && clear_2 {
                                let top = if id.ends_with("_slab")
                                    && blk.props.iter().any(|&(k, v)| k == "type" && v == "bottom")
                                {
                                    f64::from(y) + 0.5
                                } else {
                                    f64::from(y) + 1.0
                                };
                                cands.push(top);
                            }
                        }
                        cands
                    };

                    // Walk continuously along centerline Z from bank 0 to bank d-1 without hitting roofs/lintels.
                    let ext_ground_top = if use_terrain { f64::from(bank_terr) + 1.0 } else { 1.0 };
                    let mut prev_top: Option<f64> = None;
                    for lz in 0..i32::from(d) {
                        let target_ref = prev_top.unwrap_or(ext_ground_top);
                        let center_cands = walkway_candidates(cx, lz);
                        let center_top = center_cands
                            .iter()
                            .copied()
                            .min_by(|a, b| {
                                (a - target_ref)
                                    .abs()
                                    .partial_cmp(&(b - target_ref).abs())
                                    .unwrap()
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "Bridge '{desc}' seed={seed} h={h} terrain={use_terrain}: no walkable deck at center z={lz}"
                                )
                            });

                        if lz == 0 || lz == i32::from(d) - 1 {
                            let max_entry = if use_terrain { 0.55 } else { 1.01 };
                            assert!(
                                (center_top - ext_ground_top).abs() <= max_entry,
                                "Bridge '{desc}' seed={seed} h={h} terrain={use_terrain}: bank entrance z={lz} center_top={center_top} vs ext_ground={ext_ground_top}"
                            );
                        }
                        if let Some(pt) = prev_top {
                            assert!(
                                (center_top - pt).abs() <= 1.01,
                                "Bridge '{desc}' seed={seed} h={h}: steep jump {pt} -> {center_top} at z={lz}"
                            );
                        }

                        // All 3 central lanes (cx-1..=cx+1) must be unobstructed with >= 3.0 headroom at deck level!
                        for lx in (cx - 1)..=(cx + 1) {
                            let cands = walkway_candidates(lx, lz);
                            let lane_top = cands
                                .iter()
                                .copied()
                                .min_by(|a, b| {
                                    (a - center_top)
                                        .abs()
                                        .partial_cmp(&(b - center_top).abs())
                                        .unwrap()
                                })
                                .unwrap_or_else(|| {
                                    panic!(
                                        "Bridge '{desc}' seed={seed} h={h} terrain={use_terrain}: blocked walkway lane x={lx} z={lz} near center_top={center_top}"
                                    )
                                });
                            assert!(
                                (lane_top - center_top).abs() <= 0.55,
                                "Bridge '{desc}' seed={seed} h={h}: lane x={lx} z={lz} misaligned lane_top={lane_top} vs center_top={center_top}"
                            );
                            let base_block_y = (lane_top - 0.01).floor() as i32;
                            for cy in (base_block_y + 1)..=(base_block_y + 3).min(i32::from(h) - 1) {
                                let blk = bp.blocks.get(&[lx, cy, lz]);
                                assert!(
                                    blk.is_none_or(|b| b.block_id == "minecraft:air"),
                                    "Bridge '{desc}' seed={seed} h={h} terrain={use_terrain}: head obstacle {:?} at [{lx},{cy},{lz}] above tread top={lane_top}",
                                    blk.map(|b| &b.block_id)
                                );
                            }
                        }

                        prev_top = Some(center_top);
                    }
                }
            }
        }

        // Asymmetric river bank heights (-2 on north, +2 on south) stay flush at both ends!
        let w = 13u16;
        let d = 25u16;
        let mut asym = vec![0i32; usize::from(w) * usize::from(d)];
        for lz in 0..i32::from(d) {
            let y = if lz < 5 {
                -2
            } else if lz > 19 {
                2
            } else {
                -4
            };
            for lx in 0..i32::from(w) {
                asym[(lz * i32::from(w) + lx) as usize] = y;
            }
        }
        let mut req = sample_request("крепостной разводной мост со сторожевыми башнями", 31415);
        req.category = BuildCategory::Bridge;
        req.width = w;
        req.depth = d;
        req.height = 14;
        req.adapt_terrain = true;
        let bp = generate_blueprint(&req, Some(&asym));
        let cx = i32::from(w) / 2;
        assert!(
            bp.blocks.get(&[cx, -2, 0]).is_some_and(|b| b.block_id != "minecraft:air"),
            "North bank z=0 must anchor at terr=-2"
        );
        assert!(
            bp.blocks.get(&[cx, 2, i32::from(d) - 1]).is_some_and(|b| b.block_id != "minecraft:air"),
            "South bank z=end must anchor at terr=+2"
        );
    }

    #[test]
    fn live_3d_hologram_preview_renders_translucent_bluish_shapes_and_creases() {
        let mut req = sample_request("cozy oak house with porch, windows, basement, and gable roof", 7777);
        req.width = 15;
        req.depth = 13;
        req.height = 12;
        let bp = generate_blueprint(&req, None);
        let mesh = crate::gen_preview::build_hologram_mesh(&bp, None);

        assert!(mesh.quads.len() >= 25, "expected rich 3D hologram faces, got {}", mesh.quads.len());
        assert!(mesh.lines.len() >= 15, "expected architectural crease edges, got {}", mesh.lines.len());

        let has_negative_y_cell = bp.blocks.keys().any(|&[_, y, _]| y < 0);
        let mut saw_negative_y_quad = false;
        for q in &mesh.quads {
            let a = (q.color >> 24) & 0xFF;
            let r = (q.color >> 16) & 0xFF;
            let g = (q.color >> 8) & 0xFF;
            let b = q.color & 0xFF;
            assert!(
                (0x20..=0x90).contains(&a),
                "hologram alpha 0x{a:02X} must be semi-transparent"
            );
            assert!(
                b >= g && b >= r,
                "hologram color 0x{:08X} should preserve bluish tint",
                q.color
            );
            for c in q.corners {
                assert!(c[0].is_finite() && c[1].is_finite() && c[2].is_finite());
                if c[1] < -0.05 {
                    saw_negative_y_quad = true;
                }
            }
        }
        if has_negative_y_cell {
            assert!(saw_negative_y_quad, "basement/foundation geometry below Y=0 must appear in 3D preview");
        }
        let (fitted_q, fitted_l) =
            mesh.fit_budget(crate::gen_preview::TARGET_MAX_QUADS, crate::gen_preview::TARGET_MAX_LINES);
        assert!(!fitted_q.is_empty() && fitted_q.len() <= crate::gen_preview::TARGET_MAX_QUADS);
        assert!(fitted_l.len() <= crate::gen_preview::TARGET_MAX_LINES);
    }

    #[test]
    fn live_3d_hologram_slice_and_large_structures_stay_within_view_capacity() {
        let mut req = sample_request("cozy oak house with windows and gable roof", 4242);
        req.width = 13;
        req.depth = 11;
        req.height = 12;
        let bp = generate_blueprint(&req, None);
        let sliced = crate::gen_preview::build_hologram_mesh(&bp, Some(4));
        assert!(!sliced.quads.is_empty());
        assert!(
            sliced
                .quads
                .iter()
                .all(|q| q.corners.iter().all(|c| c[1] <= 5.05)),
            "sliced preview must clip geometry above Y=4"
        );
        assert!(
            sliced.quads.iter().any(|q| q.color == crate::gen_preview::SLICE_CAP_ARGB),
            "sliced preview should highlight the horizontal Y-cut plane"
        );

        let mut bridge_req = sample_request("каменный арочный мост с фонарями и башнями", 9001);
        bridge_req.category = BuildCategory::Bridge;
        bridge_req.width = 13;
        bridge_req.depth = 37;
        bridge_req.height = 14;
        let bridge_bp = generate_blueprint(&bridge_req, None);
        let bridge_mesh = crate::gen_preview::build_hologram_mesh(&bridge_bp, None);
        let (bq, bl) = bridge_mesh.fit_budget(
            crate::gen_preview::TARGET_MAX_QUADS,
            crate::gen_preview::TARGET_MAX_LINES,
        );
        assert!(!bq.is_empty() && bq.len() <= crate::gen_preview::TARGET_MAX_QUADS);
        assert!(bl.len() <= crate::gen_preview::TARGET_MAX_LINES);

        let mut city_req = sample_request("средневековый каменный город с рынком, фонтаном и стенами", 5555);
        city_req.category = BuildCategory::Settlement;
        city_req.width = 72;
        city_req.depth = 72;
        city_req.height = 30;
        let city_bp = generate_blueprint(&city_req, None);
        let city_mesh = crate::gen_preview::build_hologram_mesh(&city_bp, None);
        let (cq, cl) = city_mesh.fit_budget(
            crate::gen_preview::TARGET_MAX_QUADS,
            crate::gen_preview::TARGET_MAX_LINES,
        );
        assert!(
            !cq.is_empty() && cq.len() <= crate::gen_preview::TARGET_MAX_QUADS,
            "large settlement preview exceeded quad budget: {}",
            cq.len()
        );
        assert!(
            cl.len() <= crate::gen_preview::TARGET_MAX_LINES,
            "large settlement preview exceeded line budget: {}",
            cl.len()
        );
    }

    fn is_horizon_allowed_hmod_path(rel: &str) -> bool {
        if rel.is_empty() || rel.contains('\\') || rel.starts_with('/') {
            return false;
        }
        if matches!(rel, "horizon-mod.toml" | "layouts.toml" | "uses.json" | "icon.png") {
            return true;
        }
        if rel.starts_with("assets/") || rel.starts_with("data/") {
            return true;
        }
        if matches!(rel, "Cargo.toml" | "Cargo.lock" | "build.rs") {
            return true;
        }
        if let Some(sub) = rel.strip_prefix("src/") {
            return sub.ends_with(".rs") || sub.ends_with(".toml");
        }
        !rel.contains('/')
            && ["README", "LICENSE", "LICENCE", "COPYING", "NOTICE"]
                .iter()
                .any(|pfx| rel.starts_with(pfx))
    }

    #[test]
    fn hmod_package_path_allowlist_rejects_gitignore_and_unknown_files() {
        assert!(!is_horizon_allowed_hmod_path(".gitignore"));
        assert!(!is_horizon_allowed_hmod_path(".git/HEAD"));
        assert!(!is_horizon_allowed_hmod_path("target/release/mod.dll"));
        assert!(!is_horizon_allowed_hmod_path("HOW_TO_INSTALL.txt"));
        assert!(!is_horizon_allowed_hmod_path("building_generator-0.4.1.hmod"));

        for rel in [
            "horizon-mod.toml",
            "uses.json",
            "Cargo.toml",
            "icon.png",
            "LICENSE",
            "README.md",
            "src/lib.rs",
            "src/gen_preview.rs",
        ] {
            assert!(is_horizon_allowed_hmod_path(rel), "expected valid path {rel}");
        }
    }

    #[test]
    fn v043_bridges_settlements_and_presets_never_panic_at_tiny_or_narrow_sizes() {
        // Specifically stress narrow Aqueduct & Pontoon bridges (7..15 along_len) that panicked at max_spans=1.
        for along in [7u16, 8, 9, 10, 11, 13, 15, 25] {
            for h in [5u16, 7, 10, 14] {
                for prompt in ["roman aqueduct bridge", "pontoon dock bridge", "covered wooden bridge"] {
                    let mut req = sample_request(prompt, 777 + i64::from(along));
                    req.category = BuildCategory::Bridge;
                    req.width = 7;
                    req.depth = along;
                    req.height = h;
                    let bp = generate_blueprint(&req, None);
                    assert!(bp.non_air_count() > 0);
                }
            }
        }

        // Reproduces exact Settlement -> Aqueduct narrow bridge panic plus stresses every category/preset.
        for &(w, d, h) in &[(7u16, 7u16, 5u16), (9, 13, 8), (19, 9, 8), (11, 25, 10), (36, 36, 12)] {
            for &cat in &BuildCategory::ALL {
                for &facing in &[
                    CardinalFacing::North,
                    CardinalFacing::South,
                    CardinalFacing::East,
                    CardinalFacing::West,
                ] {
                    let mut req = sample_request("aqueduct bridge town", 4242);
                    req.category = cat;
                    req.facing = facing;
                    req.width = w;
                    req.depth = d;
                    req.height = h;
                    let bp = generate_blueprint(&req, None);
                    assert!(bp.non_air_count() > 0);
                }
            }
        }

        // Settlement 2.0 across many seeds with river terrain offsets (triggers all 9 BridgeVariants at narrow spans).
        let mut terrain = vec![0i32; 40 * 40];
        for z in 0..40 {
            for x in 0..40 {
                terrain[z * 40 + x] = if (16..=24).contains(&x) { -3 } else { 1 };
            }
        }
        let water: Vec<i32> = terrain.iter().map(|&t| if t < 0 { 3 } else { 0 }).collect();
        for s in 0..24i64 {
            let mut req = sample_request("medieval fortified walled city with bridge", 9000 + s * 131);
            req.category = BuildCategory::Settlement;
            req.width = 40;
            req.depth = 40;
            req.height = 18;
            req.adapt_terrain = true;
            let bp = crate::generator::generate_blueprint_with_water(&req, Some(&terrain), Some(&water));
            assert!(bp.non_air_count() > 100);
        }

        // Sweep all 76 CLI presets at minimum slider dimensions (7x7x5) and small studio sizes (11x11x7).
        for &(w, d, h) in &[(7u16, 7u16, 5u16), (11, 11, 7)] {
            for (idx, &(slug, cat, style, _pw, _pd, _ph, prompt)) in CLI_PRESETS.iter().enumerate() {
                let mut req = sample_request(prompt, 50_000 + idx as i64);
                req.category = cat;
                req.style = style;
                req.width = w;
                req.depth = d;
                req.height = h;
                let bp = generate_blueprint(&req, None);
                assert!(
                    bp.non_air_count() > 0,
                    "preset {slug} failed at {w}x{d}x{h}",
                );
            }
        }
    }

    #[test]
    fn v050_hologram_world_collision_and_air_only_filtering() {
        let mut req = sample_request("cozy stone cottage", 501);
        req.width = 11;
        req.depth = 11;
        req.height = 9;
        let bp = generate_blueprint(&req, None);
        let mesh_plain = crate::gen_preview::build_hologram_mesh(&bp, None);
        assert!(!mesh_plain.quads.is_empty());

        // Mark a non-air world obstacle intersecting the building's solid blocks.
        let mut mask = crate::gen_preview::PreviewWorldMask::new(
            i32::from(bp.width),
            0,
            i32::from(bp.height),
            i32::from(bp.depth),
        );
        let mut marked = 0usize;
        for (&[x, y, z], blk) in &bp.blocks {
            if y >= 1 && blk.block_id != "minecraft:air" && x <= 3 {
                mask.set_blocked(x, y, z, true);
                marked += 1;
            }
        }
        assert!(marked > 0);

        let mesh_col = crate::gen_preview::build_hologram_mesh_with_world(
            &bp,
            None,
            Some(&mask),
            false,
            true,
        );
        let has_coral_collision = mesh_col.quads.iter().any(|q| {
            let r = (q.color >> 16) & 0xFF;
            let b = q.color & 0xFF;
            r > b && r >= 96
        });
        assert!(has_coral_collision, "expected warm coral collision quads");
        assert!(mesh_col.lines.len() <= crate::gen_preview::TARGET_MAX_LINES || !mesh_col.macro_lines.is_empty());

        let mesh_air = crate::gen_preview::build_hologram_mesh_with_world(
            &bp,
            None,
            Some(&mask),
            true,
            true,
        );
        let (fitted_q, fitted_l) = mesh_air.fit_budget(
            crate::gen_preview::TARGET_MAX_QUADS,
            crate::gen_preview::TARGET_MAX_LINES,
        );
        assert!(fitted_q.len() <= crate::gen_preview::TARGET_MAX_QUADS);
        assert!(fitted_l.len() <= crate::gen_preview::TARGET_MAX_LINES);
        // No coral collision quads should be emitted when blocked voxels are filtered out by Air Only.
        assert!(
            !mesh_air.quads.iter().any(|q| ((q.color >> 24) & 0xFF) == 0x70),
            "air-only mode should omit blocked voxels rather than highlighting them"
        );
    }

    #[test]
    fn v050_populated_containers_curate_themed_loot_and_names() {
        for (cat, prompt, expected_en_sub) in [
            (BuildCategory::Forge, "blacksmith forge", "Smith"),
            (BuildCategory::House, "wizard mage tower with alchemy", "Arcane"),
            (BuildCategory::House, "grand library archive", "Scholar"),
            (BuildCategory::House, "warm timber tavern inn", "Tavern"),
            (BuildCategory::Castle, "medieval fortress", "Treasury"),
        ] {
            let mut req = sample_request(prompt, 7707);
            req.category = cat;
            req.width = 13;
            req.depth = 13;
            req.height = 14;
            let bp = generate_blueprint(&req, None);
            let en_containers = crate::server_builder::collect_post_build_containers(&bp, 7707, crate::model::UiLang::En);
            let ru_containers = crate::server_builder::collect_post_build_containers(&bp, 7707, crate::model::UiLang::Ru);
            assert!(
                !en_containers.is_empty(),
                "expected at least one container for {prompt}"
            );
            assert_eq!(en_containers.len(), ru_containers.len());
            assert!(
                en_containers.iter().all(|c| !c.items.is_empty() && !c.custom_name.is_empty()),
                "every curated container for {prompt} should have items & title"
            );
            let has_expected_theme = en_containers
                .iter()
                .any(|c| c.custom_name.contains(expected_en_sub) || c.custom_name.contains("Alchemist") || c.custom_name.contains("Keg"));
            assert!(
                has_expected_theme,
                "expected thematic container name containing '{expected_en_sub}' in {:?}",
                en_containers.iter().map(|c| c.custom_name.as_str()).collect::<Vec<_>>()
            );
            assert_ne!(en_containers[0].custom_name, ru_containers[0].custom_name);
        }
    }

    #[test]
    fn v050_hillside_doorway_approach_and_chandeliers_clear_headroom() {
        // Sloped front yard rising +2..+3 blocks above origin right outside the South doorway.
        let mut req = sample_request("cozy cottage", 9123);
        req.width = 13;
        req.depth = 15;
        req.height = 12;
        req.adapt_terrain = true;
        let w = usize::from(req.width);
        let d = usize::from(req.depth);
        let mut terrain = vec![0i32; w * d];
        for z in 0..d {
            for x in 0..w {
                if z >= 10 {
                    terrain[z * w + x] = 3;
                }
            }
        }
        let bp = generate_blueprint(&req, Some(&terrain));
        let door = bp.door_pos.expect("cottage must have front door");
        // Verify walking headroom right outside front door (+Z toward hillside) stays clear (`air`) or stair steps.
        let mut found_step_or_clear = false;
        for dz in 1..=3 {
            let pz = door[2] + dz;
            if pz < i32::from(bp.depth) {
                for dy in 0..=2 {
                    if let Some(b) = bp.blocks.get(&[door[0], door[1] + dy, pz])
                        && (b.block_id.ends_with("_stairs") || b.block_id == "minecraft:air")
                    {
                        found_step_or_clear = true;
                    }
                }
            }
        }
        assert!(found_step_or_clear, "expected hillside terrace stairs/clearance at front doorway");

        // Tall 13x13x14 hall should produce chandelier lantern(s) + chain with clear walking headroom underneath.
        let mut req2 = sample_request("grand manor hall", 8888);
        req2.width = 13;
        req2.depth = 13;
        req2.height = 14;
        let bp2 = generate_blueprint(&req2, None);
        let has_chain_lantern = bp2
            .blocks
            .values()
            .any(|b| b.block_id == "minecraft:iron_chain");
        assert!(has_chain_lantern, "expected ceiling chain chandelier in tall hall");
    }

    #[test]
    fn v060_categories_dungeon_and_estate_parse_and_roundtrip() {
        assert_eq!(BuildCategory::ALL.len(), 14);
        assert_eq!(BuildCategory::from_slug("dungeon"), Some(BuildCategory::Dungeon));
        assert_eq!(BuildCategory::from_slug("шахта"), Some(BuildCategory::Dungeon));
        assert_eq!(BuildCategory::from_slug("estate"), Some(BuildCategory::Estate));
        assert_eq!(BuildCategory::from_slug("усадьба"), Some(BuildCategory::Estate));

        // Explicit descriptions disambiguate Dungeon vs existing MausoleumCrypt, and Estate vs Settlement.
        let s_dun = parse_description(
            BuildCategory::House,
            BuildStyle::Auto,
            "deep abandoned mineshaft dungeon with rails and ore veins",
            27,
            27,
            18,
            NoiseLevel::Balanced,
            42,
        );
        assert_eq!(s_dun.category, BuildCategory::Dungeon);

        let s_crypt = parse_description(
            BuildCategory::Temple,
            BuildStyle::Gothic,
            "мрачный склеп мавзолей с подземными катакомбами и саркофагом",
            19,
            21,
            14,
            NoiseLevel::Balanced,
            42,
        );
        assert_eq!(s_crypt.category, BuildCategory::Temple);

        let s_est = parse_description(
            BuildCategory::House,
            BuildStyle::Auto,
            "сельская усадьба поместье с домом, амбаром и полями",
            35,
            33,
            20,
            NoiseLevel::Balanced,
            42,
        );
        assert_eq!(s_est.category, BuildCategory::Estate);

        // BG2 share code round-trip for Dungeon (9) and Estate (10) preserves exact indices!
        for &cat in &[BuildCategory::Dungeon, BuildCategory::Estate] {
            let p = crate::persist::SavedPreset {
                name: format!("Test {:?}", cat),
                category: cat,
                style: BuildStyle::Medieval,
                roof_raw: 0,
                toggles: [0; 5],
                width: 27,
                depth: 27,
                height: 18,
                noise_level: NoiseLevel::Balanced,
                adapt_terrain: true,
                air_only: false,
                wear_pct: 15,
                overgrowth_pct: 20,
                seed: 777_123,
                description: cat.slug().to_owned(),
                palette_manual: false,
                palette: MaterialPalette::preset(0),
                living_decor: true,
                spawn_npcs: false,
                blend_radius: 4,
            };
            let code = crate::persist::encode_share_code(&p);
            let decoded = crate::persist::decode_share_code(&code, None).expect("decode BG2 share code");
            assert_eq!(decoded.category, cat);
        }
    }

    #[test]
    fn v060_dungeon_generates_multi_tier_subterranean_vaults_with_stairs_and_loot() {
        // 1. Multi-tier Gothic Dungeon Catacombs
        let mut req = sample_request("multi-level gothic dungeon catacombs with chests", 6001);
        req.category = BuildCategory::Dungeon;
        req.style = BuildStyle::Gothic;
        req.width = 27;
        req.depth = 27;
        req.height = 18;
        let bp = generate_blueprint(&req, None);
        assert_eq!(bp.spec.category, BuildCategory::Dungeon);
        let min_y = bp.blocks.keys().map(|p| p[1]).min().unwrap_or(0);
        assert!(min_y <= -12, "Multi-tier dungeon must extend deep underground (min_y={min_y})");

        let stair_positions: Vec<[i32; 3]> = bp
            .blocks
            .iter()
            .filter_map(|(pos, b)| {
                if pos[1] <= 0 && b.block_id.ends_with("_stairs") {
                    Some(*pos)
                } else {
                    None
                }
            })
            .collect();
        assert!(!stair_positions.is_empty(), "Dungeon tiers must be linked by subterranean stairs");
        for [sx, sy, sz] in stair_positions {
            let head_clear = bp
                .blocks
                .get(&[sx, sy + 2, sz])
                .is_none_or(|b| b.block_id == "minecraft:air");
            assert!(head_clear, "Subterranean stair at [{sx},{sy},{sz}] must keep headroom clear");
        }

        let loot = crate::server_builder::collect_post_build_containers(&bp, req.seed as u64, crate::model::UiLang::En);
        assert!(!loot.is_empty(), "Dungeon vaults must populate chest/barrel/brewing loot");
        assert!(
            loot.iter().any(|c| c.custom_name.contains("Crypt") || c.custom_name.contains("Miner")),
            "Dungeon containers should get dedicated Dungeon loot titles"
        );

        // 2. Abandoned Mineshaft Dungeon with rails & ores
        let mut mine_req = sample_request("deep abandoned timber mineshaft dungeon with rails and ore veins", 6002);
        mine_req.category = BuildCategory::Dungeon;
        mine_req.style = BuildStyle::Medieval;
        mine_req.width = 29;
        mine_req.depth = 29;
        mine_req.height = 18;
        let mine_bp = generate_blueprint(&mine_req, None);
        assert!(mine_bp.blocks.values().any(|b| b.block_id == "minecraft:rail"), "Mineshaft must lay rail tracks");
        assert!(
            mine_bp.blocks.values().any(|b| b.block_id.ends_with("_ore")),
            "Mineshaft walls should expose ore veins"
        );

        // 3. Dwarven Lava Sanctum
        let mut dw_req = sample_request("dwarven subterranean lava forge dungeon sanctum", 6003);
        dw_req.category = BuildCategory::Dungeon;
        dw_req.style = BuildStyle::Nordic;
        dw_req.width = 27;
        dw_req.depth = 27;
        dw_req.height = 18;
        let dw_bp = generate_blueprint(&dw_req, None);
        assert!(
            dw_bp.blocks.values().any(|b| b.block_id == "minecraft:lava"),
            "Dwarven foundry dungeon should include sub-floor lava channel"
        );
    }

    #[test]
    fn v060_estate_generates_compound_manor_barn_crops_paddock_and_windmill() {
        let mut req = sample_request("rural country manor estate compound with barn and fields", 6101);
        req.category = BuildCategory::Estate;
        req.style = BuildStyle::Cottage;
        req.width = 35;
        req.depth = 35;
        req.height = 20;
        req.adapt_terrain = true;
        // Gently sloping terrain across estate footprint
        let mut terrain = vec![0i32; 35 * 35];
        for z in 0..35usize {
            for x in 0..35usize {
                terrain[z * 35 + x] = (x as i32 / 10) - 1;
            }
        }
        let bp = crate::generator::generate_blueprint_with_water(&req, Some(&terrain), None);
        assert_eq!(bp.spec.category, BuildCategory::Estate);
        assert!(bp.non_air_count() > 1500, "Large 35x35 estate should build rich compound buildings & fields");
        assert!(
            bp.blocks.values().any(|b| b.block_id == "minecraft:farmland"),
            "Estate SW quadrant must create irrigated farmland"
        );
        assert!(
            bp.blocks.values().any(|b| matches!(b.block_id.as_str(), "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" | "minecraft:beetroots")),
            "Estate fields must grow crops"
        );
        assert!(
            bp.blocks.values().any(|b| b.block_id == "minecraft:water_cauldron"),
            "Livestock paddock should place water trough cauldron"
        );
        assert!(
            bp.blocks.values().any(|b| b.block_id == "minecraft:hay_block"),
            "Estate barn/paddock should include hay blocks"
        );
        let containers = crate::server_builder::collect_post_build_containers(&bp, req.seed as u64, crate::model::UiLang::En);
        assert!(
            !containers.is_empty(),
            "Estate manor/barn/field must contain populated barrels/chests"
        );

        // Compact 15x15 estate on hillside should adapt smoothly without errors
        let mut small_req = sample_request("cozy country homestead estate", 6102);
        small_req.category = BuildCategory::Estate;
        small_req.width = 15;
        small_req.depth = 15;
        small_req.height = 12;
        small_req.adapt_terrain = true;
        let small_terrain: Vec<i32> = (0..(15 * 15)).map(|i| ((i % 15) as i32) / 5 - 1).collect();
        let small_bp = crate::generator::generate_blueprint_with_water(&small_req, Some(&small_terrain), None);
        assert!(small_bp.blocks.values().any(|b| b.block_id == "minecraft:farmland"));
        assert!(small_bp.non_air_count() > 150);
    }

    #[test]
    fn v060_persistent_undo_redo_survives_simulated_restart_and_world_switch() {
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            crate::server_builder::clear_history_memory_cache();

            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let world_a_files = MemWorldFiles::default();
            let mut host = TestWorldHost::new(keys.clone());
            host.world_dir_path = Some(PathBuf::from("test_world_alpha"));
            host.world_files = world_a_files.clone();

            host.fake.join(88, 100);
            let player = host.player(88).unwrap();
            host.set_player_perm(player.uuid(), 2);
            let before_snapshot = host.world_blocks.clone();

            let mut req = sample_request("cozy timber cottage with porch", 6601);
            req.width = 11;
            req.depth = 11;
            req.height = 10;
            let build_resp = execute_build(&mut host, Some(player), req);
            assert!(build_resp.ok, "build failed: {}", build_resp.message);
            assert_ne!(host.world_blocks, before_snapshot);

            // Verify BGH1 binary snapshot was persisted to World A's data files!
            let undo_key = format!("undo_{:032x}.bin", player.uuid());
            assert!(
                world_a_files.files.lock().unwrap().get(&undo_key).is_some_and(|b| b.starts_with(b"BGH1")),
                "Undo history must be written as BGH1 to world_data_files"
            );

            // Simulate full game restart: wipe ALL in-memory caches while keeping world_a_files!
            crate::server_builder::clear_history_memory_cache();

            let undo_resp = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(undo_resp.ok, "Persisted Undo after restart must succeed: {}", undo_resp.message);
            assert!(undo_resp.message.contains("redo: 1/") || undo_resp.message.contains("возврат: 1/"));
            assert_eq!(host.world_blocks, before_snapshot, "Undo from disk must restore exact pre-build blocks");

            // Simulate another restart, then verify persisted Redo stack survives too!
            crate::server_builder::clear_history_memory_cache();
            let redo_resp = execute_redo(&mut host, Some(player), "minecraft:overworld");
            assert!(redo_resp.ok, "Persisted Redo after restart must succeed: {}", redo_resp.message);
            assert_ne!(host.world_blocks, before_snapshot, "Redo from disk must re-apply structure");

            // Switching to World B must isolate history cleanly and not leak World A's undo stack.
            host.world_dir_path = Some(PathBuf::from("test_world_beta"));
            host.world_files = MemWorldFiles::default();
            let b_undo = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(!b_undo.ok, "Switching worlds must not inherit another save's undo stack");

            // Switching back to World A must automatically reload World A's persisted undo stack!
            host.world_dir_path = Some(PathBuf::from("test_world_alpha"));
            host.world_files = world_a_files;
            let a_undo_again = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(a_undo_again.ok, "Returning to World A must reload its disk-saved Undo history");
            assert_eq!(host.world_blocks, before_snapshot);

            crate::server_builder::clear_history_memory_cache();
        });
    }

    #[test]
    fn v061_grounding_and_no_dry_stilts() {
        use crate::model::{BuildCategory, BuildStyle};

        // Steep dry slope must build masonry down-slope instead of switching onto wooden stilts.
        let mut dry_req = sample_request("medieval stone manor with corner turret and arcade", 6101);
        dry_req.category = BuildCategory::House;
        dry_req.style = BuildStyle::Medieval;
        dry_req.width = 19;
        dry_req.depth = 17;
        dry_req.height = 16;
        let idx_len = usize::from(dry_req.width) * usize::from(dry_req.depth);
        let mut dry_terr = vec![0i32; idx_len];
        for z in 0..i32::from(dry_req.depth) {
            for x in 0..i32::from(dry_req.width) {
                dry_terr[(z * i32::from(dry_req.width) + x) as usize] = (-x + 2).clamp(-14, 2);
            }
        }
        let dry_bp = generate_blueprint(&dry_req, Some(&dry_terr));
        assert!(
            !dry_bp.spec.on_stilts,
            "Dry hillside terrain must not trigger wooden water stilts"
        );
        assert!(
            dry_bp.blocks.iter().any(|(&[_x, y, _z], b)| {
                y <= -10 && b.block_id != "minecraft:air" && !b.block_id.ends_with("_fence")
            }),
            "Foundation skirt must extend down below y=-10 on steep dry valleys"
        );

        // Desert terrain adaptation must skirt valleys with sandstone rather than dirt/grass.
        let mut desert_req = sample_request("desert house", 6102);
        desert_req.style = BuildStyle::Desert;
        desert_req.width = 15;
        desert_req.depth = 15;
        desert_req.height = 12;
        let d_terr = vec![-10i32; 15 * 15];
        let desert_bp = generate_blueprint(&desert_req, Some(&d_terr));
        assert!(
            !desert_bp
                .blocks
                .values()
                .any(|b| b.block_id == "minecraft:grass_block" || b.block_id == "minecraft:dirt"),
            "Desert terrain skirts must blend with sandstone rather than grass/dirt"
        );

        // True deep water must still activate stilt piling mode.
        let mut water_req = sample_request("lake house on water", 6103);
        water_req.width = 13;
        water_req.depth = 13;
        water_req.height = 12;
        let w_terr = vec![-3i32; 13 * 13];
        let w_depths = vec![4i32; 13 * 13];
        let water_bp = crate::generator::generate_blueprint_with_water(
            &water_req,
            Some(&w_terr),
            Some(&w_depths),
        );
        assert!(
            water_bp.spec.on_stilts,
            "Submerged footprints must keep aquatic stilts enabled"
        );
    }

    #[test]
    fn v061_settlement_grading_canals_gates_and_pockets() {
        use crate::model::{BuildCategory, BuildStyle};

        let w = 60i32;
        let mut terr = vec![0i32; (w * w) as usize];
        for z in 0..w {
            for x in 0..w {
                terr[(z * w + x) as usize] = (((x - w / 2) / 3) + ((z - w / 2) / 4)).clamp(-10, 10);
            }
        }
        let mut req = sample_request("fortified desert canal settlement", 6105);
        req.category = BuildCategory::Settlement;
        req.style = BuildStyle::Desert;
        req.width = 60;
        req.depth = 60;
        req.height = 18;
        let bp = generate_blueprint(&req, Some(&terr));

        // Walls/bastions/lots in deep -10 valleys must ground below y=-6.
        assert!(
            bp.blocks.iter().any(|(&[_x, y, _z], b)| {
                y <= -8
                    && b.block_id != "minecraft:air"
                    && b.block_id != "minecraft:sand"
                    && b.block_id != "minecraft:red_sand"
            }),
            "Sloped settlements must ground structural walls and lot plinths deep into valleys"
        );

        // Every placed water canal cell must stay open to the sky or sit cleanly under a bridge span.
        let mut floating_sand_over_water = 0usize;
        for (&[x, y, z], blk) in &bp.blocks {
            if blk.block_id == "minecraft:water" {
                let above_sand = bp.blocks.get(&[x, y + 1, z]).is_some_and(|u| {
                    matches!(u.block_id.as_str(), "minecraft:sand" | "minecraft:red_sand" | "minecraft:sandstone")
                });
                if above_sand {
                    floating_sand_over_water += 1;
                }
            }
        }
        assert_eq!(
            floating_sand_over_water, 0,
            "Canal water channels must clear sand/terrain overburden above water surface"
        );

        // Every door in the settlement must be a complete lower+upper pair on a solid block.
        for (&[x, y, z], blk) in &bp.blocks {
            if blk.block_id.ends_with("_door") {
                let is_upper = blk.props.iter().any(|&(k, v)| k == "half" && v == "upper");
                if is_upper {
                    assert!(
                        bp.blocks.get(&[x, y - 1, z]).is_some_and(|b| b.block_id.ends_with("_door")),
                        "Settlement upper door half at ({x},{y},{z}) missing lower half"
                    );
                } else {
                    assert!(
                        bp.blocks.get(&[x, y + 1, z]).is_some_and(|b| b.block_id.ends_with("_door")),
                        "Settlement lower door half at ({x},{y},{z}) missing upper half"
                    );
                    assert!(
                        bp.blocks.get(&[x, y - 1, z]).is_some_and(|b| b.block_id != "minecraft:air"),
                        "Settlement door at ({x},{y},{z}) must rest on solid support"
                    );
                }
            }
        }
    }

    #[test]
    fn v061_complete_doors_porch_stairs_and_windows() {
        use crate::model::{BuildCategory, BuildStyle, MaterialPalette};

        for &pal_idx in &[0usize, 1, 5, 7, 16] {
            for &cat in &[
                BuildCategory::House,
                BuildCategory::Temple,
                BuildCategory::Castle,
                BuildCategory::Forge,
            ] {
                for &style in &[
                    BuildStyle::Medieval,
                    BuildStyle::Classical,
                    BuildStyle::Steampunk,
                    BuildStyle::Desert,
                ] {
                    for seed in [1i64, 2, 7, 9, 10] {
                        let mut r = sample_request("cozy house with porch", seed);
                        r.category = cat;
                        r.style = style;
                        r.width = 13;
                        r.depth = 13;
                        r.height = 14;
                        r.palette = MaterialPalette::preset(pal_idx);
                        let b = generate_blueprint(&r, None);
                        for (&[x, y, z], blk) in &b.blocks {
                            if !blk.block_id.ends_with("_door") {
                                continue;
                            }
                            let is_upper =
                                blk.props.iter().any(|&(k, v)| k == "half" && v == "upper");
                            if is_upper {
                                assert!(
                                    b.blocks
                                        .get(&[x, y - 1, z])
                                        .is_some_and(|v| v.block_id.ends_with("_door")),
                                    "Orphan upper door in pal={pal_idx} {cat:?} {style:?} seed={seed}"
                                );
                                continue;
                            }
                            assert!(
                                b.blocks
                                    .get(&[x, y + 1, z])
                                    .is_some_and(|v| v.block_id.ends_with("_door")),
                                "Half-door missing upper in pal={pal_idx} {cat:?} {style:?} seed={seed}"
                            );
                            assert!(
                                b.blocks
                                    .get(&[x, y - 1, z])
                                    .is_some_and(|v| v.block_id != "minecraft:air"),
                                "Door floating without threshold at ({x},{y},{z})"
                            );
                            let lintel_bad = b.blocks.get(&[x, y + 2, z]).is_none_or(|v| {
                                let id = v.block_id.as_str();
                                id == "minecraft:air" || id.contains("glass") || id.ends_with("_grate")
                            });
                            assert!(
                                !lintel_bad,
                                "Transparent/air door lintel in pal={pal_idx} {cat:?} {style:?} seed={seed}"
                            );

                            let facing = blk
                                .props
                                .iter()
                                .find(|&&(k, _)| k == "facing")
                                .map(|&(_, v)| v)
                                .unwrap_or("south");
                            let (jx, jz) = match facing {
                                "east" | "west" => (0, 1),
                                _ => (1, 0),
                            };
                            for sign in [-1, 1] {
                                for dy in 0..=2 {
                                    let jb = b.blocks.get(&[x + sign * jx, y + dy, z + sign * jz]);
                                    let has_glass_or_grate = jb.is_some_and(|v| {
                                        v.block_id.contains("glass") || v.block_id.ends_with("_grate")
                                    });
                                    assert!(
                                        !has_glass_or_grate,
                                        "Glass/grate jamb directly adjacent to door in pal={pal_idx} {cat:?} {style:?} seed={seed}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Verify elevated 1-step-up porches ALWAYS generate grounded front stairs (Photo 6 bug).
        let mut porch_req = sample_request("cozy cottage with porch", 6111);
        porch_req.width = 13;
        porch_req.depth = 13;
        porch_req.height = 12;
        porch_req.style = BuildStyle::Cottage;
        let porch_bp = generate_blueprint(&porch_req, None);
        assert!(
            porch_bp.blocks.iter().any(|(&[x, y, z], b)| {
                b.block_id.ends_with("_stairs")
                    && porch_bp.door_pos.is_some_and(|[dx, dy, dz]| {
                        (x - dx).abs() <= 2 && (z - dz).abs() <= 4 && (y - dy).abs() <= 2
                    })
            }),
            "Porch entrances must generate front stairs connecting porch deck to ground"
        );

        // Verify air_only server builds never leave single half-doors when upper slot is obstructed.
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            crate::server_builder::clear_history_memory_cache();
            crate::server_builder::clear_rate_limit_for_test();
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            host.fake.join(89, 100);
            let player = host.player(89).unwrap();
            host.set_player_perm(player.uuid(), 2);

            let mut req = sample_request("cozy house", 6120);
            req.width = 11;
            req.depth = 11;
            req.height = 10;
            req.air_only = true;
            // Pre-block upper door slot so air_only filtering drops the entire door pair cleanly.
            let preview_bp = generate_blueprint(&req, None);
            if let Some([dx, dy, dz]) = preview_bp.door_pos {
                let wx = req.origin_x + dx;
                let wy = req.origin_y + dy + 1;
                let wz = req.origin_z + dz;
                let stone_key = host.id_to_key["minecraft:cobblestone"];
                let stone_state: BlockState = BlockStateRecord::new(stone_key, 0).into();
                host.world_blocks.insert([wx, wy, wz], stone_state);
            }
            let resp = crate::server_builder::execute_build(&mut host, Some(player), req);
            assert!(resp.ok, "air_only build failed: {}", resp.message);
            for (&[wx, wy, wz], state) in &host.world_blocks {
                let bkey = BlockStateRecord::from(state).block;
                let is_door = host
                    .key_to_id
                    .get(&bkey)
                    .is_some_and(|id| id.ends_with("_door"));
                if is_door {
                    let has_partner = host
                        .world_blocks
                        .get(&[wx, wy + 1, wz])
                        .and_then(|u| host.key_to_id.get(&BlockStateRecord::from(u).block))
                        .is_some_and(|id| id.ends_with("_door"))
                        || host
                            .world_blocks
                            .get(&[wx, wy - 1, wz])
                            .and_then(|l| host.key_to_id.get(&BlockStateRecord::from(l).block))
                            .is_some_and(|id| id.ends_with("_door"));
                    assert!(
                        has_partner,
                        "air_only mode must never leave an unpaired half-door at ({wx},{wy},{wz})"
                    );
                }
            }
            crate::server_builder::clear_history_memory_cache();
        });
    }

    #[test]
    fn v061_gable_roof_ends_and_eaves_sealed() {
        use crate::model::{BuildCategory, BuildStyle};

        // Landmark gable roofs with overhanging eaves must seal both triangular gable walls and wall-to-eave seams.
        let mut lm_blocks = std::collections::BTreeMap::new();
        let grid = crate::gen_common::LocalGrid::new(13, 13, crate::model::CardinalFacing::South);
        let pal = crate::model::MaterialPalette::preset(0);
        // Wall perimeter at (2..=10, 2..=10), roof extends (1..=11, 1..=11) with 1-block overhang.
        for v in 2..=10 {
            for u in 2..=10 {
                if u == 2 || u == 10 || v == 2 || v == 10 {
                    crate::gen_common::set_uv(
                        &mut lm_blocks,
                        grid,
                        u,
                        5,
                        v,
                        crate::gen_common::PlannedBlock::simple("minecraft:bricks"),
                    );
                }
            }
        }
        crate::gen_landmarks::build_gable_roof(
            &mut lm_blocks,
            grid,
            &pal,
            42,
            1,
            11,
            1,
            11,
            5,
            16,
            true,
        );
        // Center of gable ends (u=6, v=2 and u=6, v=10) reaches ry=8; y=6..=7 must be filled solidly!
        for v_end in [2, 10] {
            for wy in 6..=7 {
                assert!(
                    crate::gen_common::get_uv(&lm_blocks, grid, 6, wy, v_end)
                        .is_some_and(|b| b.block_id != "minecraft:air"),
                    "Landmark gable end wall (6,{wy},{v_end}) must be sealed cleanly under overhanging ridge"
                );
            }
        }

        // Landmark CLI presets must never report broken door_pos blocks or orphan half-doors.
        for &(slug, cat, style, pw, pd, ph, desc) in crate::server_builder::CLI_PRESETS.iter() {
            let mut r = sample_request(desc, 7);
            r.category = cat;
            r.style = style;
            r.width = pw;
            r.depth = pd;
            r.height = ph;
            let b = generate_blueprint(&r, None);
            for (&[x, y, z], blk) in &b.blocks {
                if blk.block_id.ends_with("_door") && !blk.props.iter().any(|&(k, v)| k == "half" && v == "upper") {
                    assert!(
                        b.blocks.get(&[x, y + 1, z]).is_some_and(|v| v.block_id.ends_with("_door")),
                        "Preset [{slug}] produced incomplete door at ({x},{y},{z})"
                    );
                }
            }
        }

        // Multi-wing gable roof must seal under every cross-ridge without leaving eave sky gaps.
        let mut wing_req = sample_request("l-shaped medieval manor with wing extension", 6130);
        wing_req.category = BuildCategory::House;
        wing_req.style = BuildStyle::Medieval;
        wing_req.width = 19;
        wing_req.depth = 17;
        wing_req.height = 16;
        let wing_bp = generate_blueprint(&wing_req, None);
        assert!(wing_bp.non_air_count() > 250);
    }

    #[test]
    fn v070_three_new_categories_12_themes_and_88_presets() {
        use crate::model::{BuildCategory, RoofStyle};
        use crate::parser::StructureTheme;
        use crate::server_builder::{CLI_PRESETS, CLI_PRESET_DESCS_EN, find_cli_preset};

        assert_eq!(BuildCategory::ALL.len(), 14);
        assert_eq!(CLI_PRESETS.len(), 88);
        assert_eq!(CLI_PRESET_DESCS_EN.len(), 88);

        assert_eq!(BuildCategory::from_slug("engineering"), Some(BuildCategory::Engineering));
        assert_eq!(BuildCategory::from_slug("инженер"), Some(BuildCategory::Engineering));
        assert_eq!(BuildCategory::from_slug("agricultural"), Some(BuildCategory::Agricultural));
        assert_eq!(BuildCategory::from_slug("сельск"), Some(BuildCategory::Agricultural));
        assert_eq!(BuildCategory::from_slug("decor_art"), Some(BuildCategory::DecorArt));
        assert_eq!(BuildCategory::from_slug("арт"), Some(BuildCategory::DecorArt));

        let theme_cases = [
            ("canal navigation lock", "судоходный шлюз канал", StructureTheme::CanalLock, BuildCategory::Engineering),
            ("railway roundhouse turntable", "веерное депо паровозов", StructureTheme::RailwayRoundhouse, BuildCategory::Engineering),
            ("gantry drydock slipway", "корабельный сухой док", StructureTheme::GantryDrydock, BuildCategory::Engineering),
            ("electrical power substation", "трансформаторная подстанция", StructureTheme::PowerSubstation, BuildCategory::Engineering),
            ("vineyard wine press house", "террасный виноградник", StructureTheme::VineyardPress, BuildCategory::Agricultural),
            ("grain farmstead with silos", "фермерский элеватор с силосом", StructureTheme::GrainSiloFarm, BuildCategory::Agricultural),
            ("blossom orchard apiary", "пасека и ульи в саду", StructureTheme::OrchardApiary, BuildCategory::Agricultural),
            ("royal horse stables", "королевская конюшня", StructureTheme::HorseStables, BuildCategory::Agricultural),
            ("grand plaza fountain", "городской фонтан на площади", StructureTheme::PlazaFountain, BuildCategory::DecorArt),
            ("monumental guardian statue", "монументальная статуя стража", StructureTheme::GuardianStatue, BuildCategory::DecorArt),
            ("topiary hedge maze park", "парковый лабиринт из живой изгороди", StructureTheme::HedgeMazePark, BuildCategory::DecorArt),
            ("japanese zen garden karesansui", "японский сад камней карэсансуй", StructureTheme::ZenKaresansui, BuildCategory::DecorArt),
        ];

        for (en, ru, expected_theme, expected_cat) in theme_cases {
            let s_en = parse_description(expected_cat, BuildStyle::Auto, en, 21, 21, 14, NoiseLevel::Balanced, 70);
            let s_ru = parse_description(expected_cat, BuildStyle::Auto, ru, 21, 21, 14, NoiseLevel::Balanced, 70);
            assert_eq!(s_en.theme, expected_theme, "EN '{en}'");
            assert_eq!(s_ru.theme, expected_theme, "RU '{ru}'");
            assert_eq!(s_en.roof_style != RoofStyle::Flat || expected_theme == StructureTheme::PowerSubstation, true);
        }

        // Verify all 12 newly added presets [76..88] build cleanly with rich blocks & living plans.
        for idx in 76..88 {
            let (slug, cat, style, w, d, h, desc_ru) = CLI_PRESETS[idx];
            assert_eq!(find_cli_preset(slug).map(|p| p.0), Some(slug));
            let mut req = sample_request(desc_ru, 700 + idx as i64);
            req.category = cat;
            req.style = style;
            req.width = w;
            req.depth = d;
            req.height = h;
            req.living_decor = true;
            req.spawn_npcs = true;
            req.palette = crate::parser::infer_palette_with_style_and_seed(style, desc_ru, req.seed as u64);
            let bp = generate_blueprint(&req, None);
            assert!(
                bp.non_air_count() >= 180,
                "v0.7.0 preset '{slug}' generated too few blocks ({})",
                bp.non_air_count()
            );
            assert!(
                !bp.living.signs.is_empty() || !bp.living.banners.is_empty() || !bp.living.lecterns.is_empty(),
                "v0.7.0 preset '{slug}' should populate LivingPlan"
            );
            for (pos, blk) in &bp.blocks {
                assert_ne!(blk.block_id, "minecraft:chain", "Removed 26.3 block in '{slug}' at {pos:?}");
                if blk.block_id.ends_with("_leaves") {
                    assert!(
                        blk.props.iter().any(|&(k, v)| k == "persistent" && v == "true"),
                        "Decaying leaves in '{slug}' at {pos:?}"
                    );
                }
                if blk.block_id.ends_with("_door") && !blk.block_id.ends_with("trapdoor") {
                    let is_lower = blk.props.iter().any(|&(k, v)| k == "half" && v == "lower");
                    let mate = [pos[0], if is_lower { pos[1] + 1 } else { pos[1] - 1 }, pos[2]];
                    assert!(
                        bp.blocks.get(&mate).is_some_and(|m| m.block_id.ends_with("_door")),
                        "Incomplete door in '{slug}' at {pos:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn v070_living_interiors_redstone_books_banners_npcs_and_bg2_v5() {
        use crate::gen_common::LivingEntityKind;
        use crate::persist::{SavedPreset, decode_share_code_lang, encode_share_code};

        // 1. Castle / Tavern / House with Living Decor ON + NPCs ON vs OFF filtering
        let mut req = sample_request("medieval castle keep with library and courtyard", 7101);
        req.category = BuildCategory::Castle;
        req.style = BuildStyle::Medieval;
        req.width = 21;
        req.depth = 19;
        req.height = 18;
        req.living_decor = true;
        req.spawn_npcs = false;
        let bp_decor = generate_blueprint(&req, None);
        assert!(
            bp_decor.blocks.values().any(|b| b.block_id.ends_with("_pressure_plate")),
            "Threshold should place pressure plate"
        );
        assert!(
            bp_decor.blocks.values().any(|b| b.block_id == "minecraft:lever"),
            "Castle entrance should place gate lever"
        );
        assert!(
            bp_decor.blocks.values().any(|b| b.block_id == "minecraft:daylight_detector"
                && b.props.iter().any(|&(k, v)| k == "inverted" && v == "true")),
            "Yard should include inverted daylight sensor lamp"
        );
        assert!(!bp_decor.living.banners.is_empty(), "Expected wall banners");
        assert!(!bp_decor.living.signs.is_empty(), "Expected entrance/interior signs");
        assert!(!bp_decor.living.lecterns.is_empty(), "Expected readable lectern book");
        assert!(
            bp_decor.living.entities.iter().any(|e| e.is_decor),
            "Expected decor entities (ArmorStand / Painting / ItemFrame)"
        );
        assert!(
            bp_decor.living.entities.iter().all(|e| e.is_decor),
            "NPCs must NOT spawn when spawn_npcs=false"
        );

        let mut req_npcs = req.clone();
        req_npcs.spawn_npcs = true;
        let bp_npcs = generate_blueprint(&req_npcs, None);
        assert!(
            bp_npcs.living.entities.iter().any(|e| matches!(e.kind, LivingEntityKind::Villager | LivingEntityKind::IronGolem)),
            "spawn_npcs=true should populate Villager/IronGolem"
        );

        let mut req_off = req.clone();
        req_off.living_decor = false;
        req_off.spawn_npcs = false;
        let bp_off = generate_blueprint(&req_off, None);
        assert!(bp_off.living.signs.is_empty());
        assert!(bp_off.living.banners.is_empty());
        assert!(bp_off.living.entities.is_empty());

        // 2. Dungeon redstone trap + Estate livestock paddock & crops
        let mut dreq = sample_request("multi-tier gothic dungeon vaults with traps", 7102);
        dreq.category = BuildCategory::Dungeon;
        dreq.style = BuildStyle::Gothic;
        dreq.width = 25;
        dreq.depth = 25;
        dreq.height = 18;
        dreq.living_decor = true;
        dreq.spawn_npcs = true;
        let dbp = generate_blueprint(&dreq, None);
        assert!(dbp.blocks.values().any(|b| b.block_id == "minecraft:dispenser"));
        assert!(dbp.living.entities.iter().any(|e| matches!(e.kind, LivingEntityKind::Skeleton | LivingEntityKind::Zombie)));

        let mut ereq = sample_request("country manor estate with barn and livestock", 7103);
        ereq.category = BuildCategory::Estate;
        ereq.style = BuildStyle::Cottage;
        ereq.width = 33;
        ereq.depth = 31;
        ereq.height = 18;
        ereq.living_decor = true;
        ereq.spawn_npcs = true;
        let ebp = generate_blueprint(&ereq, None);
        assert!(ebp.living.entities.iter().any(|e| matches!(e.kind, LivingEntityKind::Cow | LivingEntityKind::Sheep | LivingEntityKind::Horse)));
        assert!(ebp.living.entities.iter().any(|e| e.kind == LivingEntityKind::Villager));

        // 3. BG2 v5 share code roundtrip + backwards decode check
        let preset = SavedPreset {
            name: "LivingKeep".into(),
            width: 21,
            depth: 19,
            height: 18,
            category: BuildCategory::Castle,
            style: BuildStyle::Medieval,
            roof_raw: 1,
            toggles: [1, 1, 1, 0, 1],
            noise_level: NoiseLevel::Balanced,
            adapt_terrain: true,
            air_only: false,
            wear_pct: 15,
            overgrowth_pct: 20,
            description: "castle keep".into(),
            seed: 4242,
            palette_manual: false,
            palette: MaterialPalette::preset(0),
            living_decor: false,
            spawn_npcs: true,
            blend_radius: 7,
        };
        let code = encode_share_code(&preset);
        let decoded = decode_share_code_lang(&code, Some("LivingKeep"), crate::model::UiLang::En)
            .expect("BG2 v5 roundtrip");
        assert_eq!(decoded.living_decor, false);
        assert_eq!(decoded.spawn_npcs, true);
        assert_eq!(decoded.blend_radius, 7);
        assert_eq!(decoded.wear_pct, 15);
        assert_eq!(decoded.overgrowth_pct, 20);
    }

    #[test]
    fn v070_radial_terrain_blend_and_biome_surfaces() {
        use crate::generator::biome_surface_blocks;

        assert_eq!(biome_surface_blocks("minecraft:badlands", BuildStyle::Auto).0, "minecraft:red_sand");
        assert_eq!(biome_surface_blocks("minecraft:desert", BuildStyle::Auto).0, "minecraft:sand");
        assert_eq!(biome_surface_blocks("minecraft:snowy_taiga", BuildStyle::Auto).0, "minecraft:snow_block");
        assert_eq!(biome_surface_blocks("minecraft:taiga", BuildStyle::Auto).0, "minecraft:podzol");
        assert_eq!(biome_surface_blocks("minecraft:mangrove_swamp", BuildStyle::Auto).1, "minecraft:mud");

        // Steep cliff dropoff (-7) & high hill (+6) with Snowy Taiga biome and wide blend_radius=6
        let mut req = sample_request("nordic cabin", 7201);
        req.width = 23;
        req.depth = 23;
        req.height = 14;
        req.style = BuildStyle::Nordic;
        req.biome_hint = "minecraft:snowy_taiga".into();
        req.adapt_terrain = true;
        req.blend_radius = 6;

        let mut offsets = vec![0i32; 23 * 23];
        for z in 0..23 {
            for x in 0..23 {
                offsets[z * 23 + x] = if x >= 18 {
                    -7
                } else if z <= 3 {
                    6
                } else {
                    0
                };
            }
        }
        let bp_wide = generate_blueprint(&req, Some(&offsets));
        assert!(
            bp_wide.blocks.iter().any(|(&[x, y, _], b)| x >= 18 && y < 0 && b.block_id == "minecraft:snow_block"),
            "Snowy Taiga downhill apron should place snow_block ramps down to cliff"
        );
        assert!(
            bp_wide.blocks.iter().any(|(&[_, y, z], b)| z <= 3 && y >= 1 && b.block_id == "minecraft:snow_block"),
            "Snowy Taiga uphill cut should smoothly terrace snow_block"
        );

        let mut req_zero = req.clone();
        req_zero.blend_radius = 0;
        let bp_none = generate_blueprint(&req_zero, Some(&offsets));
        let outer_wide_count = bp_wide.blocks.iter().filter(|(pos, b)| pos[0] >= 19 && b.block_id != "minecraft:air").count();
        let outer_none_count = bp_none.blocks.iter().filter(|(pos, b)| pos[0] >= 19 && b.block_id != "minecraft:air").count();
        assert!(
            outer_wide_count > outer_none_count,
            "blend_radius=6 must generate outer skirt blocks vs blend_radius=0 ({outer_wide_count} vs {outer_none_count})"
        );

        // Landmark smooth edges test on PlazaFountain
        let mut lm_req = sample_request("grand plaza fountain", 7202);
        lm_req.category = BuildCategory::DecorArt;
        lm_req.width = 23;
        lm_req.depth = 23;
        lm_req.height = 14;
        lm_req.biome_hint = "minecraft:badlands".into();
        lm_req.blend_radius = 5;
        let bp_lm = generate_blueprint(&lm_req, Some(&offsets));
        assert!(
            bp_lm.blocks.values().any(|b| b.block_id == "minecraft:red_sand" || b.block_id == "minecraft:red_sandstone"),
            "Landmark edge blend in badlands should integrate red_sand/red_sandstone surfaces"
        );
    }

    #[test]
    fn v070_road_connector_auto_bridge_and_server_undo() {
        use crate::gen_road::generate_road_connector;
        use crate::model::RoadConnectRequest;
        use crate::server_builder::{clear_history_memory_cache, clear_rate_limit_for_test, execute_road_connect};

        let rreq = RoadConnectRequest {
            dimension: "minecraft:overworld".into(),
            start_x: 4,
            start_y: 64,
            start_z: 14,
            end_x: 28,
            end_y: 64,
            end_z: 14,
            road_width: 3,
            auto_bridge: true,
            style: BuildStyle::Medieval,
            seed: 7301,
            air_only: false,
            blend_radius: 4,
            living_decor: true,
            spawn_npcs: true,
            biome_hint: String::new(),
            palette: MaterialPalette::preset(0),
        };
        let bytes = encode_payload(&rreq, 16_384).expect("encode RoadConnectRequest");
        let rdec: RoadConnectRequest = decode_payload(&bytes).expect("decode RoadConnectRequest");
        assert_eq!(rreq, rdec);

        // Simulate river channel (`water_depths=3`) across x=14..=18 & gentle hill slope at x=22..32
        let bw = 35u16;
        let bd = 29u16;
        let mut offsets = vec![0i32; usize::from(bw) * usize::from(bd)];
        let mut depths = vec![0i32; usize::from(bw) * usize::from(bd)];
        for z in 0..i32::from(bd) {
            for x in 0..i32::from(bw) {
                let idx = (z * i32::from(bw) + x) as usize;
                if (14..=18).contains(&x) {
                    offsets[idx] = -1;
                    depths[idx] = 3;
                } else if x >= 22 {
                    offsets[idx] = ((x - 21) / 3).min(3);
                }
            }
        }
        let road_bp = generate_road_connector(&rreq, &offsets, &depths, [0, 64, 0], bw, bd);
        assert!(!road_bp.blocks.is_empty());
        assert!(!road_bp.living.signs.is_empty(), "Road should include waypost signs");
        assert!(
            road_bp.blocks.values().any(|b| b.block_id == "minecraft:daylight_detector"),
            "Road shoulders should include automated daylight streetlamps"
        );
        // Check arched bridge over river (`x=16` has solid deck strictly above water `y >= 1` and fence rail)
        let has_bridge_deck = road_bp
            .blocks
            .iter()
            .any(|(&[x, y, z], b)| x == 16 && z == 14 && y >= 0 && b.block_id != "minecraft:air" && b.block_id != "minecraft:water");
        assert!(has_bridge_deck, "Road with auto_bridge=true must build elevated bridge deck over water");

        // End-to-end server test: Build with NPCs + Outer Blend -> Undo discards entities & restores all blocks!
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            clear_history_memory_cache();
            clear_rate_limit_for_test();
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            let hill_grass: BlockState = BlockStateRecord::new(host.id_to_key["minecraft:grass_block"], 0).into();
            // Add sloped hill beside site [10..23, 20..31] so outer halo blends outside bounds too
            for x in 23..30 {
                for z in 18..34 {
                    for y in 64..=66 {
                        host.world_blocks.insert([x, y, z], hill_grass);
                    }
                }
            }
            host.fake.join(77, 100);
            let player = host.player(77).unwrap();

            let mut breq = sample_request("medieval castle with guards and villagers", 7302);
            breq.category = BuildCategory::Castle;
            breq.style = BuildStyle::Medieval;
            breq.width = 15;
            breq.depth = 15;
            breq.height = 16;
            breq.origin_x = 10;
            breq.origin_y = 64;
            breq.origin_z = 18;
            breq.living_decor = true;
            breq.spawn_npcs = true;
            breq.blend_radius = 4;

            let resp = execute_build(&mut host, Some(player), breq);
            assert!(resp.ok, "Build failed: {}", resp.message);
            assert!(
                host.summoned_entities.iter().any(|e| e.4),
                "Living Interiors + NPCs should summon entities on server"
            );

            let undo_resp = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(undo_resp.ok, "Undo failed: {}", undo_resp.message);
            assert!(
                host.summoned_entities.iter().all(|e| !e.4),
                "Undo must discard all entities summoned for the undone build"
            );

            // Road Connect execution & Undo on server
            clear_rate_limit_for_test();
            let r_server = RoadConnectRequest {
                start_x: 5,
                start_y: 63,
                start_z: 10,
                end_x: 20,
                end_y: 63,
                end_z: 16,
                blend_radius: 2,
                ..rreq.clone()
            };
            let road_resp = execute_road_connect(&mut host, Some(player), r_server);
            assert!(road_resp.ok, "Road build failed: {}", road_resp.message);
            assert!(road_resp.blocks_changed > 15);
            let road_undo = execute_undo(&mut host, Some(player), "minecraft:overworld");
            assert!(road_undo.ok);
        });
    }

    #[test]
    fn v071_no_floating_blocks_in_settlements_engawa_yards_or_ruins() {
        use std::collections::{HashSet, VecDeque};

        fn assert_grounded_blueprint(bp: &crate::generator::GeneratedBlueprint, offsets: &[i32], label: &str) {
            let w = i32::from(bp.width);
            let d = i32::from(bp.depth);
            let surf_y = |x: i32, z: i32| -> i32 {
                if (0..w).contains(&x) && (0..d).contains(&z) {
                    offsets.get((z * w + x) as usize).copied().unwrap_or(0)
                } else {
                    0
                }
            };

            let active: Vec<[i32; 3]> = bp
                .blocks
                .iter()
                .filter_map(|(&p, b)| {
                    (!matches!(
                        b.block_id.as_str(),
                        "minecraft:air" | "minecraft:water" | "minecraft:lava"
                    ))
                    .then_some(p)
                })
                .collect();
            assert!(!active.is_empty(), "{label}: blueprint must not be empty");
            let active_set: HashSet<[i32; 3]> = active.iter().copied().collect();
            let mut grounded: HashSet<[i32; 3]> = HashSet::new();
            let mut queue: VecDeque<[i32; 3]> = VecDeque::new();

            for &p in &active {
                let [x, y, z] = p;
                let s = surf_y(x, z);
                let below = bp.blocks.get(&[x, y - 1, z]);
                let on_ground = y <= s
                    || (y == s + 1 && below.is_none_or(|b| b.block_id != "minecraft:air"))
                    || (bp.blocks.get(&p).is_some_and(|b| b.block_id == "minecraft:lily_pad")
                        && below.is_some_and(|b| b.block_id == "minecraft:water"));
                if on_ground && grounded.insert(p) {
                    queue.push_back(p);
                }
            }

            let is_wood_or_flora = |id: &str| {
                id.ends_with("_log")
                    || id.ends_with("_wood")
                    || id.ends_with("_leaves")
                    || id.ends_with("_stem")
                    || matches!(
                        id,
                        "minecraft:bamboo"
                            | "minecraft:vine"
                            | "minecraft:azalea"
                            | "minecraft:flowering_azalea"
                    )
            };
            let is_roof_step = |id: &str| id.ends_with("_stairs") || id.ends_with("_slab");

            while let Some(cur) = queue.pop_front() {
                let cur_id = bp.blocks.get(&cur).map(|b| b.block_id.as_str()).unwrap_or("");
                let [cx, cy, cz] = cur;
                for np in [
                    [cx + 1, cy, cz],
                    [cx - 1, cy, cz],
                    [cx, cy + 1, cz],
                    [cx, cy - 1, cz],
                    [cx, cy, cz + 1],
                    [cx, cy, cz - 1],
                ] {
                    if active_set.contains(&np) && grounded.insert(np) {
                        queue.push_back(np);
                    }
                }
                if is_roof_step(cur_id) {
                    for [dx, dy, dz] in [
                        [1, 1, 0],
                        [-1, 1, 0],
                        [1, -1, 0],
                        [-1, -1, 0],
                        [0, 1, 1],
                        [0, 1, -1],
                        [0, -1, 1],
                        [0, -1, -1],
                    ] {
                        let np = [cx + dx, cy + dy, cz + dz];
                        if active_set.contains(&np)
                            && bp.blocks.get(&np).is_some_and(|nb| is_roof_step(&nb.block_id))
                            && grounded.insert(np)
                        {
                            queue.push_back(np);
                        }
                    }
                }
                if is_wood_or_flora(cur_id) {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            for dx in -1..=1 {
                                if dx == 0 && dy == 0 && dz == 0 {
                                    continue;
                                }
                                let np = [cx + dx, cy + dy, cz + dz];
                                if active_set.contains(&np)
                                    && bp.blocks.get(&np).is_some_and(|nb| is_wood_or_flora(&nb.block_id))
                                    && grounded.insert(np)
                                {
                                    queue.push_back(np);
                                }
                            }
                        }
                    }
                }
            }

            let orphans: Vec<_> = active
                .iter()
                .filter(|p| !grounded.contains(*p))
                .take(10)
                .map(|p| (*p, bp.blocks[p].block_id.as_str()))
                .collect();
            assert!(
                orphans.is_empty(),
                "{label}: found disconnected floating blocks: {orphans:?}"
            );

            // Every wall, post-mounted daylight_detector/redstone_lamp, lower door, and floor cauldron
            // must rest on non-air underneath (never hovering with air beneath its column).
            for (&[x, y, z], b) in &bp.blocks {
                let id = b.block_id.as_str();
                let s = surf_y(x, z);
                if y <= s {
                    continue;
                }
                let below_is_air = |yy: i32| {
                    bp.blocks
                        .get(&[x, yy - 1, z])
                        .is_some_and(|nb| nb.block_id == "minecraft:air")
                        || (yy - 1 > s && !bp.blocks.contains_key(&[x, yy - 1, z]))
                };
                if id == "minecraft:daylight_detector" || id == "minecraft:redstone_lamp" {
                    let mut yy = y;
                    while yy > s {
                        assert!(
                            !below_is_air(yy),
                            "{label}: streetlamp {id} at [{x}, {y}, {z}] has air gap at y={}",
                            yy - 1
                        );
                        yy -= 1;
                    }
                }
                if id.ends_with("_fence") && y == s + 1 {
                    assert!(
                        !below_is_air(y),
                        "{label}: fence post {id} at [{x}, {y}, {z}] hovers over air"
                    );
                }
            }
        }

        // 1. Sloped Badlands Settlement matching user report (`seed = 2218384`, `60x60x18`, `living_decor = true`)
        let mut sreq = sample_request(
            "огромный город с домами площадью каналами стенами и воротами",
            2_218_384,
        );
        sreq.category = BuildCategory::Settlement;
        sreq.style = crate::model::BuildStyle::Desert;
        sreq.width = 60;
        sreq.depth = 60;
        sreq.height = 18;
        sreq.biome_hint = "minecraft:badlands".to_owned();
        sreq.living_decor = true;
        sreq.adapt_terrain = true;
        sreq.blend_radius = 4;
        let sw = i32::from(sreq.width);
        let sd = i32::from(sreq.depth);
        let s_offsets: Vec<i32> = (0..(sw * sd))
            .map(|idx| {
                let x = idx % sw;
                let z = idx / sw;
                ((x - sw / 2) / 14 + (z - sd / 2) / 16).clamp(-3, 3)
            })
            .collect();
        let s_bp = crate::generator::generate_blueprint_with_water(&sreq, Some(&s_offsets), None);
        assert_grounded_blueprint(&s_bp, &s_offsets, "Badlands Sloped Settlement");

        // 2. Sloped Japanese Mansion (Engawa + yard trees + market awning + downspouts + living streetlamps)
        let mut jreq = sample_request(
            "японская усадьба с верандой энгава садом фонарями деревом навесом и водостоком",
            77_441,
        );
        jreq.style = crate::model::BuildStyle::Japanese;
        jreq.width = 28;
        jreq.depth = 28;
        jreq.height = 16;
        jreq.living_decor = true;
        jreq.adapt_terrain = true;
        jreq.blend_radius = 3;
        let jw = i32::from(jreq.width);
        let jd = i32::from(jreq.depth);
        let j_offsets: Vec<i32> = (0..(jw * jd))
            .map(|idx| {
                let x = idx % jw;
                let z = idx / jw;
                ((x - jw / 2) / 6 - (z - jd / 2) / 9).clamp(-4, 2)
            })
            .collect();
        let j_bp = crate::generator::generate_blueprint_with_water(&jreq, Some(&j_offsets), None);
        assert_grounded_blueprint(&j_bp, &j_offsets, "Sloped Japanese Mansion");
        // Verify Engawa / perimeter slabs over downhill slopes sit atop a foundation skirt rather than hovering!
        let mut engawa_checked = 0usize;
        for (&[x, y, z], b) in &j_bp.blocks {
            if (x == 3 || x == jw - 4) && y == 1 && b.block_id.ends_with("_slab") {
                let s = j_offsets[(z * jw + x) as usize];
                if s < 0 {
                    engawa_checked += 1;
                    assert!(
                        j_bp.blocks.get(&[x, 0, z]).is_some_and(|nb| nb.block_id != "minecraft:air"),
                        "Engawa slab at [{x}, {y}, {z}] over slope s={s} lacks support at y=0"
                    );
                }
            }
        }
        assert!(engawa_checked >= 2 || j_bp.non_air_count() > 500);

        // 3. Heavily eroded Ruined Castle (wear=55%, overgrowth=45%)
        let mut rreq = sample_request("древний разрушенный замок в руинах со мхом", 91_205);
        rreq.category = BuildCategory::Castle;
        rreq.style = crate::model::BuildStyle::Medieval;
        rreq.width = 24;
        rreq.depth = 24;
        rreq.height = 18;
        rreq.wear_pct = 55;
        rreq.overgrowth_pct = 45;
        let flat = vec![0i32; 24 * 24];
        let r_bp = crate::generator::generate_blueprint_with_water(&rreq, Some(&flat), None);
        assert_grounded_blueprint(&r_bp, &flat, "Ruined Castle Wear 55%");

        // 4. Sailing Ship keeps sails + dark_oak keel with zero disconnected blocks
        let ship_p = crate::server_builder::find_cli_preset("ship").unwrap();
        let mut ship_req = sample_request(ship_p.6, 4242);
        ship_req.category = ship_p.1;
        ship_req.style = ship_p.2;
        ship_req.width = ship_p.3;
        ship_req.depth = ship_p.4;
        ship_req.height = ship_p.5;
        let ship_flat = vec![0i32; usize::from(ship_req.width) * usize::from(ship_req.depth)];
        let ship_bp = crate::generator::generate_blueprint_with_water(&ship_req, Some(&ship_flat), None);
        assert!(ship_bp.blocks.values().any(|b| b.block_id == "minecraft:white_wool"));
        assert!(ship_bp.blocks.values().any(|b| b.block_id == "minecraft:dark_oak_planks"));
        assert_grounded_blueprint(&ship_bp, &ship_flat, "Sailing Ship");
    }

    #[test]
    fn v072_road_generation_matches_selected_style_biome_palette_and_connect_memory() {
        use crate::gen_road::generate_road_connector;
        use crate::model::RoadConnectRequest;
        use crate::server_builder::{
            clear_history_memory_cache, clear_rate_limit_for_test, execute_build,
        };
        use mod_api::{CommandArgument, CommandArgumentValue, CommandInvocation};

        let bw = 33u16;
        let bd = 21u16;
        let offsets = vec![0i32; usize::from(bw) * usize::from(bd)];
        let mut depths = vec![0i32; usize::from(bw) * usize::from(bd)];
        for z in 0..i32::from(bd) {
            for x in 14..=18 {
                depths[(z * i32::from(bw) + x) as usize] = 3;
            }
        }

        let base_req = RoadConnectRequest {
            dimension: "minecraft:overworld".into(),
            start_x: 4,
            start_y: 64,
            start_z: 10,
            end_x: 28,
            end_y: 64,
            end_z: 10,
            road_width: 3,
            auto_bridge: true,
            style: BuildStyle::Medieval,
            seed: 4201,
            air_only: false,
            blend_radius: 0,
            living_decor: true,
            spawn_npcs: false,
            biome_hint: String::new(),
            palette: MaterialPalette::preset(0),
        };

        // 1. All 9 explicit styles generate distinct style-matched paving, bridge lights & wayposts
        let style_expectations: &[(BuildStyle, &str, &str)] = &[
            (BuildStyle::Medieval, "minecraft:cobblestone", "minecraft:spruce_sign"),
            (BuildStyle::Cottage, "minecraft:packed_mud", "minecraft:oak_sign"),
            (BuildStyle::Japanese, "minecraft:polished_andesite", "minecraft:bamboo_sign"),
            (BuildStyle::Gothic, "minecraft:deepslate_bricks", "minecraft:soul_lantern"),
            (BuildStyle::Nordic, "minecraft:cobbled_deepslate", "minecraft:tuff_bricks"),
            (BuildStyle::Modern, "minecraft:white_concrete", "minecraft:sea_lantern"),
            (BuildStyle::Desert, "minecraft:smooth_sandstone", "minecraft:cut_sandstone"),
            (BuildStyle::Steampunk, "minecraft:mud_bricks", "minecraft:copper_lantern"),
            (BuildStyle::Classical, "minecraft:quartz_bricks", "minecraft:chiseled_quartz_block"),
        ];

        for &(style, expected_a, expected_b) in style_expectations {
            let mut req = base_req.clone();
            req.style = style;
            req.palette = MaterialPalette::preset_for_style(style);
            let bp = generate_road_connector(&req, &offsets, &depths, [0, 64, 0], bw, bd);
            assert!(
                bp.blocks.values().any(|b| b.block_id == expected_a),
                "Style {:?} road should contain '{expected_a}'",
                style
            );
            assert!(
                bp.blocks.values().any(|b| b.block_id == expected_b),
                "Style {:?} road should contain '{expected_b}'",
                style
            );
        }

        // 2. BuildStyle::Auto honors biome_hint cleanly
        let mut auto_cherry = base_req.clone();
        auto_cherry.style = BuildStyle::Auto;
        auto_cherry.biome_hint = "minecraft:cherry_grove".into();
        let bp_cherry = generate_road_connector(&auto_cherry, &offsets, &depths, [0, 64, 0], bw, bd);
        assert!(
            bp_cherry.blocks.values().any(|b| b.block_id == "minecraft:andesite" || b.block_id == "minecraft:polished_andesite"),
            "Auto style in cherry_grove should resolve to Japanese road stone"
        );

        let mut auto_desert = base_req.clone();
        auto_desert.style = BuildStyle::Auto;
        auto_desert.biome_hint = "minecraft:badlands".into();
        let bp_badlands = generate_road_connector(&auto_desert, &offsets, &depths, [0, 64, 0], bw, bd);
        assert!(
            bp_badlands.blocks.values().any(|b| b.block_id == "minecraft:smooth_red_sandstone"),
            "Auto style in badlands should use red sandstone desert paving"
        );

        // 3. Explicit custom palette override flows into road paving & fencing
        let mut custom_req = base_req.clone();
        custom_req.style = BuildStyle::Medieval;
        let mut custom_pal = MaterialPalette::preset(1);
        custom_pal.paving = "minecraft:bricks".into();
        custom_pal.fence = "minecraft:cherry_fence".into();
        custom_req.palette = custom_pal;
        let bp_custom = generate_road_connector(&custom_req, &offsets, &depths, [0, 64, 0], bw, bd);
        assert!(bp_custom.blocks.values().any(|b| b.block_id == "minecraft:bricks"));
        assert!(bp_custom.blocks.values().any(|b| b.block_id == "minecraft:cherry_fence"));

        // 4. Server end-to-end: `/buildgen connect` inherits player's last selected style,
        //    and `/buildgen road ... <width> <style>` parses explicit style cleanly.
        let entries = vec![
            ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
            ModEntry::new("test_registry", Box::new(TestRegistryMod)),
        ];
        with_mods(entries.into(), || {
            let install_entries = vec![
                ModEntry::new("building_generator", Box::new(BuildingGeneratorMod)),
                ModEntry::new("test_registry", Box::new(TestRegistryMod)),
            ];
            mod_api::__private::part_tests::install_parts_fixture(&install_entries).unwrap();
            let _world_phase =
                mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
            clear_history_memory_cache();
            clear_rate_limit_for_test();
            let keys = REGISTERED_KEYS.get().unwrap().lock().unwrap().clone();
            let mut host = TestWorldHost::new(keys);
            host.fake.join(77, 100);
            let player = host.player(77).unwrap();
            let dim = host.level("minecraft:overworld").unwrap().dimension();

            // Player builds a Modern house -> server remembers `Modern` style for that player
            let mut mreq = sample_request("modern glass house", 9001);
            mreq.style = BuildStyle::Modern;
            mreq.origin_x = 0;
            mreq.origin_y = 64;
            mreq.origin_z = 0;
            let b_resp = execute_build(&mut host, Some(player), mreq);
            assert!(b_resp.ok, "Build failed: {}", b_resp.message);

            // `/buildgen road 22 8 36 8 3` (without explicit style) inherits active `Modern` style!
            let white_conc_key = host.id_to_key["minecraft:white_concrete"];
            let road_inherit_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "road".into()],
                vec![
                    CommandArgument::new("x1".into(), CommandArgumentValue::Integer(22)),
                    CommandArgument::new("z1".into(), CommandArgumentValue::Integer(8)),
                    CommandArgument::new("x2".into(), CommandArgumentValue::Integer(36)),
                    CommandArgument::new("z2".into(), CommandArgumentValue::Integer(8)),
                    CommandArgument::new("width".into(), CommandArgumentValue::Integer(3)),
                ],
                dim.clone(),
                [22.0, 64.0, 8.0],
            );
            let mut ev_inherit = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &road_inherit_cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut ev_inherit), Flow::Cancel);
            assert!(
                host.world_blocks
                    .iter()
                    .any(|(&[x, _, z], st)| x >= 22 && (5..=11).contains(&z) && st.block() == white_conc_key),
                "/buildgen road without explicit style should inherit player's active Modern style"
            );

            // Explicit style argument on `/buildgen road ... <width> <style>`
            let sand_key = host.id_to_key["minecraft:smooth_sandstone"];
            let road_cmd = CommandInvocation::new(
                vec!["buildgen".into(), "road".into()],
                vec![
                    CommandArgument::new("x1".into(), CommandArgumentValue::Integer(4)),
                    CommandArgument::new("z1".into(), CommandArgumentValue::Integer(30)),
                    CommandArgument::new("x2".into(), CommandArgumentValue::Integer(18)),
                    CommandArgument::new("z2".into(), CommandArgumentValue::Integer(30)),
                    CommandArgument::new("width".into(), CommandArgumentValue::Integer(5)),
                    CommandArgument::new("style".into(), CommandArgumentValue::String("desert".into())),
                ],
                dim.clone(),
                [4.0, 64.0, 30.0],
            );
            let mut road_ev = RunModCommandEvent {
                source: &CommandSource::Player {
                    player,
                    permission: PermissionLevel::Owners,
                },
                command: &road_cmd,
                cancelled: false,
            };
            assert_eq!(handle_command(&mut host, &mut road_ev), Flow::Cancel);
            assert!(
                host.world_blocks
                    .iter()
                    .any(|(&[_, _, z], st)| z >= 27 && st.block() == sand_key),
                "/buildgen road with 'desert' style must place sandstone road"
            );

            // Check tab completion suggests road styles after width
            let mut sugs = Vec::new();
            let mut tab_ev = mod_api::TabCompleteEvent {
                player: 1,
                name: "buildgen",
                command: "/buildgen road 0 0 10 10 3 ",
                range: [26, 26],
                suggestions: &mut sugs,
            };
            crate::server_builder::tab_complete(&mut tab_ev);
            assert!(
                sugs.iter().any(|s| s.text == "japanese") && sugs.iter().any(|s| s.text == "modern"),
                "Tab completion should suggest road styles, got {:?}",
                sugs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>()
            );
        });
    }
}
