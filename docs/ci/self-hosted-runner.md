# The flop gate runner: WSL2 on Caleb's machine

Phase 4 Decision 12 (2026-09-09): the on-demand `flop-gate` job runs on a self-hosted
GitHub Actions runner inside WSL2 on Caleb's 16 GB development PC. GitHub's hosted
runners for a private repository have 8 GB, and the gate needs the solver's 12 GiB limit
plus tooling. Running it on this machine also measures the hardware the app must ship on.

This runbook is written for Caleb to follow once. Nothing in it goes into the repository
except this file: the runner token is entered on the command line and never written down.

## What the gate needs from the machine

| Need | Value | Why |
|---|---|---|
| Memory visible inside WSL2 | 14 GB (`memory=14GB` in `.wslconfig`) | The solver's default limit is 12 GiB (`config/solver.toml`, `memory_limit_mib = 12288`); the reference tooling, the runner, and the OS need about 1 GiB more. Windows keeps the remaining 2 GB. |
| Swap inside WSL2 | none (`swap=0`) | A peak-memory measurement with swap is not a measurement. |
| CPUs | all (default) | The gate is on demand; nothing else should run during it. |
| Disk | 20 GB free on the WSL2 disk | Rust target directory, the pinned reference build, and artifacts. |
| Time | up to 6 hours per gate run | The job's timeout is 360 minutes (phase 4 step 8). |

Do not use the PC for anything heavy while a gate runs. The measurement is only honest
when the VM has the memory the table says.

The prerequisite step inside the job reads `MemTotal` from `/proc/meminfo`, the CPU count,
the image name, and any cgroup limit, and refuses to solve when `MemTotal` is below
`memory_limit_mib` plus 1 GiB. With `memory=14GB` the VM reports roughly 13.9 GB, so the
check passes with about 1 GiB to spare. If it refuses, the `.wslconfig` setting did not
take: run `wsl --shutdown` and start again.

## One-time setup

All commands run in a WSL2 Ubuntu terminal unless marked PowerShell.

1. **Install WSL2 with Ubuntu** (PowerShell, as administrator, then reboot):
   ```powershell
   wsl --install -d Ubuntu-24.04
   ```
   Smart App Control does not affect Linux binaries inside the VM, so `cargo` runs there.

2. **Give the VM its memory** (PowerShell, creates or edits `C:\Users\Caleb\.wslconfig`):
   ```powershell
   @"
   [wsl2]
   memory=14GB
   swap=0
   "@ | Out-File -Encoding ascii "$env:USERPROFILE\.wslconfig"
   wsl --shutdown
   ```
   Then open Ubuntu again and confirm:
   ```bash
   grep MemTotal /proc/meminfo   # expect about 14000000 kB
   free -h                        # Swap: 0B
   nproc
   ```

3. **Toolchain inside the VM**, matching what CI installs:
   ```bash
   sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev curl git python3 python3-pip python3-venv
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source "$HOME/.cargo/env"
   # Node for the reference capture, same major as .github/workflows/ci.yml
   curl -fsSL https://deb.nodesource.com/setup_24.x | sudo -E bash - && sudo apt install -y nodejs
   ```
   `rust-toolchain.toml` in the repository pins the Rust version; `rustup` reads it on
   first build. Confirm `python3 --version` is 3.12 or later and `node --version` is 24.

4. **Create the runner user and folder.** The runner runs as an ordinary user with no
   access outside its folder:
   ```bash
   sudo useradd -m -s /bin/bash gharunner
   sudo -iu gharunner
   mkdir actions-runner && cd actions-runner
   ```

5. **Download and register the runner.** In the browser: repository Settings, Actions,
   Runners, "New self-hosted runner", Linux, x64. The page shows the exact download URL,
   its SHA-256, and a registration token that expires in an hour. Run the download and
   `./config.sh` lines it shows, and when `config.sh` asks:
   * runner group: default;
   * name: `caleb-wsl2-flop-gate`;
   * labels: add `flop-gate` (the job selects `[self-hosted, linux, x64, flop-gate]`);
   * work folder: default `_work`.
   Never paste the token anywhere else; it is single use and short lived.

6. **Start the runner when a gate is needed.** The gate is on demand, so the simplest
   safe mode is to start the runner by hand and stop it afterwards:
   ```bash
   sudo -iu gharunner
   cd actions-runner && ./run.sh
   ```
   The repository's Runners page shows it as Idle. Stop it with `Ctrl+C` after the run.
   If a permanent service is preferred, enable systemd in WSL2 first (`/etc/wsl.conf`,
   `[boot]` `systemd=true`, then `wsl --shutdown`) and run `sudo ./svc.sh install
   gharunner && sudo ./svc.sh start` from the runner folder. A permanently online runner
   is a larger attack surface; the manual mode is the recommended one.

7. **Prove it works before the real gate.** From the Actions tab, dispatch the
   `flop-gate` workflow with its dry-run input (step 8 adds it), or push a commit whose
   message carries the `[flop-gate]` tag once step 8 defines it. The prerequisite step's
   log must show the recorded memory, CPU count, and image, and must not refuse.

## Security notes for Astra's review

* The repository is private. Pull requests from forks cannot reach a self-hosted runner
  on a private repository, and no fork exists.
* The `flop-gate` job runs only on `workflow_dispatch` or a commit-message tag (Decision
  7). Nothing runs on the runner on an ordinary push.
* The runner user `gharunner` owns only its folder. It has no sudo. Secrets are not
  needed by the job; the workflow uses `persist-credentials: false` at checkout.
* The runner is offline except while a gate runs (manual mode above).
* If the repository is ever made public (the recorded fallback host option), remove this
  runner first: GitHub advises against self-hosted runners on public repositories because
  any pull request could execute code on it.

## Removing the runner

```bash
sudo -iu gharunner
cd actions-runner && ./config.sh remove --token <removal token from the Runners page>
```
Then delete the user with `sudo userdel -r gharunner`. Removing `.wslconfig` returns the
VM to its default memory share.

## Checks recorded per gate run

The job records, in its artifact: `MemTotal`, `nproc`, `/etc/os-release` PRETTY_NAME,
`cat /sys/fs/cgroup/memory.max` when present, the kernel version, and the runner name.
The phase 4 plan's Gates table requires those beside every peak-RSS number, so a
benchmark can never be quoted without the machine it ran on.
