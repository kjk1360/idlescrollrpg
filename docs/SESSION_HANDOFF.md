# Session Handoff

Read this first in future sessions.

## User Goal

Build a Rust-first belt-scroll Idle RPG production platform, not just a single prototype game.

Target game style:

- 2D belt-scroll RPG with pseudo-3D depth movement
- endless movement to the left
- automatic player/enemy combat
- formation, unit groups, waves, maps, skills, equipment, drops, and behavior patterns are data-driven

Target tool style:

- genre-specific toolchain, closer to RPG Maker than Unity/Unreal/Godot
- sheet-like Data Studio for table/field/row editing
- relation, nested, and joined views are first-class workflow features
- explicit Code Generate button/API
- runtime uses generated Rust types and cached data; runtime does not generate classes dynamically
- Codex should be able to use local CLI/API to validate, view, edit, generate, and build data

## Repository

Local path:

```powershell
C:\Users\Cookapps\belt-scroll-rpg
```

Remote:

```text
https://kjk1360@github.com/kjk1360/idlescrollrpg.git
```

## Current State

Implemented:

- Rust workspace
- `belt_core`: deterministic belt-scroll auto-battle simulation
- `data_studio_core`: schema, row data, relation field kinds, validation, fingerprints
- `RelationIndex`: indexed table/field/row lookup
- `DataView`, `ViewJoin`, `ViewColumn`, `MaterializedView`
- validation for duplicate table/field/row keys
- validation for unknown row cell field ids
- validation for field/cell kind mismatch
- validation for empty required relation lists
- generated typed table accessors with `get_by_id` and `get_by_key`
- generated relation cache for relation fields
- `belt_tools simulate --project` uses generated accessors and relation cache
- `game_data_adapter`: converts generated data into `belt_core::BattleConfig`
- `belt_tools`: CLI for simulation, validation, status, view, codegen, data build
- `scripts/package_tools.ps1`: release packaging for `belt_tools.exe` and `projects/sample`
- `belt_tools serve`: local Data Studio web UI and JSON API
- `belt_tools play`: local playable canvas preview backed by Rust simulation frames
- `projects/sample`: file-based sample data project
- `crates/generated_data`: generated Rust crate from sample schema
- explicit `unit_group_member` data with `unit`, `x`, and `lane`
- visual data tables for texture assets, sprite animations, visual states, state machines, and unit visuals
- Aseprite import through CLI/API into texture, sprite frame, and sprite animation data
- Visual tab sprite sheet grid slicer for bulk `sprite_frame` creation from texture assets
- Visual tab animation frame list editor for active state `sprite_animation` rows
- Visual tab state machine editor for active `visual_state_machine` rows
- `/api/assets` project image browser and Visual tab texture asset create/update UI
- tick-based 1D line `belt_core` wave combat with prepare/engage phases and map clear
- initial item/drop/energy/storage data tables for dungeon reward and operation UI foundations
- initial CellPattern-based skill, skill step, skill effect, and behavior rule data tables
- `unit_def.skills` linked to sample unit skills; adapter reads primary skill cooldown
- `belt_core` runtime skill models for `SkillDef`, `SkillStep`, `SkillEffect`, `CellPattern`, and rotated `CellOffset` cells
- primary skills execute immediate `skill_step` damage effects against a selected line target
- skill use is gated by `skill_def.range`, cooldown, and payable costs
- units can overlap on the combat line; occupancy, lane movement, and path blocking are not part of the current combat rule
- knockback effect plumbing exists as simple distance displacement
- `projectile_damage` effects launch a delayed projectile impact based on line distance
- `skill_effect.impact_pattern` and `CellPattern` data remain for compatibility, but current combat resolution is range/target based
- `tick_offset > 0` skill steps are queued and executed after their tick delay
- sample knight slash has a one-tick delayed 3x3 aftershock step
- `unit_def.behavior_rules` selects skills by descending priority
- supported behavior conditions are `nearest_enemy_in_cast_pattern` and `always`
- `stat_def`, `unit_base_stat`, and `condition_def` tables exist
- runtime `StatBlock` exists on unit definitions and spawned unit states
- behavior rules can evaluate structured conditions against `self` or `target` stats
- `stat_delta` skill effects can add to `self` or `target` stats
- sample slime tackle applies `bleed_stack +1`
- `skill_stat_cost` and `skill_def.costs` exist for Stat-based skill resource costs
- skill selection ignores skills the caster cannot pay for, and skill execution subtracts caster costs before effects run
- `skill_effect.stat_duration_ticks` and `skill_effect.stat_tick_delta` support timed Stat modifiers
- timed Stat modifiers can expire by reversing the initial `stat_delta` and can apply per-tick Stat changes while active
- `belt_tools simulate` previews account energy dispatch cost/recovery and deterministic `drop_table` rewards on map clear
- `belt_tools simulate` previews reward storage settlement by tab and one-day overflow mail output
- `belt_tools simulate` can load/save local account state JSON with energy, inventory stacks, and one-day overflow mail
- account-state reward writeback fills partial stacks before opening new slots and sends capacity overflow to mail
- Data Studio Operation tab displays local account energy, warehouse slots, inventory stacks, and overflow mail
- `/api/account-state` returns the local account-state snapshot and `/api/account-dispatch` runs dungeon dispatch with writeback
- `/api/account-mail/claim` and `/api/account-mail/delete` mutate local overflow mail from the Operation tab
- `/api/account-energy/recover` persists real-time account energy recovery, and dispatch applies recovery before spending energy
- Play Preview renders impact flashes on the combat line
- Play Preview renders projectile previews as red circular orbs with white outlines and ground shadows

