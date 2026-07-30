#ifndef LUMIA_RAW_BRIDGE_H
#define LUMIA_RAW_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#define LUMIA_RAW_API __declspec(dllexport)
#else
#define LUMIA_RAW_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define LUMIA_RAW_BRIDGE_ABI_VERSION 1u

typedef enum LumiaRawStatus {
  LUMIA_RAW_OK = 0,
  LUMIA_RAW_UNSUPPORTED = 1,
  LUMIA_RAW_CORRUPT = 2,
  LUMIA_RAW_RESOURCE_LIMIT = 3,
  LUMIA_RAW_DECODE_FAILED = 4,
  LUMIA_RAW_INVALID_ARGUMENT = 5
} LumiaRawStatus;

typedef struct LumiaRawProbe {
  uint32_t width;
  uint32_t height;
  double iso;
  double exposure_seconds;
  double aperture;
  double focal_length_mm;
  double latitude;
  double longitude;
  double altitude_meters;
  uint8_t gps_valid;
  uint8_t altitude_valid;
  uint8_t reserved[6];
  char camera_make[64];
  char camera_model[64];
  char lens[128];
  char date_taken[32];
} LumiaRawProbe;

typedef struct LumiaRawImage {
  const uint8_t *data;
  size_t data_len;
  void *owner;
  uint32_t width;
  uint32_t height;
  uint32_t stride;
  uint8_t channels;
  uint8_t bits_per_channel;
  uint8_t reserved[2];
} LumiaRawImage;

LUMIA_RAW_API uint32_t lumia_raw_bridge_abi_version(void);
LUMIA_RAW_API int32_t lumia_raw_bridge_probe(const uint8_t *path_utf8,
                                              size_t path_len,
                                              LumiaRawProbe *output);
LUMIA_RAW_API int32_t lumia_raw_bridge_decode(const uint8_t *path_utf8,
                                               size_t path_len,
                                               LumiaRawImage *output);
LUMIA_RAW_API void lumia_raw_bridge_free_image(LumiaRawImage *image);
LUMIA_RAW_API size_t lumia_raw_bridge_last_error(uint8_t *buffer,
                                                 size_t capacity);

#ifdef __cplusplus
}
#endif

#endif
