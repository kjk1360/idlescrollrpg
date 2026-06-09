use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitDefId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillDefId(pub u32);

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
    pub skill_cooldown_ticks: u32,
}

#[derive(Debug, Clone)]
pub struct SkillDef {
    pub id: SkillDefId,
    pub name: String,
    pub cooldown_ticks: u32,
    pub cast_pattern: CellPattern,
    pub steps: Vec<SkillStep>,
    pub target_rule: String,
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
    pub trigger_skill: Option<SkillDefId>,
    pub trigger_timing: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillEffectKind {
    Damage,
    ProjectileDamage,
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
    pub skill_cooldown_ticks: u32,
    pub position: BeltPosition,
    pub grid: GridPosition,
    pub home_grid: GridPosition,
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
    pending_impacts: Vec<PendingImpact>,
}

#[derive(Debug, Clone)]
struct PendingImpact {
    caster_id: UnitId,
    caster_team: Team,
    cells: Vec<GridPosition>,
    damage: i32,
    knockback_cells: i32,
    facing: Direction,
    ticks_remaining: u32,
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
            pending_impacts: Vec::new(),
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

        self.tick_pending_impacts();

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
            let Some(skill) = self.primary_skill_for(&self.units[index]) else {
                if !grid_in_range(
                    self.units[index].grid,
                    target.grid,
                    self.units[index].attack_range,
                ) {
                    self.move_toward(index, target.grid);
                }
                continue;
            };

