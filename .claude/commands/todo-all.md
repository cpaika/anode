# Process All Todo Tasks

Process all incomplete tasks from the queue autonomously.

## Instructions

Follow this loop until all tasks are complete:

### Step 1: Check for tasks
Run `.claude/scripts/todo-get.sh` to get the next incomplete task.

### Step 2: If no task, stop
If the script returns empty, announce:
> All tasks complete! Processed N tasks total.

Then stop.

### Step 3: Process the task using a subagent
Use the Task tool to launch an agent that will:
- Implement the task completely
- Run any necessary tests/builds
- Mark the task complete using `.claude/scripts/todo-complete.sh`
- Commit changes if appropriate

**CRITICAL**: Use the Task tool with a detailed prompt. This keeps verbose output (file edits, test output, build logs) in the subagent's context, preserving the main conversation's token budget.

Example Task tool invocation:
```
subagent_type: "general-purpose"
prompt: "Process this todo task: [TASK].
1. Implement what's needed
2. Test/build if applicable
3. Run: .claude/scripts/todo-complete.sh \"[TASK]\"
4. Commit with a descriptive message
5. Return a 1-2 sentence summary"
```

### Step 4: Report progress
After the subagent returns, briefly report:
> Completed: [task] - [summary from agent]
> Tasks remaining: N

### Step 5: Repeat
Go back to Step 1.

## Important Notes

- **Context efficiency**: Each task runs in a subagent to keep the main context clean
- **User can edit**: The user may add/reorder tasks in `.claude/todo.md` while you work
- **Failure handling**: If a task fails, report it and continue to the next task
- **Progress visibility**: Always report after each task so the user can monitor
