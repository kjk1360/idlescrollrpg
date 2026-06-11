use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn game(args: &[String]) -> Result<(), String> {
    let project_path = crate::option_value_for_args(args, "--project")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("projects/sample"));
    let addr = crate::option_value_for_args(args, "--addr").unwrap_or("127.0.0.1:7880");

    let listener =
        TcpListener::bind(addr).map_err(|error| format!("failed to bind {addr}: {error}"))?;
    println!("Idle Scroll RPG: http://{addr}");
    println!("Project: {}", project_path.display());

    let state = GameServerState { project_path };
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(stream, &state) {
                    eprintln!("game error: {error}");
                }
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }
    Ok(())
}

struct GameServerState {
    project_path: PathBuf,
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn handle_stream(mut stream: TcpStream, state: &GameServerState) -> Result<(), String> {
    let request = read_request(&mut stream)?;
    let response = route_request(&request, state);
    stream
        .write_all(&response)
        .map_err(|error| format!("failed to write response: {error}"))
}

fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    let mut buffer = Vec::new();
    let mut temp = [0; 4096];
    let header_end;

    loop {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            return Err("empty request".to_string());
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = index;
            break;
        }
        if buffer.len() > 1024 * 1024 {
            return Err("request headers too large".to_string());
        }
    }

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| "missing request path".to_string())?
        .to_string();
    let content_length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("failed to read request body: {error}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
    }
    let body = buffer
        .get(body_start..body_start + content_length)
        .unwrap_or(&[])
        .to_vec();

    Ok(Request { method, path, body })
}

fn route_request(request: &Request, state: &GameServerState) -> Vec<u8> {
    let result = match (request.method.as_str(), path_without_query(&request.path)) {
        ("GET", "/") => Ok(html(GAME_HTML)),
        ("GET", "/asset") => asset(request, state),
        ("GET", "/api/account-state") => account_state(state),
        ("POST", "/api/account-dispatch") => account_dispatch(request, state),
        ("POST", "/api/account-energy/recover") => recover_energy(state),
        ("POST", "/api/account-alchemy/craft") => craft(request, state, "alchemy"),
        ("POST", "/api/account-forge/craft") => craft(request, state, "forge"),
        ("POST", "/api/account-refinement/craft") => craft(request, state, "refinement"),
        ("POST", "/api/account-hero/equip") => equip_hero(request, state),
        ("POST", "/api/account-hero/unequip") => unequip_hero(request, state),
        _ => Err(("not found".to_string(), 404)),
    };
    match result {
        Ok(response) => response,
        Err((message, status)) => json_response(status, &json!({ "ok": false, "error": message })),
    }
}

fn account_state(state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    crate::account_state_snapshot_for_api(&state.project_path)
        .map(|value| json_response(200, &value))
        .map_err(|error| (error, 500))
}

fn account_dispatch(request: &Request, state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    let body = request_json(request)?;
    let map_key = body
        .get("map_key")
        .and_then(Value::as_str)
        .unwrap_or("endless_left_road");
    let seed = body.get("seed").and_then(Value::as_u64).unwrap_or(1);
    crate::dispatch_account_for_api(&state.project_path, map_key, seed, current_unix_time())
        .map(|value| {
            json_response(
                200,
                &json!({
                    "ok": true,
                    "message": format!("dispatched {map_key}"),
                    "account": value,
                }),
            )
        })
        .map_err(|error| (error, 500))
}

fn recover_energy(state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    crate::recover_energy_for_api(&state.project_path, current_unix_time())
        .map(|value| json_response(200, &value))
        .map_err(|error| (error, 500))
}

fn craft(
    request: &Request,
    state: &GameServerState,
    device: &str,
) -> Result<Vec<u8>, (String, u16)> {
    let body = request_json(request)?;
    let recipe_key = body
        .get("recipe_key")
        .and_then(Value::as_str)
        .ok_or_else(|| ("missing recipe_key".to_string(), 400))?;
    let result = match device {
        "alchemy" => {
            crate::craft_alchemy_for_api(&state.project_path, recipe_key, current_unix_time())
        }
        "forge" => crate::craft_forge_for_api(&state.project_path, recipe_key, current_unix_time()),
        "refinement" => {
            crate::craft_refinement_for_api(&state.project_path, recipe_key, current_unix_time())
        }
        _ => Err(format!("unknown craft device {device}")),
    };
    result
        .map(|value| json_response(200, &value))
        .map_err(|error| (error, 500))
}

