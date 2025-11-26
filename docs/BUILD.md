# Building Documentation

This directory contains comprehensive Doctave documentation for Rusthon.

## Prerequisites

Install Doctave:

### Linux
```bash
curl -L https://github.com/Doctave/doctave/releases/download/0.4.2/doctave-0.4.2-x86_64-unknown-linux-gnu.tar.gz -o doctave.tar.gz
tar -xzf doctave.tar.gz
sudo mv doctave /usr/local/bin/
chmod +x /usr/local/bin/doctave
rm doctave.tar.gz
```

### macOS
```bash
curl -L https://github.com/Doctave/doctave/releases/download/0.4.2/doctave-0.4.2-x86_64-apple-darwin.tar.gz -o doctave.tar.gz
tar -xzf doctave.tar.gz
sudo mv doctave /usr/local/bin/
chmod +x /usr/local/bin/doctave
rm doctave.tar.gz
```

### Using Devcontainer

Doctave is pre-installed in the devcontainer. Just open the project in VS Code with the Remote-Containers extension.

## Building Documentation

### Development Server

Start a local development server with hot reload:

```bash
cd docs
doctave serve
```

Then open http://localhost:4001 in your browser.

### Build Static Site

Build the documentation to static HTML:

```bash
cd docs
doctave build
```

Output will be in `docs/_site/`.

## Documentation Structure

```
docs/
├── doctave.yaml              # Configuration
├── README.md                 # Home page
├── getting-started/
│   ├── README.md
│   ├── installation.md
│   ├── quick-start.md
│   └── your-first-program.md
├── architecture/
│   ├── README.md
│   ├── compilation-pipeline.md
│   ├── type-system.md
│   └── memory-model.md
├── language-features/
│   └── README.md
├── implementation/
│   └── (future implementation guides)
├── testing/
│   └── README.md
├── contributing.md
├── limitations.md
└── roadmap.md
```

## Writing Documentation

### Markdown Files

All documentation is written in Markdown. Place files in appropriate directories.

### Links

Use relative links:
```markdown
See [Architecture](/architecture) for details.
See [Type System](/architecture/type-system) for more.
```

### Code Blocks

Use fenced code blocks with language specification:

````markdown
```python
def hello():
    print("Hello, World!")
```

```rust
fn main() {
    println!("Hello, World!");
}
```

```llvm
define i32 @main() {
  ret i32 0
}
```
````

### Tables

```markdown
| Feature | Status |
|---------|--------|
| Integers | ✅ Supported |
| Lists | ❌ Not supported |
```

### Navigation

Update `doctave.yaml` to add pages to navigation:

```yaml
navigation:
  - path: "/"
  - path: "/getting-started"
    children:
      - "/getting-started/installation"
      - "/getting-started/quick-start"
```

## Deployment

### GitHub Pages

```bash
# Build static site
cd docs
doctave build

# Deploy to GitHub Pages
# (configure GitHub Pages to serve from docs/_site)
```

### Netlify / Vercel

Connect your repository and configure:
- **Build command**: `cd docs && doctave build`
- **Publish directory**: `docs/_site`

## Next Steps

- Read the generated documentation at http://localhost:4001
- Contribute new pages (see [Contributing](/contributing))
- Report documentation issues on GitHub
