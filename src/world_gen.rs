use crate::voxel::{VoxelGrid, VoxelType, GRID_X, GRID_Y, GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y};
use macroquad::prelude::{IVec3, Vec3};

/// Levels are authored in a compact 96×96-voxel design space (the original map size). After a
/// level is built, [`expand_layout`] spreads it across the full grid: columns occupied by tall
/// structures keep a 1:1 scale, while the open ground between them is stretched. Buildings keep
/// their exact shape and structural detail but end up much farther apart.
pub const DESIGN_SIZE: i32 = 96;

/// Fraction of the tallest structure's per-slice voxel count that marks a design column as
/// "structure" (kept 1:1) rather than open ground (stretched).
const KEEP_SLICE_FRACTION: f32 = 0.12;
/// Absolute floor for the keep threshold, so sparse props (lamps, cables) never count as structure.
const KEEP_SLICE_MIN: u32 = 60;
/// Structure columns are dilated by this many design voxels so foundation skirts stay 1:1.
const KEEP_DILATION: i32 = 2;
/// Only voxels at or above this height count toward structure detection (ignores paving/footings).
const KEEP_MIN_Y: i32 = 8;

/// Mapping from design-space columns to grid columns produced by [`expand_layout`].
#[derive(Clone, Debug)]
pub struct LevelLayout {
    /// For each design column `x`, the half-open range `[start, end)` of grid columns it fills.
    pub x_spans: Vec<(i32, i32)>,
    /// For each design column `z`, the half-open range `[start, end)` of grid columns it fills.
    pub z_spans: Vec<(i32, i32)>,
}

impl LevelLayout {
    /// Identity layout centered in the grid (no stretching).
    pub fn centered() -> Self {
        let ox = (GRID_X as i32 - DESIGN_SIZE) / 2;
        let oz = (GRID_Z as i32 - DESIGN_SIZE) / 2;
        Self {
            x_spans: (0..DESIGN_SIZE).map(|x| (x + ox, x + ox + 1)).collect(),
            z_spans: (0..DESIGN_SIZE).map(|z| (z + oz, z + oz + 1)).collect(),
        }
    }

    fn span(spans: &[(i32, i32)], d: i32) -> (i32, i32) {
        let n = spans.len() as i32;
        if d < 0 {
            let s = spans[0].0 + d;
            (s, s + 1)
        } else if d >= n {
            let s = spans[(n - 1) as usize].1 + (d - n);
            (s, s + 1)
        } else {
            spans[d as usize]
        }
    }

    /// Grid column range `[start, end)` covered by design column `x`.
    pub fn span_x(&self, x: i32) -> (i32, i32) {
        Self::span(&self.x_spans, x)
    }

    /// Grid column range `[start, end)` covered by design column `z`.
    pub fn span_z(&self, z: i32) -> (i32, i32) {
        Self::span(&self.z_spans, z)
    }

    /// Representative grid column (center of the span) for design column `x`.
    pub fn map_x(&self, x: i32) -> i32 {
        let (s, e) = self.span_x(x);
        (s + e - 1) / 2
    }

    /// Representative grid column (center of the span) for design column `z`.
    pub fn map_z(&self, z: i32) -> i32 {
        let (s, e) = self.span_z(z);
        (s + e - 1) / 2
    }

    /// Maps a design-space voxel coordinate to the grid (y is unchanged).
    pub fn map_voxel(&self, x: i32, y: i32, z: i32) -> IVec3 {
        IVec3::new(self.map_x(x), y, self.map_z(z))
    }

    /// Inclusive grid range covering the inclusive design range `a..=b` along x.
    pub fn map_range_x(&self, a: i32, b: i32) -> (i32, i32) {
        (self.span_x(a.min(b)).0, self.span_x(a.max(b)).1 - 1)
    }

    /// Inclusive grid range covering the inclusive design range `a..=b` along z.
    pub fn map_range_z(&self, a: i32, b: i32) -> (i32, i32) {
        (self.span_z(a.min(b)).0, self.span_z(a.max(b)).1 - 1)
    }

    /// World-space position of a design-space voxel center.
    pub fn design_to_world(&self, grid: &VoxelGrid, x: i32, y: i32, z: i32) -> Vec3 {
        grid.voxel_to_world(self.map_voxel(x, y, z))
    }

    /// Player spawn: the original design spawn (south edge, facing the district center).
    pub fn spawn_point(&self, grid: &VoxelGrid) -> Vec3 {
        let p = self.design_to_world(grid, DESIGN_SIZE / 2, 0, 89);
        Vec3::new(p.x, 0.5, p.z)
    }
}

/// Default geology of an untouched column (what [`VoxelGrid::clear`] produces).
#[inline]
fn default_voxel_at(y: i32) -> VoxelType {
    if y <= -41 || y == 0 {
        VoxelType::Bedrock
    } else if y < 0 {
        VoxelType::Basalt
    } else {
        VoxelType::Air
    }
}

/// Computes per-axis spans: structure columns map 1:1, open-ground columns share the rest evenly.
fn axis_spans(keep: &[bool], target: i32) -> Vec<(i32, i32)> {
    let n = keep.len() as i32;
    let k = keep.iter().filter(|&&b| b).count() as i32;
    let g = n - k;
    if g == 0 || target <= n {
        let off = (target - n).max(0) / 2;
        return (0..n).map(|i| (i + off, i + off + 1)).collect();
    }
    let stretch = (target - k) as f64 / g as f64;
    let mut pos = 0.0f64;
    let mut spans = Vec::with_capacity(n as usize);
    for &is_keep in keep {
        let start = pos.round() as i32;
        pos += if is_keep { 1.0 } else { stretch };
        let end = (pos.round() as i32).max(start + 1).min(target);
        spans.push((start, end));
    }
    spans
}

/// Spreads a level authored in the design space (`0..DESIGN_SIZE` in x/z) across the whole grid.
/// Every design voxel is copied into its stretched footprint, so structures in open ground
/// (roads, decks, pylons, bridges) stay continuous, while tall structures keep their exact shape.
pub fn expand_layout(grid: &mut VoxelGrid) -> LevelLayout {
    let d = DESIGN_SIZE as usize;
    let ny = GRID_Y;

    // 1. Snapshot the design region (x, y, z order with z contiguous) and measure structure mass.
    let mut snap = vec![VoxelType::Air; d * ny * d];
    let mut count_x = vec![0u32; d];
    let mut count_z = vec![0u32; d];
    for x in 0..d {
        for yi in 0..ny {
            let y = yi as i32 + MIN_VOXEL_Y;
            for z in 0..d {
                let vt = grid.voxels[VoxelGrid::idx(x, y, z)];
                snap[(x * ny + yi) * d + z] = vt;
                if y >= KEEP_MIN_Y && vt.is_solid() {
                    count_x[x] += 1;
                    count_z[z] += 1;
                }
            }
        }
    }

    // 2. Classify columns: dense slices are structures (kept 1:1), the rest is open ground.
    let classify = |counts: &[u32]| -> Vec<bool> {
        let max_c = counts.iter().copied().max().unwrap_or(0);
        let threshold = ((max_c as f32 * KEEP_SLICE_FRACTION) as u32).max(KEEP_SLICE_MIN);
        let raw: Vec<bool> = counts.iter().map(|&c| c >= threshold).collect();
        (0..d as i32)
            .map(|i| {
                ((i - KEEP_DILATION).max(0)..=(i + KEEP_DILATION).min(d as i32 - 1))
                    .any(|j| raw[j as usize])
            })
            .collect()
    };
    let keep_x = classify(&count_x);
    let keep_z = classify(&count_z);
    let layout = LevelLayout {
        x_spans: axis_spans(&keep_x, GRID_X as i32),
        z_spans: axis_spans(&keep_z, GRID_Z as i32),
    };

    // 3. Rebuild the grid from the snapshot into stretched footprints.
    grid.clear();
    for x in 0..d {
        let (x0, x1) = layout.x_spans[x];
        for z in 0..d {
            let (z0, z1) = layout.z_spans[z];
            for yi in 0..ny {
                let vt = snap[(x * ny + yi) * d + z];
                let y = yi as i32 + MIN_VOXEL_Y;
                if vt == default_voxel_at(y) {
                    continue;
                }
                for gx in x0..x1 {
                    for gz in z0..z1 {
                        grid.set(gx, y, gz, vt);
                    }
                }
            }
        }
    }
    layout
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LevelPreset {
    ApexSkyscraper = 0,
    SuspensionSkybridge = 1,
    MilitaryFortress = 2,
    DominoMonoliths = 3,
    GothicCitadel = 4,
    NeoTokyoSpire = 5,
    ChemicalRefinery = 6,
    OrbitalLaunchGantry = 7,
    QuantumFusionReactor = 8,
}

impl LevelPreset {
    pub const ALL: [LevelPreset; 9] = [
        LevelPreset::ApexSkyscraper,
        LevelPreset::SuspensionSkybridge,
        LevelPreset::MilitaryFortress,
        LevelPreset::DominoMonoliths,
        LevelPreset::GothicCitadel,
        LevelPreset::NeoTokyoSpire,
        LevelPreset::ChemicalRefinery,
        LevelPreset::OrbitalLaunchGantry,
        LevelPreset::QuantumFusionReactor,
    ];

    pub fn all() -> &'static [LevelPreset] {
        &Self::ALL
    }

    pub fn next(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|p| p == self).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }

    pub fn prev(&self) -> Self {
        let all = Self::all();
        let idx = all.iter().position(|p| p == self).unwrap_or(0);
        if idx == 0 {
            all[all.len() - 1]
        } else {
            all[idx - 1]
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            LevelPreset::ApexSkyscraper => "LEVEL 1: CORPORATE MEGA-BLOCK",
            LevelPreset::SuspensionSkybridge => "LEVEL 2: INDUSTRIAL FUSION DISTRICT",
            LevelPreset::MilitaryFortress => "LEVEL 3: HIGH-TECH AEROPOLIS",
            LevelPreset::DominoMonoliths => "LEVEL 4: FORTRESS CITADEL",
            LevelPreset::GothicCitadel => "LEVEL 5: DENSE NEON METROPOLIS",
            LevelPreset::NeoTokyoSpire => "LEVEL 6: NEO-TOKYO CYBER-SPIRE",
            LevelPreset::ChemicalRefinery => "LEVEL 7: CHEMICAL REFINERY & FUEL DEPOT",
            LevelPreset::OrbitalLaunchGantry => "LEVEL 8: ORBITAL LAUNCH GANTRY",
            LevelPreset::QuantumFusionReactor => "LEVEL 9: QUANTUM FUSION TOKAMAK",
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            LevelPreset::ApexSkyscraper => "Sprawling 6-skyscraper corporate mega-block (85m-105m tall) anchored by a 210-voxel central headquarters spire, financial monoliths, and tech arcologies linked by aerial skybridges.",
            LevelPreset::SuspensionSkybridge => "Colossal 5-tower industrial fusion energy grid with twin 108m suspension towers, 92m trade center, high-level coolant skyways, and rapid transit.",
            LevelPreset::MilitaryFortress => "Futuristic 7-skyscraper aeropolis complex featuring a 110m deep-space communications array, twin orbital transit pylons, and four modular R&D towers with subterranean propellant silos.",
            LevelPreset::DominoMonoliths => "Impenetrable 8-tower defensive fortress redoubt featuring a 105m High Citadel Spire, three 90m command keeps, four 80m defensive bastions, and heavy curtain wall ramparts.",
            LevelPreset::GothicCitadel => "Dense asymmetric cyberpunk metropolis of 8 skyscrapers (80m-105m tall) with cantilevered neon slabs, stepped facade terraces, and overhead entablature skyways calibrated for domino collapse.",
            LevelPreset::NeoTokyoSpire => "Twin 74-voxel cyber-spire towers connected by dual skybridges, glass curtain walls, and composite exoskeletons.",
            LevelPreset::ChemicalRefinery => "Petrochemical refinery complex with twin cooling towers, catalytic cracking stacks, pressurized tanks, and volatile fuel conduit pipelines.",
            LevelPreset::OrbitalLaunchGantry => "Heavy orbital launch complex with an 84-voxel umbilical service gantry tower, swing arms, and a multi-stage liquid propellant rocket.",
            LevelPreset::QuantumFusionReactor => "High-energy magnetic confinement tokamak facility with superconducting cryo coils, high-voltage capacitor banks, and stabilized plasma core.",
        }
    }

    pub fn dimensions(&self) -> &'static str {
        match self {
            LevelPreset::ApexSkyscraper => "DISTRICT: 6 SKYSCRAPERS | HEIGHT: 105.0m [210 VOXELS] | FOOTPRINT: 192x192 VOXELS",
            LevelPreset::SuspensionSkybridge => "DISTRICT: 5 MEGA-TOWERS | HEIGHT: 108.0m [216 VOXELS] | FOOTPRINT: 192x192 VOXELS",
            LevelPreset::MilitaryFortress => "DISTRICT: 7 SKYSCRAPERS | HEIGHT: 110.0m [220 VOXELS] | FOOTPRINT: 192x192 VOXELS",
            LevelPreset::DominoMonoliths => "DISTRICT: 8 FORTRESS TOWERS | HEIGHT: 105.0m [210 VOXELS] | FOOTPRINT: 192x192 VOXELS",
            LevelPreset::GothicCitadel => "DISTRICT: 8 NEON TOWERS | HEIGHT: 105.0m [210 VOXELS] | FOOTPRINT: 192x192 VOXELS",
            LevelPreset::NeoTokyoSpire => "TWIN TOWERS: 19x19 VOXELS | HEIGHT: 39.5m [79 VOXELS]",
            LevelPreset::ChemicalRefinery => "REFINERY FOOTPRINT: 54x54 VOXELS | HEIGHT: 34.0m [68 VOXELS]",
            LevelPreset::OrbitalLaunchGantry => "GANTRY FOOTPRINT: 48x48 VOXELS | HEIGHT: 44.0m [88 VOXELS]",
            LevelPreset::QuantumFusionReactor => "TOKAMAK FACILITY: 52x52 VOXELS | HEIGHT: 31.0m [62 VOXELS]",
        }
    }
}

