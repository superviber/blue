# Blue Task Sync

When you call `blue_rfc_get`, it returns a `claude_code_tasks` array with pre-formatted tasks ready for TaskCreate. Each task includes:
- 💙 prefix in subject (indicates Blue-synced task)
- `blue_rfc` and `blue_task_index` in metadata

Create these tasks to track RFC work in Claude Code's task UI.

When you mark a task complete that has `blue_rfc` metadata, call `blue_rfc_task_complete` with:
- `title`: the RFC title from metadata
- `task_index`: the task index from metadata

This updates the `.plan.md` file automatically.
