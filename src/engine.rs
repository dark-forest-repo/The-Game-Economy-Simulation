// Dark Forest 引擎 — SoA 版: self.players → self.store
#![allow(dead_code)]

use std::collections::{HashMap, BTreeSet};
use rand::Rng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::math_engine as m;
use crate::player::{self, PersonalityPreset, SPAWN_DISTRIBUTION};
use crate::store::EntityStore;
use crate::battle_engine::{BattleEngine, AttackOrder};

// ═══════════════════════════════════════════════════════════
// 配置
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub simulation_days: u64, pub seed: u64,
    pub dft_daily_emission: u128,
    pub spawn_in_cluster: bool, pub cluster_radius: u128, pub random_spawn_pct: f64,
    pub initial_players: usize, pub daily_spawn: usize, pub daily_spawn_variance: f64,
    pub invite_enabled: bool, pub diplomacy_enabled: bool, pub rebuild_enabled: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            simulation_days: 3650, seed: 42,
            dft_daily_emission: m::DAILY_DFT_EMISSION,
            spawn_in_cluster: true, cluster_radius: 3000, random_spawn_pct: 0.05,
            initial_players: 200, daily_spawn: 20, daily_spawn_variance: 0.5,
            invite_enabled: true, diplomacy_enabled: true, rebuild_enabled: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════
// 联盟
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AllianceData {
    pub id: u32,
    pub name: String,
    pub leader: u32,
    pub members: Vec<u32>,
    pub created_at: u64,
    pub cohesion: f64,
    pub alive_member_count: usize,
    pub war_targets: Vec<u32>,
    pub allies: Vec<u32>,
    pub total_kills: u32, pub total_deaths: u32,
}

impl AllianceData {
    pub fn new(id: u32, name: String, leader: u32, created_at: u64) -> Self {
        Self {
            id, name, leader, members: vec![leader],
            created_at, cohesion: 0.8, alive_member_count: 1,
            war_targets: Vec::new(), allies: Vec::new(),
            total_kills: 0, total_deaths: 0,
        }
    }

    pub fn alive_count(&self, store: &EntityStore) -> usize {
        self.members.iter().filter(|&&m| store.is_active(m)).count()
    }
}

// ═══════════════════════════════════════════════════════════
// 指标
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SimMetrics {
    pub day: u64, pub total_spawned: u32,
    pub active_players: usize, pub ruins: usize,
    pub total_dft_minted: u128, pub total_dft_burned: u128,
    pub total_attacks: u32, pub total_deaths: u32,
    pub total_rebuilds: u32, pub total_invites: u32,
    pub total_quits: u32, pub active_alliances: usize,
    pub wars_active: usize,
    pub avg_level: f64, pub gini_energy: f64, pub gini_dft: f64,
}

// ═══════════════════════════════════════════════════════════
// 引擎
// ═══════════════════════════════════════════════════════════

pub struct GameEngine {
    pub cfg: SimConfig,
    pub rng: rand::rngs::StdRng,
    pub store: EntityStore,
    pub time: u64,
    pub day: u64,
    pub generation: u32,
    pub next_alliance_id: u32,
    initialized: bool,

    pub alliances: HashMap<u32, AllianceData>,
    pub alliance_sizes: BTreeSet<(usize, u32)>,
    pub alliance_name_map: HashMap<String, u32>,
    pub alliance_enabled: bool,

    spatial_grid: HashMap<(i64, i64, i64), Vec<u32>>,
    grid_size: i64,
    grid_dirty: bool,

    pub total_dft_minted: u128, pub total_dft_burned: u128,
    pub total_rebuilds: u32, pub total_invites: u32, pub total_quits: u32,

    pub metrics_history: Vec<SimMetrics>,
    pub global_state: m::GlobalState,

    pub market_rate: u128, pub market_dft_fees: u128,
    pub market_daily_volume: u128, pub market_daily_trades: usize,

    battle_engine: BattleEngine,
}

impl GameEngine {
    pub fn new(config: SimConfig) -> Self {
        let seed = config.seed;
        Self {
            cfg: config, rng: rand::rngs::StdRng::seed_from_u64(seed),
            store: EntityStore::new(),
            time: 0, day: 0, generation: 0, next_alliance_id: 0, initialized: false,
            alliances: HashMap::new(), alliance_sizes: BTreeSet::new(),
            alliance_name_map: HashMap::new(), alliance_enabled: true,
            spatial_grid: HashMap::new(), grid_size: 10000, grid_dirty: true,
            total_dft_minted: 0, total_dft_burned: 0,
            total_rebuilds: 0, total_invites: 0, total_quits: 0,
            metrics_history: Vec::new(),
            global_state: m::GlobalState::new(),
            market_rate: 100, market_dft_fees: 0,
            market_daily_volume: 0, market_daily_trades: 0,
            battle_engine: BattleEngine::new(),
        }
    }

