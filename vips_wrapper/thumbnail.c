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
  for (int i = 0; i < params.num_out_paths; ++i) {
    // printf("in: %s, out: %s\n", params.in_path, params.out_paths[i]);
    VipsImage* out;
    int ret = vips_thumbnail(params.in_path, &out, params.width, /* params.height, VIPS_INTERESTING_ATTENTION, */ NULL);
    if (ret) {
      printf("libvips error: %s", vips_error_buffer());
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
