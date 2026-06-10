use belt_core::{
    BattleConfig, BehaviorCondition, BehaviorRule, BeltPosition, CellOffset, CellPattern,
    CompareOperator, ConditionDef, ConditionKind, ConditionSubject, FacingMode, MapDef, SkillDef,
    SkillDefId, SkillEffect, SkillEffectKind, SkillStatCost, SkillStep, SkillStepOrigin,
    SpecialTriggerDef, StatBlock, StatCompare, StatDefId, UnitDef, UnitDefId, UnitGroup, UnitSpawn,
    WaveDef,
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

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeUnitEquipment {
    pub unit_key: String,
    pub stat_options: Vec<RuntimeStatOption>,
    pub special_option_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeStatOption {
    pub stat_key: String,
    pub value: f32,
}

pub fn battle_config_from_project_with_runtime_equipment(
    project: &DataProject,
    map_key: &str,
    equipment: &[RuntimeUnitEquipment],
) -> Result<BattleConfig, String> {
    let db = GeneratedDatabase::from_project(project)?;
    let cache = GeneratedRelationCache::build(&db)?;
    battle_config_from_generated_with_runtime_equipment(&db, &cache, map_key, equipment)
}

pub fn battle_config_from_generated(
    db: &GeneratedDatabase,
    cache: &GeneratedRelationCache,
    map_key: &str,
) -> Result<BattleConfig, String> {
    battle_config_from_generated_with_runtime_equipment(db, cache, map_key, &[])
}

pub fn battle_config_from_generated_with_runtime_equipment(
    db: &GeneratedDatabase,
    cache: &GeneratedRelationCache,
    map_key: &str,
    equipment: &[RuntimeUnitEquipment],
) -> Result<BattleConfig, String> {
    let mut unit_defs = db
        .unit_def
        .rows
        .iter()
        .map(|row| unit_def_from_data(db, row))
        .collect::<Result<Vec<_>, _>>()?;
    apply_unit_special_option_loadouts(db, &mut unit_defs)?;
    apply_runtime_unit_equipment(db, &mut unit_defs, equipment)?;
    let skill_defs = db
        .skill_def
        .rows
        .iter()
        .map(|row| skill_def_from_data(db, row))
        .collect::<Result<Vec<_>, _>>()?;

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
        skill_defs,
        left_scroll_speed: map.left_scroll_speed,
        wave_spawn_x: map.wave_spawn_x,
        tick_duration: 0.2,
        prepare_ticks: 5,
    })
}

