//! ============================================================================
//! GENERATION 6 COMPREHENSIVE PHYSICS VERIFICATION TEST SUITE
//! Target: tests/g6_physics_verification.rs
//! Milestone: G6-M3 (Dedicated Physics Verification Test Suites & Adversarial Hardening)
//! ============================================================================
//!
//! This suite validates all five core physics verification scenarios:
//! 1. High-Velocity Swept Anti-Tunneling (20-40 m/s vs 0.5m Concrete/Brick slabs, dt=0.033s)
//! 2. 30,000+ Voxel Monolith Toppling Domino Cascade (100m tower cascade into 80m tower, < 50ms/step)
//! 3. Continuous 8-Story Pancake Collapse (no upward bounce, sequential pulverization down to bedrock)
//! 4. Post-Demolition Zero Floating Voxel / Sky-Hook Audit (ground-truth BFS scanner seeded at true bedrock)
//! 5. Multi-Chunk Stacking Stability (>48 chunks) & Crater Re-Awakening (64 chunks, rubble_dirty flag)

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, Mat3, PhysicsSystem, CONTACT_SLOP};
use voxel_demolition::voxel::{
    DamageType, IslandChunk, VoxelGrid, VoxelType, BEDROCK_Y, CANTILEVER_MAX_DEPTH, GRID_X, GRID_Y,
    GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE,
};

// ============================================================================
// SHARED TEST HARNESS HELPERS
// ============================================================================

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

/// Helper: construct a test dynamic chunk with given kinematics and accurate mass.
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

/// Reconstruct external support at each foot without trusting the chunk's own cached tops.
/// Settled voxels support their raster cells, matching the engine's collision proxy.
/// This checks collision support, not exact contact between rendered rotated cubes.
fn external_support_gap(chunk_idx: usize, phys: &PhysicsSystem, grid: &VoxelGrid) -> Option<f32> {
    let chunk = &phys.chunks[chunk_idx];
    let half = VOXEL_SIZE * 0.5;
    let mut nearest: Option<f32> = None;
    for (local, _) in &chunk.voxels {
        let center = chunk.local_to_world(*local);
        let bottom_y = center.y - half;
        for offset in [Vec2::ZERO, Vec2::new(-half, -half), Vec2::new(half, -half), Vec2::new(-half, half), Vec2::new(half, half)] {
            let foot = Vec3::new(center.x + offset.x, bottom_y, center.z + offset.y);
            let cell = grid.world_to_voxel(foot);
            for y in [cell.y - 1, cell.y] {
                if grid.get(cell.x, y, cell.z).is_solid() {
                    let support_center = grid.voxel_to_world(IVec3::new(cell.x, y, cell.z));
                    if support_center.y <= center.y {
                        let gap = bottom_y - (support_center.y + half);
                        nearest = Some(nearest.map_or(gap, |previous| previous.min(gap)));
                    }
                }
            }
            let Some(foot_cell) = PhysicsSystem::world_to_rubble_grid_coords(foot.x, foot.z) else {
                continue;
            };
            for (other_idx, other) in phys.chunks.iter().enumerate() {
                // A settled chunk's own stamped tops cannot prove it has external support.
                if other_idx == chunk_idx || !other.settled {
                    continue;
                }
                for (other_local, _) in &other.voxels {
                    let support_center = other.local_to_world(*other_local);
                    let support_cell = PhysicsSystem::world_to_rubble_grid_coords(support_center.x, support_center.z);
                    if support_cell == Some(foot_cell) && support_center.y <= center.y {
                        let gap = bottom_y - (support_center.y + half);
                        nearest = Some(nearest.map_or(gap, |previous| previous.min(gap)));
                    }
                }
            }
        }
    }
    nearest
}

