// Horizon BBS - guns, ammo/crafting, 3D gun models, BBS-style actors, chroma
// blocks and the Rust Alkaline policy core.
//
// This file is both the standalone Cargo library and the injected
// `crate::game::horizon_bbs` module; no generated copy exists. It uses only
// `std`, inline modules and ordinary comments, as horizon-mixins requires.
//
// Everything here is plain, host-independent Rust so it can be unit tested
// without a running game. Wiring it to Horizon's real entities, input and
// renderer needs hook targets verified against the pinned Horizon source; see
// README.md ("What is and is not connected").

// ---------------------------------------------------------------------------
// Small vector helpers
// ---------------------------------------------------------------------------
pub mod math {
    pub type Vec3 = [f32; 3];

    pub fn add(a: Vec3, b: Vec3) -> Vec3 {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }

    pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }

    pub fn scale(a: Vec3, s: f32) -> Vec3 {
        [a[0] * s, a[1] * s, a[2] * s]
    }

    pub fn dot(a: Vec3, b: Vec3) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }

    pub fn length(a: Vec3) -> f32 {
        dot(a, a).sqrt()
    }

    pub fn distance(a: Vec3, b: Vec3) -> f32 {
        length(sub(a, b))
    }

    pub fn normalize(a: Vec3) -> Vec3 {
        let len = length(a);
        if len <= f32::EPSILON {
            [0.0, 0.0, 1.0]
        } else {
            scale(a, 1.0 / len)
        }
    }

    // Slab test. Returns the distance along `dir` (unit length) to the first
    // face of the box, or `None` if the ray misses it. A ray starting inside
    // the box reports distance 0.
    pub fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
        let mut t_near = 0.0_f32;
        let mut t_far = f32::INFINITY;
        for axis in 0..3 {
            if dir[axis].abs() < 1e-8 {
                if origin[axis] < min[axis] || origin[axis] > max[axis] {
                    return None;
                }
            } else {
                let inv = 1.0 / dir[axis];
                let mut t0 = (min[axis] - origin[axis]) * inv;
                let mut t1 = (max[axis] - origin[axis]) * inv;
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                t_near = t_near.max(t0);
                t_far = t_far.min(t1);
                if t_near > t_far {
                    return None;
                }
            }
        }
        Some(t_near)
    }
}

// ---------------------------------------------------------------------------
// Deterministic random numbers (xorshift64*), so spread is reproducible
// ---------------------------------------------------------------------------
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    // Uniform in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

// ---------------------------------------------------------------------------
// Guns
// ---------------------------------------------------------------------------
pub mod guns {
    use super::Rng;
    use super::math::{self, Vec3};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub enum GunKind {
        Pistol,
        Rifle,
        Sniper,
        Shotgun,
    }

    // Damage is in health points; a default actor/player has 20 (10 hearts).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct GunStats {
        // Damage of one projectile at point-blank body hit.
        pub damage: f32,
        // Projectiles per trigger pull (shotgun fires several).
        pub pellets: u8,
        // Half-angle of the random spread cone, in degrees.
        pub spread_deg: f32,
        // Maximum hitscan distance in blocks.
        pub range: f32,
        // Distance where damage starts to drop off.
        pub falloff_start: f32,
        // Fraction of damage lost at maximum range (0.0 - 1.0).
        pub max_falloff: f32,
        // Ticks (1/20 s) between shots.
        pub cooldown_ticks: u32,
        pub magazine: u32,
        pub reload_ticks: u32,
        pub headshot_multiplier: f32,
    }

    impl GunKind {
        pub const ALL: [GunKind; 4] = [
            GunKind::Pistol,
            GunKind::Rifle,
            GunKind::Sniper,
            GunKind::Shotgun,
        ];

        pub fn name(self) -> &'static str {
            match self {
                GunKind::Pistol => "pistol",
                GunKind::Rifle => "rifle",
                GunKind::Sniper => "sniper",
                GunKind::Shotgun => "shotgun",
            }
        }

        pub fn from_name(name: &str) -> Option<GunKind> {
            GunKind::ALL
                .into_iter()
                .find(|kind| kind.name().eq_ignore_ascii_case(name))
        }

        pub fn stats(self) -> GunStats {
            match self {
                // Reliable sidearm: 6 damage, 4 body shots to kill 20 hp.
                GunKind::Pistol => GunStats {
                    damage: 6.0,
                    pellets: 1,
                    spread_deg: 1.0,
                    range: 40.0,
                    falloff_start: 15.0,
                    max_falloff: 0.5,
                    cooldown_ticks: 8,
                    magazine: 12,
                    reload_ticks: 30,
                    headshot_multiplier: 1.5,
                },
                // Fast and sustained: 8 damage, 3 shots to kill, 6.7 shots/s.
                GunKind::Rifle => GunStats {
                    damage: 8.0,
                    pellets: 1,
                    spread_deg: 2.0,
                    range: 80.0,
                    falloff_start: 35.0,
                    max_falloff: 0.4,
                    cooldown_ticks: 3,
                    magazine: 30,
                    reload_ticks: 50,
                    headshot_multiplier: 1.75,
                },
                // 40 per body shot, 100 on a headshot: one hit kills 20 hp.
                GunKind::Sniper => GunStats {
                    damage: 40.0,
                    pellets: 1,
                    spread_deg: 0.1,
                    range: 200.0,
                    falloff_start: 120.0,
                    max_falloff: 0.15,
                    cooldown_ticks: 30,
                    magazine: 5,
                    reload_ticks: 70,
                    headshot_multiplier: 2.5,
                },
                // 8 pellets x 4.5 = 36 at point blank, weak past a few blocks.
                GunKind::Shotgun => GunStats {
                    damage: 4.5,
                    pellets: 8,
                    spread_deg: 6.0,
                    range: 20.0,
                    falloff_start: 5.0,
                    max_falloff: 0.8,
                    cooldown_ticks: 20,
                    magazine: 6,
                    reload_ticks: 60,
                    headshot_multiplier: 1.25,
                },
            }
        }
    }

    // Damage of one projectile that travelled `distance` blocks. Zero out of range.
    pub fn damage_at(stats: &GunStats, distance: f32, headshot: bool) -> f32 {
        if distance > stats.range || distance < 0.0 {
            return 0.0;
        }
        let span = (stats.range - stats.falloff_start).max(f32::EPSILON);
        let over = ((distance - stats.falloff_start) / span).clamp(0.0, 1.0);
        let mut damage = stats.damage * (1.0 - stats.max_falloff * over);
        if headshot {
            damage *= stats.headshot_multiplier;
        }
        damage
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Gun {
        pub kind: GunKind,
        pub ammo: u32,
        pub cooldown: u32,
        pub reload_left: u32,
        // Rounds that will be loaded when the current reload finishes.
        pub pending_load: u32,
    }

    impl Gun {
        pub fn new(kind: GunKind) -> Self {
            Self {
                kind,
                ammo: kind.stats().magazine,
                cooldown: 0,
                reload_left: 0,
                pending_load: 0,
            }
        }

        pub fn is_reloading(&self) -> bool {
            self.reload_left > 0
        }

        pub fn can_fire(&self) -> bool {
            self.ammo > 0 && self.cooldown == 0 && self.reload_left == 0
        }

        // Spends one round and starts the cooldown. False if the gun can't fire now.
        pub fn try_fire(&mut self) -> bool {
            if !self.can_fire() {
                return false;
            }
            self.ammo -= 1;
            self.cooldown = self.kind.stats().cooldown_ticks;
            true
        }

        // Actors fire without ammo or reloading: only the cooldown applies.
        pub fn try_fire_free(&mut self) -> bool {
            if self.cooldown > 0 {
                return false;
            }
            self.cooldown = self.kind.stats().cooldown_ticks;
            true
        }

        // Rounds missing from the magazine.
        pub fn room(&self) -> u32 {
            self.kind.stats().magazine.saturating_sub(self.ammo)
        }

        // Starts a reload that will add `rounds` (already taken from the player's
        // inventory) when it finishes. False if there is no room, nothing to load
        // or a reload is already running.
        pub fn begin_reload(&mut self, rounds: u32) -> bool {
            if self.reload_left > 0 || rounds == 0 || self.room() == 0 {
                return false;
            }
            self.reload_left = self.kind.stats().reload_ticks;
            self.pending_load = rounds.min(self.room());
            true
        }

        pub fn tick(&mut self) {
            self.cooldown = self.cooldown.saturating_sub(1);
            if self.reload_left > 0 {
                self.reload_left -= 1;
                if self.reload_left == 0 {
                    self.ammo = (self.ammo + self.pending_load).min(self.kind.stats().magazine);
                    self.pending_load = 0;
                }
            }
        }
    }

    // A box a shot can hit. `head_min_y` is the height above which a hit counts
    // as a headshot.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Target {
        pub id: u32,
        pub min: Vec3,
        pub max: Vec3,
        pub head_min_y: f32,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ShotResult {
        // `None` means the projectile hit nothing (or a block).
        pub target: Option<u32>,
        pub distance: f32,
        pub damage: f32,
        pub headshot: bool,
    }

    // Random direction inside a cone of `spread_deg` half-angle around `dir`.
    pub fn spread_direction(dir: Vec3, spread_deg: f32, rng: &mut Rng) -> Vec3 {
        let dir = math::normalize(dir);
        if spread_deg <= 0.0 {
            return dir;
        }
        let helper = if dir[1].abs() < 0.99 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let right = math::normalize(math::cross(dir, helper));
        let up = math::cross(right, dir);
        let angle = spread_deg.to_radians() * rng.next_f32().sqrt();
        let around = std::f32::consts::TAU * rng.next_f32();
        let offset = math::add(
            math::scale(right, angle.sin() * around.cos()),
            math::scale(up, angle.sin() * around.sin()),
        );
        math::normalize(math::add(math::scale(dir, angle.cos()), offset))
    }

    // Resolves every projectile of one trigger pull as a hitscan.
    // `block_distance(origin, dir, max)` reports the distance to the first solid
    // block along a ray, if any; shots stop there. Pass `&|_, _, _| None` for
    // open space. The shooter must not be in `targets`.
    pub fn fire_hitscan(
        kind: GunKind,
        origin: Vec3,
        dir: Vec3,
        targets: &[Target],
        rng: &mut Rng,
        block_distance: &dyn Fn(Vec3, Vec3, f32) -> Option<f32>,
    ) -> Vec<ShotResult> {
        let stats = kind.stats();
        let mut results = Vec::with_capacity(stats.pellets as usize);
        for _ in 0..stats.pellets {
            let pellet_dir = spread_direction(dir, stats.spread_deg, rng);
            let wall = block_distance(origin, pellet_dir, stats.range).unwrap_or(f32::INFINITY);
            let mut best: Option<(f32, &Target)> = None;
            for target in targets {
                if let Some(t) = math::ray_aabb(origin, pellet_dir, target.min, target.max) {
                    let closer = match best {
                        Some((best_t, _)) => t < best_t,
                        None => true,
                    };
                    if t <= stats.range && t < wall && closer {
                        best = Some((t, target));
                    }
                }
            }
            results.push(match best {
                Some((t, target)) => {
                    let hit_y = origin[1] + pellet_dir[1] * t;
                    let headshot = hit_y >= target.head_min_y;
                    ShotResult {
                        target: Some(target.id),
                        distance: t,
                        damage: damage_at(&stats, t, headshot),
                        headshot,
                    }
                }
                None => ShotResult {
                    target: None,
                    distance: wall.min(stats.range),
                    damage: 0.0,
                    headshot: false,
                },
            });
        }
        results
    }
}

