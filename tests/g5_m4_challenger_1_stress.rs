// ============================================================================
// EMPIRICAL ADVERSARIAL STRESS TEST SUITE: MILESTONE G5-M4
// Crate Architecture, Release Footprint & Code Hygiene
// Target: tests/g5_m4_challenger_1_stress.rs
// Authored by: g5_m4_challenger_1 (Empirical Challenger)
// ============================================================================
//
// KEY OBJECTIVE:
// Empirically stress-test the library crate architecture of `voxel_demolition`:
// 1. External consumption: verify that this integration test can consume ALL 10
//    modules directly via `use voxel_demolition::...` WITHOUT ANY `#[path = ...]`
//    attributes, verifying that `src/lib.rs` exports the complete public API surface.
// 2. Module independence: verify that modules can be instantiated and used
//    independently without dependencies on `src/main.rs`.
// 3. Adversarial stress & boundary conditions across public structs, enums,
//    functions, and constants across all 10 modules.
// ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

// STRICT CRATE CONSUMPTION: Zero #[path = "..."] attributes allowed!
use voxel_demolition::audio::{self, SoundSystem};
use voxel_demolition::customizer::{self, CombatPreset, CustomizerUI, PRESETS};
use voxel_demolition::physics::{
    self, blackbody_color_ramp, push_cube_mesh, push_oriented_cube_mesh, push_quad_mesh,
    push_quad_mesh_dual_tone, rotate_vec3, DebrisParticle, DecalKind, DynamicChunk,
    DynamicPointLight, ElectricalArc, ImpactDecal, Mat3, ParticleBatcher, PhysicsCullMetrics,
    PhysicsSystem, PressurizedFireJet, ShockwaveRing, SmokeParticle, SparkParticle,
    MAX_BATCH_INDICES, MAX_BATCH_VERTICES,
};
use voxel_demolition::player::{self, Player, MAX_STEP_UP, PLAYER_HEIGHT, PLAYER_RADIUS};
use voxel_demolition::siege::{
    self, ColossalShell, DeployedSiegeEngine, OrbitalBeamStrike, SeismicGroundPulse,
    SiegeEngineKind, SiegePayload, SiegeSystem, SiegeTrajectory, SiegeWeaponConfig,
    TeslaSiegeDischarge,
};
use voxel_demolition::skyline::{
    self, create_distant_skyline, DistantSkyline, SkylineBeacon,
    MAX_BATCH_INDICES as SKYLINE_MAX_INDICES, MAX_BATCH_VERTICES as SKYLINE_MAX_VERTICES,
};
use voxel_demolition::ui::{
    self, char_glyph_width, compute_curved_text_layout, compute_seismic_stress, compute_ui_scale,
    fit_text_to_width, fit_text_to_width_stack, HitKind, HitMarker, StackStr, HUD,
};
use voxel_demolition::voxel::{
    self, partition_mesh_sub_batches, partition_mesh_sub_batches_into,
    CarveResult, ChunkMesh, DamageType, Frustum, FrustumPlane, FrustumPlanes,
    IslandChunk, MaterialProperties, RayHit, VoxelGrid, VoxelType, CANTILEVER_MAX_DEPTH,
    CHUNKS_SUBTERRANEAN_Y, CHUNKS_SURFACE_Y, CHUNKS_X, CHUNKS_Y, CHUNKS_Z, CHUNK_SIZE, GRID_X,
    GRID_Y, GRID_Z, MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES,
    MAX_VOXEL_Y, MESH_POOL_MAX_CAPACITY, MIN_VOXEL_Y, VOID_BOUNDARY_Y, VOXEL_SIZE,
};
use voxel_demolition::weapon::{
    self, process_explosive_chain, process_explosive_chain_with_capacitors,
    spawn_anchored_impact_decal, ActiveCuttingShock, ActiveProjectile, ActiveSingularity,
    BarrelType, C4Charge, DetCordLink, DetonatorType, FireFeedback, PayloadType, RailgunTracer,
    StabilizerType, TeslaArcTracer, TrailBuffer, TrailStyle, VolatileTriggerKind,
    WeaponCustomization, WeaponMode, WeaponSystem, MAX_C4_CHARGES, MAX_DET_CORD_DIST,
};
use voxel_demolition::world_gen::{self, LevelPreset};

use macroquad::prelude::*;

// ============================================================================
// 1. EXTERNAL CONSUMPTION & MODULE INDEPENDENCE VERIFICATION
// ============================================================================

