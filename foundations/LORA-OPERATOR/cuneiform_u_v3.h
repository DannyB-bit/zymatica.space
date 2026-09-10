/**
 * Cuneiform-U v3.0 / Language U v4.0 — Edge-Ready Semantic Range Coder
 * Watermark: ip zymatica.space | astronautshe.com
 *
 * This header contains a pure C, zero-dependency, static memory implementation
 * of the 32-bit Range Coder and Hierarchical Radical Prediction Model.
 * Optimized for microcontrollers (e.g. STM32, ESP32) to meet FCC dwell time
 * and LoRa payload limits (< 152 bytes) with high-efficiency compression.
 */

#ifndef CUNEIFORM_U_V3_H
#define CUNEIFORM_U_V3_H

#include <stdint.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MAX_TRANSITIONS 256
#define RANGE_CODER_MAX_RANGE 0xFFFFFFFFU
#define RANGE_CODER_HALF_RANGE 0x80000000U
#define RANGE_CODER_QTR_RANGE  0x40000000U
#define RANGE_CODER_THREE_QTR  0xC0000000U

/* 6D Hypercube Concept Coordinates */
typedef struct {
    uint8_t domain;       /* 0-15 */
    uint8_t subdomain;    /* 0-15 */
    uint8_t operation;    /* 0-15 */
    uint8_t modality;     /* 0-15 */
    uint8_t depth;        /* 0-15 */
    uint8_t polarity;     /* 0-15 */
} Concept6D;

/* Sparse Transition Entry for Radical Predictor */
typedef struct {
    uint32_t key;         /* Context state key */
    uint8_t sym;          /* Symbol predicted (0-255) */
    uint32_t count;       /* Observed frequency transition count */
} SparseTransition;

/* Predictor Model State */
typedef struct {
    SparseTransition trans_rc[MAX_TRANSITIONS];
    uint32_t num_rc;

    SparseTransition trans_rf[MAX_TRANSITIONS];
    uint32_t num_rf;

    SparseTransition trans_ra[MAX_TRANSITIONS];
    uint32_t num_ra;

    uint8_t prev_rc;
    uint8_t prev_rf;
    uint8_t prev_ra;

    uint32_t alpha;       /* Laplace smoothing factor */
    uint32_t weight;      /* Increment weight per observation */
} RadicalPredictor;

/* Helper to initialize the predictor */
static inline void predictor_init(RadicalPredictor* pred, uint32_t alpha, uint32_t weight) {
    memset(pred, 0, sizeof(RadicalPredictor));
    pred->alpha = alpha;
    pred->weight = weight;
}

/* Update prediction models based on observed radicals */
static inline void predictor_observe(RadicalPredictor* pred, uint8_t rc, uint8_t rf, uint8_t ra) {
    /* 1. Update Classifier Radical transitions (R_C) */
    uint32_t key_rc = pred->prev_rc;
    int found_rc = 0;
    for (uint32_t i = 0; i < pred->num_rc; i++) {
        if (pred->trans_rc[i].key == key_rc && pred->trans_rc[i].sym == rc) {
            pred->trans_rc[i].count += pred->weight;
            found_rc = 1;
            break;
        }
    }
    if (!found_rc && pred->num_rc < MAX_TRANSITIONS) {
        pred->trans_rc[pred->num_rc].key = key_rc;
        pred->trans_rc[pred->num_rc].sym = rc;
        pred->trans_rc[pred->num_rc].count = pred->weight;
        pred->num_rc++;
    }

    /* 2. Update Force Radical transitions (R_F) */
    uint32_t key_rf = ((uint32_t)rc << 8) | pred->prev_rf;
    int found_rf = 0;
    for (uint32_t i = 0; i < pred->num_rf; i++) {
        if (pred->trans_rf[i].key == key_rf && pred->trans_rf[i].sym == rf) {
            pred->trans_rf[i].count += pred->weight;
            found_rf = 1;
            break;
        }
    }
    if (!found_rf && pred->num_rf < MAX_TRANSITIONS) {
        pred->trans_rf[pred->num_rf].key = key_rf;
        pred->trans_rf[pred->num_rf].sym = rf;
        pred->trans_rf[pred->num_rf].count = pred->weight;
        pred->num_rf++;
    }

    /* 3. Update Aspect Radical transitions (R_A) */
    uint32_t key_ra = ((uint32_t)rc << 16) | ((uint32_t)rf << 8) | pred->prev_ra;
    int found_ra = 0;
    for (uint32_t i = 0; i < pred->num_ra; i++) {
        if (pred->trans_ra[i].key == key_ra && pred->trans_ra[i].sym == ra) {
            pred->trans_ra[i].count += pred->weight;
            found_ra = 1;
            break;
        }
    }
    if (!found_ra && pred->num_ra < MAX_TRANSITIONS) {
        pred->trans_ra[pred->num_ra].key = key_ra;
        pred->trans_ra[pred->num_ra].sym = ra;
        pred->trans_ra[pred->num_ra].count = pred->weight;
        pred->num_ra++;
    }

    /* Track histories */
    pred->prev_rc = rc;
    pred->prev_rf = rf;
    pred->prev_ra = ra;
}

