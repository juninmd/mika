import { defineMikaUI } from '@mika/sdk-ui'
import { GameHudWidget } from './widgets/game-hud.widget'
import { MiniGamesWidget } from './widgets/mini-games.widget'
import { SteamLibraryWidget } from './widgets/steam-library.widget'

export default defineMikaUI({
  views: {
    miniGames: MiniGamesWidget,
    gameHud: GameHudWidget,
    steamLibrary: SteamLibraryWidget,
  },
})
