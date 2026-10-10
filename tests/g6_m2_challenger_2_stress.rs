//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G6-M2
//! ADVERSARIAL STRESS-TESTING: ANTI-TUNNELING, BEDROCK GROUNDING (-24M),
//! SKY-HOOK ELIMINATION, AND POST-DEMOLITION GRID INTEGRITY
//! Target: tests/g6_m2_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use std::collections::{HashSet, VecDeque};
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, Mat3, PhysicsSystem};
use voxel_demolition::voxel::{
    VoxelGrid, VoxelType, BEDROCK_Y, CANTILEVER_MAX_DEPTH, GRID_X, GRID_Y, GRID_Z, MAX_VOXEL_Y,
    MIN_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE,
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

/// Helper: construct a test dynamic chunk with given kinematics.
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

/// Helper: independent Ground-Truth Sky-Hook Scanner.
/// Scans the entire VoxelGrid to find any solid voxel that is NOT grounded down
/// to true bedrock (MIN_VOXEL_Y = -48) and exceeds cantilever limit.
fn find_floating_sky_hooks(grid: &VoxelGrid) -> Vec<IVec3> {
    let mut grounded = vec![false; GRID_X * GRID_Y * GRID_Z];
    let mut queue = VecDeque::new();

    // Seed strictly at true bedrock bottom MIN_VOXEL_Y (-48)
    for x in 0..GRID_X {
        for z in 0..GRID_Z {
            let idx = VoxelGrid::idx(x, MIN_VOXEL_Y, z);
            if grid.get(x as i32, MIN_VOXEL_Y, z as i32).is_solid() {
                grounded[idx] = true;
                queue.push_back((x as i32, MIN_VOXEL_Y, z as i32));
            }
        }
    }

    let neighbors = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
        (1, 0, 1),
        (1, 0, -1),
        (-1, 0, 1),
        (-1, 0, -1),
        (1, 1, 0),
        (-1, 1, 0),
        (1, -1, 0),
        (-1, -1, 0),
        (0, 1, 1),
        (0, 1, -1),
        (0, -1, 1),
        (0, -1, -1),
    ];

    while let Some((cx, cy, cz)) = queue.pop_front() {
        for (dx, dy, dz) in neighbors {
            let nx = cx + dx;
            let ny = cy + dy;
            let nz = cz + dz;
            if grid.in_bounds(nx, ny, nz) {
                let ni = VoxelGrid::idx(nx as usize, ny, nz as usize);
                if !grounded[ni] && grid.get(nx, ny, nz).is_solid() {
                    grounded[ni] = true;
                    queue.push_back((nx, ny, nz));
                }
            }
        }
    }

    // Now collect all solid voxels that are ungrounded
    let mut sky_hooks = Vec::new();
    for x in 0..GRID_X as i32 {
        for y in (MIN_VOXEL_Y + 1)..MAX_VOXEL_Y {
            for z in 0..GRID_Z as i32 {
                let idx = VoxelGrid::idx(x as usize, y, z as usize);
                if grid.get(x, y, z).is_solid() && !grounded[idx] {
                    sky_hooks.push(IVec3::new(x, y, z));
                }
            }
        }
    }
    sky_hooks
}

// ============================================================================
// SUITE 1: ANTI-TUNNELING STRESS TESTS (HIGH-SPEED CHUNKS 20-40 M/S VS THIN SLABS)
// ============================================================================

#[test]
fn test_anti_tunneling_high_speed_chunk_pulverizes_thin_floor_slab() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a 1-voxel thin floor slab of Concrete at y = 20 (world y = 10.0m..10.5m)
    // Slab area: 10x10 voxels from x: 40..50, z: 40..50
    let slab_y = 20;
    for x in 40..50 {
        for z in 40..50 {
            grid.set(x, slab_y, z, VoxelType::Concrete);
        }
    }
    let initial_solid_count = grid.solid_count();
    assert!(initial_solid_count >= 100);

    // Slab center in world space
    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));

    // Drop a high-speed chunk (v = 25 m/s downward) directly above the slab
    let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(slab_center.x, 11.2, slab_center.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -25.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    physics.chunks.push(chunk);

    // Step physics with dt = 0.04s
    physics.update(0.04, &mut grid, &audio, slab_center + Vec3::new(0.0, 5.0, 0.0));

    // Verify collision was detected: floor must be pulverized / damaged
    let remaining_solids = grid.solid_count();
    assert!(
        remaining_solids < initial_solid_count,
        "Swept collision probing must detect thin floor slab and pulverize it rather than tunneling through! Initial: {}, Remaining: {}",
        initial_solid_count,
        remaining_solids
    );
}

