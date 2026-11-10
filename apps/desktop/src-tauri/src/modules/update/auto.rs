//! Atualização automática: o que o laço de fundo de `update.rs` faz a cada minuto.
//!
//! A Mika checava UMA vez, no boot, e parava ali: baixar e instalar dependia de o usuário abrir
//! Configurações → Atualizações e clicar duas vezes. Uma companheira que fica dias aberta nunca
//! via a versão seguinte. Aqui mora só a DECISÃO, pura — o laço que a executa fica em `update.rs`.
//!
//! As regras que importam para quem divide o PC com um jogo:
//! - nunca baixa durante jogo nem na bateria (banda e energia são do jogador);
//! - só instala com o PC OCIOSO — instalar reinicia o app, e isso não acontece na cara de ninguém;
//! - com um pacote já baixado não checa de novo: o `check()` do plugin descarta os bytes guardados.

use crate::modules::system::governor::policy::PowerState;

/// `app.autoUpdate` (schema em `config/schemas/mod.rs`, controle em Configurações → Atualizações).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Nada em segundo plano; só o botão manual.
    Off,
    /// Checa sozinha e avisa; baixar e instalar continuam com o usuário.
    Notify,
    /// Checa, baixa e instala sozinha quando o PC fica ocioso.
    Install,
}

impl Mode {
    /// Valor desconhecido cai em `Notify`: avisar é o comportamento que nunca surpreende.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "off" => Self::Off,
            "install" => Self::Install,
            _ => Self::Notify,
        }
    }
}

/// Onde a atualização está, lido do estado que `update.rs` já guarda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Nenhuma versão nova conhecida (ou ainda não checou).
    UpToDate,
    /// O último check achou versão nova; nada baixado.
    Available,
    /// Pacote baixado e com a assinatura verificada, aguardando instalar.
    Downloaded,
}

/// O que já pode ser tentado de novo: a checagem tem intervalo, e um download que falhou espera
/// antes de repetir (sem isso uma rede fora do ar vira uma tentativa por minuto).
#[derive(Clone, Copy, Debug)]
pub struct Due {
    pub check: bool,
    pub download: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    Check,
    Download,
    Install,
}

pub fn next_step(mode: Mode, phase: Phase, due: Due, power: PowerState) -> Step {
    if mode == Mode::Off || phase == Phase::Downloaded && mode != Mode::Install {
        return Step::Wait;
    }
    if phase == Phase::Downloaded {
        return if power == PowerState::Idle { Step::Install } else { Step::Wait };
    }
    let may_download = !matches!(power, PowerState::Gaming | PowerState::Battery);
    if mode == Mode::Install && phase == Phase::Available && due.download && may_download {
        return Step::Download;
    }
    if due.check {
        Step::Check
    } else {
        Step::Wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_DUE: Due = Due { check: true, download: true };
    const NOTHING_DUE: Due = Due { check: false, download: false };
    const EVERY_POWER: [PowerState; 4] = [PowerState::Active, PowerState::Idle, PowerState::Gaming, PowerState::Battery];
    const EVERY_PHASE: [Phase; 3] = [Phase::UpToDate, Phase::Available, Phase::Downloaded];

    #[test]
    fn off_never_touches_the_network_or_the_installer() {
        for phase in EVERY_PHASE {
            for power in EVERY_POWER {
                assert_eq!(next_step(Mode::Off, phase, ALL_DUE, power), Step::Wait, "{phase:?}/{power:?}");
            }
        }
    }

    #[test]
    fn notify_checks_on_schedule_and_leaves_download_and_install_to_the_user() {
        for power in EVERY_POWER {
            assert_eq!(next_step(Mode::Notify, Phase::UpToDate, ALL_DUE, power), Step::Check);
            assert_eq!(next_step(Mode::Notify, Phase::Available, ALL_DUE, power), Step::Check);
            assert_eq!(next_step(Mode::Notify, Phase::Available, NOTHING_DUE, power), Step::Wait);
            assert_eq!(next_step(Mode::Notify, Phase::Downloaded, ALL_DUE, power), Step::Wait);
        }
    }

    #[test]
    fn install_downloads_a_found_update_unless_the_pc_is_gaming_or_on_battery() {
        let no_check = Due { check: false, download: true };
        assert_eq!(next_step(Mode::Install, Phase::Available, no_check, PowerState::Active), Step::Download);
        assert_eq!(next_step(Mode::Install, Phase::Available, no_check, PowerState::Idle), Step::Download);
        assert_eq!(next_step(Mode::Install, Phase::Available, no_check, PowerState::Gaming), Step::Wait);
        assert_eq!(next_step(Mode::Install, Phase::Available, no_check, PowerState::Battery), Step::Wait);
    }

    #[test]
    fn a_failed_download_waits_for_its_retry_window() {
        let failed_recently = Due { check: false, download: false };
        assert_eq!(next_step(Mode::Install, Phase::Available, failed_recently, PowerState::Active), Step::Wait);
        let check_only = Due { check: true, download: false };
        assert_eq!(next_step(Mode::Install, Phase::Available, check_only, PowerState::Active), Step::Check);
    }

    #[test]
    fn install_only_runs_the_installer_while_the_pc_is_idle() {
        assert_eq!(next_step(Mode::Install, Phase::Downloaded, ALL_DUE, PowerState::Idle), Step::Install);
        for power in [PowerState::Active, PowerState::Gaming, PowerState::Battery] {
            assert_eq!(next_step(Mode::Install, Phase::Downloaded, ALL_DUE, power), Step::Wait, "{power:?}");
        }
    }

    #[test]
    fn a_downloaded_package_is_never_thrown_away_by_a_new_check() {
        for mode in [Mode::Notify, Mode::Install] {
            for power in EVERY_POWER {
                assert_ne!(next_step(mode, Phase::Downloaded, ALL_DUE, power), Step::Check, "{mode:?}/{power:?}");
            }
        }
    }

    #[test]
    fn nothing_new_just_checks_on_schedule() {
        assert_eq!(next_step(Mode::Install, Phase::UpToDate, ALL_DUE, PowerState::Gaming), Step::Check);
        assert_eq!(next_step(Mode::Install, Phase::UpToDate, NOTHING_DUE, PowerState::Idle), Step::Wait);
    }

    /// The Updates panel offers these three; a value the parser does not know must not turn into
    /// silent installs.
    #[test]
    fn mode_parses_the_schema_values_and_falls_back_to_notify() {
        assert_eq!(Mode::parse("off"), Mode::Off);
        assert_eq!(Mode::parse("notify"), Mode::Notify);
        assert_eq!(Mode::parse("install"), Mode::Install);
        assert_eq!(Mode::parse(""), Mode::Notify);
        assert_eq!(Mode::parse("auto"), Mode::Notify);
    }
}
