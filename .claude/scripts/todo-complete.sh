#!/bin/bash
# Mark a task as complete in todo.md
# Usage: todo-complete.sh "task text"

TODO_FILE=".claude/todo.md"
TASK="$1"

if [[ -z "$TASK" ]]; then
    echo "Usage: todo-complete.sh \"task text\""
    exit 1
fi

if [[ ! -f "$TODO_FILE" ]]; then
    echo "Error: $TODO_FILE not found"
    exit 1
fi

# Escape special regex characters in the task
ESCAPED_TASK=$(printf '%s\n' "$TASK" | sed 's/[[\.*^$()+?{|]/\\&/g')

# Replace "- [ ] task" with "- [x] task"
if grep -q "^\s*- \[ \] $ESCAPED_TASK" "$TODO_FILE"; then
    sed -i '' "s/^\(\s*\)- \[ \] $ESCAPED_TASK/\1- [x] $ESCAPED_TASK/" "$TODO_FILE"
    echo "Marked complete: $TASK"
else
    echo "Task not found: $TASK"
    exit 1
fi
