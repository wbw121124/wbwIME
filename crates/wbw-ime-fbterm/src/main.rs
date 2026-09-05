//! wbwIME fbterm 输入法服务端（仅 Linux）
//!
//! 实现 fbterm 的 client-server 输入法协议，通过 Unix domain socket 与 fbterm 通信。
//! 用法: wbw-ime-fbterm <词典路径>
//! 启动: fbterm -i wbw-ime-fbterm

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::process;

#[cfg(unix)]
use wbw_dict::{DictBuilder, FstDict};
#[cfg(unix)]
use wbw_matcher::{Matcher, MatcherConfig};
#[cfg(unix)]
use wbw_rank::Ranker;
#[cfg(unix)]
use wbw_types::{Candidate, InputContext, InputMode, RankConfig};

// ========== fbterm IM 协议定义（仅 Linux） ==========

#[cfg(unix)]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MsgType {
    Connect = 1,
    Disconnect = 2,
    Active = 3,
    Deactive = 4,
    SendKey = 5,
    PutText = 6,
    SetWins = 7,
    AckWins = 8,
    CursorPosition = 9,
    FbTermInfo = 10,
    HideUI = 11,
    ShowUI = 12,
    AckHideUI = 13,
}

#[cfg(unix)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct MsgHeader {
    msg_type: u32,
    length: u32,
}

#[cfg(unix)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct ImWin {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[cfg(unix)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct FbTermInfoData {
    term_width: u32,
    term_height: u32,
    rotation: u32,
}

// ========== IM 服务端 ==========

#[cfg(unix)]
struct FbtermIme {
    matcher: Matcher,
    ranker: Ranker,
    buffer: String,
    #[allow(dead_code)]
    active: bool,
    candidates: Vec<Candidate>,
    selected_index: usize,
    #[allow(dead_code)]
    cursor_x: u32,
    #[allow(dead_code)]
    cursor_y: u32,
}

#[cfg(unix)]
impl FbtermIme {
    fn new(dict_path: &str) -> Self {
        let path = std::path::Path::new(dict_path);
        let dict = if path.extension().and_then(|e| e.to_str()) == Some("fst") {
            FstDict::from_file(path).expect("无法加载 .fst 词典")
        } else {
            let mut builder = DictBuilder::new();
            builder.load_cin(path).expect("无法加载 .cin 词典");
            builder.deduplicate();
            builder.sort();
            builder.build_fst().expect("无法构建 FST 词典")
        };

        let matcher_config = MatcherConfig {
            fuzzy_enabled: true,
            ..MatcherConfig::default()
        };
        let matcher = Matcher::with_dict(matcher_config, dict);
        let ranker = Ranker::new(RankConfig::default());

        Self {
            matcher,
            ranker,
            buffer: String::new(),
            active: false,
            candidates: Vec::new(),
            selected_index: 0,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    fn process_key(&mut self, keys: &[u8]) -> Option<String> {
        let &key = keys.first()?;
        match key {
            0x0d | 0x0a => {
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    let text = self.candidates[self.selected_index].text.clone();
                    self.buffer.clear();
                    self.candidates.clear();
                    self.selected_index = 0;
                    Some(text)
                } else {
                    None
                }
            }
            0x08 | 0x7f => {
                if !self.buffer.is_empty() {
                    self.buffer.pop();
                    self.update_candidates();
                }
                None
            }
            0x1b => {
                self.buffer.clear();
                self.candidates.clear();
                self.selected_index = 0;
                None
            }
            0x20 => {
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    let text = self.candidates[0].text.clone();
                    self.buffer.clear();
                    self.candidates.clear();
                    self.selected_index = 0;
                    Some(text)
                } else {
                    None
                }
            }
            b'1'..=b'9' => {
                let idx = (key - b'1') as usize;
                if !self.buffer.is_empty() && idx < self.candidates.len() {
                    let text = self.candidates[idx].text.clone();
                    self.buffer.clear();
                    self.candidates.clear();
                    self.selected_index = 0;
                    Some(text)
                } else {
                    None
                }
            }
            b'0' => {
                if !self.buffer.is_empty() && self.candidates.len() > 9 {
                    let text = self.candidates[9].text.clone();
                    self.buffer.clear();
                    self.candidates.clear();
                    self.selected_index = 0;
                    Some(text)
                } else {
                    None
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' => {
                self.buffer.push((key as char).to_ascii_lowercase());
                self.update_candidates();
                None
            }
            _ => None,
        }
    }

    fn update_candidates(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.selected_index = 0;
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
        self.candidates = self.ranker.rank(&matched);
        self.selected_index = 0;
    }
}

// ========== fbterm 协议通信（仅 Linux） ==========

#[cfg(unix)]
fn send_message(stream: &mut UnixStream, msg_type: MsgType, payload: &[u8]) -> std::io::Result<()> {
    let header = MsgHeader {
        msg_type: msg_type as u32,
        length: payload.len() as u32,
    };
    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const MsgHeader as *const u8,
            std::mem::size_of::<MsgHeader>(),
        )
    };
    stream.write_all(header_bytes)?;
    if !payload.is_empty() {
        stream.write_all(payload)?;
    }
    stream.flush()
}

