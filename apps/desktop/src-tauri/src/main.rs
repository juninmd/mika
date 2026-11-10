// Sem console no Windows em release — o app é um avatar, não uma CLI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mika_desktop_lib::run()
}
