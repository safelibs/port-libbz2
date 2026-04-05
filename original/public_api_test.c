#define _XOPEN_SOURCE 700

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "bzlib.h"

#define STREAM_PAYLOAD_LEN 180000U
#define BUFFER_PAYLOAD_LEN 9000U
#define FILE_PAYLOAD_LEN 64000U
#define WRAPPER_PAYLOAD_LEN 48000U
#define UNUSED_COPY_MAX BZ_MAX_UNUSED

static void fail(const char* message)
{
   fprintf(stderr, "public_api_test: %s\n", message);
   exit(1);
}

static void fail_errno(const char* message)
{
   fprintf(stderr, "public_api_test: %s: %s\n", message, strerror(errno));
   exit(1);
}

static void fail_bz(const char* call, int code)
{
   fprintf(stderr, "public_api_test: %s failed with %d\n", call, code);
   exit(1);
}

static void expect(int condition, const char* message)
{
   if (!condition) fail(message);
}

static void expect_bz(int actual, int expected, const char* call)
{
   if (actual != expected) fail_bz(call, actual);
}

static void* xmalloc(size_t size)
{
   void* ptr = malloc(size);
   if (ptr == NULL) fail("out of memory");
   return ptr;
}

static void fill_payload(unsigned char* dst, unsigned int len, unsigned int seed)
{
   static const char alphabet[] =
      "The quick brown fox jumps over the lazy dog. 0123456789\n";
   unsigned int i;

   for (i = 0; i < len; i++) {
      unsigned int mix = i + (seed * 17U) + (i / 29U) + (i / 251U);
      dst[i] = (unsigned char)alphabet[mix % (sizeof(alphabet) - 1U)];
      if ((i % 97U) >= 64U) dst[i] = (unsigned char)('A' + (mix % 23U));
   }
}

static unsigned int compressed_bound(unsigned int source_len)
{
   return source_len + (source_len / 100U) + 601U;
}

static unsigned int stream_compress(
   const unsigned char* source,
   unsigned int source_len,
   unsigned char* dest,
   unsigned int dest_cap
)
{
   bz_stream strm;
   unsigned int source_off = 0;
   unsigned int dest_off = 0;
   int ret;

   memset(&strm, 0, sizeof(strm));
   expect_bz(BZ2_bzCompressInit(&strm, 1, 0, 30), BZ_OK, "BZ2_bzCompressInit");

   while (source_off < source_len) {
      unsigned int chunk = source_len - source_off;
      if (chunk > 4093U) chunk = 4093U;
      strm.next_in = (char*)(source + source_off);
      strm.avail_in = chunk;

      while (strm.avail_in > 0) {
         unsigned int out_chunk = dest_cap - dest_off;
         if (out_chunk == 0U) fail("stream_compress overflow");
         if (out_chunk > 1537U) out_chunk = 1537U;

         strm.next_out = (char*)(dest + dest_off);
         strm.avail_out = out_chunk;
         ret = BZ2_bzCompress(&strm, BZ_RUN);
         expect_bz(ret, BZ_RUN_OK, "BZ2_bzCompress");
         dest_off += out_chunk - strm.avail_out;
      }

      source_off += chunk;
   }

   while (1) {
      unsigned int out_chunk = dest_cap - dest_off;
      if (out_chunk == 0U) fail("stream_compress finish overflow");
      if (out_chunk > 1537U) out_chunk = 1537U;

      strm.next_out = (char*)(dest + dest_off);
      strm.avail_out = out_chunk;
      ret = BZ2_bzCompress(&strm, BZ_FINISH);
      dest_off += out_chunk - strm.avail_out;

      if (ret == BZ_STREAM_END) break;
      expect_bz(ret, BZ_FINISH_OK, "BZ2_bzCompress");
   }

   expect(strm.total_in_lo32 == source_len, "stream compress input count mismatch");
   expect(strm.total_in_hi32 == 0U, "stream compress input high bits mismatch");
   expect(strm.total_out_lo32 == dest_off, "stream compress output count mismatch");

   expect_bz(BZ2_bzCompressEnd(&strm), BZ_OK, "BZ2_bzCompressEnd");
   return dest_off;
}

