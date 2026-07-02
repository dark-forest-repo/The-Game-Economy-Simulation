// Dark Forest 玩家模块 — SoA 版: 所有函数接受 (store, idx)
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
        fn jitter_val(v: u8, rng: &mut impl rand::Rng) -> u8 {
            ((v as f64 + (rng.gen::<f64>() - 0.5) * 30.0).round().max(0.0).min(100.0)) as u8
        }
        [jitter_val(self.aggression, rng), jitter_val(self.greed, rng), jitter_val(self.boldness, rng), jitter_val(self.sociability, rng), jitter_val(self.emotionality, rng)]
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

/// 生成随机 EVM 地址
pub fn random_evm_address(rng: &mut impl rand::Rng) -> String {
    let bytes: [u8; 20] = rng.gen();
    format!("0x{}", hex::encode(bytes))
}

// ═══════════════════════════════════════════════════════════
// 行为推导 (store, idx) 版
// ═══════════════════════════════════════════════════════════

pub fn p(store: &EntityStore, idx: u32, field: fn(&EntityStore) -> &Vec<u8>) -> u8 {
    field(store)[idx as usize]
}

macro_rules! pf {
    ($store:expr, $idx:expr, $field:ident) => { $store.$field[$idx as usize] };
}

// 人格值转 f64
fn pf64(store: &EntityStore, idx: u32, field: fn(&EntityStore) -> &Vec<u8>) -> f64 {
    field(store)[idx as usize] as f64 / 100.0
}

macro_rules! pff {
    ($store:expr, $idx:expr, $field:ident) => { $store.$field[$idx as usize] as f64 / 100.0 };
}

pub fn derive_max_attacks(store: &EntityStore, idx: u32) -> usize {
    let a = pff!(store, idx, aggression);
    let b = pff!(store, idx, boldness);
    let ang = store.anger[idx as usize] as f64 / 100.0;
    let ela = store.elation[idx as usize] as f64 / 100.0;
    let fe = store.fear[idx as usize] as f64 / 100.0;
    let tilt = store.tilt_level[idx as usize] as f64 / 100.0;
    let raw = a * 80.0 + b * 40.0 + ang * 60.0 + ela * 30.0 - fe * 50.0 + tilt * 100.0;
    (raw.max(0.0) as usize).min(300)
}

pub fn derive_attack_shield_threshold(store: &EntityStore, idx: u32) -> u8 {
    let b = pff!(store, idx, boldness);
    let a = pff!(store, idx, aggression);
    let ang = store.anger[idx as usize] as f64 / 100.0;
    let ela = store.elation[idx as usize] as f64 / 100.0;
    let fe = store.fear[idx as usize] as f64 / 100.0;
    let tilt = store.tilt_level[idx as usize] as f64 / 100.0;
    let raw = 30.0 + b * 30.0 + a * 40.0 + ang * 20.0 + ela * 10.0 - fe * 40.0 + tilt * 30.0;
    raw.max(10.0).min(100.0) as u8
}

pub fn derive_sell_energy_pct(store: &EntityStore, idx: u32) -> f64 {
    let g = pff!(store, idx, greed);
    let b = pff!(store, idx, boldness);
    let fe = store.fear[idx as usize] as f64 / 100.0;
    let ang = store.anger[idx as usize] as f64 / 100.0;
    let ela = store.elation[idx as usize] as f64 / 100.0;
    let tilt = store.tilt_level[idx as usize] as f64 / 100.0;
    let burn = store.burnout[idx as usize] as f64 / 100.0;
    let raw = g * 0.6 - b * 0.2 + fe * 0.3 - ang * 0.2 - ela * 0.1 - tilt * 0.3 + burn * 0.2;
    raw.max(0.0).min(0.9)
}

pub fn derive_min_energy(store: &EntityStore, idx: u32) -> u128 {
    let fe = store.fear[idx as usize] as f64 / 100.0;
    let b = pff!(store, idx, boldness);
    let tilt = store.tilt_level[idx as usize] as f64 / 100.0;
    let raw = 200.0 + fe * 10000.0 - b * 200.0 - tilt * 500.0;
    (raw.max(50.0) as u128)
}

pub fn derive_focus_fire(store: &EntityStore, idx: u32) -> bool {
    let a = pff!(store, idx, aggression);
    let e = pff!(store, idx, emotionality);
    let ang = store.anger[idx as usize] as f64 / 100.0;
    let tilt = store.tilt_level[idx as usize] as f64 / 100.0;
    (a * 0.4 + e * 0.4 + ang * 0.3 + tilt * 0.5) > 0.5
}

pub fn derive_prefer_alliance(store: &EntityStore, idx: u32) -> bool {
    pff!(store, idx, sociability) > 0.3
}

pub fn scan_range(store: &EntityStore, idx: u32) -> u128 {
    m::calc_radar(store.radar_lv[idx as usize] as u128)
}

