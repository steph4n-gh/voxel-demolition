//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE 2: MILESTONE G6-M1
//! ADVERSARIAL STRESS-TESTING: DYNAMIC CHUNK FRACTURE, ANGULAR MOMENTUM
//! INHERITANCE, TANGENTIAL VELOCITY FLING & KINETIC ENERGY CONSERVATION
//! Target: tests/g6_m1_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(unused_imports, dead_code)]

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, Mat3, PhysicsSystem};
use voxel_demolition::voxel::{VoxelGrid, VoxelType, GRID_X, GRID_Z, VOXEL_SIZE};

/// Helper: construct a test chunk with a set of voxels and kinematic parameters.
fn create_test_chunk(
    voxels: Vec<(Vec3, VoxelType)>,
    position: Vec3,
    velocity: Vec3,
    rotation: Vec3,
    angular_velocity: Vec3,
) -> DynamicChunk {
    let mass: f32 = voxels.iter().map(|(_, vt)| vt.properties().density).sum();
    let aabb = DynamicChunk::compute_aabb(&voxels);
    DynamicChunk::new(
        position,
        velocity,
        rotation,
        angular_velocity,
        voxels,
        mass.max(1.0),
        0.0,
        false,
        aabb,
    )
}

/// Helper: build an elongated beam along a specified axis.
fn build_beam_voxels(axis: char, count: usize, spacing: f32, vtype: VoxelType) -> Vec<(Vec3, VoxelType)> {
    let mut voxels = Vec::with_capacity(count);
    let half = (count.saturating_sub(1)) as f32 / 2.0;
    for i in 0..count {
        let coord = (i as f32 - half) * spacing;
        let pos = match axis {
            'x' | 'X' => Vec3::new(coord, 0.0, 0.0),
            'y' | 'Y' => Vec3::new(0.0, coord, 0.0),
            _ => Vec3::new(0.0, 0.0, coord),
        };
        voxels.push((pos, vtype));
    }
    voxels
}

/// Helper: build a solid 3D box of voxels.
fn build_box_voxels(nx: usize, ny: usize, nz: usize, spacing: f32, vtype: VoxelType) -> Vec<(Vec3, VoxelType)> {
    let mut voxels = Vec::with_capacity(nx * ny * nz);
    let hx = (nx.saturating_sub(1)) as f32 / 2.0;
    let hy = (ny.saturating_sub(1)) as f32 / 2.0;
    let hz = (nz.saturating_sub(1)) as f32 / 2.0;
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let pos = Vec3::new(
                    (x as f32 - hx) * spacing,
                    (y as f32 - hy) * spacing,
                    (z as f32 - hz) * spacing,
                );
                voxels.push((pos, vtype));
            }
        }
    }
    voxels
}

// ============================================================================
// SUITE 1: DISPATCH SPECIFIED STRESS TEST (omega = [0, 8, 0], v = [15, -10, 0])
// ============================================================================

