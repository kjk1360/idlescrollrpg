# Architecture

## Workspace

```text
belt-scroll-rpg/
  Cargo.toml
  README.md
  docs/
    PLAN.md
    ARCHITECTURE.md
    SESSION_HANDOFF.md
    NEXT_STEPS.md
  crates/
    belt_core/
    data_studio_core/
    tools/
```

## Crates

### `belt_core`

역할:

- 벨트스크롤 자동전투 시뮬레이션
- 유닛, 진형, 웨이브, 맵의 런타임 모델
- 결정론적 tick 기반 전투 진행
- 그래픽 렌더러와 분리된 순수 코어

현재 구현:

- `UnitDef`
- `UnitGroup`
- `WaveDef`
- `MapDef`
- `BattleWorld`
- `BattleEvent`
- `sample_battle_config`

중요한 현재 규칙:

- 플레이어는 기본적으로 왼쪽으로 이동한다.
- 적이 있으면 가장 가까운 적을 찾는다.
- 사거리 밖이면 접근한다.
- 사거리 안이면 공격한다.
- 적 웨이브가 전멸하면 다음 웨이브를 시작한다.
- 모든 웨이브가 끝나면 같은 맵을 loop한다.

### `data_studio_core`

역할:

- 시트형 데이터 툴의 도메인 모델
- schema/data fingerprint
- validation
- codegen preview

현재 구현:

- `TableId`, `FieldId`, `RowId`
- `FieldKind`
- `FieldSchema`
- `TableSchema`
- `CellValue`
- `RowData`
- `TableData`
- `DataProject`
- `ProjectFingerprints`
- `ProjectStatus`

현재 지원 필드:

- Bool
- I32
- I64
- F32
- String
- Text
- Enum
- AssetRef
- RelationOne
- RelationMany
- ReferenceGroup
- OwnedNestedTable

### `belt_tools`

역할:

- 현재는 CLI
- 이후 로컬 API 서버와 Data Studio 백엔드로 확장

현재 명령:

```powershell
cargo run -p belt_tools -- simulate
cargo run -p belt_tools -- data-status
cargo run -p belt_tools -- codegen-preview
```

## Data Pipeline

의도한 최종 흐름:

```text
schema files + row data files
-> validate
-> codegen
-> generated Rust code
-> cargo build
-> data build
-> runtime load
-> relation cache
-> game systems
```

## Codegen Button Semantics

Codegen은 자동 실행이 아니라 명시적 작업이다.

상태 계산:

```rust
schema_hash != generated_schema_hash => CodegenRequired
data_hash != built_data_hash => DataBuildRequired
```

Data Studio UI에서는 이 상태를 버튼 색상과 status badge로 노출한다.

## Nested Group Semantics

두 종류가 필요하다.

### Reference Group

기존 독립 테이블의 row들을 선택해 묶는다.

예:

- Skill.allowed_weapons -> WeaponType rows
- Stage.monster_pool -> Monster rows
- EventShop.products -> Item rows

원본 row 수정은 대상 테이블에서만 한다.

### Owned Nested Table

상위 row에 종속된 하위 row 목록이다.

예:

- DropTable.entries
- Skill.hit_frames
- Stage.waves
- Quest.conditions
- Monster.ai_phases

부모 row를 통해서만 편집한다. 부모 삭제 시 함께 삭제한다.

