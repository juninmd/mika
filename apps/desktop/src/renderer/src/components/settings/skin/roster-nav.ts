// Arrow-key walk over the roster grid. Left/Right wrap through the whole list; Up/Down move by a
// row and stop at the edge, since wrapping a column jump would land on an unrelated card.
export function rosterKeyTarget(key: string, index: number, count: number, columns: number): number | null {
  if (count <= 0) return null
  switch (key) {
    case 'ArrowRight':
      return (index + 1) % count
    case 'ArrowLeft':
      return (index - 1 + count) % count
    case 'ArrowDown':
      return index + columns < count ? index + columns : index
    case 'ArrowUp':
      return index - columns >= 0 ? index - columns : index
    case 'Home':
      return 0
    case 'End':
      return count - 1
    default:
      return null
  }
}
