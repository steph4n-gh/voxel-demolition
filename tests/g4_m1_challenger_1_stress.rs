#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DynamicChunk, PhysicsSystem};
use player::Player;
use voxel::{
    DamageType, VoxelGrid, VoxelType, CHUNKS_SUBTERRANEAN_Y, CHUNKS_SURFACE_Y, CHUNK_SIZE, GRID_X,
    GRID_Y, GRID_Z, MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES, MAX_VOXEL_Y, MIN_VOXEL_Y,
    VOID_BOUNDARY_Y, VOXEL_SIZE,
};

// ============================================================================
// CHALLENGER 1 SUITE: 100% DESTRUCTIBILITY, BEDROCK/BASALT SHATTERING & CRATERING
// ============================================================================

// ----------------------------------------------------------------------------
// Group 1: Material Registry & Destructibility Invariants
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_material_properties_and_destructibility() {
    let bedrock_props = VoxelType::Bedrock.properties();
    assert_eq!(bedrock_props.name, "REINFORCED GRANITE BEDROCK");
    assert_eq!(bedrock_props.max_health, 800.0);
    assert_eq!(bedrock_props.hardness, 9.8);
    assert_eq!(bedrock_props.toughness, 9.5);
    assert_eq!(bedrock_props.blast_threshold, 45.0);
    assert_eq!(bedrock_props.density, 10.5);
    assert_eq!(bedrock_props.thermal_soften, 0.4);
    assert_eq!(bedrock_props.conductivity, 0.0);
    assert!(!bedrock_props.is_emissive);
    assert!(VoxelType::Bedrock.is_destructible());
    assert!(VoxelType::Bedrock.is_solid());
    assert!(VoxelType::Bedrock.is_opaque());

    let basalt_props = VoxelType::Basalt.properties();
    assert_eq!(basalt_props.name, "SUBTERRANEAN BASALT SUBSTRATE");
    assert_eq!(basalt_props.max_health, 500.0);
    assert_eq!(basalt_props.hardness, 9.2);
    assert_eq!(basalt_props.toughness, 8.8);
    assert_eq!(basalt_props.blast_threshold, 32.0);
    assert_eq!(basalt_props.density, 8.8);
    assert_eq!(basalt_props.thermal_soften, 0.6);
    assert_eq!(basalt_props.conductivity, 0.0);
    assert!(!basalt_props.is_emissive);
    assert!(VoxelType::Basalt.is_destructible());
    assert!(VoxelType::Basalt.is_solid());
    assert!(VoxelType::Basalt.is_opaque());

    // 100% environmental destructibility: Every single solid voxel type must be destructible!
    let all_solids = [
        VoxelType::Concrete,
        VoxelType::SteelRebar,
        VoxelType::Brick,
        VoxelType::Glass,
        VoxelType::Explosive,
        VoxelType::Bedrock,
        VoxelType::CryoIce,
        VoxelType::GlowCore,
        VoxelType::HeavyArmor,
        VoxelType::CompositeRebar,
        VoxelType::PowerCapacitor,
        VoxelType::Basalt,
    ];
    for vt in all_solids {
        assert!(vt.is_destructible(), "{:?} must be destructible", vt);
        assert!(vt.is_solid(), "{:?} must be solid", vt);
    }
    assert!(!VoxelType::Air.is_destructible());
    assert!(!VoxelType::Air.is_solid());
}

