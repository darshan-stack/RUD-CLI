// Quantum Data Shard (QDS) - Zero-copy shared memory ring buffer abstraction.
// Each shard is a fixed-size memory-mapped region subdivided into equal-sized
// slots that producers write to and consumers read from without data copies.

use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};
use memmap2::MmapMut;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

const MAGIC: u64 = 0x5144535F52554400; // "QDS_RUD\0"
const SLOT_HEADER_SIZE: usize = 16; // seq(8) + len(4) + flags(4)

#[derive(Debug, Clone)]
pub struct QdsConfig {
    pub shard_count: usize,
    pub shard_size_bytes: usize,
    pub slot_size: usize,
    pub base_path: PathBuf,
}

impl QdsConfig {
    pub fn slot_count(&self) -> usize {
        (self.shard_size_bytes - std::mem::size_of::<ShardHeader>()) / self.slot_size
    }
}

#[repr(C)]
struct ShardHeader {
    magic: u64,
    shard_id: u32,
    slot_size: u32,
    slot_count: u32,
    _pad: u32,
    write_seq: AtomicU64,
    read_seq: AtomicU64,
}

pub struct QuantumShard {
    _file_path: PathBuf,
    mmap: Mutex<MmapMut>,
    slot_size: usize,
    slot_count: usize,
}

impl QuantumShard {
    pub fn create(path: &Path, shard_id: u32, size_bytes: usize, slot_size: usize) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .context("failed to open QDS shard file")?;

        file.set_len(size_bytes as u64)
            .context("failed to set shard file size")?;

        let mut mmap = unsafe { MmapMut::map_mut(&file).context("failed to mmap shard")? };

        let header_size = std::mem::size_of::<ShardHeader>();
        let slot_count = (size_bytes - header_size) / slot_size;

        // Write header
        let hdr = mmap.as_mut_ptr() as *mut ShardHeader;
        unsafe {
            (*hdr).magic = MAGIC;
            (*hdr).shard_id = shard_id;
            (*hdr).slot_size = slot_size as u32;
            (*hdr).slot_count = slot_count as u32;
        }
        mmap.flush().context("failed to flush shard header")?;

        debug!(shard_id, slot_count, slot_size, "QDS shard created");

        Ok(Self {
            _file_path: path.to_path_buf(),
            mmap: Mutex::new(mmap),
            slot_size,
            slot_count,
        })
    }

    pub fn write(&self, data: &[u8]) -> Result<u64> {
        let mut mmap = self.mmap.lock();
        let hdr = mmap.as_mut_ptr() as *mut ShardHeader;

        let seq = unsafe { (*hdr).write_seq.fetch_add(1, Ordering::SeqCst) };
        let slot_idx = (seq as usize) % self.slot_count;

        let header_size = std::mem::size_of::<ShardHeader>();
        let slot_start = header_size + slot_idx * self.slot_size;

        let payload_len = data.len().min(self.slot_size - SLOT_HEADER_SIZE);

        // Write slot header: [seq(8)][len(4)][flags(4)]
        let slot = &mut mmap[slot_start..slot_start + self.slot_size];
        slot[..8].copy_from_slice(&seq.to_le_bytes());
        slot[8..12].copy_from_slice(&(payload_len as u32).to_le_bytes());
        slot[12..16].copy_from_slice(&0u32.to_le_bytes()); // flags
        slot[SLOT_HEADER_SIZE..SLOT_HEADER_SIZE + payload_len].copy_from_slice(&data[..payload_len]);

        Ok(seq)
    }

    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    pub fn slot_size(&self) -> usize {
        self.slot_size
    }
}

pub struct QdsFabric {
    shards: Vec<QuantumShard>,
    config: QdsConfig,
}

impl QdsFabric {
    pub fn initialize(config: QdsConfig) -> Result<Self> {
        std::fs::create_dir_all(&config.base_path)
            .context("failed to create QDS base path")?;

        let mut shards = Vec::with_capacity(config.shard_count);
        for i in 0..config.shard_count {
            let path = config.base_path.join(format!("shard_{:04}.qds", i));
            let shard = QuantumShard::create(
                &path,
                i as u32,
                config.shard_size_bytes,
                config.slot_size,
            )?;
            shards.push(shard);
        }

        info!(
            shard_count = config.shard_count,
            shard_size_mb = config.shard_size_bytes / (1024 * 1024),
            "QDS fabric initialized"
        );

        Ok(Self { shards, config })
    }

    pub fn route_write(&self, key: u64, data: &[u8]) -> Result<u64> {
        let shard_idx = (key as usize) % self.shards.len();
        self.shards[shard_idx].write(data)
    }

    pub fn shard_count(&self) -> usize {
        self.config.shard_count
    }

    pub fn total_capacity_mb(&self) -> usize {
        (self.config.shard_count * self.config.shard_size_bytes) / (1024 * 1024)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdsStats {
    pub shard_count: usize,
    pub slot_size: usize,
    pub slots_per_shard: usize,
    pub total_capacity_mb: usize,
}

impl QdsFabric {
    pub fn stats(&self) -> QdsStats {
        QdsStats {
            shard_count: self.config.shard_count,
            slot_size: self.config.slot_size,
            slots_per_shard: self.config.slot_count(),
            total_capacity_mb: self.total_capacity_mb(),
        }
    }
}
