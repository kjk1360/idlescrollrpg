# Next Steps

## Completed Since Initial Foundation

- Added `serde`/`serde_json` support to `data_studio_core`.
- Added file-based `DataProject::load_from_dir`.
- Added file-based `DataProject::save_to_dir`.
- Added project fingerprint loading/writing.
- Added `projects/sample`.
- Added CLI `--project` support for data commands.
- Added `validate`.
- Added `codegen`.
- Added `data-build`.
- Added `crates/generated_data`.
- Added `RelationIndex`.
- Added `DataView`, `ViewJoin`, `ViewColumn`, and `MaterializedView`.
- Added `belt_tools view`.
- Added `projects/sample/views/views.json`.
- Added `map_wave_preview`, which expands map -> wave -> enemy group -> enemy unit.
- Added `unit_group_member` with explicit `unit`, `x`, and `lane`.
- Updated battle simulation to use explicit member slot positions.
- Strengthened validation for duplicate table/field/row keys.
- Strengthened validation for unknown row cell field ids.
- Strengthened validation for field/cell kind mismatch.
- Strengthened validation for empty required relation lists.
- Added generated typed table accessors.
- Added generated `GeneratedDatabase::from_project`.
- Added generated `GeneratedTable<T>::get_by_id` and `get_by_key`.

## Immediate Next Milestone: Data-Driven BattleConfig

Completed. This command now loads `projects/sample` and converts table rows into `belt_core::BattleConfig`:

```powershell
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road
```

The temporary adapter is implemented in `crates/tools/src/main.rs`.

## Completed Milestone: Explicit Formation Data

Previous limitation:

```text
unit_group.members: reference_group -> unit_def
```

Current structure:

```text
unit_group.members: relation_many -> unit_group_member

unit_group_member:
  unit: relation_one -> unit_def
  x: f32
  lane: f32
```

The current implementation uses an explicit top-level `unit_group_member` table. Later, the visual Data Studio can present this as an owned nested editor under `unit_group`.

## Completed Milestone: Generated Accessors Before UI

Generated accessors now load typed rows from a `DataProject`.

Example generated concepts:

```text
GeneratedDatabase
GeneratedTable<T>
get_by_id(RowId)
get_by_key(&str)
```

## Immediate Next Milestone: Relation Cache And Adapter Cleanup

## Step 1: Add Relation Cache Skeleton

Current generated `table_accessors.rs` is a stub. Add either:

- generic typed row readers in `data_studio_core`, or
- generated typed table accessors in `generated_data`.

For the next milestone, a pragmatic manual adapter is acceptable:

```rust
fn battle_config_from_project(project: &DataProject, map_key: &str) -> Result<BattleConfig, Error>
```

This currently lives in `belt_tools`. Move it later into a dedicated adapter crate or generated access layer.

Before building complex gameplay data, add a minimal relation index:

```text
table_id -> row_id -> RowData
table_key -> TableSchema
row_key -> RowId
```

This will reduce ad hoc row scanning and prepare for generated accessors.

## Step 2: Extend Codegen

Current generated files:

- `schema_types.rs`: generated structs
- `table_accessors.rs`: stub
- `relation_cache.rs`: stub

Next codegen targets:

- typed table container structs
- `get_by_id`
- `get_by_key`
- relation cache structs
- schema fingerprint comments or constants

## Step 6: Visual UI Later

Do not start the visual Data Studio UI until the file format and CLI/API workflow are stable.

Initial UI should include:

- table list
- schema editor
- row grid
- relation picker
- nested table editor
- status badges
- Validate / Code Generate / Build Data buttons