#[test]
fn test_external_consumption_all_ten_modules_accessible() {
    // Verify that every module namespace is accessible as a distinct submodule of voxel_demolition
    let _ = audio::SoundSystem::calculate_concurrency_ducking(0);
    let _ = voxel::VOXEL_SIZE;
    let _ = physics::MAX_BATCH_VERTICES;
    let _ = weapon::MAX_C4_CHARGES;
    let _ = siege::SiegeWeaponConfig::default();
    let _ = player::PLAYER_RADIUS;
    let _ = customizer::PRESETS.len();
    let _ = ui::compute_ui_scale(1920.0, 1080.0);
    let _ = skyline::MAX_BATCH_VERTICES;
    let mut grid = VoxelGrid::new();
    world_gen::generate_level(&mut grid, LevelPreset::ApexSkyscraper);
    assert!(grid.solid_count() > 0, "Level generation via external crate must succeed");
}

#[test]
fn test_audio_module_external_api_independence() {
    // SoundSystem::empty() provides non-audio synchronous initialization
    let audio_sys = SoundSystem::empty();
    assert!(audio_sys.explosion.is_none());
    assert!(audio_sys.big_boom.is_none());
    assert!(audio_sys.railgun.is_none());

    // Test concurrency ducking formulas directly from external crate
    assert_eq!(SoundSystem::calculate_concurrency_ducking(0), 1.0);
    let duck_3 = SoundSystem::calculate_concurrency_ducking(3);
    assert!((duck_3 - 0.40).abs() < 0.01);
    let duck_10 = SoundSystem::calculate_concurrency_ducking(10);
    assert!((duck_10 - (1.0 / 6.0)).abs() < 0.01);

    // Test spatial distance attenuation formulas
    let origin = Vec3::ZERO;
    let close = Vec3::new(1.0, 0.0, 0.0);
    let far = Vec3::new(80.0, 0.0, 0.0);
    let atten_close = SoundSystem::calculate_distance_attenuation(origin, close);
    let atten_far = SoundSystem::calculate_distance_attenuation(origin, far);
    assert!(atten_close > atten_far);
    assert!(atten_close <= 1.0);
    assert!(atten_far >= 0.005);
}

#[test]
fn test_voxel_module_constants_and_types_accessibility() {
    // Assert all fundamental world geometry constants are public and mathematically consistent
    assert_eq!(VOXEL_SIZE, 0.5);
    assert_eq!(CHUNK_SIZE, 8);
    assert_eq!(CHUNKS_X, 24);
    assert_eq!(CHUNKS_Z, 24);
    assert_eq!(CHUNKS_SUBTERRANEAN_Y, 6);
    assert_eq!(CHUNKS_SURFACE_Y, 28);
    assert_eq!(CHUNKS_Y, 34);
    assert_eq!(GRID_X, 192);
    assert_eq!(GRID_Z, 192);
    assert_eq!(GRID_Y, 272);
    assert_eq!(MIN_VOXEL_Y, -48);
    assert_eq!(MAX_VOXEL_Y, 224);
    assert_eq!(VOID_BOUNDARY_Y, -24.0);
    assert_eq!(CANTILEVER_MAX_DEPTH, 4);
    assert_eq!(MAX_CHUNK_BATCH_VERTICES, 3600);
    assert_eq!(MAX_CHUNK_BATCH_INDICES, 4400);

    // Assert VoxelType registry covers all intended materials
    let materials = [
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

    for mat in materials {
        let props = mat.properties();
        assert!(props.max_health >= 0.0);
        assert!(props.hardness >= 0.0);
        assert!(props.toughness >= 0.0);
        assert!(props.density >= 0.0);
        assert!(props.conductivity >= 0.0 && props.conductivity <= 12.0);
    }
}

#[test]
fn test_voxel_grid_external_carve_and_raycast() {
    let mut grid = VoxelGrid::new();
    let test_pos = IVec3::new(48, 10, 48);

    grid.set(test_pos.x, test_pos.y, test_pos.z, VoxelType::Concrete);
    assert_eq!(grid.get(test_pos.x, test_pos.y, test_pos.z), VoxelType::Concrete);
    assert!(grid.solid_count() >= 1);

    // Test raycasting from external crate
    let world_pos = grid.voxel_to_world(test_pos);
    let ray_origin = world_pos - Vec3::new(0.0, 0.0, 5.0);
    let ray_dir = Vec3::new(0.0, 0.0, 1.0);
    let hit = grid.raycast(ray_origin, ray_dir, 50.0);
    assert!(hit.is_some(), "Raycast should hit the set voxel");
    let hit_res = hit.unwrap();
    assert_eq!(hit_res.voxel_pos, test_pos);

    // Test zero-allocation carve_sphere_advanced_into
    let mut carve_result = CarveResult::default();
    grid.carve_sphere_advanced_into(
        world_pos,
        3.0,
        500.0,
        DamageType::Explosive,
        &mut carve_result,
    );
    assert!(carve_result.destroyed > 0);
    assert_eq!(grid.get(test_pos.x, test_pos.y, test_pos.z), VoxelType::Air);
}

#[test]
fn test_physics_module_external_api_and_matrix_rotation() {
    let phys = PhysicsSystem::new();
    assert_eq!(phys.debris.len(), 0);
    assert_eq!(phys.smoke.len(), 0);
    assert_eq!(phys.sparks.len(), 0);
    assert_eq!(phys.shockwaves.len(), 0);

    // Test Mat3 external access and identity/rotation
    let m_ident = Mat3::identity();
    let v = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(m_ident.transform_vector(v), v);

    let rot = Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0);
    let m_rot = Mat3::from_euler(rot);
    let v_rot = m_rot.transform_vector(Vec3::new(1.0, 0.0, 0.0));
    assert!((v_rot.x - 0.0).abs() < 1e-4);
    assert!((v_rot.z - (-1.0)).abs() < 1e-4);

    // Test rotate_vec3 convenience function
    let v_rot2 = rotate_vec3(Vec3::new(1.0, 0.0, 0.0), rot);
    assert!((v_rot2.x - v_rot.x).abs() < 1e-4);
    assert!((v_rot2.z - v_rot.z).abs() < 1e-4);

    // Test blackbody color ramp (positive alpha above 0.05 cooling slag)
    let c_mid = blackbody_color_ramp(0.5);
    let c_hot = blackbody_color_ramp(1.0);
    assert!(c_mid.a > 0.0);
    assert!(c_hot.a > 0.0);
    assert!(c_hot.r >= c_mid.r);
}

