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
- Converted `belt_core` battle runtime to tick-based wave combat.
- Changed maps to clear after their final wave instead of looping indefinitely.
- Removed the operation harvesting/production premise from the locked design direction.
- Added initial item, drop table, account energy, and storage tab data tables.
- Added initial CellPattern-based skill, skill step, skill effect, and behavior rule data tables.
- Linked `unit_def.skills` to sample unit skills and wired primary skill cooldown into battle config conversion.
- Added `belt_core` runtime models for `SkillDef`, `SkillStep`, `SkillEffect`, `CellPattern`, and rotated `CellOffset` cells.
- Simplified combat from 3-lane grid tactics to 1D distance-based line combat with overlap allowed.
- Added `skill_def.range`; skills fire when the target is within range, cooldown is ready, and costs can be paid.
- Kept `CellPattern` data for compatibility, but current runtime skill judgment is range-based.

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
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --current-energy 4 --elapsed-seconds 1200 --seed 1
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --seed 1 --occupied-material-slots 40
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road --seed 1 --account-state projects\sample\account_state.json --write-account-state
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
- Browser canvas renders endless-left belt-scroll presentation over 1D line combat data.
- Battle runtime uses fixed tick stepping, prepare/engage wave phases, line movement, overlapping units, and map clear.
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
- Battle runtime now reads generated skill data and resolves immediate damage against the selected line target.
- Battle runtime supports `projectile_damage` effects that launch a projectile, travel linearly by distance, and resolve delayed single-target impact damage.
- `skill_effect.impact_pattern` remains in data for compatibility, but current line combat resolves projectile impact against the selected target.
- Play Preview renders default skill impact flashes on the line.
- Play Preview renders projectile previews as red circular orbs with white outlines and ground shadows.
- Added queued `skill_step` execution for `tick_offset > 0`.
- Added a sample knight aftershock step that fires one tick after the initial slash.
- Added `unit_def.behavior_rules` and runtime behavior rule selection by priority.
- Added `nearest_enemy_in_cast_pattern` and `always` behavior conditions.
- Added `stat_def`, `unit_base_stat`, and `condition_def` tables.
- Added runtime `StatBlock` and behavior conditions that can compare `self` or `target` stats.
- Added stat compare modes for fixed value, other stat, and other stat ratio.
- Added `stat_delta` skill effects that can add to `self` or `target` stats.
- Added sample slime bleed-stack application through `stat_delta`.
- Added `skill_stat_cost` data and `skill_def.costs` relations for Stat-based skill resource costs.
- Battle runtime checks skill costs before behavior-rule selection/fallback skill use and pays the caster's costs when the skill executes.
- Added timed Stat modifiers through `skill_effect.stat_duration_ticks` and `skill_effect.stat_tick_delta`.
- Timed Stat modifiers can expire by reversing the initial `stat_delta` and can also apply per-tick Stat changes while active.
- `belt_tools simulate` now previews account energy dispatch cost/recovery and deterministic `drop_table` rewards on map clear.
- `belt_tools simulate` now previews reward storage settlement by storage tab and sends overflow quantities to one-day overflow mail output.
- `belt_tools simulate` can load/save local account state JSON with energy, inventory stacks, and expiring overflow mail.
- Account-state reward writeback fills partial inventory stacks before opening new slots and sends capacity overflow to one-day mail.
- Data Studio Operation tab can display local account energy, warehouse slots, inventory stacks, and overflow mail.
- Data Studio can dispatch the sample dungeon through `/api/account-dispatch` and persist the local account-state file.
- Operation mail rows now support claim/delete actions through local account-state APIs.

## Locked Design Direction

- Combat and operation are the two core loops.
- Operation is a UI-driven crafting, enhancement, storage, hero management, and reincarnation hub.
- Operation has no harvest node loop, no offline production job loop, no production time, and no production energy cost.
- Combat/dungeon dispatch is the source of base materials and part of advanced materials.
- Account energy is a dungeon dispatch/fatigue resource.
- Account energy recovers by real elapsed time, can later be sold through web shop flows, and can be restored by consumable items.
- Combat is automatic tick-based 1D line belt-scroll dungeon combat.
- Units are positioned by distance on a single horizontal combat line.
- Multiple units can overlap; there is no occupancy, path blocking, or lane movement.
- Collision/pushing is not part of normal movement; knockback is a simple distance displacement effect.
- There is no basic attack concept; every action is a skill.
- A basic attack is represented as a zero-cost skill with range and cooldown.
- Skill judgment is intentionally simple: target in `skill_def.range`, cooldown ready, and costs payable.
- `CellPattern` is no longer the primary combat rule model; it is retained as compatibility/authoring data until removed or repurposed.
- Projectile visuals are presentation for runtime projectile entities; impact judgment happens when the projectile reaches the target distance.
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

## Current Account-State Format

The local account-state file is intentionally small and server-portable:

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

`--write-account-state` defaults to `projects\sample\account_state.json` when `--project projects\sample` is used. Without `--write-account-state`, `--account-state <path>` only previews the account settlement.

## Immediate Next Milestone: Operation Account Loop v1

The next production-facing step is to expose the local account state in the UI and make it behave like the later server-backed account model:

- Energy display with elapsed real-time recovery preview instead of raw unix values.
- Account-state API endpoints in `belt_tools play` for preview/test workflows.
- First recipe tables and instant alchemy/forge/refinement commands that mutate the same account-state file.
- Mail expiry cleanup pass that removes expired local mail on account-state load/save.

## Server Direction

Use Supabase only for shared online systems where a server is actually needed. The current priority is auction house first; chat, guild, and rankings are later. Basic user state can remain local because the target is a Steam-style client game and the server exists mainly for cross-user content.

When Supabase is introduced, keep security official and conservative:

- Supabase Auth for identity.
- Postgres RLS for table-level access boundaries.
- Edge Functions or Postgres RPC for all auction/economy mutations.
- No direct client writes for listing creation, purchase settlement, currency transfer, or mail rewards created by auction actions.
- Local account-state remains the offline gameplay source of truth until a specific shared feature requires server authority.

## Immediate Next Milestone: Combat Skill Runtime v1

The runtime is tick-based 1D line combat. Primary skills execute immediate and delayed `skill_step` entries against a selected target when that target is within `skill_def.range`, costs can be paid, and cooldown is ready. Units can choose skills from priority behavior rules with stat-based conditions. Next, extend this into the full skill execution model:

- explicit projectile authoring fields such as speed, visual type, pierce/block rules, and collision policy
- trigger timing runtime for conditional skill activation
- richer behavior conditions such as ally/enemy counts, cooldown availability, distance checks, and target stat filters
- richer resource flows around skill costs, such as mana gain effects, generated UI presets, and cost preview labels
- authoring presets and UI hints for temporary stacks, shields, buffs, debuffs, and over-time effects
- persistent inventory writeback, stack merging with existing partial stacks, and overflow mail expiration state
- account energy persistence and consumable energy restore handling
- battle simulation states to visual state machine keys
- line-combat skill authoring presets for single-target, nearest enemy, self, ally, and all-enemies effects
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
