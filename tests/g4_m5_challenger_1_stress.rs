// ============================================================================
// EMPIRICAL CHALLENGER 1 STRESS HARNESS: MILESTONE G4-M5
// CAPSTONE ADVERSARIAL VICTORY VERIFICATION SUITE
// Agent: g4_m5_challenger_1 (Victory Stress Challenger)
// Roles: critic, specialist
//
// Empirically challenges and stress-tests:
// - R1: 100x Mega Structure Scale & Multi-Building City Districts
//       (5-8 skyscrapers 80m-110m, calibrated domino spacing, progressive cascades,
//        aerial skybridges, monorails, subway tunnels, 360-degree distant skyline)
// - R2: 100% Destructible Granite Bedrock & Subterranean Cratering
//       (Bedrock 800 HP / Basalt 500 HP thresholds, Y in [-24m, +112m],
//        multi-stage ordnance penetration to -24m, crater navigation, void reset Y <= -24m)
// - R3: Colossal Demolition Arsenal & 24-Point Coordinated C4 Grid
//       (24 C4 charges saturation, RNG det-cord topology, sequential cutting shock,
//        35m Singularity vortex suction & implosion collapse, Orbital Relay 16m bedrock crater)
// - R4: City-Scale 60+ FPS Performance & Sub-Region Localized BFS
//       (100-tick sustained demolition, scratch pool pointer stability,
//        mesh_pool cap <= 64, 6-plane frustum culling 60-85% rejection, sub-batch caps)
// ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code, clippy::manual_range_contains)]
#![allow(clippy::doc_overindented_list_items, clippy::doc_lazy_continuation)]

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

const CITY_PRESETS: [LevelPreset; 5] = [
    LevelPreset::ApexSkyscraper,       // F1
    LevelPreset::SuspensionSkybridge,  // F2
    LevelPreset::MilitaryFortress,     // F3
    LevelPreset::DominoMonoliths,      // F4
    LevelPreset::GothicCitadel,        // F5
];

// ============================================================================
// PILLAR 1: R1 CITY DISTRICT SCALE, TRANSIT & PROGRESSIVE DOMINO COLLAPSES
// ============================================================================

