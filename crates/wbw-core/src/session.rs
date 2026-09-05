//! Session 管理模块

use crate::candidate::CandidateList;
use crate::context::{ContextManager, ContextSnapshot};
use std::collections::{HashMap, VecDeque};
use wbw_types::SessionConfig;

/// 会话管理器
pub struct SessionManager {
    /// 活跃会话
    sessions: HashMap<u64, SessionState>,
    /// 下一个会话 ID
    next_id: u64,
    /// 默认配置
    default_config: SessionConfig,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
            default_config: SessionConfig::default(),
        }
    }

    /// 创建新会话
    pub fn create_session(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let state = SessionState::new(id, self.default_config.clone());
        self.sessions.insert(id, state);

        id
    }

    /// 创建带配置的会话
    pub fn create_session_with_config(&mut self, config: SessionConfig) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let state = SessionState::new(id, config);
        self.sessions.insert(id, state);

        id
    }

    /// 获取会话状态
    pub fn get_session(&self, id: u64) -> Option<&SessionState> {
        self.sessions.get(&id)
    }

    /// 获取可变会话状态
    pub fn get_session_mut(&mut self, id: u64) -> Option<&mut SessionState> {
        self.sessions.get_mut(&id)
    }

    /// 关闭会话
    pub fn close_session(&mut self, id: u64) -> bool {
        self.sessions.remove(&id).is_some()
    }

    /// 获取所有活跃会话
    pub fn active_sessions(&self) -> Vec<u64> {
        self.sessions.keys().cloned().collect()
    }

    /// 获取活跃会话数量
    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }

    /// 关闭所有会话
    pub fn close_all(&mut self) {
        self.sessions.clear();
    }

    /// 获取默认配置
    pub fn default_config(&self) -> &SessionConfig {
        &self.default_config
    }

    /// 设置默认配置
    pub fn set_default_config(&mut self, config: SessionConfig) {
        self.default_config = config;
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 会话状态
pub struct SessionState {
    /// 会话 ID
    pub id: u64,
    /// 上下文管理器
    pub context: ContextManager,
    /// 候选词列表
    pub candidates: CandidateList,
    /// 会话配置
    pub config: SessionConfig,
    /// 会话创建时间（UNIX 纪元秒数，内存态，不参与序列化）
    pub created_at: u64,
    /// 最后活动时间（UNIX 纪元秒数，内存态，不参与序列化）
    pub last_active: u64,
    /// 历史快照
    pub snapshots: VecDeque<ContextSnapshot>,
}

impl SessionState {
    /// 创建新的会话状态
    pub fn new(id: u64, config: SessionConfig) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            context: ContextManager::new(id),
            candidates: CandidateList::new(Vec::new(), 0, 10),
            config,
            created_at: now,
            last_active: now,
            snapshots: VecDeque::new(),
        }
    }

    /// 更新最后活动时间
    pub fn touch(&mut self) {
        self.last_active = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// 保存快照
    pub fn save_snapshot(&mut self) {
        let snapshot = ContextSnapshot::from(self.context.current());
        self.snapshots.push_back(snapshot);

        // 限制快照数量
        if self.snapshots.len() > 50 {
            self.snapshots.pop_front();
        }
    }

    /// 恢复快照
    pub fn restore_snapshot(&mut self, index: usize) -> bool {
        if let Some(snapshot) = self.snapshots.get(index) {
            let ctx = snapshot.to_input_context(self.id);
            let mut ctx_mgr = ContextManager::new(self.id);
            *ctx_mgr.current_mut() = ctx;
            self.context = ctx_mgr;
            true
        } else {
            false
        }
    }

    /// 获取快照数量
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// 清空快照
    pub fn clear_snapshots(&mut self) {
        self.snapshots.clear();
    }

    /// 获取会话 ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 获取配置
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: SessionConfig) {
        self.config = config;
    }

    /// 检查会话是否超时
    pub fn is_timeout(&self, timeout_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(self.last_active) > timeout_secs
    }

    /// 获取会话持续时间
    pub fn duration_secs(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(self.created_at)
    }
}

/// 会话事件
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// 会话创建
    Created(u64),
    /// 会话关闭
    Closed(u64),
    /// 会话超时
    Timeout(u64),
    /// 会话错误
    Error(u64, String),
}

/// 会话统计信息
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    /// 总会话数
    pub total_sessions: usize,
    /// 活跃会话数
    pub active_sessions: usize,
    /// 平均会话时长（秒）
    pub avg_duration_secs: f64,
    /// 平均候选词数
    pub avg_candidates: f64,
    /// 总输入字符数
    pub total_chars: usize,
}
