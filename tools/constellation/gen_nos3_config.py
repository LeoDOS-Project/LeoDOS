#!/usr/bin/env python3
"""Generate NOS3 configuration for a multi-satellite constellation.

Creates 42 orbit/spacecraft files and NOS3 simulator XML with
per-spacecraft bus names (spi_sc0, usart_sc0, etc.).

Usage:
    python3 tools/constellation/gen_nos3_config.py \
        --orbits 3 --sats-per-orbit 3
"""

import argparse
from pathlib import Path
from textwrap import dedent


def copy_orbit_file(src_dir: Path, orbit_index: int, num_orbits: int,
                    dest_dir: Path) -> None:
    """Copies the existing orbit template, adjusting RAAN for the orbit plane."""
    src = src_dir / "Orb_LEO.txt"
    if not src.exists():
        raise FileNotFoundError(f"Orbit template not found: {src}")
    content = src.read_text()
    raan = orbit_index * (360.0 / num_orbits)
    # Replace the RAAN line (follows "Inclination" line)
    lines = content.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "Right Ascension" in line:
            parts = line.split("!")
            lines[i] = f"{raan:.1f}                         !{parts[1]}"
            break
    (dest_dir / f"Orb_{orbit_index}.txt").write_text("".join(lines))


def copy_sc_file(src_dir: Path, sc_index: int, sats_per_orbit: int,
                 dest_dir: Path) -> None:
    """Copies the existing SC template, adjusting true anomaly for spacing."""
    src = src_dir / "SC_NOS3.txt"
    if not src.exists():
        raise FileNotFoundError(f"SC template not found: {src}")
    content = src.read_text()
    sat_in_orbit = sc_index % sats_per_orbit
    true_anomaly = sat_in_orbit * (360.0 / sats_per_orbit)
    # Replace the initial true anomaly (usually "True Anomaly" line in orbit section)
    lines = content.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "True Anomaly" in line and "Initial" in line:
            parts = line.split("!")
            lines[i] = f"{true_anomaly:.1f}                           !{parts[1]}"
            break
    (dest_dir / f"SC_{sc_index}.txt").write_text("".join(lines))


def generate_inp_sim(src_dir: Path, num_orbits: int, sats_per_orbit: int) -> str:
    """Reads the original Inp_Sim.txt and patches orbit/SC counts and filenames."""
    src = src_dir / "Inp_Sim.txt"
    if not src.exists():
        raise FileNotFoundError(f"Template not found: {src}")

    lines = src.read_text().splitlines(keepends=True)
    result = []
    i = 0
    while i < len(lines):
        line = lines[i]

        # Disable graphics for headless Docker
        if "Graphics Front End" in line:
            result.append("FALSE                           !  Graphics Front End?\n")
            i += 1
            continue

        # Patch number of reference orbits
        if "Number of Reference Orbits" in line:
            result.append(f"{num_orbits}                               !  Number of Reference Orbits\n")
            i += 1
            # Skip old orbit lines, write new ones
            while i < len(lines) and "***" not in lines[i] and lines[i].strip():
                i += 1
            for o in range(num_orbits):
                result.append(f"TRUE   Orb_{o}.txt              !  Input file name for Orb {o}\n")
            continue

        # Patch number of spacecraft
        if "Number of Spacecraft" in line:
            total = num_orbits * sats_per_orbit
            result.append(f"{total}                               !  Number of Spacecraft\n")
            i += 1
            while i < len(lines) and "***" not in lines[i] and lines[i].strip():
                i += 1
            for sc in range(total):
                orb_ref = sc // sats_per_orbit
                result.append(f"TRUE  {orb_ref} SC_{sc}.txt             !  Existence, RefOrb, Input file for SC {sc}\n")
            continue

        result.append(line)
        i += 1

    return "".join(result)


def generate_simulator_xml(src_dir: Path, num_orbits: int, sats_per_orbit: int) -> str:
    """Patches the original nos3-simulator.xml: replaces single GPS/camera
    entries with per-spacecraft entries using unique bus names."""
    src = src_dir / "nos3-simulator.xml"
    if not src.exists():
        raise FileNotFoundError(f"Template not found: {src}")
    content = src.read_text()

    import re

    # Use single NOS Engine server for all connections
    content = content.replace(":12001", ":12000")
    content = re.sub(r'sc\d+-nos-engine-server', 'nos-engine-server', content)

    # Remove existing gps and thermal-cam-sim entries
    content = re.sub(
        r'<simulator>\s*<name>(gps|thermal-cam-sim)</name>.*?</simulator>',
        '', content, flags=re.DOTALL)

    # Insert per-spacecraft sims before </simulators>
    new_sims = _generate_per_sc_sims(num_orbits, sats_per_orbit)
    content = content.replace('</simulators>', new_sims + '\n        </simulators>')
    return content


