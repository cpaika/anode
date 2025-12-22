#!/bin/bash
# Add a new task to todo.md
# Usage: todo-add.sh "task description"

TODO_FILE=".claude/todo.md"
TASK="$1"

if [[ -z "$TASK" ]]; then
    echo "Usage: todo-add.sh \"task description\""
    exit 1
fi

if [[ ! -f "$TODO_FILE" ]]; then
    # Create the file with basic structure
    cat > "$TODO_FILE" << 'EOF'
# Task Queue

## Tasks

## Completed
EOF
fi

# Find the "## Tasks" section and add the task after it
# If there are existing tasks, add after the last one
if grep -q '^\s*- \[ \]' "$TODO_FILE"; then
    # Add after the last incomplete task
    last_task_line=$(grep -n '^\s*- \[ \]' "$TODO_FILE" | tail -1 | cut -d: -f1)
    sed -i '' "${last_task_line}a\\
- [ ] $TASK
" "$TODO_FILE"
else
    # No existing tasks, add after "## Tasks" line
    sed -i '' "/^## Tasks/a\\
\\
- [ ] $TASK" "$TODO_FILE"
fi

echo "Added: $TASK"
