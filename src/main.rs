mod ir;
mod vm;

use std::{
    io::{stdin, stdout},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use ir::brainfuck_parser::compile_peg;
use vm::{VMInterface, VM, VMCranelift, LLVM};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(name = "FILE")]
    source_file: PathBuf,
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Jit {
        #[clap(short, long, value_enum)]
        method: JitMethod,
        #[clap(long, default_value_t = false)]
        dump_ir: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum JitMethod {
    Cranelift,
    LLVM,
}

fn main() -> anyhow::Result<()> {
    let opt = Cli::parse();

    let src = std::fs::read_to_string(&opt.source_file)?;
    let ir = compile_peg(src.as_str())?;

    let duration = match opt.command {
        Some(Commands::Jit {dump_ir, method}) => {
            match method {
                JitMethod::Cranelift => {
                    println!("Running program with {:?} JIT:", JitMethod::Cranelift);
                    let mut vm = VMCranelift::new(
                        ir,
                        Box::new(stdin().lock()),
                        Box::new(stdout().lock()),
                    )?;

                    vm.compile()?;

                    if dump_ir {
                        println!("{}", vm.get_ir());
                    }

                    vm.run()?
                }
                JitMethod::LLVM => {
                    println!("Running program with {:?} JIT:", JitMethod::LLVM);
                    use inkwell::context::Context;
                    let context = Context::create();
                    let mut vm = LLVM::new(
                        ir,
                        Box::new(stdin().lock()),
                        Box::new(stdout().lock()),
                    )?;

                    vm.compile(&context)?;

                    if dump_ir {
                        println!("{}", vm.get_ir()?);
                    }

                    vm.run()?
                }
            }
        }
        _ => {
            println!("Running program without JIT:");
            let mut vm = VM::new(
                ir,
                Box::new(stdin().lock()),
                Box::new(stdout().lock()),
            )?;

            vm.run()?
        }
    };

    println!("The code took: {:?} to run", duration);
    Ok(())
}
