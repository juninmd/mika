import { memo, type ReactNode } from 'react'
import { cn } from '../utils/cn'

interface LazyListProps<T> {
  items: readonly T[]

  itemKey: (item: T, index: number) => string | number
  renderItem: (item: T, index: number) => ReactNode

  estimatedItemHeight: number
  className?: string

  empty?: ReactNode
  role?: string
  ariaLabel?: string
}

const Row = memo(function Row({
  children,
  estimatedItemHeight,
  isList,
}: {
  children: ReactNode
  estimatedItemHeight: number
  isList?: boolean
}) {
  return (
    <div
      role={isList ? 'listitem' : undefined}
      style={{ contentVisibility: 'auto', containIntrinsicSize: `0 ${estimatedItemHeight}px` }}
    >
      {children}
    </div>
  )
})

export function LazyList<T>({
  items,
  itemKey,
  renderItem,
  estimatedItemHeight,
  className,
  empty,
  role,
  ariaLabel,
}: LazyListProps<T>) {
  if (items.length === 0 && empty) return <>{empty}</>
  const isList = role === 'list'
  return (
    // biome-ignore lint/a11y/useAriaPropsSupportedByRole: `role` is a caller-supplied prop (both current call sites always pass one alongside ariaLabel); biome can't see through the variable to know it's never the unsupported bare-div "generic" role.
    <div className={cn(className)} role={role} aria-label={ariaLabel}>
      {items.map((item, index) => (
        <Row key={itemKey(item, index)} estimatedItemHeight={estimatedItemHeight} isList={isList}>
          {renderItem(item, index)}
        </Row>
      ))}
    </div>
  )
}
