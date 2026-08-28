pub mod candidate_window;
pub mod ime_host;
pub mod key_mapper;

// 重新导出常用类型
pub use candidate_window::{CandidateWindow, CandidateWindowManager, WindowPosition, WindowStyle};
pub use ime_host::{ImeConfig, ImeEvent, ImeFactory, ImeHost, ImeResponse, ImeState};
pub use key_mapper::{KeyAction, KeyEvent, KeyMapper, KeyType};
