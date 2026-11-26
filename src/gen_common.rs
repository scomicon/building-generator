use std::collections::{BTreeMap, VecDeque};

use crate::model::{BuildStyle, CardinalFacing, MaterialPalette};

/// Canonical block placement emitted by the generator (pure data, game-independent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedBlock {
    pub block_id: String,
    pub props: Vec<(&'static str, &'static str)>,
}

impl PlannedBlock {
    pub fn simple(block_id: impl Into<String>) -> Self {
        Self {
            block_id: block_id.into(),
            props: Vec::new(),
        }
    }

    pub fn with_props(block_id: impl Into<String>, props: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            block_id: block_id.into(),
            props,
        }
    }
}

/// Complete generated voxel blueprint ready for unit inspection or `edit_blocks` placement.
#[derive(Clone, Debug)]
pub struct GeneratedBlueprint {
    pub spec: crate::parser::BlueprintSpec,
    pub origin: [i32; 3],
    pub width: u16,
    pub depth: u16,
    pub height: u16,
    /// Map from local `[lx, ly, lz]` (relative to `origin`) to block state spec.
    pub blocks: BTreeMap<[i32; 3], PlannedBlock>,
    pub door_pos: Option<[i32; 3]>,
}

impl GeneratedBlueprint {
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn non_air_count(&self) -> usize {
        self.blocks
            .values()
            .filter(|b| b.block_id != "minecraft:air")
            .count()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalGrid {
    pub w: i32,
    pub d: i32,
    pub facing: CardinalFacing,
    pub orig_w: i32,
    pub orig_d: i32,
}

impl LocalGrid {
    pub fn new(width: u16, depth: u16, facing: CardinalFacing) -> Self {
        let orig_w = i32::from(width.clamp(7, 96));
        let orig_d = i32::from(depth.clamp(7, 96));
        let (w, d) = match facing {
            CardinalFacing::North | CardinalFacing::South => (orig_w, orig_d),
            CardinalFacing::East | CardinalFacing::West => (orig_d, orig_w),
        };
        Self {
            w,
            d,
            facing,
            orig_w,
            orig_d,
        }
    }

    pub fn to_local_xz(&self, u: i32, v: i32) -> [i32; 2] {
        match self.facing {
            CardinalFacing::South => [u, v],
            CardinalFacing::North => [self.w - 1 - u, self.d - 1 - v],
            CardinalFacing::East => [v, self.w - 1 - u],
            CardinalFacing::West => [self.d - 1 - v, u],
        }
    }

    pub fn in_bounds(&self, u: i32, v: i32) -> bool {
        u >= 0 && u < self.w && v >= 0 && v < self.d
    }

    pub fn idx(&self, u: i32, v: i32) -> usize {
        (v * self.w + u) as usize
    }

    pub fn dir_front(&self) -> &'static str {
        self.facing.as_mc()
    }

    pub fn dir_back(&self) -> &'static str {
        match self.facing {
            CardinalFacing::North => "south",
            CardinalFacing::South => "north",
            CardinalFacing::East => "west",
            CardinalFacing::West => "east",
        }
    }

    pub fn dir_right(&self) -> &'static str {
        match self.facing {
            CardinalFacing::South => "east",
            CardinalFacing::North => "west",
            CardinalFacing::East => "north",
            CardinalFacing::West => "south",
        }
    }

    pub fn dir_left(&self) -> &'static str {
        match self.facing {
            CardinalFacing::South => "west",
            CardinalFacing::North => "east",
            CardinalFacing::East => "south",
            CardinalFacing::West => "north",
        }
    }

    pub fn uv_to_mc_dir(&self, du: i32, dv: i32) -> &'static str {
        if dv > 0 {
            self.dir_front()
        } else if dv < 0 {
            self.dir_back()
        } else if du > 0 {
            self.dir_right()
        } else {
            self.dir_left()
        }
    }

    pub fn axis_u(&self) -> &'static str {
        match self.facing {
            CardinalFacing::North | CardinalFacing::South => "x",
            CardinalFacing::East | CardinalFacing::West => "z",
        }
    }

    pub fn axis_v(&self) -> &'static str {
        match self.facing {
            CardinalFacing::North | CardinalFacing::South => "z",
            CardinalFacing::East | CardinalFacing::West => "x",
        }
    }
}

