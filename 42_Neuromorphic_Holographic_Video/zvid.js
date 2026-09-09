/**
 * ============================================================================
 * zvid.js — Zero-Dependency .ZVID Neuromorphic Super-Resolution Video Engine (v5.2.0)
 * Language-U Foundational Suite • Class 42: Neuromorphic Holographic Video
 * Author: zymatica.space | Architect: Devs One
 * License: Zymatica Covenant License 2026 (Source-Available)
 * ============================================================================
 * 
 * Features:
 *   - Native 720p transmission stream (252 KB) upscaled to 1080p / 4K UHD in real-time
 *   - Audio Mute / Unmute controls with volume state sync
 *   - Three Vertical Dots (⋮) Overflow Settings Menu:
 *       • Download (.zvid stream capsule)
 *       • Playback Speed (0.5x, 0.75x, Normal 1.0x, 1.25x, 1.5x, 2.0x)
 *       • Quality / Super-Resolution Submenu (All 5 Modes: FSR 4K, Bilateral 4K, Auto 4K, 720p, A/B Split)
 *       • Picture in Picture (PiP)
 *   - On-Video Timeline Scrubber, Play/Pause Center Flash, Fullscreen
 *   - Interactive A/B Split-Screen Slider with Emerald Laser Divider
 *   - Pure WebGL2/WebGL shader pipeline with automatic 2D canvas fallback
 */

