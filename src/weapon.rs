use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, VoxelType, DamageType};
use crate::physics::PhysicsSystem;
use crate::audio::SoundSystem;
use std::collections::{VecDeque, HashSet};

/// The surrounding slab only exists outside the destructible voxel arena.
pub(crate) fn surrounding_foundation_hit(start: Vec3, end: Vec3) -> Option<Vec3> {
    if start.y < 0.5 || end.y > 0.5 || end.y >= start.y {
        return None;
    }
    let t = (start.y - 0.5) / (start.y - end.y);
    let hit = start.lerp(end, t);
    let half_x = crate::voxel::GRID_X as f32 * crate::voxel::VOXEL_SIZE * 0.5;
    let half_z = crate::voxel::GRID_Z as f32 * crate::voxel::VOXEL_SIZE * 0.5;
    let outside_grid = hit.x < -half_x || hit.x >= half_x || hit.z < -half_z || hit.z >= half_z;
    (outside_grid && hit.x.abs() <= 240.0 && hit.z.abs() <= 240.0).then_some(hit)
}

pub fn spawn_anchored_impact_decal(
    grid: &VoxelGrid,
    physics: &mut PhysicsSystem,
    hit_pos: Vec3,
    hit_norm: Vec3,
    radius: f32,
    color: Color,
    kind: crate::physics::DecalKind,
) {
    let crater_ray = grid.raycast(hit_pos + hit_norm * 0.5, -hit_norm, radius * 2.0 + 1.0);
    if let Some(crater) = crater_ray {
        physics.spawn_decal_anchored(
            crater.world_pos,
            crater.normal,
            radius,
            color,
            kind,
            Some(crater.voxel_pos),
        );
    } else if let Some(anchor) = grid.find_nearest_solid(hit_pos, radius + 1.2) {
        let a_pos = grid.voxel_to_world(anchor);
        physics.spawn_decal_anchored(
            a_pos,
            hit_norm,
            radius,
            color,
            kind,
            Some(anchor),
        );
    }
}

/// The three volatile trigger modes participating in chain reactions
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VolatileTriggerKind {
    Explosive,
    GlowCore,
    PowerCapacitor,
}

/// Feature 9: Tri-Modal Volatile Chain Reactions with progressive dissipation,
/// unified chronological queue (up to 24 hops), deduplication, and high-voltage arc jumps.
pub fn process_explosive_chain_with_capacitors(
    grid: &mut VoxelGrid,
    physics: &mut PhysicsSystem,
    audio: &SoundSystem,
    initial_explosives: Vec<IVec3>,
    initial_glow_cores: Vec<IVec3>,
    initial_capacitors: Vec<IVec3>,
    camera_pos: Vec3,
) -> f32 {
    process_explosive_chain_at_world_positions(
        grid, physics, audio, initial_explosives, initial_glow_cores,
        initial_capacitors, Vec::new(), camera_pos,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_explosive_chain_at_world_positions(
    grid: &mut VoxelGrid,
    physics: &mut PhysicsSystem,
    audio: &SoundSystem,
    initial_explosives: Vec<IVec3>,
    initial_glow_cores: Vec<IVec3>,
    initial_capacitors: Vec<IVec3>,
    initial_dynamic: Vec<(Vec3, VoxelType)>,
    camera_pos: Vec3,
) -> f32 {
    let mut trauma = 0.0f32;
    let mut cascade_queue: VecDeque<(VolatileTriggerKind, Vec3)> = VecDeque::new();
    let mut visited: HashSet<IVec3> = HashSet::new();

    // Enqueue initial triggers with automatic routing by voxel identity
    for exp_v in initial_explosives {
        if visited.insert(exp_v) {
            let vt = grid.get(exp_v.x, exp_v.y, exp_v.z);
            if vt == VoxelType::PowerCapacitor {
                cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(exp_v)));
            } else if vt == VoxelType::GlowCore {
                cascade_queue.push_back((VolatileTriggerKind::GlowCore, grid.voxel_to_world(exp_v)));
            } else {
                cascade_queue.push_back((VolatileTriggerKind::Explosive, grid.voxel_to_world(exp_v)));
            }
        }
    }
    for core_v in initial_glow_cores {
        if visited.insert(core_v) {
            let vt = grid.get(core_v.x, core_v.y, core_v.z);
            if vt == VoxelType::PowerCapacitor {
                cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(core_v)));
            } else {
                cascade_queue.push_back((VolatileTriggerKind::GlowCore, grid.voxel_to_world(core_v)));
            }
        }
    }
    for cap_v in initial_capacitors {
        if visited.insert(cap_v) {
            cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(cap_v)));
        }
    }
    for (position, voxel_type) in initial_dynamic {
        let kind = match voxel_type {
            VoxelType::Explosive => VolatileTriggerKind::Explosive,
            VoxelType::GlowCore => VolatileTriggerKind::GlowCore,
            VoxelType::PowerCapacitor => VolatileTriggerKind::PowerCapacitor,
            _ => continue,
        };
        cascade_queue.push_back((kind, position));
    }

    const MAX_TOTAL_HOPS: usize = 24;
    let mut total_detonations = 0;

    while let Some((kind, world_pos)) = cascade_queue.pop_front() {
        if total_detonations >= MAX_TOTAL_HOPS {
            break;
        }
        total_detonations += 1;

        let dist_atten = SoundSystem::calculate_distance_attenuation(world_pos, camera_pos);
        let dissipation = (1.0 - (total_detonations as f32 / MAX_TOTAL_HOPS as f32) * 0.45).max(0.40);

        let (blast_radius, blast_damage, damage_type) = match kind {
            VolatileTriggerKind::Explosive => {
                // Mode 1: Concussive kinetic blast (Fuel Conduit / Silo)
                let blast_r = (2.8 * dissipation).max(1.2);
                let blast_dmg = 320.0 * dissipation;

                // Shockwave ring for concussive overpressure wave
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: world_pos,
                    normal: Vec3::Y,
                    radius: 0.4,
                    max_radius: (blast_r * 2.2).max(4.0),
                    speed: 24.0,
                    color: Color::new(1.0, 0.45, 0.1, 0.9),
                    thickness: 0.35,
                });

                physics.add_explosion_effects(world_pos, blast_r * 1.2, 32.0 * dissipation, Color::new(1.0, 0.40, 0.1, 1.0), 22);
                physics.apply_radial_blast(world_pos, blast_r * 1.4, 38.0 * dissipation);
                audio.play_spatial(audio.big_boom.as_ref(), world_pos, camera_pos, 0.65);
                trauma += 0.15 * dist_atten * dissipation;

                // Secondary conduit blowout: pressurized fire jet erupts along trajectory
                let jet_dir = Vec3::new(
                    macroquad::rand::gen_range(-0.4f32, 0.4),
                    macroquad::rand::gen_range(0.7f32, 1.0),
                    macroquad::rand::gen_range(-0.4f32, 0.4),
                ).normalize();
                physics.spawn_fire_jet(world_pos, jet_dir, macroquad::rand::gen_range(1.2, 2.2));

                let res = grid.carve_sphere_advanced(world_pos, blast_r, blast_dmg, DamageType::Explosive);
                if res.ice_shattered > 0 {
                    physics.spawn_ice_shatter_effects(world_pos, blast_r, (res.ice_shattered * 4).clamp(15, 35));
                    audio.play_spatial(audio.cryo_shatter.as_ref(), world_pos, camera_pos, 0.55);
                }

                for new_exp in res.explosive_triggers {
                    if visited.insert(new_exp) {
                        cascade_queue.push_back((VolatileTriggerKind::Explosive, grid.voxel_to_world(new_exp)));
                    }
                }
                for new_core in res.glow_core_triggers {
                    if visited.insert(new_core) {
                        cascade_queue.push_back((VolatileTriggerKind::GlowCore, grid.voxel_to_world(new_core)));
                    }
                }
                for new_cap in res.capacitor_triggers {
                    if visited.insert(new_cap) {
                        cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(new_cap)));
                    }
                }
                (blast_r, blast_dmg, DamageType::Explosive)
            }

            VolatileTriggerKind::GlowCore => {
                // Mode 2: Thermal plasma breach (Quantum Core / Energy Core)
                let blast_r = (3.4 * dissipation).max(1.5);
                let blast_dmg = 420.0 * dissipation;

                // Blinding cyan/yellow plasma shockwave discharge
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: world_pos,
                    normal: Vec3::Y,
                    radius: 0.5,
                    max_radius: (8.0 * dissipation).max(4.0),
                    speed: 32.0,
                    color: Color::new(1.0, 0.9, 0.2, 1.0),
                    thickness: 0.40,
                });
                physics.spawn_molten_fissure(world_pos, Vec3::Y, 20);
                physics.apply_radial_blast(world_pos, 8.0 * dissipation, 60.0 * dissipation);
                physics.add_explosion_effects(world_pos, 3.5 * dissipation, 40.0 * dissipation, Color::new(0.2, 0.95, 1.0, 1.0), 32);

                audio.play_spatial(audio.singularity_implode.as_ref(), world_pos, camera_pos, 0.70);
                audio.play_spatial(audio.railgun_boom.as_ref(), world_pos, camera_pos, 0.60);
                trauma += 0.25 * dist_atten * dissipation;

                let res = grid.carve_sphere_advanced(world_pos, blast_r, blast_dmg, DamageType::Thermal);
                for new_exp in res.explosive_triggers {
                    if visited.insert(new_exp) {
                        cascade_queue.push_back((VolatileTriggerKind::Explosive, grid.voxel_to_world(new_exp)));
                    }
                }
                for new_core in res.glow_core_triggers {
                    if visited.insert(new_core) {
                        cascade_queue.push_back((VolatileTriggerKind::GlowCore, grid.voxel_to_world(new_core)));
                    }
                }
                for new_cap in res.capacitor_triggers {
                    if visited.insert(new_cap) {
                        cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(new_cap)));
                    }
                }
                (blast_r, blast_dmg, DamageType::Thermal)
            }

            VolatileTriggerKind::PowerCapacitor => {
                // Mode 3: High-Voltage Electrical Arc Discharge & Overload
                let blast_r = (2.2 * dissipation).max(1.0);
                let blast_dmg = 180.0 * dissipation;
                let arc_dmg = 140.0 * dissipation;

                // Electric violet discharge shockwave ring
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: world_pos,
                    normal: Vec3::Y,
                    radius: 0.4,
                    max_radius: (6.5 * dissipation).max(3.5),
                    speed: 28.0,
                    color: Color::new(0.68, 0.28, 0.98, 1.0),
                    thickness: 0.35,
                });

                // High-voltage sparks and plasma burst
                physics.add_explosion_effects(world_pos, 2.5 * dissipation, 35.0 * dissipation, Color::new(0.70, 0.30, 1.0, 1.0), 30);
                physics.apply_radial_blast(world_pos, 4.5 * dissipation, 32.0 * dissipation);

                audio.play_spatial(audio.railgun_boom.as_ref(), world_pos, camera_pos, 0.60);
                trauma += 0.20 * dist_atten * dissipation;

                // Carve immediate capacitor breach crater
                let res = grid.carve_sphere_advanced(world_pos, blast_r, blast_dmg, DamageType::Thermal);
                for new_exp in res.explosive_triggers {
                    if visited.insert(new_exp) {
                        cascade_queue.push_back((VolatileTriggerKind::Explosive, grid.voxel_to_world(new_exp)));
                    }
                }
                for new_core in res.glow_core_triggers {
                    if visited.insert(new_core) {
                        cascade_queue.push_back((VolatileTriggerKind::GlowCore, grid.voxel_to_world(new_core)));
                    }
                }
                for new_cap in res.capacitor_triggers {
                    if visited.insert(new_cap) {
                        cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, grid.voxel_to_world(new_cap)));
                    }
                }

                // Electric arc jump to conductive elements within 6-8m (7.0m)
                let vpos = grid.world_to_voxel(world_pos);
                let arc_range = 7.0 * dissipation;
                let r_vox = (arc_range / crate::voxel::VOXEL_SIZE).ceil() as i32;
                let mut conductive_targets = Vec::new();

                for dx in -r_vox..=r_vox {
                    for dy in -r_vox..=r_vox {
                        for dz in -r_vox..=r_vox {
                            let cv = IVec3::new(vpos.x + dx, vpos.y + dy, vpos.z + dz);
                            if !grid.in_bounds(cv.x, cv.y, cv.z) {
                                continue;
                            }
                            let c_wpos = grid.voxel_to_world(cv);
                            let d2 = world_pos.distance_squared(c_wpos);
                            if d2 <= arc_range * arc_range {
                                let vt = grid.get(cv.x, cv.y, cv.z);
                                let cond = vt.properties().conductivity;
                                if cond > 0.0 {
                                    conductive_targets.push((cv, cond, d2));
                                }
                            }
                        }
                    }
                }

                // Sort targets by conductivity descending (favoring capacitors, rebar, armor) then distance
                conductive_targets.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                });

                // Arc jump up to 6 targets
                let arc_limit = 6;
                for (cv, _, _) in conductive_targets.into_iter().take(arc_limit) {
                    let target_wpos = grid.voxel_to_world(cv);
                    // Spawn spark line / arc tracer and animated jagged arc
                    physics.spawn_spall_impact(target_wpos, (target_wpos - world_pos).normalize_or_zero(), Color::new(0.35, 0.75, 1.0, 1.0));
                    physics.spawn_electrical_arc(world_pos, target_wpos, macroquad::rand::gen_range(0.2, 0.45), Color::new(0.25, 0.70, 1.0, 1.0));

                    let vt_before = grid.get(cv.x, cv.y, cv.z);
                    let is_destroyed = grid.damage_voxel(cv.x, cv.y, cv.z, arc_dmg, DamageType::Thermal);
                    if is_destroyed {
                        match vt_before {
                            VoxelType::PowerCapacitor if visited.insert(cv) => {
                                cascade_queue.push_back((VolatileTriggerKind::PowerCapacitor, target_wpos));
                            }
                            VoxelType::GlowCore if visited.insert(cv) => {
                                cascade_queue.push_back((VolatileTriggerKind::GlowCore, target_wpos));
                            }
                            VoxelType::Explosive if visited.insert(cv) => {
                                cascade_queue.push_back((VolatileTriggerKind::Explosive, target_wpos));
                            }
                            _ => {}
                        }
                    }
                }
                (blast_r, blast_dmg, DamageType::Thermal)
            }
        };
        // Removed dynamic voxels cannot trigger again; keep their actual moving-body centers.
        let (_, dynamic_triggers) = physics.carve_chunk_sphere(grid, world_pos, blast_radius, blast_damage, damage_type, false);
        for (position, voxel_type) in dynamic_triggers {
            let kind = match voxel_type {
                VoxelType::Explosive => VolatileTriggerKind::Explosive,
                VoxelType::GlowCore => VolatileTriggerKind::GlowCore,
                VoxelType::PowerCapacitor => VolatileTriggerKind::PowerCapacitor,
                _ => continue,
            };
            cascade_queue.push_back((kind, position));
        }
    }

    trauma.min(0.95)
}

