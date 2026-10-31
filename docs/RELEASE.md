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

### 0.2.4 release status

`lume-code@0.2.4` is published and is the npm `latest` version. GitHub Actions run `41241691359` succeeded at commit `6039d22`, including all three native build jobs and the npm publish job. The account owner configured npm trusted publishing for GitHub Actions (`Selfnify/Lume-code`, `native-release.yml`) with publish permission before that run.

Verified the package through npm: `npm view lume-code version dist-tags dist.tarball --json` reports version/latest `0.2.4`. `npm pack lume-code@0.2.4 --dry-run --json` reports all nine files, including Windows x64, Intel macOS, and Apple Silicon executables. Both macOS binaries have executable mode `755`. Package size is 34,534,816 bytes (183,308,307 unpacked); shasum `e0463b3f059f2e762bf30e8157f18a45279e55f7`.

Two release runs overlapped. Run `41241949289` also built successfully, but its publish job failed after run `41241691359` had published `0.2.4`. npm versions are immutable, so a second publish of the same version is expected to fail; this does not undo the successful publish. Do not retry or republish `0.2.4`.

The workflow cleanup that removes setup-node's auth-token placeholder was added in commit `3c41cec`, after the successful run's source commit. It is retained as a defensive OIDC configuration improvement for future releases, but should not be described as a proven cause/fix for the successful `0.2.4` publish. For future versions, push one version tag once and avoid overlapping release runs.

The npm account owner configured the trusted publisher with 2FA using:

```powershell
npm trust github lume-code --file native-release.yml --repository Selfnify/Lume-code --allow-publish --yes
```

The owner completed npm authentication and npm confirmed GitHub Actions trust for repository `Selfnify/Lume-code`, workflow `native-release.yml`, with `publish` and `stage publish` permissions (ID `c9f404a2-b49c-4d0b-8cc9-281765a5d75d`). A subsequent full run was triggered after trust configuration. A defensive setup-node auth-token cleanup is also on `main` as `3c41cec`; the tag points to that workflow improvement for future runs. The successful publish is verified above. The later overlapping run failed at publish after the same version had already been published, so it should not be retried.

## Configure npm trusted publishing

For `npm publish --provenance` from GitHub Actions, configure the `lume-code` package on npmjs.com under **Package settings → Trusted publishing**:

- Publisher: GitHub Actions
- Organization or user: `Selfnify`
- Repository: `Lume-code`
- Workflow filename: `native-release.yml`
- Allow direct `npm publish`

The publisher values must match exactly. The workflow grants `id-token: write`, and its publish job installs npm 11.5.1 or newer, as required for npm OIDC trusted publishing. The package metadata must keep `repository.url` set to `https://github.com/Selfnify/Lume-code.git`. If the npm job returns `E404`, check these trusted-publisher settings first; npm may return that status when the OIDC identity or package authorization does not match.

`actions/setup-node` writes an `_authToken` placeholder into its temporary `.npmrc` when `registry-url` is set. The publish job removes that line and the obsolete `always-auth` entry before publishing, so npm can perform the OIDC trusted-publishing exchange. Keep this cleanup step in place for the token-free OIDC flow.

For a manual local publish instead, build and stage binaries for every supported OS/architecture first, then from `npm/` run `npm publish --access public`. The release workflow is preferred because it builds and tests each native target on its matching runner and publishes a single package containing all targets.
