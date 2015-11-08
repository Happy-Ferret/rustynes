use std::fmt; //for custom Debug

use nes::{Memory, TICKS_PER_SCANLINE};

mod flag {
    pub const SIGN      : u8 = 0x80;
    pub const OVERFLOW  : u8 = 0x40;
    pub const BREAK     : u8 = 0x10;
    pub const DECIMAL   : u8 = 0x08;
    pub const INTERRUPT : u8 = 0x04;
    pub const ZERO      : u8 = 0x02;
    pub const CARRY     : u8 = 0x01;
}

#[derive(Clone)]
pub enum BreakCondition {
    RunToPc(u16),
    RunNext,
    RunToScanline,
    RunFrame
}

pub struct Cpu {
    //registers
    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    pub pc: u16,
    
    //flags
    carry: bool,
    zero: bool,
    interrupt: bool,
    decimal: bool,
    brk: bool,
    overflow: bool,
    sign: bool,
    
    //ticks and timers
    pub tick_count: u32,
    
    pub is_debugging: bool,
    
    //helper fields
    current_opcode: u8,
}

impl fmt::Debug for Cpu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{opcode:{0:02x} a:{1:02x} x:{2:02x} y:{3:02x} sp:{4:02x} pc:{5:04x} flags:{6}{7}{8}{9}{10}{11}}} tick: {12}", self.current_opcode,
            self.a, self.x, self.y, self.sp, self.pc, 
            if self.sign {'N'} else {'-'}, if self.zero { 'Z' } else {'-'}, if self.carry { 'C' } else {'-'}, 
            if self.interrupt {'I'} else {'-'}, if self.decimal {'D'} else {'-'}, if self.overflow {'V'} else {'-'},
            self.tick_count)
    }
}

fn make_address(c: u8, d: u8) -> u16 {
    ((d as u16) << 8) + (c as u16)
}

impl Cpu {
    pub fn new() -> Cpu{
        Cpu {
            a: 0, 
            x: 0, 
            y: 0, 
            sp: 0xff,
            pc: 0xfffc,
            
            carry: false,
            zero: false,
            interrupt: false,
            decimal: false,
            brk: false,
            overflow: false,
            sign: false,
            
            tick_count: 0,
            
            is_debugging: false,
            
            current_opcode: 0,
        }
    }
    fn zero_page(&self, mem: &mut Memory,c: u8) -> u8 {
        mem.mmu.read_u8(&mut mem.ppu, c as u16)
    }
    
    fn zero_page_x(&self, mem: &mut Memory,c: u8) -> u8 {
        let new_addr = 0xff & (c as u16 + self.x as u16);
        mem.mmu.read_u8(&mut mem.ppu, new_addr)
    }
    
    fn zero_page_y(&self, mem: &mut Memory,c: u8) -> u8 {
        let new_addr = 0xff & (c as u16 + self.y as u16);
        mem.mmu.read_u8(&mut mem.ppu, new_addr)
    }
    
    fn absolute(&self, mem: &mut Memory,c: u8, d: u8) -> u8 {
        mem.mmu.read_u8(&mut mem.ppu, make_address(c, d))
    }
    
    fn absolute_x(&mut self, mem: &mut Memory,c: u8, d:u8, check_page: bool) -> u8 {
        if check_page {
            if (make_address(c, d) & 0xFF00) != 
                ((make_address(c, d) + self.x as u16) & 0xFF00) {
                
                self.tick_count += 1;
            }
        }
        
        mem.mmu.read_u8(&mut mem.ppu, make_address(c, d) + self.x as u16)
    }
    
    fn absolute_y(&mut self, mem: &mut Memory,c: u8, d:u8, check_page: bool) -> u8 {
        if check_page {
            if (make_address(c, d) & 0xFF00) != 
                ((make_address(c, d) + self.y as u16) & 0xFF00) {
                
                self.tick_count += 1;
            }
        }
        
        mem.mmu.read_u8(&mut mem.ppu, make_address(c, d) + self.y as u16)
    }
    
    fn indirect_x(&self, mem: &mut Memory,c: u8) -> u8 {
        let new_addr = mem.mmu.read_u16(&mut mem.ppu, 0xff & ((c as u16) + self.x as u16));        
        mem.mmu.read_u8(&mut mem.ppu, new_addr)
    }
    
    fn indirect_y(&mut self, mem: &mut Memory,c: u8, check_page: bool) -> u8 {
        if check_page {
            if (mem.mmu.read_u16(&mut mem.ppu, c as u16) & 0xFF00) !=
                ((mem.mmu.read_u16(&mut mem.ppu, c as u16) + self.y as u16) & 0xFF00) {
                
                self.tick_count += 1;
            }
        }
        
        let addr = mem.mmu.read_u16(&mut mem.ppu, c as u16) + self.y as u16;
        mem.mmu.read_u8(&mut mem.ppu, addr)
    }
    
    fn zero_page_write(&mut self, mem: &mut Memory,c: u8, data: u8) {
        mem.mmu.write_u8(&mut mem.ppu, c as u16, data);
    }
    
