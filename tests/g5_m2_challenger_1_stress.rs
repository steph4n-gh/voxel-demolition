//! ============================================================================
//! EMPIRICAL ADVERSARIAL STRESS TEST SUITE: MILESTONE G5-M2
//! Physics, Collision & Weapons Subsystem Inner-Loop Optimization
//! File Target: tests/g5_m2_challenger_1_stress.rs
//! Authored by: g5_m2_challenger_1 (Empirical Challenger)
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{
    DebrisParticle, DynamicChunk, Mat3, PhysicsSystem,
};
use siege::{ColossalShell, SiegeEngineKind, SiegePayload, SiegeTrajectory, SiegeWeaponConfig};
use voxel::{
    CarveResult, DamageType, VoxelGrid, VoxelType, GRID_X, GRID_Z, MIN_VOXEL_Y, VOID_BOUNDARY_Y,
    VOXEL_SIZE,
};
use weapon::{ActiveProjectile, TrailBuffer, WeaponSystem, MAX_C4_CHARGES};

// ============================================================================
// PART 1: DYNAMIC CHUNK ROTATION MATRIX CACHING (`rot_mat`) EMPIRICAL STRESS
// ============================================================================

/// Helper to compute max component-wise difference between two Mat3 matrices.
fn mat3_max_diff(a: &Mat3, b: &Mat3) -> f32 {
    let mut max_diff = 0.0f32;
    for r in 0..3 {
        for c in 0..3 {
            let diff = (a.m[r][c] - b.m[r][c]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
        }
    }
    max_diff
}

#[test]
fn test_rot_mat_fast_spinning_continuous_simulation() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Spawn 10 dynamic chunks with extreme angular velocities (up to 120 rad/s)
    let angular_velocities = [
        Vec3::new(45.0, 0.0, 0.0),
        Vec3::new(0.0, 75.0, 0.0),
        Vec3::new(0.0, 0.0, 110.0),
        Vec3::new(35.0, -45.0, 60.0),
        Vec3::new(-80.0, 50.0, -90.0),
        Vec3::new(12.5, 33.3, -44.4),
        Vec3::new(-100.0, -100.0, 100.0),
        Vec3::new(0.5, 120.0, -0.5),
        Vec3::new(60.0, 60.0, 60.0),
        Vec3::new(-50.0, 85.0, -25.0),
    ];

    for (idx, &ang_vel) in angular_velocities.iter().enumerate() {
        let pos = Vec3::new((idx as f32 - 4.5) * 4.0, 80.0, 0.0);
        let voxels = vec![
            (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(VOXEL_SIZE, 0.0, 0.0), VoxelType::SteelRebar),
            (Vec3::new(0.0, VOXEL_SIZE, 0.0), VoxelType::HeavyArmor),
            (Vec3::new(VOXEL_SIZE, VOXEL_SIZE, 0.0), VoxelType::Glass),
        ];
        let aabb = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk::new(
            pos,
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(idx as f32 * 0.3, idx as f32 * 0.5, idx as f32 * 0.7),
            ang_vel,
            voxels,
            10.0,
            0.0,
            false,
            aabb,
        ));
    }

    // Step physics forward for 600 ticks (10 seconds at 60 FPS)
    // At EVERY tick, verify rot_mat exactly equals Mat3::from_euler(chunk.rotation)
    for tick in 0..600 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 80.0, 20.0));

        for (i, chunk) in phys.chunks.iter().enumerate() {
            let expected_mat = Mat3::from_euler(chunk.rotation);
            let diff = mat3_max_diff(&chunk.rot_mat, &expected_mat);
            assert!(
                diff < 1e-5,
                "Tick {}: Chunk {} rot_mat diverged from Mat3::from_euler(rotation)! Max diff = {}, rot = {:?}",
                tick, i, diff, chunk.rotation
            );

            // Also verify that lowest_world_y matches full matrix evaluation
            let lowest_cached = chunk.lowest_world_y();
            let mut lowest_expected = f32::INFINITY;
            for (p, _) in &chunk.voxels {
                let wy = chunk.position.y + expected_mat.transform_y(*p);
                if wy < lowest_expected {
                    lowest_expected = wy;
                }
            }
            lowest_expected -= VOXEL_SIZE * 0.5;
            assert!(
                (lowest_cached - lowest_expected).abs() < 1e-4,
                "Tick {}: Chunk {} lowest_world_y mismatch! cached = {}, expected = {}",
                tick, i, lowest_cached, lowest_expected
            );
        }
    }
}

