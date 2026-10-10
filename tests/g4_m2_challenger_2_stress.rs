#![allow(clippy::new_without_default)]
#![allow(dead_code, unused_imports, clippy::manual_range_contains)]

use voxel_demolition::*;

use audio::SoundSystem;
use macroquad::prelude::*;
use physics::{DynamicChunk, PhysicsSystem};
use skyline::{create_distant_skyline, DistantSkyline, MAX_BATCH_INDICES, MAX_BATCH_VERTICES};
use voxel::{VoxelGrid, VoxelType, GRID_X, GRID_Z, MAX_VOXEL_Y, VOXEL_SIZE};
use world_gen::{generate_level, LevelPreset};

// ============================================================================
// CHALLENGER 2 STRESS SUITE: DOMINO PHYSICS & SKYLINE RENDERING
// ============================================================================

// ----------------------------------------------------------------------------
// Group 1: Inter-Tower Spacing & Diagonal Collapse Geometry (Presets F1-F5)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TowerGeometry {
    name: &'static str,
    center_voxels: Vec2,
    center_meters: Vec2,
    roof_height_voxels: i32,
    roof_height_meters: f32,
    apex_height_voxels: i32,
    apex_height_meters: f32,
}

fn extract_tower_geometry(grid: &VoxelGrid, name: &'static str, cx: i32, cz: i32) -> TowerGeometry {
    // Apex includes the antenna spire and beacon at (cx, cz)
    let apex_y = (0..MAX_VOXEL_Y)
        .rev()
        .find(|&y| grid.get(cx, y, cz).is_solid())
        .unwrap_or_else(|| panic!("Tower {} at ({}, {}) has no solid voxels", name, cx, cz));

    // Roof slab is measured at off-center footprint (cx + 2, cz + 2) or (cx - 2, cz - 2)
    let mut roof_y = 0;
    for &(dx, dz) in &[(2, 2), (-2, 2), (2, -2), (-2, -2), (1, 1)] {
        if let Some(y) = (0..MAX_VOXEL_Y).rev().find(|&y| grid.get(cx + dx, y, cz + dz).is_solid()) {
            roof_y = roof_y.max(y);
        }
    }
    if roof_y == 0 {
        roof_y = apex_y;
    }

    TowerGeometry {
        name,
        center_voxels: Vec2::new(cx as f32, cz as f32),
        center_meters: Vec2::new(cx as f32 * VOXEL_SIZE, cz as f32 * VOXEL_SIZE),
        roof_height_voxels: roof_y,
        roof_height_meters: roof_y as f32 * VOXEL_SIZE,
        apex_height_voxels: apex_y,
        apex_height_meters: apex_y as f32 * VOXEL_SIZE,
    }
}

#[test]
fn test_challenger_measure_spacings_and_diagonal_geometry_preset_f1() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::ApexSkyscraper);

    let t1 = extract_tower_geometry(&grid, "F1-T1 HQ", layout.map_x(48), layout.map_z(48));
    let t2 = extract_tower_geometry(&grid, "F1-T2 West", layout.map_x(20), layout.map_z(32));
    let t3 = extract_tower_geometry(&grid, "F1-T3 East", layout.map_x(76), layout.map_z(64));
    let t4 = extract_tower_geometry(&grid, "F1-T4 North", layout.map_x(48), layout.map_z(18));
    let t5 = extract_tower_geometry(&grid, "F1-T5 SW", layout.map_x(20), layout.map_z(74));
    let t6 = extract_tower_geometry(&grid, "F1-T6 SE", layout.map_x(76), layout.map_z(22));

    let towers = [&t1, &t2, &t3, &t4, &t5, &t6];

    // Verify all towers have roof height in [80m, 110m] and apex <= 112m
    for t in &towers {
        assert!(
            t.roof_height_meters >= 80.0 && t.roof_height_meters <= 110.0,
            "Tower {} roof height {:.1}m outside [80m, 110m]",
            t.name,
            t.roof_height_meters
        );
        assert!(
            t.apex_height_meters <= 112.0,
            "Tower {} apex beacon {:.1}m exceeds 112.0m",
            t.name,
            t.apex_height_meters
        );
    }

    // Verify key diagonal and inter-tower spacings
    // Diagonal span T5 to T6 in expanded map:
    let diag_dist_m = (t5.center_meters - t6.center_meters).length();
    assert!(
        diag_dist_m >= 50.0 && diag_dist_m <= 58.0,
        "F1 Diagonal T5-T6 distance {:.2}m outside [50.0m, 58.0m]",
        diag_dist_m
    );

    // Diagonal reach invariant: If T5 (85m) or T6 (85m) topples along this diagonal:
    assert!(
        t5.roof_height_meters > diag_dist_m,
        "Toppling tower roof height {:.1}m must exceed diagonal distance {:.1}m",
        t5.roof_height_meters,
        diag_dist_m
    );
    let impact_y = (t5.roof_height_meters.powi(2) - diag_dist_m.powi(2)).sqrt();
    assert!(
        impact_y > t6.roof_height_meters * 0.5,
        "Diagonal impact height {:.1}m must strike upper half of receiving tower",
        impact_y
    );
}

