# ZYMATICA: RAK2287 LoRa Hardware Integration Guide
*IP Class 06/11 | Zymatica Covenant License 2.0 (zymatica.space)*

> *Watermark: ip zymatica.space | astronautshe.com*

---

This document outlines the hardware requirements, initialization sequence, and low-level driver configurations required to establish reliable over-the-air semantic coordinate transmissions using the RAK2287 (SX1302-based) SPI concentrator module.

---

## 1. Hardware Pin Configurations
The RAK2287 concentrator utilizes specific GPIO lines for chip power control and reset timing. Incorrect mappings will cause SPI communication failures. The verified pinouts are:

* **Concentrator Interface:** `/dev/spidev0.0`
* **SX1302 Reset Line:** **GPIO 17** (must be driven low to reset, high for active state)
* **SX1302 Power Enable:** **GPIO 18** (must be driven high to enable radio power)
* **SX1261 Reset Line:** **GPIO 22**
* **AD5338R Reset Line:** **GPIO 13**

### Mandatory Startup Wait Constraint
The Semtech SX1302 concentrator requires time to initialize registers and load external firmware parameters. An explicit **2-second delay** must be executed immediately following the GPIO 17 reset toggle before any SPI calls are made. Probing the bus early will fail to retrieve the correct chip version signature (`0x10`).

---

## 2. Temperature Sensor Patch (libloragw)
By default, the Semtech hardware abstraction library (HAL) assumes the presence of a board-mounted STTS751 temperature sensor and halts execution if the sensor fails to respond during gateway teardown. RAK2287 concentrators do not populate this sensor, leading to a fatal exit code during `lgw_stop()`.

### Patch Logic
To prevent false halts, modify `libloragw/src/loragw_hal.c` to catch and ignore the Linux I2C device close failure (`i2c_linuxdev_close`). 

1. Locate the close routine for the temperature sensor file descriptor (`ts_fd`) inside `lgw_stop()`.
2. Intercept the error condition where `i2c_linuxdev_close(ts_fd) != 0`.
3. Log the occurrence as an optional warning rather than returning a fatal `LGW_HAL_ERROR`.
4. Rebuild the HAL libraries:
   ```bash
   make clean
   make
   ```

---

## 3. Radio Frequency & Transmission Tuning
Dynamic validation sweeps must conform to the working RF chain limitations of the RAK2287 board:

* **Active TX Channel:** **RF Chain 0** (must be specified using the `-c 0` argument in transmission scripts).
* **Disabled TX Channel:** **RF Chain 1** (this channel is hardware-disabled for transmit operations on these boards and will throw a HAL error if engaged).
* **Carrier Frequency:** **903.9 MHz** (select this channel to avoid local frequency offset deafness).
* **Spreading Factor (SF):** **SF7** or **SF9** (verified limits).
* **Signal Bandwidth:** **125 kHz**.
* **RF Power Output:** **14 dBm (25 mW)** (provides optimal local link reliability while remaining fully FCC compliant).

---

## 4. Chip Diagnostics & Verification
Before running high-level coordinate scripts, verify that the concentrator is responsive and has loaded correctly:

```bash
cd ~/sx1302_hal/util_chip_id
./chip_id -d /dev/spidev0.0 -r 1250 -k 0
```

### Expected Diagnostic Output:
Upon success, the tool should output:
```text
Note: chip version is 0x10 (v1.0)
INFO: concentrator EUI: 0x0016C001...
Closing SPI communication interface
WARNING: optional STTS751 temperature sensor close failed; ignored (err=-1)
```
If `ERROR: failed to stop the gateway` appears, re-verify the temperature sensor patch or rebuild the HAL directory.

---

## 5. Low-Level RF Validation Benchmarks
Use the standard HAL diagnostic tools to check link state before starting dynamic coordinate scripts.

### receiver Mode (Miner B / RX Node):
Start the receiver sweep on 903.9 MHz:
```bash
cd ~/sx1302_hal/libloragw
./test_loragw_hal_rx -d /dev/spidev0.0 -r 1250 -a 903.9 -b 903.9 -k 0 -m 1 -j -z 255 -n 1
```

### transmitter Mode (Miner A / TX Node):
Transmit a 10-packet burst from the active TX chain:
```bash
cd ~/sx1302_hal/libloragw
./test_loragw_hal_tx -d /dev/spidev0.0 -k 0 -c 0 -r 1250 -f 903.9 -m LORA -s 7 -b 125 -l 8 -n 10 -z 39 -p 14 -j --pa 1 --pwid 12
```

---

## 6. Important Safety & Operational Constraints
* **Antenna Load Warning:** **Never** trigger the transmit script (`test_loragw_hal_tx` or the Python transmitter) unless a 915 MHz antenna or a 50-ohm dummy load is firmly attached to the active RF SMA port. Operating the transceivers without an antenna will cause signal reflection and permanently damage the power amplifier.
* **Avoid TX Chain 1:** Do not attempts to select chain 1 (`-c 1`) for transmission. Doing so will disable the radio link.
* **Reset GPIO Timing:** Ensure that your startup scripts do not continuously toggle GPIO 17, as this will prevent the SPI bus from stabilizing.
