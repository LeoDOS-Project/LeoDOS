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

#endif
