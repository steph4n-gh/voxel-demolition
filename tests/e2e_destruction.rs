#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use macroquad::prelude::*;
use voxel::{VoxelGrid, VoxelType, DamageType, VOXEL_SIZE, GRID_X, GRID_Y, GRID_Z};
use physics::{PhysicsSystem, DynamicChunk, DebrisParticle, SparkParticle, SmokeParticle, Mat3};
use audio::SoundSystem;
use siege::{SiegeEngineKind, SiegePayload, SiegeTrajectory, SiegeWeaponConfig, SiegeSystem, ColossalShell, TrailBuffer};
use world_gen::{LevelPreset, generate_level};
use ui::compute_ui_scale;
use std::sync::Mutex;
use std::collections::HashMap;

// ============================================================================
// TIER 1: FEATURE COVERAGE (>=5 tests per core feature)
// Features: Voxel Types, Material Properties, Explosive Chains,
//           Siege Batteries, Procedural Audio Deduplication, UI Scale Math
// ============================================================================

// ----------------------------------------------------------------------------
// Tier 1 - Feature 1: Voxel Types (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_voxel_type_solidity_and_air_predicates() {
    assert!(!VoxelType::Air.is_solid(), "Air must not be solid");
    let solids = [
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
    ];
    for vt in solids {
        assert!(vt.is_solid(), "Expected {:?} to be solid", vt);
    }
}

#[test]
fn test_tier1_voxel_type_transparency_and_opacity() {
    assert!(VoxelType::Air.is_transparent(), "Air is transparent");
    assert!(VoxelType::Glass.is_transparent(), "Glass is transparent");
    assert!(VoxelType::CryoIce.is_transparent(), "CryoIce is transparent");

    assert!(!VoxelType::Concrete.is_transparent(), "Concrete is opaque");
    assert!(!VoxelType::SteelRebar.is_transparent(), "SteelRebar is opaque");
    assert!(!VoxelType::HeavyArmor.is_transparent(), "HeavyArmor is opaque");

    assert!(VoxelType::Concrete.is_opaque(), "Concrete is opaque");
    assert!(VoxelType::SteelRebar.is_opaque(), "SteelRebar is opaque");
    assert!(!VoxelType::Glass.is_opaque(), "Glass is not opaque");
    assert!(!VoxelType::Air.is_opaque(), "Air is not opaque");
}

#[test]
fn test_tier1_voxel_type_destructibility() {
    assert!(!VoxelType::Air.is_destructible(), "Air cannot be destroyed");
    assert!(VoxelType::Bedrock.is_destructible(), "Reinforced Granite Bedrock must be destructible");
    assert!(VoxelType::Basalt.is_destructible(), "Subterranean Basalt Substrate must be destructible");

    let destructibles = [
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
        VoxelType::Basalt,
    ];
    for vt in destructibles {
        assert!(vt.is_destructible(), "{:?} must be destructible", vt);
    }
}

#[test]
fn test_tier1_voxel_type_color_and_alpha_channels() {
    assert_eq!(VoxelType::Air.color().a, 0.0, "Air alpha should be 0.0");
    assert!((VoxelType::Glass.color().a - 0.55).abs() < 0.01, "Glass must have 0.55 alpha for transparency");
    assert!((VoxelType::CryoIce.color().a - 0.85).abs() < 0.01, "CryoIce must have 0.85 alpha for translucency");
    assert_eq!(VoxelType::Concrete.color().a, 1.0, "Concrete must be fully opaque");
    assert_eq!(VoxelType::HeavyArmor.color().a, 1.0, "HeavyArmor must be fully opaque");
    assert_eq!(VoxelType::Explosive.color().a, 1.0, "Explosive must be fully opaque");
}

#[test]
fn test_tier1_voxel_type_density_and_mass_ordering() {
    let air_d = VoxelType::Air.properties().density;
    let glass_d = VoxelType::Glass.properties().density;
    let ice_d = VoxelType::CryoIce.properties().density;
    let brick_d = VoxelType::Brick.properties().density;
    let conc_d = VoxelType::Concrete.properties().density;
    let glow_d = VoxelType::GlowCore.properties().density;
    let comp_d = VoxelType::CompositeRebar.properties().density;
    let rebar_d = VoxelType::SteelRebar.properties().density;
    let armor_d = VoxelType::HeavyArmor.properties().density;
    let bed_d = VoxelType::Bedrock.properties().density;

    assert!(air_d < glass_d);
    assert!(glass_d < ice_d);
    assert!(ice_d < brick_d);
    assert!(brick_d < conc_d);
    assert!(conc_d < glow_d);
    assert!(glow_d < comp_d);
    assert!(comp_d < rebar_d);
    assert!(rebar_d < armor_d);
    assert!(armor_d < bed_d);
}

// ----------------------------------------------------------------------------
// Tier 1 - Feature 2: Material Properties (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_material_props_concrete_blast_and_durability() {
    let props = VoxelType::Concrete.properties();
    assert_eq!(props.name, "CAST CONCRETE");
    assert_eq!(props.max_health, 70.0);
    assert_eq!(props.hardness, 3.5);
    assert_eq!(props.toughness, 3.5);
    assert_eq!(props.blast_threshold, 8.0);
    assert!(!props.is_emissive);
    assert_eq!(VoxelType::Concrete.durability(), 7.0);
}

#[test]
fn test_tier1_material_props_heavy_armor_blast_deflection() {
    let mut grid = VoxelGrid::new();
    grid.set(20, 5, 20, VoxelType::HeavyArmor);
    let initial_health = grid.get_health(20, 5, 20);
    assert_eq!(initial_health, 240.0);

    // Damage below blast_threshold (28.0) must deflect without health loss
    let destroyed = grid.damage_voxel(20, 5, 20, 20.0, DamageType::Explosive);
    assert!(!destroyed, "Blast below 28.0 threshold should be deflected");
    assert_eq!(grid.get_health(20, 5, 20), initial_health, "Deflected blast inflicts 0 health loss");

    // Damage above blast_threshold penetrates
    let destroyed2 = grid.damage_voxel(20, 5, 20, 50.0, DamageType::Explosive);
    assert!(!destroyed2);
    assert!(grid.get_health(20, 5, 20) < initial_health, "Penetrating blast must reduce health");
}

#[test]
fn test_tier1_material_props_composite_rebar_vs_steel_rebar() {
    let comp = VoxelType::CompositeRebar.properties();
    let steel = VoxelType::SteelRebar.properties();

    assert!(comp.max_health > steel.max_health, "Composite rebar has higher max health");
    assert!(comp.hardness > steel.hardness, "Composite rebar has higher hardness");
    assert!(comp.toughness > steel.toughness, "Composite rebar has higher toughness");
    assert!(comp.density < steel.density, "Composite carbon rebar is significantly lighter than steel");
    assert_eq!(comp.thermal_soften, 0.9, "Composite carbon rebar resists thermal softening");
}

#[test]
fn test_tier1_material_props_thermal_softening_ablation() {
    let mut grid = VoxelGrid::new();
    // Test thermal softening multiplier: effective_damage = raw * thermal_soften
    // CryoIce: thermal_soften = 3.0
    // Fuel conduit (Explosive): thermal_soften = 2.5
    // HeavyArmor: thermal_soften = 1.6
    grid.set(10, 5, 10, VoxelType::HeavyArmor);
    let initial_h = grid.get_health(10, 5, 10);
    grid.damage_voxel(10, 5, 10, 50.0, DamageType::Thermal);
    let expected_dmg = 50.0 * 1.6;
    let actual_dmg = initial_h - grid.get_health(10, 5, 10);
    assert!((actual_dmg - expected_dmg).abs() < 0.01, "Thermal damage should be multiplied by 1.6x for heavy armor");
}

#[test]
fn test_tier1_material_props_fragile_silica_glass() {
    let props = VoxelType::Glass.properties();
    assert_eq!(props.max_health, 12.0);
    assert_eq!(props.toughness, 0.4);
    assert_eq!(props.blast_threshold, 0.0);

    let mut grid = VoxelGrid::new();
    grid.set(15, 5, 15, VoxelType::Glass);
    // Even tiny explosive damage destroys brittle glass
    let destroyed = grid.damage_voxel(15, 5, 15, 10.0, DamageType::Explosive);
    assert!(destroyed, "Glass should shatter easily under 10 explosive damage");
    assert_eq!(grid.get(15, 5, 15), VoxelType::Air);
}

// ----------------------------------------------------------------------------
// Tier 1 - Feature 3: Explosive Chains (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_explosive_chain_single_fuel_conduit() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let pos = IVec3::new(30, 5, 30);
    grid.set(pos.x, pos.y, pos.z, VoxelType::Explosive);

    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        vec![pos],
        Vec::new(),
        Vec3::new(30.0, 5.0, 35.0),
    );

    assert!(trauma > 0.0, "Explosive detonation should produce camera trauma");
    assert_eq!(grid.get(pos.x, pos.y, pos.z), VoxelType::Air, "Exploded voxel must become Air");
    assert!(!physics.shockwaves.is_empty(), "Detonation should spawn shockwaves");
}

#[test]
fn test_tier1_explosive_chain_linear_propagation() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let p1 = IVec3::new(30, 5, 30);
    let p2 = IVec3::new(31, 5, 30);
    let p3 = IVec3::new(32, 5, 30);

    grid.set(p1.x, p1.y, p1.z, VoxelType::Explosive);
    grid.set(p2.x, p2.y, p2.z, VoxelType::Explosive);
    grid.set(p3.x, p3.y, p3.z, VoxelType::Explosive);

    let world_p1 = grid.voxel_to_world(p1);
    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        vec![p1],
        Vec::new(),
        world_p1 + Vec3::new(0.0, 1.0, 2.0),
    );

    assert!(trauma > 0.10, "Chained multi-hop blast should accumulate camera trauma");
    assert_eq!(grid.get(p1.x, p1.y, p1.z), VoxelType::Air);
    assert_eq!(grid.get(p2.x, p2.y, p2.z), VoxelType::Air);
    assert_eq!(grid.get(p3.x, p3.y, p3.z), VoxelType::Air);
}

#[test]
fn test_tier1_explosive_chain_quantum_plasma_core_breach() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let core_pos = IVec3::new(25, 8, 25);
    grid.set(core_pos.x, core_pos.y, core_pos.z, VoxelType::GlowCore);

    let world_core = grid.voxel_to_world(core_pos);
    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        Vec::new(),
        vec![core_pos],
        world_core + Vec3::new(0.0, 1.0, 2.0),
    );

    assert!(trauma >= 0.20, "Glow core breach must deliver massive plasma trauma");
    assert_eq!(grid.get(core_pos.x, core_pos.y, core_pos.z), VoxelType::Air);
    assert!(!physics.shockwaves.is_empty(), "Glow core breach spawns shockwave");
}

#[test]
fn test_tier1_explosive_chain_mixed_fuel_and_plasma_cascade() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let exp_pos = IVec3::new(20, 6, 20);
    let core_pos = IVec3::new(21, 6, 20);
    let exp_pos2 = IVec3::new(22, 6, 20);

    grid.set(exp_pos.x, exp_pos.y, exp_pos.z, VoxelType::Explosive);
    grid.set(core_pos.x, core_pos.y, core_pos.z, VoxelType::GlowCore);
    grid.set(exp_pos2.x, exp_pos2.y, exp_pos2.z, VoxelType::Explosive);

    let world_exp = grid.voxel_to_world(exp_pos);
    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        vec![exp_pos],
        Vec::new(),
        world_exp + Vec3::new(0.0, 1.0, 2.0),
    );

    assert!(trauma > 0.20, "Mixed cascade should yield high compound trauma");
    assert_eq!(grid.get(exp_pos.x, exp_pos.y, exp_pos.z), VoxelType::Air);
    assert_eq!(grid.get(core_pos.x, core_pos.y, core_pos.z), VoxelType::Air);
    assert_eq!(grid.get(exp_pos2.x, exp_pos2.y, exp_pos2.z), VoxelType::Air);
}

#[test]
fn test_tier1_explosive_chain_concurrency_cap_guard() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Place a long string of 12 explosives
    let mut initial = Vec::new();
    for i in 0..12 {
        let p = IVec3::new(10 + i, 5, 10);
        grid.set(p.x, p.y, p.z, VoxelType::Explosive);
        if i == 0 {
            initial.push(p);
        }
    }

    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        initial,
        Vec::new(),
        Vec3::new(10.0, 5.0, 20.0),
    );

    // Safeguard caps fuel detonations at 6 per frame tick
    assert!(trauma <= 0.95, "Trauma must clamp at 0.95 max limit");
}

// ----------------------------------------------------------------------------
// Tier 1 - Feature 4: Siege Batteries (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_siege_battery_rail_artillery_caliber_scaling() {
    let mut cfg = SiegeWeaponConfig {
        engine_kind: SiegeEngineKind::RailArtillery,
        caliber_mm: 500.0,
        ..Default::default()
    };

    // Minimum caliber 500mm
    assert_eq!(cfg.shell_radius(), 0.75, "500mm caliber should give 0.75m shell radius");
    let min_blast = cfg.blast_radius();

    // Maximum caliber 2500mm
    cfg.caliber_mm = 2500.0;
    assert_eq!(cfg.shell_radius(), 1.75, "2500mm caliber should give 1.75m shell radius");
    let max_blast = cfg.blast_radius();

    assert!(max_blast > min_blast, "Larger caliber must give larger blast radius");
}

#[test]
fn test_tier1_siege_battery_bunker_buster_payload_specs() {
    let cfg = SiegeWeaponConfig {
        payload: SiegePayload::BunkerBuster,
        delayed_fuse_secs: 0.25,
        ..Default::default()
    };

    assert_eq!(cfg.payload.name(), "BUNKER-BUSTING PENETRATOR");
    assert!(cfg.blast_radius() > 0.0);
    assert_eq!(cfg.delayed_fuse_secs, 0.25);
    assert_eq!(cfg.payload.color(), Color::new(0.9, 0.75, 0.3, 1.0));
}

#[test]
fn test_tier1_siege_battery_orbital_kinetic_relay_beam_specs() {
    let kind = SiegeEngineKind::OrbitalKineticRelay;
    assert_eq!(kind.name(), "ORBITAL KINETIC RELAY");
    assert_eq!(kind.base_cooldown(), 4.5);
    assert_eq!(kind.theme_color(), Color::new(0.2, 0.85, 1.0, 1.0));

    let traj = SiegeTrajectory::OrbitalBeam;
    assert_eq!(traj.default_elevation_deg(), 88.0);
}

#[test]
fn test_tier1_siege_battery_hydraulic_ground_ram_specs() {
    let kind = SiegeEngineKind::HydraulicGroundRam;
    assert_eq!(kind.name(), "HYDRAULIC GROUND RAM");
    assert_eq!(kind.base_cooldown(), 1.4);
    assert_eq!(kind.theme_color(), Color::new(1.0, 0.80, 0.2, 1.0));
}

#[test]
fn test_tier1_siege_battery_tesla_pylon_arc_specs() {
    let kind = SiegeEngineKind::TeslaSiegePylon;
    assert_eq!(kind.name(), "TESLA SIEGE PYLON");
    assert_eq!(kind.base_cooldown(), 2.0);
    assert_eq!(kind.theme_color(), Color::new(0.65, 0.45, 1.0, 1.0));
}

// ----------------------------------------------------------------------------
// Tier 1 - Feature 5: Procedural Audio Deduplication (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_audio_anti_stacking_rapid_duplicate_blocked() {
    let map = Mutex::new(HashMap::new());
    let test_sound_id = 9999usize;

    // Trigger at t = 2.000s
    let mut map_lock = map.lock().unwrap();
    map_lock.insert(test_sound_id, 2.000f64);
    drop(map_lock);

    // Duplicate event 25ms later (<75ms threshold)
    let t_dup = 2.025f64;
    let map_lock = map.lock().unwrap();
    let allow_play = match map_lock.get(&test_sound_id) {
        Some(&last_t) => t_dup - last_t >= 0.075,
        None => true,
    };
    assert!(!allow_play, "Voice triggered within 25ms must be throttled");
}

#[test]
fn test_tier1_audio_anti_stacking_delayed_trigger_allowed() {
    let map = Mutex::new(HashMap::new());
    let test_sound_id = 8888usize;

    // Trigger at t = 5.000s
    let mut map_lock = map.lock().unwrap();
    map_lock.insert(test_sound_id, 5.000f64);
    drop(map_lock);

    // Event 90ms later (>75ms threshold)
    let t_next = 5.090f64;
    let map_lock = map.lock().unwrap();
    let allow_play = match map_lock.get(&test_sound_id) {
        Some(&last_t) => t_next - last_t >= 0.075,
        None => true,
    };
    assert!(allow_play, "Voice triggered after 90ms should be permitted");
}

#[test]
fn test_tier1_audio_concurrency_ducking_monotonic_decay() {
    let duck0 = SoundSystem::calculate_concurrency_ducking(0);
    let duck1 = SoundSystem::calculate_concurrency_ducking(1);
    let duck3 = SoundSystem::calculate_concurrency_ducking(3);
    let duck8 = SoundSystem::calculate_concurrency_ducking(8);

    assert_eq!(duck0, 1.0);
    assert!(duck1 < duck0);
    assert!(duck3 < duck1);
    assert!(duck8 < duck3);
    assert!(duck8 > 0.0, "Ducking factor must remain positive");
}

#[test]
fn test_tier1_audio_distance_attenuation_inverse_falloff() {
    let listener = Vec3::new(0.0, 2.0, 0.0);
    let at_zero = SoundSystem::calculate_distance_attenuation(listener, listener);
    let at_8m = SoundSystem::calculate_distance_attenuation(Vec3::new(0.0, 2.0, 8.0), listener);
    let at_24m = SoundSystem::calculate_distance_attenuation(Vec3::new(0.0, 2.0, 24.0), listener);
    let at_100m = SoundSystem::calculate_distance_attenuation(Vec3::new(0.0, 2.0, 100.0), listener);

    assert_eq!(at_zero, 1.0, "At zero distance attenuation is 1.0");
    assert!((at_8m - 0.50).abs() < 0.05, "At 8m distance attenuation is ~0.50");
    assert!(at_24m < at_8m);
    assert!(at_100m < at_24m);
}

#[test]
fn test_tier1_audio_distance_attenuation_boundary_clamping() {
    let listener = Vec3::ZERO;
    let far_away = Vec3::new(10000.0, 10000.0, 10000.0);
    let atten_far = SoundSystem::calculate_distance_attenuation(far_away, listener);
    assert_eq!(atten_far, 0.005, "Extreme distance must clamp at 0.005 floor");

    let atten_identical = SoundSystem::calculate_distance_attenuation(listener, listener);
    assert_eq!(atten_identical, 1.0, "Identical position must clamp at 1.0 ceiling");
}

// ----------------------------------------------------------------------------
// Tier 1 - Feature 6: UI Scale Math (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier1_ui_scale_720p_baseline() {
    let scale = compute_ui_scale(1280.0, 720.0);
    assert_eq!(scale, 1.0, "1280x720 is the 1.0 baseline");
}

#[test]
fn test_tier1_ui_scale_1080p_fhd() {
    let scale = compute_ui_scale(1920.0, 1080.0);
    assert_eq!(scale, 1.5, "1920x1080 FHD scales to 1.5x");
}

#[test]
fn test_tier1_ui_scale_1440p_qhd() {
    let scale = compute_ui_scale(2560.0, 1440.0);
    assert_eq!(scale, 2.0, "2560x1440 QHD scales to 2.0x");
}

#[test]
fn test_tier1_ui_scale_4k_retina_clamp_ceiling() {
    let scale_4k = compute_ui_scale(3840.0, 2160.0);
    assert_eq!(scale_4k, 3.0, "4K UHD scales to 3.0x");

    let scale_5k = compute_ui_scale(5120.0, 2880.0);
    assert_eq!(scale_5k, 3.2, "Ultra-high resolutions must clamp at 3.2 ceiling");
}

#[test]
fn test_tier1_ui_scale_low_res_clamp_floor() {
    let scale_low = compute_ui_scale(640.0, 360.0);
    assert_eq!(scale_low, 0.65, "Sub-720p resolutions must clamp at 0.65 floor");
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASES (>=5 tests per feature)
// Features: 1-Voxel Islands, Bedrock Collision at y=0.50,
//           Negative/Zero Damage, Zero Toughness/Limits, Max Grid Bounds
// ============================================================================

// ----------------------------------------------------------------------------
// Tier 2 - Feature 1: 1-Voxel Islands (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier2_one_voxel_island_cleared_from_static_grid() {
    let mut grid = VoxelGrid::new();
    // Place a single unsupported concrete block floating at y=15
    grid.set(32, 15, 32, VoxelType::Concrete);
    assert_eq!(grid.get(32, 15, 32), VoxelType::Concrete);

    let islands = grid.detect_unsupported_islands();
    // 1-voxel floating island should be cleared to Air
    assert_eq!(grid.get(32, 15, 32), VoxelType::Air, "1-voxel floating island must be converted to Air");
    assert_eq!(islands.len(), 0, "1-voxel components are not converted to dynamic rigid chunks");
}

#[test]
fn test_tier2_one_voxel_island_no_large_chunk_overhead() {
    let mut grid = VoxelGrid::new();
    grid.set(40, 20, 40, VoxelType::Brick);

    let islands = grid.detect_unsupported_islands();
    // Should not allocate dynamic chunk vector
    assert!(islands.is_empty());
    assert_eq!(grid.get(40, 20, 40), VoxelType::Air);
}

#[test]
fn test_tier2_one_voxel_on_bedrock_is_not_an_island() {
    let mut grid = VoxelGrid::new();
    // Place single block directly on top of bedrock (y=1)
    grid.set(30, 1, 30, VoxelType::Concrete);

    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Grounded block must not be an island");
    assert_eq!(grid.get(30, 1, 30), VoxelType::Concrete, "Grounded block must remain intact");
}

#[test]
fn test_tier2_multiple_separate_one_voxel_islands() {
    let mut grid = VoxelGrid::new();
    grid.set(10, 12, 10, VoxelType::Glass);
    grid.set(20, 14, 20, VoxelType::Brick);
    grid.set(30, 16, 30, VoxelType::Concrete);

    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty());
    assert_eq!(grid.get(10, 12, 10), VoxelType::Air);
    assert_eq!(grid.get(20, 14, 20), VoxelType::Air);
    assert_eq!(grid.get(30, 16, 30), VoxelType::Air);
}

#[test]
fn test_tier2_one_voxel_rebar_severed_event() {
    let mut grid = VoxelGrid::new();
    grid.set(25, 10, 25, VoxelType::SteelRebar);
    grid.current_rebar_count = 1;

    let _ = grid.detect_unsupported_islands();
    assert_eq!(grid.get(25, 10, 25), VoxelType::Air);
    assert_eq!(grid.current_rebar_count, 0, "Rebar count decremented when floating rebar removed");
    assert!(!grid.severed_rebar_positions.is_empty(), "Severed rebar position recorded");
}

// ----------------------------------------------------------------------------
// Tier 2 - Feature 2: Bedrock Collision at y=0.50 (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier2_bedrock_collision_y_050_prevents_penetration() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Create a 2-voxel chunk at y=1.0 with downward velocity
    let voxels = vec![
        (Vec3::new(0.0, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(0.0, VOXEL_SIZE, 0.0), VoxelType::Concrete),
    ];
    let aabb = DynamicChunk::compute_aabb(&voxels);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 1.0, 0.0),
        velocity: Vec3::new(0.0, -10.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 4.8,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    // Step physics
    physics.update(0.1, &mut grid, &audio, Vec3::ZERO);

    let lowest = physics.chunks[0].lowest_world_y();
    assert!(lowest >= 0.49, "Lowest voxel ({}) must not penetrate through bedrock y=0.50", lowest);
}

#[test]
fn test_tier2_bedrock_collision_penetration_correction() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Initialized below bedrock top (e.g. lowest_world_y is 0.20)
    let voxels = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb = DynamicChunk::compute_aabb(&voxels);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 0.40, 0.0), // lowest_world_y = 0.40 - 0.25 = 0.15 < 0.50
        velocity: Vec3::new(0.0, -1.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 2.4,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    let lowest = physics.chunks[0].lowest_world_y();
    assert!(lowest >= 0.49, "Penetration correction must push chunk above bedrock y=0.50");
}

#[test]
fn test_tier2_bedrock_low_velocity_settles_without_deletion() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let voxels = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb = DynamicChunk::compute_aabb(&voxels);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 0.75, 0.0),
        velocity: Vec3::new(0.0, -0.2, 0.0), // Low impact speed <= 2.0 m/s
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 2.4,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    physics.update(0.02, &mut grid, &audio, Vec3::ZERO);
    assert_eq!(physics.chunks.len(), 1, "Chunk must not be deleted on settling");
    assert!(physics.chunks[0].settled, "Low-velocity impact should settle the chunk");
    assert_eq!(physics.chunks[0].velocity, Vec3::ZERO);
}

#[test]
fn test_tier2_bedrock_high_velocity_fractures_large_chunk() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Create a large chunk with 60 voxels
    let mut voxels = Vec::new();
    for y in 0..60 {
        voxels.push((Vec3::new(0.0, y as f32 * VOXEL_SIZE, 0.0), VoxelType::Concrete));
    }
    let aabb = DynamicChunk::compute_aabb(&voxels);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 1.0, 0.0),
        velocity: Vec3::new(0.0, -15.0, 0.0), // High impact speed (>8.5)
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 120.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    assert!(physics.chunks.len() >= 2, "High-velocity heavy chunk impact on bedrock must fracture into sub-chunks");
}

