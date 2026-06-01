# `config` — Configuration Management

The `config` subcommand family manages the user's `config.toml` file
located at `~/.config/sprachspiel/config.toml` (or
`$XDG_CONFIG_HOME/sprachspiel/config.toml`).

## Synopsis

```bash
sprach config <SUBCOMMAND> [OPTIONS]
```

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `upgrade`  | Merge missing default fields into existing `config.toml` |

Run `sprach config help <subcommand>` for details on a specific
subcommand.

## `config upgrade` — Merge missing defaults

```bash
sprach config upgrade [--dry-run] [--no-backup]
```

### What it does

Every release of sprachspiel adds new configuration fields (e.g.
`[feedback]` in v0.40, `[facts]` in v0.42, `[retrieval]` in v0.43,
`[thinking_trace]` in v0.45). When you upgrade sprachspiel, your
existing `config.toml` is **not** automatically updated — the new
fields silently fall back to their built-in defaults.

`config upgrade` solves this. It:

1. Reads your existing `config.toml`
2. Compares it against `Settings::default()` (the canonical list of
   all fields and their default values for the current version of
   sprachspiel)
3. Inserts every field that is missing from your config, with its
   default value and a doc-comment extracted from the sample
   configuration
4. Writes the result back to `config.toml`

### Guarantees

- **Insert-only**: the command never modifies or removes existing
  values. Your customizations are preserved.
- **Format-preserving**: the underlying `toml_edit` library preserves
  your existing comments, blank lines, and key ordering.
- **Documented**: each inserted field comes with a doc-comment
  explaining what it does (extracted from the sample config).
- **Backed up by default**: before any write, the command creates
  `config.toml.bak` (or `config.toml.bak.YYYYMMDD-HHMMSS` if `.bak`
  already exists). Use `--no-backup` to opt out.
- **Safe on invalid TOML**: if your `config.toml` has a syntax
  error, the command reports the parser error and aborts. It never
  overwrites a config it cannot parse.

### Options

| Flag          | Description                                                  |
|---------------|--------------------------------------------------------------|
| `--dry-run`   | Show what would be added without modifying the file.         |
| `--no-backup` | Skip creating a `.bak` backup file.                          |

### Examples

Upgrade your config with a backup (default behavior):

```bash
sprach config upgrade
```

Preview changes without modifying the file:

```bash
sprach config upgrade --dry-run
```

Upgrade without creating a backup (use with caution):

```bash
sprach config upgrade --no-backup
```

### Sample output

```
$ sprach config upgrade
Config: /home/user/.config/sprachspiel/config.toml

Found 3 new field(s):
  - facts.auto_extract (default: true, bool)
  - facts.max_facts (default: 3, int)
  - facts.auto_extract_notify (default: true, bool)

Backup created: /home/user/.config/sprachspiel/config.toml.bak
Upgraded 3 field(s) successfully.
```

### When to run

Run `sprach config upgrade` after upgrading sprachspiel to a new
version. Compare the current version against the previous version in
the [CHANGELOG](../../CHANGELOG.md) — if new configuration sections
or fields were added, run this command to merge them.

### Troubleshooting

**"Config file not found"**

You have not initialized a config yet. Run `sprach --init-config`
first, then re-run `sprach config upgrade`.

**"Invalid TOML in config.toml"**

Your `config.toml` has a syntax error. The command will not modify
it. Either fix the syntax manually (the error message includes the
line and column), or run `sprach --init-config` to create a new
config (your existing file will NOT be overwritten unless you do so
explicitly).

**"No new fields detected"**

Your config is already up to date. The command reports "Config is
already up to date." and exits successfully.
