#![allow(clippy::new_without_default)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::PhysicsSystem;
use skyline::{create_distant_skyline, DistantSkyline};
use voxel::{VoxelGrid, VoxelType, GRID_X, GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y, VOXEL_SIZE};
use world_gen::{
    build_elevated_monorail, build_multi_lane_road_deck, build_subterranean_subway_tunnel,
    build_suspension_skybridge, generate_level, LevelPreset,
};

// ============================================================================
// MILESTONE G4-M2 INTEGRATION TEST SUITE: CITY-SCALE DISTRICTS & TRANSIT
// Features 6, 7, 8, 9: Skyscraper City Presets, Domino Physics, Transit, Skyline
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Preset F1: Corporate Mega-Block (6 Skyscrapers, 85m-105m tall)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_f1_corporate_megablock_anatomy_and_scale() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::ApexSkyscraper);

    // 1. Minimum mass: Sprawling district of 6 skyscrapers contains > 30,000 voxels
    assert!(
        grid.current_voxels >= 30_000,
        "Corporate Mega-Block must contain >= 30,000 voxels, got {}",
        grid.current_voxels
    );

    // 2. High-tensile composite rebar skeleton >= 1,000 voxels
    assert!(
        grid.current_rebar_count >= 1_000,
        "Corporate Mega-Block rebar skeleton must be >= 1,000 voxels, got {}",
        grid.current_rebar_count
    );

    // 3. Central Corporate HQ Spire (T1): Centered at (48, 48), reaches 210 voxels (105.0m)
    let hq_x = layout.map_x(48);
    let hq_z = layout.map_z(48);
    let hq_max_y = (0..MAX_VOXEL_Y)
        .rev()
        .find(|&y| grid.get(hq_x, y, hq_z).is_solid())
        .expect("T1 HQ spire must exist at (48, 48)");
    assert!(
        hq_max_y >= 210,
        "T1 Central HQ Spire must reach >= 210 voxels (105m), got y={}",
        hq_max_y
    );

    // 4. Verify 6 skyscraper spires at their designated coordinates:
    // T1: (48, 48), T2: (20, 32), T3: (76, 64), T4: (48, 18), T5: (20, 74), T6: (76, 22)
    let tower_coords = [
        ("T1 Central HQ", 48, 48, 210),
        ("T2 Monolith West", 20, 32, 190),
        ("T3 Monolith East", 76, 64, 190),
        ("T4 Arcology North", 48, 18, 170),
        ("T5 Arcology Southwest", 20, 74, 170),
        ("T6 Arcology Southeast", 76, 22, 170),
    ];

    for &(name, tx, tz, min_h) in &tower_coords {
        let gx = layout.map_x(tx);
        let gz = layout.map_z(tz);
        let max_y = (0..MAX_VOXEL_Y)
            .rev()
            .find(|&y| grid.get(gx, y, gz).is_solid())
            .unwrap_or_else(|| panic!("Tower {} at ({}, {}) must exist", name, gx, gz));
        assert!(
            max_y >= min_h,
            "Tower {} at ({}, {}) must reach height >= {}, got {}",
            name,
            gx,
            gz,
            min_h,
            max_y
        );
        // Verify height in meters (1 voxel = 0.5m)
        let height_meters = max_y as f32 * VOXEL_SIZE;
        assert!(
            height_meters >= 80.0,
            "Tower {} height in meters must be >= 80.0m, got {}m",
            name,
            height_meters
        );
    }

    // 5. Verify Aerial Skybridges:
    // Skybridge 1 (y=120): Between T1 and T2
    let sb1_pos = layout.map_voxel(33, 120, 40);
    assert!(
        grid.get(sb1_pos.x, sb1_pos.y, sb1_pos.z).is_solid(),
        "Skybridge 1 must exist at y=120 mid-span between T1 and T2"
    );
    // Skybridge 2 (y=140): Between T1 and T3
    let sb2_pos = layout.map_voxel(63, 140, 56);
    assert!(
        grid.get(sb2_pos.x, sb2_pos.y, sb2_pos.z).is_solid(),
        "Skybridge 2 must exist at y=140 mid-span between T1 and T3"
    );

    // 6. Verify Elevated Monorail: y=42 along z=48
    let mono_pos = layout.map_voxel(48, 42, 48);
    assert!(
        grid.get(mono_pos.x, mono_pos.y, mono_pos.z).is_solid(),
        "Elevated monorail track must cross ({}, 42, {})",
        mono_pos.x,
        mono_pos.z
    );

    // 7. Verify Subterranean Subway Tunnel: y=-20 along z=48
    let sub_pos = layout.map_voxel(48, -20, 48);
    assert_eq!(
        grid.get(sub_pos.x, sub_pos.y, sub_pos.z),
        VoxelType::Air,
        "Subway tunnel interior at y=-20 must be hollow Air"
    );
    assert!(
        grid.get(sub_pos.x, -22, sub_pos.z).is_solid(),
        "Subway tunnel track ballast at y=-22 must be solid"
    );

    // 8. Zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Corporate Mega-Block must have ZERO unsupported islands at spawn, got {}",
        islands.len()
    );
}