## Locked Design Direction

Combat and operation are the two main cores.

Operation:

- UI-driven crafting, enhancement, storage, hero management, and reincarnation hub
- no harvest node loop, offline production job loop, production time, or production energy cost
- combat/dungeon dispatch is the source of base materials and part of advanced materials
- account-level energy is a dungeon dispatch/fatigue resource
- account energy recovers by real elapsed time
- energy can later be sold through web shop flows
- some consumable items can restore energy
- server/db usage is limited to auction, mail, guild, ranking, and similar non-realtime systems
- Supabase is the chosen backend direction when shared online systems are added
- auction house is the first server-backed priority
- chat is explicitly lower priority
- basic player/user state can remain local for the Steam-style client game

Combat:

- automatic tick-based 1D line belt-scroll dungeon combat
- units are positioned by distance on one horizontal combat line
- multiple units can overlap
- no occupancy, path blocking, or lane movement
- no normal collision/pushing
- knockback is simple distance displacement
- no basic attacks; every action is a skill
- a basic attack is a zero-cost skill with range and cooldown
- skill judgment is target in `skill_def.range`, cooldown ready, and costs payable
- `CellPattern` data is retained for compatibility/authoring experiments, but it is not the current combat rule model
- projectile visuals are presentation for runtime projectile entities
- projectile impact judgment happens when the projectile reaches the target distance
- wave flow is `Prepare -> Engage -> Resolve -> NextWave/Clear/Defeat`
- visual scrolling is presentation; systemically, waves align units to start grids, fight, then prepare the next wave

Growth:

- unit rarity does not exist directly
- skills, traits, and stats can have rarity
- consumable growth costs increase as a unit grows
- reincarnation resets growth costs while preserving selected/random skill, trait, or stat elements
- extra reincarnation consumables can increase preserved element count or control random/fixed preservation
- equipment is freely swappable and can be destroyed on combat defeat

Operation UI:

- top-level sections are Warehouse, Hero, and Operation
- Warehouse tabs are material, equipment, and consumable
- each warehouse tab has separate capacity and upgrade rules
- material items stack up to their item stack size, currently defaulting to 10
- equipment and consumables are unique/non-stacking inventory items
- overflow inventory goes to client-local mail for one real day before deletion
- Hero UI manages owned heroes, equipment, consumable use, release, and reincarnation
- Operation tabs are Alchemy Furnace, Forge, and Refinement Workbench
- Alchemy Furnace registers one recipe per output item and crafts non-equipment items instantly
- Forge crafts equipment from a consumable equipment recipe plus required material slots
- Refinement Workbench cubing uses one equipment slot and one material slot for option rerolls/mutation

## Important Commands

Run from workspace root:

```powershell
cd C:\Users\Cookapps\belt-scroll-rpg
```

Commands:

```powershell
cargo test
cargo run -p belt_tools -- simulate
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --current-energy 4 --elapsed-seconds 1200 --seed 1
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --seed 1 --occupied-material-slots 40
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --seed 1 --account-state projects\sample\account_state.json --write-account-state
cargo run -p belt_tools -- data-status --project projects\sample
cargo run -p belt_tools -- validate --project projects\sample
cargo run -p belt_tools -- view --project projects\sample --view map_wave_preview
cargo run -p belt_tools -- codegen --project projects\sample --out crates\generated_data\src
cargo run -p belt_tools -- data-build --project projects\sample --out build\sample_data
cargo run -p belt_tools -- import-aseprite --project projects\sample --file C:\path\unit.aseprite
cargo run -p belt_tools -- serve --project projects\sample --addr 127.0.0.1:7878
cargo run -p belt_tools -- play --project projects\sample --map endless_left_road --addr 127.0.0.1:7879
```