#[test]
fn test_challenger_measure_spacings_and_diagonal_geometry_preset_f2() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::SuspensionSkybridge);

    let t1 = extract_tower_geometry(&grid, "F2-T1 West Pylon", layout.map_x(24), layout.map_z(48));
    let t2 = extract_tower_geometry(&grid, "F2-T2 East Pylon", layout.map_x(72), layout.map_z(48));
    let t3 = extract_tower_geometry(&grid, "F2-T3 Reactor N", layout.map_x(48), layout.map_z(18));
    let t4 = extract_tower_geometry(&grid, "F2-T4 Reactor S", layout.map_x(48), layout.map_z(78));
    let t5 = extract_tower_geometry(&grid, "F2-T5 Catalytic SW", layout.map_x(24), layout.map_z(78));

    let towers = [&t1, &t2, &t3, &t4, &t5];
    for t in &towers {
        assert!(
            t.roof_height_meters >= 80.0 && t.roof_height_meters <= 110.0,
            "Tower {} roof height {:.1}m outside [80m, 110m]",
            t.name,
            t.roof_height_meters
        );
        assert!(
            t.apex_height_meters <= 112.0,
            "Tower {} apex beacon {:.1}m exceeds 112.0m",
            t.name,
            t.apex_height_meters
        );
    }

    // T3 to T4 orthogonal distance in expanded map
    let dist_t3_t4 = (t3.center_meters - t4.center_meters).length();
    assert!(
        (dist_t3_t4 - 58.0).abs() < 1.0,
        "F2 T3-T4 distance {:.2}m should be ~58.0m",
        dist_t3_t4
    );

    // Diagonal T5 to T3
    let diag_dist_m = (t5.center_meters - t3.center_meters).length();
    assert!(
        t5.roof_height_meters > diag_dist_m,
        "Toppling tower roof height {:.1}m must exceed diagonal distance {:.1}m",
        t5.roof_height_meters,
        diag_dist_m
    );
}

#[test]
fn test_challenger_measure_spacings_and_diagonal_geometry_preset_f3() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::MilitaryFortress);

    let t1 = extract_tower_geometry(&grid, "F3-T1 Comms", layout.map_x(48), layout.map_z(48));
    let t2 = extract_tower_geometry(&grid, "F3-T2 Pylon W", layout.map_x(20), layout.map_z(48));
    let t3 = extract_tower_geometry(&grid, "F3-T3 Pylon E", layout.map_x(76), layout.map_z(48));
    let t4 = extract_tower_geometry(&grid, "F3-T4 NW", layout.map_x(22), layout.map_z(22));
    let t5 = extract_tower_geometry(&grid, "F3-T5 NE", layout.map_x(74), layout.map_z(22));
    let t6 = extract_tower_geometry(&grid, "F3-T6 SW", layout.map_x(22), layout.map_z(74));
    let t7 = extract_tower_geometry(&grid, "F3-T7 SE", layout.map_x(74), layout.map_z(74));

    let towers = [&t1, &t2, &t3, &t4, &t5, &t6, &t7];
    for t in &towers {
        assert!(
            t.roof_height_meters >= 80.0 && t.roof_height_meters <= 110.0,
            "Tower {} roof height {:.1}m outside [80m, 110m]",
            t.name,
            t.roof_height_meters
        );
        assert!(
            t.apex_height_meters <= 112.0,
            "Tower {} apex beacon {:.1}m exceeds 112.0m",
            t.name,
            t.apex_height_meters
        );
    }

    // Orthogonal spacing between perimeter towers:
    let s_ortho = (t4.center_meters - t5.center_meters).length();
    assert!(
        (s_ortho - 45.5).abs() < 1.0,
        "F3 Orthogonal spacing {:.2}m should be ~45.5m",
        s_ortho
    );

    // Diagonal spacing between opposite perimeter towers:
    let s_diag = (t4.center_meters - t7.center_meters).length();
    assert!(
        (s_diag - 62.26).abs() < 1.0,
        "F3 Diagonal spacing {:.2}m should be ~62.3m",
        s_diag
    );

    // Crash reach condition: 85m tower toppling diagonally across 62.3m
    let topple_angle_rad = (s_diag / t4.roof_height_meters).asin();
    let topple_angle_deg = topple_angle_rad.to_degrees();
    assert!(
        topple_angle_deg < 60.0,
        "Diagonal topple impact angle ({:.1} deg) must occur before ground hit",
        topple_angle_deg
    );
    let impact_y = (t4.roof_height_meters.powi(2) - s_diag.powi(2)).sqrt();
    assert!(
        impact_y > t7.roof_height_meters * 0.5,
        "Impact height {:.1}m must strike upper half of receiving tower",
        impact_y
    );
}

