#ifndef ROUTER_GROUND_TABLE_H
#define ROUTER_GROUND_TABLE_H

#include "common_types.h"

/* Must match `ROUTER_MAX_GROUND_STATIONS` in
 * `default_router_platform_cfg.h` and the corresponding Rust
 * constant on the router app's side. */
#define ROUTER_GROUND_MAX_STATIONS 4

typedef struct {
    uint8 station_id;
    uint8 _pad[3];
    float lat_deg;
    float lon_deg;
} RouterGroundEntry_t;

/* Runtime-configurable constellation + ground-station table.
 *
 * Static compile-time defaults live in `router_ground.c`; leo-viz
 * overrides via `router_ground.bin` in the shared log volume. Both
 * sides must agree on this struct's byte layout. */
typedef struct {
    /* Constellation grid */
    uint8 num_orbs;
    uint8 num_sats;
    uint8 _pad0[2];
    /* Orbital shell parameters used by DistanceMinimizing routing
     * and the GatewayTable LOS computation. */
    float altitude_m;
    float inclination_deg;
    /* Ground stations: valid entries are entries[0..count). */
    uint8 count;
    uint8 _pad1[3];
    RouterGroundEntry_t entries[ROUTER_GROUND_MAX_STATIONS];
} RouterGroundTable_t;

#endif
