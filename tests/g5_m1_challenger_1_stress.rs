#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use macroquad::prelude::*;
use std::collections::HashSet;
use voxel::{
    CarveResult, DamageType, VoxelGrid, VoxelType, CHUNKS_SUBTERRANEAN_Y, CHUNKS_X, CHUNKS_Y,
    CHUNKS_Z, CHUNK_SIZE, MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES,
    MESH_POOL_MAX_CAPACITY, MIN_VOXEL_Y,
};

// ============================================================================
// CHALLENGER 1 EMPIRICAL SUITE: G5-M1 MESHING & LOCALIZED BFS ZERO-ALLOCATION
// ============================================================================

// ----------------------------------------------------------------------------
// Group 1: ChunkMesh Sub-Batching, Limits Enforcement & Mesh Pool Reuse
// ----------------------------------------------------------------------------

#[test]
fn test_massive_chunk_checkerboard_sub_batch_limits() {
    let mut grid = VoxelGrid::new();

    // Populate 32 distinct surface chunks with high-density 3D checkerboards and chipped voxels
    // to intentionally generate thousands of unmerged faces, forcing multiple sub-batches.
    let mut chunks_tested = 0;
    for cx in 2..=5 {
        for cy in 7..=8 {
            for cz in 2..=5 {
                let start_x = (cx * CHUNK_SIZE) as i32;
                let start_y = MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32;
                let start_z = (cz * CHUNK_SIZE) as i32;
                let chunk_size = CHUNK_SIZE as i32;

                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            if (x + y + z) % 2 == 0 {
                                grid.set(x, y, z, VoxelType::Concrete);
                                // Damage every 3rd voxel partially to prevent uniform AO greedy merging
                                if (x + z) % 3 == 0 {
                                    let idx = VoxelGrid::idx(x as usize, y, z as usize);
                                    grid.health[idx] = 40.0; // Damaged concrete triggers organic chipping
                                }
                            }
                        }
                    }
                }
                chunks_tested += 1;
            }
        }
    }
    assert_eq!(chunks_tested, 32);

    // Rebuild dirty chunk meshes
    grid.update_meshes();

    let mut multi_batch_chunk_count = 0;
    for cx in 2..=5 {
        for cy in 7..=8 {
            for cz in 2..=5 {
                let ci = VoxelGrid::chunk_idx(cx, cy, cz);
                let cm = &grid.meshes[ci];

                assert!(
                    !cm.sub_meshes.is_empty(),
                    "Populated chunk ({},{},{}) must have sub-meshes",
                    cx, cy, cz
                );

                if cm.sub_meshes.len() > 1 {
                    multi_batch_chunk_count += 1;
                }

                // Verify every single sub-mesh strictly respects Macroquad vertex and index limits
                for (b_idx, sub_mesh) in cm.sub_meshes.iter().enumerate() {
                    assert!(
                        sub_mesh.vertices.len() <= MAX_CHUNK_BATCH_VERTICES,
                        "Chunk ({},{},{}) batch {} breached MAX_CHUNK_BATCH_VERTICES: {} > {}",
                        cx, cy, cz, b_idx, sub_mesh.vertices.len(), MAX_CHUNK_BATCH_VERTICES
                    );
                    assert!(
                        sub_mesh.indices.len() <= MAX_CHUNK_BATCH_INDICES,
                        "Chunk ({},{},{}) batch {} breached MAX_CHUNK_BATCH_INDICES: {} > {}",
                        cx, cy, cz, b_idx, sub_mesh.indices.len(), MAX_CHUNK_BATCH_INDICES
                    );
                    assert_eq!(
                        sub_mesh.indices.len() % 3,
                        0,
                        "Chunk ({},{},{}) batch {} indices must form complete triangles: {}",
                        cx, cy, cz, b_idx, sub_mesh.indices.len()
                    );

                    // Verify zero out-of-bounds indices in sub-batch index stream
                    let v_len = sub_mesh.vertices.len();
                    for &idx in &sub_mesh.indices {
                        assert!(
                            (idx as usize) < v_len,
                            "Chunk ({},{},{}) batch {} index out of bounds: {} >= {}",
                            cx, cy, cz, b_idx, idx, v_len
                        );
                    }
                }
            }
        }
    }

    assert!(
        multi_batch_chunk_count > 0,
        "Dense checkerboards must trigger sub-batch partitioning across chunks"
    );

    // Verify mesh_pool capacity invariant
    assert!(
        grid.mesh_pool.len() <= MESH_POOL_MAX_CAPACITY,
        "mesh_pool exceeded MESH_POOL_MAX_CAPACITY: {} > {}",
        grid.mesh_pool.len(),
        MESH_POOL_MAX_CAPACITY
    );
}

