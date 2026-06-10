use belt_core::{BattleEvent, BattleWorld, GridPosition, Team, UnitDefId};
use data_studio_core::{CellValue, DataProject, FieldId, RowData, RowId, TableId};
use game_data_adapter::battle_config_from_project_with_runtime_equipment;
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
        ("GET", "/asset") => asset(request, state),
        ("GET", "/api/play") => api_play(state),
        _ => Err(("not found".to_string(), 404)),
    };

    match result {
        Ok(response) => response,
        Err((message, status)) => json_response(status, &json!({ "ok": false, "error": message })),
    }
}

fn asset(request: &Request, state: &PlayServerState) -> Result<Vec<u8>, (String, u16)> {
    let raw_path =
        query_value(&request.path, "path").ok_or_else(|| ("missing path".to_string(), 400))?;
    if raw_path.contains("..") || raw_path.starts_with('/') || raw_path.starts_with('\\') {
        return Err(("invalid asset path".to_string(), 400));
    }
    let path = state.project_path.join(raw_path.replace('/', "\\"));
    let bytes = std::fs::read(&path).map_err(|error| {
        (
            format!("failed to read asset {}: {error}", path.display()),
            404,
        )
    })?;
    Ok(response(200, content_type_for_path(&path), &bytes))
}

fn api_play(state: &PlayServerState) -> Result<Vec<u8>, (String, u16)> {
    let project = DataProject::load_from_dir(&state.project_path)
        .map_err(|error| (error.to_string(), 500))?;
    let playback = build_playback(&project, &state.project_path, &state.map_key)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &playback))
}

