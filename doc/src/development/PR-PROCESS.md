# Pull Request Process

This document describes the mandatory workflow for implementing features and fixes.

## ⚠️ CRITICAL RULES

### NEVER Do These

1. **NEVER close issues before PR merge** - Issues are closed automatically when PR is merged
2. **NEVER move cards to "Done"** - Only the reviewer moves cards to "Done" after approval
3. **NEVER merge without approval** - PRs must be reviewed before merge

### ALWAYS Do These

1. **ALWAYS create PR as DRAFT first** - Then implement, then mark "ready for review"
2. **ALWAYS move card to "In Review"** - After creating PR and before marking ready
3. **ALWAYS update CHANGELOG and IMPLEMENTATION.md** - Before committing code changes
4. **ALWAYS reference the issue in PR body** - Use "Closes #N" or "Related #N"

## Workflow: Step by Step

### Phase 1: Setup

```
1. Create branch: git checkout -b <type>/<description>
   Types: feat/, fix/, refactor/, docs/, test/

2. Verify you're on the correct branch

3. Read IMPLEMENTATION.md to understand the task status
```

### Phase 2: Documentation FIRST

```
4. Update CHANGELOG.md:
   - Add entry under appropriate version
   - Use "Added", "Changed", "Fixed", "Removed" sections

5. Update IMPLEMENTATION.md:
   - Mark task status as 🔄 IN PROGRESS when starting
   - Will mark as ✅ COMPLETED only after merge

6. Commit documentation: git commit -m "docs: update CHANGELOG for <feature>"
```

### Phase 3: Implementation

```
7. Implement the feature/fix

8. Run tests: cargo test --all-features

9. Run clippy: cargo clippy --all-features -- -D warnings

10. Commit code changes (conventional commits):
    feat: <description>
    fix: <description>
    refactor: <description>

11. Push branch: git push -u origin <branch>
```

### Phase 4: Pull Request

```
12. Create PR as DRAFT:
    gh pr create --draft --title "<type>: <description>" --body "..."

13. Move GitHub Project card to "In Review" (both Status and Scrum Status)

14. Mark PR as "ready for review":
    gh pr ready <number>

15. Add comment to issue: "PR #N ready for review"
```

### Phase 5: After Approval (REVIEWER ONLY)

```
16. Reviewer approves and merges PR

17. Reviewer moves card to "Done"

18. Issue is closed automatically (via "Closes #N" in PR body)
```

## GitHub Project Status Flow

```
Todo → In Progress → In Review → Done
                ↑         ↑         ↑
            (start)   (PR created) (approved)
```

- **Todo**: Task is planned but not started
- **In Progress**: Task is being implemented
- **In Review**: PR created, awaiting review
- **Done**: PR merged (REVIEWER ONLY)

## Conventional Commits

Format: `<type>: <description>`

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code refactoring |
| `docs` | Documentation only |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |

## Example PR Body

```markdown
## Summary

Brief description of changes.

## Changes

| File | Change |
|------|--------|
| `src/foo.rs` | Added X |
| `doc/src/CHANGELOG.md` | Added entry |

## Testing

- [ ] `cargo build --all-features`
- [ ] `cargo clippy --all-features -- -D warnings`
- [ ] `cargo test --all-features`

## Related

Closes #N  (use only when PR will completely close the issue)
Related #N (use when PR is related but not closing)
```

## Quick Reference

```bash
# Create branch
git checkout -b feat/my-feature

# Create draft PR
gh pr create --draft --title "feat: my feature" --body "..."

# Move card to In Review (requires project item ID)
gh api graphql -f query='
mutation {
  updateProjectV2ItemFieldValue(
    input: {
      projectId: "PVT_kwHOADplIc4BRnZ9"
      itemId: "ITEM_ID"
      fieldId: "PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg"
      value: { singleSelectOptionId: "77520bb7" }
    }
  ) { projectV2Item { id } }
}'

# Mark PR ready for review
gh pr ready PR_NUMBER

# DO NOT: Close issue, move to Done, merge without approval
```

## Project Information

- **Project ID**: `PVT_kwHOADplIc4BRnZ9`
- **Status Field ID**: `PVTSSF_lAHOADplIc4BRnZ9zg_ZGpg`
- **Scrum Status Field ID**: `PVTSSF_lAHOADplIc4BRnZ9zg_ZHUY`

**Status Options:**
| Name | ID |
|------|-----|
| Todo | `f75ad846` |
| In Progress | `47fc9ee4` |
| In Review | `77520bb7` |
| Done | `98236657` |

**Scrum Status Options:**
| Name | ID |
|------|-----|
| Backlog | `94ed2e0f` |
| Ready | `70e88e2e` |
| In Progress | `c2eae8ae` |
| In Review | `d242b7c7` |
| Done | `a456e7a8` |