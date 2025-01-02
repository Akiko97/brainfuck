use std::io::{Read, Write};
use cranelift::codegen::ir::FuncRef;
use cranelift::codegen::write_function;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::ir::BrainfuckIR;
use crate::vm::{VMInterface, MEMORY_SIZE};

struct JITContext<'io> {
    // cranelift jit
    module: JITModule,
    builder_ctx: FunctionBuilderContext,
    ctx: codegen::Context,
    ir: String,
    // context
    memory: Vec<u8>,
    input: Box<dyn Read + 'io>,
    output: Box<dyn Write + 'io>,
}

impl<'io> JITContext<'io> {
    fn new(input: Box<dyn Read + 'io>, output: Box<dyn Write + 'io>) -> anyhow::Result<Self> {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed_and_size")?;

        // isa
        let isa = cranelift_native::builder()
            .unwrap()
            .finish(settings::Flags::new(flag_builder))?;

        // create JITBuilder & register external symbol
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        builder.symbol("bf_put", bf_put as *const u8);
        builder.symbol("bf_get", bf_get as *const u8);

        // create JITModule
        let module = JITModule::new(builder);

        Ok(Self {
            module,
            builder_ctx: FunctionBuilderContext::new(),
            ctx: codegen::Context::new(),
            ir: String::new(),
            memory: vec![0; MEMORY_SIZE],
            input,
            output,
        })
    }

    fn compile_brainfuck_ir(&mut self, ir: &[BrainfuckIR]) -> anyhow::Result<FuncId> {
        // clean ctx
        self.ctx.clear();

        // declare function signature
        {
            let sig = &mut self.ctx.func.signature;
            sig.params.push(AbiParam::new(types::I64)); // memory_ptr
            sig.params.push(AbiParam::new(types::I64)); // context_ptr
        }

        // register function
        let func_id = self.module.declare_function(
            "bf_jit_main",
            Linkage::Local,
            &self.ctx.func.signature,
        )?;

        // build the function body
        {
            let mut func_ctx = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);

            // register import func: bf_put(*mut JITContext, u8)
            let mut put_sig = self.module.make_signature();
            put_sig.params.push(AbiParam::new(types::I64));
            put_sig.params.push(AbiParam::new(types::I32));
            let put_func_id = self.module.declare_function(
                "bf_put",
                Linkage::Import,
                &put_sig
            )?;
            let put_func_ref = self.module.declare_func_in_func(put_func_id, &mut func_ctx.func);

            // register import func: bf_get(*mut JITContext) -> u8
            let mut get_sig = self.module.make_signature();
            get_sig.params.push(AbiParam::new(types::I64));
            get_sig.returns.push(AbiParam::new(types::I32));
            let get_sig_id = self.module.declare_function(
                "bf_get",
                Linkage::Import,
                &get_sig
            )?;
            let get_sig_ref = self.module.declare_func_in_func(get_sig_id, &mut func_ctx.func);

            // create entry block
            let entry_block = func_ctx.create_block();
            func_ctx.append_block_params_for_function_params(entry_block);

            // switch to entry block & seal it
            func_ctx.switch_to_block(entry_block);
            func_ctx.seal_block(entry_block);

            // get the parameters passed in
            let memory_ptr = func_ctx.block_params(entry_block)[0];
            let context_ptr = func_ctx.block_params(entry_block)[1];

            // declare a variable representing memory offset & init with 0
            let pointer_var = Variable::from_u32(0);
            func_ctx.declare_var(pointer_var, types::I32);
            {
                let zero = func_ctx.ins().iconst(types::I32, 0);
                func_ctx.def_var(pointer_var, zero);
            }

            // generate cranelift ir
            codegen_bf_block(&mut func_ctx, &memory_ptr, &pointer_var, ir, &put_func_ref, &get_sig_ref, &context_ptr)?;

            // return void
            func_ctx.ins().return_(&[]);
            func_ctx.finalize();
        }