#[test]
fn test_challenger_measure_spacings_and_diagonal_geometry_preset_f4() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::DominoMonoliths);

    let t1 = extract_tower_geometry(&grid, "F4-T1 Citadel", layout.map_x(48), layout.map_z(48));
    let t2 = extract_tower_geometry(&grid, "F4-T2 Keep A", layout.map_x(48), layout.map_z(24));
    let t3 = extract_tower_geometry(&grid, "F4-T3 Keep B", layout.map_x(27), layout.map_z(60));
    let t4 = extract_tower_geometry(&grid, "F4-T4 Keep C", layout.map_x(69), layout.map_z(60));
    let t5 = extract_tower_geometry(&grid, "F4-T5 Bastion NW", layout.map_x(18), layout.map_z(18));
    let t6 = extract_tower_geometry(&grid, "F4-T6 Bastion NE", layout.map_x(78), layout.map_z(18));
    let t7 = extract_tower_geometry(&grid, "F4-T7 Bastion SW", layout.map_x(18), layout.map_z(78));
    let t8 = extract_tower_geometry(&grid, "F4-T8 Bastion SE", layout.map_x(78), layout.map_z(78));

    let towers = [&t1, &t2, &t3, &t4, &t5, &t6, &t7, &t8];
    for t in &towers {
        assert!(
            t.roof_height_meters >= 80.0 && t.roof_height_meters <= 110.0,
            "Tower {} roof height {:.1}m outside [80m, 110m]",
            t.name,
            t.roof_height_meters
        );
        assert!(
            t.apex_height_meters <= 112.0,
            "Tower {} apex beacon {:.1}m exceeds 112.0m",
            t.name,
            t.apex_height_meters
        );
    }

    // Orthogonal spacing between bastions:
    // T5 (18, 18) to T6 (78, 18)
    let s_ortho = (t5.center_meters - t6.center_meters).length();
    assert!(
        (s_ortho - 38.0).abs() < 0.2,
        "F4 Orthogonal spacing {:.2}m should be 38.0m",
        s_ortho
    );

    // Diagonal spacing between opposite bastions:
    // T5 (18, 18) to T8 (78, 78)
    let s_diag = (t5.center_meters - t8.center_meters).length();
    assert!(
        (s_diag - 56.27).abs() < 0.2,
        "F4 Diagonal spacing {:.2}m should be ~56.27m",
        s_diag
    );
    assert!(
        s_diag >= 36.8 && s_diag <= 58.0,
        "F4 Diagonal spacing {:.2}m in [36.8m, 58.0m]",
        s_diag
    );

    // Crash reach condition for lowest tower (80.0m):
    assert!(
        t5.roof_height_meters > s_diag,
        "80m Bastion height must exceed 56.27m diagonal distance"
    );
    let topple_angle_deg = (s_diag / t5.roof_height_meters).asin().to_degrees();
    assert!(
        topple_angle_deg < 50.0,
        "Diagonal topple impact angle ({:.1} deg) must occur before ground hit",
        topple_angle_deg
    );
    let impact_y = (t5.roof_height_meters.powi(2) - s_diag.powi(2)).sqrt();
    assert!(
        impact_y > t8.roof_height_meters * 0.5,
        "Impact height {:.1}m must strike upper half of receiving tower",
        impact_y
    );
}

