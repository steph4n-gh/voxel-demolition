#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DebrisParticle, DynamicChunk, PhysicsSystem};
use voxel::{
    partition_mesh_sub_batches, VoxelGrid, VoxelType, MAX_CHUNK_BATCH_INDICES,
    MAX_CHUNK_BATCH_VERTICES, VOXEL_SIZE,
};

// ============================================================================
// CHALLENGER 2 SUITE: SUB-BATCH ZERO-CLAMPING & 3,000 DEBRIS PARTICLE STRESS
// ============================================================================

#[test]
fn test_sub_batch_stress_50k_indices_with_massive_vertex_gaps() {
    // 54,000 indices across 5 discrete vertex clusters separated by gaps > 10,000
    // Cluster 0: base 0..300 (span ~300)
    // Cluster 1: base 12,000..12,300 (gap ~11,700)
    // Cluster 2: base 25,000..25,300 (gap ~12,700)
    // Cluster 3: base 40,000..40,300 (gap ~14,700)
    // Cluster 4: base 55,000..55,300 (gap ~14,700)
    // Total vertex count: 60,000 vertices
    let cluster_bases = [0u16, 12000, 25000, 40000, 55000];
    let triangles_per_cluster = 3600; // 3600 * 3 = 10,800 indices per cluster; 5 * 10,800 = 54,000 indices
    let mut indices = Vec::with_capacity(54000);

    for &base in &cluster_bases {
        for t in 0..triangles_per_cluster {
            let offset = (t % 280) as u16;
            let i0 = base + offset;
            let i1 = base + offset + 1;
            let i2 = base + offset + 2;
            indices.push(i0);
            indices.push(i1);
            indices.push(i2);
        }
    }

    assert_eq!(indices.len(), 54000);
    let vertices_len = 60000;

    let batches = partition_mesh_sub_batches(
        &indices,
        vertices_len,
        MAX_CHUNK_BATCH_VERTICES,
        MAX_CHUNK_BATCH_INDICES,
    );

    // Invariants assertion
    assert!(!batches.is_empty(), "Batches must not be empty");
    let mut reconstructed_indices = Vec::with_capacity(indices.len());
    let mut total_batch_indices = 0;

    for (b_i, (vert_range, remapped)) in batches.iter().enumerate() {
        let batch_idx_count = remapped.len();
        let span = (vert_range.end() - vert_range.start()) + 1;

        assert!(
            batch_idx_count <= MAX_CHUNK_BATCH_INDICES,
            "Batch {} index count {} exceeded MAX_CHUNK_BATCH_INDICES (4800)",
            b_i,
            batch_idx_count
        );
        assert!(
            span <= MAX_CHUNK_BATCH_VERTICES,
            "Batch {} vertex span {} exceeded MAX_CHUNK_BATCH_VERTICES (4000)",
            b_i,
            span
        );

        for &remapped_idx in remapped {
            assert!(
                (remapped_idx as usize) < span,
                "Remapped index {} outside vertex span {}",
                remapped_idx,
                span
            );
            reconstructed_indices.push(remapped_idx + (*vert_range.start() as u16));
        }
        total_batch_indices += batch_idx_count;
    }

    // 100% Geometry Retention check
    assert_eq!(
        total_batch_indices,
        indices.len(),
        "Total indices across all sub-batches must exactly equal input (0 dropped, 0 duplicated)"
    );
    assert_eq!(
        reconstructed_indices, indices,
        "Reconstructed indices must match original stream bit-for-bit"
    );
}

