import { useEffect, useState } from 'react'

export function useResponsiveCols(): number {
  const [cols, setCols] = useState(1)
  useEffect(() => {
    const check = () => setCols(window.innerWidth > 1400 ? 2 : 1)
    check()
    window.addEventListener('resize', check)
    return () => window.removeEventListener('resize', check)
  }, [])
  return cols
}
