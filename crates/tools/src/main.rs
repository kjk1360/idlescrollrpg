use belt_core::{sample_battle_config, BattleEvent, BattleWorld};
use data_studio_core::{sample_project, DataProject, ProjectFingerprints, ProjectStatus};
use game_data_adapter::battle_config_from_project;
use generated_data::relation_cache::GeneratedRelationCache;
use generated_data::table_accessors::GeneratedDatabase;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod aseprite;
mod play;
mod serve;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");

    let result = match command {
        "simulate" => simulate(&args[1..]),
        "data-status" => data_status(&args[1..]),
        "validate" => validate(&args[1..]),
        "view" => view(&args[1..]),
        "codegen-preview" => codegen_preview(&args[1..]),
        "codegen" => codegen(&args[1..]),
        "data-build" => data_build(&args[1..]),
        "import-aseprite" => import_aseprite(&args[1..]),
        "serve" => serve::serve(&args[1..]),
        "play" => play::play(&args[1..]),
        _ => {
            help();
            Ok(())
        }
    };

    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn help() {
    println!("belt_tools");
    println!();
    println!("Commands:");
    println!("  simulate         Run the endless-left battle simulation");
    println!("  data-status      Print schema/data freshness state");
    println!("  validate         Validate a data project");
    println!("  view             Print a materialized data view");
    println!("  codegen-preview  Print generated Rust struct preview");
    println!("  codegen          Write generated Rust files");
    println!("  data-build       Write a JSON data snapshot and data fingerprint");
    println!("  import-aseprite  Import an Aseprite file or exported JSON into visual data");
    println!("  serve            Start the local Data Studio web UI");
    println!("  play             Start the playable belt-scroll preview");
    println!();
    println!("Common options:");
    println!("  --project <dir>  Load a file-based data project");
    println!("  --out <dir>      Output directory for codegen or data-build");
    println!("  --addr <addr>    Local server address for serve");
    println!("  --file <path>    Aseprite .aseprite/.ase or exported JSON file");
    println!("  --current-energy <n>  Account energy before elapsed recovery in simulate");
    println!("  --elapsed-seconds <n> Account energy recovery seconds before simulate");
    println!("  --seed <n>       Deterministic reward seed for simulate");
    println!("  --occupied-material-slots <n> Existing occupied material storage slots");
    println!("  --occupied-equipment-slots <n> Existing occupied equipment storage slots");
    println!("  --occupied-consumable-slots <n> Existing occupied consumable storage slots");
    println!("  --account-state <path> Load local account state JSON for simulate");
    println!("  --write-account-state Save energy, inventory, and overflow mail after simulate");
    println!("  --now-unix <n>  Deterministic unix time for account-state writeback");
}

fn simulate(args: &[String]) -> Result<(), String> {
    let map_key = option_value(args, "--map").unwrap_or("endless_left_road");
    let current_energy_override = option_value(args, "--current-energy")
        .map(parse_i32)
        .transpose()?;
    let elapsed_seconds = option_value(args, "--elapsed-seconds")
        .map(parse_i64)
        .transpose()?
        .unwrap_or(0);
    let seed = option_value(args, "--seed")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1);
    let now_unix = option_value(args, "--now-unix")
        .map(parse_i64)
        .transpose()?
        .unwrap_or_else(current_unix_time);
    let write_account_state = has_flag(args, "--write-account-state");
    let occupied_slots = occupied_slots_from_args(args)?;
    let loaded_project = if option_value(args, "--project").is_some() {
        Some(load_project(args)?)
    } else {
        None
    };
    let account_state_path = option_value(args, "--account-state")
        .map(PathBuf::from)
        .or_else(|| {
            if !write_account_state {
                return None;
            }
            loaded_project
                .as_ref()
                .and_then(|(_, path)| path.as_ref().map(|path| path.join("account_state.json")))
        });

    let mut account_state = match (&loaded_project, &account_state_path) {
        (Some((project, _)), Some(path)) => Some(load_or_create_account_state(project, path)?),
        (None, Some(_)) => {
            return Err(
                "--account-state requires --project so item and energy data can be resolved"
                    .to_string(),
            )
        }
        _ => None,
    };

    let project_for_rewards = loaded_project.as_ref().map(|(project, _)| project);
    let config = if let Some(project) = project_for_rewards {
        battle_config_from_project(project, map_key)?
    } else {
        sample_battle_config()
    };
    if let Some(project) = project_for_rewards {
        let current_energy =
            current_energy_override.or_else(|| account_state.as_ref().map(|state| state.energy));
        let energy = energy_preview(project, map_key, current_energy, elapsed_seconds)?;
        if energy.after_recovery < energy.cost {
            return Err(format!(
                "not enough account energy: current_after_recovery={} cost={}",
                energy.after_recovery, energy.cost
            ));
        }
        println!(
            "energy: current={}, recovered={}, cost={}, after_dispatch={}",
            energy.current,
            energy.after_recovery,
            energy.cost,
            energy.after_dispatch()
        );
        if let Some(state) = account_state.as_mut() {
            state.energy = energy.after_dispatch();
            state.last_energy_update_unix = now_unix;
        }
    }
    let mut world = BattleWorld::new(config);
    let mut wave_clears = 0;
    let mut kills = 0;
    let mut map_cleared = false;

    for frame in 0..360 {
        world.tick(0.1);
        for event in world.drain_events() {
            match event {
                BattleEvent::WaveStarted { wave_id } => {
                    println!("[{frame:03}] wave started: {wave_id}");
                }
                BattleEvent::UnitSpawned {
                    unit_id,
                    name,
                    team,
                } => {
                    println!("[{frame:03}] spawned {:?} {name} ({team:?})", unit_id);
                }
                BattleEvent::UnitKilled { unit_id } => {
                    kills += 1;
                    println!("[{frame:03}] killed {:?}", unit_id);
                }
                BattleEvent::WaveCleared { wave_id } => {
                    wave_clears += 1;
                    println!("[{frame:03}] wave cleared: {wave_id}");
                }
                BattleEvent::MapCleared { map_id } => {
                    map_cleared = true;
                    println!("[{frame:03}] map cleared: {map_id}");
                }
                BattleEvent::MapLooped { map_id, loop_count } => {
                    println!("[{frame:03}] map looped: {map_id} loop={loop_count}");
                }
                _ => {}
            }
        }
    }

    let living_players = world
        .units()
        .iter()
        .filter(|unit| unit.team == belt_core::Team::Player)
        .count();

    println!();
    println!("summary: kills={kills}, wave_clears={wave_clears}, living_players={living_players}");
    if let Some(project) = project_for_rewards {
        let rewards = if map_cleared {
            settle_rewards(project, map_key, seed)?
        } else {
            Vec::new()
        };
        print_rewards(&rewards);
        let storage = settle_storage(project, &rewards, &occupied_slots)?;
        print_storage_settlement(&storage);
        if let Some(state) = account_state.as_mut() {
            let account_settlement =
                apply_rewards_to_account_state(project, state, &rewards, now_unix)?;
            print_account_settlement(&account_settlement);
            if write_account_state {
                let path = account_state_path
                    .as_ref()
                    .ok_or_else(|| "missing account state path".to_string())?;
                save_account_state(path, state)?;
                println!("account_state: saved {}", path.display());
            } else if let Some(path) = &account_state_path {
                println!("account_state: preview only {}", path.display());
            }
        }
    }
    Ok(())
}

