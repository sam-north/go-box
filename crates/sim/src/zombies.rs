//! Survival (`Mode::Survival`, on a map with `Arena::survival`): everyone together against zombies
//! coming in from both sides, faster and tougher the longer it goes on, for as long as anyone's
//! standing. Zombies are bodies in the `ZOMBIES` slots (so every swing, throw, blast and special
//! hurts them), on their own side. They only punch: two punches close together knock you down for
//! a moment, and while you're down two or more of them on you bite, and you turn: your body joins
//! them (`Fighter::zombie`) and can't be revived. Go down any other way and a teammate can revive
//! you by holding grab over your body for a few seconds (a hit on them starts it over).

use crate::sim::{secs, Action, Event, Fighter, Match, Source, DT, FLOOR_Y, MAX_HP, N_BODIES, N_FIGHTERS, W, WALL, ZOMBIES};
use glam::vec2;
use std::ops::RangeInclusive;

/// How long a survival match can go (nobody lasts this long), and its health pickups over it (one
/// every 20 seconds or so).
pub const SURVIVAL_MAX: u32 = secs(20 * 60);
pub const SURVIVAL_HEALS: RangeInclusive<u32> = 55..=65;
/// After the protected start, the first zombie.
pub const FIRST_ZOMBIE: u32 = secs(2);

/// Between zombies coming in: this long at first, shrinking as the match goes on (halved after
/// `SPAWN_HALVES` seconds, a third after twice that...), never under `SPAWN_MIN`.
const SPAWN_START: f32 = 3.5;
const SPAWN_HALVES: f32 = 60.0;
const SPAWN_MIN: f32 = 0.45;
/// A zombie's health, growing with the match; and its pace (times a run).
const ZOMBIE_HP: f32 = 30.0;
const ZOMBIE_HP_PER_MIN: f32 = 10.0;
const ZOMBIE_HP_MAX: f32 = 120.0;
const PACE: (f32, f32) = (0.36, 0.5);
const PACE_PER_MIN: f32 = 0.05;
const PACE_MAX: f32 = 0.8;
/// A dead zombie lies there this long, then it's gone (freeing its slot).
const CORPSE: u32 = secs(3);

// Its punch: a little damage, a little shove. KNOCKS of them within KNOCK_WINDOW (from one zombie
// or several) knock you down for DOWN.
const ZOMBIE_HIT: f32 = 4.0;
const KNOCKS: u32 = 3;
const KNOCK_WINDOW: u32 = secs(3) / 2;
pub const DOWN: u32 = 80;
/// While you're down, this many zombies this close to you for BITE_HOLD bite, and you turn (time
/// enough for a teammate to knock them off you). A bitten player lies there RISE before getting up
/// one of them.
const BITE_SWARM: usize = 2;
const BITE_RANGE: f32 = 16.0;
const BITE_HOLD: u32 = 40;
const RISE: u32 = 90;

/// Reviving: how close over the body, how long holding grab, and the health you come back with.
const REVIVE_RANGE: (f32, f32) = (10.0, 12.0);
pub const REVIVE_TICKS: u32 = secs(3) / 2;
const REVIVE_HP: f32 = 50.0;
/// A bot only goes to revive someone this near, with no zombie this near them.
const BOT_REVIVE_RANGE: f32 = 60.0;
const BOT_REVIVE_SAFE: f32 = 50.0;

impl Match {
    /// How far into the match we are, in seconds (for how fast and tough zombies come).
    fn survival_secs(&self) -> f32 {
        self.play_ticks() as f32 * DT
    }

