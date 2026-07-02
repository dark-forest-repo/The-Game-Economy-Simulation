//! EntityStore — SoA (Struct of Arrays) 数据存储
//!
//! 所有玩家数据按列存储: energy[42], health[42], weapon_lv[42] ...
//! 替代 HashMap<String, Civilization> 的时代。
//!
//! 热字段 (攻击路径) 在各自 Vec 里连续排列, CPU cache 友好。
//! 冷字段在后面, 不影响攻击性能。

#![allow(dead_code)]

use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════
// EntityStore — 核心 SoA 结构
// ═══════════════════════════════════════════════════════════

pub struct EntityStore {
    // ── 热字段 (攻击路径, 必须紧凑) ──
    pub energy: Vec<u128>,
    pub health: Vec<u128>,
    pub shield_hp: Vec<u128>,
    pub weapon_lv: Vec<u8>,
    pub shield_lv: Vec<u8>,
    pub x: Vec<i128>, pub y: Vec<i128>, pub z: Vec<i128>,
    pub attack_tokens: Vec<u8>,
    pub shield_durability: Vec<u32>,
    pub is_ruins: Vec<u8>,
    pub alliance_idx: Vec<Option<u32>>,
    pub is_newbie_until: Vec<u64>,

    // ── 温字段 (每天碰一两次) ──
    pub dft: Vec<u128>,
    pub collector_lv: Vec<u8>,
    pub radar_lv: Vec<u8>,
    pub engine_lv: Vec<u8>,
    pub collector_durability: Vec<u64>,
    pub last_collect_time: Vec<u64>,
    pub last_token_time: Vec<u64>,
    pub creation_time: Vec<u64>,
    pub max_health: Vec<u32>,
    pub total_attacks: Vec<u32>,
    pub total_victims: Vec<u32>,
    pub total_deaths: Vec<u32>,
    pub total_dft_spent: Vec<u128>,
    pub total_dft_earned: Vec<u128>,
    pub total_plundered: Vec<u128>,
    pub total_rebuilds: Vec<u32>,
    pub total_energy_collected: Vec<u128>,
    pub rebirth_count: Vec<u16>,
    pub growth_multiplier: Vec<f32>,
    pub attack_count_today: Vec<u16>,

    // ── 人格 (行为决策用) ──
    pub aggression: Vec<u8>,
    pub greed: Vec<u8>,
    pub boldness: Vec<u8>,
    pub sociability: Vec<u8>,
    pub emotionality: Vec<u8>,

    // ── 情绪 ──
    pub anger: Vec<u8>,
    pub fear: Vec<u8>,
    pub elation: Vec<u8>,
    pub boredom: Vec<u8>,
    pub tilt_level: Vec<u8>,
    pub burnout: Vec<u8>,
    pub consecutive_wins: Vec<u16>,
    pub consecutive_losses: Vec<u16>,

    // ── 社交 (少访问) ──
    pub enemies: Vec<Vec<(u32, u8)>>,  // (target_idx, severity 0-255)

    // ── 邀请系统 ──
    pub invites_remaining: Vec<u8>,
    pub invited_by: Vec<Option<u32>>,
    pub referral_count: Vec<u16>,

    // ── 杂项 ──
    pub days_since_last_attack: Vec<u32>,
    pub days_since_last_death: Vec<u32>,
    pub generation: Vec<u32>,

    // ── 字符串 (极少访问) ──
    pub ids: Vec<String>,
    pub addresses: Vec<String>,
    pub names: Vec<String>,
    pub personality_types: Vec<&'static str>,

    // ── 索引 ──
    pub id_map: HashMap<String, u32>,
    pub free_list: Vec<u32>,
    pub active_count: u32,

    pub next_id: u32,
}

