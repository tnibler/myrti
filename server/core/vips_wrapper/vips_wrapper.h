#ifndef __VIPS_WRAPPER_H
#define __VIPS_WRAPPER_H

#include <stdbool.h>

int init();
void teardown();

typedef struct ThumbnailOptions {
  const char *in_path;
  const char *const *out_paths;
  unsigned long long num_out_paths;
  bool keep_aspect;
  int width;
  int height;
} ThumbnailParams;

int thumbnail(ThumbnailParams);

typedef struct ImageInfo {
  int width;
  int height;
} ImageInfo;

int read_image_info(const char *path, ImageInfo *out);

typedef struct HeifSaveParams {
  int quality;
  int lossless;
  int bit_depth;
  // VipsForeignHeifCompression
  // 1 = VIPS_FOREIGN_HEIF_COMPRESSION_HEVC
  // 2 = VIPS_FOREIGN_HEIF_COMPRESSION_AVC
  // 3 = VIPS_FOREIGN_HEIF_COMPRESSION_JPEG
  // 4 = VIPS_FOREIGN_HEIF_COMPRESSION_AV1
  int compression;
} HeifSaveParams;

typedef struct Scale {
  bool do_scale;
  double scale;
} Scale;

typedef struct ConvertHeifResult {
  int err;
  int width;
  int height;
} ConvertHeifResult;

ConvertHeifResult convert_heif(const char *, const char *, HeifSaveParams,
                               Scale);

typedef struct JpegSaveParams {
  int quality;
} JpegSaveParams;

typedef struct ConvertJpegResult {
  int err;
  int width;
  int height;
} ConvertJpegResult;

ConvertJpegResult convert_jpeg(const char *, const char *, JpegSaveParams,
                               Scale);

int save_test_heif_image(const char *, HeifSaveParams);
int save_test_jpeg_image(const char *, JpegSaveParams);
int save_test_webp_image(const char *);
#endif // __VIPS_WRAPPER_H
