//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE 1: MILESTONE G6-M2
//! TOPPLING DOMINO COLLAPSE, MULTI-DIRECTIONAL CRUSHING & EDGE-CASE HARDENING
//! Target: tests/g6_m2_challenger_1_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, Mat3, PhysicsSystem};
use voxel_demolition::voxel::{
    IslandChunk, VoxelGrid, VoxelType, BEDROCK_Y, GRID_X, GRID_Y, GRID_Z, MAX_VOXEL_Y,
    MIN_VOXEL_Y, VOXEL_SIZE,
};

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

/// Helper: count remaining solid voxels inside a specific volume
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

/// Helper to construct a dynamic chunk from a grid box
fn create_chunk_from_box(
    center: Vec3,
    vel: Vec3,
    ang_vel: Vec3,
    nx: usize,
    ny: usize,
    nz: usize,
    vtype: VoxelType,
) -> DynamicChunk {
    let count = nx * ny * nz;
    let mut voxels = Vec::with_capacity(count);
    let hx = (nx as f32 - 1.0) * 0.5 * VOXEL_SIZE;
    let hy = (ny as f32 - 1.0) * 0.5 * VOXEL_SIZE;
    let hz = (nz as f32 - 1.0) * 0.5 * VOXEL_SIZE;

    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let local_p = Vec3::new(
                    x as f32 * VOXEL_SIZE - hx,
                    y as f32 * VOXEL_SIZE - hy,
                    z as f32 * VOXEL_SIZE - hz,
                );
                voxels.push((local_p, vtype));
            }
        }
    }

    let mass = count as f32 * vtype.properties().density;
    let aabb = DynamicChunk::compute_aabb(&voxels);
    DynamicChunk::new(
        center,
        vel,
        Vec3::ZERO,
        ang_vel,
        voxels,
        mass.max(1.0),
        0.0,
        false,
        aabb,
    )
}

// ============================================================================
// SUITE 1: 100M+ SKYSCRAPER TOPPLING SIDEWAYS INTO ADJACENT BUILDING
// ============================================================================
// Tests dispatch requirement:
// - 100m+ structure (200 voxels tall = 100.0m) toppling sideways into an adjacent building
// - Effective 3D speed (|v| + |omega| * r > 0.5) pulverizes static voxels in adjacent tower
// - Physical damage occurs to receiving tower (island count increases or severed voxels become dynamic chunks)
// - No simulation stalls, zero NaN/infinite floats, smooth execution
// ============================================================================

