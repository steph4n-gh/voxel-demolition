//! ============================================================================
//! EMPIRICAL ADVERSARIAL STRESS TEST SUITE: MILESTONE G5-M3
//! Lighting, Rendering & UI Zero-Allocation Optimization
//! File Target: tests/g5_m3_challenger_1_stress.rs
//! Authored by: g5_m3_challenger_1 (Empirical Challenger)
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use macroquad::prelude::*;
use physics::DynamicPointLight;
use skyline::{create_distant_skyline, DistantSkyline, SkylineBeacon};
use std::fmt::Write;
use ui::{fit_text_to_width_stack, StackStr};
use voxel::VoxelGrid;
use weapon::{WeaponSystem, MAX_C4_CHARGES};

// ============================================================================
// PART 1: BATCHED SKYLINE BEACONS BILLBOARD MESH ADVERSARIAL STRESS
// ============================================================================

#[test]
fn test_beacons_mesh_scaling_counts_n24_n36_n64_n128() {
    let test_counts = [24, 36, 64, 128];
    for &n in &test_counts {
        let mut beacons = Vec::with_capacity(n);
        for i in 0..n {
            let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
            beacons.push(SkylineBeacon {
                position: Vec3::new(angle.cos() * 180.0, 45.0 + (i as f32 * 0.5), angle.sin() * 180.0),
                radius: 1.8,
                color: Color::new(1.0, 0.15, 0.1, 0.95),
                pulse_freq: 2.0 + (i as f32 * 0.1),
                phase: i as f32 * 0.3,
            });
        }

        let mut skyline = DistantSkyline {
            base_sub_meshes: Vec::new(),
            beacons: beacons.clone(),
            beacons_mesh: Mesh {
                vertices: Vec::with_capacity(n * 4),
                indices: Vec::with_capacity(n * 6),
                texture: None,
            },
        };

        let cam_pos = Vec3::new(32.5, 12.0, -45.0);
        let time = 10.5f32;
        skyline.update_beacons_mesh(cam_pos, time);

        assert_eq!(
            skyline.beacons_mesh.vertices.len(),
            n * 4,
            "Failed for N={}: vertex count must be exactly N * 4",
            n
        );
        assert_eq!(
            skyline.beacons_mesh.indices.len(),
            n * 6,
            "Failed for N={}: index count must be exactly N * 6",
            n
        );

        // Verify index bounding
        let max_vert = (n * 4) as u16;
        for &idx in &skyline.beacons_mesh.indices {
            assert!(
                idx < max_vert,
                "Index {} exceeds max vertex {} for N={}",
                idx,
                max_vert,
                n
            );
        }

        // Verify buffer capacity stability across repeated calls
        let vert_cap = skyline.beacons_mesh.vertices.capacity();
        let idx_cap = skyline.beacons_mesh.indices.capacity();
        for step in 1..=50 {
            let offset_cam = cam_pos + Vec3::new(step as f32 * 2.0, 0.0, step as f32 * -1.5);
            skyline.update_beacons_mesh(offset_cam, time + step as f32 * 0.05);
            assert_eq!(skyline.beacons_mesh.vertices.len(), n * 4);
            assert_eq!(skyline.beacons_mesh.indices.len(), n * 6);
            assert_eq!(
                skyline.beacons_mesh.vertices.capacity(),
                vert_cap,
                "Vertex buffer reallocated at step {} for N={}",
                step,
                n
            );
            assert_eq!(
                skyline.beacons_mesh.indices.capacity(),
                idx_cap,
                "Index buffer reallocated at step {} for N={}",
                step,
                n
            );
        }
    }
}

