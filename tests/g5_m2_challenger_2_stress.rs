//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G5-M2
//! ADVERSARIAL STRESS-TESTING: WEAPON TRAILS, C4 DET-CORD CACHING,
//! AND SCRATCH SUB-BLOCKS POOL STABILIZATION
//! Target: tests/g5_m2_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DynamicChunk, Mat3, PhysicsSystem};
use siege::{ColossalShell, SiegeSystem, SiegeWeaponConfig};
use voxel::{VoxelGrid, VoxelType, GRID_X, GRID_Y, GRID_Z};
use weapon::{ActiveProjectile, TrailBuffer, WeaponMode, WeaponSystem, MAX_C4_CHARGES};

// ============================================================================
// SUITE 1: TRAILBUFFER<16> & TRAILBUFFER<18> STRESS TESTING
// ============================================================================

#[test]
fn test_adversarial_trail_buffer_wraparound_exact_oldest_to_newest_ordering() {
    let mut tb = TrailBuffer::<16>::new();
    assert_eq!(tb.len(), 0);
    assert!(tb.is_empty());

    // Push 1..=16 elements
    for step in 1..=16 {
        tb.push(Vec3::new(step as f32, 0.0, 0.0));
        assert_eq!(tb.len(), step);
        assert_eq!(tb[0].x, 1.0, "Oldest element must always be 1.0 during initial fill");
        assert_eq!(tb[step - 1].x, step as f32, "Newest element must match step");
    }

    // Now push elements 17..=10,000 (continuous wraparound)
    for step in 17..=10_000 {
        tb.push(Vec3::new(step as f32, 0.0, 0.0));
        assert_eq!(tb.len(), 16, "Buffer length must cap strictly at 16");

        let expected_oldest = (step - 15) as f32;
        let expected_newest = step as f32;
        assert_eq!(
            tb[0].x, expected_oldest,
            "tb[0] must be oldest element {} at step {}",
            expected_oldest, step
        );
        assert_eq!(
            tb[15].x, expected_newest,
            "tb[15] must be newest element {} at step {}",
            expected_newest, step
        );

        // Verify strictly monotonic ordering from 0 to 15
        for i in 0..15 {
            let curr = tb[i].x;
            let next = tb[i + 1].x;
            assert_eq!(
                next,
                curr + 1.0,
                "Ordering invariant broken at step {}: tb[{}]={} != tb[{}]={}",
                step,
                i + 1,
                next,
                i,
                curr
            );
        }
    }
}

#[test]
fn test_adversarial_trail_buffer_colossal_shell_18_wraparound_and_bounds() {
    let mut tb = siege::TrailBuffer::<18>::new();
    assert_eq!(tb.len(), 0);
    assert!(tb.is_empty());

    // Push 10,000 steps into TrailBuffer<18>
    for step in 1..=10_000 {
        tb.push(Vec3::new(0.0, step as f32, 0.0));
        let expected_len = step.min(18);
        assert_eq!(tb.len(), expected_len);

        let oldest_idx = if step >= 18 { step - 17 } else { 1 };
        assert_eq!(
            tb[0].y, oldest_idx as f32,
            "Oldest element tb[0] mismatch at step {}",
            step
        );
        assert_eq!(
            tb[expected_len - 1].y,
            step as f32,
            "Newest element tb[{}] mismatch at step {}",
            expected_len - 1,
            step
        );

        // Verify get() boundary contract
        assert!(tb.get(expected_len).is_none(), "get(len) must return None");
        assert!(tb.get(expected_len + 5).is_none(), "get(len + 5) must return None");
        assert!(tb.get(expected_len - 1).is_some(), "get(len - 1) must return Some");
    }

    // Verify clear() contract
    tb.clear();
    assert_eq!(tb.len(), 0);
    assert!(tb.is_empty());
    assert!(tb.get(0).is_none());
}

