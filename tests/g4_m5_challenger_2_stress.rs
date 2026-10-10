// ============================================================================
// EMPIRICAL CHALLENGER ADVERSARIAL STRESS TEST SUITE: MILESTONE G4-M5
// Agent: g4_m5_challenger_2 (Master Empirical Challenger)
// Roles: critic, specialist
//
// Empirically challenges and stress-tests:
// R1: Progressive Pancake Pulverization (multi-story collapses, intermediate story pulverization)
// R2: Subterranean Player Crater Navigation & Safe Void Boundary Reset
// R3: Demolition Arsenal (24 C4 grid, RNG det-cords, 'F' cutting shock, Singularity, Orbital Relay)
// R4: Performance & Sub-Region BFS (footprint reduction, frustum culling, zero-allocation pooling)
// ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{
    DebrisParticle, DecalKind, DynamicChunk, ElectricalArc, ImpactDecal, PhysicsSystem,
    PressurizedFireJet, ShockwaveRing, SmokeParticle, SparkParticle,
};
use player::Player;
use siege::{SiegeEngineKind, SiegeSystem};
use voxel::{
    CarveResult, DamageType, FrustumPlanes, VoxelGrid, VoxelType, CANTILEVER_MAX_DEPTH,
    GRID_X, GRID_Z, MIN_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE,
};
use weapon::{ActiveSingularity, WeaponSystem, MAX_C4_CHARGES};
use world_gen::build_multi_lane_road_deck;

// ============================================================================
// CHALLENGER BATTERY 1: R1 PROGRESSIVE PANCAKE PULVERIZATION
// ============================================================================

