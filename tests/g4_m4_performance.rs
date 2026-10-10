#![allow(clippy::new_without_default)]

#[allow(dead_code, unused_imports)]
#[path = "../src/audio.rs"]
pub mod audio;
#[allow(dead_code, unused_imports)]
#[path = "../src/voxel.rs"]
pub mod voxel;
#[allow(dead_code, unused_imports)]
#[path = "../src/physics.rs"]
pub mod physics;
#[allow(dead_code, unused_imports)]
#[path = "../src/weapon.rs"]
pub mod weapon;
#[allow(dead_code, unused_imports)]
#[path = "../src/siege.rs"]
pub mod siege;
#[allow(dead_code, unused_imports)]
#[path = "../src/player.rs"]
pub mod player;
#[allow(dead_code, unused_imports)]
#[path = "../src/world_gen.rs"]
pub mod world_gen;
#[allow(dead_code, unused_imports)]
#[path = "../src/customizer.rs"]
pub mod customizer;
#[allow(dead_code, unused_imports)]
#[path = "../src/ui.rs"]
pub mod ui;
#[allow(dead_code, unused_imports)]
#[path = "../src/controls.rs"]
pub mod controls;
#[allow(dead_code, unused_imports)]
#[path = "../src/skyline.rs"]
pub mod skyline;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{
    DecalKind, DynamicChunk, ImpactDecal, PhysicsSystem,
    PressurizedFireJet, ShockwaveRing, SmokeParticle, SparkParticle, ElectricalArc, DebrisParticle
};
use voxel::{
    FrustumPlanes, VoxelGrid, VoxelType,
    MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES,
    partition_mesh_sub_batches, partition_mesh_sub_batches_into,
};

// ============================================================================
// MILESTONE G4-M4 INTEGRATION TEST SUITE: HIGH-PERFORMANCE ENGINE HARDENING
// Feature 15: Localized Sub-Region BFS Traversal
// Feature 16: 6-Plane Camera Frustum Culling on Particles & Decals
// Feature 17: Zero-Allocation Meshing & Inner Loop Pooling
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Feature 15: Localized Sub-Region BFS Traversal Tests
// ----------------------------------------------------------------------------

#[test]
fn test_dirty_region_marking_and_bounds_containment() {
    let mut grid = VoxelGrid::new();

    // Verify initial state: no dirty bounds
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // Single point mark
    let pt = IVec3::new(25, 12, 40);
    grid.mark_dirty_point(pt);
    assert_eq!(grid.dirty_min, Some(pt));
    assert_eq!(grid.dirty_max, Some(pt));

    // Region mark expanding bounding box
    let r_min = IVec3::new(10, 5, 30);
    let r_max = IVec3::new(35, 20, 55);
    grid.mark_dirty_region(r_min, r_max);

    assert_eq!(grid.dirty_min, Some(IVec3::new(10, 5, 30)));
    assert_eq!(grid.dirty_max, Some(IVec3::new(35, 20, 55)));

    // Setting a voxel marks the point dirty
    grid.set(50, 2, 50, VoxelType::Concrete);
    assert_eq!(grid.dirty_max.unwrap().x, 50);

    // Carving a sphere marks the affected region
    let carve_center = grid.voxel_to_world(IVec3::new(20, 10, 20));
    grid.carve_sphere(carve_center, 2.0);
    assert!(grid.dirty_min.is_some());
    assert!(grid.dirty_max.is_some());

    // After detect_unsupported_islands, dirty bounds are atomically taken and consumed
    let _ = grid.detect_unsupported_islands();
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());
}

#[test]
fn test_cantilever_depth_4_invariance_under_subregion_bfs() {
    let mut grid = VoxelGrid::new();
    grid.set_preserve_single_voxel_debris(true);

    // Build grounded column at (40, 40) from bedrock y=0 to y=10
    for y in 0..=10 {
        grid.set(40, y, 40, VoxelType::Bedrock);
    }

    // Build horizontal cantilevers of lengths 1, 2, 3, 4 protruding from (40, 10, 40) along +X
    for dx in 1..=4 {
        grid.set(40 + dx, 10, 40, VoxelType::Glass);
    }

    // Protrude length 5 (should be unsupported since CANTILEVER_MAX_DEPTH == 4)
    grid.set(40 + 5, 10, 40, VoxelType::Glass);

    // Carve a small voxel near the cantilever tip to mark sub-region dirty
    grid.set(40 + 5, 11, 40, VoxelType::Air);

    let islands = grid.detect_unsupported_islands();
    // Length 5 should sever, while lengths 1..=4 must remain firmly anchored!
    assert!(!islands.is_empty(), "Depth 5 cantilever should sever");
    let total_island_voxels: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(total_island_voxels, 1, "Only the 5th voxel (beyond depth 4) should sever");

    // Verify depths 1..=4 are still solid in the grid
    for dx in 1..=4 {
        assert_eq!(grid.get(40 + dx, 10, 40), VoxelType::Glass);
    }
    // Verify depth 5 is now Air
    assert_eq!(grid.get(40 + 5, 10, 40), VoxelType::Air);
}

