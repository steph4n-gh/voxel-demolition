//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G5-M1
//! ADVERSARIAL STRESS-TESTING: VERTICAL LOCALIZATION, CEILING EXPANSION,
//! SUBWAY TUNNEL GENERATION & SPALL EVENT ZERO-ALLOCATION SHORT-CIRCUIT
//! Target: tests/g5_m1_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use macroquad::prelude::*;
use player::{Player, PLAYER_HEIGHT, PLAYER_RADIUS};
use voxel::{
    CarveResult, DamageType, VoxelGrid, VoxelType, CANTILEVER_MAX_DEPTH, GRID_X, GRID_Y, GRID_Z,
    MAX_VOXEL_Y, MIN_VOXEL_Y, VOXEL_SIZE,
};
use world_gen::{build_subterranean_subway_tunnel, generate_level, LevelPreset};

// ============================================================================
// SUITE 1: ADAPTIVE CEILING EXPANSION & VERTICAL LOCALIZATION
// ============================================================================

/// Helper to construct a clean grid with bedrock foundation at y=MIN_VOXEL_Y..=0
fn create_test_bedrock_grid() -> VoxelGrid {
    let mut grid = VoxelGrid::new();
    grid.clear();
    for x in 0..GRID_X {
        for z in 0..GRID_Z {
            for y in MIN_VOXEL_Y..=0 {
                grid.set(x as i32, y, z as i32, VoxelType::Bedrock);
            }
        }
    }
    grid.total_initial_voxels = grid.solid_count;
    grid.current_voxels = grid.solid_count;
    grid
}

