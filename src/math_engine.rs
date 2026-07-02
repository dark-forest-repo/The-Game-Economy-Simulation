// Dark Forest 合约数学 — 精确复制 Solidity 逻辑，供模拟器使用。
#![allow(dead_code)]
//!
//! 与 `math_engine.py` 保持 1:1 对应。

/// 整数平方根 — 快速路径 for u64, Newton for larger
pub fn isqrt(x: u128) -> u128 {
    if x == 0 {
        return 0;
    }
    // 64-bit 以内用 f64 (快 10x)
    if x <= u64::MAX as u128 {
        return (x as f64).sqrt() as u128;
    }
    // Newton 迭代 for > 64-bit
    let mut z = (x + 1) / 2;
    let mut y = x;
    while z < y {
        y = z;
        z = (x / z + z) / 2;
    }
    y
}

// ═══════════════════════════════════════════════════════════
// 游戏常量
// ═══════════════════════════════════════════════════════════

pub const INITIAL_ENERGY: u128 = 2000;
pub const INITIAL_HEALTH: u128 = 3000;
pub const INITIAL_SCAN_RANGE: u128 = 1000;
pub const MAX_HEALTH: u128 = 20000;

// DFT 发行参数
pub const DAILY_DFT_EMISSION: u128 = 400_000 * 10u128.pow(18); // 40 万 DFT/天
pub const TOTAL_SUPPLY: u128 = 400_000 * 3650 * 10u128.pow(18); // 14.6 亿 DFT (10年总量)
pub const REFERRAL_REWARD: u128 = 150;
pub const NEWBIE_PROTECTION: u128 = 24 * 3600;

pub const ENTRY_FEE_MIN: u128 = 10u128.pow(16); // 0.01 * 10^18
pub const ENTRY_FEE_MAX: u128 = 5 * 10u128.pow(16); // 0.05 * 10^18

// 升级成本 (已对齐至 DFT 单位: 1 DFT = 10^18 wei)
// cost(N) = A * N * (N + B) / 100  wei
pub const UP_A_COLLECTOR: u128 = 4 * 10u128.pow(19);
pub const UP_A_WEAPON: u128 = 8 * 10u128.pow(19);
pub const UP_A_SHIELD: u128 = 6 * 10u128.pow(19);
pub const UP_A_RADAR: u128 = 64 * 10u128.pow(18);
pub const UP_A_ENGINE: u128 = 48 * 10u128.pow(18);

pub const UP_B_COLLECTOR: u128 = 8;
pub const UP_B_WEAPON: u128 = 12;
pub const UP_B_SHIELD: u128 = 9;
pub const UP_B_RADAR: u128 = 11;
pub const UP_B_ENGINE: u128 = 8;

// 能量升级消耗: 固定公式
pub const ENERGY_UP_BASE: u128 = 200;

// 引擎速度
pub const ENGINE_SPEED_BASE: u128 = 10;
pub const ENGINE_SPEED_PER_LV: u128 = 5;

// 雷达: Range(N) = 1000 + 150N + 5N²
pub const RADAR_BASE: u128 = 1000;
pub const RADAR_LINEAR: u128 = 150;
pub const RADAR_QUAD: u128 = 5;

// 战斗数值 (N² 增长)
pub const ATK_BASE: u128 = 900;
pub const ATK_RATE: u128 = 10;
pub const DEF_BASE: u128 = 540;
pub const DEF_RATE: u128 = 6;
pub const SHIELD_HP_BASE: u128 = 3600;
pub const SHIELD_HP_RATE: u128 = 15;
pub const REGEN_BASE: u128 = 50;
pub const REGEN_RATE: u128 = 1;
pub const SHIELD_DMG_BONUS: u128 = 200;

// 攻击消耗
pub const ATTACK_ENERGY_BASE: u128 = 1000;
pub const ATTACK_ENERGY_PER_LV: u128 = 2000;
pub const PLUNDER_RATIO: u128 = 1500; // 15%
pub const DESTRUCTION_RATE: u128 = 4000;
pub const DOWNGRADE_DIVISOR: u128 = 10;
pub const SHIELD_REGEN_RATIO: u128 = 1;

