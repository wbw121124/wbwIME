pub mod session;
pub mod candidate;
pub mod context;
pub mod error;

// 重新导出常用类型
pub use session::{SessionManager, SessionState, SessionEvent, SessionStats};
pub use candidate::{CandidateList, CandidateSelector, CandidateFilter};
pub use context::{ContextManager, ContextEvent, ContextSnapshot};
pub use error::{CoreError, CoreResult, ErrorContext, RecoveryStrategy};