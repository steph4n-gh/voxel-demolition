use macroquad::prelude::*;
use crate::player::Player;
use crate::physics::PhysicsSystem;
use crate::weapon::WeaponSystem;
use crate::siege::SiegeSystem;

pub const REPLAY_BUFFER_SECONDS: f32 = 60.0;
pub const REPLAY_SAMPLE_RATE_HZ: f32 = 20.0; // 20 frames/sec = 1,200 frames total for 60s
pub const MAX_REPLAY_FRAMES: usize = (REPLAY_BUFFER_SECONDS * REPLAY_SAMPLE_RATE_HZ) as usize;

/// Recorded state of an active dynamic chunk in the simulation
#[derive(Debug, Clone, Copy)]
pub struct ReplayChunkSnapshot {
    pub position: Vec3,
    pub rotation: Vec3,
    pub voxel_count: u16,
    pub local_bounds: (Vec3, Vec3),
}

/// Recorded state of an in-flight projectile
#[derive(Debug, Clone, Copy)]
pub struct ReplayProjectileSnapshot {
    pub position: Vec3,
    pub velocity: Vec3,
    pub is_artillery: bool,
}

/// Keyframe snapshot in the 60-second circular ring buffer
#[derive(Debug, Clone)]
pub struct ReplayFrame {
    pub timestamp: f32,
    pub player_pos: Vec3,
    pub player_rot: Vec3,
    pub player_cam_pos: Vec3,
    pub chunks: Vec<ReplayChunkSnapshot>,
    pub projectiles: Vec<ReplayProjectileSnapshot>,
}

/// Cinematic Replay & Flight Cam Controller
pub struct ReplaySystem {
    pub ring_buffer: Vec<Option<ReplayFrame>>,
    pub head_idx: usize,
    pub total_recorded_frames: usize,
    pub sample_timer: f32,
    pub recording_clock: f32,

    // Playback state
    pub is_active: bool,
    pub is_paused: bool,
    pub playback_time: f32,
    pub playback_speed: f32, // e.g. 0.1, 0.5, 1.0, 2.0, or negative for reverse
    pub orbit_cam_pos: Vec3,
    pub orbit_target: Vec3,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32,

    // Artillery Projectile Tracking Flight Cam
    pub flight_cam_active: bool,
    pub tracked_projectile_pos: Option<Vec3>,
    pub tracked_projectile_dir: Option<Vec3>,
}

impl ReplaySystem {
    pub fn new() -> Self {
        let mut ring_buffer = Vec::with_capacity(MAX_REPLAY_FRAMES);
        for _ in 0..MAX_REPLAY_FRAMES {
            ring_buffer.push(None);
        }

        Self {
            ring_buffer,
            head_idx: 0,
            total_recorded_frames: 0,
            sample_timer: 0.0,
            recording_clock: 0.0,
            is_active: false,
            is_paused: false,
            playback_time: 0.0,
            playback_speed: 1.0,
            orbit_cam_pos: Vec3::new(0.0, 15.0, 35.0),
            orbit_target: Vec3::new(0.0, 10.0, 0.0),
            orbit_yaw: 0.0,
            orbit_pitch: 0.25,
            orbit_distance: 35.0,
            flight_cam_active: false,
            tracked_projectile_pos: None,
            tracked_projectile_dir: None,
        }
    }

    pub fn clear(&mut self) {
        self.ring_buffer.fill(None);
        self.head_idx = 0;
        self.total_recorded_frames = 0;
        self.sample_timer = 0.0;
        self.recording_clock = 0.0;
        self.is_active = false;
        self.is_paused = false;
        self.playback_time = 0.0;
        self.playback_speed = 1.0;
        self.flight_cam_active = false;
        self.tracked_projectile_pos = None;
        self.tracked_projectile_dir = None;
    }