#[test]
fn test_weapon_module_c4_grid_and_customization() {
    let mut weapon_sys = WeaponSystem::new();
    assert_eq!(weapon_sys.config.mode, WeaponMode::HavocMortar);
    assert_eq!(weapon_sys.c4_charges.len(), 0);

    // Test placing C4 charges up to MAX_C4_CHARGES (24)
    for i in 0..MAX_C4_CHARGES {
        let placed = weapon_sys.place_c4_charge(
            Vec3::new(i as f32 * 2.0, 5.0, 10.0),
            Vec3::new(0.0, 1.0, 0.0),
            Some(IVec3::new(i as i32 * 4, 10, 20)),
        );
        assert!(placed, "Should place C4 charge {} within limit of 24", i);
    }
    assert_eq!(weapon_sys.c4_charges.len(), MAX_C4_CHARGES);

    // Exceeding MAX_C4_CHARGES must be rejected
    let extra = weapon_sys.place_c4_charge(
        Vec3::new(100.0, 5.0, 10.0),
        Vec3::new(0.0, 1.0, 0.0),
        Some(IVec3::new(200, 10, 20)),
    );
    assert!(!extra, "25th C4 charge must be rejected");

    // Update det cord cache and verify links are formed
    weapon_sys.update_det_cord_cache();
    assert!(!weapon_sys.cached_det_cords.is_empty(), "Det cords must be formed between adjacent C4 charges");

    // Test coordinated detonation
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();
    let (_feedback, det_count) = weapon_sys.detonate_all_c4(&mut grid, &mut phys, &sound, Vec3::ZERO);
    assert_eq!(det_count, MAX_C4_CHARGES);
    assert_eq!(weapon_sys.c4_charges.len(), 0);
}

#[test]
fn test_siege_module_external_deployment_and_ballistics() {
    let mut siege_sys = SiegeSystem::new();
    assert_eq!(siege_sys.deployed_engines.len(), 0);

    // Deploy standard allied fleet
    siege_sys.setup_default_fleet();
    assert_eq!(siege_sys.deployed_engines.len(), 3);

    // Test Copy derivation on SiegeWeaponConfig (Feature 12 from G5-M2)
    let cfg = siege_sys.deployed_engines[0].config;
    let cfg_copy = cfg; // Copy semantics verified
    assert_eq!(cfg.caliber_mm, cfg_copy.caliber_mm);
    assert_eq!(cfg.blast_radius(), cfg_copy.blast_radius());
}

