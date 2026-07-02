// Dark Forest 玩家模块 — 人格驱动 + 非理性行为 + 社交网络
#![allow(dead_code)]

use crate::math_engine as m;

// ═══════════════════════════════════════════════════════════
// 人格五维模型
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Personality {
    pub aggression: f64,    // 0-1 攻击倾向
    pub greed: f64,         // 0-1 贪婪程度
    pub boldness: f64,      // 0-1 冒险精神
    pub sociability: f64,   // 0-1 社交倾向
    pub emotionality: f64,  // 0-1 情绪波动
}

impl Personality {
    pub fn farmer() -> Self { Self { aggression: 0.10, greed: 0.60, boldness: 0.20, sociability: 0.80, emotionality: 0.30 } }
    pub fn balanced() -> Self { Self { aggression: 0.50, greed: 0.50, boldness: 0.50, sociability: 0.50, emotionality: 0.50 } }
    pub fn hunter() -> Self { Self { aggression: 0.80, greed: 0.40, boldness: 0.70, sociability: 0.20, emotionality: 0.60 } }
    pub fn whale() -> Self { Self { aggression: 0.90, greed: 0.90, boldness: 0.80, sociability: 0.70, emotionality: 0.40 } }
    pub fn turtle() -> Self { Self { aggression: 0.10, greed: 0.30, boldness: 0.10, sociability: 0.60, emotionality: 0.20 } }
    pub fn nomad() -> Self { Self { aggression: 0.60, greed: 0.30, boldness: 0.80, sociability: 0.10, emotionality: 0.70 } }
    pub fn merchant() -> Self { Self { aggression: 0.00, greed: 0.90, boldness: 0.30, sociability: 0.70, emotionality: 0.20 } }
    pub fn general() -> Self { Self { aggression: 0.70, greed: 0.50, boldness: 0.60, sociability: 0.90, emotionality: 0.40 } }
    pub fn scavenger() -> Self { Self { aggression: 0.40, greed: 0.60, boldness: 0.30, sociability: 0.20, emotionality: 0.50 } }
    pub fn berserker() -> Self { Self { aggression: 1.00, greed: 0.30, boldness: 1.00, sociability: 0.00, emotionality: 0.90 } }

    pub fn jitter(&self, rng: &mut impl rand::Rng) -> Self {
        Self {
            aggression: (self.aggression + (rng.gen::<f64>() - 0.5) * 0.3).clamp(0.0, 1.0),
            greed: (self.greed + (rng.gen::<f64>() - 0.5) * 0.3).clamp(0.0, 1.0),
            boldness: (self.boldness + (rng.gen::<f64>() - 0.5) * 0.3).clamp(0.0, 1.0),
            sociability: (self.sociability + (rng.gen::<f64>() - 0.5) * 0.3).clamp(0.0, 1.0),
            emotionality: (self.emotionality + (rng.gen::<f64>() - 0.5) * 0.3).clamp(0.0, 1.0),
        }
    }
}

pub fn make_personality(name: &str, rng: &mut impl rand::Rng) -> Personality {
    let base = match name {
        "farmer"    => Personality::farmer(), "balanced"  => Personality::balanced(),
        "hunter"    => Personality::hunter(), "whale"     => Personality::whale(),
        "turtle"    => Personality::turtle(), "nomad"     => Personality::nomad(),
        "merchant"  => Personality::merchant(), "general" => Personality::general(),
        "scavenger" => Personality::scavenger(), "berserker" => Personality::berserker(),
        _           => Personality::balanced(),
    };
    base.jitter(rng)
}

pub const SPAWN_DISTRIBUTION: &[(&str, f64)] = &[
    ("farmer", 0.25), ("balanced", 0.20), ("hunter", 0.10),
    ("whale", 0.05), ("turtle", 0.10), ("nomad", 0.05),
    ("merchant", 0.05), ("general", 0.05), ("scavenger", 0.08),
    ("berserker", 0.07),
];

// ═══════════════════════════════════════════════════════════
// 情绪状态
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct EmotionalState {
    pub anger: f64,
    pub fear: f64,
    pub elation: f64,
    pub boredom: f64,
}

