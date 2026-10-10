#![allow(clippy::new_without_default)]
#![allow(dead_code, unused_imports, clippy::manual_range_contains)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::PhysicsSystem;
use voxel::{VoxelGrid, VoxelType, DamageType, VOXEL_SIZE};
use weapon::{WeaponSystem, MAX_C4_CHARGES, MAX_DET_CORD_DIST};

// ============================================================================
// EMPIRICAL CHALLENGER STRESS SUITE: MILESTONE G4-M3 C4 DEMOLITION ARSENAL
// 1. 24-Charge Grid Saturation & Origin (0,0,0) Anchoring
// 2. RNG Laser Det-Cord Empty-Graph Invariant (Square Perimeter vs Diagonals)
// 3. Laser Det-Cord > 25.0m Strict Cutoff & Degenerate Distance Rejection
// 4. Foundation Cutting Shock Column Severance & Structural Island Detachment
// 5. Full 24-Point Foundation Grid Demolition & Upper Section Catastrophe
// ============================================================================

// ----------------------------------------------------------------------------
// Task 1A: 24-Charge Demolition Grid Saturation & 25th Rejection
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_c4_grid_saturation_24_charges_and_25th_rejection() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();

    assert_eq!(MAX_C4_CHARGES, 24, "MAX_C4_CHARGES must be 24");

    // Place 24 independent columns across the district
    for i in 0..24 {
        let vx = 10 + (i % 6) * 6;
        let vz = 10 + (i / 6) * 6;
        grid.set(vx, 3, vz, VoxelType::Concrete);

        let pos = grid.voxel_to_world(IVec3::new(vx, 3, vz));
        let success = weapon.place_c4_charge(pos, Vec3::Y, Some(IVec3::new(vx, 3, vz)));
        assert!(success, "Charge {} of 24 must be successfully placed", i + 1);
        assert_eq!(weapon.c4_charges.len(), i as usize + 1);
    }

    assert_eq!(weapon.c4_charges.len(), 24, "Demolition grid must contain exactly 24 charges");

    // Attempt to place 25th, 26th, and 27th charges
    for extra in 25..=27 {
        let vx = 80 + extra;
        grid.set(vx, 3, 80, VoxelType::Concrete);
        let extra_pos = grid.voxel_to_world(IVec3::new(vx, 3, 80));
        let extra_placed = weapon.place_c4_charge(extra_pos, Vec3::Y, Some(IVec3::new(vx, 3, 80)));
        assert!(!extra_placed, "Charge {} must be rejected due to MAX_C4_CHARGES=24 saturation cap", extra);
        assert_eq!(weapon.c4_charges.len(), 24, "Active charge count must remain clamped at 24");
    }
}

// ----------------------------------------------------------------------------
// Task 1B: Origin (0, 0, 0) Anchoring Invariant
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_c4_origin_anchoring_and_destruction_lifecycle() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Verify grid coordinate origin (0, 0, 0) is valid in-bounds
    assert!(grid.in_bounds(0, 0, 0), "Voxel origin (0, 0, 0) must be within grid boundaries");
    grid.set(0, 0, 0, VoxelType::Concrete);
    assert_eq!(grid.get(0, 0, 0), VoxelType::Concrete);

    let origin_voxel = IVec3::new(0, 0, 0);
    let origin_world = grid.voxel_to_world(origin_voxel);

    // Place C4 charge anchored to origin voxel
    let placed = weapon.place_c4_charge(origin_world, Vec3::Y, Some(origin_voxel));
    assert!(placed, "C4 charge at origin voxel must be placed successfully");
    assert_eq!(weapon.c4_charges.len(), 1);
    assert_eq!(weapon.c4_charges[0].anchor_voxel, Some(origin_voxel));

    // Step physics/weapon: since origin voxel is solid Concrete, charge must remain anchored
    weapon.update(0.1, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.c4_charges.len(), 1, "Origin charge must remain anchored while anchor voxel is solid");

    // Also place a second charge anchored to world coordinate origin Vec3::ZERO
    let world_origin_voxel = grid.world_to_voxel(Vec3::ZERO);
    assert!(grid.in_bounds(world_origin_voxel.x, world_origin_voxel.y, world_origin_voxel.z));
    grid.set(world_origin_voxel.x, world_origin_voxel.y, world_origin_voxel.z, VoxelType::SteelRebar);

    let placed_world_origin = weapon.place_c4_charge(Vec3::ZERO, Vec3::Y, Some(world_origin_voxel));
    assert!(placed_world_origin, "C4 charge at world origin (0,0,0) must place successfully");
    assert_eq!(weapon.c4_charges.len(), 2);

    // Step update: both remain anchored
    weapon.update(0.1, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.c4_charges.len(), 2);

    // Now destroy the anchor voxel at origin (0, 0, 0) by setting to Air
    grid.set(0, 0, 0, VoxelType::Air);
    assert_eq!(grid.get(0, 0, 0), VoxelType::Air);

    // Weapon update must detect missing anchor and drop the origin charge
    weapon.update(0.1, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.c4_charges.len(), 1, "Charge at destroyed origin anchor must be purged");
    assert_eq!(weapon.c4_charges[0].anchor_voxel, Some(world_origin_voxel), "Surviving charge must be the other one");
}