pub(crate) fn is_log_or_pillar(block_id: &str) -> bool {
    block_id.ends_with("_log")
        || block_id.ends_with("_wood")
        || block_id.ends_with("_stem")
        || block_id.ends_with("_hyphae")
        || block_id.ends_with("_pillar")
        || block_id.ends_with("basalt")
        || matches!(
            block_id,
            "minecraft:bamboo_block" | "minecraft:stripped_bamboo_block"
        )
}

pub(crate) fn is_lantern(block_id: &str) -> bool {
    block_id.ends_with("lantern") && block_id != "minecraft:sea_lantern"
}

pub(crate) fn make_block_with_axis(block_id: &str, axis: &'static str) -> PlannedBlock {
    if is_log_or_pillar(block_id) {
        PlannedBlock::with_props(block_id.to_owned(), vec![("axis", axis)])
    } else {
        PlannedBlock::simple(block_id.to_owned())
    }
}

pub(crate) fn make_frame_block_at(
    palette: &MaterialPalette,
    axis: &'static str,
    seed: u64,
    u: i32,
    y: i32,
    v: i32,
) -> PlannedBlock {
    make_block_with_axis(&palette.pick(3, seed, u, y, v), axis)
}

pub(crate) fn make_beam_block_at(
    palette: &MaterialPalette,
    axis: &'static str,
    seed: u64,
    u: i32,
    y: i32,
    v: i32,
) -> PlannedBlock {
    make_block_with_axis(&palette.pick(14, seed, u, y, v), axis)
}

pub(crate) fn make_pillar_block_at(
    palette: &MaterialPalette,
    axis: &'static str,
    seed: u64,
    u: i32,
    y: i32,
    v: i32,
) -> PlannedBlock {
    make_block_with_axis(&palette.pick(15, seed, u, y, v), axis)
}

pub(crate) fn make_wall_sconce(block_id: &str, facing: &'static str) -> PlannedBlock {
    match block_id {
        "minecraft:torch" => PlannedBlock::with_props("minecraft:wall_torch", vec![("facing", facing)]),
        "minecraft:soul_torch" => PlannedBlock::with_props("minecraft:soul_wall_torch", vec![("facing", facing)]),
        "minecraft:copper_torch" => PlannedBlock::with_props("minecraft:copper_wall_torch", vec![("facing", facing)]),
        id if is_lantern(id) => PlannedBlock::with_props(id.to_owned(), vec![("hanging", "false")]),
        "minecraft:end_rod" => PlannedBlock::with_props("minecraft:end_rod", vec![("facing", facing)]),
        id => PlannedBlock::simple(id.to_owned()),
    }
}

pub(crate) fn resolve_slab_id(block_id: &str) -> String {
    MaterialPalette::resolve_slab_id(block_id)
}

pub(crate) fn make_stair(block_id: &str, facing: &'static str, half: &'static str) -> PlannedBlock {
    PlannedBlock::with_props(
        block_id.to_owned(),
        vec![("facing", facing), ("half", half), ("shape", "straight")],
    )
}

pub(crate) fn make_slab(block_id: &str, slab_type: &'static str) -> PlannedBlock {
    PlannedBlock::with_props(resolve_slab_id(block_id), vec![("type", slab_type)])
}

pub(crate) fn make_trapdoor(
    block_id: &str,
    facing: &'static str,
    half: &'static str,
    open: bool,
) -> PlannedBlock {
    PlannedBlock::with_props(
        block_id.to_owned(),
        vec![
            ("facing", facing),
            ("half", half),
            ("open", if open { "true" } else { "false" }),
        ],
    )
}

pub(crate) fn make_leaves(block_id: &str) -> PlannedBlock {
    if block_id.ends_with("_leaves") {
        PlannedBlock::with_props(
            block_id.to_owned(),
            vec![("persistent", "true"), ("distance", "1")],
        )
    } else {
        PlannedBlock::simple(block_id.to_owned())
    }
}

pub(crate) fn set_uv(
    blocks: &mut BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
    block: PlannedBlock,
) {
    if !grid.in_bounds(u, v) {
        return;
    }
    let [lx, lz] = grid.to_local_xz(u, v);
    blocks.insert([lx, y, lz], block);
}