#[test]
fn test_skyline_skyscraper_catastrophic_severance_adaptive_vertical_expansion() {
    let mut grid = VoxelGrid::new();

    // Construct a substantial skyscraper column (4x4 footprint, from y=1 up to y=60 = 960 voxels)
    // with cross-braces spanning to 2,600+ voxels
    let base_x = 44;
    let base_z = 44;
    let mut expected_building_voxels = 0;

    for y in 1..=65 {
        for dx in 0..8 {
            for dz in 0..8 {
                // Hollow skyscraper floor with core and perimeter
                if dx == 0 || dx == 7 || dz == 0 || dz == 7 || ((2..=5).contains(&dx) && (2..=5).contains(&dz)) {
                    grid.set(base_x + dx, y, base_z + dz, VoxelType::Concrete);
                    expected_building_voxels += 1;
                }
            }
        }
    }

    assert!(expected_building_voxels >= 2500, "Building voxel count {} must be >= 2500 for collapse test", expected_building_voxels);

    // Initial check: building is grounded on bedrock (y=0)
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Initially grounded skyscraper must produce 0 islands");

    // Sever the base horizontally across y=1..=2 strictly within the building footprint
    for dx in 0..8 {
        for dz in 0..8 {
            grid.set(base_x + dx, 1, base_z + dz, VoxelType::Air);
            grid.set(base_x + dx, 2, base_z + dz, VoxelType::Air);
        }
    }

    // Localized dirty sub-region is now set to y=1..=2 around the base.
    // The adaptive vertical expansion loop MUST climb all the way to y=65
    // and extract the entire severed building (>2,500 voxels)!
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severed skyscraper base must trigger catastrophic collapse");

    let total_collapsed_voxels: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert!(
        total_collapsed_voxels >= 2400,
        "Adaptive vertical expansion must collect full building crown, got {} voxels (expected >= 2400)",
        total_collapsed_voxels
    );

    // Verify the grid voxels are now cleared
    assert_eq!(grid.get(base_x + 1, 30, base_z + 1), VoxelType::Air);
    assert_eq!(grid.get(base_x + 7, 60, base_z + 7), VoxelType::Air);
}

#[test]
fn test_scratch_buffer_zero_reallocation_pointer_stability() {
    let mut grid = VoxelGrid::new();

    // Record initial buffer pointer addresses
    let ptr_grounded = grid.scratch_grounded.as_ptr();
    let ptr_visited = grid.scratch_visited.as_ptr();
    let ptr_tension = grid.scratch_tension.as_ptr();
    let ptr_depth = grid.scratch_depth.as_ptr();
    let cap_queue = grid.scratch_queue.capacity();

    // Perform multiple operations and BFS runs
    for i in 0..10 {
        grid.set(30 + i, 1, 30, VoxelType::SteelRebar);
        grid.set(30 + i, 2, 30, VoxelType::Concrete);
        let _ = grid.detect_unsupported_islands();
    }

    // Verify pointer stability across multiple runs (zero reallocations!)
    assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded must never reallocate");
    assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited must never reallocate");
    assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension must never reallocate");
    assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth must never reallocate");
    assert!(grid.scratch_queue.capacity() >= cap_queue, "scratch_queue must not thrash");
}

// ----------------------------------------------------------------------------
// 2. Feature 16: 6-Plane Camera Frustum Culling on Particles & Decals Tests
// ----------------------------------------------------------------------------

