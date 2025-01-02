#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BrainfuckIR {
    AddVal(u8),             // +
    SubVal(u8),             // -
    PtrMovRight(u32),       // >
    PtrMovLeft(u32),        // <
    PutByte,                // .
    GetByte,                // ,
    Loop(Vec<BrainfuckIR>), // [ loop_block ]
}

peg::parser!(pub grammar brainfuck_parser() for str {
    pub rule compile_peg() -> Vec<BrainfuckIR>
        = skip()* inst:instruction_with_skip()* skip()* { inst }

    rule instruction_with_skip() -> BrainfuckIR
        = skip()* inst:instruction() skip()* { inst }

    rule instruction() -> BrainfuckIR
        = add_val()
        / sub_val()
        / ptr_right()
        / ptr_left()
        / put_byte()
        / get_byte()
        / r#loop()

    rule add_val() -> BrainfuckIR
        = n:"+"+ {
            BrainfuckIR::AddVal(n.len() as u8)
        }

    rule sub_val() -> BrainfuckIR
        = n:"-"+ {
            BrainfuckIR::SubVal(n.len() as u8)
        }

    rule ptr_right() -> BrainfuckIR
        = n:">"+ {
            BrainfuckIR::PtrMovRight(n.len() as u32)
        }

    rule ptr_left() -> BrainfuckIR
        = n:"<"+ {
            BrainfuckIR::PtrMovLeft(n.len() as u32)
        }

    rule put_byte() -> BrainfuckIR
        = "." {
            BrainfuckIR::PutByte
        }

    rule get_byte() -> BrainfuckIR
        = "," {
            BrainfuckIR::GetByte
        }

    rule r#loop() -> BrainfuckIR
        = "[" loop_block:instruction_with_skip()* "]" {
            BrainfuckIR::Loop(loop_block)
        }

    rule skip()
        = [' ' | '\n' | '\t']
});
