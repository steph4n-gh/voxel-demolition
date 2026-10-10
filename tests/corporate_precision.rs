use macroquad::prelude::{rand, Vec3};
use voxel_demolition::{
    audio::SoundSystem,
    campaign::{CampaignManager, ContractStatus},
    fluid::{FluidSystem, FluidType},
    physics::PhysicsSystem,
    player::Player,
    voxel::{VoxelGrid, VOXEL_SIZE},
    weapon::{StabilizerType, WeaponMode, WeaponSystem},
    world_gen::{generate_level, LevelPreset},
};

/// A fixed-60-Hz reachability check using legal workbench settings and the main
/// loop's physical feedback, player, muzzle, cooldown, cost, and grading order.
/// Run explicitly with `cargo test --test corporate_precision -- --ignored`.
#[test]
#[ignore = "Runs the full Corporate Drop collapse and rubble settlement"]
fn corporate_drop_precision_reaches_legitimate_settled_win() {
    const DT: f32 = 1.0 / 60.0;
    rand::srand(0xB04A);
    let audio = SoundSystem::empty();
    let mut campaign = CampaignManager::new();
    let contract = campaign.active_contract();
    assert_eq!(contract.target_level, LevelPreset::ApexSkyscraper);
    assert_eq!(contract.target_destruction_ratio, 0.70);
    assert_eq!(contract.time_limit_seconds, 120.0);
    assert_eq!(contract.max_collateral_budget, 100_000.0);
    assert_eq!(contract.max_ordnance_budget, 300_000.0);
    assert_eq!(contract.max_c4_charges, 12);
    let zone = contract.drop_zone.as_ref().unwrap();
    assert_eq!(zone.min, Vec3::new(-20.0, 0.0, -20.0));
    assert_eq!(zone.max, Vec3::new(20.0, 40.0, 20.0));

    let mut grid = VoxelGrid::new();
    let mut physics = PhysicsSystem::new();
    let layout = generate_level(&mut grid, contract.target_level);
    physics.clear();
    let mut player = Player::new(layout.spawn_point(&grid));
    let mut fluids = FluidSystem::new();
    let mut weapon = WeaponSystem::new();
    weapon.config.mode = WeaponMode::EventHorizon;
    weapon.config.blast_radius_mult = 3.0;
    weapon.config.fire_rate_mult = 3.0;
    weapon.config.velocity_mult = 2.5;
    weapon.config.impulse_mult = 0.5;
    weapon.config.stabilizer = StabilizerType::HeavyCompensator;
    weapon.config.recoil_damping = 2.0;
    campaign.start_contract(grid.solid_count());

    // Four cuts down each of the central and four surrounding towers. These
    // are design coordinates converted by the generated level's real layout.
    let mut targets = Vec::new();
    for (x, z, top) in [
        (48, 57, 190),
        (20, 81, 160),
        (76, 71, 180),
        (20, 39, 180),
        (76, 29, 160),
    ] {
        for y in [top, top - 50, top - 100, (top - 150).max(8)] {
            targets.push((x, y, z));
        }
    }
    let mut shots = 0;
    let mut next_shot = 0.0;
    let mut bullet_time_timer: f32 = 0.0;

    for _ in 0..7_202 {
        if campaign.status != ContractStatus::Active {
            break;
        }
        player.update(DT, &grid, Some(&physics), false);
        let cam_pos = player.camera_position_at(campaign.elapsed_time);
        let mut fired = false;
        if shots < targets.len()
            && campaign.current_destruction_ratio < 0.70
            && campaign.elapsed_time >= next_shot
            && weapon.fire_cooldown <= 0.0
        {
            let (x, y, z) = targets[shots];
            let aim = (layout.design_to_world(&grid, x, y, z) - cam_pos).normalize();
            player.yaw = aim.z.atan2(aim.x);
            player.pitch = aim.y.asin().clamp(-1.52, 1.52);
            let dir = player.forward_dir();
            let muzzle = cam_pos + player.right_dir() * 0.25 - player.up_dir() * 0.2 + dir * 0.6;
            let previous_cooldown = weapon.fire_cooldown;
            let feedback =
                weapon.fire_primary(muzzle, dir, cam_pos, &mut grid, &mut physics, &audio);
            if weapon.fire_cooldown > previous_cooldown {
                shots += 1;
                fired = true;
                campaign.record_ordnance_spend(500.0);
                player.add_trauma(feedback.camera_trauma);
                player.apply_weapon_recoil(feedback.player_recoil);
                player.fov_kick += feedback.fov_kick;
            }
        }

        let (trauma, _) = fluids.update_structural_feedback(
            DT,
            &mut grid,
            &mut physics,
            &audio,
            FluidType::Water,
            cam_pos,
        );
        player.add_trauma(trauma);
        let sim_dt = if bullet_time_timer > 0.0 {
            DT * 0.25
        } else {
            DT
        };
        bullet_time_timer = (bullet_time_timer - DT).max(0.0);
        player.add_trauma(physics.update(sim_dt, &mut grid, &audio, cam_pos));
        player.add_trauma(weapon.update(sim_dt, &mut grid, &mut physics, &audio, cam_pos));
        fluids.update(sim_dt, &mut grid, &mut physics, &audio);

        if grid.structural_dirty {
            let islands = grid.detect_unsupported_islands();
            if !islands.is_empty() {
                grid.structural_dirty = true;
            }
            let count = islands
                .iter()
                .map(|island| island.voxels.len())
                .sum::<usize>();
            if count >= 4_000 {
                bullet_time_timer = 1.2;
            }
            if let Some(closest) =
                islands
                    .iter()
                    .map(|island| island.center_of_mass)
                    .min_by(|a, b| {
                        a.distance_squared(cam_pos)
                            .partial_cmp(&b.distance_squared(cam_pos))
                            .unwrap()
                    })
            {
                let trauma = (count as f32 / 140.0).clamp(0.0, 0.65)
                    * SoundSystem::calculate_distance_attenuation(closest, cam_pos);
                if trauma > 0.01 {
                    player.add_trauma(trauma);
                }
            }
            for island in islands {
                physics.spawn_stress_fissure(island.center_of_mass, Vec3::Y, 35);
                let radius =
                    ((island.voxels.len() as f32).cbrt() * VOXEL_SIZE * 1.5).clamp(1.5, 16.0);
                physics.spawn_structural_collapse_dust_storm(
                    island.center_of_mass,
                    radius,
                    island.voxels.len(),
                );
                let radius = ((island.voxels.len() as f32).cbrt() * VOXEL_SIZE).max(0.5);
                let scale = (0.5 / radius).min(1.0);
                let impulse = Vec3::new(
                    rand::gen_range(-1.5, 1.5) * scale,
                    rand::gen_range(-2.5, -1.0),
                    rand::gen_range(-1.5, 1.5) * scale,
                );
                let spin = Vec3::new(
                    rand::gen_range(-1.2, 1.2),
                    rand::gen_range(-0.6, 0.6),
                    rand::gen_range(-1.2, 1.2),
                ) * scale;
                physics.spawn_island_chunk(island, impulse, spin);
            }
        }
        campaign.update(DT, &grid, &physics);
        if fired {
            next_shot = campaign.elapsed_time + 0.35;
        }
    }

    println!(
        "Corporate Drop: {:?}, destruction={:.3}%, time={:.3}s, collateral=${:.0}, \
         ordnance=${:.0}, accepted_shots={}, chunks={}, falling={}",
        campaign.status,
        campaign.current_destruction_ratio * 100.0,
        campaign.elapsed_time,
        campaign.collateral_damage_dollars,
        campaign.ordnance_dollars_spent,
        shots,
        physics.chunks.len(),
        physics
            .chunks
            .iter()
            .filter(|chunk| !chunk.settled && !chunk.voxels.is_empty())
            .count(),
    );
    assert!(matches!(campaign.status, ContractStatus::Passed(_)));
    assert!(campaign.current_destruction_ratio >= 0.70);
    assert!(campaign.elapsed_time <= 120.0);
    assert!(campaign.collateral_damage_dollars <= 100_000.0);
    assert_eq!(shots, 20);
    assert_eq!(campaign.ordnance_dollars_spent, 10_000.0);
    assert_eq!(campaign.c4_charges_used, 0);
    assert!(physics
        .chunks
        .iter()
        .all(|chunk| chunk.settled || chunk.voxels.is_empty()));
    assert!(weapon.projectiles.is_empty());
    assert!(weapon.singularities.is_empty());
    assert!(weapon.c4_charges.is_empty());
    assert!(weapon.sticky_charges.is_empty());
}
