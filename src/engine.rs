// Dark Forest 引擎 — 人格驱动 + 邀请系统 + 联盟外交 + 非理性行为
#![allow(dead_code)]

use std::collections::{HashMap, BTreeSet};
use rand::Rng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::math_engine as m;
use crate::player::{self, Civilization, AttackOutcome, Personality, SPAWN_DISTRIBUTION};
use crate::battle_engine::{BattleEngine, AttackOrder};

// ═══════════════════════════════════════════════════════════
// 配置
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub simulation_days: u64,
    pub seed: u64,
    pub dft_daily_emission: u128,
    pub spawn_in_cluster: bool,
    pub cluster_radius: u128,
    pub random_spawn_pct: f64,
    pub initial_players: usize,    // 初始玩家人数
    pub daily_spawn: usize,        // 每日基础自然流量
    pub daily_spawn_variance: f64, // 每日波动比例 (0.5 = ±50%)
    pub invite_enabled: bool,
    pub diplomacy_enabled: bool,
    pub rebuild_enabled: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            simulation_days: 3650, seed: 42,
            dft_daily_emission: m::DAILY_DFT_EMISSION,
            spawn_in_cluster: true, cluster_radius: 3000,
            random_spawn_pct: 0.05,
            initial_players: 200,
            daily_spawn: 20,
            daily_spawn_variance: 0.5,
            invite_enabled: true, diplomacy_enabled: true, rebuild_enabled: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// 联盟数据 (含外交)
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AllianceData {
    pub id: String,
    pub leader: String,
    pub members: Vec<String>,
    pub created_at: u128,
    pub cohesion: f64,
    pub alive_member_count: usize,  // 缓存, 每天更新
    pub war_targets: Vec<String>,
    pub allies: Vec<String>,
    pub total_kills: u128,
    pub total_deaths: u128,
}

impl AllianceData {
    pub fn new(id: String, leader: String, created_at: u128) -> Self {
        let leader_clone = leader.clone();
        Self {
            id, leader, members: vec![leader_clone],
            created_at, cohesion: 0.8, alive_member_count: 1,
            war_targets: Vec::new(), allies: Vec::new(),
            total_kills: 0, total_deaths: 0,
        }
    }

    pub fn alive_members(&self, players: &HashMap<String, Civilization>) -> Vec<String> {
        self.members.iter().filter(|m| {
            players.get(*m).map_or(false, |p| !p.is_ruins && !p.is_quit())
        }).cloned().collect()
    }

    pub fn alive_count(&self, players: &HashMap<String, Civilization>) -> usize {
        self.alive_members(players).len()
    }

    pub fn is_at_war(&self, other: &str) -> bool {
        self.war_targets.contains(&other.to_string())
    }
}

// ═══════════════════════════════════════════════════════════
// 指标
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SimMetrics {
    pub day: u64, pub total_spawned: usize,
    pub active_players: usize, pub ruins: usize,
    pub total_dft_minted: u128, pub total_dft_burned: u128,
    pub total_attacks: u128, pub total_deaths: u128,
    pub total_rebuilds: u128, pub total_invites: u128,
    pub total_quits: u128, pub active_alliances: usize,
    pub wars_active: usize,
    pub avg_level: f64, pub gini_energy: f64, pub gini_dft: f64,
}

// ═══════════════════════════════════════════════════════════
// 引擎
// ═══════════════════════════════════════════════════════════

pub struct GameEngine {
    pub cfg: SimConfig,
    pub rng: rand::rngs::StdRng,
    pub players: HashMap<String, Civilization>,
    pub time: u128,
    pub day: u64,
    pub generation: u64,
    initialized: bool,  // 是否已完成初始玩家生成

    pub alliances: HashMap<String, AllianceData>,
    pub alliance_enabled: bool,

    // 空间网格
    spatial_grid: HashMap<(i64, i64, i64), Vec<String>>,
    grid_size: i64,
    grid_dirty: bool,              // 网格需要重建
    last_grid_rebuild: i64,

    // 活跃玩家缓存 (避免遍历 50K HashMap)
    active_ids: Vec<String>,

    // 联盟大小索引 (BTreeSet<(alive_count, id)>, O(log N) 最小查找)
    alliance_sizes: BTreeSet<(usize, String)>,

    // 战斗撮合引擎
    battle_engine: BattleEngine,

    pub total_dft_minted: u128, pub total_dft_burned: u128,
    pub total_rebuilds: u128, pub total_invites: u128, pub total_quits: u128,
    pub total_energy_burned: u128,

    pub metrics_history: Vec<SimMetrics>,
    pub global_state: m::GlobalState,

    // 市场
    pub market_rate: u128, pub market_dft_fees: u128,
    pub market_daily_volume: u128, pub market_daily_trades: usize,

    next_player_id: u64,
}

impl GameEngine {
    pub fn new(config: SimConfig) -> Self {
        let seed = config.seed;
        Self {
            cfg: config, rng: rand::rngs::StdRng::seed_from_u64(seed),
            players: HashMap::new(),
            time: 0, day: 0, generation: 0, initialized: false,
            alliances: HashMap::new(), alliance_enabled: true,
            spatial_grid: HashMap::new(), grid_size: 10000, grid_dirty: true, last_grid_rebuild: -99999,
            active_ids: Vec::new(),
            alliance_sizes: BTreeSet::new(),
            battle_engine: BattleEngine::new(),
            total_dft_minted: 0, total_dft_burned: 0,
            total_rebuilds: 0, total_invites: 0, total_quits: 0, total_energy_burned: 0,
            metrics_history: Vec::new(),
            global_state: m::GlobalState::new(),
            market_rate: 100, market_dft_fees: 0,
            market_daily_volume: 0, market_daily_trades: 0,
            next_player_id: 0,
        }
    }

    /// 活跃玩家迭代器 (使用 active_ids 缓存, 不遍历 50K HashMap)
    pub fn active_players(&self) -> impl Iterator<Item = &Civilization> {
        self.active_ids.iter().filter_map(|id| self.players.get(id))
    }

    // --- 活跃 ID 维护 ---

    fn _add_active(&mut self, pid: &str) {
        if !self.active_ids.iter().any(|id| id == pid) {
            self.active_ids.push(pid.to_string());
            self.grid_dirty = true;
        }
    }

    fn _remove_active(&mut self, pid: &str) {
        if let Some(pos) = self.active_ids.iter().position(|id| id == pid) {
            self.active_ids.swap_remove(pos);
            self.grid_dirty = true;
        }
    }