impl EntityStore {
    pub fn new() -> Self {
        Self {
            energy: Vec::new(), health: Vec::new(), shield_hp: Vec::new(),
            weapon_lv: Vec::new(), shield_lv: Vec::new(),
            x: Vec::new(), y: Vec::new(), z: Vec::new(),
            attack_tokens: Vec::new(), shield_durability: Vec::new(),
            is_ruins: Vec::new(), alliance_idx: Vec::new(), is_newbie_until: Vec::new(),
            dft: Vec::new(), collector_lv: Vec::new(), radar_lv: Vec::new(),
            engine_lv: Vec::new(), collector_durability: Vec::new(),
            last_collect_time: Vec::new(), last_token_time: Vec::new(),
            creation_time: Vec::new(), max_health: Vec::new(),
            total_attacks: Vec::new(), total_victims: Vec::new(), total_deaths: Vec::new(),
            total_dft_spent: Vec::new(), total_dft_earned: Vec::new(),
            total_plundered: Vec::new(), total_rebuilds: Vec::new(),
            total_energy_collected: Vec::new(),
            rebirth_count: Vec::new(), growth_multiplier: Vec::new(),
            attack_count_today: Vec::new(),
            aggression: Vec::new(), greed: Vec::new(), boldness: Vec::new(),
            sociability: Vec::new(), emotionality: Vec::new(),
            anger: Vec::new(), fear: Vec::new(), elation: Vec::new(),
            boredom: Vec::new(), tilt_level: Vec::new(), burnout: Vec::new(),
            consecutive_wins: Vec::new(), consecutive_losses: Vec::new(),
            enemies: Vec::new(),
            invites_remaining: Vec::new(), invited_by: Vec::new(), referral_count: Vec::new(),
            days_since_last_attack: Vec::new(), days_since_last_death: Vec::new(),
            generation: Vec::new(),
            ids: Vec::new(), addresses: Vec::new(), names: Vec::new(),
            personality_types: Vec::new(),
            id_map: HashMap::new(), free_list: Vec::new(),
            active_count: 0, next_id: 0,
        }
    }

    // ── 分配 ──

    /// 新增一个玩家, 返回索引
    pub fn alloc(&mut self) -> u32 {
        let idx = if let Some(free) = self.free_list.pop() {
            free
        } else {
            let i = self.next_id;
            self.next_id += 1;
            // 预填充所有 Vec (push default)
            self._grow_to(i as usize + 1);
            i
        };
        self._reset_at(idx as usize);
        idx
    }

    /// 释放一个玩家索引, 标记为可用
    pub fn free(&mut self, idx: u32) {
        self.is_ruins[idx as usize] = 1;
        self.free_list.push(idx);
    }

    fn _grow_to(&mut self, n: usize) {
        macro_rules! grow {
            ($v:ident, $val:expr) => { while self.$v.len() < n { self.$v.push($val); } };
            ($v:ident) => { while self.$v.len() < n { self.$v.push(Default::default()); } };
        }
        grow!(energy, 0); grow!(health, 0); grow!(shield_hp, 0);
        grow!(weapon_lv, 0); grow!(shield_lv, 0);
        grow!(x, 0); grow!(y, 0); grow!(z, 0);
        grow!(attack_tokens, 0); grow!(shield_durability, 0);
        grow!(is_ruins, 1); grow!(alliance_idx, None);
        grow!(is_newbie_until, 0);
        grow!(dft, 0);
        grow!(collector_lv, 0); grow!(radar_lv, 0); grow!(engine_lv, 0);
        grow!(collector_durability, 0);
        grow!(last_collect_time, 0); grow!(last_token_time, 0);
        grow!(creation_time, 0); grow!(max_health, 0);
        grow!(total_attacks, 0); grow!(total_victims, 0); grow!(total_deaths, 0);
        grow!(total_dft_spent, 0); grow!(total_dft_earned, 0);
        grow!(total_plundered, 0); grow!(total_rebuilds, 0);
        grow!(total_energy_collected, 0);
        grow!(rebirth_count, 0); grow!(growth_multiplier, 1.0f32);
        grow!(attack_count_today, 0);
        grow!(aggression, 50); grow!(greed, 50); grow!(boldness, 50);
        grow!(sociability, 50); grow!(emotionality, 50);
        grow!(anger, 0); grow!(fear, 0); grow!(elation, 0);
        grow!(boredom, 50); grow!(tilt_level, 0); grow!(burnout, 0);
        grow!(consecutive_wins, 0); grow!(consecutive_losses, 0);
        grow!(enemies, Vec::new());
        grow!(invites_remaining, 0); grow!(invited_by, None); grow!(referral_count, 0);
        grow!(days_since_last_attack, 0); grow!(days_since_last_death, 0);
        grow!(generation, 0);
        grow!(ids, String::new()); grow!(addresses, String::new());
        grow!(names, String::new()); grow!(personality_types, "");
    }

