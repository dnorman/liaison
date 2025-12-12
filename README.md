<div align="center">

<img src="./logo-128.png" alt="liaison logo" width="128" />

# liaison

[![Crates.io](https://img.shields.io/crates/v/liaison.svg)](https://crates.io/crates/liaison)
[![CI](https://github.com/dnorman/liaison/workflows/CI/badge.svg)](https://github.com/dnorman/liaison/actions)

A content transclusion tool that materializes references into source files **in place**, perfect for documentation that embeds code snippets.

Works with HTML, Markdown, Rust, TypeScript, Python, and any text format.

</div>

## Installation

```bash
cargo install liaison
```

Or from source:

```bash
git clone https://github.com/dnorman/liaison
cd liaison
cargo install --path .
```

## Quick Start

**1. Mark content in your source files:**

```rust
// src/lib.rs
pub fn example() {
    // liaison id=demo-code
    let x = 42;
    println!("Hello: {}", x);
    // liaison end
}
```

**2. Reference it in documentation:**

```markdown
<!-- README.md -->

Here's how to use it:

<!-- liaison transclude="src/lib.rs#demo-code" -->
<!-- liaison end -->
```

**3. Run liaison:**

```bash
liaison README.md
```

**4. Your documentation now contains the actual code:**

```markdown
<!-- README.md -->

Here's how to use it:

<!-- liaison transclude="src/lib.rs#demo-code" -->

let x = 42;
println!("Hello: {}", x);

<!-- liaison end -->
```

## Usage

```bash
# Process specific files
liaison index.html README.md

# Check what would change (dry run)
liaison --check README.md

# Clear all transcluded content (for testing)
liaison --reset README.md

# Continue processing even if some transclusions fail
liaison --ignore-errors README.md

# Process files matching patterns (.liaison.toml)
liaison
```

## Configuration

Create `.liaison.toml` in your repository root:

```toml
[glob]
include = ["docs/**/*.{md,html}", "README.md"]
exclude = ["target/**", "node_modules/**"]
```

**Default:** Empty include list (process nothing unless files specified via CLI).

## Syntax

### Plaintext Files

For code files (`.rs`, `.ts`, `.py`, `.js`, etc.) and Markdown:

**Define reusable blocks:**

```rust
// liaison id=my-function
fn my_function() -> i32 {
    42
}
// liaison end
```

**Reference them:**

```markdown
<!-- liaison transclude="src/lib.rs#my-function" -->
<!-- liaison end -->
```

**Comment styles auto-detected:**

- Rust, TypeScript, JavaScript: `//`
- Python, Shell: `#`
- Markdown, HTML: `<!-- -->`

### HTML Files

HTML supports two transclusion styles:

**Element transclusion** (using attributes):

```html
<!-- source.html -->
<section id="intro">
  <h1>Welcome</h1>
  <p>Getting started guide</p>
</section>

<!-- target.html -->
<div transclude="source.html#intro"></div>
```

After running liaison:

```html
<div transclude="source.html#intro">
  <h1>Welcome</h1>
  <p>Getting started guide</p>
</div>
```

**Comment transclusion** (same syntax as Markdown):

```html
<!-- works in HTML files too -->
<!-- liaison transclude="components/header.html#banner" -->
<!-- liaison end -->
```

**Self-closing tags** are automatically expanded:

```html
<!-- before -->
<header transclude="header.html#nav" />

<!-- after -->
<header transclude="header.html#nav">
  <nav>...</nav>
</header>
```

## Features

### Whitespace Normalization

Content is automatically dedented based on the marker's indentation:

```rust
fn main() {
    // liaison id=indented
    let x = 5;
    if x > 0 {
        println!("positive");
    }
    // liaison end
}
```

Extracts as:

```rust
let x = 5;
if x > 0 {
    println!("positive");
}
```

### Recursive Transclusion

Transcluded content can itself contain transclusions, which are automatically expanded:

```html
<!-- index.html transcludes header.html -->
<header transclude="header.html#banner"></header>

<!-- header.html contains a nested transclude -->
<div id="banner">
  <h1>My Site</h1>
  <!-- liaison transclude="tagline.md#slogan" -->
  <!-- liaison end -->
</div>
```

All levels are expanded in a single pass.

### HTML Indentation

HTML hosts automatically indent transcluded content to match the target element:

```html
<div transclude="content.html#section">
  <!-- content is indented to match -->
</div>
```

### Cycle Detection

Prevents infinite loops from circular references.

### Atomic Operations

All changes succeed or none are applied—no partial updates on errors.

### Remote Content

Fetch content from HTTP(S) URLs:

```markdown
<!-- liaison transclude="https://example.com/api/snippet.rs#demo" -->
<!-- liaison end -->
```

### HTML Escaping

Code from plaintext files is automatically HTML-escaped when transcluded into HTML:

```html
<pre><code transclude="src/lib.rs#generic-function"></code></pre>
```

Rust code with `<T>` generics becomes `&lt;T&gt;` in HTML.

## Path Resolution

Paths are resolved **relative to the Git repository root** of the file being processed:

```bash
# Works from any directory
cd ~/projects/tool && liaison ~/projects/docs/index.html
```

**File-relative paths** (since v0.1.0): Paths are first tried relative to the current file's directory, then fall back to repo-relative:

```html
<!-- docs/index.html -->
<div transclude="header.html#banner"></div>
<!-- Looks in docs/header.html first -->
```

**Cross-repository**: All files in a single command must be from the same repository.

## Safety

- **No directory traversal**: `..` in paths is rejected
- **Git-aware**: Automatically detects repository boundaries
- **Atomic writes**: Changes are transactional
- **Preserves structure**: Only innerHTML is replaced, attributes preserved

## CLI Reference

```
liaison [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Files to process (overrides glob config)

Options:
      --check          Check if changes would be made (dry run)
      --reset          Clear all transcluded content to empty
      --ignore-errors  Continue processing even if some transclusions fail
  -h, --help           Print help
  -V, --version        Print version
```

## Examples

See the [`demo/`](demo/) directory and [`tests/fixtures/`](tests/fixtures/) for working examples.

## Use Cases

- **Documentation**: Keep code examples in sync with actual source
- **Static sites**: Embed code snippets from your repository
- **Books/tutorials**: Auto-update code blocks from tested examples
- **API docs**: Include implementation snippets inline

## The Maniacal Plan

[Transclusion](https://en.wikipedia.org/wiki/Transclusion)—a term coined by Ted Nelson in 1963—is the inclusion of one document inside another by reference. Instead of copying content, you point to it. The source stays authoritative; the reference stays current.

Most formats don't support this natively. Liaison fixes that.

**Phase 1** _(you are here)_  
CLI transclusion between HTML and plaintext files. Define blocks with `id`, reference them with `transclude`, run the command. Useful, harmless, nothing to see here.

**Phase 2**  
Web content. Fetch from remote URLs with caching and authentication. Full CSS selector support for extracting content from any webpage. Now we can point at _anything on the internet_.

**Phase 3**  
More formats. XML (XPath), JSON (JSONPath), CSV (cell ranges), spreadsheets, maybe even Word docs. If it can be parsed, it can be transcluded.

**Phase 4**  
A daemon. Watches your files and folders for changes. Re-materializes references automatically. Support for transcluding entire files into directories. Keeps everything synchronized. While you sleep.

**Phase 5**  
Graph database overlay. The transclusion graph becomes its own persistent layer—point to things that live only in the graph, or pull external content into the graph by pointing at it. Track the live version, the archived version, or both. The graph remembers everything you've ever referenced.

**Phase 6**  
At this point, files are just... views. The graph is the source of truth. Liaison _is_ the filesystem muahahaha.

## License

Dual-licensed under MIT or Apache 2.0 (your choice).