#[test]
fn test_100m_skyscraper_toppling_sideways_into_adjacent_building() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // 1. Build Tower B (the receiving building):
    // Position: X in 42..=46, Z in 46..=50, Y from 1 up to 180 (90.0m tall)
    let b_min_x = 42;
    let b_max_x = 46;
    let b_min_z = 46;
    let b_max_z = 50;
    let b_max_y = 180;
    fill_rect(
        &mut grid,
        b_min_x,
        b_max_x,
        1,
        b_max_y,
        b_min_z,
        b_max_z,
        VoxelType::Concrete,
    );
    let tower_b_initial_solids = count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, b_min_z, b_max_z);
    assert!(tower_b_initial_solids > 0, "Tower B must have solid voxels");

    // 2. Build Tower A (100m+ skyscraper toppling into Tower B):
    // 200 voxels tall = 100.0m tall! (Y = 1..=200).
    // Position: X in 26..=30 (gap of 12 voxels = 6.0m to Tower B), Z in 46..=50.
    let a_min_x = 26;
    let a_max_x = 30;
    let a_min_z = 46;
    let a_max_z = 50;
    let a_max_y = 200; // 200 * 0.5m = 100.0m tall
    fill_rect(
        &mut grid,
        a_min_x,
        a_max_x,
        1,
        a_max_y,
        a_min_z,
        a_max_z,
        VoxelType::Concrete,
    );

    // Verify initial stability: both towers are grounded
    grid.structural_dirty = true;
    let initial_islands = grid.detect_unsupported_islands();
    assert_eq!(
        initial_islands.len(),
        0,
        "Both towers must be initially grounded with zero unsupported islands"
    );

    // 3. Sever Tower A's foundation at Y=1..=4
    fill_rect(
        &mut grid,
        a_min_x,
        a_max_x,
        1,
        4,
        a_min_z,
        a_max_z,
        VoxelType::Air,
    );
    grid.structural_dirty = true;

    // Detect severed Tower A as an unsupported island
    let severed_islands = grid.detect_unsupported_islands();
    assert_eq!(
        severed_islands.len(),
        1,
        "Severed 100m Tower A must be detected as 1 unsupported island"
    );
    let tower_a_island = severed_islands.into_iter().next().unwrap();
    assert!(
        tower_a_island.voxels.len() >= 4000,
        "Tower A island must contain >= 4000 voxels, got {}",
        tower_a_island.voxels.len()
    );

    // Spawn Tower A with angular impulse toppling sideways into Tower B (+X direction)
    // Negative Z angular velocity rotates +Y towards +X:
    // omega = (0.0, 0.0, -0.75 rad/s)
    let initial_impulse = Vec3::new(1.5, -0.5, 0.0);
    let angular_impulse = Vec3::new(0.0, 0.0, -0.75);
    phys.spawn_island_chunk(tower_a_island, initial_impulse, angular_impulse);
    assert_eq!(phys.chunks.len(), 1);

    // 4. Run multi-step simulation and verify physical damage to Tower B
    let dt = 1.0 / 60.0;
    let max_steps = 120; // 2.0 seconds of simulation
    let mut tower_b_damaged = false;
    let mut secondary_islands_total = 0;
    let mut max_effective_speed = 0.0f32;

    for step in 0..max_steps {
        // Track effective speed before step
        for chunk in &phys.chunks {
            let (min_aabb, max_aabb) = chunk.local_aabb;
            let half_extent = (max_aabb - min_aabb) * 0.5;
            let eff_spd = chunk.velocity.length() + chunk.angular_velocity.length() * half_extent.length();
            if eff_spd > max_effective_speed {
                max_effective_speed = eff_spd;
            }
        }

        // Step physics
        let _trauma = phys.update(dt, &mut grid, &sound, cam_pos);

        // Check for NaN or Inf in chunk states (robustness invariant)
        for chunk in &phys.chunks {
            assert!(
                chunk.position.is_finite(),
                "Step {}: Chunk position must remain finite, got {:?}",
                step,
                chunk.position
            );
            assert!(
                chunk.velocity.is_finite(),
                "Step {}: Chunk velocity must remain finite, got {:?}",
                step,
                chunk.velocity
            );
            assert!(
                chunk.angular_velocity.is_finite(),
                "Step {}: Chunk angular_velocity must remain finite, got {:?}",
                step,
                chunk.angular_velocity
            );
        }

        // Check if Tower B sustained damage
        let current_b_solids = count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, b_min_z, b_max_z);
        if current_b_solids < tower_b_initial_solids {
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
                        Vec3::new(rand::gen_range(0.5, 2.0), -1.0, 0.0),
                        Vec3::new(0.0, 0.0, -0.5),
                    );
                }
            }
        }

        if tower_b_damaged && secondary_islands_total > 0 {
            // Both physical damage and secondary structural collapse confirmed!
            break;
        }
    }

    // 5. Verification Assertions:
    assert!(
        max_effective_speed > 0.5,
        "Effective 3D speed must exceed 0.5 m/s, reached: {:.2} m/s",
        max_effective_speed
    );
    assert!(
        tower_b_damaged,
        "Tower B must sustain physical damage from toppling Tower A (initial: {}, remaining: {})",
        tower_b_initial_solids,
        count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, b_min_z, b_max_z)
    );
    assert!(
        secondary_islands_total > 0,
        "Secondary island collapse must occur in Tower B as structural columns are severed! Total islands: {}",
        secondary_islands_total
    );
    assert!(
        phys.chunks.len() >= 2,
        "Dynamic chunk count must increase due to shattered sections and secondary collapse"
    );
}

// ============================================================================
// SUITE 2: MULTI-DIRECTIONAL CRUSHING WITHOUT VERTICAL SPEED (vy = 0)
// ============================================================================
// Verifies that horizontal flight and pure angular rotation (vy = 0)
// pulverizes static structures, overcoming the old vy < -0.25 limitation.
// ============================================================================