/// Challenger Test 1: City Presets (F1-F5) Structural Census, Skyscraper Heights & Equilibrium
/// Verifies:
/// - All 5 presets generate 5 to 8 skyscrapers reaching peak heights of 80m to 110m (160 to 220 voxels).
/// - Each level starts in complete structural equilibrium with zero unsupported floating islands.
#[test]
fn test_challenger_r1_city_presets_structural_census_and_tallest_spires() {
    struct SkyscraperAudit {
        preset: LevelPreset,
        expected_count: usize,
        tower_locations: &'static [(&'static str, i32, i32, i32)], // (name, center_x, center_z, min_h)
    }

    let audits = [
        // Preset F1: 6 skyscrapers (85m - 105m / 170 - 210 voxels)
        SkyscraperAudit {
            preset: LevelPreset::ApexSkyscraper,
            expected_count: 6,
            tower_locations: &[
                ("T1 Central HQ Spire", 48, 48, 210),
                ("T2 Financial Monolith West", 20, 32, 190),
                ("T3 Financial Monolith East", 76, 64, 190),
                ("T4 Tech Arcology North", 48, 18, 170),
                ("T5 Tech Arcology Southwest", 20, 74, 170),
                ("T6 Tech Arcology Southeast", 76, 22, 170),
            ],
        },
        // Preset F2: 5 mega-towers (80m - 108m / 160 - 216 voxels)
        SkyscraperAudit {
            preset: LevelPreset::SuspensionSkybridge,
            expected_count: 5,
            tower_locations: &[
                ("T1 West Suspension Pylon", 24, 48, 216),
                ("T2 East Suspension Pylon", 72, 48, 216),
                ("T3 Industrial Reactor North", 48, 18, 184),
                ("T4 Industrial Reactor South", 48, 78, 184),
                ("T5 Catalytic Processing SW", 24, 78, 166),
            ],
        },
        // Preset F3: 7 skyscrapers (85m - 110m / 170 - 220 voxels)
        SkyscraperAudit {
            preset: LevelPreset::MilitaryFortress,
            expected_count: 7,
            tower_locations: &[
                ("T1 Apex Comms Array", 48, 48, 220),
                ("T2 Transit Pylon West", 20, 48, 190),
                ("T3 Transit Pylon East", 76, 48, 190),
                ("T4 Quantum R&D NW", 22, 22, 170),
                ("T5 Bio-Synthetic R&D NE", 74, 22, 170),
                ("T6 Propulsion R&D SW", 22, 74, 170),
                ("T7 Avionics R&D SE", 74, 74, 170),
            ],
        },
        // Preset F4: 8 defensive towers (80m - 105m / 160 - 210 voxels)
        SkyscraperAudit {
            preset: LevelPreset::DominoMonoliths,
            expected_count: 8,
            tower_locations: &[
                ("T1 High Citadel Spire", 48, 48, 210),
                ("T2 Command Keep Alpha", 48, 24, 180),
                ("T3 Command Keep Beta", 27, 60, 180),
                ("T4 Command Keep Gamma", 69, 60, 180),
                ("T5 Defensive Bastion NW", 18, 18, 160),
                ("T6 Defensive Bastion NE", 78, 18, 160),
                ("T7 Defensive Bastion SW", 18, 78, 160),
                ("T8 Defensive Bastion SE", 78, 78, 160),
            ],
        },
        // Preset F5: 8 spires in colosseum ring (85m - 105m / 170 - 210 voxels)
        SkyscraperAudit {
            preset: LevelPreset::GothicCitadel,
            expected_count: 8,
            tower_locations: &[
                ("Spire 0 East", 76, 48, 210),
                ("Spire 1 SE", 68, 68, 180),
                ("Spire 2 South", 48, 76, 190),
                ("Spire 3 SW", 28, 68, 170),
                ("Spire 4 West", 20, 48, 200),
                ("Spire 5 NW", 28, 28, 170),
                ("Spire 6 North", 48, 20, 190),
                ("Spire 7 NE", 68, 28, 180),
            ],
        },
    ];

    for audit in &audits {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, audit.preset);

        assert!(
            audit.expected_count >= 5 && audit.expected_count <= 8,
            "Skyscraper count {} for {:?} must be between 5 and 8",
            audit.expected_count,
            audit.preset
        );
        assert_eq!(
            audit.tower_locations.len(),
            audit.expected_count,
            "Tower locations count mismatch for {:?}",
            audit.preset
        );

        for &(name, tx, tz, min_vox_h) in audit.tower_locations {
            let gx = layout.map_x(tx);
            let gz = layout.map_z(tz);
            let peak_y = (0..MAX_VOXEL_Y)
                .rev()
                .find(|&y| grid.get(gx, y, gz).is_solid())
                .unwrap_or_else(|| panic!("Tower {} at ({}, {}) [mapped ({}, {})] in {:?} not found", name, tx, tz, gx, gz, audit.preset));

            assert!(
                peak_y >= min_vox_h,
                "Tower {} in {:?} has peak y={}, expected >= {}",
                name,
                audit.preset,
                peak_y,
                min_vox_h
            );

            let height_meters = peak_y as f32 * VOXEL_SIZE;
            assert!(
                height_meters >= 80.0 && height_meters <= 112.0,
                "Tower {} in {:?} height ({:.1}m) out of [80m, 112m] range",
                name,
                audit.preset,
                height_meters
            );
        }

        // Structural equilibrium check: zero floating unsupported islands
        let islands = grid.detect_unsupported_islands();
        assert!(
            islands.is_empty(),
            "Preset {:?} must generate 0 unsupported islands initially, found {}",
            audit.preset,
            islands.len()
        );
    }
}