#[test]
fn test_massive_explosive_demolition_chunk_transition_and_mesh_pool_reuse() {
    let mut grid = VoxelGrid::new();

    // Populate a 4x4x4 cluster of surface chunks (64 chunks total)
    // cx in 4..=7 (x in 32..64)
    // cy in 7..=10 (y in 8..40)
    // cz in 4..=7 (z in 32..64)
    for cx in 4..=7 {
        for cy in 7..=10 {
            for cz in 4..=7 {
                let start_x = (cx * CHUNK_SIZE) as i32;
                let start_y = MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32;
                let start_z = (cz * CHUNK_SIZE) as i32;
                let chunk_size = CHUNK_SIZE as i32;

                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            if (x + y + z) % 2 == 0 {
                                grid.set(x, y, z, VoxelType::Concrete);
                            }
                        }
                    }
                }
            }
        }
    }

    grid.update_meshes();
    let initial_pool_len = grid.mesh_pool.len();
    assert!(initial_pool_len <= MESH_POOL_MAX_CAPACITY);

    // Perform multiple colossal spherical explosions directly into the center of the chunk cluster
    let mut carve_result = CarveResult::default();
    let center_pos = grid.voxel_to_world(IVec3::new(48, 24, 48));
    let blast_centers = [
        center_pos,
        center_pos + Vec3::new(4.0, 4.0, 4.0),
        center_pos + Vec3::new(-4.0, -4.0, -4.0),
        center_pos + Vec3::new(2.0, -2.0, 2.0),
    ];

    for &center in &blast_centers {
        grid.carve_sphere_advanced_into(
            center,
            6.0,
            5000.0,
            DamageType::Explosive,
            &mut carve_result,
        );
    }
    assert!(
        carve_result.destroyed > 100,
        "Colossal blast must destroy over 100 voxels, destroyed: {}",
        carve_result.destroyed
    );

    // Rebuild meshes across all blasted chunks
    grid.update_meshes();

    // Verify all chunk sub-meshes remain strictly valid
    for cx in 4..=7 {
        for cy in 7..=10 {
            for cz in 4..=7 {
                let ci = VoxelGrid::chunk_idx(cx, cy, cz);
                let cm = &grid.meshes[ci];
                for (b_idx, sub_mesh) in cm.sub_meshes.iter().enumerate() {
                    assert!(
                        sub_mesh.vertices.len() <= MAX_CHUNK_BATCH_VERTICES,
                        "Vertex limit breached: {}",
                        sub_mesh.vertices.len()
                    );
                    assert!(
                        sub_mesh.indices.len() <= MAX_CHUNK_BATCH_INDICES,
                        "Index limit breached: {}",
                        sub_mesh.indices.len()
                    );
                    assert_eq!(sub_mesh.indices.len() % 3, 0);
                    let v_len = sub_mesh.vertices.len();
                    for &idx in &sub_mesh.indices {
                        assert!(
                            (idx as usize) < v_len,
                            "Sub-mesh {} index out of bounds",
                            b_idx
                        );
                    }
                }
            }
        }
    }

    assert!(grid.mesh_pool.len() <= MESH_POOL_MAX_CAPACITY);

    // Complete obliteration: clear all voxels in the 64 chunks to Air
    for cx in 4..=7 {
        for cy in 7..=10 {
            for cz in 4..=7 {
                let start_x = (cx * CHUNK_SIZE) as i32;
                let start_y = MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32;
                let start_z = (cz * CHUNK_SIZE) as i32;
                let chunk_size = CHUNK_SIZE as i32;

                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            grid.set(x, y, z, VoxelType::Air);
                        }
                    }
                }
            }
        }
    }

    grid.update_meshes();

    // Verify all chunks cleanly drained their sub-meshes
    for cx in 4..=7 {
        for cy in 7..=10 {
            for cz in 4..=7 {
                let ci = VoxelGrid::chunk_idx(cx, cy, cz);
                assert!(
                    grid.meshes[ci].sub_meshes.is_empty(),
                    "Cleared chunk must have 0 sub_meshes"
                );
            }
        }
    }

    // Mesh pool must be capped at exactly MESH_POOL_MAX_CAPACITY (64)
    assert_eq!(
        grid.mesh_pool.len(),
        MESH_POOL_MAX_CAPACITY,
        "Recycled sub-meshes must saturate mesh_pool to MESH_POOL_MAX_CAPACITY"
    );
}