    // ═══════════════════════════════════════════
    // 主循环
    // ═══════════════════════════════════════════

    pub(crate) fn _daily_step(&mut self) {
        self.global_state.update(self.total_dft_minted, self.total_dft_burned);

        // 0. 初始玩家 (仅第一天)
        if !self.initialized {
            self._spawn_initial_players();
            self.initialized = true;
        }

        // 1. 基础玩家注入 (每日波动)
        self._spawn_players();

        // 2. 邀请
        if self.cfg.invite_enabled { self._process_invitations(); }

        // 3. DFT 每日释放
        if self.global_state.can_mint() {
            let mut day_ids: Vec<String> = self.active_players().map(|p| p.id.clone()).collect();
            let n = day_ids.len();
            if n > 0 {
                let actual = std::cmp::min(self.cfg.dft_daily_emission, m::TOTAL_SUPPLY.saturating_sub(self.global_state.total_minted));
                let per = actual / n as u128;
                if per > 0 {
                    self.total_dft_minted += per * n as u128;
                    for pid in &day_ids {
                        if let Some(p) = self.players.get_mut(pid) {
                            p.dft += per; p.total_dft_earned += per;
                        }
                    }
                }
            }
        }

        // 4. 所有玩家行动
        let mut day_ids: Vec<String> = self.active_players().map(|p| p.id.clone()).collect();
        day_ids.shuffle(&mut self.rng);

        // Phase A: 从 HashMap 提取所有活跃玩家到 Vec (单线程)
        let mut batch: Vec<(String, Civilization)> = day_ids.iter()
            .filter_map(|pid| self.players.remove(pid).map(|c| (pid.clone(), c)))
            .collect();

        // Phase B: 并行处理玩家本地操作 (采集/升级/回盾/弃坑)
        use rayon::prelude::*;
        let is_ps = self.global_state.is_post_scarcity();
        let burned_agg = std::sync::Mutex::new(0u128);
        let quits_agg = std::sync::Mutex::new(0u128);
        let adds = std::sync::Mutex::new(Vec::<String>::new());
        let removes = std::sync::Mutex::new(Vec::<String>::new());

        batch.par_iter_mut().for_each(|(pid, civ)| {
            let was_ruins = civ.is_ruins;
            let quit = civ.daily_update(self.day);

            if quit && civ.quit_day.is_some() && !civ.is_ruins {
                civ.is_ruins = true; civ.alliance_id = None;
                *quits_agg.lock().unwrap() += 1;
                removes.lock().unwrap().push(pid.clone());
            }
            if quit && civ.quit_day.is_none() && was_ruins {
                adds.lock().unwrap().push(pid.clone());
            }
            if civ.is_ruins || civ.is_quit() { return; }

            civ.collect_energy(self.time, false);
            civ.regen_tokens(self.time);
            if civ.shield_hp > 0 {
                let mh = m::calc_shield_hp(civ.shield_lv);
                if civ.shield_hp < mh {
                    let r = m::calc_shield_regen(civ.shield_lv) * 12;
                    let c = r * m::SHIELD_REGEN_RATIO;
                    if civ.energy >= c { civ.energy -= c; civ.shield_hp = std::cmp::min(civ.shield_hp + r, mh); }
                }
            }
            let max_up = if civ.burnout > 0.8 { 3 } else if civ.burnout > 0.5 { 8 } else if civ.tilt_level > 0.5 { 30 } else { 20 };
            let plan = player::plan_upgrades(civ);
            for _ in 0..max_up {
                let mut ok = false;
                for sys in &plan {
                    let (s, b) = civ.try_upgrade(sys, self.time, is_ps);
                    if s { *burned_agg.lock().unwrap() += b; ok = true; break; }
                }
                if !ok { break; }
            }
        });

        self.total_quits += *quits_agg.lock().unwrap();
        self.total_dft_burned += *burned_agg.lock().unwrap();
        for p in removes.lock().unwrap().iter() { self._remove_active(p); }
        for p in adds.lock().unwrap().iter() { self._add_active(p); }

        // Phase C: 并行收集攻击订单 (玩家仍在 batch 中, 读 players 找目标)
        // 然后放回 HashMap
        let time = self.time;
        let grid_size = self.grid_size;
        let grid_ref = &self.spatial_grid;
        let alliances_ref = &self.alliances;
        let mut all_orders: Vec<AttackOrder> = Vec::new();
        let mut destroyed_targets: Vec<String> = Vec::new();

        use rayon::prelude::*;
        let orders_mutex = std::sync::Mutex::new(&mut all_orders);

        // 先放回 HashMap (为攻击找目标提供数据)
        for (pid, civ) in batch.drain(..) {
            self.players.insert(pid, civ);
        }

        // 重新取出: 这次只做攻击
        let mut day_ids_attack: Vec<String> = self.active_players().map(|p| p.id.clone()).collect();
        day_ids_attack.shuffle(&mut self.rng);

        // 并行: 找目标, 生成攻击订单 (搜一次网格, 候选列表缓存)
        let attack_orders: std::sync::Mutex<Vec<AttackOrder>> = std::sync::Mutex::new(Vec::new());
        day_ids_attack.par_iter().for_each(|pid| {
            let civ = match self.players.get(pid) { Some(c) => c, None => return };
            if civ.is_ruins || civ.is_quit() { return; }

            let budget = civ.max_attacks_per_day();
            let threshold = civ.attack_shield_threshold();
            let focus = civ.focus_fire();
            let mut my_orders = Vec::with_capacity(budget.min(10));
            let mut submitted_targets = std::collections::HashSet::new();

            // 搜一次网格, 获取排序后的候选列表
            let candidates = Self::_find_all_targets(civ, &self.players, grid_ref, time, grid_size,
                &mut rand::thread_rng(), budget * 2);

            for (_, tid) in &candidates {
                if my_orders.len() >= budget { break; }

                let other = match self.players.get(tid) { Some(o) => o, None => continue };
                if other.shield_percent() > threshold { continue; }

                let energy_cost = m::calc_attack_energy_cost(civ.weapon_lv);
                if civ.energy < energy_cost * (my_orders.len() as u128 + 1) { break; }

                let bonus = other.alliance_id.as_ref().and_then(|aid| {
                    alliances_ref.get(aid).map(|a| a.alive_member_count.saturating_sub(1) as u128 * m::ALLIANCE_DEF_BONUS)
                }).unwrap_or(0);

                // 计算需要多少次攻击才能杀死该目标 (focus_fire 时多发)
                let orders_for_this_target = if focus {
                    let atk = m::calc_attack(civ.weapon_lv);
                    let def = m::calc_defense(other.shield_lv) + bonus;
                    let dmg_per_hit = if atk > def { (atk - def) * 2 + (atk + m::SHIELD_DMG_BONUS - def).max(0) } else { 0 };
                    if dmg_per_hit > 0 {
                        let total_hp = other.health + other.shield_hp;
                        ((total_hp + dmg_per_hit - 1) / dmg_per_hit).min(10) as usize // 最多10次
                    } else { 1 }
                } else { 1 };

                if submitted_targets.contains(tid) && !focus { continue; }

                for _ in 0..orders_for_this_target {
                    if my_orders.len() >= budget { break; }
                    my_orders.push(AttackOrder {
                        attacker_id: pid.clone(),
                        target_id: tid.clone(),
                        attacker_weapon_lv: civ.weapon_lv,
                        attacker_energy: civ.energy,
                        energy_cost,
                        has_tokens: civ.attack_tokens > 0,
                        time,
                        alliance_bonus: bonus,
                        attacker_alliance: civ.alliance_id.clone(),
                    });
                }

                submitted_targets.insert(tid.clone());
            }

            attack_orders.lock().unwrap().extend(my_orders);
        });

        // Phase D: 战斗引擎撮合所有订单
        let mut engine = BattleEngine::new();
        engine.submit_batch(attack_orders.into_inner().unwrap());
        let results = engine.execute(&mut self.players);

        // 处理结果
        for r in &results {
            if r.success {
                // 更新攻击者的攻击计数 (通过结果关联)
                if let Some(attacker) = self.players.get_mut(&r.attacker_id) {
                    attacker.attack_count_today += 1;
                }
                if r.target_destroyed {
                    destroyed_targets.push(r.target_id.clone());
                }
            }
        }
        for tid in &destroyed_targets {
            self._remove_active(tid);
        }

        // 5. 联盟外交
        if self.cfg.diplomacy_enabled { self._alliance_diplomacy_tick(); }

        // 6. 市场
        self._market_tick();

        // 7. 重建 (非弃坑废墟)
        if self.cfg.rebuild_enabled {
            let ruined: Vec<String> = self.players.values()
                .filter(|p| p.is_ruins && !p.is_quit())
                .map(|p| p.id.clone()).collect();
            let limit = std::cmp::min(5, ruined.len());
            let mut count = 0;
            for rid in &ruined {
                if count >= limit { break; }
                if let Some(civ) = self.players.get_mut(rid) {
                    if civ.try_rebuild(self.global_state.is_post_scarcity()) {
                        self.total_rebuilds += 1;
                        self._add_active(rid);
                        self._add_to_alliance_by_id(&rid.clone());
                        count += 1;
                    }
                }
            }
        }

        // 8. 空间网格
        self._rebuild_grid();
    }