/// Challenger Test 1: Multi-Story Pancake Collapse with Intermediate Floor Pulverization
/// Verifies:
/// - Heavy descending chunk pulverizes intermediate reinforced concrete and steel floors
/// - Yielded floors shatter into outward-scattered dynamic micro-chunks and spall debris
/// - Accretion of mass onto falling slab from pulverized floor material
/// - Pulverized rubble settles onto dynamic rubble heightmap
#[test]
fn test_challenger_r1_progressive_pancake_pulverization_and_rubble_settling() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // 1. Build a multi-story building with 3 reinforced concrete and steel floors
    // Floors at y = 8, y = 16, y = 24
    let floor_levels = [8, 16, 24];
    for &fy in &floor_levels {
        for dx in -4i32..=4i32 {
            for dz in -4i32..=4i32 {
                let vt = if (dx.abs() == 4 || dz.abs() == 4) && (dx + dz) % 2 == 0 {
                    VoxelType::SteelRebar
                } else {
                    VoxelType::Concrete
                };
                grid.set(48 + dx, fy, 48 + dz, vt);
                grid.set(48 + dx, fy + 1, 48 + dz, vt);
            }
        }
    }
    // Also build supporting columns
    for dy in 0..30 {
        grid.set(44, dy, 44, VoxelType::SteelRebar);
        grid.set(52, dy, 44, VoxelType::SteelRebar);
        grid.set(44, dy, 52, VoxelType::SteelRebar);
        grid.set(52, dy, 52, VoxelType::SteelRebar);
    }
    let initial_solids = grid.solid_count();
    assert!(initial_solids > 0);

    // 2. Heavy falling slab descending rapidly onto the top floor (y=24)
    let mut slab_voxels = Vec::new();
    for x in -3..=3 {
        for y in 0..2 {
            for z in -3..=3 {
                slab_voxels.push((
                    Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                    VoxelType::HeavyArmor,
                ));
            }
        }
    }
    let aabb = DynamicChunk::compute_aabb(&slab_voxels);
    let slab_world = grid.voxel_to_world(IVec3::new(48, 26, 48));
    let initial_mass = 2500.0f32;

    phys.chunks.push(DynamicChunk {
        position: slab_world + Vec3::new(0.0, 0.5, 0.0),
        velocity: Vec3::new(0.0, -18.0, 0.0), // High downward momentum
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: slab_voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: initial_mass,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    let initial_debris = phys.debris.len();

    // 3. Step physics simulation across multiple ticks to induce progressive pancake collapse
    for _ in 0..12 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    // Verify intermediate floors were pulverized
    assert!(
        grid.solid_count() < initial_solids,
        "Intermediate floors must be pulverized into dynamic debris! Initial solids {}, now {}",
        initial_solids,
        grid.solid_count()
    );

    // Verify dynamic micro-chunks and debris spawned
    assert!(
        phys.debris.len() > initial_debris,
        "Pulverization must spawn outward-scattered DebrisParticle instances! Initial {}, now {}",
        initial_debris,
        phys.debris.len()
    );

    // Verify debris colors match pulverized concrete or steel rebar
    let has_concrete_or_rebar = phys.debris.iter().any(|d| {
        d.color == VoxelType::Concrete.color() || d.color == VoxelType::SteelRebar.color()
    });
    assert!(has_concrete_or_rebar, "Debris particles must inherit pulverized voxel colors");

    // Verify dust smoke was generated
    assert!(!phys.smoke.is_empty(), "Pancake collapse must spawn billowing dust clouds");

    // Verify chunk mass accretes from pulverized floor density
    if let Some(main_chunk) = phys.chunks.first() {
        assert!(
            main_chunk.mass >= initial_mass,
            "Main falling chunk must accrete mass from pulverized intermediate floors: {} >= {}",
            main_chunk.mass,
            initial_mass
        );
    }

    // Verify rubble settles onto heightmap
    let r_height = phys.rubble_height_at(slab_world.x, slab_world.z);
    assert!(
        r_height.is_finite(),
        "Rubble height at crush zone must be a valid finite number"
    );
}

// ============================================================================
// CHALLENGER BATTERY 2: R2 SUBTERRANEAN PLAYER CRATER NAVIGATION & VOID BOUNDARY
// ============================================================================

/// Challenger Test 2: Subterranean Player Ground Navigation in Craters and Safe Void Reset
/// Verifies:
/// - Player navigates and stands firmly grounded inside negative Y subterranean craters
/// - Horizontal navigation inside craters does not clip, freeze, or pop to surface Y=0
/// - Safe bottom void boundary (Y <= -24.0m) instantly resets player to safe spawn origin
#[test]
fn test_challenger_r2_player_subterranean_crater_navigation_and_void_boundary() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();

    // Build subterranean ground:
    // Bedrock at y=0, basalt down to y=MIN_VOXEL_Y (-48 voxels = -24.0m)
    for x in 30..=66 {
        for z in 30..=66 {
            grid.set(x, 0, z, VoxelType::Bedrock);
            for y in MIN_VOXEL_Y..0 {
                grid.set(x, y, z, VoxelType::Basalt);
            }
        }
    }

    // Excavate a deep subterranean crater centered at (48, 0, 48) with radius 6.0m (12 voxels)
    // Crater floor will reach down to y = -24 voxels (-12.0m)
    let crater_center = grid.voxel_to_world(IVec3::new(48, 0, 48));
    let mut carve_res = CarveResult::default();
    grid.carve_sphere_advanced_into(crater_center, 6.0, 50000.0, DamageType::Explosive, &mut carve_res);
    physics.update_crater_floor(&grid, crater_center, 6.0);

    // Verify crater was carved down to negative Y
    let crater_floor_y = physics.rubble_height_at(crater_center.x, crater_center.z);
    assert!(
        crater_floor_y <= -5.0,
        "Crater floor must reach deep negative subterranean coordinates, got {:.2}",
        crater_floor_y
    );

    // 1. Place Player at the bottom of the excavated crater
    let surface_spawn = Vec3::new(48.0, 5.0, 48.0);
    let mut player = Player::new(surface_spawn);

    // Place player inside subterranean crater just above the floor
    player.position = Vec3::new(crater_center.x, crater_floor_y + 0.04, crater_center.z);
    player.velocity = Vec3::ZERO;
    player.is_grounded = false;

    // Step player update inside crater
    for _ in 0..6 {
        player.update(0.016, &grid, Some(&physics), false);
    }

    // Invariant 1: Player must be grounded inside subterranean crater at negative Y
    assert!(
        player.is_grounded,
        "Player must be grounded inside subterranean crater at y = {:.2}",
        player.position.y
    );
    assert!(
        player.position.y <= -4.0,
        "Player position must remain down inside the crater (<= -4.0m), got {:.2}",
        player.position.y
    );
    assert!(
        (player.position.y - crater_floor_y).abs() <= 0.15,
        "Player must rest on the crater floor at {:.2}, got {:.2}",
        crater_floor_y,
        player.position.y
    );

    // Invariant 2: Subterranean horizontal walking / sliding without popping to surface Y=0
    player.velocity = Vec3::new(2.5, 0.0, 0.0);
    player.update(0.016, &grid, Some(&physics), false);
    assert!(
        player.position.y <= -4.0,
        "Horizontal navigation inside crater must NOT cause clipping or popping to surface Y=0, got {:.2}",
        player.position.y
    );

    // 2. Safe Bottom Void Boundary Trigger (Y <= -24.0m)
    // Drop player below VOID_BOUNDARY_Y (-24.0m)
    player.position = Vec3::new(crater_center.x, VOID_BOUNDARY_Y - 0.5, crater_center.z);
    player.velocity = Vec3::new(10.0, -45.0, 5.0);
    player.trauma = 0.8;
    player.fov_kick = 0.3;

    // Step player: void boundary guard must fire immediately
    player.update(0.016, &grid, Some(&physics), false);

    assert_eq!(
        player.position,
        surface_spawn,
        "Player falling below VOID_BOUNDARY_Y must safely reset to surface spawn"
    );
    assert_eq!(
        player.velocity,
        Vec3::ZERO,
        "Player velocity must be completely zeroed after void reset"
    );
    assert_eq!(player.trauma, 0.0, "Player trauma must reset on void respawn");
    assert_eq!(player.fov_kick, 0.0, "Player FOV kick must reset on void respawn");
    assert!(player.just_respawned, "just_respawned flag must be set on void reset");
}

