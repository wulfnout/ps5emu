//! Guest memory.
//!
//! Current scope (Phase 1–3): a set of non-overlapping mapped regions, each with
//! read / write / execute permissions. Every access is bounds- and permission-
//! checked and returns a [`MemoryFault`] instead of panicking.
//!
//! NOT YET IMPLEMENTED (tracked in docs/compatibility.md):
//! - virtual address translation / page tables
//! - memory-mapped devices
//! - the PS5's unified GDDR6 layout and its GPU-visible mappings

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

impl Perms {
    pub const R: Perms = Perms { read: true, write: false, exec: false };
    pub const RW: Perms = Perms { read: true, write: true, exec: false };
    pub const RX: Perms = Perms { read: true, write: false, exec: true };
    pub const RWX: Perms = Perms { read: true, write: true, exec: true };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// No region is mapped at this address.
    Unmapped,
    /// A region is mapped but does not allow this access.
    Permission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryFault {
    pub addr: u64,
    pub size: usize,
    pub access: Access,
    pub kind: FaultKind,
}

impl fmt::Display for MemoryFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} fault ({:?}) at {:#x} size {}",
            self.access, self.kind, self.addr, self.size
        )
    }
}

impl std::error::Error for MemoryFault {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    Overlap { base: u64, size: u64 },
    ZeroSize,
    AddressOverflow,
}

#[derive(Debug, Clone)]
struct Region {
    base: u64,
    perms: Perms,
    bytes: Vec<u8>,
}

impl Region {
    fn end(&self) -> u64 {
        self.base + self.bytes.len() as u64
    }
}

#[derive(Debug, Clone, Default)]
pub struct GuestMemory {
    regions: Vec<Region>,
}