/// Builds `preset` and spreads it across the full grid. Returns the design→grid layout mapping
/// (use it to locate authored features, e.g. `layout.map_voxel(48, 15, 70)`).
pub fn generate_level(grid: &mut VoxelGrid, preset: LevelPreset) -> LevelLayout {
    grid.clear();

    match preset {
        LevelPreset::ApexSkyscraper => build_orbital_monolith(grid),
        LevelPreset::SuspensionSkybridge => build_suspension_hyperbridge(grid),
        LevelPreset::MilitaryFortress => build_cyber_bunker(grid),
        LevelPreset::DominoMonoliths => build_quantum_core_facility(grid),
        LevelPreset::GothicCitadel => build_domino_colosseum(grid),
        LevelPreset::NeoTokyoSpire => build_neotokyo_spire(grid),
        LevelPreset::ChemicalRefinery => build_chemical_refinery(grid),
        LevelPreset::OrbitalLaunchGantry => build_orbital_launch_gantry(grid),
        LevelPreset::QuantumFusionReactor => build_quantum_fusion_tokamak(grid),
    }
    let layout = expand_layout(grid);

    // Recalculate voxel count, rebar count, and mark all chunks dirty
    let mut count = 0;
    let mut rebar_count = 0;
    let mut max_y = 0;
    for x in 0..GRID_X {
        for y in 1..MAX_VOXEL_Y {
            for z in 0..GRID_Z {
                let vt = grid.voxels[VoxelGrid::idx(x, y, z)];
                if vt.is_solid() {
                    count += 1;
                    if y > max_y {
                        max_y = y;
                    }
                    if vt == VoxelType::SteelRebar || vt == VoxelType::CompositeRebar {
                        rebar_count += 1;
                    }
                }
            }
        }
    }

    grid.total_initial_voxels = count;
    grid.current_voxels = count;
    grid.solid_count = count;
    grid.initial_rebar_count = rebar_count;
    grid.current_rebar_count = rebar_count;
    grid.max_solid_y = max_y;
    grid.has_dirty_meshes = true;
    grid.structural_dirty = false;
    // Construction touched every column; drop those bounds so the first impact gets a localized check.
    grid.dirty_min = None;
    grid.dirty_max = None;
    grid.severed_rebar_event = 0;
    grid.severed_rebar_positions.clear();
    grid.spall_events.clear();

    grid.dirty_chunks.clear();
    grid.has_dirty_meshes = true;
    for mesh in &mut grid.meshes {
        mesh.dirty = true;
    }
    grid.update_meshes();
    layout
}

// ============================================================================
// TRANSIT & INTERCONNECTION PROCEDURAL GENERATORS (G4-M2 FEATURES 7 & 8)
// ============================================================================

/// Builds an enclosed aerial suspension skybridge connecting two points at height `y_deck`.
/// Height `y_deck` is in voxel coordinates ($y \in [110, 170]$, i.e. $55\text{m}–85\text{m}$).
pub fn build_suspension_skybridge(
    grid: &mut VoxelGrid,
    p1: (i32, i32),
    p2: (i32, i32),
    y_deck: i32,
    half_width: i32,
    is_armored: bool,
) {
    let dx = p2.0 - p1.0;
    let dz = p2.1 - p1.1;
    let steps = dx.abs().max(dz.abs()).max(1);

    let len = ((dx * dx + dz * dz) as f32).sqrt().max(1.0);
    let nx = (-dz as f32 / len).round() as i32;
    let nz = (dx as f32 / len).round() as i32;

    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = (p1.0 as f32 + dx as f32 * t).round() as i32;
        let cz = (p1.1 as f32 + dz as f32 * t).round() as i32;

        let is_rib = s % 3 == 0;
        let is_anchor = s <= 1 || s >= steps - 1;

        for w in -half_width..=half_width {
            let vx = cx + nx * w;
            let vz = cz + nz * w;
            let is_edge = w == -half_width || w == half_width;

            // 1. Bottom Structural Truss & Tension Chords (y_deck - 1)
            if is_edge || is_rib {
                grid.set(vx, y_deck - 1, vz, VoxelType::CompositeRebar);
            } else {
                grid.set(vx, y_deck - 1, vz, VoxelType::SteelRebar);
            }

            // 2. Walkway Floor Slab (y_deck)
            if is_edge {
                grid.set(vx, y_deck, vz, VoxelType::CompositeRebar);
            } else {
                grid.set(vx, y_deck, vz, VoxelType::Concrete);
            }

            // 3. Facade Enclosure (y_deck + 1 ..= y_deck + 3)
            for dy in 1..=3 {
                let fy = y_deck + dy;
                if is_edge {
                    if is_anchor || is_armored {
                        grid.set(vx, fy, vz, VoxelType::HeavyArmor);
                    } else if is_rib {
                        grid.set(vx, fy, vz, VoxelType::CompositeRebar);
                    } else {
                        grid.set(vx, fy, vz, VoxelType::Glass);
                    }
                } else {
                    grid.set(vx, fy, vz, VoxelType::Air);
                }
            }

            // 4. Roof Slab & Suspension Cable Anchor (y_deck + 4)
            if is_edge || w == 0 {
                grid.set(vx, y_deck + 4, vz, VoxelType::CompositeRebar);
            } else {
                grid.set(vx, y_deck + 4, vz, VoxelType::Concrete);
            }

            // 5. Overhead Suspension Stay Cables (continuous vertical hanger ribs)
            if is_edge && s > 0 && s < steps {
                let dist_from_end = s.min(steps - s) as f32;
                let cable_h = (dist_from_end * 0.75).round() as i32;
                if cable_h > 0 && cable_h <= 16 {
                    for hy in (y_deck + 4)..=(y_deck + 4 + cable_h) {
                        grid.set(vx, hy, vz, VoxelType::SteelRebar);
                    }
                }
            }
        }
    }
}

/// Builds an elevated dual-beam monorail transit guideway at height `y_beam` ($y \in [40, 50]$, i.e. $20\text{m}–25\text{m}$).
pub fn build_elevated_monorail(
    grid: &mut VoxelGrid,
    p1: (i32, i32),
    p2: (i32, i32),
    y_beam: i32,
    pylon_interval: usize,
    with_train: bool,
) {
    let dx = p2.0 - p1.0;
    let dz = p2.1 - p1.1;
    let steps = dx.abs().max(dz.abs()).max(1);

    let len = ((dx * dx + dz * dz) as f32).sqrt().max(1.0);
    let nx = (-dz as f32 / len).round() as i32;
    let nz = (dx as f32 / len).round() as i32;

    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = (p1.0 as f32 + dx as f32 * t).round() as i32;
        let cz = (p1.1 as f32 + dz as f32 * t).round() as i32;

        let is_pylon = s == 0 || s == steps || (s % (pylon_interval as i32) == 0);

        // 1. Concrete Support T-Pylon (rooted from bedrock y=1 to y_beam)
        if is_pylon {
            for y in 1..y_beam {
                for px in -1..=0 {
                    for pz in -1..=0 {
                        grid.set(cx + px, y, cz + pz, VoxelType::Concrete);
                    }
                }
                grid.set(cx, y, cz, VoxelType::CompositeRebar);
            }
            // Cantilevered T-arm crosshead at y_beam - 1
            for w in -2..=2 {
                let px = cx + nx * w;
                let pz = cz + nz * w;
                grid.set(px, y_beam - 1, pz, VoxelType::SteelRebar);
            }
        }

        // 2. Continuous Track Bed Crossbeam at y_beam
        for w in -2..=2 {
            let px = cx + nx * w;
            let pz = cz + nz * w;
            grid.set(px, y_beam, pz, VoxelType::Concrete);
        }

        // 3. Dual Guide-Beams & Electrified Third Rail
        let lx = cx - nx * 2;
        let lz = cz - nz * 2;
        grid.set(lx, y_beam + 1, lz, VoxelType::SteelRebar);

        let rx = cx + nx * 2;
        let rz = cz + nz * 2;
        grid.set(rx, y_beam + 1, rz, VoxelType::SteelRebar);

        grid.set(cx, y_beam + 1, cz, VoxelType::PowerCapacitor);
    }

    // 3. Optional Aerodynamic Monorail Train Carriage parked on track
    if with_train && steps >= 8 {
        let mid_s = steps / 2;
        let train_half_len = 4;
        for s in (mid_s - train_half_len)..=(mid_s + train_half_len) {
            let t = s as f32 / steps as f32;
            let cx = (p1.0 as f32 + dx as f32 * t).round() as i32;
            let cz = (p1.1 as f32 + dz as f32 * t).round() as i32;

            for w in -2..=2 {
                let tx = cx + nx * w;
                let tz = cz + nz * w;
                let is_nose = s == mid_s - train_half_len || s == mid_s + train_half_len;

                // Train Floor
                grid.set(tx, y_beam + 2, tz, VoxelType::HeavyArmor);

                // Train Walls / Windows
                if w == -2 || w == 2 {
                    if is_nose {
                        grid.set(tx, y_beam + 3, tz, VoxelType::HeavyArmor);
                    } else {
                        grid.set(tx, y_beam + 3, tz, VoxelType::Glass);
                    }
                } else if is_nose {
                    grid.set(tx, y_beam + 3, tz, VoxelType::Glass);
                }

                // Train Roof & Capacitor Power Core
                if is_nose {
                    grid.set(tx, y_beam + 4, tz, VoxelType::HeavyArmor);
                } else if w == 0 {
                    grid.set(tx, y_beam + 4, tz, VoxelType::PowerCapacitor);
                } else {
                    grid.set(tx, y_beam + 4, tz, VoxelType::HeavyArmor);
                }
            }
        }
    }
}

