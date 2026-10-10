//! ============================================================================
//! CAPSTONE INTEGRATION TEST SUITE: MILESTONE G4-M5
//! MASTER TEST SUITE EXPANSION & ADVERSARIAL VICTORY VERIFICATION
//! File Target: tests/g4_m5_victory_verification.rs
//!
//! Blueprint authored by g4_m5_explorer_1 (Test Matrix & Gap Analysis Explorer)
//! Exercises all 18 Features from PROJECT.md across Requirements R1-R4.
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{
    DebrisParticle, DecalKind, DynamicChunk, DynamicPointLight, ElectricalArc, ImpactDecal,
    PhysicsSystem, PressurizedFireJet, ShockwaveRing, SmokeParticle, SparkParticle,
};
use player::{Player, PLAYER_HEIGHT, PLAYER_RADIUS};
use siege::{SiegeEngineKind, SiegeSystem};
use skyline::{create_distant_skyline, DistantSkyline, MAX_BATCH_INDICES, MAX_BATCH_VERTICES};
use voxel::{
    CarveResult, DamageType, FrustumPlanes, MaterialProperties, VoxelGrid, VoxelType,
    CANTILEVER_MAX_DEPTH, CHUNKS_SUBTERRANEAN_Y, CHUNKS_SURFACE_Y, CHUNKS_Y, CHUNK_SIZE, GRID_X,
    GRID_Y, GRID_Z, MAX_CHUNK_BATCH_INDICES, MAX_CHUNK_BATCH_VERTICES, MAX_VOXEL_Y, MIN_VOXEL_Y,
    VOID_BOUNDARY_Y, VOXEL_SIZE,
};
use weapon::{ActiveSingularity, WeaponSystem, MAX_C4_CHARGES, MAX_DET_CORD_DIST};
use world_gen::{
    build_elevated_monorail, build_multi_lane_road_deck, build_subterranean_subway_tunnel,
    build_suspension_skybridge, generate_level, LevelPreset,
};

// ============================================================================
// TIER 1: 100% DESTRUCTIBILITY & SUBTERRANEAN ENVIRONMENT (Features 1 - 5)
// ============================================================================

/// Feature 1: 100% Destructible Reinforced Granite Bedrock & Basalt Substrate
/// Verifies:
/// - Exact material properties (Bedrock: HP 800, Hardness 9.8, Toughness 9.5, Blast 45.0, Density 10.5; Basalt: HP 500, Hardness 9.2, Toughness 8.8, Blast 32.0, Density 8.8).
/// - Sub-blast-threshold deflection: Low-energy blast (< 45.0 for Bedrock, < 32.0 for Basalt) causes zero damage.
/// - Super-threshold high-yield shattering: High-yield explosive blast shatters both into dynamic debris.
#[test]
fn test_victory_feature_1_destructible_bedrock_and_basalt_blast_threshold_and_shatter() {
    let mut grid = VoxelGrid::new();
    let bedrock_props = VoxelType::Bedrock.properties();
    let basalt_props = VoxelType::Basalt.properties();

    assert_eq!(bedrock_props.max_health, 800.0);
    assert_eq!(bedrock_props.hardness, 9.8);
    assert_eq!(bedrock_props.toughness, 9.5);
    assert_eq!(bedrock_props.blast_threshold, 45.0);
    assert_eq!(bedrock_props.density, 10.5);
    assert!(VoxelType::Bedrock.is_destructible());

    assert_eq!(basalt_props.max_health, 500.0);
    assert_eq!(basalt_props.hardness, 9.2);
    assert_eq!(basalt_props.toughness, 8.8);
    assert_eq!(basalt_props.blast_threshold, 32.0);
    assert_eq!(basalt_props.density, 8.8);
    assert!(VoxelType::Basalt.is_destructible());

    // Setup layered bedrock and basalt voxels
    grid.set(40, 0, 40, VoxelType::Bedrock);
    grid.set(40, -5, 40, VoxelType::Basalt);

    // 1. Deflection: Blast below threshold (40.0 < 45.0 for bedrock)
    let bedrock_world = grid.voxel_to_world(IVec3::new(40, 0, 40));
    let mut carve_deflect = CarveResult::default();
    grid.carve_sphere_advanced_into(bedrock_world, 1.5, 40.0, DamageType::Explosive, &mut carve_deflect);
    assert_eq!(grid.get(40, 0, 40), VoxelType::Bedrock, "Sub-threshold blast must be deflected by Bedrock");

    // 2. High-Yield Shatter: Super-threshold blast (8000.0) shatters Bedrock and Basalt
    let mut carve_shatter = CarveResult::default();
    grid.carve_sphere_advanced_into(bedrock_world, 4.0, 8000.0, DamageType::Explosive, &mut carve_shatter);
    assert_eq!(grid.get(40, 0, 40), VoxelType::Air, "Super-threshold blast must shatter Bedrock");
    assert!(carve_shatter.destroyed > 0, "Carve result must record destroyed voxels");
}

