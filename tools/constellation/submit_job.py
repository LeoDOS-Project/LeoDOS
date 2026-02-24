#!/usr/bin/env python3
"""
Submit a SpaceCoMP job to the LOS satellite (orbit 1, sat 1).

Constructs a minimal ISL routing telecommand wrapping a SpaceCoMP
SubmitJob packet and sends it via UDP to the satellite's ground port.

Usage:
    python3 tools/submit_job.py [--host HOST] [--port PORT] [--job-id ID]
"""

import argparse
import socket
import struct

PORT_BASE = 6000
PORTS_PER_SAT = 10
PORT_GROUND = 8

SPACECOMP_APID = 0x60
OPCODE_SUBMIT_JOB = 0x00
OPCODE_JOB_RESULT = 0x05


def sat_ground_port(orbit: int, sat: int, num_sats: int = 3) -> int:
    base = PORT_BASE + (orbit * num_sats + sat) * PORTS_PER_SAT
    return base + PORT_GROUND + 1


def build_cfe_primary_header(apid: int, length: int) -> bytes:
    stream_id = 0x1800 | (apid & 0x7FF)
    sequence = 0xC000
    data_len = length - 7
    return struct.pack(">HHH", stream_id, sequence, data_len)


def build_cfe_tc_secondary() -> bytes:
    return struct.pack(">BB", 0, 0)


def build_isl_header(target_orbit: int, target_sat: int) -> bytes:
    ground_or_orbit = target_orbit + 1
    station_or_sat = target_sat
    return struct.pack(
        ">BBBBHB",
        ground_or_orbit, station_or_sat,
        0, 0,
        0,
        0,
    )


def build_spacecomp_header(opcode: int, job_id: int) -> bytes:
    return struct.pack(">BBH", opcode, 0, job_id)


def build_submit_job_packet(job_id: int) -> bytes:
    isl_hdr = build_isl_header(0, 1)
    sc_hdr = build_spacecomp_header(OPCODE_SUBMIT_JOB, job_id)
    inner = isl_hdr + sc_hdr

    tc_sec = build_cfe_tc_secondary()
    total_len = 6 + len(tc_sec) + len(inner)
    primary = build_cfe_primary_header(SPACECOMP_APID, total_len)

    packet = bytearray(primary + tc_sec + inner)

    checksum_idx = 6
    total = sum(packet) & 0xFF
    packet[checksum_idx] = (0x100 - total) & 0xFF

    return bytes(packet)


def main():
    parser = argparse.ArgumentParser(description="Submit a SpaceCoMP job")
    parser.add_argument("--host", default="172.20.0.10",
                        help="LOS satellite host (default: 172.20.0.10)")
    parser.add_argument("--port", type=int, default=None,
                        help="LOS satellite ground port")
    parser.add_argument("--job-id", type=int, default=1,
                        help="Job ID (default: 1)")
    parser.add_argument("--num-sats", type=int, default=3,
                        help="Satellites per orbit (default: 3)")
    parser.add_argument("--listen", action="store_true",
                        help="Listen for job result after submission")
    args = parser.parse_args()

    if args.port is None:
        args.port = sat_ground_port(0, 1, args.num_sats)

    packet = build_submit_job_packet(args.job_id)

    print(f"Submitting job {args.job_id} to {args.host}:{args.port}")
    print(f"Packet ({len(packet)} bytes): {packet.hex()}")

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.sendto(packet, (args.host, args.port))
    print("Job submitted.")

    if args.listen:
        print("Listening for result...")
        sock.settimeout(30.0)
        try:
            data, addr = sock.recvfrom(4096)
            print(f"Received {len(data)} bytes from {addr}")
            print(f"Data: {data.hex()}")
        except socket.timeout:
            print("Timed out waiting for result.")

    sock.close()


if __name__ == "__main__":
    main()
