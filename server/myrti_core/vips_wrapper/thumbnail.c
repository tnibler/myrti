#include <assert.h>
#include <vips/vips.h>
#include <vips/conversion.h>
#include <vips/error.h>
#include <vips/foreign.h>
#include <vips/resample.h>
#include <vips/memory.h>
#include <vips/image.h>
#include "vips_wrapper.h"


int init() {
  int ret = VIPS_INIT("vips_wrapper"); 
  vips_cache_set_max(0);
  return ret;
}

void teardown() { vips_shutdown(); }

int thumbnail(ThumbnailParams params, ThumbnailResult* result) {
  if (result == NULL) {
    return -1;
  }

  for (unsigned long long i = 0; i < params.num_out_paths; ++i) {
    VipsImage* out = NULL;
    int ret;
    if (params.keep_aspect) {
       ret = vips_thumbnail(params.in_path, &out, params.width, NULL);
    } else {
       ret = vips_thumbnail(params.in_path, &out, params.width, "height", params.height, "crop", VIPS_INTERESTING_ATTENTION, NULL);
    }
    if (ret) {
      printf("libvips error: %s", vips_error_buffer());
      if (out != NULL) {
        g_object_unref(out);
      }
      return ret;
    }
    assert(out);
    ret = vips_image_write_to_file(out, params.out_paths[i], NULL);
    result->actual_width = out->Xsize;
    result->actual_height = out->Ysize;
    if (out != NULL) {
      g_object_unref(out);
    }
    if (ret) {
      printf("libvips error: %s", vips_error_buffer());
      return ret;
    }
  }
  return 0;
}

// Create a thumbnail in RGBA, converting if necessary
int thumbnail_for_thumbhash(const char* path, int width, ImageBuffer* out) {
  if (path == NULL || out == NULL) {
    return -1;
  }
  VipsImage* image = NULL;
  int ret = 0;

  if ((ret = vips_thumbnail(path, &image, width, NULL)) != 0) {
    goto cleanup;
  }

  if (!vips_image_hasalpha(image)) {
    VipsImage* image_alpha = NULL;
    if ((ret = vips_addalpha(image, &image_alpha, NULL)) != 0) {
      goto cleanup;
    }
    g_object_unref(image);
    image = image_alpha;
  }
  if (vips_image_get_bands(image) != 4) {
    VipsImage* image_rgba = NULL;
    if ((ret = vips_colourspace(image, &image_rgba, VIPS_INTERPRETATION_sRGB, NULL)) != 0) {
      goto cleanup;
    }
    g_object_unref(image);
    image = image_rgba;
  }
  unsigned long size;
  const char* buf = vips_image_write_to_memory(image, &size);
  if (buf == NULL) {
      ret = -1;
      goto cleanup;
  }
  out->width = image->Xsize;
  out->height = image->Ysize;
  out->size = size;
  out->buf = buf;

cleanup:
  g_object_unref(image);
  return ret;
}

void free_image_buffer(ImageBuffer buf) {
  g_free((void*)buf.buf);
}
