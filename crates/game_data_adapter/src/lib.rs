use belt_core::{
    BattleConfig, BeltPosition, MapDef, UnitDef, UnitDefId, UnitGroup, UnitSpawn, WaveDef,
};
use data_studio_core::{DataProject, RowId};
use generated_data::relation_cache::GeneratedRelationCache;
use generated_data::schema_types as data_types;
use generated_data::table_accessors::GeneratedDatabase;

pub fn battle_config_from_project(
    project: &DataProject,
    map_key: &str,
) -> Result<BattleConfig, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let cache = GeneratedRelationCache::build(&db)?;
    battle_config_from_generated(&db, &cache, map_key)
}

pub fn battle_config_from_generated(
    db: &GeneratedDatabase,
    cache: &GeneratedRelationCache,
    map_key: &str,
) -> Result<BattleConfig, String> {
    let unit_defs = db
        .unit_def
        .rows
        .iter()
        .map(unit_def_from_data)
        .collect::<Vec<_>>();

    let map = db
        .map_def
        .get_by_key(map_key)
        .ok_or_else(|| format!("missing map {map_key}"))?;
    let party_id = cache
        .get_map_def_party(map.id)
        .ok_or_else(|| format!("missing party relation for map {}", map.key))?;
    let party = unit_group_from_data(db, cache, party_id, 0.0)?;
    let wave_row_ids = cache
        .get_map_def_waves(map.id)
        .ok_or_else(|| format!("missing waves relation for map {}", map.key))?;
    let waves = wave_row_ids
        .iter()
        .map(|row_id| wave_from_data(db, cache, *row_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BattleConfig {
        party,
        map: MapDef {
            id: map.key.clone(),
            waves,
        },
        unit_defs,
        left_scroll_speed: map.left_scroll_speed,
        wave_spawn_x: map.wave_spawn_x,
    })
}

fn unit_def_from_data(row: &data_types::UnitDef) -> UnitDef {
    UnitDef {
        id: UnitDefId(row.id.0 as u32),
        name: row.name.clone(),
        max_hp: row.max_hp,
        attack: row.attack,
        attack_range: row.attack_range,
        attack_interval: row.attack_interval,
        move_speed: row.move_speed,
    }
}

fn wave_from_data(
    db: &GeneratedDatabase,
    cache: &GeneratedRelationCache,
    row_id: RowId,
) -> Result<WaveDef, String> {
    let row = db
        .wave_def
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing wave {:?}", row_id))?;
    let enemy_group_ids = cache
        .get_wave_def_enemy_groups(row.id)
        .ok_or_else(|| format!("missing enemy group relation for wave {}", row.key))?;
    let enemy_groups = enemy_group_ids
        .iter()
        .map(|group_id| unit_group_from_data(db, cache, *group_id, 0.0))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WaveDef {
        id: row.key.clone(),
        enemy_groups,
    })
}

fn unit_group_from_data(
    db: &GeneratedDatabase,
    cache: &GeneratedRelationCache,
    row_id: RowId,
    start_x: f32,
) -> Result<UnitGroup, String> {
    let row = db
        .unit_group
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing unit group {:?}", row_id))?;
    let member_ids = cache
        .get_unit_group_members(row.id)
        .ok_or_else(|| format!("missing member relation for group {}", row.key))?;
    let spawns = member_ids
        .iter()
        .map(|member_id| {
            let member = db
                .unit_group_member
                .get_by_id(*member_id)
                .ok_or_else(|| format!("missing unit group member {:?}", member_id))?;
            let unit_id = cache
                .get_unit_group_member_unit(member.id)
                .ok_or_else(|| format!("missing unit relation for member {}", member.key))?;
            Ok(UnitSpawn {
                def_id: UnitDefId(unit_id.0 as u32),
                position: BeltPosition {
                    x: start_x + member.x,
                    lane: member.lane,
                },
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(UnitGroup {
        id: row.key.clone(),
        spawns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_sample_battle_config() {
        let project = DataProject::load_from_dir("../../projects/sample").expect("project loads");
        let config =
            battle_config_from_project(&project, "endless_left_road").expect("config loads");

        assert_eq!(config.map.id, "endless_left_road");
        assert_eq!(config.party.spawns.len(), 2);
        assert_eq!(config.map.waves.len(), 2);
    }
}