#[test]
fn test_support_audit_rejects_self_stamped_and_airborne_support() {
    let grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let voxel = vec![(Vec3::ZERO, VoxelType::Concrete)];
    phys.chunks.push(create_test_chunk(voxel.clone(), Vec3::new(0.0, 40.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO));
    phys.settle_chunk(0);
    assert!(phys.rubble_height_at(0.0, 0.0) > 40.0, "Control must contain the chunk's own stamped cache");
    assert!(external_support_gap(0, &phys, &grid).is_none(), "An isolated airborne chunk cannot support itself");

    phys.chunks.push(create_test_chunk(voxel, Vec3::new(0.0, 39.5, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO));
    assert!(external_support_gap(0, &phys, &grid).is_none(), "An airborne neighbor cannot supply settled support");
    phys.chunks[1].settled = true;
    assert_eq!(external_support_gap(0, &phys, &grid), Some(0.0));
    phys.chunks[1].position.y -= CONTACT_SLOP * 2.0;
    assert!(external_support_gap(0, &phys, &grid).unwrap() > CONTACT_SLOP + 1e-5, "A separated neighbor must fail the existing contact slop");
    phys.chunks[1].position.y = 41.0;
    assert!(external_support_gap(0, &phys, &grid).is_none(), "An overhead neighbor cannot support a foot from below");
}

#[test]
fn test_support_audit_accepts_contact_feet_over_an_empty_center_column() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let cell = IVec3::new(96, 8, 96);
    grid.set(cell.x - 1, cell.y, cell.z, VoxelType::HeavyArmor);
    grid.set(cell.x + 1, cell.y, cell.z, VoxelType::HeavyArmor);
    let center = grid.voxel_to_world(cell) + Vec3::Y * VOXEL_SIZE;
    let voxels = [-VOXEL_SIZE, 0.0, VOXEL_SIZE].map(|x| (Vec3::new(x, 0.0, 0.0), VoxelType::Concrete)).to_vec();
    phys.chunks.push(create_test_chunk(voxels, center, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO));
    assert_eq!(grid.get(cell.x, cell.y, cell.z), VoxelType::Air);
    assert_eq!(external_support_gap(0, &phys, &grid), Some(0.0), "Either side foot can support a bridge above an empty center column");
}

/// Independent ground-truth BFS sky-hook scanner auditing the entire VoxelGrid.
/// Seeds strictly at true subterranean geological bedrock foundation y = MIN_VOXEL_Y (-48, world -24.0m)
/// up to y = 0. Any solid voxel above y = 0 that lacks a connected solid path down to true bedrock is
/// identified as an ungrounded floating "sky-hook".
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

    // 2. 18-connected neighbor traversal (6 cardinal + 12 diagonals)
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

    // 3. Collect all solid voxels with y > 0 that lack connectivity to true bedrock
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
// SUITE 1: HIGH-VELOCITY SWEPT ANTI-TUNNELING (4 TESTS)
// ============================================================================

#[test]
fn test_suite1_anti_tunneling_concrete_slab_20_meters_per_sec() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    // 1-voxel thin Concrete slab at y = 20 (thickness 0.5m, world y in [10.0, 10.5])
    let slab_y = 20;
    fill_rect(&mut grid, 40, 49, slab_y, slab_y, 40, 49, VoxelType::Concrete);
    let initial_solids = grid.solid_count();
    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));

    // High-speed chunk at 20 m/s with dt = 0.033s (step displacement = 0.66m > 0.5m slab thickness)
    let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(slab_center.x, 11.2, slab_center.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -20.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    phys.update(0.033, &mut grid, &audio, cam_pos);

    let remaining_solids = grid.solid_count();
    assert!(
        remaining_solids < initial_solids,
        "Swept collision probing failed at 20 m/s: slab remained intact ({} / {})",
        remaining_solids,
        initial_solids
    );
    assert!(phys.chunks[0].position.is_finite(), "Chunk position must be finite");
    assert!(phys.chunks[0].velocity.is_finite(), "Chunk velocity must be finite");
    assert!(
        phys.chunks[0].velocity.y <= 0.001,
        "Chunk must not rebound with positive vy after impact: vy = {}",
        phys.chunks[0].velocity.y
    );
}

#[test]
fn test_suite1_anti_tunneling_concrete_slab_30_meters_per_sec() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    // 1-voxel thin Concrete slab at y = 20 (thickness 0.5m)
    let slab_y = 20;
    fill_rect(&mut grid, 40, 49, slab_y, slab_y, 40, 49, VoxelType::Concrete);
    let initial_solids = grid.solid_count();
    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));

    // High-speed chunk at 30 m/s with dt = 0.033s (step displacement = 0.99m ~ 2 voxels)
    let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::Concrete);
    let start_pos = Vec3::new(slab_center.x, 11.4, slab_center.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -30.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    phys.update(0.033, &mut grid, &audio, cam_pos);

    let remaining_solids = grid.solid_count();
    assert!(
        remaining_solids < initial_solids,
        "Swept collision probing failed at 30 m/s: slab remained intact ({} / {})",
        remaining_solids,
        initial_solids
    );
    assert!(phys.chunks[0].position.is_finite());
    assert!(phys.chunks[0].velocity.is_finite());
    assert!(phys.chunks[0].velocity.y <= 0.001);
}

#[test]
fn test_suite1_anti_tunneling_brick_slab_40_meters_per_sec() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    // 1-voxel thin Brick slab at y = 20 (thickness 0.5m)
    let slab_y = 20;
    fill_rect(&mut grid, 42, 47, slab_y, slab_y, 42, 47, VoxelType::Brick);
    let slab_solids_before = count_solids_in_box(&grid, 42, 47, slab_y, slab_y, 42, 47);
    assert_eq!(slab_solids_before, 36);
    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));

    // High-speed chunk at 40 m/s with dt = 0.033s (step displacement = 1.32m = 2.64 voxels!)
    let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(slab_center.x, 11.8, slab_center.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -40.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    phys.update(0.033, &mut grid, &audio, cam_pos);

    let slab_solids_after = count_solids_in_box(&grid, 42, 47, slab_y, slab_y, 42, 47);
    assert!(
        slab_solids_after < slab_solids_before,
        "Swept collision probing failed at 40 m/s against Brick slab: completely tunneled ({} / {})",
        slab_solids_after,
        slab_solids_before
    );
    assert!(phys.chunks[0].position.is_finite());
    assert!(phys.chunks[0].velocity.is_finite());
}

#[test]
fn test_suite1_anti_tunneling_variable_substep_swept_verification() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    // Vertical 1-voxel thin Brick wall at x = 50, y in 10..=20, z in 45..=55
    fill_rect(&mut grid, 50, 50, 10, 20, 45, 55, VoxelType::Brick);
    let initial_wall_solids = count_solids_in_box(&grid, 50, 50, 10, 20, 45, 55);
    assert_eq!(initial_wall_solids, 121);

    // Diagonal penetrator with v = (25.0, -20.0, 0.0) m/s, magnitude = 32.0 m/s
    // Moving from x = 48.0 towards the wall at x = 50 (world x = 25.0m)
    let chunk_voxels = build_box_voxels(3, 3, 3, VOXEL_SIZE, VoxelType::Concrete);
    let wall_world = grid.voxel_to_world(IVec3::new(50, 15, 50));
    let start_pos = wall_world - Vec3::new(1.0, -0.8, 0.0);

    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(25.0, -20.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    phys.chunks.push(chunk);

    // Test with variable time step dt = 0.033s
    phys.update(0.033, &mut grid, &audio, cam_pos);

    let remaining_wall_solids = count_solids_in_box(&grid, 50, 50, 10, 20, 45, 55);
    assert!(
        remaining_wall_solids < initial_wall_solids,
        "Diagonal swept probing failed to pulverize Brick wall: {} / {}",
        remaining_wall_solids,
        initial_wall_solids
    );
    assert!(phys.chunks[0].position.is_finite());
    assert!(phys.chunks[0].velocity.is_finite());
}

