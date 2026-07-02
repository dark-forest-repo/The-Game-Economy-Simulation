//! 战斗撮合引擎 — 像交易所撮合一样处理攻击订单
//!
//! 设计:
//!   Phase 1: 玩家提交 AttackOrder (可并行生成)
//!   Phase 2: 引擎按目标分组撮合 (每目标只一次 HashMap 操作)
//!   对比原来的每攻击一次 remove/insert, 按目标分组后每目标只一次

#![allow(dead_code)]

use std::collections::HashMap;
use crate::math_engine as m;
use crate::player::{Civilization, AttackOutcome};

// ═══════════════════════════════════════════════════════════
// 攻击订单
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AttackOrder {
    pub attacker_id: String,
    pub target_id: String,
    pub attacker_weapon_lv: u128,
    pub attacker_energy: u128,       // 攻击前能量 (用于检查)
    pub energy_cost: u128,
    pub has_tokens: bool,
    pub time: u128,
    pub alliance_bonus: u128,
    pub attacker_alliance: Option<String>,
}

// ═══════════════════════════════════════════════════════════
// 撮合结果
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub attacker_id: String,
    pub target_id: String,
    pub success: bool,
    pub reason: &'static str,
    pub stolen_energy: u128,
    pub stolen_dft: u128,
    pub target_destroyed: bool,
    pub emotion_anger: f64,       // attacker anger change
    pub emotion_elation: f64,     // attacker elation change
    pub target_anger: f64,
    pub target_fear: f64,
}

// ═══════════════════════════════════════════════════════════
// 战斗引擎
// ═══════════════════════════════════════════════════════════

pub struct BattleEngine {
    orders: Vec<AttackOrder>,
}

impl BattleEngine {
    pub fn new() -> Self {
        Self { orders: Vec::new() }
    }

    /// 提交一个攻击订单
    pub fn submit(&mut self, order: AttackOrder) {
        self.orders.push(order);
    }

    /// 提交一批订单
    pub fn submit_batch(&mut self, orders: Vec<AttackOrder>) {
        self.orders.extend(orders);
    }

