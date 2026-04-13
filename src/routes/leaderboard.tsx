import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/leaderboard')({
  head: () => ({
    meta: [
      { title: 'Leaderboard — Anturix' },
      { name: 'description', content: 'Top Antalers leaderboard on Anturix.' },
    ],
  }),
  component: LeaderboardPage,
});

function LeaderboardPage() {
  return <div />;
}
