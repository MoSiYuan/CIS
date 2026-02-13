# CIS Migration Guide: TOML to SQLite

**Version**: v1.1.6
**Last Updated**: 2026-02-13
**Target Audience**: CIS users upgrading from v1.1.x to v1.1.6

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Understanding the Migration](#understanding-the-migration)
3. [Pre-Migration Checklist](#pre-migration-checklist)
4. [Migration Process](#migration-process)
5. [Post-Migration Steps](#post-migration-steps)
6. [Verifying Migration](#verifying-migration)
7. [Handling Common Issues](#handling-common-issues)
8. [Rollback Procedures](#rollback-procedures)
9. [Best Practices](#best-practices)
10. [FAQ](#faq)

---

## Prerequisites

### System Requirements

- **CIS Version**: v1.1.5 or later (for TOML format)
- **Target Version**: v1.1.6 or later (for SQLite format)
- **Disk Space**: At least 2x the size of current TOML files
- **Database**: SQLite 3.35 or later (included with CIS v1.1.6)
- **Backup Space**: Sufficient space for backups

### Required Tools

```bash
# Verify prerequisites
cis --version  # Should be 1.1.6+
sqlite3 --version  # Should be 3.35+
toml --version  # For TOML validation (optional)
```

### Knowledge Requirements

- Basic command-line operations
- Understanding of file system paths
- Familiarity with TOML format (helpful but not required)
- Database concepts (helpful but not required)

---

## Understanding the Migration

### Why Migrate?

CIS v1.1.6 introduces a fundamental shift from file-based TOML storage to a SQLite database:

| Aspect | TOML Format (v1.1.x) | SQLite Format (v1.1.6+) |
|--------|----------------------|------------------------|
| **Storage** | Text files | Binary database |
| **Querying** | File parsing | SQL queries |
| **Performance** | Slower for large datasets | Faster with indexing |
| **Concurrency** | File locking | Database transactions |
| **Scalability** | Limited | Highly scalable |
| **Integrity** | Manual validation | ACID guarantees |
| **Backup** | File copy | Database dump |

### What Gets Migrated?

The migration process transfers:

- ✅ **Task Definitions**: All task attributes, dependencies, metadata
- ✅ **Team Configurations**: Team definitions, capabilities, members
- ✅ **Task Status**: Current state of all tasks
- ✅ **Context Variables**: Prompt templates and context data
- ✅ **Metadata**: Custom fields and attributes

### What Doesn't Get Migrated?

- ❌ **Execution Logs**: Retained in original locations
- ❌ **Temporary Files**: Cache and scratch data
- ❌ **Archived Tasks**: Tasks marked as archived (configurable)
- ❌ **Skill Data**: Skill definitions (managed separately)

### Migration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Migration Process                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐      ┌──────────────┐                  │
│  │  TOML Files  │─────▶│   Parser     │                  │
│  │  (Source)    │      │  (Rust)      │                  │
│  └──────────────┘      └──────┬───────┘                  │
│                              │                            │
│                              ▼                            │
│                     ┌─────────────────┐                   │
│                     │ Migration Tool  │                   │
│                     │   (Validator)   │                   │
│                     └────────┬────────┘                   │
│                              │                            │
│                              ▼                            │
│                     ┌─────────────────┐                   │
│                     │ Task Service    │                   │
│                     │ (Transformer)   │                   │
│                     └────────┬────────┘                   │
│                              │                            │
│                              ▼                            │
│                     ┌─────────────────┐                   │
│                     │ SQLite Database │                   │
│                     │  (Dest)        │                   │
│                     └─────────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Pre-Migration Checklist

### 1. Backup Current Data

```bash
# Create backup directory
BACKUP_DIR="$HOME/cis-backup-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# Backup TOML files
cp -r ~/.cis/plan "$BACKUP_DIR/"
cp -r ~/.cis/tasks "$BACKUP_DIR/" 2>/dev/null || true

# Backup existing database (if any)
cp ~/.cis/data/tasks.db "$BACKUP_DIR/" 2>/dev/null || true

# Verify backup
ls -lh "$BACKUP_DIR"
echo "Backup created at: $BACKUP_DIR"
```

### 2. Validate TOML Files

```bash
# Check TOML syntax
find ~/.cis/plan -name "*.toml" -exec toml check {} \; 2>&1

# Alternative: Use Python
python3 <<EOF
import toml
import sys
from pathlib import Path

for toml_file in Path("~/.cis/plan").rglob("*.toml"):
    try:
        toml.load(toml_file.expanduser())
        print(f"✓ {toml_file}")
    except Exception as e:
        print(f"✗ {toml_file}: {e}")
        sys.exit(1)
EOF
```

### 3. Check Database Readiness

```bash
# Test SQLite database creation
TEST_DB="/tmp/cis-test-$(date +%s).db"
sqlite3 "$TEST_DB" <<EOF
CREATE TABLE test (id INTEGER PRIMARY KEY);
INSERT INTO test VALUES (1);
SELECT * FROM test;
EOF

if [ $? -eq 0 ]; then
  echo "✓ SQLite is ready"
  rm "$TEST_DB"
else
  echo "✗ SQLite test failed"
  exit 1
fi
```

### 4. Verify Disk Space

```bash
# Calculate required space
TOML_SIZE=$(du -sh ~/.cis/plan | cut -f1)
REQUIRED_GB=2  # Minimum 2GB free

FREE_SPACE=$(df -h ~ | tail -1 | awk '{print $4}')
echo "TOML data size: $TOML_SIZE"
echo "Free space: $FREE_SPACE"

# Check if sufficient space
FREE_BLOCKS=$(df -k ~ | tail -1 | awk '{print $4}')
if [ "$FREE_BLOCKS" -lt 2097152 ]; then  # 2GB in KB
  echo "✗ Insufficient disk space"
  echo "Required: 2GB, Available: $FREE_SPACE"
  exit 1
fi
```

### 5. Review Current Tasks

```bash
# Count tasks in TOML files
find ~/.cis/plan -name "*.toml" -exec grep -h "^id = " {} \; | wc -l

# List all TOML files
find ~/.cis/plan -name "*.toml" -type f

# Review critical tasks
grep -r "priority = \"P0\"" ~/.cis/plan/
```

### 6. Document Current State

```bash
# Create pre-migration report
cat > "$BACKUP_DIR/pre-migration-report.txt" <<EOF
CIS Pre-Migration Report
Generated: $(date)
================================

CIS Version: $(cis --version)
System: $(uname -a)
Disk Usage: $(df -h ~ | tail -1)

TOML Files:
$(find ~/.cis/plan -name "*.toml" -type f | wc -l) files

Task Count:
$(find ~/.cis/plan -name "*.toml" -exec grep -h "^id = " {} \; | wc -l) tasks

Database Status:
$(ls -lh ~/.cis/data/tasks.db 2>/dev/null || echo "No existing database")
EOF

cat "$BACKUP_DIR/pre-migration-report.txt"
```

---

## Migration Process

### Step 1: Dry-Run Migration

```bash
# Test migration without writing to database
cis migrate run ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml \
  --dry-run \
  --verbose
```

Expected output:
```
[INFO] Starting migration (dry-run mode)
[INFO] Found 25 tasks in TOML file
[INFO] Found 3 teams in TOML file
[INFO] Validation passed
[INFO] Would migrate 25 tasks, 3 teams
[INFO] Dry-run complete, no changes made
```

### Step 2: Migrate Single File

```bash
# Migrate individual TOML file
cis migrate run ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml \
  --verbose
```

### Step 3: Migrate Directory

```bash
# Migrate all TOML files in directory
cis migrate run ~/.cis/plan/v1.1.6/ \
  --verify
```

### Step 4: Verify Migration

```bash
# Check migrated tasks
cis task list --all

# Verify specific task
cis task show TASK-001

# Count migrated tasks
MIGRATED=$(cis task list --all --json | jq 'length')
echo "Migrated $MIGRATED tasks"
```

### Step 5: Validate Database

```bash
# Check database integrity
sqlite3 ~/.cis/data/tasks.db "PRAGMA integrity_check;"

# Check table counts
sqlite3 ~/.cis/data/tasks.db <<EOF
SELECT 'tasks' as table_name, COUNT(*) as count FROM tasks
UNION ALL
SELECT 'teams', COUNT(*) FROM teams;
EOF

# Verify indexes
sqlite3 ~/.cis/data/tasks.db ".indexes tasks"
```

---

## Post-Migration Steps

### 1. Compare Data

```bash
# Compare task counts
TOML_COUNT=$(find ~/.cis/plan -name "*.toml" -exec grep -h "^id = " {} \; | wc -l)
DB_COUNT=$(sqlite3 ~/.cis/data/tasks.db "SELECT COUNT(*) FROM tasks;")

echo "TOML tasks: $TOML_COUNT"
echo "Database tasks: $DB_COUNT"

if [ "$TOML_COUNT" -eq "$DB_COUNT" ]; then
  echo "✓ Task counts match"
else
  echo "✗ Task count mismatch!"
  echo "Difference: $((DB_COUNT - TOML_COUNT))"
fi
```

### 2. Update References

```bash
# Update scripts that reference TOML files
find ~/scripts -name "*.sh" -exec sed -i.bak \
  's|~/.cis/plan/TASKS.toml|cis task list|g' {} \;

# Update documentation
grep -r "TASKS.toml" ~/docs/ | \
  while read file; do
    echo "Update: $file"
  done
```

### 3. Archive TOML Files

```bash
# Move TOML files to archive
ARCHIVE_DIR="$HOME/.cis/archive/toml-$(date +%Y%m%d)"
mkdir -p "$ARCHIVE_DIR"

mv ~/.cis/plan/v1.1.6/*.toml "$ARCHIVE_DIR/"

echo "TOML files archived to: $ARCHIVE_DIR"
```

### 4. Update Automation

```bash
# Update CI/CD pipelines
cat > .github/workflows/cis-task-update.yml <<EOF
name: Update CIS Tasks
on:
  push:
    paths:
      - 'src/**'

jobs:
  update-tasks:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Update task status
        run: |
          cis task update TASK-001 --status running
          # Run build...
          cis task update TASK-001 --status completed
EOF
```

### 5. Test Functionality

```bash
# Test task creation
cis task create TEST-MIGRATION-001 "Test migration" \
  --type test \
  --priority P2

# Verify task appears
cis task show TEST-MIGRATION-001

# Clean up test task
cis task delete TEST-MIGRATION-001
```

---

## Verifying Migration

### Verification Checklist

```bash
#!/bin/bash
# verify-migration.sh

echo "CIS Migration Verification"
echo "========================="

# 1. Database exists
if [ -f ~/.cis/data/tasks.db ]; then
  echo "✓ Database exists"
else
  echo "✗ Database not found"
  exit 1
fi

# 2. Database is readable
if sqlite3 ~/.cis/data/tasks.db "SELECT COUNT(*) FROM tasks;" > /dev/null 2>&1; then
  echo "✓ Database is readable"
else
  echo "✗ Database is not readable"
  exit 1
fi

# 3. Tasks migrated
TASK_COUNT=$(sqlite3 ~/.cis/data/tasks.db "SELECT COUNT(*) FROM tasks;")
if [ "$TASK_COUNT" -gt 0 ]; then
  echo "✓ Tasks migrated: $TASK_COUNT"
else
  echo "✗ No tasks found"
  exit 1
fi

# 4. Teams migrated
TEAM_COUNT=$(sqlite3 ~/.cis/data/tasks.db "SELECT COUNT(DISTINCT assigned_team_id) FROM tasks WHERE assigned_team_id IS NOT NULL;")
if [ "$TEAM_COUNT" -gt 0 ]; then
  echo "✓ Teams found: $TEAM_COUNT"
else
  echo "⚠ No teams found (may be expected)"
fi

# 5. Dependencies preserved
DEP_COUNT=$(sqlite3 ~/.cis/data/tasks.db "SELECT COUNT(*) FROM task_dependencies WHERE 1=1;")
if [ "$DEP_COUNT" -gt 0 ]; then
  echo "✓ Dependencies preserved: $DEP_COUNT"
else
  echo "⚠ No dependencies found"
fi

# 6. CLI functionality
if cis task list > /dev/null 2>&1; then
  echo "✓ CLI task list works"
else
  echo "✗ CLI task list failed"
  exit 1
fi

echo ""
echo "Migration verification complete!"
```

### Data Integrity Checks

```bash
# Check for orphaned tasks
sqlite3 ~/.cis/data/tasks.db <<EOF
SELECT 'Orphaned tasks (dependencies on non-existent tasks):' as check;
SELECT t1.task_id, t1.dependencies
FROM tasks t1
WHERE t1.dependencies IS NOT NULL
  AND NOT EXISTS (
    SELECT 1 FROM tasks t2
    WHERE t2.task_id IN (SELECT value FROM json_each(t1.dependencies))
  );
EOF

# Check for circular dependencies
sqlite3 ~/.cis/data/tasks.db <<EOF
WITH RECURSIVE dep_chain(task_id, depth, path) AS (
  SELECT task_id, 0, task_id FROM tasks
  UNION ALL
  SELECT t.task_id, dc.depth + 1, dc.path || ' -> ' || t.task_id
  FROM tasks t
  JOIN dep_chain dc ON t.task_id IN (
    SELECT value FROM json_each(dc.dependencies)
  )
  WHERE dc.depth < 10
)
SELECT 'Potential circular dependencies:' as check, * FROM dep_chain WHERE depth > 0 LIMIT 10;
EOF
```

### Performance Validation

```bash
# Test query performance
time sqlite3 ~/.cis/data/tasks.db \
  "SELECT * FROM tasks WHERE status = 'pending' ORDER BY priority DESC;"

# Test CLI performance
time cis task list --status pending --limit 100

# Expected: < 1 second for 1000 tasks
```

---

## Handling Common Issues

### Issue 1: Migration Fails with Parse Error

**Symptom**:
```
Error: Failed to parse TOML file: expected '=' at line 42
```

**Solution**:

```bash
# Validate TOML syntax
toml check ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Find error line
sed -n '40,45p' ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Fix syntax error (example: missing quote)
sed -i 's/name = Unquoted/name = "Quoted"/' ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Re-run migration
cis migrate run ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml
```

### Issue 2: Database Lock Error

**Symptom**:
```
Error: database is locked
```

**Solution**:

```bash
# Check for other processes
lsof ~/.cis/data/tasks.db

# Kill stuck processes
kill -9 $(lsof -t ~/.cis/data/tasks.db)

# Remove lock file
rm -f ~/.cis/data/tasks.db-shm ~/.cis/data/tasks.db-wal

# Retry migration
cis migrate run ~/.cis/plan/v1.1.6/ --verify
```

### Issue 3: Task Count Mismatch

**Symptom**: Database has fewer tasks than TOML files

**Solution**:

```bash
# Find missing tasks
TOML_TASKS=$(find ~/.cis/plan -name "*.toml" -exec grep -h "^id = " {} \; | sort)
DB_TASKS=$(sqlite3 ~/.cis/data/tasks.db "SELECT task_id FROM tasks;" | sort)

comm -23 <(echo "$TOML_TASKS") <(echo "$DB_TASKS") > missing_tasks.txt

echo "Missing tasks:"
cat missing_tasks.txt

# Re-migrate only missing tasks
while read task_id; do
  echo "Migrating missing task: $task_id"
  # Extract and migrate specific task
done < missing_tasks.txt
```

### Issue 4: Character Encoding Issues

**Symptom**: Special characters display incorrectly

**Solution**:

```bash
# Check file encoding
file -i ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Convert to UTF-8 if needed
iconv -f ISO-8859-1 -t UTF-8 ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml > /tmp/tasks-utf8.toml
mv /tmp/tasks-utf8.toml ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Re-migrate
cis migrate run ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml
```

### Issue 5: Dependency Errors

**Symptom**: Tasks have invalid dependencies

**Solution**:

```bash
# Find invalid dependencies
sqlite3 ~/.cis/data/tasks.db <<EOF
SELECT task_id, dependencies
FROM tasks
WHERE dependencies IS NOT NULL
  AND json_valid(dependencies) = 0;
EOF

# Fix or remove invalid dependencies
sqlite3 ~/.cis/data/tasks.db <<EOF
UPDATE tasks
SET dependencies = NULL
WHERE task_id = 'PROBLEMATIC-TASK-ID';
EOF
```

---

## Rollback Procedures

### Automatic Rollback on Failure

```bash
# Migration with automatic rollback on error
cis migrate run ~/.cis/plan/v1.1.6/ \
  --verify \
  --auto-rollback
```

### Manual Rollback

```bash
# 1. Stop all CIS processes
pkill -f cis-node
pkill -f cis-core

# 2. Backup current database
cp ~/.cis/data/tasks.db ~/.cis/data/tasks.db.failed

# 3. Restore from backup
cp ~/cis-backup-YYYYMMDD/tasks.db ~/.cis/data/tasks.db

# 4. Restore TOML files
cp -r ~/cis-backup-YYYYMMDD/plan/* ~/.cis/plan/

# 5. Restart CIS
cis core init
```

### Selective Rollback

```bash
# Rollback tasks created after timestamp
TARGET_TS=$(date -d "2026-02-13 00:00:00" +%s)

sqlite3 ~/.cis/data/tasks.db <<EOF
-- Create backup table
CREATE TABLE tasks_backup AS SELECT * FROM tasks;

-- Delete recent tasks
DELETE FROM tasks
WHERE created_at_ts > $TARGET_TS;

-- Verify
SELECT COUNT(*) as deleted_count FROM tasks_backup
WHERE created_at_ts > $TARGET_TS;
EOF
```

### Point-in-Time Recovery

```bash
# 1. Identify rollback point
cis migrate list-history

# 2. Rollback to specific migration
cis migrate rollback \
  --before 1678886400 \
  --database ~/.cis/data/tasks.db

# 3. Verify
cis task list --all
```

---

## Best Practices

### 1. Test Migration on Copy

```bash
# Create test environment
cp -r ~/.cis /tmp/cis-test

# Migrate test copy
export CIS_DATA_DIR=/tmp/cis-test/data
cis migrate run /tmp/cis-test/plan/v1.1.6/ --verify

# Validate
cis --data-dir /tmp/cis-test/data task list

# If successful, migrate real data
cis migrate run ~/.cis/plan/v1.1.6/ --verify
```

### 2. Incremental Migration

```bash
# Migrate in phases
# Phase 1: Migrate P0 tasks
cis migrate run p0-tasks.toml --verify

# Phase 2: Migrate P1 tasks
cis migrate run p1-tasks.toml --verify

# Phase 3: Migrate remaining tasks
cis migrate run all-tasks.toml --verify
```

### 3. Validation After Each Step

```bash
# Validate after each migration
validate_migration() {
  local toml_file="$1"
  local expected_count="$2"

  cis migrate run "$toml_file" --verify

  local actual_count=$(cis task list --all --json | jq 'length')

  if [ "$actual_count" -eq "$expected_count" ]; then
    echo "✓ Validation passed: $actual_count tasks"
  else
    echo "✗ Validation failed: expected $expected_count, got $actual_count"
    return 1
  fi
}

# Use in migration script
validate_migration "p0-tasks.toml" 25
validate_migration "p1-tasks.toml" 50
```

### 4. Monitor Performance

```bash
# Monitor migration progress
watch -n 1 '
echo "Migrated tasks: $(cis task list --all --json | jq length)"
echo "Database size: $(du -h ~/.cis/data/tasks.db | cut -f1)"
echo "Recent tasks:"
cis task list --limit 3
'
```

### 5. Document Migration Process

```bash
# Create migration log
exec > >(tee -a ~/cis-migration.log)
exec 2>&1

echo "=== Migration started at $(date) ==="

# Migration steps
cis migrate run ~/.cis/plan/v1.1.6/ --verify

echo "=== Migration completed at $(date) ==="
```

### 6. Backup Strategy

```bash
# Pre-migration backup
backup_before_migration() {
  local backup_dir="$HOME/cis-backup-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$backup_dir"

  # Backup TOML files
  cp -r ~/.cis/plan "$backup_dir/"

  # Backup database (if exists)
  [ -f ~/.cis/data/tasks.db ] && cp ~/.cis/data/tasks.db "$backup_dir/"

  # Create manifest
  cat > "$backup_dir/MANIFEST.txt" <<EOF
Backup created: $(date)
CIS version: $(cis --version)
Files backed up: $(find "$backup_dir" -type f | wc -l)
Total size: $(du -sh "$backup_dir" | cut -f1)
EOF

  echo "$backup_dir"
}

# Use before migration
BACKUP_DIR=$(backup_before_migration)
echo "Backup created at: $BACKUP_DIR"
```

---

## FAQ

### Q: Can I keep using TOML files after migration?

A: Yes, but CIS v1.1.6+ reads from SQLite by default. To continue using TOML:

```bash
# Force TOML mode (not recommended)
export CIS_STORAGE_BACKEND=toml
cis task list
```

### Q: How long does migration take?

A: Migration time depends on data size:

- 100 tasks: ~5 seconds
- 1,000 tasks: ~30 seconds
- 10,000 tasks: ~5 minutes

### Q: Can I migrate without downtime?

A: Yes, using read-only mode:

```bash
# 1. Start migration in background
cis migrate run ~/.cis/plan/ --verify &
MIGRATE_PID=$!

# 2. Continue using CIS (reads from TOML)
cis task list

# 3. After migration completes, switch to SQLite
wait $MIGRATE_PID
cis task list  # Now reads from SQLite
```

### Q: What if migration is interrupted?

A: Migration is transactional. Interrupted migrations are automatically rolled back:

```bash
# If migration is interrupted
cis migrate run ~/.cis/plan/ --verify

# Resume from checkpoint
cis migrate run ~/.cis/plan/ --resume
```

### Q: Can I migrate multiple times?

A: Yes, but duplicate tasks are skipped:

```bash
# First migration
cis migrate run ~/.cis/plan/ --verify

# Second migration (safe, skips existing)
cis migrate run ~/.cis/plan/ --verify

# Force re-migration (overwrites existing)
cis migrate run ~/.cis/plan/ --force
```

### Q: How do I migrate custom fields?

A: Custom fields are preserved in metadata:

```bash
# In TOML
[task.metadata]
custom_field = "custom_value"

# After migration
cis task show TASK-001 --json | jq '.metadata.custom_field'
```

### Q: Can I migrate specific tasks only?

A: Yes, by creating filtered TOML files:

```bash
# Extract specific tasks
grep -A 20 'id = "TASK-001"' ~/.cis/plan/tasks.toml > tasks-filtered.toml

# Migrate filtered file
cis migrate run tasks-filtered.toml --verify
```

### Q: What about skill definitions?

A: Skills are managed separately:

```bash
# Skills are not migrated with tasks
# Verify skills are still accessible
cis skill list

# Re-install skills if needed
cis skill install <skill-name>
```

---

## Advanced Topics

### Custom Migration Scripts

```bash
#!/bin/bash
# custom-migration.sh

# Transform TOML before migration
transform_toml() {
  local input="$1"
  local output="$2"

  # Apply custom transformations
  python3 <<EOF
import toml
import json
import sys

data = toml.load(input)

# Custom transformation logic
for task in data.get('task', []):
    if task.get('priority') == 'urgent':
        task['priority'] = 'P0'

# Save transformed data
with open(output, 'w') as f:
    toml.dump(data, f)
EOF
}

# Use in migration
transform_toml ~/.cis/plan/tasks.toml /tmp/tasks-transformed.toml
cis migrate run /tmp/tasks-transformed.toml --verify
```

### Database Optimization

```bash
# After migration, optimize database
sqlite3 ~/.cis/data/tasks.db <<EOF
-- Analyze tables for query optimization
ANALYZE;

-- Rebuild database
VACUUM;

-- Reindex
REINDEX;
EOF

# Verify optimization
sqlite3 ~/.cis/data/tasks.db "PRAGMA optimize;"
```

### Parallel Migration

```bash
# Migrate multiple TOML files in parallel
find ~/.cis/plan -name "*.toml" | parallel \
  "cis migrate run {} --verify && echo 'Migrated: {}'"

# Wait for all to complete
wait
```

---

## Reference

### Migration Commands

```bash
cis migrate run <source> [options]
cis migrate verify [options]
cis migrate rollback --before <timestamp> [options]
```

### SQLite Queries

```sql
-- Count tasks by status
SELECT status, COUNT(*) FROM tasks GROUP BY status;

-- Find tasks with dependencies
SELECT task_id, dependencies FROM tasks WHERE dependencies IS NOT NULL;

-- Find high-priority pending tasks
SELECT task_id, name, priority FROM tasks
WHERE status = 'pending' AND priority IN ('P0', 'P1')
ORDER BY priority DESC;
```

### Error Codes

| Code | Meaning | Action |
|------|---------|--------|
| `MIG-001` | TOML parse error | Fix TOML syntax |
| `MIG-002` | Database lock | Stop other processes |
| `MIG-003` | Duplicate task ID | Use `--force` to overwrite |
| `MIG-004` | Invalid dependency | Fix dependency reference |
| `MIG-005` | Insufficient space | Free up disk space |

---

## Additional Resources

- [Task Management Guide](./task-management-guide.md)
- [CIS CLI Reference](./cli-reference.md)
- [CIS Architecture](../ARCHITECTURE.md)
- [SQLite Documentation](https://www.sqlite.org/docs.html)

---

**Last Updated**: 2026-02-13
**For questions or issues**, visit [CIS GitHub](https://github.com/your-org/cis)