#[test]
fn test_rot_mat_sleeping_resting_and_wake_transitions() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Fill a floor at bedrock (vy = 0)
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
        }
    }
    phys.rebuild_rubble_heightmap_with_grid(&grid);

    // Spawn a chunk just above the ground with low velocity so it settles quickly
    let voxels = vec![
        (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(VOXEL_SIZE, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(0.0, 0.0, VOXEL_SIZE), VoxelType::Concrete),
    ];
    let aabb = DynamicChunk::compute_aabb(&voxels);
    let initial_rot = Vec3::new(0.2, 0.4, 0.1);
    phys.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(0.0, -0.5, 0.0),
        initial_rot,
        Vec3::new(0.1, 0.2, 0.0),
        voxels,
        5.0,
        0.0,
        false,
        aabb,
    ));

    // Simulate until settled
    let mut settled = false;
    for _ in 0..200 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));
        if !phys.chunks.is_empty() && phys.chunks[0].settled {
            settled = true;
            break;
        }
    }
    assert!(settled, "Chunk should have settled onto the rubble heightfield!");

    let chunk = &phys.chunks[0];
    let settled_rot = chunk.rotation;
    let settled_mat = chunk.rot_mat;
    let expected_settled_mat = Mat3::from_euler(settled_rot);
    let diff = mat3_max_diff(&settled_mat, &expected_settled_mat);
    assert!(
        diff < 1e-5,
        "Settled chunk rot_mat does not match Mat3::from_euler! diff = {}",
        diff
    );

    // Run 50 more frames while sleeping: orientation and rot_mat must remain completely static
    for _ in 0..50 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));
        let chunk = &phys.chunks[0];
        assert!(chunk.settled, "Chunk must remain settled!");
        assert_eq!(chunk.rotation, settled_rot, "Sleeping chunk rotation must not drift!");
        assert_eq!(chunk.rot_mat.m, settled_mat.m, "Sleeping chunk rot_mat must remain frozen!");
    }

    // Now detonate an explosive blast nearby to wake it up!
    phys.un_settle_rubble_crater(Vec3::new(1.0, 0.5, 1.0), 4.0, 15.0);

    let chunk_after_blast = &phys.chunks[0];
    assert!(
        !chunk_after_blast.settled,
        "Chunk must have been woken up by un_settle_rubble_crater!"
    );
    assert!(
        chunk_after_blast.angular_velocity.length_squared() > 1.0,
        "Woken chunk must receive angular impulse!"
    );

    // Simulate post-wakeup flight for 60 ticks: verify rot_mat dynamically tracks new rotation
    for tick in 0..60 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));
        if phys.chunks.is_empty() {
            break;
        }
        let c = &phys.chunks[0];
        let exp = Mat3::from_euler(c.rotation);
        let d = mat3_max_diff(&c.rot_mat, &exp);
        assert!(
            d < 1e-5,
            "Post-wake tick {}: rot_mat diverged after wake-up! diff = {}",
            tick, d
        );
    }
}

