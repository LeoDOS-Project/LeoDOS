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

typedef struct {
    uint8 count;            /* valid entries are entries[0..count) */
    uint8 _pad[3];
    RouterGroundEntry_t entries[ROUTER_GROUND_MAX_STATIONS];
} RouterGroundTable_t;

#endif
