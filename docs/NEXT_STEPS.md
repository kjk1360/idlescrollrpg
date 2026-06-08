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
- Added generated `GeneratedRelationCache`.
- Updated `belt_tools simulate --project` to use generated accessors and relation cache.
- Added `game_data_adapter`.
- Moved battle config conversion out of `belt_tools`.

## Current Stable CLI Flow

Run from workspace root:

```powershell
cd C:\Users\Cookapps\belt-scroll-rpg
```

Useful commands:

```powershell
cargo test
cargo run -p belt_tools -- data-status --project projects\sample
cargo run -p belt_tools -- validate --project projects\sample
cargo run -p belt_tools -- view --project projects\sample --view map_wave_preview
cargo run -p belt_tools -- codegen --project projects\sample --out crates\generated_data\src
cargo run -p belt_tools -- data-build --project projects\sample --out build\sample_data
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road
```

## Immediate Next Milestone: Tool Packaging Before UI

The backend is now split enough to package the CLI before starting a visual UI.

### Step 1: Package CLI

Add a repeatable release build flow for:

```text
belt_tools.exe
```

Initial packaging target:

```text
dist/tools/belt_tools.exe
dist/projects/sample/
```

The future visual Data Studio should call this executable or its API-equivalent backend.

### Step 2: Add Richer View Validation

Current views can materialize relation joins. Add validation for:

- missing join source alias
- join field not present on source alias table
- join field target table mismatch
- output column alias not found
- output column field not present on alias table

### Step 3: Minimal Data Studio UI

Do not build a broad editor yet. Start with a focused local UI:

- table list
- row grid
- view preview grid
- validate button
- codegen button
- data-build button
- simulate button

The first UI can be read/edit-light. Relation/nested editing can follow after the view grid is stable.