#[test]
fn test_dispatch_stress_rotating_chunk_fracture_kinematics() {
    // Dispatch requirements:
    // omega = (0, 8.0, 0), v = (15, -10, 0)
    let parent_pos = Vec3::new(30.0, 45.0, 30.0);
    let parent_vel = Vec3::new(15.0, -10.0, 0.0);
    let parent_ang = Vec3::new(0.0, 8.0, 0.0);

    // 160-voxel horizontal bar (major along X) so it splits into 4 sub-chunks
    let voxels = build_beam_voxels('x', 160, 0.15, VoxelType::Concrete);
    let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);
    let parent_mass = parent.mass;
    let parent_linear_ke = 0.5 * parent_mass * parent_vel.length_squared();
    let parent_rot_ke: f32 = parent.voxels.iter().map(|(p, vt)| {
        let dm = vt.properties().density;
        let v_rot = parent_ang.cross(*p);
        0.5 * dm * v_rot.length_squared()
    }).sum();
    let parent_total_ke = parent_linear_ke + parent_rot_ke;

    let (sub_chunks, _, _) = parent.fracture();

    assert_eq!(
        sub_chunks.len(),
        4,
        "Chunk with 160 voxels must subdivide into exactly 4 sub-chunks"
    );

    let mut total_sub_mass = 0.0f32;
    let mut total_sub_linear_ke = 0.0f32;
    let mut total_sub_rot_ke = 0.0f32;
    let mut com_vel_acc = Vec3::ZERO;

    for (idx, sub) in sub_chunks.iter().enumerate() {
        total_sub_mass += sub.mass;
        total_sub_linear_ke += 0.5 * sub.mass * sub.velocity.length_squared();
        let sub_rot: f32 = sub.voxels.iter().map(|(p, vt)| {
            let dm = vt.properties().density;
            let v_rot = sub.angular_velocity.cross(*p);
            0.5 * dm * v_rot.length_squared()
        }).sum();
        total_sub_rot_ke += sub_rot;
        com_vel_acc += sub.velocity * sub.mass;

        // 1. Angular velocity inheritance:
        // Must inherit parent's +Y spin (8.0 rad/s) + scatter in [-1.5, 1.5]
        assert!(
            (6.5..=9.5).contains(&sub.angular_velocity.y),
            "Sub-chunk {} angular_velocity.y must inherit parent spin in [6.5, 9.5] (got {})",
            idx,
            sub.angular_velocity.y
        );
        assert!(
            sub.angular_velocity.x.abs() <= 1.6 && sub.angular_velocity.z.abs() <= 1.6,
            "Off-axis angular scatter must be within [-1.5, 1.5] (got x={}, z={})",
            sub.angular_velocity.x,
            sub.angular_velocity.z
        );

        // 2. Downward velocity must NOT be destroyed:
        assert!(
            sub.velocity.y < -5.0,
            "Sub-chunk downward velocity must be preserved (got vy = {})",
            sub.velocity.y
        );

        // 3. Tangential velocity flings sub-chunks outwards:
        // r_offset is primarily along X: r = (rx, ry, rz)
        // omega x r = (0, 8, 0) x (rx, ry, rz) = (-8 * rz, 0, 8 * rx)
        // For sub-chunks with rx > 0, tangential velocity vz must be positive (+8 * rx)
        // For sub-chunks with rx < 0, tangential velocity vz must be negative (-8 * |rx|)
        let r_offset = sub.position - parent_pos;
        let expected_tangential = parent_ang.cross(r_offset);
        let actual_rel_vel = sub.velocity - parent_vel;
        let scatter_part = actual_rel_vel - expected_tangential;

        assert!(
            scatter_part.length() <= 4.0,
            "Relative velocity minus tangential velocity must equal scatter (magnitude <= 4.0, got {})",
            scatter_part.length()
        );

        if r_offset.x.abs() > 1.0 {
            // By right-hand rule: (0, wy, 0) x (rx, 0, 0) = (0, 0, -wy * rx)
            // So expected_tangential.z and r_offset.x have OPPOSITE signs!
            assert!(
                (expected_tangential.z * r_offset.x) < 0.0,
                "Expected tangential vz must be opposite sign of rx: rx={}, vz={}",
                r_offset.x,
                expected_tangential.z
            );
            assert!(
                (actual_rel_vel.z * r_offset.x) < 0.0,
                "Actual relative velocity vz must match tangential fling direction: rx={}, vz={}",
                r_offset.x,
                actual_rel_vel.z
            );
        }
    }

    // 4. Exact mass conservation
    assert!(
        (total_sub_mass - parent_mass).abs() < 1e-3,
        "Total mass must be conserved: parent={}, sub_sum={}",
        parent_mass,
        total_sub_mass
    );

    // 5. Total kinetic energy must be physically sensible (no vanishing KE, conserved total KE)
    // (a) Sub-chunk linear KE must NOT vanish (must not collapse like old 0.35 bug):
    assert!(
        total_sub_linear_ke >= parent_linear_ke * 0.85,
        "Linear kinetic energy must not vanish! parent_linear_ke = {}, sub_linear_ke = {}",
        parent_linear_ke,
        total_sub_linear_ke
    );
    // (b) Total kinetic energy (linear + rotational) before vs after fracture must be conserved within scatter impulse bounds:
    let total_sub_ke = total_sub_linear_ke + total_sub_rot_ke;
    let total_ke_ratio = total_sub_ke / parent_total_ke;
    assert!(
        (0.90..=1.25).contains(&total_ke_ratio),
        "Total kinetic energy must be physically conserved! ratio = {} (parent = {}, sub = {})",
        total_ke_ratio,
        parent_total_ke,
        total_sub_ke
    );

    // 6. Center of mass velocity shift bounded by scatter impulse
    let com_vel = com_vel_acc / total_sub_mass;
    let vel_drift = (com_vel - parent_vel).length();
    assert!(
        vel_drift <= 3.8,
        "Center of mass velocity drift must be bounded by scatter speed (got drift = {})",
        vel_drift
    );
}

// ============================================================================
// SUITE 2: TANGENTIAL VELOCITY RADIUS-DEPENDENCE & RADIAL GRADIENT
// ============================================================================

