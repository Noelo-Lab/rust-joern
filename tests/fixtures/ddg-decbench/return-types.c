typedef int T; struct S { int a; }; enum E { E1 }; union U { int a; };
unsigned long def_unsigned_long(void) { return 0; }
unsigned long proto_unsigned_long(void);
unsigned long long proto_unsigned_long_long(void);
unsigned short int *proto_unsigned_short_pointer(void);
const unsigned long proto_const_unsigned_long(void);
volatile int proto_volatile(void);
register int proto_register(void);
static inline unsigned long proto_static_inline(void);
char *const proto_const_pointer(void);
const void *const *proto_const_pointer_pointer(void);
volatile T *proto_volatile_named(void);
T *const proto_named_const_pointer(void);
T *def_named_pointer(void) { return 0; }
const T *def_const_named_pointer(void) { return 0; }
undefined4 *def_unknown_pointer(void) { return 0; }
struct S def_struct(void) { struct S s; return s; }
struct S *def_struct_pointer(void) { return 0; }
const struct S *def_const_struct_pointer(void) { return 0; }
struct S const *def_struct_const_pointer(void) { return 0; }
enum E def_enum(void) { return E1; }
union U *def_union_pointer(void) { return 0; }
struct timespec __attribute__ ((__pure__)) def_struct_attribute(int a) { struct timespec t; return t; }
wchar_t *def_wide(wchar_t *s) { return s; }
DWORD __attribute__((__stdcall__)) proto_attribute(LPVOID);
DWORD __attribute__((__stdcall__)) def_attribute(LPVOID p) { return 0; }
void void def_repeated_void(int a) { }
int int def_repeated_int(int a) { return a; }
void void proto_repeated_void(int a);
static inline T (*def_array_pointer(void))[4] { return 0; }
int (def_grouped)(int x) { return x; }
long long (**def_function_pointer_pointer())(long long) { return 0; }
long long (*def_function_pointer(int a))(long long, wchar_t *) { return 0; }
char *(*def_function_pointer_types(void))(const char *, unsigned long *, struct S *) { return 0; }
T (*def_named_function_pointer(void))(int) { return 0; }