#[test]
fn test_challenger_subterranean_grid_dimensions_and_constants() {
    assert_eq!(CHUNKS_SUBTERRANEAN_Y, 6);
    assert_eq!(CHUNKS_SURFACE_Y, 28);
    assert_eq!(CHUNKS_SUBTERRANEAN_Y + CHUNKS_SURFACE_Y, 34);
    assert_eq!(CHUNK_SIZE, 8);
    assert_eq!(GRID_X, 192);
    assert_eq!(GRID_Y, 272);
    assert_eq!(GRID_Z, 192);
    assert_eq!(MIN_VOXEL_Y, -48);
    assert_eq!(MAX_VOXEL_Y, 224);
    assert_eq!(VOID_BOUNDARY_Y, -24.0);

    // Verify default subterranean layer assignment in newly created grid
    let grid = VoxelGrid::new();

    // Surface bedrock slab at y = 0
    assert_eq!(grid.get(48, 0, 48), VoxelType::Bedrock);
    assert_eq!(grid.health[VoxelGrid::idx(48, 0, 48)], 800.0);

    // Subterranean basalt substrate at y in -40..=-1
    for y in [-1, -10, -20, -30, -40] {
        assert_eq!(grid.get(48, y, 48), VoxelType::Basalt, "y = {}", y);
        assert_eq!(grid.health[VoxelGrid::idx(48, y, 48)], 500.0, "health at y = {}", y);
    }

    // Bedrock deep foundation at y in -48..=-41
    for y in [-41, -45, -48] {
        assert_eq!(grid.get(48, y, 48), VoxelType::Bedrock, "y = {}", y);
        assert_eq!(grid.health[VoxelGrid::idx(48, y, 48)], 800.0, "health at y = {}", y);
    }

    // Sky above surface bedrock slab is Air
    for y in [1, 10, 50, 100, 223] {
        assert_eq!(grid.get(48, y, 48), VoxelType::Air, "y = {}", y);
    }
}

// ----------------------------------------------------------------------------
// Group 2: Bedrock & Basalt Shattering Under Explosive, Kinetic, and Thermal Attacks
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_bedrock_blast_deflection_and_shattering() {
    let mut grid = VoxelGrid::new();
    let idx = VoxelGrid::idx(48, 0, 48);

    // 1. Blast threshold: Bedrock blast_threshold is 45.0
    // Blast damage below 45.0 must deflect without causing any damage
    let destroyed = grid.damage_voxel(48, 0, 48, 44.0, DamageType::Explosive);
    assert!(!destroyed, "Blast under 45.0 should not destroy bedrock");
    assert_eq!(grid.health[idx], 800.0, "Health must remain 800.0 on deflected blast");

    // 2. Blast above threshold: 50.0 explosive
    // effective_damage = 50.0 / (9.5 * 0.22) = 50.0 / 2.09 = 23.9234
    let destroyed = grid.damage_voxel(48, 0, 48, 50.0, DamageType::Explosive);
    assert!(!destroyed);
    assert!((grid.health[idx] - (800.0 - 50.0 / 2.09)).abs() < 0.1);

    // 3. Multi-hit damage accumulation: repeated explosive strikes degrade bedrock health
    for _ in 0..10 {
        grid.damage_voxel(48, 0, 48, 100.0, DamageType::Explosive);
    }
    assert!(grid.health[idx] < 350.0, "Repeated hits must degrade bedrock health");
    assert_eq!(grid.get(48, 0, 48), VoxelType::Bedrock);

    // 4. Overkill blast shatters bedrock into Air
    let destroyed = grid.damage_voxel(48, 0, 48, 2000.0, DamageType::Explosive);
    assert!(destroyed, "Overkill blast must shatter bedrock");
    assert_eq!(grid.get(48, 0, 48), VoxelType::Air);
    assert_eq!(grid.health[idx], 0.0);
}

#[test]
fn test_challenger_basalt_blast_deflection_and_shattering() {
    let mut grid = VoxelGrid::new();
    let idx = VoxelGrid::idx(48, -10, 48);

    // 1. Blast threshold: Basalt blast_threshold is 32.0
    let destroyed = grid.damage_voxel(48, -10, 48, 30.0, DamageType::Explosive);
    assert!(!destroyed);
    assert_eq!(grid.health[idx], 500.0, "Health must remain 500.0 on deflected blast");

    // 2. Blast above threshold: 40.0 explosive
    // effective_damage = 40.0 / (8.8 * 0.22) = 40.0 / 1.936 = 20.66
    let destroyed = grid.damage_voxel(48, -10, 48, 40.0, DamageType::Explosive);
    assert!(!destroyed);
    assert!((grid.health[idx] - (500.0 - 40.0 / 1.936)).abs() < 0.1);

    // 3. Overkill blast shatters basalt into Air
    let destroyed = grid.damage_voxel(48, -10, 48, 1500.0, DamageType::Explosive);
    assert!(destroyed, "Overkill blast must shatter basalt");
    assert_eq!(grid.get(48, -10, 48), VoxelType::Air);
    assert_eq!(grid.health[idx], 0.0);
}