#[test]
fn test_anti_tunneling_at_extreme_speed_40_meters_per_second() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a 1-voxel thin floor slab of Concrete at y = 21 (world y = 10.5m..11.0m)
    // Thickness: 0.5m
    let slab_y = 21;
    for x in 42..48 {
        for z in 42..48 {
            grid.set(x, slab_y, z, VoxelType::Concrete);
        }
    }
    let slab_voxels_before = 36;
    let slab_center = grid.voxel_to_world(IVec3::new(45, slab_y, 45));

    // Drop a massive chunk at 40 m/s with dt = 0.05s (displacement = 2.0m = 4 voxels!)
    // Chunk starts with bottom at y = 11.2m, ends at y = 9.2m.
    // The slab is at y = 10.5m..11.0m (voxel 21).
    let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(slab_center.x, 11.8, slab_center.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -40.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    physics.chunks.push(chunk);

    // Step physics with dt = 0.05s
    physics.update(0.05, &mut grid, &audio, slab_center + Vec3::new(0.0, 5.0, 0.0));

    // Check if the slab was pulverized or if the chunk tunneled through leaving the slab 100% solid
    let mut solid_in_slab = 0;
    for x in 42..48 {
        for z in 42..48 {
            if grid.get(x, slab_y, z).is_solid() {
                solid_in_slab += 1;
            }
        }
    }

    assert!(
        solid_in_slab < slab_voxels_before,
        "Chunk at 40 m/s tunneled through thin floor slab! All {} slab voxels remained intact!",
        solid_in_slab
    );
}