fn simulate_to_string(project: &DataProject, map_key: &str) -> Result<String, String> {
    let energy = energy_preview(project, map_key, None, 0)?;
    let config = battle_config_from_project(project, map_key)?;
    let mut world = BattleWorld::new(config);
    let mut wave_clears = 0;
    let mut kills = 0;
    let mut map_cleared = false;
    let mut lines = Vec::new();
    lines.push(format!(
        "energy: current={}, recovered={}, cost={}, after_dispatch={}",
        energy.current,
        energy.after_recovery,
        energy.cost,
        energy.after_dispatch()
    ));

    for frame in 0..360 {
        world.tick(0.1);
        for event in world.drain_events() {
            match event {
                BattleEvent::WaveStarted { wave_id } => {
                    lines.push(format!("[{frame:03}] wave started: {wave_id}"));
                }
                BattleEvent::UnitSpawned {
                    unit_id,
                    name,
                    team,
                } => {
                    lines.push(format!(
                        "[{frame:03}] spawned {:?} {name} ({team:?})",
                        unit_id
                    ));
                }
                BattleEvent::UnitKilled { unit_id } => {
                    kills += 1;
                    lines.push(format!("[{frame:03}] killed {:?}", unit_id));
                }
                BattleEvent::WaveCleared { wave_id } => {
                    wave_clears += 1;
                    lines.push(format!("[{frame:03}] wave cleared: {wave_id}"));
                }
                BattleEvent::MapCleared { map_id } => {
                    map_cleared = true;
                    lines.push(format!("[{frame:03}] map cleared: {map_id}"));
                }
                BattleEvent::MapLooped { map_id, loop_count } => {
                    lines.push(format!(
                        "[{frame:03}] map looped: {map_id} loop={loop_count}"
                    ));
                }
                _ => {}
            }
        }
    }

    let living_players = world
        .units()
        .iter()
        .filter(|unit| unit.team == belt_core::Team::Player)
        .count();

    lines.push(String::new());
    lines.push(format!(
        "summary: kills={kills}, wave_clears={wave_clears}, living_players={living_players}"
    ));
    if map_cleared {
        let rewards = settle_rewards(project, map_key, 1)?;
        lines.extend(reward_lines(&rewards));
        let storage = settle_storage(project, &rewards, &HashMap::new())?;
        lines.extend(storage_settlement_lines(&storage));
    } else {
        lines.push("rewards: none".to_string());
        lines.push("storage: none".to_string());
    }
    Ok(lines.join("\n"))
}

#[derive(Debug, Clone)]
struct EnergyPreview {
    current: i32,
    after_recovery: i32,
    cost: i32,
}