static unsigned int stream_decompress(
   const unsigned char* source,
   unsigned int source_len,
   unsigned char* dest,
   unsigned int dest_cap,
   int small
)
{
   bz_stream strm;
   unsigned int source_off = 0;
   unsigned int dest_off = 0;
   int ret;

   memset(&strm, 0, sizeof(strm));
   expect_bz(BZ2_bzDecompressInit(&strm, 0, small), BZ_OK, "BZ2_bzDecompressInit");

   while (1) {
      unsigned int out_chunk;

      if (strm.avail_in == 0U && source_off < source_len) {
         unsigned int chunk = source_len - source_off;
         if (chunk > 811U) chunk = 811U;
         strm.next_in = (char*)(source + source_off);
         strm.avail_in = chunk;
         source_off += chunk;
      }

      out_chunk = dest_cap - dest_off;
      if (out_chunk == 0U) fail("stream_decompress overflow");
      if (out_chunk > 997U) out_chunk = 997U;

      strm.next_out = (char*)(dest + dest_off);
      strm.avail_out = out_chunk;
      ret = BZ2_bzDecompress(&strm);
      dest_off += out_chunk - strm.avail_out;

      if (ret == BZ_STREAM_END) break;
      expect_bz(ret, BZ_OK, "BZ2_bzDecompress");
      if (source_off == source_len && strm.avail_in == 0U && strm.avail_out > 0U)
         fail("stream_decompress ended before stream end");
   }

   expect(strm.total_in_lo32 == source_len, "stream decompress input count mismatch");
   expect(strm.total_in_hi32 == 0U, "stream decompress input high bits mismatch");
   expect(strm.total_out_lo32 == dest_off, "stream decompress output count mismatch");

   expect_bz(BZ2_bzDecompressEnd(&strm), BZ_OK, "BZ2_bzDecompressEnd");
   return dest_off;
}

static unsigned int read_bz_stream(BZFILE* bzf, unsigned char* dest, unsigned int dest_cap)
{
   unsigned int total = 0;
   int bzerr = BZ_OK;
   unsigned char extra;

   while (1) {
      unsigned int chunk = dest_cap - total;
      int nread;

      if (chunk == 0U) {
         nread = BZ2_bzRead(&bzerr, bzf, &extra, 1);
         if (bzerr == BZ_STREAM_END && nread == 0) break;
         fail("read_bz_stream overflow");
      }
      if (chunk > 257U) chunk = 257U;

      nread = BZ2_bzRead(&bzerr, bzf, dest + total, (int)chunk);
      expect(nread >= 0, "BZ2_bzRead returned a negative length");
      total += (unsigned int)nread;

      if (bzerr == BZ_OK) continue;
      if (bzerr == BZ_STREAM_END) break;
      fail_bz("BZ2_bzRead", bzerr);
   }

   return total;
}

static void make_temp_path(char path[], const char* path_template)
{
   strcpy(path, path_template);
   int fd = mkstemp(path);
   if (fd < 0) fail_errno("mkstemp");
   if (close(fd) != 0) fail_errno("close");
   if (unlink(path) != 0) fail_errno("unlink");
}

static void write_all(FILE* fp, const void* data, size_t len)
{
   if (fwrite(data, 1, len, fp) != len) fail_errno("fwrite");
}

static void test_version_string(void)
{
   const char* version = BZ2_bzlibVersion();
   expect(version != NULL, "BZ2_bzlibVersion returned NULL");
   expect(version[0] != '\0', "BZ2_bzlibVersion returned an empty string");
}

static void test_core_stream_api(void)
{
   unsigned char* source = (unsigned char*)xmalloc(STREAM_PAYLOAD_LEN);
   unsigned char* compressed = (unsigned char*)xmalloc(compressed_bound(STREAM_PAYLOAD_LEN));
   unsigned char* restored = (unsigned char*)xmalloc(STREAM_PAYLOAD_LEN);
   unsigned int compressed_len;
   unsigned int restored_len;

   fill_payload(source, STREAM_PAYLOAD_LEN, 1U);
   compressed_len =
      stream_compress(source, STREAM_PAYLOAD_LEN, compressed, compressed_bound(STREAM_PAYLOAD_LEN));
   restored_len =
      stream_decompress(compressed, compressed_len, restored, STREAM_PAYLOAD_LEN, 1);

   expect(restored_len == STREAM_PAYLOAD_LEN, "stream round-trip length mismatch");
   expect(memcmp(source, restored, STREAM_PAYLOAD_LEN) == 0, "stream round-trip data mismatch");

   free(restored);
   free(compressed);
   free(source);
}

