extern crate elf;

use memory::*;
use std::path::PathBuf;

pub fn load<'out, T: Memory>(filename: &str, mem: &'out mut T) -> Result<u64, &'out str>
{
    let path = PathBuf::from(filename);
    let file = match elf::File::open_path(&path) {
        Ok(f) => f,
        Err(e) => panic!("Error: {:?}", e),
    };

    if file.ehdr.machine.0 != 0xf3 {
        return Err("Wrong machine");
    }

    for section in &file.sections {
        if section.shdr.shtype == elf::types::SHT_PROGBITS
            && (section.shdr.flags.0) & (elf::types::SHF_ALLOC.0) != 0 {
            println!("Section {}", section.shdr.name);
            println!("{:X}", section.shdr.addr);

            for i in 0..section.shdr.size {
                match mem.write(section.shdr.addr + i, 1, section.data[i as usize] as u64) {
                    Err(err) => println!("Failed to write to addr 0x{:X}", section.shdr.addr + i),
                    _ => (),
                }
            }
        }
    }

    return Ok(file.ehdr.entry);
}
