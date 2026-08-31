#ifndef __MYRTI_VIPS_WRAPPER_H
#define __MYRTI_VIPS_WRAPPER_H

#include <stdbool.h>

int init();
void teardown();

typedef void* VipsImagePtr;
void free_vips_image(VipsImagePtr);

typedef struct ThumbnailOptions {
  const char *in_path;
  bool keep_aspect;
  int width;
  int height;
} ThumbnailParams;

typedef struct ThumbnailResult {
  VipsImagePtr image;
  int actual_width;
  int actual_height;
} ThumbnailResult;

typedef struct ImageBuffer {
  int width;
  int height;
  unsigned long size;
  const char* buf;
} ImageBuffer;

int create_thumbnail(ThumbnailParams, ThumbnailResult *);

typedef struct ImageInfo {
  int width;
  int height;
} ImageInfo;

int read_image_info(const char *path, ImageInfo *out);

int thumbnail_for_thumbhash(const char* path, int width, ImageBuffer* out);
void free_image_buffer(ImageBuffer buf);

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
  // 0 fast - 9 slow
  int effort;
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

int save_image_jpeg(VipsImagePtr img, const char* out_path, JpegSaveParams params);
int save_image_heif(VipsImagePtr img, const char* out_path, HeifSaveParams params);
int save_image_webp(VipsImagePtr img, const char* out_path);

int save_test_heif_image(const char *, HeifSaveParams);
int save_test_jpeg_image(const char *, JpegSaveParams);
int save_test_webp_image(const char *);


#endif // __MYRTI_VIPS_WRAPPER_H
