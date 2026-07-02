// Dark Forest 玩家模块 — SoA + ColdStore 版
#![allow(dead_code)]

use crate::math_engine as m;
use crate::store::EntityStore;

// ═══════════════════════════════════════════════════════════
// 人格预设
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct PersonalityPreset {
    pub name: &'static str,
    pub aggression: u8, pub greed: u8, pub boldness: u8,
    pub sociability: u8, pub emotionality: u8,
}

impl PersonalityPreset {
    pub fn farmer() -> Self { Self { name: "farmer", aggression: 10, greed: 60, boldness: 20, sociability: 80, emotionality: 30 } }
    pub fn balanced() -> Self { Self { name: "balanced", aggression: 50, greed: 50, boldness: 50, sociability: 50, emotionality: 50 } }
    pub fn hunter() -> Self { Self { name: "hunter", aggression: 80, greed: 40, boldness: 70, sociability: 20, emotionality: 60 } }
    pub fn whale() -> Self { Self { name: "whale", aggression: 90, greed: 90, boldness: 80, sociability: 70, emotionality: 40 } }
    pub fn turtle() -> Self { Self { name: "turtle", aggression: 10, greed: 30, boldness: 10, sociability: 60, emotionality: 20 } }
    pub fn nomad() -> Self { Self { name: "nomad", aggression: 60, greed: 30, boldness: 80, sociability: 10, emotionality: 70 } }
    pub fn merchant() -> Self { Self { name: "merchant", aggression: 0, greed: 90, boldness: 30, sociability: 70, emotionality: 20 } }
    pub fn general() -> Self { Self { name: "general", aggression: 70, greed: 50, boldness: 60, sociability: 90, emotionality: 40 } }
    pub fn scavenger() -> Self { Self { name: "scavenger", aggression: 40, greed: 60, boldness: 30, sociability: 20, emotionality: 50 } }
    pub fn berserker() -> Self { Self { name: "berserker", aggression: 100, greed: 30, boldness: 100, sociability: 0, emotionality: 90 } }

    pub fn jitter(&self, rng: &mut impl rand::Rng) -> [u8; 5] {
        fn jv(v: u8, r: &mut impl rand::Rng) -> u8 {
            ((v as f64 + (r.gen::<f64>() - 0.5) * 30.0).round().max(0.0).min(100.0)) as u8
        }
        [jv(self.aggression, rng), jv(self.greed, rng), jv(self.boldness, rng), jv(self.sociability, rng), jv(self.emotionality, rng)]
    }
}

pub fn make_preset(name: &str) -> PersonalityPreset {
    match name {
        "farmer" => PersonalityPreset::farmer(), "balanced" => PersonalityPreset::balanced(),
        "hunter" => PersonalityPreset::hunter(), "whale" => PersonalityPreset::whale(),
        "turtle" => PersonalityPreset::turtle(), "nomad" => PersonalityPreset::nomad(),
        "merchant" => PersonalityPreset::merchant(), "general" => PersonalityPreset::general(),
        "scavenger" => PersonalityPreset::scavenger(), "berserker" => PersonalityPreset::berserker(),
        _ => PersonalityPreset::balanced(),
    }
}

pub const SPAWN_DISTRIBUTION: &[(&str, f64)] = &[
    ("farmer", 0.25), ("balanced", 0.20), ("hunter", 0.10),
    ("whale", 0.05), ("turtle", 0.10), ("nomad", 0.05),
    ("merchant", 0.05), ("general", 0.05), ("scavenger", 0.08),
    ("berserker", 0.07),
];

pub fn random_evm_address(rng: &mut impl rand::Rng) -> String {
    let bytes: [u8; 20] = rng.gen();
    format!("0x{}", hex::encode(bytes))
}

