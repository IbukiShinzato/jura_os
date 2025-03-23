use core::fmt::{self, Write};

use lazy_static::lazy_static;
use spin::Mutex;
use volatile::Volatile;
use x86_64::instructions::interrupts;

// use crate::{exit_qemu, QemuExitCode};

#[allow(dead_code)]
// 出力と比較を可能にする
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 8bitで格納
#[repr(u8)]
// colorを文字として扱う
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    pub row_position: usize,
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            // 改行なら何もしない
            b'\n' => self.new_line(),
            byte => {
                // 現在の行がいっぱいかの確認
                // いっぱいだったら改行
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = self.row_position;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        };
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 出力可能
                0x20..=0x7e | b'\n' => self.write_byte(byte),

                // 出力不可能
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn new_line(&mut self) {
        // 出力を一番上の行に持ってくる
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                // そこにある文字を読み込む
                let character = self.buffer.chars[row][col].read();
                // 一個上の行に書き込む
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        // 何もない文字を作成
        // 背景はselfのを引き継ぐ
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            // 元々の行を全て空白に書き換える
            self.buffer.chars[row][col].write(blank);
        }
    }

    #[allow(dead_code)]
    pub fn clear_word(&mut self) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };

        if self.column_position == 0 {
            self.column_position = BUFFER_WIDTH - 1;
            self.row_position -= 1;
        } else {
            self.column_position -= 1;
        }

        let row = self.row_position;
        let col = self.column_position;

        self.buffer.chars[row][col].write(blank);
    }
}

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    // spin::Mutexで内部可変性を追加
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        row_position: BUFFER_HEIGHT - 1,
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        // 0xb8000はVGAテキストモードのバッファが配置されている物理アドレス
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    // Mutexがロックされている間は割り込みが発生しないことを保証する
    interrupts::without_interrupts(|| {
        // Writeトレイトの関数write_fmt
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        ($crate::print!("{}\n",format_args!($($arg)*)));
    }
}

// testクレートは標準ライブラリに依存している
// no_std環境下ではtestは使えない！
// test_caseはさまざまな引数でのテストが可能
#[test_case]
fn test_println_sample() {
    println!("test_println_sample output!");
}

#[test_case]
fn test_println_many() {
    for i in 0..200 {
        println!("test_println_many output: {}", i);
    }
}

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");

        for (i, c) in s.chars().enumerate() {
            // println!で出力された文字列は下から2行目の位置にある
            // ただし、改行された文字には対応できていない

            // // WRITER.lock()だと待機中でscreen_charを取得することができない
            // let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