impl GuestMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a zero-filled region.
    pub fn map(&mut self, base: u64, size: u64, perms: Perms) -> Result<(), MapError> {
        if size == 0 {
            return Err(MapError::ZeroSize);
        }
        let end = base.checked_add(size).ok_or(MapError::AddressOverflow)?;
        if self.regions.iter().any(|r| base < r.end() && r.base < end) {
            return Err(MapError::Overlap { base, size });
        }
        self.regions.push(Region { base, perms, bytes: vec![0; size as usize] });
        self.regions.sort_by_key(|r| r.base);
        Ok(())
    }

    pub fn regions(&self) -> impl Iterator<Item = (u64, u64, Perms)> + '_ {
        self.regions.iter().map(|r| (r.base, r.bytes.len() as u64, r.perms))
    }

    /// Find the region that fully contains `[addr, addr+size)`.
    /// Accesses that straddle two regions are reported as faults; this is
    /// stricter than real hardware and is documented as a known limitation.
    fn locate(&self, addr: u64, size: usize, access: Access) -> Result<(usize, usize), MemoryFault> {
        let fault = |kind| MemoryFault { addr, size, access, kind };
        let end = addr.checked_add(size as u64).ok_or(fault(FaultKind::Unmapped))?;
        let idx = self
            .regions
            .iter()
            .position(|r| addr >= r.base && end <= r.end())
            .ok_or(fault(FaultKind::Unmapped))?;
        let r = &self.regions[idx];
        let allowed = match access {
            Access::Read => r.perms.read,
            Access::Write => r.perms.write,
            Access::Execute => r.perms.exec,
        };
        if !allowed {
            return Err(fault(FaultKind::Permission));
        }
        Ok((idx, (addr - r.base) as usize))
    }

    pub fn read(&self, addr: u64, out: &mut [u8], access: Access) -> Result<(), MemoryFault> {
        let (i, off) = self.locate(addr, out.len(), access)?;
        out.copy_from_slice(&self.regions[i].bytes[off..off + out.len()]);
        Ok(())
    }

    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), MemoryFault> {
        let (i, off) = self.locate(addr, data.len(), Access::Write)?;
        self.regions[i].bytes[off..off + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Host-side loader write: ignores the write permission (used to place
    /// code into read-only/executable regions). Still bounds-checked.
    pub fn load(&mut self, addr: u64, data: &[u8]) -> Result<(), MemoryFault> {
        let (i, off) = self.locate(addr, data.len(), Access::Read)?;
        self.regions[i].bytes[off..off + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Fetch up to `out.len()` instruction bytes starting at `addr`, stopping at
    /// the end of the containing region. Returns how many bytes were fetched.
    pub fn fetch(&self, addr: u64, out: &mut [u8]) -> Result<usize, MemoryFault> {
        let (i, off) = self.locate(addr, 1, Access::Execute)?;
        let r = &self.regions[i];
        let n = out.len().min(r.bytes.len() - off);
        out[..n].copy_from_slice(&r.bytes[off..off + n]);
        Ok(n)
    }

    /// Host-side read-only view of `len` bytes at `addr`, if fully inside one
    /// region. Used by frontends to present the screen without copying.
    pub fn view(&self, addr: u64, len: usize) -> Option<&[u8]> {
        let (i, off) = self.locate(addr, len, Access::Read).ok()?;
        Some(&self.regions[i].bytes[off..off + len])
    }

    pub fn read_u8(&self, addr: u64) -> Result<u8, MemoryFault> {
        let mut b = [0; 1];
        self.read(addr, &mut b, Access::Read)?;
        Ok(b[0])
    }

    pub fn read_u32(&self, addr: u64) -> Result<u32, MemoryFault> {
        let mut b = [0; 4];
        self.read(addr, &mut b, Access::Read)?;
        Ok(u32::from_le_bytes(b))
    }

    pub fn read_u64(&self, addr: u64) -> Result<u64, MemoryFault> {
        let mut b = [0; 8];
        self.read(addr, &mut b, Access::Read)?;
        Ok(u64::from_le_bytes(b))
    }

    pub fn write_u32(&mut self, addr: u64, v: u32) -> Result<(), MemoryFault> {
        self.write(addr, &v.to_le_bytes())
    }

    pub fn write_u64(&mut self, addr: u64, v: u64) -> Result<(), MemoryFault> {
        self.write(addr, &v.to_le_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_u64_little_endian() {
        let mut m = GuestMemory::new();
        m.map(0x1000, 0x100, Perms::RW).unwrap();
        m.write_u64(0x1008, 0x1122_3344_5566_7788).unwrap();
        assert_eq!(m.read_u64(0x1008).unwrap(), 0x1122_3344_5566_7788);
        assert_eq!(m.read_u8(0x1008).unwrap(), 0x88);
        assert_eq!(m.read_u32(0x100C).unwrap(), 0x1122_3344);
    }

    #[test]
    fn unmapped_access_faults() {
        let m = GuestMemory::new();
        let e = m.read_u64(0x5000).unwrap_err();
        assert_eq!(e.kind, FaultKind::Unmapped);
        assert_eq!(e.access, Access::Read);
    }

    #[test]
    fn access_past_region_end_faults() {
        let mut m = GuestMemory::new();
        m.map(0x1000, 0x10, Perms::RW).unwrap();
        assert!(m.read_u64(0x1008).is_ok());
        assert_eq!(m.read_u64(0x1009).unwrap_err().kind, FaultKind::Unmapped);
    }

    #[test]
    fn write_to_read_only_region_faults() {
        let mut m = GuestMemory::new();
        m.map(0x0, 0x100, Perms::RX).unwrap();
        assert_eq!(m.write_u64(0x10, 1).unwrap_err().kind, FaultKind::Permission);
        // the loader may still place code there
        m.load(0x10, &[0xF4]).unwrap();
        assert_eq!(m.read_u8(0x10).unwrap(), 0xF4);
    }

    #[test]
    fn execute_from_non_exec_region_faults() {
        let mut m = GuestMemory::new();
        m.map(0x0, 0x100, Perms::RW).unwrap();
        let mut buf = [0; 15];
        assert_eq!(m.fetch(0, &mut buf).unwrap_err().kind, FaultKind::Permission);
    }

    #[test]
    fn overlapping_map_rejected() {
        let mut m = GuestMemory::new();
        m.map(0x1000, 0x1000, Perms::RW).unwrap();
        assert!(matches!(m.map(0x1800, 0x1000, Perms::RW), Err(MapError::Overlap { .. })));
        assert!(m.map(0x2000, 0x1000, Perms::RW).is_ok());
    }

    #[test]
    fn fetch_stops_at_region_end() {
        let mut m = GuestMemory::new();
        m.map(0x0, 4, Perms::RX).unwrap();
        let mut buf = [0; 15];
        assert_eq!(m.fetch(1, &mut buf).unwrap(), 3);
    }
}