#[test]
fn test_adaptive_ceiling_expansion_unsupported_touching_box_max_y_collapses() {
    let mut grid = create_test_bedrock_grid();

    // 1. Create a tall grounded pillar at (10, 10) to set max_solid_y = 60
    for y in 1..=60 {
        grid.set(10, y, 10, VoxelType::Concrete);
    }
    grid.max_solid_y = 60;

    // 2. Place an unsupported floating structure at x=30..=32, z=30..=32 spanning y=15..=20
    // d_max.y will be set to 12. With pad = 8, box_max_y = 12 + 8 = 20.
    // The structure touches box_max_y = 20, but y=21 is Air.
    for x in 30..=32 {
        for z in 30..=32 {
            for y in 15..=20 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }

    // Mark dirty region so localized BFS activates with d_min.y=10, d_max.y=12 -> box_max_y = 20
    grid.mark_dirty_region(IVec3::new(28, 10, 28), IVec3::new(34, 12, 34));

    // Run island detection
    let islands = grid.detect_unsupported_islands();

    // The unsupported floating block touches box_max_y=20, has Air at y=21, so needs_vertical_expand is false.
    // It is unsupported and must be detected as an island chunk and removed from the grid.
    assert!(!islands.is_empty(), "Unsupported structure touching box_max_y must be detected as island");
    let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    // 3 * 3 * 6 = 54 voxels
    assert_eq!(total_island_voxels, 54, "All 54 voxels of the unsupported platform must collapse");

    // Verify grid has Air where the unsupported structure was
    for x in 30..=32 {
        for z in 30..=32 {
            for y in 15..=20 {
                assert_eq!(grid.get(x, y, z), VoxelType::Air, "Collapsed voxels must be cleared to Air");
            }
        }
    }
}

#[test]
fn test_adaptive_ceiling_expansion_connected_to_roof_propagates_grounding() {
    let mut grid = create_test_bedrock_grid();

    // 1. Tall grounded column at (30, 25) going all the way from ground y=1 up to y=40
    // Must be tensile rebar so continuous tension anchoring is established from foundation
    for y in 1..=40 {
        grid.set(30, y, 25, VoxelType::SteelRebar);
    }
    grid.max_solid_y = 60; // global_max_y = 61

    // 2. Horizontal roof/beam at y=40 connecting the column at (30, 25) to (30, 30)
    for z in 25..=30 {
        grid.set(30, 40, z, VoxelType::SteelRebar); // Rebar beam provides structural tension
    }

    // 3. Hanging vertical column suspended from the roof at (30, 30) from y=39 down to y=15
    // It crosses box_max_y = 20! (y=21 is solid, y=20 is solid).
    for y in 15..=39 {
        grid.set(30, y, 30, VoxelType::SteelRebar);
    }

    // 4. Mark localized dirty region with d_max.y = 12 -> pad = 8 -> box_max_y = 20.
    // Initial box_max_y = 20 touches the hanging column at y=20, and y=21 is solid!
    // This MUST trigger adaptive vertical expansion to global_max_y, discover the roof at y=40,
    // and ground the entire suspended column via the grounded column at (30, 25).
    grid.mark_dirty_region(IVec3::new(28, 10, 23), IVec3::new(32, 12, 32));

    let islands = grid.detect_unsupported_islands();

    // Since the structure is properly anchored via the roof at y=40 to the grounded column at (30, 25),
    // adaptive expansion must prevent false collapse.
    assert!(
        islands.is_empty(),
        "Structure supported by roof above box_max_y must NOT collapse (found {} islands)",
        islands.len()
    );

    // Verify all voxels in the suspended column remain solid
    for y in 15..=39 {
        assert_eq!(
            grid.get(30, y, 30),
            VoxelType::SteelRebar,
            "Suspended rebar at y={} must remain intact",
            y
        );
    }
}

#[test]
fn test_adaptive_ceiling_expansion_floating_roof_remains_unsupported() {
    let mut grid = create_test_bedrock_grid();

    // 1. Independent tall column to set global_max_y high
    for y in 1..=60 {
        grid.set(10, y, 10, VoxelType::Concrete);
    }
    grid.max_solid_y = 60;

    // 2. Floating structure spanning y=15..=20, continuing upward across y=21 up to y=35 (floating roof)
    // Entire structure is floating in midair (starts at y=15, nowhere connects to ground).
    for y in 15..=35 {
        grid.set(30, y, 30, VoxelType::Concrete);
    }

    // Mark dirty region so initial box_max_y = 12 + 8 = 20.
    // y=20 is solid and y=21 is solid, so adaptive ceiling expansion expands to global_max_y.
    // However, the roof is floating, so NO grounded status reaches it.
    grid.mark_dirty_region(IVec3::new(28, 10, 28), IVec3::new(32, 12, 32));

    let islands = grid.detect_unsupported_islands();

    assert!(
        !islands.is_empty(),
        "Floating structure with floating roof must collapse after vertical expansion"
    );

    let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    // 35 - 15 + 1 = 21 voxels
    assert_eq!(
        total_island_voxels, 21,
        "Entire floating structure from y=15 to y=35 must be collected as island"
    );

    for y in 15..=35 {
        assert_eq!(
            grid.get(30, y, 30),
            VoxelType::Air,
            "Voxel at y={} must be cleared to Air",
            y
        );
    }
}

#[test]
fn test_adaptive_ceiling_expansion_cantilever_depth_limit_enforced() {
    let mut grid = create_test_bedrock_grid();

    // Grounded pillar up to y=30
    for y in 1..=30 {
        grid.set(20, y, 20, VoxelType::Concrete);
    }
    grid.max_solid_y = 50;

    // Overhang extends horizontally at y=30 from x=20 to x=28 (8 voxels wide)
    // CANTILEVER_MAX_DEPTH is 4. Voxels x=21..=24 can be supported, x=25..=28 exceed cantilever limit!
    for x in 21..=28 {
        grid.set(x, 30, 20, VoxelType::Concrete);
    }

    // Localized dirty region with d_max.y = 12 -> box_max_y = 20.
    // The grounded column touches box_max_y = 20 and continues at y=21, triggering vertical expansion.
    grid.mark_dirty_region(IVec3::new(18, 10, 18), IVec3::new(22, 12, 22));

    let islands = grid.detect_unsupported_islands();

    // Overhanging voxels x=25..=28 must collapse as an island
    assert!(!islands.is_empty(), "Overhanging cantilever beyond depth 4 must collapse");
    let collapsed_voxels: Vec<IVec3> = islands.iter().flat_map(|isl| isl.voxels.iter().map(|(p, _)| *p)).collect();

    for x in 25..=28 {
        assert!(
            collapsed_voxels.contains(&IVec3::new(x, 30, 20)),
            "Overextended voxel at x={} must collapse",
            x
        );
    }
    // Voxels within cantilever limit (x=21..=24) must remain solid
    for x in 21..=24 {
        assert_eq!(
            grid.get(x, 30, 20),
            VoxelType::Concrete,
            "Supported cantilever voxel at x={} must remain",
            x
        );
    }
}

// ============================================================================
// SUITE 2: SUBWAY TUNNEL GENERATION ACROSS ALL 5 LEVEL PRESETS (F1–F5)
// ============================================================================

const PRESETS_F1_TO_F5: [LevelPreset; 5] = [
    LevelPreset::ApexSkyscraper,       // F1
    LevelPreset::SuspensionSkybridge,  // F2
    LevelPreset::MilitaryFortress,     // F3
    LevelPreset::DominoMonoliths,      // F4
    LevelPreset::GothicCitadel,        // F5
];

#[test]
fn test_subway_tunnel_glowcore_lights_in_all_five_presets() {
    for &preset in &PRESETS_F1_TO_F5 {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, preset);

        let center_z = layout.map_z(48);
        let station_z = layout.map_z(50);
        let (min_x, max_x) = layout.map_range_x(2, 94);
        let (station_min_x, station_max_x) = layout.map_range_x(42, 54);

        let mut ceiling_glow_count = 0;
        let mut station_glow_count = 0;

        for x in min_x..=max_x {
            if grid.get(x, -17, center_z) == VoxelType::GlowCore {
                ceiling_glow_count += 1;
            }
        }

        for x in station_min_x..=station_max_x {
            if grid.get(x, -21, station_z) == VoxelType::GlowCore {
                station_glow_count += 1;
            }
        }

        assert!(
            ceiling_glow_count >= 20,
            "Preset {:?} must generate at least 20 GlowCore ceiling lights along subway tunnel (found {})",
            preset,
            ceiling_glow_count
        );

        assert!(
            station_glow_count >= 8,
            "Preset {:?} must generate station platform GlowCore lights at y=-21 (found {})",
            preset,
            station_glow_count
        );
    }
}

