#!/usr/bin/env bash
TIME_PREFIX=$(date +%s | xargs printf "%X")

if [ -z "$1" ]; then
  echo "Usage: $0 <migration_name>"
  exit 1
fi

DIR_SELF="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
MIGRATIONS="$DIR_SELF/../src/store/sql/migrations"
MIGRATION_NAME=$1

mkdir -p $MIGRATIONS/$TIME_PREFIX-$MIGRATION_NAME
touch $MIGRATIONS/$TIME_PREFIX-$MIGRATION_NAME/up.sql
touch $MIGRATIONS/$TIME_PREFIX-$MIGRATION_NAME/down.sql
echo "Created migration files @ $MIGRATIONS/$TIME_PREFIX-$MIGRATION_NAME/"
