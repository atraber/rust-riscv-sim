mod memory;
use memory::*;
mod elf;
use elf::load as elf_load;

fn main() {
    let mut mem = SimpleRam::new(128*1024);
    let mut cpu = CpuRegisters::new(0x0);

    match elf_load("tmp/hello.elf", &mut mem) {
        Ok(data) => {
            println!("Loading ok!");
            cpu.pc = data
        },
        Err(err) => println!("Error {}", err),
    };

    for i in 0..100 {
        riscv_decode(&mut mem, &mut cpu);
    }
}

struct CpuRegisters {
    pc: u64,
    gpr: [u64; 32],
    fpr: [u64; 32], // TODO: currently unused
    spr: [u64; 4096],
}

impl CpuRegisters {
    fn new(boot_addr: u64) -> CpuRegisters {
        return CpuRegisters {
            pc: boot_addr,
            gpr: [0; 32],
            fpr: [0; 32],
            spr: [0; 4096],
        };
    }
}

enum RiscvOpcode {
    Load      = 0b0_00_000_11,
    Store     = 0b0_01_000_11,
    Madd      = 0b0_10_000_11,
    Msub      = 0b0_10_001_11,
    Branch    = 0b0_11_000_11,
    Jal       = 0b0_11_011_11,
    Jalr      = 0b0_11_001_11,
    AuiPC     = 0b0_00_101_11,
    Lui       = 0b0_01_101_11,
    System    = 0b0_11_100_11,
    MiscMem   = 0b0_00_011_11,
    OpImm32   = 0b0_00_110_11,
    Op32      = 0b0_01_110_11,
    OpImm     = 0b0_00_100_11,
    Op        = 0b0_01_100_11,
    OpFp      = 0b0_10_100_11,
    LoadFp    = 0b0_00_001_11,
    StoreFp   = 0b0_01_001_11,
    Amo       = 0b0_01_011_11,
    Nmadd     = 0b0_10_011_11,
    Nmsub     = 0b0_10_010_11,
    Unknown   = 0b0_00_000_00,
}

fn int_to_opcode(insn: u32) -> RiscvOpcode
{
    return match insn & 0x7F {
      0b0_00_000_11 => RiscvOpcode::Load,
      0b0_01_000_11 => RiscvOpcode::Store,
      0b0_10_000_11 => RiscvOpcode::Madd,
      0b0_10_001_11 => RiscvOpcode::Msub,
      0b0_11_000_11 => RiscvOpcode::Branch,
      0b0_11_011_11 => RiscvOpcode::Jal,
      0b0_11_001_11 => RiscvOpcode::Jalr,
      0b0_00_101_11 => RiscvOpcode::AuiPC,
      0b0_01_101_11 => RiscvOpcode::Lui,
      0b0_11_100_11 => RiscvOpcode::System,
      0b0_00_011_11 => RiscvOpcode::MiscMem,
      0b0_00_110_11 => RiscvOpcode::OpImm32,
      0b0_01_110_11 => RiscvOpcode::Op32,
      0b0_00_100_11 => RiscvOpcode::OpImm,
      0b0_01_100_11 => RiscvOpcode::Op,
      0b0_10_100_11 => RiscvOpcode::OpFp,
      0b0_00_001_11 => RiscvOpcode::LoadFp,
      0b0_01_001_11 => RiscvOpcode::StoreFp,
      0b0_01_011_11 => RiscvOpcode::Amo,
      0b0_10_011_11 => RiscvOpcode::Nmadd,
      0b0_10_010_11 => RiscvOpcode::Nmsub,
      _             => RiscvOpcode::Unknown,
    }
}

fn instr_load<T: Memory>(mem: &T, addr: u64) -> Result<u32, &str>
{
    return match mem.read(addr, 4) {
        Ok(data) => Ok(data as u32),
        Err(err) => Err(err),
    };
}

fn data_store<T: Memory>(mem: &mut T, addr: u64, data: u64, size: usize) -> Result<(), &str>
{
    if size != 1 && size != 2 && size != 4 && size != 8 {
        return Err("Invalid size");
    }
    return mem.write(addr, size, data);
}