/// Builds a multi-lane highway deck (either ground-level arterial boulevard or elevated overpass).
pub fn build_multi_lane_road_deck(
    grid: &mut VoxelGrid,
    p1: (i32, i32),
    p2: (i32, i32),
    y_level: i32,
    half_width: i32,
    is_elevated: bool,
) {
    let dx = p2.0 - p1.0;
    let dz = p2.1 - p1.1;
    let steps = dx.abs().max(dz.abs()).max(1);

    let len = ((dx * dx + dz * dz) as f32).sqrt().max(1.0);
    let nx = (-dz as f32 / len).round() as i32;
    let nz = (dx as f32 / len).round() as i32;

    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = (p1.0 as f32 + dx as f32 * t).round() as i32;
        let cz = (p1.1 as f32 + dz as f32 * t).round() as i32;

        let is_pier = is_elevated && (s > 0 && s < steps && s % 10 == 0);
        let has_lamp = s % 8 == 0;

        // 1. Support Piers for Elevated Decks
        if is_pier {
            for py in 1..y_level {
                grid.set(cx - nx * (half_width - 1), py, cz - nz * (half_width - 1), VoxelType::Concrete);
                grid.set(cx + nx * (half_width - 1), py, cz + nz * (half_width - 1), VoxelType::Concrete);
            }
        }

        // 2. Roadway Surface & Lane Dividers
        for w in -half_width..=half_width {
            let rx = cx + nx * w;
            let rz = cz + nz * w;

            let is_shoulder = w == -half_width || w == half_width;
            let is_center_median = w == 0;
            let is_lane_divider = w == -half_width / 2 || w == half_width / 2;

            if is_shoulder {
                // Concrete curb & crash barrier
                grid.set(rx, y_level, rz, VoxelType::Concrete);
                grid.set(rx, y_level + 1, rz, VoxelType::HeavyArmor);

                // Streetlight Pylons
                if has_lamp {
                    grid.set(rx, y_level + 2, rz, VoxelType::SteelRebar);
                    grid.set(rx, y_level + 3, rz, VoxelType::SteelRebar);
                    grid.set(rx, y_level + 4, rz, VoxelType::GlowCore);
                }
            } else if is_center_median {
                if s % 2 == 0 {
                    grid.set(rx, y_level, rz, VoxelType::GlowCore);
                } else {
                    grid.set(rx, y_level, rz, VoxelType::Concrete);
                }
            } else if is_lane_divider {
                if s % 3 == 0 {
                    grid.set(rx, y_level, rz, VoxelType::Concrete);
                } else {
                    grid.set(rx, y_level, rz, VoxelType::Basalt);
                }
            } else {
                grid.set(rx, y_level, rz, VoxelType::Basalt);
            }

            if is_elevated {
                grid.set(rx, y_level - 1, rz, VoxelType::SteelRebar);
            }
        }
    }
}

/// Excavates and constructs a subterranean subway transit tunnel at depth `y_center`
/// ($y \approx -24$, i.e. $Y = -12.0\text{m}$, within subterranean Basalt substrate).
pub fn build_subterranean_subway_tunnel(
    grid: &mut VoxelGrid,
    p1: (i32, i32),
    p2: (i32, i32),
    y_center: i32,
    station_pos: Option<(i32, i32)>,
) {
    let dx = p2.0 - p1.0;
    let dz = p2.1 - p1.1;
    let steps = dx.abs().max(dz.abs()).max(1);

    let len = ((dx * dx + dz * dz) as f32).sqrt().max(1.0);
    let nx = (-dz as f32 / len).round() as i32;
    let nz = (dx as f32 / len).round() as i32;

    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = (p1.0 as f32 + dx as f32 * t).round() as i32;
        let cz = (p1.1 as f32 + dz as f32 * t).round() as i32;

        let is_station_zone = if let Some(sp) = station_pos {
            let dist_sq = (cx - sp.0).pow(2) + (cz - sp.1).pow(2);
            dist_sq <= 36
        } else {
            false
        };

        let tunnel_half_w = if is_station_zone { 4 } else { 3 };

        for w in -(tunnel_half_w + 1)..=(tunnel_half_w + 1) {
            let tx = cx + nx * w;
            let tz = cz + nz * w;

            for dy in -3..=3 {
                let ty = y_center + dy;
                // Preserve surface bedrock slab at y=0
                if ty == 0 {
                    continue;
                }

                let is_wall = w.abs() == tunnel_half_w + 1;
                let is_floor = dy == -3;
                let is_ceiling = dy == 3;

                if (is_ceiling && w == 0 && s % 4 == 0) || (is_station_zone && dy == -1 && w == 2) {
                    grid.set(tx, ty, tz, VoxelType::GlowCore);
                } else if is_floor || is_ceiling || is_wall {
                    grid.set(tx, ty, tz, VoxelType::Concrete);
                } else if dy == -2 {
                    if is_station_zone && w > 1 {
                        grid.set(tx, ty, tz, VoxelType::Concrete);
                    } else if w == -2 || w == 0 {
                        grid.set(tx, ty, tz, VoxelType::SteelRebar);
                    } else if w == -1 {
                        grid.set(tx, ty, tz, VoxelType::PowerCapacitor);
                    } else {
                        grid.set(tx, ty, tz, VoxelType::Air);
                    }
                } else {
                    grid.set(tx, ty, tz, VoxelType::Air);
                }
            }
        }
    }

    // Station Vertical Elevator Access Shaft (ascending from y_center up to y=4, keeping y=0 as Bedrock)
    if let Some(sp) = station_pos {
        for y in y_center..=4 {
            if y == 0 {
                continue;
            }
            for ex in -1..=1 {
                for ez in -1..=1 {
                    let sx = sp.0 + ex;
                    let sz = sp.1 + ez;
                    let is_shaft_wall = ex.abs() == 1 || ez.abs() == 1;
                    if is_shaft_wall {
                        grid.set(sx, y, sz, VoxelType::SteelRebar);
                    } else {
                        grid.set(sx, y, sz, VoxelType::Air);
                    }
                }
            }
        }
    }
}

// ============================================================================
// SKYSCRAPER PROCEDURAL GENERATOR (G4-M2 FEATURE 6)
// ============================================================================

#[derive(Clone, Debug)]
pub struct SkyscraperSpec {
    pub min_x: i32,
    pub max_x: i32,
    pub min_z: i32,
    pub max_z: i32,
    pub height: i32,                 // in voxels (160 to 220)
    pub floor_height: i32,           // typically 6 or 7
    pub core_half_w: i32,            // 2 for 4x4, 3 for 6x6 core
    pub facade_material: VoxelType,  // Glass, Brick, HeavyArmor
    pub structural_material: VoxelType, // Concrete, HeavyArmor
    pub rebar_material: VoxelType,   // CompositeRebar, SteelRebar
    pub corner_col_width: i32,       // 2 or 3
    pub has_skirt: bool,
    pub crown_spire_height: i32,     // extra antenna voxels
    pub capacitor_floors: &'static [i32],
    pub explosive_floors: &'static [i32],
    pub cryo_floors: &'static [i32],
}

pub fn build_standard_skyscraper(grid: &mut VoxelGrid, spec: &SkyscraperSpec) {
    let cx = (spec.min_x + spec.max_x) / 2;
    let cz = (spec.min_z + spec.max_z) / 2;
    let floors = (spec.height - 2) / spec.floor_height + 1;

    // 1. Foundational Footings (y = 1..=4)
    let skirt_extra = if spec.has_skirt { 2 } else { 0 };
    for x in (spec.min_x - skirt_extra)..=(spec.max_x + skirt_extra) {
        for z in (spec.min_z - skirt_extra)..=(spec.max_z + skirt_extra) {
            let is_edge = x < spec.min_x || x > spec.max_x || z < spec.min_z || z > spec.max_z;
            for y in 1..=4 {
                // To ensure continuous tension anchoring, rebar must go all the way down.
                if (x + z) % 2 == 0 {
                    grid.set(x, y, z, spec.rebar_material);
                } else if is_edge {
                    grid.set(x, y, z, VoxelType::HeavyArmor);
                } else {
                    grid.set(x, y, z, spec.structural_material);
                }
            }
        }
    }

    // 2. Stories & Floor Slabs (y = 5..=spec.height)
    for f in 0..floors {
        let y_base = 1 + f * spec.floor_height;
        let y_ceil = (y_base + spec.floor_height - 1).min(spec.height);

        // A. Inter-Floor Slab at y_base
        for x in spec.min_x..=spec.max_x {
            for z in spec.min_z..=spec.max_z {
                if (x + z) % 3 == 0 {
                    grid.set(x, y_base, z, spec.rebar_material);
                } else {
                    grid.set(x, y_base, z, spec.structural_material);
                }
            }
        }

        // B. Central Reinforced Shear Core & Hollow Elevator Shaft
        for y in y_base..=y_ceil {
            for x in (cx - spec.core_half_w)..=(cx + spec.core_half_w) {
                for z in (cz - spec.core_half_w)..=(cz + spec.core_half_w) {
                    let is_core_wall = x == cx - spec.core_half_w
                        || x == cx + spec.core_half_w
                        || z == cz - spec.core_half_w
                        || z == cz + spec.core_half_w;
                    if is_core_wall {
                        // FIX: (x + z) % 2 instead of (x+z+y)%2 to provide continuous vertical tension rebar
                        if (x + z) % 2 == 0 {
                            grid.set(x, y, z, VoxelType::HeavyArmor);
                        } else {
                            grid.set(x, y, z, spec.rebar_material);
                        }
                    } else if (f == 8 || f == 18) && y >= y_base && y <= y_base + 3 {
                        // Elevator counterweight / cab
                        grid.set(x, y, z, VoxelType::HeavyArmor);
                    } else {
                        grid.set(x, y, z, VoxelType::Air);
                    }
                }
            }
        }

        // C. Perimeter Columns
        let w = spec.corner_col_width;
        let col_origins = [
            (spec.min_x, spec.min_z),
            (spec.min_x, spec.max_z - w + 1),
            (spec.max_x - w + 1, spec.min_z),
            (spec.max_x - w + 1, spec.max_z - w + 1),
        ];
        for &(px, pz) in &col_origins {
            for y in y_base..=y_ceil {
                for dx in 0..w {
                    for dz in 0..w {
                        if dx == 1 && dz == 1 {
                            grid.set(px + dx, y, pz + dz, spec.rebar_material);
                        } else {
                            grid.set(px + dx, y, pz + dz, spec.structural_material);
                        }
                    }
                }
            }
        }

        // D. Facade Windows & Spandrels
        for y in (y_base + 1)..y_ceil {
            for x in (spec.min_x + w)..(spec.max_x - w + 1) {
                grid.set(x, y, spec.min_z, spec.facade_material);
                grid.set(x, y, spec.max_z, spec.facade_material);
            }
            for z in (spec.min_z + w)..(spec.max_z - w + 1) {
                grid.set(spec.min_x, y, z, spec.facade_material);
                grid.set(spec.max_x, y, z, spec.facade_material);
            }
        }

        // E. Special Sub-Systems: Capacitors / Explosives / Cryo
        if spec.capacitor_floors.contains(&f) {
            for y in (y_base + 1)..=y_ceil.min(y_base + 2) {
                grid.set(cx - spec.core_half_w - 1, y, cz, VoxelType::PowerCapacitor);
                grid.set(cx + spec.core_half_w + 1, y, cz, VoxelType::PowerCapacitor);
            }
        }
        if spec.explosive_floors.contains(&f) {
            for y in (y_base + 1)..=y_ceil.min(y_base + 2) {
                grid.set(cx, y, cz - spec.core_half_w - 1, VoxelType::Explosive);
                grid.set(cx, y, cz + spec.core_half_w + 1, VoxelType::Explosive);
            }
        }
        if spec.cryo_floors.contains(&f) {
            for y in (y_base + 1)..=y_ceil.min(y_base + 2) {
                grid.set(cx, y, cz + spec.core_half_w + 1, VoxelType::CryoIce);
            }
        }
    }

    // 3. Roof Crown Platform & Antenna Spire
    let roof_y = spec.height;
    for x in spec.min_x..=spec.max_x {
        for z in spec.min_z..=spec.max_z {
            if (x + z) % 2 == 0 {
                grid.set(x, roof_y, z, spec.rebar_material);
            } else {
                grid.set(x, roof_y, z, VoxelType::HeavyArmor);
            }
        }
    }
    if spec.crown_spire_height > 0 {
        let max_spire_y = (roof_y + spec.crown_spire_height).min(MAX_VOXEL_Y - 2);
        for y in (roof_y + 1)..=max_spire_y {
            grid.set(cx, y, cz, spec.rebar_material);
        }
        grid.set(cx, max_spire_y + 1, cz, VoxelType::GlowCore);
    }
}

