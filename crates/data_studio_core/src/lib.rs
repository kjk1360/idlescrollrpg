use std::collections::{hash_map::DefaultHasher, BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TableId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RowId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldKind {
    Bool,
    I32,
    I64,
    F32,
    String,
    Text,
    Enum { values: Vec<String> },
    AssetRef { asset_kind: String },
    RelationOne { target_table: TableId },
    RelationMany { target_table: TableId },
    ReferenceGroup { target_table: TableId },
    OwnedNestedTable { nested_table: TableId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldSchema {
    pub id: FieldId,
    pub key: String,
    pub display_name: String,
    pub kind: FieldKind,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TableSchema {
    pub id: TableId,
    pub key: String,
    pub display_name: String,
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CellValue {
    Empty,
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    String(String),
    Row(RowId),
    Rows(Vec<RowId>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowData {
    pub id: RowId,
    pub key: String,
    pub cells: BTreeMap<FieldId, CellValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableData {
    pub table_id: TableId,
    pub rows: Vec<RowData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectStatus {
    AllFresh,
    CodegenRequired,
    DataBuildRequired,
    CodegenAndDataBuildRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFingerprints {
    pub schema_hash: u64,
    pub generated_schema_hash: u64,
    pub data_hash: u64,
    pub built_data_hash: u64,
}

impl ProjectFingerprints {
    pub fn status(self) -> ProjectStatus {
        let code_dirty = self.schema_hash != self.generated_schema_hash;
        let data_dirty = self.data_hash != self.built_data_hash;

        match (code_dirty, data_dirty) {
            (false, false) => ProjectStatus::AllFresh,
            (true, false) => ProjectStatus::CodegenRequired,
            (false, true) => ProjectStatus::DataBuildRequired,
            (true, true) => ProjectStatus::CodegenAndDataBuildRequired,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataProject {
    pub tables: Vec<TableSchema>,
    pub data: Vec<TableData>,
    #[serde(default)]
    pub views: Vec<DataView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DataView {
    pub id: u64,
    pub key: String,
    pub display_name: String,
    pub source_table: TableId,
    pub joins: Vec<ViewJoin>,
    pub columns: Vec<ViewColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewJoin {
    pub from_alias: String,
    pub field: FieldId,
    pub alias: String,
    pub target_table: TableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewColumn {
    pub alias: String,
    pub field: FieldId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedView {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct RelationIndex<'a> {
    tables_by_id: HashMap<TableId, &'a TableSchema>,
    tables_by_key: HashMap<&'a str, &'a TableSchema>,
    data_by_table: HashMap<TableId, &'a TableData>,
    rows_by_id: HashMap<(TableId, RowId), &'a RowData>,
    rows_by_key: HashMap<(TableId, &'a str), &'a RowData>,
    fields_by_id: HashMap<(TableId, FieldId), &'a FieldSchema>,
    fields_by_key: HashMap<(TableId, &'a str), &'a FieldSchema>,
}

#[derive(Debug, Clone)]
struct RelationCodegenInfo {
    source_table_key: String,
    source_table_field: String,
    target_table_field: String,
    source_field: String,
    cache_field: String,
    method_name: String,
    cache_value_type: String,
    method_return_type: String,
    many: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format_version: u32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintFile {
    pub hash: u64,
}

#[derive(Debug)]
pub enum DataProjectError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for DataProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to access {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DataProjectError {}

impl DataProject {
    pub fn load_from_dir(path: impl AsRef<Path>) -> Result<Self, DataProjectError> {
        let root = path.as_ref();
        let tables_path = root.join("schema").join("tables.json");
        let tables: Vec<TableSchema> = read_json(&tables_path)?;
        let views_path = root.join("views").join("views.json");
        let views = if views_path.exists() {
            read_json(&views_path)?
        } else {
            Vec::new()
        };
        let mut data = Vec::new();

        for table in &tables {
            let data_path = root.join("data").join(format!("{}.json", table.key));
            let rows = if data_path.exists() {
                read_json(&data_path)?
            } else {
                Vec::new()
            };
            data.push(TableData {
                table_id: table.id,
                rows,
            });
        }

        Ok(Self {
            tables,
            data,
            views,
        })
    }

    pub fn save_to_dir(&self, path: impl AsRef<Path>, name: &str) -> Result<(), DataProjectError> {
        let root = path.as_ref();
        let schema_dir = root.join("schema");
        let data_dir = root.join("data");
        let build_dir = root.join("build");
        let views_dir = root.join("views");

        create_dir_all(&schema_dir)?;
        create_dir_all(&data_dir)?;
        create_dir_all(&build_dir)?;
        create_dir_all(&views_dir)?;

        write_json(
            &root.join("project.json"),
            &ProjectFile {
                format_version: 1,
                name: name.to_string(),
            },
        )?;
        write_json(&schema_dir.join("tables.json"), &self.tables)?;
        write_json(&views_dir.join("views.json"), &self.views)?;

        for table in &self.tables {
            let rows = self
                .data
                .iter()
                .find(|table_data| table_data.table_id == table.id)
                .map(|table_data| table_data.rows.as_slice())
                .unwrap_or(&[]);
            write_json(&data_dir.join(format!("{}.json", table.key)), rows)?;
        }

        Ok(())
    }

    pub fn fingerprints_from_dir(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ProjectFingerprints, DataProjectError> {
        let root = path.as_ref();
        Ok(ProjectFingerprints {
            schema_hash: self.schema_hash(),
            generated_schema_hash: read_fingerprint(root, "generated_schema_fingerprint.json")?
                .unwrap_or(0),
            data_hash: self.data_hash(),
            built_data_hash: read_fingerprint(root, "built_data_fingerprint.json")?.unwrap_or(0),
        })
    }

    pub fn write_generated_schema_fingerprint(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), DataProjectError> {
        write_fingerprint(
            path.as_ref(),
            "generated_schema_fingerprint.json",
            self.schema_hash(),
        )
    }

    pub fn write_built_data_fingerprint(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), DataProjectError> {
        write_fingerprint(
            path.as_ref(),
            "built_data_fingerprint.json",
            self.data_hash(),
        )
    }

    pub fn schema_hash(&self) -> u64 {
        stable_hash(&(self.tables.as_slice(), self.views.as_slice()))
    }

    pub fn data_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for table in &self.data {
            table.table_id.hash(&mut hasher);
            for row in &table.rows {
                row.id.hash(&mut hasher);
                row.key.hash(&mut hasher);
                for (field_id, value) in &row.cells {
                    field_id.hash(&mut hasher);
                    hash_cell_value(value, &mut hasher);
                }
            }
        }
        hasher.finish()
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let mut table_keys = HashMap::new();
        let mut table_ids = HashMap::new();

        for table in &self.tables {
            if table.key.trim().is_empty() {
                issues.push(error(format!("table {:?} has empty key", table.id)));
            }
            if let Some(previous) = table_keys.insert(table.key.as_str(), table.id) {
                issues.push(error(format!(
                    "duplicate table key {} used by {:?} and {:?}",
                    table.key, previous, table.id
                )));
            }
            if let Some(previous) = table_ids.insert(table.id, table.key.as_str()) {
                issues.push(error(format!(
                    "duplicate table id {:?} used by {} and {}",
                    table.id, previous, table.key
                )));
            }

            let mut field_keys = HashMap::new();
            let mut field_ids = HashMap::new();

            for field in &table.fields {
                if field.key.trim().is_empty() {
                    issues.push(error(format!(
                        "table {} has field {:?} with empty key",
                        table.key, field.id
                    )));
                }
                if let Some(previous) = field_keys.insert(field.key.as_str(), field.id) {
                    issues.push(error(format!(
                        "duplicate field key {}.{} used by {:?} and {:?}",
                        table.key, field.key, previous, field.id
                    )));
                }
                if let Some(previous) = field_ids.insert(field.id, field.key.as_str()) {
                    issues.push(error(format!(
                        "duplicate field id {:?} in table {} used by {} and {}",
                        field.id, table.key, previous, field.key
                    )));
                }

                match field.kind {
                    FieldKind::RelationOne { target_table }
                    | FieldKind::RelationMany { target_table }
                    | FieldKind::ReferenceGroup { target_table } => {
                        if self.table(target_table).is_none() {
                            issues.push(error(format!(
                                "field {}.{} references missing table {:?}",
                                table.key, field.key, target_table
                            )));
                        }
                    }
                    FieldKind::OwnedNestedTable { nested_table } => {
                        if self.table(nested_table).is_none() {
                            issues.push(error(format!(
                                "field {}.{} owns missing nested table {:?}",
                                table.key, field.key, nested_table
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }

        for table_data in &self.data {
            let Some(table_schema) = self.table(table_data.table_id) else {
                issues.push(error(format!(
                    "data exists for missing table {:?}",
                    table_data.table_id
                )));
                continue;
            };
            let mut row_keys = HashMap::new();
            let mut row_ids = HashMap::new();

            for row in &table_data.rows {
                if row.key.trim().is_empty() {
                    issues.push(error(format!(
                        "row {:?} in {} has empty key",
                        row.id, table_schema.key
                    )));
                }
                if let Some(previous) = row_keys.insert(row.key.as_str(), row.id) {
                    issues.push(error(format!(
                        "duplicate row key {}.{} used by {:?} and {:?}",
                        table_schema.key, row.key, previous, row.id
                    )));
                }
                if let Some(previous) = row_ids.insert(row.id, row.key.as_str()) {
                    issues.push(error(format!(
                        "duplicate row id {:?} in table {} used by {} and {}",
                        row.id, table_schema.key, previous, row.key
                    )));
                }

                for field_id in row.cells.keys() {
                    if !table_schema
                        .fields
                        .iter()
                        .any(|field| field.id == *field_id)
                    {
                        issues.push(error(format!(
                            "row {}.{} has cell for unknown field {:?}",
                            table_schema.key, row.key, field_id
                        )));
                    }
                }

                for field in &table_schema.fields {
                    let value = row.cells.get(&field.id).unwrap_or(&CellValue::Empty);
                    if field.required && matches!(value, CellValue::Empty) {
                        issues.push(error(format!(
                            "required field {}.{} is empty in row {}",
                            table_schema.key, field.key, row.key
                        )));
                    }
                    self.validate_cell_kind(table_schema, field, row, value, &mut issues);
                    self.validate_cell_relation(table_schema, field, value, &mut issues);
                }
            }
        }

        for view in &self.views {
            if view.key.trim().is_empty() {
                issues.push(error(format!("view {} has empty key", view.id)));
            }
            if self.table(view.source_table).is_none() {
                issues.push(error(format!(
                    "view {} references missing source table {:?}",
                    view.key, view.source_table
                )));
            }
            for join in &view.joins {
                if self.table(join.target_table).is_none() {
                    issues.push(error(format!(
                        "view {} join {} references missing target table {:?}",
                        view.key, join.alias, join.target_table
                    )));
                }
            }
        }

        issues
    }

    pub fn relation_index(&self) -> RelationIndex<'_> {
        RelationIndex::new(self)
    }

    pub fn view(&self, key: &str) -> Option<&DataView> {
        self.views.iter().find(|view| view.key == key)
    }

    pub fn materialize_view(&self, view_key: &str) -> Result<MaterializedView, String> {
        let view = self
            .view(view_key)
            .ok_or_else(|| format!("missing view {view_key}"))?;
        self.materialize_view_def(view)
    }

    pub fn materialize_view_def(&self, view: &DataView) -> Result<MaterializedView, String> {
        let index = self.relation_index();
        let source_data = index
            .table_data(view.source_table)
            .ok_or_else(|| format!("missing source data for view {}", view.key))?;
        let source_alias = "source".to_string();
        let mut records = source_data
            .rows
            .iter()
            .map(|row| {
                let mut record = BTreeMap::new();
                record.insert(source_alias.clone(), row);
                record
            })
            .collect::<Vec<_>>();

        for join in &view.joins {
            let mut expanded = Vec::new();
            for record in &records {
                let Some(from_row) = record.get(&join.from_alias) else {
                    return Err(format!(
                        "view {} join {} references missing alias {}",
                        view.key, join.alias, join.from_alias
                    ));
                };
                let target_rows =
                    resolve_join_rows(&index, join.target_table, from_row, join.field)?;
                for target_row in target_rows {
                    let mut next = record.clone();
                    next.insert(join.alias.clone(), target_row);
                    expanded.push(next);
                }
            }
            records = expanded;
        }

        let headers = view
            .columns
            .iter()
            .map(|column| column.label.clone())
            .collect::<Vec<_>>();
        let rows = records
            .iter()
            .map(|record| {
                view.columns
                    .iter()
                    .map(|column| {
                        record
                            .get(&column.alias)
                            .and_then(|row| row.cells.get(&column.field))
                            .map(cell_display)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        Ok(MaterializedView { headers, rows })
    }

    pub fn generate_rust_structs(&self) -> String {
        let mut output = String::new();
        output.push_str("// Generated by belt data studio. Do not edit manually.\n\n");
        output.push_str("use data_studio_core::RowId;\n\n");

        for table in &self.tables {
            output.push_str("#[derive(Debug, Clone)]\n");
            output.push_str(&format!("pub struct {} {{\n", rust_type_name(&table.key)));
            output.push_str("    pub id: RowId,\n");
            output.push_str("    pub key: String,\n");
            for field in &table.fields {
                output.push_str(&format!(
                    "    pub {}: {},\n",
                    rust_field_name(&field.key),
                    rust_type_for_field(&field.kind)
                ));
            }
            output.push_str("}\n\n");
        }

        output
    }

    pub fn generate_table_accessors(&self) -> String {
        let mut output = String::new();
        output.push_str("// Generated by belt data studio. Do not edit manually.\n\n");
        output.push_str("#![allow(dead_code)]\n\n");
        output.push_str("use std::collections::HashMap;\n\n");
        output.push_str(
            "use data_studio_core::{CellValue, DataProject, FieldId, RowData, RowId, TableId};\n\n",
        );
        output.push_str("use crate::schema_types::*;\n\n");
        output.push_str("#[derive(Debug, Clone)]\n");
        output.push_str("pub struct GeneratedDatabase {\n");
        for table in &self.tables {
            output.push_str(&format!(
                "    pub {}: GeneratedTable<{}>,\n",
                rust_field_name(&table.key),
                rust_type_name(&table.key)
            ));
        }
        output.push_str("}\n\n");

        output.push_str("#[derive(Debug, Clone)]\n");
        output.push_str("pub struct GeneratedTable<T> {\n");
        output.push_str("    pub rows: Vec<T>,\n");
        output.push_str("    by_id: HashMap<RowId, usize>,\n");
        output.push_str("    by_key: HashMap<String, usize>,\n");
        output.push_str("}\n\n");
        output.push_str("impl<T> GeneratedTable<T> {\n");
        output.push_str("    fn new(rows: Vec<T>, ids: Vec<RowId>, keys: Vec<String>) -> Self {\n");
        output.push_str("        let by_id = ids.into_iter().enumerate().map(|(index, id)| (id, index)).collect();\n");
        output.push_str("        let by_key = keys.into_iter().enumerate().map(|(index, key)| (key, index)).collect();\n");
        output.push_str("        Self { rows, by_id, by_key }\n");
        output.push_str("    }\n\n");
        output.push_str("    pub fn get_by_id(&self, id: RowId) -> Option<&T> {\n");
        output.push_str("        self.by_id.get(&id).and_then(|index| self.rows.get(*index))\n");
        output.push_str("    }\n\n");
        output.push_str("    pub fn get_by_key(&self, key: &str) -> Option<&T> {\n");
        output.push_str("        self.by_key.get(key).and_then(|index| self.rows.get(*index))\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        output.push_str("impl GeneratedDatabase {\n");
        output
            .push_str("    pub fn from_project(project: &DataProject) -> Result<Self, String> {\n");
        output.push_str("        Ok(Self {\n");
        for table in &self.tables {
            output.push_str(&format!(
                "            {}: load_{}(project)?,\n",
                rust_field_name(&table.key),
                rust_field_name(&table.key)
            ));
        }
        output.push_str("        })\n");
        output.push_str("    }\n");
        output.push_str("}\n\n");

        for table in &self.tables {
            let rust_type = rust_type_name(&table.key);
            let rust_fn = rust_field_name(&table.key);
            output.push_str(&format!(
                "fn load_{}(project: &DataProject) -> Result<GeneratedTable<{}>, String> {{\n",
                rust_fn, rust_type
            ));
            output.push_str(&format!(
                "    let rows = table_rows(project, TableId({}))?;\n",
                table.id.0
            ));
            output.push_str("    let mut typed_rows = Vec::new();\n");
            output.push_str("    let mut ids = Vec::new();\n");
            output.push_str("    let mut keys = Vec::new();\n");
            output.push_str("    for row in rows {\n");
            output.push_str(&format!("        typed_rows.push({} {{\n", rust_type));
            output.push_str("            id: row.id,\n");
            output.push_str("            key: row.key.clone(),\n");
            for field in &table.fields {
                output.push_str(&format!(
                    "            {}: read_{}(row, FieldId({}), \"{}.{0}\")?,\n",
                    rust_field_name(&field.key),
                    accessor_suffix(&field.kind),
                    field.id.0,
                    table.key
                ));
            }
            output.push_str("        });\n");
            output.push_str("        ids.push(row.id);\n");
            output.push_str("        keys.push(row.key.clone());\n");
            output.push_str("    }\n");
            output.push_str("    Ok(GeneratedTable::new(typed_rows, ids, keys))\n");
            output.push_str("}\n\n");
        }

        output.push_str("fn table_rows(project: &DataProject, table_id: TableId) -> Result<&[RowData], String> {\n");
        output.push_str("    project.data.iter().find(|table| table.table_id == table_id).map(|table| table.rows.as_slice()).ok_or_else(|| format!(\"missing table data {:?}\", table_id))\n");
        output.push_str("}\n\n");
        output.push_str("fn cell<'a>(row: &'a RowData, field_id: FieldId, label: &str) -> Result<&'a CellValue, String> {\n");
        output.push_str("    row.cells.get(&field_id).ok_or_else(|| format!(\"missing cell {label} in row {}\", row.key))\n");
        output.push_str("}\n\n");
        output.push_str("fn read_bool(row: &RowData, field_id: FieldId, label: &str) -> Result<bool, String> {\n");
        output.push_str("    match cell(row, field_id, label)? { CellValue::Bool(value) => Ok(*value), value => Err(format!(\"expected bool for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str(
            "fn read_i32(row: &RowData, field_id: FieldId, label: &str) -> Result<i32, String> {\n",
        );
        output.push_str("    match cell(row, field_id, label)? { CellValue::I32(value) => Ok(*value), value => Err(format!(\"expected i32 for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str(
            "fn read_i64(row: &RowData, field_id: FieldId, label: &str) -> Result<i64, String> {\n",
        );
        output.push_str("    match cell(row, field_id, label)? { CellValue::I64(value) => Ok(*value), value => Err(format!(\"expected i64 for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str(
            "fn read_f32(row: &RowData, field_id: FieldId, label: &str) -> Result<f32, String> {\n",
        );
        output.push_str("    match cell(row, field_id, label)? { CellValue::F32(value) => Ok(*value), value => Err(format!(\"expected f32 for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str("fn read_string(row: &RowData, field_id: FieldId, label: &str) -> Result<String, String> {\n");
        output.push_str("    match cell(row, field_id, label)? { CellValue::String(value) => Ok(value.clone()), value => Err(format!(\"expected string for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str("fn read_row(row: &RowData, field_id: FieldId, label: &str) -> Result<RowId, String> {\n");
        output.push_str("    match cell(row, field_id, label)? { CellValue::Row(value) => Ok(*value), value => Err(format!(\"expected row id for {label}, got {value:?}\")) }\n");
        output.push_str("}\n\n");
        output.push_str("fn read_rows(row: &RowData, field_id: FieldId, label: &str) -> Result<Vec<RowId>, String> {\n");
        output.push_str("    match cell(row, field_id, label)? { CellValue::Rows(value) => Ok(value.clone()), value => Err(format!(\"expected row id list for {label}, got {value:?}\")) }\n");
        output.push_str("}\n");

        output
    }

    pub fn generate_relation_cache(&self) -> String {
        let relations = self.relation_fields();
        let mut output = String::new();
        output.push_str("// Generated by belt data studio. Do not edit manually.\n\n");
        output.push_str("use std::collections::HashMap;\n\n");
        output.push_str("use data_studio_core::RowId;\n\n");
        output.push_str("use crate::table_accessors::GeneratedDatabase;\n\n");
        output.push_str("#[derive(Debug, Clone, Default)]\n");
        output.push_str("pub struct GeneratedRelationCache {\n");
        for relation in &relations {
            output.push_str(&format!(
                "    pub {}: HashMap<RowId, {}>,\n",
                relation.cache_field, relation.cache_value_type
            ));
        }
        output.push_str("}\n\n");

        output.push_str("impl GeneratedRelationCache {\n");
        output.push_str("    pub fn build(db: &GeneratedDatabase) -> Result<Self, String> {\n");
        output.push_str("        let mut cache = Self::default();\n");
        for relation in &relations {
            output.push_str(&format!(
                "        for row in &db.{}.rows {{\n",
                relation.source_table_field
            ));
            match relation.many {
                false => {
                    output.push_str(&format!(
                        "            if db.{}.get_by_id(row.{}).is_none() {{\n",
                        relation.target_table_field, relation.source_field
                    ));
                    output.push_str(&format!(
                        "                return Err(format!(\"missing relation target for {}.{} from {{:?}} to {{:?}}\", row.id, row.{}));\n",
                        relation.source_table_key, relation.source_field, relation.source_field
                    ));
                    output.push_str("            }\n");
                    output.push_str(&format!(
                        "            cache.{}.insert(row.id, row.{});\n",
                        relation.cache_field, relation.source_field
                    ));
                }
                true => {
                    output.push_str(&format!(
                        "            for target_id in &row.{} {{\n",
                        relation.source_field
                    ));
                    output.push_str(&format!(
                        "                if db.{}.get_by_id(*target_id).is_none() {{\n",
                        relation.target_table_field
                    ));
                    output.push_str(&format!(
                        "                    return Err(format!(\"missing relation target for {}.{} from {{:?}} to {{:?}}\", row.id, target_id));\n",
                        relation.source_table_key, relation.source_field
                    ));
                    output.push_str("                }\n");
                    output.push_str("            }\n");
                    output.push_str(&format!(
                        "            cache.{}.insert(row.id, row.{}.clone());\n",
                        relation.cache_field, relation.source_field
                    ));
                }
            }
            output.push_str("        }\n");
        }
        output.push_str("        Ok(cache)\n");
        output.push_str("    }\n");

        for relation in &relations {
            output.push('\n');
            output.push_str(&format!(
                "    pub fn {}(&self, source: RowId) -> Option<{}> {{\n",
                relation.method_name, relation.method_return_type
            ));
            match relation.many {
                false => {
                    output.push_str(&format!(
                        "        self.{}.get(&source).copied()\n",
                        relation.cache_field
                    ));
                }
                true => {
                    output.push_str(&format!(
                        "        self.{}.get(&source).map(Vec::as_slice)\n",
                        relation.cache_field
                    ));
                }
            }
            output.push_str("    }\n");
        }

        output.push_str("}\n");
        output
    }

    fn relation_fields(&self) -> Vec<RelationCodegenInfo> {
        let mut relations = Vec::new();
        for table in &self.tables {
            for field in &table.fields {
                let Some((target_table, many)) = relation_target(&field.kind) else {
                    continue;
                };
                let Some(target_table_schema) = self.table(target_table) else {
                    continue;
                };
                let source_table_field = rust_field_name(&table.key);
                let target_table_field = rust_field_name(&target_table_schema.key);
                let source_field = rust_field_name(&field.key);
                let cache_field = format!("{}_{}", source_table_field, source_field);
                relations.push(RelationCodegenInfo {
                    source_table_key: table.key.clone(),
                    source_table_field,
                    target_table_field,
                    source_field: source_field.clone(),
                    cache_field: cache_field.clone(),
                    method_name: format!("get_{cache_field}"),
                    cache_value_type: if many {
                        "Vec<RowId>".to_string()
                    } else {
                        "RowId".to_string()
                    },
                    method_return_type: if many {
                        "&[RowId]".to_string()
                    } else {
                        "RowId".to_string()
                    },
                    many,
                });
            }
        }
        relations
    }

    fn table(&self, table_id: TableId) -> Option<&TableSchema> {
        self.tables.iter().find(|table| table.id == table_id)
    }

    fn row_exists(&self, table_id: TableId, row_id: RowId) -> bool {
        self.data
            .iter()
            .find(|table| table.table_id == table_id)
            .map(|table| table.rows.iter().any(|row| row.id == row_id))
            .unwrap_or(false)
    }

    fn validate_cell_relation(
        &self,
        table_schema: &TableSchema,
        field: &FieldSchema,
        value: &CellValue,
        issues: &mut Vec<ValidationIssue>,
    ) {
        match (&field.kind, value) {
            (FieldKind::RelationOne { target_table }, CellValue::Row(row_id)) => {
                if !self.row_exists(*target_table, *row_id) {
                    issues.push(error(format!(
                        "{}.{} points to missing row {:?}",
                        table_schema.key, field.key, row_id
                    )));
                }
            }
            (
                FieldKind::RelationMany { target_table }
                | FieldKind::ReferenceGroup { target_table },
                CellValue::Rows(row_ids),
            ) => {
                for row_id in row_ids {
                    if !self.row_exists(*target_table, *row_id) {
                        issues.push(error(format!(
                            "{}.{} points to missing row {:?}",
                            table_schema.key, field.key, row_id
                        )));
                    }
                }
            }
            _ => {}
        }
    }

    fn validate_cell_kind(
        &self,
        table_schema: &TableSchema,
        field: &FieldSchema,
        row: &RowData,
        value: &CellValue,
        issues: &mut Vec<ValidationIssue>,
    ) {
        if matches!(value, CellValue::Empty) {
            return;
        }

        let valid = matches!(
            (&field.kind, value),
            (FieldKind::Bool, CellValue::Bool(_))
                | (FieldKind::I32, CellValue::I32(_))
                | (FieldKind::I64, CellValue::I64(_))
                | (FieldKind::F32, CellValue::F32(_))
                | (FieldKind::String | FieldKind::Text, CellValue::String(_))
                | (FieldKind::Enum { .. }, CellValue::String(_))
                | (FieldKind::AssetRef { .. }, CellValue::String(_))
                | (FieldKind::RelationOne { .. }, CellValue::Row(_))
                | (FieldKind::RelationMany { .. }, CellValue::Rows(_))
                | (FieldKind::ReferenceGroup { .. }, CellValue::Rows(_))
                | (FieldKind::OwnedNestedTable { .. }, CellValue::Rows(_))
        );

        if !valid {
            issues.push(error(format!(
                "field {}.{} in row {} has invalid value kind {:?}",
                table_schema.key, field.key, row.key, value
            )));
            return;
        }

        match (&field.kind, value) {
            (FieldKind::Enum { values }, CellValue::String(value)) => {
                if !values.iter().any(|allowed| allowed == value) {
                    issues.push(error(format!(
                        "field {}.{} in row {} has invalid enum value {}",
                        table_schema.key, field.key, row.key, value
                    )));
                }
            }
            (
                FieldKind::RelationMany { .. }
                | FieldKind::ReferenceGroup { .. }
                | FieldKind::OwnedNestedTable { .. },
                CellValue::Rows(row_ids),
            ) if field.required && row_ids.is_empty() => {
                issues.push(error(format!(
                    "required relation list {}.{} is empty in row {}",
                    table_schema.key, field.key, row.key
                )));
            }
            _ => {}
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, DataProjectError> {
    let text = fs::read_to_string(path).map_err(|source| DataProjectError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| DataProjectError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), DataProjectError> {
    let text = serde_json::to_string_pretty(value).map_err(|source| DataProjectError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, text).map_err(|source| DataProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn create_dir_all(path: &Path) -> Result<(), DataProjectError> {
    fs::create_dir_all(path).map_err(|source| DataProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_fingerprint(root: &Path, filename: &str) -> Result<Option<u64>, DataProjectError> {
    let path = root.join("build").join(filename);
    if !path.exists() {
        return Ok(None);
    }
    let file: FingerprintFile = read_json(&path)?;
    Ok(Some(file.hash))
}

fn write_fingerprint(root: &Path, filename: &str, hash: u64) -> Result<(), DataProjectError> {
    let build_dir = root.join("build");
    create_dir_all(&build_dir)?;
    write_json(&build_dir.join(filename), &FingerprintFile { hash })
}

pub fn sample_project() -> DataProject {
    let unit_table = TableSchema {
        id: TableId(1),
        key: "unit_def".to_string(),
        display_name: "Unit Def".to_string(),
        fields: vec![
            field(1, "name", FieldKind::String, true),
            field(2, "max_hp", FieldKind::I32, true),
            field(3, "attack", FieldKind::I32, true),
            field(4, "attack_range", FieldKind::F32, true),
        ],
    };

    let group_table = TableSchema {
        id: TableId(2),
        key: "unit_group".to_string(),
        display_name: "Unit Group".to_string(),
        fields: vec![
            field(10, "name", FieldKind::String, true),
            field(
                11,
                "members",
                FieldKind::ReferenceGroup {
                    target_table: unit_table.id,
                },
                true,
            ),
        ],
    };

    let data = vec![
        TableData {
            table_id: unit_table.id,
            rows: vec![
                RowData {
                    id: RowId(1001),
                    key: "knight".to_string(),
                    cells: cells(vec![
                        (FieldId(1), CellValue::String("Knight".to_string())),
                        (FieldId(2), CellValue::I32(120)),
                        (FieldId(3), CellValue::I32(18)),
                        (FieldId(4), CellValue::F32(1.3)),
                    ]),
                },
                RowData {
                    id: RowId(1002),
                    key: "archer".to_string(),
                    cells: cells(vec![
                        (FieldId(1), CellValue::String("Archer".to_string())),
                        (FieldId(2), CellValue::I32(70)),
                        (FieldId(3), CellValue::I32(12)),
                        (FieldId(4), CellValue::F32(5.0)),
                    ]),
                },
            ],
        },
        TableData {
            table_id: group_table.id,
            rows: vec![RowData {
                id: RowId(2001),
                key: "party_start".to_string(),
                cells: cells(vec![
                    (FieldId(10), CellValue::String("Start Party".to_string())),
                    (FieldId(11), CellValue::Rows(vec![RowId(1001), RowId(1002)])),
                ]),
            }],
        },
    ];

    DataProject {
        tables: vec![unit_table, group_table],
        data,
        views: Vec::new(),
    }
}

fn field(id: u64, key: &str, kind: FieldKind, required: bool) -> FieldSchema {
    FieldSchema {
        id: FieldId(id),
        key: key.to_string(),
        display_name: key.to_string(),
        kind,
        required,
    }
}

fn cells(items: Vec<(FieldId, CellValue)>) -> BTreeMap<FieldId, CellValue> {
    items.into_iter().collect()
}

fn error(message: String) -> ValidationIssue {
    ValidationIssue {
        severity: ValidationSeverity::Error,
        message,
    }
}

fn stable_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hash_cell_value(value: &CellValue, hasher: &mut DefaultHasher) {
    match value {
        CellValue::Empty => 0_u8.hash(hasher),
        CellValue::Bool(value) => {
            1_u8.hash(hasher);
            value.hash(hasher);
        }
        CellValue::I32(value) => {
            2_u8.hash(hasher);
            value.hash(hasher);
        }
        CellValue::I64(value) => {
            3_u8.hash(hasher);
            value.hash(hasher);
        }
        CellValue::F32(value) => {
            4_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        CellValue::String(value) => {
            5_u8.hash(hasher);
            value.hash(hasher);
        }
        CellValue::Row(value) => {
            6_u8.hash(hasher);
            value.hash(hasher);
        }
        CellValue::Rows(values) => {
            7_u8.hash(hasher);
            values.hash(hasher);
        }
    }
}

fn rust_type_name(key: &str) -> String {
    key.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn rust_field_name(key: &str) -> String {
    key.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn rust_type_for_field(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Bool => "bool",
        FieldKind::I32 => "i32",
        FieldKind::I64 => "i64",
        FieldKind::F32 => "f32",
        FieldKind::String
        | FieldKind::Text
        | FieldKind::Enum { .. }
        | FieldKind::AssetRef { .. } => "String",
        FieldKind::RelationOne { .. } => "RowId",
        FieldKind::RelationMany { .. } | FieldKind::ReferenceGroup { .. } => "Vec<RowId>",
        FieldKind::OwnedNestedTable { .. } => "Vec<RowId>",
    }
}

fn accessor_suffix(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Bool => "bool",
        FieldKind::I32 => "i32",
        FieldKind::I64 => "i64",
        FieldKind::F32 => "f32",
        FieldKind::String
        | FieldKind::Text
        | FieldKind::Enum { .. }
        | FieldKind::AssetRef { .. } => "string",
        FieldKind::RelationOne { .. } => "row",
        FieldKind::RelationMany { .. }
        | FieldKind::ReferenceGroup { .. }
        | FieldKind::OwnedNestedTable { .. } => "rows",
    }
}

fn relation_target(kind: &FieldKind) -> Option<(TableId, bool)> {
    match kind {
        FieldKind::RelationOne { target_table } => Some((*target_table, false)),
        FieldKind::RelationMany { target_table } | FieldKind::ReferenceGroup { target_table } => {
            Some((*target_table, true))
        }
        FieldKind::OwnedNestedTable { nested_table } => Some((*nested_table, true)),
        _ => None,
    }
}

impl<'a> RelationIndex<'a> {
    pub fn new(project: &'a DataProject) -> Self {
        let mut index = Self {
            tables_by_id: HashMap::new(),
            tables_by_key: HashMap::new(),
            data_by_table: HashMap::new(),
            rows_by_id: HashMap::new(),
            rows_by_key: HashMap::new(),
            fields_by_id: HashMap::new(),
            fields_by_key: HashMap::new(),
        };

        for table in &project.tables {
            index.tables_by_id.insert(table.id, table);
            index.tables_by_key.insert(table.key.as_str(), table);
            for field in &table.fields {
                index.fields_by_id.insert((table.id, field.id), field);
                index
                    .fields_by_key
                    .insert((table.id, field.key.as_str()), field);
            }
        }

        for table_data in &project.data {
            index.data_by_table.insert(table_data.table_id, table_data);
            for row in &table_data.rows {
                index.rows_by_id.insert((table_data.table_id, row.id), row);
                index
                    .rows_by_key
                    .insert((table_data.table_id, row.key.as_str()), row);
            }
        }

        index
    }

    pub fn table_by_id(&self, table_id: TableId) -> Option<&'a TableSchema> {
        self.tables_by_id.get(&table_id).copied()
    }

    pub fn table_by_key(&self, key: &str) -> Option<&'a TableSchema> {
        self.tables_by_key.get(key).copied()
    }

    pub fn table_data(&self, table_id: TableId) -> Option<&'a TableData> {
        self.data_by_table.get(&table_id).copied()
    }

    pub fn row_by_id(&self, table_id: TableId, row_id: RowId) -> Option<&'a RowData> {
        self.rows_by_id.get(&(table_id, row_id)).copied()
    }

    pub fn row_by_key(&self, table_id: TableId, key: &str) -> Option<&'a RowData> {
        self.rows_by_key.get(&(table_id, key)).copied()
    }

    pub fn field_by_id(&self, table_id: TableId, field_id: FieldId) -> Option<&'a FieldSchema> {
        self.fields_by_id.get(&(table_id, field_id)).copied()
    }

    pub fn field_by_key(&self, table_id: TableId, key: &str) -> Option<&'a FieldSchema> {
        self.fields_by_key.get(&(table_id, key)).copied()
    }
}

fn resolve_join_rows<'a>(
    index: &RelationIndex<'a>,
    target_table: TableId,
    from_row: &'a RowData,
    field: FieldId,
) -> Result<Vec<&'a RowData>, String> {
    match from_row.cells.get(&field).unwrap_or(&CellValue::Empty) {
        CellValue::Row(row_id) => index
            .row_by_id(target_table, *row_id)
            .map(|row| vec![row])
            .ok_or_else(|| {
                format!(
                    "missing joined row {:?} in table {:?}",
                    row_id, target_table
                )
            }),
        CellValue::Rows(row_ids) => row_ids
            .iter()
            .map(|row_id| {
                index.row_by_id(target_table, *row_id).ok_or_else(|| {
                    format!(
                        "missing joined row {:?} in table {:?}",
                        row_id, target_table
                    )
                })
            })
            .collect(),
        CellValue::Empty => Ok(Vec::new()),
        value => Err(format!("cannot join through non-relation value {value:?}")),
    }
}

fn cell_display(value: &CellValue) -> String {
    match value {
        CellValue::Empty => String::new(),
        CellValue::Bool(value) => value.to_string(),
        CellValue::I32(value) => value.to_string(),
        CellValue::I64(value) => value.to_string(),
        CellValue::F32(value) => value.to_string(),
        CellValue::String(value) => value.clone(),
        CellValue::Row(value) => value.0.to_string(),
        CellValue::Rows(values) => values
            .iter()
            .map(|row_id| row_id.0.to_string())
            .collect::<Vec<_>>()
            .join(","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegen_status_detects_schema_drift() {
        let project = sample_project();
        let schema_hash = project.schema_hash();
        let data_hash = project.data_hash();

        let status = ProjectFingerprints {
            schema_hash,
            generated_schema_hash: schema_hash + 1,
            data_hash,
            built_data_hash: data_hash,
        }
        .status();

        assert_eq!(status, ProjectStatus::CodegenRequired);
    }

    #[test]
    fn sample_project_validates() {
        let issues = sample_project().validate();
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn generates_rust_struct_names_from_table_keys() {
        let generated = sample_project().generate_rust_structs();
        assert!(generated.contains("pub struct UnitDef"));
        assert!(generated.contains("pub members: Vec<RowId>"));
    }

    #[test]
    fn save_and_load_project_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "belt_data_project_roundtrip_{}",
            std::process::id()
        ));
        let project = sample_project();

        project
            .save_to_dir(&root, "Roundtrip")
            .expect("sample project should save");
        let loaded = DataProject::load_from_dir(&root).expect("sample project should load");

        assert_eq!(loaded.schema_hash(), project.schema_hash());
        assert_eq!(loaded.data_hash(), project.data_hash());
        assert!(loaded.validate().is_empty());
    }

    #[test]
    fn materializes_source_table_view() {
        let mut project = sample_project();
        project.views.push(DataView {
            id: 1,
            key: "unit_names".to_string(),
            display_name: "Unit Names".to_string(),
            source_table: TableId(1),
            joins: Vec::new(),
            columns: vec![ViewColumn {
                alias: "source".to_string(),
                field: FieldId(1),
                label: "Name".to_string(),
            }],
        });

        let view = project
            .materialize_view("unit_names")
            .expect("view should materialize");

        assert_eq!(view.headers, vec!["Name"]);
        assert_eq!(view.rows, vec![vec!["Knight"], vec!["Archer"]]);
    }

    #[test]
    fn validation_catches_duplicate_row_keys() {
        let mut project = sample_project();
        project.data[0].rows[1].key = project.data[0].rows[0].key.clone();

        let issues = project.validate();

        assert!(has_issue(&issues, "duplicate row key"));
    }

    #[test]
    fn validation_catches_unknown_cell_fields() {
        let mut project = sample_project();
        project.data[0].rows[0]
            .cells
            .insert(FieldId(999), CellValue::I32(1));

        let issues = project.validate();

        assert!(has_issue(&issues, "unknown field"));
    }

    #[test]
    fn validation_catches_cell_kind_mismatch() {
        let mut project = sample_project();
        project.data[0].rows[0]
            .cells
            .insert(FieldId(2), CellValue::String("bad".to_string()));

        let issues = project.validate();

        assert!(has_issue(&issues, "invalid value kind"));
    }

    #[test]
    fn validation_catches_empty_required_relation_list() {
        let mut project = sample_project();
        project.data[1].rows[0]
            .cells
            .insert(FieldId(11), CellValue::Rows(Vec::new()));

        let issues = project.validate();

        assert!(has_issue(&issues, "required relation list"));
    }

    fn has_issue(issues: &[ValidationIssue], pattern: &str) -> bool {
        issues.iter().any(|issue| issue.message.contains(pattern))
    }
}
