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

- **dbhelper-core** — DB-agnostic schema representation, diff engine, lint rules, optimization suggestions
- **dbhelper-postgres** — Postgres introspection and schema parsing (uses sqlx)
- **dbhelper-mysql** — MySQL introspection and schema parsing (uses sqlx)
- **dbhelper-container** — Docker container management for ephemeral test databases (uses testcontainers)
- **dbhelper-cli** — CLI binary (uses clap)

## Integration Sub-packages

Under `integrations/`:

- **drizzle/** — Node/TS project using drizzle-kit to generate Postgres/MySQL migration fixtures
- **sqlalchemy/** — Python project using Alembic to generate migration fixtures

These produce SQL migration files consumed by the Rust test suite in `tests/fixtures/`.

## Database Support

| Database | Crate              | Status |
|----------|-------------------|--------|
| Postgres | dbhelper-postgres | Stub   |
| MySQL    | dbhelper-mysql    | Stub   |

## Conventions

- All async code uses tokio runtime
- Schema types live in dbhelper-core and are DB-agnostic
- Dialect crates convert DB-specific schemas into core types
- Use `thiserror` for error types