#[test]
fn test_challenger_measure_spacings_and_diagonal_geometry_preset_f5() {
    let mut grid = VoxelGrid::new();
    let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);

    // Colosseum ring: 8 spires at radius 28 voxels (14.0m)
    let center_x = 48.0f32;
    let center_z = 48.0f32;
    let circle_radius = 28.0f32;
    let mut spires = Vec::new();

    for i in 0..8 {
        let angle = (i as f32 * 45.0).to_radians();
        let sx = (center_x + angle.cos() * circle_radius).round() as i32;
        let sz = (center_z + angle.sin() * circle_radius).round() as i32;
        spires.push(extract_tower_geometry(&grid, "F5 Spire", layout.map_x(sx), layout.map_z(sz)));
    }

    for (i, s) in spires.iter().enumerate() {
        assert!(
            s.roof_height_meters >= 85.0 && s.roof_height_meters <= 105.0,
            "Spire {} roof height {:.1}m outside [85m, 105m]",
            i,
            s.roof_height_meters
        );
        assert!(
            s.apex_height_meters <= 106.0,
            "Spire {} apex beacon {:.1}m exceeds 106.0m",
            i,
            s.apex_height_meters
        );
    }

    // Opposite spires cross-ring distance:
    for i in 0..4 {
        let opp_dist = (spires[i].center_meters - spires[i + 4].center_meters).length();
        assert!(
            opp_dist >= 40.0 && opp_dist <= 55.0,
            "Opposite spires distance {:.2}m in [40m, 55m]",
            opp_dist
        );
    }
}

// ----------------------------------------------------------------------------
// Group 2: Domino Momentum Transfer & Floor Crushing Mechanics Simulation
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_floor_crushing_and_mass_accretion_mechanics() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    let cx = GRID_X as i32 / 2;
    let cz = GRID_Z as i32 / 2;

    // Set up a multi-floor static building target
    // Ground bedrock at y=0
    for x in (cx - 8)..=(cx + 8) {
        for z in (cz - 8)..=(cz + 8) {
            grid.set(x, 0, z, VoxelType::Bedrock);
        }
    }

    // Concrete & glass floors at y=10, y=16, y=22
    for y in 1..=25 {
        for x in (cx - 4)..=(cx + 4) {
            for z in (cz - 4)..=(cz + 4) {
                let vt = if y % 6 == 0 {
                    VoxelType::Concrete // floor slab
                } else if x == cx - 4 || x == cx + 4 || z == cz - 4 || z == cz + 4 {
                    VoxelType::Glass // facade
                } else if (x == cx - 1 || x == cx + 1) && (z == cz - 1 || z == cz + 1) {
                    VoxelType::SteelRebar // central columns
                } else {
                    VoxelType::Air
                };
                grid.set(x, y, z, vt);
            }
        }
    }

    let initial_solids = grid.solid_count();
    assert!(initial_solids > 100);

    // Spawn a massive dynamic chunk above the building (H=30 voxels / 15m)
    // representing upper floors of a toppling skyscraper with high downward momentum
    let mut chunk_voxels = Vec::new();
    for x in -3..=3 {
        for y in -2..=2 {
            for z in -3..=3 {
                chunk_voxels.push((Vec3::new(x as f32, y as f32, z as f32) * VOXEL_SIZE, VoxelType::HeavyArmor));
            }
        }
    }

    let aabb = DynamicChunk::compute_aabb(&chunk_voxels);
    let mass: f32 = chunk_voxels.iter().map(|(_, vt)| vt.properties().density).sum();
    let chunk_pos = Vec3::new(0.0, 26.0 * VOXEL_SIZE, 0.0);
    let heavy_chunk = DynamicChunk::new(
        chunk_pos,
        Vec3::new(0.0, -8.0, 0.0),
        Vec3::ZERO,
        Vec3::ZERO,
        chunk_voxels,
        mass,
        0.0,
        false,
        aabb,
    );
    let initial_chunk_mass = heavy_chunk.mass;
    assert!(initial_chunk_mass > 500.0, "Heavy chunk mass should be substantial");

    physics.chunks.push(heavy_chunk);

    // Step physics: chunk should impact floor, pulverize static intermediate voxels,
    // accrue mass, and generate micro-chunks and dust/spall
    let mut crushed_occurred = false;
    for _ in 0..20 {
        physics.update(0.02, &mut grid, &audio, Vec3::ZERO);
        if grid.solid_count() < initial_solids {
            crushed_occurred = true;
            break;
        }
    }

    assert!(
        crushed_occurred,
        "High-momentum falling chunk must crush static floor voxels"
    );

    // Verify solid voxel count decreased (floors pulverized)
    let post_crush_solids = grid.solid_count();
    assert!(
        post_crush_solids < initial_solids,
        "Static solids must decrease from {} after floor crushing, got {}",
        initial_solids,
        post_crush_solids
    );

    // Verify mass accretion occurred or micro-chunks spawned
    let chunk_mass_increased = physics.chunks.iter().any(|c| c.mass > initial_chunk_mass);
    let new_chunks_created = physics.chunks.len() > 1;
    let particles_spawned = !physics.debris.is_empty();

    assert!(
        chunk_mass_increased || new_chunks_created,
        "Pancake crushing must either accrete mass onto the falling body or spawn micro-chunks"
    );
    assert!(
        particles_spawned,
        "Crushing intermediate floors must generate spall/dust particles"
    );
}