#[test]
fn test_beacons_mesh_extreme_camera_orientations_and_rotational_velocity() {
    let mut skyline = create_distant_skyline();
    let n = skyline.beacons.len();
    assert!(n >= 24, "Default skyline must have at least 24 beacons");

    // Extreme camera positions: deep underground, stratosphere, edge of coordinate space
    let extreme_positions = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, -10_000.0, 0.0),
        Vec3::new(0.0, 50_000.0, 0.0),
        Vec3::new(100_000.0, 500.0, 100_000.0),
        Vec3::new(-500_000.0, -250.0, 800_000.0),
        Vec3::new(1e6, 1e4, -1e6),
        Vec3::new(-1e6, -1e4, 1e6),
    ];

    for &cam_pos in &extreme_positions {
        skyline.update_beacons_mesh(cam_pos, 42.0);

        for (v_idx, v) in skyline.beacons_mesh.vertices.iter().enumerate() {
            assert!(
                v.position.x.is_finite() && v.position.y.is_finite() && v.position.z.is_finite(),
                "Vertex {} position {:?} contains NaN/Inf at cam_pos {:?}",
                v_idx,
                v.position,
                cam_pos
            );
            assert!(
                v.uv.x.is_finite() && v.uv.y.is_finite(),
                "Vertex {} uv {:?} contains NaN/Inf",
                v_idx,
                v.uv
            );
        }
    }

    // High rotational velocities (spinning at high angular velocity)
    let mut time = 0.0f32;
    for spin in 0..500 {
        let angle = spin as f32 * 250.0;
        let radius = 150.0;
        let cam_pos = Vec3::new(angle.cos() * radius, 30.0 + (angle * 0.1).sin() * 20.0, angle.sin() * radius);
        time += 0.016;

        skyline.update_beacons_mesh(cam_pos, time);

        assert_eq!(skyline.beacons_mesh.vertices.len(), n * 4);
        assert_eq!(skyline.beacons_mesh.indices.len(), n * 6);

        for v in &skyline.beacons_mesh.vertices {
            assert!(
                v.position.x.is_finite() && v.position.y.is_finite() && v.position.z.is_finite(),
                "Vertex has non-finite position under high rotational velocity"
            );
        }
    }
}

#[test]
fn test_beacons_mesh_parallel_offset_and_degenerate_vectors_no_nan() {
    let beacons = vec![
        // 1. Beacon directly along Y axis (x=0, z=0)
        SkylineBeacon {
            position: Vec3::new(0.0, 50.0, 0.0),
            radius: 2.0,
            color: Color::new(1.0, 0.0, 0.0, 1.0),
            pulse_freq: 1.0,
            phase: 0.0,
        },
        // 2. Beacon with offset identical to camera
        SkylineBeacon {
            position: Vec3::new(10.0, 20.0, -10.0),
            radius: 1.5,
            color: Color::new(0.0, 1.0, 0.0, 1.0),
            pulse_freq: 2.0,
            phase: 1.0,
        },
        // 3. Beacon with zero radius
        SkylineBeacon {
            position: Vec3::new(-50.0, 30.0, 50.0),
            radius: 0.0,
            color: Color::new(0.0, 0.0, 1.0, 1.0),
            pulse_freq: 1.5,
            phase: 2.0,
        },
    ];

    let mut skyline = DistantSkyline {
        base_sub_meshes: Vec::new(),
        beacons,
        beacons_mesh: Mesh {
            vertices: Vec::with_capacity(3 * 4),
            indices: Vec::with_capacity(3 * 6),
            texture: None,
        },
    };

    // Test cases where camera is directly parallel, above, below, or on top of beacon
    let test_cams = [
        Vec3::new(0.0, 50.0, 0.0),          // exact position as beacon 0
        Vec3::new(0.0, 1000.0, 0.0),        // directly above beacon 0 along Y
        Vec3::new(0.0, -1000.0, 0.0),       // directly below beacon 0 along -Y
        Vec3::new(10.0, 20.0, -10.0),       // exact position as beacon 1
        Vec3::new(0.0, 50.0 + 1e-5, 0.0),   // micro epsilon along Y
        Vec3::new(1e-5, 50.0, 0.0),         // micro epsilon along X
        Vec3::new(0.0, 50.0, 1e-5),         // micro epsilon along Z
    ];

    for &cam_pos in &test_cams {
        skyline.update_beacons_mesh(cam_pos, 1.0);

        for (idx, v) in skyline.beacons_mesh.vertices.iter().enumerate() {
            assert!(
                !v.position.x.is_nan() && !v.position.y.is_nan() && !v.position.z.is_nan(),
                "Vertex {} position {:?} contains NaN for cam_pos {:?}",
                idx,
                v.position,
                cam_pos
            );
            assert!(
                !v.position.x.is_infinite() && !v.position.y.is_infinite() && !v.position.z.is_infinite(),
                "Vertex {} position {:?} is infinite for cam_pos {:?}",
                idx,
                v.position,
                cam_pos
            );
        }
    }
}