#[test]
fn test_pure_horizontal_velocity_pulverizes_static_wall_with_zero_vy() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 10.0, 50.0);

    // Build a static Concrete wall at X = 50, Y = 5..=15, Z = 45..=51 (1 voxel thick in X)
    let wall_x = 50;
    fill_rect(&mut grid, wall_x, wall_x, 5, 15, 45, 51, VoxelType::Concrete);
    let initial_solids = count_solids_in_box(&grid, wall_x, wall_x, 5, 15, 45, 51);
    assert!(initial_solids > 0);

    // Create a 5x5x5 heavy dynamic chunk at X = 45, Y = 10, Z = 48 moving purely in +X
    // Strictly ZERO vertical velocity: vy = 0.0!
    // Horizontal speed = 12.0 m/s
    let chunk_pos = grid.voxel_to_world(IVec3::new(45, 10, 48));
    let chunk_vel = Vec3::new(12.0, 0.0, 0.0); // vy == 0.0
    let chunk_ang = Vec3::ZERO;
    let chunk = create_chunk_from_box(chunk_pos, chunk_vel, chunk_ang, 4, 4, 4, VoxelType::HeavyArmor);

    phys.chunks.push(chunk);

    // Step physics for 20 frames
    let dt = 1.0 / 60.0;
    let mut wall_crushed = false;
    for _ in 0..20 {
        // Enforce vy = 0.0 to strictly test horizontal crushing without vertical assistance
        if !phys.chunks.is_empty() {
            phys.chunks[0].velocity.y = 0.0;
        }

        phys.update(dt, &mut grid, &sound, cam_pos);

        let remaining = count_solids_in_box(&grid, wall_x, wall_x, 5, 15, 45, 51);
        if remaining < initial_solids {
            wall_crushed = true;
            break;
        }
    }

    assert!(
        wall_crushed,
        "Horizontal kinetic crushing must pulverize static wall voxels even when vy = 0.0!"
    );
    assert!(grid.structural_dirty, "Structural dirty flag must be set upon crushing");
}

#[test]
fn test_pure_rotational_velocity_pulverizes_static_column_with_zero_linear_speed() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 10.0, 50.0);

    // Build a static Concrete column at X = 52, Y = 10, Z = 48
    let col_x = 52;
    fill_rect(&mut grid, col_x, col_x, 8, 12, 48, 48, VoxelType::Concrete);
    let initial_solids = count_solids_in_box(&grid, col_x, col_x, 8, 12, 48, 48);
    assert!(initial_solids > 0);

    // Create an elongated dynamic bar chunk centered at X = 48, Y = 10, Z = 48
    // with ZERO linear velocity: v = (0, 0, 0)
    // and high angular velocity around Y: omega = (0, 8.0, 0)
    // The tips of the bar will swing through X = 52!
    let center = grid.voxel_to_world(IVec3::new(48, 10, 48));
    let vel = Vec3::ZERO;
    let ang_vel = Vec3::new(0.0, 8.0, 0.0);
    // 16 voxels long along X = 8.0m long, half-extent = 4.0m
    let bar_chunk = create_chunk_from_box(center, vel, ang_vel, 16, 2, 2, VoxelType::HeavyArmor);

    let (min_aabb, max_aabb) = bar_chunk.local_aabb;
    let half_extent = (max_aabb - min_aabb) * 0.5;
    let eff_speed = ang_vel.length() * half_extent.length();
    assert!(eff_speed > 0.5, "Effective speed must be > 0.5 m/s");

    phys.chunks.push(bar_chunk);

    // Step physics
    let dt = 1.0 / 60.0;
    let mut column_crushed = false;
    for _ in 0..15 {
        // Enforce zero linear velocity
        if !phys.chunks.is_empty() {
            phys.chunks[0].velocity = Vec3::ZERO;
            phys.chunks[0].angular_velocity = ang_vel; // maintain rotation
        }

        phys.update(dt, &mut grid, &sound, cam_pos);

        let remaining = count_solids_in_box(&grid, col_x, col_x, 8, 12, 48, 48);
        if remaining < initial_solids {
            column_crushed = true;
            break;
        }
    }

    assert!(
        column_crushed,
        "Pure rotational angular velocity must pulverize static architecture via effective speed"
    );
}

// ============================================================================
// SUITE 3: SWEPT ANTI-TUNNELING AT EXTREME VELOCITIES
// ============================================================================
// Fast-moving chunks (v > 35 m/s) travel further than VOXEL_SIZE in one frame.
// Swept probing along wpos - v*dt must catch 1-voxel thin barriers and prevent tunneling.
// ============================================================================

