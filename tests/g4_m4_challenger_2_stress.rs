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
    DebrisParticle, DecalKind, DynamicChunk, ElectricalArc, ImpactDecal, PhysicsSystem,
    PressurizedFireJet, ShockwaveRing, SmokeParticle, SparkParticle,
};
use voxel::{
    FrustumPlanes, VoxelGrid, VoxelType,
    CHUNK_SIZE,
    MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES,
    partition_mesh_sub_batches_into,
};

// ============================================================================
// EMPIRICAL CHALLENGER ADVERSARIAL STRESS TEST SUITE: MILESTONE G4-M4
// Agent: g4_m4_challenger_2 (Adversarial Robustness Challenger)
//
// Target Features:
// - Feature 15: Multi-Block Dirty Unions, Tensile Rebar Boundary Bridges,
//               Boundary Fallback Invariants
// - Feature 16: Dynamic Chunk Culling, 6-Plane Frustum Particle/Decal Culling,
//               Radial Particle 60%-85% Efficiency Invariant
// - Feature 17: Rapid Mesh Pool Carving (Cap <= 64), Sub-Batch Remapping,
//               100-Tick Inner-Loop Scratch Pointer Stability
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Feature 15: Multi-Block Dirty Unions & Disjoint Structural Evaluation
// ----------------------------------------------------------------------------

#[test]
fn test_multiblock_dirty_unions_disjoint_carves_and_structural_evaluation() {
    let mut grid = VoxelGrid::new();

    // Tower A column at (20, 0..30, 20)
    for y in 0..=30 {
        grid.set(20, y, 20, VoxelType::Concrete);
    }
    // Tower B column at (70, 0..30, 70)
    for y in 0..=30 {
        grid.set(70, y, 70, VoxelType::Concrete);
    }

    // Baseline: fully grounded, 0 islands
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Grounded towers must produce 0 islands");
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // Disjoint mutation: sever Tower A base at y=1..2
    grid.set(20, 1, 20, VoxelType::Air);
    grid.set(20, 2, 20, VoxelType::Air);

    // Disjoint mutation: sever Tower B base at y=1..2 in the same frame
    grid.set(70, 1, 70, VoxelType::Air);
    grid.set(70, 2, 70, VoxelType::Air);

    // Verify union bounding box spans both distant sites
    let d_min = grid.dirty_min.expect("Dirty min must be set");
    let d_max = grid.dirty_max.expect("Dirty max must be set");
    assert!(d_min.x <= 20 && d_min.z <= 20);
    assert!(d_max.x >= 70 && d_max.z >= 70);

    // BFS evaluation over wide bounding box union: BOTH towers must collapse!
    let islands = grid.detect_unsupported_islands();
    let total_collapsed: usize = islands.iter().map(|c| c.voxels.len()).sum();
    // Tower A had y=3..=30 (28 voxels) floating, Tower B had y=3..=30 (28 voxels) floating = 56 voxels
    assert_eq!(
        total_collapsed, 56,
        "Disjoint multi-block dirty union must collapse both severed towers (got {}, expected 56)",
        total_collapsed
    );

    // Verify both crowns are now Air
    assert_eq!(grid.get(20, 30, 20), VoxelType::Air);
    assert_eq!(grid.get(70, 30, 70), VoxelType::Air);

    // Now test asymmetric severance: rebuild Tower B, sever only Tower B, Tower A remains Air
    for y in 0..=20 {
        grid.set(70, y, 70, VoxelType::Concrete);
    }
    let _ = grid.detect_unsupported_islands(); // flush

    // Sever only Tower B
    grid.set(70, 1, 70, VoxelType::Air);
    let d_min = grid.dirty_min.unwrap();
    let d_max = grid.dirty_max.unwrap();
    // Bounding box must be strictly localized to Tower B!
    assert_eq!(d_min.x, 70);
    assert_eq!(d_max.x, 70);

    let islands = grid.detect_unsupported_islands();
    let collapsed: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(collapsed, 19, "Only Tower B crown (y=2..=20) should collapse");
}

