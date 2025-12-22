#!/bin/bash
# Count incomplete tasks in todo.md

TODO_FILE="${1:-.claude/todo.md}"

if [[ ! -f "$TODO_FILE" ]]; then
    echo "0"
    exit 0
fi

count=$(grep -c '^\s*- \[ \]' "$TODO_FILE" || echo "0")
echo "$count"
