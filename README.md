# dbhelper

A database linter, schema diff, and optimization tool. Supports Postgres and MySQL.

dbhelper introspects live databases or parses SQL migration files (from tools like [drizzle-kit](https://orm.drizzle.team/kit-docs/overview) and [Alembic](https://alembic.sqlalchemy.org/)), then provides:

- **Schema diffing** — compare two schemas and produce a structured diff
- **Linting** — catch common schema mistakes and anti-patterns
- **Optimization suggestions** — recommend indexes, type improvements, and structural changes

## Status

Early development. The project structure and integration generators are in place; core logic is being built out. See [open issues](https://github.com/Pseudo-Development/dbhelper/issues) for the roadmap.

## Quick Start

```bash
# Build
cargo build

# Run the CLI
cargo run -- --help

# Available subcommands
cargo run -- diff <from> <to>      # Diff two schemas
cargo run -- lint <target>         # Lint a schema
cargo run -- optimize <target>     # Get optimization suggestions
```

## Architecture

```
dbhelper/
├── crates/
│   ├── dbhelper-core/           # Schema types, diff engine, lint rules, optimization
│   ├── dbhelper-postgres/       # Postgres introspection via sqlx
│   ├── dbhelper-mysql/          # MySQL introspection via sqlx
│   ├── dbhelper-container/      # Ephemeral test DB containers (testcontainers)
│   └── dbhelper-cli/            # CLI binary (clap)
├── integrations/
│   ├── drizzle/                 # Drizzle-kit schema definitions → SQL fixtures
│   └── sqlalchemy/              # SQLAlchemy/Alembic models → SQL fixtures
└── tests/fixtures/              # SQL migration fixtures for testing
```

### Crates

| Crate | Purpose | Key deps |
|-------|---------|----------|
| `dbhelper-core` | DB-agnostic schema representation, diffing, linting, optimization | serde, thiserror |
| `dbhelper-postgres` | Introspect Postgres `information_schema` / `pg_catalog` | sqlx (postgres) |
| `dbhelper-mysql` | Introspect MySQL `information_schema` | sqlx (mysql) |
| `dbhelper-container` | Spin up ephemeral Postgres/MySQL containers for testing | testcontainers |
| `dbhelper-cli` | CLI interface wiring everything together | clap, tokio |

### Integration Generators

The `integrations/` directory contains non-Rust projects that define comprehensive database schemas using popular ORMs. These generate SQL migration files used as test fixtures, and they define the full feature set dbhelper must support.

**Drizzle** (TypeScript):
```bash
cd integrations/drizzle
npm install
npm run generate        # generates migrations/postgres/ and migrations/mysql/
```

**SQLAlchemy** (Python):
```bash
cd integrations/sqlalchemy
pip install -e .
alembic revision --autogenerate -m "initial"
alembic upgrade --sql head    # emit SQL without running it
```

## Supported Features

| Feature | Postgres | MySQL |
|---------|----------|-------|
| Column types (20+) | smallint through macaddr | tinyint through varbinary |
| Primary keys (single + composite) | Yes | Yes |
| Foreign keys (simple + composite) | Yes | Yes |
| Self-referencing FKs | Yes | Yes |
| FK actions (cascade, set null, restrict) | Yes | Yes |
| Unique indexes (single + composite) | Yes | Yes |
| Partial/filtered indexes | Yes | N/A |
| Check constraints | Yes | Yes (MySQL 8.0+) |
| Named enums | `CREATE TYPE` | Inline `ENUM(...)` |
| JSON/JSONB | Yes | JSON |
| Named schemas | Yes (`analytics`) | N/A |
| Unsigned integers | N/A | Yes |
| `ON UPDATE CURRENT_TIMESTAMP` | N/A | Yes |
| Binary types | bytea | `BINARY`/`VARBINARY` |

## Development

```bash
cargo build              # Build all crates
cargo test               # Run all tests
cargo clippy             # Lint
cargo test -p <crate>    # Test a single crate
```

## License

MIT