    fn _reset_at(&mut self, i: usize) {
        self.energy[i] = 2000; self.health[i] = 3000; self.shield_hp[i] = 3615;
        self.weapon_lv[i] = 1; self.shield_lv[i] = 1;
        self.x[i] = 0; self.y[i] = 0; self.z[i] = 0;
        self.attack_tokens[i] = 3; self.shield_durability[i] = 259200;
        self.is_ruins[i] = 0; self.alliance_idx[i] = None; self.is_newbie_until[i] = u64::MAX;
        self.dft[i] = 0;
        self.collector_lv[i] = 1; self.radar_lv[i] = 1; self.engine_lv[i] = 1;
        self.collector_durability[i] = 86400;
        self.last_collect_time[i] = 0; self.last_token_time[i] = 0;
        self.creation_time[i] = 0; self.max_health[i] = 20000;
        self.total_attacks[i] = 0; self.total_victims[i] = 0; self.total_deaths[i] = 0;
        self.total_dft_spent[i] = 0; self.total_dft_earned[i] = 0;
        self.total_plundered[i] = 0; self.total_rebuilds[i] = 0;
        self.total_energy_collected[i] = 0;
        self.rebirth_count[i] = 0; self.growth_multiplier[i] = 1.0;
        self.attack_count_today[i] = 0;
        self.aggression[i] = 50; self.greed[i] = 50; self.boldness[i] = 50;
        self.sociability[i] = 50; self.emotionality[i] = 50;
        self.anger[i] = 0; self.fear[i] = 0; self.elation[i] = 0;
        self.boredom[i] = 50; self.tilt_level[i] = 0; self.burnout[i] = 0;
        self.consecutive_wins[i] = 0; self.consecutive_losses[i] = 0;
        self.enemies[i].clear();
        self.invites_remaining[i] = 0; self.invited_by[i] = None; self.referral_count[i] = 0;
        self.days_since_last_attack[i] = 0; self.days_since_last_death[i] = 0;
        self.generation[i] = 0;
        self.ids[i].clear(); self.addresses[i].clear(); self.names[i].clear();
        self.personality_types[i] = "";
    }

    // ── 访问辅助 ──

    pub fn is_active(&self, idx: u32) -> bool {
        let i = idx as usize;
        i < self.ids.len() && self.is_ruins[i] == 0
    }

    pub fn total_level(&self, idx: u32) -> u32 {
        let i = idx as usize;
        self.collector_lv[i] as u32 + self.weapon_lv[i] as u32
            + self.shield_lv[i] as u32 + self.radar_lv[i] as u32
            + self.engine_lv[i] as u32
    }

    pub fn shield_percent(&self, idx: u32) -> u8 {
        let i = idx as usize;
        let hp = self.shield_hp[i];
        let max = crate::math_engine::calc_shield_hp(self.shield_lv[i] as u128);
        if max == 0 { return 0; }
        (hp * 100 / max) as u8
    }

    /// 添加仇人
    pub fn add_enemy(&mut self, idx: u32, enemy_idx: u32, severity: u8) {
        let enemies = &mut self.enemies[idx as usize];
        if let Some(pos) = enemies.iter().position(|(id, _)| *id == enemy_idx) {
            enemies[pos].1 = enemies[pos].1.saturating_add(severity).min(255);
        } else {
            enemies.push((enemy_idx, severity));
            if enemies.len() > 30 { enemies.remove(0); }
        }
    }

    pub fn has_enemy(&self, idx: u32, enemy_idx: u32) -> u8 {
        self.enemies[idx as usize].iter()
            .find(|(id, _)| *id == enemy_idx)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    }

    // ── 迭代 ──

    /// 活跃玩家索引迭代器 (不返回废墟)
    pub fn active_indices(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.next_id).filter(|&i| self.is_active(i))
    }
}
