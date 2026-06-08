use data_studio_core::{
    CellValue, DataProject, FieldId, FieldKind, FieldSchema, ProjectFingerprints, RowId, TableData,
    TableId, TableSchema,
};
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

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
        ("GET", "/api/project") => api_project(state),
        ("GET", "/api/status") => api_status(state),
        ("GET", "/api/view") => api_view(request, state),
        ("POST", "/api/cell") => api_update_cell(request, state),
        ("POST", "/api/schema/table") => api_add_table(request, state),
        ("POST", "/api/schema/table/delete") => api_delete_table(request, state),
        ("POST", "/api/schema/field") => api_add_field(request, state),
        ("POST", "/api/schema/field/delete") => api_delete_field(request, state),
        ("POST", "/api/validate") => api_validate(state),
        ("POST", "/api/codegen") => api_codegen(state),
        ("POST", "/api/data-build") => api_data_build(state),
        ("POST", "/api/simulate") => api_simulate(request, state),
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
    let before = project.tables.len();
    project.tables.retain(|table| table.id != table_id);
    if project.tables.len() == before {
        return Err(("unknown table".to_string(), 404));
    }
    project.data.retain(|table| table.table_id != table_id);
    project.views.retain(|view| view.source_table != table_id);
    for table in &mut project.tables {
        table
            .fields
            .retain(|field| !field_targets_table(field, table_id));
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
    let display_name = payload
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(key);
    let kind = string_value(&payload, "kind")?;
    let required = payload
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target_table = payload.get("target_table").and_then(Value::as_u64);

    let mut project = load_project(&state.project_path)?;
    let field_id = next_field_id(&project);
    let field_kind = parse_field_kind(kind, target_table)?;
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
        display_name: display_name.to_string(),
        kind: field_kind,
        required,
    });
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
    let before = table.fields.len();
    table.fields.retain(|field| field.id != field_id);
    if table.fields.len() == before {
        return Err(("unknown field".to_string(), 404));
    }
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

fn load_project(path: &Path) -> Result<DataProject, (String, u16)> {
    DataProject::load_from_dir(path).map_err(|error| (error.to_string(), 500))
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
        return Ok(CellValue::Empty);
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
        FieldKind::RelationOne { .. } | FieldKind::OwnedNestedTable { .. } => raw
            .parse::<u64>()
            .map(|id| CellValue::Row(RowId(id)))
            .map_err(|_| ("expected row id".to_string(), 400)),
        FieldKind::RelationMany { .. } | FieldKind::ReferenceGroup { .. } => raw
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

fn field_targets_table(field: &FieldSchema, table_id: TableId) -> bool {
    match field.kind {
        FieldKind::RelationOne { target_table }
        | FieldKind::RelationMany { target_table }
        | FieldKind::ReferenceGroup { target_table } => target_table == table_id,
        FieldKind::OwnedNestedTable { nested_table } => nested_table == table_id,
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
      grid-template-columns: minmax(110px, 1fr) minmax(130px, 1fr) 170px 170px 90px 110px;
      gap: 8px;
      margin-bottom: 10px;
      align-items: center;
      max-width: 980px;
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
    let state = { project: null, mode: 'schema', selected: null };
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

    function kindKey(kind) {
      return typeof kind === 'string' ? kind : kind.kind;
    }

    function kindLabel(kind) {
      const key = kindKey(kind);
      const target = kind.target_table ?? kind.nested_table;
      return target ? `${key} -> #${target}` : key;
    }

    function fieldCell(row, fieldId) {
      return row.cells[String(fieldId)] || row.cells[fieldId] || { kind: 'empty' };
    }

    function renderNav() {
      $('tables').innerHTML = state.project.tables.map(table => `
        <button class="nav-item ${state.selected?.type === 'table' && state.selected.key === table.key ? 'active' : ''}"
          onclick="selectTable('${table.key}')">
          <span>${table.display_name}</span><span>${table.key}</span>
        </button>`).join('');
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
      $('sheetTools').innerHTML = '';
      const headers = [`<th class="key">key</th>`, ...table.fields.map(field => `<th>${field.display_name}<br><small>${field.key}</small></th>`)].join('');
      const rows = data.rows.map(row => {
        const cells = table.fields.map(field => {
          const value = cellText(fieldCell(row, field.id));
          return `<td><input class="cell-input" value="${escapeAttr(value)}"
            onchange="updateCell(${table.id}, ${row.id}, ${field.id}, this.value)"></td>`;
        }).join('');
        return `<tr><td class="key">${row.key}<br><small>#${row.id}</small></td>${cells}</tr>`;
      }).join('');
      $('grid').innerHTML = `<table><thead><tr>${headers}</tr></thead><tbody>${rows}</tbody></table>`;
    }

    function renderSchemaTable(table) {
      $('sheetTitle').textContent = table.display_name;
      $('sheetMeta').textContent = `${table.key} / ${table.fields.length} fields`;
      $('sheetTools').innerHTML = `
        <button class="danger" onclick="deleteTable(${table.id})">Delete Table</button>`;
      const targetOptions = state.project.tables
        .map(target => `<option value="${target.id}">${target.display_name} (${target.key})</option>`)
        .join('');
      const form = `
        <div class="schema-form">
          <input id="fieldKey" placeholder="field_key">
          <input id="fieldName" placeholder="Field Name">
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

    function selectTable(key) {
      state.selected = { type: 'table', key };
      renderNav();
      const table = state.project.tables.find(table => table.key === key);
      if (state.mode === 'schema') renderSchemaTable(table);
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

    function syncFieldTarget() {
      const kind = $('fieldKind');
      const target = $('fieldTarget');
      if (!kind || !target) return;
      const needsTarget = ['relation_one', 'relation_many', 'reference_group', 'owned_nested_table'].includes(kind.value);
      target.disabled = !needsTarget;
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
      const displayName = $('fieldName').value.trim() || key;
      const kind = $('fieldKind').value;
      const targetKinds = ['relation_one', 'relation_many', 'reference_group', 'owned_nested_table'];
      const payload = {
        table_id: tableId,
        key,
        display_name: displayName,
        kind,
        required: $('fieldRequired').checked
      };
      if (targetKinds.includes(kind)) payload.target_table = Number($('fieldTarget').value);
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
      state.project = data.project;
      $('projectPath').textContent = data.project_path;
      renderStatus(data.status);
      renderNav();
      if (selectFirst && state.project.tables.length) selectTable(state.project.tables[0].key);
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

    $('validateBtn').onclick = () => command('/api/validate', 'validate');
    $('codegenBtn').onclick = () => command('/api/codegen', 'codegen');
    $('buildBtn').onclick = () => command('/api/data-build', 'data-build');
    $('simulateBtn').onclick = () => command('/api/simulate', 'simulate', { map_key: 'endless_left_road' });
    $('schemaTab').onclick = () => setMode('schema');
    $('dataTab').onclick = () => setMode('data');
    $('addTableBtn').onclick = addTable;
    loadProject().catch(error => log(`error: ${error.message}`));
  </script>
</body>
</html>
"#;
