pub mod ime_host;
pub mod candidate_window;
pub mod key_mapper;

// 重新导出常用类型
pub use ime_host::{ImeHost, ImeConfig, ImeState, ImeResponse, ImeEvent, ImeFactory};
pub use candidate_window::{CandidateWindow, CandidateWindowManager, WindowPosition, WindowStyle};
pub use key_mapper::{KeyMapper, KeyEvent, KeyAction, KeyType};