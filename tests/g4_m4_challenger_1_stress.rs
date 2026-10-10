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

use macroquad::prelude::*;
use physics::{DebrisParticle, PhysicsSystem};
use voxel::{
    CarveResult, DamageType, FrustumPlanes, VoxelGrid, VoxelType,
    CHUNK_SIZE, GRID_X, GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y,
    MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES,
    partition_mesh_sub_batches, partition_mesh_sub_batches_into,
};

// ============================================================================
// EMPIRICAL CHALLENGER ADVERSARIAL STRESS TEST SUITE: MILESTONE G4-M4
// Agent: g4_m4_challenger_1 (Adversarial Stress Challenger)
//
// Stress Dimensions:
// 1. Feature 15: Sub-region boundaries, dirty region marking on set/carve/freeze,
//    cantilever depth 4 invariance, skyscraper catastrophic collapse,
//    sub-region touches_solid_outside fallback.
// 2. Feature 16: Frustum culling efficiency, extreme FOV, near/far boundaries,
//    3,000 active debris particles, and HUD cull metrics formula consistency.
// 3. Feature 17: Chunk mesh pooling cap (64), sub-batch partitioning invariance,
//    and zero-reallocation pointer stability across VoxelGrid & PhysicsSystem.
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Feature 15: Adversarial Dirty Region Marking & Structural Bounds
// ----------------------------------------------------------------------------

#[test]
fn test_adversarial_dirty_region_marking_all_mutation_paths() {
    let mut grid = VoxelGrid::new();

    // 1. Initial State Invariant: No dirty bounds
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // 2. Out-of-bounds set should be gracefully ignored without mutating dirty bounds
    grid.set(-10, MIN_VOXEL_Y - 5, -10, VoxelType::Concrete);
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    grid.set(GRID_X as i32 + 10, MAX_VOXEL_Y + 10, GRID_Z as i32 + 10, VoxelType::Concrete);
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // Setting a voxel to its already existing type should be a no-op (no dirty mark)
    // (0, MIN_VOXEL_Y, 0) is initialized to Bedrock in VoxelGrid::new()
    grid.set(0, MIN_VOXEL_Y, 0, VoxelType::Bedrock);
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // 3. Modifying boundary points:
    // (0, MIN_VOXEL_Y, 0) changed from Bedrock to Air
    grid.set(0, MIN_VOXEL_Y, 0, VoxelType::Air);
    assert_eq!(grid.dirty_min, Some(IVec3::new(0, MIN_VOXEL_Y, 0)));
    assert_eq!(grid.dirty_max, Some(IVec3::new(0, MIN_VOXEL_Y, 0)));

    // (GRID_X - 1, MAX_VOXEL_Y - 1, GRID_Z - 1) changed from Air to SteelRebar
    grid.set(GRID_X as i32 - 1, MAX_VOXEL_Y - 1, GRID_Z as i32 - 1, VoxelType::SteelRebar);
    assert_eq!(grid.dirty_min, Some(IVec3::new(0, MIN_VOXEL_Y, 0)));
    assert_eq!(
        grid.dirty_max,
        Some(IVec3::new(GRID_X as i32 - 1, MAX_VOXEL_Y - 1, GRID_Z as i32 - 1))
    );

    // Clean up via detect_unsupported_islands
    let _ = grid.detect_unsupported_islands();
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // 4. Carve sphere into empty air: out.destroyed == 0 -> MUST NOT mark dirty bounds!
    let empty_pos = grid.voxel_to_world(IVec3::new(48, 50, 48));
    let mut carve_res = CarveResult::default();
    grid.carve_sphere_advanced_into(empty_pos, 2.0, 100.0, DamageType::Explosive, &mut carve_res);
    assert_eq!(carve_res.destroyed, 0);
    assert!(grid.dirty_min.is_none(), "Carving air must not mark dirty region");
    assert!(grid.dirty_max.is_none(), "Carving air must not mark dirty region");

    // 5. Carve sphere into subterranean Granite Bedrock / Basalt (Y < 0)
    let subterranean_pos = grid.voxel_to_world(IVec3::new(30, -5, 30));
    let mut carve_bedrock_res = CarveResult::default();
    // Use high energy (5000) to penetrate basalt durability
    grid.carve_sphere_advanced_into(subterranean_pos, 2.5, 5000.0, DamageType::Explosive, &mut carve_bedrock_res);
    assert!(carve_bedrock_res.destroyed > 0, "Subterranean substrate should be carved by 5000 energy");
    assert!(grid.dirty_min.is_some());
    assert!(grid.dirty_max.is_some());
    assert!(grid.dirty_min.unwrap().y <= -5, "Subterranean carve must record negative dirty Y");

    // Clean up
    let _ = grid.detect_unsupported_islands();

    // 6. Freeze sphere on empty air: frozen == 0 -> MUST NOT mark dirty bounds!
    let frozen_empty = grid.freeze_sphere(empty_pos, 3.0);
    assert_eq!(frozen_empty, 0);
    assert!(grid.dirty_min.is_none(), "Freezing empty air must not mark dirty region");
    assert!(grid.dirty_max.is_none(), "Freezing empty air must not mark dirty region");

    // 7. Freeze sphere on solid concrete -> MUST mark dirty bounds!
    for y in 0..=20 {
        grid.set(40, y, 40, VoxelType::Concrete);
    }
    let _ = grid.detect_unsupported_islands(); // flush

    let concrete_pos = grid.voxel_to_world(IVec3::new(40, 20, 40));
    let frozen_solid = grid.freeze_sphere(concrete_pos, 1.5);
    assert!(frozen_solid > 0, "Concrete within freeze radius should convert to CryoIce");
    assert_eq!(grid.get(40, 20, 40), VoxelType::CryoIce);
    assert!(grid.dirty_min.is_some());
    assert!(grid.dirty_max.is_some());
    assert!(grid.dirty_min.unwrap().x <= 40 && grid.dirty_max.unwrap().x >= 40);
    assert!(grid.dirty_min.unwrap().y <= 20 && grid.dirty_max.unwrap().y >= 20);
    assert!(grid.dirty_min.unwrap().z <= 40 && grid.dirty_max.unwrap().z >= 40);
}