// ============================================================================
// SUITE 2: 30,000+ VOXEL MONOLITH TOPPLING DOMINO CASCADE (2 TESTS)
// ============================================================================

#[test]
fn test_suite2_monolith_toppling_domino_cascade_30000_voxels() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 60.0, 120.0);

    // 1. Build Tower B (Receiving Skyscraper):
    // X in 42..=51 (10 voxels), Z in 58..=70 (13 voxels), Y in 1..=160 (80m tall)
    let b_x_min = 42;
    let b_x_max = 51;
    let b_z_min = 58;
    let b_z_max = 70;
    let b_y_max = 160;

    // Structural columns (perimeter & interior grid)
    for y in 1..=b_y_max {
        for &cx in &[b_x_min, b_x_min + 1, (b_x_min + b_x_max) / 2, (b_x_min + b_x_max) / 2 + 1, b_x_max - 1, b_x_max] {
            for &cz in &[b_z_min, b_z_min + 1, (b_z_min + b_z_max) / 2, (b_z_min + b_z_max) / 2 + 1, b_z_max - 1, b_z_max] {
                grid.set(cx, y, cz, VoxelType::Concrete);
            }
        }
    }
    // Floor plates every 10 voxels
    for y in (10..=b_y_max).step_by(10) {
        fill_rect(&mut grid, b_x_min, b_x_max, y, y, b_z_min, b_z_max, VoxelType::Concrete);
    }
    let b_initial_solids = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);
    assert!(b_initial_solids > 1000, "Tower B must have substantial structural mass");

    // 2. Build Tower A (The 30,000+ Voxel Monolith):
    // X in 20..=31 (12 voxels), Z in 58..=70 (13 voxels), Y in 1..=200 (100m tall)
    // Total voxels = 12 * 13 * 200 = 31,200 voxels!
    let a_x_min = 20;
    let a_x_max = 31;
    let a_z_min = 58;
    let a_z_max = 70;
    let a_y_max = 200;

    fill_rect(&mut grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max, VoxelType::Concrete);
    let a_initial_solids = count_solids_in_box(&grid, a_x_min, a_x_max, 1, a_y_max, a_z_min, a_z_max);
    assert_eq!(a_initial_solids, 31_200, "Tower A must contain exactly 31,200 voxels");

    // Verify initial stability: both structures grounded, 0 unsupported islands
    grid.structural_dirty = true;
    let initial_islands = grid.detect_unsupported_islands();
    assert_eq!(initial_islands.len(), 0, "Both towers must initially be 100% grounded");

    // 3. Sever Tower A's base at Y = 1..=4
    fill_rect(&mut grid, a_x_min, a_x_max, 1, 4, a_z_min, a_z_max, VoxelType::Air);
    grid.structural_dirty = true;

    // Detect severed Tower A as a 30,000+ voxel unsupported island
    let severed_islands = grid.detect_unsupported_islands();
    assert_eq!(severed_islands.len(), 1, "Severed Tower A must form exactly 1 unsupported island");
    let tower_a_island = severed_islands.into_iter().next().unwrap();
    assert!(
        tower_a_island.voxels.len() >= 30_000,
        "Tower A island must contain >= 30,000 voxels, got {}",
        tower_a_island.voxels.len()
    );

    // Spawn Tower A toppling sideways (+X) into Tower B
    let impulse = Vec3::new(1.5, -0.2, 0.0);
    let angular = Vec3::new(0.0, 0.0, -0.65);
    phys.spawn_island_chunk(tower_a_island, impulse, angular);

    // 4. Run simulation loop with latency benchmarking & numerical validation
    let dt = 1.0 / 60.0;
    let max_steps = 150;
    let mut tower_b_damaged = false;
    let mut secondary_islands_total = 0;
    let mut max_effective_speed = 0.0f32;
    let mut max_step_time = Duration::ZERO;

    for step in 0..max_steps {
        let t0 = Instant::now();

        // Track max effective speed before step
        for chunk in &phys.chunks {
            let (min_aabb, max_aabb) = chunk.local_aabb;
            let half_extent = (max_aabb - min_aabb) * 0.5;
            let eff_spd = chunk.velocity.length() + chunk.angular_velocity.length() * half_extent.length();
            if eff_spd > max_effective_speed {
                max_effective_speed = eff_spd;
            }
        }

        // Step physics
        phys.update(dt, &mut grid, &audio, cam_pos);

        // Check for NaN or Inf in chunk states
        for chunk in &phys.chunks {
            assert!(chunk.position.is_finite(), "Step {}: Chunk position must remain finite", step);
            assert!(chunk.velocity.is_finite(), "Step {}: Chunk velocity must remain finite", step);
            assert!(chunk.angular_velocity.is_finite(), "Step {}: Chunk angular_velocity must remain finite", step);
        }

        // Check if Tower B sustained damage
        let current_b_solids = count_solids_in_box(&grid, b_x_min, b_x_max, 1, b_y_max, b_z_min, b_z_max);
        if current_b_solids < b_initial_solids {
            tower_b_damaged = true;
        }

        // Run structural island detection if grid was dirtied
        if grid.structural_dirty {
            let new_islands = grid.detect_unsupported_islands();
            if !new_islands.is_empty() {
                secondary_islands_total += new_islands.len();
                for island in new_islands {
                    phys.spawn_island_chunk(
                        island,
                        Vec3::new(1.5, -1.0, 0.0),
                        Vec3::new(0.0, 0.0, -0.5),
                    );
                }
            }
        }

        let elapsed = t0.elapsed();
        if elapsed > max_step_time {
            max_step_time = elapsed;
        }

        // Performance invariant: zero simulation stalls (< 50ms/step)
        assert!(
            elapsed.as_millis() < 50,
            "Step {} took {} ms, exceeding 50ms/step stall limit!",
            step,
            elapsed.as_millis()
        );

        if tower_b_damaged && secondary_islands_total > 0 {
            break;
        }
    }

    // 5. Final Assertions
    assert!(tower_b_damaged, "Tower B must sustain physical damage from toppling 30k monolith");
    assert!(secondary_islands_total > 0, "Secondary collapse must generate new unsupported islands");
    assert!(max_effective_speed > 0.5, "Effective speed must exceed crush threshold 0.5 m/s");
    assert!(phys.chunks.len() >= 2, "Dynamic chunks count must increase from secondary collapses");
}