#[test]
fn test_challenger_kinetic_and_thermal_penetration_against_bedrock_and_basalt() {
    let mut grid = VoxelGrid::new();

    // Kinetic damage against Bedrock:
    // effective = raw / (props.hardness * 0.20 + props.toughness * 0.12)
    // = raw / (9.8 * 0.20 + 9.5 * 0.12) = raw / (1.96 + 1.14) = raw / 3.10
    // High-yield ordnance kinetic damage is 9000.0 (Bunker Buster) or 9500.0 (Orbital Relay).
    // 9000.0 / 3.10 = 2903.2 > 800.0 => 1-hit complete shatter!
    let destroyed = grid.damage_voxel(48, 0, 48, 9000.0, DamageType::Kinetic);
    assert!(destroyed, "Bunker buster kinetic blast must 1-hit shatter bedrock");
    assert_eq!(grid.get(48, 0, 48), VoxelType::Air);

    // Kinetic damage against Basalt:
    // effective = 9000.0 / (9.2 * 0.20 + 8.8 * 0.12) = 9000.0 / (1.84 + 1.056) = 9000.0 / 2.896 = 3107.7 > 500.0
    let destroyed = grid.damage_voxel(48, -10, 48, 9000.0, DamageType::Kinetic);
    assert!(destroyed, "Bunker buster kinetic blast must 1-hit shatter basalt");
    assert_eq!(grid.get(48, -10, 48), VoxelType::Air);

    // Thermal softening:
    // Bedrock thermal_soften is 0.4.
    // Basalt thermal_soften is 0.6.
    grid.set(30, 0, 30, VoxelType::Bedrock);
    let destroyed = grid.damage_voxel(30, 0, 30, 3000.0, DamageType::Thermal);
    // 3000.0 * 0.4 = 1200.0 > 800.0 => shatters!
    assert!(destroyed, "Plasma thermal ablation must shatter bedrock");
    assert_eq!(grid.get(30, 0, 30), VoxelType::Air);

    grid.set(30, -5, 30, VoxelType::Basalt);
    let destroyed = grid.damage_voxel(30, -5, 30, 1000.0, DamageType::Thermal);
    // 1000.0 * 0.6 = 600.0 > 500.0 => shatters!
    assert!(destroyed, "Plasma thermal ablation must shatter basalt");
    assert_eq!(grid.get(30, -5, 30), VoxelType::Air);
}

// ----------------------------------------------------------------------------
// Group 3: Subterranean Deep Cratering Down to -16m and -24m
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_deep_subterranean_crater_carving() {
    let mut grid = VoxelGrid::new();

    // Surface position (48, 0, 48) maps to world Vec3::new(0.25, 0.25, 0.25)
    let world_center = grid.voxel_to_world(IVec3::new(48, 0, 48));

    // Carve a crater with radius 6.0m (12 voxels deep) using high-yield explosive blast
    let res = grid.carve_sphere_advanced(world_center, 6.0, 3500.0, DamageType::Explosive);
    assert!(res.destroyed > 100, "Should destroy hundreds of voxels");

    // Surface bedrock at y = 0 must be destroyed
    assert_eq!(grid.get(48, 0, 48), VoxelType::Air);

    // Subterranean basalt at y = -1 and y = -5 must be carved into Air in 1 hit
    assert_eq!(grid.get(48, -1, 48), VoxelType::Air);
    assert_eq!(grid.get(48, -5, 48), VoxelType::Air);

    // Basalt at y = -10 is near the crater periphery (10 out of 12 voxels):
    // It takes heavy falloff damage on hit 1 (health < 250.0)
    let idx_10 = VoxelGrid::idx(48, -10, 48);
    assert!(grid.health[idx_10] < 250.0, "Health must be degraded by blast falloff");

    // Second strike shatters the damaged basalt at y = -10 into Air!
    let res2 = grid.carve_sphere_advanced(world_center, 6.0, 3500.0, DamageType::Explosive);
    assert!(res2.destroyed > 0);
    assert_eq!(grid.get(48, -10, 48), VoxelType::Air, "Second hit must shatter damaged basalt");

    // Voxels deeper than crater bottom should remain intact
    assert_eq!(grid.get(48, -25, 48), VoxelType::Basalt);
}

