int bare_name(x) { return x; }
int bare_type(off_t) { return 0; }
int declare_inferred(off_t x) { return 0; }
int after_inferred(off_t) { return 0; }
int unnamed_proto(off_t);
int named_proto(off_t x);
int primitive_proto(int);
int qualifier_unknown(volatile mystery) { return 0; }
int bool_named(bool x) { return x; }
int bool_ptr(bool *x) { return 0; }
int bool_const(const bool x) { return x; }
int bool_volatile(volatile bool x) { return x; }
int const_param(const int x) { return x; }
int volatile_param(volatile int x) { return x; }
int unsigned_param(unsigned int x) { return x; }
int unsignedlong_param(unsigned long x) { return x; }
int longunsigned_param(long unsigned x) { return x; }
int longlong_param(long long x) { return x; }
int longunsignedlong_param(unsigned long long x) { return x; }
unsigned long unsignedlong_ret(void) { return 0; }
long unsigned longunsigned_ret(void) { return 0; }
long long longlong_ret(void) { return 0; }
unsigned long long unsignedlonglong_ret(void) { return 0; }
int * const pointer_const_ret(void) { return 0; }
int * volatile pointer_volatile_ret(void) { return 0; }
const volatile int *cv_ret(void) { return 0; }
const int *const_declared(void);
const int *const_declared(void) { return 0; }
int c_bool_token(_Bool) { return 0; }