#[test]
fn test_rot_mat_high_speed_fracture_sub_chunks_invariants() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Bedrock floor
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
        }
    }
    phys.rebuild_rubble_heightmap_with_grid(&grid);

    // Create a large fragile glass chunk with 80 voxels
    let mut voxels = Vec::new();
    for x in 0..4 {
        for y in 0..5 {
            for z in 0..4 {
                voxels.push((
                    Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                    VoxelType::Glass,
                ));
            }
        }
    }
    let aabb = DynamicChunk::compute_aabb(&voxels);
    let initial_rot = Vec3::new(0.78, 0.52, -0.33);

    // High downward velocity (-28.0 m/s) directly towards bedrock to trigger fracture
    phys.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::new(0.0, -28.0, 0.0),
        initial_rot,
        Vec3::new(2.0, 1.0, 3.0),
        voxels,
        80.0,
        0.0,
        false,
        aabb,
    ));

    // Update 1 tick: impact occurs and fractures into sub-chunks
    phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));

    // Sub-chunks should have been spawned from fracture
    assert!(
        phys.chunks.len() > 1,
        "High-speed impact should have fractured the chunk into multiple sub-chunks!"
    );

    // Verify all sub-chunks have valid rot_mat matching Mat3::from_euler(chunk.rotation)
    for (i, sub) in phys.chunks.iter().enumerate() {
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    sub.rot_mat.m[r][c].is_finite(),
                    "Sub-chunk {} rot_mat[{}][{}] is not finite!",
                    i, r, c
                );
            }
        }
        let exp = Mat3::from_euler(sub.rotation);
        let diff = mat3_max_diff(&sub.rot_mat, &exp);
        assert!(
            diff < 1e-5,
            "Sub-chunk {} rot_mat diverged from initial rotation! diff = {}",
            i, diff
        );
    }
}

#[test]
fn test_rot_mat_aabb_and_lowest_y_mathematical_invariance() {
    // Generate 500 random orientations and test complex chunk topologies
    let mut rng_seed = 987654321u64;
    let mut pseudo_rand = || -> f32 {
        rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((rng_seed >> 33) as f32) / (u32::MAX as f32 * 0.5) - 1.0
    };

    // Construct an asymmetrical L-shaped chunk
    let mut voxels = Vec::new();
    for y in 0..8 {
        voxels.push((Vec3::new(0.0, y as f32 * VOXEL_SIZE, 0.0), VoxelType::Concrete));
    }
    for x in 1..6 {
        voxels.push((Vec3::new(x as f32 * VOXEL_SIZE, 0.0, 0.0), VoxelType::SteelRebar));
    }
    for z in 1..4 {
        voxels.push((Vec3::new(0.0, 0.0, z as f32 * VOXEL_SIZE), VoxelType::HeavyArmor));
    }
    let local_aabb = DynamicChunk::compute_aabb(&voxels);

    for iter in 0..500 {
        let rot = Vec3::new(
            pseudo_rand() * std::f32::consts::PI,
            pseudo_rand() * std::f32::consts::PI,
            pseudo_rand() * std::f32::consts::PI,
        );
        let pos = Vec3::new(pseudo_rand() * 20.0, pseudo_rand() * 20.0, pseudo_rand() * 20.0);

        let chunk = DynamicChunk::new(
            pos,
            Vec3::ZERO,
            rot,
            Vec3::ZERO,
            voxels.clone(),
            20.0,
            0.0,
            false,
            local_aabb,
        );

        let expected_mat = Mat3::from_euler(rot);

        // 1. Check rot_mat vs from_euler
        let d = mat3_max_diff(&chunk.rot_mat, &expected_mat);
        assert!(d < 1e-6, "Iteration {}: rot_mat mismatch, diff = {}", iter, d);

        // 2. Check world_aabb() using cached rot_mat vs using expected_mat
        let (cached_min, cached_max) = chunk.world_aabb();
        let (direct_min, direct_max) = chunk.world_aabb_from_mat(&expected_mat);
        assert!(
            (cached_min - direct_min).length() < 1e-5,
            "Iteration {}: cached AABB min mismatch! diff = {:?}",
            iter, cached_min - direct_min
        );
        assert!(
            (cached_max - direct_max).length() < 1e-5,
            "Iteration {}: cached AABB max mismatch! diff = {:?}",
            iter, cached_max - direct_max
        );

        // 3. Verify ALL voxels transformed into world space are strictly inside world_aabb()
        for (local_v, _) in &chunk.voxels {
            let wp = chunk.local_to_world(*local_v);
            let margin = 1e-4;
            assert!(
                wp.x >= cached_min.x - margin && wp.x <= cached_max.x + margin &&
                wp.y >= cached_min.y - margin && wp.y <= cached_max.y + margin &&
                wp.z >= cached_min.z - margin && wp.z <= cached_max.z + margin,
                "Iteration {}: Transformed voxel {:?} escaped world AABB [{:?}, {:?}]",
                iter, wp, cached_min, cached_max
            );
        }

        // 4. Verify lowest_world_y() bounds
        let lowest_y = chunk.lowest_world_y();
        let mut min_voxel_bottom = f32::INFINITY;
        for (local_v, _) in &chunk.voxels {
            let wp = chunk.local_to_world(*local_v);
            let bottom = wp.y - VOXEL_SIZE * 0.5;
            if bottom < min_voxel_bottom {
                min_voxel_bottom = bottom;
            }
        }
        assert!(
            (lowest_y - min_voxel_bottom).abs() < 1e-4,
            "Iteration {}: lowest_world_y ({}) does not match min voxel bottom ({})",
            iter, lowest_y, min_voxel_bottom
        );
    }
}