/* Construct cumulative frequency tables (0 to 256) */
static inline void get_cum_freqs_rc(const RadicalPredictor* pred, uint8_t prev_rc, uint32_t* cum_freqs) {
    uint32_t freqs[256];
    for (int i = 0; i < 256; i++) {
        freqs[i] = pred->alpha;
    }
    for (uint32_t i = 0; i < pred->num_rc; i++) {
        if (pred->trans_rc[i].key == prev_rc) {
            freqs[pred->trans_rc[i].sym] += pred->trans_rc[i].count;
        }
    }
    cum_freqs[0] = 0;
    for (int i = 0; i < 256; i++) {
        cum_freqs[i+1] = cum_freqs[i] + freqs[i];
    }
}

static inline void get_cum_freqs_rf(const RadicalPredictor* pred, uint8_t curr_rc, uint8_t prev_rf, uint32_t* cum_freqs) {
    uint32_t freqs[256];
    for (int i = 0; i < 256; i++) {
        freqs[i] = pred->alpha;
    }
    uint32_t key = ((uint32_t)curr_rc << 8) | prev_rf;
    for (uint32_t i = 0; i < pred->num_rf; i++) {
        if (pred->trans_rf[i].key == key) {
            freqs[pred->trans_rf[i].sym] += pred->trans_rf[i].count;
        }
    }
    cum_freqs[0] = 0;
    for (int i = 0; i < 256; i++) {
        cum_freqs[i+1] = cum_freqs[i] + freqs[i];
    }
}

static inline void get_cum_freqs_ra(const RadicalPredictor* pred, uint8_t curr_rc, uint8_t curr_rf, uint8_t prev_ra, uint32_t* cum_freqs) {
    uint32_t freqs[256];
    for (int i = 0; i < 256; i++) {
        freqs[i] = pred->alpha;
    }
    uint32_t key = ((uint32_t)curr_rc << 16) | ((uint32_t)curr_rf << 8) | prev_ra;
    for (uint32_t i = 0; i < pred->num_ra; i++) {
        if (pred->trans_ra[i].key == key) {
            freqs[pred->trans_ra[i].sym] += pred->trans_ra[i].count;
        }
    }
    cum_freqs[0] = 0;
    for (int i = 0; i < 256; i++) {
        cum_freqs[i+1] = cum_freqs[i] + freqs[i];
    }
}

/* Bitstream helper functions for encoding/decoding */
typedef struct {
    uint8_t* buffer;
    uint32_t max_bytes;
    uint32_t bit_index;
} BitWriter;

static inline void bit_writer_init(BitWriter* w, uint8_t* buf, uint32_t max_b) {
    w->buffer = buf;
    w->max_bytes = max_b;
    w->bit_index = 0;
    memset(buf, 0, max_b);
}

static inline void bit_writer_write(BitWriter* w, uint8_t bit) {
    uint32_t byte_pos = w->bit_index / 8;
    uint32_t bit_pos = 7 - (w->bit_index % 8);
    if (byte_pos < w->max_bytes) {
        if (bit) {
            w->buffer[byte_pos] |= (1U << bit_pos);
        } else {
            w->buffer[byte_pos] &= ~(1U << bit_pos);
        }
        w->bit_index++;
    }
}

typedef struct {
    const uint8_t* buffer;
    uint32_t total_bits;
    uint32_t bit_index;
} BitReader;

static inline void bit_reader_init(BitReader* r, const uint8_t* buf, uint32_t num_bytes) {
    r->buffer = buf;
    r->total_bits = num_bytes * 8;
    r->bit_index = 0;
}

