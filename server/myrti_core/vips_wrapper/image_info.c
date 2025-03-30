#include <vips/vips.h>
#include <vips/header.h>
#include <vips/image.h>
#include "vips_wrapper.h"

int read_image_info(const char *path, ImageInfo *out) {
  if (path == NULL || out == NULL) {
    return 1;
  }
  VipsImage* image = vips_image_new_from_file(path, NULL);
  if (image == NULL) {
    return 1;
  }
  int swap = vips_image_get_orientation_swap(image);
  if (swap == 0) {
    out->width = vips_image_get_width(image);
    out->height = vips_image_get_height(image);
  } else {
    out->height = vips_image_get_width(image);
    out->width = vips_image_get_height(image);
  }
  g_object_unref(image);
  return 0;
}
