// Evita que se abra una consola adicional en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    moneywatcher_lib::run()
}
