import { PageHeader as SonidoPageHeader } from '@juninmd/sonido'
import type { LucideIcon } from 'lucide-react'
import { isValidElement, type ReactNode } from 'react'

interface PageHeroProps {
  /** Ícone lucide ou nó já dimensionado (glifo de plugin, monograma). */
  icon: LucideIcon | ReactNode
  title: string
  /** Rótulo trackeado acima do título (ex.: "Hub do sistema"); some quando há `pills`. */
  eyebrow?: string
  /** Fatos rápidos em `StatusPill` (categoria, estado, contagem). */
  pills?: ReactNode
  description?: ReactNode
  /** Linha de contexto com ícone de info, abaixo do cabeçalho. */
  note?: ReactNode
  /** Botão antes do ícone (ex.: voltar para Módulos). */
  leading?: ReactNode
  /** Ações à direita (testar, liga/desliga, atualizar). */
  actions?: ReactNode
  /** Grade de `StatTile` à direita - o snapshot numérico da página. */
  aside?: ReactNode
  /** Conteúdo extra dentro do card (resultado de teste, aviso). */
  children?: ReactNode
  /** Mantido na API: o Sonido rotula a seção pelo título, não por `aria-labelledby`. */
  headingId?: string
  className?: string
}

function HeroIcon({ icon }: { icon: PageHeroProps['icon'] }) {
  if (isValidElement(icon)) return icon
  const Icon = icon as LucideIcon
  return <Icon size={20} aria-hidden="true" />
}

/**
 * Canonical settings page header. Layout, icon frame, pills, aside grid and the note line come
 * from Sonido's `PageHeader`; the app keeps its props. `eyebrow` renders as the tracked label in
 * the pills row when no pills were given, matching the old priority (`pills` wins).
 */
export function PageHero({
  icon,
  title,
  eyebrow,
  pills,
  description,
  note,
  leading,
  actions,
  aside,
  children,
  className,
}: PageHeroProps) {
  const eyebrowNode =
    eyebrow && !pills ? (
      <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{eyebrow}</span>
    ) : undefined
  return (
    <SonidoPageHeader
      accent
      title={title}
      icon={<HeroIcon icon={icon} />}
      pills={pills ?? eyebrowNode}
      description={description}
      leading={leading}
      actions={actions}
      aside={aside}
      note={note}
      titleAs="h2"
      className={className}
    >
      {children}
    </SonidoPageHeader>
  )
}