pub(crate) fn get_uv<'a>(
    blocks: &'a BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
) -> Option<&'a PlannedBlock> {
    if !grid.in_bounds(u, v) {
        return None;
    }
    let [lx, lz] = grid.to_local_xz(u, v);
    blocks.get(&[lx, y, lz])
}

pub(crate) fn is_solid_or_occupied(
    blocks: &BTreeMap<[i32; 3], PlannedBlock>,
    grid: LocalGrid,
    u: i32,
    y: i32,
    v: i32,
) -> bool {
    get_uv(blocks, grid, u, y, v).is_some_and(|b| b.block_id != "minecraft:air")
}

pub(crate) fn terrain_rel_at(
    terrain_offsets: Option<&[i32]>,
    grid: LocalGrid,
    u: i32,
    v: i32,
) -> i32 {
    if !grid.in_bounds(u, v) {
        return 0;
    }
    let [lx, lz] = grid.to_local_xz(u, v);
    terrain_offsets
        .and_then(|off| off.get((lz * grid.orig_w + lx) as usize).copied())
        .unwrap_or(0)
}

pub(crate) fn water_depth_at(
    water_depths: Option<&[i32]>,
    grid: LocalGrid,
    u: i32,
    v: i32,
) -> i32 {
    if !grid.in_bounds(u, v) {
        return 0;
    }
    let [lx, lz] = grid.to_local_xz(u, v);
    water_depths
        .and_then(|wd| wd.get((lz * grid.orig_w + lx) as usize).copied())
        .unwrap_or(0)
        .max(0)
}

pub(crate) fn fill_rect(
    fp: &mut [bool],
    grid: LocalGrid,
    u0: i32,
    v0: i32,
    u1: i32,
    v1: i32,
    val: bool,
) {
    for v in v0.max(0)..=v1.min(grid.d - 1) {
        for u in u0.max(0)..=u1.min(grid.w - 1) {
            fp[grid.idx(u, v)] = val;
        }
    }
}

pub(crate) fn compute_tight_bounds(grid: LocalGrid, fp: &[bool]) -> [i32; 4] {
    let mut b_u0 = grid.w;
    let mut b_v0 = grid.d;
    let mut b_u1 = 0;
    let mut b_v1 = 0;
    for v in 0..grid.d {
        for u in 0..grid.w {
            if fp[grid.idx(u, v)] {
                b_u0 = b_u0.min(u);
                b_v0 = b_v0.min(v);
                b_u1 = b_u1.max(u);
                b_v1 = b_v1.max(v);
            }
        }
    }
    [b_u0, b_v0, b_u1, b_v1]
}

pub(crate) fn is_fp(grid: LocalGrid, fp: &[bool], u: i32, v: i32) -> bool {
    grid.in_bounds(u, v) && fp[grid.idx(u, v)]
}

pub(crate) fn compute_edge_distance(grid: LocalGrid, fp: &[bool]) -> Vec<i32> {
    let mut dist = vec![i32::MAX; fp.len()];
    let mut queue = VecDeque::new();
    for v in 0..grid.d {
        for u in 0..grid.w {
            let i = grid.idx(u, v);
            if !fp[i] {
                dist[i] = 0;
                queue.push_back((u, v));
            } else if u == 0 || v == 0 || u == grid.w - 1 || v == grid.d - 1 {
                dist[i] = 1;
                queue.push_back((u, v));
            }
        }
    }
    while let Some((u, v)) = queue.pop_front() {
        let d0 = dist[grid.idx(u, v)];
        for (du, dv) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nu = u + du;
            let nv = v + dv;
            if grid.in_bounds(nu, nv) {
                let ni = grid.idx(nu, nv);
                if fp[ni] && dist[ni] > d0 + 1 {
                    dist[ni] = d0 + 1;
                    queue.push_back((nu, nv));
                }
            }
        }
    }
    dist
}

pub(crate) fn style_carpet(style: BuildStyle) -> &'static str {
    match style {
        BuildStyle::Gothic | BuildStyle::Medieval | BuildStyle::Classical => "minecraft:red_carpet",
        BuildStyle::Japanese | BuildStyle::Modern | BuildStyle::Desert => "minecraft:white_carpet",
        _ => "minecraft:moss_carpet",
    }
}