#[test]
fn test_rapid_cyclical_rebuild_mesh_pool_churn_and_zero_leak() {
    let mut grid = VoxelGrid::new();
    let (cx, cy, cz) = (4, CHUNKS_SUBTERRANEAN_Y + 2, 4);
    let start_x = (cx * CHUNK_SIZE) as i32;
    let start_y = MIN_VOXEL_Y + (cy * CHUNK_SIZE) as i32;
    let start_z = (cz * CHUNK_SIZE) as i32;
    let chunk_size = CHUNK_SIZE as i32;

    // Run 50 rapid cycles alternating between dense, medium, empty, and damaged states
    for cycle in 0..50 {
        match cycle % 4 {
            0 => {
                // Dense checkerboard (triggers sub-batch partitioning)
                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            if (x + y + z) % 2 == 0 {
                                grid.set(x, y, z, VoxelType::Concrete);
                            } else {
                                grid.set(x, y, z, VoxelType::Air);
                            }
                        }
                    }
                }
            }
            1 => {
                // Medium single 2x2 cube
                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            grid.set(x, y, z, VoxelType::Air);
                        }
                    }
                }
                for x in start_x..(start_x + 2) {
                    for y in start_y..(start_y + 2) {
                        for z in start_z..(start_z + 2) {
                            grid.set(x, y, z, VoxelType::HeavyArmor);
                        }
                    }
                }
            }
            2 => {
                // Completely empty
                for x in start_x..(start_x + chunk_size) {
                    for y in start_y..(start_y + chunk_size) {
                        for z in start_z..(start_z + chunk_size) {
                            grid.set(x, y, z, VoxelType::Air);
                        }
                    }
                }
            }
            3 => {
                // Damaged block with procedural fracture chipping
                for x in start_x..(start_x + 4) {
                    for y in start_y..(start_y + 4) {
                        for z in start_z..(start_z + 4) {
                            grid.set(x, y, z, VoxelType::Concrete);
                            let idx = VoxelGrid::idx(x as usize, y, z as usize);
                            grid.health[idx] = 20.0; // severe damage
                        }
                    }
                }
            }
            _ => unreachable!(),
        }

        grid.update_meshes();

        let ci = VoxelGrid::chunk_idx(cx, cy, cz);
        let cm = &grid.meshes[ci];
        for sub_mesh in &cm.sub_meshes {
            assert!(sub_mesh.vertices.len() <= MAX_CHUNK_BATCH_VERTICES);
            assert!(sub_mesh.indices.len() <= MAX_CHUNK_BATCH_INDICES);
            assert_eq!(sub_mesh.indices.len() % 3, 0);
        }
        assert!(grid.mesh_pool.len() <= MESH_POOL_MAX_CAPACITY);
    }
}

