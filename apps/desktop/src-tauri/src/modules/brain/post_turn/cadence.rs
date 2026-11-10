//! Porte de `PostTurnPipeline.shouldRunHeavyExtraction` (`brain/chat/pipeline.ts`): decide se ESTE
//! turno é "pesado" (extração de contexto + tentativa de consolidação) ou "leve" (só o que já roda
//! sempre — episódica + fatos). Contador de instância, mesmo formato que
//! `super::super::reflection::cadence::ReflectionCadence`, mas com regra DIFERENTE: aqui o
//! intervalo muda conforme o tamanho da mensagem, lá é fixo a cada 5.

/// = `SHORT_MSG_THRESHOLD`: abaixo disso (os dois lados do turno) conta como "mensagem curta".
const SHORT_MSG_THRESHOLD: usize = 30;
/// = `HEAVY_INTERVAL_LONG`: intervalo entre turnos pesados quando o turno NÃO é curto.
const HEAVY_INTERVAL_LONG: u32 = 2;
/// = `HEAVY_INTERVAL_SHORT`: intervalo maior quando é curto — conversa de "ok"/"kkk" não merece
/// extração pesada com a mesma frequência de uma conversa de verdade.
const HEAVY_INTERVAL_SHORT: u32 = 4;

/// = o `turnCounter` privado de `PostTurnPipeline`.
#[derive(Debug, Default)]
pub struct HeavyCadence {
    turn_counter: u32,
}

impl HeavyCadence {
    pub fn new() -> Self {
        Self::default()
    }

    /// = `shouldRunHeavyExtraction`: incrementa o contador e testa `% interval == 0` — mesma ordem
    /// do TS (incrementa ANTES de testar, então o primeiro turno já pode ser pesado se o intervalo
    /// for 1, o que nunca acontece aqui, mas mantém a paridade de comportamento).
    pub fn should_run_heavy(&mut self, user: &str, assistant: &str) -> bool {
        self.turn_counter += 1;
        let is_short = user.chars().count() < SHORT_MSG_THRESHOLD && assistant.chars().count() < SHORT_MSG_THRESHOLD;
        let interval = if is_short { HEAVY_INTERVAL_SHORT } else { HEAVY_INTERVAL_LONG };
        self.turn_counter % interval == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marca_pesado_a_cada_2_turnos_longos() {
        let mut cadence = HeavyCadence::new();
        let long = "a".repeat(40);
        assert!(!cadence.should_run_heavy(&long, &long), "turno 1");
        assert!(cadence.should_run_heavy(&long, &long), "turno 2");
        assert!(!cadence.should_run_heavy(&long, &long), "turno 3");
        assert!(cadence.should_run_heavy(&long, &long), "turno 4");
    }

    #[test]
    fn marca_pesado_a_cada_4_turnos_curtos() {
        let mut cadence = HeavyCadence::new();
        for expected in [false, false, false, true] {
            assert_eq!(cadence.should_run_heavy("oi", "kk"), expected);
        }
    }

    /// Um dos dois lados curto e o outro longo NÃO conta como "curto" — os DOIS precisam estar
    /// abaixo do limite, mesma condição `&&` do TS.
    #[test]
    fn precisa_dos_dois_lados_curtos_para_usar_o_intervalo_maior() {
        let mut cadence = HeavyCadence::new();
        let long = "a".repeat(40);
        assert!(!cadence.should_run_heavy("oi", &long));
        assert!(cadence.should_run_heavy("oi", &long), "com um lado longo, intervalo é 2");
    }
}