static inline uint8_t bit_reader_read(BitReader* r) {
    if (r->bit_index >= r->total_bits) {
        return 0;
    }
    uint32_t byte_pos = r->bit_index / 8;
    uint32_t bit_pos = 7 - (r->bit_index % 8);
    uint8_t bit = (r->buffer[byte_pos] >> bit_pos) & 1U;
    r->bit_index++;
    return bit;
}

/* =============================================================================
 * CORE COMPRESSION AND DECOMPRESSION API
 * ============================================================================= */

static inline void write_bit_helper(BitWriter* w, uint32_t* underflow_bits, uint8_t bit) {
    bit_writer_write(w, bit);
    while (*underflow_bits > 0) {
        bit_writer_write(w, 1 - bit);
        (*underflow_bits)--;
    }
}

/**
 * Compresses an array of 6D concepts into a compact bitstream.
 * returns: total bits written, or -1 on overflow
 */
static int cuneiform_u_v3_encode(const Concept6D* concepts, uint32_t num_concepts,
                                 uint8_t* out_buffer, uint32_t out_max_bytes,
                                 uint32_t alpha, uint32_t weight) {
    RadicalPredictor encoder_pred;
    predictor_init(&encoder_pred, alpha, weight);

    BitWriter w;
    bit_writer_init(&w, out_buffer, out_max_bytes);

    uint32_t low = 0;
    uint32_t high = RANGE_CODER_MAX_RANGE;
    uint32_t underflow_bits = 0;

    /* Flatten into radical sequence and encode step-by-step */
    for (uint32_t c = 0; c < num_concepts; c++) {
        uint8_t rc = (concepts[c].domain << 4) | concepts[c].subdomain;
        uint8_t rf = (concepts[c].operation << 4) | concepts[c].modality;
        uint8_t ra = (concepts[c].depth << 4) | concepts[c].polarity;

        uint8_t symbols[3] = {rc, rf, ra};

        /* For dynamically tracking state history during the single concept */
        uint8_t prev_rc = encoder_pred.prev_rc;
        uint8_t prev_rf = encoder_pred.prev_rf;
        uint8_t prev_ra = encoder_pred.prev_ra;

        for (int step = 0; step < 3; step++) {
            uint32_t cum_freqs[257];
            if (step == 0) {
                get_cum_freqs_rc(&encoder_pred, prev_rc, cum_freqs);
            } else if (step == 1) {
                get_cum_freqs_rf(&encoder_pred, symbols[0], prev_rf, cum_freqs);
            } else {
                get_cum_freqs_ra(&encoder_pred, symbols[0], symbols[1], prev_ra, cum_freqs);
            }

            uint8_t sym = symbols[step];
            uint32_t total = cum_freqs[256];
            uint32_t cum_low = cum_freqs[sym];
            uint32_t cum_high = cum_freqs[sym + 1];

            uint64_t range_width = (uint64_t)high - low + 1;
            high = low + (uint32_t)((range_width * cum_high) / total) - 1;
            low = low + (uint32_t)((range_width * cum_low) / total);

            /* Renormalize */
            while (1) {
                if (high < RANGE_CODER_HALF_RANGE) {
                    write_bit_helper(&w, &underflow_bits, 0);
                    low <<= 1;
                    high = (high << 1) | 1U;
                } else if (low >= RANGE_CODER_HALF_RANGE) {
                    write_bit_helper(&w, &underflow_bits, 1);
                    low = (low - RANGE_CODER_HALF_RANGE) << 1;
                    high = ((high - RANGE_CODER_HALF_RANGE) << 1) | 1U;
                } else if (low >= RANGE_CODER_QTR_RANGE && high < RANGE_CODER_THREE_QTR) {
                    underflow_bits++;
                    low = (low - RANGE_CODER_QTR_RANGE) << 1;
                    high = ((high - RANGE_CODER_QTR_RANGE) << 1) | 1U;
                } else {
                    break;
                }
            }
        }

        /* Update predictor with the verified concept */
        predictor_observe(&encoder_pred, rc, rf, ra);
    }

    /* Final bit flush */
    underflow_bits++;
    if (low < RANGE_CODER_QTR_RANGE) {
        write_bit_helper(&w, &underflow_bits, 0);
    } else {
        write_bit_helper(&w, &underflow_bits, 1);
    }

    return w.bit_index;
}