// ============================================================================
// PART 2: LOCALIZED RUBBLE HEIGHTFIELD REBUILD & CRATERING EMPIRICAL STRESS
// ============================================================================

#[test]
fn test_rubble_localized_vs_full_rebuild_at_all_coordinates() {
    let mut grid = VoxelGrid::new();

    // Fill bedrock at ground level and subterranean layers
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
            grid.set(x, -5, z, VoxelType::Basalt);
            grid.set(x, -15, z, VoxelType::Basalt);
            grid.set(x, MIN_VOXEL_Y, z, VoxelType::Bedrock);
        }
    }

    // Comprehensive test points within the 96x96 world bounds (half_x = 24.0, half_z = 24.0)
    let test_crater_specs = [
        // (Center Vec3, radius)
        (Vec3::new(0.0, 0.0, 0.0), 4.0),               // Center
        (Vec3::new(18.0, 0.0, 0.0), 3.5),              // +X Boundary
        (Vec3::new(-18.0, 0.0, 0.0), 3.5),             // -X Boundary
        (Vec3::new(0.0, 0.0, 18.0), 3.5),              // +Z Boundary
        (Vec3::new(0.0, 0.0, -18.0), 3.5),             // -Z Boundary
        (Vec3::new(18.0, 0.0, 18.0), 3.0),             // (+X, +Z) Corner
        (Vec3::new(-18.0, 0.0, 18.0), 3.0),            // (-X, +Z) Corner
        (Vec3::new(18.0, 0.0, -18.0), 3.0),            // (+X, -Z) Corner
        (Vec3::new(-18.0, 0.0, -18.0), 3.0),           // (-X, -Z) Corner
        (Vec3::new(0.0, -4.0, 0.0), 6.0),              // Shallow Subterranean
        (Vec3::new(5.0, -10.0, -5.0), 8.0),            // Deep Subterranean (-18m)
        (Vec3::new(-8.0, -15.0, 8.0), 8.0),            // Near Void Boundary (-23m)
    ];

    for (case_idx, &(center, radius)) in test_crater_specs.iter().enumerate() {
        // Carve sphere in grid to simulate actual blast crater
        let mut scratch = CarveResult::with_capacity(64);
        grid.carve_sphere_advanced_into(
            center,
            radius,
            100.0,
            DamageType::Explosive,
            &mut scratch,
        );

        let mut phys_localized = PhysicsSystem::new();
        let mut phys_full = PhysicsSystem::new();

        // Add a settled chunk nearby to verify settled chunk integration
        let chunk_voxels = vec![
            (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(VOXEL_SIZE, 0.0, 0.0), VoxelType::Concrete),
        ];
        let chunk_aabb = DynamicChunk::compute_aabb(&chunk_voxels);
        let chunk_pos = center + Vec3::new(radius * 0.3, 1.0, radius * 0.3);
        phys_localized.chunks.push(DynamicChunk::new(
            chunk_pos,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            chunk_voxels.clone(),
            10.0,
            0.0,
            true, // Settled
            chunk_aabb,
        ));
        phys_full.chunks.push(DynamicChunk::new(
            chunk_pos,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            chunk_voxels,
            10.0,
            0.0,
            true, // Settled
            chunk_aabb,
        ));

        // 1. Run full rebuild on phys_full
        phys_full.rebuild_rubble_heightmap_with_grid(&grid);

        // 2. Run localized rebuild on phys_localized
        let pad = Vec3::splat(radius + 2.0);
        let min_pos = PhysicsSystem::world_pos_to_voxel(center - pad);
        let max_pos = PhysicsSystem::world_pos_to_voxel(center + pad);

        // Initialize phys_localized with full rebuild first to simulate ongoing game state
        phys_localized.rebuild_rubble_heightmap_with_grid(&grid);
        // Then execute the localized rebuild over the dirty region
        phys_localized.rebuild_rubble_heightmap_in_region(&grid, min_pos, max_pos);

        // 3. Verify ALL 16,384 columns of the heightfield match exactly between localized and full rebuild
        let mut mismatch_count = 0;
        let mut max_diff = 0.0f32;
        for gx in 0..GRID_X {
            for gz in 0..GRID_Z {
                let idx = gx * GRID_Z + gz;
                let h_loc = phys_localized.rubble_height_grid[idx];
                let h_full = phys_full.rubble_height_grid[idx];
                let diff = (h_loc - h_full).abs();
                if diff > 1e-4 {
                    mismatch_count += 1;
                    if diff > max_diff {
                        max_diff = diff;
                    }
                }
            }
        }

        assert_eq!(
            mismatch_count, 0,
            "Case {}: Localized rebuild at center {:?}, radius {} differed from full rebuild in {} columns! Max diff = {}",
            case_idx, center, radius, mismatch_count, max_diff
        );

        if (phys_localized.max_rubble_height - phys_full.max_rubble_height).abs() >= 1e-4 {
            let (c_min, c_max) = phys_localized.chunks[0].world_aabb();
            println!("DEBUG Case {}: center={:?}, radius={}, min_pos={:?}, max_pos={:?}, chunk_pos={:?}, chunk_aabb=({:?}, {:?}), loc_max_h={}, full_max_h={}",
                case_idx, center, radius, min_pos, max_pos, chunk_pos, c_min, c_max, phys_localized.max_rubble_height, phys_full.max_rubble_height);
        }
        assert!(
            (phys_localized.max_rubble_height - phys_full.max_rubble_height).abs() < 1e-4,
            "Case {}: max_rubble_height mismatch: localized = {}, full = {}",
            case_idx, phys_localized.max_rubble_height, phys_full.max_rubble_height
        );
    }
}

