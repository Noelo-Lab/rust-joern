int work(int);
int decorated((regparm(1))) decorated(int n) { if(n) return work(n); return 0; }
int after_decorated(int n) { return work(n); }
int arbitrary((something(2))) arbitrary(int n) { if(n) return work(n); return 0; }
int after_arbitrary(int n) { return work(n); }
int single((regparm(2))) { if(1) work(1); return 0; }
int after_single(int n) { return work(n); }
int grouped_parameter((int n)) { return work(n); }
int after_grouped(int n) { return work(n); }
int valid_function_pointer(int (*fn)(int),int n) { return fn(n); }
int after_valid(int n) { return work(n); }
