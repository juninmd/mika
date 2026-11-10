import { motion } from 'framer-motion'
import { Info, type LucideIcon } from 'lucide-react'
import { isValidElement, type ReactNode } from 'react'
import { REVEAL } from '../motion'
import { cn } from '../utils/cn'

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
  /** Grade de `StatTile` à direita — o snapshot numérico da página. */
  aside?: ReactNode
  /** Conteúdo extra dentro do card (resultado de teste, aviso). */
  children?: ReactNode
  headingId?: string
  className?: string
}

function HeroIcon({ icon }: { icon: PageHeroProps['icon'] }) {
  const Icon = icon as LucideIcon
  return (
    <div
      aria-hidden="true"
      className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent shadow-[0_0_18px_var(--accent-glow)]"
    >
      {isValidElement(icon) ? icon : <Icon size={20} />}
    </div>
  )
}

/**
 * Cabeçalho canônico de página de configurações (padrão Mika > Personagem): ícone temático,
 * pílulas de estado, título, descrição, ações e snapshot numérico. Antes, três cabeçalhos
 * diferentes (hero de feature, hero de deck e um `h1` solto) conviviam na mesma janela, com
 * tamanhos de título, ícone e raio distintos de uma aba para a outra.
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
  headingId = 'page-hero-heading',
  className,
}: PageHeroProps) {
  return (
    <motion.section
      aria-labelledby={headingId}
      variants={REVEAL.item}
      className={cn(
        'relative overflow-hidden rounded-2xl border border-border-subtle bg-bg-surface p-5',
        'bg-[radial-gradient(120%_140%_at_0%_0%,var(--accent-soft),transparent_55%)]',
        className,
      )}
      style={{ boxShadow: '0 18px 44px rgba(0,0,0,0.18), inset 0 1px 0 var(--border-subtle)' }}
    >
      <div className="flex flex-wrap items-center gap-x-6 gap-y-4">
        <div className="flex min-w-0 flex-1 basis-80 items-center gap-4">
          {leading}
          <HeroIcon icon={icon} />
          <div className="min-w-0 flex-1">
            {pills ? (
              <div className="mb-1.5 flex flex-wrap items-center gap-1.5">{pills}</div>
            ) : (
              eyebrow && (
                <p className="mb-1 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{eyebrow}</p>
              )
            )}
            <h2 id={headingId} className="font-display text-lg font-bold tracking-tight text-text-main">
              {title}
            </h2>
            {description && <p className="mt-0.5 max-w-2xl text-xs leading-relaxed text-text-muted">{description}</p>}
          </div>
        </div>
        {actions && <div className="flex shrink-0 flex-wrap items-center gap-3">{actions}</div>}
        {aside && <div className="grid w-full shrink-0 grid-cols-2 gap-3 sm:w-auto">{aside}</div>}
      </div>
      {children}
      {note && (
        <p className="mt-4 flex items-start gap-2 border-t border-border-subtle pt-3 text-xs leading-relaxed text-text-muted">
          <Info className="mt-0.5 size-3.5 shrink-0 text-info" aria-hidden="true" />
          <span>{note}</span>
        </p>
      )}
    </motion.section>
  )
}
