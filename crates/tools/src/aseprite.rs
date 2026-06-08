use data_studio_core::{CellValue, DataProject, FieldId, RowData, RowId, TableId};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const TEXTURE_ASSET_TABLE: TableId = TableId(6);
const SPRITE_ANIMATION_TABLE: TableId = TableId(7);
const SPRITE_FRAME_TABLE: TableId = TableId(11);

#[derive(Debug, Clone)]
pub(crate) struct ImportSummary {
    pub texture_key: String,
    pub frame_count: usize,
    pub animation_count: usize,
}

#[derive(Debug, Clone)]
struct AseFrame {
    name: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    duration_ms: i32,
}

#[derive(Debug, Clone)]
struct AseTag {
    name: String,
    from: usize,
    to: usize,
    direction: String,
}

pub(crate) fn import_aseprite(
    project_path: &Path,
    input_file: &Path,
) -> Result<ImportSummary, String> {
    let json_file = prepare_aseprite_json(project_path, input_file)?;
    let json_text = fs::read_to_string(&json_file)
        .map_err(|error| format!("failed to read {}: {error}", json_file.display()))?;
    let json: Value = serde_json::from_str(json_text.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("failed to parse {}: {error}", json_file.display()))?;
    let (frames, tags, image_path, sheet_w, sheet_h) = parse_aseprite_json(&json, &json_file)?;

    let mut project =
        DataProject::load_from_dir(project_path).map_err(|error| error.to_string())?;
    let import_key = sanitize_key(
        input_file
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("aseprite"),
    );
    let texture_key = unique_row_key(&project, TEXTURE_ASSET_TABLE, &import_key);
    let asset_path = copy_texture(project_path, &image_path, &texture_key)?;
    let texture_id = next_row_id(&project);

    push_row(
        &mut project,
        TEXTURE_ASSET_TABLE,
        RowData {
            id: texture_id,
            key: texture_key.clone(),
            cells: cells([
                (44, CellValue::String(display_name(&texture_key))),
                (45, CellValue::String(asset_path)),
                (46, CellValue::I32(sheet_w)),
                (47, CellValue::I32(sheet_h)),
            ]),
        },
    )?;

    let mut frame_ids = Vec::new();
    for frame in &frames {
        let row_id = next_row_id(&project);
        frame_ids.push(row_id);
        let key = unique_row_key(
            &project,
            SPRITE_FRAME_TABLE,
            &format!("{}_{}", texture_key, sanitize_key(&frame.name)),
        );
        push_row(
            &mut project,
            SPRITE_FRAME_TABLE,
            RowData {
                id: row_id,
                key,
                cells: cells([
                    (90, CellValue::String(frame.name.clone())),
                    (91, CellValue::Row(texture_id)),
                    (92, CellValue::I32(frame.x)),
                    (93, CellValue::I32(frame.y)),
                    (94, CellValue::I32(frame.w)),
                    (95, CellValue::I32(frame.h)),
                    (96, CellValue::F32(0.5)),
                    (97, CellValue::F32(0.85)),
                    (98, CellValue::F32(frame.duration_ms as f32 / 1000.0)),
                ]),
            },
        )?;
    }

    let animation_specs = if tags.is_empty() {
        vec![AseTag {
            name: texture_key.clone(),
            from: 0,
            to: frame_ids.len().saturating_sub(1),
            direction: "forward".to_string(),
        }]
    } else {
        tags
    };

    let mut animation_count = 0;
    for tag in animation_specs {
        let selected = tag_frame_ids(&tag, &frame_ids);
        if selected.is_empty() {
            continue;
        }
        let row_id = next_row_id(&project);
        let key = unique_row_key(
            &project,
            SPRITE_ANIMATION_TABLE,
            &format!("{}_{}", texture_key, sanitize_key(&tag.name)),
        );
        let average_duration = average_duration_seconds(&tag, &frames);
        let fps = if average_duration > 0.0 {
            1.0 / average_duration
        } else {
            12.0
        };
        push_row(
            &mut project,
            SPRITE_ANIMATION_TABLE,
            RowData {
                id: row_id,
                key,
                cells: cells([
                    (50, CellValue::String(tag.name.clone())),
                    (51, CellValue::Row(texture_id)),
                    (52, CellValue::I32(selected.len() as i32)),
                    (53, CellValue::F32(fps)),
                    (54, CellValue::Bool(true)),
                    (55, CellValue::Rows(selected)),
                ]),
            },
        )?;
        animation_count += 1;
    }

    project
        .save_to_dir(project_path, &project_name(project_path))
        .map_err(|error| error.to_string())?;

    Ok(ImportSummary {
        texture_key,
        frame_count: frame_ids.len(),
        animation_count,
    })
}

fn prepare_aseprite_json(project_path: &Path, input_file: &Path) -> Result<PathBuf, String> {
    let extension = input_file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "json" {
        return Ok(input_file.to_path_buf());
    }
    if extension != "aseprite" && extension != "ase" {
        return Err("expected .aseprite, .ase, or Aseprite JSON export".to_string());
    }

    let stem = sanitize_key(
        input_file
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("aseprite"),
    );
    let export_dir = project_path.join("assets").join("aseprite");
    fs::create_dir_all(&export_dir)
        .map_err(|error| format!("failed to create {}: {error}", export_dir.display()))?;
    let sheet = export_dir.join(format!("{stem}.png"));
    let json = export_dir.join(format!("{stem}.json"));
    let status = Command::new("aseprite")
        .arg("-b")
        .arg(input_file)
        .arg("--sheet")
        .arg(&sheet)
        .arg("--data")
        .arg(&json)
        .arg("--format")
        .arg("json-array")
        .status()
        .map_err(|error| {
            format!(
                "failed to run Aseprite CLI: {error}. Install Aseprite CLI or import an exported JSON file."
            )
        })?;
    if !status.success() {
        return Err(format!("Aseprite CLI export failed with status {status}"));
    }
    Ok(json)
}

