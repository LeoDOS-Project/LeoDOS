#include "cfe_tbl_filedef.h"
#include "router_ground.h"

/* Compile-time defaults. The runtime override file written by
 * leo-viz (`router_ground.bin`) supersedes these values at startup.
 * Keep these in sync with `DEFAULT_NUM_*` in `apps/router/fsw/src/lib.rs`. */
RouterGroundTable_t RouterGroundTable = {
    .num_orbs = 3,
    .num_sats = 3,
    ._pad0 = {0, 0},
    .altitude_m = 550000.0f,
    .inclination_deg = 87.0f,
    .phasing = 1.0f,
    .count = 3,
    ._pad1 = {0, 0, 0},
    .entries = {
        {0, {0, 0, 0}, 67.86f, 20.22f},   /* Kiruna */
        {1, {0, 0, 0}, 78.23f, 15.39f},   /* Svalbard */
        {2, {0, 0, 0}, 64.86f, -147.72f}, /* Fairbanks */
        {0, {0, 0, 0}, 0.0f, 0.0f},       /* unused */
    },
};

CFE_TBL_FILEDEF(RouterGroundTable, ROUTER_APP.GroundTable, Router Ground Stations, router_ground.tbl)