// 能量采集
pub const BASE_COLLECT: u128 = 3;
pub const COLLECT_BONUS: u128 = 10;
pub const DURABILITY_BASE: u128 = 86400; // 1 day
pub const DURABILITY_PER_LV: u128 = 7200; // 2 hours
pub const REPAIR_COST_PER_SEC: u128 = 1;

// 系统耐久
pub const WEAPON_DUR_BASE: u128 = 500;
pub const WEAPON_DUR_PER_LV: u128 = 100;
pub const WEAPON_REPAIR_COST: u128 = 2;
pub const SHIELD_DUR_BASE: u128 = 259200; // 3 days
pub const SHIELD_DUR_PER_LV: u128 = 172800; // 2 days
pub const SHIELD_REPAIR_COST: u128 = 2;
pub const ENGINE_DUR_BASE: u128 = 50;
pub const ENGINE_DUR_PER_LV: u128 = 10;
pub const ENGINE_REPAIR_COST: u128 = 3;

// Token 桶
pub const TOKEN_INTERVAL_MS_BASE: u128 = 300;
pub const TOKEN_INTERVAL_REDUCTION: u128 = 10;
pub const TOKEN_BASE_MAX: u128 = 3;
pub const TOKEN_MAX_CAP: u128 = 10;

// 跳跃
pub const JUMP_ENERGY_BASE: u128 = 5000;
pub const JUMP_ENERGY_PER_SQRT: u128 = 5000;
pub const JUMP_ENERGY_MAX: u128 = 150000;
pub const JUMP_DFT_BASE: u128 = 3000;
pub const JUMP_DFT_PER_SQRT: u128 = 3000;
pub const JUMP_DFT_MAX: u128 = 100000;
pub const JUMP_COOLDOWN: u128 = 3600;
pub const TRACKING_RADAR_LV: u128 = 20;

// 联盟
pub const ALLIANCE_DEF_BONUS: u128 = 8;
pub const MAX_ALLIANCE_MEMBERS: usize = 25;

// 系统 ID
pub const SYS_COLLECTOR: usize = 0;
pub const SYS_WEAPON: usize = 1;
pub const SYS_SHIELD: usize = 2;
pub const SYS_RADAR: usize = 3;
pub const SYS_ENGINE: usize = 4;

// 后稀缺
pub const POST_SCARCITY_THRESHOLD: u128 = 970; // 千分比 97.0%

// ═══════════════════════════════════════════════════════════
// 全局 DFT 发行/销毁状态
// ═══════════════════════════════════════════════════════════
#[derive(Clone, Debug)]
pub struct GlobalState {
    pub total_minted: u128,
    pub total_burned: u128,
    pub post_scarcity: bool,
    pub supply_exhausted: bool,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            total_minted: 0,
            total_burned: 0,
            post_scarcity: false,
            supply_exhausted: false,
        }
    }

    pub fn update(&mut self, minted: u128, burned: u128) {
        self.total_minted = minted;
        self.total_burned = burned;
        self.supply_exhausted = minted >= TOTAL_SUPPLY;
        if minted > 0 {
            let ratio = burned * 1000 / minted;
            self.post_scarcity = ratio >= POST_SCARCITY_THRESHOLD;
        }
    }

    pub fn is_post_scarcity(&self) -> bool {
        self.post_scarcity
    }

    pub fn can_mint(&self) -> bool {
        self.total_minted < TOTAL_SUPPLY
    }
}

// ═══════════════════════════════════════════════════════════
// 精确合约公式
// ═══════════════════════════════════════════════════════════

pub fn calc_upgrade_cost(system: &str, lv: u128) -> u128 {
    let (a, b) = match system {
        "collector" => (UP_A_COLLECTOR, UP_B_COLLECTOR),
        "weapon" => (UP_A_WEAPON, UP_B_WEAPON),
        "shield" => (UP_A_SHIELD, UP_B_SHIELD),
        "radar" => (UP_A_RADAR, UP_B_RADAR),
        "engine" => (UP_A_ENGINE, UP_B_ENGINE),
        _ => (0, 0),
    };
    a * lv * (lv + b) / 100
}