#[test]
fn test_swept_anti_tunneling_catches_thin_barrier_at_high_speed() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 10.0, 50.0);

    // Build a 1-voxel thin Concrete wall at X = 50, spanning Y = 5..=15, Z = 45..=51
    let wall_x = 50;
    fill_rect(&mut grid, wall_x, wall_x, 5, 15, 45, 51, VoxelType::Concrete);
    let initial_solids = count_solids_in_box(&grid, wall_x, wall_x, 5, 15, 45, 51);

    // Place a compact high-mass penetrator chunk at X = 48.5 (world space: ~1.5 voxels before wall)
    // Moving at vx = 40.0 m/s. In dt = 1/60s, it moves 0.67m (> VOXEL_SIZE 0.5m).
    // In a single discrete step without swept collision, it leaps from X = 49.25 to X = 50.6,
    // skipping cleanly past the voxel at X = 50.0!
    let start_pos = grid.voxel_to_world(IVec3::new(48, 10, 48));
    let fast_vel = Vec3::new(40.0, 0.0, 0.0);
    let penetrator = create_chunk_from_box(start_pos, fast_vel, Vec3::ZERO, 2, 2, 2, VoxelType::HeavyArmor);

    phys.chunks.push(penetrator);

    let dt = 1.0 / 60.0;
    let mut wall_impacted = false;

    // Single step
    phys.update(dt, &mut grid, &sound, cam_pos);

    let remaining_solids = count_solids_in_box(&grid, wall_x, wall_x, 5, 15, 45, 51);
    if remaining_solids < initial_solids {
        wall_impacted = true;
    }

    assert!(
        wall_impacted,
        "Swept collision candidate points must detect and crush thin 1-voxel wall at 40 m/s without tunneling!"
    );
}

// ============================================================================
// SUITE 4: LOCALIZED BFS CEILING EXPANSION ACROSS MULTI-STORY AIR GAPS
// ============================================================================
// Verifies that when a foundation is severed, localized BFS expands across
// interior air gaps (window voids, atriums) up to the tower top without leaving sky-hooks.
// ============================================================================

#[test]
fn test_localized_bfs_expands_across_interior_air_gap_to_prevent_sky_hooks() {
    let mut grid = VoxelGrid::new();

    // Construct a 90m tall tower (180 voxels) at X=30..=34, Z=46..=50:
    // Lower segment: Y = 1..=15 (ground floor & podium)
    // Open air gap: Y = 16..=26 (11-voxel open glass atrium / structural air gap)
    // Upper skyscraper: Y = 27..=180 (154 voxels of heavy skyscraper suspended above)
    fill_rect(&mut grid, 30, 34, 1, 15, 46, 50, VoxelType::Concrete);
    // Atrium supported only by thin corner pillars at Y = 16..=26:
    fill_rect(&mut grid, 30, 30, 16, 26, 46, 46, VoxelType::Concrete); // Column 1
    fill_rect(&mut grid, 34, 34, 16, 26, 50, 50, VoxelType::Concrete); // Column 2
    // Heavy upper tower:
    fill_rect(&mut grid, 30, 34, 27, 180, 46, 50, VoxelType::Concrete);

    // Initial check: fully grounded
    grid.structural_dirty = true;
    let init_islands = grid.detect_unsupported_islands();
    assert_eq!(init_islands.len(), 0, "Initially grounded structure must have 0 islands");

    // Now sever the podium at the very bottom: Y = 1..=3
    // The dirty box will be at min_y = 1, max_y = 3.
    // With pad = 8, initial box_max_y = 3 + 8 = 11.
    // The air gap begins at Y = 16, well above box_max_y!
    fill_rect(&mut grid, 30, 34, 1, 3, 46, 50, VoxelType::Air);
    grid.structural_dirty = true;

    // Run localized BFS:
    let islands = grid.detect_unsupported_islands();

    // Must detect the severed structure:
    assert!(
        !islands.is_empty(),
        "Severing foundation must detect unsupported island(s)"
    );

    // Verify ZERO solid voxels remain floating in the upper tower:
    let remaining_floating_voxels = count_solids_in_box(&grid, 30, 34, 4, 180, 46, 50);
    assert_eq!(
        remaining_floating_voxels, 0,
        "Localized BFS must expand ceiling across the air gap and capture all 180m voxels! Found {} sky-hook voxels!",
        remaining_floating_voxels
    );
}

