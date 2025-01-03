mod vm;
mod cranelift;
mod llvm;

use std::{io::{Read, Write}, time::Duration};

use crate::ir::BrainfuckIR;

pub trait VMInterface {
    fn new(ir: Vec<BrainfuckIR>, input: Box<dyn Read>, output: Box<dyn Write>) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn run(&mut self) -> anyhow::Result<Duration>;
}

pub const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

pub struct IO {
    pub input: Box<dyn Read>,
    pub output: Box<dyn Write>,
}

#[no_mangle]
pub extern "C" fn bf_put(context: *mut IO, ch: u8) {
    unsafe {
        // get IO
        let ctx = &mut *context;
        // write to output
        ctx.output.write_all(&[ch]).unwrap();
    }
}

#[no_mangle]
pub extern "C" fn bf_get(context: *mut IO) -> u8 {
    unsafe {
        // get IO
        let ctx = &mut *context;
        // read from input
        let mut buffer = [0u8; 1];
        ctx.input.read(buffer.as_mut()).unwrap();
        buffer[0]
    }
}

pub use vm::VM;
pub use cranelift::VMCranelift;
pub use llvm::LLVM;