#[test]
#[should_panic(expected = "TrailBuffer index out of bounds: 16 >= 16")]
fn test_adversarial_trail_buffer_out_of_bounds_panics_at_capacity() {
    let mut tb = TrailBuffer::<16>::new();
    for i in 0..20 {
        tb.push(Vec3::splat(i as f32));
    }
    let _ = tb[16]; // Must panic with exact bounds message
}

#[test]
#[should_panic(expected = "TrailBuffer index out of bounds: 5 >= 5")]
fn test_adversarial_trail_buffer_out_of_bounds_panics_at_partial_fill() {
    let mut tb = TrailBuffer::<16>::new();
    for i in 0..5 {
        tb.push(Vec3::splat(i as f32));
    }
    let _ = tb[5]; // Must panic with exact bounds message
}

#[test]
fn test_adversarial_weapon_and_shell_trail_simulation_hundreds_projectiles_1000_steps() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let mut weapon = WeaponSystem::new();
    let mut siege = SiegeSystem::new();

    // Spawn 200 active projectiles
    for i in 0..200 {
        weapon.projectiles.push(ActiveProjectile {
            position: Vec3::new(i as f32 * 0.5, 50.0, 10.0),
            velocity: Vec3::new(1.0, 0.0, 0.5),
            gravity: 9.8,
            radius: 0.1,
            blast_radius: 1.0,
            impulse: 10.0,
            mode: WeaponMode::HavocMortar,
            barrel: weapon::BarrelType::Precision,
            payload: weapon::PayloadType::HighExplosive,
            trail_style: weapon::TrailStyle::PlasmaGlow,
            color: Color::new(1.0, 0.5, 0.0, 1.0),
            life: 0.0,
            is_cluster_child: false,
            is_airburst: false,
            is_sticky: false,
            stuck_normal: Vec3::ZERO,
            trail_points: TrailBuffer::single(Vec3::new(i as f32 * 0.5, 50.0, 10.0)),
        });
    }

    // Spawn 100 colossal shells
    for i in 0..100 {
        siege.colossal_shells.push(ColossalShell {
            position: Vec3::new(i as f32 * 0.5, 80.0, 20.0),
            velocity: Vec3::new(0.5, -0.01, 0.2),
            caliber_mm: 800.0,
            radius: 0.5,
            blast_radius: 5.0,
            payload: siege::SiegePayload::ColossalHighExplosive,
            penetrations_remaining: 5,
            fuse_timer: 0.0,
            is_fuse_active: false,
            life: 0.0,
            max_life: 1000.0,
            color: Color::new(1.0, 0.5, 0.0, 1.0),
            trail_points: siege::TrailBuffer::new(),
            has_detonated: false,
        });
    }

    // Step simulation for 1,200 steps (>1,000 steps required)
    for step in 0..1200 {
        weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);
        siege.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);

        // Spot-check first and last projectile trails
        if let Some(p) = weapon.projectiles.first() {
            assert!(p.trail_points.len() <= 16);
            if p.trail_points.len() >= 2 {
                for idx in 0..p.trail_points.len() - 1 {
                    let p1 = p.trail_points[idx];
                    let p2 = p.trail_points[idx + 1];
                    // Position moved forward along velocity vector
                    assert!(
                        p2.x >= p1.x - 1e-4,
                        "Projectile trail monotonicity failure at step {}",
                        step
                    );
                }
            }
        }

        // Spot-check colossal shells
        if let Some(shell) = siege.colossal_shells.first() {
            assert!(shell.trail_points.len() <= 18);
            if shell.trail_points.len() >= 2 {
                for idx in 0..shell.trail_points.len() - 1 {
                    let p1 = shell.trail_points[idx];
                    let p2 = shell.trail_points[idx + 1];
                    assert!(
                        p2.x >= p1.x - 1e-4,
                        "Colossal shell trail monotonicity failure at step {}",
                        step
                    );
                }
            }
        }
    }
}

