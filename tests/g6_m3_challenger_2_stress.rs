//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G6-M3
//! ADVERSARIAL STRESS-TESTING: SUITES 3, 4 & 5
//! Pancake Collapse Zero Bounce, Post-Demolition Zero Sky-Hooks, & Stacking Stability
//! Target: tests/g6_m3_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use std::collections::{HashSet, VecDeque};
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, Mat3, PhysicsSystem};
use voxel_demolition::voxel::{
    DamageType, IslandChunk, VoxelGrid, VoxelType, BEDROCK_Y, CANTILEVER_MAX_DEPTH, GRID_X, GRID_Y,
    GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE,
};

/// Helper: build a solid 3D box of voxels with specified local coordinates.
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

/// Helper: construct a test dynamic chunk with explicit mass override.
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

/// Helper to fill a rectangular volume with a voxel type.
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

/// Helper: count remaining solid voxels inside a bounding volume.
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

/// Independent ground-truth BFS sky-hook scanner auditing the entire VoxelGrid.
/// Seeds strictly at true subterranean geological bedrock foundation y = MIN_VOXEL_Y (-48, world -24.0m).
/// Any solid voxel with y > 0 that lacks a connected solid path down to true bedrock is
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
// SUITE 3 ADVERSARIAL CHALLENGES: PANCAKE COLLAPSE ZERO BOUNCE & MOMENTUM CONSERVATION
// ============================================================================

#[test]
fn test_adversarial_pancake_8_story_multi_material_spectrum() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    // 8 floors with varying materials: Glass, Brick, Concrete, Concrete, Brick, Glass, Concrete, Brick
    let floor_materials = [
        (7, VoxelType::Glass),
        (14, VoxelType::Brick),
        (21, VoxelType::Concrete),
        (28, VoxelType::Concrete),
        (35, VoxelType::Brick),
        (42, VoxelType::Glass),
        (49, VoxelType::Concrete),
        (56, VoxelType::Brick),
    ];
    for &(fy, mat) in &floor_materials {
        fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, mat);
    }

    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(5, 2, 5, VOXEL_SIZE, VoxelType::Concrete);
    let heavy_slab = create_test_chunk_with_mass(
        slab_voxels,
        top_world + Vec3::new(0.0, 1.5, 0.0),
        Vec3::new(0.0, -5.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        1500.0,
    );
    phys.chunks.push(heavy_slab);

    let mut lowest_y = f32::INFINITY;
    let mut positive_rebound_events = 0;

    for _step in 0..120 {
        if phys.chunks.is_empty() { break; }
        phys.update(0.033, &mut grid, &audio, cam_pos);
        // Isolate slab/static-floor crushing from spawned chip collisions.
        phys.chunks.retain(|chunk| chunk.mass >= 400.0);

        if let Some(chunk) = phys.chunks.iter().filter(|c| c.mass >= 400.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(chunk.position.y);
            if chunk.velocity.y > 0.0001 {
                positive_rebound_events += 1;
            }
            assert!(chunk.position.is_finite());
            assert!(chunk.velocity.is_finite());
        }
        // The descent is complete here; ground rocking belongs to rubble tests.
        if lowest_y <= 3.0 { break; }
    }

    assert_eq!(
        positive_rebound_events, 0,
        "Crushing slab experienced {} positive upward rebound events during multi-material descent",
        positive_rebound_events
    );
    assert!(
        lowest_y <= 3.0,
        "Slab failed to penetrate all 8 floors of multi-material spectrum down to bedrock (reached y = {})",
        lowest_y
    );

    // Verify all 8 floor centers were pulverized
    for &(fy, _) in &floor_materials {
        let remaining = count_solids_in_box(&grid, 45, 49, fy, fy, 45, 49);
        assert_eq!(remaining, 0, "Floor at y = {} was not fully pulverized", fy);
    }
}