Packaged tool commands:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package_tools.ps1
dist\tools\belt_tools.exe data-status --project dist\projects\sample
dist\tools\belt_tools.exe view --project dist\projects\sample --view map_wave_preview
dist\tools\belt_tools.exe simulate --project dist\projects\sample --map endless_left_road
dist\tools\belt_tools.exe serve --project dist\projects\sample --addr 127.0.0.1:7878
dist\tools\belt_tools.exe play --project dist\projects\sample --map endless_left_road --addr 127.0.0.1:7879
```

Expected sample status:

```text
status: all_fresh
validation: ok
```

Expected `map_wave_preview` shape:

```text
Map               | Wave     | Enemy Group | Enemy Unit | X   | Lane | HP | Attack
------------------+----------+-------------+------------+-----+------+----+-------
Endless Left Road | Wave 001 | Slime Pair  | Slime      | 0   | -0.5 | 45 | 8
Endless Left Road | Wave 001 | Slime Pair  | Slime      | 1.2 | 0.5  | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 0   | -0.8 | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 0.7 | 0    | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 1.4 | 0.8  | 45 | 8
```

## Current Data Project Layout

```text
projects/sample/
  project.json
  schema/
    tables.json
  data/
    unit_def.json
    unit_group.json
    unit_group_member.json
    wave_def.json
    map_def.json
    item_def.json
    drop_table.json
    drop_entry.json
    account_energy_config.json
    storage_tab_config.json
    skill_def.json
    skill_step.json
    skill_effect.json
    cell_pattern.json
    cell_offset.json
    behavior_rule.json
    texture_asset.json
    sprite_frame.json
    sprite_animation.json
    visual_state.json
    visual_state_machine.json
    unit_visual.json
  views/
    views.json
  build/
    generated_schema_fingerprint.json
    built_data_fingerprint.json
```

## Current Game Data Adapter Crate

```text
crates/game_data_adapter/
  Cargo.toml
  src/
    lib.rs
```

## Current Generated Crate

```text
crates/generated_data/
  Cargo.toml
  src/
    lib.rs
    schema_types.rs
    table_accessors.rs
    relation_cache.rs
  tests/
    sample_project_accessors.rs
```

## Current Packaged Tool Output

Generated by `scripts/package_tools.ps1` and ignored by git:

```text
dist/
  tools/
    belt_tools.exe
  projects/
    sample/
```

Verified packaged commands:

- `data-status`: returns `status: all_fresh` and `validation: ok`.
- `view --view map_wave_preview`: prints the map/wave/enemy preview grid.
- `simulate --map endless_left_road`: runs the generated-data-backed battle simulation.
- `serve`: starts the local Data Studio at `http://127.0.0.1:7878`.
- `play`: starts the playable canvas preview at `http://127.0.0.1:7879`.

## Current Combat Runtime Boundary

Implemented:

- tick-based 1D line wave combat
- prepare/engage wave flow
- overlapping unit positions
- movement by horizontal distance with no path blocking
- primary skill selection from `unit_def.skills`
- skill cooldown from `skill_def.cooldown_ticks`
- skill range from `skill_def.range`
- immediate `skill_step` execution when `tick_offset == 0`
- queued `skill_step` execution when `tick_offset > 0`
- behavior rule skill selection by priority
- stat-based behavior conditions using value, stat, and stat-ratio comparisons
- stat-delta skill effects for stack/resource changes
- stat-cost skill payment from `skill_def.costs`
- timed stat modifiers for temporary stacks, buffs/debuffs, and over-time resource changes
- account energy dispatch preview in `simulate`
- deterministic `drop_table` reward preview in `simulate`
- reward storage settlement and overflow mail preview in `simulate`
- single-target line damage effects
- `projectile_damage` effects with delayed impact damage
- `skill_effect.impact_pattern` retained in data for compatibility, but projectile impact currently resolves against the selected target
- knockback movement plumbing as line displacement
- Play Preview `effects` frames for line impact flashes
- Play Preview `projectiles` frames for linear red orb projectile movement

Not implemented yet:

- explicit projectile authoring fields such as speed, visual type, pierce/block rules, and collision policy
- trigger timing and conditional skill activation
- richer behavior conditions such as ally/enemy counts, cooldown availability, distance checks, and target stat filters
- richer resource flows around skill costs, such as mana gain effects, generated UI presets, and cost preview labels
- authoring presets and UI hints for temporary stacks, shields, buffs, debuffs, and over-time effects
- line-combat skill authoring presets for single-target, nearest enemy, self, ally, and all-enemies effects
- persistent inventory writeback, stack merging with existing partial stacks, and overflow mail expiration state
- account energy persistence and consumable energy restore handling

## Current Data Studio UI

Start it from the workspace root:

```powershell
cargo run -p belt_tools -- serve --project projects\sample --addr 127.0.0.1:7878
```

Implemented UI/API surface:

- Schema tab for table/field definition
- Data tab for row/view editing
- table list
- schema table add/delete
- schema field add/delete
- owned nested table creation from the owning field
- nested table tree display under the owner field
- ordinary relation/reference target pickers exclude owned nested tables
- field display name derives from `field_key`
- row add/delete
- editable row grid
- relation selection view with left/right panes and Back navigation
- Data tab headers show field type under display name
- materialized view grid
- project freshness/status indicator
- Validate button
- Codegen button
- Data Build button
- Simulate button
- command/status output panel
- Visual tab for `unit_visual` and sprite animation preview
- Visual tab Aseprite import path input with dropdown hints from known texture paths
- Visual tab sprite sheet grid slicer with canvas overlay preview
- Visual tab animation editor for frame add/remove/reorder plus fps/looping edits
- Visual tab state machine editor for state add/delete, default state, and animation assignment
- Visual tab project asset browser and texture asset create/update form
- project asset serving through `/asset?path=...`
- sprite frame data with texture rect, pivot, and duration fields

Validated endpoints:

- `GET /api/project`
- `GET /api/view?view=map_wave_preview`
- `POST /api/schema/table`
- `POST /api/schema/table/delete`
- `POST /api/schema/field`
- `POST /api/schema/field/delete`
- `POST /api/row`
- `POST /api/row/delete`
- `POST /api/simulate`
- `POST /api/import/aseprite`
- `POST /api/visual/slice-grid`
- `GET /api/assets`

## Current Playable Preview

Start it from the workspace root:

```powershell
cargo run -p belt_tools -- play --project projects\sample --map endless_left_road --addr 127.0.0.1:7879
```

Implemented preview surface:

- Rust `BattleWorld` produces playback frames.
- Browser canvas renders endless-left belt-scroll presentation over tick/grid combat data.
- `BattleWorld` uses grid occupancy, 3 lanes, fixed tick stepping, prepare/engage wave phases, and map clear.
- `unit_def.visual` connects battle units to `unit_visual`.
- `unit_visual` references a visual state machine and placeholder body color.
- visual states reference sprite animations.
- sprite animations reference texture assets, sprite frame rows, and expose frame count/fps/looping data.
- Data Studio and Play Preview load project texture assets and draw frame rects.
- Data Studio Visual tab previews unit visual states with placeholder sprite playback.
- Aseprite JSON/frame tags can be imported into visual asset tables from the Visual tab or `belt_tools import-aseprite`.
- Sprite frame rows can be generated from texture assets through the Visual tab grid slicer.
- Active state animations can be edited in Visual tab without opening raw Data tab tables.
- Active state machines can be edited in Visual tab without opening raw Data tab tables.
- Project image assets can be browsed and registered as `texture_asset` rows from Visual tab.

Validated endpoints:

- `GET /`
- `GET /api/play`

## Account-State Format

Local account-state files are JSON and intentionally map cleanly to a later server-backed account model:

```json
{
  "energy": 92,
  "last_energy_update_unix": 1000,
  "inventory": [
    { "item_key": "slime_gel", "quantity": 3 },
    { "item_key": "energy_tonic", "quantity": 1 }
  ],
  "mail": [
    { "item_key": "slime_gel", "quantity": 5, "expires_at_unix": 87400 }
  ]
}
```

Use `--account-state <path>` to preview settlement against a file and `--write-account-state` to persist dispatch energy, placed inventory stacks, and one-day overflow mail. If `--write-account-state` is used without `--account-state`, the default path is `<project>\account_state.json`.

## Recommended Next Task

Expose the local account-state loop in the tool UI and playable preview.

Recommended order:

1. Add mail expiry cleanup on account-state load/save.
2. Add first recipe tables and instant alchemy/forge/refinement commands.
3. Add Operation UI sections for Alchemy Furnace, Forge, and Refinement Workbench.
4. Add Supabase design notes for auction house tables, RLS policies, and Edge Function mutation boundaries.
5. Keep chat/guild/ranking behind auction house priority.
6. Connect battle simulation states to visual state machine keys.
7. Package and verify the updated `belt_tools.exe` again.

## Caveats

- Renderer is currently a browser canvas playable preview with placeholder sprite bodies; real texture loading/slicing is not implemented yet.
- Visual Data Studio UI is implemented only as a first local web UI; owned nested tables are created and displayed with ownership, but inline editing still opens a selection/detail workflow rather than a polished embedded child editor.
- Data Build currently writes a JSON snapshot only.
- Generated relation cache validates and stores row ids, but does not yet expose typed target row helpers.
