# Session Handoff

이 문서는 다음 Codex 세션이 현재 세션과 같은 맥락으로 이어서 작업하기 위한 인수인계 파일입니다.

## User Goal

사용자는 Rust를 기본으로, 던파 같은 2D 벨트스크롤이지만 깊이축 이동을 가진 Idle RPG를 만들고 싶어한다.

단순 게임이 아니라 다음을 목표로 한다.

- 벨트스크롤 RPG 전용 제작 엔진/툴체인
- 알만툴처럼 장르 특화된 제한적이지만 편의성 높은 제작 환경
- 서비스 가능한 게임 제작과 2년 이상 운영을 고려한 데이터/툴 구조
- Data Studio에서 시각적으로 데이터 입력
- Codex도 API로 데이터/툴에 접근 가능
- UI 제작은 Unity UGUI보다 웹 페이지 제작 방식에 가깝게

## Key Design Agreement

합의된 방향:

- 게임을 바로 만들되, 반복 작업은 처음부터 툴화한다.
- 범용 엔진이 아니라 벨트스크롤 Idle RPG 제작 플랫폼으로 제한한다.
- 데이터는 시트형 툴을 원본으로 한다.
- 신규 필드/테이블 스키마를 추가하면 명시적으로 Code Generate를 실행한다.
- Code Generate 결과로 Rust struct/enum/accessor/cache 코드를 미리 만든다.
- 런타임은 새 클래스를 만들지 않고 생성된 타입에 데이터를 로드하고 캐싱한다.
- schema와 generated code가 다르면 Code Generate 버튼이 빨간색이어야 한다.
- 데이터만 바뀌면 Code Generate는 필요 없고 Data Build만 필요하다.

## Current Repository State

경로:

```powershell
C:\Users\Cookapps\belt-scroll-rpg
```

원격 저장소:

```text
https://github.com/kjk1360/idlescrollrpg.git
```

현재 완료:

- Rust workspace 생성
- `belt_core` 구현
- `data_studio_core` 구현
- `belt_tools` CLI 구현
- 계획/아키텍처/인수인계 문서 작성

검증 완료:

```powershell
cargo fmt
cargo test
cargo run -p belt_tools -- simulate
cargo run -p belt_tools -- data-status
```

테스트 결과:

- `belt_core`: 1개 통과
- `data_studio_core`: 3개 통과
- 전체 테스트 성공

## Important Files

- [README.md](../README.md)
- [PLAN.md](PLAN.md)
- [ARCHITECTURE.md](ARCHITECTURE.md)
- [NEXT_STEPS.md](NEXT_STEPS.md)
- [belt_core/src/lib.rs](../crates/belt_core/src/lib.rs)
- [data_studio_core/src/lib.rs](../crates/data_studio_core/src/lib.rs)
- [tools/src/main.rs](../crates/tools/src/main.rs)

## How To Resume

다음 세션 시작 시 먼저 실행:

```powershell
cd C:\Users\Cookapps\belt-scroll-rpg
git status --short
cargo test
```

그 다음 읽어야 할 문서:

1. `docs/SESSION_HANDOFF.md`
2. `docs/PLAN.md`
3. `docs/ARCHITECTURE.md`
4. `docs/NEXT_STEPS.md`

## Recommended Next Task

다음 작업은 `data_studio_core`를 파일 기반 프로젝트로 확장하는 것이다.

구체적으로:

1. `projects/sample/` 같은 샘플 데이터 프로젝트 폴더 추가
2. schema/data 저장 포맷 결정
3. JSON 또는 RON serialization 도입
4. `belt_tools data-status --project projects/sample` 구현
5. `belt_tools codegen --project projects/sample --out crates/generated_data/src` 구현
6. generated code fingerprint 저장
7. schema 변경과 generated code 불일치 상태를 실제 파일로 판단

## Current Caveats

- 아직 렌더러는 없다.
- 아직 Data Studio UI는 없다.
- codegen은 현재 문자열 preview만 생성한다.
- data/status fingerprint는 아직 메모리 샘플 프로젝트 기준이다.
- Git 저장소 초기화와 push는 이 문서 작성 후 진행된다.