// ============================================================================
// SUITE 5: TRUE SUBTERRANEAN BEDROCK GROUNDING AT -24M (BEDROCK_Y)
// ============================================================================
// Verifies that structures over subterranean craters must be rooted to
// BEDROCK_Y (-24), not falsely supported at Y=0 over open air.
// ============================================================================

#[test]
fn test_true_bedrock_grounding_at_negative_24m() {
    let mut grid = VoxelGrid::new();

    // Excavate a subterranean shaft around (40, 40) down to BEDROCK_Y (-24)
    // Clearing a 5x5 column from y = BEDROCK_Y to y = 10
    for dx in -2..=2 {
        for dz in -2..=2 {
            for y in BEDROCK_Y..=10 {
                grid.set(40 + dx, y, 40 + dz, VoxelType::Air);
            }
        }
    }

    // Place bedrock at BEDROCK_Y (-24) at the shaft floor
    grid.set(40, BEDROCK_Y, 40, VoxelType::Bedrock);

    // Build a vertical structural column from BEDROCK_Y up to y = 10
    for y in (BEDROCK_Y + 1)..=10 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }

    grid.structural_dirty = true;
    let init_islands = grid.detect_unsupported_islands();
    assert_eq!(
        init_islands.len(),
        0,
        "Column connected down to true bedrock BEDROCK_Y (-24) must be grounded"
    );

    // Sever the column underground at y = -12
    grid.set(40, -12, 40, VoxelType::Air);
    grid.structural_dirty = true;

    let islands = grid.detect_unsupported_islands();

    // The upper column from y=-11 to y=10 is severed from BEDROCK_Y (-24).
    // It must collapse as an unsupported island chunk!
    assert!(
        !islands.is_empty(),
        "Severing column above subterranean bedrock must collapse as unsupported island! Grounding requires connection to BEDROCK_Y (-24)"
    );
    let island = &islands[0];
    assert!(
        island.voxels.iter().any(|(p, _)| p.y == 10),
        "Island must contain the upper portion of the severed column"
    );
}

// ============================================================================
// SUITE 6: MID-AIR SETTLING PREVENTION (NO FLOATING RUBBLE SKY-HOOKS)
// ============================================================================
// Verifies that low-speed or stationary chunks in open space do NOT settle
// into rubble_height_grid mid-air, but continue falling or fracturing.
// ============================================================================

#[test]
fn test_airborne_chunks_do_not_settle_mid_air_into_rubble_grid() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // Spawn a 3x3x3 chunk suspended in mid-air at Y = 45.0m with zero velocity
    let midair_pos = Vec3::new(0.0, 45.0, 0.0);
    let chunk = create_chunk_from_box(midair_pos, Vec3::ZERO, Vec3::ZERO, 3, 3, 3, VoxelType::Concrete);
    phys.chunks.push(chunk);

    let dt = 1.0 / 60.0;

    // Step physics for 30 frames
    for _ in 0..30 {
        phys.update(dt, &mut grid, &sound, cam_pos);
    }

    // Inspect chunk state:
    assert!(
        !phys.chunks.is_empty(),
        "Chunk should still exist or have descended"
    );
    let chunk = &phys.chunks[0];

    // Must NOT be marked as settled while airborne at Y > 10.0m
    if chunk.position.y > 5.0 {
        assert!(
            !chunk.settled,
            "Airborne chunk at Y={:.2}m must NOT settle into rubble heightmap mid-air!",
            chunk.position.y
        );

        // Check rubble height at (0, 0): must NOT be stamped at 45.0m
        if let Some((rx, rz)) = PhysicsSystem::world_to_rubble_grid_coords(0.0, 0.0) {
            let height = phys.rubble_height_grid[rx * GRID_Z + rz];
            assert!(
                height < 5.0,
                "Rubble height grid must not stamp mid-air sky-hook elevations (got {:.2}m)",
                height
            );
        }
    }
}

// ============================================================================
// SUITE 7: RESTING SUPPORT STACK BEYOND 48 CHUNKS
// ============================================================================
// Verifies that the support check extends across ALL chunks, not capped at 48.
// ============================================================================

