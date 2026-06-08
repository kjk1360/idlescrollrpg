use belt_core::{
    sample_battle_config, BattleConfig, BattleEvent, BattleWorld, BeltPosition, MapDef, UnitDef,
    UnitDefId, UnitGroup, UnitSpawn, WaveDef,
};
use data_studio_core::{
    sample_project, CellValue, DataProject, FieldId, ProjectFingerprints, ProjectStatus, RowData,
    RowId, TableData, TableId, TableSchema,
};
use std::fs;
use std::path::{Path, PathBuf};

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
    println!();
    println!("Common options:");
    println!("  --project <dir>  Load a file-based data project");
    println!("  --out <dir>      Output directory for codegen or data-build");
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
        "// Generated table accessors will be expanded in the next phase.\n",
    )?;
    write_file(
        &out.join("relation_cache.rs"),
        "// Generated relation cache will be expanded in the next phase.\n",
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

fn battle_config_from_project(
    project: &DataProject,
    map_key: &str,
) -> Result<BattleConfig, String> {
    let unit_table = table(project, "unit_def")?;
    let group_table = table(project, "unit_group")?;
    let member_table = table(project, "unit_group_member")?;
    let wave_table = table(project, "wave_def")?;
    let map_table = table(project, "map_def")?;

    let unit_rows = table_data(project, unit_table.id)?;
    let unit_defs = unit_rows
        .rows
        .iter()
        .map(|row| unit_def_from_row(unit_table, row))
        .collect::<Result<Vec<_>, _>>()?;

    let map_row = row_by_key(table_data(project, map_table.id)?, map_key)?;
    let party_row_id = cell_row(map_table, map_row, "party")?;
    let party = unit_group_from_row_id(project, group_table, member_table, party_row_id, 0.0)?;
    let wave_row_ids = cell_rows(map_table, map_row, "waves")?;
    let waves = wave_row_ids
        .iter()
        .map(|row_id| wave_from_row_id(project, wave_table, group_table, member_table, *row_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BattleConfig {
        party,
        map: MapDef {
            id: map_row.key.clone(),
            waves,
        },
        unit_defs,
        left_scroll_speed: cell_f32(map_table, map_row, "left_scroll_speed")?,
        wave_spawn_x: cell_f32(map_table, map_row, "wave_spawn_x")?,
    })
}

fn unit_def_from_row(table: &TableSchema, row: &RowData) -> Result<UnitDef, String> {
    Ok(UnitDef {
        id: UnitDefId(row.id.0 as u32),
        name: cell_string(table, row, "name")?,
        max_hp: cell_i32(table, row, "max_hp")?,
        attack: cell_i32(table, row, "attack")?,
        attack_range: cell_f32(table, row, "attack_range")?,
        attack_interval: cell_f32(table, row, "attack_interval")?,
        move_speed: cell_f32(table, row, "move_speed")?,
    })
}

fn wave_from_row_id(
    project: &DataProject,
    wave_table: &TableSchema,
    group_table: &TableSchema,
    member_table: &TableSchema,
    row_id: RowId,
) -> Result<WaveDef, String> {
    let row = row_by_id(table_data(project, wave_table.id)?, row_id)?;
    let enemy_group_ids = cell_rows(wave_table, row, "enemy_groups")?;
    let enemy_groups = enemy_group_ids
        .iter()
        .map(|group_id| unit_group_from_row_id(project, group_table, member_table, *group_id, 0.0))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WaveDef {
        id: row.key.clone(),
        enemy_groups,
    })
}

fn unit_group_from_row_id(
    project: &DataProject,
    group_table: &TableSchema,
    member_table: &TableSchema,
    row_id: RowId,
    start_x: f32,
) -> Result<UnitGroup, String> {
    let row = row_by_id(table_data(project, group_table.id)?, row_id)?;
    let member_ids = cell_rows(group_table, row, "members")?;
    let member_data = table_data(project, member_table.id)?;
    let spawns = member_ids
        .iter()
        .map(|member_id| {
            let member_row = row_by_id(member_data, *member_id)?;
            let unit_id = cell_row(member_table, member_row, "unit")?;
            Ok(UnitSpawn {
                def_id: UnitDefId(unit_id.0 as u32),
                position: BeltPosition {
                    x: start_x + cell_f32(member_table, member_row, "x")?,
                    lane: cell_f32(member_table, member_row, "lane")?,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(UnitGroup {
        id: row.key.clone(),
        spawns,
    })
}

fn table<'a>(project: &'a DataProject, key: &str) -> Result<&'a TableSchema, String> {
    project
        .tables
        .iter()
        .find(|table| table.key == key)
        .ok_or_else(|| format!("missing table {key}"))
}

fn table_data(project: &DataProject, table_id: TableId) -> Result<&TableData, String> {
    project
        .data
        .iter()
        .find(|data| data.table_id == table_id)
        .ok_or_else(|| format!("missing data for table {:?}", table_id))
}

fn row_by_key<'a>(table_data: &'a TableData, key: &str) -> Result<&'a RowData, String> {
    table_data
        .rows
        .iter()
        .find(|row| row.key == key)
        .ok_or_else(|| format!("missing row {key} in table {:?}", table_data.table_id))
}

fn row_by_id(table_data: &TableData, row_id: RowId) -> Result<&RowData, String> {
    table_data
        .rows
        .iter()
        .find(|row| row.id == row_id)
        .ok_or_else(|| {
            format!(
                "missing row {:?} in table {:?}",
                row_id, table_data.table_id
            )
        })
}

fn field_id(table: &TableSchema, key: &str) -> Result<FieldId, String> {
    table
        .fields
        .iter()
        .find(|field| field.key == key)
        .map(|field| field.id)
        .ok_or_else(|| format!("missing field {}.{key}", table.key))
}

fn cell<'a>(table: &TableSchema, row: &'a RowData, key: &str) -> Result<&'a CellValue, String> {
    let field_id = field_id(table, key)?;
    row.cells
        .get(&field_id)
        .ok_or_else(|| format!("missing cell {}.{key} in row {}", table.key, row.key))
}

fn cell_string(table: &TableSchema, row: &RowData, key: &str) -> Result<String, String> {
    match cell(table, row, key)? {
        CellValue::String(value) => Ok(value.clone()),
        value => Err(format!(
            "expected string in {}.{key}, got {value:?}",
            table.key
        )),
    }
}

fn cell_i32(table: &TableSchema, row: &RowData, key: &str) -> Result<i32, String> {
    match cell(table, row, key)? {
        CellValue::I32(value) => Ok(*value),
        value => Err(format!(
            "expected i32 in {}.{key}, got {value:?}",
            table.key
        )),
    }
}

fn cell_f32(table: &TableSchema, row: &RowData, key: &str) -> Result<f32, String> {
    match cell(table, row, key)? {
        CellValue::F32(value) => Ok(*value),
        value => Err(format!(
            "expected f32 in {}.{key}, got {value:?}",
            table.key
        )),
    }
}

fn cell_row(table: &TableSchema, row: &RowData, key: &str) -> Result<RowId, String> {
    match cell(table, row, key)? {
        CellValue::Row(value) => Ok(*value),
        value => Err(format!(
            "expected row in {}.{key}, got {value:?}",
            table.key
        )),
    }
}

fn cell_rows(table: &TableSchema, row: &RowData, key: &str) -> Result<Vec<RowId>, String> {
    match cell(table, row, key)? {
        CellValue::Rows(value) => Ok(value.clone()),
        value => Err(format!(
            "expected rows in {}.{key}, got {value:?}",
            table.key
        )),
    }
}
