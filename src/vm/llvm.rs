use std::io::{Read, Write};
use thiserror::Error;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use inkwell::module::Module;
use inkwell::{AddressSpace, OptimizationLevel};
use inkwell::values::PointerValue;
use crate::ir::BrainfuckIR;
use crate::vm::{VMInterface, IO, MEMORY_SIZE, bf_put, bf_get};

type JITFunc = unsafe extern "C" fn(*mut u8, *mut IO) -> u64;

#[derive(Error, Debug)]
enum LLVMError {
    #[error("LLVM could not get param nth {0}")]
    CouldNotGetParam(usize),
    #[error("LLVM error: run without compile (please compile first)")]
    RunWithoutCompile,
    #[error("LLVM could not create engine: {0}")]
    CouldNotCreateEngine(String),
    #[error("LLVM error: could not create context")]
    CouldNotCreateContext,
    #[error("LLVM error: get a None block")]
    GetNoneBlock,
    #[error("LLVM error: get a None function")]
    GetNoneFunction,
    #[error("LLVM error: function {0} not imported")]
    FunctionNotImported(String),
    #[error("LLVM io error: {0}")]
    IOError(String),
}

struct JITContext<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    execution_engine: ExecutionEngine<'ctx>,
    jit_func: Option<JitFunction<'ctx, JITFunc>>,
    ir: String,
}

impl<'ctx> JITContext<'ctx> {
    fn new(context: &'ctx Context) -> anyhow::Result<Self> {
        let module = context.create_module("bf-jit-module");
        let execution_engine = module
            .create_jit_execution_engine(OptimizationLevel::Aggressive)
            .map_err(|err| LLVMError::CouldNotCreateEngine(err.to_string()))?;

        Ok(Self {
            context: &context,
            module,
            builder: context.create_builder(),
            execution_engine,
            jit_func: None,
            ir: String::new(),
        })
    }

    fn compile(&mut self, ir: &[BrainfuckIR]) -> anyhow::Result<()> {
        let i64_type = self.context.i64_type();
        let i8_type = self.context.i8_type();
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        // fn(memory: *mut u8, io: *mut IOContext) -> i64
        let fn_type = i64_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);

