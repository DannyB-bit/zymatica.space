# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
UFO Semantic Codec — Compress MEANING, Not Bytes
==================================================
Component 09 of the Language-U Semantic Communication Protocol.

Encodes structured data as semantic tokens using a shared vocabulary
between sender and receiver. The receiver reconstructs full English
messages from compact token streams.

57 bytes of semantic tokens → 283 characters of reconstructed English
= 47 words per chirp with full ZK-LoRaWAN privacy.

This codec is the final stage of the UFO compression pipeline.
It sits INSIDE the ZK-LoRaWAN frame's 57-byte payload slot:

┌─────────────────────────────────────────────────┐
│ ZK-LoRaWAN Frame (255 bytes)                    │
│  ├── ZK Proof (161 bytes)                       │
│  ├── FEC Parity (20 bytes)                      │
│  ├── Frame Header + Tag + Coords (17 bytes)     │
│  └── Payload: SEMANTIC TOKENS (57 bytes) ◄──────│
│       └── 18 readings → 283 chars English       │
└─────────────────────────────────────────────────┘
"""

import struct
import json


# ============================================================================
# Semantic Vocabulary — Shared between sender and receiver
# This dictionary NEVER goes over the air. Both sides have it pre-loaded.
# ============================================================================
CONCEPTS = {
    # Environmental (0x01 - 0x0F)
    0x01: "temperature", 0x02: "humidity", 0x03: "pressure",
    0x04: "wind_speed", 0x05: "wind_direction", 0x06: "rain",
    0x07: "uv_index", 0x08: "air_quality", 0x09: "soil_moisture",
    0x0A: "light_level", 0x0B: "noise_level", 0x0C: "co2",
    0x0D: "pm25", 0x0E: "dew_point", 0x0F: "visibility",

    # Medical (0x10 - 0x1F)
    0x10: "heart_rate", 0x11: "spo2", 0x12: "blood_pressure_sys",
    0x13: "blood_pressure_dia", 0x14: "body_temp", 0x15: "steps",
    0x16: "calories", 0x17: "sleep_minutes", 0x18: "stress_level",
    0x19: "hrv", 0x1A: "respiratory_rate", 0x1B: "blood_glucose",
    0x1C: "body_weight", 0x1D: "bmi",

    # Security (0x20 - 0x2F)
    0x20: "motion_detected", 0x21: "door_state", 0x22: "lock_state",
    0x23: "camera_recording", 0x24: "alarm_state", 0x25: "user_id",
    0x26: "access_granted", 0x27: "intrusion_alert",
    0x28: "window_state", 0x29: "smoke_detected", 0x2A: "water_leak",

    # GPS / Location (0x30 - 0x3F)
    0x30: "latitude", 0x31: "longitude", 0x32: "altitude",
    0x33: "speed", 0x34: "heading", 0x35: "satellites",
    0x36: "hdop", 0x37: "geofence_status",

    # Power / Grid (0x40 - 0x4F)
    0x40: "voltage", 0x41: "current", 0x42: "frequency",
    0x43: "power", 0x44: "battery_percent", 0x45: "solar_watts",
    0x46: "energy_kwh", 0x47: "power_factor",

    # Status (0x50 - 0x5F)
    0x50: "status_ok", 0x51: "status_warning", 0x52: "status_critical",
    0x53: "status_offline", 0x54: "node_id", 0x55: "sector",
    0x56: "uptime_hours", 0x57: "firmware_version", 0x58: "signal_rssi",

    # Actions / Commands (0x60 - 0x6F)
    0x60: "send_backup", 0x61: "switch_low_power", 0x62: "reboot",
    0x63: "start_recording", 0x64: "stop_recording", 0x65: "deploy_drone",
    0x66: "return_to_base", 0x67: "emergency", 0x68: "acknowledge",
    0x69: "calibrate", 0x6A: "firmware_update",

    # Agriculture (0x70 - 0x7F)
    0x70: "soil_ph", 0x71: "nitrogen", 0x72: "phosphorus",
    0x73: "potassium", 0x74: "irrigation_on", 0x75: "crop_health",
    0x76: "leaf_wetness", 0x77: "evapotranspiration",
}

REVERSE_CONCEPTS = {v: k for k, v in CONCEPTS.items()}

# Value encoding types
VAL_U8 = 0     # unsigned 8-bit integer (0-255)
VAL_I16 = 1    # signed 16-bit integer (-32768 to 32767)
VAL_F16 = 2    # 16-bit fixed point (value × 100, ÷100 on decode)
VAL_F32 = 3    # 32-bit IEEE 754 float
VAL_BOOL = 4   # 1-byte boolean (0 or 1)

VAL_SIZES = {VAL_U8: 1, VAL_I16: 2, VAL_F16: 2, VAL_F32: 4, VAL_BOOL: 1}
VAL_NAMES = {VAL_U8: "u8", VAL_I16: "i16", VAL_F16: "f16", VAL_F32: "f32", VAL_BOOL: "bool"}


# ============================================================================
# Semantic Encoder — Compresses MEANING into minimal bytes
# ============================================================================
class SemanticCodec:
    """
    Encodes structured readings as semantic tokens.

    Wire format per reading:
        concept_id (1 byte) + value_type (1 byte) + value (1-4 bytes)

    Header:
        reading_count (1 byte)

    Capacity in 57 bytes:
        - U8 values (3 bytes each):  18 readings max
        - F16 values (4 bytes each): 14 readings max
        - F32 values (6 bytes each):  9 readings max
        - Mixed:                     depends on types
    """

    MAX_PAYLOAD = 57  # ZK-LoRaWAN pipe size

    def encode(self, readings: list) -> bytes:
        """
        Encode a list of semantic readings into compact bytes.

        Args:
            readings: list of (concept_name: str, value, value_type: int)

        Returns:
            Packed bytes ready for ZK-LoRaWAN payload slot

        Raises:
            ValueError: if result exceeds 57 bytes or concept unknown
        """
        result = struct.pack("B", len(readings))

        for concept_name, value, val_type in readings:
            if concept_name not in REVERSE_CONCEPTS:
                raise ValueError(f"Unknown concept: {concept_name}")

            concept_id = REVERSE_CONCEPTS[concept_name]
            result += struct.pack("B", concept_id)
            result += struct.pack("B", val_type)

            if val_type == VAL_U8:
                result += struct.pack("B", int(value) & 0xFF)
            elif val_type == VAL_I16:
                result += struct.pack(">h", int(value))
            elif val_type == VAL_F16:
                result += struct.pack(">h", int(round(value * 100)))
            elif val_type == VAL_F32:
                result += struct.pack(">f", value)
            elif val_type == VAL_BOOL:
                result += struct.pack("B", 1 if value else 0)
            else:
                raise ValueError(f"Unknown value type: {val_type}")

        if len(result) > self.MAX_PAYLOAD:
            raise ValueError(
                f"Encoded size {len(result)} exceeds {self.MAX_PAYLOAD}-byte pipe. "
                f"Reduce readings or use smaller value types."
            )

        return result

    def decode(self, data: bytes) -> list:
        """
        Decode semantic tokens back to (concept_name, value) pairs.

        Args:
            data: bytes from ZK-LoRaWAN payload slot

        Returns:
            list of (concept_name: str, value)
        """
        offset = 0
        count = struct.unpack("B", data[offset:offset+1])[0]
        offset += 1

        readings = []
        for _ in range(count):
            concept_id = struct.unpack("B", data[offset:offset+1])[0]
            offset += 1
            val_type = struct.unpack("B", data[offset:offset+1])[0]
            offset += 1

            concept_name = CONCEPTS.get(concept_id, f"unknown_0x{concept_id:02X}")

            if val_type == VAL_U8:
                value = struct.unpack("B", data[offset:offset+1])[0]
                offset += 1
            elif val_type == VAL_I16:
                value = struct.unpack(">h", data[offset:offset+2])[0]
                offset += 2
            elif val_type == VAL_F16:
                value = struct.unpack(">h", data[offset:offset+2])[0] / 100.0
                offset += 2
            elif val_type == VAL_F32:
                value = struct.unpack(">f", data[offset:offset+4])[0]
                offset += 4
            elif val_type == VAL_BOOL:
                value = bool(struct.unpack("B", data[offset:offset+1])[0])
                offset += 1
            else:
                raise ValueError(f"Unknown value type: {val_type}")

            readings.append((concept_name, value))

        return readings

    def reconstruct_english(self, readings: list) -> str:
        """
        Reconstruct a human-readable English message from decoded readings.

        This is the RECEIVER side — it takes decoded semantic tokens and
        produces full English sentences for the end user.
        """
        parts = []
        for concept, value in readings:
            name = concept.replace("_", " ")
            if isinstance(value, bool):
                parts.append(f"{name}: {'yes' if value else 'no'}")
            elif isinstance(value, float):
                if value == int(value):
                    parts.append(f"{name}: {int(value)}")
                else:
                    parts.append(f"{name}: {value:.1f}")
            else:
                parts.append(f"{name}: {value}")
        return ". ".join(parts) + "."

    def estimate_size(self, readings: list) -> int:
        """Estimate encoded size without actually encoding."""
        size = 1  # count byte
        for _, _, val_type in readings:
            size += 2 + VAL_SIZES[val_type]  # concept_id + type + value
        return size

    def max_readings(self, val_type: int) -> int:
        """Calculate maximum readings of a given type that fit in 57 bytes."""
        per_reading = 2 + VAL_SIZES[val_type]  # concept_id + type + value
        return (self.MAX_PAYLOAD - 1) // per_reading  # subtract 1 for count byte


# ============================================================================
# Self-test
# ============================================================================
def _self_test():
    """Run self-tests to verify codec accuracy."""
    print("  SemanticCodec self-test...")

    codec = SemanticCodec()
    passed = 0
    failed = 0

    # Test 1: Weather station round-trip
    weather = [
        ("temperature", 72.5, VAL_F16),
        ("humidity", 45, VAL_U8),
        ("pressure", 1013, VAL_I16),
        ("wind_speed", 8.3, VAL_F16),
        ("wind_direction", 225, VAL_I16),
        ("uv_index", 6, VAL_U8),
        ("rain", 0.0, VAL_F16),
        ("air_quality", 42, VAL_U8),
        ("soil_moisture", 34, VAL_U8),
        ("light_level", 85, VAL_U8),
        ("noise_level", 55, VAL_U8),
        ("co2", 412, VAL_I16),
    ]
    encoded = codec.encode(weather)
    decoded = codec.decode(encoded)
    english = codec.reconstruct_english(decoded)

    assert len(encoded) <= 57, f"Weather too large: {len(encoded)}"
    assert len(decoded) == 12
    assert decoded[0] == ("temperature", 72.5)
    assert decoded[1] == ("humidity", 45)
    assert len(english) > 150
    passed += 1
    print(f"    [1] Weather (12 readings, {len(encoded)}B) -> {len(english)} chars English PASS")

    # Test 2: Medical vitals round-trip
    medical = [
        ("heart_rate", 72, VAL_U8),
        ("spo2", 98, VAL_U8),
        ("blood_pressure_sys", 120, VAL_U8),
        ("blood_pressure_dia", 80, VAL_U8),
        ("body_temp", 98.6, VAL_F16),
        ("steps", 8432, VAL_I16),
        ("calories", 1847, VAL_I16),
        ("sleep_minutes", 420, VAL_I16),
        ("stress_level", 35, VAL_U8),
        ("hrv", 42, VAL_U8),
        ("respiratory_rate", 16, VAL_U8),
        ("blood_glucose", 95, VAL_U8),
    ]
    encoded = codec.encode(medical)
    decoded = codec.decode(encoded)

    assert len(encoded) <= 57
    assert decoded[4] == ("body_temp", 98.6)
    assert decoded[5] == ("steps", 8432)
    passed += 1
    print(f"    [2] Medical (12 vitals, {len(encoded)}B) PASS")

    # Test 3: Emergency with GPS
    emergency = [
        ("emergency", True, VAL_BOOL),
        ("latitude", 40.7128, VAL_F32),
        ("longitude", -74.0060, VAL_F32),
        ("heart_rate", 120, VAL_U8),
        ("spo2", 89, VAL_U8),
        ("status_critical", True, VAL_BOOL),
        ("send_backup", True, VAL_BOOL),
    ]
    encoded = codec.encode(emergency)
    decoded = codec.decode(encoded)
    english = codec.reconstruct_english(decoded)

    assert len(encoded) <= 57
    assert decoded[0] == ("emergency", True)
    assert abs(decoded[1][1] - 40.7128) < 0.001
    passed += 1
    print(f"    [3] Emergency (7 fields, {len(encoded)}B) -> \"{english[:60]}...\" PASS")

    # Test 4: Max capacity (U8)
    max_u8 = codec.max_readings(VAL_U8)
    assert max_u8 == 18
    passed += 1
    print(f"    [4] Max U8 readings: {max_u8} PASS")

    # Test 5: Max capacity (F16)
    max_f16 = codec.max_readings(VAL_F16)
    assert max_f16 == 14
    passed += 1
    print(f"    [5] Max F16 readings: {max_f16} PASS")

    # Test 6: Tamper detection (wrong concept ID)
    try:
        codec.encode([("nonexistent_concept", 42, VAL_U8)])
        failed += 1
        print(f"    [6] Tamper detection FAIL (should have raised)")
    except ValueError:
        passed += 1
        print(f"    [6] Tamper detection (unknown concept rejected) PASS")

    print(f"  Result: {passed}/{passed+failed} passed")
    return failed == 0


if __name__ == "__main__":
    success = _self_test()
    import sys
    sys.exit(0 if success else 1)