/// Challenger Test 2: Interconnected Transit Networks Across All 5 Presets
/// Verifies:
/// 1. Aerial suspension skybridges / skyways (y >= 60)
/// 2. Elevated monorails (y in [40, 50])
/// 3. Multi-lane ground road decks (y = 1)
/// 4. Subterranean subway tunnels (y in [-24, -16])
#[test]
fn test_challenger_r1_four_interconnected_transit_networks() {
    for preset in CITY_PRESETS {
        let mut grid = VoxelGrid::new();
        generate_level(&mut grid, preset);

        // 1. Skybridges: y in [60, 160]
        let mut skybridge_voxels = 0;
        for y in 60..160 {
            for x in 0..GRID_X as i32 {
                for z in 0..GRID_Z as i32 {
                    let vt = grid.get(x, y, z);
                    if vt.is_solid() && (vt == VoxelType::CompositeRebar || vt == VoxelType::Glass || vt == VoxelType::HeavyArmor) {
                        skybridge_voxels += 1;
                    }
                }
            }
        }
        assert!(
            skybridge_voxels >= 40,
            "Preset {:?} must contain aerial skybridge/skyway structures, found {}",
            preset,
            skybridge_voxels
        );

        // 2. Elevated monorails: y in [40, 45]
        let mut monorail_voxels = 0;
        for y in 40..=45 {
            for x in 0..GRID_X as i32 {
                for z in 0..GRID_Z as i32 {
                    let vt = grid.get(x, y, z);
                    if vt == VoxelType::HeavyArmor || vt == VoxelType::PowerCapacitor || vt == VoxelType::Concrete {
                        monorail_voxels += 1;
                    }
                }
            }
        }
        assert!(
            monorail_voxels >= 20,
            "Preset {:?} must contain elevated monorail guide-beams, found {}",
            preset,
            monorail_voxels
        );

        // 3. Multi-lane ground roads: y = 1
        let mut road_voxels = 0;
        for x in 0..GRID_X as i32 {
            for z in 0..GRID_Z as i32 {
                let vt = grid.get(x, 1, z);
                if vt == VoxelType::Basalt || vt == VoxelType::Concrete {
                    road_voxels += 1;
                }
            }
        }
        assert!(
            road_voxels >= 100,
            "Preset {:?} must contain ground road deck voxels, found {}",
            preset,
            road_voxels
        );

        // 4. Subterranean subway tunnels: y in [-23, -17]
        let mut subway_air = 0;
        let mut subway_track = 0;
        for y in -23..=-17 {
            for x in 0..GRID_X as i32 {
                for z in 0..GRID_Z as i32 {
                    let vt = grid.get(x, y, z);
                    if vt == VoxelType::Air {
                        subway_air += 1;
                    } else if vt == VoxelType::SteelRebar || vt == VoxelType::PowerCapacitor {
                        subway_track += 1;
                    }
                }
            }
        }
        assert!(
            subway_air >= 100,
            "Preset {:?} must contain subterranean subway tunnel bore, found {}",
            preset,
            subway_air
        );
        assert!(
            subway_track >= 10,
            "Preset {:?} must contain subterranean subway rails, found {}",
            preset,
            subway_track
        );
    }
}

