use alloc::string::String;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use futures_util::{stream::Stream, StreamExt};
use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use spin::Mutex;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();
static PROMPT: &str = "?e235718?jura_os ";

use crate::{exit_qemu, print, println, QemuExitCode};

lazy_static! {
    static ref COMMANDS: Mutex<Vec<char>> = Mutex::new(Vec::new());
    static ref CLEAR_FRAG: Mutex<bool> = Mutex::new(false);
}

// lib.rsからのみ利用可能
#[allow(dead_code)]
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            println!("WARNING: scancode queue full; dropping keyboard input.");
        } else {
            // scancodeへのpushが成功
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized.");
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<u8>> {
        // scancodeの参照を取得
        let queue = SCANCODE_QUEUE.try_get().expect("not intialized");
        if let Ok(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Ok(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

pub async fn print_keypress() {
    // debug
    print!("{}", PROMPT);

    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                parse_keypress(key);
            }
        }
    }
}

fn parse_keypress(key: DecodedKey) {
    let mut commands = COMMANDS.lock();
    let mut clear_flag = CLEAR_FRAG.lock();

    match key {
        DecodedKey::Unicode(character) => match character {
            '\n' => {
                let msg: String = commands.iter().collect();
                print!("\n{}\n\n{}", msg, PROMPT);

                commands.clear();
            }
            'l' => {
                if *clear_flag {
                    for _ in 0..25 {
                        println!();
                    }
                    let msg: String = commands.iter().collect();
                    print!("{}{}", PROMPT, msg);
                } else {
                    commands.push(character);
                    print!("{}", character);
                }
                *clear_flag = false;
            }
            'c' => {
                // qemuを終了
                if *clear_flag {
                    exit_qemu(QemuExitCode::Success);
                }
            }
            _ => {
                commands.push(character);
                print!("{}", character);
                *clear_flag = false;
            }
        },
        DecodedKey::RawKey(key) => match key {
            KeyCode::LShift | KeyCode::RShift | KeyCode::RControl => {}
            KeyCode::LControl => *clear_flag = true,
            _ => print!("{:?}", key),
        },
    }
}
