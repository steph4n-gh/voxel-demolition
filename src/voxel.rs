use macroquad::prelude::*;
use std::collections::VecDeque;

pub const VOXEL_SIZE: f32 = 0.5;
pub const CHUNK_SIZE: usize = 8;
pub const CHUNKS_X: usize = 24;
pub const CHUNKS_SUBTERRANEAN_Y: usize = 6; // 48 voxels (-24.0m to 0.0m)
pub const CHUNKS_SURFACE_Y: usize = 28; // 224 voxels (0.0m to 112.0m)
pub const CHUNKS_Y: usize = CHUNKS_SUBTERRANEAN_Y + CHUNKS_SURFACE_Y; // 34 chunks (272 voxels)
pub const CHUNKS_Z: usize = 24;

pub const GRID_X: usize = CHUNKS_X * CHUNK_SIZE; // 192 (96.0m)
pub const GRID_Y: usize = CHUNKS_Y * CHUNK_SIZE; // 272 (136.0m total vertical span)
pub const GRID_Z: usize = CHUNKS_Z * CHUNK_SIZE; // 192 (96.0m)

pub const MIN_VOXEL_Y: i32 = -(CHUNKS_SUBTERRANEAN_Y as i32 * CHUNK_SIZE as i32); // -48
pub const MAX_VOXEL_Y: i32 = CHUNKS_SURFACE_Y as i32 * CHUNK_SIZE as i32; // 224
pub const VOID_BOUNDARY_Y: f32 = -24.0;
pub const BEDROCK_Y: i32 = -24;

pub const CANTILEVER_MAX_DEPTH: u8 = 4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum VoxelType {
    Air = 0,
    Concrete = 1,
    SteelRebar = 2,
    Brick = 3,
    Glass = 4,
    Explosive = 5,
    Bedrock = 6,
    CryoIce = 7,
    GlowCore = 8,
    HeavyArmor = 9,
    CompositeRebar = 10,
    PowerCapacitor = 11,
    Basalt = 12,
}

#[derive(Copy, Clone, Debug)]
pub struct MaterialProperties {
    pub name: &'static str,
    pub max_health: f32,
    pub hardness: f32,        // Resistance to penetration & drilling (1.0 = base, 10.0 = extreme)
    pub toughness: f32,       // Resistance to explosive shattering (1.0 = base, 10.0 = extreme)
    pub blast_threshold: f32, // Minimum blast damage to cause injury (blast deflection)
    pub density: f32,         // Physics mass density for rigid bodies
    pub thermal_soften: f32,  // Multiplier for plasma/thermal ablation
    pub is_emissive: bool,
    pub conductivity: f32,    // Electrical conductivity (0.0 = non-conductive, 12.0 = superconductor)
    pub compressive_capacity: f32, // Maximum compressive load capacity before crushing/buckling (kg/voxel)
    pub tensile_capacity: f32,     // Maximum tensile load capacity before snapping/severing (kg/voxel)
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DamageType {
    Explosive,
    Kinetic,
    Thermal,
    Cryo,
    Concussive,
}

impl VoxelType {
    /// Material response shared by static structures and detached debris.
    pub fn effective_damage(self, raw_damage: f32, dtype: DamageType) -> f32 {
        let props = self.properties();
        if matches!(dtype, DamageType::Explosive | DamageType::Concussive)
            && raw_damage < props.blast_threshold
        {
            return 0.0;
        }
        match dtype {
            DamageType::Explosive => raw_damage / (props.toughness * 0.22).max(0.2),
            DamageType::Kinetic => raw_damage / (props.hardness * 0.20 + props.toughness * 0.12).max(0.2),
            DamageType::Thermal => raw_damage * props.thermal_soften,
            DamageType::Cryo => raw_damage * 1.5,
            DamageType::Concussive if props.toughness < 4.0 => raw_damage * 1.6,
            DamageType::Concussive => raw_damage / (props.toughness * 0.3).max(0.25),
        }
    }

    pub fn is_solid(&self) -> bool {
        !matches!(self, VoxelType::Air)
    }

    pub fn is_transparent(&self) -> bool {
        matches!(self, VoxelType::Air | VoxelType::Glass | VoxelType::CryoIce)
    }

    pub fn is_opaque(&self) -> bool {
        self.is_solid() && !matches!(self, VoxelType::Glass | VoxelType::CryoIce)
    }

    pub fn is_destructible(&self) -> bool {
        !matches!(self, VoxelType::Air)
    }

    pub fn properties(&self) -> MaterialProperties {
        match self {
            VoxelType::Air => MaterialProperties {
                name: "AIR",
                max_health: 0.0,
                hardness: 0.0,
                toughness: 0.0,
                blast_threshold: 0.0,
                density: 0.0,
                thermal_soften: 1.0,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 0.0,
                tensile_capacity: 0.0,
            },
            VoxelType::Concrete => MaterialProperties {
                name: "CAST CONCRETE",
                max_health: 70.0,
                hardness: 3.5,
                toughness: 3.5,
                blast_threshold: 8.0,
                density: 2.4,
                thermal_soften: 1.0,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 12000.0,
                tensile_capacity: 800.0,
            },
            VoxelType::SteelRebar => MaterialProperties {
                name: "REINFORCED STEEL REBAR",
                max_health: 140.0,
                hardness: 7.0,
                toughness: 7.5,
                blast_threshold: 18.0,
                density: 5.5,
                thermal_soften: 1.2,
                is_emissive: false,
                conductivity: 10.0,
                compressive_capacity: 30000.0,
                tensile_capacity: 25000.0,
            },
            VoxelType::HeavyArmor => MaterialProperties {
                name: "TUNGSTEN HEAVY ARMOR",
                max_health: 240.0,
                hardness: 9.5,
                toughness: 8.5,
                blast_threshold: 28.0,
                density: 7.8,
                thermal_soften: 1.6,
                is_emissive: false,
                conductivity: 9.0,
                compressive_capacity: 50000.0,
                tensile_capacity: 20000.0,
            },
            VoxelType::CompositeRebar => MaterialProperties {
                name: "COMPOSITE CARBON REBAR",
                max_health: 180.0,
                hardness: 8.5,
                toughness: 8.8,
                blast_threshold: 22.0,
                density: 4.2,
                thermal_soften: 0.9,
                is_emissive: false,
                conductivity: 8.5,
                compressive_capacity: 25000.0,
                tensile_capacity: 35000.0,
            },
            VoxelType::Brick => MaterialProperties {
                name: "BRICK MASONRY",
                max_health: 40.0,
                hardness: 2.0,
                toughness: 1.8,
                blast_threshold: 4.0,
                density: 1.8,
                thermal_soften: 0.8,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 6000.0,
                tensile_capacity: 400.0,
            },
            VoxelType::Glass => MaterialProperties {
                name: "CRYSTALLINE SILICA GLASS",
                max_health: 12.0,
                hardness: 1.2,
                toughness: 0.4,
                blast_threshold: 0.0,
                density: 0.8,
                thermal_soften: 2.0,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 1500.0,
                tensile_capacity: 100.0,
            },
            VoxelType::Explosive => MaterialProperties {
                name: "VOLATILE FUEL CONDUIT",
                max_health: 30.0,
                hardness: 1.8,
                toughness: 1.2,
                blast_threshold: 3.0,
                density: 1.6,
                thermal_soften: 2.5,
                is_emissive: true,
                conductivity: 0.0,
                compressive_capacity: 2000.0,
                tensile_capacity: 200.0,
            },
            VoxelType::Bedrock => MaterialProperties {
                name: "REINFORCED GRANITE BEDROCK",
                max_health: 800.0,
                hardness: 9.8,
                toughness: 9.5,
                blast_threshold: 45.0,
                density: 10.5,
                thermal_soften: 0.4,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 1000000.0,
                tensile_capacity: 1000000.0,
            },
            VoxelType::Basalt => MaterialProperties {
                name: "SUBTERRANEAN BASALT SUBSTRATE",
                max_health: 500.0,
                hardness: 9.2,
                toughness: 8.8,
                blast_threshold: 32.0,
                density: 8.8,
                thermal_soften: 0.6,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 800000.0,
                tensile_capacity: 800000.0,
            },
            VoxelType::CryoIce => MaterialProperties {
                name: "SUPERCOOLED CRYO-ICE",
                max_health: 14.0,
                hardness: 1.0,
                toughness: 0.3,
                blast_threshold: 0.0,
                density: 0.9,
                thermal_soften: 3.0,
                is_emissive: false,
                conductivity: 0.0,
                compressive_capacity: 1000.0,
                tensile_capacity: 80.0,
            },
            VoxelType::GlowCore => MaterialProperties {
                name: "QUANTUM PLASMA CORE",
                max_health: 120.0,
                hardness: 5.0,
                toughness: 6.0,
                blast_threshold: 12.0,
                density: 3.8,
                thermal_soften: 0.8,
                is_emissive: true,
                conductivity: 6.0,
                compressive_capacity: 15000.0,
                tensile_capacity: 5000.0,
            },
            VoxelType::PowerCapacitor => MaterialProperties {
                name: "HIGH-VOLTAGE POWER CAPACITOR",
                max_health: 65.0,
                hardness: 2.8,
                toughness: 2.2,
                blast_threshold: 6.0,
                density: 3.2,
                thermal_soften: 1.8,
                is_emissive: true,
                conductivity: 12.0,
                compressive_capacity: 8000.0,
                tensile_capacity: 1200.0,
            },
        }
    }

    pub fn color(&self) -> Color {
        match self {
            VoxelType::Air => BLANK,
            VoxelType::Concrete => Color::new(0.72, 0.74, 0.77, 1.0), // Architectural cast concrete
            VoxelType::SteelRebar => Color::new(0.92, 0.46, 0.12, 1.0), // Vibrant construction hazard orange rebar skeleton
            VoxelType::HeavyArmor => Color::new(0.16, 0.18, 0.22, 1.0), // Deep matte tungsten gunmetal
            VoxelType::CompositeRebar => Color::new(0.10, 0.78, 0.88, 1.0), // Electric cyan carbon-fiber truss
            VoxelType::Brick => Color::new(0.78, 0.28, 0.18, 1.0), // Terracotta masonry
            VoxelType::Glass => Color::new(0.35, 0.88, 0.98, 0.55), // Crystalline bright cyan glass
            VoxelType::Explosive => Color::new(1.0, 0.22, 0.10, 1.0), // Warning hazard red/orange canister
            VoxelType::Bedrock => Color::new(0.12, 0.14, 0.17, 1.0), // Charcoal granite bedrock foundation
            VoxelType::Basalt => Color::new(0.20, 0.22, 0.25, 1.0),  // Volcanic basalt substrate
            VoxelType::CryoIce => Color::new(0.65, 0.94, 1.0, 0.85), // Supercooled cryo-ice
            VoxelType::GlowCore => Color::new(1.0, 0.90, 0.24, 1.0), // Radiant fusion plasma core
            VoxelType::PowerCapacitor => Color::new(0.68, 0.28, 0.98, 1.0), // High-voltage ionized violet with cyan shimmer
        }
    }