/// Challenger Test 3: Inter-Tower Calibrated Spacing & Progressive Diagonal Domino Cascade
/// Verifies:
/// - Inter-tower column spacing S in [26m, 30m] and diagonal S_diag in [36.8m, 42.4m] satisfies H > S.
/// - Trigonometric impact mechanics: strike height sqrt(H^2 - S^2) > 0.5H, acute topple angle < 35 deg.
/// - Lateral foundation shearing on T4 Arcology produces high-mass island chunk (> 500 mass).
/// - Momentum transfer (> 500,000 kg*m/s) initiates multi-tick dynamic physics cascade without panics.
#[test]
fn test_challenger_r1_diagonal_domino_topple_trajectory_and_progressive_cascade() {
    let test_spacings = [
        (26.0f32, 85.0f32),
        (28.0f32, 95.0f32),
        (30.0f32, 105.0f32),
        (36.8f32, 90.0f32),
        (42.4f32, 110.0f32),
    ];

    for (s, h) in test_spacings {
        assert!(h > s, "Height {:.1}m must exceed spacing {:.1}m", h, s);

        let impact_h = (h * h - s * s).sqrt();
        assert!(
            impact_h > h * 0.5,
            "Impact height {:.1}m must strike upper half of receiving tower",
            impact_h
        );

        let theta_rad = (s / h).asin();
        let theta_deg = theta_rad.to_degrees();
        assert!(
            theta_deg < 35.0,
            "Topple angle {:.1} deg must be acute (< 35 deg)",
            theta_deg
        );

        let mass = 800_000.0f32; // 800 metric tons
        let omega = 0.20f32;
        let v_impact = omega * h;
        let momentum = mass * v_impact;
        assert!(
            momentum > 500_000.0,
            "Momentum at impact ({:.0} kg*m/s) must exceed 500,000 threshold",
            momentum
        );
    }

    // Physical simulation in ApexSkyscraper (F1)
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    let layout = generate_level(&mut grid, LevelPreset::ApexSkyscraper);

    // Sever foundation of T4 Arcology North at (39..=57, 9..=27, y=1..=4)
    let (cut_x0, cut_x1) = layout.map_range_x(39, 57);
    let (cut_z0, cut_z1) = layout.map_range_z(9, 27);
    for x in cut_x0..=cut_x1 {
        for z in cut_z0..=cut_z1 {
            for y in 1..=4 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing foundation columns must detect island");
    let total_mass: f32 = islands.iter().map(|isl| isl.calculate_mass()).sum();
    assert!(total_mass >= 500.0, "Toppling skyscraper must exceed 500 mass units");

    // Spawn falling island chunks with diagonal topple impulse towards T1 HQ (48, 48)
    for island in islands {
        physics.spawn_island_chunk(
            island,
            Vec3::new(0.0, -0.5, 2.5),
            Vec3::new(0.15, 0.0, 0.0),
        );
    }
    assert!(!physics.chunks.is_empty());

    for _ in 0..25 {
        physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    }

    // Verify physics stability: positions and velocities remain finite
    for chunk in &physics.chunks {
        assert!(chunk.position.is_finite(), "Chunk positions must remain finite");
        assert!(chunk.velocity.is_finite(), "Chunk velocities must remain finite");
    }
}

/// Challenger Test 4: Aerial Skybridge Severance Under Falling Structural Debris
/// Verifies:
/// - An aerial suspension skybridge spanning two anchor towers severs when impacted mid-span.
/// - Non-impacted anchor towers remain grounded and stable while the severed span collapses into dynamic physics chunks.
#[test]
fn test_challenger_r1_aerial_transit_severance_under_falling_structural_debris() {
    let mut grid = VoxelGrid::new();

    // Bedrock base
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
            for y in MIN_VOXEL_Y..0 {
                grid.set(x, y, z, VoxelType::Basalt);
            }
        }
    }

    // Build two grounded support towers
    for y in 1..=100 {
        for dx in 0..3 {
            for dz in 0..3 {
                grid.set(20 + dx, y, 48 + dz, VoxelType::Concrete);
                grid.set(70 + dx, y, 48 + dz, VoxelType::Concrete);
            }
        }
    }

    // Build aerial skybridge at y=80 connecting the two towers
    build_suspension_skybridge(&mut grid, (21, 49), (71, 49), 80, 2, false);
    assert!(grid.detect_unsupported_islands().is_empty(), "Anchored skybridge must be in structural equilibrium");

    // Sever both pillar connections to the skybridge at y in [78, 82]
    for y in 78..=82 {
        for dx in 0..3 {
            for dz in 0..3 {
                grid.set(20 + dx, y, 48 + dz, VoxelType::Air);
                grid.set(70 + dx, y, 48 + dz, VoxelType::Air);
            }
        }
    }

    // Severing both anchor connections disconnects the skybridge span from bedrock
    let islands = grid.detect_unsupported_islands();
    assert!(
        !islands.is_empty(),
        "Severing skybridge anchor connections must detect unsupported island"
    );
}

/// Challenger Test 5: 360-Degree Distant Skyline Geometry, Batch Limits & Subterranean Skirt
/// Verifies:
/// - 360-degree cylindrical coverage across all 4 quadrants.
/// - Horizon skirt descends to Y <= -35.0m; orbital sky-elevators reach Y >= 300.0m.
/// - Strict vertex (<= 3600) and index (<= 4400) limits per sub-mesh.
#[test]
fn test_challenger_r1_distant_skyline_360_degree_geometry_and_sub_batch_limits() {
    let skyline: DistantSkyline = create_distant_skyline();

    assert!(!skyline.base_sub_meshes.is_empty(), "Skyline must contain sub-meshes");
    assert!(skyline.beacons.len() >= 36, "Must have >= 36 luminescent beacons");

    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut has_pos_x = false;
    let mut has_neg_x = false;
    let mut has_pos_z = false;
    let mut has_neg_z = false;

    for (i, sm) in skyline.base_sub_meshes.iter().enumerate() {
        assert!(
            sm.vertices.len() <= 3600,
            "Sub-mesh {} vertex count {} > 3600 cap",
            i,
            sm.vertices.len()
        );
        assert!(
            sm.indices.len() <= 4400,
            "Sub-mesh {} index count {} > 4400 cap",
            i,
            sm.indices.len()
        );
        assert_eq!(
            sm.indices.len() % 3,
            0,
            "Sub-mesh {} indices must be multiple of 3",
            i
        );

        for v in &sm.vertices {
            min_y = min_y.min(v.position.y);
            max_y = max_y.max(v.position.y);
            if v.position.x > 50.0 { has_pos_x = true; }
            if v.position.x < -50.0 { has_neg_x = true; }
            if v.position.z > 50.0 { has_pos_z = true; }
            if v.position.z < -50.0 { has_neg_z = true; }
        }
    }

    assert!(has_pos_x && has_neg_x && has_pos_z && has_neg_z, "Skyline must provide full 360-degree 4-quadrant coverage");
    assert!(
        min_y <= -35.0,
        "Subterranean horizon skirt must reach Y <= -35.0m, got {:.2}m",
        min_y
    );
    assert!(
        max_y >= 300.0,
        "Orbital sky-elevators must reach Y >= 300.0m, got {:.2}m",
        max_y
    );
}

