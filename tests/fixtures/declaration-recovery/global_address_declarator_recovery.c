typedef unsigned long uintptr_t;
typedef struct Profile Profile;
struct Profile { int field; };
extern Profile *(*(Profile **)(uintptr_t)0x2000);
int global_use(int x) { if((*(Profile **)(uintptr_t)0x2000)->field) return x; return 0; }
extern int (*valid_callback)(int);
int callback_use(int x) { if(valid_callback(x)) return x; return 0; }
int valid_grouped(int);
int (valid_grouped)(int x) { return x; }
typedef Profile *Pointer;
Pointer ordinary_pointer;
int ordinary_use(int x) { if(ordinary_pointer->field) return x; return 0; }
