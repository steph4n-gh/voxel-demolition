#![allow(clippy::new_without_default)]
#![allow(dead_code, unused_imports, clippy::manual_range_contains)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DebrisParticle, DynamicChunk, DynamicPointLight, PhysicsSystem, SmokeParticle, SparkParticle};
use siege::{SiegeEngineKind, SiegeSystem};
use voxel::{DamageType, VoxelGrid, VoxelType, MIN_VOXEL_Y, VOXEL_SIZE};
use weapon::{ActiveSingularity, WeaponSystem};

// ============================================================================
// CHALLENGER 2 EMPIRICAL STRESS TEST SUITE: G4-M3 ARSENAL
// 1. Event Horizon Singularity (35m Vortex, Multi-Particle Suction, Core Erosion, Collapse Implosion)
// 2. Orbital Kinetic Relay (Multi-Floor Vertical Bore, 16m Bedrock Crater, Rubble Floor Excavation)
// ============================================================================

// ----------------------------------------------------------------------------
// Group 1: Event Horizon Singularity 35m Vortex & 30m Distance Multi-Entity Acceleration
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_singularity_35m_suction_boundary_and_30m_multi_entity_acceleration() {
    assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0, "Singularity pull radius must be strictly 35.0m");
    assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0, "Singularity pull strength must be strictly 72.0 m/s^2");

    let mut physics = PhysicsSystem::new();
    let vortex_center = Vec3::new(60.0, 25.0, 60.0);
    let dt = 0.1;

    // Place entities at exactly 30.0m distance in different directions:
    // 1. Dynamic chunk at +X (dist = 30.0m)
    let chunk_pos = vortex_center + Vec3::new(30.0, 0.0, 0.0);
    physics.chunks.push(DynamicChunk {
        position: chunk_pos,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 120.0,
        life: 15.0,
        settled: true,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // 2. Debris particle at -X (dist = 30.0m)
    let debris_pos = vortex_center + Vec3::new(-30.0, 0.0, 0.0);
    physics.debris.push(DebrisParticle {
        position: debris_pos,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        size: 0.35,
        color: Color::new(0.8, 0.3, 0.1, 1.0),
        life: 5.0,
        max_life: 5.0,
        bounces: 0,
    });

    // 3. Smoke particle at +Z (dist = 30.0m)
    let smoke_pos = vortex_center + Vec3::new(0.0, 0.0, 30.0);
    physics.smoke.push(SmokeParticle {
        position: smoke_pos,
        velocity: Vec3::ZERO,
        size: 1.2,
        start_size: 1.2,
        max_size: 3.5,
        color: Color::new(0.1, 0.1, 0.1, 0.7),
        life: 4.0,
        max_life: 4.0,
    });

    // 4. Spark particle at -Z (dist = 30.0m)
    let spark_pos = vortex_center + Vec3::new(0.0, 0.0, -30.0);
    physics.sparks.push(SparkParticle {
        position: spark_pos,
        velocity: Vec3::ZERO,
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 2.0,
        max_life: 2.0,
    });

    // 5. Boundary Stress: entity at 34.9m (inside pull boundary) vs 35.2m (outside pull boundary)
    let inside_near_edge_pos = vortex_center + Vec3::new(34.9, 0.0, 0.0);
    physics.chunks.push(DynamicChunk {
        position: inside_near_edge_pos,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 100.0,
        life: 15.0,
        settled: true,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    let outside_edge_pos = vortex_center + Vec3::new(35.2, 0.0, 0.0);
    physics.chunks.push(DynamicChunk {
        position: outside_edge_pos,
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 100.0,
        life: 15.0,
        settled: true,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // Run 1 simulation tick of vortex suction
    physics.apply_vortex_suction(vortex_center, ActiveSingularity::PULL_RADIUS, ActiveSingularity::PULL_STRENGTH, dt);

    // Dynamic Chunk at 30m (+X):
    let chunk_at_30m = &physics.chunks[0];
    assert!(!chunk_at_30m.settled, "Chunk at 30m must be forcefully unsettled");
    assert!(chunk_at_30m.velocity.x < 0.0, "Chunk at +X=30m must accelerate in -X toward core, got vx={:.3}", chunk_at_30m.velocity.x);
    assert!(chunk_at_30m.velocity.y > 0.0, "Chunk must receive anti-gravity lift, got vy={:.3}", chunk_at_30m.velocity.y);
    assert!(chunk_at_30m.velocity.z.abs() > 0.001, "Chunk must receive tangential swirl velocity");

    // Debris Particle at 30m (-X):
    let debris_at_30m = &physics.debris[0];
    assert!(debris_at_30m.velocity.x > 0.0, "Debris at -X=30m must accelerate in +X toward core, got vx={:.3}", debris_at_30m.velocity.x);

    // Smoke Particle at 30m (+Z):
    let smoke_at_30m = &physics.smoke[0];
    assert!(smoke_at_30m.velocity.z < 0.0, "Smoke at +Z=30m must accelerate in -Z toward core, got vz={:.3}", smoke_at_30m.velocity.z);

    // Spark Particle at 30m (-Z):
    let spark_at_30m = &physics.sparks[0];
    assert!(spark_at_30m.velocity.z > 0.0, "Spark at -Z=30m must accelerate in +Z toward core, got vz={:.3}", spark_at_30m.velocity.z);

    // Boundary Validation:
    let chunk_inside_edge = &physics.chunks[1];
    assert!(!chunk_inside_edge.settled, "Chunk at 34.9m must be unsettled");
    assert!(chunk_inside_edge.velocity.x < 0.0, "Chunk at 34.9m must accelerate toward core");

    let chunk_outside_edge = &physics.chunks[2];
    assert!(chunk_outside_edge.settled, "Chunk at 35.2m must remain settled");
    assert_eq!(chunk_outside_edge.velocity, Vec3::ZERO, "Chunk at 35.2m must have zero velocity");
}

#[test]
fn test_challenger_singularity_core_damping_prevents_slingshot() {
    let mut physics = PhysicsSystem::new();
    let vortex_center = Vec3::new(40.0, 15.0, 40.0);

    // Place a high-speed chunk very close to core (< 3.2m)
    let core_chunk_pos = vortex_center + Vec3::new(1.5, 0.0, 0.0);
    physics.chunks.push(DynamicChunk {
        position: core_chunk_pos,
        velocity: Vec3::new(0.0, 0.0, 50.0), // High tangential velocity
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 50.0,
        life: 10.0,
        settled: false,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    let initial_speed = physics.chunks[0].velocity.length();
    for _ in 0..10 {
        physics.apply_vortex_suction(vortex_center, ActiveSingularity::PULL_RADIUS, ActiveSingularity::PULL_STRENGTH, 0.05);
    }
    let final_speed = physics.chunks[0].velocity.length();
    // Core damping should restrain the velocity
    assert!(final_speed < initial_speed * 1.5, "Core damping must prevent slingshot velocity explosion");
}

#[test]
fn test_challenger_singularity_core_erosion_expanding_spheres_and_950_collapse_into_bedrock() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Place singularity directly at bedrock level (y=0) surrounded by Granite Bedrock and Basalt
    let sing_pos = grid.voxel_to_world(IVec3::new(48, 0, 48));

    // Bedrock has 800 health, Basalt has 500 health.
    // Verify voxel grid properties at this location
    let center_voxel = grid.world_to_voxel(sing_pos);
    assert_eq!(grid.get(center_voxel.x, 0, center_voxel.z), VoxelType::Bedrock);
    assert_eq!(grid.get(center_voxel.x, -5, center_voxel.z), VoxelType::Basalt);

    // Add singularity with timer = 0.0s, max_time = 2.4s, blast_radius = 20.0m
    weapon.singularities.push(ActiveSingularity {
        position: sing_pos,
        timer: 0.0,
        max_time: 2.4,
        pull_radius: ActiveSingularity::PULL_RADIUS,
        pull_strength: ActiveSingularity::PULL_STRENGTH,
        blast_radius: 20.0,
        vacuum_played: false,
        erosion_timer: 0.0,
    });

    // 1. Advance by 0.16s: triggers first core erosion tick at radius ~2.5m with 220 kinetic damage
    weapon.update(0.16, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.singularities.len(), 1);

    // 2. Advance to 1.20s (50% progress): core erosion radius ~ 2.5 + 0.5 * 3.5 = 4.25m
    for _ in 0..10 {
        weapon.update(0.104, &mut grid, &mut physics, &audio, Vec3::ZERO);
    }
    assert_eq!(weapon.singularities.len(), 1);

    // 3. Advance to near collapse (2.38s)
    let remaining_time = 2.38 - weapon.singularities[0].timer;
    if remaining_time > 0.0 {
        weapon.update(remaining_time, &mut grid, &mut physics, &audio, Vec3::ZERO);
    }
    assert_eq!(weapon.singularities.len(), 1);

    // 4. Final step crossing 2.40s: Catastrophic Collapse Implosion with 950 damage
    weapon.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(weapon.singularities.is_empty(), "Singularity must implode and remove itself upon timer >= max_time");

    // Check crater collapse radius:
    // Collapse radius was clamped between COLLAPSE_RADIUS_MIN (16.0m) and COLLAPSE_RADIUS_MAX (22.0m).
    // With blast_radius = 20.0m, collapse_r = 20.0m (40 voxels diameter / 20 voxels radius).
    // Damage = 950.0, which breaches Reinforced Granite Bedrock (800 HP) and Basalt Substrate (500 HP)!

    // Check that bedrock at center (y = 0) is destroyed by erosion + collapse:
    assert_eq!(
        grid.get(center_voxel.x, 0, center_voxel.z),
        VoxelType::Air,
        "Center bedrock must be obliterated by erosion + 950 collapse damage"
    );

    // Check that basalt within the core erosion + collapse zone (e.g. y = -2.0m, vy = -4) is obliterated:
    assert_eq!(
        grid.get(center_voxel.x, -4, center_voxel.z),
        VoxelType::Air,
        "Subterranean basalt at y=-2.0m must be obliterated inside the erosion + collapse zone"
    );

    // Check that basalt at deeper distance (y = -10.0m) took severe damage from the 950 collapse blast:
    let basalt_vy = ((-10.0) / VOXEL_SIZE).round() as i32;
    let basalt_health = grid.get_health(center_voxel.x, basalt_vy, center_voxel.z);
    assert!(
        basalt_health < 500.0,
        "Subterranean basalt at y=-10.0m must receive damage from 950 collapse blast, got health {:.1}/500.0",
        basalt_health
    );
    assert!(
        basalt_health <= 300.0,
        "Subterranean basalt at y=-10.0m must take >= 200 damage from 950 collapse blast, got health {:.1}/500.0",
        basalt_health
    );

    // Verify rubble floor was updated below ground level:
    let crater_rubble_h = physics.rubble_height_at(sing_pos.x, sing_pos.z);
    assert!(
        crater_rubble_h < 0.0,
        "Rubble heightmap must be excavated down into subterranean space, got {:.2}m",
        crater_rubble_h
    );

    // Verify catastrophic chromatic shockwave was produced:
    assert!(!physics.shockwaves.is_empty(), "Collapse must create chromatic shockwave");
    let max_shockwave_r = physics.shockwaves.iter().map(|s| s.max_radius).fold(0.0f32, f32::max);
    assert!(
        max_shockwave_r >= 20.0,
        "Shockwave radius must expand to >= 20.0m, got {:.2}m",
        max_shockwave_r
    );
}

// ----------------------------------------------------------------------------
// Group 2: Orbital Kinetic Relay Multi-Floor Penetration & 16m Bedrock Crater
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_orbital_relay_punches_through_8_story_skyscraper_down_to_bedrock() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct an 8-story skyscraper at (x=60, z=60):
    // Heights:
    // Floor 1: y = 2m  (vy = 4)
    // Floor 2: y = 6m  (vy = 12)
    // Floor 3: y = 10m (vy = 20)
    // Floor 4: y = 14m (vy = 28)
    // Floor 5: y = 18m (vy = 36)
    // Floor 6: y = 22m (vy = 44)
    // Floor 7: y = 26m (vy = 52)
    // Roof:    y = 30m (vy = 60)
    // Columns made of SteelRebar, floor slabs of Concrete and CompositeRebar.
    for vy in 1..=62 {
        for dx in -2i32..=2i32 {
            for dz in -2i32..=2i32 {
                let vx = 60 + dx;
                let vz = 60 + dz;
                // Columns at corners
                if (dx.abs() == 2 && dz.abs() == 2) || (dx == 0 && dz == 0) {
                    grid.set(vx, vy, vz, VoxelType::SteelRebar);
                } else if vy % 8 == 0 || vy == 60 {
                    // Floor slabs
                    grid.set(vx, vy, vz, VoxelType::CompositeRebar);
                } else {
                    grid.set(vx, vy, vz, VoxelType::Concrete);
                }
            }
        }
    }

    // Ground and subterranean layers:
    // y = 0: Reinforced Granite Bedrock
    // y < 0: Basalt Substrate down to y = -24.0m
    assert_eq!(grid.get(60, 0, 60), VoxelType::Bedrock);
    assert_eq!(grid.get(60, -10, 60), VoxelType::Basalt);
    assert_eq!(grid.get(60, -20, 60), VoxelType::Basalt);

    // Aim Orbital Kinetic Relay specifically at the skyscraper roof (y = 30.0m)
    let roof_target = grid.voxel_to_world(IVec3::new(60, 60, 60));
    assert!(roof_target.y >= 30.0, "Target must be on the roof");

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(20.0, 0.5, 20.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(roof_target), &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert_eq!(siege.orbital_strikes.len(), 1);
    // Instant strike execution
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // Verify vertical strike punched through EVERY SINGLE FLOOR without stopping at the roof:
    assert_eq!(grid.get(60, 60, 60), VoxelType::Air, "Roof (vy=60, y=30m) must be vaporized");
    assert_eq!(grid.get(60, 52, 60), VoxelType::Air, "Floor 7 (vy=52, y=26m) must be vaporized");
    assert_eq!(grid.get(60, 44, 60), VoxelType::Air, "Floor 6 (vy=44, y=22m) must be vaporized");
    assert_eq!(grid.get(60, 36, 60), VoxelType::Air, "Floor 5 (vy=36, y=18m) must be vaporized");
    assert_eq!(grid.get(60, 28, 60), VoxelType::Air, "Floor 4 (vy=28, y=14m) must be vaporized");
    assert_eq!(grid.get(60, 20, 60), VoxelType::Air, "Floor 3 (vy=20, y=10m) must be vaporized");
    assert_eq!(grid.get(60, 12, 60), VoxelType::Air, "Floor 2 (vy=12, y=6m) must be vaporized");
    assert_eq!(grid.get(60, 4, 60), VoxelType::Air, "Floor 1 (vy=4, y=2m) must be vaporized");

    // Verify it punched through bedrock (y = 0) and into basalt (y = -10m, -16m):
    assert_eq!(grid.get(60, 0, 60), VoxelType::Air, "Bedrock slab (vy=0, y=0m) must be vaporized");
    assert_eq!(grid.get(60, -10, 60), VoxelType::Air, "Basalt (vy=-10, y=-5m) must be vaporized");
    assert_eq!(grid.get(60, -20, 60), VoxelType::Air, "Basalt (vy=-20, y=-10m) must be vaporized");
    assert_eq!(grid.get(60, -32, 60), VoxelType::Air, "Basalt down to -16.0m (vy=-32) must be excavated");
}

#[test]
fn test_challenger_orbital_relay_16m_bedrock_crater_exact_geometry_and_rubble_depth() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let target_vx = 55;
    let target_vz = 55;
    let target_pos = grid.voxel_to_world(IVec3::new(target_vx, 0, target_vz));

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(15.0, 0.5, 15.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(target_pos), &mut grid, &mut physics, &audio, Vec3::ZERO);

    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // 1. Check Crater Diameter at Y = 0 (Bedrock level):
    // Required: 16-meter diameter (R = 8.0m, which is 16 voxels radius at 0.5m/voxel).
    // Center at target_vx, target_vz.
    // All voxels strictly inside R < 8.0m (i.e. distance < 15.5 voxels) MUST be Air.
    for dx in -15..=15 {
        for dz in -15..=15 {
            let dist_vox = ((dx * dx + dz * dz) as f32).sqrt();
            let dist_m = dist_vox * VOXEL_SIZE;
            if dist_m < 7.8 {
                assert_eq!(
                    grid.get(target_vx + dx, 0, target_vz + dz),
                    VoxelType::Air,
                    "Bedrock at offset ({}, {}) (dist={:.2}m) must be completely excavated in 16m crater",
                    dx, dz, dist_m
                );
            }
        }
    }

    // Voxels outside the blast radius (e.g. at R = 11.0m / 22 voxels) must remain intact Bedrock:
    assert_eq!(
        grid.get(target_vx + 22, 0, target_vz),
        VoxelType::Bedrock,
        "Crater rim at R=11.0m must remain intact"
    );
    assert_eq!(
        grid.get(target_vx, 0, target_vz + 22),
        VoxelType::Bedrock,
        "Crater rim at R=11.0m must remain intact"
    );

    // 2. Check Depth down into subterranean basalt:
    // Center is (target_vx, target_vz). Depth down to -16.0m = -32 voxels.
    for vy in (-32..=0).rev() {
        assert_eq!(
            grid.get(target_vx, vy, target_vz),
            VoxelType::Air,
            "Crater center at vy={} (y={:.1}m) must be excavated to Air",
            vy, (vy as f32) * VOXEL_SIZE
        );
    }

    // Below -16.0m crater bottom (e.g. at y = -18.0m / vy = -36), basalt remains intact
    assert_eq!(
        grid.get(target_vx, -36, target_vz),
        VoxelType::Basalt,
        "Basalt substrate below -16m crater floor must remain intact"
    );

    // 3. Rubble Heightmap Excavation Check:
    // Required: rubble heightmap is excavated to Y <= -12.0m.
    let floor_h = physics.rubble_height_at(target_pos.x, target_pos.z);
    assert!(
        floor_h <= -12.0,
        "Rubble heightmap at crater center must be <= -12.0m, got {:.2}m",
        floor_h
    );
    assert!(
        floor_h <= -15.0,
        "Rubble heightmap at crater center must be excavated to ~ -16.0m (actual: {:.2}m)",
        floor_h
    );

    // 4. Secondary Effects Check:
    // Sparks (ejecta shower >= 80)
    assert!(physics.sparks.len() >= 80, "Must spawn >= 80 incandescent ejecta sparks, got {}", physics.sparks.len());

    // Shockwaves (must include ground cratering shockwave)
    assert!(!physics.shockwaves.is_empty(), "Must produce ground cratering shockwave");
    let max_shockwave = physics.shockwaves.iter().map(|s| s.max_radius).fold(0.0f32, f32::max);
    assert!(max_shockwave >= 28.0, "Ground shockwave must reach at least 28m radius, got {:.1}m", max_shockwave);

    // Subterranean point light illumination
    let mut lights: Vec<DynamicPointLight> = Vec::new();
    siege.collect_dynamic_lights(&mut lights);
    let subterranean_light = lights.iter().find(|l| l.position.y <= 0.0 && l.radius >= 30.0);
    assert!(subterranean_light.is_some(), "Dynamic point light must illuminate subterranean crater bowl");
}

#[test]
fn test_challenger_orbital_relay_continuous_bombardment_multi_strike() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Fire 3 consecutive orbital strikes at adjacent locations (spacing = 16m)
    let strike_locs = [
        grid.voxel_to_world(IVec3::new(40, 0, 40)),
        grid.voxel_to_world(IVec3::new(56, 0, 40)),
        grid.voxel_to_world(IVec3::new(40, 0, 56)),
    ];

    for (i, &loc) in strike_locs.iter().enumerate() {
        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(10.0 + (i as f32) * 5.0, 0.5, 10.0), Vec3::X, &audio);
        siege.fire_active_engine(Some(loc), &mut grid, &mut physics, &audio, Vec3::ZERO);
        siege.orbital_strikes.last_mut().unwrap().warmup_timer = 0.0;
        siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

        let floor = physics.rubble_height_at(loc.x, loc.z);
        assert!(floor <= -12.0, "Strike {} floor must be <= -12.0m, got {:.2}m", i, floor);
    }

    // Verify all 3 crater centers are excavated down into subterranean depths
    assert_eq!(grid.get(40, -20, 40), VoxelType::Air);
    assert_eq!(grid.get(56, -20, 40), VoxelType::Air);
    assert_eq!(grid.get(40, -20, 56), VoxelType::Air);
}