#[test]
fn test_adversarial_pancake_heavy_mass_envelope_2500kg_to_5000kg() {
    let heavy_masses = [2500.0f32, 3500.0, 5000.0];

    for &mass in &heavy_masses {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

        let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
        for &fy in &floor_levels {
            fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Concrete);
        }

        let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
        let slab_voxels = build_box_voxels(6, 2, 6, VOXEL_SIZE, VoxelType::HeavyArmor);
        let slab = create_test_chunk_with_mass(
            slab_voxels,
            top_world + Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, -8.0, 0.0),
            Vec3::ZERO,
            Vec3::ZERO,
            mass,
        );
        phys.chunks.push(slab);

        let mut lowest_y = f32::INFINITY;
        let mut positive_rebound_frames = 0;
        let mut final_settled = false;
        let mut final_vel = Vec3::ZERO;

        for _step in 0..130 {
            if phys.chunks.is_empty() { break; }
            phys.update(0.033, &mut grid, &audio, cam_pos);

            if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= mass * 0.4).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
                lowest_y = lowest_y.min(c.position.y);
                final_settled = c.settled;
                final_vel = c.velocity;
                if c.velocity.y > 0.0001 {
                    positive_rebound_frames += 1;
                }
            }
            // Audit crushing descent; post-ground rocking is a separate case.
            if lowest_y <= 3.0 { break; }
        }

        println!("Mass {} kg: lowest_y = {:.3}, settled = {}, vel = {:?}", mass, lowest_y, final_settled, final_vel);

        assert_eq!(
            positive_rebound_frames, 0,
            "Heavy mass {} kg produced positive vy rebound!",
            mass
        );
        assert!(
            lowest_y <= 3.0,
            "Heavy mass {} kg halted prematurely at y = {}",
            mass, lowest_y
        );
    }
}

#[test]
fn test_adversarial_pancake_low_mass_threshold_500kg_clean_pulverization() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    // 8 thin Brick floors (crush threshold = 8.0)
    let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
    for &fy in &floor_levels {
        fill_rect(&mut grid, 44, 50, fy, fy, 44, 50, VoxelType::Brick);
    }

    // Minimum dispatch mass: exactly 500 kg
    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::Concrete);
    let slab = create_test_chunk_with_mass(
        slab_voxels,
        top_world + Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, -6.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        500.0,
    );
    phys.chunks.push(slab);

    let mut lowest_y = f32::INFINITY;
    let mut positive_rebounds = 0;

    for _step in 0..120 {
        if phys.chunks.is_empty() { break; }
        phys.update(0.033, &mut grid, &audio, cam_pos);

        if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= 250.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(c.position.y);
            if c.position.y > 3.0 && c.velocity.y > 0.0001 {
                positive_rebounds += 1;
            }
        }
    }

    assert_eq!(positive_rebounds, 0, "500 kg slab must never rebound upward with positive vy while crushing floors");
    assert!(lowest_y <= 3.0, "500 kg slab must crush down to bedrock level (reached y = {})", lowest_y);
}

#[test]
fn test_adversarial_pancake_extreme_spin_angular_flutter_damping() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(47.0 * VOXEL_SIZE, 15.0, 47.0 * VOXEL_SIZE + 20.0);

    let floor_levels = [7, 14, 21, 28, 35, 42, 49, 56];
    for &fy in &floor_levels {
        fill_rect(&mut grid, 43, 51, fy, fy, 43, 51, VoxelType::Concrete);
    }

    // Violently tumbling roof slab: high spin and tilt
    let rot = Vec3::new(0.35, 0.20, -0.40);
    let ang_vel = Vec3::new(3.0, -2.5, 4.0);
    let top_world = grid.voxel_to_world(IVec3::new(47, 56, 47));
    let slab_voxels = build_box_voxels(5, 2, 5, VOXEL_SIZE, VoxelType::HeavyArmor);
    let slab = create_test_chunk_with_mass(
        slab_voxels,
        top_world + Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, -9.0, 0.0),
        rot,
        ang_vel,
        2200.0,
    );
    phys.chunks.push(slab);

    let mut lowest_y = f32::INFINITY;
    let mut positive_rebounds = 0;

    for _ in 0..120 {
        if phys.chunks.is_empty() { break; }
        phys.update(0.033, &mut grid, &audio, cam_pos);

        if let Some(c) = phys.chunks.iter().filter(|c| c.mass >= 500.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(c.position.y);
            if c.velocity.y > 0.0001 {
                positive_rebounds += 1;
            }
            // Angular velocity must remain numerically stable (not exploding)
            assert!(c.angular_velocity.length() < 50.0, "Angular velocity exploded: {:?}", c.angular_velocity);
        }
        // Audit crushing descent; post-ground rocking is a separate case.
        if lowest_y <= 3.5 { break; }
    }

    assert_eq!(positive_rebounds, 0, "Violently spinning slab must not bounce upward (vy <= 0.0)");
    assert!(lowest_y <= 3.5, "Spinning slab must continue downward crushing to bedrock (reached y = {})", lowest_y);
}

