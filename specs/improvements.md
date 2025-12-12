# Liaison Improvement Roadmap

## Recently Completed

### Recursive Transclusion in HTML
- HTML files can now contain comment-based transcludes (`<!-- liaison transclude="..." -->`)
- These are recursively expanded when the HTML is transcluded into another file
- Proper indentation is maintained at each level

### HTML Indentation Behavior
- HTML hosts apply indentation to transcluded content
- Element transcludes: content indented to match element position
- Comment transcludes: content indented to match comment marker
- Non-HTML hosts (plaintext) do not apply indentation

### Self-Closing Tag Support
- `<div transclude="..."/>` now correctly converts to `<div transclude="...">content</div>`
- Previously content was appended after the self-closing tag

### Reset Behavior
- `--reset` now collapses to `<tag></tag>` with no whitespace
- Previously left newlines between opening and closing tags

---

## Proposed Improvements

### 1. Host Descriptor Pattern (Refactoring)
**Priority:** Medium  
**Status:** Foundation created in `hosts.rs`

Replace hardcoded file type checks with a trait-based host system:
```rust
pub trait HostType {
    fn matches(&self, path: &Path) -> bool;
    fn find_transcludes(&self, content: &str) -> Vec<TranscludeMatch>;
    fn replace(&self, content: &str, match: &TranscludeMatch, resolved: &str) -> String;
    fn applies_indentation(&self) -> bool;
}
```

Benefits:
- Easier to add new file types (JSX, Vue, Svelte, etc.)
- Cleaner separation of concerns
- Plugin-like extensibility

### 2. Extended CSS Selectors
**Priority:** High  
**Status:** Spec says supported, not implemented

Currently only `#id` selectors work. Per spec, should support:
- `.class` - select by class
- `tag#id` - element type + id
- `A > B#id` - child combinator
- Comma-separated lists

### 3. URL Transclusion
**Priority:** Medium  
**Status:** Spec says supported, needs verification

Support fetching content from `https://` and `http://` URLs:
```html
<code transclude="https://raw.githubusercontent.com/user/repo/main/src/lib.rs#helper"></code>
```

Considerations:
- Timeout handling
- Error messages for network failures
- Optional caching (with TTL?)

### 4. Watch Mode
**Priority:** Low  
**Status:** Not started

```bash
liaison --watch <paths>
```

- Watch source files for changes
- Re-run transclusion when dependencies change
- Useful for documentation workflows

### 5. Diff Output for --check
**Priority:** Low  
**Status:** Not started

```bash
liaison --check --diff index.html
```

Show unified diff of what would change, similar to `rustfmt --check`.

### 6. Better Error Messages
**Priority:** Medium  
**Status:** Partial

Improve error messages to include:
- File path and line number
- Snippet of the problematic line
- Suggestions for common mistakes

Example:
```
error: No block with id 'helper' found
  --> src/lib.rs
  |
  | // liaison transclude="utils.rs#helper"
  |                                 ^^^^^^
  |
  = help: Available ids in utils.rs: validate, parse, format
```

### 7. Markdown Rendering Option
**Priority:** Low  
**Status:** Not started

When transcluding markdown into HTML, optionally render to HTML:
```html
<div transclude="README.md#intro" render="markdown"></div>
```

Would require a markdown parser dependency (pulldown-cmark).

### 8. Parallel Processing
**Priority:** Low  
**Status:** Not started

Process independent files concurrently. The spec says "No concurrency" but this could be opt-in:
```bash
liaison --parallel <paths>
```

### 9. Config File Enhancements
**Priority:** Low  
**Status:** Basic support exists

Extend `.liaison.toml`:
```toml
[glob]
include = ["docs/**/*.md", "src/**/*.rs"]
exclude = ["target/**"]

[html]
indent_style = "match"  # or "none", "2", "4", "tab"

[plaintext]
# Custom comment tokens for specific extensions
[plaintext.comments]
".jsx" = "//"
".vue" = "<!--"
```

---

## Non-Goals

- **LSP/IDE integration** - Out of scope for CLI tool
- **Bidirectional sync** - Liaison is one-way (source → target)
- **Templating** - No variable substitution or conditionals
- **Transformation** - No content modification beyond indentation

