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
- Added owned nested table creation from the owning field.
- Excluded owned nested tables from ordinary relation/reference target selection.
- Rendered nested tables as a recursive table tree under the owning field.
- Changed field display names to derive from `field_key`.
- Changed Data tab column headers to show field type instead of field key.
- Added visual data tables: `texture_asset`, `sprite_animation`, `visual_state`, `visual_state_machine`, and `unit_visual`.
- Added `unit_def.visual`.
- Added `belt_tools play`, a local playable canvas preview backed by Rust battle simulation frames.
- Added a Data Studio Visual tab for unit visual and sprite animation preview.
- Added `sprite_frame` data with texture rect, pivot, and duration fields.
- Added project asset serving through `/asset?path=...`.
- Added sample sprite sheet asset at `projects/sample/assets/units/placeholder_units.svg`.
- Updated Data Studio Visual tab and Play Preview to draw texture frame rects when available.
- Added `belt_tools import-aseprite` for `.aseprite`/`.ase` files through the Aseprite CLI and direct Aseprite JSON exports.
- Added `POST /api/import/aseprite` and a Visual tab import control for Aseprite assets.
- Added Visual tab sprite sheet grid slicer preview and `POST /api/visual/slice-grid` for bulk `sprite_frame` creation.
- Added Visual tab animation frame list editor for active state animations.
- Added Visual tab state machine editor for state add/delete, default state, and animation assignment.
- Added `/api/assets` project asset browser and Visual tab texture asset create/update UI.
- Converted `belt_core` battle runtime to tick/grid-based wave combat.
- Changed maps to clear after their final wave instead of looping indefinitely.
- Removed the operation harvesting/production premise from the locked design direction.
- Added initial item, drop table, account energy, and storage tab data tables.
- Added initial CellPattern-based skill, skill step, skill effect, and behavior rule data tables.
- Linked `unit_def.skills` to sample unit skills and wired primary skill cooldown into battle config conversion.
- Added `belt_core` runtime models for `SkillDef`, `SkillStep`, `SkillEffect`, `CellPattern`, and rotated `CellOffset` cells.
- Changed grid combat to execute primary skills through `CellPattern` judgment and immediate `skill_step` damage effects.
- Added knockback effect plumbing for forced grid movement, including occupied-cell blocking and lane clamping.

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
cargo run -p belt_tools -- import-aseprite --project projects\sample --file C:\path\unit.aseprite
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road
cargo run -p belt_tools -- serve --project projects\sample --addr 127.0.0.1:7878
cargo run -p belt_tools -- play --project projects\sample --map endless_left_road --addr 127.0.0.1:7879
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
dist\tools\belt_tools.exe play --project dist\projects\sample --map endless_left_road --addr 127.0.0.1:7879
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
- owned nested field creation creates a new child table instead of selecting an existing table
- nested tables are shown under their owner field in the table tree
- Data tab headers show display name plus field type

## Completed Milestone: Playable Preview v0

The first playable preview is available through `belt_tools play`:

- Rust `BattleWorld` produces simulation frames from project data.
- Browser canvas renders endless-left belt-scroll presentation over tick/grid combat data.
- Battle runtime uses grid occupancy, 3 lanes, fixed tick stepping, prepare/engage wave phases, and map clear.
- Unit visuals are driven by `unit_def.visual`.
- Visual data supports texture asset references, sprite animation settings, visual states, state machines, and unit visual settings.
- Placeholder sprite rendering uses `unit_visual.body_color` until real texture loading/slicing is added.
- Data Studio Visual tab previews `unit_visual` state animations with placeholder sprite playback.
- Sprite animations can reference explicit `sprite_frame` rows.
- Data Studio and Play Preview load project texture assets and draw frame rects.
- Aseprite imports create `texture_asset`, `sprite_frame`, and `sprite_animation` rows from spritesheet JSON and frame tags.
- Visual tab can now generate `sprite_frame` rows from a selected texture through a grid slicer.
- Visual tab can edit the active `sprite_animation.frames` order, add/remove frames, and update fps/looping.
- Visual tab can edit the active `visual_state_machine` states, default state, and state animation references.
- Visual tab can browse project image files and create/update `texture_asset` rows.
- Battle runtime now reads generated skill data and resolves immediate damage through directional `CellPattern` cells.
- Battle runtime supports `projectile_damage` effects that launch a projectile, travel linearly at one grid cell per 0.2s tick, and resolve delayed impact damage.
- Added `skill_effect.impact_pattern` so projectile impacts can resolve a `CellPattern` centered on the projectile destination grid.
- Added sample `impact_3x3` data and linked it to the archer projectile impact.
- Play Preview renders default skill area flashes as translucent animated red grid squares.
- Play Preview renders projectile previews as red circular orbs with white outlines and ground shadows.
- Added queued `skill_step` execution for `tick_offset > 0`.
- Added a sample knight aftershock step that fires one tick after the initial slash.
- Added `unit_def.behavior_rules` and runtime behavior rule selection by priority.
- Added `nearest_enemy_in_cast_pattern` and `always` behavior conditions.
- Added `stat_def`, `unit_base_stat`, and `condition_def` tables.
- Added runtime `StatBlock` and behavior conditions that can compare `self` or `target` stats.
- Added stat compare modes for fixed value, other stat, and other stat ratio.

