pub mod effects;
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

    pub fn device_name(&self) -> &str {
        &self.virtual_device.sink_name
    }

    pub fn play_wav(&self, wav_data: Vec<u8>, monitor: bool) -> Result<()> {
        let name = self.virtual_device.sink_name.clone();
        playback::play_wav(wav_data, &name, monitor)
    }
}