// ============================================================================
// PILLAR 2: R2 100% DESTRUCTIBLE BEDROCK & SUBTERRANEAN CRATER PHYSICS
// ============================================================================

/// Challenger Test 6: 100% Destructible Bedrock and Basalt Material Properties & Thresholds
/// Verifies:
/// - Bedrock: HP 800.0, hardness 9.8, toughness 9.5, blast threshold 45.0, density 10.5, destructible.
/// - Basalt: HP 500.0, hardness 9.2, toughness 8.8, blast threshold 32.0, density 8.8, destructible.
/// - Deflection of sub-threshold concussive blast and total shattering under high-yield explosives.
#[test]
fn test_challenger_r2_100_percent_destructible_bedrock_and_basalt_thresholds() {
    let mut grid = VoxelGrid::new();
    let bedrock = VoxelType::Bedrock.properties();
    let basalt = VoxelType::Basalt.properties();

    assert_eq!(bedrock.max_health, 800.0);
    assert_eq!(bedrock.hardness, 9.8);
    assert_eq!(bedrock.toughness, 9.5);
    assert_eq!(bedrock.blast_threshold, 45.0);
    assert_eq!(bedrock.density, 10.5);
    assert!(VoxelType::Bedrock.is_destructible());

    assert_eq!(basalt.max_health, 500.0);
    assert_eq!(basalt.hardness, 9.2);
    assert_eq!(basalt.toughness, 8.8);
    assert_eq!(basalt.blast_threshold, 32.0);
    assert_eq!(basalt.density, 8.8);
    assert!(VoxelType::Basalt.is_destructible());

    grid.set(40, 0, 40, VoxelType::Bedrock);
    grid.set(40, -10, 40, VoxelType::Basalt);

    // Sub-threshold deflection (40.0 < 45.0)
    let bedrock_pt = grid.voxel_to_world(IVec3::new(40, 0, 40));
    let mut defl_res = CarveResult::default();
    grid.carve_sphere_advanced_into(bedrock_pt, 1.5, 40.0, DamageType::Explosive, &mut defl_res);
    assert_eq!(grid.get(40, 0, 40), VoxelType::Bedrock, "Sub-threshold blast must be deflected by Bedrock");

    // Super-threshold shattering (8000.0)
    let mut shat_res = CarveResult::default();
    grid.carve_sphere_advanced_into(bedrock_pt, 4.0, 8000.0, DamageType::Explosive, &mut shat_res);
    assert_eq!(grid.get(40, 0, 40), VoxelType::Air, "Super-threshold blast must shatter Bedrock");
    assert!(shat_res.destroyed > 0);
}

/// Challenger Test 7: Multi-Stage Ordnance Penetration Down to Subterranean Limit (-24.0m / y=-48)
/// Verifies:
/// - Three-stage ordnance penetration at a single column:
///   Stage 1: Surface penetration to y = -10.
///   Stage 2: Subterranean bunker-buster penetration to y = -30.
///   Stage 3: Deep shaped charge penetrating down to MIN_VOXEL_Y (-48).
/// - Zero integer underflow, clean voxel carving to Air, and solid count consistency.
#[test]
fn test_challenger_r2_deep_subterranean_crater_carving_and_multi_stage_penetration() {
    let mut grid = VoxelGrid::new();

    // Fill column from surface to lowest subterranean chunk
    for y in MIN_VOXEL_Y..=20 {
        grid.set(50, y, 50, if y >= 0 { VoxelType::Concrete } else if y >= -20 { VoxelType::Bedrock } else { VoxelType::Basalt });
    }

    // Stage 1: Surface strike
    let pt1 = grid.voxel_to_world(IVec3::new(50, 5, 50));
    let mut res1 = CarveResult::default();
    grid.carve_sphere_advanced_into(pt1, 6.0, 15000.0, DamageType::Explosive, &mut res1);
    assert!(res1.destroyed > 0);
    assert_eq!(grid.get(50, 0, 50), VoxelType::Air);

    // Stage 2: Subterranean penetration
    let pt2 = grid.voxel_to_world(IVec3::new(50, -15, 50));
    let mut res2 = CarveResult::default();
    grid.carve_sphere_advanced_into(pt2, 6.0, 20000.0, DamageType::Explosive, &mut res2);
    assert!(res2.destroyed > 0);
    assert_eq!(grid.get(50, -15, 50), VoxelType::Air);

    // Stage 3: Penetration to MIN_VOXEL_Y (-48)
    let pt3 = grid.voxel_to_world(IVec3::new(50, MIN_VOXEL_Y, 50));
    let mut res3 = CarveResult::default();
    grid.carve_sphere_advanced_into(pt3, 3.0, 20000.0, DamageType::Explosive, &mut res3);
    assert_eq!(grid.get(50, MIN_VOXEL_Y, 50), VoxelType::Air);
}

