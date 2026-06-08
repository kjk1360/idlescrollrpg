# Belt Scroll RPG

Rust 기반 벨트스크롤 Idle RPG 런타임과, 해당 장르의 데이터를 빠르게 제작하기 위한 전용 Data Studio/Code Generate 툴체인입니다.

목표는 범용 게임 엔진이 아니라, 서비스형 벨트스크롤 Idle RPG를 만들기 위한 제한적이지만 강력한 제작 플랫폼입니다.

## Current Scope

- 왼쪽으로 무한 진행하는 2D 벨트스크롤 자동전투 코어
- 플레이어/적 유닛의 자동전투, 진형, 웨이브, 맵 데이터화
- 스키마 기반 시트형 데이터 모델
- 명시적 Code Generate 상태 판단
- 생성된 Rust 타입에 런타임 데이터를 로드하고 relation cache를 구성하는 방향

## Run

```powershell
cargo run -p belt_tools -- simulate
cargo run -p belt_tools -- simulate --project projects\sample --map endless_left_road
cargo run -p belt_tools -- data-status
cargo run -p belt_tools -- data-status --project projects\sample
cargo run -p belt_tools -- codegen-preview
cargo test
```

## Documentation

다른 세션에서 이어서 작업할 때는 아래 문서를 먼저 읽습니다.

- [Session Handoff](docs/SESSION_HANDOFF.md)
- [Plan](docs/PLAN.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Next Steps](docs/NEXT_STEPS.md)
