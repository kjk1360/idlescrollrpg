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
- Added first `alchemy_recipe` and `recipe_ingredient` data tables.
- Added instant Alchemy Furnace crafting that consumes account inventory ingredients and writes crafted output back to the same local account-state file.
- Data Studio Operation tab now shows Alchemy Furnace recipes, ingredient availability, craftable state, and a Craft action.
- Added `POST /api/account-alchemy/craft` for local account-state mutation.
- Added first `forge_recipe` and `forge_ingredient` data tables.
- Added instant Forge crafting that consumes an equipment recipe item plus material slots and writes equipment output back to the same local account-state file.
- Data Studio Operation tab now shows Forge recipes, slot requirements, craftable state, and a Forge action.
- Added `POST /api/account-forge/craft` for local account-state mutation.
- Added first `refinement_recipe` data table.
- Added instant Refinement Workbench crafting that consumes one equipment item plus one material item and writes equipment output back to the same local account-state file.
- Data Studio Operation tab now shows Refinement Workbench recipes, input equipment, material requirements, craftable state, and a Refine action.
- Added `POST /api/account-refinement/craft` for local account-state mutation.
- Added account-level equipment instances with stable instance ids and option lists.
- Forge equipment output now creates equipment instances instead of equipment item stacks.
- Refinement now consumes an equipment instance, preserves its existing options, adds the recipe effect option, and writes a new equipment instance.
- Operation UI now shows Equipment Instances and their options separately from stack inventory.
- Added `special_option_def` and `special_option_stat_delta` data tables for named non-stat-only equipment special options.
- Added sample special option `moonless_black_night`, with rarity, trigger key, effect summary, Moonlight stat delta, and granted skill reference.
- Equipment instances now store stat options and special options as separate collections.
- Refinement recipes can attach authored special options to the output equipment instance.
- Added `unit_special_option_loadout` as a preview/runtime bridge table so authored special options can be applied to sample unit definitions before the hero equipment assignment model exists.
- Battle config conversion now applies special option `on_equip` stat deltas to `UnitDef.base_stats` and can add non-duplicate granted-skill behavior rules.
- Added account heroes with equipment slots, defaulting from the sample map party when an account state is created or migrated.
- Added hero equip/unequip API and Operation UI controls for assigning equipment instances to a hero `main_hand` slot.
- Play Preview and `simulate --account-state` can now convert equipped hero items into runtime equipment modifiers and apply stat options plus special option keys to battle config.
- Equipment stat options affect combat only when their `stat_key` exists in `stat_def`; display/crafting-only option keys are ignored by combat runtime.
- Added combat special trigger runtime v1 for `combat_tick_5s_moonlight_3`: every 5 seconds it gains Moonlight, consumes 3 stacks, starts a 10-second periodic attack state, and hits the nearest enemy for 100% attack roughly every 0.5 seconds.
- Battle events now include `SpecialTriggered` so preview/UI layers can react to equipment special option triggers.
- Added `special_trigger_def` table so special option `trigger_key` values resolve to authored interval, stack stat, stack threshold, consume policy, duration, periodic interval, damage scale, and target rule data.
- `combat_tick_5s_moonlight_3` is now data-authored through `special_trigger_def`; the runtime still supports a narrow trigger shape, but its numbers and target rule come from data.
- Split special trigger authoring into `special_trigger_condition` and `special_trigger_effect` tables.
- `special_trigger_def` now references condition/effect rows; the Moonlight trigger is represented as an interval stat-delta effect, a stat threshold condition, and an on-trigger periodic damage effect.
- Special trigger runtime now supports condition kinds `always`, `stat_gte`, `stat_lte`, `stat_eq`, `target_exists`, and `target_in_range`.
- `special_trigger_condition` rows now carry `target_rule` and `range`, so equipment/special-option triggers can require a nearest enemy to exist or be inside line-combat range.
- Special trigger runtime now supports effect kinds `stat_delta`, `timed_stat_delta`, `instant_damage`, `periodic_damage`, and `cast_skill`; `stat_delta`, `timed_stat_delta`, and `cast_skill` can run on interval or on trigger.
- `timed_stat_delta` applies a stat change to `self` or `nearest_enemy` through `target_rule`, then reverts it after `duration_seconds`; this is the first buff/debuff preset for equipment special options.
- `special_trigger_effect.trigger_skill` references a `skill_def` row for `cast_skill`, allowing equipment/special-option triggers to fire authored skills through the normal skill execution path.
- `cast_skill` effects now have `pay_skill_cost` and `require_skill_cooldown` policy fields, so trigger-fired skills can be free extra effects or can respect the caster's shared skill cooldown.
- Added `special_option_skill_mutation` and `special_option_def.skill_mutations` so authored equipment special options can mutate a target skill for the equipped unit.
- Special option skill mutations currently clone the target skill into a unit-specific runtime skill, apply cooldown/range deltas, and support `damage_scale_add` for damage/projectile-damage effects without mutating the global source skill.
- Sample `moonless_black_night` now mutates Knight Slash into an empowered unit-specific version while keeping the original Knight Slash available for other users.
- Play Preview uses a fixed 1280x720 test layout with a 1280x676 canvas below the header; browser window changes no longer resize the gameplay layout.
- Play Preview has mouse-clickable playback controls for pause/play, restart, and speed; combat itself remains an automated data playback rather than direct unit control.
- Data Studio renders dropdown editors for common special trigger enum fields such as condition kind, effect kind, timing, and target rule.

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
- Operation energy display now shows stored energy, recoverable energy, max energy, recovery rate, and next recovery time.
- `/api/account-energy/recover` persists real-time account energy recovery, and dungeon dispatch applies recovery before spending energy.
- Account-state load/save paths now remove expired local mail automatically.
- Play Preview and Data Studio Operation tab now include the first Guild House visual shell with stacked expedition strips.
- Data Studio Operation tab now shows short Guild House visual feedback for dungeon dispatch, Alchemy Furnace, Forge, and Refinement Workbench actions.
- Data Studio Operation tab can craft `alchemy_furnace` recipes through `/api/account-alchemy/craft`, consuming ingredients and persisting crafted output.

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
- The shipped game's first screen is the Guild House, not an abstract menu.
- Guild House is the main visual scene for player work: alchemy furnace, forge, exit door, fantasy tavern bar, heroes, and active work feedback are visible there.
- Functional menus such as Warehouse, Hero, and Operation are UI overlays around the Guild House scene.
- Active dungeon explorations appear as narrow horizontal views on the left side; multiple parties stack vertically, with about six exploration views matching the Guild House plus UI height.
- Dungeon access can come from permanent basic dungeons or consumable map items.
- Basic dungeons are always open, practice-like, and wave endlessly with scaling enemies. Failure returns gathered rewards without loss, but there is no final success state.
- Maps are consumable items that open a timed dungeon entry. One opened dungeon accepts only one party at a time, but can be retried with the same or a different party until the map duration ends.
- Map dungeons target specific reward goals: item families, high-grade drops at low waves, boss-guaranteed reward grades, named equipment, special consumables, recipes, or map-only materials.
- Map fragments and complete maps can drop from basic dungeons, and fragments can be synthesized in the Alchemy Furnace.

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
  "equipment": [
    {
      "instance_id": "eq_1000_basic_sword_1",
      "item_key": "basic_sword",
      "display_name": "Basic Sword",
      "rarity": "common",
      "options": [
        { "stat_key": "strength", "value": 1, "rarity": "common" }
      ],
      "special_options": [
        {
          "option_key": "moonless_black_night",
          "name": "Moonless Black Night",
          "rarity": "legendary",
          "trigger_key": "combat_tick_5s_moonlight_3",
          "effect_summary": "Consumes 3 Moonlight to apply Moon's Wrath for 10 seconds.",
          "stat_deltas": [
            { "stat_key": "moonlight", "value": 1.0, "condition": "on_equip" }
          ],
          "granted_skill_key": "knight_slash"
        }
      ]
    }
  ],
  "mail": [
    { "item_key": "slime_gel", "quantity": 5, "expires_at_unix": 87400 }
  ]
}
```

`--write-account-state` defaults to `projects\sample\account_state.json` when `--project projects\sample` is used. Without `--write-account-state`, `--account-state <path>` only previews the account settlement.

## Immediate Next Milestone: Operation Account Loop v1

The next production-facing step is to expose the local account state in the UI and make it behave like the later server-backed account model:

- Account-state API endpoints in `belt_tools play` for preview/test workflows.
- Retire the temporary `unit_special_option_loadout` bridge once the account hero equipment path fully covers editor/preview sample needs.
- Add richer condition presets and buff/debuff authoring hints to the composable special trigger tables.
- Extend equipment special option mutations beyond `damage_scale_add` into effect add/remove/replace, projectile/AOE conversion, and richer conditional mutation presets.
- Add refinement effect rules for reroll/mutation instead of the current fixed sample option attachment.

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
- structured trigger timing runtime for conditional skill activation, including cooldown/resource checks and equipment special option triggers beyond periodic stack gain, threshold effects, and direct skill casts
- richer behavior conditions such as ally/enemy counts, cooldown availability, distance checks, and target stat filters
- richer resource flows around skill costs, such as mana gain effects, generated UI presets, and cost preview labels
- authoring presets and UI hints for temporary stacks, shields, richer buffs/debuffs, and over-time effects
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