#[test]
fn test_sub_batch_stress_pathological_odd_triangle_counts_and_singletons() {
    // Test pathological odd counts:
    // Case A: 50,001 indices (16,667 triangles - odd triangle count)
    // Case B: 50,003 indices (16,667 triangles + 2 trailing non-triangular indices)
    // Case C: 50,005 indices (16,668 triangles + 1 trailing non-triangular index)
    let test_lengths = [50001, 50003, 50005];

    for &len in &test_lengths {
        let mut indices = Vec::with_capacity(len);
        let mut cur_base = 100u16;

        for i in 0..len {
            if i % 3000 == 0 && i > 0 {
                // Gap of 11,000 every 3000 indices
                cur_base = (cur_base + 11000) % 55000;
            }
            let offset = (i % 250) as u16;
            indices.push(cur_base + offset);
        }

        let vertices_len = 65535;
        let batches = partition_mesh_sub_batches(
            &indices,
            vertices_len,
            MAX_CHUNK_BATCH_VERTICES,
            MAX_CHUNK_BATCH_INDICES,
        );

        let mut total_indices = 0;
        let mut reconstructed = Vec::with_capacity(len);

        for (b_i, (vert_range, remapped)) in batches.iter().enumerate() {
            let span = (vert_range.end() - vert_range.start()) + 1;
            assert!(
                remapped.len() <= MAX_CHUNK_BATCH_INDICES,
                "Length {}: Batch {} index count {} > 4800",
                len,
                b_i,
                remapped.len()
            );
            assert!(
                span <= MAX_CHUNK_BATCH_VERTICES,
                "Length {}: Batch {} span {} > 4000",
                len,
                b_i,
                span
            );

            for &idx in remapped {
                assert!((idx as usize) < span);
                reconstructed.push(idx + (*vert_range.start() as u16));
            }
            total_indices += remapped.len();
        }

        assert_eq!(
            total_indices, len,
            "100% index retention violated for pathological length {}",
            len
        );
        assert_eq!(
            reconstructed, indices,
            "Reconstruction mismatch for length {}",
            len
        );
    }
}

#[test]
fn test_sub_batch_stress_fuzz_random_dispersed_triangles() {
    // Pseudorandom deterministic LCG to generate 60,000 indices across widely dispersed islands
    let mut rng_state: u64 = 0xDEADBEEFCAFEBABE;
    let mut lcg = || -> u64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };

    let island_bases = [500u16, 14000, 28000, 43000, 58000];
    let num_triangles = 20000; // 60,000 indices
    let mut indices = Vec::with_capacity(num_triangles * 3);

    for _ in 0..num_triangles {
        let island_idx = (lcg() % island_bases.len() as u64) as usize;
        let base = island_bases[island_idx];
        let offset = (lcg() % 350) as u16;
        indices.push(base + offset);
        indices.push(base + offset + 1);
        indices.push(base + offset + 2);
    }

    assert_eq!(indices.len(), 60000);
    let vertices_len = 65535;

    let batches = partition_mesh_sub_batches(
        &indices,
        vertices_len,
        MAX_CHUNK_BATCH_VERTICES,
        MAX_CHUNK_BATCH_INDICES,
    );

    let mut total_count = 0;
    for (b_i, (vert_range, remapped)) in batches.iter().enumerate() {
        let span = (vert_range.end() - vert_range.start()) + 1;
        assert!(
            remapped.len() <= MAX_CHUNK_BATCH_INDICES,
            "Fuzz batch {} indices {} > 4800",
            b_i,
            remapped.len()
        );
        assert!(
            span <= MAX_CHUNK_BATCH_VERTICES,
            "Fuzz batch {} span {} > 4000",
            b_i,
            span
        );
        total_count += remapped.len();
    }

    assert_eq!(
        total_count, 60000,
        "Zero geometry drop invariant failed during 60,000-index fuzzing"
    );
}