#[test]
fn test_frustum_construction_and_containment_primitives() {
    // Camera at (0, 10, 0), looking toward (0, 10, 50) along +Z, aspect=1.0, fov=60 deg
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let cam_dir = Vec3::Z;
    let cam_up = Vec3::Y;
    let frustum = FrustumPlanes::from_camera(cam_pos, cam_dir, cam_up, 1.0, std::f32::consts::FRAC_PI_3, 0.1, 200.0);

    // 1. Point directly in front of camera
    assert!(frustum.contains_point(Vec3::new(0.0, 10.0, 20.0)));

    // 2. Point directly behind camera
    assert!(!frustum.contains_point(Vec3::new(0.0, 10.0, -20.0)));

    // 3. Point far to the left
    assert!(!frustum.contains_point(Vec3::new(-100.0, 10.0, 20.0)));

    // 4. Point beyond far plane
    assert!(!frustum.contains_point(Vec3::new(0.0, 10.0, 300.0)));

    // 5. Sphere containment
    assert!(frustum.is_sphere_in_frustum(Vec3::new(0.0, 10.0, 15.0), 2.0));
    assert!(!frustum.is_sphere_in_frustum(Vec3::new(0.0, 10.0, -15.0), 2.0));

    // 6. AABB containment
    assert!(frustum.is_aabb_in_frustum(Vec3::new(-2.0, 8.0, 10.0), Vec3::new(2.0, 12.0, 14.0)));
    assert!(!frustum.is_aabb_in_frustum(Vec3::new(-2.0, 8.0, -20.0), Vec3::new(2.0, 12.0, -10.0)));
}

