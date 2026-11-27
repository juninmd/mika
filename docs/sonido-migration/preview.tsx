import { Button } from '@juninmd/sonido'
import { Check, History, Lightbulb, Sparkles, Trash2, Users } from 'lucide-react'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import '../../apps/desktop/src/renderer/src/styles.css'
import '@juninmd/sonido/styles.css'
import '../../apps/desktop/src/renderer/src/sonido-bridge.css'

// Depois: o mesmo arquivo do app, agora apoiado no Sonido.
import { ConfirmButton, EmptyState, PageHero, StatTile, StatusPill } from '../../packages/sdk-ui/src/settings'
// Antes: cópia literal de origin/main, só o caminho do import foi ajustado.
import {
  ConfirmButton as LegacyConfirmButton,
  EmptyState as LegacyEmptyState,
  PageHero as LegacyPageHero,
  StatTile as LegacyStatTile,
  StatusPill as LegacyStatusPill,
} from './legacy'

const PILLS: Array<{
  variant: 'ok' | 'warn' | 'error' | 'loading' | 'inactive' | 'new' | 'pending' | 'neutral' | 'accent' | 'info'
  label: string
  dot?: boolean
}> = [
  { variant: 'ok', label: 'online', dot: true },
  { variant: 'warn', label: 'atenção', dot: true },
  { variant: 'error', label: 'erro', dot: true },
  { variant: 'loading', label: 'carregando', dot: true },
  { variant: 'inactive', label: 'desligado' },
  { variant: 'new', label: 'novo' },
  { variant: 'pending', label: 'pendente' },
  { variant: 'neutral', label: 'neutro' },
  { variant: 'accent', label: 'destaque' },
  { variant: 'info', label: 'informação' },
]

const TONES: Array<{
  tone: 'neutral' | 'accent' | 'ok' | 'warn' | 'danger' | 'info'
  label: string
  value: string
  hint: string
  icon: typeof Users
}> = [
  { tone: 'neutral', label: 'Mensagens', value: '128', hint: 'no histórico', icon: History },
  { tone: 'accent', label: 'Provedor', value: 'Ollama', hint: 'local', icon: Sparkles },
  { tone: 'ok', label: 'Memórias', value: '42', hint: 'episódicas', icon: Lightbulb },
  { tone: 'warn', label: 'Pessoas', value: '3', hint: 'sem contato há 30 dias', icon: Users },
  { tone: 'danger', label: 'Falhas', value: '1', hint: 'nas últimas 24 h', icon: Trash2 },
  { tone: 'info', label: 'Modelos', value: '5', hint: 'instalados', icon: Check },
]

const card: React.CSSProperties = {
  background: 'var(--color-glass)',
  border: '1px solid var(--color-border-subtle)',
  borderRadius: 14,
  padding: 16,
  minWidth: 0,
}
const grid: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: '1fr 1fr',
  gap: 16,
  alignItems: 'stretch',
}
const tag = (color: string): React.CSSProperties => ({
  fontSize: 10,
  letterSpacing: '0.12em',
  textTransform: 'uppercase',
  color,
  fontWeight: 700,
  marginBottom: 10,
})
const page: React.CSSProperties = {
  background: 'var(--bg)',
  color: 'var(--color-text-main)',
  fontFamily: 'var(--font-sans)',
  padding: 28,
  minHeight: '100vh',
  display: 'grid',
  gap: 22,
}
const rowBody: React.CSSProperties = { display: 'grid', gap: 10, alignContent: 'start' }

function Row({
  title,
  hint,
  before,
  after,
}: {
  title: string
  hint: string
  before: React.ReactNode
  after: React.ReactNode
}) {
  return (
    <section style={{ display: 'grid', gap: 8 }}>
      <div>
        <h2 style={{ margin: 0, fontSize: 15 }}>{title}</h2>
        <p style={{ margin: '2px 0 0', fontSize: 12, color: 'var(--color-text-muted)' }}>{hint}</p>
      </div>
      <div style={grid}>
        <div style={card}>
          <div style={tag('var(--color-text-subtle)')}>Antes · componente do mika</div>
          <div style={rowBody}>{before}</div>
        </div>
        <div style={card}>
          <div style={tag('var(--color-accent)')}>Depois · @juninmd/sonido</div>
          <div style={rowBody}>{after}</div>
        </div>
      </div>
    </section>
  )
}

