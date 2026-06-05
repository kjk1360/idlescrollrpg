use belt_core::{sample_battle_config, BattleEvent, BattleWorld};
use data_studio_core::{sample_project, ProjectFingerprints, ProjectStatus};

fn main() {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "simulate" => simulate(),
        "data-status" => data_status(),
        "codegen-preview" => codegen_preview(),
        _ => help(),
    }
}

fn help() {
    println!("belt_tools");
    println!();
    println!("Commands:");
    println!("  simulate         Run the initial endless-left battle simulation");
    println!("  data-status      Print schema/data freshness state");
    println!("  codegen-preview  Print generated Rust struct preview");
}

fn simulate() {
    let mut world = BattleWorld::new(sample_battle_config());
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
}

fn data_status() {
    let project = sample_project();
    let schema_hash = project.schema_hash();
    let data_hash = project.data_hash();
    let generated_schema_hash = schema_hash.wrapping_add(1);
    let built_data_hash = data_hash;
    let status = ProjectFingerprints {
        schema_hash,
        generated_schema_hash,
        data_hash,
        built_data_hash,
    }
    .status();

    println!("schema_hash: {schema_hash}");
    println!("generated_schema_hash: {generated_schema_hash}");
    println!("data_hash: {data_hash}");
    println!("built_data_hash: {built_data_hash}");
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
}

fn codegen_preview() {
    let project = sample_project();
    println!("{}", project.generate_rust_structs());
}

fn status_label(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::AllFresh => "all_fresh",
        ProjectStatus::CodegenRequired => "codegen_required",
        ProjectStatus::DataBuildRequired => "data_build_required",
        ProjectStatus::CodegenAndDataBuildRequired => "codegen_and_data_build_required",
    }
}