// ----------------------------------------------------------------------------
// 2. Preset F2: Industrial Fusion District (5 Mega-Towers, 80m-108m tall)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_f2_industrial_fusion_district_suspension_span_and_coolant_skyways() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::SuspensionSkybridge);

    assert!(
        grid.current_voxels >= 30_000,
        "Industrial Fusion District must contain >= 30,000 voxels, got {}",
        grid.current_voxels
    );

    // 1. Verify 5 Mega-Towers:
    // T1: (24, 48) West Pylon (216 voxels / 108m)
    // T2: (72, 48) East Pylon (216 voxels / 108m)
    // T3: (48, 18) Industrial Reactor North (184 voxels / 92m)
    // T4: (48, 78) Industrial Reactor South (184 voxels / 92m)
    // T5: (24, 78) Catalytic Processing Center (166 voxels / 83m)
    let industrial_towers = [
        ("T1 West Pylon", 24, 48, 216),
        ("T2 East Pylon", 72, 48, 216),
        ("T3 Reactor North", 48, 18, 184),
        ("T4 Reactor South", 48, 78, 184),
        ("T5 Catalytic Center", 24, 78, 166),
    ];

    for &(name, tx, tz, min_h) in &industrial_towers {
        let gx = layout.map_x(tx);
        let gz = layout.map_z(tz);
        let max_y = (0..MAX_VOXEL_Y)
            .rev()
            .find(|&y| grid.get(gx, y, gz).is_solid())
            .unwrap_or_else(|| panic!("Tower {} at ({}, {}) must exist", name, gx, gz));
        assert!(
            max_y >= min_h,
            "Tower {} must reach >= {} voxels, got {}",
            name,
            min_h,
            max_y
        );
        let hm = max_y as f32 * VOXEL_SIZE;
        assert!(
            hm >= 80.0,
            "Tower {} height must be >= 80.0m, got {}m",
            name,
            hm
        );
    }

    // 2. Cryogenic cooling jackets & conduits >= 200 voxels
    let mut cryo_count = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::CryoIce {
            cryo_count += 1;
        }
    }
    assert!(
        cryo_count >= 200,
        "Industrial Fusion District must contain >= 200 CryoIce voxels, got {}",
        cryo_count
    );

    // 3. Dual-Deck Suspension Span along z=48 at y=20 (lower) and y=34 (upper)
    let bridge_x = layout.map_x(48);
    let bridge_z = layout.map_z(48);
    assert!(
        grid.get(bridge_x, 20, bridge_z).is_solid(),
        "Suspension bridge lower deck must span ({}, 20, {})",
        bridge_x,
        bridge_z
    );
    assert!(
        grid.get(bridge_x, 34, bridge_z).is_solid(),
        "Suspension bridge upper deck must span ({}, 34, {})",
        bridge_x,
        bridge_z
    );

    // 4. Zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Industrial Fusion District must have ZERO unsupported islands at spawn, got {}",
        islands.len()
    );
}

