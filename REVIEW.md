# Hetzner VPS Security & Robustness Report

This report evaluates the current security posture, service robustness, and disk/resource management of the Hetzner VPS hosting **rocketrecap.com**.

---

## 1. System Overview

### Specifications & Status
* **Operating System:** Ubuntu 24.04.4 LTS (Noble)
* **Kernel:** `Linux 6.8.0-124-generic`
* **CPU & RAM:** 2 GB total RAM (1.9 GB addressable)
* **Swap Memory:** **0 MB (Disabled)**
* **Disk Space:** 38 GB total | **31 GB Used (86%)** | 5.1 GB Available (14%)

### Active Web Services & Port Mappings
Below is the map of incoming traffic routing:

| Domain | External Port | Internal Process | Bound Address | Run Context | Systemd Service |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **rocketrecap.com** | `80` & `443` | `rs-summarizer` | `0.0.0.0:5001` (Public) | User `kiel` (manual in `tmux`) | **None** |
| **jobs.rocketrecap.com** | `80` & `443` | `rs-scrape serve` | `127.0.0.1:3000` (Local) | User `www-data` | `jobs-rocketrecap.service` |

---

## 2. Security Vulnerabilities & Remediation

### ⚠️ Critical: `rs-summarizer` Exposed Publicly on Port 5001
* **Vulnerability:** The Rust program binds to `0.0.0.0:5001` in `src/main.rs`. Anyone on the internet can bypass Nginx and access the app directly via `http://<VPS_IP>:5001`. This bypasses TLS/HTTPS encryption, Nginx gzip compression, client rate-limiting, and any custom security/country headers (like the GeoIP country code headers configured in Nginx).
* **Fix:** Modify `src/main.rs` to bind only to localhost (`127.0.0.1`):
  ```diff
  -    let addr = SocketAddr::from(([0, 0, 0, 0], 5001));
  +    let addr = SocketAddr::from(([127, 0, 0, 1], 5001));
  ```
  After changing this, rebuild the binary and restart the service.

### ⚠️ Medium: Elevated Process Privileges for Web App
* **Vulnerability:** `rs-summarizer` is run under your interactive user account `kiel`. The `kiel` user has sudo access and full read/write permissions on key project files. If a remote code execution (RCE) bug is found in Axum, `yt-dlp`, or other dependencies, an attacker gains immediate command line control as user `kiel`.
* **Fix:** Transition `rs-summarizer` to run under a restricted, unprivileged service account (e.g. `www-data` or a dedicated `rs-summarizer` user), mirroring how `rs-scrape` runs.

### ⚠️ Medium: Permissive SSH Configuration
* **Vulnerability:** The SSH configuration `/etc/ssh/sshd_config` permits direct root login (`PermitRootLogin yes`) and leaves password authentication enabled (commented out defaults to `yes` on Ubuntu). While `fail2ban` is active and mitigates brute-force attacks, exposing password login and root login to the open web is an unnecessary risk.
* **Fix:** 
  1. Ensure you have your SSH key successfully added to `/home/kiel/.ssh/authorized_keys`.
  2. Edit `/etc/ssh/sshd_config` (or config file inside `/etc/ssh/sshd_config.d/`) to set:
     ```ini
     PermitRootLogin prohibit-password
     PasswordAuthentication no
     ```
  3. Reload sshd: `sudo systemctl reload ssh`

---

## 3. Robustness & Resource Management

### ⚠️ High: `rs-summarizer` Runs inside an Interactive Tmux Session
* **Vulnerability:** The main server process is running in the foreground of a `tmux` session pane. If the host reboots or the process crashes (e.g., due to OOM or database issues), the web app goes down and will **not** restart automatically.
* **Fix:** Create a custom systemd service file `/etc/systemd/system/rs-summarizer.service`:
  ```ini
  [Unit]
  Description=RocketRecap Summarizer Web Service
  After=network.target

  [Service]
  Type=simple
  User=kiel
  WorkingDirectory=/home/kiel/host
  ExecStart=/home/kiel/host/rs-summarizer
  Restart=always
  RestartSec=3
  EnvironmentFile=/home/kiel/host/.env

  [Install]
  WantedBy=multi-user.target
  ```
  To use this:
  1. Put `GEMINI_API_KEY=your_key_here` into `/home/kiel/host/.env`.
  2. Save this service file, reload systemd, and enable/start it:
     ```bash
     sudo systemctl daemon-reload
     sudo systemctl enable rs-summarizer
     sudo systemctl start rs-summarizer
     ```

### ⚠️ High: Out-of-Memory (OOM) Crash Risk (0 Swap)
* **Vulnerability:** The VPS has 2 GB of RAM and **0 Swap space**. Rust applications, particularly those running background scrapers or dealing with large video/audio processing files via `yt-dlp`, can spike memory. When the OS runs out of memory, the kernel OOM killer will kill the heaviest process (`rs-summarizer` or `rs-scrape`) instantly.
* **Fix:** Configure a 2 GB swap file to act as an emergency memory buffer:
  ```bash
  sudo fallocate -l 2G /swapfile
  sudo chmod 600 /swapfile
  sudo mkswap /swapfile
  sudo swapon /swapfile
  echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
  ```

### ⚠️ Medium: Low Disk Space (86% Used)
* **Vulnerability:** SQLite is used in Write-Ahead Log (WAL) mode for the database (`data/summaries.db`, currently **1.9 GB**). If the disk fills up to 100%, SQLite writes will fail, potentially corrupting transaction logs.
* **Action Plan to Reclaim ~7+ GB of Disk Space:**
  1. **Clean Python `uv` package cache (Saves ~3.8 GB):**
     ```bash
     uv cache clean
     # Or manual removal if uv isn't in global PATH:
     rm -rf /home/kiel/.cache/uv
     ```
  2. **Vacuum Systemd Journals (Saves ~1.9 GB):**
     Limit systemd journal size so it doesn't grow indefinitely. Run:
     ```bash
     sudo journalctl --vacuum-size=100M
     ```
     To persist this limit, edit `/etc/systemd/journald.conf` and set:
     ```ini
     SystemMaxUse=100M
     ```
  3. **Remove Old Snap Revisions (Saves ~2 GB):**
     Ubuntu's `snapd` keeps old versions of snapped utilities. You can restrict the retention count:
     ```bash
     sudo snap set system refresh.retain=2
     ```
     Then run this script to clean up disabled/old revisions:
     ```bash
     sudo snap list --all | awk '/disabled/{print $1, $3}' | while read snapname revision; do sudo snap remove "$snapname" --revision="$revision"; done
     ```

---

## 4. SQLite WAL Mode & Backups

Because the database utilizes WAL mode, simply copying `summaries.db` while the server is running might result in a copy that is missing the latest transactions (still in `summaries.db-wal`).
* **Safe Backup Command:**
  Create a cron job that runs a hot-backup of the SQLite database safely:
  ```bash
  sqlite3 /home/kiel/host/data/summaries.db ".backup /home/kiel/host/data/summaries_backup.db"
  ```
  This creates a consistent state backup without requiring you to stop the `rs-summarizer` server.
