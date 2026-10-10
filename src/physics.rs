use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, VoxelType, IslandChunk, VOXEL_SIZE, VOID_BOUNDARY_Y, MIN_VOXEL_Y, MAX_VOXEL_Y, FrustumPlanes};
use crate::audio::SoundSystem;
use std::cell::RefCell;

/// Visual arc flicker must not consume the random stream used by destruction physics.
pub(crate) fn cosmetic_arc_jitter(origin: Vec3, time: f32, segment: i32) -> Vec3 {
    let phase = time * 97.0 + segment as f32 * 17.0 + origin.dot(Vec3::new(0.31, 0.47, 0.73));
    Vec3::new(phase.sin(), (phase * 1.37).sin(), (phase * 1.79).sin()) * 0.15
}

type SubBlockBucket = ((Vec3, Vec3), Vec<usize>);

thread_local! {
    static SPARE_SUB_BLOCKS: RefCell<Vec<Option<SubBlockBucket>>> = const { RefCell::new(Vec::new()) };
}

#[derive(Debug, Clone, Copy)]
pub struct Mat3 {
    pub m: [[f32; 3]; 3],
}

impl Mat3 {
    #[allow(dead_code)]
    pub const IDENTITY: Self = Self {
        m: [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
    };

    #[inline]
    #[allow(dead_code)]
    pub fn identity() -> Self {
        Self::IDENTITY
    }

    #[inline]
    pub fn from_euler(rot: Vec3) -> Self {
        let (sx, cx) = rot.x.sin_cos();
        let (sy, cy) = rot.y.sin_cos();
        let (sz, cz) = rot.z.sin_cos();

        Self {
            m: [
                [cz * cy, cz * sx * sy - sz * cx, cz * cx * sy + sz * sx],
                [sz * cy, sz * sx * sy + cz * cx, sz * cx * sy - cz * sx],
                [-sy, sx * cy, cx * cy],
            ],
        }
    }

    #[inline(always)]
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }

    #[inline(always)]
    pub fn transform_y(&self, v: Vec3) -> f32 {
        self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z
    }

    /// Applies the transpose (= inverse for a rotation): world vector -> body frame.
    #[inline(always)]
    pub fn transform_vector_transposed(&self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[1][0] * v.y + self.m[2][0] * v.z,
            self.m[0][1] * v.x + self.m[1][1] * v.y + self.m[2][1] * v.z,
            self.m[0][2] * v.x + self.m[1][2] * v.y + self.m[2][2] * v.z,
        )
    }

    #[inline]
    pub fn to_glam(&self) -> macroquad::math::Mat3 {
        macroquad::math::Mat3::from_cols(
            Vec3::new(self.m[0][0], self.m[1][0], self.m[2][0]),
            Vec3::new(self.m[0][1], self.m[1][1], self.m[2][1]),
            Vec3::new(self.m[0][2], self.m[1][2], self.m[2][2]),
        )
    }

    #[inline]
    pub fn from_glam(g: &macroquad::math::Mat3) -> Self {
        Self {
            m: [
                [g.x_axis.x, g.y_axis.x, g.z_axis.x],
                [g.x_axis.y, g.y_axis.y, g.z_axis.y],
                [g.x_axis.z, g.y_axis.z, g.z_axis.z],
            ],
        }
    }

    /// Rotates this orientation by a world-frame angular velocity over `dt` using an exact
    /// axis-angle step on a renormalized quaternion (no Euler-angle drift or gimbal lock).
    #[inline]
    pub fn integrate_angular(&self, omega: Vec3, dt: f32) -> Self {
        let w_len = omega.length();
        let angle = w_len * dt;
        if angle < 1e-7 {
            return *self;
        }
        let step = Quat::from_axis_angle(omega / w_len, angle);
        let q = (step * Quat::from_mat3(&self.to_glam())).normalize();
        Self::from_glam(&macroquad::math::Mat3::from_quat(q))
    }

    /// Extracts (x, y, z) Euler angles in the same convention as [`Mat3::from_euler`]
    /// (R = Rz · Ry · Rx), so `from_euler(m.to_euler())` reproduces `m`.
    #[inline]
    pub fn to_euler(&self) -> Vec3 {
        let sy = (-self.m[2][0]).clamp(-1.0, 1.0);
        let y = sy.asin();
        if sy.abs() < 0.99999 {
            Vec3::new(self.m[2][1].atan2(self.m[2][2]), y, self.m[1][0].atan2(self.m[0][0]))
        } else {
            // Gimbal lock: fold all roll into x.
            Vec3::new((-self.m[1][2]).atan2(self.m[1][1]), y, 0.0)
        }
    }
}

#[inline]
#[allow(dead_code)]
pub fn rotate_vec3(v: Vec3, rot: Vec3) -> Vec3 {
    Mat3::from_euler(rot).transform_vector(v)
}

/// Kinetic energy a dynamic chunk must spend to pulverize one static voxel, per unit of the
/// material's crush threshold. Concrete (threshold 15) costs 300; steel rebar (25) costs 500.
/// Calibration: a 12-voxel concrete chunk at 6 m/s (KE ≈ 630) breaks ~2 floor voxels, a floor
/// slab falling one story pancakes a comparable slab below, while a 3-voxel chip at 5 m/s
/// (KE ≈ 90) can no longer break concrete. This is what stops self-sustaining crush cascades.
pub const CRUSH_ENERGY_PER_THRESHOLD: f32 = 20.0;

/// Below this contact speed a chunk is resting against a barrier, not impacting it, so it
/// deals no kinetic damage (prevents frame-rate-dependent grinding of armor/rebar).
pub const RESISTED_IMPACT_MIN_SPEED: f32 = 2.0;

/// Momentum threshold (effective point mass × contact speed) above which a falling chunk can
/// crush a static voxel. Penetrators (compact, dense chunks) punch through at lower thresholds.
#[inline]
pub fn crush_threshold(vt: VoxelType, is_penetrator: bool) -> f32 {
    if is_penetrator {
        match vt {
            VoxelType::Glass | VoxelType::CryoIce => 1.0,
            VoxelType::Explosive => 2.0,
            VoxelType::Brick => 4.0,
            VoxelType::PowerCapacitor => 5.0,
            VoxelType::Concrete => 6.0,
            VoxelType::GlowCore => 8.0,
            VoxelType::SteelRebar | VoxelType::CompositeRebar => 10.0,
            VoxelType::HeavyArmor => 35.0,
            _ => f32::INFINITY,
        }
    } else {
        match vt {
            VoxelType::Glass | VoxelType::CryoIce => 2.0,
            VoxelType::Explosive => 4.0,
            VoxelType::Brick => 8.0,
            VoxelType::PowerCapacitor => 10.0,
            VoxelType::Concrete => 15.0,
            VoxelType::GlowCore => 20.0,
            VoxelType::SteelRebar | VoxelType::CompositeRebar => 25.0,
            VoxelType::HeavyArmor => 250.0,
            _ => f32::INFINITY,
        }
    }
}

/// Coulomb friction coefficient for chunk-vs-world contacts (rough concrete on concrete ≈ 0.6).
pub const CONTACT_FRICTION: f32 = 0.6;
/// Coulomb friction coefficient for chunk-vs-chunk contacts.
pub const CHUNK_CHUNK_FRICTION: f32 = 0.5;
/// Restitution for chunk-vs-chunk contacts (masonry is nearly perfectly inelastic).
pub const CHUNK_CHUNK_RESTITUTION: f32 = 0.1;
/// Contacts within this gap (m) count toward the support polygon even if not penetrating.
pub const CONTACT_SLOP: f32 = 0.06;
/// Sequential-impulse iterations per contact manifold.
pub const CONTACT_ITERATIONS: usize = 4;
/// Rolling resistance: decay rate (1/s) of angular velocity perpendicular to the contact normal.
pub const ROLLING_RESISTANCE: f32 = 1.5;
/// Spin friction: decay rate (1/s) of angular velocity about the contact normal.
pub const SPIN_FRICTION: f32 = 5.0;
/// Upper bound on surface speed due to spin (m/s); |ω| is clamped to this over the chunk radius.
pub const MAX_SPIN_SURFACE_SPEED: f32 = 30.0;
/// Maximum number of chunk-vs-chunk pairs resolved per frame (bounds worst-case cost).
pub const MAX_CHUNK_PAIRS_PER_FRAME: usize = 96;

/// One rigid contact between a dynamic chunk and the world.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContactPoint {
    /// World-space contact point (on the chunk's surface).
    pub point: Vec3,
    /// Unit normal pointing from the world into the chunk.
    pub normal: Vec3,
    /// Penetration depth (m). Negative values are near-contacts within [`CONTACT_SLOP`].
    pub depth: f32,
}

/// Maximum number of contacts kept per manifold.
pub const MAX_CONTACTS: usize = 8;

/// Fixed-capacity contact manifold (no heap allocation). Keeps the deepest [`MAX_CONTACTS`]
/// points for the impulse solve, and the xz bounds of *every* upward-facing contact for the
/// support-polygon test that decides whether a resting chunk is balanced or must tip over.
#[derive(Clone, Copy, Debug)]
pub struct ContactManifold {
    pub points: [ContactPoint; MAX_CONTACTS],
    pub len: usize,
    pub support_min: Vec2,
    pub support_max: Vec2,
    pub support_count: usize,
}

impl Default for ContactManifold {
    fn default() -> Self {
        Self {
            points: [ContactPoint::default(); MAX_CONTACTS],
            len: 0,
            support_min: Vec2::splat(f32::INFINITY),
            support_max: Vec2::splat(f32::NEG_INFINITY),
            support_count: 0,
        }
    }
}

impl ContactManifold {
    pub fn push(&mut self, c: ContactPoint) {
        if c.normal.y > 0.6 {
            let p = Vec2::new(c.point.x, c.point.z);
            self.support_min = self.support_min.min(p);
            self.support_max = self.support_max.max(p);
            self.support_count += 1;
        }
        if self.len < MAX_CONTACTS {
            self.points[self.len] = c;
            self.len += 1;
            return;
        }
        let mut shallowest = 0;
        for k in 1..MAX_CONTACTS {
            if self.points[k].depth < self.points[shallowest].depth {
                shallowest = k;
            }
        }
        if c.depth > self.points[shallowest].depth {
            self.points[shallowest] = c;
        } else if (c.depth - self.points[shallowest].depth).abs() <= 1e-5 {
            // Equal-depth points are interchangeable as penetration constraints, but their
            // spread matters for torque. Keeping only the first face of a wide slab invents
            // an off-center impact; replace its most central redundant point with a wider one.
            let center = self.contacts().iter().fold(Vec3::ZERO, |sum, p| sum + p.point) / self.len as f32;
            let mut nearest = None;
            let mut nearest_dist = f32::INFINITY;
            for k in 0..self.len {
                if (self.points[k].depth - c.depth).abs() <= 1e-5 && self.points[k].normal.dot(c.normal) > 0.99 {
                    let dist = (self.points[k].point - center).length_squared();
                    if dist < nearest_dist {
                        nearest = Some(k);
                        nearest_dist = dist;
                    }
                }
            }
            if (c.point - center).length_squared() > nearest_dist + 1e-6 {
                if let Some(nearest) = nearest {
                    self.points[nearest] = c;
                }
            }
        }
    }

    #[inline]
    pub fn contacts(&self) -> &[ContactPoint] {
        &self.points[..self.len]
    }

    /// Deepest contact (largest penetration), if any.
    pub fn deepest(&self) -> Option<ContactPoint> {
        self.contacts().iter().copied().fold(None, |best: Option<ContactPoint>, c| match best {
            Some(b) if b.depth >= c.depth => Some(b),
            _ => Some(c),
        })
    }

    /// True if any contact actually penetrates.
    #[inline]
    pub fn has_penetration(&self) -> bool {
        self.contacts().iter().any(|c| c.depth > 0.0)
    }

    /// Average normal of the touching contacts (falls back to +Y).
    pub fn mean_normal(&self) -> Vec3 {
        let sum = self.contacts().iter().filter(|c| c.depth > -CONTACT_SLOP).fold(Vec3::ZERO, |a, c| a + c.normal);
        let n = sum.normalize_or_zero();
        if n == Vec3::ZERO { Vec3::Y } else { n }
    }

    /// Whether the center of mass projects inside the xz bounds of the upward-facing contacts
    /// (expanded by `margin`). A chunk whose COM overhangs its support must tip, not settle.
    pub fn supports_com(&self, com: Vec3, margin: f32) -> bool {
        if self.support_count == 0 {
            return false;
        }
        let p = Vec2::new(com.x, com.z);
        p.x >= self.support_min.x - margin
            && p.x <= self.support_max.x + margin
            && p.y >= self.support_min.y - margin
            && p.y <= self.support_max.y + margin
    }
}

/// Contact between a chunk voxel centered at `wpos` and a static voxel centered at `svc`. Static
/// voxel faces are axis-aligned, so the normal is the axis of greatest center separation
/// (vertical wins ties, so resting on a floor always yields an upward normal).
#[inline]
pub fn static_voxel_contact(wpos: Vec3, svc: Vec3) -> ContactPoint {
    let d = wpos - svc;
    let a = d.abs();
    let (normal, sep) = if a.y >= a.x && a.y >= a.z {
        (Vec3::new(0.0, d.y.signum(), 0.0), a.y)
    } else if a.x >= a.z {
        (Vec3::new(d.x.signum(), 0.0, 0.0), a.x)
    } else {
        (Vec3::new(0.0, 0.0, d.z.signum()), a.z)
    };
    ContactPoint { point: wpos - normal * (VOXEL_SIZE * 0.5), normal, depth: VOXEL_SIZE - sep }
}

/// Impact speed (m/s) above which a chunk shatters on a rigid contact: mostly-glass/ice chunks
/// are brittle, mostly-armor chunks are tough, everything else sits in between.
pub fn fracture_speed_threshold(voxels: &[(Vec3, VoxelType)]) -> f32 {
    let fragile = voxels.iter().filter(|(_, vt)| matches!(vt, VoxelType::Glass | VoxelType::CryoIce)).count();
    let armor = voxels.iter().filter(|(_, vt)| *vt == VoxelType::HeavyArmor).count();
    if fragile * 2 >= voxels.len() {
        4.5
    } else if armor * 2 >= voxels.len() {
        24.0
    } else {
        8.5
    }
}


pub struct ParticleBatcher {
    pub debris_mesh: Mesh,
    pub sparks_mesh: Mesh,
    pub smoke_mesh: Mesh,
    pub solids_mesh: Mesh,
}

pub const MAX_BATCH_VERTICES: usize = 4000;
pub const MAX_BATCH_INDICES: usize = 4800;

impl ParticleBatcher {
    pub fn new() -> Self {
        Self {
            debris_mesh: Mesh {
                vertices: Vec::with_capacity(4096),
                indices: Vec::with_capacity(5120),
                texture: None,
            },
            sparks_mesh: Mesh {
                vertices: Vec::with_capacity(4096),
                indices: Vec::with_capacity(5120),
                texture: None,
            },
            smoke_mesh: Mesh {
                vertices: Vec::with_capacity(4096),
                indices: Vec::with_capacity(5120),
                texture: None,
            },
            solids_mesh: Mesh {
                vertices: Vec::with_capacity(4096),
                indices: Vec::with_capacity(5120),
                texture: None,
            },
        }
    }
}

#[inline(always)]
fn safe_draw_mesh(mesh: &Mesh) {
    #[cfg(not(test))]
    draw_mesh(mesh);
    #[cfg(test)]
    let _ = mesh;
}

#[inline(always)]
fn safe_draw_cylinder(pos: Vec3, r_top: f32, r_bot: f32, height: f32, texture: Option<&Texture2D>, color: Color) {
    #[cfg(not(test))]
    draw_cylinder(pos, r_top, r_bot, height, texture, color);
    #[cfg(test)]
    let _ = (pos, r_top, r_bot, height, texture, color);
}

#[inline(always)]
fn safe_draw_line_3d(start: Vec3, end: Vec3, color: Color) {
    #[cfg(not(test))]
    draw_line_3d(start, end, color);
    #[cfg(test)]
    let _ = (start, end, color);
}

#[inline(always)]
fn safe_draw_sphere(center: Vec3, radius: f32, texture: Option<&Texture2D>, color: Color) {
    #[cfg(not(test))]
    draw_sphere(center, radius, texture, color);
    #[cfg(test)]
    let _ = (center, radius, texture, color);
}

#[inline]
pub fn push_oriented_cube_mesh(
    mesh: &mut Mesh,
    center: Vec3,
    hx: Vec3,
    hy: Vec3,
    hz: Vec3,
    color: Color,
) {
    if mesh.vertices.len() + 24 >= MAX_BATCH_VERTICES || mesh.indices.len() + 36 >= MAX_BATCH_INDICES {
        safe_draw_mesh(mesh);
        mesh.vertices.clear();
        mesh.indices.clear();
    }

    let faces: [(Vec3, Vec3, Vec3, Vec3, f32); 6] = [
        // Top (+Y)
        (center + hy - hx + hz, center + hy + hx + hz, center + hy + hx - hz, center + hy - hx - hz, 1.15),
        // Bottom (-Y)
        (center - hy - hx - hz, center - hy + hx - hz, center - hy + hx + hz, center - hy - hx + hz, 0.48),
        // Right (+X)
        (center + hx + hy + hz, center + hx - hy + hz, center + hx - hy - hz, center + hx + hy - hz, 0.88),
        // Left (-X)
        (center - hx + hy - hz, center - hx - hy - hz, center - hx - hy + hz, center - hx + hy + hz, 0.72),
        // Front (+Z)
        (center + hz - hx + hy, center + hz + hx + hy, center + hz + hx - hy, center + hz - hx - hy, 0.98),
        // Back (-Z)
        (center - hz + hx + hy, center - hz - hx + hy, center - hz - hx - hy, center - hz + hx - hy, 0.60),
    ];

    for &(p0, p1, p2, p3, shade) in &faces {
        let v_base = mesh.vertices.len() as u16;
        let face_col = Color::new(
            (color.r * shade).min(1.0),
            (color.g * shade).min(1.0),
            (color.b * shade).min(1.0),
            color.a,
        );
        let c_raw: [u8; 4] = face_col.into();

        mesh.vertices.push(Vertex { position: p0, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        mesh.vertices.push(Vertex { position: p1, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        mesh.vertices.push(Vertex { position: p2, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        mesh.vertices.push(Vertex { position: p3, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });

        mesh.indices.extend_from_slice(&[
            v_base, v_base + 1, v_base + 2,
            v_base, v_base + 2, v_base + 3,
        ]);
    }
}

#[inline]
pub fn push_cube_mesh(mesh: &mut Mesh, center: Vec3, size: Vec3, color: Color) {
    let hx = Vec3::new(size.x * 0.5, 0.0, 0.0);
    let hy = Vec3::new(0.0, size.y * 0.5, 0.0);
    let hz = Vec3::new(0.0, 0.0, size.z * 0.5);
    push_oriented_cube_mesh(mesh, center, hx, hy, hz, color);
}

#[inline]
pub fn push_quad_mesh(
    mesh: &mut Mesh,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    color: Color,
) {
    if mesh.vertices.len() + 4 >= MAX_BATCH_VERTICES || mesh.indices.len() + 6 >= MAX_BATCH_INDICES {
        safe_draw_mesh(mesh);
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

#[inline]
pub fn push_quad_mesh_dual_tone(
    mesh: &mut Mesh,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    col_bottom: Color,
    col_top: Color,
) {
    if mesh.vertices.len() + 4 >= MAX_BATCH_VERTICES || mesh.indices.len() + 6 >= MAX_BATCH_INDICES {
        safe_draw_mesh(mesh);
        mesh.vertices.clear();
        mesh.indices.clear();
    }
    let v_base = mesh.vertices.len() as u16;
    let cb_raw: [u8; 4] = col_bottom.into();
    let ct_raw: [u8; 4] = col_top.into();
    mesh.vertices.push(Vertex { position: p0, uv: Vec2::ZERO, color: cb_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p1, uv: Vec2::ZERO, color: cb_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p2, uv: Vec2::ZERO, color: ct_raw, normal: Vec4::ZERO });
    mesh.vertices.push(Vertex { position: p3, uv: Vec2::ZERO, color: ct_raw, normal: Vec4::ZERO });

    mesh.indices.extend_from_slice(&[
        v_base, v_base + 1, v_base + 2,
        v_base, v_base + 2, v_base + 3,
    ]);
}

pub struct DebrisParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Vec3,
    pub angular_velocity: Vec3,
    pub size: f32,
    pub color: Color,
    pub life: f32,
    pub max_life: f32,
    pub bounces: u8,
}

pub struct SmokeParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    pub start_size: f32,
    pub max_size: f32,
    pub color: Color,
    pub life: f32,
    pub max_life: f32,
}

pub struct SparkParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: Color,
    pub life: f32,
    pub max_life: f32,
}

pub struct ShockwaveRing {
    pub position: Vec3,
    pub normal: Vec3,
    pub radius: f32,
    pub max_radius: f32,
    pub speed: f32,
    pub color: Color,
    pub thickness: f32,
}

#[derive(Clone, Debug)]
pub struct ElectricalArc {
    pub start: Vec3,
    pub end: Vec3,
    pub timer: f32,
    pub max_timer: f32,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct PressurizedFireJet {
    pub position: Vec3,
    pub direction: Vec3,
    pub timer: f32,
    pub max_timer: f32,
}

pub struct DynamicChunk {
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Vec3, // Pitch, Yaw, Roll
    pub rot_mat: Mat3,
    pub angular_velocity: Vec3,
    pub voxels: Vec<(Vec3, VoxelType)>, // Local relative position, VoxelType
    pub exposed_voxels: Vec<(Vec3, VoxelType)>,
    /// Remaining health, aligned with voxels; empty until a body first takes damage.
    pub voxel_health: Vec<f32>,
    pub mass: f32,
    pub life: f32,
    pub settled: bool,
    pub local_aabb: (Vec3, Vec3),
}

impl DynamicChunk {
    pub fn compute_exposed_voxels(voxels: &[(Vec3, VoxelType)]) -> Vec<(Vec3, VoxelType)> {
        if voxels.len() <= 6 {
            return voxels.to_vec();
        }
        let inv_size = 1.0 / VOXEL_SIZE;
        // Centers share a voxel lattice but its origin can be anywhere after recentering
        // a body. Round relative to one center to avoid a gap at +/- half-grid offsets.
        let grid_origin = voxels[0].0;
        let mut occupied = std::collections::HashSet::with_capacity(voxels.len());
        for (p, _) in voxels {
            let cell = (*p - grid_origin) * inv_size;
            let ix = cell.x.round() as i32;
            let iy = cell.y.round() as i32;
            let iz = cell.z.round() as i32;
            occupied.insert((ix, iy, iz));
        }
        let mut exposed = Vec::with_capacity(voxels.len());
        let deltas = [
            (1, 0, 0), (-1, 0, 0),
            (0, 1, 0), (0, -1, 0),
            (0, 0, 1), (0, 0, -1),
        ];
        for (p, vt) in voxels {
            let cell = (*p - grid_origin) * inv_size;
            let ix = cell.x.round() as i32;
            let iy = cell.y.round() as i32;
            let iz = cell.z.round() as i32;
            let mut is_exposed = false;
            for (dx, dy, dz) in deltas {
                if !occupied.contains(&(ix + dx, iy + dy, iz + dz)) {
                    is_exposed = true;
                    break;
                }
            }
            if is_exposed {
                exposed.push((*p, *vt));
            }
        }
        exposed
    }

    #[allow(dead_code)]
    pub fn rebuild_exposed_voxels(&mut self) {
        self.exposed_voxels = Self::compute_exposed_voxels(&self.voxels);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position: Vec3,
        velocity: Vec3,
        rotation: Vec3,
        angular_velocity: Vec3,
        voxels: Vec<(Vec3, VoxelType)>,
        mass: f32,
        life: f32,
        settled: bool,
        local_aabb: (Vec3, Vec3),
    ) -> Self {
        let exposed_voxels = Self::compute_exposed_voxels(&voxels);
        let rot_mat = Mat3::from_euler(rotation);
        Self {
            position,
            velocity,
            rotation,
            rot_mat,
            angular_velocity,
            voxels,
            exposed_voxels,
            voxel_health: Vec::new(),
            mass,
            life,
            settled,
            local_aabb,
        }
    }

    pub fn compute_aabb(voxels: &[(Vec3, VoxelType)]) -> (Vec3, Vec3) {
        if voxels.is_empty() {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let mut min_pos = Vec3::splat(f32::INFINITY);
        let mut max_pos = Vec3::splat(f32::NEG_INFINITY);
        for (p, _) in voxels {
            min_pos = min_pos.min(*p);
            max_pos = max_pos.max(*p);
        }
        (min_pos, max_pos)
    }

    pub fn local_to_world(&self, local_v: Vec3) -> Vec3 {
        self.position + self.rot_mat.transform_vector(local_v)
    }

    /// Principal moments of inertia in the body frame (kg·m²), modeling the chunk as a uniform
    /// box filling its local bounds, plus the parallel-axis offset of that box from the chunk's
    /// origin (its voxel centroid). Derived from current mass, so accretion is accounted for.
    #[inline]
    pub fn body_inertia(&self) -> Vec3 {
        let (lo, hi) = self.local_aabb;
        let d = (hi - lo) + Vec3::splat(VOXEL_SIZE);
        let c = (hi + lo) * 0.5;
        let m = self.mass.max(0.1);
        let box_i = Vec3::new(d.y * d.y + d.z * d.z, d.x * d.x + d.z * d.z, d.x * d.x + d.y * d.y) * (m / 12.0);
        let offset_i = Vec3::new(c.y * c.y + c.z * c.z, c.x * c.x + c.z * c.z, c.x * c.x + c.y * c.y) * m;
        (box_i + offset_i).max(Vec3::splat(1e-3))
    }

    /// Applies the world-frame inverse inertia tensor: I_w⁻¹ · v = R · I_b⁻¹ · Rᵀ · v.
    #[inline]
    pub fn apply_inv_inertia(&self, inertia: Vec3, v: Vec3) -> Vec3 {
        let b = self.rot_mat.transform_vector_transposed(v);
        self.rot_mat.transform_vector(b / inertia)
    }

    /// Velocity of the material point at world offset `r` from the chunk origin.
    #[inline]
    pub fn point_velocity(&self, r: Vec3) -> Vec3 {
        self.velocity + self.angular_velocity.cross(r)
    }

    /// Effective inverse mass of the chunk along direction `n` at contact offset `r`.
    #[inline]
    pub fn inv_effective_mass(&self, inertia: Vec3, r: Vec3, n: Vec3) -> f32 {
        let rn = r.cross(n);
        1.0 / self.mass.max(0.1) + n.dot(self.apply_inv_inertia(inertia, rn).cross(r))
    }

    /// Applies an impulse `j` at world offset `r` from the chunk origin.
    #[inline]
    pub fn apply_impulse(&mut self, inertia: Vec3, r: Vec3, j: Vec3) {
        self.velocity += j / self.mass.max(0.1);
        self.angular_velocity += self.apply_inv_inertia(inertia, r.cross(j));
    }

    /// Advances orientation by the current angular velocity, including the torque-free
    /// gyroscopic precession of an asymmetric body (angular momentum, not ω, is conserved).
    pub fn integrate_rotation(&mut self, dt: f32) {
        if self.angular_velocity == Vec3::ZERO {
            return;
        }
        let inertia = self.body_inertia();
        let w_b = self.rot_mat.transform_vector_transposed(self.angular_velocity);
        let l_b = w_b * inertia;
        let energy = w_b.dot(l_b);
        // Euler's equations: I·dω/dt = -ω × (I·ω); re-scale to conserve rotational energy.
        let mut w_new = w_b - (w_b.cross(l_b) / inertia) * dt;
        let e_new = w_new.dot(w_new * inertia);
        if e_new > 1e-9 {
            w_new *= (energy / e_new).sqrt();
        }
        self.angular_velocity = self.rot_mat.transform_vector(w_new);
        self.rot_mat = self.rot_mat.integrate_angular(self.angular_velocity, dt);
        self.rotation = self.rot_mat.to_euler();
    }

    /// Bounding radius of the chunk about its origin (m), at least one voxel.
    #[inline]
    pub fn bounding_radius(&self) -> f32 {
        let (lo, hi) = self.local_aabb;
        (lo.abs().max(hi.abs()) + Vec3::splat(VOXEL_SIZE * 0.5)).length().max(VOXEL_SIZE)
    }

    /// Peak approach speed (m/s, ≥ 0) over contacts that are touching or will be reached
    /// within `dt` at the current velocity.
    pub fn contact_approach_speed(&self, manifold: &ContactManifold, dt: f32) -> f32 {
        manifold
            .contacts()
            .iter()
            .map(|c| (c, -self.point_velocity(c.point - self.position).dot(c.normal)))
            .filter(|(c, approach)| c.depth > 0.0 || *approach * dt > -c.depth)
            .map(|(_, approach)| approach)
            .fold(0.0f32, f32::max)
    }

    /// Sequential-impulse solve of the chunk against static contacts (the world is immovable).
    /// Normal impulses are accumulated and clamped to be non-negative (contacts push, never
    /// pull); friction impulses are clamped to the Coulomb cone `|jt| ≤ μ·jn`. Off-center contact
    /// impulses generate torque, so a chunk resting on an edge pivots and tips over naturally.
    /// Separated contacts are speculative: they only act if the gap would close within `dt`.
    pub fn solve_static_contacts(&mut self, manifold: &ContactManifold, restitution: f32, friction: f32, dt: f32) {
        let inertia = self.body_inertia();
        let mut acc_n = [0.0f32; MAX_CONTACTS];
        let mut acc_t = [Vec3::ZERO; MAX_CONTACTS];
        let mut target_vn = [0.0f32; MAX_CONTACTS];
        let contacts = manifold.contacts();
        let inv_dt = 1.0 / dt.max(1e-4);
        for (k, c) in contacts.iter().enumerate() {
            let vn = self.point_velocity(c.point - self.position).dot(c.normal);
            target_vn[k] = if c.depth <= 0.0 {
                // Allowed approach speed: exactly enough to close the gap this step.
                c.depth * inv_dt
            } else if vn < -1.0 {
                // Bounce only genuine impacts; resting contacts get zero restitution (no jitter).
                -restitution * vn
            } else {
                0.0
            };
        }
        for _ in 0..CONTACT_ITERATIONS {
            for (k, c) in contacts.iter().enumerate() {
                let r = c.point - self.position;
                let n = c.normal;
                let vn = self.point_velocity(r).dot(n);
                let k_n = self.inv_effective_mass(inertia, r, n);
                if k_n <= 1e-9 {
                    continue;
                }
                let new_acc = (acc_n[k] + (target_vn[k] - vn) / k_n).max(0.0);
                let dj = new_acc - acc_n[k];
                acc_n[k] = new_acc;
                if dj != 0.0 {
                    self.apply_impulse(inertia, r, n * dj);
                }

                let v = self.point_velocity(r);
                let vt = v - n * v.dot(n);
                let vt_len = vt.length();
                if vt_len > 1e-6 {
                    let t = vt / vt_len;
                    let k_t = self.inv_effective_mass(inertia, r, t);
                    if k_t > 1e-9 {
                        let old = acc_t[k];
                        let mut new_t = old - t * (vt_len / k_t);
                        let max_f = friction * acc_n[k];
                        let len = new_t.length();
                        if len > max_f {
                            new_t *= max_f / len.max(1e-9);
                        }
                        acc_t[k] = new_t;
                        self.apply_impulse(inertia, r, new_t - old);
                    }
                }
            }
        }
    }

    /// Rolling resistance and spin friction while in contact, plus a physical cap on spin.
    pub fn apply_contact_damping(&mut self, normal: Vec3, dt: f32) {
        let spin = normal * self.angular_velocity.dot(normal);
        let roll = self.angular_velocity - spin;
        self.angular_velocity =
            roll * (1.0 - ROLLING_RESISTANCE * dt).max(0.0) + spin * (1.0 - SPIN_FRICTION * dt).max(0.0);
        self.clamp_spin();
    }

    /// Caps |ω| so no surface point spins faster than [`MAX_SPIN_SURFACE_SPEED`].
    #[inline]
    pub fn clamp_spin(&mut self) {
        let max_w = MAX_SPIN_SURFACE_SPEED / self.bounding_radius();
        let w = self.angular_velocity.length();
        if w > max_w {
            self.angular_velocity *= max_w / w;
        }
    }

    pub fn lowest_world_y(&self) -> f32 {
        let mut min_y = f32::INFINITY;
        let rot_mat = self.rot_mat;
        // The lowest point of a solid always lies on its surface, so scanning the cached exposed
        // voxels is exact (strided sampling of all voxels could miss it on rotated chunks).
        let src: &[(Vec3, VoxelType)] = if self.exposed_voxels.is_empty() {
            &self.voxels
        } else {
            &self.exposed_voxels
        };
        let step = (src.len() / 4096).max(1);
        for j in (0..src.len()).step_by(step) {
            let wy = self.position.y + rot_mat.transform_y(src[j].0);
            if wy < min_y {
                min_y = wy;
            }
        }
        if min_y.is_finite() {
            min_y - VOXEL_SIZE * 0.5
        } else {
            self.position.y
        }
    }

    #[inline]
    pub fn world_aabb_from_mat(&self, rot_mat: &Mat3) -> (Vec3, Vec3) {
        let center = (self.local_aabb.0 + self.local_aabb.1) * 0.5;
        let extents = (self.local_aabb.1 - self.local_aabb.0) * 0.5;
        let world_center = self.position + rot_mat.transform_vector(center);
        let ex = rot_mat.m[0][0].abs() * extents.x + rot_mat.m[0][1].abs() * extents.y + rot_mat.m[0][2].abs() * extents.z;
        let ey = rot_mat.m[1][0].abs() * extents.x + rot_mat.m[1][1].abs() * extents.y + rot_mat.m[1][2].abs() * extents.z;
        let ez = rot_mat.m[2][0].abs() * extents.x + rot_mat.m[2][1].abs() * extents.y + rot_mat.m[2][2].abs() * extents.z;
        let world_extents = Vec3::new(ex, ey, ez);
        (world_center - world_extents, world_center + world_extents)
    }

    #[allow(dead_code)]
    #[inline]
    pub fn world_aabb(&self) -> (Vec3, Vec3) {
        self.world_aabb_from_mat(&self.rot_mat)
    }

    #[inline]
    pub fn transform_sub_aabb(pos: Vec3, local_aabb: (Vec3, Vec3), rot_mat: &Mat3) -> (Vec3, Vec3) {
        let center = (local_aabb.0 + local_aabb.1) * 0.5;
        let extents = (local_aabb.1 - local_aabb.0) * 0.5;
        let world_center = pos + rot_mat.transform_vector(center);
        let ex = rot_mat.m[0][0].abs() * extents.x + rot_mat.m[0][1].abs() * extents.y + rot_mat.m[0][2].abs() * extents.z;
        let ey = rot_mat.m[1][0].abs() * extents.x + rot_mat.m[1][1].abs() * extents.y + rot_mat.m[1][2].abs() * extents.z;
        let ez = rot_mat.m[2][0].abs() * extents.x + rot_mat.m[2][1].abs() * extents.y + rot_mat.m[2][2].abs() * extents.z;
        let world_extents = Vec3::new(ex, ey, ez);
        (world_center - world_extents, world_center + world_extents)
    }

    /// Zero-allocation hierarchical sub-block partitioning into pre-allocated reusable scratch buffers
    #[allow(dead_code)]
    pub fn populate_sub_blocks(&self, out: &mut Vec<((Vec3, Vec3), Vec<usize>)>) {
        let num_splits = if self.voxels.len() <= 24 {
            1
        } else if self.voxels.len() > 64 {
            4
        } else {
            2
        };

        SPARE_SUB_BLOCKS.with(|pool| {
            let mut pool = pool.borrow_mut();
            while out.len() > num_splits {
                let idx = out.len() - 1;
                let spare = out.pop().unwrap();
                if pool.len() <= idx {
                    pool.resize_with(idx + 1, || None);
                }
                pool[idx] = Some(spare);
            }
            while out.len() < num_splits {
                let idx = out.len();
                if idx < pool.len() && pool[idx].is_some() {
                    out.push(pool[idx].take().unwrap());
                } else {
                    out.push(((Vec3::ZERO, Vec3::ZERO), Vec::with_capacity(self.voxels.len() / num_splits + 4)));
                }
            }
        });

        for bucket in out.iter_mut() {
            bucket.0 = (Vec3::ZERO, Vec3::ZERO);
            bucket.1.clear();
        }

        if num_splits == 1 {
            out[0].0 = self.local_aabb;
            out[0].1.extend(0..self.voxels.len());
            return;
        }

        // Split along the longest local axis into 2 to 4 spatial clusters
        let extents = self.local_aabb.1 - self.local_aabb.0;
        let split_axis = if extents.x >= extents.y && extents.x >= extents.z {
            0
        } else if extents.y >= extents.z {
            1
        } else {
            2
        };

        let min_val = match split_axis {
            0 => self.local_aabb.0.x,
            1 => self.local_aabb.0.y,
            _ => self.local_aabb.0.z,
        };
        let span = match split_axis {
            0 => extents.x.max(0.01),
            1 => extents.y.max(0.01),
            _ => extents.z.max(0.01),
        };

        for (idx, (p, _)) in self.voxels.iter().enumerate() {
            let val = match split_axis {
                0 => p.x,
                1 => p.y,
                _ => p.z,
            };
            let norm = ((val - min_val) / span).clamp(0.0, 0.999);
            let b = ((norm * num_splits as f32).floor() as usize).min(num_splits - 1);
            out[b].1.push(idx);
        }

        for bucket in out.iter_mut() {
            if bucket.1.is_empty() {
                bucket.0 = (Vec3::ZERO, Vec3::ZERO);
                continue;
            }
            let mut b_min = Vec3::splat(f32::INFINITY);
            let mut b_max = Vec3::splat(f32::NEG_INFINITY);
            for &idx in &bucket.1 {
                let p = self.voxels[idx].0;
                b_min = b_min.min(p);
                b_max = b_max.max(p);
            }
            bucket.0 = (b_min, b_max);
        }
    }

    /// Partitions voxels into hierarchical sub-blocks with local AABBs for collision early-outs
    #[allow(dead_code)]
    pub fn sub_blocks(&self) -> Vec<((Vec3, Vec3), Vec<usize>)> {
        let mut result = Vec::with_capacity(4);
        self.populate_sub_blocks(&mut result);
        result.retain(|(_, bucket)| !bucket.is_empty());
        if result.is_empty() {
            let mut v = Vec::with_capacity(self.voxels.len());
            v.extend(0..self.voxels.len());
            result.push((self.local_aabb, v));
        }
        result
    }

    #[allow(dead_code)]
    #[inline]
    pub fn lower_world_aabb_from_mat(&self, rot_mat: &Mat3, lower_y_cutoff: f32) -> (Vec3, Vec3) {
        let (w_min, w_max) = self.world_aabb_from_mat(rot_mat);
        let capped_max_y = w_max.y.min(lower_y_cutoff);
        (w_min, Vec3::new(w_max.x, capped_max_y, w_max.z))
    }

    #[allow(dead_code)]
    pub fn highest_world_y_at(&self, world_xz: Vec2) -> Option<f32> {
        let approx_r = (self.voxels.len() as f32).powf(0.33) * VOXEL_SIZE * 0.9 + 0.6;
        let delta_x = (world_xz.x - self.position.x).abs();
        let delta_z = (world_xz.y - self.position.z).abs();
        if delta_x > approx_r || delta_z > approx_r {
            return None;
        }
        let rot_mat = self.rot_mat;
        let mut max_y = f32::NEG_INFINITY;
        let step = (self.voxels.len() / 24).max(1);
        for j in (0..self.voxels.len()).step_by(step) {
            let (p, _) = self.voxels[j];
            let wp = self.position + rot_mat.transform_vector(p);
            if (wp.x - world_xz.x).abs() <= VOXEL_SIZE * 0.8 && (wp.z - world_xz.y).abs() <= VOXEL_SIZE * 0.8 {
                let top = wp.y + VOXEL_SIZE * 0.5;
                if top > max_y {
                    max_y = top;
                }
            }
        }
        if max_y.is_finite() {
            Some(max_y)
        } else {
            None
        }
    }

    pub fn fracture(mut self) -> (Vec<DynamicChunk>, Vec<Vec3>, Vec<Vec3>) {
        let v_len = self.voxels.len();
        if v_len <= 40 {
            self.settled = true;
            self.velocity = Vec3::ZERO;
            self.angular_velocity = Vec3::ZERO;
            return (vec![self], Vec::new(), Vec::new());
        }

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;

        for (p, _) in &self.voxels {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }

        let dx = max_x - min_x;
        let dy = max_y - min_y;
        let dz = max_z - min_z;

        let num_pieces = if v_len >= 120 { 4 } else { 2 };
        let mut groups: Vec<Vec<usize>> = Vec::new();

        if num_pieces == 4 {
            let mut voxels: Vec<usize> = (0..self.voxels.len()).collect();
            if dx >= dy && dx >= dz {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.x.partial_cmp(&self.voxels[*b].0.x).unwrap_or(std::cmp::Ordering::Equal));
            } else if dy >= dx && dy >= dz {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.y.partial_cmp(&self.voxels[*b].0.y).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.z.partial_cmp(&self.voxels[*b].0.z).unwrap_or(std::cmp::Ordering::Equal));
            }

            let mid = voxels.len() / 2;
            let mut half2 = voxels.split_off(mid);
            let mut half1 = voxels;

            let (sec_dx, sec_dy, sec_dz) = if dx >= dy && dx >= dz {
                (0.0, dy, dz)
            } else if dy >= dx && dy >= dz {
                (dx, 0.0, dz)
            } else {
                (dx, dy, 0.0)
            };

            let sort_half = |h: &mut Vec<usize>| {
                if sec_dx >= sec_dy && sec_dx >= sec_dz {
                    h.sort_unstable_by(|a, b| self.voxels[*a].0.x.partial_cmp(&self.voxels[*b].0.x).unwrap_or(std::cmp::Ordering::Equal));
                } else if sec_dy >= sec_dx && sec_dy >= sec_dz {
                    h.sort_unstable_by(|a, b| self.voxels[*a].0.y.partial_cmp(&self.voxels[*b].0.y).unwrap_or(std::cmp::Ordering::Equal));
                } else {
                    h.sort_unstable_by(|a, b| self.voxels[*a].0.z.partial_cmp(&self.voxels[*b].0.z).unwrap_or(std::cmp::Ordering::Equal));
                }
            };

            sort_half(&mut half1);
            let m1 = half1.len() / 2;
            let q2 = half1.split_off(m1);
            let q1 = half1;

            sort_half(&mut half2);
            let m2 = half2.len() / 2;
            let q4 = half2.split_off(m2);
            let q3 = half2;

            if !q1.is_empty() { groups.push(q1); }
            if !q2.is_empty() { groups.push(q2); }
            if !q3.is_empty() { groups.push(q3); }
            if !q4.is_empty() { groups.push(q4); }
        } else {
            let mut voxels: Vec<usize> = (0..self.voxels.len()).collect();
            if dx >= dy && dx >= dz {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.x.partial_cmp(&self.voxels[*b].0.x).unwrap_or(std::cmp::Ordering::Equal));
            } else if dy >= dx && dy >= dz {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.y.partial_cmp(&self.voxels[*b].0.y).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                voxels.sort_unstable_by(|a, b| self.voxels[*a].0.z.partial_cmp(&self.voxels[*b].0.z).unwrap_or(std::cmp::Ordering::Equal));
            }

            let mid = voxels.len() / 2;
            let h2 = voxels.split_off(mid);
            let h1 = voxels;
            if !h1.is_empty() { groups.push(h1); }
            if !h2.is_empty() { groups.push(h2); }
        }

        let mut sub_chunks = Vec::with_capacity(groups.len());
        let mut exp_world = Vec::new();
        let mut core_world = Vec::new();
        let rot_mat = self.rot_mat;

        let mut sub_chunks_data = Vec::with_capacity(groups.len());
        let mut raw_scatters = Vec::with_capacity(groups.len());
        let mut total_scatter_momentum = Vec3::ZERO;
        let mut total_fracture_mass = 0.0f32;

        for group in groups {
            let mut weighted_pos = Vec3::ZERO;
            let mut mass = 0.0f32;
            for &index in &group {
                let (p, vt) = &self.voxels[index];
                let dm = vt.properties().density;
                weighted_pos += *p * dm;
                mass += dm;
                if *vt == VoxelType::Explosive {
                    let wp = self.position + rot_mat.transform_vector(*p);
                    exp_world.push(wp);
                } else if *vt == VoxelType::GlowCore {
                    let wp = self.position + rot_mat.transform_vector(*p);
                    core_world.push(wp);
                }
            }
            let sub_com_local = if mass > 1e-4 { weighted_pos / mass } else { Vec3::ZERO };
            let sub_world_pos = self.position + rot_mat.transform_vector(sub_com_local);

            let health: Vec<f32> = if self.voxel_health.is_empty() { Vec::new() } else {
                group.iter().map(|&index| self.voxel_health[index]).collect()
            };
            let new_voxels: Vec<(Vec3, VoxelType)> = group.into_iter().map(|index| {
                let (p, vt) = self.voxels[index];
                (p - sub_com_local, vt)
            }).collect();

            let norm_local = sub_com_local.normalize_or_zero();
            let mut scatter_world = rot_mat.transform_vector(norm_local);
            if scatter_world.length_squared() < 0.01 {
                scatter_world = Vec3::new(
                    rand::gen_range(-1.0, 1.0),
                    rand::gen_range(0.2, 0.8),
                    rand::gen_range(-1.0, 1.0),
                ).normalize();
            }
            let scatter_speed = rand::gen_range(1.5, 3.5);
            let raw_v_scatter = (scatter_world * 0.82 + Vec3::Y * 0.42).normalize_or_zero() * scatter_speed;

            total_scatter_momentum += raw_v_scatter * mass;
            total_fracture_mass += mass;

            raw_scatters.push(raw_v_scatter);
            sub_chunks_data.push((sub_world_pos, new_voxels, mass, health));
        }

        // Zero-momentum projection: eliminate net momentum drift from scatter forces
        let drift_vel = if total_fracture_mass > 1e-4 {
            total_scatter_momentum / total_fracture_mass
        } else {
            Vec3::ZERO
        };

        for (i, (sub_world_pos, new_voxels, mass, health)) in sub_chunks_data.into_iter().enumerate() {
            // Preserve linear velocity, angular velocity, and tangential velocity (omega x r_offset)
            let r_offset = sub_world_pos - self.position;
            let tangential_vel = self.angular_velocity.cross(r_offset);
            let physical_v_scatter = raw_scatters[i] - drift_vel;
            let new_vel = self.velocity + tangential_vel + physical_v_scatter;
            let ang_scatter = Vec3::new(
                rand::gen_range(-1.5, 1.5),
                rand::gen_range(-1.5, 1.5),
                rand::gen_range(-1.5, 1.5),
            );
            let new_ang = self.angular_velocity + ang_scatter;

            let sub_aabb = DynamicChunk::compute_aabb(&new_voxels);
            // Crushing can add mass without adding voxels. Keep that mass when
            // fracturing, distributed by each piece's material density.
            let child_mass = self.mass * (mass / total_fracture_mass);
            let mut child = DynamicChunk::new(
                sub_world_pos,
                new_vel,
                self.rotation,
                new_ang,
                new_voxels,
                child_mass,
                0.0,
                false,
                sub_aabb,
            );
            child.voxel_health = health;
            sub_chunks.push(child);
        }

        (sub_chunks, exp_world, core_world)
    }

    /// Arbitrary 3D planar bisection of this dynamic rigid body chunk.
    /// Slices the chunk along the plane defined by `plane_normal` and `plane_dist` (n·x + d = 0).
    /// If the plane cuts through the chunk, produces two cleanly severed child chunks with
    /// exact conservation of mass, recalculated centers of mass, and realistic separating impulses.
    #[allow(clippy::result_large_err)]
    pub fn slice_by_plane(
        self,
        plane_normal: Vec3,
        plane_dist: f32,
    ) -> Result<(DynamicChunk, DynamicChunk), DynamicChunk> {
        let mut side_a: Vec<(Vec3, VoxelType)> = Vec::new();
        let mut side_b: Vec<(Vec3, VoxelType)> = Vec::new();
        let mut health_a = Vec::new();
        let mut health_b = Vec::new();

        for (index, &(local_p, vt)) in self.voxels.iter().enumerate() {
            let world_p = self.local_to_world(local_p);
            let dist = plane_normal.dot(world_p) + plane_dist;
            if dist > 0.0 {
                side_a.push((local_p, vt));
                if !self.voxel_health.is_empty() { health_a.push(self.voxel_health[index]); }
            } else {
                side_b.push((local_p, vt));
                if !self.voxel_health.is_empty() { health_b.push(self.voxel_health[index]); }
            }
        }

        if side_a.is_empty() || side_b.is_empty() {
            return Err(self);
        }

        // Child A: centroid & local body frame (snapped to VOXEL_SIZE)
        let mut sum_a = Vec3::ZERO;
        let mut mass_a = 0.0f32;
        for (lp, vt) in &side_a {
            let density = vt.properties().density;
            sum_a += *lp * density;
            mass_a += density;
        }
        let com_local_a = sum_a / mass_a.max(0.1);
        let snap_com_a = (com_local_a / VOXEL_SIZE).round() * VOXEL_SIZE;
        let world_com_a = self.position + self.rot_mat.transform_vector(snap_com_a);
        let voxels_a: Vec<(Vec3, VoxelType)> = side_a.into_iter().map(|(lp, vt)| (lp - snap_com_a, vt)).collect();
        let aabb_a = DynamicChunk::compute_aabb(&voxels_a);

        // Child B: centroid & local body frame (snapped to VOXEL_SIZE)
        let mut sum_b = Vec3::ZERO;
        let mut mass_b = 0.0f32;
        for (lp, vt) in &side_b {
            let density = vt.properties().density;
            sum_b += *lp * density;
            mass_b += density;
        }
        let com_local_b = sum_b / mass_b.max(0.1);
        let snap_com_b = (com_local_b / VOXEL_SIZE).round() * VOXEL_SIZE;
        let world_com_b = self.position + self.rot_mat.transform_vector(snap_com_b);
        let voxels_b: Vec<(Vec3, VoxelType)> = side_b.into_iter().map(|(lp, vt)| (lp - snap_com_b, vt)).collect();
        let aabb_b = DynamicChunk::compute_aabb(&voxels_b);

        // Realistic separating impulses along cutting plane normal
        let separation_speed = 3.2f32;
        let vel_a = self.velocity + plane_normal * separation_speed;
        let vel_b = self.velocity - plane_normal * separation_speed;

        let ang_kick = plane_normal.cross(Vec3::Y) * 1.5;
        let omega_a = self.angular_velocity + ang_kick;
        let omega_b = self.angular_velocity - ang_kick;

        let total_split_mass = mass_a + mass_b;
        let final_mass_a = if total_split_mass > 0.0 {
            self.mass * (mass_a / total_split_mass)
        } else {
            self.mass * 0.5
        };
        let final_mass_b = self.mass - final_mass_a;

        let mut child_a = DynamicChunk::new(
            world_com_a,
            vel_a,
            self.rotation,
            omega_a,
            voxels_a,
            final_mass_a,
            self.life.max(5.0),
            false,
            aabb_a,
        );

        let mut child_b = DynamicChunk::new(
            world_com_b,
            vel_b,
            self.rotation,
            omega_b,
            voxels_b,
            final_mass_b,
            self.life.max(5.0),
            false,
            aabb_b,
        );

        child_a.voxel_health = health_a;
        child_b.voxel_health = health_b;
        Ok((child_a, child_b))
    }
}


#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DecalKind {
    Scorch,
    PlasmaBurn,
    CryoFrost,
    KineticImpact,
    QuenchedObsidian,
}

pub struct ImpactDecal {
    pub position: Vec3,
    pub normal: Vec3,
    pub size: f32,
    pub color: Color,
    pub life: f32,
    pub max_life: f32,
    pub kind: DecalKind,
    pub voxel_anchor: Option<IVec3>,
}

#[derive(Copy, Clone, Debug)]
pub struct DynamicPointLight {
    pub position: Vec3,
    pub color: Color,
    pub radius: f32,
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsCullMetrics {
    pub chunks_total: usize,
    pub chunks_drawn: usize,
    pub debris_total: usize,
    pub debris_drawn: usize,
    pub sparks_total: usize,
    pub sparks_drawn: usize,
    pub smoke_total: usize,
    pub smoke_drawn: usize,
    pub shockwaves_total: usize,
    pub shockwaves_drawn: usize,
    pub arcs_total: usize,
    pub arcs_drawn: usize,
    pub fire_total: usize,
    pub fire_drawn: usize,
    pub decals_total: usize,
    pub decals_drawn: usize,
}

#[allow(dead_code)]
impl PhysicsCullMetrics {
    #[inline]
    pub fn particles_total(&self) -> usize {
        self.debris_total + self.sparks_total + self.smoke_total + self.shockwaves_total + self.arcs_total + self.fire_total
    }

    #[inline]
    pub fn particles_drawn(&self) -> usize {
        self.debris_drawn + self.sparks_drawn + self.smoke_drawn + self.shockwaves_drawn + self.arcs_drawn + self.fire_drawn
    }

    #[inline]
    pub fn particles_culled(&self) -> usize {
        self.particles_total().saturating_sub(self.particles_drawn())
    }

    #[inline]
    pub fn cull_ratio(&self) -> f32 {
        let tot = self.particles_total() + self.chunks_total + self.decals_total;
        if tot == 0 {
            0.0
        } else {
            let drawn = self.particles_drawn() + self.chunks_drawn + self.decals_drawn;
            (tot.saturating_sub(drawn)) as f32 / tot as f32
        }
    }

    #[inline]
    pub fn total_particles_drawn(&self) -> usize {
        self.particles_drawn()
    }

    #[inline]
    pub fn total_particles_culled(&self) -> usize {
        self.particles_culled()
    }

    #[inline]
    pub fn total_particles(&self) -> usize {
        self.particles_total()
    }

    #[inline]
    pub fn cull_efficiency_percent(&self) -> f32 {
        self.cull_ratio() * 100.0
    }

    #[inline]
    pub fn debris_culled(&self) -> usize {
        self.debris_total.saturating_sub(self.debris_drawn)
    }

    #[inline]
    pub fn sparks_culled(&self) -> usize {
        self.sparks_total.saturating_sub(self.sparks_drawn)
    }

    #[inline]
    pub fn smoke_culled(&self) -> usize {
        self.smoke_total.saturating_sub(self.smoke_drawn)
    }

    #[inline]
    pub fn decals_culled(&self) -> usize {
        self.decals_total.saturating_sub(self.decals_drawn)
    }

    #[inline]
    pub fn chunks_culled(&self) -> usize {
        self.chunks_total.saturating_sub(self.chunks_drawn)
    }
}

pub struct PhysicsSystem {
    pub chunks: Vec<DynamicChunk>,
    pub debris: Vec<DebrisParticle>,
    pub smoke: Vec<SmokeParticle>,
    pub sparks: Vec<SparkParticle>,
    pub shockwaves: Vec<ShockwaveRing>,
    pub decals: Vec<ImpactDecal>,
    pub electrical_arcs: Vec<ElectricalArc>,
    pub fire_jets: Vec<PressurizedFireJet>,
    pub total_collapsed_chunks: usize,
    pub cull_metrics: std::cell::Cell<PhysicsCullMetrics>,
    pub scratch_chunks_to_fracture: Vec<(usize, f32)>,
    pub scratch_fuel_detonations: Vec<IVec3>,
    pub scratch_core_breaches: Vec<IVec3>,
    pub scratch_ice_shatters: Vec<(Vec3, usize)>,
    pub scratch_crush_dust: Vec<(Vec3, f32)>,
    pub scratch_craters_to_unsettle: Vec<(Vec3, f32, f32)>,
    pub scratch_chunks_to_settle: Vec<usize>,
    pub scratch_new_crush_chunks: Vec<DynamicChunk>,
    pub scratch_fracture_sub_chunks: Vec<DynamicChunk>,
    pub scratch_blasted_settled_chunks: Vec<(Vec3, f32)>,
    // === Milestone 2 Features 7 & 8 ===
    /// 96x96 spatial heightmap of settled rubble top elevations (row-major: gx * GRID_Z + gz).
    /// Base level is initialized to VOXEL_SIZE (0.50 = top of bedrock).
    pub rubble_height_grid: Vec<f32>,
    /// Highest settled rubble elevation across the world (for fast AABB collision early-outs).
    pub max_rubble_height: f32,
    /// Flag indicating settled chunk topology changed.
    pub rubble_dirty: bool,
    /// Optional bounding box (min_voxel, max_voxel) of localized rubble dirty region.
    pub rubble_dirty_region: Option<(IVec3, IVec3)>,
    /// Scratch buffer for pulverized voxels during pancake crushing (Zero-allocation!).
    pub scratch_crushed_voxels: Vec<(IVec3, VoxelType)>,
    /// Scratch buffer for hierarchical sub-blocks during pancake crush narrowphase (Zero-allocation!).
    pub scratch_sub_blocks: Vec<((Vec3, Vec3), Vec<usize>)>,
    /// Broadphase scratch for chunk-vs-chunk collision: (chunk index, world AABB min, max).
    pub scratch_pair_aabbs: Vec<(usize, Vec3, Vec3)>,
    /// Narrowphase scratch: world-space surface voxel centers of each chunk in an overlap region.
    pub scratch_overlap_a: Vec<Vec3>,
    pub scratch_overlap_b: Vec<Vec3>,
    pub batcher: std::cell::RefCell<ParticleBatcher>,
    pub elapsed_time: f32,
    pub wind: Vec3,
}

impl PhysicsSystem {
    pub const MAX_DEBRIS: usize = 3000;
    pub const SOFT_DEBRIS: usize = 2400;
    pub const MAX_SPARKS: usize = 800;
    pub const SOFT_SPARKS: usize = 640;
    pub const MAX_SMOKE: usize = 900;
    pub const SOFT_SMOKE: usize = 720;
    pub const MAX_SHOCKWAVES: usize = 64;
    pub const SOFT_SHOCKWAVES: usize = 48;
    pub const MAX_DECALS: usize = 96;
    pub const SOFT_DECALS: usize = 80;

    pub fn new() -> Self {
        let grid_cells = crate::voxel::GRID_X * crate::voxel::GRID_Z;
        Self {
            chunks: Vec::with_capacity(512),
            debris: Vec::with_capacity(4096),
            smoke: Vec::with_capacity(1024),
            sparks: Vec::with_capacity(1024),
            shockwaves: Vec::with_capacity(128),
            decals: Vec::with_capacity(128),
            electrical_arcs: Vec::with_capacity(64),
            fire_jets: Vec::with_capacity(64),
            total_collapsed_chunks: 0,
            cull_metrics: std::cell::Cell::new(PhysicsCullMetrics::default()),
            scratch_chunks_to_fracture: Vec::with_capacity(16),
            scratch_fuel_detonations: Vec::with_capacity(16),
            scratch_core_breaches: Vec::with_capacity(16),
            scratch_ice_shatters: Vec::with_capacity(16),
            scratch_crush_dust: Vec::with_capacity(32),
            scratch_craters_to_unsettle: Vec::with_capacity(32),
            scratch_chunks_to_settle: Vec::with_capacity(32),
            scratch_new_crush_chunks: Vec::with_capacity(32),
            scratch_fracture_sub_chunks: Vec::with_capacity(32),
            scratch_blasted_settled_chunks: Vec::with_capacity(32),
            rubble_height_grid: vec![VOXEL_SIZE; grid_cells],
            max_rubble_height: VOXEL_SIZE,
            rubble_dirty: false,
            rubble_dirty_region: None,
            scratch_crushed_voxels: Vec::with_capacity(128),
            scratch_sub_blocks: Vec::with_capacity(4),
            scratch_pair_aabbs: Vec::with_capacity(64),
            scratch_overlap_a: Vec::with_capacity(256),
            scratch_overlap_b: Vec::with_capacity(256),
            batcher: std::cell::RefCell::new(ParticleBatcher::new()),
            elapsed_time: 0.0,
            wind: Vec3::new(2.5, 0.2, 1.0),
        }
    }

    /// Cleanly wipes all active chunks, debris, particles, decals, and resets the rubble heightmap
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.debris.clear();
        self.smoke.clear();
        self.sparks.clear();
        self.shockwaves.clear();
        self.decals.clear();
        self.electrical_arcs.clear();
        self.fire_jets.clear();
        self.total_collapsed_chunks = 0;
        self.cull_metrics.set(PhysicsCullMetrics::default());
        self.scratch_chunks_to_fracture.clear();
        self.scratch_fuel_detonations.clear();
        self.scratch_core_breaches.clear();
        self.scratch_ice_shatters.clear();
        self.scratch_crush_dust.clear();
        self.scratch_craters_to_unsettle.clear();
        self.scratch_chunks_to_settle.clear();
        self.scratch_new_crush_chunks.clear();
        self.scratch_fracture_sub_chunks.clear();
        self.scratch_blasted_settled_chunks.clear();
        self.scratch_crushed_voxels.clear();
        for bucket in self.scratch_sub_blocks.iter_mut() {
            bucket.0 = (Vec3::ZERO, Vec3::ZERO);
            bucket.1.clear();
        }
        self.rubble_height_grid.fill(VOXEL_SIZE);
        self.max_rubble_height = VOXEL_SIZE;
        self.rubble_dirty = false;
        self.rubble_dirty_region = None;
        self.elapsed_time = 0.0;
    }

    /// Ablates detached material using the same resistance and radial falloff as the grid.
    /// Return destroyed count and volatile triggers at their actual moving world positions.
    pub fn carve_chunk_sphere(
        &mut self,
        grid: &VoxelGrid,
        center: Vec3,
        radius: f32,
        base_damage: f32,
        dtype: crate::voxel::DamageType,
        preserve_bedrock: bool,
    ) -> (usize, Vec<(Vec3, VoxelType)>) {
        if radius <= 0.0 || !radius.is_finite() || base_damage <= 0.0 || !center.is_finite() {
            return (0, Vec::new());
        }
        let mut destroyed = 0;
        let mut volatiles = Vec::new();
        let mut dirty_min = Vec3::splat(f32::INFINITY);
        let mut dirty_max = Vec3::splat(f32::NEG_INFINITY);
        let max_radius = radius * 1.35;
        for chunk in &mut self.chunks {
            if chunk.position.distance_squared(center) > (chunk.bounding_radius() + max_radius).powi(2) {
                continue;
            }
            let old_len = chunk.voxels.len();
            let old_bounds = chunk.world_aabb();
            let mut old_density = 0.0;
            let mut new_density = 0.0;
            let mut weighted_center = Vec3::ZERO;
            let position = chunk.position;
            let rotation = chunk.rot_mat;
            if chunk.voxel_health.is_empty() {
                chunk.voxel_health.extend(chunk.voxels.iter().map(|(_, material)| material.properties().max_health));
            }
            let mut read_index = 0;
            let mut write_index = 0;
            let health = &mut chunk.voxel_health;
            chunk.voxels.retain(|&(local, material)| {
                let props = material.properties();
                old_density += props.density;
                let world = position + rotation.transform_vector(local);
                let material_radius = if matches!(material, VoxelType::Glass | VoxelType::CryoIce) {
                    max_radius
                } else {
                    radius
                };
                let distance = world.distance(center);
                let protected = preserve_bedrock && material == VoxelType::Bedrock;
                let falloff = (1.0 - distance / material_radius).max(0.08);
                let damage = if !protected && distance <= material_radius {
                    material.effective_damage(base_damage * falloff, dtype)
                } else {
                    0.0
                };
                let remaining = health[read_index] - damage;
                read_index += 1;
                let remove = remaining <= 0.0;
                if remove {
                    destroyed += 1;
                    if matches!(material, VoxelType::Explosive | VoxelType::GlowCore | VoxelType::PowerCapacitor) {
                        volatiles.push((world, material));
                    }
                } else {
                    health[write_index] = remaining;
                    write_index += 1;
                    new_density += props.density;
                    weighted_center += local * props.density;
                }
                !remove
            });
            health.truncate(write_index);
            if old_len == chunk.voxels.len() {
                continue;
            }
            dirty_min = dirty_min.min(old_bounds.0);
            dirty_max = dirty_max.max(old_bounds.1);
            if chunk.voxels.is_empty() {
                continue;
            }
            let shift = weighted_center / new_density.max(0.1);
            let world_shift = rotation.transform_vector(shift);
            chunk.position += world_shift;
            chunk.velocity += chunk.angular_velocity.cross(world_shift);
            for (local, _) in &mut chunk.voxels {
                *local -= shift;
            }
            chunk.mass *= new_density / old_density.max(0.1);
            chunk.local_aabb = DynamicChunk::compute_aabb(&chunk.voxels);
            chunk.rebuild_exposed_voxels();
            chunk.settled = false;
        }
        if destroyed > 0 {
            self.chunks.retain(|chunk| !chunk.voxels.is_empty());
            let min = grid.world_to_voxel(dirty_min - Vec3::splat(VOXEL_SIZE));
            let max = grid.world_to_voxel(dirty_max + Vec3::splat(VOXEL_SIZE));
            // Refresh erased supports before the next support prepass or mission grading.
            self.rebuild_rubble_heightmap_in_region(grid, min, max);
            self.rubble_dirty = true;
            self.rubble_dirty_region = None;
        }
        (destroyed, volatiles)
    }

    /// Slices all active dynamic chunks that intersect the cutting plane at `origin` with normal `normal`.
    pub fn slice_intersecting_chunks(&mut self, origin: Vec3, normal: Vec3, max_reach: f32) -> usize {
        let plane_dist = -normal.dot(origin);
        let mut sliced_count = 0;
        let mut new_chunks = Vec::new();
        let mut i = 0;

        while i < self.chunks.len() {
            if self.chunks[i].position.distance(origin) > max_reach {
                i += 1;
                continue;
            }

            // Check if chunk spans both sides of the slicing plane before removing
            let mut has_side_a = false;
            let mut has_side_b = false;
            for &(lp, _) in &self.chunks[i].voxels {
                let wp = self.chunks[i].local_to_world(lp);
                let d = normal.dot(wp) + plane_dist;
                if d > 0.0 {
                    has_side_a = true;
                } else {
                    has_side_b = true;
                }
                if has_side_a && has_side_b {
                    break;
                }
            }

            if has_side_a && has_side_b {
                let chunk = self.chunks.swap_remove(i);
                match chunk.slice_by_plane(normal, plane_dist) {
                    Ok((a, b)) => {
                        sliced_count += 1;
                        new_chunks.push(a);
                        new_chunks.push(b);
                        // Do NOT increment i: swap_remove placed an uninspected chunk from the end at index i
                    }
                    Err(original) => {
                        self.chunks.push(original);
                        let last = self.chunks.len() - 1;
                        self.chunks.swap(i, last);
                        i += 1;
                    }
                }
            } else {
                i += 1;
            }
        }

        self.chunks.extend(new_chunks);
        sliced_count
    }

    #[inline]
    pub fn world_to_rubble_grid_coords(world_x: f32, world_z: f32) -> Option<(usize, usize)> {
        if !world_x.is_finite() || !world_z.is_finite() {
            return None;
        }
        let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let gx = ((world_x + half_x) / VOXEL_SIZE).floor() as i32;
        let gz = ((world_z + half_z) / VOXEL_SIZE).floor() as i32;
        if gx >= 0 && (gx as usize) < crate::voxel::GRID_X && gz >= 0 && (gz as usize) < crate::voxel::GRID_Z {
            Some((gx as usize, gz as usize))
        } else {
            None
        }
    }

    #[inline]
    pub fn world_pos_to_voxel(pos: Vec3) -> IVec3 {
        let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let vx = ((pos.x + half_x) / VOXEL_SIZE).floor() as i32;
        let vy = (pos.y / VOXEL_SIZE).floor() as i32;
        let vz = ((pos.z + half_z) / VOXEL_SIZE).floor() as i32;
        IVec3::new(vx, vy, vz)
    }

    #[inline]
    pub fn rubble_height_from_slice(grid: &[f32], world_x: f32, world_z: f32) -> f32 {
        if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(world_x, world_z) {
            grid[gx * crate::voxel::GRID_Z + gz]
        } else if world_x.abs() <= 240.0 && world_z.abs() <= 240.0 {
            VOXEL_SIZE
        } else {
            VOID_BOUNDARY_Y
        }
    }

    pub fn rubble_normal_from_slice(grid: &[f32], world_x: f32, world_z: f32) -> Vec3 {
        let step = VOXEL_SIZE;
        let h_l = Self::rubble_height_from_slice(grid, world_x - step, world_z);
        let h_r = Self::rubble_height_from_slice(grid, world_x + step, world_z);
        let h_d = Self::rubble_height_from_slice(grid, world_x, world_z - step);
        let h_u = Self::rubble_height_from_slice(grid, world_x, world_z + step);

        let dh_dx = (h_r - h_l) / (2.0 * step);
        let dh_dz = (h_u - h_d) / (2.0 * step);

        Vec3::new(-dh_dx, 1.0, -dh_dz).normalize_or_zero()
    }

    #[inline]
    pub fn rubble_height_at(&self, world_x: f32, world_z: f32) -> f32 {
        Self::rubble_height_from_slice(&self.rubble_height_grid, world_x, world_z)
    }

    #[allow(dead_code)]
    pub fn rubble_normal_at(&self, world_x: f32, world_z: f32) -> Vec3 {
        Self::rubble_normal_from_slice(&self.rubble_height_grid, world_x, world_z)
    }

    #[allow(dead_code)]
    pub fn rubble_heightmap_set(&mut self, world_x: f32, world_z: f32, height: f32) {
        if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(world_x, world_z) {
            let idx = gx * crate::voxel::GRID_Z + gz;
            if idx < self.rubble_height_grid.len() {
                self.rubble_height_grid[idx] = height;
                if height > self.max_rubble_height {
                    self.max_rubble_height = height;
                }
            }
        }
    }

    pub fn settle_chunk(&mut self, chunk_idx: usize) {
        if chunk_idx >= self.chunks.len() {
            return;
        }
        self.chunks[chunk_idx].settled = true;
        self.chunks[chunk_idx].velocity = Vec3::ZERO;
        self.chunks[chunk_idx].angular_velocity = Vec3::ZERO;

        let chunk = &self.chunks[chunk_idx];
        for (local_v, _) in &chunk.voxels {
            let wp = chunk.local_to_world(*local_v);
            let top_y = wp.y + VOXEL_SIZE * 0.5;
            if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(wp.x, wp.z) {
                let idx = gx * crate::voxel::GRID_Z + gz;
                let cur_h = self.rubble_height_grid[idx];
                if top_y > self.max_rubble_height {
                    self.max_rubble_height = top_y;
                }
                if top_y > cur_h {
                    self.rubble_height_grid[idx] = top_y;
                }
            }
        }
    }

    pub fn rebuild_rubble_heightmap_with_grid(&mut self, grid: &VoxelGrid) {
        for gx in 0..crate::voxel::GRID_X {
            for gz in 0..crate::voxel::GRID_Z {
                let idx = gx * crate::voxel::GRID_Z + gz;
                // Find highest solid ground voxel at or below surface level (vy <= 0)
                let mut floor_y = VOID_BOUNDARY_Y;
                for vy in (MIN_VOXEL_Y..=0).rev() {
                    if grid.in_bounds(gx as i32, vy, gz as i32) && grid.get(gx as i32, vy, gz as i32).is_solid() {
                        let v_world = grid.voxel_to_world(IVec3::new(gx as i32, vy, gz as i32));
                        floor_y = v_world.y + VOXEL_SIZE * 0.5;
                        break;
                    }
                }
                self.rubble_height_grid[idx] = floor_y;
            }
        }
        self.max_rubble_height = VOXEL_SIZE;
        for chunk in &self.chunks {
            if chunk.settled {
                for (local_v, _) in &chunk.voxels {
                    let wp = chunk.local_to_world(*local_v);
                    let top_y = wp.y + VOXEL_SIZE * 0.5;
                    if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(wp.x, wp.z) {
                        let idx = gx * crate::voxel::GRID_Z + gz;
                        let cur_h = self.rubble_height_grid[idx];
                        if top_y > self.max_rubble_height {
                            self.max_rubble_height = top_y;
                        }
                        if top_y > cur_h {
                            self.rubble_height_grid[idx] = top_y;
                        }
                    }
                }
            }
        }
    }

    #[inline]
    pub fn mark_rubble_region_dirty(&mut self, min_pos: IVec3, max_pos: IVec3) {
        self.rubble_dirty = true;
        if let Some((curr_min, curr_max)) = self.rubble_dirty_region {
            self.rubble_dirty_region = Some((curr_min.min(min_pos), curr_max.max(max_pos)));
        } else {
            self.rubble_dirty_region = Some((min_pos, max_pos));
        }
    }

    /// Localized rebuild of the rubble heightfield bounded strictly to the affected excavation/settling AABB.
    /// Replaces whole-world 451,584 voxel checks with a tight column sweep.
    pub fn rebuild_rubble_heightmap_in_region(&mut self, grid: &VoxelGrid, min_pos: IVec3, max_pos: IVec3) {
        let min_gx = (min_pos.x.min(max_pos.x).max(0) as usize).min(crate::voxel::GRID_X - 1);
        let max_gx = (min_pos.x.max(max_pos.x).max(0) as usize).min(crate::voxel::GRID_X - 1);
        let min_gz = (min_pos.z.min(max_pos.z).max(0) as usize).min(crate::voxel::GRID_Z - 1);
        let max_gz = (min_pos.z.max(max_pos.z).max(0) as usize).min(crate::voxel::GRID_Z - 1);

        for gx in min_gx..=max_gx {
            for gz in min_gz..=max_gz {
                let idx = gx * crate::voxel::GRID_Z + gz;
                // Find highest solid ground voxel at or below surface level (vy <= 0)
                let mut floor_y = VOID_BOUNDARY_Y;
                for vy in (MIN_VOXEL_Y..=0).rev() {
                    if grid.in_bounds(gx as i32, vy, gz as i32) && grid.get(gx as i32, vy, gz as i32).is_solid() {
                        let v_world = grid.voxel_to_world(IVec3::new(gx as i32, vy, gz as i32));
                        floor_y = v_world.y + VOXEL_SIZE * 0.5;
                        break;
                    }
                }
                self.rubble_height_grid[idx] = floor_y;
            }
        }

        let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let r_min_x = min_gx as f32 * VOXEL_SIZE - half_x;
        let r_max_x = (max_gx + 1) as f32 * VOXEL_SIZE - half_x;
        let r_min_z = min_gz as f32 * VOXEL_SIZE - half_z;
        let r_max_z = (max_gz + 1) as f32 * VOXEL_SIZE - half_z;

        for chunk in &self.chunks {
            if chunk.settled {
                let (c_min, c_max) = chunk.world_aabb();
                if c_max.x >= r_min_x && c_min.x <= r_max_x && c_max.z >= r_min_z && c_min.z <= r_max_z {
                    for (local_v, _) in &chunk.voxels {
                        let wp = chunk.local_to_world(*local_v);
                        let top_y = wp.y + VOXEL_SIZE * 0.5;
                        if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(wp.x, wp.z) {
                            if gx >= min_gx && gx <= max_gx && gz >= min_gz && gz <= max_gz {
                                let idx = gx * crate::voxel::GRID_Z + gz;
                                let cur_h = self.rubble_height_grid[idx];
                                if top_y > cur_h {
                                    self.rubble_height_grid[idx] = top_y;
                                }
                            }
                        }
                    }
                }
            }
        }

        self.max_rubble_height = self.rubble_height_grid.iter().copied().fold(VOXEL_SIZE, f32::max);
    }

    #[allow(dead_code)]
    pub fn rebuild_rubble_heightmap(&mut self) {
        self.rubble_height_grid.fill(VOXEL_SIZE);
        self.max_rubble_height = VOXEL_SIZE;
        for chunk in &self.chunks {
            if chunk.settled {
                for (local_v, _) in &chunk.voxels {
                    let wp = chunk.local_to_world(*local_v);
                    let top_y = wp.y + VOXEL_SIZE * 0.5;
                    if top_y > self.max_rubble_height {
                        self.max_rubble_height = top_y;
                    }
                    if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(wp.x, wp.z) {
                        let idx = gx * crate::voxel::GRID_Z + gz;
                        if top_y > self.rubble_height_grid[idx] {
                            self.rubble_height_grid[idx] = top_y;
                        }
                    }
                }
            }
        }
    }

    pub fn spawn_island_chunk(&mut self, island: IslandChunk, initial_impulse: Vec3, angular_impulse: Vec3) {
        let center = island.center_of_mass;
        let mass = island.calculate_mass();
        let mut local_voxels = Vec::with_capacity(island.voxels.len());
        for (vpos, vt) in island.voxels {
            let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
            let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
            let world_v = Vec3::new(
                vpos.x as f32 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5,
                vpos.y as f32 * VOXEL_SIZE + VOXEL_SIZE * 0.5,
                vpos.z as f32 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5,
            );
            local_voxels.push((world_v - center, vt));
        }

        let aabb = DynamicChunk::compute_aabb(&local_voxels);
        self.chunks.push(DynamicChunk::new(
            center,
            initial_impulse,
            Vec3::ZERO,
            angular_impulse,
            local_voxels,
            mass,
            0.0,
            false,
            aabb,
        ));

        self.chunks.last_mut().expect("Newly spawned island").voxel_health = island.voxel_health;
        self.total_collapsed_chunks += 1;
    }

    pub fn add_explosion_effects(
        &mut self,
        center: Vec3,
        radius: f32,
        impulse: f32,
        core_color: Color,
        destroyed_voxels: usize,
    ) {
        // Shockwave ring
        self.shockwaves.push(ShockwaveRing {
            position: center,
            normal: Vec3::Y,
            radius: 0.5,
            max_radius: radius * 3.2,
            speed: 35.0,
            color: core_color,
            thickness: 0.35,
        });

        // Flying debris particles
        let debris_count = destroyed_voxels.min(220) + 25;
        for _ in 0..debris_count {
            let dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(0.2f32, 1.5),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(6.0, 22.0) * (impulse / 10.0).clamp(0.5, 2.5);
            let size = rand::gen_range(0.12, 0.32);
            let life = rand::gen_range(6.0, 14.0);

            self.debris.push(DebrisParticle {
                position: center + dir * rand::gen_range(0.1, radius * 0.5),
                velocity: dir * speed,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::new(
                    rand::gen_range(-10.0, 10.0),
                    rand::gen_range(-10.0, 10.0),
                    rand::gen_range(-10.0, 10.0),
                ),
                size,
                color: core_color,
                life,
                max_life: life,
                bounces: 0,
            });
        }

        // Smoke billows - subtle atmospheric wisps
        let smoke_count = (radius * 0.6).clamp(1.0, 3.0) as usize;
        for _ in 0..smoke_count {
            let s_dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(-0.1f32, 1.0),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(1.0, 2.5);
            let s_life = rand::gen_range(0.35, 0.65);
            let start_size = rand::gen_range(0.06, 0.12);
            let max_size = rand::gen_range(0.18, 0.38);

            self.smoke.push(SmokeParticle {
                position: center + s_dir * rand::gen_range(0.1, radius * 0.3),
                velocity: s_dir * speed + Vec3::new(0.0, 0.5, 0.0),
                size: start_size,
                start_size,
                max_size,
                color: Color::new(0.40, 0.39, 0.38, 0.14),
                life: s_life,
                max_life: s_life,
            });
        }

        // Sparks
        let spark_count = (radius * 8.0).min(40.0) as usize;
        for _ in 0..spark_count {
            let sp_dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(0.3f32, 1.5),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(12.0, 30.0);
            let sp_life = rand::gen_range(0.6, 1.8);

            self.sparks.push(SparkParticle {
                position: center,
                velocity: sp_dir * speed,
                color: Color::new(1.0, 0.85, 0.3, 1.0),
                life: sp_life,
                max_life: sp_life,
            });
        }

        // Re-wake settled rubble within explosion crater
        self.un_settle_rubble_crater(center, (radius * 1.4).max(2.0), impulse);
    }

    pub fn un_settle_rubble_crater(&mut self, center: Vec3, radius: f32, force: f32) {
        let mut woke_count = 0;
        let mut woke_bounds: Option<(IVec3, IVec3)> = None;
        for chunk in &mut self.chunks {
            if chunk.settled {
                let dist = (chunk.position - center).length();
                let chunk_rad = chunk.local_aabb.0.length().max(chunk.local_aabb.1.length()) + VOXEL_SIZE;
                if dist <= radius + chunk_rad {
                    chunk.settled = false;
                    let (c_min, c_max) = chunk.world_aabb();
                    let c_min_vox = Self::world_pos_to_voxel(c_min);
                    let c_max_vox = Self::world_pos_to_voxel(c_max);
                    if let Some((wb_min, wb_max)) = woke_bounds {
                        woke_bounds = Some((wb_min.min(c_min_vox), wb_max.max(c_max_vox)));
                    } else {
                        woke_bounds = Some((c_min_vox, c_max_vox));
                    }
                    let blast_dir = (chunk.position - center + Vec3::new(0.0, 0.6, 0.0)).normalize_or_zero();
                    // Impulse falls off with distance and is shared by the chunk's mass using the same
                    // blast-exposure law as apply_radial_blast (dv ~ F / M^0.35). Previously every chunk,
                    // pebble or 5,000-voxel slab, received the same dv and a ±12 rad/s spin.
                    let falloff = (1.0 - dist / (radius + chunk_rad)).clamp(0.15, 1.0);
                    let dv = force * rand::gen_range(0.6, 1.3) * falloff / chunk.mass.powf(0.35).max(1.0);
                    chunk.velocity += blast_dir * dv;
                    let spin = (dv / chunk_rad.max(0.5)).min(12.0);
                    chunk.angular_velocity = Vec3::new(
                        rand::gen_range(-1.0, 1.0),
                        rand::gen_range(-1.0, 1.0),
                        rand::gen_range(-1.0, 1.0),
                    ) * spin;
                    chunk.life = 0.0;
                    woke_count += 1;
                }
            }
        }
        if let Some((wb_min, wb_max)) = woke_bounds {
            self.mark_rubble_region_dirty(wb_min, wb_max);
        } else if woke_count > 0 {
            self.rubble_dirty = true;
        }

        // Un-settle rubble heightmap and launch tumbling debris particles
        let r_cells = (radius / VOXEL_SIZE).ceil() as i32;
        let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let c_gx = ((center.x + half_x) / VOXEL_SIZE).floor() as i32;
        let c_gz = ((center.z + half_z) / VOXEL_SIZE).floor() as i32;

        let mut crater_excavated = false;
        for dx in -r_cells..=r_cells {
            for dz in -r_cells..=r_cells {
                let gx = c_gx + dx;
                let gz = c_gz + dz;
                if gx >= 0 && (gx as usize) < crate::voxel::GRID_X && gz >= 0 && (gz as usize) < crate::voxel::GRID_Z {
                    let d = ((dx * dx + dz * dz) as f32).sqrt() * VOXEL_SIZE;
                    if d < radius {
                        let idx = gx as usize * crate::voxel::GRID_Z + gz as usize;
                        let cur_h = self.rubble_height_grid[idx];
                        if cur_h > VOID_BOUNDARY_Y + 0.1 {
                            let excavate = ((radius - d) / radius * 0.6).min(cur_h - VOID_BOUNDARY_Y);
                            self.rubble_height_grid[idx] -= excavate;
                            crater_excavated = true;

                            // Launch airborne tumbling debris from crater rim
                            if self.debris.len() < Self::MAX_DEBRIS - 5 && rand::gen_range(0, 4) == 0 {
                                let world_x = gx as f32 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5;
                                let world_z = gz as f32 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5;
                                let launch_dir = Vec3::new(
                                    dx as f32 + rand::gen_range(-0.5, 0.5),
                                    rand::gen_range(1.2, 2.5),
                                    dz as f32 + rand::gen_range(-0.5, 0.5),
                                ).normalize_or_zero();
                                let launch_speed = rand::gen_range(8.0, 22.0);
                                self.debris.push(DebrisParticle {
                                    position: Vec3::new(world_x, cur_h, world_z),
                                    velocity: launch_dir * launch_speed,
                                    rotation: Vec3::ZERO,
                                    angular_velocity: Vec3::new(
                                        rand::gen_range(-15.0, 15.0),
                                        rand::gen_range(-15.0, 15.0),
                                        rand::gen_range(-15.0, 15.0),
                                    ),
                                    size: rand::gen_range(0.10, 0.25),
                                    color: Color::new(0.60, 0.58, 0.55, 1.0),
                                    life: rand::gen_range(4.0, 10.0),
                                    max_life: 10.0,
                                    bounces: 0,
                                });
                            }
                        }
                    }
                }
            }
        }
        if crater_excavated {
            let pad = Vec3::splat(radius + 2.0);
            let min_pos = Self::world_pos_to_voxel(center - pad);
            let max_pos = Self::world_pos_to_voxel(center + pad);
            self.mark_rubble_region_dirty(min_pos, max_pos);
        }
    }

    /// Dynamically calculates and stamps the true floor of remaining solid voxels in a crater
    pub fn update_crater_floor(&mut self, grid: &VoxelGrid, center: Vec3, radius: f32) {
        let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
        let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
        let r_cells = (radius / VOXEL_SIZE).ceil() as i32;
        let c_gx = ((center.x + half_x) / VOXEL_SIZE).floor() as i32;
        let c_gz = ((center.z + half_z) / VOXEL_SIZE).floor() as i32;

        let scan_top_vy = (((center.y + radius) / VOXEL_SIZE).ceil() as i32)
            .clamp(MIN_VOXEL_Y, MAX_VOXEL_Y - 1);

        let mut changed = false;
        for dx in -r_cells..=r_cells {
            for dz in -r_cells..=r_cells {
                let gx = c_gx + dx;
                let gz = c_gz + dz;
                if gx >= 0 && (gx as usize) < crate::voxel::GRID_X && gz >= 0 && (gz as usize) < crate::voxel::GRID_Z {
                    let d = ((dx * dx + dz * dz) as f32).sqrt() * VOXEL_SIZE;
                    if d <= radius {
                        let idx = (gx as usize) * crate::voxel::GRID_Z + (gz as usize);
                        // Find highest remaining solid voxel in this column at or below scan_top_vy
                        let mut solid_h = VOID_BOUNDARY_Y;
                        for vy in (MIN_VOXEL_Y..=scan_top_vy).rev() {
                            if grid.in_bounds(gx, vy, gz) && grid.get(gx, vy, gz).is_solid() {
                                let v_world = grid.voxel_to_world(IVec3::new(gx, vy, gz));
                                solid_h = v_world.y + VOXEL_SIZE * 0.5;
                                break;
                            }
                        }

                        // Also account for any remaining settled chunks that overlap this column
                        for chunk in &self.chunks {
                            if chunk.settled {
                                let wp_dist = ((chunk.position.x - (gx as f32 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5)).powi(2)
                                    + (chunk.position.z - (gz as f32 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5)).powi(2)).sqrt();
                                if wp_dist <= 3.0 {
                                    let rot_mat = chunk.rot_mat;
                                    for (local_v, _) in &chunk.voxels {
                                        let wp = chunk.position + rot_mat.transform_vector(*local_v);
                                        if let Some((cgx, cgz)) = Self::world_to_rubble_grid_coords(wp.x, wp.z) {
                                            if cgx == gx as usize && cgz == gz as usize {
                                                let chunk_top = wp.y + VOXEL_SIZE * 0.5;
                                                if chunk_top > solid_h {
                                                    solid_h = chunk_top;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        self.rubble_height_grid[idx] = solid_h.max(VOID_BOUNDARY_Y);
                        if solid_h > self.max_rubble_height {
                            self.max_rubble_height = solid_h;
                        }
                        changed = true;
                    }
                }
            }
        }
        if changed {
            let pad = Vec3::splat(radius + 2.0);
            let min_pos = grid.world_to_voxel(center - pad);
            let max_pos = grid.world_to_voxel(center + pad);
            self.mark_rubble_region_dirty(min_pos, max_pos);
        }
    }

    pub fn spawn_electrical_arc(&mut self, start: Vec3, end: Vec3, duration: f32, color: Color) {
        if self.electrical_arcs.len() < 24 {
            self.electrical_arcs.push(ElectricalArc {
                start,
                end,
                timer: duration,
                max_timer: duration,
                color,
            });
        }
    }

    pub fn spawn_fire_jet(&mut self, pos: Vec3, dir: Vec3, duration: f32) {
        if self.fire_jets.len() < 24 {
            self.fire_jets.push(PressurizedFireJet {
                position: pos,
                direction: dir.normalize_or_zero(),
                timer: duration,
                max_timer: duration,
            });
        }
    }

    pub fn spawn_column_micro_fracture(&mut self, pos: Vec3) {
        for _ in 0..5 {
            if self.debris.len() < Self::MAX_DEBRIS {
                let dir = Vec3::new(
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(-0.1f32, 0.8),
                    rand::gen_range(-1.0f32, 1.0),
                ).normalize_or_zero();
                self.debris.push(DebrisParticle {
                    position: pos + dir * 0.2,
                    velocity: dir * rand::gen_range(2.0, 6.0),
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::splat(rand::gen_range(-6.0, 6.0)),
                    size: rand::gen_range(0.04, 0.08),
                    color: Color::new(0.85, 0.82, 0.78, 1.0), // Light plaster flake
                    life: rand::gen_range(1.5, 3.0),
                    max_life: 3.0,
                    bounces: 0,
                });
            }
        }
        if self.smoke.len() < Self::MAX_SMOKE {
            self.smoke.push(SmokeParticle {
                position: pos,
                velocity: Vec3::new(0.0, 0.4, 0.0),
                size: 0.12,
                start_size: 0.12,
                max_size: 0.35,
                color: Color::new(0.78, 0.75, 0.72, 0.18),
                life: 1.0,
                max_life: 1.0,
            });
        }
    }

    /// Collects world-space surface voxel centers of `chunk` that lie inside `[omin, omax]`.
    fn gather_overlap_points(chunk: &DynamicChunk, omin: Vec3, omax: Vec3, out: &mut Vec<Vec3>, cap: usize) {
        out.clear();
        let src: &[(Vec3, VoxelType)] = if chunk.exposed_voxels.is_empty() { &chunk.voxels } else { &chunk.exposed_voxels };
        for (lv, _) in src {
            let wp = chunk.position + chunk.rot_mat.transform_vector(*lv);
            if wp.cmpge(omin).all() && wp.cmple(omax).all() {
                out.push(wp);
                if out.len() >= cap {
                    break;
                }
            }
        }
    }

    /// Resolves one candidate pair. Voxels are treated as spheres of one voxel diameter; the
    /// deepest overlapping voxel pairs become contacts (normal from `a` toward `b`).
    #[allow(clippy::too_many_arguments)]
    fn collide_chunk_pair(
        chunks: &mut [DynamicChunk],
        ia: usize,
        ib: usize,
        omin: Vec3,
        omax: Vec3,
        pts_a: &mut Vec<Vec3>,
        pts_b: &mut Vec<Vec3>,
    ) {
        const SAMPLE_CAP: usize = 160;
        const PAIR_CONTACTS: usize = 4;
        Self::gather_overlap_points(&chunks[ia], omin, omax, pts_a, SAMPLE_CAP);
        if pts_a.is_empty() {
            return;
        }
        Self::gather_overlap_points(&chunks[ib], omin, omax, pts_b, SAMPLE_CAP);
        if pts_b.is_empty() {
            return;
        }

        let diam = VOXEL_SIZE;
        let diam2 = diam * diam;
        let center_dir = (chunks[ib].position - chunks[ia].position).normalize_or_zero();
        let fallback_n = if center_dir == Vec3::ZERO { Vec3::Y } else { center_dir };
        let mut contacts = [(0.0f32, Vec3::ZERO, Vec3::ZERO); PAIR_CONTACTS];
        let mut n_contacts = 0usize;
        for &pa in pts_a.iter() {
            for &pb in pts_b.iter() {
                let d = pb - pa;
                let d2 = d.length_squared();
                if d2 >= diam2 {
                    continue;
                }
                let dist = d2.sqrt();
                let n = if dist > 1e-4 { d / dist } else { fallback_n };
                let entry = (diam - dist, (pa + pb) * 0.5, n);
                if n_contacts < PAIR_CONTACTS {
                    contacts[n_contacts] = entry;
                    n_contacts += 1;
                } else {
                    let mut shallowest = 0;
                    for k in 1..PAIR_CONTACTS {
                        if contacts[k].0 < contacts[shallowest].0 {
                            shallowest = k;
                        }
                    }
                    if entry.0 > contacts[shallowest].0 {
                        contacts[shallowest] = entry;
                    }
                }
            }
        }
        if n_contacts == 0 {
            return;
        }

        let (a, b) = if ia < ib {
            let (lo, hi) = chunks.split_at_mut(ib);
            (&mut lo[ia], &mut hi[0])
        } else {
            let (lo, hi) = chunks.split_at_mut(ia);
            (&mut hi[0], &mut lo[ib])
        };
        let inertia_a = a.body_inertia();
        let inertia_b = b.body_inertia();
        let inv_ma = 1.0 / a.mass.max(0.1);
        let inv_mb = 1.0 / b.mass.max(0.1);
        let inv_sum = inv_ma + inv_mb;

        // Positional split along the deepest contact, weighted by inverse mass (heavy chunks move less).
        let mut deepest = contacts[0];
        for c in &contacts[1..n_contacts] {
            if c.0 > deepest.0 {
                deepest = *c;
            }
        }
        let corr = deepest.2 * (deepest.0 * 0.6).min(VOXEL_SIZE * 0.25);
        a.position -= corr * (inv_ma / inv_sum);
        b.position += corr * (inv_mb / inv_sum);

        let mut acc_n = [0.0f32; PAIR_CONTACTS];
        let mut acc_t = [Vec3::ZERO; PAIR_CONTACTS];
        let mut target = [0.0f32; PAIR_CONTACTS];
        for k in 0..n_contacts {
            let (_, p, n) = contacts[k];
            let vrel = b.point_velocity(p - b.position) - a.point_velocity(p - a.position);
            let vn = vrel.dot(n);
            target[k] = if vn < -1.0 { -CHUNK_CHUNK_RESTITUTION * vn } else { 0.0 };
        }
        for _ in 0..CONTACT_ITERATIONS {
            for k in 0..n_contacts {
                let (_, p, n) = contacts[k];
                let ra = p - a.position;
                let rb = p - b.position;
                let vrel = b.point_velocity(rb) - a.point_velocity(ra);
                let vn = vrel.dot(n);
                let kn = a.inv_effective_mass(inertia_a, ra, n) + b.inv_effective_mass(inertia_b, rb, n);
                if kn <= 1e-9 {
                    continue;
                }
                let new_acc = (acc_n[k] + (target[k] - vn) / kn).max(0.0);
                let dj = new_acc - acc_n[k];
                acc_n[k] = new_acc;
                if dj != 0.0 {
                    a.apply_impulse(inertia_a, ra, -n * dj);
                    b.apply_impulse(inertia_b, rb, n * dj);
                }

                let vrel = b.point_velocity(rb) - a.point_velocity(ra);
                let vt = vrel - n * vrel.dot(n);
                let vt_len = vt.length();
                if vt_len > 1e-6 {
                    let t = vt / vt_len;
                    let kt = a.inv_effective_mass(inertia_a, ra, t) + b.inv_effective_mass(inertia_b, rb, t);
                    if kt > 1e-9 {
                        let old = acc_t[k];
                        let mut new_t = old - t * (vt_len / kt);
                        let max_f = CHUNK_CHUNK_FRICTION * acc_n[k];
                        let len = new_t.length();
                        if len > max_f {
                            new_t *= max_f / len.max(1e-9);
                        }
                        acc_t[k] = new_t;
                        let dj_t = new_t - old;
                        a.apply_impulse(inertia_a, ra, -dj_t);
                        b.apply_impulse(inertia_b, rb, dj_t);
                    }
                }
            }
        }
        a.clamp_spin();
        b.clamp_spin();
    }

    /// Chunk-vs-chunk collision between moving chunks (settled rubble is already part of the
    /// rubble heightmap, so moving chunks land on it through the ground contact solver).
    /// Broadphase: sort-and-sweep on world AABBs. Narrowphase: see [`Self::collide_chunk_pair`].
    /// Cost is bounded by [`MAX_CHUNK_PAIRS_PER_FRAME`] and per-chunk sample caps, and all
    /// buffers are persistent scratch (no steady-state allocation).
    pub fn resolve_chunk_collisions(&mut self) {
        let mut aabbs = std::mem::take(&mut self.scratch_pair_aabbs);
        let mut pts_a = std::mem::take(&mut self.scratch_overlap_a);
        let mut pts_b = std::mem::take(&mut self.scratch_overlap_b);
        aabbs.clear();
        let half = Vec3::splat(VOXEL_SIZE * 0.5);
        for (idx, c) in self.chunks.iter().enumerate() {
            if !c.settled && !c.voxels.is_empty() {
                let (mn, mx) = c.world_aabb_from_mat(&c.rot_mat);
                aabbs.push((idx, mn - half, mx + half));
            }
        }
        if aabbs.len() >= 2 {
            aabbs.sort_unstable_by(|a, b| a.1.x.total_cmp(&b.1.x));
            let mut pairs = 0usize;
            'sweep: for ai in 0..aabbs.len() {
                let (ia, amin, amax) = aabbs[ai];
                for &(ib, bmin, bmax) in &aabbs[ai + 1..] {
                    if bmin.x > amax.x {
                        break;
                    }
                    if bmin.y > amax.y || bmax.y < amin.y || bmin.z > amax.z || bmax.z < amin.z {
                        continue;
                    }
                    if pairs >= MAX_CHUNK_PAIRS_PER_FRAME {
                        break 'sweep;
                    }
                    pairs += 1;
                    Self::collide_chunk_pair(&mut self.chunks, ia, ib, amin.max(bmin), amax.min(bmax), &mut pts_a, &mut pts_b);
                }
            }
        }
        self.scratch_pair_aabbs = aabbs;
        self.scratch_overlap_a = pts_a;
        self.scratch_overlap_b = pts_b;
    }

    pub fn update(&mut self, dt: f32, grid: &mut VoxelGrid, audio: &SoundSystem, camera_pos: Vec3) -> f32 {
        self.elapsed_time += dt;
        let gravity = Vec3::new(0.0, -22.0, 0.0);
        let mut camera_trauma_impulse = 0.0f32;

        // Drain spall events generated from weapon impacts on hardened voxels
        for &(pos, col) in &grid.spall_events {
            self.spawn_spall_impact(pos, Vec3::Y, col);
        }
        grid.spall_events.clear();

        // Support check pre-pass for settled chunks: if supporting floor or rubble collapsed, wake up!
        let mut any_woke_up = false;
        if grid.structural_dirty {
            if let (Some(d_min), Some(d_max)) = (grid.dirty_min, grid.dirty_max) {
                if d_min.y <= 1 {
                    self.rubble_dirty = true;
                    self.mark_rubble_region_dirty(d_min, d_max);
                }
            }
        }
        if grid.structural_dirty || self.rubble_dirty {
            for i in 0..self.chunks.len() {
                if self.chunks[i].settled {
                    let lowest_y = self.chunks[i].lowest_world_y();
                    if lowest_y > VOID_BOUNDARY_Y + 0.15 {
                        let mut has_support = false;
                        let chunk = &self.chunks[i];
                        let rot_mat = chunk.rot_mat;
                        // Only the contact layer (voxels whose bottom is near the chunk's lowest point) can
                        // rest on anything. Sampling arbitrary voxels often picked top-layer voxels with
                        // nothing beneath them and woke perfectly supported rubble.
                        let sample_src: &[(Vec3, VoxelType)] = if chunk.exposed_voxels.is_empty() {
                            &chunk.voxels
                        } else {
                            &chunk.exposed_voxels
                        };
                        let contact_ceiling = lowest_y + VOXEL_SIZE * 0.6;
                        let mut contact_samples = 0usize;
                        'sample_loop: for &(local_v, _) in sample_src {
                            let wp = chunk.position + rot_mat.transform_vector(local_v);
                            let bottom_y = wp.y - VOXEL_SIZE * 0.5;
                            if bottom_y > contact_ceiling {
                                continue;
                            }
                            contact_samples += 1;
                            if contact_samples > 32 {
                                break;
                            }
                            let check_p = wp - Vec3::new(0.0, VOXEL_SIZE * 0.75, 0.0);
                            let check_v = grid.world_to_voxel(check_p);
                            if grid.in_bounds(check_v.x, check_v.y, check_v.z) && grid.get(check_v.x, check_v.y, check_v.z).is_solid() {
                                has_support = true;
                                break 'sample_loop;
                            }

                            // Fast O(1) multi-tap support check against rubble heightmap
                            let r_center = self.rubble_height_at(wp.x, wp.z);
                            let r_max = r_center
                                .max(self.rubble_height_at(wp.x + 0.20, wp.z))
                                .max(self.rubble_height_at(wp.x - 0.20, wp.z))
                                .max(self.rubble_height_at(wp.x, wp.z + 0.20))
                                .max(self.rubble_height_at(wp.x, wp.z - 0.20));
                            if r_max >= bottom_y - 0.28 && r_max <= bottom_y + 0.18 {
                                has_support = true;
                                break 'sample_loop;
                            }

                            // Fallback check if resting on another settled chunk below (pruned to close chunks)
                            for (k, other) in self.chunks.iter().enumerate() {
                                if k != i && other.settled {
                                    let dx = (wp.x - other.position.x).abs();
                                    let dz = (wp.z - other.position.z).abs();
                                    if dx > 4.0 || dz > 4.0 {
                                        continue;
                                    }
                                    if let Some(top_y) = other.highest_world_y_at(Vec2::new(wp.x, wp.z)) {
                                        if top_y >= bottom_y - 0.25 && top_y <= bottom_y + 0.15 {
                                            has_support = true;
                                            break 'sample_loop;
                                        }
                                    }
                                }
                            }
                        }
                        if !has_support {
                            // Floor or rubble support collapsed! Immediately wake up chunk with downward gravity velocity
                            let chunk = &mut self.chunks[i];
                            let chunk_r = ((chunk.local_aabb.1 - chunk.local_aabb.0) * 0.5).length().max(0.5);
                            let wake_spin = (0.5 / chunk_r).min(1.0) * 1.2;
                            chunk.settled = false;
                            chunk.velocity = Vec3::new(
                                rand::gen_range(-0.4, 0.4),
                                -1.8,
                                rand::gen_range(-0.4, 0.4),
                            );
                            chunk.angular_velocity = Vec3::new(
                                rand::gen_range(-1.0, 1.0),
                                rand::gen_range(-1.0, 1.0),
                                rand::gen_range(-1.0, 1.0),
                            ) * wake_spin;
                            any_woke_up = true;
                        }
                    }
                }
            }
        }
        if any_woke_up {
            self.rubble_dirty = true;
        }

        // Sync or rebuild rubble heightmap if dirty
        if self.rubble_dirty {
            if let Some((min_pos, max_pos)) = self.rubble_dirty_region.take() {
                self.rebuild_rubble_heightmap_in_region(grid, min_pos, max_pos);
            } else {
                self.rebuild_rubble_heightmap_with_grid(grid);
            }
            self.rubble_dirty = false;
        }

        // 1. Update Dynamic Chunks with Voxel Crushing and 3D Rotation
        let mut chunks_to_fracture = std::mem::take(&mut self.scratch_chunks_to_fracture);
        let mut all_fuel_detonations = std::mem::take(&mut self.scratch_fuel_detonations);
        let mut all_core_breaches = std::mem::take(&mut self.scratch_core_breaches);
        let mut all_ice_shatters = std::mem::take(&mut self.scratch_ice_shatters);
        let mut all_crush_dust = std::mem::take(&mut self.scratch_crush_dust);
        chunks_to_fracture.clear();
        all_fuel_detonations.clear();
        all_core_breaches.clear();
        all_ice_shatters.clear();
        all_crush_dust.clear();

        let mut craters_to_unsettle = std::mem::take(&mut self.scratch_craters_to_unsettle);
        let mut chunks_to_settle = std::mem::take(&mut self.scratch_chunks_to_settle);
        let mut new_crush_chunks = std::mem::take(&mut self.scratch_new_crush_chunks);
        craters_to_unsettle.clear();
        chunks_to_settle.clear();
        new_crush_chunks.clear();
        let total_chunks = self.chunks.len();
        let mut i = 0;
        while i < self.chunks.len() {
            let chunk = &mut self.chunks[i];
            chunk.life += dt;

            // Bottom Void Boundary despawn check:
            // ANY dynamic chunk whose position is at or below VOID_BOUNDARY_Y must despawn,
            // regardless of whether chunk.settled is true or false.
            if chunk.position.y <= VOID_BOUNDARY_Y {
                self.chunks.swap_remove(i);
                continue;
            }

            if !chunk.settled {
                let mut static_support_bounds = None;

                let step_dist = chunk.velocity.length() * dt;
                let num_substeps = if step_dist > VOXEL_SIZE * 0.8 {
                    ((step_dist / (VOXEL_SIZE * 0.5)).ceil() as usize).clamp(2, 5)
                } else {
                    1
                };
                let sub_dt = dt / num_substeps as f32;
                let mut despawned = false;
                for _ in 0..num_substeps {
                    chunk.velocity += gravity * sub_dt;
                    chunk.position += chunk.velocity * sub_dt;
                    chunk.integrate_rotation(sub_dt);

                    // Air drag (scaled by mass - heavier chunks maintain momentum)
                    let drag = (0.20 / chunk.mass.powf(0.25)).min(0.4);
                    chunk.velocity *= (1.0 - drag * sub_dt).max(0.0);
                    chunk.angular_velocity *= (1.0 - 0.35 * sub_dt).max(0.0);

                    if chunk.position.y <= VOID_BOUNDARY_Y {
                        despawned = true;
                        break;
                    }
                }
                if despawned {
                    self.chunks.swap_remove(i);
                    continue;
                }

                let speed = chunk.velocity.length();

                // Feature 7: Multi-Tier Pancake Collapse & Pulverization
                // Falling chunks crush static building floors based on physical material resistance!
                let grid_max_y = (grid.max_solid_y as f32 + 1.0) * VOXEL_SIZE;
                let half_x = (crate::voxel::GRID_X as f32 * VOXEL_SIZE) * 0.5;
                let half_z = (crate::voxel::GRID_Z as f32 * VOXEL_SIZE) * 0.5;
                let rot_mat = chunk.rot_mat;
                let (chunk_world_min, chunk_world_max) = chunk.world_aabb_from_mat(&rot_mat);

                let in_grid_volume = (grid.solid_count() > 0 || chunk_world_min.y <= 0.0)
                    && (chunk_world_min.y - VOXEL_SIZE * 1.5) <= grid_max_y
                    && chunk_world_max.y >= VOID_BOUNDARY_Y - VOXEL_SIZE
                    && chunk_world_max.x >= -half_x - VOXEL_SIZE
                    && chunk_world_min.x <= half_x + VOXEL_SIZE
                    && chunk_world_max.z >= -half_z - VOXEL_SIZE
                    && chunk_world_min.z <= half_z + VOXEL_SIZE;

                let (min_aabb, max_aabb) = chunk.local_aabb;
                let extent_x = max_aabb.x - min_aabb.x;
                let extent_z = max_aabb.z - min_aabb.z;
                let half_extent = (max_aabb - min_aabb) * 0.5;
                let effective_speed = chunk.velocity.length() + chunk.angular_velocity.length() * half_extent.length();

                if effective_speed > 0.05 && in_grid_volume {
                    let downward_speed = (-chunk.velocity.y).max(0.0);
                    let is_compact = extent_x <= 3.0 * VOXEL_SIZE && extent_z <= 3.0 * VOXEL_SIZE;
                    let is_penetrator = is_compact
                        && ((chunk.mass / chunk.voxels.len().max(1) as f32) >= 7.0
                            || chunk.voxels.iter().any(|(_, vt)| *vt == VoxelType::HeavyArmor));
                    let world_height = (chunk_world_max.y - chunk_world_min.y).max(0.1);
                    let sweep_dist = (effective_speed * dt).max(VOXEL_SIZE * 1.5);
                    let test_min = Vec3::new(
                        chunk_world_min.x - sweep_dist,
                        (chunk_world_min.y - sweep_dist).max(VOID_BOUNDARY_Y),
                        chunk_world_min.z - sweep_dist,
                    );
                    let test_max = Vec3::new(
                        chunk_world_max.x + sweep_dist,
                        (chunk_world_max.y.min(grid_max_y) + sweep_dist).min(grid_max_y + VOXEL_SIZE),
                        chunk_world_max.z + sweep_dist,
                    );
                    let v_min = grid.world_to_voxel(test_min);
                    let v_max = grid.world_to_voxel(test_max);

                    if grid.region_has_solids(v_min.x, v_max.x, v_min.y, v_max.y, v_min.z, v_max.z) {
                        let mut crushed_ice = 0;
                        let mut static_contacts = ContactManifold::default();

                        self.scratch_crushed_voxels.clear();

                        // Kinetic energy budget for this frame's crushing (translational + rotational).
                        // Every crushed voxel consumes energy; a chunk can never pulverize more
                        // material than its own motion can pay for, so collapses dissipate
                        // instead of feeding on themselves.
                        let approx_inertia = 0.4 * chunk.mass * half_extent.length_squared();
                        let initial_crush_energy = 0.5 * chunk.mass * chunk.velocity.length_squared()
                            + 0.5 * approx_inertia * chunk.angular_velocity.length_squared();
                        let mut crush_energy = initial_crush_energy;

                        // Hierarchical Sub-Block AABB narrowphase (Zero-allocation scratch buffer reuse):
                        // Early-out sub-blocks whose world AABB does not intersect solid geometry!
                        chunk.populate_sub_blocks(&mut self.scratch_sub_blocks);
                        for (sub_local_aabb, voxel_indices) in &self.scratch_sub_blocks {
                            if voxel_indices.is_empty() {
                                continue;
                            }
                            let (sub_world_min, sub_world_max) = DynamicChunk::transform_sub_aabb(chunk.position, *sub_local_aabb, &rot_mat);
                            if sub_world_min.y - sweep_dist > grid_max_y {
                                continue; // Entire sub-block is above highest solid voxel in the grid
                            }
                            let sub_test_min = Vec3::new(
                                sub_world_min.x - sweep_dist,
                                (sub_world_min.y - sweep_dist).max(VOID_BOUNDARY_Y),
                                sub_world_min.z - sweep_dist,
                            );
                            let sub_test_max = Vec3::new(
                                sub_world_max.x + sweep_dist,
                                (sub_world_max.y.min(grid_max_y) + sweep_dist).min(grid_max_y + VOXEL_SIZE),
                                sub_world_max.z + sweep_dist,
                            );
                            let sv_min = grid.world_to_voxel(sub_test_min);
                            let sv_max = grid.world_to_voxel(sub_test_max);

                            if !grid.region_has_solids(sv_min.x, sv_max.x, sv_min.y, sv_max.y, sv_min.z, sv_max.z) {
                                continue; // Sub-block clear: early out all constituent voxels!
                            }

                            let sub_sample_count = voxel_indices.len().min(64);
                            let step = (voxel_indices.len() / sub_sample_count).max(1);

                            for j in (0..voxel_indices.len()).step_by(step) {
                                let v_idx = voxel_indices[j];
                                let (local_v, _) = chunk.voxels[v_idx];
                                let wpos = chunk.position + rot_mat.transform_vector(local_v);
                                if wpos.y - sweep_dist > grid_max_y {
                                    continue;
                                }

                                let r = rot_mat.transform_vector(local_v);
                                let v_point = chunk.velocity + chunk.angular_velocity.cross(r);
                                // Each contact uses its OWN point velocity. (Using the fastest point on the
                                // chunk made a toppling tower's pivot crush as if moving at tip speed.)
                                let point_speed = v_point.length();
                                // Distribute kinetic energy properly: apply mass proportional to the chunk's 1D scale to avoid infinite vaporization
                                let effective_point_mass = (chunk.mass / chunk.voxels.len().max(1) as f32) * (chunk.voxels.len() as f32).powf(0.333).min(15.0);
                                let point_momentum = effective_point_mass * point_speed;
                                let sweep_offset = v_point * dt;

                                let candidate_points = [
                                    wpos,
                                    wpos - sweep_offset * 0.5,
                                    wpos - sweep_offset,
                                    wpos - Vec3::new(0.0, (downward_speed * dt).max(VOXEL_SIZE * 0.75), 0.0),
                                ];

                                for &test_p in &candidate_points {
                                    let vpos = grid.world_to_voxel(test_p);
                                    if grid.in_bounds(vpos.x, vpos.y, vpos.z) && vpos.y > MIN_VOXEL_Y {
                                        let vt = grid.get(vpos.x, vpos.y, vpos.z);
                                        if vt.is_solid() && vt.is_destructible() {
                                            // Already pulverized by another sample this frame: no double charge
                                            if self.scratch_crushed_voxels.iter().any(|(p, _)| *p == vpos) {
                                                break;
                                            }
                                            let crush_thresh = crush_threshold(vt, is_penetrator);
                                            let crush_cost = crush_thresh * CRUSH_ENERGY_PER_THRESHOLD;

                                            if point_momentum >= crush_thresh && crush_energy >= crush_cost {
                                                crush_energy -= crush_cost;
                                                self.scratch_crushed_voxels.push((vpos, vt));
                                            } else if point_speed > RESISTED_IMPACT_MIN_SPEED
                                                && vt != VoxelType::Bedrock
                                                && vt != VoxelType::Basalt
                                                && grid.damage_voxel(vpos.x, vpos.y, vpos.z, point_momentum * 1.5, crate::voxel::DamageType::Kinetic)
                                            {
                                                // Genuine impact on a resistant barrier broke it: deliver kinetic damage
                                                // once per contact (resting contact never grinds voxels away).
                                                crush_energy = (crush_energy - crush_cost).max(0.0);
                                                self.scratch_crushed_voxels.push((vpos, vt));
                                            } else {
                                                // The barrier holds: it becomes a rigid contact.
                                                let c = static_voxel_contact(wpos, grid.voxel_to_world(vpos));
                                                if c.depth > -CONTACT_SLOP {
                                                    static_contacts.push(c);
                                                }
                                            }
                                            // The swept probe stops at the first solid voxel it meets
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                    let crushed_count = self.scratch_crushed_voxels.len();

                    // Process all pulverized static voxels
                    if crushed_count > 0 {
                        let mut crushed_mass_accrued = 0.0f32;
                        for &(vpos, vt) in &self.scratch_crushed_voxels {
                            let density = vt.properties().density;
                            crushed_mass_accrued += density * 0.35;
                            if vt == VoxelType::Explosive {
                                all_fuel_detonations.push(vpos);
                            } else if vt == VoxelType::GlowCore || vt == VoxelType::PowerCapacitor {
                                all_core_breaches.push(vpos);
                            } else if vt == VoxelType::CryoIce || vt == VoxelType::Glass {
                                crushed_ice += 1;
                            } else if vt == VoxelType::SteelRebar || vt == VoxelType::CompositeRebar {
                                grid.current_rebar_count = grid.current_rebar_count.saturating_sub(1);
                                grid.severed_rebar_positions.push(grid.voxel_to_world(vpos));
                            }
                            grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Air);
                        }
                        let mass_before_accretion = chunk.mass;
                        chunk.mass += crushed_mass_accrued;

                        // 1. Convert 20% to 30% of pulverized voxels into small dynamic rubble sub-chunks (2-4 voxels)
                        let target_sub_voxels = (crushed_count as f32 * 0.25).round() as usize;
                        let mut sub_voxels_created = 0;
                        let mut c_idx = 0;

                        while c_idx < crushed_count && sub_voxels_created < target_sub_voxels && (total_chunks + new_crush_chunks.len()) < 300 {
                            let cluster_size = rand::gen_range(2, 5).min(crushed_count - c_idx);
                            if cluster_size < 2 { break; }

                            let mut cluster_wps = [Vec3::ZERO; 4];
                            let mut cluster_vts = [VoxelType::Air; 4];
                            let mut center_sum = Vec3::ZERO;
                            let mut cluster_mass = 0.0f32;

                            for k in 0..cluster_size {
                                let (vp, vt) = self.scratch_crushed_voxels[c_idx + k];
                                let wp = grid.voxel_to_world(vp);
                                center_sum += wp;
                                cluster_mass += vt.properties().density;
                                cluster_wps[k] = wp;
                                cluster_vts[k] = vt;
                            }

                            let cluster_center = center_sum / cluster_size as f32;
                            let mut local_voxels = Vec::with_capacity(cluster_size);
                            for k in 0..cluster_size {
                                local_voxels.push((cluster_wps[k] - cluster_center, cluster_vts[k]));
                            }
                            let aabb = DynamicChunk::compute_aabb(&local_voxels);

                            let lateral_offset = Vec3::new(
                                cluster_center.x - chunk.position.x,
                                0.3,
                                cluster_center.z - chunk.position.z,
                            ).normalize_or_zero();

                            let scatter_dir = if lateral_offset.length_squared() < 0.05 {
                                Vec3::new(rand::gen_range(-1.0, 1.0), rand::gen_range(0.2, 0.7), rand::gen_range(-1.0, 1.0)).normalize_or_zero()
                            } else {
                                lateral_offset
                            };

                            // Chips inherit a fraction of the crusher's motion plus a small outward scatter.
                            // (Launching them at 6-16 m/s gave them enough energy to crush concrete
                            // themselves, which made crush cascades self-sustaining.)
                            let scatter_speed = rand::gen_range(1.5, 4.0);

                            new_crush_chunks.push(DynamicChunk::new(
                                cluster_center,
                                chunk.velocity * 0.35 + scatter_dir * scatter_speed,
                                Vec3::ZERO,
                                Vec3::new(
                                    rand::gen_range(-3.0, 3.0),
                                    rand::gen_range(-3.0, 3.0),
                                    rand::gen_range(-3.0, 3.0),
                                ),
                                local_voxels,
                                cluster_mass.max(1.0),
                                0.0,
                                false,
                                aabb,
                            ));

                            sub_voxels_created += cluster_size;
                            c_idx += cluster_size;
                        }

                        // 2. Emit flying spall clouds (high-speed concrete chips and incandescent ricochet sparks) along floor perimeters
                        let spall_points = (crushed_count.min(24) / 2).max(4);
                        for k in 0..spall_points {
                            let angle = (k as f32 / spall_points as f32) * std::f32::consts::TAU;
                            let (s_sin, s_cos) = angle.sin_cos();
                            let outward_dir = Vec3::new(s_cos, rand::gen_range(0.1, 0.45), s_sin).normalize_or_zero();
                            let spall_origin = chunk.position + Vec3::new(s_cos, 0.0, s_sin) * (world_height * 0.5 + 0.4);

                            // Incandescent ricochet sparks along perimeter
                            let spark_burst = rand::gen_range(3, 7);
                            for _ in 0..spark_burst {
                                if self.sparks.len() < Self::MAX_SPARKS {
                                    let spark_spread = Vec3::new(
                                        rand::gen_range(-0.4, 0.4),
                                        rand::gen_range(0.1, 0.5),
                                        rand::gen_range(-0.4, 0.4),
                                    );
                                    let spark_dir = (outward_dir + spark_spread).normalize_or_zero();
                                    let spark_speed = rand::gen_range(15.0, 32.0);
                                    self.sparks.push(SparkParticle {
                                        position: spall_origin + spark_dir * 0.1,
                                        velocity: spark_dir * spark_speed,
                                        color: Color::new(1.0, rand::gen_range(0.75, 0.95), 0.30, 1.0),
                                        life: rand::gen_range(0.25, 0.65),
                                        max_life: 0.65,
                                    });
                                }
                            }

                            // High-speed concrete chips / micro-spall flakes
                            let chip_burst = rand::gen_range(2, 5);
                            for _ in 0..chip_burst {
                                if self.debris.len() < Self::MAX_DEBRIS {
                                    let chip_spread = Vec3::new(
                                        rand::gen_range(-0.3, 0.3),
                                        rand::gen_range(0.15, 0.6),
                                        rand::gen_range(-0.3, 0.3),
                                    );
                                    let chip_dir = (outward_dir + chip_spread).normalize_or_zero();
                                    let chip_speed = rand::gen_range(14.0, 28.0);
                                    self.debris.push(DebrisParticle {
                                        position: spall_origin + chip_dir * 0.15,
                                        velocity: chip_dir * chip_speed,
                                        rotation: Vec3::ZERO,
                                        angular_velocity: Vec3::new(
                                            rand::gen_range(-18.0, 18.0),
                                            rand::gen_range(-18.0, 18.0),
                                            rand::gen_range(-18.0, 18.0),
                                        ),
                                        size: rand::gen_range(0.05, 0.12),
                                        color: Color::new(0.65, 0.63, 0.60, 1.0),
                                        life: rand::gen_range(3.0, 6.5),
                                        max_life: 6.5,
                                        bounces: 0,
                                    });
                                }
                            }
                        }

                        // 3. Pulverize intermediate crushed story voxels into 2-3 dynamic micro-chunks per voxel, outward-scattered
                        for &(vpos, vt) in &self.scratch_crushed_voxels {
                            let wp = grid.voxel_to_world(vpos);
                            let lateral_offset = Vec3::new(
                                wp.x - chunk.position.x,
                                rand::gen_range(0.15, 0.55),
                                wp.z - chunk.position.z,
                            ).normalize_or_zero();

                            let base_scatter_dir = if lateral_offset.length_squared() < 0.05 {
                                Vec3::new(rand::gen_range(-1.0, 1.0), rand::gen_range(0.2, 0.8), rand::gen_range(-1.0, 1.0)).normalize_or_zero()
                            } else {
                                lateral_offset
                            };

                            let micro_chunks_count = rand::gen_range(2, 4); // 2 to 3 dynamic micro-chunks per voxel
                            for _ in 0..micro_chunks_count {
                                if self.debris.len() >= Self::MAX_DEBRIS - 10 {
                                    break;
                                }
                                let jitter = Vec3::new(
                                    rand::gen_range(-0.35, 0.35),
                                    rand::gen_range(-0.15, 0.25),
                                    rand::gen_range(-0.35, 0.35),
                                );
                                let scatter_dir = (base_scatter_dir + jitter).normalize_or_zero();
                                let scatter_speed = rand::gen_range(12.0, 26.0);

                                self.debris.push(DebrisParticle {
                                    position: wp + scatter_dir * rand::gen_range(0.08, 0.30),
                                    velocity: scatter_dir * scatter_speed,
                                    rotation: Vec3::ZERO,
                                    angular_velocity: Vec3::new(
                                        rand::gen_range(-20.0, 20.0),
                                        rand::gen_range(-20.0, 20.0),
                                        rand::gen_range(-20.0, 20.0),
                                    ),
                                    size: VOXEL_SIZE * rand::gen_range(0.15, 0.45),
                                    color: vt.color(),
                                    life: rand::gen_range(6.0, 14.0),
                                    max_life: 14.0,
                                    bounces: 0,
                                });
                            }
                        }

                        // 4. Spawn localized radial dust clouds around perimeter of crush zone
                        let dust_count = (crushed_count.min(24) / 2).max(4);
                        for k in 0..dust_count {
                            let angle = (k as f32 / dust_count as f32) * std::f32::consts::TAU;
                            let (s_sin, s_cos) = angle.sin_cos();
                            let p_dir = Vec3::new(s_cos, rand::gen_range(0.05, 0.35), s_sin);
                            let p_pos = chunk.position + p_dir * rand::gen_range(0.5, 2.5);
                            let s_life = rand::gen_range(1.2, 2.8);
                            let start_size = rand::gen_range(0.12, 0.20);
                            let max_size = rand::gen_range(0.80, 1.50);

                            if self.smoke.len() < Self::MAX_SMOKE {
                                self.smoke.push(SmokeParticle {
                                    position: p_pos,
                                    velocity: p_dir * rand::gen_range(3.5, 7.5),
                                    size: start_size,
                                    start_size,
                                    max_size,
                                    color: Color::new(0.68, 0.65, 0.60, 0.24),
                                    life: s_life,
                                    max_life: s_life,
                                });
                            }
                        }

                        // Energy & momentum bookkeeping:
                        // 1) kinetic energy spent pulverizing voxels is removed from the chunk's motion,
                        // 2) accreted rubble mass conserves momentum (v' = v * M / (M + dm)).
                        let spent_fraction = if initial_crush_energy > 1e-4 {
                            ((initial_crush_energy - crush_energy) / initial_crush_energy)
                                .clamp(0.0, 1.0)
                        } else {
                            1.0
                        };
                        let speed_keep = (1.0 - spent_fraction).sqrt();
                        let accretion_keep = mass_before_accretion / chunk.mass.max(1e-4);
                        chunk.velocity *= speed_keep * accretion_keep;
                        chunk.angular_velocity *= speed_keep;
                        let dist_atten = SoundSystem::calculate_distance_attenuation(chunk.position, camera_pos);
                        camera_trauma_impulse += ((crushed_count as f32 * 0.03) * (speed / 10.0)).clamp(0.05, 0.45) * dist_atten;
                        grid.structural_dirty = true;
                        all_crush_dust.push((chunk.position, (crushed_count as f32).min(8.0)));
                    }

                    if crushed_count > 0 {
                        // Floor yielded to progressive pancake crushing!
                        // Downward momentum plows straight through without bouncing back up.
                        grid.structural_dirty = true;
                    }
                    if static_contacts.len > 0 {
                        if static_contacts.support_count > 0 {
                            static_support_bounds = Some((static_contacts.support_min, static_contacts.support_max));
                        }
                        // Static voxels the chunk could not afford to crush (armor, bedrock, or anything
                        // beyond its remaining kinetic energy) are rigid contacts: push-out, impulse solve
                        // with friction, and a support-polygon settle test. Chunks no longer sink through
                        // resistant floors, and large chunks are no longer force-fractured just for resting.
                        let impact_spd = chunk.contact_approach_speed(&static_contacts, dt);
                        if crushed_count == 0
                            && impact_spd > fracture_speed_threshold(&chunk.voxels)
                            && chunk.voxels.len() > 40
                            && total_chunks < 320
                        {
                            chunks_to_fracture.push((i, impact_spd));
                        } else {
                            let mut push_pos = Vec3::ZERO;
                            let mut push_neg = Vec3::ZERO;
                            for c in static_contacts.contacts() {
                                if c.depth > 0.0 {
                                    let p = c.normal * c.depth;
                                    push_pos = push_pos.max(p);
                                    push_neg = push_neg.min(p);
                                }
                            }
                            let mut correction = (push_pos + push_neg) * 0.8;
                            let max_corr = VOXEL_SIZE * 0.5;
                            if correction.length_squared() > max_corr * max_corr {
                                correction = correction.normalize_or_zero() * max_corr;
                            }
                            let restitution = 0.0f32;
                            chunk.solve_static_contacts(&static_contacts, restitution, CONTACT_FRICTION, dt);
                            chunk.position += correction;
                            chunk.apply_contact_damping(static_contacts.mean_normal(), dt);
                            if impact_spd > RESISTED_IMPACT_MIN_SPEED {
                                all_crush_dust.push((chunk.position, 6.0));
                            }
                            let at_rest = chunk.velocity.length_squared() < 0.64 && chunk.angular_velocity.length_squared() < 1.0;
                            // Real static contacts establish support at any elevation. Restricting
                            // sleep to the ground floor leaves debris on intact upper floors awake forever.
                            if crushed_count == 0 && at_rest && static_contacts.supports_com(chunk.position, VOXEL_SIZE * 0.5) {
                                chunks_to_settle.push(i);
                            }
                        }
                    }

                    if crushed_ice > 0 {
                        all_ice_shatters.push((chunk.position, (crushed_ice * 6).min(40)));
                    }
                }
            }

                // Feature 8: Collision with Bedrock & 3D Rubble Heightmap (multi-point contact manifold).
                // Surface voxels near the ground each contribute a contact; the deepest MAX_CONTACTS
                // drive a sequential-impulse solve (normal + Coulomb friction, with torque from
                // off-center contacts), and every upward contact feeds the support polygon that decides
                // whether a slow chunk is balanced (settles) or overhanging (keeps tipping under gravity).
                let mut ground = ContactManifold::default();
                let max_global_rubble = self.max_rubble_height.max(VOXEL_SIZE);
                let (ground_test_min, _) = chunk.world_aabb_from_mat(&chunk.rot_mat);
                if max_global_rubble > VOID_BOUNDARY_Y && ground_test_min.y - (VOXEL_SIZE * 0.5 + CONTACT_SLOP) <= max_global_rubble {
                    let rot_mat = chunk.rot_mat;
                    let src: &[(Vec3, VoxelType)] = if chunk.exposed_voxels.is_empty() {
                        &chunk.voxels
                    } else {
                        &chunk.exposed_voxels
                    };
                    let step = (src.len() / 512).max(1);
                    let gate = max_global_rubble + CONTACT_SLOP;
                    for j in (0..src.len()).step_by(step) {
                        let local_v = src[j].0;
                        let bottom_y = chunk.position.y + rot_mat.transform_y(local_v) - VOXEL_SIZE * 0.5;
                        if bottom_y > gate {
                            continue;
                        }
                        let wp = chunk.position + rot_mat.transform_vector(local_v);
                        // Centers alone miss a cube edge landing on a neighboring higher cell.
                        // Use one contact at the highest part of its footprint. Redundant coplanar
                        // corner contacts would crowd other support voxels out of the fixed manifold.
                        let half = VOXEL_SIZE * 0.5;
                        let mut top_height = VOID_BOUNDARY_Y;
                        let mut center_height = VOID_BOUNDARY_Y;
                        let mut point_sum = Vec2::ZERO;
                        let mut point_count = 0;
                        let mut support_min = Vec2::splat(f32::INFINITY);
                        let mut support_max = Vec2::splat(f32::NEG_INFINITY);
                        for offset in [Vec2::ZERO, Vec2::new(-half, -half), Vec2::new(half, -half), Vec2::new(-half, half), Vec2::new(half, half)] {
                            let x = wp.x + offset.x;
                            let z = wp.z + offset.y;
                            let ground_y = Self::rubble_height_from_slice(&self.rubble_height_grid, x, z);
                            if offset == Vec2::ZERO {
                                center_height = ground_y;
                            }
                            if ground_y <= VOID_BOUNDARY_Y {
                                continue;
                            }
                            let point = Vec2::new(x, z);
                            if ground_y > top_height + 1e-5 {
                                top_height = ground_y;
                                point_sum = point;
                                point_count = 1;
                                support_min = point;
                                support_max = point;
                            } else if (ground_y - top_height).abs() <= 1e-5 {
                                point_sum += point;
                                point_count += 1;
                                support_min = support_min.min(point);
                                support_max = support_max.max(point);
                            }
                        }
                        let depth = top_height - bottom_y;
                        if point_count > 0 && depth > -CONTACT_SLOP {
                            // Preserve a centered support impulse when the center itself touches
                            // the highest surface; only a higher neighboring step needs an offset.
                            let point = if (center_height - top_height).abs() <= 1e-5 {
                                Vec2::new(wp.x, wp.z)
                            } else {
                                point_sum / point_count as f32
                            };
                            ground.push(ContactPoint {
                                point: Vec3::new(point.x, bottom_y, point.y),
                                // Raster cells store horizontal tops, rather than the artificial
                                // slope obtained by differentiating across neighboring vertical steps.
                                normal: Vec3::Y,
                                depth,
                            });
                            ground.support_min = ground.support_min.min(support_min);
                            ground.support_max = ground.support_max.max(support_max);
                        }
                    }
                }

                if ground.has_penetration() {
                    let deepest = ground.deepest().unwrap_or_default();
                    let contact_normal = ground.mean_normal();
                    let impact_speed = chunk.contact_approach_speed(&ground, dt);
                    let lift = deepest.depth;

                    if impact_speed > fracture_speed_threshold(&chunk.voxels) && chunk.voxels.len() > 40 && total_chunks < 320 {
                        // High-speed catastrophic impact! Subdivide / fracture into rubble sub-chunks and re-wake crater
                        chunk.position.y += lift;
                        chunks_to_fracture.push((i, impact_speed));
                        let c_radius = (chunk.mass.powf(0.33) * 1.5).clamp(1.5, 8.0);
                        craters_to_unsettle.push((deepest.point, c_radius, impact_speed * 1.2));
                    } else {
                        // Masonry on masonry is essentially inelastic: heavy chunks and ground contacts
                        // never bounce; light chunks on steep rubble get a faint rebound.
                        let restitution = if chunk.mass >= 250.0 || contact_normal.y >= 0.5 {
                            0.0f32
                        } else {
                            0.04f32 / (1.0 + chunk.mass / 250.0)
                        };
                        chunk.solve_static_contacts(&ground, restitution, CONTACT_FRICTION, dt);
                        chunk.position.y += lift;
                        chunk.apply_contact_damping(contact_normal, dt);

                        if impact_speed > 2.0 {
                            audio.play_spatial(audio.ram_impact.as_ref(), chunk.position, camera_pos, 0.45);
                            audio.play_spatial(audio.debris_clatter.as_ref(), chunk.position, camera_pos, 0.35);
                            let dist_atten = SoundSystem::calculate_distance_attenuation(chunk.position, camera_pos);
                            camera_trauma_impulse += ((chunk.mass / 80.0) * (impact_speed / 8.0)).clamp(0.0, 0.40) * dist_atten;

                            // Dust shockwave ring on heavy impact
                            let dust_count = (chunk.mass.sqrt() * 0.8).clamp(1.0, 3.0) as usize;
                            for _ in 0..dust_count {
                                let s_dir = Vec3::new(
                                    rand::gen_range(-1.0f32, 1.0),
                                    rand::gen_range(0.05f32, 0.35),
                                    rand::gen_range(-1.0f32, 1.0),
                                ).normalize_or_zero();
                                let s_life = rand::gen_range(0.35, 0.60);
                                let start_size = 0.07;
                                let max_size = 0.24;
                                self.smoke.push(SmokeParticle {
                                    position: chunk.position + s_dir * rand::gen_range(0.2, 0.8),
                                    velocity: s_dir * rand::gen_range(1.5, 3.5) + Vec3::new(0.0, 0.4, 0.0),
                                    size: start_size,
                                    start_size,
                                    max_size,
                                    color: Color::new(0.55, 0.53, 0.50, 0.12),
                                    life: s_life,
                                    max_life: s_life,
                                });
                            }
                        }

                        // The penetrating contact manifold establishes actual support under the
                        // chunk's feet. Its center column can be lower when it straddles a rubble pile.
                        let at_rest = chunk.velocity.length_squared() < 0.64 && chunk.angular_velocity.length_squared() < 1.0;
                        let supported = if let Some((static_min, static_max)) = static_support_bounds {
                            // Static support points are voxel centers; ground support already samples
                            // their footprint corners, so do not expand those corners a second time.
                            let margin = Vec2::splat(VOXEL_SIZE * 0.5);
                            let min = ground.support_min.min(static_min - margin);
                            let max = ground.support_max.max(static_max + margin);
                            chunk.position.x >= min.x && chunk.position.x <= max.x
                                && chunk.position.z >= min.y && chunk.position.z <= max.y
                        } else {
                            ground.supports_com(chunk.position, 0.0)
                        };
                        if at_rest
                            && contact_normal.y >= 0.85
                            && chunk.position.y > VOID_BOUNDARY_Y
                            && supported
                        {
                            chunks_to_settle.push(i);
                        }
                    }
                }

                if chunk.position.y <= VOID_BOUNDARY_Y {
                    self.chunks.swap_remove(i);
                    continue;
                }
            }

            i += 1;
        }

        // Feature: Chunk-vs-chunk collisions between active tumbling bodies
        self.resolve_chunk_collisions();

        // Settle marked chunks into rubble heightmap
        if !chunks_to_settle.is_empty() {
            for &idx in &chunks_to_settle {
                let Some(chunk) = self.chunks.get(idx) else { continue; };
                if chunk.settled || chunk.velocity.length_squared() >= 0.64
                    || chunk.angular_velocity.length_squared() >= 1.0 {
                    continue;
                }
                // Pair corrections can move a previously supported body away from
                // its feet. Awake candidates are absent from the rubble cache.
                let half = VOXEL_SIZE * 0.5;
                let supported = chunk.voxels.iter().any(|(local, _)| {
                    let center = chunk.local_to_world(*local);
                    let bottom = center.y - half;
                    [Vec2::ZERO, Vec2::new(-half, -half), Vec2::new(half, -half), Vec2::new(-half, half), Vec2::new(half, half)]
                        .iter().any(|offset| {
                            let x = center.x + offset.x;
                            let z = center.z + offset.y;
                            let top = self.rubble_height_at(x, z);
                            if top >= bottom - CONTACT_SLOP && top <= center.y + half {
                                return true;
                            }
                            let cell = grid.world_to_voxel(Vec3::new(x, bottom, z));
                            [cell.y - 1, cell.y].iter().any(|&y| {
                                let support = grid.voxel_to_world(IVec3::new(cell.x, y, cell.z));
                                grid.get(cell.x, y, cell.z).is_solid()
                                    && support.y <= center.y
                                    && support.y + half >= bottom - CONTACT_SLOP
                            })
                        })
                });
                if supported {
                    self.settle_chunk(idx);
                }
            }
            // Incremental rubble footprint updates: settle_chunk updates only the (x, z) footprint
            // of the settled chunk directly, eliminating whole-grid dirtying & rebuilding.
        }
        // Add new dynamic sub-chunks from pulverization
        if !new_crush_chunks.is_empty() {
            grid.structural_dirty = true;
            self.chunks.append(&mut new_crush_chunks);
        }

        // Process secondary events from chunk crushing pass
        if !all_fuel_detonations.is_empty() || !all_core_breaches.is_empty() {
            let chain_trauma = crate::weapon::process_explosive_chain(
                grid,
                self,
                audio,
                all_fuel_detonations.clone(),
                all_core_breaches.clone(),
                camera_pos,
            );
            camera_trauma_impulse += chain_trauma;
        }

        for &(ice_pos, count) in &all_ice_shatters {
            self.spawn_ice_shatter_effects(ice_pos, 3.0, count);
            audio.play_spatial(audio.cryo_shatter.as_ref(), ice_pos, camera_pos, 0.55);
        }

        for &(pos, count) in &all_crush_dust {
            audio.play_spatial(audio.ram_impact.as_ref(), pos, camera_pos, 0.35);
            let p_count = ((count / 4.0) as usize).clamp(1, 2);
            for _ in 0..p_count {
                let s_life = rand::gen_range(0.30, 0.50);
                let start_size = 0.05;
                let max_size = 0.18;
                self.smoke.push(SmokeParticle {
                    position: pos + Vec3::new(
                        rand::gen_range(-0.3, 0.3),
                        rand::gen_range(-0.1, 0.1),
                        rand::gen_range(-0.3, 0.3),
                    ),
                    velocity: Vec3::new(
                        rand::gen_range(-1.2, 1.2),
                        rand::gen_range(0.4, 1.2),
                        rand::gen_range(-1.2, 1.2),
                    ),
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.65, 0.63, 0.60, 0.12),
                    life: s_life,
                    max_life: s_life,
                });
            }
        }

        // Process fractured chunks (Subdivision into dynamic rubble sub-chunks, NEVER disappearing!)
        chunks_to_fracture.sort_unstable_by_key(|a| std::cmp::Reverse(a.0));
        chunks_to_fracture.dedup_by(|a, b| a.0 == b.0);

        let mut all_new_sub_chunks = std::mem::take(&mut self.scratch_fracture_sub_chunks);
        all_new_sub_chunks.clear();
        for &(idx, impact_speed) in &chunks_to_fracture {
            if idx >= self.chunks.len() {
                continue;
            }
            let chunk = self.chunks.swap_remove(idx);
            let chunk_pos = chunk.position;
            let chunk_mass = chunk.mass;
            let mut chip_colors = [Color::default(); 16];
            let mut num_chip_colors = 0;
            for (_, vt) in chunk.voxels.iter().take(16) {
                chip_colors[num_chip_colors] = vt.color();
                num_chip_colors += 1;
            }
            let chip_colors = &chip_colors[..num_chip_colors];

            audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), chunk_pos, camera_pos, 0.60);
            audio.play_spatial(audio.debris_clatter.as_ref(), chunk_pos, camera_pos, 0.45);

            let dist_atten = SoundSystem::calculate_distance_attenuation(chunk_pos, camera_pos);
            let impact_trauma = ((chunk_mass / 60.0) * (impact_speed / 8.0)).clamp(0.0, 0.45) * dist_atten;
            camera_trauma_impulse += impact_trauma;

            let shatter_blast_r = (chunk_mass.powf(0.33) * 2.5).clamp(2.5, 8.0);
            // A fracture is not an explosion: only dust & debris particles are pushed. (Applying a
            // full radial blast here hurled neighboring chunks and woke stacked rubble, which
            // then fell, fractured and blasted again — a self-feeding chain reaction.)
            self.apply_radial_blast_particles(chunk_pos, shatter_blast_r, impact_speed * 1.8);

            let chunk_vel = chunk.velocity;
            // Subdivide/fracture chunk into 2 to 4 smaller dynamic rubble sub-chunks
            let (mut sub_chunks, exp_world, core_world) = chunk.fracture();

            let mut exp_voxels = Vec::new();
            for wp in exp_world {
                exp_voxels.push(grid.world_to_voxel(wp));
            }
            let mut core_voxels = Vec::new();
            for wp in core_world {
                core_voxels.push(grid.world_to_voxel(wp));
            }

            if !exp_voxels.is_empty() || !core_voxels.is_empty() {
                let chain_trauma = crate::weapon::process_explosive_chain(
                    grid,
                    self,
                    audio,
                    exp_voxels,
                    core_voxels,
                    camera_pos,
                );
                camera_trauma_impulse += chain_trauma;
            }

            // Spawn subtle billowing dust wisps
            let dust_count = (chunk_mass.sqrt() * 0.6).clamp(1.0, 3.0) as usize;
            for _ in 0..dust_count {
                let s_dir = Vec3::new(
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(0.08f32, 0.65),
                    rand::gen_range(-1.0f32, 1.0),
                ).normalize();
                let s_life = rand::gen_range(0.35, 0.60);
                let start_size = rand::gen_range(0.06, 0.12);
                let max_size = rand::gen_range(0.18, 0.35);

                self.smoke.push(SmokeParticle {
                    position: chunk_pos + s_dir * rand::gen_range(0.2, 0.8),
                    velocity: s_dir * rand::gen_range(1.5, 3.2) + Vec3::new(0.0, 0.5, 0.0),
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.50, 0.48, 0.45, 0.12),
                    life: s_life,
                    max_life: s_life,
                });
            }

            // Spawn clattering debris chips
            for &col in chip_colors {
                let burst_dir = Vec3::new(
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(0.3f32, 1.8),
                    rand::gen_range(-1.0f32, 1.0),
                ).normalize();

                self.debris.push(DebrisParticle {
                    position: chunk_pos + burst_dir * rand::gen_range(0.2, 1.5),
                    velocity: burst_dir * rand::gen_range(6.0, 18.0),
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::new(
                        rand::gen_range(-10.0, 10.0),
                        rand::gen_range(-10.0, 10.0),
                        rand::gen_range(-10.0, 10.0),
                    ),
                    size: VOXEL_SIZE * rand::gen_range(0.3, 0.7),
                    color: col,
                    life: rand::gen_range(8.0, 14.0),
                    max_life: 14.0,
                    bounces: 0,
                });
            }

            // Retain ALL sub-chunks in the world as dynamic rubble (airborne sub-chunks continue falling)
            for sub in &mut sub_chunks {
                if sub.settled && sub.lowest_world_y() > 1.5 {
                    sub.settled = false;
                    sub.velocity = chunk_vel * 0.5;
                }
            }
            all_new_sub_chunks.extend(sub_chunks);
        }
        if !chunks_to_fracture.is_empty() || !all_new_sub_chunks.is_empty() {
            grid.structural_dirty = true;
        }
        self.chunks.append(&mut all_new_sub_chunks);
        self.scratch_chunks_to_fracture = chunks_to_fracture;
        self.scratch_fuel_detonations = all_fuel_detonations;
        self.scratch_core_breaches = all_core_breaches;
        self.scratch_ice_shatters = all_ice_shatters;
        self.scratch_crush_dust = all_crush_dust;

        for &(c_pos, c_rad, c_force) in &craters_to_unsettle {
            self.un_settle_rubble_crater(c_pos, c_rad, c_force);
        }
        craters_to_unsettle.clear();
        chunks_to_settle.clear();
        self.scratch_craters_to_unsettle = craters_to_unsettle;
        self.scratch_chunks_to_settle = chunks_to_settle;
        self.scratch_new_crush_chunks = new_crush_chunks;
        self.scratch_fracture_sub_chunks = all_new_sub_chunks;

        // 2. Update Debris Particles
        let mut d_i = 0;
        while d_i < self.debris.len() {
            let p = &mut self.debris[d_i];
            p.life -= dt;
            if p.life <= 0.0 || p.position.y <= VOID_BOUNDARY_Y {
                self.debris.swap_remove(d_i);
                continue;
            }

            // Resting debris sleep: if velocity is zero and rubble beneath is intact, skip physics checks
            if p.velocity.length_squared() < 0.0001 && p.angular_velocity.length_squared() < 0.0001 {
                if !self.rubble_dirty {
                    d_i += 1;
                    continue;
                }
                let rubble_top = if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(p.position.x, p.position.z) {
                    let h = self.rubble_height_grid[gx * crate::voxel::GRID_Z + gz];
                    if h <= VOID_BOUNDARY_Y {
                        f32::NEG_INFINITY
                    } else {
                        h
                    }
                } else {
                    f32::NEG_INFINITY
                };
                if rubble_top.is_finite() {
                    let min_y = rubble_top + p.size * 0.5;
                    if (p.position.y - min_y).abs() <= 0.06 {
                        d_i += 1;
                        continue;
                    }
                }
            }

            let old_pos = p.position;
            p.velocity += gravity * dt;
            let candidate_pos = old_pos + p.velocity * dt;
            if candidate_pos.y <= VOID_BOUNDARY_Y {
                self.debris.swap_remove(d_i);
                continue;
            }
            p.rotation += p.angular_velocity * dt;

            let mut collided = false;
            let mut col_normal = Vec3::Y;
            let mut contact_pos = candidate_pos;

            // 1. Check dynamic rubble / subterranean heightmap collision
            let rubble_top = if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(candidate_pos.x, candidate_pos.z) {
                let h = self.rubble_height_grid[gx * crate::voxel::GRID_Z + gz];
                if h <= VOID_BOUNDARY_Y {
                    f32::NEG_INFINITY
                } else {
                    h
                }
            } else {
                f32::NEG_INFINITY
            };
            let min_y = rubble_top + p.size * 0.5;
            if rubble_top.is_finite() && candidate_pos.y <= min_y {
                collided = true;
                let norm = Self::rubble_normal_from_slice(&self.rubble_height_grid, candidate_pos.x, candidate_pos.z);
                col_normal = if norm != Vec3::ZERO { norm } else { Vec3::Y };
                contact_pos = Vec3::new(candidate_pos.x, min_y, candidate_pos.z);
            } else {
                // 2. Check static voxel wall boundaries
                let v_new = grid.world_to_voxel(candidate_pos);
                if grid.in_bounds(v_new.x, v_new.y, v_new.z) && grid.get(v_new.x, v_new.y, v_new.z).is_solid() {
                    collided = true;
                    let v_old = grid.world_to_voxel(old_pos);
                    let step = v_new - v_old;
                    if step.x != 0 && step.y == 0 && step.z == 0 {
                        col_normal = Vec3::new(-step.x as f32, 0.0, 0.0);
                    } else if step.y != 0 && step.x == 0 && step.z == 0 {
                        col_normal = Vec3::new(0.0, -step.y as f32, 0.0);
                    } else if step.z != 0 && step.x == 0 && step.y == 0 {
                        col_normal = Vec3::new(0.0, 0.0, -step.z as f32);
                    } else {
                        let center = grid.voxel_to_world(v_new);
                        let d = old_pos - center;
                        if d.x.abs() >= d.y.abs() && d.x.abs() >= d.z.abs() {
                            col_normal = Vec3::new(d.x.signum(), 0.0, 0.0);
                        } else if d.y.abs() >= d.x.abs() && d.y.abs() >= d.z.abs() {
                            col_normal = Vec3::new(0.0, d.y.signum(), 0.0);
                        } else {
                            col_normal = Vec3::new(0.0, 0.0, d.z.signum());
                        }
                    }
                    contact_pos = old_pos + col_normal * 0.02;
                }
            }

            if collided {
                p.position = contact_pos;
                let vn = p.velocity.dot(col_normal);
                if vn < 0.0 {
                    if p.velocity.length_squared() > 0.36 && p.bounces < 3 {
                        let vt = p.velocity - col_normal * vn;
                        p.velocity = vt * 0.65 - col_normal * (vn * 0.42);
                        p.angular_velocity = p.angular_velocity * 0.65 + col_normal.cross(p.velocity) * 0.85;
                        p.bounces += 1;
                    } else {
                        p.velocity = Vec3::ZERO;
                        p.angular_velocity = Vec3::ZERO;
                    }
                }
            } else {
                p.position = candidate_pos;
            }

            if p.position.y <= VOID_BOUNDARY_Y {
                self.debris.swap_remove(d_i);
                continue;
            }

            d_i += 1;
        }

        // 3. Update Smoke Particles
        let mut s_i = 0;
        let cur_time = self.elapsed_time;
        let base_wind = self.wind;
        let gust = 1.0 + 0.42 * (cur_time * 0.80).sin() + 0.22 * (cur_time * 2.30).cos();
        let base_wind_gust_x = base_wind.x * gust;
        let base_wind_gust_z = base_wind.z * gust;
        let t_eddy_x = cur_time * 0.65;
        let t_eddy_z = cur_time * 0.55;
        let t_thermal = cur_time * 0.40;

        while s_i < self.smoke.len() {
            let s = &mut self.smoke[s_i];
            s.life -= dt;
            if s.life <= 0.0 || s.position.y <= VOID_BOUNDARY_Y {
                self.smoke.swap_remove(s_i);
                continue;
            }

            let eddy_x = (s.position.z * 0.08 + s.position.y * 0.05 + t_eddy_x).sin() * 0.28;
            let eddy_z = (s.position.x * 0.08 + s.position.y * 0.05 + t_eddy_z).cos() * 0.28;
            let thermal = (0.22 + 0.14 * (s.position.x * 0.06 + s.position.z * 0.06 + t_thermal).sin())
                * (-s.position.y * 0.025).exp().clamp(0.2, 1.2);
            let wind = Vec3::new(
                base_wind_gust_x + eddy_x,
                base_wind.y + thermal,
                base_wind_gust_z + eddy_z,
            );
            s.position += (s.velocity + wind) * dt;
            if s.position.y <= VOID_BOUNDARY_Y {
                self.smoke.swap_remove(s_i);
                continue;
            }
            let progress = (1.0 - (s.life / s.max_life)).clamp(0.0, 1.0);
            if progress < 0.20 {
                s.velocity *= (1.0 - 2.8 * dt).max(0.0);
            } else if progress < 0.65 {
                s.velocity *= (1.0 - 0.9 * dt).max(0.0);
                s.velocity.y += 0.35 * dt;
            } else {
                s.velocity *= (1.0 - 1.5 * dt).max(0.0);
            }
            let expansion_curve = if progress < 0.20 {
                (progress / 0.20) * 0.45
            } else {
                0.45 + ((progress - 0.20) / 0.80).powf(0.85) * 0.55
            };
            s.size = s.start_size + (s.max_size - s.start_size) * expansion_curve;

            s_i += 1;
        }

        // 4. Update Sparks
        let mut sp_i = 0;
        while sp_i < self.sparks.len() {
            let sp = &mut self.sparks[sp_i];
            sp.life -= dt;
            if sp.life <= 0.0 || sp.position.y <= VOID_BOUNDARY_Y {
                self.sparks.swap_remove(sp_i);
                continue;
            }

            sp.velocity *= (1.0 - 0.45 * dt).max(0.0);
            sp.velocity += (gravity * 0.7 + self.wind * 0.2) * dt;
            let candidate_sp_pos = sp.position + sp.velocity * dt;
            if candidate_sp_pos.y <= VOID_BOUNDARY_Y {
                self.sparks.swap_remove(sp_i);
                continue;
            }

            let mut sp_collided = false;
            let mut sp_normal = Vec3::Y;
            let mut sp_contact = candidate_sp_pos;

            // 1. Rubble / subterranean check
            let r_top = if let Some((gx, gz)) = Self::world_to_rubble_grid_coords(candidate_sp_pos.x, candidate_sp_pos.z) {
                let h = self.rubble_height_grid[gx * crate::voxel::GRID_Z + gz];
                if h <= VOID_BOUNDARY_Y {
                    f32::NEG_INFINITY
                } else {
                    h
                }
            } else {
                f32::NEG_INFINITY
            };
            if r_top.is_finite() && candidate_sp_pos.y <= r_top + 0.02 {
                sp_collided = true;
                let norm = Self::rubble_normal_from_slice(&self.rubble_height_grid, candidate_sp_pos.x, candidate_sp_pos.z);
                sp_normal = if norm != Vec3::ZERO { norm } else { Vec3::Y };
                sp_contact = Vec3::new(candidate_sp_pos.x, r_top + 0.02, candidate_sp_pos.z);
            } else {
                // 2. Static voxel wall check
                let sv_new = grid.world_to_voxel(candidate_sp_pos);
                if grid.in_bounds(sv_new.x, sv_new.y, sv_new.z) && grid.get(sv_new.x, sv_new.y, sv_new.z).is_solid() {
                    sp_collided = true;
                    let sv_old = grid.world_to_voxel(sp.position);
                    let s_step = sv_new - sv_old;
                    if s_step.x != 0 && s_step.y == 0 && s_step.z == 0 {
                        sp_normal = Vec3::new(-s_step.x as f32, 0.0, 0.0);
                    } else if s_step.y != 0 && s_step.x == 0 && s_step.z == 0 {
                        sp_normal = Vec3::new(0.0, -s_step.y as f32, 0.0);
                    } else if s_step.z != 0 && s_step.x == 0 && s_step.y == 0 {
                        sp_normal = Vec3::new(0.0, 0.0, -s_step.z as f32);
                    } else {
                        let s_center = grid.voxel_to_world(sv_new);
                        let s_d = sp.position - s_center;
                        if s_d.x.abs() >= s_d.y.abs() && s_d.x.abs() >= s_d.z.abs() {
                            sp_normal = Vec3::new(s_d.x.signum(), 0.0, 0.0);
                        } else if s_d.y.abs() >= s_d.x.abs() && s_d.y.abs() >= s_d.z.abs() {
                            sp_normal = Vec3::new(0.0, s_d.y.signum(), 0.0);
                        } else {
                            sp_normal = Vec3::new(0.0, 0.0, s_d.z.signum());
                        }
                    }
                    sp_contact = sp.position + sp_normal * 0.02;
                }
            }

            if sp_collided {
                sp.position = sp_contact;
                let vn = sp.velocity.dot(sp_normal);
                if vn < 0.0 {
                    let vt = sp.velocity - sp_normal * vn;
                    sp.velocity = vt * 0.78 - sp_normal * (vn * 0.62);
                    sp.life *= 0.88; // Spark loses thermal energy on ricochet
                }
            } else {
                sp.position = candidate_sp_pos;
            }

            if sp.position.y <= VOID_BOUNDARY_Y {
                self.sparks.swap_remove(sp_i);
                continue;
            }

            sp_i += 1;
        }

        // 5. Update Shockwave Rings
        let mut sw_i = 0;
        while sw_i < self.shockwaves.len() {
            let sw = &mut self.shockwaves[sw_i];
            sw.radius += sw.speed * dt;
            if sw.radius >= sw.max_radius {
                self.shockwaves.swap_remove(sw_i);
                continue;
            }
            sw_i += 1;
        }

        // 6. Update Impact Decals & Active Molten Fissure Heat Shimmer
        let mut d_i = 0;
        while d_i < self.decals.len() {
            let d = &mut self.decals[d_i];
            d.life -= dt;
            if d.life <= 0.0 {
                self.decals.swap_remove(d_i);
                continue;
            }

            // Check if underlying surface voxel has collapsed, been destroyed, or fallen away
            let is_intact = if let Some(anchor) = d.voxel_anchor {
                grid.in_bounds(anchor.x, anchor.y, anchor.z)
                    && grid.get(anchor.x, anchor.y, anchor.z).is_solid()
            } else {
                grid.find_nearest_solid(d.position, d.size.max(VOXEL_SIZE * 1.5)).is_some()
            };

            if !is_intact {
                // Surface gave way or detached: crumble decal into tumbling micro-spall chips
                if self.debris.len() < Self::MAX_DEBRIS {
                    self.debris.push(DebrisParticle {
                        position: d.position,
                        velocity: -d.normal * 1.2
                            + Vec3::new(
                                rand::gen_range(-1.2, 1.2),
                                rand::gen_range(0.3, 1.6),
                                rand::gen_range(-1.2, 1.2),
                            ),
                        rotation: Vec3::ZERO,
                        angular_velocity: Vec3::new(
                            rand::gen_range(-12.0, 12.0),
                            rand::gen_range(-12.0, 12.0),
                            rand::gen_range(-12.0, 12.0),
                        ),
                        size: (d.size * 0.12).clamp(0.04, 0.12),
                        color: d.color,
                        life: 1.8,
                        max_life: 1.8,
                        bounces: 0,
                    });
                }
                self.decals.swap_remove(d_i);
                continue;
            }

            // Thermal fissures vent incandescent micro-embers and hot gas wisps
            if (d.kind == DecalKind::PlasmaBurn || d.kind == DecalKind::Scorch)
                && d.life > 0.25 * d.max_life
                && rand::gen_range(0.0, 1.0) < 0.035
                && self.sparks.len() < Self::MAX_SPARKS - 30
            {
                let offset = Vec3::new(
                    rand::gen_range(-0.15, 0.15),
                    rand::gen_range(-0.05, 0.05),
                    rand::gen_range(-0.15, 0.15),
                );
                let col = if d.kind == DecalKind::PlasmaBurn {
                    Color::new(0.4, 0.9, 1.0, 0.9)
                } else {
                    Color::new(1.0, 0.65, 0.18, 0.9)
                };
                self.sparks.push(SparkParticle {
                    position: d.position + offset,
                    velocity: d.normal * 0.7 + Vec3::new(0.0, rand::gen_range(1.5, 3.2), 0.0),
                    color: col,
                    life: rand::gen_range(0.25, 0.65),
                    max_life: 0.65,
                });
            }
            d_i += 1;
        }

        // 7. Update Electrical Arcs
        let mut arc_i = 0;
        while arc_i < self.electrical_arcs.len() {
            let arc = &mut self.electrical_arcs[arc_i];
            arc.timer -= dt;
            if arc.timer <= 0.0 {
                self.electrical_arcs.swap_remove(arc_i);
                continue;
            }
            if rand::gen_range(0, 3) == 0 && self.sparks.len() < Self::MAX_SPARKS {
                let lerp_t = rand::gen_range(0.0f32, 1.0);
                let p = arc.start.lerp(arc.end, lerp_t);
                let spark_dir = Vec3::new(
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(-1.0f32, 1.0),
                ).normalize_or_zero();
                self.sparks.push(SparkParticle {
                    position: p,
                    velocity: spark_dir * rand::gen_range(6.0, 16.0),
                    color: Color::new(0.4, 0.7, 1.0, 1.0),
                    life: rand::gen_range(0.15, 0.45),
                    max_life: 0.45,
                });
            }
            arc_i += 1;
        }

        // 8. Update Pressurized Fire Jets
        let mut jet_i = 0;
        while jet_i < self.fire_jets.len() {
            let jet = &mut self.fire_jets[jet_i];
            jet.timer -= dt;
            if jet.timer <= 0.0 {
                self.fire_jets.swap_remove(jet_i);
                continue;
            }
            if self.sparks.len() < Self::MAX_SPARKS {
                let jitter = Vec3::new(
                    rand::gen_range(-0.3f32, 0.3),
                    rand::gen_range(-0.3f32, 0.3),
                    rand::gen_range(-0.3f32, 0.3),
                );
                let dir = (jet.direction + jitter).normalize_or_zero();
                self.sparks.push(SparkParticle {
                    position: jet.position,
                    velocity: dir * rand::gen_range(12.0, 24.0),
                    color: Color::new(1.0, rand::gen_range(0.3, 0.6), 0.1, 1.0),
                    life: rand::gen_range(0.2, 0.5),
                    max_life: 0.5,
                });
            }
            if self.smoke.len() < Self::MAX_SMOKE && rand::gen_range(0, 2) == 0 {
                self.smoke.push(SmokeParticle {
                    position: jet.position,
                    velocity: jet.direction * rand::gen_range(3.0, 8.0) + Vec3::new(0.0, 1.0, 0.0),
                    size: 0.15,
                    start_size: 0.15,
                    max_size: 0.6,
                    color: Color::new(0.35, 0.32, 0.30, 0.22),
                    life: rand::gen_range(0.6, 1.4),
                    max_life: 1.4,
                });
            }
            jet_i += 1;
        }

        self.enforce_particle_caps();
        camera_trauma_impulse.clamp(0.0, 0.45)
    }

    pub fn enforce_particle_caps(&mut self) {
        // Mathematically continuous accelerated decay replaces abrupt drain(0..excess)
        // Soft caps scale remaining lifetime to <= 0.12s * (1 - rank/excess)
        // Hard max caps scale remaining lifetime aggressively to 0.01s..0.04s without calling drain()

        // 1. Debris Particle Pool
        if self.debris.len() > Self::SOFT_DEBRIS {
            let excess = self.debris.len() - Self::SOFT_DEBRIS;
            for (rank, p) in self.debris.iter_mut().take(excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
                let target_fade = (0.12 * rank_factor).max(0.02);
                if p.life > target_fade {
                    let cur_ratio = if p.max_life > 0.001 {
                        (p.life / p.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    p.life = target_fade;
                    p.max_life = target_fade / cur_ratio;
                }
            }
        }
        if self.debris.len() > Self::MAX_DEBRIS {
            let hard_excess = self.debris.len() - Self::MAX_DEBRIS;
            for (rank, p) in self.debris.iter_mut().take(hard_excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / hard_excess as f32)).max(0.05);
                let target_fade = (0.04 * rank_factor).max(0.01);
                if p.life > target_fade {
                    let cur_ratio = if p.max_life > 0.001 {
                        (p.life / p.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    p.life = target_fade;
                    p.max_life = target_fade / cur_ratio;
                }
            }
        }

        // 2. Sparks Particle Pool
        if self.sparks.len() > Self::SOFT_SPARKS {
            let excess = self.sparks.len() - Self::SOFT_SPARKS;
            for (rank, sp) in self.sparks.iter_mut().take(excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
                let target_fade = (0.12 * rank_factor).max(0.02);
                if sp.life > target_fade {
                    let cur_ratio = if sp.max_life > 0.001 {
                        (sp.life / sp.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    sp.life = target_fade;
                    sp.max_life = target_fade / cur_ratio;
                }
            }
        }
        if self.sparks.len() > Self::MAX_SPARKS {
            let hard_excess = self.sparks.len() - Self::MAX_SPARKS;
            for (rank, sp) in self.sparks.iter_mut().take(hard_excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / hard_excess as f32)).max(0.05);
                let target_fade = (0.04 * rank_factor).max(0.01);
                if sp.life > target_fade {
                    let cur_ratio = if sp.max_life > 0.001 {
                        (sp.life / sp.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    sp.life = target_fade;
                    sp.max_life = target_fade / cur_ratio;
                }
            }
        }

        // 3. Smoke Particle Pool
        if self.smoke.len() > Self::SOFT_SMOKE {
            let excess = self.smoke.len() - Self::SOFT_SMOKE;
            for (rank, sm) in self.smoke.iter_mut().take(excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
                let target_fade = (0.12 * rank_factor).max(0.02);
                if sm.life > target_fade {
                    let cur_ratio = if sm.max_life > 0.001 {
                        (sm.life / sm.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    sm.life = target_fade;
                    sm.max_life = target_fade / cur_ratio;
                }
            }
        }
        if self.smoke.len() > Self::MAX_SMOKE {
            let hard_excess = self.smoke.len() - Self::MAX_SMOKE;
            for (rank, sm) in self.smoke.iter_mut().take(hard_excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / hard_excess as f32)).max(0.05);
                let target_fade = (0.04 * rank_factor).max(0.01);
                if sm.life > target_fade {
                    let cur_ratio = if sm.max_life > 0.001 {
                        (sm.life / sm.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    sm.life = target_fade;
                    sm.max_life = target_fade / cur_ratio;
                }
            }
        }

        // 4. Shockwaves Pool
        if self.shockwaves.len() > Self::SOFT_SHOCKWAVES {
            let excess = self.shockwaves.len() - Self::SOFT_SHOCKWAVES;
            for (rank, sw) in self.shockwaves.iter_mut().take(excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
                let rem = (sw.max_radius - sw.radius).max(0.05);
                let target_time = (0.12 * rank_factor).max(0.02);
                sw.speed = sw.speed.max(rem / target_time);
            }
        }
        if self.shockwaves.len() > Self::MAX_SHOCKWAVES {
            let hard_excess = self.shockwaves.len() - Self::MAX_SHOCKWAVES;
            for (rank, sw) in self.shockwaves.iter_mut().take(hard_excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / hard_excess as f32)).max(0.05);
                let rem = (sw.max_radius - sw.radius).max(0.02);
                let target_time = (0.04 * rank_factor).max(0.01);
                sw.speed = sw.speed.max(rem / target_time);
            }
        }

        // 5. Impact Decals Pool
        if self.decals.len() > Self::SOFT_DECALS {
            let excess = self.decals.len() - Self::SOFT_DECALS;
            for (rank, d) in self.decals.iter_mut().take(excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / excess as f32)).max(0.05);
                let target_fade = (0.12 * rank_factor).max(0.02);
                if d.life > target_fade {
                    let cur_ratio = if d.max_life > 0.001 {
                        (d.life / d.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    d.life = target_fade;
                    d.max_life = target_fade / cur_ratio;
                }
            }
        }
        if self.decals.len() > Self::MAX_DECALS {
            let hard_excess = self.decals.len() - Self::MAX_DECALS;
            for (rank, d) in self.decals.iter_mut().take(hard_excess).enumerate() {
                let rank_factor = (1.0 - (rank as f32 / hard_excess as f32)).max(0.05);
                let target_fade = (0.04 * rank_factor).max(0.01);
                if d.life > target_fade {
                    let cur_ratio = if d.max_life > 0.001 {
                        (d.life / d.max_life).clamp(0.01, 1.0)
                    } else {
                        1.0
                    };
                    d.life = target_fade;
                    d.max_life = target_fade / cur_ratio;
                }
            }
        }
    }

    pub fn calculate_dynamic_wind(t: f32, position: Vec3) -> Vec3 {
        let base_wind = Vec3::new(0.55, 0.12, 0.35);
        let gust = 1.0 + 0.42 * (t * 0.80).sin() + 0.22 * (t * 2.30).cos();

        let eddy_x = (position.z * 0.08 + position.y * 0.05 + t * 0.65).sin() * 0.28;
        let eddy_z = (position.x * 0.08 + position.y * 0.05 + t * 0.55).cos() * 0.28;

        let thermal = (0.22 + 0.14 * (position.x * 0.06 + position.z * 0.06 + t * 0.40).sin())
            * (-position.y * 0.025).exp().clamp(0.2, 1.2);

        Vec3::new(
            base_wind.x * gust + eddy_x,
            base_wind.y + thermal,
            base_wind.z * gust + eddy_z,
        )
    }

    #[allow(dead_code)]
    pub fn get_dynamic_wind(&self, position: Vec3) -> Vec3 {
        Self::calculate_dynamic_wind(self.elapsed_time, position)
    }

    pub fn spawn_structural_collapse_dust_storm(
        &mut self,
        center: Vec3,
        approx_radius: f32,
        voxel_count: usize,
    ) {
        let radius = approx_radius.clamp(1.5, 16.0);
        let half_w = radius;
        let half_d = radius;
        let dust_count = (voxel_count / 18).clamp(18, 64);

        // 1. Perimeter Shear Plane Radial Dust Roll
        let ring_count = dust_count / 2;
        for k in 0..ring_count {
            let angle = (k as f32 / ring_count as f32) * std::f32::consts::TAU;
            let (s_sin, s_cos) = angle.sin_cos();
            let radial_dir = Vec3::new(s_cos, rand::gen_range(0.05, 0.35), s_sin).normalize_or_zero();
            let spawn_pos = center + radial_dir * rand::gen_range(radius * 0.3, radius * 0.95);
            let roll_speed = rand::gen_range(4.0, 9.0);
            let s_life = rand::gen_range(2.5, 4.8);
            let start_size = rand::gen_range(0.30, 0.60);
            let max_size = rand::gen_range(2.2, 4.2);

            if self.smoke.len() < Self::MAX_SMOKE {
                self.smoke.push(SmokeParticle {
                    position: spawn_pos,
                    velocity: radial_dir * roll_speed,
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.68, 0.65, 0.60, 0.24),
                    life: s_life,
                    max_life: s_life,
                });
            }
        }

        // 2. Central Ascending Thermal Plume
        let core_count = dust_count - ring_count;
        for _ in 0..core_count {
            let offset = Vec3::new(
                rand::gen_range(-half_w * 0.5, half_w * 0.5),
                rand::gen_range(-0.5, 1.2),
                rand::gen_range(-half_d * 0.5, half_d * 0.5),
            );
            let lift_speed = rand::gen_range(1.8, 4.2);
            let s_life = rand::gen_range(3.2, 5.5);
            let start_size = rand::gen_range(0.40, 0.75);
            let max_size = rand::gen_range(2.8, 5.2);

            if self.smoke.len() < Self::MAX_SMOKE {
                self.smoke.push(SmokeParticle {
                    position: center + offset,
                    velocity: Vec3::new(
                        rand::gen_range(-1.2, 1.2),
                        lift_speed,
                        rand::gen_range(-1.2, 1.2),
                    ),
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.56, 0.53, 0.49, 0.22),
                    life: s_life,
                    max_life: s_life,
                });
            }
        }

        // 3. Tumbling Micro-Spall Fracture Debris Curtain
        let spall_count = (dust_count / 2).clamp(12, 32);
        for _ in 0..spall_count {
            let eject_dir = Vec3::new(
                rand::gen_range(-1.0, 1.0),
                rand::gen_range(0.2, 1.2),
                rand::gen_range(-1.0, 1.0),
            ).normalize_or_zero();

            if self.debris.len() < Self::MAX_DEBRIS {
                self.debris.push(DebrisParticle {
                    position: center + eject_dir * rand::gen_range(0.2, radius * 0.6),
                    velocity: eject_dir * rand::gen_range(6.0, 18.0),
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::splat(rand::gen_range(-12.0, 12.0)),
                    size: VOXEL_SIZE * rand::gen_range(0.25, 0.65),
                    color: Color::new(0.62, 0.60, 0.58, 1.0),
                    life: rand::gen_range(4.0, 8.0),
                    max_life: 8.0,
                    bounces: 0,
                });
            }
        }
    }

    /// Spawns gargantuan billowing dust plumes that linger and rise high above collapsing structures
    pub fn add_colossal_siege_dust_plume(
        &mut self,
        center: Vec3,
        radius: f32,
        color: Color,
    ) {
        let smoke_count = (radius * 3.2).clamp(18.0, 48.0) as usize;
        for _ in 0..smoke_count {
            let s_dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(0.2f32, 1.4),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(2.0, 7.5);
            let s_life = rand::gen_range(0.8, 1.8);
            let start_size = rand::gen_range(0.2, 0.45);
            let max_size = rand::gen_range(0.9, 2.2);

            self.smoke.push(SmokeParticle {
                position: center + s_dir * rand::gen_range(0.2, radius * 0.4),
                velocity: s_dir * speed + Vec3::new(0.0, 1.2, 0.0),
                size: start_size,
                start_size,
                max_size,
                color,
                life: s_life,
                max_life: s_life,
            });
        }
    }

    /// Spawns multi-ring ground-cratering shockwaves and ground impact scorch decals
    pub fn add_ground_cratering_shockwave(&mut self, center: Vec3, max_radius: f32) {
        let ground_y = Self::rubble_height_from_slice(&self.rubble_height_grid, center.x, center.z);
        let speeds = [24.0, 36.0, 48.0];
        let radii = [max_radius * 0.6, max_radius * 0.85, max_radius];
        for (i, (&spd, &rad)) in speeds.iter().zip(radii.iter()).enumerate() {
            let c_alpha = (0.9 - i as f32 * 0.2).clamp(0.4, 0.95);
            self.shockwaves.push(ShockwaveRing {
                position: Vec3::new(center.x, ground_y + 0.05, center.z),
                normal: Vec3::Y,
                radius: 0.8,
                max_radius: rad,
                speed: spd,
                color: Color::new(1.0, 0.75, 0.25, c_alpha),
                thickness: 0.45,
            });
        }

        self.spawn_decal_anchored(
            Vec3::new(center.x, ground_y + 0.01, center.z),
            Vec3::Y,
            (max_radius * 0.35).clamp(2.5, 7.0),
            Color::new(0.08, 0.07, 0.06, 0.95),
            DecalKind::Scorch,
            None,
        );
    }

    /// Spawns dense showers of flying rubble particles with high tumbling angular velocity
    pub fn add_flying_rubble_shower(
        &mut self,
        center: Vec3,
        radius: f32,
        impulse: f32,
        count: usize,
    ) {
        let debris_count = count.clamp(25, 350);
        for _ in 0..debris_count {
            let dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(0.3f32, 1.8),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(10.0, 32.0) * (impulse / 30.0).clamp(0.7, 2.2);
            let size = rand::gen_range(0.18, 0.48);
            let life = rand::gen_range(8.0, 16.0);

            self.debris.push(DebrisParticle {
                position: center + dir * rand::gen_range(0.2, radius * 0.5),
                velocity: dir * speed,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::new(
                    rand::gen_range(-12.0, 12.0),
                    rand::gen_range(-12.0, 12.0),
                    rand::gen_range(-12.0, 12.0),
                ),
                size,
                color: Color::new(0.65, 0.62, 0.58, 1.0),
                life,
                max_life: life,
                bounces: 0,
            });
        }
    }

    #[allow(dead_code)]
    pub fn spawn_decal(&mut self, pos: Vec3, normal: Vec3, size: f32, color: Color, kind: DecalKind) {
        self.spawn_decal_anchored(pos, normal, size, color, kind, None);
    }

    pub fn spawn_decal_anchored(
        &mut self,
        pos: Vec3,
        normal: Vec3,
        size: f32,
        color: Color,
        kind: DecalKind,
        anchor: Option<IVec3>,
    ) {
        let norm = if normal.length_squared() > 0.001 { normal.normalize() } else { Vec3::Y };
        let life = match kind {
            DecalKind::Scorch => 15.0,
            DecalKind::PlasmaBurn => 12.0,
            DecalKind::CryoFrost => 9.0,
            DecalKind::KineticImpact => 14.0,
            DecalKind::QuenchedObsidian => 18.0,
        };

        if let Some(anc) = anchor {
            if let Some(existing) = self.decals.iter_mut().find(|d| {
                d.voxel_anchor == Some(anc) && d.kind == kind && (d.position - pos).length_squared() < (size * 0.75).powi(2)
            }) {
                existing.life = life;
                existing.max_life = life;
                existing.size = (existing.size * 1.05).min(size * 1.5);
                existing.color = color;
                return;
            }
        }

        let new_decal = ImpactDecal {
            position: pos + norm * 0.015,
            normal: norm,
            size,
            color,
            life,
            max_life: life,
            kind,
            voxel_anchor: anchor,
        };

        if self.decals.len() >= Self::MAX_DECALS {
            // O(1) ring-buffer in-place replacement: replace the decal with the lowest remaining life
            let mut min_idx = 0;
            let mut min_life = f32::INFINITY;
            for (idx, d) in self.decals.iter().enumerate() {
                if d.life < min_life {
                    min_life = d.life;
                    min_idx = idx;
                }
            }
            self.decals[min_idx] = new_decal;
        } else {
            self.decals.push(new_decal);
        }

        // True 3D Volumetric Phenomena paired with surface impact:
        // 1. Dynamic 3D tumbling micro-spall chips
        let spall_count = (size * 3.0).clamp(3.0, 8.0) as usize;
        for _ in 0..spall_count {
            let spread = Vec3::new(
                rand::gen_range(-0.7, 0.7),
                rand::gen_range(0.2, 0.9),
                rand::gen_range(-0.7, 0.7),
            );
            let dir = (norm * 0.8 + spread).normalize_or_zero();
            let spall_speed = rand::gen_range(5.0, 16.0);
            self.debris.push(DebrisParticle {
                position: pos + norm * 0.05,
                velocity: dir * spall_speed,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::new(
                    rand::gen_range(-14.0, 14.0),
                    rand::gen_range(-14.0, 14.0),
                    rand::gen_range(-14.0, 14.0),
                ),
                size: rand::gen_range(0.04, 0.11),
                color,
                life: rand::gen_range(1.8, 3.5),
                max_life: 3.5,
                bounces: 0,
            });
        }

        // 2. 3D Molten fracture fissure sparks & incandescent ember streams
        let spark_count = (size * 4.0).clamp(4.0, 12.0) as usize;
        for _ in 0..spark_count {
            let spread = Vec3::new(
                rand::gen_range(-0.6, 0.6),
                rand::gen_range(0.2, 0.8),
                rand::gen_range(-0.6, 0.6),
            );
            let dir = (norm * 0.75 + spread).normalize_or_zero();
            self.sparks.push(SparkParticle {
                position: pos + norm * 0.04,
                velocity: dir * rand::gen_range(6.0, 18.0),
                color: match kind {
                    DecalKind::PlasmaBurn => Color::new(0.4, 0.9, 1.0, 1.0),
                    DecalKind::Scorch => Color::new(1.0, 0.7, 0.2, 1.0),
                    DecalKind::CryoFrost => Color::new(0.7, 0.95, 1.0, 1.0),
                    DecalKind::KineticImpact => Color::new(1.0, 0.85, 0.4, 1.0),
                    DecalKind::QuenchedObsidian => Color::new(0.65, 0.85, 1.0, 1.0),
                },
                life: rand::gen_range(0.25, 0.6),
                max_life: 0.6,
            });
        }

        // 3. Expanding 3D Spherical Blast Shockwaves for high-energy impacts
        if size > 1.2 || kind == DecalKind::KineticImpact || kind == DecalKind::PlasmaBurn {
            self.shockwaves.push(ShockwaveRing {
                position: pos + norm * 0.05,
                normal: norm,
                radius: 0.2,
                max_radius: (size * 1.6).clamp(1.5, 6.0),
                speed: 18.0,
                color: match kind {
                    DecalKind::PlasmaBurn => Color::new(0.25, 0.85, 1.0, 0.9),
                    DecalKind::Scorch => Color::new(1.0, 0.55, 0.15, 0.9),
                    DecalKind::CryoFrost => Color::new(0.6, 0.95, 1.0, 0.9),
                    DecalKind::KineticImpact => Color::new(0.8, 0.8, 0.85, 0.85),
                    DecalKind::QuenchedObsidian => Color::new(0.35, 0.75, 0.95, 0.9),
                },
                thickness: 0.18,
            });
        }
    }
}

pub fn blackbody_color_ramp(t: f32) -> Color {
    let t_clamped = t.clamp(0.0, 1.0);
    if t_clamped >= 0.70 {
        // 2200K (White-hot) -> 1500K (Incandescent Orange)
        let f = (t_clamped - 0.70) / 0.30;
        let s = f * f * (3.0 - 2.0 * f); // Hermite smoothstep
        let c_high = Color::new(1.0, 0.95, 0.85, 1.0);
        let c_low = Color::new(1.0, 0.55, 0.12, 1.0);
        Color::new(
            c_low.r + (c_high.r - c_low.r) * s,
            c_low.g + (c_high.g - c_low.g) * s,
            c_low.b + (c_high.b - c_low.b) * s,
            c_low.a + (c_high.a - c_low.a) * s,
        )
    } else if t_clamped >= 0.35 {
        // 1500K (Incandescent Orange) -> 900K (Cherry Red)
        let f = (t_clamped - 0.35) / 0.35;
        let s = f * f * (3.0 - 2.0 * f);
        let c_high = Color::new(1.0, 0.55, 0.12, 1.0);
        let c_low = Color::new(0.85, 0.18, 0.08, 0.90);
        Color::new(
            c_low.r + (c_high.r - c_low.r) * s,
            c_low.g + (c_high.g - c_low.g) * s,
            c_low.b + (c_high.b - c_low.b) * s,
            c_low.a + (c_high.a - c_low.a) * s,
        )
    } else if t_clamped >= 0.05 {
        // 900K (Cherry Red) -> 500K (Cooled Dark Slag Crust)
        let f = (t_clamped - 0.05) / 0.30;
        let s = f * f * (3.0 - 2.0 * f);
        let c_high = Color::new(0.85, 0.18, 0.08, 0.90);
        let c_low = Color::new(0.18, 0.12, 0.10, 0.70);
        Color::new(
            c_low.r + (c_high.r - c_low.r) * s,
            c_low.g + (c_high.g - c_low.g) * s,
            c_low.b + (c_high.b - c_low.b) * s,
            c_low.a + (c_high.a - c_low.a) * s,
        )
    } else {
        // 500K (Dark Slag Crust) -> 300K (Cold Ash Fadeout)
        let f = (t_clamped / 0.05).clamp(0.0, 1.0);
        Color::new(0.18, 0.12, 0.10, 0.70 * f)
    }
}

impl PhysicsSystem {
    #[allow(dead_code)]
    pub fn draw_decals(&self, cam_pos: Vec3, dynamic_lights: &[DynamicPointLight]) {
        let _ = self.draw_decals_frustum(cam_pos, dynamic_lights, None);
    }

    pub fn draw_decals_frustum(
        &self,
        cam_pos: Vec3,
        dynamic_lights: &[DynamicPointLight],
        frustum: Option<&FrustumPlanes>,
    ) -> (usize, usize) {
        let mut batcher = self.batcher.borrow_mut();
        batcher.solids_mesh.vertices.clear();
        batcher.solids_mesh.indices.clear();
        let fog_color = Color::new(0.06, 0.08, 0.12, 1.0);
        let mut decals_total = 0;
        let mut decals_drawn = 0;
        for d in &self.decals {
            decals_total += 1;
            let dist_sq = (d.position - cam_pos).length_squared();
            if dist_sq > 110.0 * 110.0 {
                continue; // Early cull far away decals
            }

            let alpha = (d.life / d.max_life).clamp(0.0, 1.0);
            if alpha < 0.02 {
                continue;
            }

            let d_radius = (d.size * 1.2).max(0.5);
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(d.position, d_radius) {
                    continue;
                }
            }
            decals_drawn += 1;

            let dist = dist_sq.sqrt();
            let dist_fog = (1.0 - (-dist * 0.012).exp()).clamp(0.0, 0.85);
            let height_fog = (-d.position.y * 0.035).exp() * 0.10;
            let fog_factor = (dist_fog + height_fog).clamp(0.0, 0.88);

            // Radiant point lighting illumination on decals
            let mut light_r = 0.0f32;
            let mut light_g = 0.0f32;
            let mut light_b = 0.0f32;
            for light in dynamic_lights {
                let dx = (d.position.x - light.position.x).abs();
                let dy = (d.position.y - light.position.y).abs();
                let dz = (d.position.z - light.position.z).abs();
                if dx > light.radius || dy > light.radius || dz > light.radius {
                    continue;
                }
                let d2 = dx * dx + dy * dy + dz * dz;
                let r2 = light.radius * light.radius;
                if d2 < r2 {
                    let atten = (1.0 - (d2 / r2)).powi(2) * light.intensity;
                    light_r += light.color.r * atten;
                    light_g += light.color.g * atten;
                    light_b += light.color.b * atten;
                }
            }

            let n = if d.normal.length_squared() > 0.01 {
                d.normal.normalize()
            } else {
                Vec3::Y
            };
            let tangent = if n.y.abs() > 0.92 {
                Vec3::X
            } else {
                n.cross(Vec3::Y).normalize_or_zero()
            };
            let bitangent = n.cross(tangent).normalize_or_zero();

            let s = d.size * 0.5;
            let p = d.position;

            let norm_age = (d.life / d.max_life).clamp(0.0, 1.0);

            let (core_col, mid_col, shard_col) = match d.kind {
                DecalKind::Scorch => {
                    let c_core = blackbody_color_ramp(norm_age.powf(0.65));
                    let c_mid = blackbody_color_ramp(norm_age);
                    let c_shard = blackbody_color_ramp(norm_age.powf(1.40));
                    (c_core, c_mid, c_shard)
                }
                DecalKind::PlasmaBurn => {
                    // Blinding ionized cyan core that rapidly transitions into 2200K white-hot blackbody
                    if norm_age > 0.85 {
                        let plasma_f = (norm_age - 0.85) / 0.15;
                        let cyan = Color::new(0.50, 0.95, 1.0, 1.0);
                        let white_hot = Color::new(1.0, 0.95, 0.85, 1.0);
                        let core = Color::new(
                            white_hot.r * (1.0 - plasma_f) + cyan.r * plasma_f,
                            white_hot.g * (1.0 - plasma_f) + cyan.g * plasma_f,
                            white_hot.b * (1.0 - plasma_f) + cyan.b * plasma_f,
                            1.0,
                        );
                        (core, blackbody_color_ramp(norm_age), blackbody_color_ramp(norm_age.powf(1.3)))
                    } else {
                        let remapped_t = norm_age / 0.85;
                        let c_core = blackbody_color_ramp(remapped_t.powf(0.70));
                        let c_mid = blackbody_color_ramp(remapped_t);
                        let c_shard = blackbody_color_ramp(remapped_t.powf(1.35));
                        (c_core, c_mid, c_shard)
                    }
                }
                DecalKind::CryoFrost => (
                    Color::new(0.92, 0.98, 1.0, alpha * 0.95),
                    Color::new(0.48, 0.86, 1.0, alpha * 0.85),
                    Color::new(0.32, 0.62, 0.82, alpha * 0.88),
                ),
                DecalKind::KineticImpact => {
                    let thermal_t = (norm_age * 1.5).min(1.0);
                    let white_orange = Color::new(1.0, 0.95, 0.70, alpha);
                    let molten_orange = Color::new(1.0, 0.45, 0.10, alpha);
                    let cherry_red = Color::new(0.55, 0.12, 0.05, alpha);
                    let charcoal = Color::new(0.24, 0.24, 0.26, alpha * 0.92);

                    let c_core = if thermal_t > 0.7 {
                        let f = (thermal_t - 0.7) / 0.3;
                        Color::new(
                            white_orange.r * f + molten_orange.r * (1.0 - f),
                            white_orange.g * f + molten_orange.g * (1.0 - f),
                            white_orange.b * f + molten_orange.b * (1.0 - f),
                            alpha,
                        )
                    } else if thermal_t > 0.3 {
                        let f = (thermal_t - 0.3) / 0.4;
                        Color::new(
                            molten_orange.r * f + cherry_red.r * (1.0 - f),
                            molten_orange.g * f + cherry_red.g * (1.0 - f),
                            molten_orange.b * f + cherry_red.b * (1.0 - f),
                            alpha,
                        )
                    } else {
                        let f = thermal_t / 0.3;
                        Color::new(
                            cherry_red.r * f + charcoal.r * (1.0 - f),
                            cherry_red.g * f + charcoal.g * (1.0 - f),
                            cherry_red.b * f + charcoal.b * (1.0 - f),
                            alpha,
                        )
                    };
                    let c_mid = Color::new(
                        (c_core.r * 0.7 + charcoal.r * 0.3).min(1.0),
                        (c_core.g * 0.7 + charcoal.g * 0.3).min(1.0),
                        (c_core.b * 0.7 + charcoal.b * 0.3).min(1.0),
                        alpha * 0.85,
                    );
                    (c_core, c_mid, charcoal)
                }
                DecalKind::QuenchedObsidian => {
                    let obsidian_black = Color::new(0.08, 0.09, 0.11, alpha * 0.96);
                    let obsidian_rim = Color::new(0.18, 0.22, 0.28, alpha * 0.90);
                    let obsidian_core = Color::new(0.04, 0.05, 0.06, alpha * 0.98);
                    (obsidian_core, obsidian_rim, obsidian_black)
                }
            };

            // Ground shard and fissure coloration with impacted surface voxel hue
            let shard_col = Color::new(
                (shard_col.r * 0.65 + d.color.r * 0.35).min(1.0),
                (shard_col.g * 0.65 + d.color.g * 0.35).min(1.0),
                (shard_col.b * 0.65 + d.color.b * 0.35).min(1.0),
                shard_col.a,
            );
            let mid_col = Color::new(
                (mid_col.r * 0.75 + d.color.r * 0.25).min(1.0),
                (mid_col.g * 0.75 + d.color.g * 0.25).min(1.0),
                (mid_col.b * 0.75 + d.color.b * 0.25).min(1.0),
                mid_col.a,
            );

            let lit_shard = Color::new(
                (shard_col.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r).min(1.0),
                (shard_col.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g).min(1.0),
                (shard_col.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b).min(1.0),
                shard_col.a,
            );
            let lit_mid = Color::new(
                (mid_col.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r).min(1.0),
                (mid_col.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g).min(1.0),
                (mid_col.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b).min(1.0),
                mid_col.a,
            );
            let lit_core = Color::new(
                (core_col.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r * 1.2).min(1.0),
                (core_col.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g * 1.2).min(1.0),
                (core_col.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b * 1.2).min(1.0),
                core_col.a,
            );

            // 1. Tactile 3D Micro-Spall Shards along Crater Rim (3D geometric blocks with real physical thickness)
            let shard_count = 6;
            for s_i in 0..shard_count {
                let angle = (s_i as f32 / shard_count as f32) * std::f32::consts::TAU + (d.size * 3.17);
                let s_radius = s * (0.65 + 0.35 * (s_i as f32 * 1.7).sin().abs());
                let offset_dir = tangent * angle.cos() + bitangent * angle.sin();
                let shard_pos = p + offset_dir * s_radius + n * 0.02;
                let shard_size = (s * 0.18).clamp(0.04, 0.16);
                push_cube_mesh(&mut batcher.solids_mesh, shard_pos, Vec3::splat(shard_size), lit_shard);
            }

            // 2. 3D Branching Molten Fracture Fissures (Jagged, multi-segment radiating 3D cracks with luminous depth)
            let branch_count = 5;
            for b_i in 0..branch_count {
                let base_angle = (b_i as f32 / branch_count as f32) * std::f32::consts::TAU + (b_i as f32 * 0.6);
                let mut prev_pt = p + n * 0.012;
                let segs = 3;
                for seg in 1..=segs {
                    let seg_frac = seg as f32 / segs as f32;
                    let seg_r = s * seg_frac;
                    let jitter_angle = base_angle + ((seg as f32 * 2.3 + b_i as f32 * 1.1).sin() * 0.28);
                    let dir = tangent * jitter_angle.cos() + bitangent * jitter_angle.sin();
                    let next_pt = p + dir * seg_r + n * (0.012 + 0.005 * (seg as f32));

                    // Outer glow fissure quad batched into solids_mesh
                    let seg_dir = (next_pt - prev_pt).normalize_or_zero();
                    let side = seg_dir.cross(n).normalize_or_zero() * 0.016;
                    push_quad_mesh(
                        &mut batcher.solids_mesh,
                        prev_pt - side,
                        prev_pt + side,
                        next_pt + side,
                        next_pt - side,
                        lit_mid,
                    );

                    // Inner luminous core quad on inner segments
                    if seg <= 2 {
                        let inner_side = side * 0.45;
                        let inner_prev = prev_pt + n * 0.003;
                        let inner_next = next_pt + n * 0.003;
                        push_quad_mesh(
                            &mut batcher.solids_mesh,
                            inner_prev - inner_side,
                            inner_prev + inner_side,
                            inner_next + inner_side,
                            inner_next - inner_side,
                            lit_core,
                        );
                    }
                    prev_pt = next_pt;
                }
            }

            // 3. Central Luminous Molten Core (Incandescent 3D ember point batched into solids_mesh)
            if alpha > 0.12 {
                let core_pulse = ((d.life * 6.0).sin() * 0.15 + 0.85) * alpha;
                let core_r = (s * 0.16).clamp(0.03, 0.14) * core_pulse;
                push_cube_mesh(&mut batcher.solids_mesh, p + n * 0.02, Vec3::splat(core_r * 1.5), lit_core);
            }
        }

        if !batcher.solids_mesh.vertices.is_empty() {
            safe_draw_mesh(&batcher.solids_mesh);
            batcher.solids_mesh.vertices.clear();
            batcher.solids_mesh.indices.clear();
        }

        (decals_total, decals_drawn)
    }

    pub fn draw_particles(&self, cam_pos: Vec3, dynamic_lights: &[DynamicPointLight]) {
        self.draw_particles_frustum(cam_pos, dynamic_lights, None);
    }

    pub fn draw_particles_frustum(
        &self,
        cam_pos: Vec3,
        dynamic_lights: &[DynamicPointLight],
        frustum: Option<&FrustumPlanes>,
    ) {
        let mut batcher = self.batcher.borrow_mut();
        batcher.sparks_mesh.vertices.clear();
        batcher.sparks_mesh.indices.clear();
        batcher.smoke_mesh.vertices.clear();
        batcher.smoke_mesh.indices.clear();

        let mut sparks_total = 0;
        let mut sparks_drawn = 0;
        let mut smoke_total = 0;
        let mut smoke_drawn = 0;
        let mut shockwaves_total = 0;
        let mut shockwaves_drawn = 0;
        let mut arcs_total = 0;
        let mut arcs_drawn = 0;
        let mut fire_total = 0;
        let mut fire_drawn = 0;

        // Draw Sparks - Velocity-aligned incandescent streaks with luminous cores
        for sp in &self.sparks {
            sparks_total += 1;
            let dist_sq = (sp.position - cam_pos).length_squared();
            if dist_sq > 110.0 * 110.0 {
                continue;
            }
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(sp.position, 0.40) {
                    continue;
                }
            }
            sparks_drawn += 1;
            let alpha = (sp.life / sp.max_life).min(1.0);
            let c = Color::new(sp.color.r, sp.color.g, sp.color.b, alpha);
            let tail = sp.position - sp.velocity * 0.035;
            let v = sp.position - tail;
            let to_cam = (sp.position - cam_pos).normalize_or_zero();
            let mut side = v.cross(to_cam).normalize_or_zero() * 0.02;
            if side.length_squared() < 1e-6 {
                side = Vec3::new(0.015, 0.0, 0.0);
            }
            push_quad_mesh(
                &mut batcher.sparks_mesh,
                tail - side,
                tail + side,
                sp.position + side,
                sp.position - side,
                c,
            );

            // Close proximity glowing diamond billboard core
            if dist_sq <= 8.0 * 8.0 {
                let up = if to_cam.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
                let right = to_cam.cross(up).normalize_or_zero() * (0.04 * alpha);
                let up_vec = right.cross(to_cam).normalize_or_zero() * (0.04 * alpha);
                let core_col = Color::new(1.0, 1.0, 0.9, alpha);
                push_quad_mesh(
                    &mut batcher.sparks_mesh,
                    sp.position - right,
                    sp.position + up_vec,
                    sp.position + right,
                    sp.position - up_vec,
                    core_col,
                );
            }
        }

        if !batcher.sparks_mesh.vertices.is_empty() {
            safe_draw_mesh(&batcher.sparks_mesh);
            batcher.sparks_mesh.vertices.clear();
            batcher.sparks_mesh.indices.clear();
        }

        // Draw Volumetric Smoke & Dust Plumes with Atmospheric Depth Falloff & Dynamic Lighting
        let fog_color = Color::new(0.06, 0.08, 0.12, 1.0);
        for sm in &self.smoke {
            smoke_total += 1;
            let dist_sq = (sm.position - cam_pos).length_squared();
            if dist_sq > 75.0 * 75.0 {
                continue;
            }
            // Cull distant sub-pixel smoke particles before expensive distance square roots
            if sm.size < 0.20 && dist_sq > 45.0 * 45.0 {
                continue;
            }
            let sm_radius = (sm.size * 1.5).max(0.2);
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(sm.position, sm_radius) {
                    continue;
                }
            }
            let progress = (1.0 - (sm.life / sm.max_life)).clamp(0.0, 1.0);
            let fade = if progress < 0.20 {
                progress / 0.20
            } else {
                (1.0 - progress) / 0.80
            };
            if fade < 0.04 {
                continue;
            }
            let dist = dist_sq.sqrt();
            let cull_feather = 1.0 - ((dist - 55.0) / 20.0).clamp(0.0, 1.0);
            let alpha = (fade * sm.color.a * cull_feather).clamp(0.0, 0.25);
            if alpha < 0.008 {
                continue;
            }
            smoke_drawn += 1;

            let dist_fog = (1.0 - (-dist * 0.012).exp()).clamp(0.0, 0.85);
            let height_fog = ((-sm.position.y * 0.15).exp() * 0.25).clamp(0.0, 0.50);
            let fog_factor = (dist_fog + height_fog).clamp(0.0, 0.88);

            // Explosion-lit dust clouds: dynamic point lights illuminate smoke & dust plumes
            let mut light_r = 0.0f32;
            let mut light_g = 0.0f32;
            let mut light_b = 0.0f32;
            for light in dynamic_lights {
                let dx = (sm.position.x - light.position.x).abs();
                let dy = (sm.position.y - light.position.y).abs();
                let dz = (sm.position.z - light.position.z).abs();
                if dx > light.radius || dy > light.radius || dz > light.radius {
                    continue;
                }
                let d2 = dx * dx + dy * dy + dz * dz;
                let r2 = light.radius * light.radius;
                if d2 < r2 {
                    let atten = (1.0 - (d2 / r2)).powi(2) * light.intensity;
                    light_r += light.color.r * atten * 1.25;
                    light_g += light.color.g * atten * 0.90;
                    light_b += light.color.b * atten * 0.60;
                }
            }

            let fogged_r = (sm.color.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r).min(1.0);
            let fogged_g = (sm.color.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g).min(1.0);
            let fogged_b = (sm.color.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b).min(1.0);

            let to_cam = (cam_pos - sm.position).normalize_or_zero();
            let up = if to_cam.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
            let right = to_cam.cross(up).normalize_or_zero() * sm.size;
            let up_vec = right.cross(to_cam).normalize_or_zero() * sm.size;

            // Dual-tone self-shadowing dust plumes:
            // Top and sun-facing edges receive ambient sun tint (0.85, 0.80, 0.72)
            // Underside receives dark ambient occlusion tint (0.25, 0.28, 0.35)
            let col_top = Color::new(
                (fogged_r * 0.85).min(1.0),
                (fogged_g * 0.80).min(1.0),
                (fogged_b * 0.72).min(1.0),
                alpha,
            );
            let col_bottom = Color::new(
                (fogged_r * 0.25).min(1.0),
                (fogged_g * 0.28).min(1.0),
                (fogged_b * 0.35).min(1.0),
                alpha,
            );

            push_quad_mesh_dual_tone(
                &mut batcher.smoke_mesh,
                sm.position - right - up_vec,
                sm.position + right - up_vec,
                sm.position + right + up_vec,
                sm.position - right + up_vec,
                col_bottom,
                col_top,
            );
        }

        if !batcher.smoke_mesh.vertices.is_empty() {
            safe_draw_mesh(&batcher.smoke_mesh);
            batcher.smoke_mesh.vertices.clear();
            batcher.smoke_mesh.indices.clear();
        }

        // Draw Shockwave Rings with Chromatic Edge Rim & Volumetric Blast Dome
        for sw in &self.shockwaves {
            shockwaves_total += 1;
            let dist_sq = (sw.position - cam_pos).length_squared();
            if dist_sq > 160.0 * 160.0 {
                continue;
            }
            let sw_radius = sw.radius + 0.5;
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(sw.position, sw_radius) {
                    continue;
                }
            }
            shockwaves_drawn += 1;
            let progress = sw.radius / sw.max_radius;
            let alpha = (1.0 - progress).powi(2) * 0.85;
            let c = Color::new(sw.color.r, sw.color.g, sw.color.b, alpha);
            let rim_c = Color::new(
                (c.r * 1.25).min(1.0),
                (c.g * 1.30).min(1.0),
                (c.b * 1.40).min(1.0),
                alpha * 0.45,
            );

            let n = if sw.normal.length_squared() > 0.001 {
                sw.normal.normalize()
            } else {
                Vec3::Y
            };

            if n.y.abs() > 0.95 {
                safe_draw_cylinder(sw.position, sw.radius, (sw.radius - sw.thickness).max(0.01), 0.15, None, c);
                safe_draw_cylinder(sw.position, sw.radius + 0.14, sw.radius, 0.08, None, rim_c);
            } else {
                let tangent = if n.y.abs() > 0.92 {
                    Vec3::X
                } else {
                    n.cross(Vec3::Y).normalize_or_zero()
                };
                let bitangent = n.cross(tangent).normalize_or_zero();
                let center_offset = sw.position + n * 0.035;

                let segs = 32;
                let mut prev_u = tangent;
                for i in 1..=segs {
                    let angle = (i as f32 / segs as f32) * std::f32::consts::TAU;
                    let cur_u = tangent * angle.cos() + bitangent * angle.sin();

                    // Primary shockfront ring
                    let p1 = center_offset + prev_u * sw.radius;
                    let p2 = center_offset + cur_u * sw.radius;
                    safe_draw_line_3d(p1, p2, c);

                    // Inner radial core ring
                    let r_in = (sw.radius - sw.thickness).max(0.01);
                    let in_p1 = center_offset + prev_u * r_in;
                    let in_p2 = center_offset + cur_u * r_in;
                    safe_draw_line_3d(in_p1, in_p2, c);

                    // Outer chromatic refraction rim
                    let r_out = sw.radius + 0.14;
                    let out_p1 = center_offset + prev_u * r_out;
                    let out_p2 = center_offset + cur_u * r_out;
                    safe_draw_line_3d(out_p1, out_p2, rim_c);

                    prev_u = cur_u;
                }
            }

            // Expanding 3D spherical blast dome (culled when faint or distant)
            if progress < 0.65 && dist_sq < 64.0 * 64.0 {
                let dome_alpha = (1.0 - progress / 0.65) * alpha * 0.22;
                if dome_alpha > 0.02 {
                    safe_draw_sphere(
                        sw.position + n * (sw.radius * 0.25),
                        sw.radius * 0.75,
                        None,
                        Color::new(sw.color.r, sw.color.g, sw.color.b, dome_alpha),
                    );
                }
            }
        }

        // Draw Electrical Arcs - high-voltage crackling blue arcs between rebar ends
        for arc in &self.electrical_arcs {
            arcs_total += 1;
            let arc_center = (arc.start + arc.end) * 0.5;
            let arc_radius = (arc.end - arc.start).length() * 0.5 + 0.2;
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(arc_center, arc_radius) {
                    continue;
                }
            }
            arcs_drawn += 1;
            let segs = 4;
            let mut prev = arc.start;
            let dir = arc.end - arc.start;
            let dist = dir.length();
            if dist > 0.01 {
                for s in 1..=segs {
                    let t = s as f32 / segs as f32;
                    let base_pt = arc.start.lerp(arc.end, t);
                    let jitter = if s < segs {
                        cosmetic_arc_jitter(arc.start, arc.timer, s)
                    } else {
                        Vec3::ZERO
                    };
                    let cur = base_pt + jitter;
                    safe_draw_line_3d(prev, cur, arc.color);
                    prev = cur;
                }
            }
        }

        // Draw Pressurized Fire Jets
        for jet in &self.fire_jets {
            fire_total += 1;
            let jet_center = jet.position + jet.direction * 0.9;
            let jet_radius = 1.0;
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(jet_center, jet_radius) {
                    continue;
                }
            }
            fire_drawn += 1;
            let start = jet.position;
            let end = jet.position + jet.direction * 1.8;
            safe_draw_line_3d(start, end, Color::new(1.0, 0.6, 0.1, 0.8));
        }

        let mut metrics = self.cull_metrics.get();
        metrics.sparks_total = sparks_total;
        metrics.sparks_drawn = sparks_drawn;
        metrics.smoke_total = smoke_total;
        metrics.smoke_drawn = smoke_drawn;
        metrics.shockwaves_total = shockwaves_total;
        metrics.shockwaves_drawn = shockwaves_drawn;
        metrics.arcs_total = arcs_total;
        metrics.arcs_drawn = arcs_drawn;
        metrics.fire_total = fire_total;
        metrics.fire_drawn = fire_drawn;
        self.cull_metrics.set(metrics);
    }

    #[allow(dead_code)]
    pub fn draw(&self) {
        self.draw_solids(Vec3::ZERO, &[]);
        self.draw_particles(Vec3::ZERO, &[]);
    }

    pub fn draw_solids(&self, cam_pos: Vec3, dynamic_lights: &[DynamicPointLight]) {
        self.draw_solids_frustum(cam_pos, dynamic_lights, None);
    }

    pub fn draw_solids_frustum(
        &self,
        cam_pos: Vec3,
        dynamic_lights: &[DynamicPointLight],
        frustum: Option<&crate::voxel::FrustumPlanes>,
    ) {
        let fog_color = Color::new(0.06, 0.08, 0.12, 1.0);

        // Draw dynamic 3D tactile impact fracture craters with distance culling & dynamic lighting
        let (decals_total, decals_drawn) = self.draw_decals_frustum(cam_pos, dynamic_lights, frustum);

        let mut batcher = self.batcher.borrow_mut();
        batcher.solids_mesh.vertices.clear();
        batcher.solids_mesh.indices.clear();
        batcher.debris_mesh.vertices.clear();
        batcher.debris_mesh.indices.clear();

        let mut chunks_total = 0;
        let mut chunks_drawn = 0;

        // Draw Dynamic Chunks with 3D Rotation, atmospheric distance fog, and dynamic lighting
        for chunk in &self.chunks {
            chunks_total += 1;
            let chunk_dist_sq = (chunk.position - cam_pos).length_squared();
            if chunk_dist_sq > 160.0 * 160.0 {
                continue; // Early cull far away chunks
            }
            let chunk_radius = chunk.local_aabb.0.length().max(chunk.local_aabb.1.length()) + VOXEL_SIZE;
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(chunk.position, chunk_radius) {
                    continue;
                }
            }
            chunks_drawn += 1;
            let mut relevant_lights = [None; 8];
            let mut relevant_count = 0;
            for light in dynamic_lights {
                let max_dist = light.radius + chunk_radius;
                let dx = (chunk.position.x - light.position.x).abs();
                let dy = (chunk.position.y - light.position.y).abs();
                let dz = (chunk.position.z - light.position.z).abs();
                if dx > max_dist || dy > max_dist || dz > max_dist {
                    continue;
                }
                if dx * dx + dy * dy + dz * dz <= max_dist * max_dist {
                    relevant_lights[relevant_count] = Some(*light);
                    relevant_count += 1;
                    if relevant_count == 8 {
                        break;
                    }
                }
            }
            let chunk_lights = &relevant_lights[..relevant_count];
            let rot_mat = chunk.rot_mat;
            let hx = rot_mat.transform_vector(Vec3::new(VOXEL_SIZE * 0.48, 0.0, 0.0));
            let hy = rot_mat.transform_vector(Vec3::new(0.0, VOXEL_SIZE * 0.48, 0.0));
            let hz = rot_mat.transform_vector(Vec3::new(0.0, 0.0, VOXEL_SIZE * 0.48));

            let voxels_to_render = if chunk.exposed_voxels.is_empty() {
                &chunk.voxels
            } else {
                &chunk.exposed_voxels
            };

            for (local_v, vt) in voxels_to_render {
                let world_pos = chunk.position + rot_mat.transform_vector(*local_v);
                let dist_sq = (world_pos - cam_pos).length_squared();
                if dist_sq > 140.0 * 140.0 {
                    continue;
                }
                let dist = dist_sq.sqrt();
                let c = vt.color();
                let dist_fog = (1.0 - (-dist * 0.012).exp()).clamp(0.0, 0.85);
                let height_fog = ((-world_pos.y * 0.15).exp() * 0.25).clamp(0.0, 0.50);
                let fog_factor = (dist_fog + height_fog).clamp(0.0, 0.88);

                // Dynamic point lighting illumination from chunk-local relevant lights
                let mut light_r = 0.0f32;
                let mut light_g = 0.0f32;
                let mut light_b = 0.0f32;
                for light_opt in chunk_lights {
                    let light = light_opt.as_ref().unwrap();
                    let dx = (world_pos.x - light.position.x).abs();
                    let dy = (world_pos.y - light.position.y).abs();
                    let dz = (world_pos.z - light.position.z).abs();
                    if dx > light.radius || dy > light.radius || dz > light.radius {
                        continue;
                    }
                    let d2 = dx * dx + dy * dy + dz * dz;
                    let r2 = light.radius * light.radius;
                    if d2 < r2 {
                        let atten = (1.0 - (d2 / r2)).powi(2) * light.intensity;
                        light_r += light.color.r * atten;
                        light_g += light.color.g * atten;
                        light_b += light.color.b * atten;
                    }
                }

                let final_c = Color::new(
                    (c.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r).min(1.0),
                    (c.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g).min(1.0),
                    (c.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b).min(1.0),
                    c.a,
                );
                push_oriented_cube_mesh(&mut batcher.solids_mesh, world_pos, hx, hy, hz, final_c);
            }
        }

        if !batcher.solids_mesh.vertices.is_empty() {
            safe_draw_mesh(&batcher.solids_mesh);
            batcher.solids_mesh.vertices.clear();
            batcher.solids_mesh.indices.clear();
        }

        let mut debris_total = 0;
        let mut debris_drawn = 0;

        // Draw Debris Particles with faceted 4-vertex chip batching, distance fog, early culling, and dynamic lighting
        for p in &self.debris {
            debris_total += 1;
            let dist_sq = (p.position - cam_pos).length_squared();
            if dist_sq > 85.0 * 85.0 {
                continue; // Early cull far away debris for rock-solid 60 FPS
            }
            let p_radius = (p.size * 0.75).max(0.05);
            if let Some(f) = frustum {
                if !f.is_sphere_in_frustum(p.position, p_radius) {
                    continue;
                }
            }
            debris_drawn += 1;
            let dist = dist_sq.sqrt();
            let alpha = (p.life / p.max_life).min(1.0);
            let dist_fog = (1.0 - (-dist * 0.012).exp()).clamp(0.0, 0.85);
            let height_fog = ((-p.position.y * 0.15).exp() * 0.25).clamp(0.0, 0.50);
            let fog_factor = (dist_fog + height_fog).clamp(0.0, 0.88);

            // Dynamic point lighting illumination with AABB broadphase pruning
            let mut light_r = 0.0f32;
            let mut light_g = 0.0f32;
            let mut light_b = 0.0f32;
            for light in dynamic_lights {
                let dx = (p.position.x - light.position.x).abs();
                let dy = (p.position.y - light.position.y).abs();
                let dz = (p.position.z - light.position.z).abs();
                if dx > light.radius || dy > light.radius || dz > light.radius {
                    continue;
                }
                let d2 = dx * dx + dy * dy + dz * dz;
                let r2 = light.radius * light.radius;
                if d2 < r2 {
                    let atten = (1.0 - (d2 / r2)).powi(2) * light.intensity;
                    light_r += light.color.r * atten;
                    light_g += light.color.g * atten;
                    light_b += light.color.b * atten;
                }
            }

            let final_c = Color::new(
                (p.color.r * (1.0 - fog_factor) + fog_color.r * fog_factor + light_r).min(1.0),
                (p.color.g * (1.0 - fog_factor) + fog_color.g * fog_factor + light_g).min(1.0),
                (p.color.b * (1.0 - fog_factor) + fog_color.b * fog_factor + light_b).min(1.0),
                p.color.a * alpha,
            );
            // 4-vertex oriented chip billboard: 83% vertex reduction over 24-vertex cube
            let rot_mat = Mat3::from_euler(p.rotation);
            let hx = rot_mat.transform_vector(Vec3::new(p.size * 0.5, 0.0, 0.0));
            let hy = rot_mat.transform_vector(Vec3::new(0.0, p.size * 0.5, 0.0));
            push_quad_mesh(
                &mut batcher.debris_mesh,
                p.position - hx - hy,
                p.position + hx - hy,
                p.position + hx + hy,
                p.position - hx + hy,
                final_c,
            );
        }

        if !batcher.debris_mesh.vertices.is_empty() {
            safe_draw_mesh(&batcher.debris_mesh);
            batcher.debris_mesh.vertices.clear();
            batcher.debris_mesh.indices.clear();
        }

        let mut metrics = self.cull_metrics.get();
        metrics.chunks_total = chunks_total;
        metrics.chunks_drawn = chunks_drawn;
        metrics.debris_total = debris_total;
        metrics.debris_drawn = debris_drawn;
        metrics.decals_total = decals_total;
        metrics.decals_drawn = decals_drawn;
        self.cull_metrics.set(metrics);
    }

    pub fn collect_dynamic_lights(&self, lights: &mut Vec<DynamicPointLight>) {
        for sw in &self.shockwaves {
            let progress = sw.radius / sw.max_radius;
            let alpha = (1.0 - progress).powi(2);
            if alpha > 0.05 {
                lights.push(DynamicPointLight {
                    position: sw.position,
                    color: sw.color,
                    radius: sw.radius * 1.8 + 4.0,
                    intensity: alpha * 1.5,
                });
            }
        }
        for arc in &self.electrical_arcs {
            let progress = arc.timer / arc.max_timer;
            if progress > 0.05 {
                let center = (arc.start + arc.end) * 0.5;
                lights.push(DynamicPointLight {
                    position: center,
                    color: arc.color,
                    radius: 7.0,
                    intensity: progress * (0.8 + (progress * 25.0).sin().abs() * 0.5),
                });
            }
        }
        for jet in &self.fire_jets {
            let progress = jet.timer / jet.max_timer;
            if progress > 0.05 {
                lights.push(DynamicPointLight {
                    position: jet.position + jet.direction * 1.5,
                    color: Color::new(1.0, 0.55, 0.15, 1.0),
                    radius: 8.0,
                    intensity: progress * 1.2,
                });
            }
        }

        // Broadphase point light pruning: cap to 24 most intense lights to eliminate shader overload
        if lights.len() > 24 {
            lights.sort_unstable_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap_or(std::cmp::Ordering::Equal));
            lights.truncate(24);
        }
    }

    pub fn settled_height_at(&self, pos: Vec3) -> Option<f32> {
        let h = self.rubble_height_at(pos.x, pos.z);
        if h > VOID_BOUNDARY_Y + 0.05 {
            Some(h)
        } else {
            None
        }
    }

    pub fn check_chunk_hit(&self, start: Vec3, end: Vec3) -> Option<(Vec3, Vec3)> {
        self.check_chunk_hit_radius(start, end, 0.0)
    }

    /// Sweeps a projectile against chunk voxels by conservatively inflating their body-frame
    /// boxes by `radius`. The hit point is the projectile center at first contact; corners
    /// can register before a true swept sphere. A zero radius gives an exact ray contact.
    pub fn check_chunk_hit_radius(&self, start: Vec3, end: Vec3, radius: f32) -> Option<(Vec3, Vec3)> {
        // Intersect in each chunk's body frame so rotating rubble keeps its actual shape.
        fn ray_box_hit(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3, limit: f32) -> Option<(f32, Vec3)> {
            let mut near = 0.0f32;
            let mut far = limit;
            let mut normal = -dir;
            for axis in 0..3 {
                if dir[axis].abs() < 1e-8 {
                    if origin[axis] < min[axis] || origin[axis] > max[axis] {
                        return None;
                    }
                    continue;
                }
                let t1 = (min[axis] - origin[axis]) / dir[axis];
                let t2 = (max[axis] - origin[axis]) / dir[axis];
                let entry = t1.min(t2);
                if entry > near {
                    near = entry;
                    normal = Vec3::ZERO;
                    normal[axis] = -dir[axis].signum();
                }
                far = far.min(t1.max(t2));
                if near > far {
                    return None;
                }
            }
            Some((near, normal))
        }

        let ray_vec = end - start;
        let ray_len = ray_vec.length();
        if ray_len < 0.001 || !ray_len.is_finite() || !radius.is_finite() { return None; }
        let ray_dir = ray_vec / ray_len;
        let half_voxel = Vec3::splat(VOXEL_SIZE * 0.5 + radius.max(0.0));
        let mut closest = ray_len;
        let mut hit = None;

        for chunk in &self.chunks {
            let local_origin = chunk.rot_mat.transform_vector_transposed(start - chunk.position);
            let local_dir = chunk.rot_mat.transform_vector_transposed(ray_dir);
            // Reject entire chunks before checking individual voxel boxes; the cached AABB
            // contains voxel centers, so include the half-voxel extent on every face.
            let Some((entry, _)) = ray_box_hit(local_origin, local_dir, chunk.local_aabb.0 - half_voxel, chunk.local_aabb.1 + half_voxel, closest) else {
                continue;
            };
            // An outside ray first enters a surface voxel. Keep the full scan when starting
            // inside the bounds so an origin within an interior voxel still hits at zero.
            let voxels = if entry > 0.0 && !chunk.exposed_voxels.is_empty() { &chunk.exposed_voxels } else { &chunk.voxels };
            for &(p, vt) in voxels {
                if !vt.is_solid() { continue; }
                if let Some((distance, normal)) = ray_box_hit(local_origin, local_dir, p - half_voxel, p + half_voxel, closest) {
                    if hit.is_none() || distance < closest {
                        closest = distance;
                        hit = Some((start + ray_dir * distance, chunk.rot_mat.transform_vector(normal)));
                    }
                }
            }
        }
        hit
    }

    // Directional kinetic blast for Titan Maul: shoves heavy chunks with immense inertia
    pub fn apply_kinetic_blast(&mut self, origin: Vec3, dir: Vec3, range: f32, force: f32) {
        let dir = dir.normalize_or_zero();
        for chunk in &mut self.chunks {
            let delta = chunk.position - origin;
            let dist = delta.length();
            if dist < range + 4.0 && dist > 0.1 {
                let norm = delta / dist;
                let dot = norm.dot(dir);
                if dot > 0.25 {
                    let cone_factor = dot.max(0.0);
                    let push_mag = (force * cone_factor) / (dist * 0.35 + 1.0);
                    let accel = push_mag / chunk.mass.powf(0.35).max(1.0);
                    chunk.velocity += (dir * 0.85 + Vec3::Y * 0.25) * accel;
                    chunk.angular_velocity += Vec3::new(
                        rand::gen_range(-6.0, 6.0),
                        rand::gen_range(-4.0, 4.0),
                        rand::gen_range(-6.0, 6.0),
                    ) * (accel * 0.15);
                    if chunk.settled {
                        self.rubble_dirty = true;
                    }
                    chunk.settled = false;
                }
            }
        }

        for p in &mut self.debris {
            let delta = p.position - origin;
            let dist = delta.length();
            if dist < range + 5.0 && dist > 0.1 {
                let norm = delta / dist;
                let dot = norm.dot(dir);
                if dot > 0.25 {
                    p.velocity += (dir * 0.85 + norm * 0.35) * (force * 1.5 / (dist * 0.3 + 1.0));
                }
            }
        }
    }

    // Radial beam shockwave for Apex Railgun: blows chunks away from beam trajectory
    pub fn apply_beam_shockwave(&mut self, start: Vec3, end: Vec3, radius: f32, force: f32) {
        let beam_vec = end - start;
        let beam_len = beam_vec.length();
        if beam_len < 0.1 {
            return;
        }
        let beam_dir = beam_vec / beam_len;

        for chunk in &mut self.chunks {
            let to_chunk = chunk.position - start;
            let proj = to_chunk.dot(beam_dir).clamp(0.0, beam_len);
            let closest = start + beam_dir * proj;
            let radial = chunk.position - closest;
            let dist = radial.length();
            if dist < radius && dist > 0.05 {
                let norm = radial / dist;
                let push = (force / (dist * 0.5 + 1.0)) / chunk.mass.powf(0.35).max(1.0);
                chunk.velocity += (norm * 0.85 + Vec3::Y * 0.25) * push;
                chunk.angular_velocity += Vec3::new(rand::gen_range(-5.0, 5.0), rand::gen_range(-5.0, 5.0), rand::gen_range(-5.0, 5.0));
                if chunk.settled {
                    self.rubble_dirty = true;
                }
                chunk.settled = false;
            }
        }
    }

    // Omnidirectional explosive blast: violently hurls dynamic chunks and debris outward
    // Phase 1 (0 to R): Intense outward kinetic impulse with compressive shock.
    // Phase 2 (R to 1.65R): Secondary negative-pressure zone that draws lightweight debris and smoke inward toward epicenter.
    pub fn apply_radial_blast(&mut self, origin: Vec3, radius: f32, force: f32) {
        let mut blasted_settled_chunks = std::mem::take(&mut self.scratch_blasted_settled_chunks);
        blasted_settled_chunks.clear();

        for chunk in &mut self.chunks {
            let delta = chunk.position - origin;
            let dist = delta.length();
            if dist < radius && dist > 0.05 {
                let norm = delta / dist;
                let falloff = (1.0 - (dist / radius)).clamp(0.0, 1.0);
                let push = (force * falloff) / chunk.mass.powf(0.35).max(1.0);
                chunk.velocity += (norm * 0.82 + Vec3::Y * 0.35) * push;
                chunk.angular_velocity += Vec3::new(
                    rand::gen_range(-4.0, 4.0),
                    rand::gen_range(-4.0, 4.0),
                    rand::gen_range(-4.0, 4.0),
                ) * (push * 0.12);
                if chunk.settled {
                    self.rubble_dirty = true;
                    blasted_settled_chunks.push((chunk.position, chunk.local_aabb.0.length().max(chunk.local_aabb.1.length()) + VOXEL_SIZE));
                }
                chunk.settled = false;
            }
        }

        // Multi-chunk stacking load transfer: Blasting supporting lower chunks re-wakes resting chunks stacked above!
        if !blasted_settled_chunks.is_empty() {
            for chunk in &mut self.chunks {
                if !chunk.settled {
                    continue;
                }
                for &(b_pos, b_rad) in &blasted_settled_chunks {
                    if chunk.position.y > b_pos.y {
                        let dx = chunk.position.x - b_pos.x;
                        let dz = chunk.position.z - b_pos.z;
                        if dx * dx + dz * dz <= (b_rad + 2.5) * (b_rad + 2.5) {
                            chunk.settled = false;
                            chunk.velocity = Vec3::new(
                                rand::gen_range(-0.4, 0.4),
                                -2.5,
                                rand::gen_range(-0.4, 0.4),
                            );
                            chunk.angular_velocity += Vec3::new(
                                rand::gen_range(-1.5, 1.5),
                                rand::gen_range(-1.5, 1.5),
                                rand::gen_range(-1.5, 1.5),
                            );
                            self.rubble_dirty = true;
                            break;
                        }
                    }
                }
            }
        }

        blasted_settled_chunks.clear();
        self.scratch_blasted_settled_chunks = blasted_settled_chunks;

        self.apply_radial_blast_particles(origin, radius, force);
    }

    /// Visual-only part of a radial blast: pushes debris particles and smoke (outward shock,
    /// then negative-pressure suction) without imparting any impulse to rigid chunks.
    pub fn apply_radial_blast_particles(&mut self, origin: Vec3, radius: f32, force: f32) {
        // Debris: Phase 1 outward shock + Phase 2 negative-pressure suction wave
        for p in &mut self.debris {
            let delta = p.position - origin;
            let dist = delta.length();
            if dist > 0.05 {
                if dist < radius {
                    let norm = delta / dist;
                    let falloff = (1.0 - (dist / radius)).clamp(0.0, 1.0);
                    p.velocity += (norm * 0.85 + Vec3::Y * 0.3) * (force * 1.4 * falloff);
                } else if dist < radius * 1.65 {
                    // Secondary negative-pressure zone: draws lightweight debris inward
                    let norm = delta / dist;
                    let suction_progress = (dist - radius) / (radius * 0.65);
                    let suction_falloff = (1.0 - suction_progress).clamp(0.0, 1.0);
                    let suction_strength = if p.size < 0.25 { 0.45 } else { 0.18 };
                    p.velocity += (-norm * (force * suction_strength) + Vec3::new(0.0, 0.6, 0.0)) * suction_falloff;
                }
            }
        }

        // Smoke: Secondary negative-pressure suction wave pulls billowing clouds inward and upward
        for sm in &mut self.smoke {
            let delta = sm.position - origin;
            let dist = delta.length();
            if dist > 0.1 {
                if dist < radius * 0.75 {
                    let norm = delta / dist;
                    sm.velocity += norm * (force * 0.45);
                } else if dist < radius * 1.75 {
                    let norm = delta / dist;
                    let suction_progress = (dist - radius * 0.75) / (radius * 1.0);
                    let suction_falloff = (1.0 - suction_progress).clamp(0.0, 1.0);
                    sm.velocity += (-norm * (force * 0.32) + Vec3::new(0.0, 1.8, 0.0)) * suction_falloff;
                }
            }
        }
    }

    // Stress Fissure Spawning: shoots out plumes of mortar dust sheets, sparks, and plaster flakes
    pub fn spawn_stress_fissure(&mut self, origin: Vec3, normal: Vec3, count: usize) {
        let tangent = if normal.y.abs() < 0.9 {
            normal.cross(Vec3::Y).normalize_or_zero()
        } else {
            normal.cross(Vec3::X).normalize_or_zero()
        };

        for _ in 0..count {
            let offset = tangent * rand::gen_range(-1.5, 1.5) + normal * rand::gen_range(0.1, 0.4);
            let eject_dir = (normal * 0.7 + tangent * rand::gen_range(-0.8, 0.8) + Vec3::new(0.0, rand::gen_range(-0.3, 0.8), 0.0)).normalize_or_zero();

            // High-speed mortar powder dust puff - subtle wisps
            let s_life = rand::gen_range(0.30, 0.55);
            let start_size = rand::gen_range(0.05, 0.10);
            let max_size = rand::gen_range(0.16, 0.28);
            self.smoke.push(SmokeParticle {
                position: origin + offset,
                velocity: eject_dir * rand::gen_range(1.5, 3.2),
                size: start_size,
                start_size,
                max_size,
                color: Color::new(0.72, 0.70, 0.67, 0.12),
                life: s_life,
                max_life: s_life,
            });

            // Concrete flakes
            self.debris.push(DebrisParticle {
                position: origin + offset,
                velocity: eject_dir * rand::gen_range(4.0, 12.0),
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::splat(rand::gen_range(-8.0, 8.0)),
                size: rand::gen_range(0.08, 0.18),
                color: Color::new(0.6, 0.6, 0.62, 1.0),
                life: rand::gen_range(4.0, 8.0),
                max_life: 8.0,
                bounces: 0,
            });

            // Friction sparks from sheared rebar
            if rand::gen_range(0.0, 1.0) < 0.4 {
                self.sparks.push(SparkParticle {
                    position: origin + offset,
                    velocity: eject_dir * rand::gen_range(8.0, 20.0),
                    color: Color::new(1.0, 0.8, 0.2, 1.0),
                    life: rand::gen_range(0.4, 0.9),
                    max_life: 0.9,
                });
            }
        }
    }

    // High-energy kinetic spall impacts: sparks and tungsten/concrete flakes spray off high-hardness materials
    pub fn spawn_spall_impact(&mut self, pos: Vec3, normal: Vec3, color: Color) {
        let count = 10;
        for _ in 0..count {
            let spread = Vec3::new(
                rand::gen_range(-0.8, 0.8),
                rand::gen_range(0.1, 0.9),
                rand::gen_range(-0.8, 0.8),
            );
            let dir = (normal * 0.7 + spread).normalize_or_zero();
            let speed = rand::gen_range(8.0, 22.0);

            // High-speed ricochet spark
            self.sparks.push(SparkParticle {
                position: pos + normal * 0.08,
                velocity: dir * speed,
                color: Color::new(1.0, 0.85, 0.35, 1.0),
                life: rand::gen_range(0.2, 0.55),
                max_life: 0.55,
            });

            // Micro spall chip
            if rand::gen_range(0.0, 1.0) < 0.65 {
                self.debris.push(DebrisParticle {
                    position: pos + normal * 0.08,
                    velocity: dir * (speed * 0.55),
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::splat(rand::gen_range(-12.0, 12.0)),
                    size: rand::gen_range(0.05, 0.12),
                    color,
                    life: rand::gen_range(2.0, 4.5),
                    max_life: 4.5,
                    bounces: 0,
                });
            }
        }
    }

    /// Bent rebar plastic deformation: severed composite/steel rebar voxels spawn
    /// bent metallic splinters curling along the impulse vector with glowing sheared tips.
    pub fn spawn_bent_rebar(&mut self, pos: Vec3, impulse: Vec3, is_composite: bool) {
        let base_col = if is_composite {
            Color::new(0.10, 0.78, 0.88, 1.0) // Electric cyan composite rebar
        } else {
            Color::new(0.92, 0.46, 0.12, 1.0) // Vibrant hazard orange steel rebar
        };
        let dir = impulse.normalize_or_zero();
        let perp = if dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let curl = dir.cross(perp).normalize_or_zero();

        for i in 0..5 {
            let seg_t = i as f32 / 4.0;
            let offset = dir * (i as f32 * 0.16) + curl * (seg_t * seg_t * 0.28);
            let tip_glow = (seg_t * 1.5).min(1.0);
            let splinter_col = Color::new(
                (base_col.r * (1.0 - tip_glow * 0.4) + 1.0 * tip_glow * 0.8).min(1.0),
                (base_col.g * (1.0 - tip_glow * 0.4) + 0.85 * tip_glow * 0.8).min(1.0),
                (base_col.b * (1.0 - tip_glow * 0.4) + 0.3 * tip_glow * 0.8).min(1.0),
                1.0,
            );

            self.debris.push(DebrisParticle {
                position: pos + offset,
                velocity: dir * rand::gen_range(3.5, 9.0) + curl * rand::gen_range(2.0, 6.0) + Vec3::new(0.0, 1.5, 0.0),
                rotation: Vec3::ZERO,
                angular_velocity: curl * rand::gen_range(12.0, 24.0),
                size: rand::gen_range(0.10, 0.20),
                color: splinter_col,
                life: rand::gen_range(5.0, 9.0),
                max_life: 9.0,
                bounces: 0,
            });
        }

        // Friction sparks from shearing rebar
        for _ in 0..8 {
            self.sparks.push(SparkParticle {
                position: pos,
                velocity: (dir * 0.7 + curl * rand::gen_range(-0.5, 0.5) + Vec3::new(0.0, rand::gen_range(0.2, 0.8), 0.0)).normalize_or_zero() * rand::gen_range(8.0, 18.0),
                color: Color::new(1.0, 0.9, 0.4, 1.0),
                life: rand::gen_range(0.25, 0.55),
                max_life: 0.55,
            });
        }
    }

    // Molten thermal fissure emission: incandescent embers and hot gas for plasma drill cuts
    pub fn spawn_molten_fissure(&mut self, origin: Vec3, normal: Vec3, count: usize) {
        for _ in 0..count {
            let offset = Vec3::new(
                rand::gen_range(-0.35, 0.35),
                rand::gen_range(-0.35, 0.35),
                rand::gen_range(-0.35, 0.35),
            );
            let dir = (normal + offset).normalize_or_zero();
            self.sparks.push(SparkParticle {
                position: origin + offset,
                velocity: dir * rand::gen_range(4.0, 12.0),
                color: Color::new(1.0, rand::gen_range(0.4, 0.7), 0.08, 1.0),
                life: rand::gen_range(0.5, 1.1),
                max_life: 1.1,
            });
            let s_life = rand::gen_range(0.30, 0.50);
            let start_size = rand::gen_range(0.05, 0.09);
            let max_size = rand::gen_range(0.16, 0.26);
            self.smoke.push(SmokeParticle {
                position: origin + offset,
                velocity: dir * 0.9 + Vec3::new(0.0, 0.6, 0.0),
                size: start_size,
                start_size,
                max_size,
                color: Color::new(0.28, 0.20, 0.16, 0.11),
                life: s_life,
                max_life: s_life,
            });
        }
    }

    // Ice Shatter Effects for Frostbite cryo-fracture
    pub fn spawn_ice_shatter_effects(&mut self, center: Vec3, radius: f32, count: usize) {
        self.shockwaves.push(ShockwaveRing {
            position: center,
            normal: Vec3::Y,
            radius: 0.3,
            max_radius: radius * 2.5,
            speed: 28.0,
            color: Color::new(0.6, 0.95, 1.0, 0.9),
            thickness: 0.25,
        });

        for _ in 0..count {
            let dir = Vec3::new(
                rand::gen_range(-1.0f32, 1.0),
                rand::gen_range(0.2f32, 1.6),
                rand::gen_range(-1.0f32, 1.0),
            ).normalize();

            let speed = rand::gen_range(8.0, 26.0);
            let size = rand::gen_range(0.08, 0.25);
            let life = rand::gen_range(6.0, 12.0);

            self.debris.push(DebrisParticle {
                position: center + dir * rand::gen_range(0.1, radius * 0.6),
                velocity: dir * speed,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::splat(rand::gen_range(-14.0, 14.0)),
                size,
                color: Color::new(0.7, 0.94, 1.0, 0.85),
                life,
                max_life: life,
                bounces: 0,
            });

            if rand::gen_range(0.0, 1.0) < 0.25 {
                let s_life = rand::gen_range(0.30, 0.50);
                let start_size = 0.05;
                let max_size = 0.18;
                self.smoke.push(SmokeParticle {
                    position: center + dir * rand::gen_range(0.1, radius * 0.3),
                    velocity: dir * rand::gen_range(1.0, 2.2) + Vec3::new(0.0, 0.3, 0.0),
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.8, 0.95, 1.0, 0.11),
                    life: s_life,
                    max_life: s_life,
                });
            }
        }
        self.quench_molten_slag(center, radius);
    }

    /// Coolant pipe ruptures & slag quenching: rupturing coolant pipes sprays high-pressure vapor
    /// that rapidly solidifies glowing molten slag fissures into dark obsidian rock with steam plumes.
    pub fn quench_molten_slag(&mut self, origin: Vec3, radius: f32) -> usize {
        let mut quenched = 0;
        let r_sq = (radius * 1.5) * (radius * 1.5);
        for d in &mut self.decals {
            if (d.kind == DecalKind::PlasmaBurn || d.kind == DecalKind::KineticImpact)
                && (d.position - origin).length_squared() <= r_sq
            {
                d.kind = DecalKind::QuenchedObsidian;
                d.life = d.max_life * 0.85;
                d.color = Color::new(0.08, 0.09, 0.11, 1.0);
                quenched += 1;
            }
        }

        // High-pressure cryogenic vapor sprays & expanding steam plumes
        let steam_count = (quenched * 4).clamp(6, 24);
        for _ in 0..steam_count {
            let steam_dir = Vec3::new(
                rand::gen_range(-1.0, 1.0),
                rand::gen_range(0.3, 1.5),
                rand::gen_range(-1.0, 1.0),
            ).normalize_or_zero();
            let s_life = rand::gen_range(1.0, 2.0);
            let start_size = rand::gen_range(0.25, 0.45);
            let max_size = rand::gen_range(1.1, 2.0);
            if self.smoke.len() < Self::MAX_SMOKE {
                self.smoke.push(SmokeParticle {
                    position: origin + steam_dir * rand::gen_range(0.1, radius * 0.7),
                    velocity: steam_dir * rand::gen_range(3.0, 7.5) + self.wind * 0.35,
                    size: start_size,
                    start_size,
                    max_size,
                    color: Color::new(0.92, 0.96, 1.0, 0.30),
                    life: s_life,
                    max_life: s_life,
                });
            }
        }

        // High-pressure cryogenic vapor droplets
        let droplet_count = (quenched * 3).clamp(4, 16);
        for _ in 0..droplet_count {
            if self.sparks.len() < Self::MAX_SPARKS {
                let spray_dir = Vec3::new(
                    rand::gen_range(-1.0, 1.0),
                    rand::gen_range(0.1, 0.7),
                    rand::gen_range(-1.0, 1.0),
                ).normalize_or_zero();
                self.sparks.push(SparkParticle {
                    position: origin + spray_dir * 0.25,
                    velocity: spray_dir * rand::gen_range(7.0, 16.0),
                    color: Color::new(0.72, 0.92, 1.0, 0.9),
                    life: rand::gen_range(0.25, 0.65),
                    max_life: 0.65,
                });
            }
        }

        quenched
    }

    /// Pre-failure column buckling animation: overloaded columns visibly bulge, crack,
    /// and shed discrete concrete chunks prior to structural failure.
    pub fn spawn_column_buckling(&mut self, pos: Vec3, normal: Vec3) {
        let norm = if normal.length_squared() > 0.01 { normal.normalize() } else { Vec3::Y };

        // 1. Cracking stress fissure emission & incandescent micro-fractures
        self.spawn_stress_fissure(pos, norm, 14);

        // 2. Shed discrete concrete chunks falling under gravity
        let chunk_count = rand::gen_range(4, 8);
        for _ in 0..chunk_count {
            if self.debris.len() < Self::MAX_DEBRIS {
                let outward = (norm + Vec3::new(
                    rand::gen_range(-0.4, 0.4),
                    rand::gen_range(0.1, 0.45),
                    rand::gen_range(-0.4, 0.4),
                )).normalize_or_zero();
                let speed = rand::gen_range(3.0, 7.5);
                let size = VOXEL_SIZE * rand::gen_range(0.22, 0.50);
                self.debris.push(DebrisParticle {
                    position: pos + outward * rand::gen_range(0.08, 0.30),
                    velocity: outward * speed + Vec3::new(0.0, rand::gen_range(0.4, 1.8), 0.0),
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::new(
                        rand::gen_range(-12.0, 12.0),
                        rand::gen_range(-12.0, 12.0),
                        rand::gen_range(-12.0, 12.0),
                    ),
                    size,
                    color: Color::new(0.66, 0.63, 0.59, 1.0), // Concrete rubble tone
                    life: rand::gen_range(4.5, 9.0),
                    max_life: 9.0,
                    bounces: 0,
                });
            }
        }

        // 3. Spall dust puff from compressive buckling face
        for _ in 0..3 {
            if self.smoke.len() < Self::MAX_SMOKE {
                let s_dir = (norm + Vec3::new(rand::gen_range(-0.3, 0.3), 0.25, rand::gen_range(-0.3, 0.3))).normalize_or_zero();
                self.smoke.push(SmokeParticle {
                    position: pos + s_dir * 0.15,
                    velocity: s_dir * rand::gen_range(1.4, 3.2),
                    size: 0.14,
                    start_size: 0.14,
                    max_size: 0.48,
                    color: Color::new(0.70, 0.67, 0.63, 0.25),
                    life: rand::gen_range(0.6, 1.3),
                    max_life: 1.3,
                });
            }
        }

        // 4. Place concrete fracture / buckling decal on column surface
        self.spawn_decal_anchored(
            pos,
            norm,
            rand::gen_range(0.8, 1.4),
            Color::new(0.55, 0.52, 0.48, 1.0),
            DecalKind::KineticImpact,
            None,
        );
    }

    // Seismic Shockwave Pulse: Ground tremor shockwave with directional radial resonance
    pub fn spawn_seismic_shockwave(&mut self, origin: Vec3, radius: f32, power: f32) {
        self.shockwaves.push(ShockwaveRing {
            position: origin,
            normal: Vec3::Y,
            radius: 0.4,
            max_radius: radius * 2.2,
            speed: 34.0,
            color: Color::new(0.95, 0.75, 0.2, 0.85),
            thickness: 0.35,
        });
        self.apply_radial_blast(origin, radius * 1.5, power);
    }

    /// Gravitational Singularity Suction: Attracts dynamic chunks, physical debris, smoke clouds,
    /// and incandescent sparks towards the vortex center with orbital swirl and gravity-overriding lift.
    pub fn apply_vortex_suction(&mut self, vortex_center: Vec3, radius: f32, pull_strength: f32, dt: f32) {
        let radius_sq = radius * radius;

        // 1. Dynamic Building Chunks: Inward gravitational acceleration + tangential orbital swirl + lift
        for chunk in &mut self.chunks {
            let delta = vortex_center - chunk.position;
            let d2 = delta.length_squared();
            if d2 < radius_sq && d2 > 0.04 {
                let dist = d2.sqrt();
                let norm = delta / dist;
                let falloff = 1.0 / (dist * 0.30 + 1.20); // Softened 1/r law
                let pull = norm * (pull_strength * falloff);
                let swirl = delta.cross(Vec3::Y).normalize_or_zero() * (pull_strength * 0.52 * falloff);
                let lift = Vec3::Y * (24.0 * (1.0 - (dist / radius)).clamp(0.0, 1.0));

                chunk.velocity += (pull + swirl + lift) * dt;
                chunk.angular_velocity += Vec3::new(swirl.z, pull.y * 1.5, -swirl.x) * dt;

                // Core accretion disc damping: traps matter tightly orbiting at core without slingshotting
                if dist < 3.2 {
                    chunk.velocity *= (1.0 - 0.35 * dt).max(0.70);
                }

                if chunk.settled {
                    self.rubble_dirty = true;
                }
                chunk.settled = false;
            }
        }

        // 2. Physical Debris Particles: High-speed inward spiral into accretion disk
        for p in &mut self.debris {
            let delta = vortex_center - p.position;
            let d2 = delta.length_squared();
            if d2 < radius_sq && d2 > 0.04 {
                let dist = d2.sqrt();
                let norm = delta / dist;
                let pull = norm * (pull_strength * 1.80 / (dist * 0.28 + 1.0));
                let swirl = delta.cross(Vec3::Y).normalize_or_zero() * (pull_strength * 0.85 / (dist * 0.35 + 1.0));
                p.velocity += (pull + swirl) * dt;
            }
        }

        // 3. Smoke & Dust Plumes: Billowing dust clouds sucked into the vortex center
        for sm in &mut self.smoke {
            let delta = vortex_center - sm.position;
            let d2 = delta.length_squared();
            if d2 < radius_sq && d2 > 0.04 {
                let dist = d2.sqrt();
                let norm = delta / dist;
                let pull = norm * (pull_strength * 1.65 / (dist * 0.30 + 1.0));
                let swirl = delta.cross(Vec3::Y).normalize_or_zero() * (pull_strength * 0.75 / (dist * 0.35 + 1.0));
                sm.velocity += (pull + swirl) * dt;
            }
        }

        // 4. Incandescent Sparks: Drawn along tight spiral trajectories into relativistic photon ring
        for sp in &mut self.sparks {
            let delta = vortex_center - sp.position;
            let d2 = delta.length_squared();
            if d2 < radius_sq && d2 > 0.04 {
                let dist = d2.sqrt();
                let norm = delta / dist;
                let pull = norm * (pull_strength * 2.20 / (dist * 0.25 + 1.0));
                let swirl = delta.cross(Vec3::Y).normalize_or_zero() * (pull_strength * 1.10 / (dist * 0.30 + 1.0));
                sp.velocity += (pull + swirl) * dt;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(position: Vec3, rotation: Vec3, voxels: Vec<(Vec3, VoxelType)>) -> DynamicChunk {
        let aabb = DynamicChunk::compute_aabb(&voxels);
        DynamicChunk::new(position, Vec3::ZERO, rotation, Vec3::ZERO, voxels, 20.0, 10.0, false, aabb)
    }

    #[test]
    fn chunk_ablation_preserves_rotated_survivors_and_point_velocity() {
        let grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let mut body = chunk(Vec3::new(4.0, 8.0, 3.0), Vec3::new(0.2, 0.7, 0.3), vec![
            (Vec3::ZERO, VoxelType::Concrete),
            (Vec3::X, VoxelType::SteelRebar),
            (Vec3::X * 2.0, VoxelType::Glass),
        ]);
        body.velocity = Vec3::new(2.0, -1.0, 0.5);
        body.angular_velocity = Vec3::new(0.3, 0.6, -0.2);
        body.settled = true;
        let center = body.local_to_world(Vec3::ZERO);
        let survivors: Vec<_> = body.voxels[1..].iter().map(|&(p, material)| {
            let world = body.local_to_world(p);
            (world, material, body.point_velocity(world - body.position))
        }).collect();
        let original_mass = body.mass;
        physics.chunks.push(body);
        let (destroyed, fuels) = physics.carve_chunk_sphere(&grid, center, 0.4, 950.0, crate::voxel::DamageType::Explosive, false);
        assert_eq!(destroyed, 1);
        assert!(fuels.is_empty());
        let body = &physics.chunks[0];
        assert!(!body.settled);
        assert!(physics.rubble_dirty);
        let expected_mass = original_mass * (5.5 + 0.8) / (2.4 + 5.5 + 0.8);
        assert!((body.mass - expected_mass).abs() < 1e-5);
        assert_eq!(body.local_aabb, DynamicChunk::compute_aabb(&body.voxels));
        assert_eq!(body.exposed_voxels.len(), 2);
        for (&(local, material), &(world, old_material, velocity)) in body.voxels.iter().zip(&survivors) {
            assert_eq!(material, old_material);
            assert!(body.local_to_world(local).distance(world) < 1e-5);
            assert!(body.point_velocity(world - body.position).distance(velocity) < 1e-5);
        }
    }

    #[test]
    fn chunk_ablation_respects_resistance_brittle_radius_and_removed_supports() {
        let grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let center = Vec3::new(0.0, 5.0, 0.0);
        let mut support = chunk(center, Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Concrete)]);
        support.settled = true;
        physics.chunks.push(support);
        physics.chunks.push(chunk(center + Vec3::X * 0.5, Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::HeavyArmor)]));
        physics.chunks.push(chunk(center + Vec3::X * 1.32, Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Glass)]));
        physics.chunks.push(chunk(center + Vec3::Z * 0.5, Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Bedrock)]));
        physics.rebuild_rubble_heightmap_with_grid(&grid);
        assert!(physics.rubble_height_at(center.x, center.z) > 4.0);
        let (destroyed, _) = physics.carve_chunk_sphere(&grid, center, 1.0, 70.0, crate::voxel::DamageType::Thermal, true);
        assert_eq!(destroyed, 1, "Concrete ablates; armor needs more energy and bedrock is laser protected");
        assert!(physics.rubble_height_at(center.x, center.z) < 4.0, "Erased support cannot remain in the height map");
        let (destroyed, _) = physics.carve_chunk_sphere(&grid, center, 1.0, 950.0, crate::voxel::DamageType::Explosive, false);
        assert_eq!(destroyed, 2, "Armor and glass in the extended brittle radius shatter");
        assert_eq!(physics.chunks.len(), 1, "Resistant bedrock survives this attenuated blast");
        assert_eq!(physics.chunks[0].voxels[0].1, VoxelType::Bedrock);
    }

    #[test]
    fn chunk_damage_accumulates_and_survives_slicing_and_fracture() {
        let grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let center = Vec3::new(0.0, 8.0, 0.0);
        physics.chunks.push(chunk(center, Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::HeavyArmor)]));
        assert_eq!(physics.carve_chunk_sphere(&grid, center, 0.65, 140.0, crate::voxel::DamageType::Thermal, false).0, 0);
        assert!((physics.chunks[0].voxel_health[0] - 16.0).abs() < 1e-4);
        assert_eq!(physics.carve_chunk_sphere(&grid, center, 0.65, 140.0, crate::voxel::DamageType::Thermal, false).0, 1);
        assert!(physics.chunks.is_empty(), "A second accepted laser tick must finish wounded detached armor");

        let voxels: Vec<_> = (0..64).map(|i| (Vec3::X * (i as f32 - 31.5) * VOXEL_SIZE, VoxelType::Concrete)).collect();
        let mut body = chunk(center, Vec3::ZERO, voxels);
        body.voxel_health = (0..64).map(|i| 10.0 + i as f32 * 0.5).collect();
        let expected: Vec<_> = body.voxels.iter().zip(&body.voxel_health).map(|(&(p, _), &hp)| (body.local_to_world(p), hp)).collect();
        let (a, b) = body.slice_by_plane(Vec3::X, 0.0).unwrap_or_else(|_| panic!("Plane must bisect body"));
        for child in [&a, &b] {
            for (&(p, _), &hp) in child.voxels.iter().zip(&child.voxel_health) {
                let world = child.local_to_world(p);
                let old = expected.iter().find(|(old_world, _)| old_world.distance(world) < 1e-5).unwrap();
                assert_eq!(old.1, hp);
            }
        }
        let mut body = chunk(center, Vec3::ZERO, (0..64).map(|i| (Vec3::X * i as f32 * VOXEL_SIZE, VoxelType::Concrete)).collect());
        body.voxel_health = (0..64).map(|i| 10.0 + i as f32 * 0.5).collect();
        let (children, _, _) = body.fracture();
        assert_eq!(children.iter().map(|c| c.voxels.len()).sum::<usize>(), 64);
        for child in &children {
            assert_eq!(child.voxel_health.len(), child.voxels.len());
            for (&(p, _), &hp) in child.voxels.iter().zip(&child.voxel_health) {
                let world = child.local_to_world(p);
                assert!((hp - (10.0 + world.x)).abs() < 1e-4, "Fracture cannot heal or swap damaged voxels");
            }
        }
    }

    #[test]
    fn island_detachment_preserves_partial_voxel_health() {
        let mut grid = VoxelGrid::new();
        let cell = grid.world_to_voxel(Vec3::new(2.0, 8.0, 2.0));
        grid.set(cell.x, cell.y, cell.z, VoxelType::Concrete);
        grid.set(cell.x + 1, cell.y, cell.z, VoxelType::Concrete);
        assert!(!grid.damage_voxel(cell.x, cell.y, cell.z, 60.0, crate::voxel::DamageType::Thermal));
        assert!(!grid.damage_voxel(cell.x + 1, cell.y, cell.z, 60.0, crate::voxel::DamageType::Thermal));
        let mut islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].voxel_health, vec![10.0, 10.0]);
        let mut physics = PhysicsSystem::new();
        physics.spawn_island_chunk(islands.pop().unwrap(), Vec3::ZERO, Vec3::ZERO);
        let center = physics.chunks[0].position;
        assert_eq!(physics.chunks[0].voxel_health, vec![10.0, 10.0]);
        assert_eq!(physics.carve_chunk_sphere(&grid, center, 0.8, 25.0, crate::voxel::DamageType::Thermal, false).0, 2,
            "Detachment must not heal wounded material before the next accepted hit");
    }

    #[test]
    fn exposed_voxels_are_independent_of_body_frame_offset() {
        let mut voxels = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    let center = Vec3::new(x as f32 - 1.5, y as f32 - 1.5, z as f32 - 1.5) * VOXEL_SIZE;
                    voxels.push((center, VoxelType::Concrete));
                }
            }
        }
        let exposed = DynamicChunk::compute_exposed_voxels(&voxels);
        assert_eq!(exposed.len(), 56, "The inner eight voxels of a 4x4x4 chunk are not exposed");
        for (p, _) in &mut voxels { *p += Vec3::splat(0.125); }
        assert_eq!(DynamicChunk::compute_exposed_voxels(&voxels).len(), 56, "Moving the body origin cannot expose interior voxels");
    }

    #[test]
    fn chunk_raycast_hits_long_beam_end_and_exact_surface() {
        let mut physics = PhysicsSystem::new();
        let voxels = (-20..=20)
            .map(|x| (Vec3::new(x as f32 * VOXEL_SIZE, 0.0, 0.0), VoxelType::SteelRebar))
            .collect();
        physics.chunks.push(chunk(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, voxels));
        let (point, normal) = physics.check_chunk_hit(Vec3::new(9.5, 4.0, 5.0), Vec3::new(9.5, 4.0, -5.0)).expect("Beam end is hittable");
        assert!((point.z - VOXEL_SIZE * 0.5).abs() < 1e-5);
        assert_eq!(normal, Vec3::Z);
    }

    #[test]
    fn chunk_raycast_does_not_hit_empty_space_or_gaps() {
        let mut physics = PhysicsSystem::new();
        physics.chunks.push(chunk(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Concrete)]));
        assert!(physics.check_chunk_hit(Vec3::new(0.6, 4.0, 5.0), Vec3::new(0.6, 4.0, -5.0)).is_none(), "A single voxel cannot block a ray outside its faces");
        physics.chunks[0] = chunk(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, vec![
            (Vec3::new(-2.0, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(2.0, 0.0, 0.0), VoxelType::Concrete),
        ]);
        assert!(physics.check_chunk_hit(Vec3::new(0.0, 4.0, 5.0), Vec3::new(0.0, 4.0, -5.0)).is_none(), "A ray through the gap must miss");
    }

    #[test]
    fn chunk_raycast_rotated_nearest_hit_and_finite_normal() {
        let mut physics = PhysicsSystem::new();
        physics.chunks.push(chunk(Vec3::new(0.0, 4.0, -2.0), Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Concrete)]));
        let angle = std::f32::consts::FRAC_PI_4;
        physics.chunks.push(chunk(Vec3::new(0.0, 4.0, 1.0), Vec3::new(0.0, angle, 0.0), vec![(Vec3::ZERO, VoxelType::Concrete)]));
        let (point, normal) = physics.check_chunk_hit(Vec3::new(0.0, 4.0, 5.0), Vec3::new(0.0, 4.0, -5.0)).unwrap();
        assert!((point.z - (1.0 + VOXEL_SIZE * 0.5 * 2.0f32.sqrt())).abs() < 1e-5);
        assert!((normal.length() - 1.0).abs() < 1e-5);
        assert!(normal.z > 0.7);
        let (_, inside_normal) = physics.check_chunk_hit(Vec3::new(0.0, 4.0, 1.0), Vec3::new(0.0, 4.0, 2.0)).unwrap();
        assert!((inside_normal.length() - 1.0).abs() < 1e-5, "Starting inside rubble still needs a valid impact normal");
    }

    #[test]
    fn chunk_projectile_sweep_accounts_for_radius_and_conservative_corners() {
        let mut physics = PhysicsSystem::new();
        physics.chunks.push(chunk(Vec3::new(0.0, 4.0, 0.0), Vec3::ZERO, vec![(Vec3::ZERO, VoxelType::Concrete)]));
        let start = Vec3::new(0.6, 4.0, 5.0);
        let end = Vec3::new(0.6, 4.0, -5.0);
        assert!(physics.check_chunk_hit(start, end).is_none());
        assert!(physics.check_chunk_hit_radius(start, end, 0.0).is_none());
        assert!(physics.check_chunk_hit_radius(start, end, 0.3).is_none());
        let (point, normal) = physics.check_chunk_hit_radius(start, end, 0.4).expect("Projectile surface reaches the voxel");
        assert!((point.z - 0.65).abs() < 1e-5);
        assert_eq!(normal, Vec3::Z);

        // Inflated cubes intentionally include corner contacts outside the true sphere.
        let corner_start = Vec3::new(0.55, 4.55, 5.0);
        let corner_end = Vec3::new(0.55, 4.55, -5.0);
        assert!(physics.check_chunk_hit(corner_start, corner_end).is_none());
        assert!(physics.check_chunk_hit_radius(corner_start, corner_end, 0.35).is_some());

        let inside = Vec3::new(0.6, 4.0, 0.0);
        let (point, normal) = physics.check_chunk_hit_radius(inside, inside + Vec3::X, 0.4).unwrap();
        assert_eq!(point, inside, "Starting inside the inflated voxel contacts immediately");
        assert_eq!(normal, -Vec3::X);
    }

    #[test]
    fn test_rotate_vec3_identity() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let rotated = rotate_vec3(v, Vec3::ZERO);
        assert!((rotated - v).length() < 0.001);
    }

    #[test]
    fn test_rotate_vec3_90_degrees_yaw() {
        let v = Vec3::new(1.0, 0.0, 0.0);
        let rotated = rotate_vec3(v, Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0));
        assert!((rotated.z - (-1.0)).abs() < 0.001, "Expected z=-1, got {:?}", rotated);
        assert!(rotated.x.abs() < 0.001);
    }

    #[test]
    fn test_kinetic_blast_pushes_chunks() {
        let mut phys = PhysicsSystem::new();
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 2.0, 3.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: true,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        phys.apply_kinetic_blast(Vec3::new(0.0, 2.0, 0.0), Vec3::Z, 10.0, 50.0);
        let chunk = &phys.chunks[0];
        assert!(chunk.velocity.z > 1.0, "Chunk should be shoved forward along +Z");
        assert!(!chunk.settled, "Chunk should be unset on blast");
    }

    #[test]
    fn test_beam_shockwave_pushes_radially() {
        let mut phys = PhysicsSystem::new();
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(2.0, 0.0, 5.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 2.0,
            life: 0.0,
            settled: true,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        phys.apply_beam_shockwave(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), 5.0, 30.0);
        let chunk = &phys.chunks[0];
        assert!(chunk.velocity.x > 0.5, "Chunk should be pushed radially outward (+X)");
    }

    #[test]
    fn test_vortex_suction_attracts_and_swirls() {
        let mut phys = PhysicsSystem::new();
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(4.0, 2.0, 0.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: true,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        // Singularity at origin (0, 2, 0)
        phys.apply_vortex_suction(Vec3::new(0.0, 2.0, 0.0), 10.0, 40.0, 0.1);
        let chunk = &phys.chunks[0];
        assert!(chunk.velocity.x < 0.0, "Chunk should be pulled towards origin (-X)");
        assert!(chunk.velocity.z.abs() > 0.0, "Chunk should experience orbital swirl");
        assert!(!chunk.settled);
    }

    #[test]
    fn test_radial_blast_pushes_chunks_and_debris() {
        let mut phys = PhysicsSystem::new();
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 2.0, 3.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: true,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        // Detonate radial blast at (0, 2, 0) with radius 8.0, force 60.0
        phys.apply_radial_blast(Vec3::new(0.0, 2.0, 0.0), 8.0, 60.0);
        let chunk = &phys.chunks[0];
        assert!(chunk.velocity.z > 1.0, "Radial blast must push chunk outward along +Z");
        assert!(chunk.velocity.y > 0.0, "Radial blast should apply vertical loft");
        assert!(!chunk.settled);
    }

    #[test]
    fn test_chunk_fracture_subdivision_preserves_voxels() {
        // Create large chunk of 80 voxels
        let mut voxels = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..5 {
                    voxels.push((Vec3::new(x as f32 * 0.5, y as f32 * 0.5, z as f32 * 0.5), VoxelType::Concrete));
                }
            }
        }
        assert_eq!(voxels.len(), 80);

        let aabb = DynamicChunk::compute_aabb(&voxels);
        let chunk = DynamicChunk {
            position: Vec3::new(0.0, 10.0, 0.0),
            velocity: Vec3::new(0.0, -15.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 192.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        };

        let (sub_chunks, _exp, _core) = chunk.fracture();
        assert!(sub_chunks.len() >= 2 && sub_chunks.len() <= 4, "Must subdivide into 2-4 sub-chunks, got {}", sub_chunks.len());

        let total_sub_voxels: usize = sub_chunks.iter().map(|c| c.voxels.len()).sum();
        assert_eq!(total_sub_voxels, 80, "Total voxel count must be preserved across fractured sub-chunks");
        for sub in &sub_chunks {
            assert!(!sub.voxels.is_empty(), "Sub-chunk must not be empty");
            assert!(sub.velocity.length() > 0.1, "Sub-chunk must have scatter velocity");
        }
    }

    #[test]
    fn fracture_preserves_accreted_mass_in_material_proportions() {
        let voxels: Vec<_> = (0..48).map(|i| {
            let material = if i < 24 { VoxelType::Concrete } else { VoxelType::HeavyArmor };
            (Vec3::X * i as f32 * VOXEL_SIZE, material)
        }).collect();
        let material_mass: f32 = voxels.iter().map(|(_, material)| material.properties().density).sum();
        let mut parent = chunk(Vec3::new(0.0, 20.0, 0.0), Vec3::ZERO, voxels);
        // Pulverizing a floor adds mass without adding voxels to the slab's geometry.
        parent.mass = material_mass + 500.0;
        let parent_mass = parent.mass;

        let (children, _, _) = parent.fracture();
        assert_eq!(children.len(), 2);
        let child_mass: f32 = children.iter().map(|child| child.mass).sum();
        assert!((child_mass - parent_mass).abs() < 1e-3,
            "Fracture must retain accreted mass: parent {parent_mass}, children {child_mass}");
        for child in children {
            let child_material_mass: f32 = child.voxels.iter().map(|(_, material)| material.properties().density).sum();
            let expected_share = child_material_mass / material_mass;
            assert!((child.mass / parent_mass - expected_share).abs() < 1e-6,
                "Each piece must carry its material-density share of the full parent mass");
        }
    }

    #[test]
    fn test_small_chunk_settles_on_ground_without_deletion() {
        let mut phys = PhysicsSystem::new();
        // Create small chunk with 15 voxels
        let voxels = vec![(Vec3::ZERO, VoxelType::Concrete); 15];
        let aabb = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 0.2, 0.0),
            velocity: Vec3::new(0.0, -1.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 30.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Update physics - chunk hits ground at low speed
        let _ = phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));

        assert_eq!(phys.chunks.len(), 1, "Small chunk MUST NOT be deleted from the world");
        assert!(phys.chunks[0].settled, "Small chunk must settle into permanent rubble on the ground");
        assert_eq!(phys.chunks[0].voxels.len(), 15, "Chunk voxels must be retained completely");
    }

    #[test]
    fn test_smoke_particle_bounds_and_transparency() {
        let mut phys = PhysicsSystem::new();
        // Trigger explosion with radius 3.5
        phys.add_explosion_effects(Vec3::new(10.0, 5.0, 10.0), 3.5, 25.0, Color::new(1.0, 0.5, 0.1, 1.0), 12);

        assert!(!phys.smoke.is_empty(), "Explosion should create smoke particles");
        assert!(phys.smoke.len() <= 5, "Smoke count must be modest, got {}", phys.smoke.len());

        for sm in &phys.smoke {
            assert!(sm.color.a <= 0.16, "Smoke alpha must be subtle wisps (<= 0.16), got {}", sm.color.a);
            assert!(sm.size <= 0.15, "Initial smoke size must be small (<= 0.15), got {}", sm.size);
            assert!(sm.max_size <= 0.45, "Max smoke size must not obscure view (<= 0.45), got {}", sm.max_size);
            assert!(sm.life <= 0.75, "Smoke lifetime must be short (<= 0.75s), got {}", sm.life);
        }

        // Test smooth linear expansion without exponential inflation
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();
        let init_size = phys.smoke[0].size;
        let max_size = phys.smoke[0].max_size;
        for _ in 0..5 {
            phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        }
        if !phys.smoke.is_empty() {
            let expanded_size = phys.smoke[0].size;
            assert!(expanded_size >= init_size, "Smoke must expand over time");
            assert!(expanded_size <= max_size, "Smoke size must NEVER exceed max_size, got {}", expanded_size);
        }
    }

    #[test]
    fn test_impact_decal_lifecycle_and_capping() {
        let mut phys = PhysicsSystem::new();
        phys.spawn_decal(
            Vec3::new(5.0, 2.0, 5.0),
            Vec3::Y,
            2.0,
            Color::new(0.1, 0.1, 0.1, 0.9),
            DecalKind::Scorch,
        );

        assert_eq!(phys.decals.len(), 1);
        let d = &phys.decals[0];
        assert_eq!(d.kind, DecalKind::Scorch);
        assert!(d.position.y > 2.0, "Decal must stand off surface normal to avoid Z-fighting");

        // Spawn excess decals to test capping at PhysicsSystem::MAX_DECALS
        for _ in 0..250 {
            phys.spawn_decal(Vec3::ZERO, Vec3::Y, 1.0, Color::new(1.0, 0.5, 0.2, 0.9), DecalKind::PlasmaBurn);
        }
        assert!(phys.decals.len() <= PhysicsSystem::MAX_DECALS, "Decal count must be capped at {}, got {}", PhysicsSystem::MAX_DECALS, phys.decals.len());

        // Update physics and check decay
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();
        let init_life = phys.decals[0].life;
        phys.update(0.1, &mut grid, &audio, Vec3::ZERO);
        assert!(phys.decals[0].life < init_life, "Decal life must decay with dt");
    }

    #[test]
    fn test_anchored_decal_crumbles_when_voxel_destroyed() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        let vpos = IVec3::new(16, 5, 16);
        grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Concrete);
        let wall_pos = grid.voxel_to_world(vpos);

        phys.spawn_decal_anchored(
            wall_pos,
            Vec3::X,
            1.5,
            Color::new(0.2, 0.2, 0.2, 0.9),
            DecalKind::Scorch,
            Some(vpos),
        );
        assert_eq!(phys.decals.len(), 1);

        // Voxel remains solid: decal stays alive
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert_eq!(phys.decals.len(), 1, "Decal must remain while wall is intact");

        // Voxel is destroyed/severed: decal must crumble into tumbling micro-spall debris
        grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Air);
        let debris_before = phys.debris.len();
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert_eq!(phys.decals.len(), 0, "Decal must crumble and be removed when underlying voxel is destroyed");
        assert!(phys.debris.len() > debris_before, "Crumbling decal must spawn tumbling micro-spall chips");
    }

    #[test]
    fn test_unanchored_decal_crumbles_when_wall_falls_away() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        let vpos = IVec3::new(20, 8, 20);
        grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Concrete);
        let wall_pos = grid.voxel_to_world(vpos);

        // Spawn unanchored decal on the wall surface
        phys.spawn_decal(
            wall_pos,
            Vec3::Y,
            1.5,
            Color::new(0.3, 0.3, 0.3, 0.9),
            DecalKind::PlasmaBurn,
        );
        assert_eq!(phys.decals.len(), 1);

        // While wall is solid, decal stays alive
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert_eq!(phys.decals.len(), 1);

        // Wall falls away or is pulverized: unanchored decal must crumble into micro-spall chips
        grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Air);
        let debris_before = phys.debris.len();
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert_eq!(phys.decals.len(), 0, "Unanchored decal must be removed when wall falls away");
        assert!(phys.debris.len() > debris_before, "Crumbling decal must spawn debris chips");
    }

    #[test]
    fn test_settled_chunk_wakes_up_when_floor_destroyed() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Build a floor at y = 10 (above bedrock which is y <= 0.50)
        let floor_vpos = IVec3::new(15, 10, 15);
        grid.set(floor_vpos.x, floor_vpos.y, floor_vpos.z, VoxelType::Concrete);
        let floor_pos = grid.voxel_to_world(floor_vpos);

        // Place a settled chunk resting directly on this upper floor
        let chunk_pos = floor_pos + Vec3::new(0.0, VOXEL_SIZE, 0.0);
        let mut chunk = DynamicChunk {
            position: chunk_pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Brick)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 12.0,
            life: 0.0,
            settled: true,
            local_aabb: (-Vec3::splat(VOXEL_SIZE * 0.5), Vec3::splat(VOXEL_SIZE * 0.5)),
        };
        chunk.local_aabb = DynamicChunk::compute_aabb(&chunk.voxels);
        phys.chunks.push(chunk);

        // Update while floor is intact: chunk remains settled
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert!(phys.chunks[0].settled, "Chunk must remain settled while floor is intact");

        // Destroy floor beneath chunk (simulating floor collapse or detached island)
        grid.set(floor_vpos.x, floor_vpos.y, floor_vpos.z, VoxelType::Air);

        // Update physics: chunk must immediately wake up and start plummeting downward
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert!(!phys.chunks[0].settled, "Settled chunk must wake up when supporting floor is destroyed");
        assert!(phys.chunks[0].velocity.y < 0.0, "Woken chunk must have downward velocity");
    }

    #[test]
    fn regression_resting_fragment_settles_on_supported_upper_floor() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();
        let floor = IVec3::new(96, 12, 96);
        grid.set(floor.x, floor.y, floor.z, VoxelType::HeavyArmor);
        let floor_pos = grid.voxel_to_world(floor);
        let voxels = vec![(Vec3::ZERO, VoxelType::Brick)];
        let bounds = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk::new(
            floor_pos + Vec3::Y * VOXEL_SIZE,
            Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            voxels, 12.0, 0.0, false, bounds,
        ));

        for _ in 0..100 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }
        assert_eq!(grid.get(floor.x, floor.y, floor.z), VoxelType::HeavyArmor);
        assert!(phys.chunks[0].lowest_world_y() > 6.0, "The fragment is resting on the upper floor");
        assert!(phys.chunks[0].settled, "A slow fragment supported by an upper floor must not remain airborne forever");

        grid.set(floor.x, floor.y, floor.z, VoxelType::Air);
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        assert!(!phys.chunks[0].settled, "Removing upper-floor support must wake the fragment");
        assert!(phys.chunks[0].velocity.y < 0.0);
    }

    #[test]
    fn regression_dynamic_rubble_lands_on_outer_foundation_and_falls_beyond_it() {
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();
        for (x, z, should_land) in [(200.0, 200.0, true), (-200.0, 200.0, true), (241.0, 200.0, false)] {
            let mut phys = PhysicsSystem::new();
            let voxels = vec![
                (Vec3::new(-0.25, 0.0, -0.25), VoxelType::Brick),
                (Vec3::new(0.25, 0.0, -0.25), VoxelType::Brick),
                (Vec3::new(-0.25, 0.0, 0.25), VoxelType::Brick),
                (Vec3::new(0.25, 0.0, 0.25), VoxelType::Brick),
            ];
            let bounds = DynamicChunk::compute_aabb(&voxels);
            phys.chunks.push(DynamicChunk::new(
                Vec3::new(x, 4.0, z), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
                voxels, 48.0, 0.0, false, bounds,
            ));
            for _ in 0..200 {
                phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
            }
            if should_land {
                assert_eq!(phys.chunks.len(), 1, "Visible foundation must retain its rubble at ({x}, {z})");
                assert!(phys.chunks[0].settled, "Rubble must settle on the visible foundation at ({x}, {z})");
                assert!((phys.chunks[0].lowest_world_y() - VOXEL_SIZE).abs() < 0.05);
            } else {
                assert!(phys.chunks.is_empty(), "Rubble outside the slab must fall through the void boundary");
            }
        }
    }

    #[test]
    fn regression_rubble_straddling_a_gap_settles_on_its_contact_feet() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        // Two rubble columns stand on bedrock with an empty center column between them.
        for x in [0.25, 1.25] {
            let voxels: Vec<_> = (0..8).map(|y| (Vec3::new(0.0, y as f32 * VOXEL_SIZE - 1.75, 0.0), VoxelType::Brick)).collect();
            let bounds = DynamicChunk::compute_aabb(&voxels);
            phys.chunks.push(DynamicChunk::new(
                Vec3::new(x, 2.5, 0.25), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
                voxels, 96.0, 0.0, false, bounds,
            ));
            phys.settle_chunk(phys.chunks.len() - 1);
        }
        assert_eq!(phys.rubble_height_at(0.75, 0.25), VOXEL_SIZE);
        let voxels = vec![
            (Vec3::new(-0.5, 0.0, 0.0), VoxelType::Brick),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::Brick),
            (Vec3::new(-0.25, 0.5, 0.0), VoxelType::Brick),
            (Vec3::new(0.25, 0.5, 0.0), VoxelType::Brick),
        ];
        let bounds = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk::new(
            Vec3::new(0.75, 4.75, 0.25), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            voxels, 48.0, 0.0, false, bounds,
        ));
        for _ in 0..100 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }
        assert!(phys.chunks[2].lowest_world_y() > 4.0);
        assert!(phys.chunks[2].settled, "Real contact feet must support a bridge even when its center column is empty");
    }

    #[test]
    fn test_chunk_impact_trauma_distance_attenuation() {
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Close impact: Chunk lands 3m from camera
        let mut phys_near = PhysicsSystem::new();
        phys_near.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 0.6, 3.0),
            velocity: Vec3::new(0.0, -12.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete); 20],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 48.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });
        let near_trauma = phys_near.update(0.016, &mut grid, &audio, Vec3::ZERO);

        // 2. Distant impact: Chunk lands 80m from camera (sniping distance)
        let mut phys_far = PhysicsSystem::new();
        phys_far.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 0.6, 80.0),
            velocity: Vec3::new(0.0, -12.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete); 20],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 48.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });
        let far_trauma = phys_far.update(0.016, &mut grid, &audio, Vec3::ZERO);

        assert!(near_trauma > 0.15, "Close chunk impact must produce palpable trauma, got {}", near_trauma);
        assert!(far_trauma < 0.02, "Distant chunk impact at 80m must have near-zero trauma for sniper stability, got {}", far_trauma);
        assert!(far_trauma < near_trauma * 0.10, "Distant trauma must be <10% of near trauma");
    }

    #[test]
    fn test_volumetric_spall_and_shockwaves_on_decal_spawn() {
        let mut phys = PhysicsSystem::new();
        assert!(phys.debris.is_empty());
        assert!(phys.sparks.is_empty());
        assert!(phys.shockwaves.is_empty());

        // Spawn a high-energy kinetic impact
        phys.spawn_decal(
            Vec3::new(10.0, 5.0, 10.0),
            Vec3::Y,
            2.0,
            Color::new(0.6, 0.6, 0.6, 1.0),
            DecalKind::KineticImpact,
        );

        // Verify 3D volumetric physical destruction phenomena are paired with the impact
        assert!(!phys.debris.is_empty(), "Must spawn dynamic 3D tumbling micro-spall chips");
        assert!(!phys.sparks.is_empty(), "Must spawn incandescent ricochet sparks");
        assert!(!phys.shockwaves.is_empty(), "High energy kinetic impact must spawn an expanding 3D shockwave");

        // Verify debris has 3D tumbling angular velocity
        let debris_chip = &phys.debris[0];
        assert!(debris_chip.angular_velocity.length_squared() > 0.1, "Micro-spall chips must have 3D tumbling spin");
        assert!(debris_chip.size > 0.0, "Debris must have non-zero 3D size");

        // Verify shockwave ring has expansion speed
        let shockwave = &phys.shockwaves[0];
        assert!(shockwave.speed > 10.0, "Shockwave ring must expand radially");
        assert!(shockwave.max_radius > 1.5, "Shockwave max radius must be physical");
    }

    #[test]
    fn test_dynamic_lighting_collection_and_intensity() {
        let mut phys = PhysicsSystem::new();
        let mut lights = Vec::new();
        phys.collect_dynamic_lights(&mut lights);
        assert!(lights.is_empty(), "No lights when system is idle");

        // Spawn shockwave ring
        phys.shockwaves.push(ShockwaveRing {
            position: Vec3::new(5.0, 2.0, 5.0),
            normal: Vec3::Y,
            radius: 0.5,
            max_radius: 5.0,
            speed: 20.0,
            color: Color::new(1.0, 0.5, 0.1, 1.0),
            thickness: 0.2,
        });

        phys.collect_dynamic_lights(&mut lights);
        assert_eq!(lights.len(), 1, "Shockwave must emit a dynamic point light");
        assert!(lights[0].radius > 3.0, "Dynamic light must illuminate a radius");
        assert!(lights[0].intensity > 0.5, "Dynamic light must have radiant intensity");
    }

    #[test]
    fn test_rubble_heightmap_initialization_and_stamping() {
        let mut phys = PhysicsSystem::new();
        // 1. Initial state must be bedrock elevation (0.50)
        assert_eq!(phys.rubble_height_at(0.0, 0.0), VOXEL_SIZE);
        assert_eq!(phys.rubble_height_at(10.0, -10.0), VOXEL_SIZE);

        // 2. Settle a chunk at (2.0, 1.5, 3.0)
        let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(2.0, 1.5, 3.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });
        phys.settle_chunk(0);

        // Top of chunk is 1.5 + 0.25 = 1.75
        let h = phys.rubble_height_at(2.0, 3.0);
        assert!((h - 1.75).abs() < 0.01, "Heightmap at chunk position must be 1.75, got {}", h);
        // Unoccupied cell must remain at 0.50
        assert_eq!(phys.rubble_height_at(0.0, 0.0), VOXEL_SIZE);
    }

    #[test]
    fn test_dynamic_chunk_stacks_on_settled_rubble() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. First chunk settled on floor with top at y = 1.75
        let floor_vpos = grid.world_to_voxel(Vec3::new(0.0, 1.0, 0.0));
        grid.set(floor_vpos.x, floor_vpos.y, floor_vpos.z, VoxelType::Concrete);

        let vox1 = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb1 = DynamicChunk::compute_aabb(&vox1);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 1.5, 0.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox1,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb1,
        });
        phys.settle_chunk(0);

        // 2. Second chunk dropped from above directly onto first chunk
        let vox2 = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb2 = DynamicChunk::compute_aabb(&vox2);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 3.0, 0.0),
            velocity: Vec3::new(0.0, -1.0, 0.0), // Low impact speed
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox2,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb2,
        });

        // Step physics until Chunk 2 settles on top of Chunk 1
        for frame in 0..50 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
            println!("Frame {}: y={:.3}, vy={:.3}, settled={}", frame, phys.chunks[1].position.y, phys.chunks[1].velocity.y, phys.chunks[1].settled);
            if phys.chunks[1].settled {
                break;
            }
        }

        // Chunk 2 must settle on top of Chunk 1 (y >= 1.75), NOT at bedrock (0.50)
        assert!(phys.chunks[1].settled, "Second chunk must settle on top of first chunk");
        assert!(phys.chunks[1].position.y >= 1.75, "Chunk must rest at or above 1.75, got {}", phys.chunks[1].position.y);
        assert!(phys.chunks[1].lowest_world_y() >= 1.50);
    }

    #[test]
    fn test_off_center_rubble_impact_generates_angular_torque() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Create a settled rubble step on the right (+X) resting on concrete floor
        let floor_vpos = grid.world_to_voxel(Vec3::new(1.0, 1.0, 0.0));
        grid.set(floor_vpos.x, floor_vpos.y, floor_vpos.z, VoxelType::Concrete);

        let vox1 = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb1 = DynamicChunk::compute_aabb(&vox1);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(1.0, 1.5, 0.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox1,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb1,
        });
        phys.settle_chunk(0);

        // 2. Drop a wide 2-meter chunk centered at X=0, with zero angular velocity
        let mut wide_vox = Vec::new();
        for x in -2..=2 {
            wide_vox.push((Vec3::new(x as f32 * 0.5, 0.0, 0.0), VoxelType::Concrete));
        }
        let aabb2 = DynamicChunk::compute_aabb(&wide_vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, 2.0, 0.0),
            velocity: Vec3::new(0.0, -4.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: wide_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 25.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb2,
        });

        let _ = phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

        let chunk = &phys.chunks[1];
        // Right side hit first at X=1.0, imparting torque around Z axis!
        assert!(chunk.angular_velocity.length_squared() > 0.2, "Off-center impact must generate angular tumbling, got {:?}", chunk.angular_velocity);
    }

    #[test]
    fn test_pancake_collapse_pulverizes_floor_into_subchunks_and_debris() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Build a solid brick floor at y = 6 (world y = 3.0)
        for x in 40..50 {
            for z in 40..50 {
                grid.set(x, 6, z, VoxelType::Brick);
            }
        }
        let initial_solid = grid.solid_count();

        // 2. Heavy falling chunk descending rapidly onto the floor
        let mut heavy_vox = Vec::new();
        for x in 0..4 {
            for z in 0..4 {
                heavy_vox.push((Vec3::new(x as f32 * 0.5, 0.0, z as f32 * 0.5), VoxelType::HeavyArmor));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&heavy_vox);
        let floor_world = grid.voxel_to_world(IVec3::new(45, 6, 45));

        phys.chunks.push(DynamicChunk {
            position: floor_world + Vec3::new(0.0, 0.8, 0.0),
            velocity: Vec3::new(0.0, -12.0, 0.0), // High downward speed
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: heavy_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 500.0, // Colossal mass
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        let initial_chunk_count = phys.chunks.len();
        let initial_debris_count = phys.debris.len();

        let _ = phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        // Verify floor was pulverized
        assert!(grid.solid_count() < initial_solid, "Intermediate floor must be pulverized");
        // Verify dynamic rubble sub-chunks spawned
        assert!(phys.chunks.len() > initial_chunk_count, "Pulverization must spawn new dynamic rubble sub-chunks");
        // Verify debris particles spawned with high lateral speed
        assert!(phys.debris.len() > initial_debris_count, "Pulverization must spawn tumbling DebrisParticle instances");
        // Verify debris has brick color
        let debris_item = phys.debris.last().unwrap();
        assert_eq!(debris_item.color, VoxelType::Brick.color());
        // Verify dust smoke particles spawned
        assert!(!phys.smoke.is_empty(), "Must spawn billowing dust particles");
    }

    #[test]
    fn test_debris_graceful_accelerated_decay() {
        let mut phys = PhysicsSystem::new();
        // Spawn 2600 debris particles (exceeding SOFT_DEBRIS = 2400)
        for i in 0..2600 {
            phys.debris.push(DebrisParticle {
                position: Vec3::new(i as f32 * 0.01, 10.0, 0.0),
                velocity: Vec3::ZERO,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                size: 0.1,
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                life: 5.0,
                max_life: 5.0,
                bounces: 0,
            });
        }
        assert_eq!(phys.debris.len(), 2600);
        let pre_alpha = phys.debris[0].life / phys.debris[0].max_life;

        phys.enforce_particle_caps();

        // Length should still be 2600 (no hard popping below MAX_DEBRIS = 3000)
        assert_eq!(phys.debris.len(), 2600, "Debris count should not pop below max ceiling");
        // Oldest excess particles (first 200) must have accelerated lifetime (<= 0.15s)
        assert!(phys.debris[0].life <= 0.15, "Oldest debris life must be clamped for accelerated decay");
        // But their rendered alpha ratio must remain continuous with zero jump
        let post_alpha = phys.debris[0].life / phys.debris[0].max_life;
        assert!((post_alpha - pre_alpha).abs() < 0.01, "Alpha must remain continuous during accelerated decay");
    }

    #[test]
    fn test_dynamic_wind_gust_and_thermal() {
        let mut phys = PhysicsSystem::new();
        phys.elapsed_time = 0.0;
        let wind0 = phys.get_dynamic_wind(Vec3::new(10.0, 2.0, 10.0));
        phys.elapsed_time = 2.5;
        let wind1 = phys.get_dynamic_wind(Vec3::new(10.0, 2.0, 10.0));
        assert!(wind0 != wind1, "Wind must vary dynamically over elapsed time");

        // Thermal updraft should be stronger near ground than at high altitude
        let ground_wind = phys.get_dynamic_wind(Vec3::new(10.0, 1.0, 10.0));
        let high_wind = phys.get_dynamic_wind(Vec3::new(10.0, 40.0, 10.0));
        assert!(ground_wind.y > high_wind.y, "Thermal updraft must be stronger near ground");
    }

    #[test]
    fn test_spark_ricochets_off_vertical_concrete_wall() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();

        // Concrete wall at x = 20, y = 5, z = 20
        grid.set(20, 5, 20, VoxelType::Concrete);
        let wall_center = grid.voxel_to_world(IVec3::new(20, 5, 20));

        // High-speed spark traveling towards the wall (+X direction)
        phys.sparks.push(SparkParticle {
            position: wall_center - Vec3::new(0.20, 0.0, 0.0),
            velocity: Vec3::new(15.0, 0.0, 0.0),
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 1.0,
            max_life: 1.0,
        });

        let audio = crate::audio::SoundSystem::empty();
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        let sp = &phys.sparks[0];
        // Spark must bounce backward (-X) and not enter or penetrate the wall
        assert!(sp.velocity.x < 0.0, "Spark velocity.x must reflect to negative, got {}", sp.velocity.x);
        assert!(sp.position.x < wall_center.x, "Spark must remain outside wall");
    }

    #[test]
    fn test_debris_ricochets_off_settled_rubble_slope() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();

        // Stamp settled rubble mound at (10.0, 10.0) up to y = 3.50
        if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(10.0, 10.0) {
            let idx = gx * crate::voxel::GRID_Z + gz;
            phys.rubble_height_grid[idx] = 3.50;
        }

        // Falling micro-spall chip directly above rubble mound
        phys.debris.push(DebrisParticle {
            position: Vec3::new(10.0, 3.65, 10.0),
            velocity: Vec3::new(0.0, -8.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.10,
            color: Color::new(0.5, 0.5, 0.5, 1.0),
            life: 2.0,
            max_life: 2.0,
            bounces: 0,
        });

        let audio = crate::audio::SoundSystem::empty();
        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        let p = &phys.debris[0];
        // Debris must bounce on top of rubble (y >= 3.50 + size*0.5), not fall through to bedrock (0.50)
        assert!(p.position.y >= 3.50, "Debris must bounce above rubble surface, got {}", p.position.y);
        assert!(p.velocity.y >= 0.0, "Debris velocity.y must invert upon bouncing on rubble");
    }

    #[test]
    fn test_blackbody_color_ramp_thermodynamic_progression() {
        // 1. Fresh white-hot (t = 1.00)
        let c_white_hot = blackbody_color_ramp(1.0);
        assert!(c_white_hot.r >= 0.99 && c_white_hot.g >= 0.90 && c_white_hot.b >= 0.80);

        // 2. Incandescent orange (t = 0.70)
        let c_orange = blackbody_color_ramp(0.70);
        assert!(c_orange.r >= 0.95 && c_orange.g < 0.65 && c_orange.b < 0.20);

        // 3. Cherry red (t = 0.35)
        let c_red = blackbody_color_ramp(0.35);
        assert!(c_red.r >= 0.80 && c_red.g < 0.25 && c_red.b < 0.15);

        // 4. Cooled dark slag crust (t = 0.05)
        let c_slag = blackbody_color_ramp(0.05);
        assert!(c_slag.r < 0.25 && c_slag.g < 0.20 && c_slag.b < 0.20);
        assert!(c_slag.a > 0.50, "Slag crust must retain opacity");

        // 5. Ambient ash fadeout (t = 0.00)
        let c_ash = blackbody_color_ramp(0.0);
        assert_eq!(c_ash.a, 0.0, "Zero-age decal must fully fade alpha");
    }

    #[test]
    fn test_shockwave_normal_basis_orthogonality() {
        let normals = [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::new(0.6, 0.0, 0.8).normalize(),
            Vec3::new(0.5, 0.5, 0.707).normalize(),
        ];

        for &n in &normals {
            let tangent = if n.y.abs() > 0.92 {
                Vec3::X
            } else {
                n.cross(Vec3::Y).normalize_or_zero()
            };
            let bitangent = n.cross(tangent).normalize_or_zero();

            // Tangent and bitangent must be orthogonal to normal and each other
            assert!(tangent.dot(n).abs() < 1e-5, "Tangent must be orthogonal to normal");
            assert!(bitangent.dot(n).abs() < 1e-5, "Bitangent must be orthogonal to normal");
            assert!(tangent.dot(bitangent).abs() < 1e-5, "Tangent and bitangent must be orthogonal");
            assert!((tangent.length() - 1.0).abs() < 1e-4, "Tangent must be unit length");
            assert!((bitangent.length() - 1.0).abs() < 1e-4, "Bitangent must be unit length");
        }
    }

    #[test]
    fn test_push_oriented_cube_mesh_indices_count() {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        push_cube_mesh(&mut mesh, Vec3::ZERO, Vec3::ONE, Color::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(mesh.vertices.len(), 24, "A cube must generate exactly 24 vertices");
        assert_eq!(mesh.indices.len(), 36, "A cube must generate exactly 36 indices (6 quads * 6 indices)");
        for &idx in &mesh.indices {
            assert!((idx as usize) < mesh.vertices.len(), "Index out of range");
        }
    }

    #[test]
    fn test_dynamic_chunk_sub_blocks_partitioning() {
        let mut vox = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    vox.push((Vec3::new(x as f32 * 0.5, y as f32 * 0.5, z as f32 * 0.5), VoxelType::Concrete));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&vox);
        let chunk = DynamicChunk {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 50.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        };
        let sub_blocks = chunk.sub_blocks();
        assert!(sub_blocks.len() >= 2, "64 voxels should partition into multiple sub-blocks");
        let total_voxels_in_blocks: usize = sub_blocks.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(total_voxels_in_blocks, 64, "All voxels must be covered by sub-blocks");
    }

    #[test]
    fn test_pancake_collapse_pulverizes_floor_when_chunk_is_rotated() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Build a solid brick floor at y = 6 (world y = 3.0)
        for x in 40..50 {
            for z in 40..50 {
                grid.set(x, 6, z, VoxelType::Brick);
            }
        }
        let initial_solid = grid.solid_count();

        // 2-layer heavy chunk descending rapidly, flipped upside down (pitch = PI)
        let mut heavy_vox = Vec::new();
        for x in 0..4 {
            for y in 0..2 {
                for z in 0..4 {
                    heavy_vox.push((Vec3::new(x as f32 * 0.5, y as f32 * 0.5, z as f32 * 0.5), VoxelType::HeavyArmor));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&heavy_vox);
        let floor_world = grid.voxel_to_world(IVec3::new(45, 6, 45));

        phys.chunks.push(DynamicChunk {
            position: floor_world + Vec3::new(0.0, 1.2, 0.0),
            velocity: Vec3::new(0.0, -12.0, 0.0),
            rotation: Vec3::new(std::f32::consts::PI, 0.0, 0.0), // Upside down!
            rot_mat: Mat3::from_euler(Vec3::new(std::f32::consts::PI, 0.0, 0.0)),
            angular_velocity: Vec3::ZERO,
            voxels: heavy_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 500.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        let _ = phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        assert!(grid.solid_count() < initial_solid, "Intermediate floor must be pulverized even when chunk is inverted");
    }

    #[test]
    fn test_debris_pool_scales_to_3000_with_tumbling_bouncing_and_rubble_sleep() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Populate rubble heightmap with an elevated mound in the center
        for x in -8..=8 {
            for z in -8..=8 {
                let dist = ((x * x + z * z) as f32).sqrt();
                let h = (4.0 - dist * 0.4).max(0.50);
                phys.rubble_heightmap_set(x as f32 * 0.5, z as f32 * 0.5, h);
            }
        }
        phys.rubble_dirty = false;

        // Spawn exactly 3,000 active physical debris particles above the rubble mound
        for i in 0..3000 {
            let offset_x = (i % 25) as f32 * 0.2 - 2.5;
            let offset_z = ((i / 25) % 25) as f32 * 0.2 - 2.5;
            phys.debris.push(DebrisParticle {
                position: Vec3::new(offset_x, 5.0 + (i % 10) as f32 * 0.2, offset_z),
                velocity: Vec3::new(
                    ((i % 7) as f32 - 3.0) * 1.5,
                    -4.0 - (i % 5) as f32,
                    (((i / 7) % 7) as f32 - 3.0) * 1.5,
                ),
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::new(8.0, -12.0, 6.0),
                size: 0.15,
                color: Color::new(0.6, 0.55, 0.5, 1.0),
                life: 30.0,
                max_life: 30.0,
                bounces: 0,
            });
        }
        assert_eq!(phys.debris.len(), 3000);

        // Step 1 frame: verify zero abrupt truncation at 3,000 particles
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        assert_eq!(phys.debris.len(), 3000, "Debris count must maintain 3000 without abrupt truncation");

        // Advance 99 more simulation frames to let particles bounce, tumble, and settle onto rubble
        for _ in 0..99 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }

        assert_eq!(phys.debris.len(), PhysicsSystem::SOFT_DEBRIS, "Excess debris must gracefully decay to SOFT_DEBRIS (2400)");
        let mut sleeping_count = 0;
        for (idx, p) in phys.debris.iter().enumerate() {
            assert!(!p.position.is_nan(), "Debris {} position has NaN", idx);
            assert!(!p.velocity.is_nan(), "Debris {} velocity has NaN", idx);
            assert!(
                p.position.y >= 0.45,
                "Debris {} must not penetrate below bedrock y=0.50 (got y={})",
                idx, p.position.y
            );
            if p.velocity.length_squared() < 0.0001 && p.angular_velocity.length_squared() < 0.0001 {
                sleeping_count += 1;
            }
        }
        assert!(
            sleeping_count > 1200,
            "Majority of debris must settle into O(1) resting sleep on rubble (got {} / 2400)",
            sleeping_count
        );
    }

    #[test]
    fn test_pancake_collapse_floor_crushing_mass_accretion_and_spall_clouds() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Build a concrete and steel floor at y = 10..=11
        let center_vx = 32;
        let center_vz = 32;
        let mut floor_voxels = Vec::new();
        let mut expected_crushed_density_sum = 0.0f32;

        for dx in -3..=3 {
            for dz in -3..=3 {
                let vt = if (dx + dz) % 2 == 0 {
                    VoxelType::Concrete
                } else {
                    VoxelType::SteelRebar
                };
                grid.set(center_vx + dx, 10, center_vz + dz, vt);
                grid.set(center_vx + dx, 11, center_vz + dz, vt);
                floor_voxels.push(IVec3::new(center_vx + dx, 10, center_vz + dz));
                floor_voxels.push(IVec3::new(center_vx + dx, 11, center_vz + dz));
                expected_crushed_density_sum += vt.properties().density;
            }
        }
        let initial_solids = grid.solid_count();
        assert!(initial_solids > 0);

        // Create a massive falling heavy armor slab positioned directly above the floor
        let mut slab_voxels = Vec::new();
        for x in -2..=2 {
            for y in 0..2 {
                for z in -2..=2 {
                    slab_voxels.push((
                        Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                        VoxelType::HeavyArmor,
                    ));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&slab_voxels);
        let slab_world = grid.voxel_to_world(IVec3::new(center_vx, 12, center_vz));
        let initial_chunk_mass = 1200.0f32;

        phys.chunks.push(DynamicChunk {
            position: slab_world + Vec3::new(0.0, 0.4, 0.0),
            velocity: Vec3::new(0.0, -14.0, 0.0), // High downward speed
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: slab_voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: initial_chunk_mass,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        let pre_debris_len = phys.debris.len();
        let pre_sparks_len = phys.sparks.len();

        // Step physics to initiate pancake floor crushing
        for _ in 0..4 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }

        // Verify floor pulverization
        assert!(
            grid.solid_count() < initial_solids,
            "Static floor must be pulverized by falling heavy slab"
        );

        // Verify floor-crushing mass accretion: chunk.mass must increase by Delta M = sum(rho * 0.35)
        let falling_chunk = &phys.chunks[0];
        assert!(
            falling_chunk.mass > initial_chunk_mass,
            "Falling chunk must accrete crushed floor mass: initial {}, current {}",
            initial_chunk_mass,
            falling_chunk.mass
        );

        // Verify 2-3 dynamic micro-chunks and perimeter spall clouds generated
        assert!(
            phys.debris.len() > pre_debris_len + 30,
            "Pulverization must emit dense dynamic micro-chunks and concrete spall chips (debris: {} -> {})",
            pre_debris_len,
            phys.debris.len()
        );
        assert!(
            phys.sparks.len() > pre_sparks_len + 10,
            "Pulverization must emit incandescent ricochet sparks along floor perimeters (sparks: {} -> {})",
            pre_sparks_len,
            phys.sparks.len()
        );
        assert!(expected_crushed_density_sum > 0.0);
    }

    #[test]
    fn test_zero_allocation_populate_sub_blocks_scratch_reuse() {
        let mut vox = Vec::new();
        for x in 0..6 {
            for y in 0..6 {
                for z in 0..6 {
                    vox.push((Vec3::new(x as f32 * 0.5, y as f32 * 0.5, z as f32 * 0.5), VoxelType::Concrete));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&vox);
        let chunk1 = DynamicChunk {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 100.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        };

        let mut scratch: Vec<((Vec3, Vec3), Vec<usize>)> = Vec::with_capacity(4);
        chunk1.populate_sub_blocks(&mut scratch);
        assert_eq!(scratch.len(), 4, "216-voxel chunk should partition into 4 sub-blocks");
        let initial_cap = scratch.capacity();
        let total_vox: usize = scratch.iter().map(|(_, v)| v.len()).sum();
        assert_eq!(total_vox, 216);

        // Run 100 consecutive invocations on different chunks and verify zero re-allocation
        let small_vox = vec![(Vec3::ZERO, VoxelType::Glass), (Vec3::X, VoxelType::Glass)];
        let small_aabb = DynamicChunk::compute_aabb(&small_vox);
        let chunk_small = DynamicChunk {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: small_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: false,
            local_aabb: small_aabb,
        };

        for _ in 0..100 {
            chunk_small.populate_sub_blocks(&mut scratch);
            assert_eq!(scratch.len(), 1, "Small chunk has 1 sub-block");
            assert_eq!(scratch.capacity(), initial_cap, "Scratch buffer capacity must be preserved without churn");

            chunk1.populate_sub_blocks(&mut scratch);
            assert_eq!(scratch.len(), 4);
            assert_eq!(scratch.capacity(), initial_cap, "Scratch buffer capacity must remain stable across cycles");
        }
    }

    #[test]
    fn test_physics_system_scratch_sub_blocks_retained_during_pancake_step() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();

        // Build floor
        for x in 40..50 {
            for z in 40..50 {
                grid.set(x, 6, z, VoxelType::Glass);
            }
        }

        // Spawn falling heavy chunk above floor
        let mut vox = Vec::new();
        for x in 0..5 {
            for z in 0..5 {
                vox.push((Vec3::new(x as f32 * 0.5, 0.0, z as f32 * 0.5), VoxelType::HeavyArmor));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(22.0, 3.5, 22.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 500.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        assert!(phys.scratch_sub_blocks.capacity() >= 4);
        let initial_cap = phys.scratch_sub_blocks.capacity();

        // Step simulation: triggers pancake narrowphase
        let audio = crate::audio::SoundSystem::empty();
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

        // Verify scratch buffer was utilized and capacity preserved
        assert_eq!(phys.scratch_sub_blocks.capacity(), initial_cap, "Scratch sub-blocks capacity must be retained across steps");
    }

    #[test]
    fn test_falling_chunk_moderate_mass_crushes_concrete_rebar_floor_without_upward_bounce() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Build an intermediate concrete & composite rebar floor at y = 8 (world y = 4.0)
        for x in 30..=36 {
            for z in 30..=36 {
                let vt = if (x + z) % 3 == 0 {
                    VoxelType::CompositeRebar
                } else {
                    VoxelType::Concrete
                };
                grid.set(x, 8, z, vt);
            }
        }
        let initial_solids = grid.solid_count();
        assert!(initial_solids > 0);

        // 2. Drop a realistic falling chunk (mass 35 kg, e.g. 15 concrete voxels)
        let mut chunk_vox = Vec::new();
        for x in 0..3 {
            for y in 0..2 {
                for z in 0..2 {
                    chunk_vox.push((Vec3::new(x as f32 * 0.5, y as f32 * 0.5, z as f32 * 0.5), VoxelType::Concrete));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&chunk_vox);
        let floor_world = grid.voxel_to_world(IVec3::new(33, 8, 33));

        phys.chunks.push(DynamicChunk {
            position: floor_world + Vec3::new(0.0, 0.75, 0.0),
            velocity: Vec3::new(0.0, -6.0, 0.0), // Downward velocity
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: chunk_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 35.0, // Realistic moderate mass
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        // Step physics
        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        // Floor must yield to progressive pancake crushing!
        assert!(grid.solid_count() < initial_solids, "Concrete/rebar floor must yield to downward falling chunk");

        // The falling chunk must NOT bounce upward into the air!
        let chunk = &phys.chunks[0];
        assert!(chunk.velocity.y <= 0.0, "Falling chunk must not gain upward bounce velocity (got {})", chunk.velocity.y);
    }

    #[test]
    fn test_resistant_heavy_armor_absorbs_downward_velocity_without_upward_restitution() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Heavy armor floor that resists light debris
        for x in 30..=34 {
            for z in 30..=34 {
                grid.set(x, 8, z, VoxelType::HeavyArmor);
            }
        }

        // Small falling chunk (mass 8 kg) with moderate downward speed (momentum = 24 < 250 threshold)
        let small_vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&small_vox);
        let floor_world = grid.voxel_to_world(IVec3::new(32, 8, 32));

        phys.chunks.push(DynamicChunk {
            position: floor_world + Vec3::new(0.0, 0.35, 0.0),
            velocity: Vec3::new(0.0, -3.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: small_vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 8.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        let chunk = &phys.chunks[0];
        // Must not bounce upward with positive velocity
        assert!(chunk.velocity.y <= 0.0, "Chunk must not bounce upward off resistant barrier (got {})", chunk.velocity.y);
    }

    #[test]
    fn test_cascading_cantilever_floors_collapse_all_the_way_down() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Build a structure with two towers and an extended bridge:
        // Tower 1 at x=20, z=20 from y=1..=10
        for y in 1..=10 {
            grid.set(20, y, 20, VoxelType::Concrete);
        }
        // Bridge between towers at y=6 (x=21..=27)
        for x in 21..=27 {
            grid.set(x, 6, 20, VoxelType::Concrete);
        }
        // Tower 2 at x=28, z=20 from y=1..=6
        for y in 1..=6 {
            grid.set(28, y, 20, VoxelType::Concrete);
        }
        // Extended cantilever past Tower 2 at y=6 (x=29..=32, supported within depth 4 from Tower 2)
        for x in 29..=32 {
            grid.set(x, 6, 20, VoxelType::Concrete);
        }
        // Lower intermediate floor beneath extended cantilever at y=2 (x=29..=32), supported by column at x=32, y=1
        for x in 29..=32 {
            grid.set(x, 2, 20, VoxelType::Concrete);
        }
        grid.set(32, 1, 20, VoxelType::Concrete);

        let initial_islands = grid.detect_unsupported_islands();
        assert_eq!(initial_islands.len(), 0, "Initial structure should be fully supported");

        // 2. Sever Tower 2 at the base (y=1..=3)
        for y in 1..=3 {
            grid.set(28, y, 20, VoxelType::Air);
        }
        assert!(grid.structural_dirty);

        // 3. Structural BFS detects that without Tower 2, the extended cantilever (x=29..=32)
        // and upper tower 2 are completely unsupported (> 4 depth from Tower 1) and collapse!
        let islands = grid.detect_unsupported_islands();
        assert!(!islands.is_empty(), "Severing support column must trigger island collapse!");

        // 4. Convert detected island into dynamic falling chunk
        for island in islands {
            let initial_impulse = Vec3::new(0.0, -4.0, 0.0);
            phys.spawn_island_chunk(island, initial_impulse, Vec3::ZERO);
        }

        assert!(!phys.chunks.is_empty(), "Dynamic chunk must be spawned for falling island");
        let initial_floor_solids = grid.solid_count();

        // 5. Falling island plunges downward and pulverizes the lower intermediate floor at y=2
        for _ in 0..40 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }

        // Floor must be crushed and pulverized, continuing cascading demolition downward!
        assert!(grid.solid_count() < initial_floor_solids, "Falling chunk must pulverize lower floor voxels");
        assert!(grid.structural_dirty, "Floor pulverization must maintain structural dirty flag for chain reaction");
    }

    #[test]
    fn test_quench_molten_slag_into_dark_obsidian() {
        let mut phys = PhysicsSystem::new();
        let center = Vec3::new(20.0, 5.0, 20.0);

        // Spawn molten glowing slag fissure decals
        phys.spawn_decal(center + Vec3::new(0.5, 0.0, 0.0), Vec3::Y, 1.5, Color::new(1.0, 0.4, 0.1, 1.0), DecalKind::PlasmaBurn);
        phys.spawn_decal(center - Vec3::new(0.5, 0.0, 0.0), Vec3::Y, 1.2, Color::new(1.0, 0.3, 0.1, 1.0), DecalKind::KineticImpact);
        assert_eq!(phys.decals.len(), 2);

        // Rupturing coolant pipes quenches glowing slag into dark obsidian rock
        let quenched = phys.quench_molten_slag(center, 4.0);
        assert_eq!(quenched, 2, "Both molten fissures within coolant spray must be quenched");

        for d in &phys.decals {
            assert_eq!(d.kind, DecalKind::QuenchedObsidian, "Fissures must solidify into QuenchedObsidian");
            assert_eq!(d.color, Color::new(0.08, 0.09, 0.11, 1.0), "Quenched rock must have dark obsidian tone");
        }
        assert!(!phys.smoke.is_empty(), "Quenching must emit cryogenic steam plumes");
        assert!(!phys.sparks.is_empty(), "Quenching must spray cryogenic vapor droplets");
    }

    #[test]
    fn test_column_buckling_spawns_shed_concrete_chunks() {
        let mut phys = PhysicsSystem::new();
        let col_pos = Vec3::new(15.0, 6.0, 15.0);

        phys.spawn_column_buckling(col_pos, Vec3::X);

        // Must shed discrete concrete chunks under compressive failure
        assert!(!phys.debris.is_empty(), "Buckling column must shed concrete chunks");
        let concrete_chunks = phys.debris.iter().filter(|d| d.color == Color::new(0.66, 0.63, 0.59, 1.0)).count();
        assert!(concrete_chunks >= 4, "Must shed at least 4 concrete debris chunks");

        // Must spawn spall dust puff
        assert!(!phys.smoke.is_empty(), "Buckling must spawn spall dust puff");

        // Must place concrete fracture decal on column face
        assert!(!phys.decals.is_empty(), "Buckling must spawn surface fracture decal");
    }

    #[test]
    fn test_heavy_machinery_ballistic_penetrator_crushes_floors_and_accretes_mass() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let cx = 30;
        let cz = 30;
        // Build floor slab at y = 8..=9
        for dx in -3..=3 {
            for dz in -3..=3 {
                grid.set(cx + dx, 8, cz + dz, VoxelType::Concrete);
                grid.set(cx + dx, 9, cz + dz, VoxelType::SteelRebar);
            }
        }
        let initial_solids = grid.solid_count();

        // 4-ton elevator cab penetrator (compact 2x2x2 HeavyArmor, mass = 600)
        let mut cab_voxels = Vec::new();
        for x in 0..2 {
            for y in 0..2 {
                for z in 0..2 {
                    cab_voxels.push((
                        Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                        VoxelType::HeavyArmor,
                    ));
                }
            }
        }
        let aabb = DynamicChunk::compute_aabb(&cab_voxels);
        let start_pos = grid.voxel_to_world(IVec3::new(cx, 13, cz));
        let initial_mass = 600.0f32;

        phys.chunks.push(DynamicChunk {
            position: start_pos,
            velocity: Vec3::new(0.0, -10.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: cab_voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: initial_mass,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        // Plunge through floor
        for _ in 0..15 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }

        assert!(grid.solid_count() < initial_solids, "Penetrator must punch hole through lower floor");
        assert!(phys.chunks[0].mass > initial_mass, "Penetrator must accrete mass of crushed floor voxels");
    }

    #[test]
    fn test_frustum_culling_bounding_checks() {
        let cam_pos = Vec3::new(0.0, 5.0, 0.0);
        let cam_dir = Vec3::new(0.0, 0.0, 1.0);
        let cam_up = Vec3::Y;
        let frustum = crate::voxel::FrustumPlanes::from_camera(cam_pos, cam_dir, cam_up, 1.77, 1.05, 0.1, 100.0);

        // Point directly in front of camera
        assert!(frustum.is_sphere_in_frustum(Vec3::new(0.0, 5.0, 20.0), 1.0));
        assert!(frustum.is_aabb_in_frustum(Vec3::new(-1.0, 4.0, 19.0), Vec3::new(1.0, 6.0, 21.0)));

        // Behind camera: must be culled
        assert!(!frustum.is_sphere_in_frustum(Vec3::new(0.0, 5.0, -20.0), 1.0));
        assert!(!frustum.is_aabb_in_frustum(Vec3::new(-1.0, 4.0, -21.0), Vec3::new(1.0, 6.0, -19.0)));

        // Beyond far plane: must be culled
        assert!(!frustum.is_sphere_in_frustum(Vec3::new(0.0, 5.0, 150.0), 1.0));

        // Far off to the side: must be culled
        assert!(!frustum.is_sphere_in_frustum(Vec3::new(100.0, 5.0, 20.0), 1.0));
    }

    #[test]
    fn test_subterranean_chunk_collision_down_to_minus_24m() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // 1. Artificially stamp a subterranean crater floor at Y = -12.0m
        if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(0.0, 0.0) {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let idx = (gx as i32 + dx) as usize * crate::voxel::GRID_Z + (gz as i32 + dz) as usize;
                    phys.rubble_height_grid[idx] = -12.0;
                    for y in -24..=0 {
                        grid.set(gx as i32 + dx, y, gz as i32 + dz, VoxelType::Air);
                    }
                }
            }
        }
        grid.dirty_min = None;
        grid.dirty_max = None;
        grid.structural_dirty = false;
        phys.rubble_dirty = false;

        // 2. Drop a chunk from Y = -8.0m falling at -5 m/s
        let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, -8.0, 0.0),
            velocity: Vec3::new(0.0, -5.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        // 3. Step physics until chunk descends past 0.50m and strikes crater floor at -12.0m
        for _ in 0..40 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
            if phys.chunks[0].settled {
                break;
            }
        }

        // 4. Assert chunk descended and rests on subterranean crater floor
        assert!(phys.chunks[0].position.y < 0.0, "Chunk must descend below ground level");
        assert!(phys.chunks[0].position.y >= -12.0, "Chunk must not clip below crater floor, got {}", phys.chunks[0].position.y);
        assert!(phys.chunks[0].settled, "Chunk must settle on crater floor");
    }

    #[test]
    fn test_dynamic_crater_floor_update_on_excavation() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();

        // Carve a crater at center (0, 0, 0) removing bedrock down to -10.0m
        grid.carve_sphere_advanced(Vec3::new(0.0, 0.0, 0.0), 6.0, 2000.0, crate::voxel::DamageType::Explosive);
        phys.update_crater_floor(&grid, Vec3::new(0.0, 0.0, 0.0), 6.0);

        let h_center = phys.rubble_height_at(0.0, 0.0);
        assert!(h_center < 0.0, "Crater center rubble height must be subterranean, got {}", h_center);
        assert!(h_center >= VOID_BOUNDARY_Y, "Crater floor must be at or above void boundary");

        // Outside the crater must remain at unexcavated bedrock elevation
        let h_outside = phys.rubble_height_at(18.0, 18.0);
        assert_eq!(h_outside, VOXEL_SIZE, "Unexcavated terrain must remain at VOXEL_SIZE (0.50)");
    }

    #[test]
    fn test_debris_and_sparks_resting_in_subterranean_crater() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Carve subterranean crater down into earth
        grid.carve_sphere_advanced(Vec3::new(0.0, 0.0, 0.0), 14.5, 5000.0, crate::voxel::DamageType::Explosive);
        phys.update_crater_floor(&grid, Vec3::new(0.0, 0.0, 0.0), 14.5);
        let crater_floor_y = phys.rubble_height_at(0.0, 0.0);
        assert!(crater_floor_y < -10.0, "Crater floor must be subterranean, got {}", crater_floor_y);

        // Clear spall generated from carving so only the test debris is tracked
        grid.spall_events.clear();
        phys.debris.clear();

        // Spawn falling debris particle inside crater cavity
        phys.debris.push(DebrisParticle {
            position: Vec3::new(0.0, -5.0, 0.0),
            velocity: Vec3::new(0.0, -8.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.15,
            color: WHITE,
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });

        for _ in 0..150 {
            phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        }

        assert_eq!(phys.debris.len(), 1);
        let p = &phys.debris[0];
        assert!(
            (p.position.y - crater_floor_y).abs() <= 0.35,
            "Debris must rest on crater floor at {}, got {}",
            crater_floor_y,
            p.position.y
        );
        assert!(p.velocity.length_squared() < 0.001, "Resting debris must sleep with zero velocity");
    }

    #[test]
    fn test_clean_despawn_of_chunks_and_particles_below_void_boundary() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        // Spawn chunk, debris, spark falling at Y = -23.8m
        let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(0.0, -23.8, 0.0),
            velocity: Vec3::new(0.0, -15.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.debris.push(DebrisParticle {
            position: Vec3::new(0.0, -23.9, 0.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.1,
            color: WHITE,
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });

        phys.sparks.push(SparkParticle {
            position: Vec3::new(0.0, -23.9, 0.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            color: YELLOW,
            life: 1.0,
            max_life: 1.0,
        });

        // Step physics 2 frames
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        assert_eq!(phys.chunks.len(), 0, "Chunk below VOID_BOUNDARY_Y (-24.0m) must be cleanly despawned");
        assert_eq!(phys.debris.len(), 0, "Debris below VOID_BOUNDARY_Y must be cleanly despawned");
        assert_eq!(phys.sparks.len(), 0, "Sparks below VOID_BOUNDARY_Y must be cleanly despawned");
    }

    #[test]
    fn test_g6_m1_dynamic_chunk_fracture_preserves_linear_angular_and_tangential_momentum() {
        let mut voxels = Vec::with_capacity(50);
        for x in -25..25 {
            voxels.push((Vec3::new(x as f32 * 0.1, 0.0, 0.0), VoxelType::Concrete));
        }
        let aabb = DynamicChunk::compute_aabb(&voxels);
        let parent_pos = Vec3::new(10.0, 20.0, 10.0);
        let parent_vel = Vec3::new(8.0, -12.0, 3.0);
        let parent_ang = Vec3::new(0.0, 5.0, 0.0);

        let parent = DynamicChunk {
            position: parent_pos,
            velocity: parent_vel,
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: parent_ang,
            voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 120.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        };

        let (sub_chunks, _, _) = parent.fracture();
        assert!(sub_chunks.len() >= 2, "Chunk with 50 voxels must fracture into at least 2 sub-chunks");

        for sub in &sub_chunks {
            assert!(
                sub.velocity.y < -5.0,
                "Sub-chunk downward velocity must be preserved (got vy = {})",
                sub.velocity.y
            );
            assert!(
                (sub.angular_velocity.y - parent_ang.y).abs() <= 2.0,
                "Sub-chunk angular velocity y must inherit parent spin (got {} vs parent {})",
                sub.angular_velocity.y,
                parent_ang.y
            );
            let r_offset = sub.position - parent_pos;
            if r_offset.x.abs() > 0.5 {
                let expected_tangential_z = -parent_ang.y * r_offset.x;
                let actual_z_rel_parent = sub.velocity.z - parent_vel.z;
                if expected_tangential_z.abs() > 1.5 {
                    assert!(
                        (actual_z_rel_parent * expected_tangential_z) > 0.0,
                        "Tangential velocity must reflect omega x r: expected_tang_z={}, actual_z_rel={}",
                        expected_tangential_z,
                        actual_z_rel_parent
                    );
                }
            }
        }
    }

    #[test]
    fn test_g6_m1_restitution_scales_inversely_with_mass_zero_bounce_on_heavy_chunks() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        let mut heavy_voxels = Vec::with_capacity(100);
        for x in 0..10 {
            for z in 0..10 {
                heavy_voxels.push((Vec3::new(x as f32 * 0.25, 0.0, z as f32 * 0.25), VoxelType::Concrete));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&heavy_voxels);

        phys.chunks.push(DynamicChunk {
            position: Vec3::new(10.0, 0.2, 10.0),
            velocity: Vec3::new(0.0, -8.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: heavy_voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 2500.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(10.0, 10.0) {
            phys.rubble_height_grid[gx * crate::voxel::GRID_Z + gz] = 0.5;
        }

        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        let chunk = &phys.chunks[0];
        assert!(
            chunk.velocity.y <= 0.0,
            "High-mass chunk must have zero restitution bounce (got vy = {})",
            chunk.velocity.y
        );
    }

    #[test]
    fn test_g6_m1_corner_angular_impulse_clamped_and_upright_damping() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        let mut voxels = Vec::with_capacity(60);
        for x in -5..=5 {
            for z in -2..=2 {
                voxels.push((Vec3::new(x as f32 * 0.2, 0.0, z as f32 * 0.2), VoxelType::Concrete));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&voxels);

        phys.chunks.push(DynamicChunk {
            position: Vec3::new(15.0, 0.3, 15.0),
            velocity: Vec3::new(0.0, -2.5, 0.0),
            rotation: Vec3::new(0.1, 0.0, 0.1),
            rot_mat: Mat3::from_euler(Vec3::new(0.1, 0.0, 0.1)),
            angular_velocity: Vec3::new(1.0, 0.0, -1.0),
            voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 150.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(15.0, 15.0) {
            phys.rubble_height_grid[gx * crate::voxel::GRID_Z + gz] = 0.5;
        }

        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        let chunk = &phys.chunks[0];
        // Angular contact impulse is clamped to +/- 2.5 rad/s per axis (down from +/- 12.0)
        assert!(
            chunk.angular_velocity.x.abs() <= 2.65 && chunk.angular_velocity.y.abs() <= 2.65 && chunk.angular_velocity.z.abs() <= 2.65,
            "Angular velocity components must be clamped within +/- 2.5 rad/s contact impulse limit (got {:?})",
            chunk.angular_velocity
        );
        assert!(
            chunk.angular_velocity.length() < 4.5,
            "Angular velocity magnitude must be strictly bounded under 4.5 rad/s (got ||omega|| = {})",
            chunk.angular_velocity.length()
        );
    }

    #[test]
    fn test_g6_m1_momentum_conserving_pancake_pulverization_carries_velocity() {
        let mut phys = PhysicsSystem::new();
        let mut grid = VoxelGrid::new();
        let audio = SoundSystem::empty();

        for x in 28..=34 {
            for z in 28..=34 {
                grid.set(x, 4, z, VoxelType::Concrete);
                grid.set(x, 8, z, VoxelType::Concrete);
            }
        }

        let mut slab_voxels = Vec::with_capacity(36);
        for x in -3..=2 {
            for z in -3..=2 {
                slab_voxels.push((Vec3::new(x as f32 * 0.25, 0.0, z as f32 * 0.25), VoxelType::Concrete));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&slab_voxels);
        let floor_8_world = grid.voxel_to_world(IVec3::new(31, 8, 31));

        phys.chunks.push(DynamicChunk {
            position: floor_8_world + Vec3::new(0.0, 0.35, 0.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: slab_voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 800.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        let chunk = &phys.chunks[0];
        assert!(
            chunk.velocity.y < -6.5,
            "Downward momentum must be conserved across floor pulverization (got vy = {})",
            chunk.velocity.y
        );
        assert_ne!(chunk.velocity.y, 0.0, "Falling slab must not halt mid-air");
    }

    #[test]
    fn test_g6_m2_effective_3d_speed_multi_directional_kinetic_crushing() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        grid.set(50, 10, 50, VoxelType::Concrete);
        assert_eq!(grid.get(50, 10, 50), VoxelType::Concrete);
        let target_world = grid.voxel_to_world(IVec3::new(50, 10, 50));

        let voxels = vec![
            (Vec3::new(-0.25, 0.0, 0.0), VoxelType::HeavyArmor),
            (Vec3::new(0.25, 0.0, 0.0), VoxelType::HeavyArmor),
        ];
        let aabb = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk {
            position: target_world - Vec3::new(0.4, 0.0, 0.0),
            velocity: Vec3::new(12.0, 0.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::new(0.0, 2.0, 0.0),
            voxels,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 500.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        assert_eq!(
            grid.get(50, 10, 50),
            VoxelType::Air,
            "Horizontal kinetic momentum must crush static pillar (effective_speed > 0.5)"
        );
    }

    #[test]
    fn test_g6_m2_swept_anti_tunneling_candidate_points() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        grid.set(30, 8, 30, VoxelType::Concrete);
        let floor_world = grid.voxel_to_world(IVec3::new(30, 8, 30));

        let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: floor_world + Vec3::new(0.0, 0.8, 0.0),
            velocity: Vec3::new(0.0, -30.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 200.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        assert_eq!(
            grid.get(30, 8, 30),
            VoxelType::Air,
            "Swept collision probing must crush the floor rather than tunneling through"
        );
    }

    #[test]
    fn test_g6_m2_airborne_chunks_do_not_settle_mid_air() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
        let aabb = DynamicChunk::compute_aabb(&vox);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(20.0, 40.0, 20.0),
            velocity: Vec3::new(0.0, -0.05, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vox,
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 10.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });

        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        assert!(!phys.chunks[0].settled, "Airborne chunk without static support must not settle mid-air");
        let rubble_h = phys.rubble_height_at(20.0, 20.0);
        assert!(rubble_h < 1.0, "Rubble height must not be stamped at elevation 40m mid-air, got {}", rubble_h);
    }

    #[test]
    fn test_g6_m2_resting_support_check_all_chunks_no_48_cap() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        grid.set(10, 5, 10, VoxelType::Concrete);
        let sup_pos = grid.voxel_to_world(IVec3::new(10, 5, 10));

        for i in 0..60 {
            let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
            let aabb = DynamicChunk::compute_aabb(&vox);
            let pos = if i == 55 {
                sup_pos + Vec3::new(0.0, VOXEL_SIZE, 0.0)
            } else {
                Vec3::new(5.0 + (i as f32 * 0.5), 0.5, 5.0)
            };
            phys.chunks.push(DynamicChunk {
                position: pos,
                velocity: Vec3::ZERO,
                rotation: Vec3::ZERO,
                rot_mat: Mat3::IDENTITY,
                angular_velocity: Vec3::ZERO,
                voxels: vox,
                voxel_health: Vec::new(),
                exposed_voxels: Vec::new(),
                mass: 10.0,
                life: 1.0,
                settled: true,
                local_aabb: aabb,
            });
        }

        grid.set(10, 5, 10, VoxelType::Air);
        grid.structural_dirty = true;

        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        assert!(
            !phys.chunks[55].settled,
            "Chunk 55 must wake up after its support is destroyed (no 48-chunk cap)"
        );
    }

    #[test]
    fn test_dynamic_chunk_slice_by_plane_bisects_rigid_body() {
        // Create a 2-voxel chunk: one at local x = -0.5, one at local x = 0.5
        let voxels = vec![
            (Vec3::new(-0.5, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::Concrete),
        ];
        let aabb = DynamicChunk::compute_aabb(&voxels);
        let chunk = DynamicChunk::new(
            Vec3::new(10.0, 5.0, 10.0),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            voxels,
            20.0,
            10.0,
            false,
            aabb,
        );

        // Plane cutting along world X = 10.0 with normal (1, 0, 0)
        let plane_normal = Vec3::X;
        let plane_dist = -10.0;

        let result = chunk.slice_by_plane(plane_normal, plane_dist);
        let (child_a, child_b) = match result {
            Ok(pair) => pair,
            Err(_) => panic!("Chunk spanning plane must be cleanly bisected"),
        };
        assert_eq!(child_a.voxels.len(), 1);
        assert_eq!(child_b.voxels.len(), 1);
        assert!(child_a.velocity.x > 0.0, "Child A must receive positive separating velocity");
        assert!(child_b.velocity.x < 0.0, "Child B must receive negative separating velocity");
        assert!((child_a.mass + child_b.mass - 20.0).abs() < 1e-3, "Mass must be conserved");
    }

    #[test]
    fn test_physics_slice_intersecting_chunks() {
        let mut phys = PhysicsSystem::new();
        let voxels = vec![
            (Vec3::new(-0.5, 0.0, 0.0), VoxelType::SteelRebar),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::SteelRebar),
        ];
        let aabb = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk::new(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            voxels,
            50.0,
            10.0,
            false,
            aabb,
        ));

        let cut_count = phys.slice_intersecting_chunks(Vec3::new(0.0, 2.0, 0.0), Vec3::X, 10.0);
        assert_eq!(cut_count, 1);
        assert_eq!(phys.chunks.len(), 2, "Bisected chunk produces 2 child chunks in physics system");
    }

    #[test]
    fn test_physics_slice_intersecting_chunks_multiple_and_non_bisected() {
        let mut phys = PhysicsSystem::new();

        // Chunk 0: Completely on positive X side (does NOT span plane X = 0)
        let vox0 = vec![(Vec3::new(2.0, 0.0, 0.0), VoxelType::Concrete)];
        let aabb0 = DynamicChunk::compute_aabb(&vox0);
        phys.chunks.push(DynamicChunk::new(Vec3::new(5.0, 2.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, vox0, 10.0, 10.0, false, aabb0));

        // Chunk 1: Spans plane X = 0 (should be sliced)
        let vox1 = vec![
            (Vec3::new(-0.5, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::Concrete),
        ];
        let aabb1 = DynamicChunk::compute_aabb(&vox1);
        phys.chunks.push(DynamicChunk::new(Vec3::new(0.0, 2.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, vox1, 20.0, 10.0, false, aabb1));

        // Chunk 2: Completely on negative X side (does NOT span plane X = 0)
        let vox2 = vec![(Vec3::new(-2.0, 0.0, 0.0), VoxelType::Concrete)];
        let aabb2 = DynamicChunk::compute_aabb(&vox2);
        phys.chunks.push(DynamicChunk::new(Vec3::new(-5.0, 2.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, vox2, 10.0, 10.0, false, aabb2));

        let cut_count = phys.slice_intersecting_chunks(Vec3::ZERO, Vec3::X, 15.0);
        assert_eq!(cut_count, 1, "Only Chunk 1 should be sliced");
        assert_eq!(phys.chunks.len(), 4, "3 original chunks - 1 + 2 children = 4 chunks total");
    }

    #[test]
    fn test_dynamic_chunk_slice_500_voxels_conserves_mass_and_grid_alignment() {
        // Construct a dynamic chunk containing > 500 voxels (e.g. 8x8x8 = 512 voxels)
        let mut voxels = Vec::with_capacity(512);
        for x in 0..8 {
            for y in 0..8 {
                for z in 0..8 {
                    let p = Vec3::new(
                        (x as f32 - 3.5) * VOXEL_SIZE,
                        (y as f32 - 3.5) * VOXEL_SIZE,
                        (z as f32 - 3.5) * VOXEL_SIZE,
                    );
                    voxels.push((p, VoxelType::Concrete));
                }
            }
        }
        assert_eq!(voxels.len(), 512);
        let aabb = DynamicChunk::compute_aabb(&voxels);
        let chunk = DynamicChunk::new(
            Vec3::new(20.0, 10.0, 20.0),
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            voxels,
            512.0 * VoxelType::Concrete.properties().density,
            15.0,
            false,
            aabb,
        );

        let initial_mass = chunk.mass;
        // Slice along world X = 20.0
        let (child_a, child_b) = chunk.slice_by_plane(Vec3::X, -20.0).unwrap_or_else(|_| panic!("512-voxel chunk must be bisected"));

        assert_eq!(child_a.voxels.len(), 256);
        assert_eq!(child_b.voxels.len(), 256);
        assert_eq!(child_a.voxels.len() + child_b.voxels.len(), 512);
        assert!((child_a.mass + child_b.mass - initial_mass).abs() < 1e-3, "Mass must be precisely conserved");

        // Verify exposed voxels computation succeeds with snapped grid
        assert!(!child_a.exposed_voxels.is_empty());
        assert!(!child_b.exposed_voxels.is_empty());
    }
}