#[test]
fn test_subway_tunnel_continuous_headroom_and_navigable_all_presets() {
    for &preset in &PRESETS_F1_TO_F5 {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, preset);

        let (start_x, end_x) = layout.map_range_x(3, 93);
        let tunnel_z = layout.map_z(48);
        let platform_z = layout.map_z(51);
        let (shaft_min_x, shaft_max_x) = layout.map_range_x(47, 49);

        // Verify that from start_x to end_x, there is a navigable vertical corridor:
        // Floor at y=-23 (or platform/track at y=-22), with at least 3 voxels of Air overhead (y=-21..=-19).
        for x in start_x..=end_x {
            // Check walking clearance at tunnel_z (or station platform at platform_z when around station)
            let test_z = if (shaft_min_x..=shaft_max_x).contains(&x) {
                // Around elevator shaft, player walks on the platform at platform_z
                platform_z
            } else {
                tunnel_z
            };

            let floor_solid = grid.get(x, -23, test_z).is_solid();
            assert!(
                floor_solid,
                "Preset {:?} at x={}, z={} must have solid tunnel floor at y=-23",
                preset, x, test_z
            );

            // Check headroom: at least 3 contiguous Air voxels from y=-21 to y=-19
            let air_clearance = (-21..=-19).filter(|&y| grid.get(x, y, test_z) == VoxelType::Air).count();
            assert_eq!(
                air_clearance, 3,
                "Preset {:?} at x={}, z={} must have at least 3 voxels (1.5m+) of clear Air headroom",
                preset, x, test_z
            );
        }
    }
}

