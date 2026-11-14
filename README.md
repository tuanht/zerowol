# zerowol

A tiny Wake-on-LAN and remote shutdown server for your NAS, with a mobile-first PWA, a JSON API and an optional agent for your PCs. One static binary (under 5 MB image), no database, no external services.

- Wake machines with a magic packet.
- Power off or reboot machines through a small signed agent (HMAC-SHA256, replay-protected).
- Live status: online, agent paired, hostname, uptime.
- Schedules: wake, reboot or power off on a timetable (daily, weekdays, weekends, weekly, once).
- Installable on a phone home screen (iOS and Android).

**LAN/Tailscale only — never forward port 8090.** zerowol has no login screen by default; anyone who can reach the port can power machines on and off. Reach it from outside through Tailscale or a VPN, and set `ZEROWOL_REQUIRE_TOKEN=true` if you want a stricter API.

## Quick start

Requires Docker with Compose. Prebuilt images for `linux/amd64` and `linux/arm64` are published to GHCR as `ghcr.io/tuanht/zerowol`:

```sh
mkdir zerowol && cd zerowol
curl -fsSLO https://raw.githubusercontent.com/tuanht/zerowol/master/deploy/docker-compose.yml
mkdir -p data && sudo chown 1026:100 data   # or set PUID/PGID to your user
docker compose up -d
```

Open `http://<nas-ip>:8090/`. The container runs with host networking (needed to send WoL broadcasts), read-only root, no capabilities, and a 32 MB memory limit. State lives in `./data/state.json`.

| Tag | Meaning |
|---|---|
| `latest` | newest stable release |
| `X.Y.Z` | an exact release (recommended for pinning: `ZEROWOL_TAG=0.1.0 docker compose up -d`) |
| `X.Y`, `X` | newest release in that minor / major line |
| `edge` | latest build of `master` |
| `sha-<commit>` | one specific build of `master` |

Update with `docker compose pull && docker compose up -d`. Each release on GitHub also carries static server binaries for both architectures.

To build from source instead, the repository's own [`docker-compose.yml`](docker-compose.yml) uses the `prebuilt` image target (run `make dist` first, see [Building](#building), then `docker compose up -d --build`), or `docker build -t zerowol:latest .` builds everything inside Docker (the default `runtime` stage).

### Server settings

| Variable | Default | Meaning |
|---|---|---|
| `ZEROWOL_PORT` | `8090` | listen port |
| `ZEROWOL_BIND` | `0.0.0.0` | listen address |
| `ZEROWOL_DATA` | `/data` | state directory |
| `ZEROWOL_REQUIRE_TOKEN` | `false` | require a bearer token for non-UI API calls |
| `TZ` | unset | default schedule timezone, an IANA name such as `Europe/Berlin` (`UTC` / `Etc/UTC` count as unset); a timezone saved in the UI or via `PATCH /api/v1/settings` takes precedence |

## Enabling Wake-on-LAN on the PC

1. Enable Wake-on-LAN (sometimes "Power on by PCI-E/LAN") in the BIOS/UEFI.
2. Enable magic-packet wake on the NIC:

   ```sh
   sudo ethtool -s <if> wol g
   ```

3. Make it persistent with NetworkManager:

   ```sh
   nmcli connection modify "<connection name>" 802-3-ethernet.wake-on-lan magic
   ```

Check with `ethtool <if> | grep Wake-on` (should show `g`). Wake-on-LAN generally works over wired Ethernet only.

## Adding a machine and installing the agent

1. In the web UI, add a machine: name, MAC address, IP address, and tick **agent** if you want shutdown/reboot.
2. The UI shows an install command. Run it on the PC as root:

   ```sh
   curl -fsSL http://<nas-host>:8090/agent/install/<id>.sh | sudo sh
   ```

   The script downloads the matching agent binary (x86_64 or aarch64), verifies its SHA-256, writes `/etc/zerowol-agent.toml` (secret, port, allowed NAS IP), installs and starts a hardened systemd unit, and opens the agent port in firewalld for the NAS only if firewalld is running. It is safe to re-run.

   The command uses the `Host` header you opened the UI with, so open the UI via the address the PC can reach (LAN IP or Tailscale name).
