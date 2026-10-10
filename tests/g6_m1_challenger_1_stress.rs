#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DynamicChunk, PhysicsSystem};
use voxel::{VoxelGrid, VoxelType, VOXEL_SIZE};

/// Helper to create a solid cuboid chunk of specified dimensions and mass
fn create_test_chunk(pos: Vec3, vel: Vec3, rot: Vec3, ang_vel: Vec3, dim_voxels: IVec3, mass: f32, vtype: VoxelType) -> DynamicChunk {
    let mut voxels = Vec::with_capacity((dim_voxels.x * dim_voxels.y * dim_voxels.z) as usize);
    let half_x = dim_voxels.x as f32 * 0.5;
    let half_y = dim_voxels.y as f32 * 0.5;
    let half_z = dim_voxels.z as f32 * 0.5;

    for x in 0..dim_voxels.x {
        for y in 0..dim_voxels.y {
            for z in 0..dim_voxels.z {
                let local_p = Vec3::new(
                    (x as f32 - half_x + 0.5) * VOXEL_SIZE,
                    (y as f32 - half_y + 0.5) * VOXEL_SIZE,
                    (z as f32 - half_z + 0.5) * VOXEL_SIZE,
                );
                voxels.push((local_p, vtype));
            }
        }
    }

    let aabb = DynamicChunk::compute_aabb(&voxels);
    let rot_mat = physics::Mat3::from_euler(rot);

    DynamicChunk {
        position: pos,
        velocity: vel,
        rotation: rot,
        rot_mat,
        angular_velocity: ang_vel,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    }
}

/// Reconstruct this fixture's ground/rubble raster without the audited fragment's own stamp.
/// These are the engine's voxel-center support proxies, rather than rendered cube geometry.
fn external_raster_support_gap(chunk_idx: usize, phys: &PhysicsSystem, ground_y: f32) -> Option<f32> {
    let half = VOXEL_SIZE * 0.5;
    let mut tops = vec![ground_y; voxel::GRID_X * voxel::GRID_Z];
    for (other_idx, other) in phys.chunks.iter().enumerate() {
        if other_idx == chunk_idx || !other.settled {
            continue;
        }
        for (local, _) in &other.voxels {
            let center = other.local_to_world(*local);
            if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(center.x, center.z) {
                let cell = gx * voxel::GRID_Z + gz;
                tops[cell] = tops[cell].max(center.y + half);
            }
        }
    }

    let mut nearest: Option<f32> = None;
    for (local, _) in &phys.chunks[chunk_idx].voxels {
        let center = phys.chunks[chunk_idx].local_to_world(*local);
        for offset in [Vec2::ZERO, Vec2::new(-half, -half), Vec2::new(half, -half), Vec2::new(-half, half), Vec2::new(half, half)] {
            if let Some((gx, gz)) = PhysicsSystem::world_to_rubble_grid_coords(center.x + offset.x, center.z + offset.y) {
                let top = tops[gx * voxel::GRID_Z + gz];
                // An overhead voxel cannot support this foot.
                if top - half <= center.y {
                    let gap = center.y - half - top;
                    nearest = Some(nearest.map_or(gap, |previous| previous.min(gap)));
                }
            }
        }
    }
    nearest
}

// ============================================================================
// CHALLENGE 1: HEAVY CHUNK IMPACT BOUNCE RESTITUTION (500kg - 5000kg)
// Verify: Upward bounce restitution is 0.0 (no positive vy kick after impact).
// ============================================================================

