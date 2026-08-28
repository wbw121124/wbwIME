#![allow(clippy::upper_case_acronyms, dead_code, private_interfaces)]

pub mod guid;
pub mod state;
pub mod output;
pub mod text_service;
pub mod dll;

pub use dll::{DllMain, DllGetClassObject, DllCanUnloadNow, DllRegisterServer, DllUnregisterServer};
