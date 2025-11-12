# Liaison Specification

## Name

**liaison**

## Goal

Materialize referenced content into source files **in place** while preserving wrapper metadata. Works for HTML and any plaintext (Markdown, Rust, etc.).

## Modules

1. **HTML module**

   - Structured parse. Operates on HTML tags with attributes.

2. **Plaintext module**

   - Line-oriented. Uses comment sentinels. Applies to `.md`, `.markdown`, `.rs`, `.txt`, and any non-HTML file.

## Config

File: `.liaison.toml` at repo apex.

```toml
[glob]
include = []  # default: match nothing
exclude = ["target/**", "node_modules/**"]  # default empty
```

- Paths are repo-root relative.
- Apply `include`, then remove matches in `exclude`.
- CLI paths override the default (empty) glob config.

## File Typing

- `.html`, `.htm` → HTML module.
- Everything else → Plaintext module.

## Allowed Sources

- `https://` and `http://`.
- Repo-relative paths (no scheme). Resolution base = Git repo apex. Reject any path that escapes repo (`..` not allowed).
- No `file:` URLs.

## Blocks

### HTML Module

- **Identified block (source)**: any element with `id="…"`; **content** = innerHTML only.
- **Materialized block (target)**: any element with `transclude="…"`; rewrite replaces only innerHTML. Keep tag and attributes unchanged.

**Reference syntax:**
`transclude="<uri>[#selector]"`

**Selectors:**

- CSS subset for HTML sources: `#id`, `tag#id`, `A > B#id`, `.class`, comma-list.
- Result = innerHTML of first match only.
- Default selector if omitted: `body`.

**Example:**

```html
<section id="intro"><p>Welcome</p></section>

<article transclude="docs/guide.html#intro">
  <!-- innerHTML is replaced -->
</article>
```

### Plaintext Module

Comment-based wrappers. Only **id** and **transclude** forms.

**Identified region (source)**

```rust
// liaison id=helper
fn helper() -> i32 { 42 }
// liaison end
```

**Materialized region (target)**

```rust
// liaison transclude="src/lib.rs#helper"
// replaced lines go here
// liaison end
```

**Comment tokens:**

Detected by file extension:

- `.rs`: `// `
- `.py`, `.sh`: `# `
- `.md`, `.markdown`, `.txt`: `<!-- -->` or `# `
- Other: `# ` (fallback)

**Selectors for plaintext sources:**

- `#id` only. Matches identified regions by `<ID>`.
- First match only.
- Default selector if omitted: entire file content.

**Examples (Rust):**

```rust
// liaison id=helper
fn helper() -> i32 { 42 }
// liaison end

// liaison transclude="src/lib.rs#helper"
// replaced lines go here
// liaison end
```

**Examples (Markdown as plaintext):**

```md
# Notes

<!-- liaison id=blurb -->

This text is the blurb.

<!-- liaison end -->

<!-- liaison transclude="README.md#blurb" -->

old material

<!-- liaison end -->
```

## Processing Model

- Run before other tools.
- Build an edit plan for all matched files:

  1. Parse files and locate all targets (blocks with `transclude` attributes/markers).
  2. Resolve each reference (network or repo path).
  3. If a resolved source itself contains transclude directives, recursively expand those. Cycles → error.

- If **any** resolution fails, write **nothing** and exit non-zero.
- If all succeed, apply replacements **only between** wrapper markers (plaintext `liaison`...`end`) or innerHTML (HTML).
- No concurrency.

## Safety

- Detect repo apex via `git rev-parse --show-toplevel`; fallback to CWD if not a Git repo.
- Reject escaping paths.
- No caching. No hashing. No offline mode. Existing content remains if any failure occurs.

## Rewriting Rules

- Preserve wrapper lines/tags and attributes verbatim.
- Do not insert or keep sentinel comments beyond what plaintext blocks require. HTML module preserves no extra comments.
- Whitespace in source content is preserved as-is (no normalization of indentation or newlines).

## CLI

```bash
liaison [--check] <path>...
```

- `--check`: dry run. Exit 0 if no changes needed, 1 if changes would be made or on error.
- Without flags: perform edits in place on success.

## Errors

- Network error, selector miss, invalid path, parse failure, or cycle → non-zero exit. No files written.
- Cycle detection node = `(uri, selector)`; edges from each materialized block to its dependencies.