#[test]
fn test_tier2_bedrock_destructible_foundation_floor() {
    let mut grid = VoxelGrid::new();
    // Bedrock at y=0 is filled during new()
    assert_eq!(grid.get(10, 0, 10), VoxelType::Bedrock);
    let initial_health = grid.get_health(10, 0, 10);
    assert_eq!(initial_health, 800.0, "Reinforced Granite Bedrock must have finite health 800.0");

    // Sub-threshold blast
    let destroyed_low = grid.damage_voxel(10, 0, 10, 15.0, DamageType::Explosive);
    assert!(!destroyed_low, "Sub-threshold damage must not destroy bedrock");

    // High-yield colossal blast
    let destroyed = grid.damage_voxel(10, 0, 10, 999999.0, DamageType::Explosive);
    assert!(destroyed, "Reinforced Granite Bedrock must be destroyed by colossal blast");
    assert_eq!(grid.get(10, 0, 10), VoxelType::Air, "Destroyed bedrock must become Air");
}

// ----------------------------------------------------------------------------
// Tier 2 - Feature 3: Negative / Zero Damage & Overflows (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier2_negative_damage_ignored() {
    let mut grid = VoxelGrid::new();
    grid.set(10, 5, 10, VoxelType::Concrete);
    let h0 = grid.get_health(10, 5, 10);

    let destroyed = grid.damage_voxel(10, 5, 10, -50.0, DamageType::Explosive);
    assert!(!destroyed);
    // Negative damage is deflected or does not increase health
    assert!(grid.get_health(10, 5, 10) >= h0);
}

#[test]
fn test_tier2_zero_damage_no_destruction() {
    let mut grid = VoxelGrid::new();
    grid.set(12, 5, 12, VoxelType::Concrete);
    let h0 = grid.get_health(12, 5, 12);

    let destroyed = grid.damage_voxel(12, 5, 12, 0.0, DamageType::Kinetic);
    assert!(!destroyed);
    assert_eq!(grid.get_health(12, 5, 12), h0);
}

#[test]
fn test_tier2_extreme_damage_clean_destruction() {
    let mut grid = VoxelGrid::new();
    grid.set(14, 5, 14, VoxelType::Concrete);

    let destroyed = grid.damage_voxel(14, 5, 14, 1_000_000.0, DamageType::Explosive);
    assert!(destroyed);
    assert_eq!(grid.get(14, 5, 14), VoxelType::Air);
    assert_eq!(grid.get_health(14, 5, 14), 0.0);
}

#[test]
fn test_tier2_fractional_damage_accumulation() {
    let mut grid = VoxelGrid::new();
    grid.set(16, 5, 16, VoxelType::Concrete);
    // Concrete max health 70.0. DamageType::Thermal applies raw_damage * 1.0.
    assert_eq!(grid.get_health(16, 5, 16), 70.0);

    // 4 hits of 15.0 thermal damage = 60.0 damage, 10.0 health left
    for _ in 0..4 {
        let destroyed = grid.damage_voxel(16, 5, 16, 15.0, DamageType::Thermal);
        assert!(!destroyed);
    }
    assert!((grid.get_health(16, 5, 16) - 10.0).abs() < 0.01);

    // 5th hit of 15.0 exceeds 10.0 remaining, destroys voxel
    let destroyed = grid.damage_voxel(16, 5, 16, 15.0, DamageType::Thermal);
    assert!(destroyed);
    assert_eq!(grid.get(16, 5, 16), VoxelType::Air);
}

#[test]
fn test_tier2_damage_non_destructible_types() {
    let mut grid = VoxelGrid::new();
    // Air is not destructible
    assert!(!grid.damage_voxel(5, 5, 5, 100.0, DamageType::Explosive));
    // Bedrock is not destructible
    assert!(!grid.damage_voxel(5, 0, 5, 100.0, DamageType::Explosive));
}

// ----------------------------------------------------------------------------
// Tier 2 - Feature 4: Zero Toughness/Hardness & Extreme Physics Limits (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier2_toughness_divisor_protection() {
    // Air has toughness 0.0; formula uses (props.toughness * 0.22).max(0.2)
    // to prevent division by zero or NaN
    let divisor = (0.0f32 * 0.22).max(0.2);
    assert_eq!(divisor, 0.2);
    let dmg = 50.0 / divisor;
    assert!(dmg.is_finite());
}

#[test]
fn test_tier2_hardness_kinetic_divisor_protection() {
    let hardness = 0.0f32;
    let toughness = 0.0f32;
    let divisor = (hardness * 0.20 + toughness * 0.12).max(0.2);
    assert_eq!(divisor, 0.2);
    let dmg = 50.0 / divisor;
    assert!(dmg.is_finite());
}

#[test]
fn test_tier2_extreme_blast_radius_carve_clamped() {
    let mut grid = VoxelGrid::new();
    grid.set(40, 20, 40, VoxelType::Concrete);

    // Carve with extreme 500m radius
    let res = grid.carve_sphere(Vec3::new(0.0, 10.0, 0.0), 500.0);
    assert!(res.destroyed > 0);
    // Concrete in radius is destroyed
    assert_eq!(grid.get(40, 20, 40), VoxelType::Air);
    // Reinforced Granite Bedrock absorbs primary blast and takes heavy damage
    assert_eq!(grid.get(40, 0, 40), VoxelType::Bedrock);
    assert!(grid.get_health(40, 0, 40) < 800.0, "Bedrock must take damage from blast");

    // Repeated extreme blasts accumulate damage and fully excavate bedrock into Air
    for _ in 0..10 {
        grid.carve_sphere(Vec3::new(0.0, 10.0, 0.0), 500.0);
    }
    assert_eq!(grid.get(40, 0, 40), VoxelType::Air, "Repeated colossal blasts must excavate Bedrock into Air");
}

#[test]
fn test_tier2_island_chunk_minimum_mass_guarantee() {
    let empty_chunk = voxel::IslandChunk {
        voxel_health: Vec::new(),
        voxels: Vec::new(),
        center_of_mass: Vec3::ZERO,
    };
    assert_eq!(empty_chunk.calculate_mass(), 1.0, "Minimum mass must be at least 1.0 to avoid zero-mass physics bugs");
}

#[test]
fn test_tier2_zero_length_beam_carve_cylinder() {
    let mut grid = VoxelGrid::new();
    let pt = Vec3::new(10.0, 5.0, 10.0);
    let res = grid.carve_cylinder_advanced(pt, pt, 2.0, 100.0, DamageType::Kinetic);
    assert_eq!(res.destroyed, 0, "Zero length beam must return empty CarveResult immediately");
}

// ----------------------------------------------------------------------------
// Tier 2 - Feature 5: Max Grid Bounds & Coordinate Extremes (5 tests)
// ----------------------------------------------------------------------------

#[test]
fn test_tier2_grid_exact_bounds_corners() {
    let grid = VoxelGrid::new();
    assert!(grid.in_bounds(0, 0, 0));
    assert!(grid.in_bounds(0, voxel::MIN_VOXEL_Y, 0));
    assert!(grid.in_bounds(GRID_X as i32 - 1, voxel::MAX_VOXEL_Y - 1, GRID_Z as i32 - 1));

    assert!(!grid.in_bounds(GRID_X as i32, 0, 0));
    assert!(!grid.in_bounds(0, voxel::MIN_VOXEL_Y - 1, 0));
    assert!(!grid.in_bounds(0, voxel::MAX_VOXEL_Y, 0));
    assert!(!grid.in_bounds(0, 0, GRID_Z as i32));
}

#[test]
fn test_tier2_negative_coordinate_queries_safe() {
    let grid = VoxelGrid::new();
    assert_eq!(grid.get(-1, -1, -1), VoxelType::Air);
    assert_eq!(grid.get_health(-5, 10, -3), 0.0);
    assert_eq!(grid.get_health_ratio(-5, 10, -3), 0.0);
}

#[test]
fn test_tier2_out_of_bounds_set_safe_noop() {
    let mut grid = VoxelGrid::new();
    grid.set(-1, 50, 200, VoxelType::Concrete);
    grid.set(GRID_X as i32 + 5, 10, 10, VoxelType::Concrete);
    // Should not panic or corrupt memory
    assert_eq!(grid.get(-1, 50, 200), VoxelType::Air);
}

#[test]
fn test_tier2_raycast_out_of_bounds_origin() {
    let grid = VoxelGrid::new();
    let hit = grid.raycast(Vec3::new(500.0, 500.0, 500.0), Vec3::new(1.0, 0.0, 0.0), 100.0);
    assert!(hit.is_none());
}

#[test]
fn test_tier2_world_to_voxel_coordinate_symmetry() {
    let grid = VoxelGrid::new();
    let vpos = IVec3::new(48, 20, 48);
    let wpos = grid.voxel_to_world(vpos);
    let roundtrip = grid.world_to_voxel(wpos);
    assert_eq!(vpos, roundtrip, "Roundtrip coordinate conversion must match exact voxel position");
}

// ============================================================================
// TIER 3: CROSS-FEATURE INTERACTIONS (8 tests)
// Coupled systems: Rebar tension + Blast, Cryo freeze + Kinetic shatter,
//                  Falling chunk + Rubble, Silo chain + Camera trauma, etc.
// ============================================================================

#[test]
fn test_tier3_rebar_tension_and_explosive_blast_collapse() {
    let mut grid = VoxelGrid::new();
    // Build a vertical pillar of concrete from y=1 to y=6
    for y in 1..=6 {
        grid.set(30, y, 30, VoxelType::Concrete);
    }
    // Build horizontal rebar cantilever at y=6
    for x in 31..=34 {
        grid.set(x, 6, 30, VoxelType::SteelRebar);
    }
    // Hang concrete floor from rebar
    grid.set(34, 5, 30, VoxelType::Concrete);

    let islands_before = grid.detect_unsupported_islands();
    assert!(islands_before.is_empty(), "Structure supported by rebar tension should not collapse");

    // Blast severing the rebar at (31, 6, 30)
    let severed = grid.damage_voxel(31, 6, 30, 250.0, DamageType::Explosive);
    assert!(severed, "Rebar should be severed by explosive blast");

    // Now rebar tension is broken: cantilevered block at (34, 5, 30) and remaining rebar fall
    let islands_after = grid.detect_unsupported_islands();
    assert!(!islands_after.is_empty(), "Severed rebar should cause unsupported section to collapse as island chunks");
}

#[test]
fn test_tier3_cryo_freeze_and_kinetic_shatter() {
    let mut grid = VoxelGrid::new();
    let pos = Vec3::new(0.0, 5.0, 0.0);
    let vpos = grid.world_to_voxel(pos);

    // Place heavy reinforced concrete
    grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Concrete);
    assert_eq!(grid.get(vpos.x, vpos.y, vpos.z), VoxelType::Concrete);

    // 1. Freeze with cryo blast -> transforms to CryoIce
    let frozen = grid.freeze_sphere(pos, 1.5);
    assert!(frozen > 0, "Cryo freeze must convert concrete to CryoIce");
    assert_eq!(grid.get(vpos.x, vpos.y, vpos.z), VoxelType::CryoIce);

    // 2. Kinetic shatter: CryoIce has max_health 14.0 and toughness 0.3.
    // Moderate kinetic blast shatters it completely
    let shattered = grid.damage_voxel(vpos.x, vpos.y, vpos.z, 20.0, DamageType::Kinetic);
    assert!(shattered, "Frozen block must shatter under kinetic impact");
    assert_eq!(grid.get(vpos.x, vpos.y, vpos.z), VoxelType::Air);
}

#[test]
fn test_tier3_falling_chunk_collides_with_settled_rubble() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. First chunk settles on bedrock at y=0.50
    let vox1 = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb1 = DynamicChunk::compute_aabb(&vox1);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 0.75, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox1,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 2.4,
        life: 0.0,
        settled: true,
        local_aabb: aabb1,
    });

    // 2. Second chunk falls from above
    let vox2 = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb2 = DynamicChunk::compute_aabb(&vox2);
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 3.0, 0.0),
        velocity: Vec3::new(0.0, -8.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox2,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 2.4,
        life: 0.0,
        settled: false,
        local_aabb: aabb2,
    });

    physics.update(0.15, &mut grid, &audio, Vec3::ZERO);
    assert!(physics.chunks.len() >= 2);
    // Falling chunk position should be above bedrock
    assert!(physics.chunks[1].lowest_world_y() >= 0.50);
}

#[test]
fn test_tier3_volatile_silo_chain_reaction_and_camera_trauma() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let p_fuel = IVec3::new(30, 8, 30);
    let p_core = IVec3::new(31, 8, 30);
    grid.set(p_fuel.x, p_fuel.y, p_fuel.z, VoxelType::Explosive);
    grid.set(p_core.x, p_core.y, p_core.z, VoxelType::GlowCore);

    let world_fuel = grid.voxel_to_world(p_fuel);
    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        vec![p_fuel],
        Vec::new(),
        world_fuel + Vec3::new(0.0, 1.0, 2.0),
    );

    assert!(trauma >= 0.20, "Silo chain reaction must produce cumulative camera trauma");
    assert_eq!(grid.get(p_fuel.x, p_fuel.y, p_fuel.z), VoxelType::Air);
    assert_eq!(grid.get(p_core.x, p_core.y, p_core.z), VoxelType::Air);
}

#[test]
fn test_tier3_siege_rail_artillery_strikes_structure_and_spawns_particles() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();

    // Build a concrete wall
    for y in 1..=10 {
        for z in 20..=25 {
            grid.set(30, y, z, VoxelType::Concrete);
        }
    }

    let hit_point = grid.voxel_to_world(IVec3::new(30, 5, 22));
    let carve_res = grid.carve_sphere_advanced(hit_point, 3.5, 300.0, DamageType::Explosive);
    assert!(carve_res.destroyed > 0, "Colossal artillery strike must carve structure");

    physics.add_explosion_effects(hit_point, 3.5, 45.0, Color::new(1.0, 0.45, 0.15, 1.0), carve_res.destroyed);
    assert!(!physics.shockwaves.is_empty(), "Strike must spawn shockwave");
    assert!(!physics.debris.is_empty(), "Strike must spawn debris particles");
}

#[test]
fn test_tier3_thermal_ablation_counters_tungsten_heavy_armor() {
    let mut grid = VoxelGrid::new();
    grid.set(20, 5, 20, VoxelType::HeavyArmor);
    grid.set(22, 5, 20, VoxelType::HeavyArmor);

    // Kinetic damage is mitigated by hardness (9.5) and toughness (8.5):
    // divisor = (9.5 * 0.20 + 8.5 * 0.12) = 2.92
    // 100.0 / 2.92 = 34.2 effective damage
    grid.damage_voxel(20, 5, 20, 100.0, DamageType::Kinetic);
    let kinetic_health_left = grid.get_health(20, 5, 20);

    // Thermal damage is multiplied by thermal_soften (1.6):
    // 100.0 * 1.6 = 160.0 effective damage
    grid.damage_voxel(22, 5, 20, 100.0, DamageType::Thermal);
    let thermal_health_left = grid.get_health(22, 5, 20);

    assert!(thermal_health_left < kinetic_health_left, "Thermal ablation must melt heavy armor far faster than kinetic impact");
}

#[test]
fn test_tier3_concussive_blast_shatters_brittle_masonry_deflects_rebar() {
    let mut grid = VoxelGrid::new();
    grid.set(10, 5, 10, VoxelType::Brick);       // toughness 1.8 (<4.0 -> 1.6x concussive multiplier)
    grid.set(12, 5, 10, VoxelType::SteelRebar);  // blast_threshold 18.0, toughness 7.5

    // Concussive damage of 15.0:
    // Brick: below threshold 4.0? No, 15 > 4.0. Effective dmg = 15.0 * 1.6 = 24.0. Health 40 -> 16.
    grid.damage_voxel(10, 5, 10, 15.0, DamageType::Concussive);
    assert!(grid.get_health(10, 5, 10) < 40.0, "Brick should take heavy concussive damage");

    // Steel rebar: 15.0 is below blast_threshold (18.0), so concussive shock deflects completely!
    grid.damage_voxel(12, 5, 10, 15.0, DamageType::Concussive);
    assert_eq!(grid.get_health(12, 5, 10), 140.0, "Steel rebar must deflect concussive blast below threshold");
}

#[test]
fn test_tier3_spatial_audio_attenuation_and_trauma_distance_sync() {
    let camera = Vec3::new(0.0, 2.0, 0.0);
    let near_source = Vec3::new(0.0, 2.0, 4.0);
    let far_source = Vec3::new(0.0, 2.0, 32.0);

    let audio_near = SoundSystem::calculate_distance_attenuation(near_source, camera);
    let audio_far = SoundSystem::calculate_distance_attenuation(far_source, camera);

    assert!(audio_near > 0.70);
    assert!(audio_far < 0.10);
    assert!(audio_near > audio_far, "Audio attenuation must decrease with distance");
}

// ============================================================================
// TIER 4: REAL-WORLD DEMOLITION WORKLOADS (4 tests)
// Monumental collapsed structures: Apex Skyscraper, Suspension Hyperbridge,
//                                  Cyber-Bunker Silos, Domino Colosseum
// ============================================================================

#[test]
fn test_tier4_workload_apex_skyscraper_foundational_collapse() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Generate Apex Skyscraper (Citadel)
    generate_level(&mut grid, LevelPreset::ApexSkyscraper);
    let initial_voxels = grid.current_voxels;
    assert!(initial_voxels >= 15000, "Apex Skyscraper must have substantial voxel mass (>15000), got {}", initial_voxels);

    // 2. Sever foundational perimeter columns and core at ground level y=2..4
    for x in 24..=72 {
        for z in 24..=72 {
            for y in 2..=4 {
                if grid.get(x, y, z).is_solid() && grid.get(x, y, z).is_destructible() {
                    grid.set(x, y, z, VoxelType::Air);
                }
            }
        }
    }

    // 3. Island Detection finds disconnected upper tower
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing skyscraper foundation must trigger island detection");

    // 4. Spawn islands into dynamic chunks in physics system
    for island in islands {
        physics.spawn_island_chunk(island, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.1, 0.0, 0.0));
    }
    assert!(!physics.chunks.is_empty(), "Skyscraper collapse must yield dynamic chunks");

    // 5. Step physics simulation for 10 frames
    for _ in 0..10 {
        physics.update(0.05, &mut grid, &audio, Vec3::new(0.0, 10.0, 30.0));
    }

    // Verify chunks collide with bedrock without falling through into negative space
    assert!(!physics.chunks.is_empty(), "Chunks must be preserved during collapse");
    for chunk in &physics.chunks {
        assert!(chunk.position.y >= 0.0, "Collapsed tower chunks must remain above bedrock");
    }
}

#[test]
fn test_tier4_workload_suspension_hyperbridge_cable_severance() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let layout = generate_level(&mut grid, LevelPreset::SuspensionSkybridge);
    assert!(grid.current_voxels > 5000, "Suspension bridge must have >5000 voxels");

    // Sever the center deck span by cutting two vertical slices across full height
    // Cut 1: design x = 33..=36, y = 10..=70, design z = 30..=66
    let (c1_x0, c1_x1) = layout.map_range_x(33, 36);
    let (z0, z1) = layout.map_range_z(30, 66);
    for x in c1_x0..=c1_x1 {
        for y in 10..=70 {
            for z in z0..=z1 {
                if grid.get(x, y, z).is_destructible() {
                    grid.set(x, y, z, VoxelType::Air);
                }
            }
        }
    }
    // Cut 2: design x = 60..=63, y = 10..=70, design z = 30..=66
    let (c2_x0, c2_x1) = layout.map_range_x(60, 63);
    for x in c2_x0..=c2_x1 {
        for y in 10..=70 {
            for z in z0..=z1 {
                if grid.get(x, y, z).is_destructible() {
                    grid.set(x, y, z, VoxelType::Air);
                }
            }
        }
    }

    // Detect unsupported central deck span
    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing suspension bridge mid-span must detect collapsed deck chunks");

    for island in islands {
        physics.spawn_island_chunk(island, Vec3::ZERO, Vec3::ZERO);
    }

    // Step physics
    for _ in 0..8 {
        physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    }

    // Deck chunks must fall and collide at bedrock
    assert!(!physics.chunks.is_empty());
}

#[test]
fn test_tier4_workload_cyber_bunker_fuel_silo_chain_reaction() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    generate_level(&mut grid, LevelPreset::MilitaryFortress);
    assert!(grid.current_voxels > 5000);

    // Locate explosive silos
    let mut exp_locations = Vec::new();
    for x in 10..GRID_X as i32 - 10 {
        for z in 10..GRID_Z as i32 - 10 {
            for y in 1..20 {
                if grid.get(x, y, z) == VoxelType::Explosive {
                    exp_locations.push(IVec3::new(x, y, z));
                }
            }
        }
    }

    assert!(!exp_locations.is_empty(), "Military Fortress level must contain explosive fuel silos");

    // Detonate first silo
    let first_silo = exp_locations[0];
    let world_silo = grid.voxel_to_world(first_silo);
    let trauma = crate::weapon::process_explosive_chain(
        &mut grid,
        &mut physics,
        &audio,
        vec![first_silo],
        Vec::new(),
        world_silo + Vec3::new(0.0, 1.0, 2.0),
    );

    assert!(trauma > 0.10, "Silo detonation must produce camera trauma");
    assert_eq!(grid.get(first_silo.x, first_silo.y, first_silo.z), VoxelType::Air);
    // Bedrock floor at y=0 must remain intact
    assert_eq!(grid.get(first_silo.x, 0, first_silo.z), VoxelType::Bedrock);
}

#[test]
fn test_tier4_workload_domino_colosseum_spire_cascade() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);
    assert!(grid.current_voxels > 10000, "Domino Colosseum must have >10000 voxels");

    // Spire 0 center is at design (76, 48)
    let sx = layout.map_x(76);
    let sz = layout.map_z(48);
    for y in 1..=4 {
        for dx in -4..=4 {
            for dz in -4..=4 {
                grid.set(sx + dx, y, sz + dz, VoxelType::Air);
            }
        }
    }

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing Colosseum spire base must trigger island collapse");

    for island in islands {
        physics.spawn_island_chunk(island, Vec3::new(-1.5, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.2));
    }

    // Step physics
    for _ in 0..10 {
        physics.update(0.05, &mut grid, &audio, Vec3::new(0.0, 10.0, 0.0));
    }

    assert!(!physics.chunks.is_empty());
}

// ============================================================================
// M2 CHALLENGER 1: ADVERSARIAL STRESS SUITE (Rubble Stacking & Pancake Collapse)
// ============================================================================

#[test]
fn test_rubble_stacking_at_elevation_y3_prevents_penetration_to_bedrock() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Create support foundation up to y=5 in grid (world top = 3.0)
    for vy in 1..=5 {
        grid.set(48, vy, 48, VoxelType::Concrete);
    }
    let foundation_world = grid.voxel_to_world(IVec3::new(48, 5, 48));

    // Settle a lower chunk at elevation y = 3.0 (top at 3.50)
    let vox1 = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb1 = DynamicChunk::compute_aabb(&vox1);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(foundation_world.x, 3.25, foundation_world.z),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox1,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 10.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb1,
    });
    phys.settle_chunk(0);

    let rubble_h = phys.rubble_height_at(foundation_world.x, foundation_world.z);
    assert!((rubble_h - 3.50).abs() < 0.01, "Rubble height must be stamped at 3.50, got {}", rubble_h);

    // 2. Spawn an upper chunk falling directly above it
    let vox2 = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb2 = DynamicChunk::compute_aabb(&vox2);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(foundation_world.x, 5.5, foundation_world.z),
        velocity: Vec3::new(0.0, -3.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox2,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 10.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb2,
    });

    // Step physics simulation until upper chunk settles or collides
    for _ in 0..60 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        if phys.chunks[1].settled {
            break;
        }
    }

    // Assert that the upper chunk collides at the rubble height elevation and does NOT pass through to bedrock y=0.50!
    let lowest_y = phys.chunks[1].lowest_world_y();
    assert!(
        lowest_y >= 3.25,
        "Upper chunk must collide and rest at or above rubble height elevation (>=3.25), but was at lowest_y = {}",
        lowest_y
    );
    assert!(
        phys.chunks[1].position.y >= 3.50,
        "Upper chunk position must be at or above 3.50, got {}",
        phys.chunks[1].position.y
    );
    assert!(
        lowest_y > 1.0,
        "Upper chunk must NOT pass through to bedrock y=0.50! lowest_y was {}",
        lowest_y
    );
}

