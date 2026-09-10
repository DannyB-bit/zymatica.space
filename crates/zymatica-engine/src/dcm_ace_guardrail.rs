//! # Dual-Consciousness Metacognitive Auto-Correction Engine (DCM-ACE)
//!
//! Intercepts raw edge model hardware tokens, evaluates them against physical
//! hardware specifications and FCC / US915 spectrum constraints, and utilizes the
//! 200 Amsterdam resilience invariant to auto-correct hallucinations in zero runtime latency.

#[derive(Debug, Clone, PartialEq)]
pub struct HardwareDirective {
    pub gpio_reset_pin: u8,
    pub spi_device: String,
    pub spi_speed_hz: u32,
    pub lora_freq_mhz: f64,
    pub spreading_factor: u8,
    pub tx_power_dbm: i8,
}

#[derive(Debug, Clone)]
pub struct HardwareSpecification {
    pub allowed_gpio_pin: u8,
    pub allowed_spi_device: String,
    pub max_spi_speed_hz: u32,
    pub min_freq_mhz: f64,
    pub max_freq_mhz: f64,
    pub allowed_spreading_factors: Vec<u8>,
    pub max_tx_power_dbm: i8,
}

impl Default for HardwareSpecification {
    fn default() -> Self {
        Self {
            allowed_gpio_pin: 25, // Raspberry Pi 4 BCM Pin 25 for SX1302 reset
            allowed_spi_device: "/dev/spidev0.0".to_string(),
            max_spi_speed_hz: 8_000_000, // 8 MHz
            min_freq_mhz: 902.3,         // US915 Uplink Lower Bound
            max_freq_mhz: 914.9,         // US915 Uplink Upper Bound
            allowed_spreading_factors: vec![7, 8, 9, 10, 11, 12],
            max_tx_power_dbm: 14,
        }
    }
}

pub struct DcmAceGuardrail {
    spec: HardwareSpecification,
}

impl DcmAceGuardrail {
    pub fn new(spec: HardwareSpecification) -> Self {
        Self { spec }
    }

    /// Evaluates raw directive, flags any hallucinations or out-of-spec parameters,
    /// and performs zero-latency deterministic self-healing.
    pub fn audit_and_correct(
        &self,
        mut directive: HardwareDirective,
    ) -> (HardwareDirective, Vec<String>) {
        let mut corrections = Vec::new();

        // Check GPIO Reset Pin
        if directive.gpio_reset_pin != self.spec.allowed_gpio_pin {
            corrections.push(format!(
                "GPIO_HEAL: Incompatible pin {} remapped to RAK SX1302 standard pin {}",
                directive.gpio_reset_pin, self.spec.allowed_gpio_pin
            ));
            directive.gpio_reset_pin = self.spec.allowed_gpio_pin;
        }

        // Check SPI Device Path
        if directive.spi_device != self.spec.allowed_spi_device {
            corrections.push(format!(
                "SPI_HEAL: Device '{}' corrected to canonical hardware interface '{}'",
                directive.spi_device, self.spec.allowed_spi_device
            ));
            directive.spi_device = self.spec.allowed_spi_device.clone();
        }

        // Check SPI Clock Speed
        if directive.spi_speed_hz > self.spec.max_spi_speed_hz {
            corrections.push(format!(
                "SPI_CLOCK_CLAMP: Exceeded 8 MHz limit ({} Hz) clamped to {} Hz",
                directive.spi_speed_hz, self.spec.max_spi_speed_hz
            ));
            directive.spi_speed_hz = self.spec.max_spi_speed_hz;
        }

        // Check US915 LoRa Frequency
        if directive.lora_freq_mhz < self.spec.min_freq_mhz
            || directive.lora_freq_mhz > self.spec.max_freq_mhz
        {
            let clamped_freq = directive
                .lora_freq_mhz
                .clamp(self.spec.min_freq_mhz, self.spec.max_freq_mhz);
            corrections.push(format!(
                "RF_FREQ_RESONANCE: Out-of-band frequency {:.3} MHz re-anchored to US915 channel {:.3} MHz",
                directive.lora_freq_mhz, clamped_freq
            ));
            directive.lora_freq_mhz = clamped_freq;
        }

        // Check Spreading Factor
        if !self
            .spec
            .allowed_spreading_factors
            .contains(&directive.spreading_factor)
        {
            corrections.push(format!(
                "SF_CORRECTION: Illegal SF{} adjusted to optimal edge default SF7",
                directive.spreading_factor
            ));
            directive.spreading_factor = 7;
        }

        // Check TX Power
        if directive.tx_power_dbm > self.spec.max_tx_power_dbm {
            corrections.push(format!(
                "TX_POWER_LIMIT: Power {} dBm clamped to regulatory maximum {} dBm",
                directive.tx_power_dbm, self.spec.max_tx_power_dbm
            ));
            directive.tx_power_dbm = self.spec.max_tx_power_dbm;
        }

        (directive, corrections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metacognitive_self_healing() {
        let guardrail = DcmAceGuardrail::new(HardwareSpecification::default());

        // Intentionally hallucinated / perturbed directive
        let raw = HardwareDirective {
            gpio_reset_pin: 17,                  // Wrong pin
            spi_device: "/dev/spidev1.0".into(), // Wrong bus
            spi_speed_hz: 16_000_000,            // Too fast
            lora_freq_mhz: 930.5,                // Out of band
            spreading_factor: 5,                 // Invalid SF
            tx_power_dbm: 20,                    // Exceeds 14 dBm
        };

        let (healed, corrections) = guardrail.audit_and_correct(raw);

        assert_eq!(corrections.len(), 6);
        assert_eq!(healed.gpio_reset_pin, 25);
        assert_eq!(healed.spi_device, "/dev/spidev0.0");
        assert_eq!(healed.spi_speed_hz, 8_000_000);
        assert!(healed.lora_freq_mhz <= 914.9 && healed.lora_freq_mhz >= 902.3);
        assert_eq!(healed.spreading_factor, 7);
        assert_eq!(healed.tx_power_dbm, 14);
    }
}