    /// Everyone a zombie goes for: whoever's alive, in the arena, and not one of them.
    fn zombie_prey(&self) -> impl Iterator<Item = usize> + '_ {
        (0..N_BODIES).filter(|&j| self.f[j].present() && !self.f[j].zombie)
    }

    /// The survival tick: zombies come in, bite, and their dead are cleared away; the downed count
    /// down; revives go on.
    pub(crate) fn step_zombies(&mut self) {
        if self.config.mode != crate::sim::Mode::Survival {
            return;
        }
        for f in self.f.iter_mut() {
            if f.knock_window > 0 {
                f.knock_window -= 1;
                if f.knock_window == 0 {
                    f.knocks = 0;
                }
            }
        }
        if !self.counting_down() && self.tick >= self.next_zombie {
            self.spawn_zombie();
            let gap = (SPAWN_START / (1.0 + self.survival_secs() / SPAWN_HALVES)).max(SPAWN_MIN);
            self.next_zombie = self.tick + (gap / DT) as u32;
        }
        self.bites();
        self.revives();
        // A zombie dead long enough is gone, freeing its slot.
        for j in ZOMBIES {
            let f = &self.f[j];
            if !f.absent && !f.alive() && f.died_at.is_some_and(|t| self.tick >= t + CORPSE) {
                let pos = f.center();
                self.f[j] = Fighter::empty();
                self.events.push(Event::Poof { pos });
            }
        }
    }

    /// A zombie shambles in at one side or the other (if there's a free slot).
    fn spawn_zombie(&mut self) {
        let Some(j) = ZOMBIES.into_iter().find(|&j| self.f[j].absent) else { return };
        let t = self.survival_secs();
        let left = self.chance(0.5);
        let x = if left { WALL + 6.0 } else { W - WALL - 6.0 };
        let mut z = Fighter::new(x, if left { 1.0 } else { -1.0 });
        z.pos.y = FLOOR_Y;
        z.zombie = true;
        z.hp = (ZOMBIE_HP + ZOMBIE_HP_PER_MIN * t / 60.0).min(ZOMBIE_HP_MAX);
        let k = self.roll(1000) as f32 / 1000.0;
        z.pace = (PACE.0 + (PACE.1 - PACE.0) * k + PACE_PER_MIN * t / 60.0).min(PACE_MAX);
        z.character = crate::character::Character::ALL[self.roll(crate::character::Character::ALL.len() as u32) as usize];
        // (No specials for zombies: `zombie_action` never presses it; nor a hobby.)
        z.special_cd = u32::MAX / 2;
        self.f[j] = z;
        self.events.push(Event::ZombieIn { pos: vec2(x, FLOOR_Y) });
    }

    /// What a zombie does: shambles toward whoever's nearest (up after them, jumping, or down
    /// through a ledge after them) and swings when it's close.
    pub(crate) fn zombie_action(&self, i: usize) -> Action {
        let me = &self.f[i];
        let target = self.zombie_prey().min_by(|&a, &b| self.f[a].center().distance(me.center()).total_cmp(&self.f[b].center().distance(me.center())));
        let Some(j) = target else { return Action::default() };
        let d = self.f[j].pos - me.pos;
        let close = d.x.abs() < me.melee_reach() + 5.0 && d.y.abs() < 14.0;
        // Stuck against something (a wall, a ledge's end it can't climb): hop.
        let stuck = me.grounded && me.vel.x.abs() < 5.0 && d.x.abs() > 20.0;
        Action {
            mv: if close || d.x.abs() < 4.0 { 0.0 } else { d.x.signum() },
            run: true,
            jump: me.grounded && (d.y < -24.0 && d.x.abs() < 60.0 || stuck),
            down: d.y > 24.0 && me.pos.y < FLOOR_Y - 1.0,
            attack: close,
            target: Some(vec2(d.x, 0.0)),
            ..Action::default()
        }
    }

    /// A zombie's punch landing on `j`: a little hurt and a shove, and two close together knock
    /// them down.
    pub(crate) fn zombie_hit(&mut self, i: usize, j: usize) {
        let dir = self.f[i].facing;
        self.damage(i, j, ZOMBIE_HIT, vec2(dir * 90.0, -80.0), 8, dir, Source::Fist, false);
        let f = &mut self.f[j];
        if !f.alive() || f.zombie || f.unshakeable() {
            return;
        }
        f.knocks += 1;
        f.knock_window = KNOCK_WINDOW;
        if f.knocks >= KNOCKS && f.downed == 0 {
            f.knocks = 0;
            f.downed = DOWN;
            f.atk_timer = 0;
            let pos = f.pos;
            self.events.push(Event::KnockedDown { pos });
        }
    }

    /// Anyone down with zombies swarming them is bitten, and turns.
    fn bites(&mut self) {
        for j in 0..N_BODIES {
            let f = &self.f[j];
            if !f.present() || f.zombie || f.downed == 0 {
                self.f[j].bite = 0;
                continue;
            }
            let c = f.center();
            let swarm = (0..N_BODIES).filter(|&k| self.f[k].zombie && self.f[k].present() && self.f[k].hitstun == 0 && self.f[k].center().distance(c) < BITE_RANGE).count();
            let f = &mut self.f[j];
            f.bite = if swarm >= BITE_SWARM { f.bite + 1 } else { 0 };
            if f.bite >= BITE_HOLD && !self.protected() {
                self.turn(j);
            }
        }
    }

    /// `j` bitten: they drop what they hold, and get up one of them.
    fn turn(&mut self, j: usize) {
        if let Some(k) = self.held_item(j) {
            self.items[k].held_by = None;
        }
        let pos = self.f[j].center();
        let tick = self.tick;
        let t = self.survival_secs();
        let f = &mut self.f[j];
        f.holding = None;
        f.zombie = true;
        f.downed = 0;
        f.bite = 0;
        f.hitstun = RISE;
        f.hp = (ZOMBIE_HP + ZOMBIE_HP_PER_MIN * t / 60.0).min(ZOMBIE_HP_MAX);
        f.pace = PACE.1;
        f.died_at.get_or_insert(tick);
        self.events.push(Event::Bitten { who: j, pos });
    }

    /// Whoever's down (not turned) with a teammate standing over them holding grab comes back,
    /// once they've held it long enough; let go, step off or get hit and it starts over.
    fn revives(&mut self) {
        for j in 0..N_FIGHTERS {
            let body = &self.f[j];
            if body.absent || body.alive() || body.zombie {
                continue;
            }
            let at = body.pos;
            let helping = (0..N_FIGHTERS).any(|i| {
                let r = &self.f[i];
                i != j && r.present() && !r.zombie && r.downed == 0 && r.hitstun == 0 && r.action.grab && (r.pos.x - at.x).abs() < REVIVE_RANGE.0 && (r.pos.y - at.y).abs() < REVIVE_RANGE.1
            });
            let f = &mut self.f[j];
            f.revive = if helping { f.revive + 1 } else { 0 };
            if f.revive >= REVIVE_TICKS {
                f.revive = 0;
                f.hp = REVIVE_HP.min(MAX_HP);
                f.died_at = None;
                f.vel = glam::Vec2::ZERO;
                f.hitstun = 0;
                f.downed = 0;
                let pos = f.center();
                self.events.push(Event::Revived { who: j, pos });
            }
        }
    }

    /// A bot near a downed teammate, with no zombie close to them, goes over and holds grab.
    pub(crate) fn bot_revive(&self, i: usize, a: &mut Action) {
        let me = &self.f[i];
        if i >= N_FIGHTERS || me.zombie || me.holding.is_some() {
            return;
        }
        let body = (0..N_FIGHTERS)
            .filter(|&j| j != i && !self.f[j].absent && !self.f[j].alive() && !self.f[j].zombie)
            .map(|j| self.f[j].pos)
            .filter(|p| p.distance(me.pos) < BOT_REVIVE_RANGE)
            .min_by(|a, b| a.distance(me.pos).total_cmp(&b.distance(me.pos)));
        let Some(p) = body else { return };
        let safe = !(0..N_BODIES).any(|k| self.f[k].zombie && self.f[k].present() && self.f[k].pos.distance(p) < BOT_REVIVE_SAFE);
        if !safe {
            return;
        }
        let dx = p.x - me.pos.x;
        *a = if dx.abs() < REVIVE_RANGE.0 - 3.0 && (p.y - me.pos.y).abs() < REVIVE_RANGE.1 {
            Action { grab: true, ..Action::default() }
        } else {
            Action { mv: dx.signum(), run: true, jump: me.grounded && p.y < me.pos.y - 20.0, down: p.y > me.pos.y + 20.0, ..*a }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Brain;
    use crate::sim::{ArenaId, MatchConfig, Mode, N_FIGHTERS};

    fn outbreak() -> (Match, Brain) {
        let config = MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::for_arena(ArenaId::Outbreak) };
        let mut m = Match::new(5, &config);
        m.items.clear();
        // (No zombies but the ones a test puts in.)
        m.next_zombie = u32::MAX;
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    fn zombie_at(m: &mut Match, x: f32) -> usize {
        let j = ZOMBIES.into_iter().find(|&j| m.f[j].absent).unwrap();
        let mut z = Fighter::new(x, 1.0);
        z.zombie = true;
        z.hp = 30.0;
        z.pace = 0.4;
        m.f[j] = z;
        j
    }

    #[test]
    fn players_are_one_side_and_zombies_another() {
        let (mut m, _) = outbreak();
        assert_eq!(m.config.mode, Mode::Survival);
        assert!(!m.foes(0, 1), "teammates");
        let z = zombie_at(&mut m, 200.0);
        assert!(m.foes(0, z) && m.foes(z, 0));
    }

    #[test]
    fn zombies_come_in_faster_and_faster_from_the_sides() {
        let (mut m, z) = outbreak();
        // Everyone up on the ledges out of reach would be ideal; here just count arrivals over
        // the first and the fourth minute (killing them off so slots stay free).
        let arrivals = |m: &mut Match, from: u32, to: u32| {
            let mut n = 0;
            m.tick = from;
            m.next_zombie = from;
            while m.tick < to {
                for j in 0..N_FIGHTERS {
                    m.f[j].hp = MAX_HP;
                }
                m.step([&z; N_FIGHTERS]);
                for e in &m.events {
                    if let Event::ZombieIn { pos } = e {
                        n += 1;
                        assert!(pos.x < 20.0 || pos.x > W - 20.0, "from a side: {pos}");
                    }
                }
                for j in ZOMBIES {
                    m.f[j] = Fighter::empty();
                }
            }
            n
        };
        let early = arrivals(&mut m, secs(0), secs(60));
        let late = arrivals(&mut m, secs(180), secs(240));
        assert!(late > early * 2, "early {early}, late {late}");
    }

    #[test]
    fn quick_punches_knock_you_down_and_a_swarm_on_you_turns_you() {
        let (mut m, z) = outbreak();
        m.f[0].pos = vec2(200.0, FLOOR_Y);
        let a = zombie_at(&mut m, 210.0);
        for _ in 0..KNOCKS - 1 {
            m.zombie_hit(a, 0);
        }
        assert_eq!(m.f[0].downed, 0, "not yet");
        m.zombie_hit(a, 0);
        assert!(m.f[0].downed > 0, "down");
        m.f[0].hitstun = 0;
        m.f[a].hitstun = 0;
        // One zombie on you: no bite.
        for _ in 0..BITE_HOLD + 2 {
            m.f[a].pos = m.f[0].pos;
            m.step([&z; N_FIGHTERS]);
        }
        assert!(!m.f[0].zombie, "one isn't a swarm");
        // Two, staying on you: bitten, and turned.
        m.f[0].downed = DOWN;
        let b = zombie_at(&mut m, 200.0);
        for _ in 0..BITE_HOLD + 2 {
            m.f[a].pos = m.f[0].pos;
            m.f[b].pos = m.f[0].pos;
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[0].zombie && m.f[0].alive(), "turned");
        assert!(m.foes(0, 1), "one of them now");
    }

    #[test]
    fn standing_zombies_cant_bite() {
        let (mut m, z) = outbreak();
        m.f[0].pos = vec2(200.0, FLOOR_Y);
        for _ in 0..3 {
            let k = zombie_at(&mut m, 200.0);
            m.f[k].pos = m.f[0].pos;
        }
        m.step([&z; N_FIGHTERS]);
        assert!(!m.f[0].zombie, "not down, so no bite");
    }

    #[test]
    fn holding_grab_over_a_downed_teammate_revives_them_and_a_hit_starts_it_over() {
        let (mut m, z) = outbreak();
        // (Just the two of them: the bots in the other seats would come over and revive too.)
        m.remove(2);
        m.remove(3);
        m.f[1].pos = vec2(200.0, FLOOR_Y);
        m.f[0].pos = vec2(202.0, FLOOR_Y);
        m.damage(2, 1, MAX_HP, glam::Vec2::ZERO, 0, 1.0, Source::Grenade, false);
        assert!(!m.f[1].alive());
        let hold = Action { grab: true, ..Action::default() };
        for _ in 0..REVIVE_TICKS / 2 {
            m.step_with([&z; N_FIGHTERS], Some((0, hold)));
        }
        assert!(m.f[1].revive > 0, "under way");
        // A hit on the reviver: back to nothing.
        let zk = zombie_at(&mut m, 260.0);
        m.zombie_hit(zk, 0);
        m.step_with([&z; N_FIGHTERS], Some((0, hold)));
        assert_eq!(m.f[1].revive, 0, "started over");
        m.f[zk] = Fighter::empty();
        while m.f[0].hitstun > 0 {
            m.step_with([&z; N_FIGHTERS], Some((0, hold)));
        }
        // (Knocked off them: back over the body.)
        m.f[0].pos = vec2(202.0, FLOOR_Y);
        m.f[0].vel = glam::Vec2::ZERO;
        for _ in 0..REVIVE_TICKS + 1 {
            m.step_with([&z; N_FIGHTERS], Some((0, hold)));
        }
        assert!(m.f[1].alive(), "back up");
    }

    #[test]
    fn the_turned_cant_be_revived_and_its_over_when_nobody_stands() {
        let (mut m, z) = outbreak();
        for i in 1..N_FIGHTERS {
            m.remove(i);
        }
        m.turn(0);
        m.step([&z; N_FIGHTERS]);
        assert!(m.over, "nobody left standing");
    }

    #[test]
    fn dead_zombies_count_and_clear_away() {
        let (mut m, z) = outbreak();
        let k = zombie_at(&mut m, 300.0);
        m.damage(0, k, 100.0, glam::Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.zombies_killed, 1);
        for _ in 0..CORPSE + 2 {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[k].absent, "its slot is free again");
    }

    #[test]
    fn bots_hold_out_a_while() {
        let brains = Brain::committed();
        let lineup: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &brains[i % brains.len()]);
        let (mut lasted, mut killed) = (0, 0);
        for seed in 0..4 {
            let mut m = Match::new(seed, &MatchConfig::for_arena(ArenaId::Outbreak));
            while !m.over {
                m.step(lineup);
            }
            lasted += m.play_ticks();
            killed += m.zombies_killed;
        }
        assert!(killed > 8, "they put some down: {killed}");
        assert!(lasted > secs(60), "a minute between them at least: {}", lasted / 60);
    }
}
