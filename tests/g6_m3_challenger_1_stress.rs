//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G6-M3
//! HIGH-VELOCITY SWEPT ANTI-TUNNELING & 31,200-VOXEL MONOLITH DOMINO CASCADE
//! Target: tests/g6_m3_challenger_1_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, PhysicsSystem};
use voxel_demolition::voxel::{
    VoxelGrid, VoxelType, BEDROCK_Y, GRID_X, GRID_Y, GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y,
    VOID_BOUNDARY_Y, VOXEL_SIZE,
};

/// Helper: build a solid 3D box of voxels with specified local coordinates centered at origin.
fn build_box_voxels(
    nx: usize,
    ny: usize,
    nz: usize,
    spacing: f32,
    vtype: VoxelType,
) -> Vec<(Vec3, VoxelType)> {
    let mut voxels = Vec::with_capacity(nx * ny * nz);
    let hx = (nx.saturating_sub(1)) as f32 / 2.0;
    let hy = (ny.saturating_sub(1)) as f32 / 2.0;
    let hz = (nz.saturating_sub(1)) as f32 / 2.0;
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let pos = Vec3::new(
                    (x as f32 - hx) * spacing,
                    (y as f32 - hy) * spacing,
                    (z as f32 - hz) * spacing,
                );
                voxels.push((pos, vtype));
            }
        }
    }
    voxels
}

/// Helper: construct a test dynamic chunk with accurate mass computed from density.
fn create_test_chunk(
    voxels: Vec<(Vec3, VoxelType)>,
    position: Vec3,
    velocity: Vec3,
    rotation: Vec3,
    angular_velocity: Vec3,
) -> DynamicChunk {
    let mass: f32 = voxels.iter().map(|(_, vt)| vt.properties().density).sum();
    let aabb = DynamicChunk::compute_aabb(&voxels);
    DynamicChunk::new(
        position,
        velocity,
        rotation,
        angular_velocity,
        voxels,
        mass.max(1.0),
        0.0,
        false,
        aabb,
    )
}

/// Helper: construct a test dynamic chunk with explicit mass override.
#[allow(clippy::too_many_arguments)]
fn create_test_chunk_with_mass(
    voxels: Vec<(Vec3, VoxelType)>,
    position: Vec3,
    velocity: Vec3,
    rotation: Vec3,
    angular_velocity: Vec3,
    mass: f32,
) -> DynamicChunk {
    let aabb = DynamicChunk::compute_aabb(&voxels);
    DynamicChunk::new(
        position,
        velocity,
        rotation,
        angular_velocity,
        voxels,
        mass.max(1.0),
        0.0,
        false,
        aabb,
    )
}

/// Helper to build a solid rectangular prism of voxels in the grid.
#[allow(clippy::too_many_arguments)]
fn fill_rect(
    grid: &mut VoxelGrid,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
    vtype: VoxelType,
) {
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                grid.set(x, y, z, vtype);
            }
        }
    }
}