// 1. Corporate Mega-Block (Level 1): 6 Skyscrapers (85m-105m tall) anchored by 210-voxel HQ Spire,
// financial monoliths, tech arcologies, aerial skybridges, elevated monorail, and subterranean transit.
fn build_orbital_monolith(grid: &mut VoxelGrid) {
    // 1. Six Skyscrapers (85m - 105m / 170 - 210 voxels tall)
    let skyscrapers = [
        // T1: Central Corporate HQ Spire (105m / 210 voxels)
        SkyscraperSpec {
            min_x: 39,
            max_x: 57,
            min_z: 39,
            max_z: 57,
            height: 210,
            floor_height: 7,
            core_half_w: 3,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 3,
            has_skirt: true,
            crown_spire_height: 10,
            capacitor_floors: &[4, 8, 12, 16, 20, 24],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T2: Financial Monolith West (95m / 190 voxels)
        SkyscraperSpec {
            min_x: 13,
            max_x: 27,
            min_z: 25,
            max_z: 39,
            height: 190,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[4, 10, 16],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T3: Financial Monolith East (95m / 190 voxels)
        SkyscraperSpec {
            min_x: 69,
            max_x: 83,
            min_z: 57,
            max_z: 71,
            height: 190,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[4, 10, 16],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T4: Tech Arcology North (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 41,
            max_x: 55,
            min_z: 11,
            max_z: 25,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T5: Tech Arcology Southwest (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 13,
            max_x: 27,
            min_z: 67,
            max_z: 81,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T6: Tech Arcology Southeast (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 69,
            max_x: 83,
            min_z: 15,
            max_z: 29,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
    ];

    for spec in &skyscrapers {
        build_standard_skyscraper(grid, spec);
    }

    // 2. Grand South Concourse / Facade of T1 at x=39..=57, z=57..=71, y=1..=24
    // Integrated directly with T1 and grounded on Bedrock y=1..=4.
    // Invariant: test_orbital_monolith_direct_mortar_shot_does_not_collapse_or_chain_react
    // fires into (48, 15, 70). It creates an entry breach without severing the building or
    // causing unsupported islands.
    for x in 39..=57 {
        for z in 57..=71 {
            // Foundational footing at y=1..=4
            for y in 1..=4 {
                if y == 1 || x == 39 || x == 57 || z == 71 {
                    grid.set(x, y, z, VoxelType::HeavyArmor);
                } else if (x + z) % 2 == 0 {
                    grid.set(x, y, z, VoxelType::CompositeRebar);
                } else {
                    grid.set(x, y, z, VoxelType::Concrete);
                }
            }

            // Slabs at y=6, 12, 18, 24
            for y in [6, 12, 18, 24] {
                if (x + z) % 3 == 0 {
                    grid.set(x, y, z, VoxelType::CompositeRebar);
                } else {
                    grid.set(x, y, z, VoxelType::Concrete);
                }
            }

            // Perimeter columns and South Facade up to y=24
            for y in 5..=24 {
                let is_column = (x - 39) % 3 == 0 && (z == 57 || z == 64 || z >= 69);
                let is_south_wall = z == 70 || z == 71;

                if is_column {
                    if (y == 14 || y == 15 || y == 16) && (47..=49).contains(&x) && z >= 69 {
                        // Direct mortar shot target reinforced core
                        grid.set(x, y, z, VoxelType::HeavyArmor);
                    } else {
                        grid.set(x, y, z, VoxelType::CompositeRebar);
                    }
                } else if is_south_wall {
                    grid.set(x, y, z, VoxelType::Glass);
                }
            }
        }
    }

    // 3. Aerial Skybridges connecting skyscrapers
    // Skybridge 1: Connects T1 to T2 (HQ to Monolith West) at y = 120
    build_suspension_skybridge(
        grid,
        (39, 48),
        (27, 32),
        120,
        2,
        false,
    );
    // Skybridge 2: Connects T1 to T3 (HQ to Monolith East) at y = 140
    build_suspension_skybridge(
        grid,
        (57, 48),
        (69, 64),
        140,
        2,
        false,
    );

    // 4. Elevated Rapid Transit Monorail at y=42
    build_elevated_monorail(grid, (8, 48), (88, 48), 42, 12, true);

    // 5. Multi-Lane Ground Road Deck at y=1
    build_multi_lane_road_deck(grid, (2, 48), (94, 48), 1, 4, false);

    // 6. Subterranean Subway Transit Tunnel at y=-20
    build_subterranean_subway_tunnel(grid, (2, 48), (94, 48), -20, Some((48, 48)));

    // 7. Ground Plaza Paving between towers
    for x in 18..=78 {
        for z in 18..=78 {
            if grid.get(x, 1, z) == VoxelType::Air {
                let is_joint = x % 6 == 0 || z % 6 == 0;
                let vt = if is_joint { VoxelType::HeavyArmor } else { VoxelType::Concrete };
                grid.set(x, 1, z, vt);
            }
        }
    }
}

// 2. Industrial Fusion District (Level 2): 5 Mega-Towers (80m-108m / 160-216 voxels tall)
// with twin 108m suspension pylon towers, 92m industrial reactors, high-level coolant skyways,
// elevated monorail, and subterranean transit.
fn build_suspension_hyperbridge(grid: &mut VoxelGrid) {
    let skyscrapers = [
        // T1: West Suspension Pylon Tower (108m / 216 voxels)
        SkyscraperSpec {
            min_x: 18,
            max_x: 30,
            min_z: 42,
            max_z: 54,
            height: 216,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 3,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[4, 10, 16],
            explosive_floors: &[],
            cryo_floors: &[5, 12, 18],
        },
        // T2: East Suspension Pylon Tower (108m / 216 voxels)
        SkyscraperSpec {
            min_x: 66,
            max_x: 78,
            min_z: 42,
            max_z: 54,
            height: 216,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 3,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[4, 10, 16],
            explosive_floors: &[],
            cryo_floors: &[5, 12, 18],
        },
        // T3: Industrial Reactor North (92m / 184 voxels)
        SkyscraperSpec {
            min_x: 41,
            max_x: 55,
            min_z: 11,
            max_z: 25,
            height: 184,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::HeavyArmor,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20],
        },
        // T4: Industrial Reactor South (92m / 184 voxels)
        SkyscraperSpec {
            min_x: 41,
            max_x: 55,
            min_z: 71,
            max_z: 85,
            height: 184,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::HeavyArmor,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20],
        },
        // T5: Catalytic Processing Center Southwest (83m / 166 voxels)
        SkyscraperSpec {
            min_x: 18,
            max_x: 30,
            min_z: 71,
            max_z: 85,
            height: 166,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Concrete,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[2, 4, 6, 8, 10],
        },
    ];

    for spec in &skyscrapers {
        build_standard_skyscraper(grid, spec);
    }

    let lower_deck_y = 20;
    let upper_deck_y = 34;
    let tower_z = 48;

    // Dual-Deck Highway across full 96-voxel span (X=0..=95)
    // Invariant: No vertical columns between x=31 and x=65 so severing x=33..=36 and x=60..=63
    // causes the mid-span (x=37..=59) to detach as an unsupported island.
    for x in 0..DESIGN_SIZE {
        // Lower Deck at y=20 (width 11: Z=43..=53)
        for dz in -5..=5 {
            grid.set(x, lower_deck_y, tower_z + dz, VoxelType::Concrete);
            if dz == -5 || dz == 5 {
                grid.set(x, lower_deck_y + 1, tower_z + dz, VoxelType::HeavyArmor);
                grid.set(x, lower_deck_y + 2, tower_z + dz, VoxelType::Glass);
            }
        }
        // Suspended structural cable conduit running underneath lower deck
        if x % 2 == 0 {
            grid.set(x, lower_deck_y - 1, tower_z, VoxelType::CompositeRebar);
        } else {
            grid.set(x, lower_deck_y - 1, tower_z, VoxelType::SteelRebar);
        }

        // Upper Deck at y=34 (width 9: Z=44..=52)
        for dz in -4..=4 {
            grid.set(x, upper_deck_y, tower_z + dz, VoxelType::Concrete);
            if dz == -4 || dz == 4 {
                grid.set(x, upper_deck_y + 1, tower_z + dz, VoxelType::CompositeRebar);
            }
        }
        grid.set(x, upper_deck_y - 1, tower_z, VoxelType::CompositeRebar);

        // Cryogenic coolant lines running through the bridge
        grid.set(x, lower_deck_y + 1, tower_z, VoxelType::CryoIce);
        grid.set(x, upper_deck_y + 1, tower_z, VoxelType::CryoIce);
    }

    // Suspension stay-cables connecting towers at x=24 and x=72 to the decks
    for &(tx, pylon_top) in &[(24, 70), (72, 70)] {
        for s in 1..=24 {
            let cable_x_left = tx - s;
            let cable_x_right = tx + s;
            let cable_y = (pylon_top - (s * 35) / 24).max(upper_deck_y + 1);
            let prev_cable_y = if s == 1 { pylon_top } else { (pylon_top - ((s - 1) * 35) / 24).max(upper_deck_y + 1) };

            if cable_x_left >= 0 {
                for y in (cable_y.min(prev_cable_y))..=(cable_y.max(prev_cable_y)) {
                    grid.set(cable_x_left, y, tower_z - 2, VoxelType::CompositeRebar);
                    grid.set(cable_x_left, y, tower_z + 2, VoxelType::CompositeRebar);
                }
            }
            if cable_x_right < DESIGN_SIZE {
                for y in (cable_y.min(prev_cable_y))..=(cable_y.max(prev_cable_y)) {
                    grid.set(cable_x_right, y, tower_z - 2, VoxelType::CompositeRebar);
                    grid.set(cable_x_right, y, tower_z + 2, VoxelType::CompositeRebar);
                }
            }
        }
    }

    // High-altitude skybridge connecting T1 to T3 at y=150
    build_suspension_skybridge(
        grid,
        (24, 48),
        (48, 25),
        150,
        2,
        false,
    );

    // Elevated monorail along z=18 at y=42
    build_elevated_monorail(grid, (8, 18), (88, 18), 42, 12, true);

    // Subterranean subway transit tunnel at y=-20 along z=48
    build_subterranean_subway_tunnel(grid, (2, 48), (94, 48), -20, Some((48, 48)));

    // Multi-lane ground road deck at y=1 along z=18
    build_multi_lane_road_deck(grid, (2, 18), (94, 18), 1, 3, false);
}

// 3. High-Tech Aeropolis (Level 3): 7 Skyscrapers (85m-110m / 170-220 voxels tall)
// with 110m apex comms array, twin orbital transit pylons, 4 modular R&D towers,
// subterranean propellant silos, elevated monorail, and rapid transit.
fn build_cyber_bunker(grid: &mut VoxelGrid) {
    let skyscrapers = [
        // T1: Apex Deep-Space Communications Array (110m / 220 voxels)
        SkyscraperSpec {
            min_x: 40,
            max_x: 56,
            min_z: 40,
            max_z: 56,
            height: 220,
            floor_height: 7,
            core_half_w: 3,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 3,
            has_skirt: true,
            crown_spire_height: 14,
            capacitor_floors: &[4, 10, 16],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T2: Orbital Transit Pylon West (95m / 190 voxels)
        SkyscraperSpec {
            min_x: 13,
            max_x: 27,
            min_z: 41,
            max_z: 55,
            height: 190,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[3, 8],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T3: Orbital Transit Pylon East (95m / 190 voxels)
        SkyscraperSpec {
            min_x: 69,
            max_x: 83,
            min_z: 41,
            max_z: 55,
            height: 190,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 8,
            capacitor_floors: &[3, 8],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T4: Quantum R&D Tower NW (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 16,
            max_x: 28,
            min_z: 16,
            max_z: 28,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T5: Bio-Synthetic R&D Tower NE (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 68,
            max_x: 80,
            min_z: 16,
            max_z: 28,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T6: Propulsion R&D Tower SW (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 16,
            max_x: 28,
            min_z: 68,
            max_z: 80,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
        // T7: Avionics R&D Tower SE (85m / 170 voxels)
        SkyscraperSpec {
            min_x: 68,
            max_x: 80,
            min_z: 68,
            max_z: 80,
            height: 170,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::Concrete,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[],
            explosive_floors: &[],
            cryo_floors: &[],
        },
    ];

    for spec in &skyscrapers {
        build_standard_skyscraper(grid, spec);
    }

    // Subterranean & Ground Propellant Silos (y in [2, 10]):
    // Invariant: test_tier4_workload_cyber_bunker_fuel_silo_chain_reaction
    // and test_world_gen_adversarial_level_specific_cores_and_capacitors require:
    // bunker_explosives >= 400 in y in [1, 20], bunker_capacitors >= 8.
    // Detonating the first explosive must trigger camera trauma > 0.10,
    // clear the first silo, and preserve y=0 Bedrock!
    let silo_centers = [(22, 22), (74, 22), (22, 74), (74, 74), (48, 48)];
    for &(cx, cz) in &silo_centers {
        for y in 2..=12 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    grid.set(cx + dx, y, cz + dz, VoxelType::Explosive);
                }
            }
            // Reinforced containment jacket around explosive core
            for d in -2..=2 {
                grid.set(cx + d, y, cz - 2, VoxelType::HeavyArmor);
                grid.set(cx + d, y, cz + 2, VoxelType::HeavyArmor);
                grid.set(cx - 2, y, cz + d, VoxelType::HeavyArmor);
                grid.set(cx + 2, y, cz + d, VoxelType::HeavyArmor);
            }
        }
    }

    // Central Capacitor Substation in Transit Pylons (y in [2, 4])
    for &(px, pz) in &[(20, 48), (76, 48)] {
        for y in 2..=4 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    grid.set(px + dx, y, pz + dz, VoxelType::PowerCapacitor);
                }
            }
        }
    }

    // Elevated Mag-Lev Monorail at y=44 linking Transit Pylon West -> Apex Comms -> Transit Pylon East
    build_elevated_monorail(grid, (8, 48), (88, 48), 44, 12, true);

    // Aerial Skybridges connecting R&D towers to Apex Comms at y=110
    build_suspension_skybridge(grid, (28, 28), (40, 40), 110, 2, false);
    build_suspension_skybridge(grid, (68, 28), (56, 40), 110, 2, false);
    build_suspension_skybridge(grid, (28, 68), (40, 56), 110, 2, false);
    build_suspension_skybridge(grid, (68, 68), (56, 56), 110, 2, false);

    // Multi-lane ground road deck at y=1
    build_multi_lane_road_deck(grid, (2, 48), (94, 48), 1, 4, false);

    // Subterranean subway transit tunnel at y=-20
    build_subterranean_subway_tunnel(grid, (2, 48), (94, 48), -20, Some((48, 48)));
}

// 4. Fortress Citadel (Level 4): 8 Skyscrapers (80m-105m / 160-210 voxels tall)
// with 105m High Citadel Spire, three 90m Command Keeps, four 80m Defensive Bastions,
// curtain wall ramparts, shield capacitors, cryogenic jackets, and plasma glow cores.
fn build_quantum_core_facility(grid: &mut VoxelGrid) {
    let skyscrapers = [
        // T1: High Citadel Spire (105m / 210 voxels)
        SkyscraperSpec {
            min_x: 40,
            max_x: 56,
            min_z: 40,
            max_z: 56,
            height: 210,
            floor_height: 7,
            core_half_w: 3,
            facade_material: VoxelType::HeavyArmor,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 3,
            has_skirt: true,
            crown_spire_height: 10,
            capacitor_floors: &[4, 8, 12, 16],
            explosive_floors: &[],
            cryo_floors: &[2, 3, 5, 6, 7],
        },
        // T2: Command Keep Alpha (90m / 180 voxels)
        SkyscraperSpec {
            min_x: 42,
            max_x: 54,
            min_z: 18,
            max_z: 30,
            height: 180,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[4, 8],
            explosive_floors: &[],
            cryo_floors: &[3, 5],
        },
        // T3: Command Keep Beta (90m / 180 voxels)
        SkyscraperSpec {
            min_x: 21,
            max_x: 33,
            min_z: 54,
            max_z: 66,
            height: 180,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[4, 8],
            explosive_floors: &[],
            cryo_floors: &[3, 5],
        },
        // T4: Command Keep Gamma (90m / 180 voxels)
        SkyscraperSpec {
            min_x: 63,
            max_x: 75,
            min_z: 54,
            max_z: 66,
            height: 180,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Glass,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 6,
            capacitor_floors: &[4, 8],
            explosive_floors: &[],
            cryo_floors: &[3, 5],
        },
        // T5: Defensive Bastion NW (80m / 160 voxels)
        SkyscraperSpec {
            min_x: 12,
            max_x: 24,
            min_z: 12,
            max_z: 24,
            height: 160,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Concrete,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 4,
            capacitor_floors: &[3],
            explosive_floors: &[],
            cryo_floors: &[2],
        },
        // T6: Defensive Bastion NE (80m / 160 voxels)
        SkyscraperSpec {
            min_x: 72,
            max_x: 84,
            min_z: 12,
            max_z: 24,
            height: 160,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Concrete,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 4,
            capacitor_floors: &[3],
            explosive_floors: &[],
            cryo_floors: &[2],
        },
        // T7: Defensive Bastion SW (80m / 160 voxels)
        SkyscraperSpec {
            min_x: 12,
            max_x: 24,
            min_z: 72,
            max_z: 84,
            height: 160,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Concrete,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 4,
            capacitor_floors: &[3],
            explosive_floors: &[],
            cryo_floors: &[2],
        },
        // T8: Defensive Bastion SE (80m / 160 voxels)
        SkyscraperSpec {
            min_x: 72,
            max_x: 84,
            min_z: 72,
            max_z: 84,
            height: 160,
            floor_height: 6,
            core_half_w: 2,
            facade_material: VoxelType::Concrete,
            structural_material: VoxelType::HeavyArmor,
            rebar_material: VoxelType::CompositeRebar,
            corner_col_width: 2,
            has_skirt: true,
            crown_spire_height: 4,
            capacitor_floors: &[3],
            explosive_floors: &[],
            cryo_floors: &[2],
        },
    ];

    for spec in &skyscrapers {
        build_standard_skyscraper(grid, spec);
    }

    // Material Invariants: test_world_gen_adversarial_level_specific_cores_and_capacitors requires:
    // quantum_capacitors >= 32, quantum_ice >= 200, quantum_glow >= 50.

    // 1. Shield Capacitor Banks along the 4 outer bastion parapets at y=24
    let bastion_centers = [(18, 18), (78, 18), (18, 78), (78, 78)];
    for &(bx, bz) in &bastion_centers {
        for dx in -2..=2 {
            grid.set(bx + dx, 24, bz - 2, VoxelType::PowerCapacitor);
            grid.set(bx + dx, 24, bz + 2, VoxelType::PowerCapacitor);
        }
        for dz in -1..=1 {
            grid.set(bx - 2, 24, bz + dz, VoxelType::PowerCapacitor);
            grid.set(bx + 2, 24, bz + dz, VoxelType::PowerCapacitor);
        }
    }

    // 2. Supercooled Cryogenic Insulation Jacket in T1 Central Citadel (y=10..=45)
    for y in 10..=45 {
        for dx in [-2, 2] {
            for dz in -2..=2 {
                grid.set(48 + dx, y, 48 + dz, VoxelType::CryoIce);
            }
        }
        for dz in [-2, 2] {
            for dx in -1..=1 {
                grid.set(48 + dx, y, 48 + dz, VoxelType::CryoIce);
            }
        }
    }

    // 3. Stabilized Plasma Glow Cores in Keeps Alpha, Beta, Gamma (y=10..=13)
    let keep_centers = [(48, 24), (27, 60), (69, 60)];
    for &(kx, kz) in &keep_centers {
        for y in 10..=13 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    grid.set(kx + dx, y, kz + dz, VoxelType::GlowCore);
                }
            }
        }
    }

    // 4. Heavy Curtain Wall Ramparts linking Bastions at y=1..=16
    for x in 24..=72 {
        for y in 1..=16 {
            grid.set(x, y, 18, VoxelType::HeavyArmor);
            grid.set(x, y, 78, VoxelType::HeavyArmor);
        }
    }
    for z in 24..=72 {
        for y in 1..=16 {
            grid.set(18, y, z, VoxelType::HeavyArmor);
            grid.set(78, y, z, VoxelType::HeavyArmor);
        }
    }

    // 5. Fortified Skybridges connecting Keeps to Central Citadel Spire at y=80
    build_suspension_skybridge(grid, (48, 30), (48, 40), 80, 2, true);
    build_suspension_skybridge(grid, (33, 54), (40, 48), 80, 2, true);
    build_suspension_skybridge(grid, (63, 54), (56, 48), 80, 2, true);

    // 6. Elevated Monorail at y=42
    build_elevated_monorail(grid, (8, 48), (88, 48), 42, 12, true);

    // 7. Multi-lane ground road deck at y=1
    build_multi_lane_road_deck(grid, (2, 48), (94, 48), 1, 4, false);

    // 8. Subterranean subway transit tunnel at y=-20
    build_subterranean_subway_tunnel(grid, (2, 48), (94, 48), -20, Some((48, 48)));
}

// 5. Dense Neon Metropolis (Level 5): 8 Colossal Monolith Spires (85m-105m / 170-210 voxels tall)
// in a circular colosseum ring with Spire 0 at (76, 48), overhead entablature skyways,
// elevated monorail, and subterranean transit calibrated for cross-building domino collapse.
fn build_domino_colosseum(grid: &mut VoxelGrid) {
    let center_x = 48f32;
    let center_z = 48f32;
    let circle_radius = 28.0f32;
    let spire_count = 8;

    // Heights between 85m and 105m (170 to 210 voxels)
    let heights = [210, 180, 190, 170, 200, 170, 190, 180];

    let mut spire_centers = Vec::new();
    for i in 0..spire_count {
        let angle = (i as f32 * (360.0 / spire_count as f32)).to_radians();
        let sx = (center_x + angle.cos() * circle_radius).round() as i32;
        let sz = (center_z + angle.sin() * circle_radius).round() as i32;
        spire_centers.push((sx, sz));
    }

    for (i, &(sx, sz)) in spire_centers.iter().enumerate() {
        let height = heights[i];
        for y in 1..=height {
            for dx in -4i32..=4i32 {
                for dz in -4i32..=4i32 {
                    if y <= 4 {
                        // Foundation hinge & weakpoint (initiates cascading fall)
                        // Invariant: test_domino_colosseum_spire_severance_detects_island
                        // and test_tier4_workload_domino_colosseum_spire_cascade
                        // sever Spire 0 (sx=76, sz=48) at y=1..=4 for dx, dz in -4..=4.
                        if y == 2 || y == 3 {
                            if i == 0 {
                                // Spire 0 has explosive core in the hinge
                                if dx.abs() <= 1 && dz.abs() <= 1 {
                                    grid.set(sx + dx, y, sz + dz, VoxelType::Explosive);
                                } else {
                                    grid.set(sx + dx, y, sz + dz, VoxelType::Brick);
                                }
                            } else {
                                grid.set(sx + dx, y, sz + dz, VoxelType::Brick);
                            }
                        } else {
                            grid.set(sx + dx, y, sz + dz, VoxelType::Concrete);
                        }
                    } else if y >= height - 8 {
                        // Heavy armored hammerhead (adds huge rotational kinetic energy)
                        if dx.abs() == 4 || dz.abs() == 4 {
                            grid.set(sx + dx, y, sz + dz, VoxelType::HeavyArmor);
                        } else {
                            grid.set(sx + dx, y, sz + dz, VoxelType::CompositeRebar);
                        }
                    } else {
                        // Main shaft: Concrete / Glass shell with thick Composite Rebar core
                        if dx.abs() <= 2 && dz.abs() <= 2 {
                            if dx.abs() <= 1 && dz.abs() <= 1 {
                                grid.set(sx + dx, y, sz + dz, VoxelType::CompositeRebar);
                            } else {
                                grid.set(sx + dx, y, sz + dz, VoxelType::SteelRebar);
                            }
                        } else if (dx.abs() == 4 || dz.abs() == 4) && y % 3 == 0 {
                            grid.set(sx + dx, y, sz + dz, VoxelType::Glass);
                        } else {
                            grid.set(sx + dx, y, sz + dz, VoxelType::Concrete);
                        }
                    }
                }
            }
        }
        // Luminescent pinnacle crown
        grid.set(sx, height + 1, sz, VoxelType::GlowCore);
        grid.set(sx, height + 2, sz, VoxelType::GlowCore);
    }

    // Overhead Entablature Arches Bridging Adjacent Spires at y=60..=64
    // HeavyArmor and Concrete cannot suspend downward in tension, ensuring that
    // severing Spire 0's base detects Spire 0 as an unsupported island.
    for i in 0..spire_count {
        let (sx1, sz1) = spire_centers[i];
        let (sx2, sz2) = spire_centers[(i + 1) % spire_count];

        let steps = 20;
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let ax = (sx1 as f32 + (sx2 - sx1) as f32 * t).round() as i32;
            let az = (sz1 as f32 + (sz2 - sz1) as f32 * t).round() as i32;

            for y in 60..=64 {
                for d in -1i32..=1i32 {
                    grid.set(ax + d, y, az, VoxelType::HeavyArmor);
                    grid.set(ax, y, az + d, VoxelType::Concrete);
                }
            }
        }
    }

    // Transit Interconnections:
    // 1. Elevated monorail traversing colosseum along z=48 at y=42
    build_elevated_monorail(grid, (8, 48), (88, 48), 42, 12, true);

    // 2. Multi-lane ground road deck at y=1 along z=48
    build_multi_lane_road_deck(grid, (2, 48), (94, 48), 1, 4, false);

    // 3. Subterranean subway transit tunnel at y=-20 along z=48
    build_subterranean_subway_tunnel(grid, (2, 48), (94, 48), -20, Some((48, 48)));

    // 4. Central arena plaza paving
    for x in 30..=66 {
        for z in 30..=66 {
            let dx = x - 48;
            let dz = z - 48;
            if dx * dx + dz * dz <= 18 * 18 && grid.get(x, 1, z) == VoxelType::Air {
                grid.set(x, 1, z, VoxelType::Concrete);
            }
        }
    }
}

