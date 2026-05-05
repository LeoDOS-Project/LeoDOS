#ifndef DEFAULT_SIM_CLIENT_TOPICIDS_H
#define DEFAULT_SIM_CLIENT_TOPICIDS_H

/* Telemetry: per-tick snapshot of the local satellite's state as
 * received from the walker-delta bridge. Consumer apps subscribe to
 * this MID to obtain GPS-like position/velocity and link visibility
 * without coupling to the bridge's wire format. */
#define SIM_CLIENT_BRIDGE_STATE_TOPICID  0xA0

#endif
