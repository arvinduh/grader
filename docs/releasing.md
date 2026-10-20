# Release Process

`bsg` releases are automated via
[cargo-dist](https://github.com/axodotdev/cargo-dist) and GitHub Actions. Built
binaries are distributed directly through GitHub Releases and consumed by
`bsg update` using [axoupdater](https://github.com/axodotdev/axoupdater).

---

## Pre-flight Verification

Before cutting a release, ensure the local working tree is clean and passes all
lint and formatting gates:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

---

## Release Steps

1. **Bump the version in `Cargo.toml`**: Update `version = "0.1.X"` in
   `Cargo.toml`.

2. **Update `Cargo.lock`**:

   ```bash
   cargo check
   ```

3. **Commit the version bump**:

   ```bash
   git add Cargo.toml Cargo.lock
   git commit -m "chore(release): bump version to 0.1.X"
   ```

4. **Tag the release commit**: Tags must follow the `v<major>.<minor>.<patch>`
   format:

   ```bash
   git tag v0.1.X
   ```

5. **Push commit and tag to GitHub**:

   ```bash
   git push origin main --tags
   ```

---

## Automated CI/CD Pipeline

Pushing a `v*` tag triggers `.github/workflows/release.yml`:

1. **Matrix Builds**:
   - macOS Apple Silicon (`aarch64-apple-darwin`)
   - Windows x64 (`x86_64-pc-windows-msvc`)
2. **Installers & Hashes**:
   - Generates `grader-installer.sh` (Unix shell installer)
   - Generates `grader-installer.ps1` (PowerShell installer)
   - Computes SHA256 checksums
3. **GitHub Release Publication**:
   - Creates the release entry on GitHub.
   - Uploads compiled binaries, installers, and checksums.

---

## End-User Updates

- **New Users**: Install via the one-line curl or PowerShell scripts listed in
  [`README.md`](../README.md#installation).
- **Existing Users**: Update in place by running:

  ```bash
  bsg update
  ```

---

## Main Branch Protection (Lock)

In GitHub repository settings under **Settings** $\to$ **Rules** $\to$
**Rulesets**:

1. Target branch: `main`.
2. Enable **Require status checks to pass**:
   - Require status check: `Lint & Format` (from `.github/workflows/ci.yml`).
3. Enable **Block force pushes** and **Require pull request**.
