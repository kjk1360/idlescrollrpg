use belt_core::{sample_battle_config, BattleEvent, BattleWorld};
use data_studio_core::{sample_project, DataProject, ProjectFingerprints, ProjectStatus};
use game_data_adapter::battle_config_from_project;
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
}

fn simulate(args: &[String]) -> Result<(), String> {
    let config = if option_value(args, "--project").is_some() {
        let (project, _) = load_project(args)?;
        let map_key = option_value(args, "--map").unwrap_or("endless_left_road");
        battle_config_from_project(&project, map_key)?
    } else {
        sample_battle_config()
    };
    let mut world = BattleWorld::new(config);
    let mut wave_clears = 0;
    let mut kills = 0;

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
    Ok(())
}

fn simulate_to_string(project: &DataProject, map_key: &str) -> Result<String, String> {
    let config = battle_config_from_project(project, map_key)?;
    let mut world = BattleWorld::new(config);
    let mut wave_clears = 0;
    let mut kills = 0;
    let mut lines = Vec::new();

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
    Ok(lines.join("\n"))
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
