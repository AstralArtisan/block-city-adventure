# Client / Server Startup

The player client keeps the original in-game menu: single-player and
multiplayer are both selected from `client.exe`. The dedicated server is a
separate process used only for authoritative coop sessions.

The package now also builds dedicated binary targets:

```powershell
cargo run --bin server
cargo run --bin client
cargo run --bin client -- 127.0.0.1 --client-id 1
```

In a release build these become `server.exe` and `client.exe` under
`target/release/`. There is no third gameplay executable in the intended
release layout.

## Coop

Coop uses Lightyear on UDP `3457`. The server command starts a dedicated
authoritative server that does not occupy a player slot. Start two clients with
different client ids: `1` maps to P1 and `2` maps to P2.

```powershell
cargo run --bin server
cargo run --bin client -- 127.0.0.1 --client-id 1
cargo run --bin client -- 127.0.0.1 --client-id 2
```

For LAN play, replace `127.0.0.1` with the server machine's IPv4 address.
If `client.exe` is launched without an address, it opens the normal game menu
instead of auto-joining a server.

During gameplay, the authoritative server also watches player input freshness.
If a required client stops sending input for roughly 5 seconds, the coop session
is treated as disconnected and exits back to the lobby/main flow.

## PVP

PVP uses the custom UDP protocol on UDP `3456`.

```powershell
cargo run -- --pvp-server
cargo run -- --pvp-client 127.0.0.1
```

## Generic Form

The same startup path also accepts a generic mode/role form:

```powershell
cargo run -- --net coop-server
cargo run -- --net coop-client --host 127.0.0.1 --client-id 1
cargo run -- --net coop-client --host 127.0.0.1 --client-id 2
cargo run -- --net pvp-server
cargo run -- --net pvp-client --host 127.0.0.1
```

For Coop, `server` means dedicated authoritative server and `host` means the
original listen-host flow where the host process also controls P1:

```powershell
cargo run -- --net coop --role server
cargo run -- --net coop --role host
cargo run -- --net coop --role client --host 127.0.0.1 --client-id 1
```

## Window Placement

For side-by-side local testing, pass a window position:

```powershell
cargo run -- --coop-server --window-pos 40,40
cargo run -- --coop-client 127.0.0.1 --client-id 1 --window-pos 980,40
cargo run -- --coop-client 127.0.0.1 --client-id 2 --window-pos 980,560
```

Or with the dedicated binaries:

```powershell
cargo run --bin server -- --window-pos 40,40
cargo run --bin client -- 127.0.0.1 --client-id 1 --window-pos 980,40
cargo run --bin client -- 127.0.0.1 --client-id 2 --window-pos 980,560
```

Door transitions require both living players to confirm the same door. A single
pending door confirmation times out after 2 seconds so an accidental `E` press
does not leave the team stuck in the door-choice phase.

The old environment-variable workflow is still supported:

```powershell
$env:LOCAL_NET_DEBUG="1"
$env:LOCAL_NET_DEBUG_MODE="coop"
$env:LOCAL_NET_DEBUG_ROLE="server"
cargo run
```
