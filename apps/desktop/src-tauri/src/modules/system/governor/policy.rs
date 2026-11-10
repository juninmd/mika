//! Núcleo PURO do governador de energia: sinais do sistema -> estado -> orçamento de recursos. Sem
//! relógio, sem SO, sem config — [`super`] lê tudo isso e só chama estas funções.
//!
//! **Por quê.** A Mika roda do lado de um jogo. Antes, cada laço de fundo tinha cadência fixa (visão
//! a cada 60s, clipboard a cada 1,5s, cursor a cada 125ms, janela ativa a cada 4s...) e o ORT usava
//! todos os núcleos no Whisper/Supertonic — o mesmo custo com a pessoa conversando, fora do PC ou no
//! meio de uma partida. Um único orçamento, decidido aqui, escala todos eles.

use std::time::Duration;

/// `app.performanceProfile` — o que a pessoa escolheu em Configurações.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Auto,
    Economy,
    Quality,
}

impl Profile {
    /// Valor desconhecido (config de outra versão) vira `Auto`, nunca erro.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "economy" => Self::Economy,
            "quality" => Self::Quality,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Economy => "economy",
            Self::Quality => "quality",
        }
    }
}

/// O que a máquina está fazendo agora, do ponto de vista de quanto a Mika pode gastar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerState {
    /// Alguém no PC ou falando com a Mika.
    Active,
    /// Ninguém tocou no PC nem falou com a Mika por [`IDLE_AFTER`].
    Idle,
    /// Jogo detectado (`GameChanged`) — o jogo manda na CPU/GPU.
    Gaming,
    /// Notebook fora da tomada.
    Battery,
}

impl PowerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Gaming => "gaming",
            Self::Battery => "battery",
        }
    }
}

pub const IDLE_AFTER: Duration = Duration::from_secs(5 * 60);

pub struct Signals {
    pub gaming: bool,
    pub on_battery: bool,
    /// Tempo sem input no SO; `None` quando o SO não informa (Wayland puro, headless).
    pub os_idle: Option<Duration>,
    /// Tempo desde a última interação com a Mika (fala, turno de chat).
    pub since_interaction: Duration,
}

/// Precedência: jogo > bateria > ocioso > ativo. Sem leitura de ócio do SO, só a interação com a
/// Mika decide — fala por voz não gera input de teclado, então as duas precisam concordar.
pub fn state_for(signals: &Signals) -> PowerState {
    if signals.gaming {
        return PowerState::Gaming;
    }
    if signals.on_battery {
        return PowerState::Battery;
    }
    let os_idle = signals.os_idle.is_none_or(|idle| idle >= IDLE_AFTER);
    if os_idle && signals.since_interaction >= IDLE_AFTER {
        return PowerState::Idle;
    }
    PowerState::Active
}

/// Teto de qualidade de efeitos/avatar para o renderer — mesmos nomes de `EffectQuality`
/// (`renderer/src/lib/effect-quality.ts`): `full` 60 fps, `reduced` 30, `eco` 15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ceiling {
    Full,
    Reduced,
    Eco,
}

impl Ceiling {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reduced => "reduced",
            Self::Eco => "eco",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Multiplicador dos laços de manutenção (clipboard, cursor, janela ativa, memória, awareness).
    pub background_scale: u32,
    /// Multiplicador da cadência de captura+análise de tela (o laço mais caro: LLM de visão).
    pub vision_scale: u32,
    pub ceiling: Ceiling,
    /// `true` = threads ONNX pela metade do teto (ver [`onnx_threads`]).
    pub halve_threads: bool,
    /// Modelo ONNX sem uso por este tempo é descarregado (recarrega sob demanda); `None` = nunca.
    pub model_idle_unload: Option<Duration>,
}

const fn b(background_scale: u32, vision_scale: u32, ceiling: Ceiling, halve_threads: bool, unload_min: u64) -> Budget {
    let model_idle_unload = if unload_min == 0 { None } else { Some(Duration::from_secs(unload_min * 60)) };
    Budget { background_scale, vision_scale, ceiling, halve_threads, model_idle_unload }
}

/// A tabela inteira. `Quality` ignora o estado (a pessoa pediu desempenho máximo); `Economy` é
/// `Auto` apertado em todos os estados.
pub fn budget(profile: Profile, state: PowerState) -> Budget {
    use Ceiling::*;
    use PowerState::*;
    match (profile, state) {
        (Profile::Quality, _) => b(1, 1, Full, false, 0),
        (Profile::Auto, Active) => b(1, 1, Full, false, 15),
        (Profile::Auto, Idle) => b(4, 4, Eco, false, 5),
        (Profile::Auto, Gaming) => b(3, 2, Reduced, true, 10),
        (Profile::Auto, Battery) => b(2, 2, Reduced, true, 10),
        (Profile::Economy, Active) => b(2, 2, Reduced, true, 5),
        (Profile::Economy, Idle) => b(6, 6, Eco, true, 2),
        (Profile::Economy, Gaming | Battery) => b(4, 4, Eco, true, 3),
    }
}