// ----------------------------------------------------------------------------
// 2. Feature 15: Tensile Rebar Boundary Bridge Across Sub-Region Boundary
// ----------------------------------------------------------------------------

#[test]
fn test_tensile_rebar_boundary_bridge_cross_subregion_suspension() {
    let mut grid = VoxelGrid::new();

    // Anchor Tower A at x=24..=26, z=25 from y=0 to y=35
    for x in 24..=26 {
        for y in 0..=35 {
            grid.set(x, y, 25, VoxelType::Concrete);
        }
    }

    // Anchor Tower B at x=54..=56, z=25 from y=0 to y=35
    for x in 54..=56 {
        for y in 0..=35 {
            grid.set(x, y, 25, VoxelType::Concrete);
        }
    }

    // Aerial skybridge made of SteelRebar at y=30 from x=27 to x=53 (spanning 27 voxels)
    for x in 27..=53 {
        grid.set(x, 30, 25, VoxelType::SteelRebar);
    }

    // Verify baseline structure is grounded
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Anchored rebar bridge must have 0 islands");

    // Scenario 1: Inflict localized damage at (27, 30, 25) (sever connection to Tower A)
    // Subregion bounds around x=27 with pad=8 will be [19..=35].
    // Tower B (x=54..56) is completely OUTSIDE the sub-region!
    // The bridge spans from x=28 through x=35 (sub-region boundary) towards Tower B.
    grid.set(27, 30, 25, VoxelType::Air); // detach bridge from Tower A!

    // The bridge from x=28..=53 is now anchored ONLY by Tower B outside the sub-region.
    // The sub-region BFS will encounter the boundary at x=35.
    // Boundary fallback MUST trigger and verify that the rebar bridge is grounded via Tower B!
    let islands = grid.detect_unsupported_islands();

    // Since the bridge is continuous SteelRebar anchored to grounded Tower B,
    // rebar-to-rebar tension allows it to remain suspended!
    // None of the bridge voxels should collapse!
    let collapsed: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(
        collapsed, 0,
        "Rebar skybridge anchored to Tower B outside the sub-region must NOT collapse upon boundary fallback"
    );

    // Verify rebar bridge voxels x=28..=53 are still intact in the grid
    for x in 28..=53 {
        assert_eq!(
            grid.get(x, 30, 25),
            VoxelType::SteelRebar,
            "Rebar voxel at x={} must remain intact",
            x
        );
    }

    // Scenario 2: Now sever the bridge in the middle (x=40)
    grid.set(40, 30, 25, VoxelType::Air);

    let islands = grid.detect_unsupported_islands();
    // The segment x=28..=39 is now completely severed (not connected to Tower A nor Tower B).
    // It must collapse!
    let collapsed_voxels: usize = islands.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(
        collapsed_voxels, 12,
        "Severed rebar segment x=28..=39 (12 voxels) must collapse, got {}",
        collapsed_voxels
    );

    // Verify x=28..=39 are now Air
    for x in 28..=39 {
        assert_eq!(grid.get(x, 30, 25), VoxelType::Air);
    }

    // The segment x=41..=53 is connected to Tower B (length 13 cantilever).
    // Since rebar grounded at depth 0 spans horizontally at depth 0, it remains anchored to Tower B!
    for x in 41..=53 {
        assert_eq!(grid.get(x, 30, 25), VoxelType::SteelRebar);
    }
}

// ----------------------------------------------------------------------------
// 3. Feature 15: Boundary Fallback Island Retention Invariance
// ----------------------------------------------------------------------------

