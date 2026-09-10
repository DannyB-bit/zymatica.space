-- ZYMATICA | Language-U Cross-Language Verification Engine (Lua)
-- Watermark: ip zymatica.space | astronautshe.com

print("======================================================================")
print("ZYMATICA | Cross-Language Lua Decompressor & Range-Decoder")
print("======================================================================\n")

local RadicalPredictor = {}
RadicalPredictor.__index = RadicalPredictor

function RadicalPredictor.new(alpha, weight)
    local self = setmetatable({}, RadicalPredictor)
    self.alpha = alpha
    self.weight = weight
    self.trans_rc = {}
    self.trans_rf = {}
    self.trans_ra = {}
    self.prev_rc = 0
    self.prev_rf = 0
    self.prev_ra = 0
    return self
end

function RadicalPredictor:observe(rc, rf, ra)
    local w = self.weight
    local key_rc = self.prev_rc
    local found = false
    for _, entry in ipairs(self.trans_rc) do
        if entry.key == key_rc and entry.sym == rc then
            entry.count = entry.count + w
            found = true
            break
        end
    end
    if not found and #self.trans_rc < 256 then
        table.insert(self.trans_rc, {key = key_rc, sym = rc, count = w})
    end

    local key_rf = (rc * 256) + self.prev_rf
    found = false
    for _, entry in ipairs(self.trans_rf) do
        if entry.key == key_rf and entry.sym == rf then
            entry.count = entry.count + w
            found = true
            break
        end
    end
    if not found and #self.trans_rf < 256 then
        table.insert(self.trans_rf, {key = key_rf, sym = rf, count = w})
    end

    local key_ra = (rc * 65536) + (rf * 256) + self.prev_ra
    found = false
    for _, entry in ipairs(self.trans_ra) do
        if entry.key == key_ra and entry.sym == ra then
            entry.count = entry.count + w
            found = true
            break
        end
    end
    if not found and #self.trans_ra < 256 then
        table.insert(self.trans_ra, {key = key_ra, sym = ra, count = w})
    end

    self.prev_rc = rc
    self.prev_rf = rf
    self.prev_ra = ra
end

function RadicalPredictor:get_cum_freqs_rc(prev_rc)
    local freqs = {}
    for i = 0, 255 do freqs[i] = self.alpha end
    for _, entry in ipairs(self.trans_rc) do
        if entry.key == prev_rc then
            freqs[entry.sym] = freqs[entry.sym] + entry.count
        end
    end
    local cum_freqs = {[0] = 0}
    for i = 0, 255 do
        cum_freqs[i+1] = cum_freqs[i] + freqs[i]
    end
    return cum_freqs
end

function RadicalPredictor:get_cum_freqs_rf(curr_rc, prev_rf)
    local freqs = {}
    for i = 0, 255 do freqs[i] = self.alpha end
    local key = (curr_rc * 256) + prev_rf
    for _, entry in ipairs(self.trans_rf) do
        if entry.key == key then
            freqs[entry.sym] = freqs[entry.sym] + entry.count
        end
    end
    local cum_freqs = {[0] = 0}
    for i = 0, 255 do
        cum_freqs[i+1] = cum_freqs[i] + freqs[i]
    end
    return cum_freqs
end

function RadicalPredictor:get_cum_freqs_ra(curr_rc, curr_rf, prev_ra)
    local freqs = {}
    for i = 0, 255 do freqs[i] = self.alpha end
    local key = (curr_rc * 65536) + (curr_rf * 256) + prev_ra
    for _, entry in ipairs(self.trans_ra) do
        if entry.key == key then
            freqs[entry.sym] = freqs[entry.sym] + entry.count
        end
    end
    local cum_freqs = {[0] = 0}
    for i = 0, 255 do
        cum_freqs[i+1] = cum_freqs[i] + freqs[i]
    end
    return cum_freqs
end

local BitReader = {}
BitReader.__index = BitReader

function BitReader.new(buffer)
    local self = setmetatable({}, BitReader)
    self.buffer = buffer
    self.bit_index = 0
    self.total_bits = #buffer * 8
    return self
end

function BitReader:read_bit()
    if self.bit_index >= self.total_bits then
        return 0
    end
    local byte_pos = math.floor(self.bit_index / 8) + 1
    local bit_pos = 7 - (self.bit_index % 8)
    local bit = math.floor(self.buffer[byte_pos] / (2 ^ bit_pos)) % 2
    self.bit_index = self.bit_index + 1
    return bit
end

local function read_varint(data, state)
    local val = 0
    local shift = 1
    while true do
        if state.pos > #data then break end
        local b = string.byte(data, state.pos)
        state.pos = state.pos + 1
        local val_part = b % 128
        val = val + val_part * shift
        if b < 128 then break end
        shift = shift * 128
    end
    return val
end