#[test]
fn test_beacons_mesh_quad_geometry_and_orthonormality() {
    let mut skyline = create_distant_skyline();
    let cam_pos = Vec3::new(15.0, 25.0, -40.0);
    skyline.update_beacons_mesh(cam_pos, 0.0);

    for (b_idx, beacon) in skyline.beacons.iter().enumerate() {
        let base = b_idx * 4;
        let v0 = skyline.beacons_mesh.vertices[base].position;
        let v1 = skyline.beacons_mesh.vertices[base + 1].position;
        let v2 = skyline.beacons_mesh.vertices[base + 2].position;
        let v3 = skyline.beacons_mesh.vertices[base + 3].position;

        // Verify planar rectangle: diagonal lengths should be equal
        let diag1 = (v2 - v0).length();
        let diag2 = (v3 - v1).length();
        assert!(
            (diag1 - diag2).abs() < 1e-3,
            "Beacon {} quad is non-rectangular: diag1={}, diag2={}",
            b_idx,
            diag1,
            diag2
        );

        // Verify side lengths match 2 * r
        let side1 = (v1 - v0).length();
        let side2 = (v2 - v1).length();
        assert!(
            (side1 - side2).abs() < 1e-3,
            "Beacon {} quad is not square: side1={}, side2={}",
            b_idx,
            side1,
            side2
        );
        let center = (v0 + v1 + v2 + v3) * 0.25;
        let expected_pos = beacon.position + Vec3::new(cam_pos.x, 0.0, cam_pos.z);
        assert!(
            (center - expected_pos).length() < 1e-3,
            "Beacon {} center mismatch: center={:?}, expected={:?}",
            b_idx,
            center,
            expected_pos
        );
    }
}

// ============================================================================
// PART 2: VOLUMETRIC SCRATCH MESHES EMPIRICAL STRESS
// ============================================================================

