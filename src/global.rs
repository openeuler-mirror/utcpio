/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::sync::Mutex;

use lazy_static::lazy_static;

// If true, reset access times after reading files (-a).
// pub const RESET_TIME_FLAG: bool = false;
// pub const NAME_END: char  = '\n';

pub struct TapeOutput {
    pub output_buffer: Vec<u8>,
    pub output_size: usize,
    pub output_bytes: usize,
    pub out_buff: usize,
    pub output_is_special: bool,
    pub output_is_seekable: bool,
}

pub struct TapeInput {
    pub input_buffer: Vec<u8>,
    pub input_size: usize,
    pub input_bytes: usize,
    pub in_buff: usize,
    pub input_is_special: bool,
    pub input_is_seekable: bool,
    pub input_buffer_size: usize,
}

impl TapeOutput {
    pub fn new(size: usize) -> TapeOutput {
        TapeOutput {
            output_buffer: vec![0; size],
            output_size: 0,
            output_bytes: 0,
            out_buff: 0, //当前的读写位置
            output_is_special: false,
            output_is_seekable: false,
        }
    }
    pub fn resize(&mut self, new_size: usize) {
        self.output_buffer.resize(new_size, 0);
    }
    #[allow(dead_code)]
    pub fn print(&mut self) {
        println!("out_size: {}, output_byes: {}, out_buff: {} output_buffer_len: {} output_is_special:{}, output_is_seekable:{}", self.output_size, self.output_bytes, self.out_buff,self.output_buffer.len(), self.output_is_special, self.output_is_seekable);
    }
}

impl TapeInput {
    pub fn new(size: usize) -> TapeInput {
        TapeInput {
            input_buffer: vec![0; size],
            input_size: 0,
            input_bytes: 0,
            in_buff: 0, // 当前的读写位置
            input_is_special: false,
            input_is_seekable: false,
            input_buffer_size: size,
        }
    }
    pub fn resize(&mut self, new_size: usize) {
        self.input_buffer.resize(new_size, 0);
        self.input_buffer_size = new_size;
    }

    pub fn free(&mut self) {
        self.input_buffer.clear();
        self.input_size = 0;
        self.input_bytes = 0;
        self.in_buff = 0;
        self.input_is_special = false;
        self.input_is_seekable = false;
        self.input_buffer_size = 0;
    }
    #[allow(dead_code)]
    pub fn test(&mut self, index: usize) {
        println!(
            "TapeInput test :input_buffer[{}]={} ",
            index, self.input_buffer[index]
        );
    }
    #[allow(dead_code)]
    pub fn print(&mut self) {
        println!("input_size: {}, input_byes: {}, in_buff: {},input_buffer_size:{}, input_buffer length:{}", self.input_size, self.input_bytes, self.in_buff, self.input_buffer_size, self.input_buffer.len());
    }
}
lazy_static! {
    pub static ref TAPE_OUTPUT: Mutex<TapeOutput> = Mutex::new(TapeOutput::new(1024));
    pub static ref TAPE_INPUT: Mutex<TapeInput> = Mutex::new(TapeInput::new(1024));
}

pub fn resize_input_buffer(new_size: usize) {
    let mut tape_input = TAPE_INPUT.lock().unwrap(); // 获取锁
    tape_input.resize(new_size); // 修改 TapeInput
}

pub fn resize_output_buffer(new_size: usize) {
    let mut tape_out = TAPE_OUTPUT.lock().unwrap(); // 获取锁
    tape_out.resize(new_size); // 修改 TapeInput
}

pub fn major(device: u32) -> u8 {
    ((device >> 8) & 0xff) as u8
}

pub fn minor(device: u32) -> u8 {
    (device & 0xff) as u8
}

pub fn makedev(major: u8, minor: u8) -> u32 {
    ((major as u32) << 8) | (minor as u32)
}
