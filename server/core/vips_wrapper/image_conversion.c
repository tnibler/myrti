#include <vips/vips.h>
#include <vips/conversion.h>
#include <vips/error.h>
#include <vips/foreign.h>
#include <vips/resample.h>
#include <vips/memory.h>
#include <vips/image.h>
#include "vips_wrapper.h"

int save_jpeg(VipsImage* img, const char* out_path, JpegSaveParams params) {
  return vips_jpegsave(img, out_path, "Q", params.quality, NULL);
}

int save_heif(VipsImage* img, const char* out_path, HeifSaveParams params) {
  return vips_heifsave(img, out_path, "Q", params.quality,
      "bitdepth", params.bit_depth,
      "lossless", params.lossless,
      "compression", params.compression,
      NULL);
}

int save_webp(VipsImage* img, const char* out_path) {
  return vips_webpsave(img, out_path, NULL);
}

ConvertHeifResult convert_heif(const char * in_path, const char * out_path, HeifSaveParams params, Scale scale) {
  VipsImage* img = NULL;
  ConvertHeifResult result = {
    .width = 0,
    .height = 0,
    .err = 0
  };
  img = vips_image_new_from_file(in_path, NULL);
  if (img == NULL) {
    printf("libvips error: %s", vips_error_buffer());
    result.err = 1;
    return result;
  }
  if (scale.do_scale) {
    // TODO premultiple alpha, resample in linear colorspace, autorot as explained in
    // https://github.com/libvips/libvips/wiki/HOWTO----Image-shrinking
    VipsImage* scaled;
    int ret = vips_resize(img, &scaled, scale.scale, NULL);
    if (scaled == NULL || ret != 0) {
      printf("libvips error: %s", vips_error_buffer());
      result.err = 1;
      return result;
    }
    g_object_unref(img);
    result.width = scaled->Xsize;
    result.height = scaled->Ysize;
    img = scaled;
  }
  result.err = save_heif(img, out_path, params);
  g_object_unref(img);
  return result;
}

ConvertJpegResult convert_jpeg(const char * in_path, const char * out_path, JpegSaveParams params, Scale scale) {
  VipsImage* img = NULL;
  ConvertJpegResult result = {
    .width = 0,
    .height = 0,
    .err = 0
  };
  img = vips_image_new_from_file(in_path, NULL);
  if (img == NULL) {
    printf("libvips error: %s", vips_error_buffer());
    result.err = 1;
    return result;
  }
  if (scale.do_scale) {
    VipsImage* scaled;
    int ret = vips_resize(img, &scaled, scale.scale, NULL);
    if (scaled == NULL || ret != 0) {
      printf("libvips error: %s", vips_error_buffer());
      result.err = 1;
      return result;
    }
    result.width = scaled->Xsize;
    result.height = scaled->Ysize;
    g_object_unref(img);
    img = scaled;
  }
  result.err = save_jpeg(img, out_path, params);
  g_object_unref(img);
  return result;
}

int save_test_heif_image(const char* out_path, HeifSaveParams params) {
  VipsImage* img = NULL;
  int width = 400;
  int height = 400;
  int err = vips_black(&img, width, height, "bands", 3, NULL);
  if (err != 0) {
    return err;
  } 
  err = save_heif(img, out_path, params);
  g_object_unref(img);
  return err;
}

int save_test_jpeg_image(const char* out_path, JpegSaveParams params) {
  VipsImage* img = NULL;
  int width = 400;
  int height = 400;
  int err = vips_black(&img, width, height, "bands", 3, NULL);
  if (err != 0) {
    return err;
  } 
  err = save_jpeg(img, out_path, params);
  g_object_unref(img);
  return err;
}

int save_test_webp_image(const char* out_path) {
  VipsImage* img = NULL;
  int width = 400;
  int height = 400;
  int err = vips_black(&img, width, height, "bands", 3, NULL);
  if (err != 0) {
    return err;
  } 
  err = save_webp(img, out_path);
  g_object_unref(img);
  return err;
}
