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
- `projects/sample`: file-based sample data project
- `crates/generated_data`: generated Rust crate from sample schema
- explicit `unit_group_member` data with `unit`, `x`, and `lane`

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
  views/
    views.json
  build/
    generated_schema_fingerprint.json
    built_data_fingerprint.json
```

## Current Generated Crate

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

## Recommended Next Task

Package the CLI.

Recommended order:

1. Add a repeatable release build command or script for `belt_tools.exe`.
2. Copy `belt_tools.exe` to `dist/tools/`.
3. Copy `projects/sample` to `dist/projects/sample`.
4. Verify the packaged exe can run `data-status`, `view`, and `simulate`.
5. After that, start a minimal visual Data Studio UI.

## Caveats

- Renderer is not implemented yet.
- Visual Data Studio UI is not implemented yet.
- Data Build currently writes a JSON snapshot only.
- Generated relation cache validates and stores row ids, but does not yet expose typed target row helpers.
