use serde::{Deserialize, Serialize};

/// Database-agnostic representation of a schema (namespace).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Schema {
    /// Schema/namespace name (e.g. "public", "analytics").
    pub name: String,
    /// Named enum types defined in this schema (Postgres CREATE TYPE).
    pub enums: Vec<EnumType>,
    /// Tables in this schema.
    pub tables: Vec<Table>,
}

impl Schema {
    /// Create a new empty schema with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enums: Vec::new(),
            tables: Vec::new(),
        }
    }
}

/// A named enum type (Postgres `CREATE TYPE ... AS ENUM`).
/// MySQL inline ENUMs are represented as `DataType::Enum` on the column.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EnumType {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
    pub check_constraints: Vec<CheckConstraint>,
    pub unique_constraints: Vec<UniqueConstraint>,
    /// Table-level options (e.g. MySQL engine, charset).
    pub options: TableOptions,
}

impl Table {
    /// Create a new table with the given name and no columns/constraints.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: Vec::new(),
            primary_key: None,
            foreign_keys: Vec::new(),
            indexes: Vec::new(),
            check_constraints: Vec::new(),
            unique_constraints: Vec::new(),
            options: TableOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<ColumnDefault>,
    /// Whether this column auto-increments (serial, auto_increment, identity).
    pub auto_increment: bool,
    /// Whether this is an unsigned integer (MySQL).
    pub unsigned: bool,
}

/// Column default value — either a literal or a SQL expression.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ColumnDefault {
    /// A literal value (string, number, boolean).
    Literal(String),
    /// A SQL expression (e.g. `gen_random_uuid()`, `now()`, `CURRENT_TIMESTAMP`).
    Expression(String),
}

/// Database-agnostic data types covering Postgres and MySQL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DataType {
    // Integer types
    SmallInt,
    Integer,
    BigInt,

    // Float types
    Real,
    DoublePrecision,

    // Exact numeric
    Numeric {
        precision: Option<u32>,
        scale: Option<u32>,
    },

    // Character types
    Varchar(Option<u32>),
    Char(Option<u32>),
    Text,
    /// MySQL MEDIUMTEXT
    MediumText,
    /// MySQL LONGTEXT
    LongText,

    // Boolean
    Boolean,

    // Temporal types
    Date,
    Time,
    /// Timestamp with optional fractional seconds precision.
    Timestamp {
        with_timezone: bool,
        precision: Option<u32>,
    },
    /// Date+time without timezone (MySQL DATETIME).
    DateTime {
        precision: Option<u32>,
    },
    /// Postgres INTERVAL type.
    Interval,

    // UUID
    Uuid,

    // JSON
    Json,
    Jsonb,

    // Binary types
    Bytea,
    Binary(Option<u32>),
    VarBinary(Option<u32>),
    Blob,
    MediumBlob,
    LongBlob,

    // Network types (Postgres)
    Inet,
    Cidr,
    MacAddr,

    // Enum — named (Postgres) or inline (MySQL)
    Enum {
        /// Name of the enum type (Postgres CREATE TYPE name, or None for MySQL inline).
        name: Option<String>,
        /// Enum values (always populated for MySQL inline; may be empty for
        /// Postgres named enums where values are in Schema::enums).
        values: Vec<String>,
    },

    // MySQL SET type
    Set(Vec<String>),

    // Array type (Postgres)
    Array(Box<DataType>),

    // MySQL small integers
    TinyInt,
    MediumInt,

    // MySQL YEAR type
    Year,

    /// Fallback for unrecognized types.
    Other(String),
}

