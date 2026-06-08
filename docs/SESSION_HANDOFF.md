# Session Handoff

This file is the first document a future Codex session should read before continuing work.

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
https://github.com/kjk1360/idlescrollrpg.git
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
- `belt_tools`: CLI for simulation, validation, status, view, codegen, data build
- `projects/sample`: file-based sample data project
- `crates/generated_data`: generated Rust crate from sample schema
- `simulate --project projects\sample --map endless_left_road`
- `view --project projects\sample --view map_wave_preview`
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
Map               | Wave     | Enemy Group | Enemy Unit | HP | Attack
------------------+----------+-------------+------------+----+-------
Endless Left Road | Wave 001 | Slime Pair  | Slime      | 45 | 8
Endless Left Road | Wave 001 | Slime Pair  | Slime      | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 45 | 8
Endless Left Road | Wave 002 | Slime Line  | Slime      | 45 | 8
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
crates/generated_data/
  Cargo.toml
  src/
    lib.rs
    schema_types.rs
    table_accessors.rs
    relation_cache.rs
```

## Completed Formation Slot Data

`unit_group.members` now points to `unit_group_member` rows instead of `unit_def` rows directly.

Each `unit_group_member` row contains:

```text
unit: relation_one -> unit_def
x: f32
lane: f32
```

The battle config loader now reads these explicit `x/lane` values instead of deriving placement from member order.

## Recommended Next Task

Implement the first real Data Studio editing surface or continue backend hardening before UI.

Recommended order:

1. Add generated table accessors for `get_by_id` and `get_by_key`.
2. Add generated relation cache skeleton based on relation fields.
3. Move the temporary `battle_config_from_project` adapter out of `belt_tools`.
4. Start a minimal Data Studio UI only after accessors/validation are stable.

## Caveats

- Renderer is not implemented yet.
- Visual Data Studio UI is not implemented yet.
- Codegen currently generates type structs and stub files only.
- Relation cache generation is still a stub.
- Data Build currently writes a JSON snapshot only.
- Unit group spawn positions are currently derived from member order, not authored as data.