        let function = self.module.add_function("bf_jit_main", fn_type, None);
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);

        let put_fn_type = self.context
            .void_type()
            .fn_type(&[ptr_type.into(), i8_type.into()], false);
        let bf_put_val = self.module.add_function("bf_put", put_fn_type, None);
        let get_fn_type = i8_type.fn_type(&[ptr_type.into()], false);
        let bf_get_val = self.module.add_function("bf_get", get_fn_type, None);

        self.execution_engine.add_global_mapping(
            &bf_put_val,
            bf_put as usize,
        );
        self.execution_engine.add_global_mapping(
            &bf_get_val,
            bf_get as usize,
        );

        let memory_ptr = function
            .get_nth_param(0)
            .ok_or_else(|| LLVMError::CouldNotGetParam(0))?
            .into_pointer_value();
        let io_ptr = function
            .get_nth_param(1)
            .ok_or_else(|| LLVMError::CouldNotGetParam(1))?
            .into_pointer_value();

        let memory = self.builder
            .build_alloca(ptr_type, "mem_ptr")?;
        self.builder.build_store(memory, memory_ptr)?;
        let io = self.builder
            .build_alloca(ptr_type, "io_ptr")?;
        self.builder.build_store(io, io_ptr)?;

        for inst in ir {
            self.compile_instruction(inst, &memory, &io)?;
        }

        let zero = i64_type.const_zero();
        self.builder.build_return(Some(&zero))?;

        self.jit_func = unsafe {
            self.execution_engine
                .get_function("bf_jit_main").ok()
        };

        self.ir = self.module.print_to_string().to_string();

        Ok(())
    }

    fn compile_instruction(&self, ir: &BrainfuckIR, ptr: &PointerValue, io: &PointerValue) -> anyhow::Result<()> {
        let ptr_type = self.context.ptr_type(AddressSpace::default());
        let i8_type = self.context.i8_type();
        match ir {
            BrainfuckIR::AddVal(n) => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let current_val = self.builder
                    .build_load(i8_type, current_ptr, "mem_val")?
                    .into_int_value();
                let new_val = self.builder
                    .build_int_add(current_val, self.context.
                        i8_type().const_int(*n as u64, false), "new_val")?;
                self.builder.build_store(current_ptr, new_val)?;
            }
            BrainfuckIR::SubVal(n) => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let current_val = self.builder
                    .build_load(i8_type, current_ptr, "mem_val")?
                    .into_int_value();
                let new_val = self.builder
                    .build_int_sub(current_val, self.context.
                        i8_type().const_int(*n as u64, false), "new_val")?;
                self.builder.build_store(current_ptr, new_val)?;
            }
            BrainfuckIR::PtrMovRight(n) => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let new_ptr = unsafe {
                    self.builder.build_gep(ptr_type, current_ptr, &[self.context
                        .i64_type().const_int(*n as u64, false)], "new_ptr")?
                };
                self.builder.build_store(*ptr, new_ptr)?;
            }
            BrainfuckIR::PtrMovLeft(n) => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let new_ptr = unsafe {
                    self.builder.build_gep(ptr_type, current_ptr, &[self.context
                        .i64_type()
                        .const_int((*n as i64).wrapping_neg() as u64, false)], "new_ptr")?
                };
                self.builder.build_store(*ptr, new_ptr)?;
            }
            BrainfuckIR::PutByte => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let current_val = self.builder
                    .build_load(i8_type, current_ptr, "mem_val")?
                    .into_int_value();

                let io = self.builder
                    .build_load(ptr_type, *io, "io_ptr")?
                    .into_pointer_value();

                let put_fn = self.module
                    .get_function("bf_put")
                    .ok_or_else(|| LLVMError::FunctionNotImported("bf_put".to_string()))?;
                self.builder
                    .build_call(
                        put_fn,
                        &[io.into(), current_val.into()],
                        "call_put"
                    )?;
            }
            BrainfuckIR::GetByte => {
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();

                let io = self.builder
                    .build_load(ptr_type, *io, "io_ptr")?
                    .into_pointer_value();

                let get_fn = self.module
                    .get_function("bf_get")
                    .ok_or_else(|| LLVMError::FunctionNotImported("bf_get".to_string()))?;
                let byte_read = self.builder
                    .build_call(
                        get_fn,
                        &[io.into()],
                        "call_get"
                    )?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| LLVMError::IOError(String::from("Could not read byte")))?
                    .into_int_value();

                self.builder.build_store(current_ptr, byte_read)?;
            }
            BrainfuckIR::Loop(body) => {
                let function = self.builder
                    .get_insert_block()
                    .ok_or_else(|| LLVMError::GetNoneBlock)?
                    .get_parent()
                    .ok_or_else(|| LLVMError::GetNoneFunction)?;
                let loop_check = self.context.append_basic_block(function, "loop_check");
                let loop_body = self.context.append_basic_block(function, "loop_body");
                let loop_end = self.context.append_basic_block(function, "loop_end");

                self.builder.build_unconditional_branch(loop_check)?;

                self.builder.position_at_end(loop_check);
                let current_ptr = self.builder
                    .build_load(ptr_type, *ptr, "mem_ptr")?
                    .into_pointer_value();
                let current_val = self.builder
                    .build_load(i8_type, current_ptr, "mem_val")?
                    .into_int_value();
                let is_zero = self.builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        current_val,
                        self.context.i8_type().const_zero(),
                        "is_zero",
                    )?;
                self.builder.build_conditional_branch(is_zero, loop_end, loop_body)?;

                self.builder.position_at_end(loop_body);
                for inst in body {
                    self.compile_instruction(inst, ptr, io)?;
                }
                self.builder.build_unconditional_branch(loop_check)?;

                self.builder.position_at_end(loop_end);
            }
        }
        Ok(())
    }
}

pub struct LLVM<'ctx> {
    ir: Vec<BrainfuckIR>,
    memory: Vec<u8>,
    io: IO,
    jit_context: Option<JITContext<'ctx>>,
}

impl VMInterface for LLVM<'_> {
    fn new(ir: Vec<BrainfuckIR>, input: Box<dyn Read>, output: Box<dyn Write>) -> anyhow::Result<Self>
    where
        Self: Sized
    {
        Ok(Self {
            ir,
            jit_context: None,
            memory: vec![0; MEMORY_SIZE],
            io: IO {
                input,
                output,
            },
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        let func = self.jit_context.as_ref().ok_or_else(|| LLVMError::RunWithoutCompile)?
            .jit_func.as_ref().ok_or_else(|| LLVMError::RunWithoutCompile)?;

        let _ = unsafe { func.call(self.memory.as_mut_ptr(), &mut self.io) };
        Ok(())
    }
}

impl<'ctx> LLVM<'ctx> {
    pub fn compile(&mut self, context: &'ctx Context) -> anyhow::Result<()> {
        self.jit_context = Some(JITContext::new(context)?);
        self.jit_context
            .as_mut()
            .ok_or_else(|| LLVMError::CouldNotCreateContext)?
            .compile(&self.ir)?;

        Ok(())
    }

    pub fn get_ir(&self) -> anyhow::Result<String> {
        Ok(self.jit_context.as_ref().ok_or_else(|| LLVMError::RunWithoutCompile)?.ir.clone())
    }
}