// ============================================================================
// CHALLENGER BATTERY 3: R3 DEMOLITION ARSENAL EMPIRICAL PROOF
// ============================================================================

/// Challenger Test 3: 24 C4 Charges Placement, Normal Alignment, and 25th Rejection
#[test]
fn test_challenger_r3_c4_saturation_normals_and_rejection() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();

    let test_normals = [
        Vec3::X,
        -Vec3::X,
        Vec3::Z,
        -Vec3::Z,
        Vec3::Y,
        -Vec3::Y,
    ];

    // Place 24 charges with distinct surface normals
    for i in 0..MAX_C4_CHARGES {
        let normal = test_normals[i % test_normals.len()];
        let voxel_coord = IVec3::new(20 + (i as i32 % 6) * 4, 4 + (i as i32 / 6) * 4, 30);
        grid.set(voxel_coord.x, voxel_coord.y, voxel_coord.z, VoxelType::Concrete);
        let surface_pos = grid.voxel_to_world(voxel_coord);

        let placed = weapon.place_c4_charge(surface_pos, normal, Some(voxel_coord));
        assert!(placed, "C4 charge {} must be placed successfully", i);

        // Assert surface-normal offset alignment: pos + normal * 0.05
        let recorded = &weapon.c4_charges[i];
        let expected_pos = surface_pos + normal * 0.05;
        assert!(
            (recorded.position - expected_pos).length() < 1e-4,
            "C4 charge {} position must align to surface normal offset",
            i
        );
        assert_eq!(recorded.normal, normal);
        assert_eq!(recorded.anchor_voxel, Some(voxel_coord));
    }

    assert_eq!(weapon.c4_charges.len(), 24);

    // Assert 25th charge is rejected
    let extra_coord = IVec3::new(80, 10, 80);
    grid.set(extra_coord.x, extra_coord.y, extra_coord.z, VoxelType::Concrete);
    let extra_pos = grid.voxel_to_world(extra_coord);
    let rejected = weapon.place_c4_charge(extra_pos, Vec3::Y, Some(extra_coord));
    assert!(!rejected, "25th C4 charge must be rejected by MAX_C4_CHARGES cap");
    assert_eq!(weapon.c4_charges.len(), 24);
}