#[test]
fn test_suite2_domino_cascade_timing_and_framerate_budget() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // Setup adjacent receiving tower (8x8x180 = 11,520 voxels) at X = 36..=43
    let b_min_x = 36;
    let b_max_x = 43;
    let b_max_y = 180;
    fill_rect(&mut grid, b_min_x, b_max_x, 1, b_max_y, 44, 51, VoxelType::Concrete);
    let tower_b_initial = count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, 44, 51);
    assert!(tower_b_initial > 0);

    // Setup 30,000+ voxel skyscraper: 12 x 12 x 210 = 30,240 voxels
    let a_min_x = 18;
    let a_max_x = 29;
    let a_min_z = 42;
    let a_max_z = 53;
    let a_max_y = 210;
    fill_rect(&mut grid, a_min_x, a_max_x, 1, a_max_y, a_min_z, a_max_z, VoxelType::Concrete);

    // Sever the colossal tower foundation at Y = 1..=4
    fill_rect(&mut grid, a_min_x, a_max_x, 1, 4, a_min_z, a_max_z, VoxelType::Air);
    grid.structural_dirty = true;

    let islands = grid.detect_unsupported_islands();
    assert_eq!(islands.len(), 1);
    let colossal_island = islands.into_iter().next().unwrap();
    assert!(colossal_island.voxels.len() >= 29_000);

    let vel = Vec3::new(2.5, 0.0, 0.0);
    let ang_vel = Vec3::new(0.0, 0.0, -0.75);
    phys.spawn_island_chunk(colossal_island, vel, ang_vel);

    let dt = 1.0 / 60.0;
    let mut step_durations = Vec::with_capacity(60);
    let mut receiving_crushed = false;

    for step in 0..60 {
        let step_start = Instant::now();
        phys.update(dt, &mut grid, &audio, cam_pos);
        let step_dur = step_start.elapsed();

        assert!(
            step_dur.as_millis() < 50,
            "Step {} took {}ms (exceeded 50ms budget)",
            step,
            step_dur.as_millis()
        );

        let remaining = count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, 44, 51);
        if remaining < tower_b_initial {
            receiving_crushed = true;
        }

        if grid.structural_dirty {
            let _ = grid.detect_unsupported_islands();
        }

        step_durations.push(step_dur);
    }

    assert!(receiving_crushed, "Receiving tower must be damaged by toppling monolith");
    let avg_dur: Duration = step_durations.iter().sum::<Duration>() / step_durations.len() as u32;
    assert!(
        avg_dur.as_millis() < 30,
        "Average step duration must be < 30ms, was {}ms",
        avg_dur.as_millis()
    );
}

// ============================================================================
// SUITE 3: CONTINUOUS 8-STORY PANCAKE COLLAPSE (4 TESTS)
// ============================================================================

#[test]
fn test_suite3_pancake_collapse_continuous_8_story_concrete_descent() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    // 1. Construct 8 thin Concrete floors (0.5m thick, 3.0m air gaps)
    let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
    let floor_solids_initial = 49; // 7x7 footprint
    for &fy in &floor_levels {
        fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Concrete);
    }
    assert_eq!(grid.solid_count(), floor_solids_initial * 8);

    // 2. Spawn 2,000 kg heavy roof slab at y = 60 (world y ~ 30.5m)
    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(6, 2, 6, VOXEL_SIZE, VoxelType::Concrete);
    let start_pos = top_world + Vec3::new(0.0, 2.0, 0.0);
    let heavy_slab = create_test_chunk_with_mass(
        slab_voxels,
        start_pos,
        Vec3::new(0.0, -10.0, 0.0), // initial downward crushing velocity
        Vec3::ZERO,
        Vec3::ZERO,
        2000.0,
    );
    phys.chunks.push(heavy_slab);

    let mut lowest_y = f32::INFINITY;
    let mut zero_velocity_stalls = 0;
    let mut positive_rebound_frames = 0;
    let initial_mass = 2000.0f32;
    let mut max_observed_mass = initial_mass;

    // 3. Step physics simulation for 120 frames
    for _step in 0..120 {
        if phys.chunks.is_empty() {
            break;
        }

        phys.update(0.033, &mut grid, &audio, cam_pos);

        // Find primary crushing mass
        if let Some(chunk) = phys.chunks.iter().filter(|c| c.mass >= 500.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(chunk.position.y);
            max_observed_mass = max_observed_mass.max(chunk.mass);

            // Invariant 1: No positive restitution kick (vy <= 0.0 at all times)
            if chunk.velocity.y > 0.0001 {
                positive_rebound_frames += 1;
            }

            // Invariant 2: No mid-air stall (vy != 0.0 while airborne above y = 2.0m)
            if chunk.position.y > 2.0 && chunk.velocity.y == 0.0 {
                zero_velocity_stalls += 1;
            }

            // Invariant 3: No unnatural lateral deflection
            assert!(
                chunk.velocity.x.abs() < 4.0 && chunk.velocity.z.abs() < 4.0,
                "Crushing mass deflected unnaturally: vx={}, vz={}",
                chunk.velocity.x, chunk.velocity.z
            );
        }
        // Audit floor-crushing descent; post-ground rocking is a separate case.
        if lowest_y <= 3.0 { break; }
    }

    // 4. Assertions
    assert_eq!(
        positive_rebound_frames, 0,
        "High-mass roof chunk must NEVER rebound upward with positive vy during continuous pancake collapse"
    );
    assert_eq!(
        zero_velocity_stalls, 0,
        "Crushing chunk must NEVER stall mid-air with vy = 0.0 while crushing intermediate floors"
    );
    assert!(
        lowest_y <= 3.0,
        "Crushing mass must smash through all 8 floors down to bedrock level (reached y = {})",
        lowest_y
    );
    assert!(
        max_observed_mass > initial_mass,
        "Crushing slab must accrue mass from pulverized intermediate floors: initial={}, max={}",
        initial_mass, max_observed_mass
    );

    // Verify all 8 floors had their centers pulverized
    for (idx, &fy) in floor_levels.iter().enumerate() {
        let remaining_solids = count_solids_in_box(&grid, 45, 49, fy, fy, 45, 49);
        assert_eq!(
            remaining_solids, 0,
            "Floor {} at y = {} must have its center pulverized completely (found {} remaining voxels)",
            idx + 1, fy, remaining_solids
        );
    }
}