    // ═══════════════════════════════════════════
    // 玩家生成
    // ═══════════════════════════════════════════

    fn _spawn_initial_players(&mut self) {
        let count = self.cfg.initial_players;
        if count == 0 { return; }
        let types: Vec<&str> = SPAWN_DISTRIBUTION.iter().map(|(n, _)| *n).collect();
        let weights: Vec<f64> = SPAWN_DISTRIBUTION.iter().map(|(_, w)| *w).collect();
        for _ in 0..count {
            let (civ, pid) = self._create_player(&types, &weights, None, None);
            self.players.insert(pid.clone(), civ);
            self._add_active(&pid);
            self._add_to_alliance_by_id(&pid);
        }
        println!("  🌱 初始 {} 名玩家加入", count);
    }

    fn _spawn_players(&mut self) {
        let base = self.cfg.daily_spawn as f64;
        if base <= 0.0 { return; }
        let variance = self.cfg.daily_spawn_variance;
        let count = (base * (1.0 + self.rng.gen_range(-variance..=variance))).round() as usize;
        let count = count.max(1);
        self.generation += 1;
        let types: Vec<&str> = SPAWN_DISTRIBUTION.iter().map(|(n, _)| *n).collect();
        let weights: Vec<f64> = SPAWN_DISTRIBUTION.iter().map(|(_, w)| *w).collect();
        for _ in 0..count {
            let (civ, pid) = self._create_player(&types, &weights, None, None);
            self.players.insert(pid.clone(), civ);
            self._add_active(&pid);
            self._add_to_alliance_by_id(&pid);
        }
    }