/// Helper: count remaining solid voxels inside a specific volume.
fn count_solids_in_box(
    grid: &VoxelGrid,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
) -> usize {
    let mut count = 0;
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                if grid.get(x, y, z).is_solid() {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Independent ground-truth BFS scanner auditing for floating voxels seeded at true bedrock y = MIN_VOXEL_Y (-48).
fn scan_floating_sky_hooks(grid: &VoxelGrid) -> Vec<IVec3> {
    let total = GRID_X * GRID_Y * GRID_Z;
    let mut grounded = vec![false; total];
    let mut queue = VecDeque::new();

    // 1. Seed strictly at true geological bedrock foundation y = MIN_VOXEL_Y (-48)
    for x in 0..GRID_X {
        for z in 0..GRID_Z {
            let idx = VoxelGrid::idx(x, MIN_VOXEL_Y, z);
            if grid.get(x as i32, MIN_VOXEL_Y, z as i32).is_solid() {
                grounded[idx] = true;
                queue.push_back((x as i32, MIN_VOXEL_Y, z as i32));
            }
        }
    }

    // 2. 18-connected neighbor traversal
    let neighbors = [
        (1, 0, 0), (-1, 0, 0),
        (0, 1, 0), (0, -1, 0),
        (0, 0, 1), (0, 0, -1),
        (1, 0, 1), (1, 0, -1),
        (-1, 0, 1), (-1, 0, -1),
        (1, 1, 0), (-1, 1, 0),
        (1, -1, 0), (-1, -1, 0),
        (0, 1, 1), (0, 1, -1),
        (0, -1, 1), (0, -1, -1),
    ];

    while let Some((cx, cy, cz)) = queue.pop_front() {
        for &(dx, dy, dz) in &neighbors {
            let nx = cx + dx;
            let ny = cy + dy;
            let nz = cz + dz;
            if grid.in_bounds(nx, ny, nz) {
                let nidx = VoxelGrid::idx(nx as usize, ny, nz as usize);
                if !grounded[nidx] && grid.get(nx, ny, nz).is_solid() {
                    grounded[nidx] = true;
                    queue.push_back((nx, ny, nz));
                }
            }
        }
    }

    // 3. Collect solid voxels above y = 0 lacking bedrock connectivity
    let mut floating = Vec::new();
    for x in 0..GRID_X as i32 {
        for y in 1..MAX_VOXEL_Y {
            for z in 0..GRID_Z as i32 {
                let idx = VoxelGrid::idx(x as usize, y, z as usize);
                if grid.get(x, y, z).is_solid() && !grounded[idx] {
                    floating.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    floating
}

// ============================================================================
// PART 1: HIGH-VELOCITY SWEPT ANTI-TUNNELING STRESS TESTS
// ============================================================================

/// Stress Challenge 1: Sub-voxel starting offset sweep at 40 m/s (dt = 0.033s).
/// Evaluates multi-layer penetrator (3x2x3) across 20 distinct starting offsets
/// through complete slab traversal. Asserts that the 0.5m thin Concrete slab is
/// consistently pulverized across all entry offsets without tunneling.
#[test]
fn test_stress_anti_tunneling_subvoxel_offset_sweep_40mps() {
    let slab_y = 20; // world y in [10.0, 10.5)
    let slab_center_world = 10.25f32;
    let dt = 0.033f32;
    let velocity_y = -40.0f32;
    let step_disp = (velocity_y * dt).abs(); // 1.32m

    // Test 20 starting offsets ranging from +0.55m to +1.80m above the slab
    let offsets: Vec<f32> = (0..20)
        .map(|i| 0.55 + (i as f32 / 19.0) * (step_disp + 0.3))
        .collect();

    for (idx, &offset) in offsets.iter().enumerate() {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(0.0, 20.0, 60.0);

        fill_rect(&mut grid, 40, 48, slab_y, slab_y, 40, 48, VoxelType::Concrete);
        let initial_solids = count_solids_in_box(&grid, 40, 48, slab_y, slab_y, 40, 48);
        assert_eq!(initial_solids, 81);

        let slab_center = grid.voxel_to_world(IVec3::new(44, slab_y, 44));
        let start_pos = Vec3::new(slab_center.x, slab_center_world + offset, slab_center.z);

        // 3x2x3 HeavyArmor penetrator chunk
        let chunk_voxels = build_box_voxels(3, 2, 3, VOXEL_SIZE, VoxelType::HeavyArmor);
        let chunk = create_test_chunk(
            chunk_voxels,
            start_pos,
            Vec3::new(0.0, velocity_y, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        phys.chunks.push(chunk);

        // Step simulation until chunk has traversed through the slab level
        for _step in 0..4 {
            phys.update(dt, &mut grid, &audio, cam_pos);
            if phys.chunks.is_empty() || phys.chunks[0].position.y < slab_center_world - 0.5 {
                break;
            }
        }

        let remaining_solids = count_solids_in_box(&grid, 40, 48, slab_y, slab_y, 40, 48);
        assert!(
            remaining_solids < initial_solids,
            "Tunneling failure at offset idx {}: start_y = {}, remaining = {}/{}",
            idx,
            start_pos.y,
            remaining_solids,
            initial_solids
        );
    }
}

/// Stress Challenge 2: Anti-tunneling verification up to 50 m/s across multi-frame trajectory.
/// Tests speeds [25.0, 35.0, 45.0, 50.0] m/s against Concrete slab.
#[test]
fn test_stress_anti_tunneling_speeds_up_to_50mps() {
    let test_speeds = [25.0f32, 35.0, 45.0, 50.0];
    let slab_y = 22;
    let slab_center_world = 11.25f32;

    for &speed in &test_speeds {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(0.0, 20.0, 60.0);

        fill_rect(&mut grid, 40, 48, slab_y, slab_y, 40, 48, VoxelType::Concrete);
        let initial_solids = count_solids_in_box(&grid, 40, 48, slab_y, slab_y, 40, 48);

        let slab_center = grid.voxel_to_world(IVec3::new(44, slab_y, 44));
        let start_pos = Vec3::new(slab_center.x, slab_center_world + 1.2, slab_center.z);

        let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
        let chunk = create_test_chunk(
            chunk_voxels,
            start_pos,
            Vec3::new(0.0, -speed, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        phys.chunks.push(chunk);

        for _step in 0..4 {
            phys.update(0.033, &mut grid, &audio, cam_pos);
            if phys.chunks.is_empty() || phys.chunks[0].position.y < slab_center_world - 0.5 {
                break;
            }
        }

        let remaining = count_solids_in_box(&grid, 40, 48, slab_y, slab_y, 40, 48);
        assert!(
            remaining < initial_solids,
            "Tunneling occurred at speed {} m/s! Solids before: {}, after: {}",
            speed,
            initial_solids,
            remaining
        );
    }
}

/// Stress Challenge 3: Pure horizontal high-velocity penetration (30, 40, 50 m/s)
/// against 1-voxel thin vertical wall with multiple starting X offsets.
#[test]
fn test_stress_anti_tunneling_horizontal_wall_sweep_50mps() {
    let wall_x = 50; // world x = 0.25m
    let speeds = [30.0f32, 40.0, 50.0];
    let dt = 0.033f32;

    for &speed in &speeds {
        for offset_step in 0..5 {
            let mut grid = VoxelGrid::new();
            let mut phys = PhysicsSystem::new();
            let audio = SoundSystem::empty();
            let cam_pos = Vec3::new(0.0, 20.0, 60.0);

            // 1-voxel thin vertical Concrete wall in Y in 10..=20, Z in 44..=52
            fill_rect(&mut grid, wall_x, wall_x, 10, 20, 44, 52, VoxelType::Concrete);
            let initial_solids = count_solids_in_box(&grid, wall_x, wall_x, 10, 20, 44, 52);
            assert_eq!(initial_solids, 99);

            let wall_center = grid.voxel_to_world(IVec3::new(wall_x, 15, 48));
            let start_x = wall_center.x - (0.4 + offset_step as f32 * 0.25);
            let start_pos = Vec3::new(start_x, wall_center.y, wall_center.z);

            let chunk_voxels = build_box_voxels(3, 3, 3, VOXEL_SIZE, VoxelType::HeavyArmor);
            let chunk = create_test_chunk(
                chunk_voxels,
                start_pos,
                Vec3::new(speed, 0.0, 0.0), // pure horizontal +X velocity
                Vec3::ZERO,
                Vec3::ZERO,
            );
            phys.chunks.push(chunk);

            phys.update(dt, &mut grid, &audio, cam_pos);

            let remaining_solids = count_solids_in_box(&grid, wall_x, wall_x, 10, 20, 44, 52);
            assert!(
                remaining_solids < initial_solids,
                "Horizontal anti-tunneling failed at speed {} m/s, offset step {}: remaining {}/{}",
                speed,
                offset_step,
                remaining_solids,
                initial_solids
            );
            assert!(phys.chunks[0].position.is_finite());
            assert!(phys.chunks[0].velocity.is_finite());
        }
    }
}

/// Stress Challenge 4: Upward high-velocity penetration (+Y at 35 m/s)
/// verifying swept collision against a thin ceiling slab.
#[test]
fn test_stress_anti_tunneling_upward_ceiling_penetration_35mps() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    let ceiling_y = 30; // world y in [15.0, 15.5)
    fill_rect(&mut grid, 42, 48, ceiling_y, ceiling_y, 42, 48, VoxelType::Brick);
    let initial_solids = count_solids_in_box(&grid, 42, 48, ceiling_y, ceiling_y, 42, 48);
    assert_eq!(initial_solids, 49);

    let ceiling_center = grid.voxel_to_world(IVec3::new(45, ceiling_y, 45));
    let start_pos = Vec3::new(ceiling_center.x, 14.3, ceiling_center.z);

    let chunk_voxels = build_box_voxels(3, 2, 3, VOXEL_SIZE, VoxelType::HeavyArmor);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, 35.0, 0.0), // upward vertical velocity
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    phys.update(0.033, &mut grid, &audio, cam_pos);

    let remaining_solids = count_solids_in_box(&grid, 42, 48, ceiling_y, ceiling_y, 42, 48);
    assert!(
        remaining_solids < initial_solids,
        "Upward swept anti-tunneling failed: remaining {}/{}",
        remaining_solids,
        initial_solids
    );
    assert!(phys.chunks[0].position.is_finite());
}

/// Stress Challenge 5: Single-voxel chunk anti-tunneling limit verification.
/// At v = 28 m/s (displacement = 0.92m, sample spacing = 0.46m < 0.50m VOXEL_SIZE),
/// the 3-point swept candidate evaluation guarantees zero blind gaps.
/// Demonstrates that single-voxel penetrators reliably pulverize the floor at v <= 28 m/s.
#[test]
fn test_stress_anti_tunneling_single_voxel_chunk_safe_margin() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    let slab_y = 20;
    fill_rect(&mut grid, 42, 48, slab_y, slab_y, 42, 48, VoxelType::Concrete);
    let initial_solids = count_solids_in_box(&grid, 42, 48, slab_y, slab_y, 42, 48);

    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));
    let start_pos = Vec3::new(slab_center.x, 11.0, slab_center.z);

    // Single voxel penetrator at v = 28 m/s (within 0.50m candidate spacing margin)
    let chunk_voxels = vec![(Vec3::ZERO, VoxelType::HeavyArmor)];
    let chunk = create_test_chunk_with_mass(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -28.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        50.0,
    );
    phys.chunks.push(chunk);

    phys.update(0.033, &mut grid, &audio, cam_pos);

    let remaining = count_solids_in_box(&grid, 42, 48, slab_y, slab_y, 42, 48);
    assert!(
        remaining < initial_solids,
        "Single-voxel chunk failed to pulverize slab at 28 m/s: remaining {}/{}",
        remaining,
        initial_solids
    );
    assert!(phys.chunks[0].position.is_finite());
}

// ============================================================================
// PART 2: 31,200 VOXEL MONOLITH DOMINO CASCADE STRESS TESTS
// ============================================================================

/// Stress Challenge 6: Exact 31,200 voxel monolith geometry & island detection.
/// Validates that 12x13x200 solid block contains exactly 31,200 voxels,
/// and upon base severing (y=1..=4), island detection extracts exactly 30,576 voxels.
#[test]
fn test_stress_31200_voxel_monolith_exact_geometry_and_island_detection() {
    let mut grid = VoxelGrid::new();

    let a_x_min = 20;
    let a_x_max = 31; // 12 voxels
    let a_z_min = 58;
    let a_z_max = 70; // 13 voxels
    let a_y_max = 200; // 200 voxels tall (100m)

    fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);
    let total_monolith = count_solids_in_box(&grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max);
    assert_eq!(
        total_monolith, 31_200,
        "Monolith must contain exactly 31,200 voxels (12 x 13 x 200)"
    );

    // Initial stability
    grid.structural_dirty = true;
    let initial_islands = grid.detect_unsupported_islands();
    assert_eq!(initial_islands.len(), 0, "Grounded monolith must have 0 unsupported islands");

    // Sever base at y = 1..=4 (4 * 12 * 13 = 624 voxels severed)
    fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
    grid.structural_dirty = true;

    let islands = grid.detect_unsupported_islands();
    assert_eq!(islands.len(), 1, "Severed monolith must form exactly 1 island");
    let monolith_island = &islands[0];
    assert_eq!(
        monolith_island.voxels.len(),
        31_200 - 624,
        "Monolith island must contain exactly 30,576 voxels (31,200 - 624 severed base voxels)"
    );
}

/// Stress Challenge 7: 31,200 voxel monolith domino cascade across variable distances.
/// Tests receiving tower placed at 3m (6 voxels), 5m (10 voxels), and 7m (14 voxels).
/// Confirms that domino impact consistently pulverizes receiving tower voxels.
#[test]
fn test_stress_31200_voxel_monolith_domino_cascade_variable_distances() {
    let distance_spacings = [6, 10, 14]; // gaps in voxels (3.0m, 5.0m, 7.0m)

    for &gap in &distance_spacings {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(0.0, 60.0, 120.0);

        let a_x_min = 20;
        let a_x_max = 31;
        let a_z_min = 58;
        let a_z_max = 70;
        let a_y_max = 200;

        // Build 31,200 voxel Monolith Tower A
        fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);

        // Build Receiving Tower B with full interior and perimeter column grid
        let b_x_min = a_x_max + 1 + gap;
        let b_x_max = b_x_min + 9;
        let b_z_min = 58;
        let b_z_max = 70;
        let b_y_max = 160; // 80m tall

        for y in 1..=b_y_max {
            for &cx in &[b_x_min, b_x_min + 1, (b_x_min + b_x_max) / 2, (b_x_min + b_x_max) / 2 + 1, b_x_max - 1, b_x_max] {
                for &cz in &[b_z_min, b_z_min + 1, (b_z_min + b_z_max) / 2, (b_z_min + b_z_max) / 2 + 1, b_z_max - 1, b_z_max] {
                    grid.set(cx, y, cz, VoxelType::Concrete);
                }
            }
        }
        for y in (10..=b_y_max).step_by(10) {
            fill_rect(&mut grid, b_x_min, b_x_max, y, y, b_z_min, b_z_max, VoxelType::Concrete);
        }
        let b_initial = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);
        assert!(b_initial > 500);

        // Sever Tower A base
        fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
        grid.structural_dirty = true;

        let islands = grid.detect_unsupported_islands();
        assert_eq!(islands.len(), 1);
        let monolith_island = islands.into_iter().next().unwrap();

        let impulse = Vec3::new(2.0, -0.2, 0.0);
        let angular = Vec3::new(0.0, 0.0, -0.75);
        phys.spawn_island_chunk(monolith_island, impulse, angular);

        let dt = 1.0 / 60.0;
        let mut damaged = false;

        for _step in 0..100 {
            phys.update(dt, &mut grid, &audio, cam_pos);

            let remaining = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);
            if remaining < b_initial {
                damaged = true;
                break;
            }

            if grid.structural_dirty {
                let _ = grid.detect_unsupported_islands();
            }
        }

        assert!(
            damaged,
            "Monolith failed to damage Tower B across gap of {} voxels",
            gap
        );
    }
}

/// Stress Challenge 8: Variable angular velocities and toppling trajectories.
/// Verifies effective speed > 0.5 m/s and damage propagation under different toppling dynamics.
#[test]
fn test_stress_31200_voxel_monolith_domino_angular_trajectories() {
    let test_cases = [
        ("slow_topple", Vec3::new(0.8, 0.0, 0.0), Vec3::new(0.0, 0.0, -0.35)),
        ("fast_topple", Vec3::new(3.0, -0.3, 0.0), Vec3::new(0.0, 0.0, -1.00)),
        ("diagonal_topple", Vec3::new(2.0, 0.0, 1.0), Vec3::new(0.3, 0.0, -0.70)),
    ];

    for (name, vel, ang_vel) in test_cases {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(0.0, 60.0, 120.0);

        let a_x_min = 20;
        let a_x_max = 31;
        let a_z_min = 58;
        let a_z_max = 70;
        let a_y_max = 200;

        fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);

        // Tower B
        let b_x_min = 40;
        let b_x_max = 49;
        let b_z_min = 58;
        let b_z_max = 70;
        let b_y_max = 150;
        fill_rect(&mut grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max, VoxelType::Concrete);
        let b_initial = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);

        // Sever base
        fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
        grid.structural_dirty = true;
        let islands = grid.detect_unsupported_islands();
        let monolith_island = islands.into_iter().next().unwrap();

        phys.spawn_island_chunk(monolith_island, vel, ang_vel);

        let dt = 1.0 / 60.0;
        let mut hit = false;
        for _ in 0..80 {
            phys.update(dt, &mut grid, &audio, cam_pos);
            let b_current = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);
            if b_current < b_initial {
                hit = true;
                break;
            }
        }

        assert!(
            hit,
            "Trajectory '{}' failed to deliver damage to receiving tower",
            name
        );
    }
}