#[test]
fn test_subterranean_crater_excavation_and_rubble_settling_no_clipping() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Create solid terrain from bedrock down to MIN_VOXEL_Y (-24.0m)
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            for vy in MIN_VOXEL_Y..=0 {
                grid.set(x, vy, z, VoxelType::Bedrock);
            }
        }
    }
    phys.rebuild_rubble_heightmap_with_grid(&grid);

    // Excavate a deep subterranean crater down to -14m penetrating through surface bedrock
    let crater_center = Vec3::new(0.0, -4.0, 0.0);
    let crater_radius = 10.0;
    let mut scratch = CarveResult::with_capacity(64);
    grid.carve_sphere_advanced_into(
        crater_center,
        crater_radius,
        36000.0,
        DamageType::Kinetic,
        &mut scratch,
    );

    // Update crater floor and localized rubble heightfield
    phys.update_crater_floor(&grid, crater_center, crater_radius);
    phys.un_settle_rubble_crater(crater_center, crater_radius, 10.0);

    // Step physics to process dirty rubble region
    phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));

    // Verify crater floor height in the center: must be excavated below 0.0m and >= VOID_BOUNDARY_Y (-24.0m)
    let center_h = PhysicsSystem::rubble_height_from_slice(&phys.rubble_height_grid, 0.0, 0.0);
    assert!(
        center_h < 0.0,
        "Crater floor at center must be excavated below surface! Got center_h = {}",
        center_h
    );
    assert!(
        center_h >= VOID_BOUNDARY_Y,
        "Crater floor must remain at or above VOID_BOUNDARY_Y (-24.0m)! Got center_h = {}",
        center_h
    );

    // Spawn a dynamic chunk above the subterranean crater and drop it inside
    let chunk_voxels = vec![
        (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(VOXEL_SIZE, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(0.0, VOXEL_SIZE, 0.0), VoxelType::Concrete),
    ];
    let chunk_aabb = DynamicChunk::compute_aabb(&chunk_voxels);
    phys.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, center_h + 3.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::ZERO,
        Vec3::new(0.1, 0.1, 0.1),
        chunk_voxels,
        15.0,
        0.0,
        false,
        chunk_aabb,
    ));

    // Simulate chunk settling inside subterranean crater
    let mut chunk_settled = false;
    for _ in 0..300 {
        phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 0.0, 10.0));
        if !phys.chunks.is_empty() && phys.chunks[0].settled {
            chunk_settled = true;
            break;
        }
    }

    assert!(
        chunk_settled,
        "Dynamic chunk dropped into subterranean crater failed to settle!"
    );

    let settled_chunk = &phys.chunks[0];
    let lowest_y = settled_chunk.lowest_world_y();

    // Verify chunk rests cleanly ON the crater floor without clipping below it
    assert!(
        lowest_y >= center_h - 0.15,
        "Chunk clipped through subterranean crater floor! lowest_y = {}, floor_h = {}",
        lowest_y, center_h
    );
    assert!(
        lowest_y <= center_h + VOXEL_SIZE * 2.0,
        "Chunk floating above subterranean crater floor! lowest_y = {}, floor_h = {}",
        lowest_y, center_h
    );

    // Verify that the rubble heightfield column at chunk position reflects the settled chunk
    let updated_h = PhysicsSystem::rubble_height_from_slice(&phys.rubble_height_grid, 0.0, 0.0);
    assert!(
        updated_h >= center_h,
        "Settled chunk did not elevate rubble heightfield! updated_h = {}, previous_floor = {}",
        updated_h, center_h
    );
}

