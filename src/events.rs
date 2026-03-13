use std::fmt;

use etl::types::{ArrayCell, Cell, Event, InsertEvent, TableId, TableRow, UpdateEvent};

/// Column types that cannot contain text patterns (skipped during scanning).
const NON_TEXT_TYPES: &[&str] = &[
    "bool",
    "int2",
    "int4",
    "int8",
    "float4",
    "float8",
    "numeric",
    "oid",
    "bytea",
    "uuid",
    "date",
    "time",
    "timetz",
    "timestamp",
    "timestamptz",
    "interval",
];

pub fn is_scannable_type(type_name: &str) -> bool {
    !NON_TEXT_TYPES.contains(&type_name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Insert,
    Update,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Insert => write!(f, "INSERT"),
            Action::Update => write!(f, "UPDATE"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColumnValue {
    pub name: String,
    pub type_name: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScanEvent {
    pub database: String,
    pub table_id: TableId,
    pub schema_name: String,
    pub table_name: String,
    pub action: Action,
    pub columns: Vec<ColumnValue>,
    /// With REPLICA IDENTITY FULL, PostgreSQL marks all columns as primary.
    pub primary_keys: Vec<(String, String)>,
    pub start_lsn: u64,
    pub commit_lsn: u64,
}

/// Schema metadata for a table, built from Relation events.
#[derive(Debug, Clone)]
pub struct TableMeta {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnMeta>,
}

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub primary: bool,
}

pub fn cell_to_string(cell: &Cell) -> Option<String> {
    match cell {
        Cell::Null => None,
        Cell::Bool(b) => Some(b.to_string()),
        Cell::String(s) => Some(s.clone()),
        Cell::I16(n) => Some(n.to_string()),
        Cell::I32(n) => Some(n.to_string()),
        Cell::U32(n) => Some(n.to_string()),
        Cell::I64(n) => Some(n.to_string()),
        Cell::F32(n) => Some(n.to_string()),
        Cell::F64(n) => Some(n.to_string()),
        Cell::Numeric(n) => Some(n.to_string()),
        Cell::Date(d) => Some(d.to_string()),
        Cell::Time(t) => Some(t.to_string()),
        Cell::Timestamp(ts) => Some(ts.to_string()),
        Cell::TimestampTz(ts) => Some(ts.to_string()),
        Cell::Uuid(u) => Some(u.to_string()),
        Cell::Json(j) => Some(j.to_string()),
        Cell::Bytes(_) => None,
        Cell::Array(arr) => Some(array_cell_to_string(arr)),
    }
}

fn array_cell_to_string(arr: &ArrayCell) -> String {
    macro_rules! format_array {
        ($values:expr) => {
            $values
                .iter()
                .map(|v| match v {
                    Some(val) => val.to_string(),
                    None => "NULL".to_string(),
                })
                .collect::<Vec<_>>()
                .join(",")
        };
    }

    let inner = match arr {
        ArrayCell::Bool(v) => format_array!(v),
        ArrayCell::String(v) => format_array!(v),
        ArrayCell::I16(v) => format_array!(v),
        ArrayCell::I32(v) => format_array!(v),
        ArrayCell::U32(v) => format_array!(v),
        ArrayCell::I64(v) => format_array!(v),
        ArrayCell::F32(v) => format_array!(v),
        ArrayCell::F64(v) => format_array!(v),
        ArrayCell::Numeric(v) => format_array!(v),
        ArrayCell::Date(v) => format_array!(v),
        ArrayCell::Time(v) => format_array!(v),
        ArrayCell::Timestamp(v) => format_array!(v),
        ArrayCell::TimestampTz(v) => format_array!(v),
        ArrayCell::Uuid(v) => format_array!(v),
        ArrayCell::Json(v) => format_array!(v),
        ArrayCell::Bytes(v) => v
            .iter()
            .map(|val| match val {
                Some(_) => "<bytes>".to_string(),
                None => "NULL".to_string(),
            })
            .collect::<Vec<_>>()
            .join(","),
    };

    format!("{{{inner}}}")
}

fn extract_primary_keys(row: &TableRow, meta: &TableMeta) -> Vec<(String, String)> {
    row.values
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| {
            let col = meta.columns.get(i)?;
            if col.primary {
                cell_to_string(cell).map(|v| (col.name.clone(), v))
            } else {
                None
            }
        })
        .collect()
}

fn extract_columns(row: &TableRow, meta: &TableMeta) -> Vec<ColumnValue> {
    row.values
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let (name, type_name) = meta
                .columns
                .get(i)
                .map(|c| (c.name.clone(), c.type_name.clone()))
                .unwrap_or_else(|| (format!("col_{i}"), "unknown".to_string()));

            let value = if is_scannable_type(&type_name) { cell_to_string(cell) } else { None };

            ColumnValue { name, type_name, value }
        })
        .collect()
}

pub fn from_insert(event: &InsertEvent, meta: &TableMeta, database: &str) -> ScanEvent {
    ScanEvent {
        database: database.to_string(),
        table_id: event.table_id,
        schema_name: meta.schema.clone(),
        table_name: meta.name.clone(),
        action: Action::Insert,
        columns: extract_columns(&event.table_row, meta),
        primary_keys: extract_primary_keys(&event.table_row, meta),
        start_lsn: u64::from(event.start_lsn),
        commit_lsn: u64::from(event.commit_lsn),
    }
}