fn data_load<T: Memory>(mem: &T, addr: u64, size: usize) -> Result<u64, &str>
{
    if size != 1 && size != 2 && size != 4 && size != 8 {
        return Err("Invalid size");
    }
    return mem.read(addr, size);
}

fn insn_is_compressed(insn: u32) -> bool
{
    return (insn & 0x3) != 0x3;
}

fn insn_decompress(compressed: u32) -> Result<u32, ()>
{
    return match compressed & 0x3 {
        0b00 => Ok(0),
        0b01 => {
            return match (compressed >> 13) & 0x7 {
                0b100 => {
                    return match (compressed >> 10) & 0x3 {
                        0b11 => {
                            let opcode = 0b011_0011;
                            let rs2 = ((compressed >> 2) & 0x07) + 8;
                            let rs1 = ((compressed >> 7) & 0x07) + 8;

                            let t = ((compressed >> 10) & 0x4) | (compressed >> 5) & 0x3;
                            let (funct7, funct3) = match t {
                                0b000 => (0b0100000, 0b000), // c.sub
                                0b001 => (0b0000000, 0b100), // c.xor
                                0b010 => (0b0100000, 0b110), // c.or
                                0b011 => (0b0100000, 0b111), // c.and
                                _     => panic!("TODO"),
                            };

                            return Ok(opcode | (rs1 << 7) | (funct3 << 12) | (rs1 << 15) |
                                      (rs2 << 20) | (funct7 << 25));
                        },
                        _    => Err(()),
                    }

                },
                _     => Err(()),
            }
        },
        0b10 => Ok(0),
        _    => Err(()),
    };
}

fn decode_r_type(insn: u32) -> (u8, usize, usize, u8, usize)
{
    let funct7 = ((insn >> 25) & 0x7F) as u8;
    let rs2    = ((insn >> 20) & 0x1F) as usize;
    let rs1    = ((insn >> 15) & 0x1F) as usize;
    let funct3 = ((insn >> 12) & 0x7) as u8;
    let rd     = ((insn >>  7) & 0x1F) as usize;

    return (funct7, rs2, rs1, funct3, rd);
}

fn decode_i_type(insn: u32) -> (i16, usize, u8, usize)
{
    let imm     = ((insn >> 20) & 0x3FF) as i16;
    let rs1     = ((insn >> 15) & 0x1F) as usize;
    let funct3  = ((insn >> 12) & 0x7) as u8;
    let rd      = ((insn >>  7) & 0x1F) as usize;

    return (imm, rs1, funct3, rd);
}

fn decode_s_type(insn: u32) -> (i16, usize, usize, u8)
{
    let imm    = ((((insn >> 25) & 0x7F) << 5) | ((insn >> 7) & 0x1F)) as i16;
    let rs2    = ((insn >> 20) & 0x1F) as usize;
    let rs1    = ((insn >> 15) & 0x1F) as usize;
    let funct3 = ((insn >> 12) & 0x7) as u8;

    return (imm, rs2, rs1, funct3);
}

fn decode_sb_type(insn: u32) -> (i16, usize, usize, u8)
{
    let imm12   = ((insn >> 31) & 0x01) << 12;
    let imm11   = ((insn >>  7) & 0x01) << 11;
    let imm10_5 = ((insn >> 25) & 0x3F) <<  5;
    let imm4_1  = ((insn >>  8) & 0xF)  <<  1;
    let imm     = (imm12 | imm11 | imm10_5 | imm4_1) as i16;
    let rs2     = ((insn >> 20) & 0x1F) as usize;
    let rs1     = ((insn >> 15) & 0x1F) as usize;
    let funct3  = ((insn >> 12) & 0x7) as u8;

    return (imm, rs2, rs1, funct3);
}

fn decode_u_type(insn: u32) -> (u32, usize)
{
    let imm = (((insn >> 12) & 0xFFFFF) << 12) as u32;
    let rd  = ((insn >>  7) & 0x1F) as usize;

    return (imm, rd);
}

fn decode_uj_type(insn: u32) -> (u32, usize)
{
    let imm20    = ((insn >> 31) & 0x001) << 20;
    let imm19_12 = ((insn >> 12) & 0x0FF) << 12;
    let imm11    = ((insn >> 20) & 0x001) << 11;
    let imm10_1  = ((insn >> 21) & 0x3FF) <<  1;
    let imm      = (imm20 | imm19_12 | imm11 | imm10_1) as u32;
    let rd       = ((insn >>  7) & 0x1F) as usize;

    return (imm, rd);
}