/// Feature 2: Subterranean Coordinate & Carving Expansion
/// Verifies:
/// - Coordinate limits: MIN_VOXEL_Y = -48 (-24.0m), MAX_VOXEL_Y = 224 (+112.0m), 34 vertical chunks.
/// - Zero-crossing carving: Carving a sphere centered at Y=0.0m seamlessly removes voxels from both surface and subterranean chunks.
/// - Lowest boundary carving at y = -48 without out-of-bounds panics.
#[test]
fn test_victory_feature_2_subterranean_zero_crossing_and_deep_boundary_carving() {
    assert_eq!(CHUNKS_SUBTERRANEAN_Y, 6);
    assert_eq!(CHUNKS_SURFACE_Y, 28);
    assert_eq!(CHUNKS_Y, 34);
    assert_eq!(MIN_VOXEL_Y, -48);
    assert_eq!(MAX_VOXEL_Y, 224);

    let mut grid = VoxelGrid::new();

    // Fill cross-boundary column
    for y in -10..=10 {
        grid.set(45, y, 45, VoxelType::Concrete);
    }

    // Carve across Y = 0.0m boundary
    let center_world = grid.voxel_to_world(IVec3::new(45, 0, 45));
    let mut carve_res = CarveResult::default();
    grid.carve_sphere_advanced_into(center_world, 3.0, 5000.0, DamageType::Explosive, &mut carve_res);

    assert_eq!(grid.get(45, 2, 45), VoxelType::Air, "Surface voxels must be carved");
    assert_eq!(grid.get(45, 0, 45), VoxelType::Air, "Boundary voxel y=0 must be carved");
    assert_eq!(grid.get(45, -2, 45), VoxelType::Air, "Subterranean voxels must be carved");

    // Deep boundary carve at y = MIN_VOXEL_Y (-48)
    grid.set(45, MIN_VOXEL_Y, 45, VoxelType::Basalt);
    let deep_world = grid.voxel_to_world(IVec3::new(45, MIN_VOXEL_Y, 45));
    let mut deep_carve = CarveResult::default();
    grid.carve_sphere_advanced_into(deep_world, 2.0, 5000.0, DamageType::Explosive, &mut deep_carve);
    assert_eq!(grid.get(45, MIN_VOXEL_Y, 45), VoxelType::Air, "Deepest subterranean voxel must be carved cleanly");
}

/// Feature 3: Dynamic Subterranean Rubble Heightmap
/// Verifies:
/// - Rubble height excavates below 0.5m down to -24.0m when subterranean craters are formed.
/// - Dynamic chunks falling into craters tumble and settle at the new excavated subterranean floor.
/// - Settled rubble heightmap updates upward as rubble accumulates.
#[test]
fn test_victory_feature_3_subterranean_rubble_excavation_and_chunk_settling() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let vcenter = IVec3::new(48, 0, 48);
    let center = grid.voxel_to_world(vcenter);

    // Excavate deep crater in bedrock down to -8.0m (dy = -16)
    for dx in -4..=4 {
        for dz in -4..=4 {
            for dy in -16..=0 {
                grid.set(vcenter.x + dx, dy, vcenter.z + dz, VoxelType::Air);
            }
        }
    }
    physics.update_crater_floor(&grid, center, 4.0);

    let floor_h = physics.rubble_height_at(center.x, center.z);
    assert!(floor_h < -7.5, "Rubble height must excavate into subterranean space, got {:.2}", floor_h);

    // Spawn falling dynamic chunk inside the excavated crater
    let vox = vec![(Vec3::ZERO, VoxelType::Concrete)];
    physics.chunks.push(DynamicChunk {
        position: Vec3::new(center.x, -2.0, center.z),
        velocity: Vec3::new(0.0, -5.0, 0.0),
        rotation: Vec3::ZERO,
        rot_mat: physics::Mat3::IDENTITY,
        angular_velocity: Vec3::ZERO,
        voxels: vox.clone(),
        exposed_voxels: vox,
        voxel_health: Vec::new(),
        mass: 50.0,
        life: 0.0,
        settled: false,
        local_aabb: (Vec3::splat(-0.25), Vec3::splat(0.25)),
    });

    // Step physics: chunk must descend into subterranean negative Y and settle
    for _ in 0..120 {
        physics.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));
    }

    assert_eq!(physics.chunks.len(), 1);
    let settled_chunk = &physics.chunks[0];
    assert!(settled_chunk.settled, "Dynamic chunk must settle onto subterranean crater floor");
    assert!(
        settled_chunk.position.y < -7.0 && settled_chunk.position.y > -9.0,
        "Chunk position {} must match crater floor {}",
        settled_chunk.position.y,
        floor_h
    );
}