function Hero({ legacy }: { legacy: boolean }) {
  const HeroComponent = legacy ? LegacyPageHero : PageHero
  return (
    <HeroComponent
      icon={Sparkles}
      eyebrow="Status do núcleo"
      title="Visão geral"
      description="Como a Mika está agora: provedor, memória e o que precisa da sua atenção."
      pills={
        legacy ? (
          <>
            <LegacyStatusPill variant="ok" label="online" dot />
            <LegacyStatusPill variant="neutral" label="v1.2.3" />
          </>
        ) : (
          <>
            <StatusPill variant="ok" label="online" dot />
            <StatusPill variant="neutral" label="v1.2.3" />
          </>
        )
      }
      note="Dados fictícios desta comparação; nenhuma configuração real é lida."
      aside={
        legacy ? (
          <>
            <LegacyStatTile label="Mensagens" value="128" hint="no histórico" icon={History} />
            <LegacyStatTile label="Memórias" value="42" hint="episódicas" tone="ok" icon={Lightbulb} />
          </>
        ) : (
          <>
            <StatTile label="Mensagens" value="128" hint="no histórico" icon={History} />
            <StatTile label="Memórias" value="42" hint="episódicas" tone="ok" icon={Lightbulb} />
          </>
        )
      }
    />
  )
}

function Pills({ legacy }: { legacy: boolean }) {
  const Pill = legacy ? LegacyStatusPill : StatusPill
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
      {PILLS.map((pill) => (
        <Pill key={pill.variant} variant={pill.variant} label={pill.label} dot={pill.dot} />
      ))}
    </div>
  )
}

function Tiles({ legacy }: { legacy: boolean }) {
  const Tile = legacy ? LegacyStatTile : StatTile
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 8 }}>
      {TONES.map(({ tone, label, value, hint, icon: Icon }) => (
        <Tile key={tone} tone={tone} label={label} value={value} hint={hint} icon={Icon} />
      ))}
    </div>
  )
}

function Empty({ legacy }: { legacy: boolean }) {
  const EmptyComponent = legacy ? LegacyEmptyState : EmptyState
  return (
    <EmptyComponent
      icon={Users}
      title="Nenhuma pessoa no catálogo"
      hint="Pessoas aparecem aqui quando a Mika aprende um nome numa conversa."
      action={
        legacy ? (
          <button
            type="button"
            className="rounded-md border border-border-subtle px-2.5 py-1 text-caption text-text-muted"
          >
            adicionar pessoa
          </button>
        ) : (
          <Button size="sm" variant="secondary">
            adicionar pessoa
          </Button>
        )
      }
    />
  )
}

function Confirm({ legacy }: { legacy: boolean }) {
  const ConfirmComponent = legacy ? LegacyConfirmButton : ConfirmButton
  return (
    <div style={{ display: 'grid', gap: 10 }}>
      <p style={{ margin: 0, fontSize: 12, color: 'var(--color-text-muted)' }}>
        Clique em “parar” para ver a pergunta dos dois lados.
      </p>
      <div>
        <ConfirmComponent
          ariaLabel="parar"
          label="parar"
          confirmLabel="sim, parar"
          onConfirm={() => {}}
          icon={Trash2}
        />
      </div>
    </div>
  )
}

function App() {
  const [tick, setTick] = useState(0)
  void tick
  return (
    <div style={page}>
      <header>
        <h1 style={{ margin: 0, fontSize: 20 }}>Mika · trocas de componentes pelo Sonido</h1>
        <p style={{ margin: '4px 0 0', fontSize: 12, color: 'var(--color-text-muted)' }}>
          Mesmo conteúdo, mesmo tema escuro, dados fictícios. À esquerda o componente que o app tinha; à direita o do
          design system.
        </p>
      </header>

      <Row
        title="PageHero → PageHeader"
        hint="Cabeçalho canônico das páginas de Configurações: ícone, pílulas, título, snapshot e nota."
        before={<Hero legacy />}
        after={<Hero legacy={false} />}
      />

      <Row
        title="StatusPill → Badge"
        hint="Dez variantes de estado usadas em cards, toggles e heróis."
        before={<Pills legacy />}
        after={<Pills legacy={false} />}
      />

      <Row
        title="StatTile"
        hint="Bloco de número com tom e ícone; à esquerda o componente local, à direita o do Sonido."
        before={<Tiles legacy />}
        after={<Tiles legacy={false} />}
      />

      <Row
        title="EmptyState"
        hint="Lista vazia com ícone, motivo e uma saída."
        before={<Empty legacy />}
        after={<Empty legacy={false} />}
      />

      <Row
        title="ConfirmButton"
        hint="Ação destrutiva em dois passos: clique em “parar” nos dois lados para armar a pergunta."
        before={<Confirm legacy />}
        after={<Confirm legacy={false} />}
      />

      <footer style={{ fontSize: 11, color: 'var(--color-text-subtle)' }}>
        <button type="button" onClick={() => setTick((n) => n + 1)} style={{ fontSize: 11 }}>
          reiniciar estados
        </button>
      </footer>
    </div>
  )
}

const root = document.getElementById('root')
if (root) createRoot(root).render(<App />)