#[test]
fn test_tangential_velocity_radius_dependence_proportionality() {
    // Sub-chunks further from rotation axis must experience strictly higher tangential fling
    let parent_pos = Vec3::new(0.0, 50.0, 0.0);
    let parent_vel = Vec3::ZERO;
    let omega_y = 12.0f32;
    let parent_ang = Vec3::new(0.0, omega_y, 0.0);

    // Very long beam along X (200 voxels, 20m span)
    let voxels = build_beam_voxels('x', 200, 0.10, VoxelType::CompositeRebar);
    let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);

    let (sub_chunks, _, _) = parent.fracture();
    assert_eq!(sub_chunks.len(), 4);

    // Sort sub-chunks by radial distance from center
    let mut sorted_by_radius = sub_chunks;
    sorted_by_radius.sort_by(|a, b| {
        let ra = (a.position - parent_pos).length();
        let rb = (b.position - parent_pos).length();
        ra.partial_cmp(&rb).unwrap()
    });

    let inner_chunk_1 = &sorted_by_radius[0];
    let inner_chunk_2 = &sorted_by_radius[1];
    let outer_chunk_1 = &sorted_by_radius[2];
    let outer_chunk_2 = &sorted_by_radius[3];

    let r_inner = ((inner_chunk_1.position - parent_pos).length() + (inner_chunk_2.position - parent_pos).length()) * 0.5;
    let r_outer = ((outer_chunk_1.position - parent_pos).length() + (outer_chunk_2.position - parent_pos).length()) * 0.5;

    assert!(
        r_outer > r_inner * 2.0,
        "Outer chunks must have at least 2x larger radial distance: r_outer={}, r_inner={}",
        r_outer,
        r_inner
    );

    // Theoretical tangential velocity: v_t = omega * r
    let expected_vt_inner = omega_y * r_inner;
    let expected_vt_outer = omega_y * r_outer;

    let vt_inner_actual = (inner_chunk_1.velocity.z.abs() + inner_chunk_2.velocity.z.abs()) * 0.5;
    let vt_outer_actual = (outer_chunk_1.velocity.z.abs() + outer_chunk_2.velocity.z.abs()) * 0.5;

    // Radius-dependent speed differential assertion:
    assert!(
        vt_outer_actual > vt_inner_actual * 1.8,
        "Outer sub-chunks must have significantly higher tangential velocity than inner (outer={}, inner={})",
        vt_outer_actual,
        vt_inner_actual
    );

    // Check accuracy against v = omega * r (allowing for scatter in [-3.5, 3.5])
    assert!(
        (vt_inner_actual - expected_vt_inner).abs() <= 4.0,
        "Inner tangential speed must match omega * r (actual={}, expected={})",
        vt_inner_actual,
        expected_vt_inner
    );
    assert!(
        (vt_outer_actual - expected_vt_outer).abs() <= 4.0,
        "Outer tangential speed must match omega * r (actual={}, expected={})",
        vt_outer_actual,
        expected_vt_outer
    );
}

// ============================================================================
// SUITE 3: 3D MULTI-AXIS ANGULAR VELOCITY VECTOR INHERITANCE
// ============================================================================

#[test]
fn test_3d_multi_axis_angular_velocity_inheritance_all_axes() {
    // Test that spin along all 3 axes is faithfully inherited
    let parent_pos = Vec3::new(12.0, 24.0, -15.0);
    let parent_vel = Vec3::new(-14.0, 22.0, -8.0);
    let parent_ang = Vec3::new(7.5, -9.0, 14.0);

    let voxels = build_box_voxels(6, 6, 6, 0.20, VoxelType::SteelRebar); // 216 voxels
    let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);

    let (sub_chunks, _, _) = parent.fracture();
    assert_eq!(sub_chunks.len(), 4);

    for (i, sub) in sub_chunks.iter().enumerate() {
        let ang_diff = sub.angular_velocity - parent_ang;
        assert!(
            ang_diff.x.abs() <= 1.6,
            "Chunk {} angular_velocity.x diff exceeds scatter limit: got {}, expected ~7.5",
            i,
            sub.angular_velocity.x
        );
        assert!(
            ang_diff.y.abs() <= 1.6,
            "Chunk {} angular_velocity.y diff exceeds scatter limit: got {}, expected ~-9.0",
            i,
            sub.angular_velocity.y
        );
        assert!(
            ang_diff.z.abs() <= 1.6,
            "Chunk {} angular_velocity.z diff exceeds scatter limit: got {}, expected ~14.0",
            i,
            sub.angular_velocity.z
        );

        // Verify angular velocity vector dot product with parent is strongly positive
        let dot = sub.angular_velocity.dot(parent_ang);
        assert!(
            dot > 0.0,
            "Sub-chunk angular velocity must remain aligned with parent spin vector (dot = {})",
            dot
        );

        // 3D cross product tangential check
        let r_offset = sub.position - parent_pos;
        let expected_tangential = parent_ang.cross(r_offset);
        let actual_rel_vel = sub.velocity - parent_vel;
        let diff = (actual_rel_vel - expected_tangential).length();
        assert!(
            diff <= 4.0,
            "3D Tangential velocity must match omega.cross(r) within scatter bounds (diff = {})",
            diff
        );
    }
}

// ============================================================================
// SUITE 4: LINEAR MOMENTUM & MASS CONSERVATION ACROSS GEOMETRIES
// ============================================================================

#[test]
fn test_linear_momentum_and_mass_conservation_diverse_geometries() {
    let geometries = vec![
        ("Rod_X", build_beam_voxels('x', 140, 0.15, VoxelType::Concrete)),
        ("Rod_Y", build_beam_voxels('y', 140, 0.15, VoxelType::Concrete)),
        ("Rod_Z", build_beam_voxels('z', 140, 0.15, VoxelType::Concrete)),
        ("Flat_Plate", build_box_voxels(12, 1, 12, 0.20, VoxelType::CompositeRebar)),
        ("Solid_Cube", build_box_voxels(5, 5, 5, 0.25, VoxelType::HeavyArmor)),
    ];

    let parent_pos = Vec3::new(10.0, 20.0, 10.0);
    let parent_vel = Vec3::new(18.0, -12.0, 6.0);
    let parent_ang = Vec3::new(2.0, 6.0, -3.0);

    for (name, voxels) in geometries {
        let initial_count = voxels.len();
        let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);
        let parent_mass = parent.mass;
        let (sub_chunks, _, _) = parent.fracture();

        // 1. Total voxel count conservation (zero voxel loss or creation)
        let total_sub_voxels: usize = sub_chunks.iter().map(|s| s.voxels.len()).sum();
        assert_eq!(
            total_sub_voxels, initial_count,
            "Geometry '{}': voxel count must be strictly conserved (got {}, expected {})",
            name, total_sub_voxels, initial_count
        );

        // 2. Total mass conservation (within f32 summation tolerance)
        let total_sub_mass: f32 = sub_chunks.iter().map(|s| s.mass).sum();
        let mass_err = (total_sub_mass - parent_mass).abs();
        assert!(
            mass_err < 0.05,
            "Geometry '{}': total mass must be conserved (parent={}, sub_sum={}, err={})",
            name, parent_mass, total_sub_mass, mass_err
        );

        // 3. Center of mass position conservation
        let mut com_pos_acc = Vec3::ZERO;
        let mut com_vel_acc = Vec3::ZERO;
        for sub in &sub_chunks {
            com_pos_acc += sub.position * sub.mass;
            com_vel_acc += sub.velocity * sub.mass;

            // Ensure valid AABBs
            assert!(
                sub.local_aabb.0.x <= sub.local_aabb.1.x
                    && sub.local_aabb.0.y <= sub.local_aabb.1.y
                    && sub.local_aabb.0.z <= sub.local_aabb.1.z,
                "Geometry '{}': sub-chunk local AABB must be non-inverted",
                name
            );
        }
        let com_pos = com_pos_acc / total_sub_mass;
        assert!(
            (com_pos - parent_pos).length() < 0.15,
            "Geometry '{}': center of mass position must be conserved (drift = {})",
            name,
            (com_pos - parent_pos).length()
        );

        // 4. Center of mass linear velocity conservation
        let com_vel = com_vel_acc / total_sub_mass;
        let vel_drift = (com_vel - parent_vel).length();
        assert!(
            vel_drift <= 3.8,
            "Geometry '{}': COM velocity drift exceeds scatter impulse bound: {}",
            name,
            vel_drift
        );
    }
}

// ============================================================================
// SUITE 5: KINETIC ENERGY BOUNDS ACROSS VELOCITY & SPIN REGIMES
// ============================================================================

#[test]
fn test_kinetic_energy_bounds_no_vanishing_or_explosion() {
    let test_cases = vec![
        ("Low_Speed_Low_Spin", Vec3::new(4.0, -3.0, 2.0), Vec3::new(0.0, 2.0, 0.0)),
        ("High_Speed_Zero_Spin", Vec3::new(25.0, -40.0, 10.0), Vec3::ZERO),
        ("High_Speed_High_Spin", Vec3::new(20.0, -30.0, -15.0), Vec3::new(0.0, 15.0, 0.0)),
        ("Extreme_Spin", Vec3::new(5.0, -5.0, 0.0), Vec3::new(0.0, 30.0, 0.0)),
    ];

    for (label, vel, ang) in test_cases {
        let voxels = build_box_voxels(5, 5, 5, 0.20, VoxelType::Concrete);
        let parent = create_test_chunk(voxels, Vec3::new(10.0, 30.0, 10.0), vel, Vec3::ZERO, ang);
        let parent_mass = parent.mass;
        let parent_lin_ke = 0.5 * parent_mass * vel.length_squared();

        let (sub_chunks, _, _) = parent.fracture();
        let total_sub_lin_ke: f32 = sub_chunks.iter().map(|s| 0.5 * s.mass * s.velocity.length_squared()).sum();

        if parent_lin_ke > 100.0 {
            let ke_ratio = total_sub_lin_ke / parent_lin_ke;
            // No vanishing kinetic energy! (Old bug had ratio <= 0.13 due to 0.35x velocity scaling)
            assert!(
                ke_ratio >= 0.70,
                "Case '{}': Linear KE vanished! ratio = {} (parent = {}, sub = {})",
                label,
                ke_ratio,
                parent_lin_ke,
                total_sub_lin_ke
            );
            // No unphysical explosion!
            assert!(
                ke_ratio <= 3.5,
                "Case '{}': Linear KE exploded! ratio = {} (parent = {}, sub = {})",
                label,
                ke_ratio,
                parent_lin_ke,
                total_sub_lin_ke
            );
        } else {
            // Even at near zero speed, sub chunks have positive kinetic energy from scatter
            assert!(
                total_sub_lin_ke > 0.0,
                "Case '{}': Sub chunks must have positive KE",
                label
            );
        }
    }
}

// ============================================================================
// SUITE 6: THRESHOLD BOUNDARY CHUNK SIZES & SUBDIVISION COUNTS
// ============================================================================

#[test]
fn test_threshold_boundary_chunk_sizes() {
    // Threshold 1: <= 40 voxels does NOT fracture; settles chunk immediately
    let voxels_40 = build_beam_voxels('x', 40, 0.2, VoxelType::Concrete);
    let chunk_40 = create_test_chunk(voxels_40, Vec3::new(0.0, 10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let (res_40, _, _) = chunk_40.fracture();
    assert_eq!(res_40.len(), 1, "Chunk with <= 40 voxels must return exactly 1 chunk");
    assert!(res_40[0].settled, "Chunk with <= 40 voxels must be settled");
    assert_eq!(res_40[0].velocity, Vec3::ZERO, "Settled chunk velocity must be zeroed");
    assert_eq!(res_40[0].angular_velocity, Vec3::ZERO, "Settled chunk angular velocity must be zeroed");

    // Threshold 2: 41 voxels (between 41 and 119) fractures into 2 pieces
    let voxels_41 = build_beam_voxels('x', 41, 0.2, VoxelType::Concrete);
    let chunk_41 = create_test_chunk(voxels_41, Vec3::new(0.0, 10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let (res_41, _, _) = chunk_41.fracture();
    assert_eq!(res_41.len(), 2, "Chunk with 41 voxels must fracture into exactly 2 pieces");
    assert!(!res_41[0].settled && !res_41[1].settled, "Sub-chunks must be dynamic and unsettled");

    // Threshold 3: 119 voxels fractures into 2 pieces
    let voxels_119 = build_beam_voxels('x', 119, 0.1, VoxelType::Concrete);
    let chunk_119 = create_test_chunk(voxels_119, Vec3::new(0.0, 10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let (res_119, _, _) = chunk_119.fracture();
    assert_eq!(res_119.len(), 2, "Chunk with 119 voxels must fracture into exactly 2 pieces");

    // Threshold 4: 120 voxels fractures into 4 pieces
    let voxels_120 = build_beam_voxels('x', 120, 0.1, VoxelType::Concrete);
    let chunk_120 = create_test_chunk(voxels_120, Vec3::new(0.0, 10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let (res_120, _, _) = chunk_120.fracture();
    assert_eq!(res_120.len(), 4, "Chunk with 120 voxels must fracture into exactly 4 pieces");

    // Threshold 5: 300 voxels fractures into 4 pieces
    let voxels_300 = build_box_voxels(10, 3, 10, 0.15, VoxelType::Concrete);
    let chunk_300 = create_test_chunk(voxels_300, Vec3::new(0.0, 10.0, 0.0), Vec3::new(10.0, -10.0, 0.0), Vec3::ZERO, Vec3::new(0.0, 5.0, 0.0));
    let (res_300, _, _) = chunk_300.fracture();
    assert_eq!(res_300.len(), 4, "Chunk with 300 voxels must fracture into exactly 4 pieces");
}

// ============================================================================
// SUITE 7: SPECIAL VOXEL TRANSFORMATION & EXTRACTION ON FRACTURE
// ============================================================================

#[test]
fn test_explosive_and_glowcore_world_transformation_extraction() {
    let mut voxels = Vec::new();
    // 50 concrete base
    for i in 0..50 {
        voxels.push((Vec3::new(i as f32 * 0.1, 0.0, 0.0), VoxelType::Concrete));
    }
    // 2 Explosive voxels
    voxels.push((Vec3::new(1.0, 0.5, 0.0), VoxelType::Explosive));
    voxels.push((Vec3::new(2.0, 0.5, 0.0), VoxelType::Explosive));
    // 1 GlowCore voxel
    voxels.push((Vec3::new(1.5, -0.5, 0.0), VoxelType::GlowCore));

    let parent_pos = Vec3::new(100.0, 50.0, -40.0);
    let rot = Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0); // 90 deg rotation around Y
    let parent = create_test_chunk(
        voxels,
        parent_pos,
        Vec3::new(5.0, -10.0, 0.0),
        rot,
        Vec3::new(0.0, 4.0, 0.0),
    );

    let rot_mat = Mat3::from_euler(rot);
    let expected_exp_0 = parent_pos + rot_mat.transform_vector(Vec3::new(1.0, 0.5, 0.0));
    let expected_exp_1 = parent_pos + rot_mat.transform_vector(Vec3::new(2.0, 0.5, 0.0));
    let expected_core = parent_pos + rot_mat.transform_vector(Vec3::new(1.5, -0.5, 0.0));

    let (_, exp_world, core_world) = parent.fracture();

    assert_eq!(exp_world.len(), 2, "Must extract exactly 2 explosive world points");
    assert_eq!(core_world.len(), 1, "Must extract exactly 1 glow core world point");

    assert!(
        (exp_world[0] - expected_exp_0).length() < 1e-3 || (exp_world[1] - expected_exp_0).length() < 1e-3,
        "Explosive point 0 must match transformed world position"
    );
    assert!(
        (exp_world[0] - expected_exp_1).length() < 1e-3 || (exp_world[1] - expected_exp_1).length() < 1e-3,
        "Explosive point 1 must match transformed world position"
    );
    assert!(
        (core_world[0] - expected_core).length() < 1e-3,
        "GlowCore point must match transformed world position"
    );
}

// ============================================================================
// SUITE 8: IN-ENGINE ROTATING IMPACT FRACTURE SIMULATION & STABILITY
// ============================================================================

#[test]
fn test_in_engine_rotating_high_speed_impact_fracture_simulation() {
    let mut phys = PhysicsSystem::new();
    let mut grid = VoxelGrid::new();
    let audio = SoundSystem::empty();

    // 1. Create a sturdy bedrock foundation at y = 0
    for x in 20..=40 {
        for z in 20..=40 {
            grid.set(x, 0, z, VoxelType::Bedrock);
        }
    }
    let floor_world = grid.voxel_to_world(IVec3::new(30, 0, 30));

    // 2. Spawn a rapidly rotating chunk descending towards the bedrock floor
    let voxels = build_box_voxels(6, 4, 6, 0.25, VoxelType::Concrete); // 144 voxels -> will fracture into 4 pieces
    let start_pos = floor_world + Vec3::new(0.0, 0.40, 0.0);
    let start_vel = Vec3::new(12.0, -15.0, 0.0);
    let start_ang = Vec3::new(0.0, 8.0, 0.0);

    let parent_chunk = create_test_chunk(voxels, start_pos, start_vel, Vec3::ZERO, start_ang);
    phys.chunks.push(parent_chunk);

    assert_eq!(phys.chunks.len(), 1);

    // 3. Step physics simulation: chunk strikes ground and triggers fracture
    let camera_pos = Vec3::new(30.0, 10.0, 50.0);
    phys.update(0.032, &mut grid, &audio, camera_pos);

    // 4. Assert parent chunk fractured into 4 dynamic sub-chunks
    assert_eq!(
        phys.chunks.len(),
        4,
        "High speed impact must fracture 144-voxel chunk into 4 sub-chunks in physics system"
    );

    // 5. Verify all sub-chunks possess valid velocities and inherited angular velocities
    for (i, sub) in phys.chunks.iter().enumerate() {
        assert!(
            !sub.settled,
            "Newly fractured sub-chunk {} must remain dynamic",
            i
        );
        assert!(
            sub.velocity.length() > 0.0,
            "Sub-chunk {} velocity must be non-zero",
            i
        );
        assert!(
            !sub.velocity.is_nan(),
            "Sub-chunk {} velocity must not be NaN",
            i
        );
        assert!(
            !sub.position.is_nan(),
            "Sub-chunk {} position must not be NaN",
            i
        );
    }

    // 6. Simulate 40 subsequent frames to verify numerical stability and tumbling
    for frame in 1..=40 {
        phys.update(0.032, &mut grid, &audio, camera_pos);
        for (i, sub) in phys.chunks.iter().enumerate() {
            assert!(
                !sub.position.is_nan(),
                "Frame {}: Sub-chunk {} position became NaN",
                frame,
                i
            );
            assert!(
                !sub.velocity.is_nan(),
                "Frame {}: Sub-chunk {} velocity became NaN",
                frame,
                i
            );
            assert!(
                sub.velocity.length() < 100.0,
                "Frame {}: Sub-chunk {} velocity exploded: {}",
                frame,
                i,
                sub.velocity.length()
            );
        }
    }
}

// ============================================================================
// SUITE 9: EXTREME ROTATIONAL SPEED STRESS (omega = 50.0 rad/s)
// ============================================================================

#[test]
fn test_extreme_angular_velocity_stress_50_rad_per_sec() {
    // 50 rad/s is approx 477 RPM
    let parent_pos = Vec3::new(0.0, 100.0, 0.0);
    let parent_vel = Vec3::new(0.0, -10.0, 0.0);
    let parent_ang = Vec3::new(0.0, 50.0, 0.0);

    let voxels = build_beam_voxels('x', 140, 0.15, VoxelType::Concrete);
    let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);

    let (sub_chunks, _, _) = parent.fracture();
    assert_eq!(sub_chunks.len(), 4);

    for (i, sub) in sub_chunks.iter().enumerate() {
        assert!(
            !sub.position.is_nan() && !sub.velocity.is_nan() && !sub.angular_velocity.is_nan(),
            "Extreme spin sub-chunk {} contains NaN",
            i
        );
        assert!(
            (48.0..=52.0).contains(&sub.angular_velocity.y),
            "Extreme spin sub-chunk {} must inherit ~50 rad/s angular velocity (got {})",
            i,
            sub.angular_velocity.y
        );

        let r_offset = sub.position - parent_pos;
        let expected_tangential = parent_ang.cross(r_offset);
        let actual_rel_z = sub.velocity.z - parent_vel.z;

        if r_offset.x.abs() > 1.0 {
            // Tangential speed can be 50 * 5m = 250 m/s!
            assert!(
                (actual_rel_z * expected_tangential.z) > 0.0,
                "Extreme tangential velocity must have consistent sign with omega x r: rx={}, exp_z={}, act_z={}",
                r_offset.x,
                expected_tangential.z,
                actual_rel_z
            );
        }
    }
}

// ============================================================================
// SUITE 10: ADVERSARIAL MONTE CARLO RANDOM ROTATION & GEOMETRY GENERATOR
// ============================================================================

#[test]
fn test_adversarial_monte_carlo_fracture_stress_100_random_rotations_and_shapes() {
    // 100 randomized variations with mixed materials, shapes, and extreme spins
    let materials = [
        VoxelType::Concrete,
        VoxelType::CompositeRebar,
        VoxelType::HeavyArmor,
        VoxelType::Brick,
        VoxelType::Glass,
    ];

    for iter in 0..100 {
        // Pseudo-random generation from iteration index
        let shape_type = iter % 3;
        let count = 45 + (iter * 17) % 200; // between 45 and 244
        let mat = materials[iter % materials.len()];

        let voxels = match shape_type {
            0 => build_beam_voxels('x', count, 0.15, mat),
            1 => build_beam_voxels('y', count, 0.15, mat),
            _ => {
                let n = ((count as f32).cbrt().ceil() as usize).max(2);
                build_box_voxels(n, n, n, 0.20, mat)
            }
        };
        let initial_voxel_count = voxels.len();

        let vx = ((iter as f32 * 7.3).sin() * 40.0).clamp(-40.0, 40.0);
        let vy = -((iter as f32 * 5.1).cos().abs() * 25.0 + 5.0);
        let vz = ((iter as f32 * 11.7).sin() * 40.0).clamp(-40.0, 40.0);
        let parent_vel = Vec3::new(vx, vy, vz);

        let wx = ((iter as f32 * 3.7).sin() * 20.0).clamp(-20.0, 20.0);
        let wy = ((iter as f32 * 8.9).cos() * 20.0).clamp(-20.0, 20.0);
        let wz = ((iter as f32 * 6.1).sin() * 20.0).clamp(-20.0, 20.0);
        let parent_ang = Vec3::new(wx, wy, wz);

        let px = (iter as f32 * 1.5) - 50.0;
        let py = 30.0 + (iter as f32 * 0.4);
        let pz = (iter as f32 * 2.1) - 60.0;
        let parent_pos = Vec3::new(px, py, pz);

        let parent = create_test_chunk(voxels, parent_pos, parent_vel, Vec3::ZERO, parent_ang);
        let parent_mass = parent.mass;

        let (sub_chunks, _, _) = parent.fracture();

        // 1. Piece count invariant
        let expected_pieces = if initial_voxel_count >= 120 { 4 } else { 2 };
        assert_eq!(
            sub_chunks.len(),
            expected_pieces,
            "Iter {}: Chunk with {} voxels must produce {} pieces (got {})",
            iter, initial_voxel_count, expected_pieces, sub_chunks.len()
        );

        // 2. Voxel conservation invariant
        let sub_voxels: usize = sub_chunks.iter().map(|s| s.voxels.len()).sum();
        assert_eq!(
            sub_voxels, initial_voxel_count,
            "Iter {}: Voxel conservation failed: initial {}, sub_total {}",
            iter, initial_voxel_count, sub_voxels
        );

        // 3. Mass conservation invariant
        let total_sub_mass: f32 = sub_chunks.iter().map(|s| s.mass).sum();
        assert!(
            (total_sub_mass - parent_mass).abs() < 0.1,
            "Iter {}: Mass conservation failed: parent {}, sub_sum {}",
            iter, parent_mass, total_sub_mass
        );

        // 4. Center of mass position & linear velocity checks
        let mut com_vel_acc = Vec3::ZERO;
        let mut com_pos_acc = Vec3::ZERO;
        for sub in &sub_chunks {
            com_vel_acc += sub.velocity * sub.mass;
            com_pos_acc += sub.position * sub.mass;

            // Angular velocity alignment
            let ang_diff = (sub.angular_velocity - parent_ang).length();
            assert!(
                ang_diff <= 2.8,
                "Iter {}: Angular velocity scatter exceeded 2.8 rad/s: {}",
                iter,
                ang_diff
            );

            // Tangential velocity match
            let r_offset = sub.position - parent_pos;
            let expected_tangential = parent_ang.cross(r_offset);
            let actual_rel = sub.velocity - parent_vel;
            let scatter_mag = (actual_rel - expected_tangential).length();
            assert!(
                scatter_mag <= 4.0,
                "Iter {}: Scatter magnitude exceeded 4.0 m/s: {}",
                iter,
                scatter_mag
            );
        }

        let com_pos = com_pos_acc / total_sub_mass;
        assert!(
            (com_pos - parent_pos).length() < 0.25,
            "Iter {}: Center of mass drifted: {}",
            iter,
            (com_pos - parent_pos).length()
        );

        let com_vel = com_vel_acc / total_sub_mass;
        assert!(
            (com_vel - parent_vel).length() <= 3.8,
            "Iter {}: COM velocity drift exceeded scatter: {}",
            iter,
            (com_vel - parent_vel).length()
        );
    }
}

// ============================================================================
// SUITE 11: ADVERSARIAL MULTI-GENERATION CASCADING FRACTURE DECAY
// ============================================================================

#[test]
fn test_adversarial_cascading_multi_generation_fracture_to_settled_debris() {
    // Start with a large 400-voxel monolith.
    // Gen 0: 1 chunk (400 voxels)
    // Gen 1: 4 chunks (~100 voxels each, dynamic)
    // Gen 2: each ~100-voxel chunk splits into 2 chunks -> 8 chunks (~50 voxels each, dynamic)
    // Gen 3: each ~50-voxel chunk splits into 2 chunks -> 16 chunks (~25 voxels each, dynamic)
    // Gen 4: each ~25-voxel chunk has <= 40 voxels -> settles immediately! (16 settled chunks)
    let initial_count = 400;
    let voxels = build_beam_voxels('x', initial_count, 0.10, VoxelType::Concrete);
    let parent = create_test_chunk(
        voxels,
        Vec3::new(20.0, 50.0, 20.0),
        Vec3::new(10.0, -15.0, 0.0),
        Vec3::ZERO,
        Vec3::new(0.0, 6.0, 0.0),
    );
    let initial_mass = parent.mass;

    // Gen 1
    let (gen_1, _, _) = parent.fracture();
    assert_eq!(gen_1.len(), 4, "Gen 1: 400 voxels must split into 4 chunks");
    for c in &gen_1 {
        assert!(!c.settled);
    }

    // Gen 2
    let mut gen_2 = Vec::new();
    for c in gen_1 {
        let (subs, _, _) = c.fracture();
        gen_2.extend(subs);
    }
    assert_eq!(gen_2.len(), 8, "Gen 2: 4 x 100 voxels must split into 8 chunks");
    for c in &gen_2 {
        assert!(!c.settled);
    }

    // Gen 3
    let mut gen_3 = Vec::new();
    for c in gen_2 {
        let (subs, _, _) = c.fracture();
        gen_3.extend(subs);
    }
    assert_eq!(gen_3.len(), 16, "Gen 3: 8 x 50 voxels must split into 16 chunks");
    for c in &gen_3 {
        assert!(!c.settled);
    }

    // Gen 4: terminal decay!
    // Each chunk now has 25 voxels (<= 40), so fracture() MUST return 1 settled chunk with zero velocity
    let mut gen_4 = Vec::new();
    for c in gen_3 {
        let (subs, _, _) = c.fracture();
        gen_4.extend(subs);
    }
    assert_eq!(gen_4.len(), 16, "Gen 4: Terminal debris must return 16 settled chunks");
    for c in &gen_4 {
        assert!(c.settled, "Terminal sub-chunks (<= 40 voxels) must be settled");
        assert_eq!(c.velocity, Vec3::ZERO, "Settled chunk velocity must be zero");
        assert_eq!(c.angular_velocity, Vec3::ZERO, "Settled chunk angular velocity must be zero");
    }

    // Invariant: across all 4 generations of cascading demolition, exactly 400 voxels remain!
    let final_voxels: usize = gen_4.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(
        final_voxels, initial_count,
        "Total voxel count across 4 generations of cascading fracture must be strictly conserved"
    );

    let final_mass: f32 = gen_4.iter().map(|c| c.mass).sum();
    assert!(
        (final_mass - initial_mass).abs() < 0.1,
        "Total mass across 4 generations of cascading fracture must be strictly conserved"
    );
}