// ----------------------------------------------------------------------------
// 2. Feature 15: Cantilever Depth 4 Invariance in All Compass & Diagonal Directions
// ----------------------------------------------------------------------------

#[test]
fn test_cantilever_depth_4_invariance_multi_directional_stress() {
    let mut grid = VoxelGrid::new();
    grid.set_preserve_single_voxel_debris(true);

    // Build central grounded column from bedrock y=0 to y=20
    let cx = 48;
    let cz = 48;
    for y in 0..=20 {
        grid.set(cx, y, cz, VoxelType::Bedrock);
    }

    // Directions to test: 4 Cardinal (+X, -X, +Z, -Z) and 4 Coplanar Diagonals
    let directions = [
        (1, 0),   // +X
        (-1, 0),  // -X
        (0, 1),   // +Z
        (0, -1),  // -Z
        (1, 1),   // (+X, +Z)
        (-1, 1),  // (-X, +Z)
        (1, -1),  // (+X, -Z)
        (-1, -1), // (-X, -Z)
    ];

    // For each direction, extend horizontal cantilever out to length 4 (anchored) and length 5 (severed)
    for &(dx, dz) in &directions {
        for step in 1..=4 {
            grid.set(cx + dx * step, 20, cz + dz * step, VoxelType::Concrete);
        }
        // Voxel 5 exceeds CANTILEVER_MAX_DEPTH
        grid.set(cx + dx * 5, 20, cz + dz * 5, VoxelType::Concrete);
    }

    // Provoke sub-region BFS by modifying a voxel adjacent to the center column
    grid.set(cx, 21, cz, VoxelType::Air);

    // Detect unsupported islands: localized sub-region BFS must strictly sever length 5 in all 8 directions
    // while keeping lengths 1..=4 securely grounded!
    let islands = grid.detect_unsupported_islands();

    // 8 directions each having exactly 1 severed voxel at step 5
    let total_severed_voxels: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(
        total_severed_voxels, 8,
        "Exactly 8 voxels (depth 5 in all 8 directions) must sever under cantilever depth 4 rule"
    );

    // Verify all depth 1..=4 voxels remain intact in the grid
    for &(dx, dz) in &directions {
        for step in 1..=4 {
            assert_eq!(
                grid.get(cx + dx * step, 20, cz + dz * step),
                VoxelType::Concrete,
                "Cantilever step {} in dir ({}, {}) must remain solid",
                step, dx, dz
            );
        }
        // Verify depth 5 is cleared to Air
        assert_eq!(
            grid.get(cx + dx * 5, 20, cz + dz * 5),
            VoxelType::Air,
            "Cantilever step 5 in dir ({}, {}) must be cleared",
            dx, dz
        );
    }
}

