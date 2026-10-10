use macroquad::prelude::*;
use crate::voxel::{VoxelGrid, VoxelType, DamageType, VOXEL_SIZE, GRID_X, GRID_Z, MAX_VOXEL_Y, MIN_VOXEL_Y};
use crate::physics::{PhysicsSystem, DynamicPointLight};
use crate::audio::SoundSystem;

/// The four distinct deployable siege engine archetypes
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SiegeEngineKind {
    /// Deployable heavy battery launching colossal 1.5m–2.5m dreadnought shells
    RailArtillery = 0,
    /// Deployable target beacon calling down a 3m vertical ion beam / kinetic rod through all floors
    OrbitalKineticRelay = 1,
    /// Heavy pneumatic ground-pounding engine anchored to bedrock pumping rhythmic seismic waves
    HydraulicGroundRam = 2,
    /// High-voltage lightning capacitor tower latching electrical arcs onto building rebar networks
    TeslaSiegePylon = 3,
}

impl SiegeEngineKind {
    pub fn name(&self) -> &'static str {
        match self {
            SiegeEngineKind::RailArtillery => "ORBITAL RAIL ARTILLERY",
            SiegeEngineKind::OrbitalKineticRelay => "ORBITAL KINETIC RELAY",
            SiegeEngineKind::HydraulicGroundRam => "HYDRAULIC GROUND RAM",
            SiegeEngineKind::TeslaSiegePylon => "TESLA SIEGE PYLON",
        }
    }

    #[allow(dead_code)]
    pub fn subtitle(&self) -> &'static str {
        match self {
            SiegeEngineKind::RailArtillery => "HEAVY SIEGE TREBUCHET & RAIL BATTERY (500-2500mm)",
            SiegeEngineKind::OrbitalKineticRelay => "ION BEAM DOWNLINK TARGET BEACON",
            SiegeEngineKind::HydraulicGroundRam => "BEDROCK ANCHORED SEISMIC POUNDER",
            SiegeEngineKind::TeslaSiegePylon => "HIGH-VOLTAGE REBAR ARC CAPACITOR",
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
            SiegeEngineKind::RailArtillery => "Deployable heavy battery & siege trebuchet platform launching colossal 1.5m-2.5m shells with customizable azimuth, elevation, propellant charge, and payload. Punches devastating holes clean through exterior facades and structural cores.",
            SiegeEngineKind::OrbitalKineticRelay => "Calls down a 3-meter wide vertical kinetic rod / ion beam of light straight down through all 14 stories of a skyscraper.",
            SiegeEngineKind::HydraulicGroundRam => "Heavy pneumatic ground-pounding engine that anchors to bedrock and pumps rhythmic seismic shockwaves into foundation columns to trigger pancake collapses.",
            SiegeEngineKind::TeslaSiegePylon => "High-voltage lightning capacitor discharging cascading electrical arcs that jump through conductive building rebar grids.",
        }
    }

    pub fn theme_color(&self) -> Color {
        match self {
            SiegeEngineKind::RailArtillery => Color::new(1.0, 0.45, 0.15, 1.0),     // Blaze orange
            SiegeEngineKind::OrbitalKineticRelay => Color::new(0.2, 0.85, 1.0, 1.0), // Ion cyan
            SiegeEngineKind::HydraulicGroundRam => Color::new(1.0, 0.80, 0.2, 1.0),  // Industrial amber
            SiegeEngineKind::TeslaSiegePylon => Color::new(0.65, 0.45, 1.0, 1.0),    // Arc violet
        }
    }

    pub fn base_cooldown(&self) -> f32 {
        match self {
            SiegeEngineKind::RailArtillery => 2.4,
            SiegeEngineKind::OrbitalKineticRelay => 4.5,
            SiegeEngineKind::HydraulicGroundRam => 1.4,
            SiegeEngineKind::TeslaSiegePylon => 2.0,
        }
    }
}

/// Advanced custom warhead payloads for siege ordnance
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SiegePayload {
    /// Monumental spherical blast volume peeling outer facades and crumbling floors
    ColossalHighExplosive = 0,
    /// Reinforced hardened core penetrating 4-8 walls before terminal core blast
    BunkerBuster = 1,
    /// High-temperature exothermic compound liquefying reinforced concrete and tungsten armor
    ThermiteMelt = 2,
    /// Colossal sub-zero flash-freeze burst followed by concussive shatter wave
    CryoShatter = 3,
    /// Gravitational singularity core pulling surrounding building sections inward
    AntimatterSingularity = 4,
}

impl SiegePayload {
    pub fn name(&self) -> &'static str {
        match self {
            SiegePayload::ColossalHighExplosive => "COLOSSAL HIGH EXPLOSIVE",
            SiegePayload::BunkerBuster => "BUNKER-BUSTING PENETRATOR",
            SiegePayload::ThermiteMelt => "THERMITE MELT INCENDIARY",
            SiegePayload::CryoShatter => "CRYO SHATTER BOMB",
            SiegePayload::AntimatterSingularity => "ANTIMATTER SINGULARITY CORE",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SiegePayload::ColossalHighExplosive => "Monumental explosive volume with massive concussive overpressure peeling outer facades and crumbling floors.",
            SiegePayload::BunkerBuster => "Hardened dense penetrator with delayed fuse; pierces through consecutive exterior walls before detonating in the building core.",
            SiegePayload::ThermiteMelt => "High-temperature chemical compound melting reinforced concrete and liquefying internal structural rebar.",
            SiegePayload::CryoShatter => "Flash-freezes a 12m sphere of voxels into brittle ice, then shatters it into a cloud of airborne shards.",
            SiegePayload::AntimatterSingularity => "Miniature gravitational singularity pulling surrounding towers and walls inward into a violent vortex collapse.",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            SiegePayload::ColossalHighExplosive => Color::new(1.0, 0.40, 0.1, 1.0),
            SiegePayload::BunkerBuster => Color::new(0.9, 0.75, 0.3, 1.0),
            SiegePayload::ThermiteMelt => Color::new(1.0, 0.25, 0.05, 1.0),
            SiegePayload::CryoShatter => Color::new(0.4, 0.95, 1.0, 1.0),
            SiegePayload::AntimatterSingularity => Color::new(0.85, 0.25, 1.0, 1.0),
        }
    }
}

/// Trajectory tuning modes for siege ballistics
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SiegeTrajectory {
    /// High parabolic lob (elevation 60°-75°) arcing over outer defenses to strike roofs and penthouses
    HighMortarLob = 0,
    /// Flat high-velocity direct slug (elevation 10°-25°) punching straight through lower facades and core
    DirectBreach = 1,
    /// Vertical orbital strike (elevation 90°) beamed directly from satellite zenith
    OrbitalBeam = 2,
}

impl SiegeTrajectory {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            SiegeTrajectory::HighMortarLob => "HIGH MORTAR LOB (65°)",
            SiegeTrajectory::DirectBreach => "DIRECT BREACH (18°)",
            SiegeTrajectory::OrbitalBeam => "ORBITAL BEAM (90°)",
        }
    }

    pub fn default_elevation_deg(&self) -> f32 {
        match self {
            SiegeTrajectory::HighMortarLob => 65.0,
            SiegeTrajectory::DirectBreach => 18.0,
            SiegeTrajectory::OrbitalBeam => 88.0,
        }
    }
}

/// Full modular customization configuration for the siege weapon engine
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SiegeWeaponConfig {
    pub engine_kind: SiegeEngineKind,
    pub payload: SiegePayload,
    pub caliber_mm: f32,       // 500.0 to 2500.0 mm
    pub propellant_power: f32, // 0.5 to 3.0x
    pub trajectory: SiegeTrajectory,
    pub elevation_deg: f32,    // 10.0 to 88.0 degrees
    pub azimuth_deg: f32,      // 0.0 to 360.0 degrees
    pub delayed_fuse_secs: f32,// 0.06 to 0.45s
    pub auto_aim_reticle: bool,
}

impl Default for SiegeWeaponConfig {
    fn default() -> Self {
        Self {
            engine_kind: SiegeEngineKind::RailArtillery,
            payload: SiegePayload::ColossalHighExplosive,
            caliber_mm: 1500.0,
            propellant_power: 1.5,
            trajectory: SiegeTrajectory::HighMortarLob,
            elevation_deg: 65.0,
            azimuth_deg: 0.0,
            delayed_fuse_secs: 0.18,
            auto_aim_reticle: true,
        }
    }
}

impl SiegeWeaponConfig {
    /// Physical shell radius in meters: scales from 0.75m (1.5m diameter) to 1.75m (3.5m diameter)
    pub fn shell_radius(&self) -> f32 {
        let t = ((self.caliber_mm - 500.0) / 2000.0).clamp(0.0, 1.0);
        0.75 + t * 1.0
    }

    /// Shell blast radius: colossal detonation covering 8m to 16m radius
    pub fn blast_radius(&self) -> f32 {
        let base_r = match self.payload {
            SiegePayload::ColossalHighExplosive => 11.5,
            SiegePayload::BunkerBuster => 8.5,
            SiegePayload::ThermiteMelt => 9.5,
            SiegePayload::CryoShatter => 12.0,
            SiegePayload::AntimatterSingularity => 14.5,
        };
        let caliber_mult = 0.8 + (self.caliber_mm / 2500.0) * 0.6;
        let power_mult = self.propellant_power.sqrt().clamp(0.7, 1.8);
        base_r * caliber_mult * power_mult
    }

    /// Launch velocity in m/s
    pub fn launch_velocity(&self) -> f32 {
        let base_spd = match self.trajectory {
            SiegeTrajectory::HighMortarLob => 55.0,
            SiegeTrajectory::DirectBreach => 90.0,
            SiegeTrajectory::OrbitalBeam => 140.0,
        };
        base_spd * self.propellant_power
    }

    /// Calculates initial 3D velocity vector for firing based on azimuth & elevation
    pub fn compute_launch_velocity_vector(&self, forward_yaw: f32) -> Vec3 {
        let eff_yaw = if self.auto_aim_reticle { forward_yaw } else { self.azimuth_deg.to_radians() };
        let elev_rad = self.elevation_deg.to_radians();
        let speed = self.launch_velocity();

        let vx = eff_yaw.cos() * elev_rad.cos() * speed;
        let vy = elev_rad.sin() * speed;
        let vz = eff_yaw.sin() * elev_rad.cos() * speed;
        Vec3::new(vx, vy, vz)
    }
}

/// A deployed siege weapon platform anchored in the arena
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DeployedSiegeEngine {
    pub id: usize,
    pub kind: SiegeEngineKind,
    pub position: Vec3,
    pub facing_yaw: f32,
    pub elevation_pitch: f32,
    pub config: SiegeWeaponConfig,
    pub cooldown_timer: f32,
    pub max_cooldown: f32,
    pub anim_phase: f32,
    pub muzzle_flash_timer: f32,
    pub is_firing: bool,
    pub is_anchored: bool,
    pub shots_fired: usize,
    pub target_point: Option<Vec3>,
    pub resonance_slam_count: usize,
    pub last_slam_time: f32,
}

impl DeployedSiegeEngine {
    pub fn new(id: usize, kind: SiegeEngineKind, position: Vec3, facing_yaw: f32, config: SiegeWeaponConfig) -> Self {
        let cd = kind.base_cooldown();
        Self {
            id,
            kind,
            position,
            facing_yaw,
            elevation_pitch: config.elevation_deg.to_radians(),
            config,
            cooldown_timer: 0.0,
            max_cooldown: cd,
            anim_phase: 0.0,
            muzzle_flash_timer: 0.0,
            is_firing: false,
            is_anchored: true,
            shots_fired: 0,
            target_point: None,
            resonance_slam_count: 0,
            last_slam_time: -100.0,
        }
    }

    pub fn ready_ratio(&self) -> f32 {
        if self.max_cooldown <= 0.001 {
            1.0
        } else {
            (1.0 - (self.cooldown_timer / self.max_cooldown)).clamp(0.0, 1.0)
        }
    }