// ============================================================================
// SUITE 2: C4 DET-CORD CACHING & TOPOLOGY SYNCHRONIZATION STRESS TESTING
// ============================================================================

#[test]
fn test_adversarial_c4_det_cord_caching_full_lifecycle_and_dirty_sync() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let mut weapon = WeaponSystem::new();

    // 1. Place 24 C4 charges in a connected perimeter on solid concrete anchors
    for i in 0..MAX_C4_CHARGES {
        let x = (i % 6) as i32 * 3 + 10;
        let z = (i / 6) as i32 * 3 + 10;
        grid.set(x, 4, z, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(x, 4, z));
        let success = weapon.place_c4_charge(world_pos, Vec3::Y, Some(IVec3::new(x, 4, z)));
        assert!(success, "Placing charge {} must succeed", i);
    }
    assert_eq!(weapon.c4_charges.len(), 24);

    // Initial check: cached_det_cords must be populated with topology
    assert!(
        !weapon.cached_det_cords.is_empty(),
        "Cached det-cords must be computed on placement"
    );
    let initial_links_count = weapon.cached_det_cords.len();
    assert!(!weapon.det_cords_dirty, "Dirty flag should be cleared after update_det_cord_cache");

    // 2. Steady-state draw frames simulation: 600 frames
    // Record cached_det_cords address / len to verify zero allocations and zero churn
    let initial_links = weapon.cached_det_cords.clone();
    for frame in 0..600 {
        // In draw loop: draw_world_entities uses self.cached_det_cords directly
        assert_eq!(
            weapon.cached_det_cords.len(),
            initial_links_count,
            "Steady-state frame {} must maintain identical link count",
            frame
        );
        for (idx, link) in weapon.cached_det_cords.iter().enumerate() {
            assert_eq!(link.idx_a, initial_links[idx].idx_a);
            assert_eq!(link.idx_b, initial_links[idx].idx_b);
            assert_eq!(link.p1, initial_links[idx].p1);
            assert_eq!(link.p2, initial_links[idx].p2);
        }
        // Run update: no anchor changes, so dirty flag must remain false
        weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert!(
            !weapon.det_cords_dirty,
            "Dirty flag must stay false in steady state at frame {}",
            frame
        );
    }

    // 3. Detonate all 24 charges
    let (feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::new(15.0, 5.0, 15.0));
    assert_eq!(count, 24, "All 24 charges must detonate");
    assert_eq!(weapon.c4_charges.len(), 0, "C4 charges must be cleared after detonation");
    assert_eq!(weapon.cached_det_cords.len(), 0, "Cached det-cords must be cleared on detonation");
    assert!(!weapon.det_cords_dirty, "Dirty flag must remain false after detonation");
    assert!(feedback.camera_trauma > 0.0);

    // 4. Re-arm with 24 charges in a new arrangement
    for i in 0..MAX_C4_CHARGES {
        let x = 20 + (i as i32 % 4) * 4;
        let z = 20 + (i as i32 / 4) * 4;
        grid.set(x, 4, z, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(x, 4, z));
        let success = weapon.place_c4_charge(world_pos, Vec3::Y, Some(IVec3::new(x, 4, z)));
        assert!(success, "Re-arming charge {} must succeed", i);
    }
    assert_eq!(weapon.c4_charges.len(), 24);
    assert!(!weapon.cached_det_cords.is_empty(), "Re-armed det cords must be populated");
    assert!(!weapon.det_cords_dirty);

    // 5. Test anchor severance dirty flag trigger:
    // Sever one anchor voxel: charge should be dropped on update, triggering det_cords_dirty sync
    grid.set(20, 4, 20, VoxelType::Air);
    let old_links_count = weapon.cached_det_cords.len();
    weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.c4_charges.len(), 23, "Severed charge must be removed");
    assert!(
        weapon.cached_det_cords.len() <= old_links_count,
        "Links must be updated to reflect removed charge"
    );
    assert!(!weapon.det_cords_dirty, "Dirty flag must be reset after cache update");
}

