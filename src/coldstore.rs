//! ColdStore — mmap 冷数据存储
//!
//! 每个玩家 128 字节固定槽位, 内存映射文件。

#![allow(dead_code)]

use memmap2::MmapMut;
use std::fs::OpenOptions;

const SLOT_SIZE: usize = 128;
const INITIAL_CAPACITY: usize = 2_000_000; // 200万起步 // 200万起步

// 字段偏移 (手工计算, 紧凑排列)
const O_DFT_SPENT: usize = 0;     // 16
const O_DFT_EARNED: usize = 16;   // 16
const O_PLUNDERED: usize = 32;    // 16
const O_ENERGY_COL: usize = 48;   // 16
const O_REBUILDS: usize = 64;     // 4
const O_REBIRTH: usize = 68;      // 2
const O_GROWTH: usize = 70;       // 4 (f32)
const O_ATK_TODAY: usize = 74;    // 2
const O_AGGR: usize = 76;         // 1
const O_GREED: usize = 77;        // 1
const O_BOLD: usize = 78;         // 1
const O_SOCIAL: usize = 79;       // 1
const O_EMO: usize = 80;          // 1
const O_ANGER: usize = 81;        // 1
const O_FEAR: usize = 82;         // 1
const O_ELATION: usize = 83;      // 1
const O_BOREDOM: usize = 84;      // 1
const O_TILT: usize = 85;         // 1
const O_BURNOUT: usize = 86;      // 1
const O_CWINS: usize = 87;        // 2
const O_CLOSSES: usize = 89;      // 2
const O_DAYS_ATK: usize = 91;     // 4
const O_DAYS_DEATH: usize = 95;   // 4
const O_GEN: usize = 99;          // 4
const O_INVITES: usize = 103;     // 1
const O_INVITED: usize = 104;     // 5 (4 bytes u32 + 1 flag)
const O_REFERRAL: usize = 109;    // 2
const O_IS_ELITE: usize = 111;    // 1
// pad to 128

pub struct ColdStore {
    mmap: MmapMut,
    capacity: usize,
}

