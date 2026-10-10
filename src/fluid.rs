use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, VoxelType, DamageType};
use crate::physics::{PhysicsSystem, DebrisParticle};
use crate::audio::SoundSystem;

pub const MAX_FLUID_PARTICLES: usize = 1024;

/// The three distinct cellular fluid types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidType {
    Water,      // Pressurized water: flows fast, washes rubble, extinguishes fire
    MoltenSlag, // Superheated molten slag: melts columns, cools into solid Basalt
    ToxicGas,   // Volumetric volatile gas: drifts through halls, explosive ignition
}

impl FluidType {
    pub fn name(&self) -> &'static str {
        match self {
            FluidType::Water => "PRESSURIZED WATER",
            FluidType::MoltenSlag => "MOLTEN SLAG / LAVA",
            FluidType::ToxicGas => "VOLATILE TOXIC GAS",
        }
    }

    pub fn base_color(&self) -> Color {
        match self {
            FluidType::Water => Color::new(0.18, 0.55, 0.95, 0.75),
            FluidType::MoltenSlag => Color::new(1.0, 0.42, 0.05, 0.95),
            FluidType::ToxicGas => Color::new(0.25, 0.85, 0.30, 0.45),
        }
    }
}

/// An individual fluid cell / particle in the 3D cellular dynamics simulation
#[derive(Debug, Clone, Copy)]
pub struct FluidParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub fluid_type: FluidType,
    pub temperature: f32, // For molten slag cooling (1000.0 -> 0.0) or water
    pub life: f32,
    pub max_life: f32,
    pub radius: f32,
    pub active: bool,
}

impl Default for FluidParticle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            fluid_type: FluidType::Water,
            temperature: 20.0,
            life: 0.0,
            max_life: 1.0,
            radius: 0.25,
            active: false,
        }
    }
}

/// Localized Cellular Fluid Dynamics Simulator
pub struct FluidSystem {
    ambient_stress_timer: f32,
    pub particles: Vec<FluidParticle>,
    pub step_accumulator: f32,
    pub total_spawned: usize,
    pub basalt_conversions: usize,
    pub gas_detonations: usize,
}