impl DataType {
    /// Returns true if this is a text/string type.
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            DataType::Text
                | DataType::MediumText
                | DataType::LongText
                | DataType::Varchar(_)
                | DataType::Char(_)
        )
    }

    /// Returns true if this is a boolean type.
    pub fn is_boolean(&self) -> bool {
        matches!(self, DataType::Boolean)
    }

    /// Returns true if this is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            DataType::SmallInt
                | DataType::Integer
                | DataType::BigInt
                | DataType::TinyInt
                | DataType::MediumInt
        )
    }

    /// Returns true if this is a JSON/JSONB type.
    pub fn is_json(&self) -> bool {
        matches!(self, DataType::Json | DataType::Jsonb)
    }

    /// Returns true if this is a binary/blob type.
    pub fn is_binary(&self) -> bool {
        matches!(
            self,
            DataType::Bytea
                | DataType::Binary(_)
                | DataType::VarBinary(_)
                | DataType::Blob
                | DataType::MediumBlob
                | DataType::LongBlob
        )
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::SmallInt => write!(f, "smallint"),
            DataType::Integer => write!(f, "integer"),
            DataType::BigInt => write!(f, "bigint"),
            DataType::TinyInt => write!(f, "tinyint"),
            DataType::MediumInt => write!(f, "mediumint"),
            DataType::Real => write!(f, "real"),
            DataType::DoublePrecision => write!(f, "double precision"),
            DataType::Numeric { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => write!(f, "numeric({p},{s})"),
                (Some(p), None) => write!(f, "numeric({p})"),
                _ => write!(f, "numeric"),
            },
            DataType::Varchar(len) => match len {
                Some(n) => write!(f, "varchar({n})"),
                None => write!(f, "varchar"),
            },
            DataType::Char(len) => match len {
                Some(n) => write!(f, "char({n})"),
                None => write!(f, "char"),
            },
            DataType::Text => write!(f, "text"),
            DataType::MediumText => write!(f, "mediumtext"),
            DataType::LongText => write!(f, "longtext"),
            DataType::Boolean => write!(f, "boolean"),
            DataType::Date => write!(f, "date"),
            DataType::Time => write!(f, "time"),
            DataType::Timestamp {
                with_timezone,
                precision,
            } => {
                write!(f, "timestamp")?;
                if let Some(p) = precision {
                    write!(f, "({p})")?;
                }
                if *with_timezone {
                    write!(f, " with time zone")?;
                }
                Ok(())
            }
            DataType::DateTime { precision } => match precision {
                Some(p) => write!(f, "datetime({p})"),
                None => write!(f, "datetime"),
            },
            DataType::Interval => write!(f, "interval"),
            DataType::Uuid => write!(f, "uuid"),
            DataType::Json => write!(f, "json"),
            DataType::Jsonb => write!(f, "jsonb"),
            DataType::Bytea => write!(f, "bytea"),
            DataType::Binary(len) => match len {
                Some(n) => write!(f, "binary({n})"),
                None => write!(f, "binary"),
            },
            DataType::VarBinary(len) => match len {
                Some(n) => write!(f, "varbinary({n})"),
                None => write!(f, "varbinary"),
            },
            DataType::Blob => write!(f, "blob"),
            DataType::MediumBlob => write!(f, "mediumblob"),
            DataType::LongBlob => write!(f, "longblob"),
            DataType::Inet => write!(f, "inet"),
            DataType::Cidr => write!(f, "cidr"),
            DataType::MacAddr => write!(f, "macaddr"),
            DataType::Enum { name, .. } => match name {
                Some(n) => write!(f, "{n}"),
                None => write!(f, "enum"),
            },
            DataType::Set(values) => write!(f, "set({})", values.join(",")),
            DataType::Array(inner) => write!(f, "{inner}[]"),
            DataType::Year => write!(f, "year"),
            DataType::Other(s) => write!(f, "{s}"),
        }
    }
}

/// Primary key constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PrimaryKey {
    pub name: Option<String>,
    /// Column names in the primary key (single or composite).
    pub columns: Vec<String>,
}

/// Foreign key constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ForeignKey {
    pub name: Option<String>,
    /// Columns in the referencing (child) table.
    pub columns: Vec<String>,
    /// Referenced table name.
    pub referenced_table: String,
    /// Referenced schema (if cross-schema reference).
    pub referenced_schema: Option<String>,
    /// Columns in the referenced (parent) table.
    pub referenced_columns: Vec<String>,
    pub on_delete: ForeignKeyAction,
    pub on_update: ForeignKeyAction,
}