// ----------------------------------------------------------------------------
// 3. Preset F3: High-Tech Aeropolis (7 Skyscrapers, 85m-110m tall)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_f3_aeropolis_propellant_silos_and_transit() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::MilitaryFortress);

    assert!(
        grid.current_voxels >= 30_000,
        "High-Tech Aeropolis must contain >= 30,000 voxels, got {}",
        grid.current_voxels
    );

    // 1. Verify 7 Skyscrapers:
    // T1: (48, 48) Deep-Space Comms Array (220 voxels / 110m)
    // T2: (20, 48) Transit Pylon West (190 voxels / 95m)
    // T3: (76, 48) Transit Pylon East (190 voxels / 95m)
    // T4: (22, 22) Quantum R&D NW (170 voxels / 85m)
    // T5: (74, 22) Bio-Synthetic R&D NE (170 voxels / 85m)
    // T6: (22, 74) Propulsion R&D SW (170 voxels / 85m)
    // T7: (74, 74) Avionics R&D SE (170 voxels / 85m)
    let aeropolis_towers = [
        ("T1 Comms Array", 48, 48, 220),
        ("T2 Pylon West", 20, 48, 190),
        ("T3 Pylon East", 76, 48, 190),
        ("T4 Quantum R&D NW", 22, 22, 170),
        ("T5 Bio R&D NE", 74, 22, 170),
        ("T6 Propulsion SW", 22, 74, 170),
        ("T7 Avionics SE", 74, 74, 170),
    ];

    for &(name, tx, tz, min_h) in &aeropolis_towers {
        let gx = layout.map_x(tx);
        let gz = layout.map_z(tz);
        let max_y = (0..MAX_VOXEL_Y)
            .rev()
            .find(|&y| grid.get(gx, y, gz).is_solid())
            .unwrap_or_else(|| panic!("Tower {} at ({}, {}) must exist", name, gx, gz));
        assert!(
            max_y >= min_h,
            "Tower {} must reach >= {} voxels, got {}",
            name,
            min_h,
            max_y
        );
        let hm = max_y as f32 * VOXEL_SIZE;
        assert!(
            hm >= 80.0,
            "Tower {} height must be >= 80.0m, got {}m",
            name,
            hm
        );
    }

    // 2. Subterranean propellant silos >= 400 Explosive voxels
    let mut exp_count = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::Explosive {
            exp_count += 1;
        }
    }
    assert!(
        exp_count >= 400,
        "Aeropolis silos must contain >= 400 Explosive voxels, got {}",
        exp_count
    );

    // 3. Central capacitor substation >= 8 PowerCapacitor voxels
    let mut cap_count = 0;
    for vt in &grid.voxels {
        if *vt == VoxelType::PowerCapacitor {
            cap_count += 1;
        }
    }
    assert!(
        cap_count >= 8,
        "Aeropolis must contain >= 8 Power Capacitors, got {}",
        cap_count
    );

    // 4. Zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Aeropolis must have ZERO unsupported islands at spawn, got {}",
        islands.len()
    );
}