impl FluidSystem {
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(MAX_FLUID_PARTICLES);
        for _ in 0..MAX_FLUID_PARTICLES {
            particles.push(FluidParticle::default());
        }
        Self {
            ambient_stress_timer: 4.0,
            particles,
            step_accumulator: 0.0,
            total_spawned: 0,
            basalt_conversions: 0,
            gas_detonations: 0,
        }
    }

    pub fn clear(&mut self) {
        self.ambient_stress_timer = 4.0;
        for p in &mut self.particles {
            p.active = false;
        }
        self.step_accumulator = 0.0;
        self.total_spawned = 0;
        self.basalt_conversions = 0;
        self.gas_detonations = 0;
    }

    pub fn active_count(&self) -> usize {
        self.particles.iter().filter(|p| p.active).count()
    }

    /// Spawns high-velocity pressurized water cells (e.g. from severed cooling pipes or water tanks)
    pub fn spawn_water(&mut self, origin: Vec3, initial_vel: Vec3, count: usize) {
        for p in self.particles.iter_mut().filter(|p| !p.active).take(count) {
            let jitter = Vec3::new(
                rand::gen_range(-0.35f32, 0.35),
                rand::gen_range(-0.15f32, 0.35),
                rand::gen_range(-0.35f32, 0.35),
            );
            p.position = origin + jitter;
            p.velocity = initial_vel + jitter * 4.0;
            p.fluid_type = FluidType::Water;
            p.temperature = 15.0;
            p.life = rand::gen_range(4.0f32, 7.5);
            p.max_life = p.life;
            p.radius = rand::gen_range(0.20f32, 0.35);
            p.active = true;
            self.total_spawned += 1;
        }
    }

    /// Spawns molten slag / lava cells (e.g. from breached fusion conduits or reactors)
    pub fn spawn_slag(&mut self, origin: Vec3, initial_vel: Vec3, count: usize) {
        for p in self.particles.iter_mut().filter(|p| !p.active).take(count) {
            let jitter = Vec3::new(
                rand::gen_range(-0.25f32, 0.25),
                rand::gen_range(0.0f32, 0.30),
                rand::gen_range(-0.25f32, 0.25),
            );
            p.position = origin + jitter;
            p.velocity = initial_vel + jitter * 2.5;
            p.fluid_type = FluidType::MoltenSlag;
            p.temperature = 1200.0; // Glowing hot
            p.life = rand::gen_range(6.0f32, 10.0);
            p.max_life = p.life;
            p.radius = rand::gen_range(0.25f32, 0.40);
            p.active = true;
            self.total_spawned += 1;
        }
    }

    /// Spawns toxic volatile gas cells venting from severed infrastructure
    pub fn spawn_gas(&mut self, origin: Vec3, initial_vel: Vec3, count: usize) {
        for p in self.particles.iter_mut().filter(|p| !p.active).take(count) {
            let jitter = Vec3::new(
                rand::gen_range(-0.4f32, 0.4),
                rand::gen_range(-0.2f32, 0.2),
                rand::gen_range(-0.4f32, 0.4),
            );
            p.position = origin + jitter;
            p.velocity = initial_vel + jitter * 1.5;
            p.fluid_type = FluidType::ToxicGas;
            p.temperature = 25.0;
            p.life = rand::gen_range(8.0f32, 14.0);
            p.max_life = p.life;
            p.radius = rand::gen_range(0.40f32, 0.75);
            p.active = true;
            self.total_spawned += 1;
        }
    }

    /// Structural feedback shared by the live loop and gameplay qualification.
    pub fn update_structural_feedback(
        &mut self,
        dt: f32,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
        fluid_type: FluidType,
        cam_pos: Vec3,
    ) -> (f32, bool) {
        let mut trauma = 0.0;
        let severed_rebar = !grid.severed_rebar_positions.is_empty();
        if !grid.severed_rebar_positions.is_empty() {
            let mut col_positions = std::mem::take(&mut grid.severed_rebar_positions);
            for col_pos in &col_positions {
                physics.spawn_stress_fissure(*col_pos, Vec3::Y, 35);
                physics.spawn_column_micro_fracture(*col_pos);
                physics.spawn_column_buckling(*col_pos, Vec3::Y);
                let impulse = Vec3::new(
                    rand::gen_range(-1.2f32, 1.2),
                    rand::gen_range(0.3f32, 1.8),
                    rand::gen_range(-1.2f32, 1.2),
                );
                physics.spawn_bent_rebar(*col_pos, impulse, false);

                match fluid_type {
                    FluidType::ToxicGas => {
                        self.spawn_gas(*col_pos, Vec3::new(0.0, 1.8, 0.0), 6);
                    }
                    FluidType::MoltenSlag => {
                        self.spawn_slag(*col_pos, Vec3::new(0.0, 2.0, 0.0), 6);
                    }
                    _ => {
                        self.spawn_water(*col_pos, Vec3::new(0.0, 3.2, 0.0), 6);
                    }
                }
            }
            if let Some(closest_pos) = col_positions.iter().min_by(|a, b| {
                a.distance_squared(cam_pos).partial_cmp(&b.distance_squared(cam_pos)).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                audio.play_spatial(audio.stress_creak.as_ref(), *closest_pos, cam_pos, 0.60);
                audio.play_spatial(audio.structural_groan.as_ref(), *closest_pos, cam_pos, 0.50);
                let trauma_atten = SoundSystem::calculate_distance_attenuation(*closest_pos, cam_pos);
                let rebar_trauma = (0.35 * trauma_atten).clamp(0.0, 0.45);
                if rebar_trauma > 0.01 {
                    trauma += rebar_trauma;
                }
            }

            grid.severed_rebar_event = 0;
            col_positions.clear();
            grid.severed_rebar_positions = col_positions;
        }

        // Ambient compromised structural stress groans & dust trickles
        if grid.initial_rebar_count > 0 {
            let rebar_ratio = grid.current_rebar_count as f32 / grid.initial_rebar_count as f32;
            if rebar_ratio < 0.75 {
                self.ambient_stress_timer -= dt;
                if self.ambient_stress_timer <= 0.0 {
                    self.ambient_stress_timer = rand::gen_range(3.5, 6.0);
                    audio.play(audio.stress_creak.as_ref(), 0.50);
                    audio.play(audio.structural_groan.as_ref(), 0.45);
                    let stress_atten = SoundSystem::calculate_distance_attenuation(Vec3::new(0.0, 8.0, 0.0), cam_pos);
                    let stress_trauma = 0.08 * stress_atten;
                    if stress_trauma > 0.01 {
                        trauma += stress_trauma;
                    }
                    let crack_pos = Vec3::new(rand::gen_range(-6.0, 6.0), rand::gen_range(4.0, 16.0), rand::gen_range(-6.0, 6.0));
                    physics.spawn_stress_fissure(crack_pos, Vec3::Y, 12);
                    physics.spawn_column_micro_fracture(crack_pos);
                    physics.spawn_column_buckling(crack_pos, Vec3::Y);
                }
            }
        }
        (trauma, severed_rebar)
    }

    /// 30 Hz Cellular Automata simulation update
    pub fn update(
        &mut self,
        dt: f32,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
    ) {
        if dt <= 0.0 || !dt.is_finite() {
            return;
        }
        if !self.particles.iter().any(|p| p.active) {
            self.step_accumulator = 0.0;
            return;
        }
        // Bound catch-up work after a stall; a fluid effect must not freeze the next frame.
        self.step_accumulator += dt.min(0.25);
        let fixed_step = 1.0 / 30.0;

        while self.step_accumulator >= fixed_step {
            self.step_accumulator -= fixed_step;
            self.cellular_step(fixed_step, grid, physics, audio);
        }
    }

    fn cellular_step(
        &mut self,
        dt: f32,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
    ) {
        // Collect ignition sources for toxic gas detonation checks
        let mut ignition_points = Vec::with_capacity(physics.fire_jets.len() + 16);
        for jet in &physics.fire_jets {
            ignition_points.push(jet.position);
        }
        for spark in &physics.sparks {
            ignition_points.push(spark.position);
        }

        // Collect water locations for cooling slag and extinguishing
        let mut water_positions = Vec::new();
        for p in self.particles.iter().filter(|p| p.active && p.fluid_type == FluidType::Water) {
            water_positions.push(p.position);
        }

        let mut to_detonate_gas: Vec<Vec3> = Vec::new();
        let mut to_solidify_slag: Vec<Vec3> = Vec::new();

        for i in 0..self.particles.len() {
            if !self.particles[i].active {
                continue;
            }

            self.particles[i].life -= dt;
            if self.particles[i].life <= 0.0 {
                if self.particles[i].fluid_type == FluidType::MoltenSlag && self.particles[i].temperature <= 400.0 {
                    to_solidify_slag.push(self.particles[i].position);
                }
                self.particles[i].active = false;
                continue;
            }

            let p_type = self.particles[i].fluid_type;
            let mut pos = self.particles[i].position;
            let mut vel = self.particles[i].velocity;

            match p_type {
                FluidType::Water => {
                    // Gravity + drag
                    vel.y -= 14.5 * dt;
                    vel.x *= 1.0 - 1.2 * dt;
                    vel.z *= 1.0 - 1.2 * dt;

                    let next_pos = pos + vel * dt;
                    let v_coord = grid.world_to_voxel(next_pos);
                    let foundation_hit = crate::weapon::surrounding_foundation_hit(pos, next_pos);

                    // Check terrain collision
                    if grid.get(v_coord.x, v_coord.y, v_coord.z).is_solid() || foundation_hit.is_some() {
                        if let Some(hit) = foundation_hit {
                            pos = hit;
                        }
                        // Spread laterally (cellular fluid behavior: flow downhill / away from obstacles)
                        vel.y = 0.0;
                        let spread = Vec3::new(
                            rand::gen_range(-2.0f32, 2.0),
                            0.0,
                            rand::gen_range(-2.0f32, 2.0),
                        );
                        vel.x = vel.x * 0.4 + spread.x;
                        vel.z = vel.z * 0.4 + spread.z;

                        // Wash nearby debris particles
                        for debris in &mut physics.debris {
                            if debris.position.distance_squared(pos) < 2.5 {
                                debris.velocity += vel * 0.3;
                            }
                        }
                    } else {
                        pos = next_pos;
                    }

                    // Extinguish fire jets within splash radius
                    let splash_radius_sq = 2.25;
                    physics.fire_jets.retain(|jet| {
                        jet.position.distance_squared(pos) > splash_radius_sq
                    });
                }

                FluidType::MoltenSlag => {
                    // Viscous gravity
                    vel.y -= 9.8 * dt;
                    vel.x *= 1.0 - 3.5 * dt;
                    vel.z *= 1.0 - 3.5 * dt;

                    // Cool down over time
                    self.particles[i].temperature -= 120.0 * dt;

                    // Rapid quench if near any water particle
                    for &w_pos in &water_positions {
                        if w_pos.distance_squared(pos) < 1.8 {
                            self.particles[i].temperature = 0.0;
                            break;
                        }
                    }

                    let next_pos = pos + vel * dt;
                    let v_coord = grid.world_to_voxel(next_pos);

                    if grid.get(v_coord.x, v_coord.y, v_coord.z).is_solid() {
                        vel = Vec3::ZERO;

                        // Thermally weaken concrete columns underneath
                        grid.damage_voxel(v_coord.x, v_coord.y, v_coord.z, 25.0 * dt, DamageType::Thermal);
                    } else if let Some(hit) = crate::weapon::surrounding_foundation_hit(pos, next_pos) {
                        pos = hit;
                        vel = Vec3::ZERO;
                    } else {
                        pos = next_pos;
                    }

                    // If cooled below 350K, solidify into Basalt
                    if self.particles[i].temperature < 350.0 {
                        to_solidify_slag.push(pos);
                        self.particles[i].active = false;
                        continue;
                    }
                }

                FluidType::ToxicGas => {
                    // Buoyant slow upward rise + turbulence drift
                    vel.y = (vel.y + 1.2 * dt).clamp(-0.5, 1.8);
                    vel.x += rand::gen_range(-0.8f32, 0.8) * dt;
                    vel.z += rand::gen_range(-0.8f32, 0.8) * dt;
                    vel *= 1.0 - 0.5 * dt;

                    pos += vel * dt;

                    // Check for ignition triggers (fire jets, sparks, explosions)
                    let mut ignited = false;
                    for &ignite_pos in &ignition_points {
                        if ignite_pos.distance_squared(pos) < 3.5 {
                            ignited = true;
                            break;
                        }
                    }

                    if ignited {
                        to_detonate_gas.push(pos);
                        self.particles[i].active = false;
                        continue;
                    }
                }
            }

            self.particles[i].position = pos;
            self.particles[i].velocity = vel;
        }

        // Process solidifying molten slag -> Basalt voxels
        for p in to_solidify_slag {
            let v_pos = grid.world_to_voxel(p);
            // Place basalt in empty cell or replace damaged rubble
            if grid.in_bounds(v_pos.x, v_pos.y, v_pos.z) {
                let current_type = grid.get(v_pos.x, v_pos.y, v_pos.z);
                if current_type == VoxelType::Air || current_type == VoxelType::Glass {
                    grid.set(v_pos.x, v_pos.y, v_pos.z, VoxelType::Basalt);
                    self.basalt_conversions += 1;
                }
            }
        }

        // Process toxic gas secondary vapor explosions
        for center in to_detonate_gas {
            self.gas_detonations += 1;
            grid.carve_sphere_advanced(center, 3.2, 280.0, DamageType::Explosive);
            if physics.shockwaves.len() < PhysicsSystem::MAX_SHOCKWAVES {
                physics.shockwaves.push(crate::physics::ShockwaveRing {
                    position: center,
                    normal: Vec3::Y,
                    radius: 0.4,
                    max_radius: 6.4,
                    speed: 24.0,
                    color: Color::new(0.3, 1.0, 0.4, 0.9),
                    thickness: 0.3,
                });
            }
            audio.play_spatial(audio.explosion.as_ref(), center, center, 0.70);

            // Spawn smoke and fire burst
            for _ in 0..12 {
                if physics.debris.len() >= PhysicsSystem::MAX_DEBRIS {
                    break;
                }
                let v = Vec3::new(
                    rand::gen_range(-6.0f32, 6.0),
                    rand::gen_range(2.0f32, 9.0),
                    rand::gen_range(-6.0f32, 6.0),
                );
                physics.debris.push(DebrisParticle {
                    position: center,
                    velocity: v,
                    rotation: Vec3::ZERO,
                    angular_velocity: Vec3::ZERO,
                    color: Color::new(0.2, 0.8, 0.3, 0.8),
                    size: 0.35,
                    life: 1.8,
                    max_life: 1.8,
                    bounces: 0,
                });
            }
        }
    }

    /// Renders active fluid particles into a batched billboard mesh
    pub fn draw(&self, cam_pos: Vec3, mesh: &mut Mesh) {
        mesh.vertices.clear();
        mesh.indices.clear();

        for p in &self.particles {
            if !p.active {
                continue;
            }

            let view_dir = (cam_pos - p.position).try_normalize().unwrap_or(Vec3::Z);
            let axis = if view_dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
            let right = view_dir.cross(axis).normalize_or_zero();
            let up = right.cross(view_dir).normalize_or_zero();

            let r = p.radius;
            let mut col = p.fluid_type.base_color();

            if p.fluid_type == FluidType::MoltenSlag {
                let heat_ratio = (p.temperature / 1200.0).clamp(0.0, 1.0);
                col.r = 1.0;
                col.g = 0.2 + 0.6 * heat_ratio;
                col.b = 0.05 * heat_ratio;
            } else if p.fluid_type == FluidType::Water {
                let life_fade = (p.life / p.max_life).clamp(0.2, 1.0);
                col.a *= life_fade;
            }

            let c_raw: [u8; 4] = col.into();
            let base = mesh.vertices.len() as u16;

            let p0 = p.position - right * r - up * r;
            let p1 = p.position + right * r - up * r;
            let p2 = p.position + right * r + up * r;
            let p3 = p.position - right * r + up * r;

            mesh.vertices.push(Vertex { position: p0, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
            mesh.vertices.push(Vertex { position: p1, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
            mesh.vertices.push(Vertex { position: p2, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });
            mesh.vertices.push(Vertex { position: p3, uv: Vec2::ZERO, color: c_raw, normal: Vec4::ZERO });

            mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        if !mesh.indices.is_empty() {
            draw_mesh(mesh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluid_system_pool_allocation_and_capacity() {
        let mut sys = FluidSystem::new();
        assert_eq!(sys.particles.len(), MAX_FLUID_PARTICLES);
        assert_eq!(sys.active_count(), 0);

        sys.spawn_water(Vec3::new(0.0, 10.0, 0.0), Vec3::Y, 20);
        assert_eq!(sys.active_count(), 20);

        sys.clear();
        assert_eq!(sys.active_count(), 0);
    }

    #[test]
    fn test_water_extinguishes_fire_jets() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        physics.fire_jets.push(crate::physics::PressurizedFireJet {
            position: Vec3::new(5.0, 2.0, 5.0),
            direction: Vec3::Y,
            timer: 5.0,
            max_timer: 5.0,
        });
        assert_eq!(physics.fire_jets.len(), 1);

        // Spawn water directly onto the fire jet
        sys.spawn_water(Vec3::new(5.0, 2.0, 5.0), Vec3::ZERO, 5);
        sys.update(0.05, &mut grid, &mut physics, &audio);

        assert_eq!(physics.fire_jets.len(), 0, "Water splash must extinguish active fire jet");
    }

    #[test]
    fn test_molten_slag_solidifies_into_basalt() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let target_pos = Vec3::new(10.0, 0.55, 10.0);
        let v_coord = grid.world_to_voxel(target_pos);
        assert_eq!(grid.get(v_coord.x, v_coord.y, v_coord.z), VoxelType::Air);

        // Spawn molten slag and drop its temperature to trigger solidification
        sys.spawn_slag(target_pos, Vec3::ZERO, 1);
        for p in &mut sys.particles {
            if p.active {
                p.position = target_pos;
                p.velocity = Vec3::ZERO;
                p.temperature = 200.0; // Cool below threshold
                p.life = 2.0;
            }
        }

        sys.update(0.05, &mut grid, &mut physics, &audio);
        assert_eq!(grid.get(v_coord.x, v_coord.y, v_coord.z), VoxelType::Basalt);
        assert!(sys.basalt_conversions >= 1);
    }

    #[test]
    fn test_toxic_gas_explodes_near_ignition() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let gas_pos = Vec3::new(15.0, 3.0, 15.0);
        sys.spawn_gas(gas_pos, Vec3::ZERO, 1);

        // Place a spark right near the gas
        physics.sparks.push(crate::physics::SparkParticle {
            position: gas_pos + Vec3::new(0.2, 0.0, 0.0),
            velocity: Vec3::ZERO,
            life: 1.0,
            max_life: 1.0,
            color: WHITE,
        });

        sys.update(0.05, &mut grid, &mut physics, &audio);
        assert_eq!(sys.gas_detonations, 1, "Toxic gas must detonate upon contact with spark");
    }

    #[test]
    fn cooled_slag_solidifies_in_subterranean_empty_cells() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let pos = grid.voxel_to_world(IVec3::new(96, -4, 96));
        grid.set(96, -4, 96, VoxelType::Air);
        sys.particles[0] = FluidParticle {
            position: pos, temperature: 200.0, life: 1.0,
            fluid_type: FluidType::MoltenSlag, active: true,
            ..Default::default()
        };
        sys.update(0.05, &mut grid, &mut physics, &SoundSystem::empty());
        assert_eq!(grid.get(96, -4, 96), VoxelType::Basalt);
        assert_eq!(sys.basalt_conversions, 1);
    }

    #[test]
    fn out_of_bounds_slag_does_not_report_basalt_conversion() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        sys.particles[0] = FluidParticle {
            position: Vec3::new(1000.0, 1.0, 0.0), temperature: 200.0, life: 1.0,
            fluid_type: FluidType::MoltenSlag, active: true,
            ..Default::default()
        };
        sys.update(0.05, &mut grid, &mut physics, &SoundSystem::empty());
        assert_eq!(sys.basalt_conversions, 0);
    }

    #[test]
    fn water_flows_into_excavated_ground() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        for y in -3..=0 {
            grid.set(96, y, 96, VoxelType::Air);
        }
        sys.particles[0] = FluidParticle {
            position: Vec3::new(0.25, 0.1, 0.25), velocity: Vec3::new(0.0, -1.0, 0.0),
            life: 2.0, active: true, ..Default::default()
        };
        sys.update(0.05, &mut grid, &mut physics, &SoundSystem::empty());
        assert!(sys.particles[0].position.y < 0.1, "An excavated trench must not retain an invisible floor at y=0.5");
    }

    #[test]
    fn regression_fluid_lands_on_slab_corner_and_falls_beyond_extent() {
        for fluid_type in [FluidType::Water, FluidType::MoltenSlag] {
            for (x, should_land) in [(200.0, true), (241.0, false)] {
                let mut sys = FluidSystem::new();
                let mut grid = VoxelGrid::new();
                let mut physics = PhysicsSystem::new();
                sys.particles[0] = FluidParticle {
                    position: Vec3::new(x, 1.0, 200.0), velocity: Vec3::new(0.0, -20.0, 0.0),
                    life: 2.0, temperature: 1200.0, fluid_type, active: true,
                    ..Default::default()
                };
                sys.update(0.05, &mut grid, &mut physics, &SoundSystem::empty());
                if should_land {
                    assert_eq!(sys.particles[0].position.y, 0.5, "Fluid must land on the visible square slab corner");
                    assert_eq!(sys.particles[0].velocity.y, 0.0);
                } else {
                    assert!(sys.particles[0].position.y < 0.5, "Fluid beyond the slab must keep falling");
                }
            }
        }
    }

    #[test]
    fn spawning_respects_pool_capacity_and_clear_resets_level_statistics() {
        let mut sys = FluidSystem::new();
        sys.spawn_water(Vec3::ZERO, Vec3::ZERO, MAX_FLUID_PARTICLES + 1);
        assert_eq!(sys.active_count(), MAX_FLUID_PARTICLES);
        assert_eq!(sys.total_spawned, MAX_FLUID_PARTICLES);
        sys.spawn_gas(Vec3::ZERO, Vec3::ZERO, 10);
        assert_eq!(sys.total_spawned, MAX_FLUID_PARTICLES);
        sys.basalt_conversions = 3;
        sys.gas_detonations = 2;
        sys.clear();
        assert_eq!(sys.total_spawned, 0);
        assert_eq!(sys.basalt_conversions, 0);
        assert_eq!(sys.gas_detonations, 0);
        sys.spawn_slag(Vec3::ZERO, Vec3::ZERO, 1);
        assert_eq!(sys.active_count(), 1);
        assert_eq!(sys.total_spawned, 1);
    }

    #[test]
    fn gas_explosion_respects_visual_effect_capacity() {
        let mut sys = FluidSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let pos = Vec3::new(0.0, 10.0, 0.0);
        for _ in 0..PhysicsSystem::MAX_SHOCKWAVES {
            physics.shockwaves.push(crate::physics::ShockwaveRing {
                position: pos, normal: Vec3::Y, radius: 0.4, max_radius: 6.4,
                speed: 24.0, color: WHITE, thickness: 0.3,
            });
        }
        for _ in 0..PhysicsSystem::MAX_DEBRIS - 1 {
            physics.debris.push(DebrisParticle {
                position: pos, velocity: Vec3::ZERO, rotation: Vec3::ZERO,
                angular_velocity: Vec3::ZERO, color: WHITE, size: 0.35,
                life: 1.0, max_life: 1.0, bounces: 0,
            });
        }
        physics.sparks.push(crate::physics::SparkParticle {
            position: pos, velocity: Vec3::ZERO, life: 1.0, max_life: 1.0, color: WHITE,
        });
        sys.particles[0] = FluidParticle {
            position: pos, life: 1.0, fluid_type: FluidType::ToxicGas, active: true,
            ..Default::default()
        };
        sys.update(0.05, &mut grid, &mut physics, &SoundSystem::empty());
        assert_eq!(sys.gas_detonations, 1, "The detonation still affects gameplay when visual pools are full");
        assert_eq!(physics.shockwaves.len(), PhysicsSystem::MAX_SHOCKWAVES);
        assert_eq!(physics.debris.len(), PhysicsSystem::MAX_DEBRIS);
    }
}