    pub fn active_count(&self) -> usize { self.store.active_indices().count() }

    // ═══════════════════════════════════════════
    // 主循环
    // ═══════════════════════════════════════════

    pub(crate) fn _daily_step(&mut self) {
        self.global_state.update(self.total_dft_minted, self.total_dft_burned);

        // 0. 初始玩家
        if !self.initialized { self._spawn_initial(); self.initialized = true; }

        // 1. 每日注入
        self._spawn_daily();

        // 2. 邀请
        if self.cfg.invite_enabled { self._process_invitations(); }

        // 3. DFT 分红
        if self.global_state.can_mint() {
            let active: Vec<u32> = self.store.active_indices().collect();
            let n = active.len();
            if n > 0 {
                let actual = std::cmp::min(self.cfg.dft_daily_emission, m::TOTAL_SUPPLY.saturating_sub(self.global_state.total_minted));
                let per = actual / n as u128;
                if per > 0 {
                    self.total_dft_minted += per * n as u128;
                    for &idx in &active { self.store.dft[idx as usize] += per; }
                }
            }
        }

        // ═════ Phase B: 并行采集/升级/情绪 (Rayon) ═════
        let active_b: Vec<u32> = self.store.active_indices().collect();
        {
            use rayon::prelude::*;
            use std::sync::Mutex;
            let time = self.time;
            let is_ps = self.global_state.is_post_scarcity();
            let burned_agg = Mutex::new(0u128);
            let quits_agg = Mutex::new(0u32);

            let store_ptr: usize = &mut self.store as *mut EntityStore as usize;
            let grid_dirty_ptr: usize = &mut self.grid_dirty as *mut bool as usize;

            active_b.par_iter().for_each(|&idx| {
                let store = unsafe { &mut *(store_ptr as *mut EntityStore) };
                let i = idx as usize;

                if store.is_ruins[i] == 1 { return; }
                let quit = player::update_emotion_daily(store, idx, time);
                if quit {
                    store.is_ruins[i] = 1;
                    *quits_agg.lock().unwrap() += 1;
                    unsafe { *(grid_dirty_ptr as *mut bool) = true; }
                    return;
                }

                // 采集
                let elapsed = time.saturating_sub(store.last_collect_time[i]);
                if elapsed > 0 && store.collector_durability[i] > 0 {
                    let rate = player::collect_rate(store, idx);
                    if rate > 0 {
                        let ct = std::cmp::min(elapsed as u128, store.collector_durability[i] as u128);
                        store.energy[i] += ct * rate;
                        store.collector_durability[i] -= ct as u64;
                    }
                }
                store.last_collect_time[i] = time;

                // Token
                let interval = m::calc_token_regen_interval(store.weapon_lv[i] as u128);
                let interval_s = (interval / 100) as u64;
                if interval_s > 0 {
                    let elapsed_t = time.saturating_sub(store.last_token_time[i]);
                    if elapsed_t >= interval_s {
                        let regened = (elapsed_t / interval_s) as u8;
                        let max_t = m::calc_max_tokens(store.weapon_lv[i] as u128) as u8;
                        store.attack_tokens[i] = std::cmp::min(store.attack_tokens[i] as u16 + regened as u16, max_t as u16) as u8;
                        store.last_token_time[i] = time;
                    }
                }

                // 护盾再生
                if store.shield_hp[i] > 0 {
                    let max_hp = m::calc_shield_hp(store.shield_lv[i] as u128);
                    if store.shield_hp[i] < max_hp {
                        let regen = m::calc_shield_regen(store.shield_lv[i] as u128) * 12;
                        let cost = regen * m::SHIELD_REGEN_RATIO;
                        if store.energy[i] >= cost {
                            store.energy[i] -= cost;
                            store.shield_hp[i] = std::cmp::min(store.shield_hp[i] + regen, max_hp);
                        }
                    }
                }

                // 升级 (跳过无资源玩家)
                let has_resources = store.energy[i] > 500 || store.dft[i] > 0;
                if has_resources {
                    let plan = player::plan_upgrades(store, idx);
                    let max_up = if store.cold.burnout(idx) > 80 { 3 }
                        else if store.cold.burnout(idx) > 50 { 8 }
                        else if store.cold.tilt_level(idx) > 50 { 30 } else { 20 };
                    // 先算能升几级, 不盲目试
                    let affordable = player::count_affordable_upgrades(store, idx, is_ps, &plan).min(max_up);
                    for _ in 0..affordable {
                        let mut ok = false;
                        for &sys in &plan {
                            let (s, b) = player::try_upgrade(store, idx, sys, is_ps);
                            if s { *burned_agg.lock().unwrap() += b; ok = true; break; }
                        }
                        if !ok { break; }
                    }
                }
            });

            self.total_dft_burned += *burned_agg.lock().unwrap();
            self.total_quits += *quits_agg.lock().unwrap();
        }

        // ═════ Hybrid: 精英/群众分类 ═════
        {
            // 战斗型人格→精英, 采集型→群众
            let attacker_types = ["hunter", "whale", "general", "berserker", "scavenger", "nomad"];
            let active: Vec<u32> = self.store.active_indices().collect();

            // 先重置所有分类, 再按规则标记精英
            for &idx in &active {
                let ptype = self.store.personality_types[idx as usize];
                let is_attacker = attacker_types.contains(&ptype);
                // 战斗型自动精英, 或近期有过攻击的
                let has_attacked_recently = self.store.cold.days_since_attack(idx) < 7 && self.store.total_attacks[idx as usize] > 0;
                self.store.cold.set_is_elite(idx, is_attacker || has_attacked_recently);
            }

            // 限制精英数量 (最多 10000)
            let mut elite_count = active.iter().filter(|&&idx| self.store.cold.is_elite(idx)).count();
            if elite_count > 10000 {
                // 超额时, 按总等级排序, 只保留前 10000
                let mut sorted: Vec<u32> = active.iter().filter(|&&idx| self.store.cold.is_elite(idx)).copied().collect();
                sorted.sort_by_key(|&idx| self.store.total_level(idx));
                // 降级多余的
                for &idx in sorted.iter().take(elite_count - 10000) {
                    self.store.cold.set_is_elite(idx, false);
                }
            }
        }

        // ═════ Phase C: 并行收集攻击订单 (只针对精英) ═════
        let mut attack_orders: Vec<AttackOrder>;
        {
            use rayon::prelude::*;
            use std::sync::Mutex;
            let time = self.time;
            let grid_size = self.grid_size;

            // 提取所有需要的引用为原始指针 (绕过 borrow checker)
            let store_ptr: usize = &mut self.store as *mut EntityStore as usize;
            let alli_ptr: usize = &self.alliances as *const HashMap<u32, AllianceData> as usize;
            let grid_ptr: usize = &self.spatial_grid as *const HashMap<(i64, i64, i64), Vec<u32>> as usize;

            // 找出有攻击意愿的精英玩家
            let attackers: Vec<u32> = active_b.iter()
                .filter(|&&idx| {
                    let s = unsafe { &*(store_ptr as *const EntityStore) };
                    s.is_ruins[idx as usize] == 0 && s.cold.is_elite(idx) && player::derive_max_attacks(s, idx) > 0
                })
                .copied().collect();

            let orders_mutex: Mutex<Vec<AttackOrder>> = Mutex::new(Vec::new());

            attackers.par_iter().for_each(|&idx| {
                let store = unsafe { &mut *(store_ptr as *mut EntityStore) };
                let grid = unsafe { &*(grid_ptr as *const HashMap<(i64, i64, i64), Vec<u32>>) };
                let alliances = unsafe { &*(alli_ptr as *const HashMap<u32, AllianceData>) };

                let i = idx as usize;
                if store.is_ruins[i] == 1 { return; }
                let budget = player::derive_max_attacks(store, idx);
                let threshold = player::derive_attack_shield_threshold(store, idx);
                let focus = player::derive_focus_fire(store, idx);

                // 搜一次网格 (只读)
                let candidates = Self::_find_targets_in(store, grid, idx, grid_size, time);
                let mut submitted = std::collections::HashSet::new();
                let mut player_orders = 0usize;
                let mut my_orders: Vec<AttackOrder> = Vec::new();

                for &(_, tidx) in &candidates {
                    if player_orders >= budget { break; }
                    let t = tidx as usize;
                    if store.shield_percent(tidx) > threshold { continue; }
                    let energy_cost = m::calc_attack_energy_cost(store.weapon_lv[i] as u128);
                    if store.energy[i] < energy_cost * (player_orders as u128 + 1) { break; }

                    let bonus = store.alliance_idx[t].and_then(|aid| {
                        alliances.get(&aid).map(|a| a.alive_member_count.saturating_sub(1) as u128 * m::ALLIANCE_DEF_BONUS)
                    }).unwrap_or(0);

                    let num = if focus {
                        let atk = m::calc_attack(store.weapon_lv[i] as u128);
                        let def = m::calc_defense(store.shield_lv[t] as u128) + bonus;
                        let dmg = if atk > def { (atk - def) * 2 + (atk + m::SHIELD_DMG_BONUS - def).max(0) } else { 0 };
                        if dmg > 0 { ((store.health[t] + store.shield_hp[t] + dmg - 1) / dmg).min(10) } else { 1 }
                    } else { 1 };

                    if submitted.contains(&tidx) && !focus { continue; }
                    for _ in 0..num {
                        if player_orders >= budget { break; }
                        my_orders.push(AttackOrder {
                            attacker_idx: idx, target_idx: tidx,
                            weapon_lv: store.weapon_lv[i], energy_cost,
                            has_tokens: store.attack_tokens[i] > 0,
                            time, alliance_bonus: bonus,
                            attacker_alliance: store.alliance_idx[i],
                        });
                        player_orders += 1;
                    }
                    submitted.insert(tidx);
                    if focus { break; }
                }
                orders_mutex.lock().unwrap().extend(my_orders);
            });

            attack_orders = orders_mutex.into_inner().unwrap();
        }

        // ═════ Phase D: 战斗引擎 ═════
        let mut destroyed = Vec::new();
        let mut engine = BattleEngine::new();
        engine.submit_batch(attack_orders);
        engine.execute(&mut self.store, &mut destroyed);

        // 网格脏标记
        if !destroyed.is_empty() { self.grid_dirty = true; }

        // 4. 联盟外交
        if self.cfg.diplomacy_enabled { self._diplomacy_tick(); }

        // 5. 市场
        self._market_tick();

        // 6. 重建
        if self.cfg.rebuild_enabled { self._rebuild_tick(); }

        // 7. 网格重建
        if self.grid_dirty { self._rebuild_grid(); }
    }