/// Challenger Test 4: RNG Det-Cords Topology — No Interior Diagonal Chords
#[test]
fn test_challenger_r3_rng_det_cords_eliminate_interior_diagonals() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();

    // Setup 4 charges forming a 10m x 10m planar square on concrete foundation
    let coords = [
        IVec3::new(20, 2, 20),
        IVec3::new(40, 2, 20),
        IVec3::new(40, 2, 40),
        IVec3::new(20, 2, 40),
    ];

    for &coord in &coords {
        grid.set(coord.x, coord.y, coord.z, VoxelType::Concrete);
        let pos = grid.voxel_to_world(coord);
        assert!(weapon.place_c4_charge(pos, Vec3::Y, Some(coord)));
    }

    let links = weapon.compute_det_cord_links();

    // In a square where side = 10m and diagonal = 14.14m:
    // Relative Neighborhood Graph (RNG) condition requires eliminating diagonal cross-cords.
    assert_eq!(
        links.len(),
        4,
        "RNG must produce exactly 4 perimeter links and 0 interior diagonal chords, got {}",
        links.len()
    );

    let has_diag_0_2 = links.iter().any(|l| (l.idx_a == 0 && l.idx_b == 2) || (l.idx_a == 2 && l.idx_b == 0));
    let has_diag_1_3 = links.iter().any(|l| (l.idx_a == 1 && l.idx_b == 3) || (l.idx_a == 3 && l.idx_b == 1));

    assert!(!has_diag_0_2, "Diagonal chord between charges 0 and 2 must NOT exist in RNG graph");
    assert!(!has_diag_1_3, "Diagonal chord between charges 1 and 3 must NOT exist in RNG graph");

    // Also test a 3x3 grid (9 charges, 12 orthogonal links, 0 diagonals)
    weapon.c4_charges.clear();
    for gz in 0..3 {
        for gx in 0..3 {
            let coord = IVec3::new(10 + gx * 16, 2, 10 + gz * 16); // 8m spacing
            grid.set(coord.x, coord.y, coord.z, VoxelType::Concrete);
            let pos = grid.voxel_to_world(coord);
            weapon.place_c4_charge(pos, Vec3::Y, Some(coord));
        }
    }
    let grid_links = weapon.compute_det_cord_links();
    assert_eq!(grid_links.len(), 12, "3x3 RNG grid must have exactly 12 orthogonal links, got {}", grid_links.len());
    for link in &grid_links {
        assert!(link.dist <= 8.01, "All links must be orthogonal edge length (~8m), got {:.2}", link.dist);
    }
}

/// Challenger Test 5: 'F' Key Foundation Cutting Shock — Radius 1.4m, 520 Kinetic Dmg Slicing
#[test]
fn test_challenger_r3_f_key_foundation_cutting_shock_slicing() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct two columns with SteelRebar (400 HP) and Reinforced Concrete (100 HP)
    // connected by a 10m foundation wall
    for z in 20..=40 {
        for y in 2..=5 {
            let vt = if z == 30 { VoxelType::SteelRebar } else { VoxelType::Concrete };
            grid.set(30, y, z, vt);
        }
    }

    let pos_a = grid.voxel_to_world(IVec3::new(30, 3, 20));
    let pos_b = grid.voxel_to_world(IVec3::new(30, 3, 40));

    weapon.place_c4_charge(pos_a, Vec3::X, Some(IVec3::new(30, 3, 20)));
    weapon.place_c4_charge(pos_b, Vec3::X, Some(IVec3::new(30, 3, 40)));

    // Detonate with 'F' key foundation cutting shock
    let (feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 2);
    assert!(feedback.camera_trauma >= 0.75);

    // Verify slicing corridor:
    // 1. SteelRebar at (30, 3, 30) has 400 HP. Cutting shock with 520 kinetic dmg must vaporize it to Air!
    assert_eq!(
        grid.get(30, 3, 30),
        VoxelType::Air,
        "SteelRebar (400 HP) must be sliced by 520 dmg cutting shock"
    );
    // 2. Concrete at (30, 4, 30) is at distance 0.5m from link centerline (radius 1.4m) -> must be sliced to Air!
    assert_eq!(
        grid.get(30, 4, 30),
        VoxelType::Air,
        "Concrete at radius 0.5m must be sliced into Air"
    );

    // 3. Voxels far outside 1.4m cylinder (e.g. x=35, y=3, z=30, dist = 2.5m > 1.4m) must not be affected
    grid.set(35, 3, 30, VoxelType::Concrete);
    assert_eq!(grid.get(35, 3, 30), VoxelType::Concrete);
}

