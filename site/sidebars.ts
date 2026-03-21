import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'index',
    {
      type: 'category',
      label: 'Background',
      link: { type: 'doc', id: 'background/overview' },
      items: [
        'background/orbits',
        'background/constellations',
        {
          type: 'category',
          label: 'Links',
          items: [
            'background/satellite-links',
            'background/ground-links',
          ],
        },
        'background/sensors',
        'background/threats',
        'background/hardware',
        'background/software',
      ],
    },
    {
      type: 'category',
      label: 'Building and Running',
      link: { type: 'doc', id: 'building/overview' },
      items: [],
    },
    {
      type: 'category',
      label: 'SpaceCoMP',
      link: { type: 'doc', id: 'spacecomp/overview' },
      items: [
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
          link: { type: 'doc', id: 'spacecomp/use-cases/overview' },
          items: [
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
      link: { type: 'doc', id: 'protocols/overview' },
      items: [
        {
          type: 'category',
          label: 'Composition',
          link: { type: 'doc', id: 'protocols/composition/overview' },
          items: [
            'protocols/composition/reliability',
            'protocols/composition/security',
            'protocols/composition/time-codes',
          ],
        },
        {
          type: 'category',
          label: 'Transport',
          link: { type: 'doc', id: 'protocols/transport/overview' },
          items: [
            {
              type: 'category',
              label: 'SRSPP',
              link: { type: 'doc', id: 'protocols/transport/srspp/overview' },
              items: [
                'protocols/transport/srspp/packet-structure',
                'protocols/transport/srspp/flow-control',
                'protocols/transport/srspp/reassembly',
                'protocols/transport/srspp/reliability',
                'protocols/transport/srspp/configuration',
                'protocols/transport/srspp/dtn',
                'protocols/transport/srspp/operation',
                'protocols/transport/srspp/sequences',
              ],
            },
            'protocols/transport/cfdp',
            'protocols/transport/bp',
            'protocols/transport/ltp',
          ],
        },
        {
          type: 'category',
          label: 'Network',
          link: { type: 'doc', id: 'protocols/network/overview' },
          items: [
            'protocols/network/routing',
            'protocols/network/point-to-point',
            'protocols/network/gossip',
          ],
        },
        {
          type: 'category',
          label: 'Data Link',
          link: { type: 'doc', id: 'protocols/datalink/overview' },
          items: [
            {
              type: 'category',
              label: 'cFE Headers',
              link: { type: 'doc', id: 'protocols/datalink/cfe-headers/overview' },
              items: [
                'protocols/datalink/cfe-headers/tm',
                'protocols/datalink/cfe-headers/tc',
              ],
            },
            {
              type: 'category',
              label: 'Packet Protocol',
              link: { type: 'doc', id: 'protocols/datalink/packet/overview' },
              items: [
                'protocols/datalink/packet/spp',
                'protocols/datalink/packet/encapsulation',
              ],
            },
            {
              type: 'category',
              label: 'Transfer Frame',
              link: { type: 'doc', id: 'protocols/datalink/transfer-frame/overview' },
              items: [
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
              link: { type: 'doc', id: 'protocols/datalink/security/overview' },
              items: [
                'protocols/datalink/security/sdls',
              ],
            },
            {
              type: 'category',
              label: 'Reliability',
              link: { type: 'doc', id: 'protocols/datalink/reliability/overview' },
              items: [
                'protocols/datalink/reliability/cop1',
              ],
            },
          ],
        },
        {
          type: 'category',
          label: 'Coding',
          link: { type: 'doc', id: 'protocols/coding/overview' },
          items: [
            {
              type: 'category',
              label: 'Randomization',
              link: { type: 'doc', id: 'protocols/coding/randomization/overview' },
              items: [
                'protocols/coding/randomization/pseudo-random',
              ],
            },
            {
              type: 'category',
              label: 'Forward Error Correction',
              link: { type: 'doc', id: 'protocols/coding/fec/overview' },
              items: [
                'protocols/coding/fec/reed-solomon',
                'protocols/coding/fec/ldpc',
                'protocols/coding/fec/convolutional',
              ],
            },
            {
              type: 'category',
              label: 'Framing',
              link: { type: 'doc', id: 'protocols/coding/framing/overview' },
              items: [
                'protocols/coding/framing/asm-cadu',
                'protocols/coding/framing/cltu',
              ],
            },
            {
              type: 'category',
              label: 'Data Compression',
              link: { type: 'doc', id: 'protocols/coding/compression/overview' },
              items: [
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
          link: { type: 'doc', id: 'protocols/physical/overview' },
          items: [
            {
              type: 'category',
              label: 'Modulation',
              link: { type: 'doc', id: 'protocols/physical/modulation/overview' },
              items: [
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
              link: { type: 'doc', id: 'protocols/physical/hardware/overview' },
              items: [
                'protocols/physical/hardware/uart',
                'protocols/physical/hardware/spi',
                'protocols/physical/hardware/i2c',
                'protocols/physical/hardware/can',
                'protocols/physical/hardware/udp-tcp',
              ],
            },
          ],
        },
        {
          type: 'category',
          label: 'Misc',
          items: [
            'protocols/misc/sle',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'On-Board Analysis',
      link: { type: 'doc', id: 'analysis/overview' },
      items: [
        'analysis/indices',
        'analysis/thermal',
        'analysis/cloud',
        'analysis/stats',
        'analysis/geo',
      ],
    },
    {
      type: 'category',
      label: 'Core Flight System',
      link: { type: 'doc', id: 'cfs/overview' },
      items: [
        {
          type: 'category',
          label: 'Mission',
          link: { type: 'doc', id: 'cfs/mission/overview' },
          items: [
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
          link: { type: 'doc', id: 'cfs/cfe/overview' },
          items: [
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
      link: { type: 'doc', id: 'research/overview' },
      items: [
        'research/data-stream-processing',
        'research/security',
        'research/local-first',
      ],
    },
    {
      type: 'category',
      label: 'Simulation',
      link: { type: 'doc', id: 'simulation/overview' },
      items: [
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
      link: { type: 'doc', id: 'colonyos/overview' },
      items: [
        'colonyos/integration',
      ],
    },
  ],
};

export default sidebars;