    /// Records simulation snapshot into the circular ring buffer at 20 Hz
    pub fn record_step(
        &mut self,
        dt: f32,
        player: &Player,
        physics: &PhysicsSystem,
        weapon: &WeaponSystem,
        siege: &SiegeSystem,
    ) {
        if self.is_active || dt <= 0.0 || !dt.is_finite() {
            return; // Do not record while viewing replay
        }

        self.recording_clock += dt;
        self.sample_timer += dt;

        let sample_interval = 1.0 / REPLAY_SAMPLE_RATE_HZ;
        if self.sample_timer < sample_interval {
            return;
        }
        // One snapshot represents the current state; missed intervals cannot be reconstructed.
        // Retain only the fractional remainder so a slow frame cannot create a sample backlog.
        self.sample_timer %= sample_interval;

        // Bound snapshot storage to 128 chunks per frame.
        let mut chunks = Vec::with_capacity(physics.chunks.len().min(128));
        for c in physics.chunks.iter().take(128) {
            chunks.push(ReplayChunkSnapshot {
                position: c.position,
                rotation: c.rotation,
                voxel_count: c.voxels.len().min(65535) as u16,
                local_bounds: c.local_aabb,
            });
        }

        // Snapshot projectiles
        let mut projectiles = Vec::with_capacity(weapon.projectiles.len() + siege.colossal_shells.len());
        for p in &weapon.projectiles {
            let is_artillery = p.velocity.length() > 80.0 || p.blast_radius >= 4.0;
            projectiles.push(ReplayProjectileSnapshot {
                position: p.position,
                velocity: p.velocity,
                is_artillery,
            });
        }
        for shell in &siege.colossal_shells {
            if shell.life > 0.0 && !shell.has_detonated {
                projectiles.push(ReplayProjectileSnapshot {
                    position: shell.position,
                    velocity: shell.velocity,
                    is_artillery: true,
                });
            }
        }

        let (pitch, yaw, roll) = player.camera_rotation_angles();
        let frame = ReplayFrame {
            timestamp: self.recording_clock,
            player_pos: player.position,
            player_rot: Vec3::new(pitch, yaw, roll),
            player_cam_pos: player.camera_position(),
            chunks,
            projectiles,
        };

        self.ring_buffer[self.head_idx] = Some(frame);
        self.head_idx = (self.head_idx + 1) % MAX_REPLAY_FRAMES;
        self.total_recorded_frames = self.total_recorded_frames.saturating_add(1);
    }

    /// Tracks live artillery every rendered frame, independently of the recording sample rate.
    pub fn update_flight_cam(&mut self, weapon: &WeaponSystem, siege: &SiegeSystem) {
        let tracked = if self.flight_cam_active && !self.is_active {
            siege.colossal_shells.iter().rev()
                .find(|s| s.life > 0.0 && !s.has_detonated)
                .map(|s| (s.position, s.velocity))
                .or_else(|| weapon.projectiles.iter().rev()
                    .find(|p| p.life > 0.0 && (p.velocity.length_squared() > 3600.0 || p.blast_radius >= 3.5))
                    .map(|p| (p.position, p.velocity)))
        } else {
            None
        };
        self.tracked_projectile_pos = tracked.map(|(position, _)| position);
        self.tracked_projectile_dir = tracked.map(|(_, velocity)| velocity.try_normalize().unwrap_or(Vec3::NEG_Y));
    }

    /// Toggles between Live Simulation and Cinematic Replay mode with custom focus target
    pub fn toggle_replay_with_target(&mut self, target_pos: Vec3) -> bool {
        if !self.is_active && self.total_recorded_frames == 0 {
            return false;
        }
        self.is_active = !self.is_active;
        if self.is_active {
            self.playback_time = self.newest_timestamp();
            self.is_paused = true;
            self.orbit_target = target_pos;
            self.update_playback(0.0);
        }
        self.is_active
    }

    pub fn toggle_replay(&mut self) -> bool {
        self.toggle_replay_with_target(self.orbit_target)
    }

    pub fn duration_available(&self) -> f32 {
        (self.newest_timestamp() - self.oldest_timestamp()).max(0.0)
    }

