---
name: code-analysis
description: Analyze code structure, find patterns, and understand codebases using file tools.
---

# Code Analysis

When asked to analyze code:

1. **Understand the codebase structure**:
   - Use `list_directory` to explore directories
   - Use `search_files` to find patterns across files
   - Use `read_file` to examine specific files in detail

2. **Find definitions and patterns**:
   - Use `search_files(path, pattern)` with regex patterns:
     - Find function definitions: `search_files(".", "fn [a-z_]+")`
     - Find class definitions: `search_files(".", "class [A-Z][a-zA-Z]+")`
     - Find imports: `search_files(".", "import|use|require")`

3. **Analyze specific files**:
   - Use `read_file(path)` for complete file content
   - Use `read_file_segment(path, start, lines)` for large files
   - Use `count_lines(path)` to understand file size

4. **Code metrics**:
   - Count functions: `search_files(".", "fn ").total_results`
   - Count classes: `search_files(".", "class ").total_results`
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

## Common Patterns

```bash
# Find all TypeScript/JavaScript functions
search_files(".", "function [a-z_]+")

# Find all Rust structs
search_files(".", "struct [A-Z]")

# Find all Python classes
search_files(".", "class [A-Z][a-zA-Z]+")

# Find TODO comments
search_files(".", "TODO|FIXME|HACK")

# Find all test files
search_files(".", "_test\\.(rs|ts|js|py)$")
```

## Tips

- Use `list_directory(".", true)` for recursive listing
- Combine multiple `search_files` calls for complex patterns
- Always check line count before reading large files