#[test]
fn test_challenger_subterranean_chasm_down_to_minus_16m() {
    let mut grid = VoxelGrid::new();

    // Detonate at subterranean depth Y = -16.0m (vpos y = -32)
    let det_pos = Vec3::new(0.0, -16.0, 0.0);
    let vpos = grid.world_to_voxel(det_pos);
    assert_eq!(vpos.y, -32);

    // Carve high-yield explosion at -16m
    let res = grid.carve_sphere_advanced(det_pos, 4.0, 4000.0, DamageType::Explosive);
    assert!(res.destroyed > 50);

    // Voxels around y = -32 (Basalt substrate) are shattered
    assert_eq!(grid.get(vpos.x, -32, vpos.z), VoxelType::Air);
    assert_eq!(grid.get(vpos.x, -30, vpos.z), VoxelType::Air);
    assert_eq!(grid.get(vpos.x, -34, vpos.z), VoxelType::Air);
}

#[test]
fn test_challenger_deep_void_boundary_excavation_down_to_minus_24m() {
    let mut grid = VoxelGrid::new();

    // Deepest bedrock layer is at MIN_VOXEL_Y = -48 (-24.0m)
    assert_eq!(grid.get(48, -48, 48), VoxelType::Bedrock);

    let deep_pos = grid.voxel_to_world(IVec3::new(48, -48, 48));
    assert!((deep_pos.y - (-23.75)).abs() < 0.01);

    // Overkill blast at the deepest void boundary
    let res = grid.carve_sphere_advanced(deep_pos, 2.5, 5000.0, DamageType::Explosive);
    assert!(res.destroyed > 0);

    // The deepest bedrock at -48 is shattered into Air!
    assert_eq!(grid.get(48, -48, 48), VoxelType::Air);

    // Bounds checking: verifying no panics on out-of-bounds queries past -48
    assert_eq!(grid.get(48, -49, 48), VoxelType::Air);
    assert_eq!(grid.get(48, -100, 48), VoxelType::Air);
}

// ----------------------------------------------------------------------------
// Group 4: Orbital Relay & Bunker Buster Ordnance Tests
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_orbital_kinetic_relay_excavates_subterranean_bedrock() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();

    let target_pos = Vec3::new(0.0, 0.5, 0.0);
    let beam_top = Vec3::new(target_pos.x, 140.0, target_pos.z);
    let beam_bot = Vec3::new(target_pos.x, -16.0, target_pos.z);

    // Fire dual orbital strike (kinetic core + thermal sheath)
    let res_core = grid.carve_cylinder_advanced(beam_top, beam_bot, 4.0, 9500.0, DamageType::Kinetic);
    let res_sheath = grid.carve_cylinder_advanced(beam_top, beam_bot, 6.0, 4500.0, DamageType::Thermal);
    physics.update_crater_floor(&grid, beam_bot, 8.0);

    let total_destroyed = res_core.destroyed + res_sheath.destroyed;
    assert!(total_destroyed > 100, "Orbital Relay must destroy subterranean bedrock and basalt");

    let center_v = grid.world_to_voxel(target_pos);
    // Surface bedrock at y = 0 shattered
    assert_eq!(grid.get(center_v.x, 0, center_v.z), VoxelType::Air);

    // Subterranean basalt at y = -16 (-8.0m) and y = -32 (-16.0m) shattered
    assert_eq!(grid.get(center_v.x, -16, center_v.z), VoxelType::Air);
    assert_eq!(grid.get(center_v.x, -32, center_v.z), VoxelType::Air);

    // Rubble height grid must reflect subterranean crater excavation
    let floor_h = physics.rubble_height_at(target_pos.x, target_pos.z);
    assert!(floor_h < -15.0, "Crater floor must reach below -15m; got {}", floor_h);
}

#[test]
fn test_challenger_bunker_buster_shell_excavates_deep_crater() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();

    let impact_pos = Vec3::new(5.0, 0.5, 5.0);
    let blast_r = 7.5;

    // Bunker buster shell impact: 9000.0 kinetic damage
    let res = grid.carve_sphere_advanced(impact_pos, blast_r, 9000.0, DamageType::Kinetic);
    physics.update_crater_floor(&grid, impact_pos, blast_r);

    assert!(res.destroyed > 100, "Bunker buster must shatter substantial volume");

    let vpos = grid.world_to_voxel(impact_pos);
    assert_eq!(grid.get(vpos.x, 0, vpos.z), VoxelType::Air);
    assert_eq!(grid.get(vpos.x, -5, vpos.z), VoxelType::Air);
    assert_eq!(grid.get(vpos.x, -10, vpos.z), VoxelType::Air);

    let floor_h = physics.rubble_height_at(impact_pos.x, impact_pos.z);
    assert!(floor_h < 0.0, "Crater floor must be subterranean (Y < 0.0m); got {}", floor_h);
}

