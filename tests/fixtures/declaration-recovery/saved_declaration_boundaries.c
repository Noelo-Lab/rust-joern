int work(int);
struct Named { int member; };
int anonymous_tag(int n) { struct <anonymous> value; return work(n); }
int after_anonymous_tag(int n) { struct Named value; value.member=n; return value.member; }
int inferred_logical(int n) { Unknown local; Unknown && !n; return work(n); }
int inferred_binary_logical(int n) { Unknown local; Unknown && n; return work(n); }
int bound_logical(int n) { n && !n; return work(n); }
typedef int Actual;
int typedef_logical(int n) { Actual && !n; return work(n); }
int valid_pointer(int n) { Unknown *pointer; return work(n); }
int valid_cpp_reference(int n) { Actual &&ref = (Actual&&)n; return ref; }
Problem: missing entries.
Refresh the units/records first.
int consumed_generic(int n) { if(n) { n++; } return work(n); }
int after_generic(int n) { return work(n); }
Error: no data available.
Please analyze the function/binary first.
int consumed_error(int n) { if(n) { n++; } return work(n); }
int after_error(int n) { return work(n); }
int ordinary_label(int n) { goto end; end: return n; }