impl EmotionalState {
    pub fn new() -> Self { Self { anger: 0.0, fear: 0.0, elation: 0.0, boredom: 0.5 } }
    pub fn decay(&mut self) {
        self.anger *= 0.92; self.fear *= 0.95;
        self.elation *= 0.90; self.boredom = (self.boredom * 0.98 + 0.02).min(1.0);
    }
}

// ═══════════════════════════════════════════════════════════
// 行为参数推导
// ═══════════════════════════════════════════════════════════

pub fn derive_max_attacks(p: &Personality, e: &EmotionalState, tilt: f64) -> usize {
    let raw = p.aggression * 80.0 + p.boldness * 40.0 + e.anger * 60.0 + e.elation * 30.0
        - e.fear * 50.0
        + tilt * 100.0;  // 上头时乱打
    (raw.max(0.0) as usize).min(300)
}

pub fn derive_attack_shield_threshold(p: &Personality, e: &EmotionalState, tilt: f64) -> u128 {
    let raw = 30.0 + p.boldness * 30.0 + p.aggression * 40.0 + e.anger * 20.0
        + e.elation * 10.0 - e.fear * 40.0
        + tilt * 30.0;  // 上头时啥都敢打
    raw.max(10.0).min(100.0) as u128
}

pub fn derive_sell_energy_pct(p: &Personality, e: &EmotionalState, tilt: f64, burnout: f64) -> f64 {
    let raw = p.greed * 0.6 - p.boldness * 0.2 + e.fear * 0.3 - e.anger * 0.2 - e.elation * 0.1
        - tilt * 0.3      // 上头: 不留后路
        + burnout * 0.2;  // 摆烂: 卖资源躺平
    raw.max(0.0).min(0.9)
}

pub fn derive_min_energy(p: &Personality, e: &EmotionalState, tilt: f64) -> u128 {
    let base = 200.0;
    let fear_bonus = e.fear * 10000.0;
    let courage_discount = p.boldness * 200.0 + tilt * 500.0; // 上头不存能量
    (base + fear_bonus - courage_discount).max(50.0) as u128
}

pub fn derive_focus_fire(p: &Personality, e: &EmotionalState, tilt: f64) -> bool {
    (p.aggression * 0.4 + p.emotionality * 0.4 + e.anger * 0.3 + tilt * 0.5) > 0.5
}

pub fn derive_prefer_alliance(p: &Personality) -> bool {
    p.sociability > 0.3
}

// ═══════════════════════════════════════════════════════════
// 升级规划引擎
// ═══════════════════════════════════════════════════════════

const SYSTEMS: [&str; 5] = ["collector", "weapon", "shield", "radar", "engine"];

pub fn calc_upgrade_scores(civ: &Civilization) -> [f64; 5] {
    let p = &civ.personality;
    let e = &civ.emotion;

    let lv = |s: &str| -> u128 {
        match s {
            "collector" => civ.collector_lv, "weapon" => civ.weapon_lv,
            "shield" => civ.shield_lv, "radar" => civ.radar_lv, "engine" => civ.engine_lv,
            _ => 1,
        }
    };

    let cost_efficiency = |sys: &str| -> f64 {
        let cost = m::calc_upgrade_cost(sys, lv(sys)) as f64;
        (1_000_000_000_000_000_000_000u128.saturating_sub(cost as u128) as f64 / 1_000_000_000_000_000_000_000.0).max(0.0)
    };

    let tilt = civ.tilt_level;
    let burnout = civ.burnout;

    let scores = [
        // collector
        {
            let need = if civ.energy < 5000 { 0.3 } else { 0.0 }
                + if civ.collector_durability < 20000 { 0.2 } else { 0.0 };
            let personality = p.greed * 0.3 + (1.0 - p.aggression) * 0.2;
            let emotion = (1.0 - e.anger) * 0.1;
            let irrational = burnout * 0.2 - tilt * 0.1; // 摆烂搞经济, 上头不搞
            need + personality + emotion + irrational + cost_efficiency("collector") * 0.15
        },
        // weapon
        {
            let need = if lv("weapon") < 3 { 0.2 } else { 0.0 };
            let personality = p.aggression * 0.4 + (1.0 - p.boldness).max(0.0) * 0.1;
            let emotion = e.anger * 0.4 + e.elation * 0.2 - e.fear * 0.3;
            let irrational = tilt * 0.4 + burnout * (-0.1); // 上头狂升武器, 摆烂懒得升
            need + personality + emotion + irrational + cost_efficiency("weapon") * 0.1
        },
        // shield
        {
            let need = if civ.shield_hp < m::calc_shield_hp(lv("shield")) / 3 { 0.4 } else { 0.0 };
            let personality = (1.0 - p.boldness) * 0.3 + (1.0 - p.aggression) * 0.1;
            let emotion = e.fear * 0.5 - e.anger * 0.2 - e.elation * 0.1;
            let irrational = tilt * (-0.2) + burnout * 0.2; // 上头不升盾, 摆烂升盾保命
            need + personality + emotion + irrational + cost_efficiency("shield") * 0.1
        },
        // radar
        {
            let need = if civ.total_attacks < 5 && lv("radar") < 3 { 0.3 } else { 0.0 };
            let personality = p.aggression * 0.15 + (1.0 - p.sociability) * 0.15 + p.boldness * 0.1;
            let emotion = e.anger * 0.1 + e.boredom * 0.2;
            let irrational = tilt * 0.1; // 上头到处找人
            need + personality + emotion + irrational + cost_efficiency("radar") * 0.05
        },
        // engine
        {
            let need = if lv("engine") < 2 { 0.2 } else { 0.0 };
            let personality = e.fear * 0.3 + (1.0 - p.boldness) * 0.15 + p.boldness * 0.1;
            let emotion = e.fear * 0.3 + e.boredom * 0.1;
            let irrational = tilt * 0.1 + burnout * 0.1; // 上头/摆烂都想跑
            need + personality + emotion + irrational + cost_efficiency("engine") * 0.05
        },
    ];
    scores
}