#[test]
fn test_resting_support_check_extends_beyond_48_chunks() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 20.0, 60.0);

    // Build a vertical stack of 55 settled chunks
    let count = 55;
    for i in 0..count {
        let pos = Vec3::new(0.0, 1.0 + i as f32 * 0.5, 0.0);
        let mut chunk = create_chunk_from_box(pos, Vec3::ZERO, Vec3::ZERO, 2, 1, 2, VoxelType::Concrete);
        chunk.settled = true;
        phys.chunks.push(chunk);
    }
    assert_eq!(phys.chunks.len(), 55);

    // Remove the bottom chunk (index 0) - simulating collapse of the foundation
    phys.chunks.remove(0);
    phys.rubble_dirty = true;

    // Step physics
    let dt = 1.0 / 60.0;
    phys.update(dt, &mut grid, &sound, cam_pos);

    // Chunks beyond index 48 (e.g. index 50) must have been checked and awakened!
    let awakened_count = phys.chunks.iter().filter(|c| !c.settled).count();
    assert!(
        awakened_count > 0,
        "Loss of foundation must wake up resting chunks across the entire stack"
    );
}

// ============================================================================
// SUITE 8: THREE-SKYSCRAPER SEQUENTIAL DOMINO CASCADE (T1 -> T2 -> T3)
// ============================================================================
// Verifies cross-building domino demolition across 3 consecutive skyscrapers:
// Tower 1 (100m) topples into Tower 2 (80m), which topples into Tower 3 (60m).
// ============================================================================

#[test]
fn test_three_skyscraper_domino_cascade() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // Tower 1: 100m tall (Y = 1..=200), X = 16..=20, Z = 46..=50
    let t1_x_min = 16;
    let t1_x_max = 20;
    fill_rect(&mut grid, t1_x_min, t1_x_max, 1, 200, 46, 50, VoxelType::Concrete);

    // Tower 2: 80m tall (Y = 1..=160), X = 28..=32 (gap of 8 voxels = 4.0m), Z = 46..=50
    let t2_x_min = 28;
    let t2_x_max = 32;
    fill_rect(&mut grid, t2_x_min, t2_x_max, 1, 160, 46, 50, VoxelType::Concrete);
    let t2_initial_solids = count_solids_in_box(&grid, t2_x_min, t2_x_max, 1, 160, 46, 50);

    // Tower 3: 60m tall (Y = 1..=120), X = 40..=44 (gap of 8 voxels = 4.0m), Z = 46..=50
    let t3_x_min = 40;
    let t3_x_max = 44;
    fill_rect(&mut grid, t3_x_min, t3_x_max, 1, 120, 46, 50, VoxelType::Concrete);
    let t3_initial_solids = count_solids_in_box(&grid, t3_x_min, t3_x_max, 1, 120, 46, 50);

    // Sever Tower 1 foundation at Y=1..=3
    fill_rect(&mut grid, t1_x_min, t1_x_max, 1, 3, 46, 50, VoxelType::Air);
    grid.structural_dirty = true;

    let t1_islands = grid.detect_unsupported_islands();
    assert_eq!(t1_islands.len(), 1, "Tower 1 must become an unsupported island");
    let t1_chunk = t1_islands.into_iter().next().unwrap();

    // Spawn Tower 1 toppling towards +X into Tower 2
    let impulse = Vec3::new(2.0, -0.2, 0.0);
    let angular = Vec3::new(0.0, 0.0, -0.8);
    phys.spawn_island_chunk(t1_chunk, impulse, angular);

    let dt = 1.0 / 60.0;
    let mut t2_damaged = false;
    let mut t3_damaged = false;
    let mut domino_islands_detected = 0;

    for step in 0..120 {
        phys.update(dt, &mut grid, &sound, cam_pos);

        // Invariant: all chunk positions and velocities must remain finite
        for chunk in &phys.chunks {
            assert!(chunk.position.is_finite(), "Step {}: chunk position must be finite", step);
            assert!(chunk.velocity.is_finite(), "Step {}: chunk velocity must be finite", step);
        }

        if count_solids_in_box(&grid, t2_x_min, t2_x_max, 1, 160, 46, 50) < t2_initial_solids {
            t2_damaged = true;
        }
        if count_solids_in_box(&grid, t3_x_min, t3_x_max, 1, 120, 46, 50) < t3_initial_solids {
            t3_damaged = true;
        }

        if grid.structural_dirty {
            let new_islands = grid.detect_unsupported_islands();
            if !new_islands.is_empty() {
                domino_islands_detected += new_islands.len();
                for island in new_islands {
                    // Propagate toppling momentum forward along +X
                    phys.spawn_island_chunk(
                        island,
                        Vec3::new(2.5, -0.5, 0.0),
                        Vec3::new(0.0, 0.0, -0.8),
                    );
                }
            }
        }

        if t2_damaged && t3_damaged && domino_islands_detected >= 2 {
            break;
        }
    }

    assert!(t2_damaged, "Tower 2 must sustain physical impact damage from Tower 1");
    assert!(t3_damaged, "Tower 3 must sustain domino collapse damage from Tower 2");
    assert!(
        domino_islands_detected >= 2,
        "Domino cascade must produce multiple secondary collapsed islands (got {})",
        domino_islands_detected
    );
}

