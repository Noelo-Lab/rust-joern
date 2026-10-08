int foo(int n);
int cast_deref(int *p) { return (uch)(*p); }
int cast_chain(int n) { return (float8)(float4)n; }
int cast_builtin(int n) { return (undefined10)(long double)1; }
int cast_group_call(int n) { return (unknown)(foo(n)); }
int cast_callback_parameter(int n, int (*callback)(int)) { return (callback)(n); }
int cast_callback_unknown(int n) { return (callback)(n); }
int cast_chain_paren(int n) { return (float8)((float4)n); }
int cast_chain_group(int n) { return (float8)(float4)(n); }
