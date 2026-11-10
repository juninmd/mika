import { motion } from 'framer-motion'
import { getMoodConfig, type MoodRecord } from './mood-hud-config'

export function MoodHistoryTrail({ history }: { history: MoodRecord[] }) {
  return (
    <div className="flex items-center gap-0.5 ml-auto" aria-hidden="true">
      {history.map((h, i) => {
        const hCfg = getMoodConfig(h.label)
        const opacity = 0.3 + (i / (history.length || 1)) * 0.4
        return (
          <motion.span
            key={`${h.at}-${h.label}`}
            className="text-micro"
            initial={{ opacity: 0, scale: 0.8 }}
            animate={{ opacity, scale: 1 }}
            transition={{ duration: 0.18 }}
            style={{ color: hCfg.color }}
          >
            {hCfg.emoji}
          </motion.span>
        )
      })}
    </div>
  )
}
