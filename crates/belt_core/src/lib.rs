use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitDefId(pub u32);

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

#[derive(Debug, Clone)]
pub struct UnitDef {
    pub id: UnitDefId,
    pub name: String,
    pub max_hp: i32,
    pub attack: i32,
    pub attack_range: f32,
    pub attack_interval: f32,
    pub move_speed: f32,
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
    pub position: BeltPosition,
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
}

#[derive(Debug)]
pub struct BattleWorld {
    config: BattleConfig,
    units: Vec<UnitState>,
    events: Vec<BattleEvent>,
    active_wave_id: Option<String>,
    pending_waves: VecDeque<WaveDef>,
    next_unit_id: u64,
    map_loop_count: u64,
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
            map_loop_count: 0,
        };

        let party = world.config.party.clone();
        world.spawn_group(&party, Team::Player, 0.0);
        world.start_next_wave();
        world
    }

    pub fn units(&self) -> &[UnitState] {
        &self.units
    }

    pub fn drain_events(&mut self) -> Vec<BattleEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn tick(&mut self, dt: f32) {
        self.cleanup_dead();

        if !self.any_alive(Team::Enemy) {
            if let Some(wave_id) = self.active_wave_id.take() {
                self.events.push(BattleEvent::WaveCleared { wave_id });
            }
            self.start_next_wave();
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
                    self.units[index].position.distance_to(target.position)
                        <= self.units[index].attack_range
                })
                .unwrap_or(false);

            if let Some(target) = target {
                if in_range && self.units[index].attack_cooldown <= 0.0 {
                    attacks.push((self.units[index].id, target.id, self.units[index].attack));
                    self.units[index].attack_cooldown = self.units[index].attack_interval;
                } else if !in_range {
                    let direction = if self.units[index].team == Team::Player {
                        -1.0
                    } else {
                        1.0
                    };
                    self.units[index].position.x += direction * self.units[index].move_speed * dt;
                    self.events.push(BattleEvent::UnitMoved {
                        unit_id: self.units[index].id,
                        x: self.units[index].position.x,
                        lane: self.units[index].position.lane,
                    });
                }
            } else if self.units[index].team == Team::Player {
                self.units[index].position.x -= self.config.left_scroll_speed * dt;
                self.events.push(BattleEvent::UnitMoved {
                    unit_id: self.units[index].id,
                    x: self.units[index].position.x,
                    lane: self.units[index].position.lane,
                });
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

    fn start_next_wave(&mut self) {
        if self.pending_waves.is_empty() {
            self.map_loop_count += 1;
            self.events.push(BattleEvent::MapLooped {
                map_id: self.config.map.id.clone(),
                loop_count: self.map_loop_count,
            });
            self.pending_waves = VecDeque::from(self.config.map.waves.clone());
        }

        if let Some(wave) = self.pending_waves.pop_front() {
            self.active_wave_id = Some(wave.id.clone());
            self.events.push(BattleEvent::WaveStarted {
                wave_id: wave.id.clone(),
            });
            for group in &wave.enemy_groups {
                self.spawn_group(group, Team::Enemy, self.config.wave_spawn_x);
            }
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
                position: BeltPosition {
                    x: spawn.position.x + x_offset,
                    lane: spawn.position.lane,
                },
            };
            self.events.push(BattleEvent::UnitSpawned {
                unit_id,
                name: state.name.clone(),
                team,
            });
            self.units.push(state);
        }
    }
}

fn closest_target(units: &[UnitState], actor: &UnitState) -> Option<UnitState> {
    units
        .iter()
        .filter(|unit| unit.is_alive() && actor.team.is_enemy_of(unit.team))
        .min_by(|a, b| {
            actor
                .position
                .distance_to(a.position)
                .total_cmp(&actor.position.distance_to(b.position))
        })
        .cloned()
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
    };
    let archer = UnitDef {
        id: UnitDefId(2),
        name: "Archer".to_string(),
        max_hp: 70,
        attack: 12,
        attack_range: 5.0,
        attack_interval: 0.8,
        move_speed: 2.1,
    };
    let slime = UnitDef {
        id: UnitDefId(100),
        name: "Slime".to_string(),
        max_hp: 45,
        attack: 8,
        attack_range: 1.0,
        attack_interval: 1.2,
        move_speed: 1.5,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_battle_clears_at_least_one_wave() {
        let mut world = BattleWorld::new(sample_battle_config());

        for _ in 0..300 {
            world.tick(0.1);
        }

        let events = world.drain_events();
        assert!(events.iter().any(
            |event| matches!(event, BattleEvent::WaveCleared { wave_id } if wave_id == "wave_001")
        ));
        assert!(world.units().iter().any(|unit| unit.team == Team::Player));
    }
}
