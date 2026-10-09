int work(int);
typedef unsigned int uint32_t;
int numeric_callee(int n) { void (*0x1160)() (); work(n); return n; }
int after_numeric_callee(int n) { work(n); return n; }
int typed_numeric_callee(int n) { void (*0x1160)(uint32_t) (n); work(n); return n; }
int after_typed_numeric_callee(int n) { work(n); return n; }
int numeric_local(int n) { int 0x1160; work(n); return n; }
int after_numeric_local(int n) { work(n); return n; }
int valid_pointer_local(int n) { void (*local)(uint32_t); work(n); return n; }
int after_valid_pointer_local(int n) { work(n); return n; }
int valid_pointer_call(int n) { ((void (*)(uint32_t))0x1160)(n); work(n); return n; }
int after_valid_pointer_call(int n) { work(n); return n; }
int valid_pointer_parameter(void (*cb)(uint32_t), int n) { cb(n); return n; }
int after_valid_pointer_parameter(int n) { work(n); return n; }
