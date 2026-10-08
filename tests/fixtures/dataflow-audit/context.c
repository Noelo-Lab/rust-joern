void sink(int x);
int proto(int x);
extern void free(void*);
int empty_internal(int x) {}
int body_internal(int x) { return x; }
int empty_call(int x) { return empty_internal(x); }
int body_call(int x) { return body_internal(x); }
int proto_call(int x) { return proto(x); }
int sink_call(int x) { sink(x); return x; }
int free_call(void *x) { free(x); return 1; }