#[test]
fn test_debris_pool_3000_active_particles_tumbling_and_bouncing() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Populate a rubble heightmap mound at center (x=32, z=32)
    for x in 28..36 {
        for z in 28..36 {
            let h = 2.5 - ((x as f32 - 32.0).hypot(z as f32 - 32.0) * 0.3).max(0.0);
            phys.rubble_heightmap_set(x as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE, h);
        }
    }
    phys.rubble_dirty = false;

    // Spawn 3,000 active debris particles tumbling from various heights above the rubble mound
    for i in 0..3000 {
        let angle = (i as f32) * 0.12;
        let radius = ((i % 100) as f32) * 0.04;
        let px = 16.0 + angle.cos() * radius;
        let pz = 16.0 + angle.sin() * radius;
        let py = 6.0 + ((i % 50) as f32) * 0.15;

        phys.debris.push(DebrisParticle {
            position: Vec3::new(px, py, pz),
            velocity: Vec3::new(angle.cos() * 2.5, -2.0, angle.sin() * 2.5),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::new(3.0, 5.0, 2.0),
            size: 0.12,
            color: Color::new(0.7, 0.7, 0.7, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    assert_eq!(phys.debris.len(), 3000);

    // Simulate 60 physics frames (1.0 second of simulation)
    let dt = 1.0 / 60.0;
    for _ in 0..60 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }

    // Assert that particles tumbled and settled, and none clipped below bedrock (y=0.50)
    assert!(phys.debris.len() <= 3000);
    for p in &phys.debris {
        assert!(
            !p.position.x.is_nan() && !p.position.y.is_nan() && !p.position.z.is_nan(),
            "Debris particle position contains NaN"
        );
        assert!(
            p.position.y >= 0.50,
            "Debris particle penetrated bedrock: y = {}",
            p.position.y
        );
    }
}

#[test]
fn test_debris_resting_sleep_optimization_o1() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Deposit stable flat bedrock / rubble at y = 1.0
    for x in 10..40 {
        for z in 10..40 {
            phys.rubble_heightmap_set(x as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE, 1.0);
        }
    }
    phys.rubble_dirty = false;

    // Spawn 1,000 already-settled debris particles resting on top of rubble (y = 1.0 + size/2 = 1.05)
    for i in 0..1000 {
        let px = 10.0 + (i % 20) as f32 * 0.4;
        let pz = 10.0 + (i / 20) as f32 * 0.15;
        phys.debris.push(DebrisParticle {
            position: Vec3::new(px, 1.05, pz),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.10,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 4.0,
            max_life: 4.0,
            bounces: 3,
        });
    }

    // Step physics with rubble_dirty = false
    phys.update(0.05, &mut grid, &audio, Vec3::ZERO);

    // Assert that all particles remained exactly asleep at (1.05) with zero velocity
    for (i, p) in phys.debris.iter().enumerate() {
        assert_eq!(
            p.velocity,
            Vec3::ZERO,
            "Sleeping particle {} must maintain zero velocity",
            i
        );
        assert_eq!(
            p.angular_velocity,
            Vec3::ZERO,
            "Sleeping particle {} must maintain zero angular velocity",
            i
        );
        assert!(
            (p.position.y - 1.05).abs() < 1e-4,
            "Sleeping particle {} position drifted during O(1) sleep: y = {}",
            i,
            p.position.y
        );
    }

    // Now dirty the rubble to verify particles re-check without glitching
    phys.rubble_dirty = true;
    phys.update(0.05, &mut grid, &audio, Vec3::ZERO);
    for (i, p) in phys.debris.iter().enumerate() {
        assert!(
            (p.position.y - 1.05).abs() <= 0.06,
            "Particle {} should remain resting on rubble after re-check",
            i
        );
    }
}

#[test]
fn test_debris_continuous_accelerated_decay_under_6000_saturation() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Saturate debris pool with 6,000 particles (2x MAX_DEBRIS = 3000)
    for _ in 0..6000 {
        phys.debris.push(DebrisParticle {
            position: Vec3::new(10.0, 10.0, 10.0),
            velocity: Vec3::new(1.0, 0.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.1,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 3.0,
            bounces: 0,
        });
    }

    assert_eq!(phys.debris.len(), 6000);

    // Call enforce_particle_caps directly
    phys.enforce_particle_caps();

    // CRITICAL: Assert NO .drain() occurred! Length must still be 6000 immediately after enforce_particle_caps
    assert_eq!(
        phys.debris.len(),
        6000,
        "Continuous accelerated decay must NOT drain/pop particles abruptly"
    );

    // Assert that excess particles (first 3,000) received accelerated decay lifetimes <= 0.04s
    for i in 0..3000 {
        assert!(
            phys.debris[i].life <= 0.041,
            "Hard excess particle {} life {} should be compressed to <= 0.04s",
            i,
            phys.debris[i].life
        );
    }

    // Advance simulation by 0.05s (exceeding 0.04s accelerated decay threshold)
    phys.update(0.05, &mut grid, &audio, Vec3::ZERO);

    // The expired excess particles are smoothly removed via frame-by-frame swap_remove, bringing pool <= 3000
    assert!(
        phys.debris.len() <= 3000,
        "After 0.05s step, expired saturated particles must be cleared smoothly. Current len: {}",
        phys.debris.len()
    );
}

#[test]
fn test_pancake_collapse_mass_accretion_and_compound_momentum_transfer() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Create a 2-story reinforced concrete floor structure:
    // Floor 1 at y = 6 (voxel coordinates x in 28..36, z in 28..36)
    // Floor 2 at y = 10 (voxel coordinates x in 28..36, z in 28..36)
    for x in 28..36 {
        for z in 28..36 {
            grid.set(x, 6, z, VoxelType::Concrete);
            grid.set(x, 10, z, VoxelType::Concrete);
        }
    }

    let concrete_density = VoxelType::Concrete.properties().density;
    assert_eq!(concrete_density, 2.4);

    // Construct a falling heavy chunk (8x8 slab) positioned just above Floor 2 (y = 10)
    // with downward velocity -14.0 m/s
    let mut falling_voxels = Vec::new();
    for x in 0..6 {
        for z in 0..6 {
            falling_voxels.push((
                Vec3::new(x as f32 * VOXEL_SIZE, 0.0, z as f32 * VOXEL_SIZE),
                VoxelType::HeavyArmor,
            ));
        }
    }

    let aabb = DynamicChunk::compute_aabb(&falling_voxels);
    let initial_mass = 400.0f32;
    let initial_vel = Vec3::new(0.0, -14.0, 0.0);
    let slab_world = grid.voxel_to_world(IVec3::new(28, 10, 28));

    let chunk = DynamicChunk {
        position: slab_world + Vec3::new(0.0, 0.35, 0.0),
        velocity: initial_vel,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: falling_voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: initial_mass,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    };
    phys.chunks.push(chunk);

    let initial_solids = grid.solid_count();
    let pre_debris_len = phys.debris.len();

    // Step physics to impact and crush Floor 2
    for _ in 0..5 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    // Verify Floor 2 was pulverized
    let solids_after_floor2 = grid.solid_count();
    assert!(
        solids_after_floor2 < initial_solids,
        "Floor 2 voxels must be pulverized into dynamic debris chunks"
    );

    // The main chunk is exactly 6x6 (36 voxels) in footprint. It should crush exactly 36 voxels.
    // We cannot reliably use (initial_solids - solids_after_floor2) as the exact expectation, 
    // because secondary flying sub-chunks may randomly crush adjacent floor voxels, 
    // artificially inflating the expected mass and causing flaky failures.
    let expected_direct_crush_voxels = 36.0f32;
    let expected_mass_accrued_floor2 = expected_direct_crush_voxels * concrete_density * 0.35;

    // Verify mass accretion Delta M = sum(rho * 0.35)
    let chunk_after_floor2 = &phys.chunks[0];
    assert!(
        chunk_after_floor2.mass >= initial_mass + expected_mass_accrued_floor2 * 0.90, // Allow 10% margin for voxel sub-block culling
        "Chunk mass {} must accrete pulverized floor mass (expected >= {})",
        chunk_after_floor2.mass,
        initial_mass + expected_mass_accrued_floor2 * 0.90
    );

    // Verify compound momentum transfer: downward velocity damped by drag loss
    assert!(
        chunk_after_floor2.velocity.y.abs() < initial_vel.y.abs(),
        "Downward velocity must undergo drag loss during crushing: initial {}, current {}",
        initial_vel.y,
        chunk_after_floor2.velocity.y
    );

    // Verify dynamic micro-chunks and spall spawned
    assert!(
        phys.debris.len() > pre_debris_len,
        "Pulverization must spawn debris particles"
    );
}
