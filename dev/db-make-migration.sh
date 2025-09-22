#!/usr/bin/env bash
if [ -z "$1" ]; then
  echo "Usage: $0 <migration_name>"
  exit 1
fi

DIR_SELF="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
MIGRATIONS="$DIR_SELF/../src/store/sql/migrations"
MIGRATION_NAME=$1
MIGRATION_ID=$(ls $MIGRATIONS | sort -r | head -n 1 | cut -d':' -f1)
MIGRATION_ID=$((MIGRATION_ID + 1))

mkdir -p $MIGRATIONS/$MIGRATION_ID:$MIGRATION_NAME
touch $MIGRATIONS/$MIGRATION_ID:$MIGRATION_NAME/up.sql
touch $MIGRATIONS/$MIGRATION_ID:$MIGRATION_NAME/down.sql
echo "Created migration files @ $MIGRATIONS/$MIGRATION_ID:$MIGRATION_NAME/"
