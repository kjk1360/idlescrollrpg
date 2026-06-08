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
cargo test
```

Package the current tool executable and sample project:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package_tools.ps1
dist\tools\belt_tools.exe data-status --project dist\projects\sample
dist\tools\belt_tools.exe view --project dist\projects\sample --view map_wave_preview
dist\tools\belt_tools.exe simulate --project dist\projects\sample --map endless_left_road
```

## Documentation

Future sessions should read these first:

- [Session Handoff](docs/SESSION_HANDOFF.md)
- [Plan](docs/PLAN.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Next Steps](docs/NEXT_STEPS.md)