static void test_buffer_api(void)
{
   unsigned char* source = (unsigned char*)xmalloc(BUFFER_PAYLOAD_LEN);
   unsigned char* compressed = (unsigned char*)xmalloc(compressed_bound(BUFFER_PAYLOAD_LEN));
   unsigned char* restored = (unsigned char*)xmalloc(BUFFER_PAYLOAD_LEN);
   unsigned int compressed_len = compressed_bound(BUFFER_PAYLOAD_LEN);
   unsigned int restored_len;
   unsigned int tiny_len;
   int ret;

   fill_payload(source, BUFFER_PAYLOAD_LEN, 3U);

   ret = BZ2_bzBuffToBuffCompress(
      (char*)compressed,
      &compressed_len,
      (char*)source,
      BUFFER_PAYLOAD_LEN,
      9,
      0,
      0
   );
   expect_bz(ret, BZ_OK, "BZ2_bzBuffToBuffCompress");

   restored_len = BUFFER_PAYLOAD_LEN;
   ret = BZ2_bzBuffToBuffDecompress(
      (char*)restored,
      &restored_len,
      (char*)compressed,
      compressed_len,
      0,
      0
   );
   expect_bz(ret, BZ_OK, "BZ2_bzBuffToBuffDecompress");
   expect(restored_len == BUFFER_PAYLOAD_LEN, "buffer round-trip length mismatch");
   expect(memcmp(source, restored, BUFFER_PAYLOAD_LEN) == 0, "buffer round-trip data mismatch");

   restored_len = BUFFER_PAYLOAD_LEN;
   ret = BZ2_bzBuffToBuffDecompress(
      (char*)restored,
      &restored_len,
      (char*)compressed,
      compressed_len,
      1,
      0
   );
   expect_bz(ret, BZ_OK, "BZ2_bzBuffToBuffDecompress");
   expect(memcmp(source, restored, BUFFER_PAYLOAD_LEN) == 0, "small-mode buffer mismatch");

   tiny_len = 32U;
   ret = BZ2_bzBuffToBuffDecompress(
      (char*)restored,
      &tiny_len,
      (char*)compressed,
      compressed_len,
      0,
      0
   );
   expect_bz(ret, BZ_OUTBUFF_FULL, "BZ2_bzBuffToBuffDecompress");

   free(restored);
   free(compressed);
   free(source);
}