pub fn from_update(event: &UpdateEvent, meta: &TableMeta, database: &str) -> ScanEvent {
    ScanEvent {
        database: database.to_string(),
        table_id: event.table_id,
        schema_name: meta.schema.clone(),
        table_name: meta.name.clone(),
        action: Action::Update,
        columns: extract_columns(&event.table_row, meta),
        primary_keys: extract_primary_keys(&event.table_row, meta),
        start_lsn: u64::from(event.start_lsn),
        commit_lsn: u64::from(event.commit_lsn),
    }
}

pub fn extract_scan_events(events: &[Event], table_registry: &std::collections::HashMap<TableId, TableMeta>, database: &str) -> Vec<ScanEvent> {
    let mut scan_events = Vec::new();

    for event in events {
        match event {
            Event::Insert(e) => {
                if let Some(meta) = table_registry.get(&e.table_id) {
                    scan_events.push(from_insert(e, meta, database));
                }
            },
            Event::Update(e) => {
                if let Some(meta) = table_registry.get(&e.table_id) {
                    scan_events.push(from_update(e, meta, database));
                }
            },
            _ => {},
        }
    }

    scan_events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> TableMeta {
        TableMeta {
            schema: "public".to_string(),
            name: "t1".to_string(),
            columns: vec![
                ColumnMeta {
                    name: "pk".to_string(),
                    type_name: "int4".to_string(),
                    primary: true,
                },
                ColumnMeta {
                    name: "col_a".to_string(),
                    type_name: "text".to_string(),
                    primary: false,
                },
                ColumnMeta {
                    name: "col_b".to_string(),
                    type_name: "text".to_string(),
                    primary: false,
                },
            ],
        }
    }

    #[rstest::rstest]
    #[case(Cell::Null, None)]
    #[case(Cell::String("hello".into()), Some("hello"))]
    #[case(Cell::I32(42), Some("42"))]
    #[case(Cell::Bool(true), Some("true"))]
    #[case(Cell::F64(1.5), Some("1.5"))]
    #[case(Cell::I16(0), Some("0"))]
    #[case(Cell::Bytes(vec![1, 2, 3]), None)]
    fn cell_to_string_cases(#[case] cell: Cell, #[case] expected: Option<&str>) {
        assert_eq!(cell_to_string(&cell), expected.map(String::from));
    }

    #[test]
    fn extracts_columns() {
        let row = TableRow {
            values: vec![Cell::I32(1), Cell::String("foo".into()), Cell::String("bar".into())],
        };
        let meta = test_meta();
        let cols = extract_columns(&row, &meta);

        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0].name, "pk");
        assert_eq!(cols[0].value, None); // int4 is non-scannable, skipped
        assert_eq!(cols[1].name, "col_a");
        assert_eq!(cols[1].value, Some("foo".into()));
        assert_eq!(cols[2].name, "col_b");
        assert_eq!(cols[2].value, Some("bar".into()));
    }

    #[test]
    fn extracts_primary_keys() {
        let row = TableRow {
            values: vec![Cell::I32(1), Cell::String("foo".into()), Cell::String("bar".into())],
        };
        let meta = test_meta();
        let pks = extract_primary_keys(&row, &meta);

        assert_eq!(pks.len(), 1);
        assert_eq!(pks[0], ("pk".to_string(), "1".to_string()));
    }

    #[test]
    fn extracts_composite_primary_keys() {
        let meta = TableMeta {
            schema: "public".to_string(),
            name: "t2".to_string(),
            columns: vec![
                ColumnMeta {
                    name: "pk_a".to_string(),
                    type_name: "text".to_string(),
                    primary: true,
                },
                ColumnMeta {
                    name: "pk_b".to_string(),
                    type_name: "int4".to_string(),
                    primary: true,
                },
                ColumnMeta {
                    name: "col_a".to_string(),
                    type_name: "text".to_string(),
                    primary: false,
                },
            ],
        };
        let row = TableRow {
            values: vec![Cell::String("x".into()), Cell::I32(42), Cell::String("y".into())],
        };
        let pks = extract_primary_keys(&row, &meta);

        assert_eq!(pks.len(), 2);
        assert_eq!(pks[0], ("pk_a".to_string(), "x".to_string()));
        assert_eq!(pks[1], ("pk_b".to_string(), "42".to_string()));
    }

    #[rstest::rstest]
    #[case(Action::Insert, "INSERT")]
    #[case(Action::Update, "UPDATE")]
    fn action_display(#[case] action: Action, #[case] expected: &str) {
        assert_eq!(action.to_string(), expected);
    }

    #[rstest::rstest]
    #[case("text", true)]
    #[case("varchar", true)]
    #[case("char", true)]
    #[case("jsonb", true)]
    #[case("json", true)]
    #[case("xml", true)]
    #[case("int4", false)]
    #[case("bool", false)]
    #[case("uuid", false)]
    #[case("numeric", false)]
    #[case("timestamptz", false)]
    fn is_scannable_type_classification(#[case] type_name: &str, #[case] expected: bool) {
        assert_eq!(is_scannable_type(type_name), expected, "is_scannable_type({type_name:?})");
    }
}