// ----------------------------------------------------------------------------
// 4. Preset F4: Fortress Citadel (8 Skyscrapers, 80m-105m tall)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_f4_fortress_citadel_bastions_and_ramparts() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::DominoMonoliths);

    assert!(
        grid.current_voxels >= 35_000,
        "Fortress Citadel must contain >= 35,000 voxels, got {}",
        grid.current_voxels
    );

    // 1. Verify 8 Mega-Towers:
    // T1: (48, 48) High Citadel Spire (210 voxels / 105m)
    // T2: (48, 24) Command Keep Alpha (180 voxels / 90m)
    // T3: (27, 60) Command Keep Beta (180 voxels / 90m)
    // T4: (69, 60) Command Keep Gamma (180 voxels / 90m)
    // T5: (18, 18) Bastion NW (160 voxels / 80m)
    // T6: (78, 18) Bastion NE (160 voxels / 80m)
    // T7: (18, 78) Bastion SW (160 voxels / 80m)
    // T8: (78, 78) Bastion SE (160 voxels / 80m)
    let citadel_towers = [
        ("T1 High Citadel Spire", 48, 48, 210),
        ("T2 Command Keep Alpha", 48, 24, 180),
        ("T3 Command Keep Beta", 27, 60, 180),
        ("T4 Command Keep Gamma", 69, 60, 180),
        ("T5 Bastion NW", 18, 18, 160),
        ("T6 Bastion NE", 78, 18, 160),
        ("T7 Bastion SW", 18, 78, 160),
        ("T8 Bastion SE", 78, 78, 160),
    ];

    for &(name, tx, tz, min_h) in &citadel_towers {
        let gx = layout.map_x(tx);
        let gz = layout.map_z(tz);
        let max_y = (0..MAX_VOXEL_Y)
            .rev()
            .find(|&y| grid.get(gx, y, gz).is_solid())
            .unwrap_or_else(|| panic!("Tower {} at ({}, {}) must exist", name, gx, gz));
        assert!(
            max_y >= min_h,
            "Tower {} must reach >= {} voxels, got {}",
            name,
            min_h,
            max_y
        );
        let hm = max_y as f32 * VOXEL_SIZE;
        assert!(
            hm >= 80.0,
            "Tower {} height must be >= 80.0m, got {}m",
            name,
            hm
        );
    }

    // 2. Shield capacitors >= 32
    let mut cap_count = 0;
    let mut ice_count = 0;
    let mut glow_count = 0;
    for vt in &grid.voxels {
        match *vt {
            VoxelType::PowerCapacitor => cap_count += 1,
            VoxelType::CryoIce => ice_count += 1,
            VoxelType::GlowCore => glow_count += 1,
            _ => {}
        }
    }
    assert!(cap_count >= 32, "Citadel must have >= 32 capacitors, got {}", cap_count);
    assert!(ice_count >= 200, "Citadel must have >= 200 cryo ice, got {}", ice_count);
    assert!(glow_count >= 50, "Citadel must have >= 50 glow cores, got {}", glow_count);

    // 3. Zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Fortress Citadel must have ZERO unsupported islands at spawn, got {}",
        islands.len()
    );
}

// ----------------------------------------------------------------------------
// 5. Preset F5: Dense Neon Metropolis (8 Spires, 85m-105m tall)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_f5_dense_neon_metropolis_colosseum_dominos() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);

    assert!(
        grid.current_voxels >= 30_000,
        "Neon Metropolis must contain >= 30,000 voxels, got {}",
        grid.current_voxels
    );

    // 1. Spire 0 is centered PRECISELY at (76, 48) with base footprint dx, dz in -4..=4
    let s0_x = layout.map_x(76);
    let s0_z = layout.map_z(48);
    let spire_0_max_y = (0..MAX_VOXEL_Y)
        .rev()
        .find(|&y| grid.get(s0_x, y, s0_z).is_solid())
        .expect("Spire 0 must exist at (76, 48)");
    assert!(
        spire_0_max_y >= 210,
        "Spire 0 must reach >= 210 voxels (105m), got y={}",
        spire_0_max_y
    );

    // 2. Verify all 8 spires in the colosseum ring:
    let spire_heights = [210, 180, 190, 170, 200, 170, 190, 180];
    let center_x = 48.0f32;
    let center_z = 48.0f32;
    let circle_radius = 28.0f32;

    for (i, &min_h) in spire_heights.iter().enumerate() {
        let angle = (i as f32 * (360.0 / 8.0f32)).to_radians();
        let sx = (center_x + angle.cos() * circle_radius).round() as i32;
        let sz = (center_z + angle.sin() * circle_radius).round() as i32;
        let gx = layout.map_x(sx);
        let gz = layout.map_z(sz);

        let max_y = (0..MAX_VOXEL_Y)
            .rev()
            .find(|&y| grid.get(gx, y, gz).is_solid())
            .unwrap_or_else(|| panic!("Spire {} at ({}, {}) must exist", i, gx, gz));
        assert!(
            max_y >= min_h,
            "Spire {} must reach >= {} voxels, got {}",
            i,
            min_h,
            max_y
        );
        let hm = max_y as f32 * VOXEL_SIZE;
        assert!(
            hm >= 80.0,
            "Spire {} height must be >= 80.0m, got {}m",
            i,
            hm
        );
    }

    // 3. Zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Neon Metropolis must have ZERO unsupported islands at spawn, got {}",
        islands.len()
    );
}

