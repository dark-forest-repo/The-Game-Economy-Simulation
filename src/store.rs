//! EntityStore — SoA 热数据 (RAM) + ColdStore (mmap)
//! 冷字段 (人格/情绪/统计) 移到 mmap 文件, OS 按需换入换出。

#![allow(dead_code)]

use std::collections::HashMap;
use crate::coldstore::ColdStore;

pub struct EntityStore {
    pub cold: ColdStore,

    // ── 热字段 (攻击路径, RAM) ──
    pub energy: Vec<u128>,
    pub health: Vec<u128>,
    pub shield_hp: Vec<u128>,
    pub dft: Vec<u128>,
    pub weapon_lv: Vec<u8>,
    pub shield_lv: Vec<u8>,
    pub collector_lv: Vec<u8>,
    pub radar_lv: Vec<u8>,
    pub engine_lv: Vec<u8>,
    pub x: Vec<i128>, pub y: Vec<i128>, pub z: Vec<i128>,
    pub attack_tokens: Vec<u8>,
    pub shield_durability: Vec<u32>,
    pub is_ruins: Vec<u8>,
    pub alliance_idx: Vec<Option<u32>>,
    pub is_newbie_until: Vec<u64>,
    pub last_collect_time: Vec<u64>,
    pub last_token_time: Vec<u64>,
    pub creation_time: Vec<u64>,
    pub max_health: Vec<u32>,
    pub collector_durability: Vec<u64>,

    // ── 半热 (战争联盟用) ──
    pub total_attacks: Vec<u32>,
    pub total_victims: Vec<u32>,
    pub total_deaths: Vec<u32>,

    // ── 社交 (结构复杂, 留 RAM) ──
    pub enemies: Vec<Vec<(u32, u8)>>,

    // ── 字符串 ──
    pub ids: Vec<String>,
    pub addresses: Vec<String>,
    pub names: Vec<String>,
    pub personality_types: Vec<&'static str>,

    // ── 索引 ──
    pub id_map: HashMap<String, u32>,
    pub free_list: Vec<u32>,
    pub next_id: u32,
}

impl EntityStore {
    pub fn new() -> Self {
        let cold = ColdStore::new("/tmp/df_cold.dat", 65536).expect("ColdStore init");
        Self {
            cold,
            energy: Vec::new(), health: Vec::new(), shield_hp: Vec::new(), dft: Vec::new(),
            weapon_lv: Vec::new(), shield_lv: Vec::new(),
            collector_lv: Vec::new(), radar_lv: Vec::new(), engine_lv: Vec::new(),
            x: Vec::new(), y: Vec::new(), z: Vec::new(),
            attack_tokens: Vec::new(), shield_durability: Vec::new(),
            is_ruins: Vec::new(), alliance_idx: Vec::new(), is_newbie_until: Vec::new(),
            last_collect_time: Vec::new(), last_token_time: Vec::new(), creation_time: Vec::new(),
            max_health: Vec::new(), collector_durability: Vec::new(),
            total_attacks: Vec::new(), total_victims: Vec::new(), total_deaths: Vec::new(),
            enemies: Vec::new(),
            ids: Vec::new(), addresses: Vec::new(), names: Vec::new(),
            personality_types: Vec::new(),
            id_map: HashMap::new(), free_list: Vec::new(), next_id: 0,
        }
    }

    pub fn alloc(&mut self) -> u32 {
        let idx = if let Some(free) = self.free_list.pop() { free }
        else { let i = self.next_id; self.next_id += 1; self._grow_to(i as usize + 1); i };
        self._reset_at(idx as usize);
        self.cold.ensure(idx);
        idx
    }

    fn _grow_to(&mut self, n: usize) {
        macro_rules! grow {
            ($v:ident, $val:expr) => { while self.$v.len() < n { self.$v.push($val); } };
            ($v:ident) => { while self.$v.len() < n { self.$v.push(Default::default()); } };
        }
        grow!(energy, 0); grow!(health, 0); grow!(shield_hp, 0); grow!(dft, 0);
        grow!(weapon_lv, 0); grow!(shield_lv, 0);
        grow!(collector_lv, 0); grow!(radar_lv, 0); grow!(engine_lv, 0);
        grow!(x, 0); grow!(y, 0); grow!(z, 0);
        grow!(attack_tokens, 0); grow!(shield_durability, 0);
        grow!(is_ruins, 1); grow!(alliance_idx, None); grow!(is_newbie_until, 0);
        grow!(last_collect_time, 0); grow!(last_token_time, 0);
        grow!(creation_time, 0); grow!(max_health, 0); grow!(collector_durability, 0);
        grow!(total_attacks, 0); grow!(total_victims, 0); grow!(total_deaths, 0);
        grow!(enemies, Vec::new());
        grow!(ids, String::new()); grow!(addresses, String::new());
        grow!(names, String::new()); grow!(personality_types, "");
    }

    fn _reset_at(&mut self, i: usize) {
        self.energy[i] = 2000; self.health[i] = 3000; self.shield_hp[i] = 3615; self.dft[i] = 0;
        self.weapon_lv[i] = 1; self.shield_lv[i] = 1; self.collector_lv[i] = 1;
        self.radar_lv[i] = 1; self.engine_lv[i] = 1;
        self.x[i] = 0; self.y[i] = 0; self.z[i] = 0;
        self.attack_tokens[i] = 3; self.shield_durability[i] = 259200;
        self.is_ruins[i] = 0; self.alliance_idx[i] = None; self.is_newbie_until[i] = u64::MAX;
        self.last_collect_time[i] = 0; self.last_token_time[i] = 0; self.creation_time[i] = 0;
        self.max_health[i] = 20000; self.collector_durability[i] = 86400;
        self.total_attacks[i] = 0; self.total_victims[i] = 0; self.total_deaths[i] = 0;
        self.enemies[i].clear();
        self.ids[i].clear(); self.addresses[i].clear(); self.names[i].clear();
        self.personality_types[i] = "";

        // 冷数据重置
        let c = &mut self.cold;
        let idx = i as u32;
        c.set_dft_spent(idx, 0); c.set_dft_earned(idx, 0);
        c.set_plundered(idx, 0); c.set_energy_collected(idx, 0);
        c.set_rebuilds(idx, 0); c.set_rebirth_count(idx, 0);
        c.set_growth_multiplier(idx, 1.0); c.set_attack_count_today(idx, 0);
        c.set_aggression(idx, 50); c.set_greed(idx, 50); c.set_boldness(idx, 50);
        c.set_sociability(idx, 50); c.set_emotionality(idx, 50);
        c.set_anger(idx, 0); c.set_fear(idx, 0); c.set_elation(idx, 0);
        c.set_boredom(idx, 50); c.set_tilt_level(idx, 0); c.set_burnout(idx, 0);
        c.set_consecutive_wins(idx, 0); c.set_consecutive_losses(idx, 0);
        c.set_days_since_attack(idx, 0); c.set_days_since_death(idx, 0);
        c.set_generation(idx, 0);
        c.set_invites_remaining(idx, 0); c.set_invited_by(idx, None);
        c.set_referral_count(idx, 0);
    }

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

    pub fn active_indices(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.next_id).filter(|&i| self.is_active(i))
    }
}