// ----------------------------------------------------------------------------
// Task 2A: RNG Laser Det-Cord Square Topology (4 Perimeter Links, 0 Diagonals)
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_rng_laser_det_cords_square_perimeter_no_diagonals() {
    let mut weapon = WeaponSystem::new();

    // Set up 4 C4 charges forming a square with side length L = 10.0m
    // Vertices: A(0, 0), B(10, 0), C(10, 10), D(0, 10) at height Y = 5.0m
    let side_len = 10.0f32;
    let diag_len = (side_len * side_len + side_len * side_len).sqrt(); // ~14.142m <= 25.0m
    assert!(diag_len <= MAX_DET_CORD_DIST, "Diagonal must be <= 25.0m to challenge RNG pruning");

    let p_a = Vec3::new(20.0, 5.0, 20.0);
    let p_b = Vec3::new(20.0 + side_len, 5.0, 20.0);
    let p_c = Vec3::new(20.0 + side_len, 5.0, 20.0 + side_len);
    let p_d = Vec3::new(20.0, 5.0, 20.0 + side_len);

    weapon.place_c4_charge(p_a, Vec3::Y, None);
    weapon.place_c4_charge(p_b, Vec3::Y, None);
    weapon.place_c4_charge(p_c, Vec3::Y, None);
    weapon.place_c4_charge(p_d, Vec3::Y, None);
    assert_eq!(weapon.c4_charges.len(), 4);

    let links = weapon.compute_det_cord_links();

    // In Relative Neighborhood Graph (RNG):
    // For a square, exactly the 4 perimeter edges must exist: (A-B, B-C, C-D, D-A).
    // The 2 interior diagonal edges (A-C and B-D) MUST be strictly excluded
    // because B (and D) are closer to both A and C than dist(A, C).
    assert_eq!(
        links.len(),
        4,
        "RNG must establish exactly 4 perimeter links for a square; got {}",
        links.len()
    );

    // Verify all 4 links are perimeter links (~10.0m) and ZERO links are diagonals (~14.14m)
    let mut perimeter_count = 0;
    let mut diagonal_count = 0;

    for link in &links {
        let dist = link.dist;
        if (dist - side_len).abs() < 0.1 {
            perimeter_count += 1;
        } else if (dist - diag_len).abs() < 0.1 {
            diagonal_count += 1;
        }
    }

    assert_eq!(perimeter_count, 4, "All 4 links must be square perimeter edges");
    assert_eq!(diagonal_count, 0, "RNG must prune all interior diagonal det-cord links");
}