#[test]
fn test_suite3_pancake_collapse_pure_gravity_drop_brick_floors() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    // 8 thin Brick floors (crush threshold = 8.0)
    let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
    for &fy in &floor_levels {
        fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Brick);
    }

    // Drop 1,200 kg slab from rest (velocity = ZERO) at y = 59 (1.5m above top floor)
    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(5, 2, 5, VOXEL_SIZE, VoxelType::Concrete);
    let heavy_slab = create_test_chunk_with_mass(
        slab_voxels,
        top_world + Vec3::new(0.0, 1.5, 0.0),
        Vec3::ZERO, // pure free-fall under gravity
        Vec3::ZERO,
        Vec3::ZERO,
        1200.0,
    );
    phys.chunks.push(heavy_slab);

    let mut lowest_y = f32::INFINITY;
    for step in 0..120 {
        if phys.chunks.is_empty() {
            println!("Frame {}: phys.chunks is empty!", step);
            break;
        }
        phys.update(0.033, &mut grid, &audio, cam_pos);
        if let Some(chunk) = phys.chunks.iter().filter(|c| c.mass >= 400.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(chunk.position.y);
            assert!(chunk.velocity.y <= 0.001, "Gravity drop must not produce upward rebound kick");
        } else {
            println!("Frame {}: no chunk with mass >= 400.0! Chunks: {:?}", step, phys.chunks.iter().map(|c| (c.mass, c.position.y, c.settled)).collect::<Vec<_>>());
        }
        // Audit floor-crushing descent; post-ground rocking is a separate case.
        if lowest_y <= 3.0 { break; }
    }
    assert!(lowest_y <= 3.0, "Pure gravity drop (1200 kg) must smash through all 8 Brick floors down to bedrock level (reached y = {})", lowest_y);
}

#[test]
fn test_suite3_pancake_collapse_tilted_asymmetric_roof_slab() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
    for &fy in &floor_levels {
        fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Concrete);
    }

    // Tilted slab: pitch = 0.25 rad, roll = 0.15 rad, small angular velocity
    let rot = Vec3::new(0.25, 0.0, 0.15);
    let ang_vel = Vec3::new(0.4, 0.0, -0.3);
    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(6, 2, 6, VOXEL_SIZE, VoxelType::HeavyArmor);
    let chunk = create_test_chunk_with_mass(
        slab_voxels,
        top_world + Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, -8.0, 0.0),
        rot,
        ang_vel,
        1800.0,
    );
    phys.chunks.push(chunk);

    let mut lowest_y = f32::INFINITY;
    for _ in 0..120 {
        if phys.chunks.is_empty() { break; }
        phys.update(0.033, &mut grid, &audio, cam_pos);
        if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= 400.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(c.position.y);
            assert!(c.velocity.y <= 0.001, "Tilted impact must not induce positive upward restitution bounce");
        }
        // Audit floor-crushing descent; post-ground rocking is a separate case.
        if lowest_y <= 3.5 { break; }
    }

    assert!(lowest_y <= 3.5, "Tilted slab must continue crushing through all 8 floors down towards bedrock, got {}", lowest_y);
}

#[test]
fn test_suite3_pancake_collapse_multi_mass_matrix_invariants() {
    let test_masses = [500.0f32, 1000.0, 1500.0, 2500.0];
    for &mass in &test_masses {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

        let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
        for &fy in &floor_levels {
            fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Concrete);
        }

        let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
        let slab_voxels = build_box_voxels(5, 2, 5, VOXEL_SIZE, VoxelType::Concrete);
        let chunk = create_test_chunk_with_mass(
            slab_voxels,
            top_world + Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, -10.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            mass,
        );
        phys.chunks.push(chunk);

        let mut lowest_y = f32::INFINITY;
        for _ in 0..110 {
            if phys.chunks.is_empty() { break; }
            phys.update(0.033, &mut grid, &audio, cam_pos);
            if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= mass * 0.4).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
                lowest_y = lowest_y.min(c.position.y);
                assert!(c.velocity.y <= 0.001, "Mass {}kg produced positive vy rebound", mass);
            }
            // Audit floor-crushing descent; post-ground rocking is a separate case.
            if lowest_y <= 3.0 { break; }
        }

        assert!(
            lowest_y <= 3.0,
            "Mass {}kg failed to penetrate 8 floors, reached y = {}",
            mass, lowest_y
        );
    }
}