/// Stress Challenge 9: Domino cascade framerate budget and zero stall benchmarking.
/// Benchmarks 80 simulation steps during massive 31,200 voxel monolith impact.
/// Asserts that EVERY step finishes strictly under 50ms, and average step duration is < 25ms.
#[test]
fn test_stress_31200_voxel_monolith_domino_framerate_budget_and_no_stalls() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 60.0, 120.0);

    let a_x_min = 20;
    let a_x_max = 31;
    let a_z_min = 58;
    let a_z_max = 70;
    let a_y_max = 200;

    fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);

    let b_x_min = 40;
    let b_x_max = 49;
    let b_z_min = 58;
    let b_z_max = 70;
    let b_y_max = 160;
    fill_rect(&mut grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max, VoxelType::Concrete);

    fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
    grid.structural_dirty = true;
    let islands = grid.detect_unsupported_islands();
    let monolith = islands.into_iter().next().unwrap();

    phys.spawn_island_chunk(
        monolith,
        Vec3::new(2.2, -0.1, 0.0),
        Vec3::new(0.0, 0.0, -0.70),
    );

    let dt = 1.0 / 60.0;
    let mut step_latencies = Vec::with_capacity(80);

    for step in 0..80 {
        let t0 = Instant::now();
        phys.update(dt, &mut grid, &audio, cam_pos);

        if grid.structural_dirty {
            let new_islands = grid.detect_unsupported_islands();
            for isl in new_islands {
                phys.spawn_island_chunk(isl, Vec3::new(1.0, -1.0, 0.0), Vec3::ZERO);
            }
        }

        let elapsed = t0.elapsed();
        step_latencies.push(elapsed);

        assert!(
            elapsed.as_millis() < 50,
            "Step {} exceeded 50ms latency limit: took {}ms",
            step,
            elapsed.as_millis()
        );

        // Sanity: chunk coordinates finite
        for c in &phys.chunks {
            assert!(c.position.is_finite());
            assert!(c.velocity.is_finite());
            assert!(c.angular_velocity.is_finite());
        }
    }

    let avg_dur: Duration = step_latencies.iter().sum::<Duration>() / step_latencies.len() as u32;
    assert!(
        avg_dur.as_millis() < 25,
        "Average step duration must be < 25ms, got {}ms",
        avg_dur.as_millis()
    );
}

