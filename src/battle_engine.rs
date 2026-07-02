//! 战斗撮合引擎 — 聚合模式: 按 (weapon_lv, shield_lv) 分组统计
//! 减少 10K 次模拟 → 1 个公式

#![allow(dead_code)]

use std::collections::HashMap;
use rand::seq::SliceRandom;
use crate::math_engine as m;
use crate::store::EntityStore;

#[derive(Debug, Clone)]
pub struct AttackOrder {
    pub attacker_idx: u32, pub target_idx: u32,
    pub weapon_lv: u8, pub energy_cost: u128, pub has_tokens: bool,
    pub time: u64, pub alliance_bonus: u128, pub attacker_alliance: Option<u32>,
}

pub struct BattleEngine {
    orders: Vec<AttackOrder>,
}

impl BattleEngine {
    pub fn new() -> Self { Self { orders: Vec::new() } }
    pub fn submit(&mut self, order: AttackOrder) { self.orders.push(order); }
    pub fn submit_batch(&mut self, orders: Vec<AttackOrder>) { self.orders.extend(orders); }
    pub fn order_count(&self) -> usize { self.orders.len() }

    /// 聚合执行: 按 (weapon_lv, shield_lv) 分组 + 统计公式
    pub fn execute(&mut self, store: &mut EntityStore, destroyed: &mut Vec<u32>) {
        if self.orders.is_empty() { return; }

        // Phase 1: 分组 — (weapon_lv, target_shield_lv) → 攻击者和目标ID列表
        struct AttackGroup {
            attacker_indices: Vec<u32>,
            target_hps: Vec<u128>,   // 目标的 health + shield_hp
            weapon_lv: u8,
            shield_lv: u8,
            avg_attack: u128,        // 预计算
            avg_defense: u128,
        }

        use std::collections::HashMap;
        let mut groups: HashMap<(u8, u8), AttackGroup> = HashMap::new();

        for order in &self.orders {
            let t = order.target_idx as usize;
            let key = (order.weapon_lv, store.shield_lv[t]);
            let group = groups.entry(key).or_insert_with(|| AttackGroup {
                attacker_indices: Vec::new(), target_hps: Vec::new(),
                weapon_lv: key.0, shield_lv: key.1,
                avg_attack: m::calc_attack(key.0 as u128) + m::SHIELD_DMG_BONUS,
                avg_defense: m::calc_defense(key.1 as u128),
            });
            group.attacker_indices.push(order.attacker_idx);
            group.target_hps.push(store.health[t] + store.shield_hp[t]);
        }

        // Phase 2: 每组用公式
        for (_, group) in groups.iter() {
            let n_attackers = group.attacker_indices.len() as u128;
            if n_attackers == 0 { continue; }

            // 每次攻击净伤害 = avg_attack - avg_defense (shield portion) + (avg_attack - avg_defense) * 2 (health portion)
            let net = if group.avg_attack > group.avg_defense {
                (group.avg_attack - group.avg_defense) // shield dmg
                + (group.avg_attack - group.avg_defense) * 2 // health dmg
            } else { 0 };

            if net == 0 { continue; }

            // 总伤害 = n_attackers * net
            let total_damage = n_attackers * net;

            // 目标总 HP
            let total_hp: u128 = group.target_hps.iter().sum();
            if total_hp == 0 { continue; }

            // 杀敌数 = total_damage / avg_hp_per_target
            let n_targets = group.target_hps.len() as u128;
            let avg_hp = total_hp / n_targets;
            let kills = (total_damage / avg_hp).min(n_targets);

            // 随机选 k 个目标标记为摧毁
            if kills > 0 {
                let mut rng = rand::thread_rng();
                let all_targets: Vec<u32> = self.orders.iter()
                    .filter(|o| o.weapon_lv == group.weapon_lv)
                    .map(|o| o.target_idx).collect();
                let mut targets: std::collections::HashSet<u32> = std::collections::HashSet::new();
                for _ in 0..kills.min(all_targets.len() as u128) {
                    if let Some(&t) = all_targets.choose(&mut rng) {
                        targets.insert(t);
                    }
                }

                // 应用摧毁
                for &tidx in &targets {
                    let t = tidx as usize;
                    if store.is_ruins[t] == 1 { continue; }
                    store.is_ruins[t] = 1;
                    destroyed.push(tidx);
                }
            }
        }

        // Phase 3: 逐个执行需要的 (energy/tokens/emotion)
        // 只对有能量的攻击者扣 energy 和 token
        for order in &self.orders {
            let a = order.attacker_idx as usize;
            if store.is_ruins[a] == 0 && store.energy[a] >= order.energy_cost {
                store.energy[a] -= order.energy_cost;
                if order.has_tokens && store.attack_tokens[a] > 0 { store.attack_tokens[a] -= 1; }
                store.total_attacks[a] += 1;
                // 情绪微调 (批量)
                store.cold.set_elation(order.attacker_idx,
                    (store.cold.elation(order.attacker_idx) as u16 + 2).min(100) as u8);
                store.cold.set_consecutive_wins(order.attacker_idx, store.cold.consecutive_wins(order.attacker_idx) + 1);
                store.cold.set_consecutive_losses(order.attacker_idx, 0);
            }
        }

        self.orders.clear();
    }
}