static void test_high_level_api(void)
{
   unsigned char* source = (unsigned char*)xmalloc(FILE_PAYLOAD_LEN);
   unsigned char* restored = (unsigned char*)xmalloc(FILE_PAYLOAD_LEN);
   unsigned int in_lo = 0;
   unsigned int in_hi = 0;
   unsigned int out_lo = 0;
   unsigned int out_hi = 0;
   FILE* fp;
   BZFILE* bzf;
   int bzerr;
   unsigned int restored_len;
   char path[] = "public-api-high-level-XXXXXX";
   static const char trailer[] = "TAIL";
   unsigned char first_payload[731];
   unsigned char second_payload[887];
   unsigned char first_out[sizeof(first_payload)];
   unsigned char second_out[sizeof(second_payload)];
   unsigned char unused_copy[UNUSED_COPY_MAX];
   unsigned char member1[compressed_bound(sizeof(first_payload))];
   unsigned char member2[compressed_bound(sizeof(second_payload))];
   unsigned int member1_len = sizeof(member1);
   unsigned int member2_len = sizeof(member2);
   void* unused;
   int n_unused;

   fill_payload(source, FILE_PAYLOAD_LEN, 5U);
   make_temp_path(path, "public-api-high-level-XXXXXX");

   fp = fopen(path, "wb");
   if (fp == NULL) fail_errno("fopen write");
   bzf = BZ2_bzWriteOpen(&bzerr, fp, 3, 0, 0);
   if (bzf == NULL) fail_bz("BZ2_bzWriteOpen", bzerr);
   expect_bz(bzerr, BZ_OK, "BZ2_bzWriteOpen");

   {
      unsigned int offset = 0;
      while (offset < FILE_PAYLOAD_LEN) {
         unsigned int chunk = FILE_PAYLOAD_LEN - offset;
         if (chunk > 1531U) chunk = 1531U;
         BZ2_bzWrite(&bzerr, bzf, source + offset, (int)chunk);
         expect_bz(bzerr, BZ_OK, "BZ2_bzWrite");
         offset += chunk;
      }
   }

   BZ2_bzWriteClose64(&bzerr, bzf, 0, &in_lo, &in_hi, &out_lo, &out_hi);
   expect_bz(bzerr, BZ_OK, "BZ2_bzWriteClose64");
   expect(in_lo == FILE_PAYLOAD_LEN, "BZ2_bzWriteClose64 input count mismatch");
   expect(in_hi == 0U, "BZ2_bzWriteClose64 input high bits mismatch");
   expect(out_lo > 0U, "BZ2_bzWriteClose64 output count mismatch");
   expect(out_hi == 0U, "BZ2_bzWriteClose64 output high bits mismatch");
   if (fclose(fp) != 0) fail_errno("fclose write");

   fp = fopen(path, "rb");
   if (fp == NULL) fail_errno("fopen read");
   bzf = BZ2_bzReadOpen(&bzerr, fp, 0, 0, NULL, 0);
   if (bzf == NULL) fail_bz("BZ2_bzReadOpen", bzerr);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadOpen");
   restored_len = read_bz_stream(bzf, restored, FILE_PAYLOAD_LEN);
   expect(restored_len == FILE_PAYLOAD_LEN, "BZ2_bzRead length mismatch");
   expect(memcmp(source, restored, FILE_PAYLOAD_LEN) == 0, "BZ2_bzRead data mismatch");
   BZ2_bzReadClose(&bzerr, bzf);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadClose");
   if (fclose(fp) != 0) fail_errno("fclose read");
   if (unlink(path) != 0) fail_errno("unlink");

   fill_payload(first_payload, sizeof(first_payload), 7U);
   fill_payload(second_payload, sizeof(second_payload), 11U);

   expect_bz(
      BZ2_bzBuffToBuffCompress(
         (char*)member1,
         &member1_len,
         (char*)first_payload,
         sizeof(first_payload),
         1,
         0,
         0
      ),
      BZ_OK,
      "BZ2_bzBuffToBuffCompress"
   );
   expect_bz(
      BZ2_bzBuffToBuffCompress(
         (char*)member2,
         &member2_len,
         (char*)second_payload,
         sizeof(second_payload),
         1,
         0,
         0
      ),
      BZ_OK,
      "BZ2_bzBuffToBuffCompress"
   );

   make_temp_path(path, "public-api-high-level-XXXXXX");
   fp = fopen(path, "wb");
   if (fp == NULL) fail_errno("fopen concatenated write");
   write_all(fp, member1, member1_len);
   write_all(fp, member2, member2_len);
   write_all(fp, trailer, sizeof(trailer) - 1U);
   if (fclose(fp) != 0) fail_errno("fclose concatenated write");

   fp = fopen(path, "rb");
   if (fp == NULL) fail_errno("fopen concatenated read");
   bzf = BZ2_bzReadOpen(&bzerr, fp, 0, 0, NULL, 0);
   if (bzf == NULL) fail_bz("BZ2_bzReadOpen", bzerr);
   expect(read_bz_stream(bzf, first_out, sizeof(first_out)) == sizeof(first_payload),
          "first concatenated member length mismatch");
   expect(memcmp(first_payload, first_out, sizeof(first_payload)) == 0,
          "first concatenated member data mismatch");

   BZ2_bzReadGetUnused(&bzerr, bzf, &unused, &n_unused);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadGetUnused");
   expect(n_unused == (int)(member2_len + sizeof(trailer) - 1U),
          "unexpected unused data size after first member");
   expect((unsigned int)n_unused <= UNUSED_COPY_MAX, "unused copy buffer too small");
   memcpy(unused_copy, unused, (size_t)n_unused);

   BZ2_bzReadClose(&bzerr, bzf);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadClose");

   bzf = BZ2_bzReadOpen(&bzerr, fp, 0, 1, unused_copy, n_unused);
   if (bzf == NULL) fail_bz("BZ2_bzReadOpen", bzerr);
   expect(read_bz_stream(bzf, second_out, sizeof(second_out)) == sizeof(second_payload),
          "second concatenated member length mismatch");
   expect(memcmp(second_payload, second_out, sizeof(second_payload)) == 0,
          "second concatenated member data mismatch");

   BZ2_bzReadGetUnused(&bzerr, bzf, &unused, &n_unused);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadGetUnused");
   expect(n_unused == (int)(sizeof(trailer) - 1U), "trailer unused data length mismatch");
   expect(memcmp(unused, trailer, sizeof(trailer) - 1U) == 0, "trailer unused data mismatch");

   BZ2_bzReadClose(&bzerr, bzf);
   expect_bz(bzerr, BZ_OK, "BZ2_bzReadClose");
   if (fclose(fp) != 0) fail_errno("fclose concatenated read");
   if (unlink(path) != 0) fail_errno("unlink");

   free(restored);
   free(source);
}