pub fn collect_rate(store: &EntityStore, idx: u32) -> u128 {
    let lv = store.collector_lv[idx as usize] as u128;
    let refs = store.referral_count[idx as usize] as u128;
    m::calc_collect(lv, refs)
}

// ═══════════════════════════════════════════════════════════
// 升级规划 (SoA 版)
// ═══════════════════════════════════════════════════════════

const SYSTEMS: [&str; 5] = ["collector", "weapon", "shield", "radar", "engine"];

fn sys_lv(store: &EntityStore, idx: u32, sys: &str) -> u128 {
    match sys {
        "collector" => store.collector_lv[idx as usize] as u128,
        "weapon" => store.weapon_lv[idx as usize] as u128,
        "shield" => store.shield_lv[idx as usize] as u128,
        "radar" => store.radar_lv[idx as usize] as u128,
        "engine" => store.engine_lv[idx as usize] as u128,
        _ => 1,
    }
}

fn cost_efficiency(store: &EntityStore, idx: u32, sys: &str) -> f64 {
    let l = sys_lv(store, idx, sys);
    let cost = m::calc_upgrade_cost(sys, l) as f64;
    (1_000_000_000_000_000_000_000u128.saturating_sub(cost as u128) as f64 / 1_000_000_000_000_000_000_000.0).max(0.0)
}

pub fn calc_upgrade_scores(store: &EntityStore, idx: u32) -> [f64; 5] {
    let i = idx as usize;
    let a = pff!(store, idx, aggression);
    let g = pff!(store, idx, greed);
    let b = pff!(store, idx, boldness);
    let s = pff!(store, idx, sociability);
    let ang = store.anger[i] as f64 / 100.0;
    let fe = store.fear[i] as f64 / 100.0;
    let ela = store.elation[i] as f64 / 100.0;
    let bor = store.boredom[i] as f64 / 100.0;
    let tilt = store.tilt_level[i] as f64 / 100.0;
    let burn = store.burnout[i] as f64 / 100.0;
    let energy = store.energy[i];
    let col_dur = store.collector_durability[i];
    let shield_hp = store.shield_hp[i];
    let total_atk = store.total_attacks[i];

    let scores = [
        // collector
        {
            let need = if energy < 5000 { 0.3 } else { 0.0 } + if col_dur < 20000 { 0.2 } else { 0.0 };
            need + g * 0.3 + (1.0 - a) * 0.2 + (1.0 - ang) * 0.1
            + burn * 0.2 - tilt * 0.1 + cost_efficiency(store, idx, "collector") * 0.15
        },
        // weapon
        {
            let need = if sys_lv(store, idx, "weapon") < 3 { 0.2 } else { 0.0 };
            need + a * 0.4 + (1.0 - b).max(0.0) * 0.1
            + ang * 0.4 + ela * 0.2 - fe * 0.3
            + tilt * 0.4 - burn * 0.1 + cost_efficiency(store, idx, "weapon") * 0.1
        },
        // shield
        {
            let max_hp = m::calc_shield_hp(sys_lv(store, idx, "shield"));
            let need = if max_hp > 0 && shield_hp < max_hp / 3 { 0.4 } else { 0.0 };
            need + (1.0 - b) * 0.3 + (1.0 - a) * 0.1
            + fe * 0.5 - ang * 0.2 - ela * 0.1
            - tilt * 0.2 + burn * 0.2 + cost_efficiency(store, idx, "shield") * 0.1
        },
        // radar
        {
            let need = if total_atk < 5 && sys_lv(store, idx, "radar") < 3 { 0.3 } else { 0.0 };
            need + a * 0.15 + (1.0 - s) * 0.15 + b * 0.1
            + ang * 0.1 + bor * 0.2
            + tilt * 0.1 + cost_efficiency(store, idx, "radar") * 0.05
        },
        // engine
        {
            let need = if sys_lv(store, idx, "engine") < 2 { 0.2 } else { 0.0 };
            need + fe * 0.3 + (1.0 - b) * 0.15 + b * 0.1
            + fe * 0.3 + bor * 0.1
            + tilt * 0.1 + burn * 0.1 + cost_efficiency(store, idx, "engine") * 0.05
        },
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
// 升级执行 (直接写 store)
// ═══════════════════════════════════════════════════════════

/// 尝试升级一个系统, 返回 (是否成功, 燃烧的 DFT)
pub fn try_upgrade(store: &mut EntityStore, idx: u32, system: &str, is_post_scarcity: bool) -> (bool, u128) {
    let i = idx as usize;
    if store.is_ruins[i] == 1 { return (false, 0); }

    let lv = match system {
        "collector" => store.collector_lv[i] as u128,
        "weapon" => store.weapon_lv[i] as u128,
        "shield" => store.shield_lv[i] as u128,
        "radar" => store.radar_lv[i] as u128,
        "engine" => store.engine_lv[i] as u128,
        _ => return (false, 0),
    };
    if lv < 1 { return (false, 0); }

    let cost_dft = if is_post_scarcity { 0 } else { m::calc_upgrade_cost(system, lv) };
    let cost_energy = if is_post_scarcity { m::calc_upgrade_energy(system, lv) * 3 } else { m::calc_upgrade_energy(system, lv) };
    let min_reserve = derive_min_energy(store, idx);
    let effective_min = if store.burnout[i] > 50 { min_reserve / 2 } else { min_reserve };

    if !is_post_scarcity && store.dft[i] < cost_dft { return (false, 0); }
    if store.energy[i] < cost_energy + effective_min { return (false, 0); }

    if !is_post_scarcity { store.dft[i] -= cost_dft; }
    store.energy[i] = store.energy[i].saturating_sub(cost_energy);
    store.total_dft_spent[i] += cost_dft;

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
            store.shield_hp[i] = if old_max > 0 && store.shield_hp[i] > 0 {
                store.shield_hp[i] * new_max / old_max
            } else { new_max };
        }
        "radar" => store.radar_lv[i] += 1,
        "engine" => store.engine_lv[i] += 1,
        _ => {}
    }
    (true, cost_dft)
}

