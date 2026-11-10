/** Setas/Home/End movem a seleção como num radiogroup nativo; devolve null para tecla irrelevante. */
export function choiceTargetIndex(key: string, index: number, count: number): number | null {
  if (key === 'Home') return 0
  if (key === 'End') return count - 1
  if (key === 'ArrowRight' || key === 'ArrowDown') return (index + 1) % count
  if (key === 'ArrowLeft' || key === 'ArrowUp') return (index - 1 + count) % count
  return null
}

/** Roving tabindex: só a opção marcada entra no Tab; sem marcada (valor antigo), a primeira. */
export function choiceFocusIndex(options: ReadonlyArray<{ value: string }>, value: string): number {
  return Math.max(
    0,
    options.findIndex((o) => o.value === value),
  )
}

/** Sem descrição em nenhuma opção: card compacto, só ícone + rótulo. */
export function isDenseChoice(options: ReadonlyArray<{ description?: string }>): boolean {
  return options.every((o) => !o.description)
}

/**
 * Colunas pela LARGURA DO CONTAINER (não da janela) e pela quantidade: a mesma escolha vive tanto
 * numa linha inteira quanto num card estreito, e uma grade `auto-fit` deixava 2+1 órfão ou espremia
 * o texto. Linhas completas sempre que dá: 3 → 1 ou 3, 4 → 1, 2 ou 4.
 */
const COLS: Record<'rich' | 'dense', Record<number, string>> = {
  rich: {
    2: 'grid-cols-1 @2xs:grid-cols-2',
    3: 'grid-cols-1 @xl:grid-cols-3',
    4: 'grid-cols-1 @md:grid-cols-2 @4xl:grid-cols-4',
    5: 'grid-cols-1 @md:grid-cols-2 @3xl:grid-cols-3 @5xl:grid-cols-5',
    6: 'grid-cols-1 @md:grid-cols-2 @3xl:grid-cols-3',
  },
  dense: {
    2: 'grid-cols-2',
    3: 'grid-cols-1 @xs:grid-cols-3',
    4: 'grid-cols-2 @xl:grid-cols-4',
    5: 'grid-cols-2 @lg:grid-cols-3 @3xl:grid-cols-5',
    6: 'grid-cols-2 @lg:grid-cols-3 @4xl:grid-cols-6',
  },
}

export function choiceGridCols(count: number, dense: boolean): string {
  return COLS[dense ? 'dense' : 'rich'][Math.min(count, 6)] ?? 'grid-cols-1'
}