static void test_stdio_wrappers(void)
{
   unsigned char* source = (unsigned char*)xmalloc(WRAPPER_PAYLOAD_LEN);
   unsigned char* restored = (unsigned char*)xmalloc(WRAPPER_PAYLOAD_LEN);
   unsigned char scratch[32];
   char data_path[] = "public-api-wrapper-XXXXXX";
   char error_path[] = "public-api-wrapper-error-XXXXXX";
   BZFILE* bzf;
   unsigned int total = 0;
   int fd;
   int errnum;
   const char* errstr;

   fill_payload(source, WRAPPER_PAYLOAD_LEN, 13U);
   make_temp_path(data_path, "public-api-wrapper-XXXXXX");

   bzf = BZ2_bzopen(data_path, "w7");
   expect(bzf != NULL, "BZ2_bzopen write failed");

   while (total < WRAPPER_PAYLOAD_LEN) {
      unsigned int chunk = WRAPPER_PAYLOAD_LEN - total;
      if (chunk > 701U) chunk = 701U;
      expect(BZ2_bzwrite(bzf, source + total, (int)chunk) == (int)chunk, "BZ2_bzwrite failed");
      total += chunk;
   }
   expect(BZ2_bzflush(bzf) == 0, "BZ2_bzflush failed");
   BZ2_bzclose(bzf);

   fd = open(data_path, O_RDONLY);
   if (fd < 0) fail_errno("open");
   bzf = BZ2_bzdopen(fd, "rs");
   expect(bzf != NULL, "BZ2_bzdopen read failed");

   total = 0;
   while (1) {
      int nread = BZ2_bzread(bzf, restored + total, 389);
      if (nread < 0) fail("BZ2_bzread failed");
      if (nread == 0) break;
      total += (unsigned int)nread;
   }
   expect(total == WRAPPER_PAYLOAD_LEN, "BZ2_bzread length mismatch");
   expect(memcmp(source, restored, WRAPPER_PAYLOAD_LEN) == 0, "BZ2_bzread data mismatch");
   expect(BZ2_bzread(bzf, scratch, sizeof(scratch)) == 0, "BZ2_bzread EOF mismatch");
   BZ2_bzclose(bzf);
   if (unlink(data_path) != 0) fail_errno("unlink");

   make_temp_path(error_path, "public-api-wrapper-error-XXXXXX");
   bzf = BZ2_bzopen(error_path, "w1");
   expect(bzf != NULL, "BZ2_bzopen error-path write failed");
   expect(BZ2_bzread(bzf, scratch, sizeof(scratch)) == -1, "bzread on write handle should fail");
   errstr = BZ2_bzerror(bzf, &errnum);
   expect(errnum == BZ_SEQUENCE_ERROR, "BZ2_bzerror code mismatch");
   expect(strcmp(errstr, "SEQUENCE_ERROR") == 0, "BZ2_bzerror string mismatch");
   BZ2_bzclose(bzf);
   if (unlink(error_path) != 0) fail_errno("unlink");

   free(restored);
   free(source);
}

int main(void)
{
   test_version_string();
   test_core_stream_api();
   test_buffer_api();
   test_high_level_api();
   test_stdio_wrappers();
   puts("public_api_test: ok");
   return 0;
}