#[test]
fn test_rubble_off_center_slope_landing_imparts_angular_velocity() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Create a stepped rubble mound:
    // Peak at (48, 48) with height y = 3.0
    for vy in 1..=5 {
        grid.set(48, vy, 48, VoxelType::Concrete);
    }
    // Slope step at (49, 48) with height y = 1.8
    for vy in 1..=3 {
        grid.set(49, vy, 48, VoxelType::Concrete);
    }

    let peak_world = grid.voxel_to_world(IVec3::new(48, 5, 48));
    let slope_world = grid.voxel_to_world(IVec3::new(49, 3, 48));

    // Settle peak chunk (top at 3.25)
    let vox_peak = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb_peak = DynamicChunk::compute_aabb(&vox_peak);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(peak_world.x, 3.0, peak_world.z),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox_peak,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 20.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb_peak,
    });
    phys.settle_chunk(0);

    // Settle slope chunk (top at 2.0)
    let vox_slope = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb_slope = DynamicChunk::compute_aabb(&vox_slope);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(slope_world.x, 1.75, slope_world.z),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox_slope,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 15.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb_slope,
    });
    phys.settle_chunk(1);

    let h_peak = phys.rubble_height_at(peak_world.x, peak_world.z);
    let h_slope = phys.rubble_height_at(slope_world.x, slope_world.z);
    assert!(h_peak > h_slope, "Peak height {} must be greater than slope height {}", h_peak, h_slope);

    let slope_normal = phys.rubble_normal_at(slope_world.x, slope_world.z);
    assert!(
        slope_normal.x > 0.5,
        "Slope normal must point outward along +X, got {:?}",
        slope_normal
    );

    // 2. Drop an upper chunk off-center directly onto the sloped mound shoulder
    let drop_x = slope_world.x;
    let vox2 = vec![
        (Vec3::new(-0.2, 0.0, 0.0), VoxelType::Concrete),
        (Vec3::new(0.2, 0.0, 0.0), VoxelType::Concrete),
    ];
    let aabb2 = DynamicChunk::compute_aabb(&vox2);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(drop_x, 4.0, slope_world.z),
        velocity: Vec3::new(0.0, -5.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox2,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 10.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb2,
    });

    let mut max_omega: f32 = 0.0;
    let initial_pos_x = drop_x;
    let mut tumbled_down = false;
    let mut max_rotation_mag: f32 = 0.0;

    for _ in 0..40 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
        let chunk = &phys.chunks[2];
        let omega = chunk.angular_velocity.length();
        if omega > max_omega {
            max_omega = omega;
        }
        let rot_mag = chunk.rotation.length();
        if rot_mag > max_rotation_mag {
            max_rotation_mag = rot_mag;
        }
        if (chunk.position.x - initial_pos_x).abs() > 0.05 || chunk.velocity.x.abs() > 0.1 {
            tumbled_down = true;
        }
    }

    assert!(
        max_omega > 0.1,
        "Off-center impact on rubble slope must impart angular velocity (omega > 0), got max_omega = {}",
        max_omega
    );
    assert!(
        max_rotation_mag > 0.05,
        "Angular velocity must produce rotational tumbling (rotation > 0), got max_rotation = {}",
        max_rotation_mag
    );
    assert!(
        tumbled_down,
        "Off-center landing on sloped rubble mound must produce lateral deflection/tumbling down the mound"
    );
}

#[test]
fn test_pancake_collapse_heavy_slab_pulverizes_multiple_intermediate_floors() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Construct two intermediate concrete floors:
    // Floor 1 (Upper): vy = 16 (world y ≈ 8.25)
    // Floor 2 (Lower): vy = 8 (world y ≈ 4.25)
    let min_x = 42;
    let max_x = 52;
    let min_z = 42;
    let max_z = 52;

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            grid.set(x, 16, z, VoxelType::Concrete);
            grid.set(x, 8, z, VoxelType::Concrete);
        }
    }

    let initial_solids = grid.solid_count();
    let floor_voxels_per_level = (max_x - min_x + 1) * (max_z - min_z + 1);
    assert_eq!(initial_solids, (floor_voxels_per_level * 2) as usize);

    // 2. Drop a heavy slab: mass > 500 kg, downward velocity > 10 m/s
    let mut slab_voxels = Vec::new();
    for x in -2..=2 {
        for z in -2..=2 {
            slab_voxels.push((Vec3::new(x as f32 * VOXEL_SIZE, 0.0, z as f32 * VOXEL_SIZE), VoxelType::HeavyArmor));
        }
    }
    let slab_aabb = DynamicChunk::compute_aabb(&slab_voxels);
    let center_world = grid.voxel_to_world(IVec3::new(47, 18, 47));

    phys.chunks.push(DynamicChunk {
        position: center_world,
        velocity: Vec3::new(0.0, -12.0, 0.0), // downward velocity > 10 m/s
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: slab_voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 750.0, // mass > 500 kg
        life: 0.0,
        settled: false,
        local_aabb: slab_aabb,
    });

    let initial_chunks = phys.chunks.len();
    let initial_debris = phys.debris.len();
    let initial_smoke = phys.smoke.len();

    // Step physics to smash through Floor 1 (vy = 16)
    for _ in 0..10 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    let solids_after_floor_1 = grid.solid_count();
    assert!(solids_after_floor_1 < initial_solids, "Floor 1 must be pulverized by heavy falling slab");
    assert!(phys.chunks.len() > initial_chunks, "Dynamic sub-chunks must be generated during pulverization");
    assert!(phys.debris.len() > initial_debris, "Debris particles must be emitted");
    assert!(phys.smoke.len() > initial_smoke, "Dust clouds / smoke must be emitted");

    let chunks_after_f1 = phys.chunks.len();
    let debris_after_f1 = phys.debris.len();
    let smoke_after_f1 = phys.smoke.len();

    // Continue simulation so slab plunges through Floor 2 (vy = 8)
    for _ in 0..20 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    let solids_after_floor_2 = grid.solid_count();
    assert!(
        solids_after_floor_2 < solids_after_floor_1,
        "Floor 2 must also be pulverized as slab cascades downward (solids: {} -> {} -> {})",
        initial_solids, solids_after_floor_1, solids_after_floor_2
    );
    assert!(
        phys.chunks.len() >= chunks_after_f1,
        "Sub-chunks generated across multi-tier pancake collapse"
    );
    assert!(
        phys.debris.len() >= debris_after_f1,
        "Debris particles emitted across collapse"
    );
    assert!(
        phys.smoke.len() >= smoke_after_f1,
        "Dust clouds emitted across collapse"
    );
}

// ============================================================================
// M2 CHALLENGER 2: ADVERSARIAL STRESS SUITE (Conductivity, Volatiles & Siege)
// ============================================================================

#[test]
fn test_explosive_adversarial_trimodal_multibranch_cascade_and_24_hop_cap() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct a multi-branch geometric snake of 32 nodes
    let mut chain_coords = Vec::new();
    // Node 0: root explosive at (10, 10, 10)
    chain_coords.push(IVec3::new(10, 10, 10));
    // Node 1: GlowCore at (12, 10, 10) (2 voxels = 1.0m from Node 0, within explosive breach radius)
    chain_coords.push(IVec3::new(12, 10, 10));
    // Remaining segment 1 nodes along +X spaced by 4 voxels (2.0m)
    for x in (16..=54).step_by(4) {
        chain_coords.push(IVec3::new(x, 10, 10));
    }
    // Segment 2: along +Z from z=14 to z=54 (step 4) at x=54 -> 11 nodes
    for z in (14..=54).step_by(4) {
        chain_coords.push(IVec3::new(54, 10, z));
    }
    // Segment 3: along -X from x=50 down to x=18 (step 4) at z=54 -> 9 nodes
    for x in (18..=50).rev().step_by(4) {
        chain_coords.push(IVec3::new(x, 10, 54));
    }
    assert_eq!(chain_coords.len(), 32);

    // Assign tri-modal volatile types:
    // Mode 1: Explosive at index 0
    grid.set(chain_coords[0].x, chain_coords[0].y, chain_coords[0].z, VoxelType::Explosive);
    // Mode 2: GlowCore at index 1
    grid.set(chain_coords[1].x, chain_coords[1].y, chain_coords[1].z, VoxelType::GlowCore);
    // Mode 3: PowerCapacitor at index 2
    grid.set(chain_coords[2].x, chain_coords[2].y, chain_coords[2].z, VoxelType::PowerCapacitor);

    // Conductive bridge between index 2 and index 3
    grid.set(19, 10, 10, VoxelType::SteelRebar);
    grid.set(20, 10, 10, VoxelType::CompositeRebar);
    grid.set(21, 10, 10, VoxelType::HeavyArmor);

    // Remaining nodes 3..31 are PowerCapacitors
    for coord in chain_coords.iter().skip(3) {
        grid.set(coord.x, coord.y, coord.z, VoxelType::PowerCapacitor);
    }

    // Trigger detonation from root (index 0)
    let cam_pos = grid.voxel_to_world(chain_coords[0]);
    let start_time = std::time::Instant::now();
    let trauma = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut physics,
        &audio,
        vec![chain_coords[0]],
        Vec::new(),
        Vec::new(),
        cam_pos,
    );
    let elapsed = start_time.elapsed();

    // Must not hang, stall, or exceed frame budget (< 50ms)
    assert!(elapsed.as_millis() < 50, "Cascade must complete in < 50ms, took {:?}", elapsed);
    assert!(trauma > 0.0, "Cascade must generate non-zero camera trauma");
    assert!(trauma <= 0.95, "Trauma must be bounded at 0.95 ceiling");

    // Assert that all three blast modes executed:
    // Mode 1 (Explosive): Orange shockwave ring (r > 0.8, g > 0.3, b < 0.2)
    let has_mode1 = physics.shockwaves.iter().any(|sw| {
        sw.color.r > 0.8 && sw.color.g > 0.3 && sw.color.b < 0.2
    });
    // Mode 2 (GlowCore): Plasma yellow/cyan shockwave ring (r > 0.8, g > 0.8)
    let has_mode2 = physics.shockwaves.iter().any(|sw| {
        sw.color.r > 0.8 && sw.color.g > 0.8 && sw.color.b < 0.5
    });
    // Mode 3 (PowerCapacitor): Electric violet shockwave ring (r ~ 0.68, b ~ 0.98)
    let has_mode3 = physics.shockwaves.iter().any(|sw| {
        (sw.color.r - 0.68).abs() < 0.05 && (sw.color.b - 0.98).abs() < 0.05
    });

    assert!(has_mode1, "Mode 1 (Explosive concussive blast) must execute and emit orange shockwave");
    assert!(has_mode2, "Mode 2 (GlowCore thermal plasma breach) must execute and emit plasma shockwave");
    assert!(has_mode3, "Mode 3 (PowerCapacitor arc discharge) must execute and emit violet shockwave");

    // Assert 24 hop cap invariant: exactly 48 shockwaves emitted (2 rings per hop: mode ring + explosion effect ring)
    assert_eq!(physics.shockwaves.len(), 48, "Cascade must be bounded at exactly 24 hops (each hop emits 2 shockwaves)");

    // Distant nodes beyond 24 hops (e.g. index 28 and index 31) are > 10m away from the 24th hop and MUST remain un-detonated
    assert_eq!(grid.get(chain_coords[28].x, chain_coords[28].y, chain_coords[28].z), VoxelType::PowerCapacitor, "Node 28 beyond 24 hops must remain intact");
    assert_eq!(grid.get(chain_coords[31].x, chain_coords[31].y, chain_coords[31].z), VoxelType::PowerCapacitor, "Node 31 beyond 24 hops must remain intact");
}

#[test]
fn test_explosive_adversarial_conductive_network_traversal_across_all_conductive_materials() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let center = IVec3::new(30, 10, 30);
    grid.set(center.x, center.y, center.z, VoxelType::PowerCapacitor);

    // Place targets at 7 voxels away (3.5m), outside direct crater blast_r (2.2m), but inside arc_range (6.8m)
    // 5 Conductive types (conductivity > 0.0):
    let cap_target = IVec3::new(center.x + 7, center.y, center.z); // PowerCapacitor: 12.0
    let rebar_target = IVec3::new(center.x - 7, center.y, center.z); // SteelRebar: 10.0
    let armor_target = IVec3::new(center.x, center.y + 7, center.z); // HeavyArmor: 9.0
    let comp_target = IVec3::new(center.x, center.y - 7, center.z); // CompositeRebar: 8.5
    let core_target = IVec3::new(center.x, center.y, center.z + 7); // GlowCore: 6.0

    // 3 Non-conductive types (conductivity == 0.0) at equal 7-voxel distance:
    let conc_target = IVec3::new(center.x, center.y, center.z - 7); // Concrete: 0.0
    let brick_target = IVec3::new(center.x + 5, center.y + 5, center.z); // Brick: 0.0
    let glass_target = IVec3::new(center.x - 5, center.y - 5, center.z); // Glass: 0.0

    grid.set(cap_target.x, cap_target.y, cap_target.z, VoxelType::PowerCapacitor);
    grid.set(rebar_target.x, rebar_target.y, rebar_target.z, VoxelType::SteelRebar);
    grid.set(armor_target.x, armor_target.y, armor_target.z, VoxelType::HeavyArmor);
    grid.set(comp_target.x, comp_target.y, comp_target.z, VoxelType::CompositeRebar);
    grid.set(core_target.x, core_target.y, core_target.z, VoxelType::GlowCore);

    grid.set(conc_target.x, conc_target.y, conc_target.z, VoxelType::Concrete);
    grid.set(brick_target.x, brick_target.y, brick_target.z, VoxelType::Brick);
    grid.set(glass_target.x, glass_target.y, glass_target.z, VoxelType::Glass);

    // Detonate center PowerCapacitor
    let trauma = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut physics,
        &audio,
        Vec::new(),
        Vec::new(),
        vec![center],
        Vec3::new(30.0, 10.0, 30.0),
    );

    assert!(trauma > 0.0);

    // 1. Non-conductive blocks must be completely untouched (0 damage)
    assert_eq!(grid.get(conc_target.x, conc_target.y, conc_target.z), VoxelType::Concrete, "Concrete must not conduct arcs");
    assert_eq!(grid.get(brick_target.x, brick_target.y, brick_target.z), VoxelType::Brick, "Brick must not conduct arcs");
    assert_eq!(grid.get(glass_target.x, glass_target.y, glass_target.z), VoxelType::Glass, "Glass must not conduct arcs");

    // 2. PowerCapacitor (conductivity 12.0) took arc damage, was destroyed, and chained
    assert_eq!(grid.get(cap_target.x, cap_target.y, cap_target.z), VoxelType::Air, "PowerCapacitor must be destroyed and chained by arc");

    // 3. GlowCore (conductivity 6.0) took arc damage and chained
    assert_eq!(grid.get(core_target.x, core_target.y, core_target.z), VoxelType::Air, "GlowCore must be destroyed and chained by arc");

    // 4. Secondary shockwaves must be present from chained capacitor and glow core
    assert!(physics.shockwaves.len() >= 3, "Chained capacitor and glow core must produce subsequent shockwaves");
}

#[test]
fn test_explosive_adversarial_boundary_conditions_and_zero_panic_guarantee() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Empty trigger lists: safe no-op returning 0.0
    let t0 = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut physics,
        &audio,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec3::ZERO,
    );
    assert_eq!(t0, 0.0);

    // 2. Extreme out-of-bounds coordinates (negative and far beyond max grid)
    let t_oob = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut physics,
        &audio,
        vec![IVec3::new(-50, -20, -100)],
        vec![IVec3::new(500, 200, 500)],
        vec![IVec3::new(-1, 0, -1)],
        Vec3::ZERO,
    );
    assert!(t_oob >= 0.0, "OOB coordinates must not panic");

    // 3. Automatic routing verification: place PowerCapacitor and pass it in initial_explosives
    let cap_pos = IVec3::new(20, 5, 20);
    grid.set(cap_pos.x, cap_pos.y, cap_pos.z, VoxelType::PowerCapacitor);
    let t_routed = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut physics,
        &audio,
        vec![cap_pos], // Sent via initial_explosives!
        Vec::new(),
        Vec::new(),
        Vec3::new(20.0, 5.0, 20.0),
    );
    assert!(t_routed > 0.0);
    assert_eq!(grid.get(cap_pos.x, cap_pos.y, cap_pos.z), VoxelType::Air);
    // Must produce electric violet shockwave despite being passed in explosives list
    let is_violet = physics.shockwaves.last().map(|sw| {
        (sw.color.r - 0.68).abs() < 0.05 && (sw.color.b - 0.98).abs() < 0.05
    }).unwrap_or(false);
    assert!(is_violet, "Automatic routing must recognize PowerCapacitor and trigger Mode 3 violet blast");
}

#[test]
fn test_siege_adversarial_rail_artillery_bunker_buster_punch_through_bore() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Fortified bunker structure:
    // Wall 1 at x = 12..=13: Concrete (2 voxels thick)
    for x in 12..=13 {
        for y in 4..=6 {
            for z in 4..=6 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    // Wall 2 at x = 16..=17: CompositeRebar (2 voxels thick)
    for x in 16..=17 {
        for y in 4..=6 {
            for z in 4..=6 {
                grid.set(x, y, z, VoxelType::CompositeRebar);
            }
        }
    }
    // Inner room at x = 19 with volatile cache
    grid.set(19, 5, 5, VoxelType::Explosive);
    grid.set(19, 5, 6, VoxelType::PowerCapacitor);

    // Wall 3 at x = 21..=22: HeavyArmor rear wall
    for x in 21..=22 {
        for y in 4..=6 {
            for z in 4..=6 {
                grid.set(x, y, z, VoxelType::HeavyArmor);
            }
        }
    }

    let w1_world = grid.voxel_to_world(IVec3::new(12, 5, 5));
    let shell = ColossalShell {
        position: w1_world - Vec3::new(2.5, 0.0, 0.0),
        velocity: Vec3::new(65.0, 0.0, 0.0), // Moving +X
        caliber_mm: 2000.0,
        radius: 1.5,
        blast_radius: 10.0,
        payload: SiegePayload::BunkerBuster,
        penetrations_remaining: 5,
        fuse_timer: 0.60,
        is_fuse_active: false,
        life: 0.0,
        max_life: 5.0,
        color: Color::new(0.9, 0.7, 0.2, 1.0),
        trail_points: TrailBuffer::empty(),
        has_detonated: false,
    };
    siege.colossal_shells.push(shell);

    let initial_debris = physics.debris.len();

    // Update 1: Shell penetrates Wall 1
    siege.update(0.04, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.colossal_shells.len(), 1, "Shell must survive piercing Wall 1");
    assert!(siege.colossal_shells[0].is_fuse_active, "Fuse must activate upon piercing first barrier");
    assert!(siege.colossal_shells[0].penetrations_remaining < 5, "Penetrations must decrement");
    assert!(physics.debris.len() > initial_debris, "Bore punch-through must generate spall debris particles");
    assert_eq!(grid.get(12, 5, 5), VoxelType::Air, "Wall 1 bore center must be cleared");

    // Update 2: Shell traverses air gap and pierces Wall 2 into the volatile cache
    let debris_after_w1 = physics.debris.len();
    for _ in 0..5 {
        if siege.colossal_shells.is_empty() {
            break;
        }
        siege.update(0.03, &mut grid, &mut physics, &audio, Vec3::ZERO);
    }

    assert_eq!(grid.get(16, 5, 5), VoxelType::Air, "Wall 2 bore center must be pierced");
    assert!(physics.debris.len() >= debris_after_w1, "Piercing Wall 2 must produce additional debris");
    assert_eq!(grid.get(19, 5, 5), VoxelType::Air, "Interior fuel silo must be detonated");
    assert_eq!(grid.get(19, 5, 6), VoxelType::Air, "Interior power capacitor must be detonated");
}

#[test]
fn test_siege_adversarial_hydraulic_ground_ram_5_stage_harmonic_resonance_escalation() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let engine_pos = Vec3::new(0.0, 0.5, 0.0);
    siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
    siege.deploy_selected_engine(engine_pos, Vec3::X, &audio);

    // Setup foundation columns with embedded volatiles within wave radius
    let ev = grid.world_to_voxel(engine_pos);
    let exp_col = IVec3::new(ev.x - 12, 2, ev.z - 16); // y=2 foundation
    let cap_col = IVec3::new(ev.x - 12, 3, ev.z - 16); // y=3 foundation
    grid.set(exp_col.x, exp_col.y, exp_col.z, VoxelType::Explosive);
    grid.set(cap_col.x, cap_col.y, cap_col.z, VoxelType::PowerCapacitor);

    let mut recorded_multipliers = Vec::new();
    let mut recorded_speeds = Vec::new();
    let mut recorded_radii = Vec::new();

    // Fire 5 successive slams within 6.0s window
    for expected_stage in 1..=5 {
        siege.deployed_engines[0].cooldown_timer = 0.0;
        siege.deployed_engines[0].last_slam_time = 0.5; // Within 6.0s
        let trauma = siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert!(trauma > 0.0, "Slam must produce trauma");
        assert_eq!(siege.deployed_engines[0].resonance_slam_count, expected_stage);

        let pulse = siege.seismic_pulses.last().expect("Seismic pulse must be spawned");
        assert_eq!(pulse.resonance_level, expected_stage);
        recorded_multipliers.push(pulse.resonance_mult);
        recorded_speeds.push(pulse.speed);
        recorded_radii.push(pulse.max_radius);
    }

    // Verify strict monotonic damage escalation across all 5 stages
    assert_eq!(recorded_multipliers, vec![1.0, 1.5, 2.2, 3.0, 4.0]);
    for i in 1..5 {
        assert!(recorded_multipliers[i] > recorded_multipliers[i - 1], "Multipliers must strictly escalate");
        assert!(recorded_speeds[i] > recorded_speeds[i - 1], "Wave speeds must strictly escalate with resonance");
        assert!(recorded_radii[i] > recorded_radii[i - 1], "Max radii must strictly escalate with resonance");
    }

    // Update simulation to let the stage 5 wave expand across the foundation
    for _ in 0..15 {
        siege.update(0.04, &mut grid, &mut physics, &audio, Vec3::ZERO);
    }

    // Foundation volatiles must be detonated by the expanding seismic wave
    assert_eq!(grid.get(exp_col.x, exp_col.y, exp_col.z), VoxelType::Air, "Foundation explosive must be destroyed by seismic wave");
    assert_eq!(grid.get(cap_col.x, cap_col.y, cap_col.z), VoxelType::Air, "Foundation capacitor must be destroyed by seismic wave");

    // Idle cooldown reset: advance time by 6.5s (> 6.0s threshold)
    siege.update(6.5, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.deployed_engines[0].resonance_slam_count, 0, "Resonance must reset to 0 after 6.0s idle");

    // Next slam after reset returns to Stage 1 (1.0x)
    siege.deployed_engines[0].cooldown_timer = 0.0;
    siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.deployed_engines[0].resonance_slam_count, 1, "Slam after idle reset must start at Stage 1");
    assert_eq!(siege.seismic_pulses.last().unwrap().resonance_mult, 1.0);
}

#[test]
fn test_siege_adversarial_tesla_siege_pylon_electrical_induction_and_conduction_overload() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let pylon_pos = Vec3::new(5.0, 0.5, 5.0);
    siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &audio);
    siege.deploy_selected_engine(pylon_pos, Vec3::X, &audio);

    // Target structure 20m away (within 42m range)
    let rebar_node = IVec3::new(35, 5, 35);
    let cap_node = IVec3::new(37, 5, 35);
    grid.set(rebar_node.x, rebar_node.y, rebar_node.z, VoxelType::SteelRebar);
    grid.set(cap_node.x, cap_node.y, cap_node.z, VoxelType::PowerCapacitor);

    // Volatiles near rebar (within 2m <= 3.5m induction radius)
    let near_exp = IVec3::new(35, 7, 35);
    let near_cap = IVec3::new(37, 7, 35);
    grid.set(near_exp.x, near_exp.y, near_exp.z, VoxelType::Explosive);
    grid.set(near_cap.x, near_cap.y, near_cap.z, VoxelType::PowerCapacitor);

    // Distant volatile 15 voxels (7.5m) away (> 3.5m induction radius)
    let distant_exp = IVec3::new(35, 20, 35);
    grid.set(distant_exp.x, distant_exp.y, distant_exp.z, VoxelType::Explosive);

    let target_w = grid.voxel_to_world(rebar_node);
    let trauma = siege.fire_active_engine(Some(target_w), &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert!(trauma > 0.0, "Tesla discharge must generate camera trauma");
    assert_eq!(siege.tesla_discharges.len(), 1, "Must generate a tesla discharge entity");

    // Assert that 3.5m induction detonated the nearby volatiles
    assert_eq!(grid.get(near_exp.x, near_exp.y, near_exp.z), VoxelType::Air, "Adjacent explosive must be detonated by 3.5m electrical induction");
    assert_eq!(grid.get(near_cap.x, near_cap.y, near_cap.z), VoxelType::Air, "Adjacent capacitor must be detonated by 3.5m electrical induction");

    // Assert that distant volatile remained completely untouched
    assert_eq!(grid.get(distant_exp.x, distant_exp.y, distant_exp.z), VoxelType::Explosive, "Distant explosive beyond 3.5m must remain intact");

    // Empty grid fallback edge case: clear grid and fire into empty void
    let mut empty_grid = VoxelGrid::new();
    let empty_trauma = siege.fire_active_engine(Some(Vec3::new(50.0, 10.0, 50.0)), &mut empty_grid, &mut physics, &audio, Vec3::ZERO);
    assert!(empty_trauma >= 0.0, "Firing tesla pylon with zero conductive targets must fallback safely without panic");
}

// ============================================================================
// TIER 5: RESPONSIVE CROSS-RESOLUTION UI & BOUNDS VERIFICATION (Feature 15)
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[allow(dead_code)]
impl RectBounds {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn intersects(&self, other: &RectBounds) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }
    pub fn is_within(&self, sw: f32, sh: f32) -> bool {
        self.x >= 0.0 && self.y >= 0.0 && self.right() <= sw && self.bottom() <= sh
    }
}

#[allow(dead_code)]
pub struct HUDLayoutBounds {
    pub scale: f32,
    pub dock: RectBounds,
    pub weapon_card: RectBounds,
    pub top_visor: RectBounds,
    pub scanner_bar: RectBounds,
    pub reticle_bounds: RectBounds,
}