// ----------------------------------------------------------------------------
// Task 2B: RNG Laser Det-Cord Distance Cutoff Strictness (> 25.0m & < 0.05m)
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_rng_laser_det_cords_distance_cutoff_strictness() {
    // Subtest 1: Distance exactly at threshold 25.0m
    {
        let mut weapon = WeaponSystem::new();
        let p1 = Vec3::new(10.0, 5.0, 10.0);
        let p2 = Vec3::new(10.0 + 25.0, 5.0, 10.0);
        weapon.place_c4_charge(p1, Vec3::Y, None);
        weapon.place_c4_charge(p2, Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        assert_eq!(links.len(), 1, "Charge pair at exactly 25.0m must connect");
    }

    // Subtest 2: Distance slightly below threshold (24.95m)
    {
        let mut weapon = WeaponSystem::new();
        let p1 = Vec3::new(10.0, 5.0, 10.0);
        let p2 = Vec3::new(10.0 + 24.95, 5.0, 10.0);
        weapon.place_c4_charge(p1, Vec3::Y, None);
        weapon.place_c4_charge(p2, Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        assert_eq!(links.len(), 1, "Charge pair at 24.95m must connect");
    }

    // Subtest 3: Distance slightly exceeding threshold (25.05m)
    {
        let mut weapon = WeaponSystem::new();
        let p1 = Vec3::new(10.0, 5.0, 10.0);
        let p2 = Vec3::new(10.0 + 25.05, 5.0, 10.0);
        weapon.place_c4_charge(p1, Vec3::Y, None);
        weapon.place_c4_charge(p2, Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        assert_eq!(
            links.len(),
            0,
            "Charge pair at 25.05m must NOT connect (exceeds MAX_DET_CORD_DIST=25.0m)"
        );
    }

    // Subtest 4: Distant charges across city district (40m, 80m)
    {
        let mut weapon = WeaponSystem::new();
        let p1 = Vec3::new(10.0, 5.0, 10.0);
        let p2 = Vec3::new(50.0, 5.0, 10.0); // 40m
        let p3 = Vec3::new(90.0, 5.0, 10.0); // 80m
        weapon.place_c4_charge(p1, Vec3::Y, None);
        weapon.place_c4_charge(p2, Vec3::Y, None);
        weapon.place_c4_charge(p3, Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        assert_eq!(links.len(), 0, "No det-cords must form across distances > 25.0m");
    }

    // Subtest 5: Degenerate co-located charges (< 0.05m)
    {
        let mut weapon = WeaponSystem::new();
        let p1 = Vec3::new(10.0, 5.0, 10.0);
        let p2 = Vec3::new(10.02, 5.0, 10.0); // 0.02m apart
        weapon.place_c4_charge(p1, Vec3::Y, None);
        weapon.place_c4_charge(p2, Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        assert_eq!(
            links.len(),
            0,
            "Degenerate overlapping charges (< 0.05m) must not create zero-length det-cords"
        );
    }
}

// ----------------------------------------------------------------------------
// Task 3: Foundation Cutting Shock Slices Columns into Air & Triggers Structural Island Detachment
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_foundation_cutting_shock_slices_columns_and_detaches_structure() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a building with 2 grounded support columns and an elevated upper structure:
    // Bedrock at Y = 0
    // Column 1 at X=30, Z=25, Y=1..=8 (Reinforced Concrete)
    // Column 2 at X=30, Z=35, Y=1..=8 (Reinforced Concrete)
    // Upper floor / roof bridging the columns at Y=9..=12, spanning Z=24..=36, X=28..=32
    for y in 1..=8 {
        grid.set(30, y, 25, VoxelType::Concrete);
        grid.set(30, y, 35, VoxelType::Concrete);
    }

    for x in 28..=32 {
        for z in 24..=36 {
            grid.set(x, 9, z, VoxelType::SteelRebar); // Tensile rebar bridge across columns
            for y in 10..=12 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Verify initial structure is solidly grounded: zero unsupported islands
    let initial_islands = grid.detect_unsupported_islands();
    assert_eq!(
        initial_islands.len(),
        0,
        "Building columns must solidly ground the upper floor structure"
    );

    // Place C4 charges on the two columns at Y=4 (the foundation cutting plane)
    let pos_col1 = grid.voxel_to_world(IVec3::new(30, 4, 25));
    let pos_col2 = grid.voxel_to_world(IVec3::new(30, 4, 35));

    let dist = (pos_col1 - pos_col2).length();
    assert!(dist <= MAX_DET_CORD_DIST, "Column span ({:.1}m) is within det-cord range", dist);

    weapon.place_c4_charge(pos_col1, Vec3::X, Some(IVec3::new(30, 4, 25)));
    weapon.place_c4_charge(pos_col2, Vec3::X, Some(IVec3::new(30, 4, 35)));
    assert_eq!(weapon.c4_charges.len(), 2);

    // Detonate with 'F' key foundation cutting shock
    let (feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 2, "Both charges detonated");
    assert!(feedback.camera_trauma > 0.0);

    // 1. Verify `carve_cylinder_advanced` sliced columns into Air at the cutting plane Y=4
    assert_eq!(
        grid.get(30, 4, 25),
        VoxelType::Air,
        "Column 1 at cutting height Y=4 must be completely severed into Air"
    );
    assert_eq!(
        grid.get(30, 4, 35),
        VoxelType::Air,
        "Column 2 at cutting height Y=4 must be completely severed into Air"
    );

    // Also check intermediate voxels along the cutting beam line
    let mid_voxel = IVec3::new(30, 4, 30);
    assert_eq!(
        grid.get(mid_voxel.x, mid_voxel.y, mid_voxel.z),
        VoxelType::Air,
        "Midpoint of cutting shock cylinder must be Air"
    );

    // 2. Verify structural integrity solver detaches unsupported upper sections
    assert!(
        grid.structural_dirty,
        "Destruction from cutting shock must mark structural integrity dirty"
    );

    let collapsed_islands = grid.detect_unsupported_islands();
    assert!(
        !collapsed_islands.is_empty(),
        "Structural integrity solver must detect severed upper structure as unsupported island(s)"
    );

    let total_detached_voxels: usize = collapsed_islands.iter().map(|i| i.voxels.len()).sum();
    assert!(
        total_detached_voxels > 50,
        "Severed upper building section (columns Y=5..8 + floor Y=9..12) must detach; detached {} voxels",
        total_detached_voxels
    );

    // 3. Verify upper structure voxels are now removed from the voxel grid
    assert_eq!(
        grid.get(30, 10, 30),
        VoxelType::Air,
        "Upper floor voxels must be cleared from grid upon detachment into physics island"
    );

    // 4. Verify island chunks can be converted into active dynamic physics chunks
    let initial_chunk_count = physics.chunks.len();
    for island in collapsed_islands {
        physics.spawn_island_chunk(island, Vec3::new(0.0, -2.0, 0.0), Vec3::ZERO);
    }
    assert!(
        physics.chunks.len() > initial_chunk_count,
        "Detached islands must spawn dynamic physics chunks undergoing gravitational collapse"
    );
}

// ----------------------------------------------------------------------------
// Task 4: 24-Charge Grid Foundation Decapitation & Total Collapse
// ----------------------------------------------------------------------------
#[test]
fn test_challenger_24_c4_full_foundation_grid_implosion_cascade() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Build a 4x6 column matrix supporting a massive cantilevered roof deck
    // Columns at x in [20, 26, 32, 38], z in [20, 26, 32, 38, 44, 50]
    let col_xs = [20, 26, 32, 38];
    let col_zs = [20, 26, 32, 38, 44, 50];
    assert_eq!(col_xs.len() * col_zs.len(), 24);

    for &cx in &col_xs {
        for &cz in &col_zs {
            for cy in 1..=6 {
                grid.set(cx, cy, cz, VoxelType::Concrete);
            }
        }
    }

    // Heavy concrete roof slab across the entire matrix (y = 7..=8)
    for x in 18..=40 {
        for z in 18..=52 {
            grid.set(x, 7, z, VoxelType::Concrete);
            grid.set(x, 8, z, VoxelType::SteelRebar);
        }
    }

    // Initial state: fully supported
    assert_eq!(grid.detect_unsupported_islands().len(), 0);

    // Place all 24 C4 shaped charges at y=3 on all 24 columns
    let mut charge_count = 0;
    for &cx in &col_xs {
        for &cz in &col_zs {
            let pos = grid.voxel_to_world(IVec3::new(cx, 3, cz));
            let placed = weapon.place_c4_charge(pos, Vec3::Y, Some(IVec3::new(cx, 3, cz)));
            assert!(placed);
            charge_count += 1;
        }
    }
    assert_eq!(charge_count, 24);
    assert_eq!(weapon.c4_charges.len(), 24);

    // Verify RNG creates connected det-cord mesh
    let links = weapon.compute_det_cord_links();
    assert!(links.len() >= 24, "24-point grid must form extensive det-cord network; got {} links", links.len());

    // Detonate all 24 charges simultaneously with cutting shock
    let (feedback, detonated) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(detonated, 24);
    assert_eq!(feedback.camera_trauma, 1.0, "Full 24-C4 grid detonation must hit maximum camera trauma 1.0");

    // All columns at Y=3 must be sliced to Air
    for &cx in &col_xs {
        for &cz in &col_zs {
            assert_eq!(
                grid.get(cx, 3, cz),
                VoxelType::Air,
                "Column at ({}, 3, {}) must be severed",
                cx, cz
            );
        }
    }

    // Structural integrity check: massive roof slab must detach as collapsing island
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severed roof slab must collapse into island(s)");

    let collapsed_voxels: usize = islands.iter().map(|i| i.voxels.len()).sum();
    assert!(
        collapsed_voxels >= 800,
        "Massive roof slab must collapse hundreds of voxels; got {}",
        collapsed_voxels
    );
}