#[cfg(unix)]
fn recv_message(stream: &mut UnixStream) -> std::io::Result<(MsgType, Vec<u8>)> {
    let mut header_bytes = [0u8; 8];
    stream.read_exact(&mut header_bytes)?;
    let msg_type_val = u32::from_ne_bytes([
        header_bytes[0],
        header_bytes[1],
        header_bytes[2],
        header_bytes[3],
    ]);
    let length = u32::from_ne_bytes([
        header_bytes[4],
        header_bytes[5],
        header_bytes[6],
        header_bytes[7],
    ]);
    let msg_type = match msg_type_val {
        1 => MsgType::Connect,
        2 => MsgType::Disconnect,
        3 => MsgType::Active,
        4 => MsgType::Deactive,
        5 => MsgType::SendKey,
        6 => MsgType::PutText,
        7 => MsgType::SetWins,
        8 => MsgType::AckWins,
        9 => MsgType::CursorPosition,
        10 => MsgType::FbTermInfo,
        11 => MsgType::HideUI,
        12 => MsgType::ShowUI,
        13 => MsgType::AckHideUI,
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "未知消息类型",
            ))
        }
    };
    const MAX_PAYLOAD: usize = 1024 * 1024; // 1MB
    if length as usize > MAX_PAYLOAD {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("payload 过大: {} bytes (最大 {} bytes)", length, MAX_PAYLOAD),
        ));
    }
    let mut payload = vec![0u8; length as usize];
    if !payload.is_empty() {
        stream.read_exact(&mut payload)?;
    }
    Ok((msg_type, payload))
}

#[cfg(unix)]
fn put_text(stream: &mut UnixStream, text: &str) -> std::io::Result<()> {
    let c_text = CString::new(text).unwrap_or_default();
    send_message(stream, MsgType::PutText, c_text.to_bytes_with_nul())
}

#[cfg(unix)]
fn set_wins(stream: &mut UnixStream, wins: &[ImWin]) -> std::io::Result<()> {
    let payload = unsafe {
        std::slice::from_raw_parts(wins.as_ptr() as *const u8, std::mem::size_of_val(wins))
    };
    send_message(stream, MsgType::SetWins, payload)
}

#[cfg(unix)]
fn run_ime(stream: &mut UnixStream, ime: &mut FbtermIme) -> std::io::Result<()> {
    send_message(stream, MsgType::Connect, &[1])?;

    loop {
        let (msg_type, payload) = recv_message(stream)?;
        match msg_type {
            MsgType::Disconnect => break,
            MsgType::Active => {
                ime.active = true;
            }
            MsgType::Deactive => {
                ime.active = false;
                ime.buffer.clear();
                ime.candidates.clear();
                let _ = set_wins(stream, &[]);
            }
            MsgType::SendKey => {
                if !ime.active {
                    continue;
                }
                if let Some(text) = ime.process_key(&payload) {
                    let _ = put_text(stream, &text);
                    let _ = set_wins(stream, &[]);
                } else {
                    let _ = set_wins(stream, &[]);
                }
            }
            MsgType::CursorPosition => {
                if payload.len() >= 8 {
                    ime.cursor_x =
                        u32::from_ne_bytes([payload[0], payload[1], payload[2], payload[3]]);
                    ime.cursor_y =
                        u32::from_ne_bytes([payload[4], payload[5], payload[6], payload[7]]);
                }
            }
            MsgType::FbTermInfo => {
                if payload.len() < std::mem::size_of::<FbTermInfoData>() {
                    continue;
                }
                // 使用 from_ne_bytes 逐字段解析，避免 packed struct 的未对齐访问 UB
                let info = FbTermInfoData {
                    term_width: u32::from_ne_bytes([payload[0], payload[1], payload[2], payload[3]]),
                    term_height: u32::from_ne_bytes([payload[4], payload[5], payload[6], payload[7]]),
                    rotation: u32::from_ne_bytes([payload[8], payload[9], payload[10], payload[11]]),
                };
                let w = info.term_width;
                let h = info.term_height;
                let r = info.rotation;
                eprintln!("[fbterm] {}x{} rotation={}", w, h, r);
            }
            MsgType::HideUI => {
                let _ = set_wins(stream, &[]);
                let _ = send_message(stream, MsgType::AckHideUI, &[]);
            }
            _ => {}
        }
    }
    Ok(())
}

// ========== 入口 ==========

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: wbw-ime-fbterm <词典路径>");
        eprintln!("示例: fbterm -i wbw-ime-fbterm");
        process::exit(1);
    }

    #[cfg(not(unix))]
    {
        eprintln!("wbw-ime-fbterm 仅支持 Linux (fbterm)");
        process::exit(1);
    }

    #[cfg(unix)]
    {
        let dict_path = &args[1];
        eprintln!("[fbterm] 加载词典: {}", dict_path);
        let mut ime = FbtermIme::new(dict_path);
        eprintln!("[fbterm] 等待 fbterm 连接...");

        let socket_path = std::env::var("FBTERM_IM_SOCKET")
            .unwrap_or_else(|_| format!("/tmp/wbw-ime-{}.sock", std::process::id()));

        let _ = std::fs::remove_file(&socket_path);
        let listener =
            std::os::unix::net::UnixListener::bind(&socket_path).expect("无法创建 Unix socket");
        eprintln!("[fbterm] 监听: {}", socket_path);

        match listener.accept() {
            Ok((mut stream, _)) => {
                eprintln!("[fbterm] fbterm 已连接");
                if let Err(e) = run_ime(&mut stream, &mut ime) {
                    eprintln!("[fbterm] 通信错误: {}", e);
                }
            }
            Err(e) => eprintln!("[fbterm] 接受连接失败: {}", e),
        }

        let _ = std::fs::remove_file(&socket_path);
    }
}