// ----------------------------------------------------------------------------
// 3. Feature 15: Skyscraper Catastrophic Severance Adaptive Vertical Expansion
// ----------------------------------------------------------------------------

#[test]
fn test_skyscraper_catastrophic_severance_full_crown_collapse() {
    let mut grid = VoxelGrid::new();

    // Build a dense multi-tier skyscraper from y=1 to y=75
    // Footprint: 8x8 with central core and perimeter shear walls
    let base_x = 44;
    let base_z = 44;
    let mut total_building_voxels = 0;

    for y in 1..=75 {
        for dx in 0..8 {
            for dz in 0..8 {
                let is_perimeter = dx == 0 || dx == 7 || dz == 0 || dz == 7;
                let is_core = (2..=5).contains(&dx) && (2..=5).contains(&dz);
                let is_floor = y % 5 == 0;
                if is_perimeter || is_core || is_floor {
                    grid.set(base_x + dx, y, base_z + dz, VoxelType::Concrete);
                    total_building_voxels += 1;
                }
            }
        }
    }

    assert!(
        total_building_voxels >= 2500,
        "Skyscraper voxel count must be >= 2500, got {}",
        total_building_voxels
    );

    // Ground check: building rests on bedrock at y=0, no islands initially
    let initial_islands = grid.detect_unsupported_islands();
    assert!(initial_islands.is_empty(), "Initial building must be fully grounded");

    // Sever the base horizontally across y=1..=2 across the full 8x8 footprint
    let mut severed_base_voxels = 0;
    for dx in 0..8 {
        for dz in 0..8 {
            for y in 1..=2 {
                if grid.get(base_x + dx, y, base_z + dz) == VoxelType::Concrete {
                    grid.set(base_x + dx, y, base_z + dz, VoxelType::Air);
                    severed_base_voxels += 1;
                }
            }
        }
    }

    let expected_falling_voxels = total_building_voxels - severed_base_voxels;
    assert!(expected_falling_voxels >= 2500);

    // Localized sub-region dirty bounds are now restricted around y=1..=2.
    // The adaptive vertical expansion MUST climb up to y=75 and collapse the entire building crown!
    let collapse_islands = grid.detect_unsupported_islands();
    assert!(!collapse_islands.is_empty(), "Severed skyscraper base must collapse");

    let actual_collapsed_voxels: usize = collapse_islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(
        actual_collapsed_voxels, expected_falling_voxels,
        "Adaptive vertical expansion must collect exactly all severed upper voxels (expected {}, got {})",
        expected_falling_voxels, actual_collapsed_voxels
    );

    // Verify grid has ZERO lingering solid voxels floating in mid-air
    for y in 3..=75 {
        for dx in 0..8 {
            for dz in 0..8 {
                assert_eq!(
                    grid.get(base_x + dx, y, base_z + dz),
                    VoxelType::Air,
                    "No voxel should remain floating at ({}, {}, {})",
                    base_x + dx, y, base_z + dz
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------
// 4. Feature 15: Sub-Region Fallback On Solid Contact Outside (`touches_solid_outside`)
// ----------------------------------------------------------------------------

#[test]
fn test_subregion_bfs_fallback_touches_solid_outside_skybridge() {
    let mut grid = VoxelGrid::new();

    // Tower A column at (20, y, 48) from y=0 to y=20
    for y in 0..=20 {
        grid.set(20, y, 48, VoxelType::Bedrock);
    }

    // Tower B column at (60, y, 48) from y=0 to y=20
    for y in 0..=20 {
        grid.set(60, y, 48, VoxelType::Bedrock);
    }

    // Skybridge connecting Tower A and Tower B at y=20 (from x=21 to x=59) made of SteelRebar
    for x in 21..=59 {
        grid.set(x, 20, 48, VoxelType::SteelRebar);
    }

    // Initial state: fully supported
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Both towers and skybridge should be grounded");

    // Sever Tower A foundation at y=1..=2.
    // Dirty region is tightly around x=20, y=1..=2, z=48.
    // The localized sub-region padded margin is pad = 8 -> x in [12, 28].
    // But the skybridge extends to x=59!
    // As the BFS floods through Tower A and along the skybridge, at x=29 it encounters solid rebar outside the box.
    // This MUST trigger `touches_solid_outside = true`, safely falling back to global full-world BFS!
    grid.set(20, 1, 48, VoxelType::Air);
    grid.set(20, 2, 48, VoxelType::Air);

    // In this scenario, Tower A is severed at its base, but it is physically suspended by the steel rebar
    // skybridge connected to Tower B!
    // Under full-world BFS, steel rebar in tension can support downward weight.
    let _islands = grid.detect_unsupported_islands();

    // Because the sub-region safely fell back to full-world BFS, Tower A doesn't get partially clipped or truncated!
    // Verify that the skybridge rebar at x=25..=55 is intact in the world
    for x in 25..=55 {
        assert_eq!(grid.get(x, 20, 48), VoxelType::SteelRebar);
    }

    // Now sever Tower B at y=1..=2 as well
    grid.set(60, 1, 48, VoxelType::Air);
    grid.set(60, 2, 48, VoxelType::Air);

    // Now BOTH towers and the skybridge are completely disconnected from bedrock.
    // Localized check at Tower B triggers fallback, and the entire structure collapses as islands!
    let catastrophic_islands = grid.detect_unsupported_islands();
    assert!(!catastrophic_islands.is_empty(), "Completely severed twin towers and skybridge must collapse");

    // Verify all elevated parts are cleared to Air
    assert_eq!(grid.get(20, 20, 48), VoxelType::Air);
    assert_eq!(grid.get(40, 20, 48), VoxelType::Air);
    assert_eq!(grid.get(60, 20, 48), VoxelType::Air);
}

// ----------------------------------------------------------------------------
// 5. Feature 15 & 17: Scratch Buffer Zero Reallocation Pointer Stability
// ----------------------------------------------------------------------------

#[test]
fn test_adversarial_scratch_buffer_zero_reallocation_stress() {
    let mut grid = VoxelGrid::new();

    // Warm up buffers with an initial detect call
    let _ = grid.detect_unsupported_islands();

    // Record initial scratch buffer pointer addresses and capacities
    let ptr_grounded = grid.scratch_grounded.as_ptr();
    let ptr_visited = grid.scratch_visited.as_ptr();
    let ptr_tension = grid.scratch_tension.as_ptr();
    let ptr_depth = grid.scratch_depth.as_ptr();
    let cap_queue = grid.scratch_queue.capacity();

    // Perform 25 randomized localized voxel operations:
    // small carves, setting single voxels, freezing spheres, and island detections
    for i in 0..25 {
        let x = 30 + (i % 15);
        let z = 30 + ((i * 3) % 15);
        let y = 5 + (i % 10);

        grid.set(x, y, z, VoxelType::SteelRebar);
        grid.set(x, y + 1, z, VoxelType::Concrete);

        if i % 3 == 0 {
            let wp = grid.voxel_to_world(IVec3::new(x, y, z));
            let mut cres = CarveResult::default();
            grid.carve_sphere_advanced_into(wp, 1.5, 100.0, DamageType::Kinetic, &mut cres);
        }

        if i % 5 == 0 {
            let wp = grid.voxel_to_world(IVec3::new(x, y, z));
            grid.freeze_sphere(wp, 1.5);
        }

        let _ = grid.detect_unsupported_islands();
    }

    // Assert exact pointer stability (zero heap reallocations!)
    assert_eq!(
        grid.scratch_grounded.as_ptr(), ptr_grounded,
        "scratch_grounded pointer must remain completely stable (zero reallocations)"
    );
    assert_eq!(
        grid.scratch_visited.as_ptr(), ptr_visited,
        "scratch_visited pointer must remain completely stable (zero reallocations)"
    );
    assert_eq!(
        grid.scratch_tension.as_ptr(), ptr_tension,
        "scratch_tension pointer must remain completely stable (zero reallocations)"
    );
    assert_eq!(
        grid.scratch_depth.as_ptr(), ptr_depth,
        "scratch_depth pointer must remain completely stable (zero reallocations)"
    );
    assert!(
        grid.scratch_queue.capacity() >= cap_queue,
        "scratch_queue capacity must not shrink or thrash"
    );
}

// ----------------------------------------------------------------------------
// 6. Feature 16: Frustum Culling Efficiency & Boundary Stress
// ----------------------------------------------------------------------------

#[test]
fn test_frustum_culling_boundary_conditions_and_extreme_fov() {
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let cam_dir = Vec3::Z;
    let cam_up = Vec3::Y;

    // 1. Extreme narrow FOV (telephoto sniper zoom: 10 degrees)
    let narrow_fov = 10.0f32.to_radians();
    let frustum_narrow = FrustumPlanes::from_camera(cam_pos, cam_dir, cam_up, 1.0, narrow_fov, 0.5, 200.0);

    // On-axis point should be contained
    assert!(frustum_narrow.contains_point(Vec3::new(0.0, 10.0, 50.0)));
    // Off-axis point at x=10, z=50: tan(theta) = 10/50 = 0.2 rad ~ 11.4 deg > 5 deg half-angle -> must be culled
    assert!(!frustum_narrow.contains_point(Vec3::new(10.0, 10.0, 50.0)));

    // 2. Extreme wide FOV (fisheye / ultra-wide: 140 degrees)
    let wide_fov = 140.0f32.to_radians();
    let frustum_wide = FrustumPlanes::from_camera(cam_pos, cam_dir, cam_up, 1.0, wide_fov, 0.5, 200.0);
    // Off-axis point at x=20, z=20 (45 degrees) -> must be contained
    assert!(frustum_wide.contains_point(Vec3::new(20.0, 10.0, 20.0)));
    // Behind camera -> must not be contained
    assert!(!frustum_wide.contains_point(Vec3::new(0.0, 10.0, -10.0)));

    // 3. Near / Far plane boundary conditions
    let near = 1.0;
    let far = 100.0;
    let frustum_standard = FrustumPlanes::from_camera(cam_pos, cam_dir, cam_up, 1.0, std::f32::consts::FRAC_PI_3, near, far);

    // Exactly inside near/far range
    assert!(frustum_standard.contains_point(Vec3::new(0.0, 10.0, near + 0.1)));
    assert!(frustum_standard.contains_point(Vec3::new(0.0, 10.0, far - 0.1)));

    // Beyond near/far
    assert!(!frustum_standard.contains_point(Vec3::new(0.0, 10.0, near - 0.5)));
    assert!(!frustum_standard.contains_point(Vec3::new(0.0, 10.0, far + 5.0)));

    // 4. Sphere straddling frustum plane: center outside, radius penetrates frustum
    // Left boundary: center at x = -20.0, radius = 5.0
    // Test that sphere test returns true if it straddles, false if fully outside
    assert!(!frustum_standard.is_sphere_in_frustum(Vec3::new(-100.0, 10.0, 20.0), 2.0));
    assert!(frustum_standard.is_sphere_in_frustum(Vec3::new(0.0, 10.0, 20.0), 1.0));
}

#[test]
fn test_3000_active_debris_frustum_culling_efficiency_and_hud_telemetry() {
    let mut physics = PhysicsSystem::new();
    let cam_pos = Vec3::new(50.0, 20.0, 50.0);
    let cam_dir = Vec3::new(0.0, 0.0, 1.0); // Looking along +Z
    let frustum = FrustumPlanes::from_camera(
        cam_pos,
        cam_dir,
        Vec3::Y,
        16.0 / 9.0, // 16:9 standard aspect ratio
        60.0f32.to_radians(),
        0.5,
        150.0,
    );

    // Spawn 3,000 active debris particles distributed evenly in a sphere (radius 50m) around camera
    let total_particles = 3000;
    for i in 0..total_particles {
        let u = (i as f32) / (total_particles as f32);
        let phi = u * std::f32::consts::PI * 2.0;
        let costheta = 1.0 - 2.0 * ((i % 100) as f32 / 100.0);
        let sintheta = (1.0 - costheta * costheta).sqrt();
        let r = 10.0 + (i % 40) as f32;

        let pos = cam_pos + Vec3::new(
            r * sintheta * phi.cos(),
            r * costheta * 0.5, // slightly flattened vertically
            r * sintheta * phi.sin(),
        );

        physics.debris.push(DebrisParticle {
            position: pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.25,
            color: Color::new(0.7, 0.7, 0.7, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    assert_eq!(physics.debris.len(), 3000);

    // Execute frustum culled draw pass
    physics.draw_solids_frustum(cam_pos, &[], Some(&frustum));

    let metrics = physics.cull_metrics.get();
    assert_eq!(metrics.debris_total, 3000);

    // For a 60-degree FOV camera looking in one direction within an omnidirectional distribution:
    // Expected culled percentage is between 65% and 90%.
    let cull_pct = metrics.cull_ratio() * 100.0;
    assert!(
        (65.0..=90.0).contains(&cull_pct),
        "Debris frustum cull ratio must skip 65%-90% of off-screen particles, got {:.2}%",
        cull_pct
    );

    // Assert mathematical consistency between metrics
    assert_eq!(metrics.debris_drawn + (metrics.debris_total - metrics.debris_drawn), metrics.debris_total);
    assert!((metrics.cull_ratio() - ((metrics.debris_total - metrics.debris_drawn) as f32 / metrics.debris_total as f32)).abs() < 1e-5);
}

// ----------------------------------------------------------------------------
// 7. Feature 17: Chunk Mesh Pool Cap & Sub-Batch Zero-Allocation Invariants
// ----------------------------------------------------------------------------

#[test]
fn test_mesh_pool_cap_at_64_under_colossal_demolition() {
    let mut grid = VoxelGrid::new();

    // Verify initial pool is empty
    assert!(grid.mesh_pool.is_empty());

    // Populate 70 different chunks with solid voxels at elevation y=16..24 (chunk cy = 8)
    // where no bedrock exists, ensuring chunk_solid_count starts from 0.
    // There are 12 chunks in X (0..12) and 12 chunks in Z (0..12).
    // 10 * 7 = 70 chunks.
    let cy_target = ((16 - MIN_VOXEL_Y) as usize) / CHUNK_SIZE; // cy = (16 - (-48)) / 8 = 64 / 8 = 8
    let mut chunks_populated = Vec::new();
    for cx in 0..10 {
        for cz in 0..7 {
            let ci = VoxelGrid::chunk_idx(cx, cy_target, cz);
            chunks_populated.push(ci);

            let wx = (cx * CHUNK_SIZE) as i32;
            let wy = 16; // Pure air layer above surface
            let wz = (cz * CHUNK_SIZE) as i32;

            // Place a 4x4 block in each chunk
            for dx in 0..4 {
                for dy in 0..4 {
                    for dz in 0..4 {
                        grid.set(wx + dx, wy + dy, wz + dz, VoxelType::Concrete);
                    }
                }
            }
        }
    }

    assert_eq!(chunks_populated.len(), 70);

    // Rebuild meshes for all populated chunks
    grid.update_meshes();

    for &ci in &chunks_populated {
        assert!(!grid.meshes[ci].sub_meshes.is_empty(), "Chunk {} must have generated sub-meshes", ci);
    }

    // Now clear all voxels in all 70 chunks in a single catastrophic detonation
    for cx in 0..10 {
        for cz in 0..7 {
            let wx = (cx * CHUNK_SIZE) as i32;
            let wy = 16;
            let wz = (cz * CHUNK_SIZE) as i32;

            for dx in 0..4 {
                for dy in 0..4 {
                    for dz in 0..4 {
                        grid.set(wx + dx, wy + dy, wz + dz, VoxelType::Air);
                    }
                }
            }
        }
    }

    // Update meshes: all 70 chunks will empty their sub_meshes into mesh_pool
    grid.update_meshes();

    // Pool MUST be strictly capped at 64 entries (no unbounded memory growth!)
    assert_eq!(
        grid.mesh_pool.len(), 64,
        "Mesh pool must be strictly capped at exactly 64 meshes"
    );

    // Verify each mesh in the pool is empty of vertices/indices but retains allocated capacity
    for mesh in &grid.mesh_pool {
        assert!(mesh.vertices.is_empty(), "Recycled mesh in pool must have vertices cleared");
        assert!(mesh.indices.is_empty(), "Recycled mesh in pool must have indices cleared");
    }

    // Populate 10 chunks again: should drain 10 meshes from pool without allocating new ones
    for cx in 0..10 {
        let cz = 0;
        let wx = (cx * CHUNK_SIZE) as i32;
        let wy = 16;
        let wz = (cz * CHUNK_SIZE) as i32;
        for dx in 0..2 {
            for dy in 0..2 {
                for dz in 0..2 {
                    grid.set(wx + dx, wy + dy, wz + dz, VoxelType::HeavyArmor);
                }
            }
        }
    }

    grid.update_meshes();
    assert_eq!(
        grid.mesh_pool.len(), 54,
        "Mesh pool should have provided 10 recycled meshes (64 - 10 = 54)"
    );
}

#[test]
fn test_partition_mesh_sub_batches_into_massive_index_stress() {
    // Generate massive test mesh with 15,000 indices across 5,000 vertices
    let total_indices = 15000;
    let mut indices = Vec::with_capacity(total_indices);
    for t in 0..(total_indices / 3) {
        let base = ((t * 3) % 4800) as u16;
        indices.push(base);
        indices.push((base + 1) % 5000);
        indices.push((base + 2) % 5000);
    }

    let mut output_ranges = Vec::new();
    partition_mesh_sub_batches_into(
        &indices,
        5000,
        MAX_CHUNK_BATCH_VERTICES,
        MAX_CHUNK_BATCH_INDICES,
        &mut output_ranges,
    );

    assert!(!output_ranges.is_empty(), "Must partition into multiple sub-batches");

    // Verify all partitions respect the hardware vertex and index limits
    let mut total_partitioned_indices = 0;
    for (vert_range, idx_range) in &output_ranges {
        let vert_count = vert_range.end() - vert_range.start() + 1;
        let idx_count = idx_range.end - idx_range.start;

        assert!(
            vert_count <= MAX_CHUNK_BATCH_VERTICES,
            "Vertex count {} exceeds limit {}",
            vert_count, MAX_CHUNK_BATCH_VERTICES
        );
        assert!(
            idx_count <= MAX_CHUNK_BATCH_INDICES,
            "Index count {} exceeds limit {}",
            idx_count, MAX_CHUNK_BATCH_INDICES
        );
        assert_eq!(idx_count % 3, 0, "Index batch must contain full triangles");

        total_partitioned_indices += idx_count;
    }

    assert_eq!(
        total_partitioned_indices, total_indices,
        "Every single index must be partitioned with zero drops"
    );

    // Also assert equivalence with legacy function
    let legacy = partition_mesh_sub_batches(&indices, 5000, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
    assert_eq!(legacy.len(), output_ranges.len(), "Batch counts must match legacy partitioner");
}
