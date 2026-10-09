int work(int);
extern void wrapper((stdcall)) target(int n);
void wrapper((regparm(2))) wrapper(int n) { if(n) work(n); }
int after_wrapper(int n) { return work(n); }
extern int other((plain_word)) second(int n);
int other((plain_call(2))) other(int n) { if(n) work(n); return 0; }
int after_other(int n) { return work(n); }
int grouped_word((unknown)) { return 1; }
int after_grouped_word(int n) { return work(n); }
int valid(int (*fn)(int),int n) { return fn(n); }
int after_valid(int n) { return work(n); }
#define empty(n)
int empty(1) empty(int n) { return n; }
int after_erased_header(int n) { return work(n); }
