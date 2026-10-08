void sink(int x);
int proto(int x);
extern "C" void free(void*);
int empty_internal(int x) {}
int body_internal(int x) { return x; }
struct S { int decl(int x); int empty(int x) {} int body(int x) {return x;} };
int empty_call(int x) { return empty_internal(x); }
int body_call(int x) { return body_internal(x); }
int proto_call(int x) { return proto(x); }
int sink_call(int x) { sink(x); return x; }
int free_call(void *x) { free(x); return 1; }
int empty_member(S *s,int x) { return s->empty(x); }
int body_member(S *s,int x) { return s->body(x); }
int proto_member(S *s,int x) { return s->decl(x); }