impl ColdStore {
    /// 创建/打开冷数据文件
    pub fn new(path: &str, initial_cap: usize) -> std::io::Result<Self> {
        let cap = initial_cap.max(INITIAL_CAPACITY);
        let file_size = cap * SLOT_SIZE;
        let file = OpenOptions::new()
            .read(true).write(true).create(true).truncate(true)
            .open(path)?;
        file.set_len(file_size as u64)?;

        let mmap = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self { mmap, capacity: cap })
    }

    /// 扩容 (复制旧数据)
    pub fn grow(&mut self, new_cap: usize) {
        let new_size = new_cap * SLOT_SIZE;
        let old_size = self.capacity * SLOT_SIZE;

        // 创建新文件
        let path = "/tmp/coldstore.dat"; // 临时路径
        let file = OpenOptions::new()
            .read(true).write(true).create(true)
            .open(path).unwrap();
        file.set_len(new_size as u64).unwrap();

        // 写旧数据
        let mut new_mmap = unsafe { MmapMut::map_mut(&file).unwrap() };
        new_mmap[..old_size].copy_from_slice(&self.mmap[..old_size]);
        // 新区域保持零

        self.mmap = new_mmap;
        self.capacity = new_cap;
    }

    /// 确保能容纳 idx 的玩家
    pub fn ensure(&mut self, idx: u32) {
        let needed = idx as usize + 1;
        if needed > self.capacity {
            let new_cap = (needed * 2).max(INITIAL_CAPACITY);
            self.grow(new_cap);
        }
    }

    fn slot(&self, idx: u32) -> &[u8] {
        let start = (idx as usize) * SLOT_SIZE;
        &self.mmap[start..start + SLOT_SIZE]
    }

    fn slot_mut(&mut self, idx: u32) -> &mut [u8] {
        let start = (idx as usize) * SLOT_SIZE;
        &mut self.mmap[start..start + SLOT_SIZE]
    }

    // ── 读写方法 ──

    // u128 字段
    pub fn dft_spent(&self, idx: u32) -> u128 { self._read_u128(idx, O_DFT_SPENT) }
    pub fn set_dft_spent(&mut self, idx: u32, val: u128) { self._write_u128(idx, O_DFT_SPENT, val); }
    pub fn dft_earned(&self, idx: u32) -> u128 { self._read_u128(idx, O_DFT_EARNED) }
    pub fn set_dft_earned(&mut self, idx: u32, val: u128) { self._write_u128(idx, O_DFT_EARNED, val); }
    pub fn plundered(&self, idx: u32) -> u128 { self._read_u128(idx, O_PLUNDERED) }
    pub fn set_plundered(&mut self, idx: u32, val: u128) { self._write_u128(idx, O_PLUNDERED, val); }
    pub fn energy_collected(&self, idx: u32) -> u128 { self._read_u128(idx, O_ENERGY_COL) }
    pub fn set_energy_collected(&mut self, idx: u32, val: u128) { self._write_u128(idx, O_ENERGY_COL, val); }

    // u32 字段
    pub fn rebuilds(&self, idx: u32) -> u32 { self._read_u32(idx, O_REBUILDS) }
    pub fn set_rebuilds(&mut self, idx: u32, val: u32) { self._write_u32(idx, O_REBUILDS, val); }
    pub fn days_since_attack(&self, idx: u32) -> u32 { self._read_u32(idx, O_DAYS_ATK) }
    pub fn set_days_since_attack(&mut self, idx: u32, val: u32) { self._write_u32(idx, O_DAYS_ATK, val); }
    pub fn days_since_death(&self, idx: u32) -> u32 { self._read_u32(idx, O_DAYS_DEATH) }
    pub fn set_days_since_death(&mut self, idx: u32, val: u32) { self._write_u32(idx, O_DAYS_DEATH, val); }
    pub fn generation(&self, idx: u32) -> u32 { self._read_u32(idx, O_GEN) }
    pub fn set_generation(&mut self, idx: u32, val: u32) { self._write_u32(idx, O_GEN, val); }

    // u16 字段
    pub fn rebirth_count(&self, idx: u32) -> u16 { self._read_u16(idx, O_REBIRTH) }
    pub fn set_rebirth_count(&mut self, idx: u32, val: u16) { self._write_u16(idx, O_REBIRTH, val); }
    pub fn attack_count_today(&self, idx: u32) -> u16 { self._read_u16(idx, O_ATK_TODAY) }
    pub fn set_attack_count_today(&mut self, idx: u32, val: u16) { self._write_u16(idx, O_ATK_TODAY, val); }
    pub fn consecutive_wins(&self, idx: u32) -> u16 { self._read_u16(idx, O_CWINS) }
    pub fn set_consecutive_wins(&mut self, idx: u32, val: u16) { self._write_u16(idx, O_CWINS, val); }
    pub fn consecutive_losses(&self, idx: u32) -> u16 { self._read_u16(idx, O_CLOSSES) }
    pub fn set_consecutive_losses(&mut self, idx: u32, val: u16) { self._write_u16(idx, O_CLOSSES, val); }
    pub fn referral_count(&self, idx: u32) -> u16 { self._read_u16(idx, O_REFERRAL) }
    pub fn set_referral_count(&mut self, idx: u32, val: u16) { self._write_u16(idx, O_REFERRAL, val); }

    // is_elite
    pub fn is_elite(&self, idx: u32) -> bool { self.slot(idx)[O_IS_ELITE] != 0 }
    pub fn set_is_elite(&mut self, idx: u32, val: bool) { self.slot_mut(idx)[O_IS_ELITE] = if val { 1 } else { 0 }; }

    // f32
    pub fn growth_multiplier(&self, idx: u32) -> f32 {
        let s = self.slot(idx); f32::from_le_bytes(s[O_GROWTH..O_GROWTH + 4].try_into().unwrap())
    }
    pub fn set_growth_multiplier(&mut self, idx: u32, val: f32) {
        self.slot_mut(idx)[O_GROWTH..O_GROWTH + 4].copy_from_slice(&val.to_le_bytes());
    }

    // u8 字段
    pub fn aggression(&self, idx: u32) -> u8 { self.slot(idx)[O_AGGR] }
    pub fn set_aggression(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_AGGR] = val; }
    pub fn greed(&self, idx: u32) -> u8 { self.slot(idx)[O_GREED] }
    pub fn set_greed(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_GREED] = val; }
    pub fn boldness(&self, idx: u32) -> u8 { self.slot(idx)[O_BOLD] }
    pub fn set_boldness(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_BOLD] = val; }
    pub fn sociability(&self, idx: u32) -> u8 { self.slot(idx)[O_SOCIAL] }
    pub fn set_sociability(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_SOCIAL] = val; }
    pub fn emotionality(&self, idx: u32) -> u8 { self.slot(idx)[O_EMO] }
    pub fn set_emotionality(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_EMO] = val; }
    pub fn anger(&self, idx: u32) -> u8 { self.slot(idx)[O_ANGER] }
    pub fn set_anger(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_ANGER] = val; }
    pub fn fear(&self, idx: u32) -> u8 { self.slot(idx)[O_FEAR] }
    pub fn set_fear(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_FEAR] = val; }
    pub fn elation(&self, idx: u32) -> u8 { self.slot(idx)[O_ELATION] }
    pub fn set_elation(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_ELATION] = val; }
    pub fn boredom(&self, idx: u32) -> u8 { self.slot(idx)[O_BOREDOM] }
    pub fn set_boredom(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_BOREDOM] = val; }
    pub fn tilt_level(&self, idx: u32) -> u8 { self.slot(idx)[O_TILT] }
    pub fn set_tilt_level(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_TILT] = val; }
    pub fn burnout(&self, idx: u32) -> u8 { self.slot(idx)[O_BURNOUT] }
    pub fn set_burnout(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_BURNOUT] = val; }
    pub fn invites_remaining(&self, idx: u32) -> u8 { self.slot(idx)[O_INVITES] }
    pub fn set_invites_remaining(&mut self, idx: u32, val: u8) { self.slot_mut(idx)[O_INVITES] = val; }

    // Option<u32>
    pub fn invited_by(&self, idx: u32) -> Option<u32> {
        let s = self.slot(idx);
        if s[O_INVITED + 4] == 0 { None }
        else { Some(u32::from_le_bytes(s[O_INVITED..O_INVITED + 4].try_into().unwrap())) }
    }
    pub fn set_invited_by(&mut self, idx: u32, val: Option<u32>) {
        let s = self.slot_mut(idx);
        match val {
            Some(v) => { s[O_INVITED..O_INVITED + 4].copy_from_slice(&v.to_le_bytes()); s[O_INVITED + 4] = 1; }
            None => { s[O_INVITED + 4] = 0; }
        }
    }

    // ── 内部辅助 ──
    fn _read_u128(&self, idx: u32, off: usize) -> u128 {
        let s = self.slot(idx); let mut b = [0u8; 16];
        b.copy_from_slice(&s[off..off + 16]); u128::from_le_bytes(b)
    }
    fn _write_u128(&mut self, idx: u32, off: usize, val: u128) {
        self.slot_mut(idx)[off..off + 16].copy_from_slice(&val.to_le_bytes());
    }
    fn _read_u32(&self, idx: u32, off: usize) -> u32 {
        let s = self.slot(idx); let mut b = [0u8; 4];
        b.copy_from_slice(&s[off..off + 4]); u32::from_le_bytes(b)
    }
    fn _write_u32(&mut self, idx: u32, off: usize, val: u32) {
        self.slot_mut(idx)[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }
    fn _read_u16(&self, idx: u32, off: usize) -> u16 {
        let s = self.slot(idx); let mut b = [0u8; 2];
        b.copy_from_slice(&s[off..off + 2]); u16::from_le_bytes(b)
    }
    fn _write_u16(&mut self, idx: u32, off: usize, val: u16) {
        self.slot_mut(idx)[off..off + 2].copy_from_slice(&val.to_le_bytes());
    }
}