// ============================================================================
// SUITE 4: POST-DEMOLITION ZERO FLOATING VOXEL / SKY-HOOK AUDIT (5 TESTS)
// ============================================================================

#[test]
fn test_suite4_zero_sky_hooks_after_monolithic_tower_base_severing() {
    let mut grid = VoxelGrid::new();

    // 1. Build a 50-voxel (25m) tall monolithic tower with 4 corner columns & 5 floor slabs
    let min_x = 42;
    let max_x = 48;
    let min_z = 42;
    let max_z = 48;
    for y in 1..=50 {
        grid.set(min_x, y, min_z, VoxelType::Concrete);
        grid.set(min_x, y, max_z, VoxelType::Concrete);
        grid.set(max_x, y, min_z, VoxelType::Concrete);
        grid.set(max_x, y, max_z, VoxelType::Concrete);
    }
    for &fy in &[10, 20, 30, 40, 50] {
        fill_rect(&mut grid, min_x, max_x, fy, fy, min_z, max_z, VoxelType::Concrete);
    }

    // Flush dirty state; verify initial stability (0 islands)
    let initial_islands = grid.detect_unsupported_islands();
    assert!(initial_islands.is_empty(), "Grounded tower must have 0 islands");

    // 2. Sever all 4 base columns at y = 1..=4
    fill_rect(&mut grid, min_x - 1, max_x + 1, 1, 4, min_z - 1, max_z + 1, VoxelType::Air);

    // 3. Run island detection
    let collapse_islands = grid.detect_unsupported_islands();
    assert!(
        !collapse_islands.is_empty(),
        "Severing base of tower must produce dynamic collapse island(s)"
    );

    // 4. Exhaustive static grid scan for floating sky-hooks
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Post-severing scan found {} floating sky-hook voxels in static grid! First 5: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

#[test]
fn test_suite4_zero_sky_hooks_after_deep_subterranean_cratering() {
    let mut grid = VoxelGrid::new();

    // 1. Tower columns rooted through y = 0 into subterranean basalt at y = -10
    for y in -10..=30 {
        grid.set(40, y, 40, VoxelType::Concrete);
        grid.set(46, y, 40, VoxelType::Concrete);
        grid.set(40, y, 46, VoxelType::Concrete);
        grid.set(46, y, 46, VoxelType::Concrete);
    }
    fill_rect(&mut grid, 40, 46, 30, 30, 40, 46, VoxelType::Concrete);
    let _ = grid.detect_unsupported_islands();

    // 2. Heavy ordnance carves a subterranean crater blasting bedrock and basalt from y = -20 up to y = 5
    fill_rect(&mut grid, 38, 48, -20, 5, 38, 48, VoxelType::Air);

    // 3. Detect islands
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Cratering underneath foundation must detach structure");

    // 4. Exhaustive scan: zero floating voxels above crater
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Found {} floating sky-hooks suspended over subterranean crater!",
        sky_hooks.len()
    );
}

#[test]
fn test_suite4_zero_sky_hooks_full_demolition_and_midair_settling_audit() {
    macroquad::rand::srand(0);
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(45.0 * VOXEL_SIZE, 20.0, 45.0 * VOXEL_SIZE + 25.0);

    // 1. Build 6-story building with roof slab
    for y in 1..=36 {
        grid.set(42, y, 42, VoxelType::Concrete);
        grid.set(48, y, 42, VoxelType::Concrete);
        grid.set(42, y, 48, VoxelType::Concrete);
        grid.set(48, y, 48, VoxelType::Concrete);
    }
    for &fy in &[6, 12, 18, 24, 30, 36] {
        fill_rect(&mut grid, 42, 48, fy, fy, 42, 48, VoxelType::Concrete);
    }

    // 2. Sever base columns at y = 1..=3
    fill_rect(&mut grid, 40, 50, 1, 3, 40, 50, VoxelType::Air);

    // 3. Drop heavy 1500 kg roof chunk from above
    let top_world = grid.voxel_to_world(IVec3::new(45, 36, 45));
    let heavy_slab = create_test_chunk_with_mass(
        build_box_voxels(5, 2, 5, VOXEL_SIZE, VoxelType::Concrete),
        top_world + Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, -10.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        1500.0,
    );
    phys.chunks.push(heavy_slab);

    // 4. Run physics simulation for 100 frames, generating dynamic islands on structural dirty
    for step in 0..100 {
        if grid.structural_dirty {
            let islands = grid.detect_unsupported_islands();
            for island in islands {
                phys.spawn_island_chunk(island, Vec3::ZERO, Vec3::ZERO);
            }
        }

        phys.update(0.033, &mut grid, &audio, cam_pos);

        // Audit: Verify no dynamic chunk settles suspended in mid-air
        for (c_idx, chunk) in phys.chunks.iter().enumerate() {
            if chunk.settled {
                let support_gap = external_support_gap(c_idx, &phys, &grid);
                assert!(
                    // Negative separation is a penetrating contact, not an airborne gap.
                    support_gap.is_some_and(|gap| gap <= CONTACT_SLOP + 1e-5),
                    "Step {}: Chunk {} settled without an external foot contact at pos {:?} (support_gap = {:?})",
                    step, c_idx, chunk.position, support_gap
                );
            }
        }
    }

    // 5. Final exhaustive scan of static grid: ZERO floating sky-hooks!
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Demolition run left {} floating sky-hooks in static grid! Samples: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

#[test]
fn test_suite4_zero_sky_hooks_cantilever_boundary_enforcement() {
    let mut grid = VoxelGrid::new();

    // Central grounded column from y = 1 to y = 20 at (40, 40)
    for y in 1..=20 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }

    // Cantilever arm extending along +X from x = 41 to x = 52 (length 12 voxels)
    // CANTILEVER_MAX_DEPTH is 4 voxels (2.0m). Material is Concrete (non-tensile).
    for x in 41..=52 {
        grid.set(x, 15, 40, VoxelType::Concrete);
    }

    // Detect unsupported islands: cantilever beyond 4 voxels must detach!
    let islands = grid.detect_unsupported_islands();
    assert!(
        !islands.is_empty(),
        "Over-extended cantilever (> 4 voxels) must detach as dynamic island(s)"
    );

    // Remaining static grid must have zero floating voxels
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Cantilever detachment left {} floating sky-hooks!",
        sky_hooks.len()
    );
}

