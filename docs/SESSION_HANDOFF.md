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
- `belt_tools`: CLI for simulation, validation, status, view, codegen, data build
- `projects/sample`: file-based sample data project
- `crates/generated_data`: generated Rust crate from sample schema
- `simulate --project projects\sample --map endless_left_road`
- `view --project projects\sample --view map_wave_preview`

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

## Recommended Next Task

Replace the temporary unit group placement rule with explicit formation/member-slot data.

Recommended order:

1. Add explicit `unit_group_member` data.
2. Store `unit`, `x`, and `lane` per member instead of deriving placement by index.
3. Decide whether the first implementation uses a top-level table or `OwnedNestedTable`.
4. Update `map_wave_preview` or add a `formation_preview` view.
5. Move the temporary `battle_config_from_project` adapter out of `belt_tools` later.

## Caveats

- Renderer is not implemented yet.
- Visual Data Studio UI is not implemented yet.
- Codegen currently generates type structs and stub files only.
- Relation cache generation is still a stub.
- Data Build currently writes a JSON snapshot only.
- Unit group spawn positions are currently derived from member order, not authored as data.
