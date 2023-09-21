#ifndef __VIPS_WRAPPER_H
#define __VIPS_WRAPPER_H

int init();
void teardown();

typedef struct ThumbnailOptions {
  const char *in_path;
  const char *const *out_paths;
  unsigned long long num_out_paths;
  int keep_aspect;
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

int convert_heif(const char *, const char *, HeifSaveParams);

typedef struct JpegSaveParams {
  int quality;
} JpegSaveParams;

int convert_jpeg(const char *, const char *, JpegSaveParams);

#endif // __VIPS_WRAPPER_H
