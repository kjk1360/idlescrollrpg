use belt_core::{BattleEvent, BattleWorld, Team, UnitDefId};
use data_studio_core::{CellValue, DataProject, FieldId, RowData, RowId, TableId};
use game_data_adapter::battle_config_from_project;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

pub(crate) fn play(args: &[String]) -> Result<(), String> {
    let project_path = crate::option_value_for_args(args, "--project")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("projects/sample"));
    let addr = crate::option_value_for_args(args, "--addr").unwrap_or("127.0.0.1:7879");
    let map_key = crate::option_value_for_args(args, "--map")
        .unwrap_or("endless_left_road")
        .to_string();

    let listener =
        TcpListener::bind(addr).map_err(|error| format!("failed to bind {addr}: {error}"))?;
    println!("Playable Preview: http://{addr}");
    println!("Project: {}", project_path.display());
    println!("Map: {map_key}");

    let state = PlayServerState {
        project_path,
        map_key,
    };

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(stream, &state) {
                    eprintln!("play error: {error}");
                }
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }

    Ok(())
}

struct PlayServerState {
    project_path: PathBuf,
    map_key: String,
}

struct Request {
    method: String,
    path: String,
}

fn handle_stream(mut stream: TcpStream, state: &PlayServerState) -> Result<(), String> {
    let request = read_request(&mut stream)?;
    let response = route_request(&request, state);
    stream
        .write_all(&response)
        .map_err(|error| format!("failed to write response: {error}"))
}

fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut buffer = Vec::new();
    let mut temp = [0; 4096];

    loop {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Err("empty request".to_string());
        }
        buffer.extend_from_slice(&temp[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 1024 * 1024 {
            return Err("request headers too large".to_string());
        }
    }

    let text = String::from_utf8_lossy(&buffer);
    let request_line = text
        .lines()
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .to_string();

    Ok(Request { method, path })
}

fn route_request(request: &Request, state: &PlayServerState) -> Vec<u8> {
    let result = match (request.method.as_str(), path_without_query(&request.path)) {
        ("GET", "/") => Ok(html(PLAY_HTML)),
        ("GET", "/api/play") => api_play(state),
        _ => Err(("not found".to_string(), 404)),
    };

    match result {
        Ok(response) => response,
        Err((message, status)) => json_response(status, &json!({ "ok": false, "error": message })),
    }
}

fn api_play(state: &PlayServerState) -> Result<Vec<u8>, (String, u16)> {
    let project = DataProject::load_from_dir(&state.project_path)
        .map_err(|error| (error.to_string(), 500))?;
    let playback = build_playback(&project, &state.map_key).map_err(|error| (error, 500))?;
    Ok(json_response(200, &playback))
}

fn build_playback(project: &DataProject, map_key: &str) -> Result<Value, String> {
    let config = battle_config_from_project(project, map_key)?;
    let visuals = VisualLookup::new(project);
    let mut world = BattleWorld::new(config);
    let mut frames = Vec::new();
    let mut last_actions = std::collections::HashMap::new();
    let dt = 0.1_f32;

    for index in 0_usize..420 {
        world.tick(dt);
        for event in world.drain_events() {
            match event {
                BattleEvent::UnitMoved { unit_id, .. } => {
                    last_actions.insert(unit_id.0, ("move".to_string(), index));
                }
                BattleEvent::UnitAttacked { attacker, .. } => {
                    last_actions.insert(attacker.0, ("attack".to_string(), index));
                }
                _ => {}
            }
        }

        let units = world
            .units()
            .iter()
            .map(|unit| {
                let action = last_actions
                    .get(&unit.id.0)
                    .filter(|(_, frame)| index.saturating_sub(*frame) <= 5)
                    .map(|(action, _)| action.as_str())
                    .unwrap_or("idle");
                let visual = visuals.unit_visual(unit.def_id);
                json!({
                    "id": unit.id.0,
                    "def_id": unit.def_id.0,
                    "name": unit.name,
                    "team": team_label(unit.team),
                    "hp": unit.hp,
                    "max_hp": unit.max_hp,
                    "x": unit.position.x,
                    "lane": unit.position.lane,
                    "state": action,
                    "visual": visual,
                })
            })
            .collect::<Vec<_>>();

        frames.push(json!({
            "t": (index as f32) * dt,
            "units": units,
        }));
    }

    Ok(json!({
        "ok": true,
        "map": map_key,
        "frames": frames,
    }))
}

fn team_label(team: Team) -> &'static str {
    match team {
        Team::Player => "player",
        Team::Enemy => "enemy",
    }
}

struct VisualLookup<'a> {
    project: &'a DataProject,
}

impl<'a> VisualLookup<'a> {
    fn new(project: &'a DataProject) -> Self {
        Self { project }
    }