// ═══════════════════════════════════════════════════════════
// 情绪更新
// ═══════════════════════════════════════════════════════════

pub fn update_emotion_daily(store: &mut EntityStore, idx: u32, day: u64) -> bool {
    let i = idx as usize;
    let bold = pff!(store, idx, boldness);
    let emo = pff!(store, idx, emotionality);

    // 情绪衰减
    store.anger[i] = (store.anger[i] as f64 * 0.92) as u8;
    store.fear[i] = (store.fear[i] as f64 * 0.95) as u8;
    store.elation[i] = (store.elation[i] as f64 * 0.90) as u8;
    store.boredom[i] = ((store.boredom[i] as f64 * 0.98 + 2.0).min(100.0)) as u8;

    store.days_since_last_attack[i] += 1;
    store.days_since_last_death[i] += 1;

    if store.days_since_last_attack[i] > 10 {
        store.consecutive_wins[i] = 0;
        store.consecutive_losses[i] = 0;
    }

    if store.days_since_last_attack[i] > 20 {
        store.burnout[i] = (store.burnout[i] as u16 + 1).min(100) as u8;
    }
    if store.consecutive_losses[i] > 5 {
        store.burnout[i] = (store.burnout[i] as u16 + 5).min(100) as u8;
    }
    if store.consecutive_wins[i] > 3 {
        store.burnout[i] = (store.burnout[i] as u16).saturating_sub(3) as u8;
    }

    // 上头衰减
    store.tilt_level[i] = ((store.tilt_level[i] as f64 * 0.9) - 1.0).max(0.0) as u8;
    if store.consecutive_losses[i] > 2 {
        let add = (store.consecutive_losses[i] as u16 * 5).min(100) as u8;
        store.tilt_level[i] = (store.tilt_level[i] as u16 + add as u16).min(100) as u8;
    }

    // 弃坑检查
    let should_quit = store.burnout[i] > 90
        || (store.consecutive_losses[i] > 10 && emo > 0.6)
        || (store.total_deaths[i] > 5 && store.burnout[i] > 50);

    // 暂时返回 quit 标志 — 由调用方处理
    should_quit
}

/// 重建
pub fn try_rebuild(store: &mut EntityStore, idx: u32, is_post_scarcity: bool) -> bool {
    let i = idx as usize;
    if store.is_ruins[i] == 0 { return false; }
    let cost_dft = if is_post_scarcity { 0 } else { 50 * 10u128.pow(18) };
    let cost_energy = 5000 * if is_post_scarcity { 3 } else { 1 };
    if !is_post_scarcity && store.dft[i] < cost_dft { return false; }
    if store.energy[i] < cost_energy { return false; }
    if !is_post_scarcity { store.dft[i] -= cost_dft; }
    store.energy[i] -= cost_energy;

    // 凤凰涅槃
    let was_high = store.total_level(idx) >= 50;
    if was_high {
        store.rebirth_count[i] += 1;
        store.growth_multiplier[i] = (1.0 + store.rebirth_count[i] as f64 * 0.12).min(3.0) as f32;
    }

    store.is_ruins[i] = 0;
    store.health[i] = crate::math_engine::MAX_HEALTH / 2;
    store.shield_hp[i] = crate::math_engine::calc_shield_hp(store.shield_lv[i] as u128) / 4;
    store.collector_durability[i] = (crate::math_engine::calc_max_durability(store.collector_lv[i] as u128) / 4) as u64;
    store.creation_time[i] = store.last_collect_time[i];
    store.total_rebuilds[i] += 1;
    store.fear[i] = (store.fear[i] as f64 * 0.5).max(10.0) as u8;
    store.consecutive_losses[i] = 0;
    true
}