/// Foreign key referential action.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ForeignKeyAction {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl std::fmt::Display for ForeignKeyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForeignKeyAction::NoAction => write!(f, "NO ACTION"),
            ForeignKeyAction::Restrict => write!(f, "RESTRICT"),
            ForeignKeyAction::Cascade => write!(f, "CASCADE"),
            ForeignKeyAction::SetNull => write!(f, "SET NULL"),
            ForeignKeyAction::SetDefault => write!(f, "SET DEFAULT"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Index {
    pub name: Option<String>,
    pub columns: Vec<String>,
    pub unique: bool,
    /// Index type (btree, hash, gin, gist, etc.).
    pub index_type: Option<String>,
    /// Partial index predicate (Postgres WHERE clause).
    pub predicate: Option<String>,
}

/// CHECK constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CheckConstraint {
    pub name: Option<String>,
    /// The SQL expression for the check.
    pub expression: String,
}

/// UNIQUE constraint (table-level, distinct from unique indexes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct UniqueConstraint {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

/// Table-level options (primarily for MySQL).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableOptions {
    /// MySQL storage engine (InnoDB, MyISAM, etc.).
    pub engine: Option<String>,
    /// MySQL default charset.
    pub charset: Option<String>,
    /// MySQL default collation.
    pub collation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_new() {
        let schema = Schema::new("public");
        assert_eq!(schema.name, "public");
        assert!(schema.tables.is_empty());
        assert!(schema.enums.is_empty());
    }

    #[test]
    fn test_table_new() {
        let table = Table::new("users");
        assert_eq!(table.name, "users");
        assert!(table.columns.is_empty());
        assert!(table.primary_key.is_none());
    }

    #[test]
    fn test_data_type_display() {
        assert_eq!(DataType::SmallInt.to_string(), "smallint");
        assert_eq!(DataType::Varchar(Some(255)).to_string(), "varchar(255)");
        assert_eq!(
            DataType::Numeric {
                precision: Some(10),
                scale: Some(2)
            }
            .to_string(),
            "numeric(10,2)"
        );
        assert_eq!(
            DataType::Timestamp {
                with_timezone: true,
                precision: None
            }
            .to_string(),
            "timestamp with time zone"
        );
        assert_eq!(
            DataType::Array(Box::new(DataType::Text)).to_string(),
            "text[]"
        );
    }

    #[test]
    fn test_data_type_classification() {
        assert!(DataType::Text.is_text());
        assert!(DataType::Varchar(Some(100)).is_text());
        assert!(!DataType::Integer.is_text());

        assert!(DataType::Integer.is_integer());
        assert!(DataType::BigInt.is_integer());
        assert!(DataType::TinyInt.is_integer());
        assert!(!DataType::Text.is_integer());

        assert!(DataType::Boolean.is_boolean());
        assert!(!DataType::Integer.is_boolean());

        assert!(DataType::Json.is_json());
        assert!(DataType::Jsonb.is_json());

        assert!(DataType::Bytea.is_binary());
        assert!(DataType::Binary(Some(32)).is_binary());
    }

    #[test]
    fn test_foreign_key_action_default() {
        assert_eq!(ForeignKeyAction::default(), ForeignKeyAction::NoAction);
    }

    #[test]
    fn test_foreign_key_action_display() {
        assert_eq!(ForeignKeyAction::Cascade.to_string(), "CASCADE");
        assert_eq!(ForeignKeyAction::SetNull.to_string(), "SET NULL");
    }

    #[test]
    fn test_schema_serialization_roundtrip() {
        let schema = Schema {
            name: "public".to_string(),
            enums: vec![EnumType {
                name: "user_role".to_string(),
                values: vec!["admin".to_string(), "user".to_string()],
            }],
            tables: vec![Table {
                name: "users".to_string(),
                columns: vec![Column {
                    name: "id".to_string(),
                    data_type: DataType::BigInt,
                    nullable: false,
                    default: None,
                    auto_increment: true,
                    unsigned: false,
                }],
                primary_key: Some(PrimaryKey {
                    name: None,
                    columns: vec!["id".to_string()],
                }),
                foreign_keys: Vec::new(),
                indexes: Vec::new(),
                check_constraints: Vec::new(),
                unique_constraints: Vec::new(),
                options: TableOptions::default(),
            }],
        };

        let json = serde_json::to_string(&schema).unwrap();
        let deserialized: Schema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, deserialized);
    }
}
