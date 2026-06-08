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
    let state = { project: null, mode: 'schema', selected: null, backStack: [], visual: { key: null, state: 'idle', started: 0 }, images: {} };
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
      $('sheetTools').innerHTML = `<button onclick="selectTable('unit_visual')">Open Table</button>`;
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
        </div>`;
      renderVisualStateButtons(selected);
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

    function visualStateRows(visualRow) {
      const machineId = cellRowByKey('unit_visual', visualRow, 'state_machine');
      const machine = machineId ? rowByKey('visual_state_machine', machineId) : null;
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
      state.project = data.project;
      $('projectPath').textContent = data.project_path;
      renderStatus(data.status);
      renderNav();
      if (state.mode === 'visual') renderVisualDashboard();
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
    $('addTableBtn').onclick = addTable;
    loadProject().catch(error => log(`error: ${error.message}`));
  </script>
</body>
</html>
"#;
