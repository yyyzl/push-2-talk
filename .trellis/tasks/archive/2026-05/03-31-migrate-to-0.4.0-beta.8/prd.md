# Migration Task: Upgrade to v0.4.0-beta.8

**Created**: 2026-03-31
**From Version**: 0.3.0-rc.3
**To Version**: 0.4.0-beta.8
**Assignee**: name=yyyzl
initialized_at=2026-02-15T14:38:39.142942

## Status

- [ ] Review migration guide
- [ ] Update custom files
- [ ] Run `trellis update --migrate`
- [ ] Test workflows

---

## v0.3.0 Migration Guide

# Migration Guide: v0.2.x → v0.3.0

## Overview

All shell scripts (.sh) have been replaced by Python scripts (.py). This affects any custom workflows, slash commands, or documentation that references the old scripts.

## Requirements

- Python 3.10 or higher

## What Changed

### Script Extensions

| Old Path | New Path |
|----------|----------|
| `.trellis/scripts/*.sh` | `.trellis/scripts/*.py` |
| `.trellis/scripts/common/*.sh` | `.trellis/scripts/common/*.py` |
| `.trellis/scripts/multi-agent/*.sh` | `.trellis/scripts/multi_agent/*.py` |

### Directory Rename

| Old | New |
|-----|-----|
| `multi-agent/` (hyphen) | `multi_agent/` (underscore) |

### Invocation Change

| Old | New |
|-----|-----|
| `./script.sh` | `python3 ./script.py` |
| `./.trellis/scripts/task.sh` | `python3 ./.trellis/scripts/task.py` |

## After Migration

Run `trellis update --migrate` to archive the old shell scripts to `.trellis/scripts-shell-archive/`.

### AI Assistant Instructions

When helping with this migration:

When assisting users upgrading from 0.2.x to 0.3.0:
1. Search for files containing `.trellis/scripts/*.sh` references and update to `.py`
2. Replace `multi-agent/` directory references with `multi_agent/` (underscore)
3. Always use `python3` explicitly, not just `python` (Windows compatibility)
4. Check ALL locations: docstrings, help text, error messages, markdown docs
5. Run `trellis update --migrate` to remove legacy shell scripts

Note: Users upgrading from any 0.3.0 prerelease (beta/rc) need no file changes — only the version stamp is updated.

---

## v0.4.0-beta.1 Migration Guide

## Command Consolidation

The following commands have been merged:

| Old (removed) | New (replacement) |
|---------------|-------------------|
| `before-backend-dev` | `before-dev` |
| `before-frontend-dev` | `before-dev` |
| `check-backend` | `check` |
| `check-frontend` | `check` |

### If you never customized these commands

No action needed. `trellis update` will:
1. Auto-delete the old files (hash-verified, only if unmodified)
2. Write the new unified `before-dev` and `check` commands

### If you customized the old commands

The old files will NOT be deleted (hash mismatch protection). You need to:
1. Open the new `before-dev` / `check` command file
2. Merge your customizations from the old backend/frontend variants into the unified file
3. Delete the old files manually once merged

### Affected paths per platform

- **Claude**: `.claude/commands/trellis/`
- **Cursor**: `.cursor/commands/`
- **iFlow**: `.iflow/commands/trellis/`
- **OpenCode**: `.opencode/commands/trellis/`
- **Gemini**: `.gemini/commands/trellis/`
- **Kilo**: `.kilocode/workflows/`
- **Codex**: `.agents/skills/`
- **Kiro**: `.kiro/skills/`
- **Qoder**: `.qoder/skills/`

## Python Scripts Refactoring

All `.trellis/scripts/` files have been restructured internally. **No action needed** — all entry paths (`python3 .trellis/scripts/task.py`, etc.) remain unchanged. `trellis update` will sync the new scripts automatically.

### AI Assistant Instructions

When helping with this migration:

When helping users migrate to v0.4.0-beta.1:

1. **Check for customized old commands**: Run `git diff` or compare file hashes to see if the user modified before-backend-dev, before-frontend-dev, check-backend, or check-frontend files.
2. **If customized**: Help merge their customizations into the new unified `before-dev` and `check` files. The new files use `python3 ./.trellis/scripts/get_context.py --mode packages` to auto-detect which specs to load, replacing the hardcoded backend/frontend split.
3. **If not customized**: Just run `trellis update` — safe-file-delete will handle cleanup automatically.
4. **Python scripts**: No user action needed. The refactoring preserves all entry paths. If the user has custom scripts that import from `.trellis/scripts/common/`, they may need to update imports (e.g., `from common.io import read_json` instead of inline `_read_json_file`).
