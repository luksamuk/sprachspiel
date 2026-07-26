---
name: code-analysis
description: Load when analyzing code, exploring a codebase, understanding architecture, or finding patterns. Provides structured workflows for code exploration, pattern discovery, and code review using file tools and run_command (rg).
---

# Code Analysis

When asked to analyze code:

1. **Understand the codebase structure**:
   - Use `list_directory` to explore directories
   - Use `run_command("rg -n pattern path")` to find patterns across files
   - Use `read_file` to examine specific files in detail

2. **Find definitions and patterns**:
   - Use `run_command` with `rg` and regex patterns:
     - Find function definitions: `run_command("rg -n \"fn [a-z_]+\" src/", null, null, null)`
     - Find class definitions: `run_command("rg -n \"class [A-Z][a-zA-Z]+\" .", null, null, null)`
     - Find imports: `run_command("rg -n \"import|use|require\" .", null, null, null)`

3. **Analyze specific files**:
   - Use `read_file(path)` for complete file content
   - Use `read_file_segment(path, start, lines)` for large files
   - Use `count_lines(path)` to understand file size

4. **Code metrics**:
   - Count functions: `run_command("rg -c \"fn \" src/", null, null, null)` (count per file)
   - Count classes: `run_command("rg -c \"class \" src/", null, null, null)`
   - Lines of code: Use `count_lines` on relevant files

5. **Find dependencies**:
   - Look for package files: package.json, Cargo.toml, requirements.txt
   - Analyze import statements
   - Check for dependency files

6. **Best practices**:
   - Start with broad searches, then narrow down
   - Always show file paths when referencing code
   - Explain patterns you find
   - Suggest improvements when appropriate
   - Use head/tail to limit rg output: `run_command("rg -n pattern src/", "50", null, null)`

## Common Patterns

```bash
# Find all TypeScript/JavaScript functions
run_command("rg -n \"function [a-z_]+\" .", null, null, null)

# Find all Rust structs
run_command("rg -n \"struct [A-Z]\" .", null, null, null)

# Find all Python classes
run_command("rg -n \"class [A-Z][a-zA-Z]+\" .", null, null, null)

# Find TODO comments
run_command("rg -n \"TODO|FIXME|HACK\" .", null, null, null)

# Find all test files (by filename pattern)
run_command("rg -n --glob \"*_test.*\" \"\" .", null, null, null)

# Search only .rs files
run_command("rg -n --glob *.rs \"pattern\" .", null, null, null)
```

## Tips

- Use `list_directory(".", true)` for recursive listing
- Use `rg --glob` to filter by file type: `run_command("rg -n --glob *.rs pattern .", null, null, null)`
- Always check line count before reading large files