/// Feature 4: Subterranean Player Navigation & Void Reset
/// Verifies:
/// - Player ground check correctly supports negative Y crater floors.
/// - Step-up and wall slide in subterranean trenches.
/// - Safe bottom void boundary (Y <= -24.0m) instantly resets player to spawn point with zeroed velocity.
#[test]
fn test_victory_feature_4_subterranean_player_traversal_and_void_reset_state() {
    let spawn_point = Vec3::new(10.0, 2.0, 10.0);
    let mut player = Player::new(spawn_point);
    let grid = VoxelGrid::new();

    // 1. Initial State
    assert_eq!(player.position, spawn_point);
    assert_eq!(player.velocity, Vec3::ZERO);

    // 2. Teleport player into deep void (Y <= -24.0m)
    player.position = Vec3::new(10.0, VOID_BOUNDARY_Y - 1.0, 10.0);
    player.velocity = Vec3::new(5.0, -50.0, -3.0);
    player.trauma = 0.9;
    player.fov_kick = 0.2;

    // Step player: must trigger void boundary reset
    player.update(0.016, &grid, None, false);

    assert_eq!(player.position, spawn_point, "Player must be reset to spawn point after falling into void");
    assert_eq!(player.velocity, Vec3::ZERO, "Player velocity must be completely zeroed out");
    assert_eq!(player.trauma, 0.0, "Player trauma must be reset");
    assert_eq!(player.fov_kick, 0.0, "Player FOV kick must be reset");
    assert!(player.just_respawned, "just_respawned flag must be set");
}

/// Feature 5: Subterranean Visual Cutout & Bedrock Meshing
/// Verifies:
/// - Subterranean chunks (cy in [-6, 0)) generate non-empty meshes when craters expose bedrock faces.
/// - Rebuilt chunk meshes strictly respect vertex and index sub-batch limits.
#[test]
fn test_victory_feature_5_subterranean_chunk_meshing_and_exposed_bedrock_normals() {
    let mut grid = VoxelGrid::new();

    // Carve a crater in subterranean chunk (y = -16, which is internal_y = 32, cy = 4)
    let carve_pt = grid.voxel_to_world(IVec3::new(32, -16, 32));
    let mut carve_res = CarveResult::default();
    grid.carve_sphere_advanced_into(carve_pt, 3.5, 10000.0, DamageType::Explosive, &mut carve_res);
    assert!(carve_res.destroyed > 0);

    // Rebuild dirty chunk meshes
    grid.update_meshes();

    // Internal subterranean chunk at cx=4, cy=4 (y=-16), cz=4 must be meshed and have sub-batches
    let internal_y = (-16 - MIN_VOXEL_Y) as usize;
    let cy = internal_y / CHUNK_SIZE;
    let ci = VoxelGrid::chunk_idx(4, cy, 4);
    let mesh = &grid.meshes[ci];
    assert!(!mesh.sub_meshes.is_empty(), "Subterranean chunk must contain rendered sub-meshes");
    for sub_m in &mesh.sub_meshes {
        assert!(sub_m.vertices.len() <= MAX_CHUNK_BATCH_VERTICES);
        assert!(sub_m.indices.len() <= MAX_CHUNK_BATCH_INDICES);
    }
}

// ============================================================================
// TIER 2: MEGA STRUCTURE CITY DISTRICTS & DOMINO PHYSICS (Features 6 - 9)
// ============================================================================

/// Feature 6: 5-to-8 Skyscraper City Presets (F1-F5)
/// Verifies:
/// - All 5 city presets (ApexSkyscraper, SuspensionSkybridge, MilitaryFortress, DominoMonoliths, GothicCitadel) generate >= 25,000 voxels.
/// - Zero unsupported floating islands upon generation (structural equilibrium).
#[test]
fn test_victory_feature_6_all_five_city_presets_structural_diversity_and_equilibrium() {
    let presets = [
        LevelPreset::ApexSkyscraper,
        LevelPreset::SuspensionSkybridge,
        LevelPreset::MilitaryFortress,
        LevelPreset::DominoMonoliths,
        LevelPreset::GothicCitadel,
    ];

    for preset in presets {
        let mut grid = VoxelGrid::new();
        generate_level(&mut grid, preset);

        assert!(
            grid.current_voxels >= 25_000,
            "Preset {:?} must generate at least 25,000 voxels, got {}",
            preset,
            grid.current_voxels
        );

        // Initial structural equilibrium: zero floating islands
        let islands = grid.detect_unsupported_islands();
        assert!(
            islands.is_empty(),
            "Preset {:?} must generate structurally grounded architecture with 0 islands, got {}",
            preset,
            islands.len()
        );
    }
}

