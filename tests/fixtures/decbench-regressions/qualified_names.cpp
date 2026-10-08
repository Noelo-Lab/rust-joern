int stream;
int qual_assignment(int x) { ::stream = x; return x; }
int after_assignment(int x) { return x + 1; }
int qual_if(int x) { if (::stream && x) return 1; return 0; }
int qual_ns(int x) { if (::ns::stream && x) return 1; return 0; }
int qual_nonleading(int x) { if (ns::stream && x) return 1; return 0; }
int qual_call(int x) { return ::foo(x); }
int qual_ns_assignment(int x) { ::ns::stream = x; return x; }
int after_all(int x) { return x + 2; }
