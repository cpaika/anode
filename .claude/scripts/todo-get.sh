#!/bin/bash
# Get the next incomplete task from todo.md
# Returns the task text (without the checkbox) or empty if no tasks

TODO_FILE="${1:-.claude/todo.md}"

if [[ ! -f "$TODO_FILE" ]]; then
    echo ""
    exit 0
fi

# Find the first line matching "- [ ]" and extract the task text
next_task=$(grep -m 1 '^\s*- \[ \]' "$TODO_FILE" | sed 's/^\s*- \[ \] //')

echo "$next_task"
