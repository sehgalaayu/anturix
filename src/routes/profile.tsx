import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/profile')({
  head: () => ({
    meta: [
      { title: 'Profile — Anturix' },
      { name: 'description', content: 'Your Anturix profile and betting stats.' },
    ],
  }),
  component: ProfilePage,
});

function ProfilePage() {
  return <div />;
}