    // ═══════════════════════════════════════════
    // 寻找目标 (网格一次)
    // ═══════════════════════════════════════════

    // 网格搜索 (免 &self, 引用传入)
    fn _find_targets_in(
        store: &EntityStore,
        grid: &HashMap<(i64, i64, i64), Vec<u32>>,
        idx: u32, grid_size: i64, time: u64,
    ) -> Vec<(i64, u32)> {
        let i = idx as usize;
        let gx = store.x[i] as i64 / grid_size;
        let gy = store.y[i] as i64 / grid_size;
        let gz = store.z[i] as i64 / grid_size;
        let scan = store.scan_range[i] as u128;  // 预计算!
        let scan_sq = scan * scan;
        let mut candidates = Vec::with_capacity(64);
        let mut thread_rng = rand::thread_rng(); // 每线程独立 RNG

        for dx in -1i64..=1 { for dy in -1i64..=1 { for dz in -1i64..=1 {
            let cell = match grid.get(&(gx + dx, gy + dy, gz + dz)) { Some(c) => c, None => continue };
            let iter: Vec<&u32> = if cell.len() > 20 {
                cell.choose_multiple(&mut thread_rng, 20).collect() // 恢复随机抽样
            } else { cell.iter().collect() };

            for &&tidx in &iter {
                let t = tidx as usize;
                if tidx == idx || store.is_ruins[t] == 1 { continue; }
                if store.is_newbie_until[t] > time { continue; }
                if let Some(aaid) = store.alliance_idx[i] {
                    if let Some(taid) = store.alliance_idx[t] { if aaid == taid { continue; } }
                }
                // 平方距离过滤 (免 isqrt)
                let dx = (store.x[i] - store.x[t]).unsigned_abs();
                let dy = (store.y[i] - store.y[t]).unsigned_abs();
                let dz = (store.z[i] - store.z[t]).unsigned_abs();
                if dx * dx + dy * dy + dz * dz > scan_sq { continue; }

                let dist = m::isqrt(dx * dx + dy * dy + dz * dz);
                let sp = store.shield_percent(tidx);
                let mut score = (100i64 - sp as i64) * 10
                    + (store.energy[t] as i64) / 1000 + (store.dft[t] as i64) / 10000
                    - (dist as i64) / 10 - (store.weapon_lv[t] as i64) * 5;
                let es = store.has_enemy(idx, tidx);
                if es > 0 { score += (es as i64) * 2; }
                candidates.push((score, tidx));
            }
        }}}

        candidates.sort_by_key(|(s, _)| -s);
        candidates.truncate(100);
        candidates
    }

