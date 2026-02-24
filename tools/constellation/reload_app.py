#!/usr/bin/env python3
"""
Hot-reload a cFS app across the constellation.

Demonstrates cFE Executive Services runtime app reload:
1. Rebuilds the .so inside a build container
2. Copies the new binary into each running orbit container
3. Sends ES Reload command via CI_LAB UDP to each satellite

Usage:
    python3 tools/constellation/reload_app.py [--app spacecomp]

The ES Reload command (FC=7) tells cFE to:
  - Stop the running app
  - Unload the old shared library
  - Load the new .so from disk
  - Restart the app
All without restarting core-cpu1.

CI_LAB port per satellite:
  port = CI_LAB_BASE_UDP_PORT + cpuid - 1
  cpuid = orbit_id * max_sat + sat
"""

import argparse
import socket
import struct
import subprocess
import sys

CI_LAB_BASE_PORT = 1234
CFE_ES_CMD_MID = 0x1806
CFE_ES_RELOAD_APP_CC = 7
CFE_ES_RESTART_APP_CC = 6

OS_MAX_API_NAME = 20
OS_MAX_PATH_LEN = 64


def ci_lab_port(orbit_id: int, sat: int, max_sat: int) -> int:
    cpuid = orbit_id * max_sat + sat
    return CI_LAB_BASE_PORT + cpuid - 1


def build_es_command(function_code: int, payload: bytes) -> bytes:
    stream_id = CFE_ES_CMD_MID
    seq_flags = 0xC000
    data_len = 2 + len(payload) - 1

    primary = struct.pack(">HHH", stream_id, seq_flags, data_len)
    secondary = struct.pack("BB", function_code & 0x7F, 0)

    packet = bytearray(primary + secondary + payload)

    checksum_idx = 7
    total = sum(packet) & 0xFF
    packet[checksum_idx] = (0x100 - total) & 0xFF

    return bytes(packet)


def build_restart_cmd(app_name: str) -> bytes:
    name_bytes = app_name.encode("ascii")
    payload = name_bytes.ljust(OS_MAX_API_NAME, b"\x00")
    return build_es_command(CFE_ES_RESTART_APP_CC, payload)


def build_reload_cmd(app_name: str, file_path: str) -> bytes:
    name_bytes = app_name.encode("ascii")
    path_bytes = file_path.encode("ascii")
    payload = name_bytes.ljust(OS_MAX_API_NAME, b"\x00") + \
              path_bytes.ljust(OS_MAX_PATH_LEN, b"\x00")
    return build_es_command(CFE_ES_RELOAD_APP_CC, payload)


def copy_binary(container: str, src: str, dst: str):
    result = subprocess.run(
        ["docker", "cp", src, f"{container}:{dst}"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"  Error copying to {container}: {result.stderr.strip()}")
        return False
    return True


def send_command(host: str, port: int, packet: bytes):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.sendto(packet, (host, port))
    sock.close()


def get_running_containers() -> list[str]:
    result = subprocess.run(
        ["docker", "ps", "--filter", "name=orb-", "--format", "{{.Names}}"],
        capture_output=True, text=True
    )
    return sorted(result.stdout.strip().split("\n")) if result.stdout.strip() else []


def main():
    parser = argparse.ArgumentParser(
        description="Hot-reload a cFS app in the constellation"
    )
    parser.add_argument("--app", default="spacecomp",
                        help="App name to reload (default: spacecomp)")
    parser.add_argument("--build", action="store_true",
                        help="Rebuild the .so before reloading")
    parser.add_argument("--restart-only", action="store_true",
                        help="Just restart (re-use existing binary)")
    parser.add_argument("--max-sat", type=int, default=3,
                        help="Satellites per orbit (default: 3)")
    args = parser.parse_args()

    containers = get_running_containers()
    if not containers:
        print("No running orbit containers found.")
        sys.exit(1)

    print(f"Found containers: {', '.join(containers)}")

    so_name = f"lib{args.app}.so"
    cfs_path = f"/cf/{so_name}"

    if args.build:
        print(f"\nRebuilding {so_name}...")
        result = subprocess.run(
            ["docker", "compose", "run", "--rm", "cfs-build",
             "bash", "-c", "make"],
            capture_output=True, text=True
        )
        if result.returncode != 0:
            print(f"Build failed:\n{result.stderr}")
            sys.exit(1)
        print("Build succeeded.")

    for container in containers:
        print(f"\n--- {container} ---")

        if args.build:
            src = f"build/exe/cpu1/cf/{so_name}"
            dst = f"/cFS/build/exe/cpu1/cf/{so_name}"
            print(f"  Copying {so_name}...")
            if not copy_binary(container, src, dst):
                continue

        orbit_id = int(container.split("-")[-1])
        host = f"172.20.{orbit_id}.10"

        for sat in range(1, args.max_sat + 1):
            port = ci_lab_port(orbit_id, sat, args.max_sat)

            if args.restart_only:
                packet = build_restart_cmd(args.app)
                action = "restart"
            else:
                packet = build_reload_cmd(args.app, cfs_path)
                action = "reload"

            print(f"  Sending ES {action} to sat {sat} "
                  f"({host}:{port}, cpuid={orbit_id * args.max_sat + sat})")
            try:
                send_command(host, port, packet)
            except OSError as e:
                print(f"  Error: {e}")

    print("\nReload commands sent. Check cFS event logs "
          "for confirmation.")


if __name__ == "__main__":
    main()
