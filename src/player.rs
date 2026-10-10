use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, VOXEL_SIZE};

pub const PLAYER_RADIUS: f32 = 0.30;
pub const PLAYER_HEIGHT: f32 = 1.80;
pub const MAX_STEP_UP: f32 = 0.55;

pub struct Player {
    pub position: Vec3,
    pub velocity: Vec3,
    pub spawn_pos: Vec3,
    pub just_respawned: bool,
    pub yaw: f32,   // in radians
    pub pitch: f32, // in radians

    pub mouse_sensitivity: f32,
    pub move_speed: f32,
    pub sprint_speed: f32,
    pub jump_force: f32,

    pub is_grounded: bool,
    pub coyote_timer: f32,
    pub jump_buffer: f32,
    pub dash_cooldown: f32,

    pub eye_height: f32,
    pub bob_timer: f32,
    pub bob_amount: Vec3,

    pub trauma: f32,
    pub fov_kick: f32,
    pub is_mouse_locked: bool,
    pub invert_pitch: bool,

    pub just_landed: Option<f32>,
    pub just_jumped: bool,
    pub just_dashed: bool,
    pub dash_tilt: f32,
    pub grapple_hook: Option<Vec3>,
    pub is_grappling: bool,
    pub just_grappled: bool,
}

impl Player {
    pub fn new(spawn_pos: Vec3) -> Self {
        Self {
            position: spawn_pos,
            velocity: Vec3::ZERO,
            spawn_pos,
            just_respawned: false,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.08,
            mouse_sensitivity: 0.0022,
            move_speed: 7.0,
            sprint_speed: 12.5,
            jump_force: 8.5,
            is_grounded: true,
            coyote_timer: 0.0,
            jump_buffer: 0.0,
            dash_cooldown: 0.0,
            eye_height: 1.75,
            bob_timer: 0.0,
            bob_amount: Vec3::ZERO,
            trauma: 0.0,
            fov_kick: 0.0,
            is_mouse_locked: true,
            invert_pitch: false,
            just_landed: None,
            just_jumped: false,
            just_dashed: false,
            dash_tilt: 0.0,
            grapple_hook: None,
            is_grappling: false,
            just_grappled: false,
        }
    }

    pub fn respawn_at_spawn(&mut self) {
        self.position = self.spawn_pos;
        self.velocity = Vec3::ZERO;
        self.is_grounded = true;
        self.is_grappling = false;
        self.grapple_hook = None;
        self.trauma = 0.0;
        self.fov_kick = 0.0;
        self.just_landed = None;
        self.bob_amount = Vec3::ZERO;
        self.bob_timer = 0.0;
        self.dash_cooldown = 0.0;
        self.dash_tilt = 0.0;
        self.coyote_timer = 0.0;
        self.jump_buffer = 0.0;
        self.just_respawned = true;
    }

    pub fn add_trauma(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
    }

