# Belt Scroll RPG

Rust-first belt-scroll Idle RPG runtime plus a dedicated Data Studio / Code Generate toolchain.

The goal is not a generic game engine. The goal is a focused production platform for a serviceable belt-scroll Idle RPG.

## Current Scope

- Endless-left belt-scroll auto-battle core
- File-based data project
- Table/field/row schema model
- Relation fields and reference groups
- DataView join preview
- Explicit Code Generate and Data Build flow
- Generated Rust data crate
- Visual data tables for texture, sprite animation, visual state machine, and unit visual
- Local playable canvas preview

## Run

Run from the workspace root:

```powershell
cd C:\Users\Cookapps\belt-scroll-rpg
```

Useful commands:

```powershell
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
cargo test
```

`import-aseprite` accepts `.aseprite`/`.ase` files when the Aseprite CLI is installed. It also accepts an already exported Aseprite JSON file and imports its sheet image, frame rectangles, and frame tags into `texture_asset`, `sprite_frame`, and `sprite_animation`.

Package the current tool executable and sample project:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package_tools.ps1
dist\tools\belt_tools.exe data-status --project dist\projects\sample
dist\tools\belt_tools.exe view --project dist\projects\sample --view map_wave_preview
dist\tools\belt_tools.exe simulate --project dist\projects\sample --map endless_left_road
dist\tools\belt_tools.exe serve --project dist\projects\sample --addr 127.0.0.1:7878
dist\tools\belt_tools.exe play --project dist\projects\sample --map endless_left_road --addr 127.0.0.1:7879
```

## Documentation

Future sessions should read these first:

- [Session Handoff](docs/SESSION_HANDOFF.md)
- [Plan](docs/PLAN.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Next Steps](docs/NEXT_STEPS.md)