    /// 撮合所有订单: 按目标分组, 批量执行
    /// 返回每个订单的执行结果
    pub fn execute(&mut self, players: &mut HashMap<String, Civilization>) -> Vec<MatchResult> {
        if self.orders.is_empty() { return Vec::new(); }

        // 按目标分组
        let mut by_target: HashMap<&str, Vec<usize>> = HashMap::new();
        for (idx, order) in self.orders.iter().enumerate() {
            by_target.entry(order.target_id.as_str()).or_default().push(idx);
        }

        let mut results: Vec<MatchResult> = Vec::with_capacity(self.orders.len());
        results.resize_with(self.orders.len(), || MatchResult {
            attacker_id: String::new(), target_id: String::new(),
            success: false, reason: "unprocessed",
            stolen_energy: 0, stolen_dft: 0, target_destroyed: false,
            emotion_anger: 0.0, emotion_elation: 0.0,
            target_anger: 0.0, target_fear: 0.0,
        });

        // 按目标处理 (不同目标可并行, 这里用迭代器顺序处理)
        for (_target_str, order_indices) in &by_target {
            // 取出目标 (一次 HashMap 操作)
            let target_id = &self.orders[order_indices[0]].target_id;
            let mut target = match players.remove(target_id.as_str()) {
                Some(t) => t,
                None => {
                    // 目标不存在或已死, 标记所有攻击为失败
                    for &idx in order_indices {
                        let order = &self.orders[idx];
                        results[idx] = MatchResult {
                            attacker_id: order.attacker_id.clone(),
                            target_id: order.target_id.clone(),
                            success: false, reason: "target_gone",
                            ..Default::default()
                        };
                    }
                    continue;
                }
            };

            // 所有攻击者对这个目标连续攻击
            for &idx in order_indices {
                let order = &self.orders[idx];
                let attacker_id = &order.attacker_id;

                // 检查目标是否已被之前的攻击摧毁
                if target.is_ruins {
                    results[idx] = MatchResult {
                        attacker_id: attacker_id.clone(),
                        target_id: target_id.clone(),
                        success: false, reason: "target_already_destroyed",
                        ..Default::default()
                    };
                    continue;
                }

                // 获取攻击者
                let attacker = match players.get_mut(attacker_id) {
                    Some(a) => a,
                    None => {
                        results[idx] = MatchResult {
                            attacker_id: attacker_id.clone(),
                            target_id: target_id.clone(),
                            success: false, reason: "attacker_gone",
                            ..Default::default()
                        };
                        continue;
                    }
                };

                // 校验: 新玩家保护
                if target.is_newbie(order.time) {
                    results[idx] = MatchResult {
                        attacker_id: attacker_id.clone(),
                        target_id: target_id.clone(),
                        success: false, reason: "newbie",
                        ..Default::default()
                    };
                    continue;
                }

                // 同联盟
                if let Some(ref aaid) = order.attacker_alliance {
                    if let Some(ref taid) = target.alliance_id {
                        if aaid == taid {
                            results[idx] = MatchResult {
                                attacker_id: attacker_id.clone(), target_id: target_id.clone(),
                                success: false, reason: "same_alliance",
                                ..Default::default()
                            };
                            continue;
                        }
                    }
                }

                // 能量检查
                if attacker.energy < order.energy_cost {
                    results[idx] = MatchResult {
                        attacker_id: attacker_id.clone(), target_id: target_id.clone(),
                        success: false, reason: "low_energy",
                        ..Default::default()
                    };
                    continue;
                }

                // Token 检查
                if !order.has_tokens && attacker.attack_tokens <= 0 {
                    results[idx] = MatchResult {
                        attacker_id: attacker_id.clone(), target_id: target_id.clone(),
                        success: false, reason: "rate_limited",
                        ..Default::default()
                    };
                    continue;
                }

                // --- 执行攻击 (扣除攻击者资源) ---
                attacker.energy -= order.energy_cost;
                if order.has_tokens && attacker.attack_tokens > 0 {
                    attacker.attack_tokens -= 1;
                }
                attacker.total_attacks += 1;

                // --- 战斗计算 (使用目标当前状态) ---
                let result = m::simulate_attack(
                    order.attacker_weapon_lv, target.shield_lv, target.energy,
                    target.health, target.shield_hp, order.alliance_bonus,
                );

                // --- 应用到目标 ---
                target.shield_hp = result.remaining_shield;
                target.health = result.remaining_health;

                let mut stolen_energy = 0u128;
                let mut stolen_dft = 0u128;
                let mut target_destroyed = false;

                if result.stolen_energy > 0 {
                    stolen_energy = std::cmp::min(result.stolen_energy, target.energy);
                    target.energy -= stolen_energy;
                    attacker.energy += stolen_energy;
                    attacker.total_plundered += stolen_energy;
                }

                // DFT 掠夺
                let dft_plunder = target.dft * 2000 / 10000;
                if dft_plunder > 0 {
                    stolen_dft = std::cmp::min(dft_plunder, target.dft);
                    target.dft -= stolen_dft;
                    attacker.dft += stolen_dft;
                    attacker.total_dft_earned += stolen_dft;
                }

                if result.defender_destroyed {
                    target.is_ruins = true;
                    target.alliance_id = None;
                    attacker.total_victims += 1;
                    target.total_deaths += 1;
                    target_destroyed = true;
                }

                if target.shield_durability > 0 {
                    target.shield_durability -= 1;
                }

                // 情绪更新
                let emotion_elation = if target_destroyed { 0.15 } else { 0.05 };
                attacker.emotion.elation = (attacker.emotion.elation + emotion_elation).min(1.0);
                attacker.emotion.anger = (attacker.emotion.anger - 0.05).max(0.0);
                attacker.consecutive_wins += 1;
                attacker.consecutive_losses = 0;

                let target_anger = 0.25;
                let target_fear = 0.15 * (1.0 - target.personality.boldness);
                target.emotion.anger = (target.emotion.anger + target_anger).min(1.0);
                target.emotion.fear = (target.emotion.fear + target_fear).min(1.0);

                // 社交: 互相标记仇人
                target.add_enemy(attacker_id.clone(), 0.5);
                attacker.add_enemy(target_id.clone(), 0.2);
                for (eid, _) in &attacker.enemies.clone() {
                    if eid != &target.id {
                        target.add_enemy(eid.clone(), 0.15);
                    }
                }

                results[idx] = MatchResult {
                    attacker_id: attacker_id.clone(),
                    target_id: target_id.clone(),
                    success: true, reason: "ok",
                    stolen_energy, stolen_dft, target_destroyed,
                    emotion_anger: -0.05, emotion_elation,
                    target_anger, target_fear,
                };
            }

            // 目标放回 (再一次 HashMap 操作)
            players.insert(target_id.clone(), target);
        }

        self.orders.clear();
        results
    }

    pub fn order_count(&self) -> usize {
        self.orders.len()
    }
}

impl Default for MatchResult {
    fn default() -> Self {
        Self {
            attacker_id: String::new(), target_id: String::new(),
            success: false, reason: "",
            stolen_energy: 0, stolen_dft: 0, target_destroyed: false,
            emotion_anger: 0.0, emotion_elation: 0.0,
            target_anger: 0.0, target_fear: 0.0,
        }
    }
}