    pub fn is_ready(&self) -> bool {
        self.cooldown_timer <= 0.0
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

/// Colossal siege projectile in flight with penetration mechanics
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct ColossalShell {
    pub position: Vec3,
    pub velocity: Vec3,
    pub caliber_mm: f32,
    pub radius: f32,
    pub blast_radius: f32,
    pub payload: SiegePayload,
    pub penetrations_remaining: usize,
    pub fuse_timer: f32,
    pub is_fuse_active: bool,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
    pub trail_points: TrailBuffer<18>,
    pub has_detonated: bool,
}

/// Searing orbital ion beam strike descending from satellite zenith
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct OrbitalBeamStrike {
    pub beacon_pos: Vec3,
    pub target_pos: Vec3,
    pub radius: f32,
    pub warmup_timer: f32,
    pub strike_duration: f32,
    pub max_duration: f32,
    pub has_struck: bool,
    pub color: Color,
}

/// Bedrock seismic pulse wave pumped into building foundations
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SeismicGroundPulse {
    pub origin: Vec3,
    pub prev_radius: f32,
    pub current_radius: f32,
    pub max_radius: f32,
    pub speed: f32,
    pub power: f32,
    pub resonance_level: usize,
    pub resonance_mult: f32,
    pub life: f32,
}

/// High-voltage rebar arc discharge
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct TeslaSiegeDischarge {
    pub origin: Vec3,
    pub branches: Vec<(Vec3, Vec3)>,
    pub life: f32,
    pub max_life: f32,
    pub color: Color,
}

/// Central siege orchestration engine
#[allow(dead_code)]
pub struct SiegeSystem {
    pub config: SiegeWeaponConfig,
    pub deployed_engines: Vec<DeployedSiegeEngine>,
    pub colossal_shells: Vec<ColossalShell>,
    pub orbital_strikes: Vec<OrbitalBeamStrike>,
    pub seismic_pulses: Vec<SeismicGroundPulse>,
    pub tesla_discharges: Vec<TeslaSiegeDischarge>,
    pub next_id: usize,
    pub selected_kind: SiegeEngineKind,
    pub deploy_preview: Option<Vec3>,
    pub placement_valid: bool,
    pub hud_feedback_msg: Option<(String, f32)>,
}

impl SiegeSystem {
    pub fn new() -> Self {
        Self {
            config: SiegeWeaponConfig::default(),
            deployed_engines: Vec::new(),
            colossal_shells: Vec::new(),
            orbital_strikes: Vec::new(),
            seismic_pulses: Vec::new(),
            tesla_discharges: Vec::new(),
            next_id: 1,
            selected_kind: SiegeEngineKind::RailArtillery,
            deploy_preview: None,
            placement_valid: true,
            hud_feedback_msg: None,
        }
    }

    /// Pre-deploys a standard allied siege battery in front of the citadel
    pub fn setup_default_fleet(&mut self) {
        self.deployed_engines.clear();
        self.colossal_shells.clear();
        self.orbital_strikes.clear();
        self.seismic_pulses.clear();
        self.tesla_discharges.clear();

        // 1. Heavy Rail Artillery Battery (1500mm Dreadnought Cannon) on the left flank
        let mut rail_cfg = self.config;
        rail_cfg.engine_kind = SiegeEngineKind::RailArtillery;
        rail_cfg.caliber_mm = 1500.0;
        self.deployed_engines.push(DeployedSiegeEngine::new(
            1,
            SiegeEngineKind::RailArtillery,
            Vec3::new(-16.0, 0.50, 16.0),
            -std::f32::consts::FRAC_PI_2,
            rail_cfg,
        ));

        // 2. Orbital Kinetic Relay (Ion Downlink Pylon) on the right flank
        let mut orb_cfg = self.config;
        orb_cfg.engine_kind = SiegeEngineKind::OrbitalKineticRelay;
        orb_cfg.trajectory = SiegeTrajectory::OrbitalBeam;
        self.deployed_engines.push(DeployedSiegeEngine::new(
            2,
            SiegeEngineKind::OrbitalKineticRelay,
            Vec3::new(16.0, 0.50, 16.0),
            -std::f32::consts::FRAC_PI_2,
            orb_cfg,
        ));

        // 3. Hydraulic Ground Ram (Seismic Bedrock Pounder) center-forward
        let mut ram_cfg = self.config;
        ram_cfg.engine_kind = SiegeEngineKind::HydraulicGroundRam;
        self.deployed_engines.push(DeployedSiegeEngine::new(
            3,
            SiegeEngineKind::HydraulicGroundRam,
            Vec3::new(0.0, 0.50, 12.0),
            -std::f32::consts::FRAC_PI_2,
            ram_cfg,
        ));

        self.next_id = 4;
        self.selected_kind = SiegeEngineKind::RailArtillery;
        self.set_hud_message("SIEGE FLEET ONLINE: 3 BATTERIES DEPLOYED IN FIELD");
    }

    /// Sets the active siege engine archetype
    pub fn select_kind(&mut self, kind: SiegeEngineKind, audio: &SoundSystem) {
        if self.selected_kind != kind {
            self.selected_kind = kind;
            self.config.engine_kind = kind;
            self.set_hud_message(&format!("SELECTED SIEGE ENGINE: {}", kind.name()));
            audio.play(audio.ui_switch.as_ref(), 0.7);
        }
    }

    /// Cycle through siege engine archetypes
    pub fn cycle_engine_kind(&mut self, audio: &SoundSystem) {
        let next_kind = match self.selected_kind {
            SiegeEngineKind::RailArtillery => SiegeEngineKind::OrbitalKineticRelay,
            SiegeEngineKind::OrbitalKineticRelay => SiegeEngineKind::HydraulicGroundRam,
            SiegeEngineKind::HydraulicGroundRam => SiegeEngineKind::TeslaSiegePylon,
            SiegeEngineKind::TeslaSiegePylon => SiegeEngineKind::RailArtillery,
        };
        self.select_kind(next_kind, audio);
    }

    pub fn set_hud_message(&mut self, msg: &str) {
        self.hud_feedback_msg = Some((msg.to_string(), 3.2));
    }

    /// Deploys or repositions the currently selected siege engine at `target_pos`
    pub fn deploy_selected_engine(
        &mut self,
        target_pos: Vec3,
        facing_dir: Vec3,
        audio: &SoundSystem,
    ) -> bool {
        // Bedrock physical foundation elevation is y = 0.50
        let anchor_pos = Vec3::new(target_pos.x, 0.50, target_pos.z);
        let facing_yaw = facing_dir.z.atan2(facing_dir.x);

        // If an engine of this kind already exists, redeploy / reposition it
        if let Some(existing) = self.deployed_engines.iter_mut().find(|e| e.kind == self.selected_kind) {
            existing.position = anchor_pos;
            existing.facing_yaw = facing_yaw;
            existing.config = self.config;
            existing.cooldown_timer = 0.4;
            existing.anim_phase = 0.0;
            self.set_hud_message(&format!("REDEPLOYED: {} (ANCHORED)", self.selected_kind.name()));
            audio.play(audio.hydraulic_slam.as_ref(), 0.85);
            return true;
        }

        // Limit concurrent deployed engines to 4 (one of each kind)
        if self.deployed_engines.len() >= 4 {
            self.deployed_engines.remove(0);
        }

        let id = self.next_id;
        self.next_id += 1;
        let engine = DeployedSiegeEngine::new(id, self.selected_kind, anchor_pos, facing_yaw, self.config);
        self.deployed_engines.push(engine);

        self.set_hud_message(&format!(
            "DEPLOYED: {} [PRESS {} TO FIRE]",
            self.selected_kind.name(),
            crate::controls::SIEGE_FIRE.label
        ));
        audio.play(audio.hydraulic_slam.as_ref(), 0.90);
        true
    }

    /// Recall / undeploy all engines and despawn all active siege shells and strikes
    pub fn recall_all(&mut self, audio: &SoundSystem) {
        let had_active = !self.deployed_engines.is_empty()
            || !self.colossal_shells.is_empty()
            || !self.orbital_strikes.is_empty()
            || !self.seismic_pulses.is_empty()
            || !self.tesla_discharges.is_empty();

        self.deployed_engines.clear();
        self.colossal_shells.clear();
        self.orbital_strikes.clear();
        self.seismic_pulses.clear();
        self.tesla_discharges.clear();

        if had_active {
            self.set_hud_message("ALL SIEGE ENGINES RECALLED TO ARMORY");
            audio.play(audio.ui_switch.as_ref(), 0.7);
        }
    }

    #[allow(dead_code)]
    pub fn get_engine_config(&self, kind: SiegeEngineKind) -> SiegeWeaponConfig {
        if let Some(eng) = self.deployed_engines.iter().find(|e| e.kind == kind) {
            eng.config
        } else {
            self.config
        }
    }

    #[allow(dead_code)]
    pub fn set_engine_config(&mut self, kind: SiegeEngineKind, config: SiegeWeaponConfig) {
        if let Some(eng) = self.deployed_engines.iter_mut().find(|e| e.kind == kind) {
            eng.config = config;
        }
        self.config = config;
    }

    /// Fires the active / selected deployed siege engine towards `aim_target`
    pub fn fire_active_engine(
        &mut self,
        aim_target: Option<Vec3>,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
        camera_pos: Vec3,
    ) -> f32 {
        // Fire only the selected battery; its cooldown must not trigger another archetype.
        let engine_idx = self.deployed_engines.iter().position(|e| e.kind == self.selected_kind);

        let idx = match engine_idx {
            Some(i) if self.deployed_engines[i].is_ready() => i,
            Some(_) => {
                self.set_hud_message("SIEGE ENGINE CHARGING / RELOADING...");
                return 0.0;
            }
            None => {
                let auto_pos = Vec3::new(-16.0, 0.50, 16.0);
                let auto_eng = DeployedSiegeEngine::new(
                    self.next_id,
                    self.selected_kind,
                    auto_pos,
                    -std::f32::consts::FRAC_PI_2,
                    self.config,
                );
                self.next_id += 1;
                self.deployed_engines.push(auto_eng);
                self.deployed_engines.len() - 1
            }
        };

        let mut trauma = 0.0f32;
        let engine_pos = self.deployed_engines[idx].position;
        let engine_kind = self.deployed_engines[idx].kind;
        let config = self.deployed_engines[idx].config;

        // Update engine state
        self.deployed_engines[idx].cooldown_timer = self.deployed_engines[idx].max_cooldown;
        self.deployed_engines[idx].shots_fired += 1;
        self.deployed_engines[idx].muzzle_flash_timer = 0.25;
        self.deployed_engines[idx].is_firing = true;

        // Target orientation
        let target = aim_target.unwrap_or_else(|| {
            let fw = Vec3::new(self.deployed_engines[idx].facing_yaw.cos(), 0.0, self.deployed_engines[idx].facing_yaw.sin());
            engine_pos + fw * 45.0 + Vec3::new(0.0, 10.0, 0.0)
        });
        self.deployed_engines[idx].target_point = Some(target);

        let to_target = (target - engine_pos).normalize_or_zero();
        let target_yaw = to_target.z.atan2(to_target.x);
        let eff_facing_yaw = if config.auto_aim_reticle {
            target_yaw
        } else {
            config.azimuth_deg.to_radians()
        };
        self.deployed_engines[idx].facing_yaw = eff_facing_yaw;

        match engine_kind {
            SiegeEngineKind::RailArtillery => {
                let fire_dir_h = Vec3::new(eff_facing_yaw.cos(), 0.0, eff_facing_yaw.sin());
                let muzzle_pos = engine_pos + Vec3::new(0.0, 2.8, 0.0) + fire_dir_h * 3.5;
                let vel = config.compute_launch_velocity_vector(eff_facing_yaw);
                let shell_r = config.shell_radius();
                let blast_r = config.blast_radius();

                let shell = ColossalShell {
                    position: muzzle_pos,
                    velocity: vel,
                    caliber_mm: config.caliber_mm,
                    radius: shell_r,
                    blast_radius: blast_r,
                    payload: config.payload,
                    penetrations_remaining: match config.payload {
                        SiegePayload::BunkerBuster => 6,
                        _ => 1,
                    },
                    fuse_timer: config.delayed_fuse_secs,
                    is_fuse_active: false,
                    life: 0.0,
                    max_life: 8.0,
                    color: config.payload.color(),
                    trail_points: TrailBuffer::single(muzzle_pos),
                    has_detonated: false,
                };
                self.colossal_shells.push(shell);

                // Colossal Muzzle Blast
                physics.add_colossal_siege_dust_plume(muzzle_pos, 4.0, Color::new(0.7, 0.68, 0.65, 0.15));
                physics.add_ground_cratering_shockwave(engine_pos, 10.0);
                physics.apply_radial_blast(muzzle_pos, 8.0, 45.0);

                audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), muzzle_pos, camera_pos, 0.95);
                audio.play_spatial(audio.railgun_boom.as_ref(), muzzle_pos, camera_pos, 0.90);
                trauma += 0.35 * SoundSystem::calculate_distance_attenuation(muzzle_pos, camera_pos);
                self.set_hud_message(&format!("ORBITAL RAIL ARTILLERY: FIRED {}mm SHELL!", config.caliber_mm as i32));
            }

            SiegeEngineKind::OrbitalKineticRelay => {
                let strike_r = (config.caliber_mm / 2500.0 * 2.2).clamp(1.5, 3.5);
                let strike = OrbitalBeamStrike {
                    beacon_pos: engine_pos,
                    target_pos: target,
                    radius: strike_r,
                    warmup_timer: 0.75, // Targeting countdown
                    strike_duration: 1.2,
                    max_duration: 1.2,
                    has_struck: false,
                    color: Color::new(0.3, 0.9, 1.0, 1.0),
                };
                self.orbital_strikes.push(strike);

                audio.play_spatial(audio.singularity_launch.as_ref(), engine_pos, camera_pos, 0.85);
                self.set_hud_message("ORBITAL KINETIC RELAY: TARGET BEACON ACQUIRED - ION BEAM INCOMING!");
                trauma += 0.20;
            }

            SiegeEngineKind::HydraulicGroundRam => {
                // Harmonic resonance escalation:
                // Rapid successive slams within 6.0s escalate resonance through 5 harmonic stages
                let eng = &mut self.deployed_engines[idx];
                if eng.last_slam_time >= 0.0 && eng.last_slam_time <= 6.0 {
                    eng.resonance_slam_count = (eng.resonance_slam_count + 1).min(5);
                } else {
                    eng.resonance_slam_count = 1;
                }
                eng.last_slam_time = 0.0;
                let res_count = eng.resonance_slam_count;
                let res_mult = match res_count {
                    1 => 1.0,
                    2 => 1.5,
                    3 => 2.2,
                    4 => 3.0,
                    _ => 4.0,
                };

                let pulse = SeismicGroundPulse {
                    origin: engine_pos,
                    prev_radius: 0.0,
                    current_radius: 0.5,
                    max_radius: (config.propellant_power * 16.0 * (1.0 + 0.15 * (res_mult - 1.0))).clamp(14.0, 42.0),
                    speed: 26.0 + 3.0 * (res_mult - 1.0),
                    power: config.propellant_power * 80.0,
                    resonance_level: res_count,
                    resonance_mult: res_mult,
                    life: 0.0,
                };
                self.seismic_pulses.push(pulse);

                // Local bedrock impact visuals scaled by resonance
                physics.add_colossal_siege_dust_plume(engine_pos + Vec3::Y * 0.5, 5.0 * res_mult.sqrt(), Color::new(0.65, 0.62, 0.58, 0.16));
                physics.add_ground_cratering_shockwave(engine_pos, 14.0 * res_mult.sqrt());
                physics.apply_radial_blast(engine_pos, 10.0 * res_mult.sqrt(), 65.0 * res_mult);

                // Perimeter spall fan radiating from bedrock slam point
                let ram_spall_count = (4.0 * res_mult) as usize;
                for r_i in 0..ram_spall_count {
                    let theta = (r_i as f32 / ram_spall_count as f32) * std::f32::consts::TAU;
                    let (s_sin, s_cos) = theta.sin_cos();
                    let p_pos = engine_pos + Vec3::new(s_cos, 0.25, s_sin) * 1.5;
                    let p_dir = Vec3::new(s_cos, 0.35, s_sin).normalize_or_zero();
                    physics.spawn_spall_impact(p_pos, p_dir, Color::new(0.85, 0.82, 0.78, 1.0));
                }

                audio.play_spatial(audio.hydraulic_slam.as_ref(), engine_pos, camera_pos, (0.95 * (1.0 + 0.08 * (res_mult - 1.0))).min(1.2));
                audio.play_spatial(audio.seismic_pulse.as_ref(), engine_pos, camera_pos, (0.90 * (1.0 + 0.08 * (res_mult - 1.0))).min(1.2));
                trauma += 0.40 * res_mult.min(2.5) * SoundSystem::calculate_distance_attenuation(engine_pos, camera_pos);
                self.set_hud_message(&format!("HYDRAULIC GROUND RAM: RESONANCE STAGE {} ({:.1}x POWER) DISCHARGED!", res_count, res_mult));
            }

            SiegeEngineKind::TeslaSiegePylon => {
                let max_arc_range = 42.0;
                let mut candidates: Vec<(Vec3, f32)> = Vec::new();
                let v_center = grid.world_to_voxel(engine_pos);
                let v_target = grid.world_to_voxel(target);

                // 1. Scan in focused volume around target, checking every vertical floor (y step 1)
                let t_r = (20.0 / VOXEL_SIZE).ceil() as i32;
                let min_x = (v_target.x - t_r).max(0);
                let max_x = (v_target.x + t_r).min(GRID_X as i32 - 1);
                let min_y = (v_target.y - t_r).max(MIN_VOXEL_Y);
                let max_y = (v_target.y + t_r).min(MAX_VOXEL_Y - 1);
                let min_z = (v_target.z - t_r).max(0);
                let max_z = (v_target.z + t_r).min(GRID_Z as i32 - 1);

                for x in min_x..=max_x {
                    for z in min_z..=max_z {
                        for y in min_y..=max_y {
                            let vt = grid.get(x, y, z);
                            let cond = vt.properties().conductivity;
                            if cond > 0.0 {
                                let wp = grid.voxel_to_world(IVec3::new(x, y, z));
                                if (wp - engine_pos).length_squared() <= max_arc_range * max_arc_range {
                                    candidates.push((wp, cond));
                                }
                            }
                        }
                    }
                }

                // 2. If no conductive voxels found near target, scan around pylon radius
                if candidates.is_empty() {
                    let p_r = (max_arc_range / VOXEL_SIZE).ceil() as i32;
                    let p_min_x = (v_center.x - p_r).max(0);
                    let p_max_x = (v_center.x + p_r).min(GRID_X as i32 - 1);
                    let p_min_y = (v_center.y - p_r).max(MIN_VOXEL_Y);
                    let p_max_y = (v_center.y + p_r).min(MAX_VOXEL_Y - 1);
                    let p_min_z = (v_center.z - p_r).max(0);
                    let p_max_z = (v_center.z + p_r).min(GRID_Z as i32 - 1);

                    for x in (p_min_x..=p_max_x).step_by(2) {
                        for z in (p_min_z..=p_max_z).step_by(2) {
                            for y in p_min_y..=p_max_y {
                                let vt = grid.get(x, y, z);
                                let cond = vt.properties().conductivity;
                                if cond > 0.0 {
                                    let wp = grid.voxel_to_world(IVec3::new(x, y, z));
                                    if (wp - engine_pos).length_squared() <= max_arc_range * max_arc_range {
                                        candidates.push((wp, cond));
                                    }
                                }
                            }
                        }
                    }
                }

                // Sort candidates biasing by conductivity: high conductivity and proximity to target
                candidates.sort_by(|a, b| {
                    let score_a = a.1 * 8.0 - a.0.distance(target);
                    let score_b = b.1 * 8.0 - b.0.distance(target);
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });
                candidates.truncate(10);

                if candidates.is_empty() {
                    candidates.push((target, 0.5));
                }

                let pylon_apex = engine_pos + Vec3::new(0.0, 4.2, 0.0);
                let mut arc_branches = Vec::new();

                // 1. Primary arc from pylon apex to closest target node
                let first_node = candidates[0].0;
                arc_branches.push((pylon_apex, first_node));

                let mut trig_exp = Vec::new();
                let mut trig_glow = Vec::new();
                let mut trig_cap = Vec::new();

                // 2. Cascading conduction arcs from node to node throughout the building network
                for i in 0..candidates.len() {
                    let (node_pos, _) = candidates[i];
                    let res = grid.carve_sphere_advanced(node_pos, 2.4, 240.0, DamageType::Explosive);
                    physics.spawn_spall_impact(node_pos, Vec3::Y, Color::new(0.5, 0.8, 1.0, 1.0));
                    trig_exp.extend(res.explosive_triggers);
                    trig_glow.extend(res.glow_core_triggers);
                    trig_cap.extend(res.capacitor_triggers);

                    // 3.5m electrical induction check for nearby volatiles
                    let v_node = grid.world_to_voxel(node_pos);
                    let ind_r = (3.5 / VOXEL_SIZE).ceil() as i32;
                    for ix in (v_node.x - ind_r)..=(v_node.x + ind_r) {
                        for iy in (v_node.y - ind_r)..=(v_node.y + ind_r) {
                            for iz in (v_node.z - ind_r)..=(v_node.z + ind_r) {
                                if grid.in_bounds(ix, iy, iz) {
                                    let ivt = grid.get(ix, iy, iz);
                                    if ivt == VoxelType::Explosive || ivt == VoxelType::GlowCore || ivt == VoxelType::PowerCapacitor {
                                        let iwp = grid.voxel_to_world(IVec3::new(ix, iy, iz));
                                        if (iwp - node_pos).length_squared() <= 3.5 * 3.5 {
                                            let v_coord = IVec3::new(ix, iy, iz);
                                            grid.damage_voxel(ix, iy, iz, 300.0, DamageType::Thermal);
                                            match ivt {
                                                VoxelType::Explosive => trig_exp.push(v_coord),
                                                VoxelType::GlowCore => trig_glow.push(v_coord),
                                                VoxelType::PowerCapacitor => trig_cap.push(v_coord),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if i + 1 < candidates.len() {
                        let (next_node, _) = candidates[i + 1];
                        arc_branches.push((node_pos, next_node));
                    }
                    if i % 3 == 0 && i > 0 {
                        arc_branches.push((pylon_apex, node_pos));
                    }
                }

                if !trig_exp.is_empty() || !trig_glow.is_empty() || !trig_cap.is_empty() {
                    crate::weapon::process_explosive_chain_with_capacitors(grid, physics, audio, trig_exp, trig_glow, trig_cap, camera_pos);
                }

                if arc_branches.len() < 2 {
                    arc_branches.push((first_node, first_node + Vec3::new(0.8, -1.2, 0.5)));
                }

                self.tesla_discharges.push(TeslaSiegeDischarge {
                    origin: pylon_apex,
                    branches: arc_branches,
                    life: 0.60,
                    max_life: 0.60,
                    color: Color::new(0.55, 0.75, 1.0, 1.0),
                });

                audio.play_spatial(audio.tesla_arc.as_ref(), pylon_apex, camera_pos, 0.90);
                audio.play_spatial(audio.plasma_buzz.as_ref(), pylon_apex, camera_pos, 0.75);
                trauma += 0.25;
                self.set_hud_message(&format!("TESLA SIEGE PYLON: DISCHARGED CASCADING ARCS INTO {} CONDUCTIVE NODES!", candidates.len()));
            }
        }

        trauma
    }

    /// Updates all deployed engines, shells, orbital strikes, and seismic waves
    pub fn update(
        &mut self,
        dt: f32,
        grid: &mut VoxelGrid,
        physics: &mut PhysicsSystem,
        audio: &SoundSystem,
        camera_pos: Vec3,
    ) -> f32 {
        let mut total_trauma = 0.0f32;

        // 1. Update HUD feedback message
        if let Some((_, ref mut timer)) = self.hud_feedback_msg {
            *timer -= dt;
            if *timer <= 0.0 {
                self.hud_feedback_msg = None;
            }
        }

        // 2. Update Deployed Engines
        for engine in &mut self.deployed_engines {
            engine.cooldown_timer = (engine.cooldown_timer - dt).max(0.0);
            engine.muzzle_flash_timer = (engine.muzzle_flash_timer - dt).max(0.0);
            engine.last_slam_time += dt;
            if engine.last_slam_time > 6.0 {
                engine.resonance_slam_count = 0;
            }
            engine.anim_phase += dt * match engine.kind {
                SiegeEngineKind::HydraulicGroundRam => 2.5,
                SiegeEngineKind::TeslaSiegePylon => 4.0,
                SiegeEngineKind::OrbitalKineticRelay => 1.8,
                SiegeEngineKind::RailArtillery => 1.0,
            };
        }

        // 3. Update Colossal Shells
        let gravity = Vec3::new(0.0, -18.0, 0.0);
        let mut i = 0;
        while i < self.colossal_shells.len() {
            let shell = &mut self.colossal_shells[i];
            shell.life += dt;
            shell.velocity += gravity * dt;

            // Delayed fuse countdown
            if shell.is_fuse_active {
                shell.fuse_timer -= dt;
            }

            let move_delta = shell.velocity * dt;
            let total_dist = move_delta.length();
            let mut detonate_now = false;
            let mut det_pos = shell.position;

            let step_size = (shell.radius * 0.5).max(VOXEL_SIZE * 0.8);
            let num_substeps = if total_dist > 0.001 {
                ((total_dist / step_size).ceil() as usize).clamp(1, 16)
            } else {
                1
            };

            for _ in 0..num_substeps {
                if detonate_now {
                    break;
                }

                let sub_move = move_delta / num_substeps as f32;
                let sub_dist = sub_move.length();
                if sub_dist < 0.0001 {
                    continue;
                }
                let sub_dir = sub_move / sub_dist;
                let sub_next = shell.position + sub_move;

                let mut hit_point: Option<Vec3> = None;
                let mut hit_bedrock = false;

                if let Some(hit) = grid.raycast(shell.position, sub_dir, sub_dist + shell.radius) {
                    hit_point = Some(hit.world_pos);
                    if hit.voxel_type == VoxelType::Bedrock {
                        hit_bedrock = true;
                    }
                }
                if let Some((chunk_p, _)) = physics.check_chunk_hit_radius(shell.position, sub_next, shell.radius) {
                    if hit_point.is_none_or(|pos| shell.position.distance_squared(chunk_p) < shell.position.distance_squared(pos)) {
                        hit_point = Some(chunk_p);
                        hit_bedrock = false;
                    }
                }
                if let Some(ground_hit) = crate::weapon::surrounding_foundation_hit(shell.position, sub_next) {
                    if hit_point.is_none_or(|pos| shell.position.distance_squared(ground_hit) < shell.position.distance_squared(pos)) {
                        hit_point = Some(ground_hit);
                        hit_bedrock = true;
                    }
                }

                if let Some(hp) = hit_point {
                    if hit_bedrock {
                        det_pos = hp;
                        detonate_now = true;
                        break;
                    } else if shell.payload == SiegePayload::BunkerBuster {
                        // Bunker-buster: carve punch-through bore cavity
                        let bore_dist = (shell.radius * 2.0).max(1.2);
                        let bore_end = hp + sub_dir * bore_dist;
                        let bore_res = grid.carve_cylinder_advanced(hp, bore_end, shell.radius * 1.15, 280.0, DamageType::Kinetic);
                        physics.spawn_spall_impact(bore_end, sub_dir, shell.color);
                        physics.add_flying_rubble_shower(bore_end, shell.radius * 1.2, 40.0, bore_res.destroyed.clamp(10, 40));
                        physics.apply_radial_blast(hp, shell.radius * 2.0, 40.0);

                        if !bore_res.explosive_triggers.is_empty() || !bore_res.glow_core_triggers.is_empty() || !bore_res.capacitor_triggers.is_empty() {
                            crate::weapon::process_explosive_chain_with_capacitors(
                                grid,
                                physics,
                                audio,
                                bore_res.explosive_triggers,
                                bore_res.glow_core_triggers,
                                bore_res.capacitor_triggers,
                                camera_pos,
                            );
                        }

                        shell.penetrations_remaining = shell.penetrations_remaining.saturating_sub(1);
                        shell.is_fuse_active = true;
                        shell.velocity *= 0.88;

                        // Advance shell position past pierced obstacle
                        shell.position = hp + sub_dir * (VOXEL_SIZE * 1.1);

                        if shell.penetrations_remaining == 0 || shell.fuse_timer <= 0.0 {
                            det_pos = hp;
                            detonate_now = true;
                            break;
                        }
                    } else {
                        det_pos = hp;
                        detonate_now = true;
                        break;
                    }
                } else {
                    shell.position = sub_next;
                }

                if shell.is_fuse_active && shell.fuse_timer <= 0.0 {
                    det_pos = shell.position;
                    detonate_now = true;
                    break;
                }

            }

            if shell.life >= shell.max_life {
                det_pos = shell.position;
                detonate_now = true;
            }

            // Record trail points for high-energy smoke/spark ribbon
            shell.trail_points.push(shell.position);

            if detonate_now {
                let blast_r = shell.blast_radius;
                let payload = shell.payload;

                // Execute payload specific destruction
                let (trig_exp, trig_glow, trig_cap) = match payload {
                    SiegePayload::ColossalHighExplosive => {
                        let res = grid.carve_sphere_advanced(det_pos, blast_r, 340.0, DamageType::Explosive);
                        physics.add_colossal_siege_dust_plume(det_pos, blast_r * 1.2, Color::new(0.72, 0.68, 0.62, 0.16));
                        physics.add_ground_cratering_shockwave(det_pos, blast_r * 2.8);
                        physics.add_flying_rubble_shower(det_pos, blast_r, 45.0, res.destroyed.clamp(35, 350));
                        // Colossal secondary spall shower along crater perimeter
                        let spall_points = 8;
                        for sp_i in 0..spall_points {
                            let ang = (sp_i as f32 / spall_points as f32) * std::f32::consts::TAU;
                            let (s_sin, s_cos) = ang.sin_cos();
                            let spall_pos = det_pos + Vec3::new(s_cos, 0.25, s_sin) * (blast_r * 0.85);
                            let out_dir = Vec3::new(s_cos, 0.45, s_sin).normalize_or_zero();
                            physics.spawn_spall_impact(spall_pos, out_dir, Color::new(0.92, 0.88, 0.80, 1.0));
                        }
                        physics.apply_radial_blast(det_pos, blast_r * 1.5, 80.0);
                        audio.play_spatial(audio.big_boom.as_ref(), det_pos, camera_pos, 0.95);
                        (res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers)
                    }
                    SiegePayload::BunkerBuster => {
                        // Interior high-pressure explosion blowing walls outward
                        let res = grid.carve_sphere_advanced(det_pos, blast_r, 9000.0, DamageType::Kinetic);
                        physics.add_colossal_siege_dust_plume(det_pos, blast_r, Color::new(0.60, 0.58, 0.55, 0.16));
                        physics.add_ground_cratering_shockwave(det_pos, blast_r * 1.8);
                        physics.add_flying_rubble_shower(det_pos, blast_r, 60.0, res.destroyed.clamp(40, 140));
                        physics.apply_radial_blast(det_pos, blast_r * 1.8, 95.0);
                        audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), det_pos, camera_pos, 0.95);
                        audio.play_spatial(audio.big_boom.as_ref(), det_pos, camera_pos, 0.90);
                        (res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers)
                    }
                    SiegePayload::ThermiteMelt => {
                        let res = grid.carve_sphere_advanced(det_pos, blast_r, 400.0, DamageType::Thermal);
                        physics.add_colossal_siege_dust_plume(det_pos, blast_r, Color::new(0.85, 0.45, 0.20, 0.16));
                        physics.spawn_molten_fissure(det_pos, Vec3::Y, 30);
                        physics.apply_radial_blast(det_pos, blast_r * 1.2, 50.0);
                        audio.play_spatial(audio.plasma_fire.as_ref(), det_pos, camera_pos, 0.85);
                        audio.play_spatial(audio.big_boom.as_ref(), det_pos, camera_pos, 0.80);
                        (res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers)
                    }
                    SiegePayload::CryoShatter => {
                        // 1. Flash freeze sphere
                        let frozen = grid.freeze_sphere(det_pos, blast_r);
                        // 2. Concussive shatter
                        let res = grid.carve_sphere_advanced(det_pos, blast_r * 1.15, 260.0, DamageType::Cryo);
                        physics.spawn_ice_shatter_effects(det_pos, blast_r, (frozen * 2).clamp(25, 60));
                        physics.add_ground_cratering_shockwave(det_pos, blast_r * 1.6);
                        audio.play_spatial(audio.cryo_freeze.as_ref(), det_pos, camera_pos, 0.90);
                        audio.play_spatial(audio.cryo_shatter.as_ref(), det_pos, camera_pos, 0.95);
                        (res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers)
                    }
                    SiegePayload::AntimatterSingularity => {
                        // Singularity vortex: implosion followed by colossal blowout
                        let pull_r = blast_r * 1.8;
                        physics.apply_vortex_suction(det_pos, pull_r, 70.0, 1.2);
                        let res = grid.carve_sphere_advanced(det_pos, blast_r, 500.0, DamageType::Explosive);
                        physics.add_colossal_siege_dust_plume(det_pos, blast_r * 1.4, Color::new(0.35, 0.15, 0.45, 0.16));
                        physics.add_ground_cratering_shockwave(det_pos, blast_r * 2.5);
                        physics.apply_radial_blast(det_pos, blast_r * 2.0, 110.0);
                        audio.play_spatial(audio.singularity_implode.as_ref(), det_pos, camera_pos, 0.95);
                        audio.play_spatial(audio.big_boom.as_ref(), det_pos, camera_pos, 0.90);
                        (res.explosive_triggers, res.glow_core_triggers, res.capacitor_triggers)
                    }
                };

                physics.update_crater_floor(grid, det_pos, blast_r);

                if !trig_exp.is_empty() || !trig_glow.is_empty() || !trig_cap.is_empty() {
                    crate::weapon::process_explosive_chain_with_capacitors(grid, physics, audio, trig_exp, trig_glow, trig_cap, camera_pos);
                }

                let dist_atten = SoundSystem::calculate_distance_attenuation(det_pos, camera_pos);
                total_trauma += 0.45 * dist_atten;

                self.colossal_shells.swap_remove(i);
                continue;
            }

            i += 1;
        }

        // 4. Update Orbital Strikes (3m vertical beam carving all stories)
        let mut os_i = 0;
        while os_i < self.orbital_strikes.len() {
            let strike = &mut self.orbital_strikes[os_i];
            if strike.warmup_timer > 0.0 {
                strike.warmup_timer -= dt;
                // Targeting laser smoke / spark wisps at target
                if rand::gen_range(0.0, 1.0) < 0.35 {
                    physics.spawn_spall_impact(strike.target_pos, Vec3::Y, strike.color);
                }
            } else {
                if !strike.has_struck {
                    strike.has_struck = true;

                    // 1. Multi-Floor Stratospheric Descent
                    // Searing vertical cylinder from stratospheric zenith (y = 140.0) down through
                    // all skyscraper stories, road decks, and into subterranean bedrock (y = -16.0)
                    let beam_top = Vec3::new(strike.target_pos.x, 140.0, strike.target_pos.z);
                    let beam_bot = Vec3::new(strike.target_pos.x, -16.0, strike.target_pos.z);
                    let bedrock_pos = Vec3::new(strike.target_pos.x, 0.0, strike.target_pos.z);
                    let crater_center = Vec3::new(strike.target_pos.x, -6.0, strike.target_pos.z);

                    // Dual carving: hypersonic kinetic vertical bore + ionization thermal sheath through all floors
                    let res_core = grid.carve_cylinder_advanced(beam_top, beam_bot, strike.radius, 10000.0, DamageType::Kinetic);
                    let res_sheath = grid.carve_cylinder_advanced(beam_top, beam_bot, strike.radius * 1.5, 4800.0, DamageType::Thermal);

                    // 2. 16-Meter Bedrock Crater Excavation (R = 8.0m, depth to -16.0m)
                    // Spherical excavation centered at Y = -6.0m with R = 10.0m:
                    // Yields exact horizontal R = sqrt(10^2 - 6^2) = 8.0m (16 voxels radius, 16m diameter) at Y = 0.0m
                    // and penetrates basalt substrate down to Y = -16.0m (-32 voxels).
                    let res_crater = grid.carve_sphere_advanced(crater_center, 10.0, 36000.0, DamageType::Kinetic);
                    let res_crater_therm = grid.carve_sphere_advanced(crater_center, 10.5, 8000.0, DamageType::Thermal);

                    // 3. Dynamic Rubble Heightmap Excavation & Chunk Un-Settling
                    // Synchronize rubble heightmap with the excavated crater floor (covers full 16m diameter)
                    physics.update_crater_floor(grid, bedrock_pos, 8.5);
                    physics.un_settle_rubble_crater(bedrock_pos, 8.5, 65.0);
                    physics.apply_radial_blast(bedrock_pos, 12.0, 110.0);

                    // 4. Secondary Reactive Explosions & Chain Reactions
                    let mut trig_exp = res_core.explosive_triggers;
                    let mut trig_glow = res_core.glow_core_triggers;
                    let mut trig_cap = res_core.capacitor_triggers;
                    trig_exp.extend(res_sheath.explosive_triggers);
                    trig_glow.extend(res_sheath.glow_core_triggers);
                    trig_cap.extend(res_sheath.capacitor_triggers);
                    trig_exp.extend(res_crater.explosive_triggers);
                    trig_glow.extend(res_crater.glow_core_triggers);
                    trig_cap.extend(res_crater.capacitor_triggers);
                    trig_exp.extend(res_crater_therm.explosive_triggers);
                    trig_glow.extend(res_crater_therm.glow_core_triggers);
                    trig_cap.extend(res_crater_therm.capacitor_triggers);

                    if !trig_exp.is_empty() || !trig_glow.is_empty() || !trig_cap.is_empty() {
                        crate::weapon::process_explosive_chain_with_capacitors(grid, physics, audio, trig_exp, trig_glow, trig_cap, camera_pos);
                    }

                    // 5. Structural Cleaving Beam Shockwave
                    physics.apply_beam_shockwave(beam_top, beam_bot, strike.radius * 3.5, 80.0);
                    physics.add_colossal_siege_dust_plume(strike.target_pos, 8.0, Color::new(0.3, 0.85, 1.0, 0.16));
                    physics.add_colossal_siege_dust_plume(bedrock_pos, 12.0, Color::new(0.7, 0.65, 0.60, 0.25));

                    // 6. Colossal Ground Shockwave Rings
                    // Preserves 28m invariant for existing tests and adds 36m surface ground ring
                    physics.add_ground_cratering_shockwave(beam_bot, 28.0);
                    physics.add_ground_cratering_shockwave(bedrock_pos, 36.0);
                    physics.spawn_molten_fissure(bedrock_pos, Vec3::Y, 45);

                    // 7. Multi-Story Vertical Column Spall Bursts
                    let mut col_y = 140.0;
                    while col_y >= -16.0 {
                        let col_pos = Vec3::new(strike.target_pos.x, col_y, strike.target_pos.z);
                        let spall_dirs = [
                            Vec3::new(1.0, 0.15, 0.0),
                            Vec3::new(-1.0, 0.15, 0.0),
                            Vec3::new(0.0, 0.15, 1.0),
                            Vec3::new(0.0, 0.15, -1.0),
                        ];
                        for &dir in &spall_dirs {
                            physics.spawn_spall_impact(col_pos + dir * (strike.radius * 0.9), dir, strike.color);
                        }
                        col_y -= 10.0;
                    }

                    // 8. Incandescent Ejecta Particles & Flying Basalt/Granite Rubble Shower
                    // Upward-erupting incandescent sparks in an inverted cone from crater floor
                    for _ in 0..100 {
                        let ang = rand::gen_range(0.0, std::f32::consts::TAU);
                        let dist = rand::gen_range(0.5, 7.5);
                        let pos = Vec3::new(strike.target_pos.x + ang.cos() * dist, -6.0, strike.target_pos.z + ang.sin() * dist);
                        let ejecta_dir = Vec3::new(
                            ang.cos() * rand::gen_range(0.3, 1.2),
                            rand::gen_range(1.5, 3.5),
                            ang.sin() * rand::gen_range(0.3, 1.2),
                        ).normalize();
                        let speed = rand::gen_range(22.0, 58.0);
                        physics.sparks.push(crate::physics::SparkParticle {
                            position: pos,
                            velocity: ejecta_dir * speed,
                            color: Color::new(1.0, rand::gen_range(0.70, 0.95), rand::gen_range(0.20, 0.50), 1.0),
                            life: rand::gen_range(1.2, 2.5),
                            max_life: 2.5,
                        });
                    }

                    // Dense flying rubble shower of pulverized granite and basalt chunks
                    physics.add_flying_rubble_shower(bedrock_pos, 8.0, 85.0, 300);
                    let total_destroyed = res_core.destroyed + res_sheath.destroyed + res_crater.destroyed;
                    physics.add_flying_rubble_shower(strike.target_pos, 6.0, 50.0, total_destroyed.clamp(50, 250));

                    // 9. Sound & Violent Camera Trauma
                    audio.play_spatial(audio.railgun_boom.as_ref(), strike.target_pos, camera_pos, 0.95);
                    audio.play_spatial(audio.singularity_implode.as_ref(), bedrock_pos, camera_pos, 0.95);
                    audio.play_spatial(audio.subsonic_artillery_boom.as_ref(), bedrock_pos, camera_pos, 0.90);

                    let dist_atten = SoundSystem::calculate_distance_attenuation(bedrock_pos, camera_pos);
                    total_trauma += (0.90 * dist_atten).max(0.30);
                }

                strike.strike_duration -= dt;
                if strike.strike_duration <= 0.0 {
                    self.orbital_strikes.swap_remove(os_i);
                    continue;
                }
            }
            os_i += 1;
        }

        // 5. Update Seismic Ground Pulses (crushing foundation columns)
        let mut sp_i = 0;
        while sp_i < self.seismic_pulses.len() {
            let pulse = &mut self.seismic_pulses[sp_i];
            pulse.life += dt;
            let prev_r = pulse.current_radius;
            pulse.current_radius += pulse.speed * dt;
            pulse.prev_radius = prev_r;
            let curr_r = pulse.current_radius;

            // Damage foundation voxels along the expanding circular wavefront between prev_r and curr_r
            let v_origin = grid.world_to_voxel(pulse.origin);
            let prev_v_r = (prev_r / VOXEL_SIZE).floor() as i32;
            let curr_v_r = (curr_r / VOXEL_SIZE).ceil() as i32;

            let mut trig_exp = Vec::new();
            let mut trig_glow = Vec::new();
            let mut trig_cap = Vec::new();

            for vr in prev_v_r.max(1)..=curr_v_r {
                if vr > (pulse.max_radius / VOXEL_SIZE) as i32 {
                    continue;
                }
                let segments = (std::f32::consts::TAU * vr as f32).ceil().max(24.0) as usize;
                for s in 0..segments {
                    let theta = (s as f32 / segments as f32) * std::f32::consts::TAU;
                    let vx = v_origin.x + (theta.cos() * vr as f32).round() as i32;
                    let vz = v_origin.z + (theta.sin() * vr as f32).round() as i32;
                    // Lower 4 stories: y = 1..=4
                    for vy in 1..=4 {
                        if grid.in_bounds(vx, vy, vz) {
                            let vt = grid.get(vx, vy, vz);
                            if vt.is_destructible() {
                                let v_wpos = grid.voxel_to_world(IVec3::new(vx, vy, vz));
                                let dist = Vec2::new(v_wpos.x - pulse.origin.x, v_wpos.z - pulse.origin.z).length();
                                let falloff = (1.0 - (dist / pulse.max_radius)).max(0.15);
                                let damage = (pulse.power * pulse.resonance_mult * falloff * dt * 2.5).max(15.0 * pulse.resonance_mult);
                                let destroyed = grid.damage_voxel(vx, vy, vz, damage, DamageType::Concussive);
                                if destroyed {
                                    let v_coord = IVec3::new(vx, vy, vz);
                                    match vt {
                                        VoxelType::Explosive => trig_exp.push(v_coord),
                                        VoxelType::GlowCore => trig_glow.push(v_coord),
                                        VoxelType::PowerCapacitor => trig_cap.push(v_coord),
                                        _ => {}
                                    }
                                    let out_dir = Vec3::new(v_wpos.x - pulse.origin.x, 0.35, v_wpos.z - pulse.origin.z).normalize_or_zero();
                                    // Perimeter spall fan for shattered foundation columns
                                    physics.spawn_spall_impact(v_wpos, out_dir, vt.color());
                                    if pulse.resonance_mult >= 2.0 {
                                        let fan_dir = Vec3::new(out_dir.z, 0.20, -out_dir.x).normalize_or_zero();
                                        physics.spawn_spall_impact(v_wpos + out_dir * 0.15, (out_dir + fan_dir * 0.45).normalize_or_zero(), vt.color());
                                        physics.spawn_spall_impact(v_wpos + out_dir * 0.15, (out_dir - fan_dir * 0.45).normalize_or_zero(), vt.color());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !trig_exp.is_empty() || !trig_glow.is_empty() || !trig_cap.is_empty() {
                crate::weapon::process_explosive_chain_with_capacitors(grid, physics, audio, trig_exp, trig_glow, trig_cap, camera_pos);
            }

            // Radial physical blast pushing dynamic rubble chunks outward
            physics.apply_radial_blast(pulse.origin, curr_r, 45.0 * dt * pulse.resonance_mult);

            if pulse.current_radius >= pulse.max_radius {
                self.seismic_pulses.swap_remove(sp_i);
                continue;
            }
            sp_i += 1;
        }

        // 6. Update Tesla Discharges
        let mut td_i = 0;
        while td_i < self.tesla_discharges.len() {
            self.tesla_discharges[td_i].life -= dt;
            if self.tesla_discharges[td_i].life <= 0.0 {
                self.tesla_discharges.swap_remove(td_i);
                continue;
            }
            td_i += 1;
        }

        total_trauma.clamp(0.0, 0.70)
    }

    /// Collects radiant dynamic point lights from deployed engines, shells, and orbital beams
    pub fn collect_dynamic_lights(&self, lights: &mut Vec<DynamicPointLight>) {
        // Deployed engines muzzle flashes & charging cores
        for engine in &self.deployed_engines {
            if engine.muzzle_flash_timer > 0.0 {
                let flash_a = (engine.muzzle_flash_timer / 0.25).clamp(0.0, 1.0);
                lights.push(DynamicPointLight {
                    position: engine.position + Vec3::new(0.0, 2.5, 0.0),
                    color: engine.kind.theme_color(),
                    radius: 18.0,
                    intensity: flash_a * 2.2,
                });
            } else {
                // Subtle standby indicator glow
                lights.push(DynamicPointLight {
                    position: engine.position + Vec3::new(0.0, 1.5, 0.0),
                    color: engine.kind.theme_color(),
                    radius: 4.5,
                    intensity: 0.35,
                });
            }
        }

        // Colossal shells in flight
        for shell in &self.colossal_shells {
            lights.push(DynamicPointLight {
                position: shell.position,
                color: shell.color,
                radius: 14.0,
                intensity: 1.8,
            });
        }

        // Orbital beam ionization column & subterranean bedrock crater dynamic lights
        for strike in &self.orbital_strikes {
            if strike.has_struck {
                // 1. Upper atmospheric / building ionization column light
                lights.push(DynamicPointLight {
                    position: strike.target_pos + Vec3::new(0.0, 8.0, 0.0),
                    color: strike.color,
                    radius: 28.0,
                    intensity: 3.5,
                });

                // 2. Subterranean bedrock crater incandescent illumination
                lights.push(DynamicPointLight {
                    position: Vec3::new(strike.target_pos.x, -2.0, strike.target_pos.z),
                    color: strike.color,
                    radius: 36.0,
                    intensity: 4.5,
                });
            }
        }
    }

    /// Renders 3D procedural models of all deployed siege engines in the world
    pub fn draw_world_engines(&self, cam_pos: Vec3) {
        for engine in &self.deployed_engines {
            let dist_sq = (engine.position - cam_pos).length_squared();
            if dist_sq > 250.0 * 250.0 {
                continue;
            }

            let pos = engine.position;
            let yaw = engine.facing_yaw;
            let theme = engine.kind.theme_color();

            match engine.kind {
                SiegeEngineKind::RailArtillery => {
                    // 1. Bedrock mounting plate & outriggers
                    draw_cube(pos + Vec3::new(0.0, 0.25, 0.0), Vec3::new(4.2, 0.5, 4.2), None, Color::new(0.12, 0.14, 0.18, 1.0));
                    draw_cube_wires(pos + Vec3::new(0.0, 0.25, 0.0), Vec3::new(4.2, 0.5, 4.2), Color::new(0.3, 0.4, 0.5, 1.0));

                    // Outriggers with hazard yellow tips
                    let outriggers = [
                        Vec3::new(2.5, 0.2, 2.5),
                        Vec3::new(-2.5, 0.2, 2.5),
                        Vec3::new(2.5, 0.2, -2.5),
                        Vec3::new(-2.5, 0.2, -2.5),
                    ];
                    for o in &outriggers {
                        draw_cube(pos + *o, Vec3::new(1.0, 0.35, 1.0), None, Color::new(0.9, 0.7, 0.1, 1.0));
                    }

                    // 2. Turret pedestal
                    draw_cube(pos + Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.8, 1.0, 2.8), None, Color::new(0.18, 0.20, 0.25, 1.0));

                    // 3. Twin electromagnetic rail cannon barrels
                    let fw = Vec3::new(yaw.cos(), 0.0, yaw.sin());
                    let right = fw.cross(Vec3::Y).normalize_or_zero();
                    let elev_rad = engine.config.elevation_deg.to_radians();
                    let barrel_dir = (fw * elev_rad.cos() + Vec3::Y * elev_rad.sin()).normalize_or_zero();

                    let barrel_len = 5.2;
                    let rail_offset = 0.55;
                    let b_base = pos + Vec3::new(0.0, 2.0, 0.0);

                    // Left and right rails: draw stepped segments and 3D guide rails oriented along barrel_dir
                    let rail_segs = 8;
                    for s in 0..=rail_segs {
                        let t = (s as f32) / (rail_segs as f32) * barrel_len;
                        let p_l = b_base + right * rail_offset + barrel_dir * t;
                        let p_r = b_base - right * rail_offset + barrel_dir * t;
                        draw_cube(p_l, Vec3::splat(0.38), None, Color::new(0.25, 0.28, 0.35, 1.0));
                        draw_cube(p_r, Vec3::splat(0.38), None, Color::new(0.25, 0.28, 0.35, 1.0));
                    }
                    let l_start = b_base + right * rail_offset;
                    let l_end = l_start + barrel_dir * barrel_len;
                    let r_start = b_base - right * rail_offset;
                    let r_end = r_start + barrel_dir * barrel_len;
                    draw_line_3d(l_start, l_end, Color::new(0.4, 0.45, 0.55, 1.0));
                    draw_line_3d(r_start, r_end, Color::new(0.4, 0.45, 0.55, 1.0));

                    // Glowing capacitor coils between the rails
                    let coil_count = 5;
                    for c in 0..coil_count {
                        let t = (c as f32 + 1.0) / (coil_count as f32 + 1.0);
                        let cp = b_base + barrel_dir * (barrel_len * t);
                        draw_sphere(cp, 0.32, None, theme);
                    }

                    // Muzzle flash burst
                    if engine.muzzle_flash_timer > 0.0 {
                        let muzzle = pos + Vec3::new(0.0, 2.0, 0.0) + barrel_dir * barrel_len;
                        draw_sphere(muzzle, 1.4, None, Color::new(1.0, 0.8, 0.2, 0.95));
                        draw_sphere(muzzle, 0.7, None, WHITE);
                    }
                }

                SiegeEngineKind::OrbitalKineticRelay => {
                    // Uplink base
                    draw_cube(pos + Vec3::new(0.0, 0.3, 0.0), Vec3::new(3.5, 0.6, 3.5), None, Color::new(0.10, 0.15, 0.22, 1.0));
                    draw_cube_wires(pos + Vec3::new(0.0, 0.3, 0.0), Vec3::new(3.5, 0.6, 3.5), theme);

                    // Central beacon pillar
                    draw_cube(pos + Vec3::new(0.0, 1.8, 0.0), Vec3::new(1.2, 2.4, 1.2), None, Color::new(0.15, 0.22, 0.30, 1.0));

                    // Pulsing target dish / core
                    let pulse = (engine.anim_phase * 2.5).sin() * 0.2 + 0.8;
                    draw_sphere(pos + Vec3::new(0.0, 3.4, 0.0), 0.75 * pulse, None, theme);
                    draw_sphere(pos + Vec3::new(0.0, 3.4, 0.0), 0.38, None, WHITE);

                    // Vertical laser pointer into stratosphere
                    let beam_top = pos + Vec3::new(0.0, 80.0, 0.0);
                    draw_line_3d(pos + Vec3::new(0.0, 3.4, 0.0), beam_top, Color::new(theme.r, theme.g, theme.b, 0.45));
                }

                SiegeEngineKind::HydraulicGroundRam => {
                    // Massive triangular tripod frame
                    draw_cube(pos + Vec3::new(0.0, 0.35, 0.0), Vec3::new(4.5, 0.7, 4.5), None, Color::new(0.18, 0.16, 0.14, 1.0));
                    draw_cube_wires(pos + Vec3::new(0.0, 0.35, 0.0), Vec3::new(4.5, 0.7, 4.5), Color::new(0.8, 0.5, 0.1, 1.0));

                    // Gantry pillars
                    draw_cube(pos + Vec3::new(1.5, 2.2, 0.0), Vec3::new(0.4, 3.2, 0.4), None, Color::new(0.25, 0.23, 0.20, 1.0));
                    draw_cube(pos + Vec3::new(-1.5, 2.2, 0.0), Vec3::new(0.4, 3.2, 0.4), None, Color::new(0.25, 0.23, 0.20, 1.0));

                    // Central heavy hydraulic cylinder
                    draw_cube(pos + Vec3::new(0.0, 2.8, 0.0), Vec3::new(1.4, 2.0, 1.4), None, Color::new(0.35, 0.30, 0.25, 1.0));

                    // Moving piston hammer
                    let piston_y = 1.0 + (engine.anim_phase.sin() * 0.5 + 0.5) * 1.2;
                    draw_cube(pos + Vec3::new(0.0, piston_y, 0.0), Vec3::new(1.6, 0.8, 1.6), None, Color::new(0.85, 0.65, 0.15, 1.0));
                    draw_cube(pos + Vec3::new(0.0, piston_y + 0.8, 0.0), Vec3::new(0.7, 1.4, 0.7), None, Color::new(0.7, 0.7, 0.7, 1.0));
                }

                SiegeEngineKind::TeslaSiegePylon => {
                    // Stepped pyramid base
                    draw_cube(pos + Vec3::new(0.0, 0.3, 0.0), Vec3::new(3.8, 0.6, 3.8), None, Color::new(0.14, 0.12, 0.18, 1.0));
                    draw_cube(pos + Vec3::new(0.0, 0.8, 0.0), Vec3::new(2.8, 0.5, 2.8), None, Color::new(0.18, 0.15, 0.24, 1.0));

                    // Insulated ceramic column
                    draw_cube(pos + Vec3::new(0.0, 2.2, 0.0), Vec3::new(0.9, 2.4, 0.9), None, Color::new(0.25, 0.22, 0.35, 1.0));

                    // Massive toroidal aluminum / copper capacitor ring
                    let toroid_pos = pos + Vec3::new(0.0, 3.8, 0.0);
                    draw_sphere(toroid_pos, 1.2, None, Color::new(0.65, 0.45, 1.0, 0.85));
                    draw_sphere(toroid_pos, 0.6, None, WHITE);

                    // Static electrical sparks dancing around toroid
                    for s in 0..6 {
                        let angle = engine.anim_phase * 3.0 + s as f32 * 1.05;
                        let spark_p = toroid_pos + Vec3::new(angle.cos() * 1.6, (angle * 2.0).sin() * 0.4, angle.sin() * 1.6);
                        draw_sphere(spark_p, 0.14, None, Color::new(0.8, 0.9, 1.0, 0.95));
                    }
                }
            }
        }
    }

    /// Renders active colossal shells, orbital beams, seismic waves, and tesla arcs in 3D
    pub fn draw_colossal_projectiles_and_beams(&self, _cam_pos: Vec3) {
        // 1. Colossal Dreadnought Shells
        for shell in &self.colossal_shells {
            // High-luminosity shell core
            draw_sphere(shell.position, shell.radius, None, shell.color);
            draw_sphere(shell.position, shell.radius * 0.55, None, WHITE);

            // Exhaust blast behind shell
            let back_dir = -shell.velocity.normalize_or_zero();
            let flame_p = shell.position + back_dir * (shell.radius * 0.85);
            draw_sphere(flame_p, shell.radius * 0.90, None, Color::new(1.0, 0.6, 0.15, 0.95));

            // High-energy ribbon trail
            if shell.trail_points.len() >= 2 {
                for i in 0..shell.trail_points.len() - 1 {
                    let p1 = shell.trail_points[i];
                    let p2 = shell.trail_points[i + 1];
                    let frac = (i + 1) as f32 / shell.trail_points.len() as f32;
                    let r_col = Color::new(shell.color.r, shell.color.g, shell.color.b, frac * 0.85);
                    draw_line_3d(p1, p2, r_col);
                    draw_line_3d(p1 + Vec3::new(0.08, 0.08, 0.0), p2 + Vec3::new(0.08, 0.08, 0.0), r_col);
                    draw_line_3d(p1 - Vec3::new(0.08, 0.08, 0.0), p2 - Vec3::new(0.08, 0.08, 0.0), r_col);
                }
            }
        }

        // 2. Orbital Kinetic Strike Beams
        for strike in &self.orbital_strikes {
            if strike.warmup_timer > 0.0 {
                // Targeting laser beam
                let t_alpha = (1.0 - (strike.warmup_timer / 0.75)).clamp(0.2, 1.0);
                draw_line_3d(
                    Vec3::new(strike.target_pos.x, 140.0, strike.target_pos.z),
                    strike.target_pos,
                    Color::new(strike.color.r, strike.color.g, strike.color.b, t_alpha * 0.75),
                );
                // Reticle ring on ground
                draw_sphere(strike.target_pos, 1.2 * t_alpha, None, Color::new(1.0, 0.3, 0.2, 0.8));
            } else {
                // Searing vertical pillar of light plunging from zenith into bedrock crater floor!
                let top = Vec3::new(strike.target_pos.x, 140.0, strike.target_pos.z);
                let bot = Vec3::new(strike.target_pos.x, -16.0, strike.target_pos.z);
                let surface = Vec3::new(strike.target_pos.x, 0.50, strike.target_pos.z);

                // Central blinding white core line
                draw_line_3d(top, bot, WHITE);

                // Wide radiant ion cylinder lines
                let segs = 12;
                for s in 0..segs {
                    let theta = (s as f32 / segs as f32) * std::f32::consts::TAU;
                    let offset = Vec3::new(theta.cos() * strike.radius, 0.0, theta.sin() * strike.radius);
                    draw_line_3d(top + offset, bot + offset, strike.color);
                    draw_line_3d(top + offset * 0.6, bot + offset * 0.6, Color::new(1.0, 1.0, 1.0, 0.85));
                }

                // Subterranean crater floor splash
                draw_sphere(bot, strike.radius * 2.5, None, strike.color);
                draw_sphere(bot, strike.radius * 1.2, None, WHITE);

                // Surface crater rim radiant ionization splash
                draw_sphere(surface, strike.radius * 2.0, None, strike.color);
            }
        }

        // 3. Seismic Pulses (Ground Shockwaves)
        for pulse in &self.seismic_pulses {
            let segs = 32;
            let y = 0.55;
            for s in 0..segs {
                let theta1 = (s as f32 / segs as f32) * std::f32::consts::TAU;
                let theta2 = ((s + 1) as f32 / segs as f32) * std::f32::consts::TAU;
                let p1 = pulse.origin + Vec3::new(theta1.cos() * pulse.current_radius, y - pulse.origin.y, theta1.sin() * pulse.current_radius);
                let p2 = pulse.origin + Vec3::new(theta2.cos() * pulse.current_radius, y - pulse.origin.y, theta2.sin() * pulse.current_radius);
                let alpha = (1.0 - (pulse.current_radius / pulse.max_radius)).clamp(0.0, 1.0);
                draw_line_3d(p1, p2, Color::new(1.0, 0.75, 0.2, alpha * 0.95));
            }
        }

        // 4. Tesla Discharges
        for td in &self.tesla_discharges {
            let alpha = (td.life / td.max_life).clamp(0.0, 1.0);
            let col = Color::new(td.color.r, td.color.g, td.color.b, alpha * 0.9);
            for &(p1, p2) in &td.branches {
                draw_line_3d(p1, p2, col);
                let mid = (p1 + p2) * 0.5 + crate::physics::cosmetic_arc_jitter(p1, td.life, 1) * (0.4 / 0.15);
                draw_line_3d(p1, mid, WHITE);
                draw_line_3d(mid, p2, WHITE);
            }
        }
    }

    /// Checks whether a proposed ground footprint has unobstructed headroom
    pub fn is_placement_clear(target_pos: Vec3, grid: &VoxelGrid) -> bool {
        let v_center = grid.world_to_voxel(Vec3::new(target_pos.x, 0.50, target_pos.z));
        let check_r = 2;
        for dx in -check_r..=check_r {
            for dz in -check_r..=check_r {
                for vy in 1..=3 {
                    let vx = v_center.x + dx;
                    let vz = v_center.z + dz;
                    if grid.in_bounds(vx, vy, vz) && grid.get(vx, vy, vz).is_solid() {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Renders holographic green/cyan footprint preview when player is aiming to deploy
    pub fn draw_placement_preview(&self, target_pos: Option<Vec3>, grid: Option<&VoxelGrid>) {
        if let Some(pos) = target_pos {
            let ground_p = Vec3::new(pos.x, 0.52, pos.z);
            let theme = self.selected_kind.theme_color();
            let is_clear = grid.map(|g| Self::is_placement_clear(pos, g)).unwrap_or(true);
            let preview_col = if is_clear {
                Color::new(theme.r, theme.g, theme.b, 0.75)
            } else {
                Color::new(1.0, 0.25, 0.20, 0.85) // Obstructed warning red
            };

            // Base footprint disc
            let radius = 2.4;
            let segments = 24;
            for s in 0..segments {
                let theta1 = (s as f32 / segments as f32) * std::f32::consts::TAU;
                let theta2 = ((s + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                let p1 = ground_p + Vec3::new(theta1.cos() * radius, 0.0, theta1.sin() * radius);
                let p2 = ground_p + Vec3::new(theta2.cos() * radius, 0.0, theta2.sin() * radius);
                draw_line_3d(p1, p2, preview_col);
            }

            // Central beacon wireframe
            draw_cube_wires(ground_p + Vec3::new(0.0, 1.2, 0.0), Vec3::new(1.8, 2.4, 1.8), preview_col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_reloading_selected_battery_cannot_fire_a_different_engine() {
        let mut siege = SiegeSystem::new();
        siege.setup_default_fleet();
        siege.deployed_engines[0].cooldown_timer = 1.0;
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        let trauma = siege.fire_active_engine(Some(Vec3::new(0.0, 5.0, 0.0)), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(trauma, 0.0);
        assert!(siege.deployed_engines.iter().all(|engine| engine.shots_fired == 0));
    }

    #[test]
    fn regression_siege_shell_enters_excavated_crater() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        grid.carve_sphere_advanced(Vec3::ZERO, 5.0, 9000.0, DamageType::Kinetic);
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();
        siege.colossal_shells.push(ColossalShell {
            position: Vec3::new(0.25, 1.0, 0.25),
            velocity: Vec3::new(0.0, -20.0, 0.0),
            caliber_mm: 500.0,
            radius: 0.75,
            blast_radius: 8.0,
            payload: SiegePayload::ColossalHighExplosive,
            penetrations_remaining: 1,
            fuse_timer: 0.1,
            is_fuse_active: false,
            life: 0.0,
            max_life: 5.0,
            color: ORANGE,
            trail_points: TrailBuffer::new(),
            has_detonated: false,
        });
        siege.update(0.08, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.colossal_shells.len(), 1, "No invisible ground should stop shells entering a crater");
        assert!(siege.colossal_shells[0].position.y < 0.0);
        siege.update(0.25, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert!(siege.colossal_shells.is_empty(), "Shell must impact the real crater floor");
    }

    #[test]
    fn test_siege_config_shell_radius_scaling() {
        let mut cfg = SiegeWeaponConfig {
            caliber_mm: 500.0,
            ..Default::default()
        };
        assert_eq!(cfg.shell_radius(), 0.75, "500mm caliber must have 0.75m radius (1.5m diameter)");

        cfg.caliber_mm = 2500.0;
        assert_eq!(cfg.shell_radius(), 1.75, "2500mm caliber must have 1.75m radius (3.5m diameter)");

        cfg.caliber_mm = 1500.0;
        let mid_r = cfg.shell_radius();
        assert!((mid_r - 1.25).abs() < 0.01, "1500mm caliber must have ~1.25m radius (2.5m diameter)");
    }

    #[test]
    fn test_siege_config_blast_radius_and_payloads() {
        let mut cfg = SiegeWeaponConfig {
            propellant_power: 1.0,
            caliber_mm: 1500.0,
            payload: SiegePayload::ColossalHighExplosive,
            ..Default::default()
        };
        let he_blast = cfg.blast_radius();
        assert!(he_blast > 10.0, "Colossal HE must have > 10m blast radius, got {}", he_blast);

        cfg.payload = SiegePayload::AntimatterSingularity;
        let am_blast = cfg.blast_radius();
        assert!(am_blast > he_blast, "Antimatter Singularity must exceed standard HE blast radius");
    }

    #[test]
    fn test_siege_launch_velocity_trajectory_vectors() {
        let cfg = SiegeWeaponConfig {
            auto_aim_reticle: false,
            azimuth_deg: 0.0, // Facing +X
            elevation_deg: 45.0,
            propellant_power: 1.0,
            ..Default::default()
        };

        let vel = cfg.compute_launch_velocity_vector(0.0);
        assert!(vel.x > 0.0, "Velocity X must be positive along azimuth 0°");
        assert!(vel.y > 0.0, "Velocity Y must be positive for elevation 45°");
        assert!((vel.x - vel.y).abs() < 0.01, "At 45°, horizontal and vertical velocity should match");
    }

    #[test]
    fn test_siege_system_deployment_and_reposition() {
        let mut siege = SiegeSystem::new();
        let audio = SoundSystem::empty();

        assert!(siege.deployed_engines.is_empty());
        let deployed = siege.deploy_selected_engine(Vec3::new(10.0, 0.0, 20.0), Vec3::X, &audio);
        assert!(deployed);
        assert_eq!(siege.deployed_engines.len(), 1);
        assert_eq!(siege.deployed_engines[0].position.y, 0.50, "Engine must anchor flush to bedrock at y = 0.50");
        assert_eq!(siege.deployed_engines[0].kind, SiegeEngineKind::RailArtillery);

        // Repositioning same kind updates in place
        siege.deploy_selected_engine(Vec3::new(15.0, 0.0, 25.0), Vec3::Z, &audio);
        assert_eq!(siege.deployed_engines.len(), 1, "Redeploying same kind must update position without duplicating");
        assert_eq!(siege.deployed_engines[0].position.x, 15.0);

        // Selecting a new kind and deploying adds to fleet
        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(0.0, 0.0, 10.0), Vec3::X, &audio);
        assert_eq!(siege.deployed_engines.len(), 2);
    }

    #[test]
    fn test_siege_system_firing_spawns_colossal_shell() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Deploy Rail Artillery
        siege.select_kind(SiegeEngineKind::RailArtillery, &audio);
        siege.deploy_selected_engine(Vec3::new(0.0, 0.5, 10.0), Vec3::Z, &audio);

        assert_eq!(siege.colossal_shells.len(), 0);
        let trauma = siege.fire_active_engine(
            Some(Vec3::new(0.0, 8.0, 30.0)),
            &mut grid,
            &mut physics,
            &audio,
            Vec3::ZERO,
        );

        assert!(trauma > 0.0, "Firing heavy artillery must generate trauma feedback");
        assert_eq!(siege.colossal_shells.len(), 1, "Must spawn 1 active colossal shell");
        let shell = &siege.colossal_shells[0];
        assert!(shell.radius >= 0.75, "Colossal shell radius must be >= 0.75m, got {}", shell.radius);
        assert!(shell.blast_radius >= 8.0, "Colossal shell blast radius must be >= 8.0m, got {}", shell.blast_radius);
    }

    #[test]
    fn test_orbital_kinetic_relay_strikes_down_through_structure() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Place a concrete pillar at (5, y, 5)
        for y in 1..=10 {
            grid.set(5, y, 5, VoxelType::Concrete);
        }
        let target_world = grid.voxel_to_world(IVec3::new(5, 5, 5));

        // Deploy Orbital Relay and Fire
        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(0.0, 0.5, 0.0), Vec3::X, &audio);
        siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.orbital_strikes.len(), 1);
        let strike = &siege.orbital_strikes[0];
        assert_eq!(strike.target_pos, target_world);

        // Fast-forward warmup timer to trigger vertical ion beam
        siege.orbital_strikes[0].warmup_timer = 0.0;
        let trauma = siege.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert!(trauma > 0.0, "Ion beam strike must generate intense trauma");
        assert!(siege.orbital_strikes[0].has_struck, "Beam must have struck");

        // The pillar voxels along the beam should now be carved
        let center_vt = grid.get(5, 5, 5);
        assert_eq!(center_vt, VoxelType::Air, "Orbital beam must vaporize voxels along vertical shaft");
    }

    #[test]
    fn test_hydraulic_ground_ram_pumps_seismic_foundation_waves() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Set up concrete ground foundation columns
        for x in 4..=6 {
            for y in 1..=3 {
                grid.set(x, y, 5, VoxelType::Concrete);
            }
        }

        let origin = Vec3::new(0.0, 0.5, 5.0);
        siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
        siege.deploy_selected_engine(origin, Vec3::X, &audio);
        siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.seismic_pulses.len(), 1);
        assert_eq!(siege.seismic_pulses[0].origin, origin);

        // Update pulse wave as it expands
        siege.update(0.2, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert!(siege.seismic_pulses[0].current_radius > 0.5, "Seismic wave must expand radially");
    }

    #[test]
    fn test_bunker_buster_delayed_penetration_fuse() {
        let cfg = SiegeWeaponConfig {
            payload: SiegePayload::BunkerBuster,
            delayed_fuse_secs: 0.20,
            ..Default::default()
        };

        let shell = ColossalShell {
            position: Vec3::new(0.0, 5.0, 0.0),
            velocity: Vec3::new(0.0, 0.0, 80.0),
            caliber_mm: 1500.0,
            radius: 1.25,
            blast_radius: 8.5,
            payload: cfg.payload,
            penetrations_remaining: 5,
            fuse_timer: cfg.delayed_fuse_secs,
            is_fuse_active: false,
            life: 0.0,
            max_life: 8.0,
            color: cfg.payload.color(),
            trail_points: TrailBuffer::new(),
            has_detonated: false,
        };

        assert_eq!(shell.penetrations_remaining, 5);
        assert!(!shell.is_fuse_active);
    }

    #[test]
    fn test_tesla_siege_pylon_arcs_and_cascades_through_rebar() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Create rebar network in two adjacent columns
        grid.set(10, 2, 10, VoxelType::SteelRebar);
        grid.set(10, 4, 10, VoxelType::SteelRebar);
        grid.set(14, 2, 10, VoxelType::SteelRebar);

        let pylon_pos = Vec3::new(0.0, 0.5, 10.0);
        siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &audio);
        siege.deploy_selected_engine(pylon_pos, Vec3::X, &audio);

        let target_world = grid.voxel_to_world(IVec3::new(10, 2, 10));
        let trauma = siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert!(trauma > 0.0, "Tesla discharge must generate trauma");
        assert_eq!(siege.tesla_discharges.len(), 1, "Must generate a tesla discharge");
        let td = &siege.tesla_discharges[0];
        assert!(td.branches.len() >= 2, "Must contain primary arc plus cascading conduction branches");
    }

    #[test]
    fn test_colossal_shell_hits_physics_chunks() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Gravity takes the center ray below this voxel, but the 1.25m-radius shell
        // still clips its lower face and must detonate.
        let chunk_pos = Vec3::new(0.0, 5.0, 15.0);
        physics.chunks.push(crate::physics::DynamicChunk {
            position: chunk_pos,
            velocity: Vec3::ZERO,
            rotation: Vec3::ZERO,
            rot_mat: crate::physics::Mat3::IDENTITY,
            angular_velocity: Vec3::ZERO,
            voxels: vec![(Vec3::ZERO, VoxelType::Concrete)],
            exposed_voxels: Vec::new(),
            voxel_health: Vec::new(),
            mass: 50.0,
            life: 0.0,
            settled: false,
            local_aabb: (Vec3::ZERO, Vec3::ZERO),
        });

        // Launch shell toward the chunk; the full projectile volume matters.
        let shell = ColossalShell {
            position: Vec3::new(0.0, 5.0, 0.0),
            velocity: Vec3::new(0.0, 0.0, 90.0),
            caliber_mm: 1500.0,
            radius: 1.25,
            blast_radius: 10.0,
            payload: SiegePayload::ColossalHighExplosive,
            penetrations_remaining: 1,
            fuse_timer: 0.1,
            is_fuse_active: false,
            life: 0.0,
            max_life: 5.0,
            color: Color::new(1.0, 0.5, 0.1, 1.0),
            trail_points: TrailBuffer::new(),
            has_detonated: false,
        };
        let center_end = shell.position + (shell.velocity + Vec3::new(0.0, -18.0, 0.0) * 0.2) * 0.2;
        assert!(physics.check_chunk_hit(shell.position, center_end).is_none(), "The center ray should miss this grazing contact");
        siege.colossal_shells.push(shell);

        // Advance simulation: shell must detect chunk and detonate
        siege.update(0.2, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.colossal_shells.len(), 0, "Shell must impact chunk and detonate");
        assert!(!physics.shockwaves.is_empty(), "Detonation against chunk must create shockwave");
    }

    #[test]
    fn test_colossal_shell_substepping_bunker_buster_wall_penetration() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Place 2 consecutive concrete walls
        grid.set(10, 5, 0, VoxelType::Concrete);
        grid.set(14, 5, 0, VoxelType::Concrete);

        let w1_world = grid.voxel_to_world(IVec3::new(10, 5, 0));

        let shell = ColossalShell {
            position: w1_world - Vec3::new(2.0, 0.0, 0.0),
            velocity: Vec3::new(60.0, 0.0, 0.0), // Moving +X
            caliber_mm: 1500.0,
            radius: 1.0,
            blast_radius: 8.0,
            payload: SiegePayload::BunkerBuster,
            penetrations_remaining: 3,
            fuse_timer: 0.50, // Long fuse to allow multi-wall penetration
            is_fuse_active: false,
            life: 0.0,
            max_life: 5.0,
            color: Color::new(0.9, 0.7, 0.2, 1.0),
            trail_points: TrailBuffer::new(),
            has_detonated: false,
        };
        siege.colossal_shells.push(shell);

        // Update 0.05s: shell pierces first wall
        siege.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.colossal_shells.len(), 1, "Bunker buster must survive first wall hit");
        assert!(siege.colossal_shells[0].is_fuse_active, "Fuse must activate upon piercing wall");
        assert!(siege.colossal_shells[0].penetrations_remaining < 3, "Penetrations must decrease");
    }

    #[test]
    fn test_placement_clearance_detection() {
        let mut grid = VoxelGrid::new();
        let target_clear = Vec3::new(5.0, 0.5, 5.0);
        assert!(SiegeSystem::is_placement_clear(target_clear, &grid));

        // Place a solid wall at target position headroom
        let v_obst = grid.world_to_voxel(target_clear);
        grid.set(v_obst.x, 2, v_obst.z, VoxelType::Concrete);
        assert!(!SiegeSystem::is_placement_clear(target_clear, &grid), "Site with solid voxels in headroom must be detected as obstructed");
    }

    #[test]
    fn test_recall_all_siege_engines() {
        let mut siege = SiegeSystem::new();
        let audio = SoundSystem::empty();

        siege.select_kind(SiegeEngineKind::RailArtillery, &audio);
        siege.deploy_selected_engine(Vec3::new(0.0, 0.5, 0.0), Vec3::X, &audio);
        siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
        siege.deploy_selected_engine(Vec3::new(5.0, 0.5, 0.0), Vec3::Z, &audio);

        assert_eq!(siege.deployed_engines.len(), 2);
        siege.recall_all(&audio);
        assert_eq!(siege.deployed_engines.len(), 0);
        assert!(siege.hud_feedback_msg.is_some());
    }

    #[test]
    fn test_setup_default_fleet_deployment() {
        let mut siege = SiegeSystem::new();
        assert!(siege.deployed_engines.is_empty());

        siege.setup_default_fleet();
        assert_eq!(siege.deployed_engines.len(), 3);
        assert!(siege.deployed_engines.iter().any(|e| e.kind == SiegeEngineKind::RailArtillery));
        assert!(siege.deployed_engines.iter().any(|e| e.kind == SiegeEngineKind::OrbitalKineticRelay));
        assert!(siege.deployed_engines.iter().any(|e| e.kind == SiegeEngineKind::HydraulicGroundRam));
    }

    #[test]
    fn test_hydraulic_ground_ram_harmonic_resonance_escalation() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let pos = Vec3::new(0.0, 0.5, 0.0);
        siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
        siege.deploy_selected_engine(pos, Vec3::X, &audio);

        // 1st slam -> Stage 1 (1.0x)
        siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.deployed_engines[0].resonance_slam_count, 1);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_level, 1);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_mult, 1.0);

        // Reset cooldown and fire again within 6.0s -> Stage 2 (1.5x)
        siege.deployed_engines[0].cooldown_timer = 0.0;
        siege.deployed_engines[0].last_slam_time = 1.0;
        siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.deployed_engines[0].resonance_slam_count, 2);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_level, 2);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_mult, 1.5);

        // Advance to Stage 3 (2.2x), Stage 4 (3.0x), Stage 5 (4.0x)
        for expected_stage in [3, 4, 5] {
            siege.deployed_engines[0].cooldown_timer = 0.0;
            siege.deployed_engines[0].last_slam_time = 0.5;
            siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);
            assert_eq!(siege.deployed_engines[0].resonance_slam_count, expected_stage);
        }
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_level, 5);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_mult, 4.0);

        // Stage 5 caps at 5
        siege.deployed_engines[0].cooldown_timer = 0.0;
        siege.deployed_engines[0].last_slam_time = 0.5;
        siege.fire_active_engine(None, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.deployed_engines[0].resonance_slam_count, 5);
        assert_eq!(siege.seismic_pulses.last().unwrap().resonance_mult, 4.0);

        // Idle > 6.0s resets resonance count
        siege.update(6.5, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.deployed_engines[0].resonance_slam_count, 0, "Resonance must reset after 6.0s idle");
    }

    #[test]
    fn test_bunker_buster_cylinder_bore_and_spall_fan() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Build a 2-voxel thick reinforced wall at x = 10, 11
        for x in 10..=11 {
            for y in 4..=6 {
                for z in 4..=6 {
                    grid.set(x, y, z, VoxelType::Concrete);
                }
            }
        }

        let w_pos = grid.voxel_to_world(IVec3::new(10, 5, 5));
        let shell = ColossalShell {
            position: w_pos - Vec3::new(2.0, 0.0, 0.0),
            velocity: Vec3::new(70.0, 0.0, 0.0),
            caliber_mm: 1500.0,
            radius: 1.0,
            blast_radius: 8.0,
            payload: SiegePayload::BunkerBuster,
            penetrations_remaining: 4,
            fuse_timer: 1.0,
            is_fuse_active: false,
            life: 0.0,
            max_life: 5.0,
            color: Color::new(0.9, 0.7, 0.2, 1.0),
            trail_points: TrailBuffer::new(),
            has_detonated: false,
        };
        siege.colossal_shells.push(shell);

        let initial_particles = physics.debris.len();

        // Advance simulation: shell impacts wall and punches through with kinetic cylinder bore
        siege.update(0.04, &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.colossal_shells.len(), 1, "Bunker buster should pierce through wall before terminal blast");
        assert!(siege.colossal_shells[0].is_fuse_active, "Fuse must activate upon penetrating wall");
        assert!(siege.colossal_shells[0].penetrations_remaining < 4, "Penetrations must decrement");
        assert!(physics.debris.len() > initial_particles, "Bore punch-through must generate spall fan debris");
    }

