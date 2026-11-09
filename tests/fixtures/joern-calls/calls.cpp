// Source controls: some deliberately unresolved names model decompiler output.
int tick();
struct Box { int method(int); };
int known_member(Box* p) { return p->method(tick()); }
int unknown_member(Unknown* p) { return p->missing(tick()); }
int unknown_factory() { return factory()->missing(tick()); }
int indexed_member(Unknown** p, int i) { return p[i++]->missing(tick()); }
int direct_unknown(int x) { return missing(x); }
int nested_unknown() { return missing(missing(tick())); }
int direct_pointer(int (*fn)(int)) { return fn(tick()); }
int grouped_pointer(int (*fn)(int)) { return (fn)(tick()); }
int grouped_unknown(int x) { return (missing)(x); }
int lambda_call(int x) { auto fn = [x](int y) { return x+y; }; return fn(tick()); }
int short_circuit(int flag, Unknown* p) { return flag && p->missing(tick()); }
struct Object { ~Object(); };
void destructor(Object* p) { p->~Object(); }
struct VirtualBox { virtual int method(int); };
int known_virtual(VirtualBox* p) { return p->method(tick()); }