#[test]
fn test_suite4_zero_sky_hooks_subterranean_bedrock_minus_24m_true_grounding() {
    let mut grid = VoxelGrid::new();

    // Excavate subterranean shaft down to MIN_VOXEL_Y (-48, world -24.0m) across 38..=42, 38..=42
    fill_rect(&mut grid, 38, 42, MIN_VOXEL_Y, 5, 38, 42, VoxelType::Air);
    let _ = grid.detect_unsupported_islands();

    // Build column terminating at y = -40 (world -20.0m) up to y = 5, hanging over a 4m void down to -24m
    for y in -40..=5 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }
    fill_rect(&mut grid, 39, 41, 5, 5, 39, 41, VoxelType::Concrete);

    // Because column stops at -20m and does not reach true bedrock at -24m (-48), it MUST detach!
    let islands = grid.detect_unsupported_islands();
    let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    assert_eq!(
        total_island_voxels, 54,
        "Structure terminating at -20m must detach (54 voxels); only {} detected!",
        total_island_voxels
    );

    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Structure terminating at -20m left {} floating sky-hook voxels anchored to false ground!",
        sky_hooks.len()
    );
}

// ============================================================================
// SUITE 5: MULTI-CHUNK STACKING STABILITY (>48 CHUNKS) & CRATER RE-AWAKENING (3 TESTS)
// ============================================================================

#[test]
fn test_suite5_multi_chunk_stacking_stability_64_chunks_100_frames() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Flat Bedrock slab at y = 0, x in 10..30, z in 10..30
    fill_rect(&mut grid, 10, 30, 0, 0, 10, 30, VoxelType::Bedrock);

    // Build 64 chunks (>48 chunks) arranged in 4 layers of 4x4 chunks
    let mut initial_positions = Vec::with_capacity(64);
    for layer in 0..4 {
        let y = 0.55 + layer as f32 * 1.05;
        for cx in 0..4 {
            for cz in 0..4 {
                let x = 16.0 + cx as f32 * 1.15;
                let z = 16.0 + cz as f32 * 1.15;
                let pos = Vec3::new(x, y, z);
                let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
                let mut chunk = create_test_chunk(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
                chunk.settled = true;
                initial_positions.push(pos);
                phys.chunks.push(chunk);
            }
        }
    }

    assert_eq!(phys.chunks.len(), 64);

    // Step physics for 120 frames with dt = 0.016s (~2 seconds of simulation)
    for _ in 0..120 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // Assertions: 100% stable, zero drift, zero phantom waking, zero premature despawns
    assert_eq!(phys.chunks.len(), 64, "All 64 chunks must remain active in pool");
    for (i, chunk) in phys.chunks.iter().enumerate() {
        assert!(chunk.settled, "Chunk {} unexpectedly woke up from sleep state", i);
        assert_eq!(chunk.velocity, Vec3::ZERO, "Chunk {} gained non-zero velocity while settled", i);
        assert_eq!(chunk.angular_velocity, Vec3::ZERO, "Chunk {} gained angular velocity", i);
        let drift = (chunk.position - initial_positions[i]).length();
        assert!(
            drift < 1e-4,
            "Chunk {} drifted by {}m (settled chunks must experience 0 drift)",
            i, drift
        );
    }
}

#[test]
fn test_suite5_crater_excavation_reawakens_settled_chunks_rubble_dirty() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Solid concrete ground at center of world (voxel 45, 0, 45)
    let center_voxel = IVec3::new(45, 0, 45);
    let center_world = grid.voxel_to_world(center_voxel);
    fill_rect(&mut grid, 38, 52, 0, 2, 38, 52, VoxelType::Concrete);

    // Stack 60 settled chunks in a dense column/pyramid around center_world
    for layer in 0..6 {
        let y = center_world.y + 1.2 + layer as f32 * 1.05;
        for dx in 0..10 {
            if phys.chunks.len() >= 60 { break; }
            let x = center_world.x + (dx % 3) as f32 * 1.1 - 1.1;
            let z = center_world.z + (dx / 3) as f32 * 1.1 - 1.1;
            let pos = Vec3::new(x, y, z);
            let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
            let mut chunk = create_test_chunk(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
            chunk.settled = true;
            phys.chunks.push(chunk);
        }
    }
    assert_eq!(phys.chunks.len(), 60);

    // Detonate explosive crater charge at center of pile with radius 5.0m centered at y = -0.5m
    let blast_pos = center_world - Vec3::new(0.0, 0.75, 0.0);
    let blast_radius = 5.0;
    let res = grid.carve_sphere_advanced(blast_pos, blast_radius, 1500.0, DamageType::Explosive);
    assert!(res.destroyed > 0, "Crater must destroy ground voxels");

    phys.rubble_dirty = true;
    let min_p = grid.world_to_voxel(blast_pos - Vec3::splat(blast_radius + 1.0));
    let max_p = grid.world_to_voxel(blast_pos + Vec3::splat(blast_radius + 1.0));
    phys.mark_rubble_region_dirty(min_p, max_p);
    phys.apply_radial_blast(blast_pos, blast_radius * 1.6, 50.0);

    assert!(phys.rubble_dirty || phys.rubble_dirty_region.is_some(), "rubble_dirty must be flagged");

    // Advance 1 physics step: chunks must re-awaken
    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    let active_chunks = phys.chunks.iter().filter(|c| !c.settled).count();
    assert!(
        active_chunks >= 50,
        "At least 50 of 60 chunks must re-awaken, got {}",
        active_chunks
    );

    // Track downward chunks as gravity reverses upward blast trajectory
    let mut max_downward_chunks = 0;
    for _ in 1..=40 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        let downward = phys.chunks.iter().filter(|c| !c.settled && c.velocity.y < 0.0).count();
        if downward > max_downward_chunks {
            max_downward_chunks = downward;
        }
    }
    assert!(
        max_downward_chunks >= 30,
        "Re-awakened chunks must tumble downward into crater (observed max downward = {})",
        max_downward_chunks
    );

    // Step physics for remaining 40 frames: verify chunks tumble into crater void
    for _ in 0..40 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    let lowest_bottom_y = phys.chunks.iter().map(|c| c.lowest_world_y()).fold(f32::INFINITY, f32::min);
    assert!(
        lowest_bottom_y < 0.25,
        "Chunks must tumble into subterranean crater void (bottom reached {}), must be below initial surface 0.25",
        lowest_bottom_y
    );
}

