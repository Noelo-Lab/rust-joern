typedef long Known;
struct Tag { int x; };
int unknown_single(off_t) { return 0; }
int unknown_use(off_t) { return off_t; }
int unknown_second(size_t) { return 0; }
int unknown_two(off_t, size_t) { return 0; }
int unknown_mixed(off_t, int x) { return x; }
int unknown_ptr(off_t *) { return 0; }
int unknown_named(off_t x) { return 0; }
int unknown_qualified(const off_t) { return 0; }
int known_single(Known) { return 0; }
int known_named(Known x) { return x; }
int primitive_single(int) { return 0; }
int primitive_two(unsigned long, float) { return 0; }
int void_single(void) { return 0; }
int bool_single(bool) { return 0; }
int tagged(struct Tag) { return 0; }
const char *const_ret(void) { return 0; }
char const *trailing_const_ret(void) { return 0; }
const char * const outer_const_ret(void) { return 0; }
volatile char *volatile_ret(void) { return 0; }
const int const_scalar_ret(void) { return 0; }
static const char *static_const_ret(void) { return 0; }
extern const char *extern_const_ret(void);
bool boolean_ret(void) { return 1; }
bool *boolean_ptr_ret(void) { return 0; }
const Known *typedef_const_ret(void) { return 0; }
using KnownCpp = long;
int known_cpp_single(KnownCpp) { return 0; }
const Tag *cpp_const_ret(void) { return 0; }
const Tag &cpp_const_ref(void) { return *(Tag*)0; }