// ----------------------------------------------------------------------------
// Group 5: Rubble Height Grid Accuracy & Dynamic Chunk Settling
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_rubble_height_grid_accurate_tracking() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();

    let pos = Vec3::new(10.0, 0.5, 10.0);
    // Initial floor height before carving
    let initial_h = physics.rubble_height_at(pos.x, pos.z);
    assert_eq!(initial_h, VOXEL_SIZE); // 0.5m

    // Step 1: Carve shallow crater (3 voxels deep, down to y = -3)
    let vcenter = grid.world_to_voxel(pos);
    for dy in -3..=0 {
        grid.set(vcenter.x, dy, vcenter.z, VoxelType::Air);
    }
    physics.update_crater_floor(&grid, pos, 1.5);

    // Highest remaining solid voxel in this column is y = -4
    // Top of y = -4 is: -4 * 0.5 + 0.25 + 0.25 = -1.5m
    let h1 = physics.rubble_height_at(pos.x, pos.z);
    assert!((h1 - (-1.5)).abs() < 0.01, "Expected -1.5m, got {}", h1);

    // Step 2: Carve down to y = -20 (-10.0m)
    for dy in -20..=-4 {
        grid.set(vcenter.x, dy, vcenter.z, VoxelType::Air);
    }
    physics.update_crater_floor(&grid, pos, 1.5);

    // Highest remaining solid voxel is y = -21
    // Top of y = -21 is: -21 * 0.5 + 0.25 + 0.25 = -10.0m
    let h2 = physics.rubble_height_at(pos.x, pos.z);
    assert!((h2 - (-10.0)).abs() < 0.01, "Expected -10.0m, got {}", h2);

    // Step 3: Complete punch-through (all voxels down to MIN_VOXEL_Y = -48 removed)
    for dy in -48..=-21 {
        grid.set(vcenter.x, dy, vcenter.z, VoxelType::Air);
    }
    physics.update_crater_floor(&grid, pos, 1.5);

    // With no solid voxels remaining, must clamp to VOID_BOUNDARY_Y (-24.0m)
    let h3 = physics.rubble_height_at(pos.x, pos.z);
    assert_eq!(h3, VOID_BOUNDARY_Y, "Must clamp to VOID_BOUNDARY_Y on total punch-through");
}

#[test]
fn test_challenger_dynamic_chunk_settles_inside_subterranean_crater() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let center = Vec3::new(0.0, 0.0, 0.0);
    // Carve crater down to y = -16 (-8.0m)
    let vcenter = grid.world_to_voxel(center);
    for dx in -2..=2 {
        for dz in -2..=2 {
            for dy in -16..=0 {
                grid.set(vcenter.x + dx, dy, vcenter.z + dz, VoxelType::Air);
            }
        }
    }
    physics.update_crater_floor(&grid, center, 3.0);
    let floor_h = physics.rubble_height_at(0.0, 0.0);
    assert!(floor_h < -7.5, "Crater floor should be ~ -8.0m");

    // Spawn a dynamic chunk above the crater at y = -2.0m falling downward
    let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let chunk = DynamicChunk {
        voxels: vox.clone(),
        exposed_voxels: vox,
        voxel_health: Vec::new(),
        position: Vec3::new(0.0, -2.0, 0.0),
        velocity: Vec3::new(0.0, -5.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        mass: 50.0,
        settled: false,
        life: 0.0,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    };
    physics.chunks.push(chunk);

    // Step physics forward until the chunk settles
    for _ in 0..120 {
        physics.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));
    }

    assert_eq!(physics.chunks.len(), 1);
    let settled_chunk = &physics.chunks[0];
    assert!(settled_chunk.settled, "Chunk must settle inside subterranean crater");
    // Position must be resting on the subterranean floor (~ -8.0m)
    assert!(
        settled_chunk.position.y < -7.0 && settled_chunk.position.y > -9.0,
        "Chunk position {} must match crater floor {}",
        settled_chunk.position.y,
        floor_h
    );
}

