#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT
#endif

#include "../cuneiform_u_v3.h"

EXPORT int cuneiform_u_v3_encode_dll(const Concept6D* concepts, uint32_t num_concepts,
                                 uint8_t* out_buffer, uint32_t out_max_bytes,
                                 uint32_t alpha, uint32_t weight) {
    return cuneiform_u_v3_encode(concepts, num_concepts, out_buffer, out_max_bytes, alpha, weight);
}

EXPORT int cuneiform_u_v3_decode_dll(const uint8_t* in_buffer, uint32_t in_bytes,
                                 Concept6D* out_concepts, uint32_t num_concepts,
                                 uint32_t alpha, uint32_t weight) {
    return cuneiform_u_v3_decode(in_buffer, in_bytes, out_concepts, num_concepts, alpha, weight);
}
