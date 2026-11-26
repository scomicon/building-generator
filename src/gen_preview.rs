//! Live 3D semi-transparent bluish holographic preview mesh generator with hidden-face culling,
//! 2D greedy quad merging, architectural shape fidelity, crease wireframes, and strict view-budget fitting.

use std::collections::BTreeMap;

use crate::generator::{GeneratedBlueprint, PlannedBlock};

/// Hard safety ceiling inside Horizon's `DEFAULT_CAPACITY = 4096` per-view world primitive budget.
pub const TARGET_MAX_QUADS: usize = 3500;
pub const TARGET_MAX_LINES: usize = 3200;

/// Cached relative quad in `[0..width, 0..height, 0..depth]` coordinates around blueprint origin.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewQuad {
    pub corners: [[f64; 3]; 4],
    pub color: u32,
    pub priority: u8,
}

/// Cached relative crease/silhouette line segment around blueprint origin.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewLine {
    pub a: [f64; 3],
    pub b: [f64; 3],
    pub color: u32,
    pub width: f32,
}

/// Complete cached 3D holographic preview with both full 1:1 mesh and 2x2x2 macro fallback.
#[derive(Clone, Debug, Default)]
pub struct HologramMesh {
    pub quads: Vec<PreviewQuad>,
    pub lines: Vec<PreviewLine>,
    pub macro_quads: Vec<PreviewQuad>,
    pub macro_lines: Vec<PreviewLine>,
}

impl HologramMesh {
    /// Selects full detail when within budget, else macro downsampled fallback capped cleanly inside limits.
    pub fn fit_budget(&self, max_quads: usize, max_lines: usize) -> (&[PreviewQuad], &[PreviewLine]) {
        let q_src = if self.quads.len() <= max_quads || self.macro_quads.is_empty() {
            &self.quads[..]
        } else {
            &self.macro_quads[..]
        };
        let q_len = q_src.len().min(max_quads);

        let l_src = if q_src.len() <= max_quads && self.lines.len() <= max_lines {
            &self.lines[..]
        } else if !self.macro_lines.is_empty() && self.macro_lines.len() <= max_lines {
            &self.macro_lines[..]
        } else {
            &self.lines[..]
        };
        let l_len = l_src.len().min(max_lines);
        (&q_src[..q_len], &l_src[..l_len])
    }
}

/// Blueprint edge highlight color along active horizontal Y-slice planes.
pub const SLICE_CAP_ARGB: u32 = 0x867D_E8FF;
/// Subtle cyan highlight for exposed structural seams (roof/wall, beam/wall, glass/wall).
pub const MATERIAL_SEAM_ARGB: u32 = 0x5867_E8F9;
/// Warm coral/amber highlight for voxels colliding with existing solid world blocks.
pub const COLLISION_CREASE_ARGB: u32 = 0xD8FF_8A4C;

/// Client-sampled world collision occupancy grid aligned with a blueprint's local bounds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewWorldMask {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub min_y: i32,
    /// `0` = air/replaceable/below-threshold ground, `1` = existing world obstacle.
    pub blocked: Vec<u8>,
}

impl PreviewWorldMask {
    pub fn new(width: i32, min_y: i32, height: i32, depth: i32) -> Self {
        let w = width.max(1);
        let h = height.max(1);
        let d = depth.max(1);
        let total = (w * h * d) as usize;
        Self {
            width: w,
            height: h,
            depth: d,
            min_y,
            blocked: vec![0; total],
        }
    }

    #[inline]
    pub fn set_blocked(&mut self, x: i32, y: i32, z: i32, val: bool) {
        let ly = y - self.min_y;
        if x < 0 || x >= self.width || ly < 0 || ly >= self.height || z < 0 || z >= self.depth {
            return;
        }
        let idx = ((ly * self.depth + z) * self.width + x) as usize;
        self.blocked[idx] = u8::from(val);
    }