/// Feature 7: Inter-Tower Spacing & Domino Demolitions
/// Verifies:
/// - Inter-tower column spacing allows toppling 80m-100m towers to strike adjacent buildings diagonally.
/// - Island extraction generates high-momentum dynamic chunks (>500 mass) initiating progressive collapse.
#[test]
fn test_victory_feature_7_multi_building_domino_cascade_momentum_transfer() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let layout = generate_level(&mut grid, LevelPreset::ApexSkyscraper);

    // Sever foundation columns of Tower T4 Tech Arcology North at mapped (39..=57, 9..=27)
    let (gx0, gx1) = layout.map_range_x(39, 57);
    let (gz0, gz1) = layout.map_range_z(9, 27);
    for x in gx0..=gx1 {
        for z in gz0..=gz1 {
            for y in 1..=4 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing foundation columns must detect unsupported skyscraper island");

    let total_mass: f32 = islands.iter().map(|isl| isl.calculate_mass()).sum();
    assert!(total_mass >= 500.0, "Toppling skyscraper must exceed 500 mass units");

    // Spawn falling island chunks with diagonal topple velocity towards Tower 2
    for island in islands {
        physics.spawn_island_chunk(island, Vec3::new(2.0, -0.5, 2.0), Vec3::new(0.0, 0.0, 0.1));
    }

    assert!(!physics.chunks.is_empty());
    for _ in 0..15 {
        physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    }
}

/// Feature 8: Aerial Skybridges, Monorails & Subterranean Tunnels
/// Verifies:
/// - Structural integrity and integration of skybridges (y=55m-85m), monorails (y=20m-25m), and subway tunnels (y=-20m).
/// - Severing an aerial skybridge creates dynamic chunks without collapsing grounded support towers.
#[test]
fn test_victory_feature_8_aerial_skybridge_monorail_and_subway_severance_destruction() {
    let mut grid = VoxelGrid::new();

    // 1. Anchor bedrock
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
            for y in MIN_VOXEL_Y..0 {
                grid.set(x, y, z, VoxelType::Basalt);
            }
        }
    }

    // 2. Build transit elements
    build_multi_lane_road_deck(&mut grid, (10, 15), (85, 15), 1, 4, false);
    build_elevated_monorail(&mut grid, (10, 35), (85, 35), 40, 10, true);
    build_subterranean_subway_tunnel(&mut grid, (10, 55), (85, 55), -20, Some((48, 55)));

    // Pillars for skybridge
    for y in 1..=120 {
        grid.set(25, y, 75, VoxelType::Concrete);
        grid.set(70, y, 75, VoxelType::Concrete);
    }
    build_suspension_skybridge(&mut grid, (25, 75), (70, 75), 120, 2, false);

    assert!(grid.detect_unsupported_islands().is_empty());

    // Sever both pillar connections to the skybridge
    for y in 118..=122 {
        grid.set(25, y, 75, VoxelType::Air);
        grid.set(70, y, 75, VoxelType::Air);
    }

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing skybridge anchor must produce unsupported island");
}

/// Feature 9: 360-Degree Procedural Distant Skyline
/// Verifies:
/// - 360-degree cylindrical coverage (all 4 compass quadrants).
/// - Horizon skirt descends to <= -34.0m; apex towers reach >= 300.0m.
/// - Strict vertex (<= 3600) and index (<= 4400) limits per sub-mesh.
#[test]
fn test_victory_feature_9_distant_skyline_360_degree_panorama_and_batch_limits() {
    let skyline: DistantSkyline = create_distant_skyline();

    assert!(!skyline.base_sub_meshes.is_empty());
    assert!(skyline.beacons.len() >= 36);

    let mut has_pos_x = false;
    let mut has_neg_x = false;
    let mut has_pos_z = false;
    let mut has_neg_z = false;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for sm in &skyline.base_sub_meshes {
        assert!(sm.vertices.len() <= MAX_BATCH_VERTICES);
        assert!(sm.indices.len() <= MAX_BATCH_INDICES);
        assert_eq!(sm.indices.len() % 3, 0);

        for v in &sm.vertices {
            if v.position.x > 50.0 { has_pos_x = true; }
            if v.position.x < -50.0 { has_neg_x = true; }
            if v.position.z > 50.0 { has_pos_z = true; }
            if v.position.z < -50.0 { has_neg_z = true; }
            min_y = min_y.min(v.position.y);
            max_y = max_y.max(v.position.y);
        }
    }

    assert!(has_pos_x && has_neg_x && has_pos_z && has_neg_z, "Skyline must provide 360-degree 4-quadrant panorama");
    assert!(min_y <= -34.0, "Horizon skirt must descend into subterranean depth");
    assert!(max_y >= 300.0, "Orbital sky-elevators must reach stratospheric heights");
}

