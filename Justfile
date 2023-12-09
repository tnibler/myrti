sqlx:
  cd server/core && \
  cargo sqlx database drop -y && \
  cargo sqlx database create && \
  cargo sqlx migrate run

openapi:
  cd server && \
  cargo run --bin print-openapi > openapi.json && \
  cd ../web && pnpm run openapi

openapi-check:
  diff web/openapi.json <(cd server && cargo run --bin print-openapi) || echo "MISMATCH"

