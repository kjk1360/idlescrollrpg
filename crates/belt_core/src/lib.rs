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

        let snapshot = self.units.clone();
        let mut attacks = Vec::new();

        for index in 0..self.units.len() {
            if !self.units[index].is_alive() {
                continue;
            }

            self.units[index].attack_cooldown = (self.units[index].attack_cooldown - dt).max(0.0);

            let target = closest_target(&snapshot, &self.units[index]);
            let in_range = target
                .as_ref()
                .map(|target| {
                    grid_in_range(
                        self.units[index].grid,
                        target.grid,
                        self.units[index].attack_range,
                    )
                })
                .unwrap_or(false);

            if let Some(target) = target {
                if in_range && self.units[index].attack_cooldown <= 0.0 {
                    attacks.push((self.units[index].id, target.id, self.units[index].attack));
                    self.units[index].attack_cooldown =
                        self.units[index].skill_cooldown_ticks as f32 * self.config.tick_duration;
                } else if !in_range {
                    self.move_toward(index, target.grid);
                }
            }
        }

        for (attacker, target, damage) in attacks {
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
}