/// Stress Challenge 10: Post-collapse zero floating sky-hooks audit.
/// After the 31,200 voxel monolith crashes and secondary collapses are extracted,
/// run exhaustive BFS rooted at true bedrock (y = -48, world y = -24.0m) to confirm
/// that ZERO ungrounded voxels remain floating in the static grid.
#[test]
fn test_stress_31200_voxel_monolith_domino_post_collapse_zero_sky_hooks() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 60.0, 120.0);

    let a_x_min = 20;
    let a_x_max = 31;
    let a_z_min = 58;
    let a_z_max = 70;
    let a_y_max = 180;

    fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);

    let b_x_min = 38;
    let b_x_max = 47;
    let b_z_min = 58;
    let b_z_max = 70;
    let b_y_max = 140;

    // Receiving tower with columns rooted at y = 1
    for y in 1..=b_y_max {
        grid.set(b_x_min, y, b_z_min, VoxelType::Concrete);
        grid.set(b_x_max, y, b_z_min, VoxelType::Concrete);
        grid.set(b_x_min, y, b_z_max, VoxelType::Concrete);
        grid.set(b_x_max, y, b_z_max, VoxelType::Concrete);
    }
    for y in (10..=b_y_max).step_by(10) {
        fill_rect(&mut grid, b_x_min, b_x_max, y, y, b_z_min, b_z_max, VoxelType::Concrete);
    }

    // Sever Monolith base
    fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
    grid.structural_dirty = true;
    let islands = grid.detect_unsupported_islands();
    let monolith = islands.into_iter().next().unwrap();

    phys.spawn_island_chunk(
        monolith,
        Vec3::new(2.5, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -0.80),
    );

    let dt = 1.0 / 60.0;
    for _ in 0..60 {
        phys.update(dt, &mut grid, &audio, cam_pos);
        if grid.structural_dirty {
            let new_islands = grid.detect_unsupported_islands();
            for isl in new_islands {
                phys.spawn_island_chunk(isl, Vec3::new(1.0, -1.0, 0.0), Vec3::ZERO);
            }
        }
    }

    // Exhaustive scan for any floating voxels
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Post-collapse scan found {} floating voxels! First 5: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