    fn _create_player(
        &mut self, types: &[&'static str], weights: &[f64],
        near_x: Option<i128>, inviter_id: Option<&str>,
    ) -> (Civilization, String) {
        let idx = weighted_choice(&mut self.rng, weights);
        let stype = types[idx];

        let personality_base = match stype {
            "farmer" => Personality::farmer(), "balanced" => Personality::balanced(),
            "hunter" => Personality::hunter(), "whale" => Personality::whale(),
            "turtle" => Personality::turtle(), "nomad" => Personality::nomad(),
            "merchant" => Personality::merchant(), "general" => Personality::general(),
            "scavenger" => Personality::scavenger(), "berserker" => Personality::berserker(),
            _ => Personality::balanced(),
        };
        let personality = personality_base.jitter(&mut self.rng);
        let invites = if inviter_id.is_some() { self.rng.gen_range(1..=3) } else { self.rng.gen_range(2..=4) };

        let pid = format!("g{}p{:06}", self.generation, self.next_player_id);
        let addr = player::random_evm_address(&mut self.rng);
        let name = format!("{}_{}", stype, self.next_player_id);
        self.next_player_id += 1;

        let mut civ = Civilization::new(
            pid.clone(), addr, name, self.generation, personality, stype,
            self.time.saturating_sub(86401), self.time, self.time, invites,
        );
        civ.invited_by = inviter_id.map(|s| s.to_string());

        if let Some(nx) = near_x {
            civ.x = nx + self.rng.gen_range(-2000i128..=2000);
            civ.y = self.rng.gen_range(-2000i128..=2000);
            civ.z = self.rng.gen_range(-2000i128..=2000);
        } else if self.cfg.spawn_in_cluster {
            let r: f64 = self.rng.gen_range(0.0..=1.0f64) * self.cfg.cluster_radius as f64;
            let theta: f64 = self.rng.gen_range(0.0..std::f64::consts::TAU);
            let phi: f64 = self.rng.gen_range(0.0..std::f64::consts::PI);
            civ.x = (r * phi.sin() * theta.cos()) as i128;
            civ.y = (r * phi.sin() * theta.sin()) as i128;
            civ.z = (r * phi.cos()) as i128;
        } else {
            let range = 1i128 << 40;
            civ.x = self.rng.gen_range(-range..=range);
            civ.y = self.rng.gen_range(-range..=range);
            civ.z = self.rng.gen_range(-range..=range);
        }
        (civ, pid)
    }

    // ═══════════════════════════════════════════
    // 邀请系统 (密度依赖, 无硬上限)
    // ═══════════════════════════════════════════

    fn _process_invitations(&mut self) {
        let types: Vec<&str> = SPAWN_DISTRIBUTION.iter().map(|(n, _)| *n).collect();
        let weights: Vec<f64> = SPAWN_DISTRIBUTION.iter().map(|(_, w)| *w).collect();
        let active_count = self.active_players().count() as f64;

        // 密度依赖: 人口越多, 邀请概率越低 (logistic 模型)
        // 承载量 ~50000, 当前人口接近时邀请率下降
        let carrying_capacity = 50000.0;
        let density = (active_count / carrying_capacity).min(1.0);
        let density_factor = (1.0 - density).max(0.05);

        let mut inviters: Vec<(String, i128, bool)> = Vec::new();
        for (pid, civ) in &self.players {
            if civ.is_ruins || civ.is_quit() { continue; }
            if civ.invites_remaining == 0 || civ.is_newbie(self.time) { continue; }

            let is_invited = civ.invited_by.is_some();
            // 被邀请来的玩家更大概率邀请 (合约奖励机制: referral reward)
            // 被邀请: 3% + sociability*4% + level/100*3%
            // 基础: 1.5% + sociability*3% + level/100*2%
            let base_prob = if is_invited { 0.03 } else { 0.015 };
            let prob = (base_prob
                + civ.personality.sociability * if is_invited { 0.04 } else { 0.03 }
                + (civ.total_level() as f64 / 100.0) * if is_invited { 0.03 } else { 0.02 }) * density_factor;

            if self.rng.gen::<f64>() < prob {
                inviters.push((pid.clone(), civ.x, is_invited));
            }
        }

        for (inviter_id, inviter_x, _) in inviters {
            let inviter_ok = match self.players.get_mut(&inviter_id) {
                Some(inv) if inv.invites_remaining > 0 && !inv.is_ruins && !inv.is_quit() => {
                    inv.invites_remaining -= 1; inv.referral_count += 1; true
                }
                _ => false,
            };
            if !inviter_ok { continue; }

            if let Some(inviter) = self.players.get_mut(&inviter_id) {
                inviter.dft += m::REFERRAL_REWARD * 10u128.pow(18);
                inviter.energy += 1000;
            }

            let (mut invitee, pid) = self._create_player(&types, &weights, Some(inviter_x), Some(&inviter_id));
            invitee.dft += 200 * 10u128.pow(18);

            let inviter_aid = self.players.get(&inviter_id).and_then(|c| c.alliance_id.clone());
            self.players.insert(pid.clone(), invitee);
            self._add_active(&pid);
            self.total_invites += 1;

            if let Some(aid) = inviter_aid {
                if let Some(inv) = self.players.get_mut(&pid) { inv.alliance_id = Some(aid.clone()); }
                if let Some(alliance) = self.alliances.get_mut(&aid) {
                    if alliance.members.len() < m::MAX_ALLIANCE_MEMBERS {
                        alliance.members.push(pid.clone());
                        // 好友链: 邀请者与被邀请者成为好友
                        if let Some(inv) = self.players.get_mut(&inviter_id) {
                            inv.add_friend(pid.clone(), 0.6);
                        }
                        if let Some(invitee) = self.players.get_mut(&pid) {
                            invitee.add_friend(inviter_id.clone(), 0.6);
                        }
                        continue;
                    }
                }
            }
            self._add_to_alliance_by_id(&pid);
        }
    }

    // ═══════════════════════════════════════════
    // 联盟 (BTreeSet 索引优化)
    // ═══════════════════════════════════════════

    /// 更新联盟大小索引
    fn _update_alliance_size(&mut self, aid: &str, old_count: usize, new_count: usize) {
        self.alliance_sizes.remove(&(old_count, aid.to_string()));
        if new_count > 0 {
            self.alliance_sizes.insert((new_count, aid.to_string()));
        }
    }

    fn _add_to_alliance_by_id(&mut self, civ_id: &str) {
        let civ = match self.players.get(civ_id) {
            Some(c) => c, None => return,
        };
        if !self.alliance_enabled || !civ.prefer_alliance() || civ.is_quit() { return; }

        // 熟人优先: 加入好友所在联盟
        let friend_alliance = civ.friends.iter().find_map(|(fid, _)| {
            self.players.get(fid).and_then(|f| f.alliance_id.clone())
                .and_then(|aid| {
                    let a = self.alliances.get(&aid)?;
                    if a.alive_member_count < m::MAX_ALLIANCE_MEMBERS { Some(aid) } else { None }
                })
        });

        if let Some(aid) = friend_alliance {
            if let Some(civ) = self.players.get_mut(civ_id) { civ.alliance_id = Some(aid.clone()); }
            let new_count = if let Some(a) = self.alliances.get_mut(&aid) {
                a.members.push(civ_id.to_string());
                let nc = a.alive_count(&self.players);
                a.alive_member_count = nc;
                nc
            } else { 0 };
            // 更新索引 (在 mutable borrow 释放后)
            let old = self.alliance_sizes.iter().find(|(_, id)| id == &aid).map(|(s, _)| *s).unwrap_or(0);
            self._update_alliance_size(&aid, old, new_count);
            return;
        }

        // BTreeSet O(log N) 找最小联盟
        let smallest = self.alliance_sizes.iter().next().cloned();
        if let Some((count, aid)) = smallest {
            if count < m::MAX_ALLIANCE_MEMBERS {
                if let Some(civ) = self.players.get_mut(civ_id) { civ.alliance_id = Some(aid.clone()); }
                if let Some(a) = self.alliances.get_mut(&aid) {
                    a.members.push(civ_id.to_string());
                    let new_count = a.alive_count(&self.players);
                    a.alive_member_count = new_count;
                    self._update_alliance_size(&aid, count, new_count);
                }
                return;
            }
        }

        // 所有联盟都满了, 创建新联盟
        let aid = format!("A_{:04}", self.alliances.len());
        if let Some(civ) = self.players.get_mut(civ_id) { civ.alliance_id = Some(aid.clone()); }
        let mut a = AllianceData::new(aid.clone(), civ_id.to_string(), self.time);
        a.alive_member_count = 1;
        self.alliances.insert(aid.clone(), a);
        self.alliance_sizes.insert((1, aid));
    }

    // ═══════════════════════════════════════════
    // 联盟外交
    // ═══════════════════════════════════════════

    fn _alliance_diplomacy_tick(&mut self) {
        let alliance_ids: Vec<String> = self.alliances.keys().cloned().collect();

        // 更新联盟领袖 (最强者)
        for aid in &alliance_ids {
            let strongest = {
                let a = match self.alliances.get(aid) { Some(a) => a, None => continue };
                let alive = a.alive_members(&self.players);
                alive.iter().max_by_key(|m| {
                    self.players.get(m.as_str()).map(|p| p.total_level()).unwrap_or(0)
                }).cloned()
            };
            if let Some(ref leader) = strongest {
                if let Some(a) = self.alliances.get_mut(aid) {
                    if a.leader != *leader {
                        a.leader = leader.clone();
                        a.cohesion = (a.cohesion - 0.05).max(0.3);
                    }
                }
            }
        }

        // 宣战: 随机攻击邻近联盟
        for aid in &alliance_ids {
            let a = match self.alliances.get(aid) { Some(a) => a.clone(), None => continue };
            if a.alive_count(&self.players) < 2 { continue; }
            if self.rng.gen::<f64>() > 0.02 { continue; } // 每天 2% 概率宣战

            // 找个目标联盟
            let targets: Vec<String> = alliance_ids.iter()
                .filter(|t| *t != aid && !a.is_at_war(t) && !a.allies.contains(t))
                .cloned().collect();
            if targets.is_empty() { continue; }

            let target = targets.choose(&mut self.rng).unwrap().clone();
            if let Some(a) = self.alliances.get_mut(aid) {
                a.war_targets.push(target.clone());
                println!("  ⚔️  WAR: {} declares war on {}!", aid, target);
            }
            if let Some(t) = self.alliances.get_mut(&target) {
                t.war_targets.push(aid.clone());
            }
        }

        // 战争结束 (一方全灭或低概率和谈)
        let mut to_remove_war: Vec<(String, String)> = Vec::new();
        for aid in &alliance_ids {
            let a = match self.alliances.get(aid) { Some(a) => a, None => continue };
            for enemy in &a.war_targets {
                let enemy_alive = self.alliances.get(enemy)
                    .map(|e| e.alive_count(&self.players) > 0)
                    .unwrap_or(false);
                if !enemy_alive {
                    to_remove_war.push((aid.clone(), enemy.clone()));
                } else if self.rng.gen::<f64>() < 0.005 {
                    // 和谈 0.5%/天
                    to_remove_war.push((aid.clone(), enemy.clone()));
                    println!("  ☮️  PEACE: {} and {} make peace", aid, enemy);
                }
            }
        }
        for (a, e) in to_remove_war {
            if let Some(alliance) = self.alliances.get_mut(&a) {
                alliance.war_targets.retain(|w| w != &e);
            }
        }

        // 合并: 小联盟有概率并入大联盟
        let mut to_merge: Vec<(String, String)> = Vec::new();
        for aid in &alliance_ids {
            let a = match self.alliances.get(aid) { Some(a) => a, None => continue };
            let alive = a.alive_count(&self.players);
            if alive > 0 && alive < 3 && self.rng.gen::<f64>() < 0.01 {
                // 找大联盟合并
                let big = alliance_ids.iter()
                    .filter(|t| *t != aid)
                    .filter_map(|t| self.alliances.get(t))
                    .max_by_key(|t| t.alive_count(&self.players))
                    .map(|t| t.id.clone());
                if let Some(big_id) = big {
                    let big_alive = self.alliances.get(&big_id)
                        .map(|b| b.alive_count(&self.players)).unwrap_or(0);
                    if big_alive + alive <= m::MAX_ALLIANCE_MEMBERS * 2 {
                        to_merge.push((aid.clone(), big_id));
                    }
                }
            }
        }
        for (small, big) in to_merge {
            self._merge_alliances(&small, &big);
        }

        // 叛逃: 凝聚力低时成员离开
        let mut deserters: Vec<(String, String)> = Vec::new();
        for aid in &alliance_ids {
            let a = match self.alliances.get(aid) { Some(a) => a, None => continue };
            let cohesion = a.cohesion;
            for member in &a.members {
                if member == &a.leader { continue; }
                if let Some(civ) = self.players.get(member) {
                    if civ.is_ruins || civ.is_quit() { continue; }
                    // 低凝聚力 + 低社交性 + 情绪化 → 叛逃
                    let betray_chance = (1.0 - cohesion) * 0.05
                        + (1.0 - civ.personality.sociability) * 0.01
                        + civ.personality.emotionality * 0.01;
                    if self.rng.gen::<f64>() < betray_chance {
                        deserters.push((aid.clone(), member.clone()));
                    }
                }
            }
        }
        for (aid, member) in deserters {
            if let Some(a) = self.alliances.get_mut(&aid) {
                a.members.retain(|m| m != &member);
                a.cohesion = (a.cohesion - 0.1).max(0.0);
            }
            if let Some(civ) = self.players.get_mut(&member) {
                civ.alliance_id = None;
                civ.leave_cooldown_until = self.time + 86400 * 3;
            }
            // 叛逃者尝试加入其他联盟
            self._add_to_alliance_by_id(&member);
        }

        // 清理空联盟
        self.alliances.retain(|_, a| a.alive_count(&self.players) > 0);

        // 重建联盟大小索引
        self.alliance_sizes.clear();
        for (aid, a) in &self.alliances {
            self.alliance_sizes.insert((a.alive_member_count, aid.clone()));
        }

        // 更新联盟统计
        for aid in &alliance_ids {
            if let Some(a) = self.alliances.get_mut(aid) {
                a.total_kills = a.members.iter()
                    .filter_map(|m| self.players.get(m))
                    .map(|p| p.total_victims).sum();
                a.total_deaths = a.members.iter()
                    .filter_map(|m| self.players.get(m))
                    .map(|p| p.total_deaths).sum();
                a.alive_member_count = a.alive_count(&self.players);
                a.cohesion = (a.cohesion + 0.005).min(1.0);
            }
        }
    }

    fn _merge_alliances(&mut self, small: &str, big: &str) {
        let small_members: Vec<String> = {
            let a = match self.alliances.get(small) { Some(a) => a, None => return };
            // 限制合并人数
            let big_alive = self.alliances.get(big).map(|b| b.alive_count(&self.players)).unwrap_or(0);
            if big_alive + a.alive_count(&self.players) > m::MAX_ALLIANCE_MEMBERS * 2 { return; }
            a.members.clone()
        };

        let war_targets: Vec<String>;
        let allies_list: Vec<String>;
        {
            let a = self.alliances.get(small).unwrap();
            war_targets = a.war_targets.clone();
            allies_list = a.allies.clone();
        }

        for m in &small_members {
            if let Some(civ) = self.players.get_mut(m) {
                civ.alliance_id = Some(big.to_string());
            }
            if let Some(b) = self.alliances.get_mut(big) {
                if !b.members.contains(m) { b.members.push(m.clone()); }
            }
        }

        // 继承战争和盟友关系
        if let Some(b) = self.alliances.get_mut(big) {
            for w in war_targets { if !b.war_targets.contains(&w) { b.war_targets.push(w); } }
            for ally in allies_list { if !b.allies.contains(&ally) { b.allies.push(ally); } }
        }

        println!("  🤝 MERGE: {} merged into {}", small, big);
        self.alliances.remove(small);
    }

    // ═══════════════════════════════════════════
    // 玩家每日行动
    // ═══════════════════════════════════════════
    // 并行攻击辅助函数 (无 &self 依赖, 纯数据驱动)
    // ═══════════════════════════════════════════

    /// 找所有候选目标 (搜一次网格, 返回排序列表)
    fn _find_all_targets(
        civ: &Civilization,
        players: &HashMap<String, Civilization>,
        grid: &HashMap<(i64, i64, i64), Vec<String>>,
        time: u128, grid_size: i64, rng: &mut impl rand::Rng,
        max_candidates: usize,
    ) -> Vec<(i64, String)> {
        let gx = civ.x as i64 / grid_size;
        let gy = civ.y as i64 / grid_size;
        let gz = civ.z as i64 / grid_size;
        let scan = civ.scan_range();
        let mut candidates: Vec<(i64, String)> = Vec::with_capacity(max_candidates);

        for dx in -1i64..=1 { for dy in -1i64..=1 { for dz in -1i64..=1 {
            let cell = match grid.get(&(gx + dx, gy + dy, gz + dz)) { Some(c) => c, None => continue };
            let iter: Box<dyn Iterator<Item = &String>> = if cell.len() > 20 {
                let sampled: Vec<&String> = cell.choose_multiple(rng, 20).collect();
                Box::new(sampled.into_iter())
            } else { Box::new(cell.iter()) };

            for pid in iter {
                let other = match players.get(pid) { Some(o) => o, None => continue };
                if other.id == civ.id || other.is_ruins || other.is_quit() { continue; }
                if other.is_newbie(time) { continue; }
                if let Some(ref aid) = civ.alliance_id {
                    if let Some(ref taid) = other.alliance_id { if aid == taid { continue; } }
                }
                // 平方距离过滤 (避免 isqrt)
                let dx = (civ.x - other.x).unsigned_abs();
                let dy = (civ.y - other.y).unsigned_abs();
                let dz = (civ.z - other.z).unsigned_abs();
                let dist_sq = dx * dx + dy * dy + dz * dz;
                let scan_sq = scan * scan;
                if dist_sq > scan_sq { continue; }

                // 只有候选者才计算真实距离 (用于评分)
                let dist = m::isqrt(dist_sq);
                let mut score = (100i64).saturating_sub(other.shield_percent() as i64) * 10
                    + (other.energy as i64) / 1000 + (other.dft as i64) / 10000
                    - (dist as i64) / 10 - (other.weapon_lv as i64) * 5;
                let es = civ.has_enemy(pid);
                if es > 0.0 { score += (es * 200.0) as i64; }
                candidates.push((score, pid.clone()));
            }
        }}}
        candidates.sort_by_key(|(s, _)| -s);
        if candidates.len() > max_candidates { candidates.truncate(max_candidates); }
        candidates
    }

    /// 在给定玩家集合中找目标 (单次, 兼容旧接口)
    fn _find_target_in(
        civ: &Civilization,
        players: &HashMap<String, Civilization>,
        grid: &HashMap<(i64, i64, i64), Vec<String>>,
        time: u128, grid_size: i64, rng: &mut impl rand::Rng,
        preferred: Option<&String>, threshold: u128,
    ) -> Option<String> {
        let gx = civ.x as i64 / grid_size;
        let gy = civ.y as i64 / grid_size;
        let gz = civ.z as i64 / grid_size;
        let scan = civ.scan_range();
        let mut candidates: Vec<(i64, String)> = Vec::new();

        // 战争目标
        let war_targets: Vec<String> = civ.alliance_id.as_ref()
            .and_then(|aid| players.get(aid)) // 这里不太对，我们需要联盟的 war_targets
            .map(|_| Vec::new())
            .unwrap_or_default();
        // 简化为: 不传 war_targets (影响不大)

        for dx in -1i64..=1 { for dy in -1i64..=1 { for dz in -1i64..=1 {
            let cell = match grid.get(&(gx + dx, gy + dy, gz + dz)) { Some(c) => c, None => continue };
            let iter: Box<dyn Iterator<Item = &String>> = if cell.len() > 20 {
                let sampled: Vec<&String> = cell.choose_multiple(rng, 20).collect();
                Box::new(sampled.into_iter())
            } else { Box::new(cell.iter()) };

            for pid in iter {
                let other = match players.get(pid) { Some(o) => o, None => continue };
                if other.id == civ.id || other.is_ruins || other.is_quit() { continue; }
                if other.is_newbie(time) { continue; }
                if let Some(ref aid) = civ.alliance_id {
                    if let Some(ref taid) = other.alliance_id { if aid == taid { continue; } }
                }
                if other.shield_percent() > threshold { continue; }
                let dist = m::distance(civ.x, civ.y, civ.z, other.x, other.y, other.z);
                if dist > scan { continue; }

                let mut score = 100i64.saturating_sub(other.shield_percent() as i64) * 10
                    + (other.energy as i64) / 1000 + (other.dft as i64) / 10000
                    - (dist as i64) / 10 - (other.weapon_lv as i64) * 5;
                let es = civ.has_enemy(pid);
                if es > 0.0 { score += (es * 200.0) as i64; }
                if let Some(pref) = preferred { if *pid == *pref { score += 500; } }
                candidates.push((score, pid.clone()));
            }
        }}}
        if candidates.is_empty() { return None; }
        candidates.sort_by_key(|(s, _)| -s);
        Some(candidates[0].1.clone())
    }

    /// 联盟防御加成 (并行安全版)
    fn _alliance_bonus_from(
        target: &Civilization,
        alliances: &HashMap<String, AllianceData>,
        players: &HashMap<String, Civilization>,
    ) -> u128 {
        let aid = match target.alliance_id { Some(ref a) => a, None => return 0 };
        let a = match alliances.get(aid) { Some(a) => a, None => return 0 };
        a.alive_count(players).saturating_sub(1) as u128 * m::ALLIANCE_DEF_BONUS
    }

    // 攻击阶段 (串行, 需要 HashMap 找目标)
    fn _player_attack_tick(&mut self, civ: &mut Civilization) {
        civ.update_reputation();

        civ.attack_count_today = 0;
        let attack_budget = civ.max_attacks_per_day();
        let threshold = civ.attack_shield_threshold();
        let focus = civ.focus_fire();
        let mut preferred: Option<String> = None;

        for _ in 0..attack_budget {
            let tid = self._find_target(civ, preferred.as_ref(), threshold);
            let tid = match tid { Some(id) => id, None => break };
            let mut target = match self.players.remove(&tid) { Some(t) => t, None => continue };
            let bonus = self._alliance_defense_bonus(&target);

            // 战争加成: 如果与目标联盟处于战争状态, 攻击加成 +50%
            let at_war = civ.alliance_id.as_ref().and_then(|aid| {
                self.alliances.get(aid).map(|a| a.is_at_war(&target.alliance_id.clone().unwrap_or_default()))
            }).unwrap_or(false);
            // 我们在 math_engine 中没有战争加成, 这里额外补偿一下
            // 直接给攻击者加能量(实际是降低能量消耗)
            if at_war {
                if civ.energy > 200 { civ.energy -= 100; } // 战争狂热: 自损100能量也要打
            }

            let result = civ.attack_target(&mut target, self.time, bonus);
            let target_destroyed = target.is_ruins;
            let success = matches!(result, AttackOutcome::Success(_));
            if success {
                civ.attack_count_today += 1;
                if focus { preferred = Some(tid.clone()); }
                // 战争击杀记录
                if let Some(ref aid) = civ.alliance_id {
                    if let Some(a) = self.alliances.get_mut(aid) {
                        a.total_kills += 1;
                    }
                }
                if let Some(ref taid) = target.alliance_id {
                    if let Some(a) = self.alliances.get_mut(taid) {
                        a.total_deaths += 1;
                    }
                }
            }
            self.players.insert(tid.clone(), target);
            if target_destroyed {
                self._remove_active(&tid);
            }
            if !success && !focus { break; }
        }
    }

    // ═══════════════════════════════════════════
    // 目标寻找 (考虑战争+仇人链)
    // ═══════════════════════════════════════════

    fn _find_target(
        &mut self, civ: &Civilization, preferred: Option<&String>, threshold: u128,
    ) -> Option<String> {
        self._rebuild_grid();
        let gx = civ.x as i64 / self.grid_size;
        let gy = civ.y as i64 / self.grid_size;
        let gz = civ.z as i64 / self.grid_size;
        let scan = civ.scan_range();
        let mut candidates: Vec<(i64, String)> = Vec::new();

        // 战争目标优先
        let war_targets: Vec<String> = civ.alliance_id.as_ref()
            .and_then(|aid| self.alliances.get(aid))
            .map(|a| a.war_targets.clone())
            .unwrap_or_default();

        for dx in -1i64..=1 { for dy in -1i64..=1 { for dz in -1i64..=1 {
            let cell = match self.spatial_grid.get(&(gx + dx, gy + dy, gz + dz)) { Some(c) => c, None => continue };
            let iter: Box<dyn Iterator<Item = &String>> = if cell.len() > 20 {
                Box::new(cell.choose_multiple(&mut self.rng, 20).into_iter())
            } else { Box::new(cell.iter()) };

            for pid in iter {
                let other = match self.players.get(pid) { Some(o) => o, None => continue };
                if other.id == civ.id || other.is_ruins || other.is_quit() { continue; }
                if other.is_newbie(self.time) { continue; }
                if let Some(ref aid) = civ.alliance_id {
                    if let Some(ref taid) = other.alliance_id {
                        if aid == taid { continue; }
                    }
                }
                if other.shield_percent() > threshold { continue; }
                let dist = m::distance(civ.x, civ.y, civ.z, other.x, other.y, other.z);
                if dist > scan { continue; }

                let shield_s = 100i64.saturating_sub(other.shield_percent() as i64) * 10;
                let energy_s = (other.energy as i64) / 1000;
                let dft_s = (other.dft as i64) / 10000;
                let dist_p = -(dist as i64) / 10;
                let wep_p = -(other.weapon_lv as i64) * 5;
                let mut score = shield_s + energy_s + dft_s + dist_p + wep_p;

                // 仇人分
                let enemy_s = civ.has_enemy(pid);
                if enemy_s > 0.0 { score += (enemy_s * 200.0) as i64; }

                // 战争分
                if let Some(ref taid) = other.alliance_id {
                    if war_targets.contains(taid) { score += 300; }
                }

                // 集火
                if let Some(pref) = preferred { if *pid == *pref { score += 500; } }

                candidates.push((score, pid.clone()));
            }
        }}}

        if candidates.is_empty() { return None; }
        candidates.sort_by_key(|(s, _)| -s);
        Some(candidates[0].1.clone())
    }

    // ═══════════════════════════════════════════
    // 空间网格
    // ═══════════════════════════════════════════

    fn _rebuild_grid(&mut self) {
        if !self.grid_dirty { return; }
        self.grid_dirty = false;
        self.spatial_grid.clear();
        for pid in &self.active_ids {
            let civ = match self.players.get(pid) { Some(c) => c, None => continue };
            let gx = civ.x as i64 / self.grid_size;
            let gy = civ.y as i64 / self.grid_size;
            let gz = civ.z as i64 / self.grid_size;
            self.spatial_grid.entry((gx, gy, gz)).or_insert_with(Vec::new).push(pid.clone());
        }
    }

    // ═══════════════════════════════════════════
    // 联盟防御加成
    // ═══════════════════════════════════════════

    fn _alliance_defense_bonus(&self, target: &Civilization) -> u128 {
        let aid = match target.alliance_id { Some(ref a) => a, None => return 0 };
        let a = match self.alliances.get(aid) { Some(a) => a, None => return 0 };
        a.alive_count(&self.players).saturating_sub(1) as u128 * m::ALLIANCE_DEF_BONUS
    }

    // ═══════════════════════════════════════════
    // 市场
    // ═══════════════════════════════════════════

    fn _market_tick(&mut self) {
        let mut sell: Vec<(String, u128)> = Vec::new();
        let mut buy: Vec<(String, u128)> = Vec::new();
        for (pid, civ) in &self.players {
            if civ.is_ruins || civ.is_quit() { continue; }
            let sp = civ.sell_energy_pct();
            let ma = civ.max_attacks_per_day();
            if sp > 0.0 && civ.energy > 5000 {
                let ts = ((civ.energy - 5000) as f64 * sp) as u128;
                if ts > 100 { sell.push((pid.clone(), ts)); }
            } else if ma > 12 {
                let en = (ma as u128) * m::calc_attack_energy_cost(civ.weapon_lv);
                let ed = en.saturating_sub(civ.energy);
                if ed > 100 && civ.dft > 1000 { buy.push((pid.clone(), ed)); }
            }
        }
        sell.shuffle(&mut self.rng); buy.shuffle(&mut self.rng);
        let mut tv: u128 = 0; let mut eb: u128 = 0;
        self.market_daily_trades = 0;
        let mut sc: HashMap<String, u128> = HashMap::new();
        for (bid, need) in buy.iter().take(10) {
            let mut rem = *need;
            for (sid, amt) in &sell {
                let avail = amt.saturating_sub(*sc.get(sid).unwrap_or(&0));
                if avail < 100 || rem < 100 { continue; }
                let trade = std::cmp::min(rem, std::cmp::min(avail, 5000));
                if trade < 100 { continue; }
                let dft_cost = trade * self.market_rate / 100;
                if self.players.get(bid).map_or(0, |c| c.dft) < dft_cost { continue; }
                if self.players.get(sid).map_or(0, |c| c.energy) < trade { continue; }
                if let Some(b) = self.players.get_mut(bid) { b.dft = b.dft.saturating_sub(dft_cost); }
                if let Some(s) = self.players.get_mut(sid) { s.dft += dft_cost; }
                let burn = trade * 500 / 10000;
                if let Some(s) = self.players.get_mut(sid) { s.energy = s.energy.saturating_sub(trade); }
                if let Some(b) = self.players.get_mut(bid) { b.energy += trade.saturating_sub(burn); }
                eb += burn; self.market_dft_fees += dft_cost * 100 / 10000;
                tv += trade; self.market_daily_trades += 1;
                *sc.entry(sid.clone()).or_insert(0) += trade;
                rem = rem.saturating_sub(trade);
                if rem < 100 { break; }
            }
        }
        if tv > 0 {
            let tbd: u128 = buy.iter().take(10).map(|(_, n)| n).sum();
            let dp = if tbd > 0 { (tv as f64) / (tbd as f64) } else { 0.5 };
            self.market_rate = ((self.market_rate as f64 * (1.0 + (dp - 0.5) * 0.1)) as u128).clamp(10, 10000);
        }
        self.market_daily_volume = tv;
        self.total_energy_burned += eb;
    }

    // ═══════════════════════════════════════════
    // 指标收集
    // ═══════════════════════════════════════════

    pub fn _collect_metrics(&self) -> SimMetrics {
        let total = self.players.len();
        let active: Vec<&Civilization> = self.active_players().collect();
        let ac = active.len();
        let ruins = total.saturating_sub(ac);

        if active.is_empty() {
            return SimMetrics {
                day: self.day, total_spawned: total, active_players: 0, ruins,
                total_dft_minted: self.total_dft_minted, total_dft_burned: self.total_dft_burned,
                total_attacks: self.players.values().map(|p| p.total_attacks).sum(),
                total_deaths: self.players.values().map(|p| p.total_deaths).sum(),
                total_rebuilds: self.total_rebuilds, total_invites: self.total_invites,
                total_quits: self.total_quits, active_alliances: 0, wars_active: 0,
                avg_level: 0.0, gini_energy: 0.0, gini_dft: 0.0,
            };
        }

        let avg_lv = active.iter().map(|p| p.total_level() as f64).sum::<f64>() / ac as f64 / 5.0;

        let mut en: Vec<u128> = active.iter().map(|p| p.energy).collect(); en.sort();
        let ge = gini(&en);
        let mut dfts: Vec<u128> = active.iter().map(|p| p.dft).collect(); dfts.sort();
        let gd = gini(&dfts);

        let active_alliances = self.alliances.values()
            .filter(|a| a.alive_count(&self.players) >= 2).count();
        let wars_active = self.alliances.values()
            .filter(|a| !a.war_targets.is_empty()).count();

        SimMetrics {
            day: self.day, total_spawned: total, active_players: ac, ruins,
            total_dft_minted: self.total_dft_minted, total_dft_burned: self.total_dft_burned,
            total_attacks: self.players.values().map(|p| p.total_attacks).sum(),
            total_deaths: self.players.values().map(|p| p.total_deaths).sum(),
            total_rebuilds: self.total_rebuilds, total_invites: self.total_invites,
            total_quits: self.total_quits, active_alliances, wars_active,
            avg_level: avg_lv, gini_energy: ge, gini_dft: gd,
        }
    }
}

// ═══════════════════════════════════════════
// 辅助
// ═══════════════════════════════════════════

fn weighted_choice(rng: &mut impl Rng, weights: &[f64]) -> usize {
    let total: f64 = weights.iter().sum();
    let mut r: f64 = rng.gen_range(0.0..total);
    for (i, w) in weights.iter().enumerate() {
        r -= w; if r <= 0.0 { return i; }
    }
    weights.len() - 1
}

fn gini(values: &[u128]) -> f64 {
    let n = values.len();
    if n < 2 { return 0.0; }
    let total: u128 = values.iter().sum();
    if total == 0 { return 0.0; }
    let tf = total as f64; let nf = n as f64;
    let mut cum: u128 = 0; let mut gs: f64 = 0.0;
    for (i, v) in values.iter().enumerate() {
        cum += v; gs += (i + 1) as f64 * tf / nf - cum as f64;
    }
    (gs / (nf * tf / 2.0)).clamp(0.0, 1.0)
}