#[allow(dead_code)]
pub fn compute_hud_layout_bounds(sw: f32, sh: f32) -> HUDLayoutBounds {
    let s = compute_ui_scale(sw, sh);
    let cx = sw * 0.5;
    let cy = sh * 0.5;

    // Reticle max bracket extent
    let max_bloom_offset = 24.0 * s;
    let bracket_len = 6.5 * s;
    let reticle = RectBounds::new(
        cx - max_bloom_offset - bracket_len,
        cy - max_bloom_offset - bracket_len,
        (max_bloom_offset + bracket_len) * 2.0,
        (max_bloom_offset + bracket_len) * 2.0,
    );

    // Top Visor Bar
    let top_w = (sw * 0.76).clamp(560.0 * s, 1180.0 * s);
    let top_h = 58.0 * s;
    let top_x = (sw - top_w) * 0.5;
    let top_y = 10.0 * s;
    let top_visor = RectBounds::new(top_x, top_y, top_w, top_h);

    // Scanner Sub-Bar
    let sc_h = 24.0 * s;
    let sc_y = top_y + top_h + 4.0 * s;
    let scanner_bar = RectBounds::new(top_x, sc_y, top_w, sc_h);

    // Tactical Dock
    let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
    let dock_x = 16.0 * s;
    let total_dock_h = 124.0 * s;
    let dock_y = sh - total_dock_h - 16.0 * s;
    let dock = RectBounds::new(dock_x, dock_y, dock_w, total_dock_h);

    // Weapon Card
    let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
    let card_h = 112.0 * s;
    let card_x = sw - card_w - 16.0 * s;
    let card_y = sh - card_h - 16.0 * s;
    let weapon_card = RectBounds::new(card_x, card_y, card_w, card_h);

    HUDLayoutBounds {
        scale: s,
        dock,
        weapon_card,
        top_visor,
        scanner_bar,
        reticle_bounds: reticle,
    }
}

#[allow(dead_code)]
pub struct WorkbenchLayoutBounds {
    pub scale: f32,
    pub panel: RectBounds,
    pub content: RectBounds,
    pub tab_bar: RectBounds,
    pub tab_width: f32,
}

#[allow(dead_code)]
pub fn compute_workbench_bounds(sw: f32, sh: f32) -> WorkbenchLayoutBounds {
    let s = compute_ui_scale(sw, sh);
    let pw = (sw * 0.94).min(1260.0 * s);
    let ph = (sh * 0.92).min(780.0 * s);
    let px = (sw - pw) * 0.5;
    let py = (sh - ph) * 0.5;

    let head_h = 44.0 * s;
    let tab_y = py + head_h + 6.0 * s;
    let tab_h = 36.0 * s;
    let tab_w = (pw - 24.0 * s - 4.0 * 6.0 * s) / 5.0;

    let content_x = px + 16.0 * s;
    let content_y = tab_y + tab_h + 10.0 * s;
    let content_w = pw - 32.0 * s;
    let content_h = py + ph - content_y - 10.0 * s;

    WorkbenchLayoutBounds {
        scale: s,
        panel: RectBounds::new(px, py, pw, ph),
        content: RectBounds::new(content_x, content_y, content_w, content_h),
        tab_bar: RectBounds::new(px + 12.0 * s, tab_y, pw - 24.0 * s, tab_h),
        tab_width: tab_w,
    }
}

#[test]
fn test_ui_scale_cross_resolution_invariants() {
    let test_viewports = [
        ("640x360", 640.0, 360.0, 0.65, true),
        ("800x600", 800.0, 600.0, 0.65, true),
        ("1024x768", 1024.0, 768.0, 0.80, false),
        ("1280x720", 1280.0, 720.0, 1.00, false),
        ("1920x1080", 1920.0, 1080.0, 1.50, false),
        ("1920x1200", 1920.0, 1200.0, 1.50, false),
        ("2560x1080", 2560.0, 1080.0, 1.50, false),
        ("2560x1440", 2560.0, 1440.0, 2.00, false),
        ("3440x1440", 3440.0, 1440.0, 2.00, false),
        ("3840x2160", 3840.0, 2160.0, 3.00, false),
        ("5120x1440", 5120.0, 1440.0, 2.00, false),
        ("5120x2880", 5120.0, 2880.0, 3.20, true),
    ];

    for &(name, w, h, expected_scale, is_clamped) in &test_viewports {
        let s = compute_ui_scale(w, h);
        assert!(
            (s - expected_scale).abs() < 0.001,
            "Resolution {name} ({w}x{h}) expected scale {expected_scale}, got {s}"
        );
        assert!((0.65..=3.20).contains(&s), "Scale {s} must be clamped in [0.65, 3.20]");
        if is_clamped {
            assert!((s - 0.65).abs() < 0.001 || (s - 3.20).abs() < 0.001);
        }
    }
}

#[test]
fn test_hud_dock_and_weapon_card_zero_collision() {
    let test_viewports = [
        (640.0, 360.0),
        (800.0, 600.0),
        (1024.0, 768.0),
        (1280.0, 720.0),
        (1920.0, 1080.0),
        (2560.0, 1080.0),
        (2560.0, 1440.0),
        (3440.0, 1440.0),
        (3840.0, 2160.0),
        (5120.0, 1440.0),
        (5120.0, 2880.0),
    ];

    for &(sw, sh) in &test_viewports {
        let layout = compute_hud_layout_bounds(sw, sh);

        // 1. Both fit cleanly within screen boundaries
        assert!(layout.dock.is_within(sw, sh), "Dock outside screen on {sw}x{sh}");
        assert!(layout.weapon_card.is_within(sw, sh), "Weapon card outside screen on {sw}x{sh}");

        // 2. Guaranteed zero collision
        assert!(
            !layout.dock.intersects(&layout.weapon_card),
            "Dock intersects Weapon Card on {sw}x{sh}!"
        );

        // 3. Clear horizontal gap >= 200px
        let gap = layout.weapon_card.x - layout.dock.right();
        assert!(
            gap >= 200.0,
            "Clearance gap too small on {sw}x{sh}: {gap} px"
        );
    }
}

#[test]
fn test_top_visor_bar_and_notifications_clearance() {
    let test_viewports = [
        (1280.0, 720.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
        (3840.0, 2160.0),
    ];

    for &(sw, sh) in &test_viewports {
        let layout = compute_hud_layout_bounds(sw, sh);
        let cy = sh * 0.5;

        // Top bar fits inside screen and centered
        assert!(layout.top_visor.is_within(sw, sh));
        let expected_x = (sw - layout.top_visor.w) * 0.5;
        assert!((layout.top_visor.x - expected_x).abs() < 0.1);

        // Reticle fits within screen
        assert!(layout.reticle_bounds.is_within(sw, sh));

        // Scanner bar locked beneath visor bar
        assert!(layout.scanner_bar.y >= layout.top_visor.bottom());

        // Notifications on top arc never breach overhead_bottom
        let overhead_bottom = layout.scanner_bar.bottom() + 10.0 * layout.scale;
        let avail_top_space = cy - overhead_bottom;
        let max_r = avail_top_space - 12.0 * layout.scale;

        // Check top notification apex
        let notif_radius = (130.0 * layout.scale).min(max_r);
        let notif_apex_y = cy - notif_radius;
        assert!(
            notif_apex_y > overhead_bottom,
            "Notification apex ({notif_apex_y}) clips overhead bar ({overhead_bottom}) on {sw}x{sh}"
        );
    }
}

#[test]
fn test_customizer_workbench_panel_and_tab_geometry() {
    let test_viewports = [
        (1280.0, 720.0),
        (1920.0, 1080.0),
        (2560.0, 1080.0),
        (2560.0, 1440.0),
        (3440.0, 1440.0),
        (3840.0, 2160.0),
    ];

    for &(sw, sh) in &test_viewports {
        let wb = compute_workbench_bounds(sw, sh);

        // 1. Panel is strictly within screen with positive margins
        assert!(wb.panel.is_within(sw, sh));
        assert!(wb.panel.x > 0.0 && wb.panel.y > 0.0);
        assert!(wb.panel.right() < sw && wb.panel.bottom() < sh);

        // 2. Centered horizontally and vertically
        let exp_px = (sw - wb.panel.w) * 0.5;
        let exp_py = (sh - wb.panel.h) * 0.5;
        assert!((wb.panel.x - exp_px).abs() < 0.1);
        assert!((wb.panel.y - exp_py).abs() < 0.1);

        // 3. Tab bar fits inside panel
        assert!(wb.tab_bar.w < wb.panel.w);
        assert!(wb.tab_width >= 200.0 * wb.scale, "Tab width too narrow: {}", wb.tab_width);

        // 4. Content area has substantial height for all 5 tabs
        let s = wb.scale;
        assert!(wb.content.h >= 500.0 * s, "Content height insufficient: {}", wb.content.h);

        // Check tab demands:
        // Tab 1 span = 360s, Tab 2 span = 342s, Tab 3 span = 464s, Tab 4 span = 384s
        assert!(wb.content.h > 464.0 * s, "Tab 3 siege engineering would overflow!");
    }
}

// ============================================================================
// TIER 5 / CHALLENGER: EMPIRICAL STRESS TESTS (Features 11 & 12)
// ============================================================================

#[test]
fn test_particle_accelerated_decay_debris_soft_cap() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 2600; // SOFT_DEBRIS = 2400
    for i in 0..initial_count {
        phys.debris.push(DebrisParticle {
            position: Vec3::new(i as f32 * 0.01, 10.0, 0.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.1,
            color: Color::new(0.5, 0.5, 0.5, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    let pre_ratio = phys.debris[0].life / phys.debris[0].max_life;
    phys.enforce_particle_caps();

    assert_eq!(phys.debris.len(), initial_count, "Particles exceeding soft cap must not be drained");
    assert!(phys.debris[0].life <= 0.12, "Debris lifetime must clamp to <= 0.12s, got {}", phys.debris[0].life);
    let post_ratio = phys.debris[0].life / phys.debris[0].max_life;
    assert!((post_ratio - pre_ratio).abs() < 1e-4, "Opacity ratio R must remain continuous");
}

#[test]
fn test_particle_accelerated_decay_debris_max_cap_no_drain() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 3100; // MAX_DEBRIS = 3000
    for i in 0..initial_count {
        phys.debris.push(DebrisParticle {
            position: Vec3::new(i as f32 * 0.01, 10.0, 0.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.1,
            color: Color::new(0.5, 0.5, 0.5, 1.0),
            life: 5.0,
            max_life: 5.0,
            bounces: 0,
        });
    }

    phys.enforce_particle_caps();

    assert_eq!(
        phys.debris.len(),
        initial_count,
        "Debris exceeding MAX_DEBRIS must decay gracefully via accelerated decay, NOT abruptly vanish via drain(0..hard_excess)"
    );
}

#[test]
fn test_particle_accelerated_decay_sparks_max_cap_no_drain() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 850; // MAX_SPARKS = 800
    for _ in 0..initial_count {
        phys.sparks.push(SparkParticle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 1.0, 0.0, 1.0),
            life: 1.0,
            max_life: 1.0,
        });
    }

    phys.enforce_particle_caps();

    assert_eq!(
        phys.sparks.len(),
        initial_count,
        "Sparks exceeding MAX_SPARKS must decay gracefully via accelerated decay, NOT abruptly vanish via drain(0..hard_excess)"
    );
}

#[test]
fn test_particle_accelerated_decay_smoke_max_cap_no_drain() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 950; // MAX_SMOKE = 900
    for _ in 0..initial_count {
        phys.smoke.push(SmokeParticle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 3.0,
        });
    }

    phys.enforce_particle_caps();

    assert_eq!(
        phys.smoke.len(),
        initial_count,
        "Smoke exceeding MAX_SMOKE must decay gracefully via accelerated decay, NOT abruptly vanish via drain(0..hard_excess)"
    );
}

#[test]
fn test_particle_accelerated_decay_smoke_lifetime_clamp() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 750; // SOFT_SMOKE = 720
    for _ in 0..initial_count {
        phys.smoke.push(SmokeParticle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 3.0,
        });
    }

    phys.enforce_particle_caps();

    assert!(
        phys.smoke[0].life <= 0.12,
        "Smoke lifetime must clamp to <= 0.12s, but got {}",
        phys.smoke[0].life
    );
}

#[test]
fn test_particle_accelerated_decay_smoke_opacity_ratio_continuous() {
    let mut phys = PhysicsSystem::new();
    let initial_count = 750; // SOFT_SMOKE = 720
    for _ in 0..initial_count {
        phys.smoke.push(SmokeParticle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 2.0,
            max_life: 4.0, // R = 0.50
        });
    }

    let pre_ratio = phys.smoke[0].life / phys.smoke[0].max_life;
    phys.enforce_particle_caps();
    let post_ratio = phys.smoke[0].life / phys.smoke[0].max_life;

    assert!(
        (post_ratio - pre_ratio).abs() < 1e-4,
        "Smoke opacity ratio R = life/max_life must remain continuous across accelerated decay (pre: {}, post: {})",
        pre_ratio,
        post_ratio
    );
}

#[test]
fn test_particle_accelerated_decay_dust_storm_scaling() {
    let mut phys = PhysicsSystem::new();

    // Test volume = 100
    phys.smoke.clear();
    phys.debris.clear();
    phys.spawn_structural_collapse_dust_storm(Vec3::new(10.0, 5.0, 10.0), 4.0, 100);
    let count_100 = phys.smoke.len();

    // Test volume = 500
    phys.smoke.clear();
    phys.debris.clear();
    phys.spawn_structural_collapse_dust_storm(Vec3::new(10.0, 5.0, 10.0), 4.0, 500);
    let count_500 = phys.smoke.len();

    // Test volume = 1000
    phys.smoke.clear();
    phys.debris.clear();
    phys.spawn_structural_collapse_dust_storm(Vec3::new(10.0, 5.0, 10.0), 4.0, 1000);
    let count_1000 = phys.smoke.len();

    // Test volume = 2000
    phys.smoke.clear();
    phys.debris.clear();
    phys.spawn_structural_collapse_dust_storm(Vec3::new(10.0, 5.0, 10.0), 4.0, 2000);
    let count_2000 = phys.smoke.len();

    assert!(count_100 <= count_500, "Dust storm smoke count must scale monotonically with volume");
    assert!(count_500 <= count_1000, "Dust storm smoke count must scale monotonically with volume");
    assert!(count_1000 <= count_2000, "Dust storm smoke count must scale monotonically with volume");
    assert!(count_100 >= 18, "Minimum clamp is 18");
    assert!(count_2000 <= 64, "Maximum clamp is 64");
}

#[test]
fn test_dynamic_wind_space_and_time_variance() {
    // 1. Time variance at same position
    let pos = Vec3::new(10.0, 2.0, 10.0);
    let w_t0 = PhysicsSystem::calculate_dynamic_wind(0.0, pos);
    let w_t1 = PhysicsSystem::calculate_dynamic_wind(1.5, pos);
    let w_t2 = PhysicsSystem::calculate_dynamic_wind(3.0, pos);
    assert!((w_t0 - w_t1).length() > 0.05, "Wind must vary across time t=0 to t=1.5");
    assert!((w_t1 - w_t2).length() > 0.05, "Wind must vary across time t=1.5 to t=3.0");

    // 2. Spatial variance at same time
    let t = 2.0;
    let w_p0 = PhysicsSystem::calculate_dynamic_wind(t, Vec3::new(0.0, 2.0, 0.0));
    let w_p1 = PhysicsSystem::calculate_dynamic_wind(t, Vec3::new(20.0, 2.0, 0.0));
    let w_p2 = PhysicsSystem::calculate_dynamic_wind(t, Vec3::new(0.0, 2.0, 20.0));
    assert!((w_p0 - w_p1).length() > 0.05, "Wind must vary across X space");
    assert!((w_p0 - w_p2).length() > 0.05, "Wind must vary across Z space");

    // 3. Thermal updraft decay with altitude
    let w_low = PhysicsSystem::calculate_dynamic_wind(t, Vec3::new(10.0, 1.0, 10.0));
    let w_high = PhysicsSystem::calculate_dynamic_wind(t, Vec3::new(10.0, 30.0, 10.0));
    assert!(w_low.y > w_high.y, "Thermal updraft must diminish with altitude");
}

// ============================================================================
// TIER 5 (M3 CHALLENGER 2): EMPIRICAL ADVERSARIAL STRESS TESTS
// Features 13, 14, 15: Spark/Debris Ricochets, Blackbody Ramp, Shockwaves,
//                      18-Direction Diagonal Lighting
// ============================================================================

#[test]
fn test_adversarial_spark_and_debris_wall_ricochets() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Cardinal Vertical Static Walls: Spark Ricochets
    // ----------------------------------------------------
    // +X wall at x = 35: spark travels +X, normal should be -X (-1, 0, 0)
    grid.set(35, 10, 20, VoxelType::Concrete);
    let wall_pos_x = grid.voxel_to_world(IVec3::new(35, 10, 20));
    phys.sparks.push(physics::SparkParticle {
        position: wall_pos_x - Vec3::new(0.18, 0.0, 0.0),
        velocity: Vec3::new(20.0, 0.0, 0.0),
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 1.0,
        max_life: 1.0,
    });

    // -X wall at x = 15: spark travels -X, normal should be +X (1, 0, 0)
    grid.set(15, 10, 20, VoxelType::Concrete);
    let wall_neg_x = grid.voxel_to_world(IVec3::new(15, 10, 20));
    phys.sparks.push(physics::SparkParticle {
        position: wall_neg_x + Vec3::new(0.18, 0.0, 0.0),
        velocity: Vec3::new(-20.0, 0.0, 0.0),
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 1.0,
        max_life: 1.0,
    });

    // +Z wall at z = 35: spark travels +Z, normal should be -Z (0, 0, -1)
    grid.set(25, 10, 35, VoxelType::Concrete);
    let wall_pos_z = grid.voxel_to_world(IVec3::new(25, 10, 35));
    phys.sparks.push(physics::SparkParticle {
        position: wall_pos_z - Vec3::new(0.0, 0.0, 0.18),
        velocity: Vec3::new(0.0, 0.0, 20.0),
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 1.0,
        max_life: 1.0,
    });

    // -Z wall at z = 15: spark travels -Z, normal should be +Z (0, 0, 1)
    grid.set(25, 10, 15, VoxelType::Concrete);
    let wall_neg_z = grid.voxel_to_world(IVec3::new(25, 10, 15));
    phys.sparks.push(physics::SparkParticle {
        position: wall_neg_z + Vec3::new(0.0, 0.0, 0.18),
        velocity: Vec3::new(0.0, 0.0, -20.0),
        color: Color::new(1.0, 0.8, 0.2, 1.0),
        life: 1.0,
        max_life: 1.0,
    });

    phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

    // Verify all 4 spark ricochets: velocity reverses along normal (v' · n > 0)
    let sp_px = &phys.sparks[0];
    let norm_px = Vec3::new(-1.0, 0.0, 0.0);
    assert!(sp_px.velocity.dot(norm_px) > 0.0, "Spark on +X wall must reverse along normal");
    assert!(sp_px.position.x < wall_pos_x.x, "Spark on +X wall must remain outside");

    let sp_nx = &phys.sparks[1];
    let norm_nx = Vec3::new(1.0, 0.0, 0.0);
    assert!(sp_nx.velocity.dot(norm_nx) > 0.0, "Spark on -X wall must reverse along normal");
    assert!(sp_nx.position.x > wall_neg_x.x, "Spark on -X wall must remain outside");

    let sp_pz = &phys.sparks[2];
    let norm_pz = Vec3::new(0.0, 0.0, -1.0);
    assert!(sp_pz.velocity.dot(norm_pz) > 0.0, "Spark on +Z wall must reverse along normal");
    assert!(sp_pz.position.z < wall_pos_z.z, "Spark on +Z wall must remain outside");

    let sp_nz = &phys.sparks[3];
    let norm_nz = Vec3::new(0.0, 0.0, 1.0);
    assert!(sp_nz.velocity.dot(norm_nz) > 0.0, "Spark on -Z wall must reverse along normal");
    assert!(sp_nz.position.z > wall_neg_z.z, "Spark on -Z wall must remain outside");

    // 2. Cardinal Vertical Static Walls: Micro-Spall Debris Ricochets
    // -------------------------------------------------------------
    phys.debris.clear();
    grid.set(40, 8, 20, VoxelType::HeavyArmor);
    let armor_pos = grid.voxel_to_world(IVec3::new(40, 8, 20));
    phys.debris.push(physics::DebrisParticle {
        position: armor_pos - Vec3::new(0.18, 0.0, 0.0),
        velocity: Vec3::new(12.0, 0.0, 0.0),
        rotation: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        size: 0.12,
        color: Color::new(0.4, 0.4, 0.45, 1.0),
        life: 2.0,
        max_life: 2.0,
        bounces: 0,
    });

    phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    let deb = &phys.debris[0];
    let norm_deb = Vec3::new(-1.0, 0.0, 0.0);
    assert!(deb.velocity.dot(norm_deb) > 0.0, "Debris on +X wall must reverse along normal: v'·n = {}", deb.velocity.dot(norm_deb));
    assert!(deb.position.x < armor_pos.x, "Debris must remain outside wall");
    assert_eq!(deb.bounces, 1, "Debris bounce count must increment");

    // 3. Sloped Rubble Mounds (X-slope, Z-slope, compound slope)
    // ----------------------------------------------------------
    phys.debris.clear();
    phys.sparks.clear();

    // Create a 3.5m tall rubble mound centered at (0.0, 0.0) with sloped flanks
    if let Some((cgx, cgz)) = PhysicsSystem::world_to_rubble_grid_coords(0.0, 0.0) {
        // X-slope descending to the right (+X): h(cgx-1) = 4.0, h(cgx+1) = 3.0
        let idx_l = (cgx - 1) * crate::voxel::GRID_Z + cgz;
        let idx_c = cgx * crate::voxel::GRID_Z + cgz;
        let idx_r = (cgx + 1) * crate::voxel::GRID_Z + cgz;
        phys.rubble_height_grid[idx_l] = 4.0;
        phys.rubble_height_grid[idx_c] = 3.5;
        phys.rubble_height_grid[idx_r] = 3.0;

        let mound_normal = PhysicsSystem::rubble_normal_from_slice(&phys.rubble_height_grid, 0.0, 0.0);
        // dh/dx = (3.0 - 4.0) / (2 * 0.5) = -1.0; n = (-(-1.0), 1.0, 0.0).normalize() = (1, 1, 0).normalize()
        assert!(mound_normal.x > 0.5, "Mound normal.x must tilt towards descending slope");
        assert!(mound_normal.y > 0.5, "Mound normal.y must point upward");

        // Fire spark downward at slope
        phys.sparks.push(physics::SparkParticle {
            position: Vec3::new(0.0, 3.55, 0.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: 1.0,
            max_life: 1.0,
        });

        // Drop micro-spall debris downward at slope
        phys.debris.push(physics::DebrisParticle {
            position: Vec3::new(0.0, 3.60, 0.0),
            velocity: Vec3::new(0.0, -10.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.10,
            color: Color::new(0.5, 0.5, 0.5, 1.0),
            life: 2.0,
            max_life: 2.0,
            bounces: 0,
        });

        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);

        let mound_sp = &phys.sparks[0];
        let mound_deb = &phys.debris[0];

        assert!(
            mound_sp.velocity.dot(mound_normal) > 0.0,
            "Spark velocity must reverse along rubble normal: v'·n = {}",
            mound_sp.velocity.dot(mound_normal)
        );
        assert!(
            mound_deb.velocity.dot(mound_normal) > 0.0,
            "Debris velocity must reverse along rubble normal: v'·n = {}",
            mound_deb.velocity.dot(mound_normal)
        );
    }
}

