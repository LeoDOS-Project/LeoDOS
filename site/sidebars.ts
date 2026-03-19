import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'index',
    {
      type: 'category',
      label: 'Building and Running',
      items: [
        'building/overview',
      ],
    },
    {
      type: 'category',
      label: 'SpaceCoMP',
      items: [
        'spacecomp/overview',
        'spacecomp/constellation',
        'spacecomp/roles',
        'spacecomp/job-lifecycle',
        'spacecomp/routing',
        {
          type: 'category',
          label: 'Task Allocation',
          items: [
            'spacecomp/task-allocation/hungarian',
            'spacecomp/task-allocation/lapjv',
          ],
        },
        {
          type: 'category',
          label: 'Use Cases',
          items: [
            'spacecomp/use-cases/overview',
            'spacecomp/use-cases/tailings-dam',
            'spacecomp/use-cases/wildfire',
            'spacecomp/use-cases/deforestation',
            'spacecomp/use-cases/oil-spill',
            'spacecomp/use-cases/flood',
            'spacecomp/use-cases/sea-ice',
            'spacecomp/use-cases/anti-poaching',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'LEO Communication Protocols',
      items: [
        'protocols/overview',
        {
          type: 'category',
          label: 'Composition',
          items: [
            'protocols/composition/overview',
            'protocols/composition/reliability',
            'protocols/composition/security',
            'protocols/composition/time-codes',
          ],
        },
        {
          type: 'category',
          label: 'Transport',
          items: [
            'protocols/transport/overview',
            'protocols/transport/srspp',
            'protocols/transport/cfdp',
            'protocols/transport/bp',
          ],
        },
        {
          type: 'category',
          label: 'Network',
          items: [
            'protocols/network/overview',
            'protocols/network/routing',
            'protocols/network/point-to-point',
            'protocols/network/gossip',
          ],
        },
        {
          type: 'category',
          label: 'Data Link',
          items: [
            'protocols/datalink/overview',
            {
              type: 'category',
              label: 'cFE Headers',
              items: [
                'protocols/datalink/cfe-headers/overview',
                'protocols/datalink/cfe-headers/tm',
                'protocols/datalink/cfe-headers/tc',
              ],
            },
            {
              type: 'category',
              label: 'Packet Protocol',
              items: [
                'protocols/datalink/packet/overview',
                'protocols/datalink/packet/spp',
              ],
            },
            {
              type: 'category',
              label: 'Transfer Frame',
              items: [
                'protocols/datalink/transfer-frame/overview',
                'protocols/datalink/transfer-frame/tm',
                'protocols/datalink/transfer-frame/tc',
                'protocols/datalink/transfer-frame/aos',
                'protocols/datalink/transfer-frame/proximity1',
                'protocols/datalink/transfer-frame/uslp',
              ],
            },
            {
              type: 'category',
              label: 'Security',
              items: [
                'protocols/datalink/security/overview',
                'protocols/datalink/security/sdls',
              ],
            },
            {
              type: 'category',
              label: 'Reliability',
              items: [
                'protocols/datalink/reliability/overview',
                'protocols/datalink/reliability/cop1',
              ],
            },
          ],
        },
        {
          type: 'category',
          label: 'Coding',
          items: [
            'protocols/coding/overview',
            {
              type: 'category',
              label: 'Randomization',
              items: [
                'protocols/coding/randomization/overview',
                'protocols/coding/randomization/pseudo-random',
              ],
            },
            {
              type: 'category',
              label: 'Forward Error Correction',
              items: [
                'protocols/coding/fec/overview',
                'protocols/coding/fec/reed-solomon',
                'protocols/coding/fec/ldpc',
                'protocols/coding/fec/convolutional',
              ],
            },
            {
              type: 'category',
              label: 'Framing',
              items: [
                'protocols/coding/framing/overview',
                'protocols/coding/framing/asm-cadu',
                'protocols/coding/framing/cltu',
              ],
            },
            {
              type: 'category',
              label: 'Data Compression',
              items: [
                'protocols/coding/compression/overview',
                'protocols/coding/compression/rice',
                'protocols/coding/compression/dwt',
                'protocols/coding/compression/hyperspectral',
              ],
            },
          ],
        },
        {
          type: 'category',
          label: 'Physical',
          items: [
            'protocols/physical/overview',
            {
              type: 'category',
              label: 'Modulation',
              items: [
                'protocols/physical/modulation/overview',
                'protocols/physical/modulation/bpsk',
                'protocols/physical/modulation/qpsk',
                'protocols/physical/modulation/oqpsk',
                'protocols/physical/modulation/8psk',
                'protocols/physical/modulation/gmsk',
              ],
            },
            {
              type: 'category',
              label: 'Hardware',
              items: [
                'protocols/physical/hardware/overview',
                'protocols/physical/hardware/uart',
                'protocols/physical/hardware/spi',
                'protocols/physical/hardware/i2c',
                'protocols/physical/hardware/can',
                'protocols/physical/hardware/udp-tcp',
              ],
            },
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Core Flight System',
      items: [
        'cfs/overview',
        {
          type: 'category',
          label: 'Mission',
          items: [
            'cfs/mission/overview',
            'cfs/mission/structure',
            'cfs/mission/deployment',
            'cfs/mission/scheduling',
            'cfs/mission/communication',
            'cfs/mission/identification',
            'cfs/mission/time',
            'cfs/mission/processor',
            'cfs/mission/fault-tolerance',
            'cfs/mission/memory',
          ],
        },
        {
          type: 'category',
          label: 'cFE',
          items: [
            'cfs/cfe/overview',
            'cfs/cfe/es',
            'cfs/cfe/sb',
            'cfs/cfe/evs',
            'cfs/cfe/tbl',
            'cfs/cfe/time',
          ],
        },
        'cfs/osal',
        'cfs/psp',
      ],
    },
    {
      type: 'category',
      label: 'Research',
      items: [
        'research/overview',
        'research/data-stream-processing',
        'research/security',
      ],
    },
    {
      type: 'category',
      label: 'Simulation',
      items: [
        'simulation/overview',
        'simulation/constellation',
        'simulation/orbital-mechanics',
        'simulation/sensors',
        'simulation/communication',
        'simulation/earth-observation',
      ],
    },
    {
      type: 'category',
      label: 'ColonyOS',
      items: [
        'colonyos/overview',
        'colonyos/integration',
      ],
    },
  ],
};

export default sidebars;
