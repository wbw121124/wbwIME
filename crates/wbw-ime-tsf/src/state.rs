use wbw_dict::{DictBuilder, FstDict};
use wbw_imekit::{ImeConfig, ImeHost};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::Ranker;
use wbw_types::{Candidate, InputContext, InputMode, RankConfig};

pub struct ImeState {
    pub _host: ImeHost,
    pub matcher: Matcher,
    pub ranker: Ranker,
    pub buffer: String,
    pub composing: bool,
    pub chinese_mode: bool,
    /// 当前页候选词（供显示与数字选词）。
    pub candidates: Vec<Candidate>,
    /// 当前缓冲区完整候选词（供翻页）。
    pub all_candidates: Vec<Candidate>,
    pub page: usize,
    pub page_size: usize,
    pub selected_index: usize,
    pub composing_text: String,
    pub commit_text: Option<String>,
}

impl ImeState {
    pub fn new(dict_path: &str) -> Option<Self> {
        let path = std::path::Path::new(dict_path);
        let dict = if path.extension().and_then(|e| e.to_str()) == Some("fst") {
            FstDict::from_file(path).ok()?
        } else {
            let mut builder = DictBuilder::new();
            builder.load_cin(path).ok()?;
            builder.deduplicate();
            builder.sort();
            builder.build_fst().ok()?
        };
        let matcher = Matcher::with_dict(
            MatcherConfig {
                fuzzy_enabled: true,
                ..MatcherConfig::default()
            },
            dict,
        );
        let ranker = Ranker::new(RankConfig::default());
        let host = ImeHost::new(ImeConfig::default());
        Some(Self {
            _host: host,
            matcher,
            ranker,
            buffer: String::new(),
            composing: false,
            chinese_mode: true,
            candidates: Vec::new(),
            all_candidates: Vec::new(),
            page: 0,
            page_size: 10,
            selected_index: 0,
            composing_text: String::new(),
            commit_text: None,
        })
    }

    pub fn update_candidates(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.all_candidates.clear();
            self.selected_index = 0;
            self.composing_text.clear();
            self.page = 0;
            return;
        }
        let ctx = InputContext {
            buffer: self.buffer.clone(),
            cursor: self.buffer.len(),
            mode: InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let matched = self.matcher.match_input(&ctx);
        let ranked = self.ranker.rank(&matched);
        self.all_candidates = ranked;
        self.page = 0;
        self.selected_index = 0;
        self.apply_page();
        self.composing_text = self.buffer.clone();
    }

    /// 将当前页的候选切片写入 `candidates`，供显示与选词。
    fn apply_page(&mut self) {
        let start = self.page * self.page_size;
        self.candidates = self
            .all_candidates
            .iter()
            .skip(start)
            .take(self.page_size)
            .cloned()
            .collect();
    }

    pub fn next_page(&mut self) {
        if self.total_pages() > self.page + 1 {
            self.page += 1;
            self.selected_index = 0;
            self.apply_page();
        }
    }

    pub fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            self.selected_index = 0;
            self.apply_page();
        }
    }

    pub fn total_pages(&self) -> usize {
        if self.all_candidates.is_empty() {
            1
        } else {
            self.all_candidates.len().div_ceil(self.page_size)
        }
    }

    /// 对外部点击选词：选中当前页第 `idx` 个候选并置为待上屏。
    pub fn select_commit(&mut self, idx: usize) {
        self.commit_text = None;
        if !self.buffer.is_empty() && idx < self.candidates.len() {
            self.commit_text = Some(self.candidates[idx].text.clone());
            self.reset_composing();
        }
    }

    /// 清空组合状态（选词/确认共用）。
    fn reset_composing(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.all_candidates.clear();
        self.selected_index = 0;
        self.page = 0;
        self.composing = false;
        self.composing_text.clear();
    }

    pub fn toggle_chinese(&mut self) {
        self.chinese_mode = !self.chinese_mode;
        self.reset_composing();
    }

    /// 公开的组合重置入口（供外部 ks_key_down 在修饰键按下时调用）。
    pub fn reset_composing_ext(&mut self) {
        self.reset_composing();
    }

    pub fn process_key(&mut self, vkey: u32) {
        self.commit_text = None;

        match vkey {
            0x0D => {
                // Enter
                if !self.buffer.is_empty() && !self.candidates.is_empty()
                    && self.selected_index < self.candidates.len() {
                    self.commit_text = Some(self.candidates[self.selected_index].text.clone());
                    self.reset_composing();
                }
            }
            0x08 => {
                // Backspace
                if !self.buffer.is_empty() {
                    self.buffer.pop();
                    self.update_candidates();
                    self.composing = !self.buffer.is_empty();
                }
            }
            0x1B => {
                // Escape
                self.reset_composing();
            }
            0x20 => {
                // Space -> select first
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    self.commit_text = Some(self.candidates[0].text.clone());
                    self.reset_composing();
                }
            }
            0x21 => {
                // PageUp
                if !self.buffer.is_empty() {
                    self.prev_page();
                }
            }
            0x22 => {
                // PageDown
                if !self.buffer.is_empty() {
                    self.next_page();
                }
            }
            0x30 | 0x60 => {
                if !self.buffer.is_empty() && self.candidates.len() > 9 {
                    self.commit_text = Some(self.candidates[9].text.clone());
                    self.reset_composing();
                }
            }
            0x31..=0x39 | 0x61..=0x69 => {
                let idx = ((vkey & 0x0F) - 1) as usize;
                if !self.buffer.is_empty() && idx < self.candidates.len() {
                    self.commit_text = Some(self.candidates[idx].text.clone());
                    self.reset_composing();
                }
            }
            0x41..=0x5A => {
                // A-Z
                if !self.chinese_mode && !self.composing {
                    // 英文模式且未在组合中：字母键透传
                    return;
                }
                if self.buffer.len() < 32 {
                    self.buffer.push((vkey as u8 + 0x20) as char);
                }
                self.update_candidates();
                self.composing = true;
            }
            _ => {}
        }
    }
}