#[test]
fn test_multiblock_dirty_union_island_extraction_under_boundary_fallback() {
    let mut grid = VoxelGrid::new();
    grid.set_preserve_single_voxel_debris(true);

    // Grounded pillar at x=50, y=0..20, z=30
    for y in 0..=20 {
        grid.set(50, y, 30, VoxelType::Concrete);
    }

    // Structure 1: Floating 2x2 concrete block at (20..21, 15, 30..31)
    // Completely ungrounded, entirely inside sub-region
    for x in 20..=21 {
        for z in 30..=31 {
            grid.set(x, 15, z, VoxelType::Concrete);
        }
    }

    // Structure 2: Arm extending from x=42 to x=50 at y=15, z=30
    // Connected to grounded pillar at x=50
    for x in 42..=49 {
        grid.set(x, 15, 30, VoxelType::Concrete);
    }

    // Mark dirty region covering Structure 1 and arm root: x=20..38
    // With pad=8, box_max_x is 38 + 8 = 46.
    // The pillar at x=50 is OUTSIDE the sub-region!
    // Structure 1 (x=20..21) is an island.
    // Structure 2 (x=42..46) will touch x=47 outside the sub-region, triggering boundary fallback!
    grid.mark_dirty_region(IVec3::new(20, 15, 30), IVec3::new(38, 15, 30));

    let islands = grid.detect_unsupported_islands();

    // Verify Structure 1 (4 voxels) was NOT lost during boundary fallback!
    let structure1_collapsed = islands
        .iter()
        .flat_map(|c| c.voxels.iter())
        .filter(|(p, _)| p.x >= 20 && p.x <= 21 && p.y == 15)
        .count();

    assert_eq!(
        structure1_collapsed, 4,
        "Structure 1 voxels must be successfully extracted as an island despite boundary fallback on Structure 2"
    );

    // Structure 1 voxels must now be Air in the grid
    for x in 20..=21 {
        for z in 30..=31 {
            assert_eq!(grid.get(x, 15, z), VoxelType::Air);
        }
    }
}

// ----------------------------------------------------------------------------
// 4. Feature 16: Dynamic Chunk Culling & Large Bounding Sphere Conservatism
// ----------------------------------------------------------------------------

#[test]
fn test_dynamic_chunk_culling_all_orientations_and_large_bounding_spheres() {
    let mut physics = PhysicsSystem::new();
    let cam_pos = Vec3::new(0.0, 10.0, 0.0);
    let cam_dir = Vec3::Z;
    let frustum = FrustumPlanes::from_camera(
        cam_pos,
        cam_dir,
        Vec3::Y,
        1.0,
        std::f32::consts::FRAC_PI_3, // 60 deg FOV
        0.5,
        80.0,
    );

    // 1. Chunk in front of camera (+Z, dist 25m) -> MUST BE DRAWN
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 10.0, 25.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 2. Chunk behind camera (-Z, dist 25m) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 10.0, -25.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 3. Chunk far left (-X, dist 40m) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(-40.0, 10.0, 20.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 4. Chunk far right (+X, dist 40m) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(40.0, 10.0, 20.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 5. Chunk far above (+Y, dist 40m) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 50.0, 20.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 6. Chunk far below (-Y, dist 40m) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, -30.0, 20.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 7. Chunk beyond far plane (Z = 120m > 80m far plane) -> MUST BE CULLED
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(0.0, 10.0, 120.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::Concrete)],
        50.0,
        0.0,
        false,
        (Vec3::splat(-0.25), Vec3::splat(0.25)),
    ));

    // 8. Large Chunk Bounding Sphere Conservatism Test:
    // Chunk origin is at X = 20.0, Z = 25.0 (outside 60 deg cone where tan(30 deg)*25 = 14.43m).
    // BUT local AABB is +/- 6.0m (radius ~8.5m + VOXEL_SIZE = 9.0m).
    // Because the bounding sphere extends from 20 - 9 = 11m, it crosses into the frustum cone!
    // The frustum sphere check MUST conserve this chunk and DRAW it (preventing premature popping!).
    physics.chunks.push(DynamicChunk::new(
        Vec3::new(18.0, 10.0, 25.0),
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::ZERO,
        vec![(Vec3::ZERO, VoxelType::HeavyArmor)],
        500.0,
        0.0,
        false,
        (Vec3::splat(-6.0), Vec3::splat(6.0)),
    ));

    assert_eq!(physics.chunks.len(), 8);

    physics.draw_solids_frustum(cam_pos, &[], Some(&frustum));
    let metrics = physics.cull_metrics.get();

    assert_eq!(metrics.chunks_total, 8);
    // Exactly 2 chunks should be drawn: chunk 1 (direct front) and chunk 8 (large bounding sphere overlap)
    assert_eq!(
        metrics.chunks_drawn, 2,
        "Expected exactly 2 dynamic chunks drawn (1 front + 1 overlapping large sphere), got {}",
        metrics.chunks_drawn
    );

    let cull_ratio = metrics.cull_ratio();
    assert!(
        (cull_ratio - 0.75).abs() < 1e-4,
        "Dynamic chunk cull ratio should be 75%, got {:.2}",
        cull_ratio
    );
}

