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
    pub candidates: Vec<Candidate>,
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
            builder.build_fst()
        };
        let matcher = Matcher::with_dict(
            MatcherConfig { fuzzy_enabled: true, ..MatcherConfig::default() },
            dict,
        );
        let ranker = Ranker::new(RankConfig::default());
        let host = ImeHost::new(ImeConfig::default());
        Some(Self {
            _host: host, matcher, ranker,
            buffer: String::new(), composing: false,
            candidates: Vec::new(), selected_index: 0,
            composing_text: String::new(), commit_text: None,
        })
    }

    pub fn update_candidates(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.selected_index = 0;
            self.composing_text.clear();
            return;
        }
        let ctx = InputContext {
            buffer: self.buffer.clone(), cursor: self.buffer.len(),
            mode: InputMode::Pinyin, selected: Vec::new(), session_id: 0,
        };
        let matched = self.matcher.match_input(&ctx);
        self.candidates = self.ranker.rank(matched);
        self.selected_index = 0;
        self.composing_text = self.buffer.clone();
    }

    pub fn process_key(&mut self, vkey: u32) {
        self.commit_text = None;
        match vkey {
            0x0D => { // Enter
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    self.commit_text = Some(self.candidates[self.selected_index].text.clone());
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0;
                    self.composing = false; self.composing_text.clear();
                }
            }
            0x08 => { // Backspace
                if !self.buffer.is_empty() {
                    self.buffer.pop(); self.update_candidates();
                    self.composing = !self.buffer.is_empty();
                }
            }
            0x1B => { // Escape
                self.buffer.clear(); self.candidates.clear(); self.selected_index = 0;
                self.composing = false; self.composing_text.clear();
            }
            0x20 => { // Space -> select first
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    self.commit_text = Some(self.candidates[0].text.clone());
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0;
                    self.composing = false; self.composing_text.clear();
                }
            }
            0x31..=0x39 => {
                let idx = (vkey - 0x31) as usize;
                if !self.buffer.is_empty() && idx < self.candidates.len() {
                    self.commit_text = Some(self.candidates[idx].text.clone());
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0;
                    self.composing = false; self.composing_text.clear();
                }
            }
            0x30 => {
                if !self.buffer.is_empty() && self.candidates.len() > 9 {
                    self.commit_text = Some(self.candidates[9].text.clone());
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0;
                    self.composing = false; self.composing_text.clear();
                }
            }
            0x41..=0x5A => { // A-Z
                self.buffer.push((vkey as u8 + 0x20) as char);
                self.update_candidates();
                self.composing = true;
            }
            _ => {}
        }
    }
}