3. The machine's badge turns to "paired" once the agent answers.

To rotate an agent's secret, use the machine's rotate action in the UI (or `POST /api/v1/machines/<id>/agent/rotate`) and run the new command on the PC.

### Uninstall

```sh
sudo zerowol-agent uninstall
```

Stops and removes the service, config, firewall rule and binary.

## Troubleshooting

- **Machine shows offline although the agent is running.** If the NAS and the PC both run Tailscale and the NAS advertises your LAN as a subnet route (`192.168.23.0/24`), a PC with `--accept-routes` installs that route into Tailscale's routing table, which is consulted before the main table. The PC then sends its replies to the NAS (SYN-ACKs, so even the TCP probe fails) through `tailscale0` instead of the LAN, and no connection ever completes. Check `ip route get <nas-ip>`: it must say `dev <lan-nic>`, not `tailscale0`. Fix with `sudo tailscale set --accept-routes=false` on the PC.

## API and tokens

All endpoints live under `/api/v1`. Errors are JSON: `{"error":"<message>"}`.

```sh
BASE=http://<nas-ip>:8090/api/v1

# list machines (with status)
curl -s $BASE/machines

# wake / status / poweroff / reboot
curl -s -X POST $BASE/machines/<id>/wake
curl -s $BASE/machines/<id>/status
curl -s -X POST $BASE/machines/<id>/poweroff     # 202; one per machine per 10 s (429)
curl -s -X POST $BASE/machines/<id>/reboot
```

Status looks like `{"online":true,"agent":"paired","hostname":"pc","uptime":1234}`. If the PC's clock is out of sync with the NAS, power off fails with the agent's reason (for example `clock skew 45s`) as a `502` error. `/shutdown` is kept as an alias for `/poweroff`.

### Schedules

Rules turn a machine on, reboot it or power it off at a wall-clock time. They need a timezone: set one in the UI settings, via the API, or by running the container with a real `TZ` (not `UTC`). Without any timezone, creating a rule returns `409 timezone not set` and nothing fires.

```sh
curl -s $BASE/settings                                    # {"timezone":null,"source":null}
curl -s -X PATCH $BASE/settings -H 'content-type: application/json' -d '{"timezone":"Europe/Berlin"}'

# daily wake at 08:00 (repeat: once+date, daily, weekdays, weekends, weekly+days)
curl -s -X POST $BASE/schedules -H 'content-type: application/json' \
  -d '{"machine_id":"<id>","action":"wake","time":"08:00","repeat":{"kind":"daily"}}'
curl -s -X POST $BASE/schedules -H 'content-type: application/json' \
  -d '{"machine_id":"<id>","action":"poweroff","time":"23:30","repeat":{"kind":"weekly","days":["mon","fri"]}}'
curl -s $BASE/schedules

# what happens in a date range (at most 62 days), including skips and outcomes
curl -s "$BASE/schedules/occurrences?from=2027-03-01&to=2027-03-31"

# skip one date, and undo it
curl -s -X POST $BASE/schedules/<sid>/skip -H 'content-type: application/json' -d '{"date":"2027-03-05"}'
curl -s -X DELETE $BASE/schedules/<sid>/skip/2027-03-05
```

Times are `HH:MM` and dates `YYYY-MM-DD`, always in the schedule timezone; changing the timezone keeps each rule's wall time. A wall time that does not exist (DST gap) fires at the first valid instant after it, and one that occurs twice fires once, at the earlier instant. Reboot and power off need the agent. A one-off rule must lie in the future. A rule that fires within 2 minutes of its time counts; if zerowol was down or busy longer, that occurrence is logged `missed` and is not caught up later (at startup only the last 7 days are examined). Removing a machine's agent disables its reboot and power off rules; deleting a machine deletes its rules.

### Tokens

Create tokens in the UI settings (Settings, API Tokens), or via the API. The `curl` example below works only in the default open mode; with `ZEROWOL_REQUIRE_TOKEN=true` create tokens from the web UI.

```sh
curl -s -X POST $BASE/tokens -H 'content-type: application/json' -d '{"name":"automation"}'
# {"id":"...","name":"automation","token":"zw_..."}   (shown once)

curl -s -X POST -H "Authorization: Bearer zw_..." $BASE/machines/<id>/wake
```