pub fn plan_upgrades(civ: &Civilization) -> [&'static str; 5] {
    let scores = calc_upgrade_scores(civ);
    let mut pairs: Vec<(&str, f64)> = SYSTEMS.iter().copied().zip(scores.iter().copied()).collect();
    pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    [pairs[0].0, pairs[1].0, pairs[2].0, pairs[3].0, pairs[4].0]
}

// ═══════════════════════════════════════════════════════════
// 文明
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Civilization {
    pub id: String,
    pub address: String,        // EVM 地址 (0x + 40 hex)
    pub name: String,
    pub generation: u64,
    pub personality_type: &'static str,

    // 人格 & 情绪
    pub personality: Personality,
    pub emotion: EmotionalState,

    // 非理性行为
    pub tilt_level: f64,          // 0-1 上头程度
    pub burnout: f64,             // 0-1 摆烂程度

    // 凤凰涅槃
    pub rebirth_count: u32,       // 被摧毁+重建次数
    pub growth_multiplier: f64,   // 成长系数 (>= 1.0)
    pub consecutive_wins: u32,    // 连胜
    pub consecutive_losses: u32,  // 连败
    pub quit_day: Option<u64>,    // 弃坑日期 (如果有)
    pub last_quit_check: u64,     // 上次检查弃坑的天数

    // 基础资源
    pub energy: u128,
    pub dft: u128,
    pub health: u128,
    pub shield_hp: u128,
    pub max_health: u128,

    // 5 个系统等级
    pub collector_lv: u128, pub weapon_lv: u128, pub shield_lv: u128,
    pub radar_lv: u128, pub engine_lv: u128,

    // 系统耐久
    pub collector_durability: u128, pub weapon_durability: u128,
    pub shield_durability: u128, pub engine_durability: u128,

    // 坐标
    pub x: i128, pub y: i128, pub z: i128,

    // 状态
    pub is_ruins: bool,
    pub creation_time: u128,
    pub last_collect_time: u128,
    pub last_jump_time: u128,
    pub jump_count: u128,
    pub referral_count: u128,

    // Token
    pub attack_tokens: u128, pub max_attack_tokens: u128, pub last_token_time: u128,

    // 邀请
    pub invites_remaining: u8,
    pub invited_by: Option<String>,

    // 计数
    pub attack_count_today: usize, pub last_attack_day: u128,
    pub total_energy_collected: u128, pub total_dft_spent: u128, pub total_dft_earned: u128,
    pub total_attacks: u128, pub total_victims: u128, pub total_deaths: u128,
    pub total_plundered: u128, pub total_rebuilds: u128,
    pub last_target_id: Option<String>,

    // 联盟
    pub alliance_id: Option<String>,
    pub leave_cooldown_until: u128,

    // 社交网络
    pub enemies: Vec<(String, f64)>,       // 仇人 (id, severity)
    pub friends: Vec<(String, f64)>,        // 好友 (id, trust)
    pub reputation: f64,                    // 信誉 (-1 to 1)
    pub days_since_last_attack: u64,
    pub days_since_last_death: u64,
    pub victims_last_week: u128,
    pub deaths_last_week: u128,
}

