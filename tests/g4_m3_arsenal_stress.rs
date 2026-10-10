#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DebrisParticle, DynamicChunk, DynamicPointLight, PhysicsSystem, SmokeParticle, SparkParticle};
use siege::{SiegeEngineKind, SiegeSystem};
use voxel::{VoxelGrid, VoxelType};
use weapon::{ActiveSingularity, WeaponSystem, MAX_C4_CHARGES, MAX_DET_CORD_DIST};

// ============================================================================
// MILESTONE G4-M3 INTEGRATION TEST SUITE: ADVANCED DEMOLITION & HEAVY ARSENAL
// Features 10, 11, 12: 24-C4 Grid, Laser Det-Cords, Foundation Cutting Shock
// Feature 13: Event Horizon Singularity 35m Vortex, Particles, Collapse
// Feature 14: Orbital Kinetic Relay Multi-Floor Penetration, 16m Bedrock Crater
// ============================================================================

// ----------------------------------------------------------------------------
// 1. 24-Point C4 Demolition Grid Placement Saturation & Coordinated Detonation
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_c4_demolition_grid_24_point_saturation_and_rejection() {
    assert_eq!(MAX_C4_CHARGES, 24, "MAX_C4_CHARGES must be expanded to 24");

    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Build anchor voxels for 24 charges
    for i in 0..24i32 {
        let vx = 20 + (i % 6) * 4;
        let vz = 20 + (i / 6) * 4;
        grid.set(vx, 5, vz, VoxelType::Concrete);
    }

    // Place 24 charges successfully
    for i in 0..24i32 {
        let vx = 20 + (i % 6) * 4;
        let vz = 20 + (i / 6) * 4;
        let pos = grid.voxel_to_world(IVec3::new(vx, 5, vz));
        let placed = weapon.place_c4_charge(pos, Vec3::Y, Some(IVec3::new(vx, 5, vz)));
        assert!(placed, "Charge {} must place successfully", i + 1);
    }
    assert_eq!(weapon.c4_charges.len(), 24, "All 24 C4 charges must be active");

    // Attempt 25th charge: must be rejected
    grid.set(50, 5, 50, VoxelType::Concrete);
    let extra_pos = grid.voxel_to_world(IVec3::new(50, 5, 50));
    let extra_placed = weapon.place_c4_charge(extra_pos, Vec3::Y, Some(IVec3::new(50, 5, 50)));
    assert!(!extra_placed, "25th charge must be rejected by saturation cap");
    assert_eq!(weapon.c4_charges.len(), 24, "Grid must remain strictly at 24 charges");

    // Coordinated simultaneous detonation of all 24 charges
    let (feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 24, "All 24 charges must be detonated simultaneously");
    assert_eq!(weapon.c4_charges.len(), 0, "Charges must be cleared after detonation");
    assert!(feedback.camera_trauma >= 0.8, "24-C4 detonation must induce massive camera trauma");
    assert!(feedback.fov_kick >= 0.15, "Coordinated blast must produce severe FOV kick");
    assert!(!weapon.active_cutting_shocks.is_empty(), "Detonation must generate cutting shocks along links");
}

// ----------------------------------------------------------------------------
// 2. C4 Laser Det-Cord RNG Topology & Foundation Cutting Shock Slicing
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_c4_foundation_cutting_shock_continuous_slice() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a continuous reinforced concrete foundation wall from Z=20 to Z=36 at X=30
    for z in 20..=36 {
        for y in 2..=6 {
            grid.set(30, y, z, VoxelType::Concrete);
        }
    }

    let pos_a = grid.voxel_to_world(IVec3::new(30, 4, 20));
    let pos_b = grid.voxel_to_world(IVec3::new(30, 4, 36));
    let dist = (pos_a - pos_b).length();
    assert!(dist <= MAX_DET_CORD_DIST, "Distance ({:.1}m) must be within det-cord cutoff (25m)", dist);

    weapon.place_c4_charge(pos_a, Vec3::X, Some(IVec3::new(30, 4, 20)));
    weapon.place_c4_charge(pos_b, Vec3::X, Some(IVec3::new(30, 4, 36)));

    // Verify RNG topology creates det-cord link
    let links = weapon.compute_det_cord_links();
    assert_eq!(links.len(), 1, "RNG must establish exactly 1 laser det-cord link between the two charges");

    // Midpoint voxel before detonation is solid
    let mid_voxel = IVec3::new(30, 4, 28);
    assert_eq!(grid.get(mid_voxel.x, mid_voxel.y, mid_voxel.z), VoxelType::Concrete);

    // Coordinated detonation severing foundation
    let (_feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 2);

    // Midpoint voxel along det-cord link must be completely vaporized by cylinder cutting shock (radius 1.4m)
    assert_eq!(
        grid.get(mid_voxel.x, mid_voxel.y, mid_voxel.z),
        VoxelType::Air,
        "Midpoint of foundation between C4 charges must be carved out by cylinder shock"
    );

    // Active cutting shocks must be simulated
    assert_eq!(weapon.active_cutting_shocks.len(), 1, "One cutting shock must be simulated");
    assert!(weapon.active_cutting_shocks[0].duration > 0.0);
    weapon.update(0.15, &mut grid, &mut physics, &audio, Vec3::ZERO);
    weapon.update(0.20, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(weapon.active_cutting_shocks.is_empty(), "Cutting shock must expire after decay");
}

