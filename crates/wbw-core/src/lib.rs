pub mod candidate;
pub mod context;
pub mod error;
pub mod session;

// 重新导出常用类型
pub use candidate::{CandidateFilter, CandidateList, CandidateSelector};
pub use context::{ContextEvent, ContextManager, ContextSnapshot};
pub use error::{CoreError, CoreResult, ErrorContext, RecoveryStrategy};
pub use session::{SessionEvent, SessionManager, SessionState, SessionStats};
