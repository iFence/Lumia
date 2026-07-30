#include "lumia_raw_bridge.h"

#include <libraw/libraw.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <exception>
#include <cstring>
#include <ctime>
#include <limits>
#include <memory>
#include <new>
#include <string>

#if defined(_WIN32)
#include <windows.h>
#endif

#if LIBRAW_MAJOR_VERSION != 0 || LIBRAW_MINOR_VERSION != 22 ||                 \
    LIBRAW_PATCH_VERSION != 2
#error "lumia_raw_bridge requires LibRaw 0.22.2 headers"
#endif

namespace {
constexpr unsigned kMaxRawMemoryMegabytes = 512;
constexpr size_t kMaxProcessedBytes = 512ULL * 1024ULL * 1024ULL;
thread_local std::string last_error;

struct LibRawCloser {
  void operator()(libraw_data_t *processor) const noexcept {
    if (processor != nullptr) {
      libraw_close(processor);
    }
  }
};
using Processor = std::unique_ptr<libraw_data_t, LibRawCloser>;

int32_t fail(int32_t status, const std::string &message) {
  last_error = message;
  return status;
}

int32_t fail_libraw(const char *operation, int error) {
  const char *detail = libraw_strerror(error);
  const std::string message = std::string(operation) + ": " +
                              (detail != nullptr ? detail : "unknown error");
  switch (error) {
  case LIBRAW_FILE_UNSUPPORTED:
  case LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE:
  case LIBRAW_NOT_IMPLEMENTED:
    return fail(LUMIA_RAW_UNSUPPORTED, message);
  case LIBRAW_UNSUFFICIENT_MEMORY:
  case LIBRAW_TOO_BIG:
  case LIBRAW_MEMPOOL_OVERFLOW:
    return fail(LUMIA_RAW_RESOURCE_LIMIT, message);
  case LIBRAW_DATA_ERROR:
  case LIBRAW_IO_ERROR:
  case LIBRAW_BAD_CROP:
    return fail(LUMIA_RAW_CORRUPT, message);
  default:
    return fail(LUMIA_RAW_DECODE_FAILED, message);
  }
}

bool checked_image_bytes(uint32_t width, uint32_t height, size_t *bytes) {
  if (width == 0 || height == 0 ||
      width > std::numeric_limits<uint32_t>::max() / 3) {
    return false;
  }
  const uint64_t total = static_cast<uint64_t>(width) * height * 3;
  if (total > kMaxProcessedBytes || total > std::numeric_limits<size_t>::max()) {
    return false;
  }
  *bytes = static_cast<size_t>(total);
  return true;
}

bool path_from_utf8(const uint8_t *bytes, size_t length, std::string *path) {
  if (bytes == nullptr || length == 0 ||
      std::find(bytes, bytes + length, uint8_t{0}) != bytes + length) {
    return false;
  }
  path->assign(reinterpret_cast<const char *>(bytes), length);
  return true;
}

#if defined(_WIN32)
bool wide_path(const std::string &utf8, std::wstring *path) {
  if (utf8.size() > static_cast<size_t>(std::numeric_limits<int>::max())) {
    return false;
  }
  const int required = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                                            utf8.data(),
                                            static_cast<int>(utf8.size()),
                                            nullptr, 0);
  if (required <= 0) {
    return false;
  }
  path->resize(static_cast<size_t>(required));
  return MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8.data(),
                             static_cast<int>(utf8.size()), path->data(),
                             required) == required;
}
#endif

int open_input(libraw_data_t *processor, const std::string &path) {
#if defined(_WIN32)
  std::wstring native_path;
  if (!wide_path(path, &native_path)) {
    return LIBRAW_IO_ERROR;
  }
  return libraw_open_wfile(processor, native_path.c_str());
#else
  return libraw_open_file(processor, path.c_str());
#endif
}

Processor create_processor() {
  Processor processor(libraw_init(0));
  if (processor != nullptr) {
    processor->rawparams.max_raw_memory_mb = kMaxRawMemoryMegabytes;
    processor->params.use_camera_wb = 1;
    processor->params.output_color = LIBRAW_COLORSPACE_sRGB;
    processor->params.output_bps = 8;
    processor->params.half_size = 1;
    processor->params.user_flip = -1;
  }
  return processor;
}

