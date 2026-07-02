//! 战斗撮合引擎 — SoA 版

#![allow(dead_code)]

use crate::math_engine as m;
use crate::store::EntityStore;

#[derive(Debug, Clone)]
pub struct AttackOrder {
    pub attacker_idx: u32,
    pub target_idx: u32,
    pub weapon_lv: u8,
    pub energy_cost: u128,
    pub has_tokens: bool,
    pub time: u64,
    pub alliance_bonus: u128,
    pub attacker_alliance: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub attacker_idx: u32,
    pub target_idx: u32,
    pub success: bool,
    pub reason: &'static str,
}

pub struct BattleEngine {
    orders: Vec<AttackOrder>,
}

impl BattleEngine {
    pub fn new() -> Self { Self { orders: Vec::new() } }
    pub fn submit(&mut self, order: AttackOrder) { self.orders.push(order); }
    pub fn submit_batch(&mut self, orders: Vec<AttackOrder>) { self.orders.extend(orders); }
    pub fn order_count(&self) -> usize { self.orders.len() }

    pub fn execute(&mut self, store: &mut EntityStore, destroyed: &mut Vec<u32>) -> Vec<MatchResult> {
        if self.orders.is_empty() { return Vec::new(); }

        // 按目标分组
        let mut by_target: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
        for (idx, order) in self.orders.iter().enumerate() {
            by_target.entry(order.target_idx).or_default().push(idx);
        }

        let mut results: Vec<MatchResult> = Vec::with_capacity(self.orders.len());
        results.resize_with(self.orders.len(), || MatchResult {
            attacker_idx: 0, target_idx: 0, success: false, reason: "unprocessed",
        });

        for (&tidx, order_indices) in &by_target {
            let t = tidx as usize;

            // 目标已死?
            if store.is_ruins[t] == 1 {
                for &idx in order_indices {
                    let o = &self.orders[idx];
                    results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "target_gone" };
                }
                continue;
            }

            // 按顺序执行所有攻击
            for &idx in order_indices {
                let o = &self.orders[idx];
                let a = o.attacker_idx as usize;

                // 攻击者还在?
                if store.is_ruins[a] == 1 {
                    results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "attacker_gone" };
                    continue;
                }
                // 目标已被之前的攻击摧毁?
                if store.is_ruins[t] == 1 {
                    results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "target_already_dead" };
                    continue;
                }
                // 新玩家保护
                if store.creation_time[t] + m::NEWBIE_PROTECTION as u64 > o.time {
                    results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "newbie" };
                    continue;
                }
                // 同联盟
                if let Some(aaid) = o.attacker_alliance {
                    if let Some(taid) = store.alliance_idx[t] {
                        if aaid == taid {
                            results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "same_alliance" };
                            continue;
                        }
                    }
                }
                // 能量
                if store.energy[a] < o.energy_cost {
                    results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: false, reason: "low_energy" };
                    continue;
                }

                // --- 执行 ---
                store.energy[a] -= o.energy_cost;
                if o.has_tokens && store.attack_tokens[a] > 0 { store.attack_tokens[a] -= 1; }
                store.total_attacks[a] += 1;

                // 战斗计算
                let result = m::simulate_attack(
                    o.weapon_lv as u128, store.shield_lv[t] as u128,
                    store.energy[t], store.health[t], store.shield_hp[t], o.alliance_bonus,
                );

                store.shield_hp[t] = result.remaining_shield;
                store.health[t] = result.remaining_health;

                if result.stolen_energy > 0 {
                    let steal = std::cmp::min(result.stolen_energy, store.energy[t]);
                    store.energy[t] -= steal;
                    store.energy[a] += steal;
                }

                // DFT 掠夺
                let dft_plunder = store.dft[t] * 2000 / 10000;
                if dft_plunder > 0 {
                    let actual = std::cmp::min(dft_plunder, store.dft[t]);
                    store.dft[t] -= actual;
                    store.dft[a] += actual;
                    store.cold.set_dft_earned(o.attacker_idx, store.cold.dft_earned(o.attacker_idx) + actual);
                }

                if result.defender_destroyed {
                    store.is_ruins[t] = 1;
                    store.alliance_idx[t] = None;
                    store.total_victims[a] += 1;
                    store.total_deaths[t] += 1;
                    destroyed.push(tidx);
                }

                if store.shield_durability[t] > 0 { store.shield_durability[t] -= 1; }

                // 情绪
                store.cold.set_elation(o.attacker_idx, (store.cold.elation(o.attacker_idx) as u16 + if result.defender_destroyed { 15 } else { 5 }).min(100) as u8);
                store.cold.set_anger(o.attacker_idx, (store.cold.anger(o.attacker_idx) as i16 - 5).max(0) as u8);
                store.cold.set_consecutive_wins(o.attacker_idx, store.cold.consecutive_wins(o.attacker_idx) + 1);
                store.cold.set_consecutive_losses(o.attacker_idx, 0);

                store.cold.set_anger(o.target_idx, (store.cold.anger(o.target_idx) as u16 + 25).min(100) as u8);
                let fear_add = (15.0 * (1.0 - (store.cold.boldness(o.target_idx) as f64 / 100.0))) as u16;
                store.cold.set_fear(o.target_idx, (store.cold.fear(o.target_idx) as u16 + fear_add).min(100) as u8);

                // 仇人
                store.add_enemy(tidx, o.attacker_idx, 50);
                store.add_enemy(o.attacker_idx, tidx, 20);
                // 仇人链
                let attacker_enemies: Vec<(u32, u8)> = store.enemies[a].clone();
                for (eid, _) in &attacker_enemies {
                    if *eid != tidx { store.add_enemy(tidx, *eid, 15); }
                }

                results[idx] = MatchResult { attacker_idx: o.attacker_idx, target_idx: o.target_idx, success: true, reason: "ok" };
            }
        }

        self.orders.clear();
        results
    }
}