impl Civilization {
    pub fn new(
        id: String, address: String, name: String, generation: u64,
        personality: Personality, personality_type: &'static str,
        creation_time: u128, last_collect_time: u128, last_token_time: u128,
        invites: u8,
    ) -> Self {
        Self {
            id, address, name, generation, personality_type,
            personality, emotion: EmotionalState::new(),
            tilt_level: 0.0, burnout: 0.0,
            rebirth_count: 0, growth_multiplier: 1.0,
            consecutive_wins: 0, consecutive_losses: 0,
            quit_day: None, last_quit_check: 0,
            energy: m::INITIAL_ENERGY, dft: 0,
            health: m::INITIAL_HEALTH, shield_hp: m::calc_shield_hp(1), max_health: m::MAX_HEALTH,
            collector_lv: 1, weapon_lv: 1, shield_lv: 1, radar_lv: 1, engine_lv: 1,
            collector_durability: m::DURABILITY_BASE, weapon_durability: m::WEAPON_DUR_BASE,
            shield_durability: m::SHIELD_DUR_BASE, engine_durability: m::ENGINE_DUR_BASE,
            x: 0, y: 0, z: 0, is_ruins: false,
            creation_time, last_collect_time,
            last_jump_time: 0, jump_count: 0, referral_count: 0,
            attack_tokens: m::TOKEN_BASE_MAX, max_attack_tokens: m::TOKEN_BASE_MAX, last_token_time,
            invites_remaining: invites, invited_by: None,
            attack_count_today: 0, last_attack_day: 0,
            total_energy_collected: 0, total_dft_spent: 0, total_dft_earned: 0,
            total_attacks: 0, total_victims: 0, total_deaths: 0,
            total_plundered: 0, total_rebuilds: 0,
            last_target_id: None, alliance_id: None, leave_cooldown_until: 0,
            enemies: Vec::new(), friends: Vec::new(), reputation: 0.0,
            days_since_last_attack: 0, days_since_last_death: 0,
            victims_last_week: 0, deaths_last_week: 0,
        }
    }

    // --- 计算属性 ---

    /// 涅槃成长系数作用后的有效等级
    pub fn effective_lv(&self, actual: u128) -> u128 {
        if self.growth_multiplier > 1.0 && actual >= 10 {
            // 等级越高, 加成越明显: mult^(actual/50)
            // 10级: x1.0, 50级: x1.1, 100级: x1.21, 200级: x1.46
            let boost = self.growth_multiplier.powf(actual as f64 / 50.0);
            (actual as f64 * boost) as u128
        } else {
            actual
        }
    }

    pub fn effective_weapon_lv(&self) -> u128 { self.effective_lv(self.weapon_lv) }
    pub fn effective_shield_lv(&self) -> u128 { self.effective_lv(self.shield_lv) }

    pub fn scan_range(&self) -> u128 { m::calc_radar(self.radar_lv) }
    pub fn collect_rate(&self) -> u128 { m::calc_collect(self.effective_lv(self.collector_lv), self.referral_count) }
    pub fn token_regen_interval(&self) -> u128 { m::calc_token_regen_interval(self.weapon_lv) }

    pub fn shield_percent(&self) -> u128 {
        let max_hp = m::calc_shield_hp(self.shield_lv);
        if max_hp == 0 { return 0; }
        self.shield_hp * 100 / max_hp
    }

    pub fn total_level(&self) -> u128 {
        self.collector_lv + self.weapon_lv + self.shield_lv + self.radar_lv + self.engine_lv
    }

    pub fn is_newbie(&self, current_time: u128) -> bool {
        current_time.saturating_sub(self.creation_time) < m::NEWBIE_PROTECTION
    }

    pub fn is_quit(&self) -> bool {
        self.quit_day.is_some()
    }