pub(crate) fn style_pot(style: BuildStyle) -> &'static str {
    match style {
        BuildStyle::Japanese => "minecraft:potted_cherry_sapling",
        BuildStyle::Cottage => "minecraft:potted_red_tulip",
        BuildStyle::Nordic | BuildStyle::Gothic => "minecraft:potted_fern",
        BuildStyle::Modern => "minecraft:potted_azalea_bush",
        _ => "minecraft:potted_oak_sapling",
    }
}

fn dir_vec(dir: &str) -> (i32, i32) {
    match dir {
        "north" => (0, -1),
        "east" => (1, 0),
        "south" => (0, 1),
        _ => (-1, 0),
    }
}

fn dir_cw(dir: &str) -> &'static str {
    match dir {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        _ => "north",
    }
}

fn dir_ccw(dir: &str) -> &'static str {
    match dir {
        "north" => "west",
        "west" => "south",
        "south" => "east",
        _ => "north",
    }
}

fn is_full_connect_solid(b: &PlannedBlock) -> bool {
    let id = b.block_id.as_str();
    !matches!(
        id,
        "minecraft:air"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:ladder"
            | "minecraft:torch"
            | "minecraft:lantern"
            | "minecraft:soul_lantern"
            | "minecraft:copper_lantern"
            | "minecraft:flower_pot"
            | "minecraft:cauldron"
            | "minecraft:water_cauldron"
            | "minecraft:lava_cauldron"
            | "minecraft:anvil"
            | "minecraft:lectern"
            | "minecraft:enchanting_table"
            | "minecraft:decorated_pot"
            | "minecraft:iron_chain"
            | "minecraft:lightning_rod"
            | "minecraft:bell"
            | "minecraft:composter"
            | "minecraft:campfire"
            | "minecraft:soul_campfire"
            | "minecraft:cake"
            | "minecraft:sea_pickle"
            | "minecraft:jukebox"
            | "minecraft:ender_chest"
            | "minecraft:glass"
            | "minecraft:copper_grate"
            | "minecraft:iron_bars"
    ) && !id.ends_with("_door")
        && !id.ends_with("_bed")
        && !id.ends_with("_carpet")
        && !id.ends_with("_trapdoor")
        && !id.ends_with("_slab")
        && !id.ends_with("_stairs")
        && !id.ends_with("_fence")
        && !id.ends_with("_fence_gate")
        && !id.ends_with("_wall")
        && !id.ends_with("_shelf")
        && !id.ends_with("glass_pane")
        && !id.ends_with("_leaves")
        && !id.ends_with("_pressure_plate")
        && !id.starts_with("minecraft:potted_")
}

fn prop_val(b: &PlannedBlock, key: &str) -> Option<&'static str> {
    b.props.iter().find(|&&(k, _)| k == key).map(|&(_, v)| v)
}

pub(crate) fn prune_map_unsupported(blocks: &mut BTreeMap<[i32; 3], PlannedBlock>) {
    crate::generator::prune_unsupported_floating_blocks(blocks);
}

