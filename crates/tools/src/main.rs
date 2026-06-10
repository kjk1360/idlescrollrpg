use belt_core::{sample_battle_config, BattleEvent, BattleWorld};
use data_studio_core::{sample_project, DataProject, ProjectFingerprints, ProjectStatus};
use game_data_adapter::battle_config_from_project;
use generated_data::relation_cache::GeneratedRelationCache;
use generated_data::table_accessors::GeneratedDatabase;
use std::fs;
use std::path::{Path, PathBuf};

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
}

fn simulate(args: &[String]) -> Result<(), String> {
    let map_key = option_value(args, "--map").unwrap_or("endless_left_road");
    let current_energy = option_value(args, "--current-energy")
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
    let project_for_rewards = if option_value(args, "--project").is_some() {
        let (project, _) = load_project(args)?;
        Some(project)
    } else {
        None
    };
    let config = if let Some(project) = &project_for_rewards {
        battle_config_from_project(project, map_key)?
    } else {
        sample_battle_config()
    };
    if let Some(project) = &project_for_rewards {
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
    if let Some(project) = &project_for_rewards {
        let rewards = if map_cleared {
            settle_rewards(project, map_key, seed)?
        } else {
            Vec::new()
        };
        print_rewards(&rewards);
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
    } else {
        lines.push("rewards: none".to_string());
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
    quantity: i32,
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
            quantity,
        });
    }

    Ok(rewards)
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

fn option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
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
    fn reward_settlement_is_deterministic() {
        let project = sample_project_for_test();
        let first = settle_rewards(&project, "endless_left_road", 1).expect("first reward");
        let second = settle_rewards(&project, "endless_left_road", 1).expect("second reward");

        assert_eq!(first, second);
        assert!(first.iter().any(|reward| reward.item_key == "slime_gel"));
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
