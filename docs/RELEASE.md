# Build and release Lume Code

## Build the native binaries

Each release contains three platform folders:

- `win32-x64` for Windows x64
- `darwin-arm64` for Apple Silicon Macs
- `darwin-x64` for Intel Macs

Build on the matching operating system and architecture. From the repository root, the macOS commands used by the release workflow are:

```sh
cargo test --locked
npm test --prefix npm
cargo build --release --locked
cargo build --release --locked --manifest-path third-party/ast-grep/Cargo.toml -p ast-grep
mkdir -p npm/native/darwin-arm64
cp target/release/lume-code third-party/ast-grep/target/release/ast-grep npm/native/darwin-arm64/
chmod +x npm/native/darwin-arm64/*
```

For an Intel Mac, change the destination folder to `npm/native/darwin-x64`. Run these commands on an Apple Silicon Mac for `darwin-arm64`, and on an Intel Mac for `darwin-x64`. The outputs are native binaries; do not rename an ARM build as an Intel build or the other way around.

The automated release runs those Cargo and npm tests and build commands on GitHub-hosted runners: `macos-latest` for Apple Silicon, `macos-15-intel` for Intel, and `windows-latest` for Windows x64. Each runner uploads its platform folder. The publish job downloads all three folders, checks that the package contains them, and publishes the complete package. Consumers install the matching binary through npm; they do not need Rust installed.

## Publish a version

Keep the version in the root `Cargo.toml` and `npm/package.json` in sync. Commit the release on `main`, then create and push a version tag from that commit:

```sh
git switch main
git pull --ff-only origin main
# Update the Cargo.toml and npm/package.json versions to the same next version.
git add Cargo.toml Cargo.lock npm/package.json
git commit -m "Release Lume Code <version>"
git tag -a v<version> -m "Lume Code <version>"
git push origin main
git push origin v<version>
```

The tag starts `.github/workflows/native-release.yml`. Check the Actions run and confirm all three native builds and the `publish` job pass. Do not reuse a version that npm already has; npm versions cannot be overwritten.

## Configure npm trusted publishing

For `npm publish --provenance` from GitHub Actions, configure the `lume-code` package on npmjs.com under **Package settings → Trusted publishing**:

- Publisher: GitHub Actions
- Organization or user: `Selfnify`
- Repository: `Lume-code`
- Workflow filename: `native-release.yml`
- Allow direct `npm publish`

The publisher values must match exactly. The workflow grants `id-token: write`, and its publish job installs npm 11.5.1 or newer, as required for npm OIDC trusted publishing. The package metadata must keep `repository.url` set to `https://github.com/Selfnify/Lume-code.git`. If the npm job returns `E404`, check these trusted-publisher settings first; npm may return that status when the OIDC identity or package authorization does not match.

For a manual local publish instead, build and stage binaries for every supported OS/architecture first, then from `npm/` run `npm publish --access public`. The release workflow is preferred because it builds and tests each native target on its matching runner and publishes a single package containing all targets.