fn parse_aseprite_json(
    json: &Value,
    json_file: &Path,
) -> Result<(Vec<AseFrame>, Vec<AseTag>, PathBuf, i32, i32), String> {
    let frames_value = json
        .get("frames")
        .ok_or_else(|| "missing frames".to_string())?;
    let mut frames = Vec::new();
    match frames_value {
        Value::Array(values) => {
            for value in values {
                frames.push(parse_frame(value)?);
            }
        }
        Value::Object(values) => {
            for (name, value) in values {
                let mut frame = parse_frame(value)?;
                if frame.name.trim().is_empty() {
                    frame.name = name.clone();
                }
                frames.push(frame);
            }
        }
        _ => return Err("frames must be an array or object".to_string()),
    }

    let meta = json.get("meta").unwrap_or(&Value::Null);
    let image = meta
        .get("image")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing meta.image".to_string())?;
    let image_path = json_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(image);
    let sheet_w = meta
        .get("size")
        .and_then(|value| value.get("w"))
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let sheet_h = meta
        .get("size")
        .and_then(|value| value.get("h"))
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let tags = meta
        .get("frameTags")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(parse_tag).collect::<Vec<_>>())
        .unwrap_or_default();

    Ok((frames, tags, image_path, sheet_w, sheet_h))
}

fn parse_frame(value: &Value) -> Result<AseFrame, String> {
    let rect = value
        .get("frame")
        .ok_or_else(|| "frame entry missing frame rect".to_string())?;
    Ok(AseFrame {
        name: value
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        x: rect.get("x").and_then(Value::as_i64).unwrap_or(0) as i32,
        y: rect.get("y").and_then(Value::as_i64).unwrap_or(0) as i32,
        w: rect.get("w").and_then(Value::as_i64).unwrap_or(0) as i32,
        h: rect.get("h").and_then(Value::as_i64).unwrap_or(0) as i32,
        duration_ms: value.get("duration").and_then(Value::as_i64).unwrap_or(100) as i32,
    })
}

fn parse_tag(value: &Value) -> Option<AseTag> {
    Some(AseTag {
        name: value.get("name")?.as_str()?.to_string(),
        from: value.get("from")?.as_u64()? as usize,
        to: value.get("to")?.as_u64()? as usize,
        direction: value
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("forward")
            .to_ascii_lowercase(),
    })
}

fn tag_frame_ids(tag: &AseTag, frame_ids: &[RowId]) -> Vec<RowId> {
    let from = tag.from.min(frame_ids.len());
    let to = tag.to.min(frame_ids.len().saturating_sub(1));
    if from > to {
        return Vec::new();
    }
    let forward = frame_ids[from..=to].to_vec();
    match tag.direction.as_str() {
        "reverse" => forward.into_iter().rev().collect(),
        "pingpong" | "ping-pong" => {
            let mut frames = forward.clone();
            frames.extend(forward.iter().rev().skip(1).copied().skip(1));
            frames
        }
        _ => forward,
    }
}

fn average_duration_seconds(tag: &AseTag, frames: &[AseFrame]) -> f32 {
    let from = tag.from.min(frames.len());
    let to = tag.to.min(frames.len().saturating_sub(1));
    if from > to {
        return 0.0;
    }
    let total = frames[from..=to]
        .iter()
        .map(|frame| frame.duration_ms.max(1) as f32 / 1000.0)
        .sum::<f32>();
    total / ((to - from + 1) as f32)
}

fn copy_texture(project_path: &Path, source: &Path, key: &str) -> Result<String, String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    let dest_dir = project_path.join("assets").join("aseprite");
    fs::create_dir_all(&dest_dir)
        .map_err(|error| format!("failed to create {}: {error}", dest_dir.display()))?;
    let dest = dest_dir.join(format!("{key}.{extension}"));
    if source != dest {
        fs::copy(source, &dest).map_err(|error| {
            format!(
                "failed to copy texture {} to {}: {error}",
                source.display(),
                dest.display()
            )
        })?;
    }
    Ok(format!("assets/aseprite/{key}.{extension}").replace('\\', "/"))
}

fn push_row(project: &mut DataProject, table_id: TableId, row: RowData) -> Result<(), String> {
    let table = project
        .data
        .iter_mut()
        .find(|table| table.table_id == table_id)
        .ok_or_else(|| format!("missing table data {:?}", table_id))?;
    table.rows.push(row);
    Ok(())
}

fn cells<const N: usize>(values: [(u64, CellValue); N]) -> BTreeMap<FieldId, CellValue> {
    values
        .into_iter()
        .map(|(field_id, value)| (FieldId(field_id), value))
        .collect()
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

fn unique_row_key(project: &DataProject, table_id: TableId, base: &str) -> String {
    let existing = project
        .data
        .iter()
        .find(|table| table.table_id == table_id)
        .map(|table| {
            table
                .rows
                .iter()
                .map(|row| row.key.as_str())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if !existing.contains(base) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base}_{index}");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}

fn sanitize_key(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    output = output.trim_matches('_').to_string();
    if output
        .chars()
        .next()
        .map(|ch| ch.is_ascii_lowercase())
        .unwrap_or(false)
    {
        output
    } else {
        format!("asset_{output}")
    }
}

fn display_name(key: &str) -> String {
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