#[test]
fn test_suite5_selective_reawakening_crater_vs_supported_pile() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Flat concrete ground
    let pile_a_center_voxel = IVec3::new(35, 0, 45);
    let pile_a_world = grid.voxel_to_world(pile_a_center_voxel);
    let pile_b_center_voxel = IVec3::new(60, 0, 45);
    let pile_b_world = grid.voxel_to_world(pile_b_center_voxel);

    fill_rect(&mut grid, 25, 70, 0, 1, 35, 55, VoxelType::Concrete);

    // Pile A: 30 settled chunks centered around pile_a_world
    let mut pile_a_indices = Vec::new();
    let mut pile_a_initial_positions = Vec::new();
    for layer in 0..3 {
        let y = pile_a_world.y + 1.2 + layer as f32 * 1.05;
        for i in 0..10 {
            let x = pile_a_world.x + (i % 3) as f32 * 1.1 - 1.1;
            let z = pile_a_world.z + (i / 3) as f32 * 1.1 - 1.1;
            let pos = Vec3::new(x, y, z);
            let mut chunk = create_test_chunk(build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete), pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
            chunk.settled = true;
            pile_a_indices.push(phys.chunks.len());
            pile_a_initial_positions.push(pos);
            phys.chunks.push(chunk);
        }
    }

    // Pile B: 30 settled chunks centered around pile_b_world (12.5m away from Pile A)
    let mut pile_b_indices = Vec::new();
    let mut pile_b_initial_positions = Vec::new();
    for layer in 0..3 {
        let y = pile_b_world.y + 1.2 + layer as f32 * 1.05;
        for i in 0..10 {
            let x = pile_b_world.x + (i % 3) as f32 * 1.1 - 1.1;
            let z = pile_b_world.z + (i / 3) as f32 * 1.1 - 1.1;
            let pos = Vec3::new(x, y, z);
            let mut chunk = create_test_chunk(build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete), pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
            chunk.settled = true;
            pile_b_indices.push(phys.chunks.len());
            pile_b_initial_positions.push(pos);
            phys.chunks.push(chunk);
        }
    }

    assert_eq!(phys.chunks.len(), 60);

    // Detonate explosive crater charge ONLY at Pile A
    let blast_pos = pile_a_world;
    let blast_radius = 3.5;
    let res = grid.carve_sphere_advanced(blast_pos, blast_radius, 1200.0, DamageType::Explosive);
    assert!(res.destroyed > 0);

    phys.rubble_dirty = true;
    let min_p = grid.world_to_voxel(blast_pos - Vec3::splat(blast_radius + 1.0));
    let max_p = grid.world_to_voxel(blast_pos + Vec3::splat(blast_radius + 1.0));
    phys.mark_rubble_region_dirty(min_p, max_p);
    phys.apply_radial_blast(blast_pos, blast_radius * 1.6, 40.0);

    // Advance 1 step: verify Pile A chunks re-awaken immediately
    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    let pile_a_woken = pile_a_indices.iter().filter(|&&idx| !phys.chunks[idx].settled).count();
    assert!(
        pile_a_woken >= 20,
        "Pile A must re-awaken immediately on blast (got {} active of 30)",
        pile_a_woken
    );

    // Step physics remaining 59 frames
    for _ in 0..59 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // Pile A chunks must have tumbled and displaced into crater
    let pile_a_displaced = pile_a_indices.iter().enumerate().filter(|(i, &idx)| {
        (phys.chunks[idx].position - pile_a_initial_positions[*i]).length() > 0.3
    }).count();
    assert!(
        pile_a_displaced >= 20,
        "Pile A chunks must tumble and displace into crater (got {} of 30)",
        pile_a_displaced
    );

    // Pile B must remain 100% settled with zero drift throughout
    for (i, &idx) in pile_b_indices.iter().enumerate() {
        let chunk = &phys.chunks[idx];
        assert!(chunk.settled, "Pile B chunk {} unexpectedly woke up", i);
        assert_eq!(chunk.velocity, Vec3::ZERO);
        let drift = (chunk.position - pile_b_initial_positions[i]).length();
        assert!(drift < 1e-4, "Pile B chunk {} drifted by {}m", i, drift);
    }
}
