//! Comandos expuestos al frontend. Cada uno es una capa fina: valida, delega en
//! el núcleo y traduce el error a algo que la interfaz pueda mostrar.

pub mod accounts;
pub mod analytics;
pub mod app_info;
pub mod assistant;
pub mod categories;
pub mod imports;
pub mod rules;
pub mod transactions;
pub mod transfers;
pub mod widgets;