fn decode_cr_type(insn: u32) -> (u8, usize, usize)
{
    let funct4 = ((insn >> 12) & 0x0F) as u8;
    let rd     = ((insn >>  7) & 0x1F) as usize;
    let rs2    = ((insn >>  2) & 0x1F) as usize;

    return (funct4, rd, rs2);
}

fn decode_ci_type(insn: u32) -> (u8, i16, usize)
{
    let funct3 = ((insn >> 13) & 0x07) as u8;
    let imm6   = ((insn >> 12) & 0x01) << 5;
    let imm0_5 = ((insn >>  2) & 0x1F);
    let rd     = ((insn >>  7) & 0x1F) as usize;

    let imm = (imm0_5 | imm6) as i16;

    return (funct3, imm, rd);
}

fn decode_css_type(insn: u32) -> (u8, i16, usize)
{
    let funct3 = ((insn >> 13) & 0x07) as u8;
    let imm    = ((insn >>  7) & 0x3F) as i16;
    let rd     = ((insn >>  2) & 0x1F) as usize;

    return (funct3, imm, rd);
}

fn decode_ciw_type(insn: u32) -> (u8, i16, usize)
{
    let funct3 = ((insn >> 13) & 0x07) as u8;
    let imm    = ((insn >>  5) & 0xFF) as i16;
    let rd     = ((insn >>  2) & 0x07) as usize + 8;

    return (funct3, imm, rd);
}

fn decode_cl_type(insn: u32) -> (u8, i16, usize, usize)
{
    let funct3 = ((insn >> 13) & 0x07) as u8;
    let imm2_4 = ((insn >> 10) & 0x07) << 2;
    let imm0_1 =  (insn >>  5) & 0x03;
    let rs1    = ((insn >>  7) & 0x07) as usize + 8;
    let rd     = ((insn >>  2) & 0x07) as usize + 8;

    let imm = (imm0_1 | imm2_4) as i16;

    return (funct3, imm, rs1, rd);
}

fn decode_cs_type(insn: u32) -> (u8, i16, usize, usize)
{
    // cl and cs are functionally identical, just the name changes
    return decode_cl_type(insn);
}

fn decode_cb_type(insn: u32) -> (u8, i16, usize)
{
    let funct3 = ((insn >> 13) & 0x07) as u8;
    let off6_8 = ((insn >> 10) & 0x07) << 6;
    let off1_5 = ((insn >>  2) & 0x1F) << 1;
    let rs1    = ((insn >>  7) & 0x07) as usize + 8;

    let off = (off1_5 | off6_8) as i16;

    return (funct3, off, rs1);
}

fn decode_cj_type(insn: u32) -> (u8, i16)
{
    let funct3 =  ((insn >> 13) & 0x07) as u8;
    let target = (((insn >>  2) & 0x7FF) << 1) as i16;

    return (funct3, target);
}

fn riscv_extend(data: u64, size: usize, sign_extend: bool) -> u64 {
    return match size {
        1 => {
            if sign_extend {
                (data as i8) as u64
            } else {
                (data as u8) as u64
            }
        },
        2 => {
            if sign_extend {
                (data as i16) as u64
            } else {
                (data as u16) as u64
            }
        },
        4 => {
            if sign_extend {
                (data as i32) as u64
            } else {
                (data as u32) as u64
            }
        },
        8 => {
            data
        },
        _ => panic!("Unimplemented size"),
    }
}