/// Backwards-compatible explosive chain forwarder (for 2-vector callers)
pub fn process_explosive_chain(
    grid: &mut VoxelGrid,
    physics: &mut PhysicsSystem,
    audio: &SoundSystem,
    initial_explosives: Vec<IVec3>,
    initial_glow_cores: Vec<IVec3>,
    camera_pos: Vec3,
) -> f32 {
    process_explosive_chain_with_capacitors(
        grid,
        physics,
        audio,
        initial_explosives,
        initial_glow_cores,
        Vec::new(),
        camera_pos,
    )
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WeaponMode {
    TitanRam = 0,         // Close-range kinetic breaching ram
    HavocMortar = 1,      // Rocket / Demolition Cannon
    EventHorizon = 2,     // Gravitational Vortex Imploder
    SolsticeLance = 3,    // Continuous Plasma Drill Laser
    ApexRailgun = 4,      // Hyper-velocity wall piercer
    FrostbiteMortar = 5,  // Cryo-shatter cannon
    SeismicDisrupter = 6, // Subterranean fault resonance disruptor
    TeslaCaster = 7,      // High-voltage arc lightning conductor
    PlasmaFlamer = 8,     // Rapid superheated thermal torrent
    PlasmaWireSaw = 9,    // High-frequency plasma monofilament planar cutting tool
}

impl WeaponMode {
    pub fn name(&self) -> &'static str {
        match self {
            WeaponMode::TitanRam => "TITAN MAUL (BREACHING RAM)",
            WeaponMode::HavocMortar => "HAVOC CANNON (DEMOLITION ROCKET)",
            WeaponMode::EventHorizon => "EVENT HORIZON (SINGULARITY VORTEX)",
            WeaponMode::SolsticeLance => "SOLSTICE LANCE (PLASMA DRILL)",
            WeaponMode::ApexRailgun => "APEX RAILGUN (WALL PIERCER)",
            WeaponMode::FrostbiteMortar => "FROSTBITE (CRYO-SHATTER)",
            WeaponMode::SeismicDisrupter => "SEISMIC DISRUPTER (FAULT RESONANCE)",
            WeaponMode::TeslaCaster => "TESLA CASTER (ARC LIGHTNING)",
            WeaponMode::PlasmaFlamer => "PLASMA FLAMER (THERMAL TORRENT)",
            WeaponMode::PlasmaWireSaw => "PLASMA WIRE-SAW (PLANAR SLICER)",
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            WeaponMode::TitanRam => "Devastating forward kinetic blast. Pulverizes facades in a cone.",
            WeaponMode::HavocMortar => "Heavy explosive ordinance with devastating spherical destruction.",
            WeaponMode::EventHorizon => "Creates a gravitational singularity that pulls walls inward then detonates.",
            WeaponMode::SolsticeLance => "Sustained high-thermal laser. Slices structural columns like butter.",
            WeaponMode::ApexRailgun => "Hypersonic hitscan slug that pierces clean through entire buildings.",
            WeaponMode::FrostbiteMortar => "Freezes structures into brittle ice, then shatters them to pieces.",
            WeaponMode::SeismicDisrupter => "Fires resonant acoustic torpedoes that burrow deep and trigger tectonic collapses.",
            WeaponMode::TeslaCaster => "Discharges cascading high-voltage lightning that conducts through rebar frames.",
            WeaponMode::PlasmaFlamer => "Rapid continuous torrent of superheated plasma globes that melts armor and ignites fuel.",
            WeaponMode::PlasmaWireSaw => "High-frequency laser monofilament. Slices standing megastructures and physics chunks along planar cuts.",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            WeaponMode::TitanRam => Color::new(1.0, 0.45, 0.15, 1.0),
            WeaponMode::HavocMortar => Color::new(1.0, 0.65, 0.1, 1.0),
            WeaponMode::EventHorizon => Color::new(0.75, 0.2, 0.95, 1.0),
            WeaponMode::SolsticeLance => Color::new(0.2, 0.9, 1.0, 1.0),
            WeaponMode::ApexRailgun => Color::new(0.3, 0.7, 1.0, 1.0),
            WeaponMode::FrostbiteMortar => Color::new(0.5, 0.95, 1.0, 1.0),
            WeaponMode::SeismicDisrupter => Color::new(0.95, 0.72, 0.2, 1.0),
            WeaponMode::TeslaCaster => Color::new(0.45, 0.55, 1.0, 1.0),
            WeaponMode::PlasmaFlamer => Color::new(1.0, 0.32, 0.08, 1.0),
            WeaponMode::PlasmaWireSaw => Color::new(0.92, 0.25, 1.0, 1.0),
        }
    }

    pub fn alt_fire_name(&self) -> &'static str {
        match self {
            WeaponMode::TitanRam => "SEISMIC GROUND RING",
            WeaponMode::HavocMortar => "AIRBURST CLUSTER",
            WeaponMode::EventHorizon => "GRAV-PULSE IMPLOSION",
            WeaponMode::SolsticeLance => "MOLTEN SEARING SLICE",
            WeaponMode::ApexRailgun => "HYPER-SLUG (HOLD TO CHARGE)",
            WeaponMode::FrostbiteMortar => "BLIZZARD CRYO-SHATTER",
            WeaponMode::SeismicDisrupter => "TECTONIC FAULT RUPTURE",
            WeaponMode::TeslaCaster => "OVERLOAD ARC STORM",
            WeaponMode::PlasmaFlamer => "THERMOBARIC FLAME WALL",
            WeaponMode::PlasmaWireSaw => "CYCLE SLICE PLANE",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BarrelType {
    Precision = 0,
    TriSpread = 1,
    ClusterSplit = 2,
    Piercing = 3,
}

impl BarrelType {
    pub fn name(&self) -> &'static str {
        match self {
            BarrelType::Precision => "PRECISION BORE",
            BarrelType::TriSpread => "TRI-SPREAD FAN",
            BarrelType::ClusterSplit => "CLUSTER BOMBLET",
            BarrelType::Piercing => "ARMOR PIERCER",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DetonatorType {
    InstantImpact = 0,
    RemoteSticky = 1,
}

impl DetonatorType {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            DetonatorType::InstantImpact => "INSTANT IMPACT",
            DetonatorType::RemoteSticky => "REMOTE STICKY",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PayloadType {
    HighExplosive = 0,
    ShapedCharge = 1,
    CryoShrapnel = 2,
    Incendiary = 3,
}

impl PayloadType {
    pub fn name(&self) -> &'static str {
        match self {
            PayloadType::HighExplosive => "HIGH EXPLOSIVE (CONCUSSIVE)",
            PayloadType::ShapedCharge => "SHAPED CHARGE (ARMOR PIERCING)",
            PayloadType::CryoShrapnel => "CRYO SHRAPNEL (FREEZE SHATTER)",
            PayloadType::Incendiary => "INCENDIARY (THERMITE MELT)",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TrailStyle {
    SmokeWisps = 0,
    PlasmaGlow = 1,
    SparkShower = 2,
    HyperTracer = 3,
}

impl TrailStyle {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            TrailStyle::SmokeWisps => "ATMOSPHERIC WISPS",
            TrailStyle::PlasmaGlow => "LUMINOUS PLASMA GLOW",
            TrailStyle::SparkShower => "HOT SPARK CASCADE",
            TrailStyle::HyperTracer => "NEON HYPER TRACER",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StabilizerType {
    Standard = 0,
    HeavyCompensator = 1,
    GyroscopicThruster = 2,
    Overclocked = 3,
}

impl StabilizerType {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            StabilizerType::Standard => "STANDARD BRAKE",
            StabilizerType::HeavyCompensator => "HEAVY COMPENSATOR (-60% RECOIL)",
            StabilizerType::GyroscopicThruster => "GYRO THRUSTER (FLAT FLIGHT)",
            StabilizerType::Overclocked => "OVERCLOCKED (+30% VEL & IMPULSE)",
        }
    }
}

// Modular Weapon Configuration (The infinite customization engine)
#[derive(Clone, Debug)]
pub struct WeaponCustomization {
    pub mode: WeaponMode,
    pub barrel: BarrelType,
    pub detonator: DetonatorType,
    pub payload: PayloadType,
    pub trail_style: TrailStyle,
    pub stabilizer: StabilizerType,
    pub blast_radius_mult: f32, // 0.5 to 3.0
    pub impulse_mult: f32,      // 0.5 to 3.5
    pub fire_rate_mult: f32,    // 0.5 to 3.0
    pub velocity_mult: f32,     // 0.5 to 2.5
    pub cluster_count: usize,   // 3 to 8
    pub recoil_damping: f32,    // 0.5 to 2.0
}

impl Default for WeaponCustomization {
    fn default() -> Self {
        Self {
            mode: WeaponMode::HavocMortar,
            barrel: BarrelType::Precision,
            detonator: DetonatorType::InstantImpact,
            payload: PayloadType::HighExplosive,
            trail_style: TrailStyle::PlasmaGlow,
            stabilizer: StabilizerType::Standard,
            blast_radius_mult: 1.0,
            impulse_mult: 1.0,
            fire_rate_mult: 1.0,
            velocity_mult: 1.0,
            cluster_count: 5,
            recoil_damping: 1.0,
        }
    }
}

/// Inline fixed-size circular ring buffer for projectile ribbon trails
#[derive(Clone, Copy, Debug)]
pub struct TrailBuffer<const N: usize> {
    points: [Vec3; N],
    head: usize,
    count: usize,
}

impl<const N: usize> Default for TrailBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl<const N: usize> TrailBuffer<N> {
    pub const fn new() -> Self {
        Self {
            points: [Vec3::ZERO; N],
            head: 0,
            count: 0,
        }
    }

    pub const fn empty() -> Self {
        Self::new()
    }

    pub fn single(p: Vec3) -> Self {
        let mut tb = Self::new();
        tb.push(p);
        tb
    }

    pub fn push(&mut self, p: Vec3) {
        if self.count < N {
            self.points[self.count] = p;
            self.count += 1;
        } else {
            self.points[self.head] = p;
            self.head = (self.head + 1) % N;
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[inline]
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<Vec3> {
        if idx < self.count {
            Some(self.points[(self.head + idx) % N])
        } else {
            None
        }
    }

    #[inline]
    pub fn as_slice(&self) -> (&[Vec3], &[Vec3]) {
        if self.count < N {
            (&self.points[..self.count], &[])
        } else {
            (&self.points[self.head..], &self.points[..self.head])
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Vec3> {
        let (s1, s2) = self.as_slice();
        s1.iter().chain(s2.iter())
    }
}

impl<const N: usize> std::ops::Index<usize> for TrailBuffer<N> {
    type Output = Vec3;

    #[inline]
    fn index(&self, idx: usize) -> &Self::Output {
        assert!(idx < self.count, "TrailBuffer index out of bounds: {} >= {}", idx, self.count);
        &self.points[(self.head + idx) % N]
    }
}

pub struct ActiveProjectile {
    pub position: Vec3,
    pub velocity: Vec3,
    pub gravity: f32,
    pub radius: f32,
    pub blast_radius: f32,
    pub impulse: f32,
    pub mode: WeaponMode,
    pub barrel: BarrelType,
    pub payload: PayloadType,
    pub trail_style: TrailStyle,
    pub color: Color,
    pub life: f32,
    pub is_cluster_child: bool,
    pub is_airburst: bool,
    pub is_sticky: bool,
    pub stuck_normal: Vec3,
    pub trail_points: TrailBuffer<16>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FireFeedback {
    pub camera_trauma: f32,
    pub player_recoil: Vec3,
    pub fov_kick: f32,
}

pub struct ActiveSingularity {
    pub position: Vec3,
    pub timer: f32,
    pub max_time: f32,
    pub pull_radius: f32,
    pub pull_strength: f32,
    pub blast_radius: f32,
    pub vacuum_played: bool,
    pub erosion_timer: f32,
}

impl ActiveSingularity {
    pub const PULL_RADIUS: f32 = 35.0;
    pub const PULL_STRENGTH: f32 = 72.0;
    pub const COLLAPSE_RADIUS_MIN: f32 = 16.0;
    pub const COLLAPSE_RADIUS_MAX: f32 = 22.0;
}

pub struct RailgunTracer {
    pub start: Vec3,
    pub end: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
}

pub struct TeslaArcTracer {
    pub branches: Vec<(Vec3, Vec3)>,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
}

/// Maximum simultaneous active magnetic C4 shaped charges in the demolition grid
pub const MAX_C4_CHARGES: usize = 24;
pub const MAX_DET_CORD_DIST: f32 = 25.0;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct DetCordLink {
    pub idx_a: usize,
    pub idx_b: usize,
    pub p1: Vec3,
    pub p2: Vec3,
    pub dist: f32,
}

#[derive(Clone, Debug)]
pub struct ActiveCuttingShock {
    pub p1: Vec3,
    pub p2: Vec3,
    pub timer: f32,
    pub duration: f32,
    pub delay: f32,
    pub sound_played: bool,
}

#[derive(Clone, Debug)]
pub struct C4Charge {
    pub position: Vec3,
    pub normal: Vec3,
    pub anchor_voxel: Option<IVec3>,
    pub timer: f32,
    pub blast_radius: f32,
    pub impulse: f32,
}

pub struct WeaponSystem {
    pub config: WeaponCustomization,
    pub projectiles: Vec<ActiveProjectile>,
    pub singularities: Vec<ActiveSingularity>,
    pub tracers: Vec<RailgunTracer>,
    pub tesla_arcs: Vec<TeslaArcTracer>,
    pub sticky_charges: Vec<ActiveProjectile>,
    pub c4_charges: Vec<C4Charge>,
    pub cached_det_cords: Vec<DetCordLink>,
    pub det_cords_dirty: bool,
    pub active_cutting_shocks: Vec<ActiveCuttingShock>,
    pub fire_cooldown: f32,
    pub is_firing_laser: bool,
    pub laser_hit_pos: Option<Vec3>,
    pub laser_sound_timer: f32,
    pub recent_hit: Option<crate::ui::HitKind>,
    pub railgun_charge: f32,
    pub slice_plane_mode: usize,

    // Procedural Viewmodel Animation (Spring-damper & heat venting)
    pub recoil_pos: Vec3,
    pub recoil_rot: Vec3, // Pitch, Yaw, Roll
    pub recoil_pos_vel: Vec3,
    pub recoil_rot_vel: Vec3,
    pub heat: f32,
}

impl WeaponSystem {
    pub fn new() -> Self {
        Self {
            config: WeaponCustomization::default(),
            projectiles: Vec::new(),
            singularities: Vec::new(),
            tracers: Vec::new(),
            tesla_arcs: Vec::new(),
            sticky_charges: Vec::new(),
            c4_charges: Vec::new(),
            cached_det_cords: Vec::new(),
            det_cords_dirty: false,
            active_cutting_shocks: Vec::new(),
            fire_cooldown: 0.0,
            is_firing_laser: false,
            laser_hit_pos: None,
            laser_sound_timer: 0.0,
            recent_hit: None,
            railgun_charge: 0.0,
            slice_plane_mode: 0,
            recoil_pos: Vec3::ZERO,
            recoil_rot: Vec3::ZERO,
            recoil_pos_vel: Vec3::ZERO,
            recoil_rot_vel: Vec3::ZERO,
            heat: 0.0,
        }
    }

    pub fn update_det_cord_cache(&mut self) {
        self.cached_det_cords = self.compute_det_cord_links();
        self.det_cords_dirty = false;
    }

    #[allow(dead_code)]
    pub fn clear_c4(&mut self) {
        self.c4_charges.clear();
        self.cached_det_cords.clear();
        self.det_cords_dirty = false;
    }

    pub fn set_mode(&mut self, mode: WeaponMode, audio: &SoundSystem) {
        if self.config.mode != mode {
            self.config.mode = mode;
            self.is_firing_laser = false;
            self.laser_hit_pos = None;
            self.laser_sound_timer = 0.0;
            self.railgun_charge = 0.0;
            audio.play(audio.ui_switch.as_ref(), 0.5);
        }
    }

    pub fn fire_primary(
        &mut self,
        muzzle_pos: Vec3,
        camera_dir: Vec3,
        camera_pos: Vec3,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
    ) -> FireFeedback {
        if self.fire_cooldown > 0.0 {
            return FireFeedback::default();
        }

        let base_delay = match self.config.mode {
            WeaponMode::TitanRam => 0.60,
            WeaponMode::HavocMortar => 0.70,
            WeaponMode::EventHorizon => 1.00,
            WeaponMode::SolsticeLance => 0.06, // Rapid tick for continuous laser
            WeaponMode::ApexRailgun => 0.85,
            WeaponMode::FrostbiteMortar => 0.55,
            WeaponMode::SeismicDisrupter => 0.75,
            WeaponMode::TeslaCaster => 0.24,
            WeaponMode::PlasmaFlamer => 0.08, // Rapid continuous torrent
            WeaponMode::PlasmaWireSaw => 0.18, // Slicing sweep cadence
        };

        self.fire_cooldown = base_delay / self.config.fire_rate_mult;
        let mut feedback = FireFeedback::default();
        let horizontal_dir = Vec3::new(camera_dir.x, 0.0, camera_dir.z).normalize_or_zero();

        let base_recoil = match self.config.stabilizer {
            StabilizerType::Standard => 1.0,
            StabilizerType::HeavyCompensator => 0.40,
            StabilizerType::GyroscopicThruster => 0.85,
            StabilizerType::Overclocked => 1.40,
        };
        let recoil_factor = base_recoil / self.config.recoil_damping.max(0.2);
        let vel_factor = self.config.velocity_mult * match self.config.stabilizer {
            StabilizerType::Overclocked => 1.30,
            _ => 1.0,
        };
        let impulse_factor = self.config.impulse_mult * match self.config.stabilizer {
            StabilizerType::Overclocked => 1.30,
            _ => 1.0,
        };
        let gravity_mult = match self.config.stabilizer {
            StabilizerType::GyroscopicThruster => 0.20,
            _ => 1.0,
        };

        match self.config.mode {
            WeaponMode::TitanRam => {
                // Procedural Ram Slam: heavy forward thrust with spring-damper recovery
                self.recoil_pos = Vec3::new(0.0, -0.04, 0.35 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, -0.2, 2.2 * recoil_factor);
                self.recoil_rot = Vec3::new(10.0 * recoil_factor, 0.0, 0.0);
                self.recoil_rot_vel = Vec3::new(70.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.24 * recoil_factor;
                feedback.fov_kick = 3.5;
                feedback.player_recoil = -horizontal_dir * (0.95 * recoil_factor);

                audio.play(audio.hydraulic_slam.as_ref(), 0.65);
                audio.play(audio.ram_impact.as_ref(), 0.50);

                // Forward concussive hydraulic breach (radius 2.2m, high impulse)
                let cone_origin = camera_pos + camera_dir * 1.5;
                let breach_radius = 2.2 * self.config.blast_radius_mult;
                let blast_dist = 6.0 * self.config.blast_radius_mult;
                let steps = 12;
                let mut total_destroyed = 0;
                let mut ice_destroyed = 0;
                let mut explosives = Vec::new();
                let mut glow_cores = Vec::new();
                let mut capacitors = Vec::new();

                for s in 1..=steps {
                    let dist = (s as f32 / steps as f32) * blast_dist;
                    let radius = (1.1 + (dist / blast_dist) * 1.1).min(breach_radius);
                    let center = cone_origin + camera_dir * dist;
                    let res = grid.carve_sphere_advanced(center, radius, 220.0, crate::voxel::DamageType::Concussive);
                    total_destroyed += res.destroyed;
                    ice_destroyed += res.ice_shattered;
                    explosives.extend(res.explosive_triggers);
                    glow_cores.extend(res.glow_core_triggers);
                    capacitors.extend(res.capacitor_triggers);
                }

                if total_destroyed > 0 {
                    self.recent_hit = Some(if !explosives.is_empty() || !glow_cores.is_empty() || !capacitors.is_empty() {
                        crate::ui::HitKind::Critical
                    } else if ice_destroyed > 0 {
                        crate::ui::HitKind::CryoShatter
                    } else {
                        crate::ui::HitKind::Standard
                    });
                }

                if ice_destroyed > 0 {
                    let ice_hit = cone_origin + camera_dir * (blast_dist * 0.5);
                    physics.spawn_ice_shatter_effects(ice_hit, blast_dist, (ice_destroyed * 4).clamp(15, 60));
                    audio.play_spatial(audio.cryo_shatter.as_ref(), ice_hit, camera_pos, 0.55);
                }

                // Shove dynamic chunks and debris with immense forward kinetic impulse!
                physics.apply_kinetic_blast(
                    cone_origin,
                    camera_dir,
                    blast_dist + 3.0,
                    140.0 * impulse_factor,
                );
                physics.apply_radial_blast(
                    cone_origin + camera_dir * (blast_dist * 0.5),
                    breach_radius * 1.6,
                    75.0 * impulse_factor,
                );

                physics.add_explosion_effects(
                    cone_origin + camera_dir * (blast_dist * 0.5),
                    breach_radius * 1.4,
                    38.0 * impulse_factor,
                    self.config.mode.color(),
                    total_destroyed,
                );

                if let Some(wall_hit) = grid.raycast(cone_origin, camera_dir, blast_dist + 2.0) {
                    physics.spawn_decal_anchored(
                        wall_hit.world_pos,
                        wall_hit.normal,
                        breach_radius * 1.5,
                        Color::new(0.55, 0.55, 0.55, 0.85),
                        crate::physics::DecalKind::KineticImpact,
                        Some(wall_hit.voxel_pos),
                    );
                } else if let Some(anchor) = grid.find_nearest_solid(cone_origin + camera_dir * (blast_dist * 0.7), breach_radius) {
                    let anchor_pos = grid.voxel_to_world(anchor);
                    physics.spawn_decal_anchored(
                        anchor_pos,
                        -camera_dir,
                        breach_radius * 1.5,
                        Color::new(0.55, 0.55, 0.55, 0.85),
                        crate::physics::DecalKind::KineticImpact,
                        Some(anchor),
                    );
                }

                // Detonate any volatile fuel tanks and capacitors hit by breaching ram in an escalating chain!
                let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, explosives, glow_cores, capacitors, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_trauma * 0.35).min(0.55);
            }

            WeaponMode::HavocMortar => {
                self.recoil_pos = Vec3::new(0.0, 0.06 * recoil_factor, -0.32 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.35 * recoil_factor, -2.4 * recoil_factor);
                self.recoil_rot = Vec3::new(13.0 * recoil_factor, rand::gen_range(-1.0, 1.0), 0.0);
                self.recoil_rot_vel = Vec3::new(85.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.16 * recoil_factor;
                feedback.fov_kick = 3.0;
                feedback.player_recoil = -horizontal_dir * (1.05 * recoil_factor);

                audio.play(audio.mortar_launch.as_ref(), 0.65);

                let blast_r = 3.0 * self.config.blast_radius_mult;
                let impulse = 35.0 * impulse_factor;

                let launch_dirs = match self.config.barrel {
                    BarrelType::Precision | BarrelType::Piercing | BarrelType::ClusterSplit => vec![camera_dir],
                    BarrelType::TriSpread => {
                        let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                        vec![
                            camera_dir,
                            (camera_dir + right * 0.12).normalize_or_zero(),
                            (camera_dir - right * 0.12).normalize_or_zero(),
                        ]
                    }
                };

                for dir in launch_dirs {
                    self.projectiles.push(ActiveProjectile {
                        position: muzzle_pos,
                        velocity: dir * (34.0 * vel_factor),
                        gravity: -16.0 * gravity_mult,
                        radius: 0.35,
                        blast_radius: blast_r,
                        impulse,
                        mode: self.config.mode,
                        barrel: self.config.barrel,
                        payload: self.config.payload,
                        trail_style: self.config.trail_style,
                        color: self.config.mode.color(),
                        life: 8.0,
                        is_cluster_child: false,
                        is_airburst: false,
                        is_sticky: self.config.detonator == DetonatorType::RemoteSticky,
                        stuck_normal: Vec3::ZERO,
                        trail_points: TrailBuffer::single(muzzle_pos),
                    });
                }
            }

            WeaponMode::FrostbiteMortar => {
                self.recoil_pos = Vec3::new(0.0, 0.04 * recoil_factor, -0.22 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.25 * recoil_factor, -1.8 * recoil_factor);
                self.recoil_rot = Vec3::new(8.0 * recoil_factor, rand::gen_range(-0.8, 0.8), 0.0);
                self.recoil_rot_vel = Vec3::new(55.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.12 * recoil_factor;
                feedback.fov_kick = 2.0;
                feedback.player_recoil = -horizontal_dir * (0.65 * recoil_factor);

                audio.play(audio.mortar_launch.as_ref(), 0.55);

                let blast_r = 3.5 * self.config.blast_radius_mult;
                let impulse = 20.0 * impulse_factor;

                let launch_dirs = match self.config.barrel {
                    BarrelType::Precision | BarrelType::Piercing | BarrelType::ClusterSplit => vec![camera_dir],
                    BarrelType::TriSpread => {
                        let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                        vec![
                            camera_dir,
                            (camera_dir + right * 0.12).normalize_or_zero(),
                            (camera_dir - right * 0.12).normalize_or_zero(),
                        ]
                    }
                };

                for dir in launch_dirs {
                    self.projectiles.push(ActiveProjectile {
                        position: muzzle_pos,
                        velocity: dir * (36.0 * vel_factor),
                        gravity: -12.0 * gravity_mult,
                        radius: 0.32,
                        blast_radius: blast_r,
                        impulse,
                        mode: self.config.mode,
                        barrel: self.config.barrel,
                        payload: self.config.payload,
                        trail_style: self.config.trail_style,
                        color: self.config.mode.color(),
                        life: 8.0,
                        is_cluster_child: false,
                        is_airburst: false,
                        is_sticky: self.config.detonator == DetonatorType::RemoteSticky,
                        stuck_normal: Vec3::ZERO,
                        trail_points: TrailBuffer::single(muzzle_pos),
                    });
                }
            }

            WeaponMode::EventHorizon => {
                self.recoil_pos = Vec3::new(0.0, 0.03 * recoil_factor, -0.16 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.2 * recoil_factor, -1.2 * recoil_factor);
                self.recoil_rot = Vec3::new(5.0 * recoil_factor, 0.0, 0.0);
                self.recoil_rot_vel = Vec3::new(35.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.10 * recoil_factor;
                feedback.fov_kick = -2.2; // Space-warping contraction
                feedback.player_recoil = Vec3::ZERO;

                audio.play(audio.singularity_launch.as_ref(), 0.65);

                self.projectiles.push(ActiveProjectile {
                    position: muzzle_pos,
                    velocity: camera_dir * (26.0 * vel_factor),
                    gravity: 0.0,
                    radius: 0.45,
                    blast_radius: 3.8 * self.config.blast_radius_mult,
                    impulse: 45.0 * impulse_factor,
                    mode: WeaponMode::EventHorizon,
                    barrel: self.config.barrel,
                    payload: self.config.payload,
                    trail_style: self.config.trail_style,
                    color: Color::new(0.8, 0.2, 1.0, 1.0),
                    life: 8.0,
                    is_cluster_child: false,
                    is_airburst: false,
                    is_sticky: false,
                    stuck_normal: Vec3::ZERO,
                    trail_points: TrailBuffer::single(muzzle_pos),
                });
            }

            WeaponMode::SolsticeLance => {
                // Surgical continuous thermal cutting laser (zero screen bounce, rock-steady precision)
                self.is_firing_laser = true;
                self.heat = (self.heat + 0.035).min(1.0);
                self.recoil_pos = Vec3::new(
                    0.0,
                    0.0,
                    -0.005 * recoil_factor,
                );
                self.recoil_rot = Vec3::ZERO;
                feedback.camera_trauma = 0.0;
                feedback.fov_kick = 0.0;
                feedback.player_recoil = Vec3::ZERO;

                if self.laser_sound_timer <= 0.0 {
                    audio.play(audio.plasma_buzz.as_ref(), 0.35);
                    self.laser_sound_timer = 0.12;
                }

                let max_range = 75.0;
                let ray_end = camera_pos + camera_dir * max_range;
                let grid_hit = grid.raycast(camera_pos, camera_dir, max_range);
                let chunk_hit = physics.check_chunk_hit(camera_pos, ray_end);

                let hit_chunk = match (&grid_hit, &chunk_hit) {
                    (None, Some(_)) => true,
                    (Some(ghit), Some((cpos, _))) => {
                        (*cpos - camera_pos).length_squared() < (ghit.world_pos - camera_pos).length_squared()
                    }
                    _ => false,
                };

                if hit_chunk {
                    let (chit_pos, chit_norm) = chunk_hit.unwrap();
                    self.laser_hit_pos = Some(chit_pos);
                    let drill_radius = 0.65 * self.config.blast_radius_mult;
                    let (destroyed, dynamic_triggers) = physics.carve_chunk_sphere(
                        grid, chit_pos, drill_radius, 140.0, DamageType::Thermal, true,
                    );
                    physics.spawn_molten_fissure(chit_pos, chit_norm, 6);
                    physics.apply_kinetic_blast(
                        chit_pos - camera_dir * 0.5,
                        camera_dir,
                        2.5,
                        15.0 * impulse_factor,
                    );
                    if destroyed > 0 {
                        self.recent_hit = Some(if dynamic_triggers.is_empty() {
                            crate::ui::HitKind::Standard
                        } else {
                            crate::ui::HitKind::Critical
                        });
                        physics.add_explosion_effects(
                            chit_pos, drill_radius * 0.8, 10.0 * impulse_factor,
                            Color::new(0.2, 0.95, 1.0, 1.0), destroyed,
                        );
                    }
                    let chain_trauma = process_explosive_chain_at_world_positions(
                        grid, physics, audio, Vec::new(), Vec::new(), Vec::new(),
                        dynamic_triggers, camera_pos,
                    );
                    feedback.camera_trauma = (feedback.camera_trauma + chain_trauma * 0.25).min(0.20);
                } else if let Some(ghit) = grid_hit {
                    let hit_pos = ghit.world_pos;
                    let hit_norm = ghit.normal;
                    self.laser_hit_pos = Some(hit_pos);

                    if ghit.voxel_type == crate::voxel::VoxelType::Bedrock {
                        physics.spawn_molten_fissure(hit_pos, hit_norm, 3);
                    } else {
                        // Surgical notch-cutting thermal ablation (radius 0.65m)
                        let drill_radius = 0.65 * self.config.blast_radius_mult;
                        let res = grid.carve_sphere_advanced(hit_pos, drill_radius, 140.0, crate::voxel::DamageType::Thermal);
                        physics.spawn_molten_fissure(hit_pos, hit_norm, 10);
                        spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, drill_radius * 1.5, Color::new(0.25, 0.9, 1.0, 0.95), crate::physics::DecalKind::PlasmaBurn);

                        if res.destroyed > 0 {
                            self.recent_hit = Some(if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() {
                                crate::ui::HitKind::Critical
                            } else if res.ice_shattered > 0 {
                                crate::ui::HitKind::CryoShatter
                            } else {
                                crate::ui::HitKind::Standard
                            });

                            physics.add_explosion_effects(
                                hit_pos,
                                drill_radius * 0.8,
                                10.0 * impulse_factor,
                                Color::new(0.2, 0.95, 1.0, 1.0),
                                res.destroyed,
                            );
                            // Forward vapor-pressure push on nearby chunks
                            physics.apply_kinetic_blast(
                                hit_pos - camera_dir * 0.5,
                                camera_dir,
                                drill_radius + 2.0,
                                20.0 * impulse_factor,
                            );
                        }

                        if res.ice_shattered > 0 {
                            physics.spawn_ice_shatter_effects(hit_pos, drill_radius * 1.5, (res.ice_shattered * 3).clamp(10, 40));
                            audio.play_spatial(audio.cryo_shatter.as_ref(), hit_pos, camera_pos, 0.50);
                        }

                        // Detonate volatile fuel conduits, glow cores, and power capacitors when laser cuts into them
                        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                        feedback.camera_trauma = (feedback.camera_trauma + chain_trauma * 0.25).min(0.20);
                    }
                } else {
                    self.laser_hit_pos = Some(camera_pos + camera_dir * max_range);
                }
            }

            WeaponMode::ApexRailgun => {
                self.heat = (self.heat + 0.35).min(1.0);
                self.recoil_pos = Vec3::new(0.0, 0.02 * recoil_factor, -0.22 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.2 * recoil_factor, -2.5 * recoil_factor);
                self.recoil_rot = Vec3::new(6.0 * recoil_factor, 0.0, 0.0);
                self.recoil_rot_vel = Vec3::new(50.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.04 * recoil_factor; // Crisp mechanical snap, minimal shake
                feedback.fov_kick = 1.6;
                feedback.player_recoil = -horizontal_dir * (0.35 * recoil_factor);

                audio.play(audio.railgun_boom.as_ref(), 0.70);

                let max_range = 100.0;
                let tunnel_radius = 0.75 * self.config.blast_radius_mult;
                let ray_start = camera_pos + camera_dir * (tunnel_radius + 0.5);
                let ray_end = camera_pos + camera_dir * max_range;
                let entry_hit = grid.raycast(camera_pos, camera_dir, max_range);

                // Bore tunnel clean through multiple walls with sniper-like precision
                let res = grid.carve_cylinder_advanced(ray_start, ray_end, tunnel_radius, 400.0, crate::voxel::DamageType::Kinetic);

                if res.destroyed > 0 {
                    self.recent_hit = Some(if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() {
                        crate::ui::HitKind::Critical
                    } else if res.ice_shattered > 0 {
                        crate::ui::HitKind::CryoShatter
                    } else {
                        crate::ui::HitKind::Standard
                    });
                }

                if res.ice_shattered > 0 {
                    let ice_hit = camera_pos + camera_dir * 14.0;
                    physics.spawn_ice_shatter_effects(ice_hit, 6.0, (res.ice_shattered * 4).clamp(20, 60));
                    audio.play_spatial(audio.cryo_shatter.as_ref(), ice_hit, camera_pos, 0.55);
                }

                self.tracers.push(RailgunTracer {
                    start: muzzle_pos,
                    end: ray_end,
                    life: 0.50,
                    max_life: 0.50,
                    color: Color::new(0.4, 0.85, 1.0, 1.0),
                });

                physics.apply_beam_shockwave(
                    ray_start,
                    ray_end,
                    7.5,
                    55.0 * impulse_factor,
                );

                let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_trauma * 0.30).min(0.25);

                if let Some(entry) = entry_hit {
                    physics.add_explosion_effects(
                        entry.world_pos,
                        tunnel_radius * 2.0,
                        35.0 * impulse_factor,
                        Color::new(0.35, 0.8, 1.0, 1.0),
                        (res.destroyed / 2).max(10),
                    );
                    physics.apply_radial_blast(entry.world_pos, tunnel_radius * 2.5, 45.0 * impulse_factor);
                    if let Some(anchor) = grid.find_nearest_solid(entry.world_pos, tunnel_radius + 1.2) {
                        let a_pos = grid.voxel_to_world(anchor);
                        physics.spawn_decal_anchored(a_pos, entry.normal, tunnel_radius * 2.2, Color::new(0.3, 0.8, 1.0, 0.95), crate::physics::DecalKind::KineticImpact, Some(anchor));
                    }

                    let exit_probe = entry.world_pos + camera_dir * 18.0;
                    if let Some(exit_back) = grid.raycast(exit_probe, -camera_dir, 16.0) {
                        physics.add_explosion_effects(
                            exit_back.world_pos,
                            tunnel_radius * 2.6,
                            45.0 * impulse_factor,
                            Color::new(0.4, 0.9, 1.0, 1.0),
                            (res.destroyed / 2).max(10),
                        );
                        physics.apply_kinetic_blast(exit_back.world_pos, camera_dir, tunnel_radius * 3.5, 80.0 * impulse_factor);
                        physics.spawn_decal_anchored(exit_back.world_pos, exit_back.normal, tunnel_radius * 2.4, Color::new(0.4, 0.85, 1.0, 0.9), crate::physics::DecalKind::PlasmaBurn, Some(exit_back.voxel_pos));
                    }
                } else {
                    physics.add_explosion_effects(
                        camera_pos + camera_dir * 14.0,
                        tunnel_radius * 2.2,
                        32.0 * impulse_factor,
                        Color::new(0.35, 0.8, 1.0, 1.0),
                        res.destroyed,
                    );
                }
            }

            WeaponMode::SeismicDisrupter => {
                self.recoil_pos = Vec3::new(0.0, 0.07 * recoil_factor, -0.36 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.45 * recoil_factor, -2.4 * recoil_factor);
                self.recoil_rot = Vec3::new(14.0 * recoil_factor, 0.0, 0.0);
                self.recoil_rot_vel = Vec3::new(90.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.18 * recoil_factor;
                feedback.fov_kick = 2.8;
                feedback.player_recoil = -horizontal_dir * (1.0 * recoil_factor);

                audio.play(audio.seismic_pulse.as_ref(), 0.65);

                let blast_r = 3.6 * self.config.blast_radius_mult;
                let impulse = 45.0 * impulse_factor;

                let launch_dirs = match self.config.barrel {
                    BarrelType::Precision | BarrelType::Piercing | BarrelType::ClusterSplit => vec![camera_dir],
                    BarrelType::TriSpread => {
                        let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                        vec![
                            camera_dir,
                            (camera_dir + right * 0.14).normalize_or_zero(),
                            (camera_dir - right * 0.14).normalize_or_zero(),
                        ]
                    }
                };

                for dir in launch_dirs {
                    self.projectiles.push(ActiveProjectile {
                        position: muzzle_pos,
                        velocity: dir * (30.0 * vel_factor),
                        gravity: -10.0 * gravity_mult,
                        radius: 0.38,
                        blast_radius: blast_r,
                        impulse,
                        mode: self.config.mode,
                        barrel: self.config.barrel,
                        payload: self.config.payload,
                        trail_style: self.config.trail_style,
                        color: self.config.mode.color(),
                        life: 8.0,
                        is_cluster_child: false,
                        is_airburst: false,
                        is_sticky: self.config.detonator == DetonatorType::RemoteSticky,
                        stuck_normal: Vec3::ZERO,
                        trail_points: TrailBuffer::single(muzzle_pos),
                    });
                }
            }

            WeaponMode::TeslaCaster => {
                self.heat = (self.heat + 0.10).min(1.0);
                self.recoil_pos = Vec3::new(rand::gen_range(-0.006, 0.006), 0.015, -0.10 * recoil_factor);
                self.recoil_pos_vel = Vec3::new(0.0, 0.1 * recoil_factor, -recoil_factor);
                self.recoil_rot = Vec3::new(4.0 * recoil_factor, 0.0, 0.0);
                self.recoil_rot_vel = Vec3::new(30.0 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.05 * recoil_factor;
                feedback.fov_kick = 0.8;
                feedback.player_recoil = -horizontal_dir * (0.20 * recoil_factor);

                audio.play(audio.tesla_arc.as_ref(), 0.50);

                let max_range = 80.0;
                let ray_end = camera_pos + camera_dir * max_range;
                let mut hit_target = grid.raycast(camera_pos, camera_dir, max_range).map(|h| (h.world_pos, h.normal));
                if let Some(chunk_hit) = physics.check_chunk_hit(camera_pos, ray_end) {
                    if hit_target.is_none_or(|(pos, _)| camera_pos.distance_squared(chunk_hit.0) < camera_pos.distance_squared(pos)) {
                        hit_target = Some(chunk_hit);
                    }
                }

                if let Some((hit_pos, hit_norm)) = hit_target {
                    let mut branches = Vec::new();
                    let mut curr = muzzle_pos;
                    let main_steps = 7;
                    let delta = hit_pos - muzzle_pos;
                    for s in 1..=main_steps {
                        let frac = s as f32 / main_steps as f32;
                        let target_step = muzzle_pos + delta * frac;
                        let jitter = if s < main_steps {
                            Vec3::new(rand::gen_range(-0.16, 0.16), rand::gen_range(-0.16, 0.16), rand::gen_range(-0.16, 0.16))
                        } else {
                            Vec3::ZERO
                        };
                        let next_pt = target_step + jitter;
                        branches.push((curr, next_pt));
                        curr = next_pt;
                    }

                    let search_r = 5.0 * self.config.blast_radius_mult;
                    let res = grid.carve_sphere_advanced(hit_pos, 1.2 * self.config.blast_radius_mult, 180.0, crate::voxel::DamageType::Kinetic);
                    physics.add_explosion_effects(hit_pos, 1.0, 15.0 * impulse_factor, Color::new(0.4, 0.7, 1.0, 1.0), res.destroyed.max(8));
                    spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, 1.4, Color::new(0.4, 0.75, 1.0, 0.95), crate::physics::DecalKind::PlasmaBurn);

                    let center_v = grid.world_to_voxel(hit_pos);
                    let r_vox = (search_r / crate::voxel::VOXEL_SIZE) as i32;
                    let mut metal_nodes = Vec::new();
                    for dx in -r_vox..=r_vox {
                        for dy in -r_vox..=r_vox {
                            for dz in -r_vox..=r_vox {
                                let vt = grid.get(center_v.x + dx, center_v.y + dy, center_v.z + dz);
                                let cond = vt.properties().conductivity;
                                if cond > 0.0 {
                                    let node_pos = grid.voxel_to_world(IVec3::new(center_v.x + dx, center_v.y + dy, center_v.z + dz));
                                    metal_nodes.push((node_pos, cond));
                                }
                            }
                        }
                    }
                    metal_nodes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    let selected_nodes: Vec<Vec3> = metal_nodes.into_iter().take(4).map(|(p, _)| p).collect();

                    for node in selected_nodes {
                        branches.push((hit_pos, node));
                        let sub_res = grid.carve_sphere_advanced(node, 0.8, 140.0, crate::voxel::DamageType::Kinetic);
                        if sub_res.destroyed > 0 {
                            physics.add_explosion_effects(node, 0.6, 10.0, Color::new(0.5, 0.8, 1.0, 1.0), sub_res.destroyed);
                        }
                    }

                    self.tesla_arcs.push(TeslaArcTracer {
                        branches,
                        life: 0.18,
                        max_life: 0.18,
                        color: Color::new(0.4, 0.75, 1.0, 0.95),
                    });

                    if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() || !res.capacitor_triggers.is_empty() {
                        self.recent_hit = Some(crate::ui::HitKind::Critical);
                    } else if res.destroyed > 0 {
                        self.recent_hit = Some(crate::ui::HitKind::Standard);
                    }

                    let chain_trauma = process_explosive_chain_with_capacitors(
                        grid,
                        physics,
                        audio,
                        res.explosive_triggers,
                        res.glow_core_triggers,
                        res.capacitor_triggers,
                        camera_pos,
                    );
                    feedback.camera_trauma = (feedback.camera_trauma + chain_trauma * 0.35).min(0.25);
                } else {
                    let branches = vec![(muzzle_pos, ray_end)];
                    self.tesla_arcs.push(TeslaArcTracer {
                        branches,
                        life: 0.14,
                        max_life: 0.14,
                        color: Color::new(0.3, 0.6, 1.0, 0.8),
                    });
                }
            }

            WeaponMode::PlasmaFlamer => {
                self.heat = (self.heat + 0.025).min(1.0);
                self.recoil_pos = Vec3::new(rand::gen_range(-0.003, 0.003), rand::gen_range(-0.003, 0.003), -0.03 * recoil_factor);
                self.recoil_rot = Vec3::new(1.2 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.010 * recoil_factor;
                feedback.fov_kick = 0.2;
                feedback.player_recoil = -horizontal_dir * (0.04 * recoil_factor);

                audio.play(audio.plasma_fire.as_ref(), 0.35);

                let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                let up = right.cross(camera_dir).normalize_or_zero();
                let spread = (camera_dir + right * rand::gen_range(-0.03, 0.03) + up * rand::gen_range(-0.03, 0.03)).normalize_or_zero();

                self.projectiles.push(ActiveProjectile {
                    position: muzzle_pos,
                    velocity: spread * (40.0 * vel_factor),
                    gravity: -6.0 * gravity_mult,
                    radius: 0.28,
                    blast_radius: 1.8 * self.config.blast_radius_mult,
                    impulse: 14.0 * impulse_factor,
                    mode: WeaponMode::PlasmaFlamer,
                    barrel: self.config.barrel,
                    payload: self.config.payload,
                    trail_style: self.config.trail_style,
                    color: WeaponMode::PlasmaFlamer.color(),
                    life: 4.0,
                    is_cluster_child: false,
                    is_airburst: false,
                    is_sticky: false,
                    stuck_normal: Vec3::ZERO,
                    trail_points: TrailBuffer::single(muzzle_pos),
                });
            }

            WeaponMode::PlasmaWireSaw => {
                self.heat = (self.heat + 0.05).min(1.0);
                self.recoil_pos = Vec3::new(0.0, 0.0, -0.05 * recoil_factor);
                self.recoil_rot = Vec3::new(2.2 * recoil_factor, 0.0, 0.0);
                feedback.camera_trauma = 0.03 * recoil_factor;
                feedback.fov_kick = 0.5;
                feedback.player_recoil = -horizontal_dir * (0.06 * recoil_factor);

                audio.play(audio.plasma_buzz.as_ref(), 0.65);
                audio.play(audio.tesla_arc.as_ref(), 0.40);

                let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                let up = right.cross(camera_dir).normalize_or_zero();

                // Blade plane normal (0 = Horizontal slice, 1 = Vertical slice, 2 = Diagonal 45° slice)
                let blade_norm = match self.slice_plane_mode % 3 {
                    0 => up,
                    1 => right,
                    _ => (right + up).normalize_or_zero(),
                };

                let reach = 28.0;
                // 1. Slice standing structures in static grid
                let grid_cuts = grid.slice_planar_kerf(camera_pos, camera_dir, blade_norm, reach, 0.45);

                // 2. Bisect dynamic rigid body chunks
                let chunk_cuts = physics.slice_intersecting_chunks(camera_pos + camera_dir * 8.0, blade_norm, reach);
                if chunk_cuts > 0 {
                    physics.rebuild_rubble_heightmap_with_grid(grid);
                    physics.rubble_dirty = true;
                    physics.rubble_dirty_region = None;
                }

                if grid_cuts > 0 || chunk_cuts > 0 {
                    self.recent_hit = Some(crate::ui::HitKind::Critical);
                }

                // Plasma cutting beam flash
                let cut_center = camera_pos + camera_dir * 12.0;
                physics.sparks.push(crate::physics::SparkParticle {
                    position: cut_center,
                    velocity: camera_dir * 14.0,
                    life: 0.22,
                    max_life: 0.22,
                    color: WeaponMode::PlasmaWireSaw.color(),
                });
            }
        }

        feedback
    }

    pub fn fire_alt(
        &mut self,
        muzzle_pos: Vec3,
        camera_dir: Vec3,
        camera_pos: Vec3,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
    ) -> (FireFeedback, &'static str) {
        if self.fire_cooldown > 0.0 {
            return (FireFeedback::default(), "");
        }

        let mut feedback = FireFeedback::default();
        let notif: &'static str;

        match self.config.mode {
            WeaponMode::ApexRailgun => {
                // Apex Railgun: Overcharge Hyper-Slug
                // Carves 14-wall deep beam with 3x radius and massive kinetic force
                self.fire_cooldown = 1.10 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.65;
                feedback.fov_kick = 5.5;
                feedback.player_recoil = -camera_dir * 3.2;

                audio.play(audio.railgun_boom.as_ref(), 0.75);
                audio.play(audio.subsonic_artillery_boom.as_ref(), 0.65);

                let max_range = 80.0f32;
                let step = 1.0f32;
                let steps = (max_range / step) as usize;
                let mut current_pos = muzzle_pos;
                let mut penetrated_walls = 0;
                let mut all_exp = Vec::new();
                let mut all_cores = Vec::new();
                let mut all_caps = Vec::new();

                for _ in 0..steps {
                    current_pos += camera_dir * step;
                    let res = grid.carve_sphere_advanced(current_pos, 2.2, 1800.0, DamageType::Kinetic);
                    if res.destroyed > 0 {
                        penetrated_walls += 1;
                        physics.spawn_spall_impact(current_pos, -camera_dir, Color::new(0.4, 0.85, 1.0, 1.0));
                        physics.spawn_molten_fissure(current_pos, -camera_dir, 15);
                        physics.apply_kinetic_blast(current_pos, camera_dir, 6.0, 85.0);
                        all_exp.extend(res.explosive_triggers);
                        all_cores.extend(res.glow_core_triggers);
                        all_caps.extend(res.capacitor_triggers);
                        if penetrated_walls >= 14 {
                            break;
                        }
                    }
                }

                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: current_pos,
                    normal: camera_dir,
                    radius: 0.6,
                    max_radius: 6.5,
                    speed: 36.0,
                    color: Color::new(0.35, 0.85, 1.0, 1.0),
                    thickness: 0.40,
                });

                self.tracers.push(RailgunTracer {
                    start: muzzle_pos,
                    end: current_pos,
                    life: 0.55,
                    max_life: 0.55,
                    color: Color::new(0.5, 0.95, 1.0, 1.0),
                });

                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, all_exp, all_cores, all_caps, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: APEX HYPER-SLUG OVERCHARGE";
            }
            WeaponMode::HavocMortar => {
                // Havoc Mortar: Airburst Cluster
                // Launches an airburst shell that detonates at apex, dispersing 6 cluster sub-munitions
                self.fire_cooldown = 0.95 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.45;
                feedback.fov_kick = 4.2;
                feedback.player_recoil = -camera_dir * 2.0;

                audio.play(audio.mortar_launch.as_ref(), 0.70);

                let vel = camera_dir * 40.0;
                self.projectiles.push(ActiveProjectile {
                    position: muzzle_pos,
                    velocity: vel,
                    gravity: -16.0,
                    radius: 0.38,
                    blast_radius: 3.5,
                    impulse: 55.0,
                    mode: WeaponMode::HavocMortar,
                    barrel: BarrelType::Precision,
                    payload: PayloadType::HighExplosive,
                    trail_style: TrailStyle::SmokeWisps,
                    color: Color::new(1.0, 0.60, 0.15, 1.0),
                    life: 8.0,
                    is_cluster_child: false,
                    is_airburst: true,
                    is_sticky: false,
                    stuck_normal: Vec3::ZERO,
                    trail_points: TrailBuffer::single(muzzle_pos),
                });
                notif = "ALT-FIRE: HAVOC AIRBURST CLUSTER";
            }
            WeaponMode::TitanRam => {
                // Titan Ram: Seismic Ground Ring
                // 360-degree cylindrical shockwave carving ground-level columns
                self.fire_cooldown = 0.90 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.60;
                feedback.fov_kick = 5.0;
                feedback.player_recoil = Vec3::new(0.0, 1.8, 0.0);

                audio.play(audio.hydraulic_slam.as_ref(), 0.75);
                audio.play(audio.seismic_pulse.as_ref(), 0.65);

                let ground_y = 0.50f32;
                let ground_origin = Vec3::new(camera_pos.x, ground_y, camera_pos.z);

                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: ground_origin,
                    normal: Vec3::Y,
                    radius: 0.5,
                    max_radius: 12.0,
                    speed: 26.0,
                    color: Color::new(1.0, 0.50, 0.15, 1.0),
                    thickness: 0.50,
                });
                physics.apply_radial_blast(ground_origin, 12.0, 75.0);

                let mut all_exp = Vec::new();
                let mut all_cores = Vec::new();
                let mut all_caps = Vec::new();

                for i in 0..16 {
                    let angle = (i as f32 / 16.0) * std::f32::consts::TAU;
                    let ring_pt = ground_origin + Vec3::new(angle.cos() * 5.5, 0.5, angle.sin() * 5.5);
                    let res = grid.carve_sphere_advanced(ring_pt, 2.0, 800.0, DamageType::Kinetic);
                    physics.add_explosion_effects(ring_pt, 2.2, 38.0, Color::new(1.0, 0.45, 0.15, 1.0), res.destroyed);
                    all_exp.extend(res.explosive_triggers);
                    all_cores.extend(res.glow_core_triggers);
                    all_caps.extend(res.capacitor_triggers);
                }

                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, all_exp, all_cores, all_caps, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: TITAN SEISMIC GROUND RING";
            }
            WeaponMode::EventHorizon => {
                // Event Horizon: Gravitational Pulse
                // Collapses active black holes early or triggers instant inward implosion
                self.fire_cooldown = 1.20 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.70;
                feedback.fov_kick = 6.0;

                audio.play(audio.singularity_launch.as_ref(), 0.70);
                audio.play(audio.singularity_implode.as_ref(), 0.75);

                if !self.singularities.is_empty() {
                    for sing in &mut self.singularities {
                        sing.timer = sing.max_time; // Force instant collapse
                    }
                    notif = "ALT-FIRE: GRAVITATIONAL PULSE DETONATION";
                } else {
                    let pulse_pos = camera_pos + camera_dir * 12.0;
                    physics.apply_vortex_suction(pulse_pos, 16.0, 85.0, 0.15);
                    let res = grid.carve_sphere_advanced(pulse_pos, 4.5, 950.0, DamageType::Thermal);
                    physics.shockwaves.push(crate::physics::ShockwaveRing {
                        position: pulse_pos,
                        normal: camera_dir,
                        radius: 0.5,
                        max_radius: 10.0,
                        speed: 32.0,
                        color: Color::new(0.80, 0.25, 1.0, 1.0),
                        thickness: 0.45,
                    });
                    physics.add_explosion_effects(pulse_pos, 4.5, 55.0, Color::new(0.75, 0.2, 0.95, 1.0), res.destroyed);
                    let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                    feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                    notif = "ALT-FIRE: GRAVITATIONAL IMPLOSION PULSE";
                }
            }
            WeaponMode::SolsticeLance => {
                // Solstice Lance: Molten Searing Slice
                // Intensified fan blade slice searing through structural columns
                self.fire_cooldown = 0.75 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.40;
                feedback.fov_kick = 3.2;
                feedback.player_recoil = -camera_dir * 1.2;

                audio.play(audio.plasma_buzz.as_ref(), 0.70);
                audio.play(audio.railgun_boom.as_ref(), 0.55);

                let right = camera_dir.cross(Vec3::Y).normalize_or_zero();
                let mut all_exp = Vec::new();
                let mut all_cores = Vec::new();
                let mut all_caps = Vec::new();

                for i in -3..=3 {
                    let fan_dir = (camera_dir + right * (i as f32 * 0.12)).normalize();
                    let slice_target = camera_pos + fan_dir * 32.0;
                    let res = grid.carve_sphere_advanced(camera_pos + fan_dir * 14.0, 1.8, 550.0, DamageType::Thermal);
                    physics.spawn_molten_fissure(camera_pos + fan_dir * 14.0, Vec3::Y, 12);
                    self.tracers.push(RailgunTracer {
                        start: muzzle_pos,
                        end: slice_target,
                        life: 0.35,
                        max_life: 0.35,
                        color: Color::new(0.2, 0.95, 1.0, 0.9),
                    });
                    all_exp.extend(res.explosive_triggers);
                    all_cores.extend(res.glow_core_triggers);
                    all_caps.extend(res.capacitor_triggers);
                }

                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, all_exp, all_cores, all_caps, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: SOLSTICE MOLTEN SEARING SLICE";
            }
            WeaponMode::FrostbiteMortar => {
                self.fire_cooldown = 0.80 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.45;
                let burst_pos = camera_pos + camera_dir * 16.0;
                physics.spawn_ice_shatter_effects(burst_pos, 5.0, 45);
                audio.play_spatial(audio.cryo_freeze.as_ref(), burst_pos, camera_pos, 0.75);
                audio.play_spatial(audio.cryo_shatter.as_ref(), burst_pos, camera_pos, 0.65);
                let res = grid.carve_sphere_advanced(burst_pos, 4.0, 400.0, DamageType::Cryo);
                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: FROSTBITE CRYO-BLIZZARD";
            }
            WeaponMode::SeismicDisrupter => {
                self.fire_cooldown = 0.90 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.55;
                audio.play(audio.seismic_pulse.as_ref(), 0.75);
                let hit_pos = camera_pos + camera_dir * 20.0;
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: hit_pos,
                    normal: Vec3::Y,
                    radius: 0.5,
                    max_radius: 14.0,
                    speed: 25.0,
                    color: Color::new(0.95, 0.75, 0.2, 1.0),
                    thickness: 0.5,
                });
                physics.apply_radial_blast(hit_pos, 10.0, 70.0);
                let res = grid.carve_sphere_advanced(hit_pos, 3.5, 600.0, DamageType::Kinetic);
                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: TECTONIC FAULT RUPTURE";
            }
            WeaponMode::TeslaCaster => {
                self.fire_cooldown = 0.60 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.40;
                audio.play(audio.tesla_arc.as_ref(), 0.75);
                let hit_pos = camera_pos + camera_dir * 18.0;
                physics.add_explosion_effects(hit_pos, 3.0, 35.0, Color::new(0.65, 0.35, 1.0, 1.0), 30);
                for _ in 0..4 {
                    let arc_end = hit_pos + Vec3::new(
                        macroquad::rand::gen_range(-4.0f32, 4.0),
                        macroquad::rand::gen_range(-2.0f32, 4.0),
                        macroquad::rand::gen_range(-4.0f32, 4.0),
                    );
                    physics.spawn_electrical_arc(hit_pos, arc_end, 0.4, Color::new(0.65, 0.40, 1.0, 1.0));
                }
                let res = grid.carve_sphere_advanced(hit_pos, 3.0, 450.0, DamageType::Thermal);
                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: TESLA OVERLOAD ARC STORM";
            }
            WeaponMode::PlasmaFlamer => {
                self.fire_cooldown = 0.50 / self.config.fire_rate_mult;
                feedback.camera_trauma = 0.35;
                audio.play(audio.plasma_fire.as_ref(), 0.70);
                let wall_pos = camera_pos + camera_dir * 10.0;
                for _ in 0..3 {
                    physics.spawn_fire_jet(wall_pos, camera_dir, 1.8);
                }
                let res = grid.carve_sphere_advanced(wall_pos, 3.0, 500.0, DamageType::Thermal);
                let chain_t = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                feedback.camera_trauma = (feedback.camera_trauma + chain_t).min(1.0);
                notif = "ALT-FIRE: THERMOBARIC WALL OF FLAME";
            }
            WeaponMode::PlasmaWireSaw => {
                self.slice_plane_mode = (self.slice_plane_mode + 1) % 3;
                audio.play(audio.ui_switch.as_ref(), 0.70);
                notif = match self.slice_plane_mode {
                    0 => "PLASMA SAW PLANE: HORIZONTAL SLICE",
                    1 => "PLASMA SAW PLANE: VERTICAL SLICE",
                    _ => "PLASMA SAW PLANE: DIAGONAL 45° SLICE",
                };
            }
        }

        (feedback, notif)
    }

    // Secondary Fire: Detonate all placed remote sticky charges or trigger weapon alt-fire!
    pub fn fire_secondary(
        &mut self,
        muzzle_pos: Vec3,
        camera_dir: Vec3,
        camera_pos: Vec3,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
    ) -> (FireFeedback, &'static str) {
        if !self.sticky_charges.is_empty() {
            let charges = std::mem::take(&mut self.sticky_charges);
            let avg_pos = charges.iter().fold(Vec3::ZERO, |acc, c| acc + c.position) / (charges.len() as f32);
            audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), avg_pos, camera_pos, 0.65);
            audio.play_spatial(audio.big_boom.as_ref(), avg_pos, camera_pos, 0.55);

            let mut all_exp = Vec::new();
            let mut all_cores = Vec::new();
            let mut all_caps = Vec::new();
            for charge in charges {
                let res = grid.carve_sphere_advanced(charge.position, charge.blast_radius, 340.0, crate::voxel::DamageType::Explosive);
                if res.destroyed > 0 {
                    physics.rubble_dirty = true;
                    let min_p = grid.world_to_voxel(charge.position - Vec3::splat(charge.blast_radius + 1.0));
                    let max_p = grid.world_to_voxel(charge.position + Vec3::splat(charge.blast_radius + 1.0));
                    physics.mark_rubble_region_dirty(min_p, max_p);
                }

                if res.ice_shattered > 0 {
                    physics.spawn_ice_shatter_effects(charge.position, charge.blast_radius, (res.ice_shattered * 4).clamp(15, 50));
                    audio.play_spatial(audio.cryo_shatter.as_ref(), charge.position, camera_pos, 0.50);
                }

                physics.apply_radial_blast(charge.position, charge.blast_radius * 1.6, charge.impulse * 2.0);

                physics.add_explosion_effects(
                    charge.position,
                    charge.blast_radius,
                    charge.impulse * 1.6,
                    charge.color,
                    res.destroyed,
                );

                all_exp.extend(res.explosive_triggers);
                all_cores.extend(res.glow_core_triggers);
                all_caps.extend(res.capacitor_triggers);
            }

            let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, all_exp, all_cores, all_caps, camera_pos);
            let feedback = FireFeedback {
                camera_trauma: (0.65 + chain_trauma).min(1.0),
                player_recoil: Vec3::ZERO,
                fov_kick: 4.0,
            };
            (feedback, "REMOTE DEMOLITION CHARGES DETONATED!")
        } else {
            self.fire_alt(muzzle_pos, camera_dir, camera_pos, grid, physics, audio)
        }
    }

    /// Computes the Relative Neighborhood Graph (RNG) for active C4 charges
    /// with Euclidean distance <= MAX_DET_CORD_DIST (25.0m).
    pub fn compute_det_cord_links(&self) -> Vec<DetCordLink> {
        let n = self.c4_charges.len();
        if n < 2 {
            return Vec::new();
        }

        let mut links = Vec::with_capacity(n * 2);

        for i in 0..n {
            let p_i = self.c4_charges[i].position;
            for j in (i + 1)..n {
                let p_j = self.c4_charges[j].position;
                let dist_ij = p_i.distance(p_j);
                if !(0.05..=MAX_DET_CORD_DIST).contains(&dist_ij) {
                    continue;
                }

                // RNG condition: no third charge k is closer to both i and j than dist_ij
                let mut is_relative_neighbor = true;
                for k in 0..n {
                    if k == i || k == j {
                        continue;
                    }
                    let p_k = self.c4_charges[k].position;
                    let d_ik = p_i.distance(p_k);
                    let d_jk = p_j.distance(p_k);
                    if d_ik.max(d_jk) < dist_ij - 0.001 {
                        is_relative_neighbor = false;
                        break;
                    }
                }

                if is_relative_neighbor {
                    // Det-cord connects at the charge beacon casing (+normal * 0.18)
                    let p1 = p_i + self.c4_charges[i].normal * 0.18;
                    let p2 = p_j + self.c4_charges[j].normal * 0.18;
                    links.push(DetCordLink {
                        idx_a: i,
                        idx_b: j,
                        p1,
                        p2,
                        dist: dist_ij,
                    });
                }
            }
        }

        links
    }

    pub fn place_c4_charge(&mut self, pos: Vec3, normal: Vec3, anchor: Option<IVec3>) -> bool {
        if self.c4_charges.len() >= MAX_C4_CHARGES {
            return false;
        }
        self.c4_charges.push(C4Charge {
            position: pos + normal * 0.05,
            normal,
            anchor_voxel: anchor,
            timer: 0.0,
            blast_radius: 5.5,
            impulse: 48.0,
        });
        self.det_cords_dirty = true;
        self.update_det_cord_cache();
        true
    }

    pub fn detonate_all_c4(
        &mut self,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
        camera_pos: Vec3,
    ) -> (FireFeedback, usize) {
        if self.c4_charges.is_empty() {
            return (
                FireFeedback {
                    camera_trauma: 0.0,
                    player_recoil: Vec3::ZERO,
                    fov_kick: 0.0,
                },
                0,
            );
        }

        // 1. Calculate laser det-cord links before clearing charges
        if self.det_cords_dirty {
            self.update_det_cord_cache();
        }
        let links = self.cached_det_cords.clone();
        let charges = std::mem::take(&mut self.c4_charges);
        self.cached_det_cords.clear();
        self.det_cords_dirty = false;
        let count = charges.len();
        let avg_pos = charges.iter().fold(Vec3::ZERO, |acc, c| acc + c.position) / (count as f32);

        // Audio cues for coordinated blast
        audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), avg_pos, camera_pos, 0.90);
        audio.play_spatial(audio.big_boom.as_ref(), avg_pos, camera_pos, 0.85);
        audio.play_spatial(audio.seismic_pulse.as_ref(), avg_pos, camera_pos, 0.80);

        let mut all_exp = Vec::new();
        let mut all_cores = Vec::new();
        let mut all_caps = Vec::new();

        // 2. Sequential / Coordinated Planar Cutting Shock along Det-Cord Links
        for (link_idx, link) in links.iter().enumerate() {
            // Carve planar cutting cylinder between linked charges
            let cut_res = grid.carve_cylinder_advanced(
                link.p1,
                link.p2,
                1.4,   // 2.8m diameter cutting swath through columns & shear walls
                520.0, // Overcomes SteelRebar (400HP) & Concrete (100HP)
                DamageType::Kinetic,
            );

            let mid_pt = (link.p1 + link.p2) * 0.5;
            let cut_dir = (link.p2 - link.p1).normalize_or_zero();
            let cut_normal = if cut_dir.y.abs() < 0.9 {
                Vec3::new(-cut_dir.z, 0.0, cut_dir.x).normalize_or_zero()
            } else {
                Vec3::X
            };

            // Severed rebar plastic deformation along cut plane
            for &rebar_p in &cut_res.severed_rebar_positions {
                physics.spawn_bent_rebar(rebar_p, cut_normal * 4.0, false);
            }

            // Kinetic spall and spark spray along the cut line
            let samples = (link.dist / 2.0).ceil().clamp(2.0, 8.0) as usize;
            for s in 0..=samples {
                let t = s as f32 / samples as f32;
                let sample_p = link.p1.lerp(link.p2, t);
                physics.spawn_spall_impact(sample_p, cut_normal, Color::new(1.0, 0.85, 0.35, 1.0));
            }

            // Directional kinetic shear shockwave
            physics.apply_kinetic_blast(mid_pt, cut_normal, 4.5, 65.0);
            physics.apply_radial_blast(mid_pt, 3.5, 45.0);

            physics.add_explosion_effects(
                mid_pt,
                2.2,
                38.0,
                Color::new(1.0, 0.55, 0.15, 1.0),
                cut_res.destroyed,
            );

            all_exp.extend(cut_res.explosive_triggers);
            all_cores.extend(cut_res.glow_core_triggers);
            all_caps.extend(cut_res.capacitor_triggers);

            // Queue cinematic visual shockwave slice
            self.active_cutting_shocks.push(ActiveCuttingShock {
                p1: link.p1,
                p2: link.p2,
                timer: 0.0,
                duration: 0.35,
                delay: (link_idx as f32) * 0.035, // 35ms cascade per link
                sound_played: false,
            });
        }

        // 3. Directional shaped cutting blasts at individual C4 charge sites
        for charge in charges {
            let cut_center = charge.position - charge.normal * (charge.blast_radius * 0.45);
            let res = grid.carve_sphere_advanced(cut_center, charge.blast_radius, 480.0, DamageType::Explosive);
            if res.destroyed > 0 {
                physics.rubble_dirty = true;
                let min_p = grid.world_to_voxel(cut_center - Vec3::splat(charge.blast_radius + 1.0));
                let max_p = grid.world_to_voxel(cut_center + Vec3::splat(charge.blast_radius + 1.0));
                physics.mark_rubble_region_dirty(min_p, max_p);
            }

            if res.ice_shattered > 0 {
                physics.spawn_ice_shatter_effects(charge.position, charge.blast_radius, (res.ice_shattered * 4).clamp(15, 50));
            }

            // Directional cutting shockwave
            physics.apply_kinetic_blast(charge.position, -charge.normal, charge.blast_radius * 2.2, charge.impulse * 2.5);
            physics.apply_radial_blast(charge.position, charge.blast_radius * 1.5, charge.impulse * 1.8);

            physics.add_explosion_effects(
                charge.position,
                charge.blast_radius,
                charge.impulse * 1.8,
                Color::new(1.0, 0.40, 0.12, 1.0),
                res.destroyed,
            );

            for &rebar_p in &res.severed_rebar_positions {
                physics.spawn_bent_rebar(rebar_p, -charge.normal * 4.0, false);
            }

            all_exp.extend(res.explosive_triggers);
            all_cores.extend(res.glow_core_triggers);
            all_caps.extend(res.capacitor_triggers);
        }

        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, all_exp, all_cores, all_caps, camera_pos);

        // Scale trauma proportionally to charge grid size
        let grid_scale = (count as f32 / MAX_C4_CHARGES as f32).clamp(0.0, 1.0);
        let feedback = FireFeedback {
            camera_trauma: (0.75 + grid_scale * 0.25 + chain_trauma).min(1.0),
            player_recoil: Vec3::ZERO,
            fov_kick: 6.0 + grid_scale * 4.0,
        };

        (feedback, count)
    }

    pub fn update(
        &mut self,
        dt: f32,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
        camera_pos: Vec3,
    ) -> f32 {
        let mut trauma = 0.0f32;

        let c4_len_before = self.c4_charges.len();
        self.c4_charges.retain_mut(|charge| {
            charge.timer += dt;
            if let Some(av) = charge.anchor_voxel {
                if grid.in_bounds(av.x, av.y, av.z) && !grid.get(av.x, av.y, av.z).is_solid() {
                    return false;
                }
            }
            true
        });
        if self.det_cords_dirty || self.c4_charges.len() != c4_len_before || (self.c4_charges.is_empty() && !self.cached_det_cords.is_empty()) {
            self.update_det_cord_cache();
        }

        // Step and update active sequential cutting shockwaves
        self.active_cutting_shocks.retain_mut(|shock| {
            shock.timer += dt;
            if shock.timer >= shock.delay && !shock.sound_played {
                shock.sound_played = true;
                let mid = (shock.p1 + shock.p2) * 0.5;
                audio.play_spatial(audio.hydraulic_slam.as_ref(), mid, camera_pos, 0.70);
            }
            shock.timer < shock.delay + shock.duration
        });

        if self.fire_cooldown > 0.0 {
            self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        }
        if self.laser_sound_timer > 0.0 {
            self.laser_sound_timer = (self.laser_sound_timer - dt).max(0.0);
        }

        // 2nd-order critically-damped harmonic spring-damper for crisp weapon recovery
        let spring_k = 220.0f32;
        let damping_k = 26.0f32;
        let pos_accel = -self.recoil_pos * spring_k - self.recoil_pos_vel * damping_k;
        self.recoil_pos_vel += pos_accel * dt;
        self.recoil_pos += self.recoil_pos_vel * dt;

        let rot_spring = 240.0f32;
        let rot_damping = 28.0f32;
        let rot_accel = -self.recoil_rot * rot_spring - self.recoil_rot_vel * rot_damping;
        self.recoil_rot_vel += rot_accel * dt;
        self.recoil_rot += self.recoil_rot_vel * dt;

        // Weapon heat venting and cooling
        self.heat = (self.heat - 0.45 * dt).max(0.0);

        // Update Tracers
        let mut tr_i = 0;
        while tr_i < self.tracers.len() {
            let tr = &mut self.tracers[tr_i];
            tr.life -= dt;
            if tr.life <= 0.0 {
                self.tracers.swap_remove(tr_i);
                continue;
            }
            tr_i += 1;
        }

        // Update Tesla Arcs
        let mut arc_i = 0;
        while arc_i < self.tesla_arcs.len() {
            let arc = &mut self.tesla_arcs[arc_i];
            arc.life -= dt;
            if arc.life <= 0.0 {
                self.tesla_arcs.swap_remove(arc_i);
                continue;
            }
            arc_i += 1;
        }

        // Update Active Singularities
        let mut sing_i = 0;
        while sing_i < self.singularities.len() {
            let sing = &mut self.singularities[sing_i];
            sing.timer += dt;
            sing.erosion_timer += dt;

            // 1. Gravitational suction on debris, dynamic chunks, smoke, and sparks (35m radius)
            physics.apply_vortex_suction(sing.position, sing.pull_radius, sing.pull_strength, dt);

            // 2. Play vacuum sweep sound as it nears critical mass
            if sing.timer >= sing.max_time - 0.75 && !sing.vacuum_played {
                audio.play_spatial(audio.vacuum_sweep.as_ref(), sing.position, camera_pos, 0.75);
                sing.vacuum_played = true;
            }

            // 3. Continuous erosion/suction carving at expanding inner vortex radius
            if sing.erosion_timer >= 0.15 {
                sing.erosion_timer = 0.0;
                let progress = (sing.timer / sing.max_time).clamp(0.0, 1.0);
                let erosion_radius = 2.50 + progress * 3.50; // 2.5m to 6.0m
                let res = grid.carve_sphere_advanced(sing.position, erosion_radius, 220.0, crate::voxel::DamageType::Kinetic);
                trauma += process_explosive_chain_with_capacitors(
                    grid, physics, audio, res.explosive_triggers, res.glow_core_triggers,
                    res.capacitor_triggers, camera_pos,
                );
                if res.destroyed > 0 {
                    physics.rubble_dirty = true;
                    physics.add_explosion_effects(
                        sing.position,
                        erosion_radius,
                        14.0,
                        Color::new(0.85, 0.25, 1.0, 1.0),
                        (res.destroyed * 2).clamp(6, 28),
                    );
                    physics.add_flying_rubble_shower(
                        sing.position,
                        erosion_radius * 0.8,
                        24.0,
                        (res.destroyed / 4).clamp(2, 12),
                    );
                }
            }

            // 4. Catastrophic Implosion Collapse
            if sing.timer >= sing.max_time {
                let collapse_r = sing.blast_radius.clamp(ActiveSingularity::COLLAPSE_RADIUS_MIN, ActiveSingularity::COLLAPSE_RADIUS_MAX);

                audio.play_spatial(audio.singularity_implode.as_ref(), sing.position, camera_pos, 0.85);
                audio.play_spatial(audio.big_boom.as_ref(), sing.position, camera_pos, 0.80);
                audio.play_spatial(audio.structural_groan.as_ref(), sing.position, camera_pos, 0.70);

                // Deep crater carving into earth and bedrock (950 damage breaches 800HP bedrock)
                let res = grid.carve_sphere_advanced(sing.position, collapse_r, 950.0, crate::voxel::DamageType::Explosive);
                let (dynamic_destroyed, dynamic_triggers) = physics.carve_chunk_sphere(
                    grid, sing.position, collapse_r, 950.0, DamageType::Explosive, false,
                );
                physics.rubble_dirty = true;
                if res.ice_shattered > 0 {
                    physics.spawn_ice_shatter_effects(sing.position, collapse_r, (res.ice_shattered * 4).clamp(20, 80));
                    audio.play_spatial(audio.cryo_shatter.as_ref(), sing.position, camera_pos, 0.65);
                }

                // Update subterranean rubble floor in carved crater
                physics.update_crater_floor(grid, sing.position, collapse_r);

                // Severe outward kinetic blast propels all gathered matter outward
                // Pull and collapse share the force captured when the orb was fired.
                let impulse_scale = sing.pull_strength / ActiveSingularity::PULL_STRENGTH;
                physics.apply_radial_blast(sing.position, collapse_r * 1.85, 240.0 * impulse_scale);

                // Expanding circular ground shockwave ring and scorch decals
                physics.add_ground_cratering_shockwave(sing.position, collapse_r * 1.5);
                let ground_y = crate::physics::PhysicsSystem::rubble_height_from_slice(&physics.rubble_height_grid, sing.position.x, sing.position.z);
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: Vec3::new(sing.position.x, ground_y + 0.05, sing.position.z),
                    normal: Vec3::Y,
                    radius: 1.0,
                    max_radius: collapse_r * 1.75,
                    speed: 62.0,
                    color: Color::new(0.92, 0.35, 1.0, 0.95),
                    thickness: 0.65,
                });

                // Colossal volumetric dust storm and multi-tier explosion effects
                physics.add_explosion_effects(
                    sing.position,
                    collapse_r,
                    75.0,
                    Color::new(0.95, 0.40, 1.0, 1.0),
                    res.destroyed + dynamic_destroyed + 90,
                );

                // Camera trauma & concussion screen shake
                let dist_atten = SoundSystem::calculate_distance_attenuation(sing.position, camera_pos);
                let chain_trauma = process_explosive_chain_at_world_positions(
                    grid, physics, audio, res.explosive_triggers, res.glow_core_triggers,
                    res.capacitor_triggers, dynamic_triggers, camera_pos,
                );
                trauma += (0.95 * dist_atten + chain_trauma).min(1.0);

                self.singularities.swap_remove(sing_i);
                continue;
            }

            sing_i += 1;
        }

        // Update Projectiles
        let mut p_i = 0;
        let mut cluster_spawns = Vec::new();

        while p_i < self.projectiles.len() {
            let p = &mut self.projectiles[p_i];
            p.life -= dt;
            if p.life <= 0.0 {
                self.projectiles.swap_remove(p_i);
                continue;
            }

            // Record trail trajectory points for ribbon rendering
            p.trail_points.push(p.position);

            // Spawning subtle non-occluding trail particles based on TrailStyle
            match p.trail_style {
                TrailStyle::SmokeWisps => {
                    if rand::gen_range(0.0f32, 1.0) < 0.12 {
                        let s_size = rand::gen_range(0.04, 0.08);
                        let s_life = rand::gen_range(0.25, 0.40);
                        physics.smoke.push(crate::physics::SmokeParticle {
                            position: p.position,
                            velocity: -p.velocity.normalize_or_zero() * 0.5 + Vec3::new(0.0, 0.15, 0.0),
                            size: s_size,
                            start_size: s_size,
                            max_size: rand::gen_range(0.12, 0.22),
                            color: Color::new(0.70, 0.70, 0.70, 0.08),
                            life: s_life,
                            max_life: s_life,
                        });
                    }
                }
                TrailStyle::PlasmaGlow => {
                    if rand::gen_range(0.0f32, 1.0) < 0.35 {
                        let spark_vel = -p.velocity * 0.08 + Vec3::new(
                            rand::gen_range(-0.4, 0.4),
                            rand::gen_range(-0.4, 0.4),
                            rand::gen_range(-0.4, 0.4),
                        );
                        physics.sparks.push(crate::physics::SparkParticle {
                            position: p.position,
                            velocity: spark_vel,
                            color: p.color,
                            life: 0.25,
                            max_life: 0.25,
                        });
                    }
                }
                TrailStyle::SparkShower => {
                    if rand::gen_range(0.0f32, 1.0) < 0.45 {
                        let spark_vel = -p.velocity.normalize_or_zero() * rand::gen_range(3.0, 7.0) + Vec3::new(
                            rand::gen_range(-1.2, 1.2),
                            rand::gen_range(-1.2, 1.2),
                            rand::gen_range(-1.2, 1.2),
                        );
                        physics.sparks.push(crate::physics::SparkParticle {
                            position: p.position,
                            velocity: spark_vel,
                            color: Color::new(1.0, 0.75, 0.2, 0.95),
                            life: 0.28,
                            max_life: 0.28,
                        });
                    }
                }
                TrailStyle::HyperTracer => {
                    if rand::gen_range(0.0f32, 1.0) < 0.20 {
                        physics.sparks.push(crate::physics::SparkParticle {
                            position: p.position,
                            velocity: Vec3::ZERO,
                            color: Color::new(0.3, 1.0, 0.9, 0.9),
                            life: 0.18,
                            max_life: 0.18,
                        });
                    }
                }
            }

            let old_vy = p.velocity.y;
            p.velocity.y += p.gravity * dt;
            let next_pos = p.position + p.velocity * dt;

            // Airburst shell apex detection: when vertical climb turns into descent, detonate into 6 cluster bomblets!
            if p.is_airburst && old_vy > 0.0 && p.velocity.y <= 0.0 {
                audio.play_spatial(audio.big_boom.as_ref(), p.position, camera_pos, 0.70);
                audio.play_spatial(audio.explosion.as_ref(), p.position, camera_pos, 0.65);
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: p.position,
                    normal: Vec3::Y,
                    radius: 0.5,
                    max_radius: 7.0,
                    speed: 24.0,
                    color: Color::new(1.0, 0.55, 0.15, 1.0),
                    thickness: 0.40,
                });
                physics.add_explosion_effects(p.position, 2.8, 45.0, Color::new(1.0, 0.60, 0.20, 1.0), 12);
                cluster_spawns.push((p.position, Vec3::NEG_Y, 6, PayloadType::HighExplosive, TrailStyle::SmokeWisps));
                self.projectiles.swap_remove(p_i);
                continue;
            }

            // Check collision with voxel world using raycast, or dynamic/settled chunks
            let move_vec = next_pos - p.position;
            let dist = move_vec.length();
            let mut hit_pos_norm: Option<(Vec3, Vec3)> = None;

            if dist > 0.001 {
                if let Some(h) = grid.raycast(p.position, move_vec.normalize_or_zero(), dist) {
                    hit_pos_norm = Some((h.world_pos, h.normal));
                }
                if let Some(chunk_hit) = physics.check_chunk_hit_radius(p.position, next_pos, p.radius) {
                    if hit_pos_norm.is_none_or(|(pos, _)| p.position.distance_squared(chunk_hit.0) < p.position.distance_squared(pos)) {
                        hit_pos_norm = Some(chunk_hit);
                    }
                }
            }

            if let Some(ground_hit) = surrounding_foundation_hit(p.position, next_pos) {
                if hit_pos_norm.is_none_or(|(pos, _)| p.position.distance_squared(ground_hit) < p.position.distance_squared(pos)) {
                    hit_pos_norm = Some((ground_hit, Vec3::Y));
                }
            }

            if let Some((hit_pos, hit_norm)) = hit_pos_norm {

                if p.is_sticky {
                    p.position = hit_pos + hit_norm * 0.15;
                    p.velocity = Vec3::ZERO;
                    p.gravity = 0.0;
                    p.stuck_normal = hit_norm;
                    self.sticky_charges.push(self.projectiles.swap_remove(p_i));
                    audio.play(audio.ui_click.as_ref(), 0.6);
                    continue;
                }

                // Apply Payload Type modifications
                match p.payload {
                    PayloadType::HighExplosive => {
                        // Standard concussive expansion handled in mode logic
                    }
                    PayloadType::ShapedCharge => {
                        // Penetrating kinetic jet forward
                        let punch_dir = p.velocity.normalize_or_zero();
                        let punch_depth = 2.2;
                        let punch_res = grid.carve_cylinder_advanced(hit_pos, hit_pos + punch_dir * punch_depth, 0.7, 300.0, crate::voxel::DamageType::Kinetic);
                        if punch_res.destroyed > 0 {
                            physics.add_explosion_effects(hit_pos + punch_dir * punch_depth, 0.8, p.impulse * 0.9, Color::new(1.0, 0.85, 0.3, 1.0), punch_res.destroyed);
                        }
                    }
                    PayloadType::CryoShrapnel => {
                        let frozen = grid.freeze_sphere(hit_pos, 2.8);
                        if frozen > 0 {
                            physics.spawn_ice_shatter_effects(hit_pos, 2.8, 22);
                            audio.play(audio.cryo_freeze.as_ref(), 0.65);
                        }
                    }
                    PayloadType::Incendiary => {
                        physics.spawn_molten_fissure(hit_pos, hit_norm, 12);
                        let therm_res = grid.carve_sphere_advanced(hit_pos, 1.4, 130.0, crate::voxel::DamageType::Thermal);
                        if therm_res.destroyed > 0 {
                            physics.add_explosion_effects(hit_pos, 1.4, p.impulse * 0.6, Color::new(1.0, 0.45, 0.1, 1.0), therm_res.destroyed);
                        }
                    }
                }

                // Handle Projectile Impact Detonations
                match p.mode {
                    WeaponMode::HavocMortar => {
                        audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), hit_pos, camera_pos, 0.65);
                        audio.play_spatial(audio.big_boom.as_ref(), hit_pos, camera_pos, 0.55);
                        let res = grid.carve_sphere(hit_pos, p.blast_radius);

                        if res.destroyed > 0 {
                            physics.rubble_dirty = true;
                            self.recent_hit = Some(if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() {
                                crate::ui::HitKind::Critical
                            } else if res.ice_shattered > 0 {
                                crate::ui::HitKind::CryoShatter
                            } else {
                                crate::ui::HitKind::Standard
                            });
                        }

                        if res.ice_shattered > 0 {
                            physics.spawn_ice_shatter_effects(hit_pos, p.blast_radius, (res.ice_shattered * 4).clamp(15, 50));
                            audio.play_spatial(audio.cryo_shatter.as_ref(), hit_pos, camera_pos, 0.50);
                        }

                        physics.add_explosion_effects(
                            hit_pos,
                            p.blast_radius,
                            p.impulse,
                            p.color,
                            res.destroyed,
                        );

                        // Spawn scorch impact decal on detonation anchored to surviving surface
                        spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, p.blast_radius * 1.4, Color::new(0.12, 0.12, 0.12, 0.9), crate::physics::DecalKind::Scorch);

                        // Violent radial pressure wave on nearby chunks
                        physics.apply_radial_blast(hit_pos, p.blast_radius * 1.5, p.impulse * 1.3);

                        let dist_atten = SoundSystem::calculate_distance_attenuation(hit_pos, camera_pos);
                        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                        trauma += (0.35 * dist_atten + chain_trauma * 0.4).min(0.45);

                        if (p.barrel == BarrelType::ClusterSplit || p.is_airburst) && !p.is_cluster_child {
                            let count = if p.is_airburst { 6 } else { self.config.cluster_count };
                            cluster_spawns.push((hit_pos, hit_norm, count, p.payload, p.trail_style));
                        }
                    }

                    WeaponMode::FrostbiteMortar => {
                        let inner_r = (p.blast_radius * 0.6).max(1.8);
                        let outer_r = p.blast_radius; // 3.5m freeze radius

                        // 1. Shatter any brittle CryoIce or direct-hit core voxels with Cryo shockwave
                        let res = grid.carve_sphere_advanced(hit_pos, inner_r, 200.0, crate::voxel::DamageType::Cryo);

                        // 2. Flash-freeze surrounding architecture (3.5m radius) into brittle CryoIce
                        let frozen = grid.freeze_sphere(hit_pos, outer_r);

                        audio.play_spatial(audio.cryo_freeze.as_ref(), hit_pos, camera_pos, 0.55);

                        if res.ice_shattered > 0 || res.destroyed > 0 || p.is_cluster_child {
                            // Violent glass-acoustic crystalline shatter!
                            audio.play_spatial(audio.cryo_shatter.as_ref(), hit_pos, camera_pos, 0.55);
                            physics.spawn_ice_shatter_effects(hit_pos, inner_r * 1.2, (res.ice_shattered * 5 + 30).min(65));
                        } else {
                            physics.spawn_ice_shatter_effects(hit_pos, inner_r * 0.8, 25);
                        }

                        if res.destroyed > 0 || frozen > 0 {
                            self.recent_hit = Some(if res.ice_shattered > 0 {
                                crate::ui::HitKind::CryoShatter
                            } else {
                                crate::ui::HitKind::Standard
                            });
                        }

                        physics.add_explosion_effects(
                            hit_pos,
                            inner_r,
                            p.impulse * 0.75,
                            Color::new(0.65, 0.95, 1.0, 1.0),
                            res.destroyed + frozen,
                        );

                        // Cryo frost decal marking frozen impact anchored to surviving surface
                        spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, outer_r * 1.2, Color::new(0.65, 0.95, 1.0, 0.95), crate::physics::DecalKind::CryoFrost);

                        // Cryo shatter shockwave displaces chunks
                        physics.apply_radial_blast(hit_pos, inner_r * 1.4, p.impulse * 1.1);

                        let dist_atten = SoundSystem::calculate_distance_attenuation(hit_pos, camera_pos);
                        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                        trauma += (0.30 * dist_atten + chain_trauma * 0.35).min(0.40);
                    }

                    WeaponMode::EventHorizon => {
                        // Keep launch settings even if the workbench changes during flight.
                        let collapse_radius = (18.0 * (p.blast_radius / 3.8))
                            .clamp(ActiveSingularity::COLLAPSE_RADIUS_MIN, ActiveSingularity::COLLAPSE_RADIUS_MAX);
                        self.singularities.push(ActiveSingularity {
                            position: hit_pos,
                            timer: 0.0,
                            max_time: 2.4,
                            pull_radius: ActiveSingularity::PULL_RADIUS,
                            pull_strength: ActiveSingularity::PULL_STRENGTH * (p.impulse / 45.0),
                            blast_radius: collapse_radius,
                            vacuum_played: false,
                            erosion_timer: 0.0,
                        });
                        audio.play_spatial(audio.singularity_hum.as_ref(), hit_pos, camera_pos, 0.65);
                    }

                    WeaponMode::SeismicDisrupter => {
                        audio.play_spatial(audio.seismic_pulse.as_ref(), hit_pos, camera_pos, 0.65);
                        audio.play_spatial(audio.hydraulic_slam.as_ref(), hit_pos, camera_pos, 0.55);

                        // Carve wide cylindrical ground fracture
                        let fracture_depth = 3.5;
                        let start_cyl = hit_pos + hit_norm * 0.5;
                        let end_cyl = hit_pos - hit_norm * fracture_depth;
                        let res = grid.carve_cylinder_advanced(start_cyl, end_cyl, p.blast_radius, 260.0, crate::voxel::DamageType::Kinetic);

                        // Spawn expanding ground-shivering seismic shockwave
                        physics.spawn_seismic_shockwave(hit_pos, p.blast_radius * 1.6, p.impulse);

                        physics.add_explosion_effects(
                            hit_pos,
                            p.blast_radius,
                            p.impulse,
                            Color::new(0.95, 0.45, 0.15, 1.0),
                            res.destroyed,
                        );

                        // Kinetic fault fracture decal anchored to surviving surface
                        spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, p.blast_radius * 1.5, Color::new(0.45, 0.40, 0.35, 0.9), crate::physics::DecalKind::KineticImpact);

                        // Seismic upward heave on chunks
                        physics.apply_kinetic_blast(hit_pos - Vec3::Y * 0.5, Vec3::Y + hit_norm * 0.5, p.blast_radius * 1.8, p.impulse * 1.5);

                        if res.destroyed > 0 {
                            self.recent_hit = Some(if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() {
                                crate::ui::HitKind::Critical
                            } else {
                                crate::ui::HitKind::Standard
                            });
                        }

                        let dist_atten = SoundSystem::calculate_distance_attenuation(hit_pos, camera_pos);
                        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                        trauma += (0.35 * dist_atten + chain_trauma * 0.35).min(0.45);

                        if p.barrel == BarrelType::ClusterSplit && !p.is_cluster_child {
                            cluster_spawns.push((hit_pos, hit_norm, self.config.cluster_count, p.payload, p.trail_style));
                        }
                    }

                    WeaponMode::PlasmaFlamer => {
                        let dist_atten = SoundSystem::calculate_distance_attenuation(hit_pos, camera_pos);

                        let res = grid.carve_sphere_advanced(hit_pos, p.blast_radius, 160.0, crate::voxel::DamageType::Thermal);
                        physics.spawn_molten_fissure(hit_pos, hit_norm, 8);
                        if res.destroyed > 0 || !res.spalled_positions.is_empty() {
                            spawn_anchored_impact_decal(grid, physics, hit_pos, hit_norm, p.blast_radius * 1.2, Color::new(1.0, 0.4, 0.1, 0.95), crate::physics::DecalKind::PlasmaBurn);
                        }

                        physics.add_explosion_effects(
                            hit_pos,
                            p.blast_radius * 0.9,
                            p.impulse,
                            WeaponMode::PlasmaFlamer.color(),
                            res.destroyed,
                        );

                        physics.apply_radial_blast(hit_pos, p.blast_radius * 1.3, p.impulse);

                        if res.destroyed > 0 {
                            self.recent_hit = Some(if res.rebar_destroyed > 0 || !res.explosive_triggers.is_empty() {
                                crate::ui::HitKind::Critical
                            } else {
                                crate::ui::HitKind::Standard
                            });
                        }

                        let chain_trauma = process_explosive_chain_with_capacitors(grid, physics, audio, res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers, camera_pos);
                        trauma += 0.12 * dist_atten + chain_trauma;
                    }

                    _ => {}
                }

                self.projectiles.swap_remove(p_i);
                continue;
            }

            p.position = next_pos;
            p_i += 1;
        }

        // Spawn cluster submunitions
        for (pos, norm, count, payload, trail_style) in cluster_spawns {
            audio.play_spatial(audio.explosion.as_ref(), pos, camera_pos, 0.50);
            for _ in 0..count {
                let rand_tangent = Vec3::new(
                    rand::gen_range(-1.0f32, 1.0),
                    rand::gen_range(0.3f32, 1.0),
                    rand::gen_range(-1.0f32, 1.0),
                ).normalize_or_zero();

                let bounce_dir = (norm * 0.6 + rand_tangent * 0.8).normalize_or_zero();

                self.projectiles.push(ActiveProjectile {
                    position: pos + norm * 0.3,
                    velocity: bounce_dir * rand::gen_range(14.0, 24.0),
                    gravity: -18.0,
                    radius: 0.22,
                    blast_radius: 2.5 * self.config.blast_radius_mult,
                    impulse: 16.0 * self.config.impulse_mult,
                    mode: WeaponMode::HavocMortar,
                    barrel: BarrelType::Precision,
                    payload,
                    trail_style,
                    color: Color::new(1.0, 0.5, 0.1, 1.0),
                    life: rand::gen_range(0.35, 0.75),
                    is_cluster_child: true,
                    is_airburst: false,
                    is_sticky: false,
                    stuck_normal: Vec3::ZERO,
                    trail_points: TrailBuffer::single(pos + norm * 0.3),
                });
            }
        }

        trauma
    }

    pub fn draw_world_entities(&self, cam_pos: Vec3, cam_dir: Vec3, right: Vec3, up: Vec3) {
        // Draw Projectiles with luminous cores, exhaust flames, and trail ribbons
        for p in &self.projectiles {
            // Bright luminous core
            draw_sphere(p.position, p.radius, None, p.color);
            draw_sphere(p.position, p.radius * 0.55, None, WHITE);

            // Rocket exhaust flame behind projectile - high luminosity flare
            let back_dir = -p.velocity.normalize_or_zero();
            let flame_pos = p.position + back_dir * (p.radius * 0.75);
            let flame_color = match p.trail_style {
                TrailStyle::SmokeWisps => Color::new(1.0, 0.65, 0.2, 0.95),
                TrailStyle::PlasmaGlow => Color::new(0.2, 0.9, 1.0, 0.95),
                TrailStyle::SparkShower => Color::new(1.0, 0.5, 0.1, 0.95),
                TrailStyle::HyperTracer => Color::new(0.2, 1.0, 0.6, 0.95),
            };
            draw_sphere(flame_pos, p.radius * 0.95, None, flame_color);
            draw_sphere(flame_pos + back_dir * 0.08, p.radius * 0.48, None, Color::new(1.0, 0.95, 0.85, 1.0));

            // Trail ribbon connecting trail_points with luminous multi-line core and beads
            let pt_count = p.trail_points.len();
            if pt_count >= 2 {
                for i in 0..pt_count - 1 {
                    let p1 = p.trail_points[i];
                    let p2 = p.trail_points[i + 1];
                    let frac = (i + 1) as f32 / pt_count as f32; // 0 near tail, 1 near head
                    let ribbon_color = match p.trail_style {
                        TrailStyle::SmokeWisps => Color::new(1.0, 0.8, 0.35, (frac * 0.85).clamp(0.25, 0.95)),
                        TrailStyle::PlasmaGlow => Color::new(p.color.r, p.color.g, p.color.b, (frac * 0.9).clamp(0.3, 1.0)),
                        TrailStyle::SparkShower => Color::new(1.0, 0.65, 0.2, (frac * 0.85).clamp(0.25, 0.95)),
                        TrailStyle::HyperTracer => Color::new(0.2, 0.95, 0.85, (frac * 0.95).clamp(0.4, 1.0)),
                    };
                    draw_line_3d(p1, p2, ribbon_color);
                    let offset1 = Vec3::new(0.025, 0.025, 0.0);
                    let offset2 = Vec3::new(-0.025, 0.025, 0.0);
                    draw_line_3d(p1 + offset1, p2 + offset1, ribbon_color);
                    draw_line_3d(p1 + offset2, p2 + offset2, ribbon_color);
                    if i % 2 == 0 {
                        draw_sphere(p2, (p.radius * 0.32 * frac).max(0.04), None, ribbon_color);
                    }
                }
            }
        }

        // Draw Tesla Lightning Arcs with fractal branching chains & high-frequency corona
        let t_now = get_time() as f32;
        for arc in &self.tesla_arcs {
            let alpha = (arc.life / arc.max_life).clamp(0.0, 1.0);
            let glow_col = Color::new(arc.color.r, arc.color.g, arc.color.b, alpha * 0.75);
            let core_col = Color::new(1.0, 1.0, 1.0, alpha * 0.95);
            let fork_col = Color::new(0.6, 0.85, 1.0, alpha * 0.65);

            for (b_idx, &(p1, p2)) in arc.branches.iter().enumerate() {
                // High-frequency violent electrical vibration jitter
                let jitter_phase = t_now * 38.0 + b_idx as f32 * 1.7;
                let jitter = Vec3::new(
                    jitter_phase.sin() * 0.04,
                    (jitter_phase * 1.3).cos() * 0.04,
                    (jitter_phase * 0.7).sin() * 0.04,
                );
                let jp1 = p1 + jitter;
                let jp2 = p2 + jitter;

                // Main conduit glow and white-hot plasma core
                draw_line_3d(jp1, jp2, glow_col);
                let mid = (jp1 + jp2) * 0.5 + Vec3::new(
                    (jitter_phase * 2.1).cos() * 0.06,
                    (jitter_phase * 1.9).sin() * 0.06,
                    (jitter_phase * 2.5).cos() * 0.06,
                );
                draw_line_3d(jp1, mid, core_col);
                draw_line_3d(mid, jp2, core_col);

                // Luminous ionization nodes at branch junctions
                draw_sphere(jp1, 0.05 * alpha, None, core_col);

                // Secondary branching lightning micro-forks discharging into atmosphere
                if b_idx % 2 == 0 {
                    let seg_dir = (jp2 - jp1).normalize_or_zero();
                    let fork_perp = if seg_dir.y.abs() < 0.9 {
                        seg_dir.cross(Vec3::Y).normalize_or_zero()
                    } else {
                        seg_dir.cross(Vec3::X).normalize_or_zero()
                    };
                    let fork_len = 0.35 + (b_idx as f32 * 0.1).sin().abs() * 0.45;
                    let fork_dir = (seg_dir * 0.5 + fork_perp * ((b_idx as f32 * 3.1).sin())).normalize_or_zero();
                    let fork_tip = mid + fork_dir * fork_len;
                    draw_line_3d(mid, fork_tip, fork_col);
                    draw_sphere(fork_tip, 0.03 * alpha, None, core_col);
                }
            }
        }

        // Draw Sticky Charges (flashing beacon)
        for charge in &self.sticky_charges {
            let flash = ((get_time() * 8.0).sin() * 0.5 + 0.5) as f32;
            let c = Color::new(1.0, flash, flash * 0.2, 1.0);
            draw_cube(charge.position, Vec3::splat(0.35), None, c);
        }

        // Render intense incandescent cutting shock planes along severed det-cords
        for shock in &self.active_cutting_shocks {
            if shock.timer < shock.delay {
                continue;
            }
            let local_t = ((shock.timer - shock.delay) / shock.duration).clamp(0.0, 1.0);
            let alpha = (1.0 - local_t).clamp(0.0, 1.0);
            let slice_color = Color::new(1.0, 0.65, 0.20, alpha * 0.95);
            let core_white = Color::new(1.0, 1.0, 1.0, alpha);

            draw_line_3d(shock.p1, shock.p2, core_white);
            let offset_up = Vec3::Y * (0.25 * (1.0 - local_t));
            draw_line_3d(shock.p1 + offset_up, shock.p2 + offset_up, slice_color);
            draw_line_3d(shock.p1 - offset_up, shock.p2 - offset_up, slice_color);
        }

        // Draw Demolition Engineer C4 Shaped Charges (magnetic base, shaped charge body, flashing red LED beacon)
        for charge in &self.c4_charges {
            let n = charge.normal;
            let abs_n = n.abs();

            // Oriented magnetic mounting plate (dark tungsten metal)
            // Thickness 0.08m along surface normal, 0.40m along tangent axes
            let plate_size = Vec3::new(0.40, 0.40, 0.40) - abs_n * 0.32;
            draw_cube(charge.position, plate_size, None, Color::new(0.18, 0.20, 0.24, 1.0));

            // Directional shaped cutting charge body (hazard military olive case)
            let body_size = Vec3::new(0.28, 0.28, 0.28) - abs_n * 0.12;
            draw_cube(charge.position + n * 0.14, body_size, None, Color::new(0.65, 0.55, 0.20, 1.0));

            // Blinking tactical red LED beacon with pulsating corona
            let blink_phase = (macroquad::time::get_time() as f32 * 9.0 + charge.timer * 4.0).sin();
            if blink_phase > -0.2 {
                let beacon_pos = charge.position + n * 0.26;
                let alpha = (blink_phase.max(0.0) * 0.95).min(1.0);
                draw_sphere(beacon_pos, 0.07, None, Color::new(1.0, 0.15, 0.10, alpha));
                draw_sphere(beacon_pos, 0.16, None, Color::new(1.0, 0.30, 0.15, alpha * 0.45));
            }
        }

        // Draw Coordinated Demolition Laser Det-Cords between adjacent C4 charges (using cached topology)
        let now = macroquad::time::get_time() as f32;
        let pulse = (now * 6.5).sin() * 0.5 + 0.5;

        for (link_idx, link) in self.cached_det_cords.iter().enumerate() {
            let p1 = link.p1;
            let p2 = link.p2;
            let dir = (p2 - p1).normalize_or_zero();
            let to_cam = (cam_pos - p1).normalize_or_zero();
            let side = dir.cross(to_cam).normalize_or_zero() * 0.035;

            // 1. High-energy neon laser corona (pulsating cyan/amber)
            let corona_alpha = 0.50 + 0.40 * pulse;
            let corona_col = Color::new(0.0, 0.90, 1.0, corona_alpha);
            draw_line_3d(p1 + side, p2 + side, corona_col);
            draw_line_3d(p1 - side, p2 - side, corona_col);

            // 2. Ultra-bright white-hot laser core
            let core_col = Color::new(0.85, 0.98, 1.0, 0.95);
            draw_line_3d(p1, p2, core_col);

            // 3. Traveling ionization pulse packet along the det-cord
            let travel_t = (now * 2.5 + (link_idx * 7) as f32 * 0.15).fract();
            let packet_pos = p1.lerp(p2, travel_t);
            draw_sphere(packet_pos, 0.08, None, Color::new(1.0, 1.0, 1.0, 0.95));
            draw_sphere(packet_pos, 0.18, None, Color::new(0.2, 0.85, 1.0, 0.45));
        }

        // Draw Singularities with Glowing Accretion Vortex & Relativistic Photon Ring
        for sing in &self.singularities {
            let collapse_p = if sing.timer >= sing.max_time - 0.75 {
                (sing.timer - (sing.max_time - 0.75)) / 0.75
            } else {
                0.0
            };
            let shrink = (1.0 - collapse_p * 0.75).clamp(0.2, 1.0);
            let time_spin = sing.timer * 7.5;

            // Pitch-black gravitational event horizon core
            draw_sphere(sing.position, 1.25 * shrink, None, Color::new(0.02, 0.01, 0.03, 1.0));
            // Ultra-bright relativistic photon ring boundary
            draw_sphere(sing.position, 1.38 * shrink, None, Color::new(0.85, 0.35, 1.0, 0.35));

            // Relativistic glowing accretion disk with rotating luminous rings
            let disk_r1 = (4.5 + (sing.timer * 4.0).sin() * 0.5) * shrink;
            let disk_r2 = (7.2 + (sing.timer * 3.0).cos() * 0.6) * shrink;
            draw_cylinder(sing.position, disk_r1, (disk_r1 - 0.40).max(0.05), 0.18, None, Color::new(0.95, 0.25, 1.0, 0.85));
            draw_cylinder(sing.position, disk_r2, (disk_r2 - 0.30).max(0.05), 0.12, None, Color::new(0.55, 0.85, 1.0, 0.70));
            let inner_disk = disk_r1 * 0.65;
            draw_cylinder(sing.position, inner_disk, (inner_disk - 0.30).max(0.04), 0.24, None, Color::new(1.0, 0.90, 0.40, 0.90));

            // Swirling gravitational vortex accretion filaments (spiraling matter streams)
            let stream_count = 8;
            for s_idx in 0..stream_count {
                let stream_angle = time_spin + (s_idx as f32 / stream_count as f32) * std::f32::consts::TAU;
                let segs = 6;
                let mut prev_pt = sing.position + Vec3::new(
                    stream_angle.cos() * disk_r2,
                    ((stream_angle * 2.0).sin() * 0.25) * shrink,
                    stream_angle.sin() * disk_r2,
                );
                for seg in 1..=segs {
                    let frac = seg as f32 / segs as f32;
                    let cur_r = disk_r2 * (1.0 - frac * 0.70);
                    let cur_angle = stream_angle + frac * 2.5;
                    let cur_pt = sing.position + Vec3::new(
                        cur_angle.cos() * cur_r,
                        ((cur_angle * 2.0).sin() * 0.25 * (1.0 - frac)) * shrink,
                        cur_angle.sin() * cur_r,
                    );
                    let stream_col = Color::new(
                        0.6 + frac * 0.4,
                        0.3 + (1.0 - frac) * 0.5,
                        1.0,
                        (0.85 - frac * 0.3).max(0.1),
                    );
                    draw_line_3d(prev_pt, cur_pt, stream_col);
                    prev_pt = cur_pt;
                }
            }
        }

        // Draw Railgun Tracers with Hypersonic Vacuum Tunnel Cavitation
        for tr in &self.tracers {
            let alpha = (tr.life / tr.max_life).min(1.0);
            let c_core = Color::new(1.0, 1.0, 1.0, alpha);
            let c_outer = Color::new(tr.color.r, tr.color.g, tr.color.b, alpha * 0.85);

            // Multi-line thick hypersonic ionization core
            draw_line_3d(tr.start, tr.end, c_core);
            let offset_x = Vec3::new(0.025, 0.025, 0.0);
            let offset_y = Vec3::new(-0.025, 0.025, 0.0);
            draw_line_3d(tr.start + offset_x, tr.end + offset_x, c_outer);
            draw_line_3d(tr.start + offset_y, tr.end + offset_y, c_outer);

            // Hypersonic vacuum tunnel cavitation shock rings and Mach expansion cones
            let beam_vec = tr.end - tr.start;
            let beam_len = beam_vec.length();
            if beam_len > 1.0 {
                let dir = beam_vec / beam_len;
                let ring_spacing = 6.0;
                let ring_count = (beam_len / ring_spacing) as usize;
                let cavitation_radius = 0.45 + (1.0 - alpha) * 2.2;

                for step in 1..=ring_count {
                    let ring_center = tr.start + dir * (step as f32 * ring_spacing);
                    // Concentric cavitation vapor rings
                    draw_cylinder(
                        ring_center,
                        cavitation_radius,
                        (cavitation_radius - 0.16).max(0.02),
                        0.10,
                        None,
                        Color::new(0.35, 0.85, 1.0, alpha * 0.70),
                    );
                    // Outer supersonic refraction rim
                    draw_cylinder(
                        ring_center,
                        cavitation_radius + 0.22,
                        cavitation_radius,
                        0.06,
                        None,
                        Color::new(0.85, 0.95, 1.0, alpha * 0.40),
                    );

                    // Helical vacuum cavitation wisps along the tunnel axis
                    let angle = (step as f32 * 1.5) + (1.0 - alpha) * 12.0;
                    let perp = if dir.y.abs() < 0.9 {
                        dir.cross(Vec3::Y).normalize_or_zero()
                    } else {
                        dir.cross(Vec3::X).normalize_or_zero()
                    };
                    let perp2 = dir.cross(perp).normalize_or_zero();
                    let wisp_pos = ring_center + (perp * angle.cos() + perp2 * angle.sin()) * (cavitation_radius * 0.8);
                    draw_sphere(wisp_pos, 0.06 * alpha, None, Color::new(0.7, 0.95, 1.0, alpha * 0.8));
                }
            }
        }

        // Draw Continuous Laser Beam from muzzle to target with sizzling helical plasma filaments
        if self.is_firing_laser {
            if let Some(target) = self.laser_hit_pos {
                // Compute true muzzle origin in front of camera
                let base_offset = right * 0.28 - up * 0.22 + cam_dir * 0.55;
                let muzzle_start = cam_pos + base_offset + cam_dir * 0.25;

                // Central white-hot core
                draw_line_3d(muzzle_start, target, Color::new(0.98, 1.0, 1.0, 1.0));

                // Outer cyan beam
                draw_line_3d(muzzle_start + right * 0.015, target, Color::new(0.2, 0.85, 1.0, 0.8));
                draw_line_3d(muzzle_start - right * 0.015, target, Color::new(0.2, 0.85, 1.0, 0.8));

                // Helical swirling plasma filament lines
                let beam_delta = target - muzzle_start;
                let beam_len = beam_delta.length();
                if beam_len > 0.5 {
                    let beam_dir = beam_delta / beam_len;
                    let beam_right = if beam_dir.y.abs() < 0.9 {
                        beam_dir.cross(Vec3::Y).normalize_or_zero()
                    } else {
                        beam_dir.cross(Vec3::X).normalize_or_zero()
                    };
                    let beam_up = beam_right.cross(beam_dir).normalize_or_zero();

                    let segments = (beam_len * 3.0).clamp(6.0, 36.0) as usize;
                    let t_now = get_time() as f32 * 28.0;
                    let mut prev_p1 = muzzle_start;
                    let mut prev_p2 = muzzle_start;

                    for s in 1..=segments {
                        let frac = s as f32 / segments as f32;
                        let p = muzzle_start + beam_delta * frac;
                        let angle = t_now + frac * 25.0;
                        let offset1 = (beam_right * angle.cos() + beam_up * angle.sin()) * 0.09;
                        let offset2 = (beam_right * (angle + std::f32::consts::PI).cos() + beam_up * (angle + std::f32::consts::PI).sin()) * 0.09;

                        let p1 = p + offset1;
                        let p2 = p + offset2;
                        draw_line_3d(prev_p1, p1, Color::new(0.3, 0.95, 1.0, 0.75));
                        draw_line_3d(prev_p2, p2, Color::new(0.15, 0.65, 1.0, 0.65));
                        prev_p1 = p1;
                        prev_p2 = p2;
                    }
                }

                // Pulsing molten vaporization hotspot at target
                let pulse = (get_time() as f32 * 25.0).sin().abs() * 0.12;
                draw_sphere(target, 0.42 + pulse, None, Color::new(0.3, 0.95, 1.0, 0.85));
                draw_sphere(target, 0.22 + pulse * 0.5, None, WHITE);
                let ring_r = 0.55 + pulse * 1.2;
                draw_cylinder(target, ring_r, ring_r - 0.08, 0.05, None, Color::new(1.0, 0.65, 0.15, 0.85));
            }
        }
    }

    // Render 3D Weapon Viewmodel in First Person with procedural recoil, walk bob, heat shimmer, and glowing materials
    pub fn draw_viewmodel(&mut self, camera_pos: Vec3, camera_dir: Vec3, right: Vec3, up: Vec3, bob_timer: f32, move_speed: f32) {
        let base_offset = right * 0.28 - up * 0.22 + camera_dir * 0.55;
        let recoil = right * self.recoil_pos.x + up * self.recoil_pos.y + camera_dir * self.recoil_pos.z;

        // Dynamic procedural weapon walk-bob & breathing sway
        let bob_factor = (move_speed / 7.0).clamp(0.0, 1.25);
        let walk_sway_x = (bob_timer * 0.5).cos() * 0.016 * bob_factor;
        let walk_sway_y = (bob_timer).sin() * 0.012 * bob_factor;
        let idle_breathing_y = ((get_time() as f32) * 2.0).sin() * 0.0025;
        let dynamic_offset = right * walk_sway_x + up * (walk_sway_y + idle_breathing_y);
        let origin = camera_pos + base_offset + recoil + dynamic_offset;

        let kick_pitch = self.recoil_rot.x.to_radians();
        let kick_yaw = self.recoil_rot.y.to_radians();
        let kick_roll = self.recoil_rot.z.to_radians();

        // Kicked 3D basis vectors for true mechanical weapon recoil
        let w_dir = (camera_dir + up * kick_pitch.sin() + right * kick_yaw.sin()).normalize();
        let w_right = (right + up * kick_roll.sin()).normalize();
        let w_up = w_right.cross(w_dir).normalize();

        let w_color = self.config.mode.color();

        let mut vertices = Vec::with_capacity(384);
        let mut indices = Vec::with_capacity(576);

        // 1. Weapon Body / Receiver (Dark carbon-steel frame)
        let body_color = Color::new(0.16, 0.18, 0.22, 1.0);
        push_oriented_box(&mut vertices, &mut indices, origin, Vec3::new(0.08, 0.10, 0.32), w_dir, w_right, w_up, body_color);

        // 2. High-Tech Barrel / Muzzle
        let barrel_pos = origin + w_dir * 0.22;
        push_oriented_box(&mut vertices, &mut indices, barrel_pos, Vec3::new(0.05, 0.05, 0.18), w_dir, w_right, w_up, Color::new(0.28, 0.30, 0.34, 1.0));

        // 3. Glowing Energy Chamber / Power Rails (Reflects current mode)
        let energy_pos = origin + w_up * 0.04 - w_dir * 0.02;
        push_oriented_box(&mut vertices, &mut indices, energy_pos, Vec3::new(0.06, 0.03, 0.14), w_dir, w_right, w_up, w_color);

        // 4. Heat Venting Visuals (for high-energy weapons)
        if self.heat > 0.12 {
            let glow_val = (self.heat * 1.3).min(1.0);
            let vent_color = match self.config.mode {
                WeaponMode::SolsticeLance | WeaponMode::ApexRailgun => Color::new(0.3 * (1.0 - glow_val) + 0.9 * glow_val, 0.95, 1.0, glow_val * 0.9),
                _ => Color::new(1.0, 0.45 * (1.0 - glow_val), 0.10 * (1.0 - glow_val), glow_val * 0.9),
            };
            // Barrel heat vents
            push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_up * 0.035, Vec3::new(0.045, 0.015, 0.14), w_dir, w_right, w_up, vent_color);
            push_oriented_box(&mut vertices, &mut indices, barrel_pos - w_up * 0.035, Vec3::new(0.045, 0.015, 0.14), w_dir, w_right, w_up, vent_color);
        }

        // 5. Mode-specific attachments
        match self.config.mode {
            WeaponMode::TitanRam => {
                let piston_ext = (self.recoil_pos.z * 0.35).max(0.0);
                let ram_pos = barrel_pos + w_dir * (0.12 + piston_ext);
                push_oriented_box(&mut vertices, &mut indices, ram_pos, Vec3::new(0.16, 0.14, 0.08), w_dir, w_right, w_up, Color::new(0.85, 0.42, 0.1, 1.0));
                push_oriented_box(&mut vertices, &mut indices, ram_pos + w_right * 0.07, Vec3::new(0.02, 0.02, 0.18), w_dir, w_right, w_up, Color::new(0.7, 0.6, 0.3, 1.0));
                push_oriented_box(&mut vertices, &mut indices, ram_pos - w_right * 0.07, Vec3::new(0.02, 0.02, 0.18), w_dir, w_right, w_up, Color::new(0.7, 0.6, 0.3, 1.0));
            }
            WeaponMode::HavocMortar => {
                let muzzle_flare = barrel_pos + w_dir * 0.10;
                push_oriented_box(&mut vertices, &mut indices, muzzle_flare, Vec3::new(0.10, 0.10, 0.08), w_dir, w_right, w_up, Color::new(0.35, 0.38, 0.30, 1.0));
                push_oriented_box(&mut vertices, &mut indices, origin + w_up * 0.08, Vec3::new(0.03, 0.04, 0.12), w_dir, w_right, w_up, Color::new(0.9, 0.6, 0.1, 1.0));
            }
            WeaponMode::EventHorizon => {
                let prong_pos = barrel_pos + w_dir * 0.08;
                push_oriented_box(&mut vertices, &mut indices, prong_pos + w_right * 0.04, Vec3::new(0.015, 0.05, 0.10), w_dir, w_right, w_up, Color::new(0.5, 0.2, 0.8, 1.0));
                push_oriented_box(&mut vertices, &mut indices, prong_pos - w_right * 0.04, Vec3::new(0.015, 0.05, 0.10), w_dir, w_right, w_up, Color::new(0.5, 0.2, 0.8, 1.0));
            }
            WeaponMode::SolsticeLance => {
                push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_dir * 0.04, Vec3::new(0.07, 0.07, 0.02), w_dir, w_right, w_up, Color::new(0.3, 0.7, 1.0, 1.0));
                push_oriented_box(&mut vertices, &mut indices, barrel_pos - w_dir * 0.02, Vec3::new(0.07, 0.07, 0.02), w_dir, w_right, w_up, Color::new(0.3, 0.7, 1.0, 1.0));
            }
            WeaponMode::ApexRailgun => {
                push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_right * 0.04, Vec3::new(0.02, 0.07, 0.26), w_dir, w_right, w_up, Color::new(0.3, 0.65, 1.0, 1.0));
                push_oriented_box(&mut vertices, &mut indices, barrel_pos - w_right * 0.04, Vec3::new(0.02, 0.07, 0.26), w_dir, w_right, w_up, Color::new(0.3, 0.65, 1.0, 1.0));
                push_oriented_box(&mut vertices, &mut indices, origin + w_up * 0.06, Vec3::new(0.04, 0.03, 0.16), w_dir, w_right, w_up, Color::new(0.4, 0.9, 1.0, 1.0));
            }
            WeaponMode::FrostbiteMortar => {
                push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_dir * 0.08, Vec3::new(0.08, 0.08, 0.06), w_dir, w_right, w_up, Color::new(0.6, 0.9, 1.0, 0.9));
            }
            WeaponMode::SeismicDisrupter => {
                let hammer_pos = barrel_pos + w_dir * 0.08;
                push_oriented_box(&mut vertices, &mut indices, hammer_pos, Vec3::new(0.12, 0.12, 0.10), w_dir, w_right, w_up, Color::new(0.40, 0.35, 0.28, 1.0));
                push_oriented_box(&mut vertices, &mut indices, hammer_pos + w_right * 0.06, Vec3::new(0.02, 0.04, 0.14), w_dir, w_right, w_up, Color::new(0.65, 0.40, 0.20, 1.0));
                push_oriented_box(&mut vertices, &mut indices, hammer_pos - w_right * 0.06, Vec3::new(0.02, 0.04, 0.14), w_dir, w_right, w_up, Color::new(0.65, 0.40, 0.20, 1.0));
            }
            WeaponMode::TeslaCaster => {
                let coil_pos1 = barrel_pos + w_right * 0.045;
                let coil_pos2 = barrel_pos - w_right * 0.045;
                push_oriented_box(&mut vertices, &mut indices, coil_pos1 + w_dir * 0.10, Vec3::new(0.015, 0.015, 0.06), w_dir, w_right, w_up, Color::new(0.85, 0.95, 1.0, 1.0));
                push_oriented_box(&mut vertices, &mut indices, coil_pos2 + w_dir * 0.10, Vec3::new(0.015, 0.015, 0.06), w_dir, w_right, w_up, Color::new(0.85, 0.95, 1.0, 1.0));
            }
            WeaponMode::PlasmaFlamer => {
                push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_up * 0.05, Vec3::new(0.08, 0.02, 0.14), w_dir, w_right, w_up, Color::new(0.55, 0.35, 0.20, 1.0));
            }
            WeaponMode::PlasmaWireSaw => {
                push_oriented_box(&mut vertices, &mut indices, barrel_pos + w_dir * 0.10, Vec3::new(0.14, 0.015, 0.18), w_dir, w_right, w_up, Color::new(0.85, 0.25, 0.95, 1.0));
            }
        }

        let viewmodel_mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };
        draw_mesh(&viewmodel_mesh);

        // Spheres and special focal emitters
        if self.heat > 0.40 {
            let vent_wisp_pos = barrel_pos + w_up * 0.045 + w_dir * 0.06;
            let glow_val = (self.heat * 1.3).min(1.0);
            let vent_color = match self.config.mode {
                WeaponMode::SolsticeLance | WeaponMode::ApexRailgun => Color::new(0.3 * (1.0 - glow_val) + 0.9 * glow_val, 0.95, 1.0, glow_val * 0.9),
                _ => Color::new(1.0, 0.45 * (1.0 - glow_val), 0.10 * (1.0 - glow_val), glow_val * 0.9),
            };
            draw_sphere(vent_wisp_pos, 0.012 * self.heat, None, Color::new(vent_color.r, vent_color.g, vent_color.b, self.heat * 0.6));
        }

        match self.config.mode {
            WeaponMode::EventHorizon => {
                let prong_pos = barrel_pos + w_dir * 0.08;
                draw_sphere(prong_pos, 0.045, None, Color::new(0.1, 0.02, 0.15, 1.0));
                draw_sphere(prong_pos, 0.035, None, Color::new(0.85, 0.25, 1.0, 0.9));
            }
            WeaponMode::SolsticeLance => {
                draw_sphere(barrel_pos + w_dir * 0.09, 0.035, None, Color::new(0.2, 0.95, 1.0, 1.0));
            }
            WeaponMode::FrostbiteMortar => {
                draw_sphere(barrel_pos + w_up * 0.04, 0.025, None, Color::new(0.7, 0.95, 1.0, 0.8));
            }
            WeaponMode::SeismicDisrupter => {
                let hammer_pos = barrel_pos + w_dir * 0.08;
                draw_cylinder(hammer_pos + w_dir * 0.06, 0.07, 0.05, 0.03, None, Color::new(0.95, 0.45, 0.15, 0.95));
            }
            WeaponMode::TeslaCaster => {
                let coil_pos1 = barrel_pos + w_right * 0.045;
                let coil_pos2 = barrel_pos - w_right * 0.045;
                draw_cylinder(coil_pos1, 0.035, 0.035, 0.16, None, Color::new(0.20, 0.50, 0.85, 1.0));
                draw_cylinder(coil_pos2, 0.035, 0.035, 0.16, None, Color::new(0.20, 0.50, 0.85, 1.0));
                draw_sphere(barrel_pos + w_dir * 0.04, 0.028, None, Color::new(0.40, 0.85, 1.0, 0.95));
            }
            WeaponMode::PlasmaFlamer => {
                let nozzle_pos = barrel_pos + w_dir * 0.08;
                draw_cylinder(nozzle_pos, 0.055, 0.04, 0.12, None, Color::new(0.35, 0.25, 0.20, 1.0));
                draw_sphere(nozzle_pos + w_dir * 0.06, 0.028, None, Color::new(1.0, 0.35, 0.10, 0.95));
                draw_cylinder(origin - w_up * 0.06, 0.04, 0.04, 0.18, None, Color::new(0.80, 0.30, 0.10, 1.0));
            }
            WeaponMode::PlasmaWireSaw => {
                let blade_emitter = barrel_pos + w_dir * 0.12;
                draw_cylinder(blade_emitter, 0.04, 0.01, 0.22, None, Color::new(0.92, 0.25, 1.0, 1.0));
            }
            _ => {}
        }
    }

    pub fn collect_dynamic_lights(&self, lights: &mut Vec<crate::physics::DynamicPointLight>) {
        // Singularities emit radiant gravitational violet light
        for sing in &self.singularities {
            lights.push(crate::physics::DynamicPointLight {
                position: sing.position,
                color: Color::new(0.85, 0.25, 1.0, 1.0),
                radius: 28.0,
                intensity: 2.2,
            });
        }
        // Active projectiles in flight (Rockets, Plasma, etc.)
        for proj in &self.projectiles {
            let (col, rad, intens) = match proj.mode {
                WeaponMode::HavocMortar => (Color::new(1.0, 0.65, 0.15, 1.0), 10.0, 1.3),
                WeaponMode::SolsticeLance | WeaponMode::ApexRailgun => (Color::new(0.3, 0.85, 1.0, 1.0), 9.0, 1.1),
                WeaponMode::PlasmaFlamer => (Color::new(1.0, 0.35, 0.08, 1.0), 7.0, 1.0),
                WeaponMode::FrostbiteMortar => (Color::new(0.5, 0.9, 1.0, 1.0), 8.5, 1.0),
                _ => (proj.mode.color(), 7.5, 0.85),
            };
            lights.push(crate::physics::DynamicPointLight {
                position: proj.position,
                color: col,
                radius: rad,
                intensity: intens,
            });
        }
        // Tesla lightning arcs emit flash illumination
        for arc in &self.tesla_arcs {
            if let Some(&(p1, _)) = arc.branches.first() {
                let alpha = (arc.life / arc.max_life).clamp(0.0, 1.0);
                lights.push(crate::physics::DynamicPointLight {
                    position: p1,
                    color: Color::new(0.4, 0.7, 1.0, 1.0),
                    radius: 12.0,
                    intensity: alpha * 1.5,
                });
            }
        }

        // Thermal laser continuous beam impact hotspot
        if self.is_firing_laser {
            if let Some(target) = self.laser_hit_pos {
                lights.push(crate::physics::DynamicPointLight {
                    position: target,
                    color: Color::new(0.25, 0.9, 1.0, 1.0),
                    radius: 9.0,
                    intensity: 1.4,
                });
            }
        }

        // C4 Shaped Charge blinking beacon LEDs
        for charge in &self.c4_charges {
            let blink_phase = (get_time() as f32 * 9.0 + charge.timer * 4.0).sin();
            if blink_phase > 0.1 {
                lights.push(crate::physics::DynamicPointLight {
                    position: charge.position + charge.normal * 0.25,
                    color: Color::new(1.0, 0.15, 0.1, 1.0),
                    radius: 4.5,
                    intensity: 0.85 * blink_phase,
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_oriented_box(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    center: Vec3,
    size: Vec3,
    dir: Vec3,
    right: Vec3,
    up: Vec3,
    color: Color,
) {
    let hx = right * (size.x * 0.5);
    let hy = up * (size.y * 0.5);
    let hz = dir * (size.z * 0.5);

    let faces: [(Vec3, Vec3, Vec3, Vec3, f32); 6] = [
        // Top (+Y)
        (center + hy - hx + hz, center + hy + hx + hz, center + hy + hx - hz, center + hy - hx - hz, 1.15),
        // Bottom (-Y)
        (center - hy - hx - hz, center - hy + hx - hz, center - hy + hx + hz, center - hy - hx + hz, 0.48),
        // Right (+X)
        (center + hx + hy + hz, center + hx - hy + hz, center + hx - hy - hz, center + hx + hy - hz, 0.88),
        // Left (-X)
        (center - hx + hy - hz, center - hx - hy - hz, center - hx - hy + hz, center - hx + hy + hz, 0.72),
        // Front (+Z / muzzle)
        (center + hz - hx + hy, center + hz + hx + hy, center + hz + hx - hy, center + hz - hx - hy, 0.98),
        // Back (-Z / stock)
        (center - hz + hx + hy, center - hz - hx + hy, center - hz - hx - hy, center - hz + hx - hy, 0.60),
    ];

    for &(p0, p1, p2, p3, shade) in &faces {
        let v_base = vertices.len() as u16;
        if v_base > 65530 { break; }

        let face_col = Color::new(
            (color.r * shade).min(1.0),
            (color.g * shade).min(1.0),
            (color.b * shade).min(1.0),
            color.a,
        );
        let c_raw: [u8; 4] = face_col.into();

        vertices.push(Vertex { position: p0, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        vertices.push(Vertex { position: p1, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        vertices.push(Vertex { position: p2, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
        vertices.push(Vertex { position: p3, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });

        indices.push(v_base);
        indices.push(v_base + 1);
        indices.push(v_base + 2);

        indices.push(v_base);
        indices.push(v_base + 2);
        indices.push(v_base + 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_dynamic_test_voxel(physics: &mut PhysicsSystem, center: Vec3, material: VoxelType) {
        physics.chunks.push(crate::physics::DynamicChunk::new(
            center, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            vec![(Vec3::ZERO, material)], material.properties().density,
            10.0, false, (Vec3::ZERO, Vec3::ZERO),
        ));
    }

    #[test]
    fn regression_wire_saw_clears_settled_parent_support_before_next_update() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        let voxels = vec![
            (Vec3::new(-0.5, 0.0, 0.0), VoxelType::Concrete),
            (Vec3::new(0.5, 0.0, 0.0), VoxelType::Concrete),
        ];
        let bounds = crate::physics::DynamicChunk::compute_aabb(&voxels);
        physics.chunks.push(crate::physics::DynamicChunk::new(
            Vec3::new(0.0, 5.0, 8.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
            voxels, 4.8, 10.0, true, bounds,
        ));
        physics.rebuild_rubble_heightmap_with_grid(&grid);
        for x in [-0.5, 0.5] {
            assert_eq!(physics.rubble_height_at(x, 8.0), 5.25);
        }

        weapon.set_mode(WeaponMode::PlasmaWireSaw, &audio);
        weapon.slice_plane_mode = 1;
        let camera = Vec3::new(0.0, 5.0, 0.0);
        weapon.fire_primary(camera, Vec3::Z, camera, &mut grid, &mut physics, &audio);

        assert_eq!(physics.chunks.len(), 2, "The vertical cut must replace the settled parent");
        assert!(physics.chunks.iter().all(|chunk| !chunk.settled));
        for x in [-0.5, 0.5] {
            assert_eq!(physics.rubble_height_at(x, 8.0), 0.5,
                "Moving children must not retain the parent's cached support before the next prepass");
        }
        assert!(physics.rubble_dirty);
    }

    #[test]
    fn solstice_ablates_dynamic_concrete_and_accumulates_armor_damage() {
        for (material, pulses) in [(VoxelType::Concrete, 1), (VoxelType::HeavyArmor, 2), (VoxelType::Bedrock, 3)] {
            let mut grid = VoxelGrid::new();
            let mut physics = PhysicsSystem::new();
            let audio = SoundSystem::empty();
            let mut weapon = WeaponSystem::new();
            weapon.config.mode = WeaponMode::SolsticeLance;
            let center = Vec3::new(10.13, 8.17, 10.29);
            let camera = center - Vec3::Z * 3.0;
            add_dynamic_test_voxel(&mut physics, center, material);
            for pulse in 0..pulses {
                weapon.fire_cooldown = 0.0;
                weapon.fire_primary(camera, Vec3::Z, camera, &mut grid, &mut physics, &audio);
                if material == VoxelType::HeavyArmor && pulse == 0 {
                    assert_eq!(physics.chunks.len(), 1, "Armor must survive the first pulse");
                }
            }
            if material == VoxelType::Bedrock {
                assert_eq!(physics.chunks[0].voxels[0].1, VoxelType::Bedrock, "Thermal ablation protects bedrock");
            } else {
                assert!(physics.chunks.is_empty(), "Accepted laser pulses must cut detached {material:?}");
            }
        }
    }

    #[test]
    fn solstice_dynamic_fuel_chain_uses_actual_moving_world_positions() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::SolsticeLance;
        let first = Vec3::new(10.13, 8.17, 10.29);
        let second = first + Vec3::X;
        let camera = first - Vec3::Z * 3.0;
        add_dynamic_test_voxel(&mut physics, first, VoxelType::Explosive);
        add_dynamic_test_voxel(&mut physics, second, VoxelType::Explosive);
        weapon.fire_primary(camera, Vec3::Z, camera, &mut grid, &mut physics, &audio);
        assert!(physics.chunks.is_empty(), "Fuel destroyed by the first blast must propagate the chain");
        let fuel_rings: Vec<_> = physics.shockwaves.iter().filter(|ring| {
            ring.color == Color::new(1.0, 0.45, 0.1, 0.9)
        }).collect();
        assert_eq!(fuel_rings.len(), 2, "Each destroyed fuel voxel detonates once");
        for expected in [first, second] {
            assert!(fuel_rings.iter().any(|ring| ring.position.distance(expected) < 1e-5),
                "Dynamic fuel detonation must retain {expected:?} rather than snap to a grid cell");
        }
    }

    #[test]
    fn dynamic_capacitor_arcs_from_actual_position_to_nearby_static_conductor() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::SolsticeLance;
        let center = Vec3::new(10.13, 8.17, 10.29);
        let target = grid.world_to_voxel(center + Vec3::X * 3.0);
        let target_world = grid.voxel_to_world(target);
        grid.set(target.x, target.y, target.z, VoxelType::PowerCapacitor);
        add_dynamic_test_voxel(&mut physics, center, VoxelType::PowerCapacitor);
        let camera = center - Vec3::Z * 3.0;
        weapon.fire_primary(camera, Vec3::Z, camera, &mut grid, &mut physics, &audio);
        assert!(physics.chunks.is_empty());
        assert_eq!(grid.get(target.x, target.y, target.z), VoxelType::Air);
        assert!(physics.electrical_arcs.iter().any(|arc| {
            arc.start.distance(center) < 1e-5 && arc.end.distance(target_world) < 1e-5
        }), "The dynamic capacitor must arc from its actual location to the nearby static conductor");
    }

    #[test]
    fn regression_dynamic_capacitor_arcs_to_static_conductor_in_its_source_cell() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let target = IVec3::new(116, 16, 116);
        let target_world = grid.voxel_to_world(target);
        let center = target_world + Vec3::new(0.13, 0.17, 0.21);
        assert_eq!(grid.world_to_voxel(center), target);
        grid.set(target.x, target.y, target.z, VoxelType::CompositeRebar);

        // This conductor survives the capacitor's initial breach; the arc must finish it.
        let dissipation = 1.0 - 0.45 / 24.0;
        assert!(VoxelType::CompositeRebar.effective_damage(180.0 * dissipation, DamageType::Thermal)
            < VoxelType::CompositeRebar.properties().max_health);
        add_dynamic_test_voxel(&mut physics, center, VoxelType::PowerCapacitor);
        let (destroyed, triggers) = physics.carve_chunk_sphere(
            &grid, center, 0.3, 140.0, DamageType::Thermal, true,
        );
        assert_eq!(destroyed, 1);
        assert_eq!(triggers, vec![(center, VoxelType::PowerCapacitor)]);
        process_explosive_chain_at_world_positions(
            &mut grid, &mut physics, &audio, Vec::new(), Vec::new(), Vec::new(),
            triggers, center - Vec3::Z * 3.0,
        );

        assert_eq!(grid.get(target.x, target.y, target.z), VoxelType::Air,
            "A detached capacitor's grid cell can still contain a separate static conductor");
        assert!(physics.electrical_arcs.iter().any(|arc| {
            arc.start.distance(center) < 1e-5 && arc.end.distance(target_world) < 1e-5
        }), "The arc must include the surviving static conductor at the source cell");
    }

    #[test]
    fn dynamic_volatile_queue_retains_twenty_four_detonation_limit() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let seeds: Vec<_> = (0..28).map(|i| (Vec3::new(i as f32 * 10.0, 20.0, 0.0), VoxelType::Explosive)).collect();
        process_explosive_chain_at_world_positions(
            &mut grid, &mut physics, &audio, Vec::new(), Vec::new(), Vec::new(), seeds, Vec3::ZERO,
        );
        assert_eq!(physics.shockwaves.iter().filter(|ring| {
            ring.color == Color::new(1.0, 0.45, 0.1, 0.9)
        }).count(), 24);
    }

    #[test]
    fn regression_singularity_erosion_detonates_each_volatile_material_before_collapse() {
        for (material, ring_color) in [
            (VoxelType::Explosive, Color::new(1.0, 0.45, 0.1, 0.9)),
            (VoxelType::GlowCore, Color::new(1.0, 0.9, 0.2, 1.0)),
            (VoxelType::PowerCapacitor, Color::new(0.68, 0.28, 0.98, 1.0)),
        ] {
            let mut grid = VoxelGrid::new();
            let mut physics = PhysicsSystem::new();
            let audio = SoundSystem::empty();
            let mut weapon = WeaponSystem::new();
            let source = IVec3::new(96, 16, 96);
            let center = grid.voxel_to_world(source);
            grid.set(source.x, source.y, source.z, material);
            weapon.singularities.push(ActiveSingularity {
                position: center, timer: 0.0, max_time: 2.4,
                pull_radius: ActiveSingularity::PULL_RADIUS,
                pull_strength: ActiveSingularity::PULL_STRENGTH,
                blast_radius: 18.0, vacuum_played: false, erosion_timer: 0.0,
            });

            weapon.update(0.16, &mut grid, &mut physics, &audio, center);

            assert_eq!(weapon.singularities.len(), 1, "This update must erode without imploding");
            assert_eq!(grid.get(source.x, source.y, source.z), VoxelType::Air);
            let detonations: Vec<_> = physics.shockwaves.iter().filter(|ring| ring.color == ring_color).collect();
            assert_eq!(detonations.len(), 1, "Erosion must propagate the destroyed {material:?} trigger exactly once");
            assert!(detonations[0].position.distance(center) < 1e-5);
        }
    }

    #[test]
    fn delayed_singularity_ablates_detached_material_before_outward_blast() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        let center = Vec3::new(10.13, 8.17, 10.29);
        add_dynamic_test_voxel(&mut physics, center + Vec3::X * 2.0, VoxelType::Concrete);
        add_dynamic_test_voxel(&mut physics, center + Vec3::X * 25.0, VoxelType::Concrete);
        weapon.singularities.push(ActiveSingularity {
            position: center, timer: 2.39, max_time: 2.4,
            pull_radius: ActiveSingularity::PULL_RADIUS,
            pull_strength: ActiveSingularity::PULL_STRENGTH,
            blast_radius: 18.0, vacuum_played: false, erosion_timer: 0.0,
        });
        weapon.update(0.02, &mut grid, &mut physics, &audio, center);
        assert!(weapon.singularities.is_empty());
        assert_eq!(physics.chunks.len(), 1, "Collapse removes gathered material inside its damage radius");
        assert!(physics.chunks[0].position.distance(center + Vec3::X * 25.0) < 1e-5,
            "Material outside the ablation radius survives");
        assert!(physics.chunks[0].velocity.x > 0.0, "Surviving material still receives the outward blast");
    }

    #[test]
    fn regression_foundation_impacts_match_square_extent_and_arena_hole() {
        for (x, z) in [(200.0, 200.0), (-200.0, 200.0), (200.0, -200.0), (-240.0, -240.0)] {
            let hit = surrounding_foundation_hit(Vec3::new(x, 1.0, z), Vec3::new(x, -1.0, z));
            assert_eq!(hit, Some(Vec3::new(x, 0.5, z)), "Visible slab corners must stop falling projectiles");
        }
        for (x, z) in [(240.1, 0.0), (0.0, -240.1), (0.0, 0.0)] {
            assert_eq!(surrounding_foundation_hit(Vec3::new(x, 1.0, z), Vec3::new(x, -1.0, z)), None,
                "The slab must not create a floor outside its extent or inside the destructible arena");
        }
    }

    #[test]
    fn regression_mortar_hits_square_corner_but_falls_beyond_slab() {
        for (x, should_hit) in [(200.0, true), (241.0, false)] {
            let mut grid = VoxelGrid::new();
            let mut physics = PhysicsSystem::new();
            let audio = SoundSystem::empty();
            let mut weapon = WeaponSystem::new();
            let muzzle = Vec3::new(x, 1.0, 200.0);
            weapon.fire_primary(muzzle, Vec3::NEG_Y, muzzle, &mut grid, &mut physics, &audio);
            assert_eq!(weapon.projectiles.len(), 1);
            weapon.update(0.05, &mut grid, &mut physics, &audio, muzzle);
            if should_hit {
                assert!(weapon.projectiles.is_empty(), "A mortar at the visible corner must detonate on impact");
            } else {
                assert_eq!(weapon.projectiles.len(), 1);
                assert!(weapon.projectiles[0].position.y < 0.5);
            }
        }
    }

    #[test]
    fn regression_mortar_enters_excavated_crater() {
        let mut grid = VoxelGrid::new();
        grid.carve_sphere_advanced(Vec3::ZERO, 5.0, 9000.0, DamageType::Kinetic);
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::HavocMortar;
        let muzzle = Vec3::new(0.25, 1.0, 0.25);
        weapon.fire_primary(muzzle, Vec3::NEG_Y, muzzle, &mut grid, &mut physics, &audio);
        weapon.update(0.04, &mut grid, &mut physics, &audio, muzzle);

        assert_eq!(weapon.projectiles.len(), 1, "No invisible ground should detonate a shell over a crater");
        assert!(weapon.projectiles[0].position.y < 0.0);
        weapon.update(0.15, &mut grid, &mut physics, &audio, muzzle);
        assert!(weapon.projectiles.is_empty(), "Shell must still impact the real crater floor");
    }

    #[test]
    fn regression_projectile_hits_nearer_chunk_before_voxel_wall() {
        let mut grid = VoxelGrid::new();
        grid.set(96, 10, 103, VoxelType::Concrete);
        let mut physics = PhysicsSystem::new();
        physics.chunks.push(crate::physics::DynamicChunk {
            position: Vec3::new(0.25, 5.25, 2.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: crate::physics::Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 50.0,
            life: 0.0,
            settled: false,
            local_aabb: (-Vec3::splat(0.25), Vec3::splat(0.25)),
        });
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::EventHorizon;
        let muzzle = Vec3::new(0.25, 5.25, 0.25);
        weapon.fire_primary(muzzle, Vec3::Z, muzzle, &mut grid, &mut physics, &audio);
        weapon.update(0.15, &mut grid, &mut physics, &audio, muzzle);

        assert_eq!(weapon.singularities.len(), 1);
        assert!(weapon.singularities[0].position.z < 2.0, "Projectile must hit the chunk before the wall: {:?}", weapon.singularities[0].position);
    }


    #[test]
    fn test_weapon_mode_archetypes() {
        let mut wpn = WeaponSystem::new();
        assert_eq!(wpn.config.mode, WeaponMode::HavocMortar);

        let modes = [
            WeaponMode::TitanRam,
            WeaponMode::HavocMortar,
            WeaponMode::EventHorizon,
            WeaponMode::SolsticeLance,
            WeaponMode::ApexRailgun,
            WeaponMode::FrostbiteMortar,
            WeaponMode::SeismicDisrupter,
            WeaponMode::TeslaCaster,
            WeaponMode::PlasmaFlamer,
            WeaponMode::PlasmaWireSaw,
        ];

        for mode in modes {
            wpn.config.mode = mode;
            assert!(!wpn.config.mode.name().is_empty());
            assert!(!wpn.config.mode.description().is_empty());
            let col = wpn.config.mode.color();
            assert!(col.a > 0.0);
        }
    }

    #[test]
    fn test_customization_payload_and_stabilizers() {
        let custom = WeaponCustomization::default();
        assert_eq!(custom.payload, PayloadType::HighExplosive);
        assert_eq!(custom.trail_style, TrailStyle::PlasmaGlow);
        assert_eq!(custom.stabilizer, StabilizerType::Standard);
        assert!((custom.velocity_mult - 1.0).abs() < 0.001);

        assert_eq!(PayloadType::ShapedCharge.name(), "SHAPED CHARGE (ARMOR PIERCING)");
        assert_eq!(StabilizerType::HeavyCompensator.name(), "HEAVY COMPENSATOR (-60% RECOIL)");
        assert_eq!(TrailStyle::SmokeWisps.name(), "ATMOSPHERIC WISPS");
    }

    #[test]
    fn test_customization_defaults() {
        let custom = WeaponCustomization::default();
        assert_eq!(custom.barrel, BarrelType::Precision);
        assert_eq!(custom.detonator, DetonatorType::InstantImpact);
        assert_eq!(custom.barrel.name(), "PRECISION BORE");
        assert_eq!(custom.detonator.name(), "INSTANT IMPACT");
        assert_eq!(DetonatorType::RemoteSticky.name(), "REMOTE STICKY");
        assert_eq!(BarrelType::TriSpread.name(), "TRI-SPREAD FAN");
        assert!((custom.blast_radius_mult - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_titan_maul_cone_carves_and_records_explosives() {
        let mut grid = VoxelGrid::new();
        // Place explosive canister in front of player
        let exp_pos = IVec3::new(32, 5, 28);
        grid.set(exp_pos.x, exp_pos.y, exp_pos.z, crate::voxel::VoxelType::Explosive);

        let exp_world = grid.voxel_to_world(exp_pos);
        // Carve sphere covering the explosive
        let res = grid.carve_sphere(exp_world, 1.5);
        assert!(!res.explosive_triggers.is_empty(), "Carve must detect and record explosive canister triggers");
        assert_eq!(grid.get(exp_pos.x, exp_pos.y, exp_pos.z), crate::voxel::VoxelType::Air);
    }

    #[test]
    fn test_recoil_basis_vectors_tilt_up() {
        let recoil_rot = Vec3::new(25.0, 0.0, 0.0); // 25 degree pitch kick
        let camera_dir = Vec3::new(0.0, 0.0, -1.0);
        let up = Vec3::Y;
        let _right = Vec3::X;

        let kick_pitch = recoil_rot.x.to_radians();
        let w_dir = (camera_dir + up * kick_pitch.sin()).normalize();

        assert!(w_dir.y > 0.0, "Weapon forward axis must tilt upwards during recoil kick");
        assert!(w_dir.z < 0.0, "Weapon forward axis must remain facing forward");
    }

    #[test]
    fn test_process_explosive_chain_multi_hop() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Place primary canister and secondary canister 3 units away
        let can1 = IVec3::new(30, 5, 30);
        let can2 = IVec3::new(33, 5, 30);
        grid.set(can1.x, can1.y, can1.z, crate::voxel::VoxelType::Explosive);
        grid.set(can2.x, can2.y, can2.z, crate::voxel::VoxelType::Explosive);

        let trauma = process_explosive_chain(
            &mut grid,
            &mut physics,
            &audio,
            vec![can1],
            Vec::new(),
            Vec3::new(0.0, 5.0, 30.0),
        );

        assert!(trauma > 0.0, "Explosive chain reaction must produce trauma");
        assert_eq!(grid.get(can1.x, can1.y, can1.z), crate::voxel::VoxelType::Air, "Canister 1 must be destroyed");
        assert_eq!(grid.get(can2.x, can2.y, can2.z), crate::voxel::VoxelType::Air, "Canister 2 must be detonated in chain");
    }

    #[test]
    fn test_railgun_does_not_carve_behind_player() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::ApexRailgun;

        let cam_pos = grid.voxel_to_world(IVec3::new(30, 5, 30));
        let cam_dir = Vec3::new(0.0, 0.0, -1.0); // Facing North / -Z
        let muzzle_pos = cam_pos + cam_dir * 0.5;

        // Place wall behind player (+Z direction)
        let behind_pos = IVec3::new(30, 5, 33);
        grid.set(behind_pos.x, behind_pos.y, behind_pos.z, crate::voxel::VoxelType::HeavyArmor);

        // Place wall in front of player (-Z direction)
        let front_pos = IVec3::new(30, 5, 20);
        grid.set(front_pos.x, front_pos.y, front_pos.z, crate::voxel::VoxelType::Concrete);

        weapon.fire_primary(muzzle_pos, cam_dir, cam_pos, &mut grid, &mut physics, &audio);

        assert_eq!(
            grid.get(behind_pos.x, behind_pos.y, behind_pos.z),
            crate::voxel::VoxelType::HeavyArmor,
            "Railgun cylinder must never carve behind the player"
        );
        assert_eq!(
            grid.get(front_pos.x, front_pos.y, front_pos.z),
            crate::voxel::VoxelType::Air,
            "Railgun cylinder must pierce through wall in front of player"
        );
    }

    #[test]
    fn test_rocket_projectile_trail_points_and_smoke_wisps() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::HavocMortar;
        weapon.config.trail_style = TrailStyle::SmokeWisps;

        let cam_pos = Vec3::new(10.0, 15.0, 10.0);
        let cam_dir = Vec3::new(0.0, 0.0, 1.0);
        let muzzle_pos = cam_pos + cam_dir * 0.6;

        weapon.fire_primary(muzzle_pos, cam_dir, cam_pos, &mut grid, &mut physics, &audio);

        assert_eq!(weapon.projectiles.len(), 1, "Havoc mortar must fire 1 primary projectile");
        let proj = &weapon.projectiles[0];
        assert_eq!(proj.trail_style, TrailStyle::SmokeWisps);
        assert!(!proj.trail_points.is_empty(), "Muzzle point must be in trail_points");

        // Advance 10 ticks in empty space
        for _ in 0..10 {
            weapon.update(0.016, &mut grid, &mut physics, &audio, cam_pos);
        }

        assert_eq!(weapon.projectiles.len(), 1, "Projectile should still be in flight");
        assert!(weapon.projectiles[0].trail_points.len() > 1, "Trail points must accumulate for ribbon rendering");

        // Verify any spawned smoke particles have subtle alpha and start_size
        for sm in &physics.smoke {
            assert!(sm.color.a <= 0.15, "Smoke particle alpha must not exceed 0.15, got {}", sm.color.a);
            assert!(sm.start_size > 0.0, "Smoke particle start_size must be positive");
            assert!(sm.size <= sm.max_size, "Smoke particle size must not exceed max_size");
        }
    }

    #[test]
    fn test_beam_and_sniper_recoil_profiles_have_minimal_trauma() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();

        // 1. Solstice Lance (Continuous Laser) must have zero screen trauma and zero player recoil
        weapon.config.mode = WeaponMode::SolsticeLance;
        let cam_pos = Vec3::new(0.0, 5.0, 0.0);
        let cam_dir = Vec3::new(0.0, -0.3, -1.0).normalize();
        let muzzle = cam_pos + cam_dir * 0.5;

        let lance_feedback = weapon.fire_primary(muzzle, cam_dir, cam_pos, &mut grid, &mut physics, &audio);
        assert_eq!(lance_feedback.camera_trauma, 0.0, "Solstice Lance must produce 0.0 camera trauma");
        assert_eq!(lance_feedback.player_recoil, Vec3::ZERO, "Solstice Lance must not kick player across floor");
        assert_eq!(lance_feedback.fov_kick, 0.0, "Solstice Lance must have 0.0 FOV kick for sniper stability");

        // 2. Apex Railgun must have minimal crisp trauma and strictly horizontal player recoil
        weapon.fire_cooldown = 0.0;
        weapon.config.mode = WeaponMode::ApexRailgun;
        let rail_feedback = weapon.fire_primary(muzzle, cam_dir, cam_pos, &mut grid, &mut physics, &audio);
        assert!(rail_feedback.camera_trauma <= 0.05, "Apex Railgun camera trauma must be <= 0.05, got {}", rail_feedback.camera_trauma);
        assert_eq!(rail_feedback.player_recoil.y, 0.0, "Railgun player recoil must be horizontal (no vertical launch)");
        assert!(rail_feedback.player_recoil.length() < 1.0, "Railgun recoil push must be gentle and controlled");
    }

    #[test]
    fn test_weapon_spring_damper_recovery() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();

        // Apply kick to viewmodel recoil spring
        weapon.recoil_pos = Vec3::new(0.0, 0.08, -0.35);
        weapon.recoil_pos_vel = Vec3::new(0.0, 0.4, -2.5);
        weapon.recoil_rot = Vec3::new(14.0, 0.0, 0.0);
        weapon.recoil_rot_vel = Vec3::new(85.0, 0.0, 0.0);

        // Advance 0.4 seconds of dt ticks
        for _ in 0..25 {
            weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);
        }

        assert!(weapon.recoil_pos.length() < 0.04, "Recoil position must settle near zero under spring damper");
        assert!(weapon.recoil_rot.length() < 1.5, "Recoil rotation must settle near zero under spring damper");
    }

    #[test]
    fn test_weapon_heat_accumulation_and_cooling() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::ApexRailgun;

        let cam_pos = Vec3::new(0.0, 5.0, 0.0);
        let cam_dir = Vec3::new(0.0, 0.0, -1.0);
        let muzzle = cam_pos + cam_dir * 0.5;

        assert_eq!(weapon.heat, 0.0);
        weapon.fire_primary(muzzle, cam_dir, cam_pos, &mut grid, &mut physics, &audio);
        assert!(weapon.heat > 0.2, "Firing high-energy weapon must generate heat, got {}", weapon.heat);

        let initial_heat = weapon.heat;
        // Cool down
        for _ in 0..30 {
            weapon.update(0.016, &mut grid, &mut physics, &audio, cam_pos);
        }
        assert!(weapon.heat < initial_heat, "Weapon heat must dissipate over time");
    }

    #[test]
    fn test_oriented_box_viewmodel_mesh() {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let center = Vec3::new(0.0, 1.5, 0.0);
        let size = Vec3::new(0.2, 0.3, 0.8);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let right = Vec3::X;
        let up = Vec3::Y;
        let color = Color::new(0.5, 0.5, 0.5, 1.0);

        push_oriented_box(&mut vertices, &mut indices, center, size, dir, right, up, color);

        assert_eq!(vertices.len(), 24, "6 faces * 4 vertices = 24 vertices");
        assert_eq!(indices.len(), 36, "6 faces * 2 triangles * 3 indices = 36 indices");

        // Verify vertices are bounded around center
        for v in &vertices {
            let offset = v.position - center;
            assert!(offset.x.abs() <= 0.101);
            assert!(offset.y.abs() <= 0.151);
            assert!(offset.z.abs() <= 0.401);
        }
    }

    #[test]
    fn test_trimodal_chain_reaction_power_capacitor_discharge_and_arc_jump() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let cap1 = IVec3::new(30, 5, 30);
        let cap2 = IVec3::new(34, 5, 30); // 2 meters away (within 7m arc range)
        grid.set(cap1.x, cap1.y, cap1.z, VoxelType::PowerCapacitor);
        grid.set(cap2.x, cap2.y, cap2.z, VoxelType::PowerCapacitor);

        let trauma = process_explosive_chain_with_capacitors(
            &mut grid,
            &mut physics,
            &audio,
            Vec::new(),
            Vec::new(),
            vec![cap1],
            Vec3::new(30.0, 5.0, 40.0),
        );

        assert!(trauma > 0.0, "Capacitor discharge must generate trauma");
        assert_eq!(grid.get(cap1.x, cap1.y, cap1.z), VoxelType::Air, "Capacitor 1 must be destroyed");
        assert_eq!(grid.get(cap2.x, cap2.y, cap2.z), VoxelType::Air, "Capacitor 2 must be destroyed by electric arc overload");

        // Must spawn violet electric shockwave
        let has_violet_shockwave = physics.shockwaves.iter().any(|sw| {
            (sw.color.r - 0.68).abs() < 0.05 && (sw.color.b - 0.98).abs() < 0.05
        });
        assert!(has_violet_shockwave, "Capacitor breach must spawn neon violet shockwave ring");
    }

    #[test]
    fn test_trimodal_mixed_triad_cascade_explosive_core_and_capacitor() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let exp_pos = IVec3::new(25, 6, 25);
        let core_pos = IVec3::new(27, 6, 25);
        let cap_pos = IVec3::new(29, 6, 25);

        grid.set(exp_pos.x, exp_pos.y, exp_pos.z, VoxelType::Explosive);
        grid.set(core_pos.x, core_pos.y, core_pos.z, VoxelType::GlowCore);
        grid.set(cap_pos.x, cap_pos.y, cap_pos.z, VoxelType::PowerCapacitor);

        let world_exp = grid.voxel_to_world(exp_pos);
        let trauma = process_explosive_chain_with_capacitors(
            &mut grid,
            &mut physics,
            &audio,
            vec![exp_pos],
            Vec::new(),
            Vec::new(),
            world_exp + Vec3::new(0.0, 1.0, 2.0),
        );

        assert!(trauma >= 0.20, "Mixed triad cascade must generate cumulative trauma, got {}", trauma);
        assert_eq!(grid.get(exp_pos.x, exp_pos.y, exp_pos.z), VoxelType::Air);
        assert_eq!(grid.get(core_pos.x, core_pos.y, core_pos.z), VoxelType::Air);
        assert_eq!(grid.get(cap_pos.x, cap_pos.y, cap_pos.z), VoxelType::Air);
    }

    #[test]
    fn test_solstice_lance_chunk_hit_does_not_carve_grid() {
        let mut grid = VoxelGrid::new();
        grid.update_meshes();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Spawn a dynamic chunk directly ahead
        physics.chunks.push(crate::physics::DynamicChunk {
            position: Vec3::new(0.0, 10.0, 5.0),
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: crate::physics::Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            voxel_health: Vec::new(),
            exposed_voxels: Vec::new(),
            mass: 5.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        let mut weapon = WeaponSystem::new();
        weapon.config.mode = WeaponMode::SolsticeLance;

        let camera_pos = Vec3::new(0.0, 10.0, 0.0);
        let camera_dir = Vec3::new(0.0, 0.0, 1.0);
        let muzzle = camera_pos + camera_dir * 0.5;

        let initial_solid = grid.solid_count();
        let _feedback = weapon.fire_primary(muzzle, camera_dir, camera_pos, &mut grid, &mut physics, &audio);

        assert_eq!(grid.solid_count(), initial_solid, "Hitting dynamic chunk must not modify static grid");
        assert!(weapon.laser_hit_pos.is_some());
    }

    #[test]
    fn regression_mode_switch_ends_continuous_beam_and_discards_charge() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();
        let camera_pos = Vec3::new(0.0, 10.0, 0.0);
        let camera_dir = Vec3::Z;
        weapon.set_mode(WeaponMode::SolsticeLance, &audio);
        weapon.fire_primary(camera_pos, camera_dir, camera_pos, &mut grid, &mut physics, &audio);
        assert!(weapon.is_firing_laser);
        assert!(weapon.laser_hit_pos.is_some());
        assert!(weapon.laser_sound_timer > 0.0);

        weapon.set_mode(WeaponMode::HavocMortar, &audio);
        weapon.update(0.1, &mut grid, &mut physics, &audio, camera_pos);
        assert!(!weapon.is_firing_laser, "Switching to a projectile weapon must stop drawing the prior laser");
        assert_eq!(weapon.laser_hit_pos, None);
        assert_eq!(weapon.laser_sound_timer, 0.0);

        weapon.set_mode(WeaponMode::ApexRailgun, &audio);
        weapon.railgun_charge = 0.6;
        weapon.set_mode(WeaponMode::TitanRam, &audio);
        weapon.set_mode(WeaponMode::ApexRailgun, &audio);
        assert_eq!(weapon.railgun_charge, 0.0, "Re-equipping the railgun must not resume a prior charge");
    }

    #[test]
    fn test_c4_shaped_charges_placement_cap_and_coordinated_detonation() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();

        // 1. Verify placement cap of 24 charges
        for i in 0..MAX_C4_CHARGES {
            grid.set(i as i32, 4, 10, VoxelType::Concrete);
            let pos = Vec3::new(i as f32, 5.0, 10.0);
            let normal = Vec3::Y;
            let placed = weapon.place_c4_charge(pos, normal, Some(IVec3::new(i as i32, 4, 10)));
            assert!(placed, "Placing charge {} up to 24 should succeed", i);
        }
        assert_eq!(weapon.c4_charges.len(), MAX_C4_CHARGES);

        // 25th charge should be rejected by cap
        let placed_25th = weapon.place_c4_charge(Vec3::new(25.0, 5.0, 10.0), Vec3::Y, None);
        assert!(!placed_25th, "25th charge must be rejected by cap of 24");
        assert_eq!(weapon.c4_charges.len(), MAX_C4_CHARGES);

        // 2. Test anchor linkage: if anchor voxel becomes air, charge drops in update
        // Let's set anchor voxel 0 to Air:
        grid.set(0, 4, 10, VoxelType::Air);
        weapon.update(0.016, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(weapon.c4_charges.len(), MAX_C4_CHARGES - 1, "C4 charge whose anchor voxel was destroyed must be removed");

        // 3. Test coordinated simultaneous detonation
        let (feedback, detonated_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::new(0.0, 5.0, 0.0));
        assert_eq!(detonated_count, MAX_C4_CHARGES - 1);
        assert_eq!(weapon.c4_charges.len(), 0, "Detonating all C4 should clear active charges");
        assert!(feedback.camera_trauma >= 0.75, "Simultaneous C4 detonation must produce strong camera trauma");
        assert!(feedback.fov_kick >= 6.0, "FOV kick must scale with grid size");

        // 4. Detonating when empty should return 0 count and 0 trauma
        let (empty_feedback, empty_count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(empty_count, 0);
        assert_eq!(empty_feedback.camera_trauma, 0.0);
    }

    #[test]
    fn test_c4_laser_det_cord_links_and_rng_topology() {
        let mut weapon = WeaponSystem::new();

        // Place 4 charges in a 10m x 10m square on columns:
        // C0: (0, 4, 0), C1: (10, 4, 0), C2: (10, 4, 10), C3: (0, 4, 10)
        weapon.place_c4_charge(Vec3::new(0.0, 4.0, 0.0), Vec3::Y, None);
        weapon.place_c4_charge(Vec3::new(10.0, 4.0, 0.0), Vec3::Y, None);
        weapon.place_c4_charge(Vec3::new(10.0, 4.0, 10.0), Vec3::Y, None);
        weapon.place_c4_charge(Vec3::new(0.0, 4.0, 10.0), Vec3::Y, None);

        let links = weapon.compute_det_cord_links();
        // RNG on a square must produce exactly the 4 perimeter edges (0-1, 1-2, 2-3, 3-0)
        assert_eq!(links.len(), 4, "Square perimeter must have exactly 4 RNG edges");

        // Verify no diagonal edges (0-2 or 1-3) exist:
        for link in &links {
            let pair = (link.idx_a.min(link.idx_b), link.idx_a.max(link.idx_b));
            assert_ne!(pair, (0, 2), "Diagonal chord 0-2 must be pruned by RNG");
            assert_ne!(pair, (1, 3), "Diagonal chord 1-3 must be pruned by RNG");
        }

        // Add a 5th charge beyond MAX_DET_CORD_DIST (25.0m)
        weapon.place_c4_charge(Vec3::new(50.0, 4.0, 0.0), Vec3::Y, None);
        let links2 = weapon.compute_det_cord_links();
        assert_eq!(links2.len(), 4, "Charge beyond 25m cutoff must not form det-cord links");
    }

    #[test]
    fn test_c4_foundation_cutting_shock_severs_columns() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();

        // Build continuous concrete foundation beam from (2, 4, 5) to (8, 4, 5)
        for x in 2..=8 {
            grid.set(x, 4, 5, VoxelType::Concrete);
        }
        assert_eq!(grid.get(5, 4, 5), VoxelType::Concrete);

        // Place C4 charges at both ends: (2, 4, 5) and (8, 4, 5)
        let p1 = grid.voxel_to_world(IVec3::new(2, 4, 5));
        let p2 = grid.voxel_to_world(IVec3::new(8, 4, 5));
        weapon.place_c4_charge(p1, Vec3::Y, Some(IVec3::new(2, 4, 5)));
        weapon.place_c4_charge(p2, Vec3::Y, Some(IVec3::new(8, 4, 5)));

        // Detonate coordinated demolition
        let (_fb, count) = weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(count, 2);

        // Middle of the beam at (5, 4, 5) must be severed to Air by the 2.8m diameter cutting cylinder
        assert_eq!(grid.get(5, 4, 5), VoxelType::Air, "Center of column beam must be severed by cutting shock");
    }

    #[test]
    fn test_singularity_constants_and_35m_pull_radius() {
        assert_eq!(ActiveSingularity::PULL_RADIUS, 35.0);
        assert_eq!(ActiveSingularity::PULL_STRENGTH, 72.0);
        assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MIN, 16.0);
        assert_eq!(ActiveSingularity::COLLAPSE_RADIUS_MAX, 22.0);
    }

    #[test]
    fn singularity_preserves_launch_settings_and_scales_delayed_impulse() {
        fn delayed_blast(mult: f32, stabilizer: StabilizerType) -> Vec3 {
            let mut grid = VoxelGrid::new();
            let mut physics = PhysicsSystem::new();
            let audio = SoundSystem::empty();
            let mut weapon = WeaponSystem::new();
            // Outside the 35m suction radius but inside the 22m collapse's blast
            // reach, so this body measures only the delayed kinetic impulse.
            physics.chunks.push(crate::physics::DynamicChunk::new(
                Vec3::new(38.0, 0.5, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::ZERO,
                vec![(Vec3::ZERO, VoxelType::Concrete)], 5.0, 0.0, false,
                (Vec3::ZERO, Vec3::ZERO),
            ));
            weapon.config.mode = WeaponMode::EventHorizon;
            weapon.config.blast_radius_mult = 3.0;
            weapon.config.impulse_mult = mult;
            weapon.config.stabilizer = stabilizer;
            let camera = Vec3::new(0.0, 3.0, 0.0);
            weapon.fire_primary(camera - Vec3::Y * 0.5, Vec3::NEG_Y, camera,
                &mut grid, &mut physics, &audio);
            let scale = mult * if stabilizer == StabilizerType::Overclocked { 1.3 } else { 1.0 };
            // Changing a preset after launch must not retune an existing shot.
            weapon.config.blast_radius_mult = 0.5;
            weapon.config.impulse_mult = 3.5;
            for _ in 0..10 {
                weapon.update(0.05, &mut grid, &mut physics, &audio, camera);
                if !weapon.singularities.is_empty() { break; }
            }
            let sing = weapon.singularities.first().expect("Orb hits the foundation");
            assert_eq!(sing.blast_radius, 22.0, "Launch radius survives workbench edits");
            assert!((sing.pull_strength - ActiveSingularity::PULL_STRENGTH * scale).abs() < 1e-4);
            // Skip movement integration to isolate the actual delayed blast from
            // gravity, contacts, and the fixed visual/vortex lift component.
            for _ in 0..60 {
                weapon.update(0.05, &mut grid, &mut physics, &audio, camera);
                if weapon.singularities.is_empty() { break; }
            }
            assert!(weapon.singularities.is_empty(), "Delayed collapse must occur");
            physics.chunks[0].velocity
        }
        let baseline = delayed_blast(1.0, StabilizerType::Standard);
        assert!((baseline.x - 7.4328437).abs() < 1e-4, "Default strength preserves the previous blast");
        for mult in [0.5, 2.0, 3.5] {
            let velocity = delayed_blast(mult, StabilizerType::Standard);
            assert!((velocity - baseline * mult).length() < 1e-4,
                "Configured force must reach the delayed blast: {mult} -> {velocity:?}");
        }
        let overclocked = delayed_blast(1.0, StabilizerType::Overclocked);
        assert!((overclocked - baseline * 1.3).length() < 1e-4);
    }

    #[test]
    fn test_singularity_continuous_erosion_carving() {
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let mut weapon = WeaponSystem::new();

        // Build concrete structure around (10, 5, 10)
        for dx in -3..=3 {
            for dy in -3..=3 {
                for dz in -3..=3 {
                    grid.set(10 + dx, 5 + dy, 10 + dz, VoxelType::Concrete);
                }
            }
        }
        assert_eq!(grid.get(10, 5, 10), VoxelType::Concrete);

        // Spawn singularity at center of concrete structure
        let center_world = grid.voxel_to_world(IVec3::new(10, 5, 10));
        weapon.singularities.push(ActiveSingularity {
            position: center_world,
            timer: 0.0,
            max_time: 2.4,
            pull_radius: ActiveSingularity::PULL_RADIUS,
            pull_strength: ActiveSingularity::PULL_STRENGTH,
            blast_radius: 18.0,
            vacuum_played: false,
            erosion_timer: 0.0,
        });

        // Step by 0.20s (exceeding 0.15s erosion interval)
        weapon.update(0.20, &mut grid, &mut physics, &audio, Vec3::ZERO);

        // Center voxels must have been eroded to Air
        assert_eq!(grid.get(10, 5, 10), VoxelType::Air, "Singularity continuous erosion must carve voxels");
    }

    #[test]
    fn test_g6_m2_weapon_crater_carve_invalidates_rubble() {
        let mut grid = VoxelGrid::new();
        let mut weapon = WeaponSystem::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        physics.rubble_dirty = false;

        weapon.c4_charges.push(C4Charge {
            position: Vec3::new(10.0, 1.0, 10.0),
            normal: Vec3::Y,
            anchor_voxel: Some(IVec3::new(20, 1, 20)),
            timer: 0.0,
            blast_radius: 5.5,
            impulse: 48.0,
        });

        weapon.detonate_all_c4(&mut grid, &mut physics, &audio, Vec3::ZERO);

        assert!(
            physics.rubble_dirty,
            "C4 crater carve must set physics.rubble_dirty = true"
        );
        assert!(
            physics.rubble_dirty_region.is_some(),
            "C4 crater carve must mark localized rubble dirty region"
        );
    }
}