    #[test]
    fn test_tesla_siege_pylon_conductive_traversal_and_induction_detonation() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Set up a PowerCapacitor and adjacent Explosive within 2.0m induction distance
        let cap_coord = IVec3::new(12, 3, 12);
        let exp_coord = IVec3::new(14, 3, 12);
        grid.set(cap_coord.x, cap_coord.y, cap_coord.z, VoxelType::PowerCapacitor);
        grid.set(exp_coord.x, exp_coord.y, exp_coord.z, VoxelType::Explosive);

        let pylon_pos = Vec3::new(0.0, 0.5, 12.0);
        siege.select_kind(SiegeEngineKind::TeslaSiegePylon, &audio);
        siege.deploy_selected_engine(pylon_pos, Vec3::X, &audio);

        let target_world = grid.voxel_to_world(cap_coord);
        let trauma = siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert!(trauma > 0.0, "Tesla discharge must cause trauma");
        assert_eq!(siege.tesla_discharges.len(), 1, "Must generate a tesla discharge");
        // PowerCapacitor should be carved/discharged
        assert_eq!(grid.get(cap_coord.x, cap_coord.y, cap_coord.z), VoxelType::Air, "Capacitor node must be discharged");
        // Explosive within induction distance should be detonated
        assert_eq!(grid.get(exp_coord.x, exp_coord.y, exp_coord.z), VoxelType::Air, "Adjacent explosive must be detonated by induction");
    }

    #[test]
    fn test_orbital_kinetic_relay_core_bore_and_thermal_sheath() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Build a multi-story column at (8, y, 8)
        for y in 1..=20 {
            grid.set(8, y, 8, VoxelType::Concrete);
            grid.set(9, y, 8, VoxelType::HeavyArmor);
        }

        let target_world = grid.voxel_to_world(IVec3::new(8, 10, 8));
        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(0.0, 0.5, 0.0), Vec3::X, &audio);
        siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.orbital_strikes.len(), 1);
        // Fast-forward countdown
        siege.orbital_strikes[0].warmup_timer = 0.0;
        siege.update(0.05, &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert!(siege.orbital_strikes[0].has_struck);
        // Both the inner kinetic core and outer thermal sheath should have cleared the column
        assert_eq!(grid.get(8, 10, 8), VoxelType::Air, "Inner core must carve vertical column");
        assert_eq!(grid.get(9, 10, 8), VoxelType::Air, "Thermal sheath must carve adjacent voxels");
        assert!(!physics.shockwaves.is_empty(), "Orbital strike must produce colossal shockwave");
    }

    #[test]
    fn test_rail_artillery_colossal_he_scaled_shockwave_and_secondary_spall() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Build a concrete target bunker
        for x in 10..=16 {
            for y in 1..=6 {
                for z in 10..=16 {
                    grid.set(x, y, z, VoxelType::Concrete);
                }
            }
        }

        let target = grid.voxel_to_world(IVec3::new(13, 3, 13));
        let deploy_pos = Vec3::new(target.x - 30.0, 0.5, target.z);

        siege.select_kind(SiegeEngineKind::RailArtillery, &audio);
        siege.deploy_selected_engine(deploy_pos, Vec3::X, &audio);

        // Configure ColossalHighExplosive payload
        let mut config = siege.get_engine_config(SiegeEngineKind::RailArtillery);
        config.payload = SiegePayload::ColossalHighExplosive;
        config.caliber_mm = 1500.0;
        config.propellant_power = 1.5;
        siege.set_engine_config(SiegeEngineKind::RailArtillery, config);

        let initial_sparks = physics.sparks.len();
        let initial_debris = physics.debris.len();

        siege.fire_active_engine(Some(target), &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert_eq!(siege.colossal_shells.len(), 1);

        // Force shell impact
        siege.colossal_shells[0].position = target;
        siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

        // Verify shockwave radius is scaled to 2.8x blast_radius (~45.7m)
        let shock = physics.shockwaves.iter().find(|s| s.max_radius >= 40.0);
        assert!(
            shock.is_some(),
            "Rail Artillery HE shockwave max radius must be >= 40m (2.8x blast_r), got {:?}",
            physics.shockwaves.iter().map(|s| s.max_radius).collect::<Vec<_>>()
        );

        // Verify secondary spall shower along crater perimeter generated sparks and debris
        assert!(
            physics.sparks.len() > initial_sparks + 30,
            "Secondary spall shower must emit dense sparks along crater perimeter (sparks: {} -> {})",
            initial_sparks,
            physics.sparks.len()
        );
        assert!(
            physics.debris.len() > initial_debris + 30,
            "Flying rubble shower and spall flakes must be emitted (debris: {} -> {})",
            initial_debris,
            physics.debris.len()
        );
    }

    #[test]
    fn test_orbital_kinetic_relay_bedrock_shockwave_and_vertical_column_spall() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let target_world = grid.voxel_to_world(IVec3::new(24, 15, 24));
        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);

        let pre_sparks = physics.sparks.len();
        siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.orbital_strikes.len(), 1);
        siege.orbital_strikes[0].warmup_timer = 0.0; // Fast-forward countdown

        siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);
        assert!(siege.orbital_strikes[0].has_struck);

        // Verify bedrock shockwave expanded to 28m radius
        let bedrock_shock = physics.shockwaves.iter().find(|s| (s.max_radius - 28.0).abs() < 1.0);
        assert!(
            bedrock_shock.is_some(),
            "Orbital Kinetic Relay must spawn 28m bedrock cratering shockwave"
        );

        // Verify vertical column spall bursts from y = 140 down to bedrock
        assert!(
            physics.sparks.len() > pre_sparks + 50,
            "Vertical column spall bursts must generate abundant sparks along 140m descent (sparks: {} -> {})",
            pre_sparks,
            physics.sparks.len()
        );
    }

    #[test]
    fn test_hydraulic_ground_ram_harmonic_resonance_perimeter_spall_fans() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        let ram_pos = Vec3::new(20.0, 0.5, 20.0);
        let target = Vec3::new(28.0, 0.5, 20.0);

        // Build foundation pillars in path
        for y in 1..=4 {
            grid.set(24, y, 20, VoxelType::Concrete);
            grid.set(25, y, 20, VoxelType::SteelRebar);
        }

        siege.select_kind(SiegeEngineKind::HydraulicGroundRam, &audio);
        siege.deploy_selected_engine(ram_pos, Vec3::X, &audio);

        // Escalate to resonance stage 3 (2.2x power)
        siege.deployed_engines[0].last_slam_time = 1.0;
        siege.deployed_engines[0].resonance_slam_count = 2;

        let pre_sparks = physics.sparks.len();
        siege.fire_active_engine(Some(target), &mut grid, &mut physics, &audio, Vec3::ZERO);

        // Bedrock slam spall fan must have been spawned
        assert!(
            physics.sparks.len() > pre_sparks,
            "Bedrock slam must spawn perimeter spall fans at ram origin"
        );

        // Advance pulse wave to reach pillars
        for _ in 0..10 {
            siege.update(0.03, &mut grid, &mut physics, &audio, Vec3::ZERO);
        }

        // Foundation pillars should be damaged/crushed with spall fans
        assert!(
            physics.sparks.len() > pre_sparks + 15,
            "Harmonic resonance pulverization must generate perimeter spall fans for buckled pillars"
        );
    }

    #[test]
    fn test_orbital_kinetic_relay_multi_floor_vertical_penetration() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Construct a multi-story building at x=30, z=30:
        // Ground floor y=1..4, Floor 2 y=8..10, Floor 3 y=16..18, Roof y=24..26
        for y in 1..=26 {
            grid.set(30, y, 30, VoxelType::Concrete);
            grid.set(31, y, 30, VoxelType::SteelRebar);
            grid.set(30, y, 31, VoxelType::CompositeRebar);
        }

        // Aim specifically at the rooftop (y = 25)
        let roof_target = grid.voxel_to_world(IVec3::new(30, 25, 30));

        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
        siege.fire_active_engine(Some(roof_target), &mut grid, &mut physics, &audio, Vec3::ZERO);

        assert_eq!(siege.orbital_strikes.len(), 1);
        siege.orbital_strikes[0].warmup_timer = 0.0;
        siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

        // Verify beam did NOT detonate prematurely on the roof:
        // Roof voxels (y=25) must be carved
        assert_eq!(grid.get(30, 25, 30), VoxelType::Air, "Roof must be penetrated");
        // Intermediate floors (y=17, y=9) must be carved
        assert_eq!(grid.get(30, 17, 30), VoxelType::Air, "Mid-floor 2 must be penetrated");
        assert_eq!(grid.get(30, 9, 30), VoxelType::Air, "Mid-floor 1 must be penetrated");
        // Ground foundation (y=2) must be carved
        assert_eq!(grid.get(30, 2, 30), VoxelType::Air, "Ground story must be penetrated");
        // Rebar columns must be severed
        assert_eq!(grid.get(31, 17, 30), VoxelType::Air, "Steel rebar must be vaporized");
        assert_eq!(grid.get(30, 17, 31), VoxelType::Air, "Composite rebar must be vaporized");
    }

    #[test]
    fn test_orbital_kinetic_relay_16m_bedrock_crater_and_subterranean_excavation() {
        let mut siege = SiegeSystem::new();
        let mut grid = VoxelGrid::new();
        let mut physics = PhysicsSystem::new();
        let audio = SoundSystem::empty();

        // Target surface center (48, 0, 48)
        let target_world = grid.voxel_to_world(IVec3::new(48, 0, 48));

        siege.select_kind(SiegeEngineKind::OrbitalKineticRelay, &audio);
        siege.deploy_selected_engine(Vec3::new(10.0, 0.5, 10.0), Vec3::X, &audio);
        siege.fire_active_engine(Some(target_world), &mut grid, &mut physics, &audio, Vec3::ZERO);

        siege.orbital_strikes[0].warmup_timer = 0.0;
        siege.update(0.02, &mut grid, &mut physics, &audio, Vec3::ZERO);

        // 1. Verify 16-meter crater diameter at surface bedrock level (y = 0):
        // Radius R = 8.0m = 16 voxels. Voxels within 15 voxels must be Air.
        for dx in -14..=14 {
            for dz in -14..=14 {
                let dist_vox = ((dx * dx + dz * dz) as f32).sqrt();
                if dist_vox <= 15.0 {
                    assert_eq!(
                        grid.get(48 + dx, 0, 48 + dz),
                        VoxelType::Air,
                        "Bedrock at dx={}, dz={} must be excavated inside 16m crater",
                        dx, dz
                    );
                }
            }
        }

        // Voxels outside 20 voxels radius (10.0m) must remain unexcavated Bedrock
        assert_eq!(grid.get(48 + 20, 0, 48), VoxelType::Bedrock, "Outside crater rim must remain intact");

        // 2. Verify subterranean depth down to -16.0m (voxel y = -32, inside [-12m, -18m]):
        assert_eq!(grid.get(48, -10, 48), VoxelType::Air, "Basalt at -5.0m must be excavated");
        assert_eq!(grid.get(48, -24, 48), VoxelType::Air, "Basalt at -12.0m must be excavated");
        assert_eq!(grid.get(48, -30, 48), VoxelType::Air, "Basalt at -15.0m must be excavated");

        // Deep bedrock beneath -20m (y <= -42) remains intact
        assert_eq!(grid.get(48, -44, 48), VoxelType::Bedrock, "Deep foundation below crater remains intact");

        // 3. Verify rubble_height_grid excavation matches new subterranean depth:
        let crater_floor_h = physics.rubble_height_at(target_world.x, target_world.z);
        assert!(
            crater_floor_h < -12.0,
            "Crater center rubble height must be excavated below -12.0m, got {:.2}",
            crater_floor_h
        );
        assert!(
            crater_floor_h >= crate::voxel::VOID_BOUNDARY_Y,
            "Crater center must be at or above void boundary"
        );

        // Outside crater remains at surface terrain height
        let outside_h = physics.rubble_height_at(target_world.x + 18.0, target_world.z + 18.0);
        assert_eq!(outside_h, crate::voxel::VOXEL_SIZE);

        // 4. Verify Dynamic Point Light at bedrock crater
        let mut lights = Vec::new();
        siege.collect_dynamic_lights(&mut lights);
        let bedrock_light = lights.iter().find(|l| l.position.y <= 0.0 && l.radius >= 30.0);
        assert!(bedrock_light.is_some(), "Dynamic point light must illuminate subterranean crater bowl");

        // 5. Verify incandescent ejecta sparks erupted
        assert!(physics.sparks.len() >= 80, "Incandescent ejecta sparks must be spawned");
    }
}