    // --- 行为参数 (人格+情绪+非理性) ---

    pub fn max_attacks_per_day(&self) -> usize {
        derive_max_attacks(&self.personality, &self.emotion, self.tilt_level)
    }
    pub fn attack_shield_threshold(&self) -> u128 {
        derive_attack_shield_threshold(&self.personality, &self.emotion, self.tilt_level)
    }
    pub fn sell_energy_pct(&self) -> f64 {
        derive_sell_energy_pct(&self.personality, &self.emotion, self.tilt_level, self.burnout)
    }
    pub fn min_energy_reserve(&self) -> u128 {
        derive_min_energy(&self.personality, &self.emotion, self.tilt_level)
    }
    pub fn focus_fire(&self) -> bool {
        derive_focus_fire(&self.personality, &self.emotion, self.tilt_level)
    }
    pub fn prefer_alliance(&self) -> bool {
        derive_prefer_alliance(&self.personality)
    }

    // --- 核心操作 ---

    pub fn collect_energy(&mut self, current_time: u128, _full_day: bool) {
        if self.is_ruins { return; }
        if current_time <= self.last_collect_time { return; }
        let elapsed = current_time - self.last_collect_time;
        let rate = self.collect_rate();
        if self.collector_durability > 0 && rate > 0 {
            let ct = if elapsed < self.collector_durability { elapsed } else { self.collector_durability };
            let gained = ct * rate;
            self.energy += gained;
            self.collector_durability -= ct;
            self.total_energy_collected += gained;
        }
        self.last_collect_time = current_time;
    }

    pub fn regen_tokens(&mut self, current_time: u128) {
        if self.is_ruins { return; }
        let interval_ms = self.token_regen_interval();
        let max_t = m::calc_max_tokens(self.weapon_lv);
        let elapsed = current_time.saturating_sub(self.last_token_time);
        let interval_s = interval_ms / 100;
        if interval_s > 0 && elapsed >= interval_s {
            self.attack_tokens = std::cmp::min(self.attack_tokens + elapsed / interval_s, max_t);
            self.last_token_time = current_time;
        }
    }

    /// 升级 (无等级上限)
    pub fn try_upgrade(&mut self, system: &str, _current_time: u128, is_post_scarcity: bool) -> (bool, u128) {
        if self.is_ruins { return (false, 0); }
        let lv = match system {
            "collector" => self.collector_lv, "weapon" => self.weapon_lv,
            "shield" => self.shield_lv, "radar" => self.radar_lv, "engine" => self.engine_lv,
            _ => return (false, 0),
        };
        if lv < 1 { return (false, 0); }

        let cost_dft = if is_post_scarcity { 0 } else { m::calc_upgrade_cost(system, lv) };
        let cost_energy = if is_post_scarcity { m::calc_upgrade_energy(system, lv) * 3 }
                          else { m::calc_upgrade_energy(system, lv) };
        let min_reserve = self.min_energy_reserve();

        if !is_post_scarcity && self.dft < cost_dft { return (false, 0); }
        // 摆烂时放宽能量保留
        let effective_min = if self.burnout > 0.5 { min_reserve / 2 } else { min_reserve };
        if self.energy < cost_energy + effective_min { return (false, 0); }

        if !is_post_scarcity { self.dft -= cost_dft; }
        self.energy = self.energy.saturating_sub(cost_energy);
        self.total_dft_spent += cost_dft;

        match system {
            "collector" => {
                let old_max = m::calc_max_durability(self.collector_lv);
                self.collector_lv += 1;
                let new_max = m::calc_max_durability(self.collector_lv);
                self.collector_durability = if old_max > 0 && self.collector_durability > 0 {
                    self.collector_durability * new_max / old_max
                } else { new_max };
            }
            "weapon" => self.weapon_lv += 1,
            "shield" => {
                let old_max = m::calc_shield_hp(self.shield_lv);
                self.shield_lv += 1;
                let new_max = m::calc_shield_hp(self.shield_lv);
                self.shield_hp = if old_max > 0 && self.shield_hp > 0 {
                    self.shield_hp * new_max / old_max
                } else { new_max };
            }
            "radar" => self.radar_lv += 1,
            "engine" => self.engine_lv += 1,
            _ => {}
        }
        (true, cost_dft)
    }

