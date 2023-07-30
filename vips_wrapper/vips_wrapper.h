#ifndef __VIPS_WRAPPER_H
#define __VIPS_WRAPPER_H

typedef struct ThumbnailOptions {
  const char *in_path;
  const char *const *out_paths;
  unsigned long long num_out_paths;
  int keep_aspect;
  int width;
  int height;
} ThumbnailParams;

int init();
void teardown();
int thumbnail(ThumbnailParams);

#endif // __VIPS_WRAPPER_H
