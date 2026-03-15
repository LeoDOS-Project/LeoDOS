#ifndef DEFAULT_GOSSIP_PLATFORM_CFG_H
#define DEFAULT_GOSSIP_PLATFORM_CFG_H

#define GOSSIP_NUM_ORBS            3
#define GOSSIP_NUM_SATS            3

/* Port base for gossip UDP sockets (separate from router's 6000). */
#define GOSSIP_PORT_BASE           7000
#define GOSSIP_PORTS_PER_SAT       10

/* How often to broadcast gossip (seconds). */
#define GOSSIP_INTERVAL_SECS       5

/* APID used in outgoing gossip packets. */
#define GOSSIP_APID                0x80

/* cFE function code for gossip telecommands. */
#define GOSSIP_FUNCTION_CODE       1

/* Software version published in health state. */
#define GOSSIP_SW_VERSION          1

/* APID-based delivery routing table. */
#define GOSSIP_MAX_ROUTES          8

#define GOSSIP_ROUTE_0_APID        0x70
#define GOSSIP_ROUTE_0_TOPIC       0xB0
#define GOSSIP_ROUTE_1_APID        0x71
#define GOSSIP_ROUTE_1_TOPIC       0xB1
#define GOSSIP_ROUTE_2_APID        0x60
#define GOSSIP_ROUTE_2_TOPIC       0xB2

#endif
