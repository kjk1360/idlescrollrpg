use std::collections::{hash_map::DefaultHasher, BTreeMap};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TableId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldSchema {
    pub id: FieldId,
    pub key: String,
    pub display_name: String,
    pub kind: FieldKind,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableSchema {
    pub id: TableId,
    pub key: String,
    pub display_name: String,
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct RowData {
    pub id: RowId,
    pub key: String,
    pub cells: BTreeMap<FieldId, CellValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableData {
    pub table_id: TableId,
    pub rows: Vec<RowData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    AllFresh,
    CodegenRequired,
    DataBuildRequired,
    CodegenAndDataBuildRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default)]
pub struct DataProject {
    pub tables: Vec<TableSchema>,
    pub data: Vec<TableData>,
}

impl DataProject {
    pub fn schema_hash(&self) -> u64 {
        stable_hash(&self.tables)
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

        for table in &self.tables {
            if table.key.trim().is_empty() {
                issues.push(error(format!("table {:?} has empty key", table.id)));
            }

            for field in &table.fields {
                if field.key.trim().is_empty() {
                    issues.push(error(format!(
                        "table {} has field {:?} with empty key",
                        table.key, field.id
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

            for row in &table_data.rows {
                if row.key.trim().is_empty() {
                    issues.push(error(format!(
                        "row {:?} in {} has empty key",
                        row.id, table_schema.key
                    )));
                }

                for field in &table_schema.fields {
                    let value = row.cells.get(&field.id).unwrap_or(&CellValue::Empty);
                    if field.required && matches!(value, CellValue::Empty) {
                        issues.push(error(format!(
                            "required field {}.{} is empty in row {}",
                            table_schema.key, field.key, row.key
                        )));
                    }
                    self.validate_cell_relation(table_schema, field, value, &mut issues);
                }
            }
        }

        issues
    }

    pub fn generate_rust_structs(&self) -> String {
        let mut output = String::new();
        output.push_str("// Generated by belt data studio. Do not edit manually.\n\n");

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
}
