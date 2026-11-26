# Building Generator

**Building Generator** (`building_generator`) is an MIT-licensed bilingual (**English / Russian**) procedural architecture native mod for **Horizon** (Minecraft: Java Edition `26.3`, Horizon Native Mod API `0.2`).

Generate detailed multi-room buildings, monuments, bridges, airship docks, castles, temples, and **Settlement 2.0** multi-district cities from English or Russian text prompts, world biomes, noise seeds, or the 72-structure catalog — complete with an interactive `F8` GUI featuring one-click **EN / RU** language switching (`Lang: EN` / `Язык: RU`), **Biome Auto-Style**, **0–100% Wear/Ruin & Overgrowth**, live 3D hologram preview, **Material Estimate (BOM)**, **Air Only** placement filter, 30-slot block lists, two-phase construction, **5-step Undo/Redo**, and **server OP security**.

---

## Features

- **Bilingual UI & Chat Commands (English & Russian):**
  - Starts in **English** by default; click the **`Lang: EN` / `Язык: RU`** button in the top header bar (or run `/buildgen lang <en|ru>`) at any time to switch the entire GUI, catalog, material slots, BOM estimate, HUD overlays, BossBar progress, and chat feedback between English and Russian.
  - Selected language persists locally in `.horizon-mod-data/building_generator/lang.txt`.
  - Prompt parser, Preset Catalog search, and Material Block search accept both English and Russian queries simultaneously.
- **61 Architectural Themes & 72 Built-in Presets:**
  - **Houses, Taverns, Estates & Civic:** tea houses, medieval taverns, French mansards, modern villas, treehouses, libraries, apothecaries, bakeries/wineries, town halls, opera houses, museums, marble banks/vaults, magical academies.
  - **Ancient & Eastern Monuments:** Roman Pantheon (coffered dome + oculus), Parthenon, Colosseum amphitheater, Egyptian Pyramids & Ziggurats, Great Sphinx, Hanging Gardens of Babylon, Ishtar Gate, Angkor Wat, Mosque & Minaret Palace, Itsukushima Water Shrine & Torii avenue, Triumphal Arch / Obelisk.
  - **Fortresses, Castles & Guilds:** Gothic citadels, Concentric Motte & Bailey castles, fortified Barbican city gates, monasteries & cloisters, jousting arenas, military outposts, Vauban star forts, Japanese Tenshu castles, Big Ben clock towers, steampunk observatories, lighthouses, windmills, watermills.
  - **Bridges, Transport & Maritime (`gen_bridge`, `gen_civic_transport`):** 9 varied bridge archetypes across seeds (single arch, multi-arch viaduct, Gothic ribbed, Roman aqueduct, cable-stayed suspension, covered timber, Japanese Taiko moon bridge, fortified gate bridge, pontoon pier), 3-masted galleons, harbor shipyards, Victorian train stations with steam locomotives, airship docks with zeppelins, hydroelectric dams, circus big tops.
  - **Industrial, Modern & Fantasy:** glass curtain skyscrapers, saw-tooth steampunk factories, football stadiums, reinforced bunkers, rocket launchpads, offshore oil rigs, archmage towers, Hobbit holes, mushroom cottages, underground catacombs & mausoleums, dwarven halls, dragon bone sanctuaries, Stonehenge rune portals, Viking longhouses, alpine chalets, Moroccan kasbahs.
  - **Procedural Settlements 2.0 (`gen_settlement`):** Up to `96×96×48`, smooth road slope grading, secondary ring avenues up to **8–12 town districts**, automatic **bridges over natural rivers**, fortified perimeter ramparts with **corner bastion towers & arched gatehouses**, pocket parks with biome-matched trees & lakeside piers, plus the Venetian **Floating Village** (`FloatingVillage`).
- **Biome Auto-Style & Universal Wear / Overgrowth (`F8` Tab 2):**
  - **Biome Auto-Adaptation (`Biome: ... ✓` / `Биом: ... ✓`):** Automatically reads world biome on server & live GUI (`cherry_grove`, `desert`, `badlands`, `snowy_taiga`, `pale_garden`, `mangrove_swamp`, `bamboo_jungle`, `mushroom_fields`, `nether`, `the_end`, etc.) when Style is `Auto`, choosing cohesive styles, materials, and trees.
  - **Wear / Ruin (`0–100%`) & Overgrowth (`0–100%`):** Applies masonry cracking, copper oxidation stages, shattered panes, structural skyline collapse, fallen rubble, cobwebs, biome flora/moss carpets, clinging leaves, and hanging directional vines across all 61 themes, landmarks, bridges, and cities (`BG2:...` share codes & `favorites.txt` compatible).