    /// 攻击
    pub fn attack_target(
        &mut self, target: &mut Civilization, current_time: u128, alliance_def_bonus: u128,
    ) -> AttackOutcome {
        if self.is_ruins || target.is_ruins { return AttackOutcome::Fail; }
        if target.is_newbie(current_time) { return AttackOutcome::NewbieProtected; }
        if let Some(ref aid) = self.alliance_id {
            if let Some(ref taid) = target.alliance_id { if aid == taid { return AttackOutcome::SameAlliance; } }
        }
        let cost = m::calc_attack_energy_cost(self.weapon_lv);
        if self.energy < cost { return AttackOutcome::LowEnergy; }
        self.energy -= cost;
        self.regen_tokens(current_time);
        if self.attack_tokens <= 0 { return AttackOutcome::RateLimited; }
        self.attack_tokens -= 1;
        self.total_attacks += 1;

        // 使用有效等级计算战斗 (涅槃加成)
        let eff_weapon = self.effective_weapon_lv();
        let eff_shield = target.effective_shield_lv();
        let result = m::simulate_attack(eff_weapon, eff_shield, target.energy,
                                         target.health, target.shield_hp, alliance_def_bonus);
        target.shield_hp = result.remaining_shield;
        target.health = result.remaining_health;

        if result.stolen_energy > 0 {
            let steal = std::cmp::min(result.stolen_energy, target.energy);
            target.energy -= steal; self.energy += steal; self.total_plundered += steal;
        }
        let dft_plunder = target.dft * 2000 / 10000;
        if dft_plunder > 0 {
            let actual = std::cmp::min(dft_plunder, target.dft);
            target.dft -= actual; self.dft += actual; self.total_dft_earned += actual;
        }

        let destroyed = result.defender_destroyed;
        if destroyed {
            target.is_ruins = true; target.alliance_id = None;
            self.total_victims += 1; target.total_deaths += 1;
        }
        if target.shield_durability > 0 { target.shield_durability -= 1; }

        // 情绪更新
        if destroyed {
            self.emotion.elation = (self.emotion.elation + 0.15).min(1.0);
            self.emotion.anger = (self.emotion.anger - 0.1).max(0.0);
            self.consecutive_wins += 1;
            self.consecutive_losses = 0;
        } else {
            self.emotion.elation = (self.emotion.elation + 0.05).min(1.0);
            self.consecutive_wins += 1;
            self.consecutive_losses = 0;
        }

        // 目标情绪
        target.emotion.anger = (target.emotion.anger + 0.25).min(1.0);
        target.emotion.fear = (target.emotion.fear + 0.15 * (1.0 - target.personality.boldness)).min(1.0);

        // 社交网络: 相互标记
        target.add_enemy(self.id.clone(), 0.5);
        self.add_enemy(target.id.clone(), 0.2); // 攻击者轻度标记对方(可能报复)

        // 仇恨链: 攻击者的仇人也是目标的仇人 (敌人的朋友也是敌人)
        for (eid, _) in &self.enemies {
            if eid != &target.id {
                target.add_enemy(eid.clone(), 0.15);
            }
        }

        AttackOutcome::Success(result)
    }

    pub fn add_enemy(&mut self, player_id: String, severity: f64) {
        if let Some(pos) = self.enemies.iter().position(|(id, _)| *id == player_id) {
            self.enemies[pos].1 = (self.enemies[pos].1 + severity).min(1.0);
        } else {
            self.enemies.push((player_id, severity));
            if self.enemies.len() > 30 { self.enemies.remove(0); }
        }
    }

    pub fn has_enemy(&self, player_id: &str) -> f64 {
        self.enemies.iter().find(|(id, _)| id == player_id).map(|(_, s)| *s).unwrap_or(0.0)
    }

    pub fn add_friend(&mut self, player_id: String, trust: f64) {
        if let Some(pos) = self.friends.iter().position(|(id, _)| *id == player_id) {
            self.friends[pos].1 = (self.friends[pos].1 + trust).min(1.0);
        } else {
            self.friends.push((player_id, trust));
            if self.friends.len() > 20 { self.friends.remove(0); }
        }
    }

