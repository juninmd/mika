/**
 * Entrada única da página no alvo Tauri: instala a ponte do preload e só então sobe o app.
 * (A ponte é a mesma do Electron; `check:renderer` cobra que o resto do renderer a acesse
 * só por `getElectronApi()`, então o nome dela não se escreve aqui.)
 *
 * No Electron isso era o processo de preload, que rodava antes do renderer por construção.
 * Em Tauri não existe preload, e duas tags `<script type="module">` no HTML não bastaram —
 * o rolldown emitiu uma entrada só e a ponte sumiu do bundle sem erro, deixando a janela
 * transparente sem pintar nada. Uma entrada que importa as duas na ordem certa não depende
 * de como o bundler trata múltiplas entradas.
 *
 * O preload (agora em `src/renderer/bridge/`) é importado sem alteração de lógica:
 * `vite.tauri.config.ts` resolve `shared/electron-runtime` para a versão Tauri, e o
 * contrato congelado em `preload.module.ts` continua valendo.
 */
import '../bridge/index'
import './main'