    // 旧版: 用 &mut self (只用在外交等需要 Rng 的地方)
    fn _find_targets(&mut self, idx: u32) -> Vec<(i64, u32)> {
        Self::_find_targets_in(&self.store, &self.spatial_grid, idx, self.grid_size, self.time)
    }

    // ═══════════════════════════════════════════
    // 玩家生成
    // ═══════════════════════════════════════════

    fn _spawn_initial(&mut self) {
        for _ in 0..self.cfg.initial_players {
            self._create_player(None);
        }
        println!("  🌱 初始 {} 名玩家加入", self.cfg.initial_players);
    }

    fn _spawn_daily(&mut self) {
        let base = self.cfg.daily_spawn as f64;
        if base <= 0.0 { return; }
        let count = (base * (1.0 + self.rng.gen_range(-self.cfg.daily_spawn_variance..=self.cfg.daily_spawn_variance))).round() as usize;
        let count = count.max(1);
        self.generation += 1;
        for _ in 0..count { self._create_player(None); }
    }

    fn _create_player(&mut self, inviter: Option<u32>) -> u32 {
        let idx = self.store.alloc();
        let i = idx as usize;

        let types: Vec<&str> = SPAWN_DISTRIBUTION.iter().map(|(n, _)| *n).collect();
        let weights: Vec<f64> = SPAWN_DISTRIBUTION.iter().map(|(_, w)| *w).collect();
        let stype = weighted_choice(&mut self.rng, &weights);
        let preset = player::make_preset(types[stype]);
        let personality = preset.jitter(&mut self.rng);

        let addr = player::random_evm_address(&mut self.rng);
        let pid = format!("g{}p{:06}", self.generation, self.store.next_id);
        let name = format!("{}_{}", preset.name, self.store.next_id);

        self.store.ids[i] = pid.clone();
        self.store.addresses[i] = addr;
        self.store.names[i] = name;
        self.store.personality_types[i] = preset.name;
        self.store.cold.set_aggression(idx, personality[0]);
        self.store.cold.set_greed(idx, personality[1]);
        self.store.cold.set_boldness(idx, personality[2]);
        self.store.cold.set_sociability(idx, personality[3]);
        self.store.cold.set_emotionality(idx, personality[4]);
        self.store.cold.set_invites_remaining(idx, 255); // 无限邀请 (高配置)
        self.store.cold.set_invited_by(idx, inviter);
        self.store.creation_time[i] = self.time.saturating_sub(86401);
        self.store.last_collect_time[i] = self.time;
        self.store.last_token_time[i] = self.time;
        self.store.cold.set_generation(idx, self.generation);
        self.store.is_newbie_until[i] = self.time + m::NEWBIE_PROTECTION as u64;

        // 坐标
        if let Some(inv) = inviter {
            let ix = self.store.x[inv as usize];
            self.store.x[i] = ix + self.rng.gen_range(-2000i128..=2000);
            self.store.y[i] = self.rng.gen_range(-2000i128..=2000);
            self.store.z[i] = self.rng.gen_range(-2000i128..=2000);
        } else if self.cfg.spawn_in_cluster {
            let r: f64 = self.rng.gen_range(0.0..=1.0f64) * self.cfg.cluster_radius as f64;
            let theta: f64 = self.rng.gen_range(0.0..std::f64::consts::TAU);
            let phi: f64 = self.rng.gen_range(0.0..std::f64::consts::PI);
            self.store.x[i] = (r * phi.sin() * theta.cos()) as i128;
            self.store.y[i] = (r * phi.sin() * theta.sin()) as i128;
            self.store.z[i] = (r * phi.cos()) as i128;
        } else {
            let range = 1i128 << 40;
            self.store.x[i] = self.rng.gen_range(-range..=range);
            self.store.y[i] = self.rng.gen_range(-range..=range);
            self.store.z[i] = self.rng.gen_range(-range..=range);
        }

        self.store.id_map.insert(pid, idx);
        self.grid_dirty = true;

        // 联盟
        if player::derive_prefer_alliance(&self.store, idx) {
            self._join_alliance(idx);
        }

        idx
    }

