int work(int);
_Noreturn void global_leading(int n) { work(n); }
int ordinary_outer(int n) { int child(int x) { return work(x); } return child(n); }
int leading_outer(int n) { _Noreturn void leading_child(int x) { work(x); } leading_child(n); return n; }
int storage_outer(int n) { static _Noreturn void storage_child(int x) { work(x); } storage_child(n); return n; }
int prototype_outer(int n) { extern _Noreturn void local_proto(int); local_proto(n); return n; }
int proto_leading_outer(int n) { _Noreturn void leading_proto(int); leading_proto(n); return n; }
int after_all(int n) { return work(n); }