#[test]
fn test_adversarial_c4_clear_and_det_cords_dirty_synchronization() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let mut weapon = WeaponSystem::new();

    // 1. Place 24 charges
    for i in 0..MAX_C4_CHARGES {
        let x = (i % 6) as i32 * 3 + 10;
        let z = (i / 6) as i32 * 3 + 10;
        grid.set(x, 4, z, VoxelType::Concrete);
        let world_pos = grid.voxel_to_world(IVec3::new(x, 4, z));
        weapon.place_c4_charge(world_pos, Vec3::Y, Some(IVec3::new(x, 4, z)));
    }
    assert_eq!(weapon.c4_charges.len(), 24);
    assert!(!weapon.cached_det_cords.is_empty());

    // 2. Now simulate clear: charges are cleared and det_cords_dirty is set to true
    weapon.c4_charges.clear();
    weapon.det_cords_dirty = true;

    // Call update: does update synchronize cached_det_cords when det_cords_dirty is true?
    weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // CHALLENGE: Is cached_det_cords synchronized?
    assert!(
        !weapon.det_cords_dirty,
        "det_cords_dirty must be processed and cleared by update()"
    );
    assert_eq!(
        weapon.cached_det_cords.len(), 0,
        "cached_det_cords must be synchronized to empty when charges are cleared"
    );
}

// ============================================================================
// SUITE 3: SCRATCH SUB-BLOCKS POOL STABILITY STRESS TESTING
// ============================================================================

#[test]
fn test_adversarial_scratch_sub_blocks_capacity_retention_across_varying_chunk_sizes() {
    let mut scratch: Vec<((Vec3, Vec3), Vec<usize>)> = Vec::with_capacity(4);

    // 1. Large chunk (>64 voxels) -> requires 4 splits
    let mut large_vox = Vec::new();
    for x in 0..10 {
        for z in 0..10 {
            large_vox.push((Vec3::new(x as f32, 0.0, z as f32), VoxelType::Concrete));
        }
    }
    let large_aabb = DynamicChunk::compute_aabb(&large_vox);
    let chunk_large = DynamicChunk {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: large_vox,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 1000.0,
        life: 0.0,
        settled: false,
        local_aabb: large_aabb,
    };

    // 2. Medium chunk (25..=64 voxels) -> requires 2 splits
    let mut med_vox = Vec::new();
    for x in 0..6 {
        for z in 0..6 {
            med_vox.push((Vec3::new(x as f32, 0.0, z as f32), VoxelType::Concrete));
        }
    }
    let med_aabb = DynamicChunk::compute_aabb(&med_vox);
    let chunk_med = DynamicChunk {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: med_vox,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 300.0,
        life: 0.0,
        settled: false,
        local_aabb: med_aabb,
    };

    // 3. Small chunk (<=24 voxels) -> requires 1 split
    let mut small_vox = Vec::new();
    for x in 0..3 {
        for z in 0..3 {
            small_vox.push((Vec3::new(x as f32, 0.0, z as f32), VoxelType::Concrete));
        }
    }
    let small_aabb = DynamicChunk::compute_aabb(&small_vox);
    let chunk_small = DynamicChunk {
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: small_vox,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 50.0,
        life: 0.0,
        settled: false,
        local_aabb: small_aabb,
    };

    // First populate with large chunk: should initialize 4 buckets
    chunk_large.populate_sub_blocks(&mut scratch);
    assert_eq!(scratch.len(), 4, "Large chunk must populate 4 sub-blocks");
    let ptrs_initial: Vec<*const usize> = scratch.iter().map(|(_, v)| v.as_ptr()).collect();
    println!("Initial bucket buffer pointers: {:?}", ptrs_initial);

    // Now populate with medium chunk (25..=64 voxels) -> requires 2 splits
    chunk_med.populate_sub_blocks(&mut scratch);
    assert_eq!(scratch.len(), 2);
    let ptrs_med: Vec<*const usize> = scratch.iter().map(|(_, v)| v.as_ptr()).collect();
    println!("Medium bucket buffer pointers: {:?}", ptrs_med);

    // Now populate with small chunk (<=24 voxels) -> requires 1 split
    chunk_small.populate_sub_blocks(&mut scratch);
    assert_eq!(scratch.len(), 1);
    let ptrs_small: Vec<*const usize> = scratch.iter().map(|(_, v)| v.as_ptr()).collect();
    println!("Small bucket buffer pointers: {:?}", ptrs_small);

    // Now populate with large chunk again
    chunk_large.populate_sub_blocks(&mut scratch);
    assert_eq!(scratch.len(), 4);
    let ptrs_large2: Vec<*const usize> = scratch.iter().map(|(_, v)| v.as_ptr()).collect();
    println!("Large2 bucket buffer pointers: {:?}", ptrs_large2);

    // Check whether buckets 1, 2, 3 were reallocated:
    let bucket_2_reallocated = ptrs_large2[2] != ptrs_initial[2];
    let bucket_3_reallocated = ptrs_large2[3] != ptrs_initial[3];
    println!(
        "Reallocation check: bucket 2 reallocated = {}, bucket 3 reallocated = {}",
        bucket_2_reallocated, bucket_3_reallocated
    );
    assert!(
        !bucket_2_reallocated && !bucket_3_reallocated,
        "Scratch sub-blocks must retain inner vector allocations across varying chunk sizes (bucket 2 reallocated: {}, bucket 3 reallocated: {})",
        bucket_2_reallocated,
        bucket_3_reallocated
    );
}