        // save cranelift ir
        write_function(&mut self.ir, &self.ctx.func)?;

        // compile function & refine
        self.module.define_function(func_id, &mut self.ctx)?;
        self.module.clear_context(&mut self.ctx);

        // allocate and commit executable memory
        self.module.finalize_definitions()?;

        Ok(func_id)
    }
}

fn codegen_bf_block(
    func_ctx: &mut FunctionBuilder,
    memory_ptr: &Value,
    pointer_var: &Variable,
    ir_block: &[BrainfuckIR],
    put_func_ref: &FuncRef,
    get_func_ref: &FuncRef,
    context_ptr: &Value,
) -> anyhow::Result<()> {
    for inst in ir_block {
        match inst {
            BrainfuckIR::AddVal(n) => {
                // get memory offset
                let offset_i32 = func_ctx.use_var(*pointer_var);
                let offset_i64 = func_ctx.ins().uextend(types::I64, offset_i32);
                let mem = func_ctx.ins().iadd(*memory_ptr, offset_i64);

                // load a byte from memory
                let mem_flags = MemFlags::new();
                let old_val = func_ctx.ins().load(types::I8, mem_flags, mem, 0);
                let old_val32 = func_ctx.ins().uextend(types::I32, old_val);

                // add the immediate value n
                let new_val32 = func_ctx.ins().iadd_imm(old_val32, i64::from(*n));

                // store new value to memory
                let new_val8 = func_ctx.ins().ireduce(types::I8, new_val32);
                func_ctx.ins().store(mem_flags, new_val8, mem, 0);
            }

            BrainfuckIR::SubVal(n) => {
                // get memory offset
                let offset_i32 = func_ctx.use_var(*pointer_var);
                let offset_i64 = func_ctx.ins().uextend(types::I64, offset_i32);
                let mem = func_ctx.ins().iadd(*memory_ptr, offset_i64);

                // load a byte from memory
                let mem_flags = MemFlags::new();
                let old_val = func_ctx.ins().load(types::I8, mem_flags, mem, 0);
                let old_val32 = func_ctx.ins().uextend(types::I32, old_val);

                // subtract the immediate value n
                let new_val32 = func_ctx.ins().iadd_imm(old_val32, -i64::from(*n));

                // store new value to memory
                let new_val8 = func_ctx.ins().ireduce(types::I8, new_val32);
                func_ctx.ins().store(mem_flags, new_val8, mem, 0);
            }

            BrainfuckIR::PtrMovRight(n) => {
                // memory offset += n
                let old_ptr = func_ctx.use_var(*pointer_var);
                let new_ptr = func_ctx.ins().iadd_imm(old_ptr, i64::from(*n));
                func_ctx.def_var(*pointer_var, new_ptr);
            }

            BrainfuckIR::PtrMovLeft(n) => {
                // memory offset -= n
                let old_ptr = func_ctx.use_var(*pointer_var);
                let new_ptr = func_ctx.ins().iadd_imm(old_ptr, -i64::from(*n));
                func_ctx.def_var(*pointer_var, new_ptr);
            }

            BrainfuckIR::PutByte => {
                // load a byte from memory
                let offset_i32 = func_ctx.use_var(*pointer_var);
                let offset_i64 = func_ctx.ins().uextend(types::I64, offset_i32);
                let mem = func_ctx.ins().iadd(*memory_ptr, offset_i64);
                let val_i8 = func_ctx.ins().load(types::I8, MemFlags::new(), mem, 0);
                let val_i32 = func_ctx.ins().uextend(types::I32, val_i8);

                // call bf_put
                let call = func_ctx.ins().call(*put_func_ref, &[*context_ptr, val_i32]);
                let _ = func_ctx.inst_results(call);
            }

            BrainfuckIR::GetByte => {
                // call bf_get
                let call = func_ctx.ins().call(*get_func_ref, &[*context_ptr]);
                let results = func_ctx.inst_results(call);
                let val_i32 = results[0];

                // store to memory
                let val_i8 = func_ctx.ins().ireduce(types::I8, val_i32);
                let offset_i32 = func_ctx.use_var(*pointer_var);
                let offset_i64 = func_ctx.ins().uextend(types::I64, offset_i32);
                let mem = func_ctx.ins().iadd(*memory_ptr, offset_i64);
                func_ctx.ins().store(MemFlags::new(), val_i8, mem, 0);
            }

            BrainfuckIR::Loop(loop_ir) => {
                // create blocks
                let loop_head = func_ctx.create_block(); // judgment logic for loop
                let loop_body = func_ctx.create_block(); // loop body
                let loop_end  = func_ctx.create_block(); // loop end

                // jump to loop_head
                func_ctx.ins().jump(loop_head, &[]);

                // switch to loop_head
                func_ctx.switch_to_block(loop_head);

                // load a value from memory
                let offset_i32 = func_ctx.use_var(*pointer_var);
                let offset_i64 = func_ctx.ins().uextend(types::I64, offset_i32);
                let mem = func_ctx.ins().iadd(*memory_ptr, offset_i64);
                let val_i8 = func_ctx.ins().load(types::I8, MemFlags::new(), mem, 0);
                let val_i32 = func_ctx.ins().uextend(types::I32, val_i8);

                // brif: if value != 0 { loop_body } else { loop_end }
                func_ctx.ins().brif(val_i32, loop_body, &[], loop_end, &[]);

                // switch to loop_body
                func_ctx.switch_to_block(loop_body);

                // generate loop body instructions recursively
                codegen_bf_block(func_ctx, memory_ptr, pointer_var, loop_ir, put_func_ref, get_func_ref, context_ptr)?;
                // at the end of loop: jump back to loop_head
                func_ctx.ins().jump(loop_head, &[]);

                // switch to loop_end
                func_ctx.switch_to_block(loop_end);

                // seal all blocks
                func_ctx.seal_block(loop_head);
                func_ctx.seal_block(loop_body);
                func_ctx.seal_block(loop_end);
            }
        }
    }

    Ok(())
}

