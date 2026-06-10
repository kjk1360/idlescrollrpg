use data_studio_core::{
    CellValue, DataProject, FieldId, FieldKind, FieldSchema, ProjectFingerprints, RowData, RowId,
    TableData, TableId, TableSchema,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn serve(args: &[String]) -> Result<(), String> {
    let project_path = crate::option_value_for_args(args, "--project")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("projects/sample"));
    let addr = crate::option_value_for_args(args, "--addr").unwrap_or("127.0.0.1:7878");
    let codegen_out = crate::option_value_for_args(args, "--codegen-out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/generated_data/src"));
    let data_out = crate::option_value_for_args(args, "--data-out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("build/sample_data"));

    let listener =
        TcpListener::bind(addr).map_err(|error| format!("failed to bind {addr}: {error}"))?;
    println!("Data Studio: http://{addr}");
    println!("Project: {}", project_path.display());

    let state = ServerState {
        project_path,
        codegen_out,
        data_out,
    };

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(stream, &state) {
                    eprintln!("serve error: {error}");
                }
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }

    Ok(())
}

struct ServerState {
    project_path: PathBuf,
    codegen_out: PathBuf,
    data_out: PathBuf,
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn handle_stream(mut stream: TcpStream, state: &ServerState) -> Result<(), String> {
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
        if let Some(index) = find_header_end(&buffer) {
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

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn route_request(request: &Request, state: &ServerState) -> Vec<u8> {
    let result = match (request.method.as_str(), path_without_query(&request.path)) {
        ("GET", "/") => Ok(html(INDEX_HTML)),
        ("GET", "/asset") => asset(request, state),
        ("GET", "/api/assets") => api_assets(state),
        ("GET", "/api/account-state") => api_account_state(state),
        ("GET", "/api/project") => api_project(state),
        ("GET", "/api/status") => api_status(state),
        ("GET", "/api/view") => api_view(request, state),
        ("POST", "/api/cell") => api_update_cell(request, state),
        ("POST", "/api/row") => api_add_row(request, state),
        ("POST", "/api/row/delete") => api_delete_row(request, state),
        ("POST", "/api/schema/table") => api_add_table(request, state),
        ("POST", "/api/schema/table/delete") => api_delete_table(request, state),
        ("POST", "/api/schema/field") => api_add_field(request, state),
        ("POST", "/api/schema/field/delete") => api_delete_field(request, state),
        ("POST", "/api/validate") => api_validate(state),
        ("POST", "/api/codegen") => api_codegen(state),
        ("POST", "/api/data-build") => api_data_build(state),
        ("POST", "/api/simulate") => api_simulate(request, state),
        ("POST", "/api/account-dispatch") => api_account_dispatch(request, state),
        ("POST", "/api/account-energy/recover") => api_account_energy_recover(request, state),
        ("POST", "/api/account-mail/claim") => api_account_mail_claim(request, state),
        ("POST", "/api/account-mail/delete") => api_account_mail_delete(request, state),
        ("POST", "/api/account-alchemy/craft") => api_account_alchemy_craft(request, state),
        ("POST", "/api/account-forge/craft") => api_account_forge_craft(request, state),
        ("POST", "/api/account-refinement/craft") => api_account_refinement_craft(request, state),
        ("POST", "/api/account-hero/equip") => api_account_hero_equip(request, state),
        ("POST", "/api/account-hero/unequip") => api_account_hero_unequip(request, state),
        ("POST", "/api/import/aseprite") => api_import_aseprite(request, state),
        ("POST", "/api/visual/slice-grid") => api_slice_sprite_grid(request, state),
        _ => Err(("not found".to_string(), 404)),
    };

    match result {
        Ok(response) => response,
        Err((message, status)) => json_response(
            status,
            &json!({
                "ok": false,
                "error": message,
            }),
        ),
    }
}

fn asset(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
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

fn api_assets(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let root = state.project_path.join("assets");
    let mut files = Vec::new();
    if root.exists() {
        collect_asset_files(&root, &root, &mut files)
            .map_err(|error| (format!("failed to list assets: {error}"), 500))?;
    }
    files.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .cmp(&b.get("path").and_then(Value::as_str))
    });
    Ok(json_response(200, &json!({ "ok": true, "assets": files })))
}

fn collect_asset_files(root: &Path, dir: &Path, files: &mut Vec<Value>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_asset_files(root, &path, files)?;
            continue;
        }
        if !is_image_asset(&path) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let asset_path = format!("assets/{relative}");
        let metadata = entry.metadata()?;
        files.push(json!({
            "path": asset_path,
            "name": path.file_name().and_then(|value| value.to_str()).unwrap_or(""),
            "bytes": metadata.len(),
            "content_type": content_type_for_path(&path),
        }));
    }
    Ok(())
}

fn is_image_asset(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "svg" | "webp"
    )
}

fn api_project(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let project = load_project(&state.project_path)?;
    let status = project_status(&project, &state.project_path);
    Ok(json_response(
        200,
        &json!({
            "ok": true,
            "project_path": state.project_path,
            "project": project,
            "status": status,
        }),
    ))
}

fn api_status(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let project = load_project(&state.project_path)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "status": project_status(&project, &state.project_path) }),
    ))
}

fn api_account_state(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let snapshot =
        crate::account_state_snapshot_for_api(&state.project_path).map_err(|error| (error, 500))?;
    Ok(json_response(200, &snapshot))
}

fn api_view(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let view_key = query_value(&request.path, "view").unwrap_or("map_wave_preview");
    let project = load_project(&state.project_path)?;
    let view = project
        .materialize_view(view_key)
        .map_err(|error| (error, 400))?;
    Ok(json_response(200, &json!({ "ok": true, "view": view })))
}

fn api_update_cell(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload: Value = serde_json::from_slice(&request.body)
        .map_err(|error| (format!("invalid JSON body: {error}"), 400))?;
    let table_id = number(&payload, "table_id")?;
    let row_id = number(&payload, "row_id")?;
    let field_id = number(&payload, "field_id")?;
    let raw_value = payload
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| ("missing string value".to_string(), 400))?;

    let mut project = load_project(&state.project_path)?;
    let field = project
        .tables
        .iter()
        .find(|table| table.id == TableId(table_id))
        .and_then(|table| {
            table
                .fields
                .iter()
                .find(|field| field.id == FieldId(field_id))
        })
        .ok_or_else(|| ("unknown table field".to_string(), 404))?;
    let cell_value = parse_cell_value(&field.kind, raw_value)?;
    let table_data = project
        .data
        .iter_mut()
        .find(|table| table.table_id == TableId(table_id))
        .ok_or_else(|| ("unknown table data".to_string(), 404))?;
    let row = table_data
        .rows
        .iter_mut()
        .find(|row| row.id == RowId(row_id))
        .ok_or_else(|| ("unknown row".to_string(), 404))?;
    row.cells.insert(FieldId(field_id), cell_value);
    project
        .save_to_dir(&state.project_path, &project_name(&state.project_path))
        .map_err(|error| (error.to_string(), 500))?;
    Ok(json_response(200, &json!({ "ok": true })))
}

fn api_add_row(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let table_id = TableId(number(&payload, "table_id")?);
    let key = string_value(&payload, "key")?;
    validate_key(key)?;

    let mut project = load_project(&state.project_path)?;
    let table = project
        .tables
        .iter()
        .find(|table| table.id == table_id)
        .ok_or_else(|| ("unknown table".to_string(), 404))?;
    let row_id = next_row_id(&project);
    let table_data = project
        .data
        .iter_mut()
        .find(|table| table.table_id == table_id)
        .ok_or_else(|| ("unknown table data".to_string(), 404))?;
    if table_data.rows.iter().any(|row| row.key == key) {
        return Err((format!("row key already exists: {key}"), 400));
    }

    let mut cells = BTreeMap::new();
    for field in &table.fields {
        cells.insert(field.id, default_cell_value(&field.kind));
    }
    table_data.rows.push(RowData {
        id: row_id,
        key: key.to_string(),
        cells,
    });
    save_project(&project, state)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "row_id": row_id.0 }),
    ))
}

fn api_delete_row(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let table_id = TableId(number(&payload, "table_id")?);
    let row_id = RowId(number(&payload, "row_id")?);
    let mut project = load_project(&state.project_path)?;
    let table_data = project
        .data
        .iter_mut()
        .find(|table| table.table_id == table_id)
        .ok_or_else(|| ("unknown table data".to_string(), 404))?;
    let before = table_data.rows.len();
    table_data.rows.retain(|row| row.id != row_id);
    if table_data.rows.len() == before {
        return Err(("unknown row".to_string(), 404));
    }

    for table_data in &mut project.data {
        for row in &mut table_data.rows {
            for value in row.cells.values_mut() {
                let clear_single = matches!(value, CellValue::Row(value) if *value == row_id);
                if clear_single {
                    *value = CellValue::Empty;
                    continue;
                }
                match value {
                    CellValue::Rows(values) => {
                        values.retain(|value| *value != row_id);
                    }
                    _ => {}
                }
            }
        }
    }

    save_project(&project, state)?;
    Ok(json_response(200, &json!({ "ok": true })))
}

fn api_add_table(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let key = string_value(&payload, "key")?;
    validate_key(key)?;
    let display_name = payload
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(key);

    let mut project = load_project(&state.project_path)?;
    if project.tables.iter().any(|table| table.key == key) {
        return Err((format!("table key already exists: {key}"), 400));
    }

    let table_id = next_table_id(&project);
    project.tables.push(TableSchema {
        id: table_id,
        key: key.to_string(),
        display_name: display_name.to_string(),
        fields: Vec::new(),
    });
    project.data.push(TableData {
        table_id,
        rows: Vec::new(),
    });
    save_project(&project, state)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "table_id": table_id.0 }),
    ))
}

fn api_delete_table(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let table_id = TableId(number(&payload, "table_id")?);
    let mut project = load_project(&state.project_path)?;
    if !project.tables.iter().any(|table| table.id == table_id) {
        return Err(("unknown table".to_string(), 404));
    }
    let mut removed_tables = collect_owned_descendants(&project, table_id);
    removed_tables.insert(table_id);
    project
        .tables
        .retain(|table| !removed_tables.contains(&table.id));
    project
        .data
        .retain(|table| !removed_tables.contains(&table.table_id));
    project
        .views
        .retain(|view| !removed_tables.contains(&view.source_table));
    for table in &mut project.tables {
        table
            .fields
            .retain(|field| !field_targets_any(field, &removed_tables));
    }
    for table_data in &mut project.data {
        let Some(table) = project
            .tables
            .iter()
            .find(|table| table.id == table_data.table_id)
        else {
            continue;
        };
        let valid_fields = table
            .fields
            .iter()
            .map(|field| field.id)
            .collect::<std::collections::BTreeSet<_>>();
        for row in &mut table_data.rows {
            row.cells
                .retain(|field_id, _| valid_fields.contains(field_id));
        }
    }
    save_project(&project, state)?;
    Ok(json_response(200, &json!({ "ok": true })))
}

fn api_add_field(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let table_id = TableId(number(&payload, "table_id")?);
    let key = string_value(&payload, "key")?;
    validate_key(key)?;
    let display_name = display_name_from_key(key);
    let kind = string_value(&payload, "kind")?;
    let required = payload
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target_table = payload.get("target_table").and_then(Value::as_u64);

    let mut project = load_project(&state.project_path)?;
    let field_id = next_field_id(&project);
    let field_kind = if kind == "owned_nested_table" {
        let nested_key = payload
            .get("nested_key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}_{}",
                    table_key(&project, table_id).unwrap_or("nested"),
                    key
                )
            });
        validate_key(&nested_key)?;
        if project.tables.iter().any(|table| table.key == nested_key) {
            return Err((format!("table key already exists: {nested_key}"), 400));
        }
        let nested_table = next_table_id(&project);
        project.tables.push(TableSchema {
            id: nested_table,
            key: nested_key.clone(),
            display_name: display_name_from_key(&nested_key),
            fields: Vec::new(),
        });
        project.data.push(TableData {
            table_id: nested_table,
            rows: Vec::new(),
        });
        FieldKind::OwnedNestedTable { nested_table }
    } else {
        if let Some(target_table) = target_table.map(TableId) {
            if owned_nested_table_ids(&project).contains(&target_table) {
                return Err((
                    "nested tables cannot be selected as relation targets".to_string(),
                    400,
                ));
            }
        }
        parse_field_kind(kind, target_table)?
    };
    {
        let table = project
            .tables
            .iter_mut()
            .find(|table| table.id == table_id)
            .ok_or_else(|| ("unknown table".to_string(), 404))?;
        if table.fields.iter().any(|field| field.key == key) {
            return Err((format!("field key already exists: {key}"), 400));
        }
        table.fields.push(FieldSchema {
            id: field_id,
            key: key.to_string(),
            display_name,
            kind: field_kind,
            required,
        });
    }
    save_project(&project, state)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "field_id": field_id.0 }),
    ))
}

fn api_delete_field(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let table_id = TableId(number(&payload, "table_id")?);
    let field_id = FieldId(number(&payload, "field_id")?);
    let mut project = load_project(&state.project_path)?;
    let table = project
        .tables
        .iter_mut()
        .find(|table| table.id == table_id)
        .ok_or_else(|| ("unknown table".to_string(), 404))?;
    let removed_field = table
        .fields
        .iter()
        .find(|field| field.id == field_id)
        .cloned()
        .ok_or_else(|| ("unknown field".to_string(), 404))?;
    table.fields.retain(|field| field.id != field_id);
    if let Some(table_data) = project
        .data
        .iter_mut()
        .find(|data| data.table_id == table_id)
    {
        for row in &mut table_data.rows {
            row.cells.remove(&field_id);
        }
    }
    for view in &mut project.views {
        view.columns
            .retain(|column| !(column.alias == "source" && column.field == field_id));
        view.joins
            .retain(|join| !(join.from_alias == "source" && join.field == field_id));
    }
    if let FieldKind::OwnedNestedTable { nested_table } = removed_field.kind {
        let mut removed_tables = collect_owned_descendants(&project, nested_table);
        removed_tables.insert(nested_table);
        project
            .tables
            .retain(|table| !removed_tables.contains(&table.id));
        project
            .data
            .retain(|table| !removed_tables.contains(&table.table_id));
        project
            .views
            .retain(|view| !removed_tables.contains(&view.source_table));
        for table in &mut project.tables {
            table
                .fields
                .retain(|field| !field_targets_any(field, &removed_tables));
        }
    }
    save_project(&project, state)?;
    Ok(json_response(200, &json!({ "ok": true })))
}

fn api_validate(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let project = load_project(&state.project_path)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "issues": project.validate() }),
    ))
}

fn api_codegen(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let args = vec![
        "--project".to_string(),
        state.project_path.to_string_lossy().to_string(),
        "--out".to_string(),
        state.codegen_out.to_string_lossy().to_string(),
    ];
    crate::run_codegen_for_api(&args).map_err(|error| (error, 500))?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "message": format!("generated Rust files: {}", state.codegen_out.display()) }),
    ))
}