fn equip_hero(request: &Request, state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    let body = request_json(request)?;
    let hero_id = body
        .get("hero_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ("missing hero_id".to_string(), 400))?;
    let slot_key = body
        .get("slot_key")
        .and_then(Value::as_str)
        .unwrap_or("main_hand");
    let equipment_instance_id = body
        .get("equipment_instance_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ("missing equipment_instance_id".to_string(), 400))?;
    crate::equip_hero_for_api(
        &state.project_path,
        hero_id,
        slot_key,
        equipment_instance_id,
    )
    .map(|value| json_response(200, &value))
    .map_err(|error| (error, 500))
}

fn unequip_hero(request: &Request, state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    let body = request_json(request)?;
    let hero_id = body
        .get("hero_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ("missing hero_id".to_string(), 400))?;
    let slot_key = body
        .get("slot_key")
        .and_then(Value::as_str)
        .unwrap_or("main_hand");
    crate::unequip_hero_for_api(&state.project_path, hero_id, slot_key)
        .map(|value| json_response(200, &value))
        .map_err(|error| (error, 500))
}

fn asset(request: &Request, state: &GameServerState) -> Result<Vec<u8>, (String, u16)> {
    let raw_path =
        query_value(&request.path, "path").ok_or_else(|| ("missing path".to_string(), 400))?;
    let raw_path = percent_decode(raw_path).map_err(|error| (error, 400))?;
    if raw_path.contains("..") || raw_path.starts_with('/') || raw_path.starts_with('\\') {
        return Err(("invalid asset path".to_string(), 400));
    }
    let path = state.project_path.join(raw_path.replace('/', "\\"));
    let bytes = fs::read(&path).map_err(|error| {
        (
            format!("failed to read asset {}: {error}", path.display()),
            404,
        )
    })?;
    Ok(response(200, content_type_for_path(&path), &bytes))
}

fn request_json(request: &Request) -> Result<Value, (String, u16)> {
    if request.body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&request.body).map_err(|error| (format!("invalid json: {error}"), 400))
}

fn path_without_query(path: &str) -> &str {
    path.split_once('?').map(|(path, _)| path).unwrap_or(path)
}

fn query_value<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    let (_, query) = path.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (name == key).then_some(value)
    })
}

fn percent_decode(value: &str) -> Result<String, String> {
    let mut output = String::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value
                .get(index + 1..index + 3)
                .ok_or_else(|| "invalid percent encoding".to_string())?;
            let byte =
                u8::from_str_radix(hex, 16).map_err(|_| "invalid percent encoding".to_string())?;
            output.push(byte as char);
            index += 3;
        } else if bytes[index] == b'+' {
            output.push(' ');
            index += 1;
        } else {
            output.push(bytes[index] as char);
            index += 1;
        }
    }
    Ok(output)
}

fn current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn html(body: &str) -> Vec<u8> {
    response(200, "text/html; charset=utf-8", body.as_bytes())
}

fn json_response(status: u16, value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec());
    response(status, "application/json; charset=utf-8", &body)
}