    pub fn forward_dir(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        ).normalize()
    }

    pub fn right_dir(&self) -> Vec3 {
        self.forward_dir().cross(Vec3::Y).normalize()
    }

    pub fn up_dir(&self) -> Vec3 {
        self.right_dir().cross(self.forward_dir()).normalize()
    }

    pub fn horizontal_forward(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize()
    }

    pub fn apply_look_input(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw -= delta_x * self.mouse_sensitivity * 1000.0;
        // In Macroquad, mouse_delta_position() computes (last_position - current_position).
        // When moving mouse UP on desk towards top edge, current_y < last_y, yielding delta_y > 0.
        // In standard FPS controls, moving mouse UP tilts pitch UP (increases pitch).
        // Therefore, pitch changes by +delta_y in standard mode, and -delta_y in inverted mode.
        let pitch_factor = if self.invert_pitch { -1.0 } else { 1.0 };
        self.pitch += delta_y * self.mouse_sensitivity * 1000.0 * pitch_factor;
        self.pitch = self.pitch.clamp(-1.52, 1.52); // ~87 degrees
    }

    pub fn update(&mut self, dt: f32, grid: &VoxelGrid, physics: Option<&crate::physics::PhysicsSystem>, allow_input: bool) {
        self.just_landed = None;
        self.just_jumped = false;
        self.just_dashed = false;
        self.just_grappled = false;
        self.just_respawned = false;

        // Safe Bottom Void Boundary Early Guard: if player enters frame in deep space (Y <= -24.0m)
        if self.position.y <= crate::voxel::VOID_BOUNDARY_Y {
            self.respawn_at_spawn();
            return;
        }

        // 1. Mouse Look & Invert Pitch Toggle (I key)
        if allow_input && is_key_pressed(crate::controls::KEY_INVERT_PITCH) {
            self.invert_pitch = !self.invert_pitch;
        }

        if self.is_mouse_locked && allow_input {
            let mouse_delta = mouse_delta_position();
            self.apply_look_input(mouse_delta.x, mouse_delta.y);
        }

        // 2. Dash cooldown & Timers
        if self.dash_cooldown > 0.0 {
            self.dash_cooldown = (self.dash_cooldown - dt).max(0.0);
        }
        self.dash_tilt *= (1.0 - 9.0 * dt).max(0.0);
        if self.jump_buffer > 0.0 {
            self.jump_buffer = (self.jump_buffer - dt).max(0.0);
        }

        // 3. Movement input
        let mut move_intent = Vec3::ZERO;
        let is_sprinting = allow_input && is_key_down(crate::controls::KEY_SPRINT);

        if allow_input {
            let h_fwd = self.horizontal_forward();
            let right = self.right_dir();

            if is_key_down(crate::controls::KEY_FORWARD) { move_intent += h_fwd; }
            if is_key_down(crate::controls::KEY_BACK) { move_intent -= h_fwd; }
            if is_key_down(crate::controls::KEY_RIGHT) { move_intent += right; }
            if is_key_down(crate::controls::KEY_LEFT) { move_intent -= right; }

            // Jump input with buffer
            if is_key_pressed(crate::controls::KEY_JUMP) {
                self.jump_buffer = 0.15;
            }

            // Kinetic Grapple Harpoon Line ('Q' Key):
            // Fires high-velocity magnetic wire pulling player toward target anchor point,
            // launching into parkour jump momentum when jumping mid-air.
            if is_key_pressed(crate::controls::KEY_GRAPPLE) {
                if self.is_grappling {
                    self.is_grappling = false;
                    self.grapple_hook = None;
                } else {
                    let eye = self.position + Vec3::new(0.0, self.eye_height, 0.0);
                    let fwd = self.forward_dir();
                    if let Some(hit) = grid.raycast(eye, fwd, crate::controls::GRAPPLE_RANGE) {
                        self.grapple_hook = Some(hit.world_pos);
                        self.is_grappling = true;
                        self.just_grappled = true;
                        self.fov_kick += 4.0;
                    } else if self.dash_cooldown <= 0.0 {
                        // Fallback to aerodynamic jet-dash if aiming at open sky
                        let dash_dir = if move_intent.length_squared() > 0.01 {
                            move_intent.normalize()
                        } else {
                            h_fwd
                        };
                        let strafe_dot = dash_dir.dot(right);
                        self.dash_tilt = -strafe_dot * 0.10;
                        self.velocity += dash_dir * 18.0 + Vec3::new(0.0, 3.5, 0.0);
                        self.dash_cooldown = 0.8;
                        self.fov_kick += 8.0;
                        self.add_trauma(0.15);
                        self.just_dashed = true;
                    }
                }
            }
        }

        // Active Kinetic Grapple Winch Physics: pull player toward target anchor
        if let Some(anchor) = self.grapple_hook {
            let eye = self.position + Vec3::new(0.0, self.eye_height * 0.7, 0.0);
            let to_anchor = anchor - eye;
            let dist = to_anchor.length();
            if !(2.0..=72.0).contains(&dist) {
                self.is_grappling = false;
                self.grapple_hook = None;
            } else {
                let pull_dir = to_anchor / dist;
                self.velocity += pull_dir * (35.0 * dt) + Vec3::new(0.0, 4.0 * dt, 0.0);
                self.is_grounded = false;

                // Slingshot / Parkour Jump Boost when Space pressed mid-grapple!
                if allow_input && is_key_pressed(crate::controls::KEY_JUMP) {
                    self.velocity += pull_dir * 16.0 + Vec3::new(0.0, 8.5, 0.0);
                    self.is_grappling = false;
                    self.grapple_hook = None;
                    self.fov_kick += 10.0;
                    self.add_trauma(0.18);
                    self.just_jumped = true;
                }
            }
        }

        // 4. Ground Check & Coyote Time (Subterranean Crater Support down to -24.0m)
        let foot_pos = self.position;
        let mut ground_y = f32::NEG_INFINITY;
        let vpos = grid.world_to_voxel(foot_pos);

        if vpos.x >= 0 && (vpos.x as usize) < crate::voxel::GRID_X && vpos.z >= 0 && (vpos.z as usize) < crate::voxel::GRID_Z {
            let top_test_y = vpos.y.min(crate::voxel::MAX_VOXEL_Y - 1);
            if top_test_y >= crate::voxel::MIN_VOXEL_Y {
                for test_y in (crate::voxel::MIN_VOXEL_Y..=top_test_y).rev() {
                    if grid.get(vpos.x, test_y, vpos.z).is_solid() {
                        let v_world = grid.voxel_to_world(IVec3::new(vpos.x, test_y, vpos.z));
                        ground_y = v_world.y + VOXEL_SIZE * 0.5;
                        break;
                    }
                }
            }
        } else if foot_pos.x.abs() <= 240.0 && foot_pos.z.abs() <= 240.0 && self.position.y >= 0.0 {
            // Outside voxel grid perimeter, the surrounding industrial bedrock foundation slab sits at y = 0.50
            ground_y = 0.50;
        }

        // Check if standing on settled subterranean rubble
        if let Some(phys) = physics {
            if let Some(rubble_h) = phys.settled_height_at(foot_pos) {
                if rubble_h > ground_y && rubble_h <= self.position.y + 0.6 {
                    ground_y = rubble_h;
                }
            }
        }

        let is_on_ground = ground_y.is_finite()
            && (self.position.y - ground_y).abs() <= 0.08
            && self.velocity.y <= 0.05;

        if is_on_ground {
            if !self.is_grounded && self.velocity.y < -3.5 {
                let fall_speed = self.velocity.y.abs();
                let land_trauma = (fall_speed / 24.0).clamp(0.08, 0.50);
                self.add_trauma(land_trauma);
                // Downward camera dip from impact
                self.bob_amount += Vec3::new(0.0, -(fall_speed * 0.012).min(0.22), 0.0);
                self.just_landed = Some(fall_speed);
            }
            self.position.y = ground_y;
            self.velocity.y = 0.0;
            self.is_grounded = true;
            self.coyote_timer = 0.15;
        } else {
            self.is_grounded = false;
            self.coyote_timer = (self.coyote_timer - dt).max(0.0);
            self.velocity.y -= 22.0 * dt; // Gravity
        }

        // Jump execution (Coyote time + Buffered jump)
        if self.jump_buffer > 0.0 && (self.is_grounded || self.coyote_timer > 0.0) {
            self.velocity.y = self.jump_force;
            self.is_grounded = false;
            self.coyote_timer = 0.0;
            self.jump_buffer = 0.0;
            self.just_jumped = true;
        }

        // 5. Horizontal acceleration & friction
        let target_speed = if is_sprinting { self.sprint_speed } else { self.move_speed };
        let mut target_vel = Vec3::ZERO;
        let is_moving = move_intent.length_squared() > 0.01;
        if is_moving {
            target_vel = move_intent.normalize() * target_speed;
        }

        let accel = if self.is_grounded {
            if is_moving { 14.0 } else { 18.0 }
        } else {
            8.5
        };
        self.velocity.x += (target_vel.x - self.velocity.x) * (accel * dt).min(1.0);
        self.velocity.z += (target_vel.z - self.velocity.z) * (accel * dt).min(1.0);

        // Helper: Check highest solid obstacle in vertical window [min_y, max_y]
        let obstacle_top_at = |gx: f32, gz: f32, min_y: f32, max_y: f32| -> Option<f32> {
            let v_low = grid.world_to_voxel(Vec3::new(gx, min_y, gz));
            let v_high = grid.world_to_voxel(Vec3::new(gx, max_y, gz));
            if v_low.x < 0 || (v_low.x as usize) >= crate::voxel::GRID_X || v_low.z < 0 || (v_low.z as usize) >= crate::voxel::GRID_Z {
                return None;
            }
            let start_y = v_low.y.clamp(crate::voxel::MIN_VOXEL_Y, crate::voxel::MAX_VOXEL_Y - 1);
            let end_y = v_high.y.clamp(crate::voxel::MIN_VOXEL_Y, crate::voxel::MAX_VOXEL_Y - 1);
            let mut top_found: Option<f32> = None;
            for vy in start_y..=end_y {
                if grid.get(v_low.x, vy, v_low.z).is_solid() {
                    let vw = grid.voxel_to_world(IVec3::new(v_low.x, vy, v_low.z));
                    let yt = vw.y + VOXEL_SIZE * 0.5;
                    top_found = Some(top_found.map_or(yt, |h: f32| h.max(yt)));
                }
            }
            top_found
        };

        // ---------------------------------------------------------------------
        // Vertical Position Integration
        // ---------------------------------------------------------------------
        let old_head_y = self.position.y + PLAYER_HEIGHT;
        if !self.is_grounded {
            self.position.y += self.velocity.y * dt;
        }

        // ---------------------------------------------------------------------
        // Overhead Ceiling Collision Check (when moving upward)
        // ---------------------------------------------------------------------
        if self.velocity.y > 0.0 {
            let head_travel = self.position.y + PLAYER_HEIGHT - old_head_y;
            let check_offsets = [
                Vec2::ZERO,
                Vec2::new(PLAYER_RADIUS * 0.7, 0.0),
                Vec2::new(-PLAYER_RADIUS * 0.7, 0.0),
                Vec2::new(0.0, PLAYER_RADIUS * 0.7),
                Vec2::new(0.0, -PLAYER_RADIUS * 0.7),
            ];
            let mut ceiling_y = f32::INFINITY;
            for off in check_offsets {
                let head_start = Vec3::new(self.position.x + off.x, old_head_y, self.position.z + off.y);
                if let Some(hit) = grid.raycast(head_start, Vec3::Y, head_travel) {
                    ceiling_y = ceiling_y.min(hit.world_pos.y);
                }
            }
            if ceiling_y.is_finite() {
                self.position.y = ceiling_y - PLAYER_HEIGHT - 0.001;
                self.velocity.y = 0.0;
            }
        }

        // ---------------------------------------------------------------------
        // Clean Ground Landing Transition
        // ---------------------------------------------------------------------
        if !self.is_grounded && ground_y.is_finite() && self.position.y <= ground_y {
            if self.velocity.y < -3.5 {
                let fall_speed = self.velocity.y.abs();
                let land_trauma = (fall_speed / 24.0).clamp(0.08, 0.50);
                self.add_trauma(land_trauma);
                // Downward camera dip from impact
                self.bob_amount += Vec3::new(0.0, -(fall_speed * 0.012).min(0.22), 0.0);
                self.just_landed = Some(fall_speed);
            }
            self.position.y = ground_y;
            self.velocity.y = 0.0;
            self.is_grounded = true;
            self.coyote_timer = 0.15;
        }

        // Axis-Separated Horizontal Movement: Step-Up & Wall Sliding
        // --- Move along X ---
        let dx = self.velocity.x * dt;
        if dx.abs() > 1e-5 {
            let steps = (dx.abs() / (VOXEL_SIZE * 0.5)).ceil() as usize;
            let step_dx = dx / steps as f32;
            for _ in 0..steps {
                let cand_x = self.position.x + step_dx;
                let edge_x = cand_x + dx.signum() * PLAYER_RADIUS;
                let z_offsets = [-PLAYER_RADIUS * 0.7, 0.0, PLAYER_RADIUS * 0.7];
                let mut blocked_x = false;
                let mut step_y_needed = self.position.y;

                for &zo in &z_offsets {
                    let test_z = self.position.z + zo;
                    // Wall check at body height: cannot step over obstacles > MAX_STEP_UP
                    let body_min = self.position.y + MAX_STEP_UP + 0.05;
                    let body_max = self.position.y + PLAYER_HEIGHT;
                    if obstacle_top_at(edge_x, test_z, body_min, body_max).is_some() {
                        blocked_x = true;
                        break;
                    }
                    // Step check: obstacles <= MAX_STEP_UP
                    if let Some(step_top) = obstacle_top_at(edge_x, test_z, self.position.y + 0.05, self.position.y + MAX_STEP_UP) {
                        if step_top <= self.position.y + MAX_STEP_UP + 0.01 {
                            // Headroom check above the step
                            if obstacle_top_at(edge_x, test_z, step_top + 0.05, step_top + PLAYER_HEIGHT).is_some() {
                                blocked_x = true;
                                break;
                            }
                            step_y_needed = step_y_needed.max(step_top);
                        } else {
                            blocked_x = true;
                            break;
                        }
                    }
                }

                if blocked_x {
                    self.velocity.x = 0.0; // Wall slide: halt X, allow Z
                    break;
                } else {
                    if step_y_needed > self.position.y {
                        self.position.y = step_y_needed; // Step up onto ledge
                    }
                    self.position.x = cand_x;
                }
            }
        }

        // --- Move along Z ---
        let dz = self.velocity.z * dt;
        if dz.abs() > 1e-5 {
            let steps = (dz.abs() / (VOXEL_SIZE * 0.5)).ceil() as usize;
            let step_dz = dz / steps as f32;
            for _ in 0..steps {
                let cand_z = self.position.z + step_dz;
                let edge_z = cand_z + dz.signum() * PLAYER_RADIUS;
                let x_offsets = [-PLAYER_RADIUS * 0.7, 0.0, PLAYER_RADIUS * 0.7];
                let mut blocked_z = false;
                let mut step_y_needed = self.position.y;

                for &xo in &x_offsets {
                    let test_x = self.position.x + xo;
                    let body_min = self.position.y + MAX_STEP_UP + 0.05;
                    let body_max = self.position.y + PLAYER_HEIGHT;
                    if obstacle_top_at(test_x, edge_z, body_min, body_max).is_some() {
                        blocked_z = true;
                        break;
                    }
                    if let Some(step_top) = obstacle_top_at(test_x, edge_z, self.position.y + 0.05, self.position.y + MAX_STEP_UP) {
                        if step_top <= self.position.y + MAX_STEP_UP + 0.01 {
                            if obstacle_top_at(test_x, edge_z, step_top + 0.05, step_top + PLAYER_HEIGHT).is_some() {
                                blocked_z = true;
                                break;
                            }
                            step_y_needed = step_y_needed.max(step_top);
                        } else {
                            blocked_z = true;
                            break;
                        }
                    }
                }

                if blocked_z {
                    self.velocity.z = 0.0; // Wall slide: halt Z, allow X
                    break;
                } else {
                    if step_y_needed > self.position.y {
                        self.position.y = step_y_needed; // Step up onto ledge
                    }
                    self.position.z = cand_z;
                }
            }
        }

        // Safe Bottom Void Boundary Reset: if player falls into deep space (Y <= -24.0m)
        if self.position.y <= crate::voxel::VOID_BOUNDARY_Y {
            self.respawn_at_spawn();
            return;
        }

        // Keep player above ground floor (if supported and grounded)
        if self.is_grounded && ground_y.is_finite() && self.position.y < ground_y {
            self.position.y = ground_y;
            self.velocity.y = 0.0;
        }

        // 6. View Bobbing
        let horizontal_speed = Vec2::new(self.velocity.x, self.velocity.z).length();
        if self.is_grounded && horizontal_speed > 0.5 {
            let bob_freq = if is_sprinting { 14.0 } else { 9.0 };
            self.bob_timer += dt * bob_freq;
            let bob_y = (self.bob_timer).sin() * (if is_sprinting { 0.06 } else { 0.035 });
            let bob_x = (self.bob_timer * 0.5).cos() * (if is_sprinting { 0.04 } else { 0.02 });
            self.bob_amount = self.right_dir() * bob_x + Vec3::Y * bob_y;
        } else {
            self.bob_amount *= (1.0 - 8.0 * dt).max(0.0);
        }

        // 7. Trauma Decay & Camera Shake (Fast, snappy settling)
        self.trauma = (self.trauma - 2.5 * dt).max(0.0);
        self.fov_kick = (self.fov_kick - 32.0 * dt).max(0.0);
    }

    pub fn apply_weapon_recoil(&mut self, recoil_vel: Vec3) {
        // Weapon recoil on player must be strictly horizontal to eliminate erratic vertical bouncing
        let mut h_recoil = Vec3::new(recoil_vel.x, 0.0, recoil_vel.z);
        if self.is_grounded {
            h_recoil *= 0.85;
            self.velocity.y = self.velocity.y.min(0.0);
        }
        self.velocity += h_recoil;
    }

    pub fn camera_position(&self) -> Vec3 {
        let time = if self.trauma > 0.001 { get_time() as f32 } else { 0.0 };
        self.camera_position_at(time)
    }

    pub fn camera_position_at(&self, time: f32) -> Vec3 {
        let base = self.position + Vec3::new(0.0, self.eye_height, 0.0) + self.bob_amount;

        // Controlled, visceral micro-rumble instead of wild bouncy offsets
        if self.trauma > 0.001 {
            let shake = self.trauma * self.trauma;
            let t = time;
            let offset_x = (t * 26.0).sin() * 0.024 * shake;
            let offset_y = (t * 31.0).cos() * 0.020 * shake;
            let offset_z = (t * 21.0).sin() * 0.016 * shake;
            base + Vec3::new(offset_x, offset_y, offset_z)
        } else {
            base
        }
    }

    pub fn camera_rotation_angles(&self) -> (f32, f32, f32) {
        let time = if self.trauma > 0.001 { get_time() as f32 } else { 0.0 };
        self.camera_rotation_angles_at(time)
    }

    pub fn camera_rotation_angles_at(&self, time: f32) -> (f32, f32, f32) {
        let mut pitch = self.pitch;
        let mut yaw = self.yaw;
        let mut roll = self.dash_tilt;

        if self.trauma > 0.001 {
            let shake = self.trauma * self.trauma;
            let t = time;
            pitch += (t * 23.0).sin() * 0.010 * shake;
            yaw += (t * 19.0).cos() * 0.008 * shake;
            roll += (t * 27.0).sin() * 0.012 * shake;
        }

        (pitch, yaw, roll)
    }

    pub fn fov(&self) -> f32 {
        let base_fov = 72.0f32;
        let sprint_boost = if is_key_down(crate::controls::KEY_SPRINT) && self.is_grounded {
            8.0
        } else {
            0.0
        };
        (base_fov + sprint_boost + self.fov_kick).to_radians()
    }

    pub fn horizontal_speed(&self) -> f32 {
        Vec2::new(self.velocity.x, self.velocity.z).length()
    }

    pub fn dash_charge_ratio(&self) -> f32 {
        (1.0 - (self.dash_cooldown / 0.8)).clamp(0.0, 1.0)
    }

    pub fn compass_heading_degrees(&self) -> f32 {
        (-self.yaw.to_degrees() + 90.0).rem_euclid(360.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_outer_ground_support_matches_square_foundation() {
        let grid = VoxelGrid::new();
        for (x, z) in [(200.0, 200.0), (-200.0, 200.0), (240.0, -240.0)] {
            let mut player = Player::new(Vec3::new(x, 0.5, z));
            player.update(0.016, &grid, None, false);
            assert!(player.is_grounded, "Visible foundation must support the player at ({x}, {z})");
            assert_eq!(player.position.y, 0.5);
        }
        for (x, z) in [(240.1, 0.0), (-240.1, 0.0), (0.0, 240.1), (0.0, -240.1)] {
            let mut player = Player::new(Vec3::new(x, 0.5, z));
            player.update(0.016, &grid, None, false);
            assert!(!player.is_grounded, "No support beyond the foundation at ({x}, {z})");
            assert!(player.position.y < 0.5);
        }
    }

    #[test]
    fn regression_dash_cannot_tunnel_through_a_single_voxel_wall() {
        let mut grid = VoxelGrid::new();
        for y in 1..=8 {
            grid.set(97, y, 96, crate::voxel::VoxelType::Concrete);
        }
        let mut p = Player::new(Vec3::new(0.0, 1.5, 0.25));
        p.velocity.x = 40.0;
        p.is_grounded = false;
        p.update(0.05, &grid, None, false);

        assert!(p.position.x <= 0.20, "Swept body must stop before the wall: {:?}", p.position);
        assert_eq!(p.velocity.x, 0.0);
    }

    #[test]
    fn regression_jump_stops_below_ceiling_without_overlap() {
        let mut grid = VoxelGrid::new();
        grid.set(96, 5, 96, crate::voxel::VoxelType::Concrete);
        let mut p = Player::new(Vec3::new(0.25, 0.5, 0.25));
        p.velocity.y = 8.5;
        p.is_grounded = false;
        p.update(0.08, &grid, None, false);

        assert!(p.position.y + PLAYER_HEIGHT <= 2.501, "Head must stay below ceiling: {:?}", p.position);
        assert_eq!(p.velocity.y, 0.0);
    }

    #[test]
    fn test_player_initial_orientation() {
        let p = Player::new(Vec3::new(0.0, 3.5, 26.0));
        let fwd = p.forward_dir();
        // At yaw = -PI/2, forward_dir points along -Z towards the structure at Z=0
        assert!(fwd.z < -0.9, "Expected forward to face -Z, got {:?}", fwd);
        assert!(fwd.y > 0.0, "Expected slight upward pitch to view structure, got {:?}", fwd);
    }

    #[test]
    fn test_mouse_look_yaw_direction() {
        let mut p = Player::new(Vec3::new(0.0, 3.5, 26.0));
        // In Macroquad, moving mouse RIGHT gives a negative local delta (mouse_delta_position().x < 0).
        // With yaw -= delta_x * sens * 1000.0, yaw increases, rotating facing direction clockwise (to the right: +X).
        let mouse_delta_right = -0.05f32;
        p.yaw -= mouse_delta_right * p.mouse_sensitivity * 1000.0;
        let fwd = p.forward_dir();
        assert!(fwd.x > 0.0, "Camera must turn right (+X direction) when moving mouse right");
    }

    #[test]
    fn test_mouse_look_pitch_direction() {
        let mut p = Player::new(Vec3::new(0.0, 3.5, 26.0));
        let initial_pitch = p.pitch;

        // In Macroquad (last_pos - curr_pos), moving mouse physically UP on desk yields delta_y > 0.
        // Standard FPS controls must tilt camera UP (pitch increases).
        let mouse_delta_up = 0.05f32;
        p.apply_look_input(0.0, mouse_delta_up);
        assert!(p.pitch > initial_pitch, "Moving mouse up (delta_y > 0) must tilt pitch upward (standard look)");

        // Moving mouse physically DOWN on desk yields delta_y < 0.
        // Standard FPS controls must tilt camera DOWN (pitch decreases).
        let mouse_delta_down = -0.05f32;
        p.apply_look_input(0.0, mouse_delta_down);
        assert!((p.pitch - initial_pitch).abs() < 0.001, "Moving mouse down must tilt pitch downward");

        // Now test toggled inverted pitch (I key)
        p.invert_pitch = true;
        p.apply_look_input(0.0, mouse_delta_up);
        assert!(p.pitch < initial_pitch, "Inverted pitch mode must tilt pitch downward when moving mouse up");
    }

    #[test]
    fn test_player_landing_shock() {
        let mut p = Player::new(Vec3::new(0.0, 5.0, 26.0));
        let grid = VoxelGrid::new();
        p.is_grounded = false;
        p.velocity.y = -12.0; // High speed fall
        p.position.y = 0.02; // Touching ground

        p.update(0.016, &grid, None, false);
        assert!(p.is_grounded);
        assert!(p.just_landed.is_some(), "Landing from fall must set just_landed flag");
        assert!(p.trauma > 0.1, "Landing must produce screen shake trauma");
        assert!(p.bob_amount.y < 0.0, "Landing must produce downward camera dip");
    }

    #[test]
    fn test_player_telemetry_and_dash_tilt() {
        let mut p = Player::new(Vec3::new(0.0, 3.5, 26.0));
        assert_eq!(p.horizontal_speed(), 0.0);
        assert_eq!(p.dash_charge_ratio(), 1.0);

        // Heading: at initial yaw = -PI/2 (-90 deg), compass heading (-(-90) + 90) = 180 (facing South)
        let heading = p.compass_heading_degrees();
        assert!((heading - 180.0).abs() < 0.1, "Initial yaw must face 180 degrees");

        // Dash cooldown ratio
        p.dash_cooldown = 0.4;
        assert!((p.dash_charge_ratio() - 0.5).abs() < 0.01);

        // Dash tilt roll
        p.dash_tilt = 0.08;
        let (_pitch, _yaw, roll) = p.camera_rotation_angles();
        assert!((roll - 0.08).abs() < 0.001, "Camera roll must include dash_tilt");
    }

    #[test]
    fn test_apply_weapon_recoil_is_horizontal_and_grounded() {
        let mut p = Player::new(Vec3::new(0.0, 0.0, 0.0));
        p.is_grounded = true;
        // Weapon fired downwards creates upward recoil vector (0.0, 5.0, 3.0)
        p.apply_weapon_recoil(Vec3::new(2.0, 5.0, -3.0));

        assert_eq!(p.velocity.y, 0.0, "Weapon recoil must never give upward velocity to grounded player");
        assert!(p.velocity.x > 0.0);
        assert!(p.velocity.z < 0.0);
    }

    #[test]
    fn test_kinetic_grapple_wire_winch_physics() {
        let mut p = Player::new(Vec3::new(0.0, 2.0, 0.0));
        let grid = VoxelGrid::new();

        // 1. Attach grapple at valid distance
        let anchor = Vec3::new(0.0, 20.0, 20.0);
        p.grapple_hook = Some(anchor);
        p.is_grappling = true;
        p.is_grounded = true;

        p.update(0.1, &grid, None, false);

        assert!(p.grapple_hook.is_some(), "Grapple should remain attached at valid distance");
        assert!(p.is_grappling);
        assert!(!p.is_grounded, "Active grapple winch pull should unground player");
        assert!(p.velocity.y > 0.0, "Winch pull should add upward velocity toward anchor");
        assert!(p.velocity.z > 0.0, "Winch pull should add forward velocity toward anchor");

        // 2. Minimum distance detachment: when player gets too close (< 2.0m)
        let close_anchor = p.position + Vec3::new(0.0, p.eye_height * 0.7 + 0.5, 0.0); // dist = 0.5 < 2.0
        p.grapple_hook = Some(close_anchor);
        p.is_grappling = true;
        p.update(0.016, &grid, None, false);
        assert!(p.grapple_hook.is_none(), "Grapple must automatically detach when dist < 2.0m");
        assert!(!p.is_grappling);

        // 3. Maximum distance detachment: when anchor is beyond 72.0m
        let far_anchor = p.position + Vec3::new(0.0, 80.0, 0.0); // dist = 80 - eye_offset > 72.0
        p.grapple_hook = Some(far_anchor);
        p.is_grappling = true;
        p.update(0.016, &grid, None, false);
        assert!(p.grapple_hook.is_none(), "Grapple must automatically detach when dist > 72.0m");
        assert!(!p.is_grappling);
    }

    #[test]
    fn test_player_subterranean_crater_ground_detection() {
        let mut p = Player::new(Vec3::new(0.0, -10.0, 0.0));
        let mut grid = VoxelGrid::new();
        // Excavate crater cavity above y = -24 so y = -24 is the subterranean crater floor
        for y in -23..=1 {
            grid.set(48, y, 48, crate::voxel::VoxelType::Air);
        }
        grid.set(48, -24, 48, crate::voxel::VoxelType::Basalt);
        let v_world = grid.voxel_to_world(IVec3::new(48, -24, 48));
        let expected_ground_y = v_world.y + VOXEL_SIZE * 0.5;

        p.position = Vec3::new(v_world.x, expected_ground_y + 0.02, v_world.z);
        p.velocity.y = 0.0;
        p.update(0.016, &grid, None, false);

        assert!(p.is_grounded, "Player should be grounded inside subterranean crater");
        assert!((p.position.y - expected_ground_y).abs() < 0.01);
    }

    #[test]
    fn test_player_step_up_onto_crater_ledge() {
        let mut grid = VoxelGrid::new();
        let ground_vpos = grid.world_to_voxel(Vec3::new(0.0, 0.50, 0.0));
        grid.set(ground_vpos.x, ground_vpos.y, ground_vpos.z, crate::voxel::VoxelType::Concrete);
        grid.set(ground_vpos.x + 1, ground_vpos.y, ground_vpos.z, crate::voxel::VoxelType::Concrete);
        grid.set(ground_vpos.x + 1, ground_vpos.y + 1, ground_vpos.z, crate::voxel::VoxelType::Concrete); // 0.50m step

        let mut p = Player::new(Vec3::new(0.0, 0.50, 0.0));
        p.velocity = Vec3::new(5.0, 0.0, 0.0);
        p.update(0.05, &grid, None, false);

        assert!(p.position.x > 0.0, "Player should advance forward onto step");
        assert!(p.position.y >= 0.99, "Player should step up onto 0.50m ledge");
    }

    #[test]
    fn test_player_wall_sliding_prevents_clipping() {
        let mut grid = VoxelGrid::new();
        let ground_vpos = grid.world_to_voxel(Vec3::new(0.0, 0.50, 0.0));
        grid.set(ground_vpos.x, ground_vpos.y, ground_vpos.z, crate::voxel::VoxelType::Concrete);
        grid.set(ground_vpos.x + 1, ground_vpos.y, ground_vpos.z, crate::voxel::VoxelType::Concrete);
        grid.set(ground_vpos.x + 1, ground_vpos.y + 1, ground_vpos.z, crate::voxel::VoxelType::Concrete);
        grid.set(ground_vpos.x + 1, ground_vpos.y + 2, ground_vpos.z, crate::voxel::VoxelType::Concrete); // 1.0m wall (>0.55m)

        // Player starts near the wall at x = 0.19 (wall starts at 0.50, radius 0.30 -> edge at 0.49..0.51)
        let mut p = Player::new(Vec3::new(0.19, 0.50, 0.0));
        p.velocity = Vec3::new(5.0, 0.0, 5.0); // Diagonal motion into wall
        p.update(0.05, &grid, None, false);

        assert!(p.position.x < 0.25, "Player must not penetrate into wall");
        assert_eq!(p.velocity.x, 0.0, "Perpendicular velocity must be zeroed for sliding");
        assert!(p.position.z > 0.01, "Parallel velocity must slide along wall");
    }

    #[test]
    fn test_player_void_boundary_safe_reset() {
        let spawn = Vec3::new(0.0, 0.5, 20.5);
        let mut p = Player::new(spawn);
        let grid = VoxelGrid::new();

        p.position = Vec3::new(0.0, -24.5, 0.0); // Below VOID_BOUNDARY_Y (-24.0m)
        p.velocity = Vec3::new(10.0, -35.0, -5.0);
        p.trauma = 0.8;
        p.is_grappling = true;

        p.update(0.016, &grid, None, false);

        assert_eq!(p.position, spawn, "Player must respawn at spawn position");
        assert_eq!(p.velocity, Vec3::ZERO, "Velocity must be zeroed upon reset");
        assert!(p.is_grounded);
        assert!(!p.is_grappling);
        assert_eq!(p.trauma, 0.0);
    }
}