// ============================================================================
// SUITE 9: EFFECTIVE SPEED THRESHOLD BOUNDARY GATING (0.50 M/S)
// ============================================================================
// Below 0.50 m/s: slow-drifting chunks do NOT pulverize static voxels.
// Above 0.50 m/s: chunks exceed crush threshold and pulverize fragile static voxels.
// ============================================================================

#[test]
fn test_effective_speed_threshold_boundary_gating() {
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 10.0, 50.0);
    let dt = 1.0 / 60.0;

    // Subtest A: Speed = 0.30 m/s (< 0.50 threshold)
    {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();

        // Place a Glass voxel at (40, 10, 48) - Glass is fragile (crush_thresh = 2.0)
        grid.set(40, 10, 48, VoxelType::Glass);

        let center = grid.voxel_to_world(IVec3::new(40, 10, 48));
        let vel = Vec3::new(0.30, 0.0, 0.0); // 0.30 m/s < 0.50 m/s
        let chunk = create_chunk_from_box(center, vel, Vec3::ZERO, 2, 2, 2, VoxelType::Concrete);
        phys.chunks.push(chunk);

        phys.update(dt, &mut grid, &sound, cam_pos);

        // Must NOT be crushed because effective_speed <= 0.50
        assert_eq!(
            grid.get(40, 10, 48),
            VoxelType::Glass,
            "Chunk with effective speed 0.30 m/s (< 0.5) must NOT pulverize static voxel"
        );
    }

    // Subtest B: Speed = 0.80 m/s (> 0.50 threshold)
    {
        let mut grid = VoxelGrid::new();
        let mut phys = PhysicsSystem::new();

        grid.set(40, 10, 48, VoxelType::Glass);

        let center = grid.voxel_to_world(IVec3::new(40, 10, 48));
        let vel = Vec3::new(0.80, 0.0, 0.0); // 0.80 m/s > 0.50 m/s
        // 4x4x4 Concrete chunk has mass = 64 * 2.4 = 153.6 kg. Momentum = 153.6 * 0.8 = 122.8 >> 2.0
        let chunk = create_chunk_from_box(center, vel, Vec3::ZERO, 4, 4, 4, VoxelType::Concrete);
        phys.chunks.push(chunk);

        phys.update(dt, &mut grid, &sound, cam_pos);

        // Must be crushed because effective_speed > 0.50
        assert_eq!(
            grid.get(40, 10, 48),
            VoxelType::Air,
            "Chunk with effective speed 0.80 m/s (> 0.5) must pulverize fragile static voxel"
        );
    }
}

// ============================================================================
// SUITE 10: WEAPON CRATER RUBBLE DIRTY INVALIDATION
// ============================================================================
// When a weapon carves a crater, both the static grid is cleared and
// physics.rubble_dirty is set, causing resting chunks above the crater to awaken.
// ============================================================================

#[test]
fn test_weapon_crater_rubble_dirty_awakening() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 10.0, 50.0);

    // Place a settled chunk resting on the ground at Y = 1.0m (above 0, 0)
    let chunk_pos = Vec3::new(0.0, 1.0, 0.0);
    let mut chunk = create_chunk_from_box(chunk_pos, Vec3::ZERO, Vec3::ZERO, 2, 2, 2, VoxelType::Concrete);
    chunk.settled = true;
    phys.chunks.push(chunk);

    // Weapon detonation: carve crater in voxel grid down into bedrock
    let cx = GRID_X as i32 / 2;
    let cz = GRID_Z as i32 / 2;
    for dx in -3..=3 {
        for dz in -3..=3 {
            for y in -6..=2 {
                grid.set(cx + dx, y, cz + dz, VoxelType::Air);
            }
        }
    }
    grid.structural_dirty = true;

    // Mark rubble dirty (as done by weapon cratering in weapon.rs)
    phys.rubble_dirty = true;
    // Carve crater in rubble heightmap
    for dx in -3..=3 {
        for dz in -3..=3 {
            if let Some((rx, rz)) = PhysicsSystem::world_to_rubble_grid_coords(dx as f32 * 0.5, dz as f32 * 0.5) {
                phys.rubble_height_grid[rx * GRID_Z + rz] = -3.0;
            }
        }
    }

    let dt = 1.0 / 60.0;
    phys.update(dt, &mut grid, &sound, cam_pos);

    // Chunk must wake up and fall into the crater!
    assert!(
        !phys.chunks[0].settled,
        "Crater excavation with rubble_dirty = true must awaken resting chunk above crater"
    );
    assert!(
        phys.chunks[0].velocity.y < 0.0,
        "Awakened chunk must fall downward into crater"
    );
}