fn build_playback(
    project: &DataProject,
    project_path: &std::path::Path,
    map_key: &str,
) -> Result<Value, String> {
    let runtime_equipment = crate::runtime_equipment_modifiers_for_project(project_path)?;
    let config =
        battle_config_from_project_with_runtime_equipment(project, map_key, &runtime_equipment)?;
    let visuals = VisualLookup::new(project);
    let mut world = BattleWorld::new(config);
    let mut frames = Vec::new();
    let mut last_actions = std::collections::HashMap::new();
    let mut area_effects = Vec::new();
    let mut projectiles = Vec::new();
    let dt = 0.1_f32;

    for index in 0_usize..420 {
        world.tick(dt);
        let frame_time = (index as f32) * dt;
        for event in world.drain_events() {
            match event {
                BattleEvent::UnitMoved { unit_id, .. } => {
                    last_actions.insert(unit_id.0, ("move".to_string(), index));
                }
                BattleEvent::UnitAttacked { attacker, .. } => {
                    last_actions.insert(attacker.0, ("attack".to_string(), index));
                }
                BattleEvent::UnitKilled { unit_id } => {
                    last_actions.insert(unit_id.0, ("dead".to_string(), index));
                }
                BattleEvent::SkillAreaEffect { cells } => {
                    area_effects.push(AreaEffectPreview {
                        start: frame_time,
                        end: frame_time + 0.28,
                        cells,
                    });
                }
                BattleEvent::ProjectileLaunched {
                    caster,
                    from,
                    to,
                    duration,
                } => {
                    projectiles.push(ProjectilePreview {
                        caster: caster.0,
                        start: frame_time,
                        end: frame_time + duration.max(dt),
                        from,
                        to,
                    });
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
        let effects = area_effects
            .iter()
            .filter(|effect| effect.start <= frame_time && effect.end >= frame_time)
            .map(area_effect_json)
            .collect::<Vec<_>>();
        let frame_projectiles = projectiles
            .iter()
            .filter(|projectile| projectile.start <= frame_time && projectile.end >= frame_time)
            .map(projectile_json)
            .collect::<Vec<_>>();

        frames.push(json!({
            "t": frame_time,
            "units": units,
            "effects": effects,
            "projectiles": frame_projectiles,
        }));
    }

    Ok(json!({
        "ok": true,
        "map": map_key,
        "frames": frames,
    }))
}

#[derive(Debug, Clone)]
struct AreaEffectPreview {
    start: f32,
    end: f32,
    cells: Vec<GridPosition>,
}

#[derive(Debug, Clone)]
struct ProjectilePreview {
    caster: u64,
    start: f32,
    end: f32,
    from: GridPosition,
    to: GridPosition,
}

fn area_effect_json(effect: &AreaEffectPreview) -> Value {
    json!({
        "kind": "area_flash",
        "start": effect.start,
        "end": effect.end,
        "cells": effect.cells.iter().map(grid_json).collect::<Vec<_>>(),
    })
}

fn projectile_json(projectile: &ProjectilePreview) -> Value {
    json!({
        "kind": "red_orb",
        "caster": projectile.caster,
        "start": projectile.start,
        "end": projectile.end,
        "from": grid_json(&projectile.from),
        "to": grid_json(&projectile.to),
    })
}

fn grid_json(position: &GridPosition) -> Value {
    json!({
        "x": position.x,
        "lane": position.lane,
    })
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
        let frames = animation
            .and_then(|row| row_cell(row, FieldId(55)).and_then(cell_rows))
            .unwrap_or_default()
            .iter()
            .filter_map(|frame_id| self.sprite_frame(*frame_id))
            .collect::<Vec<_>>();
        Some(json!({
            "key": row_cell(row, FieldId(71)).and_then(cell_string).unwrap_or("idle"),
            "animation": {
                "frame_count": animation.and_then(|row| row_cell(row, FieldId(52)).and_then(cell_i32)).unwrap_or(4),
                "fps": animation.and_then(|row| row_cell(row, FieldId(53)).and_then(cell_f32)).unwrap_or(6.0),
                "loop": animation.and_then(|row| row_cell(row, FieldId(54)).and_then(cell_bool)).unwrap_or(true),
                "frames": frames,
            }
        }))
    }

    fn sprite_frame(&self, frame_id: RowId) -> Option<Value> {
        let row = self.row(TableId(11), frame_id)?;
        let texture_id = row_cell(row, FieldId(91)).and_then(cell_row)?;
        let texture = self.row(TableId(6), texture_id);
        Some(json!({
            "x": row_cell(row, FieldId(92)).and_then(cell_i32).unwrap_or(0),
            "y": row_cell(row, FieldId(93)).and_then(cell_i32).unwrap_or(0),
            "w": row_cell(row, FieldId(94)).and_then(cell_i32).unwrap_or(64),
            "h": row_cell(row, FieldId(95)).and_then(cell_i32).unwrap_or(64),
            "pivot_x": row_cell(row, FieldId(96)).and_then(cell_f32).unwrap_or(0.5),
            "pivot_y": row_cell(row, FieldId(97)).and_then(cell_f32).unwrap_or(0.85),
            "texture": {
                "path": texture.and_then(|row| row_cell(row, FieldId(45)).and_then(cell_string)).unwrap_or(""),
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

fn query_value<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let query = path.split_once('?')?.1;
    query.split('&').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name == key).then_some(value)
    })
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

fn content_type_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
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
      overflow: auto;
    }
    .stage {
      width: 1280px;
      height: 720px;
      background: #27313a;
      box-shadow: 0 0 0 1px #343c47;
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
      width: 1280px;
      height: 676px;
      background: #27313a;
    }
  </style>
</head>
<body>
  <div class="stage">
    <header>
      <h1>Belt RPG Play Preview</h1>
      <span id="map" class="pill">loading</span>
      <span id="time" class="pill">0.0s</span>
      <span class="pill">1280x720 fixed test layout</span>
    </header>
    <canvas id="game" width="1280" height="676"></canvas>
  </div>
  <script>
    const canvas = document.getElementById('game');
    const ctx = canvas.getContext('2d');
    const TEST_WIDTH = 1280;
    const TEST_HEIGHT = 676;
    const GRID_CELL_W = 46;
    const GRID_CELL_H = 34;
    let playback = null;
    const images = {};
    let start = performance.now();

    function resize() {
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.floor(TEST_WIDTH * dpr);
      canvas.height = Math.floor(TEST_HEIGHT * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
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
      const sample = samplePlayback(elapsed);
      draw(sample, elapsed);
      requestAnimationFrame(loop);
    }

    function samplePlayback(elapsed) {
      let index = 0;
      while (index + 1 < playback.frames.length && playback.frames[index + 1].t <= elapsed) index++;
      const current = playback.frames[index] || playback.frames[0];
      const next = playback.frames[index + 1] || current;
      const span = Math.max(0.001, next.t - current.t);
      const alpha = Math.max(0, Math.min(1, (elapsed - current.t) / span));
      const previousById = new Map((current.units || []).map(unit => [unit.id, unit]));
      const units = (next.units || current.units || []).map(unit => {
        const previous = previousById.get(unit.id) || unit;
        return {
          ...unit,
          render_x: lerp(previous.x, unit.x, alpha),
          render_lane: lerp(previous.lane, unit.lane, alpha)
        };
      });
      return { ...next, units };
    }

    function lerp(a, b, t) {
      return Number(a || 0) + (Number(b || 0) - Number(a || 0)) * t;
    }

    function draw(frame, elapsed) {
      const w = TEST_WIDTH;
      const h = TEST_HEIGHT;
      ctx.clearRect(0, 0, w, h);
      drawBackground(w, h, elapsed);
      drawAreaEffects(frame.effects || [], elapsed, w, h);
      drawProjectiles(frame.projectiles || [], elapsed, w, h);
      const sorted = [...frame.units].sort((a, b) => a.team.localeCompare(b.team));
      for (const unit of sorted) drawUnit(unit, elapsed, w, h);
      document.getElementById('time').textContent = `${elapsed.toFixed(1)}s / units ${frame.units.length}`;
    }

    function drawBackground(w, h, t) {
      const expeditionW = expeditionWidth(w);
      const floorTop = Math.floor(h * 0.58);
      const grad = ctx.createLinearGradient(expeditionW, 0, expeditionW, h);
      grad.addColorStop(0, '#3c2d24');
      grad.addColorStop(0.58, '#241b16');
      grad.addColorStop(1, '#17120f');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, w, h);
      drawGuildHouse(w, h, t, expeditionW);
      drawExpeditionRail(w, h, t, expeditionW);
    }

    function drawGuildHouse(w, h, t, expeditionW) {
      const x = expeditionW;
      const width = w - expeditionW;
      const floorTop = Math.floor(h * 0.58);
      ctx.fillStyle = '#4a2f20';
      ctx.fillRect(x, floorTop, width, h - floorTop);
      ctx.strokeStyle = 'rgba(255, 220, 170, 0.14)';
      ctx.lineWidth = 1;
      for (let i = 0; i < 8; i++) {
        const y = floorTop + i * 34;
        ctx.beginPath();
        ctx.moveTo(x + 20, y);
        ctx.lineTo(w - 20, y + 10);
        ctx.stroke();
      }
      drawForge(x + width * 0.16, floorTop + 20, t);
      drawAlchemy(x + width * 0.38, floorTop + 8, t);
      drawDoor(x + width * 0.62, floorTop - 120);
      drawTavernBar(x + width * 0.78, floorTop + 18, width * 0.18);
      ctx.fillStyle = '#f2dcc2';
      ctx.font = '700 18px Segoe UI';
      ctx.fillText('Guild House', x + 28, 34);
      ctx.font = '12px Segoe UI';
      ctx.fillStyle = '#bfae9a';
      ctx.fillText('warehouse / heroes / operation actions feed this scene', x + 28, 55);
    }

    function drawForge(x, y, t) {
      ctx.fillStyle = '#2c2521';
      ctx.fillRect(x - 38, y + 30, 76, 54);
      ctx.fillStyle = '#ba4d2f';
      ctx.globalAlpha = 0.72 + Math.sin(t * 5) * 0.12;
      ctx.beginPath();
      ctx.ellipse(x, y + 48, 24, 14, 0, 0, Math.PI * 2);
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.fillStyle = '#6d7780';
      ctx.fillRect(x - 52, y + 84, 104, 16);
      ctx.fillStyle = '#d9c3a4';
      ctx.font = '12px Segoe UI';
      ctx.fillText('Forge', x - 20, y + 120);
    }

    function drawAlchemy(x, y, t) {
      ctx.strokeStyle = '#8bbf9b';
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.arc(x, y + 68, 34, Math.PI, 0);
      ctx.stroke();
      ctx.fillStyle = '#20261f';
      ctx.fillRect(x - 42, y + 66, 84, 42);
      ctx.fillStyle = `rgba(96, 210, 146, ${0.22 + Math.sin(t * 3) * 0.06})`;
      ctx.fillRect(x - 32, y + 76, 64, 18);
      ctx.fillStyle = '#d9c3a4';
      ctx.font = '12px Segoe UI';
      ctx.fillText('Alchemy Furnace', x - 48, y + 128);
    }

    function drawDoor(x, y) {
      ctx.fillStyle = '#15110e';
      ctx.fillRect(x - 38, y, 76, 144);
      ctx.strokeStyle = '#8d6a42';
      ctx.lineWidth = 5;
      ctx.strokeRect(x - 38, y, 76, 144);
      ctx.fillStyle = '#b58a53';
      ctx.beginPath();
      ctx.arc(x + 22, y + 74, 4, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#d9c3a4';
      ctx.font = '12px Segoe UI';
      ctx.fillText('Dungeon Door', x - 38, y + 164);
    }

    function drawTavernBar(x, y, width) {
      ctx.fillStyle = '#5a321e';
      ctx.fillRect(x, y + 54, width, 34);
      ctx.fillStyle = '#81512e';
      ctx.fillRect(x - 12, y + 42, width + 24, 18);
      ctx.fillStyle = '#d8b15f';
      for (let i = 0; i < 3; i++) ctx.fillRect(x + 18 + i * 32, y + 10, 14, 30);
      ctx.fillStyle = '#d9c3a4';
      ctx.font = '12px Segoe UI';
      ctx.fillText('Tavern', x + 10, y + 112);
    }

    function drawExpeditionRail(w, h, t, expeditionW) {
      ctx.fillStyle = '#111820';
      ctx.fillRect(0, 0, expeditionW, h);
      const stripH = Math.max(70, Math.floor((h - 44) / 6));
      for (let i = 0; i < 6; i++) {
        const y = 12 + i * stripH;
        ctx.fillStyle = i === 0 ? '#263541' : '#1a232b';
        ctx.fillRect(12, y, expeditionW - 24, stripH - 10);
        ctx.strokeStyle = i === 0 ? '#6e93a7' : '#33414c';
        ctx.strokeRect(12, y, expeditionW - 24, stripH - 10);
        ctx.fillStyle = '#aeb8c4';
        ctx.font = '11px Segoe UI';
        ctx.fillText(i === 0 ? 'Party 1 / Endless Left Road' : `Empty expedition slot ${i + 1}`, 22, y + 18);
      }
      const floorY = laneY(0, h) + 28;
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
      ctx.beginPath();
      ctx.moveTo(18, floorY);
      ctx.lineTo(expeditionW - 18, floorY);
      ctx.stroke();
    }

    function drawAreaEffects(effects, elapsed, w, h) {
      for (const effect of effects) {
        const span = Math.max(0.001, Number(effect.end) - Number(effect.start));
        const p = Math.max(0, Math.min(1, (elapsed - Number(effect.start)) / span));
        const alpha = Math.sin(p * Math.PI) * 0.48;
        for (const cell of effect.cells || []) {
          const x = gridX(cell.x, w);
          const y = laneY(cell.lane, h);
          ctx.save();
          ctx.translate(x, y);
          ctx.fillStyle = `rgba(225, 45, 45, ${alpha})`;
          ctx.strokeStyle = `rgba(255, 212, 212, ${alpha * 0.8})`;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.ellipse(0, 20, 24, 7, 0, 0, Math.PI * 2);
          ctx.fill();
          ctx.stroke();
          ctx.restore();
        }
      }
    }

    function drawProjectiles(projectiles, elapsed, w, h) {
      for (const projectile of projectiles) {
        const span = Math.max(0.001, Number(projectile.end) - Number(projectile.start));
        const p = Math.max(0, Math.min(1, (elapsed - Number(projectile.start)) / span));
        const x = lerp(projectile.from?.x, projectile.to?.x, p);
        const lane = lerp(projectile.from?.lane, projectile.to?.lane, p);
        const sx = gridX(x, w);
        const sy = laneY(lane, h) - 28;
        const pulse = 1 + Math.sin(p * Math.PI) * 0.12;
        ctx.save();
        ctx.translate(sx, sy);
        ctx.fillStyle = 'rgba(0,0,0,0.32)';
        ctx.beginPath();
        ctx.ellipse(0, 22, 10, 4, 0, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#d83232';
        ctx.strokeStyle = '#fff1f1';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(0, 0, 7 * pulse, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
        ctx.restore();
      }
    }

    function drawUnit(unit, t, w, h) {
      const x = gridX(unit.render_x, w);
      const y = laneY(unit.render_lane, h);
      const scale = Number(unit.visual.scale || 1);
      const radius = Number(unit.visual.shadow_radius || 16) * scale;
      const state = visualState(unit.visual, unit.state);
      const anim = state?.animation || { frame_count: 4, fps: 6 };
      const frameCount = Math.max(1, (anim.frames || []).length || anim.frame_count);
      const frameIndex = Math.floor(t * anim.fps) % frameCount;
      const spriteFrame = (anim.frames || [])[frameIndex];
      const bob = Math.sin((frameIndex / frameCount) * Math.PI * 2) * 3;
      const attackLean = unit.state === 'attack' ? (unit.team === 'player' ? -8 : 8) : 0;
      ctx.save();
      ctx.translate(x + attackLean, y + bob);
      ctx.fillStyle = 'rgba(0,0,0,0.32)';
      ctx.beginPath();
      ctx.ellipse(0, 24 * scale, radius, radius * 0.36, 0, 0, Math.PI * 2);
      ctx.fill();
      if (!drawSpriteFrame(spriteFrame, scale)) {
        ctx.fillStyle = unit.visual.body_color || '#999999';
        ctx.strokeStyle = unit.team === 'player' ? '#d9efff' : '#ffe0d6';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.roundRect(-16 * scale, -30 * scale, 32 * scale, 48 * scale, 8 * scale);
        ctx.fill();
        ctx.stroke();
      }
      ctx.fillStyle = '#dbe7ef';
      ctx.font = '11px Segoe UI';
      ctx.textAlign = 'center';
      ctx.fillText(unit.name, 0, -42 * scale);
      drawHp(unit, scale);
      ctx.restore();
    }

    function drawSpriteFrame(frame, scale) {
      if (!frame?.texture?.path) return false;
      const image = textureImage(frame.texture.path);
      if (!image.complete || image.naturalWidth === 0) return false;
      const dw = frame.w * scale;
      const dh = frame.h * scale;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(image, frame.x, frame.y, frame.w, frame.h, -dw * frame.pivot_x, -dh * frame.pivot_y, dw, dh);
      return true;
    }

    function textureImage(path) {
      if (!images[path]) {
        const image = new Image();
        image.src = `/asset?path=${encodeURIComponent(path)}`;
        images[path] = image;
      }
      return images[path];
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
      const teamOffset = Number(lane || 0) * 8;
      return 72 + teamOffset;
    }

    function gridX(x, w) {
      return expeditionWidth(w) * 0.54 - Number(x || 0) * (GRID_CELL_W * 0.62);
    }

    function expeditionWidth(w) {
      return Math.max(280, Math.min(420, w * 0.34));
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