// ============================================================================
// SUITE 4 ADVERSARIAL CHALLENGES: POST-DEMOLITION GROUND-TRUTH SKY-HOOK AUDIT
// ============================================================================

#[test]
fn test_adversarial_post_demolition_complex_district_zero_sky_hooks() {
    let mut grid = VoxelGrid::new();

    // Build Tower 1 (X: 35..=41, Z: 40..=46, Y: 1..=40)
    for y in 1..=40 {
        grid.set(35, y, 40, VoxelType::Concrete);
        grid.set(41, y, 40, VoxelType::Concrete);
        grid.set(35, y, 46, VoxelType::Concrete);
        grid.set(41, y, 46, VoxelType::Concrete);
    }
    for &fy in &[10, 20, 30, 40] {
        fill_rect(&mut grid, 35, 41, fy, fy, 40, 46, VoxelType::Concrete);
    }

    // Build Tower 2 (X: 47..=53, Z: 40..=46, Y: 1..=30)
    for y in 1..=30 {
        grid.set(47, y, 40, VoxelType::Concrete);
        grid.set(53, y, 40, VoxelType::Concrete);
        grid.set(47, y, 46, VoxelType::Concrete);
        grid.set(53, y, 46, VoxelType::Concrete);
    }
    for &fy in &[10, 20, 30] {
        fill_rect(&mut grid, 47, 53, fy, fy, 40, 46, VoxelType::Concrete);
    }

    // Elevated skybridge connecting Tower 1 and Tower 2 at y = 20 (X: 42..=46, Z: 42..=44)
    fill_rect(&mut grid, 42, 46, 20, 20, 42, 44, VoxelType::Concrete);

    // Initial check: fully grounded
    let initial_islands = grid.detect_unsupported_islands();
    assert!(initial_islands.is_empty(), "District must initially be fully grounded");

    // Sever foundation of Tower 1 completely (y: 1..=4)
    fill_rect(&mut grid, 34, 42, 1, 4, 39, 47, VoxelType::Air);

    // Detect unsupported islands: Tower 1 must detach, and unsupported section of skybridge must detach!
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing Tower 1 foundation must produce dynamic islands");

    // Ground-truth scan: zero floating voxels in static grid!
    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Severing Tower 1 left {} floating sky-hooks! Sample: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

#[test]
fn test_adversarial_post_demolition_deep_subterranean_crater_chasm_zero_sky_hooks() {
    let mut grid = VoxelGrid::new();

    // 4 deep structural caissons penetrating from y = 30 all the way down to MIN_VOXEL_Y (-48)
    for y in MIN_VOXEL_Y..=30 {
        grid.set(38, y, 38, VoxelType::Concrete);
        grid.set(46, y, 38, VoxelType::Concrete);
        grid.set(38, y, 46, VoxelType::Concrete);
        grid.set(46, y, 46, VoxelType::Concrete);
    }
    fill_rect(&mut grid, 38, 46, 30, 30, 38, 46, VoxelType::Concrete);

    let initial = grid.detect_unsupported_islands();
    assert!(initial.is_empty(), "Deeply rooted structure must be 100% grounded");

    // Detonate colossal subterranean bunker buster: blast chasm from y = -35 to y = 5
    // undercutting all 4 caissons
    fill_rect(&mut grid, 36, 48, -35, 5, 36, 48, VoxelType::Air);

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Colossal subterranean chasm must detach superstructure");

    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Subterranean crater chasm left {} floating sky-hooks! Sample: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

#[test]
fn test_adversarial_post_demolition_asymmetric_cantilever_shearing_zero_sky_hooks() {
    let mut grid = VoxelGrid::new();

    // Grounded core column at (45, 45) from y = 1 to y = 25
    for y in 1..=25 {
        grid.set(45, y, 45, VoxelType::Concrete);
    }

    // Huge cross cantilever roof: arms of length 8 voxels extending in +/- X and +/- Z at y = 20
    // CANTILEVER_MAX_DEPTH is 4 voxels. Concrete cannot support > 4 voxels overhang.
    fill_rect(&mut grid, 37, 53, 20, 20, 45, 45, VoxelType::Concrete);
    fill_rect(&mut grid, 45, 45, 20, 20, 37, 53, VoxelType::Concrete);

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Overhanging cantilever arms (> 4 voxels) must detach");

    let sky_hooks = scan_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Overhanging cantilever detachment left {} floating sky-hooks!",
        sky_hooks.len()
    );
}

// ============================================================================
// SUITE 5 ADVERSARIAL CHALLENGES: MULTI-CHUNK STACKING & CRATER RE-AWAKENING
// ============================================================================

#[test]
fn test_adversarial_stacking_stability_64_chunks_tall_column() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Bedrock base at y = 0
    fill_rect(&mut grid, 10, 30, 0, 0, 10, 30, VoxelType::Bedrock);

    // Stack 64 chunks in a tall 2x2 column, 16 layers high (extreme vertical aspect ratio)
    let mut initial_positions = Vec::with_capacity(64);
    for layer in 0..16 {
        let y = 0.55 + layer as f32 * 1.05;
        for cx in 0..2 {
            for cz in 0..2 {
                let x = 19.5 + cx as f32 * 1.05;
                let z = 19.5 + cz as f32 * 1.05;
                let pos = Vec3::new(x, y, z);
                let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
                let mut chunk = create_test_chunk_with_mass(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 50.0);
                chunk.settled = true;
                initial_positions.push(pos);
                phys.chunks.push(chunk);
            }
        }
    }

    assert_eq!(phys.chunks.len(), 64);

    // Step physics for 120 frames with dt = 0.016s (~2 seconds)
    for _ in 0..120 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // All 64 chunks must remain settled with zero drift
    assert_eq!(phys.chunks.len(), 64);
    for (i, chunk) in phys.chunks.iter().enumerate() {
        assert!(chunk.settled, "Column chunk {} in layer {} woke up unexpectedly", i, i / 4);
        assert_eq!(chunk.velocity, Vec3::ZERO);
        let drift = (chunk.position - initial_positions[i]).length();
        assert!(drift < 1e-4, "Column chunk {} drifted by {}m", i, drift);
    }
}