// ----------------------------------------------------------------------------
// 5. Feature 16: Frustum Culling Radial Efficiency Across All Systems
// ----------------------------------------------------------------------------

#[test]
fn test_frustum_culling_efficiency_radial_particle_distribution_full_spectrum() {
    let mut physics = PhysicsSystem::new();
    let cam_pos = Vec3::new(0.0, 15.0, 0.0);
    let cam_dir = Vec3::Z;
    let frustum = FrustumPlanes::from_camera(
        cam_pos,
        cam_dir,
        Vec3::Y,
        1.0,
        std::f32::consts::FRAC_PI_3, // 60 deg FOV
        0.5,
        70.0,
    );

    let count_per_system = 300;
    let radius = 25.0;

    for i in 0..count_per_system {
        let angle = (i as f32 / count_per_system as f32) * std::f32::consts::TAU;
        let pos = cam_pos + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);

        // 1. Sparks
        physics.sparks.push(SparkParticle {
            position: pos,
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 1.0,
            max_life: 1.0,
        });

        // 2. Smoke
        physics.smoke.push(SmokeParticle {
            position: pos,
            velocity: Vec3::ZERO,
            size: 0.6,
            start_size: 0.6,
            max_size: 1.2,
            color: Color::new(0.5, 0.5, 0.5, 0.3),
            life: 0.5,
            max_life: 1.0,
        });

        // 3. Shockwaves
        physics.shockwaves.push(ShockwaveRing {
            position: pos,
            normal: Vec3::Y,
            radius: 1.0,
            max_radius: 3.0,
            speed: 4.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thickness: 0.2,
        });

        // 4. Arcs
        physics.electrical_arcs.push(ElectricalArc {
            start: pos - Vec3::X,
            end: pos + Vec3::X,
            color: Color::new(0.2, 0.7, 1.0, 1.0),
            timer: 0.5,
            max_timer: 0.5,
        });

        // 5. Fire Jets
        physics.fire_jets.push(PressurizedFireJet {
            position: pos,
            direction: Vec3::Y,
            timer: 0.5,
            max_timer: 0.5,
        });

        // 6. Decals
        physics.decals.push(ImpactDecal {
            position: pos,
            normal: Vec3::Y,
            size: 0.8,
            color: Color::new(0.2, 0.2, 0.2, 1.0),
            kind: DecalKind::Scorch,
            life: 5.0,
            max_life: 5.0,
            voxel_anchor: None,
        });

        // 7. Debris
        physics.debris.push(DebrisParticle {
            position: pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.25,
            color: Color::new(0.8, 0.8, 0.8, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    // Call all frustum-culled rendering passes
    physics.draw_solids_frustum(cam_pos, &[], Some(&frustum));
    physics.draw_particles_frustum(cam_pos, &[], Some(&frustum));
    physics.draw_decals_frustum(cam_pos, &[], Some(&frustum));

    let metrics = physics.cull_metrics.get();

    // Verify all 7 systems were populated
    assert_eq!(metrics.sparks_total, count_per_system);
    assert_eq!(metrics.smoke_total, count_per_system);
    assert_eq!(metrics.shockwaves_total, count_per_system);
    assert_eq!(metrics.arcs_total, count_per_system);
    assert_eq!(metrics.fire_total, count_per_system);
    assert_eq!(metrics.decals_total, count_per_system);
    assert_eq!(metrics.debris_total, count_per_system);

    // Each individual system must have some drawn and some culled
    assert!(metrics.sparks_drawn > 0 && metrics.sparks_drawn < count_per_system);
    assert!(metrics.smoke_drawn > 0 && metrics.smoke_drawn < count_per_system);
    assert!(metrics.shockwaves_drawn > 0 && metrics.shockwaves_drawn < count_per_system);
    assert!(metrics.arcs_drawn > 0 && metrics.arcs_drawn < count_per_system);
    assert!(metrics.fire_drawn > 0 && metrics.fire_drawn < count_per_system);
    assert!(metrics.decals_drawn > 0 && metrics.decals_drawn < count_per_system);
    assert!(metrics.debris_drawn > 0 && metrics.debris_drawn < count_per_system);

    // Mathematical identity check: total == drawn + culled
    let total_entities = metrics.particles_total() + metrics.chunks_total + metrics.decals_total;
    let drawn_entities = metrics.particles_drawn() + metrics.chunks_drawn + metrics.decals_drawn;
    assert_eq!(total_entities, count_per_system * 7);

    // 60-degree horizontal FOV on a 360-degree ring must produce 60%-85% culling ratio!
    let cull_percentage = metrics.cull_ratio() * 100.0;
    assert!(
        (60.0..=85.0).contains(&cull_percentage),
        "Full spectrum 360-degree radial ring cull ratio must be between 60% and 85%, got {:.2}%",
        cull_percentage
    );
    assert_eq!(total_entities - drawn_entities, (total_entities as f32 * metrics.cull_ratio()).round() as usize);
}

// ----------------------------------------------------------------------------
// 6. Feature 17: Rapid Mesh Pool Carving (Cap <= 64) & Sub-Batch Partitioning
// ----------------------------------------------------------------------------

#[test]
fn test_rapid_mesh_pool_carving_and_cap_strict_invariance() {
    let mut grid = VoxelGrid::new();

    // Verify initial pool is empty
    assert!(grid.mesh_pool.is_empty());

    // Run 5 rapid carving and rebuilding cycles across 80 surface chunks above bedrock (y=8..16)
    // Chunk cy=7 corresponds to y=8..16 (safe above y=0 bedrock!)
    let _cy = 7;
    let wy = 8;

    for cycle in 0..5 {
        // Step A: Populate 80 chunks with dense concrete structures
        for cx in 0..10 {
            for cz in 0..8 {
                let wx = (cx * CHUNK_SIZE) as i32;
                let wz = (cz * CHUNK_SIZE) as i32;
                for dx in 1..=4 {
                    for dy in 1..=4 {
                        for dz in 1..=4 {
                            grid.set(wx + dx, wy + dy, wz + dz, VoxelType::Concrete);
                        }
                    }
                }
            }
        }

        // Rebuild meshes
        grid.update_meshes();

        // Step B: Rapidly carve out all 80 chunks in a violent blast
        for cx in 0..10 {
            for cz in 0..8 {
                let wx = (cx * CHUNK_SIZE) as i32;
                let wz = (cz * CHUNK_SIZE) as i32;
                for dx in 1..=4 {
                    for dy in 1..=4 {
                        for dz in 1..=4 {
                            grid.set(wx + dx, wy + dy, wz + dz, VoxelType::Air);
                        }
                    }
                }
            }
        }

        // Update meshes: empty chunks return their sub_meshes to mesh_pool
        grid.update_meshes();

        // STRICT INVARIANT: Mesh pool must never exceed 64 entries!
        assert!(
            grid.mesh_pool.len() <= 64,
            "Cycle {}: Mesh pool length {} must be <= 64",
            cycle,
            grid.mesh_pool.len()
        );
        assert_eq!(
            grid.mesh_pool.len(), 64,
            "Cycle {}: With 80 chunks cleared, mesh pool must be exactly at full cap 64",
            cycle
        );

        // Verify all recycled meshes in the pool have empty vertices and indices (ready for clean reuse)
        for mesh in &grid.mesh_pool {
            assert!(mesh.vertices.is_empty());
            assert!(mesh.indices.is_empty());
        }
    }

    // Step C: Sub-batch partition stress with 12,000 indices across 5,000 vertices
    let mut large_indices = Vec::with_capacity(12000);
    for t in 0..4000u16 {
        let base = (t * 2) % 4500;
        large_indices.push(base);
        large_indices.push(base + 1);
        large_indices.push(base + 2);
    }

    let mut scratch_partitions = Vec::new();
    partition_mesh_sub_batches_into(
        &large_indices,
        5000,
        MAX_CHUNK_BATCH_VERTICES,
        MAX_CHUNK_BATCH_INDICES,
        &mut scratch_partitions,
    );

    assert!(scratch_partitions.len() >= 3, "12,000 indices must produce at least 3 sub-batches");

    for (vert_range, idx_range) in &scratch_partitions {
        assert!(vert_range.end() - vert_range.start() <= MAX_CHUNK_BATCH_VERTICES);
        assert!(idx_range.len() <= MAX_CHUNK_BATCH_INDICES);
    }
}

// ----------------------------------------------------------------------------
// 7. Feature 17: Physics Inner Loop Pools 100-Tick Pointer Stability
// ----------------------------------------------------------------------------

#[test]
fn test_physics_inner_loop_pools_capacity_and_pointer_stability_100_ticks() {
    let mut physics = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let cam_pos = Vec3::new(30.0, 15.0, 30.0);

    // Initial capacity assertions
    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);
    assert!(physics.scratch_fracture_sub_chunks.capacity() >= 32);
    assert!(physics.scratch_blasted_settled_chunks.capacity() >= 32);

    let ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();
    let ptr_settle = physics.scratch_chunks_to_settle.as_ptr();
    let ptr_new_crush = physics.scratch_new_crush_chunks.as_ptr();
    let ptr_fracture = physics.scratch_fracture_sub_chunks.as_ptr();
    let ptr_blasted = physics.scratch_blasted_settled_chunks.as_ptr();

    // Spawn 10 dynamic chunks at high downward velocity to trigger crushing and settling
    for i in 0..10 {
        let pos = Vec3::new(25.0 + (i as f32 * 1.5), 15.0, 25.0 + (i as f32 * 1.5));
        let voxels = vec![
            (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::SteelRebar),
            (Vec3::new(0.0, 0.5, 0.0), VoxelType::Concrete),
            (Vec3::new(0.0, 0.0, 0.5), VoxelType::Concrete),
        ];
        physics.chunks.push(DynamicChunk::new(
            pos,
            Vec3::new(0.0, -25.0, 0.0), // High downward speed
            Vec3::ZERO,
            Vec3::new(1.0, 2.0, 0.5),
            voxels,
            200.0,
            0.0,
            false,
            (Vec3::splat(-0.5), Vec3::splat(0.5)),
        ));
    }

    // Run 100 simulation ticks
    for tick in 1..=100 {
        // Intermittently inject craters and rubble un-settling
        if tick % 20 == 0 {
            physics.un_settle_rubble_crater(Vec3::new(30.0, 0.0, 30.0), 5.0, 250.0);
        }

        let _ = physics.update(0.016, &mut grid, &audio, cam_pos);
    }

    // Verify capacities are preserved and pointer addresses remain 100% stable
    assert_eq!(
        physics.scratch_craters_to_unsettle.as_ptr(),
        ptr_craters,
        "scratch_craters_to_unsettle pointer must remain stable across 100 ticks"
    );
    assert_eq!(
        physics.scratch_chunks_to_settle.as_ptr(),
        ptr_settle,
        "scratch_chunks_to_settle pointer must remain stable across 100 ticks"
    );
    assert_eq!(
        physics.scratch_new_crush_chunks.as_ptr(),
        ptr_new_crush,
        "scratch_new_crush_chunks pointer must remain stable across 100 ticks"
    );
    assert_eq!(
        physics.scratch_fracture_sub_chunks.as_ptr(),
        ptr_fracture,
        "scratch_fracture_sub_chunks pointer must remain stable across 100 ticks"
    );
    assert_eq!(
        physics.scratch_blasted_settled_chunks.as_ptr(),
        ptr_blasted,
        "scratch_blasted_settled_chunks pointer must remain stable across 100 ticks"
    );

    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);
    assert!(physics.scratch_fracture_sub_chunks.capacity() >= 32);
    assert!(physics.scratch_blasted_settled_chunks.capacity() >= 32);
}