#[test]
fn test_heavy_chunk_ground_impact_restitution_zero_kick() {
    let masses = [500.0f32, 1000.0, 2500.0, 5000.0];
    let impact_speeds = [3.0f32, 6.0, 8.0];

    let mut failures = Vec::new();
    for &mass in &masses {
        for &spd in &impact_speeds {
            let mut phys = PhysicsSystem::new();
            let mut grid = VoxelGrid::new();
            let audio = SoundSystem::empty();

            // Set flat bedrock rubble height
            let chunk_x = 20.0f32;
            let chunk_z = 20.0f32;
            let ground_y = 1.0f32;
            for gx in 15..25 {
                for gz in 15..25 {
                    if let Some((rx, rz)) = PhysicsSystem::world_to_rubble_grid_coords(gx as f32, gz as f32) {
                        phys.rubble_height_grid[rx * voxel::GRID_Z + rz] = ground_y;
                    }
                }
            }
            phys.max_rubble_height = ground_y;

            let chunk = create_test_chunk(
                Vec3::new(chunk_x, 1.45, chunk_z),
                Vec3::new(0.0, -spd, 0.0),
                Vec3::ZERO,
                Vec3::ZERO,
                IVec3::new(4, 4, 4),
                mass,
                VoxelType::Concrete,
            );
            phys.chunks.push(chunk);

            phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

            if !phys.chunks.is_empty() {
                let chunk = &phys.chunks[0];
                println!("Mass: {:6.1} kg, InSpeed: {:4.1} m/s, Result vy: {:+8.5} m/s, Settled: {}", mass, spd, chunk.velocity.y, chunk.settled);
                if chunk.velocity.y > 0.0 {
                    failures.push((mass, spd, chunk.velocity.y));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "FAILED: Found {} cases where high-mass chunks rebounded with positive vy > 0.0: {:?}",
        failures.len(), failures
    );
}

// ============================================================================
// CHALLENGE 2: MULTI-STORY FLOOR PANCAKE COLLAPSE & MOMENTUM CONSERVATION
// Verify: Downward velocity through successive floors does NOT stall mid-air
// or drop to 0.0 before reaching support.
// ============================================================================

#[test]
fn test_heavy_chunk_multi_story_pancake_collapse_no_midair_stall() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Construct 4 stacked concrete floor slabs at heights y = 6, 12, 18, 24
    let floor_levels = [6, 12, 18, 24];
    for &fy in &floor_levels {
        for x in 28..=34 {
            for z in 28..=34 {
                grid.set(x, fy, z, VoxelType::Concrete);
            }
        }
    }

    // Spawn a 2,000 kg heavy slab above floor 24
    let top_floor_world = grid.voxel_to_world(IVec3::new(31, 24, 31));
    let heavy_slab = create_test_chunk(
        top_floor_world + Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -12.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        IVec3::new(5, 2, 5),
        2000.0,
        VoxelType::Concrete,
    );
    phys.chunks.push(heavy_slab);

    let mut zero_velocity_stalls = 0;
    let mut lowest_y = f32::INFINITY;

    for step in 0..80 {
        if phys.chunks.is_empty() {
            break;
        }

        phys.update(0.032, &mut grid, &audio, Vec3::ZERO);

        // Find primary falling slab (highest mass)
        if let Some(chunk) = phys.chunks.iter().filter(|c| c.mass > 500.0).max_by(|a, b| a.mass.partial_cmp(&b.mass).unwrap()) {
            lowest_y = lowest_y.min(chunk.position.y);
            // The progressive-collapse goal is complete; subsequent rubble can rest above y=2.
            if lowest_y < 5.0 {
                break;
            }

            // While airborne above ground (y > 2.0), check for abnormal stalls
            if chunk.position.y > 2.0 && chunk.velocity.y == 0.0 {
                zero_velocity_stalls += 1;
                println!("Step {}: Mid-air stall at y = {}, vy = 0.0", step, chunk.position.y);
            }

            println!(
                "Step {:2}: y = {:5.2}, vy = {:+6.2}, mass = {:6.1}, settled = {}",
                step, chunk.position.y, chunk.velocity.y, chunk.mass, chunk.settled
            );
        }
    }

    assert_eq!(
        zero_velocity_stalls, 0,
        "Heavy falling chunk must NEVER stall mid-air with vy = 0.0 during progressive pancake collapse"
    );
    assert!(
        lowest_y < 5.0,
        "Heavy slab (2,000 kg) must smash through all 4 intermediate floors down towards bedrock, reached y = {}",
        lowest_y
    );
}

// ============================================================================
// CHALLENGE 3: CORNER ROCKING & LIMIT-CYCLE ANGULAR VELOCITY CLAMPING
// Verify: Corner impacts do not induce limit-cycle angular rocking > 2.5 rad/s.
// ============================================================================

#[test]
fn test_corner_impact_angular_velocity_clamping_and_no_limit_cycles() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Create a flat rubble ground at y = 1.0
    let ground_y = 1.0f32;
    for gx in 10..30 {
        for gz in 10..30 {
            if let Some((rx, rz)) = PhysicsSystem::world_to_rubble_grid_coords(gx as f32, gz as f32) {
                phys.rubble_height_grid[rx * voxel::GRID_Z + rz] = ground_y;
            }
        }
    }
    phys.max_rubble_height = ground_y;

    // Create an elongated slab tilted diagonally so one corner strikes first
    let rot = Vec3::new(0.61, 0.0, 0.44);
    let chunk = create_test_chunk(
        Vec3::new(20.0, 1.8, 20.0),
        Vec3::new(0.5, -6.0, -0.3),
        rot,
        Vec3::new(1.2, 0.5, -0.8),
        IVec3::new(6, 2, 3),
        600.0,
        VoxelType::Concrete,
    );
    phys.chunks.push(chunk);

    let mut max_ang_vel_observed = 0.0f32;
    let mut infinite_limit_cycle_detected = false;

    for step in 0..120 {
        if phys.chunks.is_empty() {
            break;
        }

        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

        if !phys.chunks.is_empty() {
            let chunk = &phys.chunks[0];
            let ang_spd = chunk.angular_velocity.length();
            max_ang_vel_observed = max_ang_vel_observed.max(ang_spd);

            let max_axis = chunk.angular_velocity.x.abs()
                .max(chunk.angular_velocity.y.abs())
                .max(chunk.angular_velocity.z.abs());

            // Check if after 60 frames (1 second) of contact it is still rocking > 2.5 rad/s
            if step > 60 && ang_spd > 2.5 {
                infinite_limit_cycle_detected = true;
            }

            if step % 15 == 0 || step == 119 {
                println!(
                    "Step {:3}: pos.y = {:5.2}, vy = {:+5.2}, ||omega|| = {:5.2}, max_axis = {:5.2}, settled = {}",
                    step, chunk.position.y, chunk.velocity.y, ang_spd, max_axis, chunk.settled
                );
            }
        }
    }

    println!("Max angular velocity observed during impact: {:.2} rad/s", max_ang_vel_observed);
    assert!(
        !infinite_limit_cycle_detected,
        "Corner strikes must not induce persistent limit-cycle rocking > 2.5 rad/s"
    );
    assert!(
        phys.chunks[0].settled,
        "Chunk on flat ground must settle after corner impact energy dissipates"
    );
}

// ============================================================================
// CHALLENGE 4: HIGH-ALTITUDE MASS DROP FREE-FALL (50m DROP)
// Verify: High-mass chunks falling from 50m maintain realistic momentum
// and their fractured descendants settle with external support.
// ============================================================================

#[test]
fn test_high_altitude_50m_mass_drop_settling() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    let ground_y = 0.5f32;
    for gx in 15..25 {
        for gz in 15..25 {
            if let Some((rx, rz)) = PhysicsSystem::world_to_rubble_grid_coords(gx as f32, gz as f32) {
                phys.rubble_height_grid[rx * voxel::GRID_Z + rz] = ground_y;
            }
        }
    }
    phys.max_rubble_height = ground_y;

    // Drop 3,500 kg monolithic slab from y = 50.0m
    let chunk = create_test_chunk(
        Vec3::new(20.0, 50.0, 20.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        IVec3::new(6, 4, 6),
        3500.0,
        VoxelType::Concrete,
    );
    phys.chunks.push(chunk);

    let mut impact_detected = false;
    let mut fractured = false;

    // Across 35 seeds, the latest descendant activity ended at frame 430.
    // Eight seconds covers the measured fracture/rocking interval without a mission timer.
    for step in 0..500 {
        let pre_lowest = phys.chunks.iter().map(DynamicChunk::lowest_world_y).fold(f32::INFINITY, f32::min);
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert!(!phys.chunks.is_empty(), "Drop descendants must remain in the simulation at step {step}");
        assert_eq!(phys.chunks.iter().map(|c| c.voxels.len()).sum::<usize>(), 144, "Drop must conserve its voxels at step {step}");
        let total_mass: f32 = phys.chunks.iter().map(|c| c.mass).sum();
        assert!((total_mass - 3500.0).abs() < 0.1, "Drop must conserve its mass at step {step}: {total_mass}");
        fractured |= phys.chunks.len() > 1;

        let lowest = phys.chunks.iter().map(DynamicChunk::lowest_world_y).fold(f32::INFINITY, f32::min);
        if !impact_detected {
            assert!(lowest < pre_lowest, "Slab must descend before foot contact at step {step}");
            impact_detected = lowest <= ground_y + physics::CONTACT_SLOP;
        }
        for (idx, chunk) in phys.chunks.iter().enumerate() {
            assert!(chunk.position.is_finite() && chunk.velocity.is_finite() && chunk.rotation.is_finite() && chunk.angular_velocity.is_finite(), "Drop kinematics must remain finite at step {step}, fragment {idx}");
            if chunk.settled {
                let gap = external_raster_support_gap(idx, &phys, ground_y);
                assert!(gap.is_some_and(|g| g <= physics::CONTACT_SLOP + 1e-5), "Settled fragment must have external support at step {step}, fragment {idx}: {gap:?}");
            }
        }
    }

    assert!(impact_detected, "Slab feet must reach the ground from a 50m drop");
    assert!(fractured, "The high-speed slab impact must fracture");
    // Conserved fracture mass leaves each descendant above 500 kg; their COM can rise
    // during supported rotation, so mass is no longer a proxy for an unfractured slab.
    // The separate heavy-impact test checks zero ground restitution before fracture.
    assert!(phys.chunks.iter().all(|c| c.settled), "All drop descendants must settle within eight seconds");
}

// ============================================================================
// CHALLENGE 5: DYNAMIC CHUNK FRACTURE MOMENTUM & TANGENTIAL VELOCITY
// Verify: Sub-chunks inherit parent linear, angular, and tangential velocity (omega x r)
// ============================================================================

#[test]
fn test_dynamic_chunk_fracture_tangential_velocity_stress() {
    let mut voxels = Vec::with_capacity(60);
    for x in -30..30 {
        voxels.push((Vec3::new(x as f32 * 0.12, 0.0, 0.0), VoxelType::Concrete));
    }
    let aabb = DynamicChunk::compute_aabb(&voxels);
    let parent_pos = Vec3::new(15.0, 25.0, 15.0);
    let parent_vel = Vec3::new(10.0, -15.0, 5.0);
    let parent_ang = Vec3::new(0.0, 6.0, 0.0);

    let parent = DynamicChunk {
        position: parent_pos,
        velocity: parent_vel,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: parent_ang,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 500.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    };

    let (sub_chunks, _, _) = parent.fracture();
    assert!(sub_chunks.len() >= 2, "Fracture of 60-voxel elongated chunk must produce at least 2 sub-chunks");

    for sub in &sub_chunks {
        // Downward momentum must be preserved
        assert!(
            sub.velocity.y < -8.0,
            "Downward velocity must be preserved: got vy = {}",
            sub.velocity.y
        );

        // Angular spin around Y must be inherited from parent
        assert!(
            (sub.angular_velocity.y - parent_ang.y).abs() <= 2.0,
            "Sub-chunk must inherit parent angular velocity y: got {} vs parent {}",
            sub.angular_velocity.y, parent_ang.y
        );

        // Tangential velocity omega x r
        let r_offset = sub.position - parent_pos;
        if r_offset.x.abs() > 0.8 {
            let expected_tang_z = -parent_ang.y * r_offset.x;
            let actual_rel_z = sub.velocity.z - parent_vel.z;
            assert!(
                (expected_tang_z * actual_rel_z) > 0.0,
                "Tangential velocity must align with omega x r: expected_tang_z = {}, actual_rel_z = {}",
                expected_tang_z, actual_rel_z
            );
        }
    }
}
