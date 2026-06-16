# Dedicated Server Multiplayer PR Delivery

## PR Link

Create the pull request here:

https://github.com/AstralArtisan/block-city-adventure/compare/main...StevenHuang233:codex/dedicated-server-multiplayer?expand=1

Use these PR options:

- Base repository: `AstralArtisan/block-city-adventure`
- Base branch: `main`
- Head repository: `StevenHuang233/block-city-adventure`
- Compare branch: `codex/dedicated-server-multiplayer`

Suggested PR title:

```text
feat: add dedicated multiplayer server deployment
```

## Delivery Scope

This delivery adds cloud-hosted multiplayer server support for the project.

Included changes:

- Split the game into explicit `client` and `server` binaries.
- Add a headless dedicated server entrypoint.
- Support public server startup for both multiplayer modes:
  - PVP server on UDP `3456`
  - Coop server on UDP `3457`
- Keep normal client startup compatible with the in-game GUI connection flow.
- Add Linux deployment scripts and server configuration examples.
- Add startup and Aliyun Linux deployment documentation.
- Address review feedback:
  - Restore the Chinese client window title: `勇闯方块城`
  - Fix newly introduced clippy warnings
  - Reject non-finite PVP input packets to avoid NaN/Inf state pollution
  - Centralize PVP player max HP into one constant source

## Branch And Commits

Fork branch:

```text
StevenHuang233/block-city-adventure
codex/dedicated-server-multiplayer
```

Current delivery commits:

```text
b1010dcf fix: address multiplayer server PR review
41368b43 feat: add dedicated multiplayer server deployment
```

## Verification

Verified locally:

```powershell
cargo fmt --check
cargo check --bin server
cargo check --bin client
cargo clippy --all-targets -- -D warnings
```

Not run in this pass:

```powershell
cargo test
```

## Server Deployment Notes

For a Linux cloud server, open these inbound firewall or security group rules:

- UDP `3456` for PVP
- UDP `3457` for Coop
- TCP `22` only for SSH administration

Typical server startup after uploading the server bundle:

```bash
tar -xzf block-city-server-linux.tar.gz
cd block-city-adventure
chmod +x run-server.sh
./run-server.sh --pvp-server
```

For Coop:

```bash
./run-server.sh --coop-server
```

The server should remain running. Clients can reconnect without restarting the server.

## Client Startup And Connection

Normal local client startup:

```powershell
cargo run --bin client
```

Then use the in-game multiplayer GUI:

- Server IP: public server IP
- PVP port: `3456`
- Coop port: `3457`

No extra command-line client arguments are required for the normal GUI connection flow.

## PR Description

Paste this into the GitHub PR description:

```markdown
## Changed

This PR adds dedicated multiplayer server support and deployment helpers.

Main changes:

- Split the app into explicit `client` and `server` binaries.
- Add a headless dedicated server entrypoint.
- Support public Coop/PVP server startup modes:
  - PVP: UDP `3456`
  - Coop: UDP `3457`
- Keep the normal client startup flow compatible with GUI connection usage.
- Add Linux deployment scripts and docs for running the server on a cloud machine.
- Add basic server config examples, startup scripts, and systemd service template.
- Address review feedback for the client title, clippy warnings, PVP input validation, and duplicated PVP HP constants.

## Impact

This mainly affects:

- Multiplayer networking startup flow
- Coop/PVP server runtime behavior
- App entrypoint structure
- Linux server deployment workflow

The normal client can still be started as the game client and connect through the in-game UI.

## Tests

Verified locally:

- `cargo fmt --check`
- `cargo check --bin server`
- `cargo check --bin client`
- `cargo clippy --all-targets -- -D warnings`

## Risks / Known Issues

- This touches Coop/PVP networking code, so online multiplayer should be retested after merge.
- Full `cargo test` was not run in this local pass.
- Cloud deployment still requires opening UDP ports `3456` and/or `3457` in the server security group/firewall.

## Retest Needed

Please retest:

- Client connecting to a public PVP server on UDP `3456`
- Client connecting to a public Coop server on UDP `3457`
- Repeated client reconnect without restarting the server
- Basic single-player/client startup flow
```

## Merge Notes

After opening the PR, merging requires write permission on `AstralArtisan/block-city-adventure`.

If the GitHub page shows no conflicts and checks pass:

1. Open the PR page.
2. Click `Create pull request`.
3. Wait for checks/review.
4. A maintainer with write permission clicks `Merge pull request`.
