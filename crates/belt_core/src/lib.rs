use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitDefId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillDefId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatDefId(pub u32);

pub const STAT_MAX_HP: StatDefId = StatDefId(23001);
pub const STAT_CURRENT_HP: StatDefId = StatDefId(23002);
pub const STAT_ATTACK: StatDefId = StatDefId(23003);
pub const STAT_CURRENT_MANA: StatDefId = StatDefId(23004);
pub const STAT_BLEED_STACK: StatDefId = StatDefId(23005);
pub const STAT_MOONLIGHT: StatDefId = StatDefId(23006);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
}

impl Team {
    fn is_enemy_of(self, other: Team) -> bool {
        self != other
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltPosition {
    pub x: f32,
    pub lane: f32,
}

impl BeltPosition {
    pub fn distance_to(self, other: BeltPosition) -> f32 {
        let dx = self.x - other.x;
        let dl = self.lane - other.lane;
        (dx * dx + dl * dl).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPosition {
    pub x: i32,
    pub lane: i32,
}

impl GridPosition {
    pub fn to_belt(self) -> BeltPosition {
        BeltPosition {
            x: self.x as f32,
            lane: self.lane as f32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnitDef {
    pub id: UnitDefId,
    pub name: String,
    pub max_hp: i32,
    pub attack: i32,
    pub attack_range: f32,
    pub attack_interval: f32,
    pub move_speed: f32,
    pub primary_skill: Option<SkillDefId>,
    pub behavior_rules: Vec<BehaviorRule>,
    pub base_stats: StatBlock,
    pub special_triggers: Vec<String>,
    pub skill_cooldown_ticks: u32,
}

#[derive(Debug, Clone)]
pub struct BehaviorRule {
    pub priority: i32,
    pub skill: SkillDefId,
    pub condition: BehaviorCondition,
    pub conditions: Vec<ConditionDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorCondition {
    NearestEnemyInCastPattern,
    Always,
}

#[derive(Debug, Clone)]
pub struct ConditionDef {
    pub kind: ConditionKind,
    pub subject: ConditionSubject,
    pub stat: StatDefId,
    pub operator: CompareOperator,
    pub compare: StatCompare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    NearestEnemyInCastPattern,
    StatCompare,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionSubject {
    SelfUnit,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatCompare {
    Value(f32),
    StatRatio { other_stat: StatDefId, ratio: f32 },
    Stat(StatDefId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOperator {
    Lt,
    Lte,
    Eq,
    Gte,
    Gt,
}

#[derive(Debug, Clone, Default)]
pub struct StatBlock {
    values: HashMap<StatDefId, f32>,
}

impl StatBlock {
    pub fn new(values: impl IntoIterator<Item = (StatDefId, f32)>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    pub fn get(&self, stat: StatDefId) -> f32 {
        self.values.get(&stat).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, stat: StatDefId, value: f32) {
        self.values.insert(stat, value);
    }
}

#[derive(Debug, Clone)]
pub struct SkillDef {
    pub id: SkillDefId,
    pub name: String,
    pub cooldown_ticks: u32,
    pub range: f32,
    pub cast_pattern: CellPattern,
    pub steps: Vec<SkillStep>,
    pub costs: Vec<SkillStatCost>,
    pub target_rule: String,
}

#[derive(Debug, Clone)]
pub struct SkillStatCost {
    pub stat: StatDefId,
    pub amount: f32,
}

#[derive(Debug, Clone)]
pub struct SkillStep {
    pub tick_offset: u32,
    pub origin: SkillStepOrigin,
    pub pattern: CellPattern,
    pub effects: Vec<SkillEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStepOrigin {
    Caster,
    Target,
}

#[derive(Debug, Clone)]
pub struct SkillEffect {
    pub kind: SkillEffectKind,
    pub power: i32,
    pub scaling: f32,
    pub knockback_cells: i32,
    pub impact_pattern: Option<CellPattern>,
    pub stat_target: ConditionSubject,
    pub stat: StatDefId,
    pub stat_delta: f32,
    pub stat_duration_ticks: u32,
    pub stat_tick_delta: f32,
    pub trigger_skill: Option<SkillDefId>,
    pub trigger_timing: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillEffectKind {
    Damage,
    ProjectileDamage,
    StatDelta,
}

#[derive(Debug, Clone)]
pub struct CellPattern {
    pub id: u32,
    pub name: String,
    pub facing_mode: FacingMode,
    pub cells: Vec<CellOffset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacingMode {
    RotateByFacing,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellOffset {
    pub forward: i32,
    pub side: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct UnitSpawn {
    pub def_id: UnitDefId,
    pub position: BeltPosition,
}

#[derive(Debug, Clone)]
pub struct UnitGroup {
    pub id: String,
    pub spawns: Vec<UnitSpawn>,
}

#[derive(Debug, Clone)]
pub struct WaveDef {
    pub id: String,
    pub enemy_groups: Vec<UnitGroup>,
}

#[derive(Debug, Clone)]
pub struct MapDef {
    pub id: String,
    pub waves: Vec<WaveDef>,
}

#[derive(Debug, Clone)]
pub struct UnitState {
    pub id: UnitId,
    pub def_id: UnitDefId,
    pub name: String,
    pub team: Team,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub attack_range: f32,
    pub attack_interval: f32,
    pub attack_cooldown: f32,
    pub move_speed: f32,
    pub primary_skill: Option<SkillDefId>,
    pub behavior_rules: Vec<BehaviorRule>,
    pub stats: StatBlock,
    pub special_triggers: Vec<SpecialTriggerState>,
    pub skill_cooldown_ticks: u32,
    pub position: BeltPosition,
    pub grid: GridPosition,
    pub home_grid: GridPosition,
}

#[derive(Debug, Clone)]
pub struct SpecialTriggerState {
    pub key: String,
    pub ticks_until_next: u32,
}

impl UnitState {
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BattleEvent {
    WaveStarted {
        wave_id: String,
    },
    UnitSpawned {
        unit_id: UnitId,
        name: String,
        team: Team,
    },
    UnitMoved {
        unit_id: UnitId,
        x: f32,
        lane: f32,
    },
    UnitAttacked {
        attacker: UnitId,
        target: UnitId,
        damage: i32,
    },
    SkillAreaEffect {
        cells: Vec<GridPosition>,
    },
    ProjectileLaunched {
        caster: UnitId,
        from: GridPosition,
        to: GridPosition,
        duration: f32,
    },
    SpecialTriggered {
        unit_id: UnitId,
        trigger_key: String,
    },
    UnitKilled {
        unit_id: UnitId,
    },
    WaveCleared {
        wave_id: String,
    },
    MapCleared {
        map_id: String,
    },
    MapLooped {
        map_id: String,
        loop_count: u64,
    },
}

#[derive(Debug, Clone)]
pub struct BattleConfig {
    pub party: UnitGroup,
    pub map: MapDef,
    pub unit_defs: Vec<UnitDef>,
    pub skill_defs: Vec<SkillDef>,
    pub left_scroll_speed: f32,
    pub wave_spawn_x: f32,
    pub tick_duration: f32,
    pub prepare_ticks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattlePhase {
    Prepare,
    Engage,
    Clear,
    Defeat,
}

#[derive(Debug)]
pub struct BattleWorld {
    config: BattleConfig,
    units: Vec<UnitState>,
    events: Vec<BattleEvent>,
    active_wave_id: Option<String>,
    pending_waves: VecDeque<WaveDef>,
    next_unit_id: u64,
    phase: BattlePhase,
    tick_accumulator: f32,
    prepare_ticks_remaining: u32,
    pending_skill_steps: Vec<PendingSkillStep>,
    pending_impacts: Vec<PendingImpact>,
    pending_stat_modifiers: Vec<PendingStatModifier>,
    pending_special_periodics: Vec<PendingSpecialPeriodic>,
}

#[derive(Debug, Clone)]
struct PendingSkillStep {
    caster: EffectCaster,
    target_id: UnitId,
    target_position: BeltPosition,
    facing: Direction,
    step: SkillStep,
    ticks_remaining: u32,
}

#[derive(Debug, Clone)]
struct PendingImpact {
    caster: EffectCaster,
    target: UnitId,
    target_position: BeltPosition,
    damage: i32,
    knockback_cells: i32,
    facing: Direction,
    ticks_remaining: u32,
}

#[derive(Debug, Clone)]
struct PendingStatModifier {
    target: UnitId,
    stat: StatDefId,
    expire_delta: f32,
    tick_delta: f32,
    ticks_remaining: u32,
}

#[derive(Debug, Clone)]
struct PendingSpecialPeriodic {
    source: UnitId,
    ticks_until_next: u32,
    ticks_remaining: u32,
    interval_ticks: u32,
    damage_scale: f32,
}

#[derive(Debug, Clone)]
struct EffectCaster {
    id: UnitId,
    attack: i32,
    grid: GridPosition,
    position: BeltPosition,
}

impl From<&UnitState> for EffectCaster {
    fn from(unit: &UnitState) -> Self {
        Self {
            id: unit.id,
            attack: unit.attack,
            grid: unit.grid,
            position: unit.position,
        }
    }
}

impl BattleWorld {
    pub fn new(config: BattleConfig) -> Self {
        let mut world = Self {
            pending_waves: VecDeque::from(config.map.waves.clone()),
            config,
            units: Vec::new(),
            events: Vec::new(),
            active_wave_id: None,
            next_unit_id: 1,
            phase: BattlePhase::Prepare,
            tick_accumulator: 0.0,
            prepare_ticks_remaining: 0,
            pending_skill_steps: Vec::new(),
            pending_impacts: Vec::new(),
            pending_stat_modifiers: Vec::new(),
            pending_special_periodics: Vec::new(),
        };

        let party = world.config.party.clone();
        world.spawn_group(&party, Team::Player, 0.0);
        world.prepare_next_wave();
        world
    }

    pub fn units(&self) -> &[UnitState] {
        &self.units
    }

    pub fn drain_events(&mut self) -> Vec<BattleEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn tick(&mut self, dt: f32) {
        if matches!(self.phase, BattlePhase::Clear | BattlePhase::Defeat) {
            return;
        }
        self.tick_accumulator += dt;
        let step = self.config.tick_duration.max(0.01);
        while self.tick_accumulator >= step {
            self.tick_accumulator -= step;
            self.tick_grid(step);
        }
    }

    fn tick_grid(&mut self, dt: f32) {
        self.cleanup_dead();

        if !self.any_alive(Team::Player) {
            self.phase = BattlePhase::Defeat;
            return;
        }

        if self.phase == BattlePhase::Prepare {
            self.tick_prepare();
            return;
        }

        if !self.any_alive(Team::Enemy) {
            if let Some(wave_id) = self.active_wave_id.take() {
                self.events.push(BattleEvent::WaveCleared { wave_id });
            }
            self.prepare_next_wave();
            return;
        }

        self.tick_pending_skill_steps();
        self.tick_pending_impacts();
        self.tick_pending_stat_modifiers();
        self.tick_special_triggers();
        self.tick_pending_special_periodics();

        let snapshot = self.units.clone();
        let mut actions = Vec::new();

        for index in 0..self.units.len() {
            if !self.units[index].is_alive() {
                continue;
            }

            self.units[index].attack_cooldown = (self.units[index].attack_cooldown - dt).max(0.0);

            let Some(target) = closest_target(&snapshot, &self.units[index]) else {
                continue;
            };
            let Some(skill) = self.select_skill_for(&self.units[index], &target) else {
                if !line_in_range(
                    self.units[index].position,
                    target.position,
                    self.units[index].attack_range,
                ) {
                    self.move_toward(index, target.position, dt);
                }
                continue;
            };

            if line_in_range(self.units[index].position, target.position, skill.range) {
                if self.units[index].attack_cooldown <= 0.0 {
                    actions.push((self.units[index].id, target.id, skill.id));
                    self.units[index].attack_cooldown =
                        skill.cooldown_ticks.max(1) as f32 * self.config.tick_duration;
                }
            } else {
                self.move_toward(index, target.position, dt);
            }
        }

        for (caster, target, skill_id) in actions {
            self.execute_skill(caster, target, skill_id);
        }
    }

    fn any_alive(&self, team: Team) -> bool {
        self.units
            .iter()
            .any(|unit| unit.team == team && unit.is_alive())
    }

    fn cleanup_dead(&mut self) {
        self.units.retain(UnitState::is_alive);
    }

    fn tick_prepare(&mut self) {
        for index in 0..self.units.len() {
            if !self.units[index].is_alive() {
                continue;
            }
            let target = self.units[index].home_grid.to_belt();
            if line_distance(self.units[index].position, target) > 0.05 {
                self.move_toward(index, target, self.config.tick_duration);
            } else {
                self.units[index].position = target;
                self.units[index].grid = grid_from_belt(target, 0.0);
                self.emit_move(index);
            }
        }

        if self.prepare_ticks_remaining > 0 {
            self.prepare_ticks_remaining -= 1;
        }

        let ready = self
            .units
            .iter()
            .filter(|unit| unit.is_alive())
            .all(|unit| line_distance(unit.position, unit.home_grid.to_belt()) <= 0.05);
        if ready && self.prepare_ticks_remaining == 0 {
            self.phase = BattlePhase::Engage;
            if let Some(wave_id) = &self.active_wave_id {
                self.events.push(BattleEvent::WaveStarted {
                    wave_id: wave_id.clone(),
                });
            }
        }
    }

    fn prepare_next_wave(&mut self) {
        if self.pending_waves.is_empty() {
            self.phase = BattlePhase::Clear;
            self.events.push(BattleEvent::MapCleared {
                map_id: self.config.map.id.clone(),
            });
            return;
        }

        if let Some(wave) = self.pending_waves.pop_front() {
            self.active_wave_id = Some(wave.id.clone());
            self.phase = BattlePhase::Prepare;
            self.prepare_ticks_remaining = self.config.prepare_ticks;
            self.pending_skill_steps.clear();
            self.pending_impacts.clear();
            self.pending_stat_modifiers.clear();
            self.pending_special_periodics.clear();
            self.reset_party_home_grids();
            for group in &wave.enemy_groups {
                self.spawn_group(group, Team::Enemy, self.config.wave_spawn_x);
            }
        }
    }

    fn reset_party_home_grids(&mut self) {
        let homes = self
            .config
            .party
            .spawns
            .iter()
            .map(|spawn| grid_from_belt(spawn.position, 0.0))
            .collect::<Vec<_>>();
        let mut player_index = 0;
        for unit in self
            .units
            .iter_mut()
            .filter(|unit| unit.team == Team::Player && unit.is_alive())
        {
            if let Some(home) = homes.get(player_index).copied() {
                unit.home_grid = home;
            }
            player_index += 1;
        }
    }

    fn spawn_group(&mut self, group: &UnitGroup, team: Team, x_offset: f32) {
        for spawn in &group.spawns {
            let def = self
                .config
                .unit_defs
                .iter()
                .find(|def| def.id == spawn.def_id)
                .unwrap_or_else(|| panic!("missing unit def {:?}", spawn.def_id));

            let unit_id = UnitId(self.next_unit_id);
            self.next_unit_id += 1;
            let home_grid = grid_from_belt(spawn.position, x_offset);
            let mut stats = def.base_stats.clone();
            if stats.get(STAT_MAX_HP) <= 0.0 {
                stats.set(STAT_MAX_HP, def.max_hp as f32);
            }
            if stats.get(STAT_CURRENT_HP) <= 0.0 {
                stats.set(STAT_CURRENT_HP, stats.get(STAT_MAX_HP));
            }
            if stats.get(STAT_ATTACK) <= 0.0 {
                stats.set(STAT_ATTACK, def.attack as f32);
            }
            let state = UnitState {
                id: unit_id,
                def_id: def.id,
                name: def.name.clone(),
                team,
                hp: stats.get(STAT_CURRENT_HP).round() as i32,
                max_hp: stats.get(STAT_MAX_HP).round() as i32,
                attack: stats.get(STAT_ATTACK).round() as i32,
                attack_range: def.attack_range,
                attack_interval: def.attack_interval,
                attack_cooldown: 0.0,
                move_speed: def.move_speed,
                primary_skill: def.primary_skill,
                behavior_rules: def.behavior_rules.clone(),
                stats,
                special_triggers: def
                    .special_triggers
                    .iter()
                    .map(|key| SpecialTriggerState {
                        key: key.clone(),
                        ticks_until_next: special_trigger_interval_ticks(
                            key,
                            self.config.tick_duration,
                        ),
                    })
                    .collect(),
                skill_cooldown_ticks: def.skill_cooldown_ticks,
                position: home_grid.to_belt(),
                grid: home_grid,
                home_grid,
            };
            self.events.push(BattleEvent::UnitSpawned {
                unit_id,
                name: state.name.clone(),
                team,
            });
            self.units.push(state);
        }
    }

    fn move_toward(&mut self, index: usize, target: BeltPosition, dt: f32) {
        let current = self.units[index].position;
        let dx = target.x - current.x;
        let distance = dx.abs();
        if distance > 0.001 {
            let max_step = (self.units[index].move_speed * dt).max(0.05);
            let step = max_step.min(distance);
            self.units[index].position.x += dx.signum() * step;
        }
        self.units[index].grid = grid_from_belt(self.units[index].position, 0.0);
        self.emit_move(index);
    }

    fn emit_move(&mut self, index: usize) {
        self.events.push(BattleEvent::UnitMoved {
            unit_id: self.units[index].id,
            x: self.units[index].position.x,
            lane: self.units[index].position.lane,
        });
    }

    fn primary_skill_for(&self, unit: &UnitState) -> Option<&SkillDef> {
        let skill_id = unit.primary_skill?;
        self.config
            .skill_defs
            .iter()
            .find(|skill| skill.id == skill_id)
    }

    fn select_skill_for(&self, unit: &UnitState, target: &UnitState) -> Option<&SkillDef> {
        let facing = direction_toward(unit.grid, target.grid);
        let mut rules = unit.behavior_rules.iter().collect::<Vec<_>>();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in rules {
            let Some(skill) = self.skill_by_id(rule.skill) else {
                continue;
            };
            if !can_pay_skill_cost(unit, skill) {
                continue;
            }
            if !line_in_range(unit.position, target.position, skill.range) {
                continue;
            }
            if self.behavior_rule_matches(rule, unit, target, facing, skill) {
                return Some(skill);
            }
        }

        self.primary_skill_for(unit)
            .filter(|skill| can_pay_skill_cost(unit, skill))
            .filter(|skill| line_in_range(unit.position, target.position, skill.range))
    }

    fn skill_by_id(&self, skill_id: SkillDefId) -> Option<&SkillDef> {
        self.config
            .skill_defs
            .iter()
            .find(|skill| skill.id == skill_id)
    }

    fn behavior_rule_matches(
        &self,
        rule: &BehaviorRule,
        unit: &UnitState,
        target: &UnitState,
        facing: Direction,
        skill: &SkillDef,
    ) -> bool {
        if !rule.conditions.is_empty() {
            return rule
                .conditions
                .iter()
                .all(|condition| condition_matches(condition, unit, target, facing, skill));
        }

        behavior_condition_matches(rule.condition, unit.grid, target.grid, facing, skill)
    }

    fn execute_skill(&mut self, caster_id: UnitId, target_id: UnitId, skill_id: SkillDefId) {
        let Some(skill) = self
            .config
            .skill_defs
            .iter()
            .find(|skill| skill.id == skill_id)
            .cloned()
        else {
            return;
        };
        let Some(caster) = self.units.iter().find(|unit| unit.id == caster_id).cloned() else {
            return;
        };
        let Some(target) = self.units.iter().find(|unit| unit.id == target_id).cloned() else {
            return;
        };
        if !self.pay_skill_cost(caster_id, &skill) {
            return;
        }
        let facing = direction_toward(caster.grid, target.grid);
        let caster = EffectCaster::from(&caster);

        for step in &skill.steps {
            if step.tick_offset > 0 {
                self.pending_skill_steps.push(PendingSkillStep {
                    caster: caster.clone(),
                    target_id,
                    target_position: target.position,
                    facing,
                    step: step.clone(),
                    ticks_remaining: step.tick_offset,
                });
                continue;
            }
            self.execute_skill_step(&caster, target_id, target.position, facing, step);
        }
    }

    fn execute_skill_step(
        &mut self,
        caster: &EffectCaster,
        target_id: UnitId,
        target_position: BeltPosition,
        facing: Direction,
        step: &SkillStep,
    ) {
        for effect in &step.effects {
            self.apply_effect(caster, target_id, target_position, facing, effect);
        }
    }

    fn apply_effect(
        &mut self,
        caster: &EffectCaster,
        target_id: UnitId,
        target_position: BeltPosition,
        facing: Direction,
        effect: &SkillEffect,
    ) {
        let damage = (effect.power as f32 + caster.attack as f32 * effect.scaling).round() as i32;
        match effect.kind {
            SkillEffectKind::Damage => {
                self.events.push(BattleEvent::SkillAreaEffect {
                    cells: vec![grid_from_belt(target_position, 0.0)],
                });
                self.damage_unit(caster.id, target_id, damage);
                if effect.knockback_cells > 0 {
                    self.knockback_unit(target_id, facing, effect.knockback_cells);
                }
            }
            SkillEffectKind::ProjectileDamage => {
                let distance = line_distance(caster.position, target_position).ceil() as u32;
                let travel_ticks = distance.max(1);
                self.events.push(BattleEvent::ProjectileLaunched {
                    caster: caster.id,
                    from: caster.grid,
                    to: grid_from_belt(target_position, 0.0),
                    duration: travel_ticks as f32 * self.config.tick_duration,
                });
                self.pending_impacts.push(PendingImpact {
                    caster: caster.clone(),
                    target: target_id,
                    target_position,
                    damage,
                    knockback_cells: effect.knockback_cells.max(0),
                    facing,
                    ticks_remaining: travel_ticks,
                });
            }
            SkillEffectKind::StatDelta => {
                self.events.push(BattleEvent::SkillAreaEffect {
                    cells: vec![grid_from_belt(target_position, 0.0)],
                });
                match effect.stat_target {
                    ConditionSubject::SelfUnit => {
                        self.apply_stat_effect(caster.id, effect);
                    }
                    ConditionSubject::Target => {
                        self.apply_stat_effect(target_id, effect);
                    }
                }
            }
        }
    }

    fn tick_pending_skill_steps(&mut self) {
        let mut ready = Vec::new();
        let mut pending = Vec::new();
        for mut step in self.pending_skill_steps.drain(..) {
            step.ticks_remaining = step.ticks_remaining.saturating_sub(1);
            if step.ticks_remaining == 0 {
                ready.push(step);
            } else {
                pending.push(step);
            }
        }
        self.pending_skill_steps = pending;

        for step in ready {
            self.execute_skill_step(
                &step.caster,
                step.target_id,
                step.target_position,
                step.facing,
                &step.step,
            );
        }
    }

    fn tick_pending_impacts(&mut self) {
        let mut ready = Vec::new();
        let mut pending = Vec::new();
        for mut impact in self.pending_impacts.drain(..) {
            impact.ticks_remaining = impact.ticks_remaining.saturating_sub(1);
            if impact.ticks_remaining == 0 {
                ready.push(impact);
            } else {
                pending.push(impact);
            }
        }
        self.pending_impacts = pending;

        for impact in ready {
            self.events.push(BattleEvent::SkillAreaEffect {
                cells: vec![grid_from_belt(impact.target_position, 0.0)],
            });
            self.damage_unit(impact.caster.id, impact.target, impact.damage);
            if impact.knockback_cells > 0 {
                self.knockback_unit(impact.target, impact.facing, impact.knockback_cells);
            }
        }
    }

    fn tick_pending_stat_modifiers(&mut self) {
        let mut pending = Vec::new();
        for mut modifier in self.pending_stat_modifiers.drain(..) {
            let target_alive = self
                .units
                .iter()
                .any(|unit| unit.id == modifier.target && unit.is_alive());
            if !target_alive {
                continue;
            }
            if modifier.tick_delta != 0.0 {
                apply_stat_delta_to_units(
                    &mut self.units,
                    modifier.target,
                    modifier.stat,
                    modifier.tick_delta,
                );
            }
            modifier.ticks_remaining = modifier.ticks_remaining.saturating_sub(1);
            if modifier.ticks_remaining == 0 {
                if modifier.expire_delta != 0.0 {
                    apply_stat_delta_to_units(
                        &mut self.units,
                        modifier.target,
                        modifier.stat,
                        modifier.expire_delta,
                    );
                }
            } else {
                pending.push(modifier);
            }
        }
        self.pending_stat_modifiers = pending;
    }

    fn tick_special_triggers(&mut self) {
        let mut started_periodics = Vec::new();
        for index in 0..self.units.len() {
            if !self.units[index].is_alive() {
                continue;
            }

            for trigger_index in 0..self.units[index].special_triggers.len() {
                let key = self.units[index].special_triggers[trigger_index]
                    .key
                    .clone();
                if key != "combat_tick_5s_moonlight_3" {
                    continue;
                }

                let trigger = &mut self.units[index].special_triggers[trigger_index];
                trigger.ticks_until_next = trigger.ticks_until_next.saturating_sub(1);
                if trigger.ticks_until_next > 0 {
                    continue;
                }
                trigger.ticks_until_next =
                    special_trigger_interval_ticks(&key, self.config.tick_duration);

                let current = self.units[index].stats.get(STAT_MOONLIGHT) + 1.0;
                self.units[index].stats.set(STAT_MOONLIGHT, current);
                if current + f32::EPSILON < 3.0 {
                    continue;
                }

                self.units[index].stats.set(STAT_MOONLIGHT, 0.0);
                started_periodics.push((self.units[index].id, key));
            }
        }

        for (unit_id, key) in started_periodics {
            self.events.push(BattleEvent::SpecialTriggered {
                unit_id,
                trigger_key: key,
            });
            self.pending_special_periodics.push(PendingSpecialPeriodic {
                source: unit_id,
                ticks_until_next: 0,
                ticks_remaining: seconds_to_ticks(10.0, self.config.tick_duration),
                interval_ticks: seconds_to_ticks(0.5, self.config.tick_duration),
                damage_scale: 1.0,
            });
        }
    }

    fn tick_pending_special_periodics(&mut self) {
        let snapshot = self.units.clone();
        let mut pending = Vec::new();
        let mut attacks = Vec::new();

        for mut periodic in self.pending_special_periodics.drain(..) {
            let Some(source) = snapshot
                .iter()
                .find(|unit| unit.id == periodic.source && unit.is_alive())
            else {
                continue;
            };
            periodic.ticks_remaining = periodic.ticks_remaining.saturating_sub(1);
            periodic.ticks_until_next = periodic.ticks_until_next.saturating_sub(1);

            if periodic.ticks_until_next == 0 {
                if let Some(target) = closest_target(&snapshot, source) {
                    let damage = ((source.attack as f32) * periodic.damage_scale).round() as i32;
                    attacks.push((source.id, target.id, damage));
                }
                periodic.ticks_until_next = periodic.interval_ticks.max(1);
            }

            if periodic.ticks_remaining > 0 {
                pending.push(periodic);
            }
        }

        self.pending_special_periodics = pending;
        for (source, target, damage) in attacks {
            if let Some(target_unit) = self.units.iter().find(|unit| unit.id == target) {
                self.events.push(BattleEvent::SkillAreaEffect {
                    cells: vec![target_unit.grid],
                });
            }
            self.damage_unit(source, target, damage);
        }
    }

    fn damage_unit(&mut self, attacker: UnitId, target: UnitId, damage: i32) {
        if let Some(target_unit) = self.units.iter_mut().find(|unit| unit.id == target) {
            if target_unit.is_alive() {
                target_unit.hp = (target_unit.hp - damage).max(0);
                target_unit
                    .stats
                    .set(STAT_CURRENT_HP, target_unit.hp as f32);
                self.events.push(BattleEvent::UnitAttacked {
                    attacker,
                    target,
                    damage,
                });
                if target_unit.hp == 0 {
                    self.events
                        .push(BattleEvent::UnitKilled { unit_id: target });
                }
            }
        }
    }

    fn apply_stat_delta(&mut self, target: UnitId, stat: StatDefId, delta: f32) {
        apply_stat_delta_to_units(&mut self.units, target, stat, delta);
    }

    fn apply_stat_effect(&mut self, target: UnitId, effect: &SkillEffect) {
        self.apply_stat_delta(target, effect.stat, effect.stat_delta);
        if effect.stat_duration_ticks > 0 {
            self.pending_stat_modifiers.push(PendingStatModifier {
                target,
                stat: effect.stat,
                expire_delta: -effect.stat_delta,
                tick_delta: effect.stat_tick_delta,
                ticks_remaining: effect.stat_duration_ticks,
            });
        }
    }

    fn pay_skill_cost(&mut self, caster: UnitId, skill: &SkillDef) -> bool {
        let Some(unit) = self.units.iter_mut().find(|unit| unit.id == caster) else {
            return false;
        };
        if !can_pay_skill_cost(unit, skill) {
            return false;
        }
        for cost in &skill.costs {
            let next = unit.stats.get(cost.stat) - cost.amount;
            unit.stats.set(cost.stat, next.max(0.0));
        }
        sync_legacy_fields_from_stats(unit);
        true
    }

    fn knockback_unit(&mut self, target: UnitId, facing: Direction, cells: i32) {
        let Some(index) = self.units.iter().position(|unit| unit.id == target) else {
            return;
        };
        if !self.units[index].is_alive() {
            return;
        }
        let direction = match facing {
            Direction::Left => -1.0,
            Direction::Right => 1.0,
            Direction::Up | Direction::Down => 0.0,
        };
        if direction != 0.0 {
            self.units[index].position.x += direction * cells.max(0) as f32;
            self.units[index].grid = grid_from_belt(self.units[index].position, 0.0);
        }
        self.emit_move(index);
    }
}

fn closest_target(units: &[UnitState], actor: &UnitState) -> Option<UnitState> {
    units
        .iter()
        .filter(|unit| unit.is_alive() && actor.team.is_enemy_of(unit.team))
        .min_by(|a, b| {
            line_distance(actor.position, a.position)
                .partial_cmp(&line_distance(actor.position, b.position))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn grid_from_belt(position: BeltPosition, x_offset: f32) -> GridPosition {
    GridPosition {
        x: (position.x + x_offset).round() as i32,
        lane: lane_to_grid(position.lane),
    }
}

fn lane_to_grid(lane: f32) -> i32 {
    if lane < -0.33 {
        -1
    } else if lane > 0.33 {
        1
    } else {
        0
    }
}

fn line_distance(actor: BeltPosition, target: BeltPosition) -> f32 {
    (actor.x - target.x).abs()
}

fn line_in_range(actor: BeltPosition, target: BeltPosition, range: f32) -> bool {
    line_distance(actor, target) <= range.max(0.0)
}

fn behavior_condition_matches(
    condition: BehaviorCondition,
    actor: GridPosition,
    target: GridPosition,
    _facing: Direction,
    skill: &SkillDef,
) -> bool {
    match condition {
        BehaviorCondition::NearestEnemyInCastPattern => {
            line_in_range(actor.to_belt(), target.to_belt(), skill.range)
        }
        BehaviorCondition::Always => true,
    }
}

fn condition_matches(
    condition: &ConditionDef,
    actor: &UnitState,
    target: &UnitState,
    _facing: Direction,
    skill: &SkillDef,
) -> bool {
    match condition.kind {
        ConditionKind::NearestEnemyInCastPattern => {
            line_in_range(actor.position, target.position, skill.range)
        }
        ConditionKind::Always => true,
        ConditionKind::StatCompare => {
            let subject = match condition.subject {
                ConditionSubject::SelfUnit => actor,
                ConditionSubject::Target => target,
            };
            let left = subject.stats.get(condition.stat);
            let right = match condition.compare {
                StatCompare::Value(value) => value,
                StatCompare::Stat(other_stat) => subject.stats.get(other_stat),
                StatCompare::StatRatio { other_stat, ratio } => {
                    subject.stats.get(other_stat) * ratio
                }
            };
            compare_values(left, condition.operator, right)
        }
    }
}

fn apply_stat_delta_to_units(units: &mut [UnitState], target: UnitId, stat: StatDefId, delta: f32) {
    if let Some(unit) = units.iter_mut().find(|unit| unit.id == target) {
        let value = unit.stats.get(stat) + delta;
        unit.stats.set(stat, value);
        sync_legacy_fields_from_stats(unit);
    }
}

fn special_trigger_interval_ticks(key: &str, tick_duration: f32) -> u32 {
    match key {
        "combat_tick_5s_moonlight_3" => seconds_to_ticks(5.0, tick_duration),
        _ => 1,
    }
}

fn seconds_to_ticks(seconds: f32, tick_duration: f32) -> u32 {
    (seconds / tick_duration.max(0.01)).round().max(1.0) as u32
}

fn can_pay_skill_cost(unit: &UnitState, skill: &SkillDef) -> bool {
    skill
        .costs
        .iter()
        .all(|cost| unit.stats.get(cost.stat) + f32::EPSILON >= cost.amount)
}

fn sync_legacy_fields_from_stats(unit: &mut UnitState) {
    unit.max_hp = unit.stats.get(STAT_MAX_HP).round().max(1.0) as i32;
    unit.hp = unit
        .stats
        .get(STAT_CURRENT_HP)
        .round()
        .clamp(0.0, unit.max_hp as f32) as i32;
    unit.attack = unit.stats.get(STAT_ATTACK).round().max(0.0) as i32;
    unit.stats.set(STAT_CURRENT_HP, unit.hp as f32);
    unit.stats.set(STAT_MAX_HP, unit.max_hp as f32);
    unit.stats.set(STAT_ATTACK, unit.attack as f32);
}

fn compare_values(left: f32, operator: CompareOperator, right: f32) -> bool {
    match operator {
        CompareOperator::Lt => left < right,
        CompareOperator::Lte => left <= right,
        CompareOperator::Eq => (left - right).abs() <= f32::EPSILON,
        CompareOperator::Gte => left >= right,
        CompareOperator::Gt => left > right,
    }
}

fn direction_toward(actor: GridPosition, target: GridPosition) -> Direction {
    let dx = target.x - actor.x;
    let dl = target.lane - actor.lane;
    if dx.abs() >= dl.abs() {
        if dx >= 0 {
            Direction::Right
        } else {
            Direction::Left
        }
    } else if dl >= 0 {
        Direction::Up
    } else {
        Direction::Down
    }
}

#[cfg(test)]
fn pattern_cells(
    pattern: &CellPattern,
    origin: GridPosition,
    facing: Direction,
) -> Vec<GridPosition> {
    pattern
        .cells
        .iter()
        .map(|cell| apply_cell_offset(origin, *cell, pattern.facing_mode, facing))
        .collect()
}

#[cfg(test)]
fn apply_cell_offset(
    origin: GridPosition,
    offset: CellOffset,
    facing_mode: FacingMode,
    facing: Direction,
) -> GridPosition {
    if facing_mode == FacingMode::Fixed {
        return GridPosition {
            x: origin.x + offset.forward,
            lane: origin.lane + offset.side,
        };
    }

    match facing {
        Direction::Right => GridPosition {
            x: origin.x + offset.forward,
            lane: origin.lane + offset.side,
        },
        Direction::Left => GridPosition {
            x: origin.x - offset.forward,
            lane: origin.lane - offset.side,
        },
        Direction::Up => GridPosition {
            x: origin.x - offset.side,
            lane: origin.lane + offset.forward,
        },
        Direction::Down => GridPosition {
            x: origin.x + offset.side,
            lane: origin.lane - offset.forward,
        },
    }
}

pub fn sample_battle_config() -> BattleConfig {
    let in_cast_pattern = ConditionDef {
        kind: ConditionKind::NearestEnemyInCastPattern,
        subject: ConditionSubject::Target,
        stat: STAT_CURRENT_HP,
        operator: CompareOperator::Gte,
        compare: StatCompare::Value(0.0),
    };
    let melee_pattern = CellPattern {
        id: 20001,
        name: "Melee Forward 1".to_string(),
        facing_mode: FacingMode::RotateByFacing,
        cells: vec![CellOffset {
            forward: 1,
            side: 0,
        }],
    };
    let line_pattern = CellPattern {
        id: 20002,
        name: "Line Forward 5".to_string(),
        facing_mode: FacingMode::RotateByFacing,
        cells: (1..=5)
            .map(|forward| CellOffset { forward, side: 0 })
            .collect(),
    };
    let impact_pattern = CellPattern {
        id: 20003,
        name: "Impact 3x3".to_string(),
        facing_mode: FacingMode::Fixed,
        cells: (-1..=1)
            .flat_map(|forward| (-1..=1).map(move |side| CellOffset { forward, side }))
            .collect(),
    };
    let knight_skill = SkillDef {
        id: SkillDefId(17001),
        name: "Knight Slash".to_string(),
        cooldown_ticks: 5,
        range: 1.5,
        cast_pattern: melee_pattern.clone(),
        steps: vec![
            SkillStep {
                tick_offset: 0,
                origin: SkillStepOrigin::Caster,
                pattern: melee_pattern.clone(),
                effects: vec![SkillEffect {
                    kind: SkillEffectKind::Damage,
                    power: 18,
                    scaling: 1.0,
                    knockback_cells: 0,
                    impact_pattern: Some(melee_pattern.clone()),
                    stat_target: ConditionSubject::Target,
                    stat: STAT_CURRENT_HP,
                    stat_delta: 0.0,
                    stat_duration_ticks: 0,
                    stat_tick_delta: 0.0,
                    trigger_skill: None,
                    trigger_timing: None,
                }],
            },
            SkillStep {
                tick_offset: 1,
                origin: SkillStepOrigin::Target,
                pattern: impact_pattern.clone(),
                effects: vec![SkillEffect {
                    kind: SkillEffectKind::Damage,
                    power: 5,
                    scaling: 0.0,
                    knockback_cells: 0,
                    impact_pattern: Some(impact_pattern.clone()),
                    stat_target: ConditionSubject::Target,
                    stat: STAT_CURRENT_HP,
                    stat_delta: 0.0,
                    stat_duration_ticks: 0,
                    stat_tick_delta: 0.0,
                    trigger_skill: None,
                    trigger_timing: None,
                }],
            },
        ],
        costs: Vec::new(),
        target_rule: "nearest_enemy".to_string(),
    };
    let archer_skill = SkillDef {
        id: SkillDefId(17002),
        name: "Arrow Shot".to_string(),
        cooldown_ticks: 4,
        range: 5.0,
        cast_pattern: line_pattern.clone(),
        steps: vec![SkillStep {
            tick_offset: 0,
            origin: SkillStepOrigin::Caster,
            pattern: line_pattern,
            effects: vec![SkillEffect {
                kind: SkillEffectKind::ProjectileDamage,
                power: 12,
                scaling: 1.0,
                knockback_cells: 0,
                impact_pattern: Some(impact_pattern.clone()),
                stat_target: ConditionSubject::Target,
                stat: STAT_CURRENT_HP,
                stat_delta: 0.0,
                stat_duration_ticks: 0,
                stat_tick_delta: 0.0,
                trigger_skill: None,
                trigger_timing: None,
            }],
        }],
        costs: Vec::new(),
        target_rule: "nearest_enemy".to_string(),
    };
    let slime_skill = SkillDef {
        id: SkillDefId(17003),
        name: "Slime Tackle".to_string(),
        cooldown_ticks: 6,
        range: 1.5,
        cast_pattern: melee_pattern.clone(),
        steps: vec![SkillStep {
            tick_offset: 0,
            origin: SkillStepOrigin::Caster,
            pattern: melee_pattern.clone(),
            effects: vec![
                SkillEffect {
                    kind: SkillEffectKind::Damage,
                    power: 8,
                    scaling: 1.0,
                    knockback_cells: 0,
                    impact_pattern: Some(melee_pattern.clone()),
                    stat_target: ConditionSubject::Target,
                    stat: STAT_CURRENT_HP,
                    stat_delta: 0.0,
                    stat_duration_ticks: 0,
                    stat_tick_delta: 0.0,
                    trigger_skill: None,
                    trigger_timing: None,
                },
                SkillEffect {
                    kind: SkillEffectKind::StatDelta,
                    power: 0,
                    scaling: 0.0,
                    knockback_cells: 0,
                    impact_pattern: Some(melee_pattern.clone()),
                    stat_target: ConditionSubject::Target,
                    stat: STAT_BLEED_STACK,
                    stat_delta: 1.0,
                    stat_duration_ticks: 0,
                    stat_tick_delta: 0.0,
                    trigger_skill: None,
                    trigger_timing: None,
                },
            ],
        }],
        costs: Vec::new(),
        target_rule: "nearest_enemy".to_string(),
    };
    let knight = UnitDef {
        id: UnitDefId(1),
        name: "Knight".to_string(),
        max_hp: 120,
        attack: 18,
        attack_range: 1.3,
        attack_interval: 1.0,
        move_speed: 2.4,
        primary_skill: Some(SkillDefId(17001)),
        behavior_rules: vec![BehaviorRule {
            priority: 100,
            skill: SkillDefId(17001),
            condition: BehaviorCondition::NearestEnemyInCastPattern,
            conditions: vec![in_cast_pattern.clone()],
        }],
        base_stats: StatBlock::new([
            (STAT_MAX_HP, 120.0),
            (STAT_CURRENT_HP, 120.0),
            (STAT_ATTACK, 18.0),
        ]),
        special_triggers: Vec::new(),
        skill_cooldown_ticks: 5,
    };
    let archer = UnitDef {
        id: UnitDefId(2),
        name: "Archer".to_string(),
        max_hp: 70,
        attack: 12,
        attack_range: 5.0,
        attack_interval: 0.8,
        move_speed: 2.1,
        primary_skill: Some(SkillDefId(17002)),
        behavior_rules: vec![BehaviorRule {
            priority: 100,
            skill: SkillDefId(17002),
            condition: BehaviorCondition::NearestEnemyInCastPattern,
            conditions: vec![in_cast_pattern.clone()],
        }],
        base_stats: StatBlock::new([
            (STAT_MAX_HP, 70.0),
            (STAT_CURRENT_HP, 70.0),
            (STAT_ATTACK, 12.0),
        ]),
        special_triggers: Vec::new(),
        skill_cooldown_ticks: 4,
    };
    let slime = UnitDef {
        id: UnitDefId(100),
        name: "Slime".to_string(),
        max_hp: 45,
        attack: 8,
        attack_range: 1.0,
        attack_interval: 1.2,
        move_speed: 1.5,
        primary_skill: Some(SkillDefId(17003)),
        behavior_rules: vec![BehaviorRule {
            priority: 100,
            skill: SkillDefId(17003),
            condition: BehaviorCondition::NearestEnemyInCastPattern,
            conditions: vec![in_cast_pattern],
        }],
        base_stats: StatBlock::new([
            (STAT_MAX_HP, 45.0),
            (STAT_CURRENT_HP, 45.0),
            (STAT_ATTACK, 8.0),
        ]),
        special_triggers: Vec::new(),
        skill_cooldown_ticks: 6,
    };

    BattleConfig {
        party: UnitGroup {
            id: "party_start".to_string(),
            spawns: vec![
                UnitSpawn {
                    def_id: knight.id,
                    position: BeltPosition { x: 0.0, lane: -0.6 },
                },
                UnitSpawn {
                    def_id: archer.id,
                    position: BeltPosition { x: 0.8, lane: 0.6 },
                },
            ],
        },
        map: MapDef {
            id: "endless_left_road".to_string(),
            waves: vec![
                WaveDef {
                    id: "wave_001".to_string(),
                    enemy_groups: vec![UnitGroup {
                        id: "slime_pair".to_string(),
                        spawns: vec![
                            UnitSpawn {
                                def_id: slime.id,
                                position: BeltPosition { x: 0.0, lane: -0.5 },
                            },
                            UnitSpawn {
                                def_id: slime.id,
                                position: BeltPosition { x: 1.2, lane: 0.5 },
                            },
                        ],
                    }],
                },
                WaveDef {
                    id: "wave_002".to_string(),
                    enemy_groups: vec![UnitGroup {
                        id: "slime_line".to_string(),
                        spawns: vec![
                            UnitSpawn {
                                def_id: slime.id,
                                position: BeltPosition { x: 0.0, lane: -0.8 },
                            },
                            UnitSpawn {
                                def_id: slime.id,
                                position: BeltPosition { x: 0.7, lane: 0.0 },
                            },
                            UnitSpawn {
                                def_id: slime.id,
                                position: BeltPosition { x: 1.4, lane: 0.8 },
                            },
                        ],
                    }],
                },
            ],
        },
        unit_defs: vec![knight, archer, slime],
        skill_defs: vec![knight_skill, archer_skill, slime_skill],
        left_scroll_speed: 1.8,
        wave_spawn_x: -8.0,
        tick_duration: 0.2,
        prepare_ticks: 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_battle_clears_at_least_one_wave() {
        let mut world = BattleWorld::new(sample_battle_config());

        for _ in 0..80 {
            world.tick(0.2);
        }

        let events = world.drain_events();
        assert!(events.iter().any(
            |event| matches!(event, BattleEvent::WaveCleared { wave_id } if wave_id == "wave_001")
        ));
        assert!(world.units().iter().any(|unit| unit.team == Team::Player));
    }

    #[test]
    fn cell_pattern_rotates_forward_and_side_offsets() {
        let pattern = CellPattern {
            id: 1,
            name: "test".to_string(),
            facing_mode: FacingMode::RotateByFacing,
            cells: vec![CellOffset {
                forward: 2,
                side: 1,
            }],
        };
        let origin = GridPosition { x: 10, lane: 0 };

        assert_eq!(
            pattern_cells(&pattern, origin, Direction::Right),
            vec![GridPosition { x: 12, lane: 1 }]
        );
        assert_eq!(
            pattern_cells(&pattern, origin, Direction::Left),
            vec![GridPosition { x: 8, lane: -1 }]
        );
        assert_eq!(
            pattern_cells(&pattern, origin, Direction::Up),
            vec![GridPosition { x: 9, lane: 2 }]
        );
        assert_eq!(
            pattern_cells(&pattern, origin, Direction::Down),
            vec![GridPosition { x: 11, lane: -2 }]
        );
    }

    #[test]
    fn projectile_damage_hits_single_line_target_after_travel() {
        let mut world = BattleWorld::new(sample_battle_config());
        let mut saw_projectile = false;
        let mut saw_archer_damage = false;

        for _ in 0..120 {
            world.tick(0.2);
            for event in world.drain_events() {
                match event {
                    BattleEvent::ProjectileLaunched { .. } => {
                        saw_projectile = true;
                    }
                    BattleEvent::UnitAttacked {
                        attacker: UnitId(2),
                        ..
                    } => {
                        saw_archer_damage = true;
                    }
                    _ => {}
                }
            }
        }

        assert!(saw_projectile);
        assert!(saw_archer_damage);
    }

    #[test]
    fn delayed_skill_steps_execute_after_tick_offset() {
        let mut world = BattleWorld::new(sample_battle_config());
        let mut saw_delayed_attack = false;
        let mut last_knight_attack_tick = None;

        for tick in 0..120 {
            world.tick(0.2);
            for event in world.drain_events() {
                if let BattleEvent::UnitAttacked {
                    attacker: UnitId(1),
                    ..
                } = event
                {
                    if let Some(previous_tick) = last_knight_attack_tick {
                        if tick == previous_tick + 1 {
                            saw_delayed_attack = true;
                            break;
                        }
                    }
                    last_knight_attack_tick = Some(tick);
                }
            }
            if saw_delayed_attack {
                break;
            }
        }

        assert!(saw_delayed_attack);
    }

    #[test]
    fn behavior_rules_choose_highest_priority_matching_skill() {
        let mut config = sample_battle_config();
        let knight = config
            .unit_defs
            .iter_mut()
            .find(|def| def.id == UnitDefId(1))
            .expect("knight exists");
        knight.behavior_rules = vec![BehaviorRule {
            priority: 200,
            skill: SkillDefId(17002),
            condition: BehaviorCondition::NearestEnemyInCastPattern,
            conditions: vec![ConditionDef {
                kind: ConditionKind::NearestEnemyInCastPattern,
                subject: ConditionSubject::Target,
                stat: STAT_CURRENT_HP,
                operator: CompareOperator::Gte,
                compare: StatCompare::Value(0.0),
            }],
        }];

        let mut world = BattleWorld::new(config);
        let mut saw_knight_projectile = false;
        for _ in 0..120 {
            world.tick(0.2);
            for event in world.drain_events() {
                if matches!(
                    event,
                    BattleEvent::ProjectileLaunched {
                        caster: UnitId(1),
                        ..
                    }
                ) {
                    saw_knight_projectile = true;
                }
            }
        }

        assert!(saw_knight_projectile);
    }

    #[test]
    fn behavior_rules_can_check_self_stat_ratio() {
        let mut config = sample_battle_config();
        let knight = config
            .unit_defs
            .iter_mut()
            .find(|def| def.id == UnitDefId(1))
            .expect("knight exists");
        knight.base_stats.set(STAT_CURRENT_HP, 40.0);
        knight.behavior_rules = vec![BehaviorRule {
            priority: 300,
            skill: SkillDefId(17002),
            condition: BehaviorCondition::Always,
            conditions: vec![ConditionDef {
                kind: ConditionKind::StatCompare,
                subject: ConditionSubject::SelfUnit,
                stat: STAT_CURRENT_HP,
                operator: CompareOperator::Lt,
                compare: StatCompare::StatRatio {
                    other_stat: STAT_MAX_HP,
                    ratio: 0.5,
                },
            }],
        }];

        let mut world = BattleWorld::new(config);
        let knight = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned");
        assert_eq!(knight.hp, 40);

        let mut saw_knight_projectile = false;
        for _ in 0..120 {
            world.tick(0.2);
            for event in world.drain_events() {
                if matches!(
                    event,
                    BattleEvent::ProjectileLaunched {
                        caster: UnitId(1),
                        ..
                    }
                ) {
                    saw_knight_projectile = true;
                }
            }
        }

        assert!(saw_knight_projectile);
    }

    #[test]
    fn stat_delta_effect_changes_target_stat() {
        let mut world = BattleWorld::new(sample_battle_config());
        let mut saw_bleed_stack = false;

        for _ in 0..160 {
            world.tick(0.2);
            world.drain_events();
            saw_bleed_stack = world
                .units()
                .iter()
                .filter(|unit| unit.team == Team::Player)
                .any(|unit| unit.stats.get(STAT_BLEED_STACK) >= 1.0);
            if saw_bleed_stack {
                break;
            }
        }

        assert!(saw_bleed_stack);
    }

    #[test]
    fn skill_stat_cost_is_paid_before_skill_executes() {
        let mut config = sample_battle_config();
        let knight = config
            .unit_defs
            .iter_mut()
            .find(|def| def.id == UnitDefId(1))
            .expect("knight exists");
        knight.base_stats.set(STAT_CURRENT_MANA, 1.0);
        let arrow = config
            .skill_defs
            .iter_mut()
            .find(|skill| skill.id == SkillDefId(17002))
            .expect("arrow exists");
        arrow.costs = vec![SkillStatCost {
            stat: STAT_CURRENT_MANA,
            amount: 1.0,
        }];

        let mut world = BattleWorld::new(config);
        world.drain_events();

        world.execute_skill(UnitId(1), UnitId(3), SkillDefId(17002));
        let first_projectiles = world
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event, BattleEvent::ProjectileLaunched { .. }))
            .count();
        let knight_mana_after_first = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned")
            .stats
            .get(STAT_CURRENT_MANA);

        world.execute_skill(UnitId(1), UnitId(3), SkillDefId(17002));
        let second_projectiles = world
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event, BattleEvent::ProjectileLaunched { .. }))
            .count();

        assert_eq!(first_projectiles, 1);
        assert_eq!(knight_mana_after_first, 0.0);
        assert_eq!(second_projectiles, 0);
    }

    #[test]
    fn timed_stat_delta_expires() {
        let mut world = BattleWorld::new(sample_battle_config());
        world.apply_stat_effect(
            UnitId(1),
            &SkillEffect {
                kind: SkillEffectKind::StatDelta,
                power: 0,
                scaling: 0.0,
                knockback_cells: 0,
                impact_pattern: None,
                stat_target: ConditionSubject::SelfUnit,
                stat: STAT_BLEED_STACK,
                stat_delta: 3.0,
                stat_duration_ticks: 2,
                stat_tick_delta: 0.0,
                trigger_skill: None,
                trigger_timing: None,
            },
        );

        let bleed_after_apply = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned")
            .stats
            .get(STAT_BLEED_STACK);
        assert_eq!(bleed_after_apply, 3.0);

        world.tick_pending_stat_modifiers();
        let bleed_after_one_tick = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned")
            .stats
            .get(STAT_BLEED_STACK);
        assert_eq!(bleed_after_one_tick, 3.0);

        world.tick_pending_stat_modifiers();
        let bleed_after_expire = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned")
            .stats
            .get(STAT_BLEED_STACK);
        assert_eq!(bleed_after_expire, 0.0);
    }

    #[test]
    fn timed_stat_modifier_can_apply_tick_delta() {
        let mut world = BattleWorld::new(sample_battle_config());
        world.apply_stat_effect(
            UnitId(1),
            &SkillEffect {
                kind: SkillEffectKind::StatDelta,
                power: 0,
                scaling: 0.0,
                knockback_cells: 0,
                impact_pattern: None,
                stat_target: ConditionSubject::SelfUnit,
                stat: STAT_CURRENT_MANA,
                stat_delta: 0.0,
                stat_duration_ticks: 2,
                stat_tick_delta: 1.0,
                trigger_skill: None,
                trigger_timing: None,
            },
        );

        world.tick_pending_stat_modifiers();
        world.tick_pending_stat_modifiers();

        let mana_after_ticks = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned")
            .stats
            .get(STAT_CURRENT_MANA);
        assert_eq!(mana_after_ticks, 2.0);
    }

    #[test]
    fn moonlight_special_trigger_starts_periodic_attacks_at_three_stacks() {
        let mut config = sample_battle_config();
        let knight = config
            .unit_defs
            .iter_mut()
            .find(|def| def.id == UnitDefId(1))
            .expect("knight exists");
        knight.special_triggers = vec!["combat_tick_5s_moonlight_3".to_string()];
        knight.base_stats.set(STAT_MOONLIGHT, 2.0);

        let mut world = BattleWorld::new(config);
        let knight = world
            .units
            .iter_mut()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned");
        knight.special_triggers[0].ticks_until_next = 1;
        let mut saw_trigger = false;
        let mut saw_periodic_attack = false;

        for _ in 0..35 {
            world.tick(0.2);
            for event in world.drain_events() {
                match event {
                    BattleEvent::SpecialTriggered {
                        unit_id: UnitId(1),
                        trigger_key,
                    } if trigger_key == "combat_tick_5s_moonlight_3" => {
                        saw_trigger = true;
                    }
                    BattleEvent::UnitAttacked {
                        attacker: UnitId(1),
                        damage,
                        ..
                    } if saw_trigger && damage == 18 => {
                        saw_periodic_attack = true;
                    }
                    _ => {}
                }
            }
        }

        let knight = world
            .units()
            .iter()
            .find(|unit| unit.id == UnitId(1))
            .expect("knight spawned");
        assert_eq!(knight.stats.get(STAT_MOONLIGHT), 0.0);
        assert!(saw_trigger);
        assert!(saw_periodic_attack);
    }
}