// 6. Neo-Tokyo Cyber-Spire: Twin 74-voxel tapering towers connected by dual skybridges, glass curtain walls, and composite exoskeletons
fn build_neotokyo_spire(grid: &mut VoxelGrid) {
    let tower_a_x = 32i32;
    let tower_b_x = 64i32;
    let center_z = 48i32;
    let height = 74;

    for &cx in &[tower_a_x, tower_b_x] {
        // Build tapering tower (y=1..=height)
        for y in 1..=height {
            let half_w: i32 = if y <= 24 {
                9 // Base podium (19x19)
            } else if y <= 50 {
                7 // Mid tier (15x15)
            } else if y <= 70 {
                5 // High tier (11x11)
            } else {
                3 // Crown spire (7x7)
            };

            // Foundation base (y=1..=4)
            if y <= 4 {
                for dx in -half_w..=half_w {
                    for dz in -half_w..=half_w {
                        if dx.abs() == half_w || dz.abs() == half_w || y == 1 {
                            grid.set(cx + dx, y, center_z + dz, VoxelType::HeavyArmor);
                        } else if (dx + dz) % 3 == 0 {
                            grid.set(cx + dx, y, center_z + dz, VoxelType::CompositeRebar);
                        } else {
                            grid.set(cx + dx, y, center_z + dz, VoxelType::Concrete);
                        }
                    }
                }
                continue;
            }

            // Exoskeleton corner columns
            for &ox in &[-half_w, half_w] {
                for &oz in &[-half_w, half_w] {
                    grid.set(cx + ox, y, center_z + oz, VoxelType::CompositeRebar);
                }
            }

            // Central core (4x4) with volatile pressurized conduit spine
            for dx in -2i32..=2i32 {
                for dz in -2i32..=2i32 {
                    if dx.abs() == 2 || dz.abs() == 2 {
                        grid.set(cx + dx, y, center_z + dz, VoxelType::SteelRebar);
                    } else if dx == 0 && dz == 0 {
                        if y % 8 == 2 {
                            grid.set(cx, y, center_z, VoxelType::GlowCore);
                        } else {
                            grid.set(cx, y, center_z, VoxelType::SteelRebar);
                        }
                    }
                }
            }

            // Floor slabs every 6 levels
            if y % 6 == 0 {
                for dx in -(half_w - 1)..=(half_w - 1) {
                    for dz in -(half_w - 1)..=(half_w - 1) {
                        grid.set(cx + dx, y, center_z + dz, VoxelType::Concrete);
                    }
                }
            }

            // Glass curtain walls and glowing neon bands
            let is_neon_level = y == 20 || y == 45 || y == 65;
            for dx in -(half_w - 1)..=(half_w - 1) {
                if dx.abs() > 2 {
                    let v_type = if is_neon_level { VoxelType::GlowCore } else { VoxelType::Glass };
                    grid.set(cx + dx, y, center_z - half_w, v_type);
                    grid.set(cx + dx, y, center_z + half_w, v_type);
                }
            }
            for dz in -(half_w - 1)..=(half_w - 1) {
                if dz.abs() > 2 {
                    let v_type = if is_neon_level { VoxelType::GlowCore } else { VoxelType::Glass };
                    grid.set(cx - half_w, y, center_z + dz, v_type);
                    grid.set(cx + half_w, y, center_z + dz, v_type);
                }
            }
        }

        // Antenna Mast reaching to y=78
        for y in 75..=78 {
            grid.set(cx, y, center_z, VoxelType::CompositeRebar);
        }
        grid.set(cx, 79, center_z, VoxelType::GlowCore);
    }

    // Dual Skybridges Connecting Tower A and Tower B (X=33..=63, Z=center_z-3..=center_z+3)
    // 1. Lower Skybridge at y=28..=32
    for x in (tower_a_x + 7)..(tower_b_x - 7) {
        for dz in -3..=3 {
            grid.set(x, 28, center_z + dz, VoxelType::Concrete);
            if dz == -3 || dz == 3 {
                grid.set(x, 29, center_z + dz, VoxelType::Glass);
                grid.set(x, 30, center_z + dz, VoxelType::Glass);
                grid.set(x, 31, center_z + dz, VoxelType::HeavyArmor);
            }
            grid.set(x, 32, center_z + dz, VoxelType::HeavyArmor);
        }
        // Suspended conduit under lower skybridge
        if x % 2 == 0 {
            grid.set(x, 27, center_z, VoxelType::CompositeRebar);
        } else {
            grid.set(x, 27, center_z, VoxelType::SteelRebar);
        }
    }

    // 2. Upper Skybridge at y=52..=56
    for x in (tower_a_x + 5)..(tower_b_x - 5) {
        for dz in -3..=3 {
            grid.set(x, 52, center_z + dz, VoxelType::Concrete);
            if dz == -3 || dz == 3 {
                grid.set(x, 53, center_z + dz, VoxelType::Glass);
                grid.set(x, 54, center_z + dz, VoxelType::Glass);
                grid.set(x, 55, center_z + dz, VoxelType::CompositeRebar);
            }
            grid.set(x, 56, center_z + dz, VoxelType::HeavyArmor);
        }
    }

    // Capacitor relay boxes at skybridge anchors (y=33, y=54)
    for &anch_x in &[tower_a_x + 7, tower_b_x - 8] {
        grid.set(anch_x, 33, center_z - 2, VoxelType::PowerCapacitor);
        grid.set(anch_x, 33, center_z + 2, VoxelType::PowerCapacitor);
    }
    for &anch_x in &[tower_a_x + 5, tower_b_x - 6] {
        grid.set(anch_x, 54, center_z - 2, VoxelType::PowerCapacitor);
        grid.set(anch_x, 54, center_z + 2, VoxelType::PowerCapacitor);
    }
}