// ---------------------------------------------------------------------------
// BBS-style actors: scripted characters that can carry and use guns
// ---------------------------------------------------------------------------
pub mod actors {
    use super::Rng;
    use super::guns::{self, Gun, GunKind, Target};
    use super::math::{self, Vec3};

    pub const ACTOR_WIDTH: f32 = 0.6;
    pub const ACTOR_HEIGHT: f32 = 1.8;
    pub const EYE_HEIGHT: f32 = 1.62;
    // Hits above this height are headshots.
    pub const HEAD_START: f32 = 1.4;
    // How close an actor must be to pick up or hand over a gun.
    pub const REACH: f32 = 2.5;

    #[derive(Clone, Debug)]
    pub struct Actor {
        pub id: u32,
        pub name: String,
        pub pos: Vec3,
        // Degrees. Yaw 0 faces +Z; pitch is positive when looking down.
        pub yaw: f32,
        pub pitch: f32,
        pub health: f32,
        pub max_health: f32,
        pub gun: Option<Gun>,
        // When set, the actor aims at and shoots this actor on its own.
        pub auto_target: Option<u32>,
    }

    impl Actor {
        pub fn new(id: u32, name: &str, pos: Vec3) -> Self {
            Self {
                id,
                name: name.to_string(),
                pos,
                yaw: 0.0,
                pitch: 0.0,
                health: 20.0,
                max_health: 20.0,
                gun: None,
                auto_target: None,
            }
        }

        pub fn alive(&self) -> bool {
            self.health > 0.0
        }

        pub fn eye(&self) -> Vec3 {
            [self.pos[0], self.pos[1] + EYE_HEIGHT, self.pos[2]]
        }

        pub fn look_dir(&self) -> Vec3 {
            let yaw = self.yaw.to_radians();
            let pitch = self.pitch.to_radians();
            [
                -yaw.sin() * pitch.cos(),
                -pitch.sin(),
                yaw.cos() * pitch.cos(),
            ]
        }

        pub fn aim_at(&mut self, point: Vec3) {
            let d = math::sub(point, self.eye());
            let horizontal = (d[0] * d[0] + d[2] * d[2]).sqrt();
            self.yaw = (-d[0]).atan2(d[2]).to_degrees();
            self.pitch = (-d[1]).atan2(horizontal).to_degrees();
        }

        pub fn center(&self) -> Vec3 {
            [self.pos[0], self.pos[1] + ACTOR_HEIGHT * 0.5, self.pos[2]]
        }

        pub fn hitbox(&self) -> Target {
            let half = ACTOR_WIDTH * 0.5;
            Target {
                id: self.id,
                min: [self.pos[0] - half, self.pos[1], self.pos[2] - half],
                max: [
                    self.pos[0] + half,
                    self.pos[1] + ACTOR_HEIGHT,
                    self.pos[2] + half,
                ],
                head_min_y: self.pos[1] + HEAD_START,
            }
        }
    }