pub fn calc_upgrade_energy(system: &str, lv: u128) -> u128 {
    // ENERGY_UP_BASE * 2^sysId * (1 + lv * 0.5)
    let sys_id = match system {
        "collector" => 0u128,
        "weapon" => 1,
        "shield" => 2,
        "radar" => 3,
        "engine" => 4,
        _ => 0,
    };
    let base = ENERGY_UP_BASE * (1u128 << sys_id); // 2^sys_id
    // (1 + lv * 0.5) → (100 + lv * 50) / 100
    base * (100 + lv * 50) / 100
}

pub fn calc_attack(lv: u128) -> u128 {
    ATK_BASE + ATK_RATE * lv * lv
}

pub fn calc_defense(lv: u128) -> u128 {
    DEF_BASE + DEF_RATE * lv * lv
}

pub fn calc_shield_hp(lv: u128) -> u128 {
    SHIELD_HP_BASE + SHIELD_HP_RATE * lv * lv
}

pub fn calc_shield_regen(lv: u128) -> u128 {
    REGEN_BASE + REGEN_RATE * lv * lv
}

pub fn calc_radar(lv: u128) -> u128 {
    RADAR_BASE + RADAR_LINEAR * lv + RADAR_QUAD * lv * lv
}

pub fn calc_speed(lv: u128) -> u128 {
    if lv <= 1 {
        ENGINE_SPEED_BASE
    } else {
        ENGINE_SPEED_BASE + ENGINE_SPEED_PER_LV * (lv - 1)
    }
}

pub fn calc_collect(lv: u128, referrals: u128) -> u128 {
    let base = if lv <= 1 {
        BASE_COLLECT
    } else {
        BASE_COLLECT + COLLECT_BONUS * isqrt(lv - 1)
    };
    base * (1000 + referrals * 2) / 1000
}

pub fn calc_max_durability(lv: u128) -> u128 {
    if lv <= 1 {
        DURABILITY_BASE
    } else {
        DURABILITY_BASE + DURABILITY_PER_LV * (lv - 1)
    }
}

pub fn calc_max_system_dur(sys_id: usize, lv: u128) -> u128 {
    match sys_id {
        SYS_WEAPON => {
            if lv <= 1 {
                WEAPON_DUR_BASE
            } else {
                WEAPON_DUR_BASE + WEAPON_DUR_PER_LV * (lv - 1)
            }
        }
        SYS_SHIELD => {
            if lv <= 1 {
                SHIELD_DUR_BASE
            } else {
                SHIELD_DUR_BASE + SHIELD_DUR_PER_LV * (lv - 1)
            }
        }
        SYS_ENGINE => {
            if lv <= 1 {
                ENGINE_DUR_BASE
            } else {
                ENGINE_DUR_BASE + ENGINE_DUR_PER_LV * (lv - 1)
            }
        }
        _ => 0,
    }
}

pub fn calc_attack_energy_cost(weapon_lv: u128) -> u128 {
    ATTACK_ENERGY_BASE + ATTACK_ENERGY_PER_LV * weapon_lv
}

pub fn calc_token_regen_interval(weapon_lv: u128) -> u128 {
    let ms = TOKEN_INTERVAL_MS_BASE.saturating_sub(weapon_lv * TOKEN_INTERVAL_REDUCTION);
    if ms < 100 {
        100
    } else {
        ms
    } // 返回毫秒
}

pub fn calc_max_tokens(weapon_lv: u128) -> u128 {
    let cap = TOKEN_BASE_MAX + weapon_lv / 10;
    if cap > TOKEN_MAX_CAP {
        TOKEN_MAX_CAP
    } else {
        cap
    }
}

pub fn calc_jump_energy_cost(jump_count: u128) -> u128 {
    let cost = JUMP_ENERGY_BASE + JUMP_ENERGY_PER_SQRT * isqrt(jump_count);
    if cost > JUMP_ENERGY_MAX {
        JUMP_ENERGY_MAX
    } else {
        cost
    }
}