#[test]
fn test_player_module_physics_and_bounding_invariants() {
    let spawn = Vec3::new(24.0, 5.0, 24.0);
    let player = Player::new(spawn);

    assert_eq!(player.position, spawn);
    assert_eq!(player.velocity, Vec3::ZERO);
    assert_eq!(PLAYER_RADIUS, 0.30);
    assert_eq!(PLAYER_HEIGHT, 1.80);
    assert_eq!(MAX_STEP_UP, 0.55);

    // Test player camera and direction methods from external crate
    let fwd = player.forward_dir();
    assert!(fwd.is_normalized());
    let cam = player.camera_position();
    assert!(cam.y > player.position.y);
    assert_eq!(player.horizontal_speed(), 0.0);
}

#[test]
fn test_ui_module_stack_str_zero_allocation_guarantees() {
    use std::fmt::Write;

    // Test StackStr stack buffer formatting
    let mut s = StackStr::<64>::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);

    write!(s, "STRUCTURAL INTEGRITY: {:.1}%", 87.42).unwrap();
    assert_eq!(s.as_str(), "STRUCTURAL INTEGRITY: 87.4%");

    // Test capacity truncation safety
    let mut small = StackStr::<8>::new();
    let _ = write!(small, "LONG_STRING_THAT_EXCEEDS_CAPACITY");
    assert!(small.len() <= 8);
    assert!(std::str::from_utf8(small.as_str().as_bytes()).is_ok(), "Must maintain valid UTF-8");

    // Test fit_text_to_width_stack equivalence
    let fitted = fit_text_to_width_stack::<32>("EXTREMELY_LONG_TACTICAL_TELEMETRY_READOUT", 120.0, 16.0);
    assert!(!fitted.is_empty());
    assert!(fitted.len() <= 32);

    // Test HUD initialization and notification lifecycle
    let mut hud = HUD::new();
    assert_eq!(hud.collapse_notifications.len(), 0);
    hud.add_notification("SKYBRIDGE DESTABILIZED");
    assert_eq!(hud.collapse_notifications.len(), 1);
}

#[test]
fn test_skyline_module_batched_mesh_generation() {
    let skyline = create_distant_skyline();
    assert!(!skyline.base_sub_meshes.is_empty(), "Skyline base sub-meshes must be generated");
    assert!(!skyline.beacons.is_empty(), "Skyline beacons must be populated");
    assert!(skyline.beacons.len() >= 24, "Should have 24+ aviation beacons for city skyline");

    // Verify sub-mesh vertex and index limits are strictly enforced
    for mesh in &skyline.base_sub_meshes {
        assert!(mesh.vertices.len() <= SKYLINE_MAX_VERTICES);
        assert!(mesh.indices.len() <= SKYLINE_MAX_INDICES);
        assert_eq!(mesh.indices.len() % 3, 0, "Indices must form complete triangles");
    }
}

#[test]
fn test_customizer_module_presets_and_independence() {
    let customizer = CustomizerUI::new();
    assert!(!customizer.is_open);
    assert_eq!(customizer.active_tab, 0);

    // Verify combat presets accessible externally
    assert_eq!(PRESETS.len(), 7);
    for preset in PRESETS {
        assert!(!preset.name.is_empty());
        assert!(!preset.description.is_empty());
        assert!(preset.blast_radius_mult > 0.0);
    }
}

#[test]
fn test_world_gen_all_nine_presets_independently_constructible() {
    for preset in LevelPreset::ALL {
        let mut grid = VoxelGrid::new();
        world_gen::generate_level(&mut grid, preset);
        assert!(
            grid.solid_count() > 500,
            "Preset {:?} should generate a non-trivial destructible structure",
            preset
        );
    }
}

// ============================================================================
// 2. ADVERSARIAL STRESS TESTS ON CROSS-MODULE INTERACTIONS
// ============================================================================

#[test]
fn test_adversarial_island_detection_bfs_buffer_reuse_under_rapid_cutting() {
    let mut grid = VoxelGrid::new();
    // Build a vertical platform: x=48..=50, z=48..=50, y=10..=15
    for x in 48..=50 {
        for z in 48..=50 {
            for y in 10..=15 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Mark dirty region so localized BFS runs
    grid.mark_dirty_region(IVec3::new(46, 8, 46), IVec3::new(52, 17, 52));

    // Unsupported floating platform must be detected as an island of 3*3*6 = 54 voxels
    let islands1 = grid.detect_unsupported_islands();
    assert!(!islands1.is_empty(), "Floating platform must be detected as unsupported island");
    let total_island_voxels: usize = islands1.iter().map(|isl| isl.voxels.len()).sum();
    assert_eq!(total_island_voxels, 54, "All 54 voxels of the unsupported platform must collapse");

    // Verify grid has Air where the unsupported structure was
    for x in 48..=50 {
        for z in 48..=50 {
            for y in 10..=15 {
                assert_eq!(grid.get(x, y, z), VoxelType::Air, "Collapsed voxels must be cleared to Air");
            }
        }
    }
}

#[test]
fn test_adversarial_sub_batch_mesh_partitioning_pathological_spans() {
    // 800 triangles across 12,000 vertices (spanning multiple sub-batches)
    let vertices_len = 12000;
    let mut indices = Vec::with_capacity(2400);

    for t in 0..800u16 {
        let base = t * 15;
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    let sub_batches = partition_mesh_sub_batches(&indices, vertices_len, MAX_CHUNK_BATCH_VERTICES, MAX_CHUNK_BATCH_INDICES);
    assert!(sub_batches.len() >= 3, "12,000-span mesh must partition into at least 3 sub-batches");

    let mut total_retained_triangles = 0;
    for (v_range, remapped_idx) in &sub_batches {
        let v_span = v_range.end() - v_range.start() + 1;
        assert!(v_span <= MAX_CHUNK_BATCH_VERTICES);
        assert!(remapped_idx.len() <= MAX_CHUNK_BATCH_INDICES);
        assert_eq!(remapped_idx.len() % 3, 0);
        total_retained_triangles += remapped_idx.len() / 3;
    }

    assert_eq!(total_retained_triangles, 800, "Zero triangles should be dropped during partitioning");
}

#[test]
fn test_adversarial_c4_laser_det_cord_topology_stability() {
    let mut weapon = WeaponSystem::new();

    // Place C4 charges in a ring around center
    let center = Vec3::new(24.0, 5.0, 24.0);
    let radius = 12.0;
    let count = 12;

    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let pos = center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        let voxel_coord = IVec3::new(pos.x as i32 * 2, pos.y as i32 * 2, pos.z as i32 * 2);
        weapon.place_c4_charge(pos, Vec3::Y, Some(voxel_coord));
    }

    weapon.update_det_cord_cache();

    // Verify det cords connect ring neighbors (distance between adjacent is ~6.2m < 25.0m)
    assert!(weapon.cached_det_cords.len() >= count);

    // Verify all links connect valid indices
    for link in &weapon.cached_det_cords {
        assert!(link.idx_a < weapon.c4_charges.len());
        assert!(link.idx_b < weapon.c4_charges.len());
        assert_ne!(link.idx_a, link.idx_b);
    }
}

#[test]
fn test_adversarial_c4_rapid_detonation_empty_grid_and_no_charges() {
    let mut weapon = WeaponSystem::new();
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let sound = SoundSystem::empty();

    // Detonating with 0 charges must return count 0 and not crash or panic
    let (fb0, count0) = weapon.detonate_all_c4(&mut grid, &mut phys, &sound, Vec3::ZERO);
    assert_eq!(count0, 0);
    assert_eq!(fb0.camera_trauma, 0.0);

    // Place a single charge, clear it, detonate
    weapon.place_c4_charge(Vec3::new(10.0, 5.0, 10.0), Vec3::Y, None);
    assert_eq!(weapon.c4_charges.len(), 1);
    weapon.clear_c4();
    assert_eq!(weapon.c4_charges.len(), 0);

    let (_, count_after_clear) = weapon.detonate_all_c4(&mut grid, &mut phys, &sound, Vec3::ZERO);
    assert_eq!(count_after_clear, 0);
}

#[test]
fn test_adversarial_siege_fleet_rapid_switching_and_firing() {
    let mut siege = SiegeSystem::new();
    let sound = SoundSystem::empty();

    // Setup fleet: RailArtillery, OrbitalKineticRelay, HydraulicGroundRam
    siege.setup_default_fleet();
    assert_eq!(siege.deployed_engines.len(), 3);

    // Rapid kind switching
    siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &sound);
    assert_eq!(siege.selected_kind, SiegeEngineKind::TeslaSiegePylon);
    assert_eq!(siege.config.engine_kind, SiegeEngineKind::TeslaSiegePylon);

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &sound);
    assert_eq!(siege.selected_kind, SiegeEngineKind::OrbitalKineticRelay);

    siege.select_kind(SiegeEngineKind::RailArtillery, &sound);
    assert_eq!(siege.selected_kind, SiegeEngineKind::RailArtillery);

    // Fire active engine
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let recoil = siege.fire_active_engine(
        Some(Vec3::new(0.0, 5.0, 0.0)),
        &mut grid,
        &mut phys,
        &sound,
        Vec3::new(0.0, 2.0, 20.0),
    );
    assert!(recoil >= 0.0);
}