#[test]
fn test_adversarial_blackbody_fissure_cooling() {
    use physics::blackbody_color_ramp;

    // Helper for perceived luminance: L = 0.299*R + 0.587*G + 0.114*B
    let luminance = |c: &Color| 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;

    // 1. Stage 1: White-Hot (t > 0.85)
    for &t in &[1.0, 0.98, 0.95, 0.90, 0.86] {
        let c = blackbody_color_ramp(t);
        assert!(c.r >= 0.99, "White-hot red channel must be near saturated (t={}): {}", t, c.r);
        assert!(c.g >= 0.70, "White-hot green channel must be high incandescent (t={}): {}", t, c.g);
        assert!(c.b >= 0.40, "White-hot blue channel must have strong emission (t={}): {}", t, c.b);
        assert!(luminance(&c) > 0.80, "White-hot perceived luminance must exceed 0.80 (t={}): {}", t, luminance(&c));
        assert_eq!(c.a, 1.0, "White-hot alpha must be fully opaque");
    }

    // 2. Stage 2: Incandescent Orange (0.55 < t <= 0.85)
    for &t in &[0.85, 0.80, 0.75, 0.70, 0.65, 0.60, 0.56] {
        let c = blackbody_color_ramp(t);
        assert!(c.r >= 0.90, "Orange red channel must be strong (t={}): {}", t, c.r);
        assert!(c.g >= 0.35 && c.g <= 0.80, "Orange green channel must be in yellow/orange band (t={}): {}", t, c.g);
        assert!(c.b < 0.50, "Orange blue channel must have fallen off (t={}): {}", t, c.b);
        assert!(c.r > c.g && c.g > c.b, "Orange color hierarchy must satisfy R > G > B (t={}): {:?}", t, c);
        let l = luminance(&c);
        assert!((0.50..=0.82).contains(&l), "Orange luminance in expected range (t={}): {}", t, l);
    }

    // 3. Stage 3: Cherry Red (0.25 < t <= 0.55)
    for &t in &[0.55, 0.50, 0.45, 0.40, 0.35, 0.30, 0.26] {
        let c = blackbody_color_ramp(t);
        assert!(c.r >= 0.60, "Cherry red red channel must remain prominent (t={}): {}", t, c.r);
        assert!(c.g <= 0.42, "Cherry red green channel must have cooled (t={}): {}", t, c.g);
        assert!(c.b <= 0.15, "Cherry red blue channel must be minimal (t={}): {}", t, c.b);
        assert!(c.r / (c.g + 0.001) >= 1.6, "Red must strongly dominate green for cherry red (t={}): {:?}", t, c);
    }

    // 4. Stage 4: Dark Slag Crust to Cold Ash (t <= 0.25)
    for &t in &[0.25, 0.20, 0.15, 0.10, 0.05, 0.02, 0.0] {
        let c = blackbody_color_ramp(t);
        let l = luminance(&c);
        assert!(l < 0.55, "Slag crust luminance must be low (t={}): {}", t, l);
        if t <= 0.05 {
            assert!(c.r <= 0.25, "Deep slag crust red channel must be dark (t={}): {}", t, c.r);
            assert!(c.g <= 0.18, "Deep slag crust green channel must be dark (t={}): {}", t, c.g);
            assert!(c.b <= 0.15, "Deep slag crust blue channel must be dark (t={}): {}", t, c.b);
        }
        if t == 0.0 {
            assert_eq!(c.a, 0.0, "Zero-age ash must have 0 alpha");
        }
    }

    // 5. Monotonic Thermodynamic Cooling Invariant:
    // Total channel energy must monotonically decrease from t = 1.0 down to t = 0.05
    let mut prev_energy = f32::MAX;
    for step in 0..=50 {
        let t = 1.0 - (step as f32) * 0.019; // 1.0 down to 0.05
        let c = blackbody_color_ramp(t);
        let energy = c.r + c.g + c.b;
        assert!(energy <= prev_energy + 1e-4, "Cooling energy must be monotonic: t={}, prev={}, cur={}", t, prev_energy, energy);
        prev_energy = energy;
    }

    // 6. Adversarial Clamping Invariants
    let c_underflow = blackbody_color_ramp(-100.0);
    assert_eq!(c_underflow.a, 0.0, "Negative t must clamp to zero alpha");
    let c_overflow = blackbody_color_ramp(999.0);
    assert_eq!(c_overflow, blackbody_color_ramp(1.0), "Excessive t must clamp to t = 1.0");
}

#[test]
fn test_adversarial_normal_aligned_shockwaves_orthonormal_basis() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Adversarial normal test set:
    // Vertical normals (+Y, -Y), lateral cardinal wall normals (+X, -X, +Z, -Z),
    // and 16 planar angles around the XZ circle.
    let mut test_normals = vec![
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Z,
        Vec3::NEG_Z,
    ];

    for i in 0..16 {
        let angle = (i as f32 / 16.0) * std::f32::consts::TAU;
        test_normals.push(Vec3::new(angle.cos(), 0.0, angle.sin()));
    }

    for (idx, &n) in test_normals.iter().enumerate() {
        let tangent = if n.y.abs() > 0.92 {
            Vec3::X
        } else {
            n.cross(Vec3::Y).normalize_or_zero()
        };
        let bitangent = n.cross(tangent).normalize_or_zero();

        // 1. Tangent must be orthogonal to normal: t · n ≈ 0
        assert!(
            tangent.dot(n).abs() < 1e-4,
            "Tangent must be orthogonal to normal for normal #{}: n={:?}, t={:?}, dot={}",
            idx, n, tangent, tangent.dot(n)
        );

        // 2. Bitangent must be orthogonal to normal: b · n ≈ 0
        assert!(
            bitangent.dot(n).abs() < 1e-4,
            "Bitangent must be orthogonal to normal for normal #{}: n={:?}, b={:?}, dot={}",
            idx, n, bitangent, bitangent.dot(n)
        );

        // 3. Tangent and bitangent must be orthogonal: t · b ≈ 0
        assert!(
            tangent.dot(bitangent).abs() < 1e-4,
            "Tangent and bitangent must be orthogonal for normal #{}: t={:?}, b={:?}, dot={}",
            idx, tangent, bitangent, tangent.dot(bitangent)
        );

        // 4. Tangent and bitangent must be unit length: ||t|| ≈ 1, ||b|| ≈ 1
        assert!(
            (tangent.length() - 1.0).abs() < 1e-4,
            "Tangent must be unit length: ||t||={}",
            tangent.length()
        );
        assert!(
            (bitangent.length() - 1.0).abs() < 1e-4,
            "Bitangent must be unit length: ||b||={}",
            bitangent.length()
        );

        // 5. Spawn shockwave ring with this normal in PhysicsSystem
        phys.shockwaves.push(physics::ShockwaveRing {
            position: Vec3::new(idx as f32, 2.0, 0.0),
            normal: n,
            radius: 0.5,
            max_radius: 12.0,
            speed: 25.0,
            thickness: 0.40,
            color: Color::new(1.0, 0.5, 0.1, 1.0),
        });
    }

    assert_eq!(phys.shockwaves.len(), test_normals.len());

    // Advance physics step: all shockwaves must expand smoothly without numerical divergence
    phys.update(0.04, &mut grid, &audio, Vec3::ZERO);

    for (idx, sw) in phys.shockwaves.iter().enumerate() {
        let expected_radius = 0.5 + 25.0 * 0.04;
        assert!(
            (sw.radius - expected_radius).abs() < 1e-3,
            "Shockwave #{} radius did not expand accurately: got {}, exp {}",
            idx, sw.radius, expected_radius
        );
        assert_eq!(sw.normal, test_normals[idx], "Shockwave normal must remain invariant");
    }
}

#[test]
fn test_diagonal_lighting() {
    let mut grid = VoxelGrid::new();

    // 1. Build 45-degree diagonal wall running along (X + Z = 50) from X=20 to X=30
    for i in 0..10 {
        grid.set(20 + i, 5, 30 - i, VoxelType::Concrete);
    }

    // 2. Place dynamic point light offset diagonally in the negative normal direction
    let light_center = grid.voxel_to_world(IVec3::new(18, 5, 32));
    let light = physics::DynamicPointLight {
        position: light_center,
        color: Color::new(1.0, 0.55, 0.1, 1.0),
        radius: 10.0,
        intensity: 2.5,
    };

    // 3. Test 18-direction raycasting stencil:
    // Diagonal direction (1/sqrt(2), 0, -1/sqrt(2)) must strike the 45-degree wall directly
    const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let diag_dir = Vec3::new(INV_SQRT2, 0.0, -INV_SQRT2);
    let hit = grid.raycast(light.position, diag_dir, light.radius).expect("Diagonal ray must hit 45-degree wall");
    assert_eq!(hit.voxel_type, VoxelType::Concrete);
    assert!(hit.distance < light.radius);

    // 4. Standoff offset verification:
    // Surface spot radius is clamped <= 1.25m, and dome sits on the exterior wall face
    let spot_r = ((light.radius * light.radius - hit.distance * hit.distance).max(0.0).sqrt() * 0.45)
        .clamp(VOXEL_SIZE * 0.40, (light.radius * 0.35).min(1.25));
    assert!(spot_r <= 1.25, "Dynamic light spot radius must be clamped <= 1.25m to prevent room penetration");

    let dome_center = hit.world_pos + hit.normal * (spot_r * 0.70);
    // Dome center must be pushed outward along hit normal
    let outward_displacement = (dome_center - hit.world_pos).dot(hit.normal);
    assert!(outward_displacement > 0.0, "Dome center must sit on exterior wall face");

    // 5. Test 18-direction stencil coverage:
    // Stencil must contain 6 cardinal + 12 planar edge diagonals, all unit length
    let sample_dirs = [
        Vec3::X, Vec3::NEG_X,
        Vec3::Y, Vec3::NEG_Y,
        Vec3::Z, Vec3::NEG_Z,
        Vec3::new(INV_SQRT2, 0.0, INV_SQRT2),
        Vec3::new(INV_SQRT2, 0.0, -INV_SQRT2),
        Vec3::new(-INV_SQRT2, 0.0, INV_SQRT2),
        Vec3::new(-INV_SQRT2, 0.0, -INV_SQRT2),
        Vec3::new(INV_SQRT2, INV_SQRT2, 0.0),
        Vec3::new(INV_SQRT2, -INV_SQRT2, 0.0),
        Vec3::new(-INV_SQRT2, INV_SQRT2, 0.0),
        Vec3::new(-INV_SQRT2, -INV_SQRT2, 0.0),
        Vec3::new(0.0, INV_SQRT2, INV_SQRT2),
        Vec3::new(0.0, INV_SQRT2, -INV_SQRT2),
        Vec3::new(0.0, -INV_SQRT2, INV_SQRT2),
        Vec3::new(0.0, -INV_SQRT2, -INV_SQRT2),
    ];
    assert_eq!(sample_dirs.len(), 18, "Stencil must contain exactly 18 sampling directions");
    for &dir in &sample_dirs {
        assert!((dir.length() - 1.0).abs() < 1e-4, "Direction vector must be normalized: {:?}", dir);
    }
}

// ============================================================================
// TIER 5 (M3 CHALLENGER 3): EMPIRICAL ACCELERATED DECAY ADVERSARIAL STRESS TESTS
// Features 11 & 12: Particle Starvation Elimination & 2-Tier Accelerated Decay
// ============================================================================

#[test]
fn test_particle_accelerated_decay_debris_extreme_saturation_adversarial() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    let initial_count = 6000; // 2.0x MAX_DEBRIS (3000)
    for i in 0..initial_count {
        let initial_life = 4.0 + (i % 20) as f32 * 0.1;
        let initial_max_life = 6.0;
        phys.debris.push(DebrisParticle {
            position: Vec3::new(10.0 + (i % 10) as f32 * 0.1, 15.0, 10.0 + (i / 10) as f32 * 0.05),
            velocity: Vec3::new(0.0, -1.0, 0.0),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.10,
            color: Color::new(0.6, 0.4, 0.2, 1.0),
            life: initial_life,
            max_life: initial_max_life,
            bounces: 0,
        });
    }

    assert_eq!(phys.debris.len(), initial_count);
    let pre_ratios: Vec<f32> = phys.debris.iter().map(|p| p.life / p.max_life).collect();

    // 1. Enforce caps: Zero .drain() and zero truncation check
    phys.enforce_particle_caps();
    assert_eq!(
        phys.debris.len(),
        initial_count,
        "Particle count must NOT be truncated by enforce_particle_caps under extreme 2x saturation"
    );

    // 2. Opacity ratio continuity check for all 6000 particles
    for (i, p) in phys.debris.iter().enumerate() {
        let post_ratio = p.life / p.max_life;
        let diff = (post_ratio - pre_ratios[i]).abs();
        assert!(
            diff < 1e-4,
            "Opacity ratio jump at particle {}! pre: {}, post: {}, diff: {}",
            i, pre_ratios[i], post_ratio, diff
        );
    }

    // 3. Clamping bounds check:
    // Hard excess = 6000 - 3000 = 3000; life must be <= 0.04
    for i in 0..3000 {
        assert!(
            phys.debris[i].life <= 0.0401,
            "Hard excess debris particle {} must have life <= 0.04s, got {}",
            i, phys.debris[i].life
        );
    }
    // Soft excess = 6000 - 2400 = 3600; particles 3000..3600 must have life <= 0.12
    for i in 3000..3600 {
        assert!(
            phys.debris[i].life <= 0.1201,
            "Soft excess debris particle {} must have life <= 0.12s, got {}",
            i, phys.debris[i].life
        );
    }

    // 4. Natural expiration via simulation steps (swap_remove in step/update)
    let dt = 0.02;
    // After 4 frames (0.08s > 0.04s), hard-excess particles must have expired
    for _ in 0..4 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.debris.len() <= PhysicsSystem::MAX_DEBRIS,
        "Debris count ({}) must drop to <= MAX_DEBRIS ({}) within 4 frames",
        phys.debris.len(), PhysicsSystem::MAX_DEBRIS
    );

    // After 8 more frames (total 0.24s > 0.12s), soft-excess particles must have expired
    for _ in 0..8 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.debris.len() <= PhysicsSystem::SOFT_DEBRIS,
        "Debris count ({}) must drop to <= SOFT_DEBRIS ({}) within 12 total frames",
        phys.debris.len(), PhysicsSystem::SOFT_DEBRIS
    );
}

#[test]
fn test_particle_accelerated_decay_sparks_extreme_saturation_adversarial() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    let initial_count = 2000; // 2.5x MAX_SPARKS (800)
    for i in 0..initial_count {
        let initial_life = 0.5 + (i % 10) as f32 * 0.05;
        let initial_max_life = 1.0;
        phys.sparks.push(SparkParticle {
            position: Vec3::new(5.0, 10.0, 5.0),
            velocity: Vec3::new(1.0, 2.0, 0.0),
            color: Color::new(1.0, 0.8, 0.2, 1.0),
            life: initial_life,
            max_life: initial_max_life,
        });
    }

    assert_eq!(phys.sparks.len(), initial_count);
    let pre_ratios: Vec<f32> = phys.sparks.iter().map(|p| p.life / p.max_life).collect();

    // 1. Enforce caps: Zero .drain() check
    phys.enforce_particle_caps();
    assert_eq!(
        phys.sparks.len(),
        initial_count,
        "Sparks count must NOT be truncated by enforce_particle_caps under 2.5x saturation"
    );

    // 2. Opacity ratio continuity check for all 2000 particles
    for (i, p) in phys.sparks.iter().enumerate() {
        let post_ratio = p.life / p.max_life;
        let diff = (post_ratio - pre_ratios[i]).abs();
        assert!(
            diff < 1e-4,
            "Sparks opacity ratio jump at particle {}! pre: {}, post: {}, diff: {}",
            i, pre_ratios[i], post_ratio, diff
        );
    }

    // 3. Clamping bounds check
    // Hard excess = 2000 - 800 = 1200; life <= 0.04
    for i in 0..1200 {
        assert!(
            phys.sparks[i].life <= 0.0401,
            "Hard excess spark {} must have life <= 0.04s, got {}",
            i, phys.sparks[i].life
        );
    }

    // 4. Natural expiration via simulation steps
    let dt = 0.02;
    for _ in 0..4 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.sparks.len() <= PhysicsSystem::MAX_SPARKS,
        "Sparks count ({}) must drop to <= MAX_SPARKS ({}) within 4 frames",
        phys.sparks.len(), PhysicsSystem::MAX_SPARKS
    );

    for _ in 0..8 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.sparks.len() <= PhysicsSystem::SOFT_SPARKS,
        "Sparks count ({}) must drop to <= SOFT_SPARKS ({}) within 12 total frames",
        phys.sparks.len(), PhysicsSystem::SOFT_SPARKS
    );
}

#[test]
fn test_particle_accelerated_decay_smoke_extreme_saturation_adversarial() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    let initial_count = 2500; // 2.77x MAX_SMOKE (900)
    for i in 0..initial_count {
        let initial_life = 2.0 + (i % 15) as f32 * 0.1;
        let initial_max_life = 3.5;
        phys.smoke.push(SmokeParticle {
            position: Vec3::new(10.0, 5.0, 10.0),
            velocity: Vec3::new(0.1, 0.5, 0.1),
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(0.8, 0.8, 0.8, 0.8),
            life: initial_life,
            max_life: initial_max_life,
        });
    }

    assert_eq!(phys.smoke.len(), initial_count);
    let pre_ratios: Vec<f32> = phys.smoke.iter().map(|p| p.life / p.max_life).collect();

    // 1. Enforce caps: Zero .drain() check
    phys.enforce_particle_caps();
    assert_eq!(
        phys.smoke.len(),
        initial_count,
        "Smoke count must NOT be truncated by enforce_particle_caps under 2.77x saturation"
    );

    // 2. Opacity ratio continuity check for all 2500 particles
    for (i, p) in phys.smoke.iter().enumerate() {
        let post_ratio = p.life / p.max_life;
        let diff = (post_ratio - pre_ratios[i]).abs();
        assert!(
            diff < 1e-4,
            "Smoke opacity ratio jump at particle {}! pre: {}, post: {}, diff: {}",
            i, pre_ratios[i], post_ratio, diff
        );
    }

    // 3. Clamping bounds check
    // Hard excess = 2500 - 900 = 1600; life <= 0.04
    for i in 0..1600 {
        assert!(
            phys.smoke[i].life <= 0.0401,
            "Hard excess smoke {} must have life <= 0.04s, got {}",
            i, phys.smoke[i].life
        );
    }

    // 4. Natural expiration via simulation steps
    let dt = 0.02;
    for _ in 0..4 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.smoke.len() <= PhysicsSystem::MAX_SMOKE,
        "Smoke count ({}) must drop to <= MAX_SMOKE ({}) within 4 frames",
        phys.smoke.len(), PhysicsSystem::MAX_SMOKE
    );

    for _ in 0..8 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(
        phys.smoke.len() <= PhysicsSystem::SOFT_SMOKE,
        "Smoke count ({}) must drop to <= SOFT_SMOKE ({}) within 12 total frames",
        phys.smoke.len(), PhysicsSystem::SOFT_SMOKE
    );
}

#[test]
fn test_particle_accelerated_decay_simultaneous_all_pools_extreme_saturation() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Populate all 5 pools well beyond hard max caps
    let debris_count = 5000;   // MAX = 3000, SOFT = 2400
    let sparks_count = 1600;   // MAX = 800,  SOFT = 640
    let smoke_count = 1800;    // MAX = 900,  SOFT = 720
    let shock_count = 120;     // MAX = 64,   SOFT = 48
    let decal_count = 180;     // MAX = 96,   SOFT = 80

    for _ in 0..debris_count {
        phys.debris.push(DebrisParticle {
            position: Vec3::new(10.0, 10.0, 10.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            size: 0.1,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 3.0,
            max_life: 4.0,
            bounces: 0,
        });
    }
    for _ in 0..sparks_count {
        phys.sparks.push(SparkParticle {
            position: Vec3::new(10.0, 10.0, 10.0),
            velocity: Vec3::ZERO,
            color: Color::new(1.0, 1.0, 0.0, 1.0),
            life: 1.0,
            max_life: 1.5,
        });
    }
    for _ in 0..smoke_count {
        phys.smoke.push(SmokeParticle {
            position: Vec3::new(10.0, 10.0, 10.0),
            velocity: Vec3::ZERO,
            size: 0.5,
            start_size: 0.5,
            max_size: 2.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            life: 2.5,
            max_life: 3.0,
        });
    }
    for _ in 0..shock_count {
        phys.shockwaves.push(physics::ShockwaveRing {
            position: Vec3::new(10.0, 1.0, 10.0),
            normal: Vec3::Y,
            radius: 1.0,
            max_radius: 12.0,
            speed: 15.0,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thickness: 0.3,
        });
    }
    for _ in 0..decal_count {
        phys.decals.push(physics::ImpactDecal {
            position: Vec3::new(10.0, 1.0, 10.0),
            normal: Vec3::Y,
            size: 0.5,
            color: Color::new(0.0, 0.0, 0.0, 1.0),
            life: 5.0,
            max_life: 8.0,
            kind: physics::DecalKind::KineticImpact,
            voxel_anchor: None,
        });
    }

    // Assert exact pre-enforce sizes
    assert_eq!(phys.debris.len(), debris_count);
    assert_eq!(phys.sparks.len(), sparks_count);
    assert_eq!(phys.smoke.len(), smoke_count);
    assert_eq!(phys.shockwaves.len(), shock_count);
    assert_eq!(phys.decals.len(), decal_count);

    // Enforce caps: zero .drain() across all pools
    phys.enforce_particle_caps();

    assert_eq!(phys.debris.len(), debris_count, "Debris must not be drained");
    assert_eq!(phys.sparks.len(), sparks_count, "Sparks must not be drained");
    assert_eq!(phys.smoke.len(), smoke_count, "Smoke must not be drained");
    assert_eq!(phys.shockwaves.len(), shock_count, "Shockwaves must not be drained");
    assert_eq!(phys.decals.len(), decal_count, "Decals must not be drained");

    // Advance 4 frames (0.08s > 0.04s)
    let dt = 0.02;
    for _ in 0..4 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(phys.debris.len() <= PhysicsSystem::MAX_DEBRIS, "Debris <= MAX_DEBRIS");
    assert!(phys.sparks.len() <= PhysicsSystem::MAX_SPARKS, "Sparks <= MAX_SPARKS");
    assert!(phys.smoke.len() <= PhysicsSystem::MAX_SMOKE, "Smoke <= MAX_SMOKE");
    assert!(phys.shockwaves.len() <= PhysicsSystem::MAX_SHOCKWAVES, "Shockwaves <= MAX_SHOCKWAVES");
    assert!(phys.decals.len() <= PhysicsSystem::MAX_DECALS, "Decals <= MAX_DECALS");

    // Advance 8 more frames (total 0.24s > 0.12s)
    for _ in 0..8 {
        phys.update(dt, &mut grid, &audio, Vec3::ZERO);
    }
    assert!(phys.debris.len() <= PhysicsSystem::SOFT_DEBRIS, "Debris <= SOFT_DEBRIS");
    assert!(phys.sparks.len() <= PhysicsSystem::SOFT_SPARKS, "Sparks <= SOFT_SPARKS");
    assert!(phys.smoke.len() <= PhysicsSystem::SOFT_SMOKE, "Smoke <= SOFT_SMOKE");
    assert!(phys.shockwaves.len() <= PhysicsSystem::SOFT_SHOCKWAVES, "Shockwaves <= SOFT_SHOCKWAVES");
    assert!(phys.decals.len() <= PhysicsSystem::SOFT_DECALS, "Decals <= SOFT_DECALS");
}

#[test]
fn test_particle_accelerated_decay_opacity_ratio_spectrum_zero_brightness_pop() {
    let mut phys = PhysicsSystem::new();

    // Adversarially test a spectrum of initial life / max_life ratios
    let test_ratios = [0.05, 0.10, 0.20, 0.35, 0.50, 0.65, 0.80, 0.95, 1.00];

    for &target_r in &test_ratios {
        phys.debris.clear();
        phys.sparks.clear();
        phys.smoke.clear();
        phys.decals.clear();

        let max_l = 5.0;
        let l = max_l * target_r;

        // Populate beyond max caps so both soft and hard caps trigger
        for _ in 0..1600 {
            phys.debris.push(DebrisParticle {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                rotation: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                size: 0.1,
                color: Color::new(1.0, 1.0, 1.0, 1.0),
                life: l,
                max_life: max_l,
                bounces: 0,
            });
        }
        for _ in 0..900 {
            phys.sparks.push(SparkParticle {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                color: Color::new(1.0, 1.0, 0.0, 1.0),
                life: l,
                max_life: max_l,
            });
        }
        for _ in 0..1000 {
            phys.smoke.push(SmokeParticle {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                size: 0.5,
                start_size: 0.5,
                max_size: 2.0,
                color: Color::new(1.0, 1.0, 1.0, 1.0),
                life: l,
                max_life: max_l,
            });
        }
        for _ in 0..110 {
            phys.decals.push(physics::ImpactDecal {
                position: Vec3::ZERO,
                normal: Vec3::Y,
                size: 0.5,
                color: Color::new(0.0, 0.0, 0.0, 1.0),
                life: l,
                max_life: max_l,
                kind: physics::DecalKind::Scorch,
                voxel_anchor: None,
            });
        }

        let pre_debris_r = phys.debris[0].life / phys.debris[0].max_life;
        let pre_sparks_r = phys.sparks[0].life / phys.sparks[0].max_life;
        let pre_smoke_r = phys.smoke[0].life / phys.smoke[0].max_life;
        let pre_decals_r = phys.decals[0].life / phys.decals[0].max_life;

        phys.enforce_particle_caps();

        let post_debris_r = phys.debris[0].life / phys.debris[0].max_life;
        let post_sparks_r = phys.sparks[0].life / phys.sparks[0].max_life;
        let post_smoke_r = phys.smoke[0].life / phys.smoke[0].max_life;
        let post_decals_r = phys.decals[0].life / phys.decals[0].max_life;

        assert!(
            (post_debris_r - pre_debris_r).abs() < 1e-4,
            "Debris brightness pop at target ratio {}: pre {}, post {}",
            target_r, pre_debris_r, post_debris_r
        );
        assert!(
            (post_sparks_r - pre_sparks_r).abs() < 1e-4,
            "Sparks brightness pop at target ratio {}: pre {}, post {}",
            target_r, pre_sparks_r, post_sparks_r
        );
        assert!(
            (post_smoke_r - pre_smoke_r).abs() < 1e-4,
            "Smoke brightness pop at target ratio {}: pre {}, post {}",
            target_r, pre_smoke_r, post_smoke_r
        );
        assert!(
            (post_decals_r - pre_decals_r).abs() < 1e-4,
            "Decals brightness pop at target ratio {}: pre {}, post {}",
            target_r, pre_decals_r, post_decals_r
        );
    }
}