    fn unit_visual(&self, def_id: UnitDefId) -> Value {
        let Some(unit_row) = self.row(TableId(1), RowId(def_id.0 as u64)) else {
            return default_visual();
        };
        let Some(visual_id) = row_cell(unit_row, FieldId(7)).and_then(cell_row) else {
            return default_visual();
        };
        let Some(visual_row) = self.row(TableId(10), visual_id) else {
            return default_visual();
        };
        let state_machine_id = row_cell(visual_row, FieldId(81))
            .and_then(cell_row)
            .unwrap_or(RowId(0));
        let state_machine = self.row(TableId(8), state_machine_id);
        let states = state_machine
            .and_then(|row| row_cell(row, FieldId(62)).and_then(cell_rows))
            .unwrap_or_default()
            .iter()
            .filter_map(|state_id| self.visual_state(*state_id))
            .collect::<Vec<_>>();

        json!({
            "name": row_cell(visual_row, FieldId(80)).and_then(cell_string).unwrap_or("Visual"),
            "scale": row_cell(visual_row, FieldId(82)).and_then(cell_f32).unwrap_or(1.0),
            "shadow_radius": row_cell(visual_row, FieldId(83)).and_then(cell_f32).unwrap_or(16.0),
            "body_color": row_cell(visual_row, FieldId(84)).and_then(cell_string).unwrap_or("#999999"),
            "states": states,
        })
    }

    fn visual_state(&self, state_id: RowId) -> Option<Value> {
        let row = self.row(TableId(9), state_id)?;
        let animation_id = row_cell(row, FieldId(72)).and_then(cell_row)?;
        let animation = self.row(TableId(7), animation_id);
        Some(json!({
            "key": row_cell(row, FieldId(71)).and_then(cell_string).unwrap_or("idle"),
            "animation": {
                "frame_count": animation.and_then(|row| row_cell(row, FieldId(52)).and_then(cell_i32)).unwrap_or(4),
                "fps": animation.and_then(|row| row_cell(row, FieldId(53)).and_then(cell_f32)).unwrap_or(6.0),
                "loop": animation.and_then(|row| row_cell(row, FieldId(54)).and_then(cell_bool)).unwrap_or(true),
            }
        }))
    }

    fn row(&self, table_id: TableId, row_id: RowId) -> Option<&'a RowData> {
        self.project
            .data
            .iter()
            .find(|table| table.table_id == table_id)?
            .rows
            .iter()
            .find(|row| row.id == row_id)
    }
}

fn default_visual() -> Value {
    json!({
        "name": "Default",
        "scale": 1.0,
        "shadow_radius": 16.0,
        "body_color": "#999999",
        "states": []
    })
}

fn row_cell(row: &RowData, field_id: FieldId) -> Option<&CellValue> {
    row.cells.get(&field_id)
}

fn cell_string(value: &CellValue) -> Option<&str> {
    match value {
        CellValue::String(value) => Some(value),
        _ => None,
    }
}

fn cell_i32(value: &CellValue) -> Option<i32> {
    match value {
        CellValue::I32(value) => Some(*value),
        _ => None,
    }
}

fn cell_f32(value: &CellValue) -> Option<f32> {
    match value {
        CellValue::F32(value) => Some(*value),
        _ => None,
    }
}

fn cell_bool(value: &CellValue) -> Option<bool> {
    match value {
        CellValue::Bool(value) => Some(*value),
        _ => None,
    }
}

fn cell_row(value: &CellValue) -> Option<RowId> {
    match value {
        CellValue::Row(value) => Some(*value),
        _ => None,
    }
}

fn cell_rows(value: &CellValue) -> Option<Vec<RowId>> {
    match value {
        CellValue::Rows(value) => Some(value.clone()),
        _ => None,
    }
}

fn path_without_query(path: &str) -> &str {
    path.split_once('?').map(|(path, _)| path).unwrap_or(path)
}

fn html(content: &str) -> Vec<u8> {
    response(200, "text/html; charset=utf-8", content.as_bytes())
}

fn json_response(status: u16, value: &Value) -> Vec<u8> {
    response(
        status,
        "application/json; charset=utf-8",
        value.to_string().as_bytes(),
    )
}

fn response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

const PLAY_HTML: &str = r#"<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Belt RPG Play Preview</title>
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: #1b2027;
      color: #e8edf2;
      font-family: Segoe UI, system-ui, sans-serif;
      overflow: hidden;
    }
    header {
      height: 44px;
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 0 14px;
      border-bottom: 1px solid #343c47;
      background: #111820;
    }
    h1 { margin: 0; font-size: 15px; }
    .pill {
      border: 1px solid #3f4a57;
      border-radius: 6px;
      padding: 4px 8px;
      color: #aeb8c4;
      font-size: 12px;
    }
    canvas {
      display: block;
      width: 100vw;
      height: calc(100vh - 44px);
      background: #27313a;
    }
  </style>