// 7. Chemical Refinery & Catalytic Reaction Plant:
// Industrial complex with twin hyperbolic cooling towers, central catalytic fractionation column,
// 4 pressurized spherical/cylindrical fuel storage tanks, volatile pipeline network, and substation
fn build_chemical_refinery(grid: &mut VoxelGrid) {
    // A. Foundation Pad & Safety Retention Dike
    for x in 18..=78 {
        for z in 18..=78 {
            grid.set(x, 1, z, VoxelType::Concrete);
            if x == 18 || x == 78 || z == 18 || z == 78 {
                grid.set(x, 2, z, VoxelType::HeavyArmor);
                grid.set(x, 3, z, VoxelType::Concrete);
            }
        }
    }

    // B. Twin Hyperbolic Cooling Towers at (32, 38) and (64, 38)
    for &(cx, cz) in &[(32, 38), (64, 38)] {
        // Base intake pylon stilts (y=1..=5)
        for a in 0..12 {
            let angle = a as f32 * std::f32::consts::TAU / 12.0;
            let px = cx + (angle.cos() * 9.5).round() as i32;
            let pz = cz + (angle.sin() * 9.5).round() as i32;
            for y in 1..=5 {
                grid.set(px, y, pz, VoxelType::CompositeRebar);
            }
        }

        // Hyperbolic concrete shell (y=5..=52)
        for y in 5..=52 {
            let t = (y - 5) as f32 / 47.0;
            let radius = 9.5 - 3.2 * (t * std::f32::consts::PI).sin();
            for dx in -11..=11 {
                for dz in -11..=11 {
                    let dist = ((dx * dx + dz * dz) as f32).sqrt();
                    if (dist - radius).abs() <= 0.88 {
                        // FIX: The hyperbolic shell steps diagonally upwards as it shrinks.
                        // Concrete increases cantilever depth on diagonal steps and fails after 4 blocks.
                        // We must use SteelRebar for the entire shell to allow infinite diagonal ascension.
                        let vt = if y == 52 {
                            VoxelType::HeavyArmor
                        } else {
                            VoxelType::SteelRebar
                        };
                        grid.set(cx + dx, y, cz + dz, vt);
                    }
                }
            }
        }

        // Top warning beacons
        grid.set(cx + 9, 53, cz, VoxelType::GlowCore);
        grid.set(cx - 9, 53, cz, VoxelType::GlowCore);
        grid.set(cx, 53, cz + 9, VoxelType::GlowCore);
        grid.set(cx, 53, cz - 9, VoxelType::GlowCore);

        // Internal cooling fill lattice
        for y in 2..=12 {
            for dx in -6..=6 {
                for dz in -6..=6 {
                    if (dx * dx + dz * dz) <= 36 && (dx + dz) % 2 == 0 {
                        grid.set(cx + dx, y, cz + dz, VoxelType::Brick);
                    }
                }
            }
        }
    }

    // C. Central Catalytic Fractionation Column at (48, 56)
    for y in 1..=58 {
        for dx in -3..=3 {
            for dz in -3..=3 {
                let dist_sq = dx * dx + dz * dz;
                if dist_sq <= 9 {
                    let vt = if dist_sq >= 5 {
                        if y % 6 == 0 {
                            VoxelType::CompositeRebar
                        } else {
                            VoxelType::HeavyArmor
                        }
                    } else if y % 4 == 0 {
                        VoxelType::PowerCapacitor
                    } else {
                        VoxelType::Concrete
                    };
                    grid.set(48 + dx, y, 56 + dz, vt);
                }
            }
        }
    }

    // Fractionation column flare stack
    for y in 59..=66 {
        grid.set(48, y, 56, VoxelType::SteelRebar);
    }
    grid.set(48, 67, 56, VoxelType::GlowCore);
    grid.set(48, 68, 56, VoxelType::GlowCore);

    // Structural scaffolding tower around cracker column
    for &(col_x, col_z) in &[(43, 51), (53, 51), (43, 61), (53, 61)] {
        for y in 1..=50 {
            grid.set(col_x, y, col_z, VoxelType::CompositeRebar);
        }
    }
    for &plat_y in &[14, 26, 38, 50] {
        for x in 43..=53 {
            for z in 51..=61 {
                if x == 43 || x == 53 || z == 51 || z == 61 {
                    grid.set(x, plat_y, z, VoxelType::SteelRebar);
                }
            }
        }
    }

    // D. Pressurized Liquefied Fuel Tanks (Tank Farm)
    // 2 Spherical pressurized tanks
    for &(tx, tz) in &[(26, 64), (70, 64)] {
        // Support pedestals
        for &(px, pz) in &[(tx - 3, tz - 3), (tx + 3, tz - 3), (tx - 3, tz + 3), (tx + 3, tz + 3)] {
            for y in 1..=5 {
                grid.set(px, y, pz, VoxelType::CompositeRebar);
            }
        }
        for dx in -5..=5 {
            for dy in -5..=5 {
                for dz in -5..=5 {
                    let r_sq = dx * dx + dy * dy + dz * dz;
                    if r_sq <= 25 {
                        let y = 10 + dy;
                        let vt = if r_sq >= 18 {
                            VoxelType::HeavyArmor
                        } else if r_sq >= 11 {
                            VoxelType::SteelRebar
                        } else {
                            VoxelType::Explosive
                        };
                        grid.set(tx + dx, y, tz + dz, vt);
                    }
                }
            }
        }
    }

    // 2 Cylindrical storage tanks at (24, 24) and (72, 24)
    for &(tx, tz) in &[(24, 24), (72, 24)] {
        for y in 1..=18 {
            for dx in -5..=5 {
                for dz in -5..=5 {
                    let r_sq = dx * dx + dz * dz;
                    if r_sq <= 25 {
                        let vt = if r_sq >= 18 || y == 18 {
                            VoxelType::Concrete
                        } else {
                            VoxelType::Explosive
                        };
                        grid.set(tx + dx, y, tz + dz, vt);
                    }
                }
            }
        }
    }

    // E. Volatile Fuel Pipeline Network (Pipe Racks)
    // Pipeline from West Sphere (26, 64) to Cracker (48, 56)
    for x in 26..=45 {
        grid.set(x, 6, 64, VoxelType::Explosive);
        grid.set(x, 5, 64, VoxelType::SteelRebar);
        grid.set(x, 7, 64, VoxelType::SteelRebar);
        if x % 5 == 0 {
            for py in 1..=5 {
                grid.set(x, py, 64, VoxelType::CompositeRebar);
            }
        }
    }
    for z in 56..=64 {
        grid.set(45, 6, z, VoxelType::Explosive);
        grid.set(45, 5, z, VoxelType::SteelRebar);
        grid.set(45, 7, z, VoxelType::SteelRebar);
    }

    // Pipeline from East Sphere (70, 64) to Cracker (48, 56)
    for x in 51..=70 {
        grid.set(x, 6, 64, VoxelType::Explosive);
        grid.set(x, 5, 64, VoxelType::SteelRebar);
        grid.set(x, 7, 64, VoxelType::SteelRebar);
        if x % 5 == 0 {
            for py in 1..=5 {
                grid.set(x, py, 64, VoxelType::CompositeRebar);
            }
        }
    }
    for z in 56..=64 {
        grid.set(51, 6, z, VoxelType::Explosive);
        grid.set(51, 5, z, VoxelType::SteelRebar);
        grid.set(51, 7, z, VoxelType::SteelRebar);
    }

    // High overhead conduit to cooling towers (y=16)
    for &(cx, cz) in &[(32, 38), (64, 38)] {
        let step_x = if cx < 48 { -1 } else { 1 };
        let mut cur_x = 48;
        while cur_x != cx {
            grid.set(cur_x, 16, 48, VoxelType::Explosive);
            grid.set(cur_x, 15, 48, VoxelType::SteelRebar);
            cur_x += step_x;
        }
        for z in cz..=48 {
            grid.set(cx, 16, z, VoxelType::Explosive);
            grid.set(cx, 15, z, VoxelType::SteelRebar);
        }
    }

    // F. Power Substation at (48, 72)
    for x in 44..=52 {
        for z in 70..=74 {
            grid.set(x, 2, z, VoxelType::Concrete);
            if (x + z) % 2 == 0 {
                grid.set(x, 3, z, VoxelType::PowerCapacitor);
                grid.set(x, 4, z, VoxelType::SteelRebar);
            }
        }
    }
}