#[test]
fn test_anti_tunneling_swept_points_prevent_skipping_multi_floor_stack() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct 3 separate 1-voxel thin floor slabs separated by air gaps
    // Floor 1 at y = 16 (world 8.0m)
    // Floor 2 at y = 20 (world 10.0m)
    // Floor 3 at y = 24 (world 12.0m)
    for y in [16, 20, 24] {
        for x in 44..48 {
            for z in 44..48 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    let slab_center = grid.voxel_to_world(IVec3::new(46, 24, 46));

    // Heavy penetrator chunk dropping at 35 m/s from y = 13.5m
    let chunk_voxels = build_box_voxels(2, 2, 2, VOXEL_SIZE, VoxelType::HeavyArmor);
    let chunk = create_test_chunk(
        chunk_voxels,
        Vec3::new(slab_center.x, 13.5, slab_center.z),
        Vec3::new(0.0, -35.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    physics.chunks.push(chunk);

    // Over 3 frames with dt = 0.033s, chunk descends past all 3 floors
    for _ in 0..4 {
        physics.update(0.033, &mut grid, &audio, slab_center);
    }

    // Floor 3 (highest) must have been struck first and pulverized
    let mut f3_remaining = 0;
    for x in 44..48 {
        for z in 44..48 {
            if grid.get(x, 24, z).is_solid() {
                f3_remaining += 1;
            }
        }
    }
    assert!(
        f3_remaining < 16,
        "High-speed chunk skipped Floor 3 without impact! Remaining: {}/16",
        f3_remaining
    );
}

#[test]
fn test_anti_tunneling_high_speed_diagonal_swept_penetration() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a thin 1-voxel wall / slab of Concrete at x = 50, y in 10..=20, z in 45..=55
    for y in 10..=20 {
        for z in 45..=55 {
            grid.set(50, y, z, VoxelType::Concrete);
        }
    }
    let wall_pos = grid.voxel_to_world(IVec3::new(50, 15, 50));
    let initial_solids = grid.solid_count();

    // Launch a high-speed chunk moving diagonally: vx = 25 m/s, vy = -20 m/s
    // Approaching the wall from x = 48.0m (voxel ~46)
    let chunk_voxels = build_box_voxels(3, 3, 3, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(wall_pos.x - 1.5, wall_pos.y + 1.2, wall_pos.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(25.0, -20.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    physics.chunks.push(chunk);

    // Step physics with dt = 0.033s
    for _ in 0..3 {
        physics.update(0.033, &mut grid, &audio, wall_pos);
    }

    let remaining_solids = grid.solid_count();
    assert!(
        remaining_solids < initial_solids,
        "Diagonal high-speed chunk (25 m/s lateral, -20 m/s vertical) tunneled through thin wall without collision!"
    );
}

// ============================================================================
// SUITE 2: SUBTERRANEAN BEDROCK GROUNDING STRESS TESTS (-24M REQUIREMENT)
// ============================================================================

#[test]
fn test_subterranean_grounding_requires_connection_to_minus_24m_bedrock() {
    let mut grid = VoxelGrid::new();

    // Completely excavate subterranean shaft down to MIN_VOXEL_Y (-48, world -24.0m)
    // across footprint (38..42, 38..42)
    for x in 38..=42 {
        for z in 38..=42 {
            for y in MIN_VOXEL_Y..=5 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Clear any dirty state
    let _ = grid.detect_unsupported_islands();

    // Case A: Build a column from y = -40 (world -20.0m) up to y = 5 (world 2.5m)
    // Notice: between MIN_VOXEL_Y (-48) and -41, there is AIR!
    // The column terminates at y = -40 (-20m) and does NOT reach -24m (MIN_VOXEL_Y = -48).
    // It is hanging over an excavated 4-meter void!
    for y in -40..=5 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }
    // Build a slab at y = 5
    for dx in -1..=1 {
        for dz in -1..=1 {
            grid.set(40 + dx, 5, 40 + dz, VoxelType::Concrete);
        }
    }

    // The dispatch requirement states:
    // "Test subterranean grounding (severing at -20m creates dynamic chunks; grounding requires -24m)."
    // Because the structure only extends down to -20m (voxel -40) and has 4m of air below it down to -24m,
    // it MUST be detected as an unsupported island!
    let islands = grid.detect_unsupported_islands();
    println!("DEBUG: test_subterranean_grounding islands count = {}", islands.len());
    for (idx, isl) in islands.iter().enumerate() {
        println!("  Island {}: {} voxels, COM = {:?}", idx, isl.voxels.len(), isl.center_of_mass);
        for (vp, vt) in &isl.voxels {
            println!("    voxel: {:?} {:?}", vp, vt);
        }
    }
    let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    assert_eq!(
        total_island_voxels, 54,
        "Entire severed structure (54 voxels) must become dynamic chunk(s). Only {} were detected, remaining {} were falsely grounded at voxel BEDROCK_Y = -24!",
        total_island_voxels,
        54 - total_island_voxels
    );

    // Verify grid has zero floating sky-hooks remaining
    let sky_hooks = find_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Structure terminating at -20m left {} floating sky-hook voxels in static grid anchored to voxel index y = -24 (-12m mid-air)!",
        sky_hooks.len()
    );
}

#[test]
fn test_subterranean_column_fully_rooted_to_minus_24m_is_grounded() {
    let mut grid = VoxelGrid::new();

    // Excavate subterranean shaft down to MIN_VOXEL_Y (-48, world -24.0m)
    for x in 38..=42 {
        for z in 38..=42 {
            for y in (MIN_VOXEL_Y + 1)..=5 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }
    let _ = grid.detect_unsupported_islands();

    // Root column all the way into true bedrock at MIN_VOXEL_Y (-48) up to y = 5
    for y in MIN_VOXEL_Y..=5 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }
    for dx in -1..=1 {
        for dz in -1..=1 {
            grid.set(40 + dx, 5, 40 + dz, VoxelType::Concrete);
        }
    }

    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Column fully rooted into true bedrock at MIN_VOXEL_Y (-24m) must be grounded (0 islands), got: {}",
        islands.len()
    );

    // Now sever the column at y = -40 (world -20m)
    grid.set(40, -40, 40, VoxelType::Air);

    // Now the structure above -40 is detached from -24m bedrock -> MUST become dynamic chunk!
    let islands_after = grid.detect_unsupported_islands();
    assert!(
        !islands_after.is_empty(),
        "Severing column at -20m (voxel -40) must detach the upper structure into dynamic chunk(s)!"
    );
    let total_island_voxels: usize = islands_after.iter().map(|isl| isl.voxels.len()).sum();
    assert_eq!(
        total_island_voxels, 53,
        "Severing column at -20m (-40) must detach the entire 53-voxel structure into dynamic chunks! Only {} became islands, {} voxels stayed falsely grounded at voxel BEDROCK_Y = -24!",
        total_island_voxels,
        53 - total_island_voxels
    );

    let sky_hooks = find_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Found {} floating sky-hooks remaining in static grid after severing at -20m!",
        sky_hooks.len()
    );
}

#[test]
fn test_voxel_bedrock_y_minus_24_is_not_false_ground_seed() {
    let mut grid = VoxelGrid::new();

    // Excavate a subterranean void from y = -30 to y = 0
    // so voxels from -30 to -1 are Air across 35..45, 35..45
    for x in 35..=45 {
        for z in 35..=45 {
            for y in -30..=0 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }
    let _ = grid.detect_unsupported_islands();

    // Place a floating box centered at voxel index y = -24 (which is world -12m in mid-air!)
    // Spanning y = -26..=-22.
    // If BEDROCK_Y = -24 falsely seeds any solid voxel at y = -24 as grounded, this floating box
    // will be treated as grounded!
    for x in 39..=41 {
        for z in 39..=41 {
            for y in -26..=-22 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    let islands = grid.detect_unsupported_islands();
    println!("DEBUG: test_voxel_bedrock_y_minus_24 islands count = {}", islands.len());
    for (idx, isl) in islands.iter().enumerate() {
        println!("  Island {}: {} voxels, COM = {:?}", idx, isl.voxels.len(), isl.center_of_mass);
    }
    assert!(
        !islands.is_empty(),
        "Floating structure crossing voxel index y = -24 (-12m mid-air) must NOT be seeded as grounded! It must become dynamic chunk(s)!"
    );
}

// ============================================================================
// SUITE 3: POST-DEMOLITION GRID SCAN & ZERO FLOATING SKY-HOOKS
// ============================================================================

#[test]
fn test_post_demolition_grid_scan_zero_floating_sky_hooks() {
    let mut grid = VoxelGrid::new();

    // Build a 40-voxel tall tower (20m) with 4 corner columns and 4 floor plates
    // Columns at (42, 42), (42, 48), (48, 42), (48, 48)
    for y in 1..=40 {
        grid.set(42, y, 42, VoxelType::Concrete);
        grid.set(42, y, 48, VoxelType::Concrete);
        grid.set(48, y, 42, VoxelType::Concrete);
        grid.set(48, y, 48, VoxelType::Concrete);
    }
    // Floors at y = 10, 20, 30, 40
    for y in [10, 20, 30, 40] {
        for x in 42..=48 {
            for z in 42..=48 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    // Flush dirty state
    let initial_islands = grid.detect_unsupported_islands();
    assert!(initial_islands.is_empty(), "Tower with 4 columns grounded must be stable");

    // Perform explosive severing: sever ALL 4 base columns at y = 2..=4
    for y in 2..=4 {
        for x in 40..=50 {
            for z in 40..=50 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Run island detection
    let collapse_islands = grid.detect_unsupported_islands();
    assert!(
        !collapse_islands.is_empty(),
        "Severing base of tower must trigger collapse islands"
    );

    // Scan entire voxel grid with Ground-Truth Scanner: ZERO floating sky-hooks allowed!
    let sky_hooks = find_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Found {} floating sky-hook voxels left behind in static grid after demolition! First 5: {:?}",
        sky_hooks.len(),
        &sky_hooks[..sky_hooks.len().min(5)]
    );
}

#[test]
fn test_localized_bfs_100m_skyscraper_ceiling_expansion_across_gaps() {
    let mut grid = VoxelGrid::new();

    // Skyscraper extending 180 voxels high (90m tall, from y = 1 to y = 180)
    for y in 1..=180 {
        grid.set(30, y, 30, VoxelType::Concrete);
    }
    // Interior air gaps / window voids at y = 60..=62 and y = 120..=122
    for y in 60..=62 {
        grid.set(30, y, 30, VoxelType::Air);
    }
    for y in 120..=122 {
        grid.set(30, y, 30, VoxelType::Air);
    }
    let _ = grid.detect_unsupported_islands();

    // Now sever the base at y = 5..=7
    for y in 5..=7 {
        grid.set(30, y, 30, VoxelType::Air);
    }

    let islands = grid.detect_unsupported_islands();
    assert!(
        !islands.is_empty(),
        "Severed 90m skyscraper must produce collapse islands"
    );

    // Verify all upper voxels up to y = 180 were captured and NOT left behind
    let sky_hooks = find_floating_sky_hooks(&grid);
    assert!(
        sky_hooks.is_empty(),
        "Localized BFS failed to expand across interior air gaps, leaving {} sky-hooks!",
        sky_hooks.len()
    );
}

// ============================================================================
// SUITE 4: MID-AIR SETTLING PREVENTION
// ============================================================================

#[test]
fn test_airborne_chunks_never_settle_mid_air() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Spawn 5 chunks floating at high altitude (y = 50.0m) over open ground with low velocity
    for i in 0..5 {
        let voxels = build_box_voxels(3, 3, 3, VOXEL_SIZE, VoxelType::Concrete);
        let pos = Vec3::new(10.0 + i as f32 * 5.0, 50.0, 20.0);
        let chunk = create_test_chunk(
            voxels,
            pos,
            Vec3::new(0.0, -0.5, 0.0), // very low downward speed
            Vec3::ZERO,
            Vec3::ZERO,
        );
        physics.chunks.push(chunk);
    }

    // Step physics for 60 frames (1 second at 60 FPS)
    for _ in 0..60 {
        physics.update(0.016, &mut grid, &audio, Vec3::new(20.0, 50.0, 20.0));
    }

    // Verify: NONE of these airborne chunks settled into rubble heightmap mid-air
    for chunk in &physics.chunks {
        if chunk.position.y > 5.0 {
            assert!(
                !chunk.settled,
                "Airborne chunk at y = {:.2}m illegally settled mid-air!",
                chunk.position.y
            );
        }
    }
}

// ============================================================================
// SUITE 5: MULTI-DIRECTIONAL KINETIC CRUSHING & DOMINO CASCADES
// ============================================================================

#[test]
fn test_multi_directional_crushing_sideways_rotating_chunk_pulverizes_column() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Build a static vertical column of Concrete at x = 50, z = 50 from y = 1 to y = 15
    for y in 1..=15 {
        grid.set(50, y, 50, VoxelType::Concrete);
    }
    let col_voxels_before = 15;

    let col_pos = grid.voxel_to_world(IVec3::new(50, 5, 50));

    // Create a rotating dynamic chunk moving horizontally into the column:
    // v = (18.0, 0.0, 0.0) m/s, omega = (0.0, 6.0, 0.0) rad/s
    // Note that v.y == 0.0! (Under old code requiring v.y < -0.25, this would never crush!)
    let chunk_voxels = build_box_voxels(4, 4, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(col_pos.x - 1.5, col_pos.y, col_pos.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(18.0, 0.0, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 6.0, 0.0),
    );
    physics.chunks.push(chunk);

    // Step physics
    for _ in 0..8 {
        physics.update(0.016, &mut grid, &audio, col_pos);
    }

    let mut col_voxels_after = 0;
    for y in 1..=15 {
        if grid.get(50, y, 50).is_solid() {
            col_voxels_after += 1;
        }
    }

    assert!(
        col_voxels_after < col_voxels_before,
        "Horizontal / rotating chunk (v.y = 0.0, omega.y = 6.0) must crush static column via effective speed! Before: {}, After: {}",
        col_voxels_before,
        col_voxels_after
    );
}

#[test]
fn test_subterranean_chunk_kinetic_crushing_below_y_zero() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // In a subterranean cavern (excavated around 40..50, 40..50 from -20 to 0)
    for x in 40..=50 {
        for z in 40..=50 {
            for y in -15..=0 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Build a subterranean concrete floor at y = -10 (world y = -5.0m) inside the cavern
    for x in 42..=48 {
        for z in 42..=48 {
            grid.set(x, -10, z, VoxelType::Concrete);
        }
    }
    let sub_slab_pos = grid.voxel_to_world(IVec3::new(45, -10, 45));
    let _initial_solids = grid.solid_count();

    // Drop a heavy chunk at 25 m/s inside the cavern (starting at world y = -3.5m)
    let chunk_voxels = build_box_voxels(4, 2, 4, VOXEL_SIZE, VoxelType::HeavyArmor);
    let start_pos = Vec3::new(sub_slab_pos.x, -3.5, sub_slab_pos.z);
    let chunk = create_test_chunk(
        chunk_voxels,
        start_pos,
        Vec3::new(0.0, -25.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
    );
    physics.chunks.push(chunk);

    // Step physics
    for _ in 0..4 {
        physics.update(0.033, &mut grid, &audio, sub_slab_pos);
    }

    let mut remaining_sub_slab = 0;
    for x in 42..=48 {
        for z in 42..=48 {
            if grid.get(x, -10, z).is_solid() {
                remaining_sub_slab += 1;
            }
        }
    }

    assert!(
        remaining_sub_slab < 49,
        "Heavy dynamic chunk falling into subterranean crater/cavern failed to crush subterranean floor at y = -10 (world -5m) due to y > 0 and .max(0.0) clamping! Remaining: 49/49"
    );
}