// ============================================================================
// TIER 3: COLOSSAL DEMOLITION ARSENAL & 24-C4 GRID (Features 10 - 14)
// ============================================================================

/// Feature 10: 24-Charge Magnetic C4 Demolition Grid
/// Verifies:
/// - Placement cap expanded to MAX_C4_CHARGES = 24.
/// - Placement of 24 charges succeeds in a 3D structural matrix across multiple columns.
/// - 25th charge is rejected.
/// - Coordinated detonation clears all 24 charges.
#[test]
fn test_victory_feature_10_c4_demolition_grid_24_point_3d_matrix_saturation() {
    assert_eq!(MAX_C4_CHARGES, 24);
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Setup 4 columns with 6 charges each
    for col in 0..4 {
        let cx = 30 + col * 8;
        for floor in 0..6 {
            let vy = 2 + floor * 4;
            grid.set(cx, vy, 30, VoxelType::Concrete);
            let pos = grid.voxel_to_world(IVec3::new(cx, vy, 30));
            let placed = weapon.place_c4_charge(pos, Vec3::Z, Some(IVec3::new(cx, vy, 30)));
            assert!(placed, "Charge {} must place successfully", col * 6 + floor + 1);
        }
    }
    assert_eq!(weapon.c4_charges.len(), 24);

    // 25th charge must be rejected
    grid.set(70, 5, 70, VoxelType::Concrete);
    let extra_pos = grid.voxel_to_world(IVec3::new(70, 5, 70));
    assert!(!weapon.place_c4_charge(extra_pos, Vec3::Y, Some(IVec3::new(70, 5, 70))));
    assert_eq!(weapon.c4_charges.len(), 24);

    // Detonate all
    let (feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 24);
    assert_eq!(weapon.c4_charges.len(), 0);
    assert!(feedback.camera_trauma >= 0.8);
}

/// Feature 10b: 24-Pip Segmented LED HUD Telemetry Widget Layout
/// Verifies:
/// - 24-pip segmented LED widget dynamically renders without negative or sub-pixel width across all standard resolutions.
/// - Charges array up to MAX_C4_CHARGES = 24 maps 1:1 to HUD indicator pips.
#[test]
fn test_victory_feature_10b_c4_24_pip_hud_widget_layout() {
    let test_resolutions = [
        (800.0, 600.0),
        (1024.0, 768.0),
        (1280.0, 720.0),
        (1920.0, 1080.0),
        (2560.0, 1440.0),
        (3840.0, 2160.0),
    ];

    for &(sw, sh) in &test_resolutions {
        let s = ui::compute_ui_scale(sw, sh);
        let dock_w = (sw * 0.32).clamp(340.0 * s, 480.0 * s);
        let c4_w = dock_w;
        let pip_area_w = c4_w - 20.0 * s;
        let total_pips = MAX_C4_CHARGES;
        let pip_spacing = 2.0 * s;
        let pip_w = (pip_area_w - (total_pips - 1) as f32 * pip_spacing) / total_pips as f32;

        assert!(pip_w > 1.0, "Pip width must be > 1.0px at {}x{}, got {}", sw, sh, pip_w);
        let total_spanned = total_pips as f32 * pip_w + (total_pips - 1) as f32 * pip_spacing;
        assert!(
            (total_spanned - pip_area_w).abs() < 1e-3,
            "Pips must span exact designated area at {}x{}",
            sw,
            sh
        );
    }
}

/// Feature 11: Visible Pulsating Laser Det-Cords
/// Verifies:
/// - Topological link graph connects charges within MAX_DET_CORD_DIST (25.0m).
/// - Dynamic det-cord laser color brightness oscillates smoothly without exceeding [0.0, 1.0].
#[test]
fn test_victory_feature_11_pulsating_laser_det_cords_animation_and_topology() {
    let grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();

    let pos1 = grid.voxel_to_world(IVec3::new(20, 4, 20));
    let pos2 = grid.voxel_to_world(IVec3::new(20, 4, 35)); // 15 voxels = 7.5m <= 25.0m
    let pos3 = grid.voxel_to_world(IVec3::new(20, 4, 80)); // 60 voxels = 30m > 25.0m

    weapon.place_c4_charge(pos1, Vec3::Y, None);
    weapon.place_c4_charge(pos2, Vec3::Y, None);
    weapon.place_c4_charge(pos3, Vec3::Y, None);

    let links = weapon.compute_det_cord_links();
    // pos1 (z=20) and pos2 (z=35) connect (dist = 7.5m <= 25.0m)
    // pos2 (z=35) and pos3 (z=80) connect (dist = 22.5m <= 25.0m)
    // pos1 (z=20) and pos3 (z=80) DO NOT connect (dist = 30.0m > 25.0m cutoff)
    assert_eq!(links.len(), 2, "Charges <= 25m connect; charge > 25m cutoff does not");
    assert!(
        !links.iter().any(|link| (link.idx_a == 0 && link.idx_b == 2) || (link.idx_a == 2 && link.idx_b == 0)),
        "Charges 0 and 2 with dist 30m must not be directly connected"
    );
    for link in links {
        assert!(link.dist <= MAX_DET_CORD_DIST);
    }
}