/// Teto de threads intra-op do ORT: metade dos núcleos FÍSICOS (o jogo e o LLM local disputam os
/// mesmos), no máximo 8; `halve` corta pela metade de novo. Nunca menos de 1.
pub fn onnx_threads(physical_cores: usize, halve: bool) -> usize {
    let cap = (physical_cores / 2).clamp(1, 8);
    if halve { (cap / 2).max(1) } else { cap }
}

/// `base * scale`, saturando — o laço dorme isto entre dois ticks.
pub fn scaled(base: Duration, scale: u32) -> Duration {
    base.saturating_mul(scale.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(gaming: bool, on_battery: bool, os_idle_s: Option<u64>, since_s: u64) -> Signals {
        Signals { gaming, on_battery, os_idle: os_idle_s.map(Duration::from_secs), since_interaction: Duration::from_secs(since_s) }
    }

    #[test]
    fn a_game_wins_over_battery_and_idleness() {
        assert_eq!(state_for(&signals(true, true, Some(3_600), 3_600)), PowerState::Gaming);
    }

    #[test]
    fn battery_wins_over_idleness() {
        assert_eq!(state_for(&signals(false, true, Some(3_600), 3_600)), PowerState::Battery);
    }

    #[test]
    fn idle_needs_both_the_os_and_mika_to_be_quiet() {
        assert_eq!(state_for(&signals(false, false, Some(600), 600)), PowerState::Idle);
        // Falando por voz: nenhuma tecla, mas a conversa é recente.
        assert_eq!(state_for(&signals(false, false, Some(600), 30)), PowerState::Active);
        // Digitando em outro app, sem falar com a Mika.
        assert_eq!(state_for(&signals(false, false, Some(10), 600)), PowerState::Active);
    }

    #[test]
    fn without_an_os_idle_reading_only_the_conversation_decides() {
        assert_eq!(state_for(&signals(false, false, None, 600)), PowerState::Idle);
        assert_eq!(state_for(&signals(false, false, None, 5)), PowerState::Active);
    }

    #[test]
    fn quality_never_throttles_in_any_state() {
        for state in [PowerState::Active, PowerState::Idle, PowerState::Gaming, PowerState::Battery] {
            let got = budget(Profile::Quality, state);
            assert_eq!((got.background_scale, got.vision_scale, got.ceiling), (1, 1, Ceiling::Full));
            assert_eq!(got.model_idle_unload, None, "Qualidade mantém os modelos carregados");
        }
    }

    #[test]
    fn auto_runs_at_full_speed_while_the_user_is_active() {
        let got = budget(Profile::Auto, PowerState::Active);
        assert_eq!((got.background_scale, got.vision_scale, got.ceiling, got.halve_threads), (1, 1, Ceiling::Full, false));
    }

    /// O pedido do produto: rodando junto de um jogo, ela recua — laços de fundo, visão, fps e threads.
    #[test]
    fn auto_backs_off_while_gaming() {
        let active = budget(Profile::Auto, PowerState::Active);
        let gaming = budget(Profile::Auto, PowerState::Gaming);
        assert!(gaming.background_scale > active.background_scale);
        assert!(gaming.vision_scale > active.vision_scale);
        assert_ne!(gaming.ceiling, Ceiling::Full);
        assert!(gaming.halve_threads);
    }

    #[test]
    fn economy_is_never_looser_than_auto() {
        for state in [PowerState::Active, PowerState::Idle, PowerState::Gaming, PowerState::Battery] {
            let (auto, eco) = (budget(Profile::Auto, state), budget(Profile::Economy, state));
            assert!(eco.background_scale >= auto.background_scale, "{state:?}");
            assert!(eco.vision_scale >= auto.vision_scale, "{state:?}");
            assert!(eco.model_idle_unload <= auto.model_idle_unload, "{state:?}");
        }
    }

    #[test]
    fn onnx_threads_are_capped_at_half_the_physical_cores() {
        assert_eq!(onnx_threads(8, false), 4);
        assert_eq!(onnx_threads(8, true), 2);
        assert_eq!(onnx_threads(32, false), 8, "teto absoluto");
        assert_eq!(onnx_threads(1, false), 1);
        assert_eq!(onnx_threads(2, true), 1, "nunca zero");
    }

    #[test]
    fn unknown_profiles_fall_back_to_auto() {
        assert_eq!(Profile::parse("economy"), Profile::Economy);
        assert_eq!(Profile::parse("quality"), Profile::Quality);
        assert_eq!(Profile::parse("turbo"), Profile::Auto);
    }

    /// Acordadas por minuto de cada laço de fundo, por estado (perfil Automático). Não é asserção
    /// de tabela: prova que nenhum estado fora de "ativo" acorda mais que o ativo.
    #[test]
    fn no_state_wakes_more_than_active() {
        let per_min = |base_ms: u64, scale: u32| 60_000 / scaled(Duration::from_millis(base_ms), scale).as_millis() as u64;
        let active = budget(Profile::Auto, PowerState::Active);
        for state in [PowerState::Idle, PowerState::Gaming, PowerState::Battery] {
            let got = budget(Profile::Auto, state);
            for base in [125, 1_500, 4_000, 30_000, 120_000] {
                assert!(per_min(base, got.background_scale) <= per_min(base, active.background_scale));
            }
        }
    }
}