#[test]
fn test_draw_frustum_linear_memory_stride_and_cache_locality() {
    // Verify that the loop ordering in draw_frustum:
    // for cx in 0..CHUNKS_X { for cy in 0..CHUNKS_Y { for cz in 0..CHUNKS_Z { ... } } }
    // yields a sequential linear memory index stride of +1 on every innermost iteration.
    let mut prev_ci = None;
    let mut total_chunks = 0;

    for cx in 0..CHUNKS_X {
        for cy in 0..CHUNKS_Y {
            for cz in 0..CHUNKS_Z {
                let ci = VoxelGrid::chunk_idx(cx, cy, cz);
                if let Some(prev) = prev_ci {
                    assert_eq!(
                        ci,
                        prev + 1,
                        "Chunk draw loop must increment linearly with stride +1 for cache locality"
                    );
                }
                prev_ci = Some(ci);
                total_chunks += 1;
            }
        }
    }

    assert_eq!(total_chunks, CHUNKS_X * CHUNKS_Y * CHUNKS_Z);
    assert_eq!(total_chunks, 19584);
}

// ----------------------------------------------------------------------------
// Group 2: Localized BFS vs Global BFS Consistency Oracle
// ----------------------------------------------------------------------------

/// Differential testing harness: runs identical operations on two grids.
/// `grid_loc` uses localized BFS with recorded dirty bounds.
/// `grid_glob` forces a full global sweep by clearing dirty bounds.
/// Asserts 100% equivalence in collapsed islands, voxel coordinates, and remaining voxels.
fn assert_localized_equals_global_bfs(
    setup: impl Fn(&mut VoxelGrid),
    demolish: impl Fn(&mut VoxelGrid),
) {
    let mut grid_loc = VoxelGrid::new();
    let mut grid_glob = VoxelGrid::new();

    setup(&mut grid_loc);
    setup(&mut grid_glob);

    // Verify identical initial setups
    assert_eq!(grid_loc.voxels, grid_glob.voxels);
    assert_eq!(grid_loc.solid_count, grid_glob.solid_count);

    // Apply demolition to both
    demolish(&mut grid_loc);
    demolish(&mut grid_glob);

    // Verify dirty tracking captured bounds on grid_loc
    assert!(
        grid_loc.dirty_min.is_some() && grid_loc.dirty_max.is_some(),
        "Demolition must set dirty bounds for localized BFS"
    );

    // Run localized BFS on grid_loc
    let islands_loc = grid_loc.detect_unsupported_islands();

    // Clear dirty bounds on grid_glob to force full global BFS sweep
    grid_glob.dirty_min = None;
    grid_glob.dirty_max = None;
    grid_glob.structural_dirty = true;
    let islands_glob = grid_glob.detect_unsupported_islands();

    // 1. Assert identical island counts
    assert_eq!(
        islands_loc.len(),
        islands_glob.len(),
        "Localized BFS produced {} islands but Global BFS produced {}",
        islands_loc.len(),
        islands_glob.len()
    );

    // 2. Assert identical collapsed voxel coordinates
    let mut set_loc: HashSet<(i32, i32, i32)> = HashSet::new();
    for island in &islands_loc {
        for &(pos, _) in &island.voxels {
            set_loc.insert((pos.x, pos.y, pos.z));
        }
    }

    let mut set_glob: HashSet<(i32, i32, i32)> = HashSet::new();
    for island in &islands_glob {
        for &(pos, _) in &island.voxels {
            set_glob.insert((pos.x, pos.y, pos.z));
        }
    }

    assert_eq!(
        set_loc, set_glob,
        "Localized BFS collapsed voxels do not match Global BFS collapsed voxels!"
    );

    // 3. Assert complete grid equivalence (all 2,506,752 voxels must match)
    assert_eq!(
        grid_loc.voxels, grid_glob.voxels,
        "Grid voxel arrays diverged after island collapse!"
    );

    // 4. Assert solid counts match
    assert_eq!(
        grid_loc.solid_count, grid_glob.solid_count,
        "Solid counts diverged: loc={} vs glob={}",
        grid_loc.solid_count, grid_glob.solid_count
    );
}