    #[allow(dead_code)]
    pub fn durability(&self) -> f32 {
        self.properties().hardness + self.properties().toughness
    }
}

#[allow(dead_code)]
pub struct RayHit {
    pub voxel_pos: IVec3,
    pub world_pos: Vec3,
    pub normal: Vec3,
    pub voxel_type: VoxelType,
    pub distance: f32,
    pub health_ratio: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CarveResult {
    pub destroyed: usize,
    pub rebar_destroyed: usize,
    pub armor_destroyed: usize,
    pub ice_shattered: usize,
    pub explosive_triggers: Vec<IVec3>,
    pub glow_core_triggers: Vec<IVec3>,
    pub capacitor_triggers: Vec<IVec3>,
    pub severed_rebar_positions: Vec<Vec3>,
    pub spalled_positions: Vec<(Vec3, Color)>,
}

impl CarveResult {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            destroyed: 0,
            rebar_destroyed: 0,
            armor_destroyed: 0,
            ice_shattered: 0,
            explosive_triggers: Vec::with_capacity(cap / 4),
            glow_core_triggers: Vec::with_capacity(cap / 4),
            capacitor_triggers: Vec::with_capacity(cap / 4),
            severed_rebar_positions: Vec::with_capacity(cap / 2),
            spalled_positions: Vec::with_capacity(cap),
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.destroyed = 0;
        self.rebar_destroyed = 0;
        self.armor_destroyed = 0;
        self.ice_shattered = 0;
        self.explosive_triggers.clear();
        self.glow_core_triggers.clear();
        self.capacitor_triggers.clear();
        self.severed_rebar_positions.clear();
        self.spalled_positions.clear();
    }
}

pub struct IslandChunk {
    pub voxels: Vec<(IVec3, VoxelType)>,
    pub voxel_health: Vec<f32>,
    pub center_of_mass: Vec3,
}

impl IslandChunk {
    pub fn calculate_mass(&self) -> f32 {
        let mut total = 0.0f32;
        for (_, vt) in &self.voxels {
            total += vt.properties().density;
        }
        total.max(1.0)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FrustumPlane {
    pub normal: Vec3,
    pub distance: f32,
}

impl FrustumPlane {
    #[inline(always)]
    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        self.normal.dot(p) + self.distance
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FrustumPlanes {
    pub planes: [FrustumPlane; 6],
}

#[allow(dead_code)]
pub type Frustum = FrustumPlanes;

impl FrustumPlanes {
    pub fn from_camera(
        cam_pos: Vec3,
        cam_dir: Vec3,
        cam_up: Vec3,
        aspect: f32,
        fov_y: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let fwd = cam_dir.normalize();
        let right = fwd.cross(cam_up).normalize();
        let up = right.cross(fwd).normalize();

        let half_v = (fov_y * 0.5).tan();
        let half_h = half_v * aspect;

        let near_plane = FrustumPlane {
            normal: fwd,
            distance: -fwd.dot(cam_pos + fwd * near),
        };
        let far_plane = FrustumPlane {
            normal: -fwd,
            distance: -(-fwd).dot(cam_pos + fwd * far),
        };

        let left_norm = (fwd * half_h + right).normalize();
        let left_plane = FrustumPlane {
            normal: left_norm,
            distance: -left_norm.dot(cam_pos),
        };

        let right_norm = (fwd * half_h - right).normalize();
        let right_plane = FrustumPlane {
            normal: right_norm,
            distance: -right_norm.dot(cam_pos),
        };

        let top_norm = (fwd * half_v - up).normalize();
        let top_plane = FrustumPlane {
            normal: top_norm,
            distance: -top_norm.dot(cam_pos),
        };

        let bottom_norm = (fwd * half_v + up).normalize();
        let bottom_plane = FrustumPlane {
            normal: bottom_norm,
            distance: -bottom_norm.dot(cam_pos),
        };

        Self {
            planes: [near_plane, far_plane, left_plane, right_plane, top_plane, bottom_plane],
        }
    }

    #[allow(dead_code)]
    pub fn from_camera3d(cam: &Camera3D, aspect: f32) -> Self {
        let fwd = (cam.target - cam.position).normalize();
        let near = 0.1;
        let far = if cam.z_far > 0.0 { cam.z_far } else { 380.0 };
        Self::from_camera(
            cam.position,
            fwd,
            cam.up,
            cam.aspect.unwrap_or(aspect),
            cam.fovy,
            near,
            far,
        )
    }

    #[allow(dead_code)]
    pub fn from_view_proj(vp: Mat4) -> Self {
        let row0 = Vec4::new(vp.x_axis.x, vp.y_axis.x, vp.z_axis.x, vp.w_axis.x);
        let row1 = Vec4::new(vp.x_axis.y, vp.y_axis.y, vp.z_axis.y, vp.w_axis.y);
        let row2 = Vec4::new(vp.x_axis.z, vp.y_axis.z, vp.z_axis.z, vp.w_axis.z);
        let row3 = Vec4::new(vp.x_axis.w, vp.y_axis.w, vp.z_axis.w, vp.w_axis.w);

        let make_plane = |v: Vec4| -> FrustumPlane {
            let normal = Vec3::new(v.x, v.y, v.z);
            let len = normal.length();
            if len > 1e-6 {
                FrustumPlane {
                    normal: normal / len,
                    distance: v.w / len,
                }
            } else {
                FrustumPlane {
                    normal: Vec3::Y,
                    distance: 0.0,
                }
            }
        };

        let left = make_plane(row3 + row0);
        let right = make_plane(row3 - row0);
        let bottom = make_plane(row3 + row1);
        let top = make_plane(row3 - row1);
        let near = make_plane(row3 + row2);
        let far = make_plane(row3 - row2);

        Self {
            planes: [near, far, left, right, top, bottom],
        }
    }

    #[inline(always)]
    pub fn is_aabb_in_frustum(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let p = Vec3::new(
                if plane.normal.x >= 0.0 { max.x } else { min.x },
                if plane.normal.y >= 0.0 { max.y } else { min.y },
                if plane.normal.z >= 0.0 { max.z } else { min.z },
            );
            if plane.distance_to_point(p) < 0.0 {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn contains_aabb(&self, min: Vec3, max: Vec3) -> bool {
        self.is_aabb_in_frustum(min, max)
    }

    #[inline(always)]
    pub fn is_sphere_in_frustum(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(center) < -radius {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.is_sphere_in_frustum(center, radius)
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn contains_point(&self, p: Vec3) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(p) < 0.0 {
                return false;
            }
        }
        true
    }
}

pub struct ChunkMesh {
    pub sub_meshes: Vec<Mesh>,
    pub dirty: bool,
}

pub struct VoxelGrid {
    pub voxels: Vec<VoxelType>,
    pub health: Vec<f32>,
    pub meshes: Vec<ChunkMesh>,
    pub total_initial_voxels: usize,
    pub current_voxels: usize,
    pub solid_count: usize,
    pub structural_dirty: bool,
    pub severed_rebar_event: usize,
    pub severed_rebar_positions: Vec<Vec3>,
    pub initial_rebar_count: usize,
    pub current_rebar_count: usize,
    pub spall_events: Vec<(Vec3, Color)>,
    pub scratch_vertices: Vec<Vertex>,
    pub scratch_indices: Vec<u16>,
    pub chunk_solid_count: Vec<u16>,

    // Milestone 1 Feature 1: Reusable pre-allocated scratch buffers for zero-allocation island BFS
    pub scratch_grounded: Vec<bool>,
    pub scratch_tension: Vec<bool>,
    pub scratch_depth: Vec<u8>,
    pub scratch_visited: Vec<bool>,
    pub scratch_queue: VecDeque<(usize, i32, usize)>,
    /// Per-(x,z) column: highest y such that every voxel from MIN_VOXEL_Y..=y is foundation rock.
    pub scratch_col_top: Vec<i32>,
    pub preserve_single_voxel_debris: bool,
    pub max_solid_y: i32,
    pub has_dirty_meshes: bool,
    pub dirty_chunks: Vec<usize>,
    pub light_mesh: std::cell::RefCell<Mesh>,

    // Feature 15: Localized sub-region dirty bounds tracking
    pub dirty_min: Option<IVec3>,
    pub dirty_max: Option<IVec3>,

    // Feature 17: Zero-allocation chunk meshing pools and scratch vectors
    pub scratch_dirty_chunks: Vec<usize>,
    pub scratch_partition_ranges: Vec<(std::ops::RangeInclusive<usize>, std::ops::Range<usize>)>,
    pub mesh_pool: Vec<Mesh>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum FaceDir {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

impl FaceDir {
    #[inline(always)]
    fn normal(&self) -> (i32, i32, i32) {
        match self {
            FaceDir::PosX => (1, 0, 0),
            FaceDir::NegX => (-1, 0, 0),
            FaceDir::PosY => (0, 1, 0),
            FaceDir::NegY => (0, -1, 0),
            FaceDir::PosZ => (0, 0, 1),
            FaceDir::NegZ => (0, 0, -1),
        }
    }

    #[inline(always)]
    fn shade(&self) -> f32 {
        match self {
            FaceDir::PosX => 0.86,
            FaceDir::NegX => 0.74,
            FaceDir::PosY => 1.05,
            FaceDir::NegY => 0.46,
            FaceDir::PosZ => 0.92,
            FaceDir::NegZ => 0.64,
        }
    }

    #[inline(always)]
    fn offsets(&self) -> [Vec3; 4] {
        match self {
            FaceDir::PosX => [Vec3::new(0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.5, -0.5, 0.5)],
            FaceDir::NegX => [Vec3::new(-0.5, -0.5, 0.5), Vec3::new(-0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(-0.5, -0.5, -0.5)],
            FaceDir::PosY => [Vec3::new(-0.5, 0.5, -0.5), Vec3::new(-0.5, 0.5, 0.5), Vec3::new(0.5, 0.5, 0.5), Vec3::new(0.5, 0.5, -0.5)],
            FaceDir::NegY => [Vec3::new(-0.5, -0.5, 0.5), Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, -0.5, -0.5), Vec3::new(0.5, -0.5, 0.5)],
            FaceDir::PosZ => [Vec3::new(0.5, -0.5, 0.5), Vec3::new(0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, 0.5), Vec3::new(-0.5, -0.5, 0.5)],
            FaceDir::NegZ => [Vec3::new(-0.5, -0.5, -0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, -0.5), Vec3::new(0.5, -0.5, -0.5)],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CellState {
    Empty,
    Damaged,
    NonUniformPristine,
    Mergeable(VoxelType, u8),
}

#[derive(Copy, Clone, Debug)]
struct QuadSpan {
    fd: FaceDir,
    slice: i32,
    u0: usize,
    v0: usize,
    span_u: usize,
    span_v: usize,
}

impl Default for VoxelGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelGrid {
    pub fn new() -> Self {
        let total_voxels = GRID_X * GRID_Y * GRID_Z;
        let mut voxels = vec![VoxelType::Air; total_voxels];
        let mut health = vec![0.0f32; total_voxels];
        let num_chunks = CHUNKS_X * CHUNKS_Y * CHUNKS_Z;
        let mut chunk_solid_count = vec![0u16; num_chunks];

        // Subterranean Bedrock & Basalt layers:
        // Layers [-48..=-41] (Chunk 0): Reinforced Granite Bedrock (health 800.0)
        // Layers [-40..=-1] (Chunks 1..5): Subterranean Basalt Substrate (health 500.0)
        // Layer 0: Reinforced Granite Bedrock slab (health 800.0)
        for x in 0..GRID_X {
            for z in 0..GRID_Z {
                // Bedrock deep foundation
                for y in MIN_VOXEL_Y..=-41 {
                    let i = Self::idx(x, y, z);
                    voxels[i] = VoxelType::Bedrock;
                    health[i] = 800.0;
                    let internal_y = (y - MIN_VOXEL_Y) as usize;
                    let ci = Self::chunk_idx(x / CHUNK_SIZE, internal_y / CHUNK_SIZE, z / CHUNK_SIZE);
                    chunk_solid_count[ci] = chunk_solid_count[ci].saturating_add(1);
                }
                // Basalt substrate
                for y in -40..=-1 {
                    let i = Self::idx(x, y, z);
                    voxels[i] = VoxelType::Basalt;
                    health[i] = 500.0;
                    let internal_y = (y - MIN_VOXEL_Y) as usize;
                    let ci = Self::chunk_idx(x / CHUNK_SIZE, internal_y / CHUNK_SIZE, z / CHUNK_SIZE);
                    chunk_solid_count[ci] = chunk_solid_count[ci].saturating_add(1);
                }
                // Surface Bedrock slab
                let i0 = Self::idx(x, 0, z);
                voxels[i0] = VoxelType::Bedrock;
                health[i0] = 800.0;
                let internal_y = (0 - MIN_VOXEL_Y) as usize;
                let ci = Self::chunk_idx(x / CHUNK_SIZE, internal_y / CHUNK_SIZE, z / CHUNK_SIZE);
                chunk_solid_count[ci] = chunk_solid_count[ci].saturating_add(1);
            }
        }

        let mut meshes = Vec::with_capacity(num_chunks);
        for _ in 0..num_chunks {
            meshes.push(ChunkMesh {
                sub_meshes: Vec::new(),
                dirty: true,
            });
        }

        Self {
            voxels,
            health,
            meshes,
            total_initial_voxels: 0,
            current_voxels: 0,
            solid_count: 0,
            structural_dirty: false,
            severed_rebar_event: 0,
            severed_rebar_positions: Vec::with_capacity(64),
            initial_rebar_count: 0,
            current_rebar_count: 0,
            spall_events: Vec::with_capacity(32),
            scratch_vertices: Vec::with_capacity(8192),
            scratch_indices: Vec::with_capacity(12288),
            chunk_solid_count,

            // Milestone 1 Feature 1: Pre-allocated scratch buffers
            scratch_grounded: vec![false; total_voxels],
            scratch_tension: vec![false; total_voxels],
            scratch_depth: vec![255u8; total_voxels],
            scratch_visited: vec![false; total_voxels],
            scratch_queue: VecDeque::with_capacity(32768),
            scratch_col_top: vec![MIN_VOXEL_Y - 1; GRID_X * GRID_Z],
            preserve_single_voxel_debris: false,
            max_solid_y: 0,
            has_dirty_meshes: true,
            dirty_chunks: Vec::with_capacity(64),
            light_mesh: std::cell::RefCell::new(Mesh {
                vertices: Vec::with_capacity(2048),
                indices: Vec::with_capacity(3072),
                texture: None,
            }),
            dirty_min: None,
            dirty_max: None,
            scratch_dirty_chunks: Vec::with_capacity(64),
            scratch_partition_ranges: Vec::with_capacity(16),
            mesh_pool: Vec::with_capacity(32),
        }
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn set_preserve_single_voxel_debris(&mut self, preserve: bool) {
        self.preserve_single_voxel_debris = preserve;
    }

    #[inline(always)]
    pub fn solid_count(&self) -> usize {
        self.solid_count
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.chunk_solid_count.fill(0);
        for x in 0..GRID_X {
            // Match storage order (z is contiguous) instead of striding through every
            // vertical column. Each row has one material and can be filled as a slice.
            for y in MIN_VOXEL_Y..MAX_VOXEL_Y {
                let (voxel, health) = if y <= -41 || y == 0 {
                    (VoxelType::Bedrock, 800.0)
                } else if y < 0 {
                    (VoxelType::Basalt, 500.0)
                } else {
                    (VoxelType::Air, 0.0)
                };
                let start = Self::idx(x, y, 0);
                self.voxels[start..start + GRID_Z].fill(voxel);
                self.health[start..start + GRID_Z].fill(health);
                if voxel.is_solid() {
                    let cy = ((y - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
                    for cz in 0..CHUNKS_Z {
                        let ci = Self::chunk_idx(x / CHUNK_SIZE, cy, cz);
                        self.chunk_solid_count[ci] += CHUNK_SIZE as u16;
                    }
                }
            }
        }
        self.solid_count = 0;
        self.current_voxels = 0;
        self.total_initial_voxels = 0;
        self.current_rebar_count = 0;
        self.initial_rebar_count = 0;
        self.max_solid_y = 0;
        self.has_dirty_meshes = true;
        self.dirty_chunks.clear();
        self.structural_dirty = false;
        self.dirty_min = None;
        self.dirty_max = None;
        self.severed_rebar_event = 0;
        self.severed_rebar_positions.clear();
        self.spall_events.clear();
        for mesh in &mut self.meshes {
            mesh.dirty = true;
        }
    }

    /// Mark a rectangular bounding box dirty for localized structural integrity BFS
    pub fn mark_dirty_region(&mut self, min: IVec3, max: IVec3) {
        let clamped_min = IVec3::new(
            min.x.clamp(0, GRID_X as i32 - 1),
            min.y.clamp(MIN_VOXEL_Y, MAX_VOXEL_Y - 1),
            min.z.clamp(0, GRID_Z as i32 - 1),
        );
        let clamped_max = IVec3::new(
            max.x.clamp(0, GRID_X as i32 - 1),
            max.y.clamp(MIN_VOXEL_Y, MAX_VOXEL_Y - 1),
            max.z.clamp(0, GRID_Z as i32 - 1),
        );
        if clamped_min.x > clamped_max.x || clamped_min.y > clamped_max.y || clamped_min.z > clamped_max.z {
            return;
        }
        self.dirty_min = Some(match self.dirty_min {
            Some(cur) => cur.min(clamped_min),
            None => clamped_min,
        });
        self.dirty_max = Some(match self.dirty_max {
            Some(cur) => cur.max(clamped_max),
            None => clamped_max,
        });
        self.structural_dirty = true;
    }

    /// Mark a single voxel coordinate dirty
    #[inline(always)]
    pub fn mark_dirty_point(&mut self, pt: IVec3) {
        self.mark_dirty_region(pt, pt);
    }

    #[inline(always)]
    pub fn idx(x: usize, y: i32, z: usize) -> usize {
        let internal_y = (y - MIN_VOXEL_Y) as usize;
        (x * GRID_Y + internal_y) * GRID_Z + z
    }

    #[inline(always)]
    pub fn chunk_idx(cx: usize, cy: usize, cz: usize) -> usize {
        (cx * CHUNKS_Y + cy) * CHUNKS_Z + cz
    }

    #[inline(always)]
    pub fn in_bounds(&self, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && x < GRID_X as i32 && (MIN_VOXEL_Y..MAX_VOXEL_Y).contains(&y) && z >= 0 && z < GRID_Z as i32
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> VoxelType {
        if self.in_bounds(x, y, z) {
            self.voxels[Self::idx(x as usize, y, z as usize)]
        } else {
            VoxelType::Air
        }
    }

    pub fn is_surface_voxel(&self, x: i32, y: i32, z: i32) -> bool {
        let neighbors = [
            (1, 0, 0), (-1, 0, 0),
            (0, 1, 0), (0, -1, 0),
            (0, 0, 1), (0, 0, -1),
        ];
        for (dx, dy, dz) in neighbors {
            let nx = x + dx;
            let ny = y + dy;
            let nz = z + dz;
            if !self.in_bounds(nx, ny, nz) || !self.get(nx, ny, nz).is_solid() {
                return true;
            }
        }
        false
    }

    pub fn get_health(&self, x: i32, y: i32, z: i32) -> f32 {
        if self.in_bounds(x, y, z) {
            self.health[Self::idx(x as usize, y, z as usize)]
        } else {
            0.0
        }
    }

    pub fn get_health_ratio(&self, x: i32, y: i32, z: i32) -> f32 {
        if self.in_bounds(x, y, z) {
            let vt = self.get(x, y, z);
            let max_h = vt.properties().max_health;
            if max_h.is_finite() && max_h > 0.0 {
                (self.get_health(x, y, z) / max_h).clamp(0.0, 1.0)
            } else {
                1.0
            }
        } else {
            0.0
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, vtype: VoxelType) {
        if self.in_bounds(x, y, z) {
            let i = Self::idx(x as usize, y, z as usize);
            let prev = self.voxels[i];
            if prev != vtype {
                let prev_solid = prev.is_solid();
                let new_solid = vtype.is_solid();
                let internal_y = (y - MIN_VOXEL_Y) as usize;
                let cx = (x as usize) / CHUNK_SIZE;
                let cy = internal_y / CHUNK_SIZE;
                let cz = (z as usize) / CHUNK_SIZE;
                let ci = Self::chunk_idx(cx, cy, cz);
                if !prev_solid && new_solid {
                    self.chunk_solid_count[ci] = self.chunk_solid_count[ci].saturating_add(1);
                    if y > 0 {
                        self.solid_count += 1;
                    }
                    if y > self.max_solid_y {
                        self.max_solid_y = y;
                    }
                } else if prev_solid && !new_solid {
                    self.chunk_solid_count[ci] = self.chunk_solid_count[ci].saturating_sub(1);
                    if y > 0 {
                        self.solid_count = self.solid_count.saturating_sub(1);
                    }
                }
                self.current_voxels = self.solid_count;
                self.voxels[i] = vtype;
                self.health[i] = vtype.properties().max_health;
                self.mark_chunk_dirty(x, y, z);
                self.mark_dirty_point(IVec3::new(x, y, z));
            }
        }
    }

    #[inline]
    pub fn mark_chunk_self_dirty(&mut self, x: i32, y: i32, z: i32) {
        if !self.in_bounds(x, y, z) {
            return;
        }
        let cx = (x as usize) / CHUNK_SIZE;
        let internal_y = (y - MIN_VOXEL_Y) as usize;
        let cy = internal_y / CHUNK_SIZE;
        let cz = (z as usize) / CHUNK_SIZE;
        if cx < CHUNKS_X && cy < CHUNKS_Y && cz < CHUNKS_Z {
            let ci = Self::chunk_idx(cx, cy, cz);
            if !self.meshes[ci].dirty {
                self.meshes[ci].dirty = true;
                self.dirty_chunks.push(ci);
            }
            self.has_dirty_meshes = true;
        }
    }

    pub fn mark_chunk_dirty(&mut self, x: i32, y: i32, z: i32) {
        self.mark_chunk_self_dirty(x, y, z);
        if !self.in_bounds(x, y, z) {
            return;
        }
        let cx = (x as usize) / CHUNK_SIZE;
        let internal_y = (y - MIN_VOXEL_Y) as usize;
        let cy = internal_y / CHUNK_SIZE;
        let cz = (z as usize) / CHUNK_SIZE;
        if cx >= CHUNKS_X || cy >= CHUNKS_Y || cz >= CHUNKS_Z {
            return;
        }

        // Only mark neighbor chunks dirty if neighbor voxel in that chunk is actually solid!
        let lx = (x as usize) % CHUNK_SIZE;
        let ly = internal_y % CHUNK_SIZE;
        let lz = (z as usize) % CHUNK_SIZE;
        if lx == 0 && cx > 0 && self.get(x - 1, y, z).is_solid() {
            let nci = Self::chunk_idx(cx - 1, cy, cz);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
        if lx == CHUNK_SIZE - 1 && cx + 1 < CHUNKS_X && self.get(x + 1, y, z).is_solid() {
            let nci = Self::chunk_idx(cx + 1, cy, cz);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
        if ly == 0 && cy > 0 && self.get(x, y - 1, z).is_solid() {
            let nci = Self::chunk_idx(cx, cy - 1, cz);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
        if ly == CHUNK_SIZE - 1 && cy + 1 < CHUNKS_Y && self.get(x, y + 1, z).is_solid() {
            let nci = Self::chunk_idx(cx, cy + 1, cz);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
        if lz == 0 && cz > 0 && self.get(x, y, z - 1).is_solid() {
            let nci = Self::chunk_idx(cx, cy, cz - 1);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
        if lz == CHUNK_SIZE - 1 && cz + 1 < CHUNKS_Z && self.get(x, y, z + 1).is_solid() {
            let nci = Self::chunk_idx(cx, cy, cz + 1);
            if !self.meshes[nci].dirty {
                self.meshes[nci].dirty = true;
                self.dirty_chunks.push(nci);
            }
        }
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub fn chunk_has_solid(&self, cx: usize, cy: usize, cz: usize) -> bool {
        if cx < CHUNKS_X && cy < CHUNKS_Y && cz < CHUNKS_Z {
            self.chunk_solid_count[Self::chunk_idx(cx, cy, cz)] > 0
        } else {
            false
        }
    }

    pub fn region_has_solids(&self, min_x: i32, max_x: i32, min_y: i32, max_y: i32, min_z: i32, max_z: i32) -> bool {
        if min_x > max_x || min_y > max_y || min_z > max_z {
            return false;
        }
        if min_y > self.max_solid_y || max_y < MIN_VOXEL_Y {
            return false;
        }
        if max_x < 0 || min_x >= GRID_X as i32 || max_z < 0 || min_z >= GRID_Z as i32 {
            return false;
        }
        let min_cx = (min_x.clamp(0, GRID_X as i32 - 1) as usize) / CHUNK_SIZE;
        let max_cx = (max_x.clamp(0, GRID_X as i32 - 1) as usize) / CHUNK_SIZE;
        let min_internal_y = (min_y.clamp(MIN_VOXEL_Y, MAX_VOXEL_Y - 1) - MIN_VOXEL_Y) as usize;
        let max_internal_y = (max_y.clamp(MIN_VOXEL_Y, MAX_VOXEL_Y - 1) - MIN_VOXEL_Y) as usize;
        let min_cy = min_internal_y / CHUNK_SIZE;
        let max_cy = max_internal_y / CHUNK_SIZE;
        let min_cz = (min_z.clamp(0, GRID_Z as i32 - 1) as usize) / CHUNK_SIZE;
        let max_cz = (max_z.clamp(0, GRID_Z as i32 - 1) as usize) / CHUNK_SIZE;

        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                for cz in min_cz..=max_cz {
                    if self.chunk_solid_count[Self::chunk_idx(cx, cy, cz)] > 0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn world_to_voxel(&self, pos: Vec3) -> IVec3 {
        let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let vx = ((pos.x + half_x) / VOXEL_SIZE).floor() as i32;
        let vy = (pos.y / VOXEL_SIZE).floor() as i32;
        let vz = ((pos.z + half_z) / VOXEL_SIZE).floor() as i32;
        IVec3::new(vx, vy, vz)
    }

    pub fn voxel_to_world(&self, vpos: IVec3) -> Vec3 {
        let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        Vec3::new(
            vpos.x as f32 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5,
            vpos.y as f32 * VOXEL_SIZE + VOXEL_SIZE * 0.5,
            vpos.z as f32 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5,
        )
    }

    pub fn find_nearest_solid(&self, pos: Vec3, search_radius: f32) -> Option<IVec3> {
        let center = self.world_to_voxel(pos);
        let r = (search_radius / VOXEL_SIZE).ceil().min(8.0) as i32;
        let mut closest = None;
        let mut min_dist_sq = f32::MAX;
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    let vx = center.x + dx;
                    let vy = center.y + dy;
                    let vz = center.z + dz;
                    if self.get(vx, vy, vz).is_solid() {
                        let wpos = self.voxel_to_world(IVec3::new(vx, vy, vz));
                        let dist_sq = wpos.distance_squared(pos);
                        if dist_sq < min_dist_sq {
                            min_dist_sq = dist_sq;
                            closest = Some(IVec3::new(vx, vy, vz));
                        }
                    }
                }
            }
        }
        closest
    }

    // Fast DDA 3D Raymarching
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
        let dir = dir.normalize_or_zero();
        if dir.length_squared() < 0.001 {
            return None;
        }

        let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let aabb_min = Vec3::new(-half_x, VOID_BOUNDARY_Y, -half_z);
        let effective_max_y = ((self.max_solid_y + 1) as f32 * VOXEL_SIZE).max(VOXEL_SIZE);
        let aabb_max = Vec3::new(half_x, effective_max_y, half_z);

        // Fast AABB intersection test: prune rays that miss or point away from the active world volume
        let mut t_near = 0.0f32;
        let mut t_far = max_dist;

        if dir.x.abs() > 1e-6 {
            let inv_d = 1.0 / dir.x;
            let mut t1 = (aabb_min.x - origin.x) * inv_d;
            let mut t2 = (aabb_max.x - origin.x) * inv_d;
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_near = t_near.max(t1);
            t_far = t_far.min(t2);
            if t_near > t_far { return None; }
        } else if origin.x < aabb_min.x || origin.x > aabb_max.x {
            return None;
        }

        if dir.y.abs() > 1e-6 {
            let inv_d = 1.0 / dir.y;
            let mut t1 = (aabb_min.y - origin.y) * inv_d;
            let mut t2 = (aabb_max.y - origin.y) * inv_d;
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_near = t_near.max(t1);
            t_far = t_far.min(t2);
            if t_near > t_far { return None; }
        } else if origin.y < aabb_min.y || origin.y > aabb_max.y {
            return None;
        }

        if dir.z.abs() > 1e-6 {
            let inv_d = 1.0 / dir.z;
            let mut t1 = (aabb_min.z - origin.z) * inv_d;
            let mut t2 = (aabb_max.z - origin.z) * inv_d;
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_near = t_near.max(t1);
            t_far = t_far.min(t2);
            if t_near > t_far { return None; }
        } else if origin.z < aabb_min.z || origin.z > aabb_max.z {
            return None;
        }

        if t_far < 0.0 || t_near > max_dist {
            return None;
        }

        // Transform origin into voxel coordinates (continuous)
        let ox = (origin.x + half_x) / VOXEL_SIZE;
        let oy = origin.y / VOXEL_SIZE;
        let oz = (origin.z + half_z) / VOXEL_SIZE;

        let mut map_x = ox.floor() as i32;
        let mut map_y = oy.floor() as i32;
        let mut map_z = oz.floor() as i32;

        let step_x = if dir.x > 0.0 { 1 } else { -1 };
        let step_y = if dir.y > 0.0 { 1 } else { -1 };
        let step_z = if dir.z > 0.0 { 1 } else { -1 };

        let delta_dist_x = if dir.x != 0.0 { (1.0 / dir.x).abs() } else { 1e30 };
        let delta_dist_y = if dir.y != 0.0 { (1.0 / dir.y).abs() } else { 1e30 };
        let delta_dist_z = if dir.z != 0.0 { (1.0 / dir.z).abs() } else { 1e30 };

        let mut side_dist_x = if dir.x > 0.0 {
            (map_x as f32 + 1.0 - ox) * delta_dist_x
        } else {
            (ox - map_x as f32) * delta_dist_x
        };
        let mut side_dist_y = if dir.y > 0.0 {
            (map_y as f32 + 1.0 - oy) * delta_dist_y
        } else {
            (oy - map_y as f32) * delta_dist_y
        };
        let mut side_dist_z = if dir.z > 0.0 {
            (map_z as f32 + 1.0 - oz) * delta_dist_z
        } else {
            (oz - map_z as f32) * delta_dist_z
        };

        let mut last_normal;
        let mut traveled_voxels = 0.0;
        let max_voxels = t_far / VOXEL_SIZE;

        // Check if starting inside a solid voxel
        if self.in_bounds(map_x, map_y, map_z) {
            let vt = self.get(map_x, map_y, map_z);
            if vt.is_solid() {
                let vpos = IVec3::new(map_x, map_y, map_z);
                return Some(RayHit {
                    voxel_pos: vpos,
                    world_pos: origin,
                    normal: -dir,
                    voxel_type: vt,
                    distance: 0.0,
                    health_ratio: self.get_health_ratio(map_x, map_y, map_z),
                });
            }
        }

        while traveled_voxels <= max_voxels {
            // Early out if ray has stepped outside bounds and moves further away
            if (map_x < 0 && step_x < 0)
                || (map_x >= GRID_X as i32 && step_x > 0)
                || (map_y < MIN_VOXEL_Y && step_y < 0)
                || (map_y >= MAX_VOXEL_Y && step_y > 0)
                || (map_z < 0 && step_z < 0)
                || (map_z >= GRID_Z as i32 && step_z > 0)
            {
                break;
            }

            if side_dist_x < side_dist_y && side_dist_x < side_dist_z {
                traveled_voxels = side_dist_x;
                side_dist_x += delta_dist_x;
                map_x += step_x;
                last_normal = Vec3::new(-step_x as f32, 0.0, 0.0);
            } else if side_dist_y < side_dist_z {
                traveled_voxels = side_dist_y;
                side_dist_y += delta_dist_y;
                map_y += step_y;
                last_normal = Vec3::new(0.0, -step_y as f32, 0.0);
            } else {
                traveled_voxels = side_dist_z;
                side_dist_z += delta_dist_z;
                map_z += step_z;
                last_normal = Vec3::new(0.0, 0.0, -step_z as f32);
            }

            // The next cell may begin beyond the endpoint even though the previous cell was
            // in range. Check the new entry distance before reporting a surface contact.
            if traveled_voxels > max_voxels {
                break;
            }

            if self.in_bounds(map_x, map_y, map_z) {
                let vt = self.get(map_x, map_y, map_z);
                if vt.is_solid() {
                    let vpos = IVec3::new(map_x, map_y, map_z);
                    let hit_dist = traveled_voxels * VOXEL_SIZE;
                    let surface_contact = origin + dir * hit_dist;
                    return Some(RayHit {
                        voxel_pos: vpos,
                        world_pos: surface_contact,
                        normal: last_normal,
                        voxel_type: vt,
                        distance: hit_dist,
                        health_ratio: self.get_health_ratio(map_x, map_y, map_z),
                    });
                }
            }
        }

        None
    }

    /// Damage a single voxel taking into account hardness, toughness, blast thresholds, and thermal softening.
    /// Returns true if the voxel was destroyed.
    pub fn damage_voxel(&mut self, x: i32, y: i32, z: i32, raw_damage: f32, dtype: DamageType) -> bool {
        if !self.in_bounds(x, y, z) {
            return false;
        }
        let idx = Self::idx(x as usize, y, z as usize);
        let vt = self.voxels[idx];
        if !vt.is_destructible() {
            return false;
        }
        let props = vt.properties();

        // Check blast deflection threshold for Explosive & Concussive impacts
        if (dtype == DamageType::Explosive || dtype == DamageType::Concussive) && raw_damage < props.blast_threshold {
            if self.spall_events.len() < 32 && self.is_surface_voxel(x, y, z) {
                let wp = self.voxel_to_world(IVec3::new(x, y, z));
                self.spall_events.push((wp, vt.color()));
            }
            return false;
        }

        let effective_damage = vt.effective_damage(raw_damage, dtype);

        let cur = self.health[idx];
        if effective_damage >= cur {
            // Destroyed!
            self.health[idx] = 0.0;
            self.set(x, y, z, VoxelType::Air);
            true
        } else {
            let was_pristine = (cur - props.max_health).abs() < 0.001;
            let old_step = (cur / props.max_health * 8.0).floor() as i32;
            let new_cur = cur - effective_damage;
            let new_step = (new_cur / props.max_health * 8.0).floor() as i32;
            self.health[idx] = new_cur;
            if was_pristine || old_step != new_step {
                self.mark_chunk_self_dirty(x, y, z);
            }
            if self.spall_events.len() < 32 && self.is_surface_voxel(x, y, z) {
                let wp = self.voxel_to_world(IVec3::new(x, y, z));
                self.spall_events.push((wp, vt.color()));
            }
            false
        }
    }

    /// Carve a spherical crater directly into a caller-supplied result buffer (zero allocations)
    pub fn carve_sphere_advanced_into(
        &mut self,
        center: Vec3,
        radius: f32,
        base_damage: f32,
        dtype: DamageType,
        out: &mut CarveResult,
    ) {
        let vcenter = self.world_to_voxel(center);
        // Brittle materials (CryoIce, Glass) shatter under shockwaves beyond primary blast
        let brittle_r = radius * 1.35;
        let vrad = (brittle_r / VOXEL_SIZE).ceil() as i32;

        let min_x = (vcenter.x - vrad).max(0);
        let max_x = (vcenter.x + vrad).min(GRID_X as i32 - 1);
        let min_y = (vcenter.y - vrad).max(MIN_VOXEL_Y);
        let max_y = (vcenter.y + vrad).min(self.max_solid_y.min(MAX_VOXEL_Y - 1));
        let min_z = (vcenter.z - vrad).max(0);
        let max_z = (vcenter.z + vrad).min(GRID_Z as i32 - 1);

        if min_x > max_x || min_y > max_y || min_z > max_z {
            return;
        }

        if !self.region_has_solids(min_x, max_x, min_y, max_y, min_z, max_z) {
            return;
        }

        let r2 = (radius / VOXEL_SIZE).powi(2);
        let brittle_r2 = (brittle_r / VOXEL_SIZE).powi(2);
        let initial_destroyed = out.destroyed;
        let initial_rebar = out.rebar_destroyed;

        for x in min_x..=max_x {
            let dx2 = (x - vcenter.x).pow(2) as f32;
            for z in min_z..=max_z {
                let dz2 = (z - vcenter.z).pow(2) as f32;
                if dx2 + dz2 > brittle_r2 {
                    continue;
                }
                for y in min_y..=max_y {
                    let dy2 = (y - vcenter.y).pow(2) as f32;
                    let d2 = dx2 + dy2 + dz2;
                    if d2 <= brittle_r2 {
                        let vt = self.get(x, y, z);
                        if !vt.is_destructible() {
                            continue;
                        }

                        let dist_vox = d2.sqrt();
                        let max_r_vox = if vt == VoxelType::CryoIce || vt == VoxelType::Glass {
                            brittle_r / VOXEL_SIZE
                        } else if d2 <= r2 {
                            radius / VOXEL_SIZE
                        } else {
                            continue;
                        };

                        let falloff = (1.0 - (dist_vox / max_r_vox)).max(0.08);
                        let dmg = base_damage * falloff;

                        let destroyed = self.damage_voxel(x, y, z, dmg, dtype);
                        if destroyed {
                            if vt == VoxelType::Explosive {
                                out.explosive_triggers.push(IVec3::new(x, y, z));
                            } else if vt == VoxelType::GlowCore {
                                out.glow_core_triggers.push(IVec3::new(x, y, z));
                            } else if vt == VoxelType::PowerCapacitor {
                                out.capacitor_triggers.push(IVec3::new(x, y, z));
                            } else if vt == VoxelType::SteelRebar || vt == VoxelType::CompositeRebar {
                                out.rebar_destroyed += 1;
                                let world_p = self.voxel_to_world(IVec3::new(x, y, z));
                                out.severed_rebar_positions.push(world_p);
                                self.severed_rebar_positions.push(world_p);
                                self.current_rebar_count = self.current_rebar_count.saturating_sub(1);
                            } else if vt == VoxelType::HeavyArmor {
                                out.armor_destroyed += 1;
                            } else if vt == VoxelType::CryoIce {
                                out.ice_shattered += 1;
                            }
                            out.destroyed += 1;
                        } else if out.spalled_positions.len() < 32 {
                            let wp = self.voxel_to_world(IVec3::new(x, y, z));
                            out.spalled_positions.push((wp, vt.color()));
                        }
                    }
                }
            }
        }

        if out.destroyed > initial_destroyed {
            self.mark_dirty_region(
                IVec3::new(min_x, min_y, min_z),
                IVec3::new(max_x, max_y, max_z),
            );
        }
        if out.rebar_destroyed > initial_rebar {
            self.severed_rebar_event += out.rebar_destroyed - initial_rebar;
        }
    }

    /// Carve a spherical crater with custom damage energy and damage type
    pub fn carve_sphere_advanced(
        &mut self,
        center: Vec3,
        radius: f32,
        base_damage: f32,
        dtype: DamageType,
    ) -> CarveResult {
        let mut result = CarveResult::default();
        self.carve_sphere_advanced_into(center, radius, base_damage, dtype, &mut result);
        result
    }

    // Carve a spherical crater of damage with default explosive ordnance power
    pub fn carve_sphere(&mut self, center: Vec3, radius: f32) -> CarveResult {
        self.carve_sphere_advanced(center, radius, 250.0, DamageType::Explosive)
    }

    /// Carve a continuous cylinder along a beam directly into a target buffer
    pub fn carve_cylinder_advanced_into(
        &mut self,
        start: Vec3,
        end: Vec3,
        radius: f32,
        base_damage: f32,
        dtype: DamageType,
        out: &mut CarveResult,
    ) {
        let dir = end - start;
        let len = dir.length();
        if len < 0.01 {
            return;
        }

        let max_world_y = (self.max_solid_y as f32 + 1.5) * VOXEL_SIZE;
        if start.y - radius > max_world_y && end.y - radius > max_world_y {
            return; // Completely in empty sky above all structures
        }

        let half_x_world = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z_world = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let half_x = half_x_world + radius;
        let half_z = half_z_world + radius;
        if (start.x < -half_x && end.x < -half_x) || (start.x > half_x && end.x > half_x)
            || (start.z < -half_z && end.z < -half_z) || (start.z > half_z && end.z > half_z)
        {
            return; // Completely outside horizontal grid bounds
        }

        let min_gx = ((start.x.min(end.x) - radius + half_x_world) / VOXEL_SIZE).floor() as i32;
        let max_gx = ((start.x.max(end.x) + radius + half_x_world) / VOXEL_SIZE).ceil() as i32;
        let min_gy = ((start.y.min(end.y) - radius) / VOXEL_SIZE).floor() as i32;
        let max_gy = ((start.y.max(end.y) + radius) / VOXEL_SIZE).ceil() as i32;
        let min_gz = ((start.z.min(end.z) - radius + half_z_world) / VOXEL_SIZE).floor() as i32;
        let max_gz = ((start.z.max(end.z) + radius + half_z_world) / VOXEL_SIZE).ceil() as i32;
        if !self.region_has_solids(min_gx, max_gx, min_gy, max_gy, min_gz, max_gz) {
            return;
        }

        let udir = dir / len;
        let step_dist = (radius * 0.75).max(VOXEL_SIZE * 0.5);
        let steps = (len / step_dist).ceil() as usize;

        for s in 0..=steps {
            let t = (s as f32 / steps as f32) * len;
            let p = start + udir * t;
            if p.y - radius > max_world_y {
                continue;
            }
            self.carve_sphere_advanced_into(p, radius, base_damage, dtype, out);
        }
    }

    // Carve a cylinder along a beam with advanced damage type
    pub fn carve_cylinder_advanced(
        &mut self,
        start: Vec3,
        end: Vec3,
        radius: f32,
        base_damage: f32,
        dtype: DamageType,
    ) -> CarveResult {
        let mut total_result = CarveResult::with_capacity(32);
        self.carve_cylinder_advanced_into(start, end, radius, base_damage, dtype, &mut total_result);
        total_result
    }

    // Carve a cylinder along a beam (for Railgun or Plasma laser)
    #[allow(dead_code)]
    pub fn carve_cylinder(&mut self, start: Vec3, end: Vec3, radius: f32) -> CarveResult {
        self.carve_cylinder_advanced(start, end, radius, 140.0, DamageType::Kinetic)
    }

    /// Slices a planar kerf (slot) through standing structures using a cutting plane.
    /// Clears/damages voxels within blade kerf thickness intersecting the plane
    /// within `reach` in the forward hemisphere.
    pub fn slice_planar_kerf(
        &mut self,
        origin: Vec3,
        direction: Vec3,
        blade_normal: Vec3,
        reach: f32,
        kerf_thickness: f32,
    ) -> usize {
        let dir_norm = direction.normalize_or_zero();
        let blade_norm = blade_normal.normalize_or_zero();
        let plane_dist = -blade_norm.dot(origin);

        let v_origin = self.world_to_voxel(origin);
        let v_rad = (reach / VOXEL_SIZE).ceil() as i32;

        let min_x = (v_origin.x - v_rad).max(0);
        let max_x = (v_origin.x + v_rad).min(GRID_X as i32 - 1);
        let min_y = (v_origin.y - v_rad).max(MIN_VOXEL_Y);
        let max_y = (v_origin.y + v_rad).min(MAX_VOXEL_Y - 1);
        let min_z = (v_origin.z - v_rad).max(0);
        let max_z = (v_origin.z + v_rad).min(GRID_Z as i32 - 1);

        let mut cut_count = 0;
        let half_kerf = kerf_thickness * 0.5;

        for vx in min_x..=max_x {
            for vy in min_y..=max_y {
                for vz in min_z..=max_z {
                    let vt = self.get(vx, vy, vz);
                    if vt == VoxelType::Air || vt == VoxelType::Bedrock {
                        continue;
                    }

                    let wp = self.voxel_to_world(IVec3::new(vx, vy, vz));
                    let to_p = wp - origin;
                    let dist = to_p.length();
                    if dist > reach || dist < 0.1 {
                        continue;
                    }

                    if to_p.dot(dir_norm) < 0.0 {
                        continue;
                    }

                    let plane_d = (blade_norm.dot(wp) + plane_dist).abs();
                    if plane_d <= half_kerf {
                        self.damage_voxel(vx, vy, vz, 800.0, DamageType::Thermal);
                        cut_count += 1;
                    }
                }
            }
        }

        if cut_count > 0 {
            self.structural_dirty = true;
        }

        cut_count
    }

    // Freeze voxels in radius into brittle Ice
    pub fn freeze_sphere(&mut self, center: Vec3, radius: f32) -> usize {
        let vcenter = self.world_to_voxel(center);
        let vrad = (radius / VOXEL_SIZE).ceil() as i32;
        let mut frozen = 0;

        let min_x = (vcenter.x - vrad).max(0);
        let max_x = (vcenter.x + vrad).min(GRID_X as i32 - 1);
        let min_y = (vcenter.y - vrad).max(MIN_VOXEL_Y);
        let max_y = (vcenter.y + vrad).min(MAX_VOXEL_Y - 1);
        let min_z = (vcenter.z - vrad).max(0);
        let max_z = (vcenter.z + vrad).min(GRID_Z as i32 - 1);
        let r2 = (radius / VOXEL_SIZE).powi(2);

        for x in min_x..=max_x {
            let dx2 = (x - vcenter.x).pow(2) as f32;
            for z in min_z..=max_z {
                let dz2 = (z - vcenter.z).pow(2) as f32;
                for y in min_y..=max_y {
                    let dy2 = (y - vcenter.y).pow(2) as f32;
                    if dx2 + dy2 + dz2 <= r2 {
                        let vt = self.get(x, y, z);
                        if vt.is_destructible() && vt != VoxelType::CryoIce {
                            self.set(x, y, z, VoxelType::CryoIce);
                            frozen += 1;
                        }
                    }
                }
            }
        }

        if frozen > 0 {
            self.mark_dirty_region(
                IVec3::new(min_x, min_y, min_z),
                IVec3::new(max_x, max_y, max_z),
            );
        }

        frozen
    }

    // Structural integrity: Fast BFS from bedrock to find disconnected islands
    pub fn detect_unsupported_islands(&mut self) -> Vec<IslandChunk> {
        if !self.structural_dirty {
            return Vec::new();
        }
        self.structural_dirty = false;

        // Synchronize solid_count if external systems modified current_voxels
        if self.solid_count != self.current_voxels {
            self.solid_count = self.current_voxels;
        }

        // Early out if no solid non-bedrock voxels exist in the entire world and no subterranean regions dirtied
        if self.solid_count == 0 && self.dirty_min.is_some_and(|min| min.y > 0) {
            self.dirty_min = None;
            self.dirty_max = None;
            return Vec::new();
        }

        let total = GRID_X * GRID_Y * GRID_Z;

        // Determine active dirty bounds
        let (d_min, d_max, is_localized) = match (self.dirty_min.take(), self.dirty_max.take()) {
            (Some(min), Some(max)) => (min, max, true),
            _ => (
                IVec3::new(0, MIN_VOXEL_Y, 0),
                IVec3::new(GRID_X as i32 - 1, MAX_VOXEL_Y - 1, GRID_Z as i32 - 1),
                false,
            ),
        };

        let pad = (CANTILEVER_MAX_DEPTH as i32) * 2; // 8 voxels structural margin
        let global_max_y = (self.max_solid_y.max(1) + 1).min(MAX_VOXEL_Y - 1);

        // Compute expanded sub-region bounds
        let (box_min_x, box_max_x, box_min_z, box_max_z, mut box_max_y) = if is_localized {
            (
                (d_min.x - pad).max(0),
                (d_max.x + pad).min(GRID_X as i32 - 1),
                (d_min.z - pad).max(0),
                (d_max.z + pad).min(GRID_Z as i32 - 1),
                (d_max.y + pad).min(global_max_y),
            )
        } else {
            // Find active chunk columns containing surface structures or subterranean excavations
            let mut min_cx = CHUNKS_X;
            let mut max_cx = 0;
            let mut min_cz = CHUNKS_Z;
            let mut max_cz = 0;
            let mut found_any = false;
            for cx in 0..CHUNKS_X {
                for cz in 0..CHUNKS_Z {
                    let mut is_active = false;
                    for cy in 0..CHUNKS_SUBTERRANEAN_Y {
                        if self.chunk_solid_count[Self::chunk_idx(cx, cy, cz)] != 512 {
                            is_active = true;
                            break;
                        }
                    }
                    if !is_active && self.chunk_solid_count[Self::chunk_idx(cx, CHUNKS_SUBTERRANEAN_Y, cz)] != 64 {
                        is_active = true;
                    }
                    if !is_active {
                        for cy in (CHUNKS_SUBTERRANEAN_Y + 1)..CHUNKS_Y {
                            if self.chunk_solid_count[Self::chunk_idx(cx, cy, cz)] > 0 {
                                is_active = true;
                                break;
                            }
                        }
                    }
                    if is_active {
                        min_cx = min_cx.min(cx);
                        max_cx = max_cx.max(cx);
                        min_cz = min_cz.min(cz);
                        max_cz = max_cz.max(cz);
                        found_any = true;
                    }
                }
            }
            if found_any {
                (
                    ((min_cx * CHUNK_SIZE) as i32 - pad).max(0),
                    (((max_cx + 1) * CHUNK_SIZE - 1) as i32 + pad).min(GRID_X as i32 - 1),
                    ((min_cz * CHUNK_SIZE) as i32 - pad).max(0),
                    (((max_cz + 1) * CHUNK_SIZE - 1) as i32 + pad).min(GRID_Z as i32 - 1),
                    global_max_y,
                )
            } else {
                (0, GRID_X as i32 - 1, 0, GRID_Z as i32 - 1, global_max_y)
            }
        };

        // Take pre-allocated scratch buffers from self (0 heap allocations)
        let mut grounded = std::mem::take(&mut self.scratch_grounded);
        let mut tension_anchored = std::mem::take(&mut self.scratch_tension);
        let mut downward_depth = std::mem::take(&mut self.scratch_depth);
        let mut visited_islands = std::mem::take(&mut self.scratch_visited);
        let mut queue = std::mem::take(&mut self.scratch_queue);

        // Ensure buffer lengths match total voxels
        if grounded.len() != total {
            grounded.resize(total, false);
        }
        if tension_anchored.len() != total {
            tension_anchored.resize(total, false);
        }
        if downward_depth.len() != total {
            downward_depth.resize(total, 255u8);
        }
        if visited_islands.len() != total {
            visited_islands.resize(total, false);
        }

        // Reset scratch state strictly within active bounding box (all operations stay bounded inside)
        if box_min_x == 0
            && box_max_x == GRID_X as i32 - 1
            && box_min_z == 0
            && box_max_z == GRID_Z as i32 - 1
            && box_max_y == MAX_VOXEL_Y - 1
        {
            grounded.fill(false);
            tension_anchored.fill(false);
            downward_depth.fill(255);
            visited_islands.fill(false);
        } else {
            for x in (box_min_x as usize)..=(box_max_x as usize) {
                for y in MIN_VOXEL_Y..=box_max_y {
                    let start = Self::idx(x, y, box_min_z as usize);
                    let end = start + (box_max_z - box_min_z + 1) as usize;
                    grounded[start..end].fill(false);
                    tension_anchored[start..end].fill(false);
                    downward_depth[start..end].fill(255);
                    visited_islands[start..end].fill(false);
                }
            }
        }
        queue.clear();

        // Seed from the true geological foundation y=MIN_VOXEL_Y (-48, world -24.0m) strictly within the
        // sub-region column footprint.
        //
        // Fast path: a contiguous column of foundation rock (Bedrock/Basalt at y <= 0) rising from
        // MIN_VOXEL_Y always resolves to depth 0 + tension-anchored (compression straight up, tension
        // passed through rock), which is the optimal state. Those voxels are marked directly instead
        // of being flood-filled, and only voxels that can reach a non-pre-seeded neighbor are enqueued.
        // The fixpoint is identical to seeding the bottom layer and propagating upward.
        let mut col_top = std::mem::take(&mut self.scratch_col_top);
        if col_top.len() != GRID_X * GRID_Z {
            col_top.resize(GRID_X * GRID_Z, MIN_VOXEL_Y - 1);
        }
        let rock_ceiling = box_max_y.min(0);
        for x in (box_min_x as usize)..=(box_max_x as usize) {
            for z in (box_min_z as usize)..=(box_max_z as usize) {
                let mut top = MIN_VOXEL_Y - 1;
                let mut y = MIN_VOXEL_Y;
                while y <= rock_ceiling {
                    let i = Self::idx(x, y, z);
                    if !matches!(self.voxels[i], VoxelType::Bedrock | VoxelType::Basalt) {
                        break;
                    }
                    grounded[i] = true;
                    tension_anchored[i] = true;
                    downward_depth[i] = 0;
                    top = y;
                    y += 1;
                }
                col_top[x * GRID_Z + z] = top;
                if top < MIN_VOXEL_Y {
                    // Non-rock solid at the very bottom still acts as a foundation seed.
                    let i_min = Self::idx(x, MIN_VOXEL_Y, z);
                    if self.voxels[i_min].is_solid() {
                        grounded[i_min] = true;
                        tension_anchored[i_min] = true;
                        downward_depth[i_min] = 0;
                        queue.push_back((x, MIN_VOXEL_Y, z));
                    }
                }
            }
        }
        for x in box_min_x..=box_max_x {
            for z in box_min_z..=box_max_z {
                let top = col_top[x as usize * GRID_Z + z as usize];
                if top < MIN_VOXEL_Y {
                    continue;
                }
                // Lowest lateral neighbor column top (in-box only; propagation never leaves the box).
                // Voxel y in this column can feed a non-pre-seeded neighbor at level y or y+1 iff y >= that.
                let mut m = i32::MAX;
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        if dx == 0 && dz == 0 {
                            continue;
                        }
                        let (nx, nz) = (x + dx, z + dz);
                        if nx >= box_min_x && nx <= box_max_x && nz >= box_min_z && nz <= box_max_z {
                            m = m.min(col_top[nx as usize * GRID_Z + nz as usize]);
                        }
                    }
                }
                let start = m.max(MIN_VOXEL_Y).min(top);
                for y in start..=top {
                    queue.push_back((x as usize, y, z as usize));
                }
            }
        }
        self.scratch_col_top = col_top;

        let neighbors = [
            // 6 Cardinal
            (1, 0, 0), (-1, 0, 0),
            (0, 1, 0), (0, -1, 0),
            (0, 0, 1), (0, 0, -1),
            // 4 Horizontal Coplanar Diagonals (prevents Manhattan grid distortion)
            (1, 0, 1), (1, 0, -1),
            (-1, 0, 1), (-1, 0, -1),
            // 8 Vertical Diagonals (for diagonal structural bracing trusses)
            (1, 1, 0), (-1, 1, 0),
            (1, -1, 0), (-1, -1, 0),
            (0, 1, 1), (0, 1, -1),
            (0, -1, 1), (0, -1, -1),
        ];

        // Propagate grounded status upwards through connected solid voxels
        loop {
            while let Some((cx, cy, cz)) = queue.pop_front() {
            let curr_idx = Self::idx(cx, cy, cz);
            let curr_vt = self.voxels[curr_idx];
            let curr_tension = tension_anchored[curr_idx];
            let curr_depth = downward_depth[curr_idx];
            let curr_is_tensile_rebar = curr_vt == VoxelType::SteelRebar || curr_vt == VoxelType::CompositeRebar;
            let curr_is_structural_span = curr_is_tensile_rebar || curr_vt == VoxelType::HeavyArmor;
            let curr_is_foundation_rock = (curr_vt == VoxelType::Bedrock || curr_vt == VoxelType::Basalt) && cy <= 0;

            for (dx, dy, dz) in neighbors {
                // Vertical diagonals going downwards require structural rebar trusses
                if dy < 0 && (dx != 0 || dz != 0) && !curr_is_tensile_rebar {
                    continue;
                }

                let nx = cx as i32 + dx;
                let ny = cy + dy;
                let nz = cz as i32 + dz;
                if nx >= box_min_x && nx <= box_max_x && ny >= MIN_VOXEL_Y && ny <= box_max_y && nz >= box_min_z && nz <= box_max_z {
                    let (unx, unz) = (nx as usize, nz as usize);
                    let ni = Self::idx(unx, ny, unz);
                    let next_vt = self.voxels[ni];
                    // Skip non-solid air voxels immediately
                    if !next_vt.is_solid() {
                        continue;
                    }
                    // Skip if already grounded at foundation depth 0 and cannot gain tension
                    if grounded[ni] && downward_depth[ni] == 0 && (tension_anchored[ni] || !curr_tension) {
                        continue;
                    }

                    let next_is_tensile_rebar = next_vt == VoxelType::SteelRebar || next_vt == VoxelType::CompositeRebar;
                    let next_is_structural_span = next_is_tensile_rebar || next_vt == VoxelType::HeavyArmor;
                    let next_is_foundation_rock = (next_vt == VoxelType::Bedrock || next_vt == VoxelType::Basalt) && ny <= 0;

                    // Vertical diagonals going downwards require both endpoints to be tensile rebar
                    if dy < 0 && (dx != 0 || dz != 0) && !next_is_tensile_rebar {
                        continue;
                    }

                    // Physical support rules:
                    // 1. Upward (dy > 0): Compression from below. True foundation (curr_depth == 0) propagates 0.
                    //    Supported cantilever slabs (curr_depth <= CANTILEVER_MAX_DEPTH) support surface parapets/railings.
                    // 2. Horizontal (dy == 0): Lateral cohesion bounded by CANTILEVER_MAX_DEPTH.
                    //    Structural metal (rebar & heavy armor) bridges spans at depth 0 when supported.
                    // 3. Downward (dy < 0): Tension suspension from above.
                    //    ONLY high-tensile rebar (SteelRebar, CompositeRebar) in continuous tension can suspend indefinitely downwards (depth 0).
                    //    Heavy armor and masonry cannot act as tension cables and cannot suspend heavy structures.
                    let (can_support, next_depth) = if dy > 0 {
                        let is_diag = dx != 0 || dz != 0;
                        if curr_depth == 0 {
                            if is_diag && !curr_is_tensile_rebar {
                                (true, 1u8)
                            } else {
                                (true, 0u8)
                            }
                        } else if curr_depth < CANTILEVER_MAX_DEPTH {
                            if is_diag {
                                (true, curr_depth + 1)
                            } else {
                                (true, curr_depth)
                            }
                        } else if curr_depth == CANTILEVER_MAX_DEPTH && !is_diag {
                            (true, curr_depth)
                        } else {
                            (false, 255u8)
                        }
                    } else if dy == 0 {
                        if curr_is_tensile_rebar && next_is_tensile_rebar {
                            // Rebar-to-rebar: bridges at depth 0 only if anchored in tension or foundation
                            if curr_tension || curr_depth == 0 {
                                (true, 0u8)
                            } else if curr_depth < CANTILEVER_MAX_DEPTH {
                                (true, curr_depth)
                            } else {
                                (false, 255u8)
                            }
                        } else if curr_is_structural_span && next_is_structural_span && ((curr_tension && curr_is_tensile_rebar) || curr_depth == 0) {
                            (true, 0u8)
                        } else if !curr_is_tensile_rebar && next_is_tensile_rebar && curr_depth < CANTILEVER_MAX_DEPTH {
                            // Rebar reached across concrete in a reinforced slab preserves curr_depth
                            (true, curr_depth)
                        } else if curr_tension && curr_is_tensile_rebar {
                            (true, 1u8)
                        } else if curr_depth < CANTILEVER_MAX_DEPTH {
                            (true, curr_depth + 1)
                        } else {
                            (false, 255u8)
                        }
                    } else {
                        // dy < 0 (downwards) - strict tensile suspension
                        if curr_tension && (curr_is_tensile_rebar || curr_is_foundation_rock) && next_is_tensile_rebar {
                            (true, 0u8)
                        } else if curr_tension && curr_is_tensile_rebar {
                            (true, 1u8)
                        } else if curr_depth < CANTILEVER_MAX_DEPTH {
                            (true, curr_depth + 1)
                        } else {
                            (false, 255u8)
                        }
                    };

                    if can_support {
                        let next_tension = if next_is_foundation_rock {
                            curr_tension
                        } else if !next_is_tensile_rebar {
                            false
                        } else if dy > 0 {
                            curr_tension
                        } else {
                            curr_tension && (curr_is_tensile_rebar || curr_is_foundation_rock)
                        };

                        if next_tension && !tension_anchored[ni] {
                            tension_anchored[ni] = true;
                            if grounded[ni] {
                                queue.push_back((unx, ny, unz));
                            }
                        }

                        if !grounded[ni] {
                            grounded[ni] = true;
                            downward_depth[ni] = next_depth;
                            queue.push_back((unx, ny, unz));
                        } else if next_depth < downward_depth[ni] {
                            downward_depth[ni] = next_depth;
                            queue.push_back((unx, ny, unz));
                        }
                    }
                }
            }
        }

        // Adaptive vertical expansion check:
        // If the sub-region ceiling is below global_max_y, check if ANY solid voxels exist above box_max_y
        // within the column footprint (spanning across air gaps, window voids, or blast cavities).
        if box_max_y < global_max_y {
            let mut needs_vertical_expand = false;
            if self.region_has_solids(box_min_x, box_max_x, box_max_y + 1, global_max_y, box_min_z, box_max_z) {
                'col_scan: for x in (box_min_x as usize)..=(box_max_x as usize) {
                    for z in (box_min_z as usize)..=(box_max_z as usize) {
                        for y in (box_max_y + 1)..=global_max_y {
                            if self.voxels[Self::idx(x, y, z)].is_solid() {
                                needs_vertical_expand = true;
                                break 'col_scan;
                            }
                        }
                    }
                }
            }
            if needs_vertical_expand {
                if is_localized {
                    for x in (box_min_x as usize)..=(box_max_x as usize) {
                        for y in (box_max_y + 1)..=global_max_y {
                            let start = Self::idx(x, y, box_min_z as usize);
                            let end = start + (box_max_z - box_min_z + 1) as usize;
                            grounded[start..end].fill(false);
                            tension_anchored[start..end].fill(false);
                            downward_depth[start..end].fill(255);
                            visited_islands[start..end].fill(false);
                        }
                    }
                }
                // Re-enqueue any grounded voxels at box_max_y so ground propagation continues upward
                for x in (box_min_x as usize)..=(box_max_x as usize) {
                    for z in (box_min_z as usize)..=(box_max_z as usize) {
                        let i_top = Self::idx(x, box_max_y, z);
                        if self.voxels[i_top].is_solid() && grounded[i_top] {
                            queue.push_back((x, box_max_y, z));
                        }
                    }
                }
                box_max_y = global_max_y;
                continue;
            }
        }
        break;
    }

        let mut island_chunks = Vec::new();
        let scan_min_y = MIN_VOXEL_Y + 1;
        let mut min_island_p = IVec3::splat(i32::MAX);
        let mut max_island_p = IVec3::splat(i32::MIN);
        let mut touches_solid_outside = false;

        'scan_loop: for x in (box_min_x as usize)..=(box_max_x as usize) {
            for y in scan_min_y..=box_max_y { // MIN_VOXEL_Y is bedrock foundation
                for z in (box_min_z as usize)..=(box_max_z as usize) {
                    let i = Self::idx(x, y, z);
                    if self.voxels[i].is_solid() && !grounded[i] && !visited_islands[i] {
                        // Found a floating piece! Flood-fill this component using the reusable queue
                        let mut component_voxels = Vec::with_capacity(32);

                        visited_islands[i] = true;
                        queue.push_back((x, y, z));

                        while let Some((ix, iy, iz)) = queue.pop_front() {
                            let vt = self.get(ix as i32, iy, iz as i32);
                            let v_coord = IVec3::new(ix as i32, iy, iz as i32);
                            component_voxels.push((v_coord, vt));
                            min_island_p = min_island_p.min(v_coord);
                            max_island_p = max_island_p.max(v_coord);

                            for (dx, dy, dz) in neighbors {
                                if dy != 0 && (dx != 0 || dz != 0) {
                                    let curr_is_metal = matches!(vt, VoxelType::SteelRebar | VoxelType::CompositeRebar | VoxelType::HeavyArmor);
                                    if !curr_is_metal {
                                        continue;
                                    }
                                }
                                let nx = ix as i32 + dx;
                                let ny = iy + dy;
                                let nz = iz as i32 + dz;
                                if nx >= box_min_x && nx <= box_max_x && ny >= MIN_VOXEL_Y && ny <= box_max_y && nz >= box_min_z && nz <= box_max_z {
                                    let (unx, unz) = (nx as usize, nz as usize);
                                    let ni = Self::idx(unx, ny, unz);
                                    let next_vt = self.voxels[ni];
                                    if dy != 0 && (dx != 0 || dz != 0) {
                                        let next_is_metal = matches!(next_vt, VoxelType::SteelRebar | VoxelType::CompositeRebar | VoxelType::HeavyArmor);
                                        if !next_is_metal {
                                            continue;
                                        }
                                    }
                                    if next_vt.is_solid() && !grounded[ni] && !visited_islands[ni] {
                                        visited_islands[ni] = true;
                                        queue.push_back((unx, ny, unz));
                                    }
                                } else if is_localized && self.in_bounds(nx, ny, nz) && self.get(nx, ny, nz).is_solid() {
                                    touches_solid_outside = true;
                                    break;
                                }
                            }
                            if touches_solid_outside {
                                break;
                            }
                        }

                        if touches_solid_outside {
                            break 'scan_loop;
                        }

                        let mut sum_pos = Vec3::ZERO;
                        for &(vpos, _) in &component_voxels {
                            sum_pos += self.voxel_to_world(vpos);
                        }
                        let center_of_mass = sum_pos / component_voxels.len().max(1) as f32;
                        island_chunks.push(IslandChunk {
                            voxel_health: component_voxels.iter().map(|(p, _)| self.health[Self::idx(p.x as usize, p.y, p.z as usize)]).collect(),
                            voxels: component_voxels,
                            center_of_mass,
                        });
                    }
                }
            }
        }

        // Clipped-path confirmation (lazy: only when islands were found and not already escalating).
        // A support path that leaves the box and re-enters at in-box voxel B can only change the
        // result if B's in-box state is sub-optimal. Optimal = depth 0 and, for voxels that can carry
        // tension (rebar, foundation rock), tension-anchored. Ungrounded B is already covered by
        // `touches_solid_outside` in the island flood fill above.
        let mut boundary_leak = false;
        if is_localized && !touches_solid_outside && !island_chunks.is_empty() {
            let walls_x = [
                (box_min_x > 0).then_some(box_min_x),
                (box_max_x < GRID_X as i32 - 1).then_some(box_max_x),
            ];
            let walls_z = [
                (box_min_z > 0).then_some(box_min_z),
                (box_max_z < GRID_Z as i32 - 1).then_some(box_max_z),
            ];
            let check = |x: i32, y: i32, z: i32| -> bool {
                let i = Self::idx(x as usize, y, z as usize);
                let vt = self.voxels[i];
                if !vt.is_solid() || !grounded[i] {
                    return false;
                }
                let carries_tension = matches!(vt, VoxelType::SteelRebar | VoxelType::CompositeRebar)
                    || (matches!(vt, VoxelType::Bedrock | VoxelType::Basalt) && y <= 0);
                if downward_depth[i] == 0 && (tension_anchored[i] || !carries_tension) {
                    return false;
                }
                neighbors.iter().any(|&(dx, dy, dz)| {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                    (nx < box_min_x || nx > box_max_x || nz < box_min_z || nz > box_max_z)
                        && self.in_bounds(nx, ny, nz)
                        && self.voxels[Self::idx(nx as usize, ny, nz as usize)].is_solid()
                })
            };
            'leak: for y in MIN_VOXEL_Y..=box_max_y {
                for wx in walls_x.iter().flatten() {
                    for z in box_min_z..=box_max_z {
                        if check(*wx, y, z) {
                            boundary_leak = true;
                            break 'leak;
                        }
                    }
                }
                for wz in walls_z.iter().flatten() {
                    for x in box_min_x..=box_max_x {
                        if check(x, y, *wz) {
                            boundary_leak = true;
                            break 'leak;
                        }
                    }
                }
            }
        }

        // Restore pre-allocated scratch buffers back to self (0 heap allocations)
        queue.clear();
        self.scratch_grounded = grounded;
        self.scratch_tension = tension_anchored;
        self.scratch_depth = downward_depth;
        self.scratch_visited = visited_islands;
        self.scratch_queue = queue;

        if is_localized && (touches_solid_outside || boundary_leak) {
            // Either an island extends past the box, or a support path clipped by the box may have
            // produced a false island (measured: localized results over-detected in most such cases).
            // Fall back to an exact global pass. We did NOT mutate the grid, so no restoration is needed.
            self.dirty_min = None;
            self.dirty_max = None;
            self.structural_dirty = true;
            return self.detect_unsupported_islands();
        }

        // Now that we are sure we are not aborting, actually remove the chunks from the grid
        for chunk in &island_chunks {
            for &(vpos, vt) in &chunk.voxels {
                let idx = Self::idx(vpos.x as usize, vpos.y, vpos.z as usize);
                self.voxels[idx] = VoxelType::Air;
                self.health[idx] = 0.0;
                let cx = (vpos.x as usize) / CHUNK_SIZE;
                let internal_y = (vpos.y - MIN_VOXEL_Y) as usize;
                let cy = internal_y / CHUNK_SIZE;
                let cz = (vpos.z as usize) / CHUNK_SIZE;
                let ci = Self::chunk_idx(cx, cy, cz);
                self.chunk_solid_count[ci] = self.chunk_solid_count[ci].saturating_sub(1);
                self.mark_chunk_dirty(vpos.x, vpos.y, vpos.z);
                if vt == VoxelType::SteelRebar || vt == VoxelType::CompositeRebar {
                    self.current_rebar_count = self.current_rebar_count.saturating_sub(1);
                    self.severed_rebar_positions.push(self.voxel_to_world(vpos));
                }
            }
            let above_ground = chunk.voxels.iter().filter(|(p, _)| p.y > 0).count();
            self.solid_count = self.solid_count.saturating_sub(above_ground);
        }

        // Feature 4: Single-voxel debris preservation
        if !self.preserve_single_voxel_debris {
            island_chunks.retain(|c| c.voxels.len() >= 2);
        }

        if !island_chunks.is_empty() {
            self.current_voxels = self.solid_count;
            self.mark_dirty_region(min_island_p, max_island_p);
            self.structural_dirty = true;
        } else {
            self.current_voxels = self.solid_count;
        }

        island_chunks
    }

    // Rebuild dirty chunk meshes
    pub fn update_meshes(&mut self) {
        if self.solid_count != self.current_voxels {
            self.solid_count = self.current_voxels;
        }
        if !self.has_dirty_meshes {
            return;
        }
        self.has_dirty_meshes = false;

        if !self.dirty_chunks.is_empty() {
            let mut scratch_dirty = std::mem::take(&mut self.scratch_dirty_chunks);
            scratch_dirty.clear();
            scratch_dirty.extend_from_slice(&self.dirty_chunks);
            self.dirty_chunks.clear();
            scratch_dirty.sort_unstable();
            scratch_dirty.dedup();
            for &ci in &scratch_dirty {
                if self.meshes[ci].dirty {
                    let cx = ci / (CHUNKS_Y * CHUNKS_Z);
                    let rem = ci % (CHUNKS_Y * CHUNKS_Z);
                    let cy = rem / CHUNKS_Z;
                    let cz = rem % CHUNKS_Z;
                    self.rebuild_chunk_mesh(cx, cy, cz);
                }
            }
            self.scratch_dirty_chunks = scratch_dirty;
        } else {
            for cx in 0..CHUNKS_X {
                for cy in 0..CHUNKS_Y {
                    for cz in 0..CHUNKS_Z {
                        let ci = Self::chunk_idx(cx, cy, cz);
                        if self.meshes[ci].dirty {
                            self.rebuild_chunk_mesh(cx, cy, cz);
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn vertex_ao(s1: bool, s2: bool, diag: bool) -> f32 {
        if s1 && s2 {
            0.52
        } else {
            let count = (s1 as u8) + (s2 as u8) + (diag as u8);
            match count {
                0 => 1.0,
                1 => 0.82,
                2 => 0.68,
                _ => 0.58,
            }
        }
    }

    fn get_quad_corners(
        quad: &QuadSpan,
        start_x: usize,
        start_y: i32,
        start_z: usize,
        half_x: f32,
        half_z: f32,
    ) -> [Vec3; 4] {
        let to_world = |vx: usize, vy: i32, vz: usize| -> Vec3 {
            Vec3::new(
                vx as f32 * VOXEL_SIZE - half_x,
                vy as f32 * VOXEL_SIZE,
                vz as f32 * VOXEL_SIZE - half_z,
            )
        };

        match quad.fd {
            FaceDir::PosX => {
                let x = quad.slice as usize;
                let y0 = start_y + quad.u0 as i32;
                let z0 = start_z + quad.v0;
                [
                    to_world(x + 1, y0, z0),
                    to_world(x + 1, y0 + quad.span_u as i32, z0),
                    to_world(x + 1, y0 + quad.span_u as i32, z0 + quad.span_v),
                    to_world(x + 1, y0, z0 + quad.span_v),
                ]
            }
            FaceDir::NegX => {
                let x = quad.slice as usize;
                let y0 = start_y + quad.u0 as i32;
                let z0 = start_z + quad.v0;
                [
                    to_world(x, y0, z0 + quad.span_v),
                    to_world(x, y0 + quad.span_u as i32, z0 + quad.span_v),
                    to_world(x, y0 + quad.span_u as i32, z0),
                    to_world(x, y0, z0),
                ]
            }
            FaceDir::PosY => {
                let y = quad.slice;
                let x0 = start_x + quad.u0;
                let z0 = start_z + quad.v0;
                [
                    to_world(x0, y + 1, z0),
                    to_world(x0, y + 1, z0 + quad.span_v),
                    to_world(x0 + quad.span_u, y + 1, z0 + quad.span_v),
                    to_world(x0 + quad.span_u, y + 1, z0),
                ]
            }
            FaceDir::NegY => {
                let y = quad.slice;
                let x0 = start_x + quad.u0;
                let z0 = start_z + quad.v0;
                [
                    to_world(x0, y, z0 + quad.span_v),
                    to_world(x0, y, z0),
                    to_world(x0 + quad.span_u, y, z0),
                    to_world(x0 + quad.span_u, y, z0 + quad.span_v),
                ]
            }
            FaceDir::PosZ => {
                let z = quad.slice as usize;
                let x0 = start_x + quad.u0;
                let y0 = start_y + quad.v0 as i32;
                [
                    to_world(x0 + quad.span_u, y0, z + 1),
                    to_world(x0 + quad.span_u, y0 + quad.span_v as i32, z + 1),
                    to_world(x0, y0 + quad.span_v as i32, z + 1),
                    to_world(x0, y0, z + 1),
                ]
            }
            FaceDir::NegZ => {
                let z = quad.slice as usize;
                let x0 = start_x + quad.u0;
                let y0 = start_y + quad.v0 as i32;
                [
                    to_world(x0, y0, z),
                    to_world(x0, y0 + quad.span_v as i32, z),
                    to_world(x0 + quad.span_u, y0 + quad.span_v as i32, z),
                    to_world(x0 + quad.span_u, y0, z),
                ]
            }
        }
    }

    fn rebuild_chunk_mesh(&mut self, cx: usize, cy: usize, cz: usize) {
        let ci = Self::chunk_idx(cx, cy, cz);
        if self.chunk_solid_count[ci] == 0 {
            let cm = &mut self.meshes[ci];
            for mut m in cm.sub_meshes.drain(..) {
                if self.mesh_pool.len() < MESH_POOL_MAX_CAPACITY {
                    m.vertices.clear();
                    m.indices.clear();
                    self.mesh_pool.push(m);
                }
            }
            cm.dirty = false;
            return;
        }

        let mut scratch_vertices = std::mem::take(&mut self.scratch_vertices);
        let mut scratch_indices = std::mem::take(&mut self.scratch_indices);
        scratch_vertices.clear();
        scratch_indices.clear();

        let start_x = cx * CHUNK_SIZE;
        let start_y = MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32;
        let start_z = cz * CHUNK_SIZE;

        let end_x = (start_x + CHUNK_SIZE).min(GRID_X);
        let end_y = (start_y + CHUNK_SIZE as i32).min(MAX_VOXEL_Y);
        let end_z = (start_z + CHUNK_SIZE).min(GRID_Z);

        let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;

        // Fast sub-chunk bounding pass: find tight bounding box of solid voxels
        let mut sub_min_x = usize::MAX;
        let mut sub_max_x = 0;
        let mut sub_min_y = i32::MAX;
        let mut sub_max_y = i32::MIN;
        let mut sub_min_z = usize::MAX;
        let mut sub_max_z = 0;
        let mut has_solid = false;

        for x in start_x..end_x {
            for y in start_y..end_y {
                for z in start_z..end_z {
                    let idx = Self::idx(x, y, z);
                    let vt = self.voxels[idx];
                    if vt.is_solid() {
                        has_solid = true;
                        if x < sub_min_x { sub_min_x = x; }
                        if x > sub_max_x { sub_max_x = x; }
                        if y < sub_min_y { sub_min_y = y; }
                        if y > sub_max_y { sub_max_y = y; }
                        if z < sub_min_z { sub_min_z = z; }
                        if z > sub_max_z { sub_max_z = z; }
                    }
                }
            }
        }

        if !has_solid {
            let mut mesh_pool = std::mem::take(&mut self.mesh_pool);
            let cm = &mut self.meshes[ci];
            for mut m in cm.sub_meshes.drain(..) {
                if mesh_pool.len() < MESH_POOL_MAX_CAPACITY {
                    m.vertices.clear();
                    m.indices.clear();
                    mesh_pool.push(m);
                }
            }
            cm.dirty = false;
            self.mesh_pool = mesh_pool;
            self.scratch_vertices = scratch_vertices;
            self.scratch_indices = scratch_indices;
            return;
        }

        let face_dirs = [
            FaceDir::PosX,
            FaceDir::NegX,
            FaceDir::PosY,
            FaceDir::NegY,
            FaceDir::PosZ,
            FaceDir::NegZ,
        ];

        for &fd in &face_dirs {
            let (dx, dy, dz) = fd.normal();
            let shade = fd.shade();
            let offsets = fd.offsets();

            let (u_dim, v_dim) = match fd {
                FaceDir::PosX | FaceDir::NegX => ((end_y - start_y) as usize, end_z - start_z),
                FaceDir::PosY | FaceDir::NegY => (end_x - start_x, end_z - start_z),
                FaceDir::PosZ | FaceDir::NegZ => (end_x - start_x, (end_y - start_y) as usize),
            };

            let (s_min, s_max, u_min, u_max, v_min, v_max) = match fd {
                FaceDir::PosX | FaceDir::NegX => (
                    sub_min_x - start_x, sub_max_x - start_x,
                    (sub_min_y - start_y) as usize, (sub_max_y - start_y) as usize,
                    sub_min_z - start_z, sub_max_z - start_z,
                ),
                FaceDir::PosY | FaceDir::NegY => (
                    (sub_min_y - start_y) as usize, (sub_max_y - start_y) as usize,
                    sub_min_x - start_x, sub_max_x - start_x,
                    sub_min_z - start_z, sub_max_z - start_z,
                ),
                FaceDir::PosZ | FaceDir::NegZ => (
                    sub_min_z - start_z, sub_max_z - start_z,
                    sub_min_x - start_x, sub_max_x - start_x,
                    (sub_min_y - start_y) as usize, (sub_max_y - start_y) as usize,
                ),
            };

            for s_idx in s_min..=s_max {
                let slice = match fd {
                    FaceDir::PosX | FaceDir::NegX => (start_x + s_idx) as i32,
                    FaceDir::PosY | FaceDir::NegY => start_y + s_idx as i32,
                    FaceDir::PosZ | FaceDir::NegZ => (start_z + s_idx) as i32,
                };

                let mut grid_cells = [[CellState::Empty; CHUNK_SIZE]; CHUNK_SIZE];
                let mut ao_store = [[[0.0f32; 4]; CHUNK_SIZE]; CHUNK_SIZE];

                for u in u_min..=u_max {
                    for v in v_min..=v_max {
                        let (x, y, z): (usize, i32, usize) = match fd {
                            FaceDir::PosX | FaceDir::NegX => (slice as usize, start_y + u as i32, start_z + v),
                            FaceDir::PosY | FaceDir::NegY => (start_x + u, slice, start_z + v),
                            FaceDir::PosZ | FaceDir::NegZ => (start_x + u, start_y + v as i32, slice as usize),
                        };

                        let idx = Self::idx(x, y, z);
                        let vt = self.voxels[idx];
                        if !vt.is_solid() {
                            continue;
                        }

                        let nx = x as i32 + dx;
                        let ny = y + dy;
                        let nz = z as i32 + dz;
                        let neighbor = self.get(nx, ny, nz);
                        let draw_face = if !neighbor.is_solid() {
                            true
                        } else if neighbor.is_transparent() {
                            vt.is_opaque() || vt != neighbor
                        } else {
                            false
                        };

                        if !draw_face {
                            continue;
                        }

                        let health_ratio = self.get_health_ratio(x as i32, y, z as i32);
                        let damage = (1.0 - health_ratio).clamp(0.0, 1.0);
                        let is_damaged = damage > 0.001;

                        let mut aos = [0.0f32; 4];
                        for c in 0..4 {
                            let off = offsets[c];
                            let (s1, s2, diag) = if dx != 0 {
                                let sy = if off.y > 0.0 { 1 } else { -1 };
                                let sz = if off.z > 0.0 { 1 } else { -1 };
                                (
                                    self.get(x as i32 + dx, y + sy, z as i32).is_solid(),
                                    self.get(x as i32 + dx, y, z as i32 + sz).is_solid(),
                                    self.get(x as i32 + dx, y + sy, z as i32 + sz).is_solid(),
                                )
                            } else if dy != 0 {
                                let sx = if off.x > 0.0 { 1 } else { -1 };
                                let sz = if off.z > 0.0 { 1 } else { -1 };
                                (
                                    self.get(x as i32 + sx, y + dy, z as i32).is_solid(),
                                    self.get(x as i32, y + dy, z as i32 + sz).is_solid(),
                                    self.get(x as i32 + sx, y + dy, z as i32 + sz).is_solid(),
                                )
                            } else {
                                let sx = if off.x > 0.0 { 1 } else { -1 };
                                let sy = if off.y > 0.0 { 1 } else { -1 };
                                (
                                    self.get(x as i32 + sx, y, z as i32 + dz).is_solid(),
                                    self.get(x as i32, y + sy, z as i32 + dz).is_solid(),
                                    self.get(x as i32 + sx, y + sy, z as i32 + dz).is_solid(),
                                )
                            };
                            aos[c] = Self::vertex_ao(s1, s2, diag);
                        }
                        ao_store[u][v] = aos;

                        if is_damaged {
                            grid_cells[u][v] = CellState::Damaged;
                        } else {
                            let is_uniform = (aos[0] - aos[1]).abs() < 0.001
                                && (aos[1] - aos[2]).abs() < 0.001
                                && (aos[2] - aos[3]).abs() < 0.001;

                            if is_uniform {
                                let ao_key = (aos[0] * 100.0).round() as u8;
                                grid_cells[u][v] = CellState::Mergeable(vt, ao_key);
                            } else {
                                grid_cells[u][v] = CellState::NonUniformPristine;
                            }
                        }
                    }
                }

                // Greedy meshing sweep across this slice
                for u in u_min..=u_max {
                    for v in v_min..=v_max {
                        let state = grid_cells[u][v];
                        if state == CellState::Empty {
                            continue;
                        }

                        let (x, y, z): (usize, i32, usize) = match fd {
                            FaceDir::PosX | FaceDir::NegX => (slice as usize, start_y + u as i32, start_z + v),
                            FaceDir::PosY | FaceDir::NegY => (start_x + u, slice, start_z + v),
                            FaceDir::PosZ | FaceDir::NegZ => (start_x + u, start_y + v as i32, slice as usize),
                        };

                        let v_start = scratch_vertices.len() as u16;
                        if v_start > 65530 {
                            break;
                        }

                        match state {
                            CellState::Empty => {}
                            CellState::Damaged => {
                                let vt = self.voxels[Self::idx(x, y, z)];
                                let base_color = vt.color();
                                let props = vt.properties();
                                let health_ratio = self.get_health_ratio(x as i32, y, z as i32);
                                let damage = (1.0 - health_ratio).clamp(0.0, 1.0);
                                let emissive_boost = if props.is_emissive { 1.30 } else { 1.0 };

                                let block_hash = ((x as u32).wrapping_mul(73856093))
                                    ^ ((y as u32).wrapping_mul(19349663))
                                    ^ ((z as u32).wrapping_mul(83492791));
                                let micro_jitter = 0.94 + ((block_hash & 0x1F) as f32 / 31.0) * 0.12;

                                let center = Vec3::new(
                                    x as f32 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5,
                                    y as f32 * VOXEL_SIZE + VOXEL_SIZE * 0.5,
                                    z as f32 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5,
                                );

                                for (i, &off) in offsets.iter().enumerate() {
                                    let vert_pos = center + off * VOXEL_SIZE;
                                    let ao = ao_store[u][v][i];
                                    let altitude = (vert_pos.y / 48.0).clamp(0.0, 1.0);
                                    let sky_ambient = 1.0 + altitude * 0.08;

                                    let vx = (vert_pos.x * 12.0).round() as i32;
                                    let vy = (vert_pos.y * 12.0).round() as i32;
                                    let vz = (vert_pos.z * 12.0).round() as i32;
                                    let corner_hash = ((vx.wrapping_mul(374761393) ^ vy.wrapping_mul(668265263) ^ vz.wrapping_mul(912389123)) as u32).wrapping_mul(1274126177);
                                    let fracture_noise = (corner_hash & 0xFF) as f32 / 255.0;
                                    let vein_noise = ((corner_hash >> 8) & 0xFF) as f32 / 255.0;

                                    let corner_damage = (damage * (0.65 + 0.70 * fracture_noise)).clamp(0.0, 1.0);
                                    let soot_blend = corner_damage.powf(1.15);
                                    let soot_color = Color::new(0.09, 0.08, 0.08, base_color.a);

                                    // Carbon scorch veins: procedural high-frequency fracture network
                                    let vein_pattern = ((vert_pos.x * 22.0).sin() * (vert_pos.y * 22.0).cos() * (vert_pos.z * 22.0).sin()).abs();
                                    let vein_intensity = if vein_pattern > 0.40 && corner_damage > 0.20 {
                                        ((vein_pattern - 0.40) / 0.60) * corner_damage * (0.75 + 0.25 * vein_noise)
                                    } else {
                                        0.0
                                    };

                                    // Glowing molten fracture embers along damaged fissures
                                    let ember_boost = if (props.is_emissive || vt == VoxelType::Explosive) && corner_damage > 0.30 {
                                        ((corner_hash >> 16) & 0x3F) as f32 / 63.0
                                    } else if corner_damage > 0.65 && fracture_noise > 0.55 {
                                        (corner_damage - 0.65) * 1.8 * fracture_noise
                                    } else {
                                        0.0
                                    };

                                    let r = ((base_color.r * (1.0 - soot_blend) + soot_color.r * soot_blend) * (1.0 - vein_intensity * 0.75) + ember_boost * 0.55).clamp(0.0, 1.0);
                                    let g = ((base_color.g * (1.0 - soot_blend) + soot_color.g * soot_blend) * (1.0 - vein_intensity * 0.75) + ember_boost * 0.20).clamp(0.0, 1.0);
                                    let b = ((base_color.b * (1.0 - soot_blend) + soot_color.b * soot_blend) * (1.0 - vein_intensity * 0.75) + ember_boost * 0.04).clamp(0.0, 1.0);

                                    let deg = (1.0 - corner_damage * 0.64) * (0.86 + 0.24 * fracture_noise) * (1.0 - vein_intensity * 0.35);

                                    // Volumetric edge chipping & pre-failure column buckling animation
                                    let chip_factor = corner_damage * (0.35 + 0.45 * fracture_noise);
                                    let chip_disp = (chip_factor * VOXEL_SIZE * 0.18).min(VOXEL_SIZE * 0.22);
                                    let face_norm = Vec3::new(dx as f32, dy as f32, dz as f32);
                                    let is_column_material = matches!(vt, VoxelType::Concrete | VoxelType::SteelRebar | VoxelType::CompositeRebar);
                                    let bulge_disp = if is_column_material && corner_damage > 0.18 && face_norm.y.abs() < 0.2 {
                                        (corner_damage * 0.45 + 0.35 * fracture_noise) * (VOXEL_SIZE * 0.16)
                                    } else {
                                        0.0
                                    };
                                    let deformed_pos = vert_pos + face_norm * bulge_disp - off * chip_disp - face_norm * (chip_disp * 0.35);

                                    let final_shade = (shade * deg * emissive_boost * ao * micro_jitter * sky_ambient).clamp(0.0, 1.8);
                                    let vertex_color = Color::new(
                                        (r * final_shade).min(1.0),
                                        (g * final_shade).min(1.0),
                                        (b * final_shade).min(1.0),
                                        base_color.a,
                                    );

                                    scratch_vertices.push(Vertex {
                                        position: deformed_pos,
                                        uv: Vec2::ZERO,
                                        color: vertex_color.into(),
                                        normal: Vec4::ZERO,
                                    });
                                }

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 1);
                                scratch_indices.push(v_start + 2);

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 2);
                                scratch_indices.push(v_start + 3);
                            }
                            CellState::NonUniformPristine => {
                                let vt = self.voxels[Self::idx(x, y, z)];
                                let base_color = vt.color();
                                let props = vt.properties();
                                let emissive_boost = if props.is_emissive { 1.30 } else { 1.0 };
                                let block_hash = ((x as u32).wrapping_mul(73856093))
                                    ^ ((y as u32).wrapping_mul(19349663))
                                    ^ ((z as u32).wrapping_mul(83492791));
                                let micro_jitter = 0.94 + ((block_hash & 0x1F) as f32 / 31.0) * 0.12;
                                let aos = ao_store[u][v];

                                let quad = QuadSpan {
                                    fd,
                                    slice,
                                    u0: u,
                                    v0: v,
                                    span_u: 1,
                                    span_v: 1,
                                };
                                let corners = Self::get_quad_corners(
                                    &quad, start_x, start_y, start_z, half_x, half_z,
                                );

                                for (c, &pos) in corners.iter().enumerate() {
                                    let ao = aos[c];
                                    let altitude = (pos.y / 48.0).clamp(0.0, 1.0);
                                    let sky_ambient = 1.0 + altitude * 0.08;
                                    let final_shade = (shade * emissive_boost * ao * micro_jitter * sky_ambient).clamp(0.0, 1.8);
                                    let vertex_color = Color::new(
                                        (base_color.r * final_shade).min(1.0),
                                        (base_color.g * final_shade).min(1.0),
                                        (base_color.b * final_shade).min(1.0),
                                        base_color.a,
                                    );

                                    scratch_vertices.push(Vertex {
                                        position: pos,
                                        uv: Vec2::ZERO,
                                        color: vertex_color.into(),
                                        normal: Vec4::ZERO,
                                    });
                                }

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 1);
                                scratch_indices.push(v_start + 2);

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 2);
                                scratch_indices.push(v_start + 3);
                            }
                            CellState::Mergeable(vt, ao_key) => {
                                let mut span_v = 1;
                                while v + span_v < v_dim && grid_cells[u][v + span_v] == CellState::Mergeable(vt, ao_key) {
                                    span_v += 1;
                                }

                                let mut span_u = 1;
                                'outer: while u + span_u < u_dim {
                                    for k in 0..span_v {
                                        if grid_cells[u + span_u][v + k] != CellState::Mergeable(vt, ao_key) {
                                            break 'outer;
                                        }
                                    }
                                    span_u += 1;
                                }

                                for du in 0..span_u {
                                    for dv in 0..span_v {
                                        grid_cells[u + du][v + dv] = CellState::Empty;
                                    }
                                }

                                let base_color = vt.color();
                                let props = vt.properties();
                                let emissive_boost = if props.is_emissive { 1.30 } else { 1.0 };
                                let block_hash = ((x as u32).wrapping_mul(73856093))
                                    ^ ((y as u32).wrapping_mul(19349663))
                                    ^ ((z as u32).wrapping_mul(83492791));
                                let micro_jitter = 0.94 + ((block_hash & 0x1F) as f32 / 31.0) * 0.12;

                                let ao_val = match ao_key {
                                    100 => 1.0,
                                    82 => 0.82,
                                    68 => 0.68,
                                    58 => 0.58,
                                    52 => 0.52,
                                    _ => ao_key as f32 / 100.0,
                                };

                                let quad = QuadSpan {
                                    fd,
                                    slice,
                                    u0: u,
                                    v0: v,
                                    span_u,
                                    span_v,
                                };
                                let corners = Self::get_quad_corners(
                                    &quad, start_x, start_y, start_z, half_x, half_z,
                                );

                                for &pos in &corners {
                                    let altitude = (pos.y / 48.0).clamp(0.0, 1.0);
                                    let sky_ambient = 1.0 + altitude * 0.08;
                                    let final_shade = (shade * emissive_boost * ao_val * micro_jitter * sky_ambient).clamp(0.0, 1.8);
                                    let vertex_color = Color::new(
                                        (base_color.r * final_shade).min(1.0),
                                        (base_color.g * final_shade).min(1.0),
                                        (base_color.b * final_shade).min(1.0),
                                        base_color.a,
                                    );

                                    scratch_vertices.push(Vertex {
                                        position: pos,
                                        uv: Vec2::ZERO,
                                        color: vertex_color.into(),
                                        normal: Vec4::ZERO,
                                    });
                                }

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 1);
                                scratch_indices.push(v_start + 2);

                                scratch_indices.push(v_start);
                                scratch_indices.push(v_start + 2);
                                scratch_indices.push(v_start + 3);
                            }
                        }
                    }
                }
            }
        }

        let mut mesh_pool = std::mem::take(&mut self.mesh_pool);
        let mut scratch_partition = std::mem::take(&mut self.scratch_partition_ranges);

        let ci = Self::chunk_idx(cx, cy, cz);
        let cm = &mut self.meshes[ci];

        if scratch_indices.is_empty() {
            for mut m in cm.sub_meshes.drain(..) {
                if mesh_pool.len() < MESH_POOL_MAX_CAPACITY {
                    m.vertices.clear();
                    m.indices.clear();
                    mesh_pool.push(m);
                }
            }
        } else if scratch_indices.len() < MAX_CHUNK_BATCH_INDICES && scratch_vertices.len() < MAX_CHUNK_BATCH_VERTICES {
            if cm.sub_meshes.is_empty() {
                let mut m = mesh_pool.pop().unwrap_or_else(|| Mesh {
                    vertices: Vec::with_capacity(scratch_vertices.len()),
                    indices: Vec::with_capacity(scratch_indices.len()),
                    texture: None,
                });
                m.vertices.clear();
                m.vertices.extend_from_slice(&scratch_vertices);
                m.indices.clear();
                m.indices.extend_from_slice(&scratch_indices);
                m.texture = None;
                cm.sub_meshes.push(m);
            } else {
                cm.sub_meshes[0].vertices.clear();
                cm.sub_meshes[0].vertices.extend_from_slice(&scratch_vertices);
                cm.sub_meshes[0].indices.clear();
                cm.sub_meshes[0].indices.extend_from_slice(&scratch_indices);
                cm.sub_meshes[0].texture = None;
                for mut m in cm.sub_meshes.drain(1..) {
                    if mesh_pool.len() < MESH_POOL_MAX_CAPACITY {
                        m.vertices.clear();
                        m.indices.clear();
                        mesh_pool.push(m);
                    }
                }
            }
        } else {
            partition_mesh_sub_batches_into(
                &scratch_indices,
                scratch_vertices.len(),
                MAX_CHUNK_BATCH_VERTICES,
                MAX_CHUNK_BATCH_INDICES,
                &mut scratch_partition,
            );
            let needed = scratch_partition.len();

            if cm.sub_meshes.len() > needed {
                for mut m in cm.sub_meshes.drain(needed..) {
                    if mesh_pool.len() < MESH_POOL_MAX_CAPACITY {
                        m.vertices.clear();
                        m.indices.clear();
                        mesh_pool.push(m);
                    }
                }
            }

            for (batch_idx, (vert_range, idx_range)) in scratch_partition.iter().enumerate() {
                let min_v = *vert_range.start() as u16;
                let sub_verts = &scratch_vertices[vert_range.clone()];
                let sub_indices = &scratch_indices[idx_range.clone()];

                if batch_idx < cm.sub_meshes.len() {
                    let sub_m = &mut cm.sub_meshes[batch_idx];
                    sub_m.vertices.clear();
                    sub_m.vertices.extend_from_slice(sub_verts);
                    sub_m.indices.clear();
                    sub_m.indices.reserve(sub_indices.len());
                    for &i in sub_indices {
                        sub_m.indices.push(i - min_v);
                    }
                    sub_m.texture = None;
                } else {
                    let mut sub_m = mesh_pool.pop().unwrap_or_else(|| Mesh {
                        vertices: Vec::with_capacity(sub_verts.len()),
                        indices: Vec::with_capacity(sub_indices.len()),
                        texture: None,
                    });
                    sub_m.vertices.clear();
                    sub_m.vertices.extend_from_slice(sub_verts);
                    sub_m.indices.clear();
                    sub_m.indices.reserve(sub_indices.len());
                    for &i in sub_indices {
                        sub_m.indices.push(i - min_v);
                    }
                    sub_m.texture = None;
                    cm.sub_meshes.push(sub_m);
                }
            }
        }
        cm.dirty = false;

        self.mesh_pool = mesh_pool;
        self.scratch_partition_ranges = scratch_partition;
        self.scratch_vertices = scratch_vertices;
        self.scratch_indices = scratch_indices;
    }

    #[inline(always)]
    pub fn chunk_world_aabb(cx: usize, cy: usize, cz: usize) -> (Vec3, Vec3) {
        let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let min = Vec3::new(
            (cx * CHUNK_SIZE) as f32 * VOXEL_SIZE - half_x,
            (MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32) as f32 * VOXEL_SIZE,
            (cz * CHUNK_SIZE) as f32 * VOXEL_SIZE - half_z,
        );
        let max = Vec3::new(
            ((cx + 1) * CHUNK_SIZE) as f32 * VOXEL_SIZE - half_x,
            (MIN_VOXEL_Y + ((cy + 1) * CHUNK_SIZE) as i32) as f32 * VOXEL_SIZE,
            ((cz + 1) * CHUNK_SIZE) as f32 * VOXEL_SIZE - half_z,
        );
        (min, max)
    }

    #[allow(dead_code)]
    pub fn draw(&self) {
        for cm in &self.meshes {
            for sub_mesh in &cm.sub_meshes {
                draw_mesh(sub_mesh);
            }
        }
    }

    pub fn draw_frustum(&self, frustum: &FrustumPlanes) {
        for cx in 0..CHUNKS_X {
            for cy in 0..CHUNKS_Y {
                for cz in 0..CHUNKS_Z {
                    let ci = Self::chunk_idx(cx, cy, cz);
                    let cm = &self.meshes[ci];
                    if cm.sub_meshes.is_empty() {
                        continue;
                    }
                    let (min, max) = Self::chunk_world_aabb(cx, cy, cz);
                    if !frustum.is_aabb_in_frustum(min, max) {
                        continue;
                    }
                    for sub_mesh in &cm.sub_meshes {
                        draw_mesh(sub_mesh);
                    }
                }
            }
        }
    }

#[inline]
fn push_quad_mesh_raw(
    mesh: &mut Mesh,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    color: Color,
) {
    if mesh.vertices.len() + 4 >= 4000 || mesh.indices.len() + 6 >= 4800 {
        draw_mesh(mesh);
        mesh.vertices.clear();
        mesh.indices.clear();
    }
    let v_base = mesh.vertices.len() as u16;
    let c_raw: [u8; 4] = color.into();
    mesh.vertices.push(Vertex { position: p0, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p1, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p2, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p3, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });

    mesh.indices.extend_from_slice(&[
        v_base, v_base + 1, v_base + 2,
        v_base, v_base + 2, v_base + 3,
    ]);
}

    /// Casts radiant dynamic point lighting from muzzle flashes, weapon projectiles,
    /// and explosive shockwaves directly onto adjacent solid building walls using
    /// an 18-directional stencil (6 cardinal + 12 planar edge diagonals).
    /// Batches all surface light decals into a single-pass GPU mesh.
    pub fn draw_dynamic_lighting(&self, lights: &[crate::physics::DynamicPointLight]) {
        if lights.is_empty() || self.solid_count == 0 {
            return;
        }

        let mut mesh = self.light_mesh.borrow_mut();
        mesh.vertices.clear();
        mesh.indices.clear();

        // 18-Direction Stencil: 6 Cardinal + 12 Planar Edge Diagonals
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        const SAMPLE_DIRS: [Vec3; 18] = [
            // 6 Cardinal
            Vec3::X, Vec3::NEG_X,
            Vec3::Y, Vec3::NEG_Y,
            Vec3::Z, Vec3::NEG_Z,
            // 4 XZ Planar Diagonals (Angled walls, diagonal corridors)
            Vec3::new(INV_SQRT2, 0.0, INV_SQRT2),
            Vec3::new(INV_SQRT2, 0.0, -INV_SQRT2),
            Vec3::new(-INV_SQRT2, 0.0, INV_SQRT2),
            Vec3::new(-INV_SQRT2, 0.0, -INV_SQRT2),
            // 4 XY Planar Diagonals (Angled buttresses, roof slopes)
            Vec3::new(INV_SQRT2, INV_SQRT2, 0.0),
            Vec3::new(INV_SQRT2, -INV_SQRT2, 0.0),
            Vec3::new(-INV_SQRT2, INV_SQRT2, 0.0),
            Vec3::new(-INV_SQRT2, -INV_SQRT2, 0.0),
            // 4 YZ Planar Diagonals (Shear walls, ramp inclines)
            Vec3::new(0.0, INV_SQRT2, INV_SQRT2),
            Vec3::new(0.0, INV_SQRT2, -INV_SQRT2),
            Vec3::new(0.0, -INV_SQRT2, INV_SQRT2),
            Vec3::new(0.0, -INV_SQRT2, -INV_SQRT2),
        ];

        const MAX_VOXEL_SURFACE_LIGHTS: usize = 8;
        let active_lights = if lights.len() > MAX_VOXEL_SURFACE_LIGHTS {
            &lights[..MAX_VOXEL_SURFACE_LIGHTS]
        } else {
            lights
        };

        let half_grid_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_grid_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let grid_max_y = (self.max_solid_y.max(1) as f32 + 1.0) * VOXEL_SIZE;

        for light in active_lights {
            if light.intensity < 0.05 || light.radius < 0.4 {
                continue;
            }

            // Fast bounding check: light volume must intersect static grid bounds
            if light.position.x + light.radius < -half_grid_x
                || light.position.x - light.radius > half_grid_x
                || light.position.y + light.radius < VOID_BOUNDARY_Y
                || light.position.y - light.radius > grid_max_y
                || light.position.z + light.radius < -half_grid_z
                || light.position.z - light.radius > half_grid_z
            {
                continue;
            }

            // Fixed-size stack array for zero-allocation hit deduplication
            let mut hit_voxels = [IVec3::splat(i32::MIN); 18];
            let mut hit_count = 0;

            for &dir in &SAMPLE_DIRS {
                // Ray cannot strike any solid voxels if light is above world and ray points up
                if light.position.y > grid_max_y && dir.y > 0.0 {
                    continue;
                }
                if let Some(hit) = self.raycast(light.position, dir, light.radius) {
                    let d = hit.distance;
                    if d >= light.radius {
                        continue;
                    }

                    // Check deduplication
                    let mut already_hit = false;
                    for item in hit_voxels.iter().take(hit_count) {
                        if *item == hit.voxel_pos {
                            already_hit = true;
                            break;
                        }
                    }
                    if already_hit {
                        continue;
                    }
                    if hit_count < hit_voxels.len() {
                        hit_voxels[hit_count] = hit.voxel_pos;
                        hit_count += 1;
                    }

                    let atten = (1.0 - (d / light.radius)).powi(2) * light.intensity;
                    if atten < 0.02 {
                        continue;
                    }

                    // Tight radius clamping prevents penetration through thin interior partitions (<= 1.25m)
                    let spot_r = ((light.radius * light.radius - d * d).max(0.0).sqrt() * 0.45)
                        .clamp(VOXEL_SIZE * 0.40, (light.radius * 0.35).min(1.25));
                    let n = hit.normal;

                    // Standoff offset ensures light decal sits directly on the exterior wall face without Z-fighting
                    let decal_center = hit.world_pos + n * 0.018;

                    let tangent = if n.y.abs() > 0.92 {
                        Vec3::X
                    } else {
                        n.cross(Vec3::Y).normalize_or_zero()
                    };
                    let bitangent = n.cross(tangent).normalize_or_zero();

                    // Radiant outer illumination planar quad
                    let outer_alpha = (atten * 0.24).min(0.50);
                    let outer_col = Color::new(light.color.r, light.color.g, light.color.b, outer_alpha);
                    let p0 = decal_center - tangent * spot_r - bitangent * spot_r;
                    let p1 = decal_center + tangent * spot_r - bitangent * spot_r;
                    let p2 = decal_center + tangent * spot_r + bitangent * spot_r;
                    let p3 = decal_center - tangent * spot_r + bitangent * spot_r;
                    Self::push_quad_mesh_raw(&mut mesh, p0, p1, p2, p3, outer_col);

                    // Luminous incandescent core quad
                    let inner_r = spot_r * 0.38;
                    let inner_alpha = (atten * 0.45).min(0.80);
                    let inner_col = Color::new(
                        (light.color.r * 1.25).min(1.0),
                        (light.color.g * 1.25).min(1.0),
                        (light.color.b * 1.25).min(1.0),
                        inner_alpha,
                    );
                    let core_center = decal_center + n * 0.004;
                    let q0 = core_center - tangent * inner_r - bitangent * inner_r;
                    let q1 = core_center + tangent * inner_r - bitangent * inner_r;
                    let q2 = core_center + tangent * inner_r + bitangent * inner_r;
                    let q3 = core_center - tangent * inner_r + bitangent * inner_r;
                    Self::push_quad_mesh_raw(&mut mesh, q0, q1, q2, q3, inner_col);
                }
            }
        }

        // Consolidated single-pass draw call for all dynamic wall light spots across the scene
        if !mesh.vertices.is_empty() {
            draw_mesh(&mesh);
            mesh.vertices.clear();
            mesh.indices.clear();
        }
    }
}

#[allow(dead_code)]
pub const MAX_CHUNK_BATCH_VERTICES: usize = 3600;
#[allow(dead_code)]
pub const MAX_CHUNK_BATCH_INDICES: usize = 4400;
#[allow(dead_code)]
pub const MESH_POOL_MAX_CAPACITY: usize = 64;

/// Partitions a mesh's index stream into sub-batches where each sub-batch is guaranteed to have:
/// 1. `indices.len() <= max_batch_indices` (default 4400)
/// 2. `(max_vertex - min_vertex + 1) <= max_batch_vertices` (default 3600)
///
/// Populates `out_batches` with `(vertex_range, index_range)` without heap allocation.
#[allow(dead_code)]
pub fn partition_mesh_sub_batches_into(
    indices: &[u16],
    vertices_len: usize,
    max_batch_vertices: usize,
    max_batch_indices: usize,
    out_batches: &mut Vec<(std::ops::RangeInclusive<usize>, std::ops::Range<usize>)>,
) {
    out_batches.clear();
    if indices.is_empty() || vertices_len == 0 {
        return;
    }

    let total_indices = indices.len();
    let mut start = 0;

    while start < total_indices {
        let mut end = start;
        let mut min_v = u16::MAX;
        let mut max_v = 0;

        // Advance triangle-by-triangle (3 indices) guarding BOTH vertex span and index count
        while end + 3 <= total_indices && (end + 3 - start) <= max_batch_indices {
            let i0 = indices[end];
            let i1 = indices[end + 1];
            let i2 = indices[end + 2];
            let next_min = min_v.min(i0).min(i1).min(i2);
            let next_max = max_v.max(i0).max(i1).max(i2);
            if (next_max as usize).saturating_sub(next_min as usize) + 1 > max_batch_vertices {
                break;
            }
            min_v = next_min;
            max_v = next_max;
            end += 3;
        }

        if end == start {
            // Fallback to guarantee progress for anomalous single triangles or non-triangular trailing indices
            end = (start + 3).min(total_indices);
            min_v = indices[start..end].iter().copied().min().unwrap_or(0);
            max_v = indices[start..end].iter().copied().max().unwrap_or(0);
        }

        if min_v <= max_v && (max_v as usize) < vertices_len {
            out_batches.push((min_v as usize..=max_v as usize, start..end));
        }

        start = end;
    }
}

/// Partitions a mesh's index stream into sub-batches returning a list of `(vertex_range, remapped_indices)`.
#[allow(dead_code)]
pub fn partition_mesh_sub_batches(
    indices: &[u16],
    vertices_len: usize,
    max_batch_vertices: usize,
    max_batch_indices: usize,
) -> Vec<(std::ops::RangeInclusive<usize>, Vec<u16>)> {
    let mut batch_ranges = Vec::new();
    partition_mesh_sub_batches_into(
        indices,
        vertices_len,
        max_batch_vertices,
        max_batch_indices,
        &mut batch_ranges,
    );
    let mut batches = Vec::with_capacity(batch_ranges.len());
    for (vert_range, idx_range) in batch_ranges {
        let min_v = *vert_range.start() as u16;
        let sub_indices = &indices[idx_range];
        let remapped_indices: Vec<u16> = sub_indices.iter().map(|&i| i - min_v).collect();
        batches.push((vert_range, remapped_indices));
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_raycast_respects_range_and_inclusive_endpoint() {
        let mut grid = VoxelGrid::new();
        let target = IVec3::new(96, 8, 96);
        grid.set(target.x, target.y, target.z, VoxelType::Concrete);
        let center = grid.voxel_to_world(target);
        let origin = center - Vec3::X * VOXEL_SIZE;
        let distance = VOXEL_SIZE * 0.5;
        assert!(grid.raycast(origin, Vec3::X, distance - 0.001).is_none(), "Ray cannot hit beyond its endpoint");
        let hit = grid.raycast(origin, Vec3::X, distance).expect("Endpoint contact counts as a hit");
        assert!((hit.distance - distance).abs() < 1e-6);
        assert_eq!(hit.voxel_pos, target);
    }

    #[test]
    #[ignore = "Manual reset timing; no machine-dependent threshold"]
    fn benchmark_grid_reset() {
        let mut grid = VoxelGrid::new();
        grid.clear();
        let started = std::time::Instant::now();
        for _ in 0..20 {
            grid.clear();
            std::hint::black_box(&grid);
        }
        println!("20 grid resets: {:?}", started.elapsed());
    }

    #[test]
    fn grid_reset_restores_foundation_and_chunk_occupancy() {
        let mut grid = VoxelGrid::new();
        grid.set(10, MIN_VOXEL_Y, 10, VoxelType::Air);
        grid.set(11, -5, 11, VoxelType::Air);
        grid.set(12, 0, 12, VoxelType::Air);
        grid.set(13, MAX_VOXEL_Y - 1, 13, VoxelType::SteelRebar);
        grid.structural_dirty = true;
        grid.severed_rebar_event = 2;
        grid.clear();
        assert_eq!(grid.get(10, MIN_VOXEL_Y, 10), VoxelType::Bedrock);
        assert_eq!(grid.get(11, -5, 11), VoxelType::Basalt);
        assert_eq!(grid.get(12, 0, 12), VoxelType::Bedrock);
        assert_eq!(grid.get(13, MAX_VOXEL_Y - 1, 13), VoxelType::Air);
        assert_eq!(grid.get_health(10, MIN_VOXEL_Y, 10), 800.0);
        assert_eq!(grid.get_health(11, -5, 11), 500.0);
        assert_eq!(grid.solid_count(), 0);
        assert_eq!(grid.severed_rebar_event, 0);
        assert!(!grid.structural_dirty);
        assert!(grid.dirty_min.is_none() && grid.dirty_max.is_none());
        for cx in 0..CHUNKS_X {
            for cy in 0..CHUNKS_Y {
                for cz in 0..CHUNKS_Z {
                    let expected = if cy < CHUNKS_SUBTERRANEAN_Y { 512 } else if cy == CHUNKS_SUBTERRANEAN_Y { 64 } else { 0 };
                    assert_eq!(grid.chunk_solid_count[VoxelGrid::chunk_idx(cx, cy, cz)], expected);
                }
            }
        }
    }

    #[test]
    fn test_carve_sphere_preserves_bedrock_and_destroys_concrete() {
        let mut grid = VoxelGrid::new();
        assert_eq!(grid.get(32, 0, 32), VoxelType::Bedrock);

        grid.set(32, 1, 32, VoxelType::Concrete);
        grid.set(32, 2, 32, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(32, 2, 32));

        let res = grid.carve_sphere(world_pos, 1.5);
        assert!(res.destroyed >= 2);
        assert_eq!(grid.get(32, 0, 32), VoxelType::Bedrock);
    }

    #[test]
    fn test_rebar_destruction_triggers_severed_event() {
        let mut grid = VoxelGrid::new();
        grid.initial_rebar_count = 2;
        grid.current_rebar_count = 2;
        grid.set(20, 5, 20, VoxelType::SteelRebar);
        grid.set(20, 6, 20, VoxelType::SteelRebar);
        grid.severed_rebar_event = 0;
        grid.severed_rebar_positions.clear();

        let world_pos = grid.voxel_to_world(IVec3::new(20, 5, 20));
        let res = grid.carve_sphere(world_pos, 1.2);
        assert!(res.rebar_destroyed > 0);
        assert!(grid.severed_rebar_event > 0);
        assert!(!res.severed_rebar_positions.is_empty(), "Severed rebar positions must be recorded");
        assert_eq!(grid.severed_rebar_positions.len(), res.severed_rebar_positions.len());
        assert!(grid.current_rebar_count < 2, "Current rebar count must decrease");
    }

    #[test]
    fn test_cryo_freeze_and_shatter() {
        let mut grid = VoxelGrid::new();
        grid.set(15, 3, 15, VoxelType::Concrete);
        grid.set(15, 4, 15, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(15, 3, 15));

        // Freeze concrete into CryoIce
        let frozen = grid.freeze_sphere(world_pos, 1.5);
        assert!(frozen >= 2);
        assert_eq!(grid.get(15, 3, 15), VoxelType::CryoIce);
        assert_eq!(grid.get(15, 4, 15), VoxelType::CryoIce);

        // Carve the ice: must report ice_shattered
        let res = grid.carve_sphere(world_pos, 1.5);
        assert!(res.ice_shattered >= 2);
        assert_eq!(grid.get(15, 3, 15), VoxelType::Air);
        assert_eq!(grid.get(15, 4, 15), VoxelType::Air);
    }

    #[test]
    fn test_island_mass_calculation() {
        let steel_island = IslandChunk {
            voxel_health: Vec::new(),
            voxels: vec![(IVec3::ZERO, VoxelType::SteelRebar)],
            center_of_mass: Vec3::ZERO,
        };
        let glass_island = IslandChunk {
            voxel_health: Vec::new(),
            voxels: vec![(IVec3::ZERO, VoxelType::Glass)],
            center_of_mass: Vec3::ZERO,
        };
        assert!(steel_island.calculate_mass() > glass_island.calculate_mass());
    }

    #[test]
    fn test_brittle_material_extended_shock_radius() {
        let mut grid = VoxelGrid::new();
        let center_v = IVec3::new(30, 10, 30);
        let center_world = grid.voxel_to_world(center_v);

        let ice_v = IVec3::new(30, 10, 34);
        let concrete_v = IVec3::new(30, 10, 34);

        grid.set(ice_v.x, ice_v.y, ice_v.z, VoxelType::CryoIce);
        let res = grid.carve_sphere(center_world, 1.6);
        assert!(res.ice_shattered > 0, "CryoIce within extended brittle shock radius must shatter");
        assert_eq!(grid.get(ice_v.x, ice_v.y, ice_v.z), VoxelType::Air);

        grid.set(concrete_v.x, concrete_v.y, concrete_v.z, VoxelType::Concrete);
        let _ = grid.carve_sphere(center_world, 1.6);
        assert_eq!(grid.get(concrete_v.x, concrete_v.y, concrete_v.z), VoxelType::Concrete, "Concrete outside standard blast radius must be preserved");
    }

    #[test]
    fn test_heavy_armor_blast_threshold_deflection() {
        let mut grid = VoxelGrid::new();
        grid.set(25, 2, 25, VoxelType::HeavyArmor);
        let world_pos = grid.voxel_to_world(IVec3::new(25, 2, 25));

        // Weak blast (damage 20 < blast_threshold 28) should be deflected
        let res = grid.carve_sphere_advanced(world_pos, 1.2, 20.0, DamageType::Explosive);
        assert_eq!(res.destroyed, 0, "Blast under threshold must be deflected by Heavy Armor");
        assert_eq!(grid.get(25, 2, 25), VoxelType::HeavyArmor);
    }

    #[test]
    fn test_damage_accumulation_multi_hit() {
        let mut grid = VoxelGrid::new();
        grid.set(25, 2, 25, VoxelType::HeavyArmor);
        let world_pos = grid.voxel_to_world(IVec3::new(25, 2, 25));

        // HeavyArmor has max_health = 240.0.
        // First high explosive blast deals ~90 effective damage
        let res1 = grid.carve_sphere_advanced(world_pos, 1.2, 100.0, DamageType::Explosive);
        assert_eq!(res1.destroyed, 0, "Single blast must not instantly destroy intact heavy armor");
        assert_eq!(grid.get(25, 2, 25), VoxelType::HeavyArmor);
        let h1 = grid.get_health(25, 2, 25);
        assert!(h1 < 240.0, "Health must degrade after taking damage");

        // Consecutive hits finish it off
        let _ = grid.carve_sphere_advanced(world_pos, 1.2, 280.0, DamageType::Explosive);
        let _ = grid.carve_sphere_advanced(world_pos, 1.2, 280.0, DamageType::Explosive);
        assert_eq!(grid.get(25, 2, 25), VoxelType::Air, "Accumulated damage must ultimately destroy heavy armor");
    }

    #[test]
    fn test_plasma_thermal_softening_counters_armor() {
        let mut grid = VoxelGrid::new();
        grid.set(25, 2, 25, VoxelType::HeavyArmor);
        let world_pos = grid.voxel_to_world(IVec3::new(25, 2, 25));

        // Thermal plasma ignores kinetic blast threshold and melts armor
        let res = grid.carve_sphere_advanced(world_pos, 1.2, 220.0, DamageType::Thermal);
        assert!(res.destroyed > 0, "Thermal plasma must melt through heavy armor");
        assert_eq!(grid.get(25, 2, 25), VoxelType::Air);
    }

    #[test]
    fn test_material_properties_registry() {
        let armor = VoxelType::HeavyArmor.properties();
        let concrete = VoxelType::Concrete.properties();
        let glass = VoxelType::Glass.properties();
        let composite = VoxelType::CompositeRebar.properties();

        assert!(armor.hardness > concrete.hardness);
        assert!(armor.toughness > glass.toughness);
        assert!(armor.density > concrete.density);
        assert!(armor.blast_threshold > 0.0);
        assert!(concrete.compressive_capacity > glass.compressive_capacity);
        assert!(composite.tensile_capacity > concrete.tensile_capacity);
        assert!(armor.compressive_capacity > composite.compressive_capacity);
    }

    #[test]
    fn test_edge_case_out_of_bounds_damage_and_queries() {
        let mut grid = VoxelGrid::new();
        assert_eq!(grid.get(-1, 0, 0), VoxelType::Air);
        assert_eq!(grid.get(GRID_X as i32 + 5, 0, 0), VoxelType::Air);
        assert_eq!(grid.get_health(-1, 0, 0), 0.0);
        assert_eq!(grid.get_health_ratio(-1, 0, 0), 0.0);
        assert!(!grid.damage_voxel(-1, 0, 0, 100.0, DamageType::Explosive));
    }

    #[test]
    fn test_edge_case_bedrock_extreme_damage_destructibility() {
        let mut grid = VoxelGrid::new();
        assert_eq!(grid.get(32, 0, 32), VoxelType::Bedrock);
        assert_eq!(grid.get_health(32, 0, 32), 800.0);
        let destroyed = grid.damage_voxel(32, 0, 32, 10_000_000.0, DamageType::Explosive);
        assert!(destroyed, "Bedrock must be destructible under extreme explosive damage");
        assert_eq!(grid.get(32, 0, 32), VoxelType::Air);
        assert_eq!(grid.get_health(32, 0, 32), 0.0);
    }

    #[test]
    fn test_edge_case_freezing_empty_space() {
        let mut grid = VoxelGrid::new();
        let world_pos = grid.voxel_to_world(IVec3::new(32, 20, 32));
        let frozen = grid.freeze_sphere(world_pos, 2.0);
        assert_eq!(frozen, 0, "Freezing empty air space must not create any ice");
        assert_eq!(grid.get(32, 20, 32), VoxelType::Air);
    }

    #[test]
    fn test_edge_case_micro_radius_carve() {
        let mut grid = VoxelGrid::new();
        grid.set(32, 5, 32, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(32, 5, 32));
        let res = grid.carve_sphere(world_pos, 0.001);
        // Micro-radius should handle gracefully without panic or underflow
        assert!(res.destroyed <= 1);
    }

    #[test]
    fn test_transparent_and_opaque_predicates() {
        assert!(VoxelType::Air.is_transparent());
        assert!(VoxelType::Glass.is_transparent());
        assert!(VoxelType::CryoIce.is_transparent());
        assert!(!VoxelType::Concrete.is_transparent());
        assert!(!VoxelType::HeavyArmor.is_transparent());

        assert!(VoxelType::Concrete.is_opaque());
        assert!(VoxelType::HeavyArmor.is_opaque());
        assert!(VoxelType::SteelRebar.is_opaque());
        assert!(!VoxelType::Glass.is_opaque());
        assert!(!VoxelType::CryoIce.is_opaque());
        assert!(!VoxelType::Air.is_opaque());
    }

    #[test]
    fn test_island_collapse_severed_rebar_reduction() {
        let mut grid = VoxelGrid::new();
        grid.initial_rebar_count = 10;
        grid.current_rebar_count = 10;

        // Place floating island at y=25 (not anchored to bedrock)
        grid.set(30, 25, 30, VoxelType::CompositeRebar);
        grid.set(30, 25, 31, VoxelType::Concrete);

        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Floating structure must be detected as unsupported island");
        assert_eq!(grid.get(30, 25, 30), VoxelType::Air, "Collapsed island voxels must become air");
        assert_eq!(grid.current_rebar_count, 9, "current_rebar_count must be decremented on island collapse");
        assert!(!grid.severed_rebar_positions.is_empty(), "Severed rebar positions must be logged");
    }

    #[test]
    fn test_glow_core_breach_detection() {
        let mut grid = VoxelGrid::new();
        grid.set(20, 10, 20, VoxelType::GlowCore);
        let world_pos = grid.voxel_to_world(IVec3::new(20, 10, 20));

        let res = grid.carve_sphere_advanced(world_pos, 1.5, 300.0, DamageType::Explosive);
        assert!(!res.glow_core_triggers.is_empty(), "Carve must detect destroyed GlowCore");
        assert_eq!(res.glow_core_triggers[0], IVec3::new(20, 10, 20));
        assert_eq!(grid.get(20, 10, 20), VoxelType::Air);
    }

    #[test]
    fn test_damage_voxel_partial_health_marks_chunk_dirty() {
        let mut grid = VoxelGrid::new();
        grid.set(16, 5, 16, VoxelType::HeavyArmor);
        // Clear dirty flags
        grid.update_meshes();
        let cx = 16 / CHUNK_SIZE;
        let cy = ((5 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
        let cz = 16 / CHUNK_SIZE;
        let ci = VoxelGrid::chunk_idx(cx, cy, cz);
        assert!(!grid.meshes[ci].dirty, "Mesh should not be dirty after update");

        // Deal non-lethal damage (50 < 240)
        let destroyed = grid.damage_voxel(16, 5, 16, 50.0, DamageType::Thermal);
        assert!(!destroyed, "50 thermal damage should not destroy HeavyArmor (240 HP)");
        assert!(grid.meshes[ci].dirty, "Chunk mesh MUST be marked dirty on partial damage for visual degradation");
    }

    #[test]
    fn test_raycast_returns_exact_surface_contact_point() {
        let mut grid = VoxelGrid::new();
        grid.set(40, 2, 40, VoxelType::Concrete);
        let center = grid.voxel_to_world(IVec3::new(40, 2, 40));

        // Raycast straight down from above: origin y = 5.0, dir = (0, -1, 0)
        let origin = Vec3::new(center.x, 5.0, center.z);
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let hit = grid.raycast(origin, dir, 10.0).expect("Must hit concrete block");

        // The top face of voxel y=2 is at y = 2 * 0.5 + 0.5 = 1.50
        assert!((hit.world_pos.y - 1.50).abs() < 0.001, "RayHit world_pos must be exactly at top surface y=1.50, got {}", hit.world_pos.y);
        assert_eq!(hit.normal, Vec3::Y, "Normal on top face must be +Y");
        assert_eq!(hit.voxel_pos, IVec3::new(40, 2, 40));

        // Raycast from the front (+Z face): origin z = center.z + 4.0, dir = (0, 0, -1)
        let origin_z = Vec3::new(center.x, center.y, center.z + 4.0);
        let dir_z = Vec3::new(0.0, 0.0, -1.0);
        let hit_z = grid.raycast(origin_z, dir_z, 10.0).expect("Must hit front face");
        let expected_front_z = center.z + VOXEL_SIZE * 0.5;
        assert!((hit_z.world_pos.z - expected_front_z).abs() < 0.001, "RayHit world_pos.z must be at front face, got {}", hit_z.world_pos.z);
        assert_eq!(hit_z.normal, Vec3::Z, "Normal on front face must be +Z");
    }

    #[test]
    fn test_organic_voxel_damage_degradation_shading() {
        let mut grid = VoxelGrid::new();
        grid.set(20, 5, 20, VoxelType::Concrete);
        grid.update_meshes();

        let cx = 20 / CHUNK_SIZE;
        let cy = ((5 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
        let cz = 20 / CHUNK_SIZE;
        let ci = VoxelGrid::chunk_idx(cx, cy, cz);
        let pristine_vertices = grid.meshes[ci].sub_meshes[0].vertices.clone();
        assert!(!pristine_vertices.is_empty());

        // Deal 60% partial damage (42 out of 70 HP)
        grid.damage_voxel(20, 5, 20, 32.0, DamageType::Explosive);
        assert!(grid.get_health_ratio(20, 5, 20) < 1.0);
        grid.update_meshes();

        let damaged_vertices = &grid.meshes[ci].sub_meshes[0].vertices;
        assert_eq!(pristine_vertices.len(), damaged_vertices.len());

        // Verify damaged vertices have darkened, organic soot coloration
        let mut found_darkened = false;
        let mut color_variation = false;
        let first_r = damaged_vertices[0].color[0];
        for (p_v, d_v) in pristine_vertices.iter().zip(damaged_vertices.iter()) {
            if d_v.color[0] < p_v.color[0] {
                found_darkened = true;
            }
            if (d_v.color[0] as i32 - first_r as i32).abs() > 1 {
                color_variation = true;
            }
        }
        assert!(found_darkened, "Damaged voxel mesh vertices must show degradation soot darkening");
        assert!(color_variation, "Damaged voxel corners must exhibit non-uniform organic fracture variation");
    }

    #[test]
    fn test_volumetric_edge_chipping_physically_deforms_mesh() {
        let mut grid = VoxelGrid::new();
        grid.set(16, 5, 16, VoxelType::Concrete);
        grid.update_meshes();

        let cx = 16 / CHUNK_SIZE;
        let cy = ((5 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
        let cz = 16 / CHUNK_SIZE;
        let ci = VoxelGrid::chunk_idx(cx, cy, cz);
        let pristine_vertices = grid.meshes[ci].sub_meshes[0].vertices.clone();
        assert!(!pristine_vertices.is_empty());

        // Deal 75% partial damage (52.5 out of 70 HP)
        grid.damage_voxel(16, 5, 16, 45.0, DamageType::Kinetic);
        grid.update_meshes();

        let damaged_vertices = &grid.meshes[ci].sub_meshes[0].vertices;
        assert_eq!(pristine_vertices.len(), damaged_vertices.len());

        let mut found_displaced_vertices = false;
        let mut max_displacement = 0.0f32;
        for (p_v, d_v) in pristine_vertices.iter().zip(damaged_vertices.iter()) {
            let disp = (d_v.position - p_v.position).length();
            if disp > 0.005 {
                found_displaced_vertices = true;
                max_displacement = max_displacement.max(disp);
            }
        }

        assert!(
            found_displaced_vertices,
            "Damaged voxel corners must physically bevel and chip inward in 3D"
        );
        assert!(
            max_displacement > 0.01 && max_displacement <= VOXEL_SIZE * 0.28,
            "Displacement must be physical and bounded, got {}",
            max_displacement
        );
    }

    #[test]
    fn test_greedy_meshing_merges_coplanar_faces_and_reduces_vertices() {
        let mut grid = VoxelGrid::new();
        // Place a 4x4 flat concrete slab suspended at y = 5 in chunk (2, 0, 2)
        // x in 16..20, z in 16..20, y = 5
        for x in 16..20 {
            for z in 16..20 {
                grid.set(x, 5, z, VoxelType::Concrete);
            }
        }
        grid.update_meshes();

        let cx = 16 / CHUNK_SIZE;
        let cy = ((5 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
        let cz = 16 / CHUNK_SIZE;
        let ci = VoxelGrid::chunk_idx(cx, cy, cz);

        let mesh = &grid.meshes[ci].sub_meshes[0];
        // Unmerged slab: 16 top + 16 bottom + 16 perimeter sides = 48 quads = 192 vertices.
        // Merged slab: 1 top quad (4x4) + 1 bottom quad (4x4) + 4 side quads (4x1 each) = 6 quads = 24 vertices.
        // In G4-M1, exposed bedrock at y=0 is also meshed, contributing 1 merged 8x8 foundation quad = 4 vertices (6 indices).
        assert_eq!(mesh.vertices.len(), 28, "Expected 6 merged slab quads + 1 bedrock quad = 28 vertices");
        assert_eq!(mesh.indices.len(), 42, "Expected 7 merged quads = 42 indices (14 triangles)");

        // All indices must index valid vertices
        for &idx in &mesh.indices {
            assert!((idx as usize) < mesh.vertices.len());
        }
    }

    #[test]
    fn test_greedy_meshing_preserves_damaged_voxel_unmerged_with_chipping() {
        let mut grid = VoxelGrid::new();
        for x in 16..20 {
            for z in 16..20 {
                grid.set(x, 5, z, VoxelType::Concrete);
            }
        }
        grid.update_meshes();

        let cx = 16 / CHUNK_SIZE;
        let cy = ((5 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE;
        let cz = 16 / CHUNK_SIZE;
        let ci = VoxelGrid::chunk_idx(cx, cy, cz);
        let pristine_vertex_count = grid.meshes[ci].sub_meshes[0].vertices.len();
        assert_eq!(pristine_vertex_count, 28);

        // Partially damage an interior voxel at (17, 5, 17)
        grid.damage_voxel(17, 5, 17, 35.0, DamageType::Kinetic);
        grid.update_meshes();

        let damaged_mesh = &grid.meshes[ci].sub_meshes[0];
        // Damaged voxel is broken out from the merged 4x4 faces and rendered as an individual chipped 1x1 quad.
        // The surrounding pristine voxels are partitioned into merged rectangular sub-quads.
        assert!(
            damaged_mesh.vertices.len() > pristine_vertex_count,
            "Damaged voxel unmerging should generate sub-quads for damage detail (expected > 28, got {})",
            damaged_mesh.vertices.len()
        );

        // Verify that vertices include displaced/chipped positions
        let mut found_chipped = false;
        for v in &damaged_mesh.vertices {
            let d_center = grid.voxel_to_world(IVec3::new(17, 5, 17));
            if (v.position - d_center).length() < VOXEL_SIZE * 1.5 {
                found_chipped = true;
                break;
            }
        }
        assert!(found_chipped, "Damaged voxel within greedy mesh must retain 3D chipped geometry");
    }

    #[test]
    fn test_dynamic_lighting_illuminates_adjacent_building_walls() {
        let mut grid = VoxelGrid::new();
        // Create concrete wall at x = 16, y = 5, z in 14..18
        for z in 14..18 {
            grid.set(16, 5, z, VoxelType::Concrete);
        }
        let wall_pos = grid.voxel_to_world(IVec3::new(16, 5, 16));

        // Point light placed 3m in front of the wall
        let light = crate::physics::DynamicPointLight {
            position: wall_pos + Vec3::new(3.0, 0.0, 0.0),
            color: Color::new(1.0, 0.5, 0.2, 1.0),
            radius: 8.0,
            intensity: 1.5,
        };

        // Raycast from light towards the wall (-X) must hit the concrete wall
        let hit = grid.raycast(light.position, Vec3::NEG_X, light.radius).expect("Light must strike wall");
        assert_eq!(hit.voxel_type, VoxelType::Concrete);
        assert!(hit.distance < light.radius);
        assert_eq!(hit.normal, Vec3::X);
    }

    #[test]
    fn test_island_bfs_zero_allocation_and_buffer_reuse() {
        let mut grid = VoxelGrid::new();
        let ptr_grounded = grid.scratch_grounded.as_ptr();
        let ptr_tension = grid.scratch_tension.as_ptr();
        let ptr_depth = grid.scratch_depth.as_ptr();
        let ptr_visited = grid.scratch_visited.as_ptr();

        for i in 0..5 {
            grid.set(20, 10, 20, VoxelType::Concrete);
            grid.set(20, 11, 20, VoxelType::Concrete);
            let _ = grid.detect_unsupported_islands();

            assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "grounded buffer reallocated on iter {i}");
            assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "tension buffer reallocated on iter {i}");
            assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "depth buffer reallocated on iter {i}");
            assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "visited buffer reallocated on iter {i}");
        }
    }

    #[test]
    fn test_island_bfs_clean_reset_no_stale_state() {
        let mut grid = VoxelGrid::new();
        grid.set(15, 8, 15, VoxelType::Brick);
        grid.set(15, 9, 15, VoxelType::Brick);
        let islands1 = grid.detect_unsupported_islands();
        assert_eq!(islands1.len(), 1);
        assert_eq!(grid.get(15, 8, 15), VoxelType::Air);
        assert_eq!(grid.get(15, 9, 15), VoxelType::Air);

        let islands2 = grid.detect_unsupported_islands();
        assert_eq!(islands2.len(), 0);

        grid.set(15, 1, 15, VoxelType::Concrete);
        grid.set(15, 2, 15, VoxelType::Concrete);
        let islands3 = grid.detect_unsupported_islands();
        assert_eq!(islands3.len(), 0);
        assert_eq!(grid.get(15, 1, 15), VoxelType::Concrete);
        assert_eq!(grid.get(15, 2, 15), VoxelType::Concrete);
    }

    #[test]
    fn test_solid_count_incremental_synchronization() {
        let mut grid = VoxelGrid::new();
        let initial_solid = grid.solid_count();
        assert_eq!(initial_solid, 0);

        grid.set(10, 5, 10, VoxelType::Concrete);
        assert_eq!(grid.solid_count(), initial_solid + 1);

        grid.set(10, 6, 10, VoxelType::Concrete);
        assert_eq!(grid.solid_count(), initial_solid + 2);

        grid.set(10, 6, 10, VoxelType::SteelRebar);
        assert_eq!(grid.solid_count(), initial_solid + 2);

        grid.set(10, 6, 10, VoxelType::Air);
        assert_eq!(grid.solid_count(), initial_solid + 1);

        grid.damage_voxel(10, 5, 10, 10000.0, DamageType::Explosive);
        assert_eq!(grid.get(10, 5, 10), VoxelType::Air);
        assert_eq!(grid.solid_count(), initial_solid);

        grid.set(20, 15, 20, VoxelType::Brick);
        grid.set(20, 16, 20, VoxelType::Brick);
        assert_eq!(grid.solid_count(), initial_solid + 2);
        let islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 1);
        assert_eq!(grid.solid_count(), initial_solid);

        grid.set(30, 2, 30, VoxelType::Concrete);
        assert_eq!(grid.solid_count(), initial_solid + 1);
        grid.clear();
        assert_eq!(grid.solid_count(), initial_solid);
    }

    #[test]
    fn test_beam_carve_cylinder_into_accumulates_without_churn() {
        let mut grid = VoxelGrid::new();
        for x in 10..=20 {
            grid.set(x, 5, 10, VoxelType::Concrete);
        }
        grid.set(15, 5, 10, VoxelType::GlowCore);

        let start = grid.voxel_to_world(IVec3::new(10, 5, 10));
        let end = grid.voxel_to_world(IVec3::new(20, 5, 10));

        let mut result = CarveResult::with_capacity(128);
        grid.carve_cylinder_advanced_into(start, end, 1.5, 500.0, DamageType::Thermal, &mut result);

        assert!(result.destroyed > 0, "Carve must destroy voxels");
        assert!(!result.glow_core_triggers.is_empty(), "Carve must detect destroyed GlowCore");
    }

    #[test]
    fn test_single_voxel_debris_preservation() {
        let mut grid = VoxelGrid::new();
        grid.set_preserve_single_voxel_debris(true);
        grid.set(25, 20, 25, VoxelType::Concrete);

        let islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 1, "Single-voxel debris must produce an IslandChunk when enabled");
        assert_eq!(islands[0].voxels.len(), 1);
        let expected_center = grid.voxel_to_world(IVec3::new(25, 20, 25));
        assert_eq!(islands[0].center_of_mass, expected_center);
        assert_eq!(grid.get(25, 20, 25), VoxelType::Air);
    }

    #[test]
    fn test_cantilever_depth_bounded_at_four() {
        let mut grid = VoxelGrid::new();
        // Grounded pillar from bedrock y=0 up to y=10 at x=10, z=10
        for y in 1..=10 {
            grid.set(10, y, 10, VoxelType::Concrete);
        }
        // Cantilever beam extending horizontally in +X at y=10
        // x=11: depth 1
        // x=12: depth 2
        // x=13: depth 3
        // x=14: depth 4 (supported)
        // x=15: depth 5 (unsupported > 4)
        // x=16: depth 6 (unsupported > 4)
        for x in 11..=16 {
            grid.set(x, 10, 10, VoxelType::Concrete);
        }

        let islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 1, "Cantilever voxels beyond depth 4 should collapse as an island");
        // x=15 and x=16 collapsed
        assert_eq!(islands[0].voxels.len(), 2);
        assert_eq!(grid.get(15, 10, 10), VoxelType::Air);
        assert_eq!(grid.get(16, 10, 10), VoxelType::Air);

        // x=11 through x=14 remain supported
        for x in 11..=14 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Concrete, "Voxel at x={x} should remain supported");
        }
    }

    #[test]
    fn test_rebar_extended_tension_bridging() {
        let mut grid = VoxelGrid::new();
        // Grounded pillar up to y=10
        for y in 1..=10 {
            grid.set(10, y, 10, VoxelType::Concrete);
        }
        // Steel rebar extending 8 voxels horizontally (> CANTILEVER_MAX_DEPTH)
        for x in 11..=18 {
            grid.set(x, 10, 10, VoxelType::SteelRebar);
        }

        let islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 0, "Steel rebar cantilever should bridge tension and not collapse");
        for x in 11..=18 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::SteelRebar);
        }
    }

    #[test]
    fn test_island_bfs_adversarial_scratch_buffers_pointer_stability_and_zero_churn() {
        let mut grid = VoxelGrid::new();
        let total_voxels = GRID_X * GRID_Y * GRID_Z;

        let ptr_grounded = grid.scratch_grounded.as_ptr();
        let ptr_tension = grid.scratch_tension.as_ptr();
        let ptr_depth = grid.scratch_depth.as_ptr();
        let ptr_visited = grid.scratch_visited.as_ptr();
        let initial_queue_cap = grid.scratch_queue.capacity();

        assert_eq!(grid.scratch_grounded.capacity(), total_voxels);
        assert_eq!(grid.scratch_tension.capacity(), total_voxels);
        assert_eq!(grid.scratch_depth.capacity(), total_voxels);
        assert_eq!(grid.scratch_visited.capacity(), total_voxels);
        assert!(initial_queue_cap >= 32768);

        // Adversarial multi-cycle structural severance stress test
        for cycle in 0..10 {
            // Build a multi-story structure with 4 columns, 2 slab floors, and rebar cantilevers
            for y in 0..=12 {
                grid.set(10, y, 10, VoxelType::Concrete);
                grid.set(10, y, 20, VoxelType::Concrete);
                grid.set(20, y, 10, VoxelType::Concrete);
                grid.set(20, y, 20, VoxelType::Concrete);
            }
            // Floors at y=6 and y=12 with reinforced rebar cross-beams
            for x in 10..=20 {
                for z in 10..=20 {
                    grid.set(x, 6, z, VoxelType::Concrete);
                    grid.set(x, 12, z, VoxelType::Concrete);
                }
            }
            // Perimeter rebar girders directly connect all 4 corner columns at y=6 and y=12
            for x in 10..=20 {
                grid.set(x, 6, 10, VoxelType::SteelRebar);
                grid.set(x, 6, 20, VoxelType::SteelRebar);
                grid.set(x, 12, 10, VoxelType::SteelRebar);
                grid.set(x, 12, 20, VoxelType::SteelRebar);
            }
            for z in 10..=20 {
                grid.set(10, 6, z, VoxelType::SteelRebar);
                grid.set(20, 6, z, VoxelType::SteelRebar);
                grid.set(10, 12, z, VoxelType::SteelRebar);
                grid.set(20, 12, z, VoxelType::SteelRebar);
            }
            // Interior rebar cross-trusses connecting perimeter to center
            for x in 10..=20 {
                grid.set(x, 6, 15, VoxelType::SteelRebar);
                grid.set(x, 12, 15, VoxelType::SteelRebar);
            }
            for z in 10..=20 {
                grid.set(15, 6, z, VoxelType::SteelRebar);
                grid.set(15, 12, z, VoxelType::SteelRebar);
            }
            // Cantilever balconies extending 3 voxels out (supported <= 4)
            for x in 21..=23 {
                for z in 12..=18 {
                    grid.set(x, 12, z, VoxelType::Concrete);
                }
            }
            // Extended rebar bridge extending 8 voxels out (indefinite bridging)
            for z in 21..=28 {
                grid.set(15, 12, z, VoxelType::SteelRebar);
            }

            // Sub-cycle A: Verify stable structure has 0 islands
            let islands_intact = grid.detect_unsupported_islands();
            assert_eq!(islands_intact.len(), 0, "Intact structure must have 0 islands on cycle {cycle}");

            // Verify pointer equality and zero buffer reallocation
            assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded reallocated on cycle {cycle} intact");
            assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension reallocated on cycle {cycle} intact");
            assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth reallocated on cycle {cycle} intact");
            assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited reallocated on cycle {cycle} intact");
            assert_eq!(grid.scratch_queue.len(), 0);
            assert!(grid.scratch_queue.capacity() >= initial_queue_cap);

            // Sub-cycle B: Partial foundational severance (sever column 1 and 2 at y=1..=3)
            for y in 1..=3 {
                grid.set(10, y, 10, VoxelType::Air);
                grid.set(10, y, 20, VoxelType::Air);
            }
            let _ = grid.detect_unsupported_islands();

            assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded reallocated on cycle {cycle} partial severance");
            assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension reallocated on cycle {cycle} partial severance");
            assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth reallocated on cycle {cycle} partial severance");
            assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited reallocated on cycle {cycle} partial severance");
            assert_eq!(grid.scratch_queue.len(), 0);

            // Sub-cycle C: Catastrophic total foundational severance (sever remaining columns at y=1..=3)
            for y in 1..=3 {
                grid.set(20, y, 10, VoxelType::Air);
                grid.set(20, y, 20, VoxelType::Air);
            }
            let islands_collapse = grid.detect_unsupported_islands();
            assert!(!islands_collapse.is_empty(), "Severed structure must yield unsupported islands on cycle {cycle}");

            assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded reallocated on cycle {cycle} catastrophic");
            assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension reallocated on cycle {cycle} catastrophic");
            assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth reallocated on cycle {cycle} catastrophic");
            assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited reallocated on cycle {cycle} catastrophic");
            assert_eq!(grid.scratch_queue.len(), 0);

            // Sub-cycle D: Clean post-collapse state (subsequent call yields 0 islands)
            let islands_empty = grid.detect_unsupported_islands();
            assert_eq!(islands_empty.len(), 0, "Empty post-collapse state must produce 0 islands on cycle {cycle}");

            assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded reallocated on cycle {cycle} post-collapse");
            assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension reallocated on cycle {cycle} post-collapse");
            assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth reallocated on cycle {cycle} post-collapse");
            assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited reallocated on cycle {cycle} post-collapse");
            assert_eq!(grid.scratch_queue.len(), 0);

            grid.clear();
        }
    }

    #[test]
    fn test_cantilever_adversarial_spans_and_rebar_grid_and_coplanar_diagonals() {
        let mut grid = VoxelGrid::new();

        // 1. Masonry Cantilever Spans: Lengths 3, 4, 5, 8
        // Grounded pillar up to y=10
        for y in 1..=10 {
            grid.set(10, y, 10, VoxelType::Concrete);
        }

        // Span Length 3 (x=11..=13): all supported
        for x in 11..=13 {
            grid.set(x, 10, 10, VoxelType::Concrete);
        }
        let islands_len3 = grid.detect_unsupported_islands();
        assert_eq!(islands_len3.len(), 0, "Cantilever length 3 must remain supported");
        for x in 11..=13 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Concrete);
        }

        // Span Length 4 (x=11..=14): all supported
        grid.set(14, 10, 10, VoxelType::Concrete);
        let islands_len4 = grid.detect_unsupported_islands();
        assert_eq!(islands_len4.len(), 0, "Cantilever length 4 must remain supported");
        for x in 11..=14 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Concrete);
        }

        // Span Length 5 (x=11..=15): x=15 detaches strictly (> 4)
        grid.set(15, 10, 10, VoxelType::Concrete);
        grid.set_preserve_single_voxel_debris(true);
        let islands_len5 = grid.detect_unsupported_islands();
        assert_eq!(islands_len5.len(), 1, "Cantilever length 5 must detach the 5th voxel");
        assert_eq!(islands_len5[0].voxels.len(), 1);
        assert_eq!(islands_len5[0].voxels[0].0, IVec3::new(15, 10, 10));
        for x in 11..=14 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Concrete);
        }
        assert_eq!(grid.get(15, 10, 10), VoxelType::Air);

        // Span Length 8 (x=11..=18): voxels 15..=18 detach strictly (4 voxels)
        for x in 15..=18 {
            grid.set(x, 10, 10, VoxelType::Brick);
        }
        let islands_len8 = grid.detect_unsupported_islands();
        assert_eq!(islands_len8.len(), 1, "Cantilever length 8 must detach all voxels beyond depth 4");
        assert_eq!(islands_len8[0].voxels.len(), 4);
        for x in 11..=14 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Concrete);
        }
        for x in 15..=18 {
            assert_eq!(grid.get(x, 10, 10), VoxelType::Air);
        }

        // Multi-directional Cardinal Symmetry (-X, +Z, -Z)
        // -X direction: x=9..=6 supported (4 voxels), x=5 detaches
        for x in 6..=9 {
            grid.set(x, 10, 10, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(5, 10, 10, VoxelType::Concrete);
        let islands_neg_x = grid.detect_unsupported_islands();
        assert_eq!(islands_neg_x.len(), 1);
        assert_eq!(islands_neg_x[0].voxels[0].0, IVec3::new(5, 10, 10));

        // +Z direction: z=11..=14 supported (4 voxels), z=15 detaches
        for z in 11..=14 {
            grid.set(10, 10, z, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(10, 10, 15, VoxelType::Concrete);
        let islands_pos_z = grid.detect_unsupported_islands();
        assert_eq!(islands_pos_z.len(), 1);
        assert_eq!(islands_pos_z[0].voxels[0].0, IVec3::new(10, 10, 15));

        // -Z direction: z=9..=6 supported (4 voxels), z=5 detaches
        for z in 6..=9 {
            grid.set(10, 10, z, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(10, 10, 5, VoxelType::Concrete);
        let islands_neg_z = grid.detect_unsupported_islands();
        assert_eq!(islands_neg_z.len(), 1);
        assert_eq!(islands_neg_z[0].voxels[0].0, IVec3::new(10, 10, 5));

        // 2. Rebar Indefinite Bridging (lengths 8, 12, 16)
        grid.clear();
        for y in 1..=10 {
            grid.set(30, y, 30, VoxelType::Concrete);
        }
        // Length 8 rebar beam
        for x in 31..=38 {
            grid.set(x, 10, 30, VoxelType::SteelRebar);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Steel rebar length 8 must bridge indefinitely");

        // Extend to Length 16 rebar beam
        for x in 39..=46 {
            grid.set(x, 10, 30, VoxelType::SteelRebar);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Steel rebar length 16 must bridge indefinitely");
        for x in 31..=46 {
            assert_eq!(grid.get(x, 10, 30), VoxelType::SteelRebar);
        }

        // Composite rebar length 12
        for z in 31..=42 {
            grid.set(30, 10, z, VoxelType::CompositeRebar);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Composite rebar length 12 must bridge indefinitely");
        for z in 31..=42 {
            assert_eq!(grid.get(30, 10, z), VoxelType::CompositeRebar);
        }

        // 3. Rebar 2D Grid
        grid.clear();
        for y in 1..=10 {
            grid.set(10, y, 20, VoxelType::Concrete);
            grid.set(10, y, 28, VoxelType::Concrete);
        }
        // 9x9 Rebar grid spanning x=10..=18, z=20..=28 at y=10
        for x in 10..=18 {
            for z in 20..=28 {
                grid.set(x, 10, z, VoxelType::SteelRebar);
            }
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Steel rebar 2D grid must bridge under tension");
        for x in 10..=18 {
            for z in 20..=28 {
                assert_eq!(grid.get(x, 10, z), VoxelType::SteelRebar);
            }
        }

        // Concrete deck placed on top of rebar grid at y=11
        for x in 11..=17 {
            for z in 21..=27 {
                grid.set(x, 11, z, VoxelType::Concrete);
            }
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Concrete deck on top of rebar grid must remain supported");

        // Sever the anchor connection: demolish supporting columns completely
        for y in 1..=10 {
            grid.set(10, y, 20, VoxelType::Air);
            grid.set(10, y, 28, VoxelType::Air);
        }
        for z in 20..=28 {
            grid.set(10, 10, z, VoxelType::Air);
        }
        let islands_severed_grid = grid.detect_unsupported_islands();
        assert!(!islands_severed_grid.is_empty(), "Severed rebar grid + concrete deck must collapse as unsupported island");

        // 4. Coplanar Diagonals Eliminating Manhattan Distortion
        // Vector (+1, 0, +1):
        grid.clear();
        for y in 1..=10 {
            grid.set(25, y, 25, VoxelType::Concrete);
        }
        // Diagonal chain of 4 voxels: (26, 10, 26) through (29, 10, 29)
        // Under Manhattan metric |dx|+|dz|, (29, 10, 29) is at distance 8 and would collapse!
        // With coplanar diagonals, graph depth is 4 <= CANTILEVER_MAX_DEPTH.
        for k in 1..=4 {
            grid.set(25 + k, 10, 25 + k, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0, "Length 4 coplanar diagonal cantilever must remain supported");
        for k in 1..=4 {
            assert_eq!(grid.get(25 + k, 10, 25 + k), VoxelType::Concrete);
        }

        // Add 5th diagonal voxel at (30, 10, 30) (depth 5 > 4)
        grid.set(30, 10, 30, VoxelType::Concrete);
        grid.set_preserve_single_voxel_debris(true);
        let islands_diag_pos_pos = grid.detect_unsupported_islands();
        assert_eq!(islands_diag_pos_pos.len(), 1, "Length 5 coplanar diagonal must detach the 5th voxel");
        assert_eq!(islands_diag_pos_pos[0].voxels[0].0, IVec3::new(30, 10, 30));
        assert_eq!(grid.get(30, 10, 30), VoxelType::Air);
        for k in 1..=4 {
            assert_eq!(grid.get(25 + k, 10, 25 + k), VoxelType::Concrete);
        }

        // Vector (+1, 0, -1)
        grid.clear();
        for y in 1..=10 {
            grid.set(25, y, 25, VoxelType::Concrete);
        }
        for k in 1..=4 {
            grid.set(25 + k, 10, 25 - k, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(30, 10, 20, VoxelType::Concrete);
        let islands_diag_pos_neg = grid.detect_unsupported_islands();
        assert_eq!(islands_diag_pos_neg.len(), 1);
        assert_eq!(islands_diag_pos_neg[0].voxels[0].0, IVec3::new(30, 10, 20));

        // Vector (-1, 0, +1)
        grid.clear();
        for y in 1..=10 {
            grid.set(25, y, 25, VoxelType::Concrete);
        }
        for k in 1..=4 {
            grid.set(25 - k, 10, 25 + k, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(20, 10, 30, VoxelType::Concrete);
        let islands_diag_neg_pos = grid.detect_unsupported_islands();
        assert_eq!(islands_diag_neg_pos.len(), 1);
        assert_eq!(islands_diag_neg_pos[0].voxels[0].0, IVec3::new(20, 10, 30));

        // Vector (-1, 0, -1)
        grid.clear();
        for y in 1..=10 {
            grid.set(25, y, 25, VoxelType::Concrete);
        }
        for k in 1..=4 {
            grid.set(25 - k, 10, 25 - k, VoxelType::Concrete);
        }
        assert_eq!(grid.detect_unsupported_islands().len(), 0);
        grid.set(20, 10, 20, VoxelType::Concrete);
        let islands_diag_neg_neg = grid.detect_unsupported_islands();
        assert_eq!(islands_diag_neg_neg.len(), 1);
        assert_eq!(islands_diag_neg_neg[0].voxels[0].0, IVec3::new(20, 10, 20));
    }

    #[test]
    fn test_default_voxel_grid_implementation() {
        let grid = VoxelGrid::default();
        assert_eq!(grid.solid_count(), 0);
    }

    #[test]
    fn test_solid_count_adversarial_stress() {
        struct DeterministicRng {
            state: u64,
        }

        impl DeterministicRng {
            fn new(seed: u64) -> Self {
                Self { state: seed.max(1) }
            }

            fn next_u32(&mut self) -> u32 {
                self.state ^= self.state << 13;
                self.state ^= self.state >> 7;
                self.state ^= self.state << 17;
                self.state as u32
            }

            fn gen_range(&mut self, min: i32, max: i32) -> i32 {
                if min >= max {
                    return min;
                }
                let span = (max - min + 1) as u32;
                min + (self.next_u32() % span) as i32
            }

            fn gen_f32(&mut self) -> f32 {
                (self.next_u32() % 10_000) as f32 / 10_000.0
            }
        }

        fn ground_truth_solid_count(grid: &VoxelGrid) -> usize {
            let mut count = 0;
            for x in 0..GRID_X {
                for y in 1..MAX_VOXEL_Y {
                    for z in 0..GRID_Z {
                        if grid.get(x as i32, y, z as i32).is_solid() {
                            count += 1;
                        }
                    }
                }
            }
            count
        }

        let mut grid = VoxelGrid::new();
        let mut rng = DeterministicRng::new(0x00C0_FFEE_BEEF_1234);

        assert_eq!(grid.solid_count(), 0);
        assert_eq!(grid.solid_count(), ground_truth_solid_count(&grid));

        let voxel_palette = [
            VoxelType::Air,
            VoxelType::Concrete,
            VoxelType::SteelRebar,
            VoxelType::CompositeRebar,
            VoxelType::HeavyArmor,
            VoxelType::Glass,
            VoxelType::CryoIce,
            VoxelType::Brick,
            VoxelType::Explosive,
            VoxelType::GlowCore,
            VoxelType::PowerCapacitor,
            VoxelType::Bedrock,
        ];

        let damage_types = [
            DamageType::Explosive,
            DamageType::Kinetic,
            DamageType::Thermal,
            DamageType::Cryo,
            DamageType::Concussive,
        ];

        // 600 random adversarial operations
        for step in 0..600 {
            let op = rng.gen_range(0, 6);
            match op {
                0 => {
                    // Random set (including bedrock boundary y=0 and out of bounds)
                    let x = rng.gen_range(-2, GRID_X as i32 + 2);
                    let y = rng.gen_range(0, GRID_Y as i32 + 2);
                    let z = rng.gen_range(-2, GRID_Z as i32 + 2);
                    let vt = voxel_palette[rng.gen_range(0, voxel_palette.len() as i32 - 1) as usize];
                    grid.set(x, y, z, vt);
                }
                1 => {
                    // Random damage_voxel
                    let x = rng.gen_range(0, GRID_X as i32 - 1);
                    let y = rng.gen_range(0, GRID_Y as i32 - 1);
                    let z = rng.gen_range(0, GRID_Z as i32 - 1);
                    let dmg = rng.gen_f32() * 500.0;
                    let dtype = damage_types[rng.gen_range(0, damage_types.len() as i32 - 1) as usize];
                    grid.damage_voxel(x, y, z, dmg, dtype);
                }
                2 => {
                    // Random sphere carve
                    let cx = rng.gen_range(5, GRID_X as i32 - 5);
                    let cy = rng.gen_range(0, GRID_Y as i32 - 5);
                    let cz = rng.gen_range(5, GRID_Z as i32 - 5);
                    let center = grid.voxel_to_world(IVec3::new(cx, cy, cz));
                    let radius = 0.5 + rng.gen_f32() * 4.0;
                    let dmg = 50.0 + rng.gen_f32() * 450.0;
                    let dtype = damage_types[rng.gen_range(0, damage_types.len() as i32 - 1) as usize];
                    grid.carve_sphere_advanced(center, radius, dmg, dtype);
                }
                3 => {
                    // Random cylinder carve
                    let sx = rng.gen_range(10, GRID_X as i32 - 10);
                    let sy = rng.gen_range(2, GRID_Y as i32 - 10);
                    let sz = rng.gen_range(10, GRID_Z as i32 - 10);
                    let ex = rng.gen_range(10, GRID_X as i32 - 10);
                    let ey = rng.gen_range(2, GRID_Y as i32 - 10);
                    let ez = rng.gen_range(10, GRID_Z as i32 - 10);
                    let start = grid.voxel_to_world(IVec3::new(sx, sy, sz));
                    let end = grid.voxel_to_world(IVec3::new(ex, ey, ez));
                    let radius = 0.5 + rng.gen_f32() * 2.5;
                    let dmg = 80.0 + rng.gen_f32() * 300.0;
                    let dtype = damage_types[rng.gen_range(0, damage_types.len() as i32 - 1) as usize];
                    grid.carve_cylinder_advanced(start, end, radius, dmg, dtype);
                }
                4 => {
                    // Freeze sphere
                    let cx = rng.gen_range(5, GRID_X as i32 - 5);
                    let cy = rng.gen_range(1, GRID_Y as i32 - 5);
                    let cz = rng.gen_range(5, GRID_Z as i32 - 5);
                    let center = grid.voxel_to_world(IVec3::new(cx, cy, cz));
                    let radius = 1.0 + rng.gen_f32() * 3.0;
                    grid.freeze_sphere(center, radius);
                }
                5 => {
                    // Unsupported island detection (collapse)
                    let preserve_debris = rng.gen_range(0, 1) == 1;
                    grid.set_preserve_single_voxel_debris(preserve_debris);
                    grid.detect_unsupported_islands();
                }
                _ => {
                    // Clear
                    grid.clear();
                }
            }

            let gt = ground_truth_solid_count(&grid);
            assert_eq!(
                grid.solid_count(),
                gt,
                "solid_count ({}) diverged from ground truth ({}) at step {}",
                grid.solid_count(),
                gt,
                step
            );
            assert_eq!(
                grid.current_voxels,
                gt,
                "current_voxels ({}) diverged from ground truth ({}) at step {}",
                grid.current_voxels,
                gt,
                step
            );
        }
    }

    #[test]
    fn test_beam_carve_long_cylinder_adversarial_stress() {
        let mut grid = VoxelGrid::new();

        // 1. Long beam across the entire volume cutting explosive canisters and glow cores
        // Beam length: 100 meters, radius: 0.75m -> step_dist = 0.5625 -> 178 steps
        let start = Vec3::new(-50.0, 7.5, 0.0);
        let end = Vec3::new(50.0, 7.5, 0.0);
        let len = (end - start).length();
        let step_dist = (0.75f32 * 0.75).max(VOXEL_SIZE * 0.5);
        let steps = (len / step_dist).ceil() as usize;
        assert_eq!(steps, 178, "100m beam with radius 0.75 must produce exactly 178 steps");

        // Populate voxels along the beam path
        // At y=15 (7.5m / 0.5m = 15) and z=cz (0.0m world center -> z=cz)
        let beam_y = 15;
        let cx = GRID_X as i32 / 2;
        let cz = GRID_Z as i32 / 2;
        let beam_z = cz;

        // Ground pillar so structures don't collapse beforehand
        for y in 1..=beam_y {
            grid.set(cx - 28, y, beam_z, VoxelType::Concrete);
            grid.set(cx + 22, y, beam_z, VoxelType::Concrete);
        }

        // Slab along beam
        for x in (cx - 28)..=(cx + 22) {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    grid.set(x, beam_y + dy, beam_z + dz, VoxelType::Concrete);
                }
            }
        }

        // Embed specific test elements along the beam
        let canister1_pos = IVec3::new(cx - 23, beam_y, beam_z);
        let glow1_pos = IVec3::new(cx - 13, beam_y, beam_z);
        let canister2_pos = IVec3::new(cx - 3, beam_y, beam_z);
        let glow2_pos = IVec3::new(cx + 7, beam_y, beam_z);
        let rebar_pos = IVec3::new(cx + 12, beam_y, beam_z);
        let armor_pos = IVec3::new(cx + 14, beam_y, beam_z);
        let ice_pos = IVec3::new(cx + 17, beam_y, beam_z);

        grid.set(canister1_pos.x, canister1_pos.y, canister1_pos.z, VoxelType::Explosive);
        grid.set(glow1_pos.x, glow1_pos.y, glow1_pos.z, VoxelType::GlowCore);
        grid.set(canister2_pos.x, canister2_pos.y, canister2_pos.z, VoxelType::Explosive);
        grid.set(glow2_pos.x, glow2_pos.y, glow2_pos.z, VoxelType::GlowCore);
        grid.set(rebar_pos.x, rebar_pos.y, rebar_pos.z, VoxelType::SteelRebar);
        grid.set(armor_pos.x, armor_pos.y, armor_pos.z, VoxelType::HeavyArmor);
        grid.set(ice_pos.x, ice_pos.y, ice_pos.z, VoxelType::CryoIce);

        let initial_solid = grid.solid_count();
        assert!(initial_solid > 0);

        // Run long cylinder carve (100m, 178 steps)
        let result = grid.carve_cylinder_advanced(start, end, 0.75, 500.0, DamageType::Thermal);

        // Verify destruction metrics
        assert!(result.destroyed > 0, "Long beam must destroy voxels");
        assert!(
            result.glow_core_triggers.contains(&glow1_pos),
            "Must capture first glow core trigger at {:?}",
            glow1_pos
        );
        assert!(
            result.glow_core_triggers.contains(&glow2_pos),
            "Must capture second glow core trigger at {:?}",
            glow2_pos
        );
        assert!(
            result.explosive_triggers.contains(&canister1_pos),
            "Must capture first canister trigger at {:?}",
            canister1_pos
        );
        assert!(
            result.explosive_triggers.contains(&canister2_pos),
            "Must capture second canister trigger at {:?}",
            canister2_pos
        );
        assert!(result.rebar_destroyed >= 1, "Must destroy rebar");
        assert!(result.armor_destroyed >= 1, "Must destroy heavy armor");
        assert!(result.ice_shattered >= 1, "Must shatter cryo ice");

        // Verify triggers contain NO duplicate entries
        let mut unique_glow = result.glow_core_triggers.clone();
        unique_glow.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        unique_glow.dedup();
        assert_eq!(
            result.glow_core_triggers.len(),
            unique_glow.len(),
            "glow_core_triggers must not contain duplicate events across beam steps"
        );

        let mut unique_exp = result.explosive_triggers.clone();
        unique_exp.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        unique_exp.dedup();
        assert_eq!(
            result.explosive_triggers.len(),
            unique_exp.len(),
            "explosive_triggers must not contain duplicate events across beam steps"
        );

        // Verify grid state
        assert_eq!(grid.get(glow1_pos.x, glow1_pos.y, glow1_pos.z), VoxelType::Air);
        assert_eq!(grid.get(glow2_pos.x, glow2_pos.y, glow2_pos.z), VoxelType::Air);
        assert_eq!(grid.get(canister1_pos.x, canister1_pos.y, canister1_pos.z), VoxelType::Air);
        assert_eq!(grid.get(canister2_pos.x, canister2_pos.y, canister2_pos.z), VoxelType::Air);

        // Verify solid_count synchronization with ground truth
        let mut ground_truth = 0;
        for x in 0..GRID_X {
            for y in 1..MAX_VOXEL_Y {
                for z in 0..GRID_Z {
                    if grid.get(x as i32, y, z as i32).is_solid() {
                        ground_truth += 1;
                    }
                }
            }
        }
        assert_eq!(grid.solid_count(), ground_truth);

        // 2. Adversarial Edge Case: Degenerate 0-length beam
        let zero_res = grid.carve_cylinder_advanced(start, start, 0.75, 500.0, DamageType::Kinetic);
        assert_eq!(zero_res.destroyed, 0, "0-length beam must return empty CarveResult without panic");

        // 3. Adversarial Edge Case: Completely out of bounds beam
        let oob_start = Vec3::new(-200.0, 100.0, -200.0);
        let oob_end = Vec3::new(-100.0, 100.0, -100.0);
        let oob_res = grid.carve_cylinder_advanced(oob_start, oob_end, 1.5, 500.0, DamageType::Explosive);
        assert_eq!(oob_res.destroyed, 0, "Out of bounds beam must not destroy voxels or panic");

        // 4. Subterranean Destructibility: High energy beam carves trench through Bedrock at y=0
        let bedrock_start = Vec3::new(-10.0, 0.0, 0.0);
        let bedrock_end = Vec3::new(10.0, 0.0, 0.0);
        let beam_res = grid.carve_cylinder_advanced(bedrock_start, bedrock_end, 2.0, 2000.0, DamageType::Thermal);
        assert!(beam_res.destroyed > 0, "High energy thermal beam must carve through destructible bedrock");
    }

    #[test]
    fn test_power_capacitor_material_properties_and_conductivity() {
        let cap = VoxelType::PowerCapacitor;
        let props = cap.properties();
        assert_eq!(props.conductivity, 12.0, "PowerCapacitor conductivity must be 12.0");
        assert!(props.is_emissive, "PowerCapacitor must be emissive");
        assert_eq!(props.max_health, 65.0);
        assert_eq!(props.hardness, 2.8);
        assert_eq!(props.toughness, 2.2);
        assert_eq!(props.blast_threshold, 6.0);
        assert_eq!(props.density, 3.2);
        assert!(cap.is_solid(), "PowerCapacitor must be solid");
        assert!(!cap.is_transparent(), "PowerCapacitor must not be transparent");
        assert!(cap.is_opaque(), "PowerCapacitor must be opaque");
        assert!(cap.is_destructible(), "PowerCapacitor must be destructible");
        assert_eq!(cap.color(), Color::new(0.68, 0.28, 0.98, 1.0));
    }

    #[test]
    fn test_material_conductivity_registry_values() {
        assert_eq!(VoxelType::Air.properties().conductivity, 0.0);
        assert_eq!(VoxelType::Concrete.properties().conductivity, 0.0);
        assert_eq!(VoxelType::SteelRebar.properties().conductivity, 10.0);
        assert_eq!(VoxelType::HeavyArmor.properties().conductivity, 9.0);
        assert_eq!(VoxelType::CompositeRebar.properties().conductivity, 8.5);
        assert_eq!(VoxelType::Brick.properties().conductivity, 0.0);
        assert_eq!(VoxelType::Glass.properties().conductivity, 0.0);
        assert_eq!(VoxelType::Explosive.properties().conductivity, 0.0);
        assert_eq!(VoxelType::Bedrock.properties().conductivity, 0.0);
        assert_eq!(VoxelType::Basalt.properties().conductivity, 0.0);
        assert_eq!(VoxelType::CryoIce.properties().conductivity, 0.0);
        assert_eq!(VoxelType::GlowCore.properties().conductivity, 6.0);
        assert_eq!(VoxelType::PowerCapacitor.properties().conductivity, 12.0);
    }

    #[test]
    fn test_destructible_bedrock_and_basalt_properties() {
        let bedrock = VoxelType::Bedrock;
        let bedrock_props = bedrock.properties();
        assert_eq!(bedrock_props.name, "REINFORCED GRANITE BEDROCK");
        assert_eq!(bedrock_props.max_health, 800.0);
        assert_eq!(bedrock_props.hardness, 9.8);
        assert_eq!(bedrock_props.toughness, 9.5);
        assert_eq!(bedrock_props.blast_threshold, 45.0);
        assert_eq!(bedrock_props.density, 10.5);
        assert_eq!(bedrock_props.thermal_soften, 0.4);
        assert!(bedrock.is_destructible());
        assert!(bedrock.is_solid());

        let basalt = VoxelType::Basalt;
        let basalt_props = basalt.properties();
        assert_eq!(basalt_props.name, "SUBTERRANEAN BASALT SUBSTRATE");
        assert_eq!(basalt_props.max_health, 500.0);
        assert_eq!(basalt_props.hardness, 9.2);
        assert_eq!(basalt_props.toughness, 8.8);
        assert_eq!(basalt_props.blast_threshold, 32.0);
        assert_eq!(basalt_props.density, 8.8);
        assert_eq!(basalt_props.thermal_soften, 0.6);
        assert!(basalt.is_destructible());
        assert!(basalt.is_solid());
    }

    #[test]
    fn test_carve_sphere_captures_power_capacitor_triggers() {
        let mut grid = VoxelGrid::new();
        grid.set(25, 5, 25, VoxelType::PowerCapacitor);
        let world_pos = grid.voxel_to_world(IVec3::new(25, 5, 25));
        let res = grid.carve_sphere_advanced(world_pos, 1.5, 300.0, DamageType::Explosive);
        assert_eq!(res.destroyed, 1);
        assert_eq!(res.capacitor_triggers.len(), 1);
        assert_eq!(res.capacitor_triggers[0], IVec3::new(25, 5, 25));
    }

    #[test]
    fn test_18_direction_lighting_strikes_diagonal_wall() {
        let mut grid = VoxelGrid::new();
        // Construct 45-degree diagonal wall running along (x + z = 40)
        for i in 0..6 {
            grid.set(18 + i, 5, 22 - i, VoxelType::Concrete);
        }

        // Place light offset diagonally from wall
        let light_pos = grid.voxel_to_world(IVec3::new(17, 5, 23));
        let light = crate::physics::DynamicPointLight {
            position: light_pos,
            color: Color::new(1.0, 0.6, 0.1, 1.0),
            radius: 8.0,
            intensity: 2.0,
        };

        // Raycasting along diagonal (INV_SQRT2, 0.0, -INV_SQRT2) must strike the 45-degree wall
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let diag_dir = Vec3::new(INV_SQRT2, 0.0, -INV_SQRT2);
        let hit = grid.raycast(light.position, diag_dir, light.radius).expect("Diagonal ray must strike 45-degree wall");
        assert_eq!(hit.voxel_type, VoxelType::Concrete);
        assert!(hit.distance < light.radius);
    }

    #[test]
    fn test_dirty_chunks_incremental_tracking() {
        let mut grid = VoxelGrid::new();
        grid.update_meshes();
        assert!(grid.dirty_chunks.is_empty());

        // Set one voxel in chunk (2, 7, 2)
        grid.set(18, 9, 18, VoxelType::Concrete);
        assert!(!grid.dirty_chunks.is_empty());
        let expected_ci = VoxelGrid::chunk_idx(18 / CHUNK_SIZE, ((9 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE, 18 / CHUNK_SIZE);
        assert!(grid.dirty_chunks.contains(&expected_ci));

        // Update meshes: should clear dirty_chunks and mark that chunk not dirty
        grid.update_meshes();
        assert!(grid.dirty_chunks.is_empty());
        assert!(!grid.meshes[expected_ci].dirty);
    }

    #[test]
    fn test_beam_carve_empty_air_early_out() {
        let mut grid = VoxelGrid::new();
        grid.update_meshes();

        let start = Vec3::new(10.0, 30.0, 10.0);
        let end = Vec3::new(20.0, 40.0, 20.0);
        let mut res = CarveResult::with_capacity(8);
        grid.carve_cylinder_advanced_into(start, end, 1.0, 100.0, DamageType::Thermal, &mut res);
        assert_eq!(res.destroyed, 0);
        assert!(grid.dirty_chunks.is_empty());
    }

    #[test]
    fn test_sub_batch_partitioning_empty_mesh() {
        let batches = partition_mesh_sub_batches(&[], 0, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_sub_batch_partitioning_small_mesh_single_batch() {
        // 100 quads = 200 triangles = 600 indices, 400 vertices
        let mut indices = Vec::new();
        for q in 0..100u16 {
            let base = q * 4;
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let batches = partition_mesh_sub_batches(&indices, 400, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert_eq!(batches.len(), 1, "Small mesh must partition into exactly 1 sub-batch");
        assert_eq!(batches[0].1.len(), 600);
        assert_eq!(*batches[0].0.start(), 0);
        assert_eq!(*batches[0].0.end(), 399);
    }

    #[test]
    fn test_sub_batch_partitioning_index_limit_enforced() {
        // 1200 quads = 2400 triangles = 7200 indices (exceeds 4800 index cap)
        // Vertices are clustered between 0 and 200
        let mut indices = Vec::new();
        for t in 0..2400u16 {
            let i0 = t % 150;
            let i1 = (t + 1) % 150;
            let i2 = (t + 2) % 150;
            indices.extend_from_slice(&[i0, i1, i2]);
        }
        let batches = partition_mesh_sub_batches(&indices, 200, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert!(batches.len() >= 2, "7200 indices must be partitioned into at least 2 batches");
        let mut total_indices = 0;
        for (vert_range, remapped) in &batches {
            assert!(remapped.len() <= MAX_CHUNK_BATCH_INDICES, "Sub-batch index count {} must be <= 4800", remapped.len());
            let span = (vert_range.end() - vert_range.start()) + 1;
            assert!(span <= MAX_CHUNK_BATCH_VERTICES);
            total_indices += remapped.len();
        }
        assert_eq!(total_indices, 7200, "Zero indices dropped during index-limited partitioning");
    }

    #[test]
    fn test_sub_batch_partitioning_vertex_span_limit_enforced() {
        // 800 triangles = 2400 indices (well below 4800 limit)
        // But vertices span from 0 to 12000!
        let mut indices = Vec::new();
        for t in 0..800u16 {
            let base = t * 15; // 0, 15, 30, ... up to 11985
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        let batches = partition_mesh_sub_batches(&indices, 12000, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert!(batches.len() >= 3, "12000-span mesh must partition into at least 3 sub-batches to satisfy 4000 vertex span");
        let mut total_indices = 0;
        for (vert_range, remapped) in &batches {
            let span = vert_range.end() - vert_range.start() + 1;
            assert!(span <= MAX_CHUNK_BATCH_VERTICES, "Vertex span {} must be <= 4000", span);
            assert!(remapped.len() <= MAX_CHUNK_BATCH_INDICES);
            for &idx in remapped {
                assert!((idx as usize) < span, "Remapped index {} must be strictly within vertex span {}", idx, span);
            }
            total_indices += remapped.len();
        }
        assert_eq!(total_indices, 2400, "Zero geometry dropped during vertex-span-limited partitioning");
    }

    #[test]
    fn test_sub_batch_partitioning_arbitrary_huge_mesh_zero_clamping_and_zero_drop() {
        // Massive mesh: 30,000 indices across 25,000 vertices
        let mut indices = Vec::with_capacity(30000);
        for t in 0..10000u16 {
            let base = (t * 2) % 24000;
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
        }
        let batches = partition_mesh_sub_batches(&indices, 25000, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert!(batches.len() >= 7, "30000 indices must be partitioned into multiple batches");
        let mut reconstructed_indices = Vec::with_capacity(30000);
        for (vert_range, remapped) in &batches {
            let span = vert_range.end() - vert_range.start() + 1;
            assert!(span <= MAX_CHUNK_BATCH_VERTICES, "Every sub-batch must satisfy vertex limit < 4000");
            assert!(remapped.len() <= MAX_CHUNK_BATCH_INDICES, "Every sub-batch must satisfy index limit < 4800");
            for &remapped_idx in remapped {
                assert!((remapped_idx as usize) < span);
                reconstructed_indices.push(remapped_idx + *vert_range.start() as u16);
            }
        }
        assert_eq!(reconstructed_indices.len(), indices.len(), "Total drawn indices must equal original indices");
        assert_eq!(reconstructed_indices, indices, "Reconstructed indices must match input index stream exactly with zero drop");
    }

    #[test]
    fn test_sub_batch_partitioning_anomalous_odd_and_wide_triangles() {
        // Single triangle spanning 5000 vertices, followed by odd 1 trailing index
        let indices = vec![0u16, 1, 5000, 5001];
        let batches = partition_mesh_sub_batches(&indices, 6000, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
        assert_eq!(batches.len(), 2, "Anomalous triangle + odd index must partition into 2 batches");
        let mut total_indices = 0;
        for (_, remapped) in &batches {
            total_indices += remapped.len();
        }
        assert_eq!(total_indices, 4, "All 4 indices must be processed without infinite loop");
    }

    #[test]
    fn test_g6_m2_true_bedrock_grounding_negative_24m() {
        let mut grid = VoxelGrid::new();

        // Excavate a subterranean shaft around (40, 40) down to y=-20
        for dx in -1..=1 {
            for dz in -1..=1 {
                for y in -20..=5 {
                    grid.set(40 + dx, y, 40 + dz, VoxelType::Air);
                }
            }
        }

        // Build a subterranean column rooted into bedrock substrate at y = -20 up to y=5
        for y in -20..=5 {
            grid.set(40, y, 40, VoxelType::Concrete);
        }

        // Initially fully connected down to bedrock -> 0 unsupported islands
        let islands = grid.detect_unsupported_islands();
        assert!(islands.is_empty(), "Column rooted into bedrock substrate must be grounded");

        // Sever the column underground at y = -10
        grid.set(40, -10, 40, VoxelType::Air);

        // Island detection must detect the upper portion as unsupported
        let islands_severed = grid.detect_unsupported_islands();
        assert!(!islands_severed.is_empty(), "Severing column above bedrock substrate must produce an island chunk");
    }

    #[test]
    fn test_g6_m2_localized_bfs_ceiling_expansion_across_air_gap() {
        let mut grid = VoxelGrid::new();

        // Build a tower from y=1 to y=60
        for y in 1..=60 {
            grid.set(25, y, 25, VoxelType::Concrete);
        }

        // Sever an air gap at y=5..=7 (leaving severed stories at y=8..=60)
        for y in 5..=7 {
            grid.set(25, y, 25, VoxelType::Air);
        }

        // Localized BFS dirty bounds only cover y=5..=7 + pad (which would clamp around y=15)
        // With adaptive ceiling expansion across the air gap, it must capture the severed tower up to y=60!
        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Disconnected upper tower must be detected as an island");
        let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
        assert!(
            total_island_voxels >= 50,
            "Expanded ceiling BFS must capture full height of upper tower (got {} voxels)",
            total_island_voxels
        );
    }

    #[test]
    fn test_g6_m2_subterranean_integrity_active_when_surface_cleared() {
        let mut grid = VoxelGrid::new();

        // Surface solid count is 0
        assert_eq!(grid.solid_count(), 0);

        // Excavate a subterranean void and build a suspended slab inside it at y=-15
        for dx in -2..=2 {
            for dz in -2..=2 {
                for y in -20..=-10 {
                    grid.set(30 + dx, y, 30 + dz, VoxelType::Air);
                }
            }
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                grid.set(30 + dx, -15, 30 + dz, VoxelType::Concrete);
            }
        }

        // Even though surface solid_count == 0, island detection must NOT early-out
        // because subterranean structures are modified and floating!
        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Subterranean severed slab must be detected even with surface solid_count == 0");
    }

    #[test]
    fn test_voxel_grid_slice_planar_kerf() {
        let mut grid = VoxelGrid::new();
        let center = IVec3::new(20, 10, 20);
        // Build a 3x3x3 concrete cube
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    grid.set(center.x + dx, center.y + dy, center.z + dz, VoxelType::Concrete);
                }
            }
        }
        let world_origin = grid.voxel_to_world(center);
        let player_cam = world_origin - Vec3::Z * 5.0;
        let look_dir = Vec3::Z;
        let blade_norm = Vec3::Y; // Horizontal slice

        let cuts = grid.slice_planar_kerf(player_cam, look_dir, blade_norm, 10.0, 0.45);
        assert!(cuts > 0, "Planar kerf must cut voxels along slicing plane");
        assert!(grid.structural_dirty, "Structural dirty flag must be raised");
    }
}