    fn zero_page_x_write(&mut self, mem: &mut Memory,c: u8, data: u8) {
        mem.mmu.write_u8(&mut mem.ppu, (c as u16 + self.x as u16) & 0xff, data);
    }

    fn zero_page_y_write(&mut self, mem: &mut Memory,c: u8, data: u8) {
        mem.mmu.write_u8(&mut mem.ppu, (c as u16 + self.y as u16) & 0xff, data);
    }
    
    fn absolute_write(&mut self, mem: &mut Memory,c: u8, d: u8, data: u8) {
        if make_address(c, d) == 0x204 {
            println!("Write to 0x204 at {0:x}", self.pc);
        }
        mem.mmu.write_u8(&mut mem.ppu, make_address(c, d), data);
    }
    
    fn absolute_x_write(&mut self, mem: &mut Memory,c: u8, d: u8, data: u8) {
        if make_address(c, d) + self.x as u16 == 0x204 {
            println!("Write to 0x204 at {0:x}", self.pc);
        }
        mem.mmu.write_u8(&mut mem.ppu, make_address(c, d) + self.x as u16, data);
    }
    
    fn absolute_y_write(&mut self, mem: &mut Memory,c: u8, d: u8, data: u8) {
        if make_address(c, d) + self.y as u16 == 0x204 {
            println!("Write to 0x204 at {0:x}", self.pc);
        }
        mem.mmu.write_u8(&mut mem.ppu, make_address(c, d) + self.y as u16, data);
    }
    
    fn indirect_x_write(&mut self, mem: &mut Memory,c: u8, data: u8) {
        let new_addr = mem.mmu.read_u16(&mut mem.ppu, 0xff & (c as u16 + self.x as u16));
        if new_addr == 0x204 {
            println!("Write to 0x204 at {0:x}", self.pc);
        }
        mem.mmu.write_u8(&mut mem.ppu, new_addr, data);
    }
    
    fn indirect_y_write(&mut self, mem: &mut Memory,c: u8, data: u8) {
        let new_addr = mem.mmu.read_u16(&mut mem.ppu, c as u16) + self.y as u16;
        if new_addr == 0x204 {
            println!("Write to 0x204 at {0:x}", self.pc);
        }
        mem.mmu.write_u8(&mut mem.ppu, new_addr, data);
    }
    
    fn push_u8(&mut self, mem: &mut Memory,data: u8) {
        mem.mmu.write_u8(&mut mem.ppu, 0x100 + self.sp as u16, data);
        if self.sp == 0 {
            self.sp = 0xff;
        }
        else {
            self.sp -= 1;
        }
    }
    
    pub fn push_u16(&mut self, mem: &mut Memory,data: u16) {
        self.push_u8(mem, (data >> 8) as u8);
        self.push_u8(mem, (data & 0xff) as u8);
    }
    
    pub fn push_status(&mut self, mem: &mut Memory) {
        let mut status = 0;
        if self.sign {
            status += flag::SIGN;
        }
        if self.overflow {
            status += flag::OVERFLOW;
        }
        if self.brk {
            status += flag::BREAK;
        }
        if self.decimal {
            status += flag::DECIMAL;
        }
        if self.interrupt {
            status += flag::INTERRUPT;
        }
        if self.zero {
            status += flag::ZERO;
        }
        if self.carry {
            status += flag::CARRY;
        }
        
        self.push_u8(mem, status);        
    }
    
    fn pull_u8(&mut self, mem: &mut Memory) -> u8 {
        if self.sp == 0xff {
            self.sp = 0;
        }
        else {
            self.sp += 1;
        }
        
        mem.mmu.read_u8(&mut mem.ppu, 0x100 + self.sp as u16)
    }
    
    fn pull_u16(&mut self, mem: &mut Memory) -> u16 {
        let data_1 = self.pull_u8(mem);
        let data_2 = self.pull_u8(mem);
        
        make_address(data_1, data_2)
    }
    
    fn pull_status(&mut self, mem: &mut Memory) {
        let status = self.pull_u8(mem);
        
        self.sign = (status & flag::SIGN) == flag::SIGN;
        self.overflow = (status & flag::OVERFLOW) == flag::OVERFLOW;
        self.brk = (status & flag::BREAK) == flag::BREAK;
        self.decimal = (status & flag::DECIMAL) == flag::DECIMAL;
        self.interrupt = (status & flag::INTERRUPT) == flag::INTERRUPT;
        self.zero = (status & flag::ZERO) == flag::ZERO;
        self.carry = (status & flag::CARRY) == flag::CARRY;
    }
    
    fn adc(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0x69 => arg1,
                0x65 => self.zero_page(mem, arg1),
                0x75 => self.zero_page_x(mem, arg1),
                0x6d => self.absolute(mem, arg1, arg2),
                0x7d => self.absolute_x(mem, arg1, arg2, true),
                0x79 => self.absolute_y(mem, arg1, arg2, true),
                0x61 => self.indirect_x(mem, arg1),
                0x71 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
        let total : u16 = self.a as u16 + value as u16 + 
            if self.carry {1} else {0};
        
        self.carry = total > 0xff;
        self.overflow = total > 0xff;
        self.zero = (total & 0xff) == 0;
        self.sign = (total & 0x80) == 0x80;        
        self.a = (total & 0xff) as u8;
        
        match self.current_opcode {
            0x69 => {self.tick_count += 2; self.pc += 2},
            0x65 => {self.tick_count += 3; self.pc += 2},
            0x75 => {self.tick_count += 4; self.pc += 2},
            0x6d => {self.tick_count += 4; self.pc += 3},
            0x7d => {self.tick_count += 4; self.pc += 3},
            0x79 => {self.tick_count += 4; self.pc += 3},
            0x61 => {self.tick_count += 6; self.pc += 2},
            0x71 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in adc")
        }            
    }