impl EnergyPreview {
    fn after_dispatch(&self) -> i32 {
        (self.after_recovery - self.cost).max(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RewardStack {
    item_key: String,
    item_name: String,
    category: String,
    rarity: String,
    stack_size: i32,
    quantity: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageSettlement {
    tabs: Vec<StorageTabSettlement>,
    overflow_mail: Vec<RewardStack>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageTabSettlement {
    tab_key: String,
    name: String,
    capacity: i32,
    occupied_before: i32,
    used_slots: i32,
    placed_quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AccountState {
    energy: i32,
    last_energy_update_unix: i64,
    inventory: Vec<AccountItemStack>,
    mail: Vec<AccountMail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AccountItemStack {
    item_key: String,
    quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct AccountMail {
    item_key: String,
    quantity: i32,
    expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountRewardSettlement {
    placed: Vec<RewardStack>,
    overflow_mail: Vec<RewardStack>,
    inventory_slots: HashMap<String, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EnergyRecovery {
    current: i32,
    recovered: i32,
    after_recovery: i32,
    max_energy: i32,
    recover_seconds: i32,
    recover_amount: i32,
    seconds_until_next_recovery: i64,
}

fn energy_preview(
    project: &DataProject,
    map_key: &str,
    current_energy: Option<i32>,
    elapsed_seconds: i64,
) -> Result<EnergyPreview, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let map = db
        .map_def
        .get_by_key(map_key)
        .ok_or_else(|| format!("missing map {map_key}"))?;
    let config = db
        .account_energy_config
        .rows
        .first()
        .ok_or_else(|| "missing account energy config".to_string())?;
    let current = current_energy
        .unwrap_or(config.max_energy)
        .clamp(0, config.max_energy);
    let recover_seconds = config.recover_seconds.max(1) as i64;
    let recover_amount = config.recover_amount.max(0);
    let recovered = ((elapsed_seconds.max(0) / recover_seconds) as i32) * recover_amount;
    let after_recovery = (current + recovered).min(config.max_energy);
    Ok(EnergyPreview {
        current,
        after_recovery,
        cost: map.energy_cost.max(0),
    })
}

fn settle_rewards(
    project: &DataProject,
    map_key: &str,
    seed: u64,
) -> Result<Vec<RewardStack>, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let cache = GeneratedRelationCache::build(&db)?;
    let map = db
        .map_def
        .get_by_key(map_key)
        .ok_or_else(|| format!("missing map {map_key}"))?;
    let drop_table_id = cache
        .get_map_def_drop_table(map.id)
        .ok_or_else(|| format!("missing drop table relation for map {map_key}"))?;
    let entry_ids = cache
        .get_drop_table_entries(drop_table_id)
        .ok_or_else(|| format!("missing drop entries for map {map_key}"))?;
    let mut rng = DeterministicRng::new(seed ^ map.id.0 as u64);
    let mut rewards = Vec::new();

    for entry_id in entry_ids {
        let entry = db
            .drop_entry
            .get_by_id(*entry_id)
            .ok_or_else(|| format!("missing drop entry {:?}", entry_id))?;
        let chance = entry.chance_per_10000.clamp(0, 10000);
        if rng.next_range(1, 10000) > chance {
            continue;
        }
        let min_quantity = entry.min_quantity.max(0);
        let max_quantity = entry.max_quantity.max(min_quantity);
        let quantity = rng.next_range(min_quantity, max_quantity);
        if quantity <= 0 {
            continue;
        }
        let item_id = cache
            .get_drop_entry_item(entry.id)
            .ok_or_else(|| format!("missing item relation for drop entry {}", entry.key))?;
        let item = db
            .item_def
            .get_by_id(item_id)
            .ok_or_else(|| format!("missing item {:?}", item_id))?;
        rewards.push(RewardStack {
            item_key: item.key.clone(),
            item_name: item.name.clone(),
            category: item.category.clone(),
            rarity: item.rarity.clone(),
            stack_size: item.stack_size.max(1),
            quantity,
        });
    }

    Ok(rewards)
}

fn settle_storage(
    project: &DataProject,
    rewards: &[RewardStack],
    occupied_slots: &HashMap<String, i32>,
) -> Result<StorageSettlement, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let mut overflow_mail = Vec::new();
    let mut tabs = db
        .storage_tab_config
        .rows
        .iter()
        .map(|tab| StorageTabSettlement {
            tab_key: tab.tab_key.clone(),
            name: tab.name.clone(),
            capacity: tab.base_capacity.max(0),
            occupied_before: occupied_slots
                .get(&tab.tab_key)
                .copied()
                .unwrap_or(0)
                .clamp(0, tab.base_capacity.max(0)),
            used_slots: 0,
            placed_quantity: 0,
        })
        .collect::<Vec<_>>();

    for reward in rewards {
        let Some(tab_config) = db
            .storage_tab_config
            .rows
            .iter()
            .find(|tab| tab.item_category == reward.category)
        else {
            overflow_mail.push(reward.clone());
            continue;
        };
        let Some(tab) = tabs
            .iter_mut()
            .find(|tab| tab.tab_key == tab_config.tab_key)
        else {
            overflow_mail.push(reward.clone());
            continue;
        };

        let free_slots = (tab.capacity - tab.occupied_before - tab.used_slots).max(0);
        let required_slots = div_ceil_i32(reward.quantity.max(0), reward.stack_size.max(1));
        let placed_slots = required_slots.min(free_slots);
        let placed_quantity = if placed_slots >= required_slots {
            reward.quantity
        } else {
            (placed_slots * reward.stack_size.max(1)).min(reward.quantity)
        };
        tab.used_slots += placed_slots;
        tab.placed_quantity += placed_quantity;

        let overflow_quantity = reward.quantity - placed_quantity;
        if overflow_quantity > 0 {
            let mut overflow = reward.clone();
            overflow.quantity = overflow_quantity;
            overflow_mail.push(overflow);
        }
    }

    Ok(StorageSettlement {
        tabs,
        overflow_mail,
    })
}

fn apply_rewards_to_account_state(
    project: &DataProject,
    state: &mut AccountState,
    rewards: &[RewardStack],
    now_unix: i64,
) -> Result<AccountRewardSettlement, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let mut placed = Vec::new();
    let mut overflow_mail = Vec::new();

    for reward in rewards {
        let tab = db
            .storage_tab_config
            .rows
            .iter()
            .find(|tab| tab.item_category == reward.category);
        let Some(tab) = tab else {
            push_account_mail(state, reward, reward.quantity, now_unix);
            overflow_mail.push(reward.clone());
            continue;
        };

        let capacity = tab.base_capacity.max(0);
        let mut remaining = reward.quantity.max(0);
        let mut placed_quantity = 0;

        for stack in state
            .inventory
            .iter_mut()
            .filter(|stack| stack.item_key == reward.item_key)
        {
            if remaining <= 0 {
                break;
            }
            let free_quantity = (reward.stack_size.max(1) - stack.quantity).max(0);
            let add = free_quantity.min(remaining);
            stack.quantity += add;
            remaining -= add;
            placed_quantity += add;
        }

        while remaining > 0 {
            let used_slots = inventory_slots_for_category(&db, state, &reward.category)?;
            if used_slots >= capacity {
                break;
            }
            let add = remaining.min(reward.stack_size.max(1));
            state.inventory.push(AccountItemStack {
                item_key: reward.item_key.clone(),
                quantity: add,
            });
            remaining -= add;
            placed_quantity += add;
        }

        if placed_quantity > 0 {
            let mut placed_reward = reward.clone();
            placed_reward.quantity = placed_quantity;
            placed.push(placed_reward);
        }
        if remaining > 0 {
            push_account_mail(state, reward, remaining, now_unix);
            let mut overflow = reward.clone();
            overflow.quantity = remaining;
            overflow_mail.push(overflow);
        }
    }

    Ok(AccountRewardSettlement {
        placed,
        overflow_mail,
        inventory_slots: inventory_slots_by_tab(&db, state)?,
    })
}

fn push_account_mail(state: &mut AccountState, reward: &RewardStack, quantity: i32, now_unix: i64) {
    state.mail.push(AccountMail {
        item_key: reward.item_key.clone(),
        quantity: quantity.max(0),
        expires_at_unix: now_unix + 86_400,
    });
}

fn purge_expired_mail(state: &mut AccountState, now_unix: i64) -> usize {
    let before = state.mail.len();
    state.mail.retain(|mail| mail.expires_at_unix > now_unix);
    before - state.mail.len()
}

fn inventory_slots_for_category(
    db: &GeneratedDatabase,
    state: &AccountState,
    category: &str,
) -> Result<i32, String> {
    state.inventory.iter().try_fold(0, |slots, stack| {
        let item = db.item_def.get_by_key(&stack.item_key).ok_or_else(|| {
            format!(
                "account inventory references missing item {}",
                stack.item_key
            )
        })?;
        Ok(if item.category == category {
            slots + 1
        } else {
            slots
        })
    })
}

fn inventory_slots_by_tab(
    db: &GeneratedDatabase,
    state: &AccountState,
) -> Result<HashMap<String, i32>, String> {
    let mut slots = db
        .storage_tab_config
        .rows
        .iter()
        .map(|tab| (tab.tab_key.clone(), 0))
        .collect::<HashMap<_, _>>();

    for stack in &state.inventory {
        let item = db.item_def.get_by_key(&stack.item_key).ok_or_else(|| {
            format!(
                "account inventory references missing item {}",
                stack.item_key
            )
        })?;
        if let Some(tab) = db
            .storage_tab_config
            .rows
            .iter()
            .find(|tab| tab.item_category == item.category)
        {
            *slots.entry(tab.tab_key.clone()).or_insert(0) += 1;
        }
    }

    Ok(slots)
}

fn claim_account_mail(
    project: &DataProject,
    state: &mut AccountState,
    mail_index: usize,
    now_unix: i64,
) -> Result<AccountRewardSettlement, String> {
    if mail_index >= state.mail.len() {
        return Err(format!("unknown mail index {mail_index}"));
    }
    if state.mail[mail_index].expires_at_unix <= now_unix {
        state.mail.remove(mail_index);
        return Err(format!("mail index {mail_index} has expired"));
    }
    let mail = state.mail.remove(mail_index);
    let db = GeneratedDatabase::from_project(project)?;
    let item = db
        .item_def
        .get_by_key(&mail.item_key)
        .ok_or_else(|| format!("mail references missing item {}", mail.item_key))?;
    let reward = RewardStack {
        item_key: item.key.clone(),
        item_name: item.name.clone(),
        category: item.category.clone(),
        rarity: item.rarity.clone(),
        stack_size: item.stack_size.max(1),
        quantity: mail.quantity.max(0),
    };
    apply_rewards_to_account_state(project, state, &[reward], now_unix)
}

fn delete_account_mail(state: &mut AccountState, mail_index: usize) -> Result<AccountMail, String> {
    if mail_index >= state.mail.len() {
        return Err(format!("unknown mail index {mail_index}"));
    }
    Ok(state.mail.remove(mail_index))
}

fn preview_account_energy_recovery(
    project: &DataProject,
    state: &AccountState,
    now_unix: i64,
) -> Result<EnergyRecovery, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let config = db
        .account_energy_config
        .rows
        .first()
        .ok_or_else(|| "missing account energy config".to_string())?;
    let max_energy = config.max_energy.max(0);
    let current = state.energy.clamp(0, max_energy);
    let recover_seconds = config.recover_seconds.max(1);
    let recover_amount = config.recover_amount.max(0);
    let elapsed = (now_unix - state.last_energy_update_unix).max(0);
    let recovered = if current >= max_energy {
        0
    } else {
        ((elapsed / recover_seconds as i64) as i32) * recover_amount
    };
    let after_recovery = (current + recovered).min(max_energy);
    let seconds_until_next_recovery = if after_recovery >= max_energy {
        0
    } else {
        let remainder = elapsed % recover_seconds as i64;
        (recover_seconds as i64 - remainder).max(0)
    };

    Ok(EnergyRecovery {
        current,
        recovered: after_recovery - current,
        after_recovery,
        max_energy,
        recover_seconds,
        recover_amount,
        seconds_until_next_recovery,
    })
}

fn apply_account_energy_recovery(
    project: &DataProject,
    state: &mut AccountState,
    now_unix: i64,
) -> Result<EnergyRecovery, String> {
    let recovery = preview_account_energy_recovery(project, state, now_unix)?;
    if recovery.recovered <= 0 {
        return Ok(recovery);
    }

    state.energy = recovery.after_recovery;
    if state.energy >= recovery.max_energy {
        state.last_energy_update_unix = now_unix;
    } else {
        let recovered_ticks = (recovery.recovered / recovery.recover_amount.max(1)).max(0) as i64;
        state.last_energy_update_unix += recovered_ticks * recovery.recover_seconds as i64;
    }

    Ok(recovery)
}

#[derive(Debug, Clone)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn next_range(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (max - min + 1) as u32;
        min + (self.next_u32() % span) as i32
    }
}

fn print_rewards(rewards: &[RewardStack]) {
    for line in reward_lines(rewards) {
        println!("{line}");
    }
}

fn reward_lines(rewards: &[RewardStack]) -> Vec<String> {
    if rewards.is_empty() {
        return vec!["rewards: none".to_string()];
    }
    let mut lines = vec!["rewards:".to_string()];
    lines.extend(rewards.iter().map(|reward| {
        format!(
            "- {} x{} [{} / {}]",
            reward.item_name, reward.quantity, reward.category, reward.rarity
        )
    }));
    lines
}

fn print_storage_settlement(settlement: &StorageSettlement) {
    for line in storage_settlement_lines(settlement) {
        println!("{line}");
    }
}

fn storage_settlement_lines(settlement: &StorageSettlement) -> Vec<String> {
    let mut lines = vec!["storage:".to_string()];
    lines.extend(settlement.tabs.iter().map(|tab| {
        format!(
            "- {}: +{} item(s), +{} slot(s), occupied {}/{}",
            tab.name,
            tab.placed_quantity,
            tab.used_slots,
            tab.occupied_before + tab.used_slots,
            tab.capacity
        )
    }));
    if settlement.overflow_mail.is_empty() {
        lines.push("overflow mail: none".to_string());
    } else {
        lines.push("overflow mail:".to_string());
        lines.extend(settlement.overflow_mail.iter().map(|reward| {
            format!(
                "- {} x{} [{} / {}, expires in 1 day]",
                reward.item_name, reward.quantity, reward.category, reward.rarity
            )
        }));
    }
    lines
}

fn print_account_settlement(settlement: &AccountRewardSettlement) {
    for line in account_settlement_lines(settlement) {
        println!("{line}");
    }
}

fn account_settlement_lines(settlement: &AccountRewardSettlement) -> Vec<String> {
    let mut lines = vec!["account_state settlement:".to_string()];
    if settlement.placed.is_empty() {
        lines.push("- inventory placed: none".to_string());
    } else {
        lines.extend(settlement.placed.iter().map(|reward| {
            format!(
                "- inventory placed: {} x{} [{}]",
                reward.item_name, reward.quantity, reward.category
            )
        }));
    }
    if settlement.overflow_mail.is_empty() {
        lines.push("- mail added: none".to_string());
    } else {
        lines.extend(settlement.overflow_mail.iter().map(|reward| {
            format!(
                "- mail added: {} x{} [{} / expires in 1 day]",
                reward.item_name, reward.quantity, reward.category
            )
        }));
    }
    let mut slot_lines = settlement
        .inventory_slots
        .iter()
        .map(|(tab, slots)| format!("- inventory slots: {tab}={slots}"))
        .collect::<Vec<_>>();
    slot_lines.sort();
    lines.extend(slot_lines);
    lines
}

fn data_status(args: &[String]) -> Result<(), String> {
    let (project, project_path) = load_project(args)?;
    let schema_hash = project.schema_hash();
    let data_hash = project.data_hash();
    let fingerprints = match project_path.as_ref() {
        Some(path) => project
            .fingerprints_from_dir(path)
            .map_err(|error| error.to_string())?,
        None => ProjectFingerprints {
            schema_hash,
            generated_schema_hash: schema_hash.wrapping_add(1),
            data_hash,
            built_data_hash: data_hash,
        },
    };
    let status = fingerprints.status();

    println!("schema_hash: {}", fingerprints.schema_hash);
    println!(
        "generated_schema_hash: {}",
        fingerprints.generated_schema_hash
    );
    println!("data_hash: {}", fingerprints.data_hash);
    println!("built_data_hash: {}", fingerprints.built_data_hash);
    println!("status: {}", status_label(status));

    let issues = project.validate();
    if issues.is_empty() {
        println!("validation: ok");
    } else {
        println!("validation: {} issue(s)", issues.len());
        for issue in issues {
            println!("- {:?}: {}", issue.severity, issue.message);
        }
    }

    Ok(())
}

fn validate(args: &[String]) -> Result<(), String> {
    let (project, _) = load_project(args)?;
    let issues = project.validate();

    if issues.is_empty() {
        println!("validation: ok");
        Ok(())
    } else {
        println!("validation: {} issue(s)", issues.len());
        for issue in issues {
            println!("- {:?}: {}", issue.severity, issue.message);
        }
        Err("validation failed".to_string())
    }
}

fn view(args: &[String]) -> Result<(), String> {
    let (project, _) = load_project(args)?;
    let view_key = option_value(args, "--view").unwrap_or("map_wave_preview");
    let materialized = project.materialize_view(view_key)?;
    print_table(&materialized.headers, &materialized.rows);
    Ok(())
}

fn codegen_preview(args: &[String]) -> Result<(), String> {
    let (project, _) = load_project(args)?;
    println!("{}", project.generate_rust_structs());
    Ok(())
}

fn codegen(args: &[String]) -> Result<(), String> {
    let (project, project_path) = load_project(args)?;
    let out = option_value(args, "--out")
        .map(PathBuf::from)
        .ok_or_else(|| "missing required --out <dir>".to_string())?;

    fs::create_dir_all(&out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    write_file(
        &out.join("schema_types.rs"),
        &project.generate_rust_structs(),
    )?;
    write_file(
        &out.join("table_accessors.rs"),
        &project.generate_table_accessors(),
    )?;
    write_file(
        &out.join("relation_cache.rs"),
        &project.generate_relation_cache(),
    )?;
    write_file(
        &out.join("lib.rs"),
        "pub mod relation_cache;\npub mod schema_types;\npub mod table_accessors;\n",
    )?;

    if let Some(path) = project_path {
        project
            .write_generated_schema_fingerprint(&path)
            .map_err(|error| error.to_string())?;
    }

    println!("generated Rust files: {}", out.display());
    Ok(())
}

fn data_build(args: &[String]) -> Result<(), String> {
    let (project, project_path) = load_project(args)?;
    let out = option_value(args, "--out")
        .map(PathBuf::from)
        .ok_or_else(|| "missing required --out <dir>".to_string())?;

    fs::create_dir_all(&out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    let snapshot = serde_json::to_string_pretty(&project)
        .map_err(|error| format!("failed to serialize data snapshot: {error}"))?;
    write_file(&out.join("data_snapshot.json"), &snapshot)?;

    if let Some(path) = project_path {
        project
            .write_built_data_fingerprint(&path)
            .map_err(|error| error.to_string())?;
    }

    println!(
        "built data snapshot: {}",
        out.join("data_snapshot.json").display()
    );
    Ok(())
}

fn import_aseprite(args: &[String]) -> Result<(), String> {
    let project_path = option_value(args, "--project")
        .map(PathBuf::from)
        .ok_or_else(|| "missing required --project <dir>".to_string())?;
    let file = option_value(args, "--file")
        .map(PathBuf::from)
        .ok_or_else(|| "missing required --file <path>".to_string())?;
    let summary = aseprite::import_aseprite(&project_path, &file)?;
    println!(
        "imported aseprite: texture={}, frames={}, animations={}",
        summary.texture_key, summary.frame_count, summary.animation_count
    );
    Ok(())
}

fn status_label(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::AllFresh => "all_fresh",
        ProjectStatus::CodegenRequired => "codegen_required",
        ProjectStatus::DataBuildRequired => "data_build_required",
        ProjectStatus::CodegenAndDataBuildRequired => "codegen_and_data_build_required",
    }
}

fn load_project(args: &[String]) -> Result<(DataProject, Option<PathBuf>), String> {
    if let Some(path) = option_value(args, "--project").map(PathBuf::from) {
        let project = DataProject::load_from_dir(&path).map_err(|error| error.to_string())?;
        Ok((project, Some(path)))
    } else {
        Ok((sample_project(), None))
    }
}

fn load_or_create_account_state(
    project: &DataProject,
    path: &Path,
) -> Result<AccountState, String> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        return serde_json::from_str(&content)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()));
    }