#[test]
fn test_unsettle_woken_chunks_heightfield_integrity() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Fill bedrock floor at vy = 0
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
        }
    }
    phys.rebuild_rubble_heightmap_with_grid(&grid);

    let bedrock_floor = PhysicsSystem::rubble_height_from_slice(&phys.rubble_height_grid, 6.0, 0.0);

    // Settle a chunk at x = 6.0, z = 0.0 with 20 voxels
    let mut voxels = Vec::new();
    for y in 0..4 {
        for x in 0..5 {
            voxels.push((
                Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, 0.0),
                VoxelType::Concrete,
            ));
        }
    }
    let aabb = DynamicChunk::compute_aabb(&voxels);
    let chunk_pos = Vec3::new(6.0, bedrock_floor, 0.0);
    phys.chunks.push(DynamicChunk::new(
        chunk_pos,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        voxels,
        30.0,
        0.0,
        false,
        aabb,
    ));

    // Manually settle chunk 0
    phys.settle_chunk(0);
    assert!(phys.chunks[0].settled);

    let settled_height = PhysicsSystem::rubble_height_from_slice(&phys.rubble_height_grid, 6.5, 0.0);
    assert!(
        settled_height > bedrock_floor,
        "Settled chunk must increase heightfield! settled_height = {}, bedrock = {}",
        settled_height, bedrock_floor
    );

    // Detonate crater at (0.0, 0.0, 0.0) with radius 4.0 that wakes up the chunk
    phys.un_settle_rubble_crater(Vec3::new(0.0, 0.0, 0.0), 4.0, 20.0);

    assert!(
        !phys.chunks[0].settled,
        "Chunk at distance 6.0 must be woken up by 4.0m radius blast!"
    );

    // Launch the woken chunk high into the air
    phys.chunks[0].position.y = 50.0;
    phys.chunks[0].velocity = Vec3::new(0.0, 20.0, 0.0);

    // Update physics: heightmap sync runs
    phys.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 10.0));

    // Now check: has the vacated column at x = 6.5, z = 0.0 correctly restored to the bedrock floor?
    let post_wake_height = PhysicsSystem::rubble_height_from_slice(&phys.rubble_height_grid, 6.5, 0.0);
    assert!(
        (post_wake_height - bedrock_floor).abs() < 1e-3,
        "Vacated column did not reset to bedrock floor after chunk was woken! post_wake_height = {}, bedrock_floor = {}",
        post_wake_height, bedrock_floor
    );
}