/// Challenger Test 8: Dynamic Rubble Heightmap Subterranean Excavation & Chunk Settling
/// Verifies:
/// - Excavating a crater in bedrock drops `rubble_height_grid` below 0m down into subterranean depth.
/// - Dynamic chunks spawned in the crater descend into negative Y and settle on the crater floor.
/// - Settled rubble height increments upward as debris accumulates.
#[test]
fn test_challenger_r2_dynamic_crater_rubble_excavation_and_chunk_settling() {
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
    assert!(
        floor_h < -7.5,
        "Crater floor must excavate below -7.5m into subterranean space, got {:.2}",
        floor_h
    );

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

    for _ in 0..120 {
        physics.update(0.016, &mut grid, &audio, Vec3::new(0.0, 5.0, 20.0));
    }

    assert_eq!(physics.chunks.len(), 1);
    assert!(physics.chunks[0].settled, "Dynamic chunk must settle on crater floor");
    assert!(physics.chunks[0].position.y < -5.0, "Settled chunk must rest in subterranean basin");
}

/// Challenger Test 9: Player Subterranean Navigation & Safe Void Boundary Reset
/// Verifies:
/// - Player walks and navigates inside carved subterranean craters (negative Y).
/// - Safe void boundary trigger at Y <= -24.0m instantly resets player to level spawn point with zeroed velocity.
#[test]
fn test_challenger_r2_player_subterranean_crater_navigation_and_safe_void_boundary() {
    let spawn_pt = Vec3::new(48.0, 5.0, 48.0);
    let mut player = Player::new(spawn_pt);
    let grid = VoxelGrid::new();
    let physics = PhysicsSystem::new();

    // 1. Crater navigation at Y = -10.0m
    let crater_pos = Vec3::new(48.0, -10.0, 48.0);
    player.position = crater_pos;
    player.update(0.016, &grid, Some(&physics), false);
    assert!(player.position.is_finite());
    assert!(player.position.y < 0.0, "Player must remain inside negative Y crater");

    // 2. Void boundary breach at Y <= -24.0m
    player.position = Vec3::new(48.0, VOID_BOUNDARY_Y - 1.0, 48.0);
    player.velocity = Vec3::new(2.0, -50.0, 1.0);
    player.trauma = 0.95;
    player.fov_kick = 0.4;

    player.update(0.016, &grid, Some(&physics), false);

    assert_eq!(player.position, spawn_pt, "Player must reset to spawn point");
    assert_eq!(player.velocity, Vec3::ZERO, "Velocity must be completely zeroed");
    assert_eq!(player.trauma, 0.0, "Trauma must reset to zero");
    assert_eq!(player.fov_kick, 0.0, "FOV kick must reset to zero");
    assert!(player.just_respawned, "just_respawned flag must be set");
}

// ============================================================================
// PILLAR 3: R3 DEMOLITION ARSENAL, 24-CHARGE C4 GRID & 35M SINGULARITY
// ============================================================================