    #[inline]
    pub fn is_blocked(&self, x: i32, y: i32, z: i32) -> bool {
        let ly = y - self.min_y;
        if x < 0 || x >= self.width || ly < 0 || ly >= self.height || z < 0 || z >= self.depth {
            return false;
        }
        let idx = ((ly * self.depth + z) * self.width + x) as usize;
        self.blocked[idx] != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeKind {
    Ignore,
    FullCube,
    SlabBottom,
    SlabTop,
    Stair { facing: u8, top: bool },
    ThinWall { axis_x: bool, axis_z: bool },
    Lantern,
    Glass,
    Foliage,
    Water,
}

/// Material tint family controlling semi-transparent bluish (or collision amber) hologram tone & priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoloGroup {
    Wall,
    Beam,
    Roof,
    Glass,
    Foliage,
    Water,
    Glow,
    Decor,
    Collision,
}

impl HoloGroup {
    fn id(self) -> u8 {
        match self {
            Self::Wall => 1,
            Self::Beam => 2,
            Self::Roof => 3,
            Self::Glass => 4,
            Self::Foliage => 5,
            Self::Water => 6,
            Self::Glow => 7,
            Self::Decor => 8,
            Self::Collision => 9,
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Roof | Self::Beam | Self::Glow | Self::Collision => 0,
            Self::Wall | Self::Glass | Self::Decor => 1,
            Self::Foliage | Self::Water => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct VoxelCell {
    opaque_full: bool,
    visible: bool,
    group_id: u8,
    priority: u8,
    base_rgb: [u8; 3],
    group: HoloGroup,
    shape: ShapeKind,
}

impl Default for VoxelCell {
    fn default() -> Self {
        Self {
            opaque_full: false,
            visible: false,
            group_id: 0,
            priority: 2,
            base_rgb: [0, 0, 0],
            group: HoloGroup::Wall,
            shape: ShapeKind::Ignore,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct MaskCell {
    color: u32,
    priority: u8,
}

fn prop_val<'a>(planned: &'a PlannedBlock, key: &str) -> Option<&'a str> {
    planned.props.iter().find(|&&(k, _)| k == key).map(|&(_, v)| v)
}

fn short_id(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

fn classify_shape(planned: &PlannedBlock) -> ShapeKind {
    let name = short_id(&planned.block_id);
    if matches!(
        name,
        "air" | "cave_air" | "void_air" | "barrier" | "structure_void" | "light"
    ) {
        return ShapeKind::Ignore;
    }
    if name == "water" {
        return ShapeKind::Water;
    }
    if name.ends_with("_slab") {
        return match prop_val(planned, "type") {
            Some("double") => ShapeKind::FullCube,
            Some("top") => ShapeKind::SlabTop,
            _ => ShapeKind::SlabBottom,
        };
    }
    if name.ends_with("_stairs") {
        let facing = match prop_val(planned, "facing") {
            Some("south") => 1,
            Some("west") => 2,
            Some("east") => 3,
            _ => 0, // north
        };
        let top = prop_val(planned, "half") == Some("top");
        return ShapeKind::Stair { facing, top };
    }
    if name.ends_with("lantern")
        || name.ends_with("torch")
        || name == "campfire"
        || name == "soul_campfire"
        || name.ends_with("candle")
    {
        return ShapeKind::Lantern;
    }
    if name == "glass"
        || name == "tinted_glass"
        || name == "ice"
        || name.ends_with("_stained_glass")
    {
        return ShapeKind::Glass;
    }
    if name.ends_with("_glass_pane") || name == "glass_pane" || name == "iron_bars" {
        let ew = prop_val(planned, "east") == Some("true")
            || prop_val(planned, "west") == Some("true");
        let ns = prop_val(planned, "north") == Some("true")
            || prop_val(planned, "south") == Some("true");
        return ShapeKind::ThinWall {
            axis_x: ew || !ns,
            axis_z: ns || !ew,
        };
    }
    if name.ends_with("_fence")
        || name.ends_with("_fence_gate")
        || name.ends_with("_wall")
        || name.ends_with("_door")
    {
        let (axis_x, axis_z) = if name.ends_with("_door") || name.ends_with("_fence_gate") {
            match prop_val(planned, "facing") {
                Some("east") | Some("west") => (false, true),
                _ => (true, false),
            }
        } else {
            let ew = prop_val(planned, "east") == Some("true")
                || prop_val(planned, "west") == Some("true");
            let ns = prop_val(planned, "north") == Some("true")
                || prop_val(planned, "south") == Some("true");
            (ew || !ns, ns || !ew)
        };
        return ShapeKind::ThinWall { axis_x, axis_z };
    }
    if name.ends_with("_leaves")
        || matches!(
            name,
            "vine"
                | "azalea"
                | "flowering_azalea"
                | "bamboo"
                | "sweet_berry_bush"
                | "fern"
                | "large_fern"
                | "short_grass"
                | "tall_grass"
                | "moss_carpet"
                | "poppy"
                | "dandelion"
                | "cornflower"
                | "azure_bluet"
                | "oxeye_daisy"
                | "rose_bush"
                | "peony"
                | "lilac"
                | "lily_pad"
                | "sugar_cane"
        )
    {
        return ShapeKind::Foliage;
    }
    if name.ends_with("_carpet") || name == "snow" || name.ends_with("_pressure_plate") {
        return ShapeKind::SlabBottom;
    }
    if name.ends_with("_trapdoor") {
        if prop_val(planned, "half") == Some("top") {
            return ShapeKind::SlabTop;
        }
        return ShapeKind::SlabBottom;
    }
    ShapeKind::FullCube
}

fn material_rgb_and_group(id: &str) -> ([u8; 3], HoloGroup) {
    let name = short_id(id);
    if name == "water" {
        return ([48, 120, 235], HoloGroup::Water);
    }
    if name.ends_with("lantern")
        || name.ends_with("torch")
        || name == "glowstone"
        || name == "sea_lantern"
        || name == "shroomlight"
        || name == "redstone_lamp"
        || name == "campfire"
        || name == "soul_campfire"
        || name.ends_with("candle")
        || name == "end_rod"
    {
        return ([230, 248, 255], HoloGroup::Glow);
    }
    if name.ends_with("_leaves")
        || name.starts_with("moss_")
        || name == "vine"
        || name.contains("azalea")
        || name == "bamboo"
        || name == "grass_block"
        || name.contains("fern")
        || name.contains("grass")
    {
        return ([82, 188, 130], HoloGroup::Foliage);
    }
    if name.contains("glass") || name == "ice" || name == "packed_ice" {
        return ([180, 232, 255], HoloGroup::Glass);
    }
    if name.ends_with("_log")
        || name.ends_with("_wood")
        || name.ends_with("_stem")
        || name.ends_with("_hyphae")
        || name.contains("stripped_")
        || name.ends_with("_pillar")
    {
        let rgb = if name.contains("dark_oak") || name.contains("spruce") {
            [86, 62, 46]
        } else if name.contains("birch") {
            [210, 206, 198]
        } else if name.contains("cherry") {
            [176, 124, 146]
        } else if name.contains("crimson") {
            [128, 56, 78]
        } else if name.contains("warped") {
            [56, 128, 132]
        } else {
            [120, 92, 62]
        };
        return (rgb, HoloGroup::Beam);
    }
    if name.ends_with("_stairs") || name.ends_with("_slab") {
        let rgb = palette_stone_or_wood_rgb(name);
        return (rgb, HoloGroup::Roof);
    }
    if name.ends_with("_door")
        || name.ends_with("_bed")
        || name.ends_with("_carpet")
        || name.ends_with("_banner")
        || name == "bookshelf"
        || name == "chest"
        || name == "barrel"
    {
        return ([175, 195, 240], HoloGroup::Decor);
    }
    (palette_stone_or_wood_rgb(name), HoloGroup::Wall)
}

fn palette_stone_or_wood_rgb(name: &str) -> [u8; 3] {
    if name.contains("deepslate") || name.contains("blackstone") || name.contains("obsidian") {
        [68, 76, 94]
    } else if name.contains("basalt") || name.contains("gray") || name.contains("cyan_terracotta") {
        [98, 108, 124]
    } else if name.contains("copper") && (name.contains("oxidized") || name.contains("weathered")) {
        [82, 166, 148]
    } else if name.contains("copper") {
        [182, 114, 86]
    } else if name.contains("prismarine") || name.contains("warped") {
        [78, 166, 164]
    } else if name.contains("dark_oak") {
        [92, 68, 50]
    } else if name.contains("spruce") {
        [124, 92, 62]
    } else if name.contains("mangrove") || name.contains("crimson") || name.contains("red_nether") {
        [146, 70, 78]
    } else if name.contains("acacia") || name.contains("terracotta") || name.contains("bricks") {
        [178, 114, 90]
    } else if name.contains("cherry") || name.contains("pink") {
        [220, 168, 186]
    } else if name.contains("birch")
        || name.contains("sandstone")
        || name.contains("quartz")
        || name.contains("calcite")
        || name.contains("white")
        || name.contains("end_stone")
    {
        [224, 224, 212]
    } else if name.contains("oak") || name.contains("jungle") || name.contains("bamboo") {
        [176, 142, 98]
    } else if name.contains("mud") || name.contains("dirt") {
        [128, 102, 84]
    } else {
        [152, 162, 178]
    }
}

/// Converts a block material color and face normal shade into a translucent bluish hologram ARGB color.
pub fn to_holo_argb(rgb: [u8; 3], group: HoloGroup, face_dir: usize, is_slice_cap: bool) -> u32 {
    if is_slice_cap && face_dir == 1 {
        return 0x867D_E8FF;
    }

    let shade = match face_dir {
        1 => 1.12f32, // +Y top
        0 => 0.62,    // -Y bottom
        2 | 3 => 0.80, // -Z / +Z
        _ => 0.94,    // -X / +X
    };

    if group == HoloGroup::Collision {
        let rq = (((255.0 * shade).clamp(96.0, 255.0) as u32) & !7).min(248);
        let gq = (((122.0 * shade).clamp(40.0, 180.0) as u32) & !7).min(176);
        let bq = (((68.0 * shade).clamp(24.0, 112.0) as u32) & !3).min(108);
        return (0x70u32 << 24) | (rq << 16) | (gq << 8) | bq;
    }

    let (tint_r, tint_g, tint_b, mix_tint, base_alpha): (f32, f32, f32, f32, u8) = match group {
        HoloGroup::Wall => (68.0, 174.0, 255.0, 0.74, 0x56),
        HoloGroup::Beam => (42.0, 116.0, 218.0, 0.76, 0x64),
        HoloGroup::Roof => (44.0, 156.0, 252.0, 0.76, 0x62),
        HoloGroup::Glass => (182.0, 238.0, 255.0, 0.85, 0x36),
        HoloGroup::Foliage => (66.0, 224.0, 216.0, 0.78, 0x46),
        HoloGroup::Water => (34.0, 118.0, 246.0, 0.84, 0x52),
        HoloGroup::Glow => (218.0, 248.0, 255.0, 0.72, 0x88),
        HoloGroup::Decor => (134.0, 198.0, 255.0, 0.76, 0x5C),
        HoloGroup::Collision => (255.0, 118.0, 68.0, 0.85, 0x70),
    };

    let mr = f32::from(rgb[0]);
    let mg = f32::from(rgb[1]);
    let mb = f32::from(rgb[2]);
    let lum = (0.299 * mr + 0.587 * mg + 0.114 * mb) / 255.0;
    let lum_gain = 0.72 + lum * 0.34;

    let mut r = (mr * (1.0 - mix_tint) + tint_r * mix_tint) * lum_gain * shade;
    let g = (mg * (1.0 - mix_tint) + tint_g * mix_tint) * lum_gain * shade;
    let mut b = (mb * (1.0 - mix_tint) + tint_b * mix_tint) * lum_gain * shade;

    // Preserve a crisp cool bluish channel dominance.
    b = b.max(g * 1.04 + 12.0);
    r = r.min(g * 0.96);

    // Quantize slightly (~8-step RGB buckets) to maximize greedy face merging without muddying details.
    let rq = ((r.clamp(12.0, 255.0) as u32) & !7).min(248);
    let gq = ((g.clamp(24.0, 255.0) as u32) & !7).min(248);
    let bq = ((b.clamp(48.0, 255.0) as u32) & !3).min(252);

    (u32::from(base_alpha) << 24) | (rq << 16) | (gq << 8) | bq
}

fn voxel_cell_from_block(planned: &PlannedBlock) -> VoxelCell {
    let shape = classify_shape(planned);
    if shape == ShapeKind::Ignore {
        return VoxelCell::default();
    }
    let (base_rgb, base_group) = material_rgb_and_group(&planned.block_id);
    let group = match shape {
        ShapeKind::Glass => HoloGroup::Glass,
        ShapeKind::Foliage => HoloGroup::Foliage,
        ShapeKind::Water => HoloGroup::Water,
        ShapeKind::Lantern => HoloGroup::Glow,
        _ => base_group,
    };
    let opaque_full = shape == ShapeKind::FullCube && group != HoloGroup::Glass && group != HoloGroup::Water;
    VoxelCell {
        opaque_full,
        visible: true,
        group_id: group.id(),
        priority: group.priority(),
        base_rgb,
        group,
        shape,
    }
}

/// Builds a full 3D translucent bluish holographic preview mesh (plus a 2x2x2 macro fallback),
/// including negative-Y basements, hillside terraces, and bridge pier footings.
pub fn build_hologram_mesh(bp: &GeneratedBlueprint, clip_max_y: Option<i32>) -> HologramMesh {
    build_hologram_mesh_with_world(bp, clip_max_y, None, false, false)
}

/// Builds a 3D translucent holographic preview aware of existing world obstacles:
/// omits obstructed blocks in `air_only` mode, or tints conflicting voxels coral-orange when
/// `highlight_collisions` is active.
pub fn build_hologram_mesh_with_world(
    bp: &GeneratedBlueprint,
    clip_max_y: Option<i32>,
    world_mask: Option<&PreviewWorldMask>,
    air_only: bool,
    highlight_collisions: bool,
) -> HologramMesh {
    let w = i32::from(bp.width).max(1);
    let d = i32::from(bp.depth).max(1);
    let mut min_y = 0i32;
    let mut max_y = i32::from(bp.height).saturating_sub(1);
    for &[_, y, _] in bp.blocks.keys() {
        let eff_y = clip_max_y.map_or(y, |cy| y.min(cy));
        if eff_y < min_y {
            min_y = eff_y;
        }
        if eff_y > max_y {
            max_y = eff_y;
        }
    }
    min_y = min_y.clamp(-24, 0);
    let y_shift = -min_y;
    let h = (max_y - min_y + 1).clamp(1, 96);
    let y_cap = clip_max_y.map(|cy| (cy + y_shift).clamp(0, h - 1));

    let total = (w * h * d) as usize;
    let mut grid = vec![VoxelCell::default(); total];
    let mut extras: Vec<([i32; 3], VoxelCell)> = Vec::new();

    for (&[x, y, z], planned) in &bp.blocks {
        let sy = y + y_shift;
        if let Some(cy) = y_cap
            && sy > cy
        {
            continue;
        }
        if x < 0 || x >= w || sy < 0 || sy >= h || z < 0 || z >= d {
            continue;
        }
        let world_blocked = world_mask.is_some_and(|m| m.is_blocked(x, y, z));
        if air_only && world_blocked {
            continue;
        }
        let mut cell = voxel_cell_from_block(planned);
        if !cell.visible {
            continue;
        }
        if !air_only && highlight_collisions && world_blocked {
            cell.group = HoloGroup::Collision;
            cell.group_id = HoloGroup::Collision.id();
            cell.priority = HoloGroup::Collision.priority();
            cell.base_rgb = [255, 118, 68];
        }
        let idx = ((sy * d + z) * w + x) as usize;
        grid[idx] = cell;
        if cell.shape != ShapeKind::FullCube {
            extras.push(([x, sy, z], cell));
        }
    }

    let (mut quads, mut lines) = build_detail_mesh(&grid, &extras, w, h, d, y_cap);

    let (mut macro_quads, mut macro_lines) = if quads.len() > TARGET_MAX_QUADS || lines.len() > TARGET_MAX_LINES {
        build_macro_mesh(&grid, w, h, d, y_cap)
    } else {
        (Vec::new(), Vec::new())
    };

    if min_y != 0 {
        let dy = min_y as f64;
        for q in quads.iter_mut().chain(macro_quads.iter_mut()) {
            for c in &mut q.corners {
                c[1] += dy;
            }
        }
        for l in lines.iter_mut().chain(macro_lines.iter_mut()) {
            l.a[1] += dy;
            l.b[1] += dy;
        }
    }

    HologramMesh {
        quads,
        lines,
        macro_quads,
        macro_lines,
    }
}

#[inline]
fn at(grid: &[VoxelCell], w: i32, h: i32, d: i32, x: i32, y: i32, z: i32) -> VoxelCell {
    if x < 0 || x >= w || y < 0 || y >= h || z < 0 || z >= d {
        return VoxelCell::default();
    }
    grid[((y * d + z) * w + x) as usize]
}

fn should_cull_face(cur: VoxelCell, nb: VoxelCell) -> bool {
    if !nb.visible {
        return false;
    }
    if nb.opaque_full {
        return true;
    }
    // Avoid double translucent alpha seams inside glass windows, hedges, and water bodies.
    cur.shape == nb.shape
        && cur.group == nb.group
        && matches!(cur.group, HoloGroup::Glass | HoloGroup::Water | HoloGroup::Foliage)
}

fn emit_face_quad(
    out: &mut Vec<PreviewQuad>,
    dir: usize,
    s: i32,
    u0: i32,
    v0: i32,
    uw: i32,
    vh: i32,
    color: u32,
    priority: u8,
) {
    let eps = 0.004;
    let corners = match dir {
        0 => {
            // -Y (u = x, v = z)
            let py = s as f64 - eps;
            let (x0, x1, z0, z1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[x0, py, z0], [x1, py, z0], [x1, py, z1], [x0, py, z1]]
        }
        1 => {
            // +Y (u = x, v = z)
            let py = (s + 1) as f64 + eps;
            let (x0, x1, z0, z1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[x0, py, z0], [x0, py, z1], [x1, py, z1], [x1, py, z0]]
        }
        2 => {
            // -Z (u = x, v = y)
            let pz = s as f64 - eps;
            let (x0, x1, y0, y1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[x1, y1, pz], [x1, y0, pz], [x0, y0, pz], [x0, y1, pz]]
        }
        3 => {
            // +Z (u = x, v = y)
            let pz = (s + 1) as f64 + eps;
            let (x0, x1, y0, y1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[x0, y1, pz], [x0, y0, pz], [x1, y0, pz], [x1, y1, pz]]
        }
        4 => {
            // -X (u = z, v = y)
            let px = s as f64 - eps;
            let (z0, z1, y0, y1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[px, y1, z1], [px, y0, z1], [px, y0, z0], [px, y1, z0]]
        }
        _ => {
            // +X (u = z, v = y)
            let px = (s + 1) as f64 + eps;
            let (z0, z1, y0, y1) = (u0 as f64, (u0 + uw) as f64, v0 as f64, (v0 + vh) as f64);
            [[px, y1, z0], [px, y0, z0], [px, y0, z1], [px, y1, z1]]
        }
    };
    out.push(PreviewQuad {
        corners,
        color,
        priority,
    });
}

fn push_box_quads(
    out: &mut Vec<PreviewQuad>,
    min: [f64; 3],
    max: [f64; 3],
    rgb: [u8; 3],
    group: HoloGroup,
    is_slice_top: bool,
    priority: u8,
) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let faces = [
        (0usize, [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        (1, [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]),
        (2, [[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]]),
        (3, [[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]]),
        (4, [[x0, y1, z1], [x0, y0, z1], [x0, y0, z0], [x0, y1, z0]]),
        (5, [[x1, y1, z0], [x1, y0, z0], [x1, y0, z1], [x1, y1, z1]]),
    ];
    for (dir, corners) in faces {
        out.push(PreviewQuad {
            corners,
            color: to_holo_argb(rgb, group, dir, is_slice_top),
            priority,
        });
    }
}

fn greedy_full_boxes(
    grid: &[VoxelCell],
    w: i32,
    h: i32,
    d: i32,
    y_cap: Option<i32>,
    out: &mut Vec<PreviewQuad>,
) {
    let dirs: [(i32, i32, i32); 6] = [
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];

    for (dir, &(dx, dy, dz)) in dirs.iter().enumerate() {
        let (s_len, u_len, v_len) = match dir {
            0 | 1 => (h, w, d),
            2 | 3 => (d, w, h),
            _ => (w, d, h),
        };
        let mut mask: Vec<Option<MaskCell>> = vec![None; (u_len * v_len) as usize];

        for s in 0..s_len {
            mask.fill(None);
            for v in 0..v_len {
                for u in 0..u_len {
                    let (x, y, z) = match dir {
                        0 | 1 => (u, s, v),
                        2 | 3 => (u, v, s),
                        _ => (s, v, u),
                    };
                    let cur = at(grid, w, h, d, x, y, z);
                    if !cur.visible
                        || !matches!(
                            cur.shape,
                            ShapeKind::FullCube
                                | ShapeKind::Glass
                                | ShapeKind::Foliage
                                | ShapeKind::Water
                        )
                    {
                        continue;
                    }
                    let nb = at(grid, w, h, d, x + dx, y + dy, z + dz);
                    if should_cull_face(cur, nb) {
                        continue;
                    }
                    let slice_top = y_cap == Some(y) && dir == 1;
                    let color = to_holo_argb(cur.base_rgb, cur.group, dir, slice_top);
                    mask[(v * u_len + u) as usize] = Some(MaskCell {
                        color,
                        priority: cur.priority,
                    });
                }
            }

            for v in 0..v_len {
                let mut u = 0;
                while u < u_len {
                    let idx = (v * u_len + u) as usize;
                    let Some(m) = mask[idx] else {
                        u += 1;
                        continue;
                    };
                    let mut uw = 1;
                    while u + uw < u_len && mask[(v * u_len + u + uw) as usize] == Some(m) {
                        uw += 1;
                    }
                    let mut vh = 1;
                    'grow_v: while v + vh < v_len {
                        for k in 0..uw {
                            if mask[((v + vh) * u_len + u + k) as usize] != Some(m) {
                                break 'grow_v;
                            }
                        }
                        vh += 1;
                    }
                    for vv in v..(v + vh) {
                        for uu in u..(u + uw) {
                            mask[(vv * u_len + uu) as usize] = None;
                        }
                    }
                    emit_face_quad(out, dir, s, u, v, uw, vh, m.color, m.priority);
                    u += uw;
                }
            }
        }
    }
}

fn emit_subblock_shapes(
    grid: &[VoxelCell],
    extras: &[([i32; 3], VoxelCell)],
    w: i32,
    h: i32,
    d: i32,
    y_cap: Option<i32>,
    out: &mut Vec<PreviewQuad>,
) {
    // Merge adjacent horizontal slab, stair, and railing runs to slash quad counts on roofs & bridges.
    let mut slab_bottom_map: BTreeMap<(i32, i32), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();
    let mut slab_top_map: BTreeMap<(i32, i32), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();
    let mut stair_ns_map: BTreeMap<(i32, i32, u8, bool), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();
    let mut stair_ew_map: BTreeMap<(i32, i32, u8, bool), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();
    let mut thin_x_map: BTreeMap<(i32, i32), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();
    let mut thin_z_map: BTreeMap<(i32, i32), Vec<(i32, [u8; 3], HoloGroup, u8)>> = BTreeMap::new();

    for &([x, y, z], cell) in extras {
        let fx = x as f64;
        let fy = y as f64;
        let fz = z as f64;
        let slice_top = y_cap == Some(y);
        match cell.shape {
            ShapeKind::SlabBottom => {
                slab_bottom_map
                    .entry((y, z))
                    .or_default()
                    .push((x, cell.base_rgb, cell.group, cell.priority));
            }
            ShapeKind::SlabTop => {
                slab_top_map
                    .entry((y, z))
                    .or_default()
                    .push((x, cell.base_rgb, cell.group, cell.priority));
            }
            ShapeKind::Stair { facing, top } => {
                if facing <= 1 {
                    stair_ns_map
                        .entry((y, z, facing, top))
                        .or_default()
                        .push((x, cell.base_rgb, cell.group, cell.priority));
                } else {
                    stair_ew_map
                        .entry((y, x, facing, top))
                        .or_default()
                        .push((z, cell.base_rgb, cell.group, cell.priority));
                }
            }
            ShapeKind::ThinWall { axis_x, axis_z } => {
                if axis_x && !axis_z {
                    thin_x_map
                        .entry((y, z))
                        .or_default()
                        .push((x, cell.base_rgb, cell.group, cell.priority));
                } else if axis_z && !axis_x {
                    thin_z_map
                        .entry((y, x))
                        .or_default()
                        .push((z, cell.base_rgb, cell.group, cell.priority));
                } else {
                    let (x0, x1) = if axis_x { (fx, fx + 1.0) } else { (fx + 0.36, fx + 0.64) };
                    let (z0, z1) = if axis_z { (fz, fz + 1.0) } else { (fz + 0.36, fz + 0.64) };
                    push_box_quads(
                        out,
                        [x0, fy + 0.004, z0],
                        [x1, fy + 0.996, z1],
                        cell.base_rgb,
                        cell.group,
                        slice_top,
                        cell.priority,
                    );
                }
            }
            ShapeKind::Lantern => {
                // Skip glow box only if completely entombed inside opaque solids on all 6 sides.
                let buried = at(grid, w, h, d, x - 1, y, z).opaque_full
                    && at(grid, w, h, d, x + 1, y, z).opaque_full
                    && at(grid, w, h, d, x, y - 1, z).opaque_full
                    && at(grid, w, h, d, x, y + 1, z).opaque_full
                    && at(grid, w, h, d, x, y, z - 1).opaque_full
                    && at(grid, w, h, d, x, y, z + 1).opaque_full;
                if !buried {
                    push_box_quads(
                        out,
                        [fx + 0.30, fy + 0.14, fz + 0.30],
                        [fx + 0.70, fy + 0.78, fz + 0.70],
                        cell.base_rgb,
                        cell.group,
                        slice_top,
                        cell.priority,
                    );
                }
            }
            _ => {}
        }
    }

    for ((y, z), mut runs) in slab_bottom_map {
        runs.sort_by_key(|r| r.0);
        emit_merged_slab_row(out, runs, y, z, 0.0, 0.5, y_cap == Some(y));
    }
    for ((y, z), mut runs) in slab_top_map {
        runs.sort_by_key(|r| r.0);
        emit_merged_slab_row(out, runs, y, z, 0.5, 1.0, y_cap == Some(y));
    }
    for ((y, z, facing, top), mut runs) in stair_ns_map {
        runs.sort_by_key(|r| r.0);
        let fy = y as f64;
        let fz = z as f64;
        let (base_y0, base_y1, step_y0, step_y1) = if top {
            (fy + 0.5, fy + 1.0, fy, fy + 0.5)
        } else {
            (fy, fy + 0.5, fy + 0.5, fy + 1.0)
        };
        let (sz0, sz1) = if facing == 0 { (fz + 0.002, fz + 0.50) } else { (fz + 0.50, fz + 0.998) };
        let mut i = 0;
        while i < runs.len() {
            let (x0, rgb, grp, prio) = runs[i];
            let mut x1 = x0 + 1;
            while i + 1 < runs.len() && runs[i + 1].0 == x1 && runs[i + 1].1 == rgb && runs[i + 1].2 == grp {
                x1 += 1;
                i += 1;
            }
            let slice_top = y_cap == Some(y);
            push_box_quads(
                out,
                [x0 as f64 + 0.002, base_y0 + 0.002, fz + 0.002],
                [x1 as f64 - 0.002, base_y1 - 0.002, fz + 0.998],
                rgb,
                grp,
                slice_top && !top,
                prio,
            );
            push_box_quads(
                out,
                [x0 as f64 + 0.002, step_y0, sz0],
                [x1 as f64 - 0.002, step_y1, sz1],
                rgb,
                grp,
                slice_top,
                prio,
            );
            i += 1;
        }
    }
    for ((y, x, facing, top), mut runs) in stair_ew_map {
        runs.sort_by_key(|r| r.0);
        let fy = y as f64;
        let fx = x as f64;
        let (base_y0, base_y1, step_y0, step_y1) = if top {
            (fy + 0.5, fy + 1.0, fy, fy + 0.5)
        } else {
            (fy, fy + 0.5, fy + 0.5, fy + 1.0)
        };
        let (sx0, sx1) = if facing == 2 { (fx + 0.002, fx + 0.50) } else { (fx + 0.50, fx + 0.998) };
        let mut i = 0;
        while i < runs.len() {
            let (z0, rgb, grp, prio) = runs[i];
            let mut z1 = z0 + 1;
            while i + 1 < runs.len() && runs[i + 1].0 == z1 && runs[i + 1].1 == rgb && runs[i + 1].2 == grp {
                z1 += 1;
                i += 1;
            }
            let slice_top = y_cap == Some(y);
            push_box_quads(
                out,
                [fx + 0.002, base_y0 + 0.002, z0 as f64 + 0.002],
                [fx + 0.998, base_y1 - 0.002, z1 as f64 - 0.002],
                rgb,
                grp,
                slice_top && !top,
                prio,
            );
            push_box_quads(
                out,
                [sx0, step_y0, z0 as f64 + 0.002],
                [sx1, step_y1, z1 as f64 - 0.002],
                rgb,
                grp,
                slice_top,
                prio,
            );
            i += 1;
        }
    }
    for ((y, z), mut runs) in thin_x_map {
        runs.sort_by_key(|r| r.0);
        let fy = y as f64;
        let fz = z as f64;
        let mut i = 0;
        while i < runs.len() {
            let (x0, rgb, grp, prio) = runs[i];
            let mut x1 = x0 + 1;
            while i + 1 < runs.len() && runs[i + 1].0 == x1 && runs[i + 1].1 == rgb && runs[i + 1].2 == grp {
                x1 += 1;
                i += 1;
            }
            push_box_quads(
                out,
                [x0 as f64, fy + 0.004, fz + 0.36],
                [x1 as f64, fy + 0.996, fz + 0.64],
                rgb,
                grp,
                y_cap == Some(y),
                prio,
            );
            i += 1;
        }
    }
    for ((y, x), mut runs) in thin_z_map {
        runs.sort_by_key(|r| r.0);
        let fy = y as f64;
        let fx = x as f64;
        let mut i = 0;
        while i < runs.len() {
            let (z0, rgb, grp, prio) = runs[i];
            let mut z1 = z0 + 1;
            while i + 1 < runs.len() && runs[i + 1].0 == z1 && runs[i + 1].1 == rgb && runs[i + 1].2 == grp {
                z1 += 1;
                i += 1;
            }
            push_box_quads(
                out,
                [fx + 0.36, fy + 0.004, z0 as f64],
                [fx + 0.64, fy + 0.996, z1 as f64],
                rgb,
                grp,
                y_cap == Some(y),
                prio,
            );
            i += 1;
        }
    }
}

fn emit_merged_slab_row(
    out: &mut Vec<PreviewQuad>,
    runs: Vec<(i32, [u8; 3], HoloGroup, u8)>,
    y: i32,
    z: i32,
    dy0: f64,
    dy1: f64,
    slice_top: bool,
) {
    let mut i = 0;
    while i < runs.len() {
        let (x0, rgb, grp, prio) = runs[i];
        let mut x1 = x0 + 1;
        while i + 1 < runs.len() && runs[i + 1].0 == x1 && runs[i + 1].1 == rgb && runs[i + 1].2 == grp {
            x1 += 1;
            i += 1;
        }
        push_box_quads(
            out,
            [x0 as f64 + 0.002, y as f64 + dy0, z as f64 + 0.002],
            [x1 as f64 - 0.002, y as f64 + dy1, z as f64 + 0.998],
            rgb,
            grp,
            slice_top,
            prio,
        );
        i += 1;
    }
}

/// Extracts geometric crease and structural material-transition lines along X, Y, Z grid edges.
fn extract_crease_lines(
    grid: &[VoxelCell],
    w: i32,
    h: i32,
    d: i32,
    y_cap: Option<i32>,
    out: &mut Vec<PreviewLine>,
) {
    let slice_line_col = 0xCC7D_F9FFu32;
    let edge_col = 0x8838_BDF8u32;

    let sample_tag = |x: i32, y: i32, z: i32| -> u8 {
        let c = at(grid, w, h, d, x, y, z);
        if c.visible { c.group_id } else { 0 }
    };

    // 1. Edges along X (z in 0..=d, y in 0..=h)
    for y in 0..=h {
        for z in 0..=d {
            let mut x = 0;
            while x < w {
                let tags = [
                    sample_tag(x, y - 1, z - 1),
                    sample_tag(x, y, z - 1),
                    sample_tag(x, y - 1, z),
                    sample_tag(x, y, z),
                ];
                let col_kind = edge_line_color(tags, y_cap.is_some_and(|cy| y == cy + 1), edge_col, slice_line_col);
                if let Some((col, width)) = col_kind {
                    let x0 = x;
                    while x < w {
                        let t2 = [
                            sample_tag(x, y - 1, z - 1),
                            sample_tag(x, y, z - 1),
                            sample_tag(x, y - 1, z),
                            sample_tag(x, y, z),
                        ];
                        if t2 != tags {
                            break;
                        }
                        x += 1;
                    }
                    out.push(PreviewLine {
                        a: [x0 as f64, y as f64, z as f64],
                        b: [x as f64, y as f64, z as f64],
                        color: col,
                        width,
                    });
                } else {
                    x += 1;
                }
            }
        }
    }

    // 2. Edges along Z (x in 0..=w, y in 0..=h)
    for y in 0..=h {
        for x in 0..=w {
            let mut z = 0;
            while z < d {
                let tags = [
                    sample_tag(x - 1, y - 1, z),
                    sample_tag(x, y - 1, z),
                    sample_tag(x - 1, y, z),
                    sample_tag(x, y, z),
                ];
                let col_kind = edge_line_color(tags, y_cap.is_some_and(|cy| y == cy + 1), edge_col, slice_line_col);
                if let Some((col, width)) = col_kind {
                    let z0 = z;
                    while z < d {
                        let t2 = [
                            sample_tag(x - 1, y - 1, z),
                            sample_tag(x, y - 1, z),
                            sample_tag(x - 1, y, z),
                            sample_tag(x, y, z),
                        ];
                        if t2 != tags {
                            break;
                        }
                        z += 1;
                    }
                    out.push(PreviewLine {
                        a: [x as f64, y as f64, z0 as f64],
                        b: [x as f64, y as f64, z as f64],
                        color: col,
                        width,
                    });
                } else {
                    z += 1;
                }
            }
        }
    }

    // 3. Vertical edges along Y (x in 0..=w, z in 0..=d)
    for z in 0..=d {
        for x in 0..=w {
            let mut y = 0;
            while y < h {
                let tags = [
                    sample_tag(x - 1, y, z - 1),
                    sample_tag(x, y, z - 1),
                    sample_tag(x - 1, y, z),
                    sample_tag(x, y, z),
                ];
                let col_kind = edge_line_color(tags, false, edge_col, slice_line_col);
                if let Some((col, width)) = col_kind {
                    let y0 = y;
                    while y < h {
                        let t2 = [
                            sample_tag(x - 1, y, z - 1),
                            sample_tag(x, y, z - 1),
                            sample_tag(x - 1, y, z),
                            sample_tag(x, y, z),
                        ];
                        if t2 != tags {
                            break;
                        }
                        y += 1;
                    }
                    out.push(PreviewLine {
                        a: [x as f64, y0 as f64, z as f64],
                        b: [x as f64, y as f64, z as f64],
                        color: col,
                        width,
                    });
                } else {
                    y += 1;
                }
            }
        }
    }
}

/// Returns `Some((argb, width))` for convex/concave corners, collision highlights, exposed material seams, and slice boundaries.
fn edge_line_color(tags: [u8; 4], is_slice_plane: bool, edge_col: u32, slice_col: u32) -> Option<(u32, f32)> {
    let occ = tags.iter().filter(|&&t| t != 0).count();
    let has_collision = tags.contains(&HoloGroup::Collision.id());
    if occ == 0 || occ == 4 {
        return None;
    }
    if is_slice_plane && (tags[0] != 0 || tags[2] != 0) && (tags[1] == 0 || tags[3] == 0) {
        return Some((slice_col, 2.0));
    }
    match occ {
        1 | 3 => {
            if has_collision {
                Some((COLLISION_CREASE_ARGB, 1.65))
            } else {
                Some((edge_col, 1.35))
            }
        }
        2 => {
            // Side-by-side flat surface vs diagonal touch ([0,3] or [1,2]).
            let diag = (tags[0] != 0 && tags[3] != 0) || (tags[1] != 0 && tags[2] != 0);
            if diag {
                if has_collision {
                    Some((COLLISION_CREASE_ARGB, 1.65))
                } else {
                    Some((edge_col, 1.35))
                }
            } else {
                // Exposed material transition on a flat outer surface (e.g., Wall vs Roof, Beam vs Wall, Collision vs Wall).
                let first_non_zero = tags.iter().copied().find(|&t| t != 0).unwrap_or(0);
                let distinct_groups = tags.iter().any(|&t| t != 0 && t != first_non_zero);
                if distinct_groups {
                    if has_collision {
                        Some((COLLISION_CREASE_ARGB, 1.5))
                    } else {
                        Some((MATERIAL_SEAM_ARGB, 1.05))
                    }
                } else {
                    None
                }
            }
        }
        _ => None,
    }
}

fn build_detail_mesh(
    grid: &[VoxelCell],
    extras: &[([i32; 3], VoxelCell)],
    w: i32,
    h: i32,
    d: i32,
    y_cap: Option<i32>,
) -> (Vec<PreviewQuad>, Vec<PreviewLine>) {
    let mut quads = Vec::new();
    greedy_full_boxes(grid, w, h, d, y_cap, &mut quads);
    emit_subblock_shapes(grid, extras, w, h, d, y_cap, &mut quads);
    quads.sort_by_key(|q| q.priority);

    let mut lines = Vec::new();
    extract_crease_lines(grid, w, h, d, y_cap, &mut lines);
    (quads, lines)
}

/// Builds a 2x2x2 downsampled macro voxel mesh so colossal 96x96 settlements fit safely within 4096 quads.
fn build_macro_mesh(
    grid: &[VoxelCell],
    w: i32,
    h: i32,
    d: i32,
    y_cap: Option<i32>,
) -> (Vec<PreviewQuad>, Vec<PreviewLine>) {
    let mw = (w + 1) / 2;
    let mh = (h + 1) / 2;
    let md = (d + 1) / 2;
    let mut mgrid = vec![VoxelCell::default(); (mw * mh * md) as usize];

    for my in 0..mh {
        for mz in 0..md {
            for mx in 0..mw {
                let mut best: Option<VoxelCell> = None;
                for oy in 0..2 {
                    let sy = my * 2 + oy;
                    if sy >= h {
                        continue;
                    }
                    for oz in 0..2 {
                        let sz = mz * 2 + oz;
                        if sz >= d {
                            continue;
                        }
                        for ox in 0..2 {
                            let sx = mx * 2 + ox;
                            if sx >= w {
                                continue;
                            }
                            let c = at(grid, w, h, d, sx, sy, sz);
                            if !c.visible {
                                continue;
                            }
                            if best.is_none_or(|b| c.priority < b.priority || (c.opaque_full && !b.opaque_full)) {
                                best = Some(VoxelCell {
                                    opaque_full: c.group != HoloGroup::Glass && c.group != HoloGroup::Water,
                                    shape: match c.group {
                                        HoloGroup::Glass => ShapeKind::Glass,
                                        HoloGroup::Water => ShapeKind::Water,
                                        HoloGroup::Foliage => ShapeKind::Foliage,
                                        _ => ShapeKind::FullCube,
                                    },
                                    ..c
                                });
                            }
                        }
                    }
                }
                if let Some(cell) = best {
                    mgrid[((my * md + mz) * mw + mx) as usize] = cell;
                }
            }
        }
    }

    let macro_cap = y_cap.map(|cy| cy / 2);
    let mut quads = Vec::new();
    greedy_full_boxes(&mgrid, mw, mh, md, macro_cap, &mut quads);
    for q in &mut quads {
        for c in &mut q.corners {
            c[0] *= 2.0;
            c[1] *= 2.0;
            c[2] *= 2.0;
        }
    }
    quads.sort_by_key(|q| q.priority);

    let mut lines = Vec::new();
    extract_crease_lines(&mgrid, mw, mh, md, macro_cap, &mut lines);
    for l in &mut lines {
        for k in 0..3 {
            l.a[k] *= 2.0;
            l.b[k] *= 2.0;
        }
    }

    (quads, lines)
}
