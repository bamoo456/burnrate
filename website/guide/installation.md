# Installation

## Native bundles

Download the bundle for your platform from
[GitHub Releases](https://github.com/bamoo456/burnrate/releases/latest):

- **macOS** — `.dmg` (universal).
- **Linux** — `.AppImage`, `.deb`, and `.rpm`.
- **Windows** — `.msi` installer.

::: warning Fork status
This fork has not published a release yet, and its release workflow has no
Apple signing secrets configured — so any bundle it produces today is
**unsigned**, which means Gatekeeper will warn on first open and the keychain
"Always Allow" grant will not persist. Building from source has the same
property.
:::

## From source

This fork is **not published to crates.io** (the `burnrate` crate name belongs
to upstream), so `cargo install burnrate` installs the upstream build. Build it
directly instead:

```sh
git clone https://github.com/bamoo456/burnrate && cd burnrate
npm install
./scripts/package-dmg     # macOS .app + .dmg
# or: ./scripts/build-app # bare release binary
```

## With Nix

The repository is a flake exposing the package for `x86_64-linux`,
`aarch64-linux`, and `aarch64-darwin`:

```sh
nix run github:bamoo456/burnrate
```

## Automatic updates (macOS)

::: warning Not active in this fork
Auto-update needs a minisign keypair whose public half is committed to
`tauri.conf.json`. This fork still carries upstream's public key and has no
releases, so update checks find nothing. The machinery below works once a
fork-owned key and the `TAURI_SIGNING_PRIVATE_KEY` secrets are in place.
:::

Native macOS installs update themselves: a dismissible banner and the tray's
**Check for Updates…** entry offer a signature-verified one-click **Install &
Restart**.

Two channels are available under **Preferences → Updates**:

- **Stable** — tagged releases.
- **Nightly** — a rolling pre-release built from `main` after CI passes.

::: tip Keychain and code signatures
On macOS the keychain binds its "Always Allow" grant to the app's code
signature, so a signed install remembers the grant. An unsigned build
re-prompts on every launch — see
[Troubleshooting](/guide/troubleshooting#keychain-re-prompts-on-every-launch).
:::
