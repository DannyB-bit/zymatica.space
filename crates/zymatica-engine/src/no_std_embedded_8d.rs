//! # Invention: Zymatica Bare-Metal Embedded 8D Engine (no_std)
//! Zero-heap-allocation, fixed-stack-array 8D concept engine for microcontrollers (ARM Cortex, ESP32, STM32, RISC-V).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedConcept8D {
    pub dword: u32,
}

impl EmbeddedConcept8D {
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub const fn new(
        d: u8,
        sub: u8,
        op: u8,
        mod_: u8,
        st: u8,
        pol: u8,
        temp: u8,
        cert: u8,
    ) -> Self {
        let rc = ((d & 0x0F) << 4) | (sub & 0x0F);
        let rf = ((op & 0x0F) << 4) | (mod_ & 0x0F);
        let ra = ((st & 0x0F) << 4) | (pol & 0x0F);
        let rt = ((temp & 0x0F) << 4) | (cert & 0x0F);
        let dw = ((rc as u32) << 24) | ((rf as u32) << 16) | ((ra as u32) << 8) | (rt as u32);
        Self { dword: dw }
    }

    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; 4] {
        self.dword.to_be_bytes()
    }

    #[inline(always)]
    pub fn to_swarm_chirp_16b(&self, sender: u8, epoch: u8, opcode: u8) -> [u8; 16] {
        let mut chirp = [0u8; 16];
        chirp[0] = sender;
        chirp[1] = epoch;
        chirp[2] = ((self.dword >> 24) & 0xFF) as u8;
        chirp[3] = opcode;
        chirp[4] = 100;

        let coords_bytes = self.to_bytes();
        chirp[5] = coords_bytes[0];
        chirp[6] = coords_bytes[1];
        chirp[7] = coords_bytes[2];
        chirp[8] = coords_bytes[3];
        chirp[9] = 0;
        chirp[10] = 0;

        let mut hash = 0x811c9dc5u32;
        for &b in &chirp[..11] {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }

        let crc_b = hash.to_be_bytes();
        chirp[11] = crc_b[0];
        chirp[12] = crc_b[1];
        chirp[13] = crc_b[2];
        chirp[14] = crc_b[3];
        chirp[15] = 0x5A;
        chirp
    }
}