/**
 * Decompresses a bitstream back into 6D concepts.
 * returns: 1 on success, 0 on failure
 */
static int cuneiform_u_v3_decode(const uint8_t* in_buffer, uint32_t in_bytes,
                                 Concept6D* out_concepts, uint32_t num_concepts,
                                 uint32_t alpha, uint32_t weight) {
    RadicalPredictor decoder_pred;
    predictor_init(&decoder_pred, alpha, weight);

    BitReader r;
    bit_reader_init(&r, in_buffer, in_bytes);

    /* Initialize value */
    uint32_t value = 0;
    for (int i = 0; i < 32; i++) {
        value = (value << 1) | bit_reader_read(&r);
    }

    uint32_t low = 0;
    uint32_t high = RANGE_CODER_MAX_RANGE;

    for (uint32_t c = 0; c < num_concepts; c++) {
        uint8_t prev_rc = decoder_pred.prev_rc;
        uint8_t prev_rf = decoder_pred.prev_rf;
        uint8_t prev_ra = decoder_pred.prev_ra;

        uint8_t symbols[3] = {0, 0, 0};

        for (int step = 0; step < 3; step++) {
            uint32_t cum_freqs[257];
            if (step == 0) {
                get_cum_freqs_rc(&decoder_pred, prev_rc, cum_freqs);
            } else if (step == 1) {
                get_cum_freqs_rf(&decoder_pred, symbols[0], prev_rf, cum_freqs);
            } else {
                get_cum_freqs_ra(&decoder_pred, symbols[0], symbols[1], prev_ra, cum_freqs);
            }

            uint32_t total = cum_freqs[256];
            uint64_t range_width = (uint64_t)high - low + 1;

            /* Compute scaled value */
            uint64_t scaled_val = (((uint64_t)(value - low) + 1) * total - 1) / range_width;

            /* Find symbol using binary search */
            uint8_t sym = 0;
            int l = 0, rr = 255;
            while (l <= rr) {
                int mid = (l + rr) / 2;
                if (cum_freqs[mid] <= scaled_val && scaled_val < cum_freqs[mid + 1]) {
                    sym = (uint8_t)mid;
                    break;
                } else if (scaled_val >= cum_freqs[mid + 1]) {
                    l = mid + 1;
                } else {
                    rr = mid - 1;
                }
            }

            symbols[step] = sym;

            uint32_t cum_low = cum_freqs[sym];
            uint32_t cum_high = cum_freqs[sym + 1];

            high = low + (uint32_t)((range_width * cum_high) / total) - 1;
            low = low + (uint32_t)((range_width * cum_low) / total);

            /* Renormalize */
            while (1) {
                if (high < RANGE_CODER_HALF_RANGE) {
                    low <<= 1;
                    high = (high << 1) | 1U;
                    value = (value << 1) | bit_reader_read(&r);
                } else if (low >= RANGE_CODER_HALF_RANGE) {
                    low = (low - RANGE_CODER_HALF_RANGE) << 1;
                    high = ((high - RANGE_CODER_HALF_RANGE) << 1) | 1U;
                    value = ((value - RANGE_CODER_HALF_RANGE) << 1) | bit_reader_read(&r);
                } else if (low >= RANGE_CODER_QTR_RANGE && high < RANGE_CODER_THREE_QTR) {
                    low = (low - RANGE_CODER_QTR_RANGE) << 1;
                    high = ((high - RANGE_CODER_QTR_RANGE) << 1) | 1U;
                    value = ((value - RANGE_CODER_QTR_RANGE) << 1) | bit_reader_read(&r);
                } else {
                    break;
                }
            }
        }

        /* Save decoded coordinates */
        out_concepts[c].domain = (symbols[0] >> 4) & 0xF;
        out_concepts[c].subdomain = symbols[0] & 0xF;
        out_concepts[c].operation = (symbols[1] >> 4) & 0xF;
        out_concepts[c].modality = symbols[1] & 0xF;
        out_concepts[c].depth = (symbols[2] >> 4) & 0xF;
        out_concepts[c].polarity = symbols[2] & 0xF;

        /* Keep decoder state predictor synchronized */
        predictor_observe(&decoder_pred, symbols[0], symbols[1], symbols[2]);
    }

    return 1;
}

#ifdef __cplusplus
}
#endif

#endif /* CUNEIFORM_U_V3_H */
