//! Núcleo de MoneyWatcher: toda la lógica financiera vive aquí, sin depender de
//! Tauri ni de ninguna capa de interfaz, para poder probarla de forma aislada.

pub mod domain;

pub use domain::Money;