    fn and(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0x29 => arg1,
                0x25 => self.zero_page(mem, arg1),
                0x35 => self.zero_page_x(mem, arg1),
                0x2d => self.absolute(mem, arg1, arg2),
                0x3d => self.absolute_x(mem, arg1, arg2, true),
                0x39 => self.absolute_y(mem, arg1, arg2, true),
                0x21 => self.indirect_x(mem, arg1),
                0x31 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        self.a = self.a & value;
        self.zero = (self.a & 0xff) == 0;
        self.sign = (self.a & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x29 => {self.tick_count += 2; self.pc += 2},
            0x25 => {self.tick_count += 3; self.pc += 2},
            0x35 => {self.tick_count += 4; self.pc += 2},
            0x2d => {self.tick_count += 4; self.pc += 3},
            0x3d => {self.tick_count += 4; self.pc += 3},
            0x39 => {self.tick_count += 4; self.pc += 3},
            0x21 => {self.tick_count += 6; self.pc += 2},
            0x31 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in and")
        }            
    }

    fn asl(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value : u8 = 
            match self.current_opcode {
                0x0a => self.a,
                0x06 => self.zero_page(mem, arg1),
                0x16 => self.zero_page_x(mem, arg1),
                0x0e => self.absolute(mem, arg1, arg2),
                0x1e => self.absolute_x(mem, arg1, arg2, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        self.carry = (value & 0x80) == 0x80;
        value = (0xff & ((value as u16) << 1)) as u8;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x0a => {self.a = value; 
                self.tick_count += 2; self.pc += 1},
            0x06 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0x16 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0x0e => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0x1e => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in asl")
        }            
    }
    
    fn bcc(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if !self.carry {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn bcs(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if self.carry {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn beq(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if self.zero {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn bit(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0x24 => self.zero_page(mem, arg1),
                0x2c => self.absolute(mem, arg1, arg2),
                _ => {println!("Unknown opcode"); 0}
            };
        
        self.zero = (self.a & value) == 0;
        self.sign = (value & 0x80) == 0x80;        
        self.overflow = (value & 0x40) == 0x40;
        
        match self.current_opcode {
            0x24 => {self.tick_count += 3; self.pc += 2},
            0x2c => {self.tick_count += 4; self.pc += 3},
            _ => {}
        }
    }
    
    fn bmi(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if self.sign {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn bne(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if !self.zero {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn bpl(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if !self.sign {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }
    
    fn brk(&mut self, mem: &mut Memory) {
        self.pc = 0xff & (self.pc as u16 + 2);
        let tmp_pc = self.pc;
        self.push_u16(mem, tmp_pc);
        self.brk = true;
        self.push_status(mem);
        self.interrupt = true;
        self.pc = mem.mmu.read_u16(&mut mem.ppu, 0xfffe);
        self.tick_count += 7;
    }
    
    fn bvc(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if !self.overflow {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }

    fn bvs(&mut self, mem: &mut Memory) {
        let arg1 : i8 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1) as i8;
        
        self.pc += 2;
        
        if self.overflow {
            if (self.pc & 0xff00) != ((self.pc as i16 + 2i16 + arg1 as i16) as u16 & 0xff00) {
                self.tick_count += 1;
            }
            self.pc = (0xffff & (self.pc as i32 + arg1 as i32)) as u16;
            self.tick_count += 1;
        }
        
        self.tick_count += 2;
    }
    
    fn clc(&mut self) {
        self.carry = false;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn cld(&mut self) {
        self.decimal = false;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn cli(&mut self) {
        self.interrupt = false;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn clv(&mut self) {
        self.overflow = false;
        self.pc += 1;
        self.tick_count += 2;
    }

    fn cmp(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value = 
            match self.current_opcode {
                0xc9 => arg1,
                0xc5 => self.zero_page(mem, arg1),
                0xd5 => self.zero_page_x(mem, arg1),
                0xcd => self.absolute(mem, arg1, arg2),
                0xdd => self.absolute_x(mem, arg1, arg2, true),
                0xd9 => self.absolute_y(mem, arg1, arg2, true),
                0xc1 => self.indirect_x(mem, arg1),
                0xd1 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
            
        self.carry = self.a >= value;
        value = (0xff & ((self.a as i16) - value as i16)) as u8;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;
        
        match self.current_opcode {
            0xc9 => {self.tick_count += 2; self.pc += 2},
            0xc5 => {self.tick_count += 3; self.pc += 2},
            0xd5 => {self.tick_count += 4; self.pc += 2},
            0xcd => {self.tick_count += 4; self.pc += 3},
            0xdd => {self.tick_count += 4; self.pc += 3},
            0xd9 => {self.tick_count += 4; self.pc += 3},
            0xc1 => {self.tick_count += 6; self.pc += 2},
            0xd1 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in cmp")
        }            
    }

    fn cpx(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value = 
            match self.current_opcode {
                0xe0 => arg1,
                0xe4 => self.zero_page(mem, arg1),
                0xec => self.absolute(mem, arg1, arg2),
                _ => {println!("Unknown opcode"); 0}
            };
            
        self.carry = self.x >= value;
        value = (0xff & ((self.x as i16) - value as i16)) as u8;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;
        
        match self.current_opcode {
            0xe0 => {self.tick_count += 2; self.pc += 2},
            0xe4 => {self.tick_count += 3; self.pc += 2},
            0xec => {self.tick_count += 4; self.pc += 3},
            _ => println!("unknown opcode in cpx")
        }            
    }

    fn cpy(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value = 
            match self.current_opcode {
                0xc0 => arg1,
                0xc4 => self.zero_page(mem, arg1),
                0xcc => self.absolute(mem, arg1, arg2),
                _ => {println!("Unknown opcode"); 0}
            };
            
        self.carry = self.y >= value;
        value = (0xff & ((self.y as i16) - value as i16)) as u8;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;
        
        match self.current_opcode {
            0xc0 => {self.tick_count += 2; self.pc += 2},
            0xc4 => {self.tick_count += 3; self.pc += 2},
            0xcc => {self.tick_count += 4; self.pc += 3},
            _ => println!("unknown opcode in cpy")
        }            
    }    
    
    fn dec(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value : u8 = 
            match self.current_opcode {
                0xc6 => self.zero_page(mem, arg1),
                0xd6 => self.zero_page_x(mem, arg1),
                0xce => self.absolute(mem, arg1, arg2),
                0xde => self.absolute_x(mem, arg1, arg2, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        if value == 0 {
            value = 0xff;
        }
        else {
            value -= 1;
        }
        
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0xc6 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0xd6 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0xce => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0xde => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in dec")
        }            
    }
    
    fn dex(&mut self) {
        if self.x == 0 {
            self.x = 0xff;
        }
        else {
            self.x -= 1;
        }
        
        self.zero = self.x == 0;
        self.sign = (self.x & 0x80) == 0x80;
        
        self.pc += 1;
        self.tick_count += 2;
    }

    fn dey(&mut self) {
        if self.y == 0 {
            self.y = 0xff;
        }
        else {
            self.y -= 1;
        }
        
        self.zero = self.y == 0;
        self.sign = (self.y & 0x80) == 0x80;
        
        self.pc += 1;
        self.tick_count += 2;
    }

    fn eor(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0x49 => arg1,
                0x45 => self.zero_page(mem, arg1),
                0x55 => self.zero_page_x(mem, arg1),
                0x4d => self.absolute(mem, arg1, arg2),
                0x5d => self.absolute_x(mem, arg1, arg2, true),
                0x59 => self.absolute_y(mem, arg1, arg2, true),
                0x41 => self.indirect_x(mem, arg1),
                0x51 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
 
        self.a = self.a ^ value;           
        self.zero = self.a == 0;
        self.sign = (self.a & 0x80) == 0x80;
        
        match self.current_opcode {
            0x49 => {self.tick_count += 2; self.pc += 2},
            0x45 => {self.tick_count += 3; self.pc += 2},
            0x55 => {self.tick_count += 4; self.pc += 2},
            0x4d => {self.tick_count += 4; self.pc += 3},
            0x5d => {self.tick_count += 4; self.pc += 3},
            0x59 => {self.tick_count += 4; self.pc += 3},
            0x41 => {self.tick_count += 6; self.pc += 2},
            0x51 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in cmp")
        }            
    }
    
    fn inc(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value : u8 = 
            match self.current_opcode {
                0xe6 => self.zero_page(mem, arg1),
                0xf6 => self.zero_page_x(mem, arg1),
                0xee => self.absolute(mem, arg1, arg2),
                0xfe => self.absolute_x(mem, arg1, arg2, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        if value == 0xff {
            value = 0;
        }
        else {
            value += 1;
        }
        
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0xe6 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0xf6 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0xee => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0xfe => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in inc")
        }            
    }    
    
    fn inx(&mut self) {
        if self.x == 0xff {
            self.x = 0;
        }
        else {
            self.x += 1;
        }
        
        self.zero = self.x == 0;
        self.sign = (self.x & 0x80) == 0x80;
        
        self.pc += 1;
        self.tick_count += 2;
    }

    fn iny(&mut self) {
        if self.y == 0xff {
            self.y = 0;
        }
        else {
            self.y += 1;
        }
        
        self.zero = self.y == 0;
        self.sign = (self.y & 0x80) == 0x80;
        
        self.pc += 1;
        self.tick_count += 2;
    }

    fn jmp(&mut self, mem: &mut Memory) {
        let addr = mem.mmu.read_u16(&mut mem.ppu, self.pc + 1);
        
        match self.current_opcode {
            0x4c => {self.pc = addr; self.tick_count += 3},
            0x6c => {self.pc = mem.mmu.read_u16(&mut mem.ppu, addr); self.tick_count += 5},
            _ => println!("Unknown opcode in jmp")
        }
    }
    
    fn jsr(&mut self, mem: &mut Memory) {
        let pc = self.pc;
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, pc + 2);
        self.push_u16(mem, pc + 2);
        self.pc = make_address(arg1, arg2);
        self.tick_count += 6;
    }
    
    fn lda(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        
        match self.current_opcode {
            0xa9 => {self.a = arg1; 
                self.tick_count += 2; self.pc += 2},
            0xa5 => {self.a = self.zero_page(mem, arg1); 
                self.tick_count += 3; self.pc += 2},
            0xb5 => {self.a = self.zero_page_x(mem, arg1); 
                self.tick_count += 4; self.pc += 2},
            0xad => {let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2); 
                self.a = self.absolute(mem, arg1, arg2);
                self.tick_count += 4; self.pc += 3},
            0xbd => {let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
                self.a = self.absolute_x(mem, arg1, arg2, true); 
                self.tick_count += 4; self.pc += 3},
            0xb9 => {let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
                self.a = self.absolute_y(mem, arg1, arg2, true);
                self.tick_count += 4; self.pc += 3},
            0xa1 => {self.a = self.indirect_x(mem, arg1); 
                self.tick_count += 6; self.pc += 2},
            0xb1 => {self.a = self.indirect_y(mem, arg1, true); 
                self.tick_count += 5; self.pc += 2},
            _ => println!("Unknown opcode in lda")
        }
        
        self.zero = self.a == 0;
        self.sign = (self.a & 0x80) == 0x80;
    }
    
    fn ldx(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        match self.current_opcode {
            0xa2 => {self.x = arg1; 
                self.tick_count += 2; self.pc += 2},
            0xa6 => {self.x = self.zero_page(mem, arg1); 
                self.tick_count += 3; self.pc += 2},
            0xb6 => {self.x = self.zero_page_y(mem, arg1); 
                self.tick_count += 4; self.pc += 2},
            0xae => {self.x = self.absolute(mem, arg1, arg2);
                self.tick_count += 4; self.pc += 3},
            0xbe => {self.x = self.absolute_y(mem, arg1, arg2, true);
                self.tick_count += 4; self.pc += 3},
            _ => println!("Unknown opcode in ldx")
        }
        
        self.zero = self.x == 0;
        self.sign = (self.x & 0x80) == 0x80;
    }
    
    fn ldy(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        match self.current_opcode {
            0xa0 => {self.y = arg1; 
                self.tick_count += 2; self.pc += 2},
            0xa4 => {self.y = self.zero_page(mem, arg1); 
                self.tick_count += 3; self.pc += 2},
            0xb4 => {self.y = self.zero_page_x(mem, arg1); 
                self.tick_count += 4; self.pc += 2},
            0xac => {self.y = self.absolute(mem, arg1, arg2);
                self.tick_count += 4; self.pc += 3},
            0xbc => {self.y = self.absolute_x(mem, arg1, arg2, true);
                self.tick_count += 4; self.pc += 3},
            _ => println!("Unknown opcode in ldx")
        }
        
        self.zero = self.y == 0;
        self.sign = (self.y & 0x80) == 0x80;
    }

    fn lsr(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value : u8 = 
            match self.current_opcode {
                0x4a => self.a,
                0x46 => self.zero_page(mem, arg1),
                0x56 => self.zero_page_x(mem, arg1),
                0x4e => self.absolute(mem, arg1, arg2),
                0x5e => self.absolute_x(mem, arg1, arg2, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        self.carry = (self.a & 0x1) == 0x1;
        value = value >> 1;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x4a => {self.a = value; 
                self.tick_count += 2; self.pc += 1},
            0x46 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0x56 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0x4e => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0x5e => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in lsr")
        }
    }
    
    fn nop(&mut self) {
        self.pc += 1;
        self.tick_count += 1;
    }

    fn ora(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0x09 => arg1,
                0x05 => self.zero_page(mem, arg1),
                0x15 => self.zero_page_x(mem, arg1),
                0x0d => self.absolute(mem, arg1, arg2),
                0x1d => self.absolute_x(mem, arg1, arg2, true),
                0x19 => self.absolute_y(mem, arg1, arg2, true),
                0x01 => self.indirect_x(mem, arg1),
                0x11 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        self.a = self.a | value;
        self.zero = (self.a & 0xff) == 0;
        self.sign = (self.a & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x09 => {self.tick_count += 2; self.pc += 2},
            0x05 => {self.tick_count += 3; self.pc += 2},
            0x15 => {self.tick_count += 4; self.pc += 2},
            0x0d => {self.tick_count += 4; self.pc += 3},
            0x1d => {self.tick_count += 4; self.pc += 3},
            0x19 => {self.tick_count += 4; self.pc += 3},
            0x01 => {self.tick_count += 6; self.pc += 2},
            0x11 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in and")
        }
    }
    
    fn pha(&mut self, mem: &mut Memory) {
        let a = self.a;
        self.push_u8(mem, a);
        self.pc += 1;
        self.tick_count += 3;
    }
    
    fn php(&mut self, mem: &mut Memory) {
        self.push_status(mem);
        self.pc += 1;
        self.tick_count += 3;
    }
    
    fn pla(&mut self, mem: &mut Memory) {
        self.a = self.pull_u8(mem);
        self.zero = self.a == 0;
        self.sign = (self.a & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 4;
    }
    
    fn plp(&mut self, mem: &mut Memory) {
        self.pull_status(mem);
        self.pc += 1;
        self.tick_count += 4;
    }

    fn rol(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value = 
            match self.current_opcode {
                0x2a => self.a,
                0x26 => self.zero_page(mem, arg1),
                0x36 => self.zero_page_x(mem, arg1),
                0x2e => self.absolute(mem, arg1, arg2),
                0x3e => self.absolute_x(mem, arg1, arg2, false),
                _ => {println!("Unknown opcode"); 0}
            };
        
        let bit = (value & 0x80) == 0x80;
        value = (value & 0x7f) << 1;
        value += if self.carry {1} else {0};
        self.carry = bit;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x2a => {self.a = value; 
                self.tick_count += 2; self.pc += 1},
            0x26 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0x36 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0x2e => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0x3e => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in rol")
        }
    }

    fn ror(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let mut value = 
            match self.current_opcode {
                0x6a => self.a,
                0x66 => self.zero_page(mem, arg1),
                0x76 => self.zero_page_x(mem, arg1),
                0x6e => self.absolute(mem, arg1, arg2),
                0x7e => self.absolute_x(mem, arg1, arg2, true),
                _ => {println!("Unknown opcode"); 0}
            };
        
        let bit = (value & 0x1) == 0x1;
        value = value >> 1;
        value += if self.carry {0x80} else {0};
        self.carry = bit;
        self.zero = value == 0;
        self.sign = (value & 0x80) == 0x80;        
        
        match self.current_opcode {
            0x6a => {self.a = value; 
                self.tick_count += 2; self.pc += 1},
            0x66 => {self.zero_page_write(mem, arg1, value); 
                self.tick_count += 5; self.pc += 2},
            0x76 => {self.zero_page_x_write(mem, arg1, value); 
                self.tick_count += 6; self.pc += 2},
            0x6e => {self.absolute_write(mem, arg1, arg2, value);
                self.tick_count += 6; self.pc += 3},
            0x7e => {self.absolute_x_write(mem, arg1, arg2, value);
                self.tick_count += 7; self.pc += 3},
            _ => println!("unknown opcode in ror")
        }
    }
    
    fn rti(&mut self, mem: &mut Memory) {
        self.pull_status(mem);
        self.pc = self.pull_u16(mem);
        self.tick_count += 6;
    }
    
    fn rts(&mut self, mem: &mut Memory) {
        self.pc = self.pull_u16(mem) + 1;
        self.tick_count += 6;
    }
    
    fn sbc(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let value = 
            match self.current_opcode {
                0xe9 => arg1,
                0xe5 => self.zero_page(mem, arg1),
                0xf5 => self.zero_page_x(mem, arg1),
                0xed => self.absolute(mem, arg1, arg2),
                0xfd => self.absolute_x(mem, arg1, arg2, true),
                0xf9 => self.absolute_y(mem, arg1, arg2, true),
                0xe1 => self.indirect_x(mem, arg1),
                0xf1 => self.indirect_y(mem, arg1, true),
                _ => {println!("Unknown opcode"); 0}
            };
        let total : i16 = self.a as i16 - value as i16 - 
            if self.carry {1} else {0};
        
        self.carry = total >= 0;
        self.overflow = total < 0;
        self.zero = (total & 0xff) == 0;
        self.sign = (total & 0x80) == 0x80;        
        self.a = (total & 0xff) as u8;
        
        match self.current_opcode {
            0xe9 => {self.tick_count += 2; self.pc += 2},
            0xe5 => {self.tick_count += 3; self.pc += 2},
            0xf5 => {self.tick_count += 4; self.pc += 2},
            0xed => {self.tick_count += 4; self.pc += 3},
            0xfd => {self.tick_count += 4; self.pc += 3},
            0xf9 => {self.tick_count += 4; self.pc += 3},
            0xe1 => {self.tick_count += 6; self.pc += 2},
            0xf1 => {self.tick_count += 5; self.pc += 2},
            _ => println!("unknown opcode in sbc")
        }
    }
    
    fn sec(&mut self) {
        self.carry = true;
        self.tick_count += 2;
        self.pc += 1;
    }
    
    fn sed(&mut self) {
        self.decimal = true;
        self.tick_count += 2;
        self.pc += 1;
    }
    
    fn sei(&mut self) {
        self.interrupt = true;
        self.tick_count += 2;
        self.pc += 1;
    }

    fn sta(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let a = self.a;
        match self.current_opcode {
            0x85 => {self.zero_page_write(mem, arg1, a); 
                self.tick_count += 3; self.pc += 2},
            0x95 => {self.zero_page_x_write(mem, arg1, a);
                self.tick_count += 4; self.pc += 2},
            0x8d => {self.absolute_write(mem, arg1, arg2, a); 
                self.tick_count += 4; self.pc += 3},
            0x9d => {self.absolute_x_write(mem, arg1, arg2, a);
                self.tick_count += 5; self.pc += 3},
            0x99 => {self.absolute_y_write(mem, arg1, arg2, a);
                self.tick_count += 5; self.pc += 3},
            0x81 => {self.indirect_x_write(mem, arg1, a);
                self.tick_count += 6; self.pc += 2},
            0x91 => {self.indirect_y_write(mem, arg1, a);
                self.tick_count += 6; self.pc += 2},
            _ => println!("Unknown opcode in sta")
        }
    }
    
    fn stx(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let x = self.x;
        match self.current_opcode {
            0x86 => {self.zero_page_write(mem, arg1, x); 
                self.tick_count += 3; self.pc += 2},
            0x96 => {self.zero_page_y_write(mem, arg1, x);
                self.tick_count += 4; self.pc += 2},
            0x8e => {self.absolute_write(mem, arg1, arg2, x); 
                self.tick_count += 4; self.pc += 3},
            _ => println!("Unknown opcode in stx")
        }
    }
    
    fn sty(&mut self, mem: &mut Memory) {
        let arg1 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 1);
        let arg2 = mem.mmu.read_u8(&mut mem.ppu, self.pc + 2);
        
        let y = self.y;
        match self.current_opcode {
            0x84 => {self.zero_page_write(mem, arg1, y); 
                self.tick_count += 3; self.pc += 2},
            0x94 => {self.zero_page_x_write(mem, arg1, y);
                self.tick_count += 4; self.pc += 2},
            0x8c => {self.absolute_write(mem, arg1, arg2, y); 
                self.tick_count += 4; self.pc += 3},
            _ => println!("Unknown opcode in sty")
        }
    }
    
    fn tax(&mut self) {
        self.x = self.a;
        self.zero = self.x == 0;
        self.sign = (self.x & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn tay(&mut self) {
        self.y = self.a;
        self.zero = self.y == 0;
        self.sign = (self.y & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn tsx(&mut self) {
        self.x = self.sp;
        self.zero = self.x == 0;
        self.sign = (self.x & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn txa(&mut self) {
        self.a = self.x;
        self.zero = self.a == 0;
        self.sign = (self.a & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn txs(&mut self) {
        self.sp = self.x;
        
        self.pc += 1;
        self.tick_count += 2;
    }
    
    fn tya(&mut self) {
        self.a = self.y;
        self.zero = self.a == 0;
        self.sign = (self.a & 0x80) == 0x80;
        self.pc += 1;
        self.tick_count += 2;
    }
    
    pub fn reset(&mut self, mem: &mut Memory) {
        //reset pc using reset vector
        self.pc = mem.mmu.read_u16(&mut mem.ppu, 0xfffc);
    }
    
    pub fn fetch_and_execute(&mut self, mem: &mut Memory) {
        self.current_opcode = mem.mmu.read_u8(&mut mem.ppu, self.pc);
                
        match self.current_opcode {
            0x00 => self.brk(mem),
            0x01 => self.ora(mem), 
            0x05 => self.ora(mem),  //0x05
            0x06 => self.asl(mem),
            0x08 => self.php(mem),
            0x09 => self.ora(mem),
            0x0a => self.asl(mem), 
            0x0d => self.ora(mem), 
            0x0e => self.asl(mem),   //0x0E
            0x10 => self.bpl(mem), 
            0x11 => self.ora(mem), 
            0x15 => self.ora(mem), 
            0x16 => self.asl(mem), 
            0x18 => self.clc(), 
            0x19 => self.ora(mem), 
            0x1d => self.ora(mem), 
            0x1e => self.asl(mem), 
            0x20 => self.jsr(mem),  //0x20
            0x21 => self.and(mem), 
            0x24 => self.bit(mem), 
            0x25 => self.and(mem), 
            0x26 => self.rol(mem), 
            0x28 => self.plp(mem), 
            0x29 => self.and(mem),  //0x29
            0x2a => self.rol(mem), 
            0x2c => self.bit(mem), 
            0x2d => self.and(mem), 
            0x2e => self.rol(mem), 
            0x30 => self.bmi(mem), 
            0x31 => self.and(mem), 
            0x32 => self.nop(),        //0x32
            0x33 => self.nop(), 
            0x34 => self.nop(), 
            0x35 => self.and(mem), 
            0x36 => self.rol(mem), 
            0x38 => self.sec(), 
            0x39 => self.and(mem), 
            0x3d => self.and(mem), 
            0x3e => self.rol(mem), 
            0x40 => self.rti(mem), 
            0x41 => self.eor(mem), 
            0x45 => self.eor(mem), 
            0x46 => self.lsr(mem), 
            0x48 => self.pha(mem), 
            0x49 => self.eor(mem), 
            0x4a => self.lsr(mem), 
            0x4c => self.jmp(mem), 
            0x4d => self.eor(mem), //0x4D
            0x4e => self.lsr(mem), 
            0x50 => self.bvc(mem), 
            0x51 => self.eor(mem), 
            0x55 => self.eor(mem), 
            0x56 => self.lsr(mem), //0x56
            0x58 => self.cli(), 
            0x59 => self.eor(mem), 
            0x5d => self.eor(mem), 
            0x5e => self.lsr(mem), 
            0x60 => self.rts(mem), 
            0x61 => self.adc(mem), 
            0x65 => self.adc(mem), 
            0x66 => self.ror(mem), 
            0x68 => self.pla(mem), //0x68
            0x69 => self.adc(mem), 
            0x6a => self.ror(mem), 
            0x6c => self.jmp(mem), 
            0x6d => self.adc(mem), 
            0x6e => self.ror(mem), 
            0x70 => self.bvs(mem), 
            0x71 => self.adc(mem), //0x71
            0x75 => self.adc(mem), 
            0x76 => self.ror(mem), 
            0x78 => self.sei(), 
            0x79 => self.adc(mem), 
            0x7d => self.adc(mem), 
            0x7e => self.ror(mem), 
            0x81 => self.sta(mem), 
            0x84 => self.sty(mem), 
            0x85 => self.sta(mem), 
            0x86 => self.stx(mem), 
            0x88 => self.dey(), 
            0x8a => self.txa(), 
            0x8c => self.sty(mem), //0x8C
            0x8d => self.sta(mem), 
            0x8e => self.stx(mem), 
            0x90 => self.bcc(mem), 
            0x91 => self.sta(mem), 
            0x94 => self.sty(mem), 
            0x95 => self.sta(mem), //0x95
            0x96 => self.stx(mem), 
            0x98 => self.tya(), 
            0x99 => self.sta(mem), 
            0x9a => self.txs(), 
            0x9d => self.sta(mem), 
            0xa0 => self.ldy(mem), 
            0xa1 => self.lda(mem), 
            0xa2 => self.ldx(mem), 
            0xa4 => self.ldy(mem), 
            0xa5 => self.lda(mem), 
            0xa6 => self.ldx(mem), 
            0xa8 => self.tay(), 
            0xa9 => self.lda(mem), 
            0xaa => self.tax(), 
            0xac => self.ldy(mem), 
            0xad => self.lda(mem), 
            0xae => self.ldx(mem), 
            0xb0 => self.bcs(mem), //0xB0
            0xb1 => self.lda(mem), 
            0xb4 => self.ldy(mem), 
            0xb5 => self.lda(mem), 
            0xb6 => self.ldx(mem), 
            0xb8 => self.clv(), 
            0xb9 => self.lda(mem), //0xB9
            0xba => self.tsx(), 
            0xbc => self.ldy(mem), 
            0xbd => self.lda(mem), 
            0xbe => self.ldx(mem), 
            0xc0 => self.cpy(mem), 
            0xc1 => self.cmp(mem), 
            0xc4 => self.cpy(mem), 
            0xc5 => self.cmp(mem), 
            0xc6 => self.dec(mem), 
            0xc8 => self.iny(), 
            0xc9 => self.cmp(mem), 
            0xca => self.dex(), 
            0xcc => self.cpy(mem), 
            0xcd => self.cmp(mem), 
            0xce => self.dec(mem), 
            0xd0 => self.bne(mem), 
            0xd1 => self.cmp(mem), 
            0xd5 => self.cmp(mem), 
            0xd6 => self.dec(mem), 
            0xd8 => self.cld(), 
            0xd9 => self.cmp(mem), 
            0xdd => self.cmp(mem), //0xDD
            0xde => self.dec(mem), 
            0xe0 => self.cpx(mem), 
            0xe1 => self.sbc(mem), 
            0xe4 => self.cpx(mem), 
            0xe5 => self.sbc(mem), 
            0xe6 => self.inc(mem), //0xE6
            0xe8 => self.inx(), 
            0xe9 => self.sbc(mem), 
            0xec => self.cpx(mem), 
            0xed => self.sbc(mem), 
            0xee => self.inc(mem), 
            0xf0 => self.beq(mem), 
            0xf1 => self.sbc(mem), 
            0xf5 => self.sbc(mem), 
            0xf6 => self.inc(mem), 
            0xf8 => self.sed(),       //0xF8
            0xf9 => self.sbc(mem), 
            0xfd => self.sbc(mem), 
            0xfe => self.inc(mem),
            _ => println!("Error, bad opcode: {0:x}", self.current_opcode)
        }    
    }
    
    pub fn run_until_condition(&mut self, mem: &mut Memory, break_cond: &BreakCondition) -> bool {
        let starting_tick_count = self.tick_count;
        
        while self.tick_count < TICKS_PER_SCANLINE {
            if self.is_debugging {
                //Print out each step, assuming we're not taking a step (as that will already be visible)
                match break_cond {
                     &BreakCondition::RunNext => {},
                     _ => println!("{:?}", self)
                }
            }

            self.fetch_and_execute(mem);
            
            match break_cond {
                &BreakCondition::RunToPc(pc)   => if self.pc == pc { return true; },
                &BreakCondition::RunNext       => if self.tick_count != starting_tick_count { return true; },
                &BreakCondition::RunToScanline => if self.tick_count >= TICKS_PER_SCANLINE { return true; },
                &BreakCondition::RunFrame      => {}
            }
        }
        
        false
    }
}