fn api_data_build(state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let args = vec![
        "--project".to_string(),
        state.project_path.to_string_lossy().to_string(),
        "--out".to_string(),
        state.data_out.to_string_lossy().to_string(),
    ];
    crate::run_data_build_for_api(&args).map_err(|error| (error, 500))?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "message": format!("built data snapshot: {}", state.data_out.join("data_snapshot.json").display()) }),
    ))
}

fn api_simulate(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = if request.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&request.body)
            .map_err(|error| (format!("invalid JSON body: {error}"), 400))?
    };
    let map_key = payload
        .get("map_key")
        .and_then(Value::as_str)
        .unwrap_or("endless_left_road");
    let project = load_project(&state.project_path)?;
    let output = crate::simulate_for_api(&project, map_key).map_err(|error| (error, 500))?;
    Ok(json_response(200, &json!({ "ok": true, "output": output })))
}

fn api_account_dispatch(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let map_key = payload
        .get("map_key")
        .and_then(Value::as_str)
        .unwrap_or("endless_left_road");
    let seed = payload.get("seed").and_then(Value::as_u64).unwrap_or(1);
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let snapshot = crate::dispatch_account_for_api(&state.project_path, map_key, seed, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(
        200,
        &json!({
            "ok": true,
            "message": format!("dispatched {map_key} and saved account state"),
            "account": snapshot,
        }),
    ))
}

fn api_account_energy_recover(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let response = crate::recover_energy_for_api(&state.project_path, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_mail_claim(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let mail_index = usize_number(&payload, "mail_index")?;
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let response = crate::claim_mail_for_api(&state.project_path, mail_index, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_mail_delete(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let mail_index = usize_number(&payload, "mail_index")?;
    let response = crate::delete_mail_for_api(&state.project_path, mail_index)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_alchemy_craft(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let recipe_key = string_value(&payload, "recipe_key")?;
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let response = crate::craft_alchemy_for_api(&state.project_path, recipe_key, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_forge_craft(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let recipe_key = string_value(&payload, "recipe_key")?;
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let response = crate::craft_forge_for_api(&state.project_path, recipe_key, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_refinement_craft(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let recipe_key = string_value(&payload, "recipe_key")?;
    let now_unix = payload
        .get("now_unix")
        .and_then(Value::as_i64)
        .unwrap_or_else(current_unix_time);
    let response = crate::craft_refinement_for_api(&state.project_path, recipe_key, now_unix)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_hero_equip(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let hero_id = string_value(&payload, "hero_id")?;
    let slot_key = string_value(&payload, "slot_key")?;
    let equipment_instance_id = string_value(&payload, "equipment_instance_id")?;
    let response = crate::equip_hero_for_api(
        &state.project_path,
        hero_id,
        slot_key,
        equipment_instance_id,
    )
    .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_account_hero_unequip(
    request: &Request,
    state: &ServerState,
) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let hero_id = string_value(&payload, "hero_id")?;
    let slot_key = string_value(&payload, "slot_key")?;
    let response = crate::unequip_hero_for_api(&state.project_path, hero_id, slot_key)
        .map_err(|error| (error, 500))?;
    Ok(json_response(200, &response))
}

fn api_import_aseprite(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    let payload = parse_body(&request.body)?;
    let file = string_value(&payload, "file")?;
    let summary = crate::aseprite::import_aseprite(&state.project_path, Path::new(file))
        .map_err(|error| (error, 500))?;
    Ok(json_response(
        200,
        &json!({
            "ok": true,
            "texture_key": summary.texture_key,
            "frame_count": summary.frame_count,
            "animation_count": summary.animation_count,
        }),
    ))
}

fn api_slice_sprite_grid(request: &Request, state: &ServerState) -> Result<Vec<u8>, (String, u16)> {
    const TEXTURE_ASSET_TABLE: TableId = TableId(6);
    const SPRITE_FRAME_TABLE: TableId = TableId(11);
    const FIELD_NAME: FieldId = FieldId(90);
    const FIELD_TEXTURE: FieldId = FieldId(91);
    const FIELD_X: FieldId = FieldId(92);
    const FIELD_Y: FieldId = FieldId(93);
    const FIELD_W: FieldId = FieldId(94);
    const FIELD_H: FieldId = FieldId(95);
    const FIELD_PIVOT_X: FieldId = FieldId(96);
    const FIELD_PIVOT_Y: FieldId = FieldId(97);
    const FIELD_DURATION: FieldId = FieldId(98);

    let payload = parse_body(&request.body)?;
    let texture_id = RowId(number(&payload, "texture_id")?);
    let prefix = string_value(&payload, "prefix")?;
    validate_key(prefix)?;
    let start_x = i32_number(&payload, "start_x")?;
    let start_y = i32_number(&payload, "start_y")?;
    let frame_w = positive_i32(&payload, "frame_w")?;
    let frame_h = positive_i32(&payload, "frame_h")?;
    let columns = positive_u64(&payload, "columns")?;
    let rows = positive_u64(&payload, "rows")?;
    let gap_x = i32_number(&payload, "gap_x")?;
    let gap_y = i32_number(&payload, "gap_y")?;
    let pivot_x = f32_number(&payload, "pivot_x")?;
    let pivot_y = f32_number(&payload, "pivot_y")?;
    let duration = f32_number(&payload, "duration")?;

    let mut project = load_project(&state.project_path)?;
    let texture_exists = project
        .data
        .iter()
        .find(|table| table.table_id == TEXTURE_ASSET_TABLE)
        .is_some_and(|table| table.rows.iter().any(|row| row.id == texture_id));
    if !texture_exists {
        return Err(("unknown texture row".to_string(), 404));
    }

    let mut next_id = next_row_id(&project).0;
    let frame_table = project
        .data
        .iter_mut()
        .find(|table| table.table_id == SPRITE_FRAME_TABLE)
        .ok_or_else(|| ("missing sprite_frame table data".to_string(), 404))?;
    let mut existing_keys = frame_table
        .rows
        .iter()
        .map(|row| row.key.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut created_ids = Vec::new();

    for row in 0..rows {
        for column in 0..columns {
            let index = row * columns + column;
            let base_key = format!("{prefix}_{index:03}");
            let key = unique_row_key_from_set(&mut existing_keys, &base_key);
            let row_id = RowId(next_id);
            next_id += 1;
            let x = start_x + column as i32 * (frame_w + gap_x);
            let y = start_y + row as i32 * (frame_h + gap_y);
            let mut cells = BTreeMap::new();
            cells.insert(FIELD_NAME, CellValue::String(display_name_from_key(&key)));
            cells.insert(FIELD_TEXTURE, CellValue::Row(texture_id));
            cells.insert(FIELD_X, CellValue::I32(x));
            cells.insert(FIELD_Y, CellValue::I32(y));
            cells.insert(FIELD_W, CellValue::I32(frame_w));
            cells.insert(FIELD_H, CellValue::I32(frame_h));
            cells.insert(FIELD_PIVOT_X, CellValue::F32(pivot_x));
            cells.insert(FIELD_PIVOT_Y, CellValue::F32(pivot_y));
            cells.insert(FIELD_DURATION, CellValue::F32(duration));
            frame_table.rows.push(RowData {
                id: row_id,
                key,
                cells,
            });
            created_ids.push(row_id.0);
        }
    }

    save_project(&project, state)?;
    Ok(json_response(
        200,
        &json!({ "ok": true, "created": created_ids.len(), "row_ids": created_ids }),
    ))
}

fn load_project(path: &Path) -> Result<DataProject, (String, u16)> {
    DataProject::load_from_dir(path).map_err(|error| (error.to_string(), 500))
}

fn current_unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn save_project(project: &DataProject, state: &ServerState) -> Result<(), (String, u16)> {
    project
        .save_to_dir(&state.project_path, &project_name(&state.project_path))
        .map_err(|error| (error.to_string(), 500))
}

fn project_status(project: &DataProject, path: &Path) -> Value {
    let schema_hash = project.schema_hash();
    let data_hash = project.data_hash();
    let fingerprints = project
        .fingerprints_from_dir(path)
        .unwrap_or(ProjectFingerprints {
            schema_hash,
            generated_schema_hash: 0,
            data_hash,
            built_data_hash: 0,
        });
    let issues = project.validate();

    json!({
        "schema_hash": fingerprints.schema_hash,
        "generated_schema_hash": fingerprints.generated_schema_hash,
        "data_hash": fingerprints.data_hash,
        "built_data_hash": fingerprints.built_data_hash,
        "status": crate::status_label_for_api(fingerprints.status()),
        "issues": issues,
    })
}

fn parse_cell_value(kind: &FieldKind, raw: &str) -> Result<CellValue, (String, u16)> {
    if raw.trim().is_empty() {
        return match kind {
            FieldKind::RelationMany { .. }
            | FieldKind::ReferenceGroup { .. }
            | FieldKind::OwnedNestedTable { .. } => Ok(CellValue::Rows(Vec::new())),
            _ => Ok(CellValue::Empty),
        };
    }

    match kind {
        FieldKind::Bool => raw
            .parse::<bool>()
            .map(CellValue::Bool)
            .map_err(|_| ("expected true or false".to_string(), 400)),
        FieldKind::I32 => raw
            .parse::<i32>()
            .map(CellValue::I32)
            .map_err(|_| ("expected i32".to_string(), 400)),
        FieldKind::I64 => raw
            .parse::<i64>()
            .map(CellValue::I64)
            .map_err(|_| ("expected i64".to_string(), 400)),
        FieldKind::F32 => raw
            .parse::<f32>()
            .map(CellValue::F32)
            .map_err(|_| ("expected f32".to_string(), 400)),
        FieldKind::String
        | FieldKind::Text
        | FieldKind::Enum { .. }
        | FieldKind::AssetRef { .. } => Ok(CellValue::String(raw.to_string())),
        FieldKind::RelationOne { .. } => raw
            .parse::<u64>()
            .map(|id| CellValue::Row(RowId(id)))
            .map_err(|_| ("expected row id".to_string(), 400)),
        FieldKind::RelationMany { .. }
        | FieldKind::ReferenceGroup { .. }
        | FieldKind::OwnedNestedTable { .. } => raw
            .split(',')
            .map(|part| {
                part.trim()
                    .parse::<u64>()
                    .map(RowId)
                    .map_err(|_| "expected comma-separated row ids".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(CellValue::Rows)
            .map_err(|error| (error, 400)),
    }
}

fn parse_body(body: &[u8]) -> Result<Value, (String, u16)> {
    serde_json::from_slice(body).map_err(|error| (format!("invalid JSON body: {error}"), 400))
}

fn string_value<'a>(payload: &'a Value, key: &str) -> Result<&'a str, (String, u16)> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| (format!("missing string {key}"), 400))
}

fn validate_key(key: &str) -> Result<(), (String, u16)> {
    let valid = key
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        && key.chars().next().is_some_and(|ch| ch.is_ascii_lowercase());
    if valid {
        Ok(())
    } else {
        Err((
            "key must start with a lowercase letter and use lowercase letters, digits, or '_'"
                .to_string(),
            400,
        ))
    }
}

fn parse_field_kind(kind: &str, target_table: Option<u64>) -> Result<FieldKind, (String, u16)> {
    let target = || {
        target_table.map(TableId).ok_or_else(|| {
            (
                "target_table is required for this field kind".to_string(),
                400,
            )
        })
    };
    match kind {
        "bool" => Ok(FieldKind::Bool),
        "i32" => Ok(FieldKind::I32),
        "i64" => Ok(FieldKind::I64),
        "f32" => Ok(FieldKind::F32),
        "string" => Ok(FieldKind::String),
        "text" => Ok(FieldKind::Text),
        "relation_one" => Ok(FieldKind::RelationOne {
            target_table: target()?,
        }),
        "relation_many" => Ok(FieldKind::RelationMany {
            target_table: target()?,
        }),
        "reference_group" => Ok(FieldKind::ReferenceGroup {
            target_table: target()?,
        }),
        "owned_nested_table" => Ok(FieldKind::OwnedNestedTable {
            nested_table: target()?,
        }),
        _ => Err((format!("unsupported field kind: {kind}"), 400)),
    }
}

fn table_key(project: &DataProject, table_id: TableId) -> Option<&str> {
    project
        .tables
        .iter()
        .find(|table| table.id == table_id)
        .map(|table| table.key.as_str())
}

fn display_name_from_key(key: &str) -> String {
    key.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn owned_nested_table_ids(project: &DataProject) -> std::collections::BTreeSet<TableId> {
    project
        .tables
        .iter()
        .flat_map(|table| table.fields.iter())
        .filter_map(|field| match field.kind {
            FieldKind::OwnedNestedTable { nested_table } => Some(nested_table),
            _ => None,
        })
        .collect()
}

fn collect_owned_descendants(
    project: &DataProject,
    table_id: TableId,
) -> std::collections::BTreeSet<TableId> {
    let mut descendants = std::collections::BTreeSet::new();
    collect_owned_descendants_into(project, table_id, &mut descendants);
    descendants
}

fn collect_owned_descendants_into(
    project: &DataProject,
    table_id: TableId,
    descendants: &mut std::collections::BTreeSet<TableId>,
) {
    let Some(table) = project.tables.iter().find(|table| table.id == table_id) else {
        return;
    };
    for field in &table.fields {
        if let FieldKind::OwnedNestedTable { nested_table } = field.kind {
            if descendants.insert(nested_table) {
                collect_owned_descendants_into(project, nested_table, descendants);
            }
        }
    }
}

fn next_table_id(project: &DataProject) -> TableId {
    TableId(
        project
            .tables
            .iter()
            .map(|table| table.id.0)
            .max()
            .unwrap_or(0)
            + 1,
    )
}

fn next_field_id(project: &DataProject) -> FieldId {
    FieldId(
        project
            .tables
            .iter()
            .flat_map(|table| table.fields.iter().map(|field| field.id.0))
            .max()
            .unwrap_or(0)
            + 1,
    )
}

fn next_row_id(project: &DataProject) -> RowId {
    RowId(
        project
            .data
            .iter()
            .flat_map(|table| table.rows.iter().map(|row| row.id.0))
            .max()
            .unwrap_or(1000)
            + 1,
    )
}

fn default_cell_value(kind: &FieldKind) -> CellValue {
    match kind {
        FieldKind::Bool => CellValue::Bool(false),
        FieldKind::I32 => CellValue::I32(0),
        FieldKind::I64 => CellValue::I64(0),
        FieldKind::F32 => CellValue::F32(0.0),
        FieldKind::String
        | FieldKind::Text
        | FieldKind::Enum { .. }
        | FieldKind::AssetRef { .. } => CellValue::String(String::new()),
        FieldKind::RelationOne { .. } => CellValue::Empty,
        FieldKind::RelationMany { .. }
        | FieldKind::ReferenceGroup { .. }
        | FieldKind::OwnedNestedTable { .. } => CellValue::Rows(Vec::new()),
    }
}

fn field_targets_any(field: &FieldSchema, table_ids: &std::collections::BTreeSet<TableId>) -> bool {
    match field.kind {
        FieldKind::RelationOne { target_table }
        | FieldKind::RelationMany { target_table }
        | FieldKind::ReferenceGroup { target_table } => table_ids.contains(&target_table),
        FieldKind::OwnedNestedTable { nested_table } => table_ids.contains(&nested_table),
        _ => false,
    }
}

fn project_name(path: &Path) -> String {
    let project_file = path.join("project.json");
    fs::read_to_string(project_file)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| {
            value
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Data Studio Project".to_string())
}

fn number(payload: &Value, key: &str) -> Result<u64, (String, u16)> {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| (format!("missing numeric {key}"), 400))
}

fn usize_number(payload: &Value, key: &str) -> Result<usize, (String, u16)> {
    let value = number(payload, key)?;
    usize::try_from(value).map_err(|_| (format!("{key} is too large"), 400))
}

fn positive_u64(payload: &Value, key: &str) -> Result<u64, (String, u16)> {
    let value = number(payload, key)?;
    if value == 0 {
        Err((format!("{key} must be greater than zero"), 400))
    } else {
        Ok(value)
    }
}

fn i32_number(payload: &Value, key: &str) -> Result<i32, (String, u16)> {
    let value = payload
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| (format!("missing numeric {key}"), 400))?;
    i32::try_from(value).map_err(|_| (format!("{key} is out of i32 range"), 400))
}

fn positive_i32(payload: &Value, key: &str) -> Result<i32, (String, u16)> {
    let value = i32_number(payload, key)?;
    if value <= 0 {
        Err((format!("{key} must be greater than zero"), 400))
    } else {
        Ok(value)
    }
}

fn f32_number(payload: &Value, key: &str) -> Result<f32, (String, u16)> {
    payload
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .ok_or_else(|| (format!("missing numeric {key}"), 400))
}

fn unique_row_key_from_set(
    existing: &mut std::collections::BTreeSet<String>,
    base: &str,
) -> String {
    if existing.insert(base.to_string()) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if existing.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
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

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                    .map_err(|_| "invalid percent encoding".to_string())?;
                let byte = u8::from_str_radix(hex, 16)
                    .map_err(|_| "invalid percent encoding".to_string())?;
                output.push(byte);
                index += 3;
            }
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).map_err(|_| "invalid utf-8 in query value".to_string())
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

fn content_type_for_path(path: &Path) -> &'static str {
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

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Belt Data Studio</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f6f7f9;
      --panel: #ffffff;
      --line: #d7dce2;
      --text: #161a1f;
      --muted: #66707c;
      --accent: #0f766e;
      --danger: #b42318;
      --warn: #b45309;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: Segoe UI, system-ui, sans-serif;
      background: var(--bg);
      color: var(--text);
    }
    .app {
      display: grid;
      grid-template-columns: 260px minmax(0, 1fr);
      grid-template-rows: 54px minmax(0, 1fr);
      height: 100vh;
    }
    header {
      grid-column: 1 / -1;
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 0 14px;
      border-bottom: 1px solid var(--line);
      background: var(--panel);
    }
    h1 {
      font-size: 16px;
      font-weight: 650;
      margin: 0;
      white-space: nowrap;
    }
    .status {
      display: inline-flex;
      align-items: center;
      height: 28px;
      padding: 0 10px;
      border: 1px solid var(--line);
      border-radius: 6px;
      font-size: 12px;
      color: var(--muted);
      background: #fbfcfd;
    }
    .status.dirty { color: var(--warn); border-color: #f0b36a; }
    .status.bad { color: var(--danger); border-color: #f3a19a; }
    .actions {
      display: flex;
      gap: 8px;
      margin-left: auto;
      min-width: 0;
    }
    .tabs {
      display: flex;
      gap: 4px;
      height: 32px;
      align-items: center;
    }
    .tab {
      min-width: 84px;
      background: #f5f7f9;
    }
    .tab.active {
      border-color: var(--accent);
      color: var(--accent);
      background: #e8f5f3;
      font-weight: 650;
    }
    button {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      color: var(--text);
      padding: 0 10px;
      font: inherit;
      font-size: 13px;
      cursor: pointer;
      white-space: nowrap;
    }
    button.primary {
      border-color: var(--accent);
      background: var(--accent);
      color: #fff;
    }
    aside {
      min-height: 0;
      overflow: auto;
      border-right: 1px solid var(--line);
      background: #eef2f5;
      padding: 10px;
    }
    .app.visual-mode {
      grid-template-columns: minmax(0, 1fr);
    }
    .app.visual-mode aside {
      display: none;
    }
    .nav-title {
      margin: 10px 8px 6px;
      font-size: 11px;
      font-weight: 700;
      color: var(--muted);
      text-transform: uppercase;
    }
    .nav-item {
      width: 100%;
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin: 2px 0;
      background: transparent;
      border-color: transparent;
      text-align: left;
    }
    .nav-item.active {
      background: #dfe8ee;
      border-color: #c9d4dd;
    }
    .panel-actions {
      display: grid;
      gap: 6px;
      margin: 8px 0 12px;
    }
    .panel-actions button {
      width: 100%;
      justify-content: center;
    }
    main {
      min-width: 0;
      min-height: 0;
      display: grid;
      grid-template-rows: minmax(0, 1fr) 180px;
    }
    .sheet {
      min-width: 0;
      min-height: 0;
      overflow: auto;
      padding: 12px;
    }
    .sheet-head {
      display: flex;
      align-items: baseline;
      gap: 10px;
      margin-bottom: 8px;
    }
    .sheet-tools {
      display: flex;
      gap: 8px;
      margin-left: auto;
      align-items: center;
    }
    .schema-form {
      display: grid;
      grid-template-columns: minmax(130px, 1fr) 170px 190px 190px 90px 110px;
      gap: 8px;
      margin-bottom: 10px;
      align-items: center;
      max-width: 980px;
    }
    .relation-layout {
      display: grid;
      grid-template-columns: minmax(220px, 1fr) minmax(220px, 1fr);
      gap: 12px;
      min-height: 360px;
    }
    .relation-pane {
      min-width: 0;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .relation-pane h2 {
      margin: 0;
      padding: 8px 10px;
      border-bottom: 1px solid var(--line);
      font-size: 13px;
      background: #f0f3f6;
    }
    .relation-row {
      width: 100%;
      display: grid;
      grid-template-columns: 72px minmax(0, 1fr) 80px;
      gap: 8px;
      align-items: center;
      height: 36px;
      border: 0;
      border-bottom: 1px solid var(--line);
      border-radius: 0;
      text-align: left;
    }
    .visual-layout {
      display: grid;
      grid-template-columns: 280px minmax(0, 1fr);
      gap: 12px;
      min-height: 520px;
    }
    .visual-list {
      border: 1px solid var(--line);
      background: var(--panel);
      overflow: auto;
    }
    .visual-list button {
      width: 100%;
      display: grid;
      grid-template-columns: minmax(0, 1fr) 80px;
      gap: 8px;
      border: 0;
      border-bottom: 1px solid var(--line);
      border-radius: 0;
      text-align: left;
    }
    .visual-list button.active {
      background: #dfe8ee;
      color: var(--accent);
      font-weight: 650;
    }
    .visual-preview {
      min-width: 0;
      display: grid;
      grid-template-rows: 360px auto;
      gap: 10px;
    }
    .visual-canvas {
      width: 100%;
      height: 360px;
      border: 1px solid var(--line);
      background: #202832;
    }
    .visual-states {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .import-form {
      display: flex;
      gap: 8px;
      align-items: center;
      min-width: 360px;
    }
    .import-form input {
      height: 32px;
      min-width: 280px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
    }
    .slicer-panel {
      margin-top: 12px;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .animation-panel {
      margin-top: 12px;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .state-machine-panel {
      margin-top: 12px;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .state-machine-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 8px 10px;
      border-bottom: 1px solid var(--line);
      background: #f0f3f6;
      font-weight: 650;
    }
    .state-machine-body {
      display: grid;
      grid-template-columns: minmax(360px, 1fr) 340px;
      gap: 12px;
      padding: 10px;
    }
    .state-list {
      border: 1px solid var(--line);
      min-height: 170px;
      max-height: 300px;
      overflow: auto;
    }
    .state-row {
      display: grid;
      grid-template-columns: 96px minmax(0, 1fr) 170px;
      gap: 8px;
      align-items: center;
      min-height: 40px;
      padding: 5px 6px;
      border-bottom: 1px solid var(--line);
    }
    .state-row.active {
      background: #eef7f5;
    }
    .state-row small {
      color: var(--muted);
    }
    .state-row select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      background: white;
    }
    .state-row-actions {
      display: flex;
      gap: 4px;
      justify-content: flex-end;
    }
    .state-row-actions button {
      padding: 0 8px;
    }
    .state-form {
      display: grid;
      grid-template-columns: 1fr;
      gap: 8px;
      align-content: start;
    }
    .state-form label {
      display: grid;
      gap: 3px;
      color: var(--muted);
      font-size: 11px;
      text-transform: uppercase;
    }
    .state-form input,
    .state-form select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      color: var(--text);
      background: white;
    }
    .asset-panel {
      margin-top: 12px;
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .operation-grid {
      display: grid;
      grid-template-columns: repeat(3, minmax(180px, 1fr));
      gap: 12px;
      padding: 12px;
    }
    .guild-shell {
      grid-column: 1 / -1;
      display: grid;
      grid-template-columns: minmax(220px, 34%) minmax(0, 1fr);
      min-height: 310px;
      border: 1px solid var(--line);
      background: #1a1512;
      overflow: hidden;
    }
    .expedition-rail {
      display: grid;
      grid-template-rows: repeat(6, 1fr);
      gap: 7px;
      padding: 10px;
      background: #111820;
    }
    .expedition-strip {
      border: 1px solid #31404c;
      background: linear-gradient(90deg, #263541, #1b252d);
      color: #dbe7ef;
      padding: 7px 9px;
      font-size: 12px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      min-height: 0;
    }
    .expedition-strip.empty {
      background: #1a232b;
      color: #7f8c99;
    }
    .guild-scene {
      position: relative;
      min-height: 310px;
      background:
        linear-gradient(180deg, rgba(88, 58, 38, 0.88), rgba(28, 19, 14, 0.96) 58%, #4a2f20 58%),
        repeating-linear-gradient(92deg, transparent 0 38px, rgba(255,255,255,0.04) 39px 41px);
    }
    .guild-scene::after {
      content: "";
      position: absolute;
      inset: 0;
      pointer-events: none;
      opacity: 0;
      transition: opacity 180ms ease;
    }
    .guild-scene.action-alchemy::after {
      opacity: 1;
      background: radial-gradient(circle at 39% 69%, rgba(112, 235, 164, 0.38), transparent 24%);
    }
    .guild-scene.action-forge::after {
      opacity: 1;
      background: radial-gradient(circle at 18% 72%, rgba(255, 108, 49, 0.42), transparent 24%);
    }
    .guild-scene.action-refinement::after {
      opacity: 1;
      background: radial-gradient(circle at 49% 66%, rgba(150, 190, 255, 0.38), transparent 23%);
    }
    .guild-scene.action-dispatch::after {
      opacity: 1;
      background: radial-gradient(circle at 64% 50%, rgba(227, 205, 134, 0.34), transparent 26%);
    }
    .guild-title {
      position: absolute;
      left: 24px;
      top: 18px;
      color: #f2dcc2;
      font-size: 20px;
      font-weight: 700;
    }
    .guild-object {
      position: absolute;
      color: #d9c3a4;
      font-size: 12px;
      text-align: center;
      z-index: 1;
      transition: filter 180ms ease, box-shadow 180ms ease, transform 180ms ease;
    }
    .guild-object.active {
      filter: brightness(1.35);
      transform: translateY(-2px);
    }
    .guild-forge {
      left: 12%;
      bottom: 38px;
      width: 100px;
      height: 82px;
      background: radial-gradient(circle at 50% 44%, rgba(222, 74, 37, 0.88), rgba(44,37,33,1) 38%);
      border-bottom: 16px solid #6d7780;
    }
    .guild-alchemy {
      left: 34%;
      bottom: 36px;
      width: 112px;
      height: 88px;
      border: 5px solid #8bbf9b;
      border-top: 0;
      border-radius: 0 0 56px 56px;
      background: linear-gradient(180deg, transparent 38%, rgba(96, 210, 146, 0.32) 39%);
    }
    .guild-door {
      left: 58%;
      bottom: 50px;
      width: 82px;
      height: 150px;
      background: #15110e;
      border: 5px solid #8d6a42;
    }
    .guild-bar {
      right: 7%;
      bottom: 54px;
      width: 150px;
      height: 62px;
      background: linear-gradient(180deg, #81512e 0 24px, #5a321e 25px);
    }
    .guild-refinement {
      left: 48%;
      bottom: 34px;
      width: 86px;
      height: 54px;
      border: 4px solid #8fa6c4;
      background: radial-gradient(circle at 50% 35%, rgba(156, 201, 255, 0.72), rgba(34, 42, 52, 0.96) 35%);
    }
    .guild-action-banner {
      position: absolute;
      left: 24px;
      right: 24px;
      top: 52px;
      z-index: 2;
      min-height: 44px;
      padding: 9px 12px;
      border: 1px solid rgba(242, 220, 194, 0.26);
      background: rgba(20, 14, 10, 0.74);
      color: #f2dcc2;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
    }
    .guild-action-banner small {
      color: #bda88e;
    }
    .guild-spark {
      position: absolute;
      z-index: 2;
      width: 8px;
      height: 8px;
      border-radius: 50%;
      opacity: 0;
      animation: guildSpark 900ms ease-out infinite;
    }
    .action-alchemy .guild-spark {
      left: 40%;
      bottom: 118px;
      background: #79f0ad;
    }
    .action-forge .guild-spark {
      left: 19%;
      bottom: 120px;
      background: #ff7748;
    }
    .action-refinement .guild-spark {
      left: 52%;
      bottom: 103px;
      background: #9ec8ff;
    }
    .action-dispatch .guild-spark {
      left: 65%;
      bottom: 203px;
      background: #e8c96a;
    }
    @keyframes guildSpark {
      0% { transform: translateY(10px) scale(0.55); opacity: 0; }
      25% { opacity: 1; }
      100% { transform: translateY(-30px) scale(1.2); opacity: 0; }
    }
    .operation-panel {
      border: 1px solid var(--line);
      background: var(--panel);
    }
    .operation-panel.wide {
      grid-column: 1 / -1;
    }
    .operation-head {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      padding: 10px 12px;
      border-bottom: 1px solid var(--line);
      font-weight: 700;
    }
    .operation-body {
      padding: 10px 12px;
      display: grid;
      gap: 8px;
    }
    .stat-line {
      display: flex;
      justify-content: space-between;
      gap: 12px;
      font-size: 13px;
    }
    .stat-line span:last-child {
      font-weight: 700;
      color: var(--text);
    }
    .asset-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 8px 10px;
      border-bottom: 1px solid var(--line);
      background: #f0f3f6;
      font-weight: 650;
    }
    .asset-body {
      display: grid;
      grid-template-columns: minmax(360px, 1fr) 360px;
      gap: 12px;
      padding: 10px;
    }
    .asset-browser {
      border: 1px solid var(--line);
      min-height: 260px;
      max-height: 360px;
      overflow: auto;
    }
    .asset-browser button {
      width: 100%;
      display: grid;
      grid-template-columns: minmax(0, 1fr) 84px;
      gap: 8px;
      border: 0;
      border-bottom: 1px solid var(--line);
      border-radius: 0;
      text-align: left;
    }
    .asset-browser small {
      color: var(--muted);
    }
    .asset-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
      align-content: start;
    }
    .asset-form label {
      display: grid;
      gap: 3px;
      color: var(--muted);
      font-size: 11px;
      text-transform: uppercase;
    }
    .asset-form input,
    .asset-form select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      color: var(--text);
      background: white;
    }
    .asset-form .wide {
      grid-column: 1 / -1;
    }
    .asset-preview {
      grid-column: 1 / -1;
      height: 170px;
      border: 1px solid var(--line);
      background: #202832;
      display: grid;
      place-items: center;
      overflow: hidden;
    }
    .asset-preview img {
      max-width: 100%;
      max-height: 100%;
      image-rendering: pixelated;
    }
    .animation-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 8px 10px;
      border-bottom: 1px solid var(--line);
      background: #f0f3f6;
      font-weight: 650;
    }
    .animation-body {
      display: grid;
      grid-template-columns: minmax(360px, 1fr) minmax(280px, 420px);
      gap: 12px;
      padding: 10px;
    }
    .animation-controls {
      display: grid;
      grid-template-columns: 1fr 110px 110px;
      gap: 8px;
      align-items: end;
      margin-bottom: 8px;
    }
    .animation-controls label {
      display: grid;
      gap: 3px;
      color: var(--muted);
      font-size: 11px;
      text-transform: uppercase;
    }
    .animation-controls input,
    .animation-controls select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      color: var(--text);
      background: white;
    }
    .animation-list {
      border: 1px solid var(--line);
      min-height: 220px;
      max-height: 320px;
      overflow: auto;
    }
    .animation-row {
      display: grid;
      grid-template-columns: 44px minmax(0, 1fr) 122px;
      gap: 8px;
      align-items: center;
      min-height: 38px;
      padding: 4px 6px;
      border-bottom: 1px solid var(--line);
    }
    .animation-row small {
      color: var(--muted);
    }
    .animation-row-actions {
      display: flex;
      gap: 4px;
      justify-content: flex-end;
    }
    .animation-row-actions button {
      width: 34px;
      padding: 0;
    }
    .frame-palette {
      border: 1px solid var(--line);
      min-height: 260px;
      max-height: 360px;
      overflow: auto;
    }
    .frame-palette button {
      width: 100%;
      display: grid;
      grid-template-columns: 72px minmax(0, 1fr);
      gap: 8px;
      border: 0;
      border-bottom: 1px solid var(--line);
      border-radius: 0;
      text-align: left;
    }
    .slicer-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 8px 10px;
      border-bottom: 1px solid var(--line);
      background: #f0f3f6;
      font-weight: 650;
    }
    .slicer-body {
      display: grid;
      grid-template-columns: minmax(360px, 1fr) 320px;
      gap: 12px;
      padding: 10px;
    }
    .slicer-canvas {
      width: 100%;
      height: 300px;
      border: 1px solid var(--line);
      background: #202832;
    }
    .slicer-form {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
      align-content: start;
    }
    .slicer-form label {
      display: grid;
      gap: 3px;
      color: var(--muted);
      font-size: 11px;
      text-transform: uppercase;
    }
    .slicer-form input,
    .slicer-form select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      color: var(--text);
      background: white;
    }
    .slicer-form .wide {
      grid-column: 1 / -1;
    }
    .nav-item.nested {
      padding-left: calc(10px + var(--depth, 0) * 18px);
    }
    .nav-item .owner {
      color: var(--muted);
      font-size: 11px;
    }
    .sheet-title {
      font-size: 18px;
      font-weight: 650;
    }
    .sheet-meta { color: var(--muted); font-size: 12px; }
    table {
      border-collapse: collapse;
      width: max-content;
      min-width: 100%;
      background: var(--panel);
      border: 1px solid var(--line);
    }
    th, td {
      border: 1px solid var(--line);
      padding: 0;
      height: 34px;
      min-width: 120px;
      max-width: 320px;
      font-size: 13px;
      vertical-align: middle;
    }
    th {
      position: sticky;
      top: 0;
      z-index: 1;
      background: #f0f3f6;
      text-align: left;
      padding: 0 8px;
      font-weight: 650;
    }
    td.key, th.key {
      min-width: 170px;
      background: #f8fafb;
      font-weight: 600;
    }
    .cell-input {
      width: 100%;
      height: 33px;
      border: 0;
      background: transparent;
      padding: 0 8px;
      font: inherit;
      color: var(--text);
    }
    .cell-input:focus {
      outline: 2px solid #8ccfc8;
      outline-offset: -2px;
      background: #fff;
    }
    select.cell-input {
      appearance: auto;
    }
    .schema-form input, .schema-form select {
      height: 32px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
      padding: 0 8px;
      font: inherit;
      font-size: 13px;
      min-width: 0;
    }
    .danger {
      color: var(--danger);
      border-color: #f3a19a;
    }
    .output {
      border-top: 1px solid var(--line);
      background: #111820;
      color: #dbe7ef;
      overflow: auto;
      padding: 10px 12px;
      font: 12px Consolas, monospace;
      white-space: pre-wrap;
    }
    @media (max-width: 800px) {
      .app { grid-template-columns: 1fr; grid-template-rows: auto 150px minmax(0, 1fr); }
      header { flex-wrap: wrap; height: auto; padding: 10px; }
      aside { grid-row: 2; border-right: 0; border-bottom: 1px solid var(--line); }
      main { grid-row: 3; grid-template-rows: minmax(0, 1fr) 160px; }
      .actions { width: 100%; overflow-x: auto; margin-left: 0; }
    }
  </style>
</head>
<body>
  <div class="app">
    <header>
      <h1>Belt Data Studio</h1>
      <div class="tabs">
        <button id="schemaTab" class="tab active">Schema</button>
        <button id="dataTab" class="tab">Data</button>
        <button id="visualTab" class="tab">Visual</button>
        <button id="operationTab" class="tab">Operation</button>
      </div>
      <span id="projectPath" class="status">loading</span>
      <span id="freshness" class="status">status</span>
      <div class="actions">
        <button id="validateBtn">Validate</button>
        <button id="codegenBtn">Codegen</button>
        <button id="buildBtn">Data Build</button>
        <button id="simulateBtn" class="primary">Simulate</button>
      </div>
    </header>
    <aside>
      <div id="schemaActions" class="panel-actions">
        <button id="addTableBtn">Add Table</button>
      </div>
      <div class="nav-title">Tables</div>
      <div id="tables"></div>
      <div id="viewsTitle" class="nav-title">Views</div>
      <div id="views"></div>
    </aside>
    <main>
      <section class="sheet">
        <div class="sheet-head">
          <div id="sheetTitle" class="sheet-title">Loading</div>
          <div id="sheetMeta" class="sheet-meta"></div>
          <div id="sheetTools" class="sheet-tools"></div>
        </div>
        <div id="grid"></div>
      </section>
      <pre id="output" class="output"></pre>
    </main>
  </div>
  <script>
    let state = { project: null, assets: [], account: null, mode: 'schema', selected: null, backStack: [], visual: { key: null, state: 'idle', started: 0 }, operationAction: null, images: {} };
    const $ = id => document.getElementById(id);

    async function api(path, options) {
      const res = await fetch(path, options);
      const json = await res.json();
      if (!res.ok || json.ok === false) throw new Error(json.error || res.statusText);
      return json;
    }

    function formatFloat(value) {
      const number = Number(value);
      if (!Number.isFinite(number)) return String(value);
      const rounded = number.toFixed(6).replace(/0+$/, '').replace(/\.$/, '');
      return rounded === '-0' ? '0' : rounded;
    }

    function formatDuration(seconds) {
      const total = Math.max(0, Math.floor(Number(seconds) || 0));
      const hours = Math.floor(total / 3600);
      const minutes = Math.floor((total % 3600) / 60);
      if (hours > 0) return `${hours}h ${minutes}m`;
      return `${minutes}m`;
    }

    function cellText(cell) {
      if (!cell || cell.kind === 'empty') return '';
      if (cell.kind === 'row') return String(cell.value);
      if (cell.kind === 'rows') return (cell.value || []).join(',');
      if (cell.kind === 'f32') return formatFloat(cell.value);
      return String(cell.value);
    }

    function tableData(tableId) {
      return state.project.data.find(t => t.table_id === tableId) || { rows: [] };
    }

    function tableByKey(key) {
      return state.project.tables.find(table => table.key === key);
    }

    function tableDataByKey(key) {
      const table = tableByKey(key);
      return table ? tableData(table.id) : { rows: [] };
    }

    function fieldByKey(tableKey, fieldKey) {
      return tableByKey(tableKey)?.fields.find(field => field.key === fieldKey);
    }

    function cellByKey(tableKey, row, fieldKey) {
      if (!row) return { kind: 'empty' };
      const field = fieldByKey(tableKey, fieldKey);
      return field ? fieldCell(row, field.id) : { kind: 'empty' };
    }

    function rowByKey(tableKey, rowId) {
      const table = tableByKey(tableKey);
      return table ? rowById(table.id, rowId) : null;
    }

    function cellStringByKey(tableKey, row, fieldKey, fallback = '') {
      const cell = cellByKey(tableKey, row, fieldKey);
      return cell?.kind === 'string' ? cell.value : fallback;
    }

    function cellNumberByKey(tableKey, row, fieldKey, fallback = 0) {
      const cell = cellByKey(tableKey, row, fieldKey);
      return ['i32', 'i64', 'f32'].includes(cell?.kind) ? Number(cell.value) : fallback;
    }

    function cellBoolByKey(tableKey, row, fieldKey, fallback = false) {
      const cell = cellByKey(tableKey, row, fieldKey);
      return cell?.kind === 'bool' ? Boolean(cell.value) : fallback;
    }

    function cellRowByKey(tableKey, row, fieldKey) {
      const cell = cellByKey(tableKey, row, fieldKey);
      return cell?.kind === 'row' ? cell.value : null;
    }

    function cellRowsByKey(tableKey, row, fieldKey) {
      const cell = cellByKey(tableKey, row, fieldKey);
      return cell?.kind === 'rows' ? cell.value : [];
    }

    function kindKey(kind) {
      return typeof kind === 'string' ? kind : kind.kind;
    }

    function kindLabel(kind) {
      const key = kindKey(kind);
      const target = kind.target_table ?? kind.nested_table;
      const table = target ? tableById(target) : null;
      return table ? `${key} -> ${table.display_name}` : key;
    }

    function fieldCell(row, fieldId) {
      return row.cells[String(fieldId)] || row.cells[fieldId] || { kind: 'empty' };
    }

    function tableById(tableId) {
      return state.project.tables.find(table => table.id === tableId);
    }

    function rowById(tableId, rowId) {
      return tableData(tableId).rows.find(row => row.id === rowId);
    }

    function rowTitle(tableId, rowId) {
      const row = rowById(tableId, rowId);
      if (!row) return `#${rowId}`;
      const table = tableById(tableId);
      const nameField = table?.fields.find(field => field.key === 'name') || table?.fields[0];
      const label = nameField ? cellText(fieldCell(row, nameField.id)) : '';
      return label ? `${label} (${row.key})` : row.key;
    }

    function relationTarget(kind) {
      return kind.target_table ?? kind.nested_table;
    }

    function displayNameFromKey(key) {
      return key.split('_')
        .filter(Boolean)
        .map(part => part.charAt(0).toUpperCase() + part.slice(1))
        .join(' ');
    }

    function nestedTableIds() {
      const ids = new Set();
      for (const table of state.project.tables) {
        for (const field of table.fields) {
          if (kindKey(field.kind) === 'owned_nested_table') ids.add(field.kind.nested_table);
        }
      }
      return ids;
    }

    function rootTables() {
      const nested = nestedTableIds();
      return state.project.tables.filter(table => !nested.has(table.id));
    }

    function childNestedFields(tableId) {
      const table = tableById(tableId);
      return table ? table.fields.filter(field => kindKey(field.kind) === 'owned_nested_table') : [];
    }

    function renderTableNavItem(table, depth, ownerLabel) {
      if (!table) return '';
      const active = state.selected?.type === 'table' && state.selected.key === table.key;
      const owner = ownerLabel ? `<span class="owner">${ownerLabel}</span>` : `<span>${table.key}</span>`;
      const self = `
        <button class="nav-item ${depth ? 'nested' : ''} ${active ? 'active' : ''}"
          style="--depth:${depth}" onclick="selectTable('${table.key}')">
          <span>${table.display_name}</span>${owner}
        </button>`;
      const children = childNestedFields(table.id)
        .map(field => renderTableNavItem(tableById(field.kind.nested_table), depth + 1, field.key))
        .join('');
      return self + children;
    }

    function isRelationKind(kind) {
      return ['relation_one', 'relation_many', 'reference_group', 'owned_nested_table'].includes(kindKey(kind));
    }

    function relationCellLabel(field, cell) {
      const target = relationTarget(field.kind);
      if (!cell || cell.kind === 'empty') return 'Select';
      if (cell.kind === 'row') return rowTitle(target, cell.value);
      if (cell.kind === 'rows') return `${cell.value.length} selected`;
      return cellText(cell);
    }

    function renderNav() {
      document.querySelector('.app').classList.toggle('visual-mode', state.mode === 'visual' || state.mode === 'operation');
      $('tables').innerHTML = rootTables().map(table => renderTableNavItem(table, 0, '')).join('');
      $('views').style.display = state.mode === 'data' ? '' : 'none';
      $('viewsTitle').style.display = state.mode === 'data' ? '' : 'none';
      $('schemaActions').style.display = state.mode === 'schema' ? 'grid' : 'none';
      $('views').innerHTML = state.mode === 'data' ? state.project.views.map(view => `
        <button class="nav-item ${state.selected?.type === 'view' && state.selected.key === view.key ? 'active' : ''}"
          onclick="selectView('${view.key}')">
          <span>${view.display_name}</span><span>${view.key}</span>
        </button>`).join('') : '';
      $('schemaTab').classList.toggle('active', state.mode === 'schema');
      $('dataTab').classList.toggle('active', state.mode === 'data');
      $('visualTab').classList.toggle('active', state.mode === 'visual');
      $('operationTab').classList.toggle('active', state.mode === 'operation');
    }

    function renderStatus(status) {
      $('freshness').textContent = `${status.status} / ${status.issues.length} issue(s)`;
      $('freshness').className = 'status';
      if (status.status !== 'all_fresh') $('freshness').classList.add('dirty');
      if (status.issues.some(issue => issue.severity === 'Error')) $('freshness').classList.add('bad');
    }

    function renderTable(table) {
      const data = tableData(table.id);
      $('sheetTitle').textContent = table.display_name;
      $('sheetMeta').textContent = `${table.key} / ${data.rows.length} rows`;
      $('sheetTools').innerHTML = `<button onclick="addRow(${table.id})">Add Row</button>`;
      const headers = [`<th class="key">key</th>`, ...table.fields.map(field => `<th>${field.display_name}<br><small>${kindLabel(field.kind)}</small></th>`), `<th>Action</th>`].join('');
      const rows = data.rows.map(row => {
        const cells = table.fields.map(field => {
          const value = cellText(fieldCell(row, field.id));
          if (isRelationKind(field.kind)) {
            return `<td><button onclick="openRelationPicker(${table.id}, ${row.id}, ${field.id})">${escapeHtml(relationCellLabel(field, fieldCell(row, field.id)))}</button></td>`;
          }
          return `<td><input class="cell-input" value="${escapeAttr(value)}"
            onchange="updateCell(${table.id}, ${row.id}, ${field.id}, this.value)"></td>`;
        }).join('');
        return `<tr><td class="key">${row.key}<br><small>#${row.id}</small></td>${cells}<td><button class="danger" onclick="deleteRow(${table.id}, ${row.id})">Delete</button></td></tr>`;
      }).join('');
      $('grid').innerHTML = `<table><thead><tr>${headers}</tr></thead><tbody>${rows}</tbody></table>`;
    }

    function renderSchemaTable(table) {
      $('sheetTitle').textContent = table.display_name;
      $('sheetMeta').textContent = `${table.key} / ${table.fields.length} fields`;
      $('sheetTools').innerHTML = `
        <button class="danger" onclick="deleteTable(${table.id})">Delete Table</button>`;
      const targetOptions = rootTables()
        .map(target => `<option value="${target.id}">${target.display_name} (${target.key})</option>`)
        .join('');
      const form = `
        <div class="schema-form">
          <input id="fieldKey" placeholder="field_key" oninput="syncFieldTarget()">
          <select id="fieldKind" onchange="syncFieldTarget()">
            <option value="string">string</option>
            <option value="text">text</option>
            <option value="bool">bool</option>
            <option value="i32">i32</option>
            <option value="i64">i64</option>
            <option value="f32">f32</option>
            <option value="relation_one">relation one</option>
            <option value="relation_many">relation many</option>
            <option value="reference_group">reference group</option>
            <option value="owned_nested_table">owned nested table</option>
          </select>
          <select id="fieldTarget">${targetOptions}</select>
          <input id="nestedKey" placeholder="nested_table_key">
          <label><input id="fieldRequired" type="checkbox"> required</label>
          <button onclick="addFieldFromForm(${table.id})">Add Field</button>
        </div>`;
      const rows = table.fields.map(field => `
        <tr>
          <td class="key">${field.display_name}<br><small>${field.key}</small></td>
          <td><input class="cell-input" readonly value="${escapeAttr(kindLabel(field.kind))}"></td>
          <td><input class="cell-input" readonly value="${field.required ? 'required' : 'optional'}"></td>
          <td><button class="danger" onclick="deleteField(${table.id}, ${field.id})">Remove</button></td>
        </tr>`).join('');
      $('grid').innerHTML = `${form}
        <table>
          <thead><tr><th class="key">Field</th><th>Type</th><th>Rule</th><th>Action</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>`;
      syncFieldTarget();
    }

    async function renderView(viewKey) {
      const data = await api(`/api/view?view=${viewKey}`);
      const view = state.project.views.find(v => v.key === viewKey);
      $('sheetTitle').textContent = view.display_name;
      $('sheetMeta').textContent = `${view.key} / ${data.view.rows.length} rows`;
      $('sheetTools').innerHTML = '';
      const headers = data.view.headers.map(header => `<th>${header}</th>`).join('');
      const rows = data.view.rows.map(row => `<tr>${row.map(value => `<td><input class="cell-input" readonly value="${escapeAttr(value)}"></td>`).join('')}</tr>`).join('');
      $('grid').innerHTML = `<table><thead><tr>${headers}</tr></thead><tbody>${rows}</tbody></table>`;
    }

    function renderRelationPicker(selection) {
      const sourceTable = tableById(selection.tableId);
      const sourceRow = rowById(selection.tableId, selection.rowId);
      const field = sourceTable.fields.find(field => field.id === selection.fieldId);
      const targetTableId = relationTarget(field.kind);
      const targetTable = tableById(targetTableId);
      const cell = fieldCell(sourceRow, field.id);
      const selectedIds = cell?.kind === 'row'
        ? [cell.value]
        : cell?.kind === 'rows'
          ? [...cell.value]
          : [];
      const selectedSet = new Set(selectedIds);
      const availableRows = tableData(targetTableId).rows.filter(row => !selectedSet.has(row.id));
      const selectedRows = selectedIds.map(id => rowById(targetTableId, id)).filter(Boolean);
      const nested = kindKey(field.kind) === 'owned_nested_table';

      $('sheetTitle').textContent = `${sourceRow.key}.${field.key}`;
      $('sheetMeta').textContent = `${sourceTable.display_name} -> ${targetTable.display_name}`;
      $('sheetTools').innerHTML = `
        ${nested ? `<button onclick="addNestedRow(${selection.tableId}, ${selection.rowId}, ${selection.fieldId})">Add Nested Row</button>` : ''}
        <button onclick="goBack()">Back</button>`;
      $('grid').innerHTML = `
        <div class="relation-layout">
          <div class="relation-pane">
            <h2>${targetTable.display_name}</h2>
            ${availableRows.map(row => relationRowButton(targetTableId, row, 'Add', `setRelationValue(${selection.tableId}, ${selection.rowId}, ${selection.fieldId}, ${row.id}, true)`)).join('')}
          </div>
          <div class="relation-pane">
            <h2>Selected</h2>
            ${selectedRows.map(row => relationRowButton(targetTableId, row, 'Remove', `setRelationValue(${selection.tableId}, ${selection.rowId}, ${selection.fieldId}, ${row.id}, false)`)).join('')}
          </div>
        </div>`;
    }

    async function loadAccountState() {
      const account = await api('/api/account-state');
      state.account = account;
      return account;
    }

    function setOperationAction(kind, label, detail = '') {
      state.operationAction = {
        kind,
        label,
        detail,
        expiresAt: Date.now() + 4200
      };
      setTimeout(() => {
        if (state.mode === 'operation' && state.operationAction && state.operationAction.expiresAt <= Date.now()) {
          renderOperationDashboard().catch(error => log(`error: ${error.message}`));
        }
      }, 4300);
    }

    function currentOperationAction() {
      const action = state.operationAction;
      if (!action || action.expiresAt <= Date.now()) {
        state.operationAction = null;
        return { kind: 'idle', label: 'Guild House Ready', detail: 'No active work feedback.' };
      }
      return action;
    }

    async function renderOperationDashboard() {
      const account = state.account || await loadAccountState();
      const action = currentOperationAction();
      $('sheetTitle').textContent = 'Operation';
      $('sheetMeta').textContent = `local account / ${account.path}`;
      $('sheetTools').innerHTML = `
        <button class="primary" onclick="dispatchDungeon()">Dispatch Dungeon</button>
        <button onclick="recoverEnergy()">Recover Energy</button>
        <button onclick="refreshOperation()">Refresh</button>`;
      const storage = account.storage_tabs || [];
      const inventory = account.inventory || [];
      const heroes = account.heroes || [];
      const equipment = account.equipment || [];
      const mail = account.mail || [];
      const alchemyRecipes = account.alchemy_recipes || [];
      const forgeRecipes = account.forge_recipes || [];
      const refinementRecipes = account.refinement_recipes || [];
      const byCategory = category => inventory.filter(item => item.category === category);
      $('grid').innerHTML = `
        <div class="operation-grid">
          <div class="guild-shell">
            <div class="expedition-rail">
              <div class="expedition-strip"><span>Party 1</span><span>Endless Left Road</span></div>
              <div class="expedition-strip empty"><span>Party 2</span><span>empty</span></div>
              <div class="expedition-strip empty"><span>Party 3</span><span>empty</span></div>
              <div class="expedition-strip empty"><span>Party 4</span><span>empty</span></div>
              <div class="expedition-strip empty"><span>Party 5</span><span>empty</span></div>
              <div class="expedition-strip empty"><span>Party 6</span><span>empty</span></div>
            </div>
            <div class="guild-scene action-${escapeAttr(action.kind)}">
              <div class="guild-title">Guild House</div>
              <div class="guild-action-banner">
                <span>${escapeHtml(action.label)}</span>
                <small>${escapeHtml(action.detail)}</small>
              </div>
              <div class="guild-spark"></div>
              <div class="guild-object guild-forge ${action.kind === 'forge' ? 'active' : ''}">Forge</div>
              <div class="guild-object guild-alchemy ${action.kind === 'alchemy' ? 'active' : ''}">Alchemy<br>Furnace</div>
              <div class="guild-object guild-refinement ${action.kind === 'refinement' ? 'active' : ''}">Refinement<br>Workbench</div>
              <div class="guild-object guild-door ${action.kind === 'dispatch' ? 'active' : ''}">Dungeon<br>Door</div>
              <div class="guild-object guild-bar">Tavern</div>
            </div>
          </div>
          <div class="operation-panel">
            <div class="operation-head"><span>Energy</span><span>${account.energy_after_recovery}/${account.max_energy}</span></div>
            <div class="operation-body">
              <div class="stat-line"><span>Stored</span><span>${account.energy}</span></div>
              <div class="stat-line"><span>Recoverable</span><span>${account.recoverable_energy}</span></div>
              <div class="stat-line"><span>Next</span><span>${formatDuration(account.seconds_until_next_recovery)}</span></div>
              <div class="stat-line"><span>Rate</span><span>${account.recover_amount}/${formatDuration(account.recover_seconds)}</span></div>
              <div class="stat-line"><span>Expired Mail</span><span>${account.expired_mail_removed}</span></div>
            </div>
          </div>
          ${storage.map(tab => `
            <div class="operation-panel">
              <div class="operation-head"><span>${escapeHtml(tab.name)}</span><span>${tab.used_slots}/${tab.capacity}</span></div>
              <div class="operation-body">
                <div class="stat-line"><span>Category</span><span>${escapeHtml(tab.item_category)}</span></div>
                <div class="stat-line"><span>Free Slots</span><span>${tab.free_slots}</span></div>
              </div>
            </div>`).join('')}
          <div class="operation-panel wide">
            <div class="operation-head"><span>Warehouse</span><span>${inventory.length} stacks</span></div>
            <table>
              <thead><tr><th>Item</th><th>Category</th><th>Rarity</th><th>Qty</th><th>Stack</th></tr></thead>
              <tbody>
                ${inventory.map(item => `
                  <tr>
                    <td>${escapeHtml(item.name)}<br><small>${escapeHtml(item.item_key)}</small></td>
                    <td>${escapeHtml(item.category)}</td>
                    <td>${escapeHtml(item.rarity)}</td>
                    <td>${item.quantity}</td>
                    <td>${item.stack_size}</td>
                  </tr>`).join('') || '<tr><td colspan="5">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Heroes</span><span>${heroes.length}</span></div>
            <table>
              <thead><tr><th>Hero</th><th>Unit</th><th>Equipment Slots</th></tr></thead>
              <tbody>
                ${heroes.map(hero => `
                  <tr>
                    <td>${escapeHtml(hero.name)}<br><small>${escapeHtml(hero.hero_id)}</small></td>
                    <td>${escapeHtml(hero.unit_key)}</td>
                    <td>${(hero.equipment_slots || []).map(slot => `
                      <div class="stat-line">
                        <span>${escapeHtml(slot.slot_key)}: ${escapeHtml(slot.name)}</span>
                        <button onclick="unequipHero('${escapeAttr(hero.hero_id)}', '${escapeAttr(slot.slot_key)}')">Unequip</button>
                      </div>`).join('') || 'empty'}</td>
                  </tr>`).join('') || '<tr><td colspan="3">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Alchemy Furnace</span><span>${alchemyRecipes.length} recipes</span></div>
            <table>
              <thead><tr><th>Recipe</th><th>Ingredients</th><th>Output</th><th>Action</th></tr></thead>
              <tbody>
                ${alchemyRecipes.map(recipe => `
                  <tr>
                    <td>${escapeHtml(recipe.name)}<br><small>${escapeHtml(recipe.key)}</small></td>
                    <td>${recipe.ingredients.map(item => `${escapeHtml(item.name)} ${item.available}/${item.quantity}`).join('<br>')}</td>
                    <td>${escapeHtml(recipe.output_name)} x${recipe.output_quantity}</td>
                    <td><button onclick="craftAlchemy('${escapeAttr(recipe.key)}')" ${recipe.craftable ? '' : 'disabled'}>Craft</button></td>
                  </tr>`).join('') || '<tr><td colspan="4">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Forge</span><span>${forgeRecipes.length} recipes</span></div>
            <table>
              <thead><tr><th>Recipe</th><th>Slots</th><th>Output</th><th>Action</th></tr></thead>
              <tbody>
                ${forgeRecipes.map(recipe => `
                  <tr>
                    <td>${escapeHtml(recipe.name)}<br><small>${escapeHtml(recipe.key)}</small></td>
                    <td>${recipe.ingredients.map(item => `${escapeHtml(item.slot_kind)}: ${escapeHtml(item.name)} ${item.available}/${item.quantity}`).join('<br>')}</td>
                    <td>${escapeHtml(recipe.output_name)} x${recipe.output_quantity}</td>
                    <td><button onclick="craftForge('${escapeAttr(recipe.key)}')" ${recipe.craftable ? '' : 'disabled'}>Forge</button></td>
                  </tr>`).join('') || '<tr><td colspan="4">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Refinement Workbench</span><span>${refinementRecipes.length} recipes</span></div>
            <table>
              <thead><tr><th>Recipe</th><th>Input</th><th>Material</th><th>Output</th><th>Action</th></tr></thead>
              <tbody>
                ${refinementRecipes.map(recipe => `
                  <tr>
                    <td>${escapeHtml(recipe.name)}<br><small>${escapeHtml(recipe.effect_kind)}</small></td>
                    <td>${escapeHtml(recipe.input_name)} ${recipe.input_available}/1</td>
                    <td>${escapeHtml(recipe.material_name)} ${recipe.material_available}/${recipe.material_quantity}</td>
                    <td>${escapeHtml(recipe.output_name)}</td>
                    <td><button onclick="craftRefinement('${escapeAttr(recipe.key)}')" ${recipe.craftable ? '' : 'disabled'}>Refine</button></td>
                  </tr>`).join('') || '<tr><td colspan="5">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel">
            <div class="operation-head"><span>Material</span><span>${byCategory('material').length}</span></div>
            <div class="operation-body">${byCategory('material').map(item => `<div class="stat-line"><span>${escapeHtml(item.name)}</span><span>${item.quantity}</span></div>`).join('') || '<span>empty</span>'}</div>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Equipment Instances</span><span>${equipment.length}</span></div>
            <table>
              <thead><tr><th>Equipment</th><th>Rarity</th><th>Stat Options</th><th>Special Options</th><th>Equip</th></tr></thead>
              <tbody>
                ${equipment.map(item => `
                  <tr>
                    <td>${escapeHtml(item.name)}<br><small>${escapeHtml(item.instance_id)}</small></td>
                    <td>${escapeHtml(item.rarity)}</td>
                    <td>${(item.options || []).map(option => `${escapeHtml(option.stat_key)} +${option.value} <small>${escapeHtml(option.rarity)}</small>`).join('<br>') || 'none'}</td>
                    <td>${(item.special_options || []).map(option => `${escapeHtml(option.name)} <small>${escapeHtml(option.rarity)}</small><br><small>${escapeHtml(option.effect_summary)}</small>`).join('<br>') || 'none'}</td>
                    <td>${heroes.map(hero => `<button onclick="equipHero('${escapeAttr(hero.hero_id)}', 'main_hand', '${escapeAttr(item.instance_id)}')">${escapeHtml(hero.name)}</button>`).join('') || 'no heroes'}</td>
                  </tr>`).join('') || '<tr><td colspan="5">empty</td></tr>'}
              </tbody>
            </table>
          </div>
          <div class="operation-panel">
            <div class="operation-head"><span>Consumable</span><span>${byCategory('consumable').length}</span></div>
            <div class="operation-body">${byCategory('consumable').map(item => `<div class="stat-line"><span>${escapeHtml(item.name)}</span><span>${item.quantity}</span></div>`).join('') || '<span>empty</span>'}</div>
          </div>
          <div class="operation-panel wide">
            <div class="operation-head"><span>Mail</span><span>${mail.length}</span></div>
            <table>
              <thead><tr><th>Item</th><th>Category</th><th>Qty</th><th>Remaining</th><th>Action</th></tr></thead>
              <tbody>
                ${mail.map(item => `
                  <tr>
                    <td>${escapeHtml(item.name)}<br><small>${escapeHtml(item.item_key)}</small></td>
                    <td>${escapeHtml(item.category)}</td>
                    <td>${item.quantity}</td>
                    <td>${formatDuration(item.remaining_seconds)}</td>
                    <td>
                      <button onclick="claimMail(${item.index})" ${item.expired ? 'disabled' : ''}>Claim</button>
                      <button class="danger" onclick="deleteMail(${item.index})">Delete</button>
                    </td>
                  </tr>`).join('') || '<tr><td colspan="5">empty</td></tr>'}
              </tbody>
            </table>
          </div>
        </div>`;
    }

    async function refreshOperation() {
      try {
        await loadAccountState();
        await renderOperationDashboard();
        log('account state refreshed');
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function dispatchDungeon() {
      try {
        const result = await api('/api/account-dispatch', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ map_key: 'endless_left_road', seed: Date.now() })
        });
        state.account = result.account;
        setOperationAction('dispatch', 'Dungeon Dispatch Started', result.message || 'Party left through the dungeon door.');
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function recoverEnergy() {
      try {
        const result = await api('/api/account-energy/recover', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: '{}'
        });
        state.account = result.account;
        setOperationAction('alchemy', 'Alchemy Furnace Complete', result.message || `Crafted ${recipeKey}.`);
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function claimMail(mailIndex) {
      try {
        const result = await api('/api/account-mail/claim', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mail_index: mailIndex })
        });
        state.account = result.account;
        setOperationAction('forge', 'Forge Complete', result.message || `Forged ${recipeKey}.`);
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function deleteMail(mailIndex) {
      if (!confirm(`Delete mail #${mailIndex}?`)) return;
      try {
        const result = await api('/api/account-mail/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ mail_index: mailIndex })
        });
        state.account = result.account;
        setOperationAction('refinement', 'Refinement Complete', result.message || `Refined ${recipeKey}.`);
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function craftAlchemy(recipeKey) {
      try {
        const result = await api('/api/account-alchemy/craft', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ recipe_key: recipeKey })
        });
        state.account = result.account;
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function craftForge(recipeKey) {
      try {
        const result = await api('/api/account-forge/craft', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ recipe_key: recipeKey })
        });
        state.account = result.account;
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function craftRefinement(recipeKey) {
      try {
        const result = await api('/api/account-refinement/craft', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ recipe_key: recipeKey })
        });
        state.account = result.account;
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function equipHero(heroId, slotKey, equipmentInstanceId) {
      try {
        const result = await api('/api/account-hero/equip', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ hero_id: heroId, slot_key: slotKey, equipment_instance_id: equipmentInstanceId })
        });
        state.account = result.account;
        setOperationAction('forge', 'Hero Equipped', result.message || `Equipped ${equipmentInstanceId}.`);
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function unequipHero(heroId, slotKey) {
      try {
        const result = await api('/api/account-hero/unequip', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ hero_id: heroId, slot_key: slotKey })
        });
        state.account = result.account;
        setOperationAction('forge', 'Hero Unequipped', result.message || `Unequipped ${heroId}.${slotKey}.`);
        await renderOperationDashboard();
        log(result.message);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function renderVisualDashboard() {
      const visuals = tableDataByKey('unit_visual').rows;
      const selected = visuals.find(row => row.key === state.visual.key) || visuals[0];
      if (!selected) {
        $('sheetTitle').textContent = 'Visual Preview';
        $('sheetMeta').textContent = 'no unit_visual rows';
        $('sheetTools').innerHTML = '';
        $('grid').innerHTML = '';
        return;
      }
      state.visual.key = selected.key;
      const visualName = cellStringByKey('unit_visual', selected, 'name', selected.key);
      $('sheetTitle').textContent = visualName;
      $('sheetMeta').textContent = `${selected.key} / unit visual preview`;
      $('sheetTools').innerHTML = `
        <div class="import-form">
          <input id="asepriteFile" list="asepriteHints" placeholder="C:\\path\\unit.aseprite or exported.json">
          <datalist id="asepriteHints">
            ${tableDataByKey('texture_asset').rows.map(row => `<option value="${escapeAttr(cellStringByKey('texture_asset', row, 'path', ''))}"></option>`).join('')}
          </datalist>
          <button onclick="importAseprite()">Import Aseprite</button>
        </div>`;
      $('grid').innerHTML = `
        <div class="visual-layout">
          <div class="visual-list">
            ${visuals.map(row => `<button class="${row.key === selected.key ? 'active' : ''}" onclick="selectVisual('${row.key}')">
              <span>${escapeHtml(cellStringByKey('unit_visual', row, 'name', row.key))}</span>
              <span>${row.key}</span>
            </button>`).join('')}
          </div>
          <div class="visual-preview">
            <canvas id="visualCanvas" class="visual-canvas"></canvas>
            <div id="visualStates" class="visual-states"></div>
          </div>
        </div>
        ${renderAssetBrowser()}
        ${renderStateMachineEditor(selected)}
        ${renderAnimationEditor(selected)}
        ${renderSpriteSlicer()}`;
      renderVisualStateButtons(selected);
      setTimeout(updateSlicePreview, 0);
      drawVisualPreview();
    }

    function selectVisual(key) {
      state.visual.key = key;
      state.visual.state = 'idle';
      state.visual.started = performance.now();
      renderVisualDashboard();
    }

    function selectVisualState(key) {
      state.visual.state = key;
      state.visual.started = performance.now();
      renderVisualDashboard();
    }

    async function importAseprite() {
      const file = $('asepriteFile')?.value.trim();
      if (!file) return;
      try {
        const result = await api('/api/import/aseprite', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ file })
        });
        log(`imported ${result.texture_key}: ${result.frame_count} frames, ${result.animation_count} animations`);
        state.images = {};
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function renderAssetBrowser() {
      const textures = tableDataByKey('texture_asset').rows;
      const selectedTexture = textures.find(row => row.id === Number(state.visual.textureId)) || textures[0] || null;
      const selectedPath = state.visual.assetPath || cellStringByKey('texture_asset', selectedTexture, 'path', state.assets[0]?.path || '');
      const textureOptions = textures.map(row => `<option value="${row.id}" ${selectedTexture?.id === row.id ? 'selected' : ''}>${escapeHtml(cellStringByKey('texture_asset', row, 'name', row.key))}</option>`).join('');
      const asset = state.assets.find(item => item.path === selectedPath);
      return `
        <div class="asset-panel">
          <div class="asset-head">
            <span>Project Assets</span>
            <span>${state.assets.length} image files / ${textures.length} texture assets</span>
          </div>
          <div class="asset-body">
            <div class="asset-browser">
              ${state.assets.map(item => `
                <button onclick="selectAssetPath(${JSON.stringify(item.path)})">
                  <span>${escapeHtml(item.path)}<br><small>${escapeHtml(item.content_type)}</small></span>
                  <span>${formatBytes(item.bytes)}</span>
                </button>`).join('') || `<button disabled><span>No image assets under project assets</span><span></span></button>`}
            </div>
            <div class="asset-form">
              <label class="wide">Texture Row
                <select id="textureEditRow" onchange="selectTextureRow(this.value)">
                  ${textureOptions || '<option value="">new texture</option>'}
                </select>
              </label>
              <label class="wide">Path
                <input id="texturePath" value="${escapeAttr(selectedPath)}" oninput="previewTexturePath(this.value)">
              </label>
              <label class="wide">Name
                <input id="textureName" value="${escapeAttr(cellStringByKey('texture_asset', selectedTexture, 'name', assetNameFromPath(selectedPath)))}">
              </label>
              <label>Width
                <input id="textureWidth" type="number" min="0" value="${escapeAttr(cellNumberByKey('texture_asset', selectedTexture, 'width', 0))}">
              </label>
              <label>Height
                <input id="textureHeight" type="number" min="0" value="${escapeAttr(cellNumberByKey('texture_asset', selectedTexture, 'height', 0))}">
              </label>
              <div class="asset-preview">
                ${selectedPath ? `<img id="texturePreview" src="/asset?path=${encodeURIComponent(selectedPath)}" onload="syncTextureDimensions(this)">` : ''}
              </div>
              <button class="primary" onclick="createTextureAsset()">Create Texture</button>
              <button onclick="updateTextureAsset()">Update Texture</button>
            </div>
          </div>
        </div>`;
    }

    function formatBytes(bytes) {
      const value = Number(bytes || 0);
      if (value >= 1024 * 1024) return `${formatFloat(value / (1024 * 1024))} MB`;
      if (value >= 1024) return `${formatFloat(value / 1024)} KB`;
      return `${value} B`;
    }

    function assetNameFromPath(path) {
      const file = String(path || '').split('/').pop() || 'texture';
      return displayNameFromKey(file.replace(/\.[^.]+$/, '').replaceAll('-', '_'));
    }

    function keyFromPath(path) {
      return normalizedDataKey(String(path || '').split('/').pop()?.replace(/\.[^.]+$/, '') || 'texture', 'texture');
    }

    function uniqueTextureKey(base) {
      const existing = new Set(tableDataByKey('texture_asset').rows.map(row => row.key));
      if (!existing.has(base)) return base;
      for (let index = 2; ; index++) {
        const candidate = `${base}_${index}`;
        if (!existing.has(candidate)) return candidate;
      }
    }

    function selectAssetPath(path) {
      state.visual.assetPath = path;
      renderVisualDashboard();
    }

    function selectTextureRow(rowId) {
      state.visual.textureId = Number(rowId || 0);
      const texture = rowByKey('texture_asset', state.visual.textureId);
      state.visual.assetPath = cellStringByKey('texture_asset', texture, 'path', state.visual.assetPath || '');
      renderVisualDashboard();
    }

    function previewTexturePath(path) {
      const img = $('texturePreview');
      if (img) img.src = `/asset?path=${encodeURIComponent(path)}`;
    }

    function syncTextureDimensions(image) {
      const width = $('textureWidth');
      const height = $('textureHeight');
      if (width && Number(width.value || 0) === 0) width.value = image.naturalWidth || 0;
      if (height && Number(height.value || 0) === 0) height.value = image.naturalHeight || 0;
    }

    async function createTextureAsset() {
      const path = $('texturePath')?.value.trim();
      if (!path) return;
      const key = uniqueTextureKey(keyFromPath(path));
      try {
        const created = await api('/api/row', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 6, key })
        });
        await saveTextureFields(created.row_id);
        state.visual.textureId = created.row_id;
        state.visual.assetPath = path;
        log(`created texture ${key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function updateTextureAsset() {
      const rowId = Number($('textureEditRow')?.value || 0);
      if (!rowId) return;
      try {
        await saveTextureFields(rowId);
        state.visual.textureId = rowId;
        state.visual.assetPath = $('texturePath')?.value.trim();
        state.images = {};
        log(`updated texture #${rowId}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function saveTextureFields(rowId) {
      const fields = [
        [44, $('textureName')?.value.trim() || assetNameFromPath($('texturePath')?.value)],
        [45, $('texturePath')?.value.trim()],
        [46, String(Math.trunc(Number($('textureWidth')?.value || 0)))],
        [47, String(Math.trunc(Number($('textureHeight')?.value || 0)))]
      ];
      for (const [fieldId, value] of fields) {
        await api('/api/cell', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 6, row_id: rowId, field_id: fieldId, value: String(value) })
        });
      }
    }

    function visualStateMachine(visualRow) {
      const machineId = cellRowByKey('unit_visual', visualRow, 'state_machine');
      return machineId ? rowByKey('visual_state_machine', machineId) : null;
    }

    function renderStateMachineEditor(visualRow) {
      const machine = visualStateMachine(visualRow);
      if (!machine) {
        return `
          <div class="state-machine-panel">
            <div class="state-machine-head">
              <span>State Machine</span>
              <span>no state machine</span>
            </div>
          </div>`;
      }
      const states = visualStateRows(visualRow);
      const defaultId = cellRowByKey('visual_state_machine', machine, 'default_state');
      const animations = tableDataByKey('sprite_animation').rows;
      const animationOptions = animationId => animations
        .map(row => `<option value="${row.id}" ${row.id === animationId ? 'selected' : ''}>${escapeHtml(cellStringByKey('sprite_animation', row, 'name', row.key))}</option>`)
        .join('');
      return `
        <div class="state-machine-panel">
          <div class="state-machine-head">
            <span>State Machine</span>
            <span>${escapeHtml(cellStringByKey('visual_state_machine', machine, 'name', machine.key))} / ${states.length} states</span>
          </div>
          <div class="state-machine-body">
            <div class="state-list">
              ${states.map(row => {
                const key = cellStringByKey('visual_state', row, 'state_key', row.key);
                const animationId = cellRowByKey('visual_state', row, 'animation');
                return `
                  <div class="state-row ${key === state.visual.state ? 'active' : ''}">
                    <span>${escapeHtml(key)}<br><small>${row.id === defaultId ? 'default' : row.key}</small></span>
                    <select onchange="setVisualStateAnimation(${row.id}, this.value)">
                      ${animationOptions(animationId)}
                    </select>
                    <span class="state-row-actions">
                      <button onclick="selectVisualState('${escapeAttr(key)}')">View</button>
                      <button ${row.id === defaultId ? 'disabled' : ''} onclick="setDefaultVisualState(${machine.id}, ${row.id})">Default</button>
                      <button class="danger" ${states.length <= 1 ? 'disabled' : ''} onclick="deleteVisualState(${machine.id}, ${row.id})">Del</button>
                    </span>
                  </div>`;
              }).join('') || `<div class="state-row"><span></span><span>No states</span><span></span></div>`}
            </div>
            <div class="state-form">
              <label>State Key
                <input id="newStateKey" placeholder="attack">
              </label>
              <label>Name
                <input id="newStateName" placeholder="Attack">
              </label>
              <label>Animation
                <select id="newStateAnimation">${animationOptions(animations[0]?.id)}</select>
              </label>
              <button class="primary" onclick="addVisualState(${machine.id})">Add State</button>
            </div>
          </div>
        </div>`;
    }

    function normalizedDataKey(value, fallback = 'state') {
      const key = String(value || '')
        .trim()
        .toLowerCase()
        .replaceAll(/[^a-z0-9_]+/g, '_')
        .replaceAll(/_+/g, '_')
        .replaceAll(/^_+|_+$/g, '');
      return /^[a-z]/.test(key) ? key : fallback;
    }

    async function saveMachineStateIds(machineId, stateIds) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 8, row_id: machineId, field_id: 62, value: stateIds.join(',') })
      });
    }

    async function addVisualState(machineId) {
      const machine = rowByKey('visual_state_machine', machineId);
      const stateKey = normalizedDataKey($('newStateKey')?.value, 'state');
      const name = $('newStateName')?.value.trim() || displayNameFromKey(stateKey);
      const animationId = Number($('newStateAnimation')?.value || 0);
      if (!machine || !animationId) return;
      const rowKey = normalizedDataKey(`${machine.key}_${stateKey}`, 'visual_state');
      try {
        const created = await api('/api/row', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 9, key: rowKey })
        });
        await api('/api/cell', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 9, row_id: created.row_id, field_id: 70, value: name })
        });
        await api('/api/cell', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 9, row_id: created.row_id, field_id: 71, value: stateKey })
        });
        await api('/api/cell', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 9, row_id: created.row_id, field_id: 72, value: String(animationId) })
        });
        const stateIds = [...cellRowsByKey('visual_state_machine', machine, 'states'), created.row_id];
        await saveMachineStateIds(machineId, stateIds);
        if (!cellRowByKey('visual_state_machine', machine, 'default_state')) {
          await setDefaultVisualState(machineId, created.row_id, false);
        }
        state.visual.state = stateKey;
        state.visual.started = performance.now();
        log(`added visual state ${stateKey}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function setVisualStateAnimation(stateId, animationId) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 9, row_id: stateId, field_id: 72, value: String(animationId) })
      });
      state.visual.started = performance.now();
      await loadProject(false);
    }

    async function setDefaultVisualState(machineId, stateId, reload = true) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 8, row_id: machineId, field_id: 61, value: String(stateId) })
      });
      if (reload) await loadProject(false);
    }

    async function deleteVisualState(machineId, stateId) {
      const machine = rowByKey('visual_state_machine', machineId);
      const stateRow = rowByKey('visual_state', stateId);
      if (!machine || !stateRow || !confirm(`Delete visual state ${stateRow.key}?`)) return;
      const stateIds = cellRowsByKey('visual_state_machine', machine, 'states').filter(id => id !== stateId);
      if (stateIds.length === 0) return;
      try {
        await saveMachineStateIds(machineId, stateIds);
        if (cellRowByKey('visual_state_machine', machine, 'default_state') === stateId) {
          await setDefaultVisualState(machineId, stateIds[0], false);
        }
        await api('/api/row/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: 9, row_id: stateId })
        });
        const next = rowByKey('visual_state', stateIds[0]);
        state.visual.state = next ? cellStringByKey('visual_state', next, 'state_key', next.key) : 'idle';
        state.visual.started = performance.now();
        log(`deleted visual state ${stateRow.key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function renderSpriteSlicer() {
      const textures = tableDataByKey('texture_asset').rows;
      const options = textures.map(row => {
        const label = `${cellStringByKey('texture_asset', row, 'name', row.key)} (${cellStringByKey('texture_asset', row, 'path', '')})`;
        return `<option value="${row.id}">${escapeHtml(label)}</option>`;
      }).join('');
      return `
        <div class="slicer-panel">
          <div class="slicer-head">
            <span>Sprite Sheet Slicer</span>
            <span>${tableDataByKey('sprite_frame').rows.length} frames</span>
          </div>
          <div class="slicer-body">
            <canvas id="sliceCanvas" class="slicer-canvas"></canvas>
            <div class="slicer-form">
              <label class="wide">Texture
                <select id="sliceTexture" onchange="updateSlicePreview()">${options}</select>
              </label>
              <label class="wide">Frame Key Prefix
                <input id="slicePrefix" value="slice" oninput="updateSlicePreview()">
              </label>
              <label>Start X<input id="sliceStartX" type="number" value="0" oninput="updateSlicePreview()"></label>
              <label>Start Y<input id="sliceStartY" type="number" value="0" oninput="updateSlicePreview()"></label>
              <label>Frame W<input id="sliceFrameW" type="number" min="1" value="32" oninput="updateSlicePreview()"></label>
              <label>Frame H<input id="sliceFrameH" type="number" min="1" value="32" oninput="updateSlicePreview()"></label>
              <label>Columns<input id="sliceColumns" type="number" min="1" value="4" oninput="updateSlicePreview()"></label>
              <label>Rows<input id="sliceRows" type="number" min="1" value="1" oninput="updateSlicePreview()"></label>
              <label>Gap X<input id="sliceGapX" type="number" value="0" oninput="updateSlicePreview()"></label>
              <label>Gap Y<input id="sliceGapY" type="number" value="0" oninput="updateSlicePreview()"></label>
              <label>Pivot X<input id="slicePivotX" type="number" step="0.05" value="0.5"></label>
              <label>Pivot Y<input id="slicePivotY" type="number" step="0.05" value="0.85"></label>
              <label class="wide">Duration
                <input id="sliceDuration" type="number" step="0.01" value="0.1">
              </label>
              <button class="wide primary" onclick="createSpriteFramesFromSlice()">Create Frames</button>
            </div>
          </div>
        </div>`;
    }

    function renderAnimationEditor(visualRow) {
      const animation = selectedVisualAnimation(visualRow);
      if (!animation) {
        return `
          <div class="animation-panel">
            <div class="animation-head">
              <span>Animation Frames</span>
              <span>no animation for state ${escapeHtml(state.visual.state)}</span>
            </div>
          </div>`;
      }
      const frames = animationFrames(animation);
      const textureId = cellRowByKey('sprite_animation', animation, 'texture');
      const texture = textureId ? rowByKey('texture_asset', textureId) : null;
      const available = tableDataByKey('sprite_frame').rows
        .filter(frame => !textureId || cellRowByKey('sprite_frame', frame, 'texture') === textureId);
      return `
        <div class="animation-panel">
          <div class="animation-head">
            <span>Animation Frames</span>
            <span>${escapeHtml(cellStringByKey('sprite_animation', animation, 'name', animation.key))} / ${frames.length} frames</span>
          </div>
          <div class="animation-body">
            <div>
              <div class="animation-controls">
                <label>Texture
                  <select onchange="setAnimationTexture(${animation.id}, this.value)">
                    ${tableDataByKey('texture_asset').rows.map(row => `<option value="${row.id}" ${row.id === textureId ? 'selected' : ''}>${escapeHtml(cellStringByKey('texture_asset', row, 'name', row.key))}</option>`).join('')}
                  </select>
                </label>
                <label>FPS
                  <input type="number" step="0.1" value="${escapeAttr(cellNumberByKey('sprite_animation', animation, 'fps', 6))}" onchange="setAnimationField(${animation.id}, 53, this.value)">
                </label>
                <label>Looping
                  <select onchange="setAnimationField(${animation.id}, 54, this.value)">
                    <option value="true" ${cellBoolByKey('sprite_animation', animation, 'looping', true) ? 'selected' : ''}>true</option>
                    <option value="false" ${cellBoolByKey('sprite_animation', animation, 'looping', true) ? '' : 'selected'}>false</option>
                  </select>
                </label>
              </div>
              <div class="animation-list">
                ${frames.map((frame, index) => renderAnimationFrameRow(animation, frame, index, frames.length)).join('') || `<div class="animation-row"><span></span><span>No frames</span><span></span></div>`}
              </div>
            </div>
            <div>
              <div class="animation-controls">
                <label>Available Frames
                  <input readonly value="${escapeAttr(texture ? cellStringByKey('texture_asset', texture, 'name', texture.key) : 'all textures')}">
                </label>
              </div>
              <div class="frame-palette">
                ${available.map(frame => `
                  <button onclick="addAnimationFrame(${animation.id}, ${frame.id})">
                    <span>#${frame.id}</span>
                    <span>${escapeHtml(rowTitle(11, frame.id))}<br><small>${cellNumberByKey('sprite_frame', frame, 'x', 0)}, ${cellNumberByKey('sprite_frame', frame, 'y', 0)} / ${cellNumberByKey('sprite_frame', frame, 'w', 0)}x${cellNumberByKey('sprite_frame', frame, 'h', 0)}</small></span>
                  </button>`).join('') || `<button disabled><span></span><span>No sprite frames</span></button>`}
              </div>
            </div>
          </div>
        </div>`;
    }

    function renderAnimationFrameRow(animation, frame, index, total) {
      return `
        <div class="animation-row">
          <span>${index + 1}</span>
          <span>${escapeHtml(rowTitle(11, frame.id))}<br><small>${cellNumberByKey('sprite_frame', frame, 'x', 0)}, ${cellNumberByKey('sprite_frame', frame, 'y', 0)} / ${cellNumberByKey('sprite_frame', frame, 'w', 0)}x${cellNumberByKey('sprite_frame', frame, 'h', 0)}</small></span>
          <span class="animation-row-actions">
            <button ${index === 0 ? 'disabled' : ''} onclick="moveAnimationFrame(${animation.id}, ${index}, -1)">Up</button>
            <button ${index === total - 1 ? 'disabled' : ''} onclick="moveAnimationFrame(${animation.id}, ${index}, 1)">Dn</button>
            <button class="danger" onclick="removeAnimationFrame(${animation.id}, ${index})">Del</button>
          </span>
        </div>`;
    }

    function animationFrameIds(animation) {
      return cellRowsByKey('sprite_animation', animation, 'frames');
    }

    async function saveAnimationFrameIds(animationId, frameIds) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 7, row_id: animationId, field_id: 55, value: frameIds.join(',') })
      });
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 7, row_id: animationId, field_id: 52, value: String(frameIds.length) })
      });
      state.visual.started = performance.now();
      await loadProject(false);
    }

    async function addAnimationFrame(animationId, frameId) {
      const animation = rowByKey('sprite_animation', animationId);
      if (!animation) return;
      await saveAnimationFrameIds(animationId, [...animationFrameIds(animation), frameId]);
    }

    async function removeAnimationFrame(animationId, index) {
      const animation = rowByKey('sprite_animation', animationId);
      if (!animation) return;
      const frameIds = animationFrameIds(animation).filter((_, frameIndex) => frameIndex !== index);
      await saveAnimationFrameIds(animationId, frameIds);
    }

    async function moveAnimationFrame(animationId, index, direction) {
      const animation = rowByKey('sprite_animation', animationId);
      if (!animation) return;
      const frameIds = [...animationFrameIds(animation)];
      const next = index + direction;
      if (next < 0 || next >= frameIds.length) return;
      [frameIds[index], frameIds[next]] = [frameIds[next], frameIds[index]];
      await saveAnimationFrameIds(animationId, frameIds);
    }

    async function setAnimationField(animationId, fieldId, value) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: 7, row_id: animationId, field_id: fieldId, value: String(value) })
      });
      state.visual.started = performance.now();
      await loadProject(false);
    }

    async function setAnimationTexture(animationId, textureId) {
      await setAnimationField(animationId, 51, textureId);
    }

    function selectedSliceTexture() {
      const id = Number($('sliceTexture')?.value || 0);
      return id ? rowByKey('texture_asset', id) : null;
    }

    function sliceNumber(id, fallback) {
      const value = Number($(id)?.value);
      return Number.isFinite(value) ? value : fallback;
    }

    function updateSlicePreview() {
      if (state.mode !== 'visual') return;
      const canvas = $('sliceCanvas');
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.floor(rect.width * dpr);
      canvas.height = Math.floor(rect.height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = '#202832';
      ctx.fillRect(0, 0, rect.width, rect.height);

      const texture = selectedSliceTexture();
      const image = textureImage(texture);
      if (!texture || !image || !image.complete || image.naturalWidth === 0) {
        ctx.fillStyle = '#dbe7ef';
        ctx.font = '13px Segoe UI';
        ctx.fillText('select a loaded texture asset', 14, 24);
        if (image) image.onload = updateSlicePreview;
        return;
      }

      const fit = Math.min(rect.width / image.naturalWidth, rect.height / image.naturalHeight);
      const dw = image.naturalWidth * fit;
      const dh = image.naturalHeight * fit;
      const ox = (rect.width - dw) / 2;
      const oy = (rect.height - dh) / 2;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(image, ox, oy, dw, dh);

      const startX = sliceNumber('sliceStartX', 0);
      const startY = sliceNumber('sliceStartY', 0);
      const frameW = Math.max(1, sliceNumber('sliceFrameW', 32));
      const frameH = Math.max(1, sliceNumber('sliceFrameH', 32));
      const columns = Math.max(1, sliceNumber('sliceColumns', 1));
      const rows = Math.max(1, sliceNumber('sliceRows', 1));
      const gapX = sliceNumber('sliceGapX', 0);
      const gapY = sliceNumber('sliceGapY', 0);

      ctx.strokeStyle = '#ffdd57';
      ctx.lineWidth = 2;
      ctx.fillStyle = 'rgba(255,221,87,0.14)';
      for (let row = 0; row < rows; row++) {
        for (let column = 0; column < columns; column++) {
          const x = ox + (startX + column * (frameW + gapX)) * fit;
          const y = oy + (startY + row * (frameH + gapY)) * fit;
          const w = frameW * fit;
          const h = frameH * fit;
          ctx.fillRect(x, y, w, h);
          ctx.strokeRect(x, y, w, h);
        }
      }
    }

    async function createSpriteFramesFromSlice() {
      const texture = selectedSliceTexture();
      if (!texture) return;
      const payload = {
        texture_id: texture.id,
        prefix: $('slicePrefix').value.trim(),
        start_x: Math.trunc(sliceNumber('sliceStartX', 0)),
        start_y: Math.trunc(sliceNumber('sliceStartY', 0)),
        frame_w: Math.trunc(sliceNumber('sliceFrameW', 32)),
        frame_h: Math.trunc(sliceNumber('sliceFrameH', 32)),
        columns: Math.trunc(sliceNumber('sliceColumns', 1)),
        rows: Math.trunc(sliceNumber('sliceRows', 1)),
        gap_x: Math.trunc(sliceNumber('sliceGapX', 0)),
        gap_y: Math.trunc(sliceNumber('sliceGapY', 0)),
        pivot_x: sliceNumber('slicePivotX', 0.5),
        pivot_y: sliceNumber('slicePivotY', 0.85),
        duration: sliceNumber('sliceDuration', 0.1)
      };
      try {
        const result = await api('/api/visual/slice-grid', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        log(`created ${result.created} sprite frames`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function visualStateRows(visualRow) {
      const machine = visualStateMachine(visualRow);
      return cellRowsByKey('visual_state_machine', machine, 'states')
        .map(id => rowByKey('visual_state', id))
        .filter(Boolean);
    }

    function renderVisualStateButtons(visualRow) {
      const states = visualStateRows(visualRow);
      const active = states.some(row => cellStringByKey('visual_state', row, 'state_key') === state.visual.state)
        ? state.visual.state
        : (states[0] ? cellStringByKey('visual_state', states[0], 'state_key') : 'idle');
      state.visual.state = active;
      $('visualStates').innerHTML = states.map(row => {
        const key = cellStringByKey('visual_state', row, 'state_key', row.key);
        return `<button class="${key === active ? 'primary' : ''}" onclick="selectVisualState('${key}')">${key}</button>`;
      }).join('');
    }

    function selectedVisualRow() {
      return tableDataByKey('unit_visual').rows.find(row => row.key === state.visual.key)
        || tableDataByKey('unit_visual').rows[0];
    }

    function selectedVisualAnimation(visualRow) {
      const stateRow = visualStateRows(visualRow)
        .find(row => cellStringByKey('visual_state', row, 'state_key') === state.visual.state);
      const animationId = stateRow ? cellRowByKey('visual_state', stateRow, 'animation') : null;
      return animationId ? rowByKey('sprite_animation', animationId) : null;
    }

    function animationFrames(animation) {
      return cellRowsByKey('sprite_animation', animation, 'frames')
        .map(id => rowByKey('sprite_frame', id))
        .filter(Boolean);
    }

    function frameTexture(frame) {
      const textureId = cellRowByKey('sprite_frame', frame, 'texture');
      return textureId ? rowByKey('texture_asset', textureId) : null;
    }

    function textureImage(texture) {
      const path = cellStringByKey('texture_asset', texture, 'path', '');
      if (!path) return null;
      if (!state.images[path]) {
        const img = new Image();
        img.src = `/asset?path=${encodeURIComponent(path)}`;
        state.images[path] = img;
      }
      return state.images[path];
    }

    function drawVisualPreview() {
      if (state.mode !== 'visual') return;
      const canvas = $('visualCanvas');
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.floor(rect.width * dpr);
      canvas.height = Math.floor(rect.height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const visual = selectedVisualRow();
      const animation = selectedVisualAnimation(visual);
      const frames = animationFrames(animation);
      const fps = animation ? cellNumberByKey('sprite_animation', animation, 'fps', 6) : 6;
      const frameCount = Math.max(1, frames.length || (animation ? cellNumberByKey('sprite_animation', animation, 'frame_count', 4) : 4));
      const t = (performance.now() - state.visual.started) / 1000;
      const frame = Math.floor(t * fps) % frameCount;
      ctx.clearRect(0, 0, rect.width, rect.height);
      drawVisualBackground(ctx, rect.width, rect.height);
      drawPreviewSprite(ctx, rect.width / 2, rect.height * 0.62, visual, frames[frame], frame, frameCount);
      ctx.fillStyle = '#dbe7ef';
      ctx.font = '13px Segoe UI';
      ctx.textAlign = 'left';
      ctx.fillText(`state: ${state.visual.state}`, 16, 24);
      ctx.fillText(`frame: ${frame + 1}/${frameCount} @ ${fps}fps`, 16, 44);
      requestAnimationFrame(drawVisualPreview);
    }

    function drawVisualBackground(ctx, w, h) {
      ctx.fillStyle = '#202832';
      ctx.fillRect(0, 0, w, h);
      ctx.fillStyle = '#303d30';
      ctx.fillRect(0, h * 0.58, w, h * 0.42);
      ctx.strokeStyle = 'rgba(255,255,255,0.08)';
      for (let i = 0; i < 8; i++) {
        const y = h * 0.62 + i * 22;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }
    }

    function drawPreviewSprite(ctx, x, y, visual, spriteFrame, frame, frameCount) {
      const scale = cellNumberByKey('unit_visual', visual, 'scale', 1);
      const color = cellStringByKey('unit_visual', visual, 'body_color', '#999999');
      const shadow = cellNumberByKey('unit_visual', visual, 'shadow_radius', 18) * scale;
      const bob = Math.sin((frame / frameCount) * Math.PI * 2) * 5;
      ctx.save();
      ctx.translate(x, y + bob);
      ctx.fillStyle = 'rgba(0,0,0,0.34)';
      ctx.beginPath();
      ctx.ellipse(0, 30 * scale, shadow, shadow * 0.36, 0, 0, Math.PI * 2);
      ctx.fill();
      if (spriteFrame && drawSpriteFrame(ctx, spriteFrame, scale)) {
        ctx.restore();
        return;
      }
      drawPlaceholderSprite(ctx, color, scale);
      ctx.restore();
    }

    function drawSpriteFrame(ctx, frame, scale) {
      const texture = frameTexture(frame);
      const image = textureImage(texture);
      if (!image || !image.complete || image.naturalWidth === 0) return false;
      const sx = cellNumberByKey('sprite_frame', frame, 'x', 0);
      const sy = cellNumberByKey('sprite_frame', frame, 'y', 0);
      const sw = cellNumberByKey('sprite_frame', frame, 'w', 64);
      const sh = cellNumberByKey('sprite_frame', frame, 'h', 64);
      const px = cellNumberByKey('sprite_frame', frame, 'pivot_x', 0.5);
      const py = cellNumberByKey('sprite_frame', frame, 'pivot_y', 0.85);
      const dw = sw * scale;
      const dh = sh * scale;
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(image, sx, sy, sw, sh, -dw * px, -dh * py, dw, dh);
      return true;
    }

    function drawPlaceholderSprite(ctx, color, scale) {
      ctx.fillStyle = color;
      ctx.strokeStyle = '#eef6ff';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.roundRect(-26 * scale, -56 * scale, 52 * scale, 78 * scale, 12 * scale);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = '#111820';
      ctx.beginPath();
      ctx.arc(-9 * scale, -28 * scale, 3 * scale, 0, Math.PI * 2);
      ctx.arc(9 * scale, -28 * scale, 3 * scale, 0, Math.PI * 2);
      ctx.fill();
    }

    function relationRowButton(tableId, row, action, handler) {
      return `<button class="relation-row" onclick="${handler}">
        <span>#${row.id}</span>
        <span>${escapeHtml(rowTitle(tableId, row.id))}</span>
        <span>${action}</span>
      </button>`;
    }

    function selectTable(key) {
      if (state.mode === 'visual') {
        renderVisualDashboard();
        return;
      }
      state.selected = { type: 'table', key };
      renderNav();
      const table = state.project.tables.find(table => table.key === key);
      if (state.mode === 'schema') renderSchemaTable(table);
      else if (state.mode === 'visual') renderTable(table);
      else renderTable(table);
    }

    async function selectView(key) {
      state.selected = { type: 'view', key };
      renderNav();
      await renderView(key);
    }

    async function updateCell(tableId, rowId, fieldId, value) {
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: tableId, row_id: rowId, field_id: fieldId, value })
      });
      log(`saved cell table=${tableId} row=${rowId} field=${fieldId}`);
      await loadProject(false);
    }

    async function addRow(tableId) {
      const key = prompt('row key');
      if (!key) return;
      try {
        await api('/api/row', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: tableId, key })
        });
        log(`added row ${key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function deleteRow(tableId, rowId) {
      const row = rowById(tableId, rowId);
      if (!row || !confirm(`Delete row ${row.key}?`)) return;
      try {
        await api('/api/row/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: tableId, row_id: rowId })
        });
        log(`deleted row ${row.key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function openRelationPicker(tableId, rowId, fieldId) {
      state.backStack.push({ mode: state.mode, selected: state.selected });
      state.selected = { type: 'relation', tableId, rowId, fieldId };
      renderRelationPicker(state.selected);
    }

    async function setRelationValue(tableId, rowId, fieldId, targetRowId, selected) {
      const table = tableById(tableId);
      const row = rowById(tableId, rowId);
      const field = table.fields.find(field => field.id === fieldId);
      const cell = fieldCell(row, fieldId);
      let value = '';
      if (kindKey(field.kind) === 'relation_one') {
        value = selected ? String(targetRowId) : '';
      } else {
        const values = new Set(cell?.kind === 'rows' ? cell.value : []);
        if (selected) values.add(targetRowId);
        else values.delete(targetRowId);
        value = [...values].join(',');
      }
      await api('/api/cell', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ table_id: tableId, row_id: rowId, field_id: fieldId, value })
      });
      await loadProject(false);
    }

    async function addNestedRow(tableId, rowId, fieldId) {
      const sourceTable = tableById(tableId);
      const field = sourceTable.fields.find(field => field.id === fieldId);
      const targetTableId = relationTarget(field.kind);
      const key = prompt('nested row key');
      if (!key) return;
      try {
        const created = await api('/api/row', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: targetTableId, key })
        });
        await setRelationValue(tableId, rowId, fieldId, created.row_id, true);
        log(`added nested row ${key}`);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function goBack() {
      const previous = state.backStack.pop();
      if (!previous) return;
      state.mode = previous.mode;
      state.selected = previous.selected;
      renderNav();
      if (state.selected?.type === 'table') {
        const table = state.project.tables.find(table => table.key === state.selected.key);
        if (state.mode === 'schema') renderSchemaTable(table);
        else renderTable(table);
      }
    }

    function syncFieldTarget() {
      const kind = $('fieldKind');
      const target = $('fieldTarget');
      const nestedKey = $('nestedKey');
      const fieldKey = $('fieldKey');
      if (!kind || !target) return;
      const needsTarget = ['relation_one', 'relation_many', 'reference_group'].includes(kind.value);
      const needsNested = kind.value === 'owned_nested_table';
      target.disabled = !needsTarget;
      target.style.display = needsTarget ? '' : 'none';
      if (nestedKey) {
        nestedKey.disabled = !needsNested;
        nestedKey.style.display = needsNested ? '' : 'none';
        if (needsNested && fieldKey && !nestedKey.value.trim()) {
          const table = state.selected?.type === 'table'
            ? state.project.tables.find(table => table.key === state.selected.key)
            : null;
          nestedKey.value = `${table?.key || 'nested'}_${fieldKey.value.trim() || 'items'}`;
        }
      }
    }

    async function addTable() {
      const key = prompt('table key');
      if (!key) return;
      const displayName = prompt('display name', key.replaceAll('_', ' ')) || key;
      try {
        await api('/api/schema/table', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ key, display_name: displayName })
        });
        log(`added table ${key}`);
        await loadProject(false);
        selectTable(key);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function deleteTable(tableId) {
      const table = state.project.tables.find(table => table.id === tableId);
      if (!table || !confirm(`Delete table ${table.display_name}?`)) return;
      try {
        await api('/api/schema/table/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: tableId })
        });
        log(`deleted table ${table.key}`);
        state.selected = null;
        await loadProject(true);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function addFieldFromForm(tableId) {
      const key = $('fieldKey').value.trim();
      const kind = $('fieldKind').value;
      const targetKinds = ['relation_one', 'relation_many', 'reference_group'];
      const payload = {
        table_id: tableId,
        key,
        kind,
        required: $('fieldRequired').checked
      };
      if (targetKinds.includes(kind)) payload.target_table = Number($('fieldTarget').value);
      if (kind === 'owned_nested_table') payload.nested_key = $('nestedKey').value.trim();
      try {
        await api('/api/schema/field', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload)
        });
        log(`added field ${key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function deleteField(tableId, fieldId) {
      const table = state.project.tables.find(table => table.id === tableId);
      const field = table?.fields.find(field => field.id === fieldId);
      if (!field || !confirm(`Remove field ${field.display_name}?`)) return;
      try {
        await api('/api/schema/field/delete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ table_id: tableId, field_id: fieldId })
        });
        log(`removed field ${field.key}`);
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    async function loadProject(selectFirst = true) {
      const data = await api('/api/project');
      const assets = await api('/api/assets');
      state.project = data.project;
      state.assets = assets.assets || [];
      $('projectPath').textContent = data.project_path;
      renderStatus(data.status);
      renderNav();
      if (state.mode === 'visual') renderVisualDashboard();
      else if (state.mode === 'operation') await renderOperationDashboard();
      else if (selectFirst && state.project.tables.length) selectTable(state.project.tables[0].key);
      else if (state.selected?.type === 'relation') renderRelationPicker(state.selected);
      else if (state.selected?.type === 'table') {
        const table = state.project.tables.find(table => table.key === state.selected.key) || state.project.tables[0];
        if (table) {
          state.selected = { type: 'table', key: table.key };
          if (state.mode === 'schema') renderSchemaTable(table);
          else renderTable(table);
        }
      }
      else if (state.selected?.type === 'view' && state.mode === 'data') await renderView(state.selected.key);
    }

    function setMode(mode) {
      state.mode = mode;
      state.selected = null;
      renderNav();
      if (mode === 'visual') {
        renderVisualDashboard();
        return;
      }
      if (mode === 'operation') {
        renderOperationDashboard().catch(error => log(`error: ${error.message}`));
        return;
      }
      if (state.project?.tables?.length) selectTable(state.project.tables[0].key);
    }

    async function command(path, label, body) {
      try {
        log(`> ${label}`);
        const data = await api(path, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: body ? JSON.stringify(body) : '{}'
        });
        log(data.output || data.message || JSON.stringify(data.issues || data, null, 2));
        await loadProject(false);
      } catch (error) {
        log(`error: ${error.message}`);
      }
    }

    function log(text) {
      $('output').textContent = `${text}\n\n${$('output').textContent}`;
    }

    function escapeAttr(value) {
      return String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
    }

    function escapeHtml(value) {
      return String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
    }

    $('validateBtn').onclick = () => command('/api/validate', 'validate');
    $('codegenBtn').onclick = () => command('/api/codegen', 'codegen');
    $('buildBtn').onclick = () => command('/api/data-build', 'data-build');
    $('simulateBtn').onclick = () => command('/api/simulate', 'simulate', { map_key: 'endless_left_road' });
    $('schemaTab').onclick = () => setMode('schema');
    $('dataTab').onclick = () => setMode('data');
    $('visualTab').onclick = () => setMode('visual');
    $('operationTab').onclick = () => setMode('operation');
    $('addTableBtn').onclick = addTable;
    loadProject().catch(error => log(`error: ${error.message}`));
  </script>
</body>
</html>
"#;
