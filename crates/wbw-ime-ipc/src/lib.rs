//! wbwIME 候选窗口 IPC 协议
//!
//! TSF DLL（被注入到目标进程，作为客户端）与独立候选窗口进程
//! `wbw-ime-gui`（作为服务端）之间通过 **localhost TCP** 通信。
//!
//! 帧格式：`[4 字节小端长度][JSON 字节]`。每条消息独立成帧。
//!
//! - DNSB 侧（`ks_key_down` 后）发出 [`ToGui::Show`] 带候选与光标屏幕坐标、
//!   [`ToGui::Hide`]；收到 GUI 点击回传的 [`ToDll`] 后做选词/上屏。
//! - GUI 侧监听端口，收到 [`ToGui`] 更新窗口状态并定位到光标坐标；
//!   鼠标点击候选/翻页图标回传 [`ToDll`]。

use serde::{Deserialize, Serialize};

/// 服务端监听端口（GUI 进程监听，DLL 连接）。
pub const PORT: u16 = 45123;
/// 连接/读写超时（毫秒）。
pub const TIMEOUT_MS: u64 = 3000;

/// DNSB → GUI。`x`/`y` 为光标屏幕坐标（物理像素），供窗口定位。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToGui {
    Show {
        buffer: String,
        candidates: Vec<String>,
        selected: usize,
        page: usize,
        total_pages: usize,
        x: i32,
        y: i32,
    },
    Hide,
}

/// GUI → DNSB。点击候选（页内索引）或翻页。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToDll {
    Select(usize),
    PageUp,
    PageDown,
}

/// 4 字节小端长度前缀 + JSON 载荷的读写。
pub mod frame {
    use super::*;
    use std::io::{self, Read, Write};

    /// 从 `reader` 读取一帧并反序列化为 `T`。连接关闭且无数据时返回 `Ok(None)`。
    pub fn read<T: for<'de> Deserialize<'de>>(reader: &mut impl Read) -> io::Result<Option<T>> {
        let mut len_buf = [0u8; 4];
        if !read_exact_opt(reader, &mut len_buf)? {
            return Ok(None);
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        if len > 64 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame too large: {len}"),
            ));
        }
        let mut buf = vec![0u8; len];
        read_exact_opt(reader, &mut buf)?;
        let value = serde_json::from_slice(&buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Some(value))
    }

    /// 将 `value` 序列化并写入 `writer` 一帧。
    pub fn write<T: Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
        let buf = serde_json::to_vec(value)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let len = (buf.len() as u32).to_le_bytes();
        writer.write_all(&len)?;
        writer.write_all(&buf)?;
        writer.flush()
    }

    fn read_exact_opt(reader: &mut impl Read, buf: &mut [u8]) -> io::Result<bool> {
        let mut read = 0usize;
        while read < buf.len() {
            match reader.read(&mut buf[read..]) {
                Ok(0) => {
                    if read == 0 {
                        return Ok(false);
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "unexpected eof mid-frame",
                    ));
                }
                Ok(n) => read += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_to_gui_show() {
        let msg = ToGui::Show {
            buffer: "wo".into(),
            candidates: vec!["我".into(), "喔".into()],
            selected: 0,
            page: 0,
            total_pages: 1,
            x: 120,
            y: 45,
        };
        let mut buf: Vec<u8> = Vec::new();
        frame::write(&mut buf, &msg).unwrap();
        let mut cur = std::io::Cursor::new(buf);
        let back: ToGui = frame::read(&mut cur).unwrap().unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn roundtrip_to_dll() {
        for msg in [ToDll::Select(2), ToDll::PageUp, ToDll::PageDown] {
            let mut buf: Vec<u8> = Vec::new();
            frame::write(&mut buf, &msg).unwrap();
            let mut cur = std::io::Cursor::new(buf);
            let back: ToDll = frame::read(&mut cur).unwrap().unwrap();
            assert_eq!(back, msg);
        }
    }

    #[test]
    fn eof_returns_none_on_empty() {
        let mut cur = std::io::Cursor::new(Vec::<u8>::new());
        let back: Option<ToDll> = frame::read(&mut cur).unwrap();
        assert!(back.is_none());
    }

    /// localhost 双向全双工冒烟：一端当 DLL（发 Show、收 Select），
    /// 另一端当 GUI（收 Show、回发 Select），验证 TCP 层帧读写 + flush。
    #[test]
    fn tcp_bi_directional() {
        use std::io::{BufReader, BufWriter};
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = BufWriter::new(stream);
            // 读到一个 Show，回一个 Select
            let got: ToGui = frame::read(&mut reader).unwrap().expect("show");
            match &got {
                ToGui::Show { buffer, .. } => assert_eq!(buffer, "hi"),
                ToGui::Hide => panic!("expected show"),
            }
            frame::write(&mut writer, &ToDll::Select(1)).unwrap();
        });

        let client = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
            frame::write(
                &mut stream,
                &ToGui::Show {
                    buffer: "hi".into(),
                    candidates: vec!["hi".into()],
                    selected: 0,
                    page: 0,
                    total_pages: 1,
                    x: 1,
                    y: 2,
                },
            )
            .unwrap();
            let reply: Option<ToDll> = frame::read(&mut stream).unwrap();
            assert_eq!(reply, Some(ToDll::Select(1)));
        });

        server.join().unwrap();
        client.join().unwrap();
    }
}