template <size_t DestinationSize, size_t SourceSize>
void copy_text(char (&destination)[DestinationSize],
               const char (&source)[SourceSize]) {
  const size_t length = static_cast<size_t>(
      std::find(source, source + SourceSize, '\0') - source);
  const size_t copied = std::min(length, DestinationSize - 1);
  std::memcpy(destination, source, copied);
  destination[copied] = '\0';
}

double coordinates_to_decimal(const float value[3], char reference) {
  double result = std::abs(static_cast<double>(value[0])) +
                  std::abs(static_cast<double>(value[1])) / 60.0 +
                  std::abs(static_cast<double>(value[2])) / 3600.0;
  if (reference == 'S' || reference == 's' || reference == 'W' ||
      reference == 'w' || value[0] < 0) {
    result = -result;
  }
  return result;
}

void copy_date(time_t timestamp, char (&output)[32]) {
  if (timestamp <= 0) {
    return;
  }
  std::tm value{};
#if defined(_WIN32)
  if (gmtime_s(&value, &timestamp) != 0) {
    return;
  }
#else
  if (gmtime_r(&timestamp, &value) == nullptr) {
    return;
  }
#endif
  std::strftime(output, sizeof(output), "%Y-%m-%dT%H:%M:%SZ", &value);
}

void fill_probe(const libraw_data_t &data, LumiaRawProbe *output) {
  uint32_t width = data.sizes.width;
  uint32_t height = data.sizes.height;
  if (data.sizes.flip == 5 || data.sizes.flip == 6) {
    std::swap(width, height);
  }
  output->width = width;
  output->height = height;
  output->iso = data.other.iso_speed;
  output->exposure_seconds = data.other.shutter;
  output->aperture = data.other.aperture;
  output->focal_length_mm = data.other.focal_len;
  copy_text(output->camera_make, data.idata.make);
  copy_text(output->camera_model, data.idata.model);
  copy_text(output->lens, data.lens.Lens);
  copy_date(data.other.timestamp, output->date_taken);

  const libraw_gps_info_t &gps = data.other.parsed_gps;
  if (gps.gpsparsed != 0) {
    const double latitude = coordinates_to_decimal(gps.latitude, gps.latref);
    const double longitude = coordinates_to_decimal(gps.longitude, gps.longref);
    if (std::isfinite(latitude) && std::isfinite(longitude) &&
        std::abs(latitude) <= 90.0 && std::abs(longitude) <= 180.0) {
      output->latitude = latitude;
      output->longitude = longitude;
      output->gps_valid = 1;
    }
    if (std::isfinite(gps.altitude)) {
      output->altitude_meters = gps.altref != 0 ? -gps.altitude : gps.altitude;
      output->altitude_valid = 1;
    }
  }
}

int32_t prepare(const uint8_t *path_utf8, size_t path_len,
                Processor *processor) {
  std::string path;
  if (!path_from_utf8(path_utf8, path_len, &path)) {
    return fail(LUMIA_RAW_INVALID_ARGUMENT, "input path is invalid UTF-8");
  }
  *processor = create_processor();
  if (*processor == nullptr) {
    return fail(LUMIA_RAW_RESOURCE_LIMIT, "LibRaw could not allocate a processor");
  }
  const int status = open_input(processor->get(), path);
  if (status != LIBRAW_SUCCESS) {
    return fail_libraw("open RAW input", status);
  }
  return LUMIA_RAW_OK;
}
} // namespace

