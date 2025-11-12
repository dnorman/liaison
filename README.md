# liaison

Materialize referenced content into source files **in place** while preserving wrapper metadata.

Works for HTML and any plaintext format (Markdown, Rust, etc.).

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Apply changes to specified files
liaison file1.rs file2.html

# Check if changes would be made (dry run)
liaison --check file1.rs

# Process files matching glob patterns (configure in .liaison.toml)
liaison
```

## Configuration

Create `.liaison.toml` at your repository root:

```toml
[glob]
include = ["**/*.{rs,md,html}"]
exclude = ["target/**", "node_modules/**"]
```

Default: empty include (process nothing unless files are specified via CLI).

## Plaintext Module

For `.rs`, `.py`, `.sh`, `.md`, `.txt` and other text files:

**Source file (define blocks with IDs):**

```rust
// liaison id=helper
fn helper() -> i32 {
    42
}
// liaison end
```

**Target file (reference and transclude):**

```rust
// liaison transclude="src/lib.rs#helper"
// old content replaced automatically
// liaison end
```

After running `liaison`, the target file becomes:

```rust
// liaison transclude="src/lib.rs#helper"
fn helper() -> i32 {
    42
}
// liaison end
```

## HTML Module

**Source HTML:**

```html
<section id="intro">
  <p>Welcome to the guide</p>
</section>
```

**Target HTML:**

```html
<article transclude="docs/guide.html#intro">
  <p>Old content</p>
</article>
```

After running `liaison`:

```html
<article transclude="docs/guide.html#intro">
  <p>Welcome to the guide</p>
</article>
```

## Features

- **Recursive expansion**: Transcluded content can itself contain transclude directives
- **Cycle detection**: Prevents infinite loops
- **Atomic writes**: All-or-nothing updates (no partial changes on errors)
- **HTTP sources**: Fetch content from `http://` or `https://` URLs
- **Dry run mode**: Use `--check` to preview changes

## Reference Syntax

### Plaintext

- `transclude="path/to/file.rs#id"` - extracts content from named block
- `transclude="path/to/file.rs"` - includes entire file

### HTML

- `transclude="path/to/file.html#intro"` - CSS selector (ID)
- `transclude="path/to/file.html#section.main"` - more complex selectors
- `transclude="path/to/file.html"` - defaults to `<body>` content

## Path Resolution

All paths in `transclude` directives are resolved relative to the **Git repository root** of the file being processed, not your current working directory.

```bash
# Works correctly even when run from a different directory
cd ~/my-tool && liaison ~/my-docs/index.html
```

When processing `index.html`, liaison:

1. Determines the Git repository root containing `index.html`
2. Resolves all paths in that file relative to that repository root
3. Recursively applies the same logic for any transcluded files

This allows you to process files in any repository from any working directory.

**Note**: When processing multiple files in one command, all files must be in the same repository. Liaison will fail with an error if files are from different repositories.

## Safety

- Repository-relative paths only (no `..` escapes)
- Git repo detection via `git rev-parse` for each file (fallback to file's directory)
- No caching or offline mode
- Existing content preserved on any error

## Examples

See `tests/fixtures/` for working examples.

## License

Dual-licensed under MIT or Apache 2.0.
