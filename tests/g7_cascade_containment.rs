//! Cascade-containment invariants for the crush / fracture / rubble-wake physics.
//!
//! These lock in the fixes for runaway chain reactions: a collapse must never create more
//! destructive energy than it consumes.

use macroquad::prelude::*;
use voxel_demolition::audio::SoundSystem;
use voxel_demolition::physics::{DynamicChunk, PhysicsSystem, CRUSH_ENERGY_PER_THRESHOLD};
use voxel_demolition::voxel::{VoxelGrid, VoxelType, VOXEL_SIZE};

fn concrete_floor(grid: &mut VoxelGrid, y: i32) {
    for x in 28..=38 {
        for z in 28..=38 {
            grid.set(x, y, z, VoxelType::Concrete);
        }
    }
}

fn block_chunk(position: Vec3, velocity: Vec3, nx: i32, ny: i32, nz: i32) -> DynamicChunk {
    let mut voxels = Vec::new();
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                voxels.push((
                    Vec3::new(x as f32, y as f32, z as f32) * VOXEL_SIZE,
                    VoxelType::Concrete,
                ));
            }
        }
    }
    let mass = voxels.len() as f32 * VoxelType::Concrete.properties().density;
    let aabb = DynamicChunk::compute_aabb(&voxels);
    DynamicChunk::new(position, velocity, Vec3::ZERO, Vec3::ZERO, voxels, mass, 0.0, false, aabb)
}

#[test]
fn small_chip_cannot_pulverize_concrete_on_impact() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    concrete_floor(&mut grid, 8);
    let before = grid.solid_count();

    // 3-voxel concrete chip at 5 m/s: passes the old momentum test (~18 > 15) but only carries
    // ~100 J of kinetic energy, far below the 300 J needed to pulverize one concrete voxel.
    let floor_top = grid.voxel_to_world(IVec3::new(33, 8, 33)) + Vec3::new(0.0, VOXEL_SIZE * 0.6, 0.0);
    phys.chunks.push(block_chunk(floor_top, Vec3::new(0.0, -5.0, 0.0), 3, 1, 1));

    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

    assert_eq!(
        grid.solid_count(),
        before,
        "A small chip must not pulverize structural concrete (this was the self-sustaining cascade)"
    );
}

#[test]
fn crushing_is_bounded_by_kinetic_energy() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    concrete_floor(&mut grid, 8);
    let before = grid.solid_count();

    let dt = 0.016;
    let v0 = 6.0f32;
    let chunk = block_chunk(
        grid.voxel_to_world(IVec3::new(33, 8, 33)) + Vec3::new(0.0, VOXEL_SIZE * 0.75, 0.0),
        Vec3::new(0.0, -v0, 0.0),
        3,
        2,
        2,
    );
    let mass = chunk.mass;
    phys.chunks.push(chunk);

    phys.update(dt, &mut grid, &audio, Vec3::ZERO);

    let destroyed = before - grid.solid_count();
    // Kinetic energy available this frame (after one gravity step, with generous margin).
    let v = v0 + 22.0 * dt;
    let kinetic = 0.5 * mass * v * v;
    let concrete_cost = 15.0 * CRUSH_ENERGY_PER_THRESHOLD;
    let max_affordable = (kinetic / concrete_cost).floor() as usize;

    assert!(destroyed >= 1, "A 12-voxel chunk at 6 m/s must still break a concrete floor (pancake collapse)");
    assert!(
        destroyed <= max_affordable,
        "Crushed {} voxels but kinetic energy only pays for {}",
        destroyed,
        max_affordable
    );
}

#[test]
fn crush_chips_inherit_motion_instead_of_being_launched() {
    let mut grid = VoxelGrid::new();
    let mut phys = PhysicsSystem::new();
    let audio = SoundSystem::empty();
    concrete_floor(&mut grid, 8);

    let v0 = 12.0f32;
    phys.chunks.push(block_chunk(
        grid.voxel_to_world(IVec3::new(31, 8, 31)) + Vec3::new(0.0, VOXEL_SIZE * 0.75, 0.0),
        Vec3::new(0.0, -v0, 0.0),
        5,
        1,
        5,
    ));

    phys.update(0.016, &mut grid, &audio, Vec3::ZERO);

    // Chips: inherited 35% of the crusher's velocity + at most 4 m/s scatter (+ gravity/margin).
    let max_chip_speed = (v0 + 1.0) * 0.35 + 4.0 + 0.5;
    for chip in phys.chunks.iter().skip(1) {
        assert!(
            chip.velocity.length() <= max_chip_speed,
            "Crush chip launched at {:.1} m/s (max {:.1}); fast chips re-crush concrete and feed cascades",
            chip.velocity.length(),
            max_chip_speed
        );
        assert!(chip.angular_velocity.abs().max_element() <= 3.0 + 1e-3);
    }
}

#[test]
fn rubble_wake_impulse_scales_with_mass() {
    let mut phys = PhysicsSystem::new();
    let center = Vec3::new(0.0, 0.5, 0.0);

    // Pebble and a large settled slab, both 2 m from the blast centre.
    let mut pebble = block_chunk(Vec3::new(2.0, 0.75, 0.0), Vec3::ZERO, 1, 1, 1);
    pebble.settled = true;
    let mut slab = block_chunk(Vec3::new(-2.0, 0.75, 0.0), Vec3::ZERO, 10, 2, 10);
    slab.settled = true;
    phys.chunks.push(pebble);
    phys.chunks.push(slab);

    phys.un_settle_rubble_crater(center, 4.0, 20.0);

    let pebble_dv = phys.chunks[0].velocity.length();
    let slab_dv = phys.chunks[1].velocity.length();
    assert!(pebble_dv > 0.0, "Pebble must be woken and pushed");
    assert!(
        slab_dv < pebble_dv * 0.5,
        "A 200-voxel slab (dv {:.2}) must be pushed far less than a pebble (dv {:.2})",
        slab_dv,
        pebble_dv
    );
    assert!(
        phys.chunks[1].angular_velocity.abs().max_element() < 12.0,
        "Large slabs must not be spun up to the old flat ±12 rad/s"
    );
}
