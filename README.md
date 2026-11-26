# Building Generator

**Building Generator** (`building_generator`) is an MIT-licensed bilingual (**English / Russian**) procedural architecture native mod for **Horizon** (Minecraft: Java Edition `26.3`, Horizon Native Mod API `0.2`).

Generate detailed multi-room buildings, landmarks, bridges, airship docks, castles, temples, and canal villages from English or Russian text prompts, noise seeds, or the built-in 72-structure catalog — complete with an interactive in-game GUI (`F8`) featuring one-click **EN / RU** language switching (`Lang: EN` / `Язык: RU`), live 3D hologram preview with slice inspection, 30-slot material block lists, two-phase construction, and multi-step undo.

---

## Features

- **Bilingual UI & Chat Commands (English & Russian):**
  - Starts in **English** by default; click the **`Lang: EN` / `Язык: RU`** button in the top header bar (or run `/buildgen lang <en|ru>`) at any time to switch the entire GUI, catalog, material slots, HUD overlays, BossBar progress, and chat feedback between English and Russian.
  - Selected language persists locally in `.horizon-mod-data/building_generator/lang.txt`.
  - Prompt parser, Preset Catalog search, and Material Block search accept both English and Russian queries simultaneously.
- **61 Architectural Themes & 72 Built-in Presets:**
  - **Houses, Taverns, Estates & Civic:** tea houses, medieval taverns, French mansards, modern villas, treehouses, libraries, apothecaries, bakeries/wineries, town halls, opera houses, museums, marble banks/vaults, magical academies.
  - **Ancient & Eastern Monuments:** Roman Pantheon (coffered dome + oculus), Parthenon, Colosseum amphitheater, Egyptian Pyramids & Ziggurats, Great Sphinx, Hanging Gardens of Babylon, Ishtar Gate, Angkor Wat, Mosque & Minaret Palace, Itsukushima Water Shrine & Torii avenue, Triumphal Arch / Obelisk.
  - **Fortresses, Castles & Guilds:** Gothic citadels, Concentric Motte & Bailey castles, fortified Barbican city gates, monasteries & cloisters, jousting arenas, military outposts, Vauban star forts, Japanese Tenshu castles, Big Ben clock towers, steampunk observatories, lighthouses, windmills, watermills.
  - **Bridges, Transport & Maritime (`gen_bridge`, `gen_civic_transport`):** 9 varied bridge archetypes across seeds (single arch, multi-arch viaduct, Gothic ribbed, Roman aqueduct, cable-stayed suspension, covered timber, Japanese Taiko moon bridge, fortified gate bridge, pontoon pier), 3-masted galleons, harbor shipyards, Victorian train stations with steam locomotives, airship docks with zeppelins, hydroelectric dams, circus big tops.
  - **Industrial, Modern & Fantasy:** glass curtain skyscrapers, saw-tooth steampunk factories, football stadiums, reinforced bunkers, rocket launchpads, offshore oil rigs, archmage towers, Hobbit holes, mushroom cottages, underground catacombs & mausoleums, dwarven halls, dragon bone sanctuaries, Stonehenge rune portals, Viking longhouses, alpine chalets, Moroccan kasbahs.
  - **Procedural Settlements (`gen_settlement`):** 5 city-planning modes up to `96×96×48`, including a Venetian **Floating Village** with water canals, stilt boardwalk piers, and stilt houses over water.
- **Interactive In-Game UI (`F8`):**
  - **1. Size & Area:** up to `96×96×48`, `-Y`/`+Y` nudging, in-world click/two-corner selection (`R` rotate facing, `PgUp`/`PgDn` height), terrain adaptation, persistent disk favorites (`favorites.txt`), `BG2:...` share codes, and 3D wireframe + translucent quad hologram with vertical slicing (`[` / `]`).
  - **2. Style & Noise + 72-Preset Catalog:** category/style/roof overrides, 5 tri-state toggles (porch, balcony, cellar, awning, garden), seed history, quick navigation (`◀` / `▶`), and a full-panel searchable **Preset Catalog** browser (`preset_picker_open`).
  - **3. Materials (30 Architectural Slots):** scrollable slot inspector, per-slot ordered block lists (`Patches` / `Scatter` / `Stripes` / `Checker` mix modes), and a dedicated searchable block picker with item icons (`Rec.` / `All` world blocks).
- **Safe Two-Phase Placement & Undo:** adapts foundations to terrain and detects water depth (`OceanFloor` vs `MotionBlockingNoLeaves`) for lakeside stilts; builds solid frames first and doors/beds/lanterns/decor second; supports `/buildgen reroll` and up to 5 levels of `/buildgen undo`.

---

## Commands

| Command | Description |
|---|---|
| `/buildgen` | Open the interactive mod menu (or press `F8`) |
| `/buildgen lang <en\|ru>` | Switch GUI & chat feedback language between English (`en`) and Russian (`ru`) |
| `/buildgen preset <name>` | Instantly build any of the 72 presets (supports Tab completion and EN/RU search) |
| `/buildgen generate <text>` | Generate a building from an EN/RU description in front of you |
| `/buildgen size <w> <d> <h> <text>` | Generate with explicit bounding dimensions (`7..96 × 7..96 × 5..48`) |
| `/buildgen reroll` | Undo the last build and regenerate at the same spot with a fresh seed |
| `/buildgen undo` | Revert your last build (up to 5 stored in history) |
| `/buildgen help` | Show concise command and keybind help |

---

## Installation

### From Release (`.hmod`)
1. Download `building_generator-0.2.1.hmod` from the release artifacts.
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
