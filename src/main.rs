// Dark Forest 游戏模拟器 — SoA + 混合模型 + GPU (可选)
mod math_engine;
mod player;
mod store;
mod coldstore;
mod engine;
mod battle_engine;
#[cfg(feature = "gpu")]
mod gpu_compute;

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
            let active = engine.active_count();
            let burn = if engine.total_dft_minted > 0 { engine.total_dft_burned as f64 / engine.total_dft_minted as f64 * 100.0 } else { 0.0 };
            let avg_lv: f64 = {
                let ap: Vec<_> = engine.store.active_indices().collect();
                if ap.is_empty() { 0.0 } else { ap.iter().map(|&idx| engine.store.total_level(idx) as f64).sum::<f64>() / ap.len() as f64 / 5.0 }
            };
            let wars = engine.alliances.values().filter(|a| !a.war_targets.is_empty()).count();
            let elite = engine.store.active_indices().filter(|&idx| engine.store.cold.is_elite(idx)).count();
            println!("  Y{:2} | alive={:5} dead={:5} | Lv={:5.1} elite={:5} | burn={:5.1}% PS={}⚔️{} | mkt={}E/DFT",
                engine.day / 365, active, engine.store.ids.len() - active, avg_lv, elite, burn,
                if engine.global_state.is_post_scarcity() { "Y" } else { "." },
                wars, engine.market_rate);
        }
    }

    let elapsed = start.elapsed();
    let active: Vec<_> = engine.store.active_indices().collect();
    let total_players = engine.store.ids.len();

    let mut pcount: HashMap<&str, usize> = HashMap::new();
    for &idx in &active { *pcount.entry(engine.store.personality_types[idx as usize]).or_insert(0) += 1; }
    let mut sorted: Vec<_> = pcount.into_iter().collect();
    sorted.sort_by_key(|(_, c)| *c);
    let ps: String = sorted.iter().rev().map(|(t, c)| format!("{}:{}", t, c)).collect::<Vec<_>>().join(" ");

    let total_alliances = engine.alliances.len();
    let wars_now = engine.alliances.values().filter(|a| !a.war_targets.is_empty()).count();

    let top = active.iter().max_by_key(|&&idx| engine.store.total_level(idx)).copied();
    let top_s = match top {
        Some(idx) => {
            let i = idx as usize;
            format!("Lv{} {} {} Gen{} atks={} kills={} reborn={}x{:.2} addr={}",
                engine.store.total_level(idx) / 5, engine.store.personality_types[i],
                engine.store.names[i], engine.store.cold.generation(idx),
                engine.store.total_attacks[i], engine.store.total_victims[i],
                engine.store.cold.rebirth_count(idx), engine.store.cold.growth_multiplier(idx),
                engine.store.addresses[i])
        }
        None => "N/A".to_string(),
    };

    let burn_pct = if engine.total_dft_minted > 0 {
        engine.total_dft_burned as f64 / engine.total_dft_minted as f64 * 100.0
    } else { 0.0 };

    println!("\n  ════════ 总结 ════════");
    println!("  耗时: {:.2}s", elapsed.as_secs_f64());
    println!("  存活: {}/{} (精英: {})", active.len(), total_players, active.iter().filter(|&&idx| engine.store.cold.is_elite(idx)).count());
    if let Some(d) = ps_day { println!("  后稀缺: Year {} (Day {})", d / 365, d); }
    if let Some(d) = se_day { println!("  发行完毕: Year {} (Day {})", d / 365, d); }
    println!("  联盟: {} | 战争中: {}", total_alliances, wars_now);
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
            SimConfig { simulation_days: 3650, ..Default::default() }, "10年"),
        "30yr" | "30" => run_milestone_scenario(
            SimConfig { simulation_days: 10950, ..Default::default() }, "30年"),
        "fast" | "quick" => run_milestone_scenario(
            SimConfig { simulation_days: 365, ..Default::default() }, "快速1年"),
        "competition" | "comp" => run_milestone_scenario(
            SimConfig { simulation_days: 60, initial_players: 300, daily_spawn: 0, invite_enabled: false, diplomacy_enabled: false, ..Default::default() },
            "60天竞争 (300初始)"),
        "big" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(365);
            let init: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10000);
            run_milestone_scenario(
                SimConfig { simulation_days: days, initial_players: init, daily_spawn: 0, invite_enabled: false, diplomacy_enabled: false, ..Default::default() },
                &format!("大规模: {}天 初始{}", days, init));
        }
        "custom" => {
            let days: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(365);
            let init: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
            let daily: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            run_milestone_scenario(
                SimConfig { simulation_days: days, initial_players: init, daily_spawn: daily, ..Default::default() },
                &format!("自定义: {}天 初始{} 日流{}", days, init, daily));
        }
        _ => {
            eprintln!("场景: decade | 30yr | fast | competition | big [天] [初始] | custom <天> <初始> <日流>");
            eprintln!("GPU: cargo run --features gpu -- <场景>");
        }
    }
}