    pub fn oldest_timestamp(&self) -> f32 {
        let frames = self.total_recorded_frames.min(MAX_REPLAY_FRAMES);
        if frames == 0 {
            return 0.0;
        }
        let oldest_idx = (self.head_idx + MAX_REPLAY_FRAMES - frames) % MAX_REPLAY_FRAMES;
        self.ring_buffer[oldest_idx].as_ref().map_or(0.0, |frame| frame.timestamp)
            .max(self.newest_timestamp() - REPLAY_BUFFER_SECONDS)
    }

    pub fn newest_timestamp(&self) -> f32 {
        let newest_idx = (self.head_idx + MAX_REPLAY_FRAMES - 1) % MAX_REPLAY_FRAMES;
        self.ring_buffer[newest_idx].as_ref().map_or(0.0, |frame| frame.timestamp)
    }

    /// Updates playback scrubber time
    pub fn update_playback(&mut self, dt: f32) {
        if !self.is_active {
            return;
        }

        if !self.is_paused && dt.is_finite() {
            self.playback_time += dt * self.playback_speed;
            let oldest = self.oldest_timestamp();
            let newest = self.newest_timestamp();
            let duration = newest - oldest;

            if duration <= 0.0 {
                self.playback_time = oldest;
            } else if self.playback_time > newest || self.playback_time < oldest {
                self.playback_time = oldest + (self.playback_time - oldest).rem_euclid(duration);
            }
        }

        // Orbit camera calculations
        let cos_p = self.orbit_pitch.cos();
        let sin_p = self.orbit_pitch.sin();
        let cos_y = self.orbit_yaw.cos();
        let sin_y = self.orbit_yaw.sin();

        self.orbit_cam_pos = self.orbit_target + Vec3::new(
            cos_y * cos_p * self.orbit_distance,
            sin_p * self.orbit_distance,
            sin_y * cos_p * self.orbit_distance,
        );
    }

    /// Finds the closest recorded keyframe to current playback_time
    pub fn interpolated_frame(&self) -> Option<&ReplayFrame> {
        if self.total_recorded_frames == 0 {
            return None;
        }

        let count = self.total_recorded_frames.min(MAX_REPLAY_FRAMES);
        let start = (self.head_idx + MAX_REPLAY_FRAMES - count) % MAX_REPLAY_FRAMES;
        let frame_at = |idx: usize| self.ring_buffer[(start + idx) % MAX_REPLAY_FRAMES].as_ref();
        let time = self.playback_time.clamp(self.oldest_timestamp(), self.newest_timestamp());
        // Frames are chronological even when the physical ring wraps. Binary search avoids
        // scanning up to 1,200 snapshots every playback frame.
        let (mut lo, mut hi) = (0, count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if frame_at(mid)?.timestamp < time {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return frame_at(0);
        }
        if lo == count {
            return frame_at(count - 1);
        }
        let previous = frame_at(lo - 1)?;
        let next = frame_at(lo)?;
        if previous.timestamp >= self.oldest_timestamp()
            && time - previous.timestamp <= next.timestamp - time {
            Some(previous)
        } else {
            Some(next)
        }
    }

    /// Draws recorded bounds proxies without changing live chunk transforms or requiring
    /// retained historical voxel meshes. The static environment remains the current world.
    pub fn draw_recorded_entities(&self) {
        let Some(frame) = self.interpolated_frame() else { return; };
        for chunk in &frame.chunks {
            let (min, max) = chunk.local_bounds;
            let rot = crate::physics::Mat3::from_euler(chunk.rotation);
            let corners = [
                Vec3::new(min.x, min.y, min.z), Vec3::new(max.x, min.y, min.z),
                Vec3::new(max.x, max.y, min.z), Vec3::new(min.x, max.y, min.z),
                Vec3::new(min.x, min.y, max.z), Vec3::new(max.x, min.y, max.z),
                Vec3::new(max.x, max.y, max.z), Vec3::new(min.x, max.y, max.z),
            ].map(|p| chunk.position + rot.transform_vector(p));
            for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
                draw_line_3d(corners[a], corners[b], Color::new(0.3, 0.85, 1.0, 0.85));
            }
        }
        let avatar_pos = frame.player_pos + Vec3::new(0.0, 0.9, 0.0);
        draw_cube(avatar_pos, Vec3::new(0.6, 1.8, 0.6), None, Color::new(0.2, 0.8, 1.0, 0.45));
        draw_cube_wires(avatar_pos, Vec3::new(0.6, 1.8, 0.6), Color::new(0.4, 0.9, 1.0, 0.85));
        for projectile in &frame.projectiles {
            draw_sphere(projectile.position, if projectile.is_artillery { 0.45 } else { 0.22 }, None, Color::new(1.0, 0.6, 0.1, 0.9));
        }
    }