#[test]
fn test_adversarial_pancake_collapse_and_chunk_fracture_scratch_sub_blocks_stress() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Build floor at y = 4 (height 2.0m)
    for x in 10..40 {
        for z in 10..40 {
            grid.set(x, 4, z, VoxelType::Glass);
        }
    }

    // Spawn massive multi-story pancake slab chunks falling at high velocity
    for story in 0..3 {
        let mut voxels = Vec::new();
        for x in 0..12 {
            for z in 0..12 {
                voxels.push((Vec3::new(x as f32 * 0.5, 0.0, z as f32 * 0.5), VoxelType::HeavyArmor));
            }
        }
        let aabb = DynamicChunk::compute_aabb(&voxels);
        phys.chunks.push(DynamicChunk {
            position: Vec3::new(15.0, 8.0 + story as f32 * 4.0, 15.0),
            velocity: Vec3::new(0.0, -15.0, 0.0),
            rotation: Vec3::new(0.01, 0.02, 0.0),
            rot_mat: Mat3::from_euler(Vec3::new(0.01, 0.02, 0.0)),
            angular_velocity: Vec3::new(0.05, 0.0, 0.05),
            voxels,
            exposed_voxels: Vec::new(),
            voxel_health: Vec::new(),
            mass: 2000.0,
            life: 0.0,
            settled: false,
            local_aabb: aabb,
        });
    }

    // Pre-populate scratch_sub_blocks to 4 elements
    if let Some(chunk) = phys.chunks.first() {
        chunk.populate_sub_blocks(&mut phys.scratch_sub_blocks);
    }
    assert_eq!(phys.scratch_sub_blocks.len(), 4);
    let initial_outer_cap = phys.scratch_sub_blocks.capacity();

    // Step simulation across 120 frames (pancake collapse, pulverization, fracture)
    for step in 0..120 {
        phys.update(0.016, &mut grid, &audio, Vec3::ZERO);
        assert!(
            phys.scratch_sub_blocks.capacity() >= initial_outer_cap,
            "Outer scratch capacity must never shrink at step {}",
            step
        );
    }
}