def _generate_per_sc_sims(num_orbits: int, sats_per_orbit: int) -> str:
    total = num_orbits * sats_per_orbit

    sims = []
    for sc in range(total):
        spi_bus = f"spi_sc{sc}"
        usart_bus = f"usart_sc{sc}"

        sims.append(dedent(f"""\
            <simulator>
                <name>thermal-cam-sim-sc{sc}</name>
                <active>true</active>
                <library>libthermal_cam_sim.so</library>
                <hardware-model>
                    <type>THERMAL_CAM</type>
                    <connections>
                        <connection>
                            <type>command</type>
                            <bus-name>command</bus-name>
                            <node-name>thermal-cam-command-sc{sc}</node-name>
                        </connection>
                    </connections>
                    <spi>
                        <bus>{spi_bus}</bus>
                        <chip_select>3</chip_select>
                    </spi>
                    <data-provider>
                        <type>THERMALCAMPROVIDER</type>
                        <hostname>fortytwo</hostname>
                        <port>4245</port>
                        <max-connection-attempts>30</max-connection-attempts>
                        <retry-wait-seconds>1</retry-wait-seconds>
                        <spacecraft>{sc}</spacecraft>
                    </data-provider>
                    <aoi>
                        <west>-122.5</west>
                        <south>38.0</south>
                        <east>-121.5</east>
                        <north>39.0</north>
                    </aoi>
                    <data-dir>/sim/thermal_data</data-dir>
                </hardware-model>
            </simulator>
        """))

        sims.append(dedent(f"""\
            <simulator>
                <name>gps-sim-sc{sc}</name>
                <active>true</active>
                <library>libgps_sim.so</library>
                <hardware-model>
                    <type>OEM615</type>
                    <connections>
                        <connection>
                            <type>usart</type>
                            <bus-name>{usart_bus}</bus-name>
                            <node-port>1</node-port>
                        </connection>
                    </connections>
                    <data-provider>
                        <type>GPS42SOCKET</type>
                        <hostname>fortytwo</hostname>
                        <port>4245</port>
                        <max-connection-attempts>30</max-connection-attempts>
                        <retry-wait-seconds>1</retry-wait-seconds>
                        <spacecraft>{sc}</spacecraft>
                        <GPS>0</GPS>
                        <leap-seconds>37</leap-seconds>
                    </data-provider>
                </hardware-model>
            </simulator>
        """))

    return "".join(sims)


def generate_inp_ipc(src_dir: Path) -> str:
    """Patches Inp_IPC.txt to disable unused sockets.

    42 opens IPC sockets sequentially and blocks on accept() for
    each RX socket. If a sim (e.g., reaction wheel) is not running,
    42 never proceeds to open later sockets. We keep only the
    sockets our sims actually connect to: GPS (4245) and truth (9999).
    """
    src = src_dir / "Inp_IPC.txt"
    if not src.exists():
        raise FileNotFoundError(f"Template not found: {src}")

    keep_ports = {"4245", "9999"}
    lines = src.read_text().splitlines(keepends=True)
    result = []
    i = 0
    while i < len(lines):
        line = lines[i]

        # Detect start of an IPC block (line with "****")
        if "****" in line and i + 1 < len(lines):
            # Collect the block (header + 7-9 lines until next **** or EOF)
            block = [line]
            i += 1
            while i < len(lines) and "****" not in lines[i]:
                block.append(lines[i])
                i += 1

            # Check if this block's port is one we want to keep
            port_line = next((l for l in block if "Server Host Name" in l or "Port" in l), None)
            keep = False
            if port_line:
                for port in keep_ports:
                    if port in port_line.split("!")[0]:
                        keep = True
                        break

            if keep:
                result.extend(block)
            else:
                # Set IPC mode to OFF
                result.append(block[0])  # section header
                for bl in block[1:]:
                    if "IPC Mode" in bl:
                        result.append("OFF                                     ! IPC Mode (OFF,TX,RX,TXRX,ACS,WRITEFILE,READFILE)\n")
                    else:
                        result.append(bl)
            continue

        result.append(line)
        i += 1

    return "".join(result)


def main():
    parser = argparse.ArgumentParser(description="Generate NOS3 constellation config")
    parser.add_argument("--orbits", type=int, default=3)
    parser.add_argument("--sats-per-orbit", type=int, default=3)
    parser.add_argument("--output-dir", type=str,
                        default="tools/constellation/generated")
    parser.add_argument("--template-dir", type=str,
                        default="libs/nos3/cfg/InOut",
                        help="Path to existing 42 InOut directory with templates")
    args = parser.parse_args()

    out = Path(args.output_dir)
    inout = out / "InOut"
    inout.mkdir(parents=True, exist_ok=True)
    src_42 = Path(args.template_dir)

    # 42 configs
    inp_sim = generate_inp_sim(src_42, args.orbits, args.sats_per_orbit)
    (inout / "Inp_Sim.txt").write_text(inp_sim)

    for i in range(args.orbits):
        copy_orbit_file(src_42, i, args.orbits, inout)

    total = args.orbits * args.sats_per_orbit
    for sc in range(total):
        copy_sc_file(src_42, sc, args.sats_per_orbit, inout)

    # 42 IPC config — disable unused sockets
    inp_ipc = generate_inp_ipc(src_42)
    (inout / "Inp_IPC.txt").write_text(inp_ipc)

    # NOS3 simulator XML
    sim_src = Path("libs/nos3/cfg/sims")
    sim_xml = generate_simulator_xml(sim_src, args.orbits, args.sats_per_orbit)
    (out / "nos3-simulator.xml").write_text(sim_xml)

    # Write list of per-SC sim names for the sims launcher
    sim_names = []
    for sc in range(total):
        sim_names.append(f"thermal-cam-sim-sc{sc}")
        sim_names.append(f"gps-sim-sc{sc}")
    (out / "sim-names.txt").write_text("\n".join(sim_names) + "\n")

    print(f"Generated config for {total} satellites "
          f"({args.orbits} orbits x {args.sats_per_orbit} sats)")
    print(f"  42 configs:     {inout}/")
    print(f"  Simulator XML:  {out}/nos3-simulator.xml")


if __name__ == "__main__":
    main()
