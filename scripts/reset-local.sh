#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
docker compose --env-file .env down -v
echo "Local PostgreSQL and MinIO volumes removed. This reset is destructive."
