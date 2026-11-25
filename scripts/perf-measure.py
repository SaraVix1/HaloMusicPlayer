#!/usr/bin/env python3
"""Measure Halo's startup time, memory and CPU on a *copy* of your library.

Usage:
    python3 scripts/perf-measure.py BINARY LABEL [--backend wgpu|tiny-skia]
                                    [--skips 50] [--sandbox DIR] [--out FILE.json]

What it does (Linux, X11 / XWayland session, Halo itself must not be running):
  1. Copies ~/.local/share/com.saravix.halo (the database) into a sandbox and
     edits the copy only: volume 0 (silent), Last.fm signed out (no
     scrobbles), resume on launch (paused), repeat all, and a queue of 60
     random library songs. Your real data is never touched.
  2. Starts BINARY with XDG_DATA_HOME pointing at the copy, HALO_PROFILE=1
     and ICED_BACKEND=<backend>, and reads the [profile] startup marks.
  3. Samples memory (VmRSS / VmHWM) and CPU (% of one core):
     idle on Home (20 s), playing (20 s, via MPRIS Play), then RSS before /
     after --skips track changes (MPRIS Next) as a leak check.
  4. Stops Halo and prints / saves the numbers.
"""

import argparse
import json
import os
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

APP = "com.saravix.halo"
MPRIS = ["dbus-send", "--session", "--type=method_call", "--dest=org.mpris.MediaPlayer2.halo",
         "/org/mpris/MediaPlayer2"]
HZ = os.sysconf("SC_CLK_TCK")


def running_halo():
    out = subprocess.run(["pgrep", "-x", "halo"], capture_output=True, text=True).stdout.split()
    return [int(p) for p in out]


def prepare(sandbox: Path) -> Path:
    src = Path.home() / ".local/share" / APP
    data = sandbox / "data"
    if data.exists():
        shutil.rmtree(data)
    (data / APP).mkdir(parents=True)
    for name in ["halo.db", "halo.db-wal", "halo.db-shm"]:
        if (src / name).exists():
            shutil.copy2(src / name, data / APP / name)
    db = sqlite3.connect(data / APP / "halo.db")
    kv = {
        "player.volume": "0",
        "player.status": "paused",
        "playback.resume_on_launch": "true",
        "player.repeat": "all",
        "player.shuffle": "0",
        "player.current_index": "0",
        "player.position_ms": "0",
    }
    for k, v in kv.items():
        db.execute("INSERT INTO app_state (key, value) VALUES (?, ?) "
                   "ON CONFLICT(key) DO UPDATE SET value = excluded.value", (k, v))
    for k in ["lastfm.session_key", "lastfm.username", "player.shuffle_order", "player.shuffle_pos"]:
        db.execute("DELETE FROM app_state WHERE key = ?", (k,))
    db.execute("DELETE FROM queue")
    ids = [r[0] for r in db.execute("SELECT id FROM tracks ORDER BY random() LIMIT 60")]
    db.executemany("INSERT INTO queue (track_id, position) VALUES (?, ?)", [(t, i) for i, t in enumerate(ids)])
    db.commit()
    db.close()
    return data


def status(pid: int) -> dict:
    out = {}
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        k, _, v = line.partition(":")
        if k in ("VmRSS", "VmHWM"):
            out[k] = int(v.split()[0]) / 1024  # MB
    return out


def cpu_ticks(pid: int) -> int:
    f = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    return int(f[11]) + int(f[12])  # utime + stime (fields 14, 15)


def cpu_percent(pid: int, seconds: float) -> float:
    a, t0 = cpu_ticks(pid), time.monotonic()
    time.sleep(seconds)
    b, t1 = cpu_ticks(pid), time.monotonic()
    return 100.0 * (b - a) / HZ / (t1 - t0)


def mpris(method: str):
    subprocess.run(MPRIS + [f"org.mpris.MediaPlayer2.Player.{method}"], capture_output=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("binary")
    ap.add_argument("label")
    ap.add_argument("--backend", default="wgpu")
    ap.add_argument("--skips", type=int, default=50)
    ap.add_argument("--sandbox", default="/tmp/halo-perf")
    ap.add_argument("--out")
    a = ap.parse_args()

    if running_halo():
        print("Halo is running — quit it first (tray → Quit, or `pkill -x halo`).", file=sys.stderr)
        return 2
    sandbox = Path(a.sandbox)
    data = prepare(sandbox)
    log_path = sandbox / f"{a.label}-{a.backend}.log"
    env = dict(os.environ, XDG_DATA_HOME=str(data), HALO_PROFILE="1", ICED_BACKEND=a.backend)
    t_spawn = time.monotonic()
    with open(log_path, "w") as log:
        proc = subprocess.Popen([a.binary], env=env, stdout=log, stderr=subprocess.STDOUT)
    res = {"label": a.label, "backend": a.backend, "marks": {}}
    try:
        # Startup: wait for the main window.
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            text = log_path.read_text(errors="replace")
            if "main window opened" in text:
                res["startup_wall_ms"] = round((time.monotonic() - t_spawn) * 1000)
                break
            if proc.poll() is not None:
                raise SystemExit(f"Halo exited early; see {log_path}")
            time.sleep(0.02)
        else:
            raise SystemExit(f"no window after 30 s; see {log_path}")
        time.sleep(3)
        for m in re.finditer(r"\[profile\]\s+([\d.]+) ms\s+(.+)", log_path.read_text(errors="replace")):
            res["marks"].setdefault(m.group(2).strip(), float(m.group(1)))

        pid = proc.pid
        res["idle"] = {**status(pid), "cpu": round(cpu_percent(pid, 20), 2)}
        mpris("Play")
        time.sleep(3)
        res["playing"] = {**status(pid), "cpu": round(cpu_percent(pid, 20), 2)}
        before = status(pid)["VmRSS"]
        for _ in range(a.skips):
            mpris("Next")
            time.sleep(0.4)
        time.sleep(3)
        after = status(pid)
        res["leak"] = {"skips": a.skips, "rss_before": round(before, 1), "rss_after": round(after["VmRSS"], 1),
                       "hwm": round(after["VmHWM"], 1)}
    finally:
        proc.send_signal(signal.SIGTERM)
        try:
            proc.wait(5)
        except subprocess.TimeoutExpired:
            proc.kill()

    for k in ("idle", "playing"):
        res[k] = {kk: round(v, 1) if isinstance(v, float) else v for kk, v in res[k].items()}
    print(json.dumps(res, indent=2))
    if a.out:
        Path(a.out).write_text(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
