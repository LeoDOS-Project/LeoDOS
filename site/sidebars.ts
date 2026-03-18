import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docs: [
    'index',
    {
      type: 'category',
      label: 'SpaceCoMP',
      items: [
        'spacecomp/overview',
        'spacecomp/use-cases',
        {
          type: 'category',
          label: 'Task Allocation',
          items: [
            'spacecomp/task-allocation/hungarian',
            'spacecomp/task-allocation/lapjv',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Protocols',
      items: [
        'protocols/stack',
        {
          type: 'category',
          label: 'Transport',
          items: [
            'protocols/transport/srspp',
            'protocols/transport/comparison',
          ],
        },
        {
          type: 'category',
          label: 'Network',
          items: [
            'protocols/network/routing',
            'protocols/network/gossip',
          ],
        },
        {
          type: 'category',
          label: 'Data Link',
          items: [
            'protocols/datalink/spp',
            'protocols/datalink/telecommand',
            'protocols/datalink/telemetry',
          ],
        },
        {
          type: 'category',
          label: 'Coding',
          items: [
            'protocols/coding/coding',
            'protocols/coding/ldpc',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Simulation',
      items: ['simulation/simulator'],
    },
    {
      type: 'category',
      label: 'ColonyOS',
      items: ['colonyos/integration'],
    },
  ],
};

export default sidebars;