// ============================================================================
// PART 3: TRAIL BUFFER, C4 DET-CORD & SCRATCH POOL EMPIRICAL STRESS
// ============================================================================

#[test]
fn test_trail_buffer_ring_buffer_fifo_invariants() {
    let mut trail: TrailBuffer<16> = TrailBuffer::new();
    assert!(trail.is_empty());
    assert_eq!(trail.len(), 0);

    // Push 10 points
    for i in 0..10 {
        trail.push(Vec3::new(i as f32, 0.0, 0.0));
    }
    assert_eq!(trail.len(), 10);
    assert!(!trail.is_empty());

    // Verify order via get() and Index
    for i in 0..10 {
        assert_eq!(trail.get(i).unwrap().x, i as f32);
        assert_eq!(trail[i].x, i as f32);
    }

    // Now push 20 more points to force ring-buffer wrapping (total 30 pushes)
    for i in 10..30 {
        trail.push(Vec3::new(i as f32, 0.0, 0.0));
    }
    assert_eq!(trail.len(), 16, "TrailBuffer capacity must be strictly capped at 16!");

    // The 16 elements must be points 14 through 29 in strict FIFO order
    for idx in 0..16 {
        let expected_x = (14 + idx) as f32;
        assert_eq!(
            trail[idx].x, expected_x,
            "Ring buffer FIFO order violated at index {}! got {}, expected {}",
            idx, trail[idx].x, expected_x
        );
        assert_eq!(trail.get(idx).unwrap().x, expected_x);
    }
    assert_eq!(trail.get(16), None, "Index >= count must return None");

    // Test ColossalShell TrailBuffer<18>
    let mut shell_trail: siege::TrailBuffer<18> = siege::TrailBuffer::single(Vec3::new(100.0, 200.0, 300.0));
    assert_eq!(shell_trail.len(), 1);
    assert_eq!(shell_trail.get(0).unwrap(), Vec3::new(100.0, 200.0, 300.0));

    for i in 0..50 {
        shell_trail.push(Vec3::new(i as f32, 0.0, 0.0));
    }
    assert_eq!(shell_trail.len(), 18);
    for idx in 0..18 {
        let expected_x = (32 + idx) as f32;
        assert_eq!(shell_trail.get(idx).unwrap().x, expected_x);
    }
}

#[test]
fn test_c4_det_cord_topology_caching_and_invalidation() {
    let mut weapons = WeaponSystem::new();
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    assert!(weapons.cached_det_cords.is_empty());
    assert!(!weapons.det_cords_dirty);

    // Place 8 C4 charges on columns spaced 3.0m apart
    for i in 0..8 {
        let pos = Vec3::new(i as f32 * 3.0, 2.0, 0.0);
        let normal = Vec3::Y;
        weapons.place_c4_charge(pos, normal, None);
    }

    assert_eq!(weapons.c4_charges.len(), 8);
    // place_c4_charge updates cache immediately and resets dirty flag
    assert!(!weapons.det_cords_dirty);
    assert!(
        !weapons.cached_det_cords.is_empty(),
        "Cached det cords must be populated when charges are within range!"
    );

    let cord_count = weapons.cached_det_cords.len();
    let first_cord_p1 = weapons.cached_det_cords[0].p1;

    // Simulate 10 frames with no new charges: cached_det_cords must NOT change or reallocate
    for _ in 0..10 {
        weapons.update(0.016, &mut grid, &mut phys, &audio, Vec3::ZERO);
        assert!(!weapons.det_cords_dirty);
        assert_eq!(weapons.cached_det_cords.len(), cord_count);
        assert_eq!(weapons.cached_det_cords[0].p1, first_cord_p1);
    }

    // Now place up to MAX_C4_CHARGES (24)
    for i in 8..MAX_C4_CHARGES {
        let pos = Vec3::new((i % 6) as f32 * 3.0, 2.0, (i / 6) as f32 * 3.0);
        weapons.place_c4_charge(pos, Vec3::Y, None);
    }
    assert_eq!(weapons.c4_charges.len(), MAX_C4_CHARGES);
    assert!(!weapons.det_cords_dirty);
    assert!(weapons.cached_det_cords.len() > cord_count);

    // Detonate all C4: charges and det cords must clear completely
    weapons.detonate_all_c4(&mut grid, &mut phys, &audio, Vec3::ZERO);
    assert_eq!(weapons.c4_charges.len(), 0);
    assert_eq!(weapons.cached_det_cords.len(), 0);
    assert!(!weapons.det_cords_dirty);
}