#[test]
fn test_subway_tunnel_player_physics_navigation_simulation() {
    // Empirically simulate Player entity navigating from west end to east end of the subway tunnel
    for &preset in &PRESETS_F1_TO_F5 {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, preset);

        let start_x = layout.map_x(4);
        let tunnel_z = layout.map_z(51);
        let floor_world = grid.voxel_to_world(IVec3::new(start_x, -23, tunnel_z));
        let start_pos = Vec3::new(floor_world.x, floor_world.y + VOXEL_SIZE * 0.5, floor_world.z);
        let mut player = Player::new(start_pos);

        // Step player through the tunnel eastward with forward velocity
        let dt = 0.016; // 60 FPS
        let total_steps = 1200; // Expanded world has ~2x walking span
        player.velocity = Vec3::new(8.0, 0.0, 0.0);

        let mut traversed_x = false;
        let dest_x = grid.voxel_to_world(IVec3::new(layout.map_x(80), -23, tunnel_z)).x;
        for _ in 0..total_steps {
            // Apply steady forward motion
            player.velocity.x = 8.0;
            player.update(dt, &grid, None, false);

            // Verify player has not clipped through bedrock into the deep void
            assert!(
                player.position.y > -15.0,
                "Preset {:?}: Player fell through tunnel floor! y={}",
                preset,
                player.position.y
            );

            // Reached eastern terminal
            if player.position.x >= dest_x {
                traversed_x = true;
                break;
            }
        }

        assert!(
            traversed_x,
            "Preset {:?}: Player must navigate subway tunnel smoothly without getting trapped",
            preset
        );
    }
}

#[test]
fn test_subway_station_elevator_access_shaft_open_vertical_transit() {
    for &preset in &PRESETS_F1_TO_F5 {
        let mut grid = VoxelGrid::new();
        let layout = generate_level(&mut grid, preset);

        let shaft_pos = layout.map_voxel(48, 0, 48);
        let wall_pos = layout.map_voxel(47, 0, 48);

        // The station elevator access shaft is located at (48, 48), ascending from y=-20 to y=4
        // Interior must be hollow Air for all y in -20..=-1
        for y in -20..=-1 {
            assert_eq!(
                grid.get(shaft_pos.x, y, shaft_pos.z),
                VoxelType::Air,
                "Preset {:?}: Elevator shaft interior at ({}, {}, {}) must be Air",
                preset,
                shaft_pos.x,
                y,
                shaft_pos.z
            );
        }

        // Shaft perimeter must be structural SteelRebar walls
        for y in -20..=-1 {
            assert_eq!(
                grid.get(wall_pos.x, y, wall_pos.z),
                VoxelType::SteelRebar,
                "Preset {:?}: Elevator shaft wall at ({}, {}, {}) must be SteelRebar",
                preset,
                wall_pos.x,
                y,
                wall_pos.z
            );
        }
    }
}

// ============================================================================
// SUITE 3: SPALL EVENT SHORT-CIRCUIT & ZERO-ALLOCATION DETONATIONS
// ============================================================================

#[test]
fn test_spall_event_cap_32_under_massive_detonation() {
    let mut grid = create_test_bedrock_grid();

    // Build a large 10x10x10 cube of HeavyArmor (high blast threshold)
    for x in 20..30 {
        for y in 5..15 {
            for z in 20..30 {
                grid.set(x, y, z, VoxelType::HeavyArmor);
            }
        }
    }
    grid.max_solid_y = 20;

    // Verify initial state
    assert_eq!(grid.spall_events.len(), 0);
    assert_eq!(grid.spall_events.capacity(), 32);

    // Inflict sub-threshold blast damage across hundreds of surface voxels
    // HeavyArmor blast threshold is 35.0. Inflict 20.0 damage.
    let mut damage_calls = 0;
    for x in 20..30 {
        for y in 5..15 {
            for z in 20..30 {
                let destroyed = grid.damage_voxel(x, y, z, 20.0, DamageType::Explosive);
                assert!(!destroyed, "Damage below threshold must not destroy heavy armor");
                damage_calls += 1;
            }
        }
    }

    assert!(damage_calls >= 500, "Must have called damage_voxel on at least 500 voxels");

    // CRITICAL: spall_events must be capped at EXACTLY 32!
    assert_eq!(
        grid.spall_events.len(),
        32,
        "spall_events must be capped at exactly 32"
    );

    // CRITICAL: capacity must NEVER expand beyond 32 (proving zero heap allocations)
    assert_eq!(
        grid.spall_events.capacity(),
        32,
        "spall_events capacity must remain strictly 32 without reallocations"
    );
}