fn response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
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

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
    {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

const GAME_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Idle Scroll RPG</title>
  <style>
    :root { color-scheme: dark; --bg:#121417; --panel:#1d2228; --line:#343b44; --text:#eef3f0; --muted:#9ca9a3; --gold:#d8b45f; --red:#c94c4c; --green:#67b77a; }
    * { box-sizing: border-box; }
    body { margin:0; min-height:100vh; background:#0d0f12; color:var(--text); font:14px/1.35 Segoe UI, Arial, sans-serif; }
    button { border:1px solid var(--line); background:#28313a; color:var(--text); height:30px; padding:0 10px; cursor:pointer; }
    button.primary { background:#6e5523; border-color:#9b7937; }
    button:disabled { opacity:.45; cursor:default; }
    .game { width:1280px; height:720px; margin:0 auto; background:var(--bg); overflow:hidden; display:grid; grid-template-columns: 330px 1fr 340px; grid-template-rows: 430px 290px; }
    .guild { grid-column:1 / 3; position:relative; overflow:hidden; background:linear-gradient(#27313a, #171b20 58%, #101216); border-right:1px solid var(--line); border-bottom:1px solid var(--line); }
    .guild:before { content:""; position:absolute; inset:0; background:linear-gradient(90deg, rgba(0,0,0,.35), transparent 35%, rgba(0,0,0,.28)); pointer-events:none; }
    .beam { position:absolute; left:0; right:0; bottom:78px; height:20px; background:#513723; box-shadow:0 -108px 0 #4a3322; }
    .floor { position:absolute; left:0; right:0; bottom:0; height:122px; background:repeating-linear-gradient(90deg, #272017 0 86px, #211910 86px 92px); border-top:4px solid #5c432c; }
    .door { position:absolute; left:34px; bottom:86px; width:116px; height:216px; background:#25170f; border:6px solid #6d4b2f; border-radius:54px 54px 4px 4px; }
    .furnace { position:absolute; left:224px; bottom:82px; width:150px; height:125px; background:#4c3230; border:4px solid #906a3b; }
    .furnace:after { content:""; position:absolute; left:42px; right:42px; bottom:18px; height:48px; background:#d56537; box-shadow:0 0 26px #d56537; }
    .forge { position:absolute; left:430px; bottom:80px; width:170px; height:115px; background:#2d3337; border:4px solid #7d8790; }
    .bar { position:absolute; right:58px; bottom:80px; width:230px; height:106px; background:#50351e; border-top:10px solid #8e6034; }
    .hero-sil { position:absolute; bottom:83px; width:42px; height:92px; background:#546878; border-radius:18px 18px 8px 8px; box-shadow:0 9px 0 rgba(0,0,0,.3); }
    .hero-sil.knight { left:670px; }
    .hero-sil.archer { left:734px; background:#5d7152; }
    .guild-title { position:absolute; left:22px; top:18px; font-size:24px; font-weight:750; color:#f4e1a8; }
    .guild-status { position:absolute; left:22px; top:54px; color:var(--muted); }
    .expeditions { grid-column:1; grid-row:2; border-right:1px solid var(--line); padding:12px; overflow:auto; }
    .strip { height:36px; margin-bottom:8px; border:1px solid var(--line); background:#181d22; display:flex; align-items:center; justify-content:space-between; padding:0 10px; }
    .main { grid-column:2; grid-row:2; padding:12px; overflow:auto; }
    .side { grid-column:3; grid-row:1 / 3; border-left:1px solid var(--line); display:grid; grid-template-rows: 44px 1fr; min-width:0; }
    .tabs { display:grid; grid-template-columns:repeat(3,1fr); border-bottom:1px solid var(--line); }
    .tabs button { height:43px; border-width:0 1px 0 0; background:#171c21; }
    .tabs button.active { background:#29313a; color:#f4e1a8; }
    .panel { padding:12px; overflow:auto; }
    .card { border:1px solid var(--line); background:var(--panel); padding:10px; margin-bottom:10px; }
    .row { display:flex; justify-content:space-between; gap:8px; margin:4px 0; }
    .muted { color:var(--muted); }
    .good { color:var(--green); }
    .bad { color:var(--red); }
    .actions { display:flex; gap:6px; flex-wrap:wrap; margin-top:8px; }
    table { width:100%; border-collapse:collapse; font-size:12px; }
    th, td { border-bottom:1px solid var(--line); padding:6px; text-align:left; vertical-align:top; }
    th { color:var(--muted); font-weight:600; }
  </style>
</head>
<body>
  <div class="game">
    <section class="guild">
      <div class="beam"></div><div class="floor"></div><div class="door"></div><div class="furnace"></div><div class="forge"></div><div class="bar"></div>
      <div class="hero-sil knight"></div><div class="hero-sil archer"></div>
      <div class="guild-title">Guild House</div>
      <div id="guildStatus" class="guild-status">loading account...</div>
    </section>
    <section class="expeditions">
      <div class="row"><b>Dungeon Expeditions</b><button class="primary" onclick="dispatchDungeon()">Dispatch</button></div>
      <div id="expeditionList"></div>
    </section>
    <section class="main">
      <div id="log" class="card">Ready.</div>
      <div id="operationSummary"></div>
    </section>
    <aside class="side">
      <nav class="tabs">
        <button id="tabWarehouse" class="active" onclick="selectTab('Warehouse')">Warehouse</button>
        <button id="tabHero" onclick="selectTab('Hero')">Hero</button>
        <button id="tabOperation" onclick="selectTab('Operation')">Operation</button>
      </nav>
      <div id="sidePanel" class="panel"></div>
    </aside>
  </div>
  <script>
    const state = { account:null, tab:'Warehouse', seed:1 };
    const $ = id => document.getElementById(id);
    const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
    async function api(path, options) {
      const response = await fetch(path, options);
      const data = await response.json();
      if (!data.ok && data.error) throw new Error(data.error);
      return data;
    }
    async function refresh() {
      state.account = await api('/api/account-state');
      render();
    }
    function selectTab(tab) {
      state.tab = tab;
      render();
    }
    function render() {
      const a = state.account;
      if (!a) return;
      $('guildStatus').innerHTML = `Energy <b>${a.energy_after_recovery}/${a.max_energy}</b> · Warehouse ${usedSlots(a)}`;
      $('expeditionList').innerHTML = [1,2,3,4,5,6].map(i => `<div class="strip"><span>Party ${i}</span><span class="${i === 1 ? 'good' : 'muted'}">${i === 1 ? 'Endless Left Road' : 'empty'}</span></div>`).join('');
      $('operationSummary').innerHTML = `
        <div class="card"><b>Guild Work</b>
          <div class="row"><span>Alchemy recipes</span><span>${a.alchemy_recipes.length}</span></div>
          <div class="row"><span>Forge recipes</span><span>${a.forge_recipes.length}</span></div>
          <div class="row"><span>Refinement recipes</span><span>${a.refinement_recipes.length}</span></div>
        </div>`;
      for (const name of ['Warehouse','Hero','Operation']) $('tab' + name).className = state.tab === name ? 'active' : '';
      if (state.tab === 'Warehouse') renderWarehouse(a);
      if (state.tab === 'Hero') renderHero(a);
      if (state.tab === 'Operation') renderOperation(a);
    }
    function usedSlots(a) {
      return (a.storage_tabs || []).map(tab => `${esc(tab.name)} ${tab.used_slots}/${tab.capacity}`).join(' · ');
    }
    function renderWarehouse(a) {
      $('sidePanel').innerHTML = `
        <div class="card"><b>Materials / Consumables</b>${(a.inventory || []).map(item => `<div class="row"><span>${esc(item.name)}</span><span>x${item.quantity}</span></div>`).join('') || '<div class="muted">empty</div>'}</div>
        <div class="card"><b>Equipment</b>${(a.equipment || []).map(eq => `<div class="row"><span>${esc(eq.name)}<br><small class="muted">${esc(eq.instance_id)}</small></span><span>${esc(eq.rarity)}</span></div>`).join('') || '<div class="muted">empty</div>'}</div>`;
    }
    function renderHero(a) {
      $('sidePanel').innerHTML = (a.heroes || []).map(hero => {
        const slots = (hero.equipment_slots || []).map(slot => `<div class="row"><span>${esc(slot.slot_key)}</span><span>${esc(slot.name)}</span></div>`).join('') || '<div class="muted">no equipment</div>';
        const equipButtons = (a.equipment || []).map(eq => `<button onclick="equipHero('${esc(hero.hero_id)}','${esc(eq.instance_id)}')">${esc(eq.name)}</button>`).join('');
        return `<div class="card"><b>${esc(hero.name)}</b><div class="muted">${esc(hero.unit_key)}</div>${slots}<div class="actions">${equipButtons || '<span class="muted">no equipment</span>'}<button onclick="unequipHero('${esc(hero.hero_id)}')">Unequip</button></div></div>`;
      }).join('');
    }
    function renderOperation(a) {
      $('sidePanel').innerHTML = `
        <div class="card"><b>Alchemy Furnace</b>${recipeButtons(a.alchemy_recipes, 'alchemy')}</div>
        <div class="card"><b>Forge</b>${recipeButtons(a.forge_recipes, 'forge')}</div>
        <div class="card"><b>Refinement Workbench</b>${recipeButtons(a.refinement_recipes, 'refinement')}</div>`;
    }
    function recipeButtons(recipes, kind) {
      return (recipes || []).map(recipe => `<div class="row"><span>${esc(recipe.name)}<br><small class="muted">${esc(recipe.output_name || '')}</small></span><button ${recipe.craftable ? '' : 'disabled'} onclick="craft('${kind}','${esc(recipe.key)}')">Craft</button></div>`).join('') || '<div class="muted">empty</div>';
    }
    async function dispatchDungeon() {
      try {
        const result = await api('/api/account-dispatch', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({ map_key:'endless_left_road', seed:state.seed++ }) });
        state.account = result.account;
        $('log').textContent = result.message || 'Dungeon dispatched.';
        render();
      } catch (error) { $('log').textContent = error.message; }
    }
    async function craft(kind, recipeKey) {
      try {
        const result = await api(`/api/account-${kind}/craft`, { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({ recipe_key:recipeKey }) });
        state.account = result.account;
        $('log').textContent = result.message || `Crafted ${recipeKey}.`;
        render();
      } catch (error) { $('log').textContent = error.message; }
    }
    async function equipHero(heroId, equipmentId) {
      const result = await api('/api/account-hero/equip', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({ hero_id:heroId, slot_key:'main_hand', equipment_instance_id:equipmentId }) });
      state.account = result.account;
      $('log').textContent = result.message;
      render();
    }
    async function unequipHero(heroId) {
      const result = await api('/api/account-hero/unequip', { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({ hero_id:heroId, slot_key:'main_hand' }) });
      state.account = result.account;
      $('log').textContent = result.message;
      render();
    }
    refresh();
  </script>
</body>
</html>"#;