    let db = GeneratedDatabase::from_project(project)?;
    let energy_config = db
        .account_energy_config
        .rows
        .first()
        .ok_or_else(|| "missing account energy config".to_string())?;
    Ok(AccountState {
        energy: energy_config.max_energy,
        last_energy_update_unix: 0,
        inventory: Vec::new(),
        mail: Vec::new(),
    })
}

fn save_account_state(path: &Path, state: &AccountState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(state)
        .map_err(|error| format!("failed to serialize account state: {error}"))?;
    write_file(path, &content)
}

fn account_state_path_for_project(project_path: &Path) -> PathBuf {
    project_path.join("account_state.json")
}

pub(crate) fn account_state_snapshot_for_api(
    project_path: &Path,
) -> Result<serde_json::Value, String> {
    let project = DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let path = account_state_path_for_project(project_path);
    let mut state = load_or_create_account_state(&project, &path)?;
    let db = GeneratedDatabase::from_project(&project)?;
    let now_unix = current_unix_time();
    let expired_mail_removed = purge_expired_mail(&mut state, now_unix);
    if expired_mail_removed > 0 {
        save_account_state(&path, &state)?;
    }
    let energy_recovery = preview_account_energy_recovery(&project, &state, now_unix)?;
    let slots = inventory_slots_by_tab(&db, &state)?;
    let inventory = state
        .inventory
        .iter()
        .map(|stack| {
            let item = db.item_def.get_by_key(&stack.item_key);
            serde_json::json!({
                "item_key": stack.item_key,
                "quantity": stack.quantity,
                "name": item.map(|item| item.name.as_str()).unwrap_or(stack.item_key.as_str()),
                "category": item.map(|item| item.category.as_str()).unwrap_or("unknown"),
                "rarity": item.map(|item| item.rarity.as_str()).unwrap_or("unknown"),
                "stack_size": item.map(|item| item.stack_size).unwrap_or(1),
            })
        })
        .collect::<Vec<_>>();
    let mail = state
        .mail
        .iter()
        .enumerate()
        .map(|(index, mail)| {
            let item = db.item_def.get_by_key(&mail.item_key);
            let remaining_seconds = (mail.expires_at_unix - current_unix_time()).clamp(0, 86_400);
            serde_json::json!({
                "index": index,
                "item_key": mail.item_key,
                "quantity": mail.quantity,
                "expires_at_unix": mail.expires_at_unix,
                "remaining_seconds": remaining_seconds,
                "expired": remaining_seconds <= 0,
                "name": item.map(|item| item.name.as_str()).unwrap_or(mail.item_key.as_str()),
                "category": item.map(|item| item.category.as_str()).unwrap_or("unknown"),
                "rarity": item.map(|item| item.rarity.as_str()).unwrap_or("unknown"),
            })
        })
        .collect::<Vec<_>>();
    let storage_tabs = db
        .storage_tab_config
        .rows
        .iter()
        .map(|tab| {
            let used = slots.get(&tab.tab_key).copied().unwrap_or(0);
            serde_json::json!({
                "tab_key": tab.tab_key,
                "name": tab.name,
                "item_category": tab.item_category,
                "capacity": tab.base_capacity.max(0),
                "used_slots": used,
                "free_slots": (tab.base_capacity.max(0) - used).max(0),
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "ok": true,
        "path": path,
        "energy": state.energy,
        "energy_after_recovery": energy_recovery.after_recovery,
        "recoverable_energy": energy_recovery.recovered,
        "max_energy": energy_recovery.max_energy,
        "recover_seconds": energy_recovery.recover_seconds,
        "recover_amount": energy_recovery.recover_amount,
        "seconds_until_next_recovery": energy_recovery.seconds_until_next_recovery,
        "expired_mail_removed": expired_mail_removed,
        "now_unix": now_unix,
        "last_energy_update_unix": state.last_energy_update_unix,
        "inventory": inventory,
        "mail": mail,
        "storage_tabs": storage_tabs,
    }))
}

pub(crate) fn dispatch_account_for_api(
    project_path: &Path,
    map_key: &str,
    seed: u64,
    now_unix: i64,
) -> Result<serde_json::Value, String> {
    let account_path = account_state_path_for_project(project_path);
    let project = DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let mut state = load_or_create_account_state(&project, &account_path)?;
    let expired_mail_removed = purge_expired_mail(&mut state, now_unix);
    if expired_mail_removed > 0 {
        save_account_state(&account_path, &state)?;
    }
    let elapsed_seconds = (now_unix - state.last_energy_update_unix).max(0);
    let args = vec![
        "--project".to_string(),
        project_path.to_string_lossy().to_string(),
        "--map".to_string(),
        map_key.to_string(),
        "--seed".to_string(),
        seed.to_string(),
        "--account-state".to_string(),
        account_path.to_string_lossy().to_string(),
        "--write-account-state".to_string(),
        "--elapsed-seconds".to_string(),
        elapsed_seconds.to_string(),
        "--now-unix".to_string(),
        now_unix.to_string(),
    ];
    simulate(&args)?;
    account_state_snapshot_for_api(project_path)
}

pub(crate) fn claim_mail_for_api(
    project_path: &Path,
    mail_index: usize,
    now_unix: i64,
) -> Result<serde_json::Value, String> {
    let project = DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let path = account_state_path_for_project(project_path);
    let mut state = load_or_create_account_state(&project, &path)?;
    purge_expired_mail(&mut state, now_unix);
    let settlement = claim_account_mail(&project, &mut state, mail_index, now_unix)?;
    save_account_state(&path, &state)?;
    Ok(serde_json::json!({
        "ok": true,
        "message": format!("claimed mail #{mail_index}"),
        "settlement": account_settlement_lines(&settlement),
        "account": account_state_snapshot_for_api(project_path)?,
    }))
}

pub(crate) fn recover_energy_for_api(
    project_path: &Path,
    now_unix: i64,
) -> Result<serde_json::Value, String> {
    let project = DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let path = account_state_path_for_project(project_path);
    let mut state = load_or_create_account_state(&project, &path)?;
    let recovery = apply_account_energy_recovery(&project, &mut state, now_unix)?;
    let expired_mail_removed = purge_expired_mail(&mut state, now_unix);
    save_account_state(&path, &state)?;
    Ok(serde_json::json!({
        "ok": true,
        "message": format!("recovered {} energy", recovery.recovered),
        "recovered": recovery.recovered,
        "expired_mail_removed": expired_mail_removed,
        "account": account_state_snapshot_for_api(project_path)?,
    }))
}

pub(crate) fn delete_mail_for_api(
    project_path: &Path,
    mail_index: usize,
) -> Result<serde_json::Value, String> {
    let project = DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let path = account_state_path_for_project(project_path);
    let mut state = load_or_create_account_state(&project, &path)?;
    purge_expired_mail(&mut state, current_unix_time());
    let removed = delete_account_mail(&mut state, mail_index)?;
    save_account_state(&path, &state)?;
    Ok(serde_json::json!({
        "ok": true,
        "message": format!("deleted mail #{} ({}) x{}", mail_index, removed.item_key, removed.quantity),
        "account": account_state_snapshot_for_api(project_path)?,
    }))
}

fn option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn parse_i32(value: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|error| format!("invalid i32 value {value}: {error}"))
}