/// Computes exact Minecraft 26.3 mitered stair shapes (`outer_*`/`inner_*`) and
/// `north`/`east`/`south`/`west`/`up` connections for glass panes, fences, iron bars, and walls,
/// because chunk-level `BlockEdit` writes raw states without shape-update neighbor ticks.
pub(crate) fn resolve_connections_and_shapes(blocks: &mut BTreeMap<[i32; 3], PlannedBlock>) {
    let snapshot = blocks.clone();
    let cardinal = [
        ("north", 0i32, -1i32),
        ("east", 1, 0),
        ("south", 0, 1),
        ("west", -1, 0),
    ];

    for (&[x, y, z], blk) in blocks.iter_mut() {
        let id = blk.block_id.as_str();
        if id.ends_with("_stairs") {
            let facing = prop_val(blk, "facing").unwrap_or("north");
            let half = prop_val(blk, "half").unwrap_or("bottom");
            let (fx, fz) = dir_vec(facing);
            let cw = dir_cw(facing);
            let ccw = dir_ccw(facing);
            let (cwx, cwz) = dir_vec(cw);
            let (ccwx, ccwz) = dir_vec(ccw);

            let stair_at = |px: i32, pz: i32| -> Option<&str> {
                let nb = snapshot.get(&[px, y, pz])?;
                if nb.block_id.ends_with("_stairs") && prop_val(nb, "half") == Some(half) {
                    prop_val(nb, "facing")
                } else {
                    None
                }
            };

            let mut shape = "straight";
            if let Some(ff) = stair_at(x + fx, z + fz) {
                if ff == ccw && stair_at(x + cwx, z + cwz) != Some(facing) {
                    shape = "outer_left";
                } else if ff == cw && stair_at(x + ccwx, z + ccwz) != Some(facing) {
                    shape = "outer_right";
                }
            } else if let Some(bf) = stair_at(x - fx, z - fz) {
                if bf == ccw && stair_at(x + ccwx, z + ccwz) != Some(facing) {
                    shape = "inner_left";
                } else if bf == cw && stair_at(x + cwx, z + cwz) != Some(facing) {
                    shape = "inner_right";
                }
            }

            blk.props = vec![
                ("facing", facing),
                ("half", half),
                ("shape", shape),
                ("waterlogged", "false"),
            ];
        } else if id.ends_with("glass_pane") || id == "minecraft:iron_bars" {
            let mut props = Vec::with_capacity(5);
            for (dname, dx, dz) in cardinal {
                let conn = snapshot.get(&[x + dx, y, z + dz]).is_some_and(|nb| {
                    let nid = nb.block_id.as_str();
                    nid.ends_with("glass_pane")
                        || nid == "minecraft:iron_bars"
                        || nid == "minecraft:glass"
                        || nid == "minecraft:copper_grate"
                        || is_full_connect_solid(nb)
                });
                props.push((dname, if conn { "true" } else { "false" }));
            }
            props.push(("waterlogged", "false"));
            blk.props = props;
        } else if id.ends_with("_fence") {
            let is_nether = id == "minecraft:nether_brick_fence";
            let mut props = Vec::with_capacity(5);
            for (dname, dx, dz) in cardinal {
                let conn = snapshot.get(&[x + dx, y, z + dz]).is_some_and(|nb| {
                    let nid = nb.block_id.as_str();
                    (nid.ends_with("_fence") && (nid == "minecraft:nether_brick_fence") == is_nether)
                        || nid.ends_with("_fence_gate")
                        || is_full_connect_solid(nb)
                });
                props.push((dname, if conn { "true" } else { "false" }));
            }
            props.push(("waterlogged", "false"));
            blk.props = props;
        } else if id.ends_with("_wall") {
            let above = snapshot.get(&[x, y + 1, z]);
            let above_solid = above.is_some_and(|a| is_full_connect_solid(a) || a.block_id.ends_with("_wall"));
            let mut props = Vec::with_capacity(6);
            let mut arms = ["none"; 4];
            for (idx, &(dname, dx, dz)) in cardinal.iter().enumerate() {
                let conn = snapshot.get(&[x + dx, y, z + dz]).is_some_and(|nb| {
                    let nid = nb.block_id.as_str();
                    nid.ends_with("_wall") || nid.ends_with("_fence_gate") || is_full_connect_solid(nb)
                });
                let val = if !conn {
                    "none"
                } else if above_solid
                    && snapshot
                        .get(&[x + dx, y + 1, z + dz])
                        .is_some_and(|na| is_full_connect_solid(na) || na.block_id.ends_with("_wall"))
                {
                    "tall"
                } else {
                    "low"
                };
                arms[idx] = val;
                props.push((dname, val));
            }
            let straight_ns = arms[0] != "none" && arms[2] != "none" && arms[1] == "none" && arms[3] == "none";
            let straight_ew = arms[1] != "none" && arms[3] != "none" && arms[0] == "none" && arms[2] == "none";
            let post_up = above.is_some_and(|a| a.block_id != "minecraft:air") || (!straight_ns && !straight_ew);
            props.push(("up", if post_up { "true" } else { "false" }));
            props.push(("waterlogged", "false"));
            blk.props = props;
        }
    }
}
