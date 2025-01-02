mod vm;
mod cranelift;

use std::{io::{Read, Write}};

use crate::ir::BrainfuckIR;

pub trait VMInterface<'io> {
    fn new(ir: Vec<BrainfuckIR>, input: Box<dyn Read + 'io>, output: Box<dyn Write + 'io>) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn run(&mut self) -> anyhow::Result<()>;
}

pub const MEMORY_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

pub use vm::VM;
pub use cranelift::VMCranelift;