(function () {
  'use strict';

  function formatTime(seconds) {
    if (isNaN(seconds) || seconds < 0) return '0:00';
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  }

  function roundNum(num, decimals) {
    const factor = Math.pow(10, decimals);
    return Math.round(num * factor) / factor;
  }

  class ZVideoElement extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' });

      // State
      this._src = '';
      this._isPlaying = false;
      this._isLoaded = false;
      this._duration = 0;
      this._currentTime = 0;
      this._fps = 24.0;
      this._totalFrames = 0;
      this._width = 1280;
      this._height = 720;
      this._scanlines = false;
      this._isSeeking = false;
      this._hideTimer = null;
      this._isMuted = true;
      this._playbackRate = 1.0;
      this._menuOpen = false;

      // Super-Resolution State
      this._upscaleMode = 'easu'; // Default: EASU FSR 4K
      this._modeInt = 1;
      this._splitX = 0.5;

      // Dense Stream & Anchors
      this._frames = [];
      this._anchors = [];
      this._trajectory = [];
      this._audio = null;
      this._videoSource = null;

      this._animFrameId = null;
      this._lastTimestamp = performance.now();

      this._buildUI();
    }

    static get observedAttributes() {
      return ['src', 'autoplay', 'loop', 'scanlines', 'muted', 'upscale'];
    }

    attributeChangedCallback(name, oldVal, newVal) {
      if (oldVal === newVal) return;
      if (name === 'src' && newVal) {
        this._src = newVal;
        this.load(newVal);
      }
      if (name === 'scanlines') {
        this.toggleScanlines(this.hasAttribute('scanlines'));
      }
      if (name === 'muted') {
        this.toggleMute(this.hasAttribute('muted'));
      }
      if (name === 'upscale' && newVal) {
        this.setUpscaleMode(newVal);
      }
    }

    connectedCallback() {
      if (this.getAttribute('src') && !this._isLoaded) {
        this.load(this.getAttribute('src'));
      }
      if (this.hasAttribute('upscale')) {
        this.setUpscaleMode(this.getAttribute('upscale'));
      }
      if (this.hasAttribute('muted')) {
        this.toggleMute(true);
      }
      this._setupKeyboardEvents();
    }

    disconnectedCallback() {
      if (this._animFrameId) {
        cancelAnimationFrame(this._animFrameId);
      }
      if (this._audio) {
        this._audio.pause();
      }
      if (this._hideTimer) {
        clearTimeout(this._hideTimer);
      }
    }

    _buildUI() {
      const style = document.createElement('style');
      style.textContent = `
        :host {
          display: block;
          position: relative;
          width: 100%;
          height: 100%;
          background: #000000;
          overflow: hidden;
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
          user-select: none;
          -webkit-user-select: none;
          outline: none;
        }

        :host(:fullscreen),
        :host(:-webkit-full-screen) {
          width: 100vw !important;
          height: 100vh !important;
          max-width: 100vw !important;
          max-height: 100vh !important;
          background: #000000 !important;
        }

        :host(:fullscreen) .player-viewport,
        :host(:-webkit-full-screen) .player-viewport {
          width: 100vw !important;
          height: 100vh !important;
        }

        :host(:fullscreen) canvas,
        :host(:-webkit-full-screen) canvas {
          width: 100vw !important;
          height: 100vh !important;
          max-width: 100vw !important;
          max-height: 100vh !important;
          object-fit: contain !important;
        }

        .player-viewport {
          position: relative;
          width: 100%;
          height: 100%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: #000000;
          overflow: hidden;
        }

        .player-viewport.hide-cursor {
          cursor: none;
        }

        canvas {
          max-width: 100%;
          max-height: 100%;
          aspect-ratio: 16 / 9;
          display: block;
          object-fit: contain;
          background: #000000;
        }

        .scanlines {
          position: absolute;
          inset: 0;
          background: linear-gradient(rgba(18, 16, 16, 0) 50%, rgba(0, 0, 0, 0.4) 50%),
                      linear-gradient(90deg, rgba(255, 0, 0, 0.03), rgba(0, 255, 0, 0.01), rgba(0, 0, 255, 0.03));
          background-size: 100% 4px, 6px 100%;
          pointer-events: none;
          opacity: 0;
          transition: opacity 0.3s ease;
        }

        .scanlines.active {
          opacity: 0.75;
        }

        /* Center Play/Pause Indicator */
        .center-indicator {
          position: absolute;
          top: 50%;
          left: 50%;
          transform: translate(-50%, -50%) scale(0.85);
          width: 72px;
          height: 72px;
          border-radius: 50%;
          background: rgba(15, 23, 42, 0.75);
          backdrop-filter: blur(8px);
          -webkit-backdrop-filter: blur(8px);
          border: 1px solid rgba(255, 255, 255, 0.2);
          display: flex;
          align-items: center;
          justify-content: center;
          color: #ffffff;
          opacity: 0;
          pointer-events: none;
          transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.25s ease;
          box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
        }

        .center-indicator.flash {
          opacity: 1;
          transform: translate(-50%, -50%) scale(1.1);
        }

        /* Floating Split Indicator */
        .split-indicator {
          position: absolute;
          top: 20px;
          left: 50%;
          transform: translateX(-50%);
          background: rgba(10, 15, 25, 0.88);
          border: 1px solid rgba(0, 255, 102, 0.4);
          border-radius: 20px;
          padding: 6px 18px;
          font-size: 11px;
          font-weight: 700;
          letter-spacing: 0.8px;
          color: #ffffff;
          pointer-events: none;
          backdrop-filter: blur(12px);
          -webkit-backdrop-filter: blur(12px);
          z-index: 8;
          display: flex;
          align-items: center;
          gap: 12px;
          box-shadow: 0 4px 24px rgba(0, 0, 0, 0.6);
          animation: pulseGlow 2.5s infinite alternate;
        }

        @keyframes pulseGlow {
          0% { box-shadow: 0 4px 20px rgba(0, 0, 0, 0.6); }
          100% { box-shadow: 0 4px 25px rgba(0, 255, 102, 0.3); }
        }

        .split-indicator .left-label {
          color: #94a3b8;
        }

        .split-indicator .divider-icon {
          color: #00ff66;
          font-size: 11px;
        }

        .split-indicator .right-label {
          color: #00ff66;
        }

        /* On-Video Native Controls Bar */
        .controls-overlay {
          position: absolute;
          bottom: 0;
          left: 0;
          right: 0;
          padding: 16px 20px 14px 20px;
          background: linear-gradient(180deg, transparent 0%, rgba(0, 0, 0, 0.45) 30%, rgba(0, 0, 0, 0.92) 100%);
          display: flex;
          flex-direction: column;
          gap: 10px;
          opacity: 0;
          pointer-events: none;
          transform: translateY(6px);
          transition: opacity 0.25s ease, transform 0.25s ease;
          z-index: 10;
        }

        .controls-overlay.visible {
          opacity: 1;
          pointer-events: auto;
          transform: translateY(0);
        }

        /* Timeline Scrubber */
        .scrubber-bar {
          position: relative;
          width: 100%;
          height: 16px;
          display: flex;
          align-items: center;
          cursor: pointer;
        }

        .scrubber-track {
          position: relative;
          width: 100%;
          height: 4px;
          background: rgba(255, 255, 255, 0.25);
          border-radius: 3px;
          overflow: visible;
          transition: height 0.15s ease;
        }

        .scrubber-bar:hover .scrubber-track,
        .scrubber-bar.seeking .scrubber-track {
          height: 6px;
        }

        .scrubber-fill {
          position: absolute;
          left: 0;
          top: 0;
          bottom: 0;
          width: 0%;
          background: linear-gradient(90deg, #00f0ff 0%, #00ff66 100%);
          border-radius: 3px;
          pointer-events: none;
        }

        .scrubber-thumb {
          position: absolute;
          top: 50%;
          left: 0%;
          transform: translate(-50%, -50%) scale(0);
          width: 13px;
          height: 13px;
          border-radius: 50%;
          background: #ffffff;
          box-shadow: 0 0 8px rgba(0, 255, 102, 0.8);
          pointer-events: none;
          transition: transform 0.15s ease;
        }

        .scrubber-bar:hover .scrubber-thumb,
        .scrubber-bar.seeking .scrubber-thumb {
          transform: translate(-50%, -50%) scale(1);
        }

        /* Controls Row */
        .controls-row {
          display: flex;
          align-items: center;
          gap: 12px;
          width: 100%;
          flex-wrap: nowrap;
        }

        .ctrl-btn {
          background: transparent;
          border: none;
          color: #ffffff;
          width: 36px;
          height: 36px;
          padding: 0;
          cursor: pointer;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: 6px;
          transition: background 0.18s ease, transform 0.12s ease;
          flex-shrink: 0;
        }

        .ctrl-btn:hover {
          background: rgba(255, 255, 255, 0.15);
          transform: scale(1.08);
        }

        .ctrl-btn:active {
          transform: scale(0.94);
        }

        .ctrl-btn svg {
          display: block;
        }

        .time-display {
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace;
          font-size: 13px;
          font-weight: 500;
          color: #f1f5f9;
          letter-spacing: 0.3px;
          min-width: 85px;
          flex-shrink: 0;
        }

        .time-display .current {
          color: #ffffff;
          font-weight: 600;
        }

        .time-display .separator {
          color: #64748b;
          margin: 0 4px;
        }

        .time-display .duration {
          color: #94a3b8;
        }

        /* Quick Upscale Mode Switcher Pill Group */
        .upscale-pill-group {
          display: flex;
          align-items: center;
          background: rgba(255, 255, 255, 0.08);
          border: 1px solid rgba(255, 255, 255, 0.16);
          border-radius: 7px;
          padding: 2px;
          gap: 2px;
          backdrop-filter: blur(8px);
          -webkit-backdrop-filter: blur(8px);
          flex-shrink: 0;
        }

        .pill-btn {
          background: transparent;
          border: none;
          color: #94a3b8;
          font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
          font-size: 11px;
          font-weight: 600;
          padding: 3px 8px;
          border-radius: 5px;
          cursor: pointer;
          transition: all 0.18s ease;
          white-space: nowrap;
        }

        .pill-btn:hover {
          color: #ffffff;
          background: rgba(255, 255, 255, 0.12);
        }

        .pill-btn.active {
          color: #000000;
          background: #00ff66;
          font-weight: 700;
          box-shadow: 0 0 10px rgba(0, 255, 102, 0.4);
        }

        .spacer {
          flex: 1;
        }

        .format-badge {
          font-size: 10px;
          font-weight: 700;
          letter-spacing: 0.8px;
          color: #00ff66;
          background: rgba(0, 255, 102, 0.1);
          border: 1px solid rgba(0, 255, 102, 0.28);
          padding: 3px 8px;
          border-radius: 4px;
          white-space: nowrap;
          flex-shrink: 0;
        }

        /* Three-Dots Floating Popup Menu (Matches Screenshot 3) */
        .more-menu {
          position: absolute;
          bottom: 58px;
          right: 20px;
          width: 260px;
          background: rgba(36, 36, 36, 0.95);
          backdrop-filter: blur(16px);
          -webkit-backdrop-filter: blur(16px);
          border: 1px solid rgba(255, 255, 255, 0.15);
          border-radius: 10px;
          padding: 6px 0;
          box-shadow: 0 12px 36px rgba(0, 0, 0, 0.75);
          z-index: 100;
          display: none;
          flex-direction: column;
          overflow: hidden;
          animation: menuSlideUp 0.18s cubic-bezier(0.16, 1, 0.3, 1);
        }

        .more-menu.open {
          display: flex;
        }

        @keyframes menuSlideUp {
          from {
            opacity: 0;
            transform: translateY(10px) scale(0.96);
          }
          to {
            opacity: 1;
            transform: translateY(0) scale(1);
          }
        }

        .menu-view {
          display: flex;
          flex-direction: column;
          width: 100%;
        }

        .menu-header {
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 10px 16px;
          font-size: 13px;
          font-weight: 600;
          color: #ffffff;
          border-bottom: 1px solid rgba(255, 255, 255, 0.1);
          cursor: pointer;
          background: rgba(255, 255, 255, 0.04);
        }

        .menu-header:hover {
          background: rgba(255, 255, 255, 0.08);
        }

        .menu-item {
          display: flex;
          align-items: center;
          gap: 14px;
          padding: 12px 18px;
          color: #ffffff;
          font-size: 14px;
          font-weight: 500;
          cursor: pointer;
          transition: background 0.15s ease;
        }

        .menu-item:hover {
          background: rgba(255, 255, 255, 0.14);
        }

        .menu-item svg {
          flex-shrink: 0;
          color: #ffffff;
        }

        .menu-text {
          flex: 1;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .menu-value {
          font-size: 12px;
          color: #94a3b8;
          font-weight: 500;
        }

        .menu-item .check-mark {
          width: 16px;
          font-size: 14px;
          color: #00ff66;
          font-weight: bold;
          text-align: center;
        }

        /* Minimal Spinner */
        .loader-spinner {
          position: absolute;
          width: 42px;
          height: 42px;
          border: 3px solid rgba(0, 255, 102, 0.15);
          border-top-color: #00ff66;
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
          pointer-events: none;
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }
      `;

      const viewport = document.createElement('div');
      viewport.className = 'player-viewport';
      viewport.id = 'viewport';

      const canvas = document.createElement('canvas');
      canvas.width = 3840; // 4K Super-Resolution internal buffer
      canvas.height = 2160;

      const scanlines = document.createElement('div');
      scanlines.className = 'scanlines';

      const centerIndicator = document.createElement('div');
      centerIndicator.className = 'center-indicator';
      centerIndicator.id = 'centerIndicator';
      centerIndicator.innerHTML = `
        <svg id="centerPlayIcon" viewBox="0 0 24 24" width="34" height="34" fill="currentColor">
          <path d="M8 5v14l11-7z"/>
        </svg>
        <svg id="centerPauseIcon" style="display:none;" viewBox="0 0 24 24" width="34" height="34" fill="currentColor">
          <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>
        </svg>
      `;

      const splitIndicator = document.createElement('div');
      splitIndicator.className = 'split-indicator';
      splitIndicator.id = 'splitOverlay';
      splitIndicator.style.display = 'none';
      splitIndicator.innerHTML = `
        <span class="left-label">720p NATIVE</span>
        <span class="divider-icon">◄ ❙ ►</span>
        <span class="right-label">4K SUPER-RES</span>
      `;

      // Popup More Menu (Matches Screenshot 3)
      const moreMenu = document.createElement('div');
      moreMenu.className = 'more-menu';
      moreMenu.id = 'moreMenu';
      moreMenu.innerHTML = `
        <!-- Main Menu View -->
        <div class="menu-view" id="menuMainView">
          <div class="menu-item" id="menuDownload">
            <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
            </svg>
            <span class="menu-text">Download</span>
          </div>
          <div class="menu-item" id="menuSpeed">
            <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"/>
            </svg>
            <span class="menu-text">Playback speed</span>
            <span class="menu-value" id="menuSpeedVal">Normal</span>
          </div>
          <div class="menu-item" id="menuQuality">
            <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-8 12H9.5v-2h-2v2H6V9h1.5v2.5h2V9H11v6zm7-1c0 .55-.45 1-1 1h-4V9h4c.55 0 1 .45 1 1v4zm-1.5-3.5h-2v3h2v-3z"/>
            </svg>
            <span class="menu-text">Quality (Super-Res)</span>
            <span class="menu-value" id="menuQualityVal">FSR 4K</span>
          </div>
          <div class="menu-item" id="menuPip">
            <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M19 7h-8v6h8V7zm2-4H3c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16.01H3V4.99h18v14.02z"/>
            </svg>
            <span class="menu-text">Picture in picture</span>
          </div>
        </div>

        <!-- Quality Submenu (5 Super-Resolution Modes) -->
        <div class="menu-view" id="menuQualitySubView" style="display:none;">
          <div class="menu-header" id="backQuality">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
              <path d="M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z"/>
            </svg>
            <span>Quality / Super-Resolution</span>
          </div>
          <div class="menu-item option-item" data-mode="easu">
            <span class="check-mark" id="checkEasu">✓</span>
            <span class="menu-text">FSR 4K (AMD FidelityFX)</span>
          </div>
          <div class="menu-item option-item" data-mode="bilateral">
            <span class="check-mark" id="checkBilateral"></span>
            <span class="menu-text">Bilateral 4K (Vector Edge)</span>
          </div>
          <div class="menu-item option-item" data-mode="auto">
            <span class="check-mark" id="checkAuto"></span>
            <span class="menu-text">Auto 4K (Display-Adaptive)</span>
          </div>
          <div class="menu-item option-item" data-mode="native">
            <span class="check-mark" id="checkNative"></span>
            <span class="menu-text">720p Native (Baseline)</span>
          </div>
          <div class="menu-item option-item" data-mode="split">
            <span class="check-mark" id="checkSplit"></span>
            <span class="menu-text">A/B Split-Screen Comparison</span>
          </div>
        </div>

        <!-- Playback Speed Submenu -->
        <div class="menu-view" id="menuSpeedSubView" style="display:none;">
          <div class="menu-header" id="backSpeed">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor">
              <path d="M15.41 7.41L14 6l-6 6 6 6 1.41-1.41L10.83 12z"/>
            </svg>
            <span>Playback Speed</span>
          </div>
          <div class="menu-item speed-item" data-speed="0.5">
            <span class="check-mark"></span>
            <span class="menu-text">0.5x</span>
          </div>
          <div class="menu-item speed-item" data-speed="0.75">
            <span class="check-mark"></span>
            <span class="menu-text">0.75x</span>
          </div>
          <div class="menu-item speed-item" data-speed="1.0">
            <span class="check-mark">✓</span>
            <span class="menu-text">Normal (1.0x)</span>
          </div>
          <div class="menu-item speed-item" data-speed="1.25">
            <span class="check-mark"></span>
            <span class="menu-text">1.25x</span>
          </div>
          <div class="menu-item speed-item" data-speed="1.5">
            <span class="check-mark"></span>
            <span class="menu-text">1.5x</span>
          </div>
          <div class="menu-item speed-item" data-speed="2.0">
            <span class="check-mark"></span>
            <span class="menu-text">2.0x</span>
          </div>
        </div>
      `;

      const controls = document.createElement('div');
      controls.className = 'controls-overlay';
      controls.id = 'controlsOverlay';
      controls.innerHTML = `
        <div class="scrubber-bar" id="scrubberBar" title="Seek">
          <div class="scrubber-track" id="scrubberTrack">
            <div class="scrubber-fill" id="scrubberFill"></div>
            <div class="scrubber-thumb" id="scrubberThumb"></div>
          </div>
        </div>
        <div class="controls-row">
          <button class="ctrl-btn" id="ctrlPlay" aria-label="Play/Pause" title="Play/Pause (Space)">
            <svg id="iconPlay" viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            <svg id="iconPause" style="display:none;" viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>
            </svg>
          </button>
          
          <!-- Volume Mute / Unmute Button -->
          <button class="ctrl-btn" id="ctrlMute" aria-label="Mute/Unmute" title="Mute/Unmute (M)">
            <svg id="iconVolHigh" style="display:none;" viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/>
            </svg>
            <svg id="iconVolMuted" viewBox="0 0 24 24" width="22" height="22" fill="currentColor">
              <path d="M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51C20.63 14.91 21 13.5 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z"/>
            </svg>
          </button>

          <div class="time-display" id="timeDisplay">
            <span class="current" id="timeCurrent">0:00</span><span class="separator">/</span><span class="duration" id="timeDuration">0:00</span>
          </div>
          
          <!-- Upscale Mode Switcher -->
          <div class="upscale-pill-group" id="upscalePills">
            <button class="pill-btn active" data-mode="easu" id="btnEasu" title="AMD FidelityFX Edge-Adaptive Super Resolution 4K">FSR 4K</button>
            <button class="pill-btn" data-mode="bilateral" id="btnBilateral" title="Bilateral Vector Edge-Preserving Reconstructive Filter 4K">Bilateral 4K</button>
            <button class="pill-btn" data-mode="auto" id="btnAuto" title="Display-Adaptive Multi-Sampling (Auto Retina)">Auto 4K</button>
            <button class="pill-btn" data-mode="native" id="btnNative" title="Native 720p Baseline Comparison">720p</button>
            <button class="pill-btn" data-mode="split" id="btnSplit" title="A/B Split-Screen Comparison Slider">A/B Split</button>
          </div>

          <div class="spacer"></div>
          <div class="format-badge" id="formatBadge">.ZVID v5 [FSR 4K]</div>
          
          <!-- Fullscreen Button -->
          <button class="ctrl-btn" id="ctrlFullscreen" aria-label="Toggle Fullscreen" title="Fullscreen (F)">
            <svg id="iconFsExpand" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
              <path d="M7 14H5v5h5v-2H7v-3zm-2-4h2V7h3V5H5v5zm12 7h-3v2h5v-5h-2v3zM14 5v2h3v3h2V5h-5z"/>
            </svg>
            <svg id="iconFsCompress" style="display:none;" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
              <path d="M5 16h3v3h2v-5H5v2zm3-8H5v2h5V5H8v3zm6 11h2v-3h3v-2h-5v5zm2-11V5h-2v5h5V8h-3z"/>
            </svg>
          </button>

          <!-- Three Vertical Dots Button (Matches Screenshot 1 & 2) -->
          <button class="ctrl-btn" id="ctrlMore" aria-label="More options" title="Settings / Options">
            <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
              <path d="M12 8c1.1 0 2-.9 2-2s-.9-2-2-2-2 .9-2 2 .9 2 2 2zm0 2c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2zm0 6c-1.1 0-2 .9-2 2s.9 2 2 2 2-.9 2-2-.9-2-2-2z"/>
            </svg>
          </button>
        </div>
      `;

      const loader = document.createElement('div');
      loader.className = 'loader-spinner';
      loader.id = 'loader';

      viewport.appendChild(canvas);
      viewport.appendChild(scanlines);
      viewport.appendChild(splitIndicator);
      viewport.appendChild(centerIndicator);
      viewport.appendChild(moreMenu);
      viewport.appendChild(controls);
      viewport.appendChild(loader);

      this.shadowRoot.appendChild(style);
      this.shadowRoot.appendChild(viewport);

      // Element References
      this._viewport = viewport;
      this._canvas = canvas;
      this._scanlinesEl = scanlines;
      this._centerIndicator = centerIndicator;
      this._centerPlayIcon = centerIndicator.querySelector('#centerPlayIcon');
      this._centerPauseIcon = centerIndicator.querySelector('#centerPauseIcon');
      this._splitOverlay = splitIndicator;
      this._moreMenu = moreMenu;
      this._menuMainView = moreMenu.querySelector('#menuMainView');
      this._menuQualitySubView = moreMenu.querySelector('#menuQualitySubView');
      this._menuSpeedSubView = moreMenu.querySelector('#menuSpeedSubView');
      this._menuDownload = moreMenu.querySelector('#menuDownload');
      this._menuSpeed = moreMenu.querySelector('#menuSpeed');
      this._menuSpeedVal = moreMenu.querySelector('#menuSpeedVal');
      this._menuQuality = moreMenu.querySelector('#menuQuality');
      this._menuQualityVal = moreMenu.querySelector('#menuQualityVal');
      this._menuPip = moreMenu.querySelector('#menuPip');
      this._backQuality = moreMenu.querySelector('#backQuality');
      this._backSpeed = moreMenu.querySelector('#backSpeed');

      this._controlsOverlay = controls;
      this._scrubberBar = controls.querySelector('#scrubberBar');
      this._scrubberTrack = controls.querySelector('#scrubberTrack');
      this._scrubberFill = controls.querySelector('#scrubberFill');
      this._scrubberThumb = controls.querySelector('#scrubberThumb');
      this._ctrlPlay = controls.querySelector('#ctrlPlay');
      this._iconPlay = controls.querySelector('#iconPlay');
      this._iconPause = controls.querySelector('#iconPause');
      this._ctrlMute = controls.querySelector('#ctrlMute');
      this._iconVolHigh = controls.querySelector('#iconVolHigh');
      this._iconVolMuted = controls.querySelector('#iconVolMuted');
      this._timeCurrent = controls.querySelector('#timeCurrent');
      this._timeDuration = controls.querySelector('#timeDuration');
      this._upscalePills = controls.querySelector('#upscalePills');
      this._ctrlFullscreen = controls.querySelector('#ctrlFullscreen');
      this._iconFsExpand = controls.querySelector('#iconFsExpand');
      this._iconFsCompress = controls.querySelector('#iconFsCompress');
      this._ctrlMore = controls.querySelector('#ctrlMore');
      this._formatBadge = controls.querySelector('#formatBadge');
      this._loader = loader;

      // Initialize WebGL GPU Super-Resolution Pipeline
      this._initWebGL(canvas);

      this._setupEventListeners();
    }

    _initWebGL(canvas) {
      try {
        const gl = canvas.getContext('webgl2', { alpha: false, depth: false, antialias: false, preserveDrawingBuffer: false }) ||
                   canvas.getContext('webgl', { alpha: false, depth: false, antialias: false, preserveDrawingBuffer: false });
        if (!gl) {
          console.warn('[Z-VISION] WebGL unavailable, falling back to 2D canvas context.');
          this._gl = null;
          this._ctx = canvas.getContext('2d');
          return;
        }

        this._gl = gl;

        // Vertex Shader
        const vsSource = `
          attribute vec2 a_position;
          varying vec2 v_texCoord;
          void main() {
            v_texCoord = vec2((a_position.x + 1.0) * 0.5, (1.0 - a_position.y) * 0.5);
            gl_Position = vec4(a_position, 0.0, 1.0);
          }
        `;

        // Fragment Shader: Real-Time Edge-Adaptive & Bilateral Super-Resolution
        const fsSource = `
          precision mediump float;
          varying vec2 v_texCoord;
          uniform sampler2D u_texture;
          uniform vec2 u_texSize;
          uniform int u_mode;
          uniform float u_splitX;

          float getLuma(vec3 c) {
            return dot(c, vec3(0.299, 0.587, 0.114));
          }

          // Mode 1: AMD FidelityFX Super-Resolution (EASU + RCAS)
          vec4 renderEASU_RCAS(vec2 uv) {
            vec2 dx = vec2(1.0 / u_texSize.x, 0.0);
            vec2 dy = vec2(0.0, 1.0 / u_texSize.y);

            vec3 c  = texture2D(u_texture, uv).rgb;
            vec3 n  = texture2D(u_texture, uv - dy).rgb;
            vec3 s  = texture2D(u_texture, uv + dy).rgb;
            vec3 w  = texture2D(u_texture, uv - dx).rgb;
            vec3 e  = texture2D(u_texture, uv + dx).rgb;
            vec3 nw = texture2D(u_texture, uv - dx - dy).rgb;
            vec3 ne = texture2D(u_texture, uv + dx - dy).rgb;
            vec3 sw = texture2D(u_texture, uv - dx + dy).rgb;
            vec3 se = texture2D(u_texture, uv + dx + dy).rgb;

            float lC = getLuma(c);
            float lN = getLuma(n);
            float lS = getLuma(s);
            float lW = getLuma(w);
            float lE = getLuma(e);

            float gradH = abs(lE - lW);
            float gradV = abs(lS - lN);

            vec3 edgeRecon = (c * 2.2 + (n + s) * (1.0 - gradV * 0.45) + (w + e) * (1.0 - gradH * 0.45) + (nw + ne + sw + se) * 0.25) / (4.2 + (1.0 - gradV * 0.45) * 2.0 + (1.0 - gradH * 0.45) * 2.0);

            float minLuma = min(min(min(lC, lN), min(lS, lW)), lE);
            float maxLuma = max(max(max(lC, lN), max(lS, lW)), lE);
            float contrast = maxLuma - minLuma;
            float rcasWeight = clamp(1.0 - contrast * 1.4, 0.0, 0.9);

            vec3 highPass = (n + s + w + e) * 0.25;
            vec3 sharpened = edgeRecon + (edgeRecon - highPass) * (0.75 * rcasWeight);

            return vec4(clamp(sharpened, 0.0, 1.0), 1.0);
          }

          // Mode 2: Bilateral Vector Edge-Preserving Reconstructive Filter
          vec4 renderBilateral(vec2 uv) {
            vec2 texel = 1.0 / u_texSize;
            vec3 center = texture2D(u_texture, uv).rgb;
            float centerLuma = getLuma(center);

            vec3 accumColor = vec3(0.0);
            float accumWeight = 0.0;

            for (int y = -2; y <= 2; y++) {
              for (int x = -2; x <= 2; x++) {
                vec2 offset = vec2(float(x), float(y)) * texel;
                vec3 sColor = texture2D(u_texture, uv + offset).rgb;
                float sLuma = getLuma(sColor);

                float dSpatial = float(x * x + y * y);
                float dColor = abs(sLuma - centerLuma);
                float w = exp(-dSpatial * 0.35 - dColor * 14.0);

                accumColor += sColor * w;
                accumWeight += w;
              }
            }

            vec3 filtered = accumColor / max(accumWeight, 0.0001);
            vec3 diff = center - filtered;
            vec3 crispened = center + diff * 1.35;

            return vec4(clamp(crispened, 0.0, 1.0), 1.0);
          }

          // Mode 3: Display-Adaptive Multi-Sampling (Catmull-Rom Bicubic + Micro-Sharpening)
          vec4 renderAdaptiveRetina(vec2 uv) {
            vec2 texel = 1.0 / u_texSize;
            vec2 coord = uv * u_texSize - 0.5;
            vec2 f = fract(coord);
            vec2 i = floor(coord);

            vec2 w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
            vec2 w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
            vec2 w2 = f * (0.5 + f * (2.0 - 1.5 * f));
            vec2 w3 = f * f * (-0.5 + 0.5 * f);

            vec2 w12 = w1 + w2;
            vec2 offset12 = w2 / (w1 + w2);

            vec2 texCoord0 = (i - 0.5) * texel;
            vec2 texCoord12 = (i + 0.5 + offset12) * texel;
            vec2 texCoord3 = (i + 2.5) * texel;

            vec3 col = vec3(0.0);
            col += texture2D(u_texture, vec2(texCoord12.x, texCoord12.y)).rgb * (w12.x * w12.y);
            col += texture2D(u_texture, vec2(texCoord0.x, texCoord12.y)).rgb * (w0.x * w12.y);
            col += texture2D(u_texture, vec2(texCoord3.x, texCoord12.y)).rgb * (w3.x * w12.y);
            col += texture2D(u_texture, vec2(texCoord12.x, texCoord0.y)).rgb * (w12.x * w0.y);
            col += texture2D(u_texture, vec2(texCoord12.x, texCoord3.y)).rgb * (w12.x * w3.y);

            vec3 center = texture2D(u_texture, uv).rgb;
            vec3 sharp = col + (center - col) * 0.50;

            return vec4(clamp(sharp, 0.0, 1.0), 1.0);
          }

          void main() {
            if (u_mode == 4) {
              // A/B Split-Screen Mode
              float dist = abs(v_texCoord.x - u_splitX);
              if (dist < 0.0018) {
                gl_FragColor = vec4(0.0, 1.0, 0.4, 1.0);
                return;
              }
              if (v_texCoord.x < u_splitX) {
                gl_FragColor = texture2D(u_texture, v_texCoord); // 720p Native
              } else {
                gl_FragColor = renderEASU_RCAS(v_texCoord); // 4K Super-Resolution
              }
              return;
            }

            if (u_mode == 1) {
              gl_FragColor = renderEASU_RCAS(v_texCoord);
            } else if (u_mode == 2) {
              gl_FragColor = renderBilateral(v_texCoord);
            } else if (u_mode == 3) {
              gl_FragColor = renderAdaptiveRetina(v_texCoord);
            } else {
              gl_FragColor = texture2D(u_texture, v_texCoord);
            }
          }
        `;

        const compileShader = (type, source) => {
          const s = gl.createShader(type);
          gl.shaderSource(s, source);
          gl.compileShader(s);
          if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
            console.warn('[Z-VISION WebGL] Shader error:', gl.getShaderInfoLog(s));
            gl.deleteShader(s);
            return null;
          }
          return s;
        };

        const vs = compileShader(gl.VERTEX_SHADER, vsSource);
        const fs = compileShader(gl.FRAGMENT_SHADER, fsSource);
        if (!vs || !fs) {
          this._gl = null;
          this._ctx = canvas.getContext('2d');
          return;
        }

        const program = gl.createProgram();
        gl.attachShader(program, vs);
        gl.attachShader(program, fs);
        gl.linkProgram(program);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
          console.warn('[Z-VISION WebGL] Program error:', gl.getProgramInfoLog(program));
          this._gl = null;
          this._ctx = canvas.getContext('2d');
          return;
        }

        this._program = program;
        gl.useProgram(program);

        // Quad geometry
        const quad = new Float32Array([
          -1, -1,
           1, -1,
          -1,  1,
          -1,  1,
           1, -1,
           1,  1
        ]);
        const posBuf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
        gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);

        const aPos = gl.getAttribLocation(program, 'a_position');
        gl.enableVertexAttribArray(aPos);
        gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

        // Uniforms & Texture
        this._uTextureLoc = gl.getUniformLocation(program, 'u_texture');
        this._uTexSizeLoc = gl.getUniformLocation(program, 'u_texSize');
        this._uModeLoc = gl.getUniformLocation(program, 'u_mode');
        this._uSplitXLoc = gl.getUniformLocation(program, 'u_splitX');

        this._texture = gl.createTexture();
        gl.bindTexture(gl.TEXTURE_2D, this._texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

        gl.uniform1i(this._uTextureLoc, 0);
      } catch (e) {
        console.warn('[Z-VISION] WebGL init failed, falling back to 2D:', e);
        this._gl = null;
        this._ctx = canvas.getContext('2d');
      }
    }

    _setupEventListeners() {
      // 1. Mouse Movement / Hover Activity Tracking
      const showControls = () => {
        this._controlsOverlay.classList.add('visible');
        this._viewport.classList.remove('hide-cursor');
        clearTimeout(this._hideTimer);

        if (this._isPlaying && !this._isSeeking && !this._menuOpen) {
          this._hideTimer = setTimeout(() => {
            if (this._isPlaying && !this._isSeeking && !this._menuOpen) {
              this._controlsOverlay.classList.remove('visible');
              this._viewport.classList.add('hide-cursor');
              this._closeMoreMenu();
            }
          }, 2800);
        }
      };

      const hideControls = () => {
        if (this._isPlaying && !this._isSeeking && !this._menuOpen) {
          this._controlsOverlay.classList.remove('visible');
          this._viewport.classList.add('hide-cursor');
          this._closeMoreMenu();
        }
      };

      this._viewport.addEventListener('mousemove', showControls);
      this._viewport.addEventListener('mouseenter', showControls);
      this._viewport.addEventListener('touchstart', showControls, { passive: true });
      this._viewport.addEventListener('mouseleave', hideControls);

      // 2. Play / Pause on Canvas Click
      this._canvas.addEventListener('click', (e) => {
        if (this._menuOpen) {
          this._closeMoreMenu();
          return;
        }
        if (this._upscaleMode === 'split') return;
        e.stopPropagation();
        this.togglePlay();
      });

      // 3. Play / Pause on Button Click
      this._ctrlPlay.addEventListener('click', (e) => {
        e.stopPropagation();
        this.togglePlay();
      });

      // 4. Mute / Unmute Button
      this._ctrlMute.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMute();
      });

      // 5. Interactive Timeline Scrubber (Click & Drag)
      const seekAtEvent = (e) => {
        const rect = this._scrubberTrack.getBoundingClientRect();
        const clientX = e.touches ? e.touches[0].clientX : e.clientX;
        const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
        this._currentTime = ratio * this._duration;
        if (this._videoSource) {
          this._videoSource.currentTime = this._currentTime;
        }
        this._renderFrame(this._currentTime);
        this._updateTimeUI();
      };

      const onPointerDown = (e) => {
        e.preventDefault();
        e.stopPropagation();
        this._isSeeking = true;
        this._scrubberBar.classList.add('seeking');
        seekAtEvent(e);

        const onPointerMove = (moveEvent) => {
          if (!this._isSeeking) return;
          seekAtEvent(moveEvent);
        };

        const onPointerUp = () => {
          this._isSeeking = false;
          this._scrubberBar.classList.remove('seeking');
          window.removeEventListener('mousemove', onPointerMove);
          window.removeEventListener('mouseup', onPointerUp);
          window.removeEventListener('touchmove', onPointerMove);
          window.removeEventListener('touchend', onPointerUp);
          showControls();
        };

        window.addEventListener('mousemove', onPointerMove);
        window.addEventListener('mouseup', onPointerUp);
        window.addEventListener('touchmove', onPointerMove, { passive: false });
        window.addEventListener('touchend', onPointerUp);
      };

      this._scrubberBar.addEventListener('mousedown', onPointerDown);
      this._scrubberBar.addEventListener('touchstart', onPointerDown, { passive: false });

      // 6. Fullscreen Toggle & Events
      this._ctrlFullscreen.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleFullscreen();
      });

      this._canvas.addEventListener('dblclick', (e) => {
        e.stopPropagation();
        this.toggleFullscreen();
      });

      const updateFsUI = () => {
        const isFs = !!(document.fullscreenElement || document.webkitFullscreenElement || document.mozFullScreenElement || document.msFullscreenElement);
        if (this._iconFsExpand) this._iconFsExpand.style.display = isFs ? 'none' : 'block';
        if (this._iconFsCompress) this._iconFsCompress.style.display = isFs ? 'block' : 'none';
      };
      document.addEventListener('fullscreenchange', updateFsUI);
      document.addEventListener('webkitfullscreenchange', updateFsUI);

      // 7. Three Vertical Dots (⋮) Overflow Menu Toggle
      this._ctrlMore.addEventListener('click', (e) => {
        e.stopPropagation();
        this.toggleMoreMenu();
      });

      // 8. Menu Actions: Download
      this._menuDownload.addEventListener('click', (e) => {
        e.stopPropagation();
        this.downloadVideo();
        this._closeMoreMenu();
      });

      // 9. Menu Actions: Playback Speed Submenu
      this._menuSpeed.addEventListener('click', (e) => {
        e.stopPropagation();
        this._menuMainView.style.display = 'none';
        this._menuSpeedSubView.style.display = 'flex';
      });

      this._backSpeed.addEventListener('click', (e) => {
        e.stopPropagation();
        this._menuSpeedSubView.style.display = 'none';
        this._menuMainView.style.display = 'flex';
      });

      this._menuSpeedSubView.querySelectorAll('.speed-item').forEach(item => {
        item.addEventListener('click', (e) => {
          e.stopPropagation();
          const speed = parseFloat(item.getAttribute('data-speed'));
          this.setPlaybackRate(speed);
          this._closeMoreMenu();
        });
      });

      // 10. Menu Actions: Quality Submenu (5 Super-Resolution Modes)
      this._menuQuality.addEventListener('click', (e) => {
        e.stopPropagation();
        this._menuMainView.style.display = 'none';
        this._menuQualitySubView.style.display = 'flex';
      });

      this._backQuality.addEventListener('click', (e) => {
        e.stopPropagation();
        this._menuQualitySubView.style.display = 'none';
        this._menuMainView.style.display = 'flex';
      });

      this._menuQualitySubView.querySelectorAll('.option-item').forEach(item => {
        item.addEventListener('click', (e) => {
          e.stopPropagation();
          const mode = item.getAttribute('data-mode');
          this.setUpscaleMode(mode);
          this._closeMoreMenu();
        });
      });

      // 11. Menu Actions: Picture-in-Picture
      this._menuPip.addEventListener('click', (e) => {
        e.stopPropagation();
        this.togglePip();
        this._closeMoreMenu();
      });

      // 12. Quick Upscale Mode Switcher Pills
      if (this._upscalePills) {
        this._upscalePills.addEventListener('click', (e) => {
          const btn = e.target.closest('.pill-btn');
          if (!btn) return;
          e.stopPropagation();
          const mode = btn.getAttribute('data-mode');
          this.setUpscaleMode(mode);
        });
      }

      // 13. Interactive Split-Screen Slider Dragging / Hover
      const updateSplitAtEvent = (e) => {
        if (this._upscaleMode !== 'split') return;
        const rect = this._canvas.getBoundingClientRect();
        const clientX = e.touches ? e.touches[0].clientX : e.clientX;
        const ratio = Math.max(0.02, Math.min(0.98, (clientX - rect.left) / rect.width));
        this._splitX = ratio;
        this._renderFrame(this._currentTime);
      };

      this._canvas.addEventListener('mousemove', (e) => {
        if (this._upscaleMode === 'split') {
          updateSplitAtEvent(e);
        }
      });

      this._canvas.addEventListener('click', (e) => {
        if (this._upscaleMode === 'split') {
          updateSplitAtEvent(e);
        }
      });

      this._canvas.addEventListener('touchmove', (e) => {
        if (this._upscaleMode === 'split') {
          updateSplitAtEvent(e);
        }
      }, { passive: true });
    }

    _setupKeyboardEvents() {
      window.addEventListener('keydown', (e) => {
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;

        if (e.code === 'Space') {
          e.preventDefault();
          this.togglePlay();
        } else if (e.code === 'KeyM') {
          e.preventDefault();
          this.toggleMute();
        } else if (e.code === 'KeyF') {
          e.preventDefault();
          this.toggleFullscreen();
        } else if (e.code === 'ArrowRight') {
          e.preventDefault();
          this.seek(this._currentTime + 1.0);
        } else if (e.code === 'ArrowLeft') {
          e.preventDefault();
          this.seek(this._currentTime - 1.0);
        } else if (e.code === 'Digit1') {
          this.setUpscaleMode('easu');
        } else if (e.code === 'Digit2') {
          this.setUpscaleMode('bilateral');
        } else if (e.code === 'Digit3') {
          this.setUpscaleMode('auto');
        } else if (e.code === 'Digit4') {
          this.setUpscaleMode('native');
        } else if (e.code === 'Digit5') {
          this.setUpscaleMode('split');
        }
      });
    }

    toggleMute(enable) {
      this._isMuted = enable !== undefined ? enable : !this._isMuted;
      if (this._videoSource) {
        this._videoSource.muted = this._isMuted;
      }
      if (this._audio) {
        this._audio.muted = this._isMuted;
      }
      this._iconVolHigh.style.display = this._isMuted ? 'none' : 'block';
      this._iconVolMuted.style.display = this._isMuted ? 'block' : 'none';
    }

    toggleMoreMenu() {
      if (this._menuOpen) {
        this._closeMoreMenu();
      } else {
        this._openMoreMenu();
      }
    }

    _openMoreMenu() {
      this._menuOpen = true;
      this._moreMenu.classList.add('open');
      this._menuMainView.style.display = 'flex';
      this._menuQualitySubView.style.display = 'none';
      this._menuSpeedSubView.style.display = 'none';
    }

    _closeMoreMenu() {
      this._menuOpen = false;
      this._moreMenu.classList.remove('open');
    }

    setPlaybackRate(speed) {
      this._playbackRate = speed;
      if (this._videoSource) {
        this._videoSource.playbackRate = speed;
      }
      if (this._audio) {
        this._audio.playbackRate = speed;
      }
      this._menuSpeedVal.textContent = speed === 1.0 ? 'Normal' : `${speed}x`;

      // Update check marks
      this._menuSpeedSubView.querySelectorAll('.speed-item').forEach(item => {
        const s = parseFloat(item.getAttribute('data-speed'));
        const check = item.querySelector('.check-mark');
        if (check) check.textContent = s === speed ? '✓' : '';
      });
    }

    downloadVideo() {
      const a = document.createElement('a');
      a.href = this._src || 'HUG-ART-ZYMATICA.zvid';
      a.download = (this._src ? this._src.split('/').pop() : 'HUG-ART-ZYMATICA.zvid');
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
    }

    async togglePip() {
      try {
        if (document.pictureInPictureElement) {
          await document.exitPictureInPicture();
        } else if (this._videoSource && document.pictureInPictureEnabled) {
          await this._videoSource.requestPictureInPicture();
        }
      } catch (e) {
        console.warn('[Z-VISION] PiP not supported on this stream:', e);
      }
    }

    setUpscaleMode(mode) {
      this._upscaleMode = mode;
      let modeInt = 1;
      let targetW = 3840;
      let targetH = 2160;
      let badgeText = '.ZVID v5 [FSR 4K]';
      let menuLabel = 'FSR 4K';

      if (mode === 'bilateral') {
        modeInt = 2;
        targetW = 3840;
        targetH = 2160;
        badgeText = '.ZVID v5 [BILATERAL 4K]';
        menuLabel = 'Bilateral 4K';
      } else if (mode === 'auto') {
        modeInt = 3;
        const dpr = window.devicePixelRatio || 1;
        targetW = Math.min(3840, Math.round(1280 * dpr * 1.5));
        targetH = Math.min(2160, Math.round(720 * dpr * 1.5));
        badgeText = `.ZVID v5 [AUTO ${targetH}p]`;
        menuLabel = `Auto ${targetH}p`;
      } else if (mode === 'native' || mode === 'off' || mode === '720p') {
        modeInt = 0;
        targetW = 1280;
        targetH = 720;
        badgeText = '.ZVID v5 [720p NATIVE]';
        menuLabel = '720p';
      } else if (mode === 'split') {
        modeInt = 4;
        targetW = 3840;
        targetH = 2160;
        badgeText = '.ZVID v5 [A/B SPLIT]';
        menuLabel = 'A/B Split';
      }

      this._modeInt = modeInt;
      if (this._canvas) {
        this._canvas.width = targetW;
        this._canvas.height = targetH;
      }

      if (this._formatBadge) {
        this._formatBadge.textContent = badgeText;
      }

      if (this._menuQualityVal) {
        this._menuQualityVal.textContent = menuLabel;
      }

      // Update check marks in Quality submenu
      const checks = {
        'easu': this.shadowRoot.querySelector('#checkEasu'),
        'bilateral': this.shadowRoot.querySelector('#checkBilateral'),
        'auto': this.shadowRoot.querySelector('#checkAuto'),
        'native': this.shadowRoot.querySelector('#checkNative'),
        'split': this.shadowRoot.querySelector('#checkSplit')
      };
      for (const [k, el] of Object.entries(checks)) {
        if (el) el.textContent = (k === mode || (mode === 'native' && k === 'native')) ? '✓' : '';
      }

      if (this._upscalePills) {
        const btns = this._upscalePills.querySelectorAll('.pill-btn');
        btns.forEach(btn => {
          btn.classList.toggle('active', btn.getAttribute('data-mode') === mode ||
                                         (mode === 'native' && btn.getAttribute('data-mode') === 'native'));
        });
      }

      if (this._splitOverlay) {
        this._splitOverlay.style.display = (mode === 'split') ? 'flex' : 'none';
      }

      this._renderFrame(this._currentTime);
    }

    togglePlay() {
      if (this._isPlaying) {
        this.pause();
      } else {
        this.play();
      }
    }

    play() {
      if (!this._isLoaded) return;
      this._isPlaying = true;
      this._iconPlay.style.display = 'none';
      this._iconPause.style.display = 'block';
      this._flashCenterBadge(true);

      if (this._videoSource && this._videoSource.paused) {
        this._videoSource.play().catch(() => {});
      }
      if (this._audio && this._audio.paused) {
        this._audio.play().catch(() => {});
      }

      this._lastTimestamp = performance.now();
      if (!this._animFrameId) {
        this._animFrameId = requestAnimationFrame(() => this._loop());
      }
    }

    pause() {
      this._isPlaying = false;
      this._iconPlay.style.display = 'block';
      this._iconPause.style.display = 'none';
      this._flashCenterBadge(false);

      if (this._videoSource && !this._videoSource.paused) {
        this._videoSource.pause();
      }
      if (this._audio && !this._audio.paused) {
        this._audio.pause();
      }

      if (this._animFrameId) {
        cancelAnimationFrame(this._animFrameId);
        this._animFrameId = null;
      }
    }

    seek(timeSeconds) {
      this._currentTime = Math.max(0, Math.min(this._duration, timeSeconds));
      if (this._videoSource) {
        this._videoSource.currentTime = this._currentTime;
      }
      if (this._audio) {
        this._audio.currentTime = this._currentTime;
      }
      this._renderFrame(this._currentTime);
      this._updateTimeUI();
    }

    toggleFullscreen() {
      const isFs = document.fullscreenElement || document.webkitFullscreenElement || document.mozFullScreenElement || document.msFullscreenElement;
      if (!isFs) {
        const target = this;
        if (target.requestFullscreen) {
          target.requestFullscreen().catch(() => {
            if (this._videoSource && this._videoSource.webkitEnterFullscreen) {
              this._videoSource.webkitEnterFullscreen();
            }
          });
        } else if (target.webkitRequestFullscreen) {
          target.webkitRequestFullscreen();
        } else if (this._videoSource && this._videoSource.webkitEnterFullscreen) {
          this._videoSource.webkitEnterFullscreen();
        }
      } else {
        if (document.exitFullscreen) {
          document.exitFullscreen().catch(() => {});
        } else if (document.webkitExitFullscreen) {
          document.webkitExitFullscreen();
        }
      }
    }

    toggleScanlines(enable) {
      this._scanlines = enable !== undefined ? enable : !this._scanlines;
      this._scanlinesEl.classList.toggle('active', this._scanlines);
    }

    _flashCenterBadge(playing) {
      this._centerPlayIcon.style.display = playing ? 'block' : 'none';
      this._centerPauseIcon.style.display = playing ? 'none' : 'block';
      this._centerIndicator.classList.add('flash');
      setTimeout(() => {
        this._centerIndicator.classList.remove('flash');
      }, 450);
    }

    _updateTimeUI() {
      const progress = this._duration > 0 ? (this._currentTime / this._duration) : 0;
      const pct = Math.max(0, Math.min(100, progress * 100));
      this._scrubberFill.style.width = pct + '%';
      this._scrubberThumb.style.left = pct + '%';

      this._timeCurrent.textContent = formatTime(this._currentTime);
      this._timeDuration.textContent = formatTime(this._duration);
    }

    async load(source) {
      try {
        this._loader.style.display = 'block';

        let buffer;
        if (source instanceof ArrayBuffer) {
          buffer = source;
        } else if (source instanceof Blob) {
          buffer = await source.arrayBuffer();
        } else if (typeof source === 'string') {
          if (window.location.protocol === 'file:' || source.startsWith('file:')) {
            await this._loadLocalFallback(source);
            return;
          }
          const res = await fetch(source);
          if (!res.ok) throw new Error(`HTTP ${res.status} fetching .zvid`);
          buffer = await res.arrayBuffer();
        } else {
          throw new Error('Unsupported source type');
        }

        await this._parseZvidBuffer(buffer);
        this._isLoaded = true;
        this._loader.style.display = 'none';

        this._updateTimeUI();

        if (this.hasAttribute('autoplay')) {
          this.play();
        } else {
          this._renderFrame(0);
        }

        this.dispatchEvent(new CustomEvent('loaded', { detail: { duration: this._duration, frames: this._totalFrames } }));
      } catch (err) {
        console.warn('[Z-VISION] Falling back to local multi-anchor synthesis engine:', err);
        await this._loadLocalFallback(source);
      }
    }

    async _loadLocalFallback(source) {
      try {
        const loadImg = (src) => new Promise((resolve) => {
          const img = new Image();
          img.onload = () => resolve(img);
          img.onerror = () => resolve(null);
          img.src = src;
        });

        const testImg = await loadImg('frames/frame_000.webp');
        if (testImg) {
          const framePromises = [Promise.resolve(testImg)];
          for (let i = 1; i < 152; i++) {
            const pad = String(i).padStart(3, '0');
            framePromises.push(loadImg(`frames/frame_${pad}.webp`));
          }
          this._frames = (await Promise.all(framePromises)).filter(Boolean);
        } else {
          const anchorPromises = [];
          for (let i = 0; i < 14; i++) {
            anchorPromises.push(loadImg(`anchor${i}.webp`));
          }
          const loadedAnchors = await Promise.all(anchorPromises);
          this._anchors = loadedAnchors.filter(Boolean);
        }

        this._width = 1280;
        this._height = 720;
        this._fps = 24.0;
        this._totalFrames = 152;
        this._duration = 152 / 24.0;

        this._isLoaded = true;
        this._loader.style.display = 'none';
        this._updateTimeUI();

        if (this.hasAttribute('autoplay')) {
          this.play();
        } else {
          this._renderFrame(0);
        }

        this.dispatchEvent(new CustomEvent('loaded', { detail: { duration: this._duration, frames: this._totalFrames } }));
      } catch (e) {
        console.error('[Z-VISION] Local fallback error:', e);
        this._loader.style.display = 'none';
      }
    }

    async _parseZvidBuffer(buffer) {
      const view = new DataView(buffer);

      // Verify Magic Signature: 0x5A564944 ("ZVID")
      const magic = String.fromCharCode(view.getUint8(0), view.getUint8(1), view.getUint8(2), view.getUint8(3));
      if (magic !== 'ZVID') {
        throw new Error(`Invalid magic header: ${magic}. Expected 'ZVID'.`);
      }

      const version = view.getUint16(4, true);
      this._width = view.getUint16(6, true);
      this._height = view.getUint16(8, true);
      this._fps = view.getFloat32(10, true) || 24.0;
      this._totalFrames = view.getUint32(14, true);
      this._duration = this._totalFrames / this._fps;

      // Update badge with active upscale mode
      this.setUpscaleMode(this._upscaleMode || 'easu');

      if (version === 5) {
        // v5 Compact Neuromorphic Stream Capsule (~252 KB / 152 frames)
        const videoLen = view.getUint32(18, true);
        const trajLen = view.getUint32(22, true);
        const audioLen = view.getUint32(26, true);

        let dataOffset = 30;
        const videoBytes = buffer.slice(dataOffset, dataOffset + videoLen);
        dataOffset += videoLen;

        const trajBytes = buffer.slice(dataOffset, dataOffset + trajLen);
        const decoder = new TextDecoder('utf-8');
        this._trajectory = JSON.parse(decoder.decode(trajBytes));
        dataOffset += trajLen;

        if (audioLen > 0) {
          const audioBytes = buffer.slice(dataOffset, dataOffset + audioLen);
          const audioBlob = new Blob([audioBytes], { type: 'audio/aac' });
          if (!this._audio) {
            this._audio = new Audio();
          }
          this._audio.src = URL.createObjectURL(audioBlob);
          this._audio.muted = this._isMuted;
          this._audio.playbackRate = this._playbackRate;
        }

        const blob = new Blob([videoBytes], { type: 'video/mp4' });
        this._videoBlobUrl = URL.createObjectURL(blob);

        if (!this._videoSource) {
          this._videoSource = document.createElement('video');
          this._videoSource.playsInline = true;
          this._videoSource.muted = this._isMuted;
          this._videoSource.playbackRate = this._playbackRate;
          this._videoSource.preload = 'auto';
          this._videoSource.style.display = 'none';
          this.shadowRoot.appendChild(this._videoSource);
        }

        this._videoSource.src = this._videoBlobUrl;
        await new Promise((resolve) => {
          this._videoSource.onloadeddata = () => resolve();
          this._videoSource.onerror = () => resolve();
        });
      } else if (version >= 4) {
        // v4 Dense Stream
        const audioLen = view.getUint32(18, true);
        const trajLen = view.getUint32(22, true);
        const frameLens = [];
        let tableOffset = 26;
        for (let k = 0; k < this._totalFrames; k++) {
          frameLens.push(view.getUint32(tableOffset, true));
          tableOffset += 4;
        }

        let dataOffset = tableOffset;
        const framePromises = [];
        for (let k = 0; k < this._totalFrames; k++) {
          const fLen = frameLens[k];
          const fBytes = buffer.slice(dataOffset, dataOffset + fLen);
          dataOffset += fLen;
          const blob = new Blob([fBytes], { type: 'image/webp' });
          if (typeof createImageBitmap !== 'undefined') {
            framePromises.push(createImageBitmap(blob));
          } else {
            framePromises.push(this._loadImageBlob(blob));
          }
        }
        this._frames = await Promise.all(framePromises);
      }
    }

    _loadImageBlob(blob) {
      return new Promise((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = reject;
        img.src = URL.createObjectURL(blob);
      });
    }

    _loop() {
      if (!this._isPlaying) return;
      const now = performance.now();
      const delta = ((now - this._lastTimestamp) / 1000.0) * this._playbackRate;
      this._lastTimestamp = now;

      if (!this._isSeeking) {
        if (this._videoSource && !this._videoSource.paused) {
          this._currentTime = this._videoSource.currentTime;
        } else {
          this._currentTime += delta;
        }
      }

      if (this._currentTime >= this._duration) {
        if (this.hasAttribute('loop')) {
          this._currentTime = 0;
          if (this._videoSource) this._videoSource.currentTime = 0;
          if (this._audio) this._audio.currentTime = 0;
        } else {
          this.pause();
          this._currentTime = this._duration;
          this._renderFrame(this._duration);
          this._updateTimeUI();
          return;
        }
      }

      this._renderFrame(this._currentTime);
      this._updateTimeUI();
      this._animFrameId = requestAnimationFrame(() => this._loop());
    }

    _renderFrame(t) {
      const gl = this._gl;

      if (gl) {
        gl.viewport(0, 0, this._canvas.width, this._canvas.height);
        gl.useProgram(this._program);
        gl.bindTexture(gl.TEXTURE_2D, this._texture);

        const frameIdx = Math.max(0, Math.min(this._totalFrames - 1, Math.floor(t * this._fps)));

        if (this._videoSource && this._videoSource.readyState >= 2) {
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, this._videoSource);
        } else if (this._frames && this._frames.length > 0 && this._frames[frameIdx]) {
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, this._frames[frameIdx]);
        } else if (this._anchors && this._anchors.length > 0) {
          const recipe = this._trajectory[frameIdx] || { anchor: 0 };
          const aImg = this._anchors[recipe.anchor || 0] || this._anchors[0];
          if (aImg) {
            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, aImg);
          }
        }

        gl.uniform2f(this._uTexSizeLoc, this._width || 1280.0, this._height || 720.0);
        gl.uniform1i(this._uModeLoc, this._modeInt !== undefined ? this._modeInt : 1);
        gl.uniform1f(this._uSplitXLoc, this._splitX !== undefined ? this._splitX : 0.5);

        gl.drawArrays(gl.TRIANGLES, 0, 6);
        return;
      }

      // Fallback to 2D canvas context if WebGL not available
      const ctx = this._ctx;
      if (!ctx) return;
      const w = this._canvas.width;
      const h = this._canvas.height;

      ctx.fillStyle = "#000000";
      ctx.fillRect(0, 0, w, h);

      const frameIdx = Math.max(0, Math.min(this._totalFrames - 1, Math.floor(t * this._fps)));
      if (this._videoSource && this._videoSource.readyState >= 2) {
        ctx.drawImage(this._videoSource, 0, 0, w, h);
        return;
      }
      if (this._frames && this._frames.length > 0 && this._frames[frameIdx]) {
        ctx.drawImage(this._frames[frameIdx], 0, 0, w, h);
        return;
      }
    }
  }

  // Register <z-video> tag globally
  if (!customElements.get('z-video')) {
    customElements.define('z-video', ZVideoElement);
  }

  // Expose global ZVid namespace
  window.ZVid = {
    version: '5.2.0',
    ZVideoElement: ZVideoElement
  };

})();