- **Interactive In-Game UI (`F8`):**
  - **Header Material Estimate (`Blocks: N` / `Смета: N`):** live block-count & stack breakdown (`xN (st+rem)`) across all required block types (with doors/beds counted per 2-block item) and one-click **Copy List** to clipboard.
  - **1. Size & Area:** up to `96×96×48`, `-Y`/`+Y` nudging, in-world click/two-corner selection (`R` rotate facing, `PgUp`/`PgDn` height), terrain adaptation, **Air Only** toggle (only replaces air blocks so existing builds & hillsides remain intact), persistent disk favorites (`favorites.txt`), `BG2:...` share codes (preserving `air_only`, `wear_pct`, and `overgrowth_pct`), and 3D wireframe + translucent quad hologram with vertical slicing (`[` / `]`).
  - **2. Style & Noise + 72-Preset Catalog:** category/style/roof overrides, **Biome Auto**, **Wear %**, **Moss %**, 5 tri-state toggles (porch, balcony, cellar, awning, garden), seed history, quick navigation (`◀` / `▶`), and a full-panel searchable **Preset Catalog** browser (`preset_picker_open`).
  - **3. Materials (30 Architectural Slots):** scrollable slot inspector, per-slot ordered block lists (`Patches` / `Scatter` / `Stripes` / `Checker` mix modes), and a dedicated searchable block picker with item icons (`Rec.` / `All` world blocks).
- **Safe Two-Phase Placement, Undo/Redo & Server Security:**
  - Adapts foundations to terrain, detects water depth (`OceanFloor` vs `MotionBlockingNoLeaves`), builds solid frames first and doors/beds/lanterns/decor second, and supports `/buildgen reroll` plus up to 5 steps of **`/buildgen undo` & `/buildgen redo`** (also accessible from the GUI footer).
  - Enforces **operator permission level 2+ (OP)** on all build/undo/redo/reroll commands and network payloads, sanitizes bounds/strings on the server, and rate-limits rapid GUI build requests.

---

## Commands

| Command | Description |
|---|---|
| `/buildgen` | Open the interactive mod menu (or press `F8`) |
| `/buildgen lang <en\|ru>` | Switch GUI & chat feedback language between English (`en`) and Russian (`ru`) |
| `/buildgen preset <name>` | Instantly build any of the 72 presets (OP 2+, supports Tab completion and EN/RU search) |
| `/buildgen generate <text>` | Generate a building from an EN/RU description in front of you (OP 2+) |
| `/buildgen size <w> <d> <h> <text>` | Generate with explicit bounding dimensions (`7..96 × 7..96 × 5..48`, OP 2+) |
| `/buildgen reroll` | Undo the last build and regenerate at the same spot with a fresh seed (OP 2+) |
| `/buildgen undo` | Revert your last build (up to 5 stored in history, OP 2+) |
| `/buildgen redo` | Re-apply the last undone build edit (up to 5 steps, OP 2+) |
| `/buildgen help` | Show concise command and keybind help |

---

## Installation

### From Release (`.hmod`)
1. Download `building_generator-0.4.1.hmod` from the release artifacts.
2. Place it in your Horizon instance's `horizon-mods/` (or `horizon-native-mods/`) folder, or add it via the Horizon Launcher Native Mods tab.
3. Rebuild the instance with native code trusted (`--trust-native-code`) in Horizon Launcher.

### From Source
```sh
horizon-launcher native-build . --instance horizon --trust-native-code
```

---

## Requirements & Compatibility

- **Minecraft:** Java Edition `26.3` (Horizon Rust)
- **Horizon API:** `0.2`, `side = "both"`, `capabilities = ["registries"]`
- **Pinned SDK:** `horizon = 8f83e5c85ae3b9a117f742e8d91170c5c9080a06`, `mixins = e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, plus `uses.json` compatibility table (verified on Nightly `nightly-2026-11-22-f1bb66f` / `native-f1bb66f-*`).

---

## License

MIT License — see [LICENSE](LICENSE).
