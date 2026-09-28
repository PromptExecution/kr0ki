# Valigate: Validate + Gate + Date

**Valigate** is a pre-commit validation circuit that ensures code quality, requirement compliance, and proper versioning before allowing commits. It's a small circuit of conditions and contextually aware governance documents.

## Concept

The name "valigate" combines three essential operations:

1. **Validate** - Check that code meets quality standards
2. **Gate** - Block commits that don't pass validation
3. **Date** - Timestamp successful validations for traceability

## Installation

### Prerequisites

Install [prek](https://github.com/j178/prek):

```bash
# Install prek (check their repo for latest installation method)
cargo install prek
# or
brew install prek
# or follow instructions at https://github.com/j178/prek
```

### Setup

1. Install prek hooks:

```bash
cd /home/brianh/promptexecution/kr0ki
prek install
```

2. Verify installation:

```bash
prek run valigate
```

## Validation Circuit

The valigate circuit performs the following checks:

### 1. Rust Formatting
- Ensures all Rust code follows `cargo fmt` standards
- Blocks commits with formatting issues

### 2. Rust Linting
- Runs `cargo clippy` with strict warnings
- Ensures code quality and best practices

### 3. Rust Tests
- Runs all workspace tests
- Blocks commits that break tests

### 4. Requirements Validation
- Checks that requirement documents follow the template
- Validates requirement IDs and structure
- Ensures traceability

### 5. Version Sync
- Verifies Cargo.toml version is reflected in UI
- Ensures consistency across the project

### 6. Sensitive Data Detection
- Scans for potential secrets or credentials
- Warns about sensitive data in staged files

## Configuration

The valigate circuit is configured in `.prek-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: valigate
        name: valigate (validate + gate + date)
        entry: ./scripts/valigate.sh
        language: script
        pass_filenames: false
        always_run: true
        stages: [pre-commit]
```

## Scripts

### valigate.sh

Main validation script that orchestrates all checks.

**Location:** `scripts/valigate.sh`

**Exit codes:**
- `0` - All checks passed (commit allowed)
- `1` - One or more checks failed (commit blocked)

### validate-requirements.sh

Validates requirement documents for proper format.

**Location:** `scripts/validate-requirements.sh`

**Checks:**
- Main title presence
- Overview section
- Requirement ID format (FR-001, NFR-001, etc.)
- Priority fields
- Acceptance criteria

### check-version-sync.sh

Ensures version consistency across the project.

**Location:** `scripts/check-version-sync.sh`

**Checks:**
- Cargo.toml version extraction
- Playbook Vite config reads version
- App.vue displays version

## Usage

### Manual Validation

Run valigate manually before committing:

```bash
./scripts/valigate.sh
```

### Automatic Validation

With prek installed, valigate runs automatically on every commit:

```bash
git commit -m "your message"
# valigate runs automatically
```

### Skip Validation (Emergency Only)

To bypass valigate in emergencies:

```bash
git commit --no-verify -m "emergency fix"
```

**Warning:** Only use this for critical hotfixes. All commits should pass valigate.

## Output Example

```
╔═══════════════════════════════════════════════════════════════╗
║                    VALIGATE CIRCUIT                           ║
║              validate + gate + date                           ║
╚═══════════════════════════════════════════════════════════════╝

[1/6] Checking Rust formatting...
✓ Rust code is properly formatted

[2/6] Running Rust linter...
✓ No clippy warnings

[3/6] Running Rust tests...
✓ All tests pass

[4/6] Validating requirements...
✓ Requirements README exists
✓ Requirements template exists
✓ System-normal requirements exist
✓ Requirements follow naming convention

[5/6] Checking version sync...
ℹ Cargo.toml version: 0.0.1
✓ Playbook Vite config reads version from Cargo.toml

[6/6] Checking for sensitive data...
✓ No obvious sensitive data detected

═══════════════════════════════════════════════════════════════

╔═══════════════════════════════════════════════════════════════╗
║                    ✓ VALIGATE PASSED                          ║
╚═══════════════════════════════════════════════════════════════╝

  Commit is gated and dated: 2026-09-26T07:30:45Z
```

## Governance

Valigate enforces governance through:

1. **Quality Gates** - Code must pass all checks before commit
2. **Requirement Traceability** - Requirements must be properly formatted
3. **Version Consistency** - Version must be synchronized across components
4. **Audit Trail** - Each validation is timestamped

## Extending Valigate

To add new validation checks:

1. Create a new script in `scripts/`
2. Add it to `.prek-config.yaml`
3. Update this documentation

Example:

```yaml
- id: my-custom-check
  name: my custom check
  entry: ./scripts/my-custom-check.sh
  language: script
  pass_filenames: true
  files: '.*\.rs$'
  stages: [pre-commit]
```

## Troubleshooting

### Valigate fails but code is correct

Check if dependencies are up to date:

```bash
cargo update
cargo build
```

### Tests fail in valigate but pass locally

Ensure you're running the same test command:

```bash
cargo test --workspace
```

### Version sync warning

Rebuild the playbook:

```bash
cd playbook
npm run build
```

## Related Documentation

- [Requirements Documentation](docs/requirements/README.md)
- [Requirements Template](docs/requirements/TEMPLATE.md)
- [System-Normal Requirements](docs/requirements/system-normal.md)