pub struct VMCranelift<'io> {
    ir: Vec<BrainfuckIR>,
    context: JITContext<'io>,
    func: *const u8,
}

impl<'io> VMInterface<'io> for VMCranelift<'io> {
    fn new(
        ir: Vec<BrainfuckIR>,
        input: Box<dyn Read + 'io>,
        output: Box<dyn Write + 'io>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            ir,
            context: JITContext::new(input, output)?,
            func: std::ptr::null(),
        })
    }

    fn run(&mut self) -> anyhow::Result<()> {
        // create func: fn(mem: *mut u8, ctx: *mut JITContext<'io>) -> ()
        let func = unsafe { std::mem::transmute::<_, fn(*mut u8, *mut JITContext<'io>)>(self.func) };

        // call func
        func(
            self.context.memory.as_mut_ptr(),
            &mut self.context,
        );

        Ok(())
    }
}

impl<'io> VMCranelift<'io> {
    pub fn compile(&mut self) -> anyhow::Result<()> {
        // compile
        let func_id = self.context.compile_brainfuck_ir(&self.ir)?;

        // get function pointer
        let code_ptr = self.context.module.get_finalized_function(func_id);

        self.func = code_ptr;
        Ok(())
    }

    pub fn get_ir(&self) -> String {
        self.context.ir.clone()
    }
}

#[no_mangle]
extern "C" fn bf_put(context: *mut JITContext, ch: u8) {
    unsafe {
        // get JITContext
        let ctx = &mut *context;
        // write to ctx.output
        ctx.output.write_all(&[ch]).unwrap();
    }
}

#[no_mangle]
extern "C" fn bf_get(context: *mut JITContext) -> u8 {
    unsafe {
        // get JITContext
        let ctx = &mut *context;
        // read from ctx.input
        let mut buffer = [0u8; 1];
        ctx.input.read(buffer.as_mut()).unwrap();
        buffer[0]
    }
}