// ----------------------------------------------------------------------------
// 3. Event Horizon Singularity 35m Vortex Suction & Multi-Particle Ingestion
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_event_horizon_35m_suction_multi_particle_ingestion() {
    assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0, "Singularity pull radius must be 35m");
    assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0, "Singularity pull strength must be 72 m/s^2");
    assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MIN, 16.0);
    assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MAX, 22.0);

    let mut physics = PhysicsSystem::new();
    let vortex_pos = Vec3::new(100.0, 20.0, 100.0);

    // 1. Dynamic chunk inside 35m radius (at 25m distance)
    physics.chunks.push(DynamicChunk {
        position: vortex_pos + Vec3::new(25.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 50.0,
        life: 10.0,
        settled: true,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // 2. Debris particle inside 35m radius (at 20m distance)
    physics.debris.push(DebrisParticle {
        position: vortex_pos + Vec3::new(0.0, 20.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        size: 0.3,
        color: Color::new(0.8, 0.4, 0.1, 1.0),
        life: 5.0,
        max_life: 5.0,
        bounces: 0,
    });

    // 3. Smoke particle inside 35m radius (at 30m distance)
    physics.smoke.push(SmokeParticle {
        position: vortex_pos + Vec3::new(0.0, 0.0, 30.0),
        velocity: Vec3::ZERO,
        size: 1.0,
        start_size: 1.0,
        max_size: 4.0,
        color: Color::new(0.2, 0.2, 0.2, 0.8),
        life: 3.0,
        max_life: 3.0,
    });

    // 4. Spark particle inside 35m radius (at 15m distance)
    physics.sparks.push(SparkParticle {
        position: vortex_pos + Vec3::new(15.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        color: Color::new(1.0, 0.9, 0.3, 1.0),
        life: 2.0,
        max_life: 2.0,
    });

    // 5. Far chunk outside 35m radius (at 45m distance)
    physics.chunks.push(DynamicChunk {
        position: vortex_pos + Vec3::new(45.0, 0.0, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 50.0,
        life: 10.0,
        settled: true,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // Apply vortex suction
    physics.apply_vortex_suction(vortex_pos, ActiveSingularity::PULL_RADIUS, ActiveSingularity::PULL_STRENGTH, 0.1);

    // Chunks: Inside chunk must be unsettled, pulled inward, given lift and swirl
    let inside_chunk = &physics.chunks[0];
    assert!(!inside_chunk.settled, "Chunk within 35m must be forcefully unsettled");
    assert!(inside_chunk.velocity.x < -0.1, "Chunk must accelerate inward toward vortex");
    assert!(inside_chunk.velocity.y > 0.1, "Chunk must receive anti-gravity lift (+24 m/s^2)");
    assert!(inside_chunk.velocity.z.abs() > 0.01, "Chunk must receive orbital swirl velocity");

    // Outside chunk must remain settled and zero velocity
    let outside_chunk = &physics.chunks[1];
    assert!(outside_chunk.settled, "Chunk outside 35m must remain settled");
    assert_eq!(outside_chunk.velocity, Vec3::ZERO, "Outside chunk velocity must be zero");

    // Debris particle: must be pulled inward
    assert!(physics.debris[0].velocity.y < -0.1, "Debris particle must be pulled inward toward center");

    // Smoke particle: must be pulled inward and swirled
    assert!(physics.smoke[0].velocity.z < -0.1, "Smoke particle must be pulled inward toward center");

    // Spark particle: must be pulled inward
    assert!(physics.sparks[0].velocity.x < -0.1, "Spark particle must be accelerated inward toward singularity");
}

// ----------------------------------------------------------------------------
// 4. Singularity Continuous Core Erosion & Collapse Implosion Cratering
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_event_horizon_continuous_erosion_and_collapse_implosion_crater() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let center = grid.voxel_to_world(IVec3::new(40, 10, 40));

    // Fill region with concrete
    for x in 35..=45 {
        for y in 6..=14 {
            for z in 35..=45 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Spawn singularity manually with 2.35s elapsed (near 2.4s max_time collapse)
    weapon.singularities.push(ActiveSingularity {
        position: center,
        timer: 2.35,
        max_time: 2.4,
        pull_radius: ActiveSingularity::PULL_RADIUS,
        pull_strength: ActiveSingularity::PULL_STRENGTH,
        blast_radius: 18.0,
        vacuum_played: true,
        erosion_timer: 0.14,
    });

    // Step 0.02s: triggers continuous erosion carving
    weapon.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(weapon.singularities.len(), 1);

    // Step 0.10s: crosses 2.4s threshold, triggering catastrophic collapse implosion
    weapon.update(0.10, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(weapon.singularities.is_empty(), "Singularity must implode and remove itself");

    // Collapse implosion shockwave
    assert!(!physics.shockwaves.is_empty(), "Collapse must create high-energy chromatic shockwaves");
    assert!(physics.shockwaves.iter().any(|s| s.max_radius > 15.0), "Shockwave must expand to collapse radius");

    // Verify cavity created at singularity center
    let center_voxel = grid.world_to_voxel(center);
    assert_eq!(
        grid.get(center_voxel.x, center_voxel.y, center_voxel.z),
        VoxelType::Air,
        "Singularity center must be completely hollowed out"
    );
}

// ----------------------------------------------------------------------------
// 5. Orbital Kinetic Relay Multi-Floor Vertical Penetration
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_orbital_kinetic_relay_stratospheric_multi_floor_penetration() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a multi-story building at x=30, z=30:
    // Ground floor y=1..4, Floor 2 y=8..10, Floor 3 y=16..18, Roof y=24..26
    for y in 1..=26 {
        grid.set(30, y, 30, VoxelType::Concrete);
        grid.set(31, y, 30, VoxelType::SteelRebar);
        grid.set(30, y, 31, VoxelType::CompositeRebar);
    }

    let roof_target = grid.voxel_to_world(IVec3::new(30, 25, 30));

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(roof_target), &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert_eq!(siege.orbital_strikes.len(), 1);
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // Verify beam penetrated all floors down to foundation
    assert_eq!(grid.get(30, 25, 30), VoxelType::Air, "Roof must be penetrated");
    assert_eq!(grid.get(30, 17, 30), VoxelType::Air, "Mid-floor 2 must be penetrated");
    assert_eq!(grid.get(30, 9, 30), VoxelType::Air, "Mid-floor 1 must be penetrated");
    assert_eq!(grid.get(30, 2, 30), VoxelType::Air, "Ground story must be penetrated");

    // Rebar columns must be severed
    assert_eq!(grid.get(31, 17, 30), VoxelType::Air, "Steel rebar must be vaporized");
    assert_eq!(grid.get(30, 17, 31), VoxelType::Air, "Composite rebar must be vaporized");
}

// ----------------------------------------------------------------------------
// 6. Orbital Kinetic Relay 16m Bedrock Crater Geometry & Subterranean Point Light
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m3_orbital_kinetic_relay_16m_bedrock_crater_geometry_and_subterranean_floor() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let target_world = grid.voxel_to_world(IVec3::new(48, 0, 48));

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // 1. Verify 16-meter crater diameter at surface bedrock level (y = 0):
    // Radius R = 8.0m = 16 voxels. Voxels within 15 voxels must be Air.
    for dx in -14..=14 {
        for dz in -14..=14 {
            let dist_vox = ((dx * dx + dz * dz) as f32).sqrt();
            if dist_vox <= 15.0 {
                assert_eq!(
                    grid.get(48 + dx, 0, 48 + dz),
                    VoxelType::Air,
                    "Bedrock at dx={}, dz={} must be excavated inside 16m crater",
                    dx, dz
                );
            }
        }
    }

    // Voxels outside 20 voxels radius (10.0m) must remain unexcavated Bedrock
    assert_eq!(grid.get(48 + 20, 0, 48), VoxelType::Bedrock, "Outside crater rim must remain intact");

    // 2. Verify subterranean depth down to -16.0m:
    assert_eq!(grid.get(48, -10, 48), VoxelType::Air, "Basalt at -5.0m must be excavated");
    assert_eq!(grid.get(48, -24, 48), VoxelType::Air, "Basalt at -12.0m must be excavated");
    assert_eq!(grid.get(48, -30, 48), VoxelType::Air, "Basalt at -15.0m must be excavated");

    // 3. Verify rubble_height_grid excavation matches new subterranean depth:
    let crater_floor_h = physics.rubble_height_at(target_world.x, target_world.z);
    assert!(
        crater_floor_h < -12.0,
        "Crater center rubble height must be excavated below -12.0m, got {:.2}",
        crater_floor_h
    );

    // 4. Subterranean Dynamic Point Light
    let mut lights: Vec<DynamicPointLight> = Vec::new();
    siege.collect_dynamic_lights(&mut lights);
    let bedrock_light = lights.iter().find(|l| l.position.y <= 0.0 && l.radius >= 30.0);
    assert!(bedrock_light.is_some(), "Dynamic point light must illuminate subterranean crater bowl");

    // 5. Ejecta sparks erupted
    assert!(physics.sparks.len() >= 80, "Must spawn massive incandescent ejecta spark shower (>= 80)");
}