    // ═══════════════════════════════════════════
    // 联盟
    // ═══════════════════════════════════════════

    fn _join_alliance(&mut self, idx: u32) {
        // 好友优先
        let friend_aid = self.store.enemies[idx as usize].iter().find_map(|(fid, _)| {
            let a = self.store.alliance_idx[*fid as usize]?;
            let al = self.alliances.get(&a)?;
            if al.alive_member_count < m::MAX_ALLIANCE_MEMBERS as usize { Some(a) } else { None }
        });

        if let Some(aid) = friend_aid {
            self.store.alliance_idx[idx as usize] = Some(aid);
            if let Some(a) = self.alliances.get_mut(&aid) {
                a.members.push(idx);
                a.alive_member_count = a.alive_count(&self.store);
            }
            return;
        }

        // BTreeSet 找最小
        if let Some(&(cnt, aid)) = self.alliance_sizes.iter().next() {
            if cnt < m::MAX_ALLIANCE_MEMBERS as usize {
                self.store.alliance_idx[idx as usize] = Some(aid);
                if let Some(a) = self.alliances.get_mut(&aid) {
                    a.members.push(idx);
                    let nc = a.alive_count(&self.store);
                    self.alliance_sizes.remove(&(cnt, aid));
                    self.alliance_sizes.insert((nc, aid));
                    a.alive_member_count = nc;
                }
                return;
            }
        }

        // 新联盟
        let aid = self.next_alliance_id;
        self.next_alliance_id += 1;
        let aname = format!("A_{:04}", aid);
        self.store.alliance_idx[idx as usize] = Some(aid);
        let mut al = AllianceData::new(aid, aname.clone(), idx, self.time);
        al.alive_member_count = 1;
        self.alliances.insert(aid, al);
        self.alliance_sizes.insert((1, aid));
        self.alliance_name_map.insert(aname, aid);
    }

    // ═══════════════════════════════════════════
    // 邀请
    // ═══════════════════════════════════════════

