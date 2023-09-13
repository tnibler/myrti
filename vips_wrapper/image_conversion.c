#include <assert.h>
#include <vips/vips.h>
#include <vips/conversion.h>
#include <vips/error.h>
#include <vips/foreign.h>
#include <vips/resample.h>
#include <vips/memory.h>
#include <vips/image.h>
#include "vips_wrapper.h"

int convert_heif(const char * in_path, const char * out_path, HeifSaveParams params) {
  VipsImage* img = NULL;
  img = vips_image_new_from_file(in_path, NULL);
  if (img == NULL) {
    printf("libvips error: %s", vips_error_buffer());
    return 1;
  }
  int ret = vips_heifsave(img, out_path, "Q", params.quality,
      "bitdepth", params.bit_depth,
      "lossless", params.lossless,
      "compression", params.compression,
      NULL);
  g_object_unref(img);
  return ret;
}

int convert_jpeg(const char * in_path, const char * out_path, JpegSaveParams params) {
  VipsImage* img = NULL;
  img = vips_image_new_from_file(in_path, NULL);
  if (img == NULL) {
    printf("libvips error: %s", vips_error_buffer());
    return 1;
  }
  int ret = vips_jpegsave(img, out_path, "Q", params.quality, NULL);
  g_object_unref(img);
  return ret;
}