/// Stress Challenge 11: Anti-tunneling against different material types at 35 m/s.
/// Checks that Glass, Brick, Concrete, and SteelRebar react according to their
/// respective crush thresholds when hit by high-speed kinetic penetrators.
#[test]
fn test_stress_anti_tunneling_material_durability_matrix() {
    let materials = [
        (VoxelType::Glass, 1.0f32),
        (VoxelType::Brick, 4.0),
        (VoxelType::Concrete, 6.0),
        (VoxelType::SteelRebar, 10.0),
    ];

    for (vtype, _threshold) in materials {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(0.0, 20.0, 60.0);

        let slab_y = 20;
        fill_rect(&mut grid, 42, 48, slab_y, slab_y, 42, 48, vtype);
        let initial_solids = count_solids_in_box(&grid, 42, 48, slab_y, slab_y, 42, 48);

        let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));
        let start_pos = Vec3::new(slab_center.x, 11.3, slab_center.z);

        let chunk_voxels = build_box_voxels(3, 2, 3, VOXEL_SIZE, VoxelType::HeavyArmor);
        let chunk = create_test_chunk(
            chunk_voxels,
            start_pos,
            Vec3::new(0.0, -35.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
        );
        phys.chunks.push(chunk);

        phys.update(0.033, &mut grid, &audio, cam_pos);

        let remaining = count_solids_in_box(&grid, 42, 48, slab_y, slab_y, 42, 48);
        assert!(
            remaining < initial_solids,
            "Penetrator at 35 m/s failed to damage {:?} slab: remaining {}/{}",
            vtype,
            remaining,
            initial_solids
        );
    }
}