extern "C" {

uint32_t lumia_raw_bridge_abi_version(void) {
  return libraw_versionNumber() == LIBRAW_VERSION
             ? LUMIA_RAW_BRIDGE_ABI_VERSION
             : 0;
}

int32_t lumia_raw_bridge_probe(const uint8_t *path_utf8, size_t path_len,
                               LumiaRawProbe *output) {
  last_error.clear();
  if (output == nullptr) {
    return fail(LUMIA_RAW_INVALID_ARGUMENT, "probe output is null");
  }
  std::memset(output, 0, sizeof(*output));
  try {
    Processor processor;
    const int32_t status = prepare(path_utf8, path_len, &processor);
    if (status != LUMIA_RAW_OK) {
      return status;
    }
    fill_probe(*processor, output);
    if (output->width == 0 || output->height == 0) {
      return fail(LUMIA_RAW_CORRUPT, "RAW input has invalid dimensions");
    }
    return LUMIA_RAW_OK;
  } catch (const std::bad_alloc &) {
    return fail(LUMIA_RAW_RESOURCE_LIMIT, "RAW metadata exceeded memory limits");
  } catch (const std::exception &error) {
    return fail(LUMIA_RAW_DECODE_FAILED, error.what());
  } catch (...) {
    return fail(LUMIA_RAW_DECODE_FAILED, "unknown RAW probe failure");
  }
}

int32_t lumia_raw_bridge_decode(const uint8_t *path_utf8, size_t path_len,
                                LumiaRawImage *output) {
  last_error.clear();
  if (output == nullptr) {
    return fail(LUMIA_RAW_INVALID_ARGUMENT, "image output is null");
  }
  std::memset(output, 0, sizeof(*output));
  try {
    Processor processor;
    int32_t status = prepare(path_utf8, path_len, &processor);
    if (status != LUMIA_RAW_OK) {
      return status;
    }
    int libraw_status = libraw_unpack(processor.get());
    if (libraw_status != LIBRAW_SUCCESS) {
      return fail_libraw("unpack RAW input", libraw_status);
    }
    libraw_status = libraw_dcraw_process(processor.get());
    if (libraw_status != LIBRAW_SUCCESS) {
      return fail_libraw("develop RAW preview", libraw_status);
    }
    int memory_status = LIBRAW_SUCCESS;
    libraw_processed_image_t *image =
        libraw_dcraw_make_mem_image(processor.get(), &memory_status);
    if (image == nullptr || memory_status != LIBRAW_SUCCESS) {
      if (image != nullptr) {
        libraw_dcraw_clear_mem(image);
      }
      return fail_libraw("create RAW preview bitmap", memory_status);
    }
    size_t expected = 0;
    if (image->type != LIBRAW_IMAGE_BITMAP || image->colors != 3 ||
        image->bits != 8 ||
        !checked_image_bytes(image->width, image->height, &expected) ||
        expected != image->data_size) {
      libraw_dcraw_clear_mem(image);
      return fail(LUMIA_RAW_RESOURCE_LIMIT,
                  "LibRaw returned an invalid or oversized preview bitmap");
    }
    output->data = image->data;
    output->data_len = image->data_size;
    output->owner = image;
    output->width = image->width;
    output->height = image->height;
    output->stride = image->width * 3;
    output->channels = 3;
    output->bits_per_channel = 8;
    return LUMIA_RAW_OK;
  } catch (const std::bad_alloc &) {
    return fail(LUMIA_RAW_RESOURCE_LIMIT, "RAW preview exceeded memory limits");
  } catch (const std::exception &error) {
    return fail(LUMIA_RAW_DECODE_FAILED, error.what());
  } catch (...) {
    return fail(LUMIA_RAW_DECODE_FAILED, "unknown RAW decode failure");
  }
}

void lumia_raw_bridge_free_image(LumiaRawImage *image) {
  if (image == nullptr) {
    return;
  }
  if (image->owner != nullptr) {
    libraw_dcraw_clear_mem(
        static_cast<libraw_processed_image_t *>(image->owner));
  }
  std::memset(image, 0, sizeof(*image));
}

size_t lumia_raw_bridge_last_error(uint8_t *buffer, size_t capacity) {
  if (buffer == nullptr || capacity == 0) {
    return last_error.size();
  }
  const size_t length = std::min(last_error.size(), capacity - 1);
  std::memcpy(buffer, last_error.data(), length);
  buffer[length] = 0;
  return length;
}

} // extern "C"
