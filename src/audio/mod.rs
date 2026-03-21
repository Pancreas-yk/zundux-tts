pub mod effects;
pub mod loudness;
pub mod playback;
pub mod virtual_device;

use anyhow::Result;
use virtual_device::VirtualDevice;

pub struct AudioManager {
    pub virtual_device: VirtualDevice,
}

impl AudioManager {
    pub fn new(device_name: &str) -> Self {
        Self {
            virtual_device: VirtualDevice::new(device_name),
        }
    }

    pub fn ensure_device(&mut self) -> Result<()> {
        self.virtual_device.create()
    }

    pub fn device_exists(&self) -> Result<bool> {
        self.virtual_device.exists()
    }

    pub fn destroy_device(&mut self) -> Result<()> {
        self.virtual_device.destroy()
    }
}