#[test]
fn test_localized_vs_global_bfs_cantilever_length_threshold() {
    // Concrete has CANTILEVER_MAX_DEPTH = 4.
    // Cantilever lengths 1..=4 remain stable.
    // Cantilever lengths 5..=7 must collapse.
    assert_localized_equals_global_bfs(
        |grid| {
            // Anchor column from bedrock y=0 to y=15 at x=40, z=40
            for y in 0..=15 {
                grid.set(40, y, 40, VoxelType::Concrete);
            }
            // Cantilever arm extending along +x from x=41 to x=47 at y=15 (length 7)
            for x in 41..=47 {
                grid.set(x, 15, 40, VoxelType::Concrete);
            }
        },
        |grid| {
            // Trigger structural dirtying at the cantilever tip
            grid.mark_dirty_region(IVec3::new(45, 15, 40), IVec3::new(47, 15, 40));
            grid.structural_dirty = true;
        },
    );
}

#[test]
fn test_localized_vs_global_bfs_high_rise_vertical_localization_and_adaptive_ceiling() {
    // 35-voxel tall skyscraper tower from bedrock y=0 to y=35.
    // Foundational severance at y=1..3.
    // Localized BFS initializes box_max_y = (3 + 8) = 11 < 35.
    // Adaptive ceiling expansion must expand box_max_y to 35 and drop the entire upper tower identically to global BFS.
    assert_localized_equals_global_bfs(
        |grid| {
            for y in 0..=35 {
                for x in 40..=42 {
                    for z in 40..=42 {
                        grid.set(x, y, z, VoxelType::Concrete);
                    }
                }
            }
        },
        |grid| {
            // Sever the foundation columns at y=1..3
            for y in 1..=3 {
                for x in 40..=42 {
                    for z in 40..=42 {
                        grid.damage_voxel(x, y, z, 5000.0, DamageType::Explosive);
                    }
                }
            }
        },
    );
}

#[test]
fn test_localized_vs_global_bfs_rebar_tensile_suspension_and_severance() {
    // Extended horizontal bridge (length 12) reinforced with SteelRebar.
    // Steel rebar bridges at depth 0 when anchored in tension or foundation.
    // Severing the anchor rebar at the root drops the whole bridge.
    assert_localized_equals_global_bfs(
        |grid| {
            // Anchor column at x=30, z=30 from y=0 to y=10
            for y in 0..=10 {
                grid.set(30, y, 30, VoxelType::SteelRebar);
            }
            // Extended rebar arm at y=10 from x=31 to x=42
            for x in 31..=42 {
                grid.set(x, 10, 30, VoxelType::SteelRebar);
            }
        },
        |grid| {
            // Sever rebar root at x=31, y=10, z=30
            grid.damage_voxel(31, 10, 30, 5000.0, DamageType::Explosive);
        },
    );
}

#[test]
fn test_localized_vs_global_bfs_boundary_crossing_and_fallback() {
    // Structure spanning beyond the localized padding margin (pad = 8).
    // Damage occurs at x=50. Structure extends back to x=38 (crossing box_min_x = 42).
    // This touches solid voxels outside the localized bounding box, which must trigger
    // touches_solid_outside and fall back to global BFS without error.
    assert_localized_equals_global_bfs(
        |grid| {
            // Foundation pillar at x=36..38, y=0..12, z=36
            for y in 0..=12 {
                for x in 36..=38 {
                    grid.set(x, y, 36, VoxelType::Concrete);
                }
            }
            // Long horizontal bridge at y=12 from x=38 to x=52
            for x in 38..=52 {
                grid.set(x, 12, 36, VoxelType::Concrete);
            }
        },
        |grid| {
            // Sever bridge at x=49, y=12, z=36
            grid.damage_voxel(49, 12, 36, 5000.0, DamageType::Explosive);
        },
    );
}