    fn _process_invitations(&mut self) {
        let active: Vec<u32> = self.store.active_indices().collect();
        if active.is_empty() { return; }

        let density = (active.len() as f64 / 100_000_000.0).min(1.0);
        let density_factor = (1.0 - density).max(0.1);

        let mut inviters = Vec::new();
        for &idx in &active {
            let i = idx as usize;
            // if self.store.cold.invites_remaining(idx) == 0 { continue; } // 无限邀请, 跳过检查
            if self.store.is_newbie_until[i] > self.time { continue; }

            let is_invited = self.store.cold.invited_by(idx).is_some();
            // 高邀请配置: 用于测试千万级玩家
            let base_prob = if is_invited { 0.15 } else { 0.10 };
            let s = self.store.cold.sociability(idx) as f64 / 100.0;
            let lv = self.store.total_level(idx) as f64 / 100.0;
            let prob = (base_prob
                + s * if is_invited { 0.15 } else { 0.10 }
                + lv * if is_invited { 0.08 } else { 0.05 }) * density_factor;

            if self.rng.gen::<f64>() < prob {
                inviters.push((idx, self.store.x[i]));
            }
        }

        for (inviter_idx, inviter_x) in inviters {
            let ii = inviter_idx as usize;
            // if self.store.cold.invites_remaining(inviter_idx) == 0 { continue; } // 无限
            // 不减了
            self.store.cold.set_referral_count(inviter_idx, self.store.cold.referral_count(inviter_idx) + 1);

            // 奖励邀请者
            self.store.dft[ii] += m::REFERRAL_REWARD * 10u128.pow(18);
            self.store.energy[ii] += 1000;

            let invitee_idx = self._create_player(Some(inviter_idx));
            let ie = invitee_idx as usize;
            self.store.dft[ie] += 200 * 10u128.pow(18);
            self.total_invites += 1;

            // 加入邀请者联盟
            if let Some(aid) = self.store.alliance_idx[ii] {
                self.store.alliance_idx[ie] = Some(aid);
                if let Some(a) = self.alliances.get_mut(&aid) {
                    a.members.push(invitee_idx);
                    a.alive_member_count = a.alive_count(&self.store);
                }
            }
        }
    }

    // ═══════════════════════════════════════════
    // 联盟外交
    // ═══════════════════════════════════════════

    fn _diplomacy_tick(&mut self) {
        let aids: Vec<u32> = self.alliances.keys().copied().collect();

        // 更新领袖和活跃数
        for &aid in &aids {
            let strongest = {
                let a = match self.alliances.get(&aid) { Some(a) => a, None => continue };
                let alive = a.members.iter().filter(|&&m| self.store.is_active(m));
                alive.max_by_key(|&&m| self.store.total_level(m)).copied()
            };
            if let Some(leader) = strongest {
                if let Some(a) = self.alliances.get_mut(&aid) {
                    if a.leader != leader {
                        a.leader = leader;
                        a.cohesion = (a.cohesion - 0.05).max(0.3);
                    }
                }
            }
        }

        // 宣战
        for &aid in &aids {
            let a = match self.alliances.get(&aid) { Some(a) => a.clone(), None => continue };
            if a.alive_member_count < 2 { continue; }
            if self.rng.gen::<f64>() > 0.02 { continue; }

            let targets: Vec<u32> = aids.iter()
                .filter(|&&t| t != aid && !a.war_targets.contains(&t) && !a.allies.contains(&t))
                .copied().collect();
            if let Some(&target) = targets.choose(&mut self.rng) {
                if let Some(a) = self.alliances.get_mut(&aid) { a.war_targets.push(target); }
                if let Some(t) = self.alliances.get_mut(&target) { t.war_targets.push(aid); }
            }
        }

        // 和谈
        let mut to_peace = Vec::new();
        for &aid in &aids {
            let a = match self.alliances.get(&aid) { Some(a) => a, None => continue };
            for &enemy in &a.war_targets {
                if self.alliances.get(&enemy).map(|e| e.alive_member_count == 0).unwrap_or(true) || self.rng.gen::<f64>() < 0.005 {
                    to_peace.push((aid, enemy));
                }
            }
        }
        for (a, e) in to_peace {
            if let Some(al) = self.alliances.get_mut(&a) { al.war_targets.retain(|&w| w != e); }
            if let Some(al) = self.alliances.get_mut(&e) { al.war_targets.retain(|&w| w != a); }
        }

        // 合并
        let mut to_merge = Vec::new();
        for &aid in &aids {
            let a = match self.alliances.get(&aid) { Some(a) => a, None => continue };
            if a.alive_member_count > 0 && a.alive_member_count < 3 && self.rng.gen::<f64>() < 0.01 {
                if let Some(big) = aids.iter().filter(|&&t| t != aid)
                    .filter_map(|t| self.alliances.get(&t))
                    .max_by_key(|t| t.alive_member_count)
                    .map(|t| t.id)
                {
                    let ba = self.alliances.get(&big).map(|b| b.alive_member_count).unwrap_or(0);
                    if ba + a.alive_member_count <= m::MAX_ALLIANCE_MEMBERS * 2 {
                        to_merge.push((aid, big));
                    }
                }
            }
        }
        for (small, big) in to_merge {
            self._merge_alliances(small, big);
        }

        // 叛逃
        let mut deserters = Vec::new();
        for &aid in &aids {
            let a = match self.alliances.get(&aid) { Some(a) => a, None => continue };
            let coh = a.cohesion;
            for &m in &a.members {
                if m == a.leader { continue; }
                if !self.store.is_active(m) { continue; }
                let s = self.store.cold.sociability(m) as f64 / 100.0;
                let e = self.store.cold.emotionality(m) as f64 / 100.0;
                let chance = (1.0 - coh) * 0.05 + (1.0 - s) * 0.01 + e * 0.01;
                if self.rng.gen::<f64>() < chance { deserters.push((aid, m)); }
            }
        }
        for (aid, m) in deserters {
            if let Some(a) = self.alliances.get_mut(&aid) {
                a.members.retain(|&x| x != m);
                a.cohesion = (a.cohesion - 0.1).max(0.0);
            }
            self.store.alliance_idx[m as usize] = None;
            self._join_alliance(m);
        }

        // 清理 + 重建索引
        self.alliances.retain(|_, a| a.alive_member_count > 0);
        self.alliance_sizes.clear();
        for (&aid, a) in &self.alliances {
            self.alliance_sizes.insert((a.alive_member_count, aid));
        }

        // 更新统计
        for (&aid, a) in &mut self.alliances {
            a.total_kills = a.members.iter().filter_map(|&m| {
                if self.store.is_active(m) { Some(self.store.total_victims[m as usize]) } else { None }
            }).sum();
            a.total_deaths = a.members.iter().filter_map(|&m| {
                if self.store.is_active(m) { Some(self.store.total_deaths[m as usize]) } else { None }
            }).sum();
            a.alive_member_count = a.alive_count(&self.store);
            a.cohesion = (a.cohesion + 0.005).min(1.0);
        }
    }

