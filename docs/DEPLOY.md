# Deploying to Fly.io

The game runs as one small machine on Fly.io: the server plus the browser client, built from the
`Dockerfile` on Fly's own builders. `fly.toml` holds every cloud-only setting; run locally, the
server keeps its safe defaults (this machine only, no passphrase).

**Cost:** the smallest machine (shared CPU, 256MB) is many times more than 4 matches need (a match
uses well under 1% of a CPU and ~8MB of memory). It stops when nobody's connected and starts when
someone opens the page, so you mostly pay for the hours people play: a few cents to a dollar or
two a month at light use. The first visitor after an idle spell waits a few seconds for it to wake.

## One-time setup

1. **Install flyctl and log in** (PowerShell):
   ```powershell
   iwr https://fly.io/install.ps1 -useb | iex
   fly auth login
   ```
2. **Create the app.** Pick a name; it becomes the address `https://<name>.fly.dev` and has to
   be unique across Fly:
   ```powershell
   fly apps create <name>
   ```
   Then in `fly.toml` set `app = "<name>"` and
   `GO_BOX_ORIGINS = "https://<name>.fly.dev"` (the server only accepts game connections from
   that page). Optionally change `primary_region` to the region nearest the players
   (`fly platform regions`).
3. **Set the room passphrase** (only needed to create rooms; joining takes just the code). It's
   stored encrypted by Fly and never goes in the repo:
   ```powershell
   fly secrets set GO_BOX_CREATE_PASSPHRASE="your passphrase here" -a <name>
   ```
4. **First deploy**, from the repo folder (builds on Fly's machines, not yours):
   ```powershell
   fly deploy --ha=false
   ```
   `--ha=false` matters: rooms live in the machine's memory, so there must be exactly one.
   Then open `https://<name>.fly.dev`, type the passphrase next to **Create a room**, and play.
5. **Deploy on every push** (optional): make a deploy token and give it to GitHub:
   ```powershell
   fly tokens create deploy -a <name>
   ```
   In the GitHub repo: Settings > Secrets and variables > Actions > New repository secret, name
   `FLY_API_TOKEN`, value the whole token. From then on each push to `main` runs the tests and,
   if they pass, deploys (`.github/workflows/deploy.yml`). Keep the token out of chats and files.

## Day to day

- **See what it's doing:** `fly logs -a <name>` (each match logs its seed, map and players);
  `fly status -a <name>`.
- **Health:** `https://<name>.fly.dev/healthz` shows the open rooms and connections.
- **Change the passphrase:** `fly secrets set GO_BOX_CREATE_PASSPHRASE="..." -a <name>` (the
  machine restarts with it; rooms in progress end).
- **Roll back:** `fly releases -a <name>` lists deploys; `fly deploy --image <image from a
  release> --ha=false` puts an older one back.
- **Stop it entirely:** `fly scale count 0 -a <name>` (and `fly scale count 1` to bring it back).
- **After a deploy** players with the page open see "This page is out of date with the server.
  Reload it." and reload; new visitors get the new version straight away.

## What's protecting it

- Creating a room needs the passphrase; five wrong passphrases or room codes on one connection
  closes it.
- Only the game's own page may open game connections (`GO_BOX_ORIGINS`).
- Caps: 4 rooms, 32 connections, 8 per address; per-connection message rate limits.
- Dead connections are dropped after 45s of silence, so a closed tab can't hold a seat.
- HTTPS is Fly's (automatic certificates); the server inside only talks to Fly's proxy.
