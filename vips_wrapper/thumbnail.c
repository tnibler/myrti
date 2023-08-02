#include <vips/vips.h>

#include <vips/conversion.h>
#include <vips/error.h>
#include <vips/foreign.h>
#include <vips/resample.h>
#include <vips/memory.h>
#include <vips/image.h>
#include "vips_wrapper.h"


int init() {
  printf("vips_init\n");
  int ret = VIPS_INIT("vips_wrapper"); 
  printf("vips_init done\n");
  return ret;
}

void teardown() { vips_shutdown(); }

int thumbnail(ThumbnailParams params) {
  for (unsigned long long i = 0; i < params.num_out_paths; ++i) {
    VipsImage* out;
    int ret;
    if (params.keep_aspect) {
       ret = vips_thumbnail(params.in_path, &out, params.width, NULL);
    } else {
       ret = vips_thumbnail(params.in_path, &out, params.width, "height", params.height, "crop", VIPS_INTERESTING_ATTENTION, NULL);
    }
    if (ret) {
      printf("libvips error: %s", vips_error_buffer());
      g_object_unref(out);
      return ret;
    }
    ret = vips_image_write_to_file(out, params.out_paths[i], NULL);
    g_object_unref(out);
    if (ret) {
      printf("libvips error: %s", vips_error_buffer());
      return ret;
    }
  }
  return 0;
}