// 8. Orbital Launch Gantry & Mobile Service Tower:
// Massive orbital rocket launch complex featuring an 84-voxel reinforced service tower,
// launch deck with flame trench and deflector, cantilevered swing arms, and multi-stage heavy lifter
fn build_orbital_launch_gantry(grid: &mut VoxelGrid) {
    // A. Concrete Launch Mount Table (y=1..=8, 26..=70 x 26..=70)
    for x in 26..=70 {
        for z in 26..=70 {
            // Cutout central flame exhaust trench along z for x in 43..=53
            let in_trench = (43..=53).contains(&x) && (22..=74).contains(&z);
            if in_trench {
                grid.set(x, 1, z, VoxelType::Concrete);
                if (44..=52).contains(&z) {
                    let slope_y = 5 - (z - 48).abs();
                    for y in 2..=slope_y.max(2) {
                        grid.set(x, y, z, VoxelType::HeavyArmor);
                    }
                }
            } else {
                for y in 1..=8 {
                    let vt = if y == 8 {
                        VoxelType::HeavyArmor
                    } else if (x + z) % 4 == 0 {
                        VoxelType::CompositeRebar
                    } else {
                        VoxelType::Concrete
                    };
                    grid.set(x, y, z, vt);
                }
            }
        }
    }

    // B. Reinforced Umbilical Service Gantry Tower (West flank: x in 30..=38, z in 43..=53)
    let twr_min_x = 30;
    let twr_max_x = 38;
    let twr_min_z = 43;
    let twr_max_z = 53;

    // Corner columns rising to y=84
    for &(cx, cz) in &[(twr_min_x, twr_min_z), (twr_max_x, twr_min_z), (twr_min_x, twr_max_z), (twr_max_x, twr_max_z)] {
        for y in 8..=84 {
            grid.set(cx, y, cz, VoxelType::CompositeRebar);
        }
    }

    // Tower truss framing and horizontal floor diaphragms every 4 levels
    for y in 8..=84 {
        let is_floor = y % 4 == 0;
        for x in twr_min_x..=twr_max_x {
            for z in twr_min_z..=twr_max_z {
                let is_perimeter = x == twr_min_x || x == twr_max_x || z == twr_min_z || z == twr_max_z;
                if is_perimeter && (is_floor || (x + y + z) % 3 == 0) {
                    grid.set(x, y, z, VoxelType::SteelRebar);
                }
            }
        }
        // Central elevator / umbilical spine
        grid.set(34, y, 48, VoxelType::Concrete);
        grid.set(34, y, 47, VoxelType::Glass);
        grid.set(34, y, 49, VoxelType::Glass);
    }

    // Gantry top hammerhead crane and lightning spire (y=84..=96)
    for x in 30..=56 {
        for z in 47..=49 {
            grid.set(x, 85, z, VoxelType::SteelRebar);
            grid.set(x, 86, z, VoxelType::HeavyArmor);
        }
    }
    for y in 86..=95 {
        grid.set(34, y, 48, VoxelType::CompositeRebar);
    }
    grid.set(34, 96, 48, VoxelType::GlowCore);

    // 4 Cantilevered Swing Arms extending eastward towards the rocket
    let arm_levels = [
        (22, VoxelType::PowerCapacitor),
        (40, VoxelType::Explosive),
        (58, VoxelType::CryoIce),
        (74, VoxelType::Glass),
    ];
    for &(arm_y, core_mat) in &arm_levels {
        for x in twr_max_x..=46 {
            for z in 47..=49 {
                grid.set(x, arm_y, z, VoxelType::SteelRebar);
                if x >= 42 && z == 48 {
                    grid.set(x, arm_y + 1, z, core_mat);
                }
            }
        }
    }

    // C. Multi-Stage Heavy Orbital Rocket at (48, 48)
    for &(hx, hz) in &[(44, 44), (52, 44), (44, 52), (52, 52)] {
        for y in 8..=11 {
            grid.set(hx, y, hz, VoxelType::HeavyArmor);
        }
    }

    // Rocket Stage 1 Core (y=11..=46, radius 3.5)
    for y in 11..=46 {
        for dx in -3..=3 {
            for dz in -3..=3 {
                let d_sq = dx * dx + dz * dz;
                if d_sq <= 11 {
                    let vt = if d_sq >= 7 || y <= 13 {
                        VoxelType::HeavyArmor
                    } else if (dx + dz) % 2 == 0 {
                        VoxelType::Explosive
                    } else {
                        VoxelType::SteelRebar
                    };
                    grid.set(48 + dx, y, 48 + dz, vt);
                }
            }
        }
    }

    // 2 Strap-on Liquid Boosters at (48, 40) and (48, 56) (y=11..=38, radius 2.5)
    for &(bx, bz) in &[(48, 40), (48, 56)] {
        for y in 11..=38 {
            for dx in -2..=2 {
                for dz in -2..=2 {
                    let d_sq = dx * dx + dz * dz;
                    if d_sq <= 5 {
                        let vt = if d_sq >= 4 || y == 38 {
                            VoxelType::HeavyArmor
                        } else {
                            VoxelType::Explosive
                        };
                        grid.set(bx + dx, y, bz + dz, vt);
                    }
                }
            }
        }
    }

    // Rocket Stage 2 Cryogenic Booster (y=47..=66, radius 3.5)
    for y in 47..=66 {
        for dx in -3..=3 {
            for dz in -3..=3 {
                let d_sq = dx * dx + dz * dz;
                if d_sq <= 11 {
                    let vt = if d_sq >= 7 {
                        VoxelType::CompositeRebar
                    } else if y >= 56 {
                        VoxelType::CryoIce
                    } else {
                        VoxelType::Explosive
                    };
                    grid.set(48 + dx, y, 48 + dz, vt);
                }
            }
        }
    }

    // Rocket Stage 3 & Payload Fairing (y=67..=84)
    for y in 67..=84 {
        let max_r = if y >= 77 {
            (84 - y) as f32 * 0.45 + 0.5
        } else {
            3.2
        };
        for dx in -3..=3 {
            for dz in -3..=3 {
                let dist = ((dx * dx + dz * dz) as f32).sqrt();
                if dist <= max_r {
                    let is_skin = (dist - max_r).abs() <= 0.9 || y == 84;
                    let vt = if is_skin {
                        VoxelType::HeavyArmor
                    } else if (70..=74).contains(&y) {
                        if dx == 0 && dz == 0 {
                            VoxelType::GlowCore
                        } else {
                            VoxelType::PowerCapacitor
                        }
                    } else {
                        VoxelType::CompositeRebar
                    };
                    grid.set(48 + dx, y, 48 + dz, vt);
                }
            }
        }
    }
}