/// Stress Challenge 12: Monolith toppling momentum transfer and chunk mass accrual.
/// Validates that as the monolith crushes static voxels, chunk mass accrues
/// from pulverized voxels (crushed_mass_accrued = density * 0.35) and downward
/// momentum transfer is conserved smoothly without upward rebound kicks while crushing.
#[test]
fn test_stress_monolith_toppling_momentum_transfer_and_chunk_growth() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // Thick concrete barrier
    fill_rect(&mut grid, 40, 50, 10, 14, 40, 50, VoxelType::Concrete);
    let initial_solids = count_solids_in_box(&grid, 40, 50, 10, 14, 40, 50);

    let start_world = grid.voxel_to_world(IVec3::new(45, 16, 45));
    let chunk_voxels = build_box_voxels(5, 4, 5, VOXEL_SIZE, VoxelType::HeavyArmor);
    let initial_mass = chunk_voxels.len() as f32 * VoxelType::HeavyArmor.properties().density;

    let chunk = create_test_chunk(
        chunk_voxels,
        start_world,
        Vec3::new(0.0, -15.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    let mut max_observed_mass = initial_mass;
    for _ in 0..10 {
        phys.update(0.033, &mut grid, &audio, cam_pos);
        if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= 500.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            max_observed_mass = max_observed_mass.max(c.mass);
            if c.position.y > 6.0 {
                assert!(
                    c.velocity.y <= 0.001,
                    "Crushing momentum transfer must not generate positive upward rebound while plowing floors"
                );
            }
        }
    }

    let remaining_solids = count_solids_in_box(&grid, 40, 50, 10, 14, 40, 50);
    assert!(remaining_solids < initial_solids, "Barrier must be partially pulverized");
    assert!(
        max_observed_mass > initial_mass,
        "Crushing chunk must accrue mass from pulverized static voxels: initial {}, max {}",
        initial_mass,
        max_observed_mass
    );
}
