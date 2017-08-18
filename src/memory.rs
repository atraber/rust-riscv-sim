pub struct SimpleRam {
    mem : Box<[u64]>
}

impl SimpleRam {
    pub fn new(n: usize) -> SimpleRam {
        let vec = vec![0u64; n];
        let mem = vec.into_boxed_slice();

        return SimpleRam {
            mem: mem
        }
    }
}

pub trait Memory {
    fn read(&self, addr: u64, size: usize) -> Result<u64, &str>;
    fn write(&mut self, addr: u64, size: usize, data: u64) -> Result<(), &str>;
}

impl Memory for SimpleRam {
    fn read(&self, addr: u64, size: usize) -> Result<u64, &str>
    {
        // right shift by 3 to eliminate byte address
        let index = (addr as usize) >> 3;
        let byte  = (addr as usize) & 0x7;
        if index >= self.mem.len() {
            return Err("Address out of range");
        }

        if byte + size > 8 {
            return Err("Byte address + size makes this access unaligned");
        }

        let data = self.mem[index] >> (8 * byte);

        return match size {
            1 => Ok(data & 0xFF),
            2 => Ok(data & 0xFFFF),
            4 => Ok(data & 0xFFFFFFFF),
            8 => Ok(data & 0xFFFFFFFF_FFFFFFFF),
            _ => Err("Invalid size"),
        };
    }

    fn write(&mut self, addr: u64, size: usize, data: u64) -> Result<(), &str>
    {
        // right shift by 3 to eliminate byte address
        let index = (addr as usize) >> 3;
        let byte  = (addr as usize) & 0x7;
        if index >= self.mem.len() {
            return Err("Address out of range");
        }

        if byte + size > 8 {
            return Err("Byte address + size makes this access unaligned");
        }


        let value = data << (8 * byte);
        let mask;
        match size {
            1 => mask = 0xFF << (8 * byte),
            2 => mask = 0xFFFF << (8 * byte),
            4 => mask = 0xFFFFFFFF << (8 * byte),
            8 => mask = 0xFFFFFFFF_FFFFFFFF << (8 * byte),
            _ => {
                return Err("Invalid size");
            },
        };

        let old = self.mem[index];
        self.mem[index] = (old & (!mask)) | (value & mask);

        return Ok(());
    }
}
