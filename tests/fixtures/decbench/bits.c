   typedef void *voidp;
       












       
extern int rpl_fclose (FILE *stream) __attribute__ ((__nonnull__ (1)));
extern int _gl_cxxalias_dummy;




extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;




extern int _gl_cxxalias_dummy;
extern FILE * fdopen (int fd, const char *mode) __attribute__ ((__nonnull__ (2))) 
__attribute__ ((__malloc__ (
rpl_fclose
, 
1
)))

                                                                          
                                                                         ;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern int rpl_fflush (FILE *gl_stream);
extern int _gl_cxxalias_dummy;




extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                                   ;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern FILE * fopen (const char *
__restrict__ 
filename, const char *
__restrict__ 
mode) __attribute__ ((__nonnull__ (1, 2))) 
__attribute__ ((__malloc__ (
rpl_fclose
, 
1
)))

                                                                             
                                                                            ;
extern int _gl_cxxalias_dummy
                                                                        ;


extern int _gl_cxxalias_dummy;
extern int fpurge (FILE *gl_stream) __attribute__ ((__nonnull__ (1)));

extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                                       ;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy

                                          ;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                                            ;


extern int _gl_cxxalias_dummy;
extern int rpl_fseek (FILE *fp, long offset, int whence) __attribute__ ((__nonnull__ (1)))
                                                    ;
extern int _gl_cxxalias_dummy;




extern int _gl_cxxalias_dummy;
extern int rpl_fseeko (FILE *fp, off_t offset, int whence) __attribute__ ((__nonnull__ (1)))
                                                     ;
extern int _gl_cxxalias_dummy;







extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy

                                          ;
extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern FILE * tmpfile (void) 
__attribute__ ((__malloc__ (
rpl_fclose
, 
1
)))
                                                                     
                                                                    ;
extern int _gl_cxxalias_dummy

                                                                   ;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                                   ;


extern int _gl_cxxalias_dummy;
       
       


struct __time_t_must_be_integral {
  unsigned int __floating_time_t_unsupported : (time_t) 1;
};
extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
       







extern void free (void *);
extern int _gl_cxxalias_dummy
                                                                 ;

extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy

                                                                             ;



extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                          ;
extern int _gl_cxxalias_dummy

                               ;
extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy

                                                                  ;


extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy
                                                                     ;
extern int _gl_cxxalias_dummy;
extern char * strdup (char const *__s) __attribute__ ((__nonnull__ (1))) 
__attribute__ ((__malloc__)) __attribute__ ((__malloc__ (free, 1)))


                                                                  
                                                                 ;

extern int _gl_cxxalias_dummy;

extern int _gl_cxxalias_dummy;
extern char * strndup (char const *__s, size_t __n) __attribute__ ((__nonnull__ (1))) 
__attribute__ ((__malloc__)) __attribute__ ((__malloc__ (free, 1)))


                                                                  
                                                                 ;
extern int _gl_cxxalias_dummy;


extern int _gl_cxxalias_dummy;
extern int rpl_strerror_r (int errnum, char *buf, size_t buflen) __attribute__ ((__nonnull__ (2)))
                                                         ;
extern int _gl_cxxalias_dummy;
extern int _gl_cxxalias_dummy;





typedef unsigned char uch;
typedef unsigned short ush;
typedef unsigned long ulg;
extern int method;
extern uch inbuf[];
extern uch outbuf[];
extern ush d_buf[];
extern uch window[];




   extern ush prev[];







extern unsigned insize;
extern unsigned inptr;
extern unsigned outcnt;
extern int rsync;

extern off_t bytes_in;
extern off_t bytes_out;
extern off_t header_bytes;

extern int ifd;
extern int ofd;
extern char ifname[];
extern char ofname[];
extern char *program_name;

extern struct timespec time_stamp;
extern off_t ifile_size;

typedef int file_t;
extern int exit_code;
extern int quiet;
extern int level;
extern int test;
extern int to_stdout;
extern int save_orig_name;
extern int zip (int in, int out);
extern int file_read (char *buf, unsigned size);


extern ulg unzip_crc;
extern int unzip (int in, int out);
extern int check_zipfile (int in);


extern int unpack (int in, int out);


extern int unlzh (int in, int out);


extern 
      _Noreturn 
               void abort_gzip (void);


extern off_t deflate (int pack_level);


extern void ct_init (ush *attr, int *method);
extern int ct_tally (int dist, int lc);
extern off_t flush_block (char *buf, ulg stored_len, int pad, int eof);






extern void bi_init (file_t zipfile);
extern void send_bits (int value, int length);
extern unsigned bi_reverse (unsigned value, int length) __attribute__ ((__const__));
extern void bi_windup (void);
extern void copy_block (char *buf, unsigned len, int header);
extern int (*read_buf) (char *buf, unsigned size);