// ----------------------------------------------------------------------------
// Group 6: Extreme Blast Radii & Edge Cases
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_extreme_blast_radius_across_subterranean_chunks() {
    let mut grid = VoxelGrid::new();

    // Colossal blast with radius 120.0m (exceeding entire expanded world bounds)
    // Centered at subterranean level Y = -12.0m
    let det_pos = Vec3::new(0.0, -12.0, 0.0);
    let res = grid.carve_sphere_advanced(det_pos, 120.0, 15000.0, DamageType::Explosive);

    // Must destroy massive number of voxels without panicking or index out of bounds
    assert!(res.destroyed > 1000);

    // Verify grid boundary voxels are handled gracefully
    assert_eq!(grid.get(0, -48, 0), VoxelType::Air);
    assert_eq!(grid.get(GRID_X as i32 - 1, -48, GRID_Z as i32 - 1), VoxelType::Air);
}

#[test]
fn test_challenger_boundary_coordinates_exhaustive_access() {
    let mut grid = VoxelGrid::new();

    // Check all 8 extreme corners of the bounding box
    let corners = [
        (0, MIN_VOXEL_Y, 0),
        (GRID_X as i32 - 1, MIN_VOXEL_Y, 0),
        (0, MIN_VOXEL_Y, GRID_Z as i32 - 1),
        (GRID_X as i32 - 1, MIN_VOXEL_Y, GRID_Z as i32 - 1),
        (0, MAX_VOXEL_Y - 1, 0),
        (GRID_X as i32 - 1, MAX_VOXEL_Y - 1, 0),
        (0, MAX_VOXEL_Y - 1, GRID_Z as i32 - 1),
        (GRID_X as i32 - 1, MAX_VOXEL_Y - 1, GRID_Z as i32 - 1),
    ];

    for (x, y, z) in corners {
        assert!(grid.in_bounds(x, y, z));
        let _ = grid.get(x, y, z);
        grid.set(x, y, z, VoxelType::Basalt);
        assert_eq!(grid.get(x, y, z), VoxelType::Basalt);
        let destroyed = grid.damage_voxel(x, y, z, 5000.0, DamageType::Kinetic);
        assert!(destroyed);
        assert_eq!(grid.get(x, y, z), VoxelType::Air);
    }

    // Check one step outside bounds: must return Air, false in_bounds, no panic
    let out_of_bounds = [
        (-1, 0, 0),
        (GRID_X as i32, 0, 0),
        (0, MIN_VOXEL_Y - 1, 0),
        (0, MAX_VOXEL_Y, 0),
        (0, 0, -1),
        (0, 0, GRID_Z as i32),
    ];

    for (x, y, z) in out_of_bounds {
        assert!(!grid.in_bounds(x, y, z));
        assert_eq!(grid.get(x, y, z), VoxelType::Air);
        assert!(!grid.damage_voxel(x, y, z, 1000.0, DamageType::Explosive));
    }
}

// ----------------------------------------------------------------------------
// Group 7: Subterranean Chunk Meshing & Sub-Batch Invariants
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_subterranean_chunk_meshing_generates_crater_geometry() {
    let mut grid = VoxelGrid::new();

    // Check pristine subterranean chunk (cx = 6, cy = 2, cz = 6)
    // cy = 2 corresponds to y in [-32..=-25] (solid basalt everywhere)
    // Because all neighbors are solid, no faces should be exposed.
    let ci = VoxelGrid::chunk_idx(6, 2, 6);
    grid.update_meshes();
    assert_eq!(
        grid.meshes[ci].sub_meshes.len(),
        0,
        "Pristine subterranean chunk should have 0 exposed faces"
    );

    // Carve a subterranean cave inside chunk (6, 2, 6)
    let start_x = 6 * CHUNK_SIZE as i32;
    let start_y = MIN_VOXEL_Y + (2 * CHUNK_SIZE) as i32;
    let start_z = 6 * CHUNK_SIZE as i32;

    // Excavate the interior (2x2x2) voxels of the chunk
    for dx in 2..=4 {
        for dy in 2..=4 {
            for dz in 2..=4 {
                grid.set(start_x + dx, start_y + dy, start_z + dz, VoxelType::Air);
            }
        }
    }

    // Update meshes
    grid.update_meshes();

    // Now chunk (6, 2, 6) must have exposed interior faces!
    assert!(
        !grid.meshes[ci].sub_meshes.is_empty(),
        "Carved subterranean cavity must generate sub-meshes"
    );
    assert!(
        !grid.meshes[ci].sub_meshes[0].vertices.is_empty(),
        "Carved subterranean cavity must generate mesh vertices"
    );
    assert!(
        !grid.meshes[ci].sub_meshes[0].indices.is_empty(),
        "Carved subterranean cavity must generate mesh indices"
    );

    // Verify sub-batch partitioning on chunk
    for sub in &grid.meshes[ci].sub_meshes {
        assert!(sub.vertices.len() <= MAX_CHUNK_BATCH_VERTICES);
        assert!(sub.indices.len() <= MAX_CHUNK_BATCH_INDICES);
    }
}