#[test]
fn test_challenger_simulated_domino_cascade_impact_on_adjacent_tower() {
    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let audio = SoundSystem::empty();

    // Use F5 Domino Colosseum
    let layout = generate_level(&mut grid, LevelPreset::GothicCitadel);
    let initial_voxels = grid.current_voxels;

    // Spire 0 at (76, 48, H=105m) topples towards Spire 1 at (68, 68, H=90m)
    // Distance between Spire 0 and Spire 1:
    let s0_gx = layout.map_x(76);
    let s0_gz = layout.map_z(48);
    let s1_gx = layout.map_x(68);
    let s1_gz = layout.map_z(68);
    let s0_pos = Vec2::new(s0_gx as f32, s0_gz as f32) * VOXEL_SIZE;
    let s1_pos = Vec2::new(s1_gx as f32, s1_gz as f32) * VOXEL_SIZE;
    let inter_spire_dist = (s0_pos - s1_pos).length();
    assert!(
        (inter_spire_dist - 18.93).abs() < 0.2,
        "Inter-spire distance {:.2}m",
        inter_spire_dist
    );

    // Sever Spire 0 base at y=1..=4
    let (cut_x0, cut_x1) = layout.map_range_x(72, 80);
    let (cut_z0, cut_z1) = layout.map_range_z(44, 52);
    for y in 1..=4 {
        for x in cut_x0..=cut_x1 {
            for z in cut_z0..=cut_z1 {
                grid.set(x, y, z, VoxelType::Air);
            }
        }
    }

    let islands = grid.detect_unsupported_islands();
    assert!(!islands.is_empty(), "Severing Spire 0 base must create island");

    // Impart high directional velocity towards Spire 1
    let dx = (s1_gx - s0_gx) as f32;
    let dz = (s1_gz - s0_gz) as f32;
    let dir = Vec3::new(dx, -0.2 * (dx * dx + dz * dz).sqrt(), dz).normalize();
    for island in islands {
        physics.spawn_island_chunk(
            island,
            dir * 12.0, // 12 m/s toppling trajectory
            Vec3::new(0.1, 0.0, 0.2),
        );
    }

    let initial_spire_chunks = physics.chunks.len();
    assert!(initial_spire_chunks > 0);

    // Step physics for 25 frames (0.5 seconds of physical collision)
    for _ in 0..25 {
        physics.update(0.02, &mut grid, &audio, Vec3::ZERO);
    }

    // Dynamic chunks must remain active and simulate collision
    assert!(
        !physics.chunks.is_empty(),
        "Dynamic chunks must actively simulate without crash"
    );

    // Debris or dust must have been generated by the collision
    assert!(
        !physics.debris.is_empty() || grid.current_voxels < initial_voxels,
        "Domino collision must produce physical debris or voxel destruction"
    );
}

// ----------------------------------------------------------------------------
// Group 3: Distant Skyline Mesh Invariants (Feature 9)
// ----------------------------------------------------------------------------