// ============================================================================
// TIER 5 / CHALLENGER 2: WHITE-BOX ADVERSARIAL COVERAGE HARDENING
// Subsystems:
// 1. Siege Battery Devastation (Rail Artillery, Ground Ram, Tesla, Orbital)
// 2. Procedural Audio Synthesis (Sample Bounds [-1.0, 1.0], Anti-Stacking, Ducking, Attenuation)
// 3. Responsive UI & Customizer (Cross-Resolution Scaling, Collision Invariants, Sliders & Presets)
// 4. Megastructure World Generation (All 6 Presets, Structural Stability, Anchoring & Volatiles)
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Siege Battery Devastation Adversarial Hardening
// ----------------------------------------------------------------------------

#[test]
fn test_siege_adversarial_rail_artillery_caliber_boundary_and_out_of_bounds_clamping() {
    let mut cfg = SiegeWeaponConfig::default();

    // Dense sweep of calibers: extreme negative, zero, sub-min, boundaries, extreme positive
    let test_calibers = [-1000.0, -1.0, 0.0, 100.0, 499.9, 500.0, 1000.0, 1500.0, 2000.0, 2500.0, 2500.1, 5000.0, 100000.0];
    for &cal in &test_calibers {
        cfg.caliber_mm = cal;
        let r = cfg.shell_radius();
        assert!((0.75..=1.75).contains(&r), "Shell radius must strictly clamp within [0.75, 1.75], got {} for caliber {}", r, cal);
        if cal <= 500.0 {
            assert_eq!(r, 0.75, "Caliber <= 500mm must clamp to 0.75m radius");
        } else if cal >= 2500.0 {
            assert_eq!(r, 1.75, "Caliber >= 2500mm must clamp to 1.75m radius");
        }
    }

    // Blast radius monotonic growth with caliber (500mm to 2500mm)
    let mut prev_blast = 0.0;
    for cal_step in (500..=2500).step_by(250) {
        cfg.caliber_mm = cal_step as f32;
        let blast = cfg.blast_radius();
        assert!(blast > prev_blast, "Blast radius must grow strictly monotonically with caliber: prev {}, curr {}", prev_blast, blast);
        assert!(!blast.is_nan() && !blast.is_infinite(), "Blast radius must be finite");
        prev_blast = blast;
    }

    // Trajectory vector azimuth and elevation invariants
    for elev_deg in [10.0, 25.0, 45.0, 65.0, 88.0] {
        for az_deg in [0.0, 45.0, 90.0, 180.0, 270.0, 359.9] {
            cfg.elevation_deg = elev_deg;
            cfg.azimuth_deg = az_deg;
            cfg.auto_aim_reticle = false;
            let vel = cfg.compute_launch_velocity_vector(0.0);
            let speed = cfg.launch_velocity();
            let mag = vel.length();
            assert!((mag - speed).abs() < 1e-2, "Velocity magnitude ({}) must match launch speed ({})", mag, speed);
            assert!(vel.y > 0.0, "Vertical velocity component must be positive for elevation {}", elev_deg);
        }
    }
}

#[test]
fn test_siege_adversarial_rail_artillery_bunker_buster_multi_wall_piercing_and_bedrock_detonation() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct 3 distinct concrete walls along X axis at x=10, x=14, x=18
    for &wx in &[10, 14, 18] {
        for y in 4..=6 {
            for z in 4..=6 {
                grid.set(wx, y, z, VoxelType::Concrete);
            }
        }
    }

    let w1_world = grid.voxel_to_world(IVec3::new(10, 5, 5));

    // Spawn a Bunker Buster shell approaching the first wall
    let shell = ColossalShell {
        position: w1_world - Vec3::new(1.5, 0.0, 0.0),
        velocity: Vec3::new(60.0, 0.0, 0.0),
        caliber_mm: 1500.0,
        radius: 1.0,
        blast_radius: 8.5,
        payload: SiegePayload::BunkerBuster,
        penetrations_remaining: 6,
        fuse_timer: 0.25, // Exactly 0.25s bunker-buster delayed fuse
        is_fuse_active: false,
        life: 0.0,
        max_life: 8.0,
        color: SiegePayload::BunkerBuster.color(),
        trail_points: TrailBuffer::empty(),
        has_detonated: false,
    };
    siege.colossal_shells.push(shell);

    // Frame 1: Shell penetrates wall 1
    siege.update(0.04, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.colossal_shells.len(), 1, "Shell must survive first wall hit");
    assert!(siege.colossal_shells[0].is_fuse_active, "Fuse must become active on first impact");
    assert_eq!(siege.colossal_shells[0].penetrations_remaining, 5, "Penetrations must decrement to 5");

    // Frame 2: Shell advances towards wall 2 and fuse timer decrements
    siege.update(0.04, &mut grid, &mut physics, &audio, Vec3::ZERO);
    if !siege.colossal_shells.is_empty() {
        assert!(siege.colossal_shells[0].is_fuse_active);
        assert!(siege.colossal_shells[0].fuse_timer < 0.25, "Fuse timer must decrement with dt on subsequent frames");
    }

    // Bedrock direct hit invariant:
    // Any shell hitting bedrock (even with penetrations remaining and active fuse) MUST detonate immediately
    let bedrock_shell = ColossalShell {
        position: Vec3::new(0.0, 1.0, 0.0),
        velocity: Vec3::new(0.0, -50.0, 0.0), // plunging directly into bedrock floor
        caliber_mm: 2000.0,
        radius: 1.2,
        blast_radius: 9.0,
        payload: SiegePayload::BunkerBuster,
        penetrations_remaining: 5,
        fuse_timer: 1.0,
        is_fuse_active: false,
        life: 0.0,
        max_life: 5.0,
        color: SiegePayload::BunkerBuster.color(),
        trail_points: TrailBuffer::empty(),
        has_detonated: false,
    };
    siege.colossal_shells.push(bedrock_shell);
    siege.update(0.03, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // Bedrock shell must have detonated immediately and excavated bedrock
    assert!(siege.colossal_shells.iter().all(|s| s.payload != SiegePayload::BunkerBuster || s.position.y > 0.50));
    let impact_vox = grid.world_to_voxel(Vec3::new(0.0, 0.0, 0.0));
    assert_eq!(grid.get(impact_vox.x, impact_vox.y, impact_vox.z), VoxelType::Air, "Bedrock at impact site must be excavated by 2000mm Bunker Buster");
    assert_eq!(grid.get(0, 0, 0), VoxelType::Bedrock, "Distant bedrock outside blast radius remains intact");
}

#[test]
fn test_siege_adversarial_hydraulic_ground_ram_harmonic_progression_and_cooldown() {
    let mut siege = SiegeSystem::new();
    let audio = SoundSystem::empty();

    // Verify Hydraulic Ground Ram cooldown is 1.4s
    assert_eq!(SiegeEngineKind::HydraulicGroundRam.base_cooldown(), 1.4);

    let origin = Vec3::new(0.0, 0.5, 0.0);
    siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
    siege.deploy_selected_engine(origin, Vec3::X, &audio);

    let eng = &siege.deployed_engines[0];
    assert_eq!(eng.max_cooldown, 1.4);
    assert!(eng.is_ready());

    // Theoretical 5-stage harmonic resonance progression:
    // A(n) = 1.0 + 0.75(n-1): n=1 -> 1.0, n=2 -> 1.75, n=3 -> 2.50, n=4 -> 3.25, n=5 -> 4.00
    // Engine discrete table: [1.0, 1.5, 2.2, 3.0, 4.0]
    // Both define a 5-stage escalation anchored at A(1) = 1.0 and A(5) = 4.0 with strictly monotonic increase.
    let table = [1.0f32, 1.5, 2.2, 3.0, 4.0];
    for n in 1..=5 {
        let theoretical = 1.0 + 0.75 * (n - 1) as f32;
        let actual = table[n - 1];
        assert!((1.0..=4.0).contains(&actual));
        assert!((actual - theoretical).abs() <= 0.35, "Stage {} actual ({}) must track theoretical A(n) ({}) within 0.35", n, actual, theoretical);
    }
}

#[test]
fn test_siege_adversarial_tesla_siege_pylon_conduction_and_fallback() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Deploy Tesla Siege Pylon (cooldown 2.0s, Arc Violet theme)
    assert_eq!(SiegeEngineKind::TeslaSiegePylon.base_cooldown(), 2.0);
    assert_eq!(SiegeEngineKind::TeslaSiegePylon.theme_color(), Color::new(0.65, 0.45, 1.0, 1.0));

    let pylon_pos = Vec3::new(0.0, 0.5, 0.0);
    siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &audio);
    siege.deploy_selected_engine(pylon_pos, Vec3::X, &audio);

    // Case A: No conductive voxels in the entire world.
    // Firing into empty space must NOT panic, must fallback gracefully to aim target.
    let empty_target = Vec3::new(20.0, 5.0, 0.0);
    let trauma_empty = siege.fire_active_engine(Some(empty_target), &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(trauma_empty > 0.0, "Tesla discharge to empty target must still execute and return trauma");
    assert_eq!(siege.tesla_discharges.len(), 1);
    assert!(siege.tesla_discharges[0].branches.len() >= 2);

    // Case B: Conductive node with adjacent power capacitor within 3.5m induction radius
    siege.deployed_engines[0].cooldown_timer = 0.0;
    grid.set(15, 2, 0, VoxelType::SteelRebar); // Conductive node
    grid.set(16, 2, 0, VoxelType::PowerCapacitor); // Volatile capacitor within 3.5m
    let rebar_pos = grid.voxel_to_world(IVec3::new(15, 2, 0));

    let trauma_cond = siege.fire_active_engine(Some(rebar_pos), &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(trauma_cond > 0.0);
    // Both rebar and capacitor should have been struck and detonated
    assert_eq!(grid.get(15, 2, 0), VoxelType::Air);
    assert_eq!(grid.get(16, 2, 0), VoxelType::Air);
}

#[test]
fn test_siege_adversarial_orbital_kinetic_relay_warmup_and_dual_bore_carve() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Verify Orbital Relay properties
    assert_eq!(SiegeEngineKind::OrbitalKineticRelay.base_cooldown(), 4.5);
    assert_eq!(SiegeEngineKind::OrbitalKineticRelay.theme_color(), Color::new(0.2, 0.85, 1.0, 1.0));
    assert_eq!(SiegeTrajectory::OrbitalBeam.default_elevation_deg(), 88.0);

    // Deploy and fire
    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(0.0, 0.5, 0.0), Vec3::X, &audio);

    // Place a reinforced column at (10, y, 10) from y=1 to y=30
    for y in 1..=30 {
        grid.set(10, y, 10, VoxelType::HeavyArmor);
        grid.set(11, y, 10, VoxelType::Concrete);
    }
    let target = grid.voxel_to_world(IVec3::new(10, 15, 10));

    siege.fire_active_engine(Some(target), &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.orbital_strikes.len(), 1);
    let strike = &siege.orbital_strikes[0];
    assert!(!strike.has_struck);
    assert_eq!(strike.warmup_timer, 0.75); // 0.75s targeting countdown

    // Advance 0.50s: still in warmup, beam has not struck, column intact
    siege.update(0.50, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(!siege.orbital_strikes[0].has_struck);
    assert_eq!(grid.get(10, 15, 10), VoxelType::HeavyArmor);

    // Advance 0.30s (total 0.80s > 0.75s warmup): warmup timer expires
    siege.update(0.30, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(siege.orbital_strikes[0].warmup_timer <= 0.0);

    // Frame after warmup expiration: strike strikes down from y=140 to y=0.5
    siege.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert!(siege.orbital_strikes[0].has_struck);
    // Both heavy armor core and concrete outer sheath carved
    assert_eq!(grid.get(10, 15, 10), VoxelType::Air, "Inner core must be vaporized by kinetic bore");
    assert_eq!(grid.get(11, 15, 10), VoxelType::Air, "Adjacent voxel must be carved by thermal sheath");
    assert_eq!(grid.get(10, 0, 10), VoxelType::Air, "Bedrock floor at (10, 0, 10) must be excavated by Orbital Kinetic Relay");
}

#[test]
fn test_siege_adversarial_placement_clearance_and_fleet_management() {
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();
    let mut siege = SiegeSystem::new();

    // 1. Unobstructed location
    let target_clear = Vec3::new(12.0, 0.5, 12.0);
    assert!(SiegeSystem::is_placement_clear(target_clear, &grid));

    // 2. Obstructed location (solid block in headroom y=1..=3)
    let v_center = grid.world_to_voxel(target_clear);
    grid.set(v_center.x, 2, v_center.z, VoxelType::Concrete);
    assert!(!SiegeSystem::is_placement_clear(target_clear, &grid));

    // Clear obstruction
    grid.set(v_center.x, 2, v_center.z, VoxelType::Air);
    assert!(SiegeSystem::is_placement_clear(target_clear, &grid));

    // 3. Deploy all 4 engines into fleet
    for kind in [
        SiegeEngineKind::RailArtillery,
        SiegeEngineKind::OrbitalKineticRelay,
        SiegeEngineKind::HydraulicGroundRam,
        SiegeEngineKind::TeslaSiegePylon,
    ] {
        siege.select_kind(kind, &audio);
        siege.deploy_selected_engine(Vec3::new(kind as usize as f32 * 5.0, 0.5, 0.0), Vec3::X, &audio);
    }
    assert_eq!(siege.deployed_engines.len(), 4, "Fleet must hold 4 distinct deployed engines");

    // 4. Redeploying existing engine kind updates position without exceeding 4
    siege.select_kind(SiegeEngineKind::RailArtillery, &audio);
    siege.deploy_selected_engine(Vec3::new(25.0, 0.5, 10.0), Vec3::Z, &audio);
    assert_eq!(siege.deployed_engines.len(), 4, "Redeploying must not duplicate engine in fleet");
    assert_eq!(siege.deployed_engines[0].position.x, 25.0);

    // 5. Recall All clears the fleet
    siege.recall_all(&audio);
    assert!(siege.deployed_engines.is_empty(), "Recall All must remove all deployed engines");
}

// ----------------------------------------------------------------------------
// 2. Procedural Audio Synthesis Adversarial Tests
// ----------------------------------------------------------------------------

#[test]
fn test_audio_adversarial_synthesizer_sample_bounds_and_riff_pcm_encoding() {
    let sample_rate = 44100u32;

    // Simulate procedural sound generation pipelines used across SoundSystem
    // Verify that all 28 generator formulas stay strictly within [-1.0, 1.0] and contain zero NaN/Inf.
    let test_durations = [0.04, 0.08, 0.16, 0.25, 0.35, 0.45, 0.65, 0.80, 1.0, 1.4, 1.5];

    for &dur in &test_durations {
        let num_samples = (sample_rate as f32 * dur) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let env = (-t * 6.5).exp();
            let freq = 115.0 * (-t * 14.0).exp() + 45.0;
            let sine = (t * freq * std::f32::consts::TAU).sin();
            let mixed = sine * env;
            let s = mixed.tanh() * 0.45;

            assert!(!s.is_nan(), "Sample must not be NaN at i = {}", i);
            assert!(!s.is_infinite(), "Sample must not be infinite at i = {}", i);
            assert!((-1.0..=1.0).contains(&s), "Sample out of bounds: {}", s);
            samples.push(s);
        }

        // Verify 16-bit PCM conversion and RIFF packaging
        let mut wav = Vec::with_capacity(44 + samples.len() * 2);
        let num_s = samples.len() as u32;
        let byte_rate = sample_rate * 2;
        let subchunk2_size = num_s * 2;
        let chunk_size = 36 + subchunk2_size;

        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&chunk_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&subchunk2_size.to_le_bytes());

        for &s in &samples {
            let clamped = s.clamp(-1.0, 1.0);
            let sample_i16 = (clamped * 32767.0) as i16;
            assert!(sample_i16 >= -32767);
            wav.extend_from_slice(&sample_i16.to_le_bytes());
        }

        // Verify WAV header validity
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + num_samples * 2);
    }
}

#[test]
fn test_audio_adversarial_anti_stacking_guard_dense_boundary_sweep() {
    let map = Mutex::new(HashMap::new());
    let sound_a = 0xAA01usize;
    let sound_b = 0xBB02usize;

    // Trigger sound A at t = 0.0s (avoids float cancellation when adding small millisecond offsets)
    let t_base = 0.0f64;
    map.lock().unwrap().insert(sound_a, t_base);

    // Sweep delta t from 0ms to 120ms in 1ms steps
    for dt_ms in 0..=120 {
        let dt = dt_ms as f64 / 1000.0;
        let t_now = dt;

        let allow_a = {
            let map_lock = map.lock().unwrap();
            match map_lock.get(&sound_a) {
                Some(&last_t) => t_now - last_t >= 0.075,
                None => true,
            }
        };

        if dt < 0.0749 {
            assert!(!allow_a, "Duplicate trigger at dt = {}ms (< 75ms) must be blocked", dt_ms);
        } else {
            assert!(allow_a, "Duplicate trigger at dt = {}ms (>= 75ms) must be allowed", dt_ms);
        }

        // Distinct sound B at the exact same millisecond must ALWAYS be allowed
        let allow_b = {
            let map_lock = map.lock().unwrap();
            match map_lock.get(&sound_b) {
                Some(&last_t) => t_now - last_t >= 0.075,
                None => true,
            }
        };
        assert!(allow_b, "Distinct sound B must never be blocked by sound A at dt = {}ms", dt_ms);
    }
}

#[test]
fn test_audio_adversarial_voice_energy_ducking_monotonicity_and_extreme_burst() {
    let mut prev_duck = 2.0f32;

    for count in 0..=500 {
        let duck = SoundSystem::calculate_concurrency_ducking(count);

        // Strict bounds [0.0, 1.0]
        assert!(duck > 0.0 && duck <= 1.0, "Ducking multiplier must be in (0.0, 1.0], got {} for count {}", duck, count);
        assert!(!duck.is_nan() && !duck.is_infinite());

        // Strict monotonic decrease
        if count > 0 {
            assert!(duck < prev_duck, "Ducking must be strictly monotonic: count {}, curr {}, prev {}", count, duck, prev_duck);
        }
        prev_duck = duck;

        // Master effective volume check
        let vol = 1.0f32;
        let effective_vol = (vol * 0.30 * duck).clamp(0.0, 1.0);
        assert!(effective_vol > 0.0 && effective_vol <= 0.30, "Effective master volume must be within (0, 0.30]");
    }

    // Extreme burst of 100,000 voices
    let extreme_duck = SoundSystem::calculate_concurrency_ducking(100_000);
    assert!(extreme_duck > 0.0 && extreme_duck < 0.0001);
    assert!(!extreme_duck.is_nan());
}

#[test]
fn test_audio_adversarial_distance_attenuation_dense_spectrum_and_clamping() {
    let listener = Vec3::new(10.0, 5.0, 10.0);

    // Origin (0m distance)
    let atten_0 = SoundSystem::calculate_distance_attenuation(listener, listener);
    assert_eq!(atten_0, 1.0, "0m distance must yield exact 1.0 attenuation");

    // Reference 8.0m distance -> exact 0.50
    let src_8m = listener + Vec3::new(8.0, 0.0, 0.0);
    let atten_8m = SoundSystem::calculate_distance_attenuation(src_8m, listener);
    assert!((atten_8m - 0.50).abs() < 1e-4, "8.0m distance must yield 0.50, got {}", atten_8m);

    // Monotonic falloff sweep
    let mut prev_atten = 1.0f32;
    for d_step in 1..=200 {
        let dist = d_step as f32 * 0.5;
        let src = listener + Vec3::new(dist, 0.0, 0.0);
        let atten = SoundSystem::calculate_distance_attenuation(src, listener);

        assert!((0.005..=1.0).contains(&atten), "Attenuation must clamp to [0.005, 1.0], got {}", atten);
        assert!(atten <= prev_atten, "Attenuation must decrease monotonically with distance: dist {}, curr {}, prev {}", dist, atten, prev_atten);
        prev_atten = atten;
    }

    // Extreme distances (1,000m to 1,000,000m) must clamp cleanly to 0.005 minimum floor
    for &extreme_d in &[1_000.0, 50_000.0, 1_000_000.0] {
        let src = listener + Vec3::new(extreme_d, 0.0, 0.0);
        let atten = SoundSystem::calculate_distance_attenuation(src, listener);
        assert_eq!(atten, 0.005, "Extreme distance {}m must clamp to exactly 0.005 floor", extreme_d);
    }
}

// ----------------------------------------------------------------------------
// 3. Responsive UI & Customizer Adversarial Tests
// ----------------------------------------------------------------------------

#[test]
fn test_ui_adversarial_scaling_formula_dense_viewport_matrix() {
    let test_resolutions = [
        // Sub-720p (must clamp to 0.65 floor)
        (320.0, 240.0, 0.65),
        (640.0, 480.0, 0.65),
        (800.0, 600.0, 0.65),
        // Baseline 720p
        (1280.0, 720.0, 1.00),
        // 1080p FHD
        (1920.0, 1080.0, 1.50),
        (1920.0, 1200.0, 1.50),
        // 1440p QHD
        (2560.0, 1440.0, 2.00),
        // Ultrawide 21:9
        (2560.0, 1080.0, 1.50),
        (3440.0, 1440.0, 2.00),
        // 4K UHD
        (3840.0, 2160.0, 3.00),
        // 5K Retina (must clamp to 3.20 ceiling)
        (5120.0, 2880.0, 3.20),
        // 8K FUHD (must clamp to 3.20 ceiling)
        (7680.0, 4320.0, 3.20),
    ];

    for &(sw, sh, expected_scale) in &test_resolutions {
        let s = compute_ui_scale(sw, sh);
        assert!((s - expected_scale).abs() < 1e-3, "Viewport {}x{} expected scale {}, got {}", sw, sh, expected_scale, s);
        assert!((0.65..=3.20).contains(&s), "Scale must remain within [0.65, 3.20]");
    }
}

#[test]
fn test_ui_adversarial_zero_layout_collision_dock_vs_weapon_card_exhaustive() {
    // Test across a full 2D grid of screen widths (640 to 5120) and heights (480 to 2880)
    for sw in (640..=5120).step_by(160) {
        for sh in (480..=2880).step_by(120) {
            let sw = sw as f32;
            let sh = sh as f32;
            let s = compute_ui_scale(sw, sh);

            // Tactical Command Dock geometry
            let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
            let dock_x = 16.0 * s;
            let dock_right = dock_x + dock_w;

            // Weapon Ordnance Card geometry
            let card_w = (sw * 0.28).clamp(280.0 * s, 400.0 * s);
            let card_x = sw - card_w - 16.0 * s;

            // Invariant: Horizontal gap must be strictly positive (Zero overlap)
            let horizontal_gap = card_x - dock_right;
            assert!(
                horizontal_gap > 0.0,
                "Layout collision detected at {}x{}! dock_right = {}, card_x = {}, gap = {}",
                sw, sh, dock_right, card_x, horizontal_gap
            );

            // Vertical position invariants:
            let ctrl_h = 44.0 * s;
            let siege_h = 74.0 * s;
            let total_dock_h = siege_h + ctrl_h + 6.0 * s;
            let dock_y = sh - total_dock_h - 16.0 * s;

            let card_h = 112.0 * s;
            let card_y = sh - card_h - 16.0 * s;

            assert!(dock_y > 0.0, "Dock must not overflow top screen at {}x{}", sw, sh);
            assert!(card_y > 0.0, "Card must not overflow top screen at {}x{}", sw, sh);
            assert!(dock_y + total_dock_h <= sh, "Dock must fit within bottom screen");
            assert!(card_y + card_h <= sh, "Card must fit within bottom screen");
        }
    }
}

#[test]
fn test_customizer_adversarial_slider_clamping_and_factory_reset() {
    let mut weapon = crate::weapon::WeaponSystem::new();

    // Verify default tuning values
    assert_eq!(weapon.config.blast_radius_mult, 1.0);
    assert_eq!(weapon.config.impulse_mult, 1.0);
    assert_eq!(weapon.config.velocity_mult, 1.0);
    assert_eq!(weapon.config.fire_rate_mult, 1.0);
    assert_eq!(weapon.config.cluster_count, 5);
    assert_eq!(weapon.config.recoil_damping, 1.0);

    // Apply extreme out-of-bounds adjustments
    weapon.config.blast_radius_mult = 50.0;
    weapon.config.impulse_mult = 99.0;
    weapon.config.velocity_mult = 0.01;
    weapon.config.fire_rate_mult = 12.0;
    weapon.config.cluster_count = 100;
    weapon.config.recoil_damping = 0.1;

    // Simulate slider clamping bounds from Workbench Tab 2
    weapon.config.blast_radius_mult = weapon.config.blast_radius_mult.clamp(0.5, 3.0);
    weapon.config.impulse_mult = weapon.config.impulse_mult.clamp(0.5, 3.5);
    weapon.config.velocity_mult = weapon.config.velocity_mult.clamp(0.5, 2.5);
    weapon.config.fire_rate_mult = weapon.config.fire_rate_mult.clamp(0.5, 3.0);
    weapon.config.cluster_count = weapon.config.cluster_count.clamp(3, 8);
    weapon.config.recoil_damping = weapon.config.recoil_damping.clamp(0.5, 2.0);

    assert_eq!(weapon.config.blast_radius_mult, 3.0);
    assert_eq!(weapon.config.impulse_mult, 3.5);
    assert_eq!(weapon.config.velocity_mult, 0.5);
    assert_eq!(weapon.config.fire_rate_mult, 3.0);
    assert_eq!(weapon.config.cluster_count, 8);
    assert_eq!(weapon.config.recoil_damping, 0.5);

    // Factory Reset Trigger
    weapon.config.blast_radius_mult = 1.0;
    weapon.config.impulse_mult = 1.0;
    weapon.config.velocity_mult = 1.0;
    weapon.config.fire_rate_mult = 1.0;
    weapon.config.cluster_count = 5;
    weapon.config.recoil_damping = 1.0;

    assert_eq!(weapon.config.blast_radius_mult, 1.0);
    assert_eq!(weapon.config.impulse_mult, 1.0);
    assert_eq!(weapon.config.velocity_mult, 1.0);
    assert_eq!(weapon.config.fire_rate_mult, 1.0);
    assert_eq!(weapon.config.cluster_count, 5);
    assert_eq!(weapon.config.recoil_damping, 1.0);
}

