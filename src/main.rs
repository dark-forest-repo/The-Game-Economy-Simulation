// Dark Forest 游戏模拟器 — Rust 版 (人格+邀请+外交+非理性)
// 用法: cargo run --release -- <scenario> [args...]

mod math_engine;
mod player;
mod engine;

use std::time::Instant;
use engine::{SimConfig, GameEngine};
use std::collections::HashMap;

fn print_header(title: &str) {
    println!("\n{}", "=".repeat(75));
    println!("  {}", title);
    println!("{}", "=".repeat(75));
}

fn run_milestone_scenario(config: SimConfig, label: &str) {
    print_header(label);
    let start = Instant::now();
    let mut engine = GameEngine::new(config);
    let mut ps_day = None;
    let mut se_day = None;
    let total_days = engine.cfg.simulation_days;

    for _ in 0..total_days {
        engine._daily_step();
        engine.time += 86400;
        engine.day += 1;
        engine.global_state.update(engine.total_dft_minted, engine.total_dft_burned);

        if ps_day.is_none() && engine.global_state.is_post_scarcity() { ps_day = Some(engine.day); }
        if se_day.is_none() && engine.global_state.supply_exhausted { se_day = Some(engine.day); }

        if engine.day % 365 == 0 || engine.day == total_days || engine.day <= 7 {
            let active = engine.active_players().count();
            let burn = if engine.total_dft_minted > 0 { engine.total_dft_burned as f64 / engine.total_dft_minted as f64 * 100.0 } else { 0.0 };
            let avg_lv: f64 = {
                let ap: Vec<_> = engine.active_players().collect();
                if ap.is_empty() { 0.0 } else { ap.iter().map(|p| p.total_level() as f64).sum::<f64>() / ap.len() as f64 / 5.0 }
            };
            let wars = engine.alliances.values().filter(|a| !a.war_targets.is_empty()).count();
            let quits = engine.total_quits;
            let rebirths: u32 = engine.players.values().map(|p| p.rebirth_count).sum();
            println!("  Y{:2} | alive={:5} dead={:6} quit={:5} | Lv={:5.1} | burn={:5.1}% PS={}⚔️{} | reb={} mkt={}E/DFT",
                engine.day / 365, active, engine.players.len() - active - (engine.players.values().filter(|p| p.is_quit()).count() as usize), quits,
                avg_lv, burn,
                if engine.global_state.is_post_scarcity() { "Y" } else { "." },
                wars, rebirths, engine.market_rate);
        }
    }

    let elapsed = start.elapsed();
    let active: Vec<_> = engine.active_players().collect();
    let total_players = engine.players.len();

    // 人格分布
    let mut pcount: HashMap<&str, usize> = HashMap::new();
    for p in &active { *pcount.entry(p.personality_type).or_insert(0) += 1; }
    let mut sorted: Vec<_> = pcount.into_iter().collect();
    sorted.sort_by_key(|(_, c)| *c);
    let ps: String = sorted.iter().rev().map(|(t, c)| format!("{}:{}", t, c)).collect::<Vec<_>>().join(" ");

    // 联盟状态
    let total_alliances = engine.alliances.len();
    let wars_now = engine.alliances.values().filter(|a| !a.war_targets.is_empty()).count();

    // 顶级
    let top = active.iter().max_by_key(|p| p.total_level());
    let top_s = match top {
        Some(p) => format!("Lv{} {} {} Gen{} atks={} kills={} reborn={}x{:.2} addr={}",
            p.total_level() / 5, p.personality_type, p.name, p.generation,
            p.total_attacks, p.total_victims,
            p.rebirth_count, p.growth_multiplier, p.address),
        None => "N/A".to_string(),
    };

    let burn_pct = if engine.total_dft_minted > 0 {
        engine.total_dft_burned as f64 / engine.total_dft_minted as f64 * 100.0
    } else { 0.0 };

    println!("\n  ════════ 总结 ════════");
    println!("  耗时: {:.2}s", elapsed.as_secs_f64());
    println!("  存活: {}/{}  ({} quit)", active.len(), total_players, engine.total_quits);
    if let Some(d) = ps_day { println!("  后稀缺: Year {} (Day {})", d / 365, d); }
    if let Some(d) = se_day { println!("  发行完毕: Year {} (Day {})", d / 365, d); }
    println!("  邀请: {} | 联盟: {} | 战争中: {} | 涅槃: {}",
        engine.total_invites, total_alliances, wars_now,
        engine.players.values().map(|p| p.rebirth_count as u128).sum::<u128>());
    println!("  DFT烧毁: {:.1}% ({}M / {}M)", burn_pct,
        engine.total_dft_burned / 10u128.pow(18), engine.total_dft_minted / 10u128.pow(18));
    println!("  存活人格: {}", ps);
    println!("  顶级: {}", top_s);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("decade");

    match mode {
        "decade" | "10yr" => run_milestone_scenario(
            SimConfig { simulation_days: 3650, ..Default::default() },
            "10年 (200初始 + 每日~20流量 + 邀请)"),
        "30yr" | "30" => run_milestone_scenario(
            SimConfig { simulation_days: 10950, ..Default::default() },
            "30年 (200初始 + 每日~20流量 + 邀请)"),
        "50yr" | "50" => run_milestone_scenario(
            SimConfig { simulation_days: 18250, ..Default::default() },
            "50年 (200初始 + 每日~20流量 + 邀请)"),
        "big" | "large" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(365);
            let init: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1000);
            let daily: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);
            run_milestone_scenario(
                SimConfig { simulation_days: days, initial_players: init, daily_spawn: daily, ..Default::default() },
                &format!("大规模: {}天 初始{} 每日~{}", days, init, daily));
        }
        "social" | "society" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3650);
            run_milestone_scenario(
                SimConfig { simulation_days: days, daily_spawn: 5, ..Default::default() },
                &format!("社交: {}天 每日~5人", days));
        }
        "no-spawn" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(365);
            run_milestone_scenario(
                SimConfig { simulation_days: days, daily_spawn: 0, initial_players: 0, ..Default::default() },
                &format!("纯邀请: {}天 无注入", days));
        }
        "competition" | "comp" => run_milestone_scenario(
            SimConfig { simulation_days: 60, initial_players: 300, daily_spawn: 0, ..Default::default() },
            "60天竞争 (300初始, 无注入)"),
        "fast" | "quick" => run_milestone_scenario(
            SimConfig { simulation_days: 365, ..Default::default() },
            "快速1年测试"),
        "custom" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(365);
            let init: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
            let daily: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
            let invite: bool = args.get(5).map(|s| s == "1" || s == "true").unwrap_or(true);
            let diplo: bool = args.get(6).map(|s| s == "1" || s == "true").unwrap_or(true);
            run_milestone_scenario(
                SimConfig {
                    simulation_days: days, initial_players: init, daily_spawn: daily,
                    invite_enabled: invite, diplomacy_enabled: diplo, ..Default::default()
                },
                &format!("自定义: {}天 {}初始 {}日流 邀请={} 外交={}", days, init, daily, invite, diplo));
        }
        _ => {
            eprintln!("场景:");
            eprintln!("  decade [天]      10年 (200初始 + 每日~20流量)");
            eprintln!("  30yr [天]        30年");
            eprintln!("  50yr [天]        50年");
            eprintln!("  big [天] [初始] [日流]  大规模测试");
            eprintln!("  fast             快速1年测试");
            eprintln!("  competition      60天 300初始");
            eprintln!("  custom <天> <初始> <日流> <邀请> <外交>");
            eprintln!("");
            eprintln!("默认: initial_players=200, daily_spawn=20±50%, invite=高概率, diplomacy=on");
        }
    }
}