/// Exact geometry generation for volumetric god rays (mirroring src/main.rs:275-354)
fn generate_god_rays_geometry(
    cam_pos: Vec3,
    final_dir: Vec3,
    sun_dir: Vec3,
    grid: &VoxelGrid,
    mesh: &mut Mesh,
) {
    mesh.vertices.clear();
    mesh.indices.clear();

    let sun_view_dot = final_dir.dot(sun_dir);
    if sun_view_dot < -0.15 {
        return;
    }
    let intensity = (sun_view_dot + 0.15).clamp(0.0, 1.0);
    let beam_color = Color::new(1.0, 0.92, 0.72, 0.045 * intensity);
    let c_raw: [u8; 4] = beam_color.into();

    let push_quad = |m: &mut Mesh, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [u8; 4]| {
        let base = m.vertices.len() as u16;
        m.vertices.push(Vertex { position: a, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: b, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: c, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: d, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    let ray_anchors = [
        Vec3::new(0.0, 48.0, 0.0),
        Vec3::new(-6.0, 36.0, -4.0),
        Vec3::new(6.0, 42.0, 4.0),
        Vec3::new(-10.0, 24.0, 8.0),
        Vec3::new(10.0, 30.0, -8.0),
        Vec3::new(0.0, 60.0, 0.0),
        Vec3::new(-4.0, 18.0, 2.0),
        Vec3::new(4.0, 22.0, -2.0),
    ];

    let perp1 = sun_dir.cross(Vec3::Y).normalize_or_zero();
    let perp2 = sun_dir.cross(perp1).normalize_or_zero();

    for &anchor in &ray_anchors {
        let test_voxel = grid.world_to_voxel(anchor);
        if grid.in_bounds(test_voxel.x, test_voxel.y, test_voxel.z) && grid.get(test_voxel.x, test_voxel.y, test_voxel.z).is_solid() {
            continue;
        }

        let p_top = anchor + sun_dir * 38.0;
        let p_bot = anchor - sun_dir * 28.0;
        let r = 2.4;
        let q1_a = p_top - perp1 * r;
        let q1_b = p_top + perp1 * r;
        let q1_c = p_bot + perp1 * (r * 1.7);
        let q1_d = p_bot - perp1 * (r * 1.7);

        let q2_a = p_top - perp2 * r;
        let q2_b = p_top + perp2 * r;
        let q2_c = p_bot + perp2 * (r * 1.7);
        let q2_d = p_bot - perp2 * (r * 1.7);

        push_quad(mesh, q1_a, q1_b, q1_c, q1_d, c_raw);
        push_quad(mesh, q2_a, q2_b, q2_c, q2_d, c_raw);
    }

    if sun_view_dot > 0.4 {
        let flare_alpha = ((sun_view_dot - 0.4) * 0.12).clamp(0.0, 0.08);
        let flare_col = Color::new(1.0, 0.94, 0.82, flare_alpha);
        let flare_raw: [u8; 4] = flare_col.into();
        let sun_pos = cam_pos + sun_dir * 210.0;
        let f_r = perp1 * 35.0;
        let f_u = perp2 * 35.0;
        push_quad(mesh, sun_pos - f_r, sun_pos + f_u, sun_pos + f_r, sun_pos - f_u, flare_raw);
    }
}

/// Exact geometry generation for emissive bloom coronas (mirroring src/main.rs:356-413)
fn generate_bloom_coronas_geometry(
    cam_pos: Vec3,
    final_dir: Vec3,
    final_right: Vec3,
    final_up: Vec3,
    lights: &[DynamicPointLight],
    mesh: &mut Mesh,
) {
    mesh.vertices.clear();
    mesh.indices.clear();

    let push_quad = |m: &mut Mesh, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [u8; 4]| {
        let base = m.vertices.len() as u16;
        m.vertices.push(Vertex { position: a, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: b, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: c, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.vertices.push(Vertex { position: d, uv: Vec2::ZERO, color: col, normal: Vec4::ZERO });
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    for light in lights {
        if light.intensity < 0.65 {
            continue;
        }
        let to_light = light.position - cam_pos;
        let dist = to_light.length();
        if !(1.0..=90.0).contains(&dist) {
            continue;
        }
        let dir = to_light / dist;
        if dir.dot(final_dir) < 0.2 {
            continue;
        }
        let size = (light.radius * 0.18 * light.intensity).min(4.0);
        let alpha = (light.intensity * 0.30).min(0.60);
        let corona_col = Color::new(light.color.r, light.color.g, light.color.b, alpha);
        let c_raw: [u8; 4] = corona_col.into();
        let p = light.position;

        // Diamond corona flare
        let r = final_right * size;
        let u = final_up * size;
        push_quad(mesh, p - r, p + u, p + r, p - u, c_raw);

        // Cross flare spikes
        let spike_r = final_right * (size * 2.2);
        let spike_u = final_up * (size * 0.16);
        push_quad(mesh, p - spike_r, p + spike_u, p + spike_r, p - spike_u, c_raw);

        let spike2_u = final_up * (size * 2.2);
        let spike2_r = final_right * (size * 0.16);
        push_quad(mesh, p - spike2_r, p + spike2_u, p + spike2_r, p - spike2_u, c_raw);
    }
}

#[test]
fn test_god_rays_scratch_mesh_never_reallocates_under_worst_case_sun() {
    let mut god_rays_mesh = Mesh {
        vertices: Vec::with_capacity(128),
        indices: Vec::with_capacity(192),
        texture: None,
    };

    let initial_vert_cap = god_rays_mesh.vertices.capacity();
    let initial_idx_cap = god_rays_mesh.indices.capacity();
    let vert_ptr = god_rays_mesh.vertices.as_ptr();
    let idx_ptr = god_rays_mesh.indices.as_ptr();

    let grid = VoxelGrid::new(); // Empty grid: all 8 apertures are unoccluded!
    let sun_dir = Vec3::new(-0.24, 0.38, -0.89).normalize();
    let cam_pos = Vec3::new(0.0, 20.0, 0.0);

    // 1. Worst case: facing DIRECTLY into sun (sun_view_dot = 1.0)
    let final_dir = sun_dir;
    generate_god_rays_geometry(cam_pos, final_dir, sun_dir, &grid, &mut god_rays_mesh);

    // Max geometry: 8 anchors * 2 quads * 4 verts + 1 sun flare quad * 4 verts = 68 vertices
    // Max indices: 8 anchors * 2 quads * 6 indices + 1 sun flare quad * 6 indices = 102 indices
    assert_eq!(god_rays_mesh.vertices.len(), 68, "All 8 apertures + radial sun flare must be emitted");
    assert_eq!(god_rays_mesh.indices.len(), 102, "68 verts -> 102 indices");

    // Capacity must remain strictly 128 / 192, and buffer pointers MUST remain identical (zero allocations)!
    assert_eq!(god_rays_mesh.vertices.capacity(), initial_vert_cap);
    assert_eq!(god_rays_mesh.indices.capacity(), initial_idx_cap);
    assert_eq!(god_rays_mesh.vertices.as_ptr(), vert_ptr, "Vertex buffer must not reallocate");
    assert_eq!(god_rays_mesh.indices.as_ptr(), idx_ptr, "Index buffer must not reallocate");

    // 2. Continuous sweep of 2,000 camera angles
    for step in 0..2000 {
        let angle = step as f32 * 0.01;
        let test_dir = Vec3::new(angle.cos(), (angle * 0.5).sin(), angle.sin()).normalize();
        generate_god_rays_geometry(cam_pos, test_dir, sun_dir, &grid, &mut god_rays_mesh);

        assert!(god_rays_mesh.vertices.len() <= 68);
        assert!(god_rays_mesh.indices.len() <= 102);
        assert_eq!(god_rays_mesh.vertices.capacity(), initial_vert_cap);
        assert_eq!(god_rays_mesh.indices.capacity(), initial_idx_cap);
        assert_eq!(god_rays_mesh.vertices.as_ptr(), vert_ptr);
        assert_eq!(god_rays_mesh.indices.as_ptr(), idx_ptr);
    }
}

#[test]
fn test_bloom_coronas_scratch_mesh_saturation_and_zero_reallocation_once_warmed() {
    let mut bloom_mesh = Mesh {
        vertices: Vec::with_capacity(256),
        indices: Vec::with_capacity(384),
        texture: None,
    };

    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let final_dir = Vec3::Z;
    let final_right = Vec3::X;
    let final_up = Vec3::Y;

    // Simulate intense battlefield conditions: 24 active dynamic lights (max engine limit)
    // all directly in front of camera at high intensity and optimal distance
    let mut lights = Vec::with_capacity(24);
    for i in 0..24 {
        lights.push(DynamicPointLight {
            position: cam_pos + Vec3::new((i as f32 - 12.0) * 1.5, 0.0, 10.0 + i as f32 * 2.0),
            color: Color::new(1.0, 0.6, 0.2, 1.0),
            radius: 8.0,
            intensity: 2.5,
        });
    }

    // First frame under maximum 24-light saturation:
    // 24 lights * 3 quads * 4 verts = 288 vertices (> 256 initial capacity)
    // 24 lights * 3 quads * 6 indices = 432 indices (> 384 initial capacity)
    generate_bloom_coronas_geometry(cam_pos, final_dir, final_right, final_up, &lights, &mut bloom_mesh);

    assert_eq!(bloom_mesh.vertices.len(), 288);
    assert_eq!(bloom_mesh.indices.len(), 432);

    // The scratch mesh has warmed to its peak capacity
    let warmed_vert_cap = bloom_mesh.vertices.capacity();
    let warmed_idx_cap = bloom_mesh.indices.capacity();
    let warmed_vert_ptr = bloom_mesh.vertices.as_ptr();
    let warmed_idx_ptr = bloom_mesh.indices.as_ptr();

    assert!(warmed_vert_cap >= 288, "Warmed vertex capacity must accommodate 288 vertices");
    assert!(warmed_idx_cap >= 432, "Warmed index capacity must accommodate 432 indices");

    // Stress test across 5,000 simulated explosive combat frames
    // dynamically pulsing intensities, moving positions, and toggling lights
    for frame in 0..5000 {
        let count = 1 + (frame % 24); // varying from 1 to 24 lights
        for (i, light) in lights.iter_mut().enumerate().take(count) {
            light.intensity = 0.5 + ((frame as f32 * 0.1 + i as f32).sin() * 0.5 + 0.5) * 3.0;
            light.position = cam_pos + Vec3::new(
                ((frame + i) as f32 * 0.05).sin() * 15.0,
                ((frame + i) as f32 * 0.03).cos() * 5.0,
                5.0 + (i as f32 * 2.5),
            );
        }

        generate_bloom_coronas_geometry(
            cam_pos,
            final_dir,
            final_right,
            final_up,
            &lights[..count],
            &mut bloom_mesh,
        );

        assert!(bloom_mesh.vertices.len() <= 288);
        assert!(bloom_mesh.indices.len() <= 432);

        // ZERO ALLOCATIONS: capacity and backing pointers MUST NEVER change once warmed!
        assert_eq!(
            bloom_mesh.vertices.capacity(),
            warmed_vert_cap,
            "Reallocation detected in vertex buffer at frame {}",
            frame
        );
        assert_eq!(
            bloom_mesh.indices.capacity(),
            warmed_idx_cap,
            "Reallocation detected in index buffer at frame {}",
            frame
        );
        assert_eq!(
            bloom_mesh.vertices.as_ptr(),
            warmed_vert_ptr,
            "Backing vector pointer shifted at frame {}",
            frame
        );
        assert_eq!(
            bloom_mesh.indices.as_ptr(),
            warmed_idx_ptr,
            "Backing vector pointer shifted at frame {}",
            frame
        );

        // Verify index integrity: all indices must be strictly valid
        let max_v = bloom_mesh.vertices.len() as u16;
        for &idx in &bloom_mesh.indices {
            assert!(idx < max_v, "Index {} out of bounds for len {}", idx, max_v);
        }
    }
}

#[test]
fn test_bloom_coronas_u16_overflow_protection_under_extreme_lights() {
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(1024),
        indices: Vec::with_capacity(1536),
        texture: None,
    };

    let cam_pos = Vec3::ZERO;
    let final_dir = Vec3::Z;
    let final_right = Vec3::X;
    let final_up = Vec3::Y;

    // Create 100 lights (far above 24 engine cap)
    let lights: Vec<_> = (0..100)
        .map(|i| DynamicPointLight {
            position: Vec3::new(0.0, 0.0, 5.0 + i as f32 * 0.5),
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            radius: 5.0,
            intensity: 1.5,
        })
        .collect();

    generate_bloom_coronas_geometry(cam_pos, final_dir, final_right, final_up, &lights, &mut mesh);

    assert_eq!(mesh.vertices.len(), 100 * 12);
    assert_eq!(mesh.indices.len(), 100 * 18);

    // Verify all emitted indices are strictly valid
    let max_v = mesh.vertices.len() as u16;
    for &idx in &mesh.indices {
        assert!(idx < max_v);
    }
}

// ============================================================================
// PART 3: ZERO-ALLOCATION `StackStr<N>` HUD FORMATTING ADVERSARIAL STRESS
// ============================================================================

#[test]
fn test_stack_str_multibyte_utf8_truncation_adversarial() {
    // 1-byte, 2-byte, 3-byte, and 4-byte characters
    let complex_text = "Voxel 爆発 💥 αβγδε 12345 💣🔥";

    // Test across various tight stack capacities: N = 1, 2, 3, 4, 5, 8, 11, 16, 25, 32, 64
    let mut s1 = StackStr::<1>::new();
    let _ = write!(s1, "{}", complex_text);
    assert!(std::str::from_utf8(s1.as_bytes()).is_ok(), "s1 must be valid UTF-8");
    assert_eq!(s1.as_str(), "V");

    let mut s4 = StackStr::<4>::new();
    let _ = write!(s4, "{}", complex_text);
    assert!(std::str::from_utf8(s4.as_bytes()).is_ok(), "s4 must be valid UTF-8");
    assert_eq!(s4.as_str(), "Voxe");

    let mut s7 = StackStr::<7>::new();
    let _ = write!(s7, "{}", complex_text);
    assert!(std::str::from_utf8(s7.as_bytes()).is_ok(), "s7 must be valid UTF-8");
    assert_eq!(s7.as_str(), "Voxel ");

    // At capacity 8: "Voxel " (6 bytes). Next char is '爆' (3 bytes: E7 88 86).
    // Adding 3 bytes would need 9 bytes > 8 bytes.
    // It must NOT write a partial character!
    let mut s8 = StackStr::<8>::new();
    let _ = write!(s8, "{}", complex_text);
    assert!(std::str::from_utf8(s8.as_bytes()).is_ok(), "s8 must be valid UTF-8");
    assert_eq!(s8.as_str(), "Voxel ", "Must cleanly reject 3-byte char that overflows buffer");

    // At capacity 9: can fit "Voxel " (6) + '爆' (3) = 9 bytes!
    let mut s9 = StackStr::<9>::new();
    let _ = write!(s9, "{}", complex_text);
    assert!(std::str::from_utf8(s9.as_bytes()).is_ok());
    assert_eq!(s9.as_str(), "Voxel 爆");

    // Test 4-byte emoji boundary: "💥" is 4 bytes (F0 9F 92 A5)
    let emoji_text = "ABC💥DEF";
    let mut s_emoji_fail = StackStr::<6>::new(); // 3 bytes + 4 bytes = 7 > 6
    let _ = write!(s_emoji_fail, "{}", emoji_text);
    assert_eq!(s_emoji_fail.as_str(), "ABC", "Must not truncate mid-emoji");

    let mut s_emoji_pass = StackStr::<7>::new(); // 3 bytes + 4 bytes = 7
    let _ = write!(s_emoji_pass, "{}", emoji_text);
    assert_eq!(s_emoji_pass.as_str(), "ABC💥", "Must fit complete emoji");
}

#[test]
fn test_stack_str_high_throughput_reuse_loop() {
    let mut buf = StackStr::<64>::new();
    for i in 0..100_000 {
        buf.clear();
        let _ = write!(buf, "PULSE: {:04} | FPS: {:03.1} | REBAR: {:02.1}%", i % 1000, 59.9, 98.4);
        assert!(buf.len() > 20);
        assert!(buf.len() <= 64);
        assert!(!buf.as_str().is_empty());
    }
}

#[test]
fn test_fit_text_to_width_stack_boundary_conditions() {
    use ui::fit_text_to_width;

    // Normal ASCII fitting
    let res = fit_text_to_width_stack::<32>("Target Locked", 200.0, 16.0);
    assert_eq!(res.as_str(), "Target Locked");
    assert_eq!(res.as_str(), fit_text_to_width("Target Locked", 200.0, 16.0));

    // Very narrow max_width forcing ellipsis
    let res_narrow = fit_text_to_width_stack::<32>("Extremely Long Tactical Weapon Designation", 60.0, 16.0);
    assert!(res_narrow.as_str().ends_with("..."), "Must truncate with ellipsis");
    assert!(res_narrow.len() <= 32);
    assert_eq!(res_narrow.as_str(), fit_text_to_width("Extremely Long Tactical Weapon Designation", 60.0, 16.0));

    // Japanese / multibyte characters with tight width
    let res_jp = fit_text_to_width_stack::<32>("超弩級指向性爆砕兵器シミュレーション", 80.0, 16.0);
    assert!(res_jp.as_str().ends_with("..."));
    assert!(std::str::from_utf8(res_jp.as_bytes()).is_ok(), "Must maintain valid UTF-8");
    assert_eq!(res_jp.as_str(), fit_text_to_width("超弩級指向性爆砕兵器シミュレーション", 80.0, 16.0));

    // Boundary width sweep asserting 100% equivalence between StackStr and heap String versions
    let test_widths = [0.0, 5.0, 10.0, 20.0, 30.0, 50.0, 80.0, 120.0, 250.0];
    let sample = "STRUCTURAL INTEGRITY FAILURE IMMINENT";
    for &w in &test_widths {
        let stack_res = fit_text_to_width_stack::<64>(sample, w, 14.0);
        let heap_res = fit_text_to_width(sample, w, 14.0);
        assert_eq!(
            stack_res.as_str(),
            heap_res.as_str(),
            "Mismatch at max_width={}",
            w
        );
        assert!(std::str::from_utf8(stack_res.as_bytes()).is_ok());
    }

    // Empty text
    let res_empty = fit_text_to_width_stack::<32>("", 100.0, 16.0);
    assert_eq!(res_empty.as_str(), "");
    assert_eq!(res_empty.as_str(), fit_text_to_width("", 100.0, 16.0));
}

// ============================================================================
// PART 4: LEVEL RESET C4 DET-CORD TOPOLOGY CLEANUP STRESS
// ============================================================================

#[test]
fn test_weapon_clear_c4_wipes_cached_det_cords_completely() {
    let mut weapon = WeaponSystem::new();

    // Place 24 C4 charges across 3D space
    for i in 0..MAX_C4_CHARGES {
        let pos = Vec3::new(
            (i as f32 * 1.5).cos() * 15.0,
            2.0 + (i as f32 * 0.8),
            (i as f32 * 1.5).sin() * 15.0,
        );
        let normal = Vec3::Y;
        let placed = weapon.place_c4_charge(pos, normal, None);
        assert!(placed, "Must successfully place charge {}", i);
    }
    assert_eq!(weapon.c4_charges.len(), MAX_C4_CHARGES);

    // Verify cached det-cord topology is populated
    assert!(!weapon.cached_det_cords.is_empty(), "Laser det cords must be computed");
    let cord_count = weapon.cached_det_cords.len();
    assert!(cord_count >= MAX_C4_CHARGES - 1);
    assert!(!weapon.det_cords_dirty);

    // Simulate level reload: call clear_c4() (Feature Level Reset Cleanup)
    weapon.clear_c4();

    // Verify all C4 charges and cached topology are completely wiped
    assert!(weapon.c4_charges.is_empty(), "C4 charges must be cleared");
    assert!(weapon.cached_det_cords.is_empty(), "Cached det cords must be cleared immediately");
    assert!(!weapon.det_cords_dirty, "det_cords_dirty must be reset to false");

    // Verify placing a single new charge does not connect to stale det cords
    let placed_after = weapon.place_c4_charge(Vec3::new(5.0, 1.0, 5.0), Vec3::Y, None);
    assert!(placed_after);
    assert_eq!(weapon.c4_charges.len(), 1);
    // Single charge cannot form det-cord connections
    assert!(weapon.cached_det_cords.is_empty(), "Single charge must have 0 det cords");
}
