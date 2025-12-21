#!/usr/bin/env bash
# Installation and runtime check for Conventional Commits compliance.
# It enforces the usage of specific scopes and a few style rules.
set -euo pipefail

HOOK_PATH="$(git rev-parse --git-dir)/hooks/commit-msg"

if [ $# -eq 0 ]; then
  echo "Installing commit-msg hook to $HOOK_PATH..."
  mkdir -p "$(dirname "$HOOK_PATH")"
  cp "$0" "$HOOK_PATH"
  chmod +x "$HOOK_PATH"
  echo "Installed!"
  exit 0
fi

COMMIT_MSG=$(cat "$1")

ALLOWED_TYPES="build,chore,ci,docs,feat,fix,perf,refactor,revert,style,test" # from @commitlint/config-conventional
ALLOWED_SCOPES="db,dev,log,ipc,osa,service,dispatch,discord,lastfm,brainz,musicdb,mzstatic,utf16"
PREFIX_REGEX="^([^)]+)(\(([^)]+)\))?!?: ." # type(scope)!:

if ! [[ $COMMIT_MSG =~ $PREFIX_REGEX ]]; then
  echo "Please use the Conventional Commits format for commit messages; see: https://www.conventionalcommits.org/en/v1.0.0/"
  exit 1
fi

TYPE="${BASH_REMATCH[1]}"
SCOPES="${BASH_REMATCH[3]:-}"
SUBJECT_AND_BODY="${COMMIT_MSG#${BASH_REMATCH[0]}}"

if [[ ! $SUBJECT_AND_BODY =~ ^[a-z] ]]; then
  echo "The first letter of the subject must be lowercase."
  exit 1
fi

if ! printf "%s" "$SUBJECT_AND_BODY" | perl -0777 -ne "exit 0 if /^[^\n]+(\n\n\S[^\0]+)?$/; exit 1"; then # perl needed for newline matching
  echo "An empty line must separate the subject from the body, and the body must be non-empty if present."
  exit 1
fi

if ! echo "$ALLOWED_TYPES" | tr ',' '\n' | grep -qx "$TYPE"; then
  echo "Invalid type '$TYPE'; must be one of: $ALLOWED_TYPES"
  exit 1
fi

if [ -n "$SCOPES" ]; then
  IFS=',' read -ra SCOPE_ARRAY <<< "$SCOPES"
  for SCOPE in "${SCOPE_ARRAY[@]}"; do
    SCOPE=$(echo "$SCOPE" | xargs) # trimmed
    if ! echo "$ALLOWED_SCOPES" | tr ',' '\n' | grep -qx "$SCOPE"; then
      echo "Invalid scope '$SCOPE'; must be one of: $ALLOWED_SCOPES"
      exit 1
    fi
  done
fi

exit 0
