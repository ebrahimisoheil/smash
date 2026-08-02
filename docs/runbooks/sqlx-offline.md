# SQLx offline boundary

The B3 repository foundation uses runtime `sqlx::query` calls while the
canonical schema is still settling. This keeps `cargo test --workspace
--locked` and the container build independent of a live database today.

When a query is promoted to a SQLx compile-time macro, generate and commit its
`.sqlx` metadata from the PostgreSQL migration before enabling `SQLX_OFFLINE`
for that query. The migration directory remains the schema source of truth;
the offline cache is generated evidence, not a second schema.