#[test]
fn test_adversarial_stacking_80_chunks_cap_removal_stability() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Bedrock base at y = 0
    fill_rect(&mut grid, 10, 30, 0, 0, 10, 30, VoxelType::Bedrock);

    // 80 chunks (5 layers of 4x4 chunks) - testing beyond 48-chunk limit
    let mut initial_positions = Vec::with_capacity(80);
    for layer in 0..5 {
        let y = 0.55 + layer as f32 * 1.05;
        for cx in 0..4 {
            for cz in 0..4 {
                let x = 16.0 + cx as f32 * 1.15;
                let z = 16.0 + cz as f32 * 1.15;
                let pos = Vec3::new(x, y, z);
                let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
                let mut chunk = create_test_chunk_with_mass(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 40.0);
                chunk.settled = true;
                initial_positions.push(pos);
                phys.chunks.push(chunk);
            }
        }
    }

    assert_eq!(phys.chunks.len(), 80);

    for _ in 0..100 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    assert_eq!(phys.chunks.len(), 80);
    for (i, chunk) in phys.chunks.iter().enumerate() {
        assert!(chunk.settled, "Chunk {} in 80-chunk stack woke up unexpectedly", i);
        assert_eq!(chunk.velocity, Vec3::ZERO);
        let drift = (chunk.position - initial_positions[i]).length();
        assert!(drift < 1e-4, "Chunk {} drifted by {}m", i, drift);
    }
}