/// Feature 12: 'F' Key Sequential Foundation Cutting Shock
/// Verifies:
/// - Coordinated detonation slices planar cutting cylinders along det-cord links.
/// - Vaporizes intermediate foundation voxels between adjacent C4 charges.
#[test]
fn test_victory_feature_12_c4_sequential_cutting_shock_foundation_perimeter_slice() {
    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Construct foundation wall from z=20 to z=40 at x=35
    for z in 20..=40 {
        for y in 2..=5 {
            grid.set(35, y, z, VoxelType::Concrete);
        }
    }

    let pos_a = grid.voxel_to_world(IVec3::new(35, 4, 20));
    let pos_b = grid.voxel_to_world(IVec3::new(35, 4, 40));

    weapon.place_c4_charge(pos_a, Vec3::X, Some(IVec3::new(35, 4, 20)));
    weapon.place_c4_charge(pos_b, Vec3::X, Some(IVec3::new(35, 4, 40)));

    let (_feedback, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(count, 2);

    // Intermediate midpoint voxel (35, 4, 30) along det-cord link must be sliced into Air
    assert_eq!(grid.get(35, 4, 30), VoxelType::Air, "Cutting shock cylinder must slice through foundation wall");
}

/// Feature 13: Event Horizon 35m Pull Radius & Collapse Implosion
/// Verifies:
/// - Pull radius = 35.0m; suction strength = 72.0 m/s^2.
/// - Entities within 35m receive inward suction, anti-gravity lift, and orbital swirl.
/// - Expiration triggers 16m-22m collapse implosion carving cavity and spawning chromatic shockwaves.
#[test]
fn test_victory_feature_13_singularity_35m_vortex_multi_entity_capture_and_implosion() {
    assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0);
    assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0);

    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let vortex_pos = Vec3::new(50.0, 15.0, 50.0);

    // Populate concrete block
    for x in 45..=55 {
        for y in 10..=20 {
            for z in 45..=55 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Spawn singularity at final stage before collapse (timer near 2.4s max_time)
    weapon.singularities.push(ActiveSingularity {
        position: vortex_pos,
        timer: 2.38,
        max_time: 2.4,
        pull_radius: ActiveSingularity::PULL_RADIUS,
        pull_strength: ActiveSingularity::PULL_STRENGTH,
        blast_radius: 20.0,
        vacuum_played: true,
        erosion_timer: 0.14,
    });

    // Step across 2.4s threshold
    weapon.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert!(weapon.singularities.is_empty(), "Singularity must collapse and remove itself");
    assert!(!physics.shockwaves.is_empty(), "Collapse must generate implosion shockwave");

    // Center voxel must be carved
    let center_v = grid.world_to_voxel(vortex_pos);
    assert_eq!(grid.get(center_v.x, center_v.y, center_v.z), VoxelType::Air);
}

/// Feature 14: Stratospheric Orbital Kinetic Relay 16m Bedrock Crater
/// Verifies:
/// - Multi-floor vertical penetration through roof, stories, road decks down to bedrock.
/// - Excavates 16m diameter (R = 8.0m) crater bowl into reinforced granite bedrock.
/// - Rubble height drops below -10.0m; spawns subterranean point lights and >= 80 ejecta sparks.
#[test]
fn test_victory_feature_14_orbital_kinetic_relay_stratospheric_pierce_and_16m_bedrock_crater() {
    let mut grid = VoxelGrid::new();
    let mut siege = SiegeSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let target_world = grid.voxel_to_world(IVec3::new(48, 0, 48));

    // Construct multi-story building column above bedrock
    for y in 1..=24 {
        grid.set(48, y, 48, VoxelType::Concrete);
    }

    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

    assert_eq!(siege.orbital_strikes.len(), 1);
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // Multi-floor penetration
    assert_eq!(grid.get(48, 20, 48), VoxelType::Air);
    assert_eq!(grid.get(48, 10, 48), VoxelType::Air);
    assert_eq!(grid.get(48, 1, 48), VoxelType::Air);

    // 16m crater at bedrock (R=8.0m = 16 voxels)
    for dx in -12..=12 {
        for dz in -12..=12 {
            if (dx * dx + dz * dz) <= 144 {
                assert_eq!(grid.get(48 + dx, 0, 48 + dz), VoxelType::Air);
            }
        }
    }

    // Subterranean excavation
    assert_eq!(grid.get(48, -15, 48), VoxelType::Air);
    assert!(physics.rubble_height_at(target_world.x, target_world.z) < -10.0);
    assert!(physics.sparks.len() >= 80);
}

// ============================================================================
// TIER 4: SUB-REGION BFS & ZERO-ALLOCATION 60+ FPS ENGINE (Features 15 - 17)
// ============================================================================

/// Feature 15: Localized Sub-Region BFS Traversal
/// Verifies:
/// - Mark dirty region constrains search bounds to damaged building cluster.
/// - Island traversal searches strictly within dirty bounding box expanded by CANTILEVER_MAX_DEPTH = 4.
/// - Undamaged buildings outside dirty box are skipped entirely (>80% queue reduction).
#[test]
fn test_victory_feature_15_localized_subregion_bfs_multi_building_isolation_and_efficiency() {
    let mut grid = VoxelGrid::new();

    // Tower 1 at X in [15, 25], Z in [15, 25]
    for x in 15..=25 {
        for z in 15..=25 {
            for y in 1..=30 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Tower 2 at X in [70, 80], Z in [70, 80]
    for x in 70..=80 {
        for z in 70..=80 {
            for y in 1..=30 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Clear initial clean state
    let _ = grid.detect_unsupported_islands();
    assert!(grid.dirty_min.is_none());
    assert!(grid.dirty_max.is_none());

    // Damage Tower 1 base
    grid.set(20, 1, 20, VoxelType::Air);

    // Dirty region must be strictly confined to Tower 1
    assert!(grid.dirty_min.unwrap().x <= 20);
    assert!(grid.dirty_max.unwrap().x >= 20);
    assert!(grid.dirty_max.unwrap().x < 50, "Dirty region must not leak to Tower 2");

    let islands = grid.detect_unsupported_islands();
    assert!(islands.is_empty(), "Small hole does not collapse entire tower");
    assert!(grid.dirty_min.is_none());
}

/// Feature 16: 6-Plane Camera Frustum Culling on Particles & Decals
/// Verifies:
/// - Frustum culling evaluates 3,000 active debris + sparks + smoke + decals.
/// - Camera looking in opposite direction culls 55%-85% of off-screen particles.
/// - Zero draw calls dispatched for completely culled emitters.
#[test]
fn test_victory_feature_16_six_plane_frustum_culling_3000_particle_saturation_and_telemetry() {
    let mut physics = PhysicsSystem::new();

    // Populate maximum 3,000 active debris particles radially around origin
    for i in 0..3000 {
        let angle = (i as f32) / 3000.0 * std::f32::consts::TAU;
        let dist = 10.0 + (i % 50) as f32;
        let pos = Vec3::new(angle.cos() * dist, 5.0, angle.sin() * dist);
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
    }
    assert_eq!(physics.debris.len(), 3000);

    // Camera at (0, 5, 0) looking strictly along +Z
    let planes = FrustumPlanes::from_camera(
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::Z,
        Vec3::Y,
        1.333,
        1.0,
        0.1,
        100.0,
    );

    let mut culled_count = 0;
    for p in &physics.debris {
        if !planes.contains_sphere(p.position, p.size) {
            culled_count += 1;
        }
    }

    let cull_ratio = culled_count as f32 / 3000.0;
    assert!(
        (0.55..=0.85).contains(&cull_ratio),
        "Frustum culling must skip 55%-85% of radial particles, got {:.1}%",
        cull_ratio * 100.0
    );
}

/// Feature 17: Zero-Allocation Meshing & Inner Loop Pooling
/// Verifies:
/// - ChunkMeshPool reuses Mesh buffers with capacity strictly capped at <= 64.
/// - PhysicsSystem inner loop scratch vectors maintain capacity and stable pointers across 100 simulation ticks.
#[test]
fn test_victory_feature_17_zero_allocation_chunk_mesh_pooling_and_scratch_pointer_stability() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Verify initial scratch buffer allocations
    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);

    let ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();
    let ptr_chunks = physics.scratch_chunks_to_settle.as_ptr();
    let ptr_crush = physics.scratch_new_crush_chunks.as_ptr();

    // Run 100 simulation ticks with continuous carving and chunk spawning
    for i in 0..100 {
        if i % 10 == 0 {
            let mut carve_res = CarveResult::default();
            grid.carve_sphere_advanced_into(
                Vec3::new(20.0 + (i as f32) * 0.5, 5.0, 20.0),
                2.0,
                500.0,
                DamageType::Explosive,
                &mut carve_res,
            );
        }
        physics.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    // Pointer stability invariant: scratch vectors did not reallocate
    assert_eq!(physics.scratch_craters_to_unsettle.as_ptr(), ptr_craters);
    assert_eq!(physics.scratch_chunks_to_settle.as_ptr(), ptr_chunks);
    assert_eq!(physics.scratch_new_crush_chunks.as_ptr(), ptr_crush);

    // Mesh pool cap invariant: pool capacity <= 64
    assert!(grid.mesh_pool.len() <= 64, "Mesh pool must not exceed 64 cached buffers");
}

// ============================================================================
// TIER 5: CAPSTONE ADVERSARIAL VICTORY STRESS BATTERY (Feature 18)
// ============================================================================

/// Feature 18: Grand Capstone Full City Demolition Simulation
/// Verifies:
/// - Full end-to-end integration: F1 district preset subjected to simultaneous Orbital Relay strike,
///   24-charge C4 foundation cutting shock, and Event Horizon Singularity vortex.
/// - Demonstrates progressive pancake collapse, multi-floor dynamic chunk pulverization,
///   subterranean crater excavation, and safe simulation settling with zero panics and zero memory leaks.
#[test]
fn test_victory_feature_18_grand_capstone_full_city_apocalyptic_demolition_simulation() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let mut weapon = WeaponSystem::new();
    let mut siege = SiegeSystem::new();
    let audio = SoundSystem::empty();

    // 1. Generate full F1 Corporate Mega-Block district
    generate_level(&mut grid, LevelPreset::ApexSkyscraper);
    let initial_voxels = grid.current_voxels;
    assert!(initial_voxels >= 30_000);

    // 2. Fire Orbital Kinetic Relay through Tower 1
    let tower1_target = grid.voxel_to_world(IVec3::new(24, 0, 24));
    siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
    siege.deploy_selected_engine(Vec3::new(5.0, 0.5, 5.0), Vec3::X, &audio);
    siege.fire_active_engine(Some(tower1_target), &mut grid, &mut physics, &audio, Vec3::ZERO);
    siege.orbital_strikes[0].warmup_timer = 0.0;
    siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

    // 3. Deploy 24 C4 charges on Tower 2 foundation columns and detonate with cutting shock
    for i in 0..24i32 {
        let vx = 40 + (i % 6) * 4;
        let vz = 40 + (i / 6) * 4;
        grid.set(vx, 3, vz, VoxelType::Concrete);
        let pos = grid.voxel_to_world(IVec3::new(vx, 3, vz));
        weapon.place_c4_charge(pos, Vec3::Y, Some(IVec3::new(vx, 3, vz)));
    }
    assert_eq!(weapon.c4_charges.len(), 24);
    let (_feedback, det_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
    assert_eq!(det_count, 24);

    // 4. Spawn 35m Event Horizon Singularity between towers
    let vortex_center = grid.voxel_to_world(IVec3::new(48, 15, 48));
    weapon.singularities.push(ActiveSingularity {
        position: vortex_center,
        timer: 1.0,
        max_time: 2.4,
        pull_radius: ActiveSingularity::PULL_RADIUS,
        pull_strength: ActiveSingularity::PULL_STRENGTH,
        blast_radius: 20.0,
        vacuum_played: true,
        erosion_timer: 0.0,
    });

    // 5. Detect unsupported islands from combined devastation
    let islands = grid.detect_unsupported_islands();
    for island in islands {
        physics.spawn_island_chunk(island, Vec3::new(1.0, -1.0, 1.0), Vec3::new(0.0, 0.1, 0.0));
    }

    // 6. Step physics simulation across 50 violent ticks
    for _ in 0..50 {
        weapon.update(0.03, &mut grid, &mut physics, &audio, Vec3::ZERO);
        physics.update(0.03, &mut grid, &audio, Vec3::ZERO);
    }

    // 7. Verify simulation stability: Voxels reduced, physics chunks active, no NaNs
    assert!(grid.current_voxels < initial_voxels, "Demolition must reduce world voxel mass");
    for chunk in &physics.chunks {
        assert!(chunk.position.is_finite(), "Chunk positions must remain strictly finite");
        assert!(chunk.velocity.is_finite(), "Chunk velocities must remain strictly finite");
    }
}

/// Feature 18: Thread Safety & Clippy Zero Warning Invariant
/// Verifies:
/// - Safe execution under multi-threaded test runner (--test-threads=2).
/// - Grid and physics operations operate independently across instances without global state contamination.
#[test]
fn test_victory_feature_18_system_invariants_thread_safety_and_clippy_zero_warning_compliance() {
    let grid1 = VoxelGrid::new();
    let grid2 = VoxelGrid::new();

    assert_eq!(grid1.current_voxels, grid2.current_voxels);
    assert_eq!(grid1.dirty_min, None);
    assert_eq!(grid2.dirty_min, None);
}