extern int copy (int in, int out);
extern ulg updcrc (const uch *s, unsigned n);
extern ulg getcrc (void) __attribute__ ((__pure__));
extern void setcrc (ulg c);
extern void clear_bufs (void);
extern int fill_inbuf (int eof_ok);
extern void flush_outbuf (void);
extern void flush_window (void);
extern void write_buf (int fd, voidp buf, unsigned cnt);
extern int read_buffer (int fd, voidp buf, unsigned int cnt);
extern char *strlwr (char *s);
extern char *gzip_base_name (char *fname) __attribute__ ((__pure__));
extern int xunlink (char *fname);
extern void make_simple_name (char *name);
extern char *add_envopt (int *argcp, char ***argvp, char const *env);
extern 
      _Noreturn 
               void gzip_error (char const *m);
extern 
      _Noreturn 
               void xalloc_die (void);
extern void warning (char const *m);
extern 
      _Noreturn 
               void read_error (void);
extern 
      _Noreturn 
               void write_error (void);
extern void display_ratio (off_t num, off_t den, FILE *file);
extern void fprint_off (FILE *, off_t, int);


extern int inflate (void);
static file_t zfile;


static

unsigned short bi_buf;
static

int bi_valid;




int (*read_buf) (char *buf, unsigned size);
void bi_init (zipfile)
    file_t zipfile;
{
    zfile = zipfile;
    bi_buf = 0;
    bi_valid = 0;







    if (zfile != (-1)) {
        read_buf = file_read;
    }
}





void send_bits(value, length)
    int value;
    int length;
{
    if (bi_valid > (int)(8 * 2*sizeof(char)) - length) {
        bi_buf |= (value << bi_valid);
        { if (outcnt < 0x40000 -2) { outbuf[outcnt++] = (uch) ((bi_buf) & 0xff); outbuf[outcnt++] = (uch) ((ush)(bi_buf) >> 8); } else { {outbuf[outcnt++]=(uch)((uch)((bi_buf) & 0xff)); if (outcnt==0x40000) flush_outbuf();}; {outbuf[outcnt++]=(uch)((uch)((ush)(bi_buf) >> 8)); if (outcnt==0x40000) flush_outbuf();}; } };
        bi_buf = (ush)value >> ((8 * 2*sizeof(char)) - bi_valid);
        bi_valid += length - (8 * 2*sizeof(char));
    } else {
        bi_buf |= value << bi_valid;
        bi_valid += length;
    }
}






unsigned bi_reverse(code, len)
    unsigned code;
    int len;
{
    register unsigned res = 0;
    do {
        res |= code & 1;
        code >>= 1, res <<= 1;
    } while (--len > 0);
    return res >> 1;
}




void bi_windup()
{
    if (bi_valid > 8) {
        { if (outcnt < 0x40000 -2) { outbuf[outcnt++] = (uch) ((bi_buf) & 0xff); outbuf[outcnt++] = (uch) ((ush)(bi_buf) >> 8); } else { {outbuf[outcnt++]=(uch)((uch)((bi_buf) & 0xff)); if (outcnt==0x40000) flush_outbuf();}; {outbuf[outcnt++]=(uch)((uch)((ush)(bi_buf) >> 8)); if (outcnt==0x40000) flush_outbuf();}; } };
    } else if (bi_valid > 0) {
        {outbuf[outcnt++]=(uch)(bi_buf); if (outcnt==0x40000) flush_outbuf();};
    }
    bi_buf = 0;
    bi_valid = 0;



}





void copy_block(buf, len, header)
    char *buf;
    unsigned len;
    int header;
{
    bi_windup();

    if (header) {
        { if (outcnt < 0x40000 -2) { outbuf[outcnt++] = (uch) (((ush)len) & 0xff); outbuf[outcnt++] = (uch) ((ush)((ush)len) >> 8); } else { {outbuf[outcnt++]=(uch)((uch)(((ush)len) & 0xff)); if (outcnt==0x40000) flush_outbuf();}; {outbuf[outcnt++]=(uch)((uch)((ush)((ush)len) >> 8)); if (outcnt==0x40000) flush_outbuf();}; } };
        { if (outcnt < 0x40000 -2) { outbuf[outcnt++] = (uch) (((ush)~len) & 0xff); outbuf[outcnt++] = (uch) ((ush)((ush)~len) >> 8); } else { {outbuf[outcnt++]=(uch)((uch)(((ush)~len) & 0xff)); if (outcnt==0x40000) flush_outbuf();}; {outbuf[outcnt++]=(uch)((uch)((ush)((ush)~len) >> 8)); if (outcnt==0x40000) flush_outbuf();}; } };



    }



    while (len--) {
        {outbuf[outcnt++]=(uch)(*buf++); if (outcnt==0x40000) flush_outbuf();};
    }
}
