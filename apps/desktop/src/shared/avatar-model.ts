/**
 * Avatar default; precisa ser IDÊNTICO entre o estado inicial do renderer e o `default` do schema de
 * `character.modelPath` — divergiam antes, com o schema apontando para uma amostra Live2D fora do
 * pacote distribuído. Mora em `shared/` porque nem renderer nem `character` podem importar o outro.
 */
export const DEFAULT_MODEL_PATH = 'vrm/mika/mika.vrm'