// ----------------------------------------------------------------------------
// Group 8: Player Subterranean Navigation, Step-up & Void Reset
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_player_subterranean_crater_navigation() {
    let mut grid = VoxelGrid::new();
    let mut player = Player::new(Vec3::new(0.0, 5.0, 0.0));

    // Excavate a subterranean crater at (0, 0, 0) down to y = -20 (-10.0m)
    let vcenter = grid.world_to_voxel(Vec3::ZERO);
    for dx in -3..=3 {
        for dz in -3..=3 {
            for dy in -20..=0 {
                grid.set(vcenter.x + dx, dy, vcenter.z + dz, VoxelType::Air);
            }
        }
    }

    let floor_y = -21;
    let v_world = grid.voxel_to_world(IVec3::new(vcenter.x, floor_y, vcenter.z));
    let expected_ground = v_world.y + VOXEL_SIZE * 0.5;

    // Player placed on the subterranean crater floor
    player.position = Vec3::new(v_world.x, expected_ground + 0.02, v_world.z);
    player.velocity = Vec3::ZERO;

    player.update(0.016, &grid, None, false);

    // Player should be grounded on the subterranean crater floor
    assert!(player.is_grounded, "Player must be grounded on subterranean crater floor");
    assert!(
        (player.position.y - expected_ground).abs() < 0.01,
        "Player y {} must match crater floor {}",
        player.position.y,
        expected_ground
    );
}

#[test]
fn test_challenger_player_crater_step_up_and_wall_slide() {
    let mut grid = VoxelGrid::new();
    let vcenter = grid.world_to_voxel(Vec3::new(0.0, -10.0, 0.0));
    let mut player = Player::new(Vec3::new(0.0, -10.0, 0.0));

    // Clear area at y = -20 (-10.0m)
    for dx in -3..=3 {
        for dz in -3..=3 {
            for dy in -20..=0 {
                grid.set(vcenter.x + dx, dy, vcenter.z + dz, VoxelType::Air);
            }
        }
    }

    // Place a 1-voxel step (0.5m) in front of player at x + 1, y = -20
    // Ledge height = -20 * 0.5 + 0.5 = -9.5m (within MAX_STEP_UP = 0.55m)
    grid.set(vcenter.x + 1, -20, vcenter.z, VoxelType::Basalt);

    let start_w = grid.voxel_to_world(IVec3::new(vcenter.x, -21, vcenter.z));
    player.position = Vec3::new(start_w.x + 0.15, -10.0, start_w.z);
    player.velocity = Vec3::new(5.0, 0.0, 0.0); // Moving forward towards ledge

    // Update player
    player.update(0.05, &grid, None, false);

    // Player should step up onto the ledge (position.y rises to -9.5m)
    assert!(
        player.position.y >= -9.55,
        "Player must step up onto the 0.5m ledge; position.y = {}",
        player.position.y
    );
}

#[test]
fn test_challenger_player_void_boundary_safe_respawn() {
    let grid = VoxelGrid::new();
    let mut player = Player::new(Vec3::new(0.0, 5.0, 0.0));
    player.spawn_pos = Vec3::new(0.0, 5.0, 0.0);

    // Drop player past the safe void boundary Y <= -24.0m
    player.position = Vec3::new(0.0, -24.5, 0.0);
    player.velocity = Vec3::new(0.0, -30.0, 0.0);

    // Update player
    player.update(1.0 / 60.0, &grid, None, false);

    // Player must be safely reset to spawn_pos with zero velocity
    assert_eq!(player.position, player.spawn_pos);
    assert_eq!(player.velocity, Vec3::ZERO);
    assert!(player.just_respawned, "just_respawned flag must be set");
}