pub fn calc_jump_dft_cost(jump_count: u128) -> u128 {
    let cost = JUMP_DFT_BASE + JUMP_DFT_PER_SQRT * isqrt(jump_count);
    if cost > JUMP_DFT_MAX {
        JUMP_DFT_MAX
    } else {
        cost
    }
}

// ═══════════════════════════════════════════════════════════
// 战斗模拟
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AttackResult {
    pub shield_dmg: u128,
    pub health_dmg: u128,
    pub remaining_shield: u128,
    pub remaining_health: u128,
    pub stolen_energy: u128,
    pub defender_destroyed: bool,
    pub attacker_energy_cost: u128,
}

pub fn simulate_attack(
    att_weapon: u128,
    def_shield: u128,
    def_energy: u128,
    def_health: u128,
    def_shield_hp: u128,
    alliance_members: u128,
) -> AttackResult {
    let atk = calc_attack(att_weapon);
    let defense = calc_defense(def_shield) + alliance_members * ALLIANCE_DEF_BONUS;
    let energy_cost = calc_attack_energy_cost(att_weapon);

    // 护盾伤害
    let shield_dmg = {
        let d = atk + SHIELD_DMG_BONUS - defense;
        if d > 0 { d } else { 0 }
    };

    // 生命伤害
    let health_dmg = if atk > defense {
        (atk - defense) * 2
    } else {
        0
    };

    // 护盾吸收
    let (shield_dmg, health_dmg) = if def_shield_hp < shield_dmg {
        let overflow = shield_dmg - def_shield_hp;
        (def_shield_hp, health_dmg + overflow * 3)
    } else {
        (shield_dmg, health_dmg)
    };

    let remaining_shield = def_shield_hp.saturating_sub(shield_dmg);
    let remaining_health = def_health.saturating_sub(health_dmg);

    // 掠夺
    let (stolen, destroyed) = if remaining_health == 0 {
        let s = def_energy * PLUNDER_RATIO / 10000;
        let s = if s > def_energy { def_energy } else { s };
        (s, true)
    } else {
        (0, false)
    };

    AttackResult {
        shield_dmg,
        health_dmg,
        remaining_shield,
        remaining_health,
        stolen_energy: stolen,
        defender_destroyed: destroyed,
        attacker_energy_cost: energy_cost,
    }
}

// ═══════════════════════════════════════════════════════════
// 距离计算
// ═══════════════════════════════════════════════════════════

/// 3D 欧几里得距离（合约 _distance 的精确复制）
pub fn distance(ax: i128, ay: i128, az: i128, bx: i128, by: i128, bz: i128) -> u128 {
    let dx = if (ax >= 0) == (bx >= 0) {
        let d = ax.abs_diff(bx);
        d
    } else {
        ax.unsigned_abs() + bx.unsigned_abs()
    };
    let dy = if (ay >= 0) == (by >= 0) {
        ay.abs_diff(by)
    } else {
        ay.unsigned_abs() + by.unsigned_abs()
    };
    let dz = if (az >= 0) == (bz >= 0) {
        az.abs_diff(bz)
    } else {
        az.unsigned_abs() + bz.unsigned_abs()
    };

    // 大距离提前返回
    // 2^127 ≈ 1.7e38, 检查平方溢出
    if dx > (1u128 << 127) || dy > (1u128 << 127) || dz > (1u128 << 127) {
        return u128::MAX;
    }
    isqrt(dx * dx + dy * dy + dz * dz)
}

pub fn is_in_range(
    ax: i128, ay: i128, az: i128,
    bx: i128, by: i128, bz: i128,
    scan_range: u128,
) -> bool {
    distance(ax, ay, az, bx, by, bz) <= scan_range
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(144), 12);
        assert_eq!(isqrt(1000000), 1000);
    }

    #[test]
    fn test_upgrade_cost() {
        // Lv1→2: A * 1 * (1 + B) / 100
        let c = calc_upgrade_cost("collector", 1);
        assert_eq!(c, UP_A_COLLECTOR * 1 * (1 + UP_B_COLLECTOR) / 100);
    }

    #[test]
    fn test_attack() {
        let r = simulate_attack(1, 1, 10000, 3000, 3600, 0);
        assert!(r.attacker_energy_cost > 0);
    }
}
