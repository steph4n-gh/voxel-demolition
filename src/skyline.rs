use macroquad::prelude::*;
use std::f32::consts::TAU;

pub const MAX_BATCH_VERTICES: usize = 3600;
pub const MAX_BATCH_INDICES: usize = 4400;

#[derive(Clone, Debug)]
pub struct SkylineBeacon {
    pub position: Vec3,
    pub color: Color,
    pub radius: f32,
    pub pulse_freq: f32,
    pub phase: f32,
}

pub struct DistantSkyline {
    pub base_sub_meshes: Vec<Mesh>,
    pub beacons: Vec<SkylineBeacon>,
    pub beacons_mesh: Mesh,
}

struct SkylineBatcher {
    sub_meshes: Vec<Mesh>,
    curr_vertices: Vec<Vertex>,
    curr_indices: Vec<u16>,
}

impl SkylineBatcher {
    fn new() -> Self {
        Self {
            sub_meshes: Vec::new(),
            curr_vertices: Vec::with_capacity(3000),
            curr_indices: Vec::with_capacity(4000),
        }
    }

    fn ensure_capacity(&mut self, add_verts: usize, add_indices: usize) {
        if self.curr_vertices.len() + add_verts > MAX_BATCH_VERTICES
            || self.curr_indices.len() + add_indices > MAX_BATCH_INDICES
        {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if !self.curr_indices.is_empty() {
            self.sub_meshes.push(Mesh {
                vertices: std::mem::take(&mut self.curr_vertices),
                indices: std::mem::take(&mut self.curr_indices),
                texture: None,
            });
            self.curr_vertices = Vec::with_capacity(3000);
            self.curr_indices = Vec::with_capacity(4000);
        }
    }

    fn push_quad(&mut self, v0: Vertex, v1: Vertex, v2: Vertex, v3: Vertex) {
        self.ensure_capacity(4, 6);
        let base = self.curr_vertices.len() as u16;
        self.curr_vertices.push(v0);
        self.curr_vertices.push(v1);
        self.curr_vertices.push(v2);
        self.curr_vertices.push(v3);
        self.curr_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn finish(mut self) -> Vec<Mesh> {
        self.flush();
        self.sub_meshes
    }
}

// Altitude-based atmospheric haze vertex coloring
fn altitude_color(y: f32) -> Color {
    if y <= 0.0 {
        Color::new(0.025, 0.030, 0.045, 1.0)
    } else if y <= 25.0 {
        let t = (y / 25.0).clamp(0.0, 1.0);
        Color::new(
            0.065 * (1.0 - t) + 0.045 * t,
            0.078 * (1.0 - t) + 0.055 * t,
            0.105 * (1.0 - t) + 0.078 * t,
            1.0,
        )
    } else if y <= 100.0 {
        let t = ((y - 25.0) / 75.0).clamp(0.0, 1.0);
        Color::new(
            0.045 * (1.0 - t) + 0.030 * t,
            0.055 * (1.0 - t) + 0.040 * t,
            0.078 * (1.0 - t) + 0.062 * t,
            1.0,
        )
    } else {
        let t = ((y - 100.0) / 220.0).clamp(0.0, 1.0);
        Color::new(
            0.030 * (1.0 - t) + 0.015 * t,
            0.040 * (1.0 - t) + 0.022 * t,
            0.062 * (1.0 - t) + 0.040 * t,
            1.0,
        )
    }
}

fn make_vert(pos: Vec3, col: Color) -> Vertex {
    Vertex {
        position: pos,
        uv: Vec2::ZERO,
        color: col.into(),
        normal: Vec4::ZERO,
    }
}

// Helper: oriented box facing radially inward
fn add_oriented_box(
    batcher: &mut SkylineBatcher,
    center: Vec3,
    size: Vec3,
    theta: f32,
    base_col_fn: impl Fn(f32) -> Color,
) {
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    let rad = Vec3::new(cos_t, 0.0, sin_t); // outward
    let tan = Vec3::new(-sin_t, 0.0, cos_t); // tangent

    let hx = tan * (size.x * 0.5);
    let hz = rad * (size.z * 0.5);
    let y0 = center.y - size.y * 0.5;
    let y1 = center.y + size.y * 0.5;

    let c0 = base_col_fn(y0);
    let c1 = base_col_fn(y1);

    // 4 base corners
    let b0 = center + hx - hz;
    let b1 = center - hx - hz;
    let b2 = center - hx + hz;
    let b3 = center + hx + hz;

    let p00 = Vec3::new(b0.x, y0, b0.z);
    let p10 = Vec3::new(b1.x, y0, b1.z);
    let p20 = Vec3::new(b2.x, y0, b2.z);
    let p30 = Vec3::new(b3.x, y0, b3.z);

    let p01 = Vec3::new(b0.x, y1, b0.z);
    let p11 = Vec3::new(b1.x, y1, b1.z);
    let p21 = Vec3::new(b2.x, y1, b2.z);
    let p31 = Vec3::new(b3.x, y1, b3.z);

    // Inward face (-hz, towards city)
    batcher.push_quad(
        make_vert(p00, c0),
        make_vert(p01, c1),
        make_vert(p11, c1),
        make_vert(p10, c0),
    );
    // Left face (+hx)
    batcher.push_quad(
        make_vert(p30, c0),
        make_vert(p31, c1),
        make_vert(p01, c1),
        make_vert(p00, c0),
    );
    // Right face (-hx)
    batcher.push_quad(
        make_vert(p10, c0),
        make_vert(p11, c1),
        make_vert(p21, c1),
        make_vert(p20, c0),
    );
    // Top roof
    batcher.push_quad(
        make_vert(p01, c1),
        make_vert(p31, c1),
        make_vert(p21, c1),
        make_vert(p11, c1),
    );
}

// Helper: neon rim strip along an inward ledge
fn add_neon_rim(
    batcher: &mut SkylineBatcher,
    center: Vec3,
    width: f32,
    height: f32,
    theta: f32,
    neon_col: Color,
) {
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    let tan = Vec3::new(-sin_t, 0.0, cos_t);
    let rad = Vec3::new(cos_t, 0.0, sin_t);

    let p_center = center - rad * 0.4; // slightly proud of inward face
    let left = p_center - tan * (width * 0.5);
    let right = p_center + tan * (width * 0.5);

    let v0 = make_vert(left + Vec3::new(0.0, -height * 0.5, 0.0), neon_col);
    let v1 = make_vert(left + Vec3::new(0.0, height * 0.5, 0.0), neon_col);
    let v2 = make_vert(right + Vec3::new(0.0, height * 0.5, 0.0), neon_col);
    let v3 = make_vert(right + Vec3::new(0.0, -height * 0.5, 0.0), neon_col);

    batcher.push_quad(v0, v1, v2, v3);
}

struct CoolingTowerSpec {
    height: f32,
    r_base: f32,
    r_waist: f32,
    r_top: f32,
    base_phase: f32,
}

// Generate Hyperbolic Fusion Cooling Tower
fn add_hyperbolic_cooling_tower(
    batcher: &mut SkylineBatcher,
    beacons: &mut Vec<SkylineBeacon>,
    center_pos: Vec3,
    spec: CoolingTowerSpec,
) {
    let rings = 7;
    let segments = 8;

    let mut ring_verts = Vec::with_capacity((rings + 1) * segments);
    for r in 0..=rings {
        let t = r as f32 / rings as f32;
        let y = center_pos.y + t * spec.height;
        let radius = if t <= 0.65 {
            let u = (0.65 - t) / 0.65;
            spec.r_waist + (spec.r_base - spec.r_waist) * u * u
        } else {
            let u = (t - 0.65) / 0.35;
            spec.r_waist + (spec.r_top - spec.r_waist) * u * u
        };

        let col = altitude_color(y);
        for s in 0..segments {
            let phi = (s as f32 / segments as f32) * TAU;
            let px = center_pos.x + phi.cos() * radius;
            let pz = center_pos.z + phi.sin() * radius;
            ring_verts.push(make_vert(Vec3::new(px, y, pz), col));
        }
    }

    // Connect rings
    for r in 0..rings {
        let r0 = r * segments;
        let r1 = (r + 1) * segments;
        for s in 0..segments {
            let next_s = (s + 1) % segments;
            let v0 = ring_verts[r0 + s];
            let v1 = ring_verts[r1 + s];
            let v2 = ring_verts[r1 + next_s];
            let v3 = ring_verts[r0 + next_s];
            batcher.push_quad(v0, v1, v2, v3);
        }
    }

    // Top rim glowing fusion exhaust lip
    let top_y = center_pos.y + spec.height;
    let exhaust_col = Color::new(0.18, 0.88, 1.0, 0.95);
    let inner_r = spec.r_top * 0.72;
    for s in 0..segments {
        let next_s = (s + 1) % segments;
        let phi0 = (s as f32 / segments as f32) * TAU;
        let phi1 = (next_s as f32 / segments as f32) * TAU;

        let out0 = Vec3::new(center_pos.x + phi0.cos() * spec.r_top, top_y, center_pos.z + phi0.sin() * spec.r_top);
        let out1 = Vec3::new(center_pos.x + phi1.cos() * spec.r_top, top_y, center_pos.z + phi1.sin() * spec.r_top);
        let in0 = Vec3::new(center_pos.x + phi0.cos() * inner_r, top_y - 1.5, center_pos.z + phi0.sin() * inner_r);
        let in1 = Vec3::new(center_pos.x + phi1.cos() * inner_r, top_y - 1.5, center_pos.z + phi1.sin() * inner_r);

        batcher.push_quad(
            make_vert(out0, exhaust_col),
            make_vert(in0, exhaust_col),
            make_vert(in1, exhaust_col),
            make_vert(out1, exhaust_col),
        );
    }

    // Rim aviation warning beacons
    beacons.push(SkylineBeacon {
        position: center_pos + Vec3::new(0.0, spec.height + 1.2, 0.0),
        color: Color::new(1.0, 0.20, 0.20, 1.0),
        radius: 2.2,
        pulse_freq: 2.2,
        phase: spec.base_phase,
    });
}

pub fn create_distant_skyline() -> DistantSkyline {
    let mut batcher = SkylineBatcher::new();
    let mut beacons = Vec::with_capacity(36);

    let num_sectors = 36;
    let radius = 205.0f32;

    // 1. Continuous Subterranean Horizon Curtain / Skirt (-35.0m to 0.50m)
    let skirt_segments = 36;
    let skirt_col_bot = Color::new(0.020, 0.025, 0.038, 1.0);
    let skirt_col_top = Color::new(0.065, 0.078, 0.105, 1.0);
    for s in 0..skirt_segments {
        let next_s = (s + 1) % skirt_segments;
        let t0 = (s as f32 / skirt_segments as f32) * TAU;
        let t1 = (next_s as f32 / skirt_segments as f32) * TAU;

        let b0 = Vec3::new(t0.cos() * radius, -35.0, t0.sin() * radius);
        let b1 = Vec3::new(t1.cos() * radius, -35.0, t1.sin() * radius);
        let top0 = Vec3::new(t0.cos() * radius, 0.50, t0.sin() * radius);
        let top1 = Vec3::new(t1.cos() * radius, 0.50, t1.sin() * radius);

        batcher.push_quad(
            make_vert(b0, skirt_col_bot),
            make_vert(top0, skirt_col_top),
            make_vert(top1, skirt_col_top),
            make_vert(b1, skirt_col_bot),
        );
    }

    // 2. 360-Degree Megastructures by Sector
    for s in 0..num_sectors {
        let theta = (s as f32 / num_sectors as f32) * TAU;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let center = Vec3::new(cos_t * radius, 0.0, sin_t * radius);
        let tan = Vec3::new(-sin_t, 0.0, cos_t);
        let phase = s as f32 * 0.45;

        match s {
            // Power Stations with Hyperbolic Fusion Cooling Towers (4 sectors)
            4 | 13 | 22 | 31 => {
                // Turbine hall podium
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 8.0, 0.0),
                    Vec3::new(32.0, 36.0, 20.0),
                    theta,
                    altitude_color,
                );
                // Twin cooling towers
                let tower1_pos = center + tan * (-7.5);
                let tower2_pos = center + tan * 7.5;
                add_hyperbolic_cooling_tower(
                    &mut batcher,
                    &mut beacons,
                    tower1_pos,
                    CoolingTowerSpec {
                        height: 80.0,
                        r_base: 11.5,
                        r_waist: 7.0,
                        r_top: 9.0,
                        base_phase: phase,
                    },
                );
                add_hyperbolic_cooling_tower(
                    &mut batcher,
                    &mut beacons,
                    tower2_pos,
                    CoolingTowerSpec {
                        height: 80.0,
                        r_base: 11.5,
                        r_waist: 7.0,
                        r_top: 9.0,
                        base_phase: phase + 1.2,
                    },
                );
                // Neon power conduit
                add_neon_rim(
                    &mut batcher,
                    center + Vec3::new(0.0, 26.0, 0.0),
                    28.0,
                    1.2,
                    theta,
                    Color::new(1.0, 0.60, 0.10, 1.0),
                );
            }

            // Stratospheric Orbital Sky-Elevators (2 sectors)
            0 | 18 => {
                // Ground anchor gantry
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 15.0, 0.0),
                    Vec3::new(24.0, 50.0, 18.0),
                    theta,
                    altitude_color,
                );
                // Monumental tether column stretching to Y = 320m!
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 160.0, 0.0),
                    Vec3::new(4.5, 320.0, 3.5),
                    theta,
                    altitude_color,
                );
                // Climber Carriages (Cargo Transit Pods)
                for &(climber_y, col_acc) in &[
                    (70.0f32, Color::new(0.05, 0.85, 1.0, 1.0)),
                    (155.0f32, Color::new(1.0, 0.65, 0.15, 1.0)),
                    (240.0f32, Color::new(0.98, 0.15, 0.60, 1.0)),
                ] {
                    add_oriented_box(
                        &mut batcher,
                        center + Vec3::new(0.0, climber_y, 0.0),
                        Vec3::new(8.0, 10.0, 6.0),
                        theta,
                        altitude_color,
                    );
                    add_neon_rim(
                        &mut batcher,
                        center + Vec3::new(0.0, climber_y + 4.0, 0.0),
                        7.5,
                        0.8,
                        theta,
                        col_acc,
                    );
                }
                // Beacons along tether
                beacons.push(SkylineBeacon {
                    position: center + Vec3::new(0.0, 160.0, 0.0),
                    color: Color::new(0.40, 0.85, 1.0, 1.0),
                    radius: 3.0,
                    pulse_freq: 3.0,
                    phase,
                });
                beacons.push(SkylineBeacon {
                    position: center + Vec3::new(0.0, 318.0, 0.0),
                    color: Color::new(1.0, 0.25, 0.25, 1.0),
                    radius: 3.8,
                    pulse_freq: 1.8,
                    phase: phase + 0.5,
                });
            }

            // Stepped Mega-Arcologies (Cyber Ziggurats, 6 sectors)
            2 | 8 | 14 | 20 | 26 | 32 => {
                // Tier 1
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 15.0, 0.0),
                    Vec3::new(30.0, 50.0, 20.0),
                    theta,
                    altitude_color,
                );
                add_neon_rim(
                    &mut batcher,
                    center + Vec3::new(0.0, 40.0, 0.0),
                    29.0,
                    1.2,
                    theta,
                    Color::new(0.05, 0.85, 1.0, 1.0),
                );

                // Tier 2
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 56.0, 0.0),
                    Vec3::new(22.0, 34.0, 15.0),
                    theta,
                    altitude_color,
                );
                add_neon_rim(
                    &mut batcher,
                    center + Vec3::new(0.0, 73.0, 0.0),
                    21.0,
                    1.2,
                    theta,
                    Color::new(0.98, 0.18, 0.60, 1.0),
                );

                // Tier 3
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 88.0, 0.0),
                    Vec3::new(14.0, 30.0, 10.0),
                    theta,
                    altitude_color,
                );

                // Crown Spire
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 114.0, 0.0),
                    Vec3::new(6.0, 24.0, 5.0),
                    theta,
                    altitude_color,
                );

                beacons.push(SkylineBeacon {
                    position: center + Vec3::new(0.0, 127.0, 0.0),
                    color: Color::new(1.0, 0.15, 0.15, 1.0),
                    radius: 2.5,
                    pulse_freq: 2.0,
                    phase,
                });
            }

            // Twin Spires with Aerial Skybridge (6 sectors)
            6 | 11 | 16 | 24 | 29 | 34 => {
                let h1 = 125.0f32;
                let h2 = 140.0f32;
                let p1 = center - tan * 6.5;
                let p2 = center + tan * 6.5;

                add_oriented_box(
                    &mut batcher,
                    p1 + Vec3::new(0.0, h1 * 0.5 - 10.0, 0.0),
                    Vec3::new(9.0, h1 + 20.0, 9.0),
                    theta,
                    altitude_color,
                );
                add_oriented_box(
                    &mut batcher,
                    p2 + Vec3::new(0.0, h2 * 0.5 - 10.0, 0.0),
                    Vec3::new(9.0, h2 + 20.0, 9.0),
                    theta,
                    altitude_color,
                );

                // Skybridge at y = 85m
                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, 85.0, 0.0),
                    Vec3::new(16.0, 5.0, 4.5),
                    theta,
                    altitude_color,
                );
                add_neon_rim(
                    &mut batcher,
                    center + Vec3::new(0.0, 87.5, 0.0),
                    14.0,
                    0.8,
                    theta,
                    Color::new(0.18, 0.95, 0.42, 1.0),
                );

                beacons.push(SkylineBeacon {
                    position: p1 + Vec3::new(0.0, h1 + 1.5, 0.0),
                    color: Color::new(1.0, 0.2, 0.2, 1.0),
                    radius: 2.2,
                    pulse_freq: 2.4,
                    phase,
                });
                beacons.push(SkylineBeacon {
                    position: p2 + Vec3::new(0.0, h2 + 1.5, 0.0),
                    color: Color::new(0.2, 0.8, 1.0, 1.0),
                    radius: 2.4,
                    pulse_freq: 2.8,
                    phase: phase + 1.0,
                });
            }

            // High-Density Monoliths & Refinery Pylons (Remaining 18 sectors)
            _ => {
                let h = 75.0 + ((s * 37) % 65) as f32;
                let w = 12.0 + ((s * 13) % 10) as f32;
                let d = 10.0 + ((s * 23) % 8) as f32;

                add_oriented_box(
                    &mut batcher,
                    center + Vec3::new(0.0, h * 0.5 - 10.0, 0.0),
                    Vec3::new(w, h + 20.0, d),
                    theta,
                    altitude_color,
                );

                // Accent neon band on every 2nd tower
                if s % 2 == 0 {
                    add_neon_rim(
                        &mut batcher,
                        center + Vec3::new(0.0, h * 0.75, 0.0),
                        w * 0.9,
                        1.0,
                        theta,
                        if s % 4 == 0 {
                            Color::new(0.05, 0.85, 1.0, 1.0)
                        } else {
                            Color::new(1.0, 0.60, 0.12, 1.0)
                        },
                    );
                }

                // Crown antenna and beacon
                beacons.push(SkylineBeacon {
                    position: center + Vec3::new(0.0, h + 1.5, 0.0),
                    color: Color::new(1.0, 0.35, 0.15, 1.0),
                    radius: 2.0,
                    pulse_freq: 1.9 + (s % 3) as f32 * 0.4,
                    phase,
                });
            }
        }
    }

    let beacon_count = beacons.len();
    DistantSkyline {
        base_sub_meshes: batcher.finish(),
        beacons,
        beacons_mesh: Mesh {
            vertices: Vec::with_capacity(beacon_count * 4),
            indices: Vec::with_capacity(beacon_count * 6),
            texture: None,
        },
    }
}