#[test]
fn test_c4_det_cords_dirty_flag_invalidation() {
    let mut weapons = WeaponSystem::new();
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Place 2 C4 charges
    weapons.place_c4_charge(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, None);
    weapons.place_c4_charge(Vec3::new(3.0, 1.0, 0.0), Vec3::Y, None);

    // Manually mark dirty to simulate external invalidation or repositioning
    weapons.det_cords_dirty = true;
    weapons.update(0.016, &mut grid, &mut phys, &audio, Vec3::ZERO);
    assert!(
        !weapons.det_cords_dirty,
        "WeaponSystem::update must clear det_cords_dirty when true and refresh cache"
    );
}

#[test]
fn test_siege_weapon_config_copy_derivation() {
    let cfg = SiegeWeaponConfig::default();
    // Verify Copy semantics: assignment does not move or invalidate original
    let cfg_copy = cfg;
    assert_eq!(cfg.caliber_mm, cfg_copy.caliber_mm);
    assert_eq!(cfg.blast_radius(), cfg_copy.blast_radius());
    assert_eq!(cfg.payload, cfg_copy.payload);
    assert_eq!(cfg.engine_kind, cfg_copy.engine_kind);

    let mut orbital = cfg;
    orbital.engine_kind = SiegeEngineKind::OrbitalKineticRelay;
    let mut configs = [cfg, orbital];
    // Copy array in place
    let configs_clone = configs;
    configs[0].engine_kind = SiegeEngineKind::HydraulicGroundRam;
    assert_eq!(configs_clone[0].engine_kind, SiegeEngineKind::RailArtillery);
    assert_eq!(configs[0].engine_kind, SiegeEngineKind::HydraulicGroundRam);
}

#[test]
fn test_scratch_sub_blocks_pool_stability() {
    let mut phys = PhysicsSystem::new();

    // Create a 100-voxel dynamic chunk
    let mut voxels = Vec::new();
    for x in 0..5 {
        for y in 0..5 {
            for z in 0..4 {
                voxels.push((
                    Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                    VoxelType::Concrete,
                ));
            }
        }
    }
    let aabb = DynamicChunk::compute_aabb(&voxels);
    let chunk = DynamicChunk::new(
        Vec3::new(0.0, 10.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        voxels,
        50.0,
        0.0,
        false,
        aabb,
    );

    // Call populate_sub_blocks on phys.scratch_sub_blocks
    chunk.populate_sub_blocks(&mut phys.scratch_sub_blocks);
    assert!(!phys.scratch_sub_blocks.is_empty());

    let initial_outer_cap = phys.scratch_sub_blocks.capacity();
    let initial_inner_caps: Vec<usize> = phys.scratch_sub_blocks.iter().map(|(_, b)| b.capacity()).collect();

    // Call populate_sub_blocks 50 more times: inner capacities must NOT drop or reallocate
    for _ in 0..50 {
        chunk.populate_sub_blocks(&mut phys.scratch_sub_blocks);
        assert_eq!(
            phys.scratch_sub_blocks.capacity(),
            initial_outer_cap,
            "Outer scratch vector capacity shrank!"
        );
        for (idx, (_, bucket)) in phys.scratch_sub_blocks.iter().enumerate() {
            assert!(
                bucket.capacity() >= initial_inner_caps[idx],
                "Inner bucket {} capacity shrank from {} to {}!",
                idx, initial_inner_caps[idx], bucket.capacity()
            );
        }
    }
}