#[test]
fn test_spall_short_circuit_skips_is_surface_voxel_when_full() {
    let mut grid = create_test_bedrock_grid();

    // Fill a 4x4x4 cube of HeavyArmor
    for x in 20..24 {
        for y in 5..9 {
            for z in 20..24 {
                grid.set(x, y, z, VoxelType::HeavyArmor);
            }
        }
    }

    // Fill spall_events to capacity 32 manually
    for i in 0..32 {
        grid.spall_events.push((Vec3::splat(i as f32), Color::new(1.0, 0.5, 0.2, 1.0)));
    }
    assert_eq!(grid.spall_events.len(), 32);

    // Now call damage_voxel on an interior voxel (e.g. 21, 6, 21) which is NOT a surface voxel.
    // In the old code: is_surface_voxel was evaluated before length check.
    // In the optimized code: `self.spall_events.len() < 32` short-circuits, completely bypassing is_surface_voxel!
    let destroyed = grid.damage_voxel(21, 6, 21, 10.0, DamageType::Explosive);
    assert!(!destroyed);
    assert_eq!(grid.spall_events.len(), 32);
    assert_eq!(grid.spall_events.capacity(), 32);
}

#[test]
fn test_carve_sphere_advanced_into_spalled_positions_cap() {
    let mut grid = create_test_bedrock_grid();

    // Create a dense cluster of Concrete
    for x in 10..30 {
        for y in 5..20 {
            for z in 10..30 {
                grid.set(x, y, z, VoxelType::Concrete);
            }
        }
    }
    grid.max_solid_y = 25;

    let center = grid.voxel_to_world(IVec3::new(20, 10, 20));
    let mut result = CarveResult::default();

    // Detonate with small damage that causes heavy spall but does not completely obliterate
    grid.carve_sphere_advanced_into(center, 6.0, 40.0, DamageType::Explosive, &mut result);

    assert!(result.spalled_positions.len() <= 32, "CarveResult spalled_positions must never exceed 32");
    assert!(grid.spall_events.len() <= 32, "Grid spall_events must never exceed 32");
}

#[test]
fn test_repeated_detonations_zero_capacity_growth() {
    let mut grid = create_test_bedrock_grid();

    for x in 15..35 {
        for y in 5..25 {
            for z in 15..35 {
                grid.set(x, y, z, VoxelType::HeavyArmor);
            }
        }
    }
    grid.max_solid_y = 30;

    let initial_cap = grid.spall_events.capacity();
    assert_eq!(initial_cap, 32);

    // Simulate 20 successive explosion cycles
    for cycle in 0..20 {
        let center = grid.voxel_to_world(IVec3::new(20 + (cycle % 5), 10, 20 + (cycle % 5)));
        let mut result = CarveResult::default();
        grid.carve_sphere_advanced_into(center, 4.0, 25.0, DamageType::Explosive, &mut result);

        // Verify cap is maintained during detonation
        assert!(grid.spall_events.len() <= 32);
        assert_eq!(
            grid.spall_events.capacity(),
            32,
            "Capacity must remain exactly 32 on cycle {}",
            cycle
        );

        // Simulate frame end drain (as done in PhysicsSystem::step)
        grid.spall_events.clear();
        assert_eq!(grid.spall_events.len(), 0);
        assert_eq!(grid.spall_events.capacity(), 32);
    }
}
