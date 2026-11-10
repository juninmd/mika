use super::super::shared;

/// `cargo test` roda os testes do MESMO processo, em paralelo — `shared()` é literalmente um
/// `static`, então qualquer teste que registrasse ou lesse conteúdo nele colidiria com todos os
/// outros. Este teste checa só a IDENTIDADE (mesmo endereço em todas as chamadas), que é a única
/// propriedade que se pode afirmar sobre um singleton sem tocar no que ele guarda.
#[test]
fn shared_returns_the_same_instance_every_time() {
    let first: *const _ = shared();
    let second: *const _ = shared();

    assert!(std::ptr::eq(first, second), "shared() devolveu instâncias diferentes");
}
