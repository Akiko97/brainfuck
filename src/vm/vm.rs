use std::io::{Read, Write};
use thiserror::Error;

use crate::ir::BrainfuckIR;
use crate::vm::{VMInterface, MEMORY_SIZE};

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("IO: {0}")]
    IO(#[from] std::io::Error),
    #[error("overflow")]
    Overflow,
}

struct VMContext<'io> {
    memory: Box<[u8]>,
    input: Box<dyn Read + 'io>,
    output: Box<dyn Write + 'io>,
}

pub struct VM<'io> {
    ir: Vec<BrainfuckIR>,
    context: VMContext<'io>,
}

impl<'io> VMInterface<'io> for VM<'io> {
    fn new(
        ir: Vec<BrainfuckIR>,
        input: Box<dyn Read + 'io>,
        output: Box<dyn Write + 'io>,
    ) -> anyhow::Result<Self> {
        let memory = vec![0; MEMORY_SIZE].into_boxed_slice();

        Ok(Self {
            ir,
            context: VMContext {
                memory,
                input,
                output,
            }
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        let mut ptr = 0;
        self.context.run_block(&self.ir, &mut ptr)?;
        Ok(())
    }
}

impl<'io> VMContext<'io> {
    fn run_block(&mut self, block: &[BrainfuckIR], ptr: &mut usize) -> anyhow::Result<()> {
        let mut pc = 0usize;
        while pc < block.len() {
            match &block[pc] {
                BrainfuckIR::AddVal(val) => self.memory[*ptr] = self.memory[*ptr].wrapping_add(*val),
                BrainfuckIR::SubVal(val) => self.memory[*ptr] = self.memory[*ptr].wrapping_sub(*val),
                BrainfuckIR::PtrMovRight(val) => {
                    let new_ptr = (*ptr as isize).wrapping_add(*val as isize);
                    if !(0..self.memory.len() as isize).contains(&new_ptr) {
                        return Err(RuntimeError::Overflow.into());
                    }
                    *ptr = new_ptr as usize;
                }
                BrainfuckIR::PtrMovLeft(val) => {
                    let new_ptr = (*ptr as isize).wrapping_sub(*val as isize);
                    if !(0..self.memory.len() as isize).contains(&new_ptr) {
                        return Err(RuntimeError::Overflow.into());
                    }
                    *ptr = new_ptr as usize;
                }
                BrainfuckIR::PutByte => {
                    self.output.write_all(&self.memory[*ptr..=*ptr])?;
                }
                BrainfuckIR::GetByte => {
                    let mut byte: [u8; 1] = [0; 1];
                    self.input.read_exact(&mut byte)?;
                    self.memory[*ptr] = byte[0];
                }
                BrainfuckIR::Loop(loop_block) => {
                    while self.memory[*ptr] != 0 {
                        self.run_block(loop_block, ptr)?;
                    }
                }
            }
            pc += 1;
        }
        Ok(())
    }
}