    fn _merge_alliances(&mut self, small: u32, big: u32) {
        let members: Vec<u32> = {
            let a = match self.alliances.get(&small) { Some(a) => a, None => return };
            let ba = self.alliances.get(&big).map(|b| b.alive_member_count).unwrap_or(0);
            if ba + a.alive_member_count > m::MAX_ALLIANCE_MEMBERS * 2 { return; }
            a.members.clone()
        };
        let wars: Vec<u32>;
        let allies: Vec<u32>;
        {
            let a = self.alliances.get(&small).unwrap();
            wars = a.war_targets.clone(); allies = a.allies.clone();
        }
        for &m in &members {
            self.store.alliance_idx[m as usize] = Some(big);
            if let Some(b) = self.alliances.get_mut(&big) {
                if !b.members.contains(&m) { b.members.push(m); }
            }
        }
        if let Some(b) = self.alliances.get_mut(&big) {
            for w in wars { if !b.war_targets.contains(&w) { b.war_targets.push(w); } }
            for al in allies { if !b.allies.contains(&al) { b.allies.push(al); } }
        }
        self.alliances.remove(&small);
    }

    // ═══════════════════════════════════════════
    // 市场
    // ═══════════════════════════════════════════

    fn _market_tick(&mut self) {
        let mut sell: Vec<(u32, u128)> = Vec::new();
        let mut buy: Vec<(u32, u128)> = Vec::new();
        for idx in self.store.active_indices() {
            let i = idx as usize;
            let sp = player::derive_sell_energy_pct(&self.store, idx);
            let ma = player::derive_max_attacks(&self.store, idx);
            if sp > 0.0 && self.store.energy[i] > 5000 {
                let ts = ((self.store.energy[i] - 5000) as f64 * sp) as u128;
                if ts > 100 { sell.push((idx, ts)); }
            } else if ma > 12 {
                let en = (ma as u128) * m::calc_attack_energy_cost(self.store.weapon_lv[i] as u128);
                let ed = en.saturating_sub(self.store.energy[i]);
                if ed > 100 && self.store.dft[i] > 1000 { buy.push((idx, ed)); }
            }
        }
        sell.shuffle(&mut self.rng); buy.shuffle(&mut self.rng);
        let mut tv: u128 = 0; let mut eb: u128 = 0;
        self.market_daily_trades = 0;
        let mut sc: HashMap<u32, u128> = HashMap::new();
        for (bid, need) in buy.iter().take(10) {
            let mut rem = *need;
            for (sid, amt) in &sell {
                let avail = amt.saturating_sub(*sc.get(sid).unwrap_or(&0));
                if avail < 100 || rem < 100 { continue; }
                let trade = std::cmp::min(rem, std::cmp::min(avail, 5000));
                if trade < 100 { continue; }
                let dft_cost = trade * self.market_rate / 100;
                if self.store.dft[*bid as usize] < dft_cost { continue; }
                if self.store.energy[*sid as usize] < trade { continue; }
                self.store.dft[*bid as usize] = self.store.dft[*bid as usize].saturating_sub(dft_cost);
                self.store.dft[*sid as usize] += dft_cost;
                let burn = trade * 500 / 10000;
                self.store.energy[*sid as usize] = self.store.energy[*sid as usize].saturating_sub(trade);
                self.store.energy[*bid as usize] += trade.saturating_sub(burn);
                eb += burn; self.market_dft_fees += dft_cost * 100 / 10000;
                tv += trade; self.market_daily_trades += 1;
                *sc.entry(*sid).or_insert(0) += trade;
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
    }

    // ═══════════════════════════════════════════
    // 网格
    // ═══════════════════════════════════════════

    fn _rebuild_grid(&mut self) {
        if !self.grid_dirty { return; }
        self.grid_dirty = false;
        self.spatial_grid.clear();
        for idx in self.store.active_indices() {
            let i = idx as usize;
            let gx = self.store.x[i] as i64 / self.grid_size;
            let gy = self.store.y[i] as i64 / self.grid_size;
            let gz = self.store.z[i] as i64 / self.grid_size;
            self.spatial_grid.entry((gx, gy, gz)).or_insert_with(Vec::new).push(idx);
        }
    }

    // ═══════════════════════════════════════════
    // 重建
    // ═══════════════════════════════════════════

    fn _rebuild_tick(&mut self) {
        let ruins: Vec<u32> = (0..self.store.ids.len() as u32)
            .filter(|&idx| self.store.is_ruins[idx as usize] == 1 && self.store.total_deaths[idx as usize] > 0)
            .collect();
        let limit = std::cmp::min(5, ruins.len());
        let mut count = 0;
        for &idx in &ruins {
            if count >= limit { break; }
            if player::try_rebuild(&mut self.store, idx, self.global_state.is_post_scarcity()) {
                self.total_rebuilds += 1;
                self._join_alliance(idx);
                self.grid_dirty = true;
                count += 1;
            }
        }
    }

    // ═══════════════════════════════════════════
    // 指标
    // ═══════════════════════════════════════════

    pub fn _collect_metrics(&self) -> SimMetrics {
        let active: Vec<u32> = self.store.active_indices().collect();
        let total = self.store.ids.len();

        if active.is_empty() {
            return SimMetrics {
                day: self.day, total_spawned: total as u32, active_players: 0, ruins: total,
                total_dft_minted: self.total_dft_minted, total_dft_burned: self.total_dft_burned,
                total_attacks: self.store.total_attacks.iter().sum(),
                total_deaths: self.store.total_deaths.iter().sum(),
                total_rebuilds: self.total_rebuilds, total_invites: self.total_invites,
                total_quits: self.total_quits, active_alliances: 0, wars_active: 0,
                avg_level: 0.0, gini_energy: 0.0, gini_dft: 0.0,
            };
        }

        let avg_lv = active.iter().map(|&idx| self.store.total_level(idx) as f64).sum::<f64>() / active.len() as f64 / 5.0;

        let mut en: Vec<u128> = active.iter().map(|&idx| self.store.energy[idx as usize]).collect(); en.sort();
        let ge = gini(&en);
        let mut dfts: Vec<u128> = active.iter().map(|&idx| self.store.dft[idx as usize]).collect(); dfts.sort();
        let gd = gini(&dfts);

        let aac = self.alliances.values().filter(|a| a.alive_member_count >= 2).count();
        let wars = self.alliances.values().filter(|a| !a.war_targets.is_empty()).count();

        SimMetrics {
            day: self.day, total_spawned: total as u32, active_players: active.len(), ruins: total - active.len(),
            total_dft_minted: self.total_dft_minted, total_dft_burned: self.total_dft_burned,
            total_attacks: self.store.total_attacks.iter().sum(),
            total_deaths: self.store.total_deaths.iter().sum(),
            total_rebuilds: self.total_rebuilds, total_invites: self.total_invites,
            total_quits: self.total_quits, active_alliances: aac, wars_active: wars,
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
    for (i, w) in weights.iter().enumerate() { r -= w; if r <= 0.0 { return i; } }
    weights.len() - 1
}

fn gini(values: &[u128]) -> f64 {
    let n = values.len();
    if n < 2 { return 0.0; }
    let total: u128 = values.iter().sum();
    if total == 0 { return 0.0; }
    let tf = total as f64; let nf = n as f64;
    let mut cum: u128 = 0; let mut gs: f64 = 0.0;
    for (i, v) in values.iter().enumerate() { cum += v; gs += (i + 1) as f64 * tf / nf - cum as f64; }
    (gs / (nf * tf / 2.0)).clamp(0.0, 1.0)
}