    // What an actor can be told to do - by a script, by the AI, or by the player.
    #[derive(Clone, Debug, PartialEq)]
    pub enum ActorAction {
        Move(Vec3),
        Look { yaw: f32, pitch: f32 },
        Aim(Vec3),
        // Spawn a fresh gun straight into the actor's hands.
        Equip(GunKind),
        // Actors fire without ammo, magazines or reloading.
        Fire,
        // Pick up the nearest dropped gun in reach (swapping if already armed).
        Pickup,
        Drop,
        // Hand the held gun to another unarmed actor in reach.
        GiveTo(u32),
        // Use whatever is in reach; currently the same as Pickup.
        Interact,
        // Start or stop automatic combat against an actor.
        Engage(Option<u32>),
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct DroppedGun {
        pub pos: Vec3,
        pub gun: Gun,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum Event {
        Fired {
            actor: u32,
            gun: GunKind,
        },
        Hit {
            shooter: u32,
            target: u32,
            damage: f32,
            headshot: bool,
        },
        Killed {
            shooter: u32,
            target: u32,
        },
        PickedUp {
            actor: u32,
            gun: GunKind,
        },
        Dropped {
            actor: u32,
            gun: GunKind,
        },
        Gave {
            from: u32,
            to: u32,
            gun: GunKind,
        },
    }

    pub struct World {
        pub actors: Vec<Actor>,
        pub dropped: Vec<DroppedGun>,
        pub tick: u64,
        events: Vec<Event>,
        scripts: Vec<(u64, u32, ActorAction)>,
        rng: Rng,
    }

    impl World {
        pub fn new(seed: u64) -> Self {
            Self {
                actors: Vec::new(),
                dropped: Vec::new(),
                tick: 0,
                events: Vec::new(),
                scripts: Vec::new(),
                rng: Rng::new(seed),
            }
        }

        pub fn spawn(&mut self, name: &str, pos: Vec3) -> u32 {
            let id = self.actors.iter().map(|a| a.id).max().map_or(1, |m| m + 1);
            self.actors.push(Actor::new(id, name, pos));
            id
        }

        pub fn actor(&self, id: u32) -> Option<&Actor> {
            self.actors.iter().find(|a| a.id == id)
        }

        pub fn actor_mut(&mut self, id: u32) -> Option<&mut Actor> {
            self.actors.iter_mut().find(|a| a.id == id)
        }

        pub fn drain_events(&mut self) -> Vec<Event> {
            std::mem::take(&mut self.events)
        }

        // Schedule an action at an absolute world tick, like a recorded BBS replay.
        pub fn script(&mut self, at_tick: u64, actor: u32, action: ActorAction) {
            self.scripts.push((at_tick, actor, action));
        }

        // Run one action immediately.
        pub fn perform(&mut self, id: u32, action: ActorAction) {
            self.perform_with(id, action, &|_, _, _| None);
        }

        // Like `perform`, with a block-occlusion query for shots (see
        // `guns::fire_hitscan`).
        pub fn perform_with(
            &mut self,
            id: u32,
            action: ActorAction,
            block_distance: &dyn Fn(Vec3, Vec3, f32) -> Option<f32>,
        ) {
            let Some(idx) = self.actors.iter().position(|a| a.id == id) else {
                return;
            };
            if !self.actors[idx].alive() {
                return;
            }
            match action {
                ActorAction::Move(pos) => self.actors[idx].pos = pos,
                ActorAction::Look { yaw, pitch } => {
                    self.actors[idx].yaw = yaw;
                    self.actors[idx].pitch = pitch.clamp(-90.0, 90.0);
                }
                ActorAction::Aim(point) => self.actors[idx].aim_at(point),
                ActorAction::Equip(kind) => self.actors[idx].gun = Some(Gun::new(kind)),
                ActorAction::Fire => self.fire(idx, block_distance),
                ActorAction::Pickup | ActorAction::Interact => self.pickup(idx),
                ActorAction::Drop => self.drop_gun(idx),
                ActorAction::GiveTo(other) => self.give(idx, other),
                ActorAction::Engage(target) => self.actors[idx].auto_target = target,
            }
        }

        fn fire(&mut self, idx: usize, block_distance: &dyn Fn(Vec3, Vec3, f32) -> Option<f32>) {
            let id = self.actors[idx].id;
            let origin = self.actors[idx].eye();
            let dir = self.actors[idx].look_dir();
            let kind = match self.actors[idx].gun.as_mut() {
                None => return,
                Some(gun) => {
                    if !gun.try_fire_free() {
                        return;
                    }
                    gun.kind
                }
            };
            self.events.push(Event::Fired {
                actor: id,
                gun: kind,
            });

            let targets: Vec<Target> = self
                .actors
                .iter()
                .filter(|a| a.alive() && a.id != id)
                .map(Actor::hitbox)
                .collect();
            let results =
                guns::fire_hitscan(kind, origin, dir, &targets, &mut self.rng, block_distance);

            for shot in results {
                let Some(target_id) = shot.target else {
                    continue;
                };
                let Some(tidx) = self.actors.iter().position(|a| a.id == target_id) else {
                    continue;
                };
                if !self.actors[tidx].alive() {
                    continue;
                }
                self.actors[tidx].health -= shot.damage;
                self.events.push(Event::Hit {
                    shooter: id,
                    target: target_id,
                    damage: shot.damage,
                    headshot: shot.headshot,
                });
                if !self.actors[tidx].alive() {
                    self.events.push(Event::Killed {
                        shooter: id,
                        target: target_id,
                    });
                    // A fallen actor leaves its gun behind for others to pick up.
                    if let Some(gun) = self.actors[tidx].gun.take() {
                        let pos = self.actors[tidx].pos;
                        self.events.push(Event::Dropped {
                            actor: target_id,
                            gun: gun.kind,
                        });
                        self.dropped.push(DroppedGun { pos, gun });
                    }
                    self.actors[tidx].auto_target = None;
                }
            }
        }

        fn drop_gun(&mut self, idx: usize) {
            let id = self.actors[idx].id;
            let pos = self.actors[idx].pos;
            if let Some(gun) = self.actors[idx].gun.take() {
                self.events.push(Event::Dropped {
                    actor: id,
                    gun: gun.kind,
                });
                self.dropped.push(DroppedGun { pos, gun });
            }
        }

        fn pickup(&mut self, idx: usize) {
            let id = self.actors[idx].id;
            let pos = self.actors[idx].pos;
            let nearest = self
                .dropped
                .iter()
                .enumerate()
                .map(|(i, d)| (i, math::distance(pos, d.pos)))
                .filter(|&(_, dist)| dist <= REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            let Some(i) = nearest else {
                return;
            };
            let picked = self.dropped.remove(i);
            let kind = picked.gun.kind;
            if let Some(old) = self.actors[idx].gun.replace(picked.gun) {
                self.events.push(Event::Dropped {
                    actor: id,
                    gun: old.kind,
                });
                self.dropped.push(DroppedGun { pos, gun: old });
            }
            self.events.push(Event::PickedUp {
                actor: id,
                gun: kind,
            });
        }

        fn give(&mut self, idx: usize, other: u32) {
            let from = self.actors[idx].id;
            let Some(oidx) = self.actors.iter().position(|a| a.id == other) else {
                return;
            };
            if oidx == idx
                || !self.actors[oidx].alive()
                || self.actors[oidx].gun.is_some()
                || math::distance(self.actors[idx].pos, self.actors[oidx].pos) > REACH
            {
                return;
            }
            if let Some(gun) = self.actors[idx].gun.take() {
                let kind = gun.kind;
                self.actors[oidx].gun = Some(gun);
                self.events.push(Event::Gave {
                    from,
                    to: other,
                    gun: kind,
                });
            }
        }

        // Advance one game tick: due scripted actions, then autonomous combat,
        // then gun cooldowns.
        pub fn step(&mut self) {
            self.step_with(&|_, _, _| None);
        }

        pub fn step_with(&mut self, block_distance: &dyn Fn(Vec3, Vec3, f32) -> Option<f32>) {
            let now = self.tick;
            let due: Vec<(u32, ActorAction)> = self
                .scripts
                .iter()
                .filter(|(t, _, _)| *t == now)
                .map(|(_, id, action)| (*id, action.clone()))
                .collect();
            for (id, action) in due {
                self.perform_with(id, action, block_distance);
            }

            let fighters: Vec<(u32, u32)> = self
                .actors
                .iter()
                .filter(|a| a.alive())
                .filter_map(|a| a.auto_target.map(|t| (a.id, t)))
                .collect();
            for (id, target_id) in fighters {
                let point = self
                    .actor(target_id)
                    .filter(|t| t.alive())
                    .map(Actor::center);
                let Some(point) = point else {
                    if let Some(a) = self.actor_mut(id) {
                        a.auto_target = None;
                    }
                    continue;
                };
                self.perform_with(id, ActorAction::Aim(point), block_distance);
                self.perform_with(id, ActorAction::Fire, block_distance);
            }

            for actor in &mut self.actors {
                if let Some(gun) = actor.gun.as_mut() {
                    gun.tick();
                }
            }
            for dropped in &mut self.dropped {
                dropped.gun.tick();
            }
            self.tick += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Items, inventory and crafting (ammo and magazines)
// ---------------------------------------------------------------------------
pub mod items {
    use super::guns::GunKind;

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub enum Item {
        IronIngot,
        GoldIngot,
        IronNugget,
        GoldNugget,
        RedstoneDust,
        Diamond,
        HeavyAmmo,
        RifleAmmo,
        ShotgunShell,
        PistolAmmo,
        RifleMag,
        PistolMag,
        // Placeable block that F10 recolours; see the `chroma` module.
        WhiteBlock,
    }

    impl Item {
        pub fn max_stack(self) -> u32 {
            64
        }
    }

    // The ammunition a gun loads.
    pub fn ammo_for(kind: GunKind) -> Item {
        match kind {
            GunKind::Pistol => Item::PistolAmmo,
            GunKind::Rifle => Item::RifleAmmo,
            GunKind::Sniper => Item::HeavyAmmo,
            GunKind::Shotgun => Item::ShotgunShell,
        }
    }

    // Pistols and rifles also need their (reusable) magazine item in the
    // inventory to reload; sniper and shotgun load ammo directly.
    pub fn mag_for(kind: GunKind) -> Option<Item> {
        match kind {
            GunKind::Pistol => Some(Item::PistolMag),
            GunKind::Rifle => Some(Item::RifleMag),
            GunKind::Sniper | GunKind::Shotgun => None,
        }
    }

    pub const HOTBAR_SLOTS: usize = 9;
    pub const SLOTS: usize = 36;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Stack {
        pub item: Item,
        pub count: u32,
    }

    // Slots 0..9 are the hotbar, 9..36 the main inventory.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Inventory {
        pub slots: [Option<Stack>; SLOTS],
    }

    impl Default for Inventory {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Inventory {
        pub fn new() -> Self {
            Self {
                slots: [None; SLOTS],
            }
        }

        pub fn count(&self, item: Item) -> u32 {
            self.slots
                .iter()
                .flatten()
                .filter(|s| s.item == item)
                .map(|s| s.count)
                .sum()
        }

        // Adds items, topping up existing stacks first. Returns what didn't fit.
        pub fn add(&mut self, item: Item, mut amount: u32) -> u32 {
            let max = item.max_stack();
            for stack in self.slots.iter_mut().flatten() {
                if amount == 0 {
                    break;
                }
                if stack.item == item && stack.count < max {
                    let moved = (max - stack.count).min(amount);
                    stack.count += moved;
                    amount -= moved;
                }
            }
            for slot in self.slots.iter_mut() {
                if amount == 0 {
                    break;
                }
                if slot.is_none() {
                    let moved = max.min(amount);
                    *slot = Some(Stack { item, count: moved });
                    amount -= moved;
                }
            }
            amount
        }

        // Removes up to `amount`, hotbar first, then the main inventory. Returns
        // how many were removed.
        pub fn take_up_to(&mut self, item: Item, amount: u32) -> u32 {
            let mut left = amount;
            for slot in self.slots.iter_mut() {
                if left == 0 {
                    break;
                }
                if let Some(stack) = slot.as_mut() {
                    if stack.item == item {
                        let taken = stack.count.min(left);
                        stack.count -= taken;
                        left -= taken;
                        if stack.count == 0 {
                            *slot = None;
                        }
                    }
                }
            }
            amount - left
        }

        // All-or-nothing removal.
        pub fn remove(&mut self, item: Item, amount: u32) -> bool {
            if self.count(item) < amount {
                return false;
            }
            self.take_up_to(item, amount);
            true
        }
    }

    // Shapeless recipes: the inputs in any arrangement make the output.
    #[derive(Debug)]
    pub struct Recipe {
        pub name: &'static str,
        pub inputs: &'static [(Item, u32)],
        pub output: Item,
        pub output_count: u32,
    }

    pub const RECIPES: [Recipe; 6] = [
        Recipe {
            name: "sniper / heavy ammo",
            inputs: &[(Item::GoldIngot, 1), (Item::IronIngot, 2)],
            output: Item::HeavyAmmo,
            output_count: 7,
        },
        Recipe {
            name: "rifle ammo",
            inputs: &[(Item::IronIngot, 1), (Item::GoldNugget, 1)],
            output: Item::RifleAmmo,
            output_count: 14,
        },
        Recipe {
            name: "shotgun shells",
            inputs: &[(Item::RedstoneDust, 6), (Item::IronNugget, 3)],
            output: Item::ShotgunShell,
            output_count: 3,
        },
        Recipe {
            name: "pistol ammo",
            inputs: &[(Item::IronNugget, 3)],
            output: Item::PistolAmmo,
            output_count: 9,
        },
        Recipe {
            name: "rifle mag",
            inputs: &[(Item::IronIngot, 6), (Item::Diamond, 1)],
            output: Item::RifleMag,
            output_count: 1,
        },
        Recipe {
            name: "pistol mag",
            inputs: &[(Item::IronIngot, 3), (Item::GoldIngot, 1)],
            output: Item::PistolMag,
            output_count: 1,
        },
    ];

    pub fn recipe_for(output: Item) -> Option<&'static Recipe> {
        RECIPES.iter().find(|r| r.output == output)
    }

    #[derive(Debug, Eq, PartialEq)]
    pub enum CraftError {
        MissingIngredients,
        NoSpace,
    }

    // Crafts once. On error the inventory is left untouched.
    pub fn craft(inventory: &mut Inventory, recipe: &Recipe) -> Result<(), CraftError> {
        if recipe
            .inputs
            .iter()
            .any(|&(item, n)| inventory.count(item) < n)
        {
            return Err(CraftError::MissingIngredients);
        }
        let mut trial = inventory.clone();
        for &(item, n) in recipe.inputs {
            trial.take_up_to(item, n);
        }
        if trial.add(recipe.output, recipe.output_count) > 0 {
            return Err(CraftError::NoSpace);
        }
        *inventory = trial;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Players: carry a gun and reload it from hotbar/inventory ammo
// ---------------------------------------------------------------------------
pub mod player {
    use super::Rng;
    use super::guns::{self, Gun, ShotResult, Target};
    use super::items::{self, Inventory, Item};
    use super::math::Vec3;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ReloadError {
        NoGun,
        AlreadyFull,
        AlreadyReloading,
        // Pistols/rifles need their magazine item in the inventory.
        NeedMag(Item),
        NoAmmo(Item),
    }

    #[derive(Clone, Debug, Default)]
    pub struct Player {
        pub inventory: Inventory,
        pub gun: Option<Gun>,
    }

    impl Player {
        pub fn new() -> Self {
            Self::default()
        }

        // The R key. Takes ammo from the hotbar first, then the inventory, up to
        // what the magazine can hold, and starts the reload. Returns rounds taken.
        pub fn press_reload(&mut self) -> Result<u32, ReloadError> {
            let gun = self.gun.as_mut().ok_or(ReloadError::NoGun)?;
            if gun.is_reloading() {
                return Err(ReloadError::AlreadyReloading);
            }
            if gun.room() == 0 {
                return Err(ReloadError::AlreadyFull);
            }
            if let Some(mag) = items::mag_for(gun.kind) {
                if self.inventory.count(mag) == 0 {
                    return Err(ReloadError::NeedMag(mag));
                }
            }
            let ammo = items::ammo_for(gun.kind);
            let taken = self.inventory.take_up_to(ammo, gun.room());
            if taken == 0 {
                return Err(ReloadError::NoAmmo(ammo));
            }
            gun.begin_reload(taken);
            Ok(taken)
        }

        // Pull the trigger. Empty if the gun can't fire (no gun, empty,
        // cooling down, reloading).
        pub fn fire(
            &mut self,
            origin: Vec3,
            dir: Vec3,
            targets: &[Target],
            rng: &mut Rng,
            block_distance: &dyn Fn(Vec3, Vec3, f32) -> Option<f32>,
        ) -> Vec<ShotResult> {
            let Some(gun) = self.gun.as_mut() else {
                return Vec::new();
            };
            if !gun.try_fire() {
                return Vec::new();
            }
            guns::fire_hitscan(gun.kind, origin, dir, targets, rng, block_distance)
        }

        pub fn tick(&mut self) {
            if let Some(gun) = self.gun.as_mut() {
                gun.tick();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// White blocks that F10 recolours (colours, chroma-key green/blue, back to vanilla)
// ---------------------------------------------------------------------------
pub mod chroma {
    use super::math::{self, Vec3};
    use std::collections::HashMap;

    pub const COLOURS: [[u8; 3]; 7] = [
        [220, 50, 50],
        [240, 140, 40],
        [240, 220, 60],
        [60, 200, 220],
        [60, 90, 220],
        [150, 70, 200],
        [240, 130, 190],
    ];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ChromaState {
        White,
        Colour([u8; 3]),
        // Flat chroma-key green / blue for compositing.
        ChromaGreen,
        ChromaBlue,
        // Back to the normal Minecraft block look.
        Vanilla,
    }

    impl ChromaState {
        // Flat colour to draw, or `None` to use the normal Minecraft texture.
        pub fn rgb(self) -> Option<[u8; 3]> {
            match self {
                ChromaState::White => Some([255, 255, 255]),
                ChromaState::Colour(c) => Some(c),
                ChromaState::ChromaGreen => Some([0, 177, 64]),
                ChromaState::ChromaBlue => Some([0, 71, 187]),
                ChromaState::Vanilla => None,
            }
        }

        // The F10 cycle: white -> 7 colours -> chroma green -> chroma blue ->
        // vanilla -> white.
        pub fn next(self) -> Self {
            match self {
                ChromaState::White => ChromaState::Colour(COLOURS[0]),
                ChromaState::Colour(c) => match COLOURS.iter().position(|&x| x == c) {
                    Some(i) if i + 1 < COLOURS.len() => ChromaState::Colour(COLOURS[i + 1]),
                    _ => ChromaState::ChromaGreen,
                },
                ChromaState::ChromaGreen => ChromaState::ChromaBlue,
                ChromaState::ChromaBlue => ChromaState::Vanilla,
                ChromaState::Vanilla => ChromaState::White,
            }
        }
    }

    // Block positions that are chroma blocks, and their state.
    #[derive(Clone, Debug, Default)]
    pub struct ChromaWorld {
        pub blocks: HashMap<[i32; 3], ChromaState>,
    }

    impl ChromaWorld {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn place_white(&mut self, pos: [i32; 3]) {
            self.blocks.insert(pos, ChromaState::White);
        }

        pub fn remove(&mut self, pos: [i32; 3]) -> Option<ChromaState> {
            self.blocks.remove(&pos)
        }

        // First chroma block on the ray within `reach` blocks.
        pub fn looking_at(&self, origin: Vec3, dir: Vec3, reach: f32) -> Option<[i32; 3]> {
            let dir = math::normalize(dir);
            let steps = (reach / 0.05).ceil() as u32;
            for i in 0..=steps {
                let t = i as f32 * 0.05;
                let p = math::add(origin, math::scale(dir, t));
                let cell = [
                    p[0].floor() as i32,
                    p[1].floor() as i32,
                    p[2].floor() as i32,
                ];
                if self.blocks.contains_key(&cell) {
                    return Some(cell);
                }
            }
            None
        }

        // The F10 key: advance the chroma block under the crosshair, if any.
        pub fn press_f10(&mut self, origin: Vec3, dir: Vec3, reach: f32) -> Option<ChromaState> {
            let pos = self.looking_at(origin, dir, reach)?;
            let state = self.blocks.get_mut(&pos)?;
            *state = state.next();
            Some(*state)
        }
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------
pub mod input {
    use super::chroma::{ChromaState, ChromaWorld};
    use super::math::Vec3;
    use super::player::{Player, ReloadError};

    pub const BLOCK_REACH: f32 = 6.0;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Key {
        R,
        F10,
    }

    #[derive(Debug, Eq, PartialEq)]
    pub enum KeyOutcome {
        Reload(Result<u32, ReloadError>),
        // The new state of the block that was looked at, if there was one.
        Chroma(Option<ChromaState>),
    }

    pub fn handle_key(
        key: Key,
        player: &mut Player,
        chroma: &mut ChromaWorld,
        eye: Vec3,
        look_dir: Vec3,
    ) -> KeyOutcome {
        match key {
            Key::R => KeyOutcome::Reload(player.press_reload()),
            Key::F10 => KeyOutcome::Chroma(chroma.press_f10(eye, look_dir, BLOCK_REACH)),
        }
    }
}

// ---------------------------------------------------------------------------
// 3D gun models: the sprite sheet guns, extruded one voxel per pixel
// ---------------------------------------------------------------------------
pub mod models {
    use super::guns::GunKind;
    use super::math::Vec3;

    // Indexed-colour sprite. `rows` hold one hex digit (palette index) per
    // pixel, or '.' for transparent.
    #[derive(Debug)]
    pub struct Sprite {
        pub width: usize,
        pub height: usize,
        pub palette: &'static [[u8; 3]],
        pub rows: &'static [&'static str],
    }

    impl Sprite {
        pub fn pixel(&self, x: usize, y: usize) -> Option<[u8; 3]> {
            let ch = *self.rows.get(y)?.as_bytes().get(x)?;
            let index = (ch as char).to_digit(16)? as usize;
            self.palette.get(index).copied()
        }
    }

    // Generated by tools/gen_gun_assets.py from the supplied sprite sheet. The
    // long guns are levelled so the barrel points left.
    pub mod sprites {
        use super::Sprite;

        pub const RIFLE: Sprite = Sprite {
            width: 94,
            height: 31,
            palette: &[[253, 247, 234], [229, 218, 207], [184, 175, 165], [128, 118, 108], [94, 80, 65], [96, 62, 28], [98, 53, 14], [72, 51, 30], [67, 38, 12], [32, 31, 28], [27, 23, 18], [24, 17, 12], [15, 16, 14], [11, 11, 10], [6, 6, 5], [2, 1, 0]],
            rows: &[
                ".........................10000................................................................",
                ".........................00000...1123c33.....4..43.344444c44ec4f4...............001.0.........",
                "......................232222822aa3eeccd3344f34fe4eeeefe9cceeeeedd40...00.......0000000000.....",
                "..1..............22.24777778887a7fbbddccef9f9daacfcfc999fc99c999fb1.00110.0..1102222227214....",
                "..194..........4fccbaa7745756887be9bb9cca99daa99c9c9aaa9ac99cc9abaa117201001225777756577771...",
                "..2ff2........4de9defa77775566658eadbfc99eaaacccccadcacdcdcccddccbb8788664283b55786665548410..",
                "..2ff4.......3fbe4774a47833632334dbbbeafcecceccccccdcacc9c9accccbbbb465588686666666666645300..",
                "...43c2.....3dd3.3.027432555383558b47bbaaca9aeccaaaaa9a9dd99aaaaabb8755568666666666666658810..",
                "2.2eec432234ff9fe3ffbbb78884888555544b9dacaaa9c9999999eadcaaabbabbd77855565566666666666666100.",
                "2d9eafddffcffdfffeffba758454855557477abdbdcc9ae9ecccc929d29c47777943313878885666665666666410..",
                "3fffafefffcfeeeff3f3399977588787888794a922dcceedeeee4.2....447744743.101338876666666556668100.",
                "2e39cc43.33.191.......30472282222382741.2.4ff9ccdeed4.....77477645811....1.173788656666668100.",
                "....d9.................0....00000..2.....27d9aaccddd4....277288868570.......00132388858558100.",
                "...1.........................0000........27edccac4943111.2313388656811.......0..00131887784000",
                "..........................................bc999aad122332.21130855666700.........00.0101222000.",
                ".........................................2ecadddc3.....2....0138656681................100100..",
                ".........................................aa9ac9cf2...........0376555770.......................",
                ".........................................4e9ccdcf4...........013865688100.....................",
                ".........................................e99c9e9a1............0375556520......................",
                "........................................9fcc9aea4.............01aa758820......................",
                ".......................................37fa99c999.............0.3787840.......................",
                "......................................23ceeedcde................13032.........................",
                ".....................................22fac9c9dd3..................0...........................",
                ".....................................2ecacdcce4...............................................",
                "....................................2fcca9eddc1...............................................",
                "...................................2ecccacecfc2...............................................",
                "...................................3efcccacff3................................................",
                "....................................3ffccaef3.................................................",
                ".....................................4ffcae3..................................................",
                "......................................4fff3...................................................",
                ".......................................444....................................................",
            ],
        };

        pub const SNIPER: Sprite = Sprite {
            width: 92,
            height: 19,
            palette: &[[254, 251, 234], [250, 242, 225], [244, 233, 228], [223, 213, 207], [189, 175, 156], [160, 137, 101], [161, 118, 53], [142, 116, 69], [152, 108, 45], [147, 99, 39], [128, 104, 56], [126, 92, 41], [107, 74, 33], [80, 65, 44], [55, 36, 14], [21, 5, 1]],
            rows: &[
                ".......................................................20...................................",
                ".....................................................2222........22.........................",
                ".....................................................302......4.4344........................",
                "...................................1.1211.....3aa2.453dd5522a3dfdfd21................100....",
                "................2...........1122...220013dddddfedef4fffffffffffdffe714040.......1..20310....",
                ".............0111132..02.0041144.444c44dffffffffffffecfdeeeed57db78adbc441...01001152343.0..",
                "............1111142254444fd4dcccce6d9b79efcefeecce77e77aba8955689889b88cbc3413345c55bcccd5..",
                ".3433.....13344e44ccbcdacfc7778956698989b9c8699968888cc6b9888b989888889999c5baca7c6698bbc33.",
                ".45444.34afdda776b9bb67aaf778999998699666688696666998ee99b9b7a9b88c86688689cb86666989696650.",
                "34feeeffffef777866968bb77fd768986686999666988997b77aabba6b9aaabdccec996966966b89669966668500",
                "5fffffa77dffda66669686accccac89bbb89bc9cbbbaad5cbca7a7eaeefedfdd1db7ebc99b889b9699998998cc1.",
                "a.d544444edff7ccadb6bcccaf7ca7a1cc44b4577701444505a2373d5ffffff13ed.455cb778bbbb6b899998651.",
                "..4dddd33eef5cd44d444134.11000110...11011.0001.00000000054dfffd5dd553133c7ad43c5ccc98898a552",
                "...43.......5000000.0....00000.......1100..0.......0220....4.355553533155c32411447c7c8b7da2.",
                ".............000...........0.........2120.........................3.......30002011114a4dd73.",
                "...................222...............222........................................022021402...",
                "...................222...........................................................222223.....",
                "...................22............................................................222........",
                ".................................................................................22.........",
            ],
        };

        pub const SHOTGUN: Sprite = Sprite {
            width: 89,
            height: 15,
            palette: &[[208, 208, 208], [175, 175, 175], [107, 107, 107], [56, 56, 56], [33, 33, 33], [24, 24, 24], [22, 22, 22], [19, 19, 19], [16, 16, 16], [14, 14, 14], [12, 12, 12], [10, 10, 10], [7, 7, 7], [4, 4, 4], [2, 2, 2], [0, 0, 0]],
            rows: &[
                ".................................................10.00..04301............................",
                "..........................................0041134164d44c5ccc250..........................",
                "..............................0..22.122223f34ce6dfa6d96ff5a5f5c24.....10....2.20000.11.1.",
                "....................0..0..003003f2eae6fdf886a88674a6686547cccd5f451.013343bfff6f33ea15c..",
                "......0.......00003.433d33dc39f99fbb9d877755f8e8cc767666aac5a5589bb33db4a668c5cf6cbbff94.",
                "001130043.333a3dcddab6fd7bdcd8866bbb66b7fe9585aa4e67777645caadfbb788f5c5e8c5c5bda5a56f43.",
                ".47534bffbcadfdff58ca6768887b6baaa97ecbdc88aa577355878768e77a8c58577798be8d598aeaa866ce30",
                ".f7ffcccc3334313316cab6b88a665a558c6e88f5cfba96f8a5988cd2558d8da4bc7ac87a86999a57aac55bb0",
                "22401325220131122f386868588456554446b45f8bcc44088122ae5.203441f3fbbcc4f4989999a9984dccf30",
                "...0.7b8f3ee3ffbcfe66c54f468664aa9f104000021.......1.250.44.2.002b95c584a799995cb65777730",
                "....1f58baa6a85b43cc459ffdffda2da32.....................1221.....39cc5e8dcac9cc586b7cc7c.",
                "....22ff288018.0112223332.22..20000......................11.1....1b558c.212bbfffa687cc7f.",
                ".....2.32..........000..................................1.1.......1e4200...0202fbf587ca33",
                "..................................................................0100.........1.052aff9.",
                "...................................................................................0.3e2.",
            ],
        };

        pub const PISTOL: Sprite = Sprite {
            width: 22,
            height: 20,
            palette: &[[196, 196, 196], [163, 163, 163], [133, 133, 133], [70, 70, 70], [38, 38, 38], [28, 28, 28], [23, 23, 23], [21, 21, 21], [19, 19, 19], [16, 16, 16], [14, 14, 14], [11, 11, 11], [8, 8, 8], [4, 4, 4], [2, 2, 2], [0, 0, 0]],
            rows: &[
                "................12bb2.",
                ".20...21.12112da668ec.",
                "32d29a26caacdfccf55883",
                "1f7b9acd9e5866a45dd4d3",
                "197595864c7f557eea4aff",
                "18cae66f6e6eaffb8fccf1",
                "159a45d7c99c2f5944440.",
                "054bccdd35.2.20.01....",
                ".0394498213...........",
                "..aab6b6c1............",
                "..36c9760.2...........",
                ".045568f41............",
                ".11b85552.............",
                "..f855c91.............",
                ".0ccc9533.............",
                "..46999d0.............",
                ".1a4479b..............",
                ".00f9aab1.............",
                "..934433..............",
                "..000.................",
            ],
        };
    }

    #[derive(Clone, Debug, Default)]
    pub struct Mesh {
        pub positions: Vec<Vec3>,
        pub colors: Vec<[u8; 3]>,
        // Triangles, counter-clockwise seen from outside.
        pub indices: Vec<u32>,
    }

    impl Mesh {
        pub fn quad_count(&self) -> usize {
            self.indices.len() / 6
        }
    }

    pub const DEPTH_VOXELS: u32 = 3;

    pub fn sprite_for(kind: GunKind) -> &'static Sprite {
        match kind {
            GunKind::Pistol => &sprites::PISTOL,
            GunKind::Rifle => &sprites::RIFLE,
            GunKind::Sniper => &sprites::SNIPER,
            GunKind::Shotgun => &sprites::SHOTGUN,
        }
    }

    // Model length in blocks.
    pub fn length_for(kind: GunKind) -> f32 {
        match kind {
            GunKind::Pistol => 0.3,
            GunKind::Rifle => 0.9,
            GunKind::Sniper => 1.1,
            GunKind::Shotgun => 0.95,
        }
    }

    // Extrudes every opaque pixel into a voxel `depth_voxels` thick and keeps
    // only exposed faces. X runs along the barrel, Y is up, Z is the thickness;
    // the model spans `length` blocks in X and is centred on Z = 0.
    pub fn extrude(sprite: &Sprite, depth_voxels: u32, length: f32) -> Mesh {
        let mut mesh = Mesh::default();
        if sprite.width == 0 {
            return mesh;
        }
        let s = length / sprite.width as f32;
        let half = depth_voxels as f32 * s * 0.5;
        let solid = |x: i64, y: i64| -> bool {
            x >= 0 && y >= 0 && sprite.pixel(x as usize, y as usize).is_some()
        };
        let mut quad = |pts: [Vec3; 4], color: [u8; 3]| {
            let base = mesh.positions.len() as u32;
            mesh.positions.extend_from_slice(&pts);
            mesh.colors.extend_from_slice(&[color; 4]);
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        for y in 0..sprite.height {
            for x in 0..sprite.width {
                let Some(color) = sprite.pixel(x, y) else {
                    continue;
                };
                let (xi, yi) = (x as i64, y as i64);
                let x0 = x as f32 * s;
                let x1 = x0 + s;
                let y0 = (sprite.height - y - 1) as f32 * s;
                let y1 = y0 + s;
                let (z0, z1) = (-half, half);
                if !solid(xi - 1, yi) {
                    quad(
                        [[x0, y0, z1], [x0, y0, z0], [x0, y1, z0], [x0, y1, z1]],
                        color,
                    );
                }
                if !solid(xi + 1, yi) {
                    quad(
                        [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]],
                        color,
                    );
                }
                if !solid(xi, yi - 1) {
                    quad(
                        [[x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0]],
                        color,
                    );
                }
                if !solid(xi, yi + 1) {
                    quad(
                        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
                        color,
                    );
                }
                quad(
                    [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
                    color,
                );
                quad(
                    [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
                    color,
                );
            }
        }
        mesh
    }

    pub fn mesh_for(kind: GunKind) -> Mesh {
        extrude(sprite_for(kind), DEPTH_VOXELS, length_for(kind))
    }

    // Where a held gun sits relative to the hand. Starting values: tune them
    // in-game, they have not been seen on screen.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct HeldPose {
        pub offset: Vec3,
        pub rotation_deg: Vec3,
        pub scale: f32,
    }

    pub fn held_pose(kind: GunKind) -> HeldPose {
        match kind {
            GunKind::Pistol => HeldPose {
                offset: [0.30, -0.28, -0.45],
                rotation_deg: [0.0, 90.0, 0.0],
                scale: 1.0,
            },
            GunKind::Rifle | GunKind::Shotgun => HeldPose {
                offset: [0.28, -0.30, -0.35],
                rotation_deg: [0.0, 90.0, 0.0],
                scale: 1.0,
            },
            GunKind::Sniper => HeldPose {
                offset: [0.28, -0.28, -0.30],
                rotation_deg: [0.0, 90.0, 0.0],
                scale: 1.0,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Rust Alkaline v2 - conservative renderer optimisation policy core.
//
// These are safe policy helpers; they do not issue GPU commands. A renderer
// adapter must supply trustworthy visibility evidence before they have any
// in-game effect. Unknown visibility means render.
// ---------------------------------------------------------------------------
pub mod alkaline {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ChunkPos {
        pub x: i32,
        pub z: i32,
    }

    impl ChunkPos {
        pub fn distance_squared(self, other: Self) -> u64 {
            let dx = i128::from(self.x) - i128::from(other.x);
            let dz = i128::from(self.z) - i128::from(other.z);
            (dx * dx + dz * dz).min(i128::from(u64::MAX)) as u64
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Config {
        pub full_detail_radius: u32,
        pub deep_terrain_max_y: i32,
        pub high_lod_until: u32,
        pub medium_lod_until: u32,
        pub entity_occlusion_frames: u8,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                full_detail_radius: 16,
                deep_terrain_max_y: 51,
                high_lod_until: 32,
                medium_lod_until: 64,
                entity_occlusion_frames: 3,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum LodTier {
        Full,
        High,
        Medium,
        Low,
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct SectionEvidence {
        pub max_y: i32,
        pub fully_enclosed: bool,
        pub has_open_cave_or_ravine: bool,
        pub intersects_camera_frustum: bool,
        pub contains_translucent_or_special_geometry: bool,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum TerrainDecision {
        Keep,
        SkipOutsideFrustum,
        SkipDeepEnclosed,
        Simplify(LodTier),
    }

    pub fn terrain_decision(
        camera: ChunkPos,
        section_chunk: ChunkPos,
        evidence: SectionEvidence,
        config: Config,
    ) -> TerrainDecision {
        if !evidence.intersects_camera_frustum {
            return TerrainDecision::SkipOutsideFrustum;
        }

        let radius_sq = u64::from(config.full_detail_radius) * u64::from(config.full_detail_radius);
        let distance_sq = camera.distance_squared(section_chunk);

        if distance_sq > radius_sq
            && evidence.max_y <= config.deep_terrain_max_y
            && evidence.fully_enclosed
            && !evidence.has_open_cave_or_ravine
            && !evidence.contains_translucent_or_special_geometry
        {
            return TerrainDecision::SkipDeepEnclosed;
        }

        let distance = integer_sqrt(distance_sq);
        let tier = if distance <= u64::from(config.full_detail_radius) {
            LodTier::Full
        } else if distance <= u64::from(config.high_lod_until) {
            LodTier::High
        } else if distance <= u64::from(config.medium_lod_until) {
            LodTier::Medium
        } else {
            LodTier::Low
        };

        if tier == LodTier::Full {
            TerrainDecision::Keep
        } else {
            TerrainDecision::Simplify(tier)
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct EntityEvidence {
        pub intersects_frustum: bool,
        pub occluded_consecutive_frames: u8,
        pub is_translucent_or_special: bool,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum EntityDecision {
        Render,
        SkipOutsideFrustum,
        SkipOccluded,
    }

    pub fn entity_decision(evidence: EntityEvidence, config: Config) -> EntityDecision {
        if !evidence.intersects_frustum {
            return EntityDecision::SkipOutsideFrustum;
        }
        if !evidence.is_translucent_or_special
            && config.entity_occlusion_frames > 0
            && evidence.occluded_consecutive_frames >= config.entity_occlusion_frames
        {
            return EntityDecision::SkipOccluded;
        }
        EntityDecision::Render
    }

    fn integer_sqrt(n: u64) -> u64 {
        if n < 2 {
            return n;
        }
        let mut x = n;
        let mut y = x / 2 + 1;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
    }
}

// The only hook target verified for the pinned Horizon commit is
// `Game::break_block` (from the official template). It stays inert on purpose:
// nothing in the game calls guns/actors/Alkaline yet. See README.md.
pub fn before_break(_pos: [i32; 3]) {}

#[cfg(test)]
mod tests {
    use super::actors::{ActorAction, Event, World};
    use super::alkaline::{
        ChunkPos, Config, EntityDecision, EntityEvidence, SectionEvidence, TerrainDecision,
        entity_decision, terrain_decision,
    };
    use super::chroma::{self, ChromaState, ChromaWorld};
    use super::guns::{self, Gun, GunKind, Target};
    use super::input::{Key, KeyOutcome, handle_key};
    use super::items::{self, Inventory, Item};
    use super::models::{self, Sprite};
    use super::player::{Player, ReloadError};
    use super::{Rng, math};

    fn open(_: math::Vec3, _: math::Vec3, _: f32) -> Option<f32> {
        None
    }

    fn standing_target(id: u32, z: f32) -> Target {
        Target {
            id,
            min: [-0.3, 0.0, z - 0.3],
            max: [0.3, 1.8, z + 0.3],
            head_min_y: 1.4,
        }
    }

    // ---- guns ----

    #[test]
    fn four_guns_deal_different_damage() {
        let damages: Vec<f32> = GunKind::ALL.iter().map(|k| k.stats().damage).collect();
        for i in 0..damages.len() {
            for j in (i + 1)..damages.len() {
                assert_ne!(damages[i], damages[j]);
            }
        }
        let pistol = GunKind::Pistol.stats().damage;
        let rifle = GunKind::Rifle.stats().damage;
        let sniper = GunKind::Sniper.stats().damage;
        assert!(pistol < rifle && rifle < sniper);
    }

    #[test]
    fn single_projectile_guns_hit_for_their_listed_damage_at_close_range() {
        for kind in [GunKind::Pistol, GunKind::Rifle, GunKind::Sniper] {
            let mut rng = Rng::new(7);
            let shots = guns::fire_hitscan(
                kind,
                [0.0, 0.9, 0.0],
                [0.0, 0.0, 1.0],
                &[standing_target(1, 3.0)],
                &mut rng,
                &open,
            );
            assert_eq!(shots.len(), 1);
            assert_eq!(shots[0].target, Some(1));
            assert!((shots[0].damage - kind.stats().damage).abs() < 1e-3);
        }
    }

    #[test]
    fn shotgun_fires_many_pellets_and_is_strong_up_close_weak_far_away() {
        let stats = GunKind::Shotgun.stats();
        assert_eq!(stats.pellets, 8);
        let near = guns::damage_at(&stats, 2.0, false) * f32::from(stats.pellets);
        let far = guns::damage_at(&stats, 19.0, false) * f32::from(stats.pellets);
        assert!(near > far * 3.0);
        assert_eq!(guns::damage_at(&stats, 25.0, false), 0.0);
    }

    #[test]
    fn falloff_and_headshots() {
        let stats = GunKind::Rifle.stats();
        let close = guns::damage_at(&stats, 10.0, false);
        let far = guns::damage_at(&stats, 75.0, false);
        assert!(far < close);
        let head = guns::damage_at(&stats, 10.0, true);
        assert!((head - close * stats.headshot_multiplier).abs() < 1e-3);
        assert_eq!(guns::damage_at(&stats, stats.range + 1.0, false), 0.0);
    }

    #[test]
    fn sniper_headshot_kills_a_full_health_actor_in_one_shot() {
        let stats = GunKind::Sniper.stats();
        assert!(guns::damage_at(&stats, 50.0, true) >= 20.0);
        assert!(guns::damage_at(&stats, 50.0, false) < 50.0);
    }

    #[test]
    fn gun_cooldown_ammo_and_reload() {
        let mut gun = Gun::new(GunKind::Sniper);
        assert!(gun.try_fire());
        assert!(!gun.try_fire(), "cooldown blocks the next shot");
        for _ in 0..GunKind::Sniper.stats().cooldown_ticks {
            gun.tick();
        }
        assert!(gun.can_fire());
        gun.ammo = 0;
        assert!(!gun.try_fire());
        assert!(!gun.begin_reload(0), "nothing to load");
        assert!(gun.begin_reload(5));
        assert!(!gun.begin_reload(5), "already reloading");
        assert!(!gun.can_fire());
        for _ in 0..GunKind::Sniper.stats().reload_ticks {
            gun.tick();
        }
        assert_eq!(gun.ammo, GunKind::Sniper.stats().magazine);
        assert!(!gun.begin_reload(3), "full magazine");

        // A partial reload only adds the rounds it was given.
        let mut rifle = Gun::new(GunKind::Rifle);
        rifle.ammo = 10;
        assert!(rifle.begin_reload(8));
        for _ in 0..GunKind::Rifle.stats().reload_ticks {
            rifle.tick();
        }
        assert_eq!(rifle.ammo, 18);
    }

    #[test]
    fn walls_stop_shots() {
        let wall = |_: math::Vec3, _: math::Vec3, _: f32| Some(1.5);
        let mut rng = Rng::new(3);
        let shots = guns::fire_hitscan(
            GunKind::Rifle,
            [0.0, 0.9, 0.0],
            [0.0, 0.0, 1.0],
            &[standing_target(1, 5.0)],
            &mut rng,
            &wall,
        );
        assert_eq!(shots[0].target, None);
    }

    #[test]
    fn spread_stays_inside_the_cone() {
        let mut rng = Rng::new(11);
        let dir = [0.0, 0.0, 1.0];
        for _ in 0..200 {
            let d = guns::spread_direction(dir, 6.0, &mut rng);
            let angle = math::dot(d, dir).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(angle <= 6.0 + 1e-2);
            assert!((math::length(d) - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn gun_names_round_trip() {
        for kind in GunKind::ALL {
            assert_eq!(GunKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(GunKind::from_name("RIFLE"), Some(GunKind::Rifle));
        assert_eq!(GunKind::from_name("bazooka"), None);
    }

    // ---- actors ----

    fn duel() -> (World, u32, u32) {
        let mut world = World::new(42);
        let shooter = world.spawn("shooter", [0.0, 0.0, 0.0]);
        let victim = world.spawn("victim", [0.0, 0.0, 6.0]);
        (world, shooter, victim)
    }

    #[test]
    fn actor_aims_and_fires_a_gun() {
        let (mut world, shooter, victim) = duel();
        world.perform(shooter, ActorAction::Equip(GunKind::Sniper));
        let center = world.actor(victim).unwrap().center();
        world.perform(shooter, ActorAction::Aim(center));
        world.perform(shooter, ActorAction::Fire);
        let events = world.drain_events();
        assert!(events.contains(&Event::Fired {
            actor: shooter,
            gun: GunKind::Sniper
        }));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::Hit { target, .. } if *target == victim))
        );
        assert!(world.actor(victim).unwrap().health < 20.0);
    }

    #[test]
    fn unarmed_actor_cannot_fire() {
        let (mut world, shooter, _) = duel();
        world.perform(shooter, ActorAction::Fire);
        assert!(world.drain_events().is_empty());
    }

    #[test]
    fn auto_engaged_actor_kills_target_reloads_and_victim_drops_gun() {
        let (mut world, shooter, victim) = duel();
        world.perform(shooter, ActorAction::Equip(GunKind::Rifle));
        world.perform(victim, ActorAction::Equip(GunKind::Pistol));
        world.perform(shooter, ActorAction::Engage(Some(victim)));
        for _ in 0..200 {
            world.step();
        }
        let events = world.drain_events();
        assert!(!world.actor(victim).unwrap().alive());
        assert!(events.contains(&Event::Killed {
            shooter,
            target: victim
        }));
        assert_eq!(world.dropped.len(), 1);
        assert_eq!(world.dropped[0].gun.kind, GunKind::Pistol);
        assert_eq!(world.actor(shooter).unwrap().auto_target, None);
    }

    #[test]
    fn pickup_drop_swap_and_give() {
        let mut world = World::new(1);
        let a = world.spawn("a", [0.0, 0.0, 0.0]);
        let b = world.spawn("b", [1.0, 0.0, 0.0]);
        let far = world.spawn("far", [50.0, 0.0, 0.0]);

        world.perform(a, ActorAction::Equip(GunKind::Shotgun));
        world.perform(a, ActorAction::Drop);
        assert!(world.actor(a).unwrap().gun.is_none());
        assert_eq!(world.dropped.len(), 1);

        // Out of reach: nothing happens.
        world.perform(far, ActorAction::Interact);
        assert!(world.actor(far).unwrap().gun.is_none());

        // In reach: b takes the shotgun, then swaps it for a sniper on the ground.
        world.perform(b, ActorAction::Interact);
        assert_eq!(
            world.actor(b).unwrap().gun.as_ref().map(|g| g.kind),
            Some(GunKind::Shotgun)
        );
        world.perform(a, ActorAction::Equip(GunKind::Sniper));
        world.perform(a, ActorAction::Drop);
        world.perform(b, ActorAction::Pickup);
        assert_eq!(
            world.actor(b).unwrap().gun.as_ref().map(|g| g.kind),
            Some(GunKind::Sniper)
        );
        assert_eq!(world.dropped.len(), 1);
        assert_eq!(world.dropped[0].gun.kind, GunKind::Shotgun);

        // Handing over needs an unarmed actor in reach.
        world.perform(b, ActorAction::GiveTo(far));
        assert!(world.actor(b).unwrap().gun.is_some());
        world.perform(b, ActorAction::GiveTo(a));
        assert!(world.actor(b).unwrap().gun.is_none());
        assert_eq!(
            world.actor(a).unwrap().gun.as_ref().map(|g| g.kind),
            Some(GunKind::Sniper)
        );
    }

    #[test]
    fn scripted_actions_play_back_on_their_tick() {
        let (mut world, shooter, victim) = duel();
        world.script(0, shooter, ActorAction::Equip(GunKind::Pistol));
        let center = world.actor(victim).unwrap().center();
        world.script(2, shooter, ActorAction::Aim(center));
        world.script(3, shooter, ActorAction::Fire);
        for _ in 0..3 {
            world.step();
        }
        assert!(
            world.drain_events().is_empty(),
            "nothing fires before tick 3"
        );
        world.step();
        assert!(
            world
                .drain_events()
                .iter()
                .any(|e| matches!(e, Event::Fired { .. }))
        );
    }

    #[test]
    fn actor_aim_matches_look_direction() {
        let (mut world, shooter, victim) = duel();
        let center = world.actor(victim).unwrap().center();
        world.perform(shooter, ActorAction::Aim(center));
        let actor = world.actor(shooter).unwrap();
        let want = math::normalize(math::sub(center, actor.eye()));
        let got = actor.look_dir();
        assert!(math::dot(want, got) > 0.9999);
    }

    #[test]
    fn actors_fire_without_ammo_or_reloading() {
        let (mut world, shooter, victim) = duel();
        world.perform(shooter, ActorAction::Equip(GunKind::Pistol));
        world.actor_mut(shooter).unwrap().gun.as_mut().unwrap().ammo = 0;
        let center = world.actor(victim).unwrap().center();
        world.perform(shooter, ActorAction::Aim(center));
        world.perform(shooter, ActorAction::Fire);
        assert!(
            world
                .drain_events()
                .iter()
                .any(|e| matches!(e, Event::Fired { .. }))
        );
        // Auto-combat never stalls on an empty gun either.
        world.actor_mut(shooter).unwrap().gun.as_mut().unwrap().ammo = 0;
        world.perform(shooter, ActorAction::Engage(Some(victim)));
        for _ in 0..200 {
            world.step();
        }
        assert!(!world.actor(victim).unwrap().alive());
    }

    // ---- items, crafting, reloading ----

    fn stocked(items: &[(Item, u32)]) -> Inventory {
        let mut inv = Inventory::new();
        for &(item, n) in items {
            assert_eq!(inv.add(item, n), 0);
        }
        inv
    }

    #[test]
    fn recipes_match_the_requested_costs_and_yields() {
        let expect: [(Item, &[(Item, u32)], u32); 6] = [
            (
                Item::HeavyAmmo,
                &[(Item::GoldIngot, 1), (Item::IronIngot, 2)],
                7,
            ),
            (
                Item::RifleAmmo,
                &[(Item::IronIngot, 1), (Item::GoldNugget, 1)],
                14,
            ),
            (
                Item::ShotgunShell,
                &[(Item::RedstoneDust, 6), (Item::IronNugget, 3)],
                3,
            ),
            (Item::PistolAmmo, &[(Item::IronNugget, 3)], 9),
            (
                Item::RifleMag,
                &[(Item::IronIngot, 6), (Item::Diamond, 1)],
                1,
            ),
            (
                Item::PistolMag,
                &[(Item::IronIngot, 3), (Item::GoldIngot, 1)],
                1,
            ),
        ];
        for (output, inputs, yield_count) in expect {
            let recipe = items::recipe_for(output).expect("recipe exists");
            assert_eq!(recipe.inputs, inputs);
            assert_eq!(recipe.output_count, yield_count);
            let mut inv = stocked(inputs);
            assert_eq!(items::craft(&mut inv, recipe), Ok(()));
            assert_eq!(inv.count(output), yield_count);
            for &(item, _) in inputs {
                assert_eq!(inv.count(item), 0, "ingredients are consumed");
            }
        }
    }

    #[test]
    fn crafting_fails_cleanly_when_short_or_full() {
        let recipe = items::recipe_for(Item::PistolAmmo).unwrap();
        let mut inv = stocked(&[(Item::IronNugget, 2)]);
        let before = inv.clone();
        assert_eq!(
            items::craft(&mut inv, recipe),
            Err(items::CraftError::MissingIngredients)
        );
        assert_eq!(inv, before);

        // Fill every slot with stone-like clutter (diamonds) so the output can't fit.
        let mut full = Inventory::new();
        assert_eq!(full.add(Item::Diamond, 64 * 35), 0);
        assert_eq!(full.add(Item::IronNugget, 4), 0);
        let before = full.clone();
        assert_eq!(
            items::craft(&mut full, recipe),
            Err(items::CraftError::NoSpace)
        );
        assert_eq!(full, before);
    }

    #[test]
    fn inventory_stacks_and_takes_hotbar_first() {
        let mut inv = Inventory::new();
        assert_eq!(inv.add(Item::RifleAmmo, 100), 0);
        assert_eq!(inv.slots[0].unwrap().count, 64);
        assert_eq!(inv.slots[1].unwrap().count, 36);
        // Move the second stack to the main inventory: the hotbar is drained first.
        inv.slots[20] = inv.slots[1].take();
        assert_eq!(inv.take_up_to(Item::RifleAmmo, 70), 70);
        assert!(inv.slots[0].is_none());
        assert_eq!(inv.slots[20].unwrap().count, 30);
        assert!(!inv.remove(Item::RifleAmmo, 31));
        assert!(inv.remove(Item::RifleAmmo, 30));
    }

    #[test]
    fn r_reloads_from_inventory_and_hotbar_and_needs_a_mag_for_pistol_and_rifle() {
        let mut player = Player::new();
        assert_eq!(player.press_reload(), Err(ReloadError::NoGun));

        player.gun = Some(Gun::new(GunKind::Rifle));
        player.gun.as_mut().unwrap().ammo = 0;
        assert_eq!(
            player.press_reload(),
            Err(ReloadError::NeedMag(Item::RifleMag))
        );
        player.inventory.add(Item::RifleMag, 1);
        assert_eq!(
            player.press_reload(),
            Err(ReloadError::NoAmmo(Item::RifleAmmo))
        );

        // Ammo split between the main inventory (slot 20) and the hotbar (slot 3).
        player.inventory.slots[20] = Some(items::Stack {
            item: Item::RifleAmmo,
            count: 20,
        });
        player.inventory.slots[3] = Some(items::Stack {
            item: Item::RifleAmmo,
            count: 14,
        });
        // The magazine holds 30: hotbar 14 first, then 16 from the inventory.
        assert_eq!(player.press_reload(), Ok(30));
        assert_eq!(player.inventory.count(Item::RifleAmmo), 4);
        assert_eq!(
            player.press_reload(),
            Err(ReloadError::AlreadyReloading)
        );
        assert!(!player.gun.as_ref().unwrap().can_fire());
        for _ in 0..GunKind::Rifle.stats().reload_ticks {
            player.tick();
        }
        assert_eq!(player.gun.as_ref().unwrap().ammo, 30);
        assert_eq!(player.press_reload(), Err(ReloadError::AlreadyFull));
    }

    #[test]
    fn sniper_and_shotgun_reload_without_a_mag_and_use_their_own_ammo() {
        for (kind, ammo_item) in [
            (GunKind::Sniper, Item::HeavyAmmo),
            (GunKind::Shotgun, Item::ShotgunShell),
        ] {
            let mut player = Player::new();
            player.gun = Some(Gun::new(kind));
            player.gun.as_mut().unwrap().ammo = 0;
            // Other guns' ammo is not accepted.
            player.inventory.add(Item::PistolAmmo, 20);
            assert_eq!(
                player.press_reload(),
                Err(ReloadError::NoAmmo(ammo_item))
            );
            player.inventory.add(ammo_item, 3);
            assert_eq!(player.press_reload(), Ok(3));
        }
    }

    #[test]
    fn player_fire_spends_ammo_and_is_blocked_while_reloading() {
        let mut player = Player::new();
        player.gun = Some(Gun::new(GunKind::Pistol));
        let mut rng = Rng::new(5);
        let targets = [standing_target(1, 4.0)];
        let shots = player.fire(
            [0.0, 0.9, 0.0],
            [0.0, 0.0, 1.0],
            &targets,
            &mut rng,
            &open,
        );
        assert_eq!(shots.len(), 1);
        assert_eq!(player.gun.as_ref().unwrap().ammo, 11);
        let again = player.fire(
            [0.0, 0.9, 0.0],
            [0.0, 0.0, 1.0],
            &targets,
            &mut rng,
            &open,
        );
        assert!(again.is_empty(), "cooldown");
    }

    // ---- chroma blocks and keys ----

    #[test]
    fn f10_cycles_a_white_block_through_colours_chroma_and_back_to_vanilla() {
        let mut world = ChromaWorld::new();
        world.place_white([0, 0, 3]);
        let eye = [0.5, 0.5, 0.5];
        let look = [0.0, 0.0, 1.0];
        assert_eq!(world.looking_at(eye, look, 6.0), Some([0, 0, 3]));
        let mut seen = vec![ChromaState::White];
        loop {
            let state = world.press_f10(eye, look, 6.0).unwrap();
            if state == ChromaState::White {
                break;
            }
            seen.push(state);
            assert!(seen.len() < 20, "cycle must terminate");
        }
        // white + 7 colours + chroma green + chroma blue + vanilla
        assert_eq!(seen.len(), 11);
        assert_eq!(seen[1], ChromaState::Colour(chroma::COLOURS[0]));
        assert_eq!(seen[8], ChromaState::ChromaGreen);
        assert_eq!(seen[9], ChromaState::ChromaBlue);
        assert_eq!(seen[10], ChromaState::Vanilla);
        assert_eq!(ChromaState::Vanilla.rgb(), None);
        assert_eq!(ChromaState::White.rgb(), Some([255, 255, 255]));
    }

    #[test]
    fn f10_does_nothing_unless_looking_at_a_chroma_block_in_reach() {
        let mut world = ChromaWorld::new();
        world.place_white([0, 0, 10]);
        assert_eq!(world.press_f10([0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 6.0), None);
        assert_eq!(world.press_f10([0.5, 0.5, 0.5], [0.0, 1.0, 0.0], 6.0), None);
        assert_eq!(world.blocks[&[0, 0, 10]], ChromaState::White);
    }

    #[test]
    fn key_handler_routes_r_and_f10() {
        let mut player = Player::new();
        let mut world = ChromaWorld::new();
        world.place_white([0, 0, 2]);
        let eye = [0.5, 0.5, 0.5];
        let look = [0.0, 0.0, 1.0];
        assert_eq!(
            handle_key(Key::R, &mut player, &mut world, eye, look),
            KeyOutcome::Reload(Err(ReloadError::NoGun))
        );
        assert_eq!(
            handle_key(Key::F10, &mut player, &mut world, eye, look),
            KeyOutcome::Chroma(Some(ChromaState::Colour(chroma::COLOURS[0])))
        );
    }

    // ---- 3D models ----

    #[test]
    fn extrude_keeps_only_exposed_faces() {
        static ONE: Sprite = Sprite {
            width: 1,
            height: 1,
            palette: &[[10, 20, 30]],
            rows: &["0"],
        };
        let mesh = models::extrude(&ONE, 3, 1.0);
        assert_eq!(mesh.quad_count(), 6);
        assert_eq!(mesh.positions.len(), 24);

        static TWO: Sprite = Sprite {
            width: 3,
            height: 1,
            palette: &[[1, 2, 3]],
            rows: &["0.0"],
        };
        assert_eq!(models::extrude(&TWO, 3, 1.0).quad_count(), 12);

        static JOINED: Sprite = Sprite {
            width: 2,
            height: 1,
            palette: &[[1, 2, 3]],
            rows: &["00"],
        };
        // Two touching voxels share a face: 6 + 6 - 2.
        assert_eq!(models::extrude(&JOINED, 3, 1.0).quad_count(), 10);
    }

    #[test]
    fn every_gun_has_a_valid_sprite_and_a_3d_mesh_of_the_right_length() {
        for kind in GunKind::ALL {
            let sprite = models::sprite_for(kind);
            assert!(sprite.width > 0 && sprite.height > 0);
            assert_eq!(sprite.rows.len(), sprite.height);
            let mut opaque = 0;
            for (y, row) in sprite.rows.iter().enumerate() {
                assert_eq!(row.len(), sprite.width, "{} row {y}", kind.name());
                for x in 0..sprite.width {
                    if row.as_bytes()[x] != b'.' {
                        opaque += 1;
                        assert!(sprite.pixel(x, y).is_some(), "palette index in range");
                    }
                }
            }
            assert!(opaque > 20);

            let mesh = models::mesh_for(kind);
            assert!(mesh.quad_count() > 0);
            assert_eq!(mesh.positions.len(), mesh.colors.len());
            assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()));
            let max_x = mesh.positions.iter().map(|p| p[0]).fold(0.0_f32, f32::max);
            assert!(max_x <= models::length_for(kind) + 1e-4);
            assert!(max_x > models::length_for(kind) * 0.9);
        }
    }

    // ---- alkaline ----

    fn enclosed(max_y: i32) -> SectionEvidence {
        SectionEvidence {
            max_y,
            fully_enclosed: true,
            intersects_camera_frustum: true,
            ..SectionEvidence::default()
        }
    }

    #[test]
    fn deep_enclosed_sections_only_cull_beyond_sixteen_chunks() {
        let cfg = Config::default();
        assert_eq!(
            terrain_decision(
                ChunkPos { x: 0, z: 0 },
                ChunkPos { x: 17, z: 0 },
                enclosed(48),
                cfg
            ),
            TerrainDecision::SkipDeepEnclosed
        );
        assert_eq!(
            terrain_decision(
                ChunkPos { x: 0, z: 0 },
                ChunkPos { x: 16, z: 0 },
                enclosed(48),
                cfg
            ),
            TerrainDecision::Keep
        );
    }

    #[test]
    fn caves_and_special_geometry_are_preserved() {
        let cfg = Config::default();
        let cave = SectionEvidence {
            has_open_cave_or_ravine: true,
            ..enclosed(48)
        };
        assert_ne!(
            terrain_decision(ChunkPos { x: 0, z: 0 }, ChunkPos { x: 30, z: 0 }, cave, cfg),
            TerrainDecision::SkipDeepEnclosed
        );
        let special = SectionEvidence {
            contains_translucent_or_special_geometry: true,
            ..enclosed(48)
        };
        assert_ne!(
            terrain_decision(
                ChunkPos { x: 0, z: 0 },
                ChunkPos { x: 30, z: 0 },
                special,
                cfg
            ),
            TerrainDecision::SkipDeepEnclosed
        );
    }

    #[test]
    fn y_above_51_is_not_deep_culled() {
        assert_ne!(
            terrain_decision(
                ChunkPos { x: 0, z: 0 },
                ChunkPos { x: 0, z: 30 },
                enclosed(52),
                Config::default()
            ),
            TerrainDecision::SkipDeepEnclosed
        );
    }

    #[test]
    fn entity_occlusion_requires_three_frames_and_preserves_special_entities() {
        let cfg = Config::default();
        let hidden = EntityEvidence {
            intersects_frustum: true,
            occluded_consecutive_frames: 3,
            is_translucent_or_special: false,
        };
        assert_eq!(entity_decision(hidden, cfg), EntityDecision::SkipOccluded);
        assert_eq!(
            entity_decision(
                EntityEvidence {
                    is_translucent_or_special: true,
                    ..hidden
                },
                cfg
            ),
            EntityDecision::Render
        );
    }
}