impl DistantSkyline {
    pub fn create_draw_buffers(&self) -> Vec<Mesh> {
        self.base_sub_meshes
            .iter()
            .map(|m| Mesh {
                vertices: m.vertices.clone(),
                indices: m.indices.clone(),
                texture: None,
            })
            .collect()
    }

    /// Batches all animated aviation warning beacons into a single camera-facing billboard quad mesh.
    pub fn update_beacons_mesh(&mut self, cam_pos: Vec3, time: f32) {
        let h_offset = Vec3::new(cam_pos.x, 0.0, cam_pos.z);
        self.beacons_mesh.vertices.clear();
        self.beacons_mesh.indices.clear();

        for beacon in &self.beacons {
            let pos = beacon.position + h_offset;
            let pulse = ((time * beacon.pulse_freq + beacon.phase).sin() * 0.35 + 0.65).clamp(0.25, 1.0);
            let r = beacon.radius * pulse;

            // Camera-facing billboard basis vectors
            let to_cam = cam_pos - pos;
            let fwd = if to_cam.length_squared() > 1e-4 {
                to_cam.normalize()
            } else {
                Vec3::Z
            };
            let right = {
                let cross = fwd.cross(Vec3::Y);
                if cross.length_squared() > 1e-4 {
                    cross.normalize()
                } else {
                    Vec3::X
                }
            };
            let up = right.cross(fwd).normalize();

            let rx = right * r;
            let uy = up * r;
            let pulse_col = Color::new(
                beacon.color.r,
                beacon.color.g,
                beacon.color.b,
                (beacon.color.a * (0.6 + 0.4 * pulse)).clamp(0.0, 1.0),
            );
            let col_raw: [u8; 4] = pulse_col.into();

            let base = self.beacons_mesh.vertices.len() as u16;
            self.beacons_mesh.vertices.push(Vertex { position: pos - rx - uy, uv: Vec2::new(0.0, 0.0), color: col_raw, normal: Vec4::ZERO });
            self.beacons_mesh.vertices.push(Vertex { position: pos + rx - uy, uv: Vec2::new(1.0, 0.0), color: col_raw, normal: Vec4::ZERO });
            self.beacons_mesh.vertices.push(Vertex { position: pos + rx + uy, uv: Vec2::new(1.0, 1.0), color: col_raw, normal: Vec4::ZERO });
            self.beacons_mesh.vertices.push(Vertex { position: pos - rx + uy, uv: Vec2::new(0.0, 1.0), color: col_raw, normal: Vec4::ZERO });
            self.beacons_mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub fn draw(&mut self, cam_pos: Vec3, draw_buffers: &mut [Mesh], time: f32) {
        let h_offset = Vec3::new(cam_pos.x, 0.0, cam_pos.z);

        // 1. Draw sub-meshes (horizontal tracking, constant distance, zero clipping)
        for (i, base_mesh) in self.base_sub_meshes.iter().enumerate() {
            if i < draw_buffers.len() {
                let buffer = &mut draw_buffers[i];
                if buffer.vertices.len() != base_mesh.vertices.len() {
                    buffer.vertices = base_mesh.vertices.clone();
                    buffer.indices = base_mesh.indices.clone();
                    buffer.texture = None;
                }
                for (v_idx, base_v) in base_mesh.vertices.iter().enumerate() {
                    buffer.vertices[v_idx].position = base_v.position + h_offset;
                }
                draw_mesh(buffer);
            }
        }

        // 2. Draw animated aviation warning beacons as 1 single batched billboard quad draw call
        self.update_beacons_mesh(cam_pos, time);
        if !self.beacons_mesh.indices.is_empty() {
            draw_mesh(&self.beacons_mesh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skyline_sub_batches_strictly_within_macroquad_limits() {
        let skyline = create_distant_skyline();
        assert!(
            !skyline.base_sub_meshes.is_empty(),
            "Skyline must contain at least 1 sub-mesh"
        );
        for (i, m) in skyline.base_sub_meshes.iter().enumerate() {
            assert!(
                m.vertices.len() <= MAX_BATCH_VERTICES,
                "Sub-mesh {} vertex count {} exceeds limit {}",
                i,
                m.vertices.len(),
                MAX_BATCH_VERTICES
            );
            assert!(
                m.indices.len() <= MAX_BATCH_INDICES,
                "Sub-mesh {} index count {} exceeds limit {}",
                i,
                m.indices.len(),
                MAX_BATCH_INDICES
            );
        }
    }

    #[test]
    fn test_skyline_beacons_populated_and_valid() {
        let skyline = create_distant_skyline();
        assert!(
            skyline.beacons.len() >= 24,
            "Must have at least 24 beacons around 360 ring, got {}",
            skyline.beacons.len()
        );
        for b in &skyline.beacons {
            assert!(b.position.y > 0.0, "Beacons must be above ground level");
            assert!(b.radius > 0.5 && b.radius < 10.0);
            assert!(b.pulse_freq > 0.5);
        }
    }

    #[test]
    fn test_skyline_stratospheric_orbital_elevators_reach_above_300m() {
        let skyline = create_distant_skyline();
        let max_beacon_y = skyline
            .beacons
            .iter()
            .map(|b| b.position.y)
            .fold(0.0f32, f32::max);
        assert!(
            max_beacon_y >= 300.0,
            "Stratospheric sky-elevator beacon must reach >= 300m, got {}",
            max_beacon_y
        );
    }

    #[test]
    fn test_skyline_batched_beacons_mesh() {
        let mut skyline = create_distant_skyline();
        let cam_pos = Vec3::new(12.0, 18.0, -25.0);
        let time = 3.5f32;

        skyline.update_beacons_mesh(cam_pos, time);

        let n = skyline.beacons.len();
        assert!(n >= 24, "Must have at least 24 beacons");
        assert_eq!(
            skyline.beacons_mesh.vertices.len(),
            n * 4,
            "Each beacon must emit exactly 4 vertices"
        );
        assert_eq!(
            skyline.beacons_mesh.indices.len(),
            n * 6,
            "Each beacon must emit exactly 6 indices (2 triangles)"
        );
        assert!(
            skyline.beacons_mesh.vertices.len() <= MAX_BATCH_VERTICES,
            "Beacon vertices must not exceed batch limit"
        );
        assert!(
            skyline.beacons_mesh.indices.len() <= MAX_BATCH_INDICES,
            "Beacon indices must not exceed batch limit"
        );

        // Verify index integrity
        let max_idx = skyline.beacons_mesh.vertices.len() as u16;
        for &idx in &skyline.beacons_mesh.indices {
            assert!(idx < max_idx, "Index {} out of bounds ({})", idx, max_idx);
        }

        // Verify repeated calls reuse buffer without allocation or leak
        let vert_cap = skyline.beacons_mesh.vertices.capacity();
        skyline.update_beacons_mesh(cam_pos + Vec3::new(5.0, 0.0, 5.0), time + 0.5);
        assert_eq!(skyline.beacons_mesh.vertices.len(), n * 4);
        assert_eq!(skyline.beacons_mesh.indices.len(), n * 6);
        assert_eq!(skyline.beacons_mesh.vertices.capacity(), vert_cap, "Buffer capacity must remain stable");
    }
}