fn riscv_decode<T: Memory>(mem: &mut T, state: &mut CpuRegisters) -> ()
{
    println!("Decode called! 0x{:X}", state.pc);
    let insn_loaded: u32;
    match instr_load(mem, state.pc) {
        Ok(d) => insn_loaded = d,
        Err(e) => {
            println!("Unable to load instruction");
            state.pc = 0x0; // TODO: take exception
            return
        }
    }

    println!("insn 0x{:X}", insn_loaded);

    let npc;
    let insn;

    if insn_is_compressed(insn_loaded) {
        match insn_decompress(insn_loaded) {
            Ok(dec_insn) => {
                insn = dec_insn;
                npc = state.pc + 2
            },
            Err(e) => {
                println!("Failed to decompress");
                state.pc = 0x0; // TODO: take exception
                return
            }
        }
    } else {
        insn = insn_loaded;
        npc = state.pc + 4;
    }

    println!("npc is now 0x{:X}", npc);

    let opcode = int_to_opcode(insn);

    state.pc = match opcode {
        RiscvOpcode::Load => {
            let (offset, base, width, dst) = decode_i_type(insn);
            let addr = if offset < 0 {
                state.gpr[base] - (offset as u64)
            } else {
                state.gpr[base] + (offset as u64)
            };
            let (size, sext) = match width {
                0b000 => (1, false),
                0b001 => (2, false),
                0b010 => (4, false),
                0b100 => (1, true),
                0b101 => (2, true),
                0b110 => (4, false), // RV64I
                0b011 => (8, false), // RV64I
                _     => (0, false),
            };
            match data_load(mem, addr, size) {
                Ok(data) => state.gpr[dst] = riscv_extend(data, size, sext),
                Err(e)   => {
                    println!("Unable to load data from addr 0x{:X}", addr);
                    state.pc = 0x0 // TODO: take exception
                },
            }
            npc
        },
        RiscvOpcode::Store => {
            let (offset, src, base, width) = decode_s_type(insn);
            let addr = if offset < 0 {
                state.gpr[base] - (offset as u64)
            } else {
                state.gpr[base] + (offset as u64)
            };
            let size = match width {
                0b000 => 1,
                0b001 => 2,
                0b010 => 4,
                0b011 => 8, // RV64I
                _     => 0,
            };
            let data = state.gpr[src];
            match data_store(mem, addr, data, size) {
                Ok(data) => {},
                Err(e)   => {
                    println!("Unable to store data to addr 0x{:X}", addr);
                    state.pc = 0x0 // TODO: take exception
                },
            }
            npc
        },
        RiscvOpcode::Lui => {
            let (imm, rd) = decode_u_type(insn);
            state.gpr[rd] = ((imm << 12) as i64) as u64;
            npc
        },
        RiscvOpcode::AuiPC => {
            let (imm, rd) = decode_u_type(insn);
            state.gpr[rd] = (((imm << 12) as i64) as u64) + state.pc;
            npc
        },
        RiscvOpcode::Op => {
            let (funct7, rs2, rs1, funct3, rd) = decode_r_type(insn);
            match funct7 {
                //0b0_000_0000 => sll/srl,
                //0b0_010_0000 => sra,
                _ => {
                    println!("Op not implemented");
                    state.pc = 0x0 // TODO: take exception
                },
            }
            npc
        },
        RiscvOpcode::OpImm => {
            let (imm, rs1, funct3, rd) = decode_i_type(insn);
            match funct3 {
                0b000 => state.gpr[rd] = state.gpr[rd] + imm as u64,
                0b010 => state.gpr[rd] = if (state.gpr[rd] as i64) < imm as i64 { 1 } else { 0 },
                0b011 => state.gpr[rd] = if  state.gpr[rd]         < imm as u64 { 1 } else { 0 },
                0b100 => state.gpr[rd] = state.gpr[rd] ^ imm as u64,
                _ => {
                    println!("OpImm not implemented");
                    state.pc = 0x0 // TODO: take exception
                },
            }
            npc
        },
        RiscvOpcode::OpImm32 => {
            let (imm, rs1, funct3, rd) = decode_i_type(insn);
            match funct3 {
                0b000 => state.gpr[rd] = (state.gpr[rd] as u32 + imm as u32) as u64,
                _ => {
                    println!("OpImm not implemented");
                    state.pc = 0x0 // TODO: take exception
                },
            }
            npc
        },
        RiscvOpcode::Branch => {
            println!("BRANCH");
            npc
        },
        RiscvOpcode::Jalr => {
            println!("JALR");
            npc
        },
        RiscvOpcode::Jal => {
            println!("JAL");
            npc
        },
        _ => {
            println!("Instruction not implemented");
            0x0 // TODO: take exception
        },
    }
}