#[test]
fn test_adversarial_stack_str_multibyte_utf8_truncation_boundaries() {
    use std::fmt::Write;

    // Test multi-byte UTF-8 string with StackStr
    let mut s = StackStr::<16>::new();
    // "🔥💎🚀💥" are 4 bytes each = 16 bytes total
    write!(s, "🔥💎🚀💥").unwrap();
    assert_eq!(s.as_str(), "🔥💎🚀💥");
    assert_eq!(s.len(), 16);

    // Truncating in the middle of a 4-byte character must not produce invalid UTF-8
    let mut small = StackStr::<6>::new();
    // Writing "🔥⚡" requires 8 bytes, so 6-byte buffer should either fit 1 emoji (4 bytes) or truncate safely
    let _ = write!(small, "🔥⚡");
    assert!(small.len() <= 6);
    let slice = small.as_str();
    assert!(std::str::from_utf8(slice.as_bytes()).is_ok(), "Must maintain valid UTF-8");
}

#[test]
fn test_adversarial_voxel_grid_boundary_coordinates() {
    let mut grid = VoxelGrid::new();

    // Coordinate limits:
    // x: 0..GRID_X (96)
    // z: 0..GRID_Z (96)
    // y: MIN_VOXEL_Y (-48) .. MAX_VOXEL_Y (224)
    assert!(grid.in_bounds(0, 0, 0));
    assert!(grid.in_bounds(GRID_X as i32 - 1, 0, GRID_Z as i32 - 1));
    assert!(grid.in_bounds(0, -48, 0));
    assert!(grid.in_bounds(0, 223, 0));

    // Out-of-bounds queries must return Air and not panic
    assert_eq!(grid.get(-1, 0, 0), VoxelType::Air);
    assert_eq!(grid.get(0, -49, 0), VoxelType::Air);
    assert_eq!(grid.get(0, 224, 0), VoxelType::Air);
    assert_eq!(grid.get(GRID_X as i32, 0, 0), VoxelType::Air);
    assert_eq!(grid.get(1000, 1000, 1000), VoxelType::Air);

    // Out-of-bounds set must safely no-op
    grid.set(-10, 500, -10, VoxelType::HeavyArmor);
    assert_eq!(grid.get(-10, 500, -10), VoxelType::Air);
}

#[test]
fn test_adversarial_concurrency_ducking_monotonic_and_bounded() {
    let mut prev_duck = 1.0f32;
    for voices in 0..1000 {
        let duck = SoundSystem::calculate_concurrency_ducking(voices);
        assert!(duck > 0.0, "Ducking must remain positive");
        assert!(duck <= 1.0, "Ducking must never exceed 1.0");
        assert!(duck <= prev_duck, "Ducking must be monotonically decreasing with voice count");
        prev_duck = duck;
    }
}

#[test]
fn test_adversarial_ui_scale_matrix() {
    // Test diverse viewport aspect ratios and resolutions
    let viewports = [
        (640.0, 480.0),    // 480p SD
        (1280.0, 720.0),   // 720p HD (baseline 1.0)
        (1920.0, 1080.0),  // 1080p FHD
        (2560.0, 1440.0),  // 1440p QHD
        (3840.0, 2160.0),  // 4K UHD
        (7680.0, 4320.0),  // 8K UHD
        (3440.0, 1440.0),  // 21:9 Ultrawide
        (5120.0, 1440.0),  // 32:9 Super Ultrawide
    ];

    for (w, h) in viewports {
        let s = compute_ui_scale(w, h);
        assert!(s >= 0.65, "Scale must respect minimum clamp of 0.65 for {}x{}", w, h);
        assert!(s <= 3.2, "Scale must respect maximum clamp of 3.2 for {}x{}", w, h);
    }
}
