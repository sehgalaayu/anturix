import { createFileRoute } from '@tanstack/react-router';
import { MainLayout } from '@/components/layout/MainLayout';
import { currentUser } from '@/data/mockData';
import { User, Bell, Shield, Palette, Wallet, Globe, LogOut, ChevronRight } from 'lucide-react';

export const Route = createFileRoute('/settings')({
  head: () => ({
    meta: [
      { title: 'Configuración — Anturix' },
      { name: 'description', content: 'Configura tu cuenta de Anturix.' },
    ],
  }),
  component: SettingsPage,
});

const settingsGroups = [
  {
    title: 'CUENTA',
    items: [
      { icon: User, label: 'Perfil', desc: 'Nombre, avatar, bio' },
      { icon: Wallet, label: 'Wallet', desc: 'Conectar o cambiar wallet' },
      { icon: Shield, label: 'Seguridad', desc: 'Contraseña, 2FA' },
    ],
  },
  {
    title: 'PREFERENCIAS',
    items: [
      { icon: Bell, label: 'Notificaciones', desc: 'Push, email, in-app' },
      { icon: Palette, label: 'Apariencia', desc: 'Tema, colores' },
      { icon: Globe, label: 'Idioma', desc: 'Español, English' },
    ],
  },
];

function SettingsPage() {
  return (
    <MainLayout>
      <h1 className="text-xl font-heading font-bold text-foreground mb-6">Configuración</h1>

      {/* Profile card */}
      <div className="flex items-center gap-4 p-4 rounded-xl bg-card border border-border mb-6">
        <img src={currentUser.avatar} alt={currentUser.username} className="w-14 h-14 rounded-full border-2 border-primary" />
        <div>
          <p className="font-bold text-foreground">{currentUser.username}</p>
          <p className="text-xs text-muted-foreground">{currentUser.rank} · {currentUser.reputationScore.toLocaleString()} XP</p>
        </div>
      </div>

      {settingsGroups.map(group => (
        <div key={group.title} className="mb-6">
          <h2 className="text-xs font-heading font-bold text-muted-foreground mb-3 tracking-wider">{group.title}</h2>
          <div className="rounded-xl bg-card border border-border overflow-hidden divide-y divide-border">
            {group.items.map(item => (
              <button key={item.label} className="w-full flex items-center gap-3 p-4 hover:bg-muted/50 transition-colors text-left">
                <div className="w-9 h-9 rounded-lg bg-muted flex items-center justify-center">
                  <item.icon className="w-4.5 h-4.5 text-primary" />
                </div>
                <div className="flex-1">
                  <p className="text-sm font-medium text-foreground">{item.label}</p>
                  <p className="text-xs text-muted-foreground">{item.desc}</p>
                </div>
                <ChevronRight className="w-4 h-4 text-muted-foreground" />
              </button>
            ))}
          </div>
        </div>
      ))}

      <button className="w-full flex items-center justify-center gap-2 p-4 rounded-xl border border-destructive/30 text-destructive hover:bg-destructive/10 transition-colors">
        <LogOut className="w-4 h-4" />
        <span className="text-sm font-medium">Cerrar Sesión</span>
      </button>
    </MainLayout>
  );
}