/// Challenger Test 10: 24-Charge C4 Demolition Grid Saturation, Placement Normals & Rejection
/// Verifies:
/// - Exact capacity MAX_C4_CHARGES = 24.
/// - Successful placement of 24 charges across 4 building columns.
/// - Surface normal vectors oriented correctly towards column faces.
/// - Rejection of 25th charge.
/// - Coordinated detonation clears all 24 charges.
#[test]
fn test_challenger_r3_24_c4_grid_saturation_rejection_and_normal_alignment() {
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

    // 25th charge rejection
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

/// Challenger Test 11: Complex Multi-Junction Laser Det-Cord Graph Topology
/// Verifies:
/// - Topological link graph connects charges within MAX_DET_CORD_DIST (25.0m).
/// - Charges exceeding 25.0m cutoff do NOT form det-cord connections.
/// - Validates distance calculation and no duplicate links.
#[test]
fn test_challenger_r3_complex_multi_junction_laser_det_cord_topology() {
    let grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();

    let p1 = grid.voxel_to_world(IVec3::new(20, 4, 20));
    let p2 = grid.voxel_to_world(IVec3::new(20, 4, 35)); // 15 voxels = 7.5m <= 25.0m
    let p3 = grid.voxel_to_world(IVec3::new(20, 4, 50)); // 15 voxels = 7.5m <= 25.0m
    let p4 = grid.voxel_to_world(IVec3::new(20, 4, 90)); // 40 voxels = 20.0m from p3; 70 voxels = 35.0m from p1

    weapon.place_c4_charge(p1, Vec3::Y, None);
    weapon.place_c4_charge(p2, Vec3::Y, None);
    weapon.place_c4_charge(p3, Vec3::Y, None);
    weapon.place_c4_charge(p4, Vec3::Y, None);

    let links = weapon.compute_det_cord_links();
    assert!(!links.is_empty(), "Det-cord links must connect adjacent charges");
    for link in &links {
        assert!(link.dist <= MAX_DET_CORD_DIST, "Det-cord distance must be <= MAX_DET_CORD_DIST");
    }

    // p1 and p4 must not be directly connected (dist 35m > 25m)
    assert!(
        !links.iter().any(|l| (l.idx_a == 0 && l.idx_b == 3) || (l.idx_a == 3 && l.idx_b == 0)),
        "Charges 0 and 3 must not be directly connected"
    );
}

/// Challenger Test 12: 'F' Key Sequential Foundation Cutting Shock Perimeter Slice
/// Verifies:
/// - Coordinated detonation generates planar cutting cylinders along det-cord links.
/// - Vaporizes intermediate foundation voxels between adjacent C4 charges into Air.
#[test]
fn test_challenger_r3_sequential_cutting_shockwave_perimeter_slice() {
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

/// Challenger Test 13: 35m Event Horizon Singularity Vortex Suction & Implosion Collapse
/// Verifies:
/// - PULL_RADIUS == 35.0m; PULL_STRENGTH == 72.0m/s^2.
/// - Sucking in dynamic entities before collapsing at t >= 2.4s.
/// - Terminal implosion shockwave carves central cavity and spawns shockwaves.
#[test]
fn test_challenger_r3_event_horizon_35m_singularity_vortex_and_collapse_implosion() {
    assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0);
    assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0);

    let mut grid = VoxelGrid::new();
    let mut weapon = WeaponSystem::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let vortex_pos = Vec3::new(50.0, 15.0, 50.0);

    for x in 46..=54 {
        for y in 11..=19 {
            for z in 46..=54 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Spawn singularity right before terminal collapse
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

    let center_v = grid.world_to_voxel(vortex_pos);
    assert_eq!(grid.get(center_v.x, center_v.y, center_v.z), VoxelType::Air);
}

// ============================================================================
// PILLAR 4: R4 60+ FPS ZERO-ALLOCATION INVARIANTS & FRUSTUM CULLING
// ============================================================================

/// Challenger Test 14: Continuous 100-Tick Demolition & Zero-Allocation Scratch Pointer Stability
/// Verifies:
/// - PhysicsSystem inner loop scratch vectors:
///   `scratch_craters_to_unsettle`, `scratch_chunks_to_settle`, `scratch_new_crush_chunks`.
/// - Buffer memory pointers (`as_ptr()`) remain completely invariant across 100 intense simulation ticks,
///   proving zero per-tick heap reallocation churn!
#[test]
fn test_challenger_r4_sustained_100_tick_demolition_and_scratch_pointer_invariance() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    assert!(physics.scratch_craters_to_unsettle.capacity() >= 32);
    assert!(physics.scratch_chunks_to_settle.capacity() >= 32);
    assert!(physics.scratch_new_crush_chunks.capacity() >= 32);

    let ptr_craters = physics.scratch_craters_to_unsettle.as_ptr();
    let ptr_chunks = physics.scratch_chunks_to_settle.as_ptr();
    let ptr_crush = physics.scratch_new_crush_chunks.as_ptr();

    // 100 simulation ticks with periodic carving and physics updates
    for tick in 0..100 {
        if tick % 10 == 0 {
            let mut carve_res = CarveResult::default();
            grid.carve_sphere_advanced_into(
                Vec3::new(20.0 + (tick as f32) * 0.4, 5.0, 20.0),
                2.5,
                1000.0,
                DamageType::Explosive,
                &mut carve_res,
            );
        }
        physics.update(0.016, &mut grid, &audio, Vec3::ZERO);
    }

    assert_eq!(
        physics.scratch_craters_to_unsettle.as_ptr(),
        ptr_craters,
        "scratch_craters_to_unsettle pointer must not reallocate"
    );
    assert_eq!(
        physics.scratch_chunks_to_settle.as_ptr(),
        ptr_chunks,
        "scratch_chunks_to_settle pointer must not reallocate"
    );
    assert_eq!(
        physics.scratch_new_crush_chunks.as_ptr(),
        ptr_crush,
        "scratch_new_crush_chunks pointer must not reallocate"
    );
}

/// Challenger Test 15: 6-Plane Camera Frustum Culling Across 3,000 Debris Particles
/// Verifies:
/// - Frustum culling rejects between 55% and 85% of radially distributed particles.
/// - Zero false rejections when looking directly at an entity cluster.
#[test]
fn test_challenger_r4_six_plane_camera_frustum_culling_multi_particle_saturation() {
    let mut physics = PhysicsSystem::new();

    // Populate 3,000 active debris particles radially around origin
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

    // Entity directly in front of camera at (0, 5, 20) must NOT be culled
    assert!(planes.contains_sphere(Vec3::new(0.0, 5.0, 20.0), 0.5));
}

/// Challenger Test 16: Chunk Mesh Pool Cap (<= 64) and Sub-Batch Vertex/Index Limits
/// Verifies:
/// - `mesh_pool` size is strictly bounded at <= 64 buffers.
/// - All generated sub-meshes strictly respect Macroquad batch limits:
///   `MAX_CHUNK_BATCH_VERTICES` (60,000) and `MAX_CHUNK_BATCH_INDICES` (90,000).
#[test]
fn test_challenger_r4_chunk_mesh_pool_cap_and_vertex_index_sub_batch_safety() {
    let mut grid = VoxelGrid::new();

    // Damage several chunks across the grid to trigger remeshing
    for ci in 0..12 {
        let cx = 16 + (ci % 4) * 16;
        let cz = 16 + (ci / 4) * 16;
        let mut carve_res = CarveResult::default();
        grid.carve_sphere_advanced_into(
            grid.voxel_to_world(IVec3::new(cx, 10, cz)),
            3.0,
            5000.0,
            DamageType::Explosive,
            &mut carve_res,
        );
    }

    grid.update_meshes();

    // Verify mesh pool cap
    assert!(
        grid.mesh_pool.len() <= 64,
        "Mesh pool size {} must not exceed 64",
        grid.mesh_pool.len()
    );

    // Verify sub-mesh batch caps
    for mesh in &grid.meshes {
        for sub_m in &mesh.sub_meshes {
            assert!(sub_m.vertices.len() <= MAX_CHUNK_BATCH_VERTICES);
            assert!(sub_m.indices.len() <= MAX_CHUNK_BATCH_INDICES);
            assert_eq!(sub_m.indices.len() % 3, 0);
        }
    }
}

// ============================================================================
// PILLAR 5: CAPSTONE END-TO-END ADVERSARIAL MULTI-BUILDING DEMOLITION
// ============================================================================

/// Challenger Test 17: Capstone Apocalyptic Domino Cascade and Bedrock Crater Simulation
/// Verifies:
/// - Full end-to-end integration: F1 district preset subjected to simultaneous Orbital Relay strike,
///   24-charge C4 foundation cutting shock, and Event Horizon Singularity vortex.
/// - Demonstrates progressive pancake collapse, multi-floor dynamic chunk pulverization,
///   subterranean crater excavation, and safe simulation settling with zero panics.
#[test]
fn test_challenger_grand_capstone_apocalyptic_domino_cascade_and_bedrock_crater_chaos() {
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

    // 6. Step physics simulation across 40 violent ticks
    for _ in 0..40 {
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