    pub fn try_rebuild(&mut self, is_post_scarcity: bool) -> bool {
        if !self.is_ruins { return false; }
        let cost_dft = if is_post_scarcity { 0 } else { 50 * 10u128.pow(18) };
        let cost_energy = 5000 * if is_post_scarcity { 3 } else { 1 };
        if !is_post_scarcity && self.dft < cost_dft { return false; }
        if self.energy < cost_energy { return false; }
        if !is_post_scarcity { self.dft -= cost_dft; }
        self.energy -= cost_energy;

        // 重建前检查是否 10 级以上 (总等级 >= 50)
        let was_high_level = self.total_level() >= 50;
        if was_high_level {
            self.rebirth_count += 1;
            // 成长系数: 1.0 + rebirth_count * 0.12, 上限 3.0
            // 1次: 1.12x, 5次: 1.6x, 10次: 2.2x, 17次: 3.0x cap
            self.growth_multiplier = (1.0 + self.rebirth_count as f64 * 0.12).min(3.0);
        }

        self.is_ruins = false;
        self.health = m::MAX_HEALTH / 2;
        self.shield_hp = m::calc_shield_hp(self.shield_lv) / 4;
        self.collector_durability = m::calc_max_durability(self.collector_lv) / 4;
        self.creation_time = self.last_collect_time;
        self.total_rebuilds += 1;
        self.emotion.fear = (self.emotion.fear * 0.5).max(0.1);
        self.consecutive_losses = 0;
        true
    }

    /// 每日状态更新 (情绪衰减, 非理性行为, 弃坑检查)
    pub fn daily_update(&mut self, day: u64) -> bool {
        self.emotion.decay();
        self.days_since_last_attack += 1;
        self.days_since_last_death += 1;
        self.last_quit_check = day;

        // 连胜/连败衰减 (太久没打架就清零)
        if self.days_since_last_attack > 10 {
            self.consecutive_wins = 0;
            self.consecutive_losses = 0;
        }

        // 摆烂积累: 长期无事/连续失败 → burnout↑
        if self.days_since_last_attack > 20 {
            self.burnout = (self.burnout + 0.01).min(1.0);
        }
        if self.consecutive_losses > 5 {
            self.burnout = (self.burnout + 0.05).min(1.0);
        }
        // 成功减少摆烂
        if self.consecutive_wins > 3 {
            self.burnout = (self.burnout - 0.03).max(0.0);
        }

        // 上头衰减
        self.tilt_level = (self.tilt_level * 0.9 - 0.01).max(0.0);
        // 连败 → tilt↑
        if self.consecutive_losses > 2 {
            self.tilt_level = (self.tilt_level + 0.05 * self.consecutive_losses as f64).min(1.0);
        }

        // 弃坑检查: 摆烂太久或连续死亡
        let should_quit = self.burnout > 0.9
            || (self.consecutive_losses > 10 && self.personality.emotionality > 0.6)
            || (self.total_deaths > 5 && self.burnout > 0.5);
        if should_quit && self.quit_day.is_none() {
            self.quit_day = Some(day);
            return true; // 弃坑
        }

        // 弃坑后可能回归 (小概率)
        if self.quit_day.is_some() && self.is_ruins {
            if rand::random::<f64>() < 0.001 {
                // 回归!
                self.quit_day = None;
                self.burnout = 0.3;
                self.tilt_level = 0.0;
                self.is_ruins = false;
                self.health = m::MAX_HEALTH / 2;
                self.shield_hp = m::calc_shield_hp(self.shield_lv) / 4;
                self.energy = m::INITIAL_ENERGY;
                self.total_rebuilds += 1;
                return true;
            }
        }

        false
    }

    /// 信誉更新
    pub fn update_reputation(&mut self) {
        // 杀太多人 → 信誉下降; 被杀 → 信誉上升(受害者同情)
        let raw = 0.5
            - (self.total_victims as f64 * 0.01).min(0.5)
            + (self.total_deaths as f64 * 0.02).min(0.3);
        self.reputation = raw.clamp(-1.0, 1.0);
    }
}

#[derive(Debug)]
pub enum AttackOutcome {
    Fail, NewbieProtected, SameAlliance, LowEnergy, RateLimited, Success(m::AttackResult),
}

/// 生成随机 EVM 地址
pub fn random_evm_address(rng: &mut impl rand::Rng) -> String {
    let bytes: [u8; 20] = rng.gen();
    format!("0x{}", hex::encode(bytes))
}