// ============================================================================
// SUITE 11: 30,000+ VOXEL MONOLITHIC SKYSCRAPER TOPPLING (PERFORMANCE & ZERO STALLS)
// ============================================================================
// Verifies that a massive 30,000+ voxel structure toppling sideways executes
// within high-performance time budgets with zero allocation leaks or stalls.
// ============================================================================

#[test]
fn test_30000_voxel_monolithic_skyscraper_toppling_performance_and_zero_stalls() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let cam_pos = Vec3::new(0.0, 50.0, 100.0);

    // Construct adjacent receiving tower (8x8x180 = 11,520 voxels) at X = 36..=43
    // Gap of 6 voxels = 3.0m to the toppling skyscraper
    let b_min_x = 36;
    let b_max_x = 43;
    let b_max_y = 180;
    fill_rect(&mut grid, b_min_x, b_max_x, 1, b_max_y, 44, 51, VoxelType::Concrete);
    let tower_b_initial = count_solids_in_box(&grid, b_min_x, b_max_x, 1, b_max_y, 44, 51);
    assert!(tower_b_initial > 0);

    // Construct a colossal 30,000+ voxel skyscraper in the grid:
    // 12 x 12 x 210 = 30,240 voxels (6.0m x 6.0m x 105.0m tall!)
    // Position: X = 18..=29, Z = 42..=53, Y = 1..=210
    let a_min_x = 18;
    let a_max_x = 29;
    let a_min_z = 42;
    let a_max_z = 53;
    let a_max_y = 210;
    fill_rect(&mut grid, a_min_x, a_max_x, 1, a_max_y, a_min_z, a_max_z, VoxelType::Concrete);

    // Sever the colossal tower foundation at Y = 1..=4
    fill_rect(&mut grid, a_min_x, a_max_x, 1, 4, a_min_z, a_max_z, VoxelType::Air);
    grid.structural_dirty = true;

    // Detect severed colossal tower as unsupported island
    let islands = grid.detect_unsupported_islands();
    assert_eq!(islands.len(), 1, "Colossal 30,000+ voxel tower must be detected as 1 unsupported island");
    let colossal_island = islands.into_iter().next().unwrap();
    assert!(
        colossal_island.voxels.len() >= 29000,
        "Colossal island must contain >= 29,000 voxels, got {}",
        colossal_island.voxels.len()
    );

    let vel = Vec3::new(2.5, 0.0, 0.0);
    let ang_vel = Vec3::new(0.0, 0.0, -0.75); // toppling sideways towards receiving tower
    phys.spawn_island_chunk(colossal_island, vel, ang_vel);

    let dt = 1.0 / 60.0;
    let start_time = std::time::Instant::now();
    let mut receiving_crushed = false;

    // Step physics for 60 frames (1.0 second of simulation)
    for step in 0..60 {
        let step_start = std::time::Instant::now();
        phys.update(dt, &mut grid, &sound, cam_pos);
        let step_dur = step_start.elapsed();

        // Each simulation frame must complete in < 50ms (smooth frame budget)
        assert!(
            step_dur.as_millis() < 50,
            "Step {} took {}ms (must be < 50ms for 30,000-voxel toppling)",
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
    }

    let total_elapsed = start_time.elapsed();
    println!(
        "30,000+ voxel toppling: 60 frames simulated in {:.2}ms (avg {:.2}ms/frame)",
        total_elapsed.as_secs_f64() * 1000.0,
        (total_elapsed.as_secs_f64() * 1000.0) / 60.0
    );

    // Receiving tower must sustain physical damage
    assert!(
        receiving_crushed,
        "Colossal 30,000-voxel toppling monolith must crush receiving tower static geometry"
    );
}