// 9. Quantum Fusion Tokamak Facility:
// Massive magnetic confinement energy complex featuring octagonal bioshield dome,
// toroidal tokamak chamber with 16 superconducting magnetic coils, central plasma glow core,
// and 4 high-voltage quadrant capacitor banks
fn build_quantum_fusion_tokamak(grid: &mut VoxelGrid) {
    // A. Foundation Slab (y=1..=3)
    for x in 22..=74 {
        for z in 22..=74 {
            let dx = (x as f32 - 48.0).abs();
            let dz = (z as f32 - 48.0).abs();
            let oct_dist = dx.max(dz).max((dx + dz) * 0.707);
            if oct_dist <= 26.5 {
                grid.set(x, 1, z, VoxelType::HeavyArmor);
                grid.set(x, 2, z, VoxelType::Concrete);
                grid.set(x, 3, z, VoxelType::Concrete);
            }
        }
    }

    // B. Octagonal Bioshield Perimeter Containment Wall (y=3..=28)
    for y in 3..=28 {
        for x in 22..=74 {
            for z in 22..=74 {
                let dx = (x as f32 - 48.0).abs();
                let dz = (z as f32 - 48.0).abs();
                let oct_dist = dx.max(dz).max((dx + dz) * 0.707);
                if (24.0..=26.5).contains(&oct_dist) {
                    let vt = if y % 6 == 0 {
                        VoxelType::CompositeRebar
                    } else if oct_dist >= 25.5 {
                        VoxelType::HeavyArmor
                    } else {
                        VoxelType::Concrete
                    };
                    grid.set(x, y, z, vt);
                }
            }
        }
    }

    // C. Flying Buttress Arched Roof & Compression Crown (y=28..=54)
    for a in 0..8 {
        let angle = a as f32 * std::f32::consts::TAU / 8.0;
        let (sin_a, cos_a) = angle.sin_cos();
        for step in 0..=24 {
            let t = step as f32 / 24.0;
            let cur_r = 25.0 * (1.0 - t) + 4.0 * t;
            let arch_y = (28.0 + 26.0 * (t * std::f32::consts::FRAC_PI_2).sin()).round() as i32;
            let ax = (48.0 + cos_a * cur_r).round() as i32;
            let az = (48.0 + sin_a * cur_r).round() as i32;
            grid.set(ax, arch_y, az, VoxelType::CompositeRebar);
            grid.set(ax, arch_y - 1, az, VoxelType::HeavyArmor);
        }
    }
    // Crown tension ring at (48, 48), y=53..=55
    for dx in -4..=4 {
        for dz in -4..=4 {
            let r_sq = dx * dx + dz * dz;
            if (9..=18).contains(&r_sq) {
                for y in 53..=55 {
                    grid.set(48 + dx, y, 48 + dz, VoxelType::CompositeRebar);
                }
            }
        }
    }

    // D. Toroidal Tokamak Chamber at (48, 48), y=12..=22
    for x in 28..=68 {
        for z in 28..=68 {
            let dx = x as f32 - 48.0;
            let dz = z as f32 - 48.0;
            let horiz_dist = (dx * dx + dz * dz).sqrt();
            if (9.0..=19.0).contains(&horiz_dist) {
                for y in 12..=22 {
                    let dy = y as f32 - 17.0;
                    let torus_dist = ((horiz_dist - 14.0).powi(2) + dy * dy).sqrt();
                    if (2.8..=4.2).contains(&torus_dist) {
                        grid.set(x, y, z, VoxelType::HeavyArmor);
                    }
                }
            }
        }
    }

    // 16 Poloidal Magnetic Field Coils surrounding the vacuum vessel
    for c in 0..16 {
        let theta = c as f32 * std::f32::consts::TAU / 16.0;
        let (sin_t, cos_t) = theta.sin_cos();
        for phi_step in 0..16 {
            let phi = phi_step as f32 * std::f32::consts::TAU / 16.0;
            let (sin_p, cos_p) = phi.sin_cos();
            let coil_r = 14.0 + 4.6 * cos_p;
            let coil_y = (17.0 + 4.6 * sin_p).round() as i32;
            let px = (48.0 + cos_t * coil_r).round() as i32;
            let pz = (48.0 + sin_t * coil_r).round() as i32;

            grid.set(px, coil_y, pz, VoxelType::PowerCapacitor);
            let cryo_r = 14.0 + 5.3 * cos_p;
            let cryo_y = (17.0 + 5.3 * sin_p).round() as i32;
            let cx = (48.0 + cos_t * cryo_r).round() as i32;
            let cz = (48.0 + sin_t * cryo_r).round() as i32;
            grid.set(cx, cryo_y, cz, VoxelType::CryoIce);
        }
    }

    // Tokamak support pedestals (16 legs)
    for c in 0..16 {
        let theta = c as f32 * std::f32::consts::TAU / 16.0;
        let px = (48.0 + theta.cos() * 14.0).round() as i32;
        let pz = (48.0 + theta.sin() * 14.0).round() as i32;
        for y in 3..=12 {
            grid.set(px, y, pz, VoxelType::CompositeRebar);
        }
    }

    // E. Central Solenoid & Stabilized Plasma Glow Core
    for y in 3..=26 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                let vt = if (dx == -2 || dx == 2 || dz == -2 || dz == 2) && y % 3 == 0 {
                    VoxelType::PowerCapacitor
                } else {
                    VoxelType::CompositeRebar
                };
                grid.set(48 + dx, y, 48 + dz, vt);
            }
        }
    }
    for dx in -2..=2 {
        for dy in -2..=2 {
            for dz in -2..=2 {
                if dx * dx + dy * dy + dz * dz <= 5 {
                    grid.set(48 + dx, 17 + dy, 48 + dz, VoxelType::GlowCore);
                }
            }
        }
    }

    // F. 4 High-Voltage Quadrant Capacitor Banks
    for &(qcx, qcz) in &[(33, 33), (63, 33), (33, 63), (63, 63)] {
        for y in 4..=14 {
            for dx in -3..=3 {
                for dz in -3..=3 {
                    let is_frame = dx == -3 || dx == 3 || dz == -3 || dz == 3;
                    let vt = if is_frame {
                        VoxelType::SteelRebar
                    } else if y % 3 == 0 {
                        VoxelType::PowerCapacitor
                    } else {
                        VoxelType::CompositeRebar
                    };
                    grid.set(qcx + dx, y, qcz + dz, vt);
                }
            }
        }

        let step_x = if qcx < 48 { 1 } else { -1 };
        let mut cur_x = qcx;
        while (cur_x - 48).abs() > 14 {
            grid.set(cur_x, 17, qcz, VoxelType::CompositeRebar);
            grid.set(cur_x, 18, qcz, VoxelType::PowerCapacitor);
            cur_x += step_x;
        }
    }

    // G. Elevated Observation & Control Gallery
    for x in 38..=58 {
        for z in 68..=71 {
            grid.set(x, 22, z, VoxelType::Concrete);
            grid.set(x, 27, z, VoxelType::HeavyArmor);
            if z == 68 {
                grid.set(x, 23, z, VoxelType::Glass);
                grid.set(x, 24, z, VoxelType::Glass);
                grid.set(x, 25, z, VoxelType::Glass);
                grid.set(x, 26, z, VoxelType::CompositeRebar);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_six_levels_generate_valid_structures() {
        for &preset in LevelPreset::all() {
            let mut grid = VoxelGrid::new();
            generate_level(&mut grid, preset);

            println!("{:?}: total voxels = {}, rebar = {}", preset, grid.total_initial_voxels, grid.initial_rebar_count);
            assert!(grid.total_initial_voxels > 2000, "Level {:?} must have at least 2000 voxels, got {}", preset, grid.total_initial_voxels);
            assert_eq!(grid.current_voxels, grid.total_initial_voxels);
            assert!(grid.initial_rebar_count > 100, "Level {:?} must have load-bearing rebar", preset);
            assert_eq!(grid.get(0, 0, 0), VoxelType::Bedrock, "Bedrock must remain immutable");
        }
    }

    #[test]
    fn test_all_nine_levels_contain_signature_destructible_elements() {
        let mut grid = VoxelGrid::new();

        // Level 7: Chemical Refinery
        generate_level(&mut grid, LevelPreset::ChemicalRefinery);
        let mut ref_explosives = 0;
        let mut ref_capacitors = 0;
        for vt in &grid.voxels {
            if *vt == VoxelType::Explosive { ref_explosives += 1; }
            else if *vt == VoxelType::PowerCapacitor { ref_capacitors += 1; }
        }
        assert!(ref_explosives >= 100, "Refinery must have explosive fuel conduits, got {}", ref_explosives);
        assert!(ref_capacitors >= 4, "Refinery must have substation capacitors, got {}", ref_capacitors);

        // Level 8: Orbital Launch Gantry
        generate_level(&mut grid, LevelPreset::OrbitalLaunchGantry);
        let mut gantry_explosives = 0;
        let mut gantry_armor = 0;
        for vt in &grid.voxels {
            if *vt == VoxelType::Explosive { gantry_explosives += 1; }
            else if *vt == VoxelType::HeavyArmor { gantry_armor += 1; }
        }
        assert!(gantry_explosives >= 100, "Gantry rocket must have liquid propellant fuel cores, got {}", gantry_explosives);
        assert!(gantry_armor >= 500, "Gantry rocket and pad must have heavy armor casing, got {}", gantry_armor);

        // Level 9: Quantum Fusion Tokamak
        generate_level(&mut grid, LevelPreset::QuantumFusionReactor);
        let mut tokamak_capacitors = 0;
        let mut tokamak_glow = 0;
        let mut tokamak_ice = 0;
        for vt in &grid.voxels {
            if *vt == VoxelType::PowerCapacitor { tokamak_capacitors += 1; }
            else if *vt == VoxelType::GlowCore { tokamak_glow += 1; }
            else if *vt == VoxelType::CryoIce { tokamak_ice += 1; }
        }
        assert!(tokamak_capacitors >= 32, "Tokamak must have >= 32 magnetic coils/capacitors, got {}", tokamak_capacitors);
        assert!(tokamak_glow >= 5, "Tokamak must have central plasma glow core, got {}", tokamak_glow);
        assert!(tokamak_ice >= 20, "Tokamak must have superconducting cryo cooling jackets, got {}", tokamak_ice);
    }

    #[test]
    fn test_domino_colosseum_spire_severance_detects_island() {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);

        // Spire 0 center is at design (76, 48)
        let sx = layout.map_x(76);
        let sz = layout.map_z(48);

        // Sever the base of Spire 0 at y=1..=4
        for y in 1..=4 {
            for dx in -4..=4 {
                for dz in -4..=4 {
                    grid.set(sx + dx, y, sz + dz, VoxelType::Air);
                }
            }
        }

        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Severed Spire 0 must be detected as an unsupported island!");

        let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
        assert!(total_island_voxels > 500, "Spire 0 should have substantial voxels in the unsupported island, got {}", total_island_voxels);
    }

    #[test]
    fn test_orbital_monolith_direct_mortar_shot_does_not_collapse_or_chain_react() {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, LevelPreset::ApexSkyscraper);
        let initial_voxels = grid.current_voxels;
        assert!(initial_voxels > 5000, "Orbital monolith must have substantial voxel mass");

        // Fire direct Havoc Mortar blast into front facade / central column at y=15
        let hit_pos = grid.voxel_to_world(layout.map_voxel(48, 15, 70));
        let res = grid.carve_sphere_advanced(hit_pos, 3.2, 220.0, crate::voxel::DamageType::Explosive);

        // 1. Verify ZERO explosive chain triggers inside the citadel core
        assert!(
            res.explosive_triggers.is_empty(),
            "Citadel core must NOT contain explosive conduits that cause accidental chain reactions! Found: {:?}",
            res.explosive_triggers
        );

        // 2. Verify damage was realistic (creates breach crater without wiping the building)
        assert!(res.destroyed > 0, "Shot must carve an entry breach");
        assert!(res.destroyed < 500, "A single mortar shot should not destroy more than 500 voxels, got {}", res.destroyed);

        // 3. Verify structural integrity: building MUST NOT collapse or detach as an island
        let islands = grid.detect_unsupported_islands();
        assert!(
            islands.is_empty(),
            "Orbital monolith must remain standing after a single mortar shot! Detected islands: {}",
            islands.len()
        );
    }

    #[test]
    fn test_world_gen_power_capacitors_generated_in_levels() {
        let mut grid = VoxelGrid::new();
        generate_level(&mut grid, LevelPreset::ApexSkyscraper);
        let mut cap_count = 0;
        for vt in &grid.voxels {
            if *vt == VoxelType::PowerCapacitor {
                cap_count += 1;
            }
        }
        assert!(cap_count >= 8, "ApexSkyscraper must generate capacitor banks, found {}", cap_count);

        generate_level(&mut grid, LevelPreset::MilitaryFortress);
        let mut bunker_caps = 0;
        for vt in &grid.voxels {
            if *vt == VoxelType::PowerCapacitor {
                bunker_caps += 1;
            }
        }
        assert!(bunker_caps >= 8, "MilitaryFortress must generate central capacitor substation, found {}", bunker_caps);
    }

    #[test]
    fn test_subway_tunnel_generates_glowcore_ceiling_lights() {
        let mut grid = VoxelGrid::new();
        build_subterranean_subway_tunnel(&mut grid, (10, 48), (30, 48), -20, Some((20, 48)));
        let mut ceiling_lights = 0;
        let y_ceiling = -20 + 3;
        for x in 10..=30 {
            if grid.get(x, y_ceiling, 48) == VoxelType::GlowCore {
                ceiling_lights += 1;
            }
        }
        assert!(
            ceiling_lights > 0,
            "Subterranean subway tunnel must generate GlowCore ceiling lights at y={}, found {}",
            y_ceiling,
            ceiling_lights
        );
    }
}