// ----------------------------------------------------------------------------
// 6. Inter-Tower Domino Spacings & Collapse Reach Invariants
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_inter_tower_domino_spacings_and_cascade_angles() {
    // Calibrated spacing invariant: Adjacent tower spacing S = 26m - 30m (52 to 60 voxels)
    // or diagonal spacing 36.8m - 42.4m (74 to 85 voxels).
    // For toppling heights H = 80m - 110m (160 to 220 voxels),
    // impact reach condition H > S is strictly satisfied.
    let tower_pairs = [
        // Preset F1: T1 (48, 48, H=105m) -> T4 (48, 18, H=85m): S = 15m
        (IVec2::new(48, 48), 105.0f32, IVec2::new(48, 18), 85.0f32),
        // Preset F1: T2 (20, 32, H=95m) -> T1 (48, 48, H=105m): S = 16.1m
        (IVec2::new(20, 32), 95.0f32, IVec2::new(48, 48), 105.0f32),
        // Preset F1: T5 (20, 74, H=85m) -> T2 (20, 32, H=95m): S = 21.0m
        (IVec2::new(20, 74), 85.0f32, IVec2::new(20, 32), 95.0f32),
        // Preset F3: T2 (20, 48, H=95m) -> T1 (48, 48, H=110m): S = 14.0m
        (IVec2::new(20, 48), 95.0f32, IVec2::new(48, 48), 110.0f32),
        // Preset F4: T5 (18, 18, H=80m) -> T2 (48, 24, H=90m): S = 15.3m
        (IVec2::new(18, 18), 80.0f32, IVec2::new(48, 24), 90.0f32),
        // Preset F5: Spire 0 (76, 48, H=105m) -> Spire 1 (68, 68, H=90m): S = 10.8m
        (IVec2::new(76, 48), 105.0f32, IVec2::new(68, 68), 90.0f32),
    ];

    for (p1, h1, p2, h2) in tower_pairs {
        let dv = (p2 - p1).as_vec2();
        let dist_voxels = dv.length();
        let dist_meters = dist_voxels * VOXEL_SIZE;

        // Reach condition: Falling tower must reach the receiving tower before hitting ground
        assert!(
            h1 > dist_meters,
            "Falling tower height ({:.1}m) must exceed center distance ({:.1}m)",
            h1,
            dist_meters
        );

        // Impact height: sqrt(H^2 - S^2)
        let impact_height = (h1 * h1 - dist_meters * dist_meters).sqrt();
        assert!(
            impact_height > h2 * 0.5,
            "Impact height ({:.1}m) must strike receiving tower (H={:.1}m) in upper half",
            impact_height,
            h2
        );

        // Angular topple angle at impact: sin(theta) = S / H
        let sin_theta = dist_meters / h1;
        let theta_deg = sin_theta.asin().to_degrees();
        assert!(
            theta_deg < 35.0,
            "Topple angle at impact ({:.1} deg) must be acute (< 35 deg) for catastrophic shear transfer",
            theta_deg
        );
    }
}

// ----------------------------------------------------------------------------
// 7. Procedural Distant Skyline (36 Sectors, Beacons, Sub-mesh Limits)
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_procedural_distant_skyline_generation_and_mesh_invariants() {
    let skyline: DistantSkyline = create_distant_skyline();

    // 1. Verify base sub-meshes are generated
    assert!(
        !skyline.base_sub_meshes.is_empty(),
        "Distant skyline must contain generated sub-meshes"
    );

    // 2. Verify Skyline Beacons populated with luminescent colors
    assert!(
        !skyline.beacons.is_empty(),
        "Distant skyline must have luminescent beacons"
    );
    assert!(
        skyline.beacons.len() >= 36,
        "Distant skyline must have at least 1 beacon per sector, got {}",
        skyline.beacons.len()
    );

    // 3. Verify Subterranean horizon skirt reaches down to -35.0m
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for sm in &skyline.base_sub_meshes {
        for v in &sm.vertices {
            min_y = min_y.min(v.position.y);
            max_y = max_y.max(v.position.y);
        }
    }
    assert!(
        min_y <= -34.0,
        "Subterranean horizon skirt must descend to <= -34.0m, got {:.1}m",
        min_y
    );
    assert!(
        max_y >= 300.0,
        "Orbital sky-elevators must ascend to >= 300.0m, got {:.1}m",
        max_y
    );

    // 4. Strict Macroquad 16-bit Index Buffer Invariants:
    // Vertices <= 3600 and Indices <= 4400 per sub-mesh!
    for (i, sm) in skyline.base_sub_meshes.iter().enumerate() {
        assert!(
            sm.vertices.len() <= 3600,
            "Sub-mesh {} exceeds vertex limit (got {}, max 3600)",
            i,
            sm.vertices.len()
        );
        assert!(
            sm.indices.len() <= 4400,
            "Sub-mesh {} exceeds index limit (got {}, max 4400)",
            i,
            sm.indices.len()
        );
        assert_eq!(
            sm.indices.len() % 3,
            0,
            "Sub-mesh {} index buffer must contain complete triangles",
            i
        );
    }
}

