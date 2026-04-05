/*
   minibz2
      libbz2.dll test program using the official streaming APIs.
      by Yoshioka Tsuneo (tsuneo@rr.iij4u.or.jp)
      This file is Public Domain.  Welcome any email to me.

   usage: minibz2 [-d] [-{1,2,..9}] [[srcfilename] destfilename]
*/

#define BZ_IMPORT
#include <stdio.h>
#include <stdlib.h>
#include "bzlib.h"

#ifdef _WIN32

#define BZ2_LIBNAME "libbz2.dll"

#include <windows.h>
static int BZ2DLLLoaded = 0;
static HINSTANCE BZ2DLLhLib;
int BZ2DLLLoadLibrary(void)
{
   HINSTANCE hLib;

   if (BZ2DLLLoaded == 1) return 0;
   hLib = LoadLibrary(BZ2_LIBNAME);
   if (hLib == NULL) {
      fprintf(stderr, "Can't load %s\n", BZ2_LIBNAME);
      return -1;
   }
   BZ2_bzReadOpen = GetProcAddress(hLib, "BZ2_bzReadOpen");
   BZ2_bzReadClose = GetProcAddress(hLib, "BZ2_bzReadClose");
   BZ2_bzRead = GetProcAddress(hLib, "BZ2_bzRead");
   BZ2_bzWriteOpen = GetProcAddress(hLib, "BZ2_bzWriteOpen");
   BZ2_bzWrite = GetProcAddress(hLib, "BZ2_bzWrite");
   BZ2_bzWriteClose = GetProcAddress(hLib, "BZ2_bzWriteClose");

   if (!BZ2_bzReadOpen || !BZ2_bzReadClose || !BZ2_bzRead
       || !BZ2_bzWriteOpen || !BZ2_bzWrite || !BZ2_bzWriteClose) {
      fprintf(stderr, "GetProcAddress failed.\n");
      return -1;
   }
   BZ2DLLLoaded = 1;
   BZ2DLLhLib = hLib;
   return 0;
}

int BZ2DLLFreeLibrary(void)
{
   if (BZ2DLLLoaded == 0) return 0;
   FreeLibrary(BZ2DLLhLib);
   BZ2DLLLoaded = 0;
   return 0;
}
#endif /* _WIN32 */

void usage(void)
{
   puts("usage: minibz2 [-d] [-{1,2,..9}] [[srcfilename] destfilename]");
}

static void fail_open(const char* path)
{
   printf("can't open [%s]\n", path);
   perror("reason:");
   exit(1);
}

static void fail_bzcall(const char* call, int bzerr)
{
   fprintf(stderr, "%s failed: %d\n", call, bzerr);
   exit(1);
}

static FILE* open_input(const char* path)
{
   FILE* fp;

   if (path == NULL) return stdin;
   fp = fopen(path, "rb");
   if (fp == NULL) fail_open(path);
   return fp;
}

static FILE* open_output(const char* path)
{
   FILE* fp;

   if (path == NULL) return stdout;
   fp = fopen(path, "wb");
   if (fp == NULL) fail_open(path);
   return fp;
}

int main(int argc, char *argv[])
{
   int decompress = 0;
   int level = 9;
   char *fn_r = NULL;
   char *fn_w = NULL;

#ifdef _WIN32
   if (BZ2DLLLoadLibrary() < 0) {
      fprintf(stderr, "Loading of %s failed.  Giving up.\n", BZ2_LIBNAME);
      exit(1);
   }
   printf("Loading of %s succeeded.\n", BZ2_LIBNAME);
#endif
   while (++argv, --argc) {
      if (**argv == '-' || **argv == '/') {
         char *p;

         for (p = *argv + 1; *p; p++) {
            if (*p == 'd') {
               decompress = 1;
            } else if ('1' <= *p && *p <= '9') {
               level = *p - '0';
            } else {
               usage();
               exit(1);
            }
         }
      } else {
         break;
      }
   }
   if (argc >= 1) {
      fn_r = *argv;
      argc--;
      argv++;
   }
   if (argc >= 1) {
      fn_w = *argv;
   }

   {
      int len;
      int bzerr;
      char buff[0x1000];

      if (decompress) {
         BZFILE* bzfp_r;
         FILE* fp_r = open_input(fn_r);
         FILE* fp_w = open_output(fn_w);

         bzfp_r = BZ2_bzReadOpen(&bzerr, fp_r, 0, 0, NULL, 0);
         if (bzerr != BZ_OK) fail_bzcall("BZ2_bzReadOpen", bzerr);

         while (1) {
            len = BZ2_bzRead(&bzerr, bzfp_r, buff, sizeof(buff));
            if (len > 0 && fwrite(buff, 1, len, fp_w) != (size_t)len) {
               perror("reason:");
               exit(1);
            }
            if (bzerr == BZ_OK) continue;
            if (bzerr == BZ_STREAM_END) break;
            fail_bzcall("BZ2_bzRead", bzerr);
         }

         BZ2_bzReadClose(&bzerr, bzfp_r);
         if (bzerr != BZ_OK) fail_bzcall("BZ2_bzReadClose", bzerr);
         if (fp_r != stdin) fclose(fp_r);
         if (fp_w != stdout) fclose(fp_w);
      } else {
         BZFILE* bzfp_w;
         FILE* fp_r = open_input(fn_r);
         FILE* fp_w = open_output(fn_w);

         bzfp_w = BZ2_bzWriteOpen(&bzerr, fp_w, level, 0, 0);
         if (bzerr != BZ_OK) fail_bzcall("BZ2_bzWriteOpen", bzerr);

         while ((len = fread(buff, 1, sizeof(buff), fp_r)) > 0) {
            BZ2_bzWrite(&bzerr, bzfp_w, buff, len);
            if (bzerr != BZ_OK) fail_bzcall("BZ2_bzWrite", bzerr);
         }
         if (ferror(fp_r)) {
            perror("reason:");
            exit(1);
         }

         BZ2_bzWriteClose(&bzerr, bzfp_w, 0, NULL, NULL);
         if (bzerr != BZ_OK) fail_bzcall("BZ2_bzWriteClose", bzerr);
         if (fp_r != stdin) fclose(fp_r);
         if (fp_w != stdout) fclose(fp_w);
      }
   }
#ifdef _WIN32
   BZ2DLLFreeLibrary();
#endif
   return 0;
}