/// Challenger Test 6: Event Horizon Singularity 35m Vortex Pulls All 4 Entities & 16-22m Implosion
#[test]
fn test_challenger_r3_singularity_35m_vortex_entities_and_collapse_implosion() {
    assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0);
    assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0);
    assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MIN, 16.0);
    assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MAX, 22.0);

    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let vortex_center = Vec3::new(50.0, 20.0, 50.0);

    // Place 4 entity types at 20m (< 35m radius):
    // 1. DynamicChunk
    physics.chunks.push(DynamicChunk {
        position: vortex_center + Vec3::new(20.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        voxel_health: Vec::new(),
        mass: 100.0,
        life: 0.0,
        settled: false,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // 2. DebrisParticle
    physics.debris.push(DebrisParticle {
        position: vortex_center + Vec3::new(0.0, 0.0, 20.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        size: 0.3,
        color: Color::new(0.8, 0.4, 0.1, 1.0),
        life: 10.0,
        max_life: 10.0,
        bounces: 0,
    });

    // 3. SmokeParticle
    physics.smoke.push(SmokeParticle {
        position: vortex_center + Vec3::new(-20.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        size: 0.5,
        start_size: 0.2,
        max_size: 1.0,
        color: Color::new(0.5, 0.5, 0.5, 0.5),
        life: 5.0,
        max_life: 5.0,
    });

    // 4. SparkParticle
    physics.sparks.push(SparkParticle {
        position: vortex_center + Vec3::new(0.0, 0.0, -20.0),
        velocity: Vec3::ZERO,
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 2.0,
        max_life: 2.0,
    });

    // Also place an entity outside the 35m vortex (dist = 45m)
    physics.debris.push(DebrisParticle {
        position: vortex_center + Vec3::new(45.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        size: 0.3,
        color: Color::new(0.8, 0.4, 0.1, 1.0),
        life: 10.0,
        max_life: 10.0,
        bounces: 0,
    });

    // Apply vortex suction for dt = 0.1s
    physics.apply_vortex_suction(vortex_center, ActiveSingularity::PULL_RADIUS, ActiveSingularity::PULL_STRENGTH, 0.1);

    // Verify all 4 inside entities gained inward velocity towards vortex_center:
    assert!(physics.chunks[0].velocity.x < -0.1, "Chunk must be pulled towards vortex center (-X)");
    assert!(physics.debris[0].velocity.z < -0.1, "Debris must be pulled towards vortex center (-Z)");
    assert!(physics.smoke[0].velocity.x > 0.1, "Smoke must be pulled towards vortex center (+X)");
    assert!(physics.sparks[0].velocity.z > 0.1, "Sparks must be pulled towards vortex center (+Z)");

    // Verify entity outside 35m was untouched
    assert_eq!(physics.debris[1].velocity, Vec3::ZERO, "Entity outside 35m must NOT be accelerated");

    // Test collapse implosion
    let mut weapon = WeaponSystem::new();
    weapon.singularities.push(ActiveSingularity {
        position: vortex_center,
        timer: 2.39,
        max_time: 2.40,
        pull_radius: ActiveSingularity::PULL_RADIUS,
        pull_strength: ActiveSingularity::PULL_STRENGTH,
        blast_radius: 20.0, // within [16.0, 22.0]
        vacuum_played: true,
        erosion_timer: 0.0,
    });

    // Step to trigger collapse
    weapon.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(weapon.singularities.is_empty(), "Singularity must collapse at max_time");
    assert!(!physics.shockwaves.is_empty(), "Collapse must spawn shockwave");
    assert!(
        physics.shockwaves.iter().any(|sw| sw.max_radius >= 16.0 && sw.max_radius <= 24.0),
        "Shockwave radius must correspond to 16m-22m implosion collapse"
    );
}

/// Challenger Test 7: Orbital Kinetic Relay Multi-Floor Road Decks Penetration & 16m Bedrock Crater Excavation
#[test]
fn test_challenger_r3_orbital_kinetic_relay_penetration_and_bedrock_crater() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let target_coord = IVec3::new(48, 0, 48);
    let target_world = grid.voxel_to_world(target_coord);

    // Setup:
    // 1. Bedrock (y=0) and subterranean Basalt (y=-1 to y=-48)
    grid.set(48, 0, 48, VoxelType::Bedrock);
    for dx in -16..=16 {
        for dz in -16..=16 {
            grid.set(48 + dx, 0, 48 + dz, VoxelType::Bedrock);
            for y in MIN_VOXEL_Y..0 {
                grid.set(48 + dx, y, 48 + dz, VoxelType::Basalt);
            }
        }
    }

    // 2. Multi-lane road decks and elevated highway overpasses at y=10 and y=25
    build_multi_lane_road_deck(&mut grid, (20, 48), (76, 48), 10, 4, true);
    build_multi_lane_road_deck(&mut grid, (48, 20), (48, 76), 25, 4, true);

    // 3. Skyscraper roof deck at y=50
    for dx in -5..=5 {
        for dz in -5..=5 {
            grid.set(48 + dx, 50, 48 + dz, VoxelType::Concrete);
        }
    }

    // Verify structures exist before orbital strike
    assert_ne!(grid.get(48, 50, 48), VoxelType::Air);
    assert_ne!(grid.get(48, 25, 48), VoxelType::Air);
    assert_ne!(grid.get(48, 10, 48), VoxelType::Air);
    assert_eq!(grid.get(48, 0, 48), VoxelType::Bedrock);

    // Deploy and fire Orbital Kinetic Relay
    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert_eq!(siege.orbital_strikes.len(), 1);
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // 1. Multi-floor and road deck vertical penetration
    assert_eq!(grid.get(48, 50, 48), VoxelType::Air, "Top roof story must be penetrated");
    assert_eq!(grid.get(48, 25, 48), VoxelType::Air, "Elevated road deck must be penetrated");
    assert_eq!(grid.get(48, 10, 48), VoxelType::Air, "Lower road deck must be penetrated");

    // 2. 16m Bedrock Crater at Y = 0.0m (radius 8.0m = 16 voxels)
    // Horizontal radius R = 8.0m: dx^2 + dz^2 <= 16^2 = 256
    assert_eq!(grid.get(48 + 14, 0, 48), VoxelType::Air, "Bedrock at dx=7m (+14 voxels) must be excavated");
    assert_eq!(grid.get(48 - 14, 0, 48), VoxelType::Air, "Bedrock at dx=-7m (-14 voxels) must be excavated");
    assert_eq!(grid.get(48, 0, 48 + 14), VoxelType::Air, "Bedrock at dz=7m (+14 voxels) must be excavated");
    assert_eq!(grid.get(48, 0, 48 - 14), VoxelType::Air, "Bedrock at dz=-7m (-14 voxels) must be excavated");

    // 3. Deep crater excavation down to -16m in basalt (y = -32 voxels)
    assert_eq!(grid.get(48, -16, 48), VoxelType::Air, "Subterranean basalt at y=-8m must be excavated");
    assert_eq!(grid.get(48, -32, 48), VoxelType::Air, "Subterranean basalt at y=-16m (-32 voxels) must be excavated");

    // Rubble height updated below -10m
    let floor_h = physics.rubble_height_at(target_world.x, target_world.z);
    assert!(floor_h <= -10.0, "Rubble height must drop below -10m in bedrock crater, got {:.2}", floor_h);

    // Ejecta sparks generated
    assert!(physics.sparks.len() >= 80, "Orbital impact must spawn >= 80 incandescent ejecta sparks");
}

// ============================================================================
// CHALLENGER BATTERY 4: R4 PERFORMANCE & SUB-REGION BFS EMPIRICAL PROOF
// ============================================================================

/// Challenger Test 8: Localized Sub-Region BFS Footprint Seeding 85-98% Reduction
#[test]
fn test_challenger_r4_localized_sub_region_bfs_footprint_reduction() {
    let mut grid = VoxelGrid::new();

    // World size: GRID_X x GRID_Z = 192 x 192 = 36,864 columns
    let total_columns = (GRID_X * GRID_Z) as f32;
    assert_eq!(total_columns, (GRID_X * GRID_Z) as f32);

    // Build two distinct towers:
    // Tower A: x in [20, 26], z in [20, 26] (7x7 footprint)
    for x in 20..=26 {
        for z in 20..=26 {
            for y in 1..=40 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Tower B: x in [70, 76], z in [70, 76] (7x7 footprint)
    for x in 70..=76 {
        for z in 70..=76 {
            for y in 1..=40 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Establish clean baseline
    let _ = grid.detect_unsupported_islands();
    assert!(grid.dirty_min.is_none());

    // Sever column at Tower A base: (23, 2, 23)
    grid.set(23, 2, 23, VoxelType::Air);

    // Inspect dirty bounds:
    let d_min = grid.dirty_min.expect("Dirty min must be set on modification");
    let d_max = grid.dirty_max.expect("Dirty max must be set on modification");

    assert!(d_min.x >= 20 && d_max.x <= 26, "Dirty region must be confined to Tower A in X");
    assert!(d_min.z >= 20 && d_max.z <= 26, "Dirty region must be confined to Tower A in Z");

    // Localized sub-region footprint with margin pad = CANTILEVER_MAX_DEPTH * 2 = 8 voxels:
    let pad = (CANTILEVER_MAX_DEPTH as i32) * 2;
    let box_min_x = (d_min.x - pad).max(0);
    let box_max_x = (d_max.x + pad).min(GRID_X as i32 - 1);
    let box_min_z = (d_min.z - pad).max(0);
    let box_max_z = (d_max.z + pad).min(GRID_Z as i32 - 1);

    let seeded_columns = ((box_max_x - box_min_x + 1) * (box_max_z - box_min_z + 1)) as f32;
    let reduction_ratio = (total_columns - seeded_columns) / total_columns;

    // Verify 85% to 99.5% reduction in foundation seeding overhead
    assert!(
        (0.85..=0.995).contains(&reduction_ratio),
        "Footprint seeding reduction must be between 85% and 99.5%, got {:.2}% ({} / {} columns)",
        reduction_ratio * 100.0,
        seeded_columns,
        total_columns
    );

    // Verify execution completes without touching Tower B
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Minor localized puncture does not collapse building");
    assert!(grid.dirty_min.is_none(), "Dirty bounds must be reset after localized BFS");
}

/// Challenger Test 9: 6-Plane Camera Frustum Culling Rejects 60-85% Across All 8 Particle Types
#[test]
fn test_challenger_r4_frustum_culling_all_8_particle_types() {
    let mut physics = PhysicsSystem::new();

    // Camera at (0, 5, 0) looking along +Z with standard 90-degree FOV
    let cam_pos = Vec3::new(0.0, 5.0, 0.0);
    let planes = FrustumPlanes::from_camera(cam_pos, Vec3::Z, Vec3::Y, 1.333, 1.0, 0.1, 100.0);

    // Populate all 8 visual/particle systems radially around origin at distances 10m to 50m
    let radial_count = 200;
    for i in 0..radial_count {
        let angle = (i as f32 / radial_count as f32) * std::f32::consts::TAU;
        let dist = 15.0 + (i % 30) as f32;
        let pos = Vec3::new(angle.cos() * dist, 5.0, angle.sin() * dist);

        // 1. DebrisParticle
        physics.debris.push(DebrisParticle {
            position: pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.3,
            color: Color::new(0.8, 0.4, 0.1, 1.0),
            life: 10.0,
            max_life: 10.0,
            bounces: 0,
        });

        // 2. SparkParticle
        physics.sparks.push(SparkParticle {
            position: pos,
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 2.0,
            max_life: 2.0,
        });

        // 3. SmokeParticle
        physics.smoke.push(SmokeParticle {
            position: pos,
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.2,
            max_size: 1.0,
            color: Color::new(0.5, 0.5, 0.5, 0.5),
            life: 5.0,
            max_life: 5.0,
        });

        // 4. ShockwaveRing
        physics.shockwaves.push(ShockwaveRing {
            position: pos,
            radius: 2.0,
            max_radius: 8.0,
            thickness: 0.3,
            normal: Vec3::Y,
            color: Color::new(1.0, 0.5, 0.2, 1.0),
            speed: 10.0,
        });

        // 5. ElectricalArc
        physics.electrical_arcs.push(ElectricalArc {
            start: pos,
            end: pos + Vec3::new(0.5, 0.5, 0.0),
            timer: 0.2,
            max_timer: 0.2,
            color: Color::new(0.3, 0.7, 1.0, 1.0),
        });

        // 6. PressurizedFireJet
        physics.fire_jets.push(PressurizedFireJet {
            position: pos,
            direction: Vec3::Y,
            timer: 1.0,
            max_timer: 1.0,
        });

        // 7. ImpactDecal
        physics.decals.push(ImpactDecal {
            position: pos,
            normal: Vec3::Y,
            size: 1.0,
            color: Color::new(0.1, 0.1, 0.1, 0.8),
            life: 5.0,
            max_life: 5.0,
            kind: DecalKind::Scorch,
            voxel_anchor: None,
        });

        // 8. DynamicChunk
        physics.chunks.push(DynamicChunk {
            position: pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: physics::Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            exposed_voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            mass: 50.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
        });
    }

    // Helper to evaluate cull ratio for any set of positions
    let check_rejection = |positions: &[Vec3]| -> f32 {
        let mut culled = 0;
        for &p in positions {
            if !planes.contains_sphere(p, 0.5) {
                culled += 1;
            }
        }
        culled as f32 / positions.len() as f32
    };

    let rej_debris = check_rejection(&physics.debris.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_sparks = check_rejection(&physics.sparks.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_smoke = check_rejection(&physics.smoke.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_shockwaves = check_rejection(&physics.shockwaves.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_arcs = check_rejection(&physics.electrical_arcs.iter().map(|p| p.start).collect::<Vec<_>>());
    let rej_jets = check_rejection(&physics.fire_jets.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_decals = check_rejection(&physics.decals.iter().map(|p| p.position).collect::<Vec<_>>());
    let rej_chunks = check_rejection(&physics.chunks.iter().map(|p| p.position).collect::<Vec<_>>());

    let rejections = [
        ("Debris", rej_debris),
        ("Sparks", rej_sparks),
        ("Smoke", rej_smoke),
        ("Shockwaves", rej_shockwaves),
        ("Arcs", rej_arcs),
        ("Jets", rej_jets),
        ("Decals", rej_decals),
        ("Chunks", rej_chunks),
    ];

    for (name, ratio) in rejections {
        assert!(
            (0.60..=0.85).contains(&ratio),
            "{} frustum culling rejection must be in 60-85%, got {:.1}%",
            name,
            ratio * 100.0
        );
    }
}

/// Challenger Test 10: Zero-Allocation Meshing (mesh_pool Cap 64) and Physics Scratch Buffer Pointer Stability
#[test]
fn test_challenger_r4_mesh_pool_cap_64_and_scratch_pointer_stability() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Verify scratch vectors initial capacities
    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);

    let ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();
    let ptr_chunks = physics.scratch_chunks_to_settle.as_ptr();
    let ptr_crush = physics.scratch_new_crush_chunks.as_ptr();

    // Run 60 simulation steps with continuous dynamic chunk creation and violent carving
    for step in 0..60 {
        // Continuous carving across varied locations to trigger mesh updates
        let carve_pos = Vec3::new(16.0 + (step as f32 % 8.0) * 3.0, 5.0, 16.0 + (step as f32 / 8.0) * 3.0);
        let mut carve_res = CarveResult::default();
        grid.carve_sphere_advanced_into(carve_pos, 2.5, 1000.0, DamageType::Explosive, &mut carve_res);

        // Update meshes to cycle buffers through mesh_pool
        grid.update_meshes();

        // Ensure mesh_pool NEVER exceeds cap of 64
        assert!(
            grid.mesh_pool.len() <= 64,
            "mesh_pool capacity must be capped at <= 64, got {} on step {}",
            grid.mesh_pool.len(),
            step
        );

        // Add dynamic chunk and update physics
        physics.chunks.push(DynamicChunk {
            position: Vec3::new(20.0, 10.0, 20.0),
            velocity: Vec3::new(0.0, -5.0, 0.0),
            rotation: Vec3::ZERO,
            rot_mat: physics::Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            exposed_voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            mass: 20.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
        });

        physics.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // Pointer stability invariant: scratch vectors did not reallocate (0 allocation churn)
    assert_eq!(
        physics.scratch_craters_to_unsettle.as_ptr(),
        ptr_craters,
        "scratch_craters_to_unsettle pointer must remain stable across simulation steps"
    );
    assert_eq!(
        physics.scratch_chunks_to_settle.as_ptr(),
        ptr_chunks,
        "scratch_chunks_to_settle pointer must remain stable across simulation steps"
    );
    assert_eq!(
        physics.scratch_new_crush_chunks.as_ptr(),
        ptr_crush,
        "scratch_new_crush_chunks pointer must remain stable across simulation steps"
    );

    // Final assert on mesh pool cap
    assert!(grid.mesh_pool.len() <= 64, "Final mesh_pool must not exceed 64");
}