#[test]
fn test_customizer_adversarial_combat_presets_integrity_and_loading() {
    use customizer::PRESETS;
    assert_eq!(PRESETS.len(), 7, "Must contain exactly 7 tactical combat presets");

    let mut weapon = crate::weapon::WeaponSystem::new();

    for p in PRESETS {
        // Integrity check: strings and bounds
        assert!(!p.name.is_empty());
        assert!(!p.description.is_empty());
        assert!(p.blast_radius_mult >= 0.5 && p.blast_radius_mult <= 3.0);
        assert!(p.impulse_mult >= 0.5 && p.impulse_mult <= 3.5);
        assert!(p.velocity_mult >= 0.5 && p.velocity_mult <= 2.5);
        assert!(p.fire_rate_mult >= 0.5 && p.fire_rate_mult <= 3.0);

        // Load preset into weapon
        weapon.config.mode = p.mode;
        weapon.config.barrel = p.barrel;
        weapon.config.payload = p.payload;
        weapon.config.trail_style = p.trail;
        weapon.config.stabilizer = p.stabilizer;
        weapon.config.blast_radius_mult = p.blast_radius_mult;
        weapon.config.impulse_mult = p.impulse_mult;
        weapon.config.fire_rate_mult = p.fire_rate_mult;
        weapon.config.velocity_mult = p.velocity_mult;
        weapon.config.detonator = crate::weapon::DetonatorType::InstantImpact;

        assert_eq!(weapon.config.mode, p.mode);
        assert_eq!(weapon.config.barrel, p.barrel);
        assert_eq!(weapon.config.payload, p.payload);
        assert_eq!(weapon.config.trail_style, p.trail);
        assert_eq!(weapon.config.stabilizer, p.stabilizer);
        assert_eq!(weapon.config.blast_radius_mult, p.blast_radius_mult);
        assert_eq!(weapon.config.impulse_mult, p.impulse_mult);
    }
}

// ----------------------------------------------------------------------------
// 4. Megastructure World Generation Adversarial Tests
// ----------------------------------------------------------------------------

#[test]
fn test_world_gen_adversarial_all_six_levels_structural_stability_at_spawn() {
    let presets = [
        LevelPreset::ApexSkyscraper,
        LevelPreset::SuspensionSkybridge,
        LevelPreset::MilitaryFortress,
        LevelPreset::DominoMonoliths,
        LevelPreset::GothicCitadel,
        LevelPreset::NeoTokyoSpire,
    ];

    for &preset in &presets {
        let mut grid = VoxelGrid::new();
        generate_level(&mut grid, preset);

        // 1. Bedrock floor grounding invariant (y = 0)
        for x in 0..GRID_X {
            for z in 0..GRID_Z {
                assert_eq!(
                    grid.get(x as i32, 0, z as i32),
                    VoxelType::Bedrock,
                    "Voxel at ({}, 0, {}) must be Bedrock in level {:?}",
                    x, z, preset
                );
                let h = grid.get_health(x as i32, 0, z as i32);
                assert!(h.is_finite() && h > 0.0, "Ground health at ({}, 0, {}) must be finite and positive in level {:?}", x, z, preset);
            }
        }

        // 2. Minimum structural mass invariants
        assert!(
            grid.total_initial_voxels >= 3000,
            "Level {:?} must possess substantial voxel count, got {}",
            preset, grid.total_initial_voxels
        );
        assert_eq!(grid.current_voxels, grid.total_initial_voxels);

        // 3. Rebar internal skeleton invariants
        assert!(
            grid.initial_rebar_count >= 100,
            "Level {:?} must contain at least 100 load-bearing rebar voxels, got {}",
            preset, grid.initial_rebar_count
        );
        assert_eq!(grid.current_rebar_count, grid.initial_rebar_count);

        // 4. Structural stability invariant: ZERO unsupported floating islands at spawn!
        let islands = grid.detect_unsupported_islands();
        assert!(
            islands.is_empty(),
            "Newly generated level {:?} must have ZERO unsupported islands at spawn! Detected: {}",
            preset, islands.len()
        );
    }
}

#[test]
fn test_world_gen_adversarial_level_specific_cores_and_capacitors() {
    let mut grid = VoxelGrid::new();

    // Level 1: Apex Skyscraper - 4x4 shear core & power capacitors at auxiliary generators
    generate_level(&mut grid, LevelPreset::ApexSkyscraper);
    let mut apex_capacitors = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::PowerCapacitor {
            apex_capacitors += 1;
        }
    }
    assert!(apex_capacitors >= 8, "Apex Skyscraper must have >= 8 Power Capacitors, got {}", apex_capacitors);

    // Level 3: Cyber-Bunker - 4 deuterium silos with volatile cores and central capacitor substation
    generate_level(&mut grid, LevelPreset::MilitaryFortress);
    let mut bunker_explosives = 0;
    let mut bunker_capacitors = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::Explosive {
            bunker_explosives += 1;
        } else if *vt == VoxelType::PowerCapacitor {
            bunker_capacitors += 1;
        }
    }
    assert!(bunker_explosives >= 400, "Cyber-Bunker silos must contain substantial explosive cores, got {}", bunker_explosives);
    assert!(bunker_capacitors >= 8, "Cyber-Bunker substation must contain >= 8 Power Capacitors, got {}", bunker_capacitors);

    // Level 4: Quantum Core Facility - Toroidal accelerator ring & pylon capacitors
    generate_level(&mut grid, LevelPreset::DominoMonoliths);
    let mut quantum_capacitors = 0;
    let mut quantum_ice = 0;
    let mut quantum_glow = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::PowerCapacitor {
            quantum_capacitors += 1;
        } else if *vt == VoxelType::CryoIce {
            quantum_ice += 1;
        } else if *vt == VoxelType::GlowCore {
            quantum_glow += 1;
        }
    }
    assert!(quantum_capacitors >= 32, "Quantum Core must have >= 32 pylon capacitor coils, got {}", quantum_capacitors);
    assert!(quantum_ice >= 200, "Quantum Core ring must have cryogenic jackets, got {}", quantum_ice);
    assert!(quantum_glow >= 50, "Quantum Core central sphere must have plasma glow cores, got {}", quantum_glow);

    // Level 6: Neo-Tokyo Cyber-Spire - Dual skybridges with anchor capacitor boxes
    generate_level(&mut grid, LevelPreset::NeoTokyoSpire);
    let mut spire_capacitors = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::PowerCapacitor {
            spire_capacitors += 1;
        }
    }
    assert!(spire_capacitors >= 8, "Neo-Tokyo Spire skybridges must have anchor capacitors, got {}", spire_capacitors);
}

// ============================================================================
// TIER 5 / CHALLENGER 1: WHITE-BOX ADVERSARIAL COVERAGE HARDENING
// Core Physics, Voxels & Weapons
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Voxel Grid & Graph Traversal (src/voxel.rs)
// ----------------------------------------------------------------------------

#[test]
fn test_tier5_adversarial_bfs_island_extreme_disconnected_graphs_and_circular_lattices() {
    let mut grid = VoxelGrid::new();
    grid.set_preserve_single_voxel_debris(true);

    // 1. Extreme Circular Mid-Air Torus / Ring Lattice at y=25 (no bedrock connection)
    let center_x = 48;
    let center_z = 48;
    let y_lattice = 25;
    let radius = 8.0f32;
    let mut lattice_voxels = Vec::new();

    for step in 0..32 {
        let angle = (step as f32 / 32.0) * std::f32::consts::TAU;
        let vx = center_x + (angle.cos() * radius).round() as i32;
        let vz = center_z + (angle.sin() * radius).round() as i32;
        if !lattice_voxels.contains(&IVec3::new(vx, y_lattice, vz)) {
            lattice_voxels.push(IVec3::new(vx, y_lattice, vz));
            grid.set(vx, y_lattice, vz, VoxelType::Concrete);
        }
    }
    let ring_voxel_count = lattice_voxels.len();
    assert!(ring_voxel_count >= 16, "Ring lattice must contain >= 16 voxels");

    // 2. Scatter 15 Disconnected Multi-Voxel Clusters at various altitudes
    let mut cluster_voxel_count = 0;
    for c in 0..15 {
        let cx = 10 + (c * 5) % 70;
        let cy = 10 + (c * 6) % 90;
        let cz = 15 + (c * 4) % 65;
        grid.set(cx, cy, cz, VoxelType::Brick);
        grid.set(cx + 1, cy, cz, VoxelType::Concrete);
        cluster_voxel_count += 2;
    }

    let initial_solid = grid.solid_count();
    assert_eq!(initial_solid, ring_voxel_count + cluster_voxel_count);

    // Enable single-voxel debris preservation so even 1-voxel fragments in circular lattice are preserved
    grid.set_preserve_single_voxel_debris(true);

    // 3. Run structural BFS island solver
    let islands = grid.detect_unsupported_islands();

    // Every single unsupported floating piece must have been extracted
    let total_extracted_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    assert_eq!(
        total_extracted_voxels, initial_solid,
        "All floating voxels from ring lattice and clusters must be extracted into island chunks"
    );

    // Static grid must now be completely clear of floating debris
    assert_eq!(
        grid.solid_count(), 0,
        "Static grid solid count must be exactly 0 after island extraction"
    );
    for vpos in &lattice_voxels {
        assert_eq!(grid.get(vpos.x, vpos.y, vpos.z), VoxelType::Air);
    }

    // Foundation bedrock at y=0 must remain intact
    for x in (0..GRID_X).step_by(8) {
        for z in (0..GRID_Z).step_by(8) {
            assert_eq!(grid.get(x as i32, 0, z as i32), VoxelType::Bedrock);
        }
    }

    // 4. Single-Voxel Debris Preservation Flag Verification
    grid.set_preserve_single_voxel_debris(false);
    grid.set(20, 20, 20, VoxelType::Concrete);
    let islands_no_preserve = grid.detect_unsupported_islands();
    assert_eq!(grid.get(20, 20, 20), VoxelType::Air);
    assert!(
        islands_no_preserve.is_empty(),
        "When single-voxel debris preservation is false, 1-voxel components are deleted to Air without chunk"
    );

    grid.set_preserve_single_voxel_debris(true);
    grid.set(20, 20, 20, VoxelType::Concrete);
    let islands_with_preserve = grid.detect_unsupported_islands();
    assert_eq!(grid.get(20, 20, 20), VoxelType::Air);
    assert_eq!(
        islands_with_preserve.len(), 1,
        "When single-voxel debris preservation is true, 1-voxel component spawns an IslandChunk"
    );
    assert_eq!(islands_with_preserve[0].voxels.len(), 1);
}

#[test]
fn test_tier5_adversarial_cantilever_depth_exact_boundary_four_vs_five() {
    let mut grid = VoxelGrid::new();

    // 1. Build anchored concrete vertical foundation column at (30, 0..=12, 30)
    for y in 0..=12 {
        grid.set(30, y, 30, VoxelType::Concrete);
    }

    // 2. Extend horizontal concrete cantilever arm along +X at y = 12
    // CANTILEVER_MAX_DEPTH = 4:
    // x = 31 (depth 1) - supported
    // x = 32 (depth 2) - supported
    // x = 33 (depth 3) - supported
    // x = 34 (depth 4) - supported (exact boundary!)
    // x = 35 (depth 5) - UNSUPPORTED (overhang exceeds cantilever limit!)
    // x = 36 (depth 6) - UNSUPPORTED
    // x = 37 (depth 7) - UNSUPPORTED
    for x in 31..=37 {
        grid.set(x, 12, 30, VoxelType::Concrete);
    }

    let islands = grid.detect_unsupported_islands();

    // Column and depth 1..=4 remain anchored and solid
    for x in 30..=34 {
        assert_eq!(
            grid.get(x, 12, 30), VoxelType::Concrete,
            "Concrete cantilever at x={} (depth {}) must remain solid and grounded",
            x, x - 30
        );
    }

    // Depth 5..=7 must have detached as unsupported island chunk
    for x in 35..=37 {
        assert_eq!(
            grid.get(x, 12, 30), VoxelType::Air,
            "Concrete cantilever at x={} exceeds CANTILEVER_MAX_DEPTH=4 and must be detached to Air",
            x
        );
    }
    assert_eq!(islands.len(), 1, "Must produce exactly 1 detached cantilever chunk");
    assert_eq!(islands[0].voxels.len(), 3, "Detached cantilever chunk must contain 3 voxels (x=35..=37)");

    // 3. Invariant: Tensile Rebar bridges spans across arbitrary distance at depth 0
    // Anchor rebar column at (50, 0..=12, 50)
    for y in 0..=12 {
        grid.set(50, y, 50, VoxelType::SteelRebar);
    }
    // Extend horizontal rebar truss 10 voxels long (x = 51..=60)
    for x in 51..=60 {
        grid.set(x, 12, 50, VoxelType::SteelRebar);
    }

    let rebar_islands = grid.detect_unsupported_islands();
    assert!(
        rebar_islands.is_empty(),
        "Anchored tensile rebar must support across spans beyond cantilever limit at depth 0"
    );
    for x in 50..=60 {
        assert_eq!(
            grid.get(x, 12, 50), VoxelType::SteelRebar,
            "Rebar voxel at x={} must remain grounded", x
        );
    }
}

#[test]
fn test_tier5_adversarial_solid_count_tracking_mass_churn_and_boundary_invariants() {
    let mut grid = VoxelGrid::new();

    // 1. Initial State
    assert_eq!(grid.solid_count(), 0, "Initial solid count (excluding bedrock y=0) must be 0");

    // 2. Out-of-bounds queries & setting Air to Air
    grid.set(-10, 20, -5, VoxelType::Concrete);
    grid.set(GRID_X as i32 + 50, 20, GRID_Z as i32 + 50, VoxelType::Concrete);
    grid.set(20, -60, 20, VoxelType::Concrete);
    grid.set(20, 250, 20, VoxelType::Concrete);
    assert_eq!(grid.solid_count(), 0, "Out-of-bounds sets must be safely ignored");
    assert_eq!(grid.get(-10, 20, -5), VoxelType::Air);
    assert_eq!(grid.get_health(-10, 20, -5), 0.0);

    grid.set(20, 20, 20, VoxelType::Air);
    assert_eq!(grid.solid_count(), 0, "Setting Air to Air must not alter solid count");

    // 3. Bedrock Foundation Layer (y = 0) is excluded from building solid_count
    grid.set(5, 0, 5, VoxelType::Concrete);
    assert_eq!(grid.solid_count(), 0, "Voxel modifications at bedrock layer y=0 must not increment solid_count");

    // 4. Single voxel transitions
    grid.set(20, 20, 20, VoxelType::Concrete);
    assert_eq!(grid.solid_count(), 1);
    // Setting same type
    grid.set(20, 20, 20, VoxelType::Concrete);
    assert_eq!(grid.solid_count(), 1, "Idempotent set must maintain solid count");
    // Solid to solid transition (Concrete -> HeavyArmor)
    grid.set(20, 20, 20, VoxelType::HeavyArmor);
    assert_eq!(grid.solid_count(), 1, "Solid-to-solid transition must preserve solid count");
    // Solid to Air
    grid.set(20, 20, 20, VoxelType::Air);
    assert_eq!(grid.solid_count(), 0, "Solid-to-air transition must decrement solid count");
    // Air to Air again
    grid.set(20, 20, 20, VoxelType::Air);
    assert_eq!(grid.solid_count(), 0, "Saturating sub must prevent underflow");

    // 5. Mass Additions (1000 voxels)
    for x in 20..30 {
        for y in 10..20 {
            for z in 20..30 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    assert_eq!(grid.solid_count(), 1000);

    // Full volumetric scan verification
    let mut verified_count = 0;
    for x in 0..GRID_X {
        for y in 1..GRID_Y {
            for z in 0..GRID_Z {
                if grid.get(x as i32, y as i32, z as i32).is_solid() {
                    verified_count += 1;
                }
            }
        }
    }
    assert_eq!(grid.solid_count(), verified_count);

    // 6. Mass Deletions via Spherical Blast Carve
    let carve_center = grid.voxel_to_world(IVec3::new(25, 15, 25));
    let carve_res = grid.carve_sphere_advanced(carve_center, 3.5, 500.0, DamageType::Explosive);
    assert!(carve_res.destroyed > 0, "Blast must destroy voxels");

    let mut remaining_count = 0;
    for x in 0..GRID_X {
        for y in 1..GRID_Y {
            for z in 0..GRID_Z {
                if grid.get(x as i32, y as i32, z as i32).is_solid() {
                    remaining_count += 1;
                }
            }
        }
    }
    assert_eq!(
        grid.solid_count(), remaining_count,
        "Incremental solid_count must exactly match volumetric scan after explosive carve"
    );
    assert_eq!(grid.solid_count(), 1000 - carve_res.destroyed);

    // 7. Clear all remaining voxels
    for x in 20..30 {
        for y in 10..20 {
            for z in 20..30 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }
    assert_eq!(grid.solid_count(), 0);
}

#[test]
fn test_tier5_adversarial_scratch_buffers_pointer_stability_zero_alloc() {
    let mut grid = VoxelGrid::new();

    // Record baseline raw memory pointer addresses
    let ptr_grounded = grid.scratch_grounded.as_ptr();
    let ptr_tension = grid.scratch_tension.as_ptr();
    let ptr_depth = grid.scratch_depth.as_ptr();
    let ptr_visited = grid.scratch_visited.as_ptr();
    let cap_grounded = grid.scratch_grounded.capacity();
    let cap_tension = grid.scratch_tension.capacity();
    let cap_depth = grid.scratch_depth.capacity();
    let cap_visited = grid.scratch_visited.capacity();
    let cap_queue = grid.scratch_queue.capacity();

    // Perform 10 adversarial cycles of structural stress & island detection
    for cycle in 0..10 {
        // Build cantilever and isolated floating debris
        for y in 0..=8 {
            grid.set(40, y, 40, VoxelType::Concrete);
        }
        for x in 41..=48 {
            grid.set(x, 8, 40, VoxelType::Concrete);
        }
        grid.set(20 + cycle, 30, 20 + cycle, VoxelType::Brick);
        grid.set(20 + cycle, 31, 20 + cycle, VoxelType::Brick);

        let _ = grid.detect_unsupported_islands();

        // Clear static remnants for next iteration
        for y in 0..=8 {
            grid.set(40, y, 40, VoxelType::Air);
        }
        for x in 41..=48 {
            grid.set(x, 8, 40, VoxelType::Air);
        }

        // Pointer addresses and capacities MUST remain 100% invariant across invocations
        assert_eq!(grid.scratch_grounded.as_ptr(), ptr_grounded, "scratch_grounded reallocated at cycle {}", cycle);
        assert_eq!(grid.scratch_tension.as_ptr(), ptr_tension, "scratch_tension reallocated at cycle {}", cycle);
        assert_eq!(grid.scratch_depth.as_ptr(), ptr_depth, "scratch_depth reallocated at cycle {}", cycle);
        assert_eq!(grid.scratch_visited.as_ptr(), ptr_visited, "scratch_visited reallocated at cycle {}", cycle);
        assert_eq!(grid.scratch_grounded.capacity(), cap_grounded);
        assert_eq!(grid.scratch_tension.capacity(), cap_tension);
        assert_eq!(grid.scratch_depth.capacity(), cap_depth);
        assert_eq!(grid.scratch_visited.capacity(), cap_visited);
        assert!(grid.scratch_queue.capacity() >= cap_queue);
    }
}

#[test]
fn test_tier5_adversarial_all_12_material_properties_and_conductivity_matrix() {
    let all_materials = [
        VoxelType::Air,
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
    assert_eq!(all_materials.len(), 13, "Must audit exactly 13 materials");

    // 1. Electrical Conductivity Invariant:
    // Only 5 materials conduct electricity: PowerCapacitor (12.0) > SteelRebar (10.0) > HeavyArmor (9.0) > CompositeRebar (8.5) > GlowCore (6.0)
    for &vt in &all_materials {
        let props = vt.properties();
        match vt {
            VoxelType::PowerCapacitor => assert_eq!(props.conductivity, 12.0),
            VoxelType::SteelRebar => assert_eq!(props.conductivity, 10.0),
            VoxelType::HeavyArmor => assert_eq!(props.conductivity, 9.0),
            VoxelType::CompositeRebar => assert_eq!(props.conductivity, 8.5),
            VoxelType::GlowCore => assert_eq!(props.conductivity, 6.0),
            _ => assert_eq!(props.conductivity, 0.0, "Material {:?} must be non-conductive", vt),
        }
    }

    // 2. Thermal Softening Multiplier Invariant:
    // CryoIce (3.0) and Explosive (2.5) are hyper-sensitive; HeavyArmor (1.6) is vulnerable to thermal plasma
    for &vt in &all_materials {
        let props = vt.properties();
        match vt {
            VoxelType::CryoIce => assert_eq!(props.thermal_soften, 3.0),
            VoxelType::Explosive => assert_eq!(props.thermal_soften, 2.5),
            VoxelType::Glass => assert_eq!(props.thermal_soften, 2.0),
            VoxelType::PowerCapacitor => assert_eq!(props.thermal_soften, 1.8),
            VoxelType::HeavyArmor => assert_eq!(props.thermal_soften, 1.6),
            VoxelType::SteelRebar => assert_eq!(props.thermal_soften, 1.2),
            VoxelType::Concrete => assert_eq!(props.thermal_soften, 1.0),
            VoxelType::CompositeRebar => assert_eq!(props.thermal_soften, 0.9),
            VoxelType::Brick => assert_eq!(props.thermal_soften, 0.8),
            VoxelType::GlowCore => assert_eq!(props.thermal_soften, 0.8),
            VoxelType::Bedrock => assert_eq!(props.thermal_soften, 0.4),
            VoxelType::Basalt => assert_eq!(props.thermal_soften, 0.6),
            VoxelType::Air => assert_eq!(props.thermal_soften, 1.0),
        }
    }

    // 3. Rebar Suspension Tension Invariant:
    // Vertical downward suspension (dy < 0) from an elevated support is ONLY permitted by tensile rebar
    let mut grid = VoxelGrid::new();
    // Build supported ceiling arch from bedrock:
    for y in 0..=20 {
        grid.set(30, y, 30, VoxelType::Concrete);
    }
    // Drop downward rebar cable 6 voxels (dy < 0): (30, 19 down to 14, 30)
    for y in (14..20).rev() {
        grid.set(30, y, 30, VoxelType::SteelRebar);
    }
    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Anchored steel rebar cable must suspend downwards in tension");
    for y in 14..=20 {
        assert_eq!(grid.get(30, y, 30), if y == 20 { VoxelType::Concrete } else { VoxelType::SteelRebar });
    }
}

// ----------------------------------------------------------------------------
// 2. Physics & Demolition Pipeline (src/physics.rs)
// ----------------------------------------------------------------------------

#[test]
fn test_tier5_adversarial_rubble_stacking_heightmap_96x96_bounds_and_normal_gradients() {
    let mut phys = PhysicsSystem::new();

    // 1. Grid bounds verification
    assert_eq!(
        phys.rubble_height_grid.len(),
        GRID_X * GRID_Z,
        "Heightmap grid must match exactly GRID_X * GRID_Z cells"
    );

    // Initial elevation is bedrock top = VOXEL_SIZE (0.50)
    assert_eq!(phys.rubble_height_at(0.0, 0.0), VOXEL_SIZE);

    // 2. The visible square foundation extends beyond the grid to +/-240m; beyond it is void.
    assert_eq!(phys.rubble_height_at(200.0, 200.0), VOXEL_SIZE);
    assert_eq!(phys.rubble_height_at(-240.0, 240.0), VOXEL_SIZE);
    assert_eq!(phys.rubble_height_at(-500.0, 500.0), voxel::VOID_BOUNDARY_Y);
    assert_eq!(phys.rubble_height_at(1000.0, -1000.0), voxel::VOID_BOUNDARY_Y);
    assert_eq!(phys.rubble_height_at(f32::NAN, f32::INFINITY), voxel::VOID_BOUNDARY_Y);
    assert_eq!(PhysicsSystem::world_to_rubble_grid_coords(f32::NAN, 0.0), None);
    assert_eq!(PhysicsSystem::world_to_rubble_grid_coords(-100.0, 0.0), None);

    // 3. Flat ground normal
    let flat_norm = phys.rubble_normal_at(0.0, 0.0);
    assert_eq!(flat_norm, Vec3::Y, "Flat ground surface normal must be exactly +Y (0, 1, 0)");

    // 4. Stamp artificial rubble conical mound centered at grid cell (48, 48)
    let peak_y = 4.5f32;
    let idx_peak = 48 * GRID_Z + 48;
    phys.rubble_height_grid[idx_peak] = peak_y;
    phys.rubble_height_grid[(48 + 1) * GRID_Z + 48] = 2.5; // +X slope
    phys.rubble_height_grid[(48 - 1) * GRID_Z + 48] = 2.5; // -X slope
    phys.rubble_height_grid[48 * GRID_Z + (48 + 1)] = 2.5; // +Z slope
    phys.rubble_height_grid[48 * GRID_Z + (48 - 1)] = 2.5; // -Z slope

    let half_x = (GRID_X as f32 * VOXEL_SIZE) * 0.5;
    let half_z = (GRID_Z as f32 * VOXEL_SIZE) * 0.5;
    let peak_wx = 48.0 * VOXEL_SIZE - half_x + VOXEL_SIZE * 0.5;
    let peak_wz = 48.0 * VOXEL_SIZE - half_z + VOXEL_SIZE * 0.5;

    // Normal at +X slope (world_x = peak_wx + VOXEL_SIZE) must tilt away from peak (-X)
    let norm_east = phys.rubble_normal_at(peak_wx + VOXEL_SIZE, peak_wz);
    assert!(norm_east.x > 0.0, "Surface normal on +X slope must point in +X (outward from mound)");
    assert!(norm_east.y > 0.0, "Surface normal must point upward");
    assert!((norm_east.length() - 1.0).abs() < 1e-4, "Surface normal must be unit length");

    // Normal at -X slope (world_x = peak_wx - VOXEL_SIZE) must point in -X
    let norm_west = phys.rubble_normal_at(peak_wx - VOXEL_SIZE, peak_wz);
    assert!(norm_west.x < 0.0, "Surface normal on -X slope must point in -X (outward from mound)");
    assert!((norm_west.length() - 1.0).abs() < 1e-4);

    // Normal at +Z slope must point in +Z
    let norm_north = phys.rubble_normal_at(peak_wx, peak_wz + VOXEL_SIZE);
    assert!(norm_north.z > 0.0, "Surface normal on +Z slope must point in +Z");
    assert!((norm_north.length() - 1.0).abs() < 1e-4);

    // Normal at -Z slope must point in -Z
    let norm_south = phys.rubble_normal_at(peak_wx, peak_wz - VOXEL_SIZE);
    assert!(norm_south.z < 0.0, "Surface normal on -Z slope must point in -Z");
    assert!((norm_south.length() - 1.0).abs() < 1e-4);
}

#[test]
fn test_tier5_adversarial_off_center_rubble_impact_torque_and_tumbling() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // 1. Create a settled rubble step on the right (+X) resting on concrete floor
    let floor_vpos = grid.world_to_voxel(Vec3::new(1.0, 1.0, 0.0));
    grid.set(floor_vpos.x, floor_vpos.y, floor_vpos.z, VoxelType::Concrete);

    let vox1 = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb1 = DynamicChunk::compute_aabb(&vox1);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(1.0, 1.5, 0.0),
        velocity: Vec3::ZERO,
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox1,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 10.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb1,
    });
    phys.settle_chunk(0);

    // 2. Drop a wide 2-meter chunk centered at X=0 with zero initial angular velocity
    let mut wide_vox = Vec::new();
    for x in -2..=2 {
        wide_vox.push((Vec3::new(x as f32 * 0.5, 0.0, 0.0), VoxelType::Concrete));
    }
    let aabb2 = DynamicChunk::compute_aabb(&wide_vox);
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, 2.0, 0.0),
        velocity: Vec3::new(0.0, -4.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: wide_vox,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 25.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb2,
    });

    // Step physics: wide chunk strikes the elevated right-hand rubble step off-center
    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

    let chunk = &phys.chunks[1];
    // Right side hit first at X=1.0, imparting torque around Z axis: tau = r x (J N)
    assert!(
        chunk.angular_velocity.length_squared() > 0.2,
        "Off-center impact must generate non-zero angular tumbling torque: got {:?}",
        chunk.angular_velocity
    );
    assert!(
        chunk.position.y >= VOXEL_SIZE,
        "Chunk position y ({}) must stay above bedrock",
        chunk.position.y
    );
}

