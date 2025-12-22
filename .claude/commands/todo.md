# Process One Todo Task

Process the next incomplete task from the todo list.

## Instructions

1. **Get the next task** by running: `.claude/scripts/todo-get.sh`

2. **If no task is returned**, report "No incomplete tasks in the queue" and stop.

3. **If a task is returned**, announce what you're working on:
   > Working on: [task description]

4. **Implement the task** completely:
   - Understand what needs to be done
   - Make the necessary code changes
   - Run tests if applicable
   - Ensure the build passes

5. **When complete**, mark it done by running:
   ```bash
   .claude/scripts/todo-complete.sh "exact task text"
   ```

6. **Report completion** with a brief summary of what was done.

## Important Notes

- Focus on ONE task only - don't process multiple tasks
- If a task is unclear, make reasonable assumptions and note them
- If a task fails or is blocked, report why instead of marking it complete
- Commit your changes with a descriptive message when appropriate
