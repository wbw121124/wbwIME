//! 候选窗口的 IPC 服务端（`--ipc` 模式）。
//!
//! 作为 localhost TCP 服务端监听，接收 TSF DLL 发来的 [`ToGui::Show` /
//! `ToGui::Hide`]，转发到 UI 线程；鼠标点击候选/翻页时把 [`ToDll`]
//! 写回当前连接，供 DLL 上屏。

use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use wbw_ime_ipc::{frame, ToDll, ToGui, PORT};

/// 当前到 DLL 的写端（通常只有一条连接）。
pub static DLL_WRITER: Mutex<Option<BufWriter<TcpStream>>> = Mutex::new(None);
/// 是否已有连接（UI 线程据此判断能否回传点击）。
pub static CONNECTED: AtomicBool = AtomicBool::new(false);

/// 启动监听线程。`tx` 用于把收到的 `ToGui` 交给 UI 线程。
/// 修改原因：先在当前线程同步 bind（立即发现端口占用），成功后才启动 accept 线程；返回 bool 表示 bind 是否成功。
pub fn spawn(rx: Sender<ToGui>) -> bool {
    let listener = match TcpListener::bind(("127.0.0.1", PORT)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[ipc] bind {PORT} failed: {e}");
            return false;
        }
    };
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_millis(
                        wbw_ime_ipc::TIMEOUT_MS * 4,
                    ))) {
                        eprintln!("[ipc] set timeout failed: {e}");
                    }
                    handle_connection(stream, rx.clone());
                }
                Err(e) => eprintln!("[ipc] accept failed: {e}"),
            }
        }
    });
    true
}

fn handle_connection(stream: TcpStream, rx: Sender<ToGui>) {
    let mut reader = BufReader::new(stream.try_clone().expect("try_clone"));
    let writer = BufWriter::new(stream);

    {
        let mut slot = DLL_WRITER.lock().unwrap();
        *slot = Some(writer);
    }
    CONNECTED.store(true, Ordering::SeqCst);

    // 循环读 DLL 发来的 ToGui
    loop {
        match frame::read::<ToGui>(&mut reader) {
            Ok(Some(msg)) => {
                if rx.send(msg).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("[ipc] read error: {e}");
                break;
            }
        }
    }

    CONNECTED.store(false, Ordering::SeqCst);
    DLL_WRITER.lock().unwrap().take();
}

/// 向 DLL 发送一条点击消息。无连接或失败时静默忽略。
pub fn send(msg: &ToDll) {
    if !CONNECTED.load(Ordering::SeqCst) {
        return;
    }
    let mut slot = DLL_WRITER.lock().unwrap();
    if let Some(writer) = slot.as_mut() {
        if let Err(e) = frame::write(writer, msg) {
            eprintln!("[ipc] send to dll failed: {e}");
            *slot = None;
            CONNECTED.store(false, Ordering::SeqCst);
        }
    }
}