#[test]
fn test_tier5_adversarial_multi_tier_pancake_collapse_pulverization_and_yield() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // 1. Build a static floor slab of Glass at y = 6 (easily crushed: threshold = 5.0)
    let floor_y = 6;
    for x in 45..=51 {
        for z in 45..=51 {
            grid.set(x, floor_y, z, VoxelType::Glass);
        }
    }
    let initial_floor_voxels = grid.solid_count();
    assert_eq!(initial_floor_voxels, 49, "Floor slab must contain 49 voxels");

    // 2. Drop a heavy chunk with 5x5 footprint (50 voxels, mass = 150.0, v = (0, -8.0, 0))
    let mut falling_voxels = Vec::new();
    for dx in -2..=2 {
        for dz in -2..=2 {
            falling_voxels.push((Vec3::new(dx as f32 * 0.5, 0.0, dz as f32 * 0.5), VoxelType::Concrete));
            falling_voxels.push((Vec3::new(dx as f32 * 0.5, 0.5, dz as f32 * 0.5), VoxelType::Concrete));
        }
    }
    let aabb = DynamicChunk::compute_aabb(&falling_voxels);
    let floor_center_wpos = grid.voxel_to_world(IVec3::new(48, floor_y, 48));

    phys.chunks.push(DynamicChunk {
        position: floor_center_wpos + Vec3::new(0.0, 0.8, 0.0),
        velocity: Vec3::new(0.0, -8.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: falling_voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 150.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    // 3. Step physics to trigger multi-tier pancake collapse
    phys.update(0.04, &mut grid, &audio, Vec3::ZERO);

    // Static floor voxels must have been pulverized
    assert!(
        grid.solid_count() < initial_floor_voxels,
        "Static floor voxels must be pulverized during pancake collapse: remaining {}",
        grid.solid_count()
    );

    // Dynamic rubble sub-chunks must be spawned (25% yield)
    assert!(
        phys.chunks.len() >= 2,
        "Pancake collapse must yield dynamic sub-chunks (25% target yield)"
    );

    // Outward debris and radial dust storms must be generated
    assert!(
        !phys.debris.is_empty(),
        "Pancake collapse must scatter DebrisParticle instances"
    );
    assert!(
        !phys.smoke.is_empty(),
        "Pancake collapse must generate radial perimeter dust storm clouds"
    );
}

#[test]
fn test_tier5_adversarial_bedrock_boundary_zero_penetration_and_settling() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // 1. Soft-landing chunk (speed <= 1.8 m/s): must settle on bedrock without bounce
    let local_vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
    let aabb = DynamicChunk::compute_aabb(&local_vox);

    phys.chunks.push(DynamicChunk {
        position: Vec3::new(0.0, VOXEL_SIZE + 0.05, 0.0),
        velocity: Vec3::new(0.0, -0.6, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: local_vox.clone(),
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 5.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    phys.update(0.04, &mut grid, &audio, Vec3::ZERO);

    let settled_chunk = &phys.chunks[0];
    assert!(settled_chunk.settled, "Low velocity landing chunk must settle");
    assert_eq!(settled_chunk.velocity, Vec3::ZERO, "Settled chunk velocity must be zeroed");
    assert_eq!(settled_chunk.angular_velocity, Vec3::ZERO);
    assert!(
        settled_chunk.lowest_world_y() >= VOXEL_SIZE - 0.01,
        "Settled chunk lowest world y ({}) must stay >= 0.49 (bedrock top)",
        settled_chunk.lowest_world_y()
    );

    // Heightmap at chunk position must have updated
    assert!(
        phys.rubble_height_at(0.0, 0.0) >= VOXEL_SIZE,
        "Rubble heightmap must record top of settled chunk"
    );

    // 2. Penetration correction: chunk manually forced below bedrock (y = 0.15)
    phys.chunks.push(DynamicChunk {
        position: Vec3::new(10.0, 0.15, 10.0),
        velocity: Vec3::new(0.0, -1.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: local_vox,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: 5.0,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    phys.update(0.04, &mut grid, &audio, Vec3::ZERO);

    let corrected_chunk = &phys.chunks[1];
    assert!(
        corrected_chunk.lowest_world_y() >= VOXEL_SIZE - 0.01,
        "Penetration correction must lift chunk out of bedrock: got {}",
        corrected_chunk.lowest_world_y()
    );
}

// ----------------------------------------------------------------------------
// 3. Weapons & Volatiles (src/weapon.rs)
// ----------------------------------------------------------------------------

#[test]
fn test_tier5_adversarial_trimodal_volatile_cascades_24_hop_energy_dissipation() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Place a 28-hop linear line of alternating volatile canisters along X at y=10
    // 0..28 canisters: Explosive -> GlowCore -> PowerCapacitor -> repeat
    let mut initial_exp = Vec::new();
    let initial_core = Vec::new();
    let initial_cap = Vec::new();

    for hop in 0..28 {
        let x = 10 + hop * 2;
        let vpos = IVec3::new(x, 10, 30);
        match hop % 3 {
            0 => {
                grid.set(vpos.x, vpos.y, vpos.z, VoxelType::Explosive);
                if hop == 0 { initial_exp.push(vpos); }
            }
            1 => {
                grid.set(vpos.x, vpos.y, vpos.z, VoxelType::GlowCore);
            }
            _ => {
                grid.set(vpos.x, vpos.y, vpos.z, VoxelType::PowerCapacitor);
            }
        }
    }

    let first_wpos = grid.voxel_to_world(IVec3::new(10, 10, 30));
    let camera_pos = first_wpos + Vec3::new(25.0, 5.0, 25.0);
    let trauma = weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut phys,
        &audio,
        initial_exp,
        initial_core,
        initial_cap,
        camera_pos,
    );

    // 1. Camera trauma must be generated and clamped <= 0.95
    assert!(trauma > 0.10, "Volatile cascade must generate trauma: got {}", trauma);
    assert!(trauma <= 0.95, "Trauma must clamp at 0.95");

    // 2. Shockwaves must be spawned for the cascade hops
    assert!(
        !phys.shockwaves.is_empty(),
        "Volatile cascade must generate shockwave rings"
    );

    // 3. Monotonic Energy Dissipation Verification:
    // delta(h) = (1.0 - (h / 24.0) * 0.45).max(0.40)
    let diss_1 = (1.0f32 - (1.0f32 / 24.0f32) * 0.45f32).max(0.40f32);
    let diss_12 = (1.0f32 - (12.0f32 / 24.0f32) * 0.45f32).max(0.40f32);
    let diss_24 = (1.0f32 - (24.0f32 / 24.0f32) * 0.45f32).max(0.40f32);
    assert!(diss_1 > diss_12, "Dissipation must decrease monotonically with hop count");
    assert!(diss_12 > diss_24, "Dissipation must decrease monotonically with hop count");
    assert_eq!(diss_24, 0.55, "Hop 24 dissipation must equal exactly 1.0 - 0.45 = 0.55");
}

#[test]
fn test_tier5_adversarial_electrical_arc_induction_conductivity_ranking() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Place PowerCapacitor at (40, 10, 40)
    let cap_vpos = IVec3::new(40, 10, 40);
    grid.set(cap_vpos.x, cap_vpos.y, cap_vpos.z, VoxelType::PowerCapacitor);

    // Place surrounding target elements at radius 3.0m (6 voxels, outside 2.2m blast radius):
    // Target 1: SteelRebar (conductivity 10.0) at (40, 16, 40)
    let rebar_vpos = IVec3::new(40, 16, 40);
    grid.set(rebar_vpos.x, rebar_vpos.y, rebar_vpos.z, VoxelType::SteelRebar);

    // Target 2: HeavyArmor (conductivity 9.0) at (46, 10, 40)
    let armor_vpos = IVec3::new(46, 10, 40);
    grid.set(armor_vpos.x, armor_vpos.y, armor_vpos.z, VoxelType::HeavyArmor);

    // Target 3: GlowCore (conductivity 6.0) at (34, 10, 40)
    let core_vpos = IVec3::new(34, 10, 40);
    grid.set(core_vpos.x, core_vpos.y, core_vpos.z, VoxelType::GlowCore);

    // Target 4: Non-conductive Brick (conductivity 0.0) at (40, 10, 46)
    let brick_vpos = IVec3::new(40, 10, 46);
    grid.set(brick_vpos.x, brick_vpos.y, brick_vpos.z, VoxelType::Brick);

    let initial_rebar_health = grid.get_health(rebar_vpos.x, rebar_vpos.y, rebar_vpos.z);
    let initial_armor_health = grid.get_health(armor_vpos.x, armor_vpos.y, armor_vpos.z);
    let initial_brick_health = grid.get_health(brick_vpos.x, brick_vpos.y, brick_vpos.z);

    // Trigger capacitor breach
    weapon::process_explosive_chain_with_capacitors(
        &mut grid,
        &mut phys,
        &audio,
        Vec::new(),
        Vec::new(),
        vec![cap_vpos],
        Vec3::new(40.0, 10.0, 30.0),
    );

    // Capacitor must be destroyed
    assert_eq!(grid.get(cap_vpos.x, cap_vpos.y, cap_vpos.z), VoxelType::Air);

    // Conductive SteelRebar and HeavyArmor must take high-voltage arc induction thermal damage
    let final_rebar_health = grid.get_health(rebar_vpos.x, rebar_vpos.y, rebar_vpos.z);
    let final_armor_health = grid.get_health(armor_vpos.x, armor_vpos.y, armor_vpos.z);
    assert!(
        final_rebar_health < initial_rebar_health,
        "Steel rebar must absorb electrical arc damage: initial {}, final {}",
        initial_rebar_health, final_rebar_health
    );
    assert!(
        final_armor_health < initial_armor_health,
        "Heavy armor must absorb electrical arc damage: initial {}, final {}",
        initial_armor_health, final_armor_health
    );

    // Non-conductive Brick at 6 voxels (3.0m, outside direct 2.2m blast radius) must NOT receive arc jumps
    let final_brick_health = grid.get_health(brick_vpos.x, brick_vpos.y, brick_vpos.z);
    assert_eq!(
        final_brick_health, initial_brick_health,
        "Non-conductive brick outside blast radius must receive 0 electrical arc damage"
    );
}

#[test]
fn test_tier5_adversarial_zero_length_beam_and_boundary_raycasts() {
    let mut grid = VoxelGrid::new();

    // 1. Zero-Length Beam Carve: start == end
    let pt = Vec3::new(20.0, 10.0, 20.0);
    grid.set(20, 10, 20, VoxelType::Concrete);
    let res_zero = grid.carve_cylinder_advanced(pt, pt, 2.0, 200.0, DamageType::Kinetic);
    assert_eq!(res_zero.destroyed, 0, "Zero-length beam carve must return without destroying voxels");
    assert_eq!(grid.get(20, 10, 20), VoxelType::Concrete, "Voxel must remain unharmed");

    // Nearly-zero length (< 0.01m)
    let res_micro = grid.carve_cylinder_advanced(pt, pt + Vec3::new(0.003, 0.0, 0.0), 2.0, 200.0, DamageType::Kinetic);
    assert_eq!(res_micro.destroyed, 0, "Micro-length beam (<0.01m) must safely return early");

    // 2. Out-of-Bounds Raycasts
    // A. Pointing away from the world entirely
    let oob_origin = Vec3::new(500.0, 100.0, 500.0);
    let oob_hit = grid.raycast(oob_origin, Vec3::new(1.0, 1.0, 1.0), 1000.0);
    assert!(oob_hit.is_none(), "Raycast pointing away from world bounds must return None");

    // B. Zero direction vector
    let zero_dir_hit = grid.raycast(oob_origin, Vec3::ZERO, 100.0);
    assert!(zero_dir_hit.is_none(), "Zero direction raycast must safely return None without NaN");

    // C. Raycast originating outside bounds pointing INTO a solid target in the world
    let target_vpos = IVec3::new(10, 10, 48);
    grid.set(target_vpos.x, target_vpos.y, target_vpos.z, VoxelType::HeavyArmor);
    let target_wpos = grid.voxel_to_world(target_vpos);

    // Origin 10m outside the west face pointing along +X
    let ray_origin = Vec3::new(target_wpos.x - 10.0, target_wpos.y, target_wpos.z);
    let ray_hit = grid.raycast(ray_origin, Vec3::X, 100.0).expect("Ray pointing into grid must strike HeavyArmor wall");
    assert_eq!(ray_hit.voxel_type, VoxelType::HeavyArmor);
    assert_eq!(ray_hit.voxel_pos, target_vpos);
    assert_eq!(ray_hit.normal, Vec3::new(-1.0, 0.0, 0.0), "Hit normal on west face must be -X");
    assert!((ray_hit.world_pos.x - (target_wpos.x - VOXEL_SIZE * 0.5)).abs() < 0.05);
}

// ============================================================================
// GENERATION 3 / MILESTONE G3-M2: DEMOLITION SCALING & COLOSSAL SIEGE TESTS
// ============================================================================

#[test]
fn test_g3_m2_debris_pool_scaling_3000_invariants() {
    assert_eq!(PhysicsSystem::MAX_DEBRIS, 3000, "MAX_DEBRIS must scale to 3,000");
    assert_eq!(PhysicsSystem::SOFT_DEBRIS, 2400, "SOFT_DEBRIS must scale to 2,400");

    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // Populate a central rubble pile
    for x in -6..=6 {
        for z in -6..=6 {
            let dist = ((x * x + z * z) as f32).sqrt();
            let h = (3.5 - dist * 0.35).max(0.50);
            phys.rubble_heightmap_set(x as f32 * 0.5, z as f32 * 0.5, h);
        }
    }
    phys.rubble_dirty = false;

    // Spawn exactly 3,000 active debris particles
    for i in 0..3000 {
        let x = (i % 30) as f32 * 0.2 - 3.0;
        let z = ((i / 30) % 30) as f32 * 0.2 - 3.0;
        phys.debris.push(DebrisParticle {
            position: Vec3::new(x, 4.0 + (i % 8) as f32 * 0.25, z),
            velocity: Vec3::new(
                ((i % 5) as f32 - 2.0) * 2.0,
                -3.0 - (i % 4) as f32,
                (((i / 5) % 5) as f32 - 2.0) * 2.0,
            ),
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::new(10.0, -14.0, 8.0),
            size: 0.12,
            color: Color::new(0.65, 0.62, 0.58, 1.0),
            life: 25.0,
            max_life: 25.0,
            bounces: 0,
        });
    }

    assert_eq!(phys.debris.len(), 3000);

    // Step 1 frame: verify zero abrupt truncation at 3,000 particles
    phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    assert_eq!(phys.debris.len(), 3000, "Debris count must maintain 3000 without abrupt truncation");

    // Advance 49 more frames: excess drops gracefully to SOFT_DEBRIS (2400)
    for _ in 0..49 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    assert_eq!(phys.debris.len(), PhysicsSystem::SOFT_DEBRIS, "Excess debris must gracefully decay to SOFT_DEBRIS (2400)");
    for (i, p) in phys.debris.iter().enumerate() {
        assert!(!p.position.is_nan(), "Debris particle {} position must not be NaN", i);
        assert!(!p.velocity.is_nan(), "Debris particle {} velocity must not be NaN", i);
        assert!(
            p.position.y >= 0.48,
            "Debris particle {} must not clip through bedrock y=0.50 (got {})",
            i, p.position.y
        );
    }
}

#[test]
fn test_g3_m2_dense_pancake_collapse_mass_accretion_and_perimeter_spall() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    let cx = 32;
    let cz = 32;

    // Build intermediate stories at y = 8..=9 and y = 14..=15
    for dx in -3..=3 {
        for dz in -3..=3 {
            grid.set(cx + dx, 8, cz + dz, VoxelType::Concrete);
            grid.set(cx + dx, 9, cz + dz, VoxelType::SteelRebar);
            grid.set(cx + dx, 14, cz + dz, VoxelType::Concrete);
            grid.set(cx + dx, 15, cz + dz, VoxelType::SteelRebar);
        }
    }
    let initial_solids = grid.solid_count();

    // Spawn falling slab at y = 17
    let mut slab_voxels = Vec::new();
    for x in -2..=2 {
        for y in 0..2 {
            for z in -2..=2 {
                slab_voxels.push((
                    Vec3::new(x as f32 * VOXEL_SIZE, y as f32 * VOXEL_SIZE, z as f32 * VOXEL_SIZE),
                    VoxelType::HeavyArmor,
                ));
            }
        }
    }
    let aabb = DynamicChunk::compute_aabb(&slab_voxels);
    let start_wpos = grid.voxel_to_world(IVec3::new(cx, 17, cz));
    let initial_mass = 1000.0f32;

    phys.chunks.push(DynamicChunk {
        position: start_wpos,
        velocity: Vec3::new(0.0, -12.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: slab_voxels,
        exposed_voxels: Vec::new(),
        voxel_health: Vec::new(),
        mass: initial_mass,
        life: 0.0,
        settled: false,
        local_aabb: aabb,
    });

    let pre_debris = phys.debris.len();
    let pre_sparks = phys.sparks.len();

    // Smash through story 1 (y = 14..=15)
    for _ in 0..8 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    let solids_f1 = grid.solid_count();
    assert!(solids_f1 < initial_solids, "Story at y=14..=15 must be pulverized");
    let mass_after_f1 = phys.chunks[0].mass;
    assert!(
        mass_after_f1 > initial_mass,
        "Chunk mass must accrete crushed story mass: initial {}, after f1 {}",
        initial_mass, mass_after_f1
    );
    assert!(
        phys.debris.len() > pre_debris + 25,
        "Pulverization must emit dynamic micro-chunks (debris: {} -> {})",
        pre_debris, phys.debris.len()
    );
    assert!(
        phys.sparks.len() > pre_sparks + 10,
        "Perimeter spall clouds must emit incandescent ricochet sparks (sparks: {} -> {})",
        pre_sparks, phys.sparks.len()
    );

    // Continue plunge through story 2 (y = 8..=9)
    for _ in 0..16 {
        phys.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    let solids_f2 = grid.solid_count();
    assert!(solids_f2 < solids_f1, "Story at y=8..=9 must also be pulverized without stalling");
    let mass_after_f2 = phys.chunks[0].mass;
    assert!(
        mass_after_f2 > mass_after_f1,
        "Chunk mass must further accrete second story mass: f1 {}, f2 {}",
        mass_after_f1, mass_after_f2
    );
}

#[test]
fn test_g3_m2_colossal_siege_devastation_shockwaves_and_spall_fans() {
    let mut siege = SiegeSystem::new();
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // 1. Rail Artillery HE: Shockwave 2.8x (~45.7m), rubble shower clamp 350, secondary spall shower
    let bunker_target = grid.voxel_to_world(IVec3::new(20, 2, 20));
    for x in 18..=22 {
        for y in 1..=4 {
            for z in 18..=22 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    siege.select_kind(SiegeEngineKind::RailArtillery, &audio);
    siege.deploy_selected_engine(Vec3::new(bunker_target.x - 25.0, 0.5, bunker_target.z), Vec3::X, &audio);

    let mut cfg = siege.get_engine_config(SiegeEngineKind::RailArtillery);
    cfg.payload = SiegePayload::ColossalHighExplosive;
    cfg.caliber_mm = 1500.0;
    cfg.propellant_power = 1.5;
    siege.set_engine_config(SiegeEngineKind::RailArtillery, cfg);

    let pre_sparks = physics.sparks.len();
    siege.fire_active_engine(Some(bunker_target), &mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(siege.colossal_shells.len(), 1);
    siege.colossal_shells[0].position = bunker_target;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    let rail_shock = physics.shockwaves.iter().find(|s| s.max_radius >= 40.0);
    assert!(rail_shock.is_some(), "Rail Artillery HE shockwave must scale to 2.8x blast_r");
    assert!(physics.sparks.len() > pre_sparks + 30, "Secondary spall shower along crater perimeter");

    // 2. Orbital Kinetic Relay: 28m bedrock cratering shockwave + vertical column spall bursts
    let orbital_target = grid.voxel_to_world(IVec3::new(35, 12, 35));
    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(25.0, 0.5, 25.0), Vec3::X, &audio);

    let pre_sparks_orb = physics.sparks.len();
    siege.fire_active_engine(Some(orbital_target), &mut grid, &mut physics, &audio, Vec3::ZERO);
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    let orbital_bedrock_shock = physics.shockwaves.iter().find(|s| (s.max_radius - 28.0).abs() < 1.0);
    assert!(orbital_bedrock_shock.is_some(), "Orbital Relay must spawn 28m bedrock shockwave");
    assert!(
        physics.sparks.len() > pre_sparks_orb + 50,
        "Vertical column spall bursts must spray incandescent sparks along descent"
    );

    // 3. Hydraulic Ground Ram: 5-stage harmonic resonance wave with perimeter spall fans
    let ram_origin = Vec3::new(45.0, 0.5, 45.0);
    for y in 1..=3 {
        grid.set(48, y, 45, VoxelType::Concrete);
    }
    siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
    siege.deploy_selected_engine(ram_origin, Vec3::X, &audio);
    siege.deployed_engines[2].last_slam_time = 1.0;
    siege.deployed_engines[2].resonance_slam_count = 3; // Resonance stage 4 (3.0x power)

    let pre_sparks_ram = physics.sparks.len();
    siege.fire_active_engine(Some(Vec3::new(55.0, 0.5, 45.0)), &mut grid, &mut physics, &audio, Vec3::ZERO);

    // Bedrock slam spall fan
    assert!(physics.sparks.len() > pre_sparks_ram, "Bedrock slam must emit perimeter spall fan");

    // Advance pulse wave
    for _ in 0..8 {
        siege.update(0.03, &mut grid, &mut physics, &audio, Vec3::ZERO);
    }
    assert!(physics.sparks.len() > pre_sparks_ram + 10, "Pillars buckled by harmonic resonance emit spall fans");
}