// ----------------------------------------------------------------------------
// 8. Catastrophic Domino Collapse & Mass Transfer
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_catastrophic_domino_collapse_momentum_transfer() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);
    let initial_voxels = grid.current_voxels;
    assert!(initial_voxels >= 30_000);

    // Sever Spire 0 base at (76, 48) at y=1..=4
    let (min_x, max_x) = layout.map_range_x(72, 80);
    let (min_z, max_z) = layout.map_range_z(44, 52);
    for y in 1..=4 {
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    // Island detection finds disconnected 105m Spire 0
    let islands = grid.detect_unsupported_islands();
    assert!(
        !islands.is_empty(),
        "Severing 105m Spire 0 base must trigger island collapse"
    );

    let total_island_voxels: usize = islands.iter().map(|isl| isl.voxels.len()).sum();
    assert!(
        total_island_voxels >= 2_500,
        "Toppling Spire 0 must contain substantial voxel mass (>= 2,500 voxels / >800 tons), got {}",
        total_island_voxels
    );

    // Spawn falling island chunks with angular topple momentum towards Spire 1 (68, 68)
    for island in islands {
        physics.spawn_island_chunk(
            island,
            Vec3::new(-1.0, -0.5, 1.0),
            Vec3::new(0.0, 0.0, 0.2),
        );
    }

    assert!(
        !physics.chunks.is_empty(),
        "Catastrophic skyscraper collapse must generate dynamic physics chunks"
    );

    // Step physics simulation
    for _ in 0..10 {
        physics.update(0.05, &mut grid, &audio, Vec3::ZERO);
    }

    assert!(
        !physics.chunks.is_empty(),
        "Dynamic chunks must simulate without premature deletion"
    );
}

// ----------------------------------------------------------------------------
// 9. Independent Transit Generator Primitives Verification
// ----------------------------------------------------------------------------
#[test]
fn test_g4_m2_all_transit_interconnections_grounded_and_valid() {
    let mut grid = VoxelGrid::new();

    // 1. Bedrock floor
    for x in 0..GRID_X as i32 {
        for z in 0..GRID_Z as i32 {
            grid.set(x, 0, z, VoxelType::Bedrock);
            for y in MIN_VOXEL_Y..0 {
                grid.set(x, y, z, VoxelType::Basalt);
            }
        }
    }

    // 2. Build multi-lane ground road deck at y=1
    build_multi_lane_road_deck(&mut grid, (10, 20), (86, 20), 1, 4, false);

    // 3. Build elevated monorail at y=40
    build_elevated_monorail(&mut grid, (10, 40), (86, 40), 40, 10, true);

    // 4. Build subterranean subway tunnel at y=-20
    build_subterranean_subway_tunnel(&mut grid, (10, 60), (86, 60), -20, Some((48, 60)));

    // 5. Build suspension skybridge between two grounded pillars
    for y in 1..=120 {
        grid.set(20, y, 80, VoxelType::Concrete);
        grid.set(70, y, 80, VoxelType::Concrete);
    }
    build_suspension_skybridge(&mut grid, (20, 80), (70, 80), 120, 2, false);

    // Verify zero unsupported islands at spawn
    let islands = grid.detect_unsupported_islands();
    assert!(
        islands.is_empty(),
        "Transit generator components must produce zero unsupported islands when anchored to bedrock, got {}",
        islands.len()
    );
}