    /// Renders Replay HUD / Scrubber overlays
    pub fn draw_replay_hud(&self) {
        if !self.is_active {
            return;
        }

        let w = screen_width();
        let h = screen_height();
        let s = crate::ui::compute_ui_scale(w, h);
        let pad = 24.0 * s;

        // Top Banner
        let banner_h = 40.0 * s;
        draw_rectangle(0.0, 0.0, w, banner_h, Color::new(0.04, 0.06, 0.10, 0.88));
        let title = crate::ui::fit_text_to_width("REPLAY: RECORDED BOUNDS / CURRENT ENVIRONMENT", w - 2.0 * pad, 18.0 * s);
        draw_text(&title, pad, 26.0 * s, 18.0 * s, Color::new(0.3, 0.85, 1.0, 1.0));

        // Bottom Scrubber Bar
        let bar_h = 64.0 * s;
        let bar_y = h - bar_h;
        draw_rectangle(0.0, bar_y, w, bar_h, Color::new(0.04, 0.06, 0.10, 0.88));

        let oldest = self.oldest_timestamp();
        let duration = self.duration_available().max(0.1);
        let progress = ((self.playback_time - oldest) / duration).clamp(0.0, 1.0);

        let track_x = 40.0 * s;
        let track_w = w - 80.0 * s;
        let track_y = bar_y + 24.0 * s;
        draw_rectangle(track_x, track_y, track_w, 6.0 * s, Color::new(0.2, 0.25, 0.35, 1.0));
        draw_rectangle(track_x, track_y, track_w * progress, 6.0 * s, Color::new(0.3, 0.8, 1.0, 1.0));
        draw_circle(track_x + track_w * progress, track_y + 3.0 * s, 7.0 * s, Color::new(1.0, 0.9, 0.2, 1.0));

        // Timeline text
        let status_str = if self.is_paused { "PAUSED" } else { "PLAYING" };
        let info = format!("TIME: {:.1}s / {:.1}s | SPEED: {:.2}x | {}", self.playback_time, self.newest_timestamp(), self.playback_speed, status_str);
        draw_text(&info, track_x, bar_y + 16.0 * s, 14.0 * s, WHITE);

        // Hotkey guidance strip, built from the live binding table and fitted to the screen.
        let guide = Self::guide_text();
        let guide_fit = crate::ui::fit_text_to_width(&guide, track_w, 12.0 * s);
        draw_text(&guide_fit, track_x, bar_y + 50.0 * s, 12.0 * s, Color::new(0.35, 0.88, 1.0, 0.95));
    }