## Locked Design Direction

- Combat and operation are the two core loops.
- Operation is a UI-driven crafting, enhancement, storage, hero management, and reincarnation hub.
- Operation has no harvest node loop, no offline production job loop, no production time, and no production energy cost.
- Combat/dungeon dispatch is the source of base materials and part of advanced materials.
- Account energy is a dungeon dispatch/fatigue resource.
- Account energy recovers by real elapsed time, can later be sold through web shop flows, and can be restored by consumable items.
- Combat is automatic tick/grid belt-scroll dungeon combat.
- Combat grid is an advancing-axis grid with 3 lanes.
- One unit occupies one grid cell; occupied cells cannot be entered or crossed.
- Collision/pushing is not part of normal movement; knockback is a forced grid movement effect.
- There is no basic attack concept; every action is a skill.
- Skill judgment and effects use directional grid AABB/range shapes with four cast directions.
- Skill ranges should use `CellPattern` as the primary internal model: a collection of relative `forward/side` grid offsets rotated by the four cast directions.
- AABB, line, cross, and 3x3 are authoring presets that generate `CellPattern` data, not the core runtime representation.
- Projectile visuals are presentation for runtime projectile entities; impact judgment happens when the projectile reaches the destination grid center.
- Effects that look like ground spikes, columns, or wave bursts should normally be authored as AOE steps, not projectiles, so player-facing visuals match projectile defense rules.
- A map runs waves as `Prepare -> Engage -> Resolve -> NextWave/Clear/Defeat`.
- Visual scrolling is presentation; systemically, waves align units to start grids, fight, then prepare the next wave.
- Unit rarity does not exist directly; skills, traits, and stats can have rarity.
- Unit growth consumes items with increasing costs, and reincarnation resets growth costs while preserving selected/random skill, trait, or stat elements.
- Equipment is freely swappable and can be destroyed on combat defeat.
- Storage has material/equipment/consumable tabs with separate capacities and upgrades.
- Overflow inventory goes to client-local mail for one real day before deletion.
- Operation UI has Warehouse, Hero, and Operation sections.
- Operation section has Alchemy Furnace, Forge, and Refinement Workbench tabs.
- Alchemy Furnace registers one recipe per output item and crafts non-equipment items instantly.
- Forge crafts equipment from a consumable equipment recipe plus required material slots.
- Refinement Workbench cubing uses one equipment slot and one material slot for option rerolls/mutation.

## Immediate Next Milestone: Combat Skill Runtime v1

The runtime is grid/tick based, primary skills execute immediate and delayed `skill_step` entries through generated `CellPattern` data, projectile impacts can resolve a destination-centered `CellPattern`, and units can choose skills from priority behavior rules with stat-based conditions. Next, extend this into the full skill execution model:

- explicit projectile authoring fields such as speed, visual type, pierce/block rules, and collision policy
- trigger timing runtime for conditional skill activation
- richer behavior conditions such as ally/enemy counts, cooldown availability, lane checks, and enemies in pattern with stat filters
- stat-modifying skill effects for mana gain/spend, stack application, shields, buffs, and debuffs
- dungeon reward result generation from `drop_table`
- account energy spending/recovery simulation
- battle simulation states to visual state machine keys
- skill authoring presets that generate `CellPattern` rows: AABB, line, cross, 3x3
- row preview thumbnails for sprite frame lists and palettes
- relation picker pagination/search for large tables
- richer row display labels beyond id/key/name fallback
- inline nested row editing from the parent cell without manually opening the child table
- view validation for join/column mismatch cases

## Next Validation Work

Current views can materialize relation joins. Add validation for:

- missing join source alias
- join field not present on source alias table
- join field target table mismatch
- output column alias not found
- output column field not present on alias table