// ── 快捷宏: 从 ColdStore 读/写 ──
macro_rules! cget { ($s:expr, $idx:expr, $f:ident) => { $s.cold.$f($idx) }; }
macro_rules! cset { ($s:expr, $idx:expr, $f:ident, $val:expr) => { $s.cold.set_$f($idx, $val); }; }

// ═══════════════════════════════════════════════════════════
// 行为推导
// ═══════════════════════════════════════════════════════════

pub fn derive_max_attacks(store: &EntityStore, idx: u32) -> usize {
    let c = &store.cold;
    let a = c.aggression(idx) as f64 / 100.0;
    let b = c.boldness(idx) as f64 / 100.0;
    let ang = c.anger(idx) as f64 / 100.0;
    let ela = c.elation(idx) as f64 / 100.0;
    let fe = c.fear(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    let raw = a * 80.0 + b * 40.0 + ang * 60.0 + ela * 30.0 - fe * 50.0 + tilt * 100.0;
    (raw.max(0.0) as usize).min(300)
}

pub fn derive_attack_shield_threshold(store: &EntityStore, idx: u32) -> u8 {
    let c = &store.cold;
    let a = c.aggression(idx) as f64 / 100.0;
    let b = c.boldness(idx) as f64 / 100.0;
    let ang = c.anger(idx) as f64 / 100.0;
    let ela = c.elation(idx) as f64 / 100.0;
    let fe = c.fear(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    (30.0 + b * 30.0 + a * 40.0 + ang * 20.0 + ela * 10.0 - fe * 40.0 + tilt * 30.0).max(10.0).min(100.0) as u8
}

pub fn derive_sell_energy_pct(store: &EntityStore, idx: u32) -> f64 {
    let c = &store.cold;
    let g = c.greed(idx) as f64 / 100.0;
    let b = c.boldness(idx) as f64 / 100.0;
    let fe = c.fear(idx) as f64 / 100.0;
    let ang = c.anger(idx) as f64 / 100.0;
    let ela = c.elation(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    let burn = c.burnout(idx) as f64 / 100.0;
    (g * 0.6 - b * 0.2 + fe * 0.3 - ang * 0.2 - ela * 0.1 - tilt * 0.3 + burn * 0.2).max(0.0).min(0.9)
}

pub fn derive_min_energy(store: &EntityStore, idx: u32) -> u128 {
    let c = &store.cold;
    let fe = c.fear(idx) as f64 / 100.0;
    let b = c.boldness(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    (200.0 + fe * 10000.0 - b * 200.0 - tilt * 500.0).max(50.0) as u128
}

pub fn derive_focus_fire(store: &EntityStore, idx: u32) -> bool {
    let c = &store.cold;
    let a = c.aggression(idx) as f64 / 100.0;
    let e = c.emotionality(idx) as f64 / 100.0;
    let ang = c.anger(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    (a * 0.4 + e * 0.4 + ang * 0.3 + tilt * 0.5) > 0.5
}

pub fn derive_prefer_alliance(store: &EntityStore, idx: u32) -> bool {
    store.cold.sociability(idx) as f64 / 100.0 > 0.3
}

pub fn scan_range(store: &EntityStore, idx: u32) -> u128 {
    store.scan_range[idx as usize] as u128
}

pub fn collect_rate(store: &EntityStore, idx: u32) -> u128 {
    m::calc_collect(store.collector_lv[idx as usize] as u128, store.cold.referral_count(idx) as u128)
}

// ═══════════════════════════════════════════════════════════
// 升级规划
// ═══════════════════════════════════════════════════════════

const SYSTEMS: [&str; 5] = ["collector", "weapon", "shield", "radar", "engine"];

fn sys_lv(store: &EntityStore, idx: u32, sys: &str) -> u128 {
    let i = idx as usize;
    match sys {
        "collector" => store.collector_lv[i] as u128,
        "weapon" => store.weapon_lv[i] as u128,
        "shield" => store.shield_lv[i] as u128,
        "radar" => store.radar_lv[i] as u128,
        "engine" => store.engine_lv[i] as u128,
        _ => 1,
    }
}

fn cost_efficiency(store: &EntityStore, idx: u32, sys: &str) -> f64 {
    let cost = m::calc_upgrade_cost(sys, sys_lv(store, idx, sys)) as f64;
    (1_000_000_000_000_000_000_000u128.saturating_sub(cost as u128) as f64 / 1_000_000_000_000_000_000_000.0).max(0.0)
}

pub fn calc_upgrade_scores(store: &EntityStore, idx: u32) -> [f64; 5] {
    let i = idx as usize;
    let c = &store.cold;
    let a = c.aggression(idx) as f64 / 100.0;
    let g = c.greed(idx) as f64 / 100.0;
    let b = c.boldness(idx) as f64 / 100.0;
    let s = c.sociability(idx) as f64 / 100.0;
    let ang = c.anger(idx) as f64 / 100.0;
    let fe = c.fear(idx) as f64 / 100.0;
    let ela = c.elation(idx) as f64 / 100.0;
    let bor = c.boredom(idx) as f64 / 100.0;
    let tilt = c.tilt_level(idx) as f64 / 100.0;
    let burn = c.burnout(idx) as f64 / 100.0;
    let energy = store.energy[i]; let col_dur = store.collector_durability[i];
    let shield_hp = store.shield_hp[i]; let total_atk = store.total_attacks[i];

    let scores = [
        { let need = if energy < 5000 { 0.3 } else { 0.0 } + if col_dur < 20000 { 0.2 } else { 0.0 };
          need + g * 0.3 + (1.0 - a) * 0.2 + (1.0 - ang) * 0.1 + burn * 0.2 - tilt * 0.1 + cost_efficiency(store, idx, "collector") * 0.15 },
        { let need = if sys_lv(store, idx, "weapon") < 3 { 0.2 } else { 0.0 };
          need + a * 0.4 + (1.0 - b).max(0.0) * 0.1 + ang * 0.4 + ela * 0.2 - fe * 0.3 + tilt * 0.4 - burn * 0.1 + cost_efficiency(store, idx, "weapon") * 0.1 },
        { let max_hp = m::calc_shield_hp(sys_lv(store, idx, "shield"));
          let need = if max_hp > 0 && shield_hp < max_hp / 3 { 0.4 } else { 0.0 };
          need + (1.0 - b) * 0.3 + (1.0 - a) * 0.1 + fe * 0.5 - ang * 0.2 - ela * 0.1 - tilt * 0.2 + burn * 0.2 + cost_efficiency(store, idx, "shield") * 0.1 },
        { let need = if total_atk < 5 && sys_lv(store, idx, "radar") < 3 { 0.3 } else { 0.0 };
          need + a * 0.15 + (1.0 - s) * 0.15 + b * 0.1 + ang * 0.1 + bor * 0.2 + tilt * 0.1 + cost_efficiency(store, idx, "radar") * 0.05 },
        { let need = if sys_lv(store, idx, "engine") < 2 { 0.2 } else { 0.0 };
          need + fe * 0.3 + (1.0 - b) * 0.15 + b * 0.1 + fe * 0.3 + bor * 0.1 + tilt * 0.1 + burn * 0.1 + cost_efficiency(store, idx, "engine") * 0.05 },
    ];
    scores
}

pub fn plan_upgrades(store: &EntityStore, idx: u32) -> [&'static str; 5] {
    let scores = calc_upgrade_scores(store, idx);
    let mut pairs: Vec<(&str, f64)> = SYSTEMS.iter().copied().zip(scores.iter().copied()).collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    [pairs[0].0, pairs[1].0, pairs[2].0, pairs[3].0, pairs[4].0]
}

// ═══════════════════════════════════════════════════════════
// 升级执行
// ═══════════════════════════════════════════════════════════

pub fn try_upgrade(store: &mut EntityStore, idx: u32, system: &str, is_post_scarcity: bool) -> (bool, u128) {
    let i = idx as usize;
    if store.is_ruins[i] == 1 { return (false, 0); }
    let lv = match system {
        "collector" => store.collector_lv[i] as u128, "weapon" => store.weapon_lv[i] as u128,
        "shield" => store.shield_lv[i] as u128, "radar" => store.radar_lv[i] as u128,
        "engine" => store.engine_lv[i] as u128, _ => return (false, 0),
    };
    if lv < 1 { return (false, 0); }
    let cost_dft = if is_post_scarcity { 0 } else { m::calc_upgrade_cost(system, lv) };
    let cost_energy = if is_post_scarcity { m::calc_upgrade_energy(system, lv) * 3 } else { m::calc_upgrade_energy(system, lv) };
    let min_reserve = derive_min_energy(store, idx);
    let effective_min = if store.cold.burnout(idx) > 50 { min_reserve / 2 } else { min_reserve };
    if !is_post_scarcity && store.dft[i] < cost_dft { return (false, 0); }
    if store.energy[i] < cost_energy + effective_min { return (false, 0); }
    if !is_post_scarcity { store.dft[i] -= cost_dft; }
    store.energy[i] = store.energy[i].saturating_sub(cost_energy);
    store.cold.set_dft_spent(idx, store.cold.dft_spent(idx) + cost_dft);
    match system {
        "collector" => {
            let old_max = m::calc_max_durability(store.collector_lv[i] as u128);
            store.collector_lv[i] += 1;
            let new_max = m::calc_max_durability(store.collector_lv[i] as u128);
            store.collector_durability[i] = if old_max > 0 && store.collector_durability[i] > 0 {
                (store.collector_durability[i] as u128 * new_max / old_max) as u64
            } else { new_max as u64 };
        }
        "weapon" => store.weapon_lv[i] += 1,
        "shield" => {
            let old_max = m::calc_shield_hp(store.shield_lv[i] as u128);
            store.shield_lv[i] += 1;
            let new_max = m::calc_shield_hp(store.shield_lv[i] as u128);
            store.shield_hp[i] = if old_max > 0 && store.shield_hp[i] > 0 { store.shield_hp[i] * new_max / old_max } else { new_max };
        }
        "radar" => { store.radar_lv[i] += 1; store.scan_range[i] = m::calc_radar(store.radar_lv[i] as u128) as u32; }
        "engine" => store.engine_lv[i] += 1,
        _ => {}
    }
    (true, cost_dft)
}

// ═══════════════════════════════════════════════════════════
// 情绪更新
// ═══════════════════════════════════════════════════════════

pub fn update_emotion_daily(store: &mut EntityStore, idx: u32, _day: u64) -> bool {
    let c = &mut store.cold;
    let bold = c.boldness(idx) as f64 / 100.0;
    let emo = c.emotionality(idx) as f64 / 100.0;

    c.set_anger(idx, (c.anger(idx) as f64 * 0.92) as u8);
    c.set_fear(idx, (c.fear(idx) as f64 * 0.95) as u8);
    c.set_elation(idx, (c.elation(idx) as f64 * 0.90) as u8);
    c.set_boredom(idx, ((c.boredom(idx) as f64 * 0.98 + 2.0).min(100.0)) as u8);

    c.set_days_since_attack(idx, c.days_since_attack(idx) + 1);
    c.set_days_since_death(idx, c.days_since_death(idx) + 1);

    if c.days_since_attack(idx) > 10 { c.set_consecutive_wins(idx, 0); c.set_consecutive_losses(idx, 0); }
    if c.days_since_attack(idx) > 20 { c.set_burnout(idx, (c.burnout(idx) as u16 + 1).min(100) as u8); }
    if c.consecutive_losses(idx) > 5 { c.set_burnout(idx, (c.burnout(idx) as u16 + 5).min(100) as u8); }
    if c.consecutive_wins(idx) > 3 { c.set_burnout(idx, (c.burnout(idx) as u16).saturating_sub(3) as u8); }

    c.set_tilt_level(idx, ((c.tilt_level(idx) as f64 * 0.9) - 1.0).max(0.0) as u8);
    if c.consecutive_losses(idx) > 2 {
        let add = (c.consecutive_losses(idx) as u16 * 5).min(100) as u8;
        c.set_tilt_level(idx, (c.tilt_level(idx) as u16 + add as u16).min(100) as u8);
    }

    c.burnout(idx) > 90 || (c.consecutive_losses(idx) > 10 && emo > 0.6) || (store.total_deaths[idx as usize] > 5 && c.burnout(idx) > 50)
}

/// 计算玩家当前资源能升几级 (避免盲目试 20 次)
pub fn count_affordable_upgrades(store: &EntityStore, idx: u32, is_post_scarcity: bool, priority: &[&str; 5]) -> usize {
    let i = idx as usize;
    let energy = store.energy[i];
    let dft = store.dft[i];
    let min_reserve = derive_min_energy(store, idx);
    let mut count = 0;

    for sys in priority {
        let lv = match *sys {
            "collector" => store.collector_lv[i] as u128,
            "weapon" => store.weapon_lv[i] as u128,
            "shield" => store.shield_lv[i] as u128,
            "radar" => store.radar_lv[i] as u128,
            "engine" => store.engine_lv[i] as u128,
            _ => continue,
        } as usize;

        let cost_e = if is_post_scarcity { m::calc_upgrade_energy(sys, lv as u128) * 3 } else { m::calc_upgrade_energy(sys, lv as u128) };
        let cost_d = if is_post_scarcity { 0 } else { m::calc_upgrade_cost(sys, lv as u128) };

        // 按优先级一个一个检查
        let remaining_energy = energy.saturating_sub(cost_e * (count as u128 + 1) + min_reserve);
        let remaining_dft = dft.saturating_sub(cost_d * (count as u128 + 1));
        if remaining_energy > 0 && (is_post_scarcity || remaining_dft > 0) {
            count += 1;
        }
    }
    count.min(30)
}

/// 重建
pub fn try_rebuild(store: &mut EntityStore, idx: u32, is_post_scarcity: bool, current_time: u64) -> bool {
    let i = idx as usize;
    if store.is_ruins[i] == 0 { return false; }
    let cost_dft = if is_post_scarcity { 0 } else { 50 * 10u128.pow(18) };
    let cost_energy = 5000 * if is_post_scarcity { 3 } else { 1 };
    if !is_post_scarcity && store.dft[i] < cost_dft { return false; }
    if store.energy[i] < cost_energy { return false; }
    if !is_post_scarcity { store.dft[i] -= cost_dft; }
    store.energy[i] -= cost_energy;

    let was_high = store.total_level(idx) >= 50;
    if was_high {
        let c = &mut store.cold;
        c.set_rebirth_count(idx, c.rebirth_count(idx) + 1);
        c.set_growth_multiplier(idx, (1.0 + c.rebirth_count(idx) as f64 * 0.12).min(3.0) as f32);
    }

    store.is_ruins[i] = 0;
    store.health[i] = m::MAX_HEALTH / 2;
    store.shield_hp[i] = m::calc_shield_hp(store.shield_lv[i] as u128) / 4;
    store.collector_durability[i] = (m::calc_max_durability(store.collector_lv[i] as u128) / 4) as u64;
    store.creation_time[i] = store.last_collect_time[i];
    store.last_collect_time[i] = current_time; // 防止复活后拿死亡积压能量
    store.cold.set_fear(idx, (store.cold.fear(idx) as f64 * 0.5).max(10.0) as u8);
    store.cold.set_consecutive_losses(idx, 0);
    store.cold.set_rebuilds(idx, store.cold.rebuilds(idx) + 1);
    true
}