    /// Replay control hint line, e.g. `[SPACE] PLAY/PAUSE | [LEFT/RIGHT] SCRUB | ...`.
    pub fn guide_text() -> String {
        use crate::controls;
        format!(
            "{} PLAY/PAUSE  |  {} SCRUB  |  {} SPEED  |  {} ORBIT  |  {} ZOOM  |  {} FLIGHT CAM  |  {} EXIT  |  {} HELP",
            controls::REPLAY_PAUSE.token,
            controls::REPLAY_SCRUB.token,
            controls::REPLAY_SPEED.token,
            controls::REPLAY_ORBIT.token,
            controls::REPLAY_ZOOM.token,
            controls::FLIGHT_CAM.token,
            controls::REPLAY.token,
            controls::MANUAL.token
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_system_circular_buffer_capacity() {
        let mut replay = ReplaySystem::new();
        assert_eq!(replay.ring_buffer.len(), MAX_REPLAY_FRAMES);
        assert_eq!(replay.total_recorded_frames, 0);

        let player = Player::new(Vec3::new(0.0, 5.0, 0.0));
        let physics = PhysicsSystem::new();
        let weapon = WeaponSystem::new();

        // Feed 10 steps
        for _ in 0..10 {
            replay.record_step(0.05, &player, &physics, &weapon, &SiegeSystem::new());
        }
        assert_eq!(replay.total_recorded_frames, 10);
        assert_eq!(replay.head_idx, 10);
    }

    #[test]
    fn test_replay_toggle_and_playback_scrubbing() {
        let mut replay = ReplaySystem::new();
        assert!(!replay.is_active);

        let player = Player::new(Vec3::ZERO);
        replay.record_step(0.05, &player, &PhysicsSystem::new(), &WeaponSystem::new(), &SiegeSystem::new());
        assert!(replay.toggle_replay());
        assert!(replay.is_active);
        assert!(replay.is_paused);

        // Adjust speed
        replay.playback_speed = 0.5;
        replay.is_paused = false;
        replay.update_playback(1.0);
        assert!((replay.playback_speed - 0.5).abs() < 1e-4);

        assert!(!replay.toggle_replay());
        assert!(!replay.is_active);
    }

    #[test]
    fn test_artillery_flight_cam_tracking() {
        let mut replay = ReplaySystem::new();
        replay.flight_cam_active = true;

        let player = Player::new(Vec3::ZERO);
        let physics = PhysicsSystem::new();
        let mut weapon = WeaponSystem::new();

        // Add high-velocity artillery projectile
        weapon.projectiles.push(crate::weapon::ActiveProjectile {
            position: Vec3::new(0.0, 50.0, 0.0),
            velocity: Vec3::new(0.0, -100.0, 0.0),
            gravity: 9.81,
            radius: 0.5,
            blast_radius: 5.0,
            impulse: 50.0,
            mode: crate::weapon::WeaponMode::HavocMortar,
            barrel: crate::weapon::BarrelType::Precision,
            payload: crate::weapon::PayloadType::HighExplosive,
            trail_style: crate::weapon::TrailStyle::HyperTracer,
            color: WHITE,
            life: 5.0,
            is_cluster_child: false,
            is_airburst: false,
            is_sticky: false,
            stuck_normal: Vec3::ZERO,
            trail_points: crate::weapon::TrailBuffer::new(),
        });

        replay.update_flight_cam(&weapon, &SiegeSystem::new());
        replay.record_step(0.1, &player, &physics, &weapon, &SiegeSystem::new());
        assert!(replay.tracked_projectile_pos.is_some());
        assert_eq!(replay.tracked_projectile_pos.unwrap(), Vec3::new(0.0, 50.0, 0.0));
    }

    #[test]
    fn missed_sample_intervals_do_not_create_recording_backlog() {
        let mut replay = ReplaySystem::new();
        let player = Player::new(Vec3::ZERO);
        let physics = PhysicsSystem::new();
        let weapon = WeaponSystem::new();
        replay.record_step(0.23, &player, &physics, &weapon, &SiegeSystem::new());
        assert_eq!(replay.total_recorded_frames, 1);
        replay.record_step(0.0, &player, &physics, &weapon, &SiegeSystem::new());
        assert_eq!(replay.total_recorded_frames, 1, "No new sample without elapsed time");
        assert!((replay.sample_timer - 0.03).abs() < 1e-5);
    }

    #[test]
    fn timeline_bounds_follow_recorded_timestamps_at_low_frame_rate() {
        let mut replay = ReplaySystem::new();
        let player = Player::new(Vec3::ZERO);
        let physics = PhysicsSystem::new();
        let weapon = WeaponSystem::new();
        for _ in 0..100 {
            replay.record_step(0.1, &player, &physics, &weapon, &SiegeSystem::new());
        }
        assert!((replay.oldest_timestamp() - 0.1).abs() < 1e-4);
        assert!((replay.duration_available() - 9.9).abs() < 1e-3);
    }

    #[test]
    fn clearing_replay_clears_stale_flight_camera() {
        let mut replay = ReplaySystem::new();
        replay.flight_cam_active = true;
        replay.tracked_projectile_pos = Some(Vec3::new(1.0, 2.0, 3.0));
        replay.tracked_projectile_dir = Some(Vec3::Y);
        replay.playback_speed = 0.25;
        replay.is_paused = true;
        replay.clear();
        assert!(!replay.flight_cam_active);
        assert_eq!(replay.tracked_projectile_pos, None);
        assert_eq!(replay.tracked_projectile_dir, None);
        assert!(!replay.is_paused);
        assert_eq!(replay.playback_speed, 1.0);
    }

    #[test]
    fn empty_recording_cannot_enter_replay() {
        let mut replay = ReplaySystem::new();
        assert!(!replay.toggle_replay());
        assert!(!replay.is_active);
    }

    #[test]
    fn nearest_keyframe_search_handles_ring_wrap_and_playback_overshoot() {
        let mut replay = ReplaySystem::new();
        let player = Player::new(Vec3::ZERO);
        let physics = PhysicsSystem::new();
        let weapon = WeaponSystem::new();
        let siege = SiegeSystem::new();
        for _ in 0..MAX_REPLAY_FRAMES + 20 {
            replay.record_step(0.05, &player, &physics, &weapon, &siege);
        }
        let oldest = replay.oldest_timestamp();
        let newest = replay.newest_timestamp();
        replay.playback_time = oldest + 0.21;
        let frame = replay.interpolated_frame().unwrap();
        assert!((frame.timestamp - (oldest + 0.20)).abs() < 1e-3);
        replay.is_active = true;
        replay.is_paused = false;
        replay.playback_time = newest - 0.1;
        replay.update_playback(0.3);
        assert!((replay.playback_time - (oldest + 0.2)).abs() < 1e-3);
        replay.playback_speed = -1.0;
        replay.update_playback(0.3);
        assert!((replay.playback_time - (newest - 0.1)).abs() < 1e-3);
    }

    #[test]
    fn siege_shells_are_recorded_and_tracked_between_samples() {
        let mut replay = ReplaySystem::new();
        replay.flight_cam_active = true;
        let player = Player::new(Vec3::ZERO);
        let physics = PhysicsSystem::new();
        let weapon = WeaponSystem::new();
        let mut siege = SiegeSystem::new();
        siege.colossal_shells.push(crate::siege::ColossalShell {
            position: Vec3::new(1.0, 50.0, 2.0), velocity: Vec3::new(0.0, -100.0, 0.0),
            caliber_mm: 600.0, radius: 0.5, blast_radius: 6.0,
            payload: crate::siege::SiegePayload::ColossalHighExplosive,
            penetrations_remaining: 0, fuse_timer: 0.0, is_fuse_active: false,
            life: 5.0, max_life: 5.0, color: WHITE,
            trail_points: crate::siege::TrailBuffer::new(), has_detonated: false,
        });
        replay.record_step(0.05, &player, &physics, &weapon, &siege);
        assert_eq!(replay.interpolated_frame().unwrap().projectiles.len(), 1);
        siege.colossal_shells[0].position.y = 49.0;
        replay.update_flight_cam(&weapon, &siege);
        assert_eq!(replay.tracked_projectile_pos, Some(Vec3::new(1.0, 49.0, 2.0)));
        assert_eq!(replay.tracked_projectile_dir, Some(Vec3::NEG_Y));
        siege.colossal_shells.clear();
        replay.update_flight_cam(&weapon, &siege);
        assert_eq!(replay.tracked_projectile_pos, None);
    }
}
