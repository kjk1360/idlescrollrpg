# Next Steps

## Immediate Task: File-Based Data Project

현재 `data_studio_core::sample_project()`는 메모리 샘플이다. 다음 단계는 실제 파일 기반 프로젝트를 만드는 것이다.

권장 구조:

```text
projects/
  sample/
    project.json
    schema/
      tables.json
    data/
      unit_def.json
      unit_group.json
    build/
      generated_schema_fingerprint.json
      built_data_fingerprint.json
```

## Step 1: Add Serialization

`data_studio_core`에 `serde`, `serde_json`을 추가한다.

대상 타입:

- `TableId`
- `FieldId`
- `RowId`
- `FieldKind`
- `FieldSchema`
- `TableSchema`
- `CellValue`
- `RowData`
- `TableData`
- `DataProject`

주의:

- `CellValue::F32` hash는 `to_bits()` 기준을 유지한다.
- ID 타입은 JSON에서 숫자 또는 string wrapper 중 하나로 일관되게 정한다.

## Step 2: Project Loader

추가할 API:

```rust
impl DataProject {
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, DataProjectError>;
    pub fn save_to_dir(&self, path: impl AsRef<Path>) -> Result<(), DataProjectError>;
}
```

초기에는 단순 JSON이면 충분하다.

## Step 3: CLI Project Argument

현재:

```powershell
cargo run -p belt_tools -- data-status
```

목표:

```powershell
cargo run -p belt_tools -- data-status --project projects/sample
cargo run -p belt_tools -- validate --project projects/sample
```

## Step 4: Real Codegen Output

현재:

```powershell
cargo run -p belt_tools -- codegen-preview
```

목표:

```powershell
cargo run -p belt_tools -- codegen --project projects/sample --out crates/generated_data/src
```

생성 파일:

```text
crates/generated_data/
  Cargo.toml
  src/
    lib.rs
    schema_types.rs
    table_accessors.rs
    relation_cache.rs
    schema_fingerprint.json
```

## Step 5: Data Build

목표:

```powershell
cargo run -p belt_tools -- data-build --project projects/sample --out build/data
```

초기 산출물은 JSON snapshot이어도 된다. 나중에 binary snapshot으로 바꿀 수 있다.

## Step 6: Relation Cache

빌드 시점 또는 런타임 로드 시점에 다음을 수행한다.

- RelationOne target row 확인
- RelationMany target rows 확인
- ReferenceGroup target rows 확인
- OwnedNestedTable owner 관계 확인
- 빠른 lookup map 구성

## Step 7: Combat Data Integration

`belt_core::sample_battle_config()`를 파일 데이터 기반으로 대체한다.

필요 테이블:

- UnitDef
- UnitGroup
- WaveDef
- MapDef

목표:

```powershell
cargo run -p belt_tools -- simulate --project projects/sample --map endless_left_road
```

## Step 8: Visual Prototype Later

렌더러는 위 데이터 파이프라인이 안정화된 뒤 붙인다.

초기 렌더링 목표:

- placeholder rectangle/sprite
- lane depth sort
- camera follows left progress
- combat event text overlay