With `ZEROWOL_REQUIRE_TOKEN=true`, API requests without a bearer token are rejected unless they come from the web UI itself (same-origin, or the `zw_ui` cookie the UI page sets, which also makes it work over plain HTTP). Token management endpoints never accept a bearer token.

## Migrating from zeroclaw's `wolbot`

Replace the `wolbot` sidecar with zerowol: create a token, then point the zeroclaw skill's wake and power off calls at `POST /api/v1/machines/<id>/wake` and `POST /api/v1/machines/<id>/poweroff` on the zerowol server with the header `Authorization: Bearer <token>`. Machine ids come from `GET /api/v1/machines`.

## Building

```sh
make test     # fmt check, clippy -D warnings, tests
make dist     # static x86_64 binary with both agents embedded -> dist/zerowol (needs zig + cargo-zigbuild)
make image    # docker build -t zerowol:latest .
make smoke    # end-to-end test: server + agent on loopback (uses port 9009)
make web      # rebuild the web UI into crates/zerowol/web/dist (npm ci, vitest, vite build)
make web-check # make web, then fail if the committed dist/ differs or exceeds the size budget
make web-dev  # Vite dev server with hot reload, proxying /api to `make dev` on port 8090
```

Node 22 is needed only for UI work: the built web UI (`crates/zerowol/web/dist/app.js` and `app.css`) is committed, so `cargo build`, `make dist` and the Docker build need only Rust. After changing anything in `crates/zerowol/web/src`, run `make web` and commit `dist/` with it; CI (`make web-check`) fails when the committed files differ from a fresh build.

The toolchain (Rust 1.99.1 with the musl targets) is pinned by `rust-toolchain.toml`. For an arm64 NAS: `docker buildx build --platform linux/arm64 -t zerowol:latest .` (the builder cross-compiles natively, no QEMU needed).

### Releases

`.github/workflows/release.yml` publishes multi-arch images to GHCR: every push to `master` updates `edge` and `sha-<commit>`; pushing a tag `vX.Y.Z` (which must match the workspace `version` in `Cargo.toml`) publishes `X.Y.Z`, `X.Y`, `X` and `latest` and creates a GitHub release with binaries and `SHA256SUMS`. Pre-release tags such as `v0.2.0-rc.1` get only their exact tag and never move `latest`.

```sh
git tag v0.1.0 && git push origin v0.1.0
```

GHCR packages start private. After the first publish, make the package public under the repository's Packages settings on GitHub.

The web UI is a Vue 3 + Framework7-Vue app in `crates/zerowol/web/src`, built with `make web` into the committed `web/dist/` and embedded into the binary (Vue and [Framework7](https://framework7.io) are MIT-licensed). The theme automatically matches iOS or Android (Material) based on the device, and can be overridden in Settings.

### Development server with fake machines

`make dev` runs a local server (data in `target/dev-data`, port 8090) built with the `dev-fakes` feature and `ZEROWOL_DEV_FAKES=1`. It seeds five fake machines (ids starting with `dev-`, IPs in the unroutable 192.0.2.0/24) whose status and power actions are simulated: a paired Workstation that powers off/reboots, a Media PC that wakes, a Laptop with an unreachable agent, a Gaming PC whose agent rejects requests (clock skew), and an Old Server that never wakes (to see the timeout). If no schedule timezone is saved yet, it also saves `Europe/Berlin`, so the Schedules tab works straight away. The feature is off by default, so release, `make dist` and Docker builds don't contain this code and ignore the variable.

## Screenshots

Screenshots are not committed. Generate them against a throwaway instance (the script deletes all machines on its target):

```sh
ZEROWOL_DATA=$(mktemp -d) ZEROWOL_PORT=18090 ZEROWOL_BIND=127.0.0.1 cargo run -p zerowol &
NODE_PATH=$(npm root -g) node scripts/ui-check.mjs http://127.0.0.1:18090
```

See `scripts/ui-check.mjs`; output goes to `./screenshots/`.

## License

MIT (see `Cargo.toml`).
