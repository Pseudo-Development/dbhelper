# dbhelper

Database linter, diff management, and optimization tool.

## Build Commands

```bash
cargo build                    # Build all crates
cargo test                     # Run all tests
cargo clippy                   # Lint all crates
cargo test -p dbhelper-core    # Test a single crate
cargo run -- --help            # Run the CLI
```

## Architecture

Cargo workspace with five crates:

- **dbhelper-core** (`crates/dbhelper-core/`) — DB-agnostic schema representation, diff engine, lint rules, optimization suggestions
  - `schema/` — Core type system (tables, columns, indexes, constraints, enums)
  - `diff/` — Schema diffing engine
  - `lint/` — Linting rules engine
  - `optimize/` — Schema optimization suggestions
- **dbhelper-postgres** (`crates/dbhelper-postgres/`) — Postgres introspection and schema parsing (uses sqlx)
- **dbhelper-mysql** (`crates/dbhelper-mysql/`) — MySQL introspection and schema parsing (uses sqlx)
- **dbhelper-container** (`crates/dbhelper-container/`) — Docker container management for ephemeral test databases (uses testcontainers)
- **dbhelper-cli** (`crates/dbhelper-cli/`) — CLI binary (uses clap). Binary name: `dbhelper`

## Integration Sub-packages

Under `integrations/`, non-Rust projects that generate SQL migration fixtures:

- **drizzle/** — Node/TS project with comprehensive Postgres and MySQL schema definitions
  - `src/schema/postgres.ts` — 14 tables, 3 pgEnums, named schema (`analytics`)
  - `src/schema/mysql.ts` — 14 tables with MySQL-specific types and inline enums
  - Generate migrations: `cd integrations/drizzle && npm install && npm run generate`
- **sqlalchemy/** — Python project with SQLAlchemy models and Alembic migration support
  - `models/postgres.py` — 14 PG model classes (JSONB, INET, UUID, Interval, partial indexes)
  - `models/mysql.py` — 14 MySQL model classes (unsigned ints, ENUM, MEDIUMTEXT, BINARY, fsp timestamps)
  - Generate migrations: `cd integrations/sqlalchemy && pip install -e . && alembic revision --autogenerate`

These produce SQL migration files consumed by the Rust test suite in `tests/fixtures/`.

## Database Support

| Database | Crate             | Status |
|----------|--------------------|--------|
| Postgres | dbhelper-postgres  | Stub   |
| MySQL    | dbhelper-mysql     | Stub   |

## Feature Coverage

The integration generators define the full feature set dbhelper must support:

- **Column types**: 20+ per dialect (integers, floats, numeric, varchar/char/text variants, boolean, date/time/timestamp, interval, UUID, JSON/JSONB, binary/blob, inet/cidr/macaddr, enum, set, array)
- **Primary keys**: single-column and composite
- **Foreign keys**: simple and composite, cascade/set-null/restrict actions, self-referencing
- **Indexes**: unique, composite, partial (PG `WHERE` clause)
- **Constraints**: check, unique (single and multi-column)
- **Defaults**: static values and SQL expressions (`gen_random_uuid()`, `now()`, `UUID()`, `CURRENT_TIMESTAMP`)
- **Enums**: PG named enums (`pgEnum` / `CREATE TYPE`) and MySQL inline `ENUM(...)`
- **Schemas**: PG named schemas (e.g. `analytics`)
- **MySQL-specific**: unsigned integers, `ON UPDATE CURRENT_TIMESTAMP`, `BINARY`/`VARBINARY`, engine/charset options

## Conventions

- All async code uses tokio runtime
- Schema types live in dbhelper-core and are DB-agnostic
- Dialect crates convert DB-specific schemas into core types
- Use `thiserror` for error types
- Use `serde` derive for all schema types
