//! A cadência "só a cada 5º turno" — porte de `ReflectionService`'s `this.counter`.

pub(super) const REFLECTION_INTERVAL: u32 = 5;

/// Instanciada por [`super::super::post_turn::listener`] (uma por processo, ao lado do resto do
/// estado do turno assembler).
#[derive(Debug, Default)]
pub struct ReflectionCadence {
    counter: u32,
}

impl ReflectionCadence {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Avança um turno e devolve `true` só quando é a vez do ciclo. Mesma aritmética de
    /// `this.counter++; if (this.counter % REFLECTION_INTERVAL !== 0) return null` — `0 % N == 0`
    /// nunca acontece com contador partindo de 1, então o 1º turno nunca dispara, só o 5º.
    pub fn tick(&mut self) -> bool {
        self.counter = self.counter.wrapping_add(1);
        self.counter % REFLECTION_INTERVAL == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cadence_so_dispara_a_cada_quinto_turno() {
        let mut cadence = ReflectionCadence::new();
        for _ in 0..4 {
            assert!(!cadence.tick());
        }
        assert!(cadence.tick());
    }

    #[test]
    fn cadence_continua_disparando_no_multiplo_seguinte_do_intervalo() {
        let mut cadence = ReflectionCadence::new();
        for _ in 0..9 {
            cadence.tick();
        }
        assert!(cadence.tick()); // 10º turno
    }

}