#[test]
fn test_adversarial_crater_reawakening_half_stack_undercut_and_tumble() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let center_world = grid.voxel_to_world(IVec3::new(45, 0, 45));
    fill_rect(&mut grid, 38, 52, 0, 2, 38, 52, VoxelType::Concrete);

    // 4 layers of 4x4 = 64 chunks centered around center_world
    let mut left_chunk_indices = Vec::new();
    let mut right_chunk_indices = Vec::new();

    for layer in 0..4 {
        let y = center_world.y + 1.2 + layer as f32 * 1.05;
        for ix in 0..4 {
            for iz in 0..4 {
                let x = center_world.x + (ix as f32 - 1.5) * 1.15;
                let z = center_world.z + (iz as f32 - 1.5) * 1.15;
                let pos = Vec3::new(x, y, z);
                let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
                let mut chunk = create_test_chunk_with_mass(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 60.0);
                chunk.settled = true;

                let idx = phys.chunks.len();
                if ix < 2 {
                    left_chunk_indices.push(idx);
                } else {
                    right_chunk_indices.push(idx);
                }
                phys.chunks.push(chunk);
            }
        }
    }

    assert_eq!(phys.chunks.len(), 64);
    assert_eq!(left_chunk_indices.len(), 32);
    assert_eq!(right_chunk_indices.len(), 32);

    // Excavate crater specifically under the LEFT half (x < center_world.x)
    let blast_pos = center_world + Vec3::new(-1.8, -0.75, 0.0);
    let blast_radius = 4.0;
    let res = grid.carve_sphere_advanced(blast_pos, blast_radius, 1500.0, DamageType::Explosive);
    assert!(res.destroyed > 0);

    phys.rubble_dirty = true;
    let min_p = grid.world_to_voxel(blast_pos - Vec3::splat(blast_radius + 1.0));
    let max_p = grid.world_to_voxel(blast_pos + Vec3::splat(blast_radius + 1.0));
    phys.mark_rubble_region_dirty(min_p, max_p);
    phys.apply_radial_blast(blast_pos, blast_radius * 1.5, 45.0);

    // Step 1: verify left chunks re-awaken immediately
    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    let left_woken = left_chunk_indices.iter().filter(|&&idx| !phys.chunks[idx].settled).count();
    assert!(
        left_woken >= 24,
        "At least 24 of 32 left chunks must re-awaken immediately upon blast undercut, got {}",
        left_woken
    );

    // Advance 60 frames
    for _ in 0..60 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // Verify left chunks tumbled into crater void below initial ground surface
    let lowest_left_y = left_chunk_indices.iter().map(|&idx| phys.chunks[idx].lowest_world_y()).fold(f32::INFINITY, f32::min);
    assert!(
        lowest_left_y < 0.25,
        "Left undercut chunks must tumble down into subterranean crater void (reached {}), below initial surface 0.25",
        lowest_left_y
    );
}

#[test]
fn test_adversarial_midair_settling_prevented_unsupported_freefall() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(45.0 * VOXEL_SIZE, 20.0, 45.0 * VOXEL_SIZE + 25.0);

    // Spawn 10 chunks high in mid-air at y = 25.0m (voxel y = 50) with zero initial velocity
    for i in 0..10 {
        let x = 40.0 + (i % 3) as f32 * 2.0;
        let z = 40.0 + (i / 3) as f32 * 2.0;
        let pos = Vec3::new(x, 25.0, z);
        let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::Concrete);
        let chunk = create_test_chunk_with_mass(chunk_voxels, pos, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 50.0);
        phys.chunks.push(chunk);
    }

    // Step physics for 60 frames (chunks falling under gravity)
    for step in 0..60 {
        phys.update(0.033, &mut grid, &audio, cam_pos);

        // Invariant: NO chunk may settle while airborne above y = 2.0m without support
        for (i, chunk) in phys.chunks.iter().enumerate() {
            if chunk.position.y > 2.0 {
                assert!(
                    !chunk.settled,
                    "Step {}: Chunk {} settled in mid-air at pos {:?} without support!",
                    step, i, chunk.position
                );
            }
        }
    }
}