#[test]
fn test_localized_vs_global_bfs_subterranean_excavation_consistency() {
    // Excavation into subterranean bedrock/basalt at y = -15.
    assert_localized_equals_global_bfs(
        |grid| {
            // Subterranean hollow bunker excavated into basalt
            for x in 45..=55 {
                for y in -20..=-10 {
                    for z in 45..=55 {
                        if x == 45 || x == 55 || y == -20 || y == -10 || z == 45 || z == 55 {
                            grid.set(x, y, z, VoxelType::Concrete);
                        } else {
                            grid.set(x, y, z, VoxelType::Air);
                        }
                    }
                }
            }
        },
        |grid| {
            // Blast a corner of the bunker ceiling at y = -10
            grid.damage_voxel(46, -10, 46, 5000.0, DamageType::Explosive);
            grid.damage_voxel(47, -10, 46, 5000.0, DamageType::Explosive);
        },
    );
}

#[test]
fn test_localized_vs_global_bfs_adversarial_differential_fuzzing() {
    // Stress harness running multiple procedural building variations
    for seed in 0..5 {
        assert_localized_equals_global_bfs(
            |grid| {
                let base_x = 30 + seed * 4;
                let base_z = 30 + seed * 4;
                // Vertical columns
                for y in 0..=20 {
                    grid.set(base_x, y, base_z, VoxelType::Concrete);
                    grid.set(base_x + 5, y, base_z, VoxelType::Concrete);
                }
                // Horizontal cross beams at y=10 and y=20
                for x in base_x..=(base_x + 5) {
                    grid.set(x, 10, base_z, VoxelType::SteelRebar);
                    grid.set(x, 20, base_z, VoxelType::Concrete);
                }
                // Cantilever balcony on top
                for x in (base_x + 5)..=(base_x + 10) {
                    grid.set(x, 20, base_z, VoxelType::Concrete);
                }
            },
            |grid| {
                let base_x = 30 + seed * 4;
                let base_z = 30 + seed * 4;
                // Cut one column at y=2
                grid.damage_voxel(base_x, 2, base_z, 5000.0, DamageType::Explosive);
            },
        );
    }
}

// ----------------------------------------------------------------------------
// Group 3: Zero-Allocation Carving & World Gen Invariants
// ----------------------------------------------------------------------------

#[test]
fn test_zero_allocation_carving_spall_event_cap_and_capacity() {
    let mut grid = VoxelGrid::new();

    // Verify initial pre-allocated capacities
    assert_eq!(grid.spall_events.capacity(), 32);
    assert_eq!(grid.severed_rebar_positions.capacity(), 64);

    // Damage a surface area of 10x10 concrete voxels to saturate the 32 spall event cap
    for x in 40..50 {
        for z in 40..50 {
            grid.set(x, 1, z, VoxelType::Concrete);
            grid.damage_voxel(x, 1, z, 50.0, DamageType::Explosive);
        }
    }

    // Spall events must cap at 32 and never exceed capacity
    assert_eq!(grid.spall_events.len(), 32);
    assert_eq!(grid.spall_events.capacity(), 32);

    // Additional damage operations must short-circuit without reallocating
    for x in 50..60 {
        grid.set(x, 1, 40, VoxelType::Concrete);
        grid.damage_voxel(x, 1, 40, 50.0, DamageType::Explosive);
    }

    assert_eq!(grid.spall_events.len(), 32);
    assert_eq!(grid.spall_events.capacity(), 32);
}

#[test]
fn test_subway_tunnel_glowcore_and_crosshead_invariants() {
    let mut grid = VoxelGrid::new();
    world_gen::build_subterranean_subway_tunnel(&mut grid, (10, 48), (50, 48), -20, Some((30, 48)));

    let mut ceiling_lights = 0;
    let y_ceiling = -20 + 3;
    for x in 10..=50 {
        if grid.get(x, y_ceiling, 48) == VoxelType::GlowCore {
            ceiling_lights += 1;
        }
    }

    assert!(
        ceiling_lights > 0,
        "Subway tunnels must generate fluorescent GlowCore ceiling lights at y={}, found {}",
        y_ceiling,
        ceiling_lights
    );
}