fn parse_i64(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|error| format!("invalid i64 value {value}: {error}"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid u64 value {value}: {error}"))
}

fn current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn occupied_slots_from_args(args: &[String]) -> Result<HashMap<String, i32>, String> {
    [
        ("--occupied-material-slots", "material"),
        ("--occupied-equipment-slots", "equipment"),
        ("--occupied-consumable-slots", "consumable"),
    ]
    .into_iter()
    .filter_map(|(flag, tab_key)| {
        option_value(args, flag)
            .map(|value| parse_i32(value).map(|slots| (tab_key.to_string(), slots)))
    })
    .collect()
}

fn div_ceil_i32(value: i32, divisor: i32) -> i32 {
    if value <= 0 {
        return 0;
    }
    (value + divisor.max(1) - 1) / divisor.max(1)
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn print_table(headers: &[String], rows: &[Vec<String>]) {
    let widths = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|value| value.len())
                .max()
                .unwrap_or(0)
                .max(header.len())
        })
        .collect::<Vec<_>>();

    print_row(headers, &widths);
    println!(
        "{}",
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("-+-")
    );

    for row in rows {
        print_row(row, &widths);
    }
}

fn print_row(row: &[String], widths: &[usize]) {
    let cells = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let value = row.get(index).map(String::as_str).unwrap_or("");
            format!("{value:<width$}")
        })
        .collect::<Vec<_>>();
    println!("{}", cells.join(" | "));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project_for_test() -> DataProject {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../projects/sample");
        DataProject::load_from_dir(path).expect("sample project loads")
    }

    #[test]
    fn energy_preview_recovers_before_dispatch() {
        let project = sample_project_for_test();
        let energy =
            energy_preview(&project, "endless_left_road", Some(4), 1200).expect("energy preview");

        assert_eq!(energy.current, 4);
        assert_eq!(energy.after_recovery, 8);
        assert_eq!(energy.cost, 8);
        assert_eq!(energy.after_dispatch(), 0);
    }

    #[test]
    fn account_energy_recovery_applies_elapsed_time() {
        let project = sample_project_for_test();
        let mut state = AccountState {
            energy: 10,
            last_energy_update_unix: 1000,
            inventory: Vec::new(),
            mail: Vec::new(),
        };

        let recovery =
            apply_account_energy_recovery(&project, &mut state, 1900).expect("energy recovery");

        assert_eq!(recovery.recovered, 3);
        assert_eq!(recovery.after_recovery, 13);
        assert_eq!(state.energy, 13);
        assert_eq!(state.last_energy_update_unix, 1900);
    }

    #[test]
    fn account_energy_recovery_caps_at_max() {
        let project = sample_project_for_test();
        let mut state = AccountState {
            energy: 119,
            last_energy_update_unix: 1000,
            inventory: Vec::new(),
            mail: Vec::new(),
        };

        let recovery =
            apply_account_energy_recovery(&project, &mut state, 1900).expect("energy recovery");

        assert_eq!(recovery.recovered, 1);
        assert_eq!(state.energy, 120);
        assert_eq!(state.last_energy_update_unix, 1900);
    }

    #[test]
    fn reward_settlement_is_deterministic() {
        let project = sample_project_for_test();
        let first = settle_rewards(&project, "endless_left_road", 1).expect("first reward");
        let second = settle_rewards(&project, "endless_left_road", 1).expect("second reward");

        assert_eq!(first, second);
        assert!(first.iter().any(|reward| reward.item_key == "slime_gel"));
    }

    #[test]
    fn storage_settlement_places_rewards_into_matching_tabs() {
        let project = sample_project_for_test();
        let rewards = settle_rewards(&project, "endless_left_road", 1).expect("reward");
        let settlement =
            settle_storage(&project, &rewards, &HashMap::new()).expect("storage settlement");

        let material = settlement
            .tabs
            .iter()
            .find(|tab| tab.tab_key == "material")
            .expect("material tab");
        assert!(material.placed_quantity > 0);
        assert!(settlement.overflow_mail.is_empty());
    }

    #[test]
    fn storage_settlement_overflows_to_mail_when_tab_is_full() {
        let project = sample_project_for_test();
        let rewards = vec![RewardStack {
            item_key: "slime_gel".to_string(),
            item_name: "Slime Gel".to_string(),
            category: "material".to_string(),
            rarity: "common".to_string(),
            stack_size: 10,
            quantity: 15,
        }];
        let occupied = HashMap::from([("material".to_string(), 39)]);
        let settlement = settle_storage(&project, &rewards, &occupied).expect("storage settlement");

        let material = settlement
            .tabs
            .iter()
            .find(|tab| tab.tab_key == "material")
            .expect("material tab");
        assert_eq!(material.used_slots, 1);
        assert_eq!(material.placed_quantity, 10);
        assert_eq!(settlement.overflow_mail[0].quantity, 5);
    }

    #[test]
    fn account_state_fills_partial_stack_before_new_slot() {
        let project = sample_project_for_test();
        let mut state = AccountState {
            energy: 100,
            last_energy_update_unix: 0,
            inventory: vec![AccountItemStack {
                item_key: "slime_gel".to_string(),
                quantity: 6,
            }],
            mail: Vec::new(),
        };
        let rewards = vec![RewardStack {
            item_key: "slime_gel".to_string(),
            item_name: "Slime Gel".to_string(),
            category: "material".to_string(),
            rarity: "common".to_string(),
            stack_size: 10,
            quantity: 7,
        }];

        let settlement =
            apply_rewards_to_account_state(&project, &mut state, &rewards, 1000).expect("settle");

        assert_eq!(state.inventory.len(), 2);
        assert_eq!(state.inventory[0].quantity, 10);
        assert_eq!(state.inventory[1].quantity, 3);
        assert_eq!(settlement.placed[0].quantity, 7);
        assert!(settlement.overflow_mail.is_empty());
        assert!(state.mail.is_empty());
    }

    #[test]
    fn account_state_sends_overflow_to_expiring_mail() {
        let project = sample_project_for_test();
        let mut state = AccountState {
            energy: 100,
            last_energy_update_unix: 0,
            inventory: (0..39)
                .map(|_| AccountItemStack {
                    item_key: "slime_gel".to_string(),
                    quantity: 10,
                })
                .collect(),
            mail: Vec::new(),
        };
        let rewards = vec![RewardStack {
            item_key: "slime_gel".to_string(),
            item_name: "Slime Gel".to_string(),
            category: "material".to_string(),
            rarity: "common".to_string(),
            stack_size: 10,
            quantity: 15,
        }];

        let settlement =
            apply_rewards_to_account_state(&project, &mut state, &rewards, 1000).expect("settle");

        assert_eq!(state.inventory.len(), 40);
        assert_eq!(settlement.placed[0].quantity, 10);
        assert_eq!(settlement.overflow_mail[0].quantity, 5);
        assert_eq!(state.mail[0].item_key, "slime_gel");
        assert_eq!(state.mail[0].quantity, 5);
        assert_eq!(state.mail[0].expires_at_unix, 87_400);
    }

    #[test]
    fn account_mail_claim_moves_items_into_inventory() {
        let project = sample_project_for_test();
        let mut state = AccountState {
            energy: 100,
            last_energy_update_unix: 0,
            inventory: Vec::new(),
            mail: vec![AccountMail {
                item_key: "slime_gel".to_string(),
                quantity: 5,
                expires_at_unix: 10_000,
            }],
        };

        let settlement =
            claim_account_mail(&project, &mut state, 0, 1000).expect("claim account mail");

        assert_eq!(settlement.placed[0].quantity, 5);
        assert_eq!(state.inventory[0].item_key, "slime_gel");
        assert_eq!(state.inventory[0].quantity, 5);
        assert!(state.mail.is_empty());
    }

    #[test]
    fn account_mail_delete_removes_entry_without_inventory_change() {
        let mut state = AccountState {
            energy: 100,
            last_energy_update_unix: 0,
            inventory: Vec::new(),
            mail: vec![AccountMail {
                item_key: "energy_tonic".to_string(),
                quantity: 1,
                expires_at_unix: 10_000,
            }],
        };

        let removed = delete_account_mail(&mut state, 0).expect("delete account mail");

        assert_eq!(removed.item_key, "energy_tonic");
        assert!(state.inventory.is_empty());
        assert!(state.mail.is_empty());
    }

    #[test]
    fn expired_mail_is_purged_by_time() {
        let mut state = AccountState {
            energy: 100,
            last_energy_update_unix: 0,
            inventory: Vec::new(),
            mail: vec![
                AccountMail {
                    item_key: "slime_gel".to_string(),
                    quantity: 1,
                    expires_at_unix: 999,
                },
                AccountMail {
                    item_key: "energy_tonic".to_string(),
                    quantity: 1,
                    expires_at_unix: 1001,
                },
            ],
        };

        let removed = purge_expired_mail(&mut state, 1000);

        assert_eq!(removed, 1);
        assert_eq!(state.mail.len(), 1);
        assert_eq!(state.mail[0].item_key, "energy_tonic");
    }
}

pub(crate) fn option_value_for_args<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    option_value(args, flag)
}

pub(crate) fn status_label_for_api(status: ProjectStatus) -> &'static str {
    status_label(status)
}

pub(crate) fn run_codegen_for_api(args: &[String]) -> Result<(), String> {
    codegen(args)
}

pub(crate) fn run_data_build_for_api(args: &[String]) -> Result<(), String> {
    data_build(args)
}

pub(crate) fn simulate_for_api(project: &DataProject, map_key: &str) -> Result<String, String> {
    simulate_to_string(project, map_key)
}
