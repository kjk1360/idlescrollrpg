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
- tick/grid-based `belt_core` wave combat with prepare/engage phases and map clear

## Locked Design Direction

Combat and operation are the two main cores.

Operation:

- offline-first farm/crafting simulation
- account-level energy
- resource harvesting, crafting, synthesis, equipment, consumable growth items, delivery/sink flows
- production time uses `effective_duration = base_duration * 10000 / time_multiplier`
- default `time_multiplier` is `10000`
- server/db usage is limited to auction, mail, guild, ranking, and similar non-realtime systems

Combat:

- automatic tick/grid belt-scroll dungeon combat
- advancing-axis grid with 3 lanes
- one unit per grid cell
- occupied cells cannot be entered or crossed
- no normal collision/pushing
- knockback is forced grid movement
- no basic attacks; every action is a skill
- skill judgment/effects use directional grid AABB/range shapes
- cast directions are up/down/left/right only
- wave flow is `Prepare -> Engage -> Resolve -> NextWave/Clear/Defeat`
- visual scrolling is presentation; systemically, waves align units to start grids, fight, then prepare the next wave

Growth:

- unit rarity does not exist directly
- skills, traits, and stats can have rarity
- consumable growth costs increase as a unit grows
- reincarnation resets growth costs while preserving selected/random skill, trait, or stat elements
- extra reincarnation consumables can increase preserved element count or control random/fixed preservation
- equipment is freely swappable and can be destroyed on combat defeat

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

## Recommended Next Task

Improve sprite asset editing and visual preview authoring.

Recommended order:

1. Add explicit skill, skill effect, behavior, and target rule data models.
2. Connect battle simulation states to visual state machine keys.
3. Add knockback forced movement effect.
4. Add row preview thumbnails for sprite frame lists and palettes.
5. Add pagination/search to relation picker for large target tables.
5. Package and verify the updated `belt_tools.exe` again.

## Caveats

- Renderer is currently a browser canvas playable preview with placeholder sprite bodies; real texture loading/slicing is not implemented yet.
- Visual Data Studio UI is implemented only as a first local web UI; owned nested tables are created and displayed with ownership, but inline editing still opens a selection/detail workflow rather than a polished embedded child editor.
- Data Build currently writes a JSON snapshot only.
- Generated relation cache validates and stores row ids, but does not yet expose typed target row helpers.
