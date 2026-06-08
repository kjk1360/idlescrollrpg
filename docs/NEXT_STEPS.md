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
- Added `scripts/package_tools.ps1`.
- Packaged and verified `dist\tools\belt_tools.exe` with the sample project.
- Added `belt_tools serve`, a local web Data Studio UI.
- Added UI/API support for table grids, view grids, cell edits, validate, codegen, data-build, and simulate.
- Split Data Studio into Schema and Data tabs.
- Added Schema tab support for table add/delete and field add/delete.
- Added Data tab support for row add/delete.
- Added relation selection view with Back navigation for relation-one, relation-many, reference-group, and owned-nested row references.

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
cargo run -p belt_tools -- serve --project projects\sample --addr 127.0.0.1:7878
```

## Completed Milestone: Tool Packaging Before UI

The backend can now be packaged as:

```text
belt_tools.exe
```

Packaging output:

```text
dist/tools/belt_tools.exe
dist/projects/sample/
```

Verified commands:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package_tools.ps1
dist\tools\belt_tools.exe data-status --project dist\projects\sample
dist\tools\belt_tools.exe view --project dist\projects\sample --view map_wave_preview
dist\tools\belt_tools.exe simulate --project dist\projects\sample --map endless_left_road
dist\tools\belt_tools.exe serve --project dist\projects\sample --addr 127.0.0.1:7878
```

## Completed Milestone: Minimal Data Studio UI

The first focused local UI is available through `belt_tools serve`:

- Schema tab for table/field definition.
- Data tab for row/view editing.
- table list
- row grid
- view preview grid
- validate button
- codegen button
- data-build button
- simulate button
- command/status output panel
- editable cells saved back to the file-based project
- schema field add/delete
- schema table add/delete
- row add/delete
- relation selection view with left/right panes and Back navigation

## Immediate Next Milestone: Relation/Nested Editing UX

The UI can currently edit schema fields, raw cell values, rows, and relation references. Next, make nested data and relation UX richer:

- owned nested table panel launched from the parent cell
- relation picker pagination/search for large tables
- richer row display labels beyond id/key/name fallback
- nested child row create/delete from the parent cell
- view validation for join/column mismatch cases

## Next Validation Work

Current views can materialize relation joins. Add validation for:

- missing join source alias
- join field not present on source alias table
- join field target table mismatch
- output column alias not found
- output column field not present on alias table