#[test]
fn test_emitter_culling_all_particle_types_and_metrics() {
    let mut physics = PhysicsSystem::new();
    let cam_pos = Vec3::new(0.0, 5.0, 0.0);
    let cam_dir = Vec3::Z;
    let frustum = FrustumPlanes::from_camera(cam_pos, cam_dir, Vec3::Y, 1.0, std::f32::consts::FRAC_PI_3, 0.1, 100.0);

    // Add 10 sparks in front (+Z) and 10 sparks behind (-Z)
    for i in 0..10 {
        physics.sparks.push(SparkParticle {
            position: Vec3::new(0.0, 5.0, 10.0 + i as f32),
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 1.0,
            max_life: 1.0,
        });
        physics.sparks.push(SparkParticle {
            position: Vec3::new(0.0, 5.0, -10.0 - i as f32),
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 1.0,
            max_life: 1.0,
        });
    }

    // Add 8 smoke puffs in front and 8 behind
    for i in 0..8 {
        physics.smoke.push(SmokeParticle {
            position: Vec3::new(0.0, 5.0, 8.0 + i as f32),
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 1.0,
            color: Color::new(0.5, 0.5, 0.5, 0.2),
            life: 0.5,
            max_life: 1.0,
        });
        physics.smoke.push(SmokeParticle {
            position: Vec3::new(0.0, 5.0, -8.0 - i as f32),
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 1.0,
            color: Color::new(0.5, 0.5, 0.5, 0.2),
            life: 0.5,
            max_life: 1.0,
        });
    }

    // Add 4 shockwaves in front and 4 behind
    for i in 0..4 {
        physics.shockwaves.push(ShockwaveRing {
            position: Vec3::new(0.0, 5.0, 15.0 + i as f32 * 5.0),
            normal: Vec3::Y,
            radius: 2.0,
            max_radius: 5.0,
            speed: 5.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thickness: 0.2,
        });
        physics.shockwaves.push(ShockwaveRing {
            position: Vec3::new(0.0, 5.0, -15.0 - i as f32 * 5.0),
            normal: Vec3::Y,
            radius: 2.0,
            max_radius: 5.0,
            speed: 5.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thickness: 0.2,
        });
    }

    // Add 2 electrical arcs in front and 2 behind
    for i in 0..2 {
        physics.electrical_arcs.push(ElectricalArc {
            start: Vec3::new(-1.0, 5.0, 12.0 + i as f32 * 2.0),
            end: Vec3::new(1.0, 5.0, 12.0 + i as f32 * 2.0),
            color: Color::new(0.2, 0.8, 1.0, 1.0),
            timer: 0.5,
            max_timer: 0.5,
        });
        physics.electrical_arcs.push(ElectricalArc {
            start: Vec3::new(-1.0, 5.0, -12.0 - i as f32 * 2.0),
            end: Vec3::new(1.0, 5.0, -12.0 - i as f32 * 2.0),
            color: Color::new(0.2, 0.8, 1.0, 1.0),
            timer: 0.5,
            max_timer: 0.5,
        });
    }

    // Add 2 fire jets in front and 2 behind
    for i in 0..2 {
        physics.fire_jets.push(PressurizedFireJet {
            position: Vec3::new(0.0, 5.0, 14.0 + i as f32 * 2.0),
            direction: Vec3::Z,
            timer: 0.5,
            max_timer: 0.5,
        });
        physics.fire_jets.push(PressurizedFireJet {
            position: Vec3::new(0.0, 5.0, -14.0 - i as f32 * 2.0),
            direction: -Vec3::Z,
            timer: 0.5,
            max_timer: 0.5,
        });
    }

    // Add 6 decals in front and 6 behind
    for i in 0..6 {
        physics.decals.push(ImpactDecal {
            position: Vec3::new(0.0, 5.0, 10.0 + i as f32 * 2.0),
            normal: Vec3::Y,
            size: 1.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            kind: DecalKind::Scorch,
            life: 10.0,
            max_life: 10.0,
            voxel_anchor: None,
        });
        physics.decals.push(ImpactDecal {
            position: Vec3::new(0.0, 5.0, -10.0 - i as f32 * 2.0),
            normal: Vec3::Y,
            size: 1.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            kind: DecalKind::Scorch,
            life: 10.0,
            max_life: 10.0,
            voxel_anchor: None,
        });
    }

    // Add 10 debris particles in front and 10 behind
    for i in 0..10 {
        physics.debris.push(DebrisParticle {
            position: Vec3::new(0.0, 5.0, 10.0 + i as f32),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.2,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
        physics.debris.push(DebrisParticle {
            position: Vec3::new(0.0, 5.0, -10.0 - i as f32),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.2,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    // Call frustum-culled draw passes
    physics.draw_solids_frustum(cam_pos, &[], Some(&frustum));
    physics.draw_particles_frustum(cam_pos, &[], Some(&frustum));

    let metrics = physics.cull_metrics.get();

    // Verify total counts
    assert_eq!(metrics.sparks_total, 20);
    assert_eq!(metrics.smoke_total, 16);
    assert_eq!(metrics.shockwaves_total, 8);
    assert_eq!(metrics.arcs_total, 4);
    assert_eq!(metrics.fire_total, 4);
    assert_eq!(metrics.decals_total, 12);
    assert_eq!(metrics.debris_total, 20);

    // Verify exactly half drawn (the ones in front of the camera!)
    assert_eq!(metrics.sparks_drawn, 10);
    assert_eq!(metrics.smoke_drawn, 8);
    assert_eq!(metrics.shockwaves_drawn, 4);
    assert_eq!(metrics.arcs_drawn, 2);
    assert_eq!(metrics.fire_drawn, 2);
    assert_eq!(metrics.decals_drawn, 6);
    assert_eq!(metrics.debris_drawn, 10);

    // Cull ratio must be exactly 50%
    assert!((metrics.cull_ratio() - 0.50).abs() < 0.01);
}

#[test]
fn test_frustum_culling_efficiency_radial_particle_distribution() {
    let mut physics = PhysicsSystem::new();
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let cam_dir = Vec3::Z;
    // Standard 60 degree horizontal/vertical FOV camera
    let frustum = FrustumPlanes::from_camera(cam_pos, cam_dir, Vec3::Y, 1.0, std::f32::consts::FRAC_PI_3, 0.5, 60.0);

    // Generate 360-degree radial ring of sparks (1,200 sparks, radius 25m)
    let total_sparks = 1200;
    for i in 0..total_sparks {
        let angle = (i as f32 / total_sparks as f32) * std::f32::consts::TAU;
        let pos = cam_pos + Vec3::new(angle.cos() * 25.0, 0.0, angle.sin() * 25.0);
        physics.sparks.push(SparkParticle {
            position: pos,
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 0.8, 0.3, 1.0),
            life: 2.0,
            max_life: 2.0,
        });
    }

    physics.draw_particles_frustum(cam_pos, &[], Some(&frustum));
    let metrics = physics.cull_metrics.get();

    // A 60-degree cone across a 360-degree radial ring covers ~60/360 = ~16.7% of azimuth.
    // Therefore, between 60% and 85% of particles should be culled!
    let cull_pct = metrics.cull_ratio() * 100.0;
    assert!(
        (60.0..=90.0).contains(&cull_pct),
        "Radial 360-degree distribution cull percentage must be between 60% and 90%, got {:.1}%",
        cull_pct
    );
    assert!(metrics.sparks_drawn > 0, "At least some sparks must be drawn in view");
    assert!(metrics.sparks_drawn < total_sparks, "Not all sparks should be drawn");
}

// ----------------------------------------------------------------------------
// 3. Feature 17: Zero-Allocation Meshing & Inner Loop Pooling Tests
// ----------------------------------------------------------------------------

#[test]
fn test_partition_mesh_sub_batches_into_zero_alloc_equivalence() {
    // Generate large test index stream (6,000 indices across 3,000 vertices)
    let mut indices = Vec::with_capacity(6000);
    for t in 0..2000u16 {
        let base = (t * 2) % 2800;
        indices.push(base);
        indices.push(base + 1);
        indices.push(base + 2);
    }

    let legacy_batches = partition_mesh_sub_batches(&indices, 3000, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);

    let mut scratch_batches = Vec::new();
    partition_mesh_sub_batches_into(
        &indices,
        3000,
        MAX_CHUNK_BATCH_VERTICES,
        MAX_CHUNK_BATCH_INDICES,
        &mut scratch_batches,
    );

    assert_eq!(legacy_batches.len(), scratch_batches.len(), "Batch count must match exactly");

    for (legacy, (scratch_vert_range, scratch_idx_range)) in legacy_batches.iter().zip(&scratch_batches) {
        assert_eq!(&legacy.0, scratch_vert_range, "Vertex range must match exactly");
        let min_v = *scratch_vert_range.start() as u16;
        let scratch_remapped: Vec<u16> = indices[scratch_idx_range.clone()].iter().map(|&i| i - min_v).collect();
        assert_eq!(&legacy.1, &scratch_remapped, "Remapped indices must match legacy output exactly");
    }
}

#[test]
fn test_mesh_pool_submesh_recycling_and_capacity_cap() {
    let mut grid = VoxelGrid::new();

    // Verify initial mesh pool is empty
    assert!(grid.mesh_pool.is_empty());

    // Build dense voxel geometry in chunk (1, 1, 1) that generates sub-meshes
    let (start_x, start_y, start_z) = (16, 16, 16);
    for x in start_x..start_x + 12 {
        for y in start_y..start_y + 12 {
            for z in start_z..start_z + 12 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Rebuild chunk mesh
    grid.update_meshes();
    let cx = start_x as usize / voxel::CHUNK_SIZE;
    let cy = (start_y - voxel::MIN_VOXEL_Y) as usize / voxel::CHUNK_SIZE;
    let cz = start_z as usize / voxel::CHUNK_SIZE;
    let ci = VoxelGrid::chunk_idx(cx, cy, cz);
    assert!(!grid.meshes[ci].sub_meshes.is_empty(), "Chunk must generate sub-meshes");

    // Clear chunk voxels
    for x in start_x..start_x + 12 {
        for y in start_y..start_y + 12 {
            for z in start_z..start_z + 12 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Rebuild empty chunk: sub-meshes must be recycled into mesh_pool!
    grid.update_meshes();
    assert!(grid.meshes[ci].sub_meshes.is_empty());
    assert!(!grid.mesh_pool.is_empty(), "Cleared sub-meshes must be recycled into mesh_pool");
    assert!(grid.mesh_pool.len() <= 64, "Mesh pool must remain capped <= 64");

    // Rebuilding again should reuse meshes from pool without allocating
    for x in start_x..start_x + 8 {
        for y in start_y..start_y + 8 {
            for z in start_z..start_z + 8 {
                grid.set(x, y, z, VoxelType::SteelRebar);
            }
        }
    }
    grid.update_meshes();
    assert!(!grid.meshes[ci].sub_meshes.is_empty());
}

#[test]
fn test_physics_update_scratch_buffer_reuse_and_zero_realloc() {
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(24.0, 10.0, 24.0);

    // Initial scratch buffer capacities
    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);
    assert!(physics.scratch_fracture_sub_chunks.capacity() >= 32);

    let ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();
    let ptr_settle = physics.scratch_chunks_to_settle.as_ptr();
    let ptr_new_crush = physics.scratch_new_crush_chunks.as_ptr();
    let ptr_fracture = physics.scratch_fracture_sub_chunks.as_ptr();

    // Spawn falling dynamic chunk
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(24.0, 20.0, 24.0),
        Vec3::new(0.0, -15.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // Run physics update for 60 simulation ticks
    for _ in 0..60 {
        let _ = physics.update(0.016, &mut grid, &audio, cam_pos);
    }

    // Verify scratch buffers retained their allocated capacities across ticks
    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);
    assert!(physics.scratch_fracture_sub_chunks.capacity() >= 32);

    // Buffer pointers should remain stable
    assert_eq!(physics.scratch_craters_to_unsettle.as_ptr(), ptr_craters);
    assert_eq!(physics.scratch_chunks_to_settle.as_ptr(), ptr_settle);
    assert_eq!(physics.scratch_new_crush_chunks.as_ptr(), ptr_new_crush);
    assert_eq!(physics.scratch_fracture_sub_chunks.as_ptr(), ptr_fracture);
}
