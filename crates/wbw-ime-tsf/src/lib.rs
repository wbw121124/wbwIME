#![allow(clippy::upper_case_acronyms, dead_code, private_interfaces)]

pub mod dll;
pub mod guid;
pub mod ipc;
pub mod log;
pub mod output;
pub mod state;
pub mod text_service;

pub use dll::{
    DllCanUnloadNow, DllGetClassObject, DllMain, DllRegisterServer, DllUnregisterServer,
};
