typedef unsigned short u16; typedef unsigned char u8; typedef unsigned long uptr;
typedef void *io_t; typedef struct prof { int a; } prof_t; typedef struct { int count; } data_t;
int label_declaration(int a, int b) {
    int x = 1;
    {
        bool cond:10_1 = !(a | b);
        x = 1;
        b = cond:10_1;
    }
    bool cond:3;
    do {
        cond:3 = !x;
        if (cond:3) break;
    } while (a);
    return x + b;
}
int empty_initializer(int a, int b) {
    int x = (a - 1) * b;
    bool o_1 = /* unimplemented */;
    int o_2 = ;
    if (o_1)
        x = 0x7fffffff;
    return x;
}
int64_t missing_semicolon(uint32_t arg1) {
    int64_t var_38h;
    signed int64_t var_10h;
    unsigned T v1 = arg1;
    signed int64_t v2 = arg1, v3;
    T U v4;
    rdi = arg1;
    var_10h = rdi;
    return var_10h + v1 + v2;
}
int failed_declarator(int tag, int pos) {
    u16 *const (*(u16 *)(uptr)0x200180c8u) = (u16 *)(uptr)0x200180c6;
    io_t (*(io_t *)(uptr)0x2000be44u) = source(tag);
    prof_t *(*(prof_t **)(uptr)0x200086c8u) = *(prof_t **)(uptr)0x200086c8;
    const data_t *(*(data_t *)(uptr)0x20012e58) =
        (const data_t *)(uptr)0x20012e58;
    extern prof_t (*(data_t *)(uptr)0x10002db0u);
    extern prof_t *(*(const prof_t *const *)(uptr)0x200086c8);
    u8 (*table)[3] = (u8 (*)[3])(uptr)0x20010a1c;
    return pos + ((*(data_t *)(uptr)0x20012e58)->count <= 3 ? 1 : -1) + (*table)[1];
}
int leading_identifier(int n) { _Noreturn void leading_proto(int); leading_proto(n); return n; }
int star_literal(int a) {
    *0x40040014 = 0x2;
    while (*0x40040010 >= 0) {}
    x = *0x3c + 1;
    if (*0x10 == a) a = 1;
    a = call(a, *0x20);
    a = *'c';
    a = *"str";
    a = (T)*5;
    a = *(5);
    for (a = *5; a < *6; a++) { a--; }
    switch (*7) { case 1: a = 2; }
    do { a--; } while (*9);
    return a ? *0x30 : 0;
}
int64_t indirect_calls(int64_t arg1) {
    rdi = arg1;
    uint64_t (*0x00014200)() ();
    uint64_t (*rbx + 0x48)() ();
    uint64_t (*rbx)();
    return rdi;
}
long literal_return(long p0) { return (long)(*28 == p0); }
struct sized { int a; };
int typeof_casts(long n, int precision) {
  int a = ! ((__typeof__ (n)) 0 < (__typeof__ (n)) -1);
  int b = (__typeof__ (off_t)) -1 < 0;
  int c = (typeof (precision)) +1;
  int d = (__typeof__ (n)) &a;
  int e = ((sizeof(struct sized))+((typeof(sizeof(struct sized)))(4)-1));
  return a + b + c + d + e;
}
int trailing_attributes(int n) {
  static char end[0] __attribute__((__aligned__(4))) __attribute__((unused));
  static char start[2] __attribute__((unused)) = { 1, 2 };
  int plain __attribute__((unused)) = n;
  int arr[n] __attribute__((unused));
  return plain + start[0] + arr[0];
}
int extension_prefix(int v, int b) {
  int a = __extension__ ({ int _x = (v); _x > 0 ? _x : -_x; }) < 5 && __extension__ ({ int _y = b; _y; }) < v;
  a = __extension__ v + 1;
  return a;
}
int case_range(int c) {
  switch (c) {
    case '0' ... '9': return 1;
    case 32 ... 47: return 2;
    default: return 0;
  }
}
int table[64];
int call_shaped_declarations(int a1, int v8) {
  int v5; int v17[40];
  BYTE2(table[a1 + 32]) = v8;
  LOWORD(v17[0]) = 16877;
  LOBYTE(v5) = 1;
  F(*p) = v8;
  F(r[1][2]) = v8;
  F(s.x) = v8;
  F(u + 1) = v8;
  return v5 + v17[2];
}
undefined8 pointer_before_binding(undefined1 *param_1) {
  undefined1 *before;
  before = param_1;
  return 0;
}
undefined8 binds_external_function(long p) {
  undefined1 (*pauVar4) [16];
  pauVar4 = (undefined1 (*) [16])p;
  return 0;
}
undefined8 pointer_after_binding(undefined1 *param_1) {
  undefined1 *after;
  undefined1 **after2;
  undefined1 *third, *fourth;
  undefined1 *fifth = param_1;
  undefined4 *other;
  after = param_1;
  helper(param_1);
  helper *x1;
  helper3 *x3;
  return 0;
}
typedef unsigned char known_t;
int grouped_initializer(long p, int a) {
  u8 (*tmp)[3] = (u8 (*)[3])p;
  known_t (*ktmp)[3] = (known_t (*)[3])p;
  u8 (*plain)[3];
  return (*tmp)[1] + (*ktmp)[2] + a;
}