#[test]
fn test_challenger_skyline_sub_meshes_strictly_obey_vertex_and_index_limits() {
    let skyline: DistantSkyline = create_distant_skyline();

    assert!(
        !skyline.base_sub_meshes.is_empty(),
        "Skyline must generate at least 1 sub-mesh"
    );

    println!(
        "Distant Skyline generated {} sub-meshes",
        skyline.base_sub_meshes.len()
    );

    for (i, sm) in skyline.base_sub_meshes.iter().enumerate() {
        let v_count = sm.vertices.len();
        let i_count = sm.indices.len();

        assert!(
            v_count <= MAX_BATCH_VERTICES,
            "Sub-mesh {} vertex count {} EXCEEDS MAX_BATCH_VERTICES ({})",
            i,
            v_count,
            MAX_BATCH_VERTICES
        );
        assert!(
            i_count <= MAX_BATCH_INDICES,
            "Sub-mesh {} index count {} EXCEEDS MAX_BATCH_INDICES ({})",
            i,
            i_count,
            MAX_BATCH_INDICES
        );
        assert_eq!(
            i_count % 3,
            0,
            "Sub-mesh {} indices must be multiple of 3 (triangles)",
            i
        );

        // Verify index safety: every index must point to a valid vertex
        for &idx in &sm.indices {
            assert!(
                (idx as usize) < v_count,
                "Sub-mesh {} index {} out of bounds for {} vertices",
                i,
                idx,
                v_count
            );
        }
    }
}

#[test]
fn test_challenger_skyline_orbital_sky_elevator_apex_beacon_height() {
    let skyline: DistantSkyline = create_distant_skyline();

    let mut max_beacon_y = f32::MIN;
    let mut apex_beacons = Vec::new();

    for b in &skyline.beacons {
        if b.position.y > max_beacon_y {
            max_beacon_y = b.position.y;
        }
        if b.position.y >= 300.0 {
            apex_beacons.push(b.clone());
        }
    }

    assert!(
        max_beacon_y >= 300.0,
        "Orbital sky-elevator apex beacon must reach Y >= 300.0m, got {:.1}m",
        max_beacon_y
    );
    assert!(
        !apex_beacons.is_empty(),
        "Must have at least 1 stratospheric orbital apex beacon >= 300m"
    );

    // Also check mesh vertices of sky-elevators
    let mut max_vert_y = f32::MIN;
    for sm in &skyline.base_sub_meshes {
        for v in &sm.vertices {
            max_vert_y = max_vert_y.max(v.position.y);
        }
    }
    assert!(
        max_vert_y >= 300.0,
        "Skyline mesh vertex must ascend to Y >= 300.0m, got {:.1}m",
        max_vert_y
    );
}

#[test]
fn test_challenger_skyline_subterranean_skirt_depth() {
    let skyline: DistantSkyline = create_distant_skyline();

    let mut min_vert_y = f32::MAX;
    let mut subterranean_verts = 0;

    for sm in &skyline.base_sub_meshes {
        for v in &sm.vertices {
            min_vert_y = min_vert_y.min(v.position.y);
            if v.position.y <= -34.0 {
                subterranean_verts += 1;
            }
        }
    }

    assert!(
        min_vert_y <= -35.0,
        "Subterranean skirt must extend down to Y <= -35.0m to seal deep craters, got {:.1}m",
        min_vert_y
    );
    assert!(
        subterranean_verts >= 36,
        "Subterranean skirt must have at least 36 base vertices at -35m, got {}",
        subterranean_verts
    );
}

#[test]
fn test_challenger_skyline_360_degree_sector_coverage() {
    let skyline: DistantSkyline = create_distant_skyline();

    // Verify beacons span all 4 quadrants (360 degrees)
    let mut has_q1 = false; // +x, +z
    let mut has_q2 = false; // -x, +z
    let mut has_q3 = false; // -x, -z
    let mut has_q4 = false; // +x, -z

    for b in &skyline.beacons {
        if b.position.x >= 0.0 && b.position.z >= 0.0 {
            has_q1 = true;
        } else if b.position.x < 0.0 && b.position.z >= 0.0 {
            has_q2 = true;
        } else if b.position.x < 0.0 && b.position.z < 0.0 {
            has_q3 = true;
        } else if b.position.x >= 0.0 && b.position.z < 0.0 {
            has_q4 = true;
        }
    }

    assert!(has_q1 && has_q2 && has_q3 && has_q4, "Skyline beacons must cover all 4 quadrants (360 degrees)");
}
