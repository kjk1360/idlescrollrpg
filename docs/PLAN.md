# Belt Scroll RPG Engine Plan

이 문서는 다음 세션에서 프로젝트를 이어받는 Codex/개발자가 현재 방향을 그대로 복원할 수 있도록 작성된 상위 계획서입니다.

## Product Direction

목표는 단순한 게임 1개가 아니라, 서비스 가능한 벨트스크롤 Idle RPG를 빠르게 만들고 업데이트하기 위한 전용 제작 플랫폼입니다.

핵심 방향:

- Rust 기반 런타임
- 왼쪽으로 무한 진행하는 벨트스크롤 자동전투
- 유닛, 스킬, 장비, 행동 패턴, 유닛 그룹, 웨이브, 맵을 데이터화
- 사람이 편하게 작업할 수 있는 시트형 Data Studio
- Codex가 API로 접근 가능한 데이터/코드생성/검증 파이프라인
- 런타임에서는 동적 클래스 생성 금지
- Data Studio schema 변경 후 명시적 Code Generate를 실행해 Rust 타입을 미리 생성
- 런타임은 생성된 타입에 데이터 인스턴스를 로드하고 relation cache를 구성

## Core Decisions

### Game First, Tool Together

범용 엔진을 먼저 만들지 않는다. 첫 게임의 실제 요구사항을 구현하면서 반복 작업이 필요한 영역을 도구화한다.

우선순위:

1. 벨트스크롤 자동전투 코어
2. 데이터 스키마/검증/코드생성
3. 파일 기반 데이터 프로젝트 저장
4. Web-like Data Studio UI
5. 렌더러/에디터/API 통합
6. 실제 게임 기획 데이터 제작

### Data Studio Is The Schema Source

Data Studio가 스키마 원본이다.

흐름:

```text
Data Studio에서 테이블/필드/관계 편집
-> 명시적으로 Code Generate 실행
-> generated Rust struct/enum/accessor/cache code 생성
-> 게임 빌드
-> 런타임에서 데이터 파일 로드
-> generated type에 deserialize
-> validation + relation cache 구성
-> 게임은 강타입 데이터 캐시만 사용
```

### Runtime Does Not Generate Classes

런타임은 새 클래스를 생성하지 않는다. 런타임은 이미 생성된 Rust 타입에 데이터를 채우고 캐싱한다.

### Schema Dirty And Data Dirty Are Separate

상태 판단은 최소 다음 값을 비교한다.

```text
schema_hash
generated_schema_hash
data_hash
built_data_hash
```

상태:

- `AllFresh`: 코드와 데이터 모두 최신
- `CodegenRequired`: schema가 generated code와 다름
- `DataBuildRequired`: row 데이터만 변경됨
- `CodegenAndDataBuildRequired`: schema와 data 모두 변경됨

UI에서는 Code Generate 버튼 색으로 표현한다.

- 회색/비활성: codegen 불필요
- 빨간색: schema와 generated code 불일치
- 파란색: 데이터 빌드만 필요
- 초록색: 모두 최신

## Phase 0: Foundation

현재 완료된 범위.

- Rust workspace 구성
- `belt_core`: 결정론적 전투/이동 시뮬레이션
- `data_studio_core`: Table/Field/Row schema, relation, nested 모델
- `belt_tools`: CLI/API 시작점
- 샘플 시뮬레이션과 데이터 상태 CLI

검증 명령:

```powershell
cargo fmt
cargo test
cargo run -p belt_tools -- simulate
cargo run -p belt_tools -- data-status
cargo run -p belt_tools -- codegen-preview
```

## Phase 1: Belt Scroll Combat Core

목표는 그래픽보다 먼저 테스트 가능한 자동전투 규칙을 안정화하는 것이다.

필수 모델:

- 월드 좌표: `x`는 진행축, `lane`은 깊이축
- 플레이어 파티는 왼쪽으로 이동
- 적 웨이브는 진행 방향 앞쪽에 배치
- 사거리 진입 시 이동 정지 후 자동 공격
- HP, 공격력, 공격 주기, 사거리, 이동속도
- 진형과 유닛 그룹
- 웨이브와 맵
- 맵 루프/무한 진행

다음 확장:

- 스킬 쿨다운
- 스킬 타겟팅
- hitbox/hurtbox
- 경직/슈퍼아머
- 넉백/끌어오기
- 행동 패턴
- 전투 로그/리플레이
- 결정론 테스트

## Phase 2: Data Studio Core

현재 초안 구현 완료. 다음은 실제 파일 기반 프로젝트화가 필요하다.

필드 타입 우선순위:

- Primitive: bool, i32, i64, f32, string, text
- Enum / flags enum
- AssetRef
- RelationOne
- RelationMany
- ReferenceGroup
- OwnedNestedTable

검증 우선순위:

- table key empty/duplicate
- field key empty/duplicate
- row key empty/duplicate
- missing required value
- missing relation target table
- missing relation target row
- enum value mismatch
- invalid nested table owner
- breaking schema change detection

## Phase 3: Code Generation

현재는 preview string만 생성한다. 다음 작업은 실제 파일 생성이다.

생성 대상:

- `generated/schema_types.rs`
- `generated/table_accessors.rs`
- `generated/relation_cache.rs`
- `generated/mod.rs`
- `generated/schema_fingerprint.json`

주의:

- generated 파일은 수동 편집 대상이 아니다.
- schema 변경 없이는 codegen을 다시 실행할 필요가 없어야 한다.
- schema diff와 breaking change report가 필요하다.

## Phase 4: Data Studio UI

UI는 Unity UGUI식이 아니라 웹 페이지식에 가깝게 만든다.

필수 화면:

- Table list
- Field/schema editor
- Row grid editor
- Relation picker
- Reference group editor
- Owned nested table editor
- Validate panel
- Code Generate / Build Data / Reload Runtime controls

Data Studio는 이후 Tauri 또는 웹 앱으로 구현 가능하다. 런타임과 도구 API는 우선 Rust CLI/API로 준비한다.

## Phase 5: Game Production Data

Data Studio가 작동한 후 실제 게임 데이터 구조를 만든다.

초기 테이블:

- UnitDef
- SkillDef
- EquipmentDef
- ActionPatternDef
- UnitFormation
- UnitGroup
- EnemyGroup
- WaveDef
- MapDef
- DropTable
- RewardBundle

구조 예:

```text
MapDef
-> waves: OwnedNestedTable or RelationMany<WaveDef>

WaveDef
-> enemy_groups: RelationMany<EnemyGroup>

EnemyGroup
-> formation: RelationOne<UnitFormation>
-> units: OwnedNestedTable<GroupUnitEntry>

GroupUnitEntry
-> unit: RelationOne<UnitDef>
-> slot/lane/x offset
```

## Phase 6: Renderer And Runtime App

데이터와 코어가 안정화된 뒤 붙인다.

후보:

- Bevy
- macroquad
- wgpu custom layer

초기 목표:

- 2D sprite placeholder
- belt lane depth sort
- camera follows endless-left progress
- combat event visualization
- debug overlay

