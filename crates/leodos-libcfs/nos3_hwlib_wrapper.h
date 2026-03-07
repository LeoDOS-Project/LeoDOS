/*
 * Wrapper header for NOS3 hwlib bindgen.
 *
 * Provides stub types that are normally supplied by Linux kernel
 * headers (linux/can.h, linux/spi/spidev.h) so that bindgen can
 * parse the hwlib public headers on any host OS.
 */
#ifndef NOS3_HWLIB_WRAPPER_H
#define NOS3_HWLIB_WRAPPER_H

#include <stdint.h>

/*
 * struct can_frame is defined by <linux/can.h> on Linux and by
 * libcan.h on RTEMS.  Provide a compatible definition for other
 * hosts so that can_info_t can be laid out.
 */
#if !defined(__linux__) && !defined(__rtems__)
struct can_frame {
    uint32_t can_id;
    uint8_t  can_dlc;
    uint8_t  __pad;
    uint8_t  __res0;
    uint8_t  __res1;
    uint8_t  data[8];
};
#endif

#include "hwlib.h"

#endif /* NOS3_HWLIB_WRAPPER_H */
