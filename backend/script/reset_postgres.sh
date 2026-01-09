#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  backend/script/reset_postgres.sh [options]

Options:
  --container <name>   Postgres container name (default: anicargo-postgres)
  --db <name>          Database name (default: anicargo)
  --user <name>        Database user (default: anicargo)
  --help               Show help

This drops and recreates the public schema, deleting all data.
EOF
}

CONTAINER="anicargo-postgres"
DB_NAME="anicargo"
DB_USER="anicargo"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --container)
      CONTAINER="$2"
      shift 2
      ;;
    --db)
      DB_NAME="$2"
      shift 2
      ;;
    --user)
      DB_USER="$2"
      shift 2
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

docker_cmd=(docker)
if ! docker ps >/dev/null 2>&1; then
  if command -v sudo >/dev/null 2>&1; then
    docker_cmd=(sudo docker)
  else
    echo "docker is not accessible; try running as root or install sudo." >&2
    exit 1
  fi
fi

"${docker_cmd[@]}" exec -i "$CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 <<'SQL'
DROP SCHEMA public CASCADE;
CREATE SCHEMA public;
GRANT ALL ON SCHEMA public TO public;
SQL

echo "database reset completed"
