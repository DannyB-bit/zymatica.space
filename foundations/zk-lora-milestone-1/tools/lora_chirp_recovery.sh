#!/usr/bin/env bash
set -u

# LoRa Chirp SX130x concentrator recovery for RAK/Semtech HAL test runs.
# Source this file from RX/TX harnesses, or run it directly:
#   bash tools/lora_chirp_recovery.sh node-b-rx

ROLE="${1:-${ROLE:-unknown-node}}"
SPI_DEV="${SPI_DEV:-/dev/spidev0.0}"
RECOVERY_LOG="${RECOVERY_LOG:-./lora_chirp_recovery_${ROLE}_$(date -u +%Y%m%dT%H%M%SZ).log}"

log() {
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$RECOVERY_LOG"
}

run_logged() {
  log "RUN: $*"
  "$@" >>"$RECOVERY_LOG" 2>&1
  local status=$?
  log "EXIT[$status]: $*"
  return "$status"
}

sudo_logged() {
  if [ "$(id -u)" -eq 0 ]; then
    run_logged "$@"
  else
    run_logged sudo "$@"
  fi
}

service_exists() {
  systemctl list-unit-files "$1.service" >/dev/null 2>&1 ||
    systemctl list-units --all "$1.service" >/dev/null 2>&1
}

stop_conflicting_gateway_services() {
  local services=(
    helium-gateway
    packet-forwarder
    lora_pkt_fwd
    lora-packet-forwarder
    sx1302_hal
    chirpstack-gateway-bridge
    miner
  )

  log "Stopping known gateway services so one process owns the concentrator"
  for svc in "${services[@]}"; do
    if command -v systemctl >/dev/null 2>&1 && service_exists "$svc"; then
      sudo_logged systemctl stop "$svc" || true
      sudo_logged systemctl is-active "$svc" || true
    else
      log "SKIP: service not found: $svc"
    fi
  done

  for pattern in test_loragw_hal_rx test_loragw_hal_tx lora_pkt_fwd packet_forwarder; do
    if pgrep -f "$pattern" >/dev/null 2>&1; then
      log "Stopping stale process pattern: $pattern"
      sudo_logged pkill -f "$pattern" || true
    fi
  done
}

find_reset_script() {
  if [ -n "${RESET_LGW:-}" ] && [ -x "$RESET_LGW" ]; then
    printf '%s\n' "$RESET_LGW"
    return 0
  fi

  local candidates=(
    "$PWD/reset_lgw.sh"
    "$PWD/tools/reset_lgw.sh"
    "$HOME/sx1302_hal/libloragw/reset_lgw.sh"
    "$HOME/sx1302_hal/tools/reset_lgw.sh"
    "$HOME/sx1302_hal/packet_forwarder/reset_lgw.sh"
    "/opt/sx1302_hal/libloragw/reset_lgw.sh"
    "/opt/sx1302_hal/tools/reset_lgw.sh"
  )

  local candidate
  for candidate in "${candidates[@]}"; do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

reset_with_semtech_wrapper() {
  local reset_script
  if ! reset_script="$(find_reset_script)"; then
    log "ERROR: reset_lgw.sh not found. Set RESET_LGW=/path/to/reset_lgw.sh"
    return 10
  fi

  log "Using reset script: $reset_script"
  local reset_dir
  reset_dir="$(dirname "$reset_script")"
  local reset_base
  reset_base="$(basename "$reset_script")"

  pushd "$reset_dir" >/dev/null || return 11
  sudo_logged "./$reset_base" stop || true
  sleep 1
  sudo_logged "./$reset_base" start
  local status=$?
  popd >/dev/null || true
  return "$status"
}

verify_spi_device() {
  if [ ! -e "$SPI_DEV" ]; then
    log "ERROR: SPI device missing: $SPI_DEV"
    return 20
  fi

  log "SPI device exists: $SPI_DEV"
  ls -l "$SPI_DEV" >>"$RECOVERY_LOG" 2>&1 || true
}

verify_chip_id() {
  local tools=(
    "$HOME/sx1302_hal/util_chip_id/util_chip_id"
    "$HOME/sx1302_hal/libloragw/util_chip_id"
    "$HOME/sx1302_hal/libloragw/test_loragw_spi"
    "$PWD/util_chip_id"
    "$PWD/test_loragw_spi"
  )

  local tool
  for tool in "${tools[@]}"; do
    if [ -x "$tool" ]; then
      log "Trying concentrator verification tool: $tool"
      sudo_logged "$tool" -d "$SPI_DEV"
      local status=$?
      if [ "$status" -eq 0 ] && grep -Eiq '0x10|SX1302|SX1303|concentrator' "$RECOVERY_LOG"; then
        log "LORA_CHIRP_RECOVERY_PASS=YES"
        return 0
      fi
      log "Verification tool did not prove a ready concentrator: $tool"
    fi
  done

  log "ERROR: no concentrator verification tool succeeded"
  log "LORA_CHIRP_RECOVERY_PASS=NO"
  return 30
}

lora_chirp_recover() {
  log "LORA_CHIRP_RECOVERY_BEGIN role=$ROLE spi=$SPI_DEV"
  log "RECOVERY_LOG=$RECOVERY_LOG"

  stop_conflicting_gateway_services
  reset_with_semtech_wrapper || return "$?"
  verify_spi_device || return "$?"
  verify_chip_id || return "$?"

  log "LORA_CHIRP_RECOVERY_END role=$ROLE"
  return 0
}

# Backward-compatible alias for older local scripts that sourced the first draft.
helium_concentrator_recover() {
  lora_chirp_recover "$@"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  lora_chirp_recover
fi