local function decompress_vocab(data, num_tokens)
    local tokens = {}
    local state = {pos = 1}
    local prev = ""
    for i = 1, num_tokens do
        if state.pos > #data then break end
        local common = read_varint(data, state)
        local suffix_len = read_varint(data, state)
        local suffix = string.sub(data, state.pos, state.pos + suffix_len - 1)
        state.pos = state.pos + suffix_len
        
        local prefix = string.sub(prev, 1, math.min(common, #prev))
        local token = prefix .. suffix
        table.insert(tokens, token)
        prev = token
    end
    return tokens
end

local function decode(encoded_bytes, num_concepts, alpha, weight)
    local pred = RadicalPredictor.new(alpha, weight)
    local r = BitReader.new(encoded_bytes)

    local value = 0
    for i = 1, 32 do
        value = ((value * 2) + r:read_bit()) % 0x100000000
    end

    local low = 0
    local high = 0xFFFFFFFF
    local decoded = {}

    for c_idx = 1, num_concepts do
        local prev_rc = pred.prev_rc
        local prev_rf = pred.prev_rf
        local prev_ra = pred.prev_ra
        local symbols = {[0] = 0, [1] = 0, [2] = 0}

        for step = 0, 2 do
            local cum_freqs
            if step == 0 then
                cum_freqs = pred:get_cum_freqs_rc(prev_rc)
            elseif step == 1 then
                cum_freqs = pred:get_cum_freqs_rf(symbols[0], prev_rf)
            else
                cum_freqs = pred:get_cum_freqs_ra(symbols[0], symbols[1], prev_ra)
            end

            local total = cum_freqs[256]
            local range_width = high - low + 1
            local scaled_val = math.floor((((value - low) + 1) * total - 1) / range_width)

            local sym = 0
            local l_idx, r_idx = 0, 255
            while l_idx <= r_idx do
                local m_idx = math.floor((l_idx + r_idx) / 2)
                if cum_freqs[m_idx] <= scaled_val and scaled_val < cum_freqs[m_idx + 1] then
                    sym = m_idx
                    break
                elseif scaled_val >= cum_freqs[m_idx + 1] then
                    l_idx = m_idx + 1
                else
                    r_idx = m_idx - 1
                end
            end

            symbols[step] = sym
            local cum_low = cum_freqs[sym]
            local cum_high = cum_freqs[sym + 1]

            high = low + math.floor((range_width * cum_high) / total) - 1
            low = low + math.floor((range_width * cum_low) / total)

            while true do
                if high < 0x80000000 then
                    low = (low * 2) % 0x100000000
                    high = ((high * 2) + 1) % 0x100000000
                    value = ((value * 2) + r:read_bit()) % 0x100000000
                elseif low >= 0x80000000 then
                    low = ((low - 0x80000000) * 2) % 0x100000000
                    high = (((high - 0x80000000) * 2) + 1) % 0x100000000
                    value = (((value - 0x80000000) * 2) + r:read_bit()) % 0x100000000
                elseif low >= 0x40000000 and high < 0xC0000000 then
                    low = ((low - 0x40000000) * 2) % 0x100000000
                    high = (((high - 0x40000000) * 2) + 1) % 0x100000000
                    value = (((value - 0x40000000) * 2) + r:read_bit()) % 0x100000000
                else
                    break
                end
            end
        end

        table.insert(decoded, {rc = symbols[0], rf = symbols[1], ra = symbols[2]})
        pred:observe(symbols[0], symbols[1], symbols[2])
    end
    return decoded
end

-- Load binary files
local function read_file(path)
    local f = io.open(path, "rb")
    if not f then return nil end
    local content = f:read("*all")
    f:close()
    return content
end

local names_data = read_file("Language-U-Browser/frameworks_names.bin") or read_file("frameworks_names.bin")
local coords_data = read_file("Language-U-Browser/frameworks_coordinates.bin") or read_file("frameworks_coordinates.bin")

if not names_data or not coords_data then
    print("[!] Error: Binary transport files not found. Run run_ultimate_pipeline.py first.")
    os.exit(1)
end

-- Decompress names
local names = decompress_vocab(names_data, 49)
print("[1] Lua Vocab Decompression: SUCCESS (" .. #names .. " names restored).")

-- Formulate expected coordinates
local expected = {}
for i, name in ipairs(names) do
    local domain = 1
    local lower = string.lower(name)
    if string.find(lower, "pixi") or string.find(lower, "phaser") or string.find(lower, "away") or string.find(lower, "p5") then
        domain = 2
    elseif string.find(lower, "scenejs") or string.find(lower, "glam") or string.find(lower, "deck") or string.find(lower, "cesium") or string.find(lower, "luma") or string.find(lower, "philo") then
        domain = 7
    end
    local rc = (domain * 16) + 2
    local rf = (1 * 16) + 2
    local ra = (15 * 16) + 12
    table.insert(expected, {rc = rc, rf = rf, ra = ra})
end

-- Convert coordinates data string to byte array
local coords_bytes = {}
for i = 1, #coords_data do
    coords_bytes[i] = string.byte(coords_data, i)
end

-- Range decode coordinates in Lua
local decoded = decode(coords_bytes, 49, 1, 128)
print("[2] Lua Yang Range Decoder execution: SUCCESS.")

-- Match check
local match_ok = true
for i = 1, 49 do
    if expected[i].rc ~= decoded[i].rc or expected[i].rf ~= decoded[i].rf or expected[i].ra ~= decoded[i].ra then
        print(string.format("[!] Mismatch at index %d (%s): Expected RC=%02X, RF=%02X, RA=%02X | Decoded RC=%02X, RF=%02X, RA=%02X",
            i-1, names[i], expected[i].rc, expected[i].rf, expected[i].ra, decoded[i].rc, decoded[i].rf, decoded[i].ra))
        match_ok = false
        break
    end
end

if match_ok then
    print("\n[SUCCESS] Lua range-decoder verification: 100% MATCH!")
else
    print("\n[ERROR] Lua dynamic coordinate check failed!")
    os.exit(1)
end