</head>
<body>
  <header>
    <h1>Belt RPG Play Preview</h1>
    <span id="map" class="pill">loading</span>
    <span id="time" class="pill">0.0s</span>
  </header>
  <canvas id="game"></canvas>
  <script>
    const canvas = document.getElementById('game');
    const ctx = canvas.getContext('2d');
    let playback = null;
    let start = performance.now();

    function resize() {
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.floor(canvas.clientWidth * dpr);
      canvas.height = Math.floor(canvas.clientHeight * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    window.addEventListener('resize', resize);
    resize();

    fetch('/api/play')
      .then(res => res.json())
      .then(data => {
        playback = data;
        document.getElementById('map').textContent = data.map;
        requestAnimationFrame(loop);
      });

    function loop(now) {
      if (!playback) return;
      const elapsed = ((now - start) / 1000) % playback.frames.at(-1).t;
      const frame = playback.frames.reduce((prev, next) => next.t <= elapsed ? next : prev, playback.frames[0]);
      draw(frame, elapsed);
      requestAnimationFrame(loop);
    }

    function draw(frame, elapsed) {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      ctx.clearRect(0, 0, w, h);
      drawBackground(w, h, elapsed);
      const sorted = [...frame.units].sort((a, b) => a.lane - b.lane);
      for (const unit of sorted) drawUnit(unit, elapsed, w, h);
      document.getElementById('time').textContent = `${elapsed.toFixed(1)}s / units ${frame.units.length}`;
    }

    function drawBackground(w, h, t) {
      const horizon = Math.floor(h * 0.24);
      const floorTop = Math.floor(h * 0.44);
      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, '#2f3e4c');
      grad.addColorStop(0.42, '#202933');
      grad.addColorStop(1, '#182027');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, w, h);
      ctx.fillStyle = '#354131';
      ctx.fillRect(0, floorTop, w, h - floorTop);
      ctx.strokeStyle = '#526241';
      ctx.lineWidth = 1;
      for (let i = -2; i < 18; i++) {
        const x = ((i * 120 + t * 80) % 120) - 120;
        ctx.beginPath();
        ctx.moveTo(x, h);
        ctx.lineTo(x + 180, floorTop);
        ctx.stroke();
      }
      for (let lane = -1; lane <= 1; lane += 0.5) {
        const y = laneY(lane, h);
        ctx.strokeStyle = 'rgba(255,255,255,0.08)';
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }
      ctx.fillStyle = 'rgba(0,0,0,0.28)';
      ctx.fillRect(0, horizon, w, 4);
    }

    function drawUnit(unit, t, w, h) {
      const x = w * 0.5 - unit.x * 42;
      const y = laneY(unit.lane, h);
      const scale = Number(unit.visual.scale || 1);
      const radius = Number(unit.visual.shadow_radius || 16) * scale;
      const state = visualState(unit.visual, unit.state);
      const anim = state?.animation || { frame_count: 4, fps: 6 };
      const frameIndex = Math.floor(t * anim.fps) % Math.max(1, anim.frame_count);
      const bob = Math.sin((frameIndex / Math.max(1, anim.frame_count)) * Math.PI * 2) * 3;
      const attackLean = unit.state === 'attack' ? (unit.team === 'player' ? -8 : 8) : 0;
      ctx.save();
      ctx.translate(x + attackLean, y + bob);
      ctx.fillStyle = 'rgba(0,0,0,0.32)';
      ctx.beginPath();
      ctx.ellipse(0, 24 * scale, radius, radius * 0.36, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = unit.visual.body_color || '#999999';
      ctx.strokeStyle = unit.team === 'player' ? '#d9efff' : '#ffe0d6';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.roundRect(-16 * scale, -30 * scale, 32 * scale, 48 * scale, 8 * scale);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = '#111820';
      ctx.font = '11px Segoe UI';
      ctx.textAlign = 'center';
      ctx.fillText(unit.name, 0, -38 * scale);
      drawHp(unit, scale);
      ctx.restore();
    }

    function drawHp(unit, scale) {
      const width = 42 * scale;
      const pct = Math.max(0, unit.hp / unit.max_hp);
      ctx.fillStyle = '#111820';
      ctx.fillRect(-width / 2, -50 * scale, width, 5);
      ctx.fillStyle = unit.team === 'player' ? '#58b368' : '#d95757';
      ctx.fillRect(-width / 2, -50 * scale, width * pct, 5);
    }

    function laneY(lane, h) {
      return h * 0.67 + lane * h * 0.12;
    }

    function visualState(visual, key) {
      return (visual.states || []).find(state => state.key === key)
        || (visual.states || []).find(state => state.key === 'idle')
        || null;
    }
  </script>
</body>
</html>
"#;