            let facing = direction_toward(self.units[index].grid, target.grid);
            if pattern_contains(
                &skill.cast_pattern,
                self.units[index].grid,
                facing,
                target.grid,
            ) {
                if self.units[index].attack_cooldown <= 0.0 {
                    actions.push((self.units[index].id, target.id, skill.id));
                    self.units[index].attack_cooldown =
                        skill.cooldown_ticks.max(1) as f32 * self.config.tick_duration;
                }
            } else {
                self.move_toward(index, target.grid);
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
        let occupied = self.occupied_cells();
        for index in 0..self.units.len() {
            if !self.units[index].is_alive() {
                continue;
            }
            let target = self.units[index].home_grid;
            if self.units[index].grid != target {
                self.move_toward_with_occupied(index, target, &occupied);
            } else {
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
            .all(|unit| unit.grid == unit.home_grid);
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
            self.pending_impacts.clear();
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
            let state = UnitState {
                id: unit_id,
                def_id: def.id,
                name: def.name.clone(),
                team,
                hp: def.max_hp,
                max_hp: def.max_hp,
                attack: def.attack,
                attack_range: def.attack_range,
                attack_interval: def.attack_interval,
                attack_cooldown: 0.0,
                move_speed: def.move_speed,
                primary_skill: def.primary_skill,
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

    fn occupied_cells(&self) -> HashMap<GridPosition, UnitId> {
        self.units
            .iter()
            .filter(|unit| unit.is_alive())
            .map(|unit| (unit.grid, unit.id))
            .collect()
    }

    fn move_toward(&mut self, index: usize, target: GridPosition) {
        let occupied = self.occupied_cells();
        self.move_toward_with_occupied(index, target, &occupied);
    }

    fn move_toward_with_occupied(
        &mut self,
        index: usize,
        target: GridPosition,
        occupied: &HashMap<GridPosition, UnitId>,
    ) {
        let steps = self.units[index].move_speed.floor().max(1.0) as i32;
        let unit_id = self.units[index].id;
        let mut blocked = occupied
            .iter()
            .filter_map(|(position, occupant)| (*occupant != unit_id).then_some(*position))
            .collect::<HashSet<_>>();

        for _ in 0..steps {
            let current = self.units[index].grid;
            if current == target {
                break;
            }
            let Some(next) = next_step_toward(current, target, &blocked) else {
                break;
            };
            blocked.remove(&current);
            blocked.insert(next);
            self.units[index].grid = next;
            self.units[index].position = next.to_belt();
        }
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
        let facing = direction_toward(caster.grid, target.grid);

        for step in skill.steps.iter().filter(|step| step.tick_offset == 0) {
            let origin = match step.origin {
                SkillStepOrigin::Caster => caster.grid,
                SkillStepOrigin::Target => target.grid,
            };
            let cells = pattern_cells(&step.pattern, origin, facing)
                .into_iter()
                .collect::<HashSet<_>>();
            for effect in &step.effects {
                self.apply_effect(&caster, target.grid, facing, &cells, effect);
            }
        }
    }

    fn apply_effect(
        &mut self,
        caster: &UnitState,
        target_grid: GridPosition,
        facing: Direction,
        cells: &HashSet<GridPosition>,
        effect: &SkillEffect,
    ) {
        let damage = (effect.power as f32 + caster.attack as f32 * effect.scaling).round() as i32;
        match effect.kind {
            SkillEffectKind::Damage => {
                self.events.push(BattleEvent::SkillAreaEffect {
                    cells: cells.iter().copied().collect(),
                });
                let targets = self
                    .units
                    .iter()
                    .filter(|unit| unit.is_alive() && caster.team.is_enemy_of(unit.team))
                    .filter(|unit| cells.contains(&unit.grid))
                    .map(|unit| unit.id)
                    .collect::<Vec<_>>();

                for target in targets {
                    self.damage_unit(caster.id, target, damage);
                    if effect.knockback_cells > 0 {
                        self.knockback_unit(target, facing, effect.knockback_cells);
                    }
                }
            }
            SkillEffectKind::ProjectileDamage => {
                let distance = caster.grid.x.abs_diff(target_grid.x) as u32
                    + caster.grid.lane.abs_diff(target_grid.lane) as u32;
                let travel_ticks = distance.max(1);
                self.events.push(BattleEvent::ProjectileLaunched {
                    caster: caster.id,
                    from: caster.grid,
                    to: target_grid,
                    duration: travel_ticks as f32 * self.config.tick_duration,
                });
                self.pending_impacts.push(PendingImpact {
                    caster_id: caster.id,
                    caster_team: caster.team,
                    cells: vec![target_grid],
                    damage,
                    knockback_cells: effect.knockback_cells.max(0),
                    facing,
                    ticks_remaining: travel_ticks,
                });
            }
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
                cells: impact.cells.clone(),
            });
            let cell_set = impact.cells.iter().copied().collect::<HashSet<_>>();
            let targets = self
                .units
                .iter()
                .filter(|unit| unit.is_alive() && impact.caster_team.is_enemy_of(unit.team))
                .filter(|unit| cell_set.contains(&unit.grid))
                .map(|unit| unit.id)
                .collect::<Vec<_>>();

            for target in targets {
                self.damage_unit(impact.caster_id, target, impact.damage);
                if impact.knockback_cells > 0 {
                    self.knockback_unit(target, impact.facing, impact.knockback_cells);
                }
            }
        }
    }

    fn damage_unit(&mut self, attacker: UnitId, target: UnitId, damage: i32) {
        if let Some(target_unit) = self.units.iter_mut().find(|unit| unit.id == target) {
            if target_unit.is_alive() {
                target_unit.hp = (target_unit.hp - damage).max(0);
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

    fn knockback_unit(&mut self, target: UnitId, facing: Direction, cells: i32) {
        let Some(index) = self.units.iter().position(|unit| unit.id == target) else {
            return;
        };
        if !self.units[index].is_alive() {
            return;
        }
        let mut occupied = self.occupied_cells();
        occupied.remove(&self.units[index].grid);
        let mut next = self.units[index].grid;
        for _ in 0..cells {
            let candidate = step_in_direction(next, facing);
            if candidate.lane < -1 || candidate.lane > 1 || occupied.contains_key(&candidate) {
                break;
            }
            next = candidate;
        }
        if next != self.units[index].grid {
            self.units[index].grid = next;
            self.units[index].position = next.to_belt();
            self.emit_move(index);
        }
    }
}

fn closest_target(units: &[UnitState], actor: &UnitState) -> Option<UnitState> {
    units
        .iter()
        .filter(|unit| unit.is_alive() && actor.team.is_enemy_of(unit.team))
        .min_by(|a, b| {
            actor
                .grid
                .x
                .abs_diff(a.grid.x)
                .cmp(&actor.grid.x.abs_diff(b.grid.x))
                .then_with(|| {
                    actor
                        .grid
                        .lane
                        .abs_diff(a.grid.lane)
                        .cmp(&actor.grid.lane.abs_diff(b.grid.lane))
                })
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

fn grid_in_range(actor: GridPosition, target: GridPosition, range: f32) -> bool {
    let range = range.ceil().max(1.0) as i32;
    (actor.x - target.x).abs() <= range && (actor.lane - target.lane).abs() <= range
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

fn pattern_contains(
    pattern: &CellPattern,
    origin: GridPosition,
    facing: Direction,
    target: GridPosition,
) -> bool {
    pattern_cells(pattern, origin, facing).contains(&target)
}

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

fn step_in_direction(position: GridPosition, direction: Direction) -> GridPosition {
    match direction {
        Direction::Left => GridPosition {
            x: position.x - 1,
            lane: position.lane,
        },
        Direction::Right => GridPosition {
            x: position.x + 1,
            lane: position.lane,
        },
        Direction::Up => GridPosition {
            x: position.x,
            lane: position.lane + 1,
        },
        Direction::Down => GridPosition {
            x: position.x,
            lane: position.lane - 1,
        },
    }
}

fn next_step_toward(
    current: GridPosition,
    target: GridPosition,
    blocked: &HashSet<GridPosition>,
) -> Option<GridPosition> {
    let x_dir = (target.x - current.x).signum();
    let lane_dir = (target.lane - current.lane).signum();
    let candidates = [
        GridPosition {
            x: current.x + x_dir,
            lane: current.lane,
        },
        GridPosition {
            x: current.x,
            lane: (current.lane + lane_dir).clamp(-1, 1),
        },
        GridPosition {
            x: current.x + x_dir,
            lane: (current.lane + lane_dir).clamp(-1, 1),
        },
    ];
    candidates
        .into_iter()
        .filter(|position| *position != current)
        .filter(|position| position.lane >= -1 && position.lane <= 1)
        .find(|position| !blocked.contains(position))
}

pub fn sample_battle_config() -> BattleConfig {
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
    let knight_skill = SkillDef {
        id: SkillDefId(17001),
        name: "Knight Slash".to_string(),
        cooldown_ticks: 5,
        cast_pattern: melee_pattern.clone(),
        steps: vec![SkillStep {
            tick_offset: 0,
            origin: SkillStepOrigin::Caster,
            pattern: melee_pattern.clone(),
            effects: vec![SkillEffect {
                kind: SkillEffectKind::Damage,
                power: 18,
                scaling: 1.0,
                knockback_cells: 0,
                trigger_skill: None,
                trigger_timing: None,
            }],
        }],
        target_rule: "nearest_enemy".to_string(),
    };
    let archer_skill = SkillDef {
        id: SkillDefId(17002),
        name: "Arrow Shot".to_string(),
        cooldown_ticks: 4,
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
                trigger_skill: None,
                trigger_timing: None,
            }],
        }],
        target_rule: "nearest_enemy".to_string(),
    };
    let slime_skill = SkillDef {
        id: SkillDefId(17003),
        name: "Slime Tackle".to_string(),
        cooldown_ticks: 6,
        cast_pattern: melee_pattern.clone(),
        steps: vec![SkillStep {
            tick_offset: 0,
            origin: SkillStepOrigin::Caster,
            pattern: melee_pattern,
            effects: vec![SkillEffect {
                kind: SkillEffectKind::Damage,
                power: 8,
                scaling: 1.0,
                knockback_cells: 0,
                trigger_skill: None,
                trigger_timing: None,
            }],
        }],
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
        let occupied = world
            .units()
            .iter()
            .map(|unit| unit.grid)
            .collect::<HashSet<_>>();
        assert_eq!(occupied.len(), world.units().len());
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
}
