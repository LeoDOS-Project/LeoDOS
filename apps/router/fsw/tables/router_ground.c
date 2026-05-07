#include "cfe_tbl_filedef.h"
#include "router_ground.h"

RouterGroundTable_t RouterGroundTable = {
    .count = 3,
    ._pad = {0, 0, 0},
    .entries = {
        {0, {0, 0, 0}, 67.86f, 20.22f},   /* Kiruna */
        {1, {0, 0, 0}, 78.23f, 15.39f},   /* Svalbard */
        {2, {0, 0, 0}, 64.86f, -147.72f}, /* Fairbanks */
        {0, {0, 0, 0}, 0.0f, 0.0f},       /* unused */
    },
};

CFE_TBL_FILEDEF(RouterGroundTable, ROUTER_APP.GroundTable, Router Ground Stations, router_ground.tbl)