fn skill_def_from_data(
    db: &GeneratedDatabase,
    row: &data_types::SkillDef,
) -> Result<SkillDef, String> {
    let cast_pattern = cell_pattern_from_data(db, row.cast_pattern)?;
    let steps = row
        .steps
        .iter()
        .map(|step_id| skill_step_from_data(db, *step_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SkillDef {
        id: SkillDefId(row.id.0 as u32),
        name: row.name.clone(),
        cooldown_ticks: row.cooldown_ticks.max(1) as u32,
        range: row.range.max(0.0),
        cast_pattern,
        steps,
        costs: row
            .costs
            .iter()
            .map(|cost_id| skill_stat_cost_from_data(db, *cost_id))
            .collect::<Result<Vec<_>, _>>()?,
        target_rule: row.target_rule.clone(),
    })
}

fn skill_stat_cost_from_data(
    db: &GeneratedDatabase,
    row_id: RowId,
) -> Result<SkillStatCost, String> {
    let row = db
        .skill_stat_cost
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing skill stat cost {:?}", row_id))?;
    Ok(SkillStatCost {
        stat: StatDefId(row.stat.0 as u32),
        amount: row.amount.max(0.0),
    })
}

fn skill_step_from_data(db: &GeneratedDatabase, row_id: RowId) -> Result<SkillStep, String> {
    let row = db
        .skill_step
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing skill step {:?}", row_id))?;
    let pattern = cell_pattern_from_data(db, row.pattern)?;
    let effects = row
        .effects
        .iter()
        .map(|effect_id| skill_effect_from_data(db, *effect_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SkillStep {
        tick_offset: row.tick_offset.max(0) as u32,
        origin: match row.origin.as_str() {
            "target" => SkillStepOrigin::Target,
            _ => SkillStepOrigin::Caster,
        },
        pattern,
        effects,
    })
}

fn skill_effect_from_data(db: &GeneratedDatabase, row_id: RowId) -> Result<SkillEffect, String> {
    let row = db
        .skill_effect
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing skill effect {:?}", row_id))?;

    Ok(SkillEffect {
        kind: match row.effect_kind.as_str() {
            "damage" => SkillEffectKind::Damage,
            "projectile_damage" => SkillEffectKind::ProjectileDamage,
            "stat_delta" => SkillEffectKind::StatDelta,
            other => return Err(format!("unsupported skill effect kind {other}")),
        },
        power: row.power,
        scaling: row.scaling,
        knockback_cells: row.knockback_cells.max(0),
        impact_pattern: Some(cell_pattern_from_data(db, row.impact_pattern)?),
        stat_target: match row.stat_target.as_str() {
            "self" => ConditionSubject::SelfUnit,
            "target" => ConditionSubject::Target,
            other => return Err(format!("unsupported stat target {other}")),
        },
        stat: StatDefId(row.stat.0 as u32),
        stat_delta: row.stat_delta,
        stat_duration_ticks: row.stat_duration_ticks.max(0) as u32,
        stat_tick_delta: row.stat_tick_delta,
        trigger_skill: if row.trigger_timing.is_empty() {
            None
        } else {
            Some(SkillDefId(row.trigger_skill.0 as u32))
        },
        trigger_timing: (!row.trigger_timing.is_empty()).then(|| row.trigger_timing.clone()),
    })
}

fn cell_pattern_from_data(db: &GeneratedDatabase, row_id: RowId) -> Result<CellPattern, String> {
    let row = db
        .cell_pattern
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing cell pattern {:?}", row_id))?;
    let cells = row
        .cells
        .iter()
        .map(|cell_id| {
            let cell = db
                .cell_offset
                .get_by_id(*cell_id)
                .ok_or_else(|| format!("missing cell offset {:?}", cell_id))?;
            Ok(CellOffset {
                forward: cell.forward,
                side: cell.side,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(CellPattern {
        id: row.id.0 as u32,
        name: row.name.clone(),
        facing_mode: match row.facing_mode.as_str() {
            "fixed" => FacingMode::Fixed,
            _ => FacingMode::RotateByFacing,
        },
        cells,
    })
}

fn unit_def_from_data(
    db: &GeneratedDatabase,
    row: &data_types::UnitDef,
) -> Result<UnitDef, String> {
    let primary_skill = row.skills.first().copied();
    let skill_cooldown_ticks = primary_skill
        .and_then(|skill_id| db.skill_def.get_by_id(skill_id))
        .map(|skill| skill.cooldown_ticks.max(1) as u32)
        .unwrap_or_else(|| (row.attack_interval / 0.2).ceil().max(1.0) as u32);
    let behavior_rules = row
        .behavior_rules
        .iter()
        .map(|rule_id| behavior_rule_from_data(db, *rule_id))
        .collect::<Result<Vec<_>, _>>()?;
    let base_stats = row
        .base_stats
        .iter()
        .map(|stat_id| unit_base_stat_from_data(db, *stat_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(UnitDef {
        id: UnitDefId(row.id.0 as u32),
        name: row.name.clone(),
        max_hp: row.max_hp,
        attack: row.attack,
        attack_range: row.attack_range,
        attack_interval: row.attack_interval,
        move_speed: row.move_speed,
        primary_skill: primary_skill.map(|skill_id| SkillDefId(skill_id.0 as u32)),
        behavior_rules,
        base_stats: StatBlock::new(base_stats),
        special_triggers: Vec::new(),
        skill_cooldown_ticks,
    })
}

fn apply_unit_special_option_loadouts(
    db: &GeneratedDatabase,
    unit_defs: &mut [UnitDef],
) -> Result<(), String> {
    for loadout in &db.unit_special_option_loadout.rows {
        let unit_def = unit_defs
            .iter_mut()
            .find(|unit| unit.id == UnitDefId(loadout.unit.0 as u32))
            .ok_or_else(|| {
                format!(
                    "missing unit {:?} for special option loadout {}",
                    loadout.unit, loadout.key
                )
            })?;

        for option_id in &loadout.special_options {
            apply_special_option_to_unit(db, unit_def, *option_id)?;
        }
    }

    Ok(())
}

fn apply_special_option_to_unit(
    db: &GeneratedDatabase,
    unit_def: &mut UnitDef,
    option_id: RowId,
) -> Result<(), String> {
    let option = db
        .special_option_def
        .get_by_id(option_id)
        .ok_or_else(|| format!("missing special option {:?}", option_id))?;

    for delta_id in &option.stat_deltas {
        let delta = db
            .special_option_stat_delta
            .get_by_id(*delta_id)
            .ok_or_else(|| format!("missing special option stat delta {:?}", delta_id))?;
        if delta.condition == "on_equip" {
            let stat = StatDefId(delta.stat.0 as u32);
            let current = unit_def.base_stats.get(stat);
            unit_def.base_stats.set(stat, current + delta.value);
        }
    }

    if let Some(skill) = db.skill_def.get_by_id(option.granted_skill) {
        let skill_id = SkillDefId(skill.id.0 as u32);
        let already_available = unit_def.primary_skill == Some(skill_id)
            || unit_def
                .behavior_rules
                .iter()
                .any(|rule| rule.skill == skill_id);
        if !already_available {
            unit_def.behavior_rules.push(BehaviorRule {
                priority: 50,
                skill: skill_id,
                condition: BehaviorCondition::NearestEnemyInCastPattern,
                conditions: Vec::new(),
            });
        }
    }

    if !option.trigger_key.is_empty() {
        let trigger = special_trigger_from_data(db, &option.trigger_key)?;
        if !unit_def
            .special_triggers
            .iter()
            .any(|existing| existing.key == trigger.key)
        {
            unit_def.special_triggers.push(trigger);
        }
    }

    Ok(())
}

fn special_trigger_from_data(
    db: &GeneratedDatabase,
    trigger_key: &str,
) -> Result<SpecialTriggerDef, String> {
    let row = db
        .special_trigger_def
        .get_by_key(trigger_key)
        .ok_or_else(|| format!("missing special trigger {trigger_key}"))?;
    Ok(SpecialTriggerDef {
        key: row.key.clone(),
        interval_seconds: row.interval_seconds.max(0.01),
        stack_stat: StatDefId(row.stack_stat.0 as u32),
        stack_delta: row.stack_delta,
        stack_threshold: row.stack_threshold.max(0.0),
        consume_stacks_on_trigger: row.consume_stacks_on_trigger,
        duration_seconds: row.duration_seconds.max(0.0),
        periodic_interval_seconds: row.periodic_interval_seconds.max(0.0),
        damage_scale: row.damage_scale,
        target_rule: row.target_rule.clone(),
    })
}

fn apply_runtime_unit_equipment(
    db: &GeneratedDatabase,
    unit_defs: &mut [UnitDef],
    equipment: &[RuntimeUnitEquipment],
) -> Result<(), String> {
    for unit_equipment in equipment {
        let unit_row = db
            .unit_def
            .get_by_key(&unit_equipment.unit_key)
            .ok_or_else(|| format!("missing equipped unit {}", unit_equipment.unit_key))?;
        let unit_def = unit_defs
            .iter_mut()
            .find(|unit| unit.id == UnitDefId(unit_row.id.0 as u32))
            .ok_or_else(|| format!("missing unit def {}", unit_equipment.unit_key))?;

        for option in &unit_equipment.stat_options {
            if let Some(stat) = db.stat_def.get_by_key(&option.stat_key) {
                let stat_id = StatDefId(stat.id.0 as u32);
                let current = unit_def.base_stats.get(stat_id);
                unit_def.base_stats.set(stat_id, current + option.value);
            }
        }

        for option_key in &unit_equipment.special_option_keys {
            let option = db
                .special_option_def
                .get_by_key(option_key)
                .ok_or_else(|| format!("missing equipment special option {option_key}"))?;
            apply_special_option_to_unit(db, unit_def, option.id)?;
        }
    }

    Ok(())
}

fn behavior_rule_from_data(db: &GeneratedDatabase, row_id: RowId) -> Result<BehaviorRule, String> {
    let row = db
        .behavior_rule
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing behavior rule {:?}", row_id))?;
    Ok(BehaviorRule {
        priority: row.priority,
        skill: SkillDefId(row.skill.0 as u32),
        condition: match row.condition.as_str() {
            "always" => BehaviorCondition::Always,
            "nearest_enemy_in_cast_pattern" => BehaviorCondition::NearestEnemyInCastPattern,
            other => return Err(format!("unsupported behavior condition {other}")),
        },
        conditions: row
            .conditions
            .iter()
            .map(|condition_id| condition_def_from_data(db, *condition_id))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn unit_base_stat_from_data(
    db: &GeneratedDatabase,
    row_id: RowId,
) -> Result<(StatDefId, f32), String> {
    let row = db
        .unit_base_stat
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing unit base stat {:?}", row_id))?;
    Ok((StatDefId(row.stat.0 as u32), row.value))
}

fn condition_def_from_data(db: &GeneratedDatabase, row_id: RowId) -> Result<ConditionDef, String> {
    let row = db
        .condition_def
        .get_by_id(row_id)
        .ok_or_else(|| format!("missing condition {:?}", row_id))?;
    let other_stat = StatDefId(row.other_stat.0 as u32);
    Ok(ConditionDef {
        kind: match row.condition_kind.as_str() {
            "always" => ConditionKind::Always,
            "nearest_enemy_in_cast_pattern" => ConditionKind::NearestEnemyInCastPattern,
            "stat_compare" => ConditionKind::StatCompare,
            other => return Err(format!("unsupported condition kind {other}")),
        },
        subject: match row.subject.as_str() {
            "target" => ConditionSubject::Target,
            "self" => ConditionSubject::SelfUnit,
            other => return Err(format!("unsupported condition subject {other}")),
        },
        stat: StatDefId(row.stat.0 as u32),
        operator: match row.operator.as_str() {
            "lt" => CompareOperator::Lt,
            "lte" => CompareOperator::Lte,
            "eq" => CompareOperator::Eq,
            "gte" => CompareOperator::Gte,
            "gt" => CompareOperator::Gt,
            other => return Err(format!("unsupported compare operator {other}")),
        },
        compare: match row.compare_mode.as_str() {
            "value" => StatCompare::Value(row.value),
            "stat_ratio" => StatCompare::StatRatio {
                other_stat,
                ratio: row.value,
            },
            "stat" => StatCompare::Stat(other_stat),
            other => return Err(format!("unsupported compare mode {other}")),
        },
    })
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

    #[test]
    fn applies_unit_special_option_loadout_to_battle_config() {
        let project = DataProject::load_from_dir("../../projects/sample").expect("project loads");
        let config =
            battle_config_from_project(&project, "endless_left_road").expect("config loads");
        let knight = config
            .unit_defs
            .iter()
            .find(|unit| unit.id == UnitDefId(1001))
            .expect("knight exists");

        assert_eq!(knight.base_stats.get(StatDefId(23006)), 1.0);
        assert_eq!(knight.primary_skill, Some(SkillDefId(17001)));
        assert_eq!(knight.behavior_rules.len(), 1);
        assert_eq!(
            knight
                .special_triggers
                .iter()
                .map(|trigger| trigger.key.as_str())
                .collect::<Vec<_>>(),
            vec!["combat_tick_5s_moonlight_3"]
        );
    }
}
