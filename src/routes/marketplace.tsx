import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/marketplace')({
  head: () => ({
    meta: [
      { title: 'Marketplace — Anturix' },
      { name: 'description', content: 'Expert prediction marketplace on Anturix.' },
    ],
  }),
  component: MarketplacePage,
});

function MarketplacePage() {
  return <div />;